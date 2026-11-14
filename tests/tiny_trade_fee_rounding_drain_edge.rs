//! Many tiny trades and fee rounding on the v2.2 engine (job P34, tests only).
//!
//! Questions asked of a memecoin-style market where one position atom is worth less than one
//! collateral atom:
//!   * does splitting one trade into N tiny trades pay less total fee (fee rounding to zero)?
//!   * can N tiny round trips leak value out of the vault / into the traders (conservation)?
//!   * does the fee floor (ceil notional, ceil fee) block both?
//! Skips what fees_liens_edge / rounding_edge already hold (single-fill ceil, the two-leg batch
//! example, mul_div brackets); everything here is about repetition and sweeps.
//!
//! Every test name contains `tiny_trade`, so `cargo test --test tiny_trade_fee_rounding_drain_edge
//! -- tiny_trade` selects the area.

use percolator::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, TradeRequestV16, V16Config, V16Error, POS_SCALE,
};

type Fixture = (MarketGroupV16HeaderAccount, Vec<Market<u64>>);

fn market(price: u64, max_trading_fee_bps: u64) -> Fixture {
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    cfg.max_trading_fee_bps = max_trading_fee_bps;
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut markets = vec![Market::new(0u64, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, price, 1)
        .unwrap();
    (header, markets)
}

fn account(seed: u8) -> PortfolioAccountV16Account {
    let h = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        [1; 32], [seed; 32], [3; 32],
    ));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(h).unwrap();
    a
}

fn req(size_q: i128, exec_price: u64, fee_bps: u64) -> TradeRequestV16 {
    TradeRequestV16 { asset_index: 0, size_q, exec_price, fee_bps }
}

struct World {
    header: MarketGroupV16HeaderAccount,
    markets: Vec<Market<u64>>,
    long: PortfolioAccountV16Account,
    short: PortfolioAccountV16Account,
    deposited: u128,
}

impl World {
    /// one asset at `price`, both sides deposit `cap`
    fn new(price: u64, max_fee_bps: u64, cap: u128) -> World {
        let (mut header, mut markets) = market(price, max_fee_bps);
        let (mut long, mut short) = (account(11), account(12));
        {
            let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
            m.deposit_not_atomic(&mut PortfolioV16ViewMut::new(&mut long), cap).unwrap();
            m.deposit_not_atomic(&mut PortfolioV16ViewMut::new(&mut short), cap).unwrap();
        }
        World { header, markets, long, short, deposited: 2 * cap }
    }

    /// one fill, long is the taker when size > 0. Returns total fee charged.
    fn try_trade(&mut self, size_q: i128, price: u64, fee_bps: u64) -> Result<u128, V16Error> {
        let mut m = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let out = m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut PortfolioV16ViewMut::new(&mut self.long),
            &mut PortfolioV16ViewMut::new(&mut self.short),
            req(size_q, price, fee_bps),
            size_q > 0,
        )?;
        Ok(out.fee_a + out.fee_b)
    }
    fn trade(&mut self, size_q: i128, price: u64, fee_bps: u64) -> u128 {
        self.try_trade(size_q, price, fee_bps).expect("trade fills")
    }
    fn vault(&self) -> u128 { self.header.vault.get() }
    fn c_tot(&self) -> u128 { self.header.c_tot.get() }
    fn ins(&self) -> u128 { self.header.insurance.get() }
    fn oi(&self) -> (u128, u128) {
        let a = &self.markets[0].engine.asset;
        (a.oi_eff_long_q.get(), a.oi_eff_short_q.get())
    }
    fn caps(&self) -> (u128, u128) { (self.long.capital.get(), self.short.capital.get()) }
    fn pnls(&self) -> (i128, i128) { (self.long.pnl.get(), self.short.pnl.get()) }
    /// V == deposits, V >= C + I, C + I == deposits (no leak in either direction), shape ok
    fn assert_conserved(&mut self) {
        assert_eq!(self.vault(), self.deposited, "vault moved");
        let (lc, sc) = self.caps();
        assert_eq!(self.c_tot(), lc + sc, "c_tot != sum of capitals");
        assert_eq!(self.c_tot() + self.ins(), self.deposited, "capital + insurance != deposits");
        MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets)
            .validate_shape()
            .unwrap();
    }
}

const BIG: u128 = 1_000_000_000_000_000;

/// ceil(ceil(size * price / POS_SCALE) * bps / 10_000): the fee the engine's formula gives
fn model_fee(size_q: u128, price: u64, bps: u64) -> u128 {
    let notional = (size_q * price as u128 + POS_SCALE - 1) / POS_SCALE;
    (notional * bps as u128 + 9_999) / 10_000
}

// ---------- the minimum trade ----------

#[test]
fn tiny_trade_zero_size_is_refused_and_changes_nothing() {
    let mut w = World::new(1_000_000, 10, BIG);
    let before = (w.c_tot(), w.ins(), w.oi());
    assert!(w.try_trade(0, 1_000_000, 1).is_err());
    assert_eq!((w.c_tot(), w.ins(), w.oi()), before);
}

#[test]
fn tiny_trade_one_position_atom_is_accepted_and_pays_one_fee_atom() {
    // floor notional = 1 * 999_999 / POS_SCALE (1e6) = 0: the fill is sub-atom
    let mut w = World::new(999_999, 1, BIG);
    let fee = w.trade(1, 999_999, 1);
    assert_eq!(fee, 1, "the sub-atom fill still pays the one-atom floor");
    assert_eq!(w.oi(), (1, 1));
    assert_eq!(w.ins(), 1);
    w.assert_conserved();
}

#[test]
fn tiny_trade_fee_bps_zero_is_the_only_free_setting() {
    // control: a market that configures fee 0 charges nothing for any size. That is a config
    // choice (max_trading_fee_bps = 0 is the default), not a rounding hole.
    let mut w = World::new(1_000_000, 0, BIG);
    for s in [1i128, 7, 1_000, POS_SCALE as i128] {
        assert_eq!(w.trade(s, 1_000_000, 0), 0);
    }
    assert_eq!(w.ins(), 0);
    w.assert_conserved();
    // and with a nonzero fee bps no (size, price) pair in this grid is free
    let sizes = [1u128, 2, 3, 9, 10, 99, 1_000, POS_SCALE - 1, POS_SCALE, POS_SCALE + 1];
    let prices = [1u64, 2, 3, 7, 100, 999_999, 1_000_000, 123_456_789];
    for &p in &prices {
        for &s in &sizes {
            let mut w = World::new(p, 1, BIG);
            let fee = w.trade(s as i128, p, 1);
            assert!(fee >= 1, "free fill at price {p} size {s}");
            assert_eq!(fee, model_fee(s, p, 1), "price {p} size {s}");
        }
    }
}

#[test]
fn tiny_trade_fee_bps_at_the_cap_and_one_over_is_refused_for_dust() {
    let mut w = World::new(1_000_000, 5, BIG);
    assert_eq!(w.try_trade(1, 1_000_000, 6), Err(V16Error::InvalidConfig));
    assert_eq!(w.trade(1, 1_000_000, 5), 1);
}

// ---------- splitting a trade never lowers the fee ----------

#[test]
fn tiny_trade_split_sweep_never_pays_less_than_the_single_trade() {
    let prices = [1u64, 3, 100, 999_999, 1_000_000];
    let totals = [1u128, 5, 1_000, POS_SCALE / 7, POS_SCALE, 3 * POS_SCALE + 11];
    let splits = [1u128, 2, 3, 10, 100, 997];
    let bps = [1u64, 3, 10];
    for &p in &prices {
        for &total in &totals {
            for &b in &bps {
                let single = model_fee(total, p, b);
                for &n in &splits {
                    if n > total {
                        continue;
                    }
                    // n-1 equal parts and the remainder in the last part, sizes never zero
                    let part = total / n;
                    let mut w = World::new(p, 10, BIG);
                    let mut paid = 0u128;
                    for i in 0..n {
                        let s = if i + 1 == n { total - part * (n - 1) } else { part };
                        paid += w.trade(s as i128, p, b);
                    }
                    assert_eq!(w.oi().0, total, "position lost atoms: p {p} total {total} n {n}");
                    assert!(
                        paid >= single,
                        "split saved fee: p {p} total {total} bps {b} n {n}: {paid} < {single}"
                    );
                    w.assert_conserved();
                }
            }
        }
    }
}

#[test]
fn tiny_trade_n_one_atom_fills_pay_n_fee_atoms_at_one_atom_notional() {
    // price 1: notional of one fill is ceil(size/POS_SCALE) = 1, fee ceil(1 * 1 / 10_000) = 1.
    // A single trade of the same total n atoms pays 1 atom; splitting costs n.
    for n in [1u128, 10, 1_000] {
        let mut w = World::new(1, 1, BIG);
        let mut paid = 0;
        for _ in 0..n {
            paid += w.trade(1, 1, 1);
        }
        assert_eq!(paid, n);
        assert_eq!(w.ins(), n);
        assert_eq!(w.oi().0, n);
        w.assert_conserved();
    }
}

#[test]
fn tiny_trade_fee_is_charged_to_the_taker_whichever_side_it_is() {
    let mut w = World::new(1, 1, BIG);
    let (l0, s0) = w.caps();
    w.trade(1, 1, 1);
    assert_eq!(w.caps(), (l0 - 1, s0), "long taker pays, short maker does not");
    w.trade(-1, 1, 1);
    assert_eq!(w.caps(), (l0 - 1, s0 - 1), "short taker pays, long maker does not");
    let (lc, sc) = w.caps();
    for _ in 0..50 {
        w.trade(1, 1, 1); // long taker
        w.trade(-1, 1, 1); // short taker
    }
    let (lc2, sc2) = w.caps();
    assert_eq!((lc - lc2, sc - sc2), (50, 50));
    assert_eq!(w.ins(), 102);
    assert_eq!(w.oi(), (0, 0));
    w.assert_conserved();
}

// ---------- round trips conserve value ----------

fn round_trips(n: u128, size: i128, price: u64, bps: u64) {
    let mut w = World::new(price, 10, BIG);
    let mut paid = 0u128;
    for _ in 0..n {
        paid += w.trade(size, price, bps);
        paid += w.trade(-size, price, bps);
    }
    let per = model_fee(size as u128, price, bps);
    assert_eq!(paid, 2 * n * per, "fee per fill drifted: n {n} size {size} price {price}");
    assert_eq!(w.ins(), paid);
    assert_eq!(w.oi(), (0, 0), "dust position left after {n} round trips");
    assert_eq!(w.pnls(), (0, 0), "round trip at the mark moved pnl");
    let (lc, sc) = w.caps();
    assert_eq!(lc + sc + w.ins(), w.deposited);
    w.assert_conserved();
}

#[test]
fn tiny_trade_round_trips_one_to_ten_thousand_conserve_value_with_fee() {
    for n in [1u128, 2, 10, 100, 1_000, 10_000] {
        round_trips(n, 1, 1_000_000, 1);
    }
}

#[test]
fn tiny_trade_round_trips_at_odd_sizes_and_prices_conserve_value() {
    for (size, price) in [(1i128, 1u64), (3, 7), (999_999, 1), (333_333, 3), (1, 999_999)] {
        for n in [1u128, 10, 300] {
            round_trips(n, size, price, 3);
        }
    }
}

#[test]
fn tiny_trade_round_trips_with_zero_fee_conserve_value_exactly() {
    for n in [1u128, 100, 5_000] {
        round_trips(n, 7, 123_457, 0);
    }
}

#[test]
fn tiny_trade_open_many_then_close_once_matches_open_once_then_close_many() {
    let n = 500u128;
    let (price, size) = (777u64, 1_333i128);
    let mut a = World::new(price, 10, BIG);
    let mut b = World::new(price, 10, BIG);
    let mut fa = 0;
    for _ in 0..n {
        fa += a.trade(size, price, 2);
    }
    fa += a.trade(-(size * n as i128), price, 2);
    let mut fb = b.trade(size * n as i128, price, 2);
    for _ in 0..n {
        fb += b.trade(-size, price, 2);
    }
    assert_eq!(a.oi(), (0, 0));
    assert_eq!(b.oi(), (0, 0));
    // fees land in insurance and nothing else moves
    assert_eq!(a.ins(), fa);
    assert_eq!(b.ins(), fb);
    a.assert_conserved();
    b.assert_conserved();
    assert_eq!(fa, fb);
    // splitting never costs less than one open plus one close of the whole size
    let one = 2 * model_fee(size as u128 * n, price, 2);
    assert!(fa >= one && fb >= one, "fa {fa} fb {fb} one {one}");
}

// ---------- off-mark fills and a price move with tiny sizes ----------

#[test]
fn tiny_trade_off_mark_fills_never_credit_the_pair_more_than_deposited() {
    // exec price 1 atom off the mark on sub-atom fills. Whatever the engine does with the
    // fraction, the pair's capital + pnl + insurance must not exceed what was put in.
    for off in [-1i64, 1] {
        let mark = 1_000u64;
        let mut w = World::new(mark, 10, BIG);
        for _ in 0..200 {
            let p = (mark as i64 + off) as u64;
            if w.try_trade(1, p, 1).is_err() {
                panic!("off-mark fill refused at off {off}");
            }
            w.trade(-1, mark, 1);
        }
        let (lc, sc) = w.caps();
        assert_eq!(w.oi(), (0, 0));
        let (lp, sp) = w.pnls();
        let total = lc as i128 + sc as i128 + lp + sp + w.ins() as i128;
        assert!(total <= w.deposited as i128, "off {off}: pair+insurance grew by {}", total - w.deposited as i128);
        assert_eq!(w.vault(), w.deposited);
    }
}

#[test]
fn tiny_trade_open_ten_thousand_dust_fills_then_close_in_one_conserves_value() {
    let mut w = World::new(1, 1, BIG);
    let n = 10_000u128;
    let mut paid = 0;
    for _ in 0..n {
        paid += w.trade(1, 1, 1);
    }
    assert_eq!(paid, n);
    assert_eq!(w.oi(), (n, n));
    // the closing fill is one trade of n atoms: ceil(n/POS_SCALE) = 1 notional, fee 1
    paid += w.trade(-(n as i128), 1, 1);
    assert_eq!(paid, n + 1);
    assert_eq!(w.oi(), (0, 0));
    assert_eq!(w.ins(), paid);
    w.assert_conserved();
}
