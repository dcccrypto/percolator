//! GH#292 formal evidence (Kani), kernel level. Scope is deliberately narrow and honest.
//!
//! Whole-engine harnesses (the stuck state under every public step) were attempted and do not
//! finish: CBMC needs unwinding of the memcmp/copies over the ~10 KB account and header structs
//! (unwinding-assertion failures at unwind 4, 10, 40; 25 min timeouts at unwind 260). The proof
//! therefore covers the two kernels that CREATE and HOLD the bad state; the whole-engine
//! reachability is the native test `gh292_repro_flat_locked_deficit_latches_hlock_in_live_market`
//! (real trades and accruals, green on the buggy base, red on every fix).
//!
//! 1. `proof_gh292_locked_flat_close_is_not_flagged_by_any_escalation_predicate`: a liquidation
//!    close that removes the account's last leg leaves an uncovered loss (pnl < 0 after principal)
//!    WITHOUT being flagged by `uncovered_loss_remains_with_open_risk`, the only predicate the
//!    liquidation path uses to escalate. The locked branch returns before the waterfall, so a
//!    flat close with a deficit has no exit through that function (cover: capital 0, pnl < 0).
//! 2. `proof_gh292_lock_is_held_on_a_flat_deficit_and_released_only_by_repayment`: once locked, a
//!    flat account keeps `liquidation_lock` for every pnl < 0 and releases it only at pnl >= 0
//!    (exactness + stickiness of 9b737fdc, specialised to the flat case), so the account-local
//!    half of the latch (lock + `negative_pnl_account_count`, which counts pnl < 0 accounts) has no
//!    kernel that resets it without the owner depositing or an explicit resolution.
#![cfg(kani)]

use percolator::{
    kani_active_bitmap_set as active_bitmap_set, kani_liquidation_close_would_leave_uncovered_loss_with_open_risk,
    kani_uncovered_loss_strands_a_flat_account, kani_unattributed_loss_lock_after_pnl,
    V16_EMPTY_ACTIVE_BITMAP,
};

#[kani::proof]
#[kani::unwind(8)]
fn proof_gh292_locked_flat_close_is_not_flagged_by_any_escalation_predicate() {
    let pnl: i128 = kani::any();
    let capital: u128 = kani::any();
    let leg_abs_q: u128 = kani::any();
    kani::assume(pnl < 0 && pnl > i128::MIN);
    kani::assume(leg_abs_q > 0);
    let mut bitmap = V16_EMPTY_ACTIVE_BITMAP;
    active_bitmap_set(&mut bitmap, 0).unwrap();
    // Close the only leg fully: the account is flat afterwards.
    let flagged = kani_liquidation_close_would_leave_uncovered_loss_with_open_risk(
        pnl, capital, bitmap, 0, leg_abs_q, leg_abs_q,
    )
    .unwrap();
    assert!(!flagged);
    kani::cover!(capital == 0, "zero capital, pnl < 0, flat after the close: deficit stranded");
}

#[kani::proof]
#[kani::unwind(8)]
fn proof_gh292_lock_is_held_on_a_flat_deficit_and_released_only_by_repayment() {
    let new_pnl: i128 = kani::any();
    let flat = V16_EMPTY_ACTIVE_BITMAP;
    let locked_after = kani_unattributed_loss_lock_after_pnl(true, flat, new_pnl);
    assert_eq!(locked_after, new_pnl < 0);
    kani::cover!(new_pnl < 0 && locked_after, "flat locked deficit keeps the lock");
    kani::cover!(new_pnl >= 0 && !locked_after, "repayment releases it");
}

/// Variant (a): the two predicates partition "uncovered loss after a liquidation close": with open
/// risk left it is `uncovered_loss_remains_with_open_risk` (-> Recovery), flat it is
/// `uncovered_loss_strands_a_flat_account` (-> Recovery). So no close outcome with an uncovered
/// loss is left unflagged (the base leaves the flat case silent).
#[kani::proof]
#[kani::unwind(8)]
fn proof_gh292_a_every_uncovered_close_outcome_is_escalated() {
    let pnl: i128 = kani::any();
    let capital: u128 = kani::any();
    kani::assume(pnl > i128::MIN);
    let leg_abs_q: u128 = kani::any();
    kani::assume(leg_abs_q > 0);
    let two = kani::any::<bool>();
    let mut bitmap = V16_EMPTY_ACTIVE_BITMAP;
    active_bitmap_set(&mut bitmap, 0).unwrap();
    if two {
        active_bitmap_set(&mut bitmap, 1).unwrap();
    }
    let open_risk = kani_liquidation_close_would_leave_uncovered_loss_with_open_risk(
        pnl, capital, bitmap, 0, leg_abs_q, leg_abs_q,
    )
    .unwrap();
    let flat = kani_uncovered_loss_strands_a_flat_account(
        pnl,
        capital,
        if two { bitmap } else { V16_EMPTY_ACTIVE_BITMAP },
    );
    let uncovered = pnl < 0 && (pnl.unsigned_abs() > capital);
    // exactly one predicate fires iff there is an uncovered loss
    assert_eq!(open_risk || flat, uncovered);
    assert!(!(open_risk && flat));
    kani::cover!(flat, "flat uncovered loss is flagged");
    kani::cover!(open_risk, "open-risk uncovered loss is flagged");
}
