//! Hedged two-asset account and keeper refresh order (v2.2 engine, public API only).
//!
//! Account `a` is long asset 0 and short asset 1 (a hedge, 100 units each, price 1_000, MM 5%).
//! Both prices fall 10%: `a` loses 10_000 on asset 0 and gains 10_000 on asset 1. The loss is
//! paid from capital at once; the gain is positive PnL that only counts as equity once the
//! losing side of asset 1 (counterparty `b`) has refreshed and backed it (spec.md:266,
//! src/v16.rs `account_haircut_equity`). So until `b` is refreshed, `a`'s equity is
//! `C - 10_000` instead of `C`, and maintenance is 9_000.
//!
//! Tests here measure that window (margin sweep, both crank orders, trader loss, repeats,
//! conservation). The single-point case "gain counts only after the counterparty refreshed" is
//! in multi_asset_portfolio_cross_edge.rs (px 900 only; this file sweeps price and margin). Test names start with
//! `hedged_refresh_` so `cargo test -- hedged_refresh` selects the area.
#![allow(dead_code)]

use percolator::{
    EngineAssetSlotV16Account, LiquidationOutcomeV16, LiquidationRequestV16, Market,
    MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, PermissionlessCrankActionV16,
    PermissionlessCrankRequestV16, PortfolioAccountV16Account, PortfolioV16ViewMut,
    ProvenanceHeaderV16, ProvenanceHeaderV16Account, TradeRequestV16, V16Config, V16Error,
    V16PodU64, POS_SCALE,
};

const P0: u64 = 1_000;
const Q: u128 = 100 * POS_SCALE;
const BIG: u128 = 1_000_000_000;

fn account(seed: u8) -> PortfolioAccountV16Account {
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        [1; 32], [seed; 32], [3; 32],
    ));
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
    fn with(liq_fee_bps: u64) -> Self {
        let n = 2usize;
        let mut cfg = V16Config::public_user_fund_with_market_slots(n as u16, n as u32, 0, 10);
        cfg.maintenance_margin_bps = 500;
        cfg.initial_margin_bps = 1_000;
        cfg.max_price_move_bps_per_slot = 100;
        cfg.liquidation_fee_bps = liq_fee_bps;
        cfg.liquidation_fee_cap = 1_000_000;
        cfg.min_nonzero_mm_req = 1;
        cfg.min_nonzero_im_req = 2;
        let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, n as u32, 0).unwrap();
        let mut markets: Vec<Market<u64>> =
            (0..n as u64).map(|i| Market::new(i, EngineAssetSlotV16Account::default())).collect();
        for i in 0..n {
            header
                .activate_empty_asset_slot_not_atomic(i as u32, &mut markets[i].engine, P0, 1 + 2 * i as u64)
                .unwrap();
        }
        W { header, markets, slot: 2 * n as u64 - 1, n }
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
    fn trade(&mut self, i: usize, long: &mut PortfolioAccountV16Account, short: &mut PortfolioAccountV16Account, size: i128) {
        let p = self.price(i);
        self.view()
            .execute_trade_with_fee_loss_stale_scoped_not_atomic(
                &mut PortfolioV16ViewMut::new(long),
                &mut PortfolioV16ViewMut::new(short),
                TradeRequestV16 { asset_index: i, size_q: size, exec_price: p, fee_bps: 0 },
                true,
            )
            .expect("trade");
    }
    fn tick(&mut self, changes: &[(usize, u64)]) {
        self.slot += 1;
        let slot = self.slot;
        for j in 0..self.n {
            let new = changes.iter().find(|c| c.0 == j).map(|c| c.1).unwrap_or(self.price(j));
            let mut m = self.view();
            m.accrue_asset_to_not_atomic(j, slot, new, 0, true).expect("accrue");
            m.markets[j].engine.asset.raw_oracle_target_price = V16PodU64::new(new);
        }
    }
    /// Both assets walk to `target`, capped step per slot.
    fn move_both(&mut self, target: u64) {
        while self.price(0) != target || self.price(1) != target {
            let mut step = Vec::new();
            for i in 0..2 {
                let p = self.price(i);
                let s = (p * 100 / 10_000).max(1);
                step.push((i, if target > p { (p + s).min(target) } else { p.saturating_sub(s).max(target) }));
            }
            self.tick(&step);
        }
    }
    fn refresh(&mut self, a: &mut PortfolioAccountV16Account) -> percolator::HealthCertV16 {
        let slot = self.slot;
        for _ in 0..64 {
            let r = self.view().full_account_refresh_not_atomic(&mut PortfolioV16ViewMut::new(a));
            match r {
                Err(V16Error::BStale) => {
                    for i in 0..self.n {
                        let price = self.price(i);
                        let _ = self.view().permissionless_crank_not_atomic(
                            &mut PortfolioV16ViewMut::new(a),
                            PermissionlessCrankRequestV16 {
                                now_slot: slot,
                                asset_index: i,
                                effective_price: price,
                                funding_rate_e9: 0,
                                action: PermissionlessCrankActionV16::Refresh,
                            },
                        );
                    }
                }
                r => return r.expect("refresh"),
            }
        }
        panic!("refresh did not converge");
    }
    fn liquidate(&mut self, a: &mut PortfolioAccountV16Account, i: usize) -> Result<LiquidationOutcomeV16, V16Error> {
        self.view()
            .liquidate_account_not_atomic(&mut PortfolioV16ViewMut::new(a), LiquidationRequestV16 { asset_index: i })
    }
    fn total_value(&self, accts: &[&PortfolioAccountV16Account]) -> i128 {
        accts.iter().map(|a| value(a)).sum::<i128>() + self.header.insurance.get() as i128
    }
    fn sound(&mut self, accts: &mut [&mut PortfolioAccountV16Account]) {
        let (v, c, i) = (self.header.vault.get(), self.header.c_tot.get(), self.header.insurance.get());
        assert!(v >= c + i, "conservation broken: V {v} < C_tot {c} + I {i}");
        let g = self.view();
        g.validate_shape().unwrap();
        for a in accts.iter_mut() {
            PortfolioV16ViewMut::new(a).validate_with_market(&g.as_view()).unwrap();
        }
    }
}

fn pos(a: &PortfolioAccountV16Account, i: usize) -> i128 {
    a.legs
        .iter()
        .find(|l| l.active != 0 && l.asset_index.get() as usize == i)
        .map(|l| l.basis_pos_q.get())
        .unwrap_or(0)
}
fn value(a: &PortfolioAccountV16Account) -> i128 {
    a.capital.get() as i128 + a.pnl.get()
}

/// Hedged `a` (long asset 0, short asset 1) with `cap`, rich counterparty `b` on the other side
/// of both legs; then both prices fall to `px`. Nobody is refreshed yet.
fn hedged(cap: u128, px: u64, fee_bps: u64) -> (W, PortfolioAccountV16Account, PortfolioAccountV16Account) {
    let mut w = W::with(fee_bps);
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, cap);
    w.deposit(&mut b, BIG);
    w.trade(0, &mut a, &mut b, Q as i128);
    w.trade(1, &mut b, &mut a, Q as i128);
    w.move_both(px);
    (w, a, b)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Outcome {
    /// certified equity of `a` after refreshing only `a`
    equity_early: i128,
    /// certified equity of `a` once `b` has refreshed too
    equity_backed: i128,
    maintenance: u128,
    liquidatable_early: bool,
    liquidatable_backed: bool,
}

fn probe(cap: u128, px: u64) -> Outcome {
    let (mut w, mut a, mut b) = hedged(cap, px, 0);
    let early = w.refresh(&mut a);
    w.refresh(&mut b);
    let backed = w.refresh(&mut a);
    Outcome {
        equity_early: early.certified_equity,
        equity_backed: backed.certified_equity,
        maintenance: early.certified_maintenance_req,
        liquidatable_early: early.certified_liq_deficit > 0,
        liquidatable_backed: backed.certified_liq_deficit > 0,
    }
}

/// Order A: `a` is refreshed (and liquidated if the engine says so) before `b`.
/// Order B: `b` refreshes first, then `a`. Returns `a`'s value, position on asset 0, result.
fn run_order(cap: u128, px: u64, fee_bps: u64, b_first: bool) -> (i128, i128, bool, W, PortfolioAccountV16Account, PortfolioAccountV16Account) {
    let (mut w, mut a, mut b) = hedged(cap, px, fee_bps);
    if b_first {
        w.refresh(&mut b);
    }
    let cert = w.refresh(&mut a);
    let mut liq = false;
    if cert.certified_liq_deficit > 0 {
        liq = w.liquidate(&mut a, 0).is_ok() || w.liquidate(&mut a, 1).is_ok();
    }
    if !b_first {
        w.refresh(&mut b);
    }
    (value(&a), pos(&a, 0), liq, w, a, b)
}

/// Smallest capital the engine lets this hedge open with: 10% initial margin on each of the two
/// 100_000 notional legs. A thinner hedge cannot be opened, so this is the thinnest margin.
const THIN: u128 = 20_000;

/// Highest price at which `a`'s early equity is strictly below maintenance:
/// `cap - 100 * (1000 - px) < 10 * px`  <=>  `px < (100_000 - cap) / 90`.
fn window_top(cap: u128) -> u64 {
    let n = 100_000 - cap;
    ((n + 89) / 90 - 1) as u64
}

/// Lowest price of the window: the loss equals the capital. Below it `a` is liquidatable in either
/// order (equity 0 even after backing), so order no longer decides.
fn window_bottom(cap: u128) -> u64 {
    1_000 - (cap / 100) as u64
}

// ---------------------------------------------------------------------------------------------
// Measurement: how wide is the window
// ---------------------------------------------------------------------------------------------

#[test]
fn hedged_refresh_no_price_move_no_window() {
    let o = probe(THIN, 1_000);
    assert!(!o.liquidatable_early && !o.liquidatable_backed);
    assert_eq!(o.equity_early, THIN as i128);
    assert_eq!(o.equity_backed, THIN as i128);
}

#[test]
fn hedged_refresh_early_equity_is_capital_minus_loss_and_backed_equity_is_capital() {
    for px in [950u64, 900, 850, 800] {
        let o = probe(THIN, px);
        let loss = 100 * (1_000 - px as i128);
        assert_eq!(o.equity_early, THIN as i128 - loss, "px {px}");
        assert_eq!(o.equity_backed, THIN as i128, "px {px}");
        assert_eq!(o.maintenance, 10 * px as u128, "px {px}");
    }
}

#[test]
fn hedged_refresh_thinnest_margin_window_is_800_to_888() {
    // 11.2% to 20% fall of both assets: order alone decides
    let top = window_top(THIN);
    let bottom = window_bottom(THIN);
    assert_eq!((bottom, top), (800, 888));
    for px in bottom..=top {
        let o = probe(THIN, px);
        assert!(o.liquidatable_early && !o.liquidatable_backed, "px {px}: {o:?}");
    }
    // above the window both are healthy (below it, see the note on window_bottom)
    for px in [top + 1, 889, 900, 1_000] {
        let o = probe(THIN, px);
        assert!(!o.liquidatable_early && !o.liquidatable_backed, "px {px}: {o:?}");
    }
}

#[test]
fn hedged_refresh_window_shifts_with_margin_but_never_closes() {
    // more margin needs a deeper fall to enter the window, but one always exists
    for cap in [20_000u128, 30_000, 50_000] {
        let (bottom, top) = (window_bottom(cap), window_top(cap));
        assert!(bottom <= top, "cap {cap}");
        for px in [bottom, (bottom + top) / 2, top] {
            let o = probe(cap, px);
            assert!(o.liquidatable_early && !o.liquidatable_backed, "cap {cap} px {px}");
        }
        let o = probe(cap, top + 1);
        assert!(!o.liquidatable_early, "cap {cap} px {}", top + 1);
    }
}

#[test]
fn hedged_refresh_rounding_boundary_equity_exactly_at_maintenance_is_not_liquidatable() {
    // px 888: loss 11_200, MM 8_880. cap 20_080 -> early equity 8_880 exactly.
    let on = probe(20_080, 888);
    assert_eq!((on.equity_early, on.maintenance as i128), (8_880, 8_880));
    assert!(!on.liquidatable_early);
    let under = probe(20_079, 888);
    assert_eq!(under.equity_early, 8_879);
    assert!(under.liquidatable_early && !under.liquidatable_backed);
}

#[test]
fn hedged_refresh_margin_size_does_not_close_the_window_at_the_loss_cap() {
    // loss capped at capital (px at the bottom): early equity 0, still inside the window at any size
    for cap in [20_000u128, 40_000, 90_000] {
        let o = probe(cap, window_bottom(cap));
        assert_eq!(o.equity_early, 0, "cap {cap}");
        assert!(o.liquidatable_early && !o.liquidatable_backed);
    }
}

// ---------------------------------------------------------------------------------------------
// Crank order
// ---------------------------------------------------------------------------------------------

#[test]
fn hedged_refresh_counterparty_first_order_never_liquidates_in_the_window() {
    for px in 800..=888u64 {
        let (_v, p0, liq, mut w, mut a, mut b) = run_order(THIN, px, 0, true);
        assert!(!liq, "px {px}");
        assert_eq!(p0, Q as i128, "px {px}");
        assert_eq!(pos(&a, 1), -(Q as i128));
        w.sound(&mut [&mut a, &mut b]);
    }
}

/// What the engine does today when the hedged account is cranked before its counterparty:
/// it liquidates a hedge that is solvent once `b` refreshes. See the ignored test below.
#[test]
fn hedged_refresh_account_first_order_liquidates_solvent_hedge_today() {
    let mut closed = Vec::new();
    for px in [888u64, 850, 800] {
        let (v, p0, liq, mut w, mut a, mut b) = run_order(THIN, px, 0, false);
        assert!(liq, "px {px}");
        assert_eq!(v, THIN as i128, "value unchanged without fee");
        // asset-1 hedge leg is NOT closed: the account is left naked short
        assert_eq!(pos(&a, 1), -(Q as i128), "px {px}");
        closed.push(p0);
        w.sound(&mut [&mut a, &mut b]);
    }
    // measured: how much of the long leg REMAINS after the forced close; a fall to 800 closes all of it
    assert_eq!(closed, vec![98_198_198, 17_647_058, 0]);
}

#[test]
fn hedged_refresh_both_orders_hold_the_same_total_value() {
    for px in [889u64, 888, 850, 800] {
        let (_, _, _, w1, a1, b1) = run_order(THIN, px, 0, false);
        let (_, _, _, w2, a2, b2) = run_order(THIN, px, 0, true);
        assert_eq!(w1.total_value(&[&a1, &b1]), w2.total_value(&[&a2, &b2]), "px {px}");
        assert_eq!(w1.header.vault.get(), w2.header.vault.get());
    }
}

#[test]
fn hedged_refresh_repeated_refresh_after_backing_changes_nothing() {
    let (mut w, mut a, mut b) = hedged(THIN, 850, 0);
    w.refresh(&mut b);
    let first = w.refresh(&mut a);
    for _ in 0..3 {
        w.refresh(&mut b);
        let again = w.refresh(&mut a);
        assert_eq!(again, first);
    }
    assert_eq!(first.certified_equity, THIN as i128);
    assert_eq!(w.liquidate(&mut a, 0), Err(V16Error::NonProgress));
    assert_eq!(w.liquidate(&mut a, 1), Err(V16Error::NonProgress));
    w.sound(&mut [&mut a, &mut b]);
}

#[test]
fn hedged_refresh_repeated_liquidation_after_early_crank_ends_and_conserves() {
    let (mut w, mut a, mut b) = hedged(THIN, 850, 0);
    w.refresh(&mut a);
    let mut calls = 0;
    while w.liquidate(&mut a, 0).is_ok() {
        calls += 1;
        assert!(calls < 20, "liquidation must converge");
    }
    assert!(calls >= 1);
    let v = w.header.vault.get();
    // asking again changes nothing
    assert!(w.liquidate(&mut a, 0).is_err());
    assert_eq!(w.header.vault.get(), v);
    w.refresh(&mut b);
    w.refresh(&mut a);
    w.sound(&mut [&mut a, &mut b]);
}

#[test]
fn hedged_refresh_liquidating_asset_one_early_also_hits_a_solvent_hedge() {
    // the other leg is just as exposed: asset 1 can be the liquidation target too
    let (mut w, mut a, mut b) = hedged(THIN, 850, 0);
    w.refresh(&mut a);
    let out = w.liquidate(&mut a, 1).expect("liquidates through asset 1");
    assert!(out.closed_q > 0);
    assert_eq!(pos(&a, 0), Q as i128, "asset 0 leg untouched");
    w.sound(&mut [&mut a, &mut b]);
}

#[test]
fn hedged_refresh_counterparty_refresh_only_does_not_move_equity_of_a() {
    // refreshing `b` alone does not change `a`'s stored capital or pnl; only a's next refresh
    // sees the backing
    let (mut w, mut a, mut b) = hedged(THIN, 850, 0);
    let before = (a.capital.get(), a.pnl.get());
    w.refresh(&mut b);
    assert_eq!(before, (a.capital.get(), a.pnl.get()));
    w.sound(&mut [&mut a, &mut b]);
}

// ---------------------------------------------------------------------------------------------
// Real bug (kept failing)
// ---------------------------------------------------------------------------------------------

/// BUG: a hedged account whose gain is only waiting on the counterparty's refresh is liquidated
/// when its own crank comes first. Cap 20_000, both prices 1_000 -> 850: the true equity is
/// 20_000 against a maintenance of 8_500, yet the engine force-closes most of the long leg
/// because the 15_000 gain on the short leg is not counted (src/v16.rs:14780
/// `account_haircut_equity`, used by the health check at src/v16.rs:16836 and by
/// `liquidate_account_not_atomic`; spec.md:266 says open PnL is not backing, but maintenance equity in spec.md:899 and 1213 is C + PNL - FeeDebt with the full PnL, and the existing test multi_asset_profit_on_one_leg_covers_loss_on_the_other_for_maintenance pins the unbacked behaviour, so the owners must pick one). Expected: the liquidation attempt is refused
/// (nothing to liquidate), as it is when `b` is refreshed first.
#[test]
#[ignore = "BUG: crank order alone liquidates a solvent hedged account (account_haircut_equity, src/v16.rs:14780)"]
fn hedged_refresh_solvent_hedge_must_not_be_liquidated_before_counterparty_refresh() {
    let (mut w, mut a, mut b) = hedged(THIN, 850, 0);
    w.refresh(&mut a);
    assert_eq!(w.liquidate(&mut a, 0), Err(V16Error::NonProgress));
    assert_eq!(pos(&a, 0), Q as i128);
    assert_eq!(pos(&a, 1), -(Q as i128));
    w.sound(&mut [&mut a, &mut b]);
}

/// Same bug at the smallest price move that triggers it (px 888, ~11.2% fall).
#[test]
#[ignore = "BUG: crank order alone liquidates a solvent hedged account (account_haircut_equity, src/v16.rs:14780)"]
fn hedged_refresh_smallest_window_point_must_not_liquidate_before_counterparty_refresh() {
    let (mut w, mut a, mut b) = hedged(THIN, 888, 0);
    let cert = w.refresh(&mut a);
    assert_eq!(cert.certified_liq_deficit, 0, "equity {} vs MM 8_880", cert.certified_equity);
    assert_eq!(w.liquidate(&mut a, 0), Err(V16Error::NonProgress));
    w.sound(&mut [&mut a, &mut b]);
}
