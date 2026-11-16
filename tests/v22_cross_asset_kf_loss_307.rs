//! dcccrypto/percolator#307 (Nacodex f-4016, upstream aeyakovenko/percolator-prog#457, cross-domain
//! case). A solvent account X is long on two assets. Its asset-1 gain sits as a claim in the
//! asset-1 short-loser domain whose credit rate drops below 1 while the losers have not booked.
//! A refresh then settles its asset-0 K/F loss. The loss must be paid from X's free capital and the
//! partially backed face kept (upstream `settle_kf_loss_before_support_not_atomic`, 7d2e243d..d5485eab,
//! 891653a6): burning face at the stored partial rate destroys value that becomes realizable once the
//! domain's losers book. X's equity change must equal its exact mark-to-market change.
#![allow(dead_code, unused_imports)]
use percolator::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16, ProvenanceHeaderV16Account,
    TradeRequestV16, V16Config, V16PodU64, POS_SCALE,
};

const P0: u64 = 1_000_000;
const UNITS: i128 = 1_000;

fn account(seed: u8) -> PortfolioAccountV16Account {
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        [1; 32], [seed; 32], [3; 32],
    ));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(header).unwrap();
    a
}

struct World {
    header: MarketGroupV16HeaderAccount,
    markets: Vec<Market<u64>>,
    slot: u64,
}

impl World {
    fn new() -> Self {
        let mut cfg = V16Config::public_user_fund_with_market_slots(2, 2, 0, 6_480_000);
        cfg.max_abs_funding_e9_per_slot = 10_000;
        cfg.max_price_move_bps_per_slot = 200;
        cfg.initial_margin_bps = 1_000;
        cfg.maintenance_margin_bps = 500;
        cfg.max_accrual_dt_slots = 2;
        cfg.min_funding_lifetime_slots = 10_000_000;
        let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 2, 0).unwrap();
        let mut markets: Vec<Market<u64>> =
            (0..2u64).map(|i| Market::new(i, EngineAssetSlotV16Account::default())).collect();
        for i in 0..2usize {
            header
                .activate_empty_asset_slot_not_atomic(i as u32, &mut markets[i].engine, P0, 1 + 2 * i as u64)
                .unwrap();
        }
        World { header, markets, slot: 3 }
    }
    fn view(&mut self) -> MarketGroupV16ViewMut<'_, u64> {
        MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets)
    }
    fn price(&self, i: usize) -> u64 {
        self.markets[i].engine.asset.effective_price.get()
    }
    fn deposit(&mut self, a: &mut PortfolioAccountV16Account, amt: u128) {
        self.view().deposit_not_atomic(&mut PortfolioV16ViewMut::new(a), amt).unwrap();
    }
    fn trade(&mut self, i: usize, long: &mut PortfolioAccountV16Account, short: &mut PortfolioAccountV16Account) {
        let p = self.price(i);
        self.view()
            .execute_trade_with_fee_loss_stale_scoped_not_atomic(
                &mut PortfolioV16ViewMut::new(long),
                &mut PortfolioV16ViewMut::new(short),
                TradeRequestV16 { asset_index: i, size_q: UNITS * POS_SCALE as i128, exec_price: p, fee_bps: 0 },
                true,
            )
            .expect("trade");
    }
    /// One slot: asset 0 moves `d0` bps, asset 1 moves `d1` bps.
    fn step(&mut self, d0: i64, d1: i64) {
        self.slot += 1;
        let slot = self.slot;
        for (j, d) in [(0usize, d0), (1usize, d1)] {
            let old = self.price(j) as i128;
            let new = (old + old * d as i128 / 10_000).max(1) as u64;
            let mut m = self.view();
            m.accrue_asset_to_not_atomic(j, slot, new, 0, true).expect("accrue");
            m.markets[j].engine.asset.raw_oracle_target_price = V16PodU64::new(new);
        }
    }
    fn refresh(&mut self, a: &mut PortfolioAccountV16Account) {
        let mut budget = percolator::S10_MAX_MOVES_PER_INSTRUCTION;
        self.view()
            .full_account_refresh_with_s10_budget_not_atomic(&mut PortfolioV16ViewMut::new(a), &mut budget)
            .expect("refresh");
    }
}

fn equity(a: &PortfolioAccountV16Account) -> i128 {
    a.capital.get() as i128 + a.pnl.get()
}

#[test]
fn i660_4016_two_asset_solvent_partial_rate() {
    let mut w = World::new();
    let (mut maker, mut x, mut win) = (account(10), account(11), account(12));
    w.deposit(&mut maker, 1_000_000_000_000_000);
    w.deposit(&mut x, 1_000_000_000);
    w.deposit(&mut win, 1_000_000_000);
    w.trade(0, &mut x, &mut maker);
    w.trade(1, &mut x, &mut maker);
    w.trade(1, &mut win, &mut maker);
    w.step(0, 150);
    w.refresh(&mut maker);
    w.refresh(&mut win);
    w.refresh(&mut x);
    let (p0a, p1a) = (w.price(0) as i128, w.price(1) as i128);
    let e_a = equity(&x);
    for _ in 0..18 {
        w.step(-200, 10);
        w.refresh(&mut maker);
        w.refresh(&mut win);
    }
    let sc = w.markets[1].engine.source_credit_short.try_to_runtime().unwrap();
    eprintln!("pre-X: rate_num {} (scale {})", sc.credit_rate_num, sc.credit_rate_num.max(1));
    w.refresh(&mut x);
    let (p0b, p1b) = (w.price(0) as i128, w.price(1) as i128);
    let ideal = UNITS * (p0b - p0a) + UNITS * (p1b - p1a);
    let e_b = equity(&x);
    let excess = (e_b - e_a) - ideal;
    eprintln!("X cap {} pnl {} equity change {} exact {} excess {}", x.capital.get(), x.pnl.get(), e_b - e_a, ideal, excess);
    assert!(x.capital.get() > 0, "X must stay solvent");
    assert_eq!(excess, 0, "solvent X lost {} atoms beyond its mark-to-market change", -excess);
    let (v, c, i) = (w.header.vault.get(), w.header.c_tot.get(), w.header.insurance.get());
    assert!(v >= c + i, "conservation");
}
