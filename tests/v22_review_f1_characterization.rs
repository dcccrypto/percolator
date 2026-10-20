//! Kani review 2026-10-14, lane engine-f1f3: native CHARACTERIZATION of ledger finding F1
//! (v22-market-simulations-2026-10-14.md §3). Not a safety proof; it pins today's behaviour so a
//! fix (keeper `[refresh T, crank T]` or an engine refresh-continues-into-liquidate change) shows
//! up as a deliberate test edit.
//!
//! F1 at engine level: the auto-crank classifier sets `liquidatable` only on a CURRENT health
//! certificate (`build_actionable_summary_and_selected_assets`: `live && cert_current &&
//! certified_liq_deficit != 0 && has_open_risk && ...`). Any accrual that moves the price bumps
//! `oracle_epoch`, so an underwater account whose last certificate predates that accrual
//! classifies as `stale`, the selector returns `RefreshAccount`, and the Refresh dispatch returns
//! `AccountCurrent` without continuing into `Liquidate`. Only a SECOND crank on the same account
//! with no intervening price move (same transaction on chain) sees the current deficit and
//! liquidates. With one crank per account per round and a price push before it every round, the
//! account is never liquidated.

use percolator::{
    AssetStateV16Account, AutoCrankObservationV16, AutoCrankPlanV16, AutoCrankWorkV16,
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    PortfolioAccountV16Account, PortfolioLegV16, PortfolioLegV16Account, PortfolioV16ViewMut,
    ProvenanceHeaderV16, ProvenanceHeaderV16Account, SideV16, V16Config, V16PodI128, V16PodU128,
    V16PodU64,
};
use percolator::{ADL_ONE, POS_SCALE};

fn ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    ([1; 32], [2; 32], [3; 32])
}

fn market_fixture(init_price: u64) -> (MarketGroupV16HeaderAccount, Vec<Market<u64>>) {
    let (market_id, _, _) = ids();
    let cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(market_id, cfg, 1, 0).unwrap();
    let mut markets = vec![Market::new(0u64, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, init_price, 1)
        .unwrap();
    (header, markets)
}

fn account_fixture(seed: u8) -> PortfolioAccountV16Account {
    let (market_id, _, owner) = ids();
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        market_id, [seed; 32], owner,
    ));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(header).unwrap();
    a
}

/// Same state as `v16_auto_crank_liquidates_current_account_without_observation`
/// (v16_spec_tests.rs): one long of 1 unit at price 100, pnl -5, capital 0 (below maintenance).
fn underwater_long() -> (
    MarketGroupV16HeaderAccount,
    Vec<Market<u64>>,
    PortfolioAccountV16Account,
) {
    let (mut header, mut markets) = market_fixture(100);
    let mut account_header = account_fixture(14);
    header.current_slot = V16PodU64::new(10);
    header.slot_last = V16PodU64::new(10);
    header.vault = V16PodU128::new(50);
    header.insurance = V16PodU128::new(50);
    header.negative_pnl_account_count = V16PodU64::new(1);
    let mut asset = markets[0].engine.asset.try_to_runtime().unwrap();
    asset.slot_last = 10;
    asset.oi_eff_long_q = 2 * POS_SCALE;
    asset.oi_eff_short_q = 2 * POS_SCALE;
    asset.loss_weight_sum_long = 2 * POS_SCALE;
    asset.loss_weight_sum_short = 2 * POS_SCALE;
    asset.stored_pos_count_long = 2;
    asset.stored_pos_count_short = 2;
    markets[0].engine.asset = AssetStateV16Account::from_runtime(&asset);
    header.resolved_payout_blocker_count = V16PodU64::new(4);
    account_header.pnl = V16PodI128::new(-5);
    account_header.legs[0] = PortfolioLegV16Account::from_runtime(&PortfolioLegV16 {
        active: true,
        asset_index: 0,
        market_id: asset.market_id,
        side: SideV16::Long,
        basis_pos_q: POS_SCALE as i128,
        a_basis: ADL_ONE,
        k_snap: asset.k_long,
        f_snap: asset.f_long_num,
        k_rem_num: 0,
        f_rem_num: 0,
        kf_epoch_snap: 0,
        epoch_snap: asset.epoch_long,
        loss_weight: POS_SCALE,
        b_snap: asset.b_long_num,
        b_rem: 0,
        b_epoch_snap: asset.epoch_long,
        b_stale: false,
        stale: false,
        band_epoch_snap: 0,
        band_liq_pending: false,
        rent_snap: 0,
        rent_carry: 0,
    });
    account_header.active_bitmap[0] = V16PodU64::new(1);
    (header, markets, account_header)
}

fn work(now_slot: u64, obs: &[AutoCrankObservationV16]) -> AutoCrankWorkV16<'_> {
    AutoCrankWorkV16 {
        now_slot,
        observations: obs,
        resolved_close_fee_rate_per_slot: 0,
    }
}

/// F1 round shape `[accrue (price moves), crank T]`, repeated: T is below maintenance after every
/// round (current certificate with a deficit), yet every crank only refreshes. Never liquidated.
#[test]
fn f1_char_one_crank_per_round_after_a_price_push_never_liquidates() {
    let (mut header, mut markets, mut account_header) = underwater_long();
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header);
    market.full_account_refresh_not_atomic(&mut account).unwrap();
    let s0 = market.build_actionable_summary(&account.as_view()).unwrap();
    assert!(s0.liquidatable && !s0.stale, "setup: current and liquidatable: {s0:?}");

    let mut refreshes = 0;
    for round in 0..12u64 {
        let slot = 11 + round;
        // Alternate 99/100 so every push is a price move (oracle_epoch bump) inside the per-slot cap.
        let price = if round % 2 == 0 { 99 } else { 100 };
        // The keeper's observation crank through another account (the LP) accrues the asset first;
        // `true` = that crank committed protective progress on its own account, as on chain.
        market
            .accrue_asset_to_not_atomic(0, slot, price, 0, true)
            .unwrap();
        let pre = market.build_actionable_summary(&account.as_view()).unwrap();
        assert!(pre.stale && !pre.liquidatable, "round {round}: stale cert hides the deficit: {pre:?}");
        let obs = [AutoCrankObservationV16 { asset_index: 0, effective_price: price, funding_rate_e9: 0 }];
        let r = market
            .permissionless_auto_crank_not_atomic(&mut account, work(slot, &obs))
            .unwrap();
        assert_eq!(r.selected, AutoCrankPlanV16::RefreshAccount { asset_index: Some(0) }, "round {round}");
        assert_eq!(account.header.active_bitmap[0].get(), 1, "round {round}: position still open");
        let post = market.build_actionable_summary(&account.as_view()).unwrap();
        assert!(post.liquidatable && !post.stale, "round {round}: below maintenance after the refresh: {post:?}");
        refreshes += 1;
    }
    assert_eq!(refreshes, 12);
}

/// The liq2 shape `[accrue, crank T, crank T]`: the second crank in the same round liquidates.
#[test]
fn f1_char_second_crank_same_round_liquidates() {
    let (mut header, mut markets, mut account_header) = underwater_long();
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header);
    market.full_account_refresh_not_atomic(&mut account).unwrap();
    market.accrue_asset_to_not_atomic(0, 11, 99, 0, true).unwrap();
    let obs = [AutoCrankObservationV16 { asset_index: 0, effective_price: 99, funding_rate_e9: 0 }];
    let r1 = market
        .permissionless_auto_crank_not_atomic(&mut account, work(11, &obs))
        .unwrap();
    assert_eq!(r1.selected, AutoCrankPlanV16::RefreshAccount { asset_index: Some(0) });
    assert_eq!(account.header.active_bitmap[0].get(), 1);
    let r2 = market
        .permissionless_auto_crank_not_atomic(&mut account, work(11, &obs))
        .unwrap();
    assert_eq!(r2.selected, AutoCrankPlanV16::Liquidate { asset_index: 0 });
    assert_eq!(account.header.active_bitmap[0].get(), 0, "second crank closes the position");
    market.validate_shape().unwrap();
}

/// Probe of the ledger's "a second crank in a LATER transaction returns 22": after the refresh
/// round, a crank in the next round with a fresh observation but NO separate accrual before it.
/// At engine level this liquidates (the classifier reads the still-current certificate before the
/// crank's own accrual), so the on-chain 22 comes from the wrapper's tx shape (an accrual ahead of
/// the crank), not from the engine liquidation path.
#[test]
fn f1_char_later_round_crank_without_prior_accrual() {
    let (mut header, mut markets, mut account_header) = underwater_long();
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header);
    market.full_account_refresh_not_atomic(&mut account).unwrap();
    market.accrue_asset_to_not_atomic(0, 11, 99, 0, true).unwrap();
    let obs = [AutoCrankObservationV16 { asset_index: 0, effective_price: 99, funding_rate_e9: 0 }];
    let r1 = market
        .permissionless_auto_crank_not_atomic(&mut account, work(11, &obs))
        .unwrap();
    assert_eq!(r1.selected, AutoCrankPlanV16::RefreshAccount { asset_index: Some(0) });
    for (slot, price) in [(12u64, 99u64), (12, 100), (13, 98)] {
        let obs2 = [AutoCrankObservationV16 { asset_index: 0, effective_price: price, funding_rate_e9: 0 }];
        let mut h2 = *market.header;
        let mut m2 = market.markets.to_vec();
        let mut a2 = *account.header;
        let mut mk2 = MarketGroupV16ViewMut::new(&mut h2, &mut m2);
        let mut ac2 = PortfolioV16ViewMut::new(&mut a2);
        let r = mk2.permissionless_auto_crank_not_atomic(&mut ac2, work(slot, &obs2));
        println!(
            "F1-PROBE later-round crank slot={slot} price={price}: {:?} bitmap_after={}",
            r.as_ref().map(|x| x.selected),
            ac2.header.active_bitmap[0].get()
        );
        assert_eq!(r.unwrap().selected, AutoCrankPlanV16::Liquidate { asset_index: 0 });
        assert_eq!(ac2.header.active_bitmap[0].get(), 0);
    }
}
