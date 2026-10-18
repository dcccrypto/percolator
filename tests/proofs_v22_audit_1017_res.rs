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

/// K-E6 (#450): the loss-created backing a forfeit may credit to a domain never exceeds what that
/// domain's claimants own (unbacked claim bound plus the provider receivable already advanced),
/// in whole atoms: `capacity * BOUND_SCALE <= positive_claim_bound + provider_receivable`.
#[kani::proof]
#[kani::unwind(70)]
fn kani_audit1017_e6_forfeit_capacity_never_exceeds_claimant_ownership() {
    let cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    {
        let mut v = View::new(&mut header, &mut markets);
        v.activate_empty_market_not_atomic(0, 100, 1).unwrap();
    }
    let mut v = View::new(&mut header, &mut markets);
    let st = SourceCreditStateV16 {
        positive_claim_bound_num: kani::any(),
        exact_positive_claim_num: 0,
        fresh_reserved_backing_num: kani::any(),
        spent_backing_num: 0,
        provider_receivable_num: kani::any(),
        valid_liened_backing_num: kani::any(),
        impaired_liened_backing_num: 0,
        insurance_credit_reserved_num: kani::any(),
        valid_liened_insurance_num: kani::any(),
        impaired_liened_insurance_num: 0,
        credit_rate_num: 0,
        credit_epoch: 0,
    };
    let domain: usize = kani::any();
    kani::assume(domain < 2);
    let set = v.kani_audit1017_set_source_credit(domain, st);
    kani::assume(set.is_ok());
    match v.kani_audit1017_forfeit_capacity(domain) {
        Ok(cap) => {
            let owned = st.positive_claim_bound_num.checked_add(st.provider_receivable_num);
            assert!(owned.is_some());
            assert!(cap <= owned.unwrap() / BOUND_SCALE);
            kani::cover!(cap > 0, "positive forfeit capacity");
            kani::cover!(cap == 0 && st.positive_claim_bound_num > 0, "claims fully backed: zero capacity");
            kani::cover!(st.provider_receivable_num >= BOUND_SCALE && cap > 0, "receivable contributes");
        }
        Err(_) => {
            kani::cover!(true, "refused (shape / overflow)");
        }
    }
}
