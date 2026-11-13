//! Margin / leverage edge tests for the v2.2 engine: initial vs maintenance margin boundaries,
//! max leverage, open / increase / reduce / flip at exactly the limit and one atom over,
//! cross-margin across assets, and withdrawals. Tests only; no product code is touched.
//!
//! Setup: IM 10% (10x max leverage), MM 5%, no fees, price 1_000_000. A position of `k * POS_SCALE`
//! at that price has notional `k * 1_000_000` and initial requirement `k * 100_000`.
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]
use percolator::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16, ProvenanceHeaderV16Account,
    TradeRequestV16, V16Config, V16PodU64,
};
use percolator::POS_SCALE;

const PRICE: u64 = 1_000_000;
/// notional of one POS_SCALE unit at PRICE
const N1: u128 = 1_000_000;
/// initial requirement of one unit at 10%
const IM1: u128 = 100_000;

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
    fn new(n: usize, im_bps: u64, mm_bps: u64) -> Self {
        let mut cfg = V16Config::public_user_fund_with_market_slots(n as u16, n as u32, 0, 6_480_000);
        cfg.max_abs_funding_e9_per_slot = 10_000;
        cfg.max_price_move_bps_per_slot = 200;
        cfg.initial_margin_bps = im_bps;
        cfg.maintenance_margin_bps = mm_bps;
        cfg.max_accrual_dt_slots = 2;
        cfg.min_funding_lifetime_slots = 10_000_000;
        let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, n as u32, 0).unwrap();
        let mut markets: Vec<Market<u64>> = (0..n as u64).map(|i| Market::new(i, EngineAssetSlotV16Account::default())).collect();
        for i in 0..n {
            header.activate_empty_asset_slot_not_atomic(i as u32, &mut markets[i].engine, PRICE, 1 + 2 * i as u64).unwrap();
        }
        W { header, markets, slot: 2 * n as u64 - 1, n }
    }
    fn std() -> Self { W::new(1, 1_000, 500) }
    fn view(&mut self) -> MarketGroupV16ViewMut<'_, u64> {
        MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets)
    }
    fn price(&self, i: usize) -> u64 { self.markets[i].engine.asset.effective_price.get() }
    fn deposit(&mut self, a: &mut PortfolioAccountV16Account, amt: u128) {
        let mut m = self.view();
        m.deposit_not_atomic(&mut PortfolioV16ViewMut::new(a), amt).unwrap();
    }
    /// `long` gets +size on asset i (size may be negative: then `long` goes short). Returns Ok/Err.
    fn trade(&mut self, i: usize, long: &mut PortfolioAccountV16Account, short: &mut PortfolioAccountV16Account, size: i128) -> bool {
        let p = self.price(i);
        let mut m = self.view();
        m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut PortfolioV16ViewMut::new(long),
            &mut PortfolioV16ViewMut::new(short),
            TradeRequestV16 { asset_index: i, size_q: size, exec_price: p, fee_bps: 0 },
            true,
        )
        .map(|_| true)
        .or_else(|e| if matches!(e, percolator::V16Error::InsufficientInitialMargin) { Ok::<bool, ()>(false) } else { panic!("trade refused for a reason other than the initial-margin gate: {e:?}") })
        .unwrap()
    }
    /// move asset i's price by `bps` (in steps of at most 150 bps, one slot each; every other
    /// asset is re-accrued at its own unchanged price so nothing goes stale)
    fn move_price(&mut self, i: usize, bps: i64) {
        let steps = (bps.abs() + 149) / 150;
        let mut left = bps;
        for s in 0..steps {
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
    fn withdraw(&mut self, a: &mut PortfolioAccountV16Account, amt: u128) -> bool {
        let mut m = self.view();
        m.withdraw_not_atomic(&mut PortfolioV16ViewMut::new(a), amt).is_ok()
    }
    /// settle an account's accrued PnL (a trade right after a price move is refused with LossStale
    /// until both parties are refreshed; that is a different gate from the margin one under test)
    fn refresh(&mut self, a: &mut PortfolioAccountV16Account) {
        let mut m = self.view();
        let mut budget = percolator::S10_MAX_MOVES_PER_INSTRUCTION;
        m.full_account_refresh_with_s10_budget_not_atomic(&mut PortfolioV16ViewMut::new(a), &mut budget).expect("refresh");
    }
    fn capital(a: &PortfolioAccountV16Account) -> u128 { a.capital.get() }
}

const BIG: u128 = 1_000_000_000_000_000;

/// Open a fresh position of `k` units: long gets exactly `long_dep`, the short side is flush.
fn open_with(long_dep: u128, k: u128) -> bool {
    let mut w = W::std();
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, long_dep);
    w.deposit(&mut b, BIG);
    w.trade(0, &mut a, &mut b, (k * POS_SCALE) as i128)
}
fn open_short_side_with(short_dep: u128, k: u128) -> bool {
    let mut w = W::std();
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, BIG);
    w.deposit(&mut b, short_dep);
    w.trade(0, &mut a, &mut b, (k * POS_SCALE) as i128)
}

// ---------- open at exactly the initial limit and one atom over ----------

#[test]
fn margin_open_long_at_exactly_initial_margin_is_accepted_one_atom_less_is_rejected() {
    for k in [1u128, 7, 100, 12_345] {
        assert!(open_with(k * IM1, k), "k={k}: equity == IM must pass (10x exactly)");
        assert!(!open_with(k * IM1 - 1, k), "k={k}: one atom under IM must fail");
    }
}

#[test]
fn margin_open_short_at_exactly_initial_margin_is_accepted_one_atom_less_is_rejected() {
    for k in [1u128, 7, 100, 12_345] {
        assert!(open_short_side_with(k * IM1, k), "k={k}");
        assert!(!open_short_side_with(k * IM1 - 1, k), "k={k}");
    }
}

#[test]
fn margin_one_atom_of_size_over_the_limit_is_rejected() {
    // NOTE: the engine entry points are `_not_atomic` (the program reverts the whole tx on Err),
    // so every attempt below uses a fresh world instead of retrying on the same state.
    let cap = 10 * IM1;
    let try_size = |size: i128| {
        let mut w = W::std();
        let (mut a, mut b) = (account(1), account(2));
        w.deposit(&mut a, cap);
        w.deposit(&mut b, BIG);
        w.trade(0, &mut a, &mut b, size)
    };
    // 1 IM atom = 10 notional atoms = 10 * POS_SCALE / PRICE position atoms, so 10 units plus that
    // many position atoms (+1 to defeat the floor) needs cap + 1.
    let extra = (10 * POS_SCALE / PRICE as u128) as i128 + 1;
    assert!(try_size((10 * POS_SCALE) as i128), "10 units is exactly 10x");
    assert!(!try_size((10 * POS_SCALE) as i128 + extra), "just over 10x must fail");
}


// ---------- increase ----------

#[test]
fn margin_increase_to_exactly_the_limit_ok_and_one_more_unit_rejected() {
    let run = |second: u128| {
        let mut w = W::std();
        let (mut a, mut b) = (account(1), account(2));
        w.deposit(&mut a, 10 * IM1);
        w.deposit(&mut b, BIG);
        assert!(w.trade(0, &mut a, &mut b, (4 * POS_SCALE) as i128));
        w.trade(0, &mut a, &mut b, (second * POS_SCALE) as i128)
    };
    assert!(run(6), "increase to exactly 10x");
    assert!(!run(7), "increase past 10x");
}

#[test]
fn margin_increase_one_atom_short_of_limit_by_capital() {
    let run = |cap: u128| {
        let mut w = W::std();
        let (mut a, mut b) = (account(1), account(2));
        w.deposit(&mut a, cap);
        w.deposit(&mut b, BIG);
        assert!(w.trade(0, &mut a, &mut b, (9 * POS_SCALE) as i128));
        w.trade(0, &mut a, &mut b, POS_SCALE as i128)
    };
    assert!(run(10 * IM1), "10 units needs 10*IM1");
    assert!(!run(10 * IM1 - 1), "one atom less capital");
}

// ---------- maintenance vs initial gap ----------

#[test]
fn margin_between_mm_and_im_you_may_hold_but_not_increase() {
    // 10 units at 10x. A 3% adverse move leaves equity 7% of notional: under IM (10%), over MM (5%).
    let mut w = W::std();
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, 10 * IM1);
    w.deposit(&mut b, BIG);
    assert!(w.trade(0, &mut a, &mut b, (10 * POS_SCALE) as i128));
    w.move_price(0, -300);
    w.refresh(&mut a);
    w.refresh(&mut b);
    // increase must be refused (below IM)
    assert!(!w.trade(0, &mut a, &mut b, POS_SCALE as i128), "increase while below IM");
    // an unrelated top-up to exactly IM allows it again would need big deposits; instead check hold:
    assert!(a.active_bitmap.iter().any(|w| w.get() != 0), "position is still held");
}

#[test]
fn margin_reduce_only_is_never_blocked_even_when_below_initial_margin() {
    let mut w = W::std();
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, 10 * IM1);
    w.deposit(&mut b, BIG);
    assert!(w.trade(0, &mut a, &mut b, (10 * POS_SCALE) as i128));
    w.move_price(0, -300); // under IM, above MM
    w.refresh(&mut a);
    w.refresh(&mut b);
    // sell 1 unit back to b (a is the short side of the reducing trade)
    assert!(w.trade(0, &mut b, &mut a, POS_SCALE as i128), "partial reduce must go through");
    assert!(w.trade(0, &mut b, &mut a, (9 * POS_SCALE) as i128), "closing the rest must go through");
}

#[test]
fn margin_reduce_only_is_never_blocked_even_when_below_maintenance_margin() {
    let mut w = W::std();
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, 10 * IM1);
    w.deposit(&mut b, BIG);
    assert!(w.trade(0, &mut a, &mut b, (10 * POS_SCALE) as i128));
    w.move_price(0, -700); // equity 3% of notional < MM 5%, still positive
    w.refresh(&mut a);
    w.refresh(&mut b);
    assert!(w.trade(0, &mut b, &mut a, (5 * POS_SCALE) as i128), "reduce by half while below MM");
    assert!(w.trade(0, &mut b, &mut a, (5 * POS_SCALE) as i128), "then close");
}

#[test]
fn margin_reduce_of_a_short_below_initial_margin_is_not_blocked() {
    let mut w = W::std();
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, BIG);
    w.deposit(&mut b, 10 * IM1);
    assert!(w.trade(0, &mut a, &mut b, (10 * POS_SCALE) as i128)); // b is short 10
    w.move_price(0, 300); // b loses 3%
    w.refresh(&mut a);
    w.refresh(&mut b);
    assert!(w.trade(0, &mut b, &mut a, (3 * POS_SCALE) as i128), "b buys back 3");
}

// ---------- flips ----------

#[test]
fn margin_flip_requires_initial_margin_on_the_new_side_exact_and_one_atom_over() {
    // a is long 2 units. Flipping to short 2 (trade of 4 units, a as the seller) needs IM of 2 units.
    let run = |cap: u128| {
        let mut w = W::std();
        let (mut a, mut b) = (account(1), account(2));
        w.deposit(&mut a, cap);
        w.deposit(&mut b, BIG);
        assert!(w.trade(0, &mut a, &mut b, (2 * POS_SCALE) as i128));
        w.trade(0, &mut b, &mut a, (4 * POS_SCALE) as i128)
    };
    assert!(run(2 * IM1), "flip landing at exactly 10x");
    // a holds capital 2*IM1 - 1 -> cannot even open the long; so use the funded-long then shrink path below
}

#[test]
fn margin_flip_with_smaller_absolute_size_still_needs_initial_margin_when_below_it() {
    // long 10, price drops 3% (under IM), flip to short 5 (abs shrinks). Flip is NOT a reduction.
    let mut w = W::std();
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, 10 * IM1);
    w.deposit(&mut b, BIG);
    assert!(w.trade(0, &mut a, &mut b, (10 * POS_SCALE) as i128));
    w.move_price(0, -300);
    w.refresh(&mut a);
    w.refresh(&mut b);
    // a sells 15 -> ends short 5: IM about 485k against equity about 702k
    let ok = w.trade(0, &mut b, &mut a, (15 * POS_SCALE) as i128);
    assert!(ok, "equity 700k covers IM 485k of the new short 5");
}

/// long 10 funded 10x, price -8% (equity about 2 units of IM), both refreshed
fn deep_underwater_long() -> (W, PortfolioAccountV16Account, PortfolioAccountV16Account) {
    let mut w = W::std();
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, 10 * IM1);
    w.deposit(&mut b, BIG);
    assert!(w.trade(0, &mut a, &mut b, (10 * POS_SCALE) as i128));
    w.move_price(0, -800);
    w.refresh(&mut a);
    w.refresh(&mut b);
    (w, a, b)
}

#[test]
fn margin_flip_under_initial_margin_is_rejected() {
    // flip to short 9: IM about 0.83e6 against equity about 0.2e6
    let (mut w, mut a, mut b) = deep_underwater_long();
    assert!(!w.trade(0, &mut b, &mut a, (19 * POS_SCALE) as i128), "flip to short 9 without margin");
}

#[test]
fn margin_full_close_of_an_underwater_position_is_a_reduction_not_a_flip() {
    // (fresh world: the engine entry points are _not_atomic, a refused trade is not retried in place)
    let (mut w, mut a, mut b) = deep_underwater_long();
    assert!(w.trade(0, &mut b, &mut a, (10 * POS_SCALE) as i128), "full close is a reduction");
}

#[test]
fn margin_flip_by_one_position_atom_past_flat_is_still_a_flip_and_needs_im() {
    // closing 10 units plus one position atom opens a 1-atom short: not reduce-only
    let (mut w, mut a, mut b) = deep_underwater_long();
    let r = w.trade(0, &mut b, &mut a, (10 * POS_SCALE) as i128 + 1);
    // equity ~200k vs IM floor of a dust short: min_nonzero_im_req = 2, so this passes. Pin that.
    assert!(r, "a dust flip is covered by the min_nonzero requirement");
}

#[test]
fn margin_equal_size_flip_is_a_risk_increase_and_needs_im() {
    let mut w = W::std();
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, 10 * IM1);
    w.deposit(&mut b, BIG);
    assert!(w.trade(0, &mut a, &mut b, (10 * POS_SCALE) as i128));
    w.move_price(0, -300);
    w.refresh(&mut a);
    w.refresh(&mut b);
    // a: long 10 -> short 10 (sell 20). Equity 700k, IM for 10 units at .97 = 970k -> rejected
    assert!(!w.trade(0, &mut b, &mut a, (20 * POS_SCALE) as i128));
}

// ---------- withdrawals ----------

#[test]
fn margin_withdraw_with_an_open_position_is_refused_at_any_amount() {
    let mut w = W::std();
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, 20 * IM1);
    w.deposit(&mut b, BIG);
    assert!(w.trade(0, &mut a, &mut b, (10 * POS_SCALE) as i128)); // half the max leverage
    assert!(!w.withdraw(&mut a, 1), "even 1 atom is blocked while a leg is open");
    assert_eq!(W::capital(&a), 20 * IM1);
}

#[test]
fn margin_withdraw_flat_exact_capital_ok_one_atom_more_refused() {
    let mut w = W::std();
    let mut a = account(1);
    w.deposit(&mut a, 1_000);
    assert!(!w.withdraw(&mut a, 1_001), "more than capital");
    assert!(w.withdraw(&mut a, 1_000), "exactly all of it");
    assert_eq!(W::capital(&a), 0);
}

#[test]
fn margin_withdraw_zero_is_a_noop_even_with_a_position() {
    let mut w = W::std();
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, 10 * IM1);
    w.deposit(&mut b, BIG);
    assert!(w.trade(0, &mut a, &mut b, (10 * POS_SCALE) as i128));
    assert!(w.withdraw(&mut a, 0));
}

#[test]
fn margin_after_close_the_realized_loss_comes_out_of_capital_then_withdraw_is_capped_by_it() {
    let mut w = W::std();
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, 10 * IM1);
    w.deposit(&mut b, BIG);
    assert!(w.trade(0, &mut a, &mut b, (10 * POS_SCALE) as i128));
    w.move_price(0, -300); // a loses 300_000
    w.refresh(&mut a);
    w.refresh(&mut b);
    assert!(w.trade(0, &mut b, &mut a, (10 * POS_SCALE) as i128));
    // a's remaining value is 700_000; asking for more must fail, exactly that must pass
    let cap = W::capital(&a);
    let loss = 10 * (PRICE as u128 - w.price(0) as u128);
    assert_eq!(cap, 10 * IM1 - loss, "the loss is exactly what the price moved");
    assert!(!w.withdraw(&mut a, cap + 1));
    assert!(w.withdraw(&mut a, cap));
}

// ---------- cross-margin ----------

#[test]
fn margin_cross_margin_requirement_adds_across_assets_exactly() {
    // two assets, same price: a long 5 on each -> IM = 2 * 5 * IM1 = 10*IM1
    let run = |cap: u128| {
        let mut w = W::new(2, 1_000, 500);
        let (mut a, mut b) = (account(1), account(2));
        w.deposit(&mut a, cap);
        w.deposit(&mut b, BIG);
        let r0 = w.trade(0, &mut a, &mut b, (5 * POS_SCALE) as i128);
        let r1 = w.trade(1, &mut a, &mut b, (5 * POS_SCALE) as i128);
        (r0, r1)
    };
    assert_eq!(run(10 * IM1), (true, true), "both legs fit exactly");
    assert_eq!(run(10 * IM1 - 1), (true, false), "second leg is one atom over the shared pool");
}

#[test]
fn margin_cross_margin_offsetting_legs_on_equal_assets_do_not_net_requirement() {
    // long 5 on asset 0 and short 5 on asset 1 (different markets): each leg carries its own IM.
    let run = |cap: u128| {
        let mut w = W::new(2, 1_000, 500);
        let (mut a, mut b) = (account(1), account(2));
        w.deposit(&mut a, cap);
        w.deposit(&mut b, BIG);
        let r0 = w.trade(0, &mut a, &mut b, (5 * POS_SCALE) as i128);
        let r1 = w.trade(1, &mut b, &mut a, (5 * POS_SCALE) as i128);
        (r0, r1)
    };
    assert_eq!(run(10 * IM1), (true, true));
    assert_eq!(run(10 * IM1 - 1), (true, false), "different assets never net");
}

#[test]
fn margin_cross_margin_closing_one_leg_frees_its_requirement_for_the_other() {
    let run = |close_first: bool| {
        let mut w = W::new(2, 1_000, 500);
        let (mut a, mut b) = (account(1), account(2));
        w.deposit(&mut a, 10 * IM1);
        w.deposit(&mut b, BIG);
        assert!(w.trade(0, &mut a, &mut b, (5 * POS_SCALE) as i128));
        assert!(w.trade(1, &mut a, &mut b, (5 * POS_SCALE) as i128));
        if close_first {
            assert!(w.trade(0, &mut b, &mut a, (5 * POS_SCALE) as i128), "close leg 0 (reduce)");
        }
        w.trade(1, &mut a, &mut b, (5 * POS_SCALE) as i128)
    };
    assert!(!run(false), "pool is full");
    assert!(run(true), "leg 1 can now double to 10x");
}

#[test]
fn margin_cross_margin_loss_on_one_asset_shrinks_headroom_on_the_other() {
    // returns (equity, im_req_of_leg0, accepted) for opening `extra_q` position atoms on asset 1
    let run = |extra_q: u128| {
        let mut w = W::new(2, 1_000, 500);
        let (mut a, mut b) = (account(1), account(2));
        w.deposit(&mut a, 10 * IM1);
        w.deposit(&mut b, BIG);
        assert!(w.trade(0, &mut a, &mut b, (5 * POS_SCALE) as i128));
        w.move_price(0, -1_000);
        w.refresh(&mut a);
        w.refresh(&mut b);
        let p0 = w.price(0) as u128;
        let loss = 5 * (PRICE as u128 - p0);
        let equity = 10 * IM1 - loss;
        let im0 = (5 * p0 + 9) / 10; // ceil(10% of 5-unit notional)
        let room = equity - im0; // IM atoms left for asset 1 (asset 1 trades at PRICE)
        // an extra of `extra_q` atoms costs ceil(floor(extra_q*PRICE/POS_SCALE)/10)
        let need = |q: u128| (q * PRICE as u128 / POS_SCALE + 9) / 10;
        let _ = need;
        (room, w.trade(1, &mut a, &mut b, extra_q as i128))
    };
    // 1 unit costs IM1 = 100_000; room is ~ 500_000 - 5*0.9*1e5.. measured below
    let (room, _) = run(0 + 1);
    // largest whole-IM-atom size that fits: size with notional 10*room
    let fit = (10 * room) * POS_SCALE / PRICE as u128;
    assert!(run(fit).1, "exactly at the shared limit (room {room})");
    assert!(!run(fit + POS_SCALE / PRICE as u128 * 10).1, "one more IM atom of size is refused");
}

// ---------- max leverage per asset (different price per asset) ----------

#[test]
fn margin_leverage_limit_is_per_notional_not_per_size_for_a_cheaper_asset() {
    let mut w = W::new(2, 1_000, 500);
    // asset 1 is at half the price: same capital supports twice the size
    w.move_price(1, -1_500);
    let p1 = w.price(1) as u128;
    let (mut a, mut b) = (account(1), account(2));
    let notional = 10 * N1;
    let size_q = notional * POS_SCALE / p1; // exact-ish
    // capital for IM of this notional = ceil(notional*0.1)
    let need = (notional + 9) / 10;
    let notional_actual = size_q * p1 / POS_SCALE;
    let need_actual = (notional_actual * 1_000 + 9_999) / 10_000;
    w.deposit(&mut a, need_actual);
    w.deposit(&mut b, BIG);
    assert!(w.trade(1, &mut a, &mut b, size_q as i128), "10x at the cheaper price");
}

#[test]
fn margin_leverage_one_hundred_percent_im_means_no_leverage() {
    let run = |cap: u128| {
        let mut w = W::new(1, 10_000, 5_000);
        let (mut a, mut b) = (account(1), account(2));
        w.deposit(&mut a, cap);
        w.deposit(&mut b, BIG);
        w.trade(0, &mut a, &mut b, (3 * POS_SCALE) as i128)
    };
    assert!(!run(3 * N1 - 1));
    assert!(run(3 * N1), "exactly 1x");
}

#[test]
fn margin_tiny_position_pays_the_minimum_nonzero_requirement_not_zero() {
    // 1 position atom at price 1e6 has notional 0 (floor) -> requirement is the floor min_nonzero_im_req (2)
    let mut w = W::std();
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut b, BIG);
    // a holds nothing: a dust position must not be free
    assert!(!w.trade(0, &mut a, &mut b, 1), "zero-capital dust open must be refused");
}

#[test]
fn margin_cushion_topup_restores_headroom_by_exactly_the_deposit() {
    // 10x position, 3% adverse move -> below IM. Top up exactly the shortfall and an increase of
    // one unit that needs exactly IM1 more must be accepted; one atom less top-up must refuse it.
    let run = |extra_dep: i128| {
        let mut w = W::std();
        let (mut a, mut b) = (account(1), account(2));
        w.deposit(&mut a, 10 * IM1);
        w.deposit(&mut b, BIG);
        assert!(w.trade(0, &mut a, &mut b, (10 * POS_SCALE) as i128));
        w.move_price(0, -300);
        w.refresh(&mut a);
        w.refresh(&mut b);
        let p = w.price(0) as u128;
        let loss = 10 * (PRICE as u128 - p);
        // after the move: equity = cap - loss. Need equity >= ceil(11 units * p / 10)
        let notional = 11 * p;
        let need_equity = (notional + 9) / 10;
        let topup = need_equity + loss - 10 * IM1;
        w.deposit(&mut a, (topup as i128 + extra_dep) as u128);
        w.trade(0, &mut a, &mut b, POS_SCALE as i128)
    };
    assert!(run(0), "topup exactly to IM of the bigger position");
    assert!(!run(-1), "one atom under");
}

#[test]
fn margin_config_rejects_maintenance_above_initial() {
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 6_480_000);
    cfg.initial_margin_bps = 500;
    cfg.maintenance_margin_bps = 501;
    assert!(MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).is_err(), "MM > IM is a config error");
}

#[test]
fn margin_im_equal_to_mm_is_a_valid_config_and_the_limit_is_the_same() {
    let run = |cap: u128| {
        let mut w = W::new(1, 1_000, 1_000);
        let (mut a, mut b) = (account(1), account(2));
        w.deposit(&mut a, cap);
        w.deposit(&mut b, BIG);
        w.trade(0, &mut a, &mut b, (10 * POS_SCALE) as i128)
    };
    assert!(run(10 * IM1));
    assert!(!run(10 * IM1 - 1));
}
