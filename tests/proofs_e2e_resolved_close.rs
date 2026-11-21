//! E2E Kani proof (d) + (a): resolved close pays out exactly what is owed, and value is conserved
//! end to end across deposit -> trade -> mark move (K/F settle in refresh) -> resolve -> close.
//!
//! World (REAL public operations only, except the wrapper-owned raw oracle target):
//! one asset at P0 = 100 (mm = im = 10%, max move 5%/slot, no fees).
//!   1. `deposit_not_atomic(long, D)` with SYMBOLIC `D`; `deposit_not_atomic(short, 1_000)`;
//!   2. `execute_trade_with_fee_loss_stale_scoped_not_atomic`: long +10 units at 100 vs short;
//!   3. `accrue_asset_to_not_atomic(0, 2, P1, 0, true)` with SYMBOLIC `P1`; raw target := P1;
//!   4. `resolve_market_not_atomic(2)`;
//!   5. `close_resolved_account_not_atomic` on the LOSER first, then on the WINNER (the engine
//!      makes a winner wait with `ProgressOnly` until losers are closed; at P1 == P0 the order is
//!      long first).
//!
//! Owed (independent spec, written here): `owed_long = D + 10 * (P1 - 100)`,
//! `owed_short = 1_000 - 10 * (P1 - 100)` (signed; both positive in range: no bankruptcy).
//!
//! Statement. For every D in 100..=163 and P1 in 95..=105:
//!   * each close returns `Closed { payout }` with `payout == owed` exactly (long and short);
//!   * total paid out == total deposited (`D + 1_000`): nothing created, nothing destroyed;
//!   * afterwards vault == 0, c_tot == 0, insurance == 0, and both accounts have capital 0,
//!     pnl 0;
//!   * after step 4 (before any close) `c_tot + insurance <= vault`.
//!
//! Bounds. One asset, two accounts, one leg each; trade size concrete; no fees; no bankruptcy
//! (loss <= 50 < D). Bankrupt resolved close is NOT covered by this harness.
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

use percolator::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, ResolvedCloseOutcomeV16, TradeRequestV16, V16Config, V16PodU64,
    POS_SCALE,
};

const P0: u64 = 100;
const Q_UNITS: i128 = 10;
const SHORT_DEPOSIT: u128 = 1_000;

fn account(seed: u8) -> PortfolioAccountV16Account {
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        [1; 32], [seed; 32], [3; 32],
    ));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(header).unwrap();
    a
}

fn config() -> V16Config {
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    cfg.maintenance_margin_bps = 1_000;
    cfg.initial_margin_bps = 1_000;
    cfg.max_price_move_bps_per_slot = 500;
    cfg.min_nonzero_mm_req = 2;
    cfg.min_nonzero_im_req = 3;
    cfg
}

fn payout_of(r: ResolvedCloseOutcomeV16) -> u128 {
    match r {
        ResolvedCloseOutcomeV16::Closed { payout } => payout,
        _ => panic!("close did not finish in one call"),
    }
}

fn check_resolved_close_pays_exactly_owed(d: u128, p1: u64) {
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], config(), 1, 0).unwrap();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, P0, 1)
        .unwrap();
    let mut long = account(10);
    let mut short = account(11);
    let mut g = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut lv = PortfolioV16ViewMut::new(&mut long);
    let mut sv = PortfolioV16ViewMut::new(&mut short);

    g.deposit_not_atomic(&mut lv, d).unwrap();
    g.deposit_not_atomic(&mut sv, SHORT_DEPOSIT).unwrap();
    let req = TradeRequestV16 {
        asset_index: 0,
        size_q: Q_UNITS * POS_SCALE as i128,
        exec_price: P0,
        fee_bps: 0,
    };
    g.execute_trade_with_fee_loss_stale_scoped_not_atomic(&mut lv, &mut sv, req, true)
        .unwrap();
    g.accrue_asset_to_not_atomic(0, 2, p1, 0, true).unwrap();
    g.markets[0].engine.asset.raw_oracle_target_price = V16PodU64::new(p1);
    g.resolve_market_not_atomic(2).unwrap();
    assert!(g.header.c_tot.get() + g.header.insurance.get() <= g.header.vault.get());

    // Independent spec of what each side is owed.
    let move_value = Q_UNITS * (p1 as i128 - P0 as i128);
    let owed_long = d as i128 + move_value;
    let owed_short = SHORT_DEPOSIT as i128 - move_value;

    let (paid_long, paid_short) = if p1 >= P0 {
        // short is the loser (or flat move): close it first
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
    assert!(paid_long + paid_short == d + SHORT_DEPOSIT);
    assert!(g.header.vault.get() == 0);
    assert!(g.header.c_tot.get() == 0);
    assert!(g.header.insurance.get() == 0);
    assert!(lv.header.capital.get() == 0 && lv.header.pnl.get() == 0);
    assert!(sv.header.capital.get() == 0 && sv.header.pnl.get() == 0);

    cover!(p1 < P0 && d > 100, "long loses, short wins");
    cover!(p1 > P0, "long wins: winner waits for the loser, then is paid its gain");
    cover!(p1 == P0, "no move: each gets its deposit back");
}

#[cfg(kani)]
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_e2e_resolved_close_pays_exactly_owed_and_conserves_value() {
    let d: u8 = kani::any();
    let p1: u8 = kani::any();
    kani::assume((100..=163).contains(&d));
    kani::assume((95..=105).contains(&p1));
    check_resolved_close_pays_exactly_owed(d as u128, p1 as u64);
}

/// Narrower variant (P1 concrete) in case the symbolic-price harness does not finish.
#[cfg(kani)]
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_e2e_resolved_close_pays_exactly_owed_fixed_price() {
    let d: u8 = kani::any();
    kani::assume((100..=163).contains(&d));
    check_resolved_close_pays_exactly_owed(d as u128, 95);
}

/// Fallback (d'), narrower world, larger symbolic range: ONE flat account in a market that is
/// already Resolved. The pre-state is written field by field (as
/// `proof_v16_public_resolved_close_flat_account_pays_only_capital_and_vault` in
/// tests/proofs_v16.rs does) and is CHECKED valid before the call: `validate_shape() == Ok` and
/// `validate_with_market == Ok` are asserted, not assumed.
/// Symbolic (u8 each): the account's capital C, its loss L (pnl = -L), the other accounts'
/// capital O (c_tot = C + O), insurance I, unallocated surplus U (vault = C + O + I + U).
/// Statement: the real `close_resolved_account_not_atomic` returns `Closed { payout }` with
///   payout == C - min(C, L)          (capital net of the loss; an uncovered remainder is written
///                                     off account-locally, not paid by anyone else),
///   vault' == vault - payout, c_tot' == O, insurance' == I, account capital/pnl 0,
///   c_tot' + insurance' <= vault', and the engine's own checks pass after.
pub fn check_resolved_close_flat_with_loss(c: u128, l: u128, o: u128, i: u128, u: u128) {
    let cfg = config();
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, P0, 1)
        .unwrap();
    let mut acct = account(10);
    header.mode = 1; // Resolved
    header.current_slot = V16PodU64::new(2);
    header.resolved_slot = V16PodU64::new(2);
    let vault = c + o + i + u;
    header.vault = percolator::V16PodU128::new(vault);
    header.c_tot = percolator::V16PodU128::new(c + o);
    header.insurance = percolator::V16PodU128::new(i);
    if l > 0 {
        header.negative_pnl_account_count = percolator::V16PodU64::new(1);
    }
    acct.capital = percolator::V16PodU128::new(c);
    acct.pnl = percolator::V16PodI128::new(-(l as i128));
    acct.last_fee_slot = V16PodU64::new(2);
    let mut g = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut av = PortfolioV16ViewMut::new(&mut acct);
    assert!(g.validate_shape().is_ok());
    assert!(av.validate_with_market(&g.as_view()).is_ok());

    let payout = payout_of(g.close_resolved_account_not_atomic(&mut av, 0).unwrap());

    let expected = c - core::cmp::min(c, l);
    assert!(payout == expected);
    assert!(g.header.vault.get() == vault - payout);
    assert!(g.header.c_tot.get() == o);
    assert!(g.header.insurance.get() == i);
    assert!(av.header.capital.get() == 0 && av.header.pnl.get() == 0);
    assert!(g.header.c_tot.get() + g.header.insurance.get() <= g.header.vault.get());
    assert!(g.validate_shape().is_ok());
    cover!(l > c && c > 0 && i > 0, "loss exceeds capital: nothing paid, insurance untouched");
    cover!(l > 0 && l < c, "loss netted from capital, remainder paid");
    cover!(l == 0 && c > 0, "no loss: full capital paid");
}

#[cfg(kani)]
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_e2e_resolved_close_flat_account_with_loss_pays_capital_net_of_loss() {
    let c: u8 = kani::any();
    let l: u8 = kani::any();
    let o: u8 = kani::any();
    let i: u8 = kani::any();
    let u: u8 = kani::any();
    check_resolved_close_flat_with_loss(c as u128, l as u128, o as u128, i as u128, u as u128);
}

#[cfg(not(kani))]
#[test]
fn native_exhaustive_resolved_close_domain() {
    for p1 in 95..=105u64 {
        for d in 100..=163u128 {
            check_resolved_close_pays_exactly_owed(d, p1);
        }
    }
}

/// The flat fallback's domain is u8^5 (2^40 points); natively only a corner-heavy sample runs.
#[cfg(not(kani))]
#[test]
fn native_sampled_resolved_close_flat_domain() {
    let pts = [0u128, 1, 2, 7, 100, 127, 128, 254, 255];
    for c in pts { for l in pts { for o in [0u128, 1, 255] { for i in [0u128, 1, 255] { for u in [0u128, 1, 255] {
        check_resolved_close_flat_with_loss(c, l, o, i, u);
    } } } } }
}

/// Narrow form of (d'), sized like the suite's
/// `proof_v16_public_resolved_close_flat_account_pays_only_capital_and_vault` (which finishes):
/// C in 0..=5 and L in 0..=5 symbolic, O = 3, I = 2, U = 1 concrete. Same check function.
#[cfg(kani)]
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_e2e_resolved_close_flat_account_with_loss_narrow() {
    let c: u8 = kani::any();
    let l: u8 = kani::any();
    kani::assume(c <= 5 && l <= 5);
    check_resolved_close_flat_with_loss(c as u128, l as u128, 3, 2, 1);
}
