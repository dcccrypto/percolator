//! `expected_source_credit_rate_num_for_state` and `source_credit_protective_rate` (src/v16.rs) now compute
//! min(floor(available * CREDIT_RATE_SCALE / positive_claim_bound_num), CREDIT_RATE_SCALE) in u128
//! when the product fits, instead of always through U256. This pins the result and the error to
//! the original U256 computation. Domain: positive_claim_bound_num != 0 (the caller returns
//! CREDIT_RATE_SCALE before this point when it is 0) and available != 0.
//! Needs `--features fuzz` (exposes the hooks, same as tests/rounding_edge.rs).
#![cfg(feature = "fuzz")]
use percolator::{
    source_credit_rate_num_for_test as fast,
    source_credit_rate_num_wide_reference_for_test as wide, BOUND_SCALE, CREDIT_RATE_SCALE,
};

fn check(available: u128, bound: u128) {
    assert_eq!(
        fast(available, bound),
        wide(available, bound),
        "available={available} bound={bound}"
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

/// u128::MAX / CREDIT_RATE_SCALE: the largest `available` whose product still fits u128.
const FIT_EDGE: u128 = u128::MAX / CREDIT_RATE_SCALE;

#[test]
fn credit_rate_matches_u256_on_edges() {
    let values = [
        1u128,
        2,
        3,
        BOUND_SCALE - 1,
        BOUND_SCALE,
        BOUND_SCALE + 1,
        CREDIT_RATE_SCALE - 1,
        CREDIT_RATE_SCALE,
        CREDIT_RATE_SCALE + 1,
        u64::MAX as u128,
        FIT_EDGE - 1,
        FIT_EDGE,
        FIT_EDGE + 1,
        u128::MAX / 2,
        u128::MAX - 1,
        u128::MAX,
    ];
    for available in values {
        for bound in values {
            check(available, bound);
        }
    }
    // available == 0 reaches the helper from source_credit_protective_rate (no early return there)
    for bound in values {
        check(0, bound);
        assert_eq!(fast(0, bound), Ok(0));
    }
}

#[test]
fn credit_rate_matches_u256_on_realistic_and_random_values() {
    let mut rng = XorShift(0x6a09_e667_f3bc_c908);
    for _ in 0..1_000_000 {
        // atom-scaled amounts (BOUND_SCALE units), the shape source-credit states carry
        let available = (1 + rng.below(1u128 << 64)) * BOUND_SCALE;
        let bound = (1 + rng.below(1u128 << 64)) * BOUND_SCALE;
        check(available, bound);
        // available close to bound: rate near the cap
        let near = bound.saturating_sub(rng.below(BOUND_SCALE * 4)).max(1);
        check(near, bound);
        check(bound + rng.below(BOUND_SCALE), bound);
    }
    for _ in 0..500_000 {
        let available = match rng.next() % 3 {
            0 => 1 + rng.below(u128::MAX - 1),
            1 => 1 + rng.below(FIT_EDGE + 2),
            _ => FIT_EDGE.saturating_sub(rng.below(1_000)) + rng.below(2_000),
        };
        let bound = match rng.next() % 3 {
            0 => 1 + rng.below(u128::MAX - 1),
            1 => 1 + rng.below(u64::MAX as u128),
            _ => 1 + rng.below(1_000),
        };
        check(available, bound);
    }
}

/// `source_credit_protective_rate` calls the same helper with available < denominator. There the
/// uncapped U256 quotient is already below CREDIT_RATE_SCALE, so the helper's cap leaves it as is
/// and the value equals the old uncapped expression.
#[test]
fn protective_rate_domain_is_below_the_cap() {
    let mut rng = XorShift(0xbb67_ae85_84ca_a73b);
    for _ in 0..300_000 {
        let denominator = match rng.next() % 3 {
            0 => 2 + rng.below(u128::MAX - 2),
            1 => 2 + rng.below(u64::MAX as u128),
            _ => (1 + rng.below(1u128 << 64)) * BOUND_SCALE,
        };
        let available = 1 + rng.below(denominator - 1);
        let r = wide(available, denominator).unwrap();
        assert!(r < CREDIT_RATE_SCALE, "available={available} denominator={denominator}");
        assert_eq!(fast(available, denominator), Ok(r));
    }
}
