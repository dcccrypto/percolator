//! #299 path 2b: Kani proofs for the loss-provenance record and the liquidation charge decision.
//! Spec: upstream spec.md req 5 (residual charged only to the domain whose exposure generated it)
//! and req 27 (residual attribution deterministic and independent of caller ordering).
//! Run locally only: `cargo kani --tests -Z stubbing --harness proof_e299_`.
#![cfg(kani)]

use percolator::v16::{
    decode_loss_provenance, effective_loss_provenance, encode_loss_provenance,
    kani_liquidation_residual_charge, kani_loss_provenance_after_pnl,
    LiquidationResidualChargeV16, LossProvenanceV16,
};

/// Any record, with the domain drawn from a small symbolic range (domains are opaque indices).
fn any_record() -> LossProvenanceV16 {
    match kani::any::<u8>() % 3 {
        0 => LossProvenanceV16::None,
        1 => LossProvenanceV16::Unattributable,
        _ => LossProvenanceV16::Domain(kani::any::<u8>() as usize),
    }
}

fn any_pnl() -> i128 {
    let p: i64 = kani::any();
    p as i128
}

fn any_loss() -> i128 {
    let a: u32 = kani::any();
    kani::assume(a > 0);
    a as i128
}

/// Req 27: two losses applied in either order leave the same record (the K/F negative phase of
/// one refresh is order-independent), for every prior record and starting pnl.
#[kani::proof]
fn proof_e299_loss_merge_is_order_independent() {
    let old = any_record();
    let p0 = any_pnl();
    let (a1, a2) = (any_loss(), any_loss());
    let (d1, d2) = (kani::any::<u8>() as usize, kani::any::<u8>() as usize);
    let multi: bool = kani::any();
    let p2 = p0 - a1 - a2;
    let x1 = p0 - a1;
    let first = kani_loss_provenance_after_pnl(
        kani_loss_provenance_after_pnl(old, p0, x1, Some(d1), multi),
        x1,
        p2,
        Some(d2),
        multi,
    );
    let y1 = p0 - a2;
    let second = kani_loss_provenance_after_pnl(
        kani_loss_provenance_after_pnl(old, p0, y1, Some(d2), multi),
        y1,
        p2,
        Some(d1),
        multi,
    );
    assert_eq!(first, second);
    kani::cover!(matches!(first, LossProvenanceV16::Domain(_)), "attributed after two losses");
    kani::cover!(first == LossProvenanceV16::Unattributable && d1 != d2, "two domains merge");
}

/// Req 5: a loss write never makes the record name a domain other than the one that realized it,
/// and a loss with no named leg that leaves pnl negative is never attributed.
#[kani::proof]
fn proof_e299_record_never_names_a_non_realizing_domain() {
    let old = any_record();
    let p0 = any_pnl();
    let p1 = p0 - any_loss();
    let d = kani::any::<u8>() as usize;
    let multi: bool = kani::any();
    if let LossProvenanceV16::Domain(x) = kani_loss_provenance_after_pnl(old, p0, p1, Some(d), multi) {
        assert_eq!(x, d);
        kani::cover!(true, "a loss write attributes");
    }
    if p1 < 0 {
        assert!(!matches!(
            kani_loss_provenance_after_pnl(old, p0, p1, None, multi),
            LossProvenanceV16::Domain(_)
        ));
    }
    // an unrecorded deficit is never laundered into an attribution
    if p0 < 0 && old == LossProvenanceV16::None {
        assert_eq!(
            kani_loss_provenance_after_pnl(old, p0, p1, Some(d), multi),
            LossProvenanceV16::Unattributable
        );
    }
}

/// A non-decreasing write: with at most one leg open, reaching pnl >= 0 clears the record; with
/// several legs open the record is kept (sticky for the episode); while pnl stays negative the
/// effective record is kept. (A loss landing exactly on 0, or a non-loss decrease, may leave a
/// record beside pnl == 0; only a gain with <= 1 leg, or a resolving liquidation, clears it.)
#[kani::proof]
fn proof_e299_gain_keeps_or_clears_the_record() {
    let old = any_record();
    let p0 = any_pnl();
    let p1 = any_pnl();
    kani::assume(p1 >= p0);
    let multi: bool = kani::any();
    let d = if kani::any() { Some(kani::any::<u8>() as usize) } else { None };
    let r = kani_loss_provenance_after_pnl(old, p0, p1, d, multi);
    if p1 >= 0 && !multi {
        assert_eq!(r, LossProvenanceV16::None);
        kani::cover!(old != LossProvenanceV16::None, "single-leg gain clears a record");
    } else {
        kani::cover!(p1 >= 0 && multi && matches!(old, LossProvenanceV16::Domain(_)), "multi-leg gain keeps a record");
        assert_eq!(r, effective_loss_provenance(old, p0));
    }
}

/// Req 27 across refreshes: in a multi-leg episode, a loss from d1, a gain, and a loss from d2
/// realized in ANY of the orders a caller can produce (by accruing assets one at a time and
/// refreshing in between) leave the same record. Before the multi-leg-sticky rule the gain could
/// reset the record between the two losses (security review of 9dc1395c).
#[kani::proof]
fn proof_e299_multi_leg_record_ignores_gain_interleaving() {
    let old = any_record();
    let p0 = any_pnl();
    let (a1, a2) = (any_loss(), any_loss());
    let g: u32 = kani::any();
    let g = g as i128;
    let (d1, d2) = (kani::any::<u8>() as usize, kani::any::<u8>() as usize);
    let f = |o, from: i128, to: i128, d| kani_loss_provenance_after_pnl(o, from, to, d, true);
    // L1, G, L2
    let x1 = p0 - a1;
    let x2 = x1 + g;
    let x3 = x2 - a2;
    let r1 = f(f(f(old, p0, x1, Some(d1)), x1, x2, None), x2, x3, Some(d2));
    // L2, G, L1
    let y1 = p0 - a2;
    let y2 = y1 + g;
    let r2 = f(f(f(old, p0, y1, Some(d2)), y1, y2, None), y2, x3, Some(d1));
    // G, L1, L2
    let z1 = p0 + g;
    let z2 = z1 - a1;
    let r3 = f(f(f(old, p0, z1, None), z1, z2, Some(d1)), z2, x3, Some(d2));
    assert_eq!(r1, r2);
    assert_eq!(r1, r3);
    kani::cover!(matches!(r1, LossProvenanceV16::Domain(_)) && x2 >= 0, "gain crossed zero, still attributed");
    kani::cover!(r1 == LossProvenanceV16::Unattributable && x2 >= 0 && old == LossProvenanceV16::None, "gain crossed zero, two losers");
}

/// The charge decision. Locked: a charge names exactly the recorded domain, never the closed leg
/// otherwise; no record / unattributable is never charged and becomes Recovery once flat; a flat
/// close never stays reduce-only. Two different closed legs that both charge name the same domain
/// (the caller's choice of leg cannot change who pays). Unlocked: the closed leg (unchanged path).
#[kani::proof]
fn proof_e299_locked_charge_is_the_record_or_nothing() {
    let locked: bool = kani::any();
    let p = any_record();
    let (c1, c2) = (kani::any::<u8>() as usize, kani::any::<u8>() as usize);
    let (full1, full2, flat1, flat2): (bool, bool, bool, bool) =
        (kani::any(), kani::any(), kani::any(), kani::any());
    let r1 = kani_liquidation_residual_charge(locked, p, c1, full1, flat1);
    let r2 = kani_liquidation_residual_charge(locked, p, c2, full2, flat2);
    if !locked {
        assert_eq!(r1, LiquidationResidualChargeV16::Charge(c1));
        return;
    }
    match (p, r1) {
        (_, LiquidationResidualChargeV16::Charge(c)) => {
            assert_eq!(p, LossProvenanceV16::Domain(c));
            kani::cover!(c != c1 || !full1, "locked charge of a domain other than the closed full leg");
        }
        (LossProvenanceV16::Domain(_), LiquidationResidualChargeV16::WriteOff) => {
            panic!("an attributed deficit never routes to a write-off from the decision");
        }
        _ => {}
    }
    if flat1 {
        assert_ne!(r1, LiquidationResidualChargeV16::ReduceOnly);
    }
    if let (LiquidationResidualChargeV16::Charge(x), LiquidationResidualChargeV16::Charge(y)) =
        (r1, r2)
    {
        assert_eq!(x, y);
    }
}

/// The wire encoding is a bijection on every u32 (no value aliases another record).
#[kani::proof]
fn proof_e299_provenance_wire_round_trip() {
    let raw: u32 = kani::any();
    let p = decode_loss_provenance(raw).unwrap();
    assert_eq!(encode_loss_provenance(p), Ok(raw));
    kani::cover!(matches!(p, LossProvenanceV16::Domain(_)), "attributed raw");
}

/// GH#292: the latch cannot be CREATED by the decision. Whatever the record, a locked close that
/// leaves the account flat is never `ReduceOnly` (the branch that used to return Ok with the
/// deficit untouched): it is `Charge(d)` (recorded domain only) or `WriteOff` (b-prime; never Recovery).
#[kani::proof]
fn proof_gh292_locked_flat_close_never_reduce_only() {
    let rec = any_record();
    let closed_domain = kani::any::<u8>() as usize;
    let full: bool = kani::any();
    let out = kani_liquidation_residual_charge(true, rec, closed_domain, full, true);
    assert!(out != LiquidationResidualChargeV16::ReduceOnly);
    match (rec, out) {
        (LossProvenanceV16::Domain(d), LiquidationResidualChargeV16::Charge(c)) => assert_eq!(c, d),
        (LossProvenanceV16::Domain(_), _) => assert!(false),
        (_, o) => assert_eq!(o, LiquidationResidualChargeV16::WriteOff),
    }
    kani::cover!(matches!(out, LiquidationResidualChargeV16::Charge(_)), "flat close charges recorded domain");
    kani::cover!(out == LiquidationResidualChargeV16::WriteOff, "flat close with no attribution is written off (no Recovery)");
}

/// b-prime: the decision never declares Recovery for ANY input (the enum has no such arm), and a
/// write-off is only ever chosen for a locked close that leaves the account flat with no single
/// recorded domain.
#[kani::proof]
fn proof_gh292_bprime_writeoff_only_for_locked_flat_unattributable() {
    let rec = any_record();
    let locked: bool = kani::any();
    let flat: bool = kani::any();
    let full: bool = kani::any();
    let closed_domain = kani::any::<u8>() as usize;
    let out = kani_liquidation_residual_charge(locked, rec, closed_domain, full, flat);
    if out == LiquidationResidualChargeV16::WriteOff {
        assert!(locked && flat);
        assert!(!matches!(rec, LossProvenanceV16::Domain(_)));
    }
    kani::cover!(out == LiquidationResidualChargeV16::WriteOff, "write-off reachable");
}
