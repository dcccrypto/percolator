use percolator::v16::{
    v16_domain_count_for_market_slots, AssetStateV16Account, BackingBucketStatusV16,
    BackingBucketV16, BackingBucketV16Account, EngineAssetSlotV16Account, LiquidationRequestV16,
    Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, PermissionlessCrankActionV16,
    PermissionlessCrankRequestV16, PermissionlessProgressOutcomeV16,
    PermissionlessRecoveryReasonV16, PortfolioAccountV16Account, PortfolioLegV16,
    PortfolioLegV16Account, PortfolioSourceDomainV16Account, PortfolioV16View, PortfolioV16ViewMut,
    ProvenanceHeaderV16, ProvenanceHeaderV16Account, ResolvedPayoutLedgerV16,
    ResolvedPayoutLedgerV16Account, ResolvedPayoutReceiptV16, ResolvedPayoutReceiptV16Account,
    SideV16, SourceCreditStateV16, SourceCreditStateV16Account, TradeRequestV16, V16Config,
    V16Error, V16PodI128, V16PodU128, V16PodU32, V16PodU64,
};
use percolator::{ADL_ONE, BOUND_SCALE, CREDIT_RATE_SCALE, POS_SCALE};

fn ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    ([1; 32], [2; 32], [3; 32])
}

fn market_fixture(
    market_slots: u32,
    init_price: u64,
) -> (MarketGroupV16HeaderAccount, Vec<Market<u64>>) {
    let (market_id, _, _) = ids();
    let cfg =
        V16Config::public_user_fund_with_market_slots(market_slots as u16, market_slots, 0, 10);
    let mut header =
        MarketGroupV16HeaderAccount::new_dynamic(market_id, cfg, market_slots, 0).unwrap();
    let mut markets = (0..market_slots)
        .map(|i| Market::new(i as u64, EngineAssetSlotV16Account::default()))
        .collect::<Vec<_>>();
    {
        let mut view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        for i in 0..market_slots as usize {
            view.activate_empty_market_not_atomic(i as u32, init_price, (i + 1) as u64)
                .unwrap();
        }
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
    PortfolioAccountV16Account::try_empty(header).unwrap()
}

#[test]
fn v16_public_fund_validator_accepts_nontrivial_exact_solvency_profile() {
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 1, 10);
    cfg.maintenance_margin_bps = 10_000;
    cfg.initial_margin_bps = 10_000;
    cfg.max_price_move_bps_per_slot = 100;
    cfg.max_accrual_dt_slots = 1;
    cfg.min_funding_lifetime_slots = 1;
    cfg.max_abs_funding_e9_per_slot = 0;
    cfg.liquidation_fee_bps = 100;
    cfg.min_liquidation_abs = 1;
    cfg.liquidation_fee_cap = 1;
    cfg.min_nonzero_mm_req = 2;
    cfg.min_nonzero_im_req = 3;

    assert_eq!(cfg.validate_public_user_fund(), Ok(()));
}

#[test]
fn v16_view_deposit_and_withdraw_are_the_tested_paths() {
    let (mut header, mut markets) = market_fixture(1, 100);
    let mut account_header = account_fixture(1, 2);
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account_view = PortfolioV16ViewMut::new(&mut account_header);

    market_view
        .deposit_not_atomic(&mut account_view, 11)
        .unwrap();
    market_view
        .withdraw_not_atomic(&mut account_view, 4)
        .unwrap();

    assert_eq!(account_view.header.capital.get(), 7);
    assert_eq!(market_view.header.c_tot.get(), 7);
    assert_eq!(market_view.header.vault.get(), 7);
    market_view.validate_shape().unwrap();
    account_view
        .validate_with_market(&market_view.as_view())
        .unwrap();
}

#[test]
fn v16_view_fee_sync_settles_flat_loss_before_fee() {
    let (mut header, mut markets) = market_fixture(1, 100);
    let mut account_header = account_fixture(1, 4);
    header.vault = V16PodU128::new(100);
    header.c_tot = V16PodU128::new(100);
    header.negative_pnl_account_count = V16PodU64::new(1);
    header.current_slot = V16PodU64::new(10);
    header.slot_last = V16PodU64::new(10);
    account_header.capital = V16PodU128::new(100);
    account_header.pnl = V16PodI128::new(-40);

    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account_view = PortfolioV16ViewMut::new(&mut account_header);
    let charged = market_view
        .sync_account_fee_to_slot_not_atomic(&mut account_view, 10, 10)
        .unwrap();

    assert_eq!(charged, 60);
    assert_eq!(account_view.header.pnl.get(), 0);
    assert_eq!(account_view.header.capital.get(), 0);
    assert_eq!(market_view.header.c_tot.get(), 0);
    assert_eq!(market_view.header.insurance.get(), 60);
    assert_eq!(market_view.header.vault.get(), 100);
    assert_eq!(market_view.header.negative_pnl_account_count.get(), 0);
}

#[test]
fn v16_fee_sync_on_nonflat_account_settles_hidden_k_loss_before_fee() {
    let (mut header, mut markets) = market_fixture(1, 100);
    let mut long_header = account_fixture(1, 14);
    let mut short_header = account_fixture(1, 15);
    {
        let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut long = PortfolioV16ViewMut::new(&mut long_header);
        let mut short = PortfolioV16ViewMut::new(&mut short_header);
        market.deposit_not_atomic(&mut long, 100).unwrap();
        market.deposit_not_atomic(&mut short, 1_000).unwrap();
        market
            .execute_trade_with_fee_in_place_not_atomic(
                &mut long,
                &mut short,
                TradeRequestV16 {
                    asset_index: 0,
                    size_q: POS_SCALE,
                    exec_price: 100,
                    fee_bps: 0,
                },
            )
            .unwrap();
        market
            .accrue_asset_to_not_atomic(0, 2, 50, 0, true)
            .unwrap();
    }
    assert_eq!(long_header.pnl.get(), 0);
    assert_eq!(long_header.capital.get(), 100);
    assert_eq!(header.insurance.get(), 0);

    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut long = PortfolioV16ViewMut::new(&mut long_header);
    let charged = market
        .sync_account_fee_to_slot_not_atomic(&mut long, 2, 100)
        .unwrap();

    assert_eq!(
        charged, 50,
        "lazy K loss must consume principal before recurring fee collection"
    );
    assert_eq!(long.header.capital.get(), 0);
    assert_eq!(long.header.pnl.get(), 0);
    assert_eq!(market.header.insurance.get(), 50);
    market.validate_shape().unwrap();
    long.validate_with_market(&market.as_view()).unwrap();
}

#[test]
fn v16_view_dynamic_market_slots_can_be_activated_without_runtime_vec_engine() {
    let (mut header, mut markets) = market_fixture(3, 100);
    let view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    view.validate_shape().unwrap();

    assert_eq!(
        view.header
            .config
            .try_to_runtime()
            .unwrap()
            .max_market_slots,
        3
    );
    assert_eq!(view.markets.len(), 3);
    assert_eq!(view.markets[2].engine.asset.market_id.get(), 3);
    assert_eq!(view.markets[2].engine.asset.effective_price.get(), 100);
}

#[test]
fn v16_reused_market_slot_rejects_old_market_id_leg() {
    let (mut header, mut markets) = market_fixture(1, 100);
    let mut account_header = account_fixture(1, 16);
    let old_market_id = markets[0].engine.asset.market_id.get();
    {
        let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        market.retire_empty_asset_not_atomic(0, 1).unwrap();
        market.activate_empty_market_not_atomic(0, 200, 2).unwrap();
    }
    assert_ne!(markets[0].engine.asset.market_id.get(), old_market_id);

    account_header.legs[0] = PortfolioLegV16Account::from_runtime(&PortfolioLegV16 {
        active: true,
        asset_index: 0,
        market_id: old_market_id,
        side: SideV16::Long,
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
    });
    account_header.active_bitmap[0] = V16PodU64::new(1);

    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header);
    assert_eq!(
        market.full_account_refresh_not_atomic(&mut account),
        Err(V16Error::HiddenLeg),
        "stale legs from a retired market slot must not bind to the reactivated market"
    );
    market.validate_shape().unwrap();
}

#[test]
fn v16_view_rejects_overwithdraw() {
    let (mut header, mut markets) = market_fixture(1, 100);
    let mut account_header = account_fixture(1, 6);
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account_view = PortfolioV16ViewMut::new(&mut account_header);
    market_view
        .deposit_not_atomic(&mut account_view, 3)
        .unwrap();

    let err = market_view.withdraw_not_atomic(&mut account_view, 4);

    assert_eq!(err, Err(V16Error::LockActive));
}

#[test]
fn v16_insurance_lien_consume_rejects_fractional_bound_amount() {
    let (mut header, mut markets) = market_fixture(1, 100);
    header.vault = V16PodU128::new(10);
    header.insurance = V16PodU128::new(10);
    markets[0].engine.insurance_domain_budget_long = V16PodU128::new(10);

    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    market
        .reserve_insurance_credit_not_atomic(0, BOUND_SCALE)
        .unwrap();
    market
        .create_source_credit_lien_from_insurance_not_atomic(0, BOUND_SCALE)
        .unwrap();

    let before_insurance = market.header.insurance;
    let before_spent = market.markets[0].engine.insurance_domain_spent_long;
    let before_reservation = market.markets[0].engine.insurance_reservation_long;
    let before_source = market.markets[0].engine.source_credit_long;

    let err = market.consume_source_credit_lien_from_insurance_not_atomic(0, 1);

    assert_eq!(err, Err(V16Error::InvalidConfig));
    assert_eq!(market.header.insurance, before_insurance);
    assert_eq!(
        market.markets[0].engine.insurance_domain_spent_long,
        before_spent
    );
    assert_eq!(
        market.markets[0].engine.insurance_reservation_long,
        before_reservation
    );
    assert_eq!(market.markets[0].engine.source_credit_long, before_source);
}

#[test]
fn v16_public_liquidation_on_unfunded_domain_cannot_drain_shared_insurance() {
    let (mut header, mut markets) = market_fixture(1, 100);
    let mut account_header = account_fixture(1, 10);
    header.vault = V16PodU128::new(50);
    header.insurance = V16PodU128::new(50);
    header.negative_pnl_account_count = V16PodU64::new(1);

    let mut asset = markets[0].engine.asset.try_to_runtime().unwrap();
    asset.oi_eff_long_q = 2 * POS_SCALE;
    asset.oi_eff_short_q = 2 * POS_SCALE;
    asset.loss_weight_sum_long = 2 * POS_SCALE;
    asset.loss_weight_sum_short = 2 * POS_SCALE;
    asset.stored_pos_count_long = 2;
    asset.stored_pos_count_short = 2;
    markets[0].engine.asset = AssetStateV16Account::from_runtime(&asset);

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
        epoch_snap: asset.epoch_long,
        loss_weight: POS_SCALE,
        b_snap: asset.b_long_num,
        b_rem: 0,
        b_epoch_snap: asset.epoch_long,
        b_stale: false,
        stale: false,
    });
    account_header.active_bitmap[0] = V16PodU64::new(1);

    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header);
    let insurance_before = market.header.insurance.get();
    let vault_before = market.header.vault.get();

    let out = market
        .liquidate_account_not_atomic(
            &mut account,
            LiquidationRequestV16 {
                asset_index: 0,
                close_q: POS_SCALE,
                fee_bps: 0,
            },
        )
        .expect("liquidation should progress by booking residual, not draining other domains");

    assert_eq!(out.insurance_used, 0);
    assert_eq!(market.header.insurance.get(), insurance_before);
    assert_eq!(market.header.vault.get(), vault_before);
    assert_eq!(
        market.markets[0].engine.insurance_domain_spent_short.get(),
        0
    );
    assert!(out.residual_booked > 0);
    market.validate_shape().unwrap();
    account.validate_with_market(&market.as_view()).unwrap();
}

#[test]
fn v16_permissionless_liquidation_progresses_when_unrelated_asset_is_loss_stale() {
    let (mut header, mut markets) = market_fixture(2, 100);
    let mut account_header = account_fixture(2, 11);
    header.current_slot = V16PodU64::new(10);
    header.slot_last = V16PodU64::new(9);
    header.loss_stale_active = 1;
    header.vault = V16PodU128::new(50);
    header.insurance = V16PodU128::new(50);
    header.negative_pnl_account_count = V16PodU64::new(1);

    let mut asset0 = markets[0].engine.asset.try_to_runtime().unwrap();
    asset0.slot_last = 10;
    asset0.oi_eff_long_q = 2 * POS_SCALE;
    asset0.oi_eff_short_q = 2 * POS_SCALE;
    asset0.loss_weight_sum_long = 2 * POS_SCALE;
    asset0.loss_weight_sum_short = 2 * POS_SCALE;
    asset0.stored_pos_count_long = 2;
    asset0.stored_pos_count_short = 2;
    markets[0].engine.asset = AssetStateV16Account::from_runtime(&asset0);
    let mut asset1 = markets[1].engine.asset.try_to_runtime().unwrap();
    asset1.slot_last = 9;
    asset1.oi_eff_long_q = POS_SCALE;
    asset1.oi_eff_short_q = POS_SCALE;
    asset1.loss_weight_sum_long = POS_SCALE;
    asset1.loss_weight_sum_short = POS_SCALE;
    asset1.stored_pos_count_long = 1;
    asset1.stored_pos_count_short = 1;
    markets[1].engine.asset = AssetStateV16Account::from_runtime(&asset1);

    account_header.pnl = V16PodI128::new(-5);
    account_header.legs[0] = PortfolioLegV16Account::from_runtime(&PortfolioLegV16 {
        active: true,
        asset_index: 0,
        market_id: asset0.market_id,
        side: SideV16::Long,
        basis_pos_q: POS_SCALE as i128,
        a_basis: ADL_ONE,
        k_snap: asset0.k_long,
        f_snap: asset0.f_long_num,
        epoch_snap: asset0.epoch_long,
        loss_weight: POS_SCALE,
        b_snap: asset0.b_long_num,
        b_rem: 0,
        b_epoch_snap: asset0.epoch_long,
        b_stale: false,
        stale: false,
    });
    account_header.active_bitmap[0] = V16PodU64::new(1);

    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header);
    let outcome = market
        .permissionless_crank_not_atomic(
            &mut account,
            percolator::v16::PermissionlessCrankRequestV16 {
                now_slot: 10,
                asset_index: 0,
                effective_price: 100,
                funding_rate_e9: 0,
                action: percolator::v16::PermissionlessCrankActionV16::Liquidate(
                    LiquidationRequestV16 {
                        asset_index: 0,
                        close_q: POS_SCALE,
                        fee_bps: 0,
                    },
                ),
            },
        )
        .expect(
            "locally current liquidation must progress despite unrelated global loss-staleness",
        );

    assert_eq!(
        outcome,
        percolator::v16::PermissionlessProgressOutcomeV16::AccountCurrent
    );
    assert_eq!(market.header.loss_stale_active, 1);
    assert_eq!(market.header.slot_last.get(), 9);
    assert_eq!(account.header.active_bitmap[0].get(), 0);
    market.validate_shape().unwrap();
    account.validate_with_market(&market.as_view()).unwrap();
}

#[test]
fn v16_permissionless_recovery_crank_is_value_neutral_and_idempotent() {
    let (mut header, mut markets) = market_fixture(1, 100);
    let mut account_header = account_fixture(1, 12);
    {
        let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut account = PortfolioV16ViewMut::new(&mut account_header);
        market.deposit_not_atomic(&mut account, 7).unwrap();
    }
    header.insurance = V16PodU128::new(3);
    header.vault = V16PodU128::new(10);
    let vault_before = header.vault;
    let c_tot_before = header.c_tot;
    let insurance_before = header.insurance;
    let capital_before = account_header.capital;
    let pnl_before = account_header.pnl;

    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header);
    let first = market
        .permissionless_crank_not_atomic(
            &mut account,
            PermissionlessCrankRequestV16 {
                now_slot: 1,
                asset_index: 0,
                effective_price: 100,
                funding_rate_e9: 0,
                action: PermissionlessCrankActionV16::Recover(
                    PermissionlessRecoveryReasonV16::ExplicitLossOrDustAuditOverflow,
                ),
            },
        )
        .unwrap();
    let second = market
        .permissionless_crank_not_atomic(
            &mut account,
            PermissionlessCrankRequestV16 {
                now_slot: 1,
                asset_index: 0,
                effective_price: 100,
                funding_rate_e9: 0,
                action: PermissionlessCrankActionV16::Recover(
                    PermissionlessRecoveryReasonV16::BIndexHeadroomExhausted,
                ),
            },
        )
        .unwrap();
    let refresh_after_recovery = market.permissionless_crank_not_atomic(
        &mut account,
        PermissionlessCrankRequestV16 {
            now_slot: 1,
            asset_index: 0,
            effective_price: 100,
            funding_rate_e9: 0,
            action: PermissionlessCrankActionV16::Refresh,
        },
    );

    assert_eq!(
        first,
        PermissionlessProgressOutcomeV16::RecoveryDeclared(
            PermissionlessRecoveryReasonV16::ExplicitLossOrDustAuditOverflow
        )
    );
    assert_eq!(second, first);
    assert_eq!(refresh_after_recovery, Err(V16Error::LockActive));
    assert_eq!(market.header.vault, vault_before);
    assert_eq!(market.header.c_tot, c_tot_before);
    assert_eq!(market.header.insurance, insurance_before);
    assert_eq!(account.header.capital, capital_before);
    assert_eq!(account.header.pnl, pnl_before);
    market.validate_shape().unwrap();
    account.validate_with_market(&market.as_view()).unwrap();
}

#[test]
fn v16_resolved_payout_topup_finishes_receipt_without_overpaying() {
    let (mut header, mut markets) = market_fixture(1, 100);
    let mut account_header = account_fixture(1, 13);
    {
        let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        market.resolve_market_not_atomic(1).unwrap();
    }
    let terminal_claim = 10u128;
    header.vault = V16PodU128::new(4);
    header.payout_snapshot_captured = 1;
    header.resolved_payout_ledger =
        ResolvedPayoutLedgerV16Account::from_runtime(&ResolvedPayoutLedgerV16 {
            snapshot_residual: terminal_claim,
            terminal_claim_exact_receipts_num: terminal_claim * BOUND_SCALE,
            terminal_claim_bound_unreceipted_num: 0,
            current_payout_rate_num: 1,
            current_payout_rate_den: 1,
            snapshot_slot: 1,
            payout_halted: false,
            finalized: false,
        });
    account_header.resolved_payout_receipt =
        ResolvedPayoutReceiptV16Account::from_runtime(&ResolvedPayoutReceiptV16 {
            present: true,
            prior_bound_contribution_num: terminal_claim * BOUND_SCALE,
            live_released_face_at_receipt: 0,
            terminal_positive_claim_face: terminal_claim,
            paid_effective: 2,
            finalized: false,
        });

    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header);
    let first = market
        .claim_resolved_payout_topup_not_atomic(&mut account)
        .unwrap();
    let after_first = account
        .header
        .resolved_payout_receipt
        .try_to_runtime()
        .unwrap();
    market.header.vault = V16PodU128::new(4);
    let second = market
        .claim_resolved_payout_topup_not_atomic(&mut account)
        .unwrap();
    let after_second = account
        .header
        .resolved_payout_receipt
        .try_to_runtime()
        .unwrap();
    let third = market
        .claim_resolved_payout_topup_not_atomic(&mut account)
        .unwrap();

    assert_eq!(first, 4);
    assert_eq!(after_first.paid_effective, 6);
    assert!(!after_first.finalized);
    assert_eq!(second, 4);
    assert_eq!(after_second.paid_effective, terminal_claim);
    assert!(after_second.finalized);
    assert_eq!(third, 0);
    assert_eq!(market.header.vault.get(), 0);
    market.validate_shape().unwrap();
    account.validate_with_market(&market.as_view()).unwrap();
}

#[test]
fn v16_risk_increasing_trade_creates_source_credit_lien_for_im() {
    let (mut header, mut markets) = market_fixture(1, 1);
    let mut long_header = account_fixture(1, 8);
    let mut short_header = account_fixture(1, 9);
    let claim = 100u128;
    let claim_num = claim * BOUND_SCALE;
    long_header.pnl = V16PodI128::new(claim as i128);
    long_header.source_domains[0].domain = V16PodU32::new(0);
    long_header.source_domains[0].source_claim_market_id = V16PodU64::new(1);
    long_header.source_domains[0].source_claim_bound_num = V16PodU128::new(claim_num);
    header.pnl_pos_tot = V16PodU128::new(claim);
    header.pnl_pos_bound_tot_num = V16PodU128::new(claim_num);
    header.pnl_pos_bound_tot = V16PodU128::new(claim);
    markets[0].engine.source_credit_long =
        SourceCreditStateV16Account::from_runtime(&SourceCreditStateV16 {
            positive_claim_bound_num: claim_num,
            exact_positive_claim_num: claim_num,
            fresh_reserved_backing_num: claim_num,
            credit_rate_num: CREDIT_RATE_SCALE,
            ..SourceCreditStateV16::EMPTY
        });
    markets[0].engine.backing_long = BackingBucketV16Account::from_runtime(&BackingBucketV16 {
        market_id: 1,
        fresh_unliened_backing_num: claim_num,
        expiry_slot: 100,
        status: BackingBucketStatusV16::Fresh,
        ..BackingBucketV16::EMPTY
    });
    {
        let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut short = PortfolioV16ViewMut::new(&mut short_header);
        market.deposit_not_atomic(&mut short, 1_000).unwrap();
    }

    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut long = PortfolioV16ViewMut::new(&mut long_header);
    let mut short = PortfolioV16ViewMut::new(&mut short_header);
    market
        .execute_trade_with_fee_in_place_not_atomic(
            &mut long,
            &mut short,
            TradeRequestV16 {
                asset_index: 0,
                size_q: 10 * POS_SCALE,
                exec_price: 1,
                fee_bps: 0,
            },
        )
        .expect("risk-increasing trade should atomically lien backed source credit for IM");

    assert_eq!(long.header.capital.get(), 0);
    assert_eq!(
        long.header.source_domains[0].source_claim_liened_num.get(),
        10 * BOUND_SCALE
    );
    assert_eq!(
        long.header.source_domains[0]
            .source_lien_effective_reserved
            .get(),
        10
    );
    assert_eq!(
        long.header.source_domains[0]
            .source_lien_counterparty_backing_num
            .get(),
        10 * BOUND_SCALE
    );
    assert_eq!(
        market.markets[0]
            .engine
            .source_credit_long
            .valid_liened_backing_num
            .get(),
        10 * BOUND_SCALE
    );
    assert_eq!(
        market.markets[0]
            .engine
            .backing_long
            .valid_liened_backing_num
            .get(),
        10 * BOUND_SCALE
    );
    assert_eq!(
        market.markets[0]
            .engine
            .backing_long
            .fresh_unliened_backing_num
            .get(),
        90 * BOUND_SCALE
    );
    assert_eq!(
        market.convert_released_pnl_to_capital_not_atomic(&mut long),
        Err(V16Error::LockActive),
        "source-backed positive PnL must not be realized while the source-claim exposure remains open"
    );
    market.validate_shape().unwrap();
    long.validate_with_market(&market.as_view()).unwrap();
    short.validate_with_market(&market.as_view()).unwrap();
}

#[test]
fn v16_source_backed_conversion_clears_sparse_source_domain_slot() {
    let (mut header, mut markets) = market_fixture(1, 1);
    let mut account_header = account_fixture(1, 18);
    let claim = 20u128;
    let claim_num = claim * BOUND_SCALE;
    header.vault = V16PodU128::new(claim);
    header.pnl_pos_tot = V16PodU128::new(claim);
    header.pnl_pos_bound_tot_num = V16PodU128::new(claim_num);
    header.pnl_pos_bound_tot = V16PodU128::new(claim);
    account_header.pnl = V16PodI128::new(claim as i128);
    account_header.source_domains[0].domain = V16PodU32::new(0);
    account_header.source_domains[0].source_claim_market_id = V16PodU64::new(1);
    account_header.source_domains[0].source_claim_bound_num = V16PodU128::new(claim_num);
    markets[0].engine.source_credit_long =
        SourceCreditStateV16Account::from_runtime(&SourceCreditStateV16 {
            positive_claim_bound_num: claim_num,
            exact_positive_claim_num: claim_num,
            fresh_reserved_backing_num: claim_num,
            credit_rate_num: CREDIT_RATE_SCALE,
            ..SourceCreditStateV16::EMPTY
        });
    markets[0].engine.backing_long = BackingBucketV16Account::from_runtime(&BackingBucketV16 {
        market_id: 1,
        fresh_unliened_backing_num: claim_num,
        expiry_slot: 100,
        status: BackingBucketStatusV16::Fresh,
        ..BackingBucketV16::EMPTY
    });

    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header);
    market
        .full_account_refresh_not_atomic(&mut account)
        .unwrap();
    let converted = market
        .convert_released_pnl_to_capital_not_atomic(&mut account)
        .expect("flat source-backed PnL should be convertible when backing is available");

    assert_eq!(converted, claim);
    assert_eq!(account.header.pnl.get(), 0);
    assert_eq!(account.header.capital.get(), claim);
    assert_eq!(
        account.header.source_domains[0],
        PortfolioSourceDomainV16Account::default()
    );
    account.validate_with_market(&market.as_view()).unwrap();
    market.validate_shape().unwrap();
}

#[test]
fn v16_sparse_source_domains_reject_noncompact_hidden_tail() {
    let (mut header, mut markets) = market_fixture(1, 1);
    let mut account_header = account_fixture(1, 19);
    account_header.pnl = V16PodI128::new(1);
    account_header.source_domains[1].domain = V16PodU32::new(0);
    account_header.source_domains[1].source_claim_market_id = V16PodU64::new(1);
    account_header.source_domains[1].source_claim_bound_num = V16PodU128::new(BOUND_SCALE);

    let market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let account = PortfolioV16View::new(&account_header);
    assert_eq!(
        account.validate_with_market(&market.as_view()),
        Err(V16Error::HiddenLeg),
        "a default source-domain hole must not hide later occupied entries from short-circuit scans"
    );
}

#[test]
fn v16_mutable_view_compacts_persisted_domain_indexed_source_claim_before_deposit() {
    let (mut header, mut markets) = market_fixture(1, 1);
    let mut account_header = account_fixture(1, 20);
    let claim = 7u128;
    let claim_num = claim * BOUND_SCALE;
    header.vault = V16PodU128::new(claim);
    header.c_tot = V16PodU128::new(0);
    header.pnl_pos_tot = V16PodU128::new(claim);
    header.pnl_pos_bound_tot_num = V16PodU128::new(claim_num);
    header.pnl_pos_bound_tot = V16PodU128::new(claim);
    account_header.pnl = V16PodI128::new(claim as i128);
    account_header.source_domains[1].domain = V16PodU32::new(1);
    account_header.source_domains[1].source_claim_market_id = V16PodU64::new(1);
    account_header.source_domains[1].source_claim_bound_num = V16PodU128::new(claim_num);
    markets[0].engine.source_credit_short =
        SourceCreditStateV16Account::from_runtime(&SourceCreditStateV16 {
            positive_claim_bound_num: claim_num,
            exact_positive_claim_num: claim_num,
            fresh_reserved_backing_num: claim_num,
            credit_rate_num: CREDIT_RATE_SCALE,
            ..SourceCreditStateV16::EMPTY
        });
    markets[0].engine.backing_short = BackingBucketV16Account::from_runtime(&BackingBucketV16 {
        market_id: 1,
        fresh_unliened_backing_num: claim_num,
        expiry_slot: 100,
        status: BackingBucketStatusV16::Fresh,
        ..BackingBucketV16::EMPTY
    });

    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header);
    market
        .deposit_not_atomic(&mut account, 3)
        .expect("later deposit must accept a persisted parked source claim");

    assert_eq!(account.header.capital.get(), 3);
    assert_eq!(account.header.source_domains[0].domain.get(), 1);
    assert_eq!(
        account.header.source_domains[0]
            .source_claim_bound_num
            .get(),
        claim_num
    );
    assert_eq!(
        account.header.source_domains[1],
        PortfolioSourceDomainV16Account::default()
    );
    account.validate_with_market(&market.as_view()).unwrap();
    market.validate_shape().unwrap();
}
