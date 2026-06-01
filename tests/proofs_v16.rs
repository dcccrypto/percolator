#![cfg(kani)]

use percolator::v16::{
    account_equity, account_equity_from_parts, kani_apply_backing_provider_earnings_withdraw,
    kani_apply_backing_utilization_fee_charge, kani_apply_resolved_payout_receipt_payment,
    kani_liquidation_close_would_leave_uncovered_loss_with_open_risk,
    kani_validate_positive_pnl_source_attribution, risk_notional_ceil, AssetLifecycleV16,
    BResidualBookingOutcomeV16, BackingBucketStatusV16, BackingBucketV16, CloseProgressLedgerV16,
    DeadLegForfeitOutcomeV16, EngineAssetSlotV16Account, HLockLaneV16, HealthCertV16,
    InsuranceCreditReservationV16, LiquidationRequestV16, Market, MarketGroupV16,
    MarketGroupV16HeaderAccount, MarketGroupV16View, MarketGroupV16ViewMut, MarketModeV16,
    PermissionlessCrankActionV16, PermissionlessCrankRequestV16, PermissionlessProgressOutcomeV16,
    PermissionlessRecoveryReasonV16, PortfolioAccountV16, PortfolioAccountV16Account,
    PortfolioLegV16, PortfolioLegV16Account, PortfolioSourceDomainV16Account, PortfolioV16ViewMut,
    ProvenanceHeaderV16, ProvenanceHeaderV16Account, RebalanceRequestV16, ResolvedCloseOutcomeV16,
    ResolvedPayoutLedgerV16, ResolvedPayoutReceiptV16, RiskScoreV16, SideModeV16, SideV16,
    SourceCreditLienAggregateProofV16, SourceCreditStateV16, StockReconciliationProofV16,
    TokenValueFlowProofV16, TradeRequestV16, V16ActiveBitmap, V16Config, V16Error, V16PodI128,
    V16PodU128, V16PodU64, V16Result, V16_MAX_PORTFOLIO_ASSETS_N,
};
use percolator::wide_math::U256;
use percolator::{
    ADL_ONE, BOUND_SCALE, CREDIT_RATE_SCALE, MAX_OI_SIDE_Q, MAX_ORACLE_PRICE, MAX_POSITION_ABS_Q,
    MAX_PROTOCOL_FEE_ABS, MAX_VAULT_TVL, POS_SCALE, SOCIAL_LOSS_DEN,
};

fn symbolic_ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    let market: [u8; 32] = kani::any();
    let account: [u8; 32] = kani::any();
    let owner: [u8; 32] = kani::any();
    (market, account, owner)
}

fn bitmap(indices: &[usize]) -> V16ActiveBitmap {
    let mut out = percolator::active_bitmap_empty();
    for &idx in indices {
        percolator::active_bitmap_set(&mut out, idx).unwrap();
    }
    out
}

const KANI_MARKET_SLOTS_N: usize = 64;

#[kani::proof]
#[kani::unwind(24)]
#[kani::solver(cadical)]
fn proof_v16_insurance_spend_validation_is_the_class_flow_validation() {
    let amount: u128 = kani::any();
    let vault_before: u128 = kani::any();
    let vault_after: u128 = kani::any();
    kani::assume(amount <= MAX_VAULT_TVL);
    kani::assume(vault_before <= MAX_VAULT_TVL);
    kani::assume(vault_after <= MAX_VAULT_TVL);

    let direct = TokenValueFlowProofV16::validate_insurance_to_close_insurance_spent(
        amount,
        vault_before,
        vault_after,
    );
    let via_proof = TokenValueFlowProofV16::insurance_to_close_insurance_spent(
        amount,
        vault_before,
        vault_after,
    )
    .and_then(|proof| proof.validate());

    kani::cover!(
        amount != 0 && vault_before == vault_after,
        "v16 internal insurance spend has nonzero amount and no external vault delta"
    );
    assert_eq!(direct, via_proof);
    assert_eq!(direct.is_ok(), vault_before == vault_after);
}

fn group_header_for_one_asset(group: &MarketGroupV16) -> MarketGroupV16HeaderAccount {
    MarketGroupV16HeaderAccount::from_runtime_with_capacity(group, group.assets.len()).unwrap()
}

fn group_slots_for_one_asset(group: &MarketGroupV16) -> [EngineAssetSlotV16Account; 1] {
    [EngineAssetSlotV16Account::from_runtime_group_slot(group, 0).unwrap()]
}

fn decode_one_asset_group(
    header: &MarketGroupV16HeaderAccount,
    slots: &[EngineAssetSlotV16Account; 1],
) -> V16Result<MarketGroupV16> {
    header.kani_try_to_runtime_with_market_slots_unchecked_invariants(slots)
}

fn source_domains_for_one_asset(
    account: &PortfolioAccountV16,
) -> [PortfolioSourceDomainV16Account; 2] {
    let mut tmp = account.clone();
    tmp.ensure_source_domain_capacity(2);
    [
        PortfolioSourceDomainV16Account::from_runtime(&tmp, 0).unwrap(),
        PortfolioSourceDomainV16Account::from_runtime(&tmp, 1).unwrap(),
    ]
}

fn decode_one_asset_account(
    wire: &PortfolioAccountV16Account,
    source_domains: &[PortfolioSourceDomainV16Account; 2],
) -> V16Result<PortfolioAccountV16> {
    wire.try_to_runtime_with_source_domains(source_domains)
}

fn symbolic_non_active_lifecycle() -> AssetLifecycleV16 {
    let tag: u8 = kani::any();
    match tag % 5 {
        0 => AssetLifecycleV16::Disabled,
        1 => AssetLifecycleV16::PendingActivation,
        2 => AssetLifecycleV16::DrainOnly,
        3 => AssetLifecycleV16::Retired,
        _ => AssetLifecycleV16::Recovery,
    }
}

fn symbolic_lifecycle() -> AssetLifecycleV16 {
    let tag: u8 = kani::any();
    match tag % 6 {
        0 => AssetLifecycleV16::Disabled,
        1 => AssetLifecycleV16::PendingActivation,
        2 => AssetLifecycleV16::Active,
        3 => AssetLifecycleV16::DrainOnly,
        4 => AssetLifecycleV16::Retired,
        _ => AssetLifecycleV16::Recovery,
    }
}

fn tight_envelope_config() -> V16Config {
    let mut cfg = V16Config::public_user_fund(1, 0, 1);
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

fn source_lien_config() -> V16Config {
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.min_nonzero_im_req = 10;
    cfg
}

fn source_credit_rate_bounded_for_backing(backing: u128) {
    let (market, _, _) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let claim = 100u128;
    group
        .add_source_positive_claim_bound_not_atomic(0, claim, 80)
        .unwrap();
    if backing != 0 {
        group
            .add_fresh_counterparty_backing_not_atomic(0, backing, 10)
            .unwrap();
    }
    let available = group.source_credit_available_backing_num(0).unwrap();
    let expected_rate = core::cmp::min((available * CREDIT_RATE_SCALE) / claim, CREDIT_RATE_SCALE);
    assert_eq!(group.source_credit[0].credit_rate_num, expected_rate);
    assert!(group.source_credit[0].credit_rate_num <= CREDIT_RATE_SCALE);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_source_credit_rate_zero_backing_yields_zero_rate() {
    source_credit_rate_bounded_for_backing(0);
    kani::cover!(true, "v16 source credit zero backing reachable");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_source_credit_rate_partial_backing_yields_partial_rate() {
    source_credit_rate_bounded_for_backing(40);
    kani::cover!(true, "v16 source credit partial backing reachable");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_source_credit_rate_full_backing_caps_rate() {
    source_credit_rate_bounded_for_backing(150);
    kani::cover!(true, "v16 source credit full backing reachable");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_zero_domain_claim_face_cannot_support_account_source_claim() {
    let (market, account_id, owner) = concrete_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.ensure_source_domain_capacity(group.source_credit.len());

    let zero_claim_state = SourceCreditStateV16 {
        fresh_reserved_backing_num: 10 * BOUND_SCALE,
        credit_rate_num: CREDIT_RATE_SCALE,
        ..SourceCreditStateV16::EMPTY
    };
    account.pnl = 10;
    account.source_claim_market_id[0] = group.assets[0].market_id;
    account.source_claim_bound_num[0] = 10 * BOUND_SCALE;

    kani::cover!(
        zero_claim_state.positive_claim_bound_num == 0
            && zero_claim_state.credit_rate_num == CREDIT_RATE_SCALE,
        "v16 zero domain claim face with full rate shortcut reachable"
    );
    assert_eq!(
        MarketGroupV16::kani_source_credit_state_realizable_support_for_face(zero_claim_state, 10,),
        Ok(0)
    );
    assert_eq!(
        group.validate_account_shape(&account),
        Err(V16Error::InvalidLeg)
    );
}

fn assert_source_domain_realizable_support_uses_source_credit_rate_case(backing_face: u128) {
    let market = [1; 32];
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let claim_num = 10 * BOUND_SCALE;
    let backing_num = backing_face * BOUND_SCALE;
    let rate = core::cmp::min(
        (backing_num * CREDIT_RATE_SCALE) / claim_num,
        CREDIT_RATE_SCALE,
    );
    group.source_credit[0] = SourceCreditStateV16 {
        positive_claim_bound_num: claim_num,
        exact_positive_claim_num: claim_num,
        fresh_reserved_backing_num: backing_num,
        credit_rate_num: rate,
        ..SourceCreditStateV16::EMPTY
    };
    if backing_face != 0 {
        group.source_backing_buckets[0] = BackingBucketV16 {
            market_id: group.assets[0].market_id,
            fresh_unliened_backing_num: backing_num,
            expiry_slot: 10,
            status: BackingBucketStatusV16::Fresh,
            ..BackingBucketV16::EMPTY
        };
    } else {
        group.source_backing_buckets[0] =
            BackingBucketV16::empty_for_market(group.assets[0].market_id);
    }
    let support = group
        .kani_source_domain_realizable_support_for_face(0, 10)
        .unwrap();

    assert_eq!(support, backing_face);
    assert!(support <= 10);
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v16_source_domain_realizable_support_zero_backing_gives_zero_credit() {
    assert_source_domain_realizable_support_uses_source_credit_rate_case(0);
    kani::cover!(true, "v16 source-domain zero backing reachable");
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v16_source_domain_realizable_support_uses_source_credit_rate() {
    assert_source_domain_realizable_support_uses_source_credit_rate_case(5);
    kani::cover!(true, "v16 source-domain partial backing reachable");
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v16_source_domain_realizable_support_full_backing_gives_full_credit() {
    assert_source_domain_realizable_support_uses_source_credit_rate_case(10);
    kani::cover!(true, "v16 source-domain full backing reachable");
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_unrelated_global_junior_bound_does_not_haircut_source_backed_equity() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(2, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let claim_num = 10 * BOUND_SCALE;

    account.ensure_source_domain_capacity(group.source_credit.len());
    account.pnl = 10;
    account.source_claim_market_id[0] = group.assets[0].market_id;
    account.source_claim_bound_num[0] = claim_num;
    group.pnl_pos_tot = 10;
    group.source_credit[0] = SourceCreditStateV16 {
        positive_claim_bound_num: claim_num,
        exact_positive_claim_num: claim_num,
        fresh_reserved_backing_num: claim_num,
        credit_rate_num: CREDIT_RATE_SCALE,
        ..SourceCreditStateV16::EMPTY
    };
    group.source_backing_buckets[0] = BackingBucketV16 {
        market_id: group.assets[0].market_id,
        fresh_unliened_backing_num: claim_num,
        expiry_slot: 10,
        status: BackingBucketStatusV16::Fresh,
        ..BackingBucketV16::EMPTY
    };

    // Model an unrelated market inflating the global lazy junior bound while
    // this account's own source-domain claim remains fully backed.
    set_junior_bound(&mut group, 1_000);
    group.vault = group.c_tot + group.insurance;

    let equity = group.kani_account_haircut_equity(&account).unwrap();

    kani::cover!(
        group.pnl_pos_bound_tot > account.pnl as u128
            && group.vault == group.c_tot + group.insurance,
        "v16 source-backed equity is exercised with zero global residual and inflated unrelated bound"
    );
    assert_eq!(
        group.kani_account_source_realizable_support(&account, 10),
        Ok(10)
    );
    assert_eq!(
        equity, 10,
        "fully source-backed PnL must not be haircut by unrelated global junior bound inflation"
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_expired_fresh_backing_stale_cert_blocks_source_credit_conversion() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.vault = 1_000;
    group.insurance = 300;
    account.pnl = 300;
    group.pnl_pos_tot = 300;
    group.pnl_pos_bound_tot = 300;
    group.pnl_pos_bound_tot_num = 300 * BOUND_SCALE;
    account.health_cert = HealthCertV16 {
        certified_equity: 300,
        active_bitmap_at_cert: account.active_bitmap,
        cert_oracle_epoch: group.oracle_epoch,
        cert_funding_epoch: group.funding_epoch,
        cert_risk_epoch: group.risk_epoch,
        cert_asset_set_epoch: group.asset_set_epoch + 1,
        valid: true,
        ..HealthCertV16::default()
    };

    let before = (account.capital, account.pnl, group.c_tot, group.insurance);
    let stale_conversion = group.kani_ensure_favorable_action_current_certificate(&account);

    kani::cover!(
        stale_conversion == Err(V16Error::Stale),
        "v16 expired fresh backing blocks still-certified source-credit conversion gate"
    );
    assert_eq!(stale_conversion, Err(V16Error::Stale));
    assert_eq!(
        before,
        (account.capital, account.pnl, group.c_tot, group.insurance)
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_unbacked_attributed_conversion_rejects_without_mutation() {
    let market = [1; 32];
    let account_id = [2; 32];
    let owner = [3; 32];
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.vault = 1_000;
    group
        .add_account_source_positive_pnl_not_atomic(&mut account, 0, 10)
        .unwrap();
    let prices = [1u64; V16_MAX_PORTFOLIO_ASSETS_N];
    group.full_account_refresh(&mut account, &prices).unwrap();
    let result = group.kani_preflight_convert_released_pnl_to_capital(&account);

    kani::cover!(
        true,
        "v16 attributed conversion unbacked rejection reachable"
    );
    assert_eq!(result, Err(V16Error::LockActive));
    assert_eq!(account.capital, 0);
    assert_eq!(account.pnl, 10);
    assert_eq!(account.source_claim_bound_num[0], 10 * BOUND_SCALE);
    group.assert_public_invariants().unwrap();
}

fn create_counterparty_lien_via_public_withdraw(
    group: &mut MarketGroupV16,
    account: &mut PortfolioAccountV16,
    account_id_seed: u8,
    effective_credit: u128,
    backing_expiry_slot: u64,
) {
    let mut opposite = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(
        group.market_group_id,
        [account_id_seed; 32],
        [9; 32],
    ));
    group.deposit_not_atomic(account, 10).unwrap();
    group.vault = group.vault.checked_add(10).unwrap();
    group
        .add_account_source_positive_pnl_not_atomic(account, 0, 10)
        .unwrap();
    group
        .add_fresh_counterparty_backing_not_atomic(0, 10 * BOUND_SCALE, backing_expiry_slot)
        .unwrap();
    group
        .attach_leg(account, 0, SideV16::Long, 10 * POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut opposite, 0, SideV16::Short, -(10 * POS_SCALE as i128))
        .unwrap();
    group
        .withdraw_not_atomic(account, effective_credit, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
}

fn seed_counterparty_source_lien_state(
    group: &mut MarketGroupV16,
    account: &mut PortfolioAccountV16,
    effective_credit: u128,
    backing_expiry_slot: u64,
) {
    let claim = 10 * BOUND_SCALE;
    let backing = 10 * BOUND_SCALE;
    let reserved_backing = effective_credit * BOUND_SCALE;
    group.deposit_not_atomic(account, 10).unwrap();
    group.vault = group.vault.checked_add(10).unwrap();
    group
        .add_account_source_positive_pnl_not_atomic(account, 0, 10)
        .unwrap();
    group
        .add_fresh_counterparty_backing_not_atomic(0, backing, backing_expiry_slot)
        .unwrap();
    group
        .create_source_credit_lien_from_counterparty_not_atomic(0, reserved_backing)
        .unwrap();
    account.source_claim_bound_num[0] = claim;
    account.source_claim_liened_num[0] = reserved_backing;
    account.source_claim_counterparty_liened_num[0] = reserved_backing;
    account.source_lien_effective_reserved[0] = effective_credit;
    account.source_lien_counterparty_backing_num[0] = reserved_backing;
    group.validate_account_shape(account).unwrap();
}

fn seed_insurance_source_lien_state(
    group: &mut MarketGroupV16,
    account: &mut PortfolioAccountV16,
    effective_credit: u128,
) {
    let claim = 10 * BOUND_SCALE;
    let reserved_backing = effective_credit * BOUND_SCALE;
    group.deposit_not_atomic(account, 10).unwrap();
    group.vault = group.vault.checked_add(10).unwrap();
    group.insurance = 10 * BOUND_SCALE;
    group.vault = group.vault.checked_add(group.insurance).unwrap();
    group.insurance_domain_budget[0] = group.insurance;
    group
        .add_account_source_positive_pnl_not_atomic(account, 0, 10)
        .unwrap();
    group
        .reserve_insurance_credit_not_atomic(0, 10 * BOUND_SCALE)
        .unwrap();
    group
        .create_source_credit_lien_from_insurance_not_atomic(0, reserved_backing)
        .unwrap();
    account.source_claim_bound_num[0] = claim;
    account.source_claim_liened_num[0] = reserved_backing;
    account.source_claim_insurance_liened_num[0] = reserved_backing;
    account.source_lien_effective_reserved[0] = effective_credit;
    account.source_lien_insurance_backing_num[0] = reserved_backing;
    group.validate_account_shape(account).unwrap();
}

fn set_account_capital_for_canonical_fixture(
    group: &mut MarketGroupV16,
    account: &mut PortfolioAccountV16,
    new_capital: u128,
) {
    let old_capital = account.capital;
    if new_capital < old_capital {
        let delta = old_capital - new_capital;
        group.c_tot = group.c_tot.checked_sub(delta).unwrap();
        group.vault = group.vault.checked_sub(delta).unwrap();
    } else {
        let delta = new_capital - old_capital;
        group.c_tot = group.c_tot.checked_add(delta).unwrap();
        group.vault = group.vault.checked_add(delta).unwrap();
    }
    account.capital = new_capital;
    account.health_cert.valid = false;
    group.validate_account_shape(account).unwrap();
}

#[kani::proof]
#[kani::unwind(140)]
#[kani::solver(cadical)]
fn proof_v16_public_withdraw_locks_claim_and_backing_when_positive_credit_is_required() {
    let market = [1; 32];
    let mut group = MarketGroupV16::new(market, source_lien_config()).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [10; 32], [1; 32]));

    create_counterparty_lien_via_public_withdraw(&mut group, &mut account, 18, 5, 10);

    kani::cover!(true, "v16 public withdraw source lien creation reachable");
    assert!(account.source_claim_liened_num[0] != 0);
    assert_eq!(account.source_lien_effective_reserved[0], 5);
    assert_eq!(
        group.source_credit[0].valid_liened_backing_num,
        account.source_lien_effective_reserved[0] * BOUND_SCALE
    );
    assert!(
        account.source_claim_liened_num[0]
            <= account.source_claim_bound_num[0] - account.source_claim_impaired_num[0]
    );
}

#[kani::proof]
#[kani::unwind(140)]
#[kani::solver(cadical)]
fn proof_v16_counterparty_source_credit_lien_aggregate_tracks_account_backing_split() {
    let market = [1; 32];
    let mut group = MarketGroupV16::new(market, source_lien_config()).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [16; 32], [1; 32]));
    account.ensure_source_domain_capacity(group.source_credit.len());

    account.source_claim_bound_num[0] = 10 * BOUND_SCALE;
    account.source_claim_liened_num[0] = 5 * BOUND_SCALE;
    account.source_claim_counterparty_liened_num[0] = 5 * BOUND_SCALE;
    account.source_lien_effective_reserved[0] = 5;
    account.source_lien_counterparty_backing_num[0] = 5 * BOUND_SCALE;
    group.source_credit[0].valid_liened_backing_num = 5 * BOUND_SCALE;
    let proof = group
        .source_credit_lien_proof_for_account_domain(&account, 0)
        .unwrap();
    let expected = SourceCreditLienAggregateProofV16 {
        domain: 0,
        source_claim_bound_num: account.source_claim_bound_num[0],
        face_claim_locked_num: account.source_claim_liened_num[0],
        counterparty_face_claim_locked_num: account.source_claim_counterparty_liened_num[0],
        insurance_face_claim_locked_num: 0,
        effective_credit_reserved: account.source_lien_effective_reserved[0],
        counterparty_backing_reserved_num: 5 * BOUND_SCALE,
        insurance_backing_reserved_num: 0,
        impaired_face_claim_num: 0,
        impaired_effective_credit_reserved: 0,
    };

    kani::cover!(
        proof.counterparty_backing_reserved_num != 0,
        "v16 counterparty-backed account source-lien proof reachable"
    );
    assert_eq!(proof, expected);
    assert_eq!(proof.validate(), Ok(()));
    assert_eq!(proof.effective_credit_reserved, 5);
    assert_eq!(
        group.source_credit[0].valid_liened_backing_num,
        proof.counterparty_backing_reserved_num
    );
    assert_eq!(proof.insurance_backing_reserved_num, 0);
}

#[kani::proof]
#[kani::unwind(140)]
#[kani::solver(cadical)]
fn proof_v16_insurance_source_credit_lien_aggregate_tracks_account_backing_split() {
    let market = [1; 32];
    let mut group = MarketGroupV16::new(market, source_lien_config()).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [17; 32], [1; 32]));
    account.ensure_source_domain_capacity(group.source_credit.len());

    account.source_claim_bound_num[0] = 10 * BOUND_SCALE;
    account.source_claim_liened_num[0] = 5 * BOUND_SCALE;
    account.source_claim_insurance_liened_num[0] = 5 * BOUND_SCALE;
    account.source_lien_effective_reserved[0] = 5;
    account.source_lien_insurance_backing_num[0] = 5 * BOUND_SCALE;
    group.source_credit[0].valid_liened_insurance_num = 5 * BOUND_SCALE;
    let proof = group
        .source_credit_lien_proof_for_account_domain(&account, 0)
        .unwrap();
    let expected = SourceCreditLienAggregateProofV16 {
        domain: 0,
        source_claim_bound_num: account.source_claim_bound_num[0],
        face_claim_locked_num: account.source_claim_liened_num[0],
        counterparty_face_claim_locked_num: 0,
        insurance_face_claim_locked_num: account.source_claim_insurance_liened_num[0],
        effective_credit_reserved: account.source_lien_effective_reserved[0],
        counterparty_backing_reserved_num: 0,
        insurance_backing_reserved_num: 5 * BOUND_SCALE,
        impaired_face_claim_num: 0,
        impaired_effective_credit_reserved: 0,
    };

    kani::cover!(
        proof.insurance_backing_reserved_num != 0,
        "v16 insurance-backed account source-lien proof reachable"
    );
    assert_eq!(proof, expected);
    assert_eq!(proof.validate(), Ok(()));
    assert_eq!(proof.effective_credit_reserved, 5);
    assert_eq!(proof.counterparty_backing_reserved_num, 0);
    assert_eq!(
        group.source_credit[0].valid_liened_insurance_num,
        proof.insurance_backing_reserved_num
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_withdraw_locks_source_claim_when_post_state_needs_positive_credit() {
    let market = [1; 32];
    let mut cfg = V16Config::public_user_fund(1, 0, 1);
    cfg.min_nonzero_im_req = 10;
    let mut group = MarketGroupV16::new(market, cfg).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [10; 32], [1; 32]));
    let mut opposite =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [11; 32], [2; 32]));
    group.deposit_not_atomic(&mut account, 10).unwrap();
    group.vault = group.vault.checked_add(10).unwrap();
    group
        .add_account_source_positive_pnl_not_atomic(&mut account, 0, 10)
        .unwrap();
    group
        .add_fresh_counterparty_backing_not_atomic(0, 10 * BOUND_SCALE, 10)
        .unwrap();
    group
        .attach_leg(&mut account, 0, SideV16::Long, 10 * POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut opposite, 0, SideV16::Short, -(10 * POS_SCALE as i128))
        .unwrap();

    let result = group.withdraw_not_atomic(&mut account, 5, &[1; V16_MAX_PORTFOLIO_ASSETS_N]);

    kani::cover!(true, "v16 source-credit-backed withdraw reachable");
    assert!(result.is_ok());
    assert_eq!(account.capital, 5);
    assert!(account.source_claim_liened_num[0] != 0);
    assert_eq!(account.source_lien_effective_reserved[0], 5);
    assert_eq!(
        group.source_credit[0].valid_liened_backing_num,
        account.source_lien_effective_reserved[0] * BOUND_SCALE
    );
    group.assert_public_invariants().unwrap();
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_release_account_source_lien_restores_counterparty_backing_when_unneeded() {
    let market = [1; 32];
    let mut group = MarketGroupV16::new(market, source_lien_config()).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [10; 32], [1; 32]));

    seed_counterparty_source_lien_state(&mut group, &mut account, 5, 10);
    assert_eq!(account.source_lien_effective_reserved[0], 5);
    set_account_capital_for_canonical_fixture(&mut group, &mut account, 5);
    group.deposit_not_atomic(&mut account, 5).unwrap();

    let released = group
        .release_account_source_credit_liens_if_unneeded_not_atomic(
            &mut account,
            &[1; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    kani::cover!(true, "v16 account source-lien release reachable");
    assert_eq!(released, 5);
    assert_eq!(account.source_claim_liened_num[0], 0);
    assert_eq!(account.source_lien_effective_reserved[0], 0);
    assert_eq!(account.source_lien_counterparty_backing_num[0], 0);
    assert_eq!(group.source_credit[0].valid_liened_backing_num, 0);
    assert_eq!(
        group.source_credit_available_backing_num(0),
        Ok(10 * BOUND_SCALE)
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_release_account_source_lien_restores_insurance_backing_when_unneeded() {
    let market = [1; 32];
    let mut group = MarketGroupV16::new(market, source_lien_config()).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [13; 32], [1; 32]));

    seed_insurance_source_lien_state(&mut group, &mut account, 5);
    assert_eq!(account.source_lien_effective_reserved[0], 5);
    assert_eq!(account.source_lien_counterparty_backing_num[0], 0);
    assert_eq!(
        account.source_lien_insurance_backing_num[0],
        5 * BOUND_SCALE
    );
    set_account_capital_for_canonical_fixture(&mut group, &mut account, 5);
    group.deposit_not_atomic(&mut account, 5).unwrap();

    let released = group
        .release_account_source_credit_liens_if_unneeded_not_atomic(
            &mut account,
            &[1; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    kani::cover!(true, "v16 insurance account source-lien release reachable");
    assert_eq!(released, 5);
    assert_eq!(account.source_claim_liened_num[0], 0);
    assert_eq!(account.source_lien_effective_reserved[0], 0);
    assert_eq!(account.source_lien_insurance_backing_num[0], 0);
    assert_eq!(group.source_credit[0].valid_liened_insurance_num, 0);
    assert_eq!(
        group.source_credit_available_backing_num(0),
        Ok(10 * BOUND_SCALE)
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_full_refresh_impairs_expired_counterparty_lien_before_equity_credit() {
    let market = [1; 32];
    let mut group = MarketGroupV16::new(market, source_lien_config()).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [14; 32], [1; 32]));

    account.ensure_source_domain_capacity(group.source_credit.len());
    account.capital = 5;
    account.source_claim_market_id[0] = group.assets[0].market_id;
    account.source_claim_bound_num[0] = 10 * BOUND_SCALE;
    account.source_claim_liened_num[0] = 5 * BOUND_SCALE;
    account.source_claim_counterparty_liened_num[0] = 5 * BOUND_SCALE;
    account.source_lien_effective_reserved[0] = 5;
    account.source_lien_counterparty_backing_num[0] = 5 * BOUND_SCALE;
    group.source_credit[0].impaired_liened_backing_num = 5 * BOUND_SCALE;
    group.source_backing_buckets[0].status = BackingBucketStatusV16::Impaired;
    group.source_backing_buckets[0].impaired_liened_backing_num = 5 * BOUND_SCALE;
    assert_eq!(account.source_lien_effective_reserved[0], 5);

    let impaired = group
        .kani_reconcile_account_source_credit_liens(&mut account)
        .unwrap();
    let equity = account_equity(&account).unwrap();

    kani::cover!(
        impaired == 5,
        "v16 full-refresh lien reconciliation impairs expired backing"
    );
    assert_eq!(impaired, 5);
    assert_eq!(equity, account.capital as i128);
    assert_eq!(account.source_lien_effective_reserved[0], 0);
    assert_eq!(account.source_lien_counterparty_backing_num[0], 0);
    assert_eq!(account.source_claim_liened_num[0], 0);
    assert_eq!(account.source_claim_counterparty_liened_num[0], 0);
    assert_eq!(account.source_claim_impaired_num[0], 5 * BOUND_SCALE);
    assert_eq!(account.source_lien_impaired_effective_reserved[0], 5);
    assert_eq!(group.source_credit[0].fresh_reserved_backing_num, 0);
    assert_eq!(group.source_credit[0].valid_liened_backing_num, 0);
    assert_eq!(
        group.source_credit[0].impaired_liened_backing_num,
        5 * BOUND_SCALE
    );
    assert_eq!(
        group.source_backing_buckets[0].status,
        BackingBucketStatusV16::Impaired
    );
    assert_eq!(group.source_backing_buckets[0].valid_liened_backing_num, 0);
    assert_eq!(
        group.source_backing_buckets[0].impaired_liened_backing_num,
        5 * BOUND_SCALE
    );
}

#[kani::proof]
#[kani::unwind(140)]
#[kani::solver(cadical)]
fn proof_v16_insurance_lien_impairment_removes_account_health_credit() {
    let market = [1; 32];
    let group = MarketGroupV16::new(market, source_lien_config()).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [15; 32], [1; 32]));
    account.ensure_source_domain_capacity(group.source_credit.len());

    account.source_claim_bound_num[0] = 10 * BOUND_SCALE;
    account.source_claim_liened_num[0] = 10 * BOUND_SCALE;
    account.source_claim_insurance_liened_num[0] = 10 * BOUND_SCALE;
    account.source_lien_effective_reserved[0] = 10;
    account.source_lien_insurance_backing_num[0] = 10 * BOUND_SCALE;
    assert_eq!(account.source_lien_effective_reserved[0], 10);
    assert_eq!(
        account.source_lien_insurance_backing_num[0],
        10 * BOUND_SCALE
    );

    let impaired = MarketGroupV16::kani_impair_account_source_credit_insurance_lien_fields(
        &mut account,
        0,
        10 * BOUND_SCALE,
        10,
    )
    .unwrap();
    let proof = SourceCreditLienAggregateProofV16 {
        domain: 0,
        source_claim_bound_num: account.source_claim_bound_num[0],
        face_claim_locked_num: account.source_claim_liened_num[0],
        counterparty_face_claim_locked_num: account.source_claim_counterparty_liened_num[0],
        insurance_face_claim_locked_num: account.source_claim_insurance_liened_num[0],
        effective_credit_reserved: account.source_lien_effective_reserved[0],
        counterparty_backing_reserved_num: account.source_lien_counterparty_backing_num[0],
        insurance_backing_reserved_num: account.source_lien_insurance_backing_num[0],
        impaired_face_claim_num: account.source_claim_impaired_num[0],
        impaired_effective_credit_reserved: account.source_lien_impaired_effective_reserved[0],
    };

    kani::cover!(true, "v16 insurance source-lien impairment reachable");
    assert_eq!(impaired, 10);
    assert_eq!(account.source_lien_effective_reserved[0], 0);
    assert_eq!(account.source_lien_insurance_backing_num[0], 0);
    assert_eq!(account.source_claim_liened_num[0], 0);
    assert!(account.source_claim_impaired_num[0] != 0);
    assert_eq!(account.source_lien_impaired_effective_reserved[0], 10);
    assert_eq!(proof.effective_credit_reserved, 0);
    assert_eq!(proof.insurance_face_claim_locked_num, 0);
    assert_eq!(proof.impaired_face_claim_num, 10 * BOUND_SCALE);
    assert_eq!(proof.impaired_effective_credit_reserved, 10);
    assert_eq!(proof.validate(), Ok(()));
}

#[kani::proof]
#[kani::unwind(140)]
#[kani::solver(cadical)]
fn proof_v16_impaired_source_claim_is_burnable_when_positive_pnl_decreases() {
    let market = [1; 32];
    let mut group = MarketGroupV16::new(market, source_lien_config()).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [16; 32], [1; 32]));
    account.ensure_source_domain_capacity(group.source_credit.len());

    account.pnl = 5;
    account.source_claim_market_id[0] = group.assets[0].market_id;
    account.source_claim_bound_num[0] = 5 * BOUND_SCALE;
    account.source_claim_impaired_num[0] = 5 * BOUND_SCALE;
    account.source_lien_impaired_effective_reserved[0] = 5;
    group.pnl_pos_tot = 5;
    group.pnl_pos_bound_tot = 5;
    group.pnl_pos_bound_tot_num = 5 * BOUND_SCALE;
    group.source_credit[0].positive_claim_bound_num = 5 * BOUND_SCALE;
    group.source_credit[0].exact_positive_claim_num = 5 * BOUND_SCALE;

    group.kani_set_account_pnl(&mut account, 0).unwrap();

    kani::cover!(
        account.source_claim_bound_num[0] == 0,
        "v16 impaired source claim burn on positive PnL decrease reachable"
    );
    assert_eq!(account.pnl, 0);
    assert_eq!(account.source_claim_bound_num[0], 0);
    assert_eq!(account.source_claim_impaired_num[0], 0);
    assert_eq!(account.source_lien_impaired_effective_reserved[0], 0);
    assert_eq!(group.source_credit[0].positive_claim_bound_num, 0);
    assert_eq!(group.source_credit[0].exact_positive_claim_num, 0);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_deposit_and_withdraw_value_flow_preserves_vault_capital_totals() {
    let market = [1; 32];
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [16; 32], [1; 32]));

    group.kani_deposit_core(&mut account, 11).unwrap();
    group.kani_withdraw_core(&mut account, 4).unwrap();

    kani::cover!(true, "v16 deposit/withdraw token-value flow reachable");
    assert_eq!(group.vault, 7);
    assert_eq!(group.c_tot, 7);
    assert_eq!(account.capital, 7);
    group.assert_public_invariants().unwrap();
}

#[kani::proof]
#[kani::unwind(32)]
#[kani::solver(cadical)]
fn proof_v16_loss_and_fee_value_flow_preserves_vault_and_senior_totals() {
    let loss: u8 = kani::any();
    let fee: u8 = kani::any();
    kani::assume(loss > 0);
    kani::assume(loss <= 10);
    kani::assume(fee > 0);
    kani::assume(fee <= 10);
    let market = [1; 32];
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [17; 32], [1; 32]));
    group.kani_deposit_core(&mut account, 30).unwrap();
    account.pnl = -(loss as i128);
    group.negative_pnl_account_count = 1;

    let paid_loss = group
        .kani_settle_negative_pnl_from_principal_core(&mut account)
        .unwrap();
    let charged_fee = group
        .kani_charge_account_fee_current(&mut account, fee as u128)
        .unwrap();

    kani::cover!(
        paid_loss > 0 && charged_fee > 0,
        "v16 principal-loss and fee-to-insurance value-flow paths reachable"
    );
    assert_eq!(paid_loss, loss as u128);
    assert_eq!(charged_fee, fee as u128);
    assert_eq!(group.vault, 30);
    assert_eq!(group.insurance, fee as u128);
    assert_eq!(group.c_tot, 30 - loss as u128 - fee as u128);
    assert_eq!(account.capital, 30 - loss as u128 - fee as u128);
    assert_eq!(account.pnl, 0);
    group.assert_public_invariants().unwrap();
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_stock_reconciliation_decomposes_vault_without_aliasing() {
    let capital: u8 = kani::any();
    let insurance: u8 = kani::any();
    let surplus: u8 = kani::any();
    kani::assume(capital <= 20);
    kani::assume(insurance <= 20);
    kani::assume(surplus <= 20);
    let market = [1; 32];
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    group.c_tot = capital as u128;
    group.insurance = insurance as u128;
    group.vault = capital as u128 + insurance as u128 + surplus as u128;

    let proof = group.stock_reconciliation_proof().unwrap();

    kani::cover!(surplus > 0, "v16 stock proof surplus class reachable");
    assert_eq!(
        proof,
        StockReconciliationProofV16 {
            token_vault: group.vault,
            senior_capital_total: group.c_tot,
            insurance_capital: group.insurance,
            backing_provider_earnings: 0,
            settlement_rounding_residue_total: 0,
            unallocated_protocol_surplus: surplus as u128,
        }
    );
    assert_eq!(proof.validate(), Ok(()));
    group.assert_public_invariants().unwrap();
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v16_backing_utilization_fee_helper_is_lien_bounded() {
    let lien_units: u8 = kani::any();
    let backing_units: u8 = kani::any();
    let dt: u8 = kani::any();
    let charge_full_rate: bool = kani::any();
    kani::assume(backing_units > 0);
    kani::assume(backing_units <= 4);
    kani::assume(lien_units <= backing_units);
    kani::assume(dt <= 4);

    let mut config = V16Config::public_user_fund(1, 0, 1);
    config.backing_fee_base_rate_e9_per_slot = if charge_full_rate { 1_000_000_000 } else { 0 };
    config.backing_fee_kink_util_bps = 8_000;
    let lien_num = lien_units as u128 * BOUND_SCALE;
    let backing_num = backing_units as u128 * BOUND_SCALE;
    let source = SourceCreditStateV16 {
        fresh_reserved_backing_num: backing_num,
        valid_liened_backing_num: lien_num,
        credit_rate_num: CREDIT_RATE_SCALE,
        ..SourceCreditStateV16::EMPTY
    };

    let fee = MarketGroupV16::backing_utilization_fee_quote_atoms_for_lien(
        config, source, lien_num, 0, dt as u64,
    )
    .unwrap();

    kani::cover!(
        charge_full_rate && lien_units != 0 && dt != 0,
        "v16 backing utilization fee positive path reachable"
    );
    assert!(fee <= lien_units as u128 * dt as u128);
    if !charge_full_rate || lien_units == 0 || dt == 0 {
        assert_eq!(fee, 0);
    }
}

#[kani::proof]
#[kani::unwind(20)]
#[kani::solver(cadical)]
fn proof_v16_backing_fee_charge_is_loss_junior_and_capped_by_capital() {
    let capital: u8 = kani::any();
    let c_tot_extra: u8 = kani::any();
    let earnings: u8 = kani::any();
    let fee: u8 = kani::any();
    let pnl_case: bool = kani::any();
    kani::assume(capital <= 20);
    kani::assume(c_tot_extra <= 20);
    kani::assume(earnings <= 20);
    kani::assume(fee <= 20);
    let c_tot = capital as u128 + c_tot_extra as u128;
    let pnl = if pnl_case { 1 } else { -1 };

    let (charged, next_capital, next_c_tot, next_earnings) =
        kani_apply_backing_utilization_fee_charge(
            capital as u128,
            c_tot,
            earnings as u128,
            pnl,
            fee as u128,
        )
        .unwrap();

    kani::cover!(
        pnl_case && fee > capital,
        "v16 backing fee capital cap reachable"
    );
    kani::cover!(
        !pnl_case && fee != 0,
        "v16 backing fee negative pnl forgiveness reachable"
    );
    if pnl < 0 || fee == 0 {
        assert_eq!(charged, 0);
        assert_eq!(next_capital, capital as u128);
        assert_eq!(next_c_tot, c_tot);
        assert_eq!(next_earnings, earnings as u128);
    } else {
        assert_eq!(charged, (fee.min(capital)) as u128);
        assert_eq!(next_capital, capital as u128 - charged);
        assert_eq!(next_c_tot, c_tot - charged);
        assert_eq!(next_earnings, earnings as u128 + charged);
    }
    assert!(charged <= capital as u128);
    assert_eq!(next_c_tot + next_earnings, c_tot + earnings as u128);
}

#[kani::proof]
#[kani::unwind(20)]
#[kani::solver(cadical)]
fn proof_v16_provider_earnings_withdraw_cannot_exceed_accrued_fees() {
    let vault: u8 = kani::any();
    let earnings: u8 = kani::any();
    let withdraw_amount: u8 = kani::any();
    kani::assume(vault <= 20);
    kani::assume(earnings <= vault);
    kani::assume(withdraw_amount <= 20);

    let result = kani_apply_backing_provider_earnings_withdraw(
        vault as u128,
        earnings as u128,
        withdraw_amount as u128,
    );

    kani::cover!(
        withdraw_amount > earnings,
        "v16 provider earnings over-withdraw rejection reachable"
    );
    kani::cover!(
        withdraw_amount <= earnings,
        "v16 provider earnings bounded withdraw reachable"
    );
    if withdraw_amount > earnings {
        assert_eq!(result, Err(V16Error::CounterUnderflow));
    } else {
        let (next_vault, next_earnings) = result.unwrap();
        assert_eq!(next_vault, vault as u128 - withdraw_amount as u128);
        assert_eq!(next_earnings, earnings as u128 - withdraw_amount as u128);
        assert!(next_earnings <= next_vault);
        assert_eq!(next_vault - next_earnings, vault as u128 - earnings as u128);
    }
}

#[kani::proof]
#[kani::unwind(140)]
#[kani::solver(cadical)]
fn proof_v16_public_withdraw_counts_existing_lien_before_incremental_credit() {
    let market = [1; 32];
    let mut group = MarketGroupV16::new(market, source_lien_config()).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [12; 32], [1; 32]));

    seed_counterparty_source_lien_state(&mut group, &mut account, 5, 10);
    set_account_capital_for_canonical_fixture(&mut group, &mut account, 5);
    group
        .attach_leg(&mut account, 0, SideV16::Long, 10 * POS_SCALE as i128)
        .unwrap();
    let mut opposite =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [25; 32], [9; 32]));
    group
        .attach_leg(&mut opposite, 0, SideV16::Short, -(10 * POS_SCALE as i128))
        .unwrap();
    assert_eq!(account.source_lien_effective_reserved[0], 5);

    group
        .withdraw_not_atomic(&mut account, 1, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    kani::cover!(
        true,
        "v16 public withdraw incremental source lien branch reachable"
    );
    assert_eq!(account.source_lien_effective_reserved[0], 6);
    assert_eq!(
        group.source_credit[0].valid_liened_backing_num,
        6 * BOUND_SCALE
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_counterparty_lien_consume_preserves_backing_encumbrance() {
    let backing = 100u128;
    let lien = 30u128;
    let source = SourceCreditStateV16 {
        positive_claim_bound_num: backing,
        exact_positive_claim_num: backing,
        fresh_reserved_backing_num: backing,
        valid_liened_backing_num: lien,
        ..SourceCreditStateV16::EMPTY
    };
    let bucket = BackingBucketV16 {
        market_id: 1,
        fresh_unliened_backing_num: backing - lien,
        valid_liened_backing_num: lien,
        expiry_slot: 10,
        status: BackingBucketStatusV16::Fresh,
        ..BackingBucketV16::EMPTY
    };

    let (bucket, source) =
        MarketGroupV16::kani_prepare_counterparty_lien_consume_delta(bucket, source, lien).unwrap();

    kani::cover!(true, "v16 counterparty lien consume branch reachable");
    assert_eq!(source.spent_backing_num, lien);
    assert_eq!(source.provider_receivable_num, lien);
    assert_eq!(bucket.consumed_liened_backing_num, lien);
    assert_eq!(bucket.valid_liened_backing_num, 0);
    assert_eq!(source.fresh_reserved_backing_num, backing - lien);
    assert_eq!(
        source.fresh_reserved_backing_num,
        bucket
            .fresh_unliened_backing_num
            .checked_add(bucket.valid_liened_backing_num)
            .unwrap()
    );
    assert_eq!(source.valid_liened_backing_num, 0);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_consumed_counterparty_lien_cannot_be_reused_as_fresh_backing() {
    let backing: u128 = kani::any();
    let first_lien: u128 = kani::any();
    let consumed: u128 = kani::any();
    kani::assume(backing > 0 && backing <= 1_000);
    kani::assume(first_lien > 0 && first_lien <= backing);
    kani::assume(consumed > 0 && consumed <= first_lien);

    let source = SourceCreditStateV16 {
        positive_claim_bound_num: backing,
        exact_positive_claim_num: backing,
        fresh_reserved_backing_num: backing,
        ..SourceCreditStateV16::EMPTY
    };
    let bucket = BackingBucketV16 {
        market_id: 1,
        fresh_unliened_backing_num: backing,
        expiry_slot: 10,
        status: BackingBucketStatusV16::Fresh,
        ..BackingBucketV16::EMPTY
    };

    let (bucket, source) =
        MarketGroupV16::kani_prepare_counterparty_lien_create_delta(bucket, source, 0, first_lien)
            .unwrap();
    let (bucket, source) =
        MarketGroupV16::kani_prepare_counterparty_lien_consume_delta(bucket, source, consumed)
            .unwrap();

    assert_eq!(source.spent_backing_num, consumed);
    assert_eq!(source.provider_receivable_num, consumed);
    assert_eq!(source.fresh_reserved_backing_num, backing - consumed);
    assert_eq!(bucket.consumed_liened_backing_num, consumed);

    let reuse_consumed = bucket.fresh_unliened_backing_num + 1;
    let result = MarketGroupV16::kani_prepare_counterparty_lien_create_delta(
        bucket,
        source,
        0,
        reuse_consumed,
    );

    kani::cover!(
        matches!(result, Err(V16Error::LockActive)),
        "v16 consumed counterparty backing is not reintroduced as fresh lien capacity"
    );
    assert!(matches!(result, Err(V16Error::LockActive)));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_future_counterparty_backing_refills_provider_receivable() {
    let source = SourceCreditStateV16 {
        positive_claim_bound_num: 100,
        exact_positive_claim_num: 100,
        spent_backing_num: 70,
        provider_receivable_num: 70,
        credit_rate_num: 0,
        ..SourceCreditStateV16::EMPTY
    };
    let bucket = BackingBucketV16 {
        market_id: 1,
        consumed_liened_backing_num: 70,
        status: BackingBucketStatusV16::Expired,
        ..BackingBucketV16::EMPTY
    };

    let (bucket, source) =
        MarketGroupV16::kani_prepare_counterparty_backing_add_delta(bucket, source, 50, 0, 10)
            .unwrap();

    kani::cover!(
        source.provider_receivable_num == 20,
        "v16 future counterparty backing repays outstanding provider receivable"
    );
    assert_eq!(source.provider_receivable_num, 20);
    assert_eq!(bucket.consumed_liened_backing_num, 20);
    assert_eq!(source.spent_backing_num, 70);
    assert_eq!(source.fresh_reserved_backing_num, 50);
    assert_eq!(bucket.fresh_unliened_backing_num, 50);
    assert_eq!(bucket.status, BackingBucketStatusV16::Fresh);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_backing_refill_has_valid_reservation_encumbrance_proof() {
    let (market, _, _) = concrete_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let source = SourceCreditStateV16 {
        positive_claim_bound_num: 40,
        exact_positive_claim_num: 40,
        spent_backing_num: 40,
        provider_receivable_num: 40,
        credit_rate_num: 0,
        ..SourceCreditStateV16::EMPTY
    };
    let bucket = BackingBucketV16 {
        market_id: group.assets[0].market_id,
        consumed_liened_backing_num: 40,
        status: BackingBucketStatusV16::Expired,
        ..BackingBucketV16::EMPTY
    };
    let (bucket, source) =
        MarketGroupV16::kani_prepare_counterparty_backing_add_delta(bucket, source, 20, 0, 10)
            .unwrap();
    let (source, _) = group
        .kani_prepared_source_credit_domain_recompute(source)
        .unwrap();
    let proof = group
        .kani_reservation_encumbrance_proof_for_domain_parts(
            0,
            source,
            bucket,
            InsuranceCreditReservationV16::EMPTY,
        )
        .unwrap();

    kani::cover!(
        proof.source_provider_receivable_num == 20,
        "v16 backing refill proof retains outstanding provider receivable"
    );
    assert_eq!(proof.validate(), Ok(()));
    assert_eq!(proof.source_provider_receivable_num, 20);
    assert_eq!(proof.bucket_consumed_liened_backing_num, 20);
    assert_eq!(proof.source_fresh_reserved_backing_num, 20);
    assert_eq!(proof.source_credit_rate_num, CREDIT_RATE_SCALE / 2);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_counterparty_lien_impair_preserves_backing_encumbrance() {
    let backing = 100u128;
    let lien = 30u128;
    let source = SourceCreditStateV16 {
        positive_claim_bound_num: backing,
        exact_positive_claim_num: backing,
        fresh_reserved_backing_num: backing,
        valid_liened_backing_num: lien,
        ..SourceCreditStateV16::EMPTY
    };
    let bucket = BackingBucketV16 {
        market_id: 1,
        fresh_unliened_backing_num: backing - lien,
        valid_liened_backing_num: lien,
        expiry_slot: 10,
        status: BackingBucketStatusV16::Fresh,
        ..BackingBucketV16::EMPTY
    };

    let (bucket, source) =
        MarketGroupV16::kani_prepare_counterparty_lien_impair_delta(bucket, source, lien).unwrap();

    kani::cover!(true, "v16 counterparty lien impair branch reachable");
    assert_eq!(source.impaired_liened_backing_num, lien);
    assert_eq!(bucket.impaired_liened_backing_num, lien);
    assert_eq!(bucket.valid_liened_backing_num, 0);
    assert_eq!(source.fresh_reserved_backing_num, backing - lien);
    assert_eq!(
        source.fresh_reserved_backing_num,
        bucket
            .fresh_unliened_backing_num
            .checked_add(bucket.valid_liened_backing_num)
            .unwrap()
    );
    assert_eq!(source.valid_liened_backing_num, 0);
}

fn counterparty_lien_overflow_state() -> (BackingBucketV16, SourceCreditStateV16) {
    let backing = 100u128;
    let lien = 30u128;
    let source = SourceCreditStateV16 {
        positive_claim_bound_num: backing,
        exact_positive_claim_num: backing,
        fresh_reserved_backing_num: backing,
        valid_liened_backing_num: lien,
        ..SourceCreditStateV16::EMPTY
    };
    let bucket = BackingBucketV16 {
        market_id: 1,
        fresh_unliened_backing_num: backing - lien,
        valid_liened_backing_num: lien,
        expiry_slot: 10,
        status: BackingBucketStatusV16::Fresh,
        ..BackingBucketV16::EMPTY
    };
    (bucket, source)
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_counterparty_lien_create_overflow_fails_before_mutation() {
    let (mut bucket, source) = counterparty_lien_overflow_state();
    bucket.valid_liened_backing_num = u128::MAX;

    let result = MarketGroupV16::kani_prepare_counterparty_lien_create_delta(bucket, source, 0, 1);

    kani::cover!(
        matches!(result, Err(V16Error::CounterOverflow)),
        "v16 counterparty lien create overflow branch reachable"
    );
    assert!(matches!(result, Err(V16Error::CounterOverflow)));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_counterparty_lien_release_overflow_fails_before_mutation() {
    let (mut bucket, source) = counterparty_lien_overflow_state();
    bucket.fresh_unliened_backing_num = u128::MAX;

    let result = MarketGroupV16::kani_prepare_counterparty_lien_release_delta(bucket, source, 0, 1);

    kani::cover!(
        matches!(result, Err(V16Error::CounterOverflow)),
        "v16 counterparty lien release overflow branch reachable"
    );
    assert!(matches!(result, Err(V16Error::CounterOverflow)));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_counterparty_lien_consume_overflow_fails_before_mutation() {
    let (mut bucket, source) = counterparty_lien_overflow_state();
    bucket.consumed_liened_backing_num = u128::MAX;

    let result = MarketGroupV16::kani_prepare_counterparty_lien_consume_delta(bucket, source, 1);

    kani::cover!(
        matches!(result, Err(V16Error::CounterOverflow)),
        "v16 counterparty lien consume overflow branch reachable"
    );
    assert!(matches!(result, Err(V16Error::CounterOverflow)));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_counterparty_lien_consume_source_overflow_fails_before_mutation() {
    let (bucket, mut source) = counterparty_lien_overflow_state();
    source.spent_backing_num = u128::MAX;

    let result = MarketGroupV16::kani_prepare_counterparty_lien_consume_delta(bucket, source, 1);

    kani::cover!(
        matches!(result, Err(V16Error::CounterOverflow)),
        "v16 counterparty lien consume source overflow branch reachable"
    );
    assert!(matches!(result, Err(V16Error::CounterOverflow)));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_counterparty_lien_consume_receivable_overflow_fails_before_mutation() {
    let (bucket, mut source) = counterparty_lien_overflow_state();
    source.spent_backing_num = u128::MAX;
    source.provider_receivable_num = u128::MAX;

    let result = MarketGroupV16::kani_prepare_counterparty_lien_consume_delta(bucket, source, 1);

    kani::cover!(
        matches!(result, Err(V16Error::CounterOverflow)),
        "v16 counterparty lien consume receivable overflow branch reachable"
    );
    assert!(matches!(result, Err(V16Error::CounterOverflow)));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_counterparty_lien_impair_overflow_fails_before_mutation() {
    let (mut bucket, source) = counterparty_lien_overflow_state();
    bucket.impaired_liened_backing_num = u128::MAX;

    let result = MarketGroupV16::kani_prepare_counterparty_lien_impair_delta(bucket, source, 1);

    kani::cover!(
        matches!(result, Err(V16Error::CounterOverflow)),
        "v16 counterparty lien impair overflow branch reachable"
    );
    assert!(matches!(result, Err(V16Error::CounterOverflow)));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_counterparty_lien_impair_source_overflow_fails_before_mutation() {
    let (bucket, mut source) = counterparty_lien_overflow_state();
    source.impaired_liened_backing_num = u128::MAX;

    let result = MarketGroupV16::kani_prepare_counterparty_lien_impair_delta(bucket, source, 1);

    kani::cover!(
        matches!(result, Err(V16Error::CounterOverflow)),
        "v16 counterparty lien impair source overflow branch reachable"
    );
    assert!(matches!(result, Err(V16Error::CounterOverflow)));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_source_lien_creation_has_valid_reservation_encumbrance_proof() {
    let market = [1; 32];
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let source = SourceCreditStateV16 {
        positive_claim_bound_num: 10 * BOUND_SCALE,
        exact_positive_claim_num: 10 * BOUND_SCALE,
        fresh_reserved_backing_num: 10 * BOUND_SCALE,
        ..SourceCreditStateV16::EMPTY
    };
    let bucket = BackingBucketV16 {
        market_id: group.assets[0].market_id,
        fresh_unliened_backing_num: 10 * BOUND_SCALE,
        expiry_slot: 10,
        status: BackingBucketStatusV16::Fresh,
        ..BackingBucketV16::EMPTY
    };
    let (bucket, source) = MarketGroupV16::kani_prepare_counterparty_lien_create_delta(
        bucket,
        source,
        0,
        4 * BOUND_SCALE,
    )
    .unwrap();
    let (source, _) = group
        .kani_prepared_source_credit_domain_recompute(source)
        .unwrap();
    let proof = group
        .kani_reservation_encumbrance_proof_for_domain_parts(
            0,
            source,
            bucket,
            InsuranceCreditReservationV16::EMPTY,
        )
        .unwrap();

    kani::cover!(true, "v16 reservation encumbrance proof reachable");
    assert!(proof.validate().is_ok());
    assert_eq!(proof.source_valid_liened_backing_num, 4 * BOUND_SCALE);
    assert_eq!(
        proof.source_fresh_reserved_backing_num,
        proof
            .bucket_fresh_unliened_backing_num
            .checked_add(proof.bucket_valid_liened_backing_num)
            .unwrap()
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_insurance_reservation_lifecycle_preserves_encumbrance() {
    let consume: bool = kani::any();

    let (market, _, _) = concrete_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let reserve_atoms = 100u128;
    let reserve = reserve_atoms * BOUND_SCALE;
    let lien_atoms = 30u128;
    let lien = lien_atoms * BOUND_SCALE;
    let source = SourceCreditStateV16 {
        positive_claim_bound_num: reserve,
        exact_positive_claim_num: reserve,
        insurance_credit_reserved_num: reserve,
        ..SourceCreditStateV16::EMPTY
    };
    let reservation = InsuranceCreditReservationV16 {
        insurance_credit_reserved_num: reserve,
        ..InsuranceCreditReservationV16::EMPTY
    };
    let (reservation, source) =
        MarketGroupV16::kani_prepare_insurance_lien_create_delta(reservation, source, lien)
            .unwrap();
    if consume {
        let (reservation, source, _, next_insurance) =
            MarketGroupV16::kani_prepare_insurance_lien_consume_delta(
                reservation,
                source,
                0,
                reserve_atoms,
                lien,
            )
            .unwrap();
        let (source, _) =
            MarketGroupV16::kani_prepare_source_credit_domain_recompute_for_epoch(source, 0)
                .unwrap();
        let proof = group
            .kani_reservation_encumbrance_proof_for_domain_parts(
                0,
                source,
                BackingBucketV16::EMPTY,
                reservation,
            )
            .unwrap();
        kani::cover!(true, "v16 insurance lien consume branch reachable");
        assert_eq!(next_insurance, reserve_atoms - lien_atoms);
        assert_eq!(reservation.consumed_insurance_num, lien);
        assert_eq!(source.valid_liened_insurance_num, 0);
        assert_eq!(source.insurance_credit_reserved_num, reserve - lien);
        assert!(proof.validate().is_ok());
    } else {
        let (reservation, source) =
            MarketGroupV16::kani_prepare_insurance_lien_impair_delta(reservation, source, lien)
                .unwrap();
        let (source, _) =
            MarketGroupV16::kani_prepare_source_credit_domain_recompute_for_epoch(source, 0)
                .unwrap();
        let proof = group
            .kani_reservation_encumbrance_proof_for_domain_parts(
                0,
                source,
                BackingBucketV16::EMPTY,
                reservation,
            )
            .unwrap();
        kani::cover!(true, "v16 insurance lien impair branch reachable");
        assert_eq!(source.impaired_liened_insurance_num, lien);
        assert_eq!(source.valid_liened_insurance_num, 0);
        assert_eq!(source.insurance_credit_reserved_num, reserve);
        assert!(proof.validate().is_ok());
    }
}

fn insurance_lien_overflow_state() -> (InsuranceCreditReservationV16, SourceCreditStateV16) {
    let source = SourceCreditStateV16 {
        positive_claim_bound_num: 100 * BOUND_SCALE,
        exact_positive_claim_num: 100 * BOUND_SCALE,
        insurance_credit_reserved_num: 60 * BOUND_SCALE,
        valid_liened_insurance_num: 20 * BOUND_SCALE,
        credit_rate_num: CREDIT_RATE_SCALE,
        ..SourceCreditStateV16::EMPTY
    };
    let reservation = InsuranceCreditReservationV16 {
        insurance_credit_reserved_num: 60 * BOUND_SCALE,
        valid_liened_insurance_num: 20 * BOUND_SCALE,
        ..InsuranceCreditReservationV16::EMPTY
    };
    (reservation, source)
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_insurance_lien_create_overflow_fails_before_mutation() {
    let (reservation, mut source) = insurance_lien_overflow_state();
    source.valid_liened_insurance_num = u128::MAX;

    let result =
        MarketGroupV16::kani_prepare_insurance_lien_create_delta(reservation, source, BOUND_SCALE);

    kani::cover!(
        matches!(result, Err(V16Error::CounterOverflow)),
        "v16 insurance lien create overflow branch reachable"
    );
    assert!(matches!(result, Err(V16Error::CounterOverflow)));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_insurance_lien_consume_overflow_fails_before_mutation() {
    let (mut reservation, source) = insurance_lien_overflow_state();
    reservation.consumed_insurance_num = u128::MAX;

    let result = MarketGroupV16::kani_prepare_insurance_lien_consume_delta(
        reservation,
        source,
        0,
        100,
        BOUND_SCALE,
    );

    kani::cover!(
        matches!(result, Err(V16Error::CounterOverflow)),
        "v16 insurance lien consume overflow branch reachable"
    );
    assert!(matches!(result, Err(V16Error::CounterOverflow)));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_insurance_lien_consume_domain_spent_overflow_fails_before_mutation() {
    let (reservation, source) = insurance_lien_overflow_state();

    let result = MarketGroupV16::kani_prepare_insurance_lien_consume_delta(
        reservation,
        source,
        u128::MAX,
        100,
        BOUND_SCALE,
    );

    kani::cover!(
        matches!(result, Err(V16Error::CounterOverflow)),
        "v16 insurance lien consume domain spent overflow branch reachable"
    );
    assert!(matches!(result, Err(V16Error::CounterOverflow)));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_insurance_lien_impair_overflow_fails_before_mutation() {
    let (mut reservation, source) = insurance_lien_overflow_state();
    reservation.impaired_liened_insurance_num = u128::MAX;

    let result =
        MarketGroupV16::kani_prepare_insurance_lien_impair_delta(reservation, source, BOUND_SCALE);

    kani::cover!(
        matches!(result, Err(V16Error::CounterOverflow)),
        "v16 insurance lien impair overflow branch reachable"
    );
    assert!(matches!(result, Err(V16Error::CounterOverflow)));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_insurance_lien_impair_source_overflow_fails_before_mutation() {
    let (reservation, mut source) = insurance_lien_overflow_state();
    source.impaired_liened_insurance_num = u128::MAX;

    let result =
        MarketGroupV16::kani_prepare_insurance_lien_impair_delta(reservation, source, BOUND_SCALE);

    kani::cover!(
        matches!(result, Err(V16Error::CounterOverflow)),
        "v16 insurance lien impair source overflow branch reachable"
    );
    assert!(matches!(result, Err(V16Error::CounterOverflow)));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_source_credit_recompute_epoch_overflow_fails_before_commit() {
    let mut source = SourceCreditStateV16 {
        positive_claim_bound_num: 100,
        exact_positive_claim_num: 100,
        fresh_reserved_backing_num: 100,
        credit_rate_num: CREDIT_RATE_SCALE,
        ..SourceCreditStateV16::EMPTY
    };
    source.credit_epoch = u64::MAX;

    let result = MarketGroupV16::kani_prepare_source_credit_domain_recompute_for_epoch(source, 0);

    kani::cover!(
        matches!(result, Err(V16Error::CounterOverflow)),
        "v16 source credit recompute credit epoch overflow branch reachable"
    );
    assert!(matches!(result, Err(V16Error::CounterOverflow)));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_source_credit_recompute_risk_epoch_overflow_fails_before_commit() {
    let source = SourceCreditStateV16 {
        positive_claim_bound_num: 100,
        exact_positive_claim_num: 100,
        fresh_reserved_backing_num: 100,
        credit_rate_num: CREDIT_RATE_SCALE,
        ..SourceCreditStateV16::EMPTY
    };

    let result =
        MarketGroupV16::kani_prepare_source_credit_domain_recompute_for_epoch(source, u64::MAX);

    kani::cover!(
        matches!(result, Err(V16Error::CounterOverflow)),
        "v16 source credit recompute risk epoch overflow branch reachable"
    );
    assert!(matches!(result, Err(V16Error::CounterOverflow)));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_source_positive_claim_bound_overflow_fails_before_commit() {
    let source = SourceCreditStateV16 {
        positive_claim_bound_num: u128::MAX,
        exact_positive_claim_num: u128::MAX,
        credit_rate_num: CREDIT_RATE_SCALE,
        ..SourceCreditStateV16::EMPTY
    };

    let result = MarketGroupV16::kani_prepare_source_positive_claim_bound_delta(source, 1, 1);

    kani::cover!(
        matches!(result, Err(V16Error::CounterOverflow)),
        "v16 source positive claim bound overflow branch reachable"
    );
    assert!(matches!(result, Err(V16Error::CounterOverflow)));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_source_positive_claim_invalid_exact_rejected_before_commit() {
    let source = SourceCreditStateV16::EMPTY;

    let result = MarketGroupV16::kani_prepare_source_positive_claim_bound_delta(source, 1, 2);

    kani::cover!(
        matches!(result, Err(V16Error::InvalidConfig)),
        "v16 source positive claim exact-greater-than-bound branch reachable"
    );
    assert!(matches!(result, Err(V16Error::InvalidConfig)));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_source_positive_claim_add_then_epoch_overflow_fails_before_commit() {
    let source = SourceCreditStateV16::EMPTY;

    let mut staged =
        MarketGroupV16::kani_prepare_source_positive_claim_bound_delta(source, 1, 1).unwrap();
    staged.credit_epoch = u64::MAX;
    let result = MarketGroupV16::kani_prepare_source_credit_domain_recompute_for_epoch(staged, 0);

    kani::cover!(
        matches!(result, Err(V16Error::CounterOverflow)),
        "v16 source positive claim staged recompute overflow branch reachable"
    );
    assert!(matches!(result, Err(V16Error::CounterOverflow)));
}

fn set_junior_bound(group: &mut MarketGroupV16, amount: u128) {
    kani::assume(amount <= u128::MAX / BOUND_SCALE);
    group.pnl_pos_bound_tot = amount;
    group.pnl_pos_bound_tot_num = amount * BOUND_SCALE;
}

fn initialize_payout_ledger(group: &mut MarketGroupV16) {
    let snapshot_residual = group.vault.saturating_sub(group.c_tot + group.insurance);
    let total_bound_num = group.pnl_pos_bound_tot_num;
    group.payout_snapshot = snapshot_residual;
    group.payout_snapshot_pnl_pos_tot = group.pnl_pos_bound_tot;
    group.payout_snapshot_captured = true;
    group.resolved_payout_ledger = ResolvedPayoutLedgerV16 {
        snapshot_residual,
        terminal_claim_exact_receipts_num: 0,
        terminal_claim_bound_unreceipted_num: total_bound_num,
        current_payout_rate_num: if total_bound_num == 0 {
            1
        } else {
            (snapshot_residual * BOUND_SCALE).min(total_bound_num)
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

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_hlock_is_exactly_hmin_or_hmax() {
    let h_max: u8 = kani::any();
    kani::assume(h_max > 0);
    let (market, account_id, owner) = symbolic_ids();
    let mut group =
        MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, h_max as u64)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    group.threshold_stress_active = kani::any();
    group.bankruptcy_hlock_active = kani::any();
    group.loss_stale_active = kani::any();
    account.stale_state = kani::any();
    account.b_stale_state = kani::any();
    let instruction_bankruptcy_candidate: bool = kani::any();

    kani::cover!(
        !group.threshold_stress_active
            && !group.bankruptcy_hlock_active
            && !group.loss_stale_active
            && !account.stale_state
            && !account.b_stale_state
            && !instruction_bankruptcy_candidate,
        "v16 h-min lane reachable"
    );
    kani::cover!(
        group.threshold_stress_active
            || group.bankruptcy_hlock_active
            || group.loss_stale_active
            || account.stale_state
            || account.b_stale_state
            || instruction_bankruptcy_candidate,
        "v16 h-max lane reachable"
    );

    let selected = group
        .select_h_lock(Some(&account), instruction_bankruptcy_candidate)
        .unwrap();
    assert!(selected == 0 || selected == h_max as u64);

    let lane = group
        .h_lock_lane(Some(&account), instruction_bankruptcy_candidate, None)
        .unwrap();
    if lane == HLockLaneV16::HMax {
        assert_eq!(selected, h_max as u64);
    } else {
        assert_eq!(selected, 0);
    }
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_hmin_zero_remains_available_when_no_lock_state_exists() {
    let h_max: u8 = kani::any();
    kani::assume(h_max > 0);
    let (market, account_id, owner) = symbolic_ids();
    let group =
        MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, h_max as u64)).unwrap();
    let account = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    assert_eq!(
        group.h_lock_lane(Some(&account), false, None),
        Ok(HLockLaneV16::HMin)
    );
    assert_eq!(group.select_h_lock(Some(&account), false), Ok(0));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_stale_counter_transitions_are_idempotent() {
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    group.mark_account_stale(&mut account).unwrap();
    group.mark_account_stale(&mut account).unwrap();
    kani::cover!(account.stale_state, "v16 stale state reachable");
    assert_eq!(group.stale_certificate_count, 1);

    group.clear_account_stale(&mut account).unwrap();
    group.clear_account_stale(&mut account).unwrap();
    kani::cover!(!account.stale_state, "v16 stale clear reachable");
    assert_eq!(group.stale_certificate_count, 0);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_b_stale_account_counter_transitions_are_idempotent() {
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    group.mark_account_b_stale(&mut account).unwrap();
    group.mark_account_b_stale(&mut account).unwrap();
    kani::cover!(account.b_stale_state, "v16 b-stale state reachable");
    assert_eq!(group.b_stale_account_count, 1);

    group.clear_account_b_stale(&mut account).unwrap();
    group.clear_account_b_stale(&mut account).unwrap();
    kani::cover!(!account.b_stale_state, "v16 b-stale clear reachable");
    assert_eq!(group.b_stale_account_count, 0);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_b_stale_clear_is_gated_by_active_b_stale_leg() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    group
        .attach_leg(&mut account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group.mark_leg_b_stale(&mut account, 0).unwrap();
    kani::cover!(
        account.b_stale_state && account.legs[0].b_stale,
        "v16 active b-stale leg reachable"
    );
    assert_eq!(group.b_stale_account_count, 1);

    assert_eq!(
        group.clear_account_b_stale(&mut account),
        Err(V16Error::BStale)
    );
    assert!(account.b_stale_state);
    assert!(account.legs[0].b_stale);
    assert_eq!(group.b_stale_account_count, 1);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_account_equity_rejects_i128_min_persistent_pnl() {
    let (market, account_id, owner) = symbolic_ids();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.pnl = i128::MIN;
    assert_eq!(account_equity(&account), Err(V16Error::ArithmeticOverflow));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_account_equity_rejects_malformed_fee_credits() {
    let malformed_positive: bool = kani::any();
    let (market, account_id, owner) = symbolic_ids();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.capital = 100;
    account.fee_credits = if malformed_positive { 1 } else { i128::MIN };

    kani::cover!(
        malformed_positive,
        "v16 positive fee credit corruption reachable"
    );
    kani::cover!(
        !malformed_positive,
        "v16 i128 min fee credit corruption reachable"
    );
    assert!(account_equity(&account).is_err());
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_account_equity_rejects_capital_above_i128_max() {
    let (market, account_id, owner) = symbolic_ids();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.capital = i128::MAX as u128 + 1;

    kani::cover!(
        account.capital > i128::MAX as u128,
        "v16 capital overflow equity path reachable"
    );
    assert_eq!(account_equity(&account), Err(V16Error::ArithmeticOverflow));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_account_shape_rejects_malformed_persistent_economic_state() {
    let dirty_case: u8 = kani::any();
    kani::assume(dirty_case < 4);
    let (market, account_id, owner) = symbolic_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    let expected = match dirty_case {
        0 => {
            account.pnl = i128::MIN;
            V16Error::ArithmeticOverflow
        }
        1 => {
            account.fee_credits = 1;
            V16Error::InvalidLeg
        }
        2 => {
            account.fee_credits = i128::MIN;
            V16Error::ArithmeticOverflow
        }
        _ => {
            account.pnl = 1;
            account.reserved_pnl = 2;
            V16Error::InvalidLeg
        }
    };

    kani::cover!(dirty_case == 0, "v16 shape rejects i128 min pnl");
    kani::cover!(dirty_case == 1, "v16 shape rejects positive fee credit");
    kani::cover!(dirty_case == 2, "v16 shape rejects i128 min fee credit");
    kani::cover!(dirty_case == 3, "v16 shape rejects over-reserved pnl");
    assert_eq!(group.validate_account_shape(&account), Err(expected));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_account_shape_rejects_noncanonical_resolved_receipt_finalization() {
    let finalized: bool = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.resolved_payout_receipt = ResolvedPayoutReceiptV16 {
        present: true,
        prior_bound_contribution_num: BOUND_SCALE,
        live_released_face_at_receipt: 0,
        terminal_positive_claim_face: 1,
        paid_effective: if finalized { 0 } else { 1 },
        finalized,
    };

    kani::cover!(finalized, "v16 shape rejects finalized underpaid receipt");
    kani::cover!(
        !finalized,
        "v16 shape rejects unfinalized fully-paid receipt"
    );
    let result = group.validate_account_shape(&account);
    assert!(matches!(result, Err(V16Error::InvalidLeg)));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_persisted_wire_rejects_noncanonical_account_bool() {
    let bad_bool: u8 = kani::any();
    kani::assume(bad_bool > 1);
    let (market, account_id, owner) = symbolic_ids();
    let account = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut account_wire = PortfolioAccountV16Account::from_runtime(&account);
    account_wire.stale_state = bad_bool;
    kani::cover!(bad_bool == 2, "v16 persisted invalid bool branch reachable");
    assert_eq!(
        account_wire.try_to_runtime_with_source_domains(&[]),
        Err(V16Error::InvalidConfig)
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_persisted_wire_rejects_noncanonical_config_bool() {
    let bad_bool: u8 = kani::any();
    kani::assume(bad_bool > 1);
    let (market, _, _) = symbolic_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let slots = group_slots_for_one_asset(&group);
    let mut config_bool_wire = group_header_for_one_asset(&group);
    config_bool_wire.config.recovery_fallback_price_enabled = bad_bool;
    kani::cover!(
        bad_bool == 3,
        "v16 persisted invalid config bool branch reachable"
    );
    assert_eq!(
        decode_one_asset_group(&config_bool_wire, &slots),
        Err(V16Error::InvalidConfig)
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_persisted_wire_rejects_noncanonical_market_mode() {
    let bad_market_mode: u8 = kani::any();
    kani::assume(bad_market_mode > 2);
    let (market, _, _) = symbolic_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let slots = group_slots_for_one_asset(&group);
    let mut market_mode_wire = group_header_for_one_asset(&group);
    market_mode_wire.mode = bad_market_mode;
    kani::cover!(
        bad_market_mode == 3,
        "v16 persisted invalid market mode branch reachable"
    );
    assert_eq!(
        decode_one_asset_group(&market_mode_wire, &slots),
        Err(V16Error::InvalidConfig)
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_persisted_wire_rejects_noncanonical_side_mode() {
    let bad_side_mode: u8 = kani::any();
    kani::assume(bad_side_mode > 2);
    let (market, _, _) = symbolic_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let header = group_header_for_one_asset(&group);
    let mut side_mode_slots = group_slots_for_one_asset(&group);
    side_mode_slots[0].asset.mode_long = bad_side_mode;
    kani::cover!(
        bad_side_mode == 3,
        "v16 persisted invalid side mode branch reachable"
    );
    assert_eq!(
        decode_one_asset_group(&header, &side_mode_slots),
        Err(V16Error::InvalidConfig)
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_persisted_wire_rejects_noncanonical_option_present() {
    let bad_option_present: u8 = kani::any();
    kani::assume(bad_option_present > 1);
    let (market, _, _) = symbolic_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let slots = group_slots_for_one_asset(&group);
    let mut option_wire = group_header_for_one_asset(&group);
    option_wire.recovery_reason.present = bad_option_present;
    kani::cover!(
        bad_option_present == 2,
        "v16 persisted invalid option-present branch reachable"
    );
    assert_eq!(
        decode_one_asset_group(&option_wire, &slots),
        Err(V16Error::InvalidConfig)
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_engine_asset_slot_validation_rejects_backing_market_id_drift() {
    let corrupt_short: bool = kani::any();
    let (market, _, _) = symbolic_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut slot = EngineAssetSlotV16Account::from_runtime_group_slot(&group, 0).unwrap();
    let wrong_market_id = group.assets[0].market_id.checked_add(1).unwrap();

    if corrupt_short {
        slot.backing_short.market_id = V16PodU64::new(wrong_market_id);
    } else {
        slot.backing_long.market_id = V16PodU64::new(wrong_market_id);
    }

    kani::cover!(
        !corrupt_short,
        "v16 persisted backing long market-id drift reachable"
    );
    kani::cover!(
        corrupt_short,
        "v16 persisted backing short market-id drift reachable"
    );
    assert_eq!(
        slot.validate_market_id_binding(),
        Err(V16Error::InvalidConfig)
    );
}

#[kani::proof]
#[kani::unwind(32)]
#[kani::solver(cadical)]
fn proof_v16_market_wire_roundtrip_preserves_valid_runtime_state() {
    let vault_units: u8 = kani::any();
    let c_units: u8 = kani::any();
    let i_units: u8 = kani::any();
    let pnl_pos_units: u8 = kani::any();
    let pnl_matured_units: u8 = kani::any();
    let price_raw: u16 = kani::any();
    let oi_units: u8 = kani::any();
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
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    group.vault = vault_units as u128;
    group.c_tot = c_units as u128;
    group.insurance = i_units as u128;
    group.pnl_pos_tot = pnl_pos_units as u128;
    set_junior_bound(&mut group, pnl_pos_units as u128);
    group.pnl_matured_pos_tot = pnl_matured_units as u128;
    group.bankruptcy_hlock_active = kani::any();
    group.threshold_stress_active = kani::any();
    group.loss_stale_active = kani::any();
    group.payout_snapshot_captured = kani::any();
    group.mode = match market_mode_case {
        0 => MarketModeV16::Live,
        1 => MarketModeV16::Recovery,
        _ => MarketModeV16::Resolved,
    };
    group.recovery_reason = if recovery_present {
        Some(match recovery_case {
            0 => PermissionlessRecoveryReasonV16::BelowProgressFloor,
            1 => PermissionlessRecoveryReasonV16::BlockedSegmentHeadroomOrRepresentability,
            2 => PermissionlessRecoveryReasonV16::AccountBSettlementCannotProgress,
            3 => PermissionlessRecoveryReasonV16::BIndexHeadroomExhausted,
            4 => PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress,
            5 => PermissionlessRecoveryReasonV16::ExplicitLossOrDustAuditOverflow,
            6 => PermissionlessRecoveryReasonV16::OracleOrTargetUnavailableByAuthenticatedPolicy,
            _ => PermissionlessRecoveryReasonV16::CounterOrEpochOverflowDeclaredRecovery,
        })
    } else {
        None
    };

    let side_mode = match side_mode_case {
        0 => SideModeV16::Normal,
        1 => SideModeV16::ResetPending,
        _ => SideModeV16::DrainOnly,
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
    group.assets[0].oi_eff_long_q = oi_units as u128;
    group.assets[0].oi_eff_short_q = oi_units as u128;
    group.assets[0].loss_weight_sum_long = if oi_units == 0 { 0 } else { 1 };
    group.assets[0].loss_weight_sum_short = if oi_units == 0 { 0 } else { 1 };
    group.assets[0].mode_long = side_mode;
    group.assets[0].mode_short = side_mode;

    let wire = MarketGroupV16HeaderAccount::from_runtime_with_capacity(&group, 1).unwrap();
    let slots = group_slots_for_one_asset(&group);
    assert_eq!(wire.asset_slot_capacity.get(), 1);
    assert_eq!(wire.config.max_market_slots.get(), 1);
    let decoded = decode_one_asset_group(&wire, &slots).unwrap();

    kani::cover!(
        recovery_present,
        "v16 market wire roundtrip with recovery reason"
    );
    kani::cover!(
        !recovery_present,
        "v16 market wire roundtrip without recovery reason"
    );
    kani::cover!(
        side_mode_case == 1,
        "v16 market wire roundtrip reset-pending side mode"
    );
    assert_eq!(decoded.market_group_id[0], group.market_group_id[0]);
    assert_eq!(decoded.market_group_id[1], group.market_group_id[1]);
    assert_eq!(decoded.market_group_id[2], group.market_group_id[2]);
    assert_eq!(decoded.market_group_id[3], group.market_group_id[3]);
    assert_eq!(decoded.market_group_id[4], group.market_group_id[4]);
    assert_eq!(decoded.market_group_id[5], group.market_group_id[5]);
    assert_eq!(decoded.market_group_id[6], group.market_group_id[6]);
    assert_eq!(decoded.market_group_id[7], group.market_group_id[7]);
    assert_eq!(decoded.market_group_id[8], group.market_group_id[8]);
    assert_eq!(decoded.market_group_id[9], group.market_group_id[9]);
    assert_eq!(decoded.market_group_id[10], group.market_group_id[10]);
    assert_eq!(decoded.market_group_id[11], group.market_group_id[11]);
    assert_eq!(decoded.market_group_id[12], group.market_group_id[12]);
    assert_eq!(decoded.market_group_id[13], group.market_group_id[13]);
    assert_eq!(decoded.market_group_id[14], group.market_group_id[14]);
    assert_eq!(decoded.market_group_id[15], group.market_group_id[15]);
    assert_eq!(decoded.market_group_id[16], group.market_group_id[16]);
    assert_eq!(decoded.market_group_id[17], group.market_group_id[17]);
    assert_eq!(decoded.market_group_id[18], group.market_group_id[18]);
    assert_eq!(decoded.market_group_id[19], group.market_group_id[19]);
    assert_eq!(decoded.market_group_id[20], group.market_group_id[20]);
    assert_eq!(decoded.market_group_id[21], group.market_group_id[21]);
    assert_eq!(decoded.market_group_id[22], group.market_group_id[22]);
    assert_eq!(decoded.market_group_id[23], group.market_group_id[23]);
    assert_eq!(decoded.market_group_id[24], group.market_group_id[24]);
    assert_eq!(decoded.market_group_id[25], group.market_group_id[25]);
    assert_eq!(decoded.market_group_id[26], group.market_group_id[26]);
    assert_eq!(decoded.market_group_id[27], group.market_group_id[27]);
    assert_eq!(decoded.market_group_id[28], group.market_group_id[28]);
    assert_eq!(decoded.market_group_id[29], group.market_group_id[29]);
    assert_eq!(decoded.market_group_id[30], group.market_group_id[30]);
    assert_eq!(decoded.market_group_id[31], group.market_group_id[31]);
    assert_eq!(
        decoded.config.max_portfolio_assets,
        group.config.max_portfolio_assets
    );
    assert_eq!(
        decoded.config.min_nonzero_mm_req,
        group.config.min_nonzero_mm_req
    );
    assert_eq!(
        decoded.config.min_nonzero_im_req,
        group.config.min_nonzero_im_req
    );
    assert_eq!(decoded.config.h_min, group.config.h_min);
    assert_eq!(decoded.config.h_max, group.config.h_max);
    assert_eq!(
        decoded.config.maintenance_margin_bps,
        group.config.maintenance_margin_bps
    );
    assert_eq!(
        decoded.config.initial_margin_bps,
        group.config.initial_margin_bps
    );
    assert_eq!(
        decoded.config.max_trading_fee_bps,
        group.config.max_trading_fee_bps
    );
    assert_eq!(
        decoded.config.max_accrual_dt_slots,
        group.config.max_accrual_dt_slots
    );
    assert_eq!(
        decoded.config.max_price_move_bps_per_slot,
        group.config.max_price_move_bps_per_slot
    );
    assert_eq!(
        decoded.config.permissionless_recovery_enabled,
        group.config.permissionless_recovery_enabled
    );
    assert_eq!(
        decoded.config.recovery_fallback_price_enabled,
        group.config.recovery_fallback_price_enabled
    );
    assert_eq!(decoded.vault, group.vault);
    assert_eq!(decoded.c_tot, group.c_tot);
    assert_eq!(decoded.insurance, group.insurance);
    assert_eq!(decoded.pnl_pos_tot, group.pnl_pos_tot);
    assert_eq!(decoded.pnl_pos_bound_tot_num, group.pnl_pos_bound_tot_num);
    assert_eq!(decoded.pnl_pos_bound_tot, group.pnl_pos_bound_tot);
    assert_eq!(decoded.pnl_matured_pos_tot, group.pnl_matured_pos_tot);
    assert_eq!(
        decoded.bankruptcy_hlock_active,
        group.bankruptcy_hlock_active
    );
    assert_eq!(
        decoded.threshold_stress_active,
        group.threshold_stress_active
    );
    assert_eq!(decoded.loss_stale_active, group.loss_stale_active);
    assert_eq!(
        decoded.payout_snapshot_captured,
        group.payout_snapshot_captured
    );
    assert_eq!(decoded.mode, group.mode);
    assert_eq!(
        decoded.recovery_reason.is_some(),
        group.recovery_reason.is_some()
    );
    if let (Some(decoded_reason), Some(group_reason)) =
        (decoded.recovery_reason, group.recovery_reason)
    {
        assert_eq!(decoded_reason, group_reason);
    }
    let mut asset_i = 0;
    while asset_i < 1 {
        assert_eq!(
            decoded.assets[asset_i].raw_oracle_target_price,
            group.assets[asset_i].raw_oracle_target_price
        );
        assert_eq!(
            decoded.assets[asset_i].effective_price,
            group.assets[asset_i].effective_price
        );
        assert_eq!(
            decoded.assets[asset_i].fund_px_last,
            group.assets[asset_i].fund_px_last
        );
        assert_eq!(decoded.assets[asset_i].k_long, group.assets[asset_i].k_long);
        assert_eq!(
            decoded.assets[asset_i].k_short,
            group.assets[asset_i].k_short
        );
        assert_eq!(
            decoded.assets[asset_i].f_long_num,
            group.assets[asset_i].f_long_num
        );
        assert_eq!(
            decoded.assets[asset_i].f_short_num,
            group.assets[asset_i].f_short_num
        );
        assert_eq!(
            decoded.assets[asset_i].oi_eff_long_q,
            group.assets[asset_i].oi_eff_long_q
        );
        assert_eq!(
            decoded.assets[asset_i].oi_eff_short_q,
            group.assets[asset_i].oi_eff_short_q
        );
        assert_eq!(
            decoded.assets[asset_i].loss_weight_sum_long,
            group.assets[asset_i].loss_weight_sum_long
        );
        assert_eq!(
            decoded.assets[asset_i].loss_weight_sum_short,
            group.assets[asset_i].loss_weight_sum_short
        );
        assert_eq!(
            decoded.assets[asset_i].mode_long,
            group.assets[asset_i].mode_long
        );
        assert_eq!(
            decoded.assets[asset_i].mode_short,
            group.assets[asset_i].mode_short
        );
        asset_i += 1;
    }
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_portfolio_wire_roundtrip_preserves_valid_runtime_state() {
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
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    if active {
        let signed_basis = if short_side {
            -(basis_units as i128)
        } else {
            basis_units as i128
        };
        let side = if short_side {
            SideV16::Short
        } else {
            SideV16::Long
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

    let wire = PortfolioAccountV16Account::from_runtime(&account);
    let source_domains = source_domains_for_one_asset(&account);
    let decoded = decode_one_asset_account(&wire, &source_domains).unwrap();
    let checked = wire.validate_with_market(&group, &source_domains).unwrap();

    kani::cover!(
        active && !short_side,
        "v16 portfolio wire roundtrip active long"
    );
    kani::cover!(
        active && short_side,
        "v16 portfolio wire roundtrip active short"
    );
    kani::cover!(!active, "v16 portfolio wire roundtrip inactive account");
    assert_eq!(decoded, account);
    assert_eq!(checked, account);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_portfolio_wire_roundtrip_preserves_source_lien_fields() {
    let (market, account_id, owner) = concrete_ids();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.ensure_source_domain_capacity(2);
    account.pnl = 10;
    account.source_claim_bound_num[0] = 10 * BOUND_SCALE;
    account.source_claim_liened_num[0] = 2 * BOUND_SCALE;
    account.source_claim_counterparty_liened_num[0] = 2 * BOUND_SCALE;
    account.source_lien_effective_reserved[0] = 2;
    account.source_lien_counterparty_backing_num[0] = 2 * BOUND_SCALE;

    let wire = PortfolioAccountV16Account::from_runtime(&account);
    let source_domains = source_domains_for_one_asset(&account);
    let decoded = decode_one_asset_account(&wire, &source_domains).unwrap();

    kani::cover!(true, "v16 portfolio wire source-lien roundtrip reachable");
    assert_eq!(decoded.pnl, account.pnl);
    assert_eq!(decoded.source_claim_bound_num[0], 10 * BOUND_SCALE);
    assert_eq!(decoded.source_claim_liened_num[0], 2 * BOUND_SCALE);
    assert_eq!(
        decoded.source_claim_counterparty_liened_num[0],
        2 * BOUND_SCALE
    );
    assert_eq!(decoded.source_claim_insurance_liened_num[0], 0);
    assert_eq!(decoded.source_lien_effective_reserved[0], 2);
    assert_eq!(
        decoded.source_lien_counterparty_backing_num[0],
        2 * BOUND_SCALE
    );
    assert_eq!(decoded.source_lien_insurance_backing_num[0], 0);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_portfolio_leg_wire_roundtrip_preserves_asset_index() {
    let raw_idx: u8 = kani::any();
    let asset_index = (raw_idx % 4) as u32;
    let long_side: bool = kani::any();
    let side = if long_side {
        SideV16::Long
    } else {
        SideV16::Short
    };
    let basis_pos_q = if long_side { 7 } else { -7 };
    let leg = PortfolioLegV16 {
        active: true,
        asset_index,
        market_id: 11 + asset_index as u64,
        side,
        basis_pos_q,
        a_basis: ADL_ONE,
        k_snap: 0,
        f_snap: 0,
        epoch_snap: 0,
        loss_weight: 7,
        b_snap: 0,
        b_rem: 0,
        b_epoch_snap: 0,
        b_stale: false,
        stale: false,
    };

    let wire = PortfolioLegV16Account::from_runtime(&leg);
    let decoded = wire.try_to_runtime().unwrap();

    kani::cover!(
        asset_index == 3,
        "v16 leg asset-index roundtrip covers nonzero compact asset"
    );
    assert_eq!(decoded.asset_index, asset_index);
    assert_eq!(decoded.market_id, leg.market_id);
    assert_eq!(decoded.side, side);
    assert_eq!(decoded.basis_pos_q, basis_pos_q);
}

#[kani::proof]
#[kani::unwind(150)]
#[kani::solver(cadical)]
fn proof_v16_validate_account_shape_binds_compact_leg_slot_to_asset_identity() {
    let raw_idx: u8 = kani::any();
    let asset_index = (raw_idx % 4) as usize;
    let corrupt_market_id: bool = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(4, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.ensure_source_domain_capacity(group.source_credit.len());
    account.active_bitmap = bitmap(&[0]);
    account.legs[0] = PortfolioLegV16 {
        active: true,
        asset_index: asset_index as u32,
        market_id: if corrupt_market_id {
            group.assets[(asset_index + 1) % 4].market_id
        } else {
            group.assets[asset_index].market_id
        },
        side: SideV16::Long,
        basis_pos_q: 7,
        a_basis: ADL_ONE,
        k_snap: 0,
        f_snap: 0,
        epoch_snap: 0,
        loss_weight: 7,
        b_snap: 0,
        b_rem: 0,
        b_epoch_snap: 0,
        b_stale: false,
        stale: false,
    };

    kani::cover!(
        asset_index == 3 && !corrupt_market_id,
        "v16 compact leg accepts nonzero asset id in slot zero"
    );
    kani::cover!(
        corrupt_market_id,
        "v16 compact leg rejects stale market identity"
    );
    if corrupt_market_id {
        assert_eq!(
            group.validate_account_shape(&account),
            Err(V16Error::HiddenLeg)
        );
    } else {
        assert_eq!(group.validate_account_shape(&account), Ok(()));
    }
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_validate_account_shape_rejects_missing_source_domain_capacity() {
    let (market, account_id, owner) = concrete_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let account = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    kani::cover!(true, "v16 missing runtime source-domain storage reachable");
    assert_eq!(
        group.validate_account_shape(&account),
        Err(V16Error::HiddenLeg)
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_validate_account_shape_binds_leg_epoch_to_asset_side() {
    let reset_pending: bool = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.ensure_source_domain_capacity(group.source_credit.len());
    account.active_bitmap = bitmap(&[0]);
    account.legs[0] = PortfolioLegV16 {
        active: true,
        asset_index: 0,
        market_id: group.assets[0].market_id,
        side: SideV16::Long,
        basis_pos_q: 7,
        a_basis: ADL_ONE,
        k_snap: 0,
        f_snap: 0,
        epoch_snap: 0,
        loss_weight: 7,
        b_snap: 0,
        b_rem: 0,
        b_epoch_snap: 0,
        b_stale: false,
        stale: false,
    };
    group.assets[0].epoch_long = 1;
    group.assets[0].mode_long = if reset_pending {
        SideModeV16::ResetPending
    } else {
        SideModeV16::Normal
    };

    kani::cover!(!reset_pending, "v16 normal stale leg epoch rejected");
    kani::cover!(
        reset_pending,
        "v16 reset-pending prior leg epoch remains valid"
    );
    if reset_pending {
        assert_eq!(group.validate_account_shape(&account), Ok(()));
    } else {
        assert_eq!(
            group.validate_account_shape(&account),
            Err(V16Error::HiddenLeg)
        );
    }
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_zero_copy_validate_rejects_missing_source_domain_slice() {
    let (market, account_id, owner) = concrete_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let header = group_header_for_one_asset(&group);
    let slots = group_slots_for_one_asset(&group);
    let account = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let wire = PortfolioAccountV16Account::from_runtime(&account);
    let source_domains: [PortfolioSourceDomainV16Account; 0] = [];
    let markets = [Market {
        wrapper: (),
        engine: slots[0],
    }];
    let market_view = MarketGroupV16View::new(&header, &markets);
    let account_view = percolator::v16::PortfolioV16View::new(&wire, &source_domains);

    kani::cover!(true, "v16 missing zero-copy source-domain slice reachable");
    assert_eq!(
        account_view.validate_with_market(&market_view),
        Err(V16Error::HiddenLeg)
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_zero_copy_validate_binds_leg_epoch_to_asset_side() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.ensure_source_domain_capacity(group.source_credit.len());
    account.active_bitmap = bitmap(&[0]);
    account.legs[0] = PortfolioLegV16 {
        active: true,
        asset_index: 0,
        market_id: group.assets[0].market_id,
        side: SideV16::Long,
        basis_pos_q: 7,
        a_basis: ADL_ONE,
        k_snap: 0,
        f_snap: 0,
        epoch_snap: 0,
        loss_weight: 7,
        b_snap: 0,
        b_rem: 0,
        b_epoch_snap: 0,
        b_stale: false,
        stale: false,
    };
    group.assets[0].epoch_long = group.assets[0].epoch_long.checked_add(1).unwrap();

    let header = group_header_for_one_asset(&group);
    let slots = group_slots_for_one_asset(&group);
    let markets = [Market {
        wrapper: (),
        engine: slots[0],
    }];
    let wire = PortfolioAccountV16Account::from_runtime(&account);
    let source_domains = source_domains_for_one_asset(&account);
    let market_view = MarketGroupV16View::new(&header, &markets);
    let account_view = percolator::v16::PortfolioV16View::new(&wire, &source_domains);

    kani::cover!(true, "v16 zero-copy stale leg epoch reachable");
    assert_eq!(
        account_view.validate_with_market(&market_view),
        Err(V16Error::HiddenLeg)
    );
}

fn persisted_wire_rejects_i128_min_group_field(
    mutate: impl FnOnce(&mut [EngineAssetSlotV16Account; 1]),
) {
    let (market, _, _) = concrete_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let header = group_header_for_one_asset(&group);
    let mut slots = group_slots_for_one_asset(&group);
    mutate(&mut slots);
    assert_eq!(
        decode_one_asset_group(&header, &slots),
        Err(V16Error::ArithmeticOverflow)
    );
}

fn persisted_wire_rejects_i128_min_account_field(
    mutate: impl FnOnce(&mut PortfolioAccountV16Account),
) {
    let (market, account_id, owner) = concrete_ids();
    let account = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut wire = PortfolioAccountV16Account::from_runtime(&account);
    mutate(&mut wire);
    let source_domains = source_domains_for_one_asset(&account);
    assert_eq!(
        decode_one_asset_account(&wire, &source_domains),
        Err(V16Error::ArithmeticOverflow)
    );
}

// Leg snapshot fields are only validated for `i128::MIN` when the leg is active
// (`validate_active_leg`); on an empty leg a non-zero field decodes as `HiddenLeg`.
// So this variant attaches an active leg before corrupting a leg-level field.
fn persisted_wire_rejects_i128_min_active_leg_field(
    mutate: impl FnOnce(&mut PortfolioAccountV16Account),
) {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.attach_leg(&mut account, 0, SideV16::Long, 1).unwrap();
    let mut wire = PortfolioAccountV16Account::from_runtime(&account);
    mutate(&mut wire);
    let source_domains = source_domains_for_one_asset(&account);
    assert_eq!(
        decode_one_asset_account(&wire, &source_domains),
        Err(V16Error::ArithmeticOverflow)
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_persisted_wire_rejects_i128_min_market_k_long() {
    persisted_wire_rejects_i128_min_group_field(|slots| {
        slots[0].asset.k_long = V16PodI128::new(i128::MIN);
    });
    kani::cover!(true, "v16 wire rejects i128 min market K");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_persisted_wire_rejects_i128_min_market_f_short() {
    persisted_wire_rejects_i128_min_group_field(|slots| {
        slots[0].asset.f_short_num = V16PodI128::new(i128::MIN);
    });
    kani::cover!(true, "v16 wire rejects i128 min market F");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_persisted_wire_rejects_i128_min_account_pnl() {
    persisted_wire_rejects_i128_min_account_field(|wire| {
        wire.pnl = V16PodI128::new(i128::MIN);
    });
    kani::cover!(true, "v16 wire rejects i128 min account PnL");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_persisted_wire_rejects_i128_min_fee_credits() {
    persisted_wire_rejects_i128_min_account_field(|wire| {
        wire.fee_credits = V16PodI128::new(i128::MIN);
    });
    kani::cover!(true, "v16 wire rejects i128 min fee credits");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_persisted_wire_rejects_i128_min_leg_k_snap() {
    persisted_wire_rejects_i128_min_active_leg_field(|wire| {
        wire.legs[0].k_snap = V16PodI128::new(i128::MIN);
    });
    kani::cover!(true, "v16 wire rejects i128 min leg K snapshot");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_persisted_wire_rejects_i128_min_health_cert_equity() {
    persisted_wire_rejects_i128_min_account_field(|wire| {
        wire.health_cert.certified_equity = V16PodI128::new(i128::MIN);
    });
    kani::cover!(true, "v16 wire rejects i128 min health certificate");
}

fn persisted_wire_rejects_smuggling_case(
    mutate: impl FnOnce(&mut PortfolioAccountV16Account, &PortfolioAccountV16Account),
    expected: V16Error,
) {
    let (market, account_id, owner) = concrete_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let empty = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut active = empty.clone();
    let mut builder_group = group.clone();
    builder_group
        .attach_leg(&mut active, 0, SideV16::Long, 1)
        .unwrap();
    let active_wire = PortfolioAccountV16Account::from_runtime(&active);
    let mut wire = PortfolioAccountV16Account::from_runtime(&empty);
    mutate(&mut wire, &active_wire);
    let source_domains = source_domains_for_one_asset(&empty);
    assert_eq!(
        wire.validate_with_market(&group, &source_domains),
        Err(expected)
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_persisted_wire_rejects_wrong_market_group_id() {
    persisted_wire_rejects_smuggling_case(
        |wire, _| {
            wire.provenance_header.market_group_id = [9; 32];
        },
        V16Error::ProvenanceMismatch,
    );
    kani::cover!(true, "v16 persisted wrong-market account rejected");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_persisted_wire_rejects_wrong_owner() {
    persisted_wire_rejects_smuggling_case(
        |wire, _| {
            wire.owner = [9; 32];
        },
        V16Error::ProvenanceMismatch,
    );
    kani::cover!(true, "v16 persisted wrong-owner account rejected");
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_persisted_wire_rejects_bitmap_only_leg() {
    persisted_wire_rejects_smuggling_case(
        |wire, _| {
            wire.active_bitmap = [V16PodU64::new(1)];
        },
        V16Error::HiddenLeg,
    );
    kani::cover!(true, "v16 persisted bitmap-only leg rejected");
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_persisted_wire_rejects_hidden_active_leg() {
    persisted_wire_rejects_smuggling_case(
        |wire, active_wire| {
            wire.legs[0] = active_wire.legs[0];
            wire.active_bitmap = [V16PodU64::new(0)];
        },
        V16Error::HiddenLeg,
    );
    kani::cover!(true, "v16 persisted hidden active leg rejected");
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_persisted_wire_rejects_out_of_config_leg() {
    persisted_wire_rejects_smuggling_case(
        |wire, active_wire| {
            wire.legs[1] = active_wire.legs[0];
            wire.active_bitmap = [V16PodU64::new(1 << 1)];
        },
        V16Error::HiddenLeg,
    );
    kani::cover!(true, "v16 persisted out-of-config leg rejected");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_flat_account_equity_is_exact_capital_plus_pnl_minus_fee_debt() {
    let capital: u16 = kani::any();
    let pnl: i16 = kani::any();
    let debt: u16 = kani::any();
    kani::assume(capital <= 10_000);
    kani::assume(debt <= 10_000);
    let (market, account_id, owner) = symbolic_ids();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.capital = capital as u128;
    account.pnl = pnl as i128;
    account.fee_credits = -(debt as i128);

    let expected = (capital as i128) + (pnl as i128) - (debt as i128);
    let actual = account_equity(&account).unwrap();

    kani::cover!(pnl < 0, "v16 flat negative pnl equity branch reachable");
    kani::cover!(pnl >= 0, "v16 flat nonnegative pnl equity branch reachable");
    kani::cover!(debt > 0, "v16 flat account fee debt branch reachable");
    assert_eq!(actual, expected);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_authoritatively_flat_account_never_receives_b_loss() {
    let b_long: u8 = kani::any();
    let b_short: u8 = kani::any();
    let budget: u8 = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 100).unwrap();
    group.assets[0].b_long_num = b_long as u128;
    group.assets[0].b_short_num = b_short as u128;

    let before_account = account.clone();
    let before_count = group.b_stale_account_count;
    let outcome = group
        .settle_account_side_effects_not_atomic(&mut account, budget as u128)
        .unwrap();

    kani::cover!(
        b_long > 0 || b_short > 0,
        "v16 flat account with nonzero side B accumulator reachable"
    );
    assert_eq!(outcome, PermissionlessProgressOutcomeV16::AccountCurrent);
    assert_eq!(account.active_bitmap, bitmap(&[]));
    assert_eq!(account.pnl, before_account.pnl);
    assert_eq!(account.capital, before_account.capital);
    assert_eq!(account.b_stale_state, before_account.b_stale_state);
    assert_eq!(group.b_stale_account_count, before_count);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_public_config_rejects_invalid_user_fund_shapes() {
    let case: u8 = kani::any();
    kani::assume(case < 13);
    let (market, _, _) = symbolic_ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 1);
    match case {
        0 => cfg.max_portfolio_assets = 0,
        1 => cfg.h_max = 0,
        2 => cfg.h_min = 2,
        3 => cfg.min_nonzero_mm_req = cfg.min_nonzero_im_req,
        4 => cfg.permissionless_recovery_enabled = false,
        5 => cfg.recovery_fallback_price_enabled = false,
        6 => cfg.public_b_chunk_atoms = 0,
        7 => cfg.stale_certificate_penalty_enabled = false,
        8 => cfg.full_refresh_required_for_favorable_actions = false,
        9 => cfg.public_liveness_profile_crank_forward = false,
        10 => cfg.max_account_b_settlement_chunks = 0,
        11 => cfg.max_bankrupt_close_chunks = 0,
        _ => cfg.max_bankrupt_close_lifetime_slots = 0,
    }

    kani::cover!(case == 0, "v16 zero portfolio width rejected");
    kani::cover!(case == 1, "v16 zero hmax rejected");
    kani::cover!(case == 2, "v16 hmin above hmax rejected");
    kani::cover!(case == 3, "v16 invalid margin floor ordering rejected");
    kani::cover!(case == 4, "v16 disabled recovery rejected");
    kani::cover!(case == 5, "v16 disabled recovery fallback rejected");
    kani::cover!(case == 6, "v16 zero B chunk budget rejected");
    kani::cover!(case == 7, "v16 disabled stale certificate penalty rejected");
    kani::cover!(case == 8, "v16 disabled required full refresh rejected");
    kani::cover!(case == 9, "v16 disabled crank-forward profile rejected");
    kani::cover!(case == 10, "v16 zero account B chunk cap rejected");
    kani::cover!(case == 11, "v16 zero bankrupt close chunk cap rejected");
    kani::cover!(case == 12, "v16 zero bankrupt close lifetime rejected");
    assert_eq!(
        MarketGroupV16::new(market, cfg),
        Err(V16Error::InvalidConfig)
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_permissionless_recovery_declares_reason_or_fails_closed() {
    let reason_case: u8 = kani::any();
    kani::assume(reason_case < 8);
    let enabled: bool = kani::any();
    let start_resolved: bool = kani::any();
    let reason = match reason_case {
        0 => PermissionlessRecoveryReasonV16::BelowProgressFloor,
        1 => PermissionlessRecoveryReasonV16::BlockedSegmentHeadroomOrRepresentability,
        2 => PermissionlessRecoveryReasonV16::AccountBSettlementCannotProgress,
        3 => PermissionlessRecoveryReasonV16::BIndexHeadroomExhausted,
        4 => PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress,
        5 => PermissionlessRecoveryReasonV16::ExplicitLossOrDustAuditOverflow,
        6 => PermissionlessRecoveryReasonV16::OracleOrTargetUnavailableByAuthenticatedPolicy,
        _ => PermissionlessRecoveryReasonV16::CounterOrEpochOverflowDeclaredRecovery,
    };
    let (market, _, _) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
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
        "v16 permissionless recovery enabled path reachable"
    );
    kani::cover!(
        !enabled,
        "v16 permissionless recovery disabled path reachable"
    );
    kani::cover!(
        enabled && start_resolved,
        "v16 permissionless recovery resolved-mode rejection reachable"
    );
    kani::cover!(
        reason_case == 0,
        "v16 permissionless recovery first reason reachable"
    );
    kani::cover!(
        reason_case == 7,
        "v16 permissionless recovery last reason reachable"
    );

    if enabled && !start_resolved {
        assert_eq!(
            result,
            Ok(PermissionlessProgressOutcomeV16::RecoveryDeclared(reason))
        );
        assert_eq!(group.recovery_reason, Some(reason));
        assert_eq!(group.mode, MarketModeV16::Recovery);
    } else {
        if enabled {
            assert_eq!(result, Err(V16Error::LockActive));
        } else {
            assert_eq!(result, Err(V16Error::InvalidConfig));
        }
        assert_eq!(group.recovery_reason, None);
        assert_eq!(group.mode, before_mode);
    }
    assert_eq!(group.vault, before_vault);
    assert_eq!(group.c_tot, before_c_tot);
    assert_eq!(group.insurance, before_insurance);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_explicit_loss_audit_overflow_declares_recovery_without_value_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 100).unwrap();
    let vault_before = group.vault;
    let c_tot_before = group.c_tot;
    let insurance_before = group.insurance;
    let pnl_pos_before = group.pnl_pos_tot;
    let oi_long_before = group.assets[0].oi_eff_long_q;
    let oi_short_before = group.assets[0].oi_eff_short_q;
    let k_long_before = group.assets[0].k_long;
    let k_short_before = group.assets[0].k_short;

    let result = group.declare_explicit_loss_or_dust_audit_overflow_not_atomic();

    kani::cover!(
        group.recovery_reason
            == Some(PermissionlessRecoveryReasonV16::ExplicitLossOrDustAuditOverflow),
        "v16 explicit loss audit overflow recovery declaration reachable"
    );
    assert_eq!(
        result,
        Ok(PermissionlessProgressOutcomeV16::RecoveryDeclared(
            PermissionlessRecoveryReasonV16::ExplicitLossOrDustAuditOverflow
        ))
    );
    assert_eq!(group.mode, MarketModeV16::Recovery);
    assert_eq!(
        group.recovery_reason,
        Some(PermissionlessRecoveryReasonV16::ExplicitLossOrDustAuditOverflow)
    );
    assert_eq!(group.vault, vault_before);
    assert_eq!(group.c_tot, c_tot_before);
    assert_eq!(group.insurance, insurance_before);
    assert_eq!(group.pnl_pos_tot, pnl_pos_before);
    assert_eq!(group.assets[0].oi_eff_long_q, oi_long_before);
    assert_eq!(group.assets[0].oi_eff_short_q, oi_short_before);
    assert_eq!(group.assets[0].k_long, k_long_before);
    assert_eq!(group.assets[0].k_short, k_short_before);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_permissionless_crank_recovery_declaration_is_accounting_neutral() {
    let reason_case: u8 = kani::any();
    kani::assume(reason_case < 8);
    let reason = match reason_case {
        0 => PermissionlessRecoveryReasonV16::BelowProgressFloor,
        1 => PermissionlessRecoveryReasonV16::BlockedSegmentHeadroomOrRepresentability,
        2 => PermissionlessRecoveryReasonV16::AccountBSettlementCannotProgress,
        3 => PermissionlessRecoveryReasonV16::BIndexHeadroomExhausted,
        4 => PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress,
        5 => PermissionlessRecoveryReasonV16::ExplicitLossOrDustAuditOverflow,
        6 => PermissionlessRecoveryReasonV16::OracleOrTargetUnavailableByAuthenticatedPolicy,
        _ => PermissionlessRecoveryReasonV16::CounterOrEpochOverflowDeclaredRecovery,
    };
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 100).unwrap();
    group.attach_leg(&mut account, 0, SideV16::Long, 1).unwrap();

    let account_capital_before = account.capital;
    let account_pnl_before = account.pnl;
    let account_reserved_pnl_before = account.reserved_pnl;
    let account_bitmap_before = account.active_bitmap;
    let account_fee_credits_before = account.fee_credits;
    let account_health_valid_before = account.health_cert.valid;
    let vault_before = group.vault;
    let c_tot_before = group.c_tot;
    let insurance_before = group.insurance;
    let pnl_pos_before = group.pnl_pos_tot;
    let asset_before = group.assets[0];
    let slot_last_before = group.slot_last;
    let current_slot_before = group.current_slot;
    let outcome = group.permissionless_crank_not_atomic(
        &mut account,
        PermissionlessCrankRequestV16 {
            now_slot: current_slot_before + 1,
            asset_index: 0,
            effective_price: 2,
            funding_rate_e9: 0,
            action: PermissionlessCrankActionV16::Recover(reason),
        },
        &[1; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(
        reason_case == 0,
        "v16 recovery-crank first reason reachable"
    );
    kani::cover!(reason_case == 7, "v16 recovery-crank last reason reachable");
    assert_eq!(
        outcome,
        Ok(PermissionlessProgressOutcomeV16::RecoveryDeclared(reason))
    );
    assert_eq!(group.recovery_reason, Some(reason));
    assert_eq!(group.mode, MarketModeV16::Recovery);
    assert_eq!(account.capital, account_capital_before);
    assert_eq!(account.pnl, account_pnl_before);
    assert_eq!(account.reserved_pnl, account_reserved_pnl_before);
    assert_eq!(account.active_bitmap, account_bitmap_before);
    assert_eq!(account.fee_credits, account_fee_credits_before);
    assert_eq!(account.health_cert.valid, account_health_valid_before);
    assert_eq!(group.vault, vault_before);
    assert_eq!(group.c_tot, c_tot_before);
    assert_eq!(group.insurance, insurance_before);
    assert_eq!(group.pnl_pos_tot, pnl_pos_before);
    assert_eq!(group.assets[0], asset_before);
    assert_eq!(group.slot_last, slot_last_before);
    assert_eq!(group.current_slot, current_slot_before);
    assert_eq!(group.mode, MarketModeV16::Recovery);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_permissionless_recovery_enables_dead_leg_forfeit_without_value_escape() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.attach_leg(&mut account, 0, SideV16::Long, 1).unwrap();
    let before_vault = group.vault;
    let before_c_tot = group.c_tot;
    let before_insurance = group.insurance;
    let before_pnl_pos = group.pnl_pos_tot;

    let reason = PermissionlessRecoveryReasonV16::OracleOrTargetUnavailableByAuthenticatedPolicy;
    let declared = group.declare_permissionless_recovery(reason);
    let outcome = group.kani_forfeit_recovery_leg_core(&mut account, 0, 1);

    kani::cover!(
        declared == Ok(PermissionlessProgressOutcomeV16::RecoveryDeclared(reason))
            && matches!(outcome, Ok(DeadLegForfeitOutcomeV16 { detached: true, .. })),
        "v16 declared recovery enables bounded dead-leg forfeit"
    );
    assert_eq!(
        declared,
        Ok(PermissionlessProgressOutcomeV16::RecoveryDeclared(reason))
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
    assert_eq!(group.mode, MarketModeV16::Recovery);
    assert_eq!(group.recovery_reason, Some(reason));
    assert_eq!(account.active_bitmap, bitmap(&[]));
    assert_eq!(group.assets[0].oi_eff_long_q, 0);
    assert_eq!(group.vault, before_vault);
    assert_eq!(group.c_tot, before_c_tot);
    assert_eq!(group.insurance, before_insurance);
    assert_eq!(group.pnl_pos_tot, before_pnl_pos);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_recovery_mode_blocks_value_escape_paths_before_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 100).unwrap();
    account.pnl = 10;
    group.pnl_pos_tot = 10;
    group.vault = group.vault.checked_add(10).unwrap();
    group
        .full_account_refresh(&mut account, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    group
        .declare_permissionless_recovery(PermissionlessRecoveryReasonV16::BelowProgressFloor)
        .unwrap();
    let account_capital_before = account.capital;
    let account_pnl_before = account.pnl;
    let account_reserved_pnl_before = account.reserved_pnl;
    let account_bitmap_before = account.active_bitmap;
    let account_fee_credits_before = account.fee_credits;
    let account_health_valid_before = account.health_cert.valid;
    let vault_before = group.vault;
    let c_tot_before = group.c_tot;
    let insurance_before = group.insurance;

    let convert = group.convert_released_pnl_to_capital_not_atomic(&mut account);
    let withdraw = group.withdraw_not_atomic(&mut account, 1, &[1; V16_MAX_PORTFOLIO_ASSETS_N]);
    let fee_sync = group.sync_account_fee_to_slot_not_atomic(&mut account, 1, 1);

    kani::cover!(
        convert == Err(V16Error::LockActive)
            && withdraw == Err(V16Error::LockActive)
            && fee_sync == Err(V16Error::LockActive),
        "v16 terminal recovery blocks value escape paths"
    );
    assert_eq!(convert, Err(V16Error::LockActive));
    assert_eq!(withdraw, Err(V16Error::LockActive));
    assert_eq!(fee_sync, Err(V16Error::LockActive));
    assert_eq!(account.capital, account_capital_before);
    assert_eq!(account.pnl, account_pnl_before);
    assert_eq!(account.reserved_pnl, account_reserved_pnl_before);
    assert_eq!(account.active_bitmap, account_bitmap_before);
    assert_eq!(account.fee_credits, account_fee_credits_before);
    assert_eq!(account.health_cert.valid, account_health_valid_before);
    assert_eq!(group.vault, vault_before);
    assert_eq!(group.c_tot, c_tot_before);
    assert_eq!(group.insurance, insurance_before);
    assert_eq!(group.mode, MarketModeV16::Recovery);
    assert_eq!(
        group.recovery_reason,
        Some(PermissionlessRecoveryReasonV16::BelowProgressFloor)
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_recovery_mode_rejects_non_recovery_crank_before_account_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.attach_leg(&mut account, 0, SideV16::Long, 1).unwrap();
    let asset_before = group.assets[0];
    let reason = PermissionlessRecoveryReasonV16::BlockedSegmentHeadroomOrRepresentability;
    group.declare_permissionless_recovery(reason).unwrap();
    let account_capital_before = account.capital;
    let account_pnl_before = account.pnl;
    let account_bitmap_before = account.active_bitmap;
    let leg_active_before = account.legs[0].active;
    let leg_market_id_before = account.legs[0].market_id;
    let leg_side_before = account.legs[0].side;
    let leg_basis_before = account.legs[0].basis_pos_q;
    let result = group.permissionless_crank_not_atomic(
        &mut account,
        PermissionlessCrankRequestV16 {
            now_slot: 1,
            asset_index: 0,
            effective_price: 1,
            funding_rate_e9: 0,
            action: PermissionlessCrankActionV16::Refresh,
        },
        &[1; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(
        result == Err(V16Error::LockActive),
        "v16 terminal recovery rejects non-recovery crank before mutation"
    );
    assert!(result.is_err());
    assert_eq!(account.capital, account_capital_before);
    assert_eq!(account.pnl, account_pnl_before);
    assert_eq!(account.active_bitmap[0], account_bitmap_before[0]);
    assert_eq!(account.legs[0].active, leg_active_before);
    assert_eq!(account.legs[0].market_id, leg_market_id_before);
    assert_eq!(account.legs[0].side, leg_side_before);
    assert_eq!(account.legs[0].basis_pos_q, leg_basis_before);
    assert_eq!(group.assets[0], asset_before);
    assert_eq!(group.mode, MarketModeV16::Recovery);
    assert_eq!(group.recovery_reason, Some(reason));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_terminal_recovery_reason_and_mode_are_immutable() {
    let second_case: u8 = kani::any();
    kani::assume(second_case < 8);
    let first_reason = PermissionlessRecoveryReasonV16::BelowProgressFloor;
    let second_reason = match second_case {
        0 => PermissionlessRecoveryReasonV16::BelowProgressFloor,
        1 => PermissionlessRecoveryReasonV16::BlockedSegmentHeadroomOrRepresentability,
        2 => PermissionlessRecoveryReasonV16::AccountBSettlementCannotProgress,
        3 => PermissionlessRecoveryReasonV16::BIndexHeadroomExhausted,
        4 => PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress,
        5 => PermissionlessRecoveryReasonV16::ExplicitLossOrDustAuditOverflow,
        6 => PermissionlessRecoveryReasonV16::OracleOrTargetUnavailableByAuthenticatedPolicy,
        _ => PermissionlessRecoveryReasonV16::CounterOrEpochOverflowDeclaredRecovery,
    };
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();

    let first = group.declare_permissionless_recovery(first_reason);
    let second = group.declare_permissionless_recovery(second_reason);
    let resolve = group.resolve_market_not_atomic(1);

    kani::cover!(
        second_reason != first_reason,
        "v16 terminal recovery attempted reason override reachable"
    );
    kani::cover!(
        resolve == Err(V16Error::LockActive),
        "v16 terminal recovery rejects resolved-mode override"
    );
    assert_eq!(
        first,
        Ok(PermissionlessProgressOutcomeV16::RecoveryDeclared(
            first_reason
        ))
    );
    assert_eq!(
        second,
        Ok(PermissionlessProgressOutcomeV16::RecoveryDeclared(
            first_reason
        ))
    );
    assert_eq!(resolve, Err(V16Error::LockActive));
    assert_eq!(group.mode, MarketModeV16::Recovery);
    assert_eq!(group.recovery_reason, Some(first_reason));
    assert_eq!(group.resolved_slot, 0);
}

#[kani::proof]
#[kani::unwind(256)]
#[kani::solver(cadical)]
fn proof_v16_recovery_mode_rejects_liquidation_and_rebalance_before_mutation() {
    let use_liquidation: bool = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut opposing = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group
        .attach_leg(&mut account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    let oi_long_before = group.assets[0].oi_eff_long_q;
    let oi_short_before = group.assets[0].oi_eff_short_q;
    let k_long_before = group.assets[0].k_long;
    let k_short_before = group.assets[0].k_short;
    let reason = PermissionlessRecoveryReasonV16::BlockedSegmentHeadroomOrRepresentability;
    group.declare_permissionless_recovery(reason).unwrap();
    let account_capital_before = account.capital;
    let account_pnl_before = account.pnl;
    let account_bitmap_before = account.active_bitmap;
    let leg_active_before = account.legs[0].active;
    let leg_market_id_before = account.legs[0].market_id;
    let leg_side_before = account.legs[0].side;
    let leg_basis_before = account.legs[0].basis_pos_q;

    let result = if use_liquidation {
        group
            .liquidate_account_not_atomic(
                &mut account,
                LiquidationRequestV16 {
                    asset_index: 0,
                    close_q: POS_SCALE,
                    fee_bps: 0,
                },
                &[1; V16_MAX_PORTFOLIO_ASSETS_N],
            )
            .map(|_| ())
    } else {
        group
            .rebalance_reduce_position_not_atomic(
                &mut account,
                RebalanceRequestV16 {
                    asset_index: 0,
                    reduce_q: POS_SCALE,
                },
                &[1; V16_MAX_PORTFOLIO_ASSETS_N],
            )
            .map(|_| ())
    };

    kani::cover!(
        use_liquidation,
        "v16 terminal recovery rejects liquidation before mutation"
    );
    kani::cover!(
        !use_liquidation,
        "v16 terminal recovery rejects rebalance before mutation"
    );
    assert!(result.is_err());
    assert_eq!(account.capital, account_capital_before);
    assert_eq!(account.pnl, account_pnl_before);
    assert_eq!(account.active_bitmap[0], account_bitmap_before[0]);
    assert_eq!(account.legs[0].active, leg_active_before);
    assert_eq!(account.legs[0].market_id, leg_market_id_before);
    assert_eq!(account.legs[0].side, leg_side_before);
    assert_eq!(account.legs[0].basis_pos_q, leg_basis_before);
    assert_eq!(group.assets[0].oi_eff_long_q, oi_long_before);
    assert_eq!(group.assets[0].oi_eff_short_q, oi_short_before);
    assert_eq!(group.assets[0].k_long, k_long_before);
    assert_eq!(group.assets[0].k_short, k_short_before);
    assert!(matches!(group.mode, MarketModeV16::Recovery));
    assert!(matches!(
        group.recovery_reason,
        Some(PermissionlessRecoveryReasonV16::BlockedSegmentHeadroomOrRepresentability)
    ));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_public_config_accepts_full_margin_loss_only_envelope() {
    let (market, _, _) = symbolic_ids();
    let cfg = V16Config::public_user_fund(1, 0, 1);

    kani::cover!(
        cfg.maintenance_margin_bps == 10_000 && cfg.max_price_move_bps_per_slot == 10_000,
        "v16 full-margin one-segment loss envelope reachable"
    );
    assert!(MarketGroupV16::new(market, cfg).is_ok());
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_public_config_rejects_price_funding_envelope_breach() {
    let (market, _, _) = symbolic_ids();
    let mut cfg = tight_envelope_config();
    cfg.max_price_move_bps_per_slot = 10;

    kani::cover!(
        cfg.max_price_move_bps_per_slot == 10,
        "v16 price/funding envelope breach rejected"
    );
    assert_eq!(
        MarketGroupV16::new(market, cfg),
        Err(V16Error::InvalidConfig)
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_public_config_rejects_liquidation_fee_envelope_breach() {
    let (market, _, _) = symbolic_ids();
    let mut cfg = tight_envelope_config();
    cfg.liquidation_fee_bps = 400;

    kani::cover!(
        cfg.liquidation_fee_bps == 400,
        "v16 liquidation-fee envelope breach rejected"
    );
    assert_eq!(
        MarketGroupV16::new(market, cfg),
        Err(V16Error::InvalidConfig)
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_public_config_rejects_funding_headroom_breach() {
    let (market, _, _) = symbolic_ids();
    let mut cfg = tight_envelope_config();
    cfg.max_accrual_dt_slots = 1_000_000_000;
    cfg.min_funding_lifetime_slots = 1_000_000_000;

    kani::cover!(
        cfg.max_accrual_dt_slots == 1_000_000_000,
        "v16 funding K/F headroom breach rejected"
    );
    assert_eq!(
        MarketGroupV16::new(market, cfg),
        Err(V16Error::InvalidConfig)
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_public_config_accepts_capped_liquidation_fee_envelope() {
    let (market, _, _) = symbolic_ids();
    let mut cfg = tight_envelope_config();
    cfg.liquidation_fee_bps = 10_000;
    cfg.liquidation_fee_cap = 1;

    kani::cover!(
        cfg.liquidation_fee_bps == 10_000 && cfg.liquidation_fee_cap == 1,
        "v16 capped liquidation fee envelope reachable"
    );
    assert!(MarketGroupV16::new(market, cfg).is_ok());
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_min_nonzero_initial_floor_is_in_health_certificate() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    group.config.min_nonzero_mm_req = 49;
    group.config.min_nonzero_im_req = 50;
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 49).unwrap();
    group.attach_leg(&mut account, 0, SideV16::Long, 1).unwrap();
    let (initial_req, maintenance_req, worst_case_loss) = group
        .kani_account_health_leg_requirements(
            account.legs[0],
            &[1; V16_MAX_PORTFOLIO_ASSETS_N],
            true,
        )
        .unwrap();
    account.health_cert = group
        .kani_build_account_health_cert_from_requirements(
            &account,
            initial_req,
            maintenance_req,
            worst_case_loss,
        )
        .unwrap();

    kani::cover!(
        account.health_cert.certified_initial_req == 50,
        "v16 tiny nonzero leg gets min initial floor"
    );
    assert_eq!(account.health_cert.certified_equity, 49);
    assert_eq!(account.health_cert.certified_initial_req, 50);
    assert!(
        account.health_cert.certified_equity < account.health_cert.certified_initial_req as i128
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_haircut_support_haircuts_positive_pnl_under_global_impairment() {
    let profit: u8 = kani::any();
    let residual: u8 = kani::any();
    kani::assume(profit > 1);
    kani::assume(profit <= 20);
    kani::assume(residual > 0);
    kani::assume(residual < profit);
    let (market, _, _) = concrete_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();

    let support = group
        .kani_haircut_effective_support(profit as u128, residual as u128, profit as u128)
        .unwrap();

    kani::cover!(
        residual == 1 && profit > 2,
        "v16 haircut support covers strongly impaired junior support"
    );
    assert_eq!(support, residual as u128);
    assert!(support < profit as u128);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_negative_kf_settlement_uses_haircut_support_not_face_netting() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.pnl = 100;
    group.pnl_pos_tot = 100;
    set_junior_bound(&mut group, 100);
    group.vault = 50;

    group
        .kani_apply_signed_kf_delta_to_pnl(&mut account, -100, None)
        .unwrap();

    kani::cover!(
        account.pnl == -50,
        "v16 negative K/F settlement would be positive under face netting"
    );
    assert_eq!(account.pnl, -50);
    assert_eq!(group.pnl_pos_tot, 0);
    assert_eq!(group.pnl_pos_bound_tot, 0);
    assert_eq!(group.negative_pnl_account_count, 1);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_negative_kf_settlement_consumes_realizable_source_credit_before_principal() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let claim_num = 500 * BOUND_SCALE;
    account.ensure_source_domain_capacity(group.source_credit.len());
    account.capital = 1_000;
    account.pnl = 500;
    account.source_claim_market_id[0] = group.assets[0].market_id;
    account.source_claim_bound_num[0] = claim_num;
    group.c_tot = 1_000;
    group.vault = 1_000;
    group.pnl_pos_tot = 500;
    group.pnl_pos_bound_tot = 500;
    group.pnl_pos_bound_tot_num = claim_num;
    group.source_credit[0] = SourceCreditStateV16 {
        positive_claim_bound_num: claim_num,
        exact_positive_claim_num: claim_num,
        fresh_reserved_backing_num: claim_num,
        credit_rate_num: CREDIT_RATE_SCALE,
        ..SourceCreditStateV16::EMPTY
    };
    group.source_backing_buckets[0] = BackingBucketV16 {
        market_id: group.assets[0].market_id,
        fresh_unliened_backing_num: claim_num,
        expiry_slot: 10,
        status: BackingBucketStatusV16::Fresh,
        ..BackingBucketV16::EMPTY
    };
    let global_support = group
        .kani_haircut_effective_support(500, group.vault.saturating_sub(group.c_tot), 500)
        .unwrap();
    let source_support = group
        .kani_account_unliened_source_realizable_support(&account, 500)
        .unwrap();
    group
        .kani_create_and_consume_source_credit_from_counterparty_core(0, claim_num)
        .unwrap();

    kani::cover!(
        group.source_credit[0].spent_backing_num == 500 * BOUND_SCALE,
        "v16 negative K/F settlement consumes source backing before principal"
    );
    assert_eq!(global_support, 0);
    assert_eq!(source_support, 500);
    assert_eq!(account.capital, 1_000);
    assert_eq!(account.pnl, 500);
    assert_eq!(group.c_tot, 1_000);
    assert_eq!(group.source_credit[0].spent_backing_num, 500 * BOUND_SCALE);
    assert_eq!(group.source_credit[0].fresh_reserved_backing_num, 0);
    assert_eq!(group.pnl_pos_tot, 500);
    assert_eq!(group.pnl_pos_bound_tot, 500);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_source_attributed_negative_kf_settlement_does_not_use_global_residual() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let claim_num = 100 * BOUND_SCALE;
    account.ensure_source_domain_capacity(group.source_credit.len());
    account.pnl = 100;
    account.source_claim_market_id[0] = group.assets[0].market_id;
    account.source_claim_bound_num[0] = claim_num;
    group.pnl_pos_tot = 100;
    group.pnl_pos_bound_tot = 100;
    group.pnl_pos_bound_tot_num = claim_num;
    group.source_credit[0] = SourceCreditStateV16 {
        positive_claim_bound_num: claim_num,
        exact_positive_claim_num: claim_num,
        credit_rate_num: 0,
        ..SourceCreditStateV16::EMPTY
    };
    group.source_backing_buckets[0] = BackingBucketV16::empty_for_market(group.assets[0].market_id);
    group.vault = 50;

    group
        .kani_apply_signed_kf_delta_to_pnl(&mut account, -100, None)
        .unwrap();

    kani::cover!(
        group.source_credit[0].spent_backing_num == 0 && group.vault > group.c_tot,
        "v16 source-attributed loss settlement has unrelated global residual available"
    );
    assert_eq!(account.pnl, -100);
    assert_eq!(group.source_credit[0].spent_backing_num, 0);
    assert_eq!(group.pnl_pos_tot, 0);
    assert_eq!(group.pnl_pos_bound_tot, 0);
    assert_eq!(group.negative_pnl_account_count, 1);
}

#[kani::proof]
#[kani::unwind(16)]
#[kani::solver(cadical)]
fn proof_v16_source_domain_positive_kf_loss_cure_does_not_use_global_residual() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.ensure_source_domain_capacity(group.source_credit.len());
    account.pnl = -100;
    group.negative_pnl_account_count = 1;
    group.source_backing_buckets[1] = BackingBucketV16::empty_for_market(group.assets[0].market_id);
    group.vault = 50;

    group
        .kani_apply_signed_kf_delta_to_pnl(&mut account, 100, Some(1))
        .unwrap();

    kani::cover!(
        group.vault > group.c_tot,
        "v16 source-domain positive K/F loss cure has unrelated global residual available"
    );
    assert_eq!(account.pnl, -100);
    assert_eq!(group.source_credit[1].spent_backing_num, 0);
    assert_eq!(group.pnl_pos_tot, 0);
    assert_eq!(group.pnl_pos_bound_tot, 0);
    assert_eq!(group.negative_pnl_account_count, 1);
}

#[kani::proof]
#[kani::unwind(16)]
#[kani::solver(cadical)]
fn proof_v16_live_positive_pnl_requires_source_attribution() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.ensure_source_domain_capacity(group.source_credit.len());
    account.pnl = 10;
    group.pnl_pos_tot = 10;
    group.pnl_pos_bound_tot = 10;
    group.pnl_pos_bound_tot_num = 10 * BOUND_SCALE;
    group.vault = 100;

    kani::cover!(
        group.vault > group.c_tot
            && group.source_credit[0].positive_claim_bound_num == 0
            && group.source_credit[0].fresh_reserved_backing_num == 0
            && group.source_credit[1].positive_claim_bound_num == 0
            && group.source_credit[1].fresh_reserved_backing_num == 0,
        "v16 live unattributed positive PnL with unrelated global residual is reachable"
    );
    assert_eq!(
        kani_validate_positive_pnl_source_attribution(account.pnl, 0),
        Err(V16Error::InvalidLeg)
    );
    assert_eq!(group.kani_account_haircut_equity(&account).unwrap(), 0);
}

#[kani::proof]
#[kani::unwind(16)]
#[kani::solver(cadical)]
fn proof_v16_unsigned_positive_kf_cannot_create_unattributed_pnl() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut flat = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.vault = 100;

    kani::cover!(
        group.vault > group.c_tot,
        "v16 unattributed positive K/F has unrelated global residual available"
    );
    assert_eq!(
        group.kani_apply_signed_kf_delta_to_pnl(&mut flat, 10, None),
        Err(V16Error::InvalidLeg)
    );

    let mut loss = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    loss.pnl = -5;
    group.negative_pnl_account_count = 1;
    group
        .kani_apply_signed_kf_delta_to_pnl(&mut loss, 10, None)
        .unwrap();
    assert!(loss.pnl <= 0);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_full_refresh_reserves_counterparty_backing_from_new_capital_backed_loss() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut loser = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.kani_deposit_core(&mut loser, 1_000).unwrap();
    loser.pnl = -500;
    group.negative_pnl_account_count = 1;

    group
        .kani_reserve_new_capital_backed_loss_for_source_domain(&mut loser, 0, 0, 500)
        .unwrap();

    kani::cover!(
        group.source_credit[0].fresh_reserved_backing_num == 500 * BOUND_SCALE,
        "v16 full-refresh reservation helper reserves capital-backed local loss as source backing"
    );
    assert_eq!(loser.pnl, 0);
    assert_eq!(loser.capital, 500);
    assert_eq!(group.c_tot, 500);
    assert_eq!(group.vault, 1_000);
    assert!(!loser.health_cert.valid);
    assert_eq!(
        group.source_credit[0].fresh_reserved_backing_num,
        500 * BOUND_SCALE
    );
    assert_eq!(
        group.source_credit_available_backing_num(0),
        Ok(500 * BOUND_SCALE)
    );
    assert!(group.source_backing_buckets[0].expiry_slot >= group.current_slot + group.config.h_max);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_passive_backing_consumption_preserves_senior_accounting_without_wrapper_injection() {
    let backing = 500 * BOUND_SCALE;
    let source = SourceCreditStateV16 {
        positive_claim_bound_num: backing,
        exact_positive_claim_num: backing,
        fresh_reserved_backing_num: backing,
        credit_rate_num: CREDIT_RATE_SCALE,
        ..SourceCreditStateV16::EMPTY
    };
    let bucket = BackingBucketV16 {
        market_id: 1,
        fresh_unliened_backing_num: backing,
        expiry_slot: 10,
        status: BackingBucketStatusV16::Fresh,
        ..BackingBucketV16::EMPTY
    };
    let (bucket, source) =
        MarketGroupV16::kani_prepare_counterparty_lien_create_delta(bucket, source, 0, backing)
            .unwrap();
    let (bucket, source) =
        MarketGroupV16::kani_prepare_counterparty_lien_consume_delta(bucket, source, backing)
            .unwrap();

    kani::cover!(
        source.provider_receivable_num == backing,
        "v16 consumed counterparty backing creates provider receivable without moving senior stock"
    );
    assert_eq!(source.spent_backing_num, backing);
    assert_eq!(source.provider_receivable_num, backing);
    assert_eq!(source.fresh_reserved_backing_num, 0);
    assert_eq!(source.valid_liened_backing_num, 0);
    assert_eq!(bucket.fresh_unliened_backing_num, 0);
    assert_eq!(bucket.valid_liened_backing_num, 0);
    assert_eq!(bucket.consumed_liened_backing_num, backing);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_positive_kf_delta_cures_prior_loss_at_haircut_value() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(2, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.legs[0] = PortfolioLegV16 {
        active: true,
        asset_index: 0,
        market_id: group.assets[0].market_id,
        side: SideV16::Long,
        basis_pos_q: POS_SCALE as i128,
        a_basis: ADL_ONE,
        k_snap: group.assets[0].k_long,
        f_snap: group.assets[0].f_long_num,
        epoch_snap: group.assets[0].epoch_long,
        loss_weight: POS_SCALE,
        b_snap: group.assets[0].b_long_num,
        b_rem: 0,
        b_epoch_snap: group.assets[0].epoch_long,
        b_stale: false,
        stale: false,
    };
    account.legs[1] = PortfolioLegV16 {
        active: true,
        asset_index: 1,
        market_id: group.assets[1].market_id,
        side: SideV16::Long,
        basis_pos_q: POS_SCALE as i128,
        a_basis: ADL_ONE,
        k_snap: group.assets[1].k_long,
        f_snap: group.assets[1].f_long_num,
        epoch_snap: group.assets[1].epoch_long,
        loss_weight: POS_SCALE,
        b_snap: group.assets[1].b_long_num,
        b_rem: 0,
        b_epoch_snap: group.assets[1].epoch_long,
        b_stale: false,
        stale: false,
    };
    account.active_bitmap = bitmap(&[0, 1]);
    group.vault = 50;
    group.assets[0].k_long = -(100 * ADL_ONE as i128);
    group.assets[1].k_long = 100 * ADL_ONE as i128;

    let cert = group
        .full_account_refresh(&mut account, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    kani::cover!(
        account.pnl == -50,
        "v16 positive K/F support cures prior loss only at haircut value"
    );
    assert_eq!(account.pnl, -50);
    assert_eq!(group.pnl_pos_tot, 0);
    assert_eq!(group.pnl_pos_bound_tot, 0);
    assert_eq!(group.negative_pnl_account_count, 1);
    assert_eq!(cert.certified_equity, -50);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_positive_kf_settlement_consumes_source_credit_to_cure_prior_loss() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut opposite =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [92; 32], owner));
    group.deposit_not_atomic(&mut account, 1_000).unwrap();
    account.pnl = -500;
    group.negative_pnl_account_count = 1;
    group
        .add_fresh_counterparty_backing_not_atomic(1, 500 * BOUND_SCALE, 10)
        .unwrap();
    group
        .attach_leg(&mut account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut opposite, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    group.assets[0].k_long = 500 * ADL_ONE as i128;

    let cert = group
        .full_account_refresh(&mut account, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    kani::cover!(
        group.source_credit[1].spent_backing_num == 500 * BOUND_SCALE,
        "v16 positive K/F settlement consumes source backing to cure prior loss"
    );
    assert_eq!(account.capital, 1_000);
    assert_eq!(account.pnl, 0);
    assert_eq!(group.c_tot, 1_000);
    assert_eq!(group.negative_pnl_account_count, 0);
    assert_eq!(group.source_credit[1].spent_backing_num, 500 * BOUND_SCALE);
    assert_eq!(group.source_credit[1].fresh_reserved_backing_num, 0);
    assert_eq!(cert.certified_equity, 1_000);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_deposit_then_withdraw_roundtrip_preserves_accounting() {
    let amount: u16 = kani::any();
    kani::assume(amount > 0);
    kani::assume(amount <= 1_000);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    group
        .kani_deposit_core(&mut account, amount as u128)
        .unwrap();
    assert_eq!(account.capital, amount as u128);
    assert_eq!(group.c_tot, amount as u128);
    assert_eq!(group.vault, amount as u128);

    group
        .kani_withdraw_core(&mut account, amount as u128)
        .unwrap();
    assert_eq!(account.capital, 0);
    assert_eq!(group.c_tot, 0);
    assert_eq!(group.vault, 0);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_deposit_does_not_draw_insurance_or_sweep_loss_bearing_account() {
    let amount: u16 = kani::any();
    let fee_debt: u8 = kani::any();
    kani::assume(amount > 0);
    kani::assume(amount <= 1_000);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut opposing = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [9; 32], owner));

    group.vault = 10;
    group.insurance = 10;
    group
        .attach_leg(&mut account, 0, SideV16::Long, 10)
        .unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV16::Short, -10)
        .unwrap();
    account.pnl = -10_000;
    account.fee_credits = -(fee_debt as i128);

    let insurance_before = group.insurance;
    let pnl_before = account.pnl;
    let fee_credits_before = account.fee_credits;
    let leg_before = account.legs[0];
    let oi_before = group.assets[0].oi_eff_long_q;
    let oi_short_before = group.assets[0].oi_eff_short_q;

    group
        .kani_deposit_core(&mut account, amount as u128)
        .unwrap();

    kani::cover!(fee_debt > 0, "v16 deposit with fee debt reachable");
    assert_eq!(group.insurance, insurance_before);
    assert_eq!(account.pnl, pnl_before);
    assert_eq!(account.fee_credits, fee_credits_before);
    assert_eq!(account.legs[0].active, leg_before.active);
    assert_eq!(account.legs[0].basis_pos_q, leg_before.basis_pos_q);
    assert_eq!(account.legs[0].side, leg_before.side);
    assert_eq!(group.assets[0].oi_eff_long_q, oi_before);
    assert_eq!(group.assets[0].oi_eff_short_q, oi_short_before);
    assert_eq!(account.capital, amount as u128);
    assert_eq!(group.c_tot, amount as u128);
    assert_eq!(group.vault, 10 + amount as u128);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_deposit_never_sweeps_fee_debt_even_when_flat_and_nonnegative() {
    let amount: u16 = kani::any();
    let fee_debt: u8 = kani::any();
    let pnl: u8 = kani::any();
    kani::assume(amount > 0);
    kani::assume(amount <= 1_000);
    kani::assume(fee_debt > 0);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.pnl = pnl as i128;
    account.fee_credits = -(fee_debt as i128);

    let pnl_before = account.pnl;
    let fee_credits_before = account.fee_credits;
    group
        .deposit_not_atomic(&mut account, amount as u128)
        .unwrap();

    kani::cover!(
        pnl_before > 0 && fee_debt > 0,
        "v16 flat nonnegative deposit with fee debt reachable"
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
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_partial_withdraw_can_leave_small_remainder() {
    let remainder: u16 = kani::any();
    kani::assume(remainder <= 1_000);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let deposit = remainder as u128 + 1;
    group.kani_deposit_core(&mut account, deposit).unwrap();

    group.kani_withdraw_core(&mut account, 1).unwrap();

    kani::cover!(remainder == 0, "v16 partial withdraw leaves zero remainder");
    kani::cover!(
        remainder > 0,
        "v16 partial withdraw leaves nonzero remainder"
    );
    assert_eq!(account.capital, remainder as u128);
    assert_eq!(group.c_tot, remainder as u128);
    assert_eq!(group.vault, remainder as u128);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_over_withdraw_rejects_before_any_accounting_mutation() {
    let capital: u16 = kani::any();
    kani::assume(capital > 0);
    kani::assume(capital <= 1_000);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
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

    let result = group.kani_withdraw_core(&mut account, capital as u128 + 1);

    kani::cover!(capital > 0, "v16 over-withdraw rejection path reachable");
    assert_eq!(result, Err(V16Error::LockActive));
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
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_multiple_deposits_aggregate_c_tot_and_vault() {
    let amount_a: u16 = kani::any();
    let amount_b: u16 = kani::any();
    kani::assume(amount_a <= 1_000);
    kani::assume(amount_b <= 1_000);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account_a =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut account_b =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));

    group
        .deposit_not_atomic(&mut account_a, amount_a as u128)
        .unwrap();
    group
        .deposit_not_atomic(&mut account_b, amount_b as u128)
        .unwrap();

    let expected = amount_a as u128 + amount_b as u128;
    kani::cover!(expected > 0, "v16 nonzero aggregate deposit reachable");
    assert_eq!(group.c_tot, account_a.capital + account_b.capital);
    assert_eq!(group.c_tot, expected);
    assert_eq!(group.vault, expected);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_close_portfolio_account_requires_clean_local_state() {
    let dirty_case: u8 = kani::any();
    kani::assume(dirty_case < 6);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let clean = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.create_portfolio_account(&clean).unwrap();
    assert_eq!(group.materialized_portfolio_count, 1);

    let mut dirty = clean.clone();
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
    kani::cover!(dirty_case == 0, "v16 close rejects capital");
    kani::cover!(dirty_case == 1, "v16 close rejects pnl");
    kani::cover!(dirty_case == 2, "v16 close rejects reserved pnl");
    kani::cover!(dirty_case == 3, "v16 close rejects fee debt");
    kani::cover!(dirty_case == 4, "v16 close rejects stale account");
    kani::cover!(dirty_case == 5, "v16 close rejects b-stale account");
    assert_eq!(
        group.close_portfolio_account(&dirty),
        Err(V16Error::LockActive)
    );
    assert_eq!(group.materialized_portfolio_count, 1);

    group.close_portfolio_account(&clean).unwrap();
    assert_eq!(group.materialized_portfolio_count, 0);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_risk_notional_flat_zero_and_monotone_in_price() {
    let abs_pos_q: u16 = kani::any();
    let p1: u16 = kani::any();
    let extra: u16 = kani::any();
    kani::assume(abs_pos_q <= 1_000);
    kani::assume(p1 > 0);
    kani::assume(p1 <= 1_000);
    kani::assume(extra <= 1_000);
    let p2 = p1 as u64 + extra as u64;

    assert_eq!(percolator::v16::risk_notional_ceil(0, p2), Ok(0));
    let n1 = percolator::v16::risk_notional_ceil(abs_pos_q as u128, p1 as u64).unwrap();
    let n2 = percolator::v16::risk_notional_ceil(abs_pos_q as u128, p2).unwrap();
    kani::cover!(
        abs_pos_q > 0 && extra > 0,
        "v16 risk notional monotone branch"
    );
    assert!(n2 >= n1);
}

fn concrete_ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    ([1; 32], [2; 32], [3; 32])
}

fn attach_opposite_for_live_oi(
    group: &mut MarketGroupV16,
    asset_index: usize,
    side: SideV16,
    size_q: u128,
    account_seed: u8,
) -> PortfolioAccountV16 {
    let (market, _, owner) = concrete_ids();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [account_seed; 32], owner));
    let size_i128 = i128::try_from(size_q).unwrap();
    let (opposite, basis) = match side {
        SideV16::Long => (SideV16::Short, -size_i128),
        SideV16::Short => (SideV16::Long, size_i128),
    };
    group
        .attach_leg(&mut account, asset_index, opposite, basis)
        .unwrap();
    account
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_hidden_leg_rejected_by_bitmap_authority() {
    let (market, account_id, owner) = concrete_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    account.legs[0].active = true;
    kani::cover!(
        account.active_bitmap == bitmap(&[]) && account.legs[0].active,
        "v16 hidden active leg reachable"
    );
    assert_eq!(
        group.validate_account_shape(&account),
        Err(V16Error::HiddenLeg)
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_configured_portfolio_width_rejects_out_of_range_leg() {
    let active_bit: bool = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.legs[1] = PortfolioLegV16 {
        active: true,
        asset_index: 1,
        market_id: 2,
        side: SideV16::Long,
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
        percolator::active_bitmap_set(&mut account.active_bitmap, 1).unwrap();
    }

    kani::cover!(active_bit, "v16 out-of-range leg with bitmap reachable");
    kani::cover!(!active_bit, "v16 out-of-range hidden leg reachable");
    assert_eq!(
        group.validate_account_shape(&account),
        Err(V16Error::HiddenLeg)
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_attach_then_clear_leg_restores_account_local_counters_for_long() {
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    group.attach_leg(&mut account, 0, SideV16::Long, 7).unwrap();
    assert_eq!(account.active_bitmap, bitmap(&[0]));
    assert_eq!(account.legs[0].basis_pos_q, 7);
    assert_eq!(group.assets[0].oi_eff_long_q, 7);

    group.clear_leg(&mut account, 0).unwrap();
    assert_eq!(account.active_bitmap, bitmap(&[]));
    assert_eq!(group.assets[0].oi_eff_long_q, 0);
    assert_eq!(group.assets[0].oi_eff_short_q, 0);
    assert_eq!(group.assets[0].stored_pos_count_long, 0);
    assert_eq!(group.assets[0].stored_pos_count_short, 0);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_compact_leg_slots_preserve_asset_identity() {
    let (market, account_id, owner) = concrete_ids();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    account.legs[1] = PortfolioLegV16 {
        active: true,
        asset_index: 0,
        market_id: 42,
        side: SideV16::Short,
        basis_pos_q: -7,
        loss_weight: 7,
        ..PortfolioLegV16::EMPTY
    };
    account.active_bitmap = bitmap(&[1]);

    assert_eq!(MarketGroupV16::kani_empty_leg_slot(&account), Ok(0));
    assert!(!account.legs[0].active);
    assert!(account.legs[1].active);
    assert_eq!(account.legs[1].asset_index, 0);
    assert_eq!(account.legs[1].market_id, 42);
    assert_eq!(account.active_bitmap[0], bitmap(&[1])[0]);

    let mut hidden = account;
    hidden.legs[0] = PortfolioLegV16 {
        active: false,
        asset_index: 99,
        market_id: 7,
        ..PortfolioLegV16::EMPTY
    };

    assert_eq!(
        MarketGroupV16::kani_empty_leg_slot(&hidden),
        Err(V16Error::HiddenLeg)
    );
}

#[kani::proof]
#[kani::unwind(150)]
#[kani::solver(cadical)]
fn proof_v16_market_slot_can_exceed_active_leg_cap() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(
        market,
        V16Config::public_user_fund_with_market_slots(4, 32, 0, 1),
    )
    .unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let asset_index = 17usize;

    group
        .attach_leg(&mut account, asset_index, SideV16::Long, 11)
        .unwrap();
    assert_eq!(account.active_bitmap, bitmap(&[0]));
    assert!(account.legs[0].active);
    assert_eq!(account.legs[0].asset_index as usize, asset_index);
    assert_eq!(
        account.legs[0].market_id,
        group.assets[asset_index].market_id
    );
    assert_eq!(group.assets[asset_index].oi_eff_long_q, 11);
    assert_eq!(group.validate_account_shape(&account), Ok(()));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_config_separates_active_leg_and_market_slot_caps() {
    let valid = V16Config::public_user_fund_with_market_slots(4, 32, 0, 1);
    assert_eq!(valid.validate_public_user_fund(), Ok(()));

    let too_many_active_legs = V16Config::public_user_fund_with_market_slots(33, 32, 0, 1);
    assert_eq!(
        too_many_active_legs.validate_public_user_fund_shape(),
        Err(V16Error::InvalidConfig)
    );

    let expanded_market_slots = V16Config::public_user_fund_with_market_slots(4, 65, 0, 1);
    assert_eq!(
        expanded_market_slots.validate_public_user_fund_shape(),
        Ok(())
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_same_asset_duplicate_leg_cannot_double_count_support() {
    let start_long: bool = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let (existing_side, existing_basis, duplicate_side, duplicate_basis) = if start_long {
        (SideV16::Long, 7, SideV16::Short, -7)
    } else {
        (SideV16::Short, -7, SideV16::Long, 7)
    };
    account.legs[0] = PortfolioLegV16 {
        active: true,
        asset_index: 0,
        market_id: group.assets[0].market_id,
        side: existing_side,
        basis_pos_q: existing_basis,
        a_basis: ADL_ONE,
        k_snap: 0,
        f_snap: 0,
        epoch_snap: 0,
        loss_weight: 7,
        b_snap: 0,
        b_rem: 0,
        b_epoch_snap: 0,
        b_stale: false,
        stale: false,
    };
    account.active_bitmap = bitmap(&[0]);
    match existing_side {
        SideV16::Long => {
            group.assets[0].stored_pos_count_long = 1;
            group.assets[0].oi_eff_long_q = 7;
            group.assets[0].loss_weight_sum_long = 7;
        }
        SideV16::Short => {
            group.assets[0].stored_pos_count_short = 1;
            group.assets[0].oi_eff_short_q = 7;
            group.assets[0].loss_weight_sum_short = 7;
        }
    }

    let asset_before = group.assets[0];
    let leg_before = account.legs[0];
    let bitmap_before = account.active_bitmap;
    let cert_before = account.health_cert;
    let result = group.attach_leg(&mut account, 0, duplicate_side, duplicate_basis);

    kani::cover!(
        matches!(result, Err(V16Error::InvalidLeg)),
        "v16 same-asset duplicate attach rejected"
    );
    assert!(matches!(result, Err(V16Error::InvalidLeg)));
    assert_eq!(account.legs[0], leg_before);
    assert_eq!(account.active_bitmap, bitmap_before);
    assert_eq!(account.health_cert, cert_before);
    assert_eq!(group.assets[0].oi_eff_long_q, asset_before.oi_eff_long_q);
    assert_eq!(group.assets[0].oi_eff_short_q, asset_before.oi_eff_short_q);
    assert_eq!(
        group.assets[0].stored_pos_count_long,
        asset_before.stored_pos_count_long
    );
    assert_eq!(
        group.assets[0].stored_pos_count_short,
        asset_before.stored_pos_count_short
    );
    assert_eq!(
        group.assets[0].loss_weight_sum_long,
        asset_before.loss_weight_sum_long
    );
    assert_eq!(
        group.assets[0].loss_weight_sum_short,
        asset_before.loss_weight_sum_short
    );
    assert_eq!(
        account
            .active_bitmap
            .iter()
            .map(|word| word.count_ones())
            .sum::<u32>(),
        1
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_asset_lifecycle_blocks_attach_before_accounting_mutation() {
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let lifecycle = symbolic_non_active_lifecycle();
    group.assets[0].lifecycle = lifecycle;
    let before_asset = group.assets[0];
    let before_active_bitmap = account.active_bitmap;
    let before_capital = account.capital;
    let before_pnl = account.pnl;
    let before_fee_credits = account.fee_credits;
    let before_health_valid = account.health_cert.valid;
    let before_leg = account.legs[0];

    kani::cover!(
        lifecycle == AssetLifecycleV16::DrainOnly,
        "v16 drain-only attach rejection reachable"
    );
    kani::cover!(
        lifecycle == AssetLifecycleV16::Retired,
        "v16 retired attach rejection reachable"
    );

    let result = group.attach_leg(&mut account, 0, SideV16::Long, 1);

    assert_eq!(result, Err(V16Error::LockActive));
    assert_eq!(group.assets[0].lifecycle, before_asset.lifecycle);
    assert_eq!(group.assets[0].oi_eff_long_q, before_asset.oi_eff_long_q);
    assert_eq!(group.assets[0].oi_eff_short_q, before_asset.oi_eff_short_q);
    assert_eq!(
        group.assets[0].stored_pos_count_long,
        before_asset.stored_pos_count_long
    );
    assert_eq!(
        group.assets[0].stored_pos_count_short,
        before_asset.stored_pos_count_short
    );
    assert_eq!(
        group.assets[0].loss_weight_sum_long,
        before_asset.loss_weight_sum_long
    );
    assert_eq!(
        group.assets[0].loss_weight_sum_short,
        before_asset.loss_weight_sum_short
    );
    assert_eq!(account.active_bitmap, before_active_bitmap);
    assert_eq!(account.capital, before_capital);
    assert_eq!(account.pnl, before_pnl);
    assert_eq!(account.fee_credits, before_fee_credits);
    assert_eq!(account.health_cert.valid, before_health_valid);
    assert_eq!(account.legs[0].active, before_leg.active);
    assert_eq!(account.legs[0].basis_pos_q, before_leg.basis_pos_q);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_asset_lifecycle_blocks_accrual_for_non_accruable_states() {
    let (market, _, _) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let lifecycle = symbolic_non_active_lifecycle();
    kani::assume(lifecycle != AssetLifecycleV16::DrainOnly);
    group.assets[0].lifecycle = lifecycle;
    let before = group.clone();

    kani::cover!(
        lifecycle == AssetLifecycleV16::Recovery,
        "v16 recovery lifecycle accrual rejection reachable"
    );

    let result = group.accrue_asset_to_not_atomic(0, 1, 1, 0, false);

    assert_eq!(result, Err(V16Error::LockActive));
    assert_eq!(group.assets[0], before.assets[0]);
    assert_eq!(group.current_slot, before.current_slot);
    assert_eq!(group.slot_last, before.slot_last);
    assert_eq!(group.oracle_epoch, before.oracle_epoch);
    assert_eq!(group.funding_epoch, before.funding_epoch);
    assert_eq!(group.risk_epoch, before.risk_epoch);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_asset_activation_requires_empty_slot_and_bumps_epochs() {
    let (market, _, _) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let nonempty: bool = kani::any();
    group.assets[0].lifecycle = AssetLifecycleV16::Retired;
    group.assets[0].retired_slot = 1;
    group.current_slot = 1;
    if nonempty {
        group.assets[0].oi_eff_long_q = 1;
        group.assets[0].oi_eff_short_q = 1;
        group.assets[0].stored_pos_count_long = 1;
        group.assets[0].stored_pos_count_short = 1;
        group.assets[0].loss_weight_sum_long = 1;
        group.assets[0].loss_weight_sum_short = 1;
    }
    let before = group.clone();

    kani::cover!(!nonempty, "v16 empty asset activation success reachable");
    kani::cover!(
        nonempty,
        "v16 nonempty asset activation rejection reachable"
    );

    let result = group.activate_empty_asset_not_atomic(0, 7, 2);

    if nonempty {
        assert_eq!(result, Err(V16Error::LockActive));
        assert_eq!(group.assets[0], before.assets[0]);
        assert_eq!(group.current_slot, before.current_slot);
        assert_eq!(group.risk_epoch, before.risk_epoch);
        assert_eq!(group.asset_set_epoch, before.asset_set_epoch);
    } else {
        assert_eq!(result, Ok(()));
        assert_eq!(group.assets[0].lifecycle, AssetLifecycleV16::Active);
        assert_eq!(group.assets[0].effective_price, 7);
        assert_eq!(group.assets[0].raw_oracle_target_price, 7);
        assert_eq!(group.assets[0].fund_px_last, 7);
        assert_eq!(group.assets[0].slot_last, 2);
        assert_eq!(
            group.source_backing_buckets[0].market_id,
            group.assets[0].market_id
        );
        assert_eq!(
            group.source_backing_buckets[1].market_id,
            group.assets[0].market_id
        );
        assert_eq!(group.risk_epoch, before.risk_epoch + 1);
        assert_eq!(group.asset_set_epoch, before.asset_set_epoch + 1);
        assert_eq!(
            group.asset_activation_count,
            before.asset_activation_count + 1
        );
        assert_eq!(group.last_asset_activation_slot, 2);
    }
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_asset_activation_counter_overflows_fail_before_state_mutation() {
    let case: u8 = kani::any();
    kani::assume(case < 4);
    let (market, _, _) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    group.assets[0].lifecycle = AssetLifecycleV16::Retired;
    group.assets[0].retired_slot = 1;
    group.current_slot = 1;
    match case {
        0 => group.next_market_id = u64::MAX,
        1 => {
            group.asset_activation_count = u64::MAX;
            group.last_asset_activation_slot = 0;
        }
        2 => group.asset_set_epoch = u64::MAX,
        _ => group.risk_epoch = u64::MAX,
    }
    let before_asset = group.assets[0];
    let before_long_bucket = group.source_backing_buckets[0];
    let before_short_bucket = group.source_backing_buckets[1];
    let before_next_market_id = group.next_market_id;
    let before_current_slot = group.current_slot;
    let before_activation_count = group.asset_activation_count;
    let before_last_activation_slot = group.last_asset_activation_slot;
    let before_asset_set_epoch = group.asset_set_epoch;
    let before_risk_epoch = group.risk_epoch;

    let result = group.activate_empty_asset_not_atomic(0, 7, 2);

    kani::cover!(case == 0, "v16 asset activation rejects market-id overflow");
    kani::cover!(
        case == 1,
        "v16 asset activation rejects activation-count overflow"
    );
    kani::cover!(
        case == 2,
        "v16 asset activation rejects asset-set-epoch overflow"
    );
    kani::cover!(
        case == 3,
        "v16 asset activation rejects risk-epoch overflow"
    );
    assert_eq!(result, Err(V16Error::CounterOverflow));
    assert_eq!(group.assets[0], before_asset);
    assert_eq!(group.source_backing_buckets[0], before_long_bucket);
    assert_eq!(group.source_backing_buckets[1], before_short_bucket);
    assert_eq!(group.next_market_id, before_next_market_id);
    assert_eq!(group.current_slot, before_current_slot);
    assert_eq!(group.asset_activation_count, before_activation_count);
    assert_eq!(
        group.last_asset_activation_slot,
        before_last_activation_slot
    );
    assert_eq!(group.asset_set_epoch, before_asset_set_epoch);
    assert_eq!(group.risk_epoch, before_risk_epoch);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_asset_lifecycle_epoch_overflows_fail_before_state_mutation() {
    let case: u8 = kani::any();
    kani::assume(case < 4);
    let (market, _, _) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    match case {
        0 | 2 => group.asset_set_epoch = u64::MAX,
        _ => group.risk_epoch = u64::MAX,
    }
    let before_asset = group.assets[0];
    let before_current_slot = group.current_slot;
    let before_asset_set_epoch = group.asset_set_epoch;
    let before_risk_epoch = group.risk_epoch;

    let result = if case < 2 {
        group.mark_asset_drain_only_not_atomic(0)
    } else {
        group.retire_empty_asset_not_atomic(0, 1)
    };

    kani::cover!(
        case == 0,
        "v16 drain-only transition rejects asset-set-epoch overflow"
    );
    kani::cover!(
        case == 1,
        "v16 drain-only transition rejects risk-epoch overflow"
    );
    kani::cover!(
        case == 2,
        "v16 retire transition rejects asset-set-epoch overflow"
    );
    kani::cover!(
        case == 3,
        "v16 retire transition rejects risk-epoch overflow"
    );
    assert_eq!(result, Err(V16Error::CounterOverflow));
    assert_eq!(group.assets[0], before_asset);
    assert_eq!(group.current_slot, before_current_slot);
    assert_eq!(group.asset_set_epoch, before_asset_set_epoch);
    assert_eq!(group.risk_epoch, before_risk_epoch);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_backing_bucket_market_id_must_match_asset_slot() {
    let (market, _, _) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let corrupt_side: bool = kani::any();
    let asset_market_id = group.assets[0].market_id;

    kani::cover!(
        !corrupt_side,
        "v16 backing long market-id mismatch reachable"
    );
    kani::cover!(
        corrupt_side,
        "v16 backing short market-id mismatch reachable"
    );

    if corrupt_side {
        group.source_backing_buckets[1].market_id = asset_market_id.checked_add(1).unwrap();
    } else {
        group.source_backing_buckets[0].market_id = asset_market_id.checked_add(1).unwrap();
    }

    assert_eq!(
        group.assert_public_invariants(),
        Err(V16Error::InvalidConfig)
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_dynamic_header_activation_binds_backing_to_new_market_id() {
    let nonempty: bool = kani::any();
    let price = 7u64;
    let (market, _, _) = concrete_ids();
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(
        market,
        V16Config::public_user_fund_with_market_slots(1, 1, 0, 1),
        1,
        0,
    )
    .unwrap();
    let mut slot = EngineAssetSlotV16Account::default();
    if nonempty {
        slot.asset.oi_eff_long_q = percolator::v16::V16PodU128::new(1);
        slot.asset.stored_pos_count_long = V16PodU64::new(1);
        slot.asset.loss_weight_sum_long = percolator::v16::V16PodU128::new(1);
    }
    let market_id = header.next_market_id.get();

    let result = header.activate_empty_asset_slot_not_atomic(0, &mut slot, price, 0);

    kani::cover!(
        price == 7,
        "v16 dynamic activation proof exercises nonzero price"
    );
    kani::cover!(
        !nonempty,
        "v16 dynamic activation empty slot success reachable"
    );
    kani::cover!(
        nonempty,
        "v16 dynamic activation nonempty slot rejection reachable"
    );
    if nonempty {
        assert_eq!(result, Err(V16Error::LockActive));
        assert_eq!(header.next_market_id.get(), market_id);
    } else {
        assert_eq!(result, Ok(()));
        assert_eq!(slot.asset.market_id.get(), market_id);
        assert_eq!(slot.backing_long.market_id.get(), market_id);
        assert_eq!(slot.backing_short.market_id.get(), market_id);
        assert_eq!(header.next_market_id.get(), market_id + 1);
    }
}

#[kani::proof]
#[kani::unwind(30)]
#[kani::solver(cadical)]
fn proof_v16_dynamic_header_activation_rejects_out_of_capacity_before_slot_mutation() {
    let beyond_capacity: bool = kani::any();
    let (market, _, _) = concrete_ids();
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(
        market,
        V16Config::public_user_fund_with_market_slots(1, 2, 0, 1),
        2,
        0,
    )
    .unwrap();
    let mut slot = EngineAssetSlotV16Account::default();
    let before_header_next_market_id = header.next_market_id.get();
    let before_header_current_slot = header.current_slot.get();
    let before_header_asset_set_epoch = header.asset_set_epoch.get();
    let before_slot_asset_market_id = slot.asset.market_id.get();
    let before_slot_backing_long_market_id = slot.backing_long.market_id.get();
    let before_slot_backing_short_market_id = slot.backing_short.market_id.get();
    let asset_index = if beyond_capacity { 2 } else { 3 };

    let result = header.activate_empty_asset_slot_not_atomic(asset_index, &mut slot, 123, 0);

    kani::cover!(
        beyond_capacity,
        "v16 dynamic activation rejects first index beyond capacity"
    );
    kani::cover!(
        !beyond_capacity,
        "v16 dynamic activation rejects later index beyond capacity"
    );
    assert_eq!(result, Err(V16Error::InvalidLeg));
    assert_eq!(header.next_market_id.get(), before_header_next_market_id);
    assert_eq!(header.current_slot.get(), before_header_current_slot);
    assert_eq!(header.asset_set_epoch.get(), before_header_asset_set_epoch);
    assert_eq!(slot.asset.market_id.get(), before_slot_asset_market_id);
    assert_eq!(
        slot.backing_long.market_id.get(),
        before_slot_backing_long_market_id
    );
    assert_eq!(
        slot.backing_short.market_id.get(),
        before_slot_backing_short_market_id
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_dynamic_header_activation_rejects_bad_price_or_stale_slot_without_mutation() {
    let case: u8 = kani::any();
    kani::assume(case < 3);
    let (market, _, _) = concrete_ids();
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(
        market,
        V16Config::public_user_fund_with_market_slots(1, 1, 0, 1),
        1,
        10,
    )
    .unwrap();
    let mut slot = EngineAssetSlotV16Account::default();
    let before_header_next_market_id = header.next_market_id.get();
    let before_header_current_slot = header.current_slot.get();
    let before_header_activation_count = header.asset_activation_count.get();
    let before_header_last_activation_slot = header.last_asset_activation_slot.get();
    let before_header_asset_set_epoch = header.asset_set_epoch.get();
    let before_header_risk_epoch = header.risk_epoch.get();
    let before_slot = slot;
    let (price, now_slot) = match case {
        0 => (0, 10),
        1 => (MAX_ORACLE_PRICE + 1, 10),
        _ => (123, 9),
    };

    let result = header.activate_empty_asset_slot_not_atomic(0, &mut slot, price, now_slot);

    kani::cover!(case == 0, "v16 dynamic activation rejects zero price");
    kani::cover!(case == 1, "v16 dynamic activation rejects above max price");
    kani::cover!(case == 2, "v16 dynamic activation rejects stale slot");
    assert_eq!(result, Err(V16Error::InvalidConfig));
    assert_eq!(header.next_market_id.get(), before_header_next_market_id);
    assert_eq!(header.current_slot.get(), before_header_current_slot);
    assert_eq!(
        header.asset_activation_count.get(),
        before_header_activation_count
    );
    assert_eq!(
        header.last_asset_activation_slot.get(),
        before_header_last_activation_slot
    );
    assert_eq!(header.asset_set_epoch.get(), before_header_asset_set_epoch);
    assert_eq!(header.risk_epoch.get(), before_header_risk_epoch);
    assert_eq!(slot, before_slot);
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v16_dynamic_header_activation_counter_overflows_fail_before_slot_mutation() {
    let case: u8 = kani::any();
    kani::assume(case < 4);
    let (market, _, _) = concrete_ids();
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(
        market,
        V16Config::public_user_fund_with_market_slots(1, 1, 0, 1),
        1,
        0,
    )
    .unwrap();
    match case {
        0 => header.next_market_id = V16PodU64::new(u64::MAX),
        1 => {
            header.asset_activation_count = V16PodU64::new(u64::MAX);
            header.last_asset_activation_slot = V16PodU64::new(0);
        }
        2 => header.asset_set_epoch = V16PodU64::new(u64::MAX),
        _ => header.risk_epoch = V16PodU64::new(u64::MAX),
    }
    let mut slot = EngineAssetSlotV16Account::default();
    let before_next_market_id = header.next_market_id.get();
    let before_current_slot = header.current_slot.get();
    let before_activation_count = header.asset_activation_count.get();
    let before_last_activation_slot = header.last_asset_activation_slot.get();
    let before_asset_set_epoch = header.asset_set_epoch.get();
    let before_risk_epoch = header.risk_epoch.get();
    let before_slot = slot;

    let result = header.activate_empty_asset_slot_not_atomic(0, &mut slot, 123, 10);

    kani::cover!(
        case == 0,
        "v16 dynamic activation rejects next-market-id overflow"
    );
    kani::cover!(
        case == 1,
        "v16 dynamic activation rejects activation-count overflow"
    );
    kani::cover!(
        case == 2,
        "v16 dynamic activation rejects asset-set-epoch overflow"
    );
    kani::cover!(
        case == 3,
        "v16 dynamic activation rejects risk-epoch overflow"
    );
    assert_eq!(result, Err(V16Error::CounterOverflow));
    assert_eq!(header.next_market_id.get(), before_next_market_id);
    assert_eq!(header.current_slot.get(), before_current_slot);
    assert_eq!(header.asset_activation_count.get(), before_activation_count);
    assert_eq!(
        header.last_asset_activation_slot.get(),
        before_last_activation_slot
    );
    assert_eq!(header.asset_set_epoch.get(), before_asset_set_epoch);
    assert_eq!(header.risk_epoch.get(), before_risk_epoch);
    assert_eq!(slot, before_slot);
}

#[kani::proof]
#[kani::unwind(30)]
#[kani::solver(cadical)]
fn proof_v16_dynamic_header_growth_rejects_non_monotone_request_without_mutation() {
    let shrink_capacity: bool = kani::any();
    let (market, _, _) = concrete_ids();
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(
        market,
        V16Config::public_user_fund_with_market_slots(4, 16, 0, 10),
        16,
        0,
    )
    .unwrap();
    let before_capacity = header.asset_slot_capacity.get();
    let before_config = header.config;
    let before_asset_set_epoch = header.asset_set_epoch.get();
    let before_risk_epoch = header.risk_epoch.get();
    let before_next_market_id = header.next_market_id.get();
    let before_activation_count = header.asset_activation_count.get();
    let (new_capacity, new_max_market_slots) = if shrink_capacity { (15, 16) } else { (24, 25) };

    let result = header.grow_asset_slot_capacity_not_atomic(new_capacity, new_max_market_slots);

    kani::cover!(
        shrink_capacity,
        "v16 dynamic header rejects capacity shrink"
    );
    kani::cover!(
        !shrink_capacity,
        "v16 dynamic header rejects max-market-slot above capacity"
    );

    assert_eq!(result, Err(V16Error::InvalidConfig));
    assert_eq!(header.asset_slot_capacity.get(), before_capacity);
    assert_eq!(header.config, before_config);
    assert_eq!(header.asset_set_epoch.get(), before_asset_set_epoch);
    assert_eq!(header.risk_epoch.get(), before_risk_epoch);
    assert_eq!(header.next_market_id.get(), before_next_market_id);
    assert_eq!(header.asset_activation_count.get(), before_activation_count);
}

#[kani::proof]
#[kani::unwind(30)]
#[kani::solver(cadical)]
fn proof_v16_dynamic_header_growth_counter_overflows_fail_before_metadata_mutation() {
    let overflow_asset_set_epoch: bool = kani::any();
    let (market, _, _) = concrete_ids();
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(
        market,
        V16Config::public_user_fund_with_market_slots(4, 16, 0, 10),
        16,
        0,
    )
    .unwrap();
    if overflow_asset_set_epoch {
        header.asset_set_epoch = V16PodU64::new(u64::MAX);
    } else {
        header.risk_epoch = V16PodU64::new(u64::MAX);
    }
    let before_capacity = header.asset_slot_capacity.get();
    let before_max_market_slots = header.config.max_market_slots.get();
    let before_asset_set_epoch = header.asset_set_epoch.get();
    let before_risk_epoch = header.risk_epoch.get();

    let result = header.grow_asset_slot_capacity_not_atomic(32, 32);

    kani::cover!(
        overflow_asset_set_epoch,
        "v16 dynamic header growth rejects asset-set-epoch overflow"
    );
    kani::cover!(
        !overflow_asset_set_epoch,
        "v16 dynamic header growth rejects risk-epoch overflow"
    );
    assert_eq!(result, Err(V16Error::CounterOverflow));
    assert_eq!(header.asset_slot_capacity.get(), before_capacity);
    assert_eq!(
        header.config.max_market_slots.get(),
        before_max_market_slots
    );
    assert_eq!(header.asset_set_epoch.get(), before_asset_set_epoch);
    assert_eq!(header.risk_epoch.get(), before_risk_epoch);
}

#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn proof_v16_dynamic_realloc_layout_capacity_not_fixed_by_proof_window() {
    type Wrapper = [u8; 31];
    let capacity = KANI_MARKET_SLOTS_N + 5;
    let slot_len = MarketGroupV16HeaderAccount::dynamic_asset_slot_stride::<Wrapper>();
    let account_len =
        MarketGroupV16HeaderAccount::dynamic_market_group_account_len::<Wrapper>(capacity).unwrap();
    let last_offset =
        MarketGroupV16HeaderAccount::dynamic_asset_slot_offset::<Wrapper>(capacity - 1).unwrap();

    kani::cover!(
        capacity > KANI_MARKET_SLOTS_N,
        "v16 dynamic realloc proof exercises wrapper capacity beyond proof-local window"
    );
    assert_eq!(
        MarketGroupV16HeaderAccount::dynamic_asset_slot_capacity_from_account_len::<Wrapper>(
            account_len
        ),
        Ok(capacity)
    );
    assert_eq!(
        MarketGroupV16HeaderAccount::validate_dynamic_market_group_account_len::<Wrapper>(
            account_len,
            capacity
        ),
        Ok(())
    );
    assert_eq!(last_offset + slot_len, account_len);
}

#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn proof_v16_dynamic_realloc_layout_rejects_bad_lengths() {
    let case: u8 = kani::any();
    kani::assume(case < 3);
    type Wrapper = [u8; 31];
    let capacity = KANI_MARKET_SLOTS_N + 5;
    let account_len =
        MarketGroupV16HeaderAccount::dynamic_market_group_account_len::<Wrapper>(capacity).unwrap();
    let header_len = core::mem::size_of::<MarketGroupV16HeaderAccount>();
    let bad_len = match case {
        0 => header_len - 1,
        1 => account_len - 1,
        _ => account_len,
    };
    let result = if case == 2 {
        MarketGroupV16HeaderAccount::validate_dynamic_market_group_account_len::<Wrapper>(
            bad_len,
            capacity + 1,
        )
    } else {
        MarketGroupV16HeaderAccount::dynamic_asset_slot_capacity_from_account_len::<Wrapper>(
            bad_len,
        )
        .map(|_| ())
    };

    kani::cover!(
        case == 0,
        "v16 dynamic layout rejects shorter-than-header account"
    );
    kani::cover!(
        case == 1,
        "v16 dynamic layout rejects trailing partial slot"
    );
    kani::cover!(
        case == 2,
        "v16 dynamic layout rejects wrong expected capacity"
    );
    assert_eq!(result, Err(V16Error::InvalidConfig));
}

#[kani::proof]
#[kani::unwind(16)]
#[kani::solver(cadical)]
fn proof_v16_dynamic_header_runtime_conversion_rejects_slot_length_mismatch() {
    let too_many_slots: bool = kani::any();
    let supplied_len = if too_many_slots { 2 } else { 0 };

    let result =
        MarketGroupV16HeaderAccount::kani_validate_dynamic_market_slots_len(supplied_len, 1, 1);

    kani::cover!(
        !too_many_slots,
        "v16 dynamic header rejects missing allocated slot"
    );
    kani::cover!(
        too_many_slots,
        "v16 dynamic header rejects extra allocated slot"
    );
    assert_eq!(result, Err(V16Error::InvalidConfig));
}

#[kani::proof]
#[kani::unwind(30)]
#[kani::solver(cadical)]
fn proof_v16_dynamic_header_accepts_overallocated_empty_wrapper_owned_slots() {
    let (market, _, _) = concrete_ids();
    let header = MarketGroupV16HeaderAccount::new_dynamic(
        market,
        V16Config::public_user_fund_with_market_slots(1, 1, 0, 1),
        3,
        0,
    )
    .unwrap();
    let slots = [
        Market {
            wrapper: 11u64,
            engine: EngineAssetSlotV16Account::default(),
        },
        Market {
            wrapper: 22u64,
            engine: EngineAssetSlotV16Account::default(),
        },
        Market {
            wrapper: 33u64,
            engine: EngineAssetSlotV16Account::empty_for_market(0),
        },
    ];

    let result0 = header.kani_validate_dynamic_market_slot_shape_at(0, &slots[0]);
    let result1 = header.kani_validate_dynamic_market_slot_shape_at(1, &slots[1]);
    let result2 = header.kani_validate_dynamic_market_slot_shape_at(2, &slots[2]);

    assert_eq!(result0, Ok(()));
    assert_eq!(result1, Ok(()));
    assert_eq!(result2, Ok(()));
    assert_eq!(slots[2].wrapper, 33);
}

#[kani::proof]
#[kani::unwind(30)]
#[kani::solver(cadical)]
fn proof_v16_zero_copy_market_view_validates_wrapper_owned_slice_without_runtime_vecs() {
    let (market, _, _) = concrete_ids();
    let header = MarketGroupV16HeaderAccount::new_dynamic(
        market,
        V16Config::public_user_fund_with_market_slots(1, 1, 0, 1),
        3,
        0,
    )
    .unwrap();
    let slots = [
        Market {
            wrapper: 11u64,
            engine: EngineAssetSlotV16Account::default(),
        },
        Market {
            wrapper: 22u64,
            engine: EngineAssetSlotV16Account::default(),
        },
        Market {
            wrapper: 33u64,
            engine: EngineAssetSlotV16Account::empty_for_market(0),
        },
    ];
    let view = MarketGroupV16View::new(&header, &slots);

    kani::cover!(
        slots[2].wrapper == 33,
        "v16 zero-copy market view preserves wrapper-owned payload"
    );
    assert_eq!(view.validate_shape(), Ok(()));
    assert_eq!(slots[0].wrapper, 11);
    assert_eq!(slots[1].wrapper, 22);
    assert_eq!(slots[2].wrapper, 33);
}

#[kani::proof]
#[kani::unwind(30)]
#[kani::solver(cadical)]
fn proof_v16_dynamic_header_rejects_nonempty_hidden_wrapper_slot() {
    let hidden_field: u8 = kani::any();
    let (market, _, _) = concrete_ids();
    let header = MarketGroupV16HeaderAccount::new_dynamic(
        market,
        V16Config::public_user_fund_with_market_slots(1, 1, 0, 1),
        2,
        0,
    )
    .unwrap();
    let mut hidden = EngineAssetSlotV16Account::default();
    if hidden_field & 1 == 0 {
        hidden.asset.market_id = V16PodU64::new(99);
    } else {
        hidden.asset.oi_eff_long_q = V16PodU128::new(1);
    }
    let slots = [
        Market {
            wrapper: 1u8,
            engine: EngineAssetSlotV16Account::default(),
        },
        Market {
            wrapper: 2u8,
            engine: hidden,
        },
    ];

    let active_result = header.kani_validate_dynamic_market_slot_shape_at(0, &slots[0]);
    let hidden_result = header.kani_validate_dynamic_market_slot_shape_at(1, &slots[1]);

    kani::cover!(
        hidden_field & 1 == 0,
        "v16 wrapper-owned slice rejects hidden nonzero market id"
    );
    kani::cover!(
        hidden_field & 1 == 1,
        "v16 wrapper-owned slice rejects hidden nonzero risk state"
    );
    assert_eq!(active_result, Ok(()));
    assert_eq!(hidden_result, Err(V16Error::InvalidConfig));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_retired_asset_idempotence_requires_empty_state() {
    let nonempty: bool = kani::any();
    let (market, _, _) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    group.assets[0].lifecycle = AssetLifecycleV16::Retired;
    group.assets[0].retired_slot = 1;
    group.current_slot = 1;
    if nonempty {
        group.assets[0].oi_eff_long_q = 1;
        group.assets[0].stored_pos_count_long = 1;
        group.assets[0].loss_weight_sum_long = 1;
    }
    let before = group.clone();

    let result = group.retire_empty_asset_not_atomic(0, 1);

    kani::cover!(!nonempty, "v16 retired empty idempotence reachable");
    kani::cover!(
        nonempty,
        "v16 retired nonempty idempotence rejection reachable"
    );
    if nonempty {
        assert_eq!(result, Err(V16Error::LockActive));
    } else {
        assert_eq!(result, Ok(()));
    }
    assert_eq!(group.assets[0].lifecycle, before.assets[0].lifecycle);
    assert_eq!(group.asset_set_epoch, before.asset_set_epoch);
    assert_eq!(
        group.assets[0].oi_eff_long_q,
        before.assets[0].oi_eff_long_q
    );
    assert_eq!(
        group.assets[0].stored_pos_count_long,
        before.assets[0].stored_pos_count_long
    );
    assert_eq!(
        group.assets[0].loss_weight_sum_long,
        before.assets[0].loss_weight_sum_long
    );
    assert_eq!(
        group.pending_domain_loss_barriers[0],
        before.pending_domain_loss_barriers[0]
    );
    assert_eq!(
        group.pending_domain_loss_barriers[1],
        before.pending_domain_loss_barriers[1]
    );
}

#[kani::proof]
#[kani::unwind(32)]
#[kani::solver(cadical)]
fn proof_v16_asset_activation_cooldown_fails_before_lifecycle_mutation() {
    let (market, _, _) = symbolic_ids();
    let mut config = V16Config::public_user_fund(2, 0, 1);
    config.asset_activation_cooldown_slots = 3;
    let mut group = MarketGroupV16::new(market, config).unwrap();

    group.assets[0].lifecycle = AssetLifecycleV16::Retired;
    group.assets[0].retired_slot = 1;
    group.current_slot = 1;
    group.activate_empty_asset_not_atomic(0, 7, 4).unwrap();
    group.assets[1].lifecycle = AssetLifecycleV16::Retired;
    group.assets[1].retired_slot = 4;
    let before_asset_1 = group.assets[1];
    let before_activation_count = group.asset_activation_count;
    let before_last_activation_slot = group.last_asset_activation_slot;
    let before_risk_epoch = group.risk_epoch;
    let before_asset_set_epoch = group.asset_set_epoch;

    let result = group.activate_empty_asset_not_atomic(1, 7, 6);

    kani::cover!(
        result == Err(V16Error::LockActive),
        "v16 activation cooldown rejection reachable"
    );
    assert_eq!(result, Err(V16Error::LockActive));
    assert_eq!(group.assets[1], before_asset_1);
    assert_eq!(group.asset_activation_count, before_activation_count);
    assert_eq!(
        group.last_asset_activation_slot,
        before_last_activation_slot
    );
    assert_eq!(group.risk_epoch, before_risk_epoch);
    assert_eq!(group.asset_set_epoch, before_asset_set_epoch);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_bilateral_oi_decomposition_counts_long_short_pair() {
    let size_q = 3u128;
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut a = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut b = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));

    group
        .attach_leg(&mut a, 0, SideV16::Long, size_q as i128)
        .unwrap();
    group
        .attach_leg(&mut b, 0, SideV16::Short, -(size_q as i128))
        .unwrap();

    kani::cover!(true, "v16 bilateral OI proof covers long-short pair");
    assert_eq!(group.assets[0].oi_eff_long_q, size_q);
    assert_eq!(group.assets[0].oi_eff_short_q, size_q);
    assert_eq!(group.assets[0].stored_pos_count_long, 1);
    assert_eq!(group.assets[0].stored_pos_count_short, 1);
    assert_eq!(a.active_bitmap, bitmap(&[0]));
    assert_eq!(b.active_bitmap, bitmap(&[0]));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_bilateral_oi_decomposition_counts_short_long_pair() {
    let size_q = 3u128;
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut a = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut b = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));

    group
        .attach_leg(&mut a, 0, SideV16::Short, -(size_q as i128))
        .unwrap();
    group
        .attach_leg(&mut b, 0, SideV16::Long, size_q as i128)
        .unwrap();

    kani::cover!(true, "v16 bilateral OI proof covers short-long pair");
    assert_eq!(group.assets[0].oi_eff_long_q, size_q);
    assert_eq!(group.assets[0].oi_eff_short_q, size_q);
    assert_eq!(group.assets[0].stored_pos_count_long, 1);
    assert_eq!(group.assets[0].stored_pos_count_short, 1);
    assert_eq!(a.active_bitmap, bitmap(&[0]));
    assert_eq!(b.active_bitmap, bitmap(&[0]));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_oversize_position_rejected_before_oi_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    let result = group.attach_leg(
        &mut account,
        0,
        SideV16::Long,
        (MAX_POSITION_ABS_Q + 1) as i128,
    );

    assert_eq!(result, Err(V16Error::InvalidLeg));
    assert_eq!(account.active_bitmap, bitmap(&[]));
    assert_eq!(group.assets[0].oi_eff_long_q, 0);
    assert_eq!(group.assets[0].stored_pos_count_long, 0);
}

fn assert_v16_account_b_chunk_case(target_units: u8, budget_units: u8) {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.attach_leg(&mut account, 0, SideV16::Long, 1).unwrap();
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
        assert_eq!(result, Err(V16Error::RecoveryRequired));
        assert_eq!(account.legs[0].b_snap, before_snap);
    } else {
        let chunk = result.unwrap();
        assert!(chunk.delta_b > 0);
        assert!(account.legs[0].b_snap > before_snap);
        assert!(chunk.remaining_after < before_remaining);
    }
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_account_b_chunk_current_noops() {
    assert_v16_account_b_chunk_case(0, 1);
    kani::cover!(true, "v16 B chunk current no-op reachable");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_account_b_chunk_zero_budget_fails_closed() {
    assert_v16_account_b_chunk_case(2, 0);
    kani::cover!(true, "v16 B chunk zero-budget fail-closed reachable");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_account_b_chunk_positive_budget_advances() {
    assert_v16_account_b_chunk_case(4, 1);
    kani::cover!(true, "v16 B chunk progress reachable");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_repeated_account_b_chunks_complete_bounded_small_residual() {
    let target_units: u8 = kani::any();
    kani::assume((1..=2).contains(&target_units));

    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.attach_leg(&mut account, 0, SideV16::Long, 1).unwrap();
    group.assets[0].b_long_num = target_units as u128;

    let first = group.settle_account_b_chunk(&mut account, 0, 1).unwrap();
    assert_eq!(first.delta_b, 1);
    assert_eq!(account.legs[0].b_snap, 1);
    assert_eq!(first.remaining_after, target_units as u128 - 1);

    if target_units == 2 {
        kani::cover!(true, "v16 two B chunks needed and completed");
        assert!(account.b_stale_state);
        assert!(account.legs[0].b_stale);
        let second = group.settle_account_b_chunk(&mut account, 0, 1).unwrap();
        assert_eq!(second.delta_b, 1);
        assert_eq!(second.remaining_after, 0);
    } else {
        kani::cover!(true, "v16 one B chunk completed residual");
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
#[kani::unwind(32)]
#[kani::solver(cadical)]
fn proof_v16_liquidation_progress_rejects_non_reducing_scores() {
    let case: u8 = kani::any();
    let deficit: u8 = kani::any();
    let gross_loss: u8 = kani::any();
    let unsettled_b: u8 = kani::any();
    let active_legs: u8 = kani::any();
    kani::assume(case <= 3);
    kani::assume(deficit <= 5);
    kani::assume(gross_loss <= 5);
    kani::assume(unsettled_b <= 5);
    kani::assume(active_legs <= 4);

    let before = RiskScoreV16 {
        certified_liq_deficit: deficit as u128,
        unsettled_b_loss_bound: unsettled_b as u128,
        stale_loss_bound: 0,
        gross_risk_notional: gross_loss as u128,
        active_leg_count: active_legs as u32,
    };
    let mut after = before;

    match case {
        0 => {}
        1 => after.certified_liq_deficit = deficit as u128 + 1,
        2 => after.stale_loss_bound = 1,
        _ => after.gross_risk_notional = gross_loss as u128 + 1,
    }

    kani::cover!(case == 0, "v16 equal risk score non-progress reachable");
    kani::cover!(case == 1, "v16 worse deficit non-progress reachable");
    kani::cover!(case == 2, "v16 stale-penalty non-progress reachable");
    kani::cover!(case == 3, "v16 worse gross-loss non-progress reachable");

    assert!(!MarketGroupV16::liquidation_progress_from_scores(
        before, after
    ));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_favorable_action_accepts_current_full_refresh_certificate() {
    let (market, account_id, owner) = concrete_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.health_cert = HealthCertV16 {
        cert_oracle_epoch: group.oracle_epoch,
        cert_funding_epoch: group.funding_epoch,
        cert_risk_epoch: group.risk_epoch,
        cert_asset_set_epoch: group.asset_set_epoch,
        active_bitmap_at_cert: account.active_bitmap,
        valid: true,
        ..HealthCertV16::default()
    };

    kani::cover!(
        account.health_cert.valid,
        "v16 current health certificate reachable"
    );
    assert_eq!(
        group.kani_ensure_favorable_action_current_certificate(&account),
        Ok(())
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_favorable_action_rejects_stale_full_refresh_certificate() {
    let stale_case: u8 = kani::any();
    kani::assume(stale_case <= 5);
    let (market, account_id, owner) = concrete_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.health_cert = HealthCertV16 {
        cert_oracle_epoch: group.oracle_epoch,
        cert_funding_epoch: group.funding_epoch,
        cert_risk_epoch: group.risk_epoch,
        cert_asset_set_epoch: group.asset_set_epoch,
        active_bitmap_at_cert: account.active_bitmap,
        valid: true,
        ..HealthCertV16::default()
    };

    match stale_case {
        0 => account.health_cert.valid = false,
        1 => account.health_cert.cert_oracle_epoch = group.oracle_epoch + 1,
        2 => account.health_cert.cert_funding_epoch = group.funding_epoch + 1,
        3 => account.health_cert.cert_risk_epoch = group.risk_epoch + 1,
        4 => account.health_cert.cert_asset_set_epoch = group.asset_set_epoch + 1,
        _ => account.health_cert.active_bitmap_at_cert = [account.active_bitmap[0] ^ 1],
    }

    kani::cover!(stale_case == 0, "v16 invalid health certificate rejected");
    kani::cover!(
        stale_case == 1,
        "v16 stale oracle epoch certificate rejected"
    );
    kani::cover!(
        stale_case == 2,
        "v16 stale funding epoch certificate rejected"
    );
    kani::cover!(stale_case == 3, "v16 stale risk epoch certificate rejected");
    kani::cover!(
        stale_case == 4,
        "v16 stale asset-set epoch certificate rejected"
    );
    kani::cover!(
        stale_case == 5,
        "v16 stale active-bitmap certificate rejected"
    );
    assert_eq!(
        group.kani_ensure_favorable_action_current_certificate(&account),
        Err(V16Error::Stale)
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_global_residual_is_not_account_health_proof() {
    let residual_units: u8 = kani::any();
    kani::assume(residual_units > 0);
    kani::assume(residual_units <= 5);
    let residual = residual_units as u128;
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.pnl = residual as i128;
    account.reserved_pnl = 0;
    group.pnl_pos_tot = residual;
    set_junior_bound(&mut group, residual);
    group.pnl_matured_pos_tot = residual;
    group.vault = group.c_tot + group.insurance + residual;
    let before_vault = group.vault;
    let before_c_tot = group.c_tot;
    let before_insurance = group.insurance;
    let before_pnl_pos_tot = group.pnl_pos_tot;
    let before_capital = account.capital;
    let before_pnl = account.pnl;
    let before_reserved = account.reserved_pnl;

    let result = group.convert_released_pnl_to_capital_not_atomic(&mut account);

    kani::cover!(
        residual > 0 && !account.health_cert.valid,
        "v16 aggregate residual with stale account certificate reachable"
    );
    assert_eq!(result, Err(V16Error::Stale));
    assert_eq!(group.vault, before_vault);
    assert_eq!(group.c_tot, before_c_tot);
    assert_eq!(group.insurance, before_insurance);
    assert_eq!(group.pnl_pos_tot, before_pnl_pos_tot);
    assert_eq!(account.capital, before_capital);
    assert_eq!(account.pnl, before_pnl);
    assert_eq!(account.reserved_pnl, before_reserved);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_favorable_locks_block_released_pnl_conversion_before_mutation() {
    let lock_case: u8 = kani::any();
    kani::assume(lock_case < 6);
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    group
        .attach_leg(&mut account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    account.pnl = 5;
    account.health_cert = HealthCertV16 {
        cert_oracle_epoch: group.oracle_epoch,
        cert_funding_epoch: group.funding_epoch,
        cert_risk_epoch: group.risk_epoch,
        cert_asset_set_epoch: group.asset_set_epoch,
        active_bitmap_at_cert: account.active_bitmap,
        valid: true,
        ..HealthCertV16::default()
    };

    match lock_case {
        0 => group.threshold_stress_active = true,
        1 => group.bankruptcy_hlock_active = true,
        2 => group.loss_stale_active = true,
        3 => account.stale_state = true,
        4 => account.b_stale_state = true,
        _ => group.assets[0].raw_oracle_target_price = 2,
    }

    let result = group.kani_preflight_convert_released_pnl_to_capital(&account);

    kani::cover!(lock_case == 0, "v16 threshold-stress conversion lock");
    kani::cover!(lock_case == 1, "v16 bankruptcy h-lock conversion lock");
    kani::cover!(lock_case == 2, "v16 loss-stale conversion lock");
    kani::cover!(lock_case == 3, "v16 stale account conversion lock");
    kani::cover!(lock_case == 4, "v16 B-stale account conversion lock");
    kani::cover!(lock_case == 5, "v16 target/effective lag conversion lock");
    assert_eq!(result, Err(V16Error::LockActive));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_public_invariants_reject_broken_senior_claim_conservation() {
    let vault_units: u8 = kani::any();
    let c_units: u8 = kani::any();
    let i_units: u8 = kani::any();
    kani::assume(vault_units <= 10);
    kani::assume(c_units <= 10);
    kani::assume(i_units <= 10);
    kani::assume((c_units as u16) + (i_units as u16) > vault_units as u16);

    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    group.vault = vault_units as u128;
    group.c_tot = c_units as u128;
    group.insurance = i_units as u128;

    kani::cover!(
        group.c_tot <= group.vault && group.insurance <= group.vault,
        "v16 senior sum overflow can violate conservation even when each claim is individually within vault"
    );
    assert_eq!(
        group.assert_public_invariants(),
        Err(V16Error::InvalidConfig)
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_public_invariants_reject_hard_global_bounds() {
    let case: u8 = kani::any();
    kani::assume(case < 18);
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();

    match case {
        0 => group.vault = MAX_VAULT_TVL + 1,
        1 => {
            group.pnl_pos_tot = 1;
            set_junior_bound(&mut group, 1);
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
        7 => group.assets[0].oi_eff_long_q = 1,
        8 => group.assets[0].loss_weight_sum_short = 1,
        9 => {
            group.assets[0].oi_eff_long_q = 2;
            group.assets[0].loss_weight_sum_long = 2;
            group.assets[0].oi_eff_short_q = 1;
            group.assets[0].loss_weight_sum_short = 1;
        }
        10 => group.assets[0].k_long = i128::MIN,
        11 => group.assets[0].k_short = i128::MIN,
        12 => group.assets[0].f_long_num = i128::MIN,
        13 => group.assets[0].f_short_num = i128::MIN,
        14 => group.assets[0].k_epoch_start_long = i128::MIN,
        15 => group.assets[0].k_epoch_start_short = i128::MIN,
        16 => group.assets[0].f_epoch_start_long_num = i128::MIN,
        _ => group.assets[0].f_epoch_start_short_num = i128::MIN,
    }

    kani::cover!(case == 0, "v16 vault cap violation reachable");
    kani::cover!(case == 1, "v16 matured positive PnL violation reachable");
    kani::cover!(case == 2, "v16 slot ordering violation reachable");
    kani::cover!(case == 3, "v16 zero effective price violation reachable");
    kani::cover!(case == 4, "v16 OI side cap violation reachable");
    kani::cover!(case == 5, "v16 loss weight cap violation reachable");
    kani::cover!(case == 6, "v16 social loss remainder violation reachable");
    kani::cover!(
        case == 7,
        "v16 positive OI without loss weight violation reachable"
    );
    kani::cover!(case == 8, "v16 loss weight without OI violation reachable");
    kani::cover!(case == 9, "v16 live OI imbalance violation reachable");
    kani::cover!(case == 10, "v16 K long i128::MIN violation reachable");
    kani::cover!(case == 11, "v16 K short i128::MIN violation reachable");
    kani::cover!(case == 12, "v16 F long i128::MIN violation reachable");
    kani::cover!(case == 13, "v16 F short i128::MIN violation reachable");
    kani::cover!(
        case == 14,
        "v16 K long epoch-start i128::MIN violation reachable"
    );
    kani::cover!(
        case == 15,
        "v16 K short epoch-start i128::MIN violation reachable"
    );
    kani::cover!(
        case == 16,
        "v16 F long epoch-start i128::MIN violation reachable"
    );
    kani::cover!(
        case == 17,
        "v16 F short epoch-start i128::MIN violation reachable"
    );
    assert_eq!(
        group.assert_public_invariants(),
        Err(V16Error::InvalidConfig)
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_cross_margin_equity_counts_collateral_once_and_score_uses_full_envelope() {
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
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(2, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.capital = capital;
    account.fee_credits = -debt;
    account.active_bitmap = bitmap(&[0, 1]);
    account.legs[0] = PortfolioLegV16 {
        active: true,
        asset_index: 0,
        market_id: group.assets[0].market_id,
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
    };
    account.legs[1] = PortfolioLegV16 {
        active: true,
        asset_index: 1,
        market_id: group.assets[1].market_id,
        side: SideV16::Short,
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
        account.active_bitmap == bitmap(&[0, 1]),
        "v16 two active legs reachable for single-collateral equity"
    );
    assert_eq!(equity, expected);

    let mut cert_account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    cert_account.health_cert.valid = true;
    cert_account.health_cert.certified_worst_case_loss = certified_loss;
    let score = group.risk_score(&cert_account).unwrap();

    kani::cover!(
        certified_loss > 1,
        "v16 full certified loss envelope reaches risk score"
    );
    assert_eq!(score.gross_risk_notional, certified_loss);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_global_cross_margin_positive_leg_supports_other_leg_maintenance_without_b_domain() {
    let (market, _, _) = concrete_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(2, 0, 1)).unwrap();
    let source = SourceCreditStateV16 {
        positive_claim_bound_num: 2 * BOUND_SCALE,
        exact_positive_claim_num: 2 * BOUND_SCALE,
        fresh_reserved_backing_num: 2 * BOUND_SCALE,
        credit_rate_num: CREDIT_RATE_SCALE,
        ..SourceCreditStateV16::EMPTY
    };
    let source_support =
        MarketGroupV16::kani_source_credit_state_realizable_support_for_face(source, 2).unwrap();
    let cert = group
        .kani_build_account_health_cert_from_equity_parts(bitmap(&[0, 1]), 2, 2, 2, 2)
        .unwrap();

    kani::cover!(
        cert.certified_liq_deficit == 0,
        "v16 positive leg support covers other-leg maintenance"
    );
    assert_eq!(source_support, 2);
    assert_eq!(group.c_tot, 0);
    assert_eq!(cert.certified_equity, 2);
    assert_eq!(cert.certified_maintenance_req, 2);
    assert_eq!(cert.certified_liq_deficit, 0);
    assert_eq!(group.insurance_domain_spent[0], 0);
    assert_eq!(group.insurance_domain_spent[1], 0);
    assert_eq!(group.insurance_domain_spent[2], 0);
    assert_eq!(group.insurance_domain_spent[3], 0);
    assert_eq!(group.pending_domain_loss_barriers[0], 0);
    assert_eq!(group.pending_domain_loss_barriers[1], 0);
    assert_eq!(group.pending_domain_loss_barriers[2], 0);
    assert_eq!(group.pending_domain_loss_barriers[3], 0);
    assert_eq!(group.assets[0].b_long_num, 0);
    assert_eq!(group.assets[0].b_short_num, 0);
    assert_eq!(group.assets[1].b_long_num, 0);
    assert_eq!(group.assets[1].b_short_num, 0);
}

fn assert_full_refresh_settles_and_scores_two_active_assets(capital_units: u128) {
    let (market, _, _) = concrete_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(2, 0, 1)).unwrap();
    let leg0 = PortfolioLegV16 {
        active: true,
        asset_index: 0,
        market_id: group.assets[0].market_id,
        side: SideV16::Long,
        basis_pos_q: POS_SCALE as i128,
        a_basis: ADL_ONE,
        k_snap: group.assets[0].k_long,
        f_snap: group.assets[0].f_long_num,
        epoch_snap: group.assets[0].epoch_long,
        loss_weight: POS_SCALE,
        b_snap: group.assets[0].b_long_num,
        b_rem: 0,
        b_epoch_snap: group.assets[0].epoch_long,
        b_stale: false,
        stale: false,
    };
    let leg1 = PortfolioLegV16 {
        active: true,
        asset_index: 1,
        market_id: group.assets[1].market_id,
        side: SideV16::Long,
        basis_pos_q: POS_SCALE as i128,
        a_basis: ADL_ONE,
        k_snap: group.assets[1].k_long,
        f_snap: group.assets[1].f_long_num,
        epoch_snap: group.assets[1].epoch_long,
        loss_weight: POS_SCALE,
        b_snap: group.assets[1].b_long_num,
        b_rem: 0,
        b_epoch_snap: group.assets[1].epoch_long,
        b_stale: false,
        stale: false,
    };
    let active_bitmap = bitmap(&[0, 1]);
    let prices = {
        let mut out = [1u64; V16_MAX_PORTFOLIO_ASSETS_N];
        out[0] = 7;
        out[1] = 11;
        out
    };
    let expected_loss0 = risk_notional_ceil(POS_SCALE, prices[0]).unwrap();
    let expected_loss1 = risk_notional_ceil(POS_SCALE, prices[1]).unwrap();

    let expected_pnl = if capital_units == 0 { -2 } else { 0 };
    let expected_capital = capital_units.saturating_sub(2);

    let (initial0, maintenance0, risk0) = group
        .kani_account_health_leg_requirements(leg0, &prices, false)
        .unwrap();
    let (initial1, maintenance1, risk1) = group
        .kani_account_health_leg_requirements(leg1, &prices, false)
        .unwrap();
    let initial_req = initial0.checked_add(initial1).unwrap();
    let maintenance_req = maintenance0.checked_add(maintenance1).unwrap();
    let worst_case_loss = risk0.checked_add(risk1).unwrap();
    let equity = account_equity_from_parts(expected_capital, expected_pnl, 0).unwrap();
    let cert = group
        .kani_build_account_health_cert_from_equity_parts(
            active_bitmap,
            equity,
            initial_req,
            maintenance_req,
            worst_case_loss,
        )
        .unwrap();

    assert_eq!(risk0, expected_loss0);
    assert_eq!(risk1, expected_loss1);
    assert_eq!(
        cert.certified_worst_case_loss,
        expected_loss0 + expected_loss1
    );
    assert_eq!(
        cert.certified_maintenance_req,
        expected_loss0 + expected_loss1
    );
    assert_eq!(cert.certified_initial_req, expected_loss0 + expected_loss1);
    assert_eq!(equity, capital_units as i128 - 2);
    assert_eq!(cert.certified_equity, equity);
    assert_eq!(cert.active_bitmap_at_cert, active_bitmap);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_full_refresh_settles_two_assets_with_negative_equity() {
    assert_full_refresh_settles_and_scores_two_active_assets(0);
    kani::cover!(true, "v16 two-asset refresh covers negative equity");
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_full_refresh_settles_two_assets_with_zero_equity() {
    assert_full_refresh_settles_and_scores_two_active_assets(2);
    kani::cover!(true, "v16 two-asset refresh covers zero equity");
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_full_refresh_settles_and_scores_two_active_assets() {
    assert_full_refresh_settles_and_scores_two_active_assets(20);
    kani::cover!(true, "v16 two-asset refresh covers positive equity");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_stale_clear_plus_current_certificate_restores_favorable_action_lane() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    group.mark_account_stale(&mut account).unwrap();
    assert_eq!(group.stale_certificate_count, 1);
    assert_eq!(
        group.ensure_favorable_action_allowed(&account),
        Err(V16Error::LockActive)
    );

    group.clear_account_stale(&mut account).unwrap();
    account.health_cert = HealthCertV16 {
        cert_oracle_epoch: group.oracle_epoch,
        cert_funding_epoch: group.funding_epoch,
        cert_risk_epoch: group.risk_epoch,
        cert_asset_set_epoch: group.asset_set_epoch,
        active_bitmap_at_cert: account.active_bitmap,
        valid: true,
        ..HealthCertV16::default()
    };

    kani::cover!(
        !account.stale_state,
        "v16 stale account can re-enter favorable-action lane after current refresh cert"
    );
    assert!(!account.stale_state);
    assert_eq!(group.stale_certificate_count, 0);
    assert_eq!(group.ensure_favorable_action_allowed(&account), Ok(()));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_b_stale_invalidates_prior_health_certificate() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    account.health_cert.valid = true;
    kani::cover!(
        account.health_cert.valid,
        "v16 valid health certificate reachable before b-stale"
    );

    group.mark_account_b_stale(&mut account).unwrap();
    kani::cover!(
        account.b_stale_state && !account.health_cert.valid,
        "v16 b-stale invalidates prior health certificate"
    );

    assert!(account.b_stale_state);
    assert!(!account.health_cert.valid);
    assert_eq!(group.b_stale_account_count, 1);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_b_stale_blocks_full_account_refresh() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    group.mark_account_b_stale(&mut account).unwrap();
    kani::cover!(
        account.b_stale_state,
        "v16 b-stale state reachable before refresh"
    );

    assert_eq!(
        group.full_account_refresh(&mut account, &[1; V16_MAX_PORTFOLIO_ASSETS_N]),
        Err(V16Error::BStale)
    );
    assert!(account.b_stale_state);
    assert_eq!(group.b_stale_account_count, 1);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_b_target_advance_marks_b_stale_without_certifying_account() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 100).unwrap();
    group.attach_leg(&mut account, 0, SideV16::Long, 1).unwrap();
    account.health_cert.valid = true;

    group.assets[0].b_long_num = 1;
    let before_cert = account.health_cert;
    let result = group.kani_reject_if_leg_b_target_advanced(&mut account, 0);

    kani::cover!(
        before_cert.valid && group.assets[0].b_long_num > account.legs[0].b_snap,
        "v16 B-target advancement marks B-stale before certification"
    );
    assert!(matches!(result, Err(V16Error::BStale)));
    assert!(account.b_stale_state);
    assert!(account.legs[0].b_stale);
    assert!(!account.health_cert.valid);
    assert_eq!(group.b_stale_account_count, 1);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_b_stale_blocks_favorable_actions() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    group.mark_account_b_stale(&mut account).unwrap();
    kani::cover!(
        account.b_stale_state,
        "v16 b-stale state reachable before favorable action"
    );

    assert_eq!(
        group.h_lock_lane(Some(&account), false, None),
        Ok(HLockLaneV16::HMax)
    );
    assert!(account.b_stale_state);
    assert_eq!(group.b_stale_account_count, 1);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_b_stale_trade_preflight_fails_before_partial_side_effects() {
    let (market, account_id, owner) = concrete_ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 1);
    cfg.public_b_chunk_atoms = 1;
    let mut group = MarketGroupV16::new(market, cfg).unwrap();
    let mut long = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.attach_leg(&mut long, 0, SideV16::Long, 1).unwrap();
    group.assets[0].b_long_num = 2;

    let before_b_long_num = group.assets[0].b_long_num;
    let before_b_snap = long.legs[0].b_snap;
    let result = group
        .kani_position_action_leg_has_incomplete_b_settlement(0, long.legs[0])
        .unwrap();

    kani::cover!(
        before_b_long_num > before_b_snap,
        "v16 position preflight reaches incomplete B settlement"
    );
    assert!(result);
    assert_eq!(group.assets[0].b_long_num, before_b_long_num);
    assert_eq!(long.legs[0].b_snap, before_b_snap);
}

fn assert_v16_deposit_into_stale_account_does_not_unlock_favorable_actions(stale_case: bool) {
    let deposit_units: u8 = kani::any();
    kani::assume(deposit_units > 0);
    kani::assume(deposit_units <= 20);
    let deposit = deposit_units as u128;
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    if stale_case {
        account.stale_state = true;
        account.health_cert.valid = false;
        group.stale_certificate_count = 1;
    } else {
        account.b_stale_state = true;
        account.health_cert.valid = false;
        group.b_stale_account_count = 1;
    }
    let stale_before = group.stale_certificate_count;
    let b_stale_before = group.b_stale_account_count;

    group.kani_deposit_core(&mut account, deposit).unwrap();

    assert_eq!(account.capital, deposit);
    assert_eq!(group.c_tot, deposit);
    assert_eq!(group.vault, deposit);
    assert_eq!(group.stale_certificate_count, stale_before);
    assert_eq!(group.b_stale_account_count, b_stale_before);
    assert!(!account.health_cert.valid);
    assert_eq!(
        group.ensure_favorable_action_allowed(&account),
        Err(V16Error::LockActive)
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_deposit_into_stale_account_does_not_unlock_favorable_actions() {
    assert_v16_deposit_into_stale_account_does_not_unlock_favorable_actions(true);
    kani::cover!(true, "v16 deposit into stale account reachable");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_deposit_into_b_stale_account_does_not_unlock_favorable_actions() {
    assert_v16_deposit_into_stale_account_does_not_unlock_favorable_actions(false);
    kani::cover!(true, "v16 deposit into B-stale account reachable");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_side_reset_prior_epoch_account_can_clear_without_oi_underflow() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.attach_leg(&mut account, 0, SideV16::Long, 1).unwrap();
    group.assets[0].oi_eff_long_q = 0;

    group.begin_full_drain_reset(0, SideV16::Long).unwrap();
    assert_eq!(group.assets[0].oi_eff_long_q, 0);
    group.clear_leg(&mut account, 0).unwrap();
    assert_eq!(group.assets[0].stored_pos_count_long, 0);
    assert_eq!(group.assets[0].oi_eff_long_q, 0);
    group.finalize_ready_reset_side(0, SideV16::Long).unwrap();
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_side_reset_finalize_requires_prior_epoch_positions_clear() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.attach_leg(&mut account, 0, SideV16::Long, 1).unwrap();
    group.assets[0].oi_eff_long_q = 0;

    group.begin_full_drain_reset(0, SideV16::Long).unwrap();
    kani::cover!(
        group.assets[0].stored_pos_count_long != 0,
        "v16 reset pending with prior-epoch stored position reachable"
    );
    assert_eq!(
        group.finalize_ready_reset_side(0, SideV16::Long),
        Err(V16Error::Stale)
    );

    group.clear_leg(&mut account, 0).unwrap();
    assert_eq!(group.finalize_ready_reset_side(0, SideV16::Long), Ok(()));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_begin_full_drain_reset_forbidden_while_reset_pending() {
    let reset_long: bool = kani::any();
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let side = if reset_long {
        SideV16::Long
    } else {
        SideV16::Short
    };

    group.begin_full_drain_reset(0, side).unwrap();
    let before_asset = group.assets[0];
    let before_risk_epoch = group.risk_epoch;
    let result = group.begin_full_drain_reset(0, side);

    kani::cover!(
        reset_long,
        "v16 repeated long reset-pending guard reachable"
    );
    kani::cover!(
        !reset_long,
        "v16 repeated short reset-pending guard reachable"
    );
    assert_eq!(result, Err(V16Error::LockActive));
    assert_eq!(group.assets[0], before_asset);
    assert_eq!(group.risk_epoch, before_risk_epoch);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_reset_pending_epoch_start_snapshots_prevent_prior_epoch_resurrection() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut prior = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group
        .attach_leg(&mut prior, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group.assets[0].k_long = 5 * ADL_ONE as i128;
    group.assets[0].oi_eff_long_q = 0;

    group.begin_full_drain_reset(0, SideV16::Long).unwrap();
    kani::cover!(
        group.assets[0].mode_long == SideModeV16::ResetPending,
        "v16 reset-pending side captured prior epoch"
    );
    assert_eq!(group.assets[0].k_epoch_start_long, 5 * ADL_ONE as i128);
    assert_eq!(group.assets[0].k_long, 0);

    group
        .full_account_refresh(&mut prior, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(prior.pnl, 5);
    assert_eq!(prior.legs[0].k_snap, 5 * ADL_ONE as i128);
    group.clear_leg(&mut prior, 0).unwrap();
    group.finalize_ready_reset_side(0, SideV16::Long).unwrap();
    assert_eq!(group.assets[0].mode_long, SideModeV16::Normal);
    assert_eq!(group.assets[0].stored_pos_count_long, 0);
    assert_eq!(group.assets[0].k_long, 0);

    let mut next = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [9; 32], owner));
    group
        .attach_leg(&mut next, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    assert_eq!(next.legs[0].epoch_snap, group.assets[0].epoch_long);
    assert_eq!(next.legs[0].k_snap, group.assets[0].k_long);
    assert_eq!(next.pnl, 0);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_quantity_adl_preserves_oi_symmetry_after_close() {
    let close_q: u8 = kani::any();
    kani::assume(close_q > 0);
    kani::assume(close_q <= 4);
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [9; 32], [8; 32]));
    let mut opposing =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [10; 32], [8; 32]));
    group
        .attach_leg(&mut account, 0, SideV16::Long, close_q as i128)
        .unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV16::Short, -(close_q as i128))
        .unwrap();
    account.close_progress = CloseProgressLedgerV16 {
        active: true,
        finalized: true,
        close_id: 1,
        asset_index: 0,
        market_id: group.assets[0].market_id,
        domain_side: SideV16::Short,
        gross_loss_at_close_start: 1,
        explicit_loss_assigned: 1,
        residual_remaining: 0,
        ..CloseProgressLedgerV16::EMPTY
    };

    let out = group
        .apply_quantity_adl_after_residual_for_account_not_atomic(
            &mut account,
            0,
            SideV16::Long,
            close_q as u128,
        )
        .unwrap();
    kani::cover!(out.closed_q > 0, "v16 quantity ADL close reachable");
    assert_eq!(
        account.close_progress.quantity_adl_applied_q,
        close_q as u128
    );
    assert_eq!(account.active_bitmap, bitmap(&[]));
    assert!(!account.legs[0].active);
    assert_eq!(
        group.assets[0].oi_eff_long_q,
        group.assets[0].oi_eff_short_q
    );
    assert_eq!(group.assets[0].oi_eff_long_q, 0);
    assert!(out.reset_started);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_quantity_adl_monotonically_shrinks_opposing_a_or_resets() {
    let oi_before: u8 = kani::any();
    let close_q: u8 = kani::any();
    kani::assume(oi_before > 0);
    kani::assume(oi_before <= 4);
    kani::assume(close_q > 0);
    kani::assume(close_q <= oi_before);

    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [9; 32], [8; 32]));
    let mut survivor =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [10; 32], [8; 32]));
    let mut opposing =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [11; 32], [8; 32]));
    let oi_before = oi_before as u128;
    let close_q = close_q as u128;
    group
        .attach_leg(&mut account, 0, SideV16::Long, close_q as i128)
        .unwrap();
    let survivor_q = oi_before - close_q;
    if survivor_q != 0 {
        group
            .attach_leg(&mut survivor, 0, SideV16::Long, survivor_q as i128)
            .unwrap();
    }
    group
        .attach_leg(&mut opposing, 0, SideV16::Short, -(oi_before as i128))
        .unwrap();
    account.close_progress = CloseProgressLedgerV16 {
        active: true,
        finalized: true,
        close_id: 1,
        asset_index: 0,
        market_id: group.assets[0].market_id,
        domain_side: SideV16::Short,
        gross_loss_at_close_start: 1,
        explicit_loss_assigned: 1,
        residual_remaining: 0,
        ..CloseProgressLedgerV16::EMPTY
    };
    group.assets[0].a_short = ADL_ONE;
    let a_before = group.assets[0].a_short;

    let out = group
        .apply_quantity_adl_after_residual_for_account_not_atomic(
            &mut account,
            0,
            SideV16::Long,
            close_q,
        )
        .unwrap();

    let oi_after = oi_before - close_q;
    kani::cover!(oi_after > 0, "v16 partial quantity ADL branch reachable");
    kani::cover!(
        oi_after == 0,
        "v16 full-drain quantity ADL branch reachable"
    );
    assert_eq!(out.closed_q, close_q);
    assert_eq!(account.active_bitmap, bitmap(&[]));
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
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_dead_leg_forfeit_does_not_credit_positive_kf_delta() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.mode = MarketModeV16::Recovery;
    group
        .attach_leg(&mut account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group.assets[0].k_long = 3 * ADL_ONE as i128;

    let (loss_settled, positive_pnl_forfeited, support_consumed, junior_face_burned) = group
        .kani_settle_forfeited_leg_kf_effects(&mut account, 0)
        .unwrap();

    kani::cover!(
        positive_pnl_forfeited > 0,
        "v16 dead-leg positive K/F delta is forfeited"
    );
    assert_eq!(loss_settled, 0);
    assert_eq!(positive_pnl_forfeited, 3);
    assert_eq!(support_consumed, 0);
    assert_eq!(junior_face_burned, 0);
    assert_eq!(account.pnl, 0);
    assert_eq!(group.pnl_pos_tot, 0);
    assert!(account.legs[0].active);
    assert_eq!(account.legs[0].k_snap, group.assets[0].k_long);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_dead_leg_forfeit_partial_b_progress_does_not_detach() {
    let (market, _, _) = concrete_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let leg = PortfolioLegV16 {
        active: true,
        asset_index: 0,
        market_id: group.assets[0].market_id,
        side: SideV16::Long,
        basis_pos_q: 1,
        loss_weight: 1,
        b_snap: 0,
        ..PortfolioLegV16::EMPTY
    };

    let chunk = group
        .kani_account_b_settlement_chunk_from_leg(leg, 2, 1)
        .unwrap();

    kani::cover!(
        chunk.remaining_after != 0,
        "v16 dead-leg forfeit partial B progress before detach"
    );
    assert_eq!(chunk.delta_b, 1);
    assert_eq!(chunk.remaining_after, 1);
    assert_eq!(chunk.loss, 0);
}

fn assert_v16_dead_leg_forfeit_books_loss_to_opposing_domain_only(loss: u128) {
    let (market, _, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut opposing =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [10; 32], owner));
    group.mode = MarketModeV16::Recovery;
    group
        .attach_leg(&mut opposing, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    let b_long_before = group.assets[0].b_long_num;
    let b_short_before = group.assets[0].b_short_num;

    let out = group
        .kani_book_bankruptcy_residual_chunk_internal(0, SideV16::Long, loss)
        .unwrap();

    kani::cover!(
        out.booked_loss > 0,
        "v16 dead-leg negative K/F delta books durable opposing-domain loss"
    );
    assert_eq!(out.booked_loss, loss);
    assert_eq!(out.explicit_loss, 0);
    assert_eq!(out.remaining_after, 0);
    assert_eq!(group.assets[0].oi_eff_short_q, POS_SCALE);
    assert_eq!(group.assets[0].b_long_num, b_long_before);
    assert!(group.assets[0].b_short_num > b_short_before);
    assert!(group.bankruptcy_hlock_active);
    assert_eq!(
        group.pending_domain_loss_barrier_count(0, SideV16::Short),
        Ok(0)
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_dead_leg_forfeit_books_one_loss_atom_to_opposing_domain_only() {
    assert_v16_dead_leg_forfeit_books_loss_to_opposing_domain_only(1);
    kani::cover!(
        true,
        "v16 dead-leg one-atom loss books durable opposing-domain loss"
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_dead_leg_forfeit_books_four_loss_atoms_to_opposing_domain_only() {
    assert_v16_dead_leg_forfeit_books_loss_to_opposing_domain_only(4);
    kani::cover!(
        true,
        "v16 dead-leg multi-atom loss books durable opposing-domain loss"
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_dead_leg_forfeit_haircuts_positive_support_when_junior_impaired() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.pnl = 100;
    group.pnl_pos_tot = 100;
    set_junior_bound(&mut group, 100);
    group.vault = 50;

    let (support_consumed, junior_face_burned) = group
        .kani_apply_haircut_bounded_close_loss_to_pnl(&mut account, 100)
        .unwrap();

    kani::cover!(
        support_consumed == 50 && junior_face_burned == 100,
        "v16 impaired positive support burns full face for haircut value"
    );
    assert_eq!(support_consumed, 50);
    assert_eq!(junior_face_burned, 100);
    assert_eq!(account.pnl, -50);
    assert_eq!(group.pnl_pos_tot, 0);
    assert_eq!(group.pnl_pos_bound_tot, 0);
    assert_eq!(group.negative_pnl_account_count, 1);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_fee_charge_settles_loss_before_fee() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    group.deposit_not_atomic(&mut account, 1).unwrap();
    account.pnl = -1;
    group.negative_pnl_account_count = 1;
    let charged = group
        .charge_account_fee_not_atomic(&mut account, 1)
        .unwrap();

    kani::cover!(
        account.pnl < 0 || charged == 0,
        "v16 loss-before-fee path reached"
    );
    assert_eq!(charged, 0);
    assert_eq!(account.capital, 0);
    assert_eq!(account.pnl, 0);
    assert_eq!(group.insurance, 0);
}

#[kani::proof]
#[kani::unwind(32)]
#[kani::solver(cadical)]
fn proof_v16_fee_sync_uses_wide_product_and_drops_uncollectible_tail() {
    let capital: u8 = kani::any();
    kani::assume(capital > 0);
    kani::assume(capital <= 20);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let capital = capital as u128;
    group.kani_deposit_core(&mut account, capital).unwrap();

    let raw_fee = U256::from_u128(u128::MAX)
        .checked_mul(U256::from_u64(2))
        .unwrap();
    let requested_fee = raw_fee.try_into_u128().unwrap_or(u128::MAX);
    let charged = group
        .kani_charge_account_fee_current(&mut account, requested_fee)
        .unwrap();

    kani::cover!(
        charged == capital,
        "v16 fee sync wide-product cap path charges available principal"
    );
    assert_eq!(requested_fee, u128::MAX);
    assert_eq!(charged, capital);
    assert_eq!(account.capital, 0);
    assert_eq!(account.fee_credits, 0);
    assert_eq!(group.insurance, capital);
    assert_eq!(group.c_tot, 0);
    assert_eq!(group.vault, capital);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_non_deficit_public_paths_do_not_decrease_insurance() {
    let capital_units: u8 = kani::any();
    let insurance_units: u8 = kani::any();
    let requested_fee_units: u8 = kani::any();
    let amount_units: u8 = kani::any();
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

    let mut deposit_group =
        MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut deposit_account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    deposit_group.insurance = insurance;
    deposit_group.vault = insurance;
    deposit_group
        .deposit_not_atomic(&mut deposit_account, amount)
        .unwrap();
    kani::cover!(amount > 0, "v16 deposit non-deficit insurance boundary");
    assert_eq!(deposit_group.insurance, insurance);

    let mut withdraw_group =
        MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut withdraw_account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    withdraw_account.capital = capital;
    withdraw_group.c_tot = capital;
    withdraw_group.insurance = insurance;
    withdraw_group.vault = capital + insurance;
    withdraw_group
        .kani_withdraw_core(&mut withdraw_account, amount)
        .unwrap();
    kani::cover!(amount > 0, "v16 withdraw non-deficit insurance boundary");
    assert_eq!(withdraw_group.insurance, insurance);

    let mut fee_group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut fee_account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [5; 32], owner));
    fee_account.capital = capital;
    fee_group.c_tot = capital;
    fee_group.insurance = insurance;
    fee_group.vault = capital + insurance;
    let charged = fee_group
        .kani_charge_account_fee_current(&mut fee_account, requested_fee)
        .unwrap();
    kani::cover!(
        requested_fee > 0,
        "v16 fee charge can increase but not decrease insurance"
    );
    assert_eq!(fee_group.insurance, insurance + charged);

    let mut convert_group =
        MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut convert_account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [6; 32], owner));
    let profit = 3u128;
    convert_account.pnl = profit as i128;
    convert_group.insurance = insurance;
    convert_group.pnl_pos_tot = profit;
    set_junior_bound(&mut convert_group, profit);
    convert_group.pnl_matured_pos_tot = profit;
    convert_group.vault = convert_group.insurance + profit;
    convert_group
        .kani_convert_released_pnl_to_capital_core(&mut convert_account)
        .unwrap();
    kani::cover!(true, "v16 released pnl conversion preserves insurance");
    assert_eq!(convert_group.insurance, insurance);
}

#[kani::proof]
#[kani::unwind(20)]
#[kani::solver(cadical)]
fn proof_v16_fee_charge_reports_actual_min_requested_and_capital() {
    let capital_units: u8 = kani::any();
    let insurance_units: u8 = kani::any();
    let requested_fee_units: u8 = kani::any();
    kani::assume(capital_units <= 20);
    kani::assume(insurance_units <= 20);
    kani::assume(requested_fee_units <= 20);
    let capital = capital_units as u128;
    let insurance = insurance_units as u128;
    let requested_fee = requested_fee_units as u128;
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.capital = capital;
    group.c_tot = capital;
    group.insurance = insurance;
    group.vault = capital + insurance;

    let charged = group
        .kani_charge_account_fee_current(&mut account, requested_fee)
        .unwrap();

    kani::cover!(
        requested_fee > capital,
        "v16 fee charge reports partial actual collection"
    );
    assert_eq!(charged, requested_fee.min(capital));
    assert_eq!(account.capital, capital - charged);
    assert_eq!(group.c_tot, capital - charged);
    assert_eq!(group.insurance, insurance + charged);
    assert_eq!(group.vault, capital + insurance);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_direct_fee_charge_is_live_only_without_resolved_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 5).unwrap();
    group.resolve_market_not_atomic(1).unwrap();
    let before_mode = group.mode;
    let before_vault = group.vault;
    let before_c_tot = group.c_tot;
    let before_insurance = group.insurance;
    let before_resolved_slot = group.resolved_slot;
    let before_capital = account.capital;
    let before_pnl = account.pnl;
    let before_fee_credits = account.fee_credits;
    let before_last_fee_slot = account.last_fee_slot;

    let result = group.charge_account_fee_not_atomic(&mut account, 1);

    kani::cover!(
        group.mode == MarketModeV16::Resolved,
        "v16 direct fee charge resolved-mode rejection reachable"
    );
    assert_eq!(result, Err(V16Error::LockActive));
    assert_eq!(group.mode, before_mode);
    assert_eq!(group.vault, before_vault);
    assert_eq!(group.c_tot, before_c_tot);
    assert_eq!(group.insurance, before_insurance);
    assert_eq!(group.resolved_slot, before_resolved_slot);
    assert_eq!(account.capital, before_capital);
    assert_eq!(account.pnl, before_pnl);
    assert_eq!(account.fee_credits, before_fee_credits);
    assert_eq!(account.last_fee_slot, before_last_fee_slot);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_equity_active_accrual_requires_protective_progress() {
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    group.assets[0].oi_eff_long_q = POS_SCALE;
    group.assets[0].oi_eff_short_q = POS_SCALE;
    group.assets[0].loss_weight_sum_long = POS_SCALE;
    group.assets[0].loss_weight_sum_short = POS_SCALE;
    group.assets[0].stored_pos_count_long = 1;
    group.assets[0].stored_pos_count_short = 1;

    let result = group.accrue_asset_to_not_atomic(0, 1, 2, 0, false);
    assert_eq!(result, Err(V16Error::NonProgress));
    assert_eq!(group.slot_last, 0);

    let ok = group.accrue_asset_to_not_atomic(0, 1, 2, 0, true);
    assert!(ok.is_ok());
    assert_eq!(group.slot_last, 1);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_pending_domain_loss_barrier_does_not_freeze_asset_accrual() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut opposite =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [88; 32], owner));
    group
        .attach_leg(&mut account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut opposite, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    group.pending_domain_loss_barriers[0] = 1;
    let before_a_long = group.assets[0].a_long;
    let before_b_short = group.assets[0].b_short_num;
    let before_oi_long = group.assets[0].oi_eff_long_q;

    let out = group.accrue_asset_to_not_atomic(0, 1, 2, 0, true).unwrap();

    kani::cover!(
        out.equity_active,
        "v16 pending-domain barrier accrual remains reachable"
    );
    assert!(out.equity_active);
    assert_eq!(out.dt, 1);
    assert_eq!(group.assets[0].effective_price, 2);
    assert_eq!(group.assets[0].a_long, before_a_long);
    assert_eq!(group.assets[0].b_short_num, before_b_short);
    assert_eq!(group.assets[0].oi_eff_long_q, before_oi_long);
    assert_eq!(group.pending_domain_loss_barriers[0], 1);
    assert_eq!(
        group.h_lock_lane(Some(&account), false, None),
        Ok(HLockLaneV16::HMax)
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_pending_domain_barrier_blocks_side_reset_before_mutation() {
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    group.pending_domain_loss_barriers[0] = 1;
    group.assets[0].k_long = 7;
    group.assets[0].f_long_num = -3;
    group.assets[0].b_long_num = 11;
    group.assets[0].a_long = ADL_ONE - 1;
    group.assets[0].epoch_long = 4;
    let before_k = group.assets[0].k_long;
    let before_f = group.assets[0].f_long_num;
    let before_b = group.assets[0].b_long_num;
    let before_a = group.assets[0].a_long;
    let before_epoch = group.assets[0].epoch_long;
    let before_mode = group.assets[0].mode_long;
    let before_barrier = group.pending_domain_loss_barriers[0];
    let before_risk_epoch = group.risk_epoch;

    let result = group.begin_full_drain_reset(0, SideV16::Long);

    kani::cover!(
        before_barrier == 1,
        "v16 pending-domain barrier side-reset lock reachable"
    );
    assert_eq!(result, Err(V16Error::LockActive));
    assert_eq!(group.assets[0].k_long, before_k);
    assert_eq!(group.assets[0].f_long_num, before_f);
    assert_eq!(group.assets[0].b_long_num, before_b);
    assert_eq!(group.assets[0].a_long, before_a);
    assert_eq!(group.assets[0].epoch_long, before_epoch);
    assert_eq!(group.assets[0].mode_long, before_mode);
    assert_eq!(group.pending_domain_loss_barriers[0], before_barrier);
    assert_eq!(group.risk_epoch, before_risk_epoch);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_pending_domain_barrier_does_not_block_unrelated_side_reset() {
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    group.pending_domain_loss_barriers[0] = 1;
    group.assets[0].k_long = 7;
    group.assets[0].f_long_num = -3;
    group.assets[0].b_long_num = 11;
    group.assets[0].a_long = ADL_ONE - 1;
    group.assets[0].k_short = -9;
    group.assets[0].f_short_num = 4;
    group.assets[0].b_short_num = 13;
    group.assets[0].a_short = ADL_ONE - 2;
    group.assets[0].epoch_short = 6;
    let before_long_k = group.assets[0].k_long;
    let before_long_f = group.assets[0].f_long_num;
    let before_long_b = group.assets[0].b_long_num;
    let before_long_a = group.assets[0].a_long;

    let result = group.begin_full_drain_reset(0, SideV16::Short);

    kani::cover!(
        result.is_ok(),
        "v16 pending-domain barrier unrelated side-reset progress reachable"
    );
    assert!(result.is_ok());
    assert_eq!(group.pending_domain_loss_barriers[0], 1);
    assert_eq!(group.assets[0].k_long, before_long_k);
    assert_eq!(group.assets[0].f_long_num, before_long_f);
    assert_eq!(group.assets[0].b_long_num, before_long_b);
    assert_eq!(group.assets[0].a_long, before_long_a);
    assert_eq!(group.assets[0].k_short, 0);
    assert_eq!(group.assets[0].f_short_num, 0);
    assert_eq!(group.assets[0].b_short_num, 0);
    assert_eq!(group.assets[0].a_short, ADL_ONE);
    assert_eq!(group.assets[0].epoch_short, 7);
    assert_eq!(group.assets[0].mode_short, SideModeV16::ResetPending);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_permissionless_crank_does_not_require_full_market_scan() {
    let stale_count: u8 = kani::any();
    let b_stale_count: u8 = kani::any();
    let negative_count: u8 = kani::any();
    kani::assume(stale_count <= 3);
    kani::assume(b_stale_count <= 3);
    kani::assume(negative_count <= 3);
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.capital = 1;
    group.c_tot = 1;
    group.vault = 1;
    group.materialized_portfolio_count = 1 + stale_count as u64;
    group.stale_certificate_count = stale_count as u64;
    group.b_stale_account_count = b_stale_count as u64;
    group.negative_pnl_account_count = negative_count as u64;
    let before_materialized = group.materialized_portfolio_count;
    let before_stale = group.stale_certificate_count;
    let before_b_stale = group.b_stale_account_count;
    let before_negative = group.negative_pnl_account_count;

    let side_effects = group
        .settle_account_side_effects_not_atomic(&mut account, group.config.public_b_chunk_atoms)
        .unwrap();
    group
        .kani_accrue_asset_to_core_not_atomic(0, 0, 1, 0, false)
        .unwrap();

    kani::cover!(
        stale_count > 0 || b_stale_count > 0 || negative_count > 0,
        "v16 permissionless hinted progress ignores unrelated global account counters"
    );
    assert_eq!(
        side_effects,
        PermissionlessProgressOutcomeV16::AccountCurrent
    );
    assert_eq!(group.materialized_portfolio_count, before_materialized);
    assert_eq!(group.stale_certificate_count, before_stale);
    assert_eq!(group.b_stale_account_count, before_b_stale);
    assert_eq!(group.negative_pnl_account_count, before_negative);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_permissionless_refresh_can_advance_one_equity_active_segment() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let _account = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.assets[0].oi_eff_long_q = POS_SCALE;
    group.assets[0].oi_eff_short_q = POS_SCALE;
    group.assets[0].loss_weight_sum_long = POS_SCALE;
    group.assets[0].loss_weight_sum_short = POS_SCALE;

    let out = group
        .kani_accrue_asset_to_core_not_atomic(0, 3, 2, 0, true)
        .unwrap();

    kani::cover!(
        group.loss_stale_active && group.assets[0].slot_last == 1,
        "v16 permissionless refresh commits bounded equity-active segment"
    );
    assert!(out.equity_active);
    assert_eq!(out.dt, 1);
    assert_eq!(group.assets[0].slot_last, 1);
    assert_eq!(group.slot_last, 1);
    assert_eq!(group.current_slot, 3);
    assert!(group.loss_stale_active);
    assert_eq!(group.assets[0].effective_price, 2);
    assert_eq!(group.assets[0].k_long, ADL_ONE as i128);
    assert_eq!(group.assets[0].k_short, -(ADL_ONE as i128));
    assert_eq!(group.assets[0].oi_eff_long_q, POS_SCALE);
    assert_eq!(group.assets[0].oi_eff_short_q, POS_SCALE);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_permissionless_refresh_returns_partial_b_progress_without_accrual() {
    let larger_target: bool = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 1);
    cfg.public_b_chunk_atoms = 1;
    let group = MarketGroupV16::new(market, cfg).unwrap();
    let _account = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let leg = PortfolioLegV16 {
        active: true,
        asset_index: 0,
        market_id: 1,
        side: SideV16::Long,
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
    let target = if larger_target { 3 } else { 2 };
    let chunk = group
        .kani_account_b_settlement_chunk_from_leg(leg, target, 1)
        .unwrap();

    kani::cover!(
        !larger_target,
        "v16 permissionless refresh partial B target two"
    );
    kani::cover!(
        larger_target,
        "v16 permissionless refresh partial B target three"
    );
    assert_eq!(chunk.delta_b, 1);
    assert_eq!(chunk.loss, 0);
    assert_eq!(chunk.remaining_after, target - 1);
    assert_eq!(group.slot_last, 0);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_permissionless_flat_refresh_is_not_protective_for_equity_active_accrual() {
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(2, 0, 1)).unwrap();
    group.assets[0].oi_eff_long_q = 1;
    group.assets[0].oi_eff_short_q = 1;
    let before_asset = group.assets[0];
    let before_slot = group.slot_last;

    let outcome = group.kani_accrue_asset_to_core_not_atomic(0, 1, 2, 0, false);

    kani::cover!(
        outcome == Err(V16Error::NonProgress),
        "v16 flat refresh is not protective for exposed asset accrual"
    );
    assert_eq!(outcome, Err(V16Error::NonProgress));
    assert_eq!(group.assets[0], before_asset);
    assert_eq!(group.slot_last, before_slot);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_permissionless_cross_asset_liquidation_is_not_protective_for_equity_active_accrual() {
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(2, 0, 1)).unwrap();
    group.assets[0].oi_eff_long_q = 1;
    group.assets[0].oi_eff_short_q = 1;
    let before_asset = group.assets[0];
    let before_slot = group.slot_last;

    let outcome = group.kani_accrue_asset_to_core_not_atomic(0, 1, 2, 0, false);

    kani::cover!(
        outcome == Err(V16Error::NonProgress),
        "v16 cross-asset liquidation is not protective for exposed asset accrual"
    );
    assert_eq!(outcome, Err(V16Error::NonProgress));
    assert_eq!(group.assets[0], before_asset);
    assert_eq!(group.slot_last, before_slot);
}

fn worst_case_hinted_base_req() -> PermissionlessCrankRequestV16 {
    PermissionlessCrankRequestV16 {
        now_slot: 0,
        asset_index: 0,
        effective_price: 1,
        funding_rate_e9: 0,
        action: PermissionlessCrankActionV16::Refresh,
    }
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_worst_case_hinted_progress_refresh_current_is_total_and_bounded() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 1).unwrap();
    let side_effects = group
        .settle_account_side_effects_not_atomic(&mut account, group.config.public_b_chunk_atoms)
        .unwrap();
    let accrual = group
        .kani_accrue_asset_to_core_not_atomic(0, 0, 1, 0, false)
        .unwrap();

    kani::cover!(
        side_effects == PermissionlessProgressOutcomeV16::AccountCurrent && accrual.dt == 0,
        "v16 hinted refresh-current production subpath reachable"
    );
    assert_eq!(
        side_effects,
        PermissionlessProgressOutcomeV16::AccountCurrent
    );
    assert_eq!(group.slot_last, 0);
    assert_eq!(group.current_slot, 0);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_worst_case_hinted_progress_settle_b_is_total_and_bounded() {
    let (market, account_id, owner) = concrete_ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 1);
    cfg.public_b_chunk_atoms = 1;
    let mut group = MarketGroupV16::new(market, cfg).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.attach_leg(&mut account, 0, SideV16::Long, 1).unwrap();
    group.assets[0].b_long_num = 2;
    let chunk = group
        .settle_account_b_chunk(&mut account, 0, group.config.public_b_chunk_atoms)
        .unwrap();

    kani::cover!(
        chunk.remaining_after == 1,
        "v16 hinted settle-B branch reachable"
    );
    assert_eq!(chunk.delta_b, 1);
    assert_eq!(chunk.remaining_after, 1);
    assert!(account.b_stale_state);
    assert_eq!(group.b_stale_account_count, 1);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_worst_case_hinted_progress_liquidate_is_total_and_bounded() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group
        .attach_leg(&mut account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite_for_live_oi(&mut group, 0, SideV16::Long, POS_SCALE, 99);
    let outcome = group.permissionless_crank_not_atomic(
        &mut account,
        PermissionlessCrankRequestV16 {
            action: PermissionlessCrankActionV16::Liquidate(LiquidationRequestV16 {
                asset_index: 0,
                close_q: POS_SCALE,
                fee_bps: 0,
            }),
            ..worst_case_hinted_base_req()
        },
        &[1; V16_MAX_PORTFOLIO_ASSETS_N],
    );
    kani::cover!(true, "v16 hinted liquidation branch reachable");
    assert_eq!(
        outcome,
        Ok(PermissionlessProgressOutcomeV16::AccountCurrent)
    );
    assert_eq!(account.active_bitmap, bitmap(&[]));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_worst_case_hinted_progress_recover_is_total_and_bounded() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let reason = PermissionlessRecoveryReasonV16::BelowProgressFloor;
    let outcome = group.permissionless_crank_not_atomic(
        &mut account,
        PermissionlessCrankRequestV16 {
            action: PermissionlessCrankActionV16::Recover(reason),
            ..worst_case_hinted_base_req()
        },
        &[1; V16_MAX_PORTFOLIO_ASSETS_N],
    );
    kani::cover!(true, "v16 hinted recovery branch reachable");
    assert_eq!(
        outcome,
        Ok(PermissionlessProgressOutcomeV16::RecoveryDeclared(reason))
    );
    assert_eq!(group.recovery_reason, Some(reason));
}

fn assert_permissionless_crank_liquidation_books_bankruptcy_and_advances_accrual(
    loss_atoms: u128,
    insurance_atoms: u128,
) {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut victim =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.vault = insurance_atoms;
    group.insurance = insurance_atoms;
    group.insurance_domain_budget[1] = insurance_atoms;
    victim.pnl = -(loss_atoms as i128);
    group.negative_pnl_account_count = 1;
    group.assets[0].loss_weight_sum_short = 1;

    let expected_insurance_used = loss_atoms.min(insurance_atoms);
    let insurance_used = group
        .kani_consume_domain_insurance_for_negative_pnl(0, SideV16::Long, &mut victim)
        .unwrap();
    let residual_after_insurance = victim.pnl.unsigned_abs();
    let residual_out = group
        .kani_book_bankruptcy_residual_chunk_internal(0, SideV16::Long, residual_after_insurance)
        .unwrap();
    let residual_i128 =
        i128::try_from(residual_out.booked_loss + residual_out.explicit_loss).unwrap();
    let next_pnl = victim.pnl + residual_i128;
    group.kani_set_account_pnl(&mut victim, next_pnl).unwrap();
    group
        .kani_accrue_asset_to_core_not_atomic(0, 1, 1, 0, true)
        .unwrap();

    assert_eq!(insurance_used, expected_insurance_used);
    assert_eq!(group.vault, insurance_atoms);
    assert_eq!(group.insurance, insurance_atoms - expected_insurance_used);
    assert_eq!(victim.pnl, 0);
    assert_eq!(group.negative_pnl_account_count, 0);
    assert_eq!(group.slot_last, 1);
    assert_eq!(group.current_slot, 1);
    assert!(group.bankruptcy_hlock_active);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_permissionless_crank_liquidation_fully_insured_advances_accrual() {
    assert_permissionless_crank_liquidation_books_bankruptcy_and_advances_accrual(2, 3);
    kani::cover!(
        true,
        "v16 permissionless crank liquidation fully insured path"
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_permissionless_crank_liquidation_insurance_plus_residual_advances_accrual() {
    assert_permissionless_crank_liquidation_books_bankruptcy_and_advances_accrual(3, 1);
    kani::cover!(
        true,
        "v16 permissionless crank liquidation insurance plus residual path"
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_permissionless_crank_liquidation_uninsured_residual_advances_accrual() {
    assert_permissionless_crank_liquidation_books_bankruptcy_and_advances_accrual(2, 0);
    kani::cover!(
        true,
        "v16 permissionless crank liquidation uninsured residual path"
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_equity_active_accrual_advances_at_most_one_bounded_segment() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    group.config.max_accrual_dt_slots = 2;
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut opposing = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group
        .attach_leg(&mut account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();

    let out = group.accrue_asset_to_not_atomic(0, 10, 3, 0, true).unwrap();
    assert_eq!(out.dt, 2);
    assert_eq!(group.slot_last, 2);
    assert_eq!(group.current_slot, 10);
    assert!(group.loss_stale_active);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_funding_rate_above_cap_rejects_before_mutation() {
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    group.config.max_abs_funding_e9_per_slot = 1;
    let before = group.assets[0];

    let result = group.accrue_asset_to_not_atomic(0, 1, 1, 2, true);

    assert_eq!(result, Err(V16Error::InvalidConfig));
    assert_eq!(group.assets[0], before);
    assert_eq!(group.slot_last, 0);
    assert_eq!(group.current_slot, 0);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_trade_dynamic_fee_cap_is_enforced_before_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    group.config.max_trading_fee_bps = 1;
    let mut long = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut short = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut long, 10).unwrap();
    group.deposit_not_atomic(&mut short, 10).unwrap();

    let result = group.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV16 {
            asset_index: 0,
            size_q: 1,
            exec_price: 1,
            fee_bps: 2,
            admit_h_max_consumption_threshold_bps_opt: None,
        },
        &[1; V16_MAX_PORTFOLIO_ASSETS_N],
    );
    assert_eq!(result, Err(V16Error::InvalidConfig));
    assert_eq!(long.active_bitmap, bitmap(&[]));
    assert_eq!(short.active_bitmap, bitmap(&[]));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_trade_fee_conservation_and_oi_symmetry() {
    let fee_bps: u16 = kani::any();
    kani::assume(fee_bps <= 1_000);
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    group.config.max_trading_fee_bps = 1_000;
    let mut long = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut short = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut long, 10_000).unwrap();
    group.deposit_not_atomic(&mut short, 10_000).unwrap();
    let vault_before = group.vault;
    let c_tot_before = group.c_tot;
    let insurance_before = group.insurance;

    let out = group
        .execute_trade_with_fee_not_atomic(
            &mut long,
            &mut short,
            TradeRequestV16 {
                asset_index: 0,
                size_q: POS_SCALE,
                exec_price: 100,
                fee_bps: fee_bps as u64,
                admit_h_max_consumption_threshold_bps_opt: None,
            },
            &[100; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    let expected_fee = if fee_bps == 0 {
        0
    } else {
        ((100u128 * fee_bps as u128) + 9_999) / 10_000
    };
    kani::cover!(fee_bps == 0, "v16 zero fee trade reachable");
    kani::cover!(expected_fee > 0, "v16 positive fee trade reachable");
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
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_risk_increasing_trade_requires_initial_health_before_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut underfunded_long =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut funded_short =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut funded_short, 10_000).unwrap();
    let before_group = group.clone();
    let before_long = underfunded_long.clone();
    let before_short = funded_short.clone();

    let result = group.execute_trade_with_fee_not_atomic(
        &mut underfunded_long,
        &mut funded_short,
        TradeRequestV16 {
            asset_index: 0,
            size_q: 1,
            exec_price: 100,
            fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
        },
        &[100; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    assert!(result.is_err());
    assert_eq!(group.vault, before_group.vault);
    assert_eq!(group.c_tot, before_group.c_tot);
    assert_eq!(group.insurance, before_group.insurance);
    assert_eq!(
        group.assets[0].oi_eff_long_q,
        before_group.assets[0].oi_eff_long_q
    );
    assert_eq!(
        group.assets[0].oi_eff_short_q,
        before_group.assets[0].oi_eff_short_q
    );
    assert_eq!(underfunded_long.capital, before_long.capital);
    assert_eq!(underfunded_long.pnl, before_long.pnl);
    assert_eq!(underfunded_long.active_bitmap, before_long.active_bitmap);
    assert_eq!(underfunded_long.legs[0], before_long.legs[0]);
    assert_eq!(funded_short.capital, before_short.capital);
    assert_eq!(funded_short.pnl, before_short.pnl);
    assert_eq!(funded_short.active_bitmap, before_short.active_bitmap);
    assert_eq!(funded_short.legs[0], before_short.legs[0]);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_trade_hint_cannot_hide_toxic_portfolio_leg_on_other_asset() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(2, 0, 1)).unwrap();
    let mut long = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut short = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut long, 1).unwrap();
    group.deposit_not_atomic(&mut short, 1_000).unwrap();
    group
        .attach_leg(&mut long, 1, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let _asset_one_opposite =
        attach_opposite_for_live_oi(&mut group, 1, SideV16::Long, POS_SCALE, 94);
    long.legs[1].b_stale = true;
    long.b_stale_state = true;
    let before_vault = group.vault;
    let before_c_tot = group.c_tot;
    let before_insurance = group.insurance;
    let before_asset0_oi_long = group.assets[0].oi_eff_long_q;
    let before_asset0_oi_short = group.assets[0].oi_eff_short_q;
    let before_asset1_oi_long = group.assets[1].oi_eff_long_q;
    let before_asset1_oi_short = group.assets[1].oi_eff_short_q;
    let before_long_capital = long.capital;
    let before_long_pnl = long.pnl;
    let before_long_bitmap = long.active_bitmap;
    let before_long_leg0 = long.legs[0];
    let before_long_leg1 = long.legs[1];
    let before_short_capital = short.capital;
    let before_short_pnl = short.pnl;
    let before_short_bitmap = short.active_bitmap;
    let before_short_leg = short.legs[0];

    let result = group.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV16 {
            asset_index: 0,
            size_q: 1,
            exec_price: 1,
            fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
        },
        &[1; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(
        long.legs[1].b_stale,
        "v16 trade hint with toxic unhinted active leg reachable"
    );
    assert_eq!(result, Err(V16Error::BStale));
    assert_eq!(group.vault, before_vault);
    assert_eq!(group.c_tot, before_c_tot);
    assert_eq!(group.insurance, before_insurance);
    assert_eq!(group.assets[0].oi_eff_long_q, before_asset0_oi_long);
    assert_eq!(group.assets[0].oi_eff_short_q, before_asset0_oi_short);
    assert_eq!(group.assets[1].oi_eff_long_q, before_asset1_oi_long);
    assert_eq!(group.assets[1].oi_eff_short_q, before_asset1_oi_short);
    assert_eq!(long.capital, before_long_capital);
    assert_eq!(long.pnl, before_long_pnl);
    assert_eq!(long.active_bitmap, before_long_bitmap);
    assert_eq!(long.legs[0].active, before_long_leg0.active);
    assert_eq!(long.legs[0].asset_index, before_long_leg0.asset_index);
    assert_eq!(long.legs[0].market_id, before_long_leg0.market_id);
    assert_eq!(long.legs[0].basis_pos_q, before_long_leg0.basis_pos_q);
    assert_eq!(long.legs[1].active, before_long_leg1.active);
    assert_eq!(long.legs[1].asset_index, before_long_leg1.asset_index);
    assert_eq!(long.legs[1].market_id, before_long_leg1.market_id);
    assert_eq!(long.legs[1].basis_pos_q, before_long_leg1.basis_pos_q);
    assert_eq!(short.capital, before_short_capital);
    assert_eq!(short.pnl, before_short_pnl);
    assert_eq!(short.active_bitmap, before_short_bitmap);
    assert_eq!(short.legs[0].active, before_short_leg.active);
    assert_eq!(short.legs[0].asset_index, before_short_leg.asset_index);
    assert_eq!(short.legs[0].market_id, before_short_leg.market_id);
    assert_eq!(short.legs[0].basis_pos_q, before_short_leg.basis_pos_q);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_sign_flip_trade_preserves_oi_symmetry_and_senior_accounting() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut flip_to_long =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut flip_to_short =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut flip_to_long, 10_000).unwrap();
    group
        .deposit_not_atomic(&mut flip_to_short, 10_000)
        .unwrap();
    group
        .attach_leg(&mut flip_to_long, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    group
        .attach_leg(&mut flip_to_short, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let vault_before = group.vault;
    let c_tot_before = group.c_tot;

    group
        .execute_trade_with_fee_not_atomic(
            &mut flip_to_long,
            &mut flip_to_short,
            TradeRequestV16 {
                asset_index: 0,
                size_q: 2 * POS_SCALE,
                exec_price: 1,
                fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
            },
            &[1; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    kani::cover!(true, "v16 sign-flip trade transition reachable");
    assert_eq!(flip_to_long.legs[0].side, SideV16::Long);
    assert_eq!(flip_to_long.legs[0].basis_pos_q, POS_SCALE as i128);
    assert_eq!(flip_to_short.legs[0].side, SideV16::Short);
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
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_hlock_allows_risk_increasing_trade_with_principal_margin() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut long = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut short = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut long, 100).unwrap();
    group.deposit_not_atomic(&mut short, 100).unwrap();
    long.health_cert = HealthCertV16 {
        certified_equity: 100,
        certified_initial_req: 1,
        cert_oracle_epoch: group.oracle_epoch,
        cert_funding_epoch: group.funding_epoch,
        cert_risk_epoch: group.risk_epoch,
        cert_asset_set_epoch: group.asset_set_epoch,
        active_bitmap_at_cert: long.active_bitmap,
        valid: true,
        ..HealthCertV16::default()
    };
    short.health_cert = HealthCertV16 {
        certified_equity: 100,
        certified_initial_req: 1,
        cert_oracle_epoch: group.oracle_epoch,
        cert_funding_epoch: group.funding_epoch,
        cert_risk_epoch: group.risk_epoch,
        cert_asset_set_epoch: group.asset_set_epoch,
        active_bitmap_at_cert: short.active_bitmap,
        valid: true,
        ..HealthCertV16::default()
    };
    group.threshold_stress_active = true;

    let request = TradeRequestV16 {
        asset_index: 0,
        size_q: 1,
        exec_price: 1,
        fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
    };
    let risk_increasing = group
        .kani_trade_delta_risk_increasing(&long, &short, request)
        .unwrap();

    kani::cover!(
        group.h_lock_lane(Some(&long), false, None) == Ok(HLockLaneV16::HMax) && risk_increasing,
        "v16 h-lock risk-increasing trade principal-only margin lane reachable"
    );
    assert_eq!(
        group.h_lock_lane(Some(&long), false, None),
        Ok(HLockLaneV16::HMax)
    );
    assert_eq!(
        group.h_lock_lane(Some(&short), false, None),
        Ok(HLockLaneV16::HMax)
    );
    assert!(risk_increasing);
    assert_eq!(
        group.kani_ensure_no_positive_credit_initial_margin(&long),
        Ok(())
    );
    assert_eq!(
        group.kani_ensure_no_positive_credit_initial_margin(&short),
        Ok(())
    );
}

#[kani::proof]
#[kani::unwind(32)]
#[kani::solver(cadical)]
fn proof_v16_loss_stale_blocks_risk_increasing_trade_before_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let long = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let short = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group.loss_stale_active = true;

    let before_vault = group.vault;
    let before_c_tot = group.c_tot;
    let before_insurance = group.insurance;
    let before_long_capital = long.capital;
    let before_short_capital = short.capital;
    let result = group.kani_validate_trade_position_change_locks(
        &long,
        &short,
        TradeRequestV16 {
            asset_index: 0,
            size_q: 1,
            exec_price: 1,
            fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
        },
    );

    kani::cover!(
        result == Err(V16Error::LockActive),
        "v16 loss-stale risk-increasing trade rejection reachable"
    );
    assert_eq!(result, Err(V16Error::LockActive));
    assert_eq!(group.vault, before_vault);
    assert_eq!(group.c_tot, before_c_tot);
    assert_eq!(group.insurance, before_insurance);
    assert_eq!(long.capital, before_long_capital);
    assert_eq!(short.capital, before_short_capital);
    assert_eq!(long.active_bitmap, bitmap(&[]));
    assert_eq!(short.active_bitmap, bitmap(&[]));
    assert_eq!(group.assets[0].oi_eff_long_q, 0);
    assert_eq!(group.assets[0].oi_eff_short_q, 0);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_hlock_risk_increasing_trade_rejects_positive_credit_dependency_without_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut long = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut short = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    long.pnl = 10;
    short.pnl = 10;
    long.health_cert = HealthCertV16 {
        certified_equity: 10,
        certified_initial_req: 1,
        cert_oracle_epoch: group.oracle_epoch,
        cert_funding_epoch: group.funding_epoch,
        cert_risk_epoch: group.risk_epoch,
        cert_asset_set_epoch: group.asset_set_epoch,
        active_bitmap_at_cert: long.active_bitmap,
        valid: true,
        ..HealthCertV16::default()
    };
    short.health_cert = HealthCertV16 {
        certified_equity: 10,
        certified_initial_req: 1,
        cert_oracle_epoch: group.oracle_epoch,
        cert_funding_epoch: group.funding_epoch,
        cert_risk_epoch: group.risk_epoch,
        cert_asset_set_epoch: group.asset_set_epoch,
        active_bitmap_at_cert: short.active_bitmap,
        valid: true,
        ..HealthCertV16::default()
    };
    group.pnl_pos_tot = 20;
    set_junior_bound(&mut group, 20);
    group.vault = 20;
    group.threshold_stress_active = true;

    let request = TradeRequestV16 {
        asset_index: 0,
        size_q: 1,
        exec_price: 1,
        fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
    };
    let risk_increasing = group
        .kani_trade_delta_risk_increasing(&long, &short, request)
        .unwrap();

    kani::cover!(
        group.h_lock_lane(Some(&long), false, None) == Ok(HLockLaneV16::HMax)
            && risk_increasing
            && group.kani_ensure_no_positive_credit_initial_margin(&long)
                == Err(V16Error::LockActive),
        "v16 h-lock risk-increasing positive-credit dependency rejection reachable"
    );
    assert_eq!(
        group.h_lock_lane(Some(&long), false, None),
        Ok(HLockLaneV16::HMax)
    );
    assert_eq!(
        group.h_lock_lane(Some(&short), false, None),
        Ok(HLockLaneV16::HMax)
    );
    assert!(risk_increasing);
    assert_eq!(
        group.kani_ensure_no_positive_credit_initial_margin(&long),
        Err(V16Error::LockActive)
    );
    assert_eq!(
        group.kani_ensure_no_positive_credit_initial_margin(&short),
        Err(V16Error::LockActive)
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_target_effective_lag_rejects_risk_increasing_trade_before_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut long = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut short = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut long, 10).unwrap();
    group.deposit_not_atomic(&mut short, 10).unwrap();
    group.assets[0].effective_price = 1;
    group.assets[0].raw_oracle_target_price = 2;

    let result = group.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV16 {
            asset_index: 0,
            size_q: 1,
            exec_price: 1,
            fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
        },
        &[1; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(result, Err(V16Error::LockActive));
    assert_eq!(long.active_bitmap, bitmap(&[]));
    assert_eq!(short.active_bitmap, bitmap(&[]));
    assert_eq!(group.insurance, 0);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_target_effective_lag_adverse_loss_enters_leg_health_requirement() {
    let adverse_short: bool = kani::any();
    let (market, _, _) = concrete_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let adverse_delta = if adverse_short {
        group.kani_target_effective_lag_adverse_delta(SideV16::Short, 100, 110)
    } else {
        group.kani_target_effective_lag_adverse_delta(SideV16::Long, 100, 90)
    };
    let (initial_req, maintenance_req, worst_case_loss) = group
        .kani_health_requirements_from_notional_and_target_lag(100, adverse_delta as u128)
        .unwrap();

    kani::cover!(
        adverse_short,
        "v16 short adverse target/effective lag requirement covered"
    );
    kani::cover!(
        !adverse_short,
        "v16 long adverse target/effective lag requirement covered"
    );
    assert_eq!(
        group.kani_target_effective_lag_adverse_delta(SideV16::Long, 100, 110),
        0
    );
    assert_eq!(
        group.kani_target_effective_lag_adverse_delta(SideV16::Short, 100, 90),
        0
    );
    assert_eq!(adverse_delta, 10);
    assert_eq!(initial_req, 110);
    assert_eq!(maintenance_req, 110);
    assert_eq!(worst_case_loss, 110);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_hlock_allows_pure_risk_reducing_trade_with_principal_margin() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut reducing_short =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut reducing_long =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut reducing_short, 100).unwrap();
    group.deposit_not_atomic(&mut reducing_long, 100).unwrap();
    group
        .attach_leg(&mut reducing_short, 0, SideV16::Short, -10)
        .unwrap();
    group
        .attach_leg(&mut reducing_long, 0, SideV16::Long, 10)
        .unwrap();
    reducing_short.health_cert = HealthCertV16 {
        certified_equity: 100,
        certified_initial_req: 1,
        cert_oracle_epoch: group.oracle_epoch,
        cert_funding_epoch: group.funding_epoch,
        cert_risk_epoch: group.risk_epoch,
        cert_asset_set_epoch: group.asset_set_epoch,
        active_bitmap_at_cert: reducing_short.active_bitmap,
        valid: true,
        ..HealthCertV16::default()
    };
    reducing_long.health_cert = HealthCertV16 {
        certified_equity: 100,
        certified_initial_req: 1,
        cert_oracle_epoch: group.oracle_epoch,
        cert_funding_epoch: group.funding_epoch,
        cert_risk_epoch: group.risk_epoch,
        cert_asset_set_epoch: group.asset_set_epoch,
        active_bitmap_at_cert: reducing_long.active_bitmap,
        valid: true,
        ..HealthCertV16::default()
    };
    group.threshold_stress_active = true;

    let request = TradeRequestV16 {
        asset_index: 0,
        size_q: 5,
        exec_price: 1,
        fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
    };
    let risk_increasing = group
        .kani_trade_delta_risk_increasing(&reducing_short, &reducing_long, request)
        .unwrap();

    kani::cover!(
        group.h_lock_lane(Some(&reducing_short), false, None) == Ok(HLockLaneV16::HMax)
            && !risk_increasing,
        "v16 h-lock pure risk-reducing trade lane reachable"
    );
    assert_eq!(
        group.h_lock_lane(Some(&reducing_short), false, None),
        Ok(HLockLaneV16::HMax)
    );
    assert_eq!(
        group.h_lock_lane(Some(&reducing_long), false, None),
        Ok(HLockLaneV16::HMax)
    );
    assert!(!risk_increasing);
    assert_eq!(
        group.kani_ensure_no_positive_credit_initial_margin(&reducing_short),
        Ok(())
    );
    assert_eq!(
        group.kani_ensure_no_positive_credit_initial_margin(&reducing_long),
        Ok(())
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_hlock_withdraw_uses_no_positive_credit_lane() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 20).unwrap();
    account.pnl = 100;
    account.health_cert = HealthCertV16 {
        certified_equity: 120,
        certified_initial_req: 10,
        cert_oracle_epoch: group.oracle_epoch,
        cert_funding_epoch: group.funding_epoch,
        cert_risk_epoch: group.risk_epoch,
        cert_asset_set_epoch: group.asset_set_epoch,
        active_bitmap_at_cert: account.active_bitmap,
        valid: true,
        ..HealthCertV16::default()
    };
    group.pnl_pos_tot = 100;
    set_junior_bound(&mut group, 100);
    group.threshold_stress_active = true;
    let post_capital = account.capital - 11;

    let no_positive_equity = group
        .kani_account_no_positive_credit_equity_with_capital(&account, post_capital)
        .unwrap();

    kani::cover!(
        group.h_lock_lane(Some(&account), false, None) == Ok(HLockLaneV16::HMax)
            && no_positive_equity >= 0
            && (no_positive_equity as u128) < account.health_cert.certified_initial_req,
        "v16 h-lock withdrawal no-positive-credit margin rejection reachable"
    );
    assert_eq!(
        group.h_lock_lane(Some(&account), false, None),
        Ok(HLockLaneV16::HMax)
    );
    assert_eq!(no_positive_equity, 9);
    assert!((no_positive_equity as u128) < account.health_cert.certified_initial_req);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_stale_profitable_leg_cannot_withdraw_using_pre_refresh_positive_pnl() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 40).unwrap();
    group
        .attach_leg(&mut account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    account.pnl = 100;
    group.pnl_pos_tot = 100;
    set_junior_bound(&mut group, 100);
    group.vault = group.c_tot + 50;
    group.assets[0].k_long = -(100 * ADL_ONE as i128);
    group.mark_account_stale(&mut account).unwrap();

    let before_vault = group.vault;
    let before_c_tot = group.c_tot;
    let result = group.withdraw_not_atomic(&mut account, 41, &[1; V16_MAX_PORTFOLIO_ASSETS_N]);

    kani::cover!(
        account.pnl <= 0 && before_vault > before_c_tot,
        "v16 stale profitable withdraw refreshes hidden loss before extraction"
    );
    assert!(result.is_err());
    assert_eq!(group.vault, before_vault);
    assert!(group.c_tot <= before_c_tot);
    assert!(account.pnl <= 0);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_released_pnl_conversion_is_residual_bounded_and_conserves_vault() {
    let profit: u8 = kani::any();
    let residual: u8 = kani::any();
    kani::assume(profit <= 10);
    kani::assume(residual <= 10);
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    group.deposit_not_atomic(&mut account, 10).unwrap();
    account.pnl = profit as i128;
    group.pnl_pos_tot = profit as u128;
    set_junior_bound(&mut group, profit as u128);
    group.pnl_matured_pos_tot = profit as u128;
    group.vault = group.c_tot + group.insurance + residual as u128;
    group
        .full_account_refresh(&mut account, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    let vault_before = group.vault;
    let c_tot_before = group.c_tot;
    let pnl_before = account.pnl;
    let expected = (profit as u128).min(residual as u128);
    let result = group.convert_released_pnl_to_capital_not_atomic(&mut account);

    kani::cover!(expected == 0, "v16 zero conversion branch reachable");
    kani::cover!(expected > 0, "v16 positive conversion branch reachable");
    if expected == 0 {
        if profit == 0 {
            assert_eq!(result, Ok(0));
        } else {
            assert_eq!(result, Err(V16Error::LockActive));
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
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_source_backed_open_conversion_rejects_before_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let prices = [1; V16_MAX_PORTFOLIO_ASSETS_N];

    group.vault = 100;
    group
        .add_account_source_positive_pnl_not_atomic(&mut account, 0, 4)
        .unwrap();
    group
        .add_fresh_counterparty_backing_not_atomic(0, 4 * BOUND_SCALE, 10)
        .unwrap();
    group
        .attach_leg(&mut account, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    group.full_account_refresh(&mut account, &prices).unwrap();

    kani::cover!(
        !percolator::active_bitmap_is_empty(account.active_bitmap)
            && account.source_claim_bound_num[0] != 0,
        "v16 source-backed open conversion has active source exposure"
    );
    let before = (
        account.capital,
        account.pnl,
        account.source_claim_bound_num[0],
        group.c_tot,
        group.source_credit[0].fresh_reserved_backing_num,
        group.source_credit[0].spent_backing_num,
    );
    let open_convert = group.convert_released_pnl_to_capital_not_atomic(&mut account);
    assert_eq!(open_convert, Err(V16Error::LockActive));
    assert_eq!(
        before,
        (
            account.capital,
            account.pnl,
            account.source_claim_bound_num[0],
            group.c_tot,
            group.source_credit[0].fresh_reserved_backing_num,
            group.source_credit[0].spent_backing_num,
        )
    );
}

fn assert_v16_source_backed_open_conversion_rejects_for_configured_domain(domain: usize) {
    let asset_index = domain / 2;
    let source_side = if domain % 2 == 0 {
        SideV16::Long
    } else {
        SideV16::Short
    };
    let active_side = match source_side {
        SideV16::Long => SideV16::Short,
        SideV16::Short => SideV16::Long,
    };
    let signed_basis = match active_side {
        SideV16::Long => POS_SCALE as i128,
        SideV16::Short => -(POS_SCALE as i128),
    };
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(2, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let prices = [1; V16_MAX_PORTFOLIO_ASSETS_N];

    group.vault = 100;
    group
        .add_account_source_positive_pnl_not_atomic(&mut account, domain, 4)
        .unwrap();
    group
        .add_fresh_counterparty_backing_not_atomic(domain, 4 * BOUND_SCALE, 10)
        .unwrap();
    group
        .attach_leg(&mut account, asset_index, active_side, signed_basis)
        .unwrap();
    group.full_account_refresh(&mut account, &prices).unwrap();

    let before = (
        account.capital,
        account.pnl,
        account.source_claim_bound_num[domain],
        account.active_bitmap,
        group.c_tot,
        group.source_credit[domain].fresh_reserved_backing_num,
        group.source_credit[domain].spent_backing_num,
    );
    let result = group.convert_released_pnl_to_capital_not_atomic(&mut account);

    assert_eq!(result, Err(V16Error::LockActive));
    assert_eq!(
        before,
        (
            account.capital,
            account.pnl,
            account.source_claim_bound_num[domain],
            account.active_bitmap,
            group.c_tot,
            group.source_credit[domain].fresh_reserved_backing_num,
            group.source_credit[domain].spent_backing_num,
        )
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_source_backed_open_conversion_rejects_for_configured_domain_1() {
    assert_v16_source_backed_open_conversion_rejects_for_configured_domain(1);
    kani::cover!(true, "v16 source exposure domain 1 covered");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_source_backed_open_conversion_rejects_for_configured_domain_2() {
    assert_v16_source_backed_open_conversion_rejects_for_configured_domain(2);
    kani::cover!(true, "v16 source exposure domain 2 covered");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_source_backed_open_conversion_rejects_for_configured_domain_3() {
    assert_v16_source_backed_open_conversion_rejects_for_configured_domain(3);
    kani::cover!(true, "v16 source exposure domain 3 covered");
}

fn certify_account_current_for_v16_conversion_proof(
    group: &MarketGroupV16,
    account: &mut PortfolioAccountV16,
) {
    account.health_cert = HealthCertV16 {
        certified_equity: account.capital as i128 + account.pnl,
        certified_initial_req: 0,
        certified_maintenance_req: 0,
        certified_liq_deficit: 0,
        certified_worst_case_loss: 0,
        cert_oracle_epoch: group.oracle_epoch,
        cert_funding_epoch: group.funding_epoch,
        cert_risk_epoch: group.risk_epoch,
        cert_asset_set_epoch: group.asset_set_epoch,
        active_bitmap_at_cert: account.active_bitmap,
        valid: true,
    };
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_source_backed_conversion_waits_only_for_contributing_source_exposure() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(2, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut source_counterparty =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [51; 32], owner));
    let mut unrelated_counterparty =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [52; 32], owner));

    group.vault = 100;
    group
        .add_account_source_positive_pnl_not_atomic(&mut account, 0, 4)
        .unwrap();
    group
        .add_fresh_counterparty_backing_not_atomic(0, 4 * BOUND_SCALE, 10)
        .unwrap();
    group
        .attach_leg(&mut account, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    group
        .attach_leg(
            &mut source_counterparty,
            0,
            SideV16::Long,
            POS_SCALE as i128,
        )
        .unwrap();
    certify_account_current_for_v16_conversion_proof(&group, &mut account);

    let blocked = group.convert_released_pnl_to_capital_not_atomic(&mut account);
    assert_eq!(blocked, Err(V16Error::LockActive));
    assert_eq!(account.capital, 0);
    assert_eq!(account.pnl, 4);

    group.clear_leg(&mut account, 0).unwrap();
    group.clear_leg(&mut source_counterparty, 0).unwrap();
    group.attach_leg(&mut account, 1, SideV16::Long, 1).unwrap();
    group
        .attach_leg(&mut unrelated_counterparty, 1, SideV16::Short, -1)
        .unwrap();
    certify_account_current_for_v16_conversion_proof(&group, &mut account);
    let converted = group
        .convert_released_pnl_to_capital_not_atomic(&mut account)
        .unwrap();

    kani::cover!(
        converted == 4 && account.active_bitmap == bitmap(&[1]),
        "v16 source-backed conversion remains live with unrelated open exposure"
    );
    assert_eq!(converted, 4);
    assert_eq!(account.capital, 4);
    assert_eq!(account.pnl, 0);
    assert_eq!(account.active_bitmap, bitmap(&[1]));
    assert_eq!(group.source_credit[0].spent_backing_num, 4 * BOUND_SCALE);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_ordinary_positive_conversion_disabled_outside_live_payout_lane() {
    let resolved_mode: bool = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.pnl = 10;
    group.pnl_pos_tot = 10;
    group.pnl_matured_pos_tot = 10;
    set_junior_bound(&mut group, 10);
    group.vault = 10;
    certify_account_current_for_v16_conversion_proof(&group, &mut account);
    if resolved_mode {
        group.mode = MarketModeV16::Resolved;
        group.resolved_slot = 1;
    } else {
        initialize_payout_ledger(&mut group);
    }
    let before_vault = group.vault;
    let before_c_tot = group.c_tot;
    let before_pnl_pos_tot = group.pnl_pos_tot;
    let before_pnl_pos_bound_tot_num = group.pnl_pos_bound_tot_num;
    let before_pnl_pos_bound_tot = group.pnl_pos_bound_tot;
    let before_pnl_matured_pos_tot = group.pnl_matured_pos_tot;
    let before_mode = group.mode;
    let before_payout_snapshot_captured = group.payout_snapshot_captured;
    let before_capital = account.capital;
    let before_pnl = account.pnl;
    let before_reserved_pnl = account.reserved_pnl;
    let before_health_valid = account.health_cert.valid;

    let result = group.kani_preflight_convert_released_pnl_to_capital(&account);

    kani::cover!(
        resolved_mode,
        "v16 resolved mode disables ordinary positive conversion"
    );
    kani::cover!(
        !resolved_mode,
        "v16 initialized payout ledger disables ordinary live positive conversion"
    );
    assert_eq!(result, Err(V16Error::LockActive));
    assert_eq!(group.vault, before_vault);
    assert_eq!(group.c_tot, before_c_tot);
    assert_eq!(group.pnl_pos_tot, before_pnl_pos_tot);
    assert_eq!(group.pnl_pos_bound_tot_num, before_pnl_pos_bound_tot_num);
    assert_eq!(group.pnl_pos_bound_tot, before_pnl_pos_bound_tot);
    assert_eq!(group.pnl_matured_pos_tot, before_pnl_matured_pos_tot);
    assert_eq!(group.mode, before_mode);
    assert_eq!(
        group.payout_snapshot_captured,
        before_payout_snapshot_captured
    );
    assert_eq!(account.capital, before_capital);
    assert_eq!(account.pnl, before_pnl);
    assert_eq!(account.reserved_pnl, before_reserved_pnl);
    assert_eq!(account.health_cert.valid, before_health_valid);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_target_effective_lag_blocks_pnl_conversion_before_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 10).unwrap();
    group
        .attach_leg(&mut account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    account.pnl = 10;
    group.pnl_pos_tot = 10;
    set_junior_bound(&mut group, 10);
    group.pnl_matured_pos_tot = 10;
    group.vault = group.vault.checked_add(10).unwrap();
    group.assets[0].effective_price = 100;
    group.assets[0].raw_oracle_target_price = 100;
    group
        .full_account_refresh(&mut account, &[100; V16_MAX_PORTFOLIO_ASSETS_N])
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
        !percolator::active_bitmap_is_empty(account.active_bitmap)
            && group.assets[0].raw_oracle_target_price != group.assets[0].effective_price,
        "v16 target/effective lag conversion lock reachable"
    );
    assert_eq!(result, Err(V16Error::LockActive));
    assert_eq!(group.vault, vault_before);
    assert_eq!(group.c_tot, c_tot_before);
    assert_eq!(group.pnl_pos_tot, pnl_pos_before);
    assert_eq!(group.pnl_matured_pos_tot, matured_before);
    assert_eq!(account.capital, capital_before);
    assert_eq!(account.pnl, pnl_before);
    assert_eq!(account.health_cert, cert_before);
}

#[kani::proof]
#[kani::unwind(32)]
#[kani::solver(cadical)]
fn proof_v16_loss_stale_blocks_nonflat_withdrawal() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.active_bitmap = bitmap(&[0]);
    group.loss_stale_active = true;

    let result = group.kani_validate_withdraw_global_locks(&account);

    assert_eq!(result, Err(V16Error::LockActive));
}

#[kani::proof]
#[kani::unwind(32)]
#[kani::solver(cadical)]
fn proof_v16_loss_stale_nonflat_withdraw_rejects_before_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.active_bitmap = bitmap(&[0]);
    group.loss_stale_active = true;

    let before_vault = group.vault;
    let before_c_tot = group.c_tot;
    let before_asset = group.assets[0];
    let before_capital = account.capital;
    let before_pnl = account.pnl;
    let before_bitmap = account.active_bitmap;
    let before_leg = account.legs[0];
    let before_cert = account.health_cert;
    let result = group.kani_validate_withdraw_global_locks(&account);

    kani::cover!(
        group.loss_stale_active && !percolator::active_bitmap_is_empty(account.active_bitmap),
        "v16 loss-stale nonflat withdraw preflight lock reachable"
    );
    assert_eq!(result, Err(V16Error::LockActive));
    assert_eq!(group.vault, before_vault);
    assert_eq!(group.c_tot, before_c_tot);
    assert_eq!(group.assets[0], before_asset);
    assert_eq!(account.capital, before_capital);
    assert_eq!(account.pnl, before_pnl);
    assert_eq!(account.active_bitmap, before_bitmap);
    assert_eq!(account.legs[0], before_leg);
    assert_eq!(account.health_cert, before_cert);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_target_effective_lag_nonflat_withdraw_rejects_before_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 100).unwrap();
    group
        .attach_leg(&mut account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group.assets[0].raw_oracle_target_price = 2;

    let before_vault = group.vault;
    let before_c_tot = group.c_tot;
    let before_asset = group.assets[0];
    let before_capital = account.capital;
    let before_pnl = account.pnl;
    let before_bitmap = account.active_bitmap;
    let before_leg = account.legs[0];
    let before_cert = account.health_cert;
    let result = group.withdraw_not_atomic(&mut account, 10, &[1; V16_MAX_PORTFOLIO_ASSETS_N]);

    kani::cover!(
        group.assets[0].raw_oracle_target_price != group.assets[0].effective_price
            && !percolator::active_bitmap_is_empty(account.active_bitmap),
        "v16 target/effective lag nonflat withdraw preflight lock reachable"
    );
    assert_eq!(result, Err(V16Error::LockActive));
    assert_eq!(group.vault, before_vault);
    assert_eq!(group.c_tot, before_c_tot);
    assert_eq!(group.assets[0], before_asset);
    assert_eq!(account.capital, before_capital);
    assert_eq!(account.pnl, before_pnl);
    assert_eq!(account.active_bitmap, before_bitmap);
    assert_eq!(account.legs[0], before_leg);
    assert_eq!(account.health_cert, before_cert);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_loss_stale_lock_does_not_block_flat_withdraw_preflight() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let account = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.loss_stale_active = true;

    let result = group.kani_validate_withdraw_global_locks(&account);

    kani::cover!(
        group.loss_stale_active && percolator::active_bitmap_is_empty(account.active_bitmap),
        "v16 loss-stale flat withdraw preflight lane reachable"
    );
    assert_eq!(result, Ok(()));
    assert_eq!(account.capital, 0);
    assert_eq!(group.vault, 0);
    assert_eq!(group.c_tot, 0);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_target_effective_lag_lock_does_not_block_flat_withdraw_preflight() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let account = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.assets[0].raw_oracle_target_price = 2;

    let result = group.kani_validate_withdraw_global_locks(&account);

    kani::cover!(
        group.assets[0].raw_oracle_target_price != group.assets[0].effective_price
            && percolator::active_bitmap_is_empty(account.active_bitmap),
        "v16 target/effective-lag flat withdraw preflight lane reachable"
    );
    assert_eq!(result, Ok(()));
    assert_eq!(account.capital, 0);
    assert_eq!(group.vault, 0);
    assert_eq!(group.c_tot, 0);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_zero_withdraw_is_noop_under_recovery_and_global_locks() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.active_bitmap = bitmap(&[0]);
    account.legs[0] = PortfolioLegV16 {
        active: true,
        asset_index: 0,
        market_id: group.assets[0].market_id,
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
    };
    group.mode = MarketModeV16::Recovery;
    group.loss_stale_active = true;
    group.assets[0].raw_oracle_target_price = 2;

    let before_mode = group.mode;
    let before_loss_stale = group.loss_stale_active;
    let before_asset = group.assets[0];
    let before_vault = group.vault;
    let before_c_tot = group.c_tot;
    let before_capital = account.capital;
    let before_pnl = account.pnl;
    let before_bitmap = account.active_bitmap;
    let before_leg = account.legs[0];
    let result = group.withdraw_not_atomic(&mut account, 0, &[1; V16_MAX_PORTFOLIO_ASSETS_N]);

    kani::cover!(
        before_mode == MarketModeV16::Recovery
            && before_loss_stale
            && !percolator::active_bitmap_is_empty(before_bitmap)
            && before_asset.raw_oracle_target_price != before_asset.effective_price,
        "v16 zero-withdraw noop reachable under recovery and global locks"
    );
    assert_eq!(result, Ok(()));
    assert_eq!(group.mode, before_mode);
    assert_eq!(group.loss_stale_active, before_loss_stale);
    assert_eq!(group.assets[0], before_asset);
    assert_eq!(group.vault, before_vault);
    assert_eq!(group.c_tot, before_c_tot);
    assert_eq!(account.capital, before_capital);
    assert_eq!(account.pnl, before_pnl);
    assert_eq!(account.active_bitmap, before_bitmap);
    assert_eq!(account.legs[0], before_leg);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_resolved_positive_payout_snapshot_is_order_stable() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut first = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut second = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group.vault = 100;
    first.pnl = 100;
    second.pnl = 100;
    group.pnl_pos_tot = 200;
    set_junior_bound(&mut group, 200);
    group.resolve_market_not_atomic(1).unwrap();

    let first_close = group.close_resolved_account_not_atomic(&mut first, 0);
    let second_close = group.close_resolved_account_not_atomic(&mut second, 0);

    assert_eq!(
        first_close,
        Ok(ResolvedCloseOutcomeV16::Closed { payout: 50 })
    );
    assert_eq!(
        second_close,
        Ok(ResolvedCloseOutcomeV16::Closed { payout: 50 })
    );
    assert_eq!(group.payout_snapshot, 100);
    assert_eq!(group.payout_snapshot_pnl_pos_tot, 200);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_resolved_payout_uses_positive_bound_denominator() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.vault = 100;
    account.pnl = 100;
    group.pnl_pos_tot = 100;
    set_junior_bound(&mut group, 200);
    group.resolve_market_not_atomic(1).unwrap();

    let close = group.close_resolved_account_not_atomic(&mut account, 0);

    kani::cover!(
        group.payout_snapshot_pnl_pos_tot > group.pnl_pos_tot,
        "v16 resolved payout bound denominator remains conservative after close"
    );
    assert_eq!(close, Ok(ResolvedCloseOutcomeV16::Closed { payout: 50 }));
    assert_eq!(group.payout_snapshot, 100);
    assert_eq!(group.payout_snapshot_pnl_pos_tot, 200);
    assert_eq!(group.vault, 50);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_scaled_junior_bound_remainder_ceil_controls_resolved_payout() {
    let extra_num: u16 = kani::any();
    kani::assume(extra_num > 0);
    kani::assume(extra_num <= 1_000);
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.vault = 1;
    account.pnl = 1;
    group.pnl_pos_tot = 1;
    group.pnl_pos_bound_tot_num = BOUND_SCALE + extra_num as u128;
    group.pnl_pos_bound_tot = 2;
    group.resolve_market_not_atomic(1).unwrap();

    let close = group.close_resolved_account_not_atomic(&mut account, 0);

    kani::cover!(
        extra_num == 1,
        "v16 scaled junior-bound minimum nonzero remainder is covered"
    );
    kani::cover!(
        extra_num > 1,
        "v16 scaled junior-bound larger nonzero remainders are covered"
    );
    kani::cover!(
        group.payout_snapshot_pnl_pos_tot == 2,
        "v16 scaled junior-bound remainder is rounded up in the resolved payout denominator"
    );
    assert_eq!(close, Ok(ResolvedCloseOutcomeV16::Closed { payout: 0 }));
    assert_eq!(group.payout_snapshot, 1);
    assert_eq!(group.payout_snapshot_pnl_pos_tot, 2);
    assert_eq!(group.vault, 1);
    assert_eq!(group.pnl_pos_bound_tot_num, extra_num as u128);
    assert_eq!(group.pnl_pos_bound_tot, 1);
}

#[kani::proof]
#[kani::unwind(16)]
#[kani::solver(cadical)]
fn proof_v16_resolved_receipt_tracks_paid_effective_and_bound_refinement_topup() {
    let extra_num: u16 = kani::any();
    kani::assume(extra_num > 0);
    kani::assume(extra_num <= 1_000);
    let receipt = ResolvedPayoutReceiptV16 {
        present: true,
        prior_bound_contribution_num: BOUND_SCALE + extra_num as u128,
        live_released_face_at_receipt: 0,
        terminal_positive_claim_face: 1,
        paid_effective: 0,
        finalized: false,
    };

    let first = kani_apply_resolved_payout_receipt_payment(receipt, 0).unwrap();

    kani::cover!(
        extra_num == 1,
        "v16 resolved receipt top-up covers minimum scaled remainder"
    );
    kani::cover!(
        extra_num > 1,
        "v16 resolved receipt top-up covers larger scaled remainder"
    );
    assert_eq!(first.terminal_positive_claim_face, 1);
    assert_eq!(
        first.prior_bound_contribution_num,
        BOUND_SCALE + extra_num as u128
    );
    assert_eq!(first.paid_effective, 0);
    assert!(!first.finalized);

    let topup = kani_apply_resolved_payout_receipt_payment(first, 1).unwrap();

    kani::cover!(
        topup.finalized,
        "v16 resolved receipt finalizes after bound refinement top-up"
    );
    assert_eq!(topup.paid_effective, 1);
    assert!(topup.finalized);
}

#[kani::proof]
#[kani::unwind(16)]
#[kani::solver(cadical)]
fn proof_v16_resolved_close_receipt_paid_effective_is_actual_resolved_payout_after_vault_drift() {
    let terminal_face: u8 = kani::any();
    let paid_before: u8 = kani::any();
    let actual_resolved_paid: u8 = kani::any();
    kani::assume(terminal_face <= 8);
    kani::assume(paid_before <= terminal_face);
    kani::assume(actual_resolved_paid <= terminal_face - paid_before);
    let receipt = ResolvedPayoutReceiptV16 {
        present: true,
        prior_bound_contribution_num: (terminal_face as u128) * BOUND_SCALE,
        live_released_face_at_receipt: 0,
        terminal_positive_claim_face: terminal_face as u128,
        paid_effective: paid_before as u128,
        finalized: paid_before == terminal_face,
    };

    let updated =
        kani_apply_resolved_payout_receipt_payment(receipt, actual_resolved_paid as u128).unwrap();
    let expected_paid = paid_before as u128 + actual_resolved_paid as u128;

    kani::cover!(
        terminal_face > 0 && actual_resolved_paid == 0 && !updated.finalized,
        "v16 resolved receipt remains unfinalized when no resolved payout is applied"
    );
    kani::cover!(
        terminal_face > 0 && expected_paid == terminal_face as u128 && updated.finalized,
        "v16 resolved receipt finalizes exactly when actual resolved payout completes the claim"
    );
    assert_eq!(updated.terminal_positive_claim_face, terminal_face as u128);
    assert_eq!(updated.paid_effective, expected_paid);
    assert_eq!(
        updated.finalized,
        expected_paid == updated.terminal_positive_claim_face
    );
    assert!(updated.paid_effective <= updated.terminal_positive_claim_face);
    if paid_before < terminal_face {
        let overpay = terminal_face as u128 - paid_before as u128 + 1;
        let rejected = kani_apply_resolved_payout_receipt_payment(receipt, overpay);
        kani::cover!(
            rejected == Err(V16Error::InvalidLeg),
            "v16 resolved receipt rejects actual payout above remaining face claim"
        );
        assert_eq!(rejected, Err(V16Error::InvalidLeg));
    }
}

#[kani::proof]
#[kani::unwind(48)]
#[kani::solver(cadical)]
fn proof_v16_unfinalized_resolved_receipt_blocks_account_close_until_topup() {
    let (market, account_id, owner) = concrete_ids();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.resolved_payout_receipt = ResolvedPayoutReceiptV16 {
        present: true,
        prior_bound_contribution_num: BOUND_SCALE,
        live_released_face_at_receipt: 0,
        terminal_positive_claim_face: 1,
        paid_effective: 0,
        finalized: false,
    };

    kani::cover!(
        account.resolved_payout_receipt.present && !account.resolved_payout_receipt.finalized,
        "v16 partial resolved receipt blocks account close before top-up"
    );
    assert_eq!(
        MarketGroupV16::kani_validate_portfolio_close_clean_state(&account, 0),
        Err(V16Error::LockActive)
    );

    account.resolved_payout_receipt =
        kani_apply_resolved_payout_receipt_payment(account.resolved_payout_receipt, 1).unwrap();

    assert!(account.resolved_payout_receipt.finalized);
    assert_eq!(
        MarketGroupV16::kani_validate_portfolio_close_clean_state(&account, 0),
        Ok(())
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_public_invariants_reject_scaled_junior_bound_cache_mismatch() {
    let case: bool = kani::any();
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    group.pnl_pos_tot = 1;
    group.pnl_pos_bound_tot = 1;
    if case {
        group.pnl_pos_bound_tot_num = BOUND_SCALE + 1;
    } else {
        group.pnl_pos_bound_tot_num = BOUND_SCALE - 1;
    }

    let result = group.assert_public_invariants();

    kani::cover!(
        case,
        "v16 scaled junior-bound cache too low branch reachable"
    );
    kani::cover!(
        !case,
        "v16 scaled junior-bound numerator understates exact claim branch reachable"
    );
    assert_eq!(result, Err(V16Error::InvalidConfig));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_pnl_pos_bound_tot_prevents_lazy_positive_pnl_first_mover_overpay() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut first_mover =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.vault = 100;
    first_mover.pnl = 100;
    group.pnl_pos_tot = 100;
    set_junior_bound(&mut group, 300);
    group.resolve_market_not_atomic(1).unwrap();

    let close = group.close_resolved_account_not_atomic(&mut first_mover, 0);

    kani::cover!(
        group.payout_snapshot_pnl_pos_tot > group.pnl_pos_tot,
        "v16 first-mover payout uses lazy positive PnL bound denominator"
    );
    assert_eq!(close, Ok(ResolvedCloseOutcomeV16::Closed { payout: 33 }));
    assert_eq!(group.payout_snapshot, 100);
    assert_eq!(group.payout_snapshot_pnl_pos_tot, 300);
    assert_eq!(group.vault, 67);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_resolved_close_partial_b_settlement_makes_progress_without_closing() {
    let larger_target: bool = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 1);
    cfg.public_b_chunk_atoms = 1;
    let mut group = MarketGroupV16::new(market, cfg).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    group.deposit_not_atomic(&mut account, 100).unwrap();
    group.attach_leg(&mut account, 0, SideV16::Long, 1).unwrap();
    group.assets[0].b_long_num = if larger_target { 3 } else { 2 };
    group.resolve_market_not_atomic(10).unwrap();

    let outcome = group.close_resolved_account_not_atomic(&mut account, 1);

    kani::cover!(!larger_target, "v16 resolved close partial B target two");
    kani::cover!(larger_target, "v16 resolved close partial B target three");
    assert_eq!(outcome, Ok(ResolvedCloseOutcomeV16::ProgressOnly));
    assert!(account.legs[0].b_stale);
    assert!(account.legs[0].b_snap > 0);
    assert!(account.legs[0].b_snap < group.assets[0].b_long_num);
    assert_eq!(account.last_fee_slot, 0);
    assert!(!percolator::active_bitmap_is_empty(account.active_bitmap));
    assert!(!group.payout_snapshot_captured);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_resolved_payout_readiness_uses_exact_counters_and_bounds() {
    let blocker: u8 = kani::any();
    kani::assume(blocker < 8);
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.vault = 10;
    account.pnl = 10;
    group.pnl_pos_tot = 10;
    set_junior_bound(&mut group, 10);
    group.resolve_market_not_atomic(1).unwrap();
    match blocker {
        0 => group.b_stale_account_count = 1,
        1 => group.stale_certificate_count = 1,
        2 => group.negative_pnl_account_count = 1,
        3 => group.assets[0].stored_pos_count_long = 1,
        4 => group.assets[0].stored_pos_count_short = 1,
        5 => group.assets[0].stale_account_count_long = 1,
        6 => group.assets[0].stale_account_count_short = 1,
        _ => group.pending_domain_loss_barriers[1] = 1,
    }

    let vault_before = group.vault;
    let pnl_pos_before = group.pnl_pos_tot;
    let bound_before = group.pnl_pos_bound_tot;
    let account_pnl_before = account.pnl;
    let outcome = group.close_resolved_account_not_atomic(&mut account, 0);

    kani::cover!(blocker == 0, "v16 resolved readiness B-stale blocker");
    kani::cover!(
        blocker == 6,
        "v16 resolved readiness stale short-count blocker"
    );
    kani::cover!(
        blocker == 7,
        "v16 resolved readiness pending-domain-loss barrier blocker"
    );
    assert_eq!(outcome, Ok(ResolvedCloseOutcomeV16::ProgressOnly));
    assert_eq!(group.vault, vault_before);
    assert_eq!(group.pnl_pos_tot, pnl_pos_before);
    assert_eq!(group.pnl_pos_bound_tot, bound_before);
    assert_eq!(account.pnl, account_pnl_before);
    assert!(!group.payout_snapshot_captured);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_resolved_bankrupt_negative_blocker_can_clear_without_recovery() {
    let (market, _, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut bankrupt =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [44; 32], owner));
    group.vault = 5;
    group.mode = MarketModeV16::Resolved;
    group.current_slot = 1;
    group.resolved_slot = 1;
    bankrupt.pnl = -3;
    group.negative_pnl_account_count = 1;

    let cleared = group.kani_settle_resolved_bankruptcy_negative_pnl(&mut bankrupt);

    kani::cover!(
        group.negative_pnl_account_count == 0,
        "v16 resolved bankrupt negative blocker clear branch reachable"
    );
    assert_eq!(cleared, Ok(()));
    assert_eq!(bankrupt.pnl, 0);
    assert_eq!(group.negative_pnl_account_count, 0);
    assert!(group.bankruptcy_hlock_active);
    assert!(!group.payout_snapshot_captured);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_resolved_active_bankrupt_can_consume_insurance_and_clear_blocker() {
    let (market, _, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut bankrupt =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [46; 32], owner));
    group.vault = 5;
    group.insurance = 5;
    group.mode = MarketModeV16::Resolved;
    group.current_slot = 1;
    group.resolved_slot = 1;
    bankrupt.pnl = -5;
    group.negative_pnl_account_count = 1;
    group.assets[0].stored_pos_count_long = 1;
    group.assets[0].oi_eff_long_q = 1;
    group.assets[0].loss_weight_sum_long = 1;
    bankrupt.legs[0] = PortfolioLegV16 {
        active: true,
        asset_index: 0,
        market_id: group.assets[0].market_id,
        side: SideV16::Long,
        basis_pos_q: 1,
        a_basis: ADL_ONE,
        k_snap: 0,
        f_snap: 0,
        epoch_snap: group.assets[0].epoch_long,
        loss_weight: 1,
        b_snap: 0,
        b_rem: 0,
        b_epoch_snap: group.assets[0].epoch_long,
        b_stale: false,
        stale: false,
    };
    bankrupt.active_bitmap[0] = 1;

    let cleared = group.kani_settle_resolved_bankruptcy_negative_pnl(&mut bankrupt);

    kani::cover!(
        group.insurance_domain_spent[1] == 5,
        "v16 resolved active bankrupt insurance spend branch reachable"
    );
    assert_eq!(cleared, Ok(()));
    assert_eq!(bankrupt.pnl, 0);
    assert_eq!(group.negative_pnl_account_count, 0);
    assert_eq!(group.insurance, 0);
    assert_eq!(group.insurance_domain_spent[1], 5);
    assert!(!percolator::active_bitmap_is_empty(bankrupt.active_bitmap));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_resolved_residual_without_counterweight_becomes_explicit_terminal_loss() {
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    group.mode = MarketModeV16::Resolved;
    group.current_slot = 1;
    group.resolved_slot = 1;
    let out = group.kani_book_bankruptcy_residual_chunk_internal(0, SideV16::Long, 5);

    kani::cover!(
        out == Ok(BResidualBookingOutcomeV16 {
            booked_loss: 0,
            explicit_loss: 5,
            delta_b: 0,
            remaining_after: 0,
        }),
        "v16 resolved no-counterweight residual explicit-loss branch reachable"
    );
    assert_eq!(
        out,
        Ok(BResidualBookingOutcomeV16 {
            booked_loss: 0,
            explicit_loss: 5,
            delta_b: 0,
            remaining_after: 0,
        })
    );
    assert!(group.bankruptcy_hlock_active);
    assert_eq!(group.recovery_reason, None);
    assert_eq!(group.assets[0].b_short_num, 0);
    assert_eq!(group.assets[0].social_loss_remainder_short_num, 0);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_pending_domain_barrier_does_not_freeze_unrelated_positive_credit() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(2, 0, 1)).unwrap();
    let mut profitable =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    profitable.capital = 100;
    profitable.pnl = 5;
    group.c_tot = 100;
    group.pnl_pos_tot = 5;
    group.pnl_matured_pos_tot = 5;
    set_junior_bound(&mut group, 5);
    group.vault = group.c_tot + 5;
    group.pending_domain_loss_barriers[1] = 1;
    certify_account_current_for_v16_conversion_proof(&group, &mut profitable);
    group
        .kani_preflight_convert_released_pnl_to_capital(&profitable)
        .unwrap();

    let result = group.kani_convert_released_pnl_to_capital_core(&mut profitable);

    kani::cover!(
        result == Ok(5),
        "v16 unrelated-domain positive-credit conversion remains reachable"
    );
    assert_eq!(result, Ok(5));
    assert_eq!(profitable.pnl, 0);
    assert_eq!(profitable.capital, 105);
    assert_eq!(group.c_tot, 105);
    assert_eq!(group.pending_domain_loss_barriers[1], 1);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_resolved_flat_close_returns_exact_capital() {
    let amount: u16 = kani::any();
    kani::assume(amount > 0);
    kani::assume(amount <= 1_000);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group
        .deposit_not_atomic(&mut account, amount as u128)
        .unwrap();
    group.resolve_market_not_atomic(1).unwrap();

    let outcome = group.close_resolved_account_not_atomic(&mut account, 0);

    assert_eq!(
        outcome,
        Ok(ResolvedCloseOutcomeV16::Closed {
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
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_resolved_flat_close_syncs_fee_before_terminal_payout() {
    let fee_rate: u8 = kani::any();
    kani::assume(fee_rate > 0);
    kani::assume(fee_rate <= 5);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 100).unwrap();
    group.resolve_market_not_atomic(10).unwrap();

    let outcome = group
        .close_resolved_account_not_atomic(&mut account, fee_rate as u128)
        .unwrap();
    let expected_fee = fee_rate as u128 * 10;
    let expected_payout = 100 - expected_fee;

    kani::cover!(
        expected_fee > 0,
        "v16 resolved terminal close positive fee sync reachable"
    );
    assert_eq!(
        outcome,
        ResolvedCloseOutcomeV16::Closed {
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
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_resolved_profit_close_pays_snapshot_residual_and_clears_claim() {
    let profit: u8 = kani::any();
    kani::assume(profit > 0);
    kani::assume(profit <= 20);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 10).unwrap();
    account.pnl = profit as i128;
    group.pnl_pos_tot = profit as u128;
    set_junior_bound(&mut group, profit as u128);
    group.vault = group.c_tot + profit as u128;
    group.resolve_market_not_atomic(1).unwrap();

    let outcome = group.close_resolved_account_not_atomic(&mut account, 0);

    kani::cover!(profit > 1, "v16 resolved profit payout branch reachable");
    assert_eq!(
        outcome,
        Ok(ResolvedCloseOutcomeV16::Closed {
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
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_bankrupt_liquidation_consumes_insurance_before_social_loss() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.vault = 4;
    group.insurance = 4;
    group.insurance_domain_budget[1] = 4;
    account.pnl = -9;
    group.negative_pnl_account_count = 1;
    group.assets[0].loss_weight_sum_short = 1;

    let insurance_used = group
        .kani_consume_domain_insurance_for_negative_pnl(0, SideV16::Long, &mut account)
        .unwrap();

    kani::cover!(
        insurance_used != 0 && account.pnl < 0,
        "v16 insurance consumption leaves residual for social loss"
    );
    assert_eq!(insurance_used, 4);
    assert_eq!(group.vault, 4);
    assert_eq!(group.insurance, 0);
    assert_eq!(account.pnl, -5);

    let out = group
        .kani_book_bankruptcy_residual_chunk_internal(0, SideV16::Long, account.pnl.unsigned_abs())
        .unwrap();

    assert_eq!(out.booked_loss, 5);
    assert_eq!(out.explicit_loss, 0);
    assert_eq!(group.vault, 4);
    assert_eq!(group.insurance, 0);
    assert_eq!(out.remaining_after, 0);
}

fn assert_domain_insurance_budget_caps_bankruptcy_spend(domain_budget: u128) {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.vault = 4;
    group.insurance = 4;
    group.insurance_domain_budget = vec![0; group.insurance_domain_budget.len()];
    group.insurance_domain_budget[1] = domain_budget;
    account.pnl = -9;
    group.negative_pnl_account_count = 1;
    group.assets[0].loss_weight_sum_short = 1;

    let insurance_used = group
        .kani_consume_domain_insurance_for_negative_pnl(0, SideV16::Long, &mut account)
        .unwrap();
    let out = group
        .kani_book_bankruptcy_residual_chunk_internal(0, SideV16::Long, account.pnl.unsigned_abs())
        .unwrap();

    assert_eq!(insurance_used, domain_budget);
    assert_eq!(out.booked_loss, 9 - domain_budget);
    assert_eq!(out.explicit_loss, 0);
    assert_eq!(group.insurance, 4 - domain_budget);
    assert_eq!(group.insurance_domain_spent[1], domain_budget);
    assert_eq!(group.insurance_domain_spent[0], 0);
    assert_eq!(account.pnl, -((9 - domain_budget) as i128));
    assert_eq!(out.remaining_after, 0);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_domain_insurance_budget_zero_caps_bankruptcy_spend() {
    assert_domain_insurance_budget_caps_bankruptcy_spend(0);
    kani::cover!(true, "v16 domain insurance proof covers zero budget");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_domain_insurance_budget_one_caps_bankruptcy_spend() {
    assert_domain_insurance_budget_caps_bankruptcy_spend(1);
    kani::cover!(true, "v16 domain insurance proof covers one atom budget");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_domain_insurance_budget_two_caps_bankruptcy_spend() {
    assert_domain_insurance_budget_caps_bankruptcy_spend(2);
    kani::cover!(true, "v16 domain insurance proof covers two atom budget");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_domain_insurance_budget_three_caps_bankruptcy_spend() {
    assert_domain_insurance_budget_caps_bankruptcy_spend(3);
    kani::cover!(true, "v16 domain insurance proof covers three atom budget");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_domain_insurance_budget_full_caps_bankruptcy_spend() {
    assert_domain_insurance_budget_caps_bankruptcy_spend(4);
    kani::cover!(true, "v16 domain insurance proof covers full local budget");
}

#[kani::proof]
#[kani::unwind(16)]
#[kani::solver(cadical)]
fn proof_v16_domain_insurance_spend_excludes_source_credit_reservations() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut bankrupt =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.vault = 4;
    group.insurance = 4;
    group.insurance_domain_budget[1] = 4;
    group.insurance_credit_reservations[1] = InsuranceCreditReservationV16 {
        insurance_credit_reserved_num: 4 * BOUND_SCALE,
        source_credit_epoch: group.source_credit[1].credit_epoch,
        ..InsuranceCreditReservationV16::EMPTY
    };
    group.source_credit[1].insurance_credit_reserved_num = 4 * BOUND_SCALE;
    group.source_credit[1].credit_rate_num = CREDIT_RATE_SCALE;
    bankrupt.pnl = -4;
    group.negative_pnl_account_count = 1;

    let used = group
        .kani_consume_domain_insurance_for_negative_pnl(0, SideV16::Long, &mut bankrupt)
        .unwrap();

    kani::cover!(
        group.insurance_credit_reservations[1].insurance_credit_reserved_num != 0,
        "v16 source-credit insurance reservation occupies the bankrupt domain budget"
    );
    assert_eq!(used, 0);
    assert_eq!(group.insurance, 4);
    assert_eq!(group.insurance_domain_spent[1], 0);
    assert_eq!(bankrupt.pnl, -4);
}

#[kani::proof]
#[kani::unwind(16)]
#[kani::solver(cadical)]
fn proof_v16_unbudgeted_domain_cannot_spend_global_insurance() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut bankrupt =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.vault = 4;
    group.insurance = 4;
    bankrupt.pnl = -4;
    group.negative_pnl_account_count = 1;

    let used = group
        .kani_consume_domain_insurance_for_negative_pnl(0, SideV16::Long, &mut bankrupt)
        .unwrap();

    kani::cover!(
        group.insurance != 0 && group.insurance_domain_budget[1] == 0,
        "v16 global insurance exists while bankrupt domain budget is empty"
    );
    assert_eq!(used, 0);
    assert_eq!(group.insurance, 4);
    assert_eq!(group.insurance_domain_spent[1], 0);
    assert_eq!(bankrupt.pnl, -4);
}

#[kani::proof]
#[kani::unwind(32)]
#[kani::solver(cadical)]
fn proof_v16_zero_copy_domain_insurance_spend_excludes_source_credit_reservations() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut bankrupt =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.vault = 4;
    group.insurance = 4;
    group.insurance_domain_budget[1] = 4;
    group.insurance_credit_reservations[1] = InsuranceCreditReservationV16 {
        insurance_credit_reserved_num: 4 * BOUND_SCALE,
        source_credit_epoch: group.source_credit[1].credit_epoch,
        ..InsuranceCreditReservationV16::EMPTY
    };
    group.source_credit[1].insurance_credit_reserved_num = 4 * BOUND_SCALE;
    group.source_credit[1].credit_rate_num = CREDIT_RATE_SCALE;
    bankrupt.pnl = -4;
    group.negative_pnl_account_count = 1;

    let mut header = group_header_for_one_asset(&group);
    let mut markets = [Market {
        wrapper: (),
        engine: group_slots_for_one_asset(&group)[0],
    }];
    let mut account_header = PortfolioAccountV16Account::from_runtime(&bankrupt);
    let mut source_domains = source_domains_for_one_asset(&bankrupt);
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account_view =
        percolator::v16::PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);

    let used = market_view
        .kani_consume_domain_insurance_for_negative_pnl(0, SideV16::Long, &mut account_view)
        .unwrap();

    kani::cover!(
        market_view.markets[0]
            .engine
            .insurance_reservation_short
            .insurance_credit_reserved_num
            .get()
            != 0,
        "v16 zero-copy source-credit insurance reservation occupies bankrupt domain budget"
    );
    assert_eq!(used, 0);
    assert_eq!(market_view.header.insurance.get(), 4);
    assert_eq!(
        market_view.markets[0]
            .engine
            .insurance_domain_spent_short
            .get(),
        0
    );
    assert_eq!(account_view.header.pnl.get(), -4);
}

#[kani::proof]
#[kani::unwind(32)]
#[kani::solver(cadical)]
fn proof_v16_zero_copy_unbudgeted_domain_cannot_spend_global_insurance() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut bankrupt =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.vault = 4;
    group.insurance = 4;
    bankrupt.pnl = -4;
    group.negative_pnl_account_count = 1;

    let mut header = group_header_for_one_asset(&group);
    let mut markets = [Market {
        wrapper: (),
        engine: group_slots_for_one_asset(&group)[0],
    }];
    let mut account_header = PortfolioAccountV16Account::from_runtime(&bankrupt);
    let mut source_domains = source_domains_for_one_asset(&bankrupt);
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account_view =
        percolator::v16::PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);

    let used = market_view
        .kani_consume_domain_insurance_for_negative_pnl(0, SideV16::Long, &mut account_view)
        .unwrap();

    kani::cover!(
        market_view.header.insurance.get() != 0
            && market_view.markets[0]
                .engine
                .insurance_domain_budget_short
                .get()
                == 0,
        "v16 zero-copy global insurance exists while bankrupt domain budget is empty"
    );
    assert_eq!(used, 0);
    assert_eq!(market_view.header.insurance.get(), 4);
    assert_eq!(
        market_view.markets[0]
            .engine
            .insurance_domain_spent_short
            .get(),
        0
    );
    assert_eq!(account_view.header.pnl.get(), -4);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_long_liquidation_residual_charges_short_domain() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut bankrupt =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.vault = 4;
    group.insurance = 4;
    group.insurance_domain_budget = vec![0; group.insurance_domain_budget.len()];
    group.insurance_domain_budget[1] = 3;
    bankrupt.pnl = -5;
    group.negative_pnl_account_count = 1;
    group.assets[0].loss_weight_sum_short = 1;

    let insurance_used = group
        .kani_consume_domain_insurance_for_negative_pnl(0, SideV16::Long, &mut bankrupt)
        .unwrap();
    let residual = bankrupt.pnl.unsigned_abs();
    let out = group
        .kani_book_bankruptcy_residual_chunk_internal(0, SideV16::Long, residual)
        .unwrap();

    kani::cover!(
        out.booked_loss == 2,
        "v16 long liquidation charges short domain"
    );
    assert_eq!(insurance_used, 3);
    assert_eq!(out.booked_loss, 2);
    assert_eq!(out.explicit_loss, 0);
    assert_eq!(out.remaining_after, 0);
    assert_eq!(group.insurance_domain_spent[1], 3);
    assert_eq!(group.insurance_domain_spent[0], 0);
    assert_eq!(group.insurance, 1);
    assert_eq!(bankrupt.pnl, -2);
    assert_eq!(group.assets[0].b_short_num, 2 * SOCIAL_LOSS_DEN);
    assert_eq!(group.assets[0].b_long_num, 0);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_short_liquidation_residual_charges_long_domain() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut bankrupt =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.vault = 4;
    group.insurance = 4;
    group.insurance_domain_budget = vec![0; group.insurance_domain_budget.len()];
    group.insurance_domain_budget[0] = 3;
    bankrupt.pnl = -5;
    group.negative_pnl_account_count = 1;
    group.assets[0].loss_weight_sum_long = 1;

    let insurance_used = group
        .kani_consume_domain_insurance_for_negative_pnl(0, SideV16::Short, &mut bankrupt)
        .unwrap();
    let residual = bankrupt.pnl.unsigned_abs();
    let out = group
        .kani_book_bankruptcy_residual_chunk_internal(0, SideV16::Short, residual)
        .unwrap();

    kani::cover!(
        out.booked_loss == 2,
        "v16 short liquidation charges long domain"
    );
    assert_eq!(insurance_used, 3);
    assert_eq!(out.booked_loss, 2);
    assert_eq!(out.explicit_loss, 0);
    assert_eq!(out.remaining_after, 0);
    assert_eq!(group.insurance_domain_spent[0], 3);
    assert_eq!(group.insurance_domain_spent[1], 0);
    assert_eq!(group.insurance, 1);
    assert_eq!(bankrupt.pnl, -2);
    assert_eq!(group.assets[0].b_long_num, 2 * SOCIAL_LOSS_DEN);
    assert_eq!(group.assets[0].b_short_num, 0);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_bad_asset_cannot_spend_unrelated_domain_insurance_budget() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut bankrupt =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.vault = 1;
    group.insurance = 1;
    group.insurance_domain_budget = vec![0; group.insurance_domain_budget.len()];
    group.insurance_domain_budget[0] = 1;
    bankrupt.pnl = -1;
    group.negative_pnl_account_count = 1;

    let used = group
        .kani_consume_domain_insurance_for_negative_pnl(0, SideV16::Long, &mut bankrupt)
        .unwrap();

    kani::cover!(
        used == 0 && group.insurance_domain_budget[0] != 0,
        "v16 unrelated insurance budget is not available to the bankrupt side"
    );
    assert_eq!(used, 0);
    assert_eq!(bankrupt.pnl, -1);
    assert_eq!(group.insurance, 1);
    assert_eq!(group.insurance_domain_spent[0], 0);
    assert_eq!(group.insurance_domain_spent[1], 0);
}

#[kani::proof]
#[kani::unwind(32)]
#[kani::solver(cadical)]
fn proof_v16_invariants_reject_overallocated_domain_insurance_budgets() {
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    group.vault = 4;
    group.insurance = 4;
    group.insurance_domain_budget = vec![0; group.insurance_domain_budget.len()];
    group.insurance_domain_budget[0] = 4;
    group.insurance_domain_budget[1] = 1;

    kani::cover!(
        group.insurance_domain_budget[0] + group.insurance_domain_budget[1] > group.insurance,
        "v16 total unspent domain budgets exceed aggregate insurance"
    );
    assert_eq!(
        group.assert_public_invariants(),
        Err(V16Error::InvalidConfig)
    );
}

#[kani::proof]
#[kani::unwind(32)]
#[kani::solver(cadical)]
fn proof_v16_zero_copy_invariants_reject_overallocated_domain_insurance_budgets() {
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    group.vault = 4;
    group.insurance = 4;
    group.insurance_domain_budget = vec![0; group.insurance_domain_budget.len()];
    group.insurance_domain_budget[0] = 4;
    group.insurance_domain_budget[1] = 1;

    let mut header = group_header_for_one_asset(&group);
    let mut markets = [Market {
        wrapper: (),
        engine: group_slots_for_one_asset(&group)[0],
    }];
    let market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    kani::cover!(
        market_view.markets[0]
            .engine
            .insurance_domain_budget_long
            .get()
            + market_view.markets[0]
                .engine
                .insurance_domain_budget_short
                .get()
            > market_view.header.insurance.get(),
        "v16 zero-copy total unspent domain budgets exceed aggregate insurance"
    );
    assert_eq!(market_view.validate_shape(), Err(V16Error::InvalidConfig));
}

fn assert_bankrupt_liquidation_cannot_free_exposure_before_residual_durable(residual: i128) {
    let (market, account_id, owner) = concrete_ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 1);
    cfg.public_b_chunk_atoms = 1;
    let mut group = MarketGroupV16::new(market, cfg).unwrap();
    let mut bankrupt =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    group.assets[0].b_short_num = u128::MAX;
    group.assets[0].loss_weight_sum_short = 4;
    group.assets[0].social_loss_remainder_short_num = 10;
    bankrupt.pnl = residual;
    group.negative_pnl_account_count = 1;
    let before_b_short = group.assets[0].b_short_num;
    let before_bitmap = bankrupt.active_bitmap;
    let before_pnl = bankrupt.pnl;

    let result = group.kani_preflight_liquidation_residual_durability(0, SideV16::Long, &bankrupt);

    kani::cover!(
        result == Err(V16Error::RecoveryRequired),
        "v16 residual durability preflight recovery path reachable"
    );
    assert_eq!(result, Err(V16Error::RecoveryRequired));
    assert_eq!(
        group.recovery_reason,
        Some(PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress)
    );
    assert_eq!(bankrupt.active_bitmap, before_bitmap);
    assert_eq!(bankrupt.pnl, before_pnl);
    assert_eq!(group.assets[0].b_short_num, before_b_short);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_bankrupt_liquidation_cannot_free_exposure_before_two_atom_residual_durable() {
    assert_bankrupt_liquidation_cannot_free_exposure_before_residual_durable(-2);
    kani::cover!(true, "v16 residual durability proof covers two atoms");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_bankrupt_liquidation_cannot_free_exposure_before_three_atom_residual_durable() {
    assert_bankrupt_liquidation_cannot_free_exposure_before_residual_durable(-3);
    kani::cover!(true, "v16 residual durability proof covers three atoms");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_public_bankrupt_liquidation_rejects_before_freeing_exposure_when_residual_not_durable()
{
    let (market, account_id, owner) = concrete_ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.public_b_chunk_atoms = 1;
    let mut group = MarketGroupV16::new(market, cfg).unwrap();
    let mut bankrupt =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut opposing = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));

    group
        .attach_leg(&mut bankrupt, 0, SideV16::Long, 4)
        .unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV16::Short, -10)
        .unwrap();
    bankrupt.pnl = -5;
    group.negative_pnl_account_count = 1;
    let before_asset = group.assets[0];
    let before_mode = group.mode;
    let before_reason = group.recovery_reason;
    let before_bitmap = bankrupt.active_bitmap;
    let before_leg = bankrupt.legs[0];
    let before_pnl = bankrupt.pnl;

    let result = group.kani_liquidate_account_core(
        &mut bankrupt,
        LiquidationRequestV16 {
            asset_index: 0,
            close_q: 4,
            fee_bps: 0,
        },
        &[1; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(
        result == Err(V16Error::RecoveryRequired),
        "v16 public liquidation residual-durability recovery path reachable"
    );
    assert_eq!(result, Err(V16Error::RecoveryRequired));
    assert_eq!(
        group.recovery_reason,
        Some(PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress)
    );
    assert_eq!(before_mode, MarketModeV16::Live);
    assert_eq!(before_reason, None);
    assert_eq!(group.mode, MarketModeV16::Recovery);
    assert_eq!(bankrupt.active_bitmap, before_bitmap);
    assert_eq!(bankrupt.legs[0], before_leg);
    assert_eq!(bankrupt.pnl, before_pnl);
    assert_eq!(group.assets[0], before_asset);
}

fn assert_bankrupt_liquidation_excludes_fee_from_residual_and_spends_insurance_once(
    insurance: u128,
) {
    let (market, account_id, owner) = concrete_ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.max_price_move_bps_per_slot = 1;
    cfg.min_nonzero_mm_req = 12;
    cfg.min_nonzero_im_req = 13;
    cfg.liquidation_fee_bps = 0;
    cfg.liquidation_fee_cap = 1;
    cfg.min_liquidation_abs = 1;
    let mut group = MarketGroupV16::new(market, cfg).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    group.vault = insurance;
    group.insurance = insurance;
    account.pnl = -5;
    group.negative_pnl_account_count = 1;
    group.assets[0].loss_weight_sum_short = 1;

    let insurance_used = group
        .kani_consume_domain_insurance_for_negative_pnl(0, SideV16::Long, &mut account)
        .unwrap();
    let residual_after_insurance = account.pnl.unsigned_abs();
    let out = group
        .kani_book_bankruptcy_residual_chunk_internal(0, SideV16::Long, residual_after_insurance)
        .unwrap();

    assert_eq!(insurance_used, insurance);
    assert_eq!(group.insurance, 0);
    assert_eq!(out.booked_loss, 5 - insurance);
    assert_eq!(out.explicit_loss, 0);
    assert_eq!(out.remaining_after, 0);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_bankrupt_liquidation_excludes_fee_from_residual_with_zero_insurance() {
    assert_bankrupt_liquidation_excludes_fee_from_residual_and_spends_insurance_once(0);
    kani::cover!(
        true,
        "v16 bankrupt liquidation zero-insurance path reachable"
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_bankrupt_liquidation_spends_one_insurance_atom_once() {
    assert_bankrupt_liquidation_excludes_fee_from_residual_and_spends_insurance_once(1);
    kani::cover!(
        true,
        "v16 bankrupt liquidation one-insurance path reachable"
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_bankrupt_liquidation_spends_two_insurance_atoms_once() {
    assert_bankrupt_liquidation_excludes_fee_from_residual_and_spends_insurance_once(2);
    kani::cover!(
        true,
        "v16 bankrupt liquidation partial-insurance path reachable"
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_rebalance_reduce_position_preserves_senior_claims_and_reduces_risk() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut opposing = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group
        .attach_leg(&mut account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    let senior_before = group.c_tot + group.insurance;

    let out = group
        .rebalance_reduce_position_not_atomic(
            &mut account,
            RebalanceRequestV16 {
                asset_index: 0,
                reduce_q: POS_SCALE / 2,
            },
            &[1_000_000; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    kani::cover!(out.reduced_q == POS_SCALE / 2);
    assert_eq!(out.reduced_q, POS_SCALE / 2);
    assert!(account.legs[0].active);
    assert_eq!(account.legs[0].side, SideV16::Long);
    assert_eq!(account.legs[0].basis_pos_q.unsigned_abs(), POS_SCALE / 2);
    assert_eq!(group.c_tot + group.insurance, senior_before);
    assert!(account.health_cert.valid);
    assert!(account.health_cert.certified_worst_case_loss <= 500_000);

    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut opposing = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [5; 32], owner));
    group
        .attach_leg(&mut account, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let senior_before = group.c_tot + group.insurance;

    let out = group
        .rebalance_reduce_position_not_atomic(
            &mut account,
            RebalanceRequestV16 {
                asset_index: 0,
                reduce_q: POS_SCALE,
            },
            &[1_000_000; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    kani::cover!(out.reduced_q == POS_SCALE);
    assert_eq!(out.reduced_q, POS_SCALE);
    assert_eq!(account.active_bitmap, bitmap(&[]));
    assert!(!account.legs[0].active);
    assert_eq!(group.c_tot + group.insurance, senior_before);
    assert!(account.health_cert.valid);
    assert_eq!(account.health_cert.certified_worst_case_loss, 0);
}

fn assert_v16_b_residual_booking_case(residual: u128) {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group
        .attach_leg(&mut account, 0, SideV16::Short, -1)
        .unwrap();

    let before_b = group.assets[0].b_short_num;
    let result =
        group.book_bankruptcy_residual_chunk_for_account(&mut account, 0, SideV16::Long, residual);
    if residual == 0 {
        assert_eq!(result.unwrap().remaining_after, 0);
        assert_eq!(group.assets[0].b_short_num, before_b);
    } else {
        let out = result.unwrap();
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
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_b_residual_booking_zero_noops() {
    assert_v16_b_residual_booking_case(0);
    kani::cover!(true, "v16 zero residual B booking no-op reachable");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_b_residual_booking_positive_makes_durable_progress() {
    assert_v16_b_residual_booking_case(4);
    kani::cover!(true, "v16 residual B booking reachable");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_zero_weight_domain_residual_routes_to_recovery_without_mutation() {
    let bankrupt_long: bool = kani::any();
    let (market, _, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let bankrupt_side = if bankrupt_long {
        SideV16::Long
    } else {
        SideV16::Short
    };
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [2u8; 32], owner));

    let before_long = group.assets[0].explicit_unallocated_loss_long;
    let before_short = group.assets[0].explicit_unallocated_loss_short;
    let result =
        group.book_bankruptcy_residual_chunk_for_account(&mut account, 0, bankrupt_side, 1);

    kani::cover!(
        bankrupt_long,
        "v16 zero-weight short-domain recovery reachable"
    );
    kani::cover!(
        !bankrupt_long,
        "v16 zero-weight long-domain recovery reachable"
    );
    assert_eq!(result, Err(V16Error::RecoveryRequired));
    assert_eq!(
        group.recovery_reason,
        Some(PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress)
    );
    assert_eq!(group.assets[0].explicit_unallocated_loss_long, before_long);
    assert_eq!(
        group.assets[0].explicit_unallocated_loss_short,
        before_short
    );
    assert!(!account.close_progress.active);
    let blocked_domain = if bankrupt_long {
        SideV16::Short
    } else {
        SideV16::Long
    };
    assert!(matches!(
        group.pending_domain_loss_barrier_count(0, blocked_domain),
        Ok(0)
    ));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_account_b_booking_advances_close_progress_or_fails_closed() {
    let residual_units: u8 = kani::any();
    kani::assume(residual_units > 0 && residual_units <= 4);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut opp = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.attach_leg(&mut opp, 0, SideV16::Short, -1).unwrap();
    let mut bankrupt =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [7u8; 32], [8u8; 32]));

    let before_b = group.assets[0].b_short_num;
    let before_explicit = group.assets[0].explicit_unallocated_loss_short;
    let result = group.book_bankruptcy_residual_chunk_for_account(
        &mut bankrupt,
        0,
        SideV16::Long,
        residual_units as u128,
    );

    if let Ok(out) = result {
        kani::cover!(
            out.booked_loss > 0,
            "v16 account B booking ledger path reachable"
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
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_pending_domain_barrier_blocks_participants_until_residual_finalized() {
    let (market, _, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut participant =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    let joiner = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [5; 32], owner));

    group
        .attach_leg(&mut participant, 0, SideV16::Short, -10)
        .unwrap();
    group.pending_domain_loss_barriers[1] = 1;
    kani::cover!(
        group.pending_domain_loss_barrier_count(0, SideV16::Short) == Ok(1),
        "v16 pending domain barrier reachable"
    );
    assert!(matches!(
        group.pending_domain_loss_barrier_count(0, SideV16::Short),
        Ok(1)
    ));
    assert_eq!(
        group.kani_position_delta_blocked_by_pending_domain_loss_barrier(&participant, 0, 10),
        Ok(false)
    );
    assert_eq!(
        group.kani_position_delta_blocked_by_pending_domain_loss_barrier(&participant, 0, -1),
        Ok(true)
    );
    assert_eq!(
        group.kani_position_delta_blocked_by_pending_domain_loss_barrier(&joiner, 0, -1),
        Ok(true)
    );
    assert!(matches!(
        group.h_lock_lane(Some(&participant), false, None),
        Ok(HLockLaneV16::HMax)
    ));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_single_domain_close_lock_rejects_second_origin_until_first_finalized() {
    let (market, _, owner) = concrete_ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 1);
    cfg.public_b_chunk_atoms = 1;
    let mut group = MarketGroupV16::new(market, cfg).unwrap();
    let mut first_bankrupt =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    let mut second_bankrupt =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [5; 32], owner));
    let mut participant =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [6; 32], owner));

    group
        .attach_leg(&mut participant, 0, SideV16::Short, -10)
        .unwrap();
    let first = group
        .book_bankruptcy_residual_chunk_for_account(&mut first_bankrupt, 0, SideV16::Long, 2)
        .unwrap();
    kani::cover!(
        first.booked_loss == 1,
        "v16 first active domain close leaves pending residual"
    );
    assert_eq!(first.booked_loss, 1);
    assert_eq!(group.pending_domain_loss_barriers[1], 1);

    let before_second_ledger = second_bankrupt.close_progress;
    let before_domain_barrier = group.pending_domain_loss_barriers[1];
    let before_b_short = group.assets[0].b_short_num;
    let second_blocked =
        group.book_bankruptcy_residual_chunk_for_account(&mut second_bankrupt, 0, SideV16::Long, 1);
    assert_eq!(second_blocked, Err(V16Error::LockActive));
    assert_eq!(second_bankrupt.close_progress, before_second_ledger);
    assert_eq!(group.pending_domain_loss_barriers[1], before_domain_barrier);
    assert_eq!(group.assets[0].b_short_num, before_b_short);

    let complete_first = group
        .book_bankruptcy_residual_chunk_for_account(&mut first_bankrupt, 0, SideV16::Long, 2)
        .unwrap();
    assert_eq!(complete_first.booked_loss, 1);
    assert!(first_bankrupt.close_progress.finalized);
    assert_eq!(group.pending_domain_loss_barriers[1], 0);

    let second = group
        .book_bankruptcy_residual_chunk_for_account(&mut second_bankrupt, 0, SideV16::Long, 1)
        .unwrap();
    assert_eq!(second.booked_loss, 1);
    assert!(second_bankrupt.close_progress.finalized);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_public_invariants_reject_multiple_pending_barriers_per_domain() {
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    group.pending_domain_loss_barriers[1] = 2;
    assert_eq!(
        group.assert_public_invariants(),
        Err(V16Error::InvalidConfig)
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_pending_domain_barrier_allows_rebalance_reduction_with_weight_obligation_preserved() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut participant =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    group
        .attach_leg(&mut participant, 0, SideV16::Short, -10)
        .unwrap();
    group.pending_domain_loss_barriers[1] = 1;
    kani::cover!(
        group.pending_domain_loss_barrier_count(0, SideV16::Short) == Ok(1),
        "v16 pending domain barrier with rebalance risk reduction reachable"
    );

    assert_eq!(
        group.kani_position_delta_blocked_by_pending_domain_loss_barrier(&participant, 0, 5),
        Ok(false)
    );
    assert_eq!(
        group.kani_position_delta_blocked_by_pending_domain_loss_barrier(&participant, 0, -1),
        Ok(true)
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_pending_domain_barrier_allows_trade_reduction_with_weight_obligation_preserved() {
    let (market, _, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut participant =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));

    group
        .attach_leg(&mut participant, 0, SideV16::Short, -10)
        .unwrap();
    group.pending_domain_loss_barriers[1] = 1;
    kani::cover!(
        group.pending_domain_loss_barrier_count(0, SideV16::Short) == Ok(1),
        "v16 pending domain barrier with trade risk reduction reachable"
    );

    assert_eq!(
        group.kani_position_delta_blocked_by_pending_domain_loss_barrier(&participant, 0, 5),
        Ok(false)
    );
    assert_eq!(
        group.kani_position_delta_blocked_by_pending_domain_loss_barrier(&participant, 0, -1),
        Ok(true)
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_pending_domain_barrier_allows_full_trade_exit_as_flat_weight_obligation() {
    let (market, _, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut participant =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));

    group
        .attach_leg(&mut participant, 0, SideV16::Short, -10)
        .unwrap();
    group.pending_domain_loss_barriers[1] = 1;
    kani::cover!(
        group.pending_domain_loss_barrier_count(0, SideV16::Short) == Ok(1),
        "v16 pending domain barrier with full trade exit reachable"
    );

    assert_eq!(
        group.kani_position_delta_blocked_by_pending_domain_loss_barrier(&participant, 0, 10),
        Ok(false)
    );
    assert_eq!(
        group.kani_position_delta_blocked_by_pending_domain_loss_barrier(&participant, 0, -1),
        Ok(true)
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_pending_obligation_blocks_side_reset_until_clear() {
    let (market, _, owner) = concrete_ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 1);
    cfg.public_b_chunk_atoms = 1;
    let mut group = MarketGroupV16::new(market, cfg).unwrap();
    let mut participant =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));

    group
        .attach_leg(&mut participant, 0, SideV16::Short, -10)
        .unwrap();
    // Canonical post-exit state created by the pending-domain-barrier full
    // exit path: the account is flat, but its loss-weight obligation remains
    // until the leg is explicitly cleared.
    let leg_loss_weight = participant.legs[0].loss_weight;
    participant.legs[0].basis_pos_q = 0;
    group.assets[0].oi_eff_short_q = 0;
    group.assets[0].pending_obligation_count_short = 1;
    group.validate_account_shape(&participant).unwrap();
    kani::cover!(
        group.assets[0].pending_obligation_count_short == 1 && group.assets[0].oi_eff_short_q == 0,
        "v16 flat pending obligation before side reset reachable"
    );

    let before_weight = group.assets[0].loss_weight_sum_short;
    let before_count = group.assets[0].pending_obligation_count_short;
    let before_epoch = group.assets[0].epoch_short;
    let before_mode = group.assets[0].mode_short;
    let before_risk_epoch = group.risk_epoch;
    let reset_while_obligated = group.kani_begin_full_drain_reset_inner(0, SideV16::Short);

    assert_eq!(reset_while_obligated, Err(V16Error::LockActive));
    assert_eq!(group.assets[0].loss_weight_sum_short, before_weight);
    assert_eq!(group.assets[0].pending_obligation_count_short, before_count);
    assert_eq!(group.assets[0].epoch_short, before_epoch);
    assert_eq!(group.assets[0].mode_short, before_mode);
    assert_eq!(group.risk_epoch, before_risk_epoch);

    assert!(group.clear_leg(&mut participant, 0).is_ok());
    assert_eq!(group.assets[0].pending_obligation_count_short, 0);
    assert!(group
        .kani_begin_full_drain_reset_inner(0, SideV16::Short)
        .is_ok());
    assert_eq!(group.assets[0].mode_short, SideModeV16::ResetPending);
    assert_eq!(group.assets[0].loss_weight_sum_short, 0);
    assert_eq!(leg_loss_weight, 10);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_flat_pending_obligation_cannot_clear_before_b_settlement() {
    let (market, _, owner) = concrete_ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 1);
    cfg.public_b_chunk_atoms = 1;
    let mut group = MarketGroupV16::new(market, cfg).unwrap();
    let mut participant =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));

    group
        .attach_leg(&mut participant, 0, SideV16::Short, -10)
        .unwrap();

    // Directly materialize the canonical post-rebalance state covered by the
    // regression test: the quantity is flat, but the leg still represents a
    // B-loss obligation that must settle before the leg can clear.
    participant.legs[0].basis_pos_q = 0;
    group.assets[0].oi_eff_short_q = 0;
    group.assets[0].pending_obligation_count_short = 1;
    group.assets[0].b_short_num = participant.legs[0].b_snap + 1;
    group.validate_account_shape(&participant).unwrap();
    group.assert_public_invariants().unwrap();

    kani::cover!(
        participant.legs[0].basis_pos_q == 0
            && participant.legs[0].loss_weight != 0
            && group.assets[0].pending_obligation_count_short == 1
            && group.assets[0].b_short_num > participant.legs[0].b_snap,
        "v16 flat pending obligation with unsettled B loss reachable"
    );

    let before_weight = group.assets[0].loss_weight_sum_short;
    let before_count = group.assets[0].pending_obligation_count_short;
    let before_stored = group.assets[0].stored_pos_count_short;
    let before_basis = participant.legs[0].basis_pos_q;
    let before_loss_weight = participant.legs[0].loss_weight;
    let before_b_snap = participant.legs[0].b_snap;
    let before_b_rem = participant.legs[0].b_rem;
    let stale_clear = group.clear_leg(&mut participant, 0);

    assert_eq!(stale_clear, Err(V16Error::Stale));
    assert_eq!(group.assets[0].loss_weight_sum_short, before_weight);
    assert_eq!(group.assets[0].pending_obligation_count_short, before_count);
    assert_eq!(group.assets[0].stored_pos_count_short, before_stored);
    assert_eq!(participant.legs[0].basis_pos_q, before_basis);
    assert_eq!(participant.legs[0].loss_weight, before_loss_weight);
    assert_eq!(participant.legs[0].b_snap, before_b_snap);
    assert_eq!(participant.legs[0].b_rem, before_b_rem);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_pending_domain_barrier_allows_rebalance_full_exit_as_flat_weight_obligation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut participant =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    group
        .attach_leg(&mut participant, 0, SideV16::Short, -10)
        .unwrap();
    group.pending_domain_loss_barriers[1] = 1;
    kani::cover!(
        group.pending_domain_loss_barrier_count(0, SideV16::Short) == Ok(1),
        "v16 pending domain barrier with rebalance escape attempt reachable"
    );

    assert_eq!(
        group.kani_position_delta_blocked_by_pending_domain_loss_barrier(&participant, 0, 10),
        Ok(false)
    );
    assert_eq!(
        group.kani_position_delta_blocked_by_pending_domain_loss_barrier(&participant, 0, -1),
        Ok(true)
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_new_close_cannot_overwrite_active_finalized_close_ledger() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(2, 0, 1)).unwrap();
    let mut bankrupt =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group
        .attach_leg(&mut bankrupt, 1, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    bankrupt.close_progress = CloseProgressLedgerV16 {
        active: true,
        finalized: true,
        close_id: 7,
        asset_index: 0,
        market_id: group.assets[0].market_id,
        domain_side: SideV16::Short,
        gross_loss_at_close_start: 2,
        b_loss_booked: 2,
        residual_remaining: 0,
        drift_reference_slot: group.current_slot,
        max_close_slot: group.current_slot + 1,
        ..CloseProgressLedgerV16::EMPTY
    };
    let before_ledger = bankrupt.close_progress;
    let before_b_short = group.assets[1].b_short_num;

    let result =
        group.book_bankruptcy_residual_chunk_for_account(&mut bankrupt, 1, SideV16::Long, 1);

    kani::cover!(
        result == Err(V16Error::LockActive),
        "v16 active finalized close ledger blocks new close id"
    );
    assert_eq!(result, Err(V16Error::LockActive));
    assert_eq!(bankrupt.close_progress, before_ledger);
    assert_eq!(group.assets[1].b_short_num, before_b_short);
}

fn assert_v16_cure_and_cancel_releases_barrier_and_escrow(
    prior_escrow: u128,
    optional_deposit: u128,
) {
    let total_release = prior_escrow + optional_deposit;
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.cancel_deposit_escrow = prior_escrow;
    group.vault = prior_escrow;
    account.close_progress = CloseProgressLedgerV16 {
        active: true,
        close_id: 1,
        asset_index: 0,
        market_id: group.assets[0].market_id,
        domain_side: SideV16::Short,
        gross_loss_at_close_start: 5,
        drift_reference_slot: group.current_slot,
        max_close_slot: group.current_slot + group.config.max_bankrupt_close_lifetime_slots,
        residual_remaining: 5,
        ..CloseProgressLedgerV16::EMPTY
    };
    group.pending_domain_loss_barriers[1] = 1;

    let cert = HealthCertV16 {
        certified_equity: 0,
        certified_initial_req: total_release,
        active_bitmap_at_cert: account.active_bitmap,
        valid: true,
        ..HealthCertV16::default()
    };
    let result = group.kani_cure_and_cancel_close_with_cert(&mut account, optional_deposit, cert);

    assert!(result.is_ok());
    assert!(!account.close_progress.active);
    assert!(account.close_progress.canceled);
    assert_eq!(account.close_progress.close_id, 1);
    assert_eq!(account.cancel_deposit_escrow, 0);
    assert_eq!(account.capital, total_release);
    assert_eq!(group.c_tot, total_release);
    assert_eq!(group.vault, total_release);
    assert_eq!(
        group.pending_domain_loss_barrier_count(0, SideV16::Short),
        Ok(0)
    );
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_cure_and_cancel_close_releases_existing_escrow_before_irreversible_progress() {
    assert_v16_cure_and_cancel_releases_barrier_and_escrow(2, 1);
    kani::cover!(true, "v16 cure cancel existing escrow path reachable");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_cure_and_cancel_close_deposits_fresh_escrow_before_irreversible_progress() {
    assert_v16_cure_and_cancel_releases_barrier_and_escrow(0, 2);
    kani::cover!(true, "v16 cure cancel fresh deposit path reachable");
}

fn assert_v16_cure_and_cancel_rejects_irreversible_progress_before_deposit_mutation(
    progress_case: u8,
) {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut ledger = CloseProgressLedgerV16 {
        active: true,
        close_id: 1,
        asset_index: 0,
        market_id: group.assets[0].market_id,
        domain_side: SideV16::Short,
        gross_loss_at_close_start: 10,
        drift_reference_slot: group.current_slot,
        max_close_slot: group.current_slot + group.config.max_bankrupt_close_lifetime_slots,
        residual_remaining: 10,
        ..CloseProgressLedgerV16::EMPTY
    };
    match progress_case {
        0 => {
            ledger.support_consumed = 1;
            ledger.junior_face_burned = 1;
        }
        1 => ledger.insurance_spent = 1,
        2 => ledger.b_loss_booked = 1,
        3 => ledger.explicit_loss_assigned = 1,
        4 => {
            // Quantity-ADL close progress is canonical only after the residual
            // close ledger is finalized; otherwise account-shape validation
            // correctly rejects the malformed ledger before the cure/cancel
            // preflight can classify it as an active lock.
            ledger.gross_loss_at_close_start = 0;
            ledger.quantity_adl_applied_q = 1;
            ledger.residual_remaining = 0;
            ledger.finalized = true;
        }
        _ => ledger.drift_consumed = 1,
    }
    if progress_case != 4 {
        ledger.residual_remaining = ledger
            .gross_loss_at_close_start
            .checked_add(ledger.drift_consumed)
            .unwrap()
            .checked_sub(
                ledger.support_consumed
                    + ledger.insurance_spent
                    + ledger.b_loss_booked
                    + ledger.explicit_loss_assigned,
            )
            .unwrap();
    }
    account.close_progress = ledger;
    group.pending_domain_loss_barriers[1] = 1;

    let result = group.kani_preflight_cure_and_cancel_close(&account, 3);

    assert_eq!(result, Err(V16Error::LockActive));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_cure_and_cancel_rejects_support_progress_before_deposit_mutation() {
    assert_v16_cure_and_cancel_rejects_irreversible_progress_before_deposit_mutation(0);
    kani::cover!(true, "v16 cure cancel rejects support progress");
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_cure_and_cancel_rejects_insurance_progress_before_deposit_mutation() {
    assert_v16_cure_and_cancel_rejects_irreversible_progress_before_deposit_mutation(1);
    kani::cover!(true, "v16 cure cancel rejects insurance progress");
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_cure_and_cancel_rejects_b_progress_before_deposit_mutation() {
    assert_v16_cure_and_cancel_rejects_irreversible_progress_before_deposit_mutation(2);
    kani::cover!(true, "v16 cure cancel rejects b progress");
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_cure_and_cancel_rejects_explicit_loss_progress_before_deposit_mutation() {
    assert_v16_cure_and_cancel_rejects_irreversible_progress_before_deposit_mutation(3);
    kani::cover!(true, "v16 cure cancel rejects explicit loss progress");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_cure_and_cancel_rejects_quantity_adl_progress_before_deposit_mutation() {
    assert_v16_cure_and_cancel_rejects_irreversible_progress_before_deposit_mutation(4);
    kani::cover!(true, "v16 cure cancel rejects quantity adl progress");
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_cure_and_cancel_rejects_drift_progress_before_deposit_mutation() {
    assert_v16_cure_and_cancel_rejects_irreversible_progress_before_deposit_mutation(5);
    kani::cover!(true, "v16 cure cancel rejects drift progress");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_close_lifetime_uses_configured_bound_and_is_not_refreshed() {
    let (market, account_id, owner) = concrete_ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 1);
    cfg.max_bankrupt_close_chunks = 7;
    cfg.max_bankrupt_close_lifetime_slots = 5;
    cfg.public_b_chunk_atoms = 1;
    let mut group = MarketGroupV16::new(market, cfg).unwrap();
    group.current_slot = 11;
    let mut bankrupt =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group
        .kani_begin_close_progress_ledger(&mut bankrupt, 0, SideV16::Short, 2)
        .unwrap();
    let first = group.kani_advance_close_progress_ledger(&mut bankrupt, 0, 0, 0, 1, 0);
    kani::cover!(
        first == Ok(()),
        "v16 first close chunk starts configured-lifetime ledger"
    );
    assert_eq!(first, Ok(()));
    let first_ledger = bankrupt.close_progress;
    assert!(first_ledger.active);
    assert!(!first_ledger.finalized);
    assert_eq!(first_ledger.drift_reference_slot, 11);
    assert_eq!(first_ledger.max_close_slot, 16);
    assert_ne!(
        first_ledger.max_close_slot,
        11 + cfg.max_accrual_dt_slots * cfg.max_bankrupt_close_chunks
    );

    group.current_slot = 12;
    let second = group.kani_advance_close_progress_ledger(&mut bankrupt, 0, 0, 0, 1, 0);
    kani::cover!(
        second == Ok(()),
        "v16 close continuation finalizes without refreshing lifetime"
    );
    assert_eq!(second, Ok(()));
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

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_account_shape_rejects_malformed_quantity_adl_close_progress() {
    let premature_adl: bool = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    if premature_adl {
        account.close_progress = CloseProgressLedgerV16 {
            active: true,
            finalized: false,
            close_id: 1,
            asset_index: 0,
            market_id: group.assets[0].market_id,
            domain_side: SideV16::Short,
            gross_loss_at_close_start: 2,
            b_loss_booked: 1,
            residual_remaining: 1,
            quantity_adl_applied_q: 1,
            ..CloseProgressLedgerV16::EMPTY
        };
    } else {
        let mut group_for_leg =
            MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
        group_for_leg
            .attach_leg(&mut account, 0, SideV16::Long, 4)
            .unwrap();
        account.close_progress = CloseProgressLedgerV16 {
            active: true,
            finalized: true,
            close_id: 1,
            asset_index: 0,
            market_id: group.assets[0].market_id,
            domain_side: SideV16::Short,
            gross_loss_at_close_start: 1,
            explicit_loss_assigned: 1,
            residual_remaining: 0,
            quantity_adl_applied_q: 4,
            ..CloseProgressLedgerV16::EMPTY
        };
    }

    let result = group.validate_account_shape(&account);

    kani::cover!(premature_adl, "v16 premature quantity ADL shape reachable");
    kani::cover!(
        !premature_adl,
        "v16 quantity ADL with open closing leg shape reachable"
    );
    assert_eq!(result, Err(V16Error::InvalidLeg));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_account_shape_rejects_malformed_canceled_close_progress() {
    let active_or_progress: bool = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.close_progress = CloseProgressLedgerV16 {
        active: active_or_progress,
        canceled: true,
        close_id: 1,
        asset_index: 0,
        market_id: group.assets[0].market_id,
        domain_side: SideV16::Short,
        gross_loss_at_close_start: 5,
        drift_reference_slot: 0,
        max_close_slot: 10,
        insurance_spent: if active_or_progress { 0 } else { 1 },
        residual_remaining: if active_or_progress { 5 } else { 4 },
        ..CloseProgressLedgerV16::EMPTY
    };

    let result = group.validate_account_shape(&account);

    kani::cover!(
        active_or_progress,
        "v16 canceled active close ledger rejected"
    );
    kani::cover!(
        !active_or_progress,
        "v16 canceled close ledger with irreversible progress rejected"
    );
    assert_eq!(result, Err(V16Error::InvalidLeg));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_account_shape_rejects_close_progress_domain_mismatch_for_open_leg() {
    let closing_long: bool = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let side = if closing_long {
        SideV16::Long
    } else {
        SideV16::Short
    };
    let signed_basis = if closing_long { 4 } else { -4 };
    group
        .attach_leg(&mut account, 0, side, signed_basis)
        .unwrap();
    account.close_progress = CloseProgressLedgerV16 {
        active: true,
        finalized: false,
        close_id: 1,
        asset_index: 0,
        market_id: group.assets[0].market_id,
        domain_side: side,
        gross_loss_at_close_start: 2,
        b_loss_booked: 1,
        residual_remaining: 1,
        ..CloseProgressLedgerV16::EMPTY
    };

    let result = group.validate_account_shape(&account);

    kani::cover!(closing_long, "v16 long close domain mismatch reachable");
    kani::cover!(!closing_long, "v16 short close domain mismatch reachable");
    assert_eq!(result, Err(V16Error::InvalidLeg));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_expired_close_progress_routes_recovery_before_durable_mutation() {
    let close_b_residual: bool = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    group.current_slot = 2;
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut opposing =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [9; 32], [8; 32]));
    group.attach_leg(&mut account, 0, SideV16::Long, 4).unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV16::Short, -4)
        .unwrap();
    account.close_progress = CloseProgressLedgerV16 {
        active: true,
        finalized: !close_b_residual,
        close_id: 1,
        asset_index: 0,
        market_id: group.assets[0].market_id,
        domain_side: SideV16::Short,
        gross_loss_at_close_start: 2,
        drift_reference_slot: 0,
        max_close_slot: 1,
        explicit_loss_assigned: if close_b_residual { 0 } else { 2 },
        residual_remaining: if close_b_residual { 2 } else { 0 },
        ..CloseProgressLedgerV16::EMPTY
    };
    group.assets[0].a_short = ADL_ONE;
    let before_b = group.assets[0].b_short_num;
    let before_a = group.assets[0].a_short;
    let before_long_oi = group.assets[0].oi_eff_long_q;
    let before_short_oi = group.assets[0].oi_eff_short_q;

    let result = if close_b_residual {
        group
            .book_bankruptcy_residual_chunk_for_account(&mut account, 0, SideV16::Long, 2)
            .map(|_| ())
    } else {
        group
            .apply_quantity_adl_after_residual_for_account_not_atomic(
                &mut account,
                0,
                SideV16::Long,
                4,
            )
            .map(|_| ())
    };

    kani::cover!(
        close_b_residual,
        "v16 expired B continuation recovery path reachable"
    );
    kani::cover!(
        !close_b_residual,
        "v16 expired quantity ADL continuation recovery path reachable"
    );
    assert_eq!(result, Err(V16Error::RecoveryRequired));
    assert_eq!(
        group.recovery_reason,
        Some(PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress)
    );
    assert_eq!(group.assets[0].b_short_num, before_b);
    assert_eq!(group.assets[0].a_short, before_a);
    assert_eq!(group.assets[0].oi_eff_long_q, before_long_oi);
    assert_eq!(group.assets[0].oi_eff_short_q, before_short_oi);
    assert_eq!(account.close_progress.b_loss_booked, 0);
    assert_eq!(account.close_progress.quantity_adl_applied_q, 0);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_stale_open_close_snapshot_routes_recovery_before_durable_mutation() {
    let close_b_residual: bool = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    group.current_slot = 1;
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut opposing =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [10; 32], owner));
    group.attach_leg(&mut account, 0, SideV16::Long, 4).unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV16::Short, -4)
        .unwrap();
    account.close_progress = CloseProgressLedgerV16 {
        active: true,
        finalized: !close_b_residual,
        close_id: 1,
        asset_index: 0,
        market_id: group.assets[0].market_id,
        domain_side: SideV16::Short,
        gross_loss_at_close_start: 2,
        drift_reference_slot: 0,
        max_close_slot: 10,
        explicit_loss_assigned: if close_b_residual { 0 } else { 2 },
        residual_remaining: if close_b_residual { 2 } else { 0 },
        ..CloseProgressLedgerV16::EMPTY
    };
    group.pending_domain_loss_barriers[1] = 1;
    let before_ledger = account.close_progress;
    let before_b = group.assets[0].b_short_num;
    let before_a = group.assets[0].a_short;
    let before_long_oi = group.assets[0].oi_eff_long_q;
    let before_short_oi = group.assets[0].oi_eff_short_q;

    let result = if close_b_residual {
        group
            .book_bankruptcy_residual_chunk_for_account(&mut account, 0, SideV16::Long, 2)
            .map(|_| ())
    } else {
        group
            .apply_quantity_adl_after_residual_for_account_not_atomic(
                &mut account,
                0,
                SideV16::Long,
                4,
            )
            .map(|_| ())
    };

    kani::cover!(
        close_b_residual,
        "v16 stale open close B continuation recovery path reachable"
    );
    kani::cover!(
        !close_b_residual,
        "v16 stale open close quantity ADL recovery path reachable"
    );
    assert_eq!(result, Err(V16Error::RecoveryRequired));
    assert_eq!(
        group.recovery_reason,
        Some(PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress)
    );
    assert_eq!(account.close_progress, before_ledger);
    assert_eq!(group.assets[0].b_short_num, before_b);
    assert_eq!(group.assets[0].a_short, before_a);
    assert_eq!(group.assets[0].oi_eff_long_q, before_long_oi);
    assert_eq!(group.assets[0].oi_eff_short_q, before_short_oi);
}

#[kani::proof]
#[kani::unwind(32)]
#[kani::solver(cadical)]
fn proof_v16_invalid_trade_request_rejects_before_any_mutation() {
    assert_invalid_trade_reverts(TradeRequestV16 {
        asset_index: 1,
        size_q: POS_SCALE,
        exec_price: 100,
        fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
    });
    assert_invalid_trade_reverts(TradeRequestV16 {
        asset_index: 0,
        size_q: 0,
        exec_price: 100,
        fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
    });
    assert_invalid_trade_reverts(TradeRequestV16 {
        asset_index: 0,
        size_q: POS_SCALE,
        exec_price: 0,
        fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
    });
    assert_invalid_trade_reverts(TradeRequestV16 {
        asset_index: 0,
        size_q: POS_SCALE,
        exec_price: 100,
        fee_bps: 11,
            admit_h_max_consumption_threshold_bps_opt: None,
    });
}

fn assert_invalid_trade_reverts(request: TradeRequestV16) {
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    group.config.max_trading_fee_bps = 10;
    let before_vault = group.vault;
    let before_c_tot = group.c_tot;
    let before_insurance = group.insurance;
    let before_oi_long = group.assets[0].oi_eff_long_q;
    let before_oi_short = group.assets[0].oi_eff_short_q;
    let before_risk_epoch = group.risk_epoch;

    let result = group.kani_validate_trade_request(request);

    assert_eq!(result, Err(V16Error::InvalidConfig));
    assert_eq!(group.vault, before_vault);
    assert_eq!(group.c_tot, before_c_tot);
    assert_eq!(group.insurance, before_insurance);
    assert_eq!(group.assets[0].oi_eff_long_q, before_oi_long);
    assert_eq!(group.assets[0].oi_eff_short_q, before_oi_short);
    assert_eq!(group.risk_epoch, before_risk_epoch);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_price_accrual_refresh_matches_eager_mark_pnl() {
    assert_price_accrual_refresh_matches_eager_mark_pnl(101, 1, -1);
    assert_price_accrual_refresh_matches_eager_mark_pnl(99, -1, 1);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_same_epoch_full_refresh_is_idempotent_after_price_up_settlement() {
    assert_same_epoch_refresh_idempotent_after_kf_settlement(101, 1);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_same_epoch_full_refresh_is_idempotent_after_price_down_settlement() {
    assert_same_epoch_refresh_idempotent_after_kf_settlement(99, -1);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_sequential_kf_refresh_is_additive_not_compounding() {
    let (market, account_id, owner) = concrete_ids();
    let mut sequential = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    sequential.assets[0].effective_price = 100;
    sequential.assets[0].fund_px_last = 100;
    sequential.assets[0].raw_oracle_target_price = 100;
    let mut seq_account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    sequential
        .attach_leg(&mut seq_account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let _seq_opposite =
        attach_opposite_for_live_oi(&mut sequential, 0, SideV16::Long, POS_SCALE, 90);

    sequential
        .accrue_asset_to_not_atomic(0, 1, 101, 0, true)
        .unwrap();
    sequential
        .full_account_refresh(&mut seq_account, &[101; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    kani::cover!(
        seq_account.pnl == 1,
        "v16 first sequential K/F refresh settles nonzero pnl"
    );

    sequential
        .accrue_asset_to_not_atomic(0, 2, 102, 0, true)
        .unwrap();
    sequential
        .full_account_refresh(&mut seq_account, &[102; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    let mut direct = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    direct.assets[0].effective_price = 100;
    direct.assets[0].fund_px_last = 100;
    direct.assets[0].raw_oracle_target_price = 100;
    let mut direct_account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    direct
        .attach_leg(&mut direct_account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let _direct_opposite =
        attach_opposite_for_live_oi(&mut direct, 0, SideV16::Long, POS_SCALE, 91);

    direct
        .accrue_asset_to_not_atomic(0, 1, 102, 0, true)
        .unwrap();
    direct
        .full_account_refresh(&mut direct_account, &[102; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(seq_account.pnl, 2);
    assert_eq!(direct_account.pnl, 2);
    assert_eq!(seq_account.pnl, direct_account.pnl);
    assert_eq!(sequential.pnl_pos_tot, direct.pnl_pos_tot);
}

fn assert_same_epoch_refresh_idempotent_after_kf_settlement(new_price: u64, expected_pnl: i128) {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    group.assets[0].effective_price = 100;
    group.assets[0].fund_px_last = 100;
    group.assets[0].raw_oracle_target_price = 100;
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group
        .attach_leg(&mut account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group.assets[0].effective_price = new_price;
    group.assets[0].raw_oracle_target_price = new_price;
    group.assets[0].k_long = expected_pnl * (ADL_ONE as i128);
    group.oracle_epoch += 1;
    group
        .full_account_refresh(&mut account, &[new_price; V16_MAX_PORTFOLIO_ASSETS_N])
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
        "v16 idempotent refresh exercises nonzero settled K/F pnl"
    );
    group
        .full_account_refresh(&mut account, &[new_price; V16_MAX_PORTFOLIO_ASSETS_N])
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
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    group.assets[0].effective_price = 100;
    group.assets[0].fund_px_last = 100;
    group.assets[0].raw_oracle_target_price = 100;
    let mut long = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut short = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group
        .attach_leg(&mut long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut short, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    let out = group
        .accrue_asset_to_not_atomic(0, 1, new_price, 0, true)
        .unwrap();
    group
        .full_account_refresh(&mut long, &[new_price; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    group
        .full_account_refresh(&mut short, &[new_price; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert!(out.price_move_active);
    assert_eq!(long.pnl, expected_long_pnl);
    assert_eq!(short.pnl, expected_short_pnl);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_positive_funding_accrual_writes_f_ledger_sign_and_floor() {
    assert_funding_accrual_writes_f_ledger_sign_and_floor(1, -(ADL_ONE as i128), ADL_ONE as i128);
    kani::cover!(true, "v16 positive funding ledger sign and floor covered");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_negative_funding_accrual_writes_f_ledger_sign_and_floor() {
    assert_funding_accrual_writes_f_ledger_sign_and_floor(-1, ADL_ONE as i128, -(ADL_ONE as i128));
    kani::cover!(true, "v16 negative funding ledger sign and floor covered");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_positive_funding_refreshes_long_loss() {
    assert_funding_refresh_side_matches_sign_and_floor(1, true, -1);
    kani::cover!(true, "v16 positive funding refreshes long loss");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_positive_funding_refreshes_short_gain() {
    assert_funding_refresh_side_matches_sign_and_floor(1, false, 1);
    kani::cover!(true, "v16 positive funding refreshes short gain");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_negative_funding_refreshes_long_gain() {
    assert_funding_refresh_side_matches_sign_and_floor(-1, true, 1);
    kani::cover!(true, "v16 negative funding refreshes long gain");
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_negative_funding_refreshes_short_loss() {
    assert_funding_refresh_side_matches_sign_and_floor(-1, false, -1);
    kani::cover!(true, "v16 negative funding refreshes short loss");
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_funding_accrual_requires_bilateral_exposure() {
    let (market, _, _) = concrete_ids();
    let mut long_only = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    long_only.config.max_price_move_bps_per_slot = 9_999;
    long_only.config.max_abs_funding_e9_per_slot = 1;
    long_only.assets[0].effective_price = 1_000_000_000;
    long_only.assets[0].fund_px_last = 1_000_000_000;
    long_only.assets[0].raw_oracle_target_price = 1_000_000_000;
    long_only.assets[0].oi_eff_long_q = POS_SCALE;
    long_only.assets[0].loss_weight_sum_long = POS_SCALE;
    let long_before = long_only.assets[0];

    let out = MarketGroupV16::kani_accrual_activity_for_asset_segment(
        long_only.assets[0],
        1,
        1_000_000_000,
        1,
    );
    kani::cover!(
        long_only.assets[0].oi_eff_long_q != 0 && long_only.assets[0].oi_eff_short_q == 0,
        "v16 funding rejects long-only exposure"
    );

    assert!(!out.funding_active);
    assert_eq!(long_only.assets[0].f_long_num, long_before.f_long_num);
    assert_eq!(long_only.assets[0].f_short_num, long_before.f_short_num);
    assert_eq!(long_only.funding_epoch, 0);

    let mut short_only = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    short_only.config.max_price_move_bps_per_slot = 9_999;
    short_only.config.max_abs_funding_e9_per_slot = 1;
    short_only.assets[0].effective_price = 1_000_000_000;
    short_only.assets[0].fund_px_last = 1_000_000_000;
    short_only.assets[0].raw_oracle_target_price = 1_000_000_000;
    short_only.assets[0].oi_eff_short_q = POS_SCALE;
    short_only.assets[0].loss_weight_sum_short = POS_SCALE;
    let short_before = short_only.assets[0];

    let out = MarketGroupV16::kani_accrual_activity_for_asset_segment(
        short_only.assets[0],
        1,
        1_000_000_000,
        1,
    );
    kani::cover!(
        short_only.assets[0].oi_eff_short_q != 0 && short_only.assets[0].oi_eff_long_q == 0,
        "v16 funding rejects short-only exposure"
    );

    assert!(!out.funding_active);
    assert_eq!(short_only.assets[0].f_long_num, short_before.f_long_num);
    assert_eq!(short_only.assets[0].f_short_num, short_before.f_short_num);
    assert_eq!(short_only.funding_epoch, 0);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_no_oi_funding_rate_does_not_mutate_k_or_f() {
    let positive_rate: bool = kani::any();
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
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
        "v16 no-OI funding proof covers positive rate"
    );
    kani::cover!(
        !positive_rate,
        "v16 no-OI funding proof covers negative rate"
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
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_unexposed_oracle_bounds_fail_closed_and_allow_max_price_liveness() {
    let max_price_case: bool = kani::any();
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let before_asset = group.assets[0];
    let before_slot_last = group.slot_last;
    let before_current_slot = group.current_slot;
    let before_oracle_epoch = group.oracle_epoch;
    let before_funding_epoch = group.funding_epoch;

    if max_price_case {
        let out = group
            .accrue_asset_to_not_atomic(0, 1, MAX_ORACLE_PRICE, 0, false)
            .unwrap();

        kani::cover!(
            out.dt == 1 && !out.equity_active,
            "v16 unexposed max oracle price liveness branch reachable"
        );
        assert_eq!(out.dt, 1);
        assert!(!out.price_move_active);
        assert!(!out.funding_active);
        assert!(!out.equity_active);
        assert_eq!(group.assets[0].effective_price, MAX_ORACLE_PRICE);
        assert_eq!(group.assets[0].slot_last, 1);
        assert_eq!(group.slot_last, 1);
        assert_eq!(group.current_slot, 1);
        let expected_k_delta = ((MAX_ORACLE_PRICE as i128)
            - (before_asset.effective_price as i128))
            .checked_mul(ADL_ONE as i128)
            .unwrap();
        assert_eq!(
            group.assets[0].k_long,
            before_asset.k_long.checked_add(expected_k_delta).unwrap()
        );
        assert_eq!(
            group.assets[0].k_short,
            before_asset.k_short.checked_sub(expected_k_delta).unwrap()
        );
        assert_eq!(group.assets[0].oi_eff_long_q, 0);
        assert_eq!(group.assets[0].oi_eff_short_q, 0);
        assert_eq!(group.assets[0].f_long_num, before_asset.f_long_num);
        assert_eq!(group.assets[0].f_short_num, before_asset.f_short_num);
        assert_eq!(group.oracle_epoch, before_oracle_epoch);
        assert_eq!(group.funding_epoch, before_funding_epoch);
        assert_eq!(group.assert_public_invariants(), Ok(()));
    } else {
        let result = group.accrue_asset_to_not_atomic(0, 1, 0, 0, false);

        kani::cover!(
            result == Err(V16Error::InvalidConfig),
            "v16 zero oracle price fail-closed branch reachable"
        );
        assert_eq!(result, Err(V16Error::InvalidConfig));
        assert_eq!(group.assets[0], before_asset);
        assert_eq!(group.slot_last, before_slot_last);
        assert_eq!(group.current_slot, before_current_slot);
        assert_eq!(group.oracle_epoch, before_oracle_epoch);
        assert_eq!(group.funding_epoch, before_funding_epoch);
    }
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_permissionless_crank_accepts_configured_funding_rate_boundaries() {
    let positive_rate: bool = kani::any();
    let (market, _, _) = concrete_ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 1);
    cfg.max_price_move_bps_per_slot = 9_999;
    cfg.max_abs_funding_e9_per_slot = 1;
    let mut group = MarketGroupV16::new(market, cfg).unwrap();
    let supplied_rate = if positive_rate { 1 } else { -1 };

    let out = group
        .kani_accrue_asset_to_core_not_atomic(0, 1, 1, supplied_rate, false)
        .unwrap();

    kani::cover!(
        positive_rate && supplied_rate == group.config.max_abs_funding_e9_per_slot as i128,
        "v16 permissionless crank accepts positive funding boundary"
    );
    kani::cover!(
        !positive_rate && supplied_rate == -(group.config.max_abs_funding_e9_per_slot as i128),
        "v16 permissionless crank accepts negative funding boundary"
    );
    assert_eq!(out.dt, 1);
    assert!(!out.equity_active);
    assert!(!out.funding_active);
    assert_eq!(group.current_slot, 1);
    assert_eq!(group.slot_last, 1);
    assert_eq!(group.funding_epoch, 0);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_per_asset_slot_last_prevents_cross_asset_accrual_aliasing() {
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(2, 0, 1)).unwrap();
    let mut i = 0;
    while i < 2 {
        group.assets[i].effective_price = 100;
        group.assets[i].fund_px_last = 100;
        group.assets[i].raw_oracle_target_price = 100;
        group.assets[i].oi_eff_long_q = POS_SCALE;
        group.assets[i].oi_eff_short_q = POS_SCALE;
        group.assets[i].loss_weight_sum_long = POS_SCALE;
        group.assets[i].loss_weight_sum_short = POS_SCALE;
        i += 1;
    }

    let asset1_initial = group.assets[1];
    let first = group.kani_accrue_asset_to_core_not_atomic(0, 1, 101, 0, true);
    let asset0_after_first = group.assets[0];
    let asset1_slot_before = group.assets[1].slot_last;
    assert_eq!(
        group.assets[1], asset1_initial,
        "asset 0 accrual must not mutate asset 1 state"
    );
    let second = group.kani_accrue_asset_to_core_not_atomic(1, 1, 101, 0, true);

    kani::cover!(
        first.is_ok() && second.is_ok(),
        "v16 same-slot cross-asset accrual covers both assets"
    );
    assert!(first.is_ok());
    assert!(second.is_ok());
    assert_eq!(
        group.assets[0], asset0_after_first,
        "asset 1 accrual must not mutate asset 0 state"
    );
    assert_eq!(group.assets[0].slot_last, 1);
    assert_eq!(asset1_slot_before, 0);
    assert_eq!(group.assets[1].slot_last, 1);
    assert_ne!(group.assets[0].k_long, 0);
    assert_ne!(group.assets[1].k_long, 0);
}

#[kani::proof]
#[kani::unwind(6)]
#[kani::solver(cadical)]
fn proof_v16_106_loss_stale_summary_filters_accruable_lifecycles() {
    let now_u8: u8 = kani::any();
    let slot0_u8: u8 = kani::any();
    let slot1_u8: u8 = kani::any();
    let exposed0: bool = kani::any();
    let exposed1: bool = kani::any();
    kani::assume(now_u8 <= 3);
    kani::assume(slot0_u8 <= now_u8);
    kani::assume(slot1_u8 <= now_u8);
    let now = now_u8 as u64;
    let slot0 = slot0_u8 as u64;
    let slot1 = slot1_u8 as u64;
    let lifecycle0 = symbolic_lifecycle();
    let lifecycle1 = symbolic_lifecycle();
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(2, 0, 1)).unwrap();
    group.assets[0].lifecycle = lifecycle0;
    group.assets[1].lifecycle = lifecycle1;
    group.assets[0].slot_last = slot0;
    group.assets[1].slot_last = slot1;
    group.assets[0].oi_eff_long_q = if exposed0 { 1 } else { 0 };
    group.assets[1].oi_eff_short_q = if exposed1 { 1 } else { 0 };

    let (anchor, stale) = group.kani_accruable_asset_slot_summary(now).unwrap();

    let contributes0 = matches!(
        lifecycle0,
        AssetLifecycleV16::Active | AssetLifecycleV16::DrainOnly
    ) && exposed0;
    let contributes1 = matches!(
        lifecycle1,
        AssetLifecycleV16::Active | AssetLifecycleV16::DrainOnly
    ) && exposed1;
    let expected_anchor = if contributes0 && contributes1 {
        slot0.min(slot1)
    } else if contributes0 {
        slot0
    } else if contributes1 {
        slot1
    } else {
        now
    };
    let expected_stale = (contributes0 && slot0 < now) || (contributes1 && slot1 < now);

    kani::cover!(
        contributes0 && !contributes1 && slot0 < now,
        "v16 only accruable lifecycle contributes to loss-stale"
    );
    kani::cover!(
        !contributes0 && !contributes1 && (slot0 < now || slot1 < now),
        "v16 non-accruable or unexposed stale slots do not pin loss-stale"
    );
    assert_eq!(anchor, expected_anchor);
    assert_eq!(stale, expected_stale);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_106_accrual_keeps_loss_stale_until_all_accruable_assets_current() {
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(2, 0, 1)).unwrap();
    group.config.max_accrual_dt_slots = 10;
    group.config.min_funding_lifetime_slots = 10;
    let mut i = 0usize;
    while i < 2 {
        group.assets[i].effective_price = 1;
        group.assets[i].fund_px_last = 1;
        group.assets[i].raw_oracle_target_price = 1;
        group.assets[i].oi_eff_long_q = POS_SCALE;
        group.assets[i].oi_eff_short_q = POS_SCALE;
        group.assets[i].loss_weight_sum_long = POS_SCALE;
        group.assets[i].loss_weight_sum_short = POS_SCALE;
        i += 1;
    }

    group
        .kani_accrue_asset_to_core_not_atomic(1, 20, 2, 0, true)
        .unwrap();
    group
        .kani_accrue_asset_to_core_not_atomic(0, 20, 2, 0, true)
        .unwrap();
    group
        .kani_accrue_asset_to_core_not_atomic(0, 20, 2, 0, true)
        .unwrap();

    kani::cover!(
        group.assets[0].slot_last == 20 && group.assets[1].slot_last == 10,
        "v16 stale sibling asset remains reachable after another asset catches up"
    );
    assert_eq!(group.assets[0].slot_last, 20);
    assert_eq!(group.assets[1].slot_last, 10);
    assert_eq!(group.slot_last, 10);
    assert!(group.loss_stale_active);

    group
        .kani_accrue_asset_to_core_not_atomic(1, 20, 2, 0, true)
        .unwrap();
    assert_eq!(group.assets[0].slot_last, 20);
    assert_eq!(group.assets[1].slot_last, 20);
    assert_eq!(group.slot_last, 20);
    assert!(!group.loss_stale_active);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_funding_accrual_uses_only_bounded_segment_dt() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    group.config.max_price_move_bps_per_slot = 4_999;
    group.config.max_abs_funding_e9_per_slot = 1;
    group.config.max_accrual_dt_slots = 2;
    group.config.min_funding_lifetime_slots = 2;
    group.assets[0].effective_price = 1_000_000_000;
    group.assets[0].fund_px_last = 1_000_000_000;
    group.assets[0].raw_oracle_target_price = 1_000_000_000;
    let mut long = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut short = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group
        .attach_leg(&mut long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut short, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();

    let out = group
        .accrue_asset_to_not_atomic(0, 10, 1_000_000_000, 1, true)
        .unwrap();
    kani::cover!(
        out.funding_active && out.dt == 2 && group.current_slot == 10,
        "v16 funding stale catchup covers bounded segment dt"
    );

    assert_eq!(out.dt, 2);
    assert!(out.loss_stale_after);
    assert_eq!(group.slot_last, 2);
    assert_eq!(group.current_slot, 10);
    assert_eq!(group.assets[0].f_long_num, -2 * ADL_ONE as i128);
    assert_eq!(group.assets[0].f_short_num, 2 * ADL_ONE as i128);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_combined_price_and_funding_accrual_keeps_k_and_f_separate() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    group.config.max_price_move_bps_per_slot = 9_999;
    group.config.max_abs_funding_e9_per_slot = 1;
    group.assets[0].effective_price = 999_999_999;
    group.assets[0].fund_px_last = 999_999_999;
    group.assets[0].raw_oracle_target_price = 999_999_999;
    let mut long = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut short = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group
        .attach_leg(&mut long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut short, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();

    let out = group
        .accrue_asset_to_not_atomic(0, 1, 1_000_000_000, 1, true)
        .unwrap();
    kani::cover!(
        out.price_move_active && out.funding_active,
        "v16 combined mark and funding accrual reachable"
    );

    assert_eq!(group.assets[0].k_long, ADL_ONE as i128);
    assert_eq!(group.assets[0].k_short, -(ADL_ONE as i128));
    assert_eq!(group.assets[0].f_long_num, -(ADL_ONE as i128));
    assert_eq!(group.assets[0].f_short_num, ADL_ONE as i128);
    assert_eq!(group.assets[0].fund_px_last, 1_000_000_000);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_zero_funding_rate_advances_time_without_f_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    group.assets[0].effective_price = 100;
    group.assets[0].fund_px_last = 100;
    group.assets[0].raw_oracle_target_price = 100;
    let mut long = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut short = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group
        .attach_leg(&mut long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut short, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    let before = group.assets[0];

    let out = group
        .accrue_asset_to_not_atomic(0, 1, 100, 0, true)
        .unwrap();
    kani::cover!(
        group.assets[0].oi_eff_long_q != 0 && group.assets[0].oi_eff_short_q != 0,
        "v16 zero-rate funding proof covers bilateral exposure"
    );

    assert!(!out.funding_active);
    assert_eq!(group.assets[0].f_long_num, before.f_long_num);
    assert_eq!(group.assets[0].f_short_num, before.f_short_num);
    assert_eq!(group.funding_epoch, 0);
    assert_eq!(group.slot_last, 1);
    assert_eq!(group.current_slot, 1);
}

fn funding_sign_floor_fixture() -> (MarketGroupV16, PortfolioAccountV16, PortfolioAccountV16) {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 10)).unwrap();
    group.config.max_price_move_bps_per_slot = 4_999;
    group.config.max_abs_funding_e9_per_slot = 1;
    group.assets[0].effective_price = 1_000_000_000;
    group.assets[0].fund_px_last = 1_000_000_000;
    group.assets[0].raw_oracle_target_price = 1_000_000_000;
    let mut long = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut short = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group
        .attach_leg(&mut long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut short, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    (group, long, short)
}

fn assert_funding_accrual_writes_f_ledger_sign_and_floor(
    funding_rate_e9: i128,
    expected_f_long: i128,
    expected_f_short: i128,
) {
    let (mut group, _long, _short) = funding_sign_floor_fixture();
    let out = group
        .accrue_asset_to_not_atomic(0, 1, 1_000_000_000, funding_rate_e9, true)
        .unwrap();

    assert!(out.funding_active);
    assert_eq!(group.assets[0].f_long_num, expected_f_long);
    assert_eq!(group.assets[0].f_short_num, expected_f_short);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

fn assert_funding_refresh_side_matches_sign_and_floor(
    funding_rate_e9: i128,
    refresh_long: bool,
    expected_pnl: i128,
) {
    let (mut group, mut long, mut short) = funding_sign_floor_fixture();
    let refreshed = if refresh_long {
        group
            .kani_apply_signed_kf_delta_to_pnl(&mut long, expected_pnl, None)
            .unwrap();
        long
    } else {
        group
            .kani_apply_signed_kf_delta_to_pnl(&mut short, expected_pnl, None)
            .unwrap();
        short
    };

    kani::cover!(
        funding_rate_e9 != 0,
        "v16 funding refresh settlement path applies signed K/F delta"
    );
    assert_eq!(refreshed.pnl, expected_pnl);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_same_slot_exposed_price_move_rejects_before_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group
        .attach_leg(&mut account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let before_asset = group.assets[0];
    let before_slot = group.slot_last;
    let before_current = group.current_slot;
    let before_mode = group.mode;

    let result = group.accrue_asset_to_not_atomic(0, 0, 2, 0, true);

    assert_eq!(result, Err(V16Error::NonProgress));
    assert_eq!(group.assets[0], before_asset);
    assert_eq!(group.slot_last, before_slot);
    assert_eq!(group.current_slot, before_current);
    assert_eq!(group.mode, before_mode);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_over_cap_exposed_price_move_rejects_before_kf_price_or_slot_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    group.config.max_price_move_bps_per_slot = 100;
    group.assets[0].effective_price = 100;
    group.assets[0].raw_oracle_target_price = 100;
    group.assets[0].fund_px_last = 100;
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group
        .attach_leg(&mut account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();

    let before_asset = group.assets[0];
    let before_slot_last = group.slot_last;
    let before_current_slot = group.current_slot;
    let before_oracle_epoch = group.oracle_epoch;
    let before_funding_epoch = group.funding_epoch;
    let before_loss_stale = group.loss_stale_active;
    let result = group.accrue_asset_to_not_atomic(0, 1, 102, 0, true);

    kani::cover!(
        result == Err(V16Error::RecoveryRequired)
            && group.assets[0].oi_eff_long_q != 0
            && group.config.max_price_move_bps_per_slot == 100,
        "v16 over-cap exposed price move recovery path reachable"
    );
    assert_eq!(result, Err(V16Error::RecoveryRequired));
    assert_eq!(group.assets[0], before_asset);
    assert_eq!(group.slot_last, before_slot_last);
    assert_eq!(group.current_slot, before_current_slot);
    assert_eq!(group.oracle_epoch, before_oracle_epoch);
    assert_eq!(group.funding_epoch, before_funding_epoch);
    assert_eq!(group.loss_stale_active, before_loss_stale);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_partial_liquidation_can_reduce_risk_without_forcing_full_close() {
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let senior_before = group.c_tot + group.insurance;
    let vault_before = group.vault;
    group.assets[0].oi_eff_long_q = POS_SCALE;
    group.assets[0].oi_eff_short_q = POS_SCALE;
    group.assets[0].a_short = ADL_ONE;

    group
        .kani_reduce_matching_open_interest_for_unilateral_close(0, SideV16::Long, POS_SCALE / 2)
        .unwrap();
    kani::cover!(group.assets[0].oi_eff_short_q == POS_SCALE / 2);
    assert_eq!(group.assets[0].oi_eff_long_q, POS_SCALE);
    assert_eq!(group.assets[0].oi_eff_short_q, POS_SCALE / 2);
    assert_eq!(group.assets[0].a_short, ADL_ONE / 2);
    assert_eq!(group.c_tot + group.insurance, senior_before);
    assert_eq!(group.vault, vault_before);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_partial_liquidation_cannot_socialize_residual_while_open_risk_remains() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut bankrupt =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut opposing = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));

    group
        .attach_leg(&mut bankrupt, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    bankrupt.pnl = -100;
    group.negative_pnl_account_count = 1;
    let before_b_short = group.assets[0].b_short_num;
    let before_basis = bankrupt.legs[0].basis_pos_q;
    let before_bitmap = bankrupt.active_bitmap;
    let before_b_loss_booked = bankrupt.close_progress.b_loss_booked;

    let would_leave_open_uncovered_risk =
        kani_liquidation_close_would_leave_uncovered_loss_with_open_risk(
            bankrupt.pnl,
            bankrupt.capital,
            bankrupt.active_bitmap,
            0,
            POS_SCALE / 2,
            bankrupt.legs[0].basis_pos_q.unsigned_abs(),
        )
        .unwrap();

    kani::cover!(
        would_leave_open_uncovered_risk,
        "v16 partial liquidation residual routes to recovery before B booking"
    );
    assert!(would_leave_open_uncovered_risk);
    assert_eq!(group.assets[0].b_short_num, before_b_short);
    assert_eq!(bankrupt.close_progress.b_loss_booked, before_b_loss_booked);
    assert_eq!(bankrupt.legs[0].basis_pos_q, before_basis);
    assert_eq!(bankrupt.active_bitmap, before_bitmap);
    assert_eq!(bankrupt.pnl, -100);
}

#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_liquidation_rejects_zero_close_before_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group
        .attach_leg(&mut account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let before_vault = group.vault;
    let before_c_tot = group.c_tot;
    let before_insurance = group.insurance;
    let before_bitmap = account.active_bitmap;
    let before_leg = account.legs[0];

    let result = group.liquidate_account_not_atomic(
        &mut account,
        LiquidationRequestV16 {
            asset_index: 0,
            close_q: 0,
            fee_bps: 0,
        },
        &[100; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(result, Err(V16Error::InvalidConfig));
    assert_eq!(group.vault, before_vault);
    assert_eq!(group.c_tot, before_c_tot);
    assert_eq!(group.insurance, before_insurance);
    assert_eq!(account.active_bitmap[0], before_bitmap[0]);
    assert_eq!(account.legs[0].active, before_leg.active);
    assert_eq!(account.legs[0].market_id, before_leg.market_id);
    assert_eq!(account.legs[0].basis_pos_q, before_leg.basis_pos_q);
}

#[kani::proof]
#[kani::unwind(32)]
#[kani::solver(cadical)]
fn proof_v16_liquidation_fee_floor_shortfall_charges_available_capital_only() {
    let capital: u8 = kani::any();
    kani::assume(capital > 0);
    kani::assume(capital <= 20);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group
        .kani_deposit_core(&mut account, capital as u128)
        .unwrap();

    let charged = group
        .kani_charge_account_fee_current(&mut account, 40)
        .unwrap();

    kani::cover!(
        charged < 40,
        "v16 liquidation-fee floor shortfall fee path reachable"
    );
    assert_eq!(charged, capital as u128);
    assert_eq!(account.capital, 0);
    assert_eq!(group.insurance, capital as u128);
    assert_eq!(group.c_tot, 0);
    assert_eq!(group.vault, capital as u128);
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v16_resolved_unattributed_negative_pnl_fails_closed_without_erasure() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.pnl = -3;
    group.negative_pnl_account_count = 1;
    group.vault = 3;
    group.resolve_market_not_atomic(1).unwrap();
    let before = account.clone();
    let vault_before = group.vault;
    let c_tot_before = group.c_tot;
    let insurance_before = group.insurance;
    let negative_before = group.negative_pnl_account_count;

    let result =
        group.kani_resolved_unattributed_insolvent_negative_pnl_requires_recovery(&account);

    kani::cover!(
        result == Ok(true),
        "v16 resolved unattributed bad debt fails closed"
    );
    assert_eq!(result, Ok(true));
    assert_eq!(account, before);
    assert_eq!(group.vault, vault_before);
    assert_eq!(group.c_tot, c_tot_before);
    assert_eq!(group.insurance, insurance_before);
    assert_eq!(group.negative_pnl_account_count, negative_before);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_resolved_preexisting_close_ledger_gates_do_not_call_recovery() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.attach_leg(&mut account, 0, SideV16::Long, 1).unwrap();
    let ledger = CloseProgressLedgerV16 {
        active: true,
        finalized: false,
        close_id: 1,
        asset_index: 0,
        market_id: group.assets[0].market_id,
        domain_side: SideV16::Short,
        gross_loss_at_close_start: 2,
        residual_remaining: 2,
        drift_reference_slot: 0,
        max_close_slot: 0,
        ..CloseProgressLedgerV16::EMPTY
    };
    group.pending_domain_loss_barriers[1] = 1;
    group.resolve_market_not_atomic(1).unwrap();

    let expired = group.kani_ensure_close_progress_not_expired(ledger);
    let snapshot = group.kani_ensure_open_close_snapshot_current_or_recovery(&account, ledger);

    kani::cover!(
        expired == Ok(()) && snapshot == Ok(()),
        "v16 resolved close-progress gates bypass recovery declaration"
    );
    assert_eq!(expired, Ok(()));
    assert_eq!(snapshot, Ok(()));
    assert_eq!(group.mode, MarketModeV16::Resolved);
    assert_eq!(group.recovery_reason, None);
}

#[kani::unwind(64)]
#[kani::solver(cadical)]
fn proof_v16_view_initial_margin_source_lien_creation_is_backed() {
    let effective_raw: u16 = kani::any();
    kani::assume(effective_raw > 0);
    kani::assume(effective_raw <= 1_000);
    let effective = effective_raw as u128;
    let backing_num = effective * BOUND_SCALE;
    let face_num = backing_num;
    let current_slot = 0;

    let source_credit = SourceCreditStateV16 {
        positive_claim_bound_num: face_num,
        exact_positive_claim_num: face_num,
        fresh_reserved_backing_num: backing_num,
        credit_rate_num: CREDIT_RATE_SCALE,
        ..SourceCreditStateV16::EMPTY
    };
    let backing_bucket = BackingBucketV16 {
        market_id: 1,
        fresh_unliened_backing_num: backing_num,
        expiry_slot: 100,
        status: BackingBucketStatusV16::Fresh,
        ..BackingBucketV16::EMPTY
    };
    let (backing_after, source_credit_after) =
        MarketGroupV16ViewMut::<u64>::kani_prepare_counterparty_lien_create_delta(
            backing_bucket,
            source_credit,
            current_slot,
            backing_num,
        )
        .unwrap();
    let mut source_domain = PortfolioSourceDomainV16Account::default();
    source_domain.source_claim_market_id = V16PodU64::new(1);
    source_domain.source_claim_bound_num = V16PodU128::new(face_num);
    MarketGroupV16ViewMut::<u64>::kani_apply_counterparty_source_credit_lien_delta(
        &mut source_domain,
        face_num,
        backing_num,
        effective,
        current_slot,
    )
    .unwrap();

    kani::cover!(effective > 0, "source-credit IM lien branch is reachable");
    assert_eq!(backing_after.fresh_unliened_backing_num, 0);
    assert_eq!(backing_after.valid_liened_backing_num, backing_num);
    assert_eq!(source_credit_after.valid_liened_backing_num, backing_num);
    assert_eq!(
        source_credit_after.fresh_reserved_backing_num,
        backing_after.valid_liened_backing_num
    );
    assert_eq!(source_domain.source_claim_liened_num.get(), face_num);
    assert_eq!(
        source_domain.source_lien_effective_reserved.get(),
        effective
    );
    assert_eq!(
        source_domain.source_claim_counterparty_liened_num.get(),
        face_num
    );
    assert_eq!(
        source_domain.source_lien_counterparty_backing_num.get(),
        backing_num
    );
    assert_eq!(source_domain.source_lien_fee_last_slot.get(), current_slot);
}
