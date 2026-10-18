#![cfg(kani)]
//! v2.2 full audit 2026-10-17, workstream K: harnesses for the engine Resolved-payout / forfeit ports
//! (23f609b3: #289 terminal recredit, #450 forfeit loss backing). Branch `kani/v22-audit-1017-resolved` only.
//! Real production fns through forwarding shims (`src/kani_audit_1017_shims.rs`, cfg(kani)).
#![allow(dead_code, unused_imports, unused_mut)]

use percolator::v16::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    ProvenanceHeaderV16, SourceCreditStateV16, V16Config,
};
use percolator::BOUND_SCALE;

type View<'a> = MarketGroupV16ViewMut<'a, u64>;

/// K-E4 (#289): the terminal recredit of unconsumed insurance spend never exceeds the domain's
/// outstanding spend nor the remaining claim-free residual, equals their minimum, and cannot be taken
/// twice (after applying it, the next amount is zero): the "recredit the same atoms twice" LoF shape.
#[kani::proof]
fn kani_audit1017_e4_terminal_recredit_bounded_and_not_repeatable() {
    let spent: u128 = kani::any();
    let resid: u128 = kani::any();
    let r1 = View::kani_audit1017_terminal_unconsumed_recredit(spent, resid);
    assert!(r1 <= spent && r1 <= resid);
    assert!(r1 == spent || r1 == resid);
    let r2 = View::kani_audit1017_terminal_unconsumed_recredit(spent - r1, resid - r1);
    assert!(r2 == 0);
    kani::cover!(r1 > 0 && r1 == spent && spent < resid, "spend fully restored, residual left");
    kani::cover!(r1 > 0 && r1 == resid && resid < spent, "residual exhausted, spend left outstanding");
    kani::cover!(r1 == 0 && spent > 0, "no residual: nothing recredited");
}

// K-E6 (#450) lives in src/kani_audit_1017_shims.rs (it stubs a private reader; first version here OOM'd in 75 s).
