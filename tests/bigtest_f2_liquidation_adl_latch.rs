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
use percolator::{
    AutoCrankObservationV16, AutoCrankPlanV16, AutoCrankResultV16, AutoCrankWorkV16, ADL_ONE,
    POS_SCALE, S10_MAX_MOVES_PER_INSTRUCTION,
};

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
    let mut cfg = V16Config::public_user_fund_with_market_slots(
        max_portfolio_assets,
        market_slots,
        0,
        100_000,
    );
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

fn deposit(
    header: &mut Header,
    markets: &mut Markets,
    a: &mut PortfolioAccountV16Account,
    amt: u128,
) {
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
    let price = markets[0]
        .engine
        .asset
        .try_to_runtime()
        .unwrap()
        .effective_price;
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

/// cpbind: bind `who` as asset 0's standing counterparty (wrapper: the vault-LP bind).
fn bind(header: &mut Header, markets: &mut Markets, who: &PortfolioAccountV16Account) {
    let mut market = MarketGroupV16ViewMut::new(header, markets);
    market
        .set_asset_standing_counterparty_not_atomic(0, who.provenance_header.portfolio_account_id)
        .unwrap();
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
    trade(
        &mut header,
        &mut markets,
        &mut trader,
        &mut lp,
        signed_q(LOT),
        PRICE,
    )
    .unwrap();
    trade(
        &mut header,
        &mut markets,
        &mut l2,
        &mut s2,
        signed_q(LOT),
        PRICE,
    )
    .unwrap();
    bind(&mut header, &mut markets, &lp);
    let mut w = World {
        header,
        markets,
        trader,
        lp,
        l2,
        s2,
    };
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
        assert_eq!(
            (asset.a_long, asset.a_short),
            (ADL_ONE, ADL_ONE),
            "nothing ADL'd while walking"
        );
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
                assert_eq!(
                    (o.insurance_used, o.residual_booked, o.explicit_loss),
                    (0, 0, 0)
                );
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
    let p = w.markets[0]
        .engine
        .asset
        .try_to_runtime()
        .unwrap()
        .effective_price;
    trade(
        &mut w.header,
        &mut w.markets,
        &mut fresh_l,
        &mut fresh_s,
        signed_q(POS_SCALE),
        p,
    )
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
    assert!(
        a.a_short < ADL_ONE,
        "the close was booked by scaling the shorts"
    );
    assert_eq!(a.oi_eff_long_q, a.oi_eff_short_q);
    assert!(
        w.header.insurance.get() >= insurance0,
        "no insurance draw: not a bankruptcy"
    );
    assert!(o.closed_q > 0);
    assert_eq!(open_new_short(&mut w), Err(V16Error::AdlReduceOnly));
}

/// The fix: the same liquidation matched against the standing counterparty.
#[test]
fn f2_matched_liquidation_keeps_the_asset_open_and_matches_the_unilateral_outcome() {
    let base = world();
    let mut uni = World {
        header: base.header,
        markets: base.markets.clone(),
        ..base
    };
    let mut mat = World {
        header: base.header,
        markets: base.markets.clone(),
        ..base
    };
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
    assert_eq!(
        a.oi_eff_long_q, au.oi_eff_long_q,
        "same OI removed either way"
    );

    // Conservation: identical total value on both paths. Before the liquidation every account
    // but the trader was settled at the walked mark; the liquidation's own refresh settles the
    // trader's mark loss (LOT * (PRICE - mark)), which is the only change in total value.
    assert_eq!(
        total_value(&mat.header, &mat.all()),
        total_value(&uni.header, &uni.all())
    );
    let mark = base.markets[0]
        .engine
        .asset
        .try_to_runtime()
        .unwrap()
        .effective_price;
    let trader_mark_loss = (LOT * u128::from(PRICE - mark) / POS_SCALE) as i128;
    assert_eq!(
        total_value(&mat.header, &mat.all()),
        value0 - trader_mark_loss
    );
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
    let mut uni = World {
        header: base.header,
        markets: base.markets.clone(),
        ..base
    };
    let mut fb = World {
        header: base.header,
        markets: base.markets.clone(),
        ..base
    };
    // Hand `fb` a counterparty on the SAME side as the trader (l2 is long).
    std::mem::swap(&mut fb.lp, &mut fb.l2);
    bind(&mut fb.header, &mut fb.markets, &fb.lp);
    let ou = uni.liquidate(false).unwrap();
    let of = fb.liquidate(true).unwrap();
    assert_eq!(of, ou);
    let (a, b) = (
        fb.markets[0].engine.asset.try_to_runtime().unwrap(),
        uni.markets[0].engine.asset.try_to_runtime().unwrap(),
    );
    assert_eq!(
        (a.a_long, a.a_short, a.oi_eff_long_q),
        (b.a_long, b.a_short, b.oi_eff_long_q)
    );
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
    trade(
        &mut header,
        &mut markets,
        &mut trader,
        &mut s2,
        signed_q(LOT),
        PRICE,
    )
    .unwrap();
    trade(
        &mut header,
        &mut markets,
        &mut l2,
        &mut lp,
        signed_q(1),
        PRICE,
    )
    .unwrap();
    bind(&mut header, &mut markets, &lp);
    let mut w = World {
        header,
        markets,
        trader,
        lp,
        l2,
        s2,
    };
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
    assert!(
        a.a_short < ADL_ONE,
        "too small to match: quantity ADL as before"
    );
    assert_eq!(
        eff_q(&w.markets, &w.lp),
        1,
        "the small counterparty leg was not touched"
    );
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
    trade(
        &mut header,
        &mut markets,
        &mut lp,
        &mut thin,
        signed_q(LOT),
        PRICE,
    )
    .unwrap();
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
    bind(&mut header, &mut markets, &lp);
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
    assert!(
        o.insurance_used + o.residual_booked + o.explicit_loss > 0,
        "bankrupt: {o:?}"
    );
    let leg = lp.legs[0].try_to_runtime().unwrap();
    // The lp's raw basis was not reduced by a matched close.
    assert!(
        !leg.active || leg.basis_pos_q.unsigned_abs() == lp_eff0,
        "no matched reduction on a bankrupt close"
    );
}

// ---------------------------------------------------------------------------------------------
// Wiring: the permissionless auto-crank (wrapper tag 5) threads the bound counterparty through.
// ---------------------------------------------------------------------------------------------

impl World {
    /// The wrapper's tag-5 call shape: one observation of asset 0 at the committed mark; the
    /// engine's plan selector picks the action. `cp_asset = None` is the plain entry.
    fn auto_crank(&mut self, cp_asset: Option<usize>) -> Result<AutoCrankResultV16, V16Error> {
        let now_slot = self.header.current_slot.get();
        let price = self.markets[0]
            .engine
            .asset
            .try_to_runtime()
            .unwrap()
            .effective_price;
        let observations = [AutoCrankObservationV16 {
            asset_index: 0,
            effective_price: price,
            funding_rate_e9: 0,
        }];
        let work = AutoCrankWorkV16 {
            now_slot,
            observations: &observations,
            resolved_close_fee_rate_per_slot: 0,
        };
        let mut market = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let mut tv = PortfolioV16ViewMut::new(&mut self.trader);
        match cp_asset {
            Some(bound) => {
                let mut cv = PortfolioV16ViewMut::new(&mut self.lp);
                market.permissionless_auto_crank_s10_with_counterparty_not_atomic(
                    &mut tv,
                    &mut cv,
                    bound,
                    work,
                    S10_MAX_MOVES_PER_INSTRUCTION,
                )
            }
            None => market.permissionless_auto_crank_s10_not_atomic(
                &mut tv,
                work,
                S10_MAX_MOVES_PER_INSTRUCTION,
            ),
        }
    }
    /// Crank until the engine's plan is the liquidation (the first step certifies the stale
    /// trader, exactly the wrapper's refresh + F1 follow-on), returning that step's result.
    fn crank_to_liquidation(&mut self, cp_asset: Option<usize>) -> AutoCrankResultV16 {
        for _ in 0..8 {
            let r = self.auto_crank(cp_asset).expect("auto crank");
            if matches!(r.selected, AutoCrankPlanV16::Liquidate { .. }) {
                return r;
            }
        }
        panic!("the plan never selected Liquidate");
    }
    fn a(&self) -> (u128, u128) {
        let a = self.markets[0].engine.asset.try_to_runtime().unwrap();
        (a.a_long, a.a_short)
    }
    fn fork(&self) -> World {
        World {
            header: self.header,
            markets: self.markets.clone(),
            trader: self.trader,
            lp: self.lp,
            l2: self.l2,
            s2: self.s2,
        }
    }
}

/// The plan-selected liquidation through the auto-crank with the counterparty bound to the
/// liquidated asset: same plan, same trader outcome as the plain crank, but no A factor moves.
#[test]
fn f2_auto_crank_with_bound_counterparty_matches_and_keeps_the_asset_open() {
    let base = world();
    let mut uni = base.fork();
    let mut mat = base.fork();
    let ru = uni.crank_to_liquidation(None);
    let rm = mat.crank_to_liquidation(Some(0));
    assert_eq!(ru.selected, AutoCrankPlanV16::Liquidate { asset_index: 0 });
    assert_eq!(rm.selected, ru.selected);
    assert_eq!(rm.outcome, ru.outcome);
    assert_eq!(mat.trader.capital.get(), uni.trader.capital.get());
    assert_eq!(mat.trader.pnl.get(), uni.trader.pnl.get());
    assert_eq!(
        eff_q(&mat.markets, &mat.trader),
        eff_q(&uni.markets, &uni.trader)
    );
    assert_eq!(
        total_value(&mat.header, &mat.all()),
        total_value(&uni.header, &uni.all())
    );
    assert!(uni.a().1 < ADL_ONE, "plain crank: quantity ADL");
    assert_eq!(mat.a(), (ADL_ONE, ADL_ONE), "bound counterparty: matched");
    let closed = eff_q(&base.markets, &base.trader) - eff_q(&mat.markets, &mat.trader);
    assert_eq!(
        eff_q(&mat.markets, &mat.lp),
        eff_q(&base.markets, &base.lp) - closed
    );
    {
        let market = MarketGroupV16ViewMut::new(&mut mat.header, &mut mat.markets);
        market.validate_shape().unwrap();
    }
    open_new_short(&mut mat).expect("asset still open");
}

/// A counterparty the caller bound to a DIFFERENT asset is never consulted: bit-for-bit the
/// plain crank, and the counterparty account is not written.
#[test]
fn f2_auto_crank_counterparty_bound_to_another_asset_is_ignored() {
    let base = world();
    let mut uni = base.fork();
    let mut other = base.fork();
    uni.crank_to_liquidation(None);
    other.crank_to_liquidation(Some(1));
    assert_eq!(other.a(), uni.a());
    assert!(other.a().1 < ADL_ONE);
    assert_eq!(
        bytemuck_eq(&other.lp, &base.lp),
        true,
        "counterparty untouched"
    );
}

/// A crank whose plan is NOT a liquidation (the counterparty itself refreshed) never touches the
/// bound counterparty either.
#[test]
fn f2_auto_crank_refresh_plan_ignores_the_counterparty() {
    let mut w = world();
    // Swap roles: crank the healthy l2 (a Refresh plan) with the lp bound.
    std::mem::swap(&mut w.trader, &mut w.l2);
    let lp0 = w.lp;
    let r = w.auto_crank(Some(0));
    match r {
        Ok(r) => assert!(
            !matches!(r.selected, AutoCrankPlanV16::Liquidate { .. }),
            "healthy account: {r:?}"
        ),
        Err(e) => assert_eq!(e, V16Error::NonProgress),
    }
    assert!(bytemuck_eq(&w.lp, &lp0));
}

/// LIVENESS: a counterparty whose refresh would FAIL (B-stale) must not block the liquidation of
/// somebody else. Before the read-only preflight the counterparty refresh returned BStale and
/// reverted the whole liquidation.
#[test]
fn f2_b_stale_counterparty_falls_back_without_failing_the_liquidation() {
    let base = world();
    let mut uni = base.fork();
    let mut fb = base.fork();
    fb.lp.b_stale_state = 1;
    let lp0 = fb.lp;
    let ou = uni.liquidate(false).unwrap();
    let of = fb
        .liquidate(true)
        .expect("a B-stale counterparty must not revert the liquidation");
    assert_eq!(of, ou);
    assert_eq!(fb.a(), uni.a());
    assert!(fb.a().1 < ADL_ONE, "unilateral fallback");
    assert!(bytemuck_eq(&fb.lp, &lp0), "counterparty untouched");
}

/// A counterparty that fails market validation (here: a lock bit with no lock episode behind it)
/// is not used either. The preflight's own `liquidation_lock` check is a second guard behind
/// `validate_with_market`; this test pins the observable fallback, not which guard fires.
#[test]
fn f2_locked_counterparty_falls_back() {
    let base = world();
    let mut uni = base.fork();
    let mut fb = base.fork();
    fb.lp.liquidation_lock = 1;
    let lp0 = fb.lp;
    let ou = uni.liquidate(false).unwrap();
    let of = fb.liquidate(true).unwrap();
    assert_eq!(of, ou);
    assert_eq!(fb.a(), uni.a());
    assert!(bytemuck_eq(&fb.lp, &lp0));
}

fn bytemuck_eq(a: &PortfolioAccountV16Account, b: &PortfolioAccountV16Account) -> bool {
    let size = core::mem::size_of::<PortfolioAccountV16Account>();
    // SAFETY: plain-old-data account structs, compared byte for byte.
    let (pa, pb) = unsafe {
        (
            core::slice::from_raw_parts(a as *const _ as *const u8, size),
            core::slice::from_raw_parts(b as *const _ as *const u8, size),
        )
    };
    pa == pb
}

// ---------------------------------------------------------------------------------------------
// Regression (local-validator run 2026-11-05, percolator-prog tests/f2_local_validator.rs):
// an already-latched asset and sequential matched liquidations.
// ---------------------------------------------------------------------------------------------

/// A second trader, long against the same lp, so two liquidations can run in one world.
fn world_two_traders() -> (World, PortfolioAccountV16Account) {
    let (mut header, mut markets) = market_fixture_slots(1, PRICE);
    let mut trader = account_fixture(1, 10);
    let mut trader2 = account_fixture(1, 14);
    let mut lp = account_fixture(1, 11);
    let mut l2 = account_fixture(1, 12);
    let mut s2 = account_fixture(1, 13);
    deposit(&mut header, &mut markets, &mut trader, 1_200_000);
    deposit(&mut header, &mut markets, &mut trader2, 1_200_000);
    for a in [&mut lp, &mut l2, &mut s2] {
        deposit(&mut header, &mut markets, a, 400_000_000);
    }
    trade(
        &mut header,
        &mut markets,
        &mut trader,
        &mut lp,
        signed_q(LOT),
        PRICE,
    )
    .unwrap();
    trade(
        &mut header,
        &mut markets,
        &mut trader2,
        &mut lp,
        signed_q(LOT),
        PRICE,
    )
    .unwrap();
    trade(
        &mut header,
        &mut markets,
        &mut l2,
        &mut s2,
        signed_q(LOT),
        PRICE,
    )
    .unwrap();
    bind(&mut header, &mut markets, &lp);
    let mut w = World {
        header,
        markets,
        trader,
        lp,
        l2,
        s2,
    };
    for step in 1..80u64 {
        let p = PRICE * (100 - step) / 100;
        accrue(&mut w.header, &mut w.markets, step + 1, p);
        for i in 1..4 {
            let a = match i {
                1 => &mut w.lp,
                2 => &mut w.l2,
                _ => &mut w.s2,
            };
            refresh(&mut w.header, &mut w.markets, a);
        }
        let mut probe = w.fork();
        match probe.liquidate(false) {
            Err(V16Error::NonProgress) => continue,
            Ok(_) => return (w, trader2),
            Err(e) => panic!("{e:?}"),
        }
    }
    panic!("never liquidatable");
}

fn liquidate_other(
    w: &mut World,
    who: &mut PortfolioAccountV16Account,
    with_lp: bool,
) -> LiquidationOutcomeV16 {
    let mut market = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
    let mut tv = PortfolioV16ViewMut::new(who);
    let req = LiquidationRequestV16 { asset_index: 0 };
    if with_lp {
        let mut cv = PortfolioV16ViewMut::new(&mut w.lp);
        market
            .liquidate_account_against_counterparty_not_atomic(&mut tv, &mut cv, req)
            .unwrap()
    } else {
        market.liquidate_account_not_atomic(&mut tv, req).unwrap()
    }
}

/// Sequential matched liquidations keep A at ADL_ONE every time, and the market stays open after each.
#[test]
fn f2_regression_sequential_matched_liquidations_keep_a_one() {
    let (mut w, mut t2) = world_two_traders();
    let lp0 = eff_q(&w.markets, &w.lp);
    let o1 = w.liquidate(true).unwrap();
    assert_eq!(w.a(), (ADL_ONE, ADL_ONE));
    let o2 = liquidate_other(&mut w, &mut t2, true);
    assert_eq!(w.a(), (ADL_ONE, ADL_ONE));
    assert_eq!(
        eff_q(&w.markets, &w.lp),
        lp0 - o1.closed_q - o2.closed_q,
        "the lp took both closes exactly"
    );
    {
        let market = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
        market.validate_shape().unwrap();
    }
    open_new_short(&mut w).expect("open after two matched liquidations");
}

/// On an asset ALREADY latched by a unilateral close, a matched liquidation still works (it is risk-reducing for both
/// legs, so the #134 gate allows it), does NOT move A any further, and does not lift the latch either: only a side
/// reset (one side flat) or tag 104 does. Pinned so nobody reads "matched" as "unlatches".
#[test]
fn f2_regression_matched_liquidation_on_a_latched_asset_neither_deepens_nor_lifts_the_latch() {
    let (mut w, mut t2) = world_two_traders();
    w.liquidate(false).unwrap();
    let (al, ash) = w.a();
    assert_eq!(al, ADL_ONE);
    assert!(ash < ADL_ONE, "latched by the unilateral close");
    let lp_eff0 = eff_q(&w.markets, &w.lp);
    let o = liquidate_other(&mut w, &mut t2, true);
    assert!(o.closed_q > 0);
    assert_eq!(w.a(), (al, ash), "A did not move");
    let lp_eff1 = eff_q(&w.markets, &w.lp);
    // eff_q rounds up per leg; the matched delta is exact up to that rounding
    assert!(
        lp_eff0 - lp_eff1 >= o.closed_q - 1 && lp_eff0 - lp_eff1 <= o.closed_q + 1,
        "{lp_eff0} -> {lp_eff1}, closed {}",
        o.closed_q
    );
    {
        let market = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
        market.validate_shape().unwrap();
    }
    assert_eq!(
        open_new_short(&mut w),
        Err(V16Error::AdlReduceOnly),
        "still latched: matched closes do not lift it"
    );
}

/// Control for the above: the same second liquidation WITHOUT the counterparty deepens the latch.
#[test]
fn f2_regression_unilateral_second_liquidation_deepens_the_latch() {
    let (mut w, mut t2) = world_two_traders();
    w.liquidate(false).unwrap();
    let ash = w.a().1;
    liquidate_other(&mut w, &mut t2, false);
    assert!(w.a().1 < ash);
}

/// cpbind: the engine, not the caller, decides who may take a matched close. `s2` holds an
/// eligible opposite leg but is NOT the asset's bound counterparty: the matched liquidation must
/// be refused, and s2's position must not move. Before the fix the engine matched against
/// whoever the caller passed, so a caller could force a targeted reduction of any holder.
#[test]
fn cpbind_unbound_counterparty_is_rejected() {
    let base = world();
    let mut w = base.fork();
    let s2_0 = w.s2;
    let r = {
        let mut market = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
        let mut tv = PortfolioV16ViewMut::new(&mut w.trader);
        let mut cv = PortfolioV16ViewMut::new(&mut w.s2);
        market.liquidate_account_against_counterparty_not_atomic(
            &mut tv,
            &mut cv,
            LiquidationRequestV16 { asset_index: 0 },
        )
    };
    assert_eq!(r, Err(V16Error::CounterpartyNotBound));
    assert!(bytemuck_eq(&w.s2, &s2_0), "unbound counterparty was written");
    assert!(bytemuck_eq(&w.trader, &base.trader), "refused before any write");
    assert_eq!(w.header, base.header);
    assert_eq!(w.markets, base.markets);
}

/// cpbind: the bound counterparty still works: matched close, no A move, LP took the close.
#[test]
fn cpbind_bound_counterparty_still_matches() {
    let base = world();
    let mut w = base.fork();
    let lp0 = eff_q(&w.markets, &w.lp);
    let o = w.liquidate(true).expect("bound counterparty");
    assert!(o.closed_q > 0);
    assert_eq!(w.a(), (ADL_ONE, ADL_ONE));
    assert_eq!(eff_q(&w.markets, &w.lp), lp0 - o.closed_q);
}

/// cpbind: no binding at all (zero id) refuses even the account that used to be bound.
#[test]
fn cpbind_no_binding_refuses_every_counterparty() {
    let base = world();
    let mut w = base.fork();
    {
        let mut market = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
        market
            .set_asset_standing_counterparty_not_atomic(0, [0; 32])
            .unwrap();
    }
    let lp0 = w.lp;
    assert_eq!(w.liquidate(true), Err(V16Error::CounterpartyNotBound));
    assert!(bytemuck_eq(&w.lp, &lp0));
    // the plain entry is unaffected
    assert!(w.liquidate(false).is_ok());
}

/// cpbind: the liquidated account can never be its own matched counterparty, even when it is
/// the bound one (a second view of the same id).
#[test]
fn cpbind_bound_account_cannot_be_matched_against_itself() {
    let base = world();
    let mut w = base.fork();
    bind(&mut w.header, &mut w.markets, &base.trader);
    w.lp = w.trader;
    assert_eq!(w.liquidate(true), Err(V16Error::CounterpartyNotBound));
}

/// cpbind: a counterparty from another market group with the bound id is refused.
#[test]
fn cpbind_counterparty_from_another_market_group_is_refused() {
    let base = world();
    let mut w = base.fork();
    w.lp.provenance_header.market_group_id = [9; 32];
    assert_eq!(w.liquidate(true), Err(V16Error::CounterpartyNotBound));
}

/// cpbind: same through the auto-crank (wrapper tag 5): when the plan is the liquidation, a
/// counterparty that is not the bound one fails the step and is not written.
#[test]
fn cpbind_auto_crank_refuses_unbound_counterparty() {
    let base = world();
    let mut w = base.fork();
    std::mem::swap(&mut w.lp, &mut w.s2); // pass s2 in the counterparty position
    let s2_0 = w.lp;
    let mut refused = false;
    for _ in 0..8 {
        match w.auto_crank(Some(0)) {
            Ok(r) => assert!(
                !matches!(r.selected, AutoCrankPlanV16::Liquidate { .. }),
                "liquidated against an unbound counterparty: {r:?}"
            ),
            Err(e) => {
                assert_eq!(e, V16Error::CounterpartyNotBound);
                refused = true;
                break;
            }
        }
    }
    assert!(refused, "the plan never reached the liquidation");
    assert!(bytemuck_eq(&w.lp, &s2_0));
}

/// cpbind: binding rules. Non-zero needs a configured, active asset; clearing always works.
#[test]
fn cpbind_setter_guards() {
    let (mut header, mut markets) = market_fixture_activated(2, 1, PRICE);
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    assert_eq!(
        market.set_asset_standing_counterparty_not_atomic(1, [7; 32]),
        Err(V16Error::InvalidConfig),
        "asset 1 is configured but never activated"
    );
    assert_eq!(
        market.set_asset_standing_counterparty_not_atomic(2, [7; 32]),
        Err(V16Error::InvalidLeg),
        "unconfigured slot"
    );
    market
        .set_asset_standing_counterparty_not_atomic(1, [0; 32])
        .unwrap();
    market
        .set_asset_standing_counterparty_not_atomic(0, [7; 32])
        .unwrap();
    assert_eq!(market.markets[0].engine.standing_counterparty_id.get(), [7; 32]);
    market.validate_shape().unwrap();
    // outside Live a non-zero binding is refused, clearing is not
    market.header.mode = 2; // Recovery
    assert_eq!(
        market.set_asset_standing_counterparty_not_atomic(0, [8; 32]),
        Err(V16Error::LockActive)
    );
    market
        .set_asset_standing_counterparty_not_atomic(0, [0; 32])
        .unwrap();
    assert_eq!(market.markets[0].engine.standing_counterparty_id.get(), [0; 32]);
}
