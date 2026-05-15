use percolator::v13::{
    account_equity, risk_notional_ceil, HLockLaneV13, LiquidationRequestV13, MarketGroupV13,
    PermissionlessCrankActionV13, PermissionlessCrankRequestV13, PermissionlessProgressOutcomeV13,
    PermissionlessRecoveryReasonV13, PortfolioAccountV13, PortfolioLegV13, ProvenanceHeaderV13,
    RebalanceRequestV13, ResolvedCloseOutcomeV13, SideV13, TradeRequestV13, V13Config, V13Error,
    V13_MAX_PORTFOLIO_ASSETS_N,
};
use percolator::{ADL_ONE, POS_SCALE, SOCIAL_LOSS_DEN};

fn ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    ([1; 32], [2; 32], [3; 32])
}

fn group() -> MarketGroupV13 {
    let (market, _, _) = ids();
    MarketGroupV13::new(market, V13Config::public_user_fund(4, 0, 10)).unwrap()
}

fn account() -> PortfolioAccountV13 {
    let (market, account_id, owner) = ids();
    PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner))
}

fn active_leg(side: SideV13, basis_pos_q: i128) -> PortfolioLegV13 {
    PortfolioLegV13 {
        active: true,
        side,
        basis_pos_q,
        a_basis: ADL_ONE,
        k_snap: 0,
        f_snap: 0,
        epoch_snap: 0,
        loss_weight: basis_pos_q.unsigned_abs(),
        b_snap: 0,
        b_rem: 0,
        b_epoch_snap: 0,
        b_stale: false,
        stale: false,
    }
}

#[test]
fn v13_hlock_is_permissionless_state_not_oracle_input() {
    let mut g = group();
    let mut a = account();

    assert_eq!(g.h_lock_lane(Some(&a), false), Ok(HLockLaneV13::HMin));
    assert_eq!(g.select_h_lock(Some(&a), false), Ok(0));

    g.threshold_stress_active = true;
    assert_eq!(g.h_lock_lane(Some(&a), false), Ok(HLockLaneV13::HMax));
    assert_eq!(g.select_h_lock(Some(&a), false), Ok(10));

    g.threshold_stress_active = false;
    assert_eq!(g.h_lock_lane(Some(&a), true), Ok(HLockLaneV13::HMax));

    a.b_stale_state = true;
    assert_eq!(g.h_lock_lane(Some(&a), false), Ok(HLockLaneV13::HMax));
}

#[test]
fn v13_provenance_binds_account_to_market_owner_and_layout() {
    let g = group();
    let mut a = account();
    assert_eq!(g.validate_portfolio_account_provenance(&a), Ok(()));

    a.provenance_header.market_group_id = [9; 32];
    assert_eq!(
        g.validate_portfolio_account_provenance(&a),
        Err(V13Error::ProvenanceMismatch)
    );
}

#[test]
fn v13_active_bitmap_is_the_only_active_leg_authority() {
    let g = group();
    let mut a = account();
    a.legs[0] = active_leg(SideV13::Long, 1);
    assert_eq!(g.validate_account_shape(&a), Err(V13Error::HiddenLeg));

    a.active_bitmap = 1;
    assert_eq!(g.validate_account_shape(&a), Ok(()));

    a.legs[5] = active_leg(SideV13::Short, -1);
    a.active_bitmap |= 1 << 5;
    assert_eq!(g.validate_account_shape(&a), Err(V13Error::HiddenLeg));
}

#[test]
fn v13_stale_and_b_stale_counters_are_exact_and_idempotent() {
    let mut g = group();
    let mut a = account();

    g.mark_account_stale(&mut a).unwrap();
    g.mark_account_stale(&mut a).unwrap();
    assert!(a.stale_state);
    assert_eq!(g.stale_certificate_count, 1);

    g.clear_account_stale(&mut a).unwrap();
    g.clear_account_stale(&mut a).unwrap();
    assert!(!a.stale_state);
    assert_eq!(g.stale_certificate_count, 0);

    g.mark_account_b_stale(&mut a).unwrap();
    g.mark_account_b_stale(&mut a).unwrap();
    assert!(a.b_stale_state);
    assert_eq!(g.b_stale_account_count, 1);

    g.clear_account_b_stale(&mut a).unwrap();
    g.clear_account_b_stale(&mut a).unwrap();
    assert!(!a.b_stale_state);
    assert_eq!(g.b_stale_account_count, 0);
}

#[test]
fn v13_full_refresh_clears_stale_certificate_but_not_b_stale_loss() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.attach_leg(&mut a, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();
    g.full_account_refresh(&mut a, &[100; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    g.mark_account_stale(&mut a).unwrap();
    assert_eq!(g.stale_certificate_count, 1);
    assert_eq!(
        g.ensure_favorable_action_allowed(&a),
        Err(V13Error::LockActive)
    );

    g.full_account_refresh(&mut a, &[100; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(g.stale_certificate_count, 0);
    assert!(!a.stale_state);
    assert_eq!(g.ensure_favorable_action_allowed(&a), Ok(()));

    g.assets[0].b_long_num = SOCIAL_LOSS_DEN;
    assert_eq!(
        g.full_account_refresh(&mut a, &[100; V13_MAX_PORTFOLIO_ASSETS_N]),
        Err(V13Error::BStale)
    );
}

#[test]
fn v13_favorable_action_requires_current_full_account_refresh() {
    let mut g = group();
    let mut a = account();
    a.capital = 100;
    g.attach_leg(&mut a, 0, SideV13::Long, 1_000_000).unwrap();
    let mut prices = [1u64; V13_MAX_PORTFOLIO_ASSETS_N];
    prices[0] = 100;

    assert_eq!(g.ensure_favorable_action_allowed(&a), Err(V13Error::Stale));

    let cert = g.full_account_refresh(&mut a, &prices).unwrap();
    assert!(cert.valid);
    assert_eq!(cert.certified_maintenance_req, 100);
    assert_eq!(g.ensure_favorable_action_allowed(&a), Ok(()));

    g.oracle_epoch += 1;
    assert_eq!(g.ensure_favorable_action_allowed(&a), Err(V13Error::Stale));
}

#[test]
fn v13_b_stale_blocks_refresh_and_favorable_actions_without_scanning_market() {
    let mut g = group();
    let mut a = account();
    a.capital = 100;
    g.attach_leg(&mut a, 0, SideV13::Long, 1_000_000).unwrap();
    let prices = [100u64; V13_MAX_PORTFOLIO_ASSETS_N];

    g.mark_account_b_stale(&mut a).unwrap();
    assert_eq!(
        g.full_account_refresh(&mut a, &prices),
        Err(V13Error::BStale)
    );
    assert_eq!(
        g.ensure_favorable_action_allowed(&a),
        Err(V13Error::LockActive)
    );
}

#[test]
fn v13_public_init_rejects_unbounded_portfolio_width() {
    let (market, _, _) = ids();
    let cfg = V13Config::public_user_fund((V13_MAX_PORTFOLIO_ASSETS_N + 1) as u8, 0, 10);
    assert_eq!(
        MarketGroupV13::new(market, cfg),
        Err(V13Error::InvalidConfig)
    );
}

#[test]
fn v13_public_init_rejects_disabled_recovery_profile() {
    let (market, _, _) = ids();
    let mut cfg = V13Config::public_user_fund(4, 0, 10);
    cfg.permissionless_recovery_enabled = false;

    assert_eq!(
        MarketGroupV13::new(market, cfg),
        Err(V13Error::InvalidConfig)
    );
}

#[test]
fn v13_risk_notional_and_equity_use_exact_conservative_shapes() {
    assert_eq!(risk_notional_ceil(1, 1), Ok(1));
    assert_eq!(risk_notional_ceil(1, 1_000_001), Ok(2));

    let mut a = account();
    a.capital = 100;
    a.pnl = -25;
    a.fee_credits = -10;
    assert_eq!(account_equity(&a), Ok(65));
}

#[test]
fn v13_deposit_withdraw_roundtrip_preserves_accounting() {
    let mut g = group();
    let mut a = account();

    g.deposit_not_atomic(&mut a, 123).unwrap();
    assert_eq!(a.capital, 123);
    assert_eq!(g.c_tot, 123);
    assert_eq!(g.vault, 123);

    g.withdraw_not_atomic(&mut a, 123, &[1; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(a.capital, 0);
    assert_eq!(g.c_tot, 0);
    assert_eq!(g.vault, 0);
    assert_eq!(g.assert_public_invariants(), Ok(()));
}

#[test]
fn v13_close_portfolio_account_requires_clean_local_state() {
    let mut g = group();
    let mut a = account();
    g.create_portfolio_account(&a).unwrap();
    assert_eq!(g.materialized_portfolio_count, 1);

    a.capital = 1;
    assert_eq!(g.close_portfolio_account(&a), Err(V13Error::LockActive));
    assert_eq!(g.materialized_portfolio_count, 1);

    a.capital = 0;
    g.close_portfolio_account(&a).unwrap();
    assert_eq!(g.materialized_portfolio_count, 0);
}

#[test]
fn v13_attach_and_clear_leg_update_only_bounded_account_and_asset_state() {
    let mut g = group();
    let mut a = account();

    g.attach_leg(&mut a, 1, SideV13::Short, -7).unwrap();
    assert_eq!(a.active_bitmap, 1 << 1);
    assert_eq!(g.assets[1].stored_pos_count_short, 1);
    assert_eq!(g.assets[1].oi_eff_short_q, 7);
    assert_eq!(g.assets[1].loss_weight_sum_short, 7);

    g.clear_leg(&mut a, 1).unwrap();
    assert_eq!(a.active_bitmap, 0);
    assert_eq!(g.assets[1].stored_pos_count_short, 0);
    assert_eq!(g.assets[1].oi_eff_short_q, 0);
    assert_eq!(g.assets[1].loss_weight_sum_short, 0);
}

#[test]
fn v13_oversize_position_is_rejected_before_oi_mutation() {
    let mut g = group();
    let mut a = account();

    let res = g.attach_leg(
        &mut a,
        0,
        SideV13::Long,
        (percolator::MAX_POSITION_ABS_Q + 1) as i128,
    );

    assert_eq!(res, Err(V13Error::InvalidLeg));
    assert_eq!(a.active_bitmap, 0);
    assert_eq!(g.assets[0].oi_eff_long_q, 0);
}

#[test]
fn v13_account_b_chunk_makes_strict_account_local_progress_or_requires_recovery() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV13::Long, 1).unwrap();
    g.assets[0].b_long_num = SOCIAL_LOSS_DEN * 2;
    g.mark_leg_b_stale(&mut a, 0).unwrap();

    let chunk = g
        .settle_account_b_chunk(&mut a, 0, SOCIAL_LOSS_DEN)
        .unwrap();
    assert!(chunk.delta_b > 0);
    assert!(a.legs[0].b_snap > 0);
    assert_eq!(a.health_cert.valid, false);

    let mut blocked = account();
    g.attach_leg(&mut blocked, 1, SideV13::Long, 1).unwrap();
    g.assets[1].b_long_num = 1;
    g.mark_leg_b_stale(&mut blocked, 1).unwrap();
    assert_eq!(
        g.settle_account_b_chunk(&mut blocked, 1, 0),
        Err(V13Error::RecoveryRequired)
    );
}

#[test]
fn v13_liquidation_progress_requires_strict_risk_score_reduction() {
    let mut g = group();
    let mut before = account();
    let mut after = account();
    g.full_account_refresh(&mut before, &[1; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    g.full_account_refresh(&mut after, &[1; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    before.health_cert.certified_liq_deficit = 10;
    after.health_cert.certified_liq_deficit = 10;
    assert_eq!(
        g.validate_liquidation_progress(&before, &after),
        Err(V13Error::NonProgress)
    );

    after.health_cert.certified_liq_deficit = 9;
    assert_eq!(g.validate_liquidation_progress(&before, &after), Ok(()));
}

#[test]
fn v13_permissionless_recovery_is_declared_by_reason_not_caller_price() {
    let mut g = group();
    let reason = PermissionlessRecoveryReasonV13::AccountBSettlementCannotProgress;
    assert_eq!(
        g.declare_permissionless_recovery(reason),
        Ok(PermissionlessProgressOutcomeV13::RecoveryDeclared(reason))
    );
    assert_eq!(g.recovery_reason, Some(reason));
}

#[test]
fn v13_fees_are_charged_only_after_realized_losses() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    a.pnl = -100;
    g.negative_pnl_account_count = 1;

    let charged = g.charge_account_fee_not_atomic(&mut a, 100).unwrap();
    assert_eq!(charged, 0);
    assert_eq!(a.capital, 0);
    assert_eq!(a.pnl, 0);
    assert_eq!(g.insurance, 0);
    assert_eq!(g.c_tot, 0);
}

#[test]
fn v13_fee_sync_settles_hidden_kf_losses_before_collecting_fee() {
    let mut g = group();
    g.assets[0].effective_price = 100;
    g.assets[0].fund_px_last = 100;
    let mut long = account();
    g.deposit_not_atomic(&mut long, 50).unwrap();
    g.attach_leg(&mut long, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();

    g.accrue_asset_to_not_atomic(0, 1, 50, 0, true).unwrap();
    let charged = g
        .sync_account_fee_to_slot_not_atomic(&mut long, 1, 100)
        .unwrap();

    assert_eq!(charged, 0);
    assert_eq!(long.capital, 0);
    assert_eq!(long.pnl, 0);
    assert_eq!(g.insurance, 0);
}

#[test]
fn v13_hlock_allows_principal_withdrawal_without_positive_credit_escape() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.attach_leg(&mut a, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();
    g.threshold_stress_active = true;

    assert_eq!(
        g.withdraw_not_atomic(&mut a, 50, &[10; V13_MAX_PORTFOLIO_ASSETS_N]),
        Ok(())
    );
    assert_eq!(a.capital, 50);
    assert_eq!(g.vault, 50);
}

#[test]
fn v13_hlock_withdraw_rejects_if_post_state_needs_positive_pnl_credit() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 20).unwrap();
    g.attach_leg(&mut a, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();
    a.pnl = 100;
    g.pnl_pos_tot = 100;
    g.threshold_stress_active = true;

    assert_eq!(
        g.withdraw_not_atomic(&mut a, 10, &[50; V13_MAX_PORTFOLIO_ASSETS_N]),
        Err(V13Error::InvalidConfig)
    );
}

#[test]
fn v13_loss_stale_blocks_nonflat_withdrawal_even_if_no_positive_credit_suffices() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.attach_leg(&mut a, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();
    g.loss_stale_active = true;

    assert_eq!(
        g.withdraw_not_atomic(&mut a, 10, &[10; V13_MAX_PORTFOLIO_ASSETS_N]),
        Err(V13Error::LockActive)
    );
}

#[test]
fn v13_target_effective_lag_blocks_risk_increasing_trade_before_mutation() {
    let mut g = group();
    let mut long = account();
    let mut short = account();
    short.provenance_header.portfolio_account_id = [4; 32];
    g.deposit_not_atomic(&mut long, 10_000).unwrap();
    g.deposit_not_atomic(&mut short, 10_000).unwrap();
    g.assets[0].effective_price = 100;
    g.assets[0].raw_oracle_target_price = 120;

    let res = g.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV13 {
            asset_index: 0,
            size_q: POS_SCALE,
            exec_price: 100,
            fee_bps: 0,
        },
        &[100; V13_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(res, Err(V13Error::LockActive));
    assert_eq!(long.active_bitmap, 0);
    assert_eq!(short.active_bitmap, 0);
}

#[test]
fn v13_target_effective_lag_allows_pure_risk_reducing_trade() {
    let mut g = group();
    let mut reducing_short = account();
    let mut reducing_long = account();
    reducing_long.provenance_header.portfolio_account_id = [4; 32];
    g.deposit_not_atomic(&mut reducing_short, 10_000).unwrap();
    g.deposit_not_atomic(&mut reducing_long, 10_000).unwrap();
    g.attach_leg(&mut reducing_short, 0, SideV13::Short, -(POS_SCALE as i128))
        .unwrap();
    g.attach_leg(&mut reducing_long, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();
    g.assets[0].effective_price = 100;
    g.assets[0].raw_oracle_target_price = 120;

    assert!(g
        .execute_trade_with_fee_not_atomic(
            &mut reducing_short,
            &mut reducing_long,
            TradeRequestV13 {
                asset_index: 0,
                size_q: POS_SCALE / 2,
                exec_price: 100,
                fee_bps: 0,
            },
            &[100; V13_MAX_PORTFOLIO_ASSETS_N],
        )
        .is_ok());
}

#[test]
fn v13_target_effective_lag_blocks_nonflat_withdrawal_and_pnl_conversion() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.attach_leg(&mut a, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();
    a.pnl = 10;
    g.pnl_pos_tot = 10;
    g.vault = g.vault.checked_add(10).unwrap();
    g.assets[0].effective_price = 100;
    g.assets[0].raw_oracle_target_price = 120;
    g.full_account_refresh(&mut a, &[100; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(
        g.withdraw_not_atomic(&mut a, 1, &[100; V13_MAX_PORTFOLIO_ASSETS_N]),
        Err(V13Error::LockActive)
    );
    assert_eq!(
        g.convert_released_pnl_to_capital_not_atomic(&mut a),
        Err(V13Error::LockActive)
    );
}

#[test]
fn v13_account_free_equity_active_accrual_requires_protective_progress() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 1000).unwrap();
    g.attach_leg(&mut a, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();
    let mut b = account();
    b.provenance_header.portfolio_account_id = [4; 32];
    g.deposit_not_atomic(&mut b, 1000).unwrap();
    g.attach_leg(&mut b, 0, SideV13::Short, -(POS_SCALE as i128))
        .unwrap();

    assert_eq!(
        g.accrue_asset_to_not_atomic(0, 1, 2, 0, false),
        Err(V13Error::NonProgress)
    );
    assert!(g.accrue_asset_to_not_atomic(0, 1, 2, 0, true).is_ok());
}

#[test]
fn v13_equity_active_accrual_commits_one_bounded_loss_stale_segment() {
    let mut g = group();
    g.config.max_accrual_dt_slots = 2;
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();

    let out = g.accrue_asset_to_not_atomic(0, 10, 3, 0, true).unwrap();
    assert_eq!(out.dt, 2);
    assert!(out.loss_stale_after);
    assert_eq!(g.slot_last, 2);
    assert_eq!(g.current_slot, 10);
    assert!(g.loss_stale_active);
}

#[test]
fn v13_funding_rate_above_cap_rejects_before_state_mutation() {
    let mut g = group();
    g.config.max_abs_funding_e9_per_slot = 1;
    let before_asset = g.assets[0];

    let res = g.accrue_asset_to_not_atomic(0, 1, 1, 2, true);

    assert_eq!(res, Err(V13Error::InvalidConfig));
    assert_eq!(g.assets[0], before_asset);
    assert_eq!(g.slot_last, 0);
    assert_eq!(g.current_slot, 0);
}

#[test]
fn v13_trade_fee_is_dynamic_bounded_and_charged_inside_engine() {
    let mut g = group();
    g.config.max_trading_fee_bps = 100;
    let mut long = account();
    let mut short = account();
    short.provenance_header.portfolio_account_id = [4; 32];
    g.deposit_not_atomic(&mut long, 10_000).unwrap();
    g.deposit_not_atomic(&mut short, 10_000).unwrap();

    let req = TradeRequestV13 {
        asset_index: 0,
        size_q: POS_SCALE,
        exec_price: 1_000,
        fee_bps: 50,
    };
    let out = g
        .execute_trade_with_fee_not_atomic(
            &mut long,
            &mut short,
            req,
            &[1_000; V13_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();
    assert_eq!(out.notional, 1_000);
    assert_eq!(out.fee_a, 5);
    assert_eq!(long.active_bitmap, 1);
    assert_eq!(short.active_bitmap, 1);
    assert_eq!(g.insurance, 10);

    let mut bad_req = req;
    bad_req.fee_bps = 101;
    assert_eq!(
        g.execute_trade_with_fee_not_atomic(
            &mut long,
            &mut short,
            bad_req,
            &[1_000; V13_MAX_PORTFOLIO_ASSETS_N],
        ),
        Err(V13Error::InvalidConfig)
    );
}

#[test]
fn v13_trade_fee_conserves_vault_and_keeps_oi_symmetric() {
    let mut g = group();
    g.config.max_trading_fee_bps = 1_000;
    let mut long = account();
    let mut short = account();
    short.provenance_header.portfolio_account_id = [4; 32];
    g.deposit_not_atomic(&mut long, 10_000).unwrap();
    g.deposit_not_atomic(&mut short, 10_000).unwrap();
    let vault_before = g.vault;
    let c_tot_before = g.c_tot;

    let out = g
        .execute_trade_with_fee_not_atomic(
            &mut long,
            &mut short,
            TradeRequestV13 {
                asset_index: 0,
                size_q: POS_SCALE,
                exec_price: 100,
                fee_bps: 100,
            },
            &[100; V13_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(out.notional, 100);
    assert_eq!(out.fee_a, 1);
    assert_eq!(out.fee_b, 1);
    assert_eq!(g.vault, vault_before);
    assert_eq!(g.insurance, 2);
    assert_eq!(g.c_tot, c_tot_before - 2);
    assert_eq!(g.assets[0].oi_eff_long_q, POS_SCALE);
    assert_eq!(g.assets[0].oi_eff_short_q, POS_SCALE);
}

#[test]
fn v13_hlock_blocks_risk_increasing_trade_before_fee_or_position_mutation() {
    let mut g = group();
    let mut long = account();
    let mut short = account();
    short.provenance_header.portfolio_account_id = [4; 32];
    g.deposit_not_atomic(&mut long, 10_000).unwrap();
    g.deposit_not_atomic(&mut short, 10_000).unwrap();
    g.threshold_stress_active = true;

    let res = g.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV13 {
            asset_index: 0,
            size_q: POS_SCALE,
            exec_price: 100,
            fee_bps: 0,
        },
        &[100; V13_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(res, Err(V13Error::LockActive));
    assert_eq!(long.active_bitmap, 0);
    assert_eq!(short.active_bitmap, 0);
    assert_eq!(g.insurance, 0);
}

#[test]
fn v13_hlock_allows_pure_risk_reducing_trade_with_no_positive_credit_margin() {
    let mut g = group();
    let mut reducing_short = account();
    let mut reducing_long = account();
    reducing_long.provenance_header.portfolio_account_id = [4; 32];
    g.deposit_not_atomic(&mut reducing_short, 10_000).unwrap();
    g.deposit_not_atomic(&mut reducing_long, 10_000).unwrap();
    g.attach_leg(&mut reducing_short, 0, SideV13::Short, -(POS_SCALE as i128))
        .unwrap();
    g.attach_leg(&mut reducing_long, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();
    g.threshold_stress_active = true;

    let out = g
        .execute_trade_with_fee_not_atomic(
            &mut reducing_short,
            &mut reducing_long,
            TradeRequestV13 {
                asset_index: 0,
                size_q: POS_SCALE / 2,
                exec_price: 100,
                fee_bps: 0,
            },
            &[100; V13_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(out.notional, 50);
    assert_eq!(
        reducing_short.legs[0].basis_pos_q.unsigned_abs(),
        POS_SCALE / 2
    );
    assert_eq!(
        reducing_long.legs[0].basis_pos_q.unsigned_abs(),
        POS_SCALE / 2
    );
}

#[test]
fn v13_hlock_rejects_reducing_trade_that_needs_positive_pnl_credit() {
    let mut g = group();
    let mut weak_short = account();
    let mut strong_long = account();
    strong_long.provenance_header.portfolio_account_id = [4; 32];
    weak_short.pnl = 100;
    g.pnl_pos_tot = 100;
    g.deposit_not_atomic(&mut strong_long, 10_000).unwrap();
    g.attach_leg(&mut weak_short, 0, SideV13::Short, -(POS_SCALE as i128))
        .unwrap();
    g.attach_leg(&mut strong_long, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();
    g.threshold_stress_active = true;

    let res = g.execute_trade_with_fee_not_atomic(
        &mut weak_short,
        &mut strong_long,
        TradeRequestV13 {
            asset_index: 0,
            size_q: POS_SCALE / 2,
            exec_price: 100,
            fee_bps: 0,
        },
        &[100; V13_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(res, Err(V13Error::LockActive));
}

#[test]
fn v13_released_pnl_conversion_is_bounded_by_residual_not_profit_only() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 10).unwrap();
    a.pnl = 50;
    g.pnl_pos_tot = 50;
    g.pnl_matured_pos_tot = 50;
    g.vault = g.c_tot + 7;
    g.full_account_refresh(&mut a, &[1; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    let converted = g
        .convert_released_pnl_to_capital_not_atomic(&mut a)
        .unwrap();

    assert_eq!(converted, 7);
    assert_eq!(g.vault, 17);
    assert_eq!(g.c_tot, 17);
    assert_eq!(a.capital, 17);
    assert_eq!(a.pnl, 43);
}

#[test]
fn v13_loss_stale_allows_pure_risk_reducing_trade_path() {
    let mut g = group();
    let mut reducing_short = account();
    let mut reducing_long = account();
    reducing_long.provenance_header.portfolio_account_id = [4; 32];
    g.deposit_not_atomic(&mut reducing_short, 10_000).unwrap();
    g.deposit_not_atomic(&mut reducing_long, 10_000).unwrap();
    g.attach_leg(&mut reducing_short, 0, SideV13::Short, -(POS_SCALE as i128))
        .unwrap();
    g.attach_leg(&mut reducing_long, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();
    g.loss_stale_active = true;

    assert!(g
        .execute_trade_with_fee_not_atomic(
            &mut reducing_short,
            &mut reducing_long,
            TradeRequestV13 {
                asset_index: 0,
                size_q: POS_SCALE / 2,
                exec_price: 100,
                fee_bps: 0,
            },
            &[100; V13_MAX_PORTFOLIO_ASSETS_N],
        )
        .is_ok());
}

#[test]
fn v13_b_residual_booking_is_bounded_and_remainder_conserving() {
    let mut g = group();
    let mut short = account();
    g.deposit_not_atomic(&mut short, 100).unwrap();
    g.attach_leg(&mut short, 0, SideV13::Short, -(POS_SCALE as i128))
        .unwrap();

    let out = g
        .book_bankruptcy_residual_chunk(0, SideV13::Long, 7)
        .unwrap();
    assert_eq!(out.booked_loss, 7);
    assert!(out.delta_b > 0);

    g.mark_leg_b_stale(&mut short, 0).unwrap();
    let chunk = g
        .settle_account_b_chunk(&mut short, 0, g.assets[0].b_short_num)
        .unwrap();
    assert_eq!(chunk.remaining_after, 0);
    assert!(short.pnl <= -7);
}

#[test]
fn v13_side_reset_snapshots_epoch_start_for_prior_epoch_accounts() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();
    g.assets[0].k_long = 5 * ADL_ONE as i128;
    g.assets[0].oi_eff_long_q = 0;

    g.begin_full_drain_reset(0, SideV13::Long).unwrap();
    assert_eq!(
        g.assets[0].mode_long,
        percolator::v13::SideModeV13::ResetPending
    );
    g.full_account_refresh(&mut a, &[1; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(a.pnl, 5);

    g.clear_leg(&mut a, 0).unwrap();
    g.finalize_ready_reset_side(0, SideV13::Long).unwrap();
    assert_eq!(g.assets[0].mode_long, percolator::v13::SideModeV13::Normal);
    assert_eq!(g.assets[0].stored_pos_count_long, 0);
}

#[test]
fn v13_quantity_adl_reduces_opposing_a_or_starts_reset_after_residual_durable() {
    let mut g = group();
    g.assets[0].oi_eff_long_q = 10;
    g.assets[0].oi_eff_short_q = 10;
    g.assets[0].a_short = ADL_ONE;

    let partial = g
        .apply_quantity_adl_after_residual_not_atomic(0, SideV13::Long, 4)
        .unwrap();
    assert_eq!(partial.closed_q, 4);
    assert_eq!(g.assets[0].oi_eff_long_q, 6);
    assert_eq!(g.assets[0].oi_eff_short_q, 6);
    assert_eq!(g.assets[0].a_short, ADL_ONE * 6 / 10);

    let full = g
        .apply_quantity_adl_after_residual_not_atomic(0, SideV13::Long, 6)
        .unwrap();
    assert!(full.reset_started);
    assert_eq!(g.assets[0].oi_eff_long_q, 0);
    assert_eq!(g.assets[0].oi_eff_short_q, 0);
}

#[test]
fn v13_permissionless_crank_commits_refresh_before_equity_active_accrual() {
    let mut g = group();
    let mut long = account();
    g.deposit_not_atomic(&mut long, 1000).unwrap();
    g.attach_leg(&mut long, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();
    let req = PermissionlessCrankRequestV13 {
        now_slot: 1,
        asset_index: 0,
        effective_price: 2,
        funding_rate_e9: 0,
        action: PermissionlessCrankActionV13::Refresh,
    };
    let out = g
        .permissionless_crank_not_atomic(&mut long, req, &[2; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(out, PermissionlessProgressOutcomeV13::AccountCurrent);
    assert_eq!(g.slot_last, 1);
}

#[test]
fn v13_permissionless_refresh_returns_partial_b_progress_without_failing() {
    let (market, _, _) = ids();
    let mut cfg = V13Config::public_user_fund(1, 0, 10);
    cfg.public_b_chunk_atoms = 1;
    let mut g = MarketGroupV13::new(market, cfg).unwrap();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.attach_leg(&mut a, 0, SideV13::Long, 1).unwrap();
    g.assets[0].b_long_num = SOCIAL_LOSS_DEN * 2;
    let req = PermissionlessCrankRequestV13 {
        now_slot: 1,
        asset_index: 0,
        effective_price: 1,
        funding_rate_e9: 0,
        action: PermissionlessCrankActionV13::Refresh,
    };

    let out = g
        .permissionless_crank_not_atomic(&mut a, req, &[1; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert!(matches!(
        out,
        PermissionlessProgressOutcomeV13::AccountBChunk(_)
    ));
    assert!(a.legs[0].b_stale);
    assert!(a.legs[0].b_snap > 0);
    assert!(a.legs[0].b_snap < g.assets[0].b_long_num);
    assert_eq!(g.slot_last, 0);
}

#[test]
fn v13_resolved_close_is_bounded_and_fee_current() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.resolve_market_not_atomic(10).unwrap();
    let out = g.close_resolved_account_not_atomic(&mut a, 1).unwrap();
    assert_eq!(out, ResolvedCloseOutcomeV13::Closed { payout: 90 });
    assert_eq!(a.last_fee_slot, 10);
    assert_eq!(a.capital, 0);
}

#[test]
fn v13_resolved_close_returns_progress_after_partial_b_settlement() {
    let (market, _, _) = ids();
    let mut cfg = V13Config::public_user_fund(1, 0, 10);
    cfg.public_b_chunk_atoms = 1;
    let mut g = MarketGroupV13::new(market, cfg).unwrap();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.attach_leg(&mut a, 0, SideV13::Long, 1).unwrap();
    g.assets[0].b_long_num = SOCIAL_LOSS_DEN * 2;
    g.resolve_market_not_atomic(10).unwrap();

    let out = g.close_resolved_account_not_atomic(&mut a, 1).unwrap();

    assert_eq!(out, ResolvedCloseOutcomeV13::ProgressOnly);
    assert!(a.legs[0].b_stale);
    assert!(a.legs[0].b_snap > 0);
    assert!(a.legs[0].b_snap < g.assets[0].b_long_num);
    assert_eq!(a.last_fee_slot, 0);
    assert_eq!(a.active_bitmap, 1);
}

#[test]
fn v13_resolved_positive_payout_uses_stable_snapshot_denominator() {
    let mut g = group();
    let mut a = account();
    let mut b = account();
    b.provenance_header.portfolio_account_id = [4; 32];
    g.vault = 100;
    a.pnl = 100;
    b.pnl = 100;
    g.pnl_pos_tot = 200;
    g.resolve_market_not_atomic(1).unwrap();

    let first = g.close_resolved_account_not_atomic(&mut a, 0).unwrap();
    let second = g.close_resolved_account_not_atomic(&mut b, 0).unwrap();

    assert_eq!(first, ResolvedCloseOutcomeV13::Closed { payout: 50 });
    assert_eq!(second, ResolvedCloseOutcomeV13::Closed { payout: 50 });
    assert_eq!(g.payout_snapshot, 100);
    assert_eq!(g.payout_snapshot_pnl_pos_tot, 200);
}

#[test]
fn v13_liquidation_requires_strict_account_risk_progress() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();
    g.accrue_asset_to_not_atomic(0, 1, 1, 0, true).unwrap();
    let req = LiquidationRequestV13 {
        asset_index: 0,
        close_q: POS_SCALE,
        fee_bps: 0,
    };
    let out = g
        .liquidate_account_not_atomic(&mut a, req, &[1; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(out.closed_q, POS_SCALE);
    assert_eq!(a.active_bitmap, 0);
}

#[test]
fn v13_bankrupt_liquidation_consumes_insurance_before_social_loss() {
    let mut g = group();
    let mut a = account();
    g.vault = 4;
    g.insurance = 4;
    a.pnl = -9;
    g.negative_pnl_account_count = 1;
    g.attach_leg(&mut a, 0, SideV13::Long, 1).unwrap();

    let out = g
        .liquidate_account_not_atomic(
            &mut a,
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
    assert_eq!(g.insurance, 0);
    assert_eq!(a.pnl, 0);
    assert_eq!(a.active_bitmap, 0);
}

#[test]
fn v13_bankrupt_liquidation_drops_uncollectible_fee_and_spends_insurance_once() {
    let (market, _, _) = ids();
    let mut cfg = V13Config::public_user_fund(1, 0, 10);
    cfg.liquidation_fee_bps = 10_000;
    cfg.liquidation_fee_cap = 10;
    cfg.min_liquidation_abs = 1;
    let mut g = MarketGroupV13::new(market, cfg).unwrap();
    let mut a = account();
    g.vault = 2;
    g.insurance = 2;
    a.pnl = -5;
    g.negative_pnl_account_count = 1;
    g.attach_leg(&mut a, 0, SideV13::Long, 1).unwrap();

    let out = g
        .liquidate_account_not_atomic(
            &mut a,
            LiquidationRequestV13 {
                asset_index: 0,
                close_q: 1,
                fee_bps: 10_000,
            },
            &[1; V13_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(out.fee_charged, 0);
    assert_eq!(out.insurance_used, 2);
    assert_eq!(out.residual_booked, 0);
    assert_eq!(out.explicit_loss, 3);
    assert_eq!(g.insurance, 0);
    assert_eq!(a.pnl, 0);
    assert_eq!(a.active_bitmap, 0);
}

#[test]
fn v13_bankrupt_liquidation_requires_residual_durable_before_freeing_exposure() {
    let (market, _, owner) = ids();
    let mut cfg = V13Config::public_user_fund(1, 0, 10);
    cfg.public_b_chunk_atoms = 1;
    let mut g = MarketGroupV13::new(market, cfg).unwrap();
    let mut bankrupt = account();
    let mut opposing = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, [4; 32], owner));

    g.attach_leg(&mut bankrupt, 0, SideV13::Long, 4).unwrap();
    g.attach_leg(&mut opposing, 0, SideV13::Short, -10).unwrap();
    bankrupt.pnl = -5;
    g.negative_pnl_account_count = 1;

    let before_bitmap = bankrupt.active_bitmap;
    let before_basis = bankrupt.legs[0].basis_pos_q;
    let before_pnl = bankrupt.pnl;
    let res = g.liquidate_account_not_atomic(
        &mut bankrupt,
        LiquidationRequestV13 {
            asset_index: 0,
            close_q: 4,
            fee_bps: 0,
        },
        &[1; V13_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(res, Err(V13Error::RecoveryRequired));
    assert_eq!(
        g.recovery_reason,
        Some(PermissionlessRecoveryReasonV13::ActiveBankruptCloseCannotProgress)
    );
    assert_eq!(bankrupt.active_bitmap, before_bitmap);
    assert_eq!(bankrupt.legs[0].basis_pos_q, before_basis);
    assert_eq!(bankrupt.pnl, before_pnl);
    assert_eq!(g.assets[0].b_short_num, 0);
}

#[test]
fn v13_rebalance_reduce_position_requires_strict_risk_progress_and_preserves_senior_claims() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();
    let senior_before = g.c_tot + g.insurance;
    let out = g
        .rebalance_reduce_position_not_atomic(
            &mut a,
            RebalanceRequestV13 {
                asset_index: 0,
                reduce_q: POS_SCALE / 2,
            },
            &[1_000_000; V13_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(out.reduced_q, POS_SCALE / 2);
    assert_eq!(a.legs[0].basis_pos_q.unsigned_abs(), POS_SCALE / 2);
    assert_eq!(g.c_tot + g.insurance, senior_before);
}

#[test]
fn v13_rebalance_rejects_missing_or_zero_progress() {
    let mut g = group();
    let mut a = account();

    assert_eq!(
        g.rebalance_reduce_position_not_atomic(
            &mut a,
            RebalanceRequestV13 {
                asset_index: 0,
                reduce_q: 1,
            },
            &[1_000_000; V13_MAX_PORTFOLIO_ASSETS_N],
        ),
        Err(V13Error::InvalidLeg)
    );
    assert_eq!(
        g.rebalance_reduce_position_not_atomic(
            &mut a,
            RebalanceRequestV13 {
                asset_index: 0,
                reduce_q: 0,
            },
            &[1_000_000; V13_MAX_PORTFOLIO_ASSETS_N],
        ),
        Err(V13Error::InvalidConfig)
    );
}
