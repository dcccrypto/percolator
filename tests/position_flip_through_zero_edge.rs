//! Position flip through zero: one trade that takes a long to short (or back) with fees, funding and
//! price moves settled at the flip, partial sizes and exactly-zero results. Tests only.
//!
//! trade_edge.rs already covers: plain flip remainder, flip == close+open, flip at oracle price,
//! flip twice. This file only adds what those lack: fees on the full flip notional, funding booked
//! across a flip, price-move pnl split, exactly-zero edges, one-atom remainders, repeated flips and
//! conservation of total value.
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
}

impl W {
    fn new(fee_cap: u64) -> Self {
        let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 6_480_000);
        cfg.max_abs_funding_e9_per_slot = 10_000;
        cfg.max_price_move_bps_per_slot = 200;
        cfg.initial_margin_bps = 1_000;
        cfg.maintenance_margin_bps = 500;
        cfg.max_accrual_dt_slots = 2;
        cfg.max_trading_fee_bps = fee_cap;
        cfg.min_funding_lifetime_slots = 10_000_000;
        let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
        let mut markets = vec![Market::new(0, EngineAssetSlotV16Account::default())];
        header.activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, PRICE, 1).unwrap();
        W { header, markets, slot: 1 }
    }
    fn view(&mut self) -> MarketGroupV16ViewMut<'_, u64> {
        MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets)
    }
    fn price(&self) -> u64 { self.markets[0].engine.asset.effective_price.get() }
    fn deposit(&mut self, a: &mut PortfolioAccountV16Account, amt: u128) {
        let mut m = self.view();
        m.deposit_not_atomic(&mut PortfolioV16ViewMut::new(a), amt).unwrap();
    }
    /// `long` gets +size (negative: `long` goes short). `long` is the taker and pays the fee.
    fn try_trade(
        &mut self,
        long: &mut PortfolioAccountV16Account,
        short: &mut PortfolioAccountV16Account,
        size: i128,
        fee_bps: u64,
    ) -> Result<percolator::BatchTradeOutcomeV16, V16Error> {
        let r = TradeRequestV16 { asset_index: 0, size_q: size, exec_price: self.price(), fee_bps };
        let mut m = self.view();
        m.execute_batch_with_fee_loss_stale_scoped_not_atomic(
            &mut PortfolioV16ViewMut::new(long),
            &mut PortfolioV16ViewMut::new(short),
            &[r],
            true,
        )
    }
    fn trade(&mut self, long: &mut PortfolioAccountV16Account, short: &mut PortfolioAccountV16Account, size: i128) {
        self.try_trade(long, short, size, 0).expect("trade");
    }
    /// one slot of accrual at `bps` price move and funding `rate` (e9 per slot)
    fn step(&mut self, bps: i64, rate: i128) {
        self.slot += 1;
        let old = self.price() as i128;
        let new = (old + old * bps as i128 / 10_000) as u64;
        let slot = self.slot;
        let mut m = self.view();
        m.accrue_asset_to_not_atomic(0, slot, new, rate, true).expect("accrue");
        m.markets[0].engine.asset.raw_oracle_target_price = V16PodU64::new(new);
    }
    fn refresh(&mut self, a: &mut PortfolioAccountV16Account) {
        let mut m = self.view();
        let mut budget = percolator::S10_MAX_MOVES_PER_INSTRUCTION;
        m.full_account_refresh_with_s10_budget_not_atomic(&mut PortfolioV16ViewMut::new(a), &mut budget).expect("refresh");
    }
    fn oi(&self) -> (u128, u128) {
        let a = &self.markets[0].engine.asset;
        (a.oi_eff_long_q.get(), a.oi_eff_short_q.get())
    }
    fn counts(&self) -> (u64, u64) {
        let a = &self.markets[0].engine.asset;
        (a.stored_pos_count_long.get(), a.stored_pos_count_short.get())
    }
}

fn pos(a: &PortfolioAccountV16Account) -> i128 {
    for l in a.legs.iter() {
        if l.active != 0 && l.asset_index.get() == 0 {
            return l.basis_pos_q.get();
        }
    }
    0
}
fn n_legs(a: &PortfolioAccountV16Account) -> usize { a.legs.iter().filter(|l| l.active != 0).count() }
fn value(a: &PortfolioAccountV16Account) -> i128 { a.capital.get() as i128 + a.pnl.get() }

fn pair(fee_cap: u64) -> (W, PortfolioAccountV16Account, PortfolioAccountV16Account) {
    let mut w = W::new(fee_cap);
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, BIG);
    w.deposit(&mut b, BIG);
    (w, a, b)
}

/// refresh both, return the pair's total settled value
fn total(w: &mut W, a: &mut PortfolioAccountV16Account, b: &mut PortfolioAccountV16Account) -> i128 {
    w.refresh(a);
    w.refresh(b);
    value(a) + value(b)
}

// ---------- exactly-zero and one-atom edges ----------

#[test]
fn selling_exactly_the_position_ends_flat_not_flipped() {
    let (mut w, mut a, mut b) = pair(0);
    w.trade(&mut a, &mut b, 7 * U);
    w.trade(&mut b, &mut a, 7 * U);
    assert_eq!((pos(&a), pos(&b)), (0, 0));
    assert_eq!((n_legs(&a), n_legs(&b)), (0, 0));
    assert_eq!(w.oi(), (0, 0));
    assert_eq!(w.counts(), (0, 0));
}

#[test]
fn one_atom_past_flat_leaves_a_one_atom_opposite_leg() {
    let (mut w, mut a, mut b) = pair(0);
    w.trade(&mut a, &mut b, 7 * U);
    // sell 7 units + 1 atom; the remainder is dust but must be a real short leg or be rejected,
    // never silently dropped from one side only.
    match w.try_trade(&mut b, &mut a, 7 * U + 1, 0) {
        Ok(_) => {
            assert_eq!(pos(&a), -1);
            assert_eq!(pos(&b), 1);
            assert_eq!(w.oi().0, w.oi().1);
            assert_eq!(w.counts(), (1, 1));
        }
        Err(_) => {
            // rejected atomically: nothing moved
            assert_eq!(pos(&a), 7 * U);
            assert_eq!(pos(&b), -7 * U);
        }
    }
}

#[test]
fn one_atom_short_of_flat_keeps_a_one_atom_long() {
    let (mut w, mut a, mut b) = pair(0);
    w.trade(&mut a, &mut b, 7 * U);
    w.trade(&mut b, &mut a, 7 * U - 1);
    assert_eq!(pos(&a), 1);
    assert_eq!(pos(&b), -1);
    assert_eq!(w.oi().0, w.oi().1);
}

#[test]
fn flip_from_a_one_atom_position_to_a_large_opposite_one() {
    let (mut w, mut a, mut b) = pair(0);
    w.trade(&mut a, &mut b, 1);
    w.trade(&mut b, &mut a, 9 * U);
    assert_eq!(pos(&a), 1 - 9 * U);
    assert_eq!(pos(&b), 9 * U - 1);
    assert_eq!(w.oi().0, w.oi().1);
    assert_eq!(w.counts(), (1, 1));
}

#[test]
fn rejected_flip_leaves_state_untouched() {
    let (mut w, mut a, mut b) = pair(0);
    w.trade(&mut a, &mut b, 3 * U);
    let snap = (pos(&a), pos(&b), a.capital.get(), b.capital.get(), w.oi(), w.counts());
    // zero-size and zero-price flips are refused
    assert!(w.try_trade(&mut b, &mut a, 0, 0).is_err());
    let r = TradeRequestV16 { asset_index: 0, size_q: 5 * U, exec_price: 0, fee_bps: 0 };
    let mut m = w.view();
    assert!(m
        .execute_batch_with_fee_loss_stale_scoped_not_atomic(
            &mut PortfolioV16ViewMut::new(&mut b),
            &mut PortfolioV16ViewMut::new(&mut a),
            &[r],
            true
        )
        .is_err());
    assert_eq!(snap, (pos(&a), pos(&b), a.capital.get(), b.capital.get(), w.oi(), w.counts()));
}

// ---------- partial sizes ----------

#[test]
fn flip_in_two_partial_trades_equals_one_flip() {
    let run = |parts: &[i128]| {
        let (mut w, mut a, mut b) = pair(0);
        w.trade(&mut a, &mut b, 6 * U);
        w.step(150, 0);
        w.step(150, 0);
        w.refresh(&mut a);
        w.refresh(&mut b);
        for p in parts {
            w.trade(&mut b, &mut a, *p);
        }
        w.refresh(&mut a);
        w.refresh(&mut b);
        (pos(&a), pos(&b), value(&a), value(&b), w.oi(), w.counts())
    };
    let one = run(&[10 * U]);
    assert_eq!(run(&[4 * U, 6 * U]), one, "4 then 6");
    assert_eq!(run(&[6 * U, 3 * U, 1 * U]), one, "exactly flat, then open in two");
    assert_eq!(run(&[5 * U + 7, 5 * U - 7]), one, "odd split across the zero crossing");
}

// ---------- fees at the flip ----------

#[test]
fn fee_at_a_flip_is_charged_on_the_whole_trade_notional() {
    let (mut w, mut a, mut b) = pair(100);
    w.trade(&mut a, &mut b, 3 * U);
    // a (taker) sells 8: closes 3, opens short 5. 10 bps of 8 * PRICE = 8_000.
    let before = (a.capital.get(), b.capital.get());
    let out = w.try_trade(&mut a, &mut b, -8 * U, 10).expect("flip with fee");
    assert_eq!(out.notional, 8 * PRICE as u128);
    assert_eq!((out.fee_a, out.fee_b), (8_000, 0));
    assert_eq!(before.0 - a.capital.get(), 8_000);
    assert_eq!(b.capital.get(), before.1);
    assert_eq!(pos(&a), -5 * U);
}

#[test]
fn fee_at_an_exactly_flat_close_is_charged_on_the_closed_size() {
    let (mut w, mut a, mut b) = pair(100);
    w.trade(&mut a, &mut b, 3 * U);
    let out = w.try_trade(&mut a, &mut b, -3 * U, 10).expect("close with fee");
    assert_eq!(out.fee_a, 3_000);
    assert_eq!(n_legs(&a), 0);
}

#[test]
fn fee_rounding_at_a_one_atom_flip_does_not_go_negative_or_charge_extra() {
    let (mut w, mut a, mut b) = pair(100);
    w.trade(&mut a, &mut b, 1);
    let before = a.capital.get();
    // tiny notional: fee rounds to a small non-negative number, never more than 10 bps
    let out = w.try_trade(&mut a, &mut b, -2, 10).expect("tiny flip");
    assert!(out.fee_a as u128 <= (out.notional * 10).div_ceil(10_000));
    assert_eq!(before - a.capital.get(), out.fee_a as u128);
    assert_eq!(pos(&a), -1);
}

#[test]
fn fees_over_repeated_flips_only_move_value_out_of_the_pair() {
    let (mut w, mut a, mut b) = pair(100);
    let v0 = value(&a) + value(&b);
    let mut fees: i128 = 0;
    w.try_trade(&mut a, &mut b, 4 * U, 10).map(|o| fees += o.fee_a as i128 + o.fee_b as i128).unwrap();
    for _ in 0..6 {
        // alternate long -> short -> long, each a through-zero flip by 8
        let o = w.try_trade(&mut b, &mut a, 8 * U, 10).unwrap();
        fees += o.fee_a as i128 + o.fee_b as i128;
        let o = w.try_trade(&mut a, &mut b, 8 * U, 10).unwrap();
        fees += o.fee_a as i128 + o.fee_b as i128;
    }
    assert!(fees > 0);
    assert_eq!(total(&mut w, &mut a, &mut b), v0 - fees);
}

// ---------- price move settled at the flip ----------

#[test]
fn flip_after_a_gain_banks_the_gain_and_opens_the_remainder_flat() {
    let (mut w, mut a, mut b) = pair(0);
    w.trade(&mut a, &mut b, 6 * U);
    w.step(150, 0);
    w.step(150, 0);
    let v0 = total(&mut w, &mut a, &mut b);
    let p = w.price() as i128;
    w.trade(&mut b, &mut a, 8 * U);
    w.refresh(&mut a);
    w.refresh(&mut b);
    // the pair is closed-out on 6 and re-opened on 2 at the oracle: total unchanged
    assert_eq!(value(&a) + value(&b), v0);
    // the new short opened at the current price: no further move => both sides flat in pnl terms
    let (va, vb) = (value(&a), value(&b));
    w.refresh(&mut a);
    assert_eq!(value(&a), va);
    assert!(va > BIG as i128, "a booked a gain from the long");
    assert!(vb < BIG as i128, "b booked the matching loss");
    assert_eq!(va - BIG as i128, BIG as i128 - vb, "gain equals loss exactly");
    let _ = p;
}

#[test]
fn new_leg_after_a_flip_earns_from_the_flip_price_only() {
    let (mut w, mut a, mut b) = pair(0);
    w.trade(&mut a, &mut b, 6 * U);
    w.step(150, 0);
    w.refresh(&mut a);
    w.refresh(&mut b);
    w.trade(&mut b, &mut a, 8 * U); // a: short 2
    let v_flip = total(&mut w, &mut a, &mut b);
    let va_flip = value(&a);
    w.step(-100, 0); // price falls 1%: the short gains
    w.refresh(&mut a);
    w.refresh(&mut b);
    assert!(value(&a) > va_flip, "short gains when price falls");
    assert_eq!(value(&a) + value(&b), v_flip);
}

#[test]
fn flip_and_price_loss_on_both_sides_conserves_value() {
    for bps in [-150i64, -1, 0, 1, 150] {
        let (mut w, mut a, mut b) = pair(0);
        let v0 = value(&a) + value(&b);
        w.trade(&mut a, &mut b, 5 * U);
        w.step(bps, 0);
        w.trade(&mut b, &mut a, 12 * U); // a: -7, b: +7
        w.step(-bps, 0);
        w.trade(&mut a, &mut b, 7 * U); // a flat again... exactly zero
        assert_eq!((pos(&a), pos(&b)), (0, 0), "bps {bps}");
        assert_eq!(total(&mut w, &mut a, &mut b), v0, "bps {bps}");
        assert_eq!(w.oi(), (0, 0));
    }
}

// ---------- funding settled across a flip ----------

#[test]
fn funding_accrued_before_a_flip_is_charged_to_the_old_side() {
    let (mut w, mut a, mut b) = pair(0);
    w.trade(&mut a, &mut b, 50 * U);
    for _ in 0..4 {
        w.step(0, 100); // longs pay shorts
    }
    let v0 = total(&mut w, &mut a, &mut b);
    // a flips long 50 -> short 50 without an explicit refresh first
    w.trade(&mut b, &mut a, 100 * U);
    w.refresh(&mut a);
    w.refresh(&mut b);
    assert_eq!(value(&a) + value(&b), v0, "funding is a transfer inside the pair");
    assert_eq!(pos(&a), -50 * U);
    // old long paid, old short received
    assert!(value(&a) < BIG as i128);
    assert!(value(&b) > BIG as i128);
    assert_eq!(BIG as i128 - value(&a), value(&b) - BIG as i128);
}

#[test]
fn funding_after_a_flip_runs_in_the_new_direction() {
    let (mut w, mut a, mut b) = pair(0);
    w.trade(&mut a, &mut b, 50 * U);
    w.step(0, 100);
    w.trade(&mut b, &mut a, 100 * U); // a short 50, b long 50
    let (va, vb) = {
        w.refresh(&mut a);
        w.refresh(&mut b);
        (value(&a), value(&b))
    };
    for _ in 0..4 {
        w.step(0, 100);
    }
    w.refresh(&mut a);
    w.refresh(&mut b);
    // positive funding: longs pay shorts, so now b (long) pays a (short)
    assert!(value(&a) > va, "new short receives");
    assert!(value(&b) < vb, "new long pays");
    assert_eq!(value(&a) + value(&b), va + vb);
}

#[test]
fn funding_with_zero_rate_across_a_flip_moves_nothing() {
    let (mut w, mut a, mut b) = pair(0);
    w.trade(&mut a, &mut b, 50 * U);
    for _ in 0..4 {
        w.step(0, 0);
    }
    w.trade(&mut b, &mut a, 100 * U);
    w.refresh(&mut a);
    w.refresh(&mut b);
    assert_eq!((value(&a), value(&b)), (BIG as i128, BIG as i128));
}

// ---------- repeated flips ----------

#[test]
fn many_flips_keep_open_interest_balanced_and_counts_one_each() {
    let (mut w, mut a, mut b) = pair(0);
    w.trade(&mut a, &mut b, 3 * U);
    for i in 0..20 {
        let sz = (2 * 3 + (i % 3)) as i128 * U; // flips by 6..8
        if i % 2 == 0 {
            w.trade(&mut b, &mut a, sz);
        } else {
            w.trade(&mut a, &mut b, sz);
        }
        assert_eq!(pos(&a), -pos(&b));
        assert_ne!(pos(&a), 0, "step {i} is never flat");
        let (ol, os) = w.oi();
        assert_eq!(ol, os);
        assert_eq!(ol, pos(&a).unsigned_abs());
        assert_eq!(w.counts(), (1, 1));
    }
}

#[test]
fn flip_back_and_forth_through_price_moves_conserves_total_value() {
    let (mut w, mut a, mut b) = pair(0);
    let v0 = value(&a) + value(&b);
    w.trade(&mut a, &mut b, 4 * U);
    for (i, bps) in [120i64, -150, 90, -30, 150, -150, 0, 60].iter().enumerate() {
        w.step(*bps, 0);
        let sz = 9 * U;
        if i % 2 == 0 {
            w.trade(&mut b, &mut a, sz);
        } else {
            w.trade(&mut a, &mut b, sz);
        }
        // the pair's recorded value never creates or destroys anything
        assert_eq!(total(&mut w, &mut a, &mut b), v0, "after flip {i}");
    }
}
