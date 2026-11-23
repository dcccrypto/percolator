//! `mul_div_floor_u128_or_wide` / `mul_div_ceil_u128_or_wide` (src/v16.rs) return `a` when
//! `b == denominator` (and `b` when `a == denominator`) without dividing, and the ceil helper tests
//! the remainder as `quotient * denominator != product` instead of `product % denominator`.
//! This compares both helpers with the U256 references on the shortcut inputs, around them, and on
//! random values, and that the shortcut (`mul_div_cancelled_factor`) fires exactly when a factor
//! equals the denominator. Needs `--features fuzz` (same hooks as tests/rounding_edge.rs).
#![cfg(feature = "fuzz")]
use percolator::{
    kani_mul_div_ceil_u128_or_wide as ceil, kani_mul_div_ceil_u128_wide_reference as ceil_ref,
    kani_mul_div_floor_u128_or_wide as floor, kani_mul_div_floor_u128_wide_reference as floor_ref,
    mul_div_cancelled_factor_for_test as cancelled, ADL_ONE, MAX_POSITION_ABS_Q, MIN_A_SIDE,
};

struct XorShift(u64);
impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn u128(&mut self) -> u128 {
        ((self.next() as u128) << 64) | self.next() as u128
    }
}

fn check(a: u128, b: u128, d: u128) {
    assert_eq!(
        floor(a, b, d),
        floor_ref(a, b, d),
        "floor a={a} b={b} d={d}"
    );
    assert_eq!(ceil(a, b, d), ceil_ref(a, b, d), "ceil a={a} b={b} d={d}");
    // the shortcut fires exactly when a factor equals the denominator, and then it is the answer
    let c = cancelled(a, b, d);
    assert_eq!(c.is_some(), d == b || d == a, "a={a} b={b} d={d}");
    if let Some(v) = c {
        if d != 0 {
            assert_eq!(Ok(v), floor_ref(a, b, d));
            assert_eq!(Ok(v), ceil_ref(a, b, d));
        }
    }
}

const EDGES: [u128; 16] = [
    0,
    1,
    2,
    3,
    999_999,
    MAX_POSITION_ABS_Q,
    MIN_A_SIDE,
    ADL_ONE - 1,
    ADL_ONE,
    u64::MAX as u128,
    (u64::MAX as u128) + 1,
    1u128 << 100,
    u128::MAX / 3,
    u128::MAX / 2,
    u128::MAX - 1,
    u128::MAX,
];

#[test]
fn shortcuts_match_u256_on_edges() {
    for a in EDGES {
        for b in EDGES {
            for d in EDGES {
                check(a, b, d);
            }
            // the shortcut inputs and their neighbours
            check(a, b, b);
            check(b, a, b);
            check(a, b, b.wrapping_add(1));
            check(a, b, b.saturating_sub(1));
            check(b, b, b);
        }
    }
}

#[test]
fn shortcuts_match_u256_on_random_values() {
    let mut rng = XorShift(0x9b05_688c_2b3e_6c1f);
    for _ in 0..100_000 {
        let a = rng.u128() >> (rng.next() % 128);
        let b = rng.u128() >> (rng.next() % 128);
        let d = rng.u128() >> (rng.next() % 128);
        check(a, b, d);
        check(a, b, b);
        check(a, d, a);
        check(a, b, d | 1);
        // the ADL-effective-quantity shape: |basis| * current_a / a_basis
        let abs = rng.u128() % (MAX_POSITION_ABS_Q + 1);
        let a_basis = MIN_A_SIDE + rng.u128() % (ADL_ONE - MIN_A_SIDE + 1);
        let current_a = 1 + rng.u128() % a_basis;
        check(abs, current_a, a_basis);
        check(abs, a_basis, a_basis);
    }
}
