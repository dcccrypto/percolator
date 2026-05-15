#![cfg(kani)]

use percolator::v13::{
    account_equity, HLockLaneV13, LiquidationRequestV13, MarketGroupV13, MarketModeV13,
    PermissionlessCrankActionV13, PermissionlessCrankRequestV13, PermissionlessProgressOutcomeV13,
    PermissionlessRecoveryReasonV13, PortfolioAccountV13, PortfolioLegV13, ProvenanceHeaderV13,
    RebalanceRequestV13, ResolvedCloseOutcomeV13, SideV13, TradeRequestV13, V13Config, V13Error,
    V13_MAX_PORTFOLIO_ASSETS_N,
};
use percolator::{ADL_ONE, MAX_POSITION_ABS_Q, MAX_PROTOCOL_FEE_ABS, POS_SCALE, SOCIAL_LOSS_DEN};

fn symbolic_ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    let market: [u8; 32] = kani::any();
    let account: [u8; 32] = kani::any();
    let owner: [u8; 32] = kani::any();
    (market, account, owner)
}

fn tight_envelope_config() -> V13Config {
    let mut cfg = V13Config::public_user_fund(1, 0, 1);
    cfg.maintenance_margin_bps = 500;
    cfg.initial_margin_bps = 600;
    cfg.min_nonzero_mm_req = 100;
    cfg.min_nonzero_im_req = 101;
    cfg.max_price_move_bps_per_slot = 3;
    cfg.max_accrual_dt_slots = 100;
    cfg.min_funding_lifetime_slots = 100;
    cfg.max_abs_funding_e9_per_slot = 10_000;
    cfg.liquidation_fee_bps = 100;
    cfg.liquidation_fee_cap = MAX_PROTOCOL_FEE_ABS;
    cfg.min_liquidation_abs = 0;
    cfg
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
fn proof_v13_b_stale_counter_transitions_are_idempotent_and_leg_gated() {
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));

    group.mark_account_b_stale(&mut account).unwrap();
    group.mark_account_b_stale(&mut account).unwrap();
    kani::cover!(account.b_stale_state, "v13 b-stale state reachable");
    assert_eq!(group.b_stale_account_count, 1);

    group.clear_account_b_stale(&mut account).unwrap();
    group.clear_account_b_stale(&mut account).unwrap();
    kani::cover!(!account.b_stale_state, "v13 b-stale clear reachable");
    assert_eq!(group.b_stale_account_count, 0);

    group
        .attach_leg(&mut account, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();
    group.mark_leg_b_stale(&mut account, 0).unwrap();
    group.mark_leg_b_stale(&mut account, 0).unwrap();
    kani::cover!(
        account.b_stale_state && account.legs[0].b_stale,
        "v13 active b-stale leg reachable"
    );
    assert_eq!(group.b_stale_account_count, 1);

    assert_eq!(
        group.clear_account_b_stale(&mut account),
        Err(V13Error::BStale)
    );
    assert!(account.b_stale_state);
    assert!(account.legs[0].b_stale);
    assert_eq!(group.b_stale_account_count, 1);
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

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_account_equity_rejects_malformed_fee_credits() {
    let malformed_positive: bool = kani::any();
    let (market, account_id, owner) = symbolic_ids();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    account.capital = 100;
    account.fee_credits = if malformed_positive { 1 } else { i128::MIN };

    kani::cover!(
        malformed_positive,
        "v13 positive fee credit corruption reachable"
    );
    kani::cover!(
        !malformed_positive,
        "v13 i128 min fee credit corruption reachable"
    );
    assert!(account_equity(&account).is_err());
}

#[kani::proof]
#[kani::unwind(10)]
#[kani::solver(cadical)]
fn proof_v13_account_equity_rejects_capital_above_i128_max() {
    let (market, account_id, owner) = symbolic_ids();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    account.capital = i128::MAX as u128 + 1;

    kani::cover!(
        account.capital > i128::MAX as u128,
        "v13 capital overflow equity path reachable"
    );
    assert_eq!(account_equity(&account), Err(V13Error::ArithmeticOverflow));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_account_shape_rejects_malformed_persistent_economic_state() {
    let dirty_case: u8 = kani::any();
    kani::assume(dirty_case < 4);
    let (market, account_id, owner) = symbolic_ids();
    let group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));

    let expected = match dirty_case {
        0 => {
            account.pnl = i128::MIN;
            V13Error::ArithmeticOverflow
        }
        1 => {
            account.fee_credits = 1;
            V13Error::InvalidLeg
        }
        2 => {
            account.fee_credits = i128::MIN;
            V13Error::ArithmeticOverflow
        }
        _ => {
            account.pnl = 1;
            account.reserved_pnl = 2;
            V13Error::InvalidLeg
        }
    };

    kani::cover!(dirty_case == 0, "v13 shape rejects i128 min pnl");
    kani::cover!(dirty_case == 1, "v13 shape rejects positive fee credit");
    kani::cover!(dirty_case == 2, "v13 shape rejects i128 min fee credit");
    kani::cover!(dirty_case == 3, "v13 shape rejects over-reserved pnl");
    assert_eq!(group.validate_account_shape(&account), Err(expected));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_flat_account_equity_is_exact_capital_plus_pnl_minus_fee_debt() {
    let capital: u16 = kani::any();
    let pnl: i16 = kani::any();
    let debt: u16 = kani::any();
    kani::assume(capital <= 10_000);
    kani::assume(debt <= 10_000);
    let (market, account_id, owner) = symbolic_ids();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    account.capital = capital as u128;
    account.pnl = pnl as i128;
    account.fee_credits = -(debt as i128);

    let expected = (capital as i128) + (pnl as i128) - (debt as i128);
    let actual = account_equity(&account).unwrap();

    kani::cover!(pnl < 0, "v13 flat negative pnl equity branch reachable");
    kani::cover!(pnl >= 0, "v13 flat nonnegative pnl equity branch reachable");
    kani::cover!(debt > 0, "v13 flat account fee debt branch reachable");
    assert_eq!(actual, expected);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_authoritatively_flat_account_never_receives_b_loss() {
    let b_long: u8 = kani::any();
    let b_short: u8 = kani::any();
    let budget: u8 = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 100).unwrap();
    group.assets[0].b_long_num = b_long as u128;
    group.assets[0].b_short_num = b_short as u128;

    let before_account = account;
    let before_count = group.b_stale_account_count;
    let outcome = group
        .settle_account_side_effects_not_atomic(&mut account, budget as u128)
        .unwrap();

    kani::cover!(
        b_long > 0 || b_short > 0,
        "v13 flat account with nonzero side B accumulator reachable"
    );
    assert_eq!(outcome, PermissionlessProgressOutcomeV13::AccountCurrent);
    assert_eq!(account.active_bitmap, 0);
    assert_eq!(account.pnl, before_account.pnl);
    assert_eq!(account.capital, before_account.capital);
    assert_eq!(account.b_stale_state, before_account.b_stale_state);
    assert_eq!(group.b_stale_account_count, before_count);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_public_config_rejects_invalid_user_fund_shapes() {
    let case: u8 = kani::any();
    kani::assume(case < 11);
    let (market, _, _) = symbolic_ids();
    let mut cfg = V13Config::public_user_fund(1, 0, 1);
    match case {
        0 => cfg.max_portfolio_assets = 0,
        1 => cfg.h_max = 0,
        2 => cfg.h_min = 2,
        3 => cfg.min_nonzero_mm_req = cfg.min_nonzero_im_req,
        4 => cfg.permissionless_recovery_enabled = false,
        5 => cfg.public_b_chunk_atoms = 0,
        6 => cfg.stale_certificate_penalty_enabled = false,
        7 => cfg.full_refresh_required_for_favorable_actions = false,
        8 => cfg.public_liveness_profile_crank_forward = false,
        9 => cfg.max_account_b_settlement_chunks = 0,
        _ => cfg.max_bankrupt_close_chunks = 0,
    }

    kani::cover!(case == 0, "v13 zero portfolio width rejected");
    kani::cover!(case == 1, "v13 zero hmax rejected");
    kani::cover!(case == 2, "v13 hmin above hmax rejected");
    kani::cover!(case == 3, "v13 invalid margin floor ordering rejected");
    kani::cover!(case == 4, "v13 disabled recovery rejected");
    kani::cover!(case == 5, "v13 zero B chunk budget rejected");
    kani::cover!(case == 6, "v13 disabled stale certificate penalty rejected");
    kani::cover!(case == 7, "v13 disabled required full refresh rejected");
    kani::cover!(case == 8, "v13 disabled crank-forward profile rejected");
    kani::cover!(case == 9, "v13 zero account B chunk cap rejected");
    kani::cover!(case == 10, "v13 zero bankrupt close chunk cap rejected");
    assert_eq!(
        MarketGroupV13::new(market, cfg),
        Err(V13Error::InvalidConfig)
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_permissionless_recovery_declares_reason_or_fails_closed() {
    let reason_case: u8 = kani::any();
    kani::assume(reason_case < 8);
    let enabled: bool = kani::any();
    let reason = match reason_case {
        0 => PermissionlessRecoveryReasonV13::BelowProgressFloor,
        1 => PermissionlessRecoveryReasonV13::BlockedSegmentHeadroomOrRepresentability,
        2 => PermissionlessRecoveryReasonV13::AccountBSettlementCannotProgress,
        3 => PermissionlessRecoveryReasonV13::BIndexHeadroomExhausted,
        4 => PermissionlessRecoveryReasonV13::ActiveBankruptCloseCannotProgress,
        5 => PermissionlessRecoveryReasonV13::ExplicitLossOrDustAuditOverflow,
        6 => PermissionlessRecoveryReasonV13::OracleOrTargetUnavailableByAuthenticatedPolicy,
        _ => PermissionlessRecoveryReasonV13::CounterOrEpochOverflowDeclaredRecovery,
    };
    let (market, _, _) = symbolic_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    group.config.permissionless_recovery_enabled = enabled;

    let before_mode = group.mode;
    let before_vault = group.vault;
    let before_c_tot = group.c_tot;
    let before_insurance = group.insurance;
    let result = group.declare_permissionless_recovery(reason);

    kani::cover!(
        enabled,
        "v13 permissionless recovery enabled path reachable"
    );
    kani::cover!(
        !enabled,
        "v13 permissionless recovery disabled path reachable"
    );
    kani::cover!(
        reason_case == 0,
        "v13 permissionless recovery first reason reachable"
    );
    kani::cover!(
        reason_case == 7,
        "v13 permissionless recovery last reason reachable"
    );

    if enabled {
        assert_eq!(
            result,
            Ok(PermissionlessProgressOutcomeV13::RecoveryDeclared(reason))
        );
        assert_eq!(group.recovery_reason, Some(reason));
    } else {
        assert_eq!(result, Err(V13Error::InvalidConfig));
        assert_eq!(group.recovery_reason, None);
    }
    assert_eq!(before_mode, MarketModeV13::Live);
    assert_eq!(group.mode, before_mode);
    assert_eq!(group.vault, before_vault);
    assert_eq!(group.c_tot, before_c_tot);
    assert_eq!(group.insurance, before_insurance);
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v13_permissionless_crank_recovery_declaration_is_accounting_neutral() {
    let reason_case: u8 = kani::any();
    kani::assume(reason_case < 8);
    let reason = match reason_case {
        0 => PermissionlessRecoveryReasonV13::BelowProgressFloor,
        1 => PermissionlessRecoveryReasonV13::BlockedSegmentHeadroomOrRepresentability,
        2 => PermissionlessRecoveryReasonV13::AccountBSettlementCannotProgress,
        3 => PermissionlessRecoveryReasonV13::BIndexHeadroomExhausted,
        4 => PermissionlessRecoveryReasonV13::ActiveBankruptCloseCannotProgress,
        5 => PermissionlessRecoveryReasonV13::ExplicitLossOrDustAuditOverflow,
        6 => PermissionlessRecoveryReasonV13::OracleOrTargetUnavailableByAuthenticatedPolicy,
        _ => PermissionlessRecoveryReasonV13::CounterOrEpochOverflowDeclaredRecovery,
    };
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 100).unwrap();
    group.attach_leg(&mut account, 0, SideV13::Long, 1).unwrap();

    let account_before = account;
    let vault_before = group.vault;
    let c_tot_before = group.c_tot;
    let insurance_before = group.insurance;
    let pnl_pos_before = group.pnl_pos_tot;
    let asset_before = group.assets[0];
    let slot_last_before = group.slot_last;
    let current_slot_before = group.current_slot;
    let outcome = group.permissionless_crank_not_atomic(
        &mut account,
        PermissionlessCrankRequestV13 {
            now_slot: current_slot_before + 1,
            asset_index: 0,
            effective_price: 2,
            funding_rate_e9: 0,
            action: PermissionlessCrankActionV13::Recover(reason),
        },
        &[1; V13_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(
        reason_case == 0,
        "v13 recovery-crank first reason reachable"
    );
    kani::cover!(reason_case == 7, "v13 recovery-crank last reason reachable");
    assert_eq!(
        outcome,
        Ok(PermissionlessProgressOutcomeV13::RecoveryDeclared(reason))
    );
    assert_eq!(group.recovery_reason, Some(reason));
    assert_eq!(account, account_before);
    assert_eq!(group.vault, vault_before);
    assert_eq!(group.c_tot, c_tot_before);
    assert_eq!(group.insurance, insurance_before);
    assert_eq!(group.pnl_pos_tot, pnl_pos_before);
    assert_eq!(group.assets[0], asset_before);
    assert_eq!(group.slot_last, slot_last_before);
    assert_eq!(group.current_slot, current_slot_before);
    assert_eq!(group.mode, MarketModeV13::Live);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_public_config_accepts_full_margin_loss_only_envelope() {
    let (market, _, _) = symbolic_ids();
    let cfg = V13Config::public_user_fund(1, 0, 1);

    kani::cover!(
        cfg.maintenance_margin_bps == 10_000 && cfg.max_price_move_bps_per_slot == 10_000,
        "v13 full-margin one-segment loss envelope reachable"
    );
    assert!(MarketGroupV13::new(market, cfg).is_ok());
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_public_config_rejects_price_funding_envelope_breach() {
    let (market, _, _) = symbolic_ids();
    let mut cfg = tight_envelope_config();
    cfg.max_price_move_bps_per_slot = 10;

    kani::cover!(
        cfg.max_price_move_bps_per_slot == 10,
        "v13 price/funding envelope breach rejected"
    );
    assert_eq!(
        MarketGroupV13::new(market, cfg),
        Err(V13Error::InvalidConfig)
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_public_config_rejects_liquidation_fee_envelope_breach() {
    let (market, _, _) = symbolic_ids();
    let mut cfg = tight_envelope_config();
    cfg.liquidation_fee_bps = 400;

    kani::cover!(
        cfg.liquidation_fee_bps == 400,
        "v13 liquidation-fee envelope breach rejected"
    );
    assert_eq!(
        MarketGroupV13::new(market, cfg),
        Err(V13Error::InvalidConfig)
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_public_config_rejects_funding_headroom_breach() {
    let (market, _, _) = symbolic_ids();
    let mut cfg = tight_envelope_config();
    cfg.max_accrual_dt_slots = 1_000_000_000;
    cfg.min_funding_lifetime_slots = 1_000_000_000;

    kani::cover!(
        cfg.max_accrual_dt_slots == 1_000_000_000,
        "v13 funding K/F headroom breach rejected"
    );
    assert_eq!(
        MarketGroupV13::new(market, cfg),
        Err(V13Error::InvalidConfig)
    );
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v13_public_config_accepts_capped_liquidation_fee_envelope() {
    let (market, _, _) = symbolic_ids();
    let mut cfg = tight_envelope_config();
    cfg.liquidation_fee_bps = 10_000;
    cfg.liquidation_fee_cap = 1;

    kani::cover!(
        cfg.liquidation_fee_bps == 10_000 && cfg.liquidation_fee_cap == 1,
        "v13 capped liquidation fee envelope reachable"
    );
    assert!(MarketGroupV13::new(market, cfg).is_ok());
}

#[kani::proof]
#[kani::unwind(45)]
#[kani::solver(cadical)]
fn proof_v13_min_nonzero_initial_floor_is_in_health_certificate() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    group.config.min_nonzero_mm_req = 49;
    group.config.min_nonzero_im_req = 50;
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 49).unwrap();
    group.attach_leg(&mut account, 0, SideV13::Long, 1).unwrap();
    group
        .full_account_refresh(&mut account, &[1; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    kani::cover!(
        account.health_cert.certified_initial_req == 50,
        "v13 tiny nonzero leg gets min initial floor"
    );
    assert_eq!(account.health_cert.certified_equity, 49);
    assert_eq!(account.health_cert.certified_initial_req, 50);
    assert!(
        account.health_cert.certified_equity < account.health_cert.certified_initial_req as i128
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_deposit_then_withdraw_roundtrip_preserves_accounting() {
    let amount: u16 = kani::any();
    kani::assume(amount > 0);
    kani::assume(amount <= 1_000);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));

    group
        .deposit_not_atomic(&mut account, amount as u128)
        .unwrap();
    assert_eq!(account.capital, amount as u128);
    assert_eq!(group.c_tot, amount as u128);
    assert_eq!(group.vault, amount as u128);

    group
        .withdraw_not_atomic(
            &mut account,
            amount as u128,
            &[1; V13_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();
    assert_eq!(account.capital, 0);
    assert_eq!(group.c_tot, 0);
    assert_eq!(group.vault, 0);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(45)]
#[kani::solver(cadical)]
fn proof_v13_deposit_does_not_draw_insurance_or_sweep_loss_bearing_account() {
    let amount: u16 = kani::any();
    let fee_debt: u8 = kani::any();
    kani::assume(amount > 0);
    kani::assume(amount <= 1_000);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));

    group.vault = 10;
    group.insurance = 10;
    group
        .attach_leg(&mut account, 0, SideV13::Long, 10)
        .unwrap();
    account.pnl = -10_000;
    account.fee_credits = -(fee_debt as i128);

    let insurance_before = group.insurance;
    let pnl_before = account.pnl;
    let fee_credits_before = account.fee_credits;
    let leg_before = account.legs[0];
    let oi_before = group.assets[0].oi_eff_long_q;

    group
        .deposit_not_atomic(&mut account, amount as u128)
        .unwrap();

    kani::cover!(fee_debt > 0, "v13 deposit with fee debt reachable");
    assert_eq!(group.insurance, insurance_before);
    assert_eq!(account.pnl, pnl_before);
    assert_eq!(account.fee_credits, fee_credits_before);
    assert_eq!(account.legs[0], leg_before);
    assert_eq!(group.assets[0].oi_eff_long_q, oi_before);
    assert_eq!(account.capital, amount as u128);
    assert_eq!(group.c_tot, amount as u128);
    assert_eq!(group.vault, 10 + amount as u128);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(45)]
#[kani::solver(cadical)]
fn proof_v13_deposit_never_sweeps_fee_debt_even_when_flat_and_nonnegative() {
    let amount: u16 = kani::any();
    let fee_debt: u8 = kani::any();
    let pnl: u8 = kani::any();
    kani::assume(amount > 0);
    kani::assume(amount <= 1_000);
    kani::assume(fee_debt > 0);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    account.pnl = pnl as i128;
    account.fee_credits = -(fee_debt as i128);

    let pnl_before = account.pnl;
    let fee_credits_before = account.fee_credits;
    group
        .deposit_not_atomic(&mut account, amount as u128)
        .unwrap();

    kani::cover!(
        pnl_before > 0 && fee_debt > 0,
        "v13 flat nonnegative deposit with fee debt reachable"
    );
    assert_eq!(account.pnl, pnl_before);
    assert_eq!(account.fee_credits, fee_credits_before);
    assert_eq!(account.capital, amount as u128);
    assert_eq!(group.c_tot, amount as u128);
    assert_eq!(group.vault, amount as u128);
    assert_eq!(group.insurance, 0);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_partial_withdraw_can_leave_small_remainder() {
    let remainder: u16 = kani::any();
    kani::assume(remainder <= 1_000);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    let deposit = remainder as u128 + 1;
    group.deposit_not_atomic(&mut account, deposit).unwrap();

    group
        .withdraw_not_atomic(&mut account, 1, &[1; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    kani::cover!(remainder == 0, "v13 partial withdraw leaves zero remainder");
    kani::cover!(
        remainder > 0,
        "v13 partial withdraw leaves nonzero remainder"
    );
    assert_eq!(account.capital, remainder as u128);
    assert_eq!(group.c_tot, remainder as u128);
    assert_eq!(group.vault, remainder as u128);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(45)]
#[kani::solver(cadical)]
fn proof_v13_over_withdraw_rejects_before_any_accounting_mutation() {
    let capital: u16 = kani::any();
    kani::assume(capital > 0);
    kani::assume(capital <= 1_000);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    group
        .deposit_not_atomic(&mut account, capital as u128)
        .unwrap();
    let capital_before = account.capital;
    let pnl_before = account.pnl;
    let fee_credits_before = account.fee_credits;
    let active_bitmap_before = account.active_bitmap;
    let legs_before = account.legs;
    let vault_before = group.vault;
    let c_tot_before = group.c_tot;
    let insurance_before = group.insurance;

    let result = group.withdraw_not_atomic(
        &mut account,
        capital as u128 + 1,
        &[1; V13_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(capital > 0, "v13 over-withdraw rejection path reachable");
    assert_eq!(result, Err(V13Error::LockActive));
    assert_eq!(account.capital, capital_before);
    assert_eq!(account.pnl, pnl_before);
    assert_eq!(account.fee_credits, fee_credits_before);
    assert_eq!(account.active_bitmap, active_bitmap_before);
    assert_eq!(account.legs, legs_before);
    assert_eq!(group.vault, vault_before);
    assert_eq!(group.c_tot, c_tot_before);
    assert_eq!(group.insurance, insurance_before);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_multiple_deposits_aggregate_c_tot_and_vault() {
    let amount_a: u16 = kani::any();
    let amount_b: u16 = kani::any();
    kani::assume(amount_a <= 1_000);
    kani::assume(amount_b <= 1_000);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account_a =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    let mut account_b =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, [4; 32], owner));

    group
        .deposit_not_atomic(&mut account_a, amount_a as u128)
        .unwrap();
    group
        .deposit_not_atomic(&mut account_b, amount_b as u128)
        .unwrap();

    let expected = amount_a as u128 + amount_b as u128;
    kani::cover!(expected > 0, "v13 nonzero aggregate deposit reachable");
    assert_eq!(group.c_tot, account_a.capital + account_b.capital);
    assert_eq!(group.c_tot, expected);
    assert_eq!(group.vault, expected);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_close_portfolio_account_requires_clean_local_state() {
    let dirty_case: u8 = kani::any();
    kani::assume(dirty_case < 6);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let clean = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    group.create_portfolio_account(&clean).unwrap();
    assert_eq!(group.materialized_portfolio_count, 1);

    let mut dirty = clean;
    match dirty_case {
        0 => dirty.capital = 1,
        1 => dirty.pnl = 1,
        2 => {
            dirty.pnl = 1;
            dirty.reserved_pnl = 1;
        }
        3 => dirty.fee_credits = -1,
        4 => dirty.stale_state = true,
        _ => dirty.b_stale_state = true,
    }
    kani::cover!(dirty_case == 0, "v13 close rejects capital");
    kani::cover!(dirty_case == 1, "v13 close rejects pnl");
    kani::cover!(dirty_case == 2, "v13 close rejects reserved pnl");
    kani::cover!(dirty_case == 3, "v13 close rejects fee debt");
    kani::cover!(dirty_case == 4, "v13 close rejects stale account");
    kani::cover!(dirty_case == 5, "v13 close rejects b-stale account");
    assert_eq!(
        group.close_portfolio_account(&dirty),
        Err(V13Error::LockActive)
    );
    assert_eq!(group.materialized_portfolio_count, 1);

    group.close_portfolio_account(&clean).unwrap();
    assert_eq!(group.materialized_portfolio_count, 0);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_risk_notional_flat_zero_and_monotone_in_price() {
    let abs_pos_q: u16 = kani::any();
    let p1: u16 = kani::any();
    let extra: u16 = kani::any();
    kani::assume(abs_pos_q <= 1_000);
    kani::assume(p1 > 0);
    kani::assume(p1 <= 1_000);
    kani::assume(extra <= 1_000);
    let p2 = p1 as u64 + extra as u64;

    assert_eq!(percolator::v13::risk_notional_ceil(0, p2), Ok(0));
    let n1 = percolator::v13::risk_notional_ceil(abs_pos_q as u128, p1 as u64).unwrap();
    let n2 = percolator::v13::risk_notional_ceil(abs_pos_q as u128, p2).unwrap();
    kani::cover!(
        abs_pos_q > 0 && extra > 0,
        "v13 risk notional monotone branch"
    );
    assert!(n2 >= n1);
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
fn proof_v13_configured_portfolio_width_rejects_out_of_range_leg() {
    let active_bit: bool = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    account.legs[1] = PortfolioLegV13 {
        active: true,
        side: SideV13::Long,
        basis_pos_q: 1,
        a_basis: ADL_ONE,
        k_snap: 0,
        f_snap: 0,
        epoch_snap: 0,
        loss_weight: 1,
        b_snap: 0,
        b_rem: 0,
        b_epoch_snap: 0,
        b_stale: false,
        stale: false,
    };
    if active_bit {
        account.active_bitmap |= 1 << 1;
    }

    kani::cover!(active_bit, "v13 out-of-range leg with bitmap reachable");
    kani::cover!(!active_bit, "v13 out-of-range hidden leg reachable");
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
#[kani::unwind(45)]
#[kani::solver(cadical)]
fn proof_v13_bilateral_oi_decomposition_counts_long_short_pair() {
    let size_q = 3u128;
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut a = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    let mut b = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, [4; 32], owner));

    group
        .attach_leg(&mut a, 0, SideV13::Long, size_q as i128)
        .unwrap();
    group
        .attach_leg(&mut b, 0, SideV13::Short, -(size_q as i128))
        .unwrap();

    kani::cover!(true, "v13 bilateral OI proof covers long-short pair");
    assert_eq!(group.assets[0].oi_eff_long_q, size_q);
    assert_eq!(group.assets[0].oi_eff_short_q, size_q);
    assert_eq!(group.assets[0].stored_pos_count_long, 1);
    assert_eq!(group.assets[0].stored_pos_count_short, 1);
    assert_eq!(a.active_bitmap, 1);
    assert_eq!(b.active_bitmap, 1);
}

#[kani::proof]
#[kani::unwind(45)]
#[kani::solver(cadical)]
fn proof_v13_bilateral_oi_decomposition_counts_short_long_pair() {
    let size_q = 3u128;
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut a = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    let mut b = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, [4; 32], owner));

    group
        .attach_leg(&mut a, 0, SideV13::Short, -(size_q as i128))
        .unwrap();
    group
        .attach_leg(&mut b, 0, SideV13::Long, size_q as i128)
        .unwrap();

    kani::cover!(true, "v13 bilateral OI proof covers short-long pair");
    assert_eq!(group.assets[0].oi_eff_long_q, size_q);
    assert_eq!(group.assets[0].oi_eff_short_q, size_q);
    assert_eq!(group.assets[0].stored_pos_count_long, 1);
    assert_eq!(group.assets[0].stored_pos_count_short, 1);
    assert_eq!(a.active_bitmap, 1);
    assert_eq!(b.active_bitmap, 1);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_oversize_position_rejected_before_oi_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));

    let result = group.attach_leg(
        &mut account,
        0,
        SideV13::Long,
        (MAX_POSITION_ABS_Q + 1) as i128,
    );

    assert_eq!(result, Err(V13Error::InvalidLeg));
    assert_eq!(account.active_bitmap, 0);
    assert_eq!(group.assets[0].oi_eff_long_q, 0);
    assert_eq!(group.assets[0].stored_pos_count_long, 0);
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
    let case: u8 = kani::any();
    let deficit: u8 = kani::any();
    let gross_loss: u8 = kani::any();
    kani::assume(case <= 3);
    kani::assume(deficit <= 5);
    kani::assume(gross_loss <= 5);
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
    before.health_cert.certified_worst_case_loss = gross_loss as u128;
    after.health_cert.certified_worst_case_loss = gross_loss as u128;

    match case {
        0 => {}
        1 => after.health_cert.certified_liq_deficit = deficit as u128 + 1,
        2 => after.stale_state = true,
        _ => after.health_cert.certified_worst_case_loss = gross_loss as u128 + 1,
    }

    kani::cover!(case == 0, "v13 equal risk score non-progress reachable");
    kani::cover!(case == 1, "v13 worse deficit non-progress reachable");
    kani::cover!(case == 2, "v13 stale-penalty non-progress reachable");
    kani::cover!(case == 3, "v13 worse gross-loss non-progress reachable");

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
fn proof_v13_global_residual_is_not_account_health_proof() {
    let residual_units: u8 = kani::any();
    kani::assume(residual_units > 0);
    kani::assume(residual_units <= 5);
    let residual = residual_units as u128;
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    account.pnl = residual as i128;
    account.reserved_pnl = 0;
    group.pnl_pos_tot = residual;
    group.pnl_matured_pos_tot = residual;
    group.vault = group.c_tot + group.insurance + residual;
    let before_group = group;
    let before_account = account;

    let result = group.convert_released_pnl_to_capital_not_atomic(&mut account);

    kani::cover!(
        residual > 0 && !account.health_cert.valid,
        "v13 aggregate residual with stale account certificate reachable"
    );
    assert_eq!(result, Err(V13Error::Stale));
    assert_eq!(group, before_group);
    assert_eq!(account, before_account);
}

#[kani::proof]
#[kani::unwind(20)]
#[kani::solver(cadical)]
fn proof_v13_public_invariants_reject_broken_senior_claim_conservation() {
    let vault_units: u8 = kani::any();
    let c_units: u8 = kani::any();
    let i_units: u8 = kani::any();
    kani::assume(vault_units <= 10);
    kani::assume(c_units <= 10);
    kani::assume(i_units <= 10);
    kani::assume((c_units as u16) + (i_units as u16) > vault_units as u16);

    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    group.vault = vault_units as u128;
    group.c_tot = c_units as u128;
    group.insurance = i_units as u128;

    kani::cover!(
        group.c_tot <= group.vault && group.insurance <= group.vault,
        "v13 senior sum overflow can violate conservation even when each claim is individually within vault"
    );
    assert_eq!(
        group.assert_public_invariants(),
        Err(V13Error::InvalidConfig)
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_cross_margin_equity_counts_collateral_once_and_score_uses_full_envelope() {
    let capital_units: u8 = kani::any();
    let debt_units: u8 = kani::any();
    let certified_loss_units: u8 = kani::any();
    kani::assume(capital_units <= 5);
    kani::assume(debt_units <= 5);
    kani::assume(certified_loss_units > 0);
    kani::assume(certified_loss_units <= 5);
    let capital = capital_units as u128;
    let debt = debt_units as i128;
    let certified_loss = certified_loss_units as u128;
    let (market, account_id, owner) = concrete_ids();
    let group = MarketGroupV13::new(market, V13Config::public_user_fund(2, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    account.capital = capital;
    account.fee_credits = -debt;
    account.active_bitmap = 0b11;
    account.legs[0] = PortfolioLegV13 {
        active: true,
        side: SideV13::Long,
        basis_pos_q: POS_SCALE as i128,
        a_basis: ADL_ONE,
        k_snap: 0,
        f_snap: 0,
        epoch_snap: 0,
        loss_weight: POS_SCALE,
        b_snap: 0,
        b_rem: 0,
        b_epoch_snap: 0,
        b_stale: false,
        stale: false,
    };
    account.legs[1] = PortfolioLegV13 {
        active: true,
        side: SideV13::Short,
        basis_pos_q: -(POS_SCALE as i128),
        a_basis: ADL_ONE,
        k_snap: 0,
        f_snap: 0,
        epoch_snap: 0,
        loss_weight: POS_SCALE,
        b_snap: 0,
        b_rem: 0,
        b_epoch_snap: 0,
        b_stale: false,
        stale: false,
    };

    let equity = account_equity(&account).unwrap();
    let expected = (capital as i128) - debt;

    kani::cover!(
        account.active_bitmap == 0b11,
        "v13 two active legs reachable for single-collateral equity"
    );
    assert_eq!(equity, expected);

    let mut cert_account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    cert_account.health_cert.valid = true;
    cert_account.health_cert.certified_worst_case_loss = certified_loss;
    let score = group.risk_score(&cert_account).unwrap();

    kani::cover!(
        certified_loss > 1,
        "v13 full certified loss envelope reaches risk score"
    );
    assert_eq!(score.gross_risk_notional, certified_loss);
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
fn proof_v13_b_stale_blocks_refresh_and_favorable_actions() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));

    group
        .full_account_refresh(&mut account, &[1; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(group.ensure_favorable_action_allowed(&account), Ok(()));

    group.mark_account_b_stale(&mut account).unwrap();
    kani::cover!(
        account.b_stale_state && !account.health_cert.valid,
        "v13 b-stale invalidates prior health certificate"
    );

    assert_eq!(
        group.full_account_refresh(&mut account, &[1; V13_MAX_PORTFOLIO_ASSETS_N]),
        Err(V13Error::BStale)
    );
    assert_eq!(
        group.ensure_favorable_action_allowed(&account),
        Err(V13Error::LockActive)
    );
    assert!(account.b_stale_state);
    assert_eq!(group.b_stale_account_count, 1);
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
fn proof_v13_side_reset_finalize_requires_prior_epoch_positions_clear() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    group.attach_leg(&mut account, 0, SideV13::Long, 1).unwrap();
    group.assets[0].oi_eff_long_q = 0;

    group.begin_full_drain_reset(0, SideV13::Long).unwrap();
    kani::cover!(
        group.assets[0].stored_pos_count_long != 0,
        "v13 reset pending with prior-epoch stored position reachable"
    );
    assert_eq!(
        group.finalize_ready_reset_side(0, SideV13::Long),
        Err(V13Error::Stale)
    );

    group.clear_leg(&mut account, 0).unwrap();
    assert_eq!(group.finalize_ready_reset_side(0, SideV13::Long), Ok(()));
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
fn proof_v13_permissionless_crank_does_not_require_full_market_scan() {
    let stale_count: u16 = kani::any();
    let b_stale_count: u16 = kani::any();
    let negative_count: u16 = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 1).unwrap();
    group.materialized_portfolio_count = 1 + stale_count as u64;
    group.stale_certificate_count = stale_count as u64;
    group.b_stale_account_count = b_stale_count as u64;
    group.negative_pnl_account_count = negative_count as u64;
    let before_materialized = group.materialized_portfolio_count;
    let before_stale = group.stale_certificate_count;
    let before_b_stale = group.b_stale_account_count;
    let before_negative = group.negative_pnl_account_count;

    let out = group
        .permissionless_crank_not_atomic(
            &mut account,
            PermissionlessCrankRequestV13 {
                now_slot: 0,
                asset_index: 0,
                effective_price: 1,
                funding_rate_e9: 0,
                action: PermissionlessCrankActionV13::Refresh,
            },
            &[1; V13_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    kani::cover!(
        stale_count > 0 || b_stale_count > 0 || negative_count > 0,
        "v13 permissionless hinted progress ignores unrelated global account counters"
    );
    assert_eq!(out, PermissionlessProgressOutcomeV13::AccountCurrent);
    assert!(account.health_cert.valid);
    assert_eq!(group.materialized_portfolio_count, before_materialized);
    assert_eq!(group.stale_certificate_count, before_stale);
    assert_eq!(group.b_stale_account_count, before_b_stale);
    assert_eq!(group.negative_pnl_account_count, before_negative);
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v13_permissionless_refresh_returns_partial_b_progress_without_accrual() {
    let larger_target: bool = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let mut cfg = V13Config::public_user_fund(1, 0, 1);
    cfg.public_b_chunk_atoms = 1;
    let mut group = MarketGroupV13::new(market, cfg).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));

    group.deposit_not_atomic(&mut account, 100).unwrap();
    group.attach_leg(&mut account, 0, SideV13::Long, 1).unwrap();
    group.assets[0].b_long_num = if larger_target { 3 } else { 2 };
    let outcome = group.permissionless_crank_not_atomic(
        &mut account,
        PermissionlessCrankRequestV13 {
            now_slot: 1,
            asset_index: 0,
            effective_price: 1,
            funding_rate_e9: 0,
            action: PermissionlessCrankActionV13::Refresh,
        },
        &[1; V13_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(
        !larger_target,
        "v13 permissionless refresh partial B target two"
    );
    kani::cover!(
        larger_target,
        "v13 permissionless refresh partial B target three"
    );
    assert!(matches!(
        outcome,
        Ok(PermissionlessProgressOutcomeV13::AccountBChunk(_))
    ));
    assert!(account.legs[0].b_stale);
    assert!(account.legs[0].b_snap > 0);
    assert!(account.legs[0].b_snap < group.assets[0].b_long_num);
    assert_eq!(group.slot_last, 0);
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v13_worst_case_hinted_progress_actions_are_total_and_bounded() {
    let case: u8 = kani::any();
    kani::assume(case < 4);
    let (market, account_id, owner) = concrete_ids();
    let base_req = PermissionlessCrankRequestV13 {
        now_slot: 0,
        asset_index: 0,
        effective_price: 1,
        funding_rate_e9: 0,
        action: PermissionlessCrankActionV13::Refresh,
    };

    match case {
        0 => {
            let mut group =
                MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
            let mut account =
                PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
            group.deposit_not_atomic(&mut account, 1).unwrap();
            let outcome = group.permissionless_crank_not_atomic(
                &mut account,
                base_req,
                &[1; V13_MAX_PORTFOLIO_ASSETS_N],
            );
            kani::cover!(true, "v13 hinted refresh-current branch reachable");
            assert_eq!(
                outcome,
                Ok(PermissionlessProgressOutcomeV13::AccountCurrent)
            );
            assert!(account.health_cert.valid);
        }
        1 => {
            let mut cfg = V13Config::public_user_fund(1, 0, 1);
            cfg.public_b_chunk_atoms = 1;
            let mut group = MarketGroupV13::new(market, cfg).unwrap();
            let mut account =
                PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
            group.attach_leg(&mut account, 0, SideV13::Long, 1).unwrap();
            group.assets[0].b_long_num = 2;
            let outcome = group.permissionless_crank_not_atomic(
                &mut account,
                PermissionlessCrankRequestV13 {
                    action: PermissionlessCrankActionV13::SettleB { asset_index: 0 },
                    ..base_req
                },
                &[1; V13_MAX_PORTFOLIO_ASSETS_N],
            );
            kani::cover!(true, "v13 hinted settle-B branch reachable");
            match outcome {
                Ok(PermissionlessProgressOutcomeV13::AccountBChunk(chunk)) => {
                    assert_eq!(chunk.delta_b, 1);
                    assert_eq!(chunk.remaining_after, 1);
                    assert!(account.b_stale_state);
                    assert_eq!(group.b_stale_account_count, 1);
                }
                _ => assert!(false),
            }
        }
        2 => {
            let mut group =
                MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
            let mut account =
                PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
            group
                .attach_leg(&mut account, 0, SideV13::Long, POS_SCALE as i128)
                .unwrap();
            let outcome = group.permissionless_crank_not_atomic(
                &mut account,
                PermissionlessCrankRequestV13 {
                    action: PermissionlessCrankActionV13::Liquidate(LiquidationRequestV13 {
                        asset_index: 0,
                        close_q: POS_SCALE,
                        fee_bps: 0,
                    }),
                    ..base_req
                },
                &[1; V13_MAX_PORTFOLIO_ASSETS_N],
            );
            kani::cover!(true, "v13 hinted liquidation branch reachable");
            assert_eq!(
                outcome,
                Ok(PermissionlessProgressOutcomeV13::AccountCurrent)
            );
            assert_eq!(account.active_bitmap, 0);
        }
        _ => {
            let mut group =
                MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
            let mut account =
                PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
            let reason = PermissionlessRecoveryReasonV13::BelowProgressFloor;
            let outcome = group.permissionless_crank_not_atomic(
                &mut account,
                PermissionlessCrankRequestV13 {
                    action: PermissionlessCrankActionV13::Recover(reason),
                    ..base_req
                },
                &[1; V13_MAX_PORTFOLIO_ASSETS_N],
            );
            kani::cover!(true, "v13 hinted recovery branch reachable");
            assert_eq!(
                outcome,
                Ok(PermissionlessProgressOutcomeV13::RecoveryDeclared(reason))
            );
            assert_eq!(group.recovery_reason, Some(reason));
        }
    }
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
fn proof_v13_funding_rate_above_cap_rejects_before_mutation() {
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    group.config.max_abs_funding_e9_per_slot = 1;
    let before = group.assets[0];

    let result = group.accrue_asset_to_not_atomic(0, 1, 1, 2, true);

    assert_eq!(result, Err(V13Error::InvalidConfig));
    assert_eq!(group.assets[0], before);
    assert_eq!(group.slot_last, 0);
    assert_eq!(group.current_slot, 0);
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
#[kani::unwind(60)]
#[kani::solver(cadical)]
fn proof_v13_trade_fee_conservation_and_oi_symmetry() {
    let fee_bps: u16 = kani::any();
    kani::assume(fee_bps <= 1_000);
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    group.config.max_trading_fee_bps = 1_000;
    let mut long = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    let mut short = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut long, 10_000).unwrap();
    group.deposit_not_atomic(&mut short, 10_000).unwrap();
    let vault_before = group.vault;
    let c_tot_before = group.c_tot;
    let insurance_before = group.insurance;

    let out = group
        .execute_trade_with_fee_not_atomic(
            &mut long,
            &mut short,
            TradeRequestV13 {
                asset_index: 0,
                size_q: POS_SCALE,
                exec_price: 100,
                fee_bps: fee_bps as u64,
            },
            &[100; V13_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    let expected_fee = if fee_bps == 0 {
        0
    } else {
        ((100u128 * fee_bps as u128) + 9_999) / 10_000
    };
    kani::cover!(fee_bps == 0, "v13 zero fee trade reachable");
    kani::cover!(expected_fee > 0, "v13 positive fee trade reachable");
    assert_eq!(out.notional, 100);
    assert_eq!(out.fee_a, expected_fee);
    assert_eq!(out.fee_b, expected_fee);
    assert_eq!(group.vault, vault_before);
    assert_eq!(group.insurance, insurance_before + expected_fee * 2);
    assert_eq!(group.c_tot, c_tot_before - expected_fee * 2);
    assert_eq!(group.assets[0].oi_eff_long_q, POS_SCALE);
    assert_eq!(group.assets[0].oi_eff_short_q, POS_SCALE);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(60)]
#[kani::solver(cadical)]
fn proof_v13_risk_increasing_trade_requires_initial_health_before_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut underfunded_long =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    let mut funded_short =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut funded_short, 10_000).unwrap();
    let before_group = group;
    let before_long = underfunded_long;
    let before_short = funded_short;

    let result = group.execute_trade_with_fee_not_atomic(
        &mut underfunded_long,
        &mut funded_short,
        TradeRequestV13 {
            asset_index: 0,
            size_q: POS_SCALE,
            exec_price: 100,
            fee_bps: 0,
        },
        &[100; V13_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(result, Err(V13Error::InvalidConfig));
    assert_eq!(group, before_group);
    assert_eq!(underfunded_long, before_long);
    assert_eq!(funded_short, before_short);
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v13_trade_hint_cannot_hide_toxic_portfolio_leg_on_other_asset() {
    let hidden_loss_units: u8 = kani::any();
    kani::assume((2..=5).contains(&hidden_loss_units));
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(2, 0, 1)).unwrap();
    let mut long = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    let mut short = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut long, 1).unwrap();
    group.deposit_not_atomic(&mut short, 1_000).unwrap();
    group
        .attach_leg(&mut long, 1, SideV13::Long, POS_SCALE as i128)
        .unwrap();
    group.assets[1].k_long = -((hidden_loss_units as i128) * (ADL_ONE as i128));
    let before_group = group;
    let before_long = long;
    let before_short = short;

    let result = group.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV13 {
            asset_index: 0,
            size_q: POS_SCALE,
            exec_price: 1,
            fee_bps: 0,
        },
        &[1; V13_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(
        hidden_loss_units > 1,
        "v13 trade hint with toxic unhinted active leg reachable"
    );
    assert!(result.is_err());
    assert_eq!(group, before_group);
    assert_eq!(long, before_long);
    assert_eq!(short, before_short);
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v13_sign_flip_trade_preserves_oi_symmetry_and_senior_accounting() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut flip_to_long =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    let mut flip_to_short =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut flip_to_long, 10_000).unwrap();
    group
        .deposit_not_atomic(&mut flip_to_short, 10_000)
        .unwrap();
    group
        .attach_leg(&mut flip_to_long, 0, SideV13::Short, -(POS_SCALE as i128))
        .unwrap();
    group
        .attach_leg(&mut flip_to_short, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();
    let vault_before = group.vault;
    let c_tot_before = group.c_tot;

    group
        .execute_trade_with_fee_not_atomic(
            &mut flip_to_long,
            &mut flip_to_short,
            TradeRequestV13 {
                asset_index: 0,
                size_q: 2 * POS_SCALE,
                exec_price: 1,
                fee_bps: 0,
            },
            &[1; V13_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    kani::cover!(true, "v13 sign-flip trade transition reachable");
    assert_eq!(flip_to_long.legs[0].side, SideV13::Long);
    assert_eq!(flip_to_long.legs[0].basis_pos_q, POS_SCALE as i128);
    assert_eq!(flip_to_short.legs[0].side, SideV13::Short);
    assert_eq!(flip_to_short.legs[0].basis_pos_q, -(POS_SCALE as i128));
    assert_eq!(group.assets[0].oi_eff_long_q, POS_SCALE);
    assert_eq!(group.assets[0].oi_eff_short_q, POS_SCALE);
    assert_eq!(group.assets[0].stored_pos_count_long, 1);
    assert_eq!(group.assets[0].stored_pos_count_short, 1);
    assert_eq!(group.vault, vault_before);
    assert_eq!(group.c_tot, c_tot_before);
    assert_eq!(group.assert_public_invariants(), Ok(()));
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
fn proof_v13_released_pnl_conversion_is_residual_bounded_and_conserves_vault() {
    let profit: u8 = kani::any();
    let residual: u8 = kani::any();
    kani::assume(profit <= 10);
    kani::assume(residual <= 10);
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));

    group.deposit_not_atomic(&mut account, 10).unwrap();
    account.pnl = profit as i128;
    group.pnl_pos_tot = profit as u128;
    group.pnl_matured_pos_tot = profit as u128;
    group.vault = group.c_tot + group.insurance + residual as u128;
    group
        .full_account_refresh(&mut account, &[1; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    let vault_before = group.vault;
    let c_tot_before = group.c_tot;
    let pnl_before = account.pnl;
    let expected = (profit as u128).min(residual as u128);
    let result = group.convert_released_pnl_to_capital_not_atomic(&mut account);

    kani::cover!(expected == 0, "v13 zero conversion branch reachable");
    kani::cover!(expected > 0, "v13 positive conversion branch reachable");
    if expected == 0 {
        if profit == 0 {
            assert_eq!(result, Ok(0));
        } else {
            assert_eq!(result, Err(V13Error::LockActive));
        }
        assert_eq!(group.vault, vault_before);
        assert_eq!(group.c_tot, c_tot_before);
        assert_eq!(account.capital, 10);
        assert_eq!(account.pnl, pnl_before);
    } else {
        let converted = result.unwrap();
        assert_eq!(converted, expected);
        assert_eq!(group.vault, vault_before);
        assert_eq!(group.c_tot, c_tot_before + expected);
        assert_eq!(account.capital, 10 + expected);
        assert_eq!(account.pnl, pnl_before - expected as i128);
    }
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v13_target_effective_lag_blocks_pnl_conversion_before_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 10).unwrap();
    group
        .attach_leg(&mut account, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();
    account.pnl = 10;
    group.pnl_pos_tot = 10;
    group.pnl_matured_pos_tot = 10;
    group.vault = group.vault.checked_add(10).unwrap();
    group.assets[0].effective_price = 100;
    group.assets[0].raw_oracle_target_price = 100;
    group
        .full_account_refresh(&mut account, &[100; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    group.assets[0].raw_oracle_target_price = 120;

    let vault_before = group.vault;
    let c_tot_before = group.c_tot;
    let pnl_pos_before = group.pnl_pos_tot;
    let matured_before = group.pnl_matured_pos_tot;
    let capital_before = account.capital;
    let pnl_before = account.pnl;
    let cert_before = account.health_cert;
    let result = group.convert_released_pnl_to_capital_not_atomic(&mut account);

    kani::cover!(
        account.active_bitmap != 0
            && group.assets[0].raw_oracle_target_price != group.assets[0].effective_price,
        "v13 target/effective lag conversion lock reachable"
    );
    assert_eq!(result, Err(V13Error::LockActive));
    assert_eq!(group.vault, vault_before);
    assert_eq!(group.c_tot, c_tot_before);
    assert_eq!(group.pnl_pos_tot, pnl_pos_before);
    assert_eq!(group.pnl_matured_pos_tot, matured_before);
    assert_eq!(account.capital, capital_before);
    assert_eq!(account.pnl, pnl_before);
    assert_eq!(account.health_cert, cert_before);
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
fn proof_v13_resolved_close_partial_b_settlement_makes_progress_without_closing() {
    let larger_target: bool = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let mut cfg = V13Config::public_user_fund(1, 0, 1);
    cfg.public_b_chunk_atoms = 1;
    let mut group = MarketGroupV13::new(market, cfg).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));

    group.deposit_not_atomic(&mut account, 100).unwrap();
    group.attach_leg(&mut account, 0, SideV13::Long, 1).unwrap();
    group.assets[0].b_long_num = if larger_target { 3 } else { 2 };
    group.resolve_market_not_atomic(10).unwrap();

    let outcome = group.close_resolved_account_not_atomic(&mut account, 1);

    kani::cover!(!larger_target, "v13 resolved close partial B target two");
    kani::cover!(larger_target, "v13 resolved close partial B target three");
    assert_eq!(outcome, Ok(ResolvedCloseOutcomeV13::ProgressOnly));
    assert!(account.legs[0].b_stale);
    assert!(account.legs[0].b_snap > 0);
    assert!(account.legs[0].b_snap < group.assets[0].b_long_num);
    assert_eq!(account.last_fee_slot, 0);
    assert_ne!(account.active_bitmap, 0);
    assert!(!group.payout_snapshot_captured);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_resolved_flat_close_returns_exact_capital() {
    let amount: u16 = kani::any();
    kani::assume(amount > 0);
    kani::assume(amount <= 1_000);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    group
        .deposit_not_atomic(&mut account, amount as u128)
        .unwrap();
    group.resolve_market_not_atomic(1).unwrap();

    let outcome = group.close_resolved_account_not_atomic(&mut account, 0);

    assert_eq!(
        outcome,
        Ok(ResolvedCloseOutcomeV13::Closed {
            payout: amount as u128
        })
    );
    assert_eq!(account.capital, 0);
    assert_eq!(account.pnl, 0);
    assert_eq!(group.c_tot, 0);
    assert_eq!(group.vault, 0);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(45)]
#[kani::solver(cadical)]
fn proof_v13_resolved_flat_close_syncs_fee_before_terminal_payout() {
    let fee_rate: u8 = kani::any();
    kani::assume(fee_rate > 0);
    kani::assume(fee_rate <= 5);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 100).unwrap();
    group.resolve_market_not_atomic(10).unwrap();

    let outcome = group
        .close_resolved_account_not_atomic(&mut account, fee_rate as u128)
        .unwrap();
    let expected_fee = fee_rate as u128 * 10;
    let expected_payout = 100 - expected_fee;

    kani::cover!(
        expected_fee > 0,
        "v13 resolved terminal close positive fee sync reachable"
    );
    assert_eq!(
        outcome,
        ResolvedCloseOutcomeV13::Closed {
            payout: expected_payout
        }
    );
    assert_eq!(account.last_fee_slot, group.resolved_slot);
    assert_eq!(account.capital, 0);
    assert_eq!(account.pnl, 0);
    assert_eq!(account.fee_credits, 0);
    assert_eq!(group.insurance, expected_fee);
    assert_eq!(group.vault, expected_fee);
    assert_eq!(group.c_tot, 0);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_resolved_profit_close_pays_snapshot_residual_and_clears_claim() {
    let profit: u8 = kani::any();
    kani::assume(profit > 0);
    kani::assume(profit <= 20);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 10).unwrap();
    account.pnl = profit as i128;
    group.pnl_pos_tot = profit as u128;
    group.vault = group.c_tot + profit as u128;
    group.resolve_market_not_atomic(1).unwrap();

    let outcome = group.close_resolved_account_not_atomic(&mut account, 0);

    kani::cover!(profit > 1, "v13 resolved profit payout branch reachable");
    assert_eq!(
        outcome,
        Ok(ResolvedCloseOutcomeV13::Closed {
            payout: 10 + profit as u128
        })
    );
    assert_eq!(account.capital, 0);
    assert_eq!(account.pnl, 0);
    assert_eq!(group.c_tot, 0);
    assert_eq!(group.pnl_pos_tot, 0);
    assert_eq!(group.vault, 0);
    assert_eq!(group.assert_public_invariants(), Ok(()));
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
#[kani::unwind(60)]
#[kani::solver(cadical)]
fn proof_v13_bankrupt_liquidation_cannot_free_exposure_before_residual_durable() {
    let larger_residual: bool = kani::any();
    let residual = if larger_residual { -3 } else { -2 };
    let (market, account_id, owner) = concrete_ids();
    let mut cfg = V13Config::public_user_fund(1, 0, 1);
    cfg.public_b_chunk_atoms = 1;
    let mut group = MarketGroupV13::new(market, cfg).unwrap();
    let mut bankrupt =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    let mut opposing = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, [4; 32], owner));

    group
        .attach_leg(&mut bankrupt, 0, SideV13::Long, 4)
        .unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV13::Short, -10)
        .unwrap();
    group.assets[0].b_short_num = u128::MAX;
    group.assets[0].social_loss_remainder_short_num = 10;
    bankrupt.pnl = residual;
    group.negative_pnl_account_count = 1;
    let before_b_short = group.assets[0].b_short_num;
    let before_bitmap = bankrupt.active_bitmap;
    let before_basis = bankrupt.legs[0].basis_pos_q;
    let before_pnl = bankrupt.pnl;

    let result = group.liquidate_account_not_atomic(
        &mut bankrupt,
        LiquidationRequestV13 {
            asset_index: 0,
            close_q: 4,
            fee_bps: 0,
        },
        &[1; V13_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(
        result == Err(V13Error::RecoveryRequired),
        "v13 partial residual recovery path reachable"
    );
    kani::cover!(
        !larger_residual,
        "v13 residual durability proof covers two atoms"
    );
    kani::cover!(
        larger_residual,
        "v13 residual durability proof covers three atoms"
    );
    assert_eq!(result, Err(V13Error::RecoveryRequired));
    assert_eq!(
        group.recovery_reason,
        Some(PermissionlessRecoveryReasonV13::ActiveBankruptCloseCannotProgress)
    );
    assert_eq!(bankrupt.active_bitmap, before_bitmap);
    assert_eq!(bankrupt.legs[0].basis_pos_q, before_basis);
    assert_eq!(bankrupt.pnl, before_pnl);
    assert_eq!(group.assets[0].b_short_num, before_b_short);
}

#[kani::proof]
#[kani::unwind(55)]
#[kani::solver(cadical)]
fn proof_v13_bankrupt_liquidation_excludes_fee_from_residual_and_spends_insurance_once() {
    let insurance_units: u8 = kani::any();
    kani::assume(insurance_units <= 2);
    let insurance = insurance_units as u128;
    let (market, account_id, owner) = concrete_ids();
    let mut cfg = V13Config::public_user_fund(1, 0, 1);
    cfg.liquidation_fee_bps = 10_000;
    cfg.liquidation_fee_cap = 10;
    cfg.min_liquidation_abs = 1;
    let mut group = MarketGroupV13::new(market, cfg).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));

    group.vault = insurance;
    group.insurance = insurance;
    account.pnl = -5;
    group.negative_pnl_account_count = 1;
    group.attach_leg(&mut account, 0, SideV13::Long, 1).unwrap();

    let out = group
        .liquidate_account_not_atomic(
            &mut account,
            LiquidationRequestV13 {
                asset_index: 0,
                close_q: 1,
                fee_bps: 10_000,
            },
            &[1; V13_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    kani::cover!(
        insurance == 0,
        "v13 bankrupt liquidation zero-insurance path reachable"
    );
    kani::cover!(
        insurance == 2,
        "v13 bankrupt liquidation partial-insurance path reachable"
    );
    assert_eq!(out.fee_charged, 0);
    assert_eq!(out.insurance_used, insurance);
    assert_eq!(group.insurance, 0);
    assert_eq!(out.residual_booked, 0);
    assert_eq!(out.explicit_loss, 5 - insurance);
    assert_eq!(account.pnl, 0);
    assert_eq!(account.active_bitmap, 0);
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v13_rebalance_reduce_position_preserves_senior_claims_and_reduces_risk() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    group
        .attach_leg(&mut account, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();
    let senior_before = group.c_tot + group.insurance;

    let out = group
        .rebalance_reduce_position_not_atomic(
            &mut account,
            RebalanceRequestV13 {
                asset_index: 0,
                reduce_q: POS_SCALE / 2,
            },
            &[1_000_000; V13_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    kani::cover!(out.reduced_q == POS_SCALE / 2);
    assert_eq!(out.reduced_q, POS_SCALE / 2);
    assert!(account.legs[0].active);
    assert_eq!(account.legs[0].side, SideV13::Long);
    assert_eq!(account.legs[0].basis_pos_q.unsigned_abs(), POS_SCALE / 2);
    assert_eq!(group.c_tot + group.insurance, senior_before);
    assert!(account.health_cert.valid);
    assert!(account.health_cert.certified_worst_case_loss <= 500_000);

    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    group
        .attach_leg(&mut account, 0, SideV13::Short, -(POS_SCALE as i128))
        .unwrap();
    let senior_before = group.c_tot + group.insurance;

    let out = group
        .rebalance_reduce_position_not_atomic(
            &mut account,
            RebalanceRequestV13 {
                asset_index: 0,
                reduce_q: POS_SCALE,
            },
            &[1_000_000; V13_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    kani::cover!(out.reduced_q == POS_SCALE);
    assert_eq!(out.reduced_q, POS_SCALE);
    assert_eq!(account.active_bitmap, 0);
    assert!(!account.legs[0].active);
    assert_eq!(group.c_tot + group.insurance, senior_before);
    assert!(account.health_cert.valid);
    assert_eq!(account.health_cert.certified_worst_case_loss, 0);
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

#[kani::proof]
#[kani::unwind(20)]
#[kani::solver(cadical)]
fn proof_v13_explicit_loss_audit_overflow_declares_recovery_without_mutation() {
    let bankrupt_long: bool = kani::any();
    let (market, _, _) = symbolic_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let bankrupt_side = if bankrupt_long {
        group.assets[0].explicit_unallocated_loss_short = u128::MAX;
        SideV13::Long
    } else {
        group.assets[0].explicit_unallocated_loss_long = u128::MAX;
        SideV13::Short
    };

    let before_long = group.assets[0].explicit_unallocated_loss_long;
    let before_short = group.assets[0].explicit_unallocated_loss_short;
    let result = group.book_bankruptcy_residual_chunk(0, bankrupt_side, 1);

    kani::cover!(
        bankrupt_long,
        "v13 explicit-loss short audit overflow reachable"
    );
    kani::cover!(
        !bankrupt_long,
        "v13 explicit-loss long audit overflow reachable"
    );
    assert_eq!(result, Err(V13Error::RecoveryRequired));
    assert_eq!(
        group.recovery_reason,
        Some(PermissionlessRecoveryReasonV13::ExplicitLossOrDustAuditOverflow)
    );
    assert_eq!(group.assets[0].explicit_unallocated_loss_long, before_long);
    assert_eq!(
        group.assets[0].explicit_unallocated_loss_short,
        before_short
    );
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v13_invalid_trade_request_rejects_before_any_mutation() {
    assert_invalid_trade_reverts(TradeRequestV13 {
        asset_index: 1,
        size_q: POS_SCALE,
        exec_price: 100,
        fee_bps: 0,
    });
    assert_invalid_trade_reverts(TradeRequestV13 {
        asset_index: 0,
        size_q: 0,
        exec_price: 100,
        fee_bps: 0,
    });
    assert_invalid_trade_reverts(TradeRequestV13 {
        asset_index: 0,
        size_q: POS_SCALE,
        exec_price: 0,
        fee_bps: 0,
    });
    assert_invalid_trade_reverts(TradeRequestV13 {
        asset_index: 0,
        size_q: POS_SCALE,
        exec_price: 100,
        fee_bps: 11,
    });
}

fn assert_invalid_trade_reverts(request: TradeRequestV13) {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    group.config.max_trading_fee_bps = 10;
    let mut long = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    let mut short = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut long, 1_000).unwrap();
    group.deposit_not_atomic(&mut short, 1_000).unwrap();
    let before_group = group;
    let before_long = long;
    let before_short = short;

    let result = group.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        request,
        &[100; V13_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(result, Err(V13Error::InvalidConfig));
    assert_eq!(group, before_group);
    assert_eq!(long, before_long);
    assert_eq!(short, before_short);
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v13_price_accrual_refresh_matches_eager_mark_pnl() {
    assert_price_accrual_refresh_matches_eager_mark_pnl(101, 1, -1);
    assert_price_accrual_refresh_matches_eager_mark_pnl(99, -1, 1);
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v13_same_epoch_full_refresh_is_idempotent_after_price_up_settlement() {
    assert_same_epoch_refresh_idempotent_after_kf_settlement(101, 1);
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v13_same_epoch_full_refresh_is_idempotent_after_price_down_settlement() {
    assert_same_epoch_refresh_idempotent_after_kf_settlement(99, -1);
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v13_sequential_kf_refresh_is_additive_not_compounding() {
    let (market, account_id, owner) = concrete_ids();
    let mut sequential = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    sequential.assets[0].effective_price = 100;
    sequential.assets[0].fund_px_last = 100;
    sequential.assets[0].raw_oracle_target_price = 100;
    let mut seq_account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    sequential
        .attach_leg(&mut seq_account, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();

    sequential
        .accrue_asset_to_not_atomic(0, 1, 101, 0, true)
        .unwrap();
    sequential
        .full_account_refresh(&mut seq_account, &[101; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    kani::cover!(
        seq_account.pnl == 1,
        "v13 first sequential K/F refresh settles nonzero pnl"
    );

    sequential
        .accrue_asset_to_not_atomic(0, 2, 102, 0, true)
        .unwrap();
    sequential
        .full_account_refresh(&mut seq_account, &[102; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    let mut direct = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    direct.assets[0].effective_price = 100;
    direct.assets[0].fund_px_last = 100;
    direct.assets[0].raw_oracle_target_price = 100;
    let mut direct_account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    direct
        .attach_leg(&mut direct_account, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();

    direct
        .accrue_asset_to_not_atomic(0, 1, 102, 0, true)
        .unwrap();
    direct
        .full_account_refresh(&mut direct_account, &[102; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(seq_account.pnl, 2);
    assert_eq!(direct_account.pnl, 2);
    assert_eq!(seq_account.pnl, direct_account.pnl);
    assert_eq!(sequential.pnl_pos_tot, direct.pnl_pos_tot);
}

fn assert_same_epoch_refresh_idempotent_after_kf_settlement(new_price: u64, expected_pnl: i128) {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    group.assets[0].effective_price = 100;
    group.assets[0].fund_px_last = 100;
    group.assets[0].raw_oracle_target_price = 100;
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    group
        .attach_leg(&mut account, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();

    group
        .accrue_asset_to_not_atomic(0, 1, new_price, 0, true)
        .unwrap();
    group
        .full_account_refresh(&mut account, &[new_price; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    let pnl_after_first = account.pnl;
    let leg_after_first = account.legs[0];
    let cert_equity_after_first = account.health_cert.certified_equity;
    let cert_initial_after_first = account.health_cert.certified_initial_req;
    let cert_maintenance_after_first = account.health_cert.certified_maintenance_req;
    let cert_deficit_after_first = account.health_cert.certified_liq_deficit;
    let pnl_pos_tot_after_first = group.pnl_pos_tot;
    let negative_count_after_first = group.negative_pnl_account_count;

    kani::cover!(
        pnl_after_first == expected_pnl,
        "v13 idempotent refresh exercises nonzero settled K/F pnl"
    );
    group
        .full_account_refresh(&mut account, &[new_price; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(account.pnl, pnl_after_first);
    assert_eq!(account.legs[0].active, leg_after_first.active);
    assert_eq!(account.legs[0].side, leg_after_first.side);
    assert_eq!(account.legs[0].basis_pos_q, leg_after_first.basis_pos_q);
    assert_eq!(account.legs[0].a_basis, leg_after_first.a_basis);
    assert_eq!(account.legs[0].k_snap, leg_after_first.k_snap);
    assert_eq!(account.legs[0].f_snap, leg_after_first.f_snap);
    assert_eq!(account.legs[0].epoch_snap, leg_after_first.epoch_snap);
    assert_eq!(
        account.health_cert.certified_equity,
        cert_equity_after_first
    );
    assert_eq!(
        account.health_cert.certified_initial_req,
        cert_initial_after_first
    );
    assert_eq!(
        account.health_cert.certified_maintenance_req,
        cert_maintenance_after_first
    );
    assert_eq!(
        account.health_cert.certified_liq_deficit,
        cert_deficit_after_first
    );
    assert_eq!(group.pnl_pos_tot, pnl_pos_tot_after_first);
    assert_eq!(group.negative_pnl_account_count, negative_count_after_first);
}

fn assert_price_accrual_refresh_matches_eager_mark_pnl(
    new_price: u64,
    expected_long_pnl: i128,
    expected_short_pnl: i128,
) {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    group.assets[0].effective_price = 100;
    group.assets[0].fund_px_last = 100;
    group.assets[0].raw_oracle_target_price = 100;
    let mut long = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    let mut short = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, [4; 32], owner));
    group
        .attach_leg(&mut long, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut short, 0, SideV13::Short, -(POS_SCALE as i128))
        .unwrap();
    let out = group
        .accrue_asset_to_not_atomic(0, 1, new_price, 0, true)
        .unwrap();
    group
        .full_account_refresh(&mut long, &[new_price; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    group
        .full_account_refresh(&mut short, &[new_price; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert!(out.price_move_active);
    assert_eq!(long.pnl, expected_long_pnl);
    assert_eq!(short.pnl, expected_short_pnl);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v13_funding_accrual_refresh_matches_sign_and_floor() {
    assert_funding_accrual_refresh_matches_sign_and_floor(10_000, -1, 1);
    assert_funding_accrual_refresh_matches_sign_and_floor(-10_000, 1, -1);
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v13_funding_accrual_requires_bilateral_exposure() {
    let (market, account_id, owner) = concrete_ids();
    let mut long_only = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    long_only.config.max_price_move_bps_per_slot = 9_999;
    long_only.config.max_abs_funding_e9_per_slot = 1;
    long_only.assets[0].effective_price = 1_000_000_000;
    long_only.assets[0].fund_px_last = 1_000_000_000;
    long_only.assets[0].raw_oracle_target_price = 1_000_000_000;
    let mut long = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    long_only
        .attach_leg(&mut long, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();
    let long_before = long_only.assets[0];

    let out = long_only
        .accrue_asset_to_not_atomic(0, 1, 1_000_000_000, 1, false)
        .unwrap();
    kani::cover!(
        long_only.assets[0].oi_eff_long_q != 0 && long_only.assets[0].oi_eff_short_q == 0,
        "v13 funding no-op covers long-only exposure"
    );

    assert!(!out.funding_active);
    assert_eq!(long_only.assets[0].f_long_num, long_before.f_long_num);
    assert_eq!(long_only.assets[0].f_short_num, long_before.f_short_num);
    assert_eq!(long_only.funding_epoch, 0);

    let mut short_only = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    short_only.config.max_price_move_bps_per_slot = 9_999;
    short_only.config.max_abs_funding_e9_per_slot = 1;
    short_only.assets[0].effective_price = 1_000_000_000;
    short_only.assets[0].fund_px_last = 1_000_000_000;
    short_only.assets[0].raw_oracle_target_price = 1_000_000_000;
    let mut short = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, [4; 32], owner));
    short_only
        .attach_leg(&mut short, 0, SideV13::Short, -(POS_SCALE as i128))
        .unwrap();
    let short_before = short_only.assets[0];

    let out = short_only
        .accrue_asset_to_not_atomic(0, 1, 1_000_000_000, 1, false)
        .unwrap();
    kani::cover!(
        short_only.assets[0].oi_eff_short_q != 0 && short_only.assets[0].oi_eff_long_q == 0,
        "v13 funding no-op covers short-only exposure"
    );

    assert!(!out.funding_active);
    assert_eq!(short_only.assets[0].f_long_num, short_before.f_long_num);
    assert_eq!(short_only.assets[0].f_short_num, short_before.f_short_num);
    assert_eq!(short_only.funding_epoch, 0);
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v13_no_oi_funding_rate_does_not_mutate_k_or_f() {
    let positive_rate: bool = kani::any();
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    group.config.max_price_move_bps_per_slot = 9_999;
    group.config.max_abs_funding_e9_per_slot = 1;
    group.assets[0].effective_price = 100;
    group.assets[0].fund_px_last = 100;
    group.assets[0].raw_oracle_target_price = 100;
    let before = group.assets[0];
    let rate = if positive_rate { 1 } else { -1 };

    let out = group
        .accrue_asset_to_not_atomic(0, 1, 100, rate, false)
        .unwrap();

    kani::cover!(
        positive_rate,
        "v13 no-OI funding proof covers positive rate"
    );
    kani::cover!(
        !positive_rate,
        "v13 no-OI funding proof covers negative rate"
    );
    assert!(!out.funding_active);
    assert!(!out.equity_active);
    assert_eq!(group.assets[0].k_long, before.k_long);
    assert_eq!(group.assets[0].k_short, before.k_short);
    assert_eq!(group.assets[0].f_long_num, before.f_long_num);
    assert_eq!(group.assets[0].f_short_num, before.f_short_num);
    assert_eq!(group.funding_epoch, 0);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v13_permissionless_crank_accepts_configured_funding_rate_boundaries() {
    let positive_rate: bool = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let mut cfg = V13Config::public_user_fund(1, 0, 1);
    cfg.max_price_move_bps_per_slot = 9_999;
    cfg.max_abs_funding_e9_per_slot = 1;
    let mut group = MarketGroupV13::new(market, cfg).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    let supplied_rate = if positive_rate { 1 } else { -1 };

    let out = group
        .permissionless_crank_not_atomic(
            &mut account,
            PermissionlessCrankRequestV13 {
                now_slot: 1,
                asset_index: 0,
                effective_price: 1,
                funding_rate_e9: supplied_rate,
                action: PermissionlessCrankActionV13::Refresh,
            },
            &[1; V13_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    kani::cover!(
        positive_rate && supplied_rate == group.config.max_abs_funding_e9_per_slot as i128,
        "v13 permissionless crank accepts positive funding boundary"
    );
    kani::cover!(
        !positive_rate && supplied_rate == -(group.config.max_abs_funding_e9_per_slot as i128),
        "v13 permissionless crank accepts negative funding boundary"
    );
    assert_eq!(out, PermissionlessProgressOutcomeV13::AccountCurrent);
    assert_eq!(group.current_slot, 1);
    assert_eq!(group.slot_last, 1);
    assert_eq!(group.funding_epoch, 0);
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v13_funding_accrual_uses_only_bounded_segment_dt() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    group.config.max_price_move_bps_per_slot = 4_999;
    group.config.max_abs_funding_e9_per_slot = 1;
    group.config.max_accrual_dt_slots = 2;
    group.config.min_funding_lifetime_slots = 2;
    group.assets[0].effective_price = 1_000_000_000;
    group.assets[0].fund_px_last = 1_000_000_000;
    group.assets[0].raw_oracle_target_price = 1_000_000_000;
    let mut long = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    let mut short = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, [4; 32], owner));
    group
        .attach_leg(&mut long, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut short, 0, SideV13::Short, -(POS_SCALE as i128))
        .unwrap();

    let out = group
        .accrue_asset_to_not_atomic(0, 10, 1_000_000_000, 1, true)
        .unwrap();
    kani::cover!(
        out.funding_active && out.dt == 2 && group.current_slot == 10,
        "v13 funding stale catchup covers bounded segment dt"
    );

    assert_eq!(out.dt, 2);
    assert!(out.loss_stale_after);
    assert_eq!(group.slot_last, 2);
    assert_eq!(group.current_slot, 10);
    assert_eq!(group.assets[0].f_long_num, -2 * ADL_ONE as i128);
    assert_eq!(group.assets[0].f_short_num, 2 * ADL_ONE as i128);
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v13_combined_price_and_funding_accrual_keeps_k_and_f_separate() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    group.config.max_price_move_bps_per_slot = 9_999;
    group.config.max_abs_funding_e9_per_slot = 1;
    group.assets[0].effective_price = 999_999_999;
    group.assets[0].fund_px_last = 999_999_999;
    group.assets[0].raw_oracle_target_price = 999_999_999;
    let mut long = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    let mut short = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, [4; 32], owner));
    group
        .attach_leg(&mut long, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut short, 0, SideV13::Short, -(POS_SCALE as i128))
        .unwrap();

    let out = group
        .accrue_asset_to_not_atomic(0, 1, 1_000_000_000, 1, true)
        .unwrap();
    kani::cover!(
        out.price_move_active && out.funding_active,
        "v13 combined mark and funding accrual reachable"
    );

    assert_eq!(group.assets[0].k_long, ADL_ONE as i128);
    assert_eq!(group.assets[0].k_short, -(ADL_ONE as i128));
    assert_eq!(group.assets[0].f_long_num, -(ADL_ONE as i128));
    assert_eq!(group.assets[0].f_short_num, ADL_ONE as i128);
    assert_eq!(group.assets[0].fund_px_last, 1_000_000_000);
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v13_zero_funding_rate_advances_time_without_f_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    group.assets[0].effective_price = 100;
    group.assets[0].fund_px_last = 100;
    group.assets[0].raw_oracle_target_price = 100;
    let mut long = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    let mut short = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, [4; 32], owner));
    group
        .attach_leg(&mut long, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut short, 0, SideV13::Short, -(POS_SCALE as i128))
        .unwrap();
    let before = group.assets[0];

    let out = group
        .accrue_asset_to_not_atomic(0, 1, 100, 0, true)
        .unwrap();
    kani::cover!(
        group.assets[0].oi_eff_long_q != 0 && group.assets[0].oi_eff_short_q != 0,
        "v13 zero-rate funding proof covers bilateral exposure"
    );

    assert!(!out.funding_active);
    assert_eq!(group.assets[0].f_long_num, before.f_long_num);
    assert_eq!(group.assets[0].f_short_num, before.f_short_num);
    assert_eq!(group.funding_epoch, 0);
    assert_eq!(group.slot_last, 1);
    assert_eq!(group.current_slot, 1);
}

fn assert_funding_accrual_refresh_matches_sign_and_floor(
    funding_rate_e9: i128,
    expected_long_pnl: i128,
    expected_short_pnl: i128,
) {
    let (market, account_id, owner) = concrete_ids();
    let mut cfg = V13Config::public_user_fund(1, 0, 1);
    cfg.max_abs_funding_e9_per_slot = 10_000;
    let mut group = MarketGroupV13::new(market, cfg).unwrap();
    group.assets[0].effective_price = 100_000;
    group.assets[0].fund_px_last = 100_000;
    group.assets[0].raw_oracle_target_price = 100_000;
    let mut long = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    let mut short = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, [4; 32], owner));
    group
        .attach_leg(&mut long, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut short, 0, SideV13::Short, -(POS_SCALE as i128))
        .unwrap();
    let out = group
        .accrue_asset_to_not_atomic(0, 1, 100_000, funding_rate_e9, true)
        .unwrap();
    group
        .full_account_refresh(&mut long, &[100_000; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    group
        .full_account_refresh(&mut short, &[100_000; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert!(out.funding_active);
    assert_eq!(long.pnl, expected_long_pnl);
    assert_eq!(short.pnl, expected_short_pnl);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_same_slot_exposed_price_move_rejects_before_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    group
        .attach_leg(&mut account, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();
    let before = group;

    let result = group.accrue_asset_to_not_atomic(0, 0, 2, 0, true);

    assert_eq!(result, Err(V13Error::NonProgress));
    assert_eq!(group, before);
}

#[kani::proof]
#[kani::unwind(60)]
#[kani::solver(cadical)]
fn proof_v13_partial_liquidation_can_reduce_risk_without_forcing_full_close() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 10).unwrap();
    group
        .attach_leg(&mut account, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();

    let out = group
        .liquidate_account_not_atomic(
            &mut account,
            LiquidationRequestV13 {
                asset_index: 0,
                close_q: POS_SCALE / 2,
                fee_bps: 0,
            },
            &[100; V13_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    kani::cover!(out.closed_q == POS_SCALE / 2);
    assert_eq!(out.closed_q, POS_SCALE / 2);
    assert_eq!(account.legs[0].basis_pos_q.unsigned_abs(), POS_SCALE / 2);
    assert_eq!(group.assets[0].oi_eff_long_q, POS_SCALE / 2);
    assert!(account.health_cert.certified_liq_deficit < 90);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(45)]
#[kani::solver(cadical)]
fn proof_v13_liquidation_rejects_zero_close_before_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    group
        .attach_leg(&mut account, 0, SideV13::Long, POS_SCALE as i128)
        .unwrap();
    let before_group = group;
    let before_account = account;

    let result = group.liquidate_account_not_atomic(
        &mut account,
        LiquidationRequestV13 {
            asset_index: 0,
            close_q: 0,
            fee_bps: 0,
        },
        &[100; V13_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(result, Err(V13Error::InvalidConfig));
    assert_eq!(group, before_group);
    assert_eq!(account, before_account);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_liquidation_fee_floor_shortfall_charges_available_capital_only() {
    let capital: u8 = kani::any();
    kani::assume(capital > 0);
    kani::assume(capital <= 20);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    group
        .deposit_not_atomic(&mut account, capital as u128)
        .unwrap();

    let charged = group
        .charge_account_fee_not_atomic(&mut account, 40)
        .unwrap();

    kani::cover!(
        charged < 40,
        "v13 liquidation-fee floor shortfall fee path reachable"
    );
    assert_eq!(charged, capital as u128);
    assert_eq!(account.capital, 0);
    assert_eq!(group.insurance, capital as u128);
    assert_eq!(group.c_tot, 0);
    assert_eq!(group.vault, capital as u128);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_resolved_active_position_close_returns_progress_without_payout() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 7).unwrap();
    group.attach_leg(&mut account, 0, SideV13::Long, 1).unwrap();
    group.resolve_market_not_atomic(1).unwrap();
    let before_vault = group.vault;
    let before_c_tot = group.c_tot;

    let outcome = group.close_resolved_account_not_atomic(&mut account, 0);

    assert_eq!(outcome, Ok(ResolvedCloseOutcomeV13::ProgressOnly));
    assert_ne!(account.active_bitmap, 0);
    assert_eq!(account.capital, 7);
    assert_eq!(group.vault, before_vault);
    assert_eq!(group.c_tot, before_c_tot);
}
