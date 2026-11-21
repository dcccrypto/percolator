//! E2E Kani proof (c), composed form: senior solvency after settlement through the REAL
//! refresh path (`full_account_refresh_not_atomic`), bankrupt and solvent.
//!
//! Composition (why): see tests/proofs_e2e_liquidate_snap.rs. Prefix as a proven-reachable
//! snapshot:
//!   state SETTLE85 = deposit(long,100); deposit(short,1_000); trade long +10 @100 (fee 0);
//!                    accrue 100 -> 95 -> 91 -> 87 -> 85 (one slot each, 5% cap; the long is not
//!                    touched in between)       [real ops, tests/e2e_snapshot_gen.rs, checked]
//!   then, in Kani: `deposit_not_atomic(long, extra)` (SYMBOLIC) and
//!                  `full_account_refresh_not_atomic(long)` (K/F settle + principal settlement).
//!
//! Spec: capital before refresh C = 100 + extra; loss = 10 * (100 - 85) = 150; paid = min(C, 150).
//! Statement, for every extra in 0..=255 (full u8):
//!   refresh Ok; long capital' == C - paid; long pnl' == paid - 150; short capital unchanged;
//!   vault, insurance unchanged; c_tot' == c_tot - paid;
//!   SENIOR SOLVENCY c_tot' + insurance' <= vault' with surplus vault' - c_tot' - insurance' ==
//!   (surplus before) + paid;
//!   C < 150: negative_pnl_account_count == 1 and the bankruptcy h-lock is active;
//!   C >= 150: negative_pnl_account_count == 0.
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

use percolator::{MarketGroupV16ViewMut, PortfolioV16ViewMut};

pub fn check_settle_from_snapshot(extra: u128) {
    let (mut header, mut markets) = e2e_snapshot::settle85_market();
    let mut long = e2e_snapshot::settle85_long();
    let mut g = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut lv = PortfolioV16ViewMut::new(&mut long);
    g.deposit_not_atomic(&mut lv, extra).unwrap();

    let vault0 = g.header.vault.get();
    let c_tot0 = g.header.c_tot.get();
    let ins0 = g.header.insurance.get();
    let surplus0 = vault0 - c_tot0 - ins0;
    let c = lv.header.capital.get();
    assert!(c == 100 + extra);

    let refreshed = g.full_account_refresh_not_atomic(&mut lv);
    assert!(refreshed.is_ok());

    let loss = 150u128;
    let paid = if c < loss { c } else { loss };
    assert!(lv.header.capital.get() == c - paid);
    assert!(lv.header.pnl.get() == paid as i128 - loss as i128);
    assert!(g.header.vault.get() == vault0);
    assert!(g.header.insurance.get() == ins0);
    assert!(g.header.c_tot.get() == c_tot0 - paid);
    let senior = g.header.c_tot.get() + g.header.insurance.get();
    assert!(senior <= g.header.vault.get());
    assert!(g.header.vault.get() - senior == surplus0 + paid);
    if c < loss {
        assert!(g.header.negative_pnl_account_count.get() == 1);
        assert!(g.header.bankruptcy_hlock_active != 0);
    } else {
        assert!(g.header.negative_pnl_account_count.get() == 0);
    }
    cover!(c < loss, "bankrupt: partial settlement, deficit remains");
    cover!(c == loss, "exact: capital covers the loss to the atom");
    cover!(c > loss, "solvent: capital left");
}

#[cfg(kani)]
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_e2e_snap_settle_refresh_preserves_senior_solvency() {
    let extra: u8 = kani::any();
    check_settle_from_snapshot(extra as u128);
}

#[cfg(not(kani))]
#[test]
fn native_exhaustive_settle_snap_domain() {
    for extra in 0..=255u128 {
        check_settle_from_snapshot(extra);
    }
}
