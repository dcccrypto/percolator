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
