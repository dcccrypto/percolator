#![cfg(kani)]

//! Issue #117 — Non-vacuous Kani proofs for asymmetric-A accrual conservation.
//!
//! # Background
//!
//! PR #115 fixed issue #114: `accrue_asset_to_not_atomic` was crediting
//! BOTH sides with a flat `price_delta * ADL_ONE` delta, ignoring the live
//! per-side `a_long` / `a_short` values. Settlement divides by the leg's
//! frozen `a_basis`, so on a balanced book (a_long == a_short == ADL_ONE)
//! the factors cancel and accrual is zero-sum. On an asymmetric-A book (a
//! partial ADL drove one side's `a` below ADL_ONE) the low-A side
//! over-realizes by `ADL_ONE / a_side`, minting withdrawable value from
//! thin air.
//!
//! The fix (spec §5.3, spec.md:1070-1079):
//! ```text
//! K_long  += A_long  * ΔP          (if OI_long  > 0)
//! K_short -= A_short * ΔP          (if OI_short > 0)
//! F_long_num  -= A_long  * fund_num_total
//! F_short_num += A_short * fund_num_total
//! ```
//!
//! # Why the existing proofs missed this
//!
//! Every existing accrue Kani harness sets `a_long == a_short == ADL_ONE`
//! (the `baseline_group()` default) — the BALANCED-BOOK fixture where the
//! bug is a no-op. None tested `a_long != a_short`.
//!
//! # What these harnesses prove (tractable scope — issue #117)
//!
//! The settlement "realize path" (`wide_signed_mul_div_floor_from_k_pair`
//! + optional `scaled_adl_delta_fast`) uses ~256-iter U256 division that
//! is CBMC-intractable under symbolic inputs. These harnesses therefore
//! prove the TWO tractable sub-properties directly at the integer-arithmetic
//! level, mirroring the strategy used for other pure-algebra proofs in this
//! codebase (e.g. `proofs_v16_fork_invariants.rs` §8):
//!
//! ## Harness 1 — `proof_asymmetric_a_accrual_formula_matches_spec_53`
//! Proves that the post-fix accrual formula for `k_delta_long` and
//! `k_delta_short` matches spec §5.3 exactly:
//! ```text
//! k_delta_long  = price_delta * a_long
//! k_delta_short = price_delta * a_short
//! ```
//! Under asymmetric A (`a_long != a_short`), these are DIFFERENT deltas,
//! so K_long and K_short move by unequal amounts — which is the load-bearing
//! invariant that the pre-fix flat code violated. The cover ensures the
//! interesting `a_long != a_short AND price_delta != 0` case is reachable
//! and that the deltas are genuinely distinct.
//!
//! ## Harness 2 — `proof_asymmetric_a_matched_legs_cancellation_identity`
//! Proves the algebraic cancellation identity: for OI-symmetric matched
//! legs where each leg's `a_basis` equals the live per-side A at the time
//! of the snapshot update, the per-leg settlement deltas sum to exactly zero,
//! regardless of what `a_long` and `a_short` individually are:
//! ```text
//! long_settle  =  qty * price_delta * a_long  / (a_long  * POS_SCALE)
//!              =  qty * price_delta / POS_SCALE
//! short_settle = -qty * price_delta * a_short / (a_short * POS_SCALE)
//!              = -qty * price_delta / POS_SCALE
//! sum = long_settle + short_settle = 0  (when POS_SCALE | qty*price_delta)
//! ```
//! The `a_basis` factors cancel exactly, making the net zero regardless of
//! asymmetry. This is the property the pre-fix code violated: with flat
//! `ADL_ONE` accrual, `long_settle = qty*price_delta*ADL_ONE/(a_long*POS_SCALE)`,
//! which is `!= -short_settle` when `a_long != a_short`.
//!
//! The divisibility precondition (`qty * price_delta % POS_SCALE == 0`) is
//! enforced via `kani::assume`; the cover proves the interesting case
//! (a_long != a_short, price_delta != 0, qty > 0) is reachable under it.
//!
//! Both harnesses use small bounded symbolic inputs (u8/i8) to stay within
//! CBMC limits — the same discipline as `proofs_v16_arithmetic.rs`.

use percolator::wide_math::floor_div_signed_conservative_i128;
use percolator::{ADL_ONE, MIN_A_SIDE, POS_SCALE};
use percolator::v16::kani_adl_scaled_accrual_index_deltas;

// ============================================================================
// Helper: inline the spec §5.3 k_delta formula, mirroring the post-fix code
// in `accrue_asset_to_not_atomic` (v16.rs:L8619-L8634 post-#115):
//
//   let k_delta_long  = price_delta * a_long   (as i128 mul)
//   let k_delta_short = price_delta * a_short  (as i128 mul)
//   asset.k_long  += k_delta_long
//   asset.k_short -= k_delta_short
//
// We replicate this in plain i128 (no private function call needed) so the
// CBMC solver sees straight multiply — no opaque function boundary.
// ============================================================================

// ============================================================================
// Harness 1 — Accrual formula matches spec §5.3 under asymmetric A
// ============================================================================

/// weak-1020 D8 (row was VACUOUS: it called no production function, it multiplied two locals and compared the
/// product with itself). Now: the REAL engine accrual kernel `kernel_adl_scaled_accrual_index_deltas` (the one
/// `accrue_asset_to_not_atomic` calls, v16.rs:17171/17534) at production-scale symbolic inputs: price delta and
/// funding-index delta over the full i64 range, a_long / a_short ANY u128 in [MIN_A_SIDE, ADL_ONE]. Property =
/// spec 5.3: K_long += A_long*dP, K_short -= A_short*dP, F_long -= A_long*dF, F_short += A_short*dF, exactly, and
/// never an overflow error in this domain (|product| < 2^113). Mutant ASYM-K1 (k_short uses a_long) must go RED.
#[kani::proof]
#[kani::solver(z3)]
fn proof_asymmetric_a_accrual_formula_matches_spec_53() {
    let price_delta = kani::any::<i64>() as i128;
    let fid = kani::any::<i64>() as i128;
    let a_long: u128 = kani::any();
    let a_short: u128 = kani::any();
    kani::assume(a_long >= MIN_A_SIDE && a_long <= ADL_ONE);
    kani::assume(a_short >= MIN_A_SIDE && a_short <= ADL_ONE);
    asym_spec_53(price_delta, fid, a_long, a_short);
}

// ============================================================================
// Harness 2 — Matched-leg cancellation identity under asymmetric A
// ============================================================================

/// weak-1020 D8 (mutant ASYM-K1 survived because the K deltas were computed by the harness): the K deltas now
/// come from the REAL kernel; each leg settles as `floor(q * dK / (a_side * POS_SCALE))` with the engine's own
/// `floor_div_signed_conservative_i128`; q is ANY u32 (no longer a multiple of POS_SCALE), price delta i16,
/// A on the k * ADL_ONE/16 grid (k = 2..=16, all >= MIN_A_SIDE). Property: two matched legs net to a value in
/// [-1, 0] (accrual never mints; at most one atom of floor dust), exactly 0 when POS_SCALE | q * dP.
#[kani::proof]
#[kani::unwind(20)]
#[kani::solver(cadical)]
fn proof_asymmetric_a_matched_legs_cancellation_identity() {
    let q = kani::any::<u32>() as i128;
    let price_delta = kani::any::<i16>() as i128;
    let kl: u8 = kani::any();
    let ks: u8 = kani::any();
    kani::assume((2..=16).contains(&kl) && (2..=16).contains(&ks));
    const A_UNIT: u128 = ADL_ONE / 16;
    let a_long = kl as u128 * A_UNIT;
    let a_short = ks as u128 * A_UNIT;
    assert!(a_long >= MIN_A_SIDE && a_short >= MIN_A_SIDE);

    let (k_long, k_short, _, _) =
        kani_adl_scaled_accrual_index_deltas(price_delta, 0, a_long, a_short).unwrap();
    let long_settle = floor_div_signed_conservative_i128(q * k_long, a_long * POS_SCALE);
    let short_settle = floor_div_signed_conservative_i128(q * k_short, a_short * POS_SCALE);
    let net = long_settle + short_settle;
    assert!(net <= 0 && net >= -1, "matched legs never mint; at most one atom of floor dust");
    if (q * price_delta) % (POS_SCALE as i128) == 0 {
        assert_eq!(net, 0, "exact when POS_SCALE divides q * dP");
    }
    kani::cover!(a_long != a_short && price_delta != 0 && q > 0 && long_settle != 0, "asymmetric A, real transfer");
    kani::cover!(net == -1, "one atom of floor dust");
    kani::cover!(net == 0 && long_settle != 0 && a_long != a_short, "exact cancellation under asymmetric A");
}


/// weak-1020 D8 pre-planned twin: i32 deltas, A on the k * ADL_ONE/16 grid (k = 2..=16), cadical.
#[kani::proof]
#[kani::solver(cadical)]
fn proof_asymmetric_a_accrual_formula_matches_spec_53_grid() {
    let price_delta = kani::any::<i32>() as i128;
    let fid = kani::any::<i32>() as i128;
    let kl: u8 = kani::any();
    let ks: u8 = kani::any();
    kani::assume((2..=16).contains(&kl) && (2..=16).contains(&ks));
    asym_spec_53(price_delta, fid, kl as u128 * (ADL_ONE / 16), ks as u128 * (ADL_ONE / 16));
}

fn asym_spec_53(price_delta: i128, fid: i128, a_long: u128, a_short: u128) {
    let r = kani_adl_scaled_accrual_index_deltas(price_delta, fid, a_long, a_short);
    assert!(r.is_ok(), "no overflow refusal for i64 deltas and A <= ADL_ONE");
    let (k_long, k_short, f_long, f_short) = r.unwrap();
    let (al, ash) = (a_long as i128, a_short as i128);
    assert_eq!(k_long, price_delta * al, "spec 5.3: K_long += A_long * dP");
    assert_eq!(k_short, -(price_delta * ash), "spec 5.3: K_short -= A_short * dP");
    assert_eq!(f_long, -(fid * al), "spec 5.3: F_long -= A_long * dF");
    assert_eq!(f_short, fid * ash, "spec 5.3: F_short += A_short * dF");
    kani::cover!(a_long != a_short && price_delta != 0 && k_long != -k_short, "asymmetric A: the two K deltas differ in size");
    kani::cover!(a_long == a_short && price_delta != 0, "balanced book");
    kani::cover!(fid != 0 && a_long < ADL_ONE && a_short == ADL_ONE, "funding on a post-ADL long side");
    kani::cover!(price_delta == 0 && fid == 0, "no movement");
}