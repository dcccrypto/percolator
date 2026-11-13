//! Rounding-direction and dust checks for the v2.2 engine's pure arithmetic kernels.
//! Tests only. Needs `--features fuzz` (exposes the kernels, same as tests/v16_fuzzing.rs).
//!
//! Rule under test: no rounding path creates value, and the error is at most 1 atom
//! in the protocol's favour. Liquidation, funding, oracle and fee/lien edge cases
//! live in their own files (#321, #323, #318, #316); this file stays on the kernels.
#![cfg(feature = "fuzz")]

use percolator::band_rent::*;
use percolator::{
    kani_adl_effective_quantity_ceil, kani_funding_index_deltas, kani_mul_div_ceil_u128_or_wide,
    kani_mul_div_ceil_u128_wide_reference, kani_mul_div_floor_u128_or_wide,
    kani_mul_div_floor_u128_wide_reference, kani_raw_basis_for_adl_effective_quantity, V16Error,
    ADL_ONE, FUNDING_DEN, MAX_ORACLE_PRICE, MAX_POSITION_ABS_Q, MIN_A_SIDE,
};

/// Deterministic xorshift so the loops are reproducible without extra deps.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn u128(&mut self) -> u128 {
        ((self.next() as u128) << 64) | self.next() as u128
    }
    /// Mix of tiny, boundary and random values.
    fn pick(&mut self, max: u128) -> u128 {
        match self.next() % 5 {
            0 => (self.next() % 4) as u128 % (max + 1),
            1 => max,
            2 => max.saturating_sub((self.next() % 3) as u128),
            _ => self.u128() % (max + 1),
        }
    }
}

// ---- mul_div floor / ceil kernels -------------------------------------------------

#[test]
fn rounding_mul_div_floor_ceil_bracket_the_exact_quotient() {
    let mut rng = Rng(0x9E3779B97F4A7C15);
    for _ in 0..20_000 {
        let a = rng.pick(u64::MAX as u128 * 4);
        let b = rng.pick(u64::MAX as u128 * 4);
        let d = rng.pick(u64::MAX as u128 * 4).max(1);
        let f = kani_mul_div_floor_u128_or_wide(a, b, d);
        let c = kani_mul_div_ceil_u128_or_wide(a, b, d);
        if f.is_err() || c.is_err() {
            // Quotient does not fit u128: both must fail, never wrap.
            assert!(
                f.is_err()
                    && c.is_err()
                    && kani_mul_div_floor_u128_wide_reference(a, b, d).is_err()
            );
            continue;
        }
        let (f, c) = (f.unwrap(), c.unwrap());
        // Same value as the wide reference.
        assert_eq!(Ok(f), kani_mul_div_floor_u128_wide_reference(a, b, d));
        assert_eq!(Ok(c), kani_mul_div_ceil_u128_wide_reference(a, b, d));
        // ceil - floor is 0 (exact) or 1, never more.
        assert!(c == f || c == f + 1, "a={a} b={b} d={d} f={f} c={c}");
        // floor never over-credits, ceil never under-charges (only checkable when a*b fits u128).
        if let Some(p) = a.checked_mul(b) {
            assert!(f.checked_mul(d).unwrap() <= p);
            assert_eq!(c == f, p % d == 0);
        }
    }
}

#[test]
fn rounding_mul_div_one_atom_and_max_value() {
    // 1 atom through a fee-style ratio: floor loses it, ceil charges it.
    assert_eq!(kani_mul_div_floor_u128_or_wide(1, 1, 10_000), Ok(0));
    assert_eq!(kani_mul_div_ceil_u128_or_wide(1, 1, 10_000), Ok(1));
    assert_eq!(kani_mul_div_ceil_u128_or_wide(0, u128::MAX, 7), Ok(0));
    assert_eq!(
        kani_mul_div_floor_u128_or_wide(u128::MAX, 1, 1),
        Ok(u128::MAX)
    );
    // Wide product, exact: (MAX*MAX)/MAX == MAX both ways.
    assert_eq!(
        kani_mul_div_floor_u128_or_wide(u128::MAX, u128::MAX, u128::MAX),
        Ok(u128::MAX)
    );
    assert_eq!(
        kani_mul_div_ceil_u128_or_wide(u128::MAX, u128::MAX, u128::MAX),
        Ok(u128::MAX)
    );
    // Result that does not fit must error, not wrap.
    assert!(kani_mul_div_floor_u128_or_wide(u128::MAX, 2, 1).is_err());
    assert!(kani_mul_div_ceil_u128_or_wide(u128::MAX, 2, 1).is_err());
    // Wide product MAX*(MAX-1) divides exactly: floor == ceil == MAX-1.
    assert_eq!(
        kani_mul_div_floor_u128_or_wide(u128::MAX, u128::MAX - 1, u128::MAX),
        Ok(u128::MAX - 1)
    );
    // Zero denominator is refused.
    assert!(kani_mul_div_floor_u128_or_wide(1, 1, 0).is_err());
    assert!(kani_mul_div_ceil_u128_or_wide(1, 1, 0).is_err());
}

#[test]
fn rounding_split_sum_of_floors_never_exceeds_floor_of_sum() {
    // Splitting an amount into parts and flooring each can only lose dust; the loss
    // is < number of parts. Ceil is the mirror: sum of ceils >= ceil of sum, excess < parts.
    let mut rng = Rng(42);
    for _ in 0..5_000 {
        let n = 2 + (rng.next() % 6) as usize;
        let d = rng.pick(1_000_000_000_000).max(1);
        let num = rng.pick(1_000_000_000_000);
        let parts: Vec<u128> = (0..n).map(|_| rng.pick(1_000_000_000_000)).collect();
        let whole: u128 = parts.iter().sum();
        let f_whole = kani_mul_div_floor_u128_or_wide(whole, num, d).unwrap();
        let c_whole = kani_mul_div_ceil_u128_or_wide(whole, num, d).unwrap();
        let f_sum: u128 = parts
            .iter()
            .map(|p| kani_mul_div_floor_u128_or_wide(*p, num, d).unwrap())
            .sum();
        let c_sum: u128 = parts
            .iter()
            .map(|p| kani_mul_div_ceil_u128_or_wide(*p, num, d).unwrap())
            .sum();
        assert!(f_sum <= f_whole && f_whole - f_sum < n as u128);
        assert!(c_sum >= c_whole && c_sum - c_whole < n as u128);
    }
}

// ---- ADL effective quantity (ceil) vs raw basis (floor) ----------------------------

#[test]
fn rounding_adl_effective_quantity_never_below_exact_and_never_grows() {
    let mut rng = Rng(7);
    for _ in 0..20_000 {
        let raw = rng.pick(MAX_POSITION_ABS_Q);
        let a_basis = MIN_A_SIDE + rng.pick(ADL_ONE - MIN_A_SIDE);
        let cur = 1 + rng.pick(a_basis - 1);
        let eff = kani_adl_effective_quantity_ceil(raw, a_basis, cur).unwrap();
        // Effective size is a ceil: never smaller than exact, within 1 atom of it,
        // and never larger than the raw basis (ADL only shrinks).
        let exact_floor = raw * cur / a_basis;
        let exact_ceil = exact_floor + u128::from((raw * cur) % a_basis != 0);
        assert_eq!(eff, exact_ceil, "must be a ceil, not a floor");
        assert!(eff <= raw);
        // A non-zero position cannot be rounded to nothing while A > 0.
        if raw > 0 {
            assert!(eff >= 1);
        }
    }
}

#[test]
fn rounding_adl_one_atom_position_survives_any_a() {
    for cur in [1u128, MIN_A_SIDE - 1, MIN_A_SIDE, ADL_ONE / 2, ADL_ONE - 1] {
        let basis = ADL_ONE;
        if cur > basis {
            continue;
        }
        assert_eq!(kani_adl_effective_quantity_ceil(1, basis, cur), Ok(1));
    }
    assert_eq!(kani_adl_effective_quantity_ceil(0, ADL_ONE, 1), Ok(0));
    assert_eq!(
        kani_adl_effective_quantity_ceil(MAX_POSITION_ABS_Q, ADL_ONE, ADL_ONE),
        Ok(MAX_POSITION_ABS_Q)
    );
    // Out of domain is an error, not a silent clamp.
    assert_eq!(
        kani_adl_effective_quantity_ceil(MAX_POSITION_ABS_Q + 1, ADL_ONE, ADL_ONE),
        Err(V16Error::InvalidLeg)
    );
    assert_eq!(
        kani_adl_effective_quantity_ceil(1, ADL_ONE, 0),
        Err(V16Error::InvalidLeg)
    );
}

#[test]
fn rounding_adl_raw_basis_floor_then_effective_ceil_does_not_exceed_target() {
    // Converting a target effective size back to a raw basis floors, so the
    // re-derived effective size never exceeds the target (no value created).
    let mut rng = Rng(99);
    for _ in 0..20_000 {
        let a_basis = MIN_A_SIDE + rng.pick(ADL_ONE - MIN_A_SIDE);
        let cur = 1 + rng.pick(a_basis - 1);
        let target = rng.pick(MAX_POSITION_ABS_Q);
        let Ok(raw) = kani_raw_basis_for_adl_effective_quantity(target, a_basis, cur) else {
            continue;
        };
        let eff = kani_adl_effective_quantity_ceil(raw, a_basis, cur).unwrap();
        assert_eq!(eff, target, "floor then ceil must round-trip exactly");
        if raw > 0 {
            assert!(eff >= 1);
        }
    }
}

// ---- funding index deltas: payer ceil, receiver floor ------------------------------

#[test]
fn rounding_funding_payer_ceil_receiver_floor() {
    let mut rng = Rng(2026);
    for _ in 0..30_000 {
        let funding_num =
            (rng.pick(1u128 << 100) as i128) * if rng.next() & 1 == 0 { 1 } else { -1 };
        let a_long = MIN_A_SIDE + rng.pick(ADL_ONE - MIN_A_SIDE);
        let a_short = MIN_A_SIDE + rng.pick(ADL_ONE - MIN_A_SIDE);
        let Ok((fl, fs)) = kani_funding_index_deltas(funding_num, a_long, a_short) else {
            continue;
        };
        if funding_num > 0 {
            assert!(fl <= 0 && fs >= 0);
        } else if funding_num < 0 {
            assert!(fl >= 0 && fs <= 0);
        } else {
            assert_eq!((fl, fs), (0, 0));
        }
        // Exact magnitudes, per-unit-of-A: payer = ceil(mag*A/DEN), receiver = floor.
        let mag = funding_num.unsigned_abs();
        let (pay_a, recv_a, pay, recv) = if funding_num > 0 {
            (a_long, a_short, fl.unsigned_abs(), fs.unsigned_abs())
        } else if funding_num < 0 {
            (a_short, a_long, fs.unsigned_abs(), fl.unsigned_abs())
        } else {
            continue;
        };
        assert_eq!(
            Ok(pay),
            kani_mul_div_ceil_u128_or_wide(mag, pay_a, FUNDING_DEN)
        );
        assert_eq!(
            Ok(recv),
            kani_mul_div_floor_u128_or_wide(mag, recv_a, FUNDING_DEN)
        );
    }
}

#[test]
fn rounding_funding_one_atom_rate_is_charged_to_payer_not_credited_to_receiver() {
    // funding_num = 1, A = ONE-1: exact index delta is 999_999.999999999 (A/FUNDING_DEN).
    let a = ADL_ONE - 1;
    let (fl, fs) = kani_funding_index_deltas(1, a, a).unwrap();
    assert_eq!(fl, -1_000_000, "payer side rounds the dust up");
    assert_eq!(fs, 999_999, "receiver side rounds the dust down");
    let (fl, fs) = kani_funding_index_deltas(-1, a, a).unwrap();
    assert_eq!((fl, fs), (999_999, -1_000_000));
    // At A == ONE it is exact and symmetric (ADL_ONE is a multiple of FUNDING_DEN).
    let (fl, fs) = kani_funding_index_deltas(3, ADL_ONE, ADL_ONE).unwrap();
    assert_eq!(fl, -fs);
}

#[test]
fn rounding_funding_max_rate_overflow_is_an_error_not_a_wrap() {
    let r = kani_funding_index_deltas(i128::MAX, ADL_ONE, ADL_ONE);
    assert!(r.is_err());
    // i128::MIN: 2^127 * 1e6 overflows, so it errors.
    let r = kani_funding_index_deltas(i128::MIN, ADL_ONE, ADL_ONE);
    assert!(r.is_err());
}

// ---- band / rent arithmetic ---------------------------------------------------------

#[test]
fn rounding_band_never_wider_than_real_valued_band() {
    let mut rng = Rng(5);
    for _ in 0..20_000 {
        let anchor = 1 + rng.pick(MAX_ORACLE_PRICE as u128 - 1) as u64;
        let d = 1 + (rng.next() % MAX_BAND_BPS);
        let (lo, hi) = band_bounds(anchor, d).unwrap();
        let a = anchor as u128;
        // lo = ceil(a*(1-d)) -> lo*10000 >= a*(10000-d); hi = floor -> hi*10000 <= a*(10000+d) (unless capped).
        assert!(lo as u128 * 10_000 >= a * (10_000 - d as u128));
        assert!((lo as u128 * 10_000) < a * (10_000 - d as u128) + 10_000);
        assert!(hi as u128 * 10_000 <= a * (10_000 + d as u128));
        assert!(lo >= 1 && lo <= anchor && anchor <= hi && hi <= MAX_ORACLE_PRICE);
    }
    // 1-atom anchor: floor/ceil collapse the band to the anchor itself.
    assert_eq!(band_bounds(1, 1).unwrap(), (1, 1));
    // Max anchor: hi is capped.
    assert_eq!(
        band_bounds(MAX_ORACLE_PRICE, 2_000).unwrap().1,
        MAX_ORACLE_PRICE
    );
}

#[test]
fn rounding_clamp_to_band_never_overshoots() {
    let mut rng = Rng(11);
    for _ in 0..10_000 {
        let anchor = 1 + rng.pick(MAX_ORACLE_PRICE as u128 - 1) as u64;
        let d = 1 + (rng.next() % MAX_BAND_BPS);
        let p = rng.pick(MAX_ORACLE_PRICE as u128) as u64;
        let c = clamp_to_band(p, anchor, d).unwrap();
        let (lo, hi) = band_bounds(anchor, d).unwrap();
        assert!(lo <= c && c <= hi);
        if (lo..=hi).contains(&p) {
            assert_eq!(c, p);
        }
    }
}

#[test]
fn rounding_worst_adverse_bps_is_ceil_of_real_value() {
    for d in 1..=MAX_BAND_BPS {
        let g = band_worst_adverse_bps(d).unwrap() as u128;
        let (num, den) = ((10_000 + d as u128).pow(2), 10_000 - d as u128);
        // (10000+g)*den >= num, and g-1 is not enough.
        assert!((10_000 + g) * den >= num);
        assert!((10_000 + g - 1) * den < num);
    }
}

#[test]
fn rounding_rent_split_settles_never_exceed_one_settle_and_carry_is_exact() {
    let mut rng = Rng(77);
    for _ in 0..20_000 {
        let q = rng.pick(MAX_POSITION_ABS_Q);
        let i1 = rng.pick(1u128 << 70);
        let i2 = i1 + rng.pick(1u128 << 70);
        let i3 = i2 + rng.pick(1u128 << 70);
        let one = rent_due_atoms(q, i3, i1).unwrap();
        let (d1, c1) = rent_due_with_carry(q, i2, i1, 0).unwrap();
        let (d2, c2) = rent_due_with_carry(q, i3, i2, c1).unwrap();
        // With carry the split equals the whole exactly; the leftover is < 1 atom.
        assert_eq!(d1 + d2, one);
        assert!((c2 as u128) < RENT_INDEX_DEN);
        // Without carry the sum of floors can only lose, by at most 1 atom.
        let n1 = rent_due_atoms(q, i2, i1).unwrap();
        let n2 = rent_due_atoms(q, i3, i2).unwrap();
        assert!(n1 + n2 <= one && one - (n1 + n2) <= 1);
    }
}

#[test]
fn rounding_rent_dust_and_max_values() {
    // 1 atom of size, tiny index move: owes nothing, carry keeps the fraction.
    let (due, carry) = rent_due_with_carry(1, 1, 0, 0).unwrap();
    assert_eq!((due, carry as u128), (0, 1));
    // Exactly one atom of rent.
    assert_eq!(rent_due_atoms(1, RENT_INDEX_DEN, 0), Ok(1));
    assert_eq!(rent_due_atoms(1, RENT_INDEX_DEN - 1, 0), Ok(0));
    // Carry that tips it over pays out one atom and leaves 0.
    assert_eq!(
        rent_due_with_carry(1, 1, 0, (RENT_INDEX_DEN - 1) as u64).unwrap(),
        (1, 0)
    );
    // Max-size leg x max index delta takes the wide path and must not wrap.
    let big = rent_due_atoms(MAX_POSITION_ABS_Q, u128::MAX, 0);
    assert!(matches!(big, Ok(_) | Err(BandRentError::Overflow)));
    if let Ok(v) = big {
        // Result must be at least the u128 floor lower bound (monotone, not wrapped).
        assert!(v >= rent_due_atoms(MAX_POSITION_ABS_Q, u128::MAX / 2, 0).unwrap());
    }
    // Ill-formed legs fail closed.
    assert_eq!(
        rent_due_with_carry(1, 0, 1, 0),
        Err(BandRentError::InvalidInput)
    );
    assert_eq!(
        rent_due_with_carry(1, 1, 0, RENT_INDEX_DEN as u64),
        Err(BandRentError::InvalidInput)
    );
    // Rent index delta is monotone and fail-closed.
    assert_eq!(rent_index_delta(MAX_ORACLE_PRICE, 10_000, 0), Ok(0));
    assert!(rent_index_delta(u64::MAX, u64::MAX, u64::MAX).is_err());
}

#[test]
fn rounding_rent_chargeable_never_exceeds_free_capital() {
    let mut rng = Rng(3);
    for _ in 0..10_000 {
        let due = rng.u128() >> (rng.next() % 100);
        let cap = rng.u128() >> (rng.next() % 100);
        let pnl = (rng.u128() >> (1 + rng.next() % 100)) as i128
            * if rng.next() & 1 == 0 { 1 } else { -1 };
        let c = rent_chargeable_atoms(due, cap, pnl);
        let owned = if pnl < 0 { pnl.unsigned_abs() } else { 0 };
        assert!(c <= due && c <= cap);
        assert!(c <= cap.saturating_sub(owned));
    }
    // Capital below the loss it already owes: no rent taken, no underflow.
    assert_eq!(rent_chargeable_atoms(10, 5, -100), 0);
    assert_eq!(rent_chargeable_atoms(10, 5, i128::MIN), 0);
}
