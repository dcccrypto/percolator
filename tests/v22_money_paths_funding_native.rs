//! Money paths 2026-10-14 (LP-audit item 9), NATIVE twin of tests/proofs_v22_money_funding.rs.
//! Funding is ZERO-SUM through the REAL accrual kernel AND the REAL settlement:
//!   n = rate * dt * price (production headroom: |n| <= cap * slots * MAX_ORACLE_PRICE, with
//!   ADL_ONE * MAX_ORACLE_PRICE * cap * slots <= i128::MAX, v16.rs:5003-5015);
//!   (f_long, f_short) = kernel_funding_index_deltas(n, a_long, a_short)   (real, fuzz-gated shim);
//!   each matched leg of basis q settles floor(q * dF / (a_side * POS_SCALE)) with carry 0 through
//!   the real `wide_signed_mul_div_floor_with_carry_from_k_pair` (the U256 path every A < ADL_ONE leg takes).
//! Asserted: long + short <= 0 (funding never mints) and the dust bound derived in the Kani file
//! header (net > -2 - q/(a_long*P) - q/(a_short*P)). Run: cargo test --features fuzz --test v22_money_paths_funding_native
#![cfg(feature = "fuzz")]
use percolator::wide_math::wide_signed_mul_div_floor_with_carry_from_k_pair as settle;
use percolator::{ADL_ONE, MAX_ORACLE_PRICE, MIN_A_SIDE, POS_SCALE};
use proptest::prelude::*;

fn leg(q: u128, df: i128, a: u128) -> i128 {
    settle(q, 0, df, a * POS_SCALE, 0).0
}

fn check(n: i128, a_long: u128, a_short: u128, q: u128) -> Result<(), TestCaseError> {
    let (fl, fs) = percolator::kani_funding_index_deltas(n, a_long, a_short).unwrap();
    let long = leg(q, fl, a_long);
    let short = leg(q, fs, a_short);
    let net = long + short;
    prop_assert!(net <= 0, "funding minted: n={n} a=({a_long},{a_short}) q={q} long={long} short={short}");
    // net > -2 - X - Y with X = q/(a_l P), Y = q/(a_s P) (reals) => net >= -(floor X + floor Y + 3)
    let slack = (q / (a_long * POS_SCALE)) as i128 + (q / (a_short * POS_SCALE)) as i128 + 3;
    prop_assert!(net >= -slack, "dust above the bound: net={net} slack={slack}");
    if n > 0 && q > 0 {
        prop_assert!(long <= 0 && short >= 0, "longs pay when the rate is positive");
    }
    if n < 0 && q > 0 {
        prop_assert!(short <= 0 && long >= 0, "shorts pay when the rate is negative");
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    /// Full production ranges: any rate/dt within the headroom, any price, asymmetric A, any u64 basis.
    #[test]
    fn money_funding_zero_sum_real_kernel_real_settlement(
        rate_dt in -(1i128 << 37)..=(1i128 << 37),
        price in 1u64..=MAX_ORACLE_PRICE,
        a_long in MIN_A_SIDE..=ADL_ONE,
        a_short in MIN_A_SIDE..=ADL_ONE,
        q in 0u128..=(u64::MAX as u128),
    ) {
        // the headroom check bounds cap*slots so that ADL_ONE * MAX_ORACLE_PRICE * |rate*dt| <= i128::MAX
        prop_assume!((rate_dt.unsigned_abs()).checked_mul(MAX_ORACLE_PRICE as u128)
            .and_then(|v| v.checked_mul(ADL_ONE)).is_some_and(|v| v <= i128::MAX as u128));
        check(rate_dt * price as i128, a_long, a_short, q)?;
    }

    /// Small funding (the rounding-dominated regime) and small A spread.
    #[test]
    fn money_funding_zero_sum_small_numerators(
        n in -1_000_000_000_000i128..=1_000_000_000_000,
        a_long in MIN_A_SIDE..=ADL_ONE,
        da in 0u128..=1_000,
        q in 0u128..=(1u128 << 40),
    ) {
        let a_short = (a_long + da).min(ADL_ONE);
        check(n, a_long, a_short, q)?;
    }
}

/// Concrete corners: extreme A pair, extreme numerator, extreme basis.
#[test]
fn money_funding_zero_sum_corners() {
    let n_max = (i128::MAX as u128 / ADL_ONE) as i128; // largest |n| the headroom admits
    for &n in &[1i128, -1, 999_999_999, -999_999_999, n_max, -n_max] {
        for &(al, as_) in &[(MIN_A_SIDE, ADL_ONE), (ADL_ONE, MIN_A_SIDE), (MIN_A_SIDE, MIN_A_SIDE), (ADL_ONE - 1, MIN_A_SIDE + 1)] {
            for &q in &[0u128, 1, 999_999, u64::MAX as u128] {
                check(n, al, as_, q).unwrap();
            }
        }
    }
}
