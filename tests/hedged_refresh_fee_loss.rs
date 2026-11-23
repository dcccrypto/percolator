//! Follow-up to hedged_refresh_order_liquidation.rs (PR #346): what the early liquidation of a
//! solvent hedge costs the trader once a liquidation fee is charged.
//!
//! #346 ran with `liquidation_fee_bps = 0` because a non-zero fee gave `InvalidConfig`. The cause is
//! `min_nonzero_mm_req` (see `fee_config_*` tests below and the file:line notes there): with the
//! floor at 1 the solvency envelope cannot hold in the floor region. Floor 2 (and IM 3) is enough
//! for fees up to 243 bps at MM 5%.
//!
//! Same hedge as #346: account `a` long asset 0 and short asset 1, 100 units each, price 1_000,
//! capital 20_000 (the thinnest it can open with), both prices fall to `px`. Order "account-first"
//! cranks `a` before its counterparty `b` and liquidates if the engine says so; "counterparty-first"
//! refreshes `b` first. Trader loss = (capital + pnl + fee_credits) before minus after, in atoms, and as % of the
//! 20_000 starting equity. This was the evidence for the spec.md:266 vs spec.md:899 decision.
//!
//! Haircut-order fix B (docs/fixes/haircut-order-B.md): spec.md:266 kept, liquidation counts the
//! in-flight backing, so the account-first loss is now 0 at every step. The tables below keep the
//! measured pre-fix numbers (`*_BEFORE_FIX`) for the record and assert the fixed ones.
//! Test names start with `hedged_fee_loss_`.
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
const THIN: u128 = 20_000;
/// realistic liquidation fee: 0.50% of closed notional
const FEE_BPS: u64 = 50;
/// smallest floor that validates with a non-zero fee
const MM_FLOOR: u128 = 2;

fn config(fee_bps: u64, min_mm_req: u128) -> V16Config {
    let mut cfg = V16Config::public_user_fund_with_market_slots(2, 2, 0, 10);
    cfg.maintenance_margin_bps = 500;
    cfg.initial_margin_bps = 1_000;
    cfg.max_price_move_bps_per_slot = 100;
    cfg.liquidation_fee_bps = fee_bps;
    cfg.liquidation_fee_cap = 1_000_000;
    cfg.min_nonzero_mm_req = min_mm_req;
    cfg.min_nonzero_im_req = min_mm_req + 1;
    cfg
}

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
    fn with(cfg: V16Config) -> Result<Self, V16Error> {
        let n = 2usize;
        let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, n as u32, 0)?;
        let mut markets: Vec<Market<u64>> =
            (0..n as u64).map(|i| Market::new(i, EngineAssetSlotV16Account::default())).collect();
        for i in 0..n {
            header
                .activate_empty_asset_slot_not_atomic(i as u32, &mut markets[i].engine, P0, 1 + 2 * i as u64)
                .unwrap();
        }
        Ok(W { header, markets, slot: 2 * n as u64 - 1, n })
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
    a.capital.get() as i128 + a.pnl.get() + a.fee_credits.get()
}

fn hedged(cfg: V16Config, px: u64) -> (W, PortfolioAccountV16Account, PortfolioAccountV16Account) {
    let mut w = W::with(cfg).expect("config");
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, THIN);
    w.deposit(&mut b, BIG);
    w.trade(0, &mut a, &mut b, Q as i128);
    w.trade(1, &mut b, &mut a, Q as i128);
    w.move_both(px);
    (w, a, b)
}

/// One measured run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Run {
    liquidated: bool,
    /// starting 20_000 minus trader value (capital + pnl + fee_credits) after both sides refreshed
    loss_atoms: i128,
    /// insurance fund increase (where the fee goes)
    insurance_gain: u128,
    long_left: i128,
    short_left: i128,
}

fn run(fee_bps: u64, px: u64, b_first: bool) -> Run {
    let (mut w, mut a, mut b) = hedged(config(fee_bps, MM_FLOOR), px);
    let ins0 = w.header.insurance.get();
    if b_first {
        w.refresh(&mut b);
    }
    let cert = w.refresh(&mut a);
    let mut liquidated = false;
    if cert.certified_liq_deficit > 0 {
        liquidated = w.liquidate(&mut a, 0).is_ok() || w.liquidate(&mut a, 1).is_ok();
    }
    if !b_first {
        w.refresh(&mut b);
    }
    w.refresh(&mut a);
    w.sound(&mut [&mut a, &mut b]);
    Run {
        liquidated,
        loss_atoms: THIN as i128 - value(&a),
        insurance_gain: w.header.insurance.get() - ins0,
        long_left: pos(&a, 0),
        short_left: pos(&a, 1),
    }
}

const STEPS: [u64; 11] = [889, 888, 880, 870, 860, 850, 840, 830, 820, 810, 800];

// ---------------------------------------------------------------------------------------------
// Which config fields must change together
// ---------------------------------------------------------------------------------------------

fn validates(fee_bps: u64, cap: u128, min_mm_req: u128) -> bool {
    let mut cfg = config(fee_bps, min_mm_req);
    cfg.liquidation_fee_cap = cap;
    cfg.validate_public_user_fund().is_ok()
}

/// BEFORE: the #346 setup (floor 1, IM 2, cap 1_000_000) plus any non-zero fee is rejected, at
/// every fee and every cap except cap 0 (which makes the fee a no-op).
#[test]
fn hedged_fee_loss_fee_config_floor_one_is_rejected() {
    for fee in [1u64, 10, 50, 100, 300] {
        for cap in [1u128, 1_000, 1_000_000] {
            assert!(!validates(fee, cap, 1), "fee {fee} cap {cap}");
        }
    }
    // fee 0 is fine at floor 1, and so is a non-zero fee whose cap is 0 (fee always 0)
    assert!(validates(0, 1_000_000, 1));
    assert!(validates(50, 0, 1));
    assert_eq!(W::with(config(FEE_BPS, 1)).err(), Some(V16Error::InvalidConfig));
}

/// Why: `validate_exact_solvency_envelope_with` (src/v16.rs:5468) requires, at every notional,
/// loss budget + worst-case liquidation fee <= maintenance requirement
/// (`solvency_envelope_total_for_notional` src/v16.rs:5263, compared to
/// `maintenance_requirement_for_notional` src/v16.rs:5290 in
/// `solvency_envelope_holds_for_notional` src/v16.rs:5296). Both terms are rounded up, so
/// in the floor region (notionals up to floor*10_000/MM = 20 here, checked at src/v16.rs:5536-5549)
/// they are 1 + 1 = 2, while `min_nonzero_mm_req = 1` makes the requirement 1. With fee 0 (or fee
/// cap 0) the fee term is 0, so floor 1 passes. (The fast path at src/v16.rs:5433 needs MM 100%, so
/// it does not apply to this config.)
/// Fix: `min_nonzero_mm_req >= 2`, which forces `min_nonzero_im_req >= 3`
/// (shape check src/v16.rs:5643, `mm_req >= im_req` rejected).
#[test]
fn hedged_fee_loss_fee_config_needs_mm_floor_two_and_im_floor_three() {
    for fee in [1u64, 10, 50, 100, 243] {
        assert!(validates(fee, 1_000_000, 2), "fee {fee}");
        assert!(!validates(fee, 1_000_000, 1), "fee {fee}");
    }
    // the shape check wants im > mm
    let mut cfg = config(FEE_BPS, 2);
    cfg.min_nonzero_im_req = 2;
    assert_eq!(cfg.validate_public_user_fund(), Err(V16Error::InvalidConfig));
    cfg.min_nonzero_im_req = 3;
    assert_eq!(cfg.validate_public_user_fund(), Ok(()));
    // the margin bps are not the blocker: floor 1 fails at MM 3%, 5%, 7%, 10%
    for mm in [300u64, 500, 700, 1_000] {
        let mut c = config(FEE_BPS, 1);
        c.maintenance_margin_bps = mm;
        c.initial_margin_bps = 2_000;
        assert!(c.validate_public_user_fund().is_err(), "mm {mm}");
    }
}

/// Floor 2 only covers fees up to a point. At MM 5% the floor region ends at notional 40, the
/// worst-case liquidation notional is 41, and the fee term ceil(41 * fee / 10_000) stays 1 up to
/// 243 bps but becomes 2 at 244 bps (1 + 2 = 3 > floor 2). Measured ceiling at floor 2: 243 bps.
/// A wider MM shrinks the floor region (MM 10%: end 20, fee rounds to 1), so it passes at 400 bps.
/// Larger fees need a larger `min_nonzero_mm_req`.
#[test]
fn hedged_fee_loss_fee_config_fee_ceiling_follows_maintenance_margin() {
    assert!(validates(243, 1_000_000, 2));
    assert!(!validates(244, 1_000_000, 2));
    // a wider maintenance margin lifts the ceiling
    let mut c = config(400, 2);
    c.maintenance_margin_bps = 1_000;
    c.initial_margin_bps = 2_000;
    assert_eq!(c.validate_public_user_fund(), Ok(()));
}

// ---------------------------------------------------------------------------------------------
// Trader loss, account-first vs counterparty-first
// ---------------------------------------------------------------------------------------------

/// (px, trader loss in atoms) for account-first, 50 bps, measured BEFORE fix B (insurance gain
/// equalled it; peak 2.1% of equity). Kept for the record; not asserted.
#[allow(dead_code)]
const TABLE_50_BEFORE_FIX: [(u64, i128); 11] = [
    (889, 0), (888, 9), (880, 89), (870, 189), (860, 289), (850, 389),
    (840, 420), (830, 415), (820, 410), (810, 405), (800, 0),
];
/// Same at 100 bps, BEFORE fix B (peak 4.25%).
#[allow(dead_code)]
const TABLE_100_BEFORE_FIX: [(u64, i128); 11] = [
    (889, 0), (888, 21), (880, 201), (870, 426), (860, 651), (850, 850),
    (840, 840), (830, 830), (820, 820), (810, 810), (800, 0),
];
/// After fix B: account-first costs nothing at any step, same as counterparty-first.
const TABLE_50: [(u64, i128); 11] = [
    (889, 0), (888, 0), (880, 0), (870, 0), (860, 0), (850, 0),
    (840, 0), (830, 0), (820, 0), (810, 0), (800, 0),
];
const TABLE_100: [(u64, i128); 11] = TABLE_50;

#[test]
fn hedged_fee_loss_account_first_loss_table_50_bps() {
    for (px, loss) in TABLE_50 {
        let r = run(50, px, false);
        assert_eq!(r.loss_atoms, loss, "px {px}");
        assert_eq!(r.insurance_gain as i128, loss, "px {px}: no fee charged");
        assert!(!r.liquidated, "px {px}: fix B, was liquidated for px <= 888");
        assert_eq!((r.long_left, r.short_left), (Q as i128, -(Q as i128)), "px {px}");
    }
}

#[test]
fn hedged_fee_loss_account_first_loss_table_100_bps() {
    for (px, loss) in TABLE_100 {
        let r = run(100, px, false);
        assert_eq!(r.loss_atoms, loss, "px {px}");
        assert_eq!(r.insurance_gain as i128, loss, "px {px}");
        assert!(!r.liquidated, "px {px}: fix B, was liquidated for px <= 888");
    }
}

#[test]
fn hedged_fee_loss_counterparty_first_costs_nothing_at_any_step() {
    for fee in [50u64, 100, 243] {
        for px in STEPS {
            let r = run(fee, px, true);
            assert!(!r.liquidated, "fee {fee} px {px}");
            assert_eq!((r.loss_atoms, r.insurance_gain), (0, 0), "fee {fee} px {px}");
            assert_eq!((r.long_left, r.short_left), (Q as i128, -(Q as i128)));
        }
    }
}

/// Loss as % of the 20_000 starting equity, in hundredths of a percent (integer). Before fix B it
/// peaked at 2.1% at 50 bps (px 840) and 4.25% at 100 bps (px 850); after it, 0 at every fee.
#[test]
fn hedged_fee_loss_peak_loss_percent_of_equity() {
    let peak = |fee| STEPS.iter().map(|px| run(fee, *px, false).loss_atoms).max().unwrap();
    assert_eq!(TABLE_50_BEFORE_FIX.iter().map(|r| r.1).max().unwrap() * 10_000 / THIN as i128, 210);
    assert_eq!(TABLE_100_BEFORE_FIX.iter().map(|r| r.1).max().unwrap() * 10_000 / THIN as i128, 425);
    for fee in [50u64, 100, 243] {
        assert_eq!(peak(fee), 0, "fee {fee}");
    }
}

/// At px 800 the loss eats all 20_000 capital before `a` is cranked. Before fix B the early
/// liquidation closed the whole long leg and charged 0 (nothing to take the fee from). After it,
/// the whole gain is in flight, nothing is liquidated and nothing is charged.
#[test]
fn hedged_fee_loss_at_the_loss_cap_the_fee_is_not_collected() {
    let r = run(50, 800, false);
    assert!(!r.liquidated);
    assert_eq!((r.loss_atoms, r.insurance_gain, r.long_left), (0, 0, Q as i128));
}

/// Also: the fee is only charged if account-first. With fee 0 the account-first run is already
/// liquidated (see #346); the fee is what turns that into a loss.
#[test]
fn hedged_fee_loss_zero_fee_has_zero_loss_in_both_orders() {
    for px in STEPS {
        let af = run(0, px, false);
        assert_eq!(af.loss_atoms, 0, "px {px}");
        assert!(!af.liquidated, "px {px}: fix B (was liquidated at fee 0 too, #346)");
        assert_eq!(run(0, px, true).loss_atoms, 0, "px {px}");
    }
}
