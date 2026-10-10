#![cfg(kani)]
//! v2.2 band + rent Kani harnesses (E-BND-1..31), design `docs/v22-band-rent-kani-design.md`
//! as amended by `~/percolator-ops/ledger/kani-v22-final-run-design-rev2-2026-10-09.md` (R1.5)
//! and `...-rev2.1-2026-10-09.md`.
//!
//! Rules (vacuity, run once): every harness calls the REAL production function (pure
//! `band_rent` fns directly; private engine kernels through the one-line forwards in
//! `src/kani_v22_band_shims.rs`); every claimed branch has a `kani::cover!`, and every
//! `kani::assume` on a fixture is followed by a probe cover; frames compare Pod / runtime
//! structs with `==`. u128 multiply/divide only at bounded operands (u8/u16/u32 cast up),
//! with the full-width claim carried by the proptests in `tests/v22_band_rent.rs`.
//!
//! Evidence labels: E-BND-19..23 (I-B5) are a ONE-LEG MODEL whose transitions call the real
//! arithmetic; label "bounded + paper (scale lemma)". Everything else is "proof" at the
//! stated bounds.
//!
//! Scale lemma (I-B5, rev 2 R1.5): the induction obligations depend on time only through
//! `t_used <= 2E` and on the Band Safety Law rate term linearly in `E`; the code floors are
//! `E >= BAND_MIN_EPOCH_SLOTS (150)` and `Pmax >= 8E` (`src/v16.rs:5301-5304`). The bounded
//! instance runs `E in {150, 151}`; a proof at the floor holds for every larger `E` together
//! with the validator's BSL instance at that `E` (paper argument, not machine-checked).

use percolator::band_rent::{
    band_bounds, band_d1_pinned_step, band_duration_pinned, band_width_ok, band_worst_adverse_bps,
    clamp_to_band, price_in_band, rent_chargeable_atoms, rent_due_with_carry, rent_index_delta,
    BAND_MAX_POSITIONS_PER_SIDE, BAND_MIN_EPOCH_SLOTS, BAND_MIN_PIN_EPOCHS, MAX_BAND_BPS,
    RENT_INDEX_DEN,
};
use percolator::v16::{
    AssetStateV16, AssetStateV16Account, EngineAssetSlotV16Account, Market,
    MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, PortfolioAccountV16Account,
    PortfolioLegV16, PortfolioLegV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, SideV16, TradeRequestV16, V16Config, V16Error, V16PodU128,
    V16PodU64,
};
use percolator::v16::kani_active_bitmap_set as active_bitmap_set;
use percolator::{ADL_ONE, FUNDING_DEN, MAX_ORACLE_PRICE, POS_SCALE};

// ---------------------------------------------------------------------------------------------
// Fixtures (real constructors only)
// ---------------------------------------------------------------------------------------------

const P0: u64 = 1_000_000;
const MARKET_ID: [u8; 32] = [7; 32];
const BAND_D: u64 = 130;
const BAND_E: u64 = 600;

/// The 10x band preset of `tests/v22_band_rent.rs` (MMR 5%, d = 130, E = 600, Pmax = 9,000):
/// above the code floors (E >= 150, Pmax >= 8E).
fn band_cfg(band_bps: u64) -> V16Config {
    let mut c = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    c.maintenance_margin_bps = 500;
    c.initial_margin_bps = 1_000;
    c.min_nonzero_mm_req = 10_000;
    c.min_nonzero_im_req = 20_000;
    c.liquidation_fee_bps = 50;
    c.liquidation_fee_cap = 1_000_000_000_000;
    c.min_liquidation_abs = 0;
    c.max_abs_funding_e9_per_slot = 111;
    c.max_price_move_bps_per_slot = 100;
    c.max_accrual_dt_slots = 3;
    c.min_funding_lifetime_slots = 3;
    c.max_trading_fee_bps = 100;
    c.rent_max_e9_per_slot = 23;
    c.max_bankrupt_close_lifetime_slots = 1_000_000;
    c.band_bps = band_bps;
    if band_bps != 0 {
        c.band_max_epoch_slots = BAND_E;
        c.band_max_pin_slots = 9_000;
        c.band_max_positions_per_side = BAND_MAX_POSITIONS_PER_SIDE;
        c.band_min_leg_notional = 1;
    }
    c
}

/// One-asset market group through `new_dynamic` + `activate_empty_asset_slot_not_atomic`.
fn market_fixture(cfg: V16Config, price: u64) -> (MarketGroupV16HeaderAccount, [Market<u64>; 1]) {
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(MARKET_ID, cfg, 1, 0).unwrap();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, price, 1)
        .unwrap();
    (header, markets)
}

fn account_fixture(tag: u8) -> PortfolioAccountV16Account {
    let mut id = [1u8; 32];
    id[0] = tag;
    let prov = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(MARKET_ID, id, [9; 32]));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(prov).unwrap();
    a
}

fn any_side() -> SideV16 {
    if kani::any() {
        SideV16::Long
    } else {
        SideV16::Short
    }
}

fn asset_of(markets: &[Market<u64>; 1]) -> AssetStateV16 {
    markets[0].engine.asset.try_to_runtime().unwrap()
}

fn put_asset(markets: &mut [Market<u64>; 1], a: &AssetStateV16) {
    markets[0].engine.asset = AssetStateV16Account::from_runtime(a);
}

/// A band-armed asset (epoch >= 1, wide anchor) as a plain runtime value, for the pure kernels.
fn armed_asset(epoch: u64) -> AssetStateV16 {
    let mut a = AssetStateV16::default();
    a.effective_price = P0;
    a.raw_oracle_target_price = P0;
    a.fund_px_last = P0;
    a.band_anchor_price = P0;
    a.band_epoch = epoch;
    a
}

fn uncertified(a: &AssetStateV16, side: SideV16) -> u64 {
    match side {
        SideV16::Long => a.band_uncertified_long,
        SideV16::Short => a.band_uncertified_short,
    }
}

fn liq_pending(a: &AssetStateV16, side: SideV16) -> u64 {
    match side {
        SideV16::Long => a.band_liq_pending_long,
        SideV16::Short => a.band_liq_pending_short,
    }
}

fn stored(a: &AssetStateV16, side: SideV16) -> u64 {
    match side {
        SideV16::Long => a.stored_pos_count_long,
        SideV16::Short => a.stored_pos_count_short,
    }
}

// ---------------------------------------------------------------------------------------------
// §1 pure arithmetic (src/band_rent.rs)
// ---------------------------------------------------------------------------------------------

/// E-BND-1 (§1 `kani_band_bounds_inward_contain_anchor`). `band_bounds` `band_rent.rs:62`.
/// Bounds: anchor u32 >= 1, d in 1..=2000; domain check over full u64. Mutants: none planned.
#[kani::proof]
#[kani::solver(cadical)]
fn kani_band_bounds_inward_contain_anchor() {
    let anchor = kani::any::<u32>() as u64;
    let d = kani::any::<u16>() as u64;
    kani::assume(anchor >= 1 && d >= 1 && d <= MAX_BAND_BPS);
    let (lo, hi) = band_bounds(anchor, d).unwrap();
    assert!(1 <= lo && lo <= anchor && anchor <= hi && hi <= MAX_ORACLE_PRICE);
    assert!(lo as u128 * 10_000 >= anchor as u128 * (10_000 - d as u128));
    assert!(hi as u128 * 10_000 <= anchor as u128 * (10_000 + d as u128));
    kani::cover!(lo == anchor, "tiny anchor: lo == anchor");
    kani::cover!(lo < anchor, "lo < anchor");
    kani::cover!(hi == anchor, "hi == anchor");
    kani::cover!(hi > anchor, "hi > anchor");

    // domain: Err iff out of domain (full u64)
    let a2: u64 = kani::any();
    let d2: u64 = kani::any();
    let out = a2 == 0 || a2 > MAX_ORACLE_PRICE || d2 == 0 || d2 > MAX_BAND_BPS;
    if out {
        assert!(band_bounds(a2, d2).is_err());
    }
    kani::cover!(out, "out-of-domain input refused");
}

/// E-BND-2 (§1 `kani_band_clamp_in_range`). `clamp_to_band` `band_rent.rs:149`.
#[kani::proof]
#[kani::solver(cadical)]
fn kani_band_clamp_in_range() {
    let price = kani::any::<u32>() as u64;
    let anchor = kani::any::<u32>() as u64;
    let d = kani::any::<u16>() as u64;
    kani::assume(anchor >= 1 && d >= 1 && d <= MAX_BAND_BPS);
    let (lo, hi) = band_bounds(anchor, d).unwrap();
    let r = clamp_to_band(price, anchor, d).unwrap();
    assert!(lo <= r && r <= hi);
    assert_eq!(clamp_to_band(r, anchor, d).unwrap(), r, "idempotent");
    if lo <= price && price <= hi {
        assert_eq!(r, price);
    }
    if price > hi {
        assert_eq!(r, hi);
    }
    if price < lo {
        assert_eq!(r, lo);
    }
    kani::cover!(lo <= price && price <= hi, "inside");
    kani::cover!(price > hi, "above");
    kani::cover!(price < lo, "below");
}

/// E-BND-3 (§1 `kani_band_G_upper_bound`, I-B6). `band_worst_adverse_bps` `band_rent.rs:157`,
/// `price_in_band` `:142`. Exhaustive d in [1, 300]; prices u16.
/// A leg certified at p1 in band(A), next anchor A' in band(A), current price p3 in band(A'):
/// |p3 - p1| * 1e4 <= G * p1 in both directions. Mutant: BND-M3 (G = 2d).
#[kani::proof]
#[kani::solver(cadical)]
fn kani_band_G_upper_bound() {
    let d = kani::any::<u16>() as u64;
    kani::assume(d >= 1 && d <= 300);
    let a = kani::any::<u16>() as u64;
    let a2 = kani::any::<u16>() as u64;
    let p1 = kani::any::<u16>() as u64;
    let p3 = kani::any::<u16>() as u64;
    kani::assume(a >= 1 && a2 >= 1);
    kani::assume(price_in_band(a2, a, d).unwrap());
    kani::assume(price_in_band(p1, a, d).unwrap());
    kani::assume(price_in_band(p3, a2, d).unwrap());
    kani::cover!(true, "fixture probe: chained bands satisfiable");
    let g = band_worst_adverse_bps(d).unwrap() as u128;
    let diff = p3.abs_diff(p1) as u128;
    assert!(diff * 10_000 <= g * p1 as u128);
    kani::cover!(p3 > p1 && a2 > a, "upward worst case");
    kani::cover!(p3 < p1 && a2 < a, "downward worst case");
    kani::cover!(a2 == a, "A' == A");
}

/// E-BND-4 (§1 `kani_band_d1_step`). `band_d1_pinned_step` `band_rent.rs:204`.
#[kani::proof]
#[kani::solver(cadical)]
fn kani_band_d1_step() {
    let current = kani::any::<u32>() as u64;
    let step = kani::any::<u32>() as u64;
    let anchor = kani::any::<u32>() as u64;
    let d = kani::any::<u16>() as u64;
    let pinned: bool = kani::any();
    kani::assume(anchor >= 1 && d >= 1 && d <= MAX_BAND_BPS);
    let (p, moved) = band_d1_pinned_step(current, step, anchor, d, pinned).unwrap();
    if pinned {
        assert_eq!((p, moved), (current, step != current));
    } else {
        let c = clamp_to_band(step, anchor, d).unwrap();
        assert_eq!((p, moved), (c, c != step));
    }
    let (lo, hi) = band_bounds(anchor, d).unwrap();
    kani::cover!(pinned && step != current, "pinned, moved");
    kani::cover!(pinned && step == current, "pinned, unchanged");
    kani::cover!(!pinned && step > hi, "edge-clamped up");
    kani::cover!(!pinned && step < lo, "edge-clamped down");
    kani::cover!(!pinned && lo <= step && step <= hi, "pass-through");
}

/// E-BND-5 (§1 `kani_band_duration_pinned`). `band_duration_pinned` `band_rent.rs:174`.
/// Mutant: BND-M4 (its call site) is caught by E-BND-14.
#[kani::proof]
fn kani_band_duration_pinned() {
    let end = kani::any::<u32>() as u64;
    let anchor = kani::any::<u32>() as u64;
    let e = kani::any::<u32>() as u64;
    let r = band_duration_pinned(end, anchor, e);
    assert_eq!(r, end > anchor && end - anchor > e);
    kani::cover!(r, "pinned");
    kani::cover!(!r && end >= anchor, "inside the window");
    kani::cover!(end < anchor, "end before anchor: inside");
}

/// E-BND-6 (§1 `kani_rent_due_with_carry_exact`). `rent_due_with_carry` `band_rent.rs:245`.
/// abs_q u16, index/snap u32, carry < RENT_INDEX_DEN. Mutants: BND-R3 (ceil).
#[kani::proof]
#[kani::solver(cadical)]
fn kani_rent_due_with_carry_exact() {
    let abs_q = kani::any::<u16>() as u128;
    let index = kani::any::<u32>() as u128;
    let snap = kani::any::<u32>() as u128;
    let carry: u64 = kani::any();
    match rent_due_with_carry(abs_q, index, snap, carry) {
        Ok((due, carry2)) => {
            assert!(snap <= index && (carry as u128) < RENT_INDEX_DEN);
            assert_eq!(due * RENT_INDEX_DEN + carry2 as u128, abs_q * (index - snap) + carry as u128);
            assert!((carry2 as u128) < RENT_INDEX_DEN);
            kani::cover!(due == 0, "due 0");
            kani::cover!(due > 0, "due > 0");
            kani::cover!(carry2 == 0, "carry' 0");
            kani::cover!(carry2 > 0, "carry' > 0");
        }
        Err(_) => {
            assert!(snap > index || carry as u128 >= RENT_INDEX_DEN);
            kani::cover!(snap > index, "snap > index refused");
        }
    }
}

/// E-BND-7 (§1 `kani_rent_split_never_overcharges`, I-R4). Two settles with the carried
/// remainder equal one settle over the whole interval. Mutant: BND-R4 (carry dropped).
#[kani::proof]
#[kani::solver(cadical)]
fn kani_rent_split_never_overcharges() {
    let abs_q = kani::any::<u8>() as u128;
    let s = kani::any::<u16>() as u128;
    let x = kani::any::<u16>() as u128;
    let y = kani::any::<u16>() as u128;
    let c0: u64 = kani::any();
    kani::assume((c0 as u128) < RENT_INDEX_DEN);
    let (d1, c1) = rent_due_with_carry(abs_q, s + x, s, c0).unwrap();
    let (d2, c2) = rent_due_with_carry(abs_q, s + x + y, s + x, c1).unwrap();
    let (d, c) = rent_due_with_carry(abs_q, s + x + y, s, c0).unwrap();
    assert_eq!(d1 + d2, d);
    assert_eq!(c2, c);
    kani::cover!(d1 > 0 && d2 > 0, "both splits non-zero");
}

/// E-BND-8 (§1 `kani_rent_chargeable_junior`, I-R7). `rent_chargeable_atoms` `band_rent.rs:280`.
#[kani::proof]
fn kani_rent_chargeable_junior() {
    let due = kani::any::<u32>() as u128;
    let capital = kani::any::<u32>() as u128;
    let pnl = kani::any::<i32>() as i128;
    let charged = rent_chargeable_atoms(due, capital, pnl);
    assert!(charged <= due && charged <= capital);
    if pnl < 0 {
        assert!(capital - charged >= core::cmp::min(pnl.unsigned_abs(), capital));
    }
    kani::cover!(pnl < 0 && charged > 0, "loss-owned part protected, rest charged");
    kani::cover!(pnl < 0 && capital <= pnl.unsigned_abs() && due > 0 && charged == 0, "fully loss-owned: waived");
    kani::cover!(pnl >= 0 && charged == core::cmp::min(due, capital) && charged > 0, "pnl >= 0");
}

/// E-BND-9 (§1 `kani_rent_index_delta_failclosed`, I-R2). `rent_index_delta` `band_rent.rs:221`.
#[kani::proof]
#[kani::solver(cadical)]
fn kani_rent_index_delta_failclosed() {
    let p = kani::any::<u32>() as u64;
    let r = kani::any::<u32>() as u64;
    let dt = kani::any::<u32>() as u64;
    // u32 operands: always fits (2^96), exact
    assert_eq!(rent_index_delta(p, r, dt).unwrap(), p as u128 * r as u128 * dt as u128);
    kani::cover!(p > 0 && r > 0 && dt > 0, "non-zero exact");
    // u64 twin at the top of the range: overflow fails closed
    let dt2: u64 = kani::any();
    kani::assume(dt2 >= 2);
    assert_eq!(rent_index_delta(u64::MAX, u64::MAX, dt2), Err(percolator::band_rent::BandRentError::Overflow));
    assert!(rent_index_delta(u64::MAX, u64::MAX, 1).is_ok());
    kani::cover!(true, "overflow arm reached");
}

// ---------------------------------------------------------------------------------------------
// §2 engine kernels
// ---------------------------------------------------------------------------------------------

/// E-BND-10 (§2 `kani_band_attach_detach_roundtrip`, I-B7). Kernels `:1483`, `:1518`.
/// Mutant: BND-M6 (detach without the decrement).
#[kani::proof]
#[kani::solver(cadical)]
fn kani_band_attach_detach_roundtrip() {
    let epoch = kani::any::<u8>() as u64;
    let side = any_side();
    let mut a = armed_asset(epoch);
    a.band_uncertified_long = kani::any::<u8>() as u64;
    a.band_uncertified_short = kani::any::<u8>() as u64;
    a.band_liq_pending_long = kani::any::<u8>() as u64;
    a.band_liq_pending_short = kani::any::<u8>() as u64;
    a.stored_pos_count_long = kani::any::<u8>() as u64;
    a.stored_pos_count_short = kani::any::<u8>() as u64;
    let a0 = a;
    let attached = a.kani_bnd_band_attach(side, BAND_D, BAND_MAX_POSITIONS_PER_SIDE).unwrap();
    if epoch == 0 {
        assert_eq!(attached, a0, "band off: counters untouched (I-B7)");
    } else {
        assert_eq!(uncertified(&attached, side), uncertified(&a0, side) + 1);
    }
    let leg = PortfolioLegV16 { active: true, side, band_epoch_snap: 0, band_liq_pending: false, ..PortfolioLegV16::EMPTY };
    let back = attached.kani_bnd_band_detach(leg).unwrap();
    assert_eq!(back, a0, "attach then detach of an uncertified leg is identity");
    kani::cover!(epoch == 0, "band off");
    kani::cover!(epoch != 0, "band on");

    // a liq-pending, certified leg leaves both cohorts it is in
    let snap = kani::any::<u8>() as u64;
    let leg2 = PortfolioLegV16 { active: true, side, band_epoch_snap: snap, band_liq_pending: true, ..PortfolioLegV16::EMPTY };
    if epoch != 0 && snap <= epoch {
      if let Ok(d2) = a0.kani_bnd_band_detach(leg2) {
        assert_eq!(liq_pending(&d2, side) + 1, liq_pending(&a0, side));
        let unc_drop = if snap < epoch { 1 } else { 0 };
        assert_eq!(uncertified(&d2, side) + unc_drop, uncertified(&a0, side));
        kani::cover!(true, "liq-pending leg detached");
      }
    }
    // a leg from the future is refused
    let leg3 = PortfolioLegV16 { active: true, side, band_epoch_snap: epoch + 1, ..PortfolioLegV16::EMPTY };
    assert_eq!(a0.kani_bnd_band_detach(leg3), Err(V16Error::InvalidLeg));
}

/// E-BND-11 (§2 `kani_band_certify_leg`). Kernel `:1566`. Mutant: BND-M5.
#[kani::proof]
#[kani::solver(cadical)]
fn kani_band_certify_leg() {
    let epoch = kani::any::<u8>() as u64;
    let side = any_side();
    let mut a = armed_asset(epoch);
    a.band_uncertified_long = kani::any::<u8>() as u64;
    a.band_uncertified_short = kani::any::<u8>() as u64;
    a.band_liq_pending_long = kani::any::<u8>() as u64;
    a.band_liq_pending_short = kani::any::<u8>() as u64;
    let snap = kani::any::<u16>() as u64;
    let liq: bool = kani::any();
    let active: bool = kani::any();
    let healthy: bool = kani::any();
    let leg = PortfolioLegV16 { active, side, band_epoch_snap: snap, band_liq_pending: liq, ..PortfolioLegV16::EMPTY };
    let r = a.kani_bnd_band_certify_leg(leg, healthy);
    if epoch == 0 || !active {
        assert_eq!(r, Ok((a, leg)));
        kani::cover!(true, "identity arm");
        return;
    }
    if snap > epoch {
        assert_eq!(r, Err(V16Error::InvalidLeg));
        kani::cover!(true, "future snap refused");
        return;
    }
    if let Ok((a2, l2)) = r {
        if healthy {
            assert_eq!(l2.band_epoch_snap, epoch);
            assert!(!l2.band_liq_pending);
            let du = if snap < epoch { 1 } else { 0 };
            let dl = if liq { 1 } else { 0 };
            assert_eq!(uncertified(&a2, side) + du, uncertified(&a, side));
            assert_eq!(liq_pending(&a2, side) + dl, liq_pending(&a, side));
            // second certification is a no-op (no double decrement)
            assert_eq!(a2.kani_bnd_band_certify_leg(l2, true), Ok((a2, l2)));
            kani::cover!(snap < epoch, "healthy, newly certified");
            kani::cover!(snap == epoch && !liq, "healthy, already certified: no-op");
            kani::cover!(liq, "healthy heals liq-pending");
        } else {
            assert!(l2.band_liq_pending);
            assert_eq!(l2.band_epoch_snap, snap, "an unhealthy leg is never certified");
            assert_eq!(uncertified(&a2, side), uncertified(&a, side));
            let dl = if liq { 0 } else { 1 };
            assert_eq!(liq_pending(&a2, side), liq_pending(&a, side) + dl);
            assert_eq!(a2.kani_bnd_band_certify_leg(l2, false), Ok((a2, l2)), "idempotent mark");
            kani::cover!(!liq, "unhealthy, newly marked");
            kani::cover!(liq, "unhealthy, already marked");
        }
    }
}

/// E-BND-12 (§2 `kani_band_reanchor_iff_certified`, I-B3). Kernels `:1613`, `:1631`.
/// Mutants: BND-M1, BND-M2.
#[kani::proof]
#[kani::solver(cadical)]
fn kani_band_reanchor_iff_certified() {
    let mut a = armed_asset(kani::any::<u8>() as u64);
    a.band_uncertified_long = kani::any::<u8>() as u64;
    a.band_uncertified_short = kani::any::<u8>() as u64;
    a.band_liq_pending_long = kani::any::<u8>() as u64;
    a.band_liq_pending_short = kani::any::<u8>() as u64;
    a.stored_pos_count_long = kani::any::<u8>() as u64;
    a.stored_pos_count_short = kani::any::<u8>() as u64;
    a.effective_price = kani::any::<u32>() as u64;
    a.band_pin_since_slot = kani::any::<u16>() as u64;
    let bl = kani::any::<u8>() as u64;
    let bs = kani::any::<u8>() as u64;
    let w = kani::any::<u16>() as u64;
    let ready = a.kani_bnd_band_reanchor_ready(bl, bs);
    let expected = a.band_epoch != 0
        && a.band_uncertified_long == 0
        && a.band_uncertified_short == 0
        && a.band_liq_pending_long == 0
        && a.band_liq_pending_short == 0
        && bl == 0
        && bs == 0;
    assert_eq!(ready, expected);
    let r = a.kani_bnd_band_reanchor(w);
    if ready {
        let a2 = r.unwrap();
        assert_eq!(a2.band_anchor_price, a.effective_price);
        assert_eq!(a2.band_anchor_slot, w);
        assert_eq!(a2.band_epoch, a.band_epoch + 1);
        assert_eq!(a2.band_uncertified_long, a.stored_pos_count_long);
        assert_eq!(a2.band_uncertified_short, a.stored_pos_count_short);
        assert_eq!(a2.band_pin_since_slot, 0);
        kani::cover!(true, "ready: re-anchored");
    }
    if a.band_epoch == 0
        || a.band_uncertified_long != 0
        || a.band_uncertified_short != 0
        || a.band_liq_pending_long != 0
        || a.band_liq_pending_short != 0
    {
        assert!(r.is_err(), "defense in depth: a not-ready asset is never re-anchored");
    }
    kani::cover!(!ready && a.band_epoch == 0, "not ready: band off");
    kani::cover!(!ready && a.band_uncertified_long != 0, "not ready: uncertified long");
    kani::cover!(!ready && a.band_uncertified_short != 0, "not ready: uncertified short");
    kani::cover!(!ready && a.band_liq_pending_long != 0, "not ready: liq long");
    kani::cover!(!ready && a.band_liq_pending_short != 0, "not ready: liq short");
    kani::cover!(!ready && bl != 0, "not ready: barrier long");
    kani::cover!(!ready && bs != 0, "not ready: barrier short");
}

const NL: usize = 3;

/// Census of a 3-leg model: (stored, uncertified, liq_pending) for `side`.
fn census(legs: &[PortfolioLegV16; NL], side: SideV16, epoch: u64) -> (u64, u64, u64) {
    let mut s = 0u64;
    let mut u = 0u64;
    let mut l = 0u64;
    let mut i = 0usize;
    while i < NL {
        let g = legs[i];
        if g.active && g.side == side {
            s += 1;
            if g.band_epoch_snap < epoch {
                u += 1;
            }
            if g.band_liq_pending {
                l += 1;
            }
        }
        i += 1;
    }
    (s, u, l)
}

fn census_holds(legs: &[PortfolioLegV16; NL], a: &AssetStateV16) -> bool {
    let (sl, ul, ll) = census(legs, SideV16::Long, a.band_epoch);
    let (ss, us, ls) = census(legs, SideV16::Short, a.band_epoch);
    let mut snaps_ok = true;
    let mut i = 0usize;
    while i < NL {
        if legs[i].active && legs[i].band_epoch_snap > a.band_epoch {
            snaps_ok = false;
        }
        i += 1;
    }
    snaps_ok
        && sl == a.stored_pos_count_long
        && ss == a.stored_pos_count_short
        && ul == a.band_uncertified_long
        && us == a.band_uncertified_short
        && ll == a.band_liq_pending_long
        && ls == a.band_liq_pending_short
}

/// E-BND-13 (§2 `kani_band_census_inductive`, I-B2). Real `kernel_attach_leg` `:1398`,
/// `kernel_clear_leg` `:1784`, `kernel_band_certify_leg`, `kernel_band_reanchor_ready` /
/// `kernel_band_reanchor`. A 3-leg model whose asset counters equal the census; ONE
/// nondeterministic transition; the census still equals the counters. Mutants: BND-M6, BND-R1
/// (via E-BND-17), C1-C3 are LiteSVM-pinned.
#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn kani_band_census_inductive() {
    let epoch = 1 + (kani::any::<u8>() % 3) as u64;
    let mut a = armed_asset(epoch);
    let mut legs = [PortfolioLegV16::EMPTY; NL];
    let mut i = 0usize;
    while i < NL {
        if kani::any() {
            let side = any_side();
            let snap = kani::any::<u8>() as u64;
            kani::assume(snap <= epoch);
            legs[i] = PortfolioLegV16 {
                active: true,
                side,
                basis_pos_q: if side == SideV16::Long { POS_SCALE as i128 } else { -(POS_SCALE as i128) },
                a_basis: ADL_ONE,
                loss_weight: 1,
                band_epoch_snap: snap,
                band_liq_pending: kani::any(),
                ..PortfolioLegV16::EMPTY
            };
            match side {
                SideV16::Long => {
                    a.oi_eff_long_q += POS_SCALE;
                    a.loss_weight_sum_long += 1;
                }
                SideV16::Short => {
                    a.oi_eff_short_q += POS_SCALE;
                    a.loss_weight_sum_short += 1;
                }
            }
        }
        i += 1;
    }
    let (sl, ul, ll) = census(&legs, SideV16::Long, epoch);
    let (ss, us, ls) = census(&legs, SideV16::Short, epoch);
    a.stored_pos_count_long = sl;
    a.stored_pos_count_short = ss;
    a.band_uncertified_long = ul;
    a.band_uncertified_short = us;
    a.band_liq_pending_long = ll;
    a.band_liq_pending_short = ls;
    assert!(census_holds(&legs, &a));

    let k = kani::any::<u8>() as usize % NL;
    let t = kani::any::<u8>() % 5;
    match t {
        0 => {
            kani::assume(!legs[k].active);
            let side = any_side();
            let q: i128 = if side == SideV16::Long { POS_SCALE as i128 } else { -(POS_SCALE as i128) };
            if let Ok((a2, leg)) = a.kani_bnd_attach_leg(side, q, 1, 0, BAND_D, BAND_MAX_POSITIONS_PER_SIDE) {
                a = a2;
                legs[k] = leg;
                kani::cover!(true, "attach taken");
            } else {
                return;
            }
        }
        1 => {
            kani::assume(legs[k].active);
            if let Ok(a2) = a.kani_bnd_clear_leg(legs[k], POS_SCALE) {
                a = a2;
                legs[k] = PortfolioLegV16::EMPTY;
                kani::cover!(true, "clear taken");
            } else {
                return;
            }
        }
        2 | 3 => {
            kani::assume(legs[k].active);
            let healthy = t == 2;
            if let Ok((a2, l2)) = a.kani_bnd_band_certify_leg(legs[k], healthy) {
                a = a2;
                legs[k] = l2;
                kani::cover!(healthy, "certify healthy taken");
                kani::cover!(!healthy, "certify unhealthy taken");
            } else {
                return;
            }
        }
        _ => {
            if a.kani_bnd_band_reanchor_ready(0, 0) {
                a = a.kani_bnd_band_reanchor(kani::any::<u16>() as u64).unwrap();
                kani::cover!(sl + ss > 0, "re-anchor with a positioned book");
            } else {
                return;
            }
        }
    }
    assert!(census_holds(&legs, &a), "census equals the counters after every transition");
}

/// Positioned band fixture: one long and one short "virtual" leg in the counters (no account),
/// both uncertified so the asset is not re-anchor-ready; shape-validated by assumption.
fn positioned_band_fixture() -> (MarketGroupV16HeaderAccount, [Market<u64>; 1]) {
    let (header, mut markets) = market_fixture(band_cfg(BAND_D), P0);
    let mut a = asset_of(&markets);
    a.stored_pos_count_long = 1;
    a.stored_pos_count_short = 1;
    a.oi_eff_long_q = POS_SCALE;
    a.oi_eff_short_q = POS_SCALE;
    a.loss_weight_sum_long = 1;
    a.loss_weight_sum_short = 1;
    a.band_uncertified_long = 1;
    a.band_uncertified_short = 1;
    put_asset(&mut markets, &a);
    (header, markets)
}

/// E-BND-14 (§2 `kani_band_accrue_rejects_before_mutation`, I-B1, I-B4, spec test 10).
/// `accrue_asset_to_with_rent_not_atomic` `:16875` (band checks `:16922-16941`).
/// An out-of-band price is `BandOutOfRange` and a moving accrual past the window is
/// `BandPinned`, both with header and slot byte-identical. Mutant: BND-M4 (rev 2.1 location
/// corrected to `:16931`, the pin check in the accrual).
#[kani::proof]
#[kani::unwind(5)]
#[kani::solver(cadical)]
fn kani_band_accrue_rejects_before_mutation() {
    let (mut header, mut markets) = positioned_band_fixture();
    let mut a = asset_of(&markets);
    // move the clock so the window can elapse; the governing anchor slot may lie up to
    // E + 300 slots in the past
    a.slot_last = 10_000;
    header.current_slot = V16PodU64::new(10_000);
    let back = kani::any::<u16>() as u64;
    kani::assume(back <= BAND_E + 300);
    a.band_anchor_slot = a.slot_last - back;
    put_asset(&mut markets, &a);
    {
        let m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        kani::assume(m.validate_shape().is_ok());
    }
    kani::cover!(true, "fixture probe: positioned band book is shape-valid");
    let h0 = header;
    let s0 = markets[0].engine;
    let price = kani::any::<u32>() as u64;
    kani::assume(price >= 1);
    let dt = kani::any::<u8>() as u64 % 4;
    let now = a.slot_last + dt;
    let r = {
        let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        m.accrue_asset_to_with_rent_not_atomic(0, now, price, 0, 0, 0, false)
    };
    let in_band = price_in_band(price, a.band_anchor_price, BAND_D).unwrap();
    let pinned = band_duration_pinned(now, a.band_anchor_slot, BAND_E);
    if !in_band {
        assert_eq!(r.map(|_| ()), Err(V16Error::BandOutOfRange));
        assert!(header == h0 && markets[0].engine == s0, "refused before any mutation");
        kani::cover!(true, "out of band refused");
    } else if pinned && price != a.effective_price {
        assert_eq!(r.map(|_| ()), Err(V16Error::BandPinned));
        assert!(header == h0 && markets[0].engine == s0, "refused before any mutation");
        kani::cover!(true, "duration-pinned moving accrual refused");
    } else {
        kani::cover!(r.is_ok(), "in-band accrual accepted");
    }
}

/// E-BND-15 (§2 `kani_band_unarmed_fails_closed`). `band_prepare_accrual` `:16789`:
/// `band_bps != 0 && band_epoch == 0` is `InvalidConfig` for any state (no shape assumption:
/// the claim is about a corrupt state).
#[kani::proof]
#[kani::unwind(5)]
#[kani::solver(cadical)]
fn kani_band_unarmed_fails_closed() {
    let (mut header, mut markets) = positioned_band_fixture();
    let mut a = asset_of(&markets);
    a.band_epoch = 0;
    put_asset(&mut markets, &a);
    let price = kani::any::<u32>() as u64;
    kani::assume(price >= 1);
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let r = m.accrue_asset_to_with_rent_not_atomic(0, a.slot_last + 1, price, 0, 0, 0, false);
    assert_eq!(r.map(|_| ()), Err(V16Error::InvalidConfig));
    kani::cover!(true, "unarmed band asset refused");
}

/// E-BND-16 (§2 `kani_rent_route_conserves`, I-R3). `rent_route_delta` `:13094` via the
/// existing shim `kani_rent_route_delta`. Full u128 (min/add/sub only). Mutant: BND-R2.
#[kani::proof]
fn kani_rent_route_conserves() {
    let ru: u128 = kani::any();
    let ins: u128 = kani::any();
    let res: u128 = kani::any();
    let ctot: u128 = kani::any();
    let cap: u128 = kani::any();
    let x_exp = ru.min(ins.saturating_sub(res));
    match MarketGroupV16ViewMut::<u64>::kani_rent_route_delta(ru, ins, res, ctot, cap) {
        Ok((x, nru, ni, nc, ncap)) => {
            assert_eq!(x, x_exp);
            assert!(x <= ru && x <= ins.saturating_sub(res));
            assert_eq!(ni, ins - x);
            assert_eq!(nc, ctot + x);
            assert_eq!(ncap, cap + x);
            assert!(nru <= ru - x && nru <= ni);
            assert_eq!(nru, (ru - x).min(ni));
            kani::cover!(x == ru && x > 0, "full route");
            kani::cover!(x < ru && nru > 0, "partial route, claim kept");
            kani::cover!(x == 0, "zero route");
        }
        Err(_) => {
            assert!(ctot.checked_add(x_exp).is_none() || cap.checked_add(x_exp).is_none());
            kani::cover!(true, "overflow fails closed");
        }
    }
}

/// E-BND-17 (§2 `kani_rent_settle_idempotent_and_attach_snap`, I-R4, I-R5).
/// `settle_leg_rent_not_atomic` `:15691` (shim), leg from the real `kernel_attach_leg`.
/// Mutant: BND-R1 (attach does not reset the rent snapshot).
#[kani::proof]
#[kani::unwind(5)]
#[kani::solver(cadical)]
fn kani_rent_settle_idempotent_and_attach_snap() {
    let (mut header, mut markets) = market_fixture(band_cfg(0), P0);
    let mut acct = account_fixture(2);
    let mut a = asset_of(&markets);
    a.rent_index_long_num = kani::any::<u32>() as u128;
    let units = 1 + (kani::any::<u8>() % 8) as i128;
    let (a1, mut leg) = a
        .kani_bnd_attach_leg(SideV16::Long, units * POS_SCALE as i128, (units as u128) * POS_SCALE, 0, 0, 0)
        .unwrap();
    assert_eq!(leg.rent_snap, a1.rent_index_long_num, "a new leg owes nothing for prior time (I-R5)");
    let capital = kani::any::<u32>() as u128;
    let pnl = kani::any::<i32>() as i128;
    acct.capital = V16PodU128::new(capital);
    acct.pnl = percolator::v16::V16PodI128::new(pnl);
    header.c_tot = V16PodU128::new(capital);
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut av = PortfolioV16ViewMut::new(&mut acct);
    assert_eq!(m.kani_bnd_settle_leg_rent(&mut av, a1, &mut leg), Ok(0), "settle at the attach index charges 0");
    let delta = kani::any::<u32>() as u128;
    let mut a2 = a1;
    a2.rent_index_long_num += delta;
    let q = a2.kani_bnd_effective_abs_q(leg).unwrap();
    let (due, _) = rent_due_with_carry(q, a2.rent_index_long_num, leg.rent_snap, leg.rent_carry).unwrap();
    let c1 = m.kani_bnd_settle_leg_rent(&mut av, a2, &mut leg);
    if let Ok(c1) = c1 {
        assert_eq!(c1, rent_chargeable_atoms(due, capital, pnl));
        assert_eq!(leg.rent_snap, a2.rent_index_long_num);
        assert_eq!(m.kani_bnd_settle_leg_rent(&mut av, a2, &mut leg), Ok(0), "second settle at the same index charges 0 (I-R4)");
        kani::cover!(pnl < 0 && capital <= pnl.unsigned_abs() && due > 0 && c1 == 0, "waiver: capital fully loss-owned");
        kani::cover!(c1 > 0 && c1 < due, "capped at chargeable capital");
        kani::cover!(c1 > 0 && c1 == due, "full charge");
    }
}

/// E-BND-18a (§2 `kani_bsl_validator_matches_pointwise`). `validate_band_safety_law` `:5056`
/// on the concrete 10x preset (validator Ok, cover), and for symbolic N the pointwise law
/// `solvency_envelope_holds_for_notional(N, G*FD + rate*2E*1e4, 1e4*FD, G)` (`:4897`) with the
/// documented budget. Cost L (the validator's bisection loop on concrete data).
#[kani::proof]
#[kani::unwind(4100)]
#[kani::solver(cadical)]
fn kani_bsl_validator_matches_pointwise_preset() {
    let c = band_cfg(BAND_D);
    let rate = c.max_abs_funding_e9_per_slot as u128 + c.rent_max_e9_per_slot as u128;
    let ok = c.kani_bnd_validate_band_safety_law(rate).is_ok();
    kani::cover!(ok, "preset passes the BSL validator");
    if ok {
        let n = kani::any::<u32>() as u128;
        let g = band_worst_adverse_bps(c.band_bps).unwrap() as u128;
        let num = g * FUNDING_DEN + rate * (2 * c.band_max_epoch_slots as u128) * 10_000;
        let den = 10_000 * FUNDING_DEN;
        assert_eq!(c.kani_bnd_envelope_holds(n, num, den, g), Ok(true));
        kani::cover!(n > 0, "non-zero notional");
    }
}

/// E-BND-18b: as 18a with the maintenance margin symbolic: validator Ok ⇒ pointwise holds.
/// Cover: some MMR is refused. Cost L / high risk.
#[kani::proof]
#[kani::unwind(4100)]
#[kani::solver(cadical)]
fn kani_bsl_validator_matches_pointwise_mmr() {
    let mut c = band_cfg(BAND_D);
    c.maintenance_margin_bps = kani::any::<u16>() as u64;
    kani::assume(c.maintenance_margin_bps >= 1 && c.maintenance_margin_bps <= 10_000);
    let rate = c.max_abs_funding_e9_per_slot as u128 + c.rent_max_e9_per_slot as u128;
    let ok = c.kani_bnd_validate_band_safety_law(rate).is_ok();
    kani::cover!(ok, "accepted MMR");
    kani::cover!(!ok, "refused MMR");
    if ok {
        let n = kani::any::<u16>() as u128;
        let g = band_worst_adverse_bps(c.band_bps).unwrap() as u128;
        let num = g * FUNDING_DEN + rate * (2 * c.band_max_epoch_slots as u128) * 10_000;
        let den = 10_000 * FUNDING_DEN;
        assert_eq!(c.kani_bnd_envelope_holds(n, num, den, g), Ok(true));
    }
}

// ---------------------------------------------------------------------------------------------
// §3 I-B5 induction: ONE-LEG MODEL over the real arithmetic. Label "bounded + paper (scale
// lemma)". State: the leg is certified in the current epoch (`cert_cur`) or the previous one;
// `a_prev` is the anchor of the certification epoch when that is the previous epoch;
// `t_prev` / `t_cur` are loss-accruing (moving) slots used since the C-event in the previous
// / current epoch's window `[aslot, aslot + E]`.
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Ib5 {
    d: u64,
    e_slots: u64,
    a_prev: u64,
    a: u64,
    cert_cur: bool,
    liq: bool,
    p_cert: u64,
    p_last: u64,
    aslot: u64,
    slot_last: u64,
    t_prev: u64,
    t_cur: u64,
}

fn ib5_any() -> Ib5 {
    let d = 1 + (kani::any::<u16>() % 300) as u64;
    let e_slots = BAND_MIN_EPOCH_SLOTS + (kani::any::<u8>() % 2) as u64;
    let s = Ib5 {
        d,
        e_slots,
        a_prev: kani::any::<u16>() as u64,
        a: kani::any::<u16>() as u64,
        cert_cur: kani::any(),
        liq: kani::any(),
        p_cert: kani::any::<u16>() as u64,
        p_last: kani::any::<u16>() as u64,
        aslot: kani::any::<u16>() as u64,
        slot_last: kani::any::<u16>() as u64,
        t_prev: kani::any::<u16>() as u64,
        t_cur: kani::any::<u16>() as u64,
    };
    kani::assume(s.a_prev >= 1 && s.a >= 1);
    s
}

fn ib5_inv(s: &Ib5) -> bool {
    let window_used = s.slot_last.min(s.aslot + s.e_slots).saturating_sub(s.aslot);
    let price_ok = if s.cert_cur {
        s.t_prev == 0 && price_in_band(s.p_cert, s.a, s.d).unwrap()
    } else {
        price_in_band(s.p_cert, s.a_prev, s.d).unwrap() && price_in_band(s.a, s.a_prev, s.d).unwrap()
    };
    s.slot_last >= s.aslot
        && s.t_prev <= s.e_slots
        && s.t_cur <= window_used
        && price_ok
        && price_in_band(s.p_last, s.a, s.d).unwrap()
}

/// The model asset the real readiness kernel sees: one leg, uncertified iff not certified in
/// the current epoch.
fn ib5_asset(s: &Ib5) -> AssetStateV16 {
    let mut a = armed_asset(2);
    a.stored_pos_count_long = 1;
    a.band_uncertified_long = if s.cert_cur { 0 } else { 1 };
    a.band_liq_pending_long = if s.liq { 1 } else { 0 };
    a.effective_price = s.p_last;
    a.band_anchor_price = s.a;
    a
}

/// E-BND-19 `kani_inv_accrue`: an accepted accrual (`price_in_band`, and no move while
/// `band_duration_pinned`) preserves INV. Label: bounded + paper (scale lemma).
#[kani::proof]
#[kani::solver(cadical)]
fn kani_inv_accrue() {
    let s = ib5_any();
    kani::assume(ib5_inv(&s));
    kani::cover!(true, "fixture probe: INV satisfiable");
    let p = kani::any::<u16>() as u64;
    let dt = kani::any::<u8>() as u64;
    let rate_nonzero: bool = kani::any();
    kani::assume(price_in_band(p, s.a, s.d).unwrap());
    let end = s.slot_last + dt;
    let moving = p != s.p_last || rate_nonzero;
    if band_duration_pinned(end, s.aslot, s.e_slots) {
        kani::assume(!moving);
    }
    let mut s2 = s;
    s2.slot_last = end;
    s2.p_last = p;
    if moving {
        s2.t_cur = s.t_cur + dt;
    }
    assert!(ib5_inv(&s2));
    assert!(s2.t_prev + s2.t_cur <= 2 * s2.e_slots, "t_used <= 2E");
    kani::cover!(moving && dt > 0, "moving accrual");
    kani::cover!(!moving && band_duration_pinned(end, s.aslot, s.e_slots), "pinned no-move accrual");
}

/// E-BND-20 `kani_inv_reanchor`: a re-anchor (real `kernel_band_reanchor_ready` on the model
/// asset, real `band_width_ok`) needs the leg certified in the closing epoch and yields
/// `e_cert == e - 1` with `A' = P_last in band(A)`. Label: bounded + paper (scale lemma).
#[kani::proof]
#[kani::solver(cadical)]
fn kani_inv_reanchor() {
    let s = ib5_any();
    kani::assume(ib5_inv(&s));
    kani::cover!(true, "fixture probe");
    let ready = ib5_asset(&s).kani_bnd_band_reanchor_ready(0, 0);
    kani::assume(ready && band_width_ok(s.p_last, s.d).unwrap());
    assert!(s.cert_cur, "only a certified leg lets the anchor advance");
    let s2 = Ib5 {
        a_prev: s.a,
        a: s.p_last,
        cert_cur: false,
        aslot: s.slot_last,
        t_prev: s.t_cur,
        t_cur: 0,
        ..s
    };
    assert!(ib5_inv(&s2));
    kani::cover!(true, "re-anchor reached");
}

/// E-BND-21 `kani_inv_certify`: a healthy C-event re-establishes INV. Label: bounded + paper.
#[kani::proof]
#[kani::solver(cadical)]
fn kani_inv_certify() {
    let s = ib5_any();
    kani::assume(ib5_inv(&s));
    let s2 = Ib5 { cert_cur: true, liq: false, p_cert: s.p_last, t_prev: 0, t_cur: 0, ..s };
    assert!(ib5_inv(&s2));
    kani::cover!(!s.cert_cur, "certify a previous-epoch leg");
}

/// E-BND-22 `kani_inv_no_third_epoch`: from `e_cert == e - 1` the anchor cannot advance
/// (real readiness kernel). Label: bounded + paper.
#[kani::proof]
#[kani::solver(cadical)]
fn kani_inv_no_third_epoch() {
    let s = ib5_any();
    kani::assume(ib5_inv(&s) && !s.cert_cur);
    assert!(!ib5_asset(&s).kani_bnd_band_reanchor_ready(0, 0));
    kani::cover!(!s.liq, "blocked by the uncertified cohort alone (liq-pending not needed)");
}

/// E-BND-23 `kani_bsl_no_bankruptcy_lemma`: INV ⇒ a long leg's equity at P_last covers the
/// liquidation fee. Composition: (i) the G bound between p_cert and P_last (real band fns,
/// checked here); (ii) the pointwise envelope at N_cert (real
/// `solvency_envelope_holds_for_notional`, the validator's guarantee by E-BND-18); (iii)
/// eq_cert >= MM(N_cert) (real `maintenance_requirement_for_notional`). Modelling assumption
/// (stated): the funding + rent loss over `t <= 2E` slots is priced on N_cert, as the design's
/// BSL rate term is. Label: bounded + paper (scale lemma).
#[kani::proof]
#[kani::solver(cadical)]
fn kani_bsl_no_bankruptcy_lemma() {
    let c = band_cfg(BAND_D);
    let s = ib5_any();
    kani::assume(s.d == BAND_D);
    kani::assume(ib5_inv(&s) && s.p_cert >= 1);
    let g = band_worst_adverse_bps(s.d).unwrap() as u128;
    // (i)
    assert!(s.p_last.abs_diff(s.p_cert) as u128 * 10_000 <= g * s.p_cert as u128);
    let units = kani::any::<u8>() as u128;
    kani::assume(units >= 1);
    let n_cert = units * s.p_cert as u128;
    let n_now = units * s.p_last as u128;
    let t = s.t_prev + s.t_cur;
    assert!(t <= 2 * s.e_slots);
    let rate = c.max_abs_funding_e9_per_slot as u128 + c.rent_max_e9_per_slot as u128;
    let num = g * FUNDING_DEN + rate * (2 * c.band_max_epoch_slots as u128) * 10_000;
    let den = 10_000 * FUNDING_DEN;
    kani::assume(c.kani_bnd_envelope_holds(n_cert, num, den, g) == Ok(true));
    let mm = c.kani_bnd_mm_req(n_cert).unwrap();
    let eq_cert = kani::any::<u32>() as u128;
    kani::assume(eq_cert >= mm);
    let price_loss = units * s.p_cert.saturating_sub(s.p_last) as u128;
    let rate_loss = (n_cert * rate * t as u128).div_ceil(1_000_000_000);
    let gain = units * s.p_last.saturating_sub(s.p_cert) as u128;
    assert!(eq_cert + gain >= price_loss + rate_loss, "equity never goes negative before liquidation");
    let eq_now = eq_cert + gain - price_loss - rate_loss;
    let fee_raw = (n_now * c.liquidation_fee_bps as u128).div_ceil(10_000);
    let fee = fee_raw.max(c.min_liquidation_abs).min(c.liquidation_fee_cap);
    assert!(eq_now >= fee, "loss <= capital: the liquidation never leaves bad debt");
    kani::cover!(s.p_last < s.p_cert && t > 0, "adverse move with time used");
}

// ---------------------------------------------------------------------------------------------
// §7 / §8 additions (rev 2 R1.5, rev 2.1)
// ---------------------------------------------------------------------------------------------

/// E-BND-24 (§7 `kani_band_attach_respects_position_cap`, §8 hard bound `cap + 1`).
/// `kernel_band_attach` `:1483` (bound `:1506`). Mutant: BND-C1 (`cap + 2`).
#[kani::proof]
#[kani::solver(cadical)]
fn kani_band_attach_respects_position_cap() {
    let side = any_side();
    let mut a = armed_asset(1 + kani::any::<u8>() as u64);
    a.band_anchor_price = kani::any::<u32>() as u64;
    kani::assume(a.band_anchor_price >= 1);
    a.stored_pos_count_long = kani::any::<u16>() as u64;
    a.stored_pos_count_short = kani::any::<u16>() as u64;
    a.band_uncertified_long = kani::any::<u8>() as u64;
    a.band_uncertified_short = kani::any::<u8>() as u64;
    let cap = 1 + (kani::any::<u16>() as u64 % BAND_MAX_POSITIONS_PER_SIDE);
    let d = 1 + (kani::any::<u16>() as u64 % MAX_BAND_BPS);
    let wide = band_width_ok(a.band_anchor_price, d).unwrap();
    let r = a.kani_bnd_band_attach(side, d, cap);
    let positions = stored(&a, side);
    match r {
        Ok(a2) => {
            assert!(wide && positions <= cap + 1);
            assert_eq!(uncertified(&a2, side), uncertified(&a, side) + 1);
            kani::cover!(positions == cap + 1, "the one exempt attach above the cap");
        }
        Err(V16Error::BandTooNarrow) => {
            assert!(!wide);
            kani::cover!(true, "BandTooNarrow");
        }
        Err(V16Error::BandPositionCap) => {
            assert!(wide && positions > cap + 1);
            kani::cover!(true, "BandPositionCap");
        }
        Err(_) => assert!(false, "no other error for a valid anchor"),
    }
}

/// E-BND-25 (§7 `kani_band_position_cap_census`, rev 2.1): over a bounded sequence of
/// attach / clear on one side through the real kernels, `stored_pos_count_side <= cap + 1`
/// always, so the per-epoch sweep is at most `2 * (cap + 1)` legs. Mutant: BND-C1.
#[kani::proof]
#[kani::unwind(6)]
#[kani::solver(cadical)]
fn kani_band_position_cap_census() {
    let cap = 1 + (kani::any::<u8>() as u64 % 3);
    let side = any_side();
    let q: i128 = if side == SideV16::Long { POS_SCALE as i128 } else { -(POS_SCALE as i128) };
    let mut a = armed_asset(1);
    let mut legs = [PortfolioLegV16::EMPTY; 5];
    let mut n = 0usize;
    let mut step = 0usize;
    while step < 5 {
        if kani::any() && n < 5 {
            if let Ok((a2, leg)) = a.kani_bnd_attach_leg(side, q, 1, 0, BAND_D, cap) {
                a = a2;
                legs[n] = leg;
                n += 1;
            }
        } else if n > 0 {
            if let Ok(a2) = a.kani_bnd_clear_leg(legs[n - 1], POS_SCALE) {
                a = a2;
                n -= 1;
            }
        }
        assert!(stored(&a, side) <= cap + 1);
        assert_eq!(stored(&a, side), n as u64);
        step += 1;
    }
    kani::cover!(n as u64 == cap + 1, "cap + 1 reached");
}

/// E-BND-26 (§7 `kani_band_width_ok_exact`, E-L1): `band_width_ok(A, d) ⇒ lo < A < hi`.
/// anchor up to MAX_ORACLE_PRICE (u64), d in 1..=2000.
#[kani::proof]
#[kani::solver(cadical)]
fn kani_band_width_ok_exact() {
    let anchor: u64 = kani::any();
    let d = 1 + (kani::any::<u16>() as u64 % MAX_BAND_BPS);
    kani::assume(anchor >= 1 && anchor <= MAX_ORACLE_PRICE);
    if band_width_ok(anchor, d).unwrap() {
        let (lo, hi) = band_bounds(anchor, d).unwrap();
        assert!(lo < anchor && anchor < hi, "the band moves both ways");
        kani::cover!(true, "wide band");
    } else {
        kani::cover!(true, "narrow band");
    }
}

/// E-BND-27 (§7 `kani_band_reanchor_never_narrow`): `band_prepare_accrual` `:16777` (shim)
/// never re-anchors onto a band narrower than `MIN_BAND_WIDTH_TICKS`.
#[kani::proof]
#[kani::unwind(5)]
#[kani::solver(cadical)]
fn kani_band_reanchor_never_narrow() {
    let (mut header, mut markets) = market_fixture(band_cfg(BAND_D), P0);
    let mut a = asset_of(&markets);
    a.stored_pos_count_long = 1; // positioned, every cohort certified: ready
    a.effective_price = kani::any::<u32>() as u64;
    kani::assume(a.effective_price >= 1);
    let m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let (a2, active) = m.kani_bnd_band_prepare_accrual(0, a, a.slot_last + 1).unwrap();
    assert!(active);
    if a2.band_epoch > a.band_epoch {
        assert_eq!(band_width_ok(a2.band_anchor_price, BAND_D), Ok(true));
        kani::cover!(true, "re-anchored onto a wide band");
    } else {
        kani::cover!(band_width_ok(a.effective_price, BAND_D) == Ok(false), "narrow price blocks the re-anchor");
    }
}

/// E-BND-28a (§7 `kani_band_config_floors`, band off): `validate_public_user_fund` `:5334`
/// refuses any band field on a band-off config (`:5290-5294`). Cost M.
#[kani::proof]
#[kani::unwind(4100)]
#[kani::solver(cadical)]
fn kani_band_config_floors_band_off() {
    let mut c = band_cfg(0);
    c.band_max_epoch_slots = kani::any::<u16>() as u64;
    c.band_max_pin_slots = kani::any::<u16>() as u64;
    c.band_max_positions_per_side = kani::any::<u16>() as u64;
    c.band_min_leg_notional = kani::any::<u16>() as u64;
    let ok = c.validate_public_user_fund().is_ok();
    if ok {
        assert!(c.band_max_epoch_slots == 0 && c.band_max_pin_slots == 0);
        assert!(c.band_max_positions_per_side == 0 && c.band_min_leg_notional == 0);
    }
    kani::cover!(ok, "clean band-off config accepted");
    kani::cover!(!ok && c.band_min_leg_notional != 0, "stray band field refused");
}

/// E-BND-28b (§7 `kani_band_config_floors`, band on, rev 2 R1.5 / rev 2.1): an accepted band
/// config has `E >= 150`, `Pmax >= 8E`, `cap in 1..=256`, `band_min_leg_notional != 0`,
/// single asset (`:5297-5313`). Cost L / high risk (the Ok path runs the BSL validator).
#[kani::proof]
#[kani::unwind(4100)]
#[kani::solver(cadical)]
fn kani_band_config_floors_band_on() {
    let mut c = band_cfg(BAND_D);
    c.band_max_epoch_slots = kani::any::<u16>() as u64;
    c.band_max_pin_slots = kani::any::<u16>() as u64;
    c.band_max_positions_per_side = kani::any::<u16>() as u64;
    c.band_min_leg_notional = kani::any::<u8>() as u64;
    let ok = c.validate_public_user_fund().is_ok();
    if ok {
        assert!(c.band_max_epoch_slots >= BAND_MIN_EPOCH_SLOTS);
        assert!(c.band_max_pin_slots >= c.band_max_epoch_slots * BAND_MIN_PIN_EPOCHS);
        assert!(c.band_max_positions_per_side >= 1 && c.band_max_positions_per_side <= BAND_MAX_POSITIONS_PER_SIDE);
        assert!(c.band_min_leg_notional != 0);
        assert!(c.max_market_slots == 1 && c.max_portfolio_assets == 1);
    }
    kani::cover!(ok, "a floor-respecting band config accepted");
    kani::cover!(!ok && c.band_max_epoch_slots < BAND_MIN_EPOCH_SLOTS, "E below the floor refused");
    kani::cover!(!ok && c.band_min_leg_notional == 0, "zero minimum notional refused");
}

/// Two accounts, each with one leg on asset 0 (built by the real `kernel_attach_leg`), in a
/// band market; `stored` counts as poked. Returns the long and short legs' units.
fn trade_shape_fixture(
    long_units: u8,
    short_units: u8,
) -> (MarketGroupV16HeaderAccount, [Market<u64>; 1], PortfolioAccountV16Account, PortfolioAccountV16Account) {
    let (mut header, mut markets) = market_fixture(band_cfg(BAND_D), P0);
    let mut a = asset_of(&markets);
    let mut la = account_fixture(2);
    let mut sa = account_fixture(3);
    if long_units != 0 {
        let (a2, leg) = a
            .kani_bnd_attach_leg(SideV16::Long, long_units as i128 * POS_SCALE as i128, long_units as u128 * POS_SCALE, 0, BAND_D, BAND_MAX_POSITIONS_PER_SIDE)
            .unwrap();
        a = a2;
        la.legs[0] = PortfolioLegV16Account::from_runtime(&leg);
        let mut b = la.active_bitmap.map(V16PodU64::get);
        active_bitmap_set(&mut b, 0).unwrap();
        la.active_bitmap = b.map(V16PodU64::new);
    }
    if short_units != 0 {
        let (a2, leg) = a
            .kani_bnd_attach_leg(SideV16::Short, -(short_units as i128) * POS_SCALE as i128, short_units as u128 * POS_SCALE, 0, BAND_D, BAND_MAX_POSITIONS_PER_SIDE)
            .unwrap();
        a = a2;
        sa.legs[0] = PortfolioLegV16Account::from_runtime(&leg);
        let mut b = sa.active_bitmap.map(V16PodU64::get);
        active_bitmap_set(&mut b, 0).unwrap();
        sa.active_bitmap = b.map(V16PodU64::new);
    }
    put_asset(&mut markets, &a);
    header.config.band_min_leg_notional = V16PodU64::new(0);
    (header, markets, la, sa)
}

fn any_side_opt() -> Option<SideV16> {
    match kani::any::<u8>() % 3 {
        0 => None,
        1 => Some(SideV16::Long),
        _ => Some(SideV16::Short),
    }
}

/// E-BND-29 / E-BND-30 (§8 `kani_band_trade_shape_cap`, `kani_band_min_notional_exempt`).
/// `require_band_trade_shape` `:18387` (shim). On `Ok`, every NON-exempt account's leg has
/// notional >= `band_min_leg_notional`, and a newly attached side has count <= cap; an exempt
/// account is never checked. Mutant: BND-C2 (end-of-trade cap check removed).
#[kani::proof]
#[kani::unwind(18)]
#[kani::solver(cadical)]
fn kani_band_trade_shape_cap_and_min_notional_exempt() {
    let lu = kani::any::<u8>() % 4;
    let su = kani::any::<u8>() % 4;
    let (mut header, mut markets, mut la, mut sa) = trade_shape_fixture(lu, su);
    let min = kani::any::<u32>() as u64;
    let cap = 1 + (kani::any::<u8>() as u64 % 4);
    header.config.band_min_leg_notional = V16PodU64::new(min);
    header.config.band_max_positions_per_side = V16PodU64::new(cap);
    let mut a = asset_of(&markets);
    a.stored_pos_count_long = kani::any::<u8>() as u64 % 8;
    a.stored_pos_count_short = kani::any::<u8>() as u64 % 8;
    put_asset(&mut markets, &a);
    let exempt_long: bool = kani::any();
    let exempt_short: bool = kani::any();
    let before = [(any_side_opt(), any_side_opt())];
    let req = [TradeRequestV16 { asset_index: 0, size_q: 0, exec_price: P0, fee_bps: 0 }];
    let m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let lv = PortfolioV16ViewMut::new(&mut la);
    let sv = PortfolioV16ViewMut::new(&mut sa);
    let r = m.kani_bnd_require_band_trade_shape(&lv, &sv, &req, &before, exempt_long, exempt_short);
    let notional = |units: u8| units as u128 * POS_SCALE * P0 as u128 / POS_SCALE;
    if r.is_ok() {
        if !exempt_long && lu != 0 {
            assert!(notional(lu) >= min as u128);
            if before[0].0 != Some(SideV16::Long) {
                assert!(a.stored_pos_count_long <= cap);
            }
        }
        if !exempt_short && su != 0 {
            assert!(notional(su) >= min as u128);
            if before[0].1 != Some(SideV16::Short) {
                assert!(a.stored_pos_count_short <= cap);
            }
        }
    }
    if exempt_long && exempt_short {
        assert!(r.is_ok(), "exempt accounts are never checked");
    }
    kani::cover!(r.is_ok() && !exempt_long && lu != 0, "non-exempt leg checked and passes");
    kani::cover!(r == Err(V16Error::BandLegBelowMinNotional), "below-min refused");
    kani::cover!(r == Err(V16Error::BandPositionCap), "end-of-trade cap refused");
    kani::cover!(r.is_ok() && exempt_short && su != 0 && notional(su) < min as u128, "exempt maker below the minimum passes");
}

/// E-BND-31 (§8 `kani_band_bilateral_close_preserves_a`): a full close of a taker leg against
/// the (exempt) maker through the real `execute_trade_band_maker_exempt_not_atomic` `:22044`
/// leaves `a_long`, `a_short` unchanged and both OI sides back where they were. Cost L / high
/// risk (whole trade path); memory-heavy, run under the RSS cap.
#[kani::proof]
#[kani::unwind(33)]
#[kani::solver(cadical)]
fn kani_band_bilateral_close_preserves_a() {
    let (mut header, mut markets) = market_fixture(band_cfg(BAND_D), P0);
    let mut taker = account_fixture(2);
    let mut lp = account_fixture(3);
    {
        let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut tv = PortfolioV16ViewMut::new(&mut taker);
        let mut lv = PortfolioV16ViewMut::new(&mut lp);
        m.deposit_not_atomic(&mut tv, 1_000_000_000_000).unwrap();
        m.deposit_not_atomic(&mut lv, 1_000_000_000_000).unwrap();
    }
    let a0 = asset_of(&markets);
    let units = 1 + (kani::any::<u8>() % 3) as i128;
    let q = units * POS_SCALE as i128;
    let open = {
        let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut tv = PortfolioV16ViewMut::new(&mut taker);
        let mut lv = PortfolioV16ViewMut::new(&mut lp);
        m.execute_trade_band_maker_exempt_not_atomic(
            &mut tv,
            &mut lv,
            TradeRequestV16 { asset_index: 0, size_q: q, exec_price: P0, fee_bps: 0 },
            true,
        )
    };
    kani::assume(open.is_ok());
    kani::cover!(true, "open reached");
    let close = {
        let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut tv = PortfolioV16ViewMut::new(&mut taker);
        let mut lv = PortfolioV16ViewMut::new(&mut lp);
        // the LP is long the close: taker sells its leg back
        m.execute_trade_band_maker_exempt_not_atomic(
            &mut lv,
            &mut tv,
            TradeRequestV16 { asset_index: 0, size_q: q, exec_price: P0, fee_bps: 0 },
            false,
        )
    };
    if close.is_ok() {
        let a1 = asset_of(&markets);
        assert_eq!(a1.a_long, a0.a_long);
        assert_eq!(a1.a_short, a0.a_short);
        assert_eq!(a1.oi_eff_long_q, a0.oi_eff_long_q);
        assert_eq!(a1.oi_eff_short_q, a0.oi_eff_short_q);
        kani::cover!(true, "bilateral close completed");
    }
}
