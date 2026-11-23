//! `bound_num_is_atom_aligned` (src/v16.rs) replaces `x % BOUND_SCALE == 0` on the source-credit
//! and lien validation paths with a u64 limb reduction for x < 2^88. This pins it to the u128
//! modulo on edges, around every multiple near the 2^64 / 2^88 boundaries, and on random values.
//! Needs `--features fuzz` (exposes the test hook, same as tests/rounding_edge.rs).
#![cfg(feature = "fuzz")]
use percolator::{bound_num_is_atom_aligned_for_test as aligned, BOUND_SCALE};

fn check(x: u128) {
    assert_eq!(aligned(x), x % BOUND_SCALE == 0, "x={x}");
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
}

#[test]
fn atom_aligned_matches_modulo_on_edges() {
    let edges = [
        0u128,
        1,
        BOUND_SCALE - 1,
        BOUND_SCALE,
        BOUND_SCALE + 1,
        2 * BOUND_SCALE,
        u64::MAX as u128,
        1u128 << 64,
        (1u128 << 88) - 1,
        1u128 << 88,
        (1u128 << 88) + 1,
        u128::MAX - 1,
        u128::MAX,
    ];
    for x in edges {
        for d in 0..3u128 {
            check(x.saturating_sub(d));
            check(x.saturating_add(d));
        }
    }
    // multiples of BOUND_SCALE straddling 2^64 and 2^88, and their neighbours
    for boundary in [1u128 << 64, 1u128 << 88, 1u128 << 96] {
        let k0 = boundary / BOUND_SCALE;
        for k in k0.saturating_sub(2_000)..k0 + 2_000 {
            let m = k * BOUND_SCALE;
            check(m);
            check(m - 1);
            check(m + 1);
        }
    }
}

#[test]
fn atom_aligned_matches_modulo_on_random_values() {
    let mut rng = XorShift(0x2545_f491_4f6c_dd1d);
    for _ in 0..3_000_000 {
        let r = rng.u128();
        // full range, below 2^88, below 2^64, and exact multiples (+-small offsets)
        check(r);
        check(r >> 40);
        check(r >> 64);
        let m = (r >> 50) * BOUND_SCALE;
        check(m);
        check(m.wrapping_add(rng.next() as u128 % 3).wrapping_sub(1));
    }
}
