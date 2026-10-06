#![cfg(kani)]
//! Funding index deltas (`kernel_funding_index_deltas`, PR #280) -- Kani harness DESIGN.
//!
//! STATUS: DESIGNED, NOT RUN, and not yet compiled under Kani. Per the Kani rule for this repo
//! (design + review first, run ONCE on final code, locally, never in CI) these harnesses are
//! committed for review; nothing here is evidence yet. Until they are run, the property is covered
//! by `tests/v21_funding_precision.rs` (proptest, both branches) only.
//!
//! Why the earlier attempt did not terminate: one harness asked the solver for a symbolic
//! `mag * a` (128-bit) AND a 128-bit division by FUNDING_DEN AND the U256 fallback, all at once
//! (> 1 h with symbolic A; > 20 min balanced). This design splits it:
//!
//!   H1  A == ADL_ONE on both sides: one multiplication by the CONSTANT 1e6. No division.
//!   L   lemma: `funding_div_rem(p)` satisfies its multiplication contract for p < 2^114.
//!       (the only harness that contains the divider; a single op on a single symbolic input)
//!   H2  A < ADL_ONE, |funding_num| < 2^64: the bounded `checked_mul` branch, with the divider
//!       replaced by the contract proven in L, and the U256 fallback replaced by an
//!       `unreachable` stub (so the proof also shows the fallback is dead under the bound).
//!
//! NOT covered here, by decision: the U256 fallback (`mag * a` overflowing u128, i.e.
//! |funding_num| >= ~2^78). It stays under proptest.
//!
//! Bound justification: funding_num = rate_e9 * dt * price. Production configs validate
//! `max_abs_funding_e9_per_slot * max_accrual_dt_slots` far below 2^34 and prices are u64 bounded
//! by MAX_ORACLE_PRICE; 2^64 is the reviewer-requested harness bound, not a protocol constant.
//! Above it the function is still exercised by proptest.
//!
//! Review checklist before the single run:
//!  - `#[kani::stub]` of the crate-private `percolator::v16::funding_div_rem` resolves (Kani
//!    resolves stub paths without regard to visibility; if not, make it `pub` under cfg(kani));
//!  - every `kani::cover!` below is SATISFIED (a SUCCESSFUL run with an unsatisfied cover is
//!    vacuous and must be reported as such);
//!  - expected cost: H1 seconds; H2 two symbolic 64x50-bit multiplies (minutes); L one 114-bit
//!    constant divider (the long pole -- if it exceeds the budget, split p into width bands
//!    rather than weakening the contract).

use percolator::v16::{kani_funding_div_rem, kani_funding_index_deltas};
use percolator::wide_math::U256;
use percolator::{ADL_ONE, FUNDING_DEN};

const MAG_BOUND: u128 = 1 << 64;
/// mag < 2^64 and a <= ADL_ONE < 2^50, so p = mag * a < 2^114 and q = p / 1e9 < 2^85.
const P_BOUND: u128 = 1 << 114;
const Q_BOUND: u128 = 1 << 85;

/// H1. With A == ADL_ONE on both sides the index moves by exactly `funding_num * 1e6`: equal and
/// opposite, no rounding, and negating the rate mirrors both sides.
#[kani::proof]
#[kani::solver(cadical)]
fn proof_v22_funding_unit_a_is_exact_and_sign_symmetric() {
    let n: i128 = kani::any();
    kani::assume(n.unsigned_abs() < MAG_BOUND);
    let k = (ADL_ONE / FUNDING_DEN) as i128;

    let r = kani_funding_index_deltas(n, ADL_ONE, ADL_ONE);
    assert!(r.is_ok());
    let (f_long, f_short) = r.unwrap();
    assert!(f_long == -(n * k));
    assert!(f_short == n * k);
    assert!(f_long + f_short == 0);

    let m = kani_funding_index_deltas(-n, ADL_ONE, ADL_ONE);
    assert!(m.is_ok());
    let (m_long, m_short) = m.unwrap();
    assert!(m_long == -f_long && m_short == -f_short);

    kani::cover!(n > 0, "longs pay");
    kani::cover!(n < 0, "shorts pay");
    kani::cover!(n == 0, "zero funding");
    kani::cover!(n.unsigned_abs() == MAG_BOUND - 1, "the bound itself is reachable");
}

/// L. The divider's contract, proven on the real function: for p < 2^114,
/// `q * FUNDING_DEN + r == p`, `r < FUNDING_DEN`, `q < 2^85`. Euclidean division is unique, so any
/// (q, r) satisfying this IS the real result: the model below is exact, not an over-approximation.
#[kani::proof]
#[kani::solver(cadical)]
fn lemma_v22_funding_div_rem_contract() {
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

/// Model of `funding_div_rem` used by H2: exactly the contract proven by L.
fn funding_div_rem_model(p: u128) -> (u128, u128) {
    assert!(p < P_BOUND, "H2 stays inside the range L proves");
    let q: u128 = kani::any();
    let r: u128 = kani::any();
    kani::assume(r < FUNDING_DEN);
    kani::assume(q < Q_BOUND); // q * FUNDING_DEN < 2^115: the next line cannot wrap
    kani::assume(q * FUNDING_DEN + r == p);
    (q, r)
}

/// Under the H2 bound `mag.checked_mul(a)` cannot overflow, so the U256 fallback is dead.
fn u256_fallback_unreachable(_a: U256, _b: U256, _d: U256) -> (U256, U256) {
    assert!(false, "U256 fallback reached with |funding_num| < 2^64 and a <= ADL_ONE");
    (U256::from_u128(0), U256::from_u128(0))
}

/// H2. Bounded `checked_mul` branch. For 0 < |funding_num| < 2^64 and any 0 < a <= ADL_ONE on each
/// side (at least one side shrunk, so the rounded path is taken):
///   payer index move    = ceil (mag * a_payer / FUNDING_DEN)   (stated without division)
///   receiver index move = floor(mag * a_recv  / FUNDING_DEN)
///   signs: payer side negative, receiver side positive;
///   equal A on both sides: the payer never pays less than the receiver gets, gap <= 1 unit;
///   it never errors.
#[kani::proof]
#[kani::stub(percolator::v16::funding_div_rem, funding_div_rem_model)]
#[kani::stub(percolator::wide_math::mul_div_floor_u256_with_rem, u256_fallback_unreachable)]
#[kani::solver(cadical)]
fn proof_v22_funding_shrunk_a_rounds_against_the_payer_bounded() {
    let mag: u128 = kani::any();
    let a_long: u128 = kani::any();
    let a_short: u128 = kani::any();
    let longs_pay: bool = kani::any();
    kani::assume(mag > 0 && mag < MAG_BOUND);
    kani::assume(a_long > 0 && a_long <= ADL_ONE);
    kani::assume(a_short > 0 && a_short <= ADL_ONE);
    kani::assume(a_long < ADL_ONE || a_short < ADL_ONE);
    let n = if longs_pay { mag as i128 } else { -(mag as i128) };

    let r = kani_funding_index_deltas(n, a_long, a_short);
    assert!(r.is_ok());
    let (f_long, f_short) = r.unwrap();
    let (pay_signed, recv_signed, a_pay, a_recv) = if longs_pay {
        (f_long, f_short, a_long, a_short)
    } else {
        (f_short, f_long, a_short, a_long)
    };
    assert!(pay_signed <= 0 && recv_signed >= 0);
    let pay = pay_signed.unsigned_abs();
    let recv = recv_signed as u128;

    // ceil: pay * DEN is the least multiple of DEN that is >= mag * a_pay
    assert!(pay * FUNDING_DEN >= mag * a_pay);
    assert!(pay * FUNDING_DEN < mag * a_pay + FUNDING_DEN);
    // floor: recv * DEN is the greatest multiple of DEN that is <= mag * a_recv
    assert!(recv * FUNDING_DEN <= mag * a_recv);
    assert!(mag * a_recv < recv * FUNDING_DEN + FUNDING_DEN);

    if a_pay == a_recv {
        assert!(pay >= recv && pay - recv <= 1);
    }

    kani::cover!(longs_pay && a_long < ADL_ONE, "shrunk payer (long)");
    kani::cover!(!longs_pay && a_short < ADL_ONE, "shrunk payer (short)");
    kani::cover!(a_pay == ADL_ONE && a_recv < ADL_ONE, "fast-path payer, rounded receiver");
    kani::cover!(a_pay == a_recv && pay == recv + 1, "rounding gap of one index unit");
    kani::cover!(a_pay == a_recv && pay == recv, "shrunk A dividing exactly");
    kani::cover!(mag == MAG_BOUND - 1 && a_pay == ADL_ONE - 1, "top of the bounded range");
}
