//! Liquidation edge cases on the v2.2 engine, driven through the public API only
//! (deposit -> trade -> accrue -> liquidate), except that the wrapper-owned raw oracle target
//! is written straight onto the market slot (the wrapper normally stores it). Each test is a small, deterministic world:
//! one asset, a target account and a counterparty that took the other side, a price move,
//! then a liquidation. They pin spec.md section 4.5 (liquidation fee), 4.6 (exact insurance
//! spend), 5.4 (bankrupt residual), 7 (margin / partial vs full close) and 10 (conservation).
//!
//! Every test name contains `liq` so `cargo test liq` selects the area.

use percolator::{
    EngineAssetSlotV16Account, LiquidationOutcomeV16, LiquidationRequestV16, Market,
    MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, PermissionlessCrankActionV16,
    PermissionlessCrankRequestV16, PortfolioAccountV16Account, PortfolioV16ViewMut,
    ProvenanceHeaderV16, ProvenanceHeaderV16Account, TradeRequestV16, V16Config, V16Error,
    POS_SCALE,
};

const Q: u128 = 100 * POS_SCALE; // 100 units, notional 100_000 at price 1_000
const P0: u64 = 1_000;

#[derive(Clone, Copy)]
struct Cfg {
    mm_bps: u64,
    fee_bps: u64,
    fee_cap: u128,
    min_liq_abs: u128,
    min_mm: u128,
    move_bps: u64,
}

impl Default for Cfg {
    fn default() -> Self {
        Cfg {
            mm_bps: 1_000,
            fee_bps: 50,
            fee_cap: 1_000_000,
            min_liq_abs: 0,
            min_mm: 100,
            move_bps: 500,
        }
    }
}

struct World {
    header: MarketGroupV16HeaderAccount,
    markets: Vec<Market<u64>>,
    tgt: PortfolioAccountV16Account,
    cp: PortfolioAccountV16Account,
    slot: u64,
    move_bps: u64,
}

fn account(seed: u8) -> PortfolioAccountV16Account {
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        [1; 32], [seed; 32], [3; 32],
    ));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(header).unwrap();
    a
}

/// One asset at price 1_000 (notional 100_000 for `Q`). The target opens `q` (long if `tgt_long`) against a well-funded
/// counterparty; `tgt_capital` is the target's deposit.
fn world(c: Cfg, tgt_capital: u128, q: u128, tgt_long: bool) -> World {
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    cfg.maintenance_margin_bps = c.mm_bps;
    cfg.initial_margin_bps = c.mm_bps;
    cfg.max_price_move_bps_per_slot = c.move_bps;
    cfg.liquidation_fee_bps = c.fee_bps;
    cfg.liquidation_fee_cap = c.fee_cap;
    cfg.min_liquidation_abs = c.min_liq_abs;
    cfg.min_nonzero_mm_req = c.min_mm;
    cfg.min_nonzero_im_req = c.min_mm + 1;
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut markets = vec![Market::new(0u64, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, P0, 1)
        .unwrap();
    let mut tgt = account(10);
    let mut cp = account(11);
    {
        let mut g = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut tv = PortfolioV16ViewMut::new(&mut tgt);
        let mut cv = PortfolioV16ViewMut::new(&mut cp);
        g.deposit_not_atomic(&mut tv, tgt_capital).unwrap();
        g.deposit_not_atomic(&mut cv, 100_000_000).unwrap();
        let req = TradeRequestV16 {
            asset_index: 0,
            size_q: q as i128,
            exec_price: P0,
            fee_bps: 0,
        };
        if tgt_long {
            g.execute_trade_with_fee_loss_stale_scoped_not_atomic(&mut tv, &mut cv, req, true)
        } else {
            g.execute_trade_with_fee_loss_stale_scoped_not_atomic(&mut cv, &mut tv, req, true)
        }
        .unwrap();
    }
    World {
        header,
        markets,
        tgt,
        cp,
        slot: 2,
        move_bps: c.move_bps,
    }
}

impl World {
    /// Walk the effective price to `target` within the configured `move_bps` per-slot cap,
    /// one slot per step.
    fn move_price(&mut self, target: u64) {
        let mut g = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let mut price = g.markets[0]
            .engine
            .asset
            .try_to_runtime()
            .unwrap()
            .effective_price;
        while price != target {
            let step = (price * self.move_bps / 10_000).max(1);
            price = if target > price {
                (price + step).min(target)
            } else {
                price.saturating_sub(step).max(target)
            };
            self.slot += 1;
            g.accrue_asset_to_not_atomic(0, self.slot, price, 0, true)
                .unwrap();
            g.markets[0].engine.asset.raw_oracle_target_price = percolator::V16PodU64::new(price);
        }
    }

    fn deposit_insurance(&mut self, domain: usize, amount: u128) {
        let mut g = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        g.deposit_domain_insurance_not_atomic(domain, amount)
            .unwrap();
    }

    fn liquidate(&mut self) -> Result<LiquidationOutcomeV16, V16Error> {
        let mut g = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let mut tv = PortfolioV16ViewMut::new(&mut self.tgt);
        g.liquidate_account_not_atomic(&mut tv, LiquidationRequestV16 { asset_index: 0 })
    }

    fn liquidate_against_cp(&mut self) -> Result<LiquidationOutcomeV16, V16Error> {
        let mut g = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let mut tv = PortfolioV16ViewMut::new(&mut self.tgt);
        let mut cv = PortfolioV16ViewMut::new(&mut self.cp);
        g.liquidate_account_against_counterparty_not_atomic(
            &mut tv,
            &mut cv,
            LiquidationRequestV16 { asset_index: 0 },
        )
    }

    fn refresh_tgt(&mut self) -> percolator::HealthCertV16 {
        let mut g = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let mut tv = PortfolioV16ViewMut::new(&mut self.tgt);
        g.full_account_refresh_not_atomic(&mut tv).unwrap()
    }

    /// Refresh the counterparty. After a B booking the plain refresh refuses (BStale); a keeper
    /// then runs the permissionless Refresh crank (same slot, same price), which settles one B
    /// chunk per call, until the plain refresh certifies.
    fn refresh_cp(&mut self) -> percolator::HealthCertV16 {
        let slot = self.slot;
        let mut g = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let price = g.markets[0]
            .engine
            .asset
            .try_to_runtime()
            .unwrap()
            .effective_price;
        let mut cv = PortfolioV16ViewMut::new(&mut self.cp);
        for _ in 0..16 {
            match g.full_account_refresh_not_atomic(&mut cv) {
                Err(V16Error::BStale) => {
                    g.permissionless_crank_not_atomic(
                        &mut cv,
                        PermissionlessCrankRequestV16 {
                            now_slot: slot,
                            asset_index: 0,
                            effective_price: price,
                            funding_rate_e9: 0,
                            action: PermissionlessCrankActionV16::Refresh,
                        },
                    )
                    .unwrap();
                }
                r => return r.unwrap(),
            }
        }
        panic!("counterparty refresh did not converge");
    }

    fn tgt_leg_q(&self) -> u128 {
        let leg = self.tgt.legs[0].try_to_runtime().unwrap();
        if leg.active {
            leg.basis_pos_q.unsigned_abs()
        } else {
            0
        }
    }

    fn tgt_flat(&self) -> bool {
        percolator::active_bitmap_is_empty(self.tgt.active_bitmap.map(|w| w.get()))
    }

    fn insurance(&self) -> u128 {
        self.header.insurance.get()
    }

    /// spec.md section 10 item 1: V >= C_tot + I, plus the engine's own shape and account checks.
    fn assert_sound(&mut self) {
        let v = self.header.vault.get();
        let c = self.header.c_tot.get();
        let i = self.header.insurance.get();
        assert!(v >= c + i, "conservation broken: V {v} < C_tot {c} + I {i}");
        let g = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        g.validate_shape().unwrap();
        let tv = PortfolioV16ViewMut::new(&mut self.tgt);
        tv.validate_with_market(&g.as_view()).unwrap();
        let cv = PortfolioV16ViewMut::new(&mut self.cp);
        cv.validate_with_market(&g.as_view()).unwrap();
    }
}

/// The insurance domain that pays a LONG's bankruptcy on asset 0. Domains are keyed by the
/// winners' side (`consume_domain_insurance_for_negative_pnl` uses `opposite_side(bankrupt)`):
/// domain = asset * 2 + side, so a long loser is covered by domain 1 (asset 0, Short).
fn domain_for_long_loser() -> usize {
    1
}

/// The insurance domain that pays a SHORT's bankruptcy on asset 0 (asset 0, Long).
fn domain_for_short_loser() -> usize {
    0
}

/// spec.md 7: RiskNotional = ceil(|q| * price / POS_SCALE); MM = max(ceil(RN * bps / 1e4), floor).
fn spec_mm(q: u128, price: u64, c: Cfg) -> u128 {
    if q == 0 {
        return 0;
    }
    let rn = (q * price as u128).div_ceil(POS_SCALE);
    (rn * c.mm_bps as u128).div_ceil(10_000).max(c.min_mm)
}

// ---------------------------------------------------------------------------------------------
// Eligibility
// ---------------------------------------------------------------------------------------------

#[test]
fn liq_edge_healthy_account_is_refused_and_untouched() {
    let mut w = world(Cfg::default(), 15_000, Q, true);
    w.move_price(950); // loss 5_000: equity 10_000 >= MM 9_500
    let pre = w.refresh_tgt(); // settle the mark first so the comparison below is like for like
    assert_eq!(pre.certified_liq_deficit, 0, "{pre:?}");
    let before = (
        format!("{:?}", w.tgt),
        w.header.insurance.get(),
        w.header.vault.get(),
        w.header.c_tot.get(),
    );
    let err = w.liquidate().unwrap_err();
    assert_eq!(err, V16Error::NonProgress);
    let after = (
        format!("{:?}", w.tgt),
        w.header.insurance.get(),
        w.header.vault.get(),
        w.header.c_tot.get(),
    );
    assert_eq!(before, after, "a refused liquidation moves nothing");
    w.assert_sound();
}

/// spec.md 7 boundary through the trade path: equity exactly at MM is NOT liquidatable.
#[test]
fn liq_edge_equity_exactly_at_maintenance_is_not_liquidated() {
    // price 950: MM = ceil(95_000 * 10%) = 9_500; capital 14_500 - loss 5_000 = 9_500 == MM.
    let mut w = world(Cfg::default(), 14_500, Q, true);
    w.move_price(950);
    let pre = w.refresh_tgt();
    assert_eq!(pre.certified_equity, 9_500);
    assert_eq!(pre.certified_maintenance_req, 9_500);
    assert_eq!(pre.certified_liq_deficit, 0);
    assert_eq!(w.liquidate().unwrap_err(), V16Error::NonProgress);
    // one atom lower is liquidatable
    let mut w = world(Cfg::default(), 14_499, Q, true);
    w.move_price(950);
    assert_eq!(w.refresh_tgt().certified_liq_deficit, 1);
    let out = w.liquidate().unwrap();
    assert!(out.closed_q > 0);
    assert_eq!(w.refresh_tgt().certified_liq_deficit, 0);
    w.assert_sound();
}

#[test]
fn liq_edge_wrong_asset_index_is_refused() {
    let mut w = world(Cfg::default(), 15_000, Q, true);
    w.move_price(850);
    let out = {
        let mut g = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
        let mut tv = PortfolioV16ViewMut::new(&mut w.tgt);
        g.liquidate_account_not_atomic(&mut tv, LiquidationRequestV16 { asset_index: 1 })
    };
    assert_eq!(
        out.unwrap_err(),
        V16Error::InvalidConfig,
        "index outside the market"
    );
    assert_eq!(w.tgt_leg_q(), Q, "nothing closed");
}

#[test]
fn liq_edge_flat_account_is_refused() {
    let mut w = world(Cfg::default(), 15_000, Q, true);
    let mut flat = account(12);
    let mut g = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
    let mut fv = PortfolioV16ViewMut::new(&mut flat);
    g.deposit_not_atomic(&mut fv, 1_000).unwrap();
    let out = g.liquidate_account_not_atomic(&mut fv, LiquidationRequestV16 { asset_index: 0 });
    assert_eq!(
        out.unwrap_err(),
        V16Error::InvalidLeg,
        "an account with no leg"
    );
    assert_eq!(fv.header.capital.get(), 1_000);
}

// ---------------------------------------------------------------------------------------------
// Partial close and health after the close
// ---------------------------------------------------------------------------------------------

/// Long, mildly underwater (equity 7_000 < MM 9_200 at price 920): the engine picks a partial
/// close, the account is maintenance-healthy afterwards, and the close is the MINIMAL one --
/// one quantum less would leave it unhealthy (spec.md 7: the remaining position must be healthy).
#[test]
fn liq_edge_partial_close_is_minimal_and_restores_health_long() {
    let c = Cfg::default();
    let mut w = world(c, 15_000, Q, true);
    w.move_price(920);
    let pre = w.refresh_tgt();
    assert!(
        pre.certified_liq_deficit > 0,
        "fixture must be liquidatable: {pre:?}"
    );
    let equity_before = pre.certified_equity;

    let out = w.liquidate().unwrap();
    assert!(
        out.closed_q > 0 && out.closed_q < Q,
        "partial close expected: {out:?}"
    );
    assert_eq!(out.insurance_used, 0);
    assert_eq!(out.residual_booked, 0);
    assert_eq!(out.explicit_loss, 0);
    assert_eq!(w.tgt_leg_q(), Q - out.closed_q);

    // health after the close, from the engine's own refresh
    let post = w.refresh_tgt();
    assert_eq!(
        post.certified_liq_deficit, 0,
        "remaining position must be healthy: {post:?}"
    );
    assert!(post.certified_equity >= post.certified_maintenance_req as i128);
    assert_eq!(
        post.certified_equity,
        equity_before - out.fee_charged as i128
    );
    assert_eq!(
        post.certified_maintenance_req,
        spec_mm(Q - out.closed_q, 920, c)
    );

    // minimality, from the spec formulas: closing one quantum less is not healthy. The fee
    // oracle uses a CEIL closed notional like the engine and upstream master
    // (`liquidation_risk_notional_ceil`); this repo's spec.md 4.5 still says floor.
    let smaller = out.closed_q - 1;
    let fee_smaller = ((smaller * 920).div_ceil(POS_SCALE) * c.fee_bps as u128).div_ceil(10_000);
    let eq_smaller = equity_before - fee_smaller as i128;
    assert!(
        eq_smaller < spec_mm(Q - smaller, 920, c) as i128,
        "a smaller close ({smaller}) would also be healthy: the engine over-closed"
    );
    w.assert_sound();
}

#[test]
fn liq_edge_partial_close_is_minimal_and_restores_health_short() {
    let c = Cfg::default();
    let mut w = world(c, 15_000, Q, false);
    w.move_price(1_080); // short loss 8_000: equity 7_000 < MM 10_800
    let pre = w.refresh_tgt();
    assert!(pre.certified_liq_deficit > 0);
    let out = w.liquidate().unwrap();
    assert!(
        out.closed_q > 0 && out.closed_q < Q,
        "partial close expected: {out:?}"
    );
    let post = w.refresh_tgt();
    assert_eq!(post.certified_liq_deficit, 0, "{post:?}");
    assert_eq!(
        post.certified_equity,
        pre.certified_equity - out.fee_charged as i128
    );
    let smaller = out.closed_q - 1;
    let fee_smaller = ((smaller * 1_080).div_ceil(POS_SCALE) * c.fee_bps as u128).div_ceil(10_000);
    assert!(pre.certified_equity - (fee_smaller as i128) < spec_mm(Q - smaller, 1_080, c) as i128);
    w.assert_sound();
}

/// After a healthy partial close, a second liquidation call is refused: no second fee, no
/// further close (lenient: the engine never takes more than restores health).
#[test]
fn liq_edge_second_call_after_partial_is_refused() {
    let mut w = world(Cfg::default(), 15_000, Q, true);
    w.move_price(920);
    let out = w.liquidate().unwrap();
    assert!(out.closed_q < Q);
    let cap = w.tgt.capital.get();
    let q = w.tgt_leg_q();
    assert_eq!(w.liquidate().unwrap_err(), V16Error::NonProgress);
    assert_eq!(w.tgt.capital.get(), cap, "no second fee");
    assert_eq!(w.tgt_leg_q(), q, "no second close");
    w.assert_sound();
}

/// A clean (non-bankrupt) unilateral partial close happens at the mark: it moves no value onto
/// or off the winning counterparty. Its certified equity is the same before and after.
#[test]
fn liq_edge_clean_unilateral_partial_does_not_charge_the_winner() {
    let mut w = world(Cfg::default(), 15_000, Q, true);
    w.move_price(920);
    // settle the loser first: its realised loss is what backs the winner's gain
    w.refresh_tgt();
    let cp_before = w.refresh_cp();
    assert_eq!(
        cp_before.certified_equity, 100_008_000,
        "gain 8_000, fully backed"
    );
    let cp_pnl = w.cp.pnl.get();
    let out = w.liquidate().unwrap();
    assert!(out.closed_q < Q);
    assert_eq!(
        out.residual_booked + out.explicit_loss + out.insurance_used,
        0
    );
    let cp_after = w.refresh_cp();
    assert_eq!(w.cp.pnl.get(), cp_pnl, "winner PnL claim unchanged");
    assert_eq!(cp_after.certified_equity, cp_before.certified_equity);
    let asset = w.markets[0].engine.asset.try_to_runtime().unwrap();
    assert_eq!(
        asset.oi_eff_long_q, asset.oi_eff_short_q,
        "effective OI stays balanced"
    );
    w.assert_sound();
}

/// A partial close leaves the account healthy, then the price keeps falling: the account
/// becomes liquidatable again and the next liquidation again restores health.
#[test]
fn liq_edge_repeated_partial_closes_on_a_falling_price() {
    let c = Cfg::default();
    let mut w = world(c, 15_000, Q, true);
    let mut closes = 0;
    for p in [920u64, 900, 890] {
        w.move_price(p);
        let pre = w.refresh_tgt();
        if pre.certified_liq_deficit == 0 {
            continue;
        }
        let out = w.liquidate().unwrap();
        closes += 1;
        assert_eq!(
            out.insurance_used + out.residual_booked + out.explicit_loss,
            0
        );
        let post = w.refresh_tgt();
        assert_eq!(
            post.certified_liq_deficit, 0,
            "healthy after close #{closes} at {p}: {post:?}"
        );
        w.assert_sound();
    }
    assert!(
        closes >= 2,
        "the walk must liquidate more than once, got {closes}"
    );
    assert!(!w.tgt_flat(), "still a partial position");
}

/// Raw oracle target below the effective price (a long's adverse lag): the maintenance
/// requirement carries `target_effective_lag_penalty = ceil(|q| * (P_eff - P_target) / POS_SCALE)`.
/// That term is upstream spec section 7 (aeyakovenko/percolator master: `portfolio_maintenance_req
/// = gross_mm + target_effective_lag_penalty`) and upstream's engine; this repo's older spec.md
/// section 7 does not list it yet. The engine's partial close sizes against the penalised
/// requirement, so the account is healthy against it afterwards, and the close is still minimal.
#[test]
fn liq_edge_partial_close_accounts_for_target_lag_penalty() {
    let c = Cfg::default();
    let mut w = world(c, 15_000, Q, true);
    w.move_price(920);
    w.markets[0].engine.asset.raw_oracle_target_price = percolator::V16PodU64::new(900);
    let pre = w.refresh_tgt();
    let lag = |q: u128| (q * 20).div_ceil(POS_SCALE);
    assert_eq!(
        pre.certified_maintenance_req,
        spec_mm(Q, 920, c) + lag(Q),
        "{pre:?}"
    );
    let out = w.liquidate().unwrap();
    assert!(out.closed_q > 0 && out.closed_q < Q, "{out:?}");
    let post = w.refresh_tgt();
    assert_eq!(post.certified_liq_deficit, 0, "{post:?}");
    let rest = Q - out.closed_q;
    assert_eq!(
        post.certified_maintenance_req,
        spec_mm(rest, 920, c) + lag(rest)
    );
    let smaller = out.closed_q - 1;
    let fee_smaller = ((smaller * 920).div_ceil(POS_SCALE) * c.fee_bps as u128).div_ceil(10_000);
    assert!(
        pre.certified_equity - (fee_smaller as i128)
            < (spec_mm(Q - smaller, 920, c) + lag(Q - smaller)) as i128,
        "the engine over-closed"
    );
    w.assert_sound();
}

/// spec.md 4.5: the charged fee is capped by `cfg_liquidation_fee_cap`.
#[test]
fn liq_edge_fee_is_capped() {
    let c = Cfg {
        fee_bps: 1_000,
        fee_cap: 7,
        ..Cfg::default()
    };
    let mut w = world(c, 15_000, Q, true);
    w.move_price(920);
    let out = w.liquidate().unwrap();
    assert_eq!(out.fee_charged, 7, "{out:?}");
    assert_eq!(w.refresh_tgt().certified_liq_deficit, 0);
    w.assert_sound();
}

/// Zero fee config: liquidation charges nothing and still closes only what restores health.
#[test]
fn liq_edge_zero_fee_partial_close() {
    let c = Cfg {
        fee_bps: 0,
        fee_cap: 0,
        ..Cfg::default()
    };
    let mut w = world(c, 15_000, Q, true);
    w.move_price(920);
    let pre = w.refresh_tgt();
    let out = w.liquidate().unwrap();
    assert_eq!(out.fee_charged, 0);
    assert!(out.closed_q < Q);
    let post = w.refresh_tgt();
    assert_eq!(
        post.certified_equity, pre.certified_equity,
        "no value moved by a zero-fee close"
    );
    assert_eq!(post.certified_liq_deficit, 0);
    // current engine timing: the 8_000 loss is settled from principal by the liquidation's
    // refresh (15_000 - 8_000); nothing else is taken
    assert_eq!(w.tgt.capital.get(), 7_000);
    assert_eq!(w.tgt.pnl.get(), 0);
    w.assert_sound();
}

/// A partial chunk whose raw fee is below `cfg_min_liquidation_abs` is not an admissible partial
/// (`liquidation_fee_from_raw_fee` returns NonProgress for it, same as upstream master); the
/// engine then closes the leg fully and charges `max(raw, floor)`, capped. Pins that policy.
#[test]
fn liq_edge_fee_floor_forces_full_close_when_partial_is_below_floor() {
    let c = Cfg {
        fee_bps: 1,
        min_liq_abs: 1_000,
        min_mm: 1_200,
        move_bps: 100,
        ..Cfg::default()
    };
    let mut w = world(c, 15_000, Q, true);
    w.move_price(920);
    let pre = w.refresh_tgt();
    assert!(pre.certified_liq_deficit > 0, "{pre:?}");
    let out = w.liquidate().unwrap();
    assert_eq!(
        out.closed_q, Q,
        "no partial is admissible under the fee floor: {out:?}"
    );
    assert!(w.tgt_flat());
    assert_eq!(out.fee_charged, 1_000, "full close charges max(raw, floor)");
    assert_eq!(w.tgt.capital.get() as i128, pre.certified_equity - 1_000);
    w.assert_sound();
}

/// Same floor, but the raw fee of the minimal healthy chunk is above the floor: the partial is
/// admissible again and the floor does not force a full close.
#[test]
fn liq_edge_fee_floor_allows_partial_when_raw_fee_clears_it() {
    let c = Cfg {
        fee_bps: 50,
        min_liq_abs: 10,
        min_mm: 1_200,
        move_bps: 100,
        ..Cfg::default()
    };
    let mut w = world(c, 15_000, Q, true);
    w.move_price(920);
    let out = w.liquidate().unwrap();
    assert!(out.closed_q < Q, "{out:?}");
    let raw = ((out.closed_q * 920).div_ceil(POS_SCALE) * 50).div_ceil(10_000);
    assert!(raw >= 10, "the raw fee clears the floor ({raw})");
    assert_eq!(out.fee_charged, raw);
    assert_eq!(w.refresh_tgt().certified_liq_deficit, 0);
    w.assert_sound();
}

// ---------------------------------------------------------------------------------------------
// Full close, bankrupt residual, insurance
// ---------------------------------------------------------------------------------------------

/// Underwater (equity < 0): the leg is fully closed and the deficit is fully covered by the
/// domain's insurance. spec.md 4.6: insurance spends exactly the deficit, not more.
#[test]
fn liq_edge_bankrupt_full_close_insurance_covers_exactly() {
    let mut w = world(Cfg::default(), 10_000, Q, true);
    w.deposit_insurance(domain_for_long_loser(), 1_000_000);
    w.move_price(850); // loss 15_000 on capital 10_000: deficit 5_000
    let pre = w.refresh_tgt();
    assert!(pre.certified_equity < 0, "{pre:?}");
    let deficit = pre.certified_equity.unsigned_abs();
    let ins_before = w.insurance();

    let out = w.liquidate().unwrap();
    assert_eq!(out.closed_q, Q);
    assert!(w.tgt_flat());
    assert_eq!(
        out.fee_charged, 0,
        "a bankrupt account pays no fee (engine: no fee when pnl < 0, no fee debt)"
    );
    assert_eq!(out.insurance_used, deficit, "exact spend: {out:?}");
    assert_eq!(out.residual_booked + out.explicit_loss, 0);
    assert_eq!(w.insurance(), ins_before - deficit);
    assert_eq!(w.tgt.capital.get(), 0);
    assert_eq!(w.tgt.pnl.get(), 0, "deficit cleared");
    w.assert_sound();
}

/// A fully insured bankruptcy still raises the bankruptcy h-lock, attributed to the bankrupt
/// domain (single-asset group), and the public re-evaluation does NOT clear it while the winner
/// still holds an unconverted claim sourced from that domain (P2b L1:
/// `bankruptcy_hlock_claim_term_clear`). Every other clear condition of
/// `try_clear_bankruptcy_hlock_if_healthy` is asserted to hold, so the claim term is what holds
/// the lock -- the deficit itself is fully gone. (No positive control: making the winner convert
/// or exit needs the warm-up/conversion path, out of scope here.)
#[test]
fn liq_edge_full_insurance_cover_hlock_waits_only_for_winner_claim() {
    let mut w = world(Cfg::default(), 10_000, Q, true);
    w.deposit_insurance(domain_for_long_loser(), 1_000_000);
    w.move_price(850);
    w.refresh_cp();
    assert_eq!(
        w.header.bankruptcy_hlock_active, 0,
        "no lock before the bankruptcy"
    );
    let out = w.liquidate().unwrap();
    assert_eq!(out.insurance_used, 5_000);
    assert_eq!(out.residual_booked + out.explicit_loss, 0);
    // all non-claim clear conditions hold
    assert_eq!(w.header.negative_pnl_account_count.get(), 0);
    assert_eq!(w.header.stale_certificate_count.get(), 0);
    assert_eq!(w.header.b_stale_account_count.get(), 0);
    assert!(w.header.recovery_reason.try_to_runtime().unwrap().is_none());
    let slot0 = &w.markets[0].engine;
    assert_eq!(
        slot0.pending_domain_loss_barrier_long.get(),
        0,
        "no unabsorbed loss"
    );
    assert_eq!(
        slot0.pending_domain_loss_barrier_short.get(),
        0,
        "no unabsorbed loss"
    );
    // the winner (short) holds a claim sourced from the bankrupt long domain (asset 0, Long)
    let src = slot0.source_credit_long.try_to_runtime().unwrap();
    assert!(
        src.positive_claim_bound_num != 0 || src.exact_positive_claim_num != 0,
        "the attributed domain carries the winner's claim: {src:?}"
    );
    let hlock = w.header.bankruptcy_hlock_active;
    assert_ne!(hlock, 0, "the bankruptcy is recorded");
    assert_eq!(
        percolator::bankruptcy_hlock_domain_mask(hlock),
        1 << 0,
        "attributed to domain 0 (asset 0, Long: the winners' claim source), not unattributed"
    );
    assert!(w.cp.pnl.get() > 0, "the winner still holds its claim");
    let cleared = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets)
        .try_clear_bankruptcy_hlock_not_atomic()
        .unwrap();
    assert!(!cleared, "held by the winner's claim");
    assert_eq!(
        w.header.bankruptcy_hlock_active, hlock,
        "a refused clear changes nothing"
    );
    w.assert_sound();
}

/// The residual left after insurance runs out goes to the winners (B booking) or explicit
/// loss, never back onto insurance below zero, and the liquidated account ends with no debt.
#[test]
fn liq_edge_residual_after_partial_insurance() {
    let mut w = world(Cfg::default(), 10_000, Q, true);
    w.deposit_insurance(domain_for_long_loser(), 2_000);
    w.move_price(850); // deficit 5_000, insurance 2_000
    let pre = w.refresh_tgt();
    let deficit = pre.certified_equity.unsigned_abs();
    assert_eq!(deficit, 5_000);
    let cp_eq_before = w.refresh_cp().certified_equity;
    let cp_pnl_before = w.cp.pnl.get();
    assert_eq!(cp_pnl_before, 15_000, "the winner's gross gain");

    let out = w.liquidate().unwrap();
    assert!(w.tgt_flat());
    assert_eq!(out.insurance_used, 2_000, "spends min(D, I): {out:?}");
    assert_eq!(
        out.residual_booked + out.explicit_loss,
        deficit - 2_000,
        "{out:?}"
    );
    assert_eq!(w.tgt.pnl.get(), 0);
    w.assert_sound();

    // The winner bears exactly the booked residual on its PnL claim (gain 15_000 minus the
    // 3_000 B booking), and what is left is fully backed by the vault: V - C_tot - I >= PnL_pos.
    // Its certified equity does not fall: it was only ever credited with the backed part
    // (loser capital), and insurance covered 2_000 of the deficit.
    let cp_eq_after = w.refresh_cp().certified_equity;
    assert_eq!(w.cp.pnl.get(), cp_pnl_before - out.residual_booked as i128);
    let backing = w.header.vault.get() - w.header.c_tot.get() - w.header.insurance.get();
    assert!(
        backing >= w.header.pnl_pos_tot.get(),
        "{backing} < {}",
        w.header.pnl_pos_tot.get()
    );
    assert!(
        cp_eq_after >= cp_eq_before,
        "winner equity fell: {cp_eq_before} -> {cp_eq_after}"
    );
    w.assert_sound();
}

/// No insurance at all: the whole residual is booked to the opposite side (or explicit loss),
/// the insurance fund is not touched and the account is cleared.
#[test]
fn liq_edge_residual_with_no_insurance_books_to_winners() {
    let mut w = world(Cfg::default(), 10_000, Q, true);
    w.move_price(850);
    let deficit = w.refresh_tgt().certified_equity.unsigned_abs();
    let out = w.liquidate().unwrap();
    assert_eq!(out.insurance_used, 0);
    assert_eq!(w.insurance(), 0);
    assert_eq!(out.residual_booked + out.explicit_loss, deficit, "{out:?}");
    assert_eq!(w.tgt.pnl.get(), 0);
    assert!(w.tgt_flat());
    w.assert_sound();
}

/// Insurance on the OTHER domain of the same asset is never spent for a long bankruptcy
/// (per-domain insurance, spec req 5/27).
#[test]
fn liq_edge_insurance_of_other_domain_is_not_spent() {
    let mut w = world(Cfg::default(), 10_000, Q, true);
    w.deposit_insurance(domain_for_short_loser(), 1_000_000);
    w.move_price(850);
    let ins_before = w.insurance();
    let out = w.liquidate().unwrap();
    assert_eq!(out.insurance_used, 0, "{out:?}");
    assert_eq!(w.insurance(), ins_before);
    assert!(out.residual_booked + out.explicit_loss > 0);
    w.assert_sound();
}

/// Short bankruptcy mirrors the long one and spends exactly the winners' (Long) domain, even
/// when the loser-side domain also holds insurance.
#[test]
fn liq_edge_short_bankrupt_full_close_uses_winners_domain() {
    let mut w = world(Cfg::default(), 10_000, Q, false);
    w.deposit_insurance(domain_for_short_loser(), 1_000_000);
    w.deposit_insurance(domain_for_long_loser(), 3_000);
    w.move_price(1_150);
    let deficit = w.refresh_tgt().certified_equity.unsigned_abs();
    assert_eq!(deficit, 5_000);
    let out = w.liquidate().unwrap();
    assert!(w.tgt_flat());
    assert_eq!(out.insurance_used, deficit);
    assert_eq!(
        w.markets[0].engine.insurance_domain_spent_long.get(),
        deficit,
        "the long (winners) domain paid"
    );
    assert_eq!(
        w.markets[0].engine.insurance_domain_spent_short.get(),
        0,
        "the loser-side domain did not"
    );
    w.assert_sound();
}

/// Equity positive but tiny (100) and the fee for the only healthy close is larger than that:
/// the fee is bounded by what the account has, the account never goes negative and nothing is
/// socialised. Engine policy (`charge_account_fee_current_not_atomic` charges `min(fee, capital)`
/// and writes no fee debt): more lenient than spec.md 4.5, which would record a collectible
/// shortfall as negative `fee_credits`.
#[test]
fn liq_edge_fee_never_exceeds_capital_on_full_close() {
    let mut w = world(Cfg::default(), 10_000, Q, true);
    w.move_price(901); // loss 9_900: equity 100 < MM 9_010
    let pre = w.refresh_tgt();
    assert_eq!(pre.certified_equity, 100, "{pre:?}");
    let out = w.liquidate().unwrap();
    assert_eq!(out.closed_q, Q, "{out:?}");
    assert!(w.tgt_flat());
    assert_eq!(
        out.fee_charged, 100,
        "fee clipped to the 100 the account has"
    );
    assert_eq!(
        out.insurance_used + out.residual_booked + out.explicit_loss,
        0
    );
    assert_eq!(w.tgt.capital.get(), 0);
    assert!(w.tgt.pnl.get() >= 0);
    w.assert_sound();
}

// ---------------------------------------------------------------------------------------------
// Matched counterparty close
// ---------------------------------------------------------------------------------------------

/// A clean partial against the bound counterparty reduces both legs by the same amount and
/// leaves both accounts consistent; the target ends healthy.
#[test]
fn liq_edge_matched_partial_reduces_both_legs_equally() {
    let mut w = world(Cfg::default(), 15_000, Q, true);
    w.move_price(920);
    w.refresh_cp();
    let cp_q_before = w.cp.legs[0]
        .try_to_runtime()
        .unwrap()
        .basis_pos_q
        .unsigned_abs();
    let out = w.liquidate_against_cp().unwrap();
    assert!(out.closed_q < Q);
    let cp_q_after = w.cp.legs[0]
        .try_to_runtime()
        .unwrap()
        .basis_pos_q
        .unsigned_abs();
    assert_eq!(
        cp_q_before - cp_q_after,
        out.closed_q,
        "matched: same quantity off both sides"
    );
    let asset = w.markets[0].engine.asset.try_to_runtime().unwrap();
    assert_eq!(
        asset.oi_eff_long_q, asset.oi_eff_short_q,
        "OI stays balanced"
    );
    assert_eq!(asset.a_long, percolator::ADL_ONE, "no quantity ADL");
    assert_eq!(asset.a_short, percolator::ADL_ONE);
    assert_eq!(w.refresh_tgt().certified_liq_deficit, 0);
    w.assert_sound();
}

/// A bankrupt close never uses the counterparty match (it falls back to the unilateral path),
/// and the outcome equals the plain liquidation.
#[test]
fn liq_edge_matched_entry_on_bankrupt_close_equals_plain() {
    let mk = || {
        let mut w = world(Cfg::default(), 10_000, Q, true);
        w.deposit_insurance(domain_for_long_loser(), 2_000);
        w.move_price(850);
        w
    };
    let mut a = mk();
    let mut b = mk();
    a.refresh_cp();
    b.refresh_cp();
    let oa = a.liquidate().unwrap();
    let ob = b.liquidate_against_cp().unwrap();
    assert_eq!(oa, ob);
    assert_eq!(format!("{:?}", a.tgt), format!("{:?}", b.tgt));
    assert_eq!(format!("{:?}", a.markets), format!("{:?}", b.markets));
    b.assert_sound();
}

// ---------------------------------------------------------------------------------------------
// Two assets (cross margin)
// ---------------------------------------------------------------------------------------------

struct World2 {
    header: MarketGroupV16HeaderAccount,
    markets: Vec<Market<u64>>,
    tgt: PortfolioAccountV16Account,
    cp: PortfolioAccountV16Account,
    slot: u64,
}

/// Two assets at price 1_000; the target is long `Q` on both against one counterparty.
fn world2(tgt_capital: u128) -> World2 {
    let c = Cfg::default();
    let mut cfg = V16Config::public_user_fund_with_market_slots(2, 2, 0, 10);
    cfg.maintenance_margin_bps = c.mm_bps;
    cfg.initial_margin_bps = c.mm_bps;
    cfg.max_price_move_bps_per_slot = c.move_bps;
    cfg.liquidation_fee_bps = c.fee_bps;
    cfg.liquidation_fee_cap = c.fee_cap;
    cfg.min_nonzero_mm_req = c.min_mm;
    cfg.min_nonzero_im_req = c.min_mm + 1;
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 2, 0).unwrap();
    let mut markets: Vec<Market<u64>> = (0..2)
        .map(|i| Market::new(i as u64, EngineAssetSlotV16Account::default()))
        .collect();
    for (i, m) in markets.iter_mut().enumerate() {
        header
            .activate_empty_asset_slot_not_atomic(i as u32, &mut m.engine, P0, (i + 1) as u64)
            .unwrap();
    }
    let mut tgt = account(20);
    let mut cp = account(21);
    {
        let mut g = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut tv = PortfolioV16ViewMut::new(&mut tgt);
        let mut cv = PortfolioV16ViewMut::new(&mut cp);
        g.deposit_not_atomic(&mut tv, tgt_capital).unwrap();
        g.deposit_not_atomic(&mut cv, 100_000_000).unwrap();
        for asset_index in 0..2 {
            let req = TradeRequestV16 {
                asset_index,
                size_q: Q as i128,
                exec_price: P0,
                fee_bps: 0,
            };
            g.execute_trade_with_fee_loss_stale_scoped_not_atomic(&mut tv, &mut cv, req, true)
                .unwrap();
        }
    }
    let mut w = World2 {
        header,
        markets,
        tgt,
        cp,
        slot: 3,
    };
    // bring both assets to one slot so neither is loss-stale
    for a in 0..2 {
        w.accrue(a, P0);
    }
    w
}

impl World2 {
    fn accrue(&mut self, asset_index: usize, price: u64) {
        let mut g = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        g.accrue_asset_to_not_atomic(asset_index, self.slot, price, 0, true)
            .unwrap();
        g.markets[asset_index].engine.asset.raw_oracle_target_price =
            percolator::V16PodU64::new(price);
    }

    /// Walk asset `a` to `target` (5%/slot), keeping the other asset current at its own price.
    fn move_price(&mut self, a: usize, target: u64) {
        let price_of = |w: &World2, i: usize| {
            w.markets[i]
                .engine
                .asset
                .try_to_runtime()
                .unwrap()
                .effective_price
        };
        let mut p = price_of(self, a);
        while p != target {
            let step = (p * 500 / 10_000).max(1);
            p = if target > p {
                (p + step).min(target)
            } else {
                p.saturating_sub(step).max(target)
            };
            self.slot += 1;
            self.accrue(a, p);
            let other = 1 - a;
            let po = price_of(self, other);
            self.accrue(other, po);
        }
    }

    fn refresh_tgt(&mut self) -> percolator::HealthCertV16 {
        let mut g = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let mut tv = PortfolioV16ViewMut::new(&mut self.tgt);
        g.full_account_refresh_not_atomic(&mut tv).unwrap()
    }

    fn liquidate(&mut self, asset_index: usize) -> Result<LiquidationOutcomeV16, V16Error> {
        let mut g = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let mut tv = PortfolioV16ViewMut::new(&mut self.tgt);
        g.liquidate_account_not_atomic(&mut tv, LiquidationRequestV16 { asset_index })
    }

    fn leg_q(&self, asset_index: u32) -> u128 {
        (0..2)
            .map(|s| self.tgt.legs[s].try_to_runtime().unwrap())
            .find(|l| l.active && l.asset_index == asset_index)
            .map(|l| l.basis_pos_q.unsigned_abs())
            .unwrap_or(0)
    }

    fn assert_sound(&mut self) {
        let (v, c, i) = (
            self.header.vault.get(),
            self.header.c_tot.get(),
            self.header.insurance.get(),
        );
        assert!(v >= c + i, "conservation broken: V {v} < C_tot {c} + I {i}");
        let g = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        g.validate_shape().unwrap();
        let tv = PortfolioV16ViewMut::new(&mut self.tgt);
        tv.validate_with_market(&g.as_view()).unwrap();
        let cv = PortfolioV16ViewMut::new(&mut self.cp);
        cv.validate_with_market(&g.as_view()).unwrap();
    }
}

/// Cross margin, solvent but below MM: asset 0 fell, asset 1 is flat in PnL. Liquidating EITHER
/// leg is a partial that restores whole-account health, and only that leg moves.
#[test]
fn liq_edge_cross_margin_partial_on_either_leg_restores_account_health() {
    for pick in [0usize, 1] {
        let mut w = world2(25_000); // MM 20_000 at par
        w.move_price(0, 920); // loss 8_000: equity 17_000 < MM 19_200
        let pre = w.refresh_tgt();
        assert!(
            pre.certified_liq_deficit > 0 && pre.certified_equity > 0,
            "{pre:?}"
        );
        let other = 1 - pick as u32;
        let out = w.liquidate(pick).unwrap();
        assert!(out.closed_q > 0 && out.closed_q < Q, "pick {pick}: {out:?}");
        assert_eq!(w.leg_q(pick as u32), Q - out.closed_q);
        assert_eq!(w.leg_q(other), Q, "the other leg is untouched");
        let post = w.refresh_tgt();
        assert_eq!(post.certified_liq_deficit, 0, "pick {pick}: {post:?}");
        assert_eq!(
            post.certified_equity,
            pre.certified_equity - out.fee_charged as i128
        );
        w.assert_sound();
    }
}

/// Cross margin, bankrupt (equity < 0) with two legs: the first leg's liquidation must not
/// force a market-wide Recovery and must not invent an attribution for the loss; the second
/// liquidation leaves the account flat with no debt left on it (spec req 5 / 27, GH#292).
#[test]
fn liq_edge_cross_margin_bankrupt_two_legs_resolves_without_recovery() {
    let mut booked_b = Vec::new();
    for order in [[1usize, 0], [0, 1]] {
        let mut w = world2(20_000);
        w.move_price(0, 780); // loss 22_000 on equity 20_000: equity -2_000
        let pre = w.refresh_tgt();
        assert_eq!(pre.certified_equity, -2_000, "{pre:?}");
        let ins_before = w.header.insurance.get();
        // every per-asset loss index a residual booking can write (B, K, social-loss carry)
        let loss_of = |w: &World2, i: usize| {
            let a = w.markets[i].engine.asset.try_to_runtime().unwrap();
            (
                (a.k_long, a.k_short, a.b_long_num, a.b_short_num),
                (a.b_epoch_start_long_num, a.b_epoch_start_short_num),
                (
                    a.social_loss_remainder_long_num,
                    a.social_loss_remainder_short_num,
                ),
                (a.social_loss_dust_long_num, a.social_loss_dust_short_num),
                (
                    a.explicit_unallocated_loss_long,
                    a.explicit_unallocated_loss_short,
                ),
            )
        };
        let mut outs = Vec::new();
        for pick in order {
            let other_before = loss_of(&w, 1 - pick);
            let own_before = loss_of(&w, pick);
            match w.liquidate(pick) {
                Ok(o) => outs.push((pick, o)),
                Err(e) => panic!(
                    "order {order:?}: liquidation of leg {pick} failed: {e:?} after {outs:?}"
                ),
            }
            assert_eq!(
                w.header.mode, 0,
                "order {order:?}: market must stay Live after leg {pick}"
            );
            assert!(w.header.recovery_reason.try_to_runtime().unwrap().is_none());
            // spec req 5/27: a liquidation call writes loss only onto the asset it closes
            assert_eq!(
                loss_of(&w, 1 - pick),
                other_before,
                "order {order:?}: closing leg {pick} touched the other asset's loss indices"
            );
            if pick == 0 {
                // the residual lands on asset 0's SHORT (winners) B; closing the only long drains
                // both sides, which reset, so the booked B survives as `b_epoch_start_short_num`
                let after = loss_of(&w, 0);
                assert_eq!(own_before.1, (0, 0));
                assert_eq!(after.1 .0, 0, "order {order:?}: asset 0 long B untouched");
                assert_ne!(after.1 .1, 0, "order {order:?}: asset 0 short B charged");
                booked_b.push(after.1 .1);
            }
            w.assert_sound();
        }
        assert_eq!(
            w.leg_q(0) + w.leg_q(1),
            0,
            "order {order:?}: flat after both closes: {outs:?}"
        );
        assert!(
            w.tgt.pnl.get() >= 0,
            "order {order:?}: no debt left on a flat account"
        );
        assert_eq!(w.header.negative_pnl_account_count.get(), 0);
        assert_eq!(w.tgt.capital.get(), 0, "all principal went to the loss");
        // the 2_000 deficit is accounted once: insurance (none funded) + booked + written off
        let resolved: u128 = outs
            .iter()
            .map(|(_, o)| o.insurance_used + o.residual_booked + o.explicit_loss)
            .sum();
        assert_eq!(
            resolved, 2_000,
            "order {order:?}: the deficit is resolved exactly once: {outs:?}"
        );
        // spec req 27: the close order does not decide who pays -- asset 0's winners, both ways
        let booked_on_leg0: u128 = outs
            .iter()
            .filter(|(p, _)| *p == 0)
            .map(|(_, o)| o.residual_booked)
            .sum();
        assert_eq!(booked_on_leg0, 2_000, "order {order:?}: {outs:?}");
        assert_eq!(
            w.header.insurance.get(),
            ins_before,
            "no fee or insurance moved on a bankrupt account"
        );
    }
    assert_eq!(
        booked_b[0], booked_b[1],
        "same B charge whichever leg closes first"
    );
}
