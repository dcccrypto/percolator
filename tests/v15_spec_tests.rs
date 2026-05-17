use percolator::v15::{
    account_equity, risk_notional_ceil, AssetStateV15Account, CloseProgressLedgerV15, HLockLaneV15,
    HealthCertV15Account, LiquidationRequestV15, MarketGroupV15, MarketGroupV15Account,
    MarketModeV15, PermissionlessCrankActionV15, PermissionlessCrankRequestV15,
    PermissionlessProgressOutcomeV15, PermissionlessRecoveryReasonV15, PortfolioAccountV15,
    PortfolioAccountV15Account, PortfolioLegV15, PortfolioLegV15Account, ProvenanceHeaderV15,
    ProvenanceHeaderV15Account, RebalanceRequestV15, ResolvedCloseOutcomeV15,
    ResolvedPayoutLedgerV15, SideModeV15, SideV15, TradeRequestV15, V15Config, V15ConfigAccount,
    V15Error, V15OptionalRecoveryReasonAccount, V15PodI128, V15PodU128, V15PodU16, V15PodU32,
    V15PodU64, V15_DOMAIN_COUNT, V15_MAX_PORTFOLIO_ASSETS_N,
};
use percolator::{
    ADL_ONE, BOUND_SCALE, MAX_ACCOUNT_NOTIONAL, MAX_ORACLE_PRICE, MAX_PROTOCOL_FEE_ABS, POS_SCALE,
    SOCIAL_LOSS_DEN,
};

fn ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    ([1; 32], [2; 32], [3; 32])
}

fn group() -> MarketGroupV15 {
    let (market, _, _) = ids();
    MarketGroupV15::new(market, V15Config::public_user_fund(4, 0, 10)).unwrap()
}

fn set_junior_bound(group: &mut MarketGroupV15, amount: u128) {
    group.pnl_pos_bound_tot = amount;
    group.pnl_pos_bound_tot_num = amount.checked_mul(BOUND_SCALE).unwrap();
}

fn initialize_payout_ledger(group: &mut MarketGroupV15) {
    let snapshot_residual = group.vault.saturating_sub(group.c_tot + group.insurance);
    let total_bound_num = group.pnl_pos_bound_tot_num;
    group.payout_snapshot = snapshot_residual;
    group.payout_snapshot_pnl_pos_tot = group.pnl_pos_bound_tot;
    group.payout_snapshot_captured = true;
    group.resolved_payout_ledger = ResolvedPayoutLedgerV15 {
        snapshot_residual,
        terminal_claim_exact_receipts_num: 0,
        terminal_claim_bound_unreceipted_num: total_bound_num,
        current_payout_rate_num: if total_bound_num == 0 {
            1
        } else {
            snapshot_residual
                .checked_mul(BOUND_SCALE)
                .unwrap()
                .min(total_bound_num)
        },
        current_payout_rate_den: if total_bound_num == 0 {
            1
        } else {
            total_bound_num
        },
        snapshot_slot: group.current_slot.max(group.resolved_slot),
        payout_halted: false,
        finalized: false,
    };
}

fn tight_envelope_config() -> V15Config {
    let mut cfg = V15Config::public_user_fund(4, 0, 10);
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

fn account() -> PortfolioAccountV15 {
    let (market, account_id, owner) = ids();
    PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, account_id, owner))
}

fn account_with_id(id: u8) -> PortfolioAccountV15 {
    let (market, _, owner) = ids();
    PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, [id; 32], owner))
}

fn attach_opposite(
    group: &mut MarketGroupV15,
    asset_index: usize,
    side_to_balance: SideV15,
    abs_q: u128,
    account_id: u8,
) -> PortfolioAccountV15 {
    let mut opposite = account_with_id(account_id);
    let abs_i128 = i128::try_from(abs_q).unwrap();
    match side_to_balance {
        SideV15::Long => group
            .attach_leg(&mut opposite, asset_index, SideV15::Short, -abs_i128)
            .unwrap(),
        SideV15::Short => group
            .attach_leg(&mut opposite, asset_index, SideV15::Long, abs_i128)
            .unwrap(),
    }
    opposite
}

fn active_leg(side: SideV15, basis_pos_q: i128) -> PortfolioLegV15 {
    PortfolioLegV15 {
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

fn assert_pod_zeroable<T: bytemuck::Pod + bytemuck::Zeroable>() {}

#[test]
fn v15_persisted_account_wire_structs_are_bytemuck_pod() {
    assert_pod_zeroable::<V15PodU16>();
    assert_pod_zeroable::<V15PodU32>();
    assert_pod_zeroable::<V15PodU64>();
    assert_pod_zeroable::<V15PodU128>();
    assert_pod_zeroable::<V15PodI128>();
    assert_pod_zeroable::<V15OptionalRecoveryReasonAccount>();
    assert_pod_zeroable::<ProvenanceHeaderV15Account>();
    assert_pod_zeroable::<V15ConfigAccount>();
    assert_pod_zeroable::<AssetStateV15Account>();
    assert_pod_zeroable::<PortfolioLegV15Account>();
    assert_pod_zeroable::<HealthCertV15Account>();
    assert_pod_zeroable::<PortfolioAccountV15Account>();
    assert_pod_zeroable::<MarketGroupV15Account>();

    assert_eq!(core::mem::align_of::<PortfolioAccountV15Account>(), 1);
    assert_eq!(core::mem::align_of::<MarketGroupV15Account>(), 1);
}

#[test]
fn v15_persisted_account_wire_roundtrips_runtime_state() {
    let mut g = group();
    let mut a = account();
    g.create_portfolio_account(&a).unwrap();
    g.deposit_not_atomic(&mut a, 10_000).unwrap();
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV15::Long, POS_SCALE, 90);
    g.full_account_refresh(&mut a, &[100; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    let wire_group = MarketGroupV15Account::from_runtime(&g);
    let wire_account = PortfolioAccountV15Account::from_runtime(&a);
    let group_bytes = bytemuck::bytes_of(&wire_group);
    let account_bytes = bytemuck::bytes_of(&wire_account);

    assert_eq!(
        group_bytes.len(),
        core::mem::size_of::<MarketGroupV15Account>()
    );
    assert_eq!(
        account_bytes.len(),
        core::mem::size_of::<PortfolioAccountV15Account>()
    );

    let decoded_group = *bytemuck::from_bytes::<MarketGroupV15Account>(group_bytes);
    let decoded_account = *bytemuck::from_bytes::<PortfolioAccountV15Account>(account_bytes);
    let runtime_group = decoded_group.validate().unwrap();
    let runtime_account = decoded_account
        .validate_with_market(&runtime_group)
        .unwrap();

    assert_eq!(runtime_group, g);
    assert_eq!(runtime_account, a);
}

#[test]
fn v15_persisted_account_wire_rejects_invalid_bool_enum_and_option_encoding() {
    let g = group();
    let a = account();

    let mut bad_account_bool = PortfolioAccountV15Account::from_runtime(&a);
    bad_account_bool.stale_state = 2;
    assert_eq!(
        bad_account_bool.try_to_runtime(),
        Err(V15Error::InvalidConfig)
    );

    let mut bad_leg_enum = PortfolioAccountV15Account::from_runtime(&a);
    bad_leg_enum.legs[0].active = 1;
    bad_leg_enum.legs[0].side = 9;
    assert_eq!(bad_leg_enum.try_to_runtime(), Err(V15Error::InvalidConfig));

    let mut bad_market_mode = MarketGroupV15Account::from_runtime(&g);
    bad_market_mode.mode = 9;
    assert_eq!(
        bad_market_mode.try_to_runtime(),
        Err(V15Error::InvalidConfig)
    );

    let mut bad_config_bool = MarketGroupV15Account::from_runtime(&g);
    bad_config_bool.config.recovery_fallback_price_enabled = 2;
    assert_eq!(
        bad_config_bool.try_to_runtime(),
        Err(V15Error::InvalidConfig)
    );

    let mut bad_side_mode = MarketGroupV15Account::from_runtime(&g);
    bad_side_mode.assets[0].mode_long = 9;
    assert_eq!(bad_side_mode.try_to_runtime(), Err(V15Error::InvalidConfig));

    let mut bad_option = MarketGroupV15Account::from_runtime(&g);
    bad_option.recovery_reason.present = 0;
    bad_option.recovery_reason.value = 1;
    assert_eq!(bad_option.try_to_runtime(), Err(V15Error::InvalidConfig));
}

#[test]
fn v15_hlock_is_permissionless_state_not_oracle_input() {
    let mut g = group();
    let mut a = account();

    assert_eq!(g.h_lock_lane(Some(&a), false), Ok(HLockLaneV15::HMin));
    assert_eq!(g.select_h_lock(Some(&a), false), Ok(0));

    g.threshold_stress_active = true;
    assert_eq!(g.h_lock_lane(Some(&a), false), Ok(HLockLaneV15::HMax));
    assert_eq!(g.select_h_lock(Some(&a), false), Ok(10));

    g.threshold_stress_active = false;
    assert_eq!(g.h_lock_lane(Some(&a), true), Ok(HLockLaneV15::HMax));

    a.b_stale_state = true;
    assert_eq!(g.h_lock_lane(Some(&a), false), Ok(HLockLaneV15::HMax));
}

#[test]
fn v15_provenance_binds_account_to_market_owner_and_layout() {
    let g = group();
    let mut a = account();
    assert_eq!(g.validate_portfolio_account_provenance(&a), Ok(()));

    a.provenance_header.market_group_id = [9; 32];
    assert_eq!(
        g.validate_portfolio_account_provenance(&a),
        Err(V15Error::ProvenanceMismatch)
    );
}

#[test]
fn v15_active_bitmap_is_the_only_active_leg_authority() {
    let g = group();
    let mut a = account();
    a.legs[0] = active_leg(SideV15::Long, 1);
    assert_eq!(g.validate_account_shape(&a), Err(V15Error::HiddenLeg));

    a.active_bitmap = 1;
    assert_eq!(g.validate_account_shape(&a), Ok(()));

    a.legs[5] = active_leg(SideV15::Short, -1);
    a.active_bitmap |= 1 << 5;
    assert_eq!(g.validate_account_shape(&a), Err(V15Error::HiddenLeg));
}

#[test]
fn v15_same_asset_duplicate_leg_cannot_double_count_support() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    let account_before = a;
    let asset_before = g.assets[0];

    assert_eq!(
        g.attach_leg(&mut a, 0, SideV15::Short, -(POS_SCALE as i128)),
        Err(V15Error::InvalidLeg)
    );
    assert_eq!(a, account_before);
    assert_eq!(g.assets[0], asset_before);
    assert_eq!(a.active_bitmap.count_ones(), 1);
    assert_eq!(g.validate_account_shape(&a), Ok(()));

    g.full_account_refresh(&mut a, &[1; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(a.health_cert.active_bitmap_at_cert, 1);
}

#[test]
fn v15_stale_and_b_stale_counters_are_exact_and_idempotent() {
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
fn v15_b_stale_account_cannot_clear_while_leg_is_b_stale() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();

    g.mark_leg_b_stale(&mut a, 0).unwrap();
    g.mark_leg_b_stale(&mut a, 0).unwrap();
    assert!(a.b_stale_state);
    assert!(a.legs[0].b_stale);
    assert_eq!(g.b_stale_account_count, 1);

    assert_eq!(g.clear_account_b_stale(&mut a), Err(V15Error::BStale));
    assert!(a.b_stale_state);
    assert_eq!(g.b_stale_account_count, 1);
}

#[test]
fn v15_full_refresh_clears_stale_certificate_but_not_b_stale_loss() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.full_account_refresh(&mut a, &[100; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    g.mark_account_stale(&mut a).unwrap();
    assert_eq!(g.stale_certificate_count, 1);
    assert_eq!(
        g.ensure_favorable_action_allowed(&a),
        Err(V15Error::LockActive)
    );

    g.full_account_refresh(&mut a, &[100; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(g.stale_certificate_count, 0);
    assert!(!a.stale_state);
    assert_eq!(g.ensure_favorable_action_allowed(&a), Ok(()));

    g.assets[0].b_long_num = SOCIAL_LOSS_DEN;
    assert_eq!(
        g.full_account_refresh(&mut a, &[100; V15_MAX_PORTFOLIO_ASSETS_N]),
        Err(V15Error::BStale)
    );
}

#[test]
fn v15_favorable_action_requires_current_full_account_refresh() {
    let mut g = group();
    let mut a = account();
    a.capital = 100;
    g.attach_leg(&mut a, 0, SideV15::Long, 1_000_000).unwrap();
    let mut prices = [1u64; V15_MAX_PORTFOLIO_ASSETS_N];
    prices[0] = 100;

    assert_eq!(g.ensure_favorable_action_allowed(&a), Err(V15Error::Stale));

    let cert = g.full_account_refresh(&mut a, &prices).unwrap();
    assert!(cert.valid);
    assert_eq!(cert.certified_maintenance_req, 100);
    assert_eq!(g.ensure_favorable_action_allowed(&a), Ok(()));

    g.oracle_epoch += 1;
    assert_eq!(g.ensure_favorable_action_allowed(&a), Err(V15Error::Stale));
}

#[test]
fn v15_health_certificate_is_bound_to_market_epochs_and_prices() {
    let mut g = group();
    let mut long = account();
    let mut short = account_with_id(111);
    g.deposit_not_atomic(&mut long, 1_000).unwrap();
    g.deposit_not_atomic(&mut short, 1_000).unwrap();
    g.attach_leg(&mut long, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut short, 0, SideV15::Short, -(POS_SCALE as i128))
        .unwrap();

    let cert = g
        .full_account_refresh(&mut long, &[1; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(cert.cert_oracle_epoch, g.oracle_epoch);
    assert_eq!(cert.cert_funding_epoch, g.funding_epoch);
    assert_eq!(cert.cert_risk_epoch, g.risk_epoch);
    assert_eq!(cert.active_bitmap_at_cert, long.active_bitmap);
    assert_eq!(g.ensure_favorable_action_allowed(&long), Ok(()));

    g.accrue_asset_to_not_atomic(0, 1, 2, 0, true).unwrap();
    assert_eq!(
        g.ensure_favorable_action_allowed(&long),
        Err(V15Error::Stale)
    );

    let refreshed = g
        .full_account_refresh(&mut long, &[2; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(refreshed.cert_oracle_epoch, g.oracle_epoch);
}

#[test]
fn v15_global_residual_is_not_account_health_proof() {
    let mut g = group();
    let mut a = account();
    a.pnl = 10;
    a.reserved_pnl = 0;
    g.pnl_pos_tot = 10;
    set_junior_bound(&mut g, 10);
    g.pnl_matured_pos_tot = 10;
    g.vault = g.c_tot + g.insurance + 10;
    assert_eq!(g.assert_public_invariants(), Ok(()));
    assert!(!a.health_cert.valid);

    let before_group = g;
    let before_account = a;
    assert_eq!(
        g.convert_released_pnl_to_capital_not_atomic(&mut a),
        Err(V15Error::Stale)
    );
    assert_eq!(g, before_group);
    assert_eq!(a, before_account);
}

#[test]
fn v15_full_refresh_haircuts_positive_pnl_credit_when_junior_claims_are_impaired() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 10).unwrap();
    a.pnl = 100;
    g.pnl_pos_tot = 100;
    set_junior_bound(&mut g, 100);
    g.vault = g.c_tot + g.insurance + 25;

    let cert = g
        .full_account_refresh(&mut a, &[1; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(account_equity(&a), Ok(110));
    assert_eq!(cert.certified_equity, 35);
}

#[test]
fn v15_full_refresh_uses_haircut_bounded_support_for_negative_kf_delta_when_impaired() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();

    a.pnl = 100;
    g.pnl_pos_tot = 100;
    set_junior_bound(&mut g, 100);
    g.vault = 50;
    g.assets[0].k_long = -(100 * ADL_ONE as i128);

    let cert = g
        .full_account_refresh(&mut a, &[1; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(
        a.pnl, -50,
        "negative K/F settlement must consume only haircut-valued positive support, not face PnL"
    );
    assert_eq!(g.pnl_pos_tot, 0);
    assert_eq!(g.pnl_pos_bound_tot, 0);
    assert_eq!(g.negative_pnl_account_count, 1);
    assert_eq!(cert.certified_equity, -50);
}

#[test]
fn v15_full_refresh_uses_haircut_bounded_new_positive_kf_to_cure_prior_loss() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut a, 1, SideV15::Long, POS_SCALE as i128)
        .unwrap();

    g.vault = 50;
    g.assets[0].k_long = -(100 * ADL_ONE as i128);
    g.assets[1].k_long = 100 * ADL_ONE as i128;

    let cert = g
        .full_account_refresh(&mut a, &[1; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(
        a.pnl, -50,
        "new positive K/F support must cure prior losses only at haircut value"
    );
    assert_eq!(g.pnl_pos_tot, 0);
    assert_eq!(g.pnl_pos_bound_tot, 0);
    assert_eq!(g.negative_pnl_account_count, 1);
    assert_eq!(cert.certified_equity, -50);
}

#[test]
fn v15_withdraw_uses_haircut_positive_credit_not_face_pnl_when_unlocked() {
    let (market, _, _) = ids();
    let mut cfg = V15Config::public_user_fund(1, 0, 10);
    cfg.min_nonzero_im_req = 20;
    let mut g = MarketGroupV15::new(market, cfg).unwrap();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 30).unwrap();
    g.attach_leg(&mut a, 0, SideV15::Long, 1).unwrap();
    a.pnl = 100;
    g.pnl_pos_tot = 100;
    set_junior_bound(&mut g, 100);
    g.vault = g.c_tot + g.insurance + 10;

    assert_eq!(
        g.withdraw_not_atomic(&mut a, 25, &[1; V15_MAX_PORTFOLIO_ASSETS_N]),
        Err(V15Error::InvalidConfig)
    );
    assert_eq!(a.capital, 30);
    assert_eq!(g.c_tot, 30);
}

#[test]
fn v15_stale_profitable_leg_cannot_withdraw_using_pre_refresh_positive_pnl() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 40).unwrap();
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    a.pnl = 100;
    g.pnl_pos_tot = 100;
    set_junior_bound(&mut g, 100);
    g.vault = g.c_tot + 50;
    g.assets[0].k_long = -(100 * ADL_ONE as i128);
    g.mark_account_stale(&mut a).unwrap();

    let before_vault = g.vault;
    let before_c_tot = g.c_tot;
    let res = g.withdraw_not_atomic(&mut a, 41, &[1; V15_MAX_PORTFOLIO_ASSETS_N]);

    assert!(res.is_err());
    assert_eq!(
        g.vault, before_vault,
        "withdraw must not extract vault value using stale positive PnL"
    );
    assert!(
        g.c_tot <= before_c_tot,
        "only loss settlement may reduce senior capital before rejection"
    );
    assert!(
        a.pnl <= 0,
        "pre-refresh positive PnL must be consumed by current hidden losses"
    );
}

#[test]
fn v15_public_invariants_reject_broken_senior_claim_conservation() {
    let mut g = group();
    g.vault = 10;
    g.c_tot = 8;
    g.insurance = 3;

    assert_eq!(g.assert_public_invariants(), Err(V15Error::InvalidConfig));

    g.insurance = 2;
    assert_eq!(g.assert_public_invariants(), Ok(()));
}

#[test]
fn v15_public_invariants_reject_persistent_asset_kf_i128_min() {
    let mut g = group();
    g.assets[0].k_long = i128::MIN;
    assert_eq!(g.assert_public_invariants(), Err(V15Error::InvalidConfig));

    let mut g = group();
    g.assets[0].f_epoch_start_short_num = i128::MIN;
    assert_eq!(g.assert_public_invariants(), Err(V15Error::InvalidConfig));
}

#[test]
fn v15_public_invariants_reject_oi_loss_weight_shape_mismatch() {
    let mut g = group();
    g.assets[0].oi_eff_long_q = 1;
    assert_eq!(g.assert_public_invariants(), Err(V15Error::InvalidConfig));

    let mut g = group();
    g.assets[0].loss_weight_sum_short = 1;
    assert_eq!(g.assert_public_invariants(), Err(V15Error::InvalidConfig));
}

#[test]
fn v15_public_invariants_reject_live_oi_imbalance() {
    let mut g = group();
    let mut long = account();
    g.attach_leg(&mut long, 0, SideV15::Long, 1).unwrap();
    assert_eq!(g.assert_public_invariants(), Err(V15Error::InvalidConfig));

    let mut short = PortfolioAccountV15::empty(ProvenanceHeaderV15::new([1; 32], [9; 32], [3; 32]));
    g.attach_leg(&mut short, 0, SideV15::Short, -1).unwrap();
    assert_eq!(g.assert_public_invariants(), Ok(()));
}

#[test]
fn v15_cross_margin_collateral_counted_once_and_not_below_loss_envelope() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 1_000_000).unwrap();
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut a, 1, SideV15::Short, -(POS_SCALE as i128))
        .unwrap();
    let prices = [1_000_000; V15_MAX_PORTFOLIO_ASSETS_N];

    let cert = g.full_account_refresh(&mut a, &prices).unwrap();
    let leg0_loss = risk_notional_ceil(POS_SCALE, prices[0]).unwrap();
    let leg1_loss = risk_notional_ceil(POS_SCALE, prices[1]).unwrap();
    let envelope = leg0_loss + leg1_loss;

    assert_eq!(cert.certified_equity, account_equity(&a).unwrap());
    assert_eq!(cert.certified_equity, 1_000_000);
    assert_eq!(cert.certified_worst_case_loss, envelope);
    assert_eq!(cert.certified_maintenance_req, envelope);
    assert_eq!(cert.certified_liq_deficit, envelope - 1_000_000);
}

#[test]
fn v15_global_cross_margin_positive_leg_supports_other_leg_maintenance_without_b_domain() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 1).unwrap();
    g.vault += 3;
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut a, 1, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    let _opp0 = attach_opposite(&mut g, 0, SideV15::Long, POS_SCALE, 9);
    let _opp1 = attach_opposite(&mut g, 1, SideV15::Long, POS_SCALE, 10);
    g.assets[0].k_long = -2 * ADL_ONE as i128;
    g.assets[1].k_long = 3 * ADL_ONE as i128;

    let cert = g
        .full_account_refresh(&mut a, &[1; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(a.pnl, 1);
    assert_eq!(cert.certified_equity, 2);
    assert_eq!(cert.certified_maintenance_req, 2);
    assert_eq!(cert.certified_liq_deficit, 0);
    assert_eq!(g.insurance_domain_spent, [0; V15_DOMAIN_COUNT]);
    assert_eq!(g.pending_domain_loss_barriers, [0; V15_DOMAIN_COUNT]);
    assert_eq!(g.assets[0].b_long_num, 0);
    assert_eq!(g.assets[0].b_short_num, 0);
    assert_eq!(g.assets[1].b_long_num, 0);
    assert_eq!(g.assets[1].b_short_num, 0);
    assert_eq!(g.assert_public_invariants(), Ok(()));
}

#[test]
fn v15_b_stale_blocks_refresh_and_favorable_actions_without_scanning_market() {
    let mut g = group();
    let mut a = account();
    a.capital = 100;
    g.attach_leg(&mut a, 0, SideV15::Long, 1_000_000).unwrap();
    let prices = [100u64; V15_MAX_PORTFOLIO_ASSETS_N];

    g.mark_account_b_stale(&mut a).unwrap();
    assert_eq!(
        g.full_account_refresh(&mut a, &prices),
        Err(V15Error::BStale)
    );
    assert_eq!(
        g.ensure_favorable_action_allowed(&a),
        Err(V15Error::LockActive)
    );
}

#[test]
fn v15_public_init_rejects_unbounded_portfolio_width() {
    let (market, _, _) = ids();
    let cfg = V15Config::public_user_fund((V15_MAX_PORTFOLIO_ASSETS_N + 1) as u8, 0, 10);
    assert_eq!(
        MarketGroupV15::new(market, cfg),
        Err(V15Error::InvalidConfig)
    );
}

#[test]
fn v15_public_init_rejects_disabled_recovery_profile() {
    let (market, _, _) = ids();
    let mut cfg = V15Config::public_user_fund(4, 0, 10);
    cfg.permissionless_recovery_enabled = false;

    assert_eq!(
        MarketGroupV15::new(market, cfg),
        Err(V15Error::InvalidConfig)
    );
}

#[test]
fn v15_public_init_rejects_disabled_recovery_fallback_price_policy() {
    let (market, _, _) = ids();
    let mut cfg = V15Config::public_user_fund(4, 0, 10);
    cfg.recovery_fallback_price_enabled = false;

    assert_eq!(
        MarketGroupV15::new(market, cfg),
        Err(V15Error::InvalidConfig)
    );
}

#[test]
fn v15_public_init_requires_crankforward_recovery_and_chunk_caps() {
    let (market, _, _) = ids();

    let mut cfg = V15Config::public_user_fund(4, 0, 10);
    cfg.stale_certificate_penalty_enabled = false;
    assert_eq!(
        MarketGroupV15::new(market, cfg),
        Err(V15Error::InvalidConfig)
    );

    let mut cfg = V15Config::public_user_fund(4, 0, 10);
    cfg.full_refresh_required_for_favorable_actions = false;
    assert_eq!(
        MarketGroupV15::new(market, cfg),
        Err(V15Error::InvalidConfig)
    );

    let mut cfg = V15Config::public_user_fund(4, 0, 10);
    cfg.public_liveness_profile_crank_forward = false;
    assert_eq!(
        MarketGroupV15::new(market, cfg),
        Err(V15Error::InvalidConfig)
    );

    let mut cfg = V15Config::public_user_fund(4, 0, 10);
    cfg.max_account_b_settlement_chunks = 0;
    assert_eq!(
        MarketGroupV15::new(market, cfg),
        Err(V15Error::InvalidConfig)
    );

    let mut cfg = V15Config::public_user_fund(4, 0, 10);
    cfg.max_bankrupt_close_chunks = 0;
    assert_eq!(
        MarketGroupV15::new(market, cfg),
        Err(V15Error::InvalidConfig)
    );

    let mut cfg = V15Config::public_user_fund(4, 0, 10);
    cfg.max_bankrupt_close_lifetime_slots = 0;
    assert_eq!(
        MarketGroupV15::new(market, cfg),
        Err(V15Error::InvalidConfig)
    );
}

#[test]
fn v15_public_init_accepts_tight_exact_solvency_envelope() {
    let (market, _, _) = ids();
    let cfg = tight_envelope_config();
    assert!(MarketGroupV15::new(market, cfg).is_ok());
}

#[test]
fn v15_public_init_rejects_price_funding_or_liquidation_envelope_breach() {
    let (market, _, _) = ids();

    let mut price_breach = tight_envelope_config();
    price_breach.max_price_move_bps_per_slot = 10;
    assert_eq!(
        MarketGroupV15::new(market, price_breach),
        Err(V15Error::InvalidConfig)
    );

    let mut funding_breach = tight_envelope_config();
    funding_breach.max_accrual_dt_slots = 10_000;
    funding_breach.min_funding_lifetime_slots = 10_000;
    assert_eq!(
        MarketGroupV15::new(market, funding_breach),
        Err(V15Error::InvalidConfig)
    );

    let mut liquidation_breach = tight_envelope_config();
    liquidation_breach.liquidation_fee_bps = 400;
    assert_eq!(
        MarketGroupV15::new(market, liquidation_breach),
        Err(V15Error::InvalidConfig)
    );
}

#[test]
fn v15_public_init_rejects_zero_price_move_cap() {
    let (market, _, _) = ids();
    let mut cfg = V15Config::public_user_fund(1, 0, 10);
    cfg.max_price_move_bps_per_slot = 0;

    assert_eq!(
        MarketGroupV15::new(market, cfg),
        Err(V15Error::InvalidConfig)
    );
}

#[test]
fn v15_oracle_price_zero_rejected_and_max_price_accepted_when_unexposed() {
    let mut g = group();
    let before = g;

    assert_eq!(
        g.accrue_asset_to_not_atomic(0, 1, 0, 0, false),
        Err(V15Error::InvalidConfig)
    );
    assert_eq!(g, before);

    let out = g
        .accrue_asset_to_not_atomic(0, 1, MAX_ORACLE_PRICE, 0, false)
        .unwrap();
    assert!(!out.equity_active);
    assert_eq!(g.assets[0].effective_price, MAX_ORACLE_PRICE);
}

#[test]
fn v15_public_init_accepts_capped_liquidation_fee_envelope() {
    let (market, _, _) = ids();
    let mut cfg = tight_envelope_config();
    cfg.liquidation_fee_bps = 10_000;
    cfg.liquidation_fee_cap = 1;
    cfg.min_liquidation_abs = 0;
    assert!(MarketGroupV15::new(market, cfg).is_ok());
}

#[test]
fn v15_public_init_accepts_capped_liquidation_fee_with_min_near_cap() {
    let (market, _, _) = ids();
    let mut cfg = tight_envelope_config();
    cfg.liquidation_fee_bps = 10_000;
    cfg.liquidation_fee_cap = 100;
    cfg.min_liquidation_abs = 99;
    cfg.min_nonzero_mm_req = 300;
    cfg.min_nonzero_im_req = 301;
    assert!(MarketGroupV15::new(market, cfg).is_ok());
}

#[test]
fn v15_public_init_handles_zero_proportional_maintenance_exactly() {
    let (market, _, _) = ids();
    let mut cfg = V15Config::public_user_fund(4, 0, 10);
    cfg.maintenance_margin_bps = 0;
    cfg.max_price_move_bps_per_slot = 1;
    cfg.max_accrual_dt_slots = 1;
    cfg.min_funding_lifetime_slots = 1;
    cfg.max_abs_funding_e9_per_slot = 0;
    cfg.min_nonzero_mm_req = MAX_ACCOUNT_NOTIONAL;
    cfg.min_nonzero_im_req = MAX_ACCOUNT_NOTIONAL + 1;
    assert!(MarketGroupV15::new(market, cfg).is_ok());

    cfg.min_nonzero_mm_req = 1;
    cfg.min_nonzero_im_req = 2;
    assert_eq!(
        MarketGroupV15::new(market, cfg),
        Err(V15Error::InvalidConfig)
    );
}

#[test]
fn v15_public_init_rejects_funding_headroom_overflow() {
    let (market, _, _) = ids();
    let mut cfg = V15Config::public_user_fund(4, 0, 10);
    cfg.max_accrual_dt_slots = 1_000_000_000;
    cfg.min_funding_lifetime_slots = 1_000_000_000;
    cfg.max_abs_funding_e9_per_slot = 10_000;
    assert_eq!(
        MarketGroupV15::new(market, cfg),
        Err(V15Error::InvalidConfig)
    );
}

#[test]
fn v15_public_init_accepts_exact_envelope_boundary() {
    let (market, _, _) = ids();
    let mut cfg = V15Config::public_user_fund(4, 0, 10);
    cfg.maintenance_margin_bps = 500;
    cfg.initial_margin_bps = 600;
    cfg.max_price_move_bps_per_slot = 390;
    cfg.max_accrual_dt_slots = 1;
    cfg.min_funding_lifetime_slots = 1;
    cfg.max_abs_funding_e9_per_slot = 0;
    cfg.min_nonzero_mm_req = 200;
    cfg.min_nonzero_im_req = 201;
    assert!(MarketGroupV15::new(market, cfg).is_ok());
}

#[test]
fn v15_risk_notional_and_equity_use_exact_conservative_shapes() {
    assert_eq!(risk_notional_ceil(1, 1), Ok(1));
    assert_eq!(risk_notional_ceil(1, 1_000_001), Ok(2));

    let mut a = account();
    a.capital = 100;
    a.pnl = -25;
    a.fee_credits = -10;
    assert_eq!(account_equity(&a), Ok(65));
}

#[test]
fn v15_account_equity_rejects_capital_above_i128_max() {
    let mut a = account();
    a.capital = i128::MAX as u128 + 1;
    assert_eq!(account_equity(&a), Err(V15Error::ArithmeticOverflow));
}

#[test]
fn v15_min_nonzero_initial_floor_blocks_tiny_risk_increasing_trade() {
    let (market, account_id, owner) = ids();
    let mut cfg = V15Config::public_user_fund(1, 0, 1);
    cfg.min_nonzero_mm_req = 49;
    cfg.min_nonzero_im_req = 50;
    let mut g = MarketGroupV15::new(market, cfg).unwrap();
    let mut long = PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, account_id, owner));
    let mut short = PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, [4; 32], owner));
    g.deposit_not_atomic(&mut long, 49).unwrap();
    g.deposit_not_atomic(&mut short, 100).unwrap();
    let before_group = g;
    let before_long = long;
    let before_short = short;

    let result = g.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV15 {
            asset_index: 0,
            size_q: 1,
            exec_price: 1,
            fee_bps: 0,
        },
        &[1; V15_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(result, Err(V15Error::InvalidConfig));
    assert_eq!(g, before_group);
    assert_eq!(long, before_long);
    assert_eq!(short, before_short);
}

#[test]
fn v15_account_shape_rejects_malformed_persistent_economic_state() {
    let g = group();

    let mut min_pnl = account();
    min_pnl.pnl = i128::MIN;
    assert_eq!(
        g.validate_account_shape(&min_pnl),
        Err(V15Error::ArithmeticOverflow)
    );

    let mut positive_fee_credit = account();
    positive_fee_credit.fee_credits = 1;
    assert_eq!(
        g.validate_account_shape(&positive_fee_credit),
        Err(V15Error::InvalidLeg)
    );

    let mut min_fee_credit = account();
    min_fee_credit.fee_credits = i128::MIN;
    assert_eq!(
        g.validate_account_shape(&min_fee_credit),
        Err(V15Error::ArithmeticOverflow)
    );

    let mut over_reserved = account();
    over_reserved.pnl = 1;
    over_reserved.reserved_pnl = 2;
    assert_eq!(
        g.validate_account_shape(&over_reserved),
        Err(V15Error::InvalidLeg)
    );
}

#[test]
fn v15_flat_account_equity_is_capital_plus_pnl_minus_fee_debt() {
    let mut a = account();
    a.capital = 123;
    a.pnl = -45;
    a.fee_credits = -6;
    assert_eq!(account_equity(&a), Ok(72));

    a.pnl = 45;
    assert_eq!(account_equity(&a), Ok(162));
}

#[test]
fn v15_authoritatively_flat_account_never_receives_b_loss() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.assets[0].b_long_num = 10;
    g.assets[0].b_short_num = 7;

    let outcome = g
        .settle_account_side_effects_not_atomic(&mut a, g.config.public_b_chunk_atoms)
        .unwrap();

    assert_eq!(outcome, PermissionlessProgressOutcomeV15::AccountCurrent);
    assert_eq!(a.active_bitmap, 0);
    assert_eq!(a.pnl, 0);
    assert_eq!(a.capital, 100);
    assert!(!a.b_stale_state);
    assert_eq!(g.b_stale_account_count, 0);
}

#[test]
fn v15_deposit_withdraw_roundtrip_preserves_accounting() {
    let mut g = group();
    let mut a = account();

    g.deposit_not_atomic(&mut a, 123).unwrap();
    assert_eq!(a.capital, 123);
    assert_eq!(g.c_tot, 123);
    assert_eq!(g.vault, 123);

    g.withdraw_not_atomic(&mut a, 123, &[1; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(a.capital, 0);
    assert_eq!(g.c_tot, 0);
    assert_eq!(g.vault, 0);
    assert_eq!(g.assert_public_invariants(), Ok(()));
}

#[test]
fn v15_deposit_does_not_draw_insurance_or_sweep_loss_bearing_account() {
    let mut g = group();
    let mut a = account();
    g.vault = 50;
    g.insurance = 50;
    g.attach_leg(&mut a, 0, SideV15::Long, 10).unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV15::Long, 10, 91);
    a.pnl = -100;
    a.fee_credits = -7;

    let insurance_before = g.insurance;
    let pnl_before = a.pnl;
    let fee_credits_before = a.fee_credits;
    let bitmap_before = a.active_bitmap;
    let leg_before = a.legs[0];

    g.deposit_not_atomic(&mut a, 10).unwrap();

    assert_eq!(g.insurance, insurance_before);
    assert_eq!(a.pnl, pnl_before);
    assert_eq!(a.fee_credits, fee_credits_before);
    assert_eq!(a.active_bitmap, bitmap_before);
    assert_eq!(a.legs[0], leg_before);
    assert_eq!(a.capital, 10);
    assert_eq!(g.c_tot, 10);
    assert_eq!(g.vault, 60);
    assert_eq!(g.assert_public_invariants(), Ok(()));
}

#[test]
fn v15_deposit_never_sweeps_fee_debt_even_when_flat_and_nonnegative() {
    let mut g = group();
    let mut a = account();
    a.pnl = 3;
    a.fee_credits = -7;

    g.deposit_not_atomic(&mut a, 10).unwrap();

    assert_eq!(a.pnl, 3);
    assert_eq!(a.fee_credits, -7);
    assert_eq!(a.capital, 10);
    assert_eq!(g.c_tot, 10);
    assert_eq!(g.vault, 10);
    assert_eq!(g.insurance, 0);
}

#[test]
fn v15_partial_withdraw_can_leave_small_remainder() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 5_000).unwrap();

    g.withdraw_not_atomic(&mut a, 4_500, &[1; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(a.capital, 500);
    assert_eq!(g.c_tot, 500);
    assert_eq!(g.vault, 500);
    assert_eq!(g.assert_public_invariants(), Ok(()));
}

#[test]
fn v15_over_withdraw_rejects_before_any_accounting_mutation() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 10).unwrap();
    let capital_before = a.capital;
    let pnl_before = a.pnl;
    let fee_credits_before = a.fee_credits;
    let active_bitmap_before = a.active_bitmap;
    let legs_before = a.legs;
    let vault_before = g.vault;
    let c_tot_before = g.c_tot;
    let insurance_before = g.insurance;

    let res = g.withdraw_not_atomic(&mut a, 11, &[1; V15_MAX_PORTFOLIO_ASSETS_N]);

    assert_eq!(res, Err(V15Error::LockActive));
    assert_eq!(a.capital, capital_before);
    assert_eq!(a.pnl, pnl_before);
    assert_eq!(a.fee_credits, fee_credits_before);
    assert_eq!(a.active_bitmap, active_bitmap_before);
    assert_eq!(a.legs, legs_before);
    assert_eq!(g.vault, vault_before);
    assert_eq!(g.c_tot, c_tot_before);
    assert_eq!(g.insurance, insurance_before);
}

#[test]
fn v15_close_portfolio_account_requires_clean_local_state() {
    let mut g = group();
    let mut a = account();
    g.create_portfolio_account(&a).unwrap();
    assert_eq!(g.materialized_portfolio_count, 1);

    a.capital = 1;
    assert_eq!(g.close_portfolio_account(&a), Err(V15Error::LockActive));
    assert_eq!(g.materialized_portfolio_count, 1);

    a.capital = 0;
    a.b_stale_state = true;
    assert_eq!(g.close_portfolio_account(&a), Err(V15Error::LockActive));
    assert_eq!(g.materialized_portfolio_count, 1);

    a.b_stale_state = false;
    a.capital = 0;
    g.close_portfolio_account(&a).unwrap();
    assert_eq!(g.materialized_portfolio_count, 0);
}

#[test]
fn v15_attach_and_clear_leg_update_only_bounded_account_and_asset_state() {
    let mut g = group();
    let mut a = account();

    g.attach_leg(&mut a, 1, SideV15::Short, -7).unwrap();
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
fn v15_bilateral_oi_decomposition_counts_only_active_side_exposure() {
    let mut g = group();
    let mut long = account();
    let mut short = account();
    short.provenance_header.portfolio_account_id = [4; 32];

    g.attach_leg(&mut long, 0, SideV15::Long, 3).unwrap();
    g.attach_leg(&mut short, 0, SideV15::Short, -3).unwrap();

    assert_eq!(g.assets[0].oi_eff_long_q, 3);
    assert_eq!(g.assets[0].oi_eff_short_q, 3);
    assert_eq!(g.assets[0].stored_pos_count_long, 1);
    assert_eq!(g.assets[0].stored_pos_count_short, 1);
    assert_eq!(long.active_bitmap, 1);
    assert_eq!(short.active_bitmap, 1);
    assert_eq!(long.legs[0].basis_pos_q, 3);
    assert_eq!(short.legs[0].basis_pos_q, -3);
}

#[test]
fn v15_oversize_position_is_rejected_before_oi_mutation() {
    let mut g = group();
    let mut a = account();

    let res = g.attach_leg(
        &mut a,
        0,
        SideV15::Long,
        (percolator::MAX_POSITION_ABS_Q + 1) as i128,
    );

    assert_eq!(res, Err(V15Error::InvalidLeg));
    assert_eq!(a.active_bitmap, 0);
    assert_eq!(g.assets[0].oi_eff_long_q, 0);
}

#[test]
fn v15_account_b_chunk_makes_strict_account_local_progress_or_requires_recovery() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV15::Long, 1).unwrap();
    g.assets[0].b_long_num = SOCIAL_LOSS_DEN * 2;
    g.mark_leg_b_stale(&mut a, 0).unwrap();

    let chunk = g
        .settle_account_b_chunk(&mut a, 0, SOCIAL_LOSS_DEN)
        .unwrap();
    assert!(chunk.delta_b > 0);
    assert!(a.legs[0].b_snap > 0);
    assert_eq!(a.health_cert.valid, false);

    let mut blocked = account();
    g.attach_leg(&mut blocked, 1, SideV15::Long, 1).unwrap();
    g.assets[1].b_long_num = 1;
    g.mark_leg_b_stale(&mut blocked, 1).unwrap();
    assert_eq!(
        g.settle_account_b_chunk(&mut blocked, 1, 0),
        Err(V15Error::RecoveryRequired)
    );
}

#[test]
fn v15_liquidation_progress_requires_strict_risk_score_reduction() {
    let mut g = group();
    let mut before = account();
    let mut after = account();
    g.full_account_refresh(&mut before, &[1; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    g.full_account_refresh(&mut after, &[1; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    before.health_cert.certified_liq_deficit = 10;
    after.health_cert.certified_liq_deficit = 10;
    assert_eq!(
        g.validate_liquidation_progress(&before, &after),
        Err(V15Error::NonProgress)
    );

    after.health_cert.certified_liq_deficit = 9;
    assert_eq!(g.validate_liquidation_progress(&before, &after), Ok(()));
}

#[test]
fn v15_cyclic_rescue_without_scalar_progress_reverts() {
    let mut g = group();
    let mut before = account();
    let mut after = account();
    g.full_account_refresh(&mut before, &[1; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    g.full_account_refresh(&mut after, &[1; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    before.health_cert.certified_liq_deficit = 5;
    before.health_cert.certified_worst_case_loss = 3;

    after.health_cert.certified_liq_deficit = 5;
    after.health_cert.certified_worst_case_loss = 4;
    assert_eq!(
        g.validate_liquidation_progress(&before, &after),
        Err(V15Error::NonProgress)
    );

    after.health_cert.certified_worst_case_loss = 3;
    after.stale_state = true;
    assert_eq!(
        g.validate_liquidation_progress(&before, &after),
        Err(V15Error::NonProgress)
    );

    after.stale_state = false;
    after.health_cert.certified_liq_deficit = 4;
    assert_eq!(g.validate_liquidation_progress(&before, &after), Ok(()));
}

#[test]
fn v15_permissionless_recovery_is_declared_by_reason_not_caller_price() {
    let mut g = group();
    let reason = PermissionlessRecoveryReasonV15::AccountBSettlementCannotProgress;
    assert_eq!(
        g.declare_permissionless_recovery(reason),
        Ok(PermissionlessProgressOutcomeV15::RecoveryDeclared(reason))
    );
    assert_eq!(g.recovery_reason, Some(reason));
    assert_eq!(g.mode, MarketModeV15::Recovery);
}

#[test]
fn v15_explicit_loss_audit_overflow_declares_recovery_without_value_mutation() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    let vault_before = g.vault;
    let c_tot_before = g.c_tot;
    let insurance_before = g.insurance;
    let pnl_pos_before = g.pnl_pos_tot;
    let asset_before = g.assets[0];

    let out = g
        .declare_explicit_loss_or_dust_audit_overflow_not_atomic()
        .unwrap();

    assert_eq!(
        out,
        PermissionlessProgressOutcomeV15::RecoveryDeclared(
            PermissionlessRecoveryReasonV15::ExplicitLossOrDustAuditOverflow
        )
    );
    assert_eq!(
        g.recovery_reason,
        Some(PermissionlessRecoveryReasonV15::ExplicitLossOrDustAuditOverflow)
    );
    assert_eq!(g.mode, MarketModeV15::Recovery);
    assert_eq!(g.vault, vault_before);
    assert_eq!(g.c_tot, c_tot_before);
    assert_eq!(g.insurance, insurance_before);
    assert_eq!(g.pnl_pos_tot, pnl_pos_before);
    assert_eq!(g.assets[0], asset_before);
}

#[test]
fn v15_permissionless_recovery_enters_terminal_mode_and_enables_dead_leg_forfeit() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV15::Long, 1).unwrap();
    assert_eq!(
        g.forfeit_recovery_leg_not_atomic(&mut a, 0, 1),
        Err(V15Error::LockActive)
    );

    let reason = PermissionlessRecoveryReasonV15::OracleOrTargetUnavailableByAuthenticatedPolicy;
    assert_eq!(
        g.declare_permissionless_recovery(reason),
        Ok(PermissionlessProgressOutcomeV15::RecoveryDeclared(reason))
    );
    let out = g.forfeit_recovery_leg_not_atomic(&mut a, 0, 1).unwrap();

    assert!(out.detached);
    assert_eq!(a.active_bitmap, 0);
    assert_eq!(g.assets[0].oi_eff_long_q, 0);
    assert_eq!(g.recovery_reason, Some(reason));
    assert_eq!(g.mode, MarketModeV15::Recovery);
}

#[test]
fn v15_permissionless_recovery_cannot_override_resolved_mode() {
    let mut g = group();
    g.resolve_market_not_atomic(1).unwrap();

    assert_eq!(
        g.declare_permissionless_recovery(PermissionlessRecoveryReasonV15::BelowProgressFloor),
        Err(V15Error::LockActive)
    );
    assert_eq!(g.mode, MarketModeV15::Resolved);
    assert_eq!(g.recovery_reason, None);
}

#[test]
fn v15_recovery_reason_is_terminal_and_idempotent() {
    let mut g = group();
    let first = PermissionlessRecoveryReasonV15::BelowProgressFloor;
    let second = PermissionlessRecoveryReasonV15::CounterOrEpochOverflowDeclaredRecovery;

    assert_eq!(
        g.declare_permissionless_recovery(first),
        Ok(PermissionlessProgressOutcomeV15::RecoveryDeclared(first))
    );
    assert_eq!(
        g.declare_permissionless_recovery(second),
        Ok(PermissionlessProgressOutcomeV15::RecoveryDeclared(first))
    );
    assert_eq!(g.recovery_reason, Some(first));
    assert_eq!(g.mode, MarketModeV15::Recovery);
}

#[test]
fn v15_recovery_mode_cannot_be_overridden_by_resolve() {
    let mut g = group();
    let reason = PermissionlessRecoveryReasonV15::BelowProgressFloor;
    g.declare_permissionless_recovery(reason).unwrap();

    assert_eq!(g.resolve_market_not_atomic(10), Err(V15Error::LockActive));
    assert_eq!(g.mode, MarketModeV15::Recovery);
    assert_eq!(g.recovery_reason, Some(reason));
    assert_eq!(g.resolved_slot, 0);
}

#[test]
fn v15_recovery_mode_blocks_value_escape_and_fee_sync_before_mutation() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    a.pnl = 10;
    g.pnl_pos_tot = 10;
    g.vault += 10;
    g.full_account_refresh(&mut a, &[1; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    let before = a;
    let vault_before = g.vault;
    let c_tot_before = g.c_tot;
    let insurance_before = g.insurance;
    g.declare_permissionless_recovery(PermissionlessRecoveryReasonV15::BelowProgressFloor)
        .unwrap();

    assert_eq!(
        g.convert_released_pnl_to_capital_not_atomic(&mut a),
        Err(V15Error::LockActive)
    );
    assert_eq!(
        g.withdraw_not_atomic(&mut a, 1, &[1; V15_MAX_PORTFOLIO_ASSETS_N]),
        Err(V15Error::LockActive)
    );
    assert_eq!(
        g.sync_account_fee_to_slot_not_atomic(&mut a, 1, 1),
        Err(V15Error::LockActive)
    );
    assert_eq!(a, before);
    assert_eq!(g.vault, vault_before);
    assert_eq!(g.c_tot, c_tot_before);
    assert_eq!(g.insurance, insurance_before);
}

#[test]
fn v15_recovery_mode_rejects_liquidation_and_rebalance_before_account_mutation() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    let account_before = a;
    let asset_before = g.assets[0];
    let reason = PermissionlessRecoveryReasonV15::BlockedSegmentHeadroomOrRepresentability;
    g.declare_permissionless_recovery(reason).unwrap();

    let liquidation = g.liquidate_account_not_atomic(
        &mut a,
        LiquidationRequestV15 {
            asset_index: 0,
            close_q: POS_SCALE,
            fee_bps: 0,
        },
        &[1; V15_MAX_PORTFOLIO_ASSETS_N],
    );
    assert_eq!(liquidation, Err(V15Error::LockActive));
    assert_eq!(a, account_before);
    assert_eq!(g.assets[0], asset_before);

    let rebalance = g.rebalance_reduce_position_not_atomic(
        &mut a,
        RebalanceRequestV15 {
            asset_index: 0,
            reduce_q: POS_SCALE,
        },
        &[1; V15_MAX_PORTFOLIO_ASSETS_N],
    );
    assert_eq!(rebalance, Err(V15Error::LockActive));
    assert_eq!(a, account_before);
    assert_eq!(g.assets[0], asset_before);
    assert_eq!(g.mode, MarketModeV15::Recovery);
    assert_eq!(g.recovery_reason, Some(reason));
}

#[test]
fn v15_recovery_mode_rejects_non_recovery_crank_before_account_mutation() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV15::Long, 1).unwrap();
    g.declare_permissionless_recovery(
        PermissionlessRecoveryReasonV15::BlockedSegmentHeadroomOrRepresentability,
    )
    .unwrap();
    let before = a;

    let res = g.permissionless_crank_not_atomic(
        &mut a,
        PermissionlessCrankRequestV15 {
            now_slot: 1,
            asset_index: 0,
            effective_price: 1,
            funding_rate_e9: 0,
            action: PermissionlessCrankActionV15::Refresh,
        },
        &[1; V15_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(res, Err(V15Error::LockActive));
    assert_eq!(a, before);
    assert_eq!(
        g.recovery_reason,
        Some(PermissionlessRecoveryReasonV15::BlockedSegmentHeadroomOrRepresentability)
    );
    assert_eq!(g.mode, MarketModeV15::Recovery);
}

#[test]
fn v15_permissionless_recovery_fails_closed_when_disabled() {
    let mut g = group();
    g.config.permissionless_recovery_enabled = false;

    assert_eq!(
        g.declare_permissionless_recovery(
            PermissionlessRecoveryReasonV15::BlockedSegmentHeadroomOrRepresentability
        ),
        Err(V15Error::InvalidConfig)
    );
    assert_eq!(g.recovery_reason, None);
    assert_eq!(g.mode, MarketModeV15::Live);
}

#[test]
fn v15_permissionless_crank_recovery_declaration_is_accounting_neutral() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.attach_leg(&mut a, 0, SideV15::Long, 1).unwrap();
    let account_before = a;
    let vault_before = g.vault;
    let c_tot_before = g.c_tot;
    let insurance_before = g.insurance;
    let pnl_pos_before = g.pnl_pos_tot;
    let asset_before = g.assets[0];
    let slot_last_before = g.slot_last;
    let current_slot_before = g.current_slot;
    let reason = PermissionlessRecoveryReasonV15::ExplicitLossOrDustAuditOverflow;

    let out = g
        .permissionless_crank_not_atomic(
            &mut a,
            PermissionlessCrankRequestV15 {
                now_slot: current_slot_before + 1,
                asset_index: 0,
                effective_price: 2,
                funding_rate_e9: 0,
                action: PermissionlessCrankActionV15::Recover(reason),
            },
            &[1; V15_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(
        out,
        PermissionlessProgressOutcomeV15::RecoveryDeclared(reason)
    );
    assert_eq!(g.recovery_reason, Some(reason));
    assert_eq!(a, account_before);
    assert_eq!(g.vault, vault_before);
    assert_eq!(g.c_tot, c_tot_before);
    assert_eq!(g.insurance, insurance_before);
    assert_eq!(g.pnl_pos_tot, pnl_pos_before);
    assert_eq!(g.assets[0], asset_before);
    assert_eq!(g.slot_last, slot_last_before);
    assert_eq!(g.current_slot, current_slot_before);
    assert_eq!(g.mode, MarketModeV15::Recovery);
}

#[test]
fn v15_fees_are_charged_only_after_realized_losses() {
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
fn v15_fee_sync_settles_hidden_kf_losses_before_collecting_fee() {
    let mut g = group();
    g.assets[0].effective_price = 100;
    g.assets[0].fund_px_last = 100;
    let mut long = account();
    g.deposit_not_atomic(&mut long, 50).unwrap();
    g.attach_leg(&mut long, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV15::Long, POS_SCALE, 92);

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
fn v15_fee_sync_uses_wide_product_and_drops_uncollectible_tail() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 1_000_000).unwrap();

    let charged = g
        .sync_account_fee_to_slot_not_atomic(&mut a, 2, u128::MAX)
        .unwrap();

    assert_eq!(charged, 1_000_000);
    assert_eq!(a.last_fee_slot, 2);
    assert_eq!(a.capital, 0);
    assert_eq!(
        a.fee_credits, 0,
        "uncollectible fee tail is dropped, not debt-socialized"
    );
    assert_eq!(g.insurance, 1_000_000);
    assert_eq!(g.c_tot, 0);
    assert_eq!(g.vault, 1_000_000);
    assert_eq!(g.assert_public_invariants(), Ok(()));
}

#[test]
fn v15_direct_fee_charge_is_live_only_but_resolved_fee_sync_still_works() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.resolve_market_not_atomic(10).unwrap();

    let before = (g, a);
    assert_eq!(
        g.charge_account_fee_not_atomic(&mut a, 10),
        Err(V15Error::LockActive)
    );
    assert_eq!((g, a), before);

    let synced = g
        .sync_account_fee_to_slot_not_atomic(&mut a, 10, 1)
        .unwrap();
    assert_eq!(synced, 10);
    assert_eq!(a.last_fee_slot, 10);
    assert_eq!(a.capital, 90);
    assert_eq!(g.insurance, 10);
    assert_eq!(g.assert_public_invariants(), Ok(()));
}

#[test]
fn v15_hlock_allows_principal_withdrawal_without_positive_credit_escape() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV15::Long, POS_SCALE, 93);
    g.threshold_stress_active = true;

    assert_eq!(
        g.withdraw_not_atomic(&mut a, 50, &[10; V15_MAX_PORTFOLIO_ASSETS_N]),
        Ok(())
    );
    assert_eq!(a.capital, 50);
    assert_eq!(g.vault, 50);
}

#[test]
fn v15_hlock_withdraw_rejects_if_post_state_needs_positive_pnl_credit() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 20).unwrap();
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    a.pnl = 100;
    g.pnl_pos_tot = 100;
    set_junior_bound(&mut g, 100);
    g.threshold_stress_active = true;

    assert_eq!(
        g.withdraw_not_atomic(&mut a, 10, &[50; V15_MAX_PORTFOLIO_ASSETS_N]),
        Err(V15Error::InvalidConfig)
    );
}

#[test]
fn v15_loss_stale_blocks_nonflat_withdrawal_even_if_no_positive_credit_suffices() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.loss_stale_active = true;

    assert_eq!(
        g.withdraw_not_atomic(&mut a, 10, &[10; V15_MAX_PORTFOLIO_ASSETS_N]),
        Err(V15Error::LockActive)
    );
}

#[test]
fn v15_target_effective_lag_blocks_risk_increasing_trade_before_mutation() {
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
        TradeRequestV15 {
            asset_index: 0,
            size_q: POS_SCALE,
            exec_price: 100,
            fee_bps: 0,
        },
        &[100; V15_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(res, Err(V15Error::LockActive));
    assert_eq!(long.active_bitmap, 0);
    assert_eq!(short.active_bitmap, 0);
}

#[test]
fn v15_target_effective_lag_allows_pure_risk_reducing_trade() {
    let mut g = group();
    let mut reducing_short = account();
    let mut reducing_long = account();
    reducing_long.provenance_header.portfolio_account_id = [4; 32];
    g.deposit_not_atomic(&mut reducing_short, 10_000).unwrap();
    g.deposit_not_atomic(&mut reducing_long, 10_000).unwrap();
    g.attach_leg(&mut reducing_short, 0, SideV15::Short, -(POS_SCALE as i128))
        .unwrap();
    g.attach_leg(&mut reducing_long, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.assets[0].effective_price = 100;
    g.assets[0].raw_oracle_target_price = 120;

    assert!(g
        .execute_trade_with_fee_not_atomic(
            &mut reducing_short,
            &mut reducing_long,
            TradeRequestV15 {
                asset_index: 0,
                size_q: POS_SCALE / 2,
                exec_price: 100,
                fee_bps: 0,
            },
            &[100; V15_MAX_PORTFOLIO_ASSETS_N],
        )
        .is_ok());
}

#[test]
fn v15_target_effective_lag_blocks_nonflat_withdrawal_and_pnl_conversion() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    a.pnl = 10;
    g.pnl_pos_tot = 10;
    set_junior_bound(&mut g, 10);
    g.vault = g.vault.checked_add(10).unwrap();
    g.assets[0].effective_price = 100;
    g.assets[0].raw_oracle_target_price = 120;
    g.full_account_refresh(&mut a, &[100; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(
        g.withdraw_not_atomic(&mut a, 1, &[100; V15_MAX_PORTFOLIO_ASSETS_N]),
        Err(V15Error::LockActive)
    );
    assert_eq!(
        g.convert_released_pnl_to_capital_not_atomic(&mut a),
        Err(V15Error::LockActive)
    );
}

#[test]
fn v15_account_free_equity_active_accrual_requires_protective_progress() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 1000).unwrap();
    let mut b = account_with_id(4);
    g.deposit_not_atomic(&mut b, 1000).unwrap();
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut b, 0, SideV15::Short, -(POS_SCALE as i128))
        .unwrap();

    assert_eq!(
        g.accrue_asset_to_not_atomic(0, 1, 2, 0, false),
        Err(V15Error::NonProgress)
    );
    assert!(g.accrue_asset_to_not_atomic(0, 1, 2, 0, true).is_ok());
}

#[test]
fn v15_equity_active_accrual_commits_one_bounded_loss_stale_segment() {
    let mut g = group();
    g.config.max_accrual_dt_slots = 2;
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV15::Long, POS_SCALE, 94);

    let out = g.accrue_asset_to_not_atomic(0, 10, 3, 0, true).unwrap();
    assert_eq!(out.dt, 2);
    assert!(out.loss_stale_after);
    assert_eq!(g.slot_last, 2);
    assert_eq!(g.current_slot, 10);
    assert!(g.loss_stale_active);
}

#[test]
fn v15_pending_domain_loss_barrier_does_not_freeze_asset_accrual() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV15::Long, POS_SCALE, 95);
    g.pending_domain_loss_barriers[0] = 1;

    let a_long_before = g.assets[0].a_long;
    let b_short_before = g.assets[0].b_short_num;
    let oi_long_before = g.assets[0].oi_eff_long_q;
    let out = g
        .accrue_asset_to_not_atomic(0, 1, 2, 0, true)
        .expect("close locks must not freeze asset-wide K/F/price/slot accrual");

    assert!(out.equity_active);
    assert_eq!(out.dt, 1);
    assert_eq!(g.assets[0].effective_price, 2);
    assert_eq!(g.assets[0].a_long, a_long_before);
    assert_eq!(g.assets[0].b_short_num, b_short_before);
    assert_eq!(g.assets[0].oi_eff_long_q, oi_long_before);
    assert_eq!(g.pending_domain_loss_barriers[0], 1);
}

#[test]
fn v15_pending_domain_loss_barrier_blocks_side_reset_before_residual_done() {
    let mut g = group();
    g.pending_domain_loss_barriers[0] = 1;
    g.assets[0].k_long = 7;
    g.assets[0].f_long_num = -3;
    g.assets[0].b_long_num = 11;
    g.assets[0].a_long = ADL_ONE - 1;
    g.assets[0].epoch_long = 4;

    let before = g;
    assert_eq!(
        g.begin_full_drain_reset(0, SideV15::Long),
        Err(V15Error::LockActive),
        "unbooked domain residual must block B/A/K/F/weight reset on that domain"
    );
    assert_eq!(g.assets[0].k_long, before.assets[0].k_long);
    assert_eq!(g.assets[0].f_long_num, before.assets[0].f_long_num);
    assert_eq!(g.assets[0].b_long_num, before.assets[0].b_long_num);
    assert_eq!(g.assets[0].a_long, before.assets[0].a_long);
    assert_eq!(g.assets[0].epoch_long, before.assets[0].epoch_long);
    assert_eq!(g.assets[0].mode_long, before.assets[0].mode_long);
    assert_eq!(g.pending_domain_loss_barriers[0], 1);
}

#[test]
fn v15_pending_domain_loss_barrier_does_not_block_unrelated_side_reset() {
    let mut g = group();
    g.pending_domain_loss_barriers[0] = 1;
    g.assets[0].k_long = 7;
    g.assets[0].f_long_num = -3;
    g.assets[0].b_long_num = 11;
    g.assets[0].a_long = ADL_ONE - 1;
    g.assets[0].k_short = -9;
    g.assets[0].f_short_num = 4;
    g.assets[0].b_short_num = 13;
    g.assets[0].a_short = ADL_ONE - 2;
    g.assets[0].epoch_short = 6;

    g.begin_full_drain_reset(0, SideV15::Short)
        .expect("pending long-domain residual must not freeze unrelated short-domain reset");
    assert_eq!(g.pending_domain_loss_barriers[0], 1);
    assert_eq!(g.assets[0].k_long, 7);
    assert_eq!(g.assets[0].f_long_num, -3);
    assert_eq!(g.assets[0].b_long_num, 11);
    assert_eq!(g.assets[0].a_long, ADL_ONE - 1);
    assert_eq!(g.assets[0].k_short, 0);
    assert_eq!(g.assets[0].f_short_num, 0);
    assert_eq!(g.assets[0].b_short_num, 0);
    assert_eq!(g.assets[0].a_short, ADL_ONE);
    assert_eq!(g.assets[0].epoch_short, 7);
    assert_eq!(g.assets[0].mode_short, SideModeV15::ResetPending);
}

#[test]
fn v15_per_asset_slot_last_prevents_cross_asset_accrual_aliasing() {
    let (market, _, _) = ids();
    let mut g = MarketGroupV15::new(market, V15Config::public_user_fund(2, 0, 10)).unwrap();
    let mut a0_long =
        PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, [31; 32], [3; 32]));
    let mut a0_short =
        PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, [32; 32], [3; 32]));
    let mut a1_long =
        PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, [33; 32], [3; 32]));
    let mut a1_short =
        PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, [34; 32], [3; 32]));
    g.attach_leg(&mut a0_long, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut a0_short, 0, SideV15::Short, -(POS_SCALE as i128))
        .unwrap();
    g.attach_leg(&mut a1_long, 1, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut a1_short, 1, SideV15::Short, -(POS_SCALE as i128))
        .unwrap();
    for i in 0..2 {
        g.assets[i].effective_price = 100;
        g.assets[i].fund_px_last = 100;
        g.assets[i].raw_oracle_target_price = 100;
    }

    let asset1_initial = g.assets[1];
    g.accrue_asset_to_not_atomic(0, 1, 101, 0, true).unwrap();
    let asset0_k = g.assets[0].k_long;
    let asset0_after_first = g.assets[0];
    let asset1_before = g.assets[1];
    assert_eq!(
        asset1_before, asset1_initial,
        "asset 0 accrual must not alias into asset 1"
    );
    g.accrue_asset_to_not_atomic(1, 1, 101, 0, true).unwrap();

    assert_eq!(
        g.assets[0], asset0_after_first,
        "asset 1 accrual must not alias back into asset 0"
    );
    assert_ne!(asset0_k, 0);
    assert_eq!(g.assets[0].slot_last, 1);
    assert_eq!(asset1_before.slot_last, 0);
    assert_eq!(g.assets[1].slot_last, 1);
    assert_ne!(g.assets[1].k_long, 0);
}

#[test]
fn v15_funding_rate_above_cap_rejects_before_state_mutation() {
    let mut g = group();
    g.config.max_abs_funding_e9_per_slot = 1;
    let before_asset = g.assets[0];

    let res = g.accrue_asset_to_not_atomic(0, 1, 1, 2, true);

    assert_eq!(res, Err(V15Error::InvalidConfig));
    assert_eq!(g.assets[0], before_asset);
    assert_eq!(g.slot_last, 0);
    assert_eq!(g.current_slot, 0);
}

#[test]
fn v15_trade_fee_is_dynamic_bounded_and_charged_inside_engine() {
    let mut g = group();
    g.config.max_trading_fee_bps = 100;
    let mut long = account();
    let mut short = account();
    short.provenance_header.portfolio_account_id = [4; 32];
    g.deposit_not_atomic(&mut long, 10_000).unwrap();
    g.deposit_not_atomic(&mut short, 10_000).unwrap();

    let req = TradeRequestV15 {
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
            &[1_000; V15_MAX_PORTFOLIO_ASSETS_N],
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
            &[1_000; V15_MAX_PORTFOLIO_ASSETS_N],
        ),
        Err(V15Error::InvalidConfig)
    );
}

#[test]
fn v15_trade_fee_conserves_vault_and_keeps_oi_symmetric() {
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
            TradeRequestV15 {
                asset_index: 0,
                size_q: POS_SCALE,
                exec_price: 100,
                fee_bps: 100,
            },
            &[100; V15_MAX_PORTFOLIO_ASSETS_N],
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
fn v15_risk_increasing_trade_requires_initial_health_after_refresh() {
    let mut g = group();
    let mut underfunded_long = account();
    let mut funded_short = account();
    funded_short.provenance_header.portfolio_account_id = [4; 32];
    g.deposit_not_atomic(&mut funded_short, 10_000).unwrap();

    let res = g.execute_trade_with_fee_not_atomic(
        &mut underfunded_long,
        &mut funded_short,
        TradeRequestV15 {
            asset_index: 0,
            size_q: POS_SCALE,
            exec_price: 100,
            fee_bps: 0,
        },
        &[100; V15_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(res, Err(V15Error::InvalidConfig));
    assert_eq!(underfunded_long.active_bitmap, 0);
    assert_eq!(g.assets[0].oi_eff_long_q, 0);
    assert_eq!(g.assets[0].oi_eff_short_q, 0);
}

#[test]
fn v15_trade_hint_cannot_hide_toxic_portfolio_leg_on_other_asset() {
    let mut g = group();
    let mut long = account();
    let mut short = account();
    short.provenance_header.portfolio_account_id = [4; 32];
    g.deposit_not_atomic(&mut long, 1).unwrap();
    g.deposit_not_atomic(&mut short, 1_000).unwrap();
    g.attach_leg(&mut long, 1, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.assets[1].k_long = -(3 * ADL_ONE as i128);
    let before_group = g;
    let before_long = long;
    let before_short = short;

    let res = g.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV15 {
            asset_index: 0,
            size_q: POS_SCALE,
            exec_price: 1,
            fee_bps: 0,
        },
        &[1; V15_MAX_PORTFOLIO_ASSETS_N],
    );

    assert!(
        res.is_err(),
        "risk-increasing trade on hinted asset must not ignore toxic active legs"
    );
    assert_eq!(g, before_group);
    assert_eq!(long, before_long);
    assert_eq!(short, before_short);
}

#[test]
fn v15_invalid_trade_request_rejects_before_any_mutation() {
    let mut g = group();
    let mut long = account();
    let mut short = account();
    short.provenance_header.portfolio_account_id = [4; 32];
    g.deposit_not_atomic(&mut long, 1_000).unwrap();
    g.deposit_not_atomic(&mut short, 1_000).unwrap();
    let before_group = g;
    let before_long = long;
    let before_short = short;

    let res = g.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV15 {
            asset_index: 0,
            size_q: 0,
            exec_price: 100,
            fee_bps: 0,
        },
        &[100; V15_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(res, Err(V15Error::InvalidConfig));
    assert_eq!(g, before_group);
    assert_eq!(long, before_long);
    assert_eq!(short, before_short);
}

#[test]
fn v15_sign_flip_trade_preserves_oi_symmetry_and_senior_accounting() {
    let mut g = group();
    let mut flip_to_long = account();
    let mut flip_to_short = account();
    flip_to_short.provenance_header.portfolio_account_id = [4; 32];
    g.deposit_not_atomic(&mut flip_to_long, 10_000).unwrap();
    g.deposit_not_atomic(&mut flip_to_short, 10_000).unwrap();
    g.attach_leg(&mut flip_to_long, 0, SideV15::Short, -(POS_SCALE as i128))
        .unwrap();
    g.attach_leg(&mut flip_to_short, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    let vault_before = g.vault;
    let c_tot_before = g.c_tot;

    g.execute_trade_with_fee_not_atomic(
        &mut flip_to_long,
        &mut flip_to_short,
        TradeRequestV15 {
            asset_index: 0,
            size_q: 2 * POS_SCALE,
            exec_price: 1,
            fee_bps: 0,
        },
        &[1; V15_MAX_PORTFOLIO_ASSETS_N],
    )
    .unwrap();

    assert_eq!(flip_to_long.legs[0].side, SideV15::Long);
    assert_eq!(flip_to_long.legs[0].basis_pos_q, POS_SCALE as i128);
    assert_eq!(flip_to_short.legs[0].side, SideV15::Short);
    assert_eq!(flip_to_short.legs[0].basis_pos_q, -(POS_SCALE as i128));
    assert_eq!(g.assets[0].oi_eff_long_q, POS_SCALE);
    assert_eq!(g.assets[0].oi_eff_short_q, POS_SCALE);
    assert_eq!(g.assets[0].stored_pos_count_long, 1);
    assert_eq!(g.assets[0].stored_pos_count_short, 1);
    assert_eq!(g.vault, vault_before);
    assert_eq!(g.c_tot, c_tot_before);
}

#[test]
fn v15_e2e_trade_mark_close_convert_withdraw_conserves() {
    let (market, _, owner) = ids();
    let mut g = group();
    let mut alice = account();
    let mut bob = PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, [4; 32], owner));
    let px1 = [1; V15_MAX_PORTFOLIO_ASSETS_N];
    let px2 = [2; V15_MAX_PORTFOLIO_ASSETS_N];

    g.deposit_not_atomic(&mut alice, 10_000).unwrap();
    g.deposit_not_atomic(&mut bob, 10_000).unwrap();
    let vault_after_deposit = g.vault;

    g.execute_trade_with_fee_not_atomic(
        &mut alice,
        &mut bob,
        TradeRequestV15 {
            asset_index: 0,
            size_q: POS_SCALE,
            exec_price: 1,
            fee_bps: 0,
        },
        &px1,
    )
    .unwrap();
    assert_eq!(g.assets[0].oi_eff_long_q, POS_SCALE);
    assert_eq!(g.assets[0].oi_eff_short_q, POS_SCALE);

    g.permissionless_crank_not_atomic(
        &mut alice,
        PermissionlessCrankRequestV15 {
            now_slot: 1,
            asset_index: 0,
            effective_price: 2,
            funding_rate_e9: 0,
            action: PermissionlessCrankActionV15::Refresh,
        },
        &px2,
    )
    .unwrap();
    g.full_account_refresh(&mut alice, &px2).unwrap();
    g.full_account_refresh(&mut bob, &px2).unwrap();
    assert!(
        alice.pnl > 0,
        "long should have mark profit after price increase"
    );
    assert!(
        bob.pnl < 0,
        "short should have mark loss after price increase"
    );

    g.execute_trade_with_fee_not_atomic(
        &mut bob,
        &mut alice,
        TradeRequestV15 {
            asset_index: 0,
            size_q: POS_SCALE,
            exec_price: 2,
            fee_bps: 0,
        },
        &px2,
    )
    .unwrap();
    assert_eq!(alice.active_bitmap, 0);
    assert_eq!(bob.active_bitmap, 0);
    assert_eq!(g.assets[0].oi_eff_long_q, 0);
    assert_eq!(g.assets[0].oi_eff_short_q, 0);

    let converted = g
        .convert_released_pnl_to_capital_not_atomic(&mut alice)
        .unwrap();
    assert_eq!(converted, 1);
    assert_eq!(alice.pnl, 0);
    assert_eq!(g.pnl_pos_tot, 0);

    g.withdraw_not_atomic(&mut alice, 100, &px2).unwrap();
    assert_eq!(g.assert_public_invariants(), Ok(()));
    assert_eq!(g.c_tot, alice.capital + bob.capital);
    assert_eq!(g.vault, vault_after_deposit - 100);
}

#[test]
fn v15_price_accrual_then_refresh_matches_eager_mark_pnl() {
    let mut g = group();
    g.assets[0].effective_price = 100;
    g.assets[0].fund_px_last = 100;
    g.assets[0].raw_oracle_target_price = 100;
    let mut long = account();
    let mut short = account();
    short.provenance_header.portfolio_account_id = [4; 32];

    g.attach_leg(&mut long, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut short, 0, SideV15::Short, -(POS_SCALE as i128))
        .unwrap();
    let out = g.accrue_asset_to_not_atomic(0, 1, 101, 0, true).unwrap();
    assert!(out.price_move_active);

    g.full_account_refresh(&mut long, &[101; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    g.full_account_refresh(&mut short, &[101; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(long.pnl, 1);
    assert_eq!(short.pnl, -1);
    assert_eq!(g.pnl_pos_tot, 1);
    assert_eq!(g.negative_pnl_account_count, 1);
}

#[test]
fn v15_same_epoch_full_refresh_is_idempotent_after_kf_settlement() {
    let mut g = group();
    g.assets[0].effective_price = 100;
    g.assets[0].fund_px_last = 100;
    g.assets[0].raw_oracle_target_price = 100;
    let mut a = account();

    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV15::Long, POS_SCALE, 96);
    g.accrue_asset_to_not_atomic(0, 1, 101, 0, true).unwrap();
    g.full_account_refresh(&mut a, &[101; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    let account_after_first = a;
    let group_after_first = g;

    g.full_account_refresh(&mut a, &[101; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(a, account_after_first);
    assert_eq!(g, group_after_first);
}

#[test]
fn v15_sequential_kf_refresh_is_additive_not_compounding() {
    let mut sequential = group();
    sequential.assets[0].effective_price = 100;
    sequential.assets[0].fund_px_last = 100;
    sequential.assets[0].raw_oracle_target_price = 100;
    let mut seq_account = account();
    sequential
        .attach_leg(&mut seq_account, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    let _seq_opposite = attach_opposite(&mut sequential, 0, SideV15::Long, POS_SCALE, 97);

    sequential
        .accrue_asset_to_not_atomic(0, 1, 101, 0, true)
        .unwrap();
    sequential
        .full_account_refresh(&mut seq_account, &[101; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(seq_account.pnl, 1);

    sequential
        .accrue_asset_to_not_atomic(0, 2, 102, 0, true)
        .unwrap();
    sequential
        .full_account_refresh(&mut seq_account, &[102; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    let mut direct = group();
    direct.assets[0].effective_price = 100;
    direct.assets[0].fund_px_last = 100;
    direct.assets[0].raw_oracle_target_price = 100;
    let mut direct_account = account();
    direct
        .attach_leg(&mut direct_account, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    let _direct_opposite = attach_opposite(&mut direct, 0, SideV15::Long, POS_SCALE, 98);

    direct
        .accrue_asset_to_not_atomic(0, 1, 102, 0, true)
        .unwrap();
    direct
        .full_account_refresh(&mut direct_account, &[102; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(seq_account.pnl, 2);
    assert_eq!(direct_account.pnl, 2);
    assert_eq!(seq_account.pnl, direct_account.pnl);
    assert_eq!(sequential.pnl_pos_tot, direct.pnl_pos_tot);
}

#[test]
fn v15_funding_accrual_then_refresh_matches_sign_and_floor() {
    let (market, _, _) = ids();
    let mut cfg = V15Config::public_user_fund(4, 0, 10);
    cfg.max_price_move_bps_per_slot = 4_999;
    cfg.max_abs_funding_e9_per_slot = 1;
    let mut g = MarketGroupV15::new(market, cfg).unwrap();
    g.assets[0].effective_price = 1_000_000_000;
    g.assets[0].fund_px_last = 1_000_000_000;
    g.assets[0].raw_oracle_target_price = 1_000_000_000;
    let mut long = account();
    let mut short = account();
    short.provenance_header.portfolio_account_id = [4; 32];

    g.attach_leg(&mut long, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut short, 0, SideV15::Short, -(POS_SCALE as i128))
        .unwrap();
    let out = g
        .accrue_asset_to_not_atomic(0, 1, 1_000_000_000, 1, true)
        .unwrap();
    assert!(out.funding_active);

    g.full_account_refresh(&mut long, &[1_000_000_000; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    g.full_account_refresh(&mut short, &[1_000_000_000; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(long.pnl, -1);
    assert_eq!(short.pnl, 1);
}

#[test]
fn v15_funding_accrual_requires_bilateral_exposure() {
    let (market, _, _) = ids();
    let mut cfg = V15Config::public_user_fund(4, 0, 10);
    cfg.max_price_move_bps_per_slot = 9_999;
    cfg.max_abs_funding_e9_per_slot = 1;
    let mut no_oi = MarketGroupV15::new(market, cfg).unwrap();
    no_oi.assets[0].effective_price = 1_000_000_000;
    no_oi.assets[0].fund_px_last = 1_000_000_000;
    no_oi.assets[0].raw_oracle_target_price = 1_000_000_000;
    let no_oi_before = no_oi.assets[0];
    let out = no_oi
        .accrue_asset_to_not_atomic(0, 1, 1_000_000_000, 1, false)
        .unwrap();
    assert!(!out.funding_active);
    assert_eq!(no_oi.assets[0].f_long_num, no_oi_before.f_long_num);
    assert_eq!(no_oi.assets[0].f_short_num, no_oi_before.f_short_num);
    assert_eq!(no_oi.funding_epoch, 0);

    let mut one_sided = MarketGroupV15::new(market, cfg).unwrap();
    one_sided.assets[0].effective_price = 1_000_000_000;
    one_sided.assets[0].fund_px_last = 1_000_000_000;
    one_sided.assets[0].raw_oracle_target_price = 1_000_000_000;
    let mut long = account();
    one_sided
        .attach_leg(&mut long, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    let one_sided_before = one_sided.assets[0];
    let out = one_sided
        .accrue_asset_to_not_atomic(0, 1, 1_000_000_000, 1, false)
        .unwrap_err();
    assert_eq!(out, V15Error::InvalidConfig);
    assert_eq!(one_sided.assets[0].f_long_num, one_sided_before.f_long_num);
    assert_eq!(
        one_sided.assets[0].f_short_num,
        one_sided_before.f_short_num
    );
    assert_eq!(one_sided.funding_epoch, 0);

    let mut short_only = MarketGroupV15::new(market, cfg).unwrap();
    short_only.assets[0].effective_price = 1_000_000_000;
    short_only.assets[0].fund_px_last = 1_000_000_000;
    short_only.assets[0].raw_oracle_target_price = 1_000_000_000;
    let mut short = account();
    short.provenance_header.portfolio_account_id = [5; 32];
    short_only
        .attach_leg(&mut short, 0, SideV15::Short, -(POS_SCALE as i128))
        .unwrap();
    let short_only_before = short_only.assets[0];
    let out = short_only
        .accrue_asset_to_not_atomic(0, 1, 1_000_000_000, 1, false)
        .unwrap_err();
    assert_eq!(out, V15Error::InvalidConfig);
    assert_eq!(
        short_only.assets[0].f_long_num,
        short_only_before.f_long_num
    );
    assert_eq!(
        short_only.assets[0].f_short_num,
        short_only_before.f_short_num
    );
    assert_eq!(short_only.funding_epoch, 0);
}

#[test]
fn v15_permissionless_crank_accepts_configured_funding_rate_boundaries() {
    let (market, _, _) = ids();
    let mut cfg = V15Config::public_user_fund(4, 0, 10);
    cfg.max_price_move_bps_per_slot = 9_999;
    cfg.max_abs_funding_e9_per_slot = 1;
    let mut positive = MarketGroupV15::new(market, cfg).unwrap();
    let mut positive_account = account();
    let req = PermissionlessCrankRequestV15 {
        now_slot: 1,
        asset_index: 0,
        effective_price: 1,
        funding_rate_e9: 1,
        action: PermissionlessCrankActionV15::Refresh,
    };
    assert_eq!(
        positive.permissionless_crank_not_atomic(
            &mut positive_account,
            req,
            &[1; V15_MAX_PORTFOLIO_ASSETS_N]
        ),
        Ok(PermissionlessProgressOutcomeV15::AccountCurrent)
    );

    let mut negative = MarketGroupV15::new(market, cfg).unwrap();
    let mut negative_account = account();
    let negative_req = PermissionlessCrankRequestV15 {
        funding_rate_e9: -1,
        ..req
    };
    assert_eq!(
        negative.permissionless_crank_not_atomic(
            &mut negative_account,
            negative_req,
            &[1; V15_MAX_PORTFOLIO_ASSETS_N]
        ),
        Ok(PermissionlessProgressOutcomeV15::AccountCurrent)
    );
}

#[test]
fn v15_funding_accrual_uses_only_bounded_segment_dt() {
    let (market, _, _) = ids();
    let mut cfg = V15Config::public_user_fund(4, 0, 10);
    cfg.max_price_move_bps_per_slot = 4_999;
    cfg.max_abs_funding_e9_per_slot = 1;
    cfg.max_accrual_dt_slots = 2;
    cfg.min_funding_lifetime_slots = 2;
    let mut g = MarketGroupV15::new(market, cfg).unwrap();
    g.assets[0].effective_price = 1_000_000_000;
    g.assets[0].fund_px_last = 1_000_000_000;
    g.assets[0].raw_oracle_target_price = 1_000_000_000;
    let mut long = account();
    let mut short = account();
    short.provenance_header.portfolio_account_id = [4; 32];
    g.attach_leg(&mut long, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut short, 0, SideV15::Short, -(POS_SCALE as i128))
        .unwrap();

    let out = g
        .accrue_asset_to_not_atomic(0, 10, 1_000_000_000, 1, true)
        .unwrap();
    assert!(out.funding_active);
    assert_eq!(out.dt, 2);
    assert!(out.loss_stale_after);
    assert_eq!(g.slot_last, 2);
    assert_eq!(g.current_slot, 10);
    assert_eq!(g.assets[0].f_long_num, -2 * ADL_ONE as i128);
    assert_eq!(g.assets[0].f_short_num, 2 * ADL_ONE as i128);
}

#[test]
fn v15_combined_price_and_funding_accrual_keeps_k_and_f_separate() {
    let (market, _, _) = ids();
    let mut cfg = V15Config::public_user_fund(4, 0, 10);
    cfg.max_price_move_bps_per_slot = 9_999;
    cfg.max_abs_funding_e9_per_slot = 1;
    let mut g = MarketGroupV15::new(market, cfg).unwrap();
    g.assets[0].effective_price = 999_999_999;
    g.assets[0].fund_px_last = 999_999_999;
    g.assets[0].raw_oracle_target_price = 999_999_999;
    let mut long = account();
    let mut short = account();
    short.provenance_header.portfolio_account_id = [4; 32];
    g.attach_leg(&mut long, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut short, 0, SideV15::Short, -(POS_SCALE as i128))
        .unwrap();

    let out = g
        .accrue_asset_to_not_atomic(0, 1, 1_000_000_000, 1, true)
        .unwrap();

    assert!(out.price_move_active);
    assert!(out.funding_active);
    assert_eq!(g.assets[0].k_long, ADL_ONE as i128);
    assert_eq!(g.assets[0].k_short, -(ADL_ONE as i128));
    assert_eq!(g.assets[0].f_long_num, -(ADL_ONE as i128));
    assert_eq!(g.assets[0].f_short_num, ADL_ONE as i128);
    assert_eq!(g.assets[0].fund_px_last, 1_000_000_000);
}

#[test]
fn v15_zero_funding_rate_advances_time_without_f_mutation() {
    let mut g = group();
    g.assets[0].effective_price = 100;
    g.assets[0].fund_px_last = 100;
    g.assets[0].raw_oracle_target_price = 100;
    let mut long = account();
    let mut short = account();
    short.provenance_header.portfolio_account_id = [4; 32];
    g.attach_leg(&mut long, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut short, 0, SideV15::Short, -(POS_SCALE as i128))
        .unwrap();
    let before = g.assets[0];

    let out = g.accrue_asset_to_not_atomic(0, 1, 100, 0, true).unwrap();

    assert!(!out.funding_active);
    assert_eq!(g.assets[0].f_long_num, before.f_long_num);
    assert_eq!(g.assets[0].f_short_num, before.f_short_num);
    assert_eq!(g.funding_epoch, 0);
    assert_eq!(g.slot_last, 1);
    assert_eq!(g.current_slot, 1);
}

#[test]
fn v15_same_slot_exposed_price_move_rejects_without_mutation() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    let before = g;

    assert_eq!(
        g.accrue_asset_to_not_atomic(0, 0, 2, 0, true),
        Err(V15Error::NonProgress)
    );
    assert_eq!(g, before);
}

#[test]
fn v15_hlock_allows_risk_increasing_trade_with_no_positive_credit_margin() {
    let mut g = group();
    let mut long = account();
    let mut short = account();
    short.provenance_header.portfolio_account_id = [4; 32];
    g.deposit_not_atomic(&mut long, 10_000).unwrap();
    g.deposit_not_atomic(&mut short, 10_000).unwrap();
    g.threshold_stress_active = true;

    let out = g
        .execute_trade_with_fee_not_atomic(
            &mut long,
            &mut short,
            TradeRequestV15 {
                asset_index: 0,
                size_q: POS_SCALE,
                exec_price: 100,
                fee_bps: 0,
            },
            &[100; V15_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(out.notional, 100);
    assert_eq!(long.legs[0].basis_pos_q, POS_SCALE as i128);
    assert_eq!(short.legs[0].basis_pos_q, -(POS_SCALE as i128));
    assert_eq!(g.assets[0].oi_eff_long_q, POS_SCALE);
    assert_eq!(g.assets[0].oi_eff_short_q, POS_SCALE);
    assert_eq!(g.insurance, 0);
}

#[test]
fn v15_loss_stale_blocks_risk_increasing_trade_even_with_no_positive_credit_margin() {
    let mut g = group();
    let mut long = account();
    let mut short = account();
    short.provenance_header.portfolio_account_id = [4; 32];
    g.deposit_not_atomic(&mut long, 10_000).unwrap();
    g.deposit_not_atomic(&mut short, 10_000).unwrap();
    g.loss_stale_active = true;

    let before = (g, long, short);
    let res = g.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV15 {
            asset_index: 0,
            size_q: POS_SCALE,
            exec_price: 100,
            fee_bps: 0,
        },
        &[100; V15_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(res, Err(V15Error::LockActive));
    assert_eq!((g, long, short), before);
}

#[test]
fn v15_hlock_rejects_risk_increasing_trade_that_needs_positive_pnl_credit() {
    let mut g = group();
    let mut long = account();
    let mut short = account();
    short.provenance_header.portfolio_account_id = [4; 32];
    long.pnl = 200;
    short.pnl = 200;
    g.pnl_pos_tot = 400;
    set_junior_bound(&mut g, 400);
    g.vault = 400;
    g.threshold_stress_active = true;

    let before = (g, long, short);
    let res = g.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV15 {
            asset_index: 0,
            size_q: POS_SCALE,
            exec_price: 100,
            fee_bps: 0,
        },
        &[100; V15_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(res, Err(V15Error::LockActive));
    assert_eq!((g, long, short), before);
}

#[test]
fn v15_hlock_allows_pure_risk_reducing_trade_with_no_positive_credit_margin() {
    let mut g = group();
    let mut reducing_short = account();
    let mut reducing_long = account();
    reducing_long.provenance_header.portfolio_account_id = [4; 32];
    g.deposit_not_atomic(&mut reducing_short, 10_000).unwrap();
    g.deposit_not_atomic(&mut reducing_long, 10_000).unwrap();
    g.attach_leg(&mut reducing_short, 0, SideV15::Short, -(POS_SCALE as i128))
        .unwrap();
    g.attach_leg(&mut reducing_long, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.threshold_stress_active = true;

    let out = g
        .execute_trade_with_fee_not_atomic(
            &mut reducing_short,
            &mut reducing_long,
            TradeRequestV15 {
                asset_index: 0,
                size_q: POS_SCALE / 2,
                exec_price: 100,
                fee_bps: 0,
            },
            &[100; V15_MAX_PORTFOLIO_ASSETS_N],
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
fn v15_hlock_rejects_reducing_trade_that_needs_positive_pnl_credit() {
    let mut g = group();
    let mut weak_short = account();
    let mut strong_long = account();
    strong_long.provenance_header.portfolio_account_id = [4; 32];
    weak_short.pnl = 100;
    g.pnl_pos_tot = 100;
    set_junior_bound(&mut g, 100);
    g.deposit_not_atomic(&mut strong_long, 10_000).unwrap();
    g.vault = g.c_tot + g.insurance + 100;
    g.attach_leg(&mut weak_short, 0, SideV15::Short, -(POS_SCALE as i128))
        .unwrap();
    g.attach_leg(&mut strong_long, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.threshold_stress_active = true;

    let res = g.execute_trade_with_fee_not_atomic(
        &mut weak_short,
        &mut strong_long,
        TradeRequestV15 {
            asset_index: 0,
            size_q: POS_SCALE / 2,
            exec_price: 100,
            fee_bps: 0,
        },
        &[100; V15_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(res, Err(V15Error::LockActive));
}

#[test]
fn v15_released_pnl_conversion_burns_face_claim_under_global_impairment() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 10).unwrap();
    a.pnl = 50;
    g.pnl_pos_tot = 50;
    set_junior_bound(&mut g, 50);
    g.pnl_matured_pos_tot = 50;
    g.vault = g.c_tot + 7;
    g.full_account_refresh(&mut a, &[1; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    let converted = g
        .convert_released_pnl_to_capital_not_atomic(&mut a)
        .unwrap();

    assert_eq!(converted, 7);
    assert_eq!(g.vault, 17);
    assert_eq!(g.c_tot, 17);
    assert_eq!(a.capital, 17);
    assert_eq!(a.pnl, 0);
    assert_eq!(g.pnl_pos_tot, 0);
    assert_eq!(g.pnl_pos_bound_tot, 0);
}

#[test]
fn v15_loss_stale_allows_pure_risk_reducing_trade_path() {
    let mut g = group();
    let mut reducing_short = account();
    let mut reducing_long = account();
    reducing_long.provenance_header.portfolio_account_id = [4; 32];
    g.deposit_not_atomic(&mut reducing_short, 10_000).unwrap();
    g.deposit_not_atomic(&mut reducing_long, 10_000).unwrap();
    g.attach_leg(&mut reducing_short, 0, SideV15::Short, -(POS_SCALE as i128))
        .unwrap();
    g.attach_leg(&mut reducing_long, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.loss_stale_active = true;

    assert!(g
        .execute_trade_with_fee_not_atomic(
            &mut reducing_short,
            &mut reducing_long,
            TradeRequestV15 {
                asset_index: 0,
                size_q: POS_SCALE / 2,
                exec_price: 100,
                fee_bps: 0,
            },
            &[100; V15_MAX_PORTFOLIO_ASSETS_N],
        )
        .is_ok());
}

#[test]
fn v15_b_residual_booking_is_bounded_and_remainder_conserving() {
    let mut g = group();
    let mut short = account();
    g.deposit_not_atomic(&mut short, 100).unwrap();
    g.attach_leg(&mut short, 0, SideV15::Short, -(POS_SCALE as i128))
        .unwrap();
    let mut bankrupt = account();

    let out = g
        .book_bankruptcy_residual_chunk_for_account(&mut bankrupt, 0, SideV15::Long, 7)
        .unwrap();
    assert_eq!(out.booked_loss, 7);
    assert!(out.delta_b > 0);
    assert_eq!(bankrupt.close_progress.b_loss_booked, 7);
    assert_eq!(bankrupt.close_progress.residual_remaining, 0);
    assert!(bankrupt.close_progress.finalized);

    g.mark_leg_b_stale(&mut short, 0).unwrap();
    let chunk = g
        .settle_account_b_chunk(&mut short, 0, g.assets[0].b_short_num)
        .unwrap();
    assert_eq!(chunk.remaining_after, 0);
    assert!(short.pnl <= -7);
}

#[test]
fn v15_zero_weight_domain_residual_cannot_clear_without_backing() {
    let mut g = group();
    let mut bankrupt = account();

    assert_eq!(
        g.book_bankruptcy_residual_chunk_for_account(&mut bankrupt, 0, SideV15::Long, 1),
        Err(V15Error::RecoveryRequired)
    );
    assert_eq!(
        g.recovery_reason,
        Some(PermissionlessRecoveryReasonV15::ActiveBankruptCloseCannotProgress)
    );
    assert_eq!(g.assets[0].explicit_unallocated_loss_short, 0);
    assert!(!bankrupt.close_progress.active);
    assert_eq!(
        g.pending_domain_loss_barrier_count(0, SideV15::Short),
        Ok(0)
    );
}

#[test]
fn v15_pending_close_progress_blocks_domain_escape_until_finalized() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    a.close_progress = CloseProgressLedgerV15 {
        active: true,
        finalized: false,
        close_id: 1,
        asset_index: 0,
        domain_side: SideV15::Short,
        gross_loss_at_close_start: 10,
        drift_reference_slot: g.current_slot,
        max_close_slot: g.current_slot + 1,
        residual_remaining: 10,
        ..CloseProgressLedgerV15::EMPTY
    };

    assert_eq!(g.clear_leg(&mut a, 0), Err(V15Error::LockActive));
    assert_eq!(g.h_lock_lane(Some(&a), false), Ok(HLockLaneV15::HMax));
}

#[test]
fn v15_new_close_cannot_overwrite_active_finalized_close_ledger() {
    let mut g = group();
    let mut bankrupt = account();
    let mut opposing =
        PortfolioAccountV15::empty(ProvenanceHeaderV15::new([1; 32], [42; 32], [3; 32]));
    g.attach_leg(&mut bankrupt, 1, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut opposing, 1, SideV15::Short, -(POS_SCALE as i128))
        .unwrap();
    bankrupt.close_progress = CloseProgressLedgerV15 {
        active: true,
        finalized: true,
        close_id: 7,
        asset_index: 0,
        domain_side: SideV15::Short,
        gross_loss_at_close_start: 2,
        b_loss_booked: 2,
        residual_remaining: 0,
        drift_reference_slot: g.current_slot,
        max_close_slot: g.current_slot + 1,
        ..CloseProgressLedgerV15::EMPTY
    };
    g.assets[1].k_long = -(100 * ADL_ONE as i128);
    let before_ledger = bankrupt.close_progress;
    let before_b_short = g.assets[1].b_short_num;

    assert_eq!(
        g.liquidate_account_not_atomic(
            &mut bankrupt,
            LiquidationRequestV15 {
                asset_index: 1,
                close_q: POS_SCALE,
                fee_bps: 0,
            },
            &[1; V15_MAX_PORTFOLIO_ASSETS_N],
        ),
        Err(V15Error::LockActive)
    );
    assert_eq!(bankrupt.close_progress, before_ledger);
    assert_eq!(g.assets[1].b_short_num, before_b_short);
}

#[test]
fn v15_pending_domain_loss_barrier_blocks_other_participants_until_residual_done() {
    let (market, _, owner) = ids();
    let mut cfg = V15Config::public_user_fund(1, 0, 10);
    cfg.public_b_chunk_atoms = 1;
    let mut g = MarketGroupV15::new(market, cfg).unwrap();
    let mut bankrupt = account();
    let mut participant =
        PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, [4; 32], owner));
    let mut joiner = PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, [5; 32], owner));

    g.attach_leg(&mut participant, 0, SideV15::Short, -10)
        .unwrap();
    let first = g
        .book_bankruptcy_residual_chunk_for_account(&mut bankrupt, 0, SideV15::Long, 2)
        .unwrap();
    assert_eq!(first.booked_loss, 1);
    assert_eq!(
        g.pending_domain_loss_barrier_count(0, SideV15::Short),
        Ok(1)
    );
    assert_eq!(g.clear_leg(&mut participant, 0), Err(V15Error::LockActive));
    assert_eq!(
        g.attach_leg(&mut joiner, 0, SideV15::Short, -1),
        Err(V15Error::LockActive)
    );

    let second = g
        .book_bankruptcy_residual_chunk_for_account(&mut bankrupt, 0, SideV15::Long, 2)
        .unwrap();
    assert_eq!(second.booked_loss, 1);
    assert!(bankrupt.close_progress.finalized);
    assert_eq!(
        g.pending_domain_loss_barrier_count(0, SideV15::Short),
        Ok(0)
    );
    g.clear_leg(&mut participant, 0).unwrap();
}

#[test]
fn v15_pending_domain_loss_barrier_blocks_trade_weight_escape_before_fee_or_position_mutation() {
    let (market, _, owner) = ids();
    let mut cfg = V15Config::public_user_fund(1, 0, 10);
    cfg.public_b_chunk_atoms = 1;
    cfg.max_trading_fee_bps = 10;
    let mut g = MarketGroupV15::new(market, cfg).unwrap();
    let mut participant =
        PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, [4; 32], owner));
    let mut counterparty =
        PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, [5; 32], owner));

    g.deposit_not_atomic(&mut participant, 1_000).unwrap();
    g.deposit_not_atomic(&mut counterparty, 1_000).unwrap();
    g.attach_leg(&mut participant, 0, SideV15::Short, -(POS_SCALE as i128))
        .unwrap();
    g.attach_leg(&mut counterparty, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.pending_domain_loss_barriers[1] = 1;
    assert_eq!(
        g.pending_domain_loss_barrier_count(0, SideV15::Short),
        Ok(1)
    );

    let before_group = g;
    let before_participant = participant;
    let before_counterparty = counterparty;
    let res = g.execute_trade_with_fee_not_atomic(
        &mut participant,
        &mut counterparty,
        TradeRequestV15 {
            asset_index: 0,
            size_q: POS_SCALE / 2,
            exec_price: 100,
            fee_bps: 10,
        },
        &[100; V15_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(res, Err(V15Error::LockActive));
    assert_eq!(g, before_group);
    assert_eq!(participant, before_participant);
    assert_eq!(counterparty, before_counterparty);
}

#[test]
fn v15_pending_domain_loss_barrier_blocks_rebalance_weight_escape_before_position_mutation() {
    let (market, _, owner) = ids();
    let mut g = MarketGroupV15::new(market, V15Config::public_user_fund(1, 0, 10)).unwrap();
    let mut participant =
        PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, [4; 32], owner));

    g.deposit_not_atomic(&mut participant, 1_000).unwrap();
    g.attach_leg(&mut participant, 0, SideV15::Short, -(POS_SCALE as i128))
        .unwrap();
    g.full_account_refresh(&mut participant, &[100; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    g.pending_domain_loss_barriers[1] = 1;

    let before_group = g;
    let before_participant = participant;
    let res = g.rebalance_reduce_position_not_atomic(
        &mut participant,
        RebalanceRequestV15 {
            asset_index: 0,
            reduce_q: POS_SCALE / 2,
        },
        &[100; V15_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(res, Err(V15Error::LockActive));
    assert_eq!(g, before_group);
    assert_eq!(participant, before_participant);
}

#[test]
fn v15_expired_close_progress_routes_recovery_before_b_booking() {
    let mut g = group();
    let mut participant = account();
    let mut bankrupt = account();
    g.attach_leg(&mut participant, 0, SideV15::Short, -10)
        .unwrap();
    bankrupt.close_progress = CloseProgressLedgerV15 {
        active: true,
        finalized: false,
        close_id: 1,
        asset_index: 0,
        domain_side: SideV15::Short,
        gross_loss_at_close_start: 2,
        drift_reference_slot: 0,
        max_close_slot: 1,
        residual_remaining: 2,
        ..CloseProgressLedgerV15::EMPTY
    };
    g.current_slot = 2;
    let b_before = g.assets[0].b_short_num;

    assert_eq!(
        g.book_bankruptcy_residual_chunk_for_account(&mut bankrupt, 0, SideV15::Long, 2),
        Err(V15Error::RecoveryRequired)
    );
    assert_eq!(
        g.recovery_reason,
        Some(PermissionlessRecoveryReasonV15::ActiveBankruptCloseCannotProgress)
    );
    assert_eq!(g.assets[0].b_short_num, b_before);
    assert_eq!(bankrupt.close_progress.b_loss_booked, 0);
    assert_eq!(bankrupt.close_progress.residual_remaining, 2);
    assert_eq!(
        g.pending_domain_loss_barrier_count(0, SideV15::Short),
        Ok(0)
    );
}

#[test]
fn v15_close_progress_uses_configured_lifetime_and_does_not_refresh_on_continuation() {
    let (market, _, owner) = ids();
    let mut cfg = V15Config::public_user_fund(1, 0, 10);
    cfg.max_bankrupt_close_chunks = 7;
    cfg.max_bankrupt_close_lifetime_slots = 5;
    cfg.public_b_chunk_atoms = 1;
    let mut g = MarketGroupV15::new(market, cfg).unwrap();
    g.current_slot = 11;
    let mut bankrupt = PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, [9; 32], owner));
    let mut participant =
        PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, [4; 32], owner));
    g.attach_leg(&mut participant, 0, SideV15::Short, -10)
        .unwrap();

    let first = g
        .book_bankruptcy_residual_chunk_for_account(&mut bankrupt, 0, SideV15::Long, 2)
        .unwrap();
    assert_eq!(first.booked_loss, 1);
    let first_ledger = bankrupt.close_progress;
    assert!(first_ledger.active);
    assert!(!first_ledger.finalized);
    assert_eq!(first_ledger.drift_reference_slot, 11);
    assert_eq!(first_ledger.max_close_slot, 16);
    assert_ne!(
        first_ledger.max_close_slot,
        11 + cfg.max_accrual_dt_slots * cfg.max_bankrupt_close_chunks
    );

    g.current_slot = 12;
    let second = g
        .book_bankruptcy_residual_chunk_for_account(&mut bankrupt, 0, SideV15::Long, 2)
        .unwrap();
    assert_eq!(second.booked_loss, 1);
    assert!(bankrupt.close_progress.finalized);
    assert_eq!(
        bankrupt.close_progress.drift_reference_slot,
        first_ledger.drift_reference_slot
    );
    assert_eq!(
        bankrupt.close_progress.max_close_slot,
        first_ledger.max_close_slot
    );
}

#[test]
fn v15_expired_close_progress_routes_recovery_before_quantity_adl() {
    let mut g = group();
    let mut closing = account();
    let mut opposing =
        PortfolioAccountV15::empty(ProvenanceHeaderV15::new([1; 32], [12; 32], [3; 32]));
    g.attach_leg(&mut closing, 0, SideV15::Long, 4).unwrap();
    g.attach_leg(&mut opposing, 0, SideV15::Short, -4).unwrap();
    closing.close_progress = CloseProgressLedgerV15 {
        active: true,
        finalized: true,
        close_id: 1,
        asset_index: 0,
        domain_side: SideV15::Short,
        gross_loss_at_close_start: 1,
        explicit_loss_assigned: 1,
        drift_reference_slot: 0,
        max_close_slot: 1,
        residual_remaining: 0,
        ..CloseProgressLedgerV15::EMPTY
    };
    g.assets[0].a_short = ADL_ONE;
    g.current_slot = 2;

    assert_eq!(
        g.apply_quantity_adl_after_residual_for_account_not_atomic(
            &mut closing,
            0,
            SideV15::Long,
            4
        ),
        Err(V15Error::RecoveryRequired)
    );
    assert_eq!(
        g.recovery_reason,
        Some(PermissionlessRecoveryReasonV15::ActiveBankruptCloseCannotProgress)
    );
    assert_eq!(closing.close_progress.quantity_adl_applied_q, 0);
    assert_eq!(g.assets[0].oi_eff_long_q, 4);
    assert_eq!(g.assets[0].oi_eff_short_q, 4);
    assert_eq!(g.assets[0].a_short, ADL_ONE);
}

#[test]
fn v15_stale_active_close_residual_routes_recovery_before_b_booking() {
    let (market, _, owner) = ids();
    let mut cfg = V15Config::public_user_fund(1, 0, 10);
    cfg.public_b_chunk_atoms = 1;
    let mut g = MarketGroupV15::new(market, cfg).unwrap();
    let mut closing = account();
    let mut opposing = PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, [4; 32], owner));
    g.attach_leg(&mut closing, 0, SideV15::Long, 4).unwrap();
    g.attach_leg(&mut opposing, 0, SideV15::Short, -4).unwrap();
    closing.close_progress = CloseProgressLedgerV15 {
        active: true,
        finalized: false,
        close_id: 1,
        asset_index: 0,
        domain_side: SideV15::Short,
        gross_loss_at_close_start: 2,
        drift_reference_slot: 0,
        max_close_slot: 10,
        residual_remaining: 2,
        ..CloseProgressLedgerV15::EMPTY
    };
    g.current_slot = 1;
    let b_before = g.assets[0].b_short_num;
    let ledger_before = closing.close_progress;

    assert_eq!(
        g.book_bankruptcy_residual_chunk_for_account(&mut closing, 0, SideV15::Long, 2),
        Err(V15Error::RecoveryRequired)
    );
    assert_eq!(
        g.recovery_reason,
        Some(PermissionlessRecoveryReasonV15::ActiveBankruptCloseCannotProgress)
    );
    assert_eq!(g.assets[0].b_short_num, b_before);
    assert_eq!(closing.close_progress, ledger_before);
}

#[test]
fn v15_stale_active_close_routes_recovery_before_quantity_adl() {
    let mut g = group();
    let mut closing = account();
    let mut opposing =
        PortfolioAccountV15::empty(ProvenanceHeaderV15::new([1; 32], [12; 32], [3; 32]));
    g.attach_leg(&mut closing, 0, SideV15::Long, 4).unwrap();
    g.attach_leg(&mut opposing, 0, SideV15::Short, -4).unwrap();
    closing.close_progress = CloseProgressLedgerV15 {
        active: true,
        finalized: true,
        close_id: 1,
        asset_index: 0,
        domain_side: SideV15::Short,
        gross_loss_at_close_start: 1,
        explicit_loss_assigned: 1,
        drift_reference_slot: 0,
        max_close_slot: 10,
        residual_remaining: 0,
        ..CloseProgressLedgerV15::EMPTY
    };
    g.current_slot = 1;
    let a_before = g.assets[0].a_short;
    let oi_long_before = g.assets[0].oi_eff_long_q;
    let oi_short_before = g.assets[0].oi_eff_short_q;

    assert_eq!(
        g.apply_quantity_adl_after_residual_for_account_not_atomic(
            &mut closing,
            0,
            SideV15::Long,
            4
        ),
        Err(V15Error::RecoveryRequired)
    );
    assert_eq!(
        g.recovery_reason,
        Some(PermissionlessRecoveryReasonV15::ActiveBankruptCloseCannotProgress)
    );
    assert_eq!(closing.close_progress.quantity_adl_applied_q, 0);
    assert_eq!(g.assets[0].a_short, a_before);
    assert_eq!(g.assets[0].oi_eff_long_q, oi_long_before);
    assert_eq!(g.assets[0].oi_eff_short_q, oi_short_before);
}

#[test]
fn v15_side_reset_snapshots_epoch_start_for_prior_epoch_accounts() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.assets[0].k_long = 5 * ADL_ONE as i128;
    g.assets[0].oi_eff_long_q = 0;

    g.begin_full_drain_reset(0, SideV15::Long).unwrap();
    assert_eq!(
        g.assets[0].mode_long,
        percolator::v15::SideModeV15::ResetPending
    );
    g.full_account_refresh(&mut a, &[1; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(a.pnl, 5);

    g.clear_leg(&mut a, 0).unwrap();
    g.finalize_ready_reset_side(0, SideV15::Long).unwrap();
    assert_eq!(g.assets[0].mode_long, percolator::v15::SideModeV15::Normal);
    assert_eq!(g.assets[0].stored_pos_count_long, 0);
}

#[test]
fn v15_side_reset_cannot_finalize_until_prior_epoch_positions_clear() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.assets[0].oi_eff_long_q = 0;

    g.begin_full_drain_reset(0, SideV15::Long).unwrap();
    assert_eq!(
        g.assets[0].mode_long,
        percolator::v15::SideModeV15::ResetPending
    );
    assert_eq!(
        g.finalize_ready_reset_side(0, SideV15::Long),
        Err(V15Error::Stale)
    );

    g.clear_leg(&mut a, 0).unwrap();
    assert_eq!(g.finalize_ready_reset_side(0, SideV15::Long), Ok(()));
    assert_eq!(g.assets[0].mode_long, percolator::v15::SideModeV15::Normal);
}

#[test]
fn v15_quantity_adl_reduces_opposing_a_or_starts_reset_after_residual_durable() {
    let mut g = group();
    let mut closing = account();
    let mut survivor =
        PortfolioAccountV15::empty(ProvenanceHeaderV15::new([1; 32], [12; 32], [3; 32]));
    let mut opposing =
        PortfolioAccountV15::empty(ProvenanceHeaderV15::new([1; 32], [13; 32], [3; 32]));
    g.attach_leg(&mut closing, 0, SideV15::Long, 4).unwrap();
    g.attach_leg(&mut survivor, 0, SideV15::Long, 6).unwrap();
    g.attach_leg(&mut opposing, 0, SideV15::Short, -10).unwrap();
    closing.close_progress = CloseProgressLedgerV15 {
        active: true,
        finalized: true,
        close_id: 1,
        asset_index: 0,
        domain_side: SideV15::Short,
        gross_loss_at_close_start: 1,
        explicit_loss_assigned: 1,
        residual_remaining: 0,
        ..CloseProgressLedgerV15::EMPTY
    };

    let partial = g
        .apply_quantity_adl_after_residual_for_account_not_atomic(&mut closing, 0, SideV15::Long, 4)
        .unwrap();
    assert_eq!(partial.closed_q, 4);
    assert_eq!(closing.close_progress.quantity_adl_applied_q, 4);
    assert_eq!(g.assets[0].oi_eff_long_q, 6);
    assert_eq!(g.assets[0].oi_eff_short_q, 6);
    assert_eq!(g.assets[0].a_short, ADL_ONE * 6 / 10);

    let mut g = group();
    let mut closing = account();
    let mut opposing =
        PortfolioAccountV15::empty(ProvenanceHeaderV15::new([1; 32], [14; 32], [3; 32]));
    g.attach_leg(&mut closing, 0, SideV15::Long, 6).unwrap();
    g.attach_leg(&mut opposing, 0, SideV15::Short, -6).unwrap();
    closing.close_progress = CloseProgressLedgerV15 {
        active: true,
        finalized: true,
        close_id: 1,
        asset_index: 0,
        domain_side: SideV15::Short,
        gross_loss_at_close_start: 1,
        explicit_loss_assigned: 1,
        residual_remaining: 0,
        ..CloseProgressLedgerV15::EMPTY
    };
    let full = g
        .apply_quantity_adl_after_residual_for_account_not_atomic(&mut closing, 0, SideV15::Long, 6)
        .unwrap();
    assert!(full.reset_started);
    assert_eq!(closing.close_progress.quantity_adl_applied_q, 6);
    assert_eq!(g.assets[0].oi_eff_long_q, 0);
    assert_eq!(g.assets[0].oi_eff_short_q, 0);
}

#[test]
fn v15_quantity_adl_finalizes_closing_leg_atomically_with_aggregate_oi() {
    let mut g = group();
    let mut closing = account();
    let mut survivor =
        PortfolioAccountV15::empty(ProvenanceHeaderV15::new([1; 32], [12; 32], [3; 32]));
    let mut opposing =
        PortfolioAccountV15::empty(ProvenanceHeaderV15::new([1; 32], [13; 32], [3; 32]));
    g.attach_leg(&mut closing, 0, SideV15::Long, 4).unwrap();
    g.attach_leg(&mut survivor, 0, SideV15::Long, 6).unwrap();
    g.attach_leg(&mut opposing, 0, SideV15::Short, -10).unwrap();
    closing.close_progress = CloseProgressLedgerV15 {
        active: true,
        finalized: true,
        close_id: 1,
        asset_index: 0,
        domain_side: SideV15::Short,
        gross_loss_at_close_start: 1,
        explicit_loss_assigned: 1,
        residual_remaining: 0,
        ..CloseProgressLedgerV15::EMPTY
    };
    let survivor_weight = survivor.legs[0].loss_weight;

    let out = g
        .apply_quantity_adl_after_residual_for_account_not_atomic(&mut closing, 0, SideV15::Long, 4)
        .unwrap();

    assert_eq!(out.closed_q, 4);
    assert_eq!(closing.active_bitmap, 0);
    assert!(!closing.legs[0].active);
    assert_eq!(closing.close_progress.quantity_adl_applied_q, 4);
    assert_eq!(g.assets[0].oi_eff_long_q, 6);
    assert_eq!(g.assets[0].oi_eff_short_q, 6);
    assert_eq!(g.assets[0].stored_pos_count_long, 1);
    assert_eq!(g.assets[0].loss_weight_sum_long, survivor_weight);
}

#[test]
fn v15_quantity_adl_requires_finalized_matching_close_ledger() {
    let mut g = group();
    let mut closing = account();
    g.assets[0].oi_eff_long_q = 1;
    g.assets[0].oi_eff_short_q = 1;

    assert_eq!(
        g.apply_quantity_adl_after_residual_for_account_not_atomic(
            &mut closing,
            0,
            SideV15::Long,
            1,
        ),
        Err(V15Error::LockActive)
    );

    closing.close_progress = CloseProgressLedgerV15 {
        active: true,
        finalized: true,
        close_id: 1,
        asset_index: 0,
        domain_side: SideV15::Long,
        gross_loss_at_close_start: 1,
        explicit_loss_assigned: 1,
        residual_remaining: 0,
        ..CloseProgressLedgerV15::EMPTY
    };
    assert_eq!(
        g.apply_quantity_adl_after_residual_for_account_not_atomic(
            &mut closing,
            0,
            SideV15::Long,
            1,
        ),
        Err(V15Error::LockActive)
    );
}

#[test]
fn v15_account_shape_rejects_malformed_quantity_adl_close_progress() {
    let mut g = group();
    let mut premature = account();
    premature.close_progress = CloseProgressLedgerV15 {
        active: true,
        finalized: false,
        close_id: 1,
        asset_index: 0,
        domain_side: SideV15::Short,
        gross_loss_at_close_start: 2,
        b_loss_booked: 1,
        residual_remaining: 1,
        quantity_adl_applied_q: 1,
        ..CloseProgressLedgerV15::EMPTY
    };
    assert_eq!(
        g.validate_account_shape(&premature),
        Err(V15Error::InvalidLeg),
        "quantity ADL cannot be durable before residual finalization"
    );

    let mut still_open = account();
    g.attach_leg(&mut still_open, 0, SideV15::Long, 4).unwrap();
    still_open.close_progress = CloseProgressLedgerV15 {
        active: true,
        finalized: true,
        close_id: 1,
        asset_index: 0,
        domain_side: SideV15::Short,
        gross_loss_at_close_start: 1,
        explicit_loss_assigned: 1,
        residual_remaining: 0,
        quantity_adl_applied_q: 4,
        ..CloseProgressLedgerV15::EMPTY
    };
    assert_eq!(
        g.validate_account_shape(&still_open),
        Err(V15Error::InvalidLeg),
        "quantity ADL and closing exposure clear must stay atomic"
    );
}

#[test]
fn v15_account_shape_rejects_close_progress_domain_mismatch_for_open_leg() {
    let mut g = group();
    let mut closing = account();
    g.attach_leg(&mut closing, 0, SideV15::Long, 4).unwrap();
    closing.close_progress = CloseProgressLedgerV15 {
        active: true,
        finalized: false,
        close_id: 1,
        asset_index: 0,
        domain_side: SideV15::Long,
        gross_loss_at_close_start: 2,
        b_loss_booked: 1,
        residual_remaining: 1,
        ..CloseProgressLedgerV15::EMPTY
    };

    assert_eq!(
        g.validate_account_shape(&closing),
        Err(V15Error::InvalidLeg),
        "a close ledger for an open long leg must attribute residual loss to the short domain"
    );
}

#[test]
fn v15_permissionless_crank_commits_refresh_before_equity_active_accrual() {
    let mut g = group();
    let mut long = account();
    g.deposit_not_atomic(&mut long, 1000).unwrap();
    g.attach_leg(&mut long, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV15::Long, POS_SCALE, 99);
    let req = PermissionlessCrankRequestV15 {
        now_slot: 1,
        asset_index: 0,
        effective_price: 2,
        funding_rate_e9: 0,
        action: PermissionlessCrankActionV15::Refresh,
    };
    let out = g
        .permissionless_crank_not_atomic(&mut long, req, &[2; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(out, PermissionlessProgressOutcomeV15::AccountCurrent);
    assert_eq!(g.slot_last, 1);
}

#[test]
fn v15_permissionless_crank_flat_refresh_is_not_protective_for_equity_active_accrual() {
    let mut g = group();
    let mut long = account();
    let mut short =
        PortfolioAccountV15::empty(ProvenanceHeaderV15::new([1; 32], [44; 32], [3; 32]));
    let mut flat = PortfolioAccountV15::empty(ProvenanceHeaderV15::new([1; 32], [45; 32], [3; 32]));
    g.deposit_not_atomic(&mut flat, 1).unwrap();
    g.attach_leg(&mut long, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut short, 0, SideV15::Short, -(POS_SCALE as i128))
        .unwrap();
    let before_asset = g.assets[0];
    let before_slot = g.slot_last;

    let res = g.permissionless_crank_not_atomic(
        &mut flat,
        PermissionlessCrankRequestV15 {
            now_slot: 1,
            asset_index: 0,
            effective_price: 2,
            funding_rate_e9: 0,
            action: PermissionlessCrankActionV15::Refresh,
        },
        &[2; V15_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(res, Err(V15Error::NonProgress));
    assert_eq!(g.assets[0], before_asset);
    assert_eq!(g.slot_last, before_slot);
}

#[test]
fn v15_permissionless_crank_cross_asset_liquidation_is_not_protective_for_accrued_asset() {
    let (market, _, _) = ids();
    let mut g = MarketGroupV15::new(market, V15Config::public_user_fund(2, 0, 10)).unwrap();
    let mut victim = account();
    let mut asset0_long =
        PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, [41; 32], [3; 32]));
    let mut asset0_short =
        PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, [42; 32], [3; 32]));
    let mut asset1_short =
        PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, [43; 32], [3; 32]));
    g.attach_leg(&mut asset0_long, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut asset0_short, 0, SideV15::Short, -(POS_SCALE as i128))
        .unwrap();
    g.attach_leg(&mut victim, 1, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut asset1_short, 1, SideV15::Short, -(POS_SCALE as i128))
        .unwrap();
    let before_asset = g.assets[0];
    let before_slot = g.slot_last;
    let req = PermissionlessCrankRequestV15 {
        now_slot: 1,
        asset_index: 0,
        effective_price: 2,
        funding_rate_e9: 0,
        action: PermissionlessCrankActionV15::Liquidate(LiquidationRequestV15 {
            asset_index: 1,
            close_q: POS_SCALE,
            fee_bps: 0,
        }),
    };

    let res = g.permissionless_crank_not_atomic(&mut victim, req, &[1; V15_MAX_PORTFOLIO_ASSETS_N]);

    assert_eq!(res, Err(V15Error::NonProgress));
    assert_eq!(g.assets[0], before_asset);
    assert_eq!(g.slot_last, before_slot);
}

#[test]
fn v15_permissionless_crank_does_not_require_full_market_scan() {
    let mut g = group();
    let mut hinted = account();
    g.deposit_not_atomic(&mut hinted, 1).unwrap();
    g.materialized_portfolio_count = 1_000_000;
    g.stale_certificate_count = 77;
    g.b_stale_account_count = 55;
    g.negative_pnl_account_count = 33;
    let req = PermissionlessCrankRequestV15 {
        now_slot: 0,
        asset_index: 0,
        effective_price: 1,
        funding_rate_e9: 0,
        action: PermissionlessCrankActionV15::Refresh,
    };

    let out = g
        .permissionless_crank_not_atomic(&mut hinted, req, &[1; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(out, PermissionlessProgressOutcomeV15::AccountCurrent);
    assert!(hinted.health_cert.valid);
    assert_eq!(g.materialized_portfolio_count, 1_000_000);
    assert_eq!(g.stale_certificate_count, 77);
    assert_eq!(g.b_stale_account_count, 55);
    assert_eq!(g.negative_pnl_account_count, 33);
}

#[test]
fn v15_permissionless_refresh_returns_partial_b_progress_without_failing() {
    let (market, _, _) = ids();
    let mut cfg = V15Config::public_user_fund(1, 0, 10);
    cfg.public_b_chunk_atoms = 1;
    let mut g = MarketGroupV15::new(market, cfg).unwrap();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.attach_leg(&mut a, 0, SideV15::Long, 1).unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV15::Long, 1, 100);
    g.assets[0].b_long_num = SOCIAL_LOSS_DEN * 2;
    let req = PermissionlessCrankRequestV15 {
        now_slot: 1,
        asset_index: 0,
        effective_price: 1,
        funding_rate_e9: 0,
        action: PermissionlessCrankActionV15::Refresh,
    };

    let out = g
        .permissionless_crank_not_atomic(&mut a, req, &[1; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert!(matches!(
        out,
        PermissionlessProgressOutcomeV15::AccountBChunk(_)
    ));
    assert!(a.legs[0].b_stale);
    assert!(a.legs[0].b_snap > 0);
    assert!(a.legs[0].b_snap < g.assets[0].b_long_num);
    assert_eq!(g.slot_last, 0);
}

#[test]
fn v15_worst_case_hinted_progress_actions_are_total_and_bounded() {
    let req_current = PermissionlessCrankRequestV15 {
        now_slot: 0,
        asset_index: 0,
        effective_price: 1,
        funding_rate_e9: 0,
        action: PermissionlessCrankActionV15::Refresh,
    };
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 1).unwrap();
    assert_eq!(
        g.permissionless_crank_not_atomic(&mut a, req_current, &[1; V15_MAX_PORTFOLIO_ASSETS_N]),
        Ok(PermissionlessProgressOutcomeV15::AccountCurrent)
    );
    assert!(a.health_cert.valid);

    let (market, _, _) = ids();
    let mut cfg = V15Config::public_user_fund(1, 0, 10);
    cfg.public_b_chunk_atoms = 1;
    let mut g = MarketGroupV15::new(market, cfg).unwrap();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV15::Long, 1).unwrap();
    g.assets[0].b_long_num = 2;
    let out = g
        .permissionless_crank_not_atomic(
            &mut a,
            PermissionlessCrankRequestV15 {
                action: PermissionlessCrankActionV15::SettleB { asset_index: 0 },
                ..req_current
            },
            &[1; V15_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();
    match out {
        PermissionlessProgressOutcomeV15::AccountBChunk(chunk) => {
            assert_eq!(chunk.delta_b, 1);
            assert_eq!(chunk.remaining_after, 1);
        }
        _ => panic!("SettleB hint must return bounded B progress"),
    }
    assert!(a.b_stale_state);
    assert_eq!(g.b_stale_account_count, 1);

    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    let _opposing = attach_opposite(&mut g, 0, SideV15::Long, POS_SCALE, 91);
    let out = g
        .permissionless_crank_not_atomic(
            &mut a,
            PermissionlessCrankRequestV15 {
                action: PermissionlessCrankActionV15::Liquidate(LiquidationRequestV15 {
                    asset_index: 0,
                    close_q: POS_SCALE,
                    fee_bps: 0,
                }),
                ..req_current
            },
            &[1; V15_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();
    assert_eq!(out, PermissionlessProgressOutcomeV15::AccountCurrent);
    assert_eq!(a.active_bitmap, 0);

    let mut g = group();
    let mut a = account();
    let reason = PermissionlessRecoveryReasonV15::BelowProgressFloor;
    assert_eq!(
        g.permissionless_crank_not_atomic(
            &mut a,
            PermissionlessCrankRequestV15 {
                action: PermissionlessCrankActionV15::Recover(reason),
                ..req_current
            },
            &[1; V15_MAX_PORTFOLIO_ASSETS_N],
        ),
        Ok(PermissionlessProgressOutcomeV15::RecoveryDeclared(reason))
    );
    assert_eq!(g.recovery_reason, Some(reason));
}

#[test]
fn v15_resolved_close_is_bounded_and_fee_current() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.resolve_market_not_atomic(10).unwrap();
    let out = g.close_resolved_account_not_atomic(&mut a, 1).unwrap();
    assert_eq!(out, ResolvedCloseOutcomeV15::Closed { payout: 90 });
    assert_eq!(a.last_fee_slot, 10);
    assert_eq!(a.capital, 0);
}

#[test]
fn v15_resolved_flat_close_returns_exact_capital() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 777).unwrap();
    g.resolve_market_not_atomic(1).unwrap();

    let out = g.close_resolved_account_not_atomic(&mut a, 0).unwrap();

    assert_eq!(out, ResolvedCloseOutcomeV15::Closed { payout: 777 });
    assert_eq!(a.capital, 0);
    assert_eq!(a.pnl, 0);
    assert_eq!(g.c_tot, 0);
    assert_eq!(g.vault, 0);
}

#[test]
fn v15_resolved_profit_close_pays_from_snapshot_residual_and_clears_claim() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 10).unwrap();
    a.pnl = 7;
    g.pnl_pos_tot = 7;
    set_junior_bound(&mut g, 7);
    g.vault = g.c_tot + 7;
    g.resolve_market_not_atomic(1).unwrap();

    let out = g.close_resolved_account_not_atomic(&mut a, 0).unwrap();

    assert_eq!(out, ResolvedCloseOutcomeV15::Closed { payout: 17 });
    assert_eq!(a.capital, 0);
    assert_eq!(a.pnl, 0);
    assert_eq!(g.c_tot, 0);
    assert_eq!(g.pnl_pos_tot, 0);
    assert_eq!(g.vault, 0);
}

#[test]
fn v15_resolved_close_with_active_position_returns_progress_only() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 777).unwrap();
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.resolve_market_not_atomic(1).unwrap();
    let before_vault = g.vault;
    let before_c_tot = g.c_tot;

    let out = g.close_resolved_account_not_atomic(&mut a, 0).unwrap();

    assert_eq!(out, ResolvedCloseOutcomeV15::ProgressOnly);
    assert_eq!(a.capital, 777);
    assert_ne!(a.active_bitmap, 0);
    assert_eq!(g.vault, before_vault);
    assert_eq!(g.c_tot, before_c_tot);
}

#[test]
fn v15_resolved_close_returns_progress_after_partial_b_settlement() {
    let (market, _, _) = ids();
    let mut cfg = V15Config::public_user_fund(1, 0, 10);
    cfg.public_b_chunk_atoms = 1;
    let mut g = MarketGroupV15::new(market, cfg).unwrap();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.attach_leg(&mut a, 0, SideV15::Long, 1).unwrap();
    g.assets[0].b_long_num = SOCIAL_LOSS_DEN * 2;
    g.resolve_market_not_atomic(10).unwrap();

    let out = g.close_resolved_account_not_atomic(&mut a, 1).unwrap();

    assert_eq!(out, ResolvedCloseOutcomeV15::ProgressOnly);
    assert!(a.legs[0].b_stale);
    assert!(a.legs[0].b_snap > 0);
    assert!(a.legs[0].b_snap < g.assets[0].b_long_num);
    assert_eq!(a.last_fee_slot, 0);
    assert_eq!(a.active_bitmap, 1);
}

#[test]
fn v15_resolved_payout_readiness_uses_exact_counters_and_bounds() {
    for case in 0..7 {
        let mut g = group();
        let mut a = account();
        g.vault = 10;
        a.pnl = 10;
        g.pnl_pos_tot = 10;
        set_junior_bound(&mut g, 10);
        g.resolve_market_not_atomic(1).unwrap();
        match case {
            0 => g.b_stale_account_count = 1,
            1 => g.stale_certificate_count = 1,
            2 => g.negative_pnl_account_count = 1,
            3 => g.assets[0].stored_pos_count_long = 1,
            4 => g.assets[0].stored_pos_count_short = 1,
            5 => g.assets[0].stale_account_count_long = 1,
            _ => g.assets[0].stale_account_count_short = 1,
        }

        let vault_before = g.vault;
        let pnl_pos_before = g.pnl_pos_tot;
        let bound_before = g.pnl_pos_bound_tot;
        let account_pnl_before = a.pnl;
        let outcome = g.close_resolved_account_not_atomic(&mut a, 0).unwrap();

        assert_eq!(
            outcome,
            ResolvedCloseOutcomeV15::ProgressOnly,
            "readiness blocker case {case} must not pay positive PnL"
        );
        assert_eq!(g.vault, vault_before);
        assert_eq!(g.pnl_pos_tot, pnl_pos_before);
        assert_eq!(g.pnl_pos_bound_tot, bound_before);
        assert_eq!(a.pnl, account_pnl_before);
        assert!(!g.payout_snapshot_captured);
    }
}

#[test]
fn v15_resolved_positive_payout_waits_for_pending_domain_loss_barrier() {
    let mut g = group();
    let mut a = account();
    g.vault = 10;
    a.pnl = 10;
    g.pnl_pos_tot = 10;
    set_junior_bound(&mut g, 10);
    g.resolve_market_not_atomic(1).unwrap();
    g.pending_domain_loss_barriers[1] = 1;

    let vault_before = g.vault;
    let pnl_pos_before = g.pnl_pos_tot;
    let bound_before = g.pnl_pos_bound_tot;
    let outcome = g.close_resolved_account_not_atomic(&mut a, 0).unwrap();

    assert_eq!(
        outcome,
        ResolvedCloseOutcomeV15::ProgressOnly,
        "pending domain-loss barriers must block positive payout readiness"
    );
    assert_eq!(g.vault, vault_before);
    assert_eq!(g.pnl_pos_tot, pnl_pos_before);
    assert_eq!(g.pnl_pos_bound_tot, bound_before);
    assert_eq!(a.pnl, 10);
    assert!(!g.payout_snapshot_captured);
}

#[test]
fn v15_pending_domain_loss_barrier_does_not_freeze_unrelated_positive_credit() {
    let (market, _, owner) = ids();
    let mut g = MarketGroupV15::new(market, V15Config::public_user_fund(2, 0, 10)).unwrap();
    let mut profitable =
        PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, [71; 32], owner));
    let mut opposite =
        PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, [72; 32], owner));
    g.deposit_not_atomic(&mut profitable, 100).unwrap();
    g.deposit_not_atomic(&mut opposite, 100).unwrap();
    g.attach_leg(&mut profitable, 1, SideV15::Long, 10).unwrap();
    g.attach_leg(&mut opposite, 1, SideV15::Short, -10).unwrap();
    profitable.pnl = 5;
    g.pnl_pos_tot = 5;
    g.pnl_matured_pos_tot = 5;
    set_junior_bound(&mut g, 5);
    g.vault = g.c_tot + 5;
    g.pending_domain_loss_barriers[1] = 1;
    g.full_account_refresh(&mut profitable, &[1; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    let converted = g
        .convert_released_pnl_to_capital_not_atomic(&mut profitable)
        .unwrap();

    assert_eq!(converted, 5);
    assert_eq!(profitable.pnl, 0);
    assert_eq!(profitable.capital, 105);
    assert_eq!(g.c_tot, 205);
    assert_eq!(g.pending_domain_loss_barriers[1], 1);
    assert_eq!(g.assets[1].oi_eff_long_q, 10);
    assert_eq!(g.assets[1].oi_eff_short_q, 10);
}

#[test]
fn v15_ordinary_positive_conversion_disabled_after_resolved_payout_lane_exists() {
    let mut g = group();
    let mut a = account();
    a.pnl = 10;
    g.pnl_pos_tot = 10;
    g.pnl_matured_pos_tot = 10;
    set_junior_bound(&mut g, 10);
    g.vault = 10;
    g.full_account_refresh(&mut a, &[1; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    g.resolve_market_not_atomic(1).unwrap();
    let before = (g, a);

    let result = g.convert_released_pnl_to_capital_not_atomic(&mut a);

    assert_eq!(result, Err(V15Error::LockActive));
    assert_eq!((g, a), before);

    let mut live = group();
    let mut live_account = account();
    live_account.pnl = 10;
    live.pnl_pos_tot = 10;
    live.pnl_matured_pos_tot = 10;
    set_junior_bound(&mut live, 10);
    live.vault = 10;
    initialize_payout_ledger(&mut live);
    live.full_account_refresh(&mut live_account, &[1; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    let before_live = (live, live_account);

    let live_result = live.convert_released_pnl_to_capital_not_atomic(&mut live_account);

    assert_eq!(live_result, Err(V15Error::LockActive));
    assert_eq!((live, live_account), before_live);
}

#[test]
fn v15_dead_leg_forfeit_is_unavailable_for_normal_live_leg() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();

    assert_eq!(
        g.forfeit_recovery_leg_not_atomic(&mut a, 0, 4),
        Err(V15Error::LockActive)
    );
    assert!(a.legs[0].active);
    assert_eq!(g.assets[0].oi_eff_long_q, POS_SCALE);
}

#[test]
fn v15_dead_leg_forfeit_detaches_without_crediting_positive_pnl() {
    let mut g = group();
    let mut a = account();
    let mut unrelated =
        PortfolioAccountV15::empty(ProvenanceHeaderV15::new([1; 32], [21; 32], [3; 32]));
    g.mode = MarketModeV15::Recovery;
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut unrelated, 1, SideV15::Short, -(POS_SCALE as i128))
        .unwrap();
    g.assets[0].k_long = 7 * ADL_ONE as i128;

    let out = g.forfeit_recovery_leg_not_atomic(&mut a, 0, 4).unwrap();

    assert!(out.detached);
    assert_eq!(out.positive_pnl_forfeited, 7);
    assert_eq!(out.residual_booked, 0);
    assert_eq!(
        a.pnl, 0,
        "forfeited dead-leg profit must not become account credit"
    );
    assert_eq!(g.pnl_pos_tot, 0);
    assert_eq!(a.active_bitmap, 0);
    assert!(!a.legs[0].active);
    assert!(unrelated.legs[1].active);
    assert_eq!(g.assets[0].oi_eff_long_q, 0);
    assert_eq!(g.assets[1].oi_eff_short_q, POS_SCALE);
}

#[test]
fn v15_dead_leg_forfeit_books_negative_residual_to_opposing_domain_only() {
    let mut g = group();
    let mut bankrupt = account();
    let mut opposing =
        PortfolioAccountV15::empty(ProvenanceHeaderV15::new([1; 32], [22; 32], [3; 32]));
    g.mode = MarketModeV15::Recovery;
    g.attach_leg(&mut bankrupt, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut opposing, 0, SideV15::Short, -(POS_SCALE as i128))
        .unwrap();
    g.assets[0].mode_long = SideModeV15::DrainOnly;
    g.assets[0].k_long = -(5 * ADL_ONE as i128);
    let long_b_before = g.assets[0].b_long_num;
    let short_b_before = g.assets[0].b_short_num;

    let out = g
        .forfeit_recovery_leg_not_atomic(&mut bankrupt, 0, 10)
        .unwrap();

    assert!(out.detached);
    assert_eq!(out.loss_settled, 5);
    assert_eq!(out.residual_booked, 5);
    assert_eq!(out.insurance_used, 0);
    assert_eq!(bankrupt.pnl, 0);
    assert!(!bankrupt.legs[0].active);
    assert_eq!(g.assets[0].oi_eff_long_q, 0);
    assert_eq!(g.assets[0].oi_eff_short_q, POS_SCALE);
    assert_eq!(g.assets[0].b_long_num, long_b_before);
    assert!(
        g.assets[0].b_short_num > short_b_before,
        "long dead-leg residual must book to the short bankruptcy domain"
    );
    assert_eq!(
        g.pending_domain_loss_barrier_count(0, SideV15::Short),
        Ok(0)
    );
    assert!(bankrupt.close_progress.finalized);
}

#[test]
fn v15_dead_leg_forfeit_haircuts_positive_support_when_junior_impaired() {
    let mut g = group();
    let mut bankrupt = account();
    let mut opposing =
        PortfolioAccountV15::empty(ProvenanceHeaderV15::new([1; 32], [23; 32], [3; 32]));
    g.mode = MarketModeV15::Recovery;
    g.attach_leg(&mut bankrupt, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut opposing, 0, SideV15::Short, -(POS_SCALE as i128))
        .unwrap();
    g.assets[0].mode_long = SideModeV15::DrainOnly;
    g.assets[0].k_long = -(100 * ADL_ONE as i128);

    bankrupt.pnl = 100;
    g.pnl_pos_tot = 100;
    set_junior_bound(&mut g, 100);
    g.vault = 50;

    let out = g
        .forfeit_recovery_leg_not_atomic(&mut bankrupt, 0, 50)
        .unwrap();

    assert!(out.detached);
    assert_eq!(out.loss_settled, 100);
    assert_eq!(out.support_consumed, 50);
    assert_eq!(out.junior_face_burned, 100);
    assert_eq!(out.residual_booked, 50);
    assert_eq!(out.insurance_used, 0);
    assert_eq!(bankrupt.pnl, 0);
    assert_eq!(g.pnl_pos_tot, 0);
    assert_eq!(g.pnl_pos_bound_tot, 0);
    assert!(
        g.assets[0].b_short_num > 0,
        "haircut-uncovered loss must be durably charged to the opposing domain"
    );
    assert_eq!(bankrupt.close_progress.gross_loss_at_close_start, 100);
    assert_eq!(bankrupt.close_progress.support_consumed, 50);
    assert_eq!(bankrupt.close_progress.junior_face_burned, 100);
    assert!(bankrupt.close_progress.finalized);
    assert!(!bankrupt.legs[0].active);
}

#[test]
fn v15_resolved_positive_payout_uses_stable_snapshot_denominator() {
    let mut g = group();
    let mut a = account();
    let mut b = account();
    b.provenance_header.portfolio_account_id = [4; 32];
    g.vault = 100;
    a.pnl = 100;
    b.pnl = 100;
    g.pnl_pos_tot = 200;
    set_junior_bound(&mut g, 200);
    g.resolve_market_not_atomic(1).unwrap();

    let first = g.close_resolved_account_not_atomic(&mut a, 0).unwrap();
    let second = g.close_resolved_account_not_atomic(&mut b, 0).unwrap();

    assert_eq!(first, ResolvedCloseOutcomeV15::Closed { payout: 50 });
    assert_eq!(second, ResolvedCloseOutcomeV15::Closed { payout: 50 });
    assert_eq!(g.payout_snapshot, 100);
    assert_eq!(g.payout_snapshot_pnl_pos_tot, 200);
}

#[test]
fn v15_resolved_positive_payout_uses_conservative_bound_denominator() {
    let mut g = group();
    let mut a = account();
    g.vault = 100;
    a.pnl = 100;
    g.pnl_pos_tot = 100;
    set_junior_bound(&mut g, 200);
    g.resolve_market_not_atomic(1).unwrap();

    let out = g.close_resolved_account_not_atomic(&mut a, 0).unwrap();

    assert_eq!(out, ResolvedCloseOutcomeV15::Closed { payout: 50 });
    assert_eq!(g.payout_snapshot, 100);
    assert_eq!(g.payout_snapshot_pnl_pos_tot, 200);
    assert_eq!(g.vault, 50);
}

#[test]
fn v15_resolved_positive_payout_uses_scaled_bound_remainder_denominator() {
    let mut g = group();
    let mut a = account();
    g.vault = 1;
    a.pnl = 1;
    g.pnl_pos_tot = 1;
    g.pnl_pos_bound_tot_num = BOUND_SCALE + 1;
    g.pnl_pos_bound_tot = 2;
    g.resolve_market_not_atomic(1).unwrap();

    let out = g.close_resolved_account_not_atomic(&mut a, 0).unwrap();

    assert_eq!(out, ResolvedCloseOutcomeV15::Closed { payout: 0 });
    assert_eq!(g.payout_snapshot, 1);
    assert_eq!(g.payout_snapshot_pnl_pos_tot, 2);
    assert_eq!(g.vault, 1);
    assert_eq!(g.pnl_pos_bound_tot_num, 1);
    assert_eq!(g.pnl_pos_bound_tot, 1);
}

#[test]
fn v15_resolved_payout_receipt_tracks_paid_effective_and_later_topup() {
    let mut g = group();
    let mut a = account();
    g.vault = 1;
    a.pnl = 1;
    g.pnl_pos_tot = 1;
    g.pnl_pos_bound_tot_num = BOUND_SCALE + 1;
    g.pnl_pos_bound_tot = 2;
    g.resolve_market_not_atomic(1).unwrap();

    let first = g.close_resolved_account_not_atomic(&mut a, 0).unwrap();

    assert_eq!(first, ResolvedCloseOutcomeV15::Closed { payout: 0 });
    assert!(a.resolved_payout_receipt.present);
    assert_eq!(a.resolved_payout_receipt.terminal_positive_claim_face, 1);
    assert_eq!(a.resolved_payout_receipt.paid_effective, 0);
    assert_eq!(
        g.resolved_payout_ledger.terminal_claim_exact_receipts_num,
        BOUND_SCALE
    );
    assert_eq!(
        g.resolved_payout_ledger
            .terminal_claim_bound_unreceipted_num,
        1
    );

    g.refine_resolved_unreceipted_bound_not_atomic(1).unwrap();
    let topup = g.claim_resolved_payout_topup_not_atomic(&mut a).unwrap();

    assert_eq!(topup, 1);
    assert_eq!(a.resolved_payout_receipt.paid_effective, 1);
    assert!(a.resolved_payout_receipt.finalized);
    assert_eq!(g.vault, 0);
}

#[test]
fn v15_public_invariants_reject_scaled_junior_bound_cache_mismatch() {
    let mut g = group();
    g.pnl_pos_tot = 1;
    g.pnl_pos_bound_tot_num = BOUND_SCALE + 1;
    g.pnl_pos_bound_tot = 1;
    assert_eq!(g.assert_public_invariants(), Err(V15Error::InvalidConfig));

    g.pnl_pos_bound_tot_num = BOUND_SCALE - 1;
    g.pnl_pos_bound_tot = 1;
    assert_eq!(g.assert_public_invariants(), Err(V15Error::InvalidConfig));
}

#[test]
fn v15_pnl_pos_bound_tot_prevents_lazy_positive_pnl_first_mover_overpay() {
    let mut g = group();
    let mut first_mover = account();
    g.vault = 100;
    first_mover.pnl = 100;
    g.pnl_pos_tot = 100;
    set_junior_bound(&mut g, 300);
    g.resolve_market_not_atomic(1).unwrap();

    let out = g
        .close_resolved_account_not_atomic(&mut first_mover, 0)
        .unwrap();

    assert_eq!(out, ResolvedCloseOutcomeV15::Closed { payout: 33 });
    assert_eq!(g.payout_snapshot, 100);
    assert_eq!(g.payout_snapshot_pnl_pos_tot, 300);
    assert_eq!(g.vault, 67);
}

#[test]
fn v15_liquidation_requires_strict_account_risk_progress() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV15::Long, POS_SCALE, 101);
    g.accrue_asset_to_not_atomic(0, 1, 1, 0, true).unwrap();
    let req = LiquidationRequestV15 {
        asset_index: 0,
        close_q: POS_SCALE,
        fee_bps: 0,
    };
    let out = g
        .liquidate_account_not_atomic(&mut a, req, &[1; V15_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(out.closed_q, POS_SCALE);
    assert_eq!(a.active_bitmap, 0);
}

#[test]
fn v15_partial_liquidation_can_reduce_risk_without_forcing_full_close() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 10).unwrap();
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV15::Long, POS_SCALE, 102);

    let out = g
        .liquidate_account_not_atomic(
            &mut a,
            LiquidationRequestV15 {
                asset_index: 0,
                close_q: POS_SCALE / 2,
                fee_bps: 0,
            },
            &[100; V15_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(out.closed_q, POS_SCALE / 2);
    assert_eq!(a.legs[0].basis_pos_q.unsigned_abs(), POS_SCALE / 2);
    assert_eq!(g.assets[0].oi_eff_long_q, POS_SCALE / 2);
    assert_eq!(a.health_cert.certified_liq_deficit, 40);
}

#[test]
fn v15_partial_liquidation_cannot_b_book_residual_while_open_risk_remains() {
    let mut g = group();
    let mut bankrupt = account();
    let mut opposing =
        PortfolioAccountV15::empty(ProvenanceHeaderV15::new([1; 32], [42; 32], [3; 32]));
    g.attach_leg(&mut bankrupt, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut opposing, 0, SideV15::Short, -(POS_SCALE as i128))
        .unwrap();
    g.assets[0].k_long = -(100 * ADL_ONE as i128);

    let before_b_short = g.assets[0].b_short_num;
    let res = g.liquidate_account_not_atomic(
        &mut bankrupt,
        LiquidationRequestV15 {
            asset_index: 0,
            close_q: POS_SCALE / 2,
            fee_bps: 0,
        },
        &[1; V15_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(res, Err(V15Error::RecoveryRequired));
    assert_eq!(
        g.assets[0].b_short_num, before_b_short,
        "partial liquidation must not socialize residual while the account still has closable risk"
    );
    assert!(bankrupt.legs[0].active);
    assert_eq!(bankrupt.legs[0].basis_pos_q.unsigned_abs(), POS_SCALE);
}

#[test]
fn v15_liquidation_rejects_zero_close_before_mutation() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    let before_group = g;
    let before_account = a;

    let res = g.liquidate_account_not_atomic(
        &mut a,
        LiquidationRequestV15 {
            asset_index: 0,
            close_q: 0,
            fee_bps: 0,
        },
        &[100; V15_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(res, Err(V15Error::InvalidConfig));
    assert_eq!(g, before_group);
    assert_eq!(a, before_account);
}

#[test]
fn v15_min_liquidation_abs_shortfall_does_not_block_risk_close() {
    let (market, account_id, owner) = ids();
    let mut cfg = V15Config::public_user_fund(1, 0, 1);
    cfg.min_nonzero_mm_req = 100;
    cfg.min_nonzero_im_req = 101;
    cfg.max_price_move_bps_per_slot = 5_000;
    cfg.liquidation_fee_cap = 40;
    cfg.min_liquidation_abs = 40;
    let mut g = MarketGroupV15::new(market, cfg).unwrap();
    let mut a = PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, account_id, owner));
    g.deposit_not_atomic(&mut a, 20).unwrap();
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV15::Long, POS_SCALE, 103);

    let out = g
        .liquidate_account_not_atomic(
            &mut a,
            LiquidationRequestV15 {
                asset_index: 0,
                close_q: POS_SCALE,
                fee_bps: 0,
            },
            &[100; V15_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(out.closed_q, POS_SCALE);
    assert_eq!(out.fee_charged, 20);
    assert_eq!(a.capital, 0);
    assert_eq!(a.active_bitmap, 0);
    assert_eq!(g.insurance, 20);
    assert_eq!(g.c_tot, 0);
    assert_eq!(g.vault, 20);
    assert_eq!(g.assert_public_invariants(), Ok(()));
}

#[test]
fn v15_bankrupt_liquidation_consumes_insurance_before_social_loss() {
    let (market, _, owner) = ids();
    let mut g = group();
    let mut a = account();
    let mut opposing = PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, [4; 32], owner));
    g.vault = 4;
    g.insurance = 4;
    a.pnl = -9;
    g.negative_pnl_account_count = 1;
    g.attach_leg(&mut a, 0, SideV15::Long, 1).unwrap();
    g.attach_leg(&mut opposing, 0, SideV15::Short, -1).unwrap();

    let out = g
        .liquidate_account_not_atomic(
            &mut a,
            LiquidationRequestV15 {
                asset_index: 0,
                close_q: 1,
                fee_bps: 0,
            },
            &[1; V15_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(out.insurance_used, 4);
    assert_eq!(out.residual_booked, 5);
    assert_eq!(out.explicit_loss, 0);
    assert_eq!(g.insurance, 0);
    assert_eq!(a.pnl, 0);
    assert_eq!(a.active_bitmap, 0);
}

#[test]
fn v15_domain_insurance_budget_caps_bankruptcy_spend_for_one_asset_side() {
    let (market, _, owner) = ids();
    let mut g = group();
    let mut a = account();
    let mut opposing = PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, [4; 32], owner));
    g.vault = 10;
    g.insurance = 10;
    g.insurance_domain_budget = [0; V15_DOMAIN_COUNT];
    let short_domain_for_bankrupt_long = 1;
    g.insurance_domain_budget[short_domain_for_bankrupt_long] = 3;
    a.pnl = -9;
    g.negative_pnl_account_count = 1;
    g.attach_leg(&mut a, 0, SideV15::Long, 1).unwrap();
    g.attach_leg(&mut opposing, 0, SideV15::Short, -1).unwrap();

    let out = g
        .liquidate_account_not_atomic(
            &mut a,
            LiquidationRequestV15 {
                asset_index: 0,
                close_q: 1,
                fee_bps: 0,
            },
            &[1; V15_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(out.insurance_used, 3);
    assert_eq!(out.residual_booked, 6);
    assert_eq!(out.explicit_loss, 0);
    assert_eq!(g.insurance, 7);
    assert_eq!(g.insurance_domain_spent[short_domain_for_bankrupt_long], 3);
    assert_eq!(g.insurance_domain_spent[0], 0);
    assert_eq!(a.pnl, 0);
    assert_eq!(a.active_bitmap, 0);
}

#[test]
fn v15_liquidation_residual_domain_is_opposite_side_for_long_and_short() {
    for bankrupt_side in [SideV15::Long, SideV15::Short] {
        let (market, _, owner) = ids();
        let mut g = group();
        let mut bankrupt = account();
        let mut opposing =
            PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, [4; 32], owner));
        g.vault = 4;
        g.insurance = 4;
        g.insurance_domain_budget = [0; V15_DOMAIN_COUNT];
        let expected_domain = match bankrupt_side {
            SideV15::Long => 1,
            SideV15::Short => 0,
        };
        let unrelated_domain = match bankrupt_side {
            SideV15::Long => 0,
            SideV15::Short => 1,
        };
        g.insurance_domain_budget[expected_domain] = 3;
        bankrupt.pnl = -5;
        g.negative_pnl_account_count = 1;
        match bankrupt_side {
            SideV15::Long => {
                g.attach_leg(&mut bankrupt, 0, SideV15::Long, 1).unwrap();
                g.attach_leg(&mut opposing, 0, SideV15::Short, -1).unwrap();
            }
            SideV15::Short => {
                g.attach_leg(&mut bankrupt, 0, SideV15::Short, -1).unwrap();
                g.attach_leg(&mut opposing, 0, SideV15::Long, 1).unwrap();
            }
        }

        let out = g
            .liquidate_account_not_atomic(
                &mut bankrupt,
                LiquidationRequestV15 {
                    asset_index: 0,
                    close_q: 1,
                    fee_bps: 0,
                },
                &[1; V15_MAX_PORTFOLIO_ASSETS_N],
            )
            .unwrap();

        assert_eq!(out.insurance_used, 3);
        assert_eq!(out.residual_booked, 2);
        assert_eq!(g.insurance_domain_spent[expected_domain], 3);
        assert_eq!(g.insurance_domain_spent[unrelated_domain], 0);
        assert_eq!(bankrupt.pnl, 0);
        assert_eq!(bankrupt.active_bitmap, 0);
    }
}

#[test]
fn v15_bad_asset_cannot_spend_unrelated_domain_insurance_budget() {
    let mut g = group();
    let mut bankrupt = account();
    let mut opposing = account_with_id(9);
    g.vault = 4;
    g.insurance = 4;
    g.insurance_domain_budget = [0; V15_DOMAIN_COUNT];
    g.insurance_domain_budget[0] = 4;
    bankrupt.pnl = -5;
    g.negative_pnl_account_count = 1;
    g.attach_leg(&mut bankrupt, 0, SideV15::Long, 1).unwrap();
    g.attach_leg(&mut opposing, 0, SideV15::Short, -1).unwrap();

    let out = g
        .liquidate_account_not_atomic(
            &mut bankrupt,
            LiquidationRequestV15 {
                asset_index: 0,
                close_q: 1,
                fee_bps: 0,
            },
            &[1; V15_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(out.insurance_used, 0);
    assert_eq!(out.residual_booked, 5);
    assert_eq!(g.insurance, 4);
    assert_eq!(g.insurance_domain_spent, [0; V15_DOMAIN_COUNT]);
    assert_eq!(
        g.pending_domain_loss_barrier_count(0, SideV15::Short),
        Ok(0)
    );
    assert_eq!(bankrupt.pnl, 0);
    assert_eq!(bankrupt.active_bitmap, 0);
}

#[test]
fn v15_bankrupt_liquidation_drops_uncollectible_fee_and_spends_insurance_once() {
    let (market, _, owner) = ids();
    let mut cfg = V15Config::public_user_fund(1, 0, 10);
    cfg.max_price_move_bps_per_slot = 1;
    cfg.min_nonzero_mm_req = 12;
    cfg.min_nonzero_im_req = 13;
    cfg.liquidation_fee_bps = 10_000;
    cfg.liquidation_fee_cap = 10;
    cfg.min_liquidation_abs = 1;
    let mut g = MarketGroupV15::new(market, cfg).unwrap();
    let mut a = account();
    let mut opposing = PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, [4; 32], owner));
    g.vault = 2;
    g.insurance = 2;
    a.pnl = -5;
    g.negative_pnl_account_count = 1;
    g.attach_leg(&mut a, 0, SideV15::Long, 1).unwrap();
    g.attach_leg(&mut opposing, 0, SideV15::Short, -1).unwrap();

    let out = g
        .liquidate_account_not_atomic(
            &mut a,
            LiquidationRequestV15 {
                asset_index: 0,
                close_q: 1,
                fee_bps: 10_000,
            },
            &[1; V15_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(out.fee_charged, 0);
    assert_eq!(out.insurance_used, 2);
    assert_eq!(out.residual_booked, 3);
    assert_eq!(out.explicit_loss, 0);
    assert_eq!(g.insurance, 0);
    assert_eq!(a.pnl, 0);
    assert_eq!(a.active_bitmap, 0);
}

#[test]
fn v15_bankrupt_liquidation_requires_residual_durable_before_freeing_exposure() {
    let (market, _, owner) = ids();
    let mut cfg = V15Config::public_user_fund(1, 0, 10);
    cfg.public_b_chunk_atoms = 1;
    let mut g = MarketGroupV15::new(market, cfg).unwrap();
    let mut bankrupt = account();
    let mut opposing = PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, [4; 32], owner));

    g.attach_leg(&mut bankrupt, 0, SideV15::Long, 4).unwrap();
    g.attach_leg(&mut opposing, 0, SideV15::Short, -10).unwrap();
    bankrupt.pnl = -5;
    g.negative_pnl_account_count = 1;

    let before_bitmap = bankrupt.active_bitmap;
    let before_basis = bankrupt.legs[0].basis_pos_q;
    let before_pnl = bankrupt.pnl;
    let res = g.liquidate_account_not_atomic(
        &mut bankrupt,
        LiquidationRequestV15 {
            asset_index: 0,
            close_q: 4,
            fee_bps: 0,
        },
        &[1; V15_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(res, Err(V15Error::RecoveryRequired));
    assert_eq!(
        g.recovery_reason,
        Some(PermissionlessRecoveryReasonV15::ActiveBankruptCloseCannotProgress)
    );
    assert_eq!(bankrupt.active_bitmap, before_bitmap);
    assert_eq!(bankrupt.legs[0].basis_pos_q, before_basis);
    assert_eq!(bankrupt.pnl, before_pnl);
    assert_eq!(g.assets[0].b_short_num, 0);
}

#[test]
fn v15_rebalance_reduce_position_requires_strict_risk_progress_and_preserves_senior_claims() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV15::Long, POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV15::Long, POS_SCALE, 104);
    let senior_before = g.c_tot + g.insurance;
    let out = g
        .rebalance_reduce_position_not_atomic(
            &mut a,
            RebalanceRequestV15 {
                asset_index: 0,
                reduce_q: POS_SCALE / 2,
            },
            &[1_000_000; V15_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(out.reduced_q, POS_SCALE / 2);
    assert_eq!(a.legs[0].basis_pos_q.unsigned_abs(), POS_SCALE / 2);
    assert_eq!(g.c_tot + g.insurance, senior_before);
}

#[test]
fn v15_rebalance_rejects_missing_or_zero_progress() {
    let mut g = group();
    let mut a = account();

    assert_eq!(
        g.rebalance_reduce_position_not_atomic(
            &mut a,
            RebalanceRequestV15 {
                asset_index: 0,
                reduce_q: 1,
            },
            &[1_000_000; V15_MAX_PORTFOLIO_ASSETS_N],
        ),
        Err(V15Error::InvalidLeg)
    );
    assert_eq!(
        g.rebalance_reduce_position_not_atomic(
            &mut a,
            RebalanceRequestV15 {
                asset_index: 0,
                reduce_q: 0,
            },
            &[1_000_000; V15_MAX_PORTFOLIO_ASSETS_N],
        ),
        Err(V15Error::InvalidConfig)
    );
}
