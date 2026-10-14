#![cfg(kani)]
//! v2.2 overnight (2026-10-14) replacements for the funding no-verdict rows of
//! `tests/proofs_v22_funding_exact.rs`. PROOF-ONLY FILE; the originals are kept unchanged.
//!
//! Rows replaced:
//!   * `lemma_v22_funding_div_rem_contract`            (official: NO-VERDICT, 8001 s, cadical)
//!   * `proof_v22_funding_shrunk_a_rounds_against_the_payer_bounded` (NO-VERDICT, 8002 s, cadical)
//!
//! Same statements, same bounds (no narrowing unless a harness says BOUNDED: ...). The only change
//! for the lemma is the SAT backend (kissat; 09-30 diagnostic: kissat clears 128-bit divider
//! lemmas where cadical/minisat time out). The H2 row is decomposed per side (see the comment on
//! `proof_v22_funding_side_*`), so each harness holds ONE symbolic product.
#![allow(dead_code, unused_imports)]

use percolator::v16::{kani_funding_div_rem, kani_funding_index_deltas};
use percolator::wide_math::U256;
use percolator::{ADL_ONE, FUNDING_DEN};

const MAG_BOUND: u128 = 1 << 64;
const P_BOUND: u128 = 1 << 114;
const Q_BOUND: u128 = 1 << 85;

/// L (kissat twin of `lemma_v22_funding_div_rem_contract`). Identical statement and bound.
#[kani::proof]
#[kani::solver(kissat)]
fn lemma_v22_funding_div_rem_contract_kissat() {
    let p: u128 = kani::any();
    kani::assume(p < P_BOUND);
    let (q, r) = kani_funding_div_rem(p);
    assert!(r < FUNDING_DEN);
    assert!(q < Q_BOUND);
    assert!(q * FUNDING_DEN + r == p);
    kani::cover!(r == 0 && p != 0, "exact division");
    kani::cover!(r == FUNDING_DEN - 1, "largest remainder");
    kani::cover!(p >= P_BOUND / 2, "top width band");
}

/// Model of `funding_div_rem`: exactly the contract proven by L (Euclidean division is unique, so
/// this is the real function on p < 2^114, not an over-approximation).
fn funding_div_rem_model(p: u128) -> (u128, u128) {
    assert!(p < P_BOUND, "stays inside the range L proves");
    let q: u128 = kani::any();
    let r: u128 = kani::any();
    kani::assume(r < FUNDING_DEN);
    kani::assume(q < Q_BOUND);
    kani::assume(q * FUNDING_DEN + r == p);
    (q, r)
}

fn u256_fallback_unreachable(_a: U256, _b: U256, _d: U256) -> (U256, U256) {
    assert!(false, "U256 fallback reached with |funding_num| < 2^64 and a <= ADL_ONE");
    (U256::from_u128(0), U256::from_u128(0))
}

// -------------------------------------------------------------------------------------------
// H2 split. `kernel_funding_index_deltas` (v16.rs:2036) computes each side with ONE call of the
// pure `funding_scaled_magnitude(mag, a_side, round_up)`; the payer side rounds up and is
// negated, the receiver side rounds down. The original H2 asserts, for both sides at once:
//   (ok) the call is Ok;  (sign) payer <= 0 <= receiver;
//   (ceil) payer move = ceil(mag * a_pay / DEN);  (floor) receiver move = floor(mag * a_recv / DEN);
//   (gap) a_pay == a_recv  =>  0 <= pay - recv <= 1.
// Split: four side harnesses, one per (tested side in {long, short}) x (payer in {long, short}).
// Each has the SAME full input domain as the original (BOTH a's symbolic, mag < 2^64), asserts
// (ok) and, for the tested side only, its sign and its ceil/floor relation. Over the four, every
// (side, role) pair is asserted on the full domain, so (ok)+(sign)+(ceil)+(floor) of the original
// hold. (gap) is pure arithmetic over the ceil/floor relations at the same product p = mag*a and
// is proven once by `lemma_v22_funding_ceil_floor_gap` over all p < 2^114 (when a_pay == a_recv
// the two relations are about the same p). The original's extra assume (some a < ADL_ONE) is
// DROPPED here (strictly more states), the U256 fallback stays stubbed to a failing assert
// (so the proof still shows it is dead under the bound), and `funding_div_rem` is replaced by
// the contract proven by L (exact model).
// -------------------------------------------------------------------------------------------

fn funding_side_body(test_long: bool, longs_pay: bool) {
    // mag is built from a u64 so its upper 64 bits are structurally zero (bound: mag < 2^64,
    // exactly the original MAG_BOUND)
    let m64: u64 = kani::any();
    let mag = m64 as u128;
    let a_long: u128 = kani::any();
    let a_short: u128 = kani::any();
    kani::assume(mag > 0);
    kani::assume(a_long > 0 && a_long <= ADL_ONE);
    kani::assume(a_short > 0 && a_short <= ADL_ONE);
    let n = if longs_pay { mag as i128 } else { -(mag as i128) };

    let r = kani_funding_index_deltas(n, a_long, a_short);
    assert!(r.is_ok());
    let (f_long, f_short) = r.unwrap();
    let (f, a) = if test_long { (f_long, a_long) } else { (f_short, a_short) };
    let is_payer = test_long == longs_pay;
    let p = mag * a;
    if is_payer {
        assert!(f <= 0);
        let pay = f.unsigned_abs();
        assert!(pay * FUNDING_DEN >= p);
        assert!(pay * FUNDING_DEN < p + FUNDING_DEN);
        kani::cover!(a < ADL_ONE && pay * FUNDING_DEN > p, "payer rounded up");
        kani::cover!(a < ADL_ONE && pay * FUNDING_DEN == p, "payer shrunk A dividing exactly");
    } else {
        assert!(f >= 0);
        let recv = f as u128;
        assert!(recv * FUNDING_DEN <= p);
        assert!(p < recv * FUNDING_DEN + FUNDING_DEN);
        kani::cover!(a < ADL_ONE && recv * FUNDING_DEN < p, "receiver rounded down");
        kani::cover!(a < ADL_ONE && recv * FUNDING_DEN == p, "receiver shrunk A dividing exactly");
    }
    kani::cover!(a == ADL_ONE, "tested side on the exact fast path");
    kani::cover!(mag == MAG_BOUND - 1 && a == ADL_ONE - 1, "top of the bounded range");
}

#[kani::proof]
#[kani::stub(percolator::v16::funding_div_rem, funding_div_rem_model)]
#[kani::stub(percolator::wide_math::mul_div_floor_u256_with_rem, u256_fallback_unreachable)]
#[kani::solver(kissat)]
fn proof_v22_funding_side_long_payer() {
    funding_side_body(true, true);
}

#[kani::proof]
#[kani::stub(percolator::v16::funding_div_rem, funding_div_rem_model)]
#[kani::stub(percolator::wide_math::mul_div_floor_u256_with_rem, u256_fallback_unreachable)]
#[kani::solver(kissat)]
fn proof_v22_funding_side_long_receiver() {
    funding_side_body(true, false);
}

#[kani::proof]
#[kani::stub(percolator::v16::funding_div_rem, funding_div_rem_model)]
#[kani::stub(percolator::wide_math::mul_div_floor_u256_with_rem, u256_fallback_unreachable)]
#[kani::solver(kissat)]
fn proof_v22_funding_side_short_payer() {
    funding_side_body(false, false);
}

#[kani::proof]
#[kani::stub(percolator::v16::funding_div_rem, funding_div_rem_model)]
#[kani::stub(percolator::wide_math::mul_div_floor_u256_with_rem, u256_fallback_unreachable)]
#[kani::solver(kissat)]
fn proof_v22_funding_side_short_receiver() {
    funding_side_body(false, true);
}

/// (gap) lemma: for any p < 2^114, if pay is the ceil and recv the floor of p / DEN (stated
/// without division, exactly as the side harnesses assert them), then 0 <= pay - recv <= 1.
/// pay, recv < 2^85 + 1 is implied by the relations (pay * DEN < p + DEN <= 2^114 + 2^30).
#[kani::proof]
#[kani::solver(kissat)]
fn lemma_v22_funding_ceil_floor_gap() {
    let p: u128 = kani::any();
    let pay: u128 = kani::any();
    let recv: u128 = kani::any();
    kani::assume(p < P_BOUND);
    kani::assume(pay < Q_BOUND + 1 && recv < Q_BOUND + 1); // implied (see doc); keeps * from wrapping
    kani::assume(pay * FUNDING_DEN >= p && pay * FUNDING_DEN < p + FUNDING_DEN);
    kani::assume(recv * FUNDING_DEN <= p && p < recv * FUNDING_DEN + FUNDING_DEN);
    assert!(pay >= recv);
    assert!(pay - recv <= 1);
    kani::cover!(pay == recv + 1, "gap of one");
    kani::cover!(pay == recv && p != 0, "exact");
}

// -------------------------------------------------------------------------------------------
// H2, decomposition v2 (the side harnesses above did not finish in 600 s under z3: they make the
// solver relate the production product `mag * a` and the divider model's `q * DEN + r == p`
// through two multiplier circuits). v2 removes every multiplication from the harness that
// touches production code:
//   (S) `proof_v22_funding_h2_structure_{longs,shorts}_pay`: `funding_div_rem` is replaced by a
//       RECORDING stub that stores the operand p it receives and returns FREE (q, r) with only
//       r < DEN and q < 2^85 (a coarser over-approximation of the real function than the L model:
//       the equation q*DEN + r == p is dropped). The harness proves, for both sides at once and
//       the full original input domain: Ok; each rounded side passes exactly p == mag * a_side to
//       the divider (so the divider sees the design operand); the payer's move is exactly
//       -(q + [r != 0]) and the receiver's exactly q for that side's (q, r); a side with
//       a == ADL_ONE moves by exactly mag * 1e6 (= mag * a / DEN, exact); the U256 fallback is dead.
//   (R) `lemma_v22_funding_ceil_floor_of_div_rem`: pure arithmetic: q*DEN + r == p, r < DEN,
//       p < 2^114 => (q + [r != 0]) is the ceil and q the floor of p / DEN (as H2 states them).
//   (G) `lemma_v22_funding_ceil_floor_gap` (above): equal products => 0 <= pay - recv <= 1.
//   (L) the divider contract q*DEN + r == p, r < DEN, q < 2^85 for (p / DEN, p % DEN), p < 2^114.
// (S) + (L) + (R) give (ok), (sign), (ceil), (floor) of H2 on the full domain; + (G) gives (gap).
// -------------------------------------------------------------------------------------------

static mut DR_CALLS: usize = 0;
static mut DR_P: [u128; 2] = [0; 2];
static mut DR_Q: [u128; 2] = [0; 2];
static mut DR_R: [u128; 2] = [0; 2];

fn funding_div_rem_recorder(p: u128) -> (u128, u128) {
    let q: u128 = kani::any();
    let r: u128 = kani::any();
    kani::assume(r < FUNDING_DEN); // (L)
    kani::assume(q < Q_BOUND); // (L), given p < 2^114 (asserted by the harness below)
    unsafe {
        assert!(DR_CALLS < 2, "at most one divider call per side");
        DR_P[DR_CALLS] = p;
        DR_Q[DR_CALLS] = q;
        DR_R[DR_CALLS] = r;
        DR_CALLS += 1;
    }
    (q, r)
}

fn funding_h2_structure_body(longs_pay: bool) {
    let m64: u64 = kani::any();
    let mag = m64 as u128;
    let a_long: u128 = kani::any();
    let a_short: u128 = kani::any();
    kani::assume(mag > 0);
    kani::assume(a_long > 0 && a_long <= ADL_ONE);
    kani::assume(a_short > 0 && a_short <= ADL_ONE);
    let n = if longs_pay { mag as i128 } else { -(mag as i128) };
    let r = kani_funding_index_deltas(n, a_long, a_short);
    assert!(r.is_ok());
    let (f_long, f_short) = r.unwrap();
    let calls = unsafe { DR_CALLS };
    let mut k = 0usize;
    for (f, a, payer) in [(f_long, a_long, longs_pay), (f_short, a_short, !longs_pay)] {
        if a == ADL_ONE {
            let exact = (mag * (ADL_ONE / FUNDING_DEN)) as i128;
            assert_eq!(f, if payer { -exact } else { exact }, "fast path is exact");
        } else {
            assert!(k < calls, "a rounded side calls the divider");
            let (p, q, rr) = unsafe { (DR_P[k], DR_Q[k], DR_R[k]) };
            assert_eq!(p, mag * a, "the divider receives exactly mag * a_side");
            assert!(p < P_BOUND, "inside the range of (L)");
            if payer {
                let up = if rr != 0 { q + 1 } else { q };
                assert_eq!(f, -(up as i128), "payer = -(q + [r != 0])");
            } else {
                assert_eq!(f, q as i128, "receiver = q");
            }
            k += 1;
        }
    }
    assert_eq!(k, calls, "no other divider call");
    kani::cover!(a_long < ADL_ONE && a_short < ADL_ONE, "both sides rounded");
    kani::cover!((a_long == ADL_ONE) != (a_short == ADL_ONE), "one side on the fast path");
    kani::cover!(a_long == ADL_ONE && a_short == ADL_ONE, "both on the fast path");
    kani::cover!(calls == 2 && unsafe { DR_R[0] != 0 }, "rounding remainder non-zero");
    kani::cover!(mag == MAG_BOUND - 1 && a_long == ADL_ONE - 1, "top of the bounded range");
}

#[kani::proof]
#[kani::stub(percolator::v16::funding_div_rem, funding_div_rem_recorder)]
#[kani::stub(percolator::wide_math::mul_div_floor_u256_with_rem, u256_fallback_unreachable)]
#[kani::unwind(3)]
#[kani::solver(kissat)]
fn proof_v22_funding_h2_structure_longs_pay() {
    funding_h2_structure_body(true);
}

#[kani::proof]
#[kani::stub(percolator::v16::funding_div_rem, funding_div_rem_recorder)]
#[kani::stub(percolator::wide_math::mul_div_floor_u256_with_rem, u256_fallback_unreachable)]
#[kani::unwind(3)]
#[kani::solver(kissat)]
fn proof_v22_funding_h2_structure_shorts_pay() {
    funding_h2_structure_body(false);
}

/// (R): q*DEN + r == p with r < DEN and p < 2^114 => ceil/floor as H2 states them.
#[kani::proof]
#[kani::solver(kissat)]
fn lemma_v22_funding_ceil_floor_of_div_rem() {
    let p: u128 = kani::any();
    let q: u128 = kani::any();
    let r: u128 = kani::any();
    kani::assume(p < P_BOUND && r < FUNDING_DEN && q < Q_BOUND);
    kani::assume(q * FUNDING_DEN + r == p);
    let pay = if r != 0 { q + 1 } else { q };
    let recv = q;
    assert!(pay * FUNDING_DEN >= p);
    assert!(pay * FUNDING_DEN < p + FUNDING_DEN);
    assert!(recv * FUNDING_DEN <= p);
    assert!(p < recv * FUNDING_DEN + FUNDING_DEN);
    kani::cover!(r != 0, "rounded");
    kani::cover!(r == 0 && p > 0, "exact");
}
