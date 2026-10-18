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
