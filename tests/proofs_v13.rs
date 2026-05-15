#![cfg(kani)]

use percolator::v13::{
    account_equity, HLockLaneV13, LiquidationRequestV13, MarketGroupV13,
    PermissionlessCrankActionV13, PermissionlessCrankRequestV13, PermissionlessProgressOutcomeV13,
    PermissionlessRecoveryReasonV13, PortfolioAccountV13, ProvenanceHeaderV13, RebalanceRequestV13,
    ResolvedCloseOutcomeV13, SideV13, TradeRequestV13, V13Config, V13Error,
    V13_MAX_PORTFOLIO_ASSETS_N,
};
use percolator::{MAX_POSITION_ABS_Q, POS_SCALE, SOCIAL_LOSS_DEN};

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
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_public_config_rejects_invalid_user_fund_shapes() {
    let case: u8 = kani::any();
    kani::assume(case < 6);
    let (market, _, _) = symbolic_ids();
    let mut cfg = V13Config::public_user_fund(1, 0, 1);
    match case {
        0 => cfg.max_portfolio_assets = 0,
        1 => cfg.h_max = 0,
        2 => cfg.h_min = 2,
        3 => cfg.min_nonzero_mm_req = cfg.min_nonzero_im_req,
        4 => cfg.permissionless_recovery_enabled = false,
        _ => cfg.public_b_chunk_atoms = 0,
    }

    kani::cover!(case == 0, "v13 zero portfolio width rejected");
    kani::cover!(case == 1, "v13 zero hmax rejected");
    kani::cover!(case == 2, "v13 hmin above hmax rejected");
    kani::cover!(case == 3, "v13 invalid margin floor ordering rejected");
    kani::cover!(case == 4, "v13 disabled recovery rejected");
    kani::cover!(case == 5, "v13 zero B chunk budget rejected");
    assert_eq!(
        MarketGroupV13::new(market, cfg),
        Err(V13Error::InvalidConfig)
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
    kani::assume(dirty_case < 5);
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
        _ => dirty.stale_state = true,
    }
    kani::cover!(dirty_case == 0, "v13 close rejects capital");
    kani::cover!(dirty_case == 1, "v13 close rejects pnl");
    kani::cover!(dirty_case == 2, "v13 close rejects reserved pnl");
    kani::cover!(dirty_case == 3, "v13 close rejects fee debt");
    kani::cover!(dirty_case == 4, "v13 close rejects stale account");
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
