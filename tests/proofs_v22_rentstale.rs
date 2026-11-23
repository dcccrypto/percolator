#![cfg(kani)]
//! rentstale (att647 NEW-1) — the bound that lets a rent-only index move skip the K/F cohort.
//!
//! The LossStale gate refuses a risk-increasing trade while some other account may still
//! recognise a loss that existed before the trade (a deficit that could be B-socialised onto the
//! entrant). An un-touched leg's unsettled rent is not such a loss iff charging it can never
//! create or grow a deficit. Every engine rent charge goes through
//! `band_rent::rent_chargeable_atoms` (`charge_account_rent_current_not_atomic`, then
//! `capital -= charged`, `I += charged`; pnl is not touched), so this harness proves, for every
//! u128 due / u128 capital / i128 pnl:
//!   - charged <= due and charged <= capital (no underflow, never over-charges);
//!   - deficit(capital - charged, pnl) == deficit(capital, pnl), where
//!     deficit(c, p) = max(0, -p - c): the charge never creates or grows a deficit;
//!   - with no loss (pnl >= 0) the charge is the full min(due, capital).
//! Repeated charges (one per touch) compose: each step preserves the deficit, so any sequence does.
//!
//! Run locally only (never in CI), once on final code:
//!   cargo kani --tests -j 2 --harness proof_v22_rentstale_rent_charge_never_creates_or_grows_a_deficit
//! Mutant (must FAIL): `rent_chargeable_atoms` returning `due.min(capital)` (drop the loss cap).

use percolator::band_rent::rent_chargeable_atoms;

fn deficit(capital: u128, pnl: i128) -> u128 {
    if pnl < 0 {
        pnl.unsigned_abs().saturating_sub(capital)
    } else {
        0
    }
}

#[kani::proof]
#[kani::solver(kissat)]
fn proof_v22_rentstale_rent_charge_never_creates_or_grows_a_deficit() {
    let due: u128 = kani::any();
    let capital: u128 = kani::any();
    let pnl: i128 = kani::any();
    let charged = rent_chargeable_atoms(due, capital, pnl);
    assert!(charged <= due, "never charges more than is due");
    assert!(charged <= capital, "never charges more than the capital");
    let capital_after = capital - charged;
    assert_eq!(
        deficit(capital_after, pnl),
        deficit(capital, pnl),
        "a rent charge never creates or grows a deficit"
    );
    if pnl >= 0 {
        assert_eq!(charged, due.min(capital), "no loss: the full due up to capital");
    }
    kani::cover!(pnl < 0 && charged > 0 && charged == due, "loser pays its full rent from loss-free capital");
    kani::cover!(pnl < 0 && charged > 0 && charged < due, "loser's rent capped at its loss-free capital");
    kani::cover!(pnl < 0 && due > 0 && charged == 0 && deficit(capital, pnl) > 0, "insolvent: no rent taken");
    kani::cover!(pnl < 0 && due > 0 && charged == 0 && deficit(capital, pnl) == 0, "exactly loss-owned: no rent");
    kani::cover!(pnl >= 0 && charged > 0 && charged < due, "no loss: capped at capital");
}
