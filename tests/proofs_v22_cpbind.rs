#![cfg(kani)]
//! cpbind: the matched-liquidation counterparty is the asset's bound one, decided by the engine.
//!
//! `liquidate_account_inner_not_atomic` refuses (`CounterpartyNotBound`, before any write) a
//! counterparty the matched close could use unless `kernel_counterparty_is_bound(bound, account,
//! counterparty)` holds. For every 32-byte binding / account id / counterparty id:
//!   - accepted => a binding exists, the counterparty IS it, and the liquidated account is not;
//!   - with no binding (all zero) nothing is accepted, not even an all-zero counterparty id;
//!   - the bound counterparty is accepted for every other liquidated account (no false refusal).
//!
//! Run locally only (never in CI):
//!   cargo kani --tests --harness proof_v22_cpbind_only_the_bound_counterparty_is_accepted
//! Mutant (must FAIL): drop `bound_id != [0; 32]` from the kernel.

use percolator::v16::kani_counterparty_is_bound;

#[kani::proof]
fn proof_v22_cpbind_only_the_bound_counterparty_is_accepted() {
    let bound: [u8; 32] = kani::any();
    let account: [u8; 32] = kani::any();
    let cp: [u8; 32] = kani::any();
    let ok = kani_counterparty_is_bound(bound, account, cp);
    if ok {
        assert!(bound != [0u8; 32]);
        assert!(cp == bound);
        assert!(account != bound);
    }
    if bound == [0u8; 32] {
        assert!(!ok);
    }
    if bound != [0u8; 32] && cp == bound && account != bound {
        assert!(ok);
    }
    kani::cover!(ok, "the bound counterparty is accepted");
    kani::cover!(!ok && cp == bound && bound != [0u8; 32], "self-match refused");
}
