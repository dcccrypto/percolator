#![cfg(kani)]
//! v2.2 overnight (2026-10-14) bounded/split replacements for the S10 no-verdict rows of
//! `tests/proofs_v22_final.rs`. PROOF-ONLY FILE; the originals are kept unchanged.
//!
//! The fixture section below (`ids` .. `s10_spec`) is a VERBATIM copy of
//! `tests/proofs_v22_final.rs` lines 13-247 (test files are separate crates and cannot share it),
//! so every replacement harness quantifies over exactly the original admitted state set unless
//! its doc comment states a `BOUNDED:` / `CASE:` restriction.
#![allow(clippy::too_many_arguments, dead_code, unused_imports, unused_mut)]

use percolator::v16::{
    adjust_slot_provider_principal, kani_available_backing_num_for_source_credit_state,
    kani_expected_source_credit_rate_num_for_state, repay_pnl_postconditions_hold,
    AssetStateV16, AssetStateV16Account, BackingBucketStatusV16, BackingBucketV16,
    BackingBucketV16Account, EngineAssetSlotV16Account, InsuranceCreditReservationV16,
    InsuranceCreditReservationV16Account, KfDriftSideV16, LiquidationRequestV16, Market,
    MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, PermissionlessCrankActionV16,
    PermissionlessCrankRequestV16, PermissionlessRecoveryReasonV16, PortfolioAccountV16Account,
    PortfolioLegV16, PortfolioLegV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, SideV16, SourceCreditStateV16, SourceCreditStateV16Account,
    TradeRequestV16, V16Config, V16Error, V16PodI128, V16PodU128, V16PodU64, V16Result,
    S10_MAX_MOVES_PER_INSTRUCTION, S10_MIN_MOVE_ATOMS, S10_PROTECT_FULL_PROVIDER_PRINCIPAL,
};
use percolator::{ADL_ONE, BOUND_SCALE, CREDIT_RATE_SCALE, MAX_VAULT_TVL, MIN_A_SIDE, POS_SCALE, SOCIAL_WEIGHT_SCALE};

type View<'a> = MarketGroupV16ViewMut<'a, u64>;

// =====================================================================================
// Shared fixtures
// =====================================================================================

fn ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    ([1; 32], [2; 32], [3; 32])
}

fn empty_account_fixture(market_id: [u8; 32], account_tag: u8) -> PortfolioAccountV16Account {
    let mut account_id = [0u8; 32];
    account_id[0] = account_tag;
    let mut owner = [0u8; 32];
    owner[0] = account_tag;
    PortfolioAccountV16Account::try_empty(ProvenanceHeaderV16Account::from_runtime(
        &ProvenanceHeaderV16::new(market_id, account_id, owner),
    ))
    .unwrap()
}

/// One active asset built with the real constructor (engine PR #147 lesson).
fn one_market_only_fixture() -> (MarketGroupV16HeaderAccount, [Market<u64>; 1]) {
    let (market_id, _, _) = ids();
    let cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(market_id, cfg, 1, 0).unwrap();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    {
        let mut view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        view.activate_empty_market_not_atomic(0, 100, 1).unwrap();
    }
    (header, markets)
}

/// S10 fixture slot clock: buckets' expiry slots are `NOW - 128 + u8`, so both lapsed and
/// unexpired buckets are reachable.
const NOW: u64 = 200;

/// One backing domain of the S10 fixture, in whole atoms (A-W: `u16` atoms, a sub-atom claim
/// remainder, `u8` expiry offset). Every amount the engine stores is `atoms * BOUND_SCALE`.
#[derive(Clone, Copy)]
struct Dom {
    status: u8,
    fr: u16,
    valid: u16,
    cons: u16,
    imp: u16,
    exp: u8,
    claims: u16,
    rem: u16,
    exact_full: bool,
    spent_extra: u8,
    pp: u16,
    ins: u8,
}

fn any_dom(with_insurance: bool) -> Dom {
    Dom {
        status: kani::any(),
        fr: kani::any(),
        valid: kani::any(),
        cons: kani::any(),
        imp: kani::any(),
        exp: kani::any(),
        claims: kani::any(),
        rem: kani::any(),
        exact_full: kani::any(),
        spent_extra: kani::any(),
        pp: kani::any(),
        ins: if with_insurance { kani::any() } else { 0 },
    }
}

fn status_of(s: u8) -> BackingBucketStatusV16 {
    match s % 4 {
        0 => BackingBucketStatusV16::Empty,
        1 => BackingBucketStatusV16::Fresh,
        2 => BackingBucketStatusV16::Expired,
        _ => BackingBucketStatusV16::Impaired,
    }
}

fn n(atoms: u16) -> u128 {
    atoms as u128 * BOUND_SCALE
}

fn dom_bucket(market_id: u64, d: Dom) -> BackingBucketV16 {
    let status = status_of(d.status);
    BackingBucketV16 {
        market_id,
        fresh_unliened_backing_num: n(d.fr),
        valid_liened_backing_num: n(d.valid),
        consumed_liened_backing_num: n(d.cons),
        impaired_liened_backing_num: n(d.imp),
        utilization_fee_earnings: 0,
        expiry_slot: if status == BackingBucketStatusV16::Empty { 0 } else { NOW - 128 + d.exp as u64 },
        status,
    }
}

fn dom_source(d: Dom) -> SourceCreditStateV16 {
    let bound = n(d.claims) + d.rem as u128;
    let mut s = SourceCreditStateV16 {
        positive_claim_bound_num: bound,
        exact_positive_claim_num: if d.exact_full { n(d.claims) } else { 0 },
        fresh_reserved_backing_num: n(d.fr) + n(d.valid),
        spent_backing_num: n(d.cons) + d.spent_extra as u128 * BOUND_SCALE,
        provider_receivable_num: n(d.cons),
        valid_liened_backing_num: n(d.valid),
        impaired_liened_backing_num: n(d.imp),
        insurance_credit_reserved_num: d.ins as u128 * BOUND_SCALE,
        valid_liened_insurance_num: 0,
        impaired_liened_insurance_num: 0,
        credit_rate_num: 0,
        credit_epoch: 0,
    };
    // the stored rate is whatever the (real or, in the `_m` twins, modelled) rate function gives:
    // a state whose rate cannot be computed is filtered out by the shape assumption below
    s.credit_rate_num = kani_expected_source_credit_rate_num_for_state(s).unwrap_or(u128::MAX);
    s
}

/// The S10 world: one Live asset at `NOW`, both domains poked from `l` / `s`, mirrors set, header
/// aggregates recomputed by the real `refresh_header_aggregate_totals_for_test`, then A-SHAPE:
/// `kani::assume(validate_shape().is_ok())` (under cfg(kani) this includes the full audit scan:
/// A-AUDIT). Returns `None` when the fixture is not shape-valid (callers `assume` it away).
fn s10_world(l: Dom, s: Dom) -> Option<(MarketGroupV16HeaderAccount, [Market<u64>; 1])> {
    let (mut header, mut markets) = one_market_only_fixture();
    header.current_slot = V16PodU64::new(NOW);
    header.slot_last = V16PodU64::new(NOW);
    markets[0].engine.asset.slot_last = V16PodU64::new(NOW);
    let market_id = markets[0].engine.asset.market_id.get();
    let ins_atoms = l.ins as u128 + s.ins as u128;
    {
        let e = &mut markets[0].engine;
        e.backing_long = BackingBucketV16Account::from_runtime(&dom_bucket(market_id, l));
        e.backing_short = BackingBucketV16Account::from_runtime(&dom_bucket(market_id, s));
        e.source_credit_long = SourceCreditStateV16Account::from_runtime(&dom_source(l));
        e.source_credit_short = SourceCreditStateV16Account::from_runtime(&dom_source(s));
        e.insurance_reservation_long =
            InsuranceCreditReservationV16Account::from_runtime(&InsuranceCreditReservationV16 {
                insurance_credit_reserved_num: l.ins as u128 * BOUND_SCALE,
                ..InsuranceCreditReservationV16::EMPTY
            });
        e.insurance_reservation_short =
            InsuranceCreditReservationV16Account::from_runtime(&InsuranceCreditReservationV16 {
                insurance_credit_reserved_num: s.ins as u128 * BOUND_SCALE,
                ..InsuranceCreditReservationV16::EMPTY
            });
        e.provider_principal_long = V16PodU128::new(n(l.pp));
        e.provider_principal_short = V16PodU128::new(n(s.pp));
    }
    header.vault = V16PodU128::new(MAX_VAULT_TVL);
    header.insurance = V16PodU128::new(ins_atoms);
    {
        let mut v = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        if v.refresh_header_aggregate_totals_for_test().is_err() {
            return None;
        }
    }
    let claims_num = header.source_claim_bound_total_num.get();
    header.pnl_pos_bound_tot_num = V16PodU128::new(claims_num);
    header.pnl_pos_bound_tot = V16PodU128::new(View::kani_v22_amount_from_bound_num(claims_num).ok()?);
    header.pnl_pos_tot = V16PodU128::new(0);
    header.pnl_matured_pos_tot = V16PodU128::new(0);
    let ok = MarketGroupV16ViewMut::new(&mut header, &mut markets).validate_shape().is_ok();
    if ok {
        Some((header, markets))
    } else {
        None
    }
}

fn bucket(m: &[Market<u64>; 1], d: usize) -> BackingBucketV16 {
    if d == 0 { m[0].engine.backing_long.try_to_runtime().unwrap() } else { m[0].engine.backing_short.try_to_runtime().unwrap() }
}
fn source(m: &[Market<u64>; 1], d: usize) -> SourceCreditStateV16 {
    if d == 0 { m[0].engine.source_credit_long.try_to_runtime().unwrap() } else { m[0].engine.source_credit_short.try_to_runtime().unwrap() }
}
fn pp_of(m: &[Market<u64>; 1], d: usize) -> u128 {
    if d == 0 { m[0].engine.provider_principal_long.get() } else { m[0].engine.provider_principal_short.get() }
}
fn av(s: SourceCreditStateV16) -> u128 {
    kani_available_backing_num_for_source_credit_state(s).unwrap()
}

/// Independent spec of the S10 move (E-S10-6): the first direction, in the code's order
/// (long->short, then short->long), whose guards hold and whose rounded amount reaches the dust
/// floor fires with exactly that amount (rule A: loser cash = fresh above the principal mirror).
/// Returns `(src, dst, amount_num)`, or `None` when nothing may move.
fn s10_spec(m: &[Market<u64>; 1], budget: u8) -> Option<(usize, usize, u128)> {
    if budget == 0 {
        return None;
    }
    for (src, dst) in [(0usize, 1usize), (1, 0)] {
        let (bs, ss, bd, sd) = (bucket(m, src), source(m, src), bucket(m, dst), source(m, dst));
        let lc = bs.fresh_unliened_backing_num.saturating_sub(pp_of(m, src));
        if lc == 0 || bs.status != BackingBucketStatusV16::Fresh || bs.expiry_slot <= NOW || bs.fresh_unliened_backing_num == 0 {
            continue;
        }
        let accepts = match bd.status {
            BackingBucketStatusV16::Empty | BackingBucketStatusV16::Expired => true,
            BackingBucketStatusV16::Fresh => bd.expiry_slot > NOW,
            BackingBucketStatusV16::Impaired => false,
        };
        if !accepts {
            continue;
        }
        let excess = lc.min(av(ss).saturating_sub(ss.positive_claim_bound_num));
        let shortfall = sd.positive_claim_bound_num.saturating_sub(av(sd));
        let x = excess.min(shortfall) / BOUND_SCALE * BOUND_SCALE;
        if x < S10_MIN_MOVE_ATOMS * BOUND_SCALE {
            continue;
        }
        return Some((src, dst, x));
    }
    None
}

/// The S10 body shared by the real-rate harness and its rate-model twin (E-S10-1, 2, 5, 6, 7, 9,

/// Deterministic checked rate model (design rev2 R2.4, A-RATE): keeps the real shape and
/// `available` error checks, so it errs exactly where the real function errs before its U256
/// step, then returns a fixed function of (claims, available) capped at the scale.
fn s10_rate_model(state: SourceCreditStateV16) -> V16Result<u128> {
    View::kani_v22_source_credit_shape_static(state)?;
    let available = kani_available_backing_num_for_source_credit_state(state)?;
    if state.positive_claim_bound_num == 0 || available >= state.positive_claim_bound_num {
        Ok(CREDIT_RATE_SCALE)
    } else {
        Ok(0)
    }
}

/// E-S10-1, 2, 5, 6, 7, 9, 10 on the REAL U256 rate. Class L (heavy lane), fallback: the `_m` twin.
/// Assumptions A-SHAPE (+A-AUDIT), A-W, A-LIVE, A-RULE. Mutants S10-M1, M2, M3, M4 (may be

// =====================================================================================
// Overnight additions
// =====================================================================================

use percolator::wide_math::U256;

/// RESTRICTED STUB (sound by construction): the real `div_rem_u256` (wide_math.rs:1103) with its
/// binary-long-division branch replaced by a FAILING assertion. On every input on which this stub
/// does not fail it returns exactly what the real function returns (the panic, the two early
/// returns and the native-u128 branch are copied verbatim); on every other input the harness
/// FAILS. So a PASS with this stub is a PASS of the real function, plus a machine-checked proof
/// that the loop branch is unreachable in that harness. (Removes the 256-iteration symbolic
/// unwinding that OOM'd the real-rate S10 rows: every S10-world U256 division has num, den < 2^128
/// because claims/available < 2^17 * BOUND_SCALE and CREDIT_RATE_SCALE < 2^40.)
fn div_rem_u256_native_only(num: U256, den: U256) -> (U256, U256) {
    if den.is_zero() {
        panic!("U256 division by zero");
    }
    if num.is_zero() {
        return (U256::ZERO, U256::ZERO);
    }
    if den > num {
        return (U256::ZERO, num);
    }
    assert!(num.hi() == 0 && den.hi() == 0, "restricted stub: only the native-u128 branch is reachable");
    let q = num.lo() / den.lo();
    let r = num.lo() % den.lo();
    (U256::from_u128(q), U256::from_u128(r))
}

/// PROBE (diagnostic only, not evidence): cost of the S10 fixture alone under the model rate.
#[kani::proof]
#[kani::stub(percolator::v16::V16Core::expected_source_credit_rate_num_for_state, s10_rate_model)]
#[kani::solver(kissat)]
fn probe_s10_world_m() {
    let w = s10_world(any_dom(false), any_dom(false));
    kani::assume(w.is_some());
    kani::cover!(true, "fixture shape-valid");
}

/// PROBE (diagnostic only, not evidence): cost of the S10 fixture alone, real rate, restricted div.
#[kani::proof]
#[kani::stub(percolator::wide_math::div_rem_u256, div_rem_u256_native_only)]
#[kani::solver(kissat)]
fn probe_s10_world_div() {
    let w = s10_world(any_dom(false), any_dom(false));
    kani::assume(w.is_some());
    kani::cover!(true, "fixture shape-valid");
}

// ---- verbatim copy of tests/proofs_v22_final.rs lines 754-851 (real-operation worlds) ----
// =====================================================================================
// Real-operation worlds (concrete amounts) for E-S10-17, E-X1-1, E-W4-3/4
// =====================================================================================

const PRICE: u64 = 1_000_000;

fn scenario_account(seed: u32) -> PortfolioAccountV16Account {
    let mut key = [0u8; 32];
    key[..4].copy_from_slice(&seed.to_le_bytes());
    key[31] = 0x5A;
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new([1; 32], key, [3; 32]));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(header).unwrap();
    a
}

/// `n_assets`-asset world, the configuration of `tests/v22_s10_cap.rs` (assets activated at slots
/// 1, 3, 5: the activation cooldown).
struct World<const N: usize> {
    header: MarketGroupV16HeaderAccount,
    markets: [Market<u64>; N],
    slot: u64,
}

impl<const N: usize> World<N> {
    fn new() -> Self {
        let mut cfg = V16Config::public_user_fund_with_market_slots(N as u16, N as u32, 0, 6_480_000);
        cfg.max_abs_funding_e9_per_slot = 10_000;
        cfg.max_price_move_bps_per_slot = 200;
        cfg.initial_margin_bps = 1_000;
        cfg.maintenance_margin_bps = 500;
        cfg.max_accrual_dt_slots = 2;
        cfg.min_funding_lifetime_slots = 10_000_000;
        let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, N as u32, 0).unwrap();
        let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default()); N];
        for (i, m) in markets.iter_mut().enumerate() {
            m.wrapper = i as u64;
        }
        for i in 0..N {
            header.activate_empty_asset_slot_not_atomic(i as u32, &mut markets[i].engine, PRICE, 1 + 2 * i as u64).unwrap();
        }
        World { header, markets, slot: 8 }
    }
    fn view(&mut self) -> MarketGroupV16ViewMut<'_, u64> {
        MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets)
    }
    fn deposit(&mut self, a: &mut PortfolioAccountV16Account, amt: u128) {
        self.view().deposit_not_atomic(&mut PortfolioV16ViewMut::new(a), amt).unwrap();
    }
    fn trade(&mut self, i: usize, long: &mut PortfolioAccountV16Account, short: &mut PortfolioAccountV16Account, size: u128) {
        let price = self.markets[i].engine.asset.effective_price.get();
        self.view()
            .execute_trade_with_fee_loss_stale_scoped_not_atomic(
                &mut PortfolioV16ViewMut::new(long),
                &mut PortfolioV16ViewMut::new(short),
                TradeRequestV16 { asset_index: i, size_q: size as i128, exec_price: price, fee_bps: 0 },
                true,
            )
            .unwrap();
    }
    fn tick(&mut self, bps: i64) {
        self.slot += 1;
        for i in 0..N {
            let old = self.markets[i].engine.asset.effective_price.get() as i128;
            let new = (old + old * bps as i128 / 10_000) as u64;
            let slot = self.slot;
            self.view().accrue_asset_to_not_atomic(i, slot, new, 0, true).unwrap();
            self.markets[i].engine.asset.raw_oracle_target_price = V16PodU64::new(new);
        }
    }
    fn refresh(&mut self, a: &mut PortfolioAccountV16Account, budget: u8) -> V16Result<()> {
        let mut b = budget;
        self.view().full_account_refresh_with_s10_budget_not_atomic(&mut PortfolioV16ViewMut::new(a), &mut b).map(|_| ())
    }
}

/// The stranded state of `tests/v22_s10_cap.rs::stranded` on `N` assets: the maker settled alone
/// at the peak, the price fell below entry, the trader settled. Returns (world, maker, trader).
fn stranded<const N: usize>() -> (World<N>, PortfolioAccountV16Account, PortfolioAccountV16Account) {
    let mut w = World::<N>::new();
    let mut a = scenario_account(300);
    let mut maker = scenario_account(0);
    w.deposit(&mut a, 1_000_000_000_000_000);
    w.deposit(&mut maker, 1_000_000_000_000_000);
    for i in 0..N {
        w.trade(i, &mut a, &mut maker, 400 * POS_SCALE);
    }
    for _ in 0..3 {
        w.tick(150);
    }
    w.refresh(&mut maker, S10_MAX_MOVES_PER_INSTRUCTION).unwrap();
    for _ in 0..8 {
        w.tick(-150);
    }
    w.refresh(&mut a, S10_MAX_MOVES_PER_INSTRUCTION).unwrap();
    (w, maker, a)
}


// =====================================================================================
// Row proof_v22_s10_settle_entry_frame (official: NO-VERDICT, timeout 4004 s)
// =====================================================================================
// Diagnosis: the stranded world is CONCRETE; the only symbolic input is the budget b <= 2. With
// b symbolic the second refresh runs on an ite-merged state, so the U256 long division
// (`div_rem_u256`, a `while i >= 0` loop with a data-dependent count) gets a SYMBOLIC trip count
// and CBMC unwinds it without bound (official log: iteration 4528 at the timeout).
// Split: CASE b = 0, 1, 2 (exhaustive: the original assumes b <= S10_MAX_MOVES_PER_INSTRUCTION
// = 2, a u8). Each case runs the original body verbatim with b concrete, so every loop has a
// concrete trip count. The conjunction of the three cases is exactly the original.

fn settle_entry_frame_body(b: u8) {
    let (w, maker, _a) = stranded::<1>();
    kani::cover!(true, "stranded world built");
    let (mut w0, mut m0) = (World::<1> { header: w.header, markets: w.markets, slot: w.slot }, maker);
    let (mut wb, mut mb) = (World::<1> { header: w.header, markets: w.markets, slot: w.slot }, maker);
    let r0 = w0.refresh(&mut m0, 0);
    let rb = wb.refresh(&mut mb, b);
    assert_eq!(r0.is_ok(), rb.is_ok());
    kani::assume(r0.is_ok());
    let e0 = &w0.markets[0].engine;
    let eb = &wb.markets[0].engine;
    assert!(eb.kf_pending_credit_long == e0.kf_pending_credit_long && eb.kf_pending_credit_short == e0.kf_pending_credit_short);
    let mut ex = *eb;
    ex.backing_long = e0.backing_long;
    ex.backing_short = e0.backing_short;
    ex.source_credit_long = e0.source_credit_long;
    ex.source_credit_short = e0.source_credit_short;
    assert!(ex == *e0, "slot frame: only the two domains differ");
    let mut hx = wb.header;
    hx.risk_epoch = w0.header.risk_epoch;
    hx.backing_provider_earnings_total = w0.header.backing_provider_earnings_total;
    hx.source_fresh_backing_total_num = w0.header.source_fresh_backing_total_num;
    assert!(hx == w0.header, "header frame");
    let mut ax = mb;
    ax.health_cert = m0.health_cert;
    assert!(ax == m0, "account frame: only the certificate's epochs may differ");
    let moved = eb.backing_short != e0.backing_short || eb.backing_long != e0.backing_long;
    if b == 0 {
        assert!(!moved);
    }
    kani::cover!(b == 0 || moved, "b > 0: the move fires inside the settle entry");
    kani::cover!(r0.is_ok(), "both refreshes Ok (the frame assertions are reached)");
}

#[kani::proof]
#[kani::solver(kissat)]
fn proof_v22_s10_settle_entry_frame_b0() {
    settle_entry_frame_body(0);
}

#[kani::proof]
#[kani::solver(kissat)]
fn proof_v22_s10_settle_entry_frame_b1() {
    settle_entry_frame_body(1);
}

#[kani::proof]
#[kani::solver(kissat)]
fn proof_v22_s10_settle_entry_frame_b2() {
    settle_entry_frame_body(2);
}

// =====================================================================================
// Row proof_v22_s10_grant_only_refresh_moves (official: NO-VERDICT-OOM)
// =====================================================================================
// Diagnosis: the world is concrete; action (4), mode (3) and reason (9) are symbolic, so symex
// merges the whole crank state across every action path (refresh, B-settle, liquidation,
// recovery) and modes. Split: CASE (action, mode) with action and mode concrete (exhaustive:
// sel % 4 ranges over the 4 actions, mode <= 2 over 3 modes = 12 cases); grant stays a symbolic
// u8 in every case; the Recover cases keep the reason symbolic (all 9). Each case asserts exactly
// the original's assertions that apply to it, so the conjunction is the original.

const ACT_REFRESH: u8 = 0;
const ACT_SETTLEB: u8 = 1;
const ACT_LIQUIDATE: u8 = 2;
const ACT_RECOVER: u8 = 3;

fn grant_body(sel: u8, mode: u8) {
    let l = Dom { status: 1, fr: 50_000, valid: 0, cons: 0, imp: 0, exp: 200, claims: 0, rem: 0, exact_full: true, spent_extra: 0, pp: 0, ins: 0 };
    let s = Dom { status: 0, fr: 0, valid: 0, cons: 0, imp: 0, exp: 0, claims: 10_000, rem: 0, exact_full: true, spent_extra: 0, pp: 0, ins: 0 };
    let world = s10_world(l, s);
    kani::assume(world.is_some());
    let (mut header, mut markets) = world.unwrap();
    kani::cover!(true, "grant fixture is shape-valid");
    let (market_id, _, _) = ids();
    let mut account = empty_account_fixture(market_id, 2);
    let reason_sel: u8 = if sel == ACT_RECOVER { kani::any() } else { 0 };
    let reason = match reason_sel % 9 {
        0 => PermissionlessRecoveryReasonV16::BelowProgressFloor,
        1 => PermissionlessRecoveryReasonV16::BlockedSegmentHeadroomOrRepresentability,
        2 => PermissionlessRecoveryReasonV16::AccountBSettlementCannotProgress,
        3 => PermissionlessRecoveryReasonV16::BIndexHeadroomExhausted,
        4 => PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress,
        5 => PermissionlessRecoveryReasonV16::ExplicitLossOrDustAuditOverflow,
        6 => PermissionlessRecoveryReasonV16::OracleOrTargetUnavailableByAuthenticatedPolicy,
        7 => PermissionlessRecoveryReasonV16::CounterOrEpochOverflowDeclaredRecovery,
        _ => PermissionlessRecoveryReasonV16::BandPinExpired,
    };
    let action = match sel {
        ACT_REFRESH => PermissionlessCrankActionV16::Refresh,
        ACT_SETTLEB => PermissionlessCrankActionV16::SettleB { asset_index: 0 },
        ACT_LIQUIDATE => PermissionlessCrankActionV16::Liquidate(LiquidationRequestV16 { asset_index: 0 }),
        _ => PermissionlessCrankActionV16::Recover(reason),
    };
    let is_refresh = sel == ACT_REFRESH;
    let is_recover = sel == ACT_RECOVER;
    header.mode = mode; // 0 Live, 1 Resolved, 2 Recovery (set after the shape assumption)
    let grant: u8 = kani::any();
    let price = markets[0].engine.asset.effective_price.get();
    let (h0, m0, a0) = (header, markets, account);
    let r = {
        let mut v = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut acc = PortfolioV16ViewMut::new(&mut account);
        v.permissionless_crank_s10_not_atomic(
            &mut acc,
            PermissionlessCrankRequestV16 { now_slot: NOW, asset_index: 0, effective_price: price, funding_rate_e9: 0, action },
            grant,
        )
    };
    let moved = bucket(&markets, 0).fresh_unliened_backing_num < bucket(&m0, 0).fresh_unliened_backing_num;
    let domains_unchanged = markets[0].engine.backing_long == m0[0].engine.backing_long
        && markets[0].engine.backing_short == m0[0].engine.backing_short
        && markets[0].engine.source_credit_long == m0[0].engine.source_credit_long
        && markets[0].engine.source_credit_short == m0[0].engine.source_credit_short;
    if !is_refresh {
        assert!(domains_unchanged, "no S10 move outside the Refresh action, any grant");
    }
    if mode != 0 && !is_recover {
        assert_eq!(r, Err(V16Error::LockActive));
        assert!(header == h0 && markets == m0 && account == a0, "refused before any mutation");
    }
    if is_refresh && grant == 0 {
        assert!(domains_unchanged, "Refresh without a grant moves nothing");
    }
    // per-case non-vacuity: the case's own branch is reached for a non-trivial grant
    kani::cover!(grant == 2, "case reached with grant 2");
    kani::cover!(grant == 0, "case reached with grant 0");
    if is_refresh && mode == 0 {
        kani::cover!(grant > 0 && r.is_ok() && moved, "Refresh with a grant moves");
    }
}

macro_rules! grant_case {
    ($name:ident, $sel:expr, $mode:expr) => {
        #[kani::proof]
        #[kani::solver(kissat)]
        fn $name() {
            grant_body($sel, $mode);
        }
    };
}
grant_case!(proof_v22_s10_grant_refresh_live, ACT_REFRESH, 0);
grant_case!(proof_v22_s10_grant_settleb_live, ACT_SETTLEB, 0);
grant_case!(proof_v22_s10_grant_liquidate_live, ACT_LIQUIDATE, 0);
grant_case!(proof_v22_s10_grant_recover_live, ACT_RECOVER, 0);
grant_case!(proof_v22_s10_grant_recover_resolved, ACT_RECOVER, 1);
grant_case!(proof_v22_s10_grant_recover_recovery, ACT_RECOVER, 2);

/// The six (non-Recover action, non-Live mode) cases in ONE harness: action in {Refresh, SettleB,
/// Liquidate} and mode in {Resolved, Recovery} stay symbolic; all six exit at the mode check
/// (v16.rs:18169) before any mutation, so symex does not merge any deep path. Same body.
#[kani::proof]
#[kani::solver(kissat)]
fn proof_v22_s10_grant_nonrecover_nonlive() {
    let sel: u8 = kani::any();
    kani::assume(sel <= ACT_LIQUIDATE);
    let mode: u8 = kani::any();
    kani::assume(mode == 1 || mode == 2);
    grant_body(sel, mode);
    kani::cover!(sel == ACT_REFRESH && mode == 1, "refresh resolved");
    kani::cover!(sel == ACT_SETTLEB && mode == 2, "settleb recovery");
    kani::cover!(sel == ACT_LIQUIDATE && mode == 1, "liquidate resolved");
}

// =====================================================================================
// Row proof_v22_s10_retry_hook_guard_m (official: NO-VERDICT-OOM, 16 GB and 32 GB)
// =====================================================================================
// Original (E-S10-11): budget 0 or any of the four counters non-zero => Ok + identity; else the
// hook's result and post-state equal the move's (run on a clone).
// The hook (`s10_retry_for_unpositioned_asset`, v16.rs:18265) is 12 lines: a guard on the budget,
// the index, and four counters read from the decoded asset, then `self.rebalance..(i, b)?; Ok(())`.
// Decomposition (modular, callee abstracted):
//   * the move is replaced by a SPY stub that records (call count, asset_index, *budget), does not
//     touch the state, and returns a symbolic choice of Ok(()) or an Err;
//   * the harness proves: guard false => the spy is never called, Ok, state byte-identical;
//     guard true => the spy is called exactly once, with the same asset_index and budget value,
//     the hook returns exactly the spy's result, and the hook itself mutates nothing.
// Why this implies the original: in the guard-true case the hook's only effect is the callee's
// effect and its return value is the callee's (the continuation after the call is `?; Ok(())`,
// which neither reads nor writes state and passes any Err through unchanged: `?` is parametric in
// the error value, so one representative Err suffices); the real move is deterministic, so the
// hook's result/post-state equals the move's on the clone. In the guard-false case the original's
// assertion is exactly what is proven. The state here is the concrete one-asset fixture with the
// four counters ARBITRARY u64 (not only the shape-valid ones the original admits): STRONGER
// (no shape assumption is needed because the guard reads only the counters).

static mut SPY_CALLS: u32 = 0;
static mut SPY_IDX: usize = usize::MAX;
static mut SPY_BUDGET: u8 = 0;
static mut SPY_OK: bool = true;

fn rebalance_spy<'a, T>(_v: &mut MarketGroupV16ViewMut<'a, T>, asset_index: usize, budget: &mut u8) -> V16Result<()> {
    let ok: bool = kani::any();
    unsafe {
        SPY_CALLS += 1;
        SPY_IDX = asset_index;
        SPY_BUDGET = *budget;
        SPY_OK = ok;
    }
    if ok { Ok(()) } else { Err(V16Error::ArithmeticOverflow) }
}

#[kani::proof]
#[kani::stub(percolator::v16::MarketGroupV16ViewMut::rebalance_unclaimed_backing_across_asset_domains_not_atomic, rebalance_spy)]
#[kani::solver(kissat)]
fn proof_v22_s10_retry_hook_guard_split_spy() {
    let (mut header, mut markets) = one_market_only_fixture();
    let c: [u64; 4] = kani::any();
    {
        let slot = &mut markets[0].engine;
        let mut a = slot.asset.try_to_runtime().unwrap();
        a.stale_account_count_long = c[0];
        a.stale_account_count_short = c[1];
        a.stored_pos_count_long = c[2];
        a.stored_pos_count_short = c[3];
        slot.asset = AssetStateV16Account::from_runtime(&a);
    }
    let b0: u8 = kani::any();
    kani::assume(b0 <= S10_MAX_MOVES_PER_INSTRUCTION);
    let (h0, m0) = (header, markets);
    let mut b = b0;
    let r = MarketGroupV16ViewMut::new(&mut header, &mut markets).kani_v22_s10_retry(0, &mut b);
    let any_count = c.iter().any(|x| *x != 0);
    let (calls, idx, bud, ok) = unsafe { (SPY_CALLS, SPY_IDX, SPY_BUDGET, SPY_OK) };
    assert!(header == h0 && markets == m0, "the hook itself mutates nothing");
    assert_eq!(b, b0, "the hook itself does not touch the budget");
    if b0 == 0 || any_count {
        assert_eq!(calls, 0, "hook skipped => the move is not run");
        assert!(r.is_ok());
    } else {
        assert_eq!(calls, 1, "guard passes => the move runs exactly once");
        assert_eq!(idx, 0);
        assert_eq!(bud, b0);
        assert_eq!(r.is_ok(), ok, "the hook returns the move's result");
        if !ok {
            assert_eq!(r, Err(V16Error::ArithmeticOverflow));
        }
    }
    kani::cover!(b0 > 0 && c[0] != 0 && c[1] == 0 && c[2] == 0 && c[3] == 0, "skip: stale long only");
    kani::cover!(b0 > 0 && c[1] != 0 && c[0] == 0 && c[2] == 0 && c[3] == 0, "skip: stale short only");
    kani::cover!(b0 > 0 && c[2] != 0 && c[0] == 0 && c[1] == 0 && c[3] == 0, "skip: stored long only");
    kani::cover!(b0 > 0 && c[3] != 0 && c[0] == 0 && c[1] == 0 && c[2] == 0, "skip: stored short only");
    kani::cover!(b0 == 0 && !any_count, "skip: budget 0");
    kani::cover!(b0 > 0 && !any_count && ok, "hook runs the move (Ok)");
    kani::cover!(b0 > 0 && !any_count && !ok, "hook runs the move (Err propagated)");
}

// =====================================================================================
// Row proof_v22_s10_wholly_empty_reset (official: NO-VERDICT-OOM)
// =====================================================================================
// Diagnosis: the setter is ~20 lines; the cost is entirely the fixture (`s10_world`: real U256
// rate twice per domain plus the full audit scan, twice). The setter (`set_backing_bucket_for_
// domain`, v16.rs:10195) reads only: the domain -> (asset, side) map, the old bucket of that
// domain, the header's `backing_provider_earnings_total`; it writes only that total, the bucket
// and (if wholly empty) that side's principal mirror.
// Replacement: WEAKER precondition, hence a STRONGER statement: the concrete one-asset fixture with
// BOTH old buckets, BOTH principal mirrors, the earnings total and the new bucket fully symbolic
// (every u128 field arbitrary, every status) -- a superset of the original's shape-valid
// `s10_world` states and of its `dom_bucket` new buckets. Same assertions as the original (under
// the same `assume(r.is_ok())`), plus a full frame (nothing else in the header or slot changes).

fn any_bucket(market_id: u64) -> BackingBucketV16 {
    let st: u8 = kani::any();
    let status = status_of(st);
    BackingBucketV16 {
        market_id,
        fresh_unliened_backing_num: kani::any(),
        valid_liened_backing_num: kani::any(),
        consumed_liened_backing_num: kani::any(),
        impaired_liened_backing_num: kani::any(),
        utilization_fee_earnings: kani::any(),
        expiry_slot: kani::any(),
        status,
    }
}

#[kani::proof]
#[kani::solver(kissat)]
fn proof_v22_s10_wholly_empty_reset_any_state() {
    let (mut header, mut markets) = one_market_only_fixture();
    let market_id = markets[0].engine.asset.market_id.get();
    {
        let e = &mut markets[0].engine;
        e.backing_long = BackingBucketV16Account::from_runtime(&any_bucket(market_id));
        e.backing_short = BackingBucketV16Account::from_runtime(&any_bucket(market_id));
        e.provider_principal_long = V16PodU128::new(kani::any());
        e.provider_principal_short = V16PodU128::new(kani::any());
    }
    header.backing_provider_earnings_total = V16PodU128::new(kani::any());
    let d: usize = if kani::any::<bool>() { 1 } else { 0 };
    let nb = any_bucket(market_id);
    let old = bucket(&markets, d);
    let (h0, m0) = (header, markets);
    let r = MarketGroupV16ViewMut::new(&mut header, &mut markets).kani_v22_set_backing_bucket_for_domain(d, nb);
    kani::assume(r.is_ok());
    let wholly_empty = nb.fresh_unliened_backing_num == 0 && nb.valid_liened_backing_num == 0
        && nb.consumed_liened_backing_num == 0 && nb.impaired_liened_backing_num == 0;
    if wholly_empty {
        assert_eq!(pp_of(&markets, d), 0);
    } else {
        assert_eq!(pp_of(&markets, d), pp_of(&m0, d));
    }
    assert_eq!(pp_of(&markets, 1 - d), pp_of(&m0, 1 - d), "other side's mirror untouched");
    assert_eq!(bucket(&markets, d), nb);
    assert_eq!(
        header.backing_provider_earnings_total.get() + old.utilization_fee_earnings,
        h0.backing_provider_earnings_total.get() + nb.utilization_fee_earnings
    );
    // frame (extra): nothing else changes
    assert!(bucket(&markets, 1 - d) == bucket(&m0, 1 - d));
    let mut hx = header;
    hx.backing_provider_earnings_total = h0.backing_provider_earnings_total;
    assert!(hx == h0, "header frame");
    let mut ex = markets[0].engine;
    ex.backing_long = m0[0].engine.backing_long;
    ex.backing_short = m0[0].engine.backing_short;
    ex.provider_principal_long = m0[0].engine.provider_principal_long;
    ex.provider_principal_short = m0[0].engine.provider_principal_short;
    assert!(ex == m0[0].engine, "slot frame");
    kani::cover!(wholly_empty && pp_of(&m0, d) > 0, "reset fires");
    kani::cover!(!wholly_empty && nb.fresh_unliened_backing_num == 0 && pp_of(&m0, d) > 0, "fresh 0 but not wholly empty: no reset");
    kani::cover!(d == 0 && wholly_empty, "long side");
    kani::cover!(d == 1 && wholly_empty, "short side");
}

// =====================================================================================
// Rows proof_v22_s10_move_conserves_bounds_exact_total{,_m,_ins_m} (official: NO-VERDICT-OOM)
// =====================================================================================
// Replacement: the ORIGINAL body (`s10_move_body`, copied below verbatim except as marked) run
//   (1) on the REAL rate function (stronger than the `_m` model-rate twins), with the U256 long
//       division replaced by the restricted stub `div_rem_u256_native_only` (sound: the stub fails
//       the proof if the loop branch is ever reachable), and
//   (2) split by CASE over the two bucket statuses (sl, ss) in {Empty, Fresh, Expired, Impaired}^2:
//       16 cases per insurance flavour. Exhaustive: the fixture maps Dom.status through
//       status_of(x) = x % 4, so fixing sl, ss in 0..4 partitions the original u8 x u8 status space
//       into 16 classes with identical worlds. All other Dom fields stay symbolic (same widths).
// Covers: each original cover is gated by a CONCRETE applicability predicate of the case
// (`!(applicable) || cond`): a cover is required to be SATISFIED in every case where its
// precondition statuses hold, and is vacuous-true elsewhere. Every original cover is applicable
// in at least one case, so the union over the 16 cases witnesses every original cover.
// Composition: original(with_insurance) = AND over (sl, ss) of case(with_insurance, sl, ss);
// `_m` twin is subsumed by the real-rate no-insurance family, `_ins_m` by the insurance family
// (the `_m` rate model was a fallback evidence label for the real rate, design rev2 R2.4).

#[derive(Clone, Copy)]
struct CaseApp {
    l_fresh: bool,
    l_to_s: bool,
    s_to_l: bool,
    any_fire: bool,
    any_fresh: bool,
    dst_empty: bool,
    dst_expired: bool,
    refill: bool,
}

impl CaseApp {
    /// status codes: 0 Empty, 1 Fresh, 2 Expired, 3 Impaired. A move needs a Fresh source and a
    /// non-Impaired destination; only Fresh buckets hold fresh backing; Empty buckets hold no
    /// consumed backing (so a receivable refill needs a Fresh or Expired destination).
    fn of(sl: u8, ss: u8) -> Self {
        let l_to_s = sl == 1 && ss != 3;
        let s_to_l = ss == 1 && sl != 3;
        CaseApp {
            l_fresh: sl == 1,
            l_to_s,
            s_to_l,
            any_fire: l_to_s || s_to_l,
            any_fresh: sl == 1 || ss == 1,
            dst_empty: (sl == 1 && ss == 0) || (ss == 1 && sl == 0),
            dst_expired: (sl == 1 && ss == 2) || (ss == 1 && sl == 2),
            refill: (sl == 1 && (ss == 1 || ss == 2)) || (ss == 1 && (sl == 1 || sl == 2)),
        }
    }
}

fn s10_move_case_body(with_insurance: bool, sl: u8, ss: u8) {
    let mut l = any_dom(with_insurance);
    let mut s = any_dom(with_insurance);
    // CASE: the two bucket statuses are fixed (status_of(x) = x % 4, so sl, ss in 0..4 range over
    // exactly the original status set); every other Dom field stays symbolic.
    l.status = sl;
    s.status = ss;
    let app = CaseApp::of(sl, ss);
    let world = s10_world(l, s);
    kani::assume(world.is_some());
    let (mut header, mut markets) = world.unwrap();
    kani::cover!(true, "S10 fixture is shape-valid");
    // A-RULE: S10_PROTECT_FULL_PROVIDER_PRINCIPAL is a const (rule A); the spec below assumes rule A,
    // so flipping the const (mutant S10-E1) turns the exactness assertion red.
    let budget0: u8 = kani::any();
    kani::assume(budget0 <= S10_MAX_MOVES_PER_INSTRUCTION);
    let mut budget = budget0;
    let (h0, m0) = (header, markets);
    let spec = s10_spec(&m0, budget0);
    let r = {
        let mut v = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        v.rebalance_unclaimed_backing_for_test_not_atomic(0, &mut budget)
    };
    // E-S10-7 totality: every shape-valid state returns Ok (no new fail-closed path in a settle)
    assert!(r.is_ok(), "S10 move is total on shape-valid states");

    let (b0, b1) = ((bucket(&m0, 0), bucket(&m0, 1)), (bucket(&markets, 0), bucket(&markets, 1)));
    let pre = [b0.0, b0.1];
    let post = [b1.0, b1.1];
    let spre = [source(&m0, 0), source(&m0, 1)];
    let spost = [source(&markets, 0), source(&markets, 1)];

    // E-S10-6 exactness (independent spec) and E-S10-2 bound; E-S10-5 budget
    match spec {
        Some((src, dst, x)) => {
            assert_eq!(post[src].fresh_unliened_backing_num, pre[src].fresh_unliened_backing_num - x);
            assert_eq!(post[dst].fresh_unliened_backing_num, pre[dst].fresh_unliened_backing_num + x);
            assert!(x % BOUND_SCALE == 0 && x >= S10_MIN_MOVE_ATOMS * BOUND_SCALE);
            assert!(post[src].fresh_unliened_backing_num >= pre[src].fresh_unliened_backing_num.min(pp_of(&m0, src)));
            assert!(av(spost[dst]) <= spost[dst].positive_claim_bound_num, "destination never over-covered");
            assert!(av(spost[src]) >= spost[src].positive_claim_bound_num, "source claimants keep coverage");
            assert_eq!(budget, budget0 - 1);
            assert_eq!(header.risk_epoch.get(), h0.risk_epoch.get() + 2);
            assert_eq!(spost[0].credit_epoch, spre[0].credit_epoch + 1);
            assert_eq!(spost[1].credit_epoch, spre[1].credit_epoch + 1);
            // E-S10-9 destination expiry rule
            if pre[dst].status == BackingBucketStatusV16::Fresh && pre[dst].expiry_slot > NOW {
                assert_eq!(post[dst].expiry_slot, pre[dst].expiry_slot);
            } else {
                assert!(post[dst].expiry_slot > NOW);
            }
            assert_eq!(post[dst].status, BackingBucketStatusV16::Fresh);
            // E-S10-9 source status transition (prepare_counterparty_backing_withdraw_delta)
            if post[src].fresh_unliened_backing_num == 0 && post[src].valid_liened_backing_num == 0 {
                if post[src].impaired_liened_backing_num != 0 {
                    assert_eq!(post[src].status, BackingBucketStatusV16::Impaired);
                } else if post[src].consumed_liened_backing_num != 0 {
                    assert_eq!(post[src].status, BackingBucketStatusV16::Expired);
                } else {
                    assert_eq!(post[src].status, BackingBucketStatusV16::Empty);
                }
            } else {
                assert_eq!(post[src].status, pre[src].status);
            }
            // receivable refill on the destination only
            let refill = x.min(spre[dst].provider_receivable_num);
            assert_eq!(spost[dst].provider_receivable_num, spre[dst].provider_receivable_num - refill);
            assert_eq!(post[dst].consumed_liened_backing_num, pre[dst].consumed_liened_backing_num - refill);
            assert_eq!(spost[src].provider_receivable_num, spre[src].provider_receivable_num);
            // E-S10-9 ledger
            let v = MarketGroupV16ViewMut::new(&mut header, &mut markets);
            assert!(v.kani_validate_source_domain_ledger_current(0).is_ok());
            assert!(v.kani_validate_source_domain_ledger_current(1).is_ok());
        }
        None => {
            assert!(header == h0 && markets == m0, "nothing moves => byte-identical");
            assert_eq!(budget, budget0);
        }
    }
    // E-S10-1 conservation (also trivially true when nothing moved)
    assert_eq!(
        pre[0].fresh_unliened_backing_num + pre[1].fresh_unliened_backing_num,
        post[0].fresh_unliened_backing_num + post[1].fresh_unliened_backing_num
    );
    assert_eq!(
        spre[0].fresh_reserved_backing_num + spre[1].fresh_reserved_backing_num,
        spost[0].fresh_reserved_backing_num + spost[1].fresh_reserved_backing_num
    );
    for d in 0..2 {
        assert_eq!(post[d].valid_liened_backing_num, pre[d].valid_liened_backing_num);
        assert_eq!(post[d].impaired_liened_backing_num, pre[d].impaired_liened_backing_num);
        assert_eq!(spost[d].spent_backing_num, spre[d].spent_backing_num);
        assert_eq!(spost[d].positive_claim_bound_num, spre[d].positive_claim_bound_num);
        assert_eq!(spost[d].exact_positive_claim_num, spre[d].exact_positive_claim_num);
        assert_eq!(spost[d].insurance_credit_reserved_num, spre[d].insurance_credit_reserved_num);
        assert_eq!(spost[d].valid_liened_insurance_num, spre[d].valid_liened_insurance_num);
        assert_eq!(spost[d].impaired_liened_insurance_num, spre[d].impaired_liened_insurance_num);
        assert!(spost[d].provider_receivable_num <= spre[d].provider_receivable_num);
        assert_eq!(
            pre[d].consumed_liened_backing_num - post[d].consumed_liened_backing_num,
            spre[d].provider_receivable_num - spost[d].provider_receivable_num
        );
        assert_eq!(pp_of(&markets, d), pp_of(&m0, d), "provider mirror untouched (rule A)");
    }
    let (e0, e1) = (&m0[0].engine, &markets[0].engine);
    assert!(e1.asset == e0.asset && e1.kf_drift_long == e0.kf_drift_long && e1.kf_drift_short == e0.kf_drift_short);
    assert!(e1.kf_pending_credit_long == e0.kf_pending_credit_long && e1.kf_pending_credit_short == e0.kf_pending_credit_short);
    assert!(e1.insurance_reservation_long == e0.insurance_reservation_long && e1.insurance_reservation_short == e0.insurance_reservation_short);
    assert!(e1.insurance_domain_budget_long == e0.insurance_domain_budget_long && e1.insurance_domain_budget_short == e0.insurance_domain_budget_short);
    assert!(e1.pending_domain_loss_barrier_long == e0.pending_domain_loss_barrier_long && e1.pending_domain_loss_barrier_short == e0.pending_domain_loss_barrier_short);
    assert!(header.vault == h0.vault && header.insurance == h0.insurance && header.c_tot == h0.c_tot);
    // header frame: only risk_epoch and the backing aggregates may differ
    let mut hx = header;
    hx.risk_epoch = h0.risk_epoch;
    hx.backing_provider_earnings_total = h0.backing_provider_earnings_total;
    hx.source_fresh_backing_total_num = h0.source_fresh_backing_total_num;
    assert!(hx == h0, "header frame");

    // E-S10-10 covers (CASE-gated: `!(applicable) || cond`; see CaseApp)
    let fired = |s: usize| matches!(spec, Some((src, _, _)) if src == s);
    kani::cover!(!(app.l_to_s) || (fired(0)), "fires long -> short");
    kani::cover!(!(app.s_to_l) || (fired(1)), "fires short -> long");
    kani::cover!(!(app.any_fire) || (spec.map_or(false, |(src, _, _)| pp_of(&m0, src) > 0 && pre[src].fresh_unliened_backing_num > pp_of(&m0, src))), "fires with provider principal and loser cash present");
    kani::cover!(!(app.any_fresh) || (spec.is_none() && (0..2).any(|d| pre[d].fresh_unliened_backing_num > 0 && pre[d].fresh_unliened_backing_num <= pp_of(&m0, d))), "skipped by the provider share");
    kani::cover!(!(app.any_fire) || (budget0 == 0 && s10_spec(&m0, 1).is_some()), "skipped by the budget");
    kani::cover!(!(app.l_fresh) || (budget0 > 0 && spec.is_none() && {
            // a would-be move below the dust floor
            let bs = pre[0];
            let x = bs.fresh_unliened_backing_num.saturating_sub(pp_of(&m0, 0))
                .min(av(spre[0]).saturating_sub(spre[0].positive_claim_bound_num))
                .min(spre[1].positive_claim_bound_num.saturating_sub(av(spre[1])));
            bs.status == BackingBucketStatusV16::Fresh && bs.expiry_slot > NOW && x > 0 && x < S10_MIN_MOVE_ATOMS * BOUND_SCALE
        }), "skipped by the dust floor");
    kani::cover!(!(app.l_fresh) || (budget0 > 0 && pre[0].status == BackingBucketStatusV16::Fresh && pre[0].expiry_slot <= NOW
            && pre[0].fresh_unliened_backing_num > pp_of(&m0, 0)
            && spre[1].positive_claim_bound_num > av(spre[1]) + S10_MIN_MOVE_ATOMS * BOUND_SCALE), "skipped: source bucket lapsed (Fresh, expiry <= now)");
    kani::cover!(!(app.l_fresh && ss == 3) || (budget0 > 0 && pre[1].status == BackingBucketStatusV16::Impaired
            && pre[0].status == BackingBucketStatusV16::Fresh && pre[0].expiry_slot > NOW
            && pre[0].fresh_unliened_backing_num > pp_of(&m0, 0) + S10_MIN_MOVE_ATOMS * BOUND_SCALE
            && spre[1].positive_claim_bound_num > av(spre[1]) + S10_MIN_MOVE_ATOMS * BOUND_SCALE), "skipped: destination Impaired");
    kani::cover!(!(app.dst_empty) || (spec.map_or(false, |(_, dst, _)| pre[dst].status == BackingBucketStatusV16::Empty)), "destination Empty reopened");
    kani::cover!(!(app.dst_expired) || (spec.map_or(false, |(_, dst, _)| pre[dst].status == BackingBucketStatusV16::Expired)), "destination Expired reopened");
    kani::cover!(!(app.refill) || (spec.map_or(false, |(_, dst, _)| spre[dst].provider_receivable_num > 0)), "receivable refill > 0");
    kani::cover!(!(app.any_fire) || (spec.map_or(false, |(src, dst, x)| x < (pre[src].fresh_unliened_backing_num.saturating_sub(pp_of(&m0, src)))
            .min(av(spre[src]).saturating_sub(spre[src].positive_claim_bound_num))
            .min(spre[dst].positive_claim_bound_num.saturating_sub(av(spre[dst]))))), "rounding to whole atoms binds");
}
macro_rules! move_case {
    ($name:ident, $ins:expr, $sl:expr, $ss:expr) => {
        #[kani::proof]
        #[kani::stub(percolator::wide_math::div_rem_u256, div_rem_u256_native_only)]
        #[kani::solver(kissat)]
        fn $name() {
            s10_move_case_body($ins, $sl, $ss);
        }
    };
}
move_case!(proof_v22_s10_move_case_e_e, false, 0, 0);
move_case!(proof_v22_s10_move_case_e_f, false, 0, 1);
move_case!(proof_v22_s10_move_case_e_x, false, 0, 2);
move_case!(proof_v22_s10_move_case_e_i, false, 0, 3);
move_case!(proof_v22_s10_move_case_f_e, false, 1, 0);
move_case!(proof_v22_s10_move_case_f_f, false, 1, 1);
move_case!(proof_v22_s10_move_case_f_x, false, 1, 2);
move_case!(proof_v22_s10_move_case_f_i, false, 1, 3);
move_case!(proof_v22_s10_move_case_x_e, false, 2, 0);
move_case!(proof_v22_s10_move_case_x_f, false, 2, 1);
move_case!(proof_v22_s10_move_case_x_x, false, 2, 2);
move_case!(proof_v22_s10_move_case_x_i, false, 2, 3);
move_case!(proof_v22_s10_move_case_i_e, false, 3, 0);
move_case!(proof_v22_s10_move_case_i_f, false, 3, 1);
move_case!(proof_v22_s10_move_case_i_x, false, 3, 2);
move_case!(proof_v22_s10_move_case_i_i, false, 3, 3);
move_case!(proof_v22_s10_move_ins_case_e_e, true, 0, 0);
move_case!(proof_v22_s10_move_ins_case_e_f, true, 0, 1);
move_case!(proof_v22_s10_move_ins_case_e_x, true, 0, 2);
move_case!(proof_v22_s10_move_ins_case_e_i, true, 0, 3);
move_case!(proof_v22_s10_move_ins_case_f_e, true, 1, 0);
move_case!(proof_v22_s10_move_ins_case_f_f, true, 1, 1);
move_case!(proof_v22_s10_move_ins_case_f_x, true, 1, 2);
move_case!(proof_v22_s10_move_ins_case_f_i, true, 1, 3);
move_case!(proof_v22_s10_move_ins_case_x_e, true, 2, 0);
move_case!(proof_v22_s10_move_ins_case_x_f, true, 2, 1);
move_case!(proof_v22_s10_move_ins_case_x_x, true, 2, 2);
move_case!(proof_v22_s10_move_ins_case_x_i, true, 2, 3);
move_case!(proof_v22_s10_move_ins_case_i_e, true, 3, 0);
move_case!(proof_v22_s10_move_ins_case_i_f, true, 3, 1);
move_case!(proof_v22_s10_move_ins_case_i_x, true, 3, 2);
move_case!(proof_v22_s10_move_ins_case_i_i, true, 3, 3);

/// PROBE (diagnostic only): fixture cost with fully concrete domains (the grant fixture's).
#[kani::proof]
#[kani::stub(percolator::wide_math::div_rem_u256, div_rem_u256_native_only)]
#[kani::solver(kissat)]
fn probe_s10_world_conc() {
    let l = Dom { status: 1, fr: 50_000, valid: 0, cons: 0, imp: 0, exp: 200, claims: 0, rem: 0, exact_full: true, spent_extra: 0, pp: 0, ins: 0 };
    let s = Dom { status: 0, fr: 0, valid: 0, cons: 0, imp: 0, exp: 0, claims: 10_000, rem: 0, exact_full: true, spent_extra: 0, pp: 0, ins: 0 };
    let w = s10_world(l, s);
    kani::assume(w.is_some());
    kani::cover!(true, "fixture shape-valid");
}

/// PROBE (diagnostic only): fixture cost with concrete statuses (Fresh, Empty), rest symbolic.
#[kani::proof]
#[kani::stub(percolator::wide_math::div_rem_u256, div_rem_u256_native_only)]
#[kani::solver(kissat)]
fn probe_s10_world_case_f_e() {
    let mut l = any_dom(false);
    let mut s = any_dom(false);
    l.status = 1;
    s.status = 0;
    let w = s10_world(l, s);
    kani::assume(w.is_some());
    kani::cover!(true, "fixture shape-valid");
}

// =====================================================================================
// Row proof_v22_s10_idle_fast_path_is_identity (official: NO-VERDICT-OOM)
// =====================================================================================
// Original (E-S10-4): shape-valid S10 world with fresh <= principal in BOTH domains (in atoms),
// any budget <= 2 => Ok, header and markets byte-identical, budget unchanged.
// The move (v16.rs:15081) returns before any decode when the budget is 0, and otherwise checks
// exactly `fresh == 0 || fresh <= principal` on the RAW slot fields of both sides and returns
// Ok(()) when both hold. Neither exit reads anything else.
// Replacement: WEAKER precondition => STRONGER statement: the concrete one-asset fixture with
// both buckets fully symbolic (every field, every status), both principal mirrors symbolic u128,
// assume fresh_l <= pp_l and fresh_s <= pp_s (the original's l.fr <= l.pp, s.fr <= s.pp scaled
// by BOUND_SCALE is a special case), no shape assumption. Same assertions as the original.

#[kani::proof]
#[kani::solver(kissat)]
fn proof_v22_s10_idle_fast_path_any_state() {
    let (mut header, mut markets) = one_market_only_fixture();
    let market_id = markets[0].engine.asset.market_id.get();
    let bl = any_bucket(market_id);
    let bs = any_bucket(market_id);
    let pl: u128 = kani::any();
    let ps: u128 = kani::any();
    kani::assume(bl.fresh_unliened_backing_num <= pl && bs.fresh_unliened_backing_num <= ps);
    {
        let e = &mut markets[0].engine;
        e.backing_long = BackingBucketV16Account::from_runtime(&bl);
        e.backing_short = BackingBucketV16Account::from_runtime(&bs);
        e.provider_principal_long = V16PodU128::new(pl);
        e.provider_principal_short = V16PodU128::new(ps);
    }
    let budget0: u8 = kani::any();
    kani::assume(budget0 <= S10_MAX_MOVES_PER_INSTRUCTION);
    let mut budget = budget0;
    let (h0, m0) = (header, markets);
    let r = MarketGroupV16ViewMut::new(&mut header, &mut markets).rebalance_unclaimed_backing_for_test_not_atomic(0, &mut budget);
    assert!(r.is_ok());
    assert!(header == h0 && markets == m0);
    assert_eq!(budget, budget0);
    kani::cover!(bl.fresh_unliened_backing_num > 0 && budget0 > 0, "idle with fresh backing below the principal");
    kani::cover!(bl.fresh_unliened_backing_num == 0 && bs.fresh_unliened_backing_num == 0, "idle with both domains empty of fresh");
    kani::cover!(budget0 == 0, "budget 0");
    kani::cover!(budget0 == 2, "budget 2");
}
