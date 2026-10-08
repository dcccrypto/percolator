//! S10 per-instruction cap, three assets: a maker and a trader hold a leg in each of three assets,
//! each asset goes through the same stranded-backing reversal, and the maker's recovery settle
//! makes all three eligible to move in ONE instruction. Pins: the budget goes to the first two
//! assets in plan order (ascending source domain, i.e. asset index), the third is skipped without
//! failing the settle, the next instruction's settle retries it, and a move skipped inside a
//! closing batch is retried by the asset's next accrual once no position is stored.
#![allow(dead_code, unused_imports, unused_mut)]
use percolator::{
    PermissionlessCrankActionV16, PermissionlessCrankRequestV16, EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16, ProvenanceHeaderV16Account,
    TradeRequestV16, V16Config, V16PodU64,
};
use percolator::POS_SCALE;

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

struct W3 {
    header: MarketGroupV16HeaderAccount,
    markets: Vec<Market<u64>>,
    slot: u64,
}

impl W3 {
    fn new() -> Self {
        let mut cfg = V16Config::public_user_fund_with_market_slots(3, 3, 0, 6_480_000);
        cfg.max_abs_funding_e9_per_slot = 10_000;
        cfg.max_price_move_bps_per_slot = 200;
        cfg.initial_margin_bps = 1_000;
        cfg.maintenance_margin_bps = 500;
        cfg.max_accrual_dt_slots = 2;
        cfg.min_funding_lifetime_slots = 10_000_000;
        let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 3, 0).unwrap();
        let mut markets: Vec<Market<u64>> = (0..3u64).map(|i| Market::new(i, EngineAssetSlotV16Account::default())).collect();
        for i in 0..3 {
            header.activate_empty_asset_slot_not_atomic(i as u32, &mut markets[i].engine, PRICE, 1 + 2 * i as u64).unwrap();
        }
        W3 { header, markets, slot: 8 }
    }
    /// default view: NO S10 budget (what a trade, batch or liquidation instruction has)
    fn view(&mut self) -> MarketGroupV16ViewMut<'_, u64> {
        MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets)
    }
    /// refresh-crank view (the budget is passed explicitly where a refresh runs; a crank grants itself one)
    fn view_crank(&mut self) -> MarketGroupV16ViewMut<'_, u64> {
        MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets)
    }
    fn price(&self, i: usize) -> u64 { self.markets[i].engine.asset.effective_price.get() }
    fn deposit(&mut self, a: &mut PortfolioAccountV16Account, amt: u128) {
        let mut m = self.view();
        m.deposit_not_atomic(&mut PortfolioV16ViewMut::new(a), amt).unwrap();
    }
    fn trade(m: &mut MarketGroupV16ViewMut<'_, u64>, i: usize, price: u64, long: &mut PortfolioAccountV16Account, short: &mut PortfolioAccountV16Account, size: u128) {
        m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut PortfolioV16ViewMut::new(long),
            &mut PortfolioV16ViewMut::new(short),
            TradeRequestV16 { asset_index: i, size_q: i128::try_from(size).unwrap(), exec_price: price, fee_bps: 0 },
            true,
        )
        .expect("trade");
    }
    /// every asset moves `bps` this tick
    fn tick(&mut self, bps: i64) {
        self.slot += 1;
        for i in 0..3 {
            let old = self.price(i) as i128;
            let new = (old + old * bps as i128 / 10_000) as u64;
            let slot = self.slot;
            let mut m = self.view();
            m.accrue_asset_to_not_atomic(i, slot, new, 0, true).expect("accrue");
            m.markets[i].engine.asset.raw_oracle_target_price = V16PodU64::new(new);
        }
    }
    fn refresh(&mut self, a: &mut PortfolioAccountV16Account) {
        let mut m = self.view_crank();
        let mut budget = percolator::S10_MAX_MOVES_PER_INSTRUCTION;
        m.full_account_refresh_with_s10_budget_not_atomic(&mut PortfolioV16ViewMut::new(a), &mut budget).expect("refresh");
    }
    /// short-domain (maker's loss domain) fresh backing of asset i, in atoms
    fn short_fresh(&self, i: usize) -> u128 {
        self.markets[i].engine.backing_short.try_to_runtime().unwrap().fresh_unliened_backing_num / 1_000_000_000_000
    }
    fn long_fresh(&self, i: usize) -> u128 {
        self.markets[i].engine.backing_long.try_to_runtime().unwrap().fresh_unliened_backing_num / 1_000_000_000_000
    }
}

/// The state just before the maker's recovery settle: the maker settled alone at the peak (three
/// short-domain bookings), the price fell below entry, the trader settled (net loss, no claim).
fn stranded() -> (W3, PortfolioAccountV16Account, PortfolioAccountV16Account) {
    let mut w = W3::new();
    let mut a = account(300);
    let mut maker = account(0);
    w.deposit(&mut a, 1_000_000_000_000_000);
    w.deposit(&mut maker, 1_000_000_000_000_000);
    {
        let mut m = w.view();
        for i in 0..3 { W3::trade(&mut m, i, PRICE, &mut a, &mut maker, 400 * POS_SCALE); }
    }
    for _ in 0..3 { w.tick(150); }
    w.refresh(&mut maker);
    for _ in 0..8 { w.tick(-150); }
    w.refresh(&mut a);
    (w, maker, a)
}

#[test]
fn three_eligible_assets_two_fire_in_plan_order_and_the_third_waits_for_the_next_instruction() {
    let (mut w, mut maker, mut a) = stranded();
    let before: Vec<u128> = (0..3).map(|i| w.short_fresh(i)).collect();
    assert!(before.iter().all(|x| *x > 0), "every asset holds the maker's stranded loss: {before:?}");
    // one instruction (one view): the maker's three leg entries all pass V1; the budget is 2
    w.refresh(&mut maker);
    let after: Vec<u128> = (0..3).map(|i| w.short_fresh(i)).collect();
    assert!(after[0] < before[0] && after[1] < before[1], "assets 0 and 1 (lowest source domains) moved: {before:?} -> {after:?}");
    assert_eq!(after[2], before[2], "asset 2 was skipped for the cap, and the settle still succeeded");
    // the next instruction (a new view, budget restored): any settlement entry of asset 2 retries it
    w.refresh(&mut a);
    assert!(w.short_fresh(2) < before[2], "the trader's refresh retried asset 2");
    assert_eq!((w.short_fresh(0), w.short_fresh(1)), (after[0], after[1]), "nothing else moved");
}

#[test]
fn a_closing_batch_moves_nothing_and_each_assets_next_refresh_crank_retries_it() {
    let (mut w, mut maker, mut a) = stranded();
    let before: Vec<u128> = (0..3).map(|i| w.short_fresh(i)).collect();
    // ONE view = one instruction: close all three legs (each trade settles both parties first).
    // Trades and liquidations carry no S10 budget (they are the heaviest CU paths).
    {
        let mut m = w.view();
        let p: Vec<u64> = (0..3).map(|i| m.markets[i].engine.asset.effective_price.get()).collect();
        for i in 0..3 { W3::trade(&mut m, i, p[i], &mut maker, &mut a, 400 * POS_SCALE); }
    }
    for i in 0..3 {
        let asset = w.markets[i].engine.asset.try_to_runtime().unwrap();
        assert_eq!(asset.stored_pos_count_long + asset.stored_pos_count_short, 0, "asset {i}: closed");
    }
    let closed: Vec<u128> = (0..3).map(|i| w.short_fresh(i)).collect();
    assert_eq!(closed, before, "a closing batch moves nothing, whatever the eligibility");
    // no position is left to settle; each asset's next refresh crank (its own instruction, its own
    // budget) accrues it and retries the move
    w.slot += 1;
    for i in 0..3 {
        let price = w.price(i);
        let now = w.slot;
        let mut m = w.view_crank();
        m.permissionless_crank_not_atomic(
            &mut PortfolioV16ViewMut::new(&mut maker),
            PermissionlessCrankRequestV16 { now_slot: now, asset_index: i, effective_price: price, funding_rate_e9: 0, action: PermissionlessCrankActionV16::Refresh },
        )
        .expect("crank");
    }
    let after: Vec<u128> = (0..3).map(|i| w.short_fresh(i)).collect();
    assert!(after.iter().zip(&before).all(|(a, b)| a < b), "every asset's refresh crank retried and moved: {before:?} -> {after:?}");
}
