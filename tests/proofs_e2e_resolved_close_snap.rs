//! E2E Kani proof (d) + (a), composed form: resolved close pays exactly what is owed and value
//! is conserved from the deposits to the final payouts.
//!
//! Composition (why): the one-harness version (tests/proofs_e2e_resolved_close.rs) does not
//! finish (40 min in CBMC symex, REPORT.md). Prefix as a proven-reachable snapshot:
//!   RES95  / RES105 = deposit(long,100); deposit(short,1_000); trade long +10 @100 (fee 0);
//!                     accrue 100 -> 95 (resp. 105), one slot   [real ops,
//!                     tests/e2e_snapshot_gen.rs: byte-identical + validate_shape/with_market]
//!   then, in Kani (SYMBOLIC extra, SYMBOLIC choice of snapshot):
//!     `deposit_not_atomic(long, extra)`; `resolve_market_not_atomic(2)`;
//!     `close_resolved_account_not_atomic` on the loser, then on the winner.
//!
//! Owed (independent spec): long = 100 + extra + 10 * (P1 - 100), short = 1_000 - 10 * (P1 - 100).
//! Statement, for every extra in 0..=255 and P1 in {95, 105}:
//!   both closes return Closed { payout } with payout == owed exactly;
//!   total paid == total deposited (1_100 + extra); vault, c_tot, insurance end at 0;
//!   both accounts end with capital 0 and pnl 0; before the closes c_tot + insurance <= vault.
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

use percolator::{MarketGroupV16ViewMut, PortfolioV16ViewMut, ResolvedCloseOutcomeV16};

fn payout_of(r: ResolvedCloseOutcomeV16) -> u128 {
    match r {
        ResolvedCloseOutcomeV16::Closed { payout } => payout,
        _ => panic!("close did not finish in one call"),
    }
}

pub fn check_resolved_from_snapshot(extra: u128, long_wins: bool) {
    let ((mut header, mut markets), mut long, mut short, p1) = if long_wins {
        (e2e_snapshot::res105_market(), e2e_snapshot::res105_long(), e2e_snapshot::res105_short(), 105i128)
    } else {
        (e2e_snapshot::res95_market(), e2e_snapshot::res95_long(), e2e_snapshot::res95_short(), 95i128)
    };
    let mut g = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut lv = PortfolioV16ViewMut::new(&mut long);
    let mut sv = PortfolioV16ViewMut::new(&mut short);
    g.deposit_not_atomic(&mut lv, extra).unwrap();
    g.resolve_market_not_atomic(2).unwrap();
    assert!(g.header.c_tot.get() + g.header.insurance.get() <= g.header.vault.get());

    let move_value = 10 * (p1 - 100);
    let owed_long = 100 + extra as i128 + move_value;
    let owed_short = 1_000 - move_value;

    let (paid_long, paid_short) = if long_wins {
        let s = payout_of(g.close_resolved_account_not_atomic(&mut sv, 0).unwrap());
        let l = payout_of(g.close_resolved_account_not_atomic(&mut lv, 0).unwrap());
        (l, s)
    } else {
        let l = payout_of(g.close_resolved_account_not_atomic(&mut lv, 0).unwrap());
        let s = payout_of(g.close_resolved_account_not_atomic(&mut sv, 0).unwrap());
        (l, s)
    };
    assert!(paid_long as i128 == owed_long);
    assert!(paid_short as i128 == owed_short);
    assert!(paid_long + paid_short == 1_100 + extra);
    assert!(g.header.vault.get() == 0);
    assert!(g.header.c_tot.get() == 0);
    assert!(g.header.insurance.get() == 0);
    assert!(lv.header.capital.get() == 0 && lv.header.pnl.get() == 0);
    assert!(sv.header.capital.get() == 0 && sv.header.pnl.get() == 0);
    cover!(!long_wins && extra > 0, "long loses: paid capital net of the loss");
    cover!(long_wins && extra > 0, "long wins: paid capital plus the gain after the loser");
}

// Did not finish on the 2026-11-21 machine (REPORT.md); compiled only with `--features e2e-long`.
#[cfg(all(kani, feature = "e2e-long"))]
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_e2e_snap_resolved_close_pays_exactly_owed_and_conserves() {
    let extra: u8 = kani::any();
    let long_wins: bool = kani::any();
    check_resolved_from_snapshot(extra as u128, long_wins);
}

#[cfg(not(kani))]
#[test]
fn native_exhaustive_resolved_close_snap_domain() {
    for extra in 0..=255u128 {
        check_resolved_from_snapshot(extra, false);
        check_resolved_from_snapshot(extra, true);
    }
}
