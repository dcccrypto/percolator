#![cfg(kani)]

use percolator::v14::{
    account_equity, risk_notional_ceil, CloseProgressLedgerV14, DeadLegForfeitOutcomeV14,
    HLockLaneV14, LiquidationRequestV14, MarketGroupV14, MarketGroupV14Account, MarketModeV14,
    PermissionlessCrankActionV14, PermissionlessCrankRequestV14, PermissionlessProgressOutcomeV14,
    PermissionlessRecoveryReasonV14, PortfolioAccountV14, PortfolioAccountV14Account,
    PortfolioLegV14, ProvenanceHeaderV14, RebalanceRequestV14, ResolvedCloseOutcomeV14,
    SideModeV14, SideV14, TradeRequestV14, V14Config, V14Error, V14PodI128, V14PodU32,
    V14_DOMAIN_COUNT, V14_MAX_PORTFOLIO_ASSETS_N,
};
use percolator::{
    ADL_ONE, MAX_OI_SIDE_Q, MAX_POSITION_ABS_Q, MAX_PROTOCOL_FEE_ABS, MAX_VAULT_TVL, POS_SCALE,
    SOCIAL_LOSS_DEN,
};

fn symbolic_ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    let market: [u8; 32] = kani::any();
    let account: [u8; 32] = kani::any();
    let owner: [u8; 32] = kani::any();
    (market, account, owner)
}

fn tight_envelope_config() -> V14Config {
    let mut cfg = V14Config::public_user_fund(1, 0, 1);
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
fn proof_v14_hlock_is_exactly_hmin_or_hmax() {
    let h_max: u8 = kani::any();
    kani::assume(h_max > 0);
    let (market, account_id, owner) = symbolic_ids();
    let mut group =
        MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, h_max as u64)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));

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
        "v14 h-min lane reachable"
    );
    kani::cover!(
        group.threshold_stress_active
            || group.bankruptcy_hlock_active
            || group.loss_stale_active
            || group.active_bankrupt_close_present
            || account.stale_state
            || account.b_stale_state
            || instruction_bankruptcy_candidate,
        "v14 h-max lane reachable"
    );

    let selected = group
        .select_h_lock(Some(&account), instruction_bankruptcy_candidate)
        .unwrap();
    assert!(selected == 0 || selected == h_max as u64);

    let lane = group
        .h_lock_lane(Some(&account), instruction_bankruptcy_candidate)
        .unwrap();
    if lane == HLockLaneV14::HMax {
        assert_eq!(selected, h_max as u64);
    } else {
        assert_eq!(selected, 0);
    }
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_hmin_zero_remains_available_when_no_lock_state_exists() {
    let h_max: u8 = kani::any();
    kani::assume(h_max > 0);
    let (market, account_id, owner) = symbolic_ids();
    let group =
        MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, h_max as u64)).unwrap();
    let account = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));

    assert_eq!(
        group.h_lock_lane(Some(&account), false),
        Ok(HLockLaneV14::HMin)
    );
    assert_eq!(group.select_h_lock(Some(&account), false), Ok(0));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_stale_counter_transitions_are_idempotent() {
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));

    group.mark_account_stale(&mut account).unwrap();
    group.mark_account_stale(&mut account).unwrap();
    kani::cover!(account.stale_state, "v14 stale state reachable");
    assert_eq!(group.stale_certificate_count, 1);

    group.clear_account_stale(&mut account).unwrap();
    group.clear_account_stale(&mut account).unwrap();
    kani::cover!(!account.stale_state, "v14 stale clear reachable");
    assert_eq!(group.stale_certificate_count, 0);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_b_stale_counter_transitions_are_idempotent_and_leg_gated() {
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));

    group.mark_account_b_stale(&mut account).unwrap();
    group.mark_account_b_stale(&mut account).unwrap();
    kani::cover!(account.b_stale_state, "v14 b-stale state reachable");
    assert_eq!(group.b_stale_account_count, 1);

    group.clear_account_b_stale(&mut account).unwrap();
    group.clear_account_b_stale(&mut account).unwrap();
    kani::cover!(!account.b_stale_state, "v14 b-stale clear reachable");
    assert_eq!(group.b_stale_account_count, 0);

    group
        .attach_leg(&mut account, 0, SideV14::Long, POS_SCALE as i128)
        .unwrap();
    group.mark_leg_b_stale(&mut account, 0).unwrap();
    group.mark_leg_b_stale(&mut account, 0).unwrap();
    kani::cover!(
        account.b_stale_state && account.legs[0].b_stale,
        "v14 active b-stale leg reachable"
    );
    assert_eq!(group.b_stale_account_count, 1);

    assert_eq!(
        group.clear_account_b_stale(&mut account),
        Err(V14Error::BStale)
    );
    assert!(account.b_stale_state);
    assert!(account.legs[0].b_stale);
    assert_eq!(group.b_stale_account_count, 1);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_account_equity_rejects_i128_min_persistent_pnl() {
    let (market, account_id, owner) = symbolic_ids();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    account.pnl = i128::MIN;
    assert_eq!(account_equity(&account), Err(V14Error::ArithmeticOverflow));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_account_equity_rejects_malformed_fee_credits() {
    let malformed_positive: bool = kani::any();
    let (market, account_id, owner) = symbolic_ids();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    account.capital = 100;
    account.fee_credits = if malformed_positive { 1 } else { i128::MIN };

    kani::cover!(
        malformed_positive,
        "v14 positive fee credit corruption reachable"
    );
    kani::cover!(
        !malformed_positive,
        "v14 i128 min fee credit corruption reachable"
    );
    assert!(account_equity(&account).is_err());
}

#[kani::proof]
#[kani::unwind(10)]
#[kani::solver(cadical)]
fn proof_v14_account_equity_rejects_capital_above_i128_max() {
    let (market, account_id, owner) = symbolic_ids();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    account.capital = i128::MAX as u128 + 1;

    kani::cover!(
        account.capital > i128::MAX as u128,
        "v14 capital overflow equity path reachable"
    );
    assert_eq!(account_equity(&account), Err(V14Error::ArithmeticOverflow));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_account_shape_rejects_malformed_persistent_economic_state() {
    let dirty_case: u8 = kani::any();
    kani::assume(dirty_case < 4);
    let (market, account_id, owner) = symbolic_ids();
    let group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));

    let expected = match dirty_case {
        0 => {
            account.pnl = i128::MIN;
            V14Error::ArithmeticOverflow
        }
        1 => {
            account.fee_credits = 1;
            V14Error::InvalidLeg
        }
        2 => {
            account.fee_credits = i128::MIN;
            V14Error::ArithmeticOverflow
        }
        _ => {
            account.pnl = 1;
            account.reserved_pnl = 2;
            V14Error::InvalidLeg
        }
    };

    kani::cover!(dirty_case == 0, "v14 shape rejects i128 min pnl");
    kani::cover!(dirty_case == 1, "v14 shape rejects positive fee credit");
    kani::cover!(dirty_case == 2, "v14 shape rejects i128 min fee credit");
    kani::cover!(dirty_case == 3, "v14 shape rejects over-reserved pnl");
    assert_eq!(group.validate_account_shape(&account), Err(expected));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_persisted_wire_rejects_noncanonical_bool_enum_and_option() {
    let bad_bool: u8 = kani::any();
    let bad_market_mode: u8 = kani::any();
    let bad_side_mode: u8 = kani::any();
    let bad_option_present: u8 = kani::any();
    kani::assume(bad_bool > 1);
    kani::assume(bad_market_mode > 2);
    kani::assume(bad_side_mode > 2);
    kani::assume(bad_option_present > 1);

    let (market, account_id, owner) = symbolic_ids();
    let group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let account = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));

    let mut account_wire = PortfolioAccountV14Account::from_runtime(&account);
    account_wire.stale_state = bad_bool;
    kani::cover!(bad_bool == 2, "v14 persisted invalid bool branch reachable");
    assert_eq!(account_wire.try_to_runtime(), Err(V14Error::InvalidConfig));

    let mut market_mode_wire = MarketGroupV14Account::from_runtime(&group);
    market_mode_wire.mode = bad_market_mode;
    kani::cover!(
        bad_market_mode == 3,
        "v14 persisted invalid market mode branch reachable"
    );
    assert_eq!(
        market_mode_wire.try_to_runtime(),
        Err(V14Error::InvalidConfig)
    );

    let mut side_mode_wire = MarketGroupV14Account::from_runtime(&group);
    side_mode_wire.assets[0].mode_long = bad_side_mode;
    kani::cover!(
        bad_side_mode == 3,
        "v14 persisted invalid side mode branch reachable"
    );
    assert_eq!(
        side_mode_wire.try_to_runtime(),
        Err(V14Error::InvalidConfig)
    );

    let mut option_wire = MarketGroupV14Account::from_runtime(&group);
    option_wire.recovery_reason.present = bad_option_present;
    kani::cover!(
        bad_option_present == 2,
        "v14 persisted invalid option-present branch reachable"
    );
    assert_eq!(option_wire.try_to_runtime(), Err(V14Error::InvalidConfig));
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v14_market_wire_roundtrip_preserves_valid_runtime_state() {
    let vault_units: u8 = kani::any();
    let c_units: u8 = kani::any();
    let i_units: u8 = kani::any();
    let pnl_pos_units: u8 = kani::any();
    let pnl_matured_units: u8 = kani::any();
    let price_raw: u16 = kani::any();
    let oi_long_units: u8 = kani::any();
    let oi_short_units: u8 = kani::any();
    let k_raw: i16 = kani::any();
    let f_raw: i16 = kani::any();
    let side_mode_case: u8 = kani::any();
    let market_mode_case: u8 = kani::any();
    let recovery_case: u8 = kani::any();
    let recovery_present: bool = kani::any();

    kani::assume((c_units as u16) + (i_units as u16) <= vault_units as u16);
    kani::assume(pnl_matured_units <= pnl_pos_units);
    kani::assume(price_raw > 0);
    kani::assume(price_raw <= 1_000);
    kani::assume(side_mode_case < 3);
    kani::assume(market_mode_case < 3);
    kani::assume(recovery_case < 8);

    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    group.vault = vault_units as u128;
    group.c_tot = c_units as u128;
    group.insurance = i_units as u128;
    group.pnl_pos_tot = pnl_pos_units as u128;
    group.pnl_pos_bound_tot = pnl_pos_units as u128;
    group.pnl_matured_pos_tot = pnl_matured_units as u128;
    group.bankruptcy_hlock_active = kani::any();
    group.threshold_stress_active = kani::any();
    group.active_bankrupt_close_present = kani::any();
    group.loss_stale_active = kani::any();
    group.payout_snapshot_captured = kani::any();
    group.mode = match market_mode_case {
        0 => MarketModeV14::Live,
        1 => MarketModeV14::Recovery,
        _ => MarketModeV14::Resolved,
    };
    group.recovery_reason = if recovery_present {
        Some(match recovery_case {
            0 => PermissionlessRecoveryReasonV14::BelowProgressFloor,
            1 => PermissionlessRecoveryReasonV14::BlockedSegmentHeadroomOrRepresentability,
            2 => PermissionlessRecoveryReasonV14::AccountBSettlementCannotProgress,
            3 => PermissionlessRecoveryReasonV14::BIndexHeadroomExhausted,
            4 => PermissionlessRecoveryReasonV14::ActiveBankruptCloseCannotProgress,
            5 => PermissionlessRecoveryReasonV14::ExplicitLossOrDustAuditOverflow,
            6 => PermissionlessRecoveryReasonV14::OracleOrTargetUnavailableByAuthenticatedPolicy,
            _ => PermissionlessRecoveryReasonV14::CounterOrEpochOverflowDeclaredRecovery,
        })
    } else {
        None
    };

    let side_mode = match side_mode_case {
        0 => SideModeV14::Normal,
        1 => SideModeV14::ResetPending,
        _ => SideModeV14::DrainOnly,
    };
    group.assets[0].raw_oracle_target_price = price_raw as u64;
    group.assets[0].effective_price = price_raw as u64;
    group.assets[0].fund_px_last = price_raw as u64;
    group.assets[0].k_long = k_raw as i128;
    group.assets[0].k_short = -(k_raw as i128);
    group.assets[0].f_long_num = f_raw as i128;
    group.assets[0].f_short_num = -(f_raw as i128);
    group.assets[0].k_epoch_start_long = k_raw as i128;
    group.assets[0].k_epoch_start_short = -(k_raw as i128);
    group.assets[0].f_epoch_start_long_num = f_raw as i128;
    group.assets[0].f_epoch_start_short_num = -(f_raw as i128);
    group.assets[0].oi_eff_long_q = oi_long_units as u128;
    group.assets[0].oi_eff_short_q = oi_short_units as u128;
    group.assets[0].mode_long = side_mode;
    group.assets[0].mode_short = side_mode;

    let wire = MarketGroupV14Account::from_runtime(&group);
    let decoded = wire.try_to_runtime().unwrap();

    kani::cover!(
        recovery_present,
        "v14 market wire roundtrip with recovery reason"
    );
    kani::cover!(
        !recovery_present,
        "v14 market wire roundtrip without recovery reason"
    );
    kani::cover!(
        side_mode_case == 1,
        "v14 market wire roundtrip reset-pending side mode"
    );
    assert_eq!(decoded, group);
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v14_portfolio_wire_roundtrip_preserves_valid_runtime_state() {
    let active: bool = kani::any();
    let short_side: bool = kani::any();
    let basis_units: u8 = kani::any();
    let capital_units: u8 = kani::any();
    let pnl_units: u8 = kani::any();
    let reserved_units: u8 = kani::any();
    let fee_debt_units: u8 = kani::any();
    let last_fee_slot: u8 = kani::any();

    kani::assume(basis_units > 0);
    kani::assume(basis_units <= 10);
    kani::assume(reserved_units <= pnl_units);

    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    if active {
        let signed_basis = if short_side {
            -(basis_units as i128)
        } else {
            basis_units as i128
        };
        let side = if short_side {
            SideV14::Short
        } else {
            SideV14::Long
        };
        group
            .attach_leg(&mut account, 0, side, signed_basis)
            .unwrap();
    }
    account.capital = capital_units as u128;
    account.pnl = pnl_units as i128;
    account.reserved_pnl = reserved_units as u128;
    account.fee_credits = -(fee_debt_units as i128);
    account.last_fee_slot = last_fee_slot as u64;
    account.stale_state = kani::any();
    account.b_stale_state = kani::any();
    account.rebalance_lock = kani::any();
    account.liquidation_lock = kani::any();
    account.health_cert.valid = kani::any();
    account.health_cert.certified_equity = account_equity(&account).unwrap();
    account.health_cert.active_bitmap_at_cert = account.active_bitmap;

    let wire = PortfolioAccountV14Account::from_runtime(&account);
    let decoded = wire.try_to_runtime().unwrap();
    let checked = wire.validate_with_market(&group).unwrap();

    kani::cover!(
        active && !short_side,
        "v14 portfolio wire roundtrip active long"
    );
    kani::cover!(
        active && short_side,
        "v14 portfolio wire roundtrip active short"
    );
    kani::cover!(!active, "v14 portfolio wire roundtrip inactive account");
    assert_eq!(decoded, account);
    assert_eq!(checked, account);
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v14_persisted_wire_rejects_i128_min_economic_fields() {
    let dirty_case: u8 = kani::any();
    kani::assume(dirty_case < 6);

    let (market, account_id, owner) = concrete_ids();
    let group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut active_group = group;
    active_group
        .attach_leg(&mut account, 0, SideV14::Long, 1)
        .unwrap();

    match dirty_case {
        0 => {
            let mut wire = MarketGroupV14Account::from_runtime(&group);
            wire.assets[0].k_long = V14PodI128::new(i128::MIN);
            assert_eq!(wire.try_to_runtime(), Err(V14Error::ArithmeticOverflow));
        }
        1 => {
            let mut wire = MarketGroupV14Account::from_runtime(&group);
            wire.assets[0].f_short_num = V14PodI128::new(i128::MIN);
            assert_eq!(wire.try_to_runtime(), Err(V14Error::ArithmeticOverflow));
        }
        2 => {
            let mut wire = PortfolioAccountV14Account::from_runtime(&account);
            wire.pnl = V14PodI128::new(i128::MIN);
            assert_eq!(wire.try_to_runtime(), Err(V14Error::ArithmeticOverflow));
        }
        3 => {
            let mut wire = PortfolioAccountV14Account::from_runtime(&account);
            wire.fee_credits = V14PodI128::new(i128::MIN);
            assert_eq!(wire.try_to_runtime(), Err(V14Error::ArithmeticOverflow));
        }
        4 => {
            let mut wire = PortfolioAccountV14Account::from_runtime(&account);
            wire.legs[0].k_snap = V14PodI128::new(i128::MIN);
            assert_eq!(wire.try_to_runtime(), Err(V14Error::ArithmeticOverflow));
        }
        _ => {
            let mut wire = PortfolioAccountV14Account::from_runtime(&account);
            wire.health_cert.certified_equity = V14PodI128::new(i128::MIN);
            assert_eq!(wire.try_to_runtime(), Err(V14Error::ArithmeticOverflow));
        }
    }

    kani::cover!(dirty_case == 0, "v14 wire rejects i128 min market K");
    kani::cover!(dirty_case == 1, "v14 wire rejects i128 min market F");
    kani::cover!(dirty_case == 2, "v14 wire rejects i128 min account PnL");
    kani::cover!(dirty_case == 3, "v14 wire rejects i128 min fee credits");
    kani::cover!(dirty_case == 4, "v14 wire rejects i128 min leg K snapshot");
    kani::cover!(
        dirty_case == 5,
        "v14 wire rejects i128 min health certificate"
    );
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v14_persisted_wire_rejects_provenance_and_hidden_leg_smuggling() {
    let case: u8 = kani::any();
    kani::assume(case < 5);
    let (market, account_id, owner) = concrete_ids();
    let group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let empty = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut active = empty;
    let mut builder_group = group;
    builder_group
        .attach_leg(&mut active, 0, SideV14::Long, 1)
        .unwrap();
    let active_wire = PortfolioAccountV14Account::from_runtime(&active);
    let mut wire = PortfolioAccountV14Account::from_runtime(&empty);

    let expected = match case {
        0 => {
            wire.provenance_header.market_group_id = [9; 32];
            V14Error::ProvenanceMismatch
        }
        1 => {
            wire.owner = [9; 32];
            V14Error::ProvenanceMismatch
        }
        2 => {
            wire.active_bitmap = V14PodU32::new(1);
            V14Error::HiddenLeg
        }
        3 => {
            wire.legs[0] = active_wire.legs[0];
            wire.active_bitmap = V14PodU32::new(0);
            V14Error::HiddenLeg
        }
        _ => {
            wire.legs[1] = active_wire.legs[0];
            wire.active_bitmap = V14PodU32::new(1 << 1);
            V14Error::HiddenLeg
        }
    };

    kani::cover!(case == 0, "v14 persisted wrong-market account rejected");
    kani::cover!(case == 1, "v14 persisted wrong-owner account rejected");
    kani::cover!(case == 2, "v14 persisted bitmap-only leg rejected");
    kani::cover!(case == 3, "v14 persisted hidden active leg rejected");
    kani::cover!(case == 4, "v14 persisted out-of-config leg rejected");
    assert_eq!(wire.validate_with_market(&group), Err(expected));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_flat_account_equity_is_exact_capital_plus_pnl_minus_fee_debt() {
    let capital: u16 = kani::any();
    let pnl: i16 = kani::any();
    let debt: u16 = kani::any();
    kani::assume(capital <= 10_000);
    kani::assume(debt <= 10_000);
    let (market, account_id, owner) = symbolic_ids();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    account.capital = capital as u128;
    account.pnl = pnl as i128;
    account.fee_credits = -(debt as i128);

    let expected = (capital as i128) + (pnl as i128) - (debt as i128);
    let actual = account_equity(&account).unwrap();

    kani::cover!(pnl < 0, "v14 flat negative pnl equity branch reachable");
    kani::cover!(pnl >= 0, "v14 flat nonnegative pnl equity branch reachable");
    kani::cover!(debt > 0, "v14 flat account fee debt branch reachable");
    assert_eq!(actual, expected);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_authoritatively_flat_account_never_receives_b_loss() {
    let b_long: u8 = kani::any();
    let b_short: u8 = kani::any();
    let budget: u8 = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
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
        "v14 flat account with nonzero side B accumulator reachable"
    );
    assert_eq!(outcome, PermissionlessProgressOutcomeV14::AccountCurrent);
    assert_eq!(account.active_bitmap, 0);
    assert_eq!(account.pnl, before_account.pnl);
    assert_eq!(account.capital, before_account.capital);
    assert_eq!(account.b_stale_state, before_account.b_stale_state);
    assert_eq!(group.b_stale_account_count, before_count);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_public_config_rejects_invalid_user_fund_shapes() {
    let case: u8 = kani::any();
    kani::assume(case < 11);
    let (market, _, _) = symbolic_ids();
    let mut cfg = V14Config::public_user_fund(1, 0, 1);
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

    kani::cover!(case == 0, "v14 zero portfolio width rejected");
    kani::cover!(case == 1, "v14 zero hmax rejected");
    kani::cover!(case == 2, "v14 hmin above hmax rejected");
    kani::cover!(case == 3, "v14 invalid margin floor ordering rejected");
    kani::cover!(case == 4, "v14 disabled recovery rejected");
    kani::cover!(case == 5, "v14 zero B chunk budget rejected");
    kani::cover!(case == 6, "v14 disabled stale certificate penalty rejected");
    kani::cover!(case == 7, "v14 disabled required full refresh rejected");
    kani::cover!(case == 8, "v14 disabled crank-forward profile rejected");
    kani::cover!(case == 9, "v14 zero account B chunk cap rejected");
    kani::cover!(case == 10, "v14 zero bankrupt close chunk cap rejected");
    assert_eq!(
        MarketGroupV14::new(market, cfg),
        Err(V14Error::InvalidConfig)
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_permissionless_recovery_declares_reason_or_fails_closed() {
    let reason_case: u8 = kani::any();
    kani::assume(reason_case < 8);
    let enabled: bool = kani::any();
    let start_resolved: bool = kani::any();
    let reason = match reason_case {
        0 => PermissionlessRecoveryReasonV14::BelowProgressFloor,
        1 => PermissionlessRecoveryReasonV14::BlockedSegmentHeadroomOrRepresentability,
        2 => PermissionlessRecoveryReasonV14::AccountBSettlementCannotProgress,
        3 => PermissionlessRecoveryReasonV14::BIndexHeadroomExhausted,
        4 => PermissionlessRecoveryReasonV14::ActiveBankruptCloseCannotProgress,
        5 => PermissionlessRecoveryReasonV14::ExplicitLossOrDustAuditOverflow,
        6 => PermissionlessRecoveryReasonV14::OracleOrTargetUnavailableByAuthenticatedPolicy,
        _ => PermissionlessRecoveryReasonV14::CounterOrEpochOverflowDeclaredRecovery,
    };
    let (market, _, _) = symbolic_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    group.config.permissionless_recovery_enabled = enabled;
    if start_resolved {
        group.resolve_market_not_atomic(0).unwrap();
    }

    let before_mode = group.mode;
    let before_vault = group.vault;
    let before_c_tot = group.c_tot;
    let before_insurance = group.insurance;
    let result = group.declare_permissionless_recovery(reason);

    kani::cover!(
        enabled,
        "v14 permissionless recovery enabled path reachable"
    );
    kani::cover!(
        !enabled,
        "v14 permissionless recovery disabled path reachable"
    );
    kani::cover!(
        enabled && start_resolved,
        "v14 permissionless recovery resolved-mode rejection reachable"
    );
    kani::cover!(
        reason_case == 0,
        "v14 permissionless recovery first reason reachable"
    );
    kani::cover!(
        reason_case == 7,
        "v14 permissionless recovery last reason reachable"
    );

    if enabled && !start_resolved {
        assert_eq!(
            result,
            Ok(PermissionlessProgressOutcomeV14::RecoveryDeclared(reason))
        );
        assert_eq!(group.recovery_reason, Some(reason));
        assert_eq!(group.mode, MarketModeV14::Recovery);
    } else {
        if enabled {
            assert_eq!(result, Err(V14Error::LockActive));
        } else {
            assert_eq!(result, Err(V14Error::InvalidConfig));
        }
        assert_eq!(group.recovery_reason, None);
        assert_eq!(group.mode, before_mode);
    }
    assert_eq!(group.vault, before_vault);
    assert_eq!(group.c_tot, before_c_tot);
    assert_eq!(group.insurance, before_insurance);
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v14_permissionless_crank_recovery_declaration_is_accounting_neutral() {
    let reason_case: u8 = kani::any();
    kani::assume(reason_case < 8);
    let reason = match reason_case {
        0 => PermissionlessRecoveryReasonV14::BelowProgressFloor,
        1 => PermissionlessRecoveryReasonV14::BlockedSegmentHeadroomOrRepresentability,
        2 => PermissionlessRecoveryReasonV14::AccountBSettlementCannotProgress,
        3 => PermissionlessRecoveryReasonV14::BIndexHeadroomExhausted,
        4 => PermissionlessRecoveryReasonV14::ActiveBankruptCloseCannotProgress,
        5 => PermissionlessRecoveryReasonV14::ExplicitLossOrDustAuditOverflow,
        6 => PermissionlessRecoveryReasonV14::OracleOrTargetUnavailableByAuthenticatedPolicy,
        _ => PermissionlessRecoveryReasonV14::CounterOrEpochOverflowDeclaredRecovery,
    };
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 100).unwrap();
    group.attach_leg(&mut account, 0, SideV14::Long, 1).unwrap();

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
        PermissionlessCrankRequestV14 {
            now_slot: current_slot_before + 1,
            asset_index: 0,
            effective_price: 2,
            funding_rate_e9: 0,
            action: PermissionlessCrankActionV14::Recover(reason),
        },
        &[1; V14_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(
        reason_case == 0,
        "v14 recovery-crank first reason reachable"
    );
    kani::cover!(reason_case == 7, "v14 recovery-crank last reason reachable");
    assert_eq!(
        outcome,
        Ok(PermissionlessProgressOutcomeV14::RecoveryDeclared(reason))
    );
    assert_eq!(group.recovery_reason, Some(reason));
    assert_eq!(group.mode, MarketModeV14::Recovery);
    assert_eq!(account, account_before);
    assert_eq!(group.vault, vault_before);
    assert_eq!(group.c_tot, c_tot_before);
    assert_eq!(group.insurance, insurance_before);
    assert_eq!(group.pnl_pos_tot, pnl_pos_before);
    assert_eq!(group.assets[0], asset_before);
    assert_eq!(group.slot_last, slot_last_before);
    assert_eq!(group.current_slot, current_slot_before);
    assert_eq!(group.mode, MarketModeV14::Recovery);
}

#[kani::proof]
#[kani::unwind(60)]
#[kani::solver(cadical)]
fn proof_v14_permissionless_recovery_enables_dead_leg_forfeit_without_value_escape() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group.attach_leg(&mut account, 0, SideV14::Long, 1).unwrap();
    let before_vault = group.vault;
    let before_c_tot = group.c_tot;
    let before_insurance = group.insurance;
    let before_pnl_pos = group.pnl_pos_tot;

    let reason = PermissionlessRecoveryReasonV14::OracleOrTargetUnavailableByAuthenticatedPolicy;
    let declared = group.declare_permissionless_recovery(reason);
    let outcome = group.forfeit_recovery_leg_not_atomic(&mut account, 0, 1);

    kani::cover!(
        declared == Ok(PermissionlessProgressOutcomeV14::RecoveryDeclared(reason))
            && matches!(outcome, Ok(DeadLegForfeitOutcomeV14 { detached: true, .. })),
        "v14 declared recovery enables bounded dead-leg forfeit"
    );
    assert_eq!(
        declared,
        Ok(PermissionlessProgressOutcomeV14::RecoveryDeclared(reason))
    );
    match outcome {
        Ok(out) => {
            assert!(out.detached);
            assert_eq!(out.positive_pnl_forfeited, 0);
            assert_eq!(out.loss_settled, 0);
            assert_eq!(out.insurance_used, 0);
            assert_eq!(out.residual_booked, 0);
            assert_eq!(out.explicit_loss, 0);
        }
        Err(_) => assert!(false),
    }
    assert_eq!(group.mode, MarketModeV14::Recovery);
    assert_eq!(group.recovery_reason, Some(reason));
    assert_eq!(account.active_bitmap, 0);
    assert_eq!(group.assets[0].oi_eff_long_q, 0);
    assert_eq!(group.vault, before_vault);
    assert_eq!(group.c_tot, before_c_tot);
    assert_eq!(group.insurance, before_insurance);
    assert_eq!(group.pnl_pos_tot, before_pnl_pos);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(48)]
#[kani::solver(cadical)]
fn proof_v14_recovery_mode_blocks_value_escape_paths_before_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 100).unwrap();
    account.pnl = 10;
    group.pnl_pos_tot = 10;
    group.vault = group.vault.checked_add(10).unwrap();
    group
        .full_account_refresh(&mut account, &[1; V14_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    group
        .declare_permissionless_recovery(PermissionlessRecoveryReasonV14::BelowProgressFloor)
        .unwrap();
    let account_before = account;
    let vault_before = group.vault;
    let c_tot_before = group.c_tot;
    let insurance_before = group.insurance;

    let convert = group.convert_released_pnl_to_capital_not_atomic(&mut account);
    let withdraw = group.withdraw_not_atomic(&mut account, 1, &[1; V14_MAX_PORTFOLIO_ASSETS_N]);
    let fee_sync = group.sync_account_fee_to_slot_not_atomic(&mut account, 1, 1);

    kani::cover!(
        convert == Err(V14Error::LockActive)
            && withdraw == Err(V14Error::LockActive)
            && fee_sync == Err(V14Error::LockActive),
        "v14 terminal recovery blocks value escape paths"
    );
    assert_eq!(convert, Err(V14Error::LockActive));
    assert_eq!(withdraw, Err(V14Error::LockActive));
    assert_eq!(fee_sync, Err(V14Error::LockActive));
    assert_eq!(account, account_before);
    assert_eq!(group.vault, vault_before);
    assert_eq!(group.c_tot, c_tot_before);
    assert_eq!(group.insurance, insurance_before);
    assert_eq!(group.mode, MarketModeV14::Recovery);
    assert_eq!(
        group.recovery_reason,
        Some(PermissionlessRecoveryReasonV14::BelowProgressFloor)
    );
}

#[kani::proof]
#[kani::unwind(48)]
#[kani::solver(cadical)]
fn proof_v14_recovery_mode_rejects_non_recovery_crank_before_account_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group.attach_leg(&mut account, 0, SideV14::Long, 1).unwrap();
    let asset_before = group.assets[0];
    let reason = PermissionlessRecoveryReasonV14::BlockedSegmentHeadroomOrRepresentability;
    group.declare_permissionless_recovery(reason).unwrap();
    let account_before = account;
    let result = group.permissionless_crank_not_atomic(
        &mut account,
        PermissionlessCrankRequestV14 {
            now_slot: 1,
            asset_index: 0,
            effective_price: 1,
            funding_rate_e9: 0,
            action: PermissionlessCrankActionV14::Refresh,
        },
        &[1; V14_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(
        result == Err(V14Error::LockActive),
        "v14 terminal recovery rejects non-recovery crank before mutation"
    );
    assert_eq!(result, Err(V14Error::LockActive));
    assert_eq!(account, account_before);
    assert_eq!(group.assets[0], asset_before);
    assert_eq!(group.mode, MarketModeV14::Recovery);
    assert_eq!(group.recovery_reason, Some(reason));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_public_config_accepts_full_margin_loss_only_envelope() {
    let (market, _, _) = symbolic_ids();
    let cfg = V14Config::public_user_fund(1, 0, 1);

    kani::cover!(
        cfg.maintenance_margin_bps == 10_000 && cfg.max_price_move_bps_per_slot == 10_000,
        "v14 full-margin one-segment loss envelope reachable"
    );
    assert!(MarketGroupV14::new(market, cfg).is_ok());
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_public_config_rejects_price_funding_envelope_breach() {
    let (market, _, _) = symbolic_ids();
    let mut cfg = tight_envelope_config();
    cfg.max_price_move_bps_per_slot = 10;

    kani::cover!(
        cfg.max_price_move_bps_per_slot == 10,
        "v14 price/funding envelope breach rejected"
    );
    assert_eq!(
        MarketGroupV14::new(market, cfg),
        Err(V14Error::InvalidConfig)
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_public_config_rejects_liquidation_fee_envelope_breach() {
    let (market, _, _) = symbolic_ids();
    let mut cfg = tight_envelope_config();
    cfg.liquidation_fee_bps = 400;

    kani::cover!(
        cfg.liquidation_fee_bps == 400,
        "v14 liquidation-fee envelope breach rejected"
    );
    assert_eq!(
        MarketGroupV14::new(market, cfg),
        Err(V14Error::InvalidConfig)
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_public_config_rejects_funding_headroom_breach() {
    let (market, _, _) = symbolic_ids();
    let mut cfg = tight_envelope_config();
    cfg.max_accrual_dt_slots = 1_000_000_000;
    cfg.min_funding_lifetime_slots = 1_000_000_000;

    kani::cover!(
        cfg.max_accrual_dt_slots == 1_000_000_000,
        "v14 funding K/F headroom breach rejected"
    );
    assert_eq!(
        MarketGroupV14::new(market, cfg),
        Err(V14Error::InvalidConfig)
    );
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v14_public_config_accepts_capped_liquidation_fee_envelope() {
    let (market, _, _) = symbolic_ids();
    let mut cfg = tight_envelope_config();
    cfg.liquidation_fee_bps = 10_000;
    cfg.liquidation_fee_cap = 1;

    kani::cover!(
        cfg.liquidation_fee_bps == 10_000 && cfg.liquidation_fee_cap == 1,
        "v14 capped liquidation fee envelope reachable"
    );
    assert!(MarketGroupV14::new(market, cfg).is_ok());
}

#[kani::proof]
#[kani::unwind(45)]
#[kani::solver(cadical)]
fn proof_v14_min_nonzero_initial_floor_is_in_health_certificate() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    group.config.min_nonzero_mm_req = 49;
    group.config.min_nonzero_im_req = 50;
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 49).unwrap();
    group.attach_leg(&mut account, 0, SideV14::Long, 1).unwrap();
    group
        .full_account_refresh(&mut account, &[1; V14_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    kani::cover!(
        account.health_cert.certified_initial_req == 50,
        "v14 tiny nonzero leg gets min initial floor"
    );
    assert_eq!(account.health_cert.certified_equity, 49);
    assert_eq!(account.health_cert.certified_initial_req, 50);
    assert!(
        account.health_cert.certified_equity < account.health_cert.certified_initial_req as i128
    );
}

#[kani::proof]
#[kani::unwind(45)]
#[kani::solver(cadical)]
fn proof_v14_full_refresh_haircuts_positive_pnl_under_global_impairment() {
    let profit: u8 = kani::any();
    let residual: u8 = kani::any();
    kani::assume(profit > 1);
    kani::assume(profit <= 20);
    kani::assume(residual > 0);
    kani::assume(residual < profit);
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));

    group.deposit_not_atomic(&mut account, 10).unwrap();
    account.pnl = profit as i128;
    group.pnl_pos_tot = profit as u128;
    group.pnl_pos_bound_tot = profit as u128;
    group.vault = group.c_tot + group.insurance + residual as u128;

    let cert = group
        .full_account_refresh(&mut account, &[1; V14_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    kani::cover!(
        residual == 1 && profit > 2,
        "v14 haircut certificate covers strongly impaired junior support"
    );
    assert_eq!(account_equity(&account), Ok(10 + profit as i128));
    assert_eq!(cert.certified_equity, 10 + residual as i128);
    assert!(cert.certified_equity < account_equity(&account).unwrap());
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v14_negative_kf_settlement_uses_haircut_support_not_face_netting() {
    let profit: u8 = kani::any();
    let residual: u8 = kani::any();
    let loss: u8 = kani::any();
    kani::assume(profit > 1);
    kani::assume(profit <= 20);
    kani::assume(residual > 0);
    kani::assume(residual < profit);
    kani::assume(loss > residual);
    kani::assume(loss <= 20);

    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group
        .attach_leg(&mut account, 0, SideV14::Long, POS_SCALE as i128)
        .unwrap();
    account.pnl = profit as i128;
    group.pnl_pos_tot = profit as u128;
    group.pnl_pos_bound_tot = profit as u128;
    group.vault = residual as u128;
    group.assets[0].k_long = -((loss as i128) * ADL_ONE as i128);

    let cert = group
        .full_account_refresh(&mut account, &[1; V14_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    let uncovered = (loss - residual) as i128;
    kani::cover!(
        profit > loss && residual < loss,
        "v14 negative K/F settlement would be positive under face netting"
    );
    assert_eq!(account.pnl, -uncovered);
    assert_eq!(group.pnl_pos_tot, 0);
    assert_eq!(group.pnl_pos_bound_tot, 0);
    assert_eq!(group.negative_pnl_account_count, 1);
    assert_eq!(cert.certified_equity, -uncovered);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v14_positive_kf_delta_cures_prior_loss_at_haircut_value() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(2, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group
        .attach_leg(&mut account, 0, SideV14::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut account, 1, SideV14::Long, POS_SCALE as i128)
        .unwrap();
    group.vault = 50;
    group.assets[0].k_long = -(100 * ADL_ONE as i128);
    group.assets[1].k_long = 100 * ADL_ONE as i128;

    let cert = group
        .full_account_refresh(&mut account, &[1; V14_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    kani::cover!(
        account.pnl == -50,
        "v14 positive K/F support cures prior loss only at haircut value"
    );
    assert_eq!(account.pnl, -50);
    assert_eq!(group.pnl_pos_tot, 0);
    assert_eq!(group.pnl_pos_bound_tot, 0);
    assert_eq!(group.negative_pnl_account_count, 1);
    assert_eq!(cert.certified_equity, -50);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_deposit_then_withdraw_roundtrip_preserves_accounting() {
    let amount: u16 = kani::any();
    kani::assume(amount > 0);
    kani::assume(amount <= 1_000);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));

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
            &[1; V14_MAX_PORTFOLIO_ASSETS_N],
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
fn proof_v14_deposit_does_not_draw_insurance_or_sweep_loss_bearing_account() {
    let amount: u16 = kani::any();
    let fee_debt: u8 = kani::any();
    kani::assume(amount > 0);
    kani::assume(amount <= 1_000);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));

    group.vault = 10;
    group.insurance = 10;
    group
        .attach_leg(&mut account, 0, SideV14::Long, 10)
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

    kani::cover!(fee_debt > 0, "v14 deposit with fee debt reachable");
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
fn proof_v14_deposit_never_sweeps_fee_debt_even_when_flat_and_nonnegative() {
    let amount: u16 = kani::any();
    let fee_debt: u8 = kani::any();
    let pnl: u8 = kani::any();
    kani::assume(amount > 0);
    kani::assume(amount <= 1_000);
    kani::assume(fee_debt > 0);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    account.pnl = pnl as i128;
    account.fee_credits = -(fee_debt as i128);

    let pnl_before = account.pnl;
    let fee_credits_before = account.fee_credits;
    group
        .deposit_not_atomic(&mut account, amount as u128)
        .unwrap();

    kani::cover!(
        pnl_before > 0 && fee_debt > 0,
        "v14 flat nonnegative deposit with fee debt reachable"
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
fn proof_v14_partial_withdraw_can_leave_small_remainder() {
    let remainder: u16 = kani::any();
    kani::assume(remainder <= 1_000);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let deposit = remainder as u128 + 1;
    group.deposit_not_atomic(&mut account, deposit).unwrap();

    group
        .withdraw_not_atomic(&mut account, 1, &[1; V14_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    kani::cover!(remainder == 0, "v14 partial withdraw leaves zero remainder");
    kani::cover!(
        remainder > 0,
        "v14 partial withdraw leaves nonzero remainder"
    );
    assert_eq!(account.capital, remainder as u128);
    assert_eq!(group.c_tot, remainder as u128);
    assert_eq!(group.vault, remainder as u128);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(45)]
#[kani::solver(cadical)]
fn proof_v14_over_withdraw_rejects_before_any_accounting_mutation() {
    let capital: u16 = kani::any();
    kani::assume(capital > 0);
    kani::assume(capital <= 1_000);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
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
        &[1; V14_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(capital > 0, "v14 over-withdraw rejection path reachable");
    assert_eq!(result, Err(V14Error::LockActive));
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
fn proof_v14_multiple_deposits_aggregate_c_tot_and_vault() {
    let amount_a: u16 = kani::any();
    let amount_b: u16 = kani::any();
    kani::assume(amount_a <= 1_000);
    kani::assume(amount_b <= 1_000);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account_a =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut account_b =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [4; 32], owner));

    group
        .deposit_not_atomic(&mut account_a, amount_a as u128)
        .unwrap();
    group
        .deposit_not_atomic(&mut account_b, amount_b as u128)
        .unwrap();

    let expected = amount_a as u128 + amount_b as u128;
    kani::cover!(expected > 0, "v14 nonzero aggregate deposit reachable");
    assert_eq!(group.c_tot, account_a.capital + account_b.capital);
    assert_eq!(group.c_tot, expected);
    assert_eq!(group.vault, expected);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_close_portfolio_account_requires_clean_local_state() {
    let dirty_case: u8 = kani::any();
    kani::assume(dirty_case < 6);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let clean = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
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
    kani::cover!(dirty_case == 0, "v14 close rejects capital");
    kani::cover!(dirty_case == 1, "v14 close rejects pnl");
    kani::cover!(dirty_case == 2, "v14 close rejects reserved pnl");
    kani::cover!(dirty_case == 3, "v14 close rejects fee debt");
    kani::cover!(dirty_case == 4, "v14 close rejects stale account");
    kani::cover!(dirty_case == 5, "v14 close rejects b-stale account");
    assert_eq!(
        group.close_portfolio_account(&dirty),
        Err(V14Error::LockActive)
    );
    assert_eq!(group.materialized_portfolio_count, 1);

    group.close_portfolio_account(&clean).unwrap();
    assert_eq!(group.materialized_portfolio_count, 0);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_risk_notional_flat_zero_and_monotone_in_price() {
    let abs_pos_q: u16 = kani::any();
    let p1: u16 = kani::any();
    let extra: u16 = kani::any();
    kani::assume(abs_pos_q <= 1_000);
    kani::assume(p1 > 0);
    kani::assume(p1 <= 1_000);
    kani::assume(extra <= 1_000);
    let p2 = p1 as u64 + extra as u64;

    assert_eq!(percolator::v14::risk_notional_ceil(0, p2), Ok(0));
    let n1 = percolator::v14::risk_notional_ceil(abs_pos_q as u128, p1 as u64).unwrap();
    let n2 = percolator::v14::risk_notional_ceil(abs_pos_q as u128, p2).unwrap();
    kani::cover!(
        abs_pos_q > 0 && extra > 0,
        "v14 risk notional monotone branch"
    );
    assert!(n2 >= n1);
}

fn concrete_ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    ([1; 32], [2; 32], [3; 32])
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_hidden_leg_rejected_by_bitmap_authority() {
    let (market, account_id, owner) = concrete_ids();
    let group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));

    account.legs[0].active = true;
    kani::cover!(
        account.active_bitmap == 0 && account.legs[0].active,
        "v14 hidden active leg reachable"
    );
    assert_eq!(
        group.validate_account_shape(&account),
        Err(V14Error::HiddenLeg)
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_configured_portfolio_width_rejects_out_of_range_leg() {
    let active_bit: bool = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    account.legs[1] = PortfolioLegV14 {
        active: true,
        side: SideV14::Long,
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

    kani::cover!(active_bit, "v14 out-of-range leg with bitmap reachable");
    kani::cover!(!active_bit, "v14 out-of-range hidden leg reachable");
    assert_eq!(
        group.validate_account_shape(&account),
        Err(V14Error::HiddenLeg)
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_attach_then_clear_leg_restores_account_local_counters_for_long() {
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));

    group.attach_leg(&mut account, 0, SideV14::Long, 7).unwrap();
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
fn proof_v14_bilateral_oi_decomposition_counts_long_short_pair() {
    let size_q = 3u128;
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut a = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut b = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [4; 32], owner));

    group
        .attach_leg(&mut a, 0, SideV14::Long, size_q as i128)
        .unwrap();
    group
        .attach_leg(&mut b, 0, SideV14::Short, -(size_q as i128))
        .unwrap();

    kani::cover!(true, "v14 bilateral OI proof covers long-short pair");
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
fn proof_v14_bilateral_oi_decomposition_counts_short_long_pair() {
    let size_q = 3u128;
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut a = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut b = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [4; 32], owner));

    group
        .attach_leg(&mut a, 0, SideV14::Short, -(size_q as i128))
        .unwrap();
    group
        .attach_leg(&mut b, 0, SideV14::Long, size_q as i128)
        .unwrap();

    kani::cover!(true, "v14 bilateral OI proof covers short-long pair");
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
fn proof_v14_oversize_position_rejected_before_oi_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));

    let result = group.attach_leg(
        &mut account,
        0,
        SideV14::Long,
        (MAX_POSITION_ABS_Q + 1) as i128,
    );

    assert_eq!(result, Err(V14Error::InvalidLeg));
    assert_eq!(account.active_bitmap, 0);
    assert_eq!(group.assets[0].oi_eff_long_q, 0);
    assert_eq!(group.assets[0].stored_pos_count_long, 0);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_account_b_chunk_either_advances_or_fails_closed() {
    let target_units: u8 = kani::any();
    let budget_units: u8 = kani::any();
    kani::assume(target_units <= 4);
    kani::assume(budget_units <= 4);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group.attach_leg(&mut account, 0, SideV14::Long, 1).unwrap();
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
        assert_eq!(result, Err(V14Error::RecoveryRequired));
        assert_eq!(account.legs[0].b_snap, before_snap);
    } else {
        let chunk = result.unwrap();
        kani::cover!(chunk.delta_b > 0, "v14 B chunk progress reachable");
        assert!(chunk.delta_b > 0);
        assert!(account.legs[0].b_snap > before_snap);
        assert!(chunk.remaining_after < before_remaining);
    }
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_repeated_account_b_chunks_complete_bounded_small_residual() {
    let target_units: u8 = kani::any();
    kani::assume((1..=2).contains(&target_units));

    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group.attach_leg(&mut account, 0, SideV14::Long, 1).unwrap();
    group.assets[0].b_long_num = target_units as u128;

    let first = group.settle_account_b_chunk(&mut account, 0, 1).unwrap();
    assert_eq!(first.delta_b, 1);
    assert_eq!(account.legs[0].b_snap, 1);
    assert_eq!(first.remaining_after, target_units as u128 - 1);

    if target_units == 2 {
        kani::cover!(true, "v14 two B chunks needed and completed");
        assert!(account.b_stale_state);
        assert!(account.legs[0].b_stale);
        let second = group.settle_account_b_chunk(&mut account, 0, 1).unwrap();
        assert_eq!(second.delta_b, 1);
        assert_eq!(second.remaining_after, 0);
    } else {
        kani::cover!(true, "v14 one B chunk completed residual");
    }

    assert_eq!(account.legs[0].b_snap, target_units as u128);
    assert_eq!(account.legs[0].b_rem, target_units as u128);
    assert_eq!(account.pnl, 0);
    assert!(!account.legs[0].b_stale);
    assert!(!account.b_stale_state);
    assert_eq!(group.b_stale_account_count, 0);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_liquidation_progress_rejects_non_reducing_scores() {
    let case: u8 = kani::any();
    let deficit: u8 = kani::any();
    let gross_loss: u8 = kani::any();
    kani::assume(case <= 3);
    kani::assume(deficit <= 5);
    kani::assume(gross_loss <= 5);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut before =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut after = before;
    group
        .full_account_refresh(&mut before, &[1; V14_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    group
        .full_account_refresh(&mut after, &[1; V14_MAX_PORTFOLIO_ASSETS_N])
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

    kani::cover!(case == 0, "v14 equal risk score non-progress reachable");
    kani::cover!(case == 1, "v14 worse deficit non-progress reachable");
    kani::cover!(case == 2, "v14 stale-penalty non-progress reachable");
    kani::cover!(case == 3, "v14 worse gross-loss non-progress reachable");

    assert_eq!(
        group.validate_liquidation_progress(&before, &after),
        Err(V14Error::NonProgress)
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_favorable_action_requires_current_full_refresh() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    account.capital = 2;

    assert_eq!(
        group.ensure_favorable_action_allowed(&account),
        Err(V14Error::Stale)
    );
    group
        .full_account_refresh(&mut account, &[1; V14_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(group.ensure_favorable_action_allowed(&account), Ok(()));
    group.oracle_epoch += 1;
    assert_eq!(
        group.ensure_favorable_action_allowed(&account),
        Err(V14Error::Stale)
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_global_residual_is_not_account_health_proof() {
    let residual_units: u8 = kani::any();
    kani::assume(residual_units > 0);
    kani::assume(residual_units <= 5);
    let residual = residual_units as u128;
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    account.pnl = residual as i128;
    account.reserved_pnl = 0;
    group.pnl_pos_tot = residual;
    group.pnl_pos_bound_tot = residual;
    group.pnl_matured_pos_tot = residual;
    group.vault = group.c_tot + group.insurance + residual;
    let before_group = group;
    let before_account = account;

    let result = group.convert_released_pnl_to_capital_not_atomic(&mut account);

    kani::cover!(
        residual > 0 && !account.health_cert.valid,
        "v14 aggregate residual with stale account certificate reachable"
    );
    assert_eq!(result, Err(V14Error::Stale));
    assert_eq!(group, before_group);
    assert_eq!(account, before_account);
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v14_favorable_locks_block_released_pnl_conversion_before_mutation() {
    let lock_case: u8 = kani::any();
    kani::assume(lock_case < 7);
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));

    group.deposit_not_atomic(&mut account, 100).unwrap();
    group
        .attach_leg(&mut account, 0, SideV14::Long, POS_SCALE as i128)
        .unwrap();
    account.pnl = 5;
    group.pnl_pos_tot = 5;
    group.pnl_pos_bound_tot = 5;
    group.pnl_matured_pos_tot = 5;
    group.vault = group.c_tot + group.insurance + 5;
    group
        .full_account_refresh(&mut account, &[1; V14_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    match lock_case {
        0 => group.threshold_stress_active = true,
        1 => group.bankruptcy_hlock_active = true,
        2 => group.loss_stale_active = true,
        3 => group.active_bankrupt_close_present = true,
        4 => account.stale_state = true,
        5 => account.b_stale_state = true,
        _ => group.assets[0].raw_oracle_target_price = 2,
    }

    let before_group = group;
    let before_account = account;
    let result = group.convert_released_pnl_to_capital_not_atomic(&mut account);

    kani::cover!(lock_case == 0, "v14 threshold-stress conversion lock");
    kani::cover!(lock_case == 1, "v14 bankruptcy h-lock conversion lock");
    kani::cover!(lock_case == 2, "v14 loss-stale conversion lock");
    kani::cover!(lock_case == 3, "v14 active-bankrupt-close conversion lock");
    kani::cover!(lock_case == 4, "v14 stale account conversion lock");
    kani::cover!(lock_case == 5, "v14 B-stale account conversion lock");
    kani::cover!(lock_case == 6, "v14 target/effective lag conversion lock");
    assert_eq!(result, Err(V14Error::LockActive));
    assert_eq!(group, before_group);
    assert_eq!(account, before_account);
}

#[kani::proof]
#[kani::unwind(20)]
#[kani::solver(cadical)]
fn proof_v14_public_invariants_reject_broken_senior_claim_conservation() {
    let vault_units: u8 = kani::any();
    let c_units: u8 = kani::any();
    let i_units: u8 = kani::any();
    kani::assume(vault_units <= 10);
    kani::assume(c_units <= 10);
    kani::assume(i_units <= 10);
    kani::assume((c_units as u16) + (i_units as u16) > vault_units as u16);

    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    group.vault = vault_units as u128;
    group.c_tot = c_units as u128;
    group.insurance = i_units as u128;

    kani::cover!(
        group.c_tot <= group.vault && group.insurance <= group.vault,
        "v14 senior sum overflow can violate conservation even when each claim is individually within vault"
    );
    assert_eq!(
        group.assert_public_invariants(),
        Err(V14Error::InvalidConfig)
    );
}

#[kani::proof]
#[kani::unwind(20)]
#[kani::solver(cadical)]
fn proof_v14_public_invariants_reject_hard_global_bounds() {
    let case: u8 = kani::any();
    kani::assume(case < 15);
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();

    match case {
        0 => group.vault = MAX_VAULT_TVL + 1,
        1 => {
            group.pnl_pos_tot = 1;
            group.pnl_pos_bound_tot = 1;
            group.pnl_matured_pos_tot = 2;
        }
        2 => {
            group.current_slot = 1;
            group.slot_last = 2;
        }
        3 => group.assets[0].effective_price = 0,
        4 => group.assets[0].oi_eff_long_q = MAX_OI_SIDE_Q + 1,
        5 => group.assets[0].loss_weight_sum_long = SOCIAL_LOSS_DEN + 1,
        6 => group.assets[0].social_loss_remainder_long_num = SOCIAL_LOSS_DEN,
        7 => group.assets[0].k_long = i128::MIN,
        8 => group.assets[0].k_short = i128::MIN,
        9 => group.assets[0].f_long_num = i128::MIN,
        10 => group.assets[0].f_short_num = i128::MIN,
        11 => group.assets[0].k_epoch_start_long = i128::MIN,
        12 => group.assets[0].k_epoch_start_short = i128::MIN,
        13 => group.assets[0].f_epoch_start_long_num = i128::MIN,
        _ => group.assets[0].f_epoch_start_short_num = i128::MIN,
    }

    kani::cover!(case == 0, "v14 vault cap violation reachable");
    kani::cover!(case == 1, "v14 matured positive PnL violation reachable");
    kani::cover!(case == 2, "v14 slot ordering violation reachable");
    kani::cover!(case == 3, "v14 zero effective price violation reachable");
    kani::cover!(case == 4, "v14 OI side cap violation reachable");
    kani::cover!(case == 5, "v14 loss weight cap violation reachable");
    kani::cover!(case == 6, "v14 social loss remainder violation reachable");
    kani::cover!(case == 7, "v14 K long i128::MIN violation reachable");
    kani::cover!(case == 8, "v14 K short i128::MIN violation reachable");
    kani::cover!(case == 9, "v14 F long i128::MIN violation reachable");
    kani::cover!(case == 10, "v14 F short i128::MIN violation reachable");
    kani::cover!(
        case == 11,
        "v14 K long epoch-start i128::MIN violation reachable"
    );
    kani::cover!(
        case == 12,
        "v14 K short epoch-start i128::MIN violation reachable"
    );
    kani::cover!(
        case == 13,
        "v14 F long epoch-start i128::MIN violation reachable"
    );
    kani::cover!(
        case == 14,
        "v14 F short epoch-start i128::MIN violation reachable"
    );
    assert_eq!(
        group.assert_public_invariants(),
        Err(V14Error::InvalidConfig)
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_cross_margin_equity_counts_collateral_once_and_score_uses_full_envelope() {
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
    let group = MarketGroupV14::new(market, V14Config::public_user_fund(2, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    account.capital = capital;
    account.fee_credits = -debt;
    account.active_bitmap = 0b11;
    account.legs[0] = PortfolioLegV14 {
        active: true,
        side: SideV14::Long,
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
    account.legs[1] = PortfolioLegV14 {
        active: true,
        side: SideV14::Short,
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
        "v14 two active legs reachable for single-collateral equity"
    );
    assert_eq!(equity, expected);

    let mut cert_account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    cert_account.health_cert.valid = true;
    cert_account.health_cert.certified_worst_case_loss = certified_loss;
    let score = group.risk_score(&cert_account).unwrap();

    kani::cover!(
        certified_loss > 1,
        "v14 full certified loss envelope reaches risk score"
    );
    assert_eq!(score.gross_risk_notional, certified_loss);
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v14_full_refresh_settles_and_scores_two_active_assets() {
    let capital_units: u8 = kani::any();
    kani::assume(capital_units <= 20);

    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(2, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    account.capital = capital_units as u128;
    group.c_tot = account.capital;
    group.vault = account.capital;

    group
        .attach_leg(&mut account, 0, SideV14::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut account, 1, SideV14::Long, POS_SCALE as i128)
        .unwrap();

    group.assets[0].k_long = ADL_ONE as i128;
    group.assets[1].k_long = -2 * (ADL_ONE as i128);
    let prices = {
        let mut out = [1u64; V14_MAX_PORTFOLIO_ASSETS_N];
        out[0] = 7;
        out[1] = 11;
        out
    };
    let expected_loss0 = risk_notional_ceil(POS_SCALE, prices[0]).unwrap();
    let expected_loss1 = risk_notional_ceil(POS_SCALE, prices[1]).unwrap();

    let cert = group.full_account_refresh(&mut account, &prices).unwrap();

    kani::cover!(
        capital_units == 0,
        "v14 two-asset refresh covers negative-equity account"
    );
    kani::cover!(
        capital_units > 1,
        "v14 two-asset refresh covers positive-equity account"
    );
    assert_eq!(account.active_bitmap, 0b11);
    assert_eq!(account.pnl, -1);
    assert_eq!(account.legs[0].k_snap, ADL_ONE as i128);
    assert_eq!(account.legs[1].k_snap, -2 * (ADL_ONE as i128));
    assert_eq!(
        cert.certified_worst_case_loss,
        expected_loss0 + expected_loss1
    );
    assert_eq!(
        cert.certified_maintenance_req,
        expected_loss0 + expected_loss1
    );
    assert_eq!(cert.certified_equity, capital_units as i128 - 1);
    assert_eq!(cert.active_bitmap_at_cert, 0b11);
    assert_eq!(group.validate_account_shape(&account), Ok(()));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_full_refresh_clears_stale_certificate() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));

    group.mark_account_stale(&mut account).unwrap();
    assert_eq!(group.stale_certificate_count, 1);
    group
        .full_account_refresh(&mut account, &[1; V14_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    kani::cover!(
        !account.stale_state,
        "v14 stale account refresh clears stale state"
    );
    assert!(!account.stale_state);
    assert_eq!(group.stale_certificate_count, 0);
    assert_eq!(group.ensure_favorable_action_allowed(&account), Ok(()));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_b_stale_blocks_refresh_and_favorable_actions() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));

    group
        .full_account_refresh(&mut account, &[1; V14_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(group.ensure_favorable_action_allowed(&account), Ok(()));

    group.mark_account_b_stale(&mut account).unwrap();
    kani::cover!(
        account.b_stale_state && !account.health_cert.valid,
        "v14 b-stale invalidates prior health certificate"
    );

    assert_eq!(
        group.full_account_refresh(&mut account, &[1; V14_MAX_PORTFOLIO_ASSETS_N]),
        Err(V14Error::BStale)
    );
    assert_eq!(
        group.ensure_favorable_action_allowed(&account),
        Err(V14Error::LockActive)
    );
    assert!(account.b_stale_state);
    assert_eq!(group.b_stale_account_count, 1);
}

#[kani::proof]
#[kani::unwind(60)]
#[kani::solver(cadical)]
fn proof_v14_b_stale_trade_preflight_rolls_back_partial_side_effects() {
    let (market, account_id, owner) = concrete_ids();
    let mut cfg = V14Config::public_user_fund(1, 0, 1);
    cfg.public_b_chunk_atoms = 1;
    let mut group = MarketGroupV14::new(market, cfg).unwrap();
    let mut long = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut short = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut long, 100).unwrap();
    group.deposit_not_atomic(&mut short, 100).unwrap();
    group.attach_leg(&mut long, 0, SideV14::Long, 1).unwrap();
    group.assets[0].b_long_num = 2;

    let before_group = group;
    let before_long = long;
    let before_short = short;
    let result = group.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV14 {
            asset_index: 0,
            size_q: 1,
            exec_price: 1,
            fee_bps: 0,
        },
        &[1; V14_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(
        before_group.assets[0].b_long_num > before_long.legs[0].b_snap,
        "v14 trade preflight reaches partial B-stale side effect"
    );
    assert_eq!(result, Err(V14Error::BStale));
    assert_eq!(group, before_group);
    assert_eq!(long, before_long);
    assert_eq!(short, before_short);
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v14_deposit_into_stale_or_b_stale_account_does_not_unlock_favorable_actions() {
    let stale_case: bool = kani::any();
    let deposit_units: u8 = kani::any();
    kani::assume(deposit_units > 0);
    kani::assume(deposit_units <= 20);
    let deposit = deposit_units as u128;
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));

    group
        .full_account_refresh(&mut account, &[1; V14_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    if stale_case {
        group.mark_account_stale(&mut account).unwrap();
    } else {
        group.mark_account_b_stale(&mut account).unwrap();
    }
    let stale_before = group.stale_certificate_count;
    let b_stale_before = group.b_stale_account_count;

    group.deposit_not_atomic(&mut account, deposit).unwrap();

    kani::cover!(stale_case, "v14 deposit into stale account reachable");
    kani::cover!(!stale_case, "v14 deposit into B-stale account reachable");
    assert_eq!(account.capital, deposit);
    assert_eq!(group.c_tot, deposit);
    assert_eq!(group.vault, deposit);
    assert_eq!(group.stale_certificate_count, stale_before);
    assert_eq!(group.b_stale_account_count, b_stale_before);
    assert!(!account.health_cert.valid);
    assert_eq!(
        group.ensure_favorable_action_allowed(&account),
        Err(V14Error::LockActive)
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_side_reset_prior_epoch_account_can_clear_without_oi_underflow() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group.attach_leg(&mut account, 0, SideV14::Long, 1).unwrap();
    group.assets[0].oi_eff_long_q = 0;

    group.begin_full_drain_reset(0, SideV14::Long).unwrap();
    assert_eq!(group.assets[0].oi_eff_long_q, 0);
    group.clear_leg(&mut account, 0).unwrap();
    assert_eq!(group.assets[0].stored_pos_count_long, 0);
    assert_eq!(group.assets[0].oi_eff_long_q, 0);
    group.finalize_ready_reset_side(0, SideV14::Long).unwrap();
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_side_reset_finalize_requires_prior_epoch_positions_clear() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group.attach_leg(&mut account, 0, SideV14::Long, 1).unwrap();
    group.assets[0].oi_eff_long_q = 0;

    group.begin_full_drain_reset(0, SideV14::Long).unwrap();
    kani::cover!(
        group.assets[0].stored_pos_count_long != 0,
        "v14 reset pending with prior-epoch stored position reachable"
    );
    assert_eq!(
        group.finalize_ready_reset_side(0, SideV14::Long),
        Err(V14Error::Stale)
    );

    group.clear_leg(&mut account, 0).unwrap();
    assert_eq!(group.finalize_ready_reset_side(0, SideV14::Long), Ok(()));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_quantity_adl_preserves_oi_symmetry_after_close() {
    let close_q: u8 = kani::any();
    kani::assume(close_q > 0);
    kani::assume(close_q <= 4);
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [9; 32], [8; 32]));
    let mut opposing =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [10; 32], [8; 32]));
    group
        .attach_leg(&mut account, 0, SideV14::Long, close_q as i128)
        .unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV14::Short, -(close_q as i128))
        .unwrap();
    account.close_progress = CloseProgressLedgerV14 {
        active: true,
        finalized: true,
        close_id: 1,
        asset_index: 0,
        domain_side: SideV14::Short,
        gross_loss_at_close_start: 1,
        explicit_loss_assigned: 1,
        residual_remaining: 0,
        ..CloseProgressLedgerV14::EMPTY
    };

    let out = group
        .apply_quantity_adl_after_residual_for_account_not_atomic(
            &mut account,
            0,
            SideV14::Long,
            close_q as u128,
        )
        .unwrap();
    kani::cover!(out.closed_q > 0, "v14 quantity ADL close reachable");
    assert_eq!(
        account.close_progress.quantity_adl_applied_q,
        close_q as u128
    );
    assert_eq!(account.active_bitmap, 0);
    assert!(!account.legs[0].active);
    assert_eq!(
        group.assets[0].oi_eff_long_q,
        group.assets[0].oi_eff_short_q
    );
    assert_eq!(group.assets[0].oi_eff_long_q, 0);
    assert!(out.reset_started);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_quantity_adl_monotonically_shrinks_opposing_a_or_resets() {
    let oi_before: u8 = kani::any();
    let close_q: u8 = kani::any();
    kani::assume(oi_before > 0);
    kani::assume(oi_before <= 4);
    kani::assume(close_q > 0);
    kani::assume(close_q <= oi_before);

    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [9; 32], [8; 32]));
    let mut survivor =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [10; 32], [8; 32]));
    let mut opposing =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [11; 32], [8; 32]));
    let oi_before = oi_before as u128;
    let close_q = close_q as u128;
    group
        .attach_leg(&mut account, 0, SideV14::Long, close_q as i128)
        .unwrap();
    let survivor_q = oi_before - close_q;
    if survivor_q != 0 {
        group
            .attach_leg(&mut survivor, 0, SideV14::Long, survivor_q as i128)
            .unwrap();
    }
    group
        .attach_leg(&mut opposing, 0, SideV14::Short, -(oi_before as i128))
        .unwrap();
    account.close_progress = CloseProgressLedgerV14 {
        active: true,
        finalized: true,
        close_id: 1,
        asset_index: 0,
        domain_side: SideV14::Short,
        gross_loss_at_close_start: 1,
        explicit_loss_assigned: 1,
        residual_remaining: 0,
        ..CloseProgressLedgerV14::EMPTY
    };
    group.assets[0].a_short = ADL_ONE;
    let a_before = group.assets[0].a_short;

    let out = group
        .apply_quantity_adl_after_residual_for_account_not_atomic(
            &mut account,
            0,
            SideV14::Long,
            close_q,
        )
        .unwrap();

    let oi_after = oi_before - close_q;
    kani::cover!(oi_after > 0, "v14 partial quantity ADL branch reachable");
    kani::cover!(
        oi_after == 0,
        "v14 full-drain quantity ADL branch reachable"
    );
    assert_eq!(out.closed_q, close_q);
    assert_eq!(account.active_bitmap, 0);
    assert!(!account.legs[0].active);
    assert_eq!(group.assets[0].oi_eff_long_q, oi_after);
    assert_eq!(group.assets[0].oi_eff_short_q, oi_after);
    if oi_after == 0 {
        assert!(out.reset_started);
        assert_eq!(group.assets[0].a_short, ADL_ONE);
    } else {
        assert!(!out.reset_started);
        assert!(group.assets[0].a_short > 0);
        assert!(group.assets[0].a_short < a_before);
    }
    assert_eq!(account.close_progress.quantity_adl_applied_q, close_q);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(60)]
#[kani::solver(cadical)]
fn proof_v14_dead_leg_forfeit_does_not_credit_positive_kf_delta() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group.mode = MarketModeV14::Recovery;
    group
        .attach_leg(&mut account, 0, SideV14::Long, POS_SCALE as i128)
        .unwrap();
    group.assets[0].k_long = 3 * ADL_ONE as i128;

    let out = group
        .forfeit_recovery_leg_not_atomic(&mut account, 0, 1)
        .unwrap();

    kani::cover!(
        out.positive_pnl_forfeited > 0,
        "v14 dead-leg positive K/F delta is forfeited"
    );
    assert!(out.detached);
    assert_eq!(out.positive_pnl_forfeited, 3);
    assert_eq!(account.pnl, 0);
    assert_eq!(group.pnl_pos_tot, 0);
    assert_eq!(account.active_bitmap, 0);
    assert_eq!(group.assets[0].oi_eff_long_q, 0);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v14_dead_leg_forfeit_books_loss_to_opposing_domain_only() {
    let loss_units: u8 = kani::any();
    kani::assume(loss_units > 0);
    kani::assume(loss_units <= 4);
    let loss = loss_units as u128;

    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut opposing =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [10; 32], owner));
    group.mode = MarketModeV14::Recovery;
    group
        .attach_leg(&mut account, 0, SideV14::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV14::Short, -(POS_SCALE as i128))
        .unwrap();
    group.assets[0].mode_long = SideModeV14::DrainOnly;
    group.assets[0].k_long = -((loss as i128) * ADL_ONE as i128);
    let b_long_before = group.assets[0].b_long_num;
    let b_short_before = group.assets[0].b_short_num;

    let out = group
        .forfeit_recovery_leg_not_atomic(&mut account, 0, loss)
        .unwrap();

    kani::cover!(
        out.residual_booked > 0,
        "v14 dead-leg negative K/F delta books durable opposing-domain loss"
    );
    assert!(out.detached);
    assert_eq!(out.loss_settled, loss);
    assert_eq!(out.residual_booked, loss);
    assert_eq!(out.insurance_used, 0);
    assert_eq!(account.pnl, 0);
    assert_eq!(group.assets[0].oi_eff_long_q, 0);
    assert_eq!(group.assets[0].oi_eff_short_q, POS_SCALE);
    assert_eq!(group.assets[0].b_long_num, b_long_before);
    assert!(group.assets[0].b_short_num > b_short_before);
    assert_eq!(
        group.pending_domain_loss_barrier_count(0, SideV14::Short),
        Ok(0)
    );
    assert!(account.close_progress.finalized);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v14_dead_leg_forfeit_haircuts_positive_support_when_junior_impaired() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut opposing =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [11; 32], owner));
    group.mode = MarketModeV14::Recovery;
    group
        .attach_leg(&mut account, 0, SideV14::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV14::Short, -(POS_SCALE as i128))
        .unwrap();
    group.assets[0].mode_long = SideModeV14::DrainOnly;
    group.assets[0].k_long = -(100 * ADL_ONE as i128);
    account.pnl = 100;
    group.pnl_pos_tot = 100;
    group.pnl_pos_bound_tot = 100;
    group.vault = 50;

    let out = group
        .forfeit_recovery_leg_not_atomic(&mut account, 0, 50)
        .unwrap();

    kani::cover!(
        out.support_consumed == 50 && out.junior_face_burned == 100,
        "v14 impaired positive support burns full face for haircut value"
    );
    assert!(out.detached);
    assert_eq!(out.loss_settled, 100);
    assert_eq!(out.support_consumed, 50);
    assert_eq!(out.junior_face_burned, 100);
    assert_eq!(out.residual_booked, 50);
    assert_eq!(out.insurance_used, 0);
    assert_eq!(account.pnl, 0);
    assert_eq!(group.pnl_pos_tot, 0);
    assert_eq!(group.pnl_pos_bound_tot, 0);
    assert!(group.assets[0].b_short_num > 0);
    assert_eq!(account.close_progress.gross_loss_at_close_start, 100);
    assert_eq!(account.close_progress.support_consumed, 50);
    assert_eq!(account.close_progress.junior_face_burned, 100);
    assert!(account.close_progress.finalized);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_fee_charge_settles_loss_before_fee() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));

    group.deposit_not_atomic(&mut account, 1).unwrap();
    account.pnl = -1;
    group.negative_pnl_account_count = 1;
    let charged = group
        .charge_account_fee_not_atomic(&mut account, 1)
        .unwrap();

    kani::cover!(
        account.pnl < 0 || charged == 0,
        "v14 loss-before-fee path reached"
    );
    assert_eq!(charged, 0);
    assert_eq!(account.capital, 0);
    assert_eq!(account.pnl, 0);
    assert_eq!(group.insurance, 0);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_fee_sync_uses_wide_product_and_drops_uncollectible_tail() {
    let capital: u8 = kani::any();
    kani::assume(capital > 0);
    kani::assume(capital <= 20);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group
        .deposit_not_atomic(&mut account, capital as u128)
        .unwrap();

    let charged = group
        .sync_account_fee_to_slot_not_atomic(&mut account, 2, u128::MAX)
        .unwrap();

    kani::cover!(
        charged == capital as u128,
        "v14 fee sync wide-product cap path charges available principal"
    );
    assert_eq!(charged, capital as u128);
    assert_eq!(account.last_fee_slot, 2);
    assert_eq!(account.capital, 0);
    assert_eq!(account.fee_credits, 0);
    assert_eq!(group.insurance, capital as u128);
    assert_eq!(group.c_tot, 0);
    assert_eq!(group.vault, capital as u128);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(60)]
#[kani::solver(cadical)]
fn proof_v14_non_deficit_public_paths_do_not_decrease_insurance() {
    let case: u8 = kani::any();
    let capital_units: u8 = kani::any();
    let insurance_units: u8 = kani::any();
    let requested_fee_units: u8 = kani::any();
    let amount_units: u8 = kani::any();
    kani::assume(case < 5);
    kani::assume(capital_units > 0);
    kani::assume(capital_units <= 20);
    kani::assume(insurance_units <= 20);
    kani::assume(requested_fee_units <= 20);
    kani::assume(amount_units <= capital_units);

    let capital = capital_units as u128;
    let insurance = insurance_units as u128;
    let requested_fee = requested_fee_units as u128;
    let amount = amount_units as u128;
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    account.capital = capital;
    group.c_tot = capital;
    group.insurance = insurance;
    group.vault = capital + insurance;
    let insurance_before = group.insurance;

    match case {
        0 => {
            group.deposit_not_atomic(&mut account, amount).unwrap();
            kani::cover!(amount > 0, "v14 deposit non-deficit insurance boundary");
            assert_eq!(group.insurance, insurance_before);
        }
        1 => {
            group
                .withdraw_not_atomic(&mut account, amount, &[1; V14_MAX_PORTFOLIO_ASSETS_N])
                .unwrap();
            kani::cover!(amount > 0, "v14 withdraw non-deficit insurance boundary");
            assert_eq!(group.insurance, insurance_before);
        }
        2 => {
            let charged = group
                .charge_account_fee_not_atomic(&mut account, requested_fee)
                .unwrap();
            kani::cover!(
                requested_fee > 0,
                "v14 fee charge can increase but not decrease insurance"
            );
            assert_eq!(group.insurance, insurance_before + charged);
        }
        3 => {
            let profit = 3u128;
            account.pnl = profit as i128;
            group.pnl_pos_tot = profit;
            group.pnl_pos_bound_tot = profit;
            group.pnl_matured_pos_tot = profit;
            group.vault = group.c_tot + group.insurance + profit;
            account.health_cert.valid = true;
            account.health_cert.cert_oracle_epoch = group.oracle_epoch;
            account.health_cert.cert_funding_epoch = group.funding_epoch;
            account.health_cert.cert_risk_epoch = group.risk_epoch;
            account.health_cert.active_bitmap_at_cert = account.active_bitmap;

            group
                .convert_released_pnl_to_capital_not_atomic(&mut account)
                .unwrap();
            kani::cover!(true, "v14 released pnl conversion preserves insurance");
            assert_eq!(group.insurance, insurance_before);
        }
        _ => {
            group.resolve_market_not_atomic(1).unwrap();
            let outcome = group.close_resolved_account_not_atomic(&mut account, 0);
            kani::cover!(true, "v14 non-deficit resolved close preserves insurance");
            assert_eq!(
                outcome,
                Ok(ResolvedCloseOutcomeV14::Closed { payout: capital })
            );
            assert_eq!(group.insurance, insurance_before);
        }
    }

    assert!(group.insurance >= insurance_before);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_direct_fee_charge_is_live_only_without_resolved_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 5).unwrap();
    group.resolve_market_not_atomic(1).unwrap();
    let before_group = group;
    let before_account = account;

    let result = group.charge_account_fee_not_atomic(&mut account, 1);

    kani::cover!(
        group.mode == MarketModeV14::Resolved,
        "v14 direct fee charge resolved-mode rejection reachable"
    );
    assert_eq!(result, Err(V14Error::LockActive));
    assert_eq!(group, before_group);
    assert_eq!(account, before_account);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_equity_active_accrual_requires_protective_progress() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group
        .attach_leg(&mut account, 0, SideV14::Long, POS_SCALE as i128)
        .unwrap();

    let result = group.accrue_asset_to_not_atomic(0, 1, 2, 0, false);
    assert_eq!(result, Err(V14Error::NonProgress));
    assert_eq!(group.slot_last, 0);

    let ok = group.accrue_asset_to_not_atomic(0, 1, 2, 0, true);
    assert!(ok.is_ok());
    assert_eq!(group.slot_last, 1);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_active_bankrupt_close_does_not_freeze_asset_accrual() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group
        .attach_leg(&mut account, 0, SideV14::Long, POS_SCALE as i128)
        .unwrap();
    group.active_bankrupt_close_present = true;
    let before_a_long = group.assets[0].a_long;
    let before_b_short = group.assets[0].b_short_num;
    let before_oi_long = group.assets[0].oi_eff_long_q;

    let out = group.accrue_asset_to_not_atomic(0, 1, 2, 0, true).unwrap();

    kani::cover!(
        out.equity_active,
        "v14 active close accrual remains reachable"
    );
    assert!(out.equity_active);
    assert_eq!(out.dt, 1);
    assert_eq!(group.assets[0].effective_price, 2);
    assert_eq!(group.assets[0].a_long, before_a_long);
    assert_eq!(group.assets[0].b_short_num, before_b_short);
    assert_eq!(group.assets[0].oi_eff_long_q, before_oi_long);
    assert!(group.active_bankrupt_close_present);
    assert_eq!(
        group.h_lock_lane(Some(&account), false),
        Ok(HLockLaneV14::HMax)
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_permissionless_crank_does_not_require_full_market_scan() {
    let stale_count: u16 = kani::any();
    let b_stale_count: u16 = kani::any();
    let negative_count: u16 = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
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
            PermissionlessCrankRequestV14 {
                now_slot: 0,
                asset_index: 0,
                effective_price: 1,
                funding_rate_e9: 0,
                action: PermissionlessCrankActionV14::Refresh,
            },
            &[1; V14_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    kani::cover!(
        stale_count > 0 || b_stale_count > 0 || negative_count > 0,
        "v14 permissionless hinted progress ignores unrelated global account counters"
    );
    assert_eq!(out, PermissionlessProgressOutcomeV14::AccountCurrent);
    assert!(account.health_cert.valid);
    assert_eq!(group.materialized_portfolio_count, before_materialized);
    assert_eq!(group.stale_certificate_count, before_stale);
    assert_eq!(group.b_stale_account_count, before_b_stale);
    assert_eq!(group.negative_pnl_account_count, before_negative);
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v14_permissionless_refresh_returns_partial_b_progress_without_accrual() {
    let larger_target: bool = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let mut cfg = V14Config::public_user_fund(1, 0, 1);
    cfg.public_b_chunk_atoms = 1;
    let mut group = MarketGroupV14::new(market, cfg).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));

    group.deposit_not_atomic(&mut account, 100).unwrap();
    group.attach_leg(&mut account, 0, SideV14::Long, 1).unwrap();
    group.assets[0].b_long_num = if larger_target { 3 } else { 2 };
    let outcome = group.permissionless_crank_not_atomic(
        &mut account,
        PermissionlessCrankRequestV14 {
            now_slot: 1,
            asset_index: 0,
            effective_price: 1,
            funding_rate_e9: 0,
            action: PermissionlessCrankActionV14::Refresh,
        },
        &[1; V14_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(
        !larger_target,
        "v14 permissionless refresh partial B target two"
    );
    kani::cover!(
        larger_target,
        "v14 permissionless refresh partial B target three"
    );
    assert!(matches!(
        outcome,
        Ok(PermissionlessProgressOutcomeV14::AccountBChunk(_))
    ));
    assert!(account.legs[0].b_stale);
    assert!(account.legs[0].b_snap > 0);
    assert!(account.legs[0].b_snap < group.assets[0].b_long_num);
    assert_eq!(group.slot_last, 0);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_permissionless_flat_refresh_is_not_protective_for_equity_active_accrual() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(2, 0, 1)).unwrap();
    group.assets[0].oi_eff_long_q = 1;
    group.assets[0].oi_eff_short_q = 1;
    let before_asset = group.assets[0];
    let before_slot = group.slot_last;

    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 1).unwrap();

    let outcome = group.permissionless_crank_not_atomic(
        &mut account,
        PermissionlessCrankRequestV14 {
            now_slot: 1,
            asset_index: 0,
            effective_price: 2,
            funding_rate_e9: 0,
            action: PermissionlessCrankActionV14::Refresh,
        },
        &[1; V14_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(
        outcome == Err(V14Error::NonProgress),
        "v14 flat refresh is not protective for exposed asset accrual"
    );
    assert_eq!(outcome, Err(V14Error::NonProgress));
    assert_eq!(group.assets[0], before_asset);
    assert_eq!(group.slot_last, before_slot);
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v14_permissionless_cross_asset_liquidation_is_not_protective_for_equity_active_accrual() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(2, 0, 1)).unwrap();
    group.assets[0].oi_eff_long_q = 1;
    group.assets[0].oi_eff_short_q = 1;
    let before_asset = group.assets[0];
    let before_slot = group.slot_last;

    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group.attach_leg(&mut account, 1, SideV14::Long, 1).unwrap();

    let outcome = group.permissionless_crank_not_atomic(
        &mut account,
        PermissionlessCrankRequestV14 {
            now_slot: 1,
            asset_index: 0,
            effective_price: 2,
            funding_rate_e9: 0,
            action: PermissionlessCrankActionV14::Liquidate(LiquidationRequestV14 {
                asset_index: 1,
                close_q: 1,
                fee_bps: 0,
            }),
        },
        &[1; V14_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(
        outcome == Err(V14Error::NonProgress),
        "v14 cross-asset liquidation is not protective for exposed asset accrual"
    );
    assert_eq!(outcome, Err(V14Error::NonProgress));
    assert_eq!(group.assets[0], before_asset);
    assert_eq!(group.slot_last, before_slot);
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v14_worst_case_hinted_progress_actions_are_total_and_bounded() {
    let case: u8 = kani::any();
    kani::assume(case < 4);
    let (market, account_id, owner) = concrete_ids();
    let base_req = PermissionlessCrankRequestV14 {
        now_slot: 0,
        asset_index: 0,
        effective_price: 1,
        funding_rate_e9: 0,
        action: PermissionlessCrankActionV14::Refresh,
    };

    match case {
        0 => {
            let mut group =
                MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
            let mut account =
                PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
            group.deposit_not_atomic(&mut account, 1).unwrap();
            let outcome = group.permissionless_crank_not_atomic(
                &mut account,
                base_req,
                &[1; V14_MAX_PORTFOLIO_ASSETS_N],
            );
            kani::cover!(true, "v14 hinted refresh-current branch reachable");
            assert_eq!(
                outcome,
                Ok(PermissionlessProgressOutcomeV14::AccountCurrent)
            );
            assert!(account.health_cert.valid);
        }
        1 => {
            let mut cfg = V14Config::public_user_fund(1, 0, 1);
            cfg.public_b_chunk_atoms = 1;
            let mut group = MarketGroupV14::new(market, cfg).unwrap();
            let mut account =
                PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
            group.attach_leg(&mut account, 0, SideV14::Long, 1).unwrap();
            group.assets[0].b_long_num = 2;
            let outcome = group.permissionless_crank_not_atomic(
                &mut account,
                PermissionlessCrankRequestV14 {
                    action: PermissionlessCrankActionV14::SettleB { asset_index: 0 },
                    ..base_req
                },
                &[1; V14_MAX_PORTFOLIO_ASSETS_N],
            );
            kani::cover!(true, "v14 hinted settle-B branch reachable");
            match outcome {
                Ok(PermissionlessProgressOutcomeV14::AccountBChunk(chunk)) => {
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
                MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
            let mut account =
                PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
            group
                .attach_leg(&mut account, 0, SideV14::Long, POS_SCALE as i128)
                .unwrap();
            let outcome = group.permissionless_crank_not_atomic(
                &mut account,
                PermissionlessCrankRequestV14 {
                    action: PermissionlessCrankActionV14::Liquidate(LiquidationRequestV14 {
                        asset_index: 0,
                        close_q: POS_SCALE,
                        fee_bps: 0,
                    }),
                    ..base_req
                },
                &[1; V14_MAX_PORTFOLIO_ASSETS_N],
            );
            kani::cover!(true, "v14 hinted liquidation branch reachable");
            assert_eq!(
                outcome,
                Ok(PermissionlessProgressOutcomeV14::AccountCurrent)
            );
            assert_eq!(account.active_bitmap, 0);
        }
        _ => {
            let mut group =
                MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
            let mut account =
                PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
            let reason = PermissionlessRecoveryReasonV14::BelowProgressFloor;
            let outcome = group.permissionless_crank_not_atomic(
                &mut account,
                PermissionlessCrankRequestV14 {
                    action: PermissionlessCrankActionV14::Recover(reason),
                    ..base_req
                },
                &[1; V14_MAX_PORTFOLIO_ASSETS_N],
            );
            kani::cover!(true, "v14 hinted recovery branch reachable");
            assert_eq!(
                outcome,
                Ok(PermissionlessProgressOutcomeV14::RecoveryDeclared(reason))
            );
            assert_eq!(group.recovery_reason, Some(reason));
        }
    }
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_equity_active_accrual_advances_at_most_one_bounded_segment() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    group.config.max_accrual_dt_slots = 2;
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group
        .attach_leg(&mut account, 0, SideV14::Long, POS_SCALE as i128)
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
fn proof_v14_funding_rate_above_cap_rejects_before_mutation() {
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    group.config.max_abs_funding_e9_per_slot = 1;
    let before = group.assets[0];

    let result = group.accrue_asset_to_not_atomic(0, 1, 1, 2, true);

    assert_eq!(result, Err(V14Error::InvalidConfig));
    assert_eq!(group.assets[0], before);
    assert_eq!(group.slot_last, 0);
    assert_eq!(group.current_slot, 0);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_trade_dynamic_fee_cap_is_enforced_before_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    group.config.max_trading_fee_bps = 1;
    let mut long = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut short = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut long, 10).unwrap();
    group.deposit_not_atomic(&mut short, 10).unwrap();

    let result = group.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV14 {
            asset_index: 0,
            size_q: 1,
            exec_price: 1,
            fee_bps: 2,
        },
        &[1; V14_MAX_PORTFOLIO_ASSETS_N],
    );
    assert_eq!(result, Err(V14Error::InvalidConfig));
    assert_eq!(long.active_bitmap, 0);
    assert_eq!(short.active_bitmap, 0);
}

#[kani::proof]
#[kani::unwind(60)]
#[kani::solver(cadical)]
fn proof_v14_trade_fee_conservation_and_oi_symmetry() {
    let fee_bps: u16 = kani::any();
    kani::assume(fee_bps <= 1_000);
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    group.config.max_trading_fee_bps = 1_000;
    let mut long = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut short = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut long, 10_000).unwrap();
    group.deposit_not_atomic(&mut short, 10_000).unwrap();
    let vault_before = group.vault;
    let c_tot_before = group.c_tot;
    let insurance_before = group.insurance;

    let out = group
        .execute_trade_with_fee_not_atomic(
            &mut long,
            &mut short,
            TradeRequestV14 {
                asset_index: 0,
                size_q: POS_SCALE,
                exec_price: 100,
                fee_bps: fee_bps as u64,
            },
            &[100; V14_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    let expected_fee = if fee_bps == 0 {
        0
    } else {
        ((100u128 * fee_bps as u128) + 9_999) / 10_000
    };
    kani::cover!(fee_bps == 0, "v14 zero fee trade reachable");
    kani::cover!(expected_fee > 0, "v14 positive fee trade reachable");
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
fn proof_v14_risk_increasing_trade_requires_initial_health_before_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut underfunded_long =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut funded_short =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut funded_short, 10_000).unwrap();
    let before_group = group;
    let before_long = underfunded_long;
    let before_short = funded_short;

    let result = group.execute_trade_with_fee_not_atomic(
        &mut underfunded_long,
        &mut funded_short,
        TradeRequestV14 {
            asset_index: 0,
            size_q: POS_SCALE,
            exec_price: 100,
            fee_bps: 0,
        },
        &[100; V14_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(result, Err(V14Error::InvalidConfig));
    assert_eq!(group, before_group);
    assert_eq!(underfunded_long, before_long);
    assert_eq!(funded_short, before_short);
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v14_trade_hint_cannot_hide_toxic_portfolio_leg_on_other_asset() {
    let hidden_loss_units: u8 = kani::any();
    kani::assume((2..=5).contains(&hidden_loss_units));
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(2, 0, 1)).unwrap();
    let mut long = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut short = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut long, 1).unwrap();
    group.deposit_not_atomic(&mut short, 1_000).unwrap();
    group
        .attach_leg(&mut long, 1, SideV14::Long, POS_SCALE as i128)
        .unwrap();
    group.assets[1].k_long = -((hidden_loss_units as i128) * (ADL_ONE as i128));
    let before_group = group;
    let before_long = long;
    let before_short = short;

    let result = group.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV14 {
            asset_index: 0,
            size_q: POS_SCALE,
            exec_price: 1,
            fee_bps: 0,
        },
        &[1; V14_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(
        hidden_loss_units > 1,
        "v14 trade hint with toxic unhinted active leg reachable"
    );
    assert!(result.is_err());
    assert_eq!(group, before_group);
    assert_eq!(long, before_long);
    assert_eq!(short, before_short);
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v14_sign_flip_trade_preserves_oi_symmetry_and_senior_accounting() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut flip_to_long =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut flip_to_short =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut flip_to_long, 10_000).unwrap();
    group
        .deposit_not_atomic(&mut flip_to_short, 10_000)
        .unwrap();
    group
        .attach_leg(&mut flip_to_long, 0, SideV14::Short, -(POS_SCALE as i128))
        .unwrap();
    group
        .attach_leg(&mut flip_to_short, 0, SideV14::Long, POS_SCALE as i128)
        .unwrap();
    let vault_before = group.vault;
    let c_tot_before = group.c_tot;

    group
        .execute_trade_with_fee_not_atomic(
            &mut flip_to_long,
            &mut flip_to_short,
            TradeRequestV14 {
                asset_index: 0,
                size_q: 2 * POS_SCALE,
                exec_price: 1,
                fee_bps: 0,
            },
            &[1; V14_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    kani::cover!(true, "v14 sign-flip trade transition reachable");
    assert_eq!(flip_to_long.legs[0].side, SideV14::Long);
    assert_eq!(flip_to_long.legs[0].basis_pos_q, POS_SCALE as i128);
    assert_eq!(flip_to_short.legs[0].side, SideV14::Short);
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
fn proof_v14_hlock_rejects_risk_increasing_trade_before_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut long = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut short = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut long, 10).unwrap();
    group.deposit_not_atomic(&mut short, 10).unwrap();
    group.threshold_stress_active = true;

    let result = group.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV14 {
            asset_index: 0,
            size_q: 1,
            exec_price: 1,
            fee_bps: 0,
        },
        &[1; V14_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(result, Err(V14Error::LockActive));
    assert_eq!(long.active_bitmap, 0);
    assert_eq!(short.active_bitmap, 0);
    assert_eq!(group.insurance, 0);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_target_effective_lag_rejects_risk_increasing_trade_before_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut long = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut short = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut long, 10).unwrap();
    group.deposit_not_atomic(&mut short, 10).unwrap();
    group.assets[0].effective_price = 1;
    group.assets[0].raw_oracle_target_price = 2;

    let result = group.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV14 {
            asset_index: 0,
            size_q: 1,
            exec_price: 1,
            fee_bps: 0,
        },
        &[1; V14_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(result, Err(V14Error::LockActive));
    assert_eq!(long.active_bitmap, 0);
    assert_eq!(short.active_bitmap, 0);
    assert_eq!(group.insurance, 0);
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v14_hlock_allows_pure_risk_reducing_trade_with_principal_margin() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut reducing_short =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut reducing_long =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut reducing_short, 100).unwrap();
    group.deposit_not_atomic(&mut reducing_long, 100).unwrap();
    group
        .attach_leg(&mut reducing_short, 0, SideV14::Short, -10)
        .unwrap();
    group
        .attach_leg(&mut reducing_long, 0, SideV14::Long, 10)
        .unwrap();
    group.threshold_stress_active = true;

    let result = group.execute_trade_with_fee_not_atomic(
        &mut reducing_short,
        &mut reducing_long,
        TradeRequestV14 {
            asset_index: 0,
            size_q: 5,
            exec_price: 1,
            fee_bps: 0,
        },
        &[1; V14_MAX_PORTFOLIO_ASSETS_N],
    );

    assert!(result.is_ok());
    assert_eq!(reducing_short.legs[0].basis_pos_q, -5);
    assert_eq!(reducing_long.legs[0].basis_pos_q, 5);
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v14_hlock_withdraw_uses_no_positive_credit_lane() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 20).unwrap();
    group
        .attach_leg(&mut account, 0, SideV14::Long, 10)
        .unwrap();
    account.pnl = 100;
    group.pnl_pos_tot = 100;
    group.pnl_pos_bound_tot = 100;
    group.threshold_stress_active = true;

    let result =
        group.withdraw_not_atomic(&mut account, 11, &[1_000_000; V14_MAX_PORTFOLIO_ASSETS_N]);

    assert_eq!(result, Err(V14Error::InvalidConfig));
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v14_released_pnl_conversion_is_residual_bounded_and_conserves_vault() {
    let profit: u8 = kani::any();
    let residual: u8 = kani::any();
    kani::assume(profit <= 10);
    kani::assume(residual <= 10);
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));

    group.deposit_not_atomic(&mut account, 10).unwrap();
    account.pnl = profit as i128;
    group.pnl_pos_tot = profit as u128;
    group.pnl_pos_bound_tot = profit as u128;
    group.pnl_matured_pos_tot = profit as u128;
    group.vault = group.c_tot + group.insurance + residual as u128;
    group
        .full_account_refresh(&mut account, &[1; V14_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    let vault_before = group.vault;
    let c_tot_before = group.c_tot;
    let pnl_before = account.pnl;
    let expected = (profit as u128).min(residual as u128);
    let result = group.convert_released_pnl_to_capital_not_atomic(&mut account);

    kani::cover!(expected == 0, "v14 zero conversion branch reachable");
    kani::cover!(expected > 0, "v14 positive conversion branch reachable");
    if expected == 0 {
        if profit == 0 {
            assert_eq!(result, Ok(0));
        } else {
            assert_eq!(result, Err(V14Error::LockActive));
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
        assert_eq!(account.pnl, 0);
        assert_eq!(group.pnl_pos_tot, 0);
        assert_eq!(group.pnl_pos_bound_tot, 0);
    }
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v14_target_effective_lag_blocks_pnl_conversion_before_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 10).unwrap();
    group
        .attach_leg(&mut account, 0, SideV14::Long, POS_SCALE as i128)
        .unwrap();
    account.pnl = 10;
    group.pnl_pos_tot = 10;
    group.pnl_pos_bound_tot = 10;
    group.pnl_matured_pos_tot = 10;
    group.vault = group.vault.checked_add(10).unwrap();
    group.assets[0].effective_price = 100;
    group.assets[0].raw_oracle_target_price = 100;
    group
        .full_account_refresh(&mut account, &[100; V14_MAX_PORTFOLIO_ASSETS_N])
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
        "v14 target/effective lag conversion lock reachable"
    );
    assert_eq!(result, Err(V14Error::LockActive));
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
fn proof_v14_loss_stale_blocks_nonflat_withdrawal() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 100).unwrap();
    group
        .attach_leg(&mut account, 0, SideV14::Long, 10)
        .unwrap();
    group.loss_stale_active = true;

    let result = group.withdraw_not_atomic(&mut account, 10, &[1; V14_MAX_PORTFOLIO_ASSETS_N]);

    assert_eq!(result, Err(V14Error::LockActive));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_resolved_positive_payout_snapshot_is_order_stable() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut first = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut second = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [4; 32], owner));
    group.vault = 100;
    first.pnl = 100;
    second.pnl = 100;
    group.pnl_pos_tot = 200;
    group.pnl_pos_bound_tot = 200;
    group.resolve_market_not_atomic(1).unwrap();

    let first_close = group.close_resolved_account_not_atomic(&mut first, 0);
    let second_close = group.close_resolved_account_not_atomic(&mut second, 0);

    assert_eq!(
        first_close,
        Ok(ResolvedCloseOutcomeV14::Closed { payout: 50 })
    );
    assert_eq!(
        second_close,
        Ok(ResolvedCloseOutcomeV14::Closed { payout: 50 })
    );
    assert_eq!(group.payout_snapshot, 100);
    assert_eq!(group.payout_snapshot_pnl_pos_tot, 200);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_resolved_payout_uses_positive_bound_denominator() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group.vault = 100;
    account.pnl = 100;
    group.pnl_pos_tot = 100;
    group.pnl_pos_bound_tot = 200;
    group.resolve_market_not_atomic(1).unwrap();

    let close = group.close_resolved_account_not_atomic(&mut account, 0);

    kani::cover!(
        group.payout_snapshot_pnl_pos_tot > group.pnl_pos_tot,
        "v14 resolved payout bound denominator remains conservative after close"
    );
    assert_eq!(close, Ok(ResolvedCloseOutcomeV14::Closed { payout: 50 }));
    assert_eq!(group.payout_snapshot, 100);
    assert_eq!(group.payout_snapshot_pnl_pos_tot, 200);
    assert_eq!(group.vault, 50);
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v14_resolved_close_partial_b_settlement_makes_progress_without_closing() {
    let larger_target: bool = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let mut cfg = V14Config::public_user_fund(1, 0, 1);
    cfg.public_b_chunk_atoms = 1;
    let mut group = MarketGroupV14::new(market, cfg).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));

    group.deposit_not_atomic(&mut account, 100).unwrap();
    group.attach_leg(&mut account, 0, SideV14::Long, 1).unwrap();
    group.assets[0].b_long_num = if larger_target { 3 } else { 2 };
    group.resolve_market_not_atomic(10).unwrap();

    let outcome = group.close_resolved_account_not_atomic(&mut account, 1);

    kani::cover!(!larger_target, "v14 resolved close partial B target two");
    kani::cover!(larger_target, "v14 resolved close partial B target three");
    assert_eq!(outcome, Ok(ResolvedCloseOutcomeV14::ProgressOnly));
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
fn proof_v14_resolved_flat_close_returns_exact_capital() {
    let amount: u16 = kani::any();
    kani::assume(amount > 0);
    kani::assume(amount <= 1_000);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group
        .deposit_not_atomic(&mut account, amount as u128)
        .unwrap();
    group.resolve_market_not_atomic(1).unwrap();

    let outcome = group.close_resolved_account_not_atomic(&mut account, 0);

    assert_eq!(
        outcome,
        Ok(ResolvedCloseOutcomeV14::Closed {
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
fn proof_v14_resolved_flat_close_syncs_fee_before_terminal_payout() {
    let fee_rate: u8 = kani::any();
    kani::assume(fee_rate > 0);
    kani::assume(fee_rate <= 5);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 100).unwrap();
    group.resolve_market_not_atomic(10).unwrap();

    let outcome = group
        .close_resolved_account_not_atomic(&mut account, fee_rate as u128)
        .unwrap();
    let expected_fee = fee_rate as u128 * 10;
    let expected_payout = 100 - expected_fee;

    kani::cover!(
        expected_fee > 0,
        "v14 resolved terminal close positive fee sync reachable"
    );
    assert_eq!(
        outcome,
        ResolvedCloseOutcomeV14::Closed {
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
fn proof_v14_resolved_profit_close_pays_snapshot_residual_and_clears_claim() {
    let profit: u8 = kani::any();
    kani::assume(profit > 0);
    kani::assume(profit <= 20);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 10).unwrap();
    account.pnl = profit as i128;
    group.pnl_pos_tot = profit as u128;
    group.pnl_pos_bound_tot = profit as u128;
    group.vault = group.c_tot + profit as u128;
    group.resolve_market_not_atomic(1).unwrap();

    let outcome = group.close_resolved_account_not_atomic(&mut account, 0);

    kani::cover!(profit > 1, "v14 resolved profit payout branch reachable");
    assert_eq!(
        outcome,
        Ok(ResolvedCloseOutcomeV14::Closed {
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
fn proof_v14_bankrupt_liquidation_consumes_insurance_before_social_loss() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut opposing = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [4; 32], owner));
    group.vault = 4;
    group.insurance = 4;
    account.pnl = -9;
    group.negative_pnl_account_count = 1;
    group.attach_leg(&mut account, 0, SideV14::Long, 1).unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV14::Short, -10)
        .unwrap();

    let out = group
        .liquidate_account_not_atomic(
            &mut account,
            LiquidationRequestV14 {
                asset_index: 0,
                close_q: 1,
                fee_bps: 0,
            },
            &[1; V14_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(out.insurance_used, 4);
    assert_eq!(out.residual_booked, 5);
    assert_eq!(out.explicit_loss, 0);
    assert_eq!(group.insurance, 0);
    assert_eq!(account.pnl, 0);
    assert_eq!(account.active_bitmap, 0);
}

#[kani::proof]
#[kani::unwind(55)]
#[kani::solver(cadical)]
fn proof_v14_domain_insurance_budget_caps_bankruptcy_spend() {
    let domain_budget: u8 = kani::any();
    kani::assume(domain_budget <= 4);
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut opposing = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [4; 32], owner));
    group.vault = 4;
    group.insurance = 4;
    group.insurance_domain_budget = [0; V14_DOMAIN_COUNT];
    group.insurance_domain_budget[1] = domain_budget as u128;
    account.pnl = -9;
    group.negative_pnl_account_count = 1;
    group.attach_leg(&mut account, 0, SideV14::Long, 1).unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV14::Short, -10)
        .unwrap();

    let out = group
        .liquidate_account_not_atomic(
            &mut account,
            LiquidationRequestV14 {
                asset_index: 0,
                close_q: 1,
                fee_bps: 0,
            },
            &[1; V14_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    kani::cover!(
        domain_budget == 0,
        "v14 domain insurance proof covers zero budget"
    );
    kani::cover!(
        domain_budget == 4,
        "v14 domain insurance proof covers full local budget"
    );
    assert_eq!(out.insurance_used, domain_budget as u128);
    assert_eq!(out.residual_booked, 9 - domain_budget as u128);
    assert_eq!(out.explicit_loss, 0);
    assert_eq!(group.insurance, 4 - domain_budget as u128);
    assert_eq!(group.insurance_domain_spent[1], domain_budget as u128);
    assert_eq!(group.insurance_domain_spent[0], 0);
    assert_eq!(account.pnl, 0);
    assert_eq!(account.active_bitmap, 0);
}

#[kani::proof]
#[kani::unwind(60)]
#[kani::solver(cadical)]
fn proof_v14_bankrupt_liquidation_cannot_free_exposure_before_residual_durable() {
    let larger_residual: bool = kani::any();
    let residual = if larger_residual { -3 } else { -2 };
    let (market, account_id, owner) = concrete_ids();
    let mut cfg = V14Config::public_user_fund(1, 0, 1);
    cfg.public_b_chunk_atoms = 1;
    let mut group = MarketGroupV14::new(market, cfg).unwrap();
    let mut bankrupt =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut opposing = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [4; 32], owner));

    group
        .attach_leg(&mut bankrupt, 0, SideV14::Long, 4)
        .unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV14::Short, -10)
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
        LiquidationRequestV14 {
            asset_index: 0,
            close_q: 4,
            fee_bps: 0,
        },
        &[1; V14_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(
        result == Err(V14Error::RecoveryRequired),
        "v14 partial residual recovery path reachable"
    );
    kani::cover!(
        !larger_residual,
        "v14 residual durability proof covers two atoms"
    );
    kani::cover!(
        larger_residual,
        "v14 residual durability proof covers three atoms"
    );
    assert_eq!(result, Err(V14Error::RecoveryRequired));
    assert_eq!(
        group.recovery_reason,
        Some(PermissionlessRecoveryReasonV14::ActiveBankruptCloseCannotProgress)
    );
    assert_eq!(bankrupt.active_bitmap, before_bitmap);
    assert_eq!(bankrupt.legs[0].basis_pos_q, before_basis);
    assert_eq!(bankrupt.pnl, before_pnl);
    assert_eq!(group.assets[0].b_short_num, before_b_short);
}

#[kani::proof]
#[kani::unwind(55)]
#[kani::solver(cadical)]
fn proof_v14_bankrupt_liquidation_excludes_fee_from_residual_and_spends_insurance_once() {
    let insurance_units: u8 = kani::any();
    kani::assume(insurance_units <= 2);
    let insurance = insurance_units as u128;
    let (market, account_id, owner) = concrete_ids();
    let mut cfg = V14Config::public_user_fund(1, 0, 10);
    cfg.max_price_move_bps_per_slot = 1;
    cfg.min_nonzero_mm_req = 12;
    cfg.min_nonzero_im_req = 13;
    cfg.liquidation_fee_bps = 0;
    cfg.liquidation_fee_cap = 1;
    cfg.min_liquidation_abs = 1;
    let mut group = MarketGroupV14::new(market, cfg).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut opposing = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [4; 32], owner));

    group.vault = insurance;
    group.insurance = insurance;
    account.pnl = -5;
    group.negative_pnl_account_count = 1;
    group.attach_leg(&mut account, 0, SideV14::Long, 1).unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV14::Short, -10)
        .unwrap();

    let out = group
        .liquidate_account_not_atomic(
            &mut account,
            LiquidationRequestV14 {
                asset_index: 0,
                close_q: 1,
                fee_bps: 0,
            },
            &[1; V14_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    kani::cover!(
        insurance == 0,
        "v14 bankrupt liquidation zero-insurance path reachable"
    );
    kani::cover!(
        insurance == 2,
        "v14 bankrupt liquidation partial-insurance path reachable"
    );
    assert_eq!(out.fee_charged, 0);
    assert_eq!(out.insurance_used, insurance);
    assert_eq!(group.insurance, 0);
    assert_eq!(out.residual_booked, 5 - insurance);
    assert_eq!(out.explicit_loss, 0);
    assert_eq!(account.pnl, 0);
    assert_eq!(account.active_bitmap, 0);
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v14_rebalance_reduce_position_preserves_senior_claims_and_reduces_risk() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group
        .attach_leg(&mut account, 0, SideV14::Long, POS_SCALE as i128)
        .unwrap();
    let senior_before = group.c_tot + group.insurance;

    let out = group
        .rebalance_reduce_position_not_atomic(
            &mut account,
            RebalanceRequestV14 {
                asset_index: 0,
                reduce_q: POS_SCALE / 2,
            },
            &[1_000_000; V14_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    kani::cover!(out.reduced_q == POS_SCALE / 2);
    assert_eq!(out.reduced_q, POS_SCALE / 2);
    assert!(account.legs[0].active);
    assert_eq!(account.legs[0].side, SideV14::Long);
    assert_eq!(account.legs[0].basis_pos_q.unsigned_abs(), POS_SCALE / 2);
    assert_eq!(group.c_tot + group.insurance, senior_before);
    assert!(account.health_cert.valid);
    assert!(account.health_cert.certified_worst_case_loss <= 500_000);

    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group
        .attach_leg(&mut account, 0, SideV14::Short, -(POS_SCALE as i128))
        .unwrap();
    let senior_before = group.c_tot + group.insurance;

    let out = group
        .rebalance_reduce_position_not_atomic(
            &mut account,
            RebalanceRequestV14 {
                asset_index: 0,
                reduce_q: POS_SCALE,
            },
            &[1_000_000; V14_MAX_PORTFOLIO_ASSETS_N],
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
fn proof_v14_b_residual_booking_makes_durable_progress_or_fails_closed() {
    let residual_units: u8 = kani::any();
    kani::assume(residual_units <= 4);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group
        .attach_leg(&mut account, 0, SideV14::Short, -1)
        .unwrap();

    let before_b = group.assets[0].b_short_num;
    let residual = residual_units as u128;
    let result =
        group.book_bankruptcy_residual_chunk_for_account(&mut account, 0, SideV14::Long, residual);
    if residual == 0 {
        assert_eq!(result.unwrap().remaining_after, 0);
        assert_eq!(group.assets[0].b_short_num, before_b);
    } else {
        let out = result.unwrap();
        kani::cover!(out.booked_loss > 0, "v14 residual B booking reachable");
        assert!(out.booked_loss > 0);
        assert_eq!(out.explicit_loss, 0);
        assert_eq!(
            account.close_progress.b_loss_booked + account.close_progress.explicit_loss_assigned,
            out.booked_loss + out.explicit_loss
        );
        assert!(account.close_progress.finalized);
        assert!(group.bankruptcy_hlock_active);
    }
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v14_zero_weight_domain_residual_routes_to_recovery_without_mutation() {
    let bankrupt_long: bool = kani::any();
    let (market, _, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let bankrupt_side = if bankrupt_long {
        SideV14::Long
    } else {
        SideV14::Short
    };
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [2u8; 32], owner));

    let before_long = group.assets[0].explicit_unallocated_loss_long;
    let before_short = group.assets[0].explicit_unallocated_loss_short;
    let result =
        group.book_bankruptcy_residual_chunk_for_account(&mut account, 0, bankrupt_side, 1);

    kani::cover!(
        bankrupt_long,
        "v14 zero-weight short-domain recovery reachable"
    );
    kani::cover!(
        !bankrupt_long,
        "v14 zero-weight long-domain recovery reachable"
    );
    assert_eq!(result, Err(V14Error::RecoveryRequired));
    assert_eq!(
        group.recovery_reason,
        Some(PermissionlessRecoveryReasonV14::ActiveBankruptCloseCannotProgress)
    );
    assert_eq!(group.assets[0].explicit_unallocated_loss_long, before_long);
    assert_eq!(
        group.assets[0].explicit_unallocated_loss_short,
        before_short
    );
    assert!(!account.close_progress.active);
    let blocked_domain = if bankrupt_long {
        SideV14::Short
    } else {
        SideV14::Long
    };
    assert!(matches!(
        group.pending_domain_loss_barrier_count(0, blocked_domain),
        Ok(0)
    ));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_account_b_booking_advances_close_progress_or_fails_closed() {
    let residual_units: u8 = kani::any();
    kani::assume(residual_units > 0 && residual_units <= 4);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut opp = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group.attach_leg(&mut opp, 0, SideV14::Short, -1).unwrap();
    let mut bankrupt =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [7u8; 32], [8u8; 32]));

    let before_b = group.assets[0].b_short_num;
    let before_explicit = group.assets[0].explicit_unallocated_loss_short;
    let result = group.book_bankruptcy_residual_chunk_for_account(
        &mut bankrupt,
        0,
        SideV14::Long,
        residual_units as u128,
    );

    if let Ok(out) = result {
        kani::cover!(
            out.booked_loss > 0,
            "v14 account B booking ledger path reachable"
        );
        assert!(bankrupt.close_progress.active);
        assert!(bankrupt.close_progress.finalized);
        assert_eq!(bankrupt.close_progress.residual_remaining, 0);
        assert_eq!(bankrupt.close_progress.b_loss_booked, out.booked_loss);
        assert_eq!(
            bankrupt.close_progress.explicit_loss_assigned,
            out.explicit_loss
        );
        assert!(group.assets[0].b_short_num >= before_b);
    } else {
        assert_eq!(group.assets[0].b_short_num, before_b);
        assert_eq!(
            group.assets[0].explicit_unallocated_loss_short,
            before_explicit
        );
    }
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v14_pending_domain_barrier_blocks_participants_until_residual_finalized() {
    let (market, account_id, owner) = concrete_ids();
    let mut cfg = V14Config::public_user_fund(1, 0, 1);
    cfg.public_b_chunk_atoms = 1;
    let mut group = MarketGroupV14::new(market, cfg).unwrap();
    let mut bankrupt =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut participant =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [4; 32], owner));
    let mut joiner = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [5; 32], owner));

    group
        .attach_leg(&mut participant, 0, SideV14::Short, -10)
        .unwrap();
    let first = group
        .book_bankruptcy_residual_chunk_for_account(&mut bankrupt, 0, SideV14::Long, 2)
        .unwrap();
    kani::cover!(
        first.booked_loss == 1,
        "v14 pending domain barrier partial B booking reachable"
    );
    assert_eq!(first.booked_loss, 1);
    assert!(matches!(
        group.pending_domain_loss_barrier_count(0, SideV14::Short),
        Ok(1)
    ));
    assert!(matches!(
        group.clear_leg(&mut participant, 0),
        Err(V14Error::LockActive)
    ));
    assert!(matches!(
        group.attach_leg(&mut joiner, 0, SideV14::Short, -1),
        Err(V14Error::LockActive)
    ));
    assert!(matches!(
        group.h_lock_lane(Some(&participant), false),
        Ok(HLockLaneV14::HMax)
    ));

    let second = group
        .book_bankruptcy_residual_chunk_for_account(&mut bankrupt, 0, SideV14::Long, 2)
        .unwrap();
    assert_eq!(second.booked_loss, 1);
    assert!(bankrupt.close_progress.finalized);
    assert!(matches!(
        group.pending_domain_loss_barrier_count(0, SideV14::Short),
        Ok(0)
    ));
    assert!(matches!(group.clear_leg(&mut participant, 0), Ok(())));
}

#[kani::proof]
#[kani::unwind(60)]
#[kani::solver(cadical)]
fn proof_v14_new_close_cannot_overwrite_active_finalized_close_ledger() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(2, 0, 1)).unwrap();
    let mut bankrupt =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut opposing = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [4; 32], owner));
    group
        .attach_leg(&mut bankrupt, 1, SideV14::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut opposing, 1, SideV14::Short, -(POS_SCALE as i128))
        .unwrap();
    bankrupt.close_progress = CloseProgressLedgerV14 {
        active: true,
        finalized: true,
        close_id: 7,
        asset_index: 0,
        domain_side: SideV14::Short,
        gross_loss_at_close_start: 2,
        b_loss_booked: 2,
        residual_remaining: 0,
        drift_reference_slot: group.current_slot,
        max_close_slot: group.current_slot + 1,
        ..CloseProgressLedgerV14::EMPTY
    };
    group.assets[1].k_long = -(100 * ADL_ONE as i128);
    let before_ledger = bankrupt.close_progress;
    let before_b_short = group.assets[1].b_short_num;

    let result = group.liquidate_account_not_atomic(
        &mut bankrupt,
        LiquidationRequestV14 {
            asset_index: 1,
            close_q: POS_SCALE,
            fee_bps: 0,
        },
        &[1; V14_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(
        result == Err(V14Error::LockActive),
        "v14 active finalized close ledger blocks new close id"
    );
    assert_eq!(result, Err(V14Error::LockActive));
    assert_eq!(bankrupt.close_progress, before_ledger);
    assert_eq!(group.assets[1].b_short_num, before_b_short);
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v14_account_shape_rejects_malformed_quantity_adl_close_progress() {
    let premature_adl: bool = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));

    if premature_adl {
        account.close_progress = CloseProgressLedgerV14 {
            active: true,
            finalized: false,
            close_id: 1,
            asset_index: 0,
            domain_side: SideV14::Short,
            gross_loss_at_close_start: 2,
            b_loss_booked: 1,
            residual_remaining: 1,
            quantity_adl_applied_q: 1,
            ..CloseProgressLedgerV14::EMPTY
        };
    } else {
        let mut group_for_leg =
            MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
        group_for_leg
            .attach_leg(&mut account, 0, SideV14::Long, 4)
            .unwrap();
        account.close_progress = CloseProgressLedgerV14 {
            active: true,
            finalized: true,
            close_id: 1,
            asset_index: 0,
            domain_side: SideV14::Short,
            gross_loss_at_close_start: 1,
            explicit_loss_assigned: 1,
            residual_remaining: 0,
            quantity_adl_applied_q: 4,
            ..CloseProgressLedgerV14::EMPTY
        };
    }

    let result = group.validate_account_shape(&account);

    kani::cover!(premature_adl, "v14 premature quantity ADL shape reachable");
    kani::cover!(
        !premature_adl,
        "v14 quantity ADL with open closing leg shape reachable"
    );
    assert_eq!(result, Err(V14Error::InvalidLeg));
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v14_account_shape_rejects_close_progress_domain_mismatch_for_open_leg() {
    let closing_long: bool = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let side = if closing_long {
        SideV14::Long
    } else {
        SideV14::Short
    };
    let signed_basis = if closing_long { 4 } else { -4 };
    group
        .attach_leg(&mut account, 0, side, signed_basis)
        .unwrap();
    account.close_progress = CloseProgressLedgerV14 {
        active: true,
        finalized: false,
        close_id: 1,
        asset_index: 0,
        domain_side: side,
        gross_loss_at_close_start: 2,
        b_loss_booked: 1,
        residual_remaining: 1,
        ..CloseProgressLedgerV14::EMPTY
    };

    let result = group.validate_account_shape(&account);

    kani::cover!(closing_long, "v14 long close domain mismatch reachable");
    kani::cover!(!closing_long, "v14 short close domain mismatch reachable");
    assert_eq!(result, Err(V14Error::InvalidLeg));
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v14_expired_close_progress_routes_recovery_before_durable_mutation() {
    let close_b_residual: bool = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    group.current_slot = 2;
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut opposing =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [9; 32], [8; 32]));
    group.attach_leg(&mut account, 0, SideV14::Long, 4).unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV14::Short, -4)
        .unwrap();
    account.close_progress = CloseProgressLedgerV14 {
        active: true,
        finalized: !close_b_residual,
        close_id: 1,
        asset_index: 0,
        domain_side: SideV14::Short,
        gross_loss_at_close_start: 2,
        drift_reference_slot: 0,
        max_close_slot: 1,
        explicit_loss_assigned: if close_b_residual { 0 } else { 2 },
        residual_remaining: if close_b_residual { 2 } else { 0 },
        ..CloseProgressLedgerV14::EMPTY
    };
    group.assets[0].a_short = ADL_ONE;
    let before_b = group.assets[0].b_short_num;
    let before_a = group.assets[0].a_short;
    let before_long_oi = group.assets[0].oi_eff_long_q;
    let before_short_oi = group.assets[0].oi_eff_short_q;

    let result = if close_b_residual {
        group
            .book_bankruptcy_residual_chunk_for_account(&mut account, 0, SideV14::Long, 2)
            .map(|_| ())
    } else {
        group
            .apply_quantity_adl_after_residual_for_account_not_atomic(
                &mut account,
                0,
                SideV14::Long,
                4,
            )
            .map(|_| ())
    };

    kani::cover!(
        close_b_residual,
        "v14 expired B continuation recovery path reachable"
    );
    kani::cover!(
        !close_b_residual,
        "v14 expired quantity ADL continuation recovery path reachable"
    );
    assert_eq!(result, Err(V14Error::RecoveryRequired));
    assert_eq!(
        group.recovery_reason,
        Some(PermissionlessRecoveryReasonV14::ActiveBankruptCloseCannotProgress)
    );
    assert_eq!(group.assets[0].b_short_num, before_b);
    assert_eq!(group.assets[0].a_short, before_a);
    assert_eq!(group.assets[0].oi_eff_long_q, before_long_oi);
    assert_eq!(group.assets[0].oi_eff_short_q, before_short_oi);
    assert_eq!(account.close_progress.b_loss_booked, 0);
    assert_eq!(account.close_progress.quantity_adl_applied_q, 0);
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v14_stale_open_close_snapshot_routes_recovery_before_durable_mutation() {
    let close_b_residual: bool = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    group.current_slot = 1;
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut opposing =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [10; 32], owner));
    group.attach_leg(&mut account, 0, SideV14::Long, 4).unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV14::Short, -4)
        .unwrap();
    account.close_progress = CloseProgressLedgerV14 {
        active: true,
        finalized: !close_b_residual,
        close_id: 1,
        asset_index: 0,
        domain_side: SideV14::Short,
        gross_loss_at_close_start: 2,
        drift_reference_slot: 0,
        max_close_slot: 10,
        explicit_loss_assigned: if close_b_residual { 0 } else { 2 },
        residual_remaining: if close_b_residual { 2 } else { 0 },
        ..CloseProgressLedgerV14::EMPTY
    };
    let before_ledger = account.close_progress;
    let before_b = group.assets[0].b_short_num;
    let before_a = group.assets[0].a_short;
    let before_long_oi = group.assets[0].oi_eff_long_q;
    let before_short_oi = group.assets[0].oi_eff_short_q;

    let result = if close_b_residual {
        group
            .book_bankruptcy_residual_chunk_for_account(&mut account, 0, SideV14::Long, 2)
            .map(|_| ())
    } else {
        group
            .apply_quantity_adl_after_residual_for_account_not_atomic(
                &mut account,
                0,
                SideV14::Long,
                4,
            )
            .map(|_| ())
    };

    kani::cover!(
        close_b_residual,
        "v14 stale open close B continuation recovery path reachable"
    );
    kani::cover!(
        !close_b_residual,
        "v14 stale open close quantity ADL recovery path reachable"
    );
    assert_eq!(result, Err(V14Error::RecoveryRequired));
    assert_eq!(
        group.recovery_reason,
        Some(PermissionlessRecoveryReasonV14::ActiveBankruptCloseCannotProgress)
    );
    assert_eq!(account.close_progress, before_ledger);
    assert_eq!(group.assets[0].b_short_num, before_b);
    assert_eq!(group.assets[0].a_short, before_a);
    assert_eq!(group.assets[0].oi_eff_long_q, before_long_oi);
    assert_eq!(group.assets[0].oi_eff_short_q, before_short_oi);
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v14_invalid_trade_request_rejects_before_any_mutation() {
    assert_invalid_trade_reverts(TradeRequestV14 {
        asset_index: 1,
        size_q: POS_SCALE,
        exec_price: 100,
        fee_bps: 0,
    });
    assert_invalid_trade_reverts(TradeRequestV14 {
        asset_index: 0,
        size_q: 0,
        exec_price: 100,
        fee_bps: 0,
    });
    assert_invalid_trade_reverts(TradeRequestV14 {
        asset_index: 0,
        size_q: POS_SCALE,
        exec_price: 0,
        fee_bps: 0,
    });
    assert_invalid_trade_reverts(TradeRequestV14 {
        asset_index: 0,
        size_q: POS_SCALE,
        exec_price: 100,
        fee_bps: 11,
    });
}

fn assert_invalid_trade_reverts(request: TradeRequestV14) {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    group.config.max_trading_fee_bps = 10;
    let mut long = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut short = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut long, 1_000).unwrap();
    group.deposit_not_atomic(&mut short, 1_000).unwrap();
    let before_group = group;
    let before_long = long;
    let before_short = short;

    let result = group.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        request,
        &[100; V14_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(result, Err(V14Error::InvalidConfig));
    assert_eq!(group, before_group);
    assert_eq!(long, before_long);
    assert_eq!(short, before_short);
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v14_price_accrual_refresh_matches_eager_mark_pnl() {
    assert_price_accrual_refresh_matches_eager_mark_pnl(101, 1, -1);
    assert_price_accrual_refresh_matches_eager_mark_pnl(99, -1, 1);
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v14_same_epoch_full_refresh_is_idempotent_after_price_up_settlement() {
    assert_same_epoch_refresh_idempotent_after_kf_settlement(101, 1);
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v14_same_epoch_full_refresh_is_idempotent_after_price_down_settlement() {
    assert_same_epoch_refresh_idempotent_after_kf_settlement(99, -1);
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v14_sequential_kf_refresh_is_additive_not_compounding() {
    let (market, account_id, owner) = concrete_ids();
    let mut sequential = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    sequential.assets[0].effective_price = 100;
    sequential.assets[0].fund_px_last = 100;
    sequential.assets[0].raw_oracle_target_price = 100;
    let mut seq_account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    sequential
        .attach_leg(&mut seq_account, 0, SideV14::Long, POS_SCALE as i128)
        .unwrap();

    sequential
        .accrue_asset_to_not_atomic(0, 1, 101, 0, true)
        .unwrap();
    sequential
        .full_account_refresh(&mut seq_account, &[101; V14_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    kani::cover!(
        seq_account.pnl == 1,
        "v14 first sequential K/F refresh settles nonzero pnl"
    );

    sequential
        .accrue_asset_to_not_atomic(0, 2, 102, 0, true)
        .unwrap();
    sequential
        .full_account_refresh(&mut seq_account, &[102; V14_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    let mut direct = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    direct.assets[0].effective_price = 100;
    direct.assets[0].fund_px_last = 100;
    direct.assets[0].raw_oracle_target_price = 100;
    let mut direct_account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    direct
        .attach_leg(&mut direct_account, 0, SideV14::Long, POS_SCALE as i128)
        .unwrap();

    direct
        .accrue_asset_to_not_atomic(0, 1, 102, 0, true)
        .unwrap();
    direct
        .full_account_refresh(&mut direct_account, &[102; V14_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(seq_account.pnl, 2);
    assert_eq!(direct_account.pnl, 2);
    assert_eq!(seq_account.pnl, direct_account.pnl);
    assert_eq!(sequential.pnl_pos_tot, direct.pnl_pos_tot);
}

fn assert_same_epoch_refresh_idempotent_after_kf_settlement(new_price: u64, expected_pnl: i128) {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    group.assets[0].effective_price = 100;
    group.assets[0].fund_px_last = 100;
    group.assets[0].raw_oracle_target_price = 100;
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group
        .attach_leg(&mut account, 0, SideV14::Long, POS_SCALE as i128)
        .unwrap();

    group
        .accrue_asset_to_not_atomic(0, 1, new_price, 0, true)
        .unwrap();
    group
        .full_account_refresh(&mut account, &[new_price; V14_MAX_PORTFOLIO_ASSETS_N])
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
        "v14 idempotent refresh exercises nonzero settled K/F pnl"
    );
    group
        .full_account_refresh(&mut account, &[new_price; V14_MAX_PORTFOLIO_ASSETS_N])
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
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    group.assets[0].effective_price = 100;
    group.assets[0].fund_px_last = 100;
    group.assets[0].raw_oracle_target_price = 100;
    let mut long = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut short = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [4; 32], owner));
    group
        .attach_leg(&mut long, 0, SideV14::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut short, 0, SideV14::Short, -(POS_SCALE as i128))
        .unwrap();
    let out = group
        .accrue_asset_to_not_atomic(0, 1, new_price, 0, true)
        .unwrap();
    group
        .full_account_refresh(&mut long, &[new_price; V14_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    group
        .full_account_refresh(&mut short, &[new_price; V14_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert!(out.price_move_active);
    assert_eq!(long.pnl, expected_long_pnl);
    assert_eq!(short.pnl, expected_short_pnl);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v14_funding_accrual_refresh_matches_sign_and_floor() {
    assert_funding_accrual_refresh_matches_sign_and_floor(10_000, -1, 1);
    assert_funding_accrual_refresh_matches_sign_and_floor(-10_000, 1, -1);
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v14_funding_accrual_requires_bilateral_exposure() {
    let (market, account_id, owner) = concrete_ids();
    let mut long_only = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    long_only.config.max_price_move_bps_per_slot = 9_999;
    long_only.config.max_abs_funding_e9_per_slot = 1;
    long_only.assets[0].effective_price = 1_000_000_000;
    long_only.assets[0].fund_px_last = 1_000_000_000;
    long_only.assets[0].raw_oracle_target_price = 1_000_000_000;
    let mut long = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    long_only
        .attach_leg(&mut long, 0, SideV14::Long, POS_SCALE as i128)
        .unwrap();
    let long_before = long_only.assets[0];

    let out = long_only
        .accrue_asset_to_not_atomic(0, 1, 1_000_000_000, 1, false)
        .unwrap();
    kani::cover!(
        long_only.assets[0].oi_eff_long_q != 0 && long_only.assets[0].oi_eff_short_q == 0,
        "v14 funding no-op covers long-only exposure"
    );

    assert!(!out.funding_active);
    assert_eq!(long_only.assets[0].f_long_num, long_before.f_long_num);
    assert_eq!(long_only.assets[0].f_short_num, long_before.f_short_num);
    assert_eq!(long_only.funding_epoch, 0);

    let mut short_only = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    short_only.config.max_price_move_bps_per_slot = 9_999;
    short_only.config.max_abs_funding_e9_per_slot = 1;
    short_only.assets[0].effective_price = 1_000_000_000;
    short_only.assets[0].fund_px_last = 1_000_000_000;
    short_only.assets[0].raw_oracle_target_price = 1_000_000_000;
    let mut short = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [4; 32], owner));
    short_only
        .attach_leg(&mut short, 0, SideV14::Short, -(POS_SCALE as i128))
        .unwrap();
    let short_before = short_only.assets[0];

    let out = short_only
        .accrue_asset_to_not_atomic(0, 1, 1_000_000_000, 1, false)
        .unwrap();
    kani::cover!(
        short_only.assets[0].oi_eff_short_q != 0 && short_only.assets[0].oi_eff_long_q == 0,
        "v14 funding no-op covers short-only exposure"
    );

    assert!(!out.funding_active);
    assert_eq!(short_only.assets[0].f_long_num, short_before.f_long_num);
    assert_eq!(short_only.assets[0].f_short_num, short_before.f_short_num);
    assert_eq!(short_only.funding_epoch, 0);
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v14_no_oi_funding_rate_does_not_mutate_k_or_f() {
    let positive_rate: bool = kani::any();
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
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
        "v14 no-OI funding proof covers positive rate"
    );
    kani::cover!(
        !positive_rate,
        "v14 no-OI funding proof covers negative rate"
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
fn proof_v14_permissionless_crank_accepts_configured_funding_rate_boundaries() {
    let positive_rate: bool = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let mut cfg = V14Config::public_user_fund(1, 0, 1);
    cfg.max_price_move_bps_per_slot = 9_999;
    cfg.max_abs_funding_e9_per_slot = 1;
    let mut group = MarketGroupV14::new(market, cfg).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let supplied_rate = if positive_rate { 1 } else { -1 };

    let out = group
        .permissionless_crank_not_atomic(
            &mut account,
            PermissionlessCrankRequestV14 {
                now_slot: 1,
                asset_index: 0,
                effective_price: 1,
                funding_rate_e9: supplied_rate,
                action: PermissionlessCrankActionV14::Refresh,
            },
            &[1; V14_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    kani::cover!(
        positive_rate && supplied_rate == group.config.max_abs_funding_e9_per_slot as i128,
        "v14 permissionless crank accepts positive funding boundary"
    );
    kani::cover!(
        !positive_rate && supplied_rate == -(group.config.max_abs_funding_e9_per_slot as i128),
        "v14 permissionless crank accepts negative funding boundary"
    );
    assert_eq!(out, PermissionlessProgressOutcomeV14::AccountCurrent);
    assert_eq!(group.current_slot, 1);
    assert_eq!(group.slot_last, 1);
    assert_eq!(group.funding_epoch, 0);
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v14_per_asset_slot_last_prevents_cross_asset_accrual_aliasing() {
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(2, 0, 1)).unwrap();
    let mut i = 0;
    while i < 2 {
        group.assets[i].oi_eff_long_q = 1;
        group.assets[i].oi_eff_short_q = 1;
        group.assets[i].effective_price = 100;
        group.assets[i].fund_px_last = 100;
        group.assets[i].raw_oracle_target_price = 100;
        i += 1;
    }

    let first = group.accrue_asset_to_not_atomic(0, 1, 101, 0, true);
    let asset1_slot_before = group.assets[1].slot_last;
    let second = group.accrue_asset_to_not_atomic(1, 1, 101, 0, true);

    kani::cover!(
        first.is_ok() && second.is_ok(),
        "v14 same-slot cross-asset accrual covers both assets"
    );
    assert!(first.is_ok());
    assert!(second.is_ok());
    assert_eq!(group.assets[0].slot_last, 1);
    assert_eq!(asset1_slot_before, 0);
    assert_eq!(group.assets[1].slot_last, 1);
    assert_ne!(group.assets[0].k_long, 0);
    assert_ne!(group.assets[1].k_long, 0);
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v14_funding_accrual_uses_only_bounded_segment_dt() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    group.config.max_price_move_bps_per_slot = 4_999;
    group.config.max_abs_funding_e9_per_slot = 1;
    group.config.max_accrual_dt_slots = 2;
    group.config.min_funding_lifetime_slots = 2;
    group.assets[0].effective_price = 1_000_000_000;
    group.assets[0].fund_px_last = 1_000_000_000;
    group.assets[0].raw_oracle_target_price = 1_000_000_000;
    let mut long = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut short = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [4; 32], owner));
    group
        .attach_leg(&mut long, 0, SideV14::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut short, 0, SideV14::Short, -(POS_SCALE as i128))
        .unwrap();

    let out = group
        .accrue_asset_to_not_atomic(0, 10, 1_000_000_000, 1, true)
        .unwrap();
    kani::cover!(
        out.funding_active && out.dt == 2 && group.current_slot == 10,
        "v14 funding stale catchup covers bounded segment dt"
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
fn proof_v14_combined_price_and_funding_accrual_keeps_k_and_f_separate() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    group.config.max_price_move_bps_per_slot = 9_999;
    group.config.max_abs_funding_e9_per_slot = 1;
    group.assets[0].effective_price = 999_999_999;
    group.assets[0].fund_px_last = 999_999_999;
    group.assets[0].raw_oracle_target_price = 999_999_999;
    let mut long = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut short = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [4; 32], owner));
    group
        .attach_leg(&mut long, 0, SideV14::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut short, 0, SideV14::Short, -(POS_SCALE as i128))
        .unwrap();

    let out = group
        .accrue_asset_to_not_atomic(0, 1, 1_000_000_000, 1, true)
        .unwrap();
    kani::cover!(
        out.price_move_active && out.funding_active,
        "v14 combined mark and funding accrual reachable"
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
fn proof_v14_zero_funding_rate_advances_time_without_f_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    group.assets[0].effective_price = 100;
    group.assets[0].fund_px_last = 100;
    group.assets[0].raw_oracle_target_price = 100;
    let mut long = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut short = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [4; 32], owner));
    group
        .attach_leg(&mut long, 0, SideV14::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut short, 0, SideV14::Short, -(POS_SCALE as i128))
        .unwrap();
    let before = group.assets[0];

    let out = group
        .accrue_asset_to_not_atomic(0, 1, 100, 0, true)
        .unwrap();
    kani::cover!(
        group.assets[0].oi_eff_long_q != 0 && group.assets[0].oi_eff_short_q != 0,
        "v14 zero-rate funding proof covers bilateral exposure"
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
    let mut cfg = V14Config::public_user_fund(1, 0, 1);
    cfg.max_abs_funding_e9_per_slot = 10_000;
    let mut group = MarketGroupV14::new(market, cfg).unwrap();
    group.assets[0].effective_price = 100_000;
    group.assets[0].fund_px_last = 100_000;
    group.assets[0].raw_oracle_target_price = 100_000;
    let mut long = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut short = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [4; 32], owner));
    group
        .attach_leg(&mut long, 0, SideV14::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut short, 0, SideV14::Short, -(POS_SCALE as i128))
        .unwrap();
    let out = group
        .accrue_asset_to_not_atomic(0, 1, 100_000, funding_rate_e9, true)
        .unwrap();
    group
        .full_account_refresh(&mut long, &[100_000; V14_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    group
        .full_account_refresh(&mut short, &[100_000; V14_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert!(out.funding_active);
    assert_eq!(long.pnl, expected_long_pnl);
    assert_eq!(short.pnl, expected_short_pnl);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_same_slot_exposed_price_move_rejects_before_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group
        .attach_leg(&mut account, 0, SideV14::Long, POS_SCALE as i128)
        .unwrap();
    let before = group;

    let result = group.accrue_asset_to_not_atomic(0, 0, 2, 0, true);

    assert_eq!(result, Err(V14Error::NonProgress));
    assert_eq!(group, before);
}

#[kani::proof]
#[kani::unwind(60)]
#[kani::solver(cadical)]
fn proof_v14_partial_liquidation_can_reduce_risk_without_forcing_full_close() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 10).unwrap();
    group
        .attach_leg(&mut account, 0, SideV14::Long, POS_SCALE as i128)
        .unwrap();

    let out = group
        .liquidate_account_not_atomic(
            &mut account,
            LiquidationRequestV14 {
                asset_index: 0,
                close_q: POS_SCALE / 2,
                fee_bps: 0,
            },
            &[100; V14_MAX_PORTFOLIO_ASSETS_N],
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
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v14_partial_liquidation_cannot_socialize_residual_while_open_risk_remains() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut bankrupt =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    let mut opposing = PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, [4; 32], owner));

    group
        .attach_leg(&mut bankrupt, 0, SideV14::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV14::Short, -(POS_SCALE as i128))
        .unwrap();
    group.assets[0].k_long = -(100 * ADL_ONE as i128);
    let before_b_short = group.assets[0].b_short_num;
    let before_basis = bankrupt.legs[0].basis_pos_q;
    let before_bitmap = bankrupt.active_bitmap;
    let before_b_loss_booked = bankrupt.close_progress.b_loss_booked;

    let result = group.liquidate_account_not_atomic(
        &mut bankrupt,
        LiquidationRequestV14 {
            asset_index: 0,
            close_q: POS_SCALE / 2,
            fee_bps: 0,
        },
        &[1; V14_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(
        result == Err(V14Error::RecoveryRequired),
        "v14 partial liquidation residual routes to recovery before B booking"
    );
    assert_eq!(result, Err(V14Error::RecoveryRequired));
    assert_eq!(
        group.recovery_reason,
        Some(PermissionlessRecoveryReasonV14::ActiveBankruptCloseCannotProgress)
    );
    assert_eq!(group.assets[0].b_short_num, before_b_short);
    assert_eq!(bankrupt.close_progress.b_loss_booked, before_b_loss_booked);
    assert_eq!(bankrupt.legs[0].basis_pos_q, before_basis);
    assert_eq!(bankrupt.active_bitmap, before_bitmap);
}

#[kani::proof]
#[kani::unwind(45)]
#[kani::solver(cadical)]
fn proof_v14_liquidation_rejects_zero_close_before_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group
        .attach_leg(&mut account, 0, SideV14::Long, POS_SCALE as i128)
        .unwrap();
    let before_group = group;
    let before_account = account;

    let result = group.liquidate_account_not_atomic(
        &mut account,
        LiquidationRequestV14 {
            asset_index: 0,
            close_q: 0,
            fee_bps: 0,
        },
        &[100; V14_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(result, Err(V14Error::InvalidConfig));
    assert_eq!(group, before_group);
    assert_eq!(account, before_account);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v14_liquidation_fee_floor_shortfall_charges_available_capital_only() {
    let capital: u8 = kani::any();
    kani::assume(capital > 0);
    kani::assume(capital <= 20);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group
        .deposit_not_atomic(&mut account, capital as u128)
        .unwrap();

    let charged = group
        .charge_account_fee_not_atomic(&mut account, 40)
        .unwrap();

    kani::cover!(
        charged < 40,
        "v14 liquidation-fee floor shortfall fee path reachable"
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
fn proof_v14_resolved_active_position_close_returns_progress_without_payout() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV14::new(market, V14Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV14::empty(ProvenanceHeaderV14::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 7).unwrap();
    group.attach_leg(&mut account, 0, SideV14::Long, 1).unwrap();
    group.resolve_market_not_atomic(1).unwrap();
    let before_vault = group.vault;
    let before_c_tot = group.c_tot;

    let outcome = group.close_resolved_account_not_atomic(&mut account, 0);

    assert_eq!(outcome, Ok(ResolvedCloseOutcomeV14::ProgressOnly));
    assert_ne!(account.active_bitmap, 0);
    assert_eq!(account.capital, 7);
    assert_eq!(group.vault, before_vault);
    assert_eq!(group.c_tot, before_c_tot);
}
