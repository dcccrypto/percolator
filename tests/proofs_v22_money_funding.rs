#![cfg(kani)]
//! Money paths 2026-10-14 (LP-audit item 9): skew / premium funding is ZERO-SUM through the REAL
//! accrual kernel. Run ONCE on final code.
//!
//! FINDING that shapes this file: the `kani_p3e_skew_funding_*` harnesses (percolator-prog
//! kani/p3-engine) model the PRE-v21 funding path (`fid = floor(rate*dt*price / 1e9)` fed to
//! `kernel_adl_scaled_accrual_index_deltas`). Production accrual (src/v16.rs:16993-17011) now
//! passes funding 0 to that kernel and computes the F deltas with `kernel_funding_index_deltas` on
//! the UNFLOORED numerator (payer ceil, receiver floor). Those p3e rows prove an obsolete formula.
//!
//! Zero-sum chain for the CURRENT code, `n = rate*dt*price`, `m = |n|`, `D = FUNDING_DEN`,
//! `P = POS_SCALE`, two matched legs of basis `q` (each side's a_basis = that side's live A):
//!   (K) kernel, this file, FULL width, REAL `kernel_funding_index_deltas`: payer index move
//!       `pay` with `pay*D - m*a_pay in [0, D)` (ceil), receiver `recv` with
//!       `m*a_recv - recv*D in [0, D)` (floor), signs, Ok, fast path exact.
//!   (S) settlement: each leg realises `floor(q * dF / (a * P))` (Euclidean, carry 0):
//!       `wide_signed_mul_div_floor_with_carry_from_k_pair` (U256) -- native proptest only
//!       (tests/v22_money_paths_funding_native.rs); no Kani verdict (256-bit long division).
//!   (P) paper (ordered ring): long `L*(a_l*P) <= -q*pay` and `pay*D >= m*a_l` give
//!       `L*P*D <= -q*m` (cancel a_l > 0); short `S*(a_s*P) <= q*recv`, `recv*D <= m*a_s` give
//!       `S*P*D <= q*m`; adding, `(L+S)*P*D <= 0`, so `L + S <= 0`: funding never mints. Native
//!       proptest checks the composed chain on the real kernel + real settlement.
use percolator::v16::kani_funding_index_deltas;
use percolator::wide_math::U256;
use percolator::{ADL_ONE, FUNDING_DEN};

/// `funding_div_rem(p)` is literally `(p / FUNDING_DEN, p % FUNDING_DEN)` (src/v16.rs:4365-4367).
/// Euclidean division by a non-zero constant is UNIQUE, so this relation model returns exactly
/// the real pair (definitional: Rust u128 `/` `%`); `proof_v22_review_funding_witnesses` (kills
/// FUND-D1, a remainder-dropping body) guards the body. `q < 2^99` follows from `q*D <= p < 2^128`.
fn funding_div_rem_euclid(p: u128) -> (u128, u128) {
    let q: u128 = kani::any();
    let r: u128 = kani::any();
    kani::assume(r < FUNDING_DEN);
    kani::assume(q < (1u128 << 99));
    kani::assume(q.checked_mul(FUNDING_DEN).and_then(|x| x.checked_add(r)) == Some(p));
    (q, r)
}

/// The U256 fallback runs only when `m * a` overflows u128; no assertion below is made on that
/// branch, so an ARBITRARY result is a sound over-approximation of it.
fn u256_fallback_any(_a: U256, _b: U256, _d: U256) -> (U256, U256) {
    (U256::from_u128(kani::any()), U256::from_u128(kani::any()))
}

/// (K) for EVERY funding numerator `n` and every `0 < a_long, a_short <= ADL_ONE` such that the
/// payer's and receiver's `m * a` fit u128 (production |n| <= ~2^77 and a <= 2^49.9, so this is
/// the production path; the U256 fallback is outside the claim).
#[kani::proof]
#[kani::stub(percolator::v16::funding_div_rem, funding_div_rem_euclid)]
#[kani::stub(percolator::wide_math::mul_div_floor_u256_with_rem, u256_fallback_any)]
#[kani::solver(z3)]
fn proof_v22_money_funding_kernel_rounds_against_the_payer_full() {
    let n: i128 = kani::any();
    let a_long: u128 = kani::any();
    let a_short: u128 = kani::any();
    kani::assume(a_long > 0 && a_long <= ADL_ONE);
    kani::assume(a_short > 0 && a_short <= ADL_ONE);
    let r = kani_funding_index_deltas(n, a_long, a_short);
    if n == 0 {
        assert!(r == Ok((0, 0)), "no funding, no move");
        return;
    }
    let m = n.unsigned_abs();
    let (a_pay, a_recv) = if n > 0 { (a_long, a_short) } else { (a_short, a_long) };
    let (Some(pp), Some(pr)) = (m.checked_mul(a_pay), m.checked_mul(a_recv)) else {
        kani::cover!(true, "COVER: U256 fallback range (outside the claim)");
        return;
    };
    assert!(r.is_ok(), "production range never errors");
    let (f_long, f_short) = r.unwrap();
    let (pay_s, recv_s) = if n > 0 { (f_long, f_short) } else { (f_short, f_long) };
    assert!(pay_s <= 0 && recv_s >= 0, "the payer side is debited, the receiver credited");
    let pay = pay_s.unsigned_abs();
    let recv = recv_s as u128;
    let pd = pay.checked_mul(FUNDING_DEN);
    let rd = recv.checked_mul(FUNDING_DEN);
    assert!(pd.is_some() && rd.is_some());
    let (pd, rd) = (pd.unwrap(), rd.unwrap());
    assert!(pd >= pp && pd - pp < FUNDING_DEN, "payer: ceil(m * a_pay / D)");
    assert!(rd <= pr && pr - rd < FUNDING_DEN, "receiver: floor(m * a_recv / D)");
    kani::cover!(n > 0 && a_long < ADL_ONE && pd > pp, "COVER: longs pay, shrunk A, rounded up");
    kani::cover!(n < 0 && a_short < ADL_ONE && pd > pp, "COVER: shorts pay, shrunk A, rounded up");
    kani::cover!(a_pay == ADL_ONE && a_recv < ADL_ONE && rd < pr, "COVER: fast-path payer, floored receiver");
    kani::cover!(m >= (1u128 << 77) && a_pay < ADL_ONE, "COVER: top of the production numerator range");
}
