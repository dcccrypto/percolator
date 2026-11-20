//! v22-cu-1113: timing + output-checksum harness for the wide_math hot helpers.
//!
//! Run: `cargo test --release --features fork-facade --test perf_wide_math_bench -- --ignored --nocapture`
//!
//! Prints ns/op per helper over a fixed pseudo-random input set (several magnitude classes,
//! including the >u128 numerators that leave the native fast path) and an FNV-1a checksum of
//! every output. A perf change to wide_math must leave every checksum byte-identical.
#![cfg(all(not(kani), feature = "fork-facade"))]

use percolator::wide_math::{
    checked_mul_div_ceil_u256, mul_div_ceil_u256, mul_div_floor_u256, mul_div_floor_u256_with_rem,
    wide_mul_div_floor_u128, wide_signed_mul_div_floor_with_carry_from_k_pair, U256,
};
use std::hint::black_box;
use std::time::Instant;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn u128_bits(&mut self, bits: u32) -> u128 {
        let v = ((self.next() as u128) << 64) | self.next() as u128;
        if bits >= 128 {
            v
        } else {
            v & ((1u128 << bits) - 1)
        }
    }
}

struct Sum(u64);
impl Sum {
    fn add_u128(&mut self, v: u128) {
        for b in v.to_le_bytes() {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(0x100_0000_01b3);
        }
    }
    fn add_u256(&mut self, v: U256) {
        self.add_u128(v.lo());
        self.add_u128(v.hi());
    }
}

const N: usize = 20_000;
const REPS: usize = 5;

/// Operand magnitude classes in bits: (a, b, d). Mixes the native path (a*b < 2^128),
/// the U256 path, and divisors of every width class.
const CLASSES: &[(u32, u32, u32)] = &[
    (40, 40, 30),
    (64, 64, 20),
    (64, 64, 64),
    (96, 96, 40),
    (120, 100, 60),
    (127, 127, 64),
    (127, 127, 100),
    (128, 128, 127),
    (128, 128, 128),
];

fn inputs() -> Vec<(u128, u128, u128)> {
    let mut r = Rng(0x9E37_79B9_7F4A_7C15);
    let mut v = Vec::with_capacity(N);
    for i in 0..N {
        let (ab, bb, db) = CLASSES[i % CLASSES.len()];
        let a = r.u128_bits(ab);
        let b = r.u128_bits(bb);
        let mut d = r.u128_bits(db);
        if d == 0 {
            d = 1;
        }
        v.push((a, b, d));
    }
    v
}

fn bench<F: FnMut(&(u128, u128, u128), &mut Sum)>(name: &str, xs: &[(u128, u128, u128)], mut f: F) {
    let mut best = f64::MAX;
    let mut sum = Sum(0xcbf2_9ce4_8422_2325);
    for rep in 0..REPS {
        let mut s = Sum(0xcbf2_9ce4_8422_2325);
        let t = Instant::now();
        for x in xs {
            f(black_box(x), &mut s);
        }
        let ns = t.elapsed().as_nanos() as f64 / xs.len() as f64;
        best = best.min(ns);
        if rep == 0 {
            sum = s;
        } else {
            assert_eq!(sum.0, s.0, "non-deterministic output");
        }
    }
    println!("WMBENCH {name:<44} {best:>9.1} ns/op  checksum {:016x}", sum.0);
}

#[test]
#[ignore]
fn wide_math_bench() {
    let xs = inputs();
    bench("mul_div_floor_u256", &xs, |&(a, b, d), s| {
        s.add_u256(mul_div_floor_u256(U256::from_u128(a), U256::from_u128(b), U256::from_u128(d)))
    });
    bench("mul_div_floor_u256_with_rem", &xs, |&(a, b, d), s| {
        let (q, r) =
            mul_div_floor_u256_with_rem(U256::from_u128(a), U256::from_u128(b), U256::from_u128(d));
        s.add_u256(q);
        s.add_u256(r);
    });
    bench("mul_div_ceil_u256", &xs, |&(a, b, d), s| {
        s.add_u256(mul_div_ceil_u256(U256::from_u128(a), U256::from_u128(b), U256::from_u128(d)))
    });
    bench("checked_mul_div_ceil_u256 (256x128/128)", &xs, |&(a, b, d), s| {
        // 256-bit a exercises the U512 path.
        let a256 = U256::new(a, b >> 64);
        match checked_mul_div_ceil_u256(a256, U256::from_u128(b), U256::from_u128(d)) {
            Some(q) => s.add_u256(q),
            None => s.add_u128(u128::MAX),
        }
    });
    bench("wide_mul_div_floor_u128 (fits)", &xs, |&(a, b, d), s| {
        // keep the quotient inside u128: b <= d
        let b = if b > d { b % d } else { b };
        s.add_u128(wide_mul_div_floor_u128(a, b, d))
    });
    bench("wide_signed_mul_div_floor_with_carry_k_pair", &xs, |&(a, b, d), s| {
        // abs_basis * |k diff| must fit i128 after the divide: bound basis to 64 bits
        let basis = a >> 64;
        let k_then = (b >> 2) as i128 - (1i128 << 124);
        let k_now = (a >> 3) as i128 - (1i128 << 123);
        let den = d | (1u128 << 100);
        let carry = b % den;
        let (q, r) = wide_signed_mul_div_floor_with_carry_from_k_pair(basis, k_then, k_now, den, carry);
        s.add_u128(q as u128);
        s.add_u128(r);
    });
}
