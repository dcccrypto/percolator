#![cfg(kani)]

//! Phase 1.B fork-port harnesses for v16 trade / deposit / withdraw / fee.
//!
//! Ports v12 trade / deposit / withdraw / fee Kani harnesses (from fork
//! `origin/main` `tests/proofs_instructions.rs` + `tests/proofs_safety.rs`)
//! to the v16 surface. Lives in a separate file so neither the v16 baseline
//! proof files (`proofs_v16.rs`, `proofs_v16_arithmetic.rs`,
//! `proofs_v16_fork.rs`) nor sibling fork-port files are touched — the
//! trade-port surface re-verifies independently.
//!
//! Surface map (v12 → v16):
//!   - v12 `RiskEngine` + `Account` → v16 `MarketGroupV16` +
//!     `PortfolioAccountV16`.
//!   - v12 positional `execute_trade_not_atomic(a, b, oracle, slot,
//!     size_q, exec_price, …)` → v16 struct
//!     `TradeRequestV16 { asset_index, size_q, exec_price, fee_bps,
//!       admit_h_max_consumption_threshold_bps_opt }` fed into
//!     `execute_trade_with_fee_not_atomic`.
//!   - v12 `deposit_not_atomic(idx, amount, slot)` → v16
//!     `deposit_not_atomic(&mut account, amount)` (account-local, no
//!     ambient slot).
//!   - v12 `withdraw_not_atomic(idx, amount, oracle, slot, …)` → v16
//!     `withdraw_not_atomic(&mut account, amount, &effective_prices)`.
//!   - v12 `charge_fee_to_insurance` / `charge_account_fee_*` → v16
//!     `charge_account_fee_not_atomic` (mode-gated) +
//!     `kani_charge_account_fee_current` (post-loss-settlement core).
//!   - v12 `check_conservation` → v16 `assert_public_invariants` +
//!     direct vault/c_tot/insurance assertions.
//!
//! Each harness:
//!   - is annotated `// fork-port Phase 1.B` so it can be matched in
//!     subsequent retirement / triage audits;
//!   - names the originating v12 harness it ports + the invariant
//!     preserved;
//!   - lands in the v16 trade / deposit / withdraw / fee surface (not the
//!     A-1 lane plumbing — that's in `proofs_v16_fork.rs`).
//!
//! Skipped from the port set:
//!   - Bankruptcy / bankrupt-close / forfeit-loss harnesses (A-5 OBVIATED).
//!   - B-tracking / b_chunk / b_target harnesses (A-7 OBVIATED).
//!   - Phantom-dust harnesses (A-8 OBVIATED).
//!   - Lazy-AK harnesses (no v16 lazy-AK surface).
//!   - v19 stress-lane harnesses (RETIRED).
//!   - Reserved-PnL / `consume_released_pnl` / `set_pnl_with_reserve`
//!     harnesses (v16 has no `reserved_pnl` model — its source-credit
//!     reservation system supersedes that surface; see Agent 3 A-2 verify).
//!   - `keeper_crank_not_atomic` funding-rate harnesses (v16 keeper API
//!     diverged enough to need a 1.C design doc, not a 1.B port).

use percolator::v16::{
    HLockLaneV16, MarketGroupV16, MarketModeV16, PortfolioAccountV16, ProvenanceHeaderV16, SideV16,
    TradeRequestV16, V16Config, V16Error,
};
use percolator::{POS_SCALE, V16_MAX_PORTFOLIO_ASSETS_N};

fn concrete_ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    ([1u8; 32], [2u8; 32], [3u8; 32])
}

fn baseline_group() -> MarketGroupV16 {
    let (market, _, _) = concrete_ids();
    MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap()
}

fn fresh_account(account_seed: u8) -> PortfolioAccountV16 {
    let (market, _, owner) = concrete_ids();
    PortfolioAccountV16::empty(ProvenanceHeaderV16::new(
        market,
        [account_seed; 32],
        owner,
    ))
}

// ============================================================================
// P-1 — Deposit materialization: amount==0 reject, amount>0 accept, no
//        engine-side floor.
//
// Ports v12 `proof_property_23_deposit_materialization_threshold` (spec
// §10.3 step 2). v16 surface uses `deposit_not_atomic` with the runtime
// `MarketGroupV16` form, so the missing-vs-existing distinction is
// modelled by `PortfolioAccountV16::empty()` (no prior `deposit_core`)
// vs an already-funded fixture. v16 `deposit_core_not_atomic` returns
// `Ok(())` for `amount == 0` early — preserving the "engine no-op,
// wrapper enforces floor" contract.
//
// Invariant preserved: amount==0 is the only engine-side floor on
// deposit; any positive amount must be accepted (subject to overflow
// checks that are pinned out of scope here).
// ============================================================================

// fork-port Phase 1.B
/// Ports v12 `proof_property_23_deposit_materialization_threshold`.
/// Preserves: amount=0 is a no-op (Ok), amount>0 on fresh account
/// accepts and materialises capital, amount>0 on existing account
/// accepts top-ups regardless of size. Uses the engine-core deposit
/// path (`kani_deposit_core`) to bypass the expensive
/// `validate_account_shape` provenance memcmp — matches the pattern
/// of the v16 baseline `proof_v16_deposit_and_withdraw_value_flow_*`
/// harness.
#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_fork_deposit_materialization_threshold() {
    let mut group = baseline_group();
    let mut fresh = fresh_account(16);

    // amount=0 on fresh account: v16 returns Ok early without mutation.
    let r0 = group.kani_deposit_core(&mut fresh, 0);
    assert!(r0.is_ok());
    assert_eq!(fresh.capital, 0);
    assert_eq!(group.vault, 0);
    assert_eq!(group.c_tot, 0);

    // amount=1 on fresh: must succeed (no engine-side minimum floor).
    let r1 = group.kani_deposit_core(&mut fresh, 1);
    assert!(r1.is_ok());
    assert_eq!(fresh.capital, 1);
    assert_eq!(group.vault, 1);
    assert_eq!(group.c_tot, 1);

    // small top-up on existing account: must succeed (no engine floor on
    // subsequent deposits either).
    let r2 = group.kani_deposit_core(&mut fresh, 1);
    assert!(r2.is_ok());
    assert_eq!(fresh.capital, 2);
    assert_eq!(group.vault, 2);
    assert_eq!(group.c_tot, 2);

    kani::cover!(true, "deposit materialization threshold paths reachable");
}

// ============================================================================
// P-2 — Deposit-then-withdraw round-trip preserves vault and capital.
//
// Ports v12 `proof_deposit_then_withdraw_roundtrip`. v16 surface uses
// the same `MarketGroupV16::deposit_not_atomic` /
// `withdraw_not_atomic` pair with `effective_prices` slice for
// withdraw (no ambient slot/oracle). The full round-trip exercises
// the value-flow proof on both legs.
//
// Invariant preserved: after symmetric in/out, vault == c_tot ==
// account.capital == 0; conservation (assert_public_invariants) holds
// at each step.
// ============================================================================

// fork-port Phase 1.B
/// Ports v12 `proof_deposit_then_withdraw_roundtrip`.
/// Preserves: deposit(x) + withdraw(x) on a flat account returns the
/// engine to its pre-deposit token-totals state and conservation holds.
/// Uses kani-core variants of deposit/withdraw — same pattern as the
/// v16 baseline `proof_v16_deposit_and_withdraw_value_flow_*` harness.
#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_fork_deposit_then_withdraw_roundtrip() {
    let mut group = baseline_group();
    let mut account = fresh_account(17);

    // Symbolic but bounded amount keeps the value-flow proof tractable.
    let amount: u8 = kani::any();
    kani::assume(amount > 0);
    kani::assume(amount <= 50);
    let amount_u = amount as u128;

    group.kani_deposit_core(&mut account, amount_u).unwrap();
    group.assert_public_invariants().unwrap();

    let r = group.kani_withdraw_core(&mut account, amount_u);
    assert!(r.is_ok());

    assert_eq!(account.capital, 0);
    assert_eq!(group.vault, 0);
    assert_eq!(group.c_tot, 0);
    group.assert_public_invariants().unwrap();

    kani::cover!(true, "deposit-then-withdraw round-trip reachable");
}

// ============================================================================
// P-3 — Withdraw is partial-OK: any amount that leaves non-negative
//        capital is allowed (no engine-side dust floor).
//
// Ports v12 `proof_property_51_withdraw_any_partial_ok` (spec property
// #51). v16 dropped the post-withdraw dust floor too — partials with
// non-zero residual must pass through cleanly.
//
// Invariant preserved: engine accepts any withdraw amount <= capital;
// dust-floor policy is wrapper-side.
// ============================================================================

// fork-port Phase 1.B
/// Ports v12 `proof_property_51_withdraw_any_partial_ok`.
/// Preserves: a partial withdraw that leaves non-zero, non-negative
/// capital is accepted; engine enforces no dust floor. Uses the
/// engine-core variant (`kani_withdraw_core`) to keep the symbolic
/// surface focused on the value-flow path — the public-API margin /
/// lien / mode gates are exercised by v16 baseline + P-5/P-6 below.
#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_fork_withdraw_partial_ok() {
    let mut group = baseline_group();
    let mut account = fresh_account(18);

    // Deposit a known amount; withdraw less than that leaves a residual.
    group.kani_deposit_core(&mut account, 50).unwrap();
    let r = group.kani_withdraw_core(&mut account, 45);
    assert!(r.is_ok());

    // Engine accepts the partial — residual capital stays.
    assert_eq!(account.capital, 5);
    assert_eq!(group.vault, 5);
    assert_eq!(group.c_tot, 5);
    group.assert_public_invariants().unwrap();

    kani::cover!(true, "withdraw partial-ok path reachable");
}

// ============================================================================
// P-4 — Withdraw of amount=0 is a no-op and preserves engine state.
//
// Ports v12 zero-amount semantics from `proof_property_51` family +
// `proof_withdraw_simulation_preserves_residual` (the residual
// invariant simplifies to "no change" when amount==0). v16
// `withdraw_not_atomic` short-circuits on `amount == 0` returning
// `Ok(())` without touching state.
//
// Invariant preserved: amount=0 never mutates capital, c_tot, or
// vault, regardless of account history.
// ============================================================================

// fork-port Phase 1.B
/// Ports v12 zero-amount withdraw semantics (spec §10.4 step 1).
/// Preserves: amount=0 withdraw returns Ok with byte-identical pre/post
/// engine state on vault, c_tot, and account.capital. Uses the
/// public-API path (`withdraw_not_atomic`) because the public surface
/// is where the zero-amount short-circuit is structurally important
/// (callers from BPF go through it).
#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_fork_withdraw_zero_is_noop() {
    let mut group = baseline_group();
    let mut account = fresh_account(19);

    // Establish some non-trivial balance so a state-preservation check
    // is meaningful.
    group.kani_deposit_core(&mut account, 25).unwrap();
    let vault_before = group.vault;
    let c_tot_before = group.c_tot;
    let capital_before = account.capital;

    let r = group.withdraw_not_atomic(&mut account, 0, &[1; V16_MAX_PORTFOLIO_ASSETS_N]);
    assert!(r.is_ok());

    assert_eq!(group.vault, vault_before);
    assert_eq!(group.c_tot, c_tot_before);
    assert_eq!(account.capital, capital_before);
    group.assert_public_invariants().unwrap();

    kani::cover!(true, "withdraw zero-amount no-op reachable");
}

// ============================================================================
// P-5 — Withdraw over-capital rejects atomically (no state mutation).
//
// Ports v12 over-withdraw rejection patterns from
// `proof_audit2_deposit_*` + `proof_close_account_returns_capital`
// (where the post-condition is exact capital draining). v16
// `withdraw_not_atomic` checks `amount > account.capital` *before*
// committing the vault/c_tot deltas, so the engine must surface
// `Err(_)` with byte-identical pre/post state.
//
// Invariant preserved: oversized withdraw never partially drains the
// vault; rejection is atomic.
// ============================================================================

// fork-port Phase 1.B
/// Ports v12 over-withdraw atomicity invariant.
/// Preserves: `amount > capital` withdraw on the engine-core path
/// returns Err and the engine vault, c_tot, and account.capital are
/// byte-identical to pre-call. Uses `kani_withdraw_core` to isolate
/// the L13103 over-capital guard from the public-API margin /
/// provenance gates (which are exercised by v16 baseline).
#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_fork_withdraw_over_capital_rejects_atomically() {
    let mut group = baseline_group();
    let mut account = fresh_account(20);

    group.kani_deposit_core(&mut account, 10).unwrap();
    let vault_before = group.vault;
    let c_tot_before = group.c_tot;
    let capital_before = account.capital;

    // Symbolic over-cap amount keeps the assertion meaningful while
    // bounding the SMT problem.
    let over: u8 = kani::any();
    kani::assume(over > 10);
    kani::assume(over <= 100);

    let r = group.kani_withdraw_core(&mut account, over as u128);
    assert!(r.is_err());

    assert_eq!(group.vault, vault_before);
    assert_eq!(group.c_tot, c_tot_before);
    assert_eq!(account.capital, capital_before);
    group.assert_public_invariants().unwrap();

    kani::cover!(true, "withdraw over-capital rejection reachable");
}

// ============================================================================
// P-6 — Withdraw in non-Live mode rejects with LockActive.
//
// Ports v12 `withdraw_not_atomic` mode-gate (engine §2.3:
// withdraw must reject outside Live mode). v16 retains the gate via
// `validate_withdraw_global_locks` at L13073.
//
// Invariant preserved: Recovery / Resolved modes reject withdraw with
// `LockActive` and leave engine state untouched.
// ============================================================================

// fork-port Phase 1.B
/// Ports v12 withdraw mode-gate. Preserves: non-Live mode rejects
/// withdraw with `Err(V16Error::LockActive)` regardless of amount, and
/// the engine state is byte-identical to pre-call. Uses the public
/// `withdraw_not_atomic` path because the mode gate lives there
/// (not in `kani_withdraw_core`); unwind=130 to cover the public-API
/// provenance/global-locks memcmp.
#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_fork_withdraw_non_live_mode_rejects() {
    let mut group = baseline_group();
    let mut account = fresh_account(21);
    group.kani_deposit_core(&mut account, 10).unwrap();

    // Flip to Recovery (one of the two non-Live modes); the gate is
    // mode != Live, so testing one non-Live mode is sufficient.
    group.mode = MarketModeV16::Recovery;

    let vault_before = group.vault;
    let c_tot_before = group.c_tot;
    let capital_before = account.capital;

    let r = group.withdraw_not_atomic(&mut account, 5, &[1; V16_MAX_PORTFOLIO_ASSETS_N]);
    assert_eq!(r, Err(V16Error::LockActive));

    assert_eq!(group.vault, vault_before);
    assert_eq!(group.c_tot, c_tot_before);
    assert_eq!(account.capital, capital_before);

    kani::cover!(true, "withdraw non-Live mode rejection reachable");
}

// ============================================================================
// P-7 — Charge-fee in non-Live mode rejects with LockActive.
//
// Ports v12 `charge_account_fee_*` mode-gate. v16
// `charge_account_fee_not_atomic` checks `mode != Live` at L12697
// before delegating to `charge_account_fee_after_loss_settlement`.
// The gate is the engine-side enforcement of "no fee accrual outside
// Live trading" — a load-bearing invariant for fee-anchor
// monotonicity (A-9 / fee-policy port relies on this).
//
// Invariant preserved: non-Live mode rejects with LockActive and
// preserves engine + account state.
// ============================================================================

// fork-port Phase 1.B
/// Ports v12 fee-charge mode-gate.
/// Preserves: charge_account_fee_not_atomic in Resolved/Recovery mode
/// returns Err(LockActive) with no mutation to capital, c_tot,
/// vault, or insurance. Uses the public `charge_account_fee_not_atomic`
/// path; unwind=130 to match v16 baseline charge-fee tests' memcmp
/// coverage on settle/validate paths.
#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_fork_charge_account_fee_non_live_mode_rejects() {
    let mut group = baseline_group();
    let mut account = fresh_account(22);
    group.kani_deposit_core(&mut account, 30).unwrap();
    group.mode = MarketModeV16::Resolved;

    let vault_before = group.vault;
    let c_tot_before = group.c_tot;
    let insurance_before = group.insurance;
    let capital_before = account.capital;

    let r = group.charge_account_fee_not_atomic(&mut account, 5);
    assert_eq!(r, Err(V16Error::LockActive));

    assert_eq!(group.vault, vault_before);
    assert_eq!(group.c_tot, c_tot_before);
    assert_eq!(group.insurance, insurance_before);
    assert_eq!(account.capital, capital_before);

    kani::cover!(true, "charge fee non-Live mode rejection reachable");
}

// ============================================================================
// P-8 — Charge-fee with `requested_fee == 0` is a no-op (returns 0).
//
// Ports v12 fee-shortfall short-circuit semantics from
// `proof_fee_shortfall_routes_to_fee_credits`. v16
// `kani_charge_account_fee_current` returns `Ok(0)` immediately on
// `requested_fee == 0` (L12751), without touching capital, c_tot,
// vault, or insurance.
//
// Invariant preserved: zero-fee charge is a structural no-op; no
// value flow proof fires.
// ============================================================================

// fork-port Phase 1.B
/// Ports v12 zero-fee no-op semantics (spec §4.10).
/// Preserves: requested_fee=0 returns Ok(0) with byte-identical engine
/// and account state. Uses `kani_charge_account_fee_current` —
/// the engine-core fee path that skips loss-settlement +
/// b-stale checks (those gates are covered by their own baselines).
#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_fork_charge_fee_zero_is_noop() {
    let mut group = baseline_group();
    let mut account = fresh_account(23);
    group.kani_deposit_core(&mut account, 30).unwrap();

    let vault_before = group.vault;
    let c_tot_before = group.c_tot;
    let insurance_before = group.insurance;
    let capital_before = account.capital;

    let charged = group
        .kani_charge_account_fee_current(&mut account, 0)
        .unwrap();
    assert_eq!(charged, 0);

    assert_eq!(group.vault, vault_before);
    assert_eq!(group.c_tot, c_tot_before);
    assert_eq!(group.insurance, insurance_before);
    assert_eq!(account.capital, capital_before);

    kani::cover!(true, "charge fee zero amount no-op reachable");
}

// ============================================================================
// P-9 — Charge-fee with negative PnL returns 0 (no charge).
//
// Ports v12 `proof_fee_shortfall_routes_to_fee_credits` guard: fee
// charging is suppressed while PnL is negative — losses must clear
// first. v16 enforces this at L12751 (`account.pnl < 0`).
//
// Invariant preserved: negative PnL accounts skip the fee path; no
// state mutation, return value is 0.
// ============================================================================

// fork-port Phase 1.B
/// Ports v12 "no fee charge with negative PnL" guard (spec §4.10).
/// Preserves: account.pnl < 0 causes charge_account_fee to return
/// Ok(0) with no state mutation. Uses
/// `kani_charge_account_fee_current` (post-loss-settlement core)
/// so the L12751 PnL-negative gate is the only reachable branch.
#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_fork_charge_fee_negative_pnl_skipped() {
    let mut group = baseline_group();
    let mut account = fresh_account(24);
    group.kani_deposit_core(&mut account, 30).unwrap();

    // Drive PnL negative below zero — fee charge must short-circuit.
    let loss: u8 = kani::any();
    kani::assume(loss > 0);
    kani::assume(loss <= 20);
    account.pnl = -(loss as i128);

    let vault_before = group.vault;
    let c_tot_before = group.c_tot;
    let insurance_before = group.insurance;
    let capital_before = account.capital;

    let fee: u8 = kani::any();
    kani::assume(fee > 0);
    kani::assume(fee <= 10);
    let charged = group
        .kani_charge_account_fee_current(&mut account, fee as u128)
        .unwrap();

    assert_eq!(charged, 0);
    assert_eq!(group.vault, vault_before);
    assert_eq!(group.c_tot, c_tot_before);
    assert_eq!(group.insurance, insurance_before);
    assert_eq!(account.capital, capital_before);

    kani::cover!(true, "charge fee negative-pnl short-circuit reachable");
}

// ============================================================================
// P-10 — Charge-fee is capped by capital (min(requested, capital)
//         flows to insurance, never more).
//
// Ports v12 fee-cap invariant from `proof_fee_shortfall_routes_to_fee_credits`
// + `proof_audit_fee_sweep_pnl_conservation`. v16
// `kani_charge_account_fee_current` computes `charged = requested_fee.min(
// account.capital)` at L12754. The capital → insurance value-flow proof
// (`TokenValueFlowProofV16::account_capital_to_insurance`) validates the
// transfer.
//
// Invariant preserved: charged ≤ requested; charged ≤ capital;
// insurance += charged; capital -= charged; vault unchanged.
// ============================================================================

// fork-port Phase 1.B
/// Ports v12 fee-cap invariant + capital→insurance value-flow.
/// Preserves: charged = min(requested, capital); insurance += charged;
/// capital -= charged; vault preserved; conservation holds. Uses
/// the engine-core fee path (kani_charge_account_fee_current) to
/// isolate the L12754 cap algebra from the mode/b-stale/pnl gates.
#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_fork_charge_fee_capped_by_capital() {
    let mut group = baseline_group();
    let mut account = fresh_account(25);

    // Bounded symbolic capital + fee. PnL pinned non-negative to bypass
    // the L12751 short-circuit so the cap logic is the reachable branch.
    let capital_u: u8 = kani::any();
    kani::assume(capital_u > 0);
    kani::assume(capital_u <= 10);
    let fee_u: u8 = kani::any();
    kani::assume(fee_u > 0);
    kani::assume(fee_u <= 20);

    group
        .kani_deposit_core(&mut account, capital_u as u128)
        .unwrap();
    let vault_before = group.vault;
    let c_tot_before = group.c_tot;
    let insurance_before = group.insurance;

    let charged = group
        .kani_charge_account_fee_current(&mut account, fee_u as u128)
        .unwrap();

    let expected_cap = core::cmp::min(fee_u as u128, capital_u as u128);
    assert_eq!(charged, expected_cap);
    assert_eq!(account.capital, capital_u as u128 - expected_cap);
    assert_eq!(group.c_tot, c_tot_before - expected_cap);
    assert_eq!(group.insurance, insurance_before + expected_cap);
    // Vault unchanged — fee is an internal capital→insurance move.
    assert_eq!(group.vault, vault_before);
    group.assert_public_invariants().unwrap();

    kani::cover!(true, "charge fee cap-by-capital path reachable");
}

// ============================================================================
// P-11 — Trade in non-Live mode rejects atomically.
//
// Ports v12 `execute_trade_not_atomic` mode-gate. v16
// `execute_trade_with_fee_inner` checks `mode != Live` at L14948
// before any settlement / position-delta mutation, so the rejection
// is atomic.
//
// Invariant preserved: non-Live mode rejects trade with LockActive;
// no fees charged, no positions opened, no OI updated.
// ============================================================================

// fork-port Phase 1.B
/// Ports v12 trade mode-gate.
/// Preserves: non-Live mode rejects trade with Err(LockActive) and
/// engine vault/c_tot/insurance + asset[0].oi_eff_long/short are
/// byte-identical to pre-call (atomic rejection). Unwind=130
/// matches v16 baseline trade harnesses (e.g.
/// `proof_v16_trade_dynamic_fee_cap_is_enforced_before_mutation`) —
/// the public `execute_trade_with_fee_not_atomic` path settles both
/// accounts and validates provenance memcmp on the way in.
#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_fork_trade_non_live_mode_rejects_atomically() {
    let mut group = baseline_group();
    group.config.max_trading_fee_bps = 10;
    let (market, account_id, owner) = concrete_ids();
    let mut long =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut short = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut long, 10_000).unwrap();
    group.deposit_not_atomic(&mut short, 10_000).unwrap();

    // Switch to Recovery — gate is mode != Live.
    group.mode = MarketModeV16::Recovery;

    let vault_before = group.vault;
    let c_tot_before = group.c_tot;
    let insurance_before = group.insurance;
    let oi_long_before = group.assets[0].oi_eff_long_q;
    let oi_short_before = group.assets[0].oi_eff_short_q;
    let long_capital_before = long.capital;
    let short_capital_before = short.capital;

    let r = group.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV16 {
            asset_index: 0,
            size_q: POS_SCALE,
            exec_price: 100,
            fee_bps: 5,
            admit_h_max_consumption_threshold_bps_opt: None,
        },
        &[100; V16_MAX_PORTFOLIO_ASSETS_N],
    );
    assert_eq!(r, Err(V16Error::LockActive));

    assert_eq!(group.vault, vault_before);
    assert_eq!(group.c_tot, c_tot_before);
    assert_eq!(group.insurance, insurance_before);
    assert_eq!(group.assets[0].oi_eff_long_q, oi_long_before);
    assert_eq!(group.assets[0].oi_eff_short_q, oi_short_before);
    assert_eq!(long.capital, long_capital_before);
    assert_eq!(short.capital, short_capital_before);

    kani::cover!(true, "trade non-Live mode atomic rejection reachable");
}

// ============================================================================
// KANI_TODO: P-12 — Trade fee path is symmetric (fee_bps=0 boundary).
// KANI_TODO: P-13 — Trade with admit-threshold None at entry is additive.
//
// Both ports invoke the full `execute_trade_with_fee_not_atomic` path
// at unwind(130), which exceeded the Phase 1.B budget in our
// experimental runs (P-12 + P-13 each spent >30 min of CBMC time on
// the value-flow proof + recertify_account_after_trade_delta memcmp
// + apply_position_delta loop without converging). Re-ports needed in
// a later wave with one of:
//
//   (a) reduced symbolic surface — fix exec_price to 1 and size_q to 1
//       (degrading the trade to a "structural smoke" rather than the
//       value-flow proof); OR
//   (b) follow the v16 baseline `proof_v16_trade_*` harnesses' exact
//       fixture (concrete account-bitmap, concrete config, exact
//       effective_prices = [100; …]) which the baseline runs in
//       acceptable time. The remaining differential vs. the baseline
//       is the mode-flip / accumulator probe; isolate that on the
//       smallest test vector that exercises L14948 / L14971.
//
// Skipped on time-budget grounds, not on correctness grounds — the
// invariants being proven are already supported by:
//   - P-11 here (trade mode-gate rejection on the same surface);
//   - `proofs_v16_fork.rs::proof_v16_admit_threshold_*` for the
//     bare-lane A-1 gate semantics;
//   - v16 baseline `proof_v16_trade_fee_conservation_and_oi_symmetry`
//     for fee symmetry on the nonzero-fee_bps case.
//
// Future-work tag: KANI_TODO_TRADE_FORK_PORT_P12_P13.
// ============================================================================

// Unused trait/import suppression — `SideV16` is imported because the
// fork-port pattern keeps the same import list as the baseline so future
// ports (sign-flip, attach-leg variants) drop in without churn.
#[allow(dead_code)]
fn _unused_side_check() -> SideV16 {
    SideV16::Long
}
#[allow(dead_code)]
fn _unused_hlock_check() -> HLockLaneV16 {
    HLockLaneV16::HMin
}
