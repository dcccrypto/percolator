//! Cross-asset edge tests for one portfolio holding several assets at once (v2.2 engine,
//! public API only): a loss on one asset and what it does to the other legs, the maintenance
//! requirement shared across assets, liquidating one asset while others stay open, closing one
//! asset while others stay open, close / liquidation order, repeated calls, and conservation.
//!
//! Single-asset liquidation and the IM cross-margin open/increase cases already live in
//! liquidations_edge.rs and margin_edge.rs and are not repeated here. Every test name starts
//! with `multi_asset_` so `cargo test -- multi_asset` selects the area.
//!
//! Setup: every asset starts at price 1_000; `Q` = 100 units (notional 100_000 per leg).
//! MM 5%, IM 10%, no trading or liquidation fee unless a test says otherwise. The wrapper-owned
//! raw oracle target is written onto the market slot next to each accrual, as the other edge
//! files do.
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
    move_bps: u64,
}

impl W {
    fn with(n: usize, mm_bps: u64, im_bps: u64, liq_fee_bps: u64) -> Self {
        let mut cfg = V16Config::public_user_fund_with_market_slots(n as u16, n as u32, 0, 10);
        cfg.maintenance_margin_bps = mm_bps;
        cfg.initial_margin_bps = im_bps;
        cfg.max_price_move_bps_per_slot = 200;
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
        W { header, markets, slot: 2 * n as u64 - 1, n, move_bps: 200 }
    }
    fn new(n: usize) -> Self {
        W::with(n, 500, 1_000, 0)
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
    /// `long` gets +size on asset i at the current mark.
    fn try_trade(
        &mut self,
        i: usize,
        long: &mut PortfolioAccountV16Account,
        short: &mut PortfolioAccountV16Account,
        size: i128,
    ) -> Result<(), V16Error> {
        let p = self.price(i);
        self.view()
            .execute_trade_with_fee_loss_stale_scoped_not_atomic(
                &mut PortfolioV16ViewMut::new(long),
                &mut PortfolioV16ViewMut::new(short),
                TradeRequestV16 { asset_index: i, size_q: size, exec_price: p, fee_bps: 0 },
                true,
            )
            .map(|_| ())
    }
    fn trade(&mut self, i: usize, long: &mut PortfolioAccountV16Account, short: &mut PortfolioAccountV16Account, size: i128) {
        self.try_trade(i, long, short, size).expect("trade");
    }
    /// Walk asset i's price to `target` (others re-accrued at their own price each slot so
    /// nothing goes stale), one capped step per slot.
    fn move_to(&mut self, i: usize, target: u64) {
        let mut price = self.price(i);
        while price != target {
            let step = (price * self.move_bps / 10_000).max(1);
            price = if target > price { (price + step).min(target) } else { price.saturating_sub(step).max(target) };
            self.tick(&[(i, price)]);
        }
    }
    /// Move several assets together, step by step, to their targets.
    fn move_many(&mut self, targets: &[(usize, u64)]) {
        loop {
            let mut step: Vec<(usize, u64)> = Vec::new();
            for &(i, t) in targets {
                let p = self.price(i);
                if p != t {
                    let s = (p * self.move_bps / 10_000).max(1);
                    step.push((i, if t > p { (p + s).min(t) } else { p.saturating_sub(s).max(t) }));
                }
            }
            if step.is_empty() {
                return;
            }
            self.tick(&step);
        }
    }
    /// One slot forward: assets in `changes` get their new price, the rest keep theirs.
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
    /// Full refresh; after a B booking the plain refresh is BStale until a keeper runs the
    /// permissionless Refresh crank, so drive it to convergence.
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
    fn insurance_into(&mut self, domain: usize, amt: u128) {
        self.view().deposit_domain_insurance_not_atomic(domain, amt).unwrap();
    }
    /// spec.md 10 item 1 (V >= C_tot + I) plus the engine's own shape / account checks.
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

/// signed basis position on asset i (0 if no leg)
fn pos(a: &PortfolioAccountV16Account, i: usize) -> i128 {
    a.legs
        .iter()
        .find(|l| l.active != 0 && l.asset_index.get() as usize == i)
        .map(|l| l.basis_pos_q.get())
        .unwrap_or(0)
}
fn n_legs(a: &PortfolioAccountV16Account) -> usize {
    a.legs.iter().filter(|l| l.active != 0).count()
}
fn bitmap_ones(a: &PortfolioAccountV16Account) -> u32 {
    a.active_bitmap.iter().map(|w| w.get().count_ones()).sum()
}
fn value(a: &PortfolioAccountV16Account) -> i128 {
    a.capital.get() as i128 + a.pnl.get()
}
/// MM of one Q leg at price p with 5%: notional 100*p, MM = ceil(100*p*500/10_000) = 5p.
fn mm_q(p: u64) -> u128 {
    5 * p as u128
}

/// Target long Q on assets 0 and 1 with `cap` capital against a rich counterparty.
fn two_long(cap: u128) -> (W, PortfolioAccountV16Account, PortfolioAccountV16Account) {
    let mut w = W::new(2);
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, cap);
    w.deposit(&mut b, BIG);
    w.trade(0, &mut a, &mut b, Q as i128);
    w.trade(1, &mut a, &mut b, Q as i128);
    (w, a, b)
}

// ---------------------------------------------------------------------------------------------
// Shared maintenance requirement
// ---------------------------------------------------------------------------------------------

/// Asset 0 falls to 890, asset 1 stays at 1_000. Equity = C - 100*110 = C - 11_000;
/// MM = 5*890 + 5*1_000 = 9_450. So C = 20_450 sits exactly on the boundary.
fn boundary_world(cap: u128) -> (W, PortfolioAccountV16Account, PortfolioAccountV16Account) {
    let (mut w, mut a, mut b) = two_long(cap);
    w.move_to(0, 890);
    w.refresh(&mut a);
    w.refresh(&mut b);
    (w, a, b)
}

#[test]
fn multi_asset_maintenance_requirement_is_the_sum_of_per_leg_requirements() {
    let (mut w, mut a, _b) = boundary_world(20_450);
    let cert = w.refresh(&mut a);
    assert_eq!(cert.certified_maintenance_req, mm_q(890) + mm_q(1_000));
    assert_eq!(cert.certified_equity, 20_450 - 11_000);
    assert_eq!(cert.certified_liq_deficit, 0);
}

#[test]
fn multi_asset_equity_exactly_at_shared_maintenance_is_not_liquidatable_on_either_asset() {
    for i in 0..2 {
        let (mut w, mut a, _b) = boundary_world(20_450);
        let before = (a.capital.get(), a.pnl.get(), pos(&a, 0), pos(&a, 1));
        assert_eq!(w.liquidate(&mut a, i), Err(V16Error::NonProgress), "asset {i}");
        assert_eq!(before, (a.capital.get(), a.pnl.get(), pos(&a, 0), pos(&a, 1)));
    }
}

#[test]
fn multi_asset_one_atom_under_shared_maintenance_is_liquidatable_through_the_flat_asset() {
    // the loss is all on asset 0, asset 1's price never moved: asset 1 is still a valid target
    let (mut w, mut a, mut b) = boundary_world(20_449);
    let cert = w.refresh(&mut a);
    assert_eq!(cert.certified_liq_deficit, 1);
    let out = w.liquidate(&mut a, 1).expect("liquidate the untouched asset");
    assert!(out.closed_q > 0 && out.closed_q <= Q);
    assert_eq!(pos(&a, 0), Q as i128, "asset 0 leg untouched by asset 1's liquidation");
    assert_eq!(pos(&a, 1), (Q - out.closed_q) as i128);
    w.sound(&mut [&mut a, &mut b]);
}

#[test]
fn multi_asset_profit_on_one_leg_covers_loss_on_the_other_for_maintenance() {
    // long asset 0, short asset 1, both fall 10%: -10_000 on 0, +10_000 on 1. Capital 20_000.
    // The loss settles from principal at once; the gain is credited as health only once the
    // losing side on asset 1 (b) has settled and backed it (spec.md 266: open positive PnL is not backing; equity credits a gain only up to its
    // source-domain support, src/v16.rs account_haircut_equity).
    let mut w = W::new(2);
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, 20_000);
    w.deposit(&mut b, BIG);
    w.trade(0, &mut a, &mut b, Q as i128);
    w.trade(1, &mut b, &mut a, Q as i128);
    w.move_many(&[(0, 900), (1, 900)]);
    let early = w.refresh(&mut a);
    assert_eq!((a.capital.get(), a.pnl.get()), (10_000, 10_000));
    assert_eq!(early.certified_equity, 10_000, "gain not backed yet");
    w.refresh(&mut b);
    let cert = w.refresh(&mut a);
    assert_eq!(cert.certified_equity, 20_000, "backed gain now offsets the loss");
    assert_eq!(cert.certified_maintenance_req, mm_q(900) * 2);
    assert_eq!(w.liquidate(&mut a, 0), Err(V16Error::NonProgress));
    assert_eq!(w.liquidate(&mut a, 1), Err(V16Error::NonProgress));
    // repeating the refreshes in either order changes nothing now
    w.refresh(&mut b);
    assert_eq!(w.refresh(&mut a).certified_equity, 20_000);
    w.sound(&mut [&mut a, &mut b]);
}

#[test]
fn multi_asset_loss_on_one_asset_alone_tips_a_two_leg_account() {
    // same capital as the boundary, loss on asset 1 instead of 0: symmetric outcome
    for (moved, other) in [(0usize, 1usize), (1, 0)] {
        let (mut w, mut a, _b) = two_long(20_449);
        w.move_to(moved, 890);
        let cert = w.refresh(&mut a);
        assert_eq!(cert.certified_liq_deficit, 1, "moved {moved}");
        assert!(w.liquidate(&mut a, other).is_ok(), "moved {moved}: other asset is a valid target");
    }
}

// ---------------------------------------------------------------------------------------------
// Liquidation on one asset, other legs open
// ---------------------------------------------------------------------------------------------

#[test]
fn multi_asset_liquidation_request_for_an_asset_the_account_does_not_hold_is_refused() {
    let mut w = W::new(2);
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, 10_000);
    w.deposit(&mut b, BIG);
    w.trade(0, &mut a, &mut b, Q as i128);
    w.move_to(0, 920);
    let cert = w.refresh(&mut a);
    assert!(cert.certified_liq_deficit > 0, "underwater on asset 0");
    let before = (a.capital.get(), a.pnl.get(), pos(&a, 0), w.header.insurance.get());
    assert_eq!(w.liquidate(&mut a, 1), Err(V16Error::InvalidLeg), "no leg on asset 1");
    assert_eq!(before, (a.capital.get(), a.pnl.get(), pos(&a, 0), w.header.insurance.get()));
    assert!(w.liquidate(&mut a, 0).is_ok(), "the held asset still is");
}

#[test]
fn multi_asset_liquidation_of_one_asset_keeps_the_other_leg_bitmap_and_counts() {
    let (mut w, mut a, mut b) = two_long(20_000);
    w.move_to(0, 850);
    w.refresh(&mut a);
    w.refresh(&mut b);
    let leg1 = pos(&a, 1);
    let mut guard = 0;
    while w.liquidate(&mut a, 0).is_ok() {
        guard += 1;
        assert!(guard < 32);
        if pos(&a, 0) == 0 {
            break;
        }
    }
    assert_eq!(pos(&a, 1), leg1, "asset 1 leg size unchanged");
    assert_eq!(n_legs(&a) as u32, bitmap_ones(&a), "bitmap agrees with active legs");
    assert!(n_legs(&a) >= 1);
    w.sound(&mut [&mut a, &mut b]);
}

#[test]
fn multi_asset_liquidation_is_repeatable_until_healthy_or_flat_and_then_stops() {
    let (mut w, mut a, mut b) = two_long(20_000);
    w.move_many(&[(0, 800), (1, 800)]);
    w.refresh(&mut b);
    let mut calls = 0;
    for _ in 0..64 {
        let r0 = w.liquidate(&mut a, 0);
        let r1 = w.liquidate(&mut a, 1);
        calls += r0.is_ok() as u32 + r1.is_ok() as u32;
        if r0.is_err() && r1.is_err() {
            break;
        }
    }
    assert!(calls > 0);
    let cert = w.refresh(&mut a);
    assert!(cert.certified_liq_deficit == 0 || n_legs(&a) == 0, "stops only when healthy or flat: {cert:?}");
    // once both refuse, another round still refuses and changes nothing
    let snap = (a.capital.get(), a.pnl.get(), pos(&a, 0), pos(&a, 1), w.header.insurance.get(), w.header.vault.get());
    assert!(w.liquidate(&mut a, 0).is_err() && w.liquidate(&mut a, 1).is_err());
    assert_eq!(snap, (a.capital.get(), a.pnl.get(), pos(&a, 0), pos(&a, 1), w.header.insurance.get(), w.header.vault.get()));
    w.sound(&mut [&mut a, &mut b]);
}

/// Drive a deep-underwater two-leg account flat by liquidating in the given asset order.
/// Returns (target value, counterparty value after refresh, insurance, vault, c_tot).
fn bankrupt_flat_in_order(order: [usize; 2], ins: u128) -> (i128, i128, u128, u128, u128) {
    let (mut w, mut a, mut b) = two_long(20_000);
    for d in 0..4 {
        w.insurance_into(d, ins);
    }
    w.move_to(1, 700); // all of the loss is on asset 1: -30_000 against 20_000 capital
    w.refresh(&mut a);
    for &i in order.iter() {
        let mut guard = 0;
        while pos(&a, i) != 0 {
            guard += 1;
            assert!(guard < 32, "asset {i} would not close");
            w.liquidate(&mut a, i).unwrap_or_else(|e| panic!("order {order:?} asset {i}: {e:?}"));
        }
    }
    assert_eq!(n_legs(&a), 0);
    assert_eq!(bitmap_ones(&a), 0);
    w.refresh(&mut b);
    w.sound(&mut [&mut a, &mut b]);
    (value(&a), value(&b), w.header.insurance.get(), w.header.vault.get(), w.header.c_tot.get())
}

#[test]
fn multi_asset_bankrupt_two_leg_account_ends_flat_with_no_negative_pnl_left() {
    for order in [[0usize, 1], [1, 0]] {
        let (tv, _, _, _, _) = bankrupt_flat_in_order(order, 0);
        assert!(tv >= 0, "order {order:?}: flat account still shows value {tv}");
    }
}

#[test]
fn multi_asset_bankrupt_close_order_does_not_change_who_pays() {
    // spec req 27: the outcome must not depend on which leg the liquidator closes first
    for ins in [0u128, 5_000, 1_000_000] {
        let a = bankrupt_flat_in_order([0, 1], ins);
        let b = bankrupt_flat_in_order([1, 0], ins);
        assert_eq!(a, b, "insurance {ins}: order 0,1 vs 1,0");
    }
}

/// Bankrupt two-leg long; insurance only in `domains`; asset prices moved to `targets`.
/// Liquidate to flat in order 0 then 1; return insurance spent.
fn bankrupt_insurance_spent(targets: &[(usize, u64)], domains: &[usize]) -> u128 {
    let (mut w, mut a, mut b) = two_long(20_000);
    for &d in domains {
        w.insurance_into(d, 1_000_000);
    }
    let ins0 = w.header.insurance.get();
    w.move_many(targets);
    w.refresh(&mut a);
    for i in 0..2 {
        let mut guard = 0;
        while pos(&a, i) != 0 {
            guard += 1;
            assert!(guard < 32);
            w.liquidate(&mut a, i).expect("liquidate");
        }
    }
    w.refresh(&mut b);
    w.sound(&mut [&mut a, &mut b]);
    ins0 - w.header.insurance.get()
}

#[test]
fn multi_asset_loss_on_asset_one_never_spends_asset_zero_insurance() {
    // a long's loss on asset k is the domain k*2+1 (winners' short side). All loss is on
    // asset 1; only asset 0's two domains hold insurance: none of it may be spent.
    assert_eq!(bankrupt_insurance_spent(&[(1, 700)], &[0, 1]), 0);
}

#[test]
fn multi_asset_loss_on_two_assets_is_not_charged_to_either_domain_insurance() {
    // both legs lose: the deficit has no single loss domain, so no domain's insurance pays it
    // (spec req 5/27: no invented attribution, no pooled-insurance fallback)
    assert_eq!(bankrupt_insurance_spent(&[(0, 820), (1, 820)], &[0, 1, 2, 3]), 0);
}

#[test]
fn multi_asset_single_domain_loss_spends_exactly_the_deficit_from_that_domain_insurance() {
    // a long loser on asset 1 is covered by domain 3 (asset 1, winners' short side); it pays
    // exactly the deficit (30_000 loss - 20_000 capital = 10_000)
    // (positive control for the two zero-spend tests above)
    assert_eq!(bankrupt_insurance_spent(&[(1, 700)], &[3]), 10_000);
}

// ---------------------------------------------------------------------------------------------
// Closing one asset while others stay open
// ---------------------------------------------------------------------------------------------

#[test]
fn multi_asset_closing_one_asset_leaves_the_other_leg_and_conserves_value() {
    let (mut w, mut a, mut b) = two_long(100_000);
    let total0 = value(&a) + value(&b);
    w.move_many(&[(0, 950), (1, 1_050)]);
    w.refresh(&mut a);
    w.refresh(&mut b);
    w.trade(0, &mut b, &mut a, Q as i128); // a closes asset 0 at a loss
    assert_eq!(pos(&a, 0), 0);
    assert_eq!(pos(&a, 1), Q as i128);
    assert_eq!(n_legs(&a), 1);
    assert_eq!(bitmap_ones(&a), 1);
    assert_eq!(value(&a) + value(&b), total0, "zero-fee close moves value, never creates it");
    // a's value is exactly capital + loss on 0 + gain on 1
    assert_eq!(value(&a), 100_000 - 5_000 + 5_000);
    w.sound(&mut [&mut a, &mut b]);
}

#[test]
fn multi_asset_close_order_of_legs_gives_the_same_final_value() {
    let run = |first: usize| {
        let (mut w, mut a, mut b) = two_long(100_000);
        w.move_many(&[(0, 930), (1, 1_070)]);
        w.refresh(&mut a);
        w.refresh(&mut b);
        w.trade(first, &mut b, &mut a, Q as i128);
        w.refresh(&mut a);
        w.refresh(&mut b);
        w.trade(1 - first, &mut b, &mut a, Q as i128);
        w.refresh(&mut a);
        w.refresh(&mut b);
        w.sound(&mut [&mut a, &mut b]);
        (value(&a), value(&b), a.capital.get(), b.capital.get(), w.header.vault.get(), w.header.c_tot.get())
    };
    assert_eq!(run(0), run(1));
}

#[test]
fn multi_asset_closing_the_losing_leg_does_not_need_initial_margin_on_the_other() {
    // equity 13_000 < IM 19_300 (but above MM) after a loss on asset 0: closing asset 0 is a
    // reduction and is allowed, and it leaves asset 1 alone
    let (mut w, mut a, mut b) = two_long(20_000);
    w.move_to(0, 930); // equity 13_000, IM 9_300 + 10_000 = 19_300, MM 9_650
    let cert = w.refresh(&mut a);
    w.refresh(&mut b);
    assert!(cert.certified_liq_deficit == 0);
    assert_eq!((cert.certified_equity, cert.certified_initial_req), (13_000, 19_300), "below IM, above MM");
    w.trade(0, &mut b, &mut a, Q as i128);
    assert_eq!(pos(&a, 0), 0);
    assert_eq!(pos(&a, 1), Q as i128);
    assert_eq!(value(&a), 13_000);
    w.sound(&mut [&mut a, &mut b]);
}

#[test]
fn multi_asset_partial_close_in_many_small_fills_equals_one_fill() {
    let run = |pieces: u128| {
        let (mut w, mut a, mut b) = two_long(100_000);
        w.move_many(&[(0, 970), (1, 1_020)]);
        w.refresh(&mut a);
        w.refresh(&mut b);
        for _ in 0..pieces {
            w.trade(0, &mut b, &mut a, (Q / 2 / pieces) as i128);
        }
        w.refresh(&mut a);
        w.refresh(&mut b);
        (value(&a), value(&b), pos(&a, 0), pos(&a, 1))
    };
    assert_eq!(run(1), run(5));
    assert_eq!(run(1).2, (Q / 2) as i128);
}

#[test]
fn multi_asset_zero_size_trade_on_second_asset_is_refused_and_first_leg_untouched() {
    let (mut w, mut a, mut b) = two_long(100_000);
    let snap = (value(&a), pos(&a, 0), pos(&a, 1), n_legs(&a));
    assert_eq!(w.try_trade(1, &mut a, &mut b, 0), Err(V16Error::InvalidConfig));
    assert_eq!(snap, (value(&a), pos(&a, 0), pos(&a, 1), n_legs(&a)));
}

#[test]
fn multi_asset_one_position_atom_legs_on_every_asset_open_and_close() {
    let n = 4;
    let mut w = W::new(n);
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, 1_000);
    w.deposit(&mut b, BIG);
    for i in 0..n {
        w.trade(i, &mut a, &mut b, 1);
    }
    assert_eq!(n_legs(&a), n);
    let cert = w.refresh(&mut a);
    // each dust leg pays the min nonzero MM (1), not zero
    assert_eq!(cert.certified_maintenance_req, n as u128);
    for i in 0..n {
        w.trade(i, &mut b, &mut a, 1);
    }
    assert_eq!(n_legs(&a), 0);
    assert_eq!(value(&a), 1_000);
    w.sound(&mut [&mut a, &mut b]);
}

// ---------------------------------------------------------------------------------------------
// Max: every portfolio slot in use
// ---------------------------------------------------------------------------------------------

#[test]
fn multi_asset_all_sixteen_assets_open_close_middle_and_reopen() {
    let n = percolator::V16_MAX_PORTFOLIO_ASSETS_N;
    let mut w = W::new(n);
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, BIG);
    w.deposit(&mut b, BIG);
    for i in 0..n {
        w.trade(i, &mut a, &mut b, ((i as u128 + 1) * POS_SCALE) as i128);
    }
    assert_eq!(n_legs(&a), n);
    assert_eq!(bitmap_ones(&a), n as u32);
    let total0 = value(&a) + value(&b);
    w.trade(7, &mut b, &mut a, (8 * POS_SCALE) as i128);
    assert_eq!(n_legs(&a), n - 1);
    assert_eq!(bitmap_ones(&a), (n - 1) as u32);
    for i in 0..n {
        if i != 7 {
            assert_eq!(pos(&a, i), ((i as u128 + 1) * POS_SCALE) as i128, "asset {i}");
        }
    }
    w.trade(7, &mut b, &mut a, (3 * POS_SCALE) as i128); // reopen on the other side
    assert_eq!(pos(&a, 7), -((3 * POS_SCALE) as i128));
    assert_eq!(n_legs(&a), n);
    assert_eq!(value(&a) + value(&b), total0);
    w.sound(&mut [&mut a, &mut b]);
}

#[test]
fn multi_asset_all_sixteen_assets_move_and_value_is_conserved() {
    let n = percolator::V16_MAX_PORTFOLIO_ASSETS_N;
    let mut w = W::new(n);
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, BIG);
    w.deposit(&mut b, BIG);
    for i in 0..n {
        let s = if i % 2 == 0 { Q as i128 } else { -(Q as i128) };
        w.trade(i, &mut a, &mut b, s);
    }
    let total0 = value(&a) + value(&b);
    let targets: Vec<(usize, u64)> = (0..n).map(|i| (i, 900 + 13 * i as u64)).collect();
    w.move_many(&targets);
    w.refresh(&mut a);
    w.refresh(&mut b);
    // a: long even assets, short odd ones
    let expect: i128 = (0..n)
        .map(|i| {
            let d = (900 + 13 * i as i128) - 1_000;
            if i % 2 == 0 { 100 * d } else { -100 * d }
        })
        .sum();
    assert_eq!(value(&a), BIG as i128 + expect);
    assert_eq!(value(&a) + value(&b), total0);
    w.sound(&mut [&mut a, &mut b]);
}

// ---------------------------------------------------------------------------------------------
// Repeated calls
// ---------------------------------------------------------------------------------------------

#[test]
fn multi_asset_repeated_refresh_changes_nothing() {
    let (mut w, mut a, mut b) = two_long(50_000);
    w.move_many(&[(0, 960), (1, 1_030)]);
    let c1 = w.refresh(&mut a);
    let s1 = (a.capital.get(), a.pnl.get(), pos(&a, 0), pos(&a, 1), w.header.vault.get(), w.header.c_tot.get());
    for _ in 0..3 {
        let c = w.refresh(&mut a);
        assert_eq!(c.certified_equity, c1.certified_equity);
        assert_eq!(c.certified_maintenance_req, c1.certified_maintenance_req);
        assert_eq!(c.certified_liq_deficit, c1.certified_liq_deficit);
    }
    assert_eq!(s1, (a.capital.get(), a.pnl.get(), pos(&a, 0), pos(&a, 1), w.header.vault.get(), w.header.c_tot.get()));
    w.refresh(&mut b);
    w.sound(&mut [&mut a, &mut b]);
}

#[test]
fn multi_asset_round_trip_price_on_both_assets_returns_to_start_value() {
    let (mut w, mut a, mut b) = two_long(50_000);
    w.move_many(&[(0, 900), (1, 1_100)]);
    w.refresh(&mut a);
    w.refresh(&mut b);
    w.move_many(&[(0, 1_000), (1, 1_000)]);
    w.refresh(&mut a);
    w.refresh(&mut b);
    assert_eq!(value(&a), 50_000);
    assert_eq!(value(&b), BIG as i128);
    w.sound(&mut [&mut a, &mut b]);
}

// ---------------------------------------------------------------------------------------------
// Conservation through loss, liquidation and close across assets
// ---------------------------------------------------------------------------------------------

#[test]
fn multi_asset_conservation_through_loss_liquidation_and_close() {
    let n = 3;
    let mut w = W::new(n);
    let (mut a, mut b, mut c) = (account(1), account(2), account(3));
    w.deposit(&mut a, 30_000);
    w.deposit(&mut b, BIG);
    w.deposit(&mut c, BIG);
    w.trade(0, &mut a, &mut b, Q as i128);
    w.trade(1, &mut c, &mut a, Q as i128);
    w.trade(2, &mut a, &mut c, Q as i128);
    let vault0 = w.header.vault.get();
    w.sound(&mut [&mut a, &mut b, &mut c]);
    // a: -12_000 on asset 0, -8_000 on its asset-1 short: equity 10_000 < MM 14_800
    w.move_many(&[(0, 880), (1, 1_080)]);
    let cert = w.refresh(&mut a);
    assert_eq!((cert.certified_equity, cert.certified_maintenance_req), (10_000, 14_800));
    w.sound(&mut [&mut a, &mut b, &mut c]);
    let mut liquidated = 0;
    for i in [2usize, 0, 1] {
        liquidated += w.liquidate(&mut a, i).is_ok() as u32;
        w.sound(&mut [&mut a, &mut b, &mut c]);
    }
    assert!(liquidated > 0, "at least one cross-asset liquidation ran");
    w.refresh(&mut a);
    w.refresh(&mut b);
    w.refresh(&mut c);
    // close whatever is left on a
    for i in 0..n {
        let p = pos(&a, i);
        if p > 0 {
            let cp = if i == 0 { &mut b } else { &mut c };
            w.trade(i, cp, &mut a, p);
        } else if p < 0 {
            w.trade(i, &mut a, &mut c, -p);
        }
        w.sound(&mut [&mut a, &mut b, &mut c]);
    }
    assert_eq!(n_legs(&a), 0);
    assert_eq!(w.header.vault.get(), vault0, "no deposit / withdrawal happened: vault is fixed");
}
