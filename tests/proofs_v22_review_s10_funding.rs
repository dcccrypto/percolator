#![cfg(kani)]
//! Kani review 2026-10-14, lane engine-s10-funding. PROOF-ONLY FILE (no production change).
//!
//! The funding H2 decomposition (S + R + G + L, `tests/proofs_v22_overnight_funding.rs`) stubs
//! `funding_div_rem` in every harness that touches production code, so a defect INSIDE the divider
//! (e.g. `p % FUNDING_DEN` replaced by 0: the payer never rounds up) is visible only to L, which
//! has never finished (cadical 8001 s, kissat 900 s). The harnesses below run the REAL divider:
//!   * `proof_v22_review_funding_witnesses`: concrete points (a test-strength oracle, seconds);
//!   * `proof_v22_review_funding_rounding_bounded_real_divider`: BOUNDED (|n| < 2^16, A within
//!     2^16 of ADL_ONE on each side, both signs), the full H2 statement on the real code.
//! Neither replaces H2 on its full domain; they exist to make funding-rounding mutants decisive.
#![allow(dead_code, unused_imports)]

use percolator::v16::kani_funding_index_deltas;
use percolator::{ADL_ONE, FUNDING_DEN};

/// Concrete witnesses, both signs, payer rounding up, receiver rounding down, exact division,
/// fast path. Values computed by hand (DEN = 1e9, ADL_ONE = 1e15):
///   mag 3, a = ADL_ONE - 1: p = 2_999_999_999_999_997 = 2_999_999 * 1e9 + 999_999_997
///     => payer 3_000_000 (ceil), receiver 2_999_999 (floor)
///   mag 2, a = 5e14: p = 1e15 = 1_000_000 * 1e9 exactly => both 1_000_000
///   mag 7, a = ADL_ONE: fast path 7 * 1e6 = 7_000_000 on both sides
#[kani::proof]
#[kani::solver(kissat)]
fn proof_v22_review_funding_witnesses() {
    const A1: u128 = ADL_ONE - 1;
    // longs pay (n > 0), both sides shrunk equally
    assert_eq!(kani_funding_index_deltas(3, A1, A1), Ok((-3_000_000, 2_999_999)));
    // shorts pay (n < 0): mirrored
    assert_eq!(kani_funding_index_deltas(-3, A1, A1), Ok((2_999_999, -3_000_000)));
    // exact division: no rounding either way
    assert_eq!(kani_funding_index_deltas(2, 500_000_000_000_000, 500_000_000_000_000), Ok((-1_000_000, 1_000_000)));
    // fast path, balanced book
    assert_eq!(kani_funding_index_deltas(7, ADL_ONE, ADL_ONE), Ok((-7_000_000, 7_000_000)));
    assert_eq!(kani_funding_index_deltas(-7, ADL_ONE, ADL_ONE), Ok((7_000_000, -7_000_000)));
    // asymmetric: payer on the fast path, receiver rounded down
    assert_eq!(kani_funding_index_deltas(3, ADL_ONE, A1), Ok((-3_000_000, 2_999_999)));
    // asymmetric: payer shrunk (rounded up), receiver on the fast path
    assert_eq!(kani_funding_index_deltas(-3, ADL_ONE, A1), Ok((3_000_000, -3_000_000)));
    assert_eq!(kani_funding_index_deltas(0, A1, A1), Ok((0, 0)));
}

/// BOUNDED: 0 < |n| < 2^16, a_side = ADL_ONE - k_side with k_side < 2^16 (so a in
/// (ADL_ONE - 2^16, ADL_ONE], including the fast path k = 0), both signs. On the REAL code
/// (no stub): Ok; payer move = -ceil(|n| a_pay / DEN), receiver move = floor(|n| a_recv / DEN)
/// (stated without division); equal A => 0 <= pay - recv <= 1.
#[kani::proof]
#[kani::solver(kissat)]
fn proof_v22_review_funding_rounding_bounded_real_divider() {
    let m16: u16 = kani::any();
    let kl: u16 = kani::any();
    let ks: u16 = kani::any();
    let longs_pay: bool = kani::any();
    kani::assume(m16 > 0);
    let mag = m16 as u128;
    let (a_long, a_short) = (ADL_ONE - kl as u128, ADL_ONE - ks as u128);
    let n = if longs_pay { mag as i128 } else { -(mag as i128) };
    let r = kani_funding_index_deltas(n, a_long, a_short);
    assert!(r.is_ok());
    let (f_long, f_short) = r.unwrap();
    let (pay_s, recv_s, a_pay, a_recv) =
        if longs_pay { (f_long, f_short, a_long, a_short) } else { (f_short, f_long, a_short, a_long) };
    assert!(pay_s <= 0 && recv_s >= 0);
    let pay = pay_s.unsigned_abs();
    let recv = recv_s as u128;
    let pp = mag * a_pay;
    let pr = mag * a_recv;
    assert!(pay * FUNDING_DEN >= pp, "payer never underpays its exact share");
    assert!(pay * FUNDING_DEN < pp + FUNDING_DEN, "payer overpays by < 1 index unit");
    assert!(recv * FUNDING_DEN <= pr, "receiver never over-receives");
    assert!(pr < recv * FUNDING_DEN + FUNDING_DEN, "receiver short by < 1 index unit");
    if a_pay == a_recv {
        assert!(pay >= recv && pay - recv <= 1);
    }
    kani::cover!(a_pay < ADL_ONE && pay * FUNDING_DEN > pp, "payer rounded up");
    kani::cover!(a_recv < ADL_ONE && recv * FUNDING_DEN < pr, "receiver rounded down");
    kani::cover!(a_pay == a_recv && a_pay < ADL_ONE && pay == recv + 1, "gap of one");
    kani::cover!(a_pay == ADL_ONE && a_recv == ADL_ONE, "fast path both sides");
    kani::cover!(!longs_pay && a_pay < ADL_ONE, "shorts pay, rounded");
}
