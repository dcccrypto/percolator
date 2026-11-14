//! PnL warmup / haircut / released-PnL-to-capital edge tests for the v2.2 engine.
//! Tests only; no product code is touched.
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]
use percolator::POS_SCALE;
use percolator::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, TradeRequestV16, V16Config, V16Error, V16PodU64,
};

const PRICE: u64 = 1_000_000;
const BIG: u128 = 1_000_000_000_000_000;

fn account(seed: u32) -> PortfolioAccountV16Account {
    let mut key = [0u8; 32];
    key[..4].copy_from_slice(&seed.to_le_bytes());
    key[31] = 0x5A;
    let header =
        ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new([1; 32], key, [3; 32]));
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
        header
            .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, PRICE, 1)
            .unwrap();
        W { header, markets, slot: 1 }
    }
    fn view(&mut self) -> MarketGroupV16ViewMut<'_, u64> {
        MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets)
    }
    fn price(&self) -> u64 {
        self.markets[0].engine.asset.effective_price.get()
    }
    fn deposit(&mut self, a: &mut PortfolioAccountV16Account, amt: u128) {
        let mut m = self.view();
        m.deposit_not_atomic(&mut PortfolioV16ViewMut::new(a), amt).unwrap();
    }
    fn trade(
        &mut self,
        long: &mut PortfolioAccountV16Account,
        short: &mut PortfolioAccountV16Account,
        size: i128,
    ) {
        let p = self.price();
        let mut m = self.view();
        m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut PortfolioV16ViewMut::new(long),
            &mut PortfolioV16ViewMut::new(short),
            TradeRequestV16 { asset_index: 0, size_q: size, exec_price: p, fee_bps: 0 },
            true,
        )
        .unwrap();
    }
    fn move_price(&mut self, bps: i64) {
        let mut left = bps;
        while left != 0 {
            let d = left.clamp(-150, 150);
            left -= d;
            self.slot += 1;
            let old = self.price() as i128;
            let new = (old + old * d as i128 / 10_000) as u64;
            let slot = self.slot;
            let mut m = self.view();
            m.accrue_asset_to_not_atomic(0, slot, new, 0, true).expect("accrue");
            m.markets[0].engine.asset.raw_oracle_target_price = V16PodU64::new(new);
        }
    }
    fn refresh(&mut self, a: &mut PortfolioAccountV16Account) {
        let mut m = self.view();
        let mut budget = percolator::S10_MAX_MOVES_PER_INSTRUCTION;
        m.full_account_refresh_with_s10_budget_not_atomic(&mut PortfolioV16ViewMut::new(a), &mut budget)
            .expect("refresh");
    }
    /// refresh first (conversion needs a valid health cert), then convert
    fn convert(&mut self, a: &mut PortfolioAccountV16Account) -> Result<u128, V16Error> {
        self.refresh(a);
        self.convert_raw(a)
    }
    fn convert_raw(&mut self, a: &mut PortfolioAccountV16Account) -> Result<u128, V16Error> {
        let mut m = self.view();
        m.convert_released_pnl_to_capital_not_atomic(&mut PortfolioV16ViewMut::new(a))
    }
}

impl W {
    fn liquidate(&mut self, a: &mut PortfolioAccountV16Account) -> Result<(), V16Error> {
        let mut m = self.view();
        m.liquidate_account_not_atomic(
            &mut PortfolioV16ViewMut::new(a),
            percolator::LiquidationRequestV16 { asset_index: 0 },
        )
        .map(|_| ())
    }
    /// refresh, running the permissionless Refresh crank while B-stale
    fn refresh_crank(&mut self, a: &mut PortfolioAccountV16Account) {
        let slot = self.slot;
        let price = self.price();
        let mut g = self.view();
        let mut v = PortfolioV16ViewMut::new(a);
        for _ in 0..32 {
            match g.full_account_refresh_not_atomic(&mut v) {
                Err(V16Error::BStale) => {
                    g.permissionless_crank_not_atomic(
                        &mut v,
                        percolator::PermissionlessCrankRequestV16 {
                            now_slot: slot,
                            asset_index: 0,
                            effective_price: price,
                            funding_rate_e9: 0,
                            action: percolator::PermissionlessCrankActionV16::Refresh,
                        },
                    )
                    .unwrap();
                }
                r => {
                    r.unwrap();
                    return;
                }
            }
        }
        panic!("refresh did not converge");
    }
    /// close `size` of a's long against b, then refresh both
    fn close_flat(&mut self, a: &mut PortfolioAccountV16Account, b: &mut PortfolioAccountV16Account, size: i128) {
        self.refresh(a);
        self.refresh(b);
        self.trade(b, a, size);
        self.refresh(a);
        self.refresh(b);
    }
    fn vault(&self) -> u128 {
        self.header.vault.get()
    }
    fn c_tot(&self) -> u128 {
        self.header.c_tot.get()
    }
    fn insurance(&self) -> u128 {
        self.header.insurance.get()
    }
}

/// winner `a` (long 10 units, +bps), loser `b`, both settled and flat. Returns the world.
fn settled_winner(a_dep: u128, bps: i64) -> (W, PortfolioAccountV16Account, PortfolioAccountV16Account) {
    let mut w = W::new();
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, a_dep);
    w.deposit(&mut b, BIG);
    w.trade(&mut a, &mut b, (10 * POS_SCALE) as i128);
    w.move_price(bps);
    w.close_flat(&mut a, &mut b, (10 * POS_SCALE) as i128);
    (w, a, b)
}

const DEP: u128 = 1_000_000;

#[test]
fn convert_zero_pnl_is_a_noop() {
    let mut w = W::new();
    let mut a = account(1);
    w.deposit(&mut a, DEP);
    let (v, c) = (w.vault(), w.c_tot());
    assert_eq!(w.convert(&mut a), Ok(0));
    assert_eq!((a.capital.get(), a.pnl.get(), w.vault(), w.c_tot()), (DEP, 0, v, c));
}

#[test]
fn convert_negative_pnl_is_a_noop() {
    let (mut w, mut a, mut b) = settled_winner(BIG, -300);
    // a lost: pnl was paid out of capital, nothing positive to convert
    assert!(a.pnl.get() <= 0);
    let cap = a.capital.get();
    assert_eq!(w.convert(&mut a), Ok(0));
    assert_eq!(a.capital.get(), cap);
    // the winner side (b) can convert, the loser cannot gain
    assert!(b.pnl.get() >= 0);
}

#[test]
fn convert_fully_backed_profit_moves_whole_pnl_into_capital() {
    let (mut w, mut a, _b) = settled_winner(DEP, 300);
    let pnl = a.pnl.get();
    assert!(pnl > 0, "winner has positive pnl");
    assert_eq!(a.reserved_pnl.get(), 0);
    let got = w.convert(&mut a).unwrap();
    assert_eq!(got as i128, pnl, "no haircut when the vault is whole");
    assert_eq!(a.pnl.get(), 0);
    assert_eq!(a.capital.get(), DEP + got);
}

#[test]
fn convert_conserves_vault_and_moves_c_tot_by_exactly_the_converted_amount() {
    let (mut w, mut a, _b) = settled_winner(DEP, 300);
    let (v0, c0, i0) = (w.vault(), w.c_tot(), w.insurance());
    let got = w.convert(&mut a).unwrap();
    assert_eq!(w.vault(), v0, "conversion moves no tokens");
    assert_eq!(w.c_tot(), c0 + got);
    assert_eq!(w.insurance(), i0, "insurance untouched by a fully backed conversion");
    assert!(w.c_tot() + w.insurance() <= w.vault());
}

#[test]
fn convert_is_idempotent_repeated_calls_return_zero() {
    let (mut w, mut a, _b) = settled_winner(DEP, 300);
    let first = w.convert(&mut a).unwrap();
    assert!(first > 0);
    let snap = (a.capital.get(), a.pnl.get(), w.c_tot(), w.vault());
    for _ in 0..5 {
        assert_eq!(w.convert(&mut a), Ok(0));
    }
    assert_eq!(snap, (a.capital.get(), a.pnl.get(), w.c_tot(), w.vault()));
}

#[test]
fn convert_total_value_is_conserved_across_both_sides() {
    let (mut w, mut a, mut b) = settled_winner(DEP, 300);
    let total = |a: &PortfolioAccountV16Account, b: &PortfolioAccountV16Account| {
        a.capital.get() as i128 + a.pnl.get() + b.capital.get() as i128 + b.pnl.get()
    };
    let before = total(&a, &b);
    w.convert(&mut a).unwrap();
    w.convert(&mut b).unwrap();
    assert_eq!(total(&a, &b), before, "conversion only relabels pnl as capital");
}

#[test]
fn convert_order_of_two_winners_does_not_change_totals() {
    let run = |a_first: bool| {
        let mut w = W::new();
        let (mut a1, mut a2, mut b) = (account(1), account(3), account(2));
        w.deposit(&mut a1, DEP);
        w.deposit(&mut a2, DEP);
        w.deposit(&mut b, BIG);
        w.trade(&mut a1, &mut b, (6 * POS_SCALE) as i128);
        w.trade(&mut a2, &mut b, (4 * POS_SCALE) as i128);
        w.move_price(300);
        w.refresh(&mut a1);
        w.refresh(&mut a2);
        w.refresh(&mut b);
        w.trade(&mut b, &mut a1, (6 * POS_SCALE) as i128);
        w.trade(&mut b, &mut a2, (4 * POS_SCALE) as i128);
        for x in [&mut a1, &mut a2, &mut b] {
            w.refresh(x);
        }
        let (g1, g2);
        if a_first {
            g1 = w.convert(&mut a1).unwrap();
            g2 = w.convert(&mut a2).unwrap();
        } else {
            g2 = w.convert(&mut a2).unwrap();
            g1 = w.convert(&mut a1).unwrap();
        }
        (g1, g2, w.c_tot(), w.vault())
    };
    assert_eq!(run(true), run(false));
}

#[test]
fn convert_refused_while_position_is_open_and_unsettled() {
    let mut w = W::new();
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, DEP);
    w.deposit(&mut b, BIG);
    w.trade(&mut a, &mut b, (10 * POS_SCALE) as i128);
    w.move_price(300);
    w.refresh(&mut a);
    w.refresh(&mut b);
    // pnl is positive but the leg is still open: the conversion is gated
    assert!(a.pnl.get() > 0);
    assert!(w.convert_raw(&mut a).is_err());
    assert_eq!(a.capital.get(), DEP, "no capital created by a refused conversion");
}

#[test]
fn convert_tiny_profit_one_price_step_rounds_down_never_up() {
    // 1 bp move on 10 units: profit is small; converted must never exceed pnl.
    let (mut w, mut a, _b) = settled_winner(DEP, 1);
    let pnl = a.pnl.get().max(0) as u128;
    match w.convert(&mut a) {
        Ok(got) => {
            assert!(got <= pnl);
            assert_eq!(a.capital.get(), DEP + got);
        }
        Err(e) => panic!("unexpected {e:?}"),
    }
}

#[test]
fn convert_with_huge_capital_does_not_overflow() {
    let (mut w, mut a, _b) = settled_winner(BIG, 300);
    let pnl = a.pnl.get() as u128;
    let got = w.convert(&mut a).unwrap();
    assert_eq!(got, pnl);
    assert_eq!(a.capital.get(), BIG + got);
}

/// Haircut: the loser is bankrupt, so the vault cannot cover the winner's full pnl.
#[test]
fn haircut_bankrupt_loser_caps_winner_pnl_at_loser_capital() {
    let mut w = W::new();
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, BIG);
    w.deposit(&mut b, DEP);
    w.trade(&mut a, &mut b, (10 * POS_SCALE) as i128);
    w.move_price(1_600);
    w.liquidate(&mut b).unwrap();
    w.refresh_crank(&mut a);
    let face = a.pnl.get().max(0) as u128;
    assert!(face > 0);
    // a's leg is closed by the liquidation; convert
    let got = w.convert(&mut a).unwrap();
    // price move would have paid 1.6M; the loser only had DEP, the rest is socialised away
    assert_eq!(face, DEP);
    assert!(got <= face);
    assert!(w.c_tot() + w.insurance() <= w.vault() + 1);
}

/// Conversion consumes the health cert: a second call without a refresh is refused (Stale),
/// even though there is nothing left to convert.
#[test]
fn convert_without_fresh_cert_is_refused_stale_after_a_conversion() {
    let (mut w, mut a, _b) = settled_winner(DEP, 300);
    assert!(w.convert(&mut a).unwrap() > 0);
    assert_eq!(w.convert_raw(&mut a), Err(V16Error::Stale));
    assert_eq!(w.convert(&mut a), Ok(0), "after a refresh it is a clean zero");
}
