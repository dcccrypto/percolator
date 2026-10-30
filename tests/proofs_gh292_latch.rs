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
    kani_flat_locked_deficit_is_writable_off, kani_unattributed_loss_lock_after_pnl,
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

/// Variant: every stuck state (Live, flat, capital 0, pnl < 0, locked, no open close ledger) is
/// recognised for write-off; after the write-off (pnl = 0) the lock kernel releases the lock; and
/// the guard never fires with open risk, without the lock, with capital, or off Live.
#[kani::proof]
#[kani::unwind(8)]
fn proof_gh292_writeoff_covers_every_stuck_state_and_releases_the_lock() {
    let pnl: i128 = kani::any();
    kani::assume(pnl < 0);
    let flat = V16_EMPTY_ACTIVE_BITMAP;
    assert!(kani_flat_locked_deficit_is_writable_off(true, pnl, 0, true, true, false));
    assert!(!kani_unattributed_loss_lock_after_pnl(true, flat, 0));
    // guard safety: each conjunct is necessary
    let live: bool = kani::any();
    let capital: u128 = kani::any();
    let locked: bool = kani::any();
    let is_flat: bool = kani::any();
    let pending: bool = kani::any();
    let any_pnl: i128 = kani::any();
    if kani_flat_locked_deficit_is_writable_off(live, any_pnl, capital, locked, is_flat, pending) {
        assert!(live && any_pnl < 0 && capital == 0 && locked && is_flat && !pending);
    }
    let mut one_leg = V16_EMPTY_ACTIVE_BITMAP;
    active_bitmap_set(&mut one_leg, 0).unwrap();
    kani::cover!(any_pnl < 0 && capital == 0 && locked && is_flat && live && !pending, "fires");
}
