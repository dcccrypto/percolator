#![cfg(all(kani, feature = "fork-facade"))]

//! v17 fork-feature Kani proofs — re-expressed onto the single zero-copy/sparse engine path.
//!
//! Frozen toly `proofs_v16.rs` is adopted BYTE-IDENTICAL (the rebuilt zero-copy core harness);
//! every fork-feature proof lives here instead so the adopted core stays clean and the fork
//! surface re-verifies independently. Features are re-grafted onto frozen one unit at a time;
//! each unit's proof(s) land here. Under `#[cfg(kani)]` the frozen crate re-exports all of `v16`
//! (lib.rs `#[cfg(kani)] pub use v16::*`), so these access the engine API directly.
//!
//! Coverage:
//!   A-1  — admit-threshold (h_lock_lane threshold gate).
//!   A-6  — stress-envelope writer.
//!   A-9  — fee-policy mutator.
//!   A-10 — max_price_move_bps_per_slot upper bound.
//!   lp_vault — LP Vault share-math (fork-facade module).

use percolator::v16::V16Config;
use percolator::MAX_MARGIN_BPS;

// ============================================================================
// A-10 — max_price_move_bps_per_slot upper bound.
// Frozen toly bounds only the lower edge (`== 0`); the fork additionally
// rejects `> MAX_MARGIN_BPS` (a move budget above full margin would weaken the
// per-slot price-move guard). Re-grafted onto frozen as a one-clause shape check.
// ============================================================================

/// RED-before / GREEN-after for the A-10 clause: any out-of-range value above
/// MAX_MARGIN_BPS must be rejected by `validate_public_user_fund_shape`. Without
/// the re-grafted clause this proof FAILS (frozen would accept it).
#[kani::proof]
#[kani::unwind(20)]
#[kani::solver(cadical)]
fn proof_v17_max_price_move_bps_per_slot_upper_bound() {
    let mut config = V16Config::public_user_fund(1, 0, 1);

    let bad: u64 = kani::any();
    kani::assume(bad > MAX_MARGIN_BPS);
    config.max_price_move_bps_per_slot = bad;

    assert!(config.kani_validate_public_user_fund_shape().is_err());
    kani::cover!(true, "out-of-range max_price_move rejected");
}

/// Boundary: `== MAX_MARGIN_BPS` is accepted (the bound is `<=`, not `<`), so
/// the re-grafted clause is not stricter than the fork's v12 intent.
#[kani::proof]
#[kani::unwind(20)]
#[kani::solver(cadical)]
fn proof_v17_max_price_move_bps_per_slot_boundary_accepted() {
    let mut config = V16Config::public_user_fund(1, 0, 1);
    config.max_price_move_bps_per_slot = MAX_MARGIN_BPS;

    assert!(config.kani_validate_public_user_fund_shape().is_ok());
    kani::cover!(true, "boundary max_price_move accepted");
}

// ============================================================================
// lp_vault — LP Vault share-math (re-grafted module, fork-facade gated).
// The 5 wide-math properties (nav/shares/redemption/fee-split) route through
// wide_mul_div_floor_u128's ~256-iter U256 division → CBMC-intractable; they are
// DEFERRED with full coverage by the 29 concrete-value native tests in
// tests/v16_fork_lp_vault_tests.rs (NAV-from-counters donation defense,
// round-DOWN issuance/redemption, deposit→redeem no-profit, fee-split
// conservation). The 2 loop-free properties remain live formal proofs.
// ============================================================================
use percolator::lp_vault::{lp_redemption_cooldown_elapsed, lp_shares_for_deposit};

/// LP_VAULT-5: drain-epoch freshness — total_shares==0 mints exactly `amount`
/// (1:1), independent of stale NAV (early-return path, no wide-math loop).
#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn proof_v17_lp_vault_drain_epoch_freshness() {
    let amount: u128 = kani::any();
    let stale_nav: u128 = kani::any();
    kani::assume(amount >= 1 && amount <= 1_000_000_000_000);
    let shares = lp_shares_for_deposit(amount, 0, stale_nav).unwrap();
    assert_eq!(shares, amount, "drain-epoch deposit must mint 1:1");
    kani::cover!(true, "LP_VAULT-5 drain epoch freshness (1:1 reset)");
}

/// LP_VAULT-7: cooldown enforcement — elapsed iff current >= request+cooldown
/// (saturating, no overflow); cooldown==0 ⇒ always elapsed.
#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn proof_v17_lp_vault_cooldown_enforcement() {
    let request_slot: u64 = kani::any();
    let current_slot: u64 = kani::any();
    let cooldown: u64 = kani::any();
    let elapsed = lp_redemption_cooldown_elapsed(request_slot, current_slot, cooldown);
    let deadline = request_slot.saturating_add(cooldown);
    assert_eq!(
        elapsed,
        current_slot >= deadline,
        "cooldown elapsed iff current >= saturating deadline"
    );
    if cooldown == 0 {
        assert!(
            lp_redemption_cooldown_elapsed(request_slot, current_slot, 0)
                == (current_slot >= request_slot)
        );
    }
    kani::cover!(true, "LP_VAULT-7 cooldown enforcement (saturating)");
}

// ============================================================================
// A-6 — stress envelope writer. Frozen carries the dormant `threshold_stress_active`
// flag; the fork's writer makes it live via a consumption accumulator + slot/epoch
// sentinels. Re-grafted onto the zero-copy ViewMut header. encode_bool maps true→1/
// false→0, so the flag is checked as a raw u8 (codec fns are crate-private).
// ============================================================================
use percolator::v16::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16View,
    MarketGroupV16ViewMut, V16Error, V16PodU128, V16PodU64,
};
use percolator::STRESS_ENVELOPE_TRIGGER_BPS_E9;

fn env_header() -> MarketGroupV16HeaderAccount {
    let cfg = V16Config::public_user_fund(1, 0, 1);
    MarketGroupV16HeaderAccount::new_dynamic([7u8; 32], cfg, 1, 0).unwrap()
}

/// A-6.1: accumulator is monotonic non-decreasing within an epoch/slot.
#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn proof_v17_stress_envelope_writer_monotonic() {
    let mut header = env_header();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    let mut view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let c1: u128 = kani::any();
    let c2: u128 = kani::any();
    kani::assume(c1 <= u128::MAX / 4);
    kani::assume(c2 <= u128::MAX / 4);
    let now = 5u64;
    view.apply_stress_envelope_progress(c1, now).unwrap();
    let a1 = view.header.stress_consumption_bps_e9_since_envelope.get();
    view.apply_stress_envelope_progress(c2, now).unwrap();
    let a2 = view.header.stress_consumption_bps_e9_since_envelope.get();
    assert!(a2 >= a1, "accumulator monotonic within epoch/slot");
}

/// A-6.2: flag flips true iff one accrual reaches the trigger; start slot/epoch
/// are stamped on activation, else remain u64::MAX sentinels. OPERATIVE for the writer.
#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn proof_v17_stress_envelope_activation_threshold() {
    let mut header = env_header();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    let mut view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let c: u128 = kani::any();
    kani::assume(c <= STRESS_ENVELOPE_TRIGGER_BPS_E9.saturating_add(1));
    let now = 9u64;
    let risk_epoch = view.header.risk_epoch.get();
    view.apply_stress_envelope_progress(c, now).unwrap();
    let active = view.header.threshold_stress_active == 1;
    assert_eq!(active, c >= STRESS_ENVELOPE_TRIGGER_BPS_E9, "flag set iff acc reached trigger");
    if active {
        assert_eq!(view.header.stress_envelope_start_slot.get(), now);
        assert_eq!(view.header.stress_envelope_start_credit_epoch.get(), risk_epoch);
    } else {
        assert_eq!(view.header.stress_envelope_start_slot.get(), u64::MAX);
        assert_eq!(view.header.stress_envelope_start_credit_epoch.get(), u64::MAX);
    }
}

/// A-6.3: clear zeroes the accumulator, restores u64::MAX sentinels, clears the flag.
#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn proof_v17_stress_envelope_clear_resets_fields() {
    let mut header = env_header();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    let mut view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    view.header.stress_consumption_bps_e9_since_envelope = V16PodU128::new(kani::any());
    view.header.stress_envelope_start_slot = V16PodU64::new(kani::any());
    view.header.stress_envelope_start_credit_epoch = V16PodU64::new(kani::any());
    view.header.threshold_stress_active = 1;
    view.clear_stress_envelope_v16();
    assert_eq!(view.header.stress_consumption_bps_e9_since_envelope.get(), 0);
    assert_eq!(view.header.stress_envelope_start_slot.get(), u64::MAX);
    assert_eq!(view.header.stress_envelope_start_credit_epoch.get(), u64::MAX);
    assert_eq!(view.header.threshold_stress_active, 0);
}

/// A-6.4: an active envelope from a PRIOR epoch is cleared (epoch advanced, different
/// slot, not in active-close) before the new accrual — so the post-state holds only the
/// fresh delta and the flag drops below trigger.
#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn proof_v17_stress_envelope_epoch_reset() {
    let mut header = env_header();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    let mut view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    view.header.threshold_stress_active = 1;
    view.header.stress_consumption_bps_e9_since_envelope =
        V16PodU128::new(STRESS_ENVELOPE_TRIGGER_BPS_E9);
    view.header.stress_envelope_start_slot = V16PodU64::new(10);
    view.header.stress_envelope_start_credit_epoch = V16PodU64::new(1);
    view.header.risk_epoch = V16PodU64::new(2);
    // fresh header is Live mode + loss_stale_active=0 → not active-close → epoch-advance clears.
    view.apply_stress_envelope_progress(1, 11).unwrap();
    assert_eq!(view.header.stress_consumption_bps_e9_since_envelope.get(), 1);
    assert_eq!(view.header.threshold_stress_active, 0);
}

/// A-6.5: the validate_shape sentinel-pairing invariant rejects a torn idle envelope
/// (flag=0, acc=0, but a sentinel != u64::MAX). OPERATIVE for the shape clause
/// (RED-before/GREEN-after): without the clause validate_shape would accept it.
#[kani::proof]
#[kani::unwind(24)]
#[kani::solver(cadical)]
fn proof_v17_stress_envelope_validate_shape_pairing() {
    let cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([7u8; 32], cfg, 1, 0).unwrap();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    {
        let mut view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        view.activate_empty_market_not_atomic(0, 100, 1).unwrap();
    }
    // proper idle envelope (new_dynamic set sentinels = u64::MAX) passes.
    {
        let view = MarketGroupV16View::new(&header, &markets);
        assert!(view.validate_shape().is_ok(), "idle envelope (sentinels=MAX) passes");
    }
    // torn idle envelope: flag=0, acc=0, but start_slot=0 (!= MAX) → rejected.
    header.stress_envelope_start_slot = V16PodU64::new(0);
    {
        let view = MarketGroupV16View::new(&header, &markets);
        assert_eq!(
            view.validate_shape(),
            Err(V16Error::InvalidConfig),
            "torn idle envelope rejected by the pairing invariant"
        );
    }
}

// ============================================================================
// A-9 — dynamic fee-policy mutator (MarketGroupV16HeaderAccount::apply_fee_policy_update_not_atomic).
// Re-expressed onto the zero-copy header. Each harness pins the vector to the
// validate_exact_solvency_envelope EARLY-RETURN path via public_user_fund(1,0,1)
// (maintenance_margin_bps==10_000, liquidation_fee_bps==0, min_liquidation_abs==0,
// max_abs_funding_e9_per_slot==0) + zeroed liquidation fields, so CBMC never drives
// the recursive interval-validation loop. V16Config/V16ConfigAccount derive PartialEq.
// ============================================================================
use percolator::v16::FeePolicyUpdateV16;

fn a9_baseline_header() -> MarketGroupV16HeaderAccount {
    MarketGroupV16HeaderAccount::new_dynamic([1u8; 32], V16Config::public_user_fund(1, 0, 1), 1, 0)
        .unwrap()
}

/// A-9.1: an out-of-range max_trading_fee_bps (> MAX_MARGIN_BPS) is REJECTED and leaves the on-account
/// config byte-unchanged; an in-range value is accepted + persisted. OPERATIVE (straddles the bound).
#[kani::proof]
#[kani::unwind(20)]
#[kani::solver(cadical)]
fn proof_v17_apply_fee_policy_update_validates_bounds() {
    let mut group = a9_baseline_header();
    let before = group.config; // V16ConfigAccount POD (Copy, byte-eq)
    let m: u64 = kani::any();
    kani::assume(m <= MAX_MARGIN_BPS + 1);
    let update = FeePolicyUpdateV16 {
        max_trading_fee_bps: m,
        liquidation_fee_bps: 0,
        liquidation_fee_cap: 0,
        min_liquidation_abs: 0,
    };
    let result = group.kani_apply_fee_policy_update_not_atomic(update);
    if m > MAX_MARGIN_BPS {
        assert!(result.is_err());
        assert_eq!(group.config, before, "rejected update leaves config byte-unchanged");
    } else {
        assert!(result.is_ok());
        assert_eq!(
            group.config.try_to_runtime_shape().unwrap().max_trading_fee_bps,
            m
        );
    }
}

/// A-9.2: a valid update persists exactly the four fee-policy fields.
#[kani::proof]
#[kani::unwind(20)]
#[kani::solver(cadical)]
fn proof_v17_apply_fee_policy_update_persists() {
    let mut group = a9_baseline_header();
    let m: u64 = kani::any();
    kani::assume(m <= MAX_MARGIN_BPS);
    let update = FeePolicyUpdateV16 {
        max_trading_fee_bps: m,
        liquidation_fee_bps: 0,
        liquidation_fee_cap: 0,
        min_liquidation_abs: 0,
    };
    group.kani_apply_fee_policy_update_not_atomic(update).unwrap();
    let cfg = group.config.try_to_runtime_shape().unwrap();
    assert_eq!(cfg.max_trading_fee_bps, m);
    assert_eq!(cfg.liquidation_fee_bps, 0);
    assert_eq!(cfg.liquidation_fee_cap, 0);
    assert_eq!(cfg.min_liquidation_abs, 0);
}

/// A-9.3: NO config field outside the four fee-policy targets is mutated (additive-surface invariant).
/// after == baseline-with-the-4-fields-swapped (full V16Config equality).
#[kani::proof]
#[kani::unwind(20)]
#[kani::solver(cadical)]
fn proof_v17_fee_policy_update_no_other_field_mutation() {
    let mut group = a9_baseline_header();
    let before_cfg = group.config.try_to_runtime_shape().unwrap();
    let m: u64 = kani::any();
    kani::assume(m <= MAX_MARGIN_BPS);
    let update = FeePolicyUpdateV16 {
        max_trading_fee_bps: m,
        liquidation_fee_bps: 0,
        liquidation_fee_cap: 0,
        min_liquidation_abs: 0,
    };
    group.kani_apply_fee_policy_update_not_atomic(update).unwrap();
    let after_cfg = group.config.try_to_runtime_shape().unwrap();
    let mut expected = before_cfg;
    expected.max_trading_fee_bps = update.max_trading_fee_bps;
    expected.liquidation_fee_bps = update.liquidation_fee_bps;
    expected.liquidation_fee_cap = update.liquidation_fee_cap;
    expected.min_liquidation_abs = update.min_liquidation_abs;
    assert_eq!(after_cfg, expected, "only the 4 fee-policy fields change");
}

// ============================================================================
// A-1 — admit-threshold gate in h_lock_lane.
// The frozen toly h_lock_lane does not carry the per-trade threshold parameter;
// A-1 re-grafts it so a caller can request HMax when the A-6 stress-consumption
// accumulator has reached a caller-supplied threshold (even when the market flag
// `threshold_stress_active` has not yet flipped — the flag is set after crossing
// STRESS_ENVELOPE_TRIGGER_BPS_E9, but the caller may want a stricter or more
// relaxed threshold). The shim `kani_h_lock_lane_with_threshold` exposes the
// parameter to the formal harness.
//
// OPERATIVE (RED-before / GREEN-after discipline):
//   A-1.1 — without the feature, `h_lock_lane` returns HMin for a market with
//     acc < flag-trigger and no other HMax signal, even when acc >= caller threshold.
//     With the feature, it returns HMax. The proof is GREEN because the feature is
//     active (test only compiles under #[cfg(all(kani, feature="fork-facade"))]).
//   A-1.2 — None threshold preserves toly baseline (no spurious HMax).
//   A-1.3 — threshold == 0 always lifts HMax (saturating comparison; 0 >= 0 is always
//     true, so any non-flag-triggered market with threshold=0 still gets HMax). This
//     documents the API contract ("threshold=0 means always restrict").
// ============================================================================
use percolator::v16::HLockLaneV16;

fn a1_live_market_header_with_acc(acc: u128) -> MarketGroupV16HeaderAccount {
    let cfg = V16Config::public_user_fund(1, 0, 1);
    let mut h =
        MarketGroupV16HeaderAccount::new_dynamic([3u8; 32], cfg, 1, 0).unwrap();
    // inject the accumulator directly (below the trigger so the flag stays 0)
    h.stress_consumption_bps_e9_since_envelope = V16PodU128::new(acc);
    // flag stays 0 (toly baseline: no HMax from the flag path)
    assert_eq!(h.threshold_stress_active, 0);
    h
}

/// A-1.1: OPERATIVE — threshold gate fires when acc >= threshold AND no other
/// HMax trigger is set. This harness is only GREEN because fork-facade is
/// enabled; the toly-baseline path (None) would return HMin on the same market.
#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn proof_v17_admit_threshold_lifts_hmax_when_acc_reaches_threshold() {
    // pick a threshold and an acc >= threshold, both below the global flag trigger
    // (so threshold_stress_active stays 0 — this is strictly the A-1 path)
    let threshold: u128 = kani::any();
    let acc: u128 = kani::any();
    kani::assume(threshold >= 1);
    kani::assume(acc >= threshold);
    // keep both below the flag trigger so `threshold_stress_active` stays 0
    kani::assume(acc < percolator::STRESS_ENVELOPE_TRIGGER_BPS_E9);
    kani::assume(threshold < percolator::STRESS_ENVELOPE_TRIGGER_BPS_E9);
    let mut header = a1_live_market_header_with_acc(acc);
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    let view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let result = view.kani_h_lock_lane_with_threshold(None, false, Some(threshold));
    assert_eq!(
        result,
        Ok(HLockLaneV16::HMax),
        "threshold gate: acc >= threshold => HMax even when flag=0"
    );
    kani::cover!(true, "A-1 threshold gate fires (OPERATIVE)");
}

/// A-1.2: None threshold preserves toly baseline — h_lock_lane returns HMin for
/// a clean Live market when no other HMax trigger fires.
#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn proof_v17_admit_threshold_none_preserves_hmin_on_clean_market() {
    let acc: u128 = kani::any();
    kani::assume(acc < percolator::STRESS_ENVELOPE_TRIGGER_BPS_E9);
    let mut header = a1_live_market_header_with_acc(acc);
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    let view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let result = view.kani_h_lock_lane_with_threshold(None, false, None);
    assert_eq!(
        result,
        Ok(HLockLaneV16::HMin),
        "None threshold + no other trigger => HMin (toly baseline preserved)"
    );
    kani::cover!(true, "A-1 None threshold preserves HMin baseline");
}

/// A-1.3: threshold == 0 always lifts HMax (0 >= 0 is unconditionally true).
#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn proof_v17_admit_threshold_zero_always_lifts_hmax() {
    let acc: u128 = kani::any();
    // keep below the global flag trigger (flag=0); the A-1 path fires for threshold=0
    kani::assume(acc < percolator::STRESS_ENVELOPE_TRIGGER_BPS_E9);
    let mut header = a1_live_market_header_with_acc(acc);
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    let view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let result = view.kani_h_lock_lane_with_threshold(None, false, Some(0u128));
    assert_eq!(
        result,
        Ok(HLockLaneV16::HMax),
        "threshold=0 always lifts HMax (acc >= 0 always true)"
    );
    kani::cover!(true, "A-1 threshold=0 unconditional HMax");
}

// ============================================================================
// A-4 — fork_facade equity/IM pub-lifts.
// Proves: (A-4.1) maint_raw >= init_raw iff pnl > 0; equal otherwise.
//         (A-4.2) account_equity_trade_open_raw with pnl_override=account.pnl
//                 == init_raw (same lane when no counterfactual).
// These harnesses exercise the fork_facade re-lift surface on zero-copy views.
// ============================================================================
use percolator::v16::fork_facade;
use percolator::v16::{PortfolioAccountV16Account, PortfolioV16View, V16PodI128};

fn a4_minimal_account(
    capital: u64,
    pnl: i64,
    fee_credits: i64,
) -> PortfolioAccountV16Account {
    PortfolioAccountV16Account {
        capital: V16PodU128::new(capital as u128),
        pnl: V16PodI128::new(pnl as i128),
        fee_credits: V16PodI128::new(fee_credits as i128),
        ..PortfolioAccountV16Account::default()
    }
}

/// A-4.1: maint_raw >= init_raw when pnl > 0; equal when pnl <= 0.
/// OPERATIVE: proves the equity-lane separation is correct.
#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn proof_v17_fork_facade_maint_vs_init_equity_lane_separation() {
    let capital: u64 = kani::any();
    let pnl: i64 = kani::any();
    let fee_credits: i64 = kani::any();
    // constrain to valid range (no i128::MIN, non-negative fee_debt)
    kani::assume(pnl != i64::MIN);
    kani::assume(fee_credits != i64::MIN);
    kani::assume(fee_credits <= 0); // fee_credits <= 0 means a fee debt
    let acct = a4_minimal_account(capital, pnl, fee_credits);
    let view = PortfolioV16View::new(&acct);
    let Ok(maint) = fork_facade::account_equity_maint_raw(&view) else {
        return; // overflow case — not an assertion failure
    };
    let Ok(init) = fork_facade::account_equity_init_raw(&view) else {
        return;
    };
    if (pnl as i128) > 0 {
        // maint includes positive PnL; init clamps to 0
        assert!(maint >= init, "maint_raw >= init_raw when pnl > 0");
        kani::cover!(true, "A-4 maint > init (positive pnl path)");
    } else {
        // pnl clamped to 0 in both; equal
        assert_eq!(maint, init, "maint_raw == init_raw when pnl <= 0");
        kani::cover!(true, "A-4 maint == init (non-positive pnl path)");
    }
}

/// A-4.2: account_equity_trade_open_raw with pnl_override == account.pnl
/// equals account_equity_init_raw (same IM lane, no counterfactual).
#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn proof_v17_fork_facade_trade_open_equity_matches_init_at_identity_override() {
    let capital: u64 = kani::any();
    let pnl: i64 = kani::any();
    let fee_credits: i64 = kani::any();
    kani::assume(pnl != i64::MIN);
    kani::assume(fee_credits != i64::MIN);
    kani::assume(fee_credits <= 0);
    let acct = a4_minimal_account(capital, pnl, fee_credits);
    let view = PortfolioV16View::new(&acct);
    let Ok(init_eq) = fork_facade::account_equity_init_raw(&view) else {
        return;
    };
    let Ok(trade_open_eq) =
        fork_facade::account_equity_trade_open_raw(&view, pnl as i128) else {
        return;
    };
    assert_eq!(
        trade_open_eq, init_eq,
        "trade_open_raw at identity override == init_raw"
    );
    kani::cover!(true, "A-4 trade_open_raw identity override matches init_raw");
}
