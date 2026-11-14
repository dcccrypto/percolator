//! Edge-case tests for the v2.2 band + rent primitives (src/band_rent.rs) and the
//! engine paths that use them. Tests only. Complements tests/v22_band_rent.rs:
//! this file does not repeat its cases, it covers boundaries, zero and max inputs,
//! carry behaviour, and rent on legs that change or close mid-period.

#![allow(dead_code)]

use percolator::band_rent::{
    band_bounds, band_d1_pinned_step, band_duration_pinned, band_genesis_price_ok,
    band_min_wide_anchor, band_width_ok, band_worst_adverse_bps, clamp_to_band, price_in_band,
    rent_chargeable_atoms, rent_due_atoms, rent_due_with_carry, rent_index_delta, BandRentError,
    MAX_BAND_BPS, MAX_RENT_E9_PER_SLOT, MIN_BAND_WIDTH_TICKS, RENT_INDEX_DEN,
};
use percolator::{MAX_ORACLE_PRICE, POS_SCALE};

// ---------------------------------------------------------------------------
// band_bounds / price_in_band / clamp_to_band boundaries
// ---------------------------------------------------------------------------

#[test]
fn bounds_reject_out_of_domain_inputs() {
    assert_eq!(band_bounds(0, 100), Err(BandRentError::InvalidInput));
    assert_eq!(
        band_bounds(MAX_ORACLE_PRICE + 1, 100),
        Err(BandRentError::InvalidInput)
    );
    assert_eq!(band_bounds(1000, 0), Err(BandRentError::InvalidInput));
    assert_eq!(
        band_bounds(1000, MAX_BAND_BPS + 1),
        Err(BandRentError::InvalidInput)
    );
    assert!(band_bounds(1, 1).is_ok());
    assert!(band_bounds(MAX_ORACLE_PRICE, MAX_BAND_BPS).is_ok());
}

#[test]
fn bounds_at_anchor_one_collapse_to_the_anchor() {
    // lo = ceil(1 * (10000-d)/10000) = 1, hi = floor(1 * (10000+d)/10000) = 1 for d < 10000.
    for d in [1, 130, 1_000, MAX_BAND_BPS] {
        assert_eq!(band_bounds(1, d), Ok((1, 1)), "d = {d}");
    }
}

#[test]
fn bounds_hi_is_capped_at_max_oracle_price() {
    let (lo, hi) = band_bounds(MAX_ORACLE_PRICE, 500).unwrap();
    assert_eq!(hi, MAX_ORACLE_PRICE);
    assert_eq!(lo, MAX_ORACLE_PRICE / 10_000 * 9_500);
    // Just under the cap the real band still reaches past MAX: hi must stay clamped.
    let (_, hi2) = band_bounds(MAX_ORACLE_PRICE - 1, 500).unwrap();
    assert_eq!(hi2, MAX_ORACLE_PRICE);
}

#[test]
fn bounds_exact_multiples_have_no_rounding() {
    // 1_000_000 at 130 bps: 987_000 .. 1_013_000 exactly.
    assert_eq!(band_bounds(1_000_000, 130), Ok((987_000, 1_013_000)));
    // 10_000 at 2000 bps: 8_000 .. 12_000.
    assert_eq!(band_bounds(10_000, 2_000), Ok((8_000, 12_000)));
}

#[test]
fn bounds_round_inward_by_one_tick_off_multiples() {
    // 101 at 130 bps: lo = ceil(101*9870/10000)= ceil(99.687)=100, hi = floor(101*10130/10000)=floor(102.313)=102.
    assert_eq!(band_bounds(101, 130), Ok((100, 102)));
}

#[test]
fn bounds_sweep_keeps_ordering_and_containment() {
    for d in [1u64, 2, 129, 130, 999, 2_000] {
        for a in (1u64..=300).chain([10_000, 999_999, 1_000_000, MAX_ORACLE_PRICE - 1, MAX_ORACLE_PRICE]) {
            let (lo, hi) = band_bounds(a, d).unwrap();
            assert!(1 <= lo && lo <= a && a <= hi && hi <= MAX_ORACLE_PRICE, "a={a} d={d}");
        }
    }
}

#[test]
fn price_in_band_is_inclusive_at_both_edges() {
    let (lo, hi) = band_bounds(1_000_000, 130).unwrap();
    assert!(price_in_band(lo, 1_000_000, 130).unwrap());
    assert!(price_in_band(hi, 1_000_000, 130).unwrap());
    assert!(!price_in_band(lo - 1, 1_000_000, 130).unwrap());
    assert!(!price_in_band(hi + 1, 1_000_000, 130).unwrap());
    assert!(!price_in_band(0, 1_000_000, 130).unwrap());
    assert!(!price_in_band(u64::MAX, 1_000_000, 130).unwrap());
    assert!(price_in_band(1_000_000, 1_000_000, 130).unwrap());
}

#[test]
fn clamp_is_identity_inside_and_snaps_to_nearest_edge_outside() {
    let (lo, hi) = band_bounds(1_000_000, 130).unwrap();
    assert_eq!(clamp_to_band(1_000_000, 1_000_000, 130), Ok(1_000_000));
    assert_eq!(clamp_to_band(lo, 1_000_000, 130), Ok(lo));
    assert_eq!(clamp_to_band(hi, 1_000_000, 130), Ok(hi));
    assert_eq!(clamp_to_band(lo - 1, 1_000_000, 130), Ok(lo));
    assert_eq!(clamp_to_band(hi + 1, 1_000_000, 130), Ok(hi));
    assert_eq!(clamp_to_band(0, 1_000_000, 130), Ok(lo));
    assert_eq!(clamp_to_band(u64::MAX, 1_000_000, 130), Ok(hi));
    assert_eq!(clamp_to_band(5, 0, 130), Err(BandRentError::InvalidInput));
}

// ---------------------------------------------------------------------------
// worst adverse move, width, genesis
// ---------------------------------------------------------------------------

#[test]
fn worst_adverse_rejects_bad_d_and_is_strictly_above_two_d() {
    assert_eq!(band_worst_adverse_bps(0), Err(BandRentError::InvalidInput));
    assert_eq!(
        band_worst_adverse_bps(MAX_BAND_BPS + 1),
        Err(BandRentError::InvalidInput)
    );
    let mut prev = 0;
    for d in 1..=MAX_BAND_BPS {
        let g = band_worst_adverse_bps(d).unwrap();
        assert!(g >= 2 * d, "G must cover two band widths at d={d}");
        assert!(g >= prev, "G is monotone in d at d={d}");
        prev = g;
    }
}

#[test]
fn width_ok_flips_exactly_at_the_documented_threshold() {
    // Width is monotone-ish in anchor; find first ok anchor by scan and check both sides.
    let d = 130;
    let mut first_ok = None;
    for a in 1..2_000u64 {
        if band_width_ok(a, d).unwrap() {
            first_ok = Some(a);
            break;
        }
    }
    let a = first_ok.expect("some anchor below 2000 is wide enough at d=130");
    let (lo, hi) = band_bounds(a, d).unwrap();
    assert!(hi - lo >= MIN_BAND_WIDTH_TICKS);
    let (lo1, hi1) = band_bounds(a - 1, d).unwrap();
    assert!(hi1 - lo1 < MIN_BAND_WIDTH_TICKS);
    assert_eq!(band_width_ok(0, d), Err(BandRentError::InvalidInput));
}

#[test]
fn min_wide_anchor_has_a_nine_long_wide_run_and_rejects_bad_d() {
    assert_eq!(band_min_wide_anchor(0), Err(BandRentError::InvalidInput));
    assert_eq!(
        band_min_wide_anchor(MAX_BAND_BPS + 1),
        Err(BandRentError::InvalidInput)
    );
    for d in [1u64, 10, 50, 130, 500, 1_000, MAX_BAND_BPS] {
        let a = band_min_wide_anchor(d).unwrap().expect("exists below the oracle cap");
        for k in 0..=8 {
            assert!(band_width_ok(a + k, d).unwrap(), "d={d} a+{k}");
        }
    }
}

#[test]
fn genesis_price_rule_is_100x_the_min_wide_anchor() {
    for d in [130u64, 1_000] {
        let min = band_min_wide_anchor(d).unwrap().unwrap();
        assert!(band_genesis_price_ok(min * 100, d).unwrap());
        assert!(!band_genesis_price_ok(min * 100 - 1, d).unwrap());
        assert!(!band_genesis_price_ok(0, d).unwrap());
    }
    assert_eq!(band_genesis_price_ok(1_000_000, 0), Err(BandRentError::InvalidInput));
}

// ---------------------------------------------------------------------------
// duration pin / D-1 step
// ---------------------------------------------------------------------------

#[test]
fn duration_pin_boundary_is_strictly_after_the_window() {
    assert!(!band_duration_pinned(1_600, 1_000, 600), "end == anchor + E is inside");
    assert!(band_duration_pinned(1_601, 1_000, 600), "one slot later is pinned");
    assert!(!band_duration_pinned(1_000, 1_000, 600));
    assert!(!band_duration_pinned(5, 1_000, 600), "segment before anchor is not pinned");
    assert!(band_duration_pinned(u64::MAX, 0, u64::MAX - 1));
    assert!(!band_duration_pinned(u64::MAX, 0, u64::MAX));
    // E = 0 degenerates to "pinned the moment the clock moves past the anchor".
    assert!(!band_duration_pinned(1_000, 1_000, 0));
    assert!(band_duration_pinned(1_001, 1_000, 0));
}

#[test]
fn d1_step_pinned_always_feeds_the_current_price() {
    assert_eq!(band_d1_pinned_step(1_000_000, 1_000_000, 1_000_000, 130, true), Ok((1_000_000, false)));
    assert_eq!(band_d1_pinned_step(1_000_000, 1_005_000, 1_000_000, 130, true), Ok((1_000_000, true)));
    // Even a capped step far outside the band is ignored while the duration pin holds.
    assert_eq!(band_d1_pinned_step(1_000_000, 1, 1_000_000, 130, true), Ok((1_000_000, true)));
}

#[test]
fn d1_step_unpinned_clamps_and_flags_only_when_it_moved_the_step() {
    let (lo, hi) = band_bounds(1_000_000, 130).unwrap();
    assert_eq!(band_d1_pinned_step(1_000_000, hi, 1_000_000, 130, false), Ok((hi, false)));
    assert_eq!(band_d1_pinned_step(1_000_000, hi + 1, 1_000_000, 130, false), Ok((hi, true)));
    assert_eq!(band_d1_pinned_step(1_000_000, lo, 1_000_000, 130, false), Ok((lo, false)));
    assert_eq!(band_d1_pinned_step(1_000_000, lo - 1, 1_000_000, 130, false), Ok((lo, true)));
    assert_eq!(
        band_d1_pinned_step(1_000_000, 1_000_000, 0, 130, false),
        Err(BandRentError::InvalidInput)
    );
}

// ---------------------------------------------------------------------------
// rent_index_delta
// ---------------------------------------------------------------------------

#[test]
fn index_delta_is_zero_when_any_factor_is_zero() {
    assert_eq!(rent_index_delta(0, 23, 100), Ok(0));
    assert_eq!(rent_index_delta(1_000_000, 0, 100), Ok(0));
    assert_eq!(rent_index_delta(1_000_000, 23, 0), Ok(0));
    assert_eq!(rent_index_delta(0, 0, 0), Ok(0));
}

#[test]
fn index_delta_fits_at_the_documented_maxima() {
    // Largest price x largest sanity rate x a whole u64 of slots stays under u128::MAX.
    let d = rent_index_delta(MAX_ORACLE_PRICE, MAX_RENT_E9_PER_SLOT, u64::MAX).unwrap();
    assert_eq!(
        d,
        MAX_ORACLE_PRICE as u128 * MAX_RENT_E9_PER_SLOT as u128 * u64::MAX as u128
    );
}

#[test]
fn index_delta_overflow_fails_closed() {
    assert_eq!(
        rent_index_delta(u64::MAX, u64::MAX, u64::MAX),
        Err(BandRentError::Overflow)
    );
    // (2^64-1)^2 * 2 is just over 2^128.
    assert_eq!(
        rent_index_delta(u64::MAX, u64::MAX, 2),
        Err(BandRentError::Overflow)
    );
    // ... and with dt = 1 it still fits.
    assert_eq!(
        rent_index_delta(u64::MAX, u64::MAX, 1),
        Ok(u64::MAX as u128 * u64::MAX as u128)
    );
}

// ---------------------------------------------------------------------------
// rent_due_with_carry / rent_due_atoms
// ---------------------------------------------------------------------------

#[test]
fn due_zero_size_charges_nothing_and_keeps_the_carry() {
    let carry = (RENT_INDEX_DEN - 1) as u64;
    assert_eq!(rent_due_with_carry(0, 10_000, 0, carry), Ok((0, carry)));
    assert_eq!(rent_due_with_carry(0, u128::MAX, 0, 0), Ok((0, 0)));
}

#[test]
fn due_with_unchanged_index_charges_nothing_and_keeps_the_carry() {
    let carry = 12_345u64;
    assert_eq!(rent_due_with_carry(10 * POS_SCALE, 777, 777, carry), Ok((0, carry)));
}

#[test]
fn due_rejects_snapshot_ahead_of_index_and_oversized_carry() {
    assert_eq!(rent_due_with_carry(1, 5, 6, 0), Err(BandRentError::InvalidInput));
    assert_eq!(
        rent_due_with_carry(1, 5, 0, RENT_INDEX_DEN as u64),
        Err(BandRentError::InvalidInput)
    );
    // The largest legal carry is accepted.
    assert!(rent_due_with_carry(1, 5, 0, (RENT_INDEX_DEN - 1) as u64).is_ok());
}

#[test]
fn due_carry_rolls_into_one_atom_exactly_at_the_denominator() {
    // total = q*delta + carry hits DEN exactly: one atom, carry back to 0.
    let carry = (RENT_INDEX_DEN - 1) as u64;
    assert_eq!(rent_due_with_carry(1, 1, 0, carry), Ok((1, 0)));
    // One short of DEN: no atom yet.
    assert_eq!(rent_due_with_carry(1, 0, 0, carry), Ok((0, carry)));
    assert_eq!(rent_due_with_carry(1, 1, 0, carry - 1), Ok((0, carry)));
}

#[test]
fn due_one_whole_atom_for_one_whole_unit_and_den_of_index() {
    // q = POS_SCALE, delta = 1e9 -> exactly one atom.
    assert_eq!(rent_due_atoms(POS_SCALE, 1_000_000_000, 0), Ok(1));
    assert_eq!(rent_due_atoms(POS_SCALE, 999_999_999, 0), Ok(0));
    assert_eq!(rent_due_atoms(POS_SCALE, 2_000_000_000 - 1, 0), Ok(1));
}

#[test]
fn due_u128_path_and_u256_path_agree_at_the_overflow_seam() {
    // Find a q where q*delta just fits, and one where it just overflows, and compare to an
    // independent floor-division by splitting the product.
    let delta: u128 = 1u128 << 70;
    let q_fit = u128::MAX / delta; // q*delta <= MAX
    let q_over = q_fit + 1; // q*delta > MAX: takes the wide path
    for q in [q_fit, q_over] {
        let (due, carry) = rent_due_with_carry(q, delta, 0, 0).unwrap();
        // Check invariant: due*DEN + carry == q*delta, using u128 only on the smaller side.
        // q*delta = q * 2^70. due*DEN = floor(q*2^70/DEN)*DEN.
        // Compare via (q >> 0) scaled: q*2^70 / DEN with DEN = POS_SCALE*1e9.
        let lhs_hi = (q >> 58) as u128; // q*2^70 = (q>>58)*2^128 + (q & (2^58-1))*2^70
        let lhs_lo = (q & ((1u128 << 58) - 1)) << 70;
        // due*DEN + carry as (hi, lo) pair
        let (lo, c1) = due.overflowing_mul(RENT_INDEX_DEN);
        let hi_from_mul = {
            // high part of due * DEN
            let d_hi = due >> 64;
            let d_lo = due & u64::MAX as u128;
            let den_hi = RENT_INDEX_DEN >> 64;
            let den_lo = RENT_INDEX_DEN & u64::MAX as u128;
            // (d_hi*2^64 + d_lo)*(den_hi*2^64 + den_lo)
            let ll = d_lo * den_lo;
            let lh = d_lo * den_hi;
            let hl = d_hi * den_lo;
            let hh = d_hi * den_hi;
            let mid = (ll >> 64) + (lh & u64::MAX as u128) + (hl & u64::MAX as u128);
            hh + (lh >> 64) + (hl >> 64) + (mid >> 64)
        };
        let _ = c1;
        let (lo2, c2) = lo.overflowing_add(carry as u128);
        assert_eq!((hi_from_mul + c2 as u128, lo2), (lhs_hi, lhs_lo), "q={q}");
        assert!((carry as u128) < RENT_INDEX_DEN);
    }
}

#[test]
fn due_saturates_instead_of_wrapping_for_absurd_sizes() {
    let (due, carry) = rent_due_with_carry(u128::MAX, u128::MAX, 0, 0).unwrap();
    assert_eq!(due, u128::MAX, "due above u128::MAX saturates");
    assert!((carry as u128) < RENT_INDEX_DEN);
}

#[test]
fn due_split_settles_equal_one_settle_when_carry_is_threaded() {
    // Deterministic schedule over awkward sizes: n settles with carry == one settle.
    for &q in &[1u128, 3, POS_SCALE - 1, POS_SCALE, 7 * POS_SCALE + 5, 1u128 << 90] {
        for &step in &[1u128, 7, 999_983, 1_000_000_007] {
            let mut idx = 0u128;
            let mut snap = 0u128;
            let mut carry = 0u64;
            let mut total = 0u128;
            for _ in 0..50 {
                idx += step * 3;
                let (due, c) = rent_due_with_carry(q, idx, snap, carry).unwrap();
                snap = idx;
                carry = c;
                total += due;
            }
            let one = rent_due_with_carry(q, idx, 0, 0).unwrap();
            assert_eq!((total, carry), one, "q={q} step={step}");
        }
    }
}

#[test]
fn due_without_carry_never_exceeds_exact_rent() {
    // Splitting a settle with no carry never overcharges relative to one settle.
    let q = 5 * POS_SCALE + 3;
    let whole = rent_due_atoms(q, 10_000_000_000, 0).unwrap();
    let mut idx = 0;
    let mut sum = 0;
    for _ in 0..100 {
        let next = idx + 100_000_000;
        sum += rent_due_atoms(q, next, idx).unwrap();
        idx = next;
    }
    assert!(sum <= whole);
}

#[test]
fn due_is_monotone_in_size_and_index() {
    let mut last = 0;
    for k in 0..200u128 {
        let d = rent_due_atoms(k * POS_SCALE / 3, 4_000_000_000, 0).unwrap();
        assert!(d >= last);
        last = d;
    }
    let mut last = 0;
    for k in 0..200u128 {
        let d = rent_due_atoms(3 * POS_SCALE, k * 777_777_777, 0).unwrap();
        assert!(d >= last);
        last = d;
    }
}

// ---------------------------------------------------------------------------
// rent_chargeable_atoms
// ---------------------------------------------------------------------------

#[test]
fn chargeable_is_min_of_due_and_loss_free_capital() {
    assert_eq!(rent_chargeable_atoms(10, 100, 0), 10);
    assert_eq!(rent_chargeable_atoms(100, 10, 0), 10);
    assert_eq!(rent_chargeable_atoms(10, 100, 50), 10, "positive pnl is not capital");
    assert_eq!(rent_chargeable_atoms(10, 100, -95), 5);
    assert_eq!(rent_chargeable_atoms(10, 100, -100), 0);
    assert_eq!(rent_chargeable_atoms(10, 100, -101), 0, "loss beyond capital: nothing");
    assert_eq!(rent_chargeable_atoms(0, 100, -5), 0);
    assert_eq!(rent_chargeable_atoms(10, 0, 0), 0);
}

#[test]
fn chargeable_handles_extreme_pnl_and_capital() {
    assert_eq!(rent_chargeable_atoms(u128::MAX, u128::MAX, 0), u128::MAX);
    assert_eq!(rent_chargeable_atoms(u128::MAX, u128::MAX, i128::MIN), u128::MAX - (1u128 << 127));
    assert_eq!(rent_chargeable_atoms(5, 1u128 << 127, i128::MIN), 0);
    assert_eq!(rent_chargeable_atoms(5, 10, i128::MAX), 5);
}

#[test]
fn chargeable_never_exceeds_due_nor_capital_nor_leaves_loss_uncovered() {
    for due in [0u128, 1, 7, 100, 10_000] {
        for cap in [0u128, 1, 7, 100, 10_000] {
            for pnl in [-20_000i128, -100, -7, -1, 0, 1, 100] {
                let c = rent_chargeable_atoms(due, cap, pnl);
                assert!(c <= due && c <= cap);
                if pnl < 0 {
                    assert!(cap - c >= pnl.unsigned_abs().min(cap), "loss-owned capital kept");
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Engine fixtures (copied from tests/v22_band_rent.rs so this file stands alone)
// ---------------------------------------------------------------------------

use percolator::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, TradeRequestV16, V16Config, V16Error,
};

const P0: u64 = 1_000_000;
const MARKET_ID: [u8; 32] = [7; 32];
const OWNER: [u8; 32] = [9; 32];

/// A 10x band market: MMR 5%, IMR 10%, liquidation fee 50 bps, funding <= 111 e9,
/// rent <= 23 e9 (~50 bps/day), cap 100 bps/slot over <= 3 slots, d = 130 bps,
/// E = 600 slots, Pmax = 9,000 slots (the design's 10x preset).
fn band_cfg(band_bps: u64) -> V16Config {
    let mut c = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    c.maintenance_margin_bps = 500;
    c.initial_margin_bps = 1_000;
    // Floors large enough that integer rounding of loss + fee cannot exceed the
    // maintenance requirement at small notionals (the exact §1.6 law checks it).
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
    // Counterparty backing stays fresh for the whole test (freshness horizon =
    // max(max_accrual_dt, h_max, max_bankrupt_close_lifetime)); with the 10-slot
    // default a winner's claim lapses and its refresh reports
    // SourceBackingExpired every time, so it could never be certified.
    c.max_bankrupt_close_lifetime_slots = 1_000_000;
    c.band_bps = band_bps;
    if band_bps != 0 {
        c.band_max_epoch_slots = 600;
        c.band_max_pin_slots = 9_000;
        c.band_max_positions_per_side = percolator::band_rent::BAND_MAX_POSITIONS_PER_SIDE;
        // Engine minimum (the wrapper floors it at whole tokens): fixture trades stay valid.
        c.band_min_leg_notional = 1;
    }
    c
}

struct World {
    header: MarketGroupV16HeaderAccount,
    markets: Vec<Market<u64>>,
    accounts: Vec<PortfolioAccountV16Account>,
    now: u64,
}

impl World {
    fn new(cfg: V16Config, n_accounts: usize, deposit: u128) -> Self {
        Self::new_at(cfg, n_accounts, deposit, P0)
    }

    /// A World launched at `P0` whose (still empty) asset is then moved to `price` by a state
    /// poke, bypassing the N-2 genesis rule: for tests that need a book near the width floor.
    fn new_poked_at(cfg: V16Config, n_accounts: usize, deposit: u128, price: u64) -> Self {
        let mut w = Self::new(cfg, n_accounts, deposit);
        let mut a = w.asset();
        a.effective_price = price;
        a.raw_oracle_target_price = price;
        a.fund_px_last = price;
        a.band_anchor_price = price;
        w.markets[0].engine.asset = percolator::AssetStateV16Account::from_runtime(&a);
        w
    }

    fn new_at(cfg: V16Config, n_accounts: usize, deposit: u128, price: u64) -> Self {
        cfg.validate_public_user_fund()
            .expect("fixture config validates");
        let mut header = MarketGroupV16HeaderAccount::new_dynamic(MARKET_ID, cfg, 1, 0).unwrap();
        let mut markets = vec![Market::new(0, EngineAssetSlotV16Account::default())];
        header
            .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, price, 1)
            .unwrap();
        let mut accounts = Vec::new();
        for i in 0..n_accounts {
            let prov = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
                MARKET_ID,
                {
                    // > 255 accounts (re-review cap tests): two key bytes.
                    let mut k = [1u8; 32];
                    k[0] = (i % 256) as u8;
                    k[1] = (i / 256) as u8 + 1;
                    k
                },
                OWNER,
            ));
            let mut a = PortfolioAccountV16Account::default();
            a.init_empty_in_place(prov).unwrap();
            accounts.push(a);
        }
        let mut w = Self {
            header,
            markets,
            accounts,
            now: 1,
        };
        for i in 0..n_accounts {
            w.with(i, |m, a| m.deposit_not_atomic(a, deposit)).unwrap();
        }
        w
    }

    fn with<R>(
        &mut self,
        i: usize,
        f: impl FnOnce(&mut MarketGroupV16ViewMut<'_, u64>, &mut PortfolioV16ViewMut<'_>) -> R,
    ) -> R {
        let mut m = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let mut a = PortfolioV16ViewMut::new(&mut self.accounts[i]);
        f(&mut m, &mut a)
    }

    fn market<R>(&mut self, f: impl FnOnce(&mut MarketGroupV16ViewMut<'_, u64>) -> R) -> R {
        let mut m = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        f(&mut m)
    }

    fn asset(&self) -> percolator::AssetStateV16 {
        self.markets[0].engine.asset.try_to_runtime().unwrap()
    }

    /// A fill whose MAKER (the non-taker account) is exempt from the band minimum-leg check:
    /// the engine entry the wrapper uses for matcher-LP fills, the dust sweep and eviction.
    fn trade_maker_exempt(
        &mut self,
        long: usize,
        short: usize,
        q: u128,
        taker_is_long: bool,
    ) -> Result<(), V16Error> {
        let price = self.asset().effective_price;
        let (lo, hi) = if long < short {
            (long, short)
        } else {
            (short, long)
        };
        let (left, right) = self.accounts.split_at_mut(hi);
        let (a_lo, a_hi) = (&mut left[lo], &mut right[0]);
        let (l, s) = if long < short {
            (a_lo, a_hi)
        } else {
            (a_hi, a_lo)
        };
        let mut m = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let mut lv = PortfolioV16ViewMut::new(l);
        let mut sv = PortfolioV16ViewMut::new(s);
        m.execute_trade_band_maker_exempt_not_atomic(
            &mut lv,
            &mut sv,
            TradeRequestV16 {
                asset_index: 0,
                size_q: q as i128,
                exec_price: price,
                fee_bps: 0,
            },
            taker_is_long,
        )
        .map(|_| ())
    }

    fn trade(&mut self, long: usize, short: usize, q: u128) -> Result<(), V16Error> {
        assert_ne!(long, short);
        let price = self.asset().effective_price;
        let (lo, hi) = if long < short {
            (long, short)
        } else {
            (short, long)
        };
        let (left, right) = self.accounts.split_at_mut(hi);
        let (a_lo, a_hi) = (&mut left[lo], &mut right[0]);
        let (l, s) = if long < short {
            (a_lo, a_hi)
        } else {
            (a_hi, a_lo)
        };
        let mut m = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let mut lv = PortfolioV16ViewMut::new(l);
        let mut sv = PortfolioV16ViewMut::new(s);
        m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut lv,
            &mut sv,
            TradeRequestV16 {
                asset_index: 0,
                size_q: q as i128,
                exec_price: price,
                fee_bps: 0,
            },
            true,
        )
        .map(|_| ())
    }

    /// The wrapper's price recipe for a single-segment accrual to `self.now`:
    /// capped staircase toward the target, then the band (edge clamp / duration
    /// pin) against the anchor the engine will use after any re-anchor.
    fn wrapper_price(&self) -> (u64, bool) {
        let a = self.asset();
        let cfg = self.header.config.try_to_runtime_shape().unwrap();
        let dt = (self.now - a.slot_last).min(cfg.max_accrual_dt_slots);
        let target = a.raw_oracle_target_price;
        let p = a.effective_price;
        let exposed = a.oi_eff_long_q != 0 || a.oi_eff_short_q != 0;
        let capped = if !exposed {
            target
        } else {
            let max_delta =
                (p as u128 * cfg.max_price_move_bps_per_slot as u128 * dt as u128 / 10_000) as u64;
            if target > p {
                p + max_delta.min(target - p)
            } else {
                p - max_delta.min(p - target)
            }
        };
        let mut header = self.header;
        let mut markets = self.markets.clone();
        let m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        match m.band_accrual_preview(0, a.slot_last + dt).unwrap() {
            None => (capped, false),
            Some((anchor, d, governing, e)) => {
                let pinned = band_duration_pinned(a.slot_last + dt, governing, e);
                let (price, _) = band_d1_pinned_step(p, capped, anchor, d, pinned).unwrap();
                (price, pinned)
            }
        }
    }

    fn accrue(&mut self, funding: i128, rent_long: u64, rent_short: u64) -> Result<(), V16Error> {
        let (price, pinned) = self.wrapper_price();
        let (f, rl, rs) = if pinned {
            (0, 0, 0)
        } else {
            (funding, rent_long, rent_short)
        };
        let now = self.now;
        self.market(|m| m.accrue_asset_to_with_rent_not_atomic(0, now, price, f, rl, rs, true))
            .map(|_| ())
    }

    fn set_target(&mut self, target: u64) {
        self.market(|m| m.set_asset_raw_oracle_target_not_atomic(0, target))
            .unwrap();
    }

    /// A keeper refresh. `Stale` from a full refresh is the engine reporting that
    /// it expired one lapsed source-backing bucket (committed progress); the
    /// keeper simply retries, exactly as the auto-crank treats
    /// `SourceBackingExpired` as progress.
    fn refresh(&mut self, i: usize) -> Result<percolator::HealthCertV16, V16Error> {
        let mut last = Err(V16Error::Stale);
        for _ in 0..8 {
            last = self.with(i, |m, a| m.full_account_refresh_not_atomic(a));
            if last != Err(V16Error::Stale) {
                break;
            }
        }
        last
    }

    fn positioned(&self, i: usize) -> bool {
        self.accounts[i].legs[0].try_to_runtime().unwrap().active
    }

    /// I-B2: the band counters equal the leg census over every portfolio.
    fn assert_census(&self) {
        let a = self.asset();
        let (mut unc_l, mut unc_s, mut lp_l, mut lp_s, mut n_l, mut n_s) = (0, 0, 0, 0, 0, 0);
        for acct in &self.accounts {
            let leg = acct.legs[0].try_to_runtime().unwrap();
            if !leg.active {
                continue;
            }
            let long = leg.side == percolator::SideV16::Long;
            if long {
                n_l += 1
            } else {
                n_s += 1
            }
            if a.band_epoch != 0 && leg.band_epoch_snap < a.band_epoch {
                if long {
                    unc_l += 1
                } else {
                    unc_s += 1
                }
            }
            if leg.band_liq_pending {
                if long {
                    lp_l += 1
                } else {
                    lp_s += 1
                }
            }
            assert!(leg.band_epoch_snap <= a.band_epoch);
        }
        assert_eq!(
            (a.stored_pos_count_long, a.stored_pos_count_short),
            (n_l, n_s)
        );
        assert_eq!(
            (a.band_uncertified_long, a.band_uncertified_short),
            (unc_l, unc_s),
            "uncertified cohort census (I-B2)"
        );
        assert_eq!(
            (a.band_liq_pending_long, a.band_liq_pending_short),
            (lp_l, lp_s),
            "liq-pending cohort census (I-B2)"
        );
    }

    fn assert_conservation(&self) {
        let v = self.header.vault.get();
        let c = self.header.c_tot.get();
        let i = self.header.insurance.get();
        assert!(v >= c + i, "V >= C_tot + I violated: {v} < {c} + {i}");
    }
}

fn open_book(w: &mut World, pairs: usize, q: u128) {
    for k in 0..pairs {
        w.trade(2 * k, 2 * k + 1, q).expect("open pair");
    }
}

/// Advance `slots` slots (3 at a time, the config's max dt) accruing rent.
fn run_rent(w: &mut World, slots: u64, rent_long: u64, rent_short: u64) {
    let mut t = 0;
    while t < slots {
        w.now += 3;
        t += 3;
        w.accrue(0, rent_long, rent_short).unwrap();
    }
}

/// Sum of account capital equals C_tot, and V >= C_tot + I.
fn assert_books(w: &World) {
    let sum: u128 = w.accounts.iter().map(|a| a.capital.get()).sum();
    assert_eq!(sum, w.header.c_tot.get(), "sum(capital) == C_tot");
    w.assert_conservation();
}

// ---------------------------------------------------------------------------
// Engine: accrual shape
// ---------------------------------------------------------------------------

#[test]
fn rent_index_uses_the_fed_price_per_segment() {
    let mut w = World::new(band_cfg(130), 2, 10_000_000);
    open_book(&mut w, 1, 10 * POS_SCALE);
    // Move the mark up inside the band; the segment's index uses the price it was fed.
    w.set_target(P0 + P0 / 200); // +50 bps, reachable in one 3-slot step at 100 bps/slot
    let before = w.asset().rent_index_long_num;
    w.now += 3;
    w.accrue(0, 10, 0).unwrap();
    let a = w.asset();
    assert_eq!(
        a.rent_index_long_num - before,
        a.effective_price as u128 * 10 * 3
    );
}

#[test]
fn rent_rates_apply_per_side_and_only_where_there_is_open_interest() {
    let mut w = World::new(band_cfg(130), 2, 10_000_000);
    open_book(&mut w, 1, 10 * POS_SCALE);
    w.now += 3;
    w.accrue(0, 7, 11).unwrap();
    let a = w.asset();
    assert_eq!(a.rent_index_long_num, P0 as u128 * 7 * 3);
    assert_eq!(a.rent_index_short_num, P0 as u128 * 11 * 3);
}

#[test]
fn rent_accrual_with_empty_book_leaves_the_indices_alone() {
    let mut w = World::new(band_cfg(130), 2, 10_000_000);
    w.now += 3;
    w.accrue(0, 23, 23).unwrap();
    let a = w.asset();
    assert_eq!((a.rent_index_long_num, a.rent_index_short_num), (0, 0));
}

#[test]
fn rent_rate_above_config_max_is_refused_and_changes_nothing() {
    let mut w = World::new(band_cfg(130), 2, 10_000_000);
    open_book(&mut w, 1, 10 * POS_SCALE);
    let (price, _) = w.wrapper_price();
    let now = w.now + 3;
    let r = w.market(|m| m.accrue_asset_to_with_rent_not_atomic(0, now, price, 0, 24, 0, true));
    assert_eq!(r.map(|_| ()), Err(V16Error::InvalidConfig));
    let r = w.market(|m| m.accrue_asset_to_with_rent_not_atomic(0, now, price, 0, 0, 24, true));
    assert_eq!(r.map(|_| ()), Err(V16Error::InvalidConfig));
    let a = w.asset();
    assert_eq!((a.rent_index_long_num, a.rent_index_short_num), (0, 0));
    // Exactly the max is fine.
    w.now += 3;
    w.accrue(0, 23, 23).unwrap();
    assert!(w.asset().rent_index_long_num > 0);
}

#[test]
fn rent_index_never_decreases_through_a_long_mixed_schedule() {
    let mut w = World::new(band_cfg(130), 2, 100_000_000);
    open_book(&mut w, 1, 10 * POS_SCALE);
    let (mut l, mut s) = (0u128, 0u128);
    for k in 0..60u64 {
        w.now += 3;
        w.accrue(0, k % 24, (k * 5) % 24).unwrap();
        let a = w.asset();
        assert!(a.rent_index_long_num >= l && a.rent_index_short_num >= s);
        l = a.rent_index_long_num;
        s = a.rent_index_short_num;
    }
}

// ---------------------------------------------------------------------------
// Engine: legs that change or close mid-period
// ---------------------------------------------------------------------------

#[test]
fn rent_closed_leg_pays_only_for_the_time_it_was_open() {
    let mut w = World::new(band_cfg(130), 4, 10_000_000);
    let q = 10 * POS_SCALE;
    open_book(&mut w, 1, q);
    run_rent(&mut w, 150, 20, 20);
    let idx_at_close = w.asset().rent_index_long_num;
    // Certify, then close the whole pair at the current price.
    w.refresh(0).unwrap();
    w.refresh(1).unwrap();
    w.trade(1, 0, q).expect("close");
    assert!(!w.positioned(0) && !w.positioned(1), "both legs closed");
    let cap0 = w.accounts[0].capital.get();
    let cap1 = w.accounts[1].capital.get();
    // Open a second pair so the index keeps moving, and run a lot more time.
    w.trade(2, 3, q).unwrap();
    run_rent(&mut w, 150, 20, 20);
    w.refresh(0).unwrap();
    w.refresh(1).unwrap();
    assert_eq!(w.accounts[0].capital.get(), cap0, "closed long pays nothing after close");
    assert_eq!(w.accounts[1].capital.get(), cap1, "closed short pays nothing after close");
    // The first period charged exactly floor(q * idx / DEN) to account 0 (deposit - paid).
    let paid0 = 10_000_000 - cap0;
    assert_eq!(paid0, rent_due_atoms(q, idx_at_close, 0).unwrap());
    assert_books(&w);
}

#[test]
fn rent_reopened_leg_starts_from_the_current_index() {
    let mut w = World::new(band_cfg(130), 4, 10_000_000);
    let q = 10 * POS_SCALE;
    open_book(&mut w, 1, q);
    run_rent(&mut w, 60, 20, 20);
    w.refresh(0).unwrap();
    w.refresh(1).unwrap();
    w.trade(1, 0, q).expect("close");
    // Keep a second pair open so the book is never empty, accrue, then reopen 0/1.
    w.trade(2, 3, q).unwrap();
    run_rent(&mut w, 90, 20, 20);
    w.refresh(2).unwrap();
    w.refresh(3).unwrap();
    w.trade(0, 1, q).expect("reopen");
    let idx = w.asset().rent_index_long_num;
    let leg = w.accounts[0].legs[0].try_to_runtime().unwrap();
    assert_eq!(leg.rent_snap, idx, "snapshot is the index at reopen");
    assert_eq!(leg.rent_carry, 0, "a fresh leg starts with no carry");
}

#[test]
fn rent_partial_close_charges_the_remaining_size_afterwards() {
    let mut w = World::new(band_cfg(130), 2, 100_000_000);
    let q = 40 * POS_SCALE;
    w.trade(0, 1, q).unwrap();
    run_rent(&mut w, 90, 20, 0);
    let idx1 = w.asset().rent_index_long_num;
    w.refresh(0).unwrap();
    w.refresh(1).unwrap();
    // Close half: account 0 sells 10 units back to account 1.
    let half = 10 * POS_SCALE;
    w.trade(1, 0, half).unwrap();
    let cap_mid = w.accounts[0].capital.get();
    run_rent(&mut w, 90, 20, 0);
    let idx2 = w.asset().rent_index_long_num;
    w.refresh(0).unwrap();
    let paid_second = cap_mid - w.accounts[0].capital.get();
    let remaining = q - half;
    // Floor of the exact total: first segment on q, second on the remaining size.
    let exact_first = rent_due_with_carry(q, idx1, 0, 0).unwrap();
    let (_, carry_mid) = exact_first;
    let (due2, _) = rent_due_with_carry(remaining, idx2, idx1, carry_mid).unwrap();
    assert_eq!(paid_second, due2, "second period charged on the remaining size only");
    assert_books(&w);
}

#[test]
fn rent_flat_account_pays_nothing_while_the_index_grows() {
    let mut w = World::new(band_cfg(130), 3, 10_000_000);
    open_book(&mut w, 1, 10 * POS_SCALE);
    let flat_cap = w.accounts[2].capital.get();
    run_rent(&mut w, 120, 23, 23);
    w.refresh(2).unwrap();
    assert_eq!(w.accounts[2].capital.get(), flat_cap);
    assert_eq!(w.header.insurance.get(), 0, "nobody was charged yet");
}

#[test]
fn rent_second_refresh_in_the_same_slot_is_a_no_op() {
    let mut w = World::new(band_cfg(130), 2, 10_000_000);
    open_book(&mut w, 1, 10 * POS_SCALE);
    run_rent(&mut w, 120, 23, 23);
    w.refresh(0).unwrap();
    let snapshot = (
        w.accounts[0].capital.get(),
        w.header.insurance.get(),
        w.header.c_tot.get(),
        w.asset().rent_unrouted_atoms,
    );
    w.refresh(0).unwrap();
    w.refresh(0).unwrap();
    assert_eq!(
        snapshot,
        (
            w.accounts[0].capital.get(),
            w.header.insurance.get(),
            w.header.c_tot.get(),
            w.asset().rent_unrouted_atoms
        )
    );
}

// ---------------------------------------------------------------------------
// Engine: rent never creates or destroys value
// ---------------------------------------------------------------------------

#[test]
fn rent_moves_capital_to_insurance_one_for_one() {
    let mut w = World::new(band_cfg(130), 2, 10_000_000);
    open_book(&mut w, 1, 10 * POS_SCALE);
    let total_before = w.header.c_tot.get() + w.header.insurance.get();
    let vault_before = w.header.vault.get();
    run_rent(&mut w, 240, 23, 23);
    w.refresh(0).unwrap();
    w.refresh(1).unwrap();
    let paid = w.header.insurance.get();
    assert!(paid > 0);
    assert_eq!(w.header.c_tot.get() + w.header.insurance.get(), total_before);
    assert_eq!(w.header.vault.get(), vault_before, "rent never touches the vault");
    assert_eq!(w.asset().rent_unrouted_atoms, paid, "every atom is booked as LP rent");
    assert_books(&w);
}

#[test]
fn rent_routing_twice_pays_out_once() {
    let mut w = World::new(band_cfg(130), 3, 10_000_000);
    open_book(&mut w, 1, 10 * POS_SCALE);
    run_rent(&mut w, 240, 23, 23);
    w.refresh(0).unwrap();
    w.refresh(1).unwrap();
    let claim = w.asset().rent_unrouted_atoms;
    let first = w.with(2, |m, a| m.route_rent_to_account_not_atomic(0, a)).unwrap();
    let second = w.with(2, |m, a| m.route_rent_to_account_not_atomic(0, a)).unwrap();
    assert_eq!((first, second), (claim, 0));
    assert_books(&w);
}

#[test]
fn rent_thin_account_settles_every_slot_and_the_books_balance() {
    // Capital just above initial margin; refresh both legs every step for 600 slots.
    let mut w = World::new(band_cfg(130), 2, 600_000);
    w.trade(0, 1, 5 * POS_SCALE).unwrap();
    let start = w.accounts[0].capital.get();
    for _ in 0..200 {
        w.now += 3;
        w.accrue(0, 23, 23).unwrap();
        w.refresh(0).unwrap();
        w.refresh(1).unwrap();
        assert_books(&w);
    }
    let paid = start - w.accounts[0].capital.get();
    // 600 slots at 23e9/slot on 5 units at P0: floor(5 * 1e6 * 23 * 600 / 1e9) = 69 atoms.
    assert_eq!(paid, 5 * P0 as u128 * 23 * 600 / 1_000_000_000);
}

#[test]
fn rent_out_of_band_accrual_with_rent_changes_nothing() {
    let mut w = World::new(band_cfg(130), 2, 10_000_000);
    open_book(&mut w, 1, 5 * POS_SCALE);
    w.now = 3;
    let (_, hi) = band_bounds(P0, 130).unwrap();
    let h = w.header;
    let m = w.markets.clone();
    let r = w.market(|m| m.accrue_asset_to_with_rent_not_atomic(0, 3, hi + 1, 0, 20, 20, true));
    assert_eq!(r.map(|_| ()), Err(V16Error::BandOutOfRange));
    assert_eq!(w.header, h);
    assert_eq!(w.markets[0].engine, m[0].engine);
    // At the edge with rent: accepted, and the index uses the edge price.
    w.market(|m| m.accrue_asset_to_with_rent_not_atomic(0, 3, hi, 0, 20, 0, true))
        .unwrap();
    assert_eq!(w.asset().rent_index_long_num, hi as u128 * 20 * 2);
}

#[test]
fn rent_settle_schedule_does_not_change_the_total_engine_level() {
    // Accounts 0/1 are refreshed every step, accounts 2/3 once at the end.
    let mut w = World::new(band_cfg(130), 4, 10_000_000);
    let q = 7 * POS_SCALE + 13;
    w.trade(0, 1, q).unwrap();
    w.trade(2, 3, q).unwrap();
    for _ in 0..80 {
        w.now += 3;
        w.accrue(0, 17, 17).unwrap();
        w.refresh(0).unwrap();
        w.refresh(1).unwrap();
    }
    w.refresh(2).unwrap();
    w.refresh(3).unwrap();
    assert_eq!(w.accounts[0].capital.get(), w.accounts[2].capital.get());
    assert_eq!(w.accounts[1].capital.get(), w.accounts[3].capital.get());
    assert_books(&w);
}

#[test]
fn rent_short_leg_pays_the_short_index_and_a_winner_still_pays() {
    let mut w = World::new(band_cfg(130), 2, 10_000_000);
    let q = 10 * POS_SCALE;
    w.trade(0, 1, q).unwrap();
    // Price falls inside the band: the short (account 1) wins, the long loses.
    w.set_target(P0 - P0 / 200);
    for _ in 0..40 {
        w.now += 3;
        w.accrue(0, 0, 23).unwrap();
    }
    let a = w.asset();
    let cap1 = w.accounts[1].capital.get();
    w.refresh(1).unwrap();
    let paid = cap1 - w.accounts[1].capital.get();
    let owed = rent_due_atoms(q, a.rent_index_short_num, 0).unwrap();
    assert_eq!(paid, owed, "winner pays rent on the short index");
    assert!(w.accounts[1].pnl.get() >= 0 || w.accounts[1].capital.get() > 0);
    // The long side's index never moved, so the long pays nothing.
    assert_eq!(a.rent_index_long_num, 0);
    // The long's capital moves only by its price loss (0.5% of 10 units at P0 is
    // not rent), so check the long owes zero rent on its index.
    assert_eq!(rent_due_atoms(q, a.rent_index_long_num, 0).unwrap(), 0);
    w.refresh(0).unwrap();
    assert_books(&w);
}
