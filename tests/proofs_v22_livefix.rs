//! LIVEFIX (v22live, 2026-10-20) Kani proof for the F-LIVE-1 crank leg selector.
//! Run locally only: `cargo kani --tests --features fuzz -Z stubbing --exact --harness
//! proof_v22_livefix_liquidation_slot_prefers_unblocked_and_is_total`.
#![cfg(kani)]

use percolator::v16::kani_auto_crank_liquidation_slot;
use percolator::V16_MAX_PORTFOLIO_ASSETS_N as N;

/// For every pair of flag vectors (unblocked need NOT be a subset of liquidatable):
/// - Some iff some leg is liquidatable (summary / plan priority unchanged by the fix);
/// - the selected slot is always liquidatable;
/// - if some liquidatable leg is unblocked, the selection is the FIRST such leg;
/// - otherwise it is the FIRST liquidatable leg (the pre-fix slot).
#[kani::proof]
#[kani::unwind(18)]
#[kani::solver(cadical)]
fn proof_v22_livefix_liquidation_slot_prefers_unblocked_and_is_total() {
    let unblocked: [bool; N] = kani::any();
    let liquidatable: [bool; N] = kani::any();
    let r = kani_auto_crank_liquidation_slot(unblocked, liquidatable);

    let mut first_liq: Option<usize> = None;
    let mut first_pref: Option<usize> = None;
    let mut i = 0usize;
    while i < N {
        if first_liq.is_none() && liquidatable[i] {
            first_liq = Some(i);
        }
        if first_pref.is_none() && liquidatable[i] && unblocked[i] {
            first_pref = Some(i);
        }
        i += 1;
    }
    assert_eq!(r.is_some(), first_liq.is_some());
    if let Some(s) = r {
        assert!(s < N);
        assert!(liquidatable[s]);
    }
    match first_pref {
        Some(p) => assert_eq!(r, Some(p)),
        None => assert_eq!(r, first_liq),
    }
    kani::cover!(first_pref.is_some() && first_pref != first_liq, "fix case: skips a blocked first leg");
    kani::cover!(first_pref.is_none() && first_liq.is_some(), "all blocked: falls back to the first leg");
    kani::cover!(first_liq.is_none(), "nothing liquidatable");
    kani::cover!((0..N).any(|j| unblocked[j] && !liquidatable[j]) && r.is_some(), "unblocked not a subset");
}
