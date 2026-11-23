//! Trade execution / position accounting edge tests for the v2.2 engine: open, increase, partial
//! reduce, full close, flip through zero, matched counterparties, batch fills, realised PnL
//! bookkeeping, size limits. Tests only; no product code is touched.
//!
//! Setup: IM 10%, MM 5%, price 1_000_000, no fees unless a test says so. Both accounts are funded
//! generously so the margin gate is never what is under test (see margin_edge.rs for that).
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]
use percolator::POS_SCALE;
use percolator::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16, ProvenanceHeaderV16Account,
    TradeRequestV16, V16Config, V16Error, V16PodU64,
};

const PRICE: u64 = 1_000_000;
const BIG: u128 = 1_000_000_000_000_000;
const U: i128 = POS_SCALE as i128;

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
    slot: u64,
    n: usize,
}

impl W {
    fn new(n: usize) -> Self { W::with_fee_cap(n, 0) }
    fn with_fee_cap(n: usize, fee_cap: u64) -> Self {
        let mut cfg = V16Config::public_user_fund_with_market_slots(n as u16, n as u32, 0, 6_480_000);
        cfg.max_abs_funding_e9_per_slot = 10_000;
        cfg.max_price_move_bps_per_slot = 200;
        cfg.initial_margin_bps = 1_000;
        cfg.maintenance_margin_bps = 500;
        cfg.max_accrual_dt_slots = 2;
        cfg.max_trading_fee_bps = fee_cap;
        cfg.min_funding_lifetime_slots = 10_000_000;
        let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, n as u32, 0).unwrap();
        let mut markets: Vec<Market<u64>> = (0..n as u64).map(|i| Market::new(i, EngineAssetSlotV16Account::default())).collect();
        for i in 0..n {
            header.activate_empty_asset_slot_not_atomic(i as u32, &mut markets[i].engine, PRICE, 1 + 2 * i as u64).unwrap();
        }
        W { header, markets, slot: 2 * n as u64 - 1, n }
    }
    fn view(&mut self) -> MarketGroupV16ViewMut<'_, u64> {
        MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets)
    }
    fn price(&self, i: usize) -> u64 { self.markets[i].engine.asset.effective_price.get() }
    fn deposit(&mut self, a: &mut PortfolioAccountV16Account, amt: u128) {
        let mut m = self.view();
        m.deposit_not_atomic(&mut PortfolioV16ViewMut::new(a), amt).unwrap();
    }
    fn try_batch(
        &mut self,
        long: &mut PortfolioAccountV16Account,
        short: &mut PortfolioAccountV16Account,
        reqs: &[TradeRequestV16],
    ) -> Result<percolator::BatchTradeOutcomeV16, V16Error> {
        let mut m = self.view();
        m.execute_batch_with_fee_loss_stale_scoped_not_atomic(
            &mut PortfolioV16ViewMut::new(long),
            &mut PortfolioV16ViewMut::new(short),
            reqs,
            true,
        )
    }
    fn req(&self, i: usize, size: i128, fee_bps: u64) -> TradeRequestV16 {
        TradeRequestV16 { asset_index: i, size_q: size, exec_price: self.price(i), fee_bps }
    }
    /// `long` gets +size on asset i (negative: `long` goes short).
    fn try_trade(&mut self, i: usize, long: &mut PortfolioAccountV16Account, short: &mut PortfolioAccountV16Account, size: i128) -> Result<(), V16Error> {
        let r = self.req(i, size, 0);
        self.try_batch(long, short, &[r]).map(|_| ())
    }
    fn trade(&mut self, i: usize, long: &mut PortfolioAccountV16Account, short: &mut PortfolioAccountV16Account, size: i128) {
        self.try_trade(i, long, short, size).expect("trade");
    }
    fn move_price(&mut self, i: usize, bps: i64) {
        let steps = (bps.abs() + 149) / 150;
        let mut left = bps;
        for _ in 0..steps {
            let d = left.clamp(-150, 150);
            left -= d;
            self.slot += 1;
            for j in 0..self.n {
                let old = self.price(j) as i128;
                let new = if j == i { (old + old * d as i128 / 10_000) as u64 } else { old as u64 };
                let slot = self.slot;
                let mut m = self.view();
                m.accrue_asset_to_not_atomic(j, slot, new, 0, true).expect("accrue");
                m.markets[j].engine.asset.raw_oracle_target_price = V16PodU64::new(new);
            }
        }
    }
    fn refresh(&mut self, a: &mut PortfolioAccountV16Account) {
        let mut m = self.view();
        let mut budget = percolator::S10_MAX_MOVES_PER_INSTRUCTION;
        m.full_account_refresh_with_s10_budget_not_atomic(&mut PortfolioV16ViewMut::new(a), &mut budget).expect("refresh");
    }
    fn oi(&self, i: usize) -> (u128, u128) {
        let a = &self.markets[i].engine.asset;
        (a.oi_eff_long_q.get(), a.oi_eff_short_q.get())
    }
    fn pos_counts(&self, i: usize) -> (u64, u64) {
        let a = &self.markets[i].engine.asset;
        (a.stored_pos_count_long.get(), a.stored_pos_count_short.get())
    }
}

/// signed basis position of the account on asset i (0 if no leg)
fn pos(a: &PortfolioAccountV16Account, i: usize) -> i128 {
    for l in a.legs.iter() {
        if l.active != 0 && l.asset_index.get() as usize == i {
            return l.basis_pos_q.get();
        }
    }
    0
}
fn n_legs(a: &PortfolioAccountV16Account) -> usize { a.legs.iter().filter(|l| l.active != 0).count() }
fn bitmap_ones(a: &PortfolioAccountV16Account) -> u32 { a.active_bitmap.iter().map(|w| w.get().count_ones()).sum() }
/// settled account value: capital plus recorded pnl
fn value(a: &PortfolioAccountV16Account) -> i128 { a.capital.get() as i128 + a.pnl.get() }

fn pair() -> (W, PortfolioAccountV16Account, PortfolioAccountV16Account) { pair_n(1) }
/// a batch may carry at most `max_portfolio_assets` fills, so batch tests use a wider group
fn pair_n(n: usize) -> (W, PortfolioAccountV16Account, PortfolioAccountV16Account) {
    let mut w = W::new(n);
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, BIG);
    w.deposit(&mut b, BIG);
    (w, a, b)
}

// ---------- open ----------

#[test]
fn open_creates_opposite_legs_and_equal_open_interest() {
    let (mut w, mut a, mut b) = pair();
    w.trade(0, &mut a, &mut b, 7 * U);
    assert_eq!(pos(&a, 0), 7 * U);
    assert_eq!(pos(&b, 0), -7 * U);
    assert_eq!(w.oi(0), (7 * POS_SCALE, 7 * POS_SCALE));
    assert_eq!(w.pos_counts(0), (1, 1));
    assert_eq!((n_legs(&a), bitmap_ones(&a)), (1, 1));
    assert_eq!((n_legs(&b), bitmap_ones(&b)), (1, 1));
}

#[test]
fn open_negative_size_makes_the_first_account_short() {
    let (mut w, mut a, mut b) = pair();
    w.trade(0, &mut a, &mut b, -4 * U);
    assert_eq!(pos(&a, 0), -4 * U);
    assert_eq!(pos(&b, 0), 4 * U);
    assert_eq!(w.oi(0), (4 * POS_SCALE, 4 * POS_SCALE));
}

#[test]
fn open_at_the_oracle_price_moves_no_value() {
    let (mut w, mut a, mut b) = pair();
    w.trade(0, &mut a, &mut b, 7 * U);
    assert_eq!(a.capital.get(), BIG);
    assert_eq!(b.capital.get(), BIG);
    assert_eq!(a.pnl.get(), 0);
    assert_eq!(b.pnl.get(), 0);
}

#[test]
fn zero_size_trade_is_rejected_and_creates_no_leg() {
    let (mut w, mut a, mut b) = pair();
    assert_eq!(w.try_trade(0, &mut a, &mut b, 0), Err(V16Error::InvalidConfig));
    assert_eq!(n_legs(&a), 0);
}

#[test]
fn zero_exec_price_is_rejected() {
    let (mut w, mut a, mut b) = pair();
    let r = TradeRequestV16 { asset_index: 0, size_q: U, exec_price: 0, fee_bps: 0 };
    assert_eq!(w.try_batch(&mut a, &mut b, &[r]).map(|_| ()), Err(V16Error::InvalidConfig));
}

#[test]
fn trade_on_a_nonexistent_asset_index_is_rejected() {
    let (mut w, mut a, mut b) = pair();
    let r = TradeRequestV16 { asset_index: 5, size_q: U, exec_price: PRICE, fee_bps: 0 };
    assert_eq!(w.try_batch(&mut a, &mut b, &[r]).map(|_| ()), Err(V16Error::InvalidConfig));
}

#[test]
fn empty_batch_is_rejected_with_non_progress() {
    let (mut w, mut a, mut b) = pair();
    assert_eq!(w.try_batch(&mut a, &mut b, &[]).map(|_| ()), Err(V16Error::NonProgress));
    assert_eq!(w.oi(0), (0, 0));
}

// ---------- increase / partial reduce / full close ----------

#[test]
fn increase_accumulates_into_one_leg_and_open_interest() {
    let (mut w, mut a, mut b) = pair();
    w.trade(0, &mut a, &mut b, 3 * U);
    w.trade(0, &mut a, &mut b, 4 * U);
    assert_eq!(pos(&a, 0), 7 * U);
    assert_eq!(pos(&b, 0), -7 * U);
    assert_eq!(n_legs(&a), 1, "an increase must reuse the leg, not add a second one");
    assert_eq!(w.oi(0), (7 * POS_SCALE, 7 * POS_SCALE));
    assert_eq!(w.pos_counts(0), (1, 1), "stored position count must not double count");
}

#[test]
fn partial_reduce_shrinks_both_legs_and_open_interest() {
    let (mut w, mut a, mut b) = pair();
    w.trade(0, &mut a, &mut b, 10 * U);
    w.trade(0, &mut b, &mut a, 4 * U); // a sells 4
    assert_eq!(pos(&a, 0), 6 * U);
    assert_eq!(pos(&b, 0), -6 * U);
    assert_eq!(w.oi(0), (6 * POS_SCALE, 6 * POS_SCALE));
    assert_eq!(w.pos_counts(0), (1, 1));
}

#[test]
fn full_close_clears_legs_bitmap_open_interest_and_counts() {
    let (mut w, mut a, mut b) = pair();
    w.trade(0, &mut a, &mut b, 10 * U);
    w.trade(0, &mut b, &mut a, 10 * U);
    assert_eq!(n_legs(&a), 0);
    assert_eq!(n_legs(&b), 0);
    assert_eq!(bitmap_ones(&a), 0);
    assert_eq!(bitmap_ones(&b), 0);
    assert_eq!(w.oi(0), (0, 0));
    assert_eq!(w.pos_counts(0), (0, 0));
}

#[test]
fn close_at_the_open_price_returns_exact_capital() {
    let (mut w, mut a, mut b) = pair();
    w.trade(0, &mut a, &mut b, 10 * U);
    w.trade(0, &mut b, &mut a, 10 * U);
    assert_eq!(a.capital.get(), BIG);
    assert_eq!(b.capital.get(), BIG);
    assert_eq!(a.pnl.get(), 0);
    assert_eq!(b.pnl.get(), 0);
}

#[test]
fn reopen_after_full_close_works_and_on_either_side() {
    let (mut w, mut a, mut b) = pair();
    w.trade(0, &mut a, &mut b, 5 * U);
    w.trade(0, &mut b, &mut a, 5 * U);
    w.trade(0, &mut b, &mut a, 3 * U); // now b long
    assert_eq!(pos(&b, 0), 3 * U);
    assert_eq!(pos(&a, 0), -3 * U);
    assert_eq!(w.oi(0), (3 * POS_SCALE, 3 * POS_SCALE));
    assert_eq!(w.pos_counts(0), (1, 1));
}

#[test]
fn many_small_fills_equal_one_big_fill() {
    let (mut w1, mut a1, mut b1) = pair();
    w1.trade(0, &mut a1, &mut b1, 12 * U);
    let (mut w2, mut a2, mut b2) = pair();
    for _ in 0..12 { w2.trade(0, &mut a2, &mut b2, U); }
    assert_eq!(pos(&a1, 0), pos(&a2, 0));
    assert_eq!(pos(&b1, 0), pos(&b2, 0));
    assert_eq!(w1.oi(0), w2.oi(0));
}

#[test]
fn one_position_atom_trade_open_and_close() {
    let (mut w, mut a, mut b) = pair();
    w.trade(0, &mut a, &mut b, 1);
    assert_eq!(pos(&a, 0), 1);
    assert_eq!(w.oi(0), (1, 1));
    w.trade(0, &mut b, &mut a, 1);
    assert_eq!(w.oi(0), (0, 0));
    assert_eq!(n_legs(&a), 0);
}

// ---------- flips ----------

#[test]
fn flip_through_zero_leaves_the_remainder_on_the_new_side() {
    let (mut w, mut a, mut b) = pair();
    w.trade(0, &mut a, &mut b, 3 * U);
    w.trade(0, &mut b, &mut a, 5 * U); // a sells 5: long 3 -> short 2
    assert_eq!(pos(&a, 0), -2 * U);
    assert_eq!(pos(&b, 0), 2 * U);
    assert_eq!(w.oi(0), (2 * POS_SCALE, 2 * POS_SCALE));
    assert_eq!(w.pos_counts(0), (1, 1));
    assert_eq!(n_legs(&a), 1);
}

#[test]
fn flip_equals_close_then_open_same_state() {
    let run = |split: bool| {
        let (mut w, mut a, mut b) = pair();
        w.trade(0, &mut a, &mut b, 6 * U);
        w.move_price(0, 300);
        w.refresh(&mut a);
        w.refresh(&mut b);
        if split {
            w.trade(0, &mut b, &mut a, 6 * U);
            w.trade(0, &mut b, &mut a, 2 * U);
        } else {
            w.trade(0, &mut b, &mut a, 8 * U);
        }
        w.refresh(&mut a);
        w.refresh(&mut b);
        (pos(&a, 0), pos(&b, 0), value(&a), value(&b), w.oi(0), w.pos_counts(0))
    };
    assert_eq!(run(true), run(false), "a flip must equal close-then-open");
}

#[test]
fn flip_at_the_oracle_price_changes_no_value_and_leaves_the_remainder() {
    let (mut w, mut a, mut b) = pair();
    w.trade(0, &mut a, &mut b, 6 * U);
    w.move_price(0, 300);
    w.refresh(&mut a);
    w.refresh(&mut b);
    let p = w.price(0) as i128;
    let v0 = value(&a);
    w.trade(0, &mut b, &mut a, 8 * U); // close 6, open short 2
    w.refresh(&mut a);
    // value must be unchanged by the flip itself (trade at the oracle price, no fee)
    assert_eq!(value(&a), v0);
    // and the new short must carry no hidden pnl: price unchanged => flat
    assert_eq!(pos(&a, 0), -2 * U);
    assert_eq!(p, 1_030_225); // two 150bps steps compound: pins the fixture
}

#[test]
fn flip_twice_returns_to_the_original_side() {
    let (mut w, mut a, mut b) = pair();
    w.trade(0, &mut a, &mut b, 3 * U);
    w.trade(0, &mut b, &mut a, 5 * U);
    w.trade(0, &mut a, &mut b, 5 * U);
    assert_eq!(pos(&a, 0), 3 * U);
    assert_eq!(pos(&b, 0), -3 * U);
    assert_eq!(w.oi(0), (3 * POS_SCALE, 3 * POS_SCALE));
}

// ---------- matched counterparties / conservation ----------

#[test]
fn both_sides_of_a_matched_trade_net_to_zero_after_a_price_move() {
    for bps in [-700i64, -150, 150, 400] {
        let (mut w, mut a, mut b) = pair();
        w.trade(0, &mut a, &mut b, 10 * U);
        w.move_price(0, bps);
        w.refresh(&mut a);
        w.refresh(&mut b);
        assert_eq!(value(&a) + value(&b), 2 * BIG as i128, "bps={bps}: zero-sum before close");
        w.trade(0, &mut b, &mut a, 10 * U);
        assert_eq!(value(&a) + value(&b), 2 * BIG as i128, "bps={bps}: zero-sum after close");
        assert!((value(&a) > BIG as i128) == (bps > 0), "bps={bps}: the long wins on a rise");
    }
}

#[test]
fn realised_pnl_matches_size_times_price_move() {
    let (mut w, mut a, mut b) = pair();
    w.trade(0, &mut a, &mut b, 10 * U);
    w.move_price(0, 300);
    w.refresh(&mut a);
    w.refresh(&mut b);
    w.trade(0, &mut b, &mut a, 10 * U);
    let moved = w.price(0) as i128 - PRICE as i128;
    assert_eq!(value(&a) - BIG as i128, 10 * moved);
    assert_eq!(value(&b) - BIG as i128, -10 * moved);
}

#[test]
fn realised_pnl_is_path_independent_for_the_same_round_trip() {
    // open 10, price up 3%, close: in one fill vs three fills vs staged reduce
    let run = |chunks: &[i128]| {
        let (mut w, mut a, mut b) = pair();
        w.trade(0, &mut a, &mut b, 10 * U);
        w.move_price(0, 300);
        w.refresh(&mut a);
        w.refresh(&mut b);
        for c in chunks { w.trade(0, &mut b, &mut a, *c * U); }
        w.refresh(&mut a);
        w.refresh(&mut b);
        (value(&a), value(&b), n_legs(&a), w.oi(0))
    };
    let one = run(&[10]);
    assert_eq!(one, run(&[5, 5]));
    assert_eq!(one, run(&[1, 2, 7]));
    assert_eq!(one, run(&[9, 1]));
}

#[test]
fn open_in_pieces_before_the_move_equals_open_at_once() {
    let run = |pieces: &[i128]| {
        let (mut w, mut a, mut b) = pair();
        for p in pieces { w.trade(0, &mut a, &mut b, *p * U); }
        w.move_price(0, -300);
        w.refresh(&mut a);
        w.refresh(&mut b);
        w.trade(0, &mut b, &mut a, 10 * U);
        (value(&a), value(&b))
    };
    assert_eq!(run(&[10]), run(&[4, 6]));
    assert_eq!(run(&[10]), run(&[1, 1, 1, 7]));
}

#[test]
fn entry_price_bookkeeping_a_second_fill_after_a_move_does_not_rewrite_history() {
    // open 5 at P0, move +3%, open 5 more at P1, move back -3%-ish, close all. Compare with the
    // same two legs held by two *different* account pairs: the total PnL must be the sum.
    let (mut w, mut a, mut b) = pair();
    w.trade(0, &mut a, &mut b, 5 * U);
    w.move_price(0, 300);
    w.refresh(&mut a);
    w.refresh(&mut b);
    let p1 = w.price(0) as i128;
    w.trade(0, &mut a, &mut b, 5 * U);
    w.move_price(0, -300);
    w.refresh(&mut a);
    w.refresh(&mut b);
    let p2 = w.price(0) as i128;
    w.trade(0, &mut b, &mut a, 10 * U);
    // leg 1 entered at PRICE, leg 2 at p1, both marked at p2
    let expected = 5 * (p2 - PRICE as i128) + 5 * (p2 - p1);
    assert_eq!(value(&a) - BIG as i128, expected);
    assert_eq!(value(&a) + value(&b), 2 * BIG as i128);
}

// ---------- fees ----------

#[test]
fn taker_pays_the_fee_and_only_the_fee_leaves_the_pair() {
    let mut w = W::with_fee_cap(1, 100);
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, BIG);
    w.deposit(&mut b, BIG);
    let r = w.req(0, 10 * U, 10); // 10 bps of notional 10_000_000 = 10_000
    let out = w.try_batch(&mut a, &mut b, &[r]).expect("trade with fee");
    assert_eq!(out.notional, 10 * PRICE as u128);
    // a is the taker (taker_is_long_account = true in try_batch)
    assert_eq!((out.fee_a, out.fee_b), (10_000, 0));
    assert_eq!(BIG - a.capital.get(), 10_000);
    assert_eq!(b.capital.get(), BIG);
}

#[test]
fn zero_fee_bps_charges_nothing() {
    let (mut w, mut a, mut b) = pair();
    let out = w.try_batch(&mut a, &mut b, &[w.req(0, 10 * U, 0)]).unwrap();
    assert_eq!((out.fee_a, out.fee_b), (0, 0));
    assert_eq!(out.notional, 10 * PRICE as u128);
}

// ---------- batch ----------

#[test]
fn batch_over_distinct_assets_equals_the_same_fills_one_by_one() {
    let (mut w1, mut a1, mut b1) = pair_n(3);
    let reqs = [w1.req(0, 3 * U, 0), w1.req(1, 4 * U, 0), w1.req(2, -2 * U, 0)];
    let out = w1.try_batch(&mut a1, &mut b1, &reqs).expect("batch");
    assert_eq!(out.fill_count, 3);
    let (mut w2, mut a2, mut b2) = pair_n(3);
    w2.trade(0, &mut a2, &mut b2, 3 * U);
    w2.trade(1, &mut a2, &mut b2, 4 * U);
    w2.trade(2, &mut a2, &mut b2, -2 * U);
    for i in 0..3 {
        assert_eq!(pos(&a1, i), pos(&a2, i));
        assert_eq!(pos(&b1, i), pos(&b2, i));
        assert_eq!(w1.oi(i), w2.oi(i));
    }
    assert_eq!(value(&a1), value(&a2));
    assert_eq!(value(&b1), value(&b2));
}

#[test]
fn batch_that_nets_to_zero_leaves_no_leg() {
    let (mut w, mut a, mut b) = pair_n(2);
    let reqs = [w.req(0, 5 * U, 0), w.req(0, -5 * U, 0)];
    w.try_batch(&mut a, &mut b, &reqs).expect("batch");
    assert_eq!(n_legs(&a), 0);
    assert_eq!(n_legs(&b), 0);
    assert_eq!(w.oi(0), (0, 0));
    assert_eq!(w.pos_counts(0), (0, 0));
}

#[test]
fn batch_same_asset_open_then_partial_reduce_matches_the_net() {
    let (mut w, mut a, mut b) = pair_n(2);
    let reqs = [w.req(0, 5 * U, 0), w.req(0, -2 * U, 0)];
    w.try_batch(&mut a, &mut b, &reqs).expect("batch");
    assert_eq!(pos(&a, 0), 3 * U);
    assert_eq!(pos(&b, 0), -3 * U);
    assert_eq!(w.oi(0), (3 * POS_SCALE, 3 * POS_SCALE));
    assert_eq!(w.pos_counts(0), (1, 1));
}

/// OBSERVATION (pinned, not asserted as a bug): inside one batch, a second fill on the same
/// asset that would add risk (an increase or a flip) is refused with LossStale, while the same
/// fills sent as separate trades go through (see `increase_accumulates_into_one_leg...`,
/// `flip_through_zero...`). Only the non-recertifying multi-fill path behaves this way.
#[test]
fn batch_same_asset_second_fill_that_adds_risk_is_refused_with_loss_stale() {
    for sizes in [[3i128, 4], [3, -5], [-3, 5]] {
        let (mut w, mut a, mut b) = pair_n(2);
        let reqs = [w.req(0, sizes[0] * U, 0), w.req(0, sizes[1] * U, 0)];
        let r = w.try_batch(&mut a, &mut b, &reqs).map(|_| ());
        assert_eq!(r, Err(V16Error::LossStale), "{sizes:?}");
    }
}

#[test]
fn batch_longer_than_max_portfolio_assets_is_rejected() {
    let (mut w, mut a, mut b) = pair_n(2);
    let reqs = [w.req(0, U, 0), w.req(1, U, 0), w.req(0, -U, 0)];
    assert_eq!(w.try_batch(&mut a, &mut b, &reqs).map(|_| ()), Err(V16Error::InvalidConfig));
}

#[test]
fn batch_across_two_assets_books_each_asset_separately() {
    let mut w = W::new(2);
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, BIG);
    w.deposit(&mut b, BIG);
    let reqs = [w.req(0, 4 * U, 0), w.req(1, -6 * U, 0)];
    w.try_batch(&mut a, &mut b, &reqs).expect("batch");
    assert_eq!((pos(&a, 0), pos(&a, 1)), (4 * U, -6 * U));
    assert_eq!((pos(&b, 0), pos(&b, 1)), (-4 * U, 6 * U));
    assert_eq!(w.oi(0), (4 * POS_SCALE, 4 * POS_SCALE));
    assert_eq!(w.oi(1), (6 * POS_SCALE, 6 * POS_SCALE));
    assert_eq!(n_legs(&a), 2);
}

#[test]
fn a_failing_fill_inside_a_batch_is_an_error_and_leaves_no_position() {
    let (mut w, mut a, mut b) = pair_n(2);
    let reqs = [w.req(0, 2 * U, 0), TradeRequestV16 { asset_index: 1, size_q: U, exec_price: 0, fee_bps: 0 }];
    assert_eq!(w.try_batch(&mut a, &mut b, &reqs).map(|_| ()), Err(V16Error::InvalidConfig));
    assert_eq!(n_legs(&a), 0);
    assert_eq!(w.oi(0), (0, 0));
}

#[test]
fn batch_fill_count_and_notional_add_up() {
    let (mut w, mut a, mut b) = pair_n(3);
    let reqs = [w.req(0, 2 * U, 0), w.req(1, 3 * U, 0), w.req(2, -1 * U, 0)];
    let out = w.try_batch(&mut a, &mut b, &reqs).unwrap();
    assert_eq!(out.fill_count, 3);
    assert_eq!(out.notional, 6 * PRICE as u128, "notional is the sum of absolute fills");
}

// ---------- size limits ----------

#[test]
fn trade_larger_than_max_trade_size_is_rejected() {
    let (mut w, mut a, mut b) = pair();
    let too_big = percolator::MAX_TRADE_SIZE_Q as i128 + 1;
    assert_eq!(w.try_trade(0, &mut a, &mut b, too_big), Err(V16Error::InvalidConfig));
    assert_eq!(w.oi(0), (0, 0));
}

#[test]
fn trade_of_exactly_max_trade_size_is_accepted() {
    let (mut w, mut a, mut b) = pair();
    w.trade(0, &mut a, &mut b, percolator::MAX_TRADE_SIZE_Q as i128);
    assert_eq!(pos(&a, 0), percolator::MAX_TRADE_SIZE_Q as i128);
    assert_eq!(w.oi(0).0, w.oi(0).1);
}

#[test]
fn position_cannot_grow_past_the_position_and_side_oi_cap() {
    let mut w = W::new(1);
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, percolator::MAX_VAULT_TVL / 2);
    w.deposit(&mut b, percolator::MAX_VAULT_TVL / 2);
    let half = (percolator::MAX_POSITION_ABS_Q / 2) as i128;
    w.trade(0, &mut a, &mut b, half);
    w.trade(0, &mut a, &mut b, half);
    // at exactly the cap now; one more atom must fail (position cap and per-side OI cap coincide)
    assert!(w.try_trade(0, &mut a, &mut b, 1).is_err(), "above MAX_POSITION_ABS_Q must be refused");
}

