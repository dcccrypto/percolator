#![cfg(kani)]

use percolator::wide_math::{
    ceil_div_positive_checked, floor_div_signed_conservative_i128, mul_div_ceil_u256,
    mul_div_floor_u256, mul_div_floor_u256_with_rem, wide_signed_mul_div_floor,
    wide_signed_mul_div_floor_from_k_pair, I256, U256,
};

fn small_signed_floor_reference(n: i128, d: u128) -> i128 {
    if n >= 0 {
        (n as u128 / d) as i128
    } else {
        let abs = n.unsigned_abs();
        let q = abs / d;
        let r = abs % d;
        -((q + u128::from(r != 0)) as i128)
    }
}

#[kani::proof]
#[kani::unwind(20)]
#[kani::solver(cadical)]
fn proof_v15_floor_div_signed_conservative_matches_small_reference() {
    let n_raw: i16 = kani::any();
    let d_raw: u8 = kani::any();
    kani::assume((-500..=500).contains(&n_raw));
    kani::assume((1..=50).contains(&d_raw));

    let n = n_raw as i128;
    let d = d_raw as u128;
    let got = floor_div_signed_conservative_i128(n, d);
    let expected = small_signed_floor_reference(n, d);

    kani::cover!(
        n < 0 && n.unsigned_abs() % d != 0,
        "negative rounded-down branch"
    );
    kani::cover!(n >= 0, "nonnegative floor branch");
    assert_eq!(got, expected);
}

#[kani::proof]
#[kani::unwind(20)]
#[kani::solver(cadical)]
fn proof_v15_mul_div_floor_u256_matches_small_reference() {
    let a_raw: u8 = kani::any();
    let b_raw: u8 = kani::any();
    let d_raw: u8 = kani::any();
    kani::assume(a_raw <= 40);
    kani::assume(b_raw <= 40);
    kani::assume((1..=40).contains(&d_raw));

    let a = a_raw as u128;
    let b = b_raw as u128;
    let d = d_raw as u128;
    let got = mul_div_floor_u256(U256::from_u128(a), U256::from_u128(b), U256::from_u128(d));

    assert_eq!(got.try_into_u128(), Some((a * b) / d));
}

#[kani::proof]
#[kani::unwind(20)]
#[kani::solver(cadical)]
fn proof_v15_mul_div_ceil_u256_is_floor_plus_remainder_indicator() {
    let a_raw: u8 = kani::any();
    let b_raw: u8 = kani::any();
    let d_raw: u8 = kani::any();
    kani::assume(a_raw <= 40);
    kani::assume(b_raw <= 40);
    kani::assume((1..=40).contains(&d_raw));

    let a = U256::from_u128(a_raw as u128);
    let b = U256::from_u128(b_raw as u128);
    let d = U256::from_u128(d_raw as u128);
    let (floor, rem) = mul_div_floor_u256_with_rem(a, b, d);
    let ceil = mul_div_ceil_u256(a, b, d);
    let floor_u128 = floor.try_into_u128().unwrap();
    let rem_u128 = rem.try_into_u128().unwrap();
    let expected = if rem_u128 == 0 {
        floor_u128
    } else {
        floor_u128 + 1
    };

    kani::cover!(rem_u128 == 0, "exact mul-div branch");
    kani::cover!(rem_u128 != 0, "ceil adds one branch");
    assert_eq!(ceil.try_into_u128(), Some(expected));
}

#[kani::proof]
#[kani::unwind(20)]
#[kani::solver(cadical)]
fn proof_v15_ceil_div_positive_checked_matches_small_reference() {
    let n_raw: u8 = kani::any();
    let d_raw: u8 = kani::any();
    kani::assume(n_raw <= 80);
    kani::assume((1..=40).contains(&d_raw));

    let n = n_raw as u128;
    let d = d_raw as u128;
    let got = ceil_div_positive_checked(U256::from_u128(n), U256::from_u128(d));
    let expected = n / d + u128::from(n % d != 0);

    assert_eq!(got.try_into_u128(), Some(expected));
}

#[kani::proof]
#[kani::unwind(20)]
#[kani::solver(cadical)]
fn proof_v15_wide_signed_mul_div_floor_matches_small_reference() {
    let abs_basis_raw: u8 = kani::any();
    let k_diff_raw: i8 = kani::any();
    let den_raw: u8 = kani::any();
    kani::assume(abs_basis_raw <= 16);
    kani::assume((-16..=16).contains(&k_diff_raw));
    kani::assume((1..=16).contains(&den_raw));

    let abs_basis = abs_basis_raw as u128;
    let k_diff = k_diff_raw as i128;
    let den = den_raw as u128;
    let got = wide_signed_mul_div_floor(
        U256::from_u128(abs_basis),
        I256::from_i128(k_diff),
        U256::from_u128(den),
    );
    let expected = small_signed_floor_reference(abs_basis as i128 * k_diff, den);

    kani::cover!(k_diff < 0, "negative wide signed branch");
    kani::cover!(k_diff > 0, "positive wide signed branch");
    assert_eq!(got.try_into_i128(), Some(expected));
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v15_k_pair_mul_div_floor_matches_small_reference() {
    let abs_basis_raw: u8 = kani::any();
    let k_then_raw: i8 = kani::any();
    let k_now_raw: i8 = kani::any();
    let den_raw: u8 = kani::any();
    kani::assume(abs_basis_raw <= 16);
    kani::assume((-16..=16).contains(&k_then_raw));
    kani::assume((-16..=16).contains(&k_now_raw));
    kani::assume((1..=16).contains(&den_raw));

    let abs_basis = abs_basis_raw as u128;
    let k_then = k_then_raw as i128;
    let k_now = k_now_raw as i128;
    let den = den_raw as u128;
    let got = wide_signed_mul_div_floor_from_k_pair(abs_basis, k_then, k_now, den);
    let expected = small_signed_floor_reference(abs_basis as i128 * (k_now - k_then), den);

    kani::cover!(k_now < k_then, "negative K-diff pair branch");
    kani::cover!(k_now > k_then, "positive K-diff pair branch");
    assert_eq!(got, expected);
}

#[kani::proof]
#[kani::unwind(20)]
#[kani::solver(cadical)]
fn proof_v15_k_pair_zero_cases_return_zero() {
    let den_raw: u8 = kani::any();
    kani::assume((1..=40).contains(&den_raw));
    let den = den_raw as u128;

    assert_eq!(wide_signed_mul_div_floor_from_k_pair(0, -7, 11, den), 0);
    assert_eq!(wide_signed_mul_div_floor_from_k_pair(9, 3, 3, den), 0);
}
