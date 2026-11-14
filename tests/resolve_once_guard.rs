//! Engine-level guard: `resolve_market_not_atomic` must not run on an already Resolved market.
//! The v2.2 wrapper already blocks a second resolve; this covers the engine's own check.
//! Recovery -> Resolved stays allowed (the auto-crank finalizes Recovery through it).
#![allow(dead_code, unused_imports)]

use percolator::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    V16Config, V16Error,
};

const PRICE: u64 = 1_000_000;

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
        header
            .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, PRICE, 1)
            .unwrap();
        W { header, markets }
    }
    fn view(&mut self) -> MarketGroupV16ViewMut<'_, u64> {
        MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets)
    }
    fn resolve(&mut self, slot: u64) -> Result<(), V16Error> {
        self.view().resolve_market_not_atomic(slot)
    }
}

#[test]
fn second_resolve_is_refused_and_changes_nothing() {
    let mut w = W::new();
    let r0 = w.header.current_slot.get() + 10;
    w.resolve(r0).unwrap();
    let before = w.header.clone();
    assert_eq!(w.resolve(r0 + 500), Err(V16Error::LockActive));
    assert_eq!(w.header, before);
    assert_eq!(w.header.resolved_slot.get(), r0);
}

#[test]
fn live_resolve_still_works() {
    let mut w = W::new();
    let r0 = w.header.current_slot.get() + 3;
    w.resolve(r0).unwrap();
    assert_eq!(w.header.resolved_slot.get(), r0);
}

#[test]
fn recovery_market_can_still_resolve() {
    let mut w = W::new();
    let s = w.header.current_slot.get();
    w.view().force_asset_recovery_not_atomic(0, s).unwrap();
    w.resolve(s + 1).unwrap();
    assert_eq!(w.header.resolved_slot.get(), s + 1);
}
