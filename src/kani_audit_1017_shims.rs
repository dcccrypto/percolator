//! v2.2 full audit 2026-10-17 (workstream K): proof-only forwarding shims for the engine Resolved-payout
//! ports (23f609b3). cfg(kani) child module of v16; never in a program build. Each function forwards to ONE
//! private production function with no logic of its own (same pattern as kani_v22_shims.rs).
#![allow(dead_code)]
use super::*;

impl<'a, T> MarketGroupV16ViewMut<'a, T> {
    /// #289: the pure terminal recredit amount for unconsumed insurance spend.
    pub fn kani_audit1017_terminal_unconsumed_recredit(spent: u128, claim_free_residual: u128) -> u128 {
        V16Core::terminal_claim_free_unconsumed_spend_recredit(spent, claim_free_residual)
    }
    /// #450: whole atoms of loss-created backing the existing claimants of `domain` own.
    pub fn kani_audit1017_forfeit_capacity(&self, domain: usize) -> V16Result<u128> {
        self.forfeit_loss_backing_capacity_atoms(domain)
    }
    /// Fixture helper: write a source-credit state through the production setter.
    pub fn kani_audit1017_set_source_credit(&mut self, domain: usize, state: SourceCreditStateV16) -> V16Result<()> {
        self.set_source_credit_for_domain(domain, state)
    }
}

// ---- K-E6: forfeit capacity (#450). The harness lives in this cfg(kani) child module of v16 so it can stub the
// PRIVATE reader `source_credit_for_domain`; the stub returns a symbolic source-credit state and remembers it, so the
// assertion is about the very state the production fn consumed. No fixture activation (the fn reads nothing else).
#[cfg(kani)]
mod e6 {
    use super::*;
    use core::sync::atomic::{AtomicU64, Ordering};
    // the engine forbids `unsafe`: remember the two fields the claim is about in atomics (hi/lo halves)
    static BOUND_HI: AtomicU64 = AtomicU64::new(0);
    static BOUND_LO: AtomicU64 = AtomicU64::new(0);
    static RECV_HI: AtomicU64 = AtomicU64::new(0);
    static RECV_LO: AtomicU64 = AtomicU64::new(0);
    fn put(hi: &AtomicU64, lo: &AtomicU64, v: u128) {
        hi.store((v >> 64) as u64, Ordering::Relaxed);
        lo.store(v as u64, Ordering::Relaxed);
    }
    fn get(hi: &AtomicU64, lo: &AtomicU64) -> u128 {
        ((hi.load(Ordering::Relaxed) as u128) << 64) | lo.load(Ordering::Relaxed) as u128
    }

    fn source_credit_any<'a, T>(_s: &MarketGroupV16ViewMut<'a, T>, _domain: usize) -> V16Result<SourceCreditStateV16>
    where
        T: 'a,
    {
        let st = SourceCreditStateV16 {
            positive_claim_bound_num: kani::any(),
            exact_positive_claim_num: kani::any(),
            fresh_reserved_backing_num: kani::any(),
            spent_backing_num: kani::any(),
            provider_receivable_num: kani::any(),
            valid_liened_backing_num: kani::any(),
            impaired_liened_backing_num: kani::any(),
            insurance_credit_reserved_num: kani::any(),
            valid_liened_insurance_num: kani::any(),
            impaired_liened_insurance_num: kani::any(),
            credit_rate_num: kani::any(),
            credit_epoch: kani::any(),
        };
        put(&BOUND_HI, &BOUND_LO, st.positive_claim_bound_num);
        put(&RECV_HI, &RECV_LO, st.provider_receivable_num);
        Ok(st)
    }

    /// K-E6 (#450): the loss-created backing a forfeit may credit to a domain never exceeds what that domain's claimants
    /// own (unbacked claim bound plus the provider receivable already advanced), in whole atoms:
    /// `capacity * BOUND_SCALE <= positive_claim_bound + provider_receivable`, for ANY source-credit state the reader
    /// returns. Also: zero when nothing is owned.
    #[kani::proof]
    #[kani::stub(super::MarketGroupV16ViewMut::source_credit_for_domain, source_credit_any)]
    fn kani_audit1017_e6_forfeit_capacity_never_exceeds_claimant_ownership() {
        let cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
        let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
        let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
        let v = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let domain: usize = kani::any();
        match v.forfeit_loss_backing_capacity_atoms(domain) {
            Ok(cap) => {
                let (bound, recv) = (get(&BOUND_HI, &BOUND_LO), get(&RECV_HI, &RECV_LO));
                let owned = bound.checked_add(recv);
                assert!(owned.is_some());
                // multiplicative form (a constant multiply is cheap for the solver; comparing two divisions is not)
                assert!(cap.checked_mul(BOUND_SCALE).is_some_and(|x| x <= owned.unwrap()));
                if bound == 0 && recv == 0 {
                    assert!(cap == 0);
                }
                kani::cover!(cap > 0, "positive forfeit capacity");
                kani::cover!(cap == 0 && bound > 0, "claims fully backed: zero capacity");
                kani::cover!(recv >= BOUND_SCALE && cap > 0, "receivable contributes");
            }
            Err(_) => {
                kani::cover!(true, "refused: shape / overflow of the state");
            }
        }
    }
}
