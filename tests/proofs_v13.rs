#![cfg(kani)]

use percolator::v13::{
    account_equity, HLockLaneV13, LiquidationRequestV13, MarketGroupV13, PortfolioAccountV13,
    ProvenanceHeaderV13, ResolvedCloseOutcomeV13, SideV13, TradeRequestV13, V13Config, V13Error,
    V13_MAX_PORTFOLIO_ASSETS_N,
};
use percolator::{POS_SCALE, SOCIAL_LOSS_DEN};

fn symbolic_ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    let market: [u8; 32] = kani::any();
    let account: [u8; 32] = kani::any();
    let owner: [u8; 32] = kani::any();
    (market, account, owner)
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_hlock_is_exactly_hmin_or_hmax() {
    let h_max: u8 = kani::any();
    kani::assume(h_max > 0);
    let (market, account_id, owner) = symbolic_ids();
    let mut group =
        MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, h_max as u64)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));

    group.threshold_stress_active = kani::any();
    group.bankruptcy_hlock_active = kani::any();
    group.loss_stale_active = kani::any();
    group.active_bankrupt_close_present = kani::any();
    account.stale_state = kani::any();
    account.b_stale_state = kani::any();
    let instruction_bankruptcy_candidate: bool = kani::any();

    kani::cover!(
        !group.threshold_stress_active
            && !group.bankruptcy_hlock_active
            && !group.loss_stale_active
            && !group.active_bankrupt_close_present
            && !account.stale_state
            && !account.b_stale_state
            && !instruction_bankruptcy_candidate,
        "v13 h-min lane reachable"
    );
    kani::cover!(
        group.threshold_stress_active
            || group.bankruptcy_hlock_active
            || group.loss_stale_active
            || group.active_bankrupt_close_present
            || account.stale_state
            || account.b_stale_state
            || instruction_bankruptcy_candidate,
        "v13 h-max lane reachable"
    );

    let selected = group
        .select_h_lock(Some(&account), instruction_bankruptcy_candidate)
        .unwrap();
    assert!(selected == 0 || selected == h_max as u64);

    let lane = group
        .h_lock_lane(Some(&account), instruction_bankruptcy_candidate)
        .unwrap();
    if lane == HLockLaneV13::HMax {
        assert_eq!(selected, h_max as u64);
    } else {
        assert_eq!(selected, 0);
    }
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_hmin_zero_remains_available_when_no_lock_state_exists() {
    let h_max: u8 = kani::any();
    kani::assume(h_max > 0);
    let (market, account_id, owner) = symbolic_ids();
    let group =
        MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, h_max as u64)).unwrap();
    let account = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));

    assert_eq!(
        group.h_lock_lane(Some(&account), false),
        Ok(HLockLaneV13::HMin)
    );
    assert_eq!(group.select_h_lock(Some(&account), false), Ok(0));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_stale_counter_transitions_are_idempotent() {
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));

    group.mark_account_stale(&mut account).unwrap();
    group.mark_account_stale(&mut account).unwrap();
    kani::cover!(account.stale_state, "v13 stale state reachable");
    assert_eq!(group.stale_certificate_count, 1);

    group.clear_account_stale(&mut account).unwrap();
    group.clear_account_stale(&mut account).unwrap();
    kani::cover!(!account.stale_state, "v13 stale clear reachable");
    assert_eq!(group.stale_certificate_count, 0);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_account_equity_rejects_i128_min_persistent_pnl() {
    let (market, account_id, owner) = symbolic_ids();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    account.pnl = i128::MIN;
    assert_eq!(account_equity(&account), Err(V13Error::ArithmeticOverflow));
}

fn concrete_ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    ([1; 32], [2; 32], [3; 32])
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_hidden_leg_rejected_by_bitmap_authority() {
    let (market, account_id, owner) = concrete_ids();
    let group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));

    account.legs[0].active = true;
    kani::cover!(
        account.active_bitmap == 0 && account.legs[0].active,
        "v13 hidden active leg reachable"
    );
    assert_eq!(
        group.validate_account_shape(&account),
        Err(V13Error::HiddenLeg)
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_attach_then_clear_leg_restores_account_local_counters_for_long() {
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));

    group.attach_leg(&mut account, 0, SideV13::Long, 7).unwrap();
    assert_eq!(account.active_bitmap, 1);
    assert_eq!(account.legs[0].basis_pos_q, 7);
    assert_eq!(group.assets[0].oi_eff_long_q, 7);

    group.clear_leg(&mut account, 0).unwrap();
    assert_eq!(account.active_bitmap, 0);
    assert_eq!(group.assets[0].oi_eff_long_q, 0);
    assert_eq!(group.assets[0].oi_eff_short_q, 0);
    assert_eq!(group.assets[0].stored_pos_count_long, 0);
    assert_eq!(group.assets[0].stored_pos_count_short, 0);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_account_b_chunk_either_advances_or_fails_closed() {
    let target_units: u8 = kani::any();
    let budget_units: u8 = kani::any();
    kani::assume(target_units <= 4);
    kani::assume(budget_units <= 4);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    group.attach_leg(&mut account, 0, SideV13::Long, 1).unwrap();
    group.assets[0].b_long_num = (target_units as u128) * SOCIAL_LOSS_DEN;
    group.mark_leg_b_stale(&mut account, 0).unwrap();

    let before_snap = account.legs[0].b_snap;
    let before_remaining = group.assets[0].b_long_num - before_snap;
    let budget = (budget_units as u128) * SOCIAL_LOSS_DEN;
    let result = group.settle_account_b_chunk(&mut account, 0, budget);

    if before_remaining == 0 {
        assert!(result.is_ok());
        assert_eq!(account.legs[0].b_snap, before_snap);
    } else if budget == 0 {
        assert_eq!(result, Err(V13Error::RecoveryRequired));
        assert_eq!(account.legs[0].b_snap, before_snap);
    } else {
        let chunk = result.unwrap();
        kani::cover!(chunk.delta_b > 0, "v13 B chunk progress reachable");
        assert!(chunk.delta_b > 0);
        assert!(account.legs[0].b_snap > before_snap);
        assert!(chunk.remaining_after < before_remaining);
    }
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_liquidation_progress_rejects_non_reducing_scores() {
    let deficit: u8 = kani::any();
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut before =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    let mut after = before;
    group
        .full_account_refresh(&mut before, &[1; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    group
        .full_account_refresh(&mut after, &[1; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    before.health_cert.certified_liq_deficit = deficit as u128;
    after.health_cert.certified_liq_deficit = deficit as u128;

    assert_eq!(
        group.validate_liquidation_progress(&before, &after),
        Err(V13Error::NonProgress)
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_favorable_action_requires_current_full_refresh() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    account.capital = 2;

    assert_eq!(
        group.ensure_favorable_action_allowed(&account),
        Err(V13Error::Stale)
    );
    group
        .full_account_refresh(&mut account, &[1; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(group.ensure_favorable_action_allowed(&account), Ok(()));
    group.oracle_epoch += 1;
    assert_eq!(
        group.ensure_favorable_action_allowed(&account),
        Err(V13Error::Stale)
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_full_refresh_clears_stale_certificate() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));

    group.mark_account_stale(&mut account).unwrap();
    assert_eq!(group.stale_certificate_count, 1);
    group
        .full_account_refresh(&mut account, &[1; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    kani::cover!(
        !account.stale_state,
        "v13 stale account refresh clears stale state"
    );
    assert!(!account.stale_state);
    assert_eq!(group.stale_certificate_count, 0);
    assert_eq!(group.ensure_favorable_action_allowed(&account), Ok(()));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_side_reset_prior_epoch_account_can_clear_without_oi_underflow() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    group.attach_leg(&mut account, 0, SideV13::Long, 1).unwrap();
    group.assets[0].oi_eff_long_q = 0;

    group.begin_full_drain_reset(0, SideV13::Long).unwrap();
    assert_eq!(group.assets[0].oi_eff_long_q, 0);
    group.clear_leg(&mut account, 0).unwrap();
    assert_eq!(group.assets[0].stored_pos_count_long, 0);
    assert_eq!(group.assets[0].oi_eff_long_q, 0);
    group.finalize_ready_reset_side(0, SideV13::Long).unwrap();
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_quantity_adl_preserves_oi_symmetry_after_close() {
    let close_q: u8 = kani::any();
    kani::assume(close_q > 0);
    kani::assume(close_q <= 4);
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    group.assets[0].oi_eff_long_q = 4;
    group.assets[0].oi_eff_short_q = 4;

    let out = group
        .apply_quantity_adl_after_residual_not_atomic(0, SideV13::Long, close_q as u128)
        .unwrap();
    kani::cover!(out.closed_q > 0, "v13 quantity ADL close reachable");
    assert_eq!(
        group.assets[0].oi_eff_long_q,
        group.assets[0].oi_eff_short_q
    );
    if close_q == 4 {
        assert!(out.reset_started);
    }
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_fee_charge_settles_loss_before_fee() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));

    group.deposit_not_atomic(&mut account, 1).unwrap();
    account.pnl = -1;
    group.negative_pnl_account_count = 1;
    let charged = group
        .charge_account_fee_not_atomic(&mut account, 1)
        .unwrap();

    kani::cover!(
        account.pnl < 0 || charged == 0,
        "v13 loss-before-fee path reached"
    );
    assert_eq!(charged, 0);
    assert_eq!(account.capital, 0);
    assert_eq!(account.pnl, 0);
    assert_eq!(group.insurance, 0);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_equity_active_accrual_requires_protective_progress() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    group
        .attach_leg(&mut account, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();

    let result = group.accrue_asset_to_not_atomic(0, 1, 2, 0, false);
    assert_eq!(result, Err(V13Error::NonProgress));
    assert_eq!(group.slot_last, 0);

    let ok = group.accrue_asset_to_not_atomic(0, 1, 2, 0, true);
    assert!(ok.is_ok());
    assert_eq!(group.slot_last, 1);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_equity_active_accrual_advances_at_most_one_bounded_segment() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    group.config.max_accrual_dt_slots = 2;
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    group
        .attach_leg(&mut account, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();

    let out = group.accrue_asset_to_not_atomic(0, 10, 3, 0, true).unwrap();
    assert_eq!(out.dt, 2);
    assert_eq!(group.slot_last, 2);
    assert_eq!(group.current_slot, 10);
    assert!(group.loss_stale_active);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_trade_dynamic_fee_cap_is_enforced_before_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    group.config.max_trading_fee_bps = 1;
    let mut long = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    let mut short = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut long, 10).unwrap();
    group.deposit_not_atomic(&mut short, 10).unwrap();

    let result = group.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV13 {
            asset_index: 0,
            size_q: 1,
            exec_price: 1,
            fee_bps: 2,
        },
        &[1; V13_MAX_PORTFOLIO_ASSETS_N],
    );
    assert_eq!(result, Err(V13Error::InvalidConfig));
    assert_eq!(long.active_bitmap, 0);
    assert_eq!(short.active_bitmap, 0);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_hlock_rejects_risk_increasing_trade_before_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut long = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    let mut short = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut long, 10).unwrap();
    group.deposit_not_atomic(&mut short, 10).unwrap();
    group.threshold_stress_active = true;

    let result = group.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV13 {
            asset_index: 0,
            size_q: 1,
            exec_price: 1,
            fee_bps: 0,
        },
        &[1; V13_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(result, Err(V13Error::LockActive));
    assert_eq!(long.active_bitmap, 0);
    assert_eq!(short.active_bitmap, 0);
    assert_eq!(group.insurance, 0);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_target_effective_lag_rejects_risk_increasing_trade_before_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut long = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    let mut short = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut long, 10).unwrap();
    group.deposit_not_atomic(&mut short, 10).unwrap();
    group.assets[0].effective_price = 1;
    group.assets[0].raw_oracle_target_price = 2;

    let result = group.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV13 {
            asset_index: 0,
            size_q: 1,
            exec_price: 1,
            fee_bps: 0,
        },
        &[1; V13_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(result, Err(V13Error::LockActive));
    assert_eq!(long.active_bitmap, 0);
    assert_eq!(short.active_bitmap, 0);
    assert_eq!(group.insurance, 0);
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v13_hlock_allows_pure_risk_reducing_trade_with_principal_margin() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut reducing_short =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    let mut reducing_long =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut reducing_short, 100).unwrap();
    group.deposit_not_atomic(&mut reducing_long, 100).unwrap();
    group
        .attach_leg(&mut reducing_short, 0, SideV13::Short, -10)
        .unwrap();
    group
        .attach_leg(&mut reducing_long, 0, SideV13::Long, 10)
        .unwrap();
    group.threshold_stress_active = true;

    let result = group.execute_trade_with_fee_not_atomic(
        &mut reducing_short,
        &mut reducing_long,
        TradeRequestV13 {
            asset_index: 0,
            size_q: 5,
            exec_price: 1,
            fee_bps: 0,
        },
        &[1; V13_MAX_PORTFOLIO_ASSETS_N],
    );

    assert!(result.is_ok());
    assert_eq!(reducing_short.legs[0].basis_pos_q, -5);
    assert_eq!(reducing_long.legs[0].basis_pos_q, 5);
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v13_hlock_withdraw_uses_no_positive_credit_lane() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 20).unwrap();
    group
        .attach_leg(&mut account, 0, SideV13::Long, 10)
        .unwrap();
    account.pnl = 100;
    group.pnl_pos_tot = 100;
    group.threshold_stress_active = true;

    let result =
        group.withdraw_not_atomic(&mut account, 11, &[1_000_000; V13_MAX_PORTFOLIO_ASSETS_N]);

    assert_eq!(result, Err(V13Error::InvalidConfig));
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v13_loss_stale_blocks_nonflat_withdrawal() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 100).unwrap();
    group
        .attach_leg(&mut account, 0, SideV13::Long, 10)
        .unwrap();
    group.loss_stale_active = true;

    let result = group.withdraw_not_atomic(&mut account, 10, &[1; V13_MAX_PORTFOLIO_ASSETS_N]);

    assert_eq!(result, Err(V13Error::LockActive));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_resolved_positive_payout_snapshot_is_order_stable() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut first = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    let mut second = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, [4; 32], owner));
    group.vault = 100;
    first.pnl = 100;
    second.pnl = 100;
    group.pnl_pos_tot = 200;
    group.resolve_market_not_atomic(1).unwrap();

    let first_close = group.close_resolved_account_not_atomic(&mut first, 0);
    let second_close = group.close_resolved_account_not_atomic(&mut second, 0);

    assert_eq!(
        first_close,
        Ok(ResolvedCloseOutcomeV13::Closed { payout: 50 })
    );
    assert_eq!(
        second_close,
        Ok(ResolvedCloseOutcomeV13::Closed { payout: 50 })
    );
    assert_eq!(group.payout_snapshot, 100);
    assert_eq!(group.payout_snapshot_pnl_pos_tot, 200);
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v13_bankrupt_liquidation_consumes_insurance_before_social_loss() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    group.vault = 4;
    group.insurance = 4;
    account.pnl = -9;
    group.negative_pnl_account_count = 1;
    group.attach_leg(&mut account, 0, SideV13::Long, 1).unwrap();

    let out = group
        .liquidate_account_not_atomic(
            &mut account,
            LiquidationRequestV13 {
                asset_index: 0,
                close_q: 1,
                fee_bps: 0,
            },
            &[1; V13_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(out.insurance_used, 4);
    assert_eq!(out.residual_booked, 0);
    assert_eq!(out.explicit_loss, 5);
    assert_eq!(group.insurance, 0);
    assert_eq!(account.pnl, 0);
    assert_eq!(account.active_bitmap, 0);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_b_residual_booking_makes_durable_progress_or_fails_closed() {
    let residual_units: u8 = kani::any();
    kani::assume(residual_units <= 4);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    group
        .attach_leg(&mut account, 0, SideV13::Short, -1)
        .unwrap();

    let before_b = group.assets[0].b_short_num;
    let residual = residual_units as u128;
    let result = group.book_bankruptcy_residual_chunk(0, SideV13::Long, residual);
    if residual == 0 {
        assert_eq!(result.unwrap().remaining_after, 0);
        assert_eq!(group.assets[0].b_short_num, before_b);
    } else {
        let out = result.unwrap();
        kani::cover!(out.booked_loss > 0, "v13 residual B booking reachable");
        assert!(out.booked_loss > 0 || out.explicit_loss > 0);
        assert!(group.bankruptcy_hlock_active);
    }
}
