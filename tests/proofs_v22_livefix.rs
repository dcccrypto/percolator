//! LIVEFIX (v22live, 2026-10-20) Kani proof for the F-LIVE-1 crank leg selector, extended
//! 2026-11-12 for the merged upstream #176 riskiest-leg rule (aeyakovenko/percolator a5e0de73).
//! Run locally only: `cargo kani --tests --features fuzz -Z stubbing --exact --harness
//! proof_v22_livefix_liquidation_slot_prefers_unblocked_and_is_total` (and
//! `proof_v22_livefix_liquidation_slot_riskiest_unblocked_reducible_leg`).
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
    // No leg has a #176 risk value: the selector must be exactly F-LIVE-1's.
    let r = kani_auto_crank_liquidation_slot(unblocked, liquidatable, [None; N]);

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

/// Upstream #176 merged with F-LIVE-1, for every flag vector and every risk vector
/// (`risk[i] == Some(_)` means "this leg can be reduced"; `risk` need NOT be masked):
/// - Some iff some leg is liquidatable (summary / plan priority unchanged);
/// - the selected slot is always liquidatable;
/// - if some liquidatable, unblocked, reducible leg exists, the selection is one of them, its
///   risk is >= every other such leg's, and it is the FIRST slot with that maximal risk
///   (slot order breaks only exact ties);
/// - otherwise the selection is exactly F-LIVE-1's (first unblocked, else first liquidatable).
#[kani::proof]
#[kani::unwind(18)]
#[kani::solver(cadical)]
fn proof_v22_livefix_liquidation_slot_riskiest_unblocked_reducible_leg() {
    let unblocked: [bool; N] = kani::any();
    let liquidatable: [bool; N] = kani::any();
    let risk: [Option<u128>; N] = kani::any();
    let r = kani_auto_crank_liquidation_slot(unblocked, liquidatable, risk);

    let mut first_liq: Option<usize> = None;
    let mut first_pref: Option<usize> = None;
    let mut any_candidate = false;
    let mut i = 0usize;
    while i < N {
        if first_liq.is_none() && liquidatable[i] {
            first_liq = Some(i);
        }
        if first_pref.is_none() && liquidatable[i] && unblocked[i] {
            first_pref = Some(i);
        }
        if liquidatable[i] && unblocked[i] && risk[i].is_some() {
            any_candidate = true;
        }
        i += 1;
    }
    assert_eq!(r.is_some(), first_liq.is_some());
    if let Some(s) = r {
        assert!(s < N);
        assert!(liquidatable[s]);
    }
    if any_candidate {
        let s = r.unwrap();
        assert!(unblocked[s]);
        let best = risk[s].unwrap();
        let mut j = 0usize;
        while j < N {
            if liquidatable[j] && unblocked[j] {
                if let Some(rj) = risk[j] {
                    assert!(rj <= best);
                    if j < s {
                        assert!(rj < best);
                    }
                }
            }
            j += 1;
        }
    } else {
        match first_pref {
            Some(p) => assert_eq!(r, Some(p)),
            None => assert_eq!(r, first_liq),
        }
    }
    kani::cover!(any_candidate && r != first_pref, "#176: a riskier later leg beats the first unblocked leg");
    kani::cover!(any_candidate && r == first_pref, "#176: the first unblocked leg is also the riskiest");
    kani::cover!(!any_candidate && first_pref.is_some(), "no reducible leg: F-LIVE-1 first unblocked");
    kani::cover!(first_pref.is_none() && first_liq.is_some(), "all blocked: falls back to the first leg");
    kani::cover!(
        (0..N).any(|j| risk[j].is_some() && !(liquidatable[j] && unblocked[j])) && r.is_some(),
        "risk not masked by the flags"
    );
}
