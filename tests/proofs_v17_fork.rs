#![cfg(kani)]

//! v17 fork-feature Kani proofs — re-expressed onto the single zero-copy/sparse engine path.
//!
//! Frozen toly `proofs_v16.rs` is adopted BYTE-IDENTICAL (the rebuilt zero-copy core harness);
//! every fork-feature proof lives here instead so the adopted core stays clean and the fork
//! surface re-verifies independently. Features are re-grafted onto frozen one unit at a time;
//! each unit's proof(s) land here. Under `#[cfg(kani)]` the frozen crate re-exports all of `v16`
//! (lib.rs `#[cfg(kani)] pub use v16::*`), so these access the engine API directly.
//!
//! Coverage so far:
//!   A-10 — max_price_move_bps_per_slot upper bound (V16Config::validate_public_user_fund_shape).

use percolator::v16::V16Config;
use percolator::MAX_MARGIN_BPS;

// ============================================================================
// A-10 — max_price_move_bps_per_slot upper bound.
// Frozen toly bounds only the lower edge (`== 0`); the fork additionally
// rejects `> MAX_MARGIN_BPS` (a move budget above full margin would weaken the
// per-slot price-move guard). Re-grafted onto frozen as a one-clause shape check.
// ============================================================================

/// RED-before / GREEN-after for the A-10 clause: any out-of-range value above
/// MAX_MARGIN_BPS must be rejected by `validate_public_user_fund_shape`. Without
/// the re-grafted clause this proof FAILS (frozen would accept it).
#[kani::proof]
#[kani::unwind(20)]
#[kani::solver(cadical)]
fn proof_v17_max_price_move_bps_per_slot_upper_bound() {
    let mut config = V16Config::public_user_fund(1, 0, 1);

    let bad: u64 = kani::any();
    kani::assume(bad > MAX_MARGIN_BPS);
    config.max_price_move_bps_per_slot = bad;

    assert!(config.kani_validate_public_user_fund_shape().is_err());
    kani::cover!(true, "out-of-range max_price_move rejected");
}

/// Boundary: `== MAX_MARGIN_BPS` is accepted (the bound is `<=`, not `<`), so
/// the re-grafted clause is not stricter than the fork's v12 intent.
#[kani::proof]
#[kani::unwind(20)]
#[kani::solver(cadical)]
fn proof_v17_max_price_move_bps_per_slot_boundary_accepted() {
    let mut config = V16Config::public_user_fund(1, 0, 1);
    config.max_price_move_bps_per_slot = MAX_MARGIN_BPS;

    assert!(config.kani_validate_public_user_fund_shape().is_ok());
    kani::cover!(true, "boundary max_price_move accepted");
}
