//! Account close, dust residuals and slot reuse edge tests for the v2.2 engine.
//! Covers: register/deregister of the materialized-portfolio counter, the "is empty" gate for
//! dematerialization (one stale field must block it), close with 1-atom dust, close with fees
//! owed, double close, reopen into a freed slot, and conservation of total value.
//! Tests only; no product code is touched. Setup helpers mirror lifecycle_edge.rs.
//!
//! Skipped on purpose (covered elsewhere): asset-slot lifecycle, resolved payout maths,
//! deposit/withdraw basics, liquidations, fees_liens.
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]
use percolator::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    PortfolioAccountV16Account, PortfolioV16View, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, ResolvedCloseOutcomeV16, V16Config, V16Error, V16PodI128, V16PodU128,
    V16PodU64,
};

const PRICE: u64 = 1_000_000;

fn account(seed: u32) -> PortfolioAccountV16Account {
    let mut key = [0u8; 32];
    key[..4].copy_from_slice(&seed.to_le_bytes());
    key[31] = 0x5A;
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new([1; 32], key, [3; 32]));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(header).unwrap();
    a
}

struct W {
    header: MarketGroupV16HeaderAccount,
    markets: Vec<Market<u64>>,
}

impl W {
    fn new() -> Self {
        let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 6_480_000);
        cfg.max_abs_funding_e9_per_slot = 10_000;
        cfg.max_price_move_bps_per_slot = 200;
        cfg.initial_margin_bps = 1_000;
        cfg.maintenance_margin_bps = 500;
        cfg.max_accrual_dt_slots = 2;
        cfg.min_funding_lifetime_slots = 10_000_000;
        let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
        let mut markets = vec![Market::new(0, EngineAssetSlotV16Account::default())];
        header.activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, PRICE, 1).unwrap();
        W { header, markets }
    }
    fn view(&mut self) -> MarketGroupV16ViewMut<'_, u64> {
        MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets)
    }
    fn vault(&self) -> u128 { self.header.vault.get() }
    fn c_tot(&self) -> u128 { self.header.c_tot.get() }
    fn insurance(&self) -> u128 { self.header.insurance.get() }
    fn count(&self) -> u64 { self.header.materialized_portfolio_count.get() }
    fn deposit(&mut self, a: &mut PortfolioAccountV16Account, amt: u128) -> bool {
        self.view().deposit_not_atomic(&mut PortfolioV16ViewMut::new(a), amt).is_ok()
    }
    fn withdraw(&mut self, a: &mut PortfolioAccountV16Account, amt: u128) -> bool {
        self.view().withdraw_not_atomic(&mut PortfolioV16ViewMut::new(a), amt).is_ok()
    }
    fn register(&mut self, a: &PortfolioAccountV16Account) -> Result<(), V16Error> {
        self.view().register_empty_materialized_portfolio_not_atomic(&PortfolioV16View::new(a))
    }
    fn deregister(&mut self, a: &PortfolioAccountV16Account) -> Result<(), V16Error> {
        self.view().deregister_empty_materialized_portfolio_not_atomic(&PortfolioV16View::new(a))
    }
    fn resolve(&mut self) {
        let s = self.header.current_slot.get() + 1;
        self.view().resolve_market_not_atomic(s).unwrap();
    }
    fn close(&mut self, a: &mut PortfolioAccountV16Account, rate: u128) -> ResolvedCloseOutcomeV16 {
        self.view().close_resolved_account_not_atomic(&mut PortfolioV16ViewMut::new(a), rate).unwrap()
    }
    fn fee_sync(&mut self, a: &mut PortfolioAccountV16Account, slot: u64, rate: u128) -> u128 {
        self.view().sync_account_fee_to_slot_not_atomic(&mut PortfolioV16ViewMut::new(a), slot, rate).unwrap()
    }
}

fn paid(o: ResolvedCloseOutcomeV16) -> u128 {
    match o {
        ResolvedCloseOutcomeV16::Closed { payout } => payout,
        other => panic!("expected Closed, got {other:?}"),
    }
}

// ---------- register / deregister counter ----------

#[test]
fn close_register_then_deregister_returns_count_to_zero() {
    let mut w = W::new();
    let a = account(1);
    assert_eq!(w.count(), 0);
    w.register(&a).unwrap();
    assert_eq!(w.count(), 1);
    w.deregister(&a).unwrap();
    assert_eq!(w.count(), 0);
}

#[test]
fn close_deregister_at_count_zero_is_refused_and_changes_nothing() {
    let mut w = W::new();
    let a = account(1);
    let before = w.header;
    assert_eq!(w.deregister(&a), Err(V16Error::CounterUnderflow));
    assert_eq!(w.header, before);
}

#[test]
fn close_repeated_register_counts_each_call() {
    // The engine does not dedupe by account: the caller owns "register once". Pin that so a
    // future change is noticed.
    let mut w = W::new();
    let a = account(1);
    for i in 1..=5u64 {
        w.register(&a).unwrap();
        assert_eq!(w.count(), i);
    }
    for i in (0..5u64).rev() {
        w.deregister(&a).unwrap();
        assert_eq!(w.count(), i);
    }
    assert!(w.deregister(&a).is_err());
}

#[test]
fn close_register_refused_with_one_atom_of_capital() {
    let mut w = W::new();
    let mut a = account(1);
    assert!(w.deposit(&mut a, 1));
    assert_eq!(w.register(&a), Err(V16Error::LockActive));
    assert_eq!(w.count(), 0);
}

#[test]
fn close_deregister_refused_with_dust_capital_until_it_is_withdrawn() {
    let mut w = W::new();
    let mut a = account(1);
    w.register(&a).unwrap();
    assert!(w.deposit(&mut a, 1));
    assert_eq!(w.deregister(&a), Err(V16Error::LockActive));
    assert_eq!(w.count(), 1);
    assert!(w.withdraw(&mut a, 1));
    w.deregister(&a).unwrap();
    assert_eq!((w.count(), w.vault(), w.c_tot()), (0, 0, 0));
}

/// Each single stale field on an otherwise empty account must block dematerialization.
#[test]
fn close_every_single_stale_field_blocks_deregister() {
    type Poke = fn(&mut PortfolioAccountV16Account);
    let pokes: [(&str, Poke); 8] = [
        ("capital", |a| a.capital = V16PodU128::new(1)),
        ("pnl +1", |a| a.pnl = V16PodI128::new(1)),
        ("pnl -1", |a| a.pnl = V16PodI128::new(-1)),
        ("reserved_pnl", |a| a.reserved_pnl = V16PodU128::new(1)),
        ("fee_credits", |a| a.fee_credits = V16PodI128::new(-1)),
        ("cancel_deposit_escrow", |a| a.cancel_deposit_escrow = V16PodU128::new(1)),
        ("stale_state", |a| a.stale_state = 1),
        ("rebalance_lock", |a| a.rebalance_lock = 1),
    ];
    for (name, poke) in pokes {
        let mut w = W::new();
        w.register(&account(9)).unwrap();
        let mut a = account(1);
        poke(&mut a);
        let before = w.header;
        let r = w.deregister(&a);
        assert!(r.is_err(), "{name}: deregister must be refused");
        assert_eq!(w.header, before, "{name}: refusal must not change the header");
    }
    // b_stale and liquidation_lock too
    for f in [|a: &mut PortfolioAccountV16Account| a.b_stale_state = 1, |a: &mut PortfolioAccountV16Account| a.liquidation_lock = 1] {
        let mut w = W::new();
        w.register(&account(9)).unwrap();
        let mut a = account(1);
        f(&mut a);
        assert!(w.deregister(&a).is_err());
    }
}

#[test]
fn close_deregister_of_a_fresh_account_after_a_dirty_one_is_independent() {
    let mut w = W::new();
    let (mut dirty, clean) = (account(1), account(2));
    w.register(&clean).unwrap();
    w.register(&clean).unwrap();
    assert!(w.deposit(&mut dirty, 3));
    assert!(w.deregister(&dirty).is_err());
    w.deregister(&clean).unwrap();
    assert_eq!(w.count(), 1);
}

// ---------- slot reuse ----------

#[test]
fn close_reinit_over_a_used_account_leaves_no_stale_state() {
    let mut a = account(1);
    a.capital = V16PodU128::new(5);
    a.pnl = V16PodI128::new(-7);
    a.reserved_pnl = V16PodU128::new(2);
    a.fee_credits = V16PodI128::new(-3);
    a.cancel_deposit_escrow = V16PodU128::new(4);
    a.last_fee_slot = V16PodU64::new(99);
    a.stale_state = 1;
    a.b_stale_state = 1;
    a.rebalance_lock = 1;
    a.liquidation_lock = 1;
    a.loss_provenance = percolator::V16PodU32::new(12345);
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new([1; 32], [7; 32], [3; 32]));
    a.init_empty_in_place(header).unwrap();
    assert_eq!(a, {
        let mut f = PortfolioAccountV16Account::default();
        f.init_empty_in_place(header).unwrap();
        f
    });
    let mut w = W::new();
    w.register(&a).unwrap();
}

#[test]
fn close_freed_slot_is_reusable_many_times_and_conserves_value() {
    let mut w = W::new();
    let mut total_in = 0u128;
    let mut total_out = 0u128;
    for round in 1..=20u128 {
        let mut a = account(round as u32);
        w.register(&a).unwrap();
        let amt = round; // 1,2,3.. atoms: lots of dust sizes
        assert!(w.deposit(&mut a, amt));
        total_in += amt;
        assert!(w.withdraw(&mut a, amt));
        total_out += amt;
        w.deregister(&a).unwrap();
        assert_eq!((w.count(), w.vault(), w.c_tot()), (0, 0, 0), "round {round}");
    }
    assert_eq!(total_in, total_out);
}

// ---------- resolved close with dust / fees owed ----------

#[test]
fn close_resolved_one_atom_dust_pays_it_out_and_account_can_deregister() {
    let mut w = W::new();
    let (mut a, mut b) = (account(1), account(2));
    w.register(&a).unwrap();
    w.register(&b).unwrap();
    assert!(w.deposit(&mut a, 1));
    assert!(w.deposit(&mut b, 1_000));
    w.resolve();
    assert_eq!(paid(w.close(&mut a, 0)), 1);
    assert_eq!((w.vault(), w.c_tot()), (1_000, 1_000));
    w.deregister(&a).unwrap();
    assert_eq!(w.count(), 1);
}

#[test]
fn close_resolved_zero_capital_pays_zero_and_changes_no_totals() {
    let mut w = W::new();
    let (a0, mut b) = (account(1), account(2));
    let mut a = a0;
    assert!(w.deposit(&mut b, 50));
    w.resolve();
    assert_eq!(paid(w.close(&mut a, 0)), 0);
    assert_eq!((w.vault(), w.c_tot(), w.insurance()), (50, 50, 0));
}

#[test]
fn close_resolved_twice_second_pays_zero_for_every_dust_size() {
    for amt in [1u128, 2, 3, 7, 1_000_003] {
        let mut w = W::new();
        let (mut a, mut b) = (account(1), account(2));
        assert!(w.deposit(&mut a, amt));
        assert!(w.deposit(&mut b, 10));
        w.resolve();
        assert_eq!(paid(w.close(&mut a, 0)), amt);
        let again = w.view().close_resolved_account_not_atomic(&mut PortfolioV16ViewMut::new(&mut a), 0);
        if let Ok(ResolvedCloseOutcomeV16::Closed { payout }) = again {
            assert_eq!(payout, 0, "amt {amt}");
        }
        assert_eq!((w.vault(), w.c_tot()), (10, 10), "amt {amt}");
    }
}

#[test]
fn close_resolved_fee_larger_than_dust_capital_takes_all_capital_and_leaves_no_debt() {
    let mut w = W::new();
    let (mut a, mut b) = (account(1), account(2));
    assert!(w.deposit(&mut a, 5));
    assert!(w.deposit(&mut b, 100));
    w.resolve(); // resolved_slot is 1, last_fee_slot 0 => dt = 1
    let out = w.close(&mut a, 1_000);
    // fee is capped at capital; nothing is owed afterwards
    assert_eq!(paid(out), 0);
    assert_eq!(a.fee_credits.get(), 0);
    assert_eq!(a.capital.get(), 0);
    // value conserved: the 5 atoms moved to insurance, vault unchanged by the fee itself
    assert_eq!((w.insurance(), w.c_tot(), w.vault()), (5, 100, 105));
    w.deregister(&a).unwrap_err(); // count is 0, so only the counter underflows
}

#[test]
fn close_resolved_fee_smaller_than_capital_pays_the_remainder() {
    let mut w = W::new();
    let (mut a, mut b) = (account(1), account(2));
    assert!(w.deposit(&mut a, 10));
    assert!(w.deposit(&mut b, 100));
    w.resolve();
    // billed from last_fee_slot 0 to resolved_slot (see the ignored test at the bottom)
    let fee = 3 * w.header.resolved_slot.get() as u128;
    assert!(fee < 10);
    assert_eq!(paid(w.close(&mut a, 3)), 10 - fee);
    assert_eq!((w.insurance(), w.c_tot(), w.vault()), (fee, 100, 100 + fee));
}

#[test]
fn close_resolved_fee_rate_u128_max_does_not_overflow_or_mint() {
    let mut w = W::new();
    let (mut a, mut b) = (account(1), account(2));
    assert!(w.deposit(&mut a, 9));
    assert!(w.deposit(&mut b, 100));
    w.resolve();
    assert_eq!(paid(w.close(&mut a, u128::MAX)), 0);
    assert_eq!((w.insurance(), w.c_tot(), w.vault()), (9, 100, 109));
}

#[test]
fn close_resolved_all_accounts_close_vault_equals_insurance() {
    let mut w = W::new();
    let mut accts: Vec<_> = (1..=4u32).map(account).collect();
    let amts = [1u128, 2, 3, 1_000];
    for (a, m) in accts.iter_mut().zip(amts) {
        assert!(w.deposit(a, m));
    }
    w.resolve();
    let mut out = 0;
    for a in accts.iter_mut() {
        out += paid(w.close(a, 2));
    }
    // every atom is either paid out or sits in insurance; nothing vanishes
    assert_eq!(out + w.insurance(), amts.iter().sum::<u128>());
    assert_eq!((w.vault(), w.c_tot()), (w.insurance(), 0));
}

// ---------- known gap vs spec ----------

/// spec.md:793: materialize sets `last_fee_slot_i = materialize_slot`. The engine's
/// `init_empty_in_place` (src/v16.rs:28058) sets it to 0 and `deposit_not_atomic`
/// (src/v16.rs:26606) never anchors it, so the first fee sync on a freshly opened (or reused)
/// account at slot N bills N slots of fee for time the account did not exist.
/// Marked ignore: it fails today. May be the wrapper's job to anchor; the engine gives it no
/// way to do so except writing the raw field.
#[test]
#[ignore = "bug: new/reused account is billed fees from slot 0 (src/v16.rs:28058, spec.md:793)"]
fn close_fresh_account_is_not_billed_for_slots_before_it_existed() {
    let mut w = W::new();
    let mut a = account(1);
    assert!(w.deposit(&mut a, 1_000));
    // account first exists "now"; sync fee at slot 100 with 1 atom/slot rate
    let charged = w.fee_sync(&mut a, 100, 1);
    assert_eq!(charged, 0, "account existed for 0 slots, got billed {charged}");
}
