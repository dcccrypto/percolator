//! Headline Kani proof (kernel level): a solvent account is never liquidated.
//!
//! Statement. For EVERY liquidation equity `e: i128` and EVERY maintenance requirement
//! `m: u128` (full 128-bit domains, no bounds):
//!
//!   solvent(e, m) := e >= 0 && (e as u128) >= m
//!
//!   solvent(e, m)  ==> the certified liquidation deficit is 0 AND the liquidation trigger gate
//!                      refuses with `V16Error::NonProgress`;
//!   !solvent(e, m) ==> the certified liquidation deficit is > 0 AND the gate lets it through
//!                      (so the refusal is exact, not a gate that refuses everything).
//!
//! What is called. The two REAL production kernels, through `cfg(kani)` shims:
//!   * `liquidation_trigger_deficit` -- the body of `V16Core::account_liquidation_deficit`, the
//!     only producer of `HealthCertV16::certified_liq_deficit` on the certify paths;
//!   * `liquidation_trigger_gate` -- the check `liquidate_account_not_atomic` applies to the
//!     freshly certified cert before it touches any state.
//!
//! Scope is deliberately narrow (see tests/proofs_gh292_latch.rs for why whole-engine harnesses
//! do not finish under CBMC). NOT proven here: that `e` and `m` are computed correctly from the
//! account (equity / haircut / in-flight support, per-leg maintenance rounding), that the cert
//! the gate reads is the one just certified, or anything about the other consumers of
//! `certified_liq_deficit` (auto-crank selection, recovery paths).
#![cfg(kani)]

use percolator::{
    kani_liquidation_trigger_deficit, kani_liquidation_trigger_gate, HealthCertV16, V16Error,
    V16_EMPTY_ACTIVE_BITMAP,
};

#[kani::proof]
fn proof_headline_solvent_account_is_never_liquidated_and_insolvent_is_never_refused() {
    let equity: i128 = kani::any();
    let maintenance_req: u128 = kani::any();

    // The real trigger-deficit kernel.
    let deficit = kani_liquidation_trigger_deficit(equity, maintenance_req);

    // Every other cert field is unconstrained: the gate must not depend on them.
    let cert = HealthCertV16 {
        certified_equity: kani::any(),
        certified_initial_req: kani::any(),
        certified_maintenance_req: maintenance_req,
        certified_liq_deficit: deficit,
        certified_worst_case_loss: kani::any(),
        cert_oracle_epoch: kani::any(),
        cert_funding_epoch: kani::any(),
        cert_risk_epoch: kani::any(),
        cert_asset_set_epoch: kani::any(),
        active_bitmap_at_cert: V16_EMPTY_ACTIVE_BITMAP,
        valid: kani::any(),
    };

    // The real liquidation gate.
    let gate = kani_liquidation_trigger_gate(cert);

    let solvent = equity >= 0 && (equity as u128) >= maintenance_req;
    if solvent {
        assert!(deficit == 0);
        assert!(gate == Err(V16Error::NonProgress));
    } else {
        assert!(deficit > 0);
        assert!(gate == Ok(()));
    }

    // Non-vacuity: both sides and the exact boundary are reachable.
    kani::cover!(solvent && maintenance_req > 0 && (equity as u128) == maintenance_req);
    kani::cover!(solvent && (equity as u128) > maintenance_req);
    kani::cover!(!solvent && equity >= 0 && (equity as u128) + 1 == maintenance_req);
    kani::cover!(!solvent && equity < 0);
}
