//! `loss_weight_for_basis` (src/v16.rs) runs on every active-leg decode. It now computes
//! ceil(abs_basis_q * SOCIAL_WEIGHT_SCALE / a_basis) in u128 when the product fits (and returns
//! abs_basis_q directly for an unscaled side) instead of always going through U512. This pins the
//! result and the error to the original U512 computation for every input class.
use percolator::{
    loss_weight_for_basis_for_test as fast, loss_weight_for_basis_wide_reference_for_test as wide,
    ADL_ONE, MAX_POSITION_ABS_Q, MIN_A_SIDE, POS_SCALE, SOCIAL_WEIGHT_SCALE,
};

fn check(abs_basis_q: u128, a_basis: u128) {
    assert_eq!(
        fast(abs_basis_q, a_basis),
        wide(abs_basis_q, a_basis),
        "abs_basis_q={abs_basis_q} a_basis={a_basis}"
    );
}

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
    fn below(&mut self, n: u128) -> u128 {
        self.u128() % n
    }
}

/// u128::MAX / SOCIAL_WEIGHT_SCALE: the largest abs_basis_q whose product still fits u128.
const FIT_EDGE: u128 = u128::MAX / SOCIAL_WEIGHT_SCALE;

fn abs_edges() -> Vec<u128> {
    vec![
        0,
        1,
        2,
        3,
        POS_SCALE - 1,
        POS_SCALE,
        POS_SCALE + 1,
        MAX_POSITION_ABS_Q - 1,
        MAX_POSITION_ABS_Q,
        MAX_POSITION_ABS_Q + 1,
        FIT_EDGE - 1,
        FIT_EDGE,
        FIT_EDGE + 1,
        u64::MAX as u128,
        u128::MAX / 2,
        u128::MAX - 1,
        u128::MAX,
    ]
}

fn a_edges() -> Vec<u128> {
    vec![
        0,
        1,
        2,
        3,
        7,
        MIN_A_SIDE - 1,
        MIN_A_SIDE,
        MIN_A_SIDE + 1,
        ADL_ONE / 3,
        ADL_ONE - 1,
        ADL_ONE,
        ADL_ONE + 1,
        SOCIAL_WEIGHT_SCALE,
        999_999_999_999_989, // prime just below ADL_ONE
        u64::MAX as u128,
        u64::MAX as u128 + 1,
        u128::MAX - 1,
        u128::MAX,
    ]
}

#[test]
fn loss_weight_fast_path_matches_u512_on_edges() {
    for abs in abs_edges() {
        for a in a_edges() {
            check(abs, a);
        }
    }
}

#[test]
fn loss_weight_fast_path_matches_u512_on_validated_leg_domain() {
    // The domain `validate_active_leg` reaches: |basis| in [1, MAX_POSITION_ABS_Q],
    // a_basis in [MIN_A_SIDE, ADL_ONE].
    let mut rng = XorShift(0x9e37_79b9_7f4a_7c15);
    for _ in 0..5_000_000 {
        let abs = 1 + rng.below(MAX_POSITION_ABS_Q);
        let a = MIN_A_SIDE + rng.below(ADL_ONE - MIN_A_SIDE + 1);
        check(abs, a);
    }
    // small bases (exact-division and remainder cases are dense here)
    for abs in 0..2_000u128 {
        for a in [MIN_A_SIDE, MIN_A_SIDE + 1, ADL_ONE / 2 + 1, ADL_ONE - 1, ADL_ONE] {
            check(abs, a);
        }
    }
}

#[test]
fn loss_weight_fast_path_matches_u512_on_any_u128() {
    // Outside the validated domain too, including products that overflow u128 (wide fallback).
    let mut rng = XorShift(0xd1b5_4a32_d192_ed03);
    for _ in 0..2_000_000 {
        let abs = match rng.next() % 4 {
            0 => rng.u128(),
            1 => rng.below(FIT_EDGE + 2),
            2 => FIT_EDGE.saturating_sub(rng.below(1_000)) + rng.below(2_000),
            _ => rng.below(u64::MAX as u128),
        };
        let a = match rng.next() % 4 {
            0 => rng.u128(),
            1 => rng.below(ADL_ONE * 4),
            2 => 1 + rng.below(1_000),
            _ => SOCIAL_WEIGHT_SCALE,
        };
        check(abs, a);
    }
}

#[test]
fn loss_weight_zero_a_basis_is_invalid_leg_on_both_paths() {
    for abs in abs_edges() {
        assert_eq!(fast(abs, 0), Err(percolator::V16Error::InvalidLeg));
        check(abs, 0);
    }
}
