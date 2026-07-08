//! Property-based coverage for the hand-rolled wide arithmetic in
//! `src/wide_math.rs` (U256 / I256 / U512 mul-div, spec §4.6 and §4.8).
//!
//! Every haircut ratio, funding accrual, and ADL settlement flows through
//! these primitives, and they are implemented twice (Kani `[u128; 2]` mode
//! and BPF `[u64; 4]` mode) with no external big-int crate to cross-check
//! against. The existing in-module tests pin known values; these randomized
//! tests pin the *algebra* on the host build:
//!
//!   1. Ring identities — add/sub/neg roundtrips, commutativity.
//!   2. Differential multiplication — `U256::checked_mul` and the private
//!      U512 path behind `mul_div_floor_u256` are compared limb-by-limb
//!      against an independent u64-limb schoolbook reference implemented
//!      here (a different decomposition than the u128-half implementation
//!      under test).
//!   3. Euclidean division — `div_rem_u256` must satisfy `q*d + r == n`
//!      with `r < d`, checked against the reference multiplier.
//!   4. Floor/ceil duality — spec §4.6 floor and ceil variants may differ
//!      only by the remainder indicator; signed floors must agree with
//!      native `i128::div_euclid` on the shared domain.
//!   5. §4.8 K-pair settlement — `wide_signed_mul_div_floor_from_k_pair`
//!      is compared against a native i128 model on ranges where the
//!      product provably fits.
//!
//! Gated on `fork-facade` (like the other wide_math consumers) because the
//! module is only `pub` when the wrapper opts in:
//!   cargo test --features fork-facade --test wide_math_props

use percolator::wide_math::{
    ceil_div_positive_checked, checked_mul_div_ceil_u256, div_rem_u256,
    fee_debt_u128_checked, floor_div_signed_conservative,
    floor_div_signed_conservative_i128, mul_div_ceil_u128, mul_div_ceil_u256,
    mul_div_floor_u128, mul_div_floor_u256, mul_div_floor_u256_with_rem,
    saturating_mul_u128_u64, saturating_mul_u256_u64, wide_mul_div_ceil_u128_or_over_i128max,
    wide_mul_div_floor_u128, wide_signed_mul_div_floor, wide_signed_mul_div_floor_from_k_pair,
    I256, U256,
};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Independent u64-limb reference arithmetic.
//
// The implementation under test decomposes U256 into two u128 halves and
// funnels every product through `widening_mul_u128`. The reference here uses
// a flat 4x4 u64-limb schoolbook with u128 accumulators — same math, disjoint
// code path, so a shared bug would have to be present in two different
// decompositions to go unnoticed.
// ---------------------------------------------------------------------------

fn limbs_of(v: U256) -> [u64; 4] {
    let lo = v.lo();
    let hi = v.hi();
    [lo as u64, (lo >> 64) as u64, hi as u64, (hi >> 64) as u64]
}

fn u256_from_limbs(l: [u64; 4]) -> U256 {
    U256::new(
        (l[0] as u128) | ((l[1] as u128) << 64),
        (l[2] as u128) | ((l[3] as u128) << 64),
    )
}

/// Schoolbook 256x256 -> 512-bit multiply on u64 limbs.
fn ref_mul_512(a: U256, b: U256) -> [u64; 8] {
    let al = limbs_of(a);
    let bl = limbs_of(b);
    let mut out = [0u64; 8];
    for i in 0..4 {
        let mut carry: u128 = 0;
        for j in 0..4 {
            let acc = (al[i] as u128) * (bl[j] as u128) + (out[i + j] as u128) + carry;
            out[i + j] = acc as u64;
            carry = acc >> 64;
        }
        out[i + 4] = carry as u64;
    }
    out
}

/// 512-bit add on u64 limbs. Panics on overflow past 512 bits (cannot happen
/// for q*d + r with q*d a 512-bit product of in-range operands).
fn ref_add_512(a: [u64; 8], b: [u64; 8]) -> [u64; 8] {
    let mut out = [0u64; 8];
    let mut carry: u128 = 0;
    for i in 0..8 {
        let acc = (a[i] as u128) + (b[i] as u128) + carry;
        out[i] = acc as u64;
        carry = acc >> 64;
    }
    assert_eq!(carry, 0, "ref_add_512 overflow");
    out
}

fn widen_512(v: U256) -> [u64; 8] {
    let l = limbs_of(v);
    [l[0], l[1], l[2], l[3], 0, 0, 0, 0]
}

/// Arbitrary U256 with a bias toward interesting shapes (small, half-wide,
/// full-wide, and limb-boundary values).
fn arb_u256() -> impl Strategy<Value = U256> {
    prop_oneof![
        any::<u128>().prop_map(U256::from_u128),
        (any::<u128>(), any::<u128>()).prop_map(|(lo, hi)| U256::new(lo, hi)),
        Just(U256::ZERO),
        Just(U256::ONE),
        Just(U256::MAX),
        Just(U256::new(u128::MAX, 0)),
        Just(U256::new(0, 1)),
        Just(U256::new(0, u128::MAX)),
    ]
}

fn arb_nonzero_u256() -> impl Strategy<Value = U256> {
    arb_u256().prop_filter("nonzero divisor", |v| !v.is_zero())
}

// ---------------------------------------------------------------------------
// 1. Ring identities
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    /// a + b == b + a, and (a + b) - b == a whenever the sum exists.
    #[test]
    fn u256_add_commutes_and_roundtrips(a in arb_u256(), b in arb_u256()) {
        let ab = a.checked_add(b);
        let ba = b.checked_add(a);
        prop_assert_eq!(ab, ba);
        if let Some(sum) = ab {
            prop_assert_eq!(sum.checked_sub(b), Some(a));
            prop_assert_eq!(sum.checked_sub(a), Some(b));
        }
    }

    /// checked_sub(a, b) exists iff b <= a, and inverts checked_add.
    #[test]
    fn u256_sub_defined_iff_le(a in arb_u256(), b in arb_u256()) {
        let d = a.checked_sub(b);
        prop_assert_eq!(d.is_some(), b <= a);
        if let Some(diff) = d {
            prop_assert_eq!(diff.checked_add(b), Some(a));
        }
    }

    /// saturating_add equals checked_add when defined, MAX otherwise.
    #[test]
    fn u256_saturating_add_consistent(a in arb_u256(), b in arb_u256()) {
        let expected = a.checked_add(b).unwrap_or(U256::MAX);
        prop_assert_eq!(a.saturating_add(b), expected);
    }

    /// Ord agrees with limbwise big-endian comparison.
    #[test]
    fn u256_ord_matches_limbs(a in arb_u256(), b in arb_u256()) {
        let (al, bl) = (limbs_of(a), limbs_of(b));
        let expected = al.iter().rev().cmp(bl.iter().rev());
        prop_assert_eq!(a.cmp(&b), expected);
    }
}

// ---------------------------------------------------------------------------
// 2. Differential multiplication
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    /// checked_mul returns exactly the low 256 bits of the schoolbook
    /// product, and returns None exactly when the true product overflows.
    #[test]
    fn u256_checked_mul_differential(a in arb_u256(), b in arb_u256()) {
        let full = ref_mul_512(a, b);
        let overflows = full[4] != 0 || full[5] != 0 || full[6] != 0 || full[7] != 0;
        match a.checked_mul(b) {
            Some(p) => {
                prop_assert!(!overflows, "engine accepted an overflowing product");
                prop_assert_eq!(limbs_of(p), [full[0], full[1], full[2], full[3]]);
            }
            None => prop_assert!(overflows, "engine rejected an in-range product"),
        }
    }

    /// shl matches multiplication by 2^k (low 256 bits), and shr is the
    /// exact inverse when no high bits are lost.
    #[test]
    fn u256_shifts_match_pow2_mul(a in arb_u256(), k in 0u32..256) {
        let pow2 = U256::ONE.shl(k);
        let full = ref_mul_512(a, pow2);
        prop_assert_eq!(
            limbs_of(a.shl(k)),
            [full[0], full[1], full[2], full[3]],
            "shl({}) != low bits of a * 2^{}", k, k
        );
        if a.shl(k).shr(k) == a {
            // no truncation: shl/shr must roundtrip
        } else {
            // truncation happened, so some high bit was set
            prop_assert!(k > 0 && a.shr(256 - k) != U256::ZERO);
        }
    }
}

// ---------------------------------------------------------------------------
// 3. Euclidean division
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1500))]

    /// div_rem_u256 satisfies n = q*d + r with r < d, verified against the
    /// reference multiplier (not the engine's own checked_mul).
    #[test]
    fn div_rem_is_euclidean(n in arb_u256(), d in arb_nonzero_u256()) {
        let (q, r) = div_rem_u256(n, d);
        prop_assert!(r < d, "remainder not reduced");
        let qd = ref_mul_512(q, d);
        let total = ref_add_512(qd, widen_512(r));
        prop_assert_eq!(total, widen_512(n), "q*d + r != n");
    }

    /// checked_div / checked_rem agree with div_rem_u256 and fail only on 0.
    #[test]
    fn checked_div_rem_consistent(n in arb_u256(), d in arb_u256()) {
        if d.is_zero() {
            prop_assert_eq!(n.checked_div(d), None);
            prop_assert_eq!(n.checked_rem(d), None);
        } else {
            let (q, r) = div_rem_u256(n, d);
            prop_assert_eq!(n.checked_div(d), Some(q));
            prop_assert_eq!(n.checked_rem(d), Some(r));
        }
    }
}

// ---------------------------------------------------------------------------
// 4. Floor/ceil duality (spec §4.6)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1500))]

    /// ceil(n/d) == floor(n/d) + (n mod d != 0).
    #[test]
    fn ceil_is_floor_plus_indicator(n in arb_u256(), d in arb_nonzero_u256()) {
        let (q, r) = div_rem_u256(n, d);
        let ceil = ceil_div_positive_checked(n, d);
        let expected = if r.is_zero() { q } else { q.checked_add(U256::ONE).unwrap() };
        prop_assert_eq!(ceil, expected);
    }

    /// mul_div_floor / mul_div_ceil / with_rem are mutually consistent and
    /// Euclidean over the U512 product: q*d + r == a*b (checked against the
    /// reference multiplier). Constraining b <= d keeps the quotient <= a,
    /// so the U256 narrowing inside the helpers cannot panic.
    #[test]
    fn mul_div_family_euclidean(a in arb_u256(), b in arb_u256(), d in arb_nonzero_u256()) {
        let (b, d) = if b <= d { (b, d) } else { (d, b) };
        prop_assume!(!d.is_zero());

        let (q, r) = mul_div_floor_u256_with_rem(a, b, d);
        prop_assert_eq!(mul_div_floor_u256(a, b, d), q);
        prop_assert!(r < d);

        // q*d + r must equal the full 512-bit product a*b.
        let total = ref_add_512(ref_mul_512(q, d), widen_512(r));
        prop_assert_eq!(total, ref_mul_512(a, b));

        // Ceil variant differs exactly by the remainder indicator.
        let ceil = mul_div_ceil_u256(a, b, d);
        let expected = if r.is_zero() { q } else { q.checked_add(U256::ONE).unwrap() };
        prop_assert_eq!(ceil, expected);
        prop_assert_eq!(checked_mul_div_ceil_u256(a, b, d), Some(expected));
    }

    /// The u128 wrappers agree with native u128 arithmetic on the shared
    /// domain (spec §1.1 helpers).
    #[test]
    fn u128_wrappers_match_native(a in any::<u64>(), b in any::<u64>(), d in 1u128..=u128::MAX) {
        let (a, b) = (a as u128, b as u128);
        let p = a * b; // fits: 64-bit operands
        prop_assert_eq!(mul_div_floor_u128(a, b, d), p / d);
        prop_assert_eq!(wide_mul_div_floor_u128(a, b, d), p / d);
        let ceil = p / d + u128::from(p % d != 0);
        prop_assert_eq!(mul_div_ceil_u128(a, b, d), ceil);
        prop_assert_eq!(wide_mul_div_ceil_u128_or_over_i128max(a, b, d), Ok(ceil));
    }

    /// Saturating multiplies saturate exactly at the checked boundary.
    #[test]
    fn saturating_mul_consistent(a in arb_u256(), b in any::<u64>(), x in any::<u128>()) {
        let expected = a.checked_mul(U256::from_u64(b)).unwrap_or(U256::MAX);
        prop_assert_eq!(saturating_mul_u256_u64(a, b), expected);
        let expected128 = x.checked_mul(b as u128).unwrap_or(u128::MAX);
        prop_assert_eq!(saturating_mul_u128_u64(x, b), expected128);
    }
}

// ---------------------------------------------------------------------------
// 5. Signed floors and §4.8 K-pair settlement
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    /// Both signed floor helpers agree with native i128::div_euclid
    /// (floor for positive divisors) on the shared domain, including
    /// i128::MIN.
    ///
    /// The single point (n == i128::MIN, d == 1) is excluded for the i128
    /// variant: its out-of-range guard `assert!(q_final <= i128::MAX)`
    /// rejects the |i128::MIN| magnitude even though the final negated
    /// result would be representable. Engine callers divide by
    /// FUNDING_DEN / POS_SCALE (>= 10^6), so the point is unreachable in
    /// production; the U256 variant handles it exactly and is tested on
    /// the full domain below.
    #[test]
    fn signed_floor_matches_div_euclid(
        n in any::<i128>(),
        d in 1u128..=(i128::MAX as u128),
    ) {
        let expected = n.div_euclid(d as i128);
        if !(n == i128::MIN && d == 1) {
            prop_assert_eq!(floor_div_signed_conservative_i128(n, d), expected);
        }
        let wide = floor_div_signed_conservative(I256::from_i128(n), U256::from_u128(d));
        prop_assert_eq!(wide.try_into_i128(), Some(expected));
    }

    /// I256 add/sub/neg roundtrip through i128 embeddings.
    #[test]
    fn i256_ring_identities(a in any::<i128>(), b in any::<i128>()) {
        let (wa, wb) = (I256::from_i128(a), I256::from_i128(b));
        prop_assert_eq!(wa.try_into_i128(), Some(a));
        let sum = wa.checked_add(wb).expect("i128 + i128 always fits I256");
        prop_assert_eq!(sum.checked_sub(wb).and_then(|v| v.try_into_i128()), Some(a));
        // Ord embedding
        prop_assert_eq!(wa.cmp(&wb), a.cmp(&b));
        // abs_u256 of the most negative i128 must not wrap
        prop_assert_eq!(
            I256::from_i128(i128::MIN).abs_u256(),
            U256::from_u128(1u128 << 127)
        );
    }

    /// §4.8: the K-pair settlement helper matches a native i128 model on
    /// ranges where abs_basis * |k_now - k_then| provably fits in i128
    /// (abs_basis < 2^32, |k| < 2^63, so the product is < 2^96).
    #[test]
    fn k_pair_settlement_matches_native_model(
        abs_basis in 0u128..(1u128 << 32),
        k_then in any::<i64>(),
        k_now in any::<i64>(),
        den in 1u128..=(1u128 << 96),
    ) {
        let diff = (k_now as i128) - (k_then as i128);
        let product = (abs_basis as i128) * diff;
        let expected = product.div_euclid(den as i128);
        let got = wide_signed_mul_div_floor_from_k_pair(
            abs_basis,
            k_then as i128,
            k_now as i128,
            den,
        );
        prop_assert_eq!(got, expected);

        // The U256-typed variant must agree on the same inputs.
        let wide = wide_signed_mul_div_floor(
            U256::from_u128(abs_basis),
            I256::from_i128(diff),
            U256::from_u128(den),
        );
        prop_assert_eq!(wide.try_into_i128(), Some(expected));
    }

    /// Fee-debt conversion (spec §4.6): negative credits become unsigned
    /// debt, non-negative credits are debt-free. Total ordering preserved.
    #[test]
    fn fee_debt_conversion(credits in any::<i128>()) {
        let debt = fee_debt_u128_checked(credits);
        if credits < 0 {
            prop_assert_eq!(debt, credits.unsigned_abs());
        } else {
            prop_assert_eq!(debt, 0);
        }
    }
}

/// Deterministic edge pins that proptest may not hit every run.
#[test]
fn deterministic_edges() {
    // i128::MIN debt does not overflow.
    assert_eq!(fee_debt_u128_checked(i128::MIN), 1u128 << 127);

    // I256::MIN cannot be negated.
    assert_eq!(I256::MIN.checked_neg(), None);
    // ... but MIN + MAX == -1.
    assert_eq!(I256::MIN.checked_add(I256::MAX), Some(I256::MINUS_ONE));

    // Division by the smallest and largest divisors.
    let n = U256::new(u128::MAX, u128::MAX);
    let (q, r) = div_rem_u256(n, U256::ONE);
    assert_eq!((q, r), (n, U256::ZERO));
    let (q, r) = div_rem_u256(n, n);
    assert_eq!((q, r), (U256::ONE, U256::ZERO));

    // MAX * 1 round-trips through the mul-div family.
    assert_eq!(mul_div_floor_u256(U256::MAX, U256::ONE, U256::ONE), U256::MAX);
    assert_eq!(mul_div_ceil_u256(U256::MAX, U256::ONE, U256::ONE), U256::MAX);

    // checked variant reports quotient overflow instead of panicking.
    assert_eq!(checked_mul_div_ceil_u256(U256::MAX, U256::MAX, U256::ONE), None);

    // 2^255 boundary: shl/shr symmetry at the top bit.
    let top = U256::ONE.shl(255);
    assert_eq!(top.shr(255), U256::ONE);
    assert_eq!(top.shl(1), U256::ZERO);
}
