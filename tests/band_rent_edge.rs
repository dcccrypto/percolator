//! Edge-case tests for the v2.2 band + rent primitives (src/band_rent.rs) and the
//! engine paths that use them. Tests only. Complements tests/v22_band_rent.rs:
//! this file does not repeat its cases, it covers boundaries, zero and max inputs,
//! carry behaviour, and rent on legs that change or close mid-period.

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
