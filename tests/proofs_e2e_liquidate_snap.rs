//! E2E Kani proof (b), composed form: a solvent account is never liquidated through the FULL
//! `liquidate_account_not_atomic` path; an insolvent one is.
//!
//! Composition (why): driving deposit -> trade -> accrue -> liquidate in one harness does not
//! finish (CBMC symbolic execution of the prefix; a fully CONCRETE run was still in symex after
//! 11 min, the symbolic one after 60+ min; REPORT.md). So the prefix is a proven-reachable
//! snapshot and the harness runs the last two real steps symbolically:
//!   state LIQ95 = deposit(long,100); deposit(short,10_000); trade long +10 @100 (fee 0);
//!                 accrue 100 -> 95 (one slot)            [real ops, tests/e2e_snapshot_gen.rs,
//!                 byte-identical check + validate_shape + validate_with_market]
//!   then, in Kani: `deposit_not_atomic(long, extra)` (SYMBOLIC extra) and
//!                  `liquidate_account_not_atomic(long, asset 0)` (the full path).
//! Config: mm = im = 10%, liquidation fee 100 bps capped 1_000, min_nonzero_mm_req 2.
//!
//! Spec (independent): equity = 100 + extra - 10 * (100 - 95) = 50 + extra; MM = 95.
//! Statement, for every extra in 0..=255 (full u8):
//!   solvent (50 + extra >= 95)  ==> Err(NonProgress); leg still active with basis +10 units;
//!                                   capital == 50 + extra (the mark loss settled, no fee, no
//!                                   close); insurance and vault unchanged;
//!   insolvent                  ==> Ok with closed_q > 0;
//!   always: vault unchanged, c_tot + insurance <= vault.
// `cover!` is `kani::cover!` under Kani and a no-op natively, so the same check function is
// proved by Kani (`#[kani::proof]` below) and run natively over its whole bounded domain
// (`#[test] native_exhaustive_*` below).
#[cfg(kani)]
macro_rules! cover {
    ($c:expr, $m:expr) => {
        kani::cover!($c, $m)
    };
}
#[cfg(not(kani))]
macro_rules! cover {
    ($c:expr, $m:expr) => {
        let _ = $c;
    };
}

#[path = "e2e_snapshot/mod.rs"]
mod e2e_snapshot;

use percolator::{
    LiquidationRequestV16, MarketGroupV16ViewMut, PortfolioV16ViewMut, V16Error, POS_SCALE,
};

pub fn check_liquidate_from_snapshot(extra: u128) {
    let (mut header, mut markets) = e2e_snapshot::liq95_market();
    let mut long = e2e_snapshot::liq95_long();
    let mut g = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut lv = PortfolioV16ViewMut::new(&mut long);

    g.deposit_not_atomic(&mut lv, extra).unwrap();
    let vault_before = g.header.vault.get();
    let insurance_before = g.header.insurance.get();

    let equity = 50 + extra;
    let mm = 95u128;
    let solvent = equity >= mm;

    let result = g.liquidate_account_not_atomic(&mut lv, LiquidationRequestV16 { asset_index: 0 });

    let leg = lv.header.legs[0].try_to_runtime().unwrap();
    if solvent {
        assert!(result == Err(V16Error::NonProgress));
        assert!(leg.active);
        assert!(leg.basis_pos_q == (10 * POS_SCALE) as i128);
        assert!(lv.header.capital.get() == equity);
        assert!(g.header.insurance.get() == insurance_before);
        cover!(equity == mm, "exact maintenance boundary: refused");
        cover!(equity > mm, "strictly solvent: refused");
    } else {
        let out = result.unwrap();
        assert!(out.closed_q > 0);
        cover!(equity + 1 == mm, "one atom below maintenance: liquidated");
        cover!(out.fee_charged > 0, "insolvent close charges a fee");
    }
    assert!(g.header.vault.get() == vault_before);
    assert!(g.header.c_tot.get() + g.header.insurance.get() <= g.header.vault.get());
}

#[cfg(kani)]
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_e2e_snap_liquidate_solvent_refused_insolvent_closed() {
    let extra: u8 = kani::any();
    check_liquidate_from_snapshot(extra as u128);
}

#[cfg(not(kani))]
#[test]
fn native_exhaustive_liquidate_snap_domain() {
    for extra in 0..=255u128 {
        check_liquidate_from_snapshot(extra);
    }
}
