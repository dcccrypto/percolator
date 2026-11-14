//! `min_liquidation_abs` boundary map at the maintenance-margin floor (mm floor = 100).
//! Scaffold copied from liquidations_edge.rs; every test name contains `min_liq_abs`.
//! Covers config validation (envelope + fee-cap bound) and the liquidation fee floor on tiny,
//! mid and large positions. No engine bug found, so there are no #[ignore] tests.
#![allow(dead_code)]

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

fn cfg_with(c: Cfg) -> V16Config {
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    cfg.maintenance_margin_bps = c.mm_bps;
    cfg.initial_margin_bps = c.mm_bps;
    cfg.max_price_move_bps_per_slot = c.move_bps;
    cfg.liquidation_fee_bps = c.fee_bps;
    cfg.liquidation_fee_cap = c.fee_cap;
    cfg.min_liquidation_abs = c.min_liq_abs;
    cfg.min_nonzero_mm_req = c.min_mm;
    cfg.min_nonzero_im_req = c.min_mm + 1;
    cfg
}

fn ok(c: Cfg) -> bool {
    MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg_with(c), 1, 0).is_ok()
}

fn bad_cfg(c: Cfg) -> bool {
    matches!(
        MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg_with(c), 1, 0),
        Err(V16Error::InvalidConfig)
    )
}

// Validation sites: src/v16.rs `validate_public_user_fund_shape` (min_liquidation_abs >
// liquidation_fee_cap, ~L5650) and `validate_exact_solvency_envelope_with` (floor region L5536-5553, per-notional fee max at L5283:
// loss budget + max(liq fee, min_liquidation_abs) must fit under the mm floor).
// Use sites: `liquidation_fee_from_raw_fee` (~L28948, partial below the floor => NonProgress,
// full close clamps up to the floor), called from `liquidation_fee_for_close` by the projection
// (~L764) and by the real liquidation (~L22455).

#[test]
fn min_liq_abs_envelope_bound_at_mm_floor_100() {
    // Default: mm 10% => floor region ends at notional 1_000; loss budget there is 50 (5% move),
    // so the largest admissible floor is 100 - 50 = 50. bound-1 / bound / bound+1:
    let at = |m| Cfg {
        min_liq_abs: m,
        ..Cfg::default()
    };
    assert!(ok(at(49)));
    assert!(ok(at(50)));
    assert!(bad_cfg(at(51)));
    // the mm floor itself and above are refused
    assert!(bad_cfg(at(100)));
    assert!(bad_cfg(at(101)));
}

#[test]
fn min_liq_abs_envelope_bound_tracks_loss_budget() {
    // 1% per-slot move => loss budget at the floor region end is 10 => bound 90.
    let at = |m| Cfg {
        min_liq_abs: m,
        move_bps: 100,
        ..Cfg::default()
    };
    assert!(ok(at(89)));
    assert!(ok(at(90)));
    assert!(bad_cfg(at(91)));
    // 1 bps move: bound is 99, so the mm floor (100) is still never accepted.
    let at = |m| Cfg {
        min_liq_abs: m,
        move_bps: 1,
        ..Cfg::default()
    };
    assert!(ok(at(98)));
    assert!(ok(at(99)));
    assert!(bad_cfg(at(100)));
    assert!(bad_cfg(at(101)));
}

#[test]
fn min_liq_abs_above_fee_cap_is_refused() {
    let at = |m, cap| Cfg {
        min_liq_abs: m,
        fee_cap: cap,
        ..Cfg::default()
    };
    assert!(ok(at(40, 40)));
    assert!(bad_cfg(at(41, 40)));
    assert!(ok(at(0, 0)));
    assert!(bad_cfg(at(1, 0)));
}

/// q = 1 unit at 1_000, capital 101 (IM 100 + 1), long, price walked to `px`.
fn tiny(m: u128, px: u64) -> (World, Result<LiquidationOutcomeV16, V16Error>) {
    let c = Cfg {
        min_liq_abs: m,
        ..Cfg::default()
    };
    let mut w = world(c, 101, POS_SCALE, true);
    w.move_price(px);
    let r = w.liquidate_against_cp();
    (w, r)
}

#[test]
fn min_liq_abs_tiny_position_full_close_charges_floor_at_bound_minus1_bound() {
    // notional ~1_000, raw fee 5. The mm floor (100) leaves no healthy partial anyway, so this
    // pins the fee clamp: the full close pays the floor (49, 50) instead of the raw 5, never refused.
    for (m, fee) in [(49u128, 49u128), (50, 50), (0, 5)] {
        let (mut w, r) = tiny(m, 998);
        let out = r.unwrap();
        assert_eq!(out.closed_q, POS_SCALE, "m={m}");
        assert_eq!(out.fee_charged, fee, "m={m}");
        assert!(w.tgt_flat());
        assert_eq!(w.tgt.capital.get(), 101 - 2 - fee);
        assert_eq!(w.insurance(), fee);
        w.assert_sound();
    }
}

#[test]
fn min_liq_abs_floor_exceeds_the_whole_position_still_closes() {
    // q = 0.04 units => notional 40 at 1_000 (24 at the liquidation price; floor 50). The close is not refused and does
    // not overshoot the position (closed_q == whole leg), but the fee (50) is larger than the
    // position's whole notional: the floor is not limited by the size it is charged on.
    let c = Cfg {
        min_liq_abs: 50,
        ..Cfg::default()
    };
    let q = POS_SCALE / 25;
    let mut w = world(c, 101, q, true);
    w.move_price(600);
    let out = w.liquidate_against_cp().unwrap();
    assert_eq!(out.closed_q, q);
    assert_eq!(out.fee_charged, 50);
    assert!(w.tgt_flat());
    w.assert_sound();
}

#[test]
fn min_liq_abs_partial_below_floor_is_refused_so_leg_closes_fully() {
    // q = 10 units, notional 10_000: every partial chunk's raw fee (<50) is under the floor,
    // so with floor 50 the engine closes the WHOLE leg where, with floor 0, a ~8.7% chunk
    // restores health. Intended by FIX E3 (no min-fee chunking) but it is a full close.
    let run = |m| {
        let c = Cfg {
            min_liq_abs: m,
            ..Cfg::default()
        };
        let mut w = world(c, 1_100, 10 * POS_SCALE, true);
        w.move_price(980);
        let out = w.liquidate_against_cp().unwrap();
        w.assert_sound();
        (out, w.tgt_leg_q(), w.tgt_flat())
    };
    let (o0, left0, flat0) = run(0);
    assert!(!flat0 && left0 > 0 && o0.closed_q < 10 * POS_SCALE);
    assert_eq!(o0.fee_charged, 5);
    let (o50, left50, flat50) = run(50);
    assert!(flat50 && left50 == 0);
    assert_eq!(o50.closed_q, 10 * POS_SCALE);
    assert_eq!(o50.fee_charged, 50);
}

#[test]
fn min_liq_abs_large_position_partial_unaffected_by_floor() {
    // notional 100_000: raw chunk fees are far above 50, so floor 50 changes nothing.
    let run = |m| {
        let c = Cfg {
            min_liq_abs: m,
            ..Cfg::default()
        };
        let mut w = world(c, 11_000, 100 * POS_SCALE, true);
        w.move_price(950);
        let out = w.liquidate_against_cp().unwrap();
        w.assert_sound();
        (out.closed_q, out.fee_charged, w.tgt_leg_q())
    };
    assert_eq!(run(0), run(50));
    assert!(run(50).2 > 0);
}

#[test]
fn min_liq_abs_underwater_tiny_position_charges_no_fee_and_closes() {
    // Loss (101 at px 899) already consumes all capital: no fee can be charged, but the
    // close is not refused and leaves no dust.
    let (mut w, r) = tiny(50, 899);
    let out = r.unwrap();
    assert_eq!(out.closed_q, POS_SCALE);
    assert_eq!(out.fee_charged, 0);
    assert!(w.tgt_flat());
    w.assert_sound();
    // Loss 40 at px 960 leaves capital 61 >= 50: the floor fee is charged in full.
    let (w2, r2) = tiny(50, 960);
    assert_eq!(r2.unwrap().fee_charged, 50);
    assert_eq!(w2.tgt.capital.get(), 11);
}
