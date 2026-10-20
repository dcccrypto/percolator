//! v22kani 2026-10-20, agent ENG: proof-only harnesses that need private engine items (stub targets or private
//! structs). cfg(kani) child module of v16; never in a program build. Production code is not changed: every
//! harness calls the real private function, and every stub is listed with its soundness argument.
#![allow(dead_code)]
use super::*;
use core::sync::atomic::{AtomicBool, Ordering};

// ------------------------------------------------------------------------------------------------------------
// #126 side flip (engine 17cdea8a). Real fn: `MarketGroupV16ViewMut::validate_trade_position_preflight`.
//
// Stubs (each returns a FREE boolean and records it; the harness asserts for EVERY value, so these over-
// approximate the real predicates and are sound for both the refusal and the success claims below):
//   asset_loss_stale_blocks_risk_increase, asset_has_target_effective_lag,
//   position_change_touches_pending_domain_loss_barrier.
// Everything else in the preflight is the production code: trade_signed_size_deltas, the classifier
// position_change_increases_risk, kernel_trade_preexisting_oi_reduction_gate on the fixture's asset OI,
// pending_domain_loss_barrier_blocks_position_change, trade_preflight_risk_gate,
// trade_account_requires_initial_margin, account_has_source_claims.
// ------------------------------------------------------------------------------------------------------------

static K126_STALE: AtomicBool = AtomicBool::new(false);
static K126_LAG: AtomicBool = AtomicBool::new(false);
static K126_BARRIER: AtomicBool = AtomicBool::new(false);

fn k126_stale_any<'a, T>(_s: &MarketGroupV16ViewMut<'a, T>, _asset_index: usize) -> V16Result<bool>
where
    T: 'a,
{
    let b: bool = kani::any();
    K126_STALE.store(b, Ordering::Relaxed);
    Ok(b)
}

fn k126_lag_any<'a, T>(_s: &MarketGroupV16ViewMut<'a, T>, _asset_index: usize) -> V16Result<bool>
where
    T: 'a,
{
    let b: bool = kani::any();
    K126_LAG.store(b, Ordering::Relaxed);
    Ok(b)
}

fn k126_barrier_any<'a, T>(
    _s: &MarketGroupV16ViewMut<'a, T>,
    _asset_index: usize,
    _current: i128,
    _next: i128,
) -> V16Result<bool>
where
    T: 'a,
{
    let b: bool = kani::any();
    if b {
        K126_BARRIER.store(true, Ordering::Relaxed);
    }
    Ok(b)
}

/// spec.md: risk-increasing = increases |effective position|, flips sign, or opens from flat.
fn spec_risk_increasing(c: i128, n: i128) -> bool {
    n != 0 && (c == 0 || (c > 0) != (n > 0) || n.unsigned_abs() > c.unsigned_abs())
}
/// f06a04a7 + #126: only a STRICT same-side reduction or a close skips the final IM gate.
fn spec_requires_im(c: i128, n: i128) -> bool {
    !(c != 0 && (n == 0 || (c > 0) == (n > 0)) && n.unsigned_abs() < c.unsigned_abs())
}
fn is_flip(c: i128, n: i128) -> bool {
    c != 0 && n != 0 && (c > 0) != (n > 0)
}
fn lookup(c: i128, n: i128) -> PositionDeltaLookupV16 {
    PositionDeltaLookupV16 {
        existing_slot: None,
        empty_slot: None,
        current_q: c,
        next_q: n,
        current_effective_q: c,
        next_effective_q: n,
    }
}

fn k126_market_fixture() -> (MarketGroupV16HeaderAccount, [Market<u64>; 1]) {
    let cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    {
        let mut view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        view.activate_empty_market_not_atomic(0, 100, 1).unwrap();
    }
    // Pre-existing OI on both sides at the protocol cap, so the pre-existing-OI reduction gate (a separate,
    // unchanged check) admits every reduction the harness generates (|positions| <= crate::MAX_OI_SIDE_Q / 2 each).
    markets[0].engine.asset.oi_eff_long_q = V16PodU128::new(crate::MAX_OI_SIDE_Q);
    markets[0].engine.asset.oi_eff_short_q = V16PodU128::new(crate::MAX_OI_SIDE_Q);
    (header, markets)
}

fn k126_account(tag: u8) -> PortfolioAccountV16Account {
    let mut account_id = [0u8; 32];
    account_id[0] = tag;
    let mut owner = [0u8; 32];
    owner[0] = tag;
    PortfolioAccountV16Account::try_empty(ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        [1; 32], account_id, owner,
    )))
    .unwrap()
}

/// ENG-126-P: the trade preflight classifies a side flip exactly like an open of the new side.
///  (a) Ok => `risk_increasing == spec(long) || spec(short)` and each `*_requires_initial_margin == spec_im`
///      (spec.md: abs increase, sign flip, or open from flat; IM skipped only for strict same-side reductions).
///  (b) a trade with any risk-increasing side (in particular ANY flip, equal or smaller size) is REFUSED when the
///      asset is loss-stale (LossStale) or target-lagged (LockActive), for every value of the other predicates.
///  (c) a pure reduce/close on both sides is NOT blocked by loss-stale or target-lag (no new over-blocking):
///      with no pending-domain barrier it is Ok whatever the stale/lag predicates say.
///  (d) for a flip c -> n on the long account, the production flags equal those of the open-from-flat 0 -> n
///      (same production function, second call): "a flip passes the same gates as an open of the new side".
#[kani::proof]
#[kani::unwind(70)]
#[kani::stub(super::MarketGroupV16ViewMut::asset_loss_stale_blocks_risk_increase, k126_stale_any)]
#[kani::stub(super::MarketGroupV16ViewMut::asset_has_target_effective_lag, k126_lag_any)]
#[kani::stub(super::MarketGroupV16ViewMut::position_change_touches_pending_domain_loss_barrier, k126_barrier_any)]
fn kani_eng1020_126_preflight_flip_is_risk_increasing_and_reduce_not_blocked() {
    let (mut header, mut markets) = k126_market_fixture();
    let long_acct = k126_account(2);
    let short_acct = k126_account(3);
    let view = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    let half_cap = (crate::MAX_OI_SIDE_Q / 2) as i128;
    let lc: i128 = kani::any();
    let sc: i128 = kani::any();
    let size: i128 = kani::any();
    kani::assume(lc >= -half_cap && lc <= half_cap);
    kani::assume(sc >= -half_cap && sc <= half_cap);
    kani::assume(size != 0 && size >= -half_cap && size <= half_cap);
    let ln = lc + size; // |.| <= crate::MAX_OI_SIDE_Q: no overflow
    let sn = sc - size;

    let request = TradeRequestV16 { asset_index: 0, size_q: size, exec_price: 100, fee_bps: 0 };
    K126_BARRIER.store(false, Ordering::Relaxed);
    let r = view.validate_trade_position_preflight(
        &PortfolioV16View::new(&long_acct),
        &PortfolioV16View::new(&short_acct),
        request,
        lookup(lc, ln),
        lookup(sc, sn),
    );
    let stale = K126_STALE.load(Ordering::Relaxed);
    let lag = K126_LAG.load(Ordering::Relaxed);
    let barrier = K126_BARRIER.load(Ordering::Relaxed);
    let ri = spec_risk_increasing(lc, ln) || spec_risk_increasing(sc, sn);

    // (b) refusal of every risk-increasing trade (flips included) under either gate
    if ri && (stale || lag) {
        assert!(r.is_err());
        if !barrier && !lag {
            assert!(matches!(r, Err(V16Error::LossStale)));
        }
    }
    // (c) pure reduce / close on both sides: stale and lag never block it
    if !ri && !barrier {
        assert!(r.is_ok());
    }
    // completeness with no barrier touched: Ok exactly when no gate applies to a risk-increasing trade
    if !barrier {
        assert_eq!(r.is_ok(), !(ri && (stale || lag)));
    }
    match r {
        Ok(out) => {
            // (a) exact classification
            assert_eq!(out.risk_increasing, ri);
            assert_eq!(out.long_requires_initial_margin, spec_requires_im(lc, ln));
            assert_eq!(out.short_requires_initial_margin, spec_requires_im(sc, sn));
            assert!(!out.long_has_source_claims && !out.short_has_source_claims);
            if is_flip(lc, ln) {
                assert!(out.risk_increasing && out.long_requires_initial_margin);
                // (d) same flags as the open-from-flat of the new side, from the production fn itself
                K126_BARRIER.store(false, Ordering::Relaxed);
                let open = view.validate_trade_position_preflight(
                    &PortfolioV16View::new(&long_acct),
                    &PortfolioV16View::new(&short_acct),
                    TradeRequestV16 { asset_index: 0, size_q: ln, exec_price: 100, fee_bps: 0 },
                    lookup(0, ln),
                    lookup(0, -ln),
                );
                if let Ok(o) = open {
                    assert_eq!(o.risk_increasing, out.risk_increasing);
                    assert_eq!(o.long_requires_initial_margin, out.long_requires_initial_margin);
                    kani::cover!(ln.unsigned_abs() < lc.unsigned_abs(), "flip to smaller size has open-from-flat flags");
                }
            }
            kani::cover!(is_flip(lc, ln) && ln.unsigned_abs() < lc.unsigned_abs(), "smaller flip admitted (no gate) with IM required");
            kani::cover!(!ri && stale && lag && lc != 0 && sc != 0, "pure reduce admitted while loss-stale AND target-lagged");
            kani::cover!(!ri && stale && ln == 0 && lc != 0, "close to flat admitted while loss-stale");
        }
        Err(e) => {
            kani::cover!(
                e == V16Error::LossStale && ln == -lc && lc != 0 && !spec_risk_increasing(sc, sn),
                "equal-size flip refused LossStale (the #126 case), other side not risk-increasing"
            );
            kani::cover!(
                e == V16Error::LockActive && lag && !stale && is_flip(lc, ln) && ln.unsigned_abs() < lc.unsigned_abs(),
                "smaller flip refused under target lag"
            );
            kani::cover!(
                is_flip(sc, sn) && sn.unsigned_abs() < sc.unsigned_abs() && !spec_risk_increasing(lc, ln) && stale,
                "short-side smaller flip refused while stale (other side a pure reduce)"
            );
        }
    }
}

// ------------------------------------------------------------------------------------------------------------
// Production-faithful (PF) stubs: the two kani/test/audit-scan-only validators that the deployed SBF build does
// not compile (percolator-prog enables `audit-scan` only for non-solana targets). Replacing them with Ok(())
// gives exactly the deployed program's checks. Public so tests/ harnesses can name them as stub replacements.
// ------------------------------------------------------------------------------------------------------------
pub fn pf_full_audit_scan_absent<'a, T>(_s: &MarketGroupV16View<'a, T>) -> V16Result<()>
where
    T: 'a,
{
    Ok(())
}
pub fn pf_slots_shape_absent<S: MarketSlotV16View>(_h: &MarketGroupV16HeaderAccount, _slots: &[S]) -> V16Result<()> {
    Ok(())
}

// ------------------------------------------------------------------------------------------------------------
// #126 end to end: the real public trade `execute_trade_with_fee_loss_stale_scoped_not_atomic` on a concrete
// one-asset world (the configuration of tests/v22_s10_cap.rs plus a 1% fee cap) after A opened long 4 units
// against B; then B buys k units from A (k symbolic in 1..=8): k<4 reduce, k=4 close, 5..=7 smaller flip,
// 8 equal-size flip. Only the PF stubs above (deployed-build semantics). Native twin: tests/eng1020_flip_native.rs.
// ------------------------------------------------------------------------------------------------------------
const K126_PRICE: u64 = 1_000_000;

fn k126_world_account(seed: u32) -> PortfolioAccountV16Account {
    let mut key = [0u8; 32];
    key[..4].copy_from_slice(&seed.to_le_bytes());
    key[31] = 0x5A;
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new([1; 32], key, [3; 32]));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(header).unwrap();
    a
}

/// (A, B, header, markets) after A long 4 units vs B short 4 units, both with `deposit` capital; if `a_capital`
/// is set, A's capital is lowered to it with c_tot and vault lowered by the same delta (A realised a loss that
/// left the pool), keeping `capital <= c_tot`, `c_tot + insurance <= vault`.
fn k126_opened(
    deposit: u128,
    a_capital: Option<u128>,
) -> (PortfolioAccountV16Account, PortfolioAccountV16Account, MarketGroupV16HeaderAccount, [Market<u64>; 1]) {
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 6_480_000);
    cfg.max_abs_funding_e9_per_slot = 10_000;
    cfg.max_price_move_bps_per_slot = 200;
    cfg.initial_margin_bps = 1_000;
    cfg.maintenance_margin_bps = 500;
    cfg.max_accrual_dt_slots = 2;
    cfg.max_trading_fee_bps = 100;
    cfg.min_funding_lifetime_slots = 10_000_000;
    let mut h = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut m = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    h.activate_empty_asset_slot_not_atomic(0, &mut m[0].engine, K126_PRICE, 1).unwrap();
    let mut a = k126_world_account(1);
    let mut b = k126_world_account(2);
    MarketGroupV16ViewMut::new(&mut h, &mut m).deposit_not_atomic(&mut PortfolioV16ViewMut::new(&mut a), deposit).unwrap();
    MarketGroupV16ViewMut::new(&mut h, &mut m).deposit_not_atomic(&mut PortfolioV16ViewMut::new(&mut b), deposit).unwrap();
    MarketGroupV16ViewMut::new(&mut h, &mut m)
        .execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut PortfolioV16ViewMut::new(&mut a),
            &mut PortfolioV16ViewMut::new(&mut b),
            TradeRequestV16 { asset_index: 0, size_q: (4 * POS_SCALE) as i128, exec_price: K126_PRICE, fee_bps: 0 },
            true,
        )
        .unwrap();
    if let Some(c) = a_capital {
        let d = a.capital.get() - c;
        a.capital = V16PodU128::new(c);
        h.c_tot = V16PodU128::new(h.c_tot.get() - d);
        h.vault = V16PodU128::new(h.vault.get() - d);
    }
    (a, b, h, m)
}

/// ENG-126-V: a flip (and every other fill) through the public trade is value-neutral: the vault is unchanged,
/// the only value that moves is the taker fee from capital to insurance (capital sum and c_tot down by exactly the
/// fee, insurance up by exactly the fee), PnL is not created on either side, and OI stays balanced at |4-k| units.
/// Every k in 1..=8 succeeds for a well-capitalised pair (liveness: sound because PF stubs keep deployed checks).
#[kani::proof]
#[kani::unwind(70)]
#[kani::stub(super::MarketGroupV16View::validate_shape_full_audit_scan, pf_full_audit_scan_absent)]
#[kani::stub(super::MarketGroupV16HeaderAccount::validate_dynamic_market_slots_shape, pf_slots_shape_absent)]
fn kani_eng1020_126_flip_is_value_neutral_end_to_end() {
    let (mut a, mut b, mut h, mut m) = k126_opened(1_000_000_000_000, None);
    let k: u128 = kani::any();
    let fee_bps: u64 = kani::any();
    kani::assume((1..=8).contains(&k));
    kani::assume(fee_bps <= 100);
    let cap_sum = a.capital.get() + b.capital.get();
    let (c_tot, vault, ins) = (h.c_tot.get(), h.vault.get(), h.insurance.get());
    let (pnl_a, pnl_b) = (a.pnl.get(), b.pnl.get());
    let r = MarketGroupV16ViewMut::new(&mut h, &mut m).execute_trade_with_fee_loss_stale_scoped_not_atomic(
        &mut PortfolioV16ViewMut::new(&mut b),
        &mut PortfolioV16ViewMut::new(&mut a),
        TradeRequestV16 { asset_index: 0, size_q: (k * POS_SCALE) as i128, exec_price: K126_PRICE, fee_bps },
        true,
    );
    assert!(r.is_ok());
    let out = r.unwrap();
    let fee = out.fee_a + out.fee_b;
    assert_eq!(h.vault.get(), vault);
    assert_eq!(a.capital.get() + b.capital.get() + fee, cap_sum);
    assert_eq!(h.c_tot.get() + fee, c_tot);
    assert_eq!(h.insurance.get(), ins + fee);
    assert_eq!(a.pnl.get(), pnl_a);
    assert_eq!(b.pnl.get(), pnl_b);
    let open_units = if k >= 4 { k - 4 } else { 4 - k };
    assert_eq!(m[0].engine.asset.oi_eff_long_q.get(), open_units * POS_SCALE);
    assert_eq!(m[0].engine.asset.oi_eff_short_q.get(), open_units * POS_SCALE);
    assert!(h.c_tot.get() + h.insurance.get() <= h.vault.get());
    kani::cover!(k == 8 && fee > 0, "equal-size flip with a fee: value-neutral");
    kani::cover!(k == 6, "smaller flip: value-neutral");
    kani::cover!(k == 4, "close to flat: value-neutral");
    kani::cover!(k == 2, "pure reduce: value-neutral");
}

/// ENG-126-IM: A is long 4 units with capital 210_000 (above maintenance 200_000, below initial 400_000 at 10% IM).
/// B buys k from A. A pure reduce (k<4) and the close (k=4) are NOT blocked (f06a04a7 kept); a flip whose new
/// side fits A's initial margin (k=5,6: |new| <= 2 units) passes; a flip whose new side does not (k=7: smaller
/// than the old position, the #126 case; k=8: equal size) is refused with InsufficientInitialMargin, exactly like
/// an open of that size. Native twin: `eng1020_flip_outcomes_below_im` (cap=210000 row).
#[kani::proof]
#[kani::unwind(70)]
#[kani::stub(super::MarketGroupV16View::validate_shape_full_audit_scan, pf_full_audit_scan_absent)]
#[kani::stub(super::MarketGroupV16HeaderAccount::validate_dynamic_market_slots_shape, pf_slots_shape_absent)]
fn kani_eng1020_126_flip_below_im_refused_reduce_not() {
    let (mut a, mut b, mut h, mut m) = k126_opened(1_000_000_000_000, Some(210_000));
    let k: u128 = kani::any();
    kani::assume((1..=8).contains(&k));
    let r = MarketGroupV16ViewMut::new(&mut h, &mut m).execute_trade_with_fee_loss_stale_scoped_not_atomic(
        &mut PortfolioV16ViewMut::new(&mut b),
        &mut PortfolioV16ViewMut::new(&mut a),
        TradeRequestV16 { asset_index: 0, size_q: (k * POS_SCALE) as i128, exec_price: K126_PRICE, fee_bps: 0 },
        true,
    );
    if k <= 6 {
        assert!(r.is_ok());
    } else {
        assert!(matches!(r, Err(V16Error::InsufficientInitialMargin)));
    }
    kani::cover!(k == 3 && r.is_ok(), "reduce below IM admitted");
    kani::cover!(k == 4 && r.is_ok(), "close below IM admitted");
    kani::cover!(k == 6 && r.is_ok(), "flip within IM admitted");
    kani::cover!(k == 7 && r.is_err(), "smaller flip beyond IM refused (#126)");
    kani::cover!(k == 8 && r.is_err(), "equal flip beyond IM refused");
}

// ------------------------------------------------------------------------------------------------------------
// G10 / K-E6 (#450): forfeit loss backing never exceeds what the domain's claimants own.
// Split (the earlier whole designs had no verdict): G10-L the floor-division lemma for BOUND_SCALE at full u128;
// G10-K the REAL `forfeit_loss_backing_capacity_atoms` over ANY state the reader can return (reader stubbed with a
// fully unconstrained state: an over-approximation of the real reader, sound for this Ok-path claim);
// G10-S the production setter -> production reader round trip that justifies reading "the domain's state".
// ------------------------------------------------------------------------------------------------------------
use core::sync::atomic::AtomicU64;

struct G10Cell {
    hi: AtomicU64,
    lo: AtomicU64,
}
impl G10Cell {
    const fn new() -> Self {
        Self { hi: AtomicU64::new(0), lo: AtomicU64::new(0) }
    }
    fn put(&self, v: u128) {
        self.hi.store((v >> 64) as u64, Ordering::Relaxed);
        self.lo.store(v as u64, Ordering::Relaxed);
    }
    fn get(&self) -> u128 {
        ((self.hi.load(Ordering::Relaxed) as u128) << 64) | self.lo.load(Ordering::Relaxed) as u128
    }
}
static G10_BOUND: G10Cell = G10Cell::new();
static G10_RECV: G10Cell = G10Cell::new();
static G10_FRESH: G10Cell = G10Cell::new();
static G10_VLB: G10Cell = G10Cell::new();
static G10_INSRES: G10Cell = G10Cell::new();
static G10_VLI: G10Cell = G10Cell::new();
static G10_ILI: G10Cell = G10Cell::new();

fn g10_reader_any<'a, T>(_s: &MarketGroupV16ViewMut<'a, T>, _domain: usize) -> V16Result<SourceCreditStateV16>
where
    T: 'a,
{
    let st = SourceCreditStateV16 {
        positive_claim_bound_num: kani::any(),
        exact_positive_claim_num: kani::any(),
        fresh_reserved_backing_num: kani::any(),
        spent_backing_num: kani::any(),
        provider_receivable_num: kani::any(),
        valid_liened_backing_num: kani::any(),
        impaired_liened_backing_num: kani::any(),
        insurance_credit_reserved_num: kani::any(),
        valid_liened_insurance_num: kani::any(),
        impaired_liened_insurance_num: kani::any(),
        credit_rate_num: kani::any(),
        credit_epoch: kani::any(),
    };
    G10_BOUND.put(st.positive_claim_bound_num);
    G10_RECV.put(st.provider_receivable_num);
    G10_FRESH.put(st.fresh_reserved_backing_num);
    G10_VLB.put(st.valid_liened_backing_num);
    G10_INSRES.put(st.insurance_credit_reserved_num);
    G10_VLI.put(st.valid_liened_insurance_num);
    G10_ILI.put(st.impaired_liened_insurance_num);
    Ok(st)
}

/// G10-L: floor division by BOUND_SCALE at full u128 width: q*BS <= x < q*BS + BS (no overflow in q*BS).
/// The rounding fact the capacity bound rests on; stated on the Rust `/` the production line uses.
#[kani::proof]
#[kani::solver(kissat)]
fn kani_eng1020_g10_l_floor_div_bound_scale_kissat() {
    let x: u128 = kani::any();
    let q = x / BOUND_SCALE;
    let p = q.checked_mul(BOUND_SCALE);
    assert!(p.is_some());
    let p = p.unwrap();
    assert!(p <= x);
    assert!(x - p < BOUND_SCALE);
    kani::cover!(q > 0 && x - p > 0, "non-zero quotient with a remainder");
    kani::cover!(x > u64::MAX as u128 && x - p == 0, "wide exact multiple");
}

/// G10-L with z3 (same body; solver diagnostic pair, each run once).
#[kani::proof]
#[kani::solver(z3)]
fn kani_eng1020_g10_l_floor_div_bound_scale_z3() {
    let x: u128 = kani::any();
    let q = x / BOUND_SCALE;
    let p = q.checked_mul(BOUND_SCALE);
    assert!(p.is_some());
    let p = p.unwrap();
    assert!(p <= x);
    assert!(x - p < BOUND_SCALE);
    kani::cover!(q > 0 && x - p > 0, "non-zero quotient with a remainder");
    kani::cover!(x > u64::MAX as u128 && x - p == 0, "wide exact multiple");
}

/// G10-K: for ANY source-credit state, a successful `forfeit_loss_backing_capacity_atoms` returns cap with
///   cap*BS <= gap + recv  (never more than the unbacked claim bound plus the receivable already advanced),
///   gap + recv - cap*BS < BS  (exact floor: nothing the claimants own is withheld beyond rounding),
///   cap*BS <= bound + recv  (whenever that sum is representable),
/// where gap = bound - min(bound, available), available = (fresh - valid_liened) + (ins_reserved - liened_ins).
/// Mutant EM6 (capacity rounds up) violates the first line whenever (gap+recv) % BS != 0.
#[kani::proof]
#[kani::solver(z3)]
#[kani::stub(super::MarketGroupV16ViewMut::source_credit_for_domain, g10_reader_any)]
fn kani_eng1020_g10_k_forfeit_capacity_le_claimant_ownership() {
    let cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    let v = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let domain: usize = kani::any();
    match v.forfeit_loss_backing_capacity_atoms(domain) {
        Ok(cap) => {
            let (bound, recv) = (G10_BOUND.get(), G10_RECV.get());
            let (fresh, vlb, insres, vli, ili) =
                (G10_FRESH.get(), G10_VLB.get(), G10_INSRES.get(), G10_VLI.get(), G10_ILI.get());
            // Ok implies the production guards held: fresh >= vlb, vli+ili fits and <= insres, sum fits
            assert!(fresh >= vlb);
            let enc = vli.checked_add(ili);
            assert!(enc.is_some_and(|e| e <= insres));
            let avail = (fresh - vlb).checked_add(insres - enc.unwrap());
            assert!(avail.is_some());
            let gap = bound.saturating_sub(avail.unwrap());
            let owned = gap.checked_add(recv);
            assert!(owned.is_some());
            let owned = owned.unwrap();
            let p = cap.checked_mul(BOUND_SCALE);
            assert!(p.is_some_and(|p| p <= owned));
            assert!(owned - p.unwrap() < BOUND_SCALE);
            if let Some(ub) = bound.checked_add(recv) {
                assert!(p.unwrap() <= ub);
            }
            kani::cover!(cap > 0 && gap > 0 && recv > 0, "positive capacity from both a support gap and a receivable");
            kani::cover!(cap == 0 && bound > 0 && recv == 0, "claims fully backed and no receivable: zero capacity");
            kani::cover!(owned % BOUND_SCALE != 0 && cap > 0, "rounding exercised (owned not a multiple of BS)");
            kani::cover!(avail.unwrap() > bound && recv >= BOUND_SCALE, "over-backed domain: capacity is the receivable only");
        }
        Err(_) => {
            kani::cover!(true, "refused (invalid shape or overflow)");
        }
    }
}

static G10S_VALIDATE_OK: AtomicBool = AtomicBool::new(true);
fn g10s_validate_any(_state: SourceCreditStateV16) -> V16Result<()> {
    let ok: bool = kani::any();
    G10S_VALIDATE_OK.store(ok, Ordering::Relaxed);
    if ok {
        Ok(())
    } else {
        Err(V16Error::InvalidConfig)
    }
}

/// G10-S: the production setter `set_source_credit_for_domain(d, st)` followed by the production reader
/// `source_credit_for_domain(d)` returns exactly `st` (when both succeed), and leaves the other side's state of the
/// same asset unchanged. The reader's static validator is replaced by a free Ok/Err (it only accepts or refuses,
/// never transforms; over-approximation, sound for the Ok-path equality). With G10-K this gives: the capacity is
/// computed from the very state the engine stored for that domain.
#[kani::proof]
#[kani::unwind(70)]
#[kani::stub(super::V16Core::validate_source_credit_state_static, g10s_validate_any)]
fn kani_eng1020_g10_s_setter_reader_round_trip() {
    // no asset activation (the activation fixture's band anchor loop dominated other runs); the setter and the
    // reader read only the config (domain count), the slot and the header aggregates.
    let cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    let mut v = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let d: usize = kani::any();
    kani::assume(d < 2);
    let other = 1 - d;
    let before_other = v.source_credit_for_domain_shape(other).unwrap();
    let st = SourceCreditStateV16 {
        positive_claim_bound_num: kani::any(),
        exact_positive_claim_num: kani::any(),
        fresh_reserved_backing_num: kani::any(),
        spent_backing_num: kani::any(),
        provider_receivable_num: kani::any(),
        valid_liened_backing_num: kani::any(),
        impaired_liened_backing_num: kani::any(),
        insurance_credit_reserved_num: kani::any(),
        valid_liened_insurance_num: kani::any(),
        impaired_liened_insurance_num: kani::any(),
        credit_rate_num: kani::any(),
        credit_epoch: kani::any(),
    };
    if v.set_source_credit_for_domain(d, st).is_ok() {
        assert_eq!(v.source_credit_for_domain_shape(other).unwrap(), before_other);
        match v.source_credit_for_domain(d) {
            Ok(back) => {
                assert_eq!(back, st);
                kani::cover!(d == 0 && st.positive_claim_bound_num > 0, "long domain round trip");
                kani::cover!(d == 1 && st.provider_receivable_num > 0, "short domain round trip");
            }
            Err(_) => {
                assert!(!G10S_VALIDATE_OK.load(Ordering::Relaxed));
                kani::cover!(true, "reader refuses a state its validator rejects");
            }
        }
    } else {
        kani::cover!(true, "setter refuses (aggregate total overflow)");
    }
}

// ------------------------------------------------------------------------------------------------------------
// ENG-126-W: wiring of the final trade checks. Real fn `finish_trade_checks_not_atomic`; the lien creator and the
// two IM gates are replaced by recorders (the lien creator returns Ok; each gate returns a FREE Ok/Err), so the
// claim is about WHICH checks run for WHICH account, for every flag combination. With ENG-126-P (a flip yields
// risk_increasing = true and *_requires_initial_margin = true, the open-from-flat flags), this gives: a flip creates
// the source lien (when the account has source claims and the market is not h-locked) and passes the IM gate,
// exactly like an open; a strict reduction (requires_initial_margin = false) creates no lien and skips the gate.
// ------------------------------------------------------------------------------------------------------------
static W_LIEN_L: AtomicBool = AtomicBool::new(false);
static W_LIEN_S: AtomicBool = AtomicBool::new(false);
static W_IM_L: AtomicBool = AtomicBool::new(false);
static W_IM_S: AtomicBool = AtomicBool::new(false);
static W_NPC_L: AtomicBool = AtomicBool::new(false);
static W_NPC_S: AtomicBool = AtomicBool::new(false);
static W_FAILED: AtomicBool = AtomicBool::new(false);

fn w_is_long(id0: u8) -> bool {
    id0 == 2
}
fn w_lien_rec<'a, T>(_s: &mut MarketGroupV16ViewMut<'a, T>, account: &mut PortfolioV16ViewMut<'_>) -> V16Result<()>
where
    T: 'a,
{
    if w_is_long(account.header.provenance_header.portfolio_account_id[0]) {
        W_LIEN_L.store(true, Ordering::Relaxed);
    } else {
        W_LIEN_S.store(true, Ordering::Relaxed);
    }
    Ok(())
}
fn w_gate(account: &PortfolioV16View<'_>, l: &AtomicBool, s: &AtomicBool) -> V16Result<()> {
    if w_is_long(account.header.provenance_header.portfolio_account_id[0]) {
        l.store(true, Ordering::Relaxed);
    } else {
        s.store(true, Ordering::Relaxed);
    }
    let ok: bool = kani::any();
    if ok {
        Ok(())
    } else {
        W_FAILED.store(true, Ordering::Relaxed);
        Err(V16Error::InsufficientInitialMargin)
    }
}
fn w_im_rec<'a, T>(account: &PortfolioV16View<'_>) -> V16Result<()>
where
    T: 'a,
{
    w_gate(account, &W_IM_L, &W_IM_S)
}
fn w_npc_rec<'a, T>(account: &PortfolioV16View<'_>) -> V16Result<()>
where
    T: 'a,
{
    w_gate(account, &W_NPC_L, &W_NPC_S)
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::stub(super::MarketGroupV16ViewMut::create_initial_margin_source_lien_if_needed, w_lien_rec)]
#[kani::stub(super::MarketGroupV16ViewMut::ensure_initial_margin, w_im_rec)]
#[kani::stub(super::MarketGroupV16ViewMut::ensure_no_positive_credit_initial_margin, w_npc_rec)]
fn kani_eng1020_126_finish_checks_lien_and_im_wiring() {
    // no asset activation: finish_trade_checks reads no asset state (first run NV at 900 s spent in the
    // activation fixture's band anchor loop)
    let cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    let mut long_acct = k126_account(2);
    let mut short_acct = k126_account(3);
    let mut v = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let (locked, ri, lreq, sreq, lhsc, shsc): (bool, bool, bool, bool, bool, bool) =
        (kani::any(), kani::any(), kani::any(), kani::any(), kani::any(), kani::any());
    let r = v.finish_trade_checks_not_atomic(
        &mut PortfolioV16ViewMut::new(&mut long_acct),
        &mut PortfolioV16ViewMut::new(&mut short_acct),
        locked,
        ri,
        lreq,
        sreq,
        lhsc,
        shsc,
    );
    let ld = |a: &AtomicBool| a.load(Ordering::Relaxed);
    if !ld(&W_FAILED) {
        // every gate passed: the call completes (the PF-free audit-scan helpers are Ok without the feature)
        assert!(r.is_ok());
        assert_eq!(ld(&W_LIEN_L), ri && !locked && lreq && lhsc);
        assert_eq!(ld(&W_LIEN_S), ri && !locked && sreq && shsc);
        assert_eq!(ld(&W_IM_L), lreq);
        assert_eq!(ld(&W_IM_S), sreq);
        assert_eq!(ld(&W_NPC_L), locked && lreq);
        assert_eq!(ld(&W_NPC_S), locked && sreq);
        kani::cover!(ri && !locked && lreq && lhsc && ld(&W_IM_L), "flip/open with claims: lien created and IM checked");
        kani::cover!(!lreq && !ld(&W_IM_L) && !ld(&W_LIEN_L) && lhsc && ri, "strict reduce: no lien, no IM gate");
        kani::cover!(locked && lreq && ld(&W_NPC_L), "locked market: no-positive-credit IM gate");
    } else {
        // a failed gate refuses the trade
        assert!(r.is_err());
        kani::cover!(ld(&W_IM_L) && lreq, "IM failure refuses");
    }
    // a required IM gate is never skipped: if lreq, the long gate ran (or an earlier gate already refused)
    if lreq && r.is_ok() {
        assert!(ld(&W_IM_L));
    }
}

// ------------------------------------------------------------------------------------------------------------
// ENG-126-V2 / ENG-126-IM2: ENG-126-V / -IM with the opened world loaded from a byte snapshot of the PRODUCTION
// fixture (tests/eng1020_flip_native.rs::eng1020_opened_snapshot_is_current keeps it equal to `opened(1e12, None)`),
// so Kani executes only the flip trade itself. First design NV at 2400 s inside the fixture + trade symex.
// ------------------------------------------------------------------------------------------------------------
mod opened_snap {
    include!("../tests/fixtures/eng1020_opened.rs");
}

fn k126_opened_snap(
    a_capital: Option<u128>,
) -> (PortfolioAccountV16Account, PortfolioAccountV16Account, MarketGroupV16HeaderAccount, [Market<u64>; 1]) {
    // byte copy into zeroed Pod values (bytemuck::pod_read_unaligned's error branch was reported reachable by Kani)
    let mut h: MarketGroupV16HeaderAccount = bytemuck::Zeroable::zeroed();
    bytemuck::bytes_of_mut(&mut h).copy_from_slice(&opened_snap::OPEN_HEADER);
    let mut e: EngineAssetSlotV16Account = bytemuck::Zeroable::zeroed();
    bytemuck::bytes_of_mut(&mut e).copy_from_slice(&opened_snap::OPEN_SLOT);
    let mut a: PortfolioAccountV16Account = bytemuck::Zeroable::zeroed();
    bytemuck::bytes_of_mut(&mut a).copy_from_slice(&opened_snap::OPEN_A);
    let mut b: PortfolioAccountV16Account = bytemuck::Zeroable::zeroed();
    bytemuck::bytes_of_mut(&mut b).copy_from_slice(&opened_snap::OPEN_B);
    if let Some(c) = a_capital {
        let d = a.capital.get() - c;
        a.capital = V16PodU128::new(c);
        h.c_tot = V16PodU128::new(h.c_tot.get() - d);
        h.vault = V16PodU128::new(h.vault.get() - d);
    }
    (a, b, h, [Market::new(0u64, e)])
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::stub(super::MarketGroupV16View::validate_shape_full_audit_scan, pf_full_audit_scan_absent)]
#[kani::stub(super::MarketGroupV16HeaderAccount::validate_dynamic_market_slots_shape, pf_slots_shape_absent)]
fn kani_eng1020_126_flip_is_value_neutral_end_to_end_snap() {
    let (mut a, mut b, mut h, mut m) = k126_opened_snap(None);
    let k: u128 = kani::any();
    let fee_bps: u64 = kani::any();
    kani::assume((1..=8).contains(&k));
    kani::assume(fee_bps <= 100);
    let cap_sum = a.capital.get() + b.capital.get();
    let (c_tot, vault, ins) = (h.c_tot.get(), h.vault.get(), h.insurance.get());
    let (pnl_a, pnl_b) = (a.pnl.get(), b.pnl.get());
    let r = MarketGroupV16ViewMut::new(&mut h, &mut m).execute_trade_with_fee_loss_stale_scoped_not_atomic(
        &mut PortfolioV16ViewMut::new(&mut b),
        &mut PortfolioV16ViewMut::new(&mut a),
        TradeRequestV16 { asset_index: 0, size_q: (k * POS_SCALE) as i128, exec_price: K126_PRICE, fee_bps },
        true,
    );
    assert!(r.is_ok());
    let out = r.unwrap();
    let fee = out.fee_a + out.fee_b;
    assert_eq!(h.vault.get(), vault);
    assert_eq!(a.capital.get() + b.capital.get() + fee, cap_sum);
    assert_eq!(h.c_tot.get() + fee, c_tot);
    assert_eq!(h.insurance.get(), ins + fee);
    assert_eq!(a.pnl.get(), pnl_a);
    assert_eq!(b.pnl.get(), pnl_b);
    let open_units = if k >= 4 { k - 4 } else { 4 - k };
    assert_eq!(m[0].engine.asset.oi_eff_long_q.get(), open_units * POS_SCALE);
    assert_eq!(m[0].engine.asset.oi_eff_short_q.get(), open_units * POS_SCALE);
    assert!(h.c_tot.get() + h.insurance.get() <= h.vault.get());
    kani::cover!(k == 8 && fee > 0, "equal-size flip with a fee: value-neutral");
    kani::cover!(k == 6, "smaller flip: value-neutral");
    kani::cover!(k == 4, "close to flat: value-neutral");
    kani::cover!(k == 2, "pure reduce: value-neutral");
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::stub(super::MarketGroupV16View::validate_shape_full_audit_scan, pf_full_audit_scan_absent)]
#[kani::stub(super::MarketGroupV16HeaderAccount::validate_dynamic_market_slots_shape, pf_slots_shape_absent)]
fn kani_eng1020_126_flip_below_im_refused_reduce_not_snap() {
    let (mut a, mut b, mut h, mut m) = k126_opened_snap(Some(210_000));
    let k: u128 = kani::any();
    kani::assume((1..=8).contains(&k));
    let r = MarketGroupV16ViewMut::new(&mut h, &mut m).execute_trade_with_fee_loss_stale_scoped_not_atomic(
        &mut PortfolioV16ViewMut::new(&mut b),
        &mut PortfolioV16ViewMut::new(&mut a),
        TradeRequestV16 { asset_index: 0, size_q: (k * POS_SCALE) as i128, exec_price: K126_PRICE, fee_bps: 0 },
        true,
    );
    if k <= 6 {
        assert!(r.is_ok());
    } else {
        assert!(matches!(r, Err(V16Error::InsufficientInitialMargin)));
    }
    kani::cover!(k == 3 && r.is_ok(), "reduce below IM admitted");
    kani::cover!(k == 4 && r.is_ok(), "close below IM admitted");
    kani::cover!(k == 6 && r.is_ok(), "flip within IM admitted");
    kani::cover!(k == 7 && r.is_err(), "smaller flip beyond IM refused (#126)");
    kani::cover!(k == 8 && r.is_err(), "equal flip beyond IM refused");
}

// Snapshot sizes must equal the type sizes in the Kani build (compile-time check).
const _: () = {
    assert!(core::mem::size_of::<MarketGroupV16HeaderAccount>() == opened_snap::OPEN_HEADER.len());
    assert!(core::mem::size_of::<EngineAssetSlotV16Account>() == opened_snap::OPEN_SLOT.len());
    assert!(core::mem::size_of::<PortfolioAccountV16Account>() == opened_snap::OPEN_A.len());
};

/// Probe: the snapshot loader alone (diagnoses the IM2 bytemuck failure).
#[kani::proof]
fn kani_eng1020_probe_snapshot_load() {
    let (a, b, h, m) = k126_opened_snap(None);
    kani::cover!(a.capital.get() > 0 && b.capital.get() > 0 && h.vault.get() > 0 && m[0].engine.asset.market_id.get() != 0, "snapshot loaded");
}
