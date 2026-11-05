//! bigtest-2026-11-05 finding 2: an ordinary (non-bankrupt) liquidation latches the asset into
//! ADL reduce-only until one side's effective OI reaches zero.
//!
//! Why: `liquidate_account_not_atomic` closes the liquidated leg UNILATERALLY (`reduce_position`),
//! so the opposite side's effective OI is scaled down through its `A` factor (quantity ADL).
//! Upstream 6ae709e0 / aeyakovenko/percolator#134 (the #131 fresh-basis-reissue LoF fix, spec:
//! "While either side has A_side != ADL_ONE, trades may reduce matched risk but MUST NOT attach,
//! flip, or enlarge a leg") then refuses every open on the asset, and `A` returns to `ADL_ONE`
//! only through a side reset at zero effective OI. Upstream has the same latch; on this fork the
//! vault LP is the standing opposite side, so "until a side is flat" means "until every user is
//! out" (or tag 104 force-closes a whole side after 9,000 slots).
//!
//! Proposed fix: `liquidate_account_against_counterparty_not_atomic` books a CLEAN close against a
//! caller-bound counterparty (wrapper: the market's vault-LP portfolio) as a matched reduction of
//! both legs, so no `A` factor moves; everything else falls back to the unilateral path.

use percolator::{
    v16_domain_count_for_market_slots, EngineAssetSlotV16Account, LiquidationOutcomeV16,
    LiquidationRequestV16, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    PermissionlessCrankActionV16, PermissionlessCrankRequestV16, PortfolioAccountV16Account,
    PortfolioV16ViewMut, ProvenanceHeaderV16, ProvenanceHeaderV16Account, TradeRequestV16,
    V16Config, V16Error, V16PodU64,
};
use percolator::{ADL_ONE, POS_SCALE};


fn ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    ([1; 32], [2; 32], [3; 32])
}

fn market_fixture_slots(
    market_slots: u32,
    init_price: u64,
) -> (MarketGroupV16HeaderAccount, Vec<Market<u64>>) {
    market_fixture_activated(market_slots, market_slots, init_price)
}

/// `market_slots` configured, only the first `activated` of them activated (the deployed
/// slabs are 14 configured slots with only asset 0 ever activated).
fn market_fixture_activated(
    market_slots: u32,
    activated: u32,
    init_price: u64,
) -> (MarketGroupV16HeaderAccount, Vec<Market<u64>>) {
    let (market_id, _, _) = ids();
    let max_portfolio_assets =
        market_slots.min(percolator::V16_MAX_PORTFOLIO_ASSETS_N as u32) as u16;
    // The config of tests/v22_r1_multi_claimant.rs (IMR 10% / MMR 5%, a 2% per-slot move cap
    // under a 2-slot accrual window, so the solvency envelope validates). The shared p2b
    // fixture is IMR = MMR = 100%, under which a long can never become liquidatable before
    // the price reaches zero.
    let mut cfg =
        V16Config::public_user_fund_with_market_slots(max_portfolio_assets, market_slots, 0, 100_000);
    cfg.max_abs_funding_e9_per_slot = 10_000;
    cfg.max_price_move_bps_per_slot = 200;
    cfg.initial_margin_bps = 1_000;
    cfg.maintenance_margin_bps = 500;
    cfg.max_accrual_dt_slots = 2;
    cfg.min_funding_lifetime_slots = 2;
    // No liquidation fee: this envelope validates no non-zero fee cap. The fee is charged before
    // the matched/unilateral branch and is identical on both paths by construction.
    let mut header =
        MarketGroupV16HeaderAccount::new_dynamic(market_id, cfg, market_slots, 0).unwrap();
    let mut markets = (0..market_slots)
        .map(|i| Market::new(i as u64, EngineAssetSlotV16Account::default()))
        .collect::<Vec<_>>();
    for (i, market) in markets.iter_mut().enumerate().take(activated as usize) {
        header
            .activate_empty_asset_slot_not_atomic(
                i as u32,
                &mut market.engine,
                init_price,
                (i + 1) as u64,
            )
            .unwrap();
    }
    {
        let view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        view.validate_shape().unwrap();
    }
    (header, markets)
}

fn account_fixture(market_slots: u32, account_seed: u8) -> PortfolioAccountV16Account {
    let (market_id, _, owner) = ids();
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        market_id,
        [account_seed; 32],
        owner,
    ));
    let _ = v16_domain_count_for_market_slots(market_slots).unwrap();
    let mut account = PortfolioAccountV16Account::default();
    account.init_empty_in_place(header).unwrap();
    account
}

fn signed_q(q: u128) -> i128 {
    i128::try_from(q).unwrap()
}

const PRICE: u64 = 1_000_000;
const LOT: u128 = 3 * POS_SCALE;

type Header = MarketGroupV16HeaderAccount;
type Markets = Vec<Market<u64>>;

fn trade(
    header: &mut Header,
    markets: &mut Markets,
    long: &mut PortfolioAccountV16Account,
    short: &mut PortfolioAccountV16Account,
    size_q: i128,
    price: u64,
) -> Result<(), V16Error> {
    let mut market = MarketGroupV16ViewMut::new(header, markets);
    let mut lv = PortfolioV16ViewMut::new(long);
    let mut sv = PortfolioV16ViewMut::new(short);
    market
        .execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut lv,
            &mut sv,
            TradeRequestV16 {
                asset_index: 0,
                size_q,
                exec_price: price,
                fee_bps: 0,
            },
            true,
        )
        .map(|_| ())
}

fn deposit(header: &mut Header, markets: &mut Markets, a: &mut PortfolioAccountV16Account, amt: u128) {
    let mut market = MarketGroupV16ViewMut::new(header, markets);
    let mut v = PortfolioV16ViewMut::new(a);
    market.deposit_not_atomic(&mut v, amt).unwrap();
}

fn accrue(header: &mut Header, markets: &mut Markets, slot: u64, price: u64) {
    let mut market = MarketGroupV16ViewMut::new(header, markets);
    market
        .accrue_asset_to_not_atomic(0, slot, price, 0, true)
        .unwrap();
    market.markets[0].engine.asset.raw_oracle_target_price = V16PodU64::new(price);
}

/// wrapper tag 5 PermissionlessCrank / Refresh, repeated until the account is no longer
/// b-stale (the keeper does exactly this).
fn refresh(header: &mut Header, markets: &mut Markets, a: &mut PortfolioAccountV16Account) {
    let now_slot = header.current_slot.get();
    let price = markets[0].engine.asset.try_to_runtime().unwrap().effective_price;
    for _ in 0..64 {
        let mut market = MarketGroupV16ViewMut::new(header, markets);
        let mut v = PortfolioV16ViewMut::new(a);
        market
            .permissionless_crank_not_atomic(
                &mut v,
                PermissionlessCrankRequestV16 {
                    now_slot,
                    asset_index: 0,
                    effective_price: price,
                    funding_rate_e9: 0,
                    action: PermissionlessCrankActionV16::Refresh,
                },
            )
            .expect("permissionless Refresh crank");
        if a.b_stale_state == 0 && a.stale_state == 0 {
            break;
        }
    }
}

fn total_value(header: &Header, accounts: &[&PortfolioAccountV16Account]) -> i128 {
    let mut v = header.insurance.get() as i128;
    for a in accounts {
        v += a.capital.get() as i128 + a.pnl.get();
    }
    v
}

fn eff_q(markets: &Markets, a: &PortfolioAccountV16Account) -> u128 {
    let asset = markets[0].engine.asset.try_to_runtime().unwrap();
    let leg = a.legs[0].try_to_runtime().unwrap();
    if !leg.active {
        return 0;
    }
    let a_now = match leg.side {
        percolator::SideV16::Long => asset.a_long,
        percolator::SideV16::Short => asset.a_short,
    };
    (leg.basis_pos_q.unsigned_abs() * a_now).div_ceil(leg.a_basis)
}

struct World {
    header: Header,
    markets: Markets,
    /// the ~2.5x long that gets liquidated
    trader: PortfolioAccountV16Account,
    /// the standing counterparty (the vault LP's role): short against `trader`
    lp: PortfolioAccountV16Account,
    /// an unrelated pair, so the short side has a holder besides the LP
    l2: PortfolioAccountV16Account,
    s2: PortfolioAccountV16Account,
}

impl World {
    fn all(&self) -> [&PortfolioAccountV16Account; 4] {
        [&self.trader, &self.lp, &self.l2, &self.s2]
    }
    fn liquidate(&mut self, with_lp: bool) -> Result<LiquidationOutcomeV16, V16Error> {
        let mut market = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let mut tv = PortfolioV16ViewMut::new(&mut self.trader);
        let req = LiquidationRequestV16 { asset_index: 0 };
        if with_lp {
            let mut cv = PortfolioV16ViewMut::new(&mut self.lp);
            market.liquidate_account_against_counterparty_not_atomic(&mut tv, &mut cv, req)
        } else {
            market.liquidate_account_not_atomic(&mut tv, req)
        }
    }
}

/// Walk the price down until the trader is liquidatable but NOT bankrupt (a clean partial
/// close: no insurance, no residual), exactly the bigtest probe shape.
fn world() -> World {
    let (mut header, mut markets) = market_fixture_slots(1, PRICE);
    let mut trader = account_fixture(1, 10);
    let mut lp = account_fixture(1, 11);
    let mut l2 = account_fixture(1, 12);
    let mut s2 = account_fixture(1, 13);
    deposit(&mut header, &mut markets, &mut trader, 1_200_000);
    for a in [&mut lp, &mut l2, &mut s2] {
        deposit(&mut header, &mut markets, a, 400_000_000);
    }
    trade(&mut header, &mut markets, &mut trader, &mut lp, signed_q(LOT), PRICE).unwrap();
    trade(&mut header, &mut markets, &mut l2, &mut s2, signed_q(LOT), PRICE).unwrap();
    let mut w = World { header, markets, trader, lp, l2, s2 };
    for step in 1..80u64 {
        let p = PRICE * (100 - step) / 100;
        accrue(&mut w.header, &mut w.markets, step + 1, p);
        // The trader is NOT refreshed here: a Refresh crank on a liquidatable account is
        // refused; the liquidation refreshes it itself.
        for i in 1..4 {
            let a = match i {
                1 => &mut w.lp,
                2 => &mut w.l2,
                _ => &mut w.s2,
            };
            refresh(&mut w.header, &mut w.markets, a);
        }
        let asset = w.markets[0].engine.asset.try_to_runtime().unwrap();
        assert_eq!((asset.a_long, asset.a_short), (ADL_ONE, ADL_ONE), "nothing ADL'd while walking");
        let mut probe = World {
            header: w.header,
            markets: w.markets.clone(),
            trader: w.trader,
            lp: w.lp,
            l2: w.l2,
            s2: w.s2,
        };
        match probe.liquidate(false) {
            Err(V16Error::NonProgress) => continue,
            Ok(o) => {
                assert_eq!((o.insurance_used, o.residual_booked, o.explicit_loss), (0, 0, 0));
                assert!(o.closed_q > 0 && o.closed_q < LOT, "a partial close: {o:?}");
                println!("liquidatable (not bankrupt) at price {p} (step {step}): {o:?}");
                return w;
            }
            Err(e) => panic!("unexpected liquidation result at {p}: {e:?}"),
        }
    }
    panic!("never became liquidatable");
}

fn open_new_short(w: &mut World) -> Result<(), V16Error> {
    let mut fresh_l = account_fixture(1, 30);
    let mut fresh_s = account_fixture(1, 31);
    deposit(&mut w.header, &mut w.markets, &mut fresh_l, 100_000_000);
    deposit(&mut w.header, &mut w.markets, &mut fresh_s, 100_000_000);
    let p = w.markets[0].engine.asset.try_to_runtime().unwrap().effective_price;
    trade(&mut w.header, &mut w.markets, &mut fresh_l, &mut fresh_s, signed_q(POS_SCALE), p)
}

/// RED on today's path (documents the finding): one clean partial liquidation leaves
/// `a_short < ADL_ONE`, and then no one can open on the asset.
#[test]
fn f2_unilateral_liquidation_latches_the_asset_reduce_only() {
    let mut w = world();
    let insurance0 = w.header.insurance.get();
    let o = w.liquidate(false).unwrap();
    let a = w.markets[0].engine.asset.try_to_runtime().unwrap();
    assert_eq!(a.a_long, ADL_ONE);
    assert!(a.a_short < ADL_ONE, "the close was booked by scaling the shorts");
    assert_eq!(a.oi_eff_long_q, a.oi_eff_short_q);
    assert!(w.header.insurance.get() >= insurance0, "no insurance draw: not a bankruptcy");
    assert!(o.closed_q > 0);
    assert_eq!(open_new_short(&mut w), Err(V16Error::AdlReduceOnly));
}

/// The fix: the same liquidation matched against the standing counterparty.
#[test]
fn f2_matched_liquidation_keeps_the_asset_open_and_matches_the_unilateral_outcome() {
    let base = world();
    let mut uni = World { header: base.header, markets: base.markets.clone(), ..base };
    let mut mat = World { header: base.header, markets: base.markets.clone(), ..base };
    let value0 = total_value(&base.header, &base.all());
    let vault0 = base.header.vault.get();
    let lp_eff0 = eff_q(&base.markets, &base.lp);
    let s2_eff0 = eff_q(&base.markets, &base.s2);

    let ou = uni.liquidate(false).unwrap();
    let om = mat.liquidate(true).unwrap();

    // Same liquidation: same size, same fee, same accounting for the liquidated trader.
    assert_eq!(om, ou);
    assert_eq!(mat.trader.capital.get(), uni.trader.capital.get());
    assert_eq!(mat.trader.pnl.get(), uni.trader.pnl.get());
    assert_eq!(mat.header.insurance.get(), uni.header.insurance.get());
    assert_eq!(mat.header.vault.get(), vault0);

    // No A factor moved; OI stays matched; the LP's leg took exactly the close; s2 untouched.
    let a = mat.markets[0].engine.asset.try_to_runtime().unwrap();
    assert_eq!((a.a_long, a.a_short), (ADL_ONE, ADL_ONE));
    assert_eq!(a.oi_eff_long_q, a.oi_eff_short_q);
    assert_eq!(eff_q(&mat.markets, &mat.lp), lp_eff0 - om.closed_q);
    assert_eq!(eff_q(&mat.markets, &mat.s2), s2_eff0);
    let au = uni.markets[0].engine.asset.try_to_runtime().unwrap();
    assert_eq!(a.oi_eff_long_q, au.oi_eff_long_q, "same OI removed either way");

    // Conservation: identical total value on both paths. Before the liquidation every account
    // but the trader was settled at the walked mark; the liquidation's own refresh settles the
    // trader's mark loss (LOT * (PRICE - mark)), which is the only change in total value.
    assert_eq!(total_value(&mat.header, &mat.all()), total_value(&uni.header, &uni.all()));
    let mark = base.markets[0].engine.asset.try_to_runtime().unwrap().effective_price;
    let trader_mark_loss = (LOT * u128::from(PRICE - mark) / POS_SCALE) as i128;
    assert_eq!(total_value(&mat.header, &mat.all()), value0 - trader_mark_loss);
    {
        let market = MarketGroupV16ViewMut::new(&mut mat.header, &mut mat.markets);
        market.validate_shape().unwrap();
    }

    // The market stays open: a fresh short opens.
    open_new_short(&mut mat).expect("no ADL latch after a matched liquidation");
    assert_eq!(open_new_short(&mut uni), Err(V16Error::AdlReduceOnly));
}

/// A counterparty that cannot take the close (no opposite leg) changes nothing: the unilateral
/// path runs, bit for bit.
#[test]
fn f2_counterparty_without_an_opposite_leg_falls_back_to_the_unilateral_close() {
    let base = world();
    let mut uni = World { header: base.header, markets: base.markets.clone(), ..base };
    let mut fb = World { header: base.header, markets: base.markets.clone(), ..base };
    // Hand `fb` a counterparty on the SAME side as the trader (l2 is long).
    std::mem::swap(&mut fb.lp, &mut fb.l2);
    let ou = uni.liquidate(false).unwrap();
    let of = fb.liquidate(true).unwrap();
    assert_eq!(of, ou);
    let (a, b) = (
        fb.markets[0].engine.asset.try_to_runtime().unwrap(),
        uni.markets[0].engine.asset.try_to_runtime().unwrap(),
    );
    assert_eq!((a.a_long, a.a_short, a.oi_eff_long_q), (b.a_long, b.a_short, b.oi_eff_long_q));
    assert!(a.a_short < ADL_ONE);
}

/// A counterparty whose opposite leg is SMALLER than the close falls back too (no partial match).
#[test]
fn f2_counterparty_too_small_falls_back_to_the_unilateral_close() {
    let (mut header, mut markets) = market_fixture_slots(1, PRICE);
    let mut trader = account_fixture(1, 10);
    let mut lp = account_fixture(1, 11);
    let mut l2 = account_fixture(1, 12);
    let mut s2 = account_fixture(1, 13);
    deposit(&mut header, &mut markets, &mut trader, 1_200_000);
    for a in [&mut lp, &mut l2, &mut s2] {
        deposit(&mut header, &mut markets, a, 400_000_000);
    }
    // The trader's short side is s2 (LOT); lp holds only one unit short against l2.
    trade(&mut header, &mut markets, &mut trader, &mut s2, signed_q(LOT), PRICE).unwrap();
    trade(&mut header, &mut markets, &mut l2, &mut lp, signed_q(1), PRICE).unwrap();
    let mut w = World { header, markets, trader, lp, l2, s2 };
    let mut closed = None;
    for step in 1..80u64 {
        let p = PRICE * (100 - step) / 100;
        accrue(&mut w.header, &mut w.markets, step + 1, p);
        refresh(&mut w.header, &mut w.markets, &mut w.lp);
        refresh(&mut w.header, &mut w.markets, &mut w.s2);
        match w.liquidate(true) {
            Err(V16Error::NonProgress) => continue,
            Ok(o) => {
                closed = Some(o.closed_q);
                break;
            }
            Err(e) => panic!("{e:?}"),
        }
    }
    let closed = closed.expect("liquidated");
    assert!(closed > 1);
    let a = w.markets[0].engine.asset.try_to_runtime().unwrap();
    assert!(a.a_short < ADL_ONE, "too small to match: quantity ADL as before");
    assert_eq!(eff_q(&w.markets, &w.lp), 1, "the small counterparty leg was not touched");
}

/// A BANKRUPT close keeps the unilateral path even with a counterparty: insurance / B booking
/// and the residual ledger are untouched by this change.
#[test]
fn f2_bankrupt_close_keeps_the_unilateral_path() {
    let (mut header, mut markets) = market_fixture_slots(1, PRICE);
    let mut thin = account_fixture(1, 20);
    let mut lp = account_fixture(1, 21);
    deposit(&mut header, &mut markets, &mut thin, 3_250_000);
    deposit(&mut header, &mut markets, &mut lp, 400_000_000);
    // thin is SHORT against the lp, then the price runs far up in one step.
    trade(&mut header, &mut markets, &mut lp, &mut thin, signed_q(LOT), PRICE).unwrap();
    // Walk the price up 2% a slot to +180% without liquidating on the way (no keeper).
    let mut p = PRICE;
    let mut slot = 1;
    while p < PRICE + 1_800_000 {
        p = (p * 102 / 100).min(PRICE + 1_800_000);
        slot += 1;
        accrue(&mut header, &mut markets, slot, p);
        refresh(&mut header, &mut markets, &mut lp);
    }
    let lp_eff0 = eff_q(&markets, &lp);
    let o = {
        let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut tv = PortfolioV16ViewMut::new(&mut thin);
        let mut cv = PortfolioV16ViewMut::new(&mut lp);
        market
            .liquidate_account_against_counterparty_not_atomic(
                &mut tv,
                &mut cv,
                LiquidationRequestV16 { asset_index: 0 },
            )
            .unwrap()
    };
    assert!(o.insurance_used + o.residual_booked + o.explicit_loss > 0, "bankrupt: {o:?}");
    let leg = lp.legs[0].try_to_runtime().unwrap();
    // The lp's raw basis was not reduced by a matched close.
    assert!(!leg.active || leg.basis_pos_q.unsigned_abs() == lp_eff0, "no matched reduction on a bankrupt close");
}
