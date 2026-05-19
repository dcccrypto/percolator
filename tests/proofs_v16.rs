#![cfg(kani)]

use percolator::v16::{
    account_equity, risk_notional_ceil, AssetLifecycleV16, CloseProgressLedgerV16,
    DeadLegForfeitOutcomeV16, EngineAssetSlotV16Account, HLockLaneV16, HealthCertV16,
    LiquidationRequestV16, MarketGroupV16, MarketGroupV16Account, MarketGroupV16HeaderAccount,
    MarketModeV16, PermissionlessCrankActionV16, PermissionlessCrankRequestV16,
    PermissionlessProgressOutcomeV16, PermissionlessRecoveryReasonV16, PortfolioAccountV16,
    PortfolioAccountV16Account, PortfolioLegV16, PortfolioLegV16Account, ProvenanceHeaderV16,
    RebalanceRequestV16, ResolvedCloseOutcomeV16, ResolvedPayoutLedgerV16,
    ResolvedPayoutReceiptV16, SideModeV16, SideV16, SourceCreditLienAggregateProofV16,
    StockReconciliationProofV16, TradeRequestV16, V16ActiveBitmap, V16Config, V16Error, V16PodI128,
    V16PodU64, V16_DOMAIN_COUNT, V16_MAX_PORTFOLIO_ASSETS_N,
};
use percolator::{
    ADL_ONE, BOUND_SCALE, CREDIT_RATE_SCALE, MAX_OI_SIDE_Q, MAX_POSITION_ABS_Q,
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

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_source_credit_rate_is_bounded_by_available_backing() {
    let case: u8 = kani::any();

    let (market, _, _) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let claim = 100u128;
    let backing = match case % 3 {
        0 => 0,
        1 => 40,
        _ => 150,
    };

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

    kani::cover!(case % 3 == 0, "v16 source credit zero backing reachable");
    kani::cover!(case % 3 == 1, "v16 source credit partial backing reachable");
    kani::cover!(case % 3 == 2, "v16 source credit full backing reachable");
    assert_eq!(group.source_credit[0].credit_rate_num, expected_rate);
    assert!(group.source_credit[0].credit_rate_num <= CREDIT_RATE_SCALE);
}

fn assert_account_source_claim_equity_uses_source_credit_rate_case(backing_face: u128) {
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
    if backing_face != 0 {
        group
            .add_fresh_counterparty_backing_not_atomic(0, backing_face * BOUND_SCALE, 10)
            .unwrap();
    }
    let prices = [1u64; V16_MAX_PORTFOLIO_ASSETS_N];
    let cert = group.full_account_refresh(&mut account, &prices).unwrap();

    assert_eq!(cert.certified_equity, backing_face as i128);
    assert!(cert.certified_equity <= account.pnl);
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v16_account_source_claim_equity_zero_backing_gives_zero_credit() {
    assert_account_source_claim_equity_uses_source_credit_rate_case(0);
    kani::cover!(true, "v16 account source claim zero backing reachable");
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v16_account_source_claim_equity_uses_source_credit_rate() {
    assert_account_source_claim_equity_uses_source_credit_rate_case(5);
    kani::cover!(true, "v16 account source claim partial backing reachable");
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v16_account_source_claim_equity_full_backing_gives_full_credit() {
    assert_account_source_claim_equity_uses_source_credit_rate_case(10);
    kani::cover!(true, "v16 account source claim full backing reachable");
}

#[kani::proof]
#[kani::unwind(95)]
#[kani::solver(cadical)]
fn proof_v16_expired_fresh_backing_requires_refresh_before_source_credit_conversion() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut other_claimant =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [49; 32], owner));
    group.vault = 1_000;
    group.insurance = 300;
    group
        .add_account_source_positive_pnl_not_atomic(&mut account, 0, 300)
        .unwrap();
    group
        .add_account_source_positive_pnl_not_atomic(&mut other_claimant, 0, 100)
        .unwrap();
    group
        .add_fresh_counterparty_backing_not_atomic(0, 100 * BOUND_SCALE, 1)
        .unwrap();
    group
        .reserve_insurance_credit_not_atomic(0, 300 * BOUND_SCALE)
        .unwrap();

    let prices = [1u64; V16_MAX_PORTFOLIO_ASSETS_N];
    group.full_account_refresh(&mut account, &prices).unwrap();
    group.accrue_asset_to_not_atomic(0, 1, 1, 0, true).unwrap();
    let before = (account.capital, account.pnl, group.c_tot, group.insurance);
    let stale_conversion = group.convert_released_pnl_to_capital_not_atomic(&mut account);

    kani::cover!(
        stale_conversion == Err(V16Error::Stale),
        "v16 expired fresh backing blocks still-certified source-credit conversion"
    );
    assert_eq!(stale_conversion, Err(V16Error::Stale));
    assert_eq!(
        before,
        (account.capital, account.pnl, group.c_tot, group.insurance)
    );

    group.full_account_refresh(&mut account, &prices).unwrap();
    assert_eq!(
        group.source_credit[0].credit_rate_num,
        CREDIT_RATE_SCALE * 3 / 4
    );
    let converted = group
        .convert_released_pnl_to_capital_not_atomic(&mut account)
        .unwrap();
    assert_eq!(converted, 225);
    assert_eq!(account.capital, 225);
    assert_eq!(account.pnl, 0);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(60)]
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
    let result = group.convert_released_pnl_to_capital_not_atomic(&mut account);

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

fn create_insurance_lien_via_public_withdraw(
    group: &mut MarketGroupV16,
    account: &mut PortfolioAccountV16,
    account_id_seed: u8,
    effective_credit: u128,
) {
    let mut opposite = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(
        group.market_group_id,
        [account_id_seed; 32],
        [9; 32],
    ));
    group.deposit_not_atomic(account, 10).unwrap();
    group.vault = group.vault.checked_add(10).unwrap();
    group.insurance = 10 * BOUND_SCALE;
    group.vault = group.vault.checked_add(group.insurance).unwrap();
    group
        .add_account_source_positive_pnl_not_atomic(account, 0, 10)
        .unwrap();
    group
        .reserve_insurance_credit_not_atomic(0, 10 * BOUND_SCALE)
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

    create_counterparty_lien_via_public_withdraw(&mut group, &mut account, 19, 5, 10);
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

    create_insurance_lien_via_public_withdraw(&mut group, &mut account, 20, 5);
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
#[kani::unwind(100)]
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
#[kani::unwind(140)]
#[kani::solver(cadical)]
fn proof_v16_full_refresh_impairs_expired_counterparty_lien_before_equity_credit() {
    let market = [1; 32];
    let mut group = MarketGroupV16::new(market, source_lien_config()).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [14; 32], [1; 32]));

    seed_counterparty_source_lien_state(&mut group, &mut account, 5, 1);
    assert_eq!(account.source_lien_effective_reserved[0], 5);
    set_account_capital_for_canonical_fixture(&mut group, &mut account, 5);
    group.current_slot = 1;

    let cert = group
        .full_account_refresh(&mut account, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    kani::cover!(true, "v16 expired source-lien impairment reachable");
    assert_eq!(cert.certified_equity, 5);
    assert_eq!(cert.certified_equity, account.capital as i128);
    assert_eq!(account.source_lien_effective_reserved[0], 0);
    assert_eq!(account.source_lien_counterparty_backing_num[0], 0);
    assert_eq!(account.source_claim_liened_num[0], 0);
    assert!(account.source_claim_impaired_num[0] != 0);
    assert_eq!(account.source_lien_impaired_effective_reserved[0], 5);
    assert_eq!(group.source_credit[0].valid_liened_backing_num, 0);
    assert_eq!(
        group.source_credit[0].impaired_liened_backing_num,
        5 * BOUND_SCALE
    );
    group.assert_public_invariants().unwrap();
}

#[kani::proof]
#[kani::unwind(140)]
#[kani::solver(cadical)]
fn proof_v16_insurance_lien_impairment_removes_account_health_credit() {
    let market = [1; 32];
    let mut group = MarketGroupV16::new(market, source_lien_config()).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [15; 32], [1; 32]));

    seed_insurance_source_lien_state(&mut group, &mut account, 10);
    assert_eq!(account.source_lien_effective_reserved[0], 10);
    set_account_capital_for_canonical_fixture(&mut group, &mut account, 0);
    assert_eq!(
        account.source_lien_insurance_backing_num[0],
        10 * BOUND_SCALE
    );

    let impaired = group
        .impair_account_source_credit_lien_from_insurance_not_atomic(&mut account, 0)
        .unwrap();
    let cert = group
        .full_account_refresh(&mut account, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    kani::cover!(true, "v16 insurance source-lien impairment reachable");
    assert_eq!(impaired, 10);
    assert_eq!(cert.certified_equity, 0);
    assert_eq!(account.source_lien_effective_reserved[0], 0);
    assert_eq!(account.source_lien_insurance_backing_num[0], 0);
    assert_eq!(account.source_claim_liened_num[0], 0);
    assert!(account.source_claim_impaired_num[0] != 0);
    assert_eq!(account.source_lien_impaired_effective_reserved[0], 10);
    assert_eq!(group.source_credit[0].valid_liened_insurance_num, 0);
    assert_eq!(
        group.source_credit[0].impaired_liened_insurance_num,
        10 * BOUND_SCALE
    );
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v16_deposit_and_withdraw_value_flow_preserves_vault_capital_totals() {
    let market = [1; 32];
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [16; 32], [1; 32]));

    group.deposit_not_atomic(&mut account, 11).unwrap();
    group
        .withdraw_not_atomic(&mut account, 4, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    kani::cover!(true, "v16 deposit/withdraw token-value flow reachable");
    assert_eq!(group.vault, 7);
    assert_eq!(group.c_tot, 7);
    assert_eq!(account.capital, 7);
    group.assert_public_invariants().unwrap();
}

#[kani::proof]
#[kani::unwind(80)]
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
    group.deposit_not_atomic(&mut account, 30).unwrap();
    account.pnl = -(loss as i128);
    group.negative_pnl_account_count = 1;

    let paid_loss = group
        .settle_negative_pnl_from_principal(&mut account)
        .unwrap();
    let charged_fee = group
        .charge_account_fee_not_atomic(&mut account, fee as u128)
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
#[kani::unwind(70)]
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
            settlement_rounding_residue_total: 0,
            unallocated_protocol_surplus: surplus as u128,
        }
    );
    assert_eq!(proof.validate(), Ok(()));
    group.assert_public_invariants().unwrap();
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
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v16_counterparty_lien_lifecycle_preserves_backing_encumbrance() {
    let consume: bool = kani::any();

    let (market, _, _) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let backing = 100u128;
    let lien = 30u128;
    group
        .add_source_positive_claim_bound_not_atomic(0, backing, backing)
        .unwrap();
    group
        .add_fresh_counterparty_backing_not_atomic(0, backing, 10)
        .unwrap();
    let available_before = group.source_credit_available_backing_num(0).unwrap();

    group
        .create_source_credit_lien_from_counterparty_not_atomic(0, lien)
        .unwrap();
    let available_liened = group.source_credit_available_backing_num(0).unwrap();
    assert_eq!(available_before, backing);
    assert_eq!(available_liened, backing - lien);
    assert_eq!(group.source_credit[0].fresh_reserved_backing_num, backing);
    assert_eq!(group.source_credit[0].valid_liened_backing_num, lien);

    if consume {
        group
            .consume_source_credit_lien_from_counterparty_not_atomic(0, lien)
            .unwrap();
        kani::cover!(true, "v16 counterparty lien consume branch reachable");
        assert_eq!(group.source_credit[0].spent_backing_num, lien);
        assert_eq!(
            group.source_credit[0].fresh_reserved_backing_num,
            backing - lien
        );
        assert_eq!(
            group.source_credit_available_backing_num(0),
            Ok(backing - lien)
        );
    } else {
        group
            .impair_source_credit_lien_from_counterparty_not_atomic(0, lien)
            .unwrap();
        kani::cover!(true, "v16 counterparty lien impair branch reachable");
        assert_eq!(group.source_credit[0].impaired_liened_backing_num, lien);
        assert_eq!(
            group.source_credit[0].fresh_reserved_backing_num,
            backing - lien
        );
        assert_eq!(
            group.source_credit_available_backing_num(0),
            Ok(backing - lien)
        );
    }
    assert_eq!(group.source_credit[0].valid_liened_backing_num, 0);
    group.assert_public_invariants().unwrap();
}

#[kani::proof]
#[kani::unwind(55)]
#[kani::solver(cadical)]
fn proof_v16_source_lien_creation_has_valid_reservation_encumbrance_proof() {
    let market = [1; 32];
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    group
        .add_source_positive_claim_bound_not_atomic(0, 10, 10)
        .unwrap();
    group
        .add_fresh_counterparty_backing_not_atomic(0, 10 * BOUND_SCALE, 10)
        .unwrap();
    group
        .create_source_credit_lien_from_counterparty_not_atomic(0, 4 * BOUND_SCALE)
        .unwrap();

    let proof = group.reservation_encumbrance_proof_for_domain(0).unwrap();

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
    group.assert_public_invariants().unwrap();
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v16_insurance_reservation_lifecycle_preserves_encumbrance() {
    let consume: bool = kani::any();

    let (market, _, _) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let reserve_atoms = 100u128;
    let reserve = reserve_atoms * BOUND_SCALE;
    let lien_atoms = 30u128;
    let lien = lien_atoms * BOUND_SCALE;
    group.vault = reserve_atoms;
    group.insurance = reserve_atoms;
    group
        .add_source_positive_claim_bound_not_atomic(0, reserve_atoms, reserve_atoms)
        .unwrap();
    group
        .reserve_insurance_credit_not_atomic(0, reserve)
        .unwrap();

    group
        .create_source_credit_lien_from_insurance_not_atomic(0, lien)
        .unwrap();
    assert_eq!(
        group.source_credit_available_backing_num(0),
        Ok(reserve - lien)
    );
    if consume {
        group
            .consume_source_credit_lien_from_insurance_not_atomic(0, lien)
            .unwrap();
        kani::cover!(true, "v16 insurance lien consume branch reachable");
        assert_eq!(group.insurance, reserve_atoms - lien_atoms);
        assert_eq!(
            group.insurance_credit_reservations[0].consumed_insurance_num,
            lien
        );
        assert_eq!(
            group.source_credit_available_backing_num(0),
            Ok(reserve - lien)
        );
    } else {
        group
            .impair_source_credit_lien_from_insurance_not_atomic(0, lien)
            .unwrap();
        kani::cover!(true, "v16 insurance lien impair branch reachable");
        assert_eq!(group.source_credit[0].impaired_liened_insurance_num, lien);
        assert_eq!(
            group.source_credit_available_backing_num(0),
            Ok(reserve - lien)
        );
    }
    assert_eq!(group.source_credit[0].valid_liened_insurance_num, 0);
    group.assert_public_invariants().unwrap();
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
#[kani::unwind(40)]
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
        .h_lock_lane(Some(&account), instruction_bankruptcy_candidate)
        .unwrap();
    if lane == HLockLaneV16::HMax {
        assert_eq!(selected, h_max as u64);
    } else {
        assert_eq!(selected, 0);
    }
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_hmin_zero_remains_available_when_no_lock_state_exists() {
    let h_max: u8 = kani::any();
    kani::assume(h_max > 0);
    let (market, account_id, owner) = symbolic_ids();
    let group =
        MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, h_max as u64)).unwrap();
    let account = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    assert_eq!(
        group.h_lock_lane(Some(&account), false),
        Ok(HLockLaneV16::HMin)
    );
    assert_eq!(group.select_h_lock(Some(&account), false), Ok(0));
}

#[kani::proof]
#[kani::unwind(40)]
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
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_b_stale_counter_transitions_are_idempotent_and_leg_gated() {
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

    group
        .attach_leg(&mut account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group.mark_leg_b_stale(&mut account, 0).unwrap();
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
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_account_equity_rejects_i128_min_persistent_pnl() {
    let (market, account_id, owner) = symbolic_ids();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.pnl = i128::MIN;
    assert_eq!(account_equity(&account), Err(V16Error::ArithmeticOverflow));
}

#[kani::proof]
#[kani::unwind(40)]
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
#[kani::unwind(10)]
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
#[kani::unwind(40)]
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
#[kani::unwind(40)]
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
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_persisted_wire_rejects_noncanonical_bool_enum_and_option() {
    let bad_bool: u8 = kani::any();
    let bad_market_mode: u8 = kani::any();
    let bad_side_mode: u8 = kani::any();
    let bad_option_present: u8 = kani::any();
    kani::assume(bad_bool > 1);
    kani::assume(bad_market_mode > 2);
    kani::assume(bad_side_mode > 2);
    kani::assume(bad_option_present > 1);

    let (market, account_id, owner) = symbolic_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let account = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    let mut account_wire = PortfolioAccountV16Account::from_runtime(&account);
    account_wire.stale_state = bad_bool;
    kani::cover!(bad_bool == 2, "v16 persisted invalid bool branch reachable");
    assert_eq!(account_wire.try_to_runtime(), Err(V16Error::InvalidConfig));

    let mut config_bool_wire = MarketGroupV16Account::from_runtime(&group);
    config_bool_wire.config.recovery_fallback_price_enabled = bad_bool;
    kani::cover!(
        bad_bool == 3,
        "v16 persisted invalid config bool branch reachable"
    );
    assert_eq!(
        config_bool_wire.try_to_runtime(),
        Err(V16Error::InvalidConfig)
    );

    let mut market_mode_wire = MarketGroupV16Account::from_runtime(&group);
    market_mode_wire.mode = bad_market_mode;
    kani::cover!(
        bad_market_mode == 3,
        "v16 persisted invalid market mode branch reachable"
    );
    assert_eq!(
        market_mode_wire.try_to_runtime(),
        Err(V16Error::InvalidConfig)
    );

    let mut side_mode_wire = MarketGroupV16Account::from_runtime(&group);
    side_mode_wire.asset_slots[0].asset.mode_long = bad_side_mode;
    kani::cover!(
        bad_side_mode == 3,
        "v16 persisted invalid side mode branch reachable"
    );
    assert_eq!(
        side_mode_wire.try_to_runtime(),
        Err(V16Error::InvalidConfig)
    );

    let mut option_wire = MarketGroupV16Account::from_runtime(&group);
    option_wire.recovery_reason.present = bad_option_present;
    kani::cover!(
        bad_option_present == 2,
        "v16 persisted invalid option-present branch reachable"
    );
    assert_eq!(option_wire.try_to_runtime(), Err(V16Error::InvalidConfig));
}

#[kani::proof]
#[kani::unwind(40)]
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
#[kani::unwind(80)]
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

    let wire = MarketGroupV16Account::from_runtime(&group);
    let decoded = wire.try_to_runtime().unwrap();

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
    let mut id_i = 0;
    while id_i < 32 {
        assert_eq!(decoded.market_group_id[id_i], group.market_group_id[id_i]);
        id_i += 1;
    }
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
    while asset_i < V16_MAX_PORTFOLIO_ASSETS_N {
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
#[kani::unwind(80)]
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
    let decoded = wire.try_to_runtime().unwrap();
    let checked = wire.validate_with_market(&group).unwrap();

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
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v16_portfolio_wire_roundtrip_preserves_source_lien_fields() {
    let (market, account_id, owner) = concrete_ids();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.pnl = 10;
    account.source_claim_bound_num[0] = 10 * BOUND_SCALE;
    account.source_claim_liened_num[0] = 2 * BOUND_SCALE;
    account.source_claim_counterparty_liened_num[0] = 2 * BOUND_SCALE;
    account.source_lien_effective_reserved[0] = 2;
    account.source_lien_counterparty_backing_num[0] = 2 * BOUND_SCALE;

    let wire = PortfolioAccountV16Account::from_runtime(&account);
    let decoded = wire.try_to_runtime().unwrap();

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
#[kani::unwind(10)]
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
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v16_persisted_wire_rejects_i128_min_economic_fields() {
    let dirty_case: u8 = kani::any();
    kani::assume(dirty_case < 6);

    let (market, account_id, owner) = concrete_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut active_group = group;
    active_group
        .attach_leg(&mut account, 0, SideV16::Long, 1)
        .unwrap();

    match dirty_case {
        0 => {
            let mut wire = MarketGroupV16Account::from_runtime(&group);
            wire.asset_slots[0].asset.k_long = V16PodI128::new(i128::MIN);
            assert_eq!(wire.try_to_runtime(), Err(V16Error::ArithmeticOverflow));
        }
        1 => {
            let mut wire = MarketGroupV16Account::from_runtime(&group);
            wire.asset_slots[0].asset.f_short_num = V16PodI128::new(i128::MIN);
            assert_eq!(wire.try_to_runtime(), Err(V16Error::ArithmeticOverflow));
        }
        2 => {
            let mut wire = PortfolioAccountV16Account::from_runtime(&account);
            wire.pnl = V16PodI128::new(i128::MIN);
            assert_eq!(wire.try_to_runtime(), Err(V16Error::ArithmeticOverflow));
        }
        3 => {
            let mut wire = PortfolioAccountV16Account::from_runtime(&account);
            wire.fee_credits = V16PodI128::new(i128::MIN);
            assert_eq!(wire.try_to_runtime(), Err(V16Error::ArithmeticOverflow));
        }
        4 => {
            let mut wire = PortfolioAccountV16Account::from_runtime(&account);
            wire.legs[0].k_snap = V16PodI128::new(i128::MIN);
            assert_eq!(wire.try_to_runtime(), Err(V16Error::ArithmeticOverflow));
        }
        _ => {
            let mut wire = PortfolioAccountV16Account::from_runtime(&account);
            wire.health_cert.certified_equity = V16PodI128::new(i128::MIN);
            assert_eq!(wire.try_to_runtime(), Err(V16Error::ArithmeticOverflow));
        }
    }

    kani::cover!(dirty_case == 0, "v16 wire rejects i128 min market K");
    kani::cover!(dirty_case == 1, "v16 wire rejects i128 min market F");
    kani::cover!(dirty_case == 2, "v16 wire rejects i128 min account PnL");
    kani::cover!(dirty_case == 3, "v16 wire rejects i128 min fee credits");
    kani::cover!(dirty_case == 4, "v16 wire rejects i128 min leg K snapshot");
    kani::cover!(
        dirty_case == 5,
        "v16 wire rejects i128 min health certificate"
    );
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v16_persisted_wire_rejects_provenance_and_hidden_leg_smuggling() {
    let case: u8 = kani::any();
    kani::assume(case < 5);
    let (market, account_id, owner) = concrete_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let empty = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut active = empty;
    let mut builder_group = group;
    builder_group
        .attach_leg(&mut active, 0, SideV16::Long, 1)
        .unwrap();
    let active_wire = PortfolioAccountV16Account::from_runtime(&active);
    let mut wire = PortfolioAccountV16Account::from_runtime(&empty);

    let expected = match case {
        0 => {
            wire.provenance_header.market_group_id = [9; 32];
            V16Error::ProvenanceMismatch
        }
        1 => {
            wire.owner = [9; 32];
            V16Error::ProvenanceMismatch
        }
        2 => {
            wire.active_bitmap = [V16PodU64::new(1)];
            V16Error::HiddenLeg
        }
        3 => {
            wire.legs[0] = active_wire.legs[0];
            wire.active_bitmap = [V16PodU64::new(0)];
            V16Error::HiddenLeg
        }
        _ => {
            wire.legs[1] = active_wire.legs[0];
            wire.active_bitmap = [V16PodU64::new(1 << 1)];
            V16Error::HiddenLeg
        }
    };

    kani::cover!(case == 0, "v16 persisted wrong-market account rejected");
    kani::cover!(case == 1, "v16 persisted wrong-owner account rejected");
    kani::cover!(case == 2, "v16 persisted bitmap-only leg rejected");
    kani::cover!(case == 3, "v16 persisted hidden active leg rejected");
    kani::cover!(case == 4, "v16 persisted out-of-config leg rejected");
    assert_eq!(wire.validate_with_market(&group), Err(expected));
}

#[kani::proof]
#[kani::unwind(40)]
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
#[kani::unwind(40)]
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

    let before_account = account;
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
#[kani::unwind(40)]
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
#[kani::unwind(40)]
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
#[kani::unwind(40)]
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
#[kani::unwind(50)]
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
#[kani::unwind(60)]
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
    let outcome = group.forfeit_recovery_leg_not_atomic(&mut account, 0, 1);

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
#[kani::unwind(48)]
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
#[kani::unwind(48)]
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
#[kani::unwind(48)]
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
#[kani::unwind(40)]
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
#[kani::unwind(40)]
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
#[kani::unwind(40)]
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
#[kani::unwind(40)]
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
#[kani::unwind(80)]
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
#[kani::unwind(45)]
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
    group
        .full_account_refresh(&mut account, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
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
#[kani::unwind(45)]
#[kani::solver(cadical)]
fn proof_v16_full_refresh_haircuts_positive_pnl_under_global_impairment() {
    let profit: u8 = kani::any();
    let residual: u8 = kani::any();
    kani::assume(profit > 1);
    kani::assume(profit <= 20);
    kani::assume(residual > 0);
    kani::assume(residual < profit);
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    group.deposit_not_atomic(&mut account, 10).unwrap();
    account.pnl = profit as i128;
    group.pnl_pos_tot = profit as u128;
    set_junior_bound(&mut group, profit as u128);
    group.vault = group.c_tot + group.insurance + residual as u128;

    let cert = group
        .full_account_refresh(&mut account, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    kani::cover!(
        residual == 1 && profit > 2,
        "v16 haircut certificate covers strongly impaired junior support"
    );
    assert_eq!(account_equity(&account), Ok(10 + profit as i128));
    assert_eq!(cert.certified_equity, 10 + residual as i128);
    assert!(cert.certified_equity < account_equity(&account).unwrap());
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v16_negative_kf_settlement_uses_haircut_support_not_face_netting() {
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
    account.pnl = 100;
    group.pnl_pos_tot = 100;
    set_junior_bound(&mut group, 100);
    group.vault = 50;
    group.assets[0].k_long = -(100 * ADL_ONE as i128);

    let cert = group
        .full_account_refresh(&mut account, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    kani::cover!(
        account.pnl == -50 && cert.certified_equity == -50,
        "v16 negative K/F settlement would be positive under face netting"
    );
    assert_eq!(account.pnl, -50);
    assert_eq!(group.pnl_pos_tot, 0);
    assert_eq!(group.pnl_pos_bound_tot, 0);
    assert_eq!(group.negative_pnl_account_count, 1);
    assert_eq!(cert.certified_equity, -50);
}

#[kani::proof]
#[kani::unwind(95)]
#[kani::solver(cadical)]
fn proof_v16_negative_kf_settlement_consumes_realizable_source_credit_before_principal() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut opposite =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [91; 32], owner));
    group.deposit_not_atomic(&mut account, 1_000).unwrap();
    group
        .add_account_source_positive_pnl_not_atomic(&mut account, 0, 500)
        .unwrap();
    group
        .add_fresh_counterparty_backing_not_atomic(0, 500 * BOUND_SCALE, 10)
        .unwrap();
    group
        .attach_leg(&mut account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut opposite, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    group.assets[0].k_long = -(500 * ADL_ONE as i128);

    let cert = group
        .full_account_refresh(&mut account, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    kani::cover!(
        group.source_credit[0].spent_backing_num == 500 * BOUND_SCALE,
        "v16 negative K/F settlement consumes source backing before principal"
    );
    assert_eq!(account.capital, 1_000);
    assert_eq!(account.pnl, 0);
    assert_eq!(group.c_tot, 1_000);
    assert_eq!(group.source_credit[0].spent_backing_num, 500 * BOUND_SCALE);
    assert_eq!(group.source_credit[0].fresh_reserved_backing_num, 0);
    assert_eq!(cert.certified_equity, 1_000);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(95)]
#[kani::solver(cadical)]
fn proof_v16_negative_kf_settlement_falls_back_to_global_residual_when_source_backing_absent() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut opposite =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [93; 32], owner));
    group
        .add_account_source_positive_pnl_not_atomic(&mut account, 0, 100)
        .unwrap();
    group
        .attach_leg(&mut account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut opposite, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    group.vault = 50;
    group.assets[0].k_long = -(100 * ADL_ONE as i128);

    let cert = group
        .full_account_refresh(&mut account, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    kani::cover!(
        group.source_credit[0].spent_backing_num == 0 && account.pnl == -50,
        "v16 source-attributed loss settlement falls back to global residual support"
    );
    assert_eq!(account.pnl, -50);
    assert_eq!(group.source_credit[0].spent_backing_num, 0);
    assert_eq!(group.pnl_pos_tot, 0);
    assert_eq!(group.pnl_pos_bound_tot, 0);
    assert_eq!(group.negative_pnl_account_count, 1);
    assert_eq!(cert.certified_equity, -50);
}

#[kani::proof]
#[kani::unwind(95)]
#[kani::solver(cadical)]
fn proof_v16_full_refresh_reserves_counterparty_backing_from_new_capital_backed_loss() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut loser = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut opposite =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [94; 32], owner));
    group.deposit_not_atomic(&mut loser, 1_000).unwrap();
    group
        .attach_leg(&mut loser, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut opposite, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    group.assets[0].k_long = -(500 * ADL_ONE as i128);

    let cert = group
        .full_account_refresh(&mut loser, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    kani::cover!(
        group.source_credit[0].fresh_reserved_backing_num == 500 * BOUND_SCALE,
        "v16 full refresh reserves capital-backed local loss as source backing"
    );
    assert_eq!(loser.pnl, 0);
    assert_eq!(loser.capital, 500);
    assert_eq!(group.c_tot, 500);
    assert_eq!(group.vault, 1_000);
    assert_eq!(cert.certified_equity, 500);
    assert_eq!(
        group.source_credit[0].fresh_reserved_backing_num,
        500 * BOUND_SCALE
    );
    assert_eq!(
        group.source_credit_available_backing_num(0),
        Ok(500 * BOUND_SCALE)
    );
    assert!(group.source_backing_buckets[0].expiry_slot >= group.current_slot + group.config.h_max);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(120)]
#[kani::solver(cadical)]
fn proof_v16_passive_backing_consumption_preserves_senior_accounting_without_wrapper_injection() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut loser = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut winner = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [95; 32], owner));
    assert_eq!(group.deposit_not_atomic(&mut loser, 1_000), Ok(()));
    assert_eq!(
        group.attach_leg(&mut loser, 0, SideV16::Long, POS_SCALE as i128),
        Ok(())
    );
    assert_eq!(
        group.attach_leg(&mut winner, 0, SideV16::Short, -(POS_SCALE as i128)),
        Ok(())
    );
    group.assets[0].k_long = -(500 * ADL_ONE as i128);
    group.assets[0].k_short = 500 * ADL_ONE as i128;

    assert!(group
        .full_account_refresh(&mut loser, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .is_ok());
    assert!(group
        .full_account_refresh(&mut winner, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .is_ok());
    assert_eq!(winner.capital, 0);
    assert_eq!(winner.pnl, 500);
    assert_eq!(group.source_credit[0].spent_backing_num, 0);
    assert_eq!(
        group.source_credit[0].fresh_reserved_backing_num,
        500 * BOUND_SCALE
    );

    assert_eq!(group.clear_leg(&mut winner, 0), Ok(()));
    assert_eq!(group.clear_leg(&mut loser, 0), Ok(()));
    assert!(group
        .full_account_refresh(&mut winner, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .is_ok());
    let converted = group.convert_released_pnl_to_capital_not_atomic(&mut winner);

    kani::cover!(
        converted == Ok(500) && group.source_credit[0].spent_backing_num == 500 * BOUND_SCALE,
        "v16 passively reserved counterparty backing is consumed by the winner"
    );
    assert_eq!(converted, Ok(500));
    assert_eq!(loser.capital, 500);
    assert_eq!(loser.pnl, 0);
    assert_eq!(winner.capital, 500);
    assert_eq!(winner.pnl, 0);
    assert_eq!(group.source_credit[0].spent_backing_num, 500 * BOUND_SCALE);
    assert_eq!(group.source_credit[0].fresh_reserved_backing_num, 0);
    assert_eq!(group.c_tot, group.vault);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v16_positive_kf_delta_cures_prior_loss_at_haircut_value() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(2, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group
        .attach_leg(&mut account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut account, 1, SideV16::Long, POS_SCALE as i128)
        .unwrap();
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
#[kani::unwind(95)]
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
#[kani::unwind(40)]
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
        .deposit_not_atomic(&mut account, amount as u128)
        .unwrap();
    assert_eq!(account.capital, amount as u128);
    assert_eq!(group.c_tot, amount as u128);
    assert_eq!(group.vault, amount as u128);

    group
        .withdraw_not_atomic(
            &mut account,
            amount as u128,
            &[1; V16_MAX_PORTFOLIO_ASSETS_N],
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
        .deposit_not_atomic(&mut account, amount as u128)
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
#[kani::unwind(45)]
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
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_partial_withdraw_can_leave_small_remainder() {
    let remainder: u16 = kani::any();
    kani::assume(remainder <= 1_000);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let deposit = remainder as u128 + 1;
    group.deposit_not_atomic(&mut account, deposit).unwrap();

    group
        .withdraw_not_atomic(&mut account, 1, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

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
#[kani::unwind(45)]
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

    let result = group.withdraw_not_atomic(
        &mut account,
        capital as u128 + 1,
        &[1; V16_MAX_PORTFOLIO_ASSETS_N],
    );

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
#[kani::unwind(40)]
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
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_close_portfolio_account_requires_clean_local_state() {
    let dirty_case: u8 = kani::any();
    kani::assume(dirty_case < 6);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let clean = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
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
#[kani::unwind(40)]
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
#[kani::unwind(40)]
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
#[kani::unwind(40)]
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
        market_id: group.assets[1].market_id,
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
#[kani::unwind(40)]
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
#[kani::unwind(150)]
#[kani::solver(cadical)]
fn proof_v16_compact_leg_slots_preserve_asset_identity() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(4, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    group
        .attach_leg(&mut account, 3, SideV16::Long, 11)
        .unwrap();
    group
        .attach_leg(&mut account, 1, SideV16::Short, -7)
        .unwrap();

    assert_eq!(account.active_bitmap, bitmap(&[0, 1]));
    assert!(account.legs[0].active);
    assert!(account.legs[1].active);
    assert_eq!(account.legs[0].asset_index, 3);
    assert_eq!(account.legs[1].asset_index, 1);
    assert_eq!(account.legs[0].market_id, group.assets[3].market_id);
    assert_eq!(account.legs[1].market_id, group.assets[1].market_id);

    group.clear_leg(&mut account, 3).unwrap();
    assert!(!account.legs[0].active);
    assert!(account.legs[1].active);
    assert_eq!(account.legs[1].asset_index, 1);
    assert_eq!(account.active_bitmap, bitmap(&[1]));

    group.attach_leg(&mut account, 2, SideV16::Long, 5).unwrap();
    assert!(account.legs[0].active);
    assert!(account.legs[1].active);
    assert_eq!(account.legs[0].asset_index, 2);
    assert_eq!(account.legs[1].asset_index, 1);
    assert_eq!(account.active_bitmap, bitmap(&[0, 1]));
    assert_eq!(group.validate_account_shape(&account), Ok(()));
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
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn proof_v16_config_separates_active_leg_and_market_slot_caps() {
    let valid = V16Config::public_user_fund_with_market_slots(4, 32, 0, 1);
    assert_eq!(valid.validate_public_user_fund(), Ok(()));

    let too_many_active_legs = V16Config::public_user_fund_with_market_slots(33, 32, 0, 1);
    assert_eq!(
        too_many_active_legs.validate_public_user_fund_shape(),
        Err(V16Error::InvalidConfig)
    );

    let too_many_market_slots = V16Config::public_user_fund_with_market_slots(4, 65, 0, 1);
    assert_eq!(
        too_many_market_slots.validate_public_user_fund_shape(),
        Err(V16Error::InvalidConfig)
    );
}

#[kani::proof]
#[kani::unwind(45)]
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
#[kani::unwind(40)]
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
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_asset_lifecycle_blocks_accrual_for_non_accruable_states() {
    let (market, _, _) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let lifecycle = symbolic_non_active_lifecycle();
    kani::assume(lifecycle != AssetLifecycleV16::DrainOnly);
    group.assets[0].lifecycle = lifecycle;
    let before = group;

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
#[kani::unwind(40)]
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
    let before = group;

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
#[kani::unwind(40)]
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
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_dynamic_header_activation_binds_backing_to_new_market_id() {
    let nonempty: bool = kani::any();
    let price = 7u64;
    let (market, _, _) = concrete_ids();
    let group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut header = MarketGroupV16HeaderAccount::from_runtime_with_capacity(&group, 1).unwrap();
    let mut slot = EngineAssetSlotV16Account::default();
    if nonempty {
        slot.asset.oi_eff_long_q = percolator::v16::V16PodU128::new(1);
        slot.asset.stored_pos_count_long = V16PodU64::new(1);
        slot.asset.loss_weight_sum_long = percolator::v16::V16PodU128::new(1);
    }
    let market_id = header.next_market_id.get();

    let result =
        header.activate_empty_asset_slot_not_atomic(0, &mut slot, price, group.current_slot);

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
#[kani::unwind(40)]
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
    let before = group;

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
#[kani::unwind(40)]
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
    let before = group;

    let result = group.activate_empty_asset_not_atomic(1, 7, 6);

    kani::cover!(
        result == Err(V16Error::LockActive),
        "v16 activation cooldown rejection reachable"
    );
    assert_eq!(result, Err(V16Error::LockActive));
    assert_eq!(group.assets[1], before.assets[1]);
    assert_eq!(group.asset_activation_count, before.asset_activation_count);
    assert_eq!(
        group.last_asset_activation_slot,
        before.last_asset_activation_slot
    );
    assert_eq!(group.risk_epoch, before.risk_epoch);
    assert_eq!(group.asset_set_epoch, before.asset_set_epoch);
}

#[kani::proof]
#[kani::unwind(45)]
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
#[kani::unwind(45)]
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
#[kani::unwind(40)]
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

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_account_b_chunk_either_advances_or_fails_closed() {
    let target_units: u8 = kani::any();
    let budget_units: u8 = kani::any();
    kani::assume(target_units <= 4);
    kani::assume(budget_units <= 4);
    let (market, account_id, owner) = symbolic_ids();
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
        kani::cover!(chunk.delta_b > 0, "v16 B chunk progress reachable");
        assert!(chunk.delta_b > 0);
        assert!(account.legs[0].b_snap > before_snap);
        assert!(chunk.remaining_after < before_remaining);
    }
}

#[kani::proof]
#[kani::unwind(40)]
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
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_liquidation_progress_rejects_non_reducing_scores() {
    let case: u8 = kani::any();
    let deficit: u8 = kani::any();
    let gross_loss: u8 = kani::any();
    kani::assume(case <= 3);
    kani::assume(deficit <= 5);
    kani::assume(gross_loss <= 5);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut before =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut after = before;
    group
        .full_account_refresh(&mut before, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    group
        .full_account_refresh(&mut after, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
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

    kani::cover!(case == 0, "v16 equal risk score non-progress reachable");
    kani::cover!(case == 1, "v16 worse deficit non-progress reachable");
    kani::cover!(case == 2, "v16 stale-penalty non-progress reachable");
    kani::cover!(case == 3, "v16 worse gross-loss non-progress reachable");

    assert_eq!(
        group.validate_liquidation_progress(&before, &after),
        Err(V16Error::NonProgress)
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_favorable_action_requires_current_full_refresh() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.capital = 2;

    assert_eq!(
        group.ensure_favorable_action_allowed(&account),
        Err(V16Error::Stale)
    );
    group
        .full_account_refresh(&mut account, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(group.ensure_favorable_action_allowed(&account), Ok(()));
    group.oracle_epoch += 1;
    assert_eq!(
        group.ensure_favorable_action_allowed(&account),
        Err(V16Error::Stale)
    );
}

#[kani::proof]
#[kani::unwind(60)]
#[kani::solver(cadical)]
fn proof_v16_health_certificate_bound_to_market_epochs_and_prices() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut long = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut short = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut long, 1_000).unwrap();
    group.deposit_not_atomic(&mut short, 1_000).unwrap();
    group
        .attach_leg(&mut long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut short, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();

    let cert = group
        .full_account_refresh(&mut long, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(cert.cert_oracle_epoch, group.oracle_epoch);
    assert_eq!(cert.cert_funding_epoch, group.funding_epoch);
    assert_eq!(cert.cert_risk_epoch, group.risk_epoch);
    assert_eq!(cert.cert_asset_set_epoch, group.asset_set_epoch);
    assert_eq!(cert.active_bitmap_at_cert, long.active_bitmap);
    assert_eq!(group.ensure_favorable_action_allowed(&long), Ok(()));

    group.asset_set_epoch += 1;
    kani::cover!(
        long.health_cert.cert_asset_set_epoch != group.asset_set_epoch,
        "v16 health certificate stale after asset-set epoch advances"
    );
    assert_eq!(
        group.ensure_favorable_action_allowed(&long),
        Err(V16Error::Stale)
    );
    group.asset_set_epoch -= 1;

    group.accrue_asset_to_not_atomic(0, 1, 2, 0, true).unwrap();

    kani::cover!(
        long.health_cert.cert_oracle_epoch != group.oracle_epoch,
        "v16 health certificate stale after price epoch advances"
    );
    assert_eq!(
        group.ensure_favorable_action_allowed(&long),
        Err(V16Error::Stale)
    );

    let refreshed = group
        .full_account_refresh(&mut long, &[2; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(refreshed.cert_oracle_epoch, group.oracle_epoch);
}

#[kani::proof]
#[kani::unwind(40)]
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
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v16_favorable_locks_block_released_pnl_conversion_before_mutation() {
    let lock_case: u8 = kani::any();
    kani::assume(lock_case < 6);
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    group.deposit_not_atomic(&mut account, 100).unwrap();
    group
        .attach_leg(&mut account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    account.pnl = 5;
    group.pnl_pos_tot = 5;
    set_junior_bound(&mut group, 5);
    group.pnl_matured_pos_tot = 5;
    group.vault = group.c_tot + group.insurance + 5;
    group
        .full_account_refresh(&mut account, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    match lock_case {
        0 => group.threshold_stress_active = true,
        1 => group.bankruptcy_hlock_active = true,
        2 => group.loss_stale_active = true,
        3 => account.stale_state = true,
        4 => account.b_stale_state = true,
        _ => group.assets[0].raw_oracle_target_price = 2,
    }

    let before_vault = group.vault;
    let before_c_tot = group.c_tot;
    let before_insurance = group.insurance;
    let before_pnl_pos_tot = group.pnl_pos_tot;
    let before_pnl_pos_bound_tot = group.pnl_pos_bound_tot;
    let before_pnl_matured_pos_tot = group.pnl_matured_pos_tot;
    let before_asset_raw_target = group.assets[0].raw_oracle_target_price;
    let before_asset_effective_price = group.assets[0].effective_price;
    let before_asset_oi_long = group.assets[0].oi_eff_long_q;
    let before_asset_oi_short = group.assets[0].oi_eff_short_q;
    let before_capital = account.capital;
    let before_pnl = account.pnl;
    let before_reserved_pnl = account.reserved_pnl;
    let before_fee_credits = account.fee_credits;
    let before_last_fee_slot = account.last_fee_slot;
    let before_active_bitmap = account.active_bitmap;
    let before_health_valid = account.health_cert.valid;
    let before_leg_active = account.legs[0].active;
    let before_leg_side = account.legs[0].side;
    let before_leg_basis = account.legs[0].basis_pos_q;
    let before_leg_a_basis = account.legs[0].a_basis;
    let before_leg_k_snap = account.legs[0].k_snap;
    let before_leg_f_snap = account.legs[0].f_snap;
    let before_stale = account.stale_state;
    let before_b_stale = account.b_stale_state;
    let result = group.convert_released_pnl_to_capital_not_atomic(&mut account);

    kani::cover!(lock_case == 0, "v16 threshold-stress conversion lock");
    kani::cover!(lock_case == 1, "v16 bankruptcy h-lock conversion lock");
    kani::cover!(lock_case == 2, "v16 loss-stale conversion lock");
    kani::cover!(lock_case == 3, "v16 stale account conversion lock");
    kani::cover!(lock_case == 4, "v16 B-stale account conversion lock");
    kani::cover!(lock_case == 5, "v16 target/effective lag conversion lock");
    assert_eq!(result, Err(V16Error::LockActive));
    assert_eq!(group.vault, before_vault);
    assert_eq!(group.c_tot, before_c_tot);
    assert_eq!(group.insurance, before_insurance);
    assert_eq!(group.pnl_pos_tot, before_pnl_pos_tot);
    assert_eq!(group.pnl_pos_bound_tot, before_pnl_pos_bound_tot);
    assert_eq!(group.pnl_matured_pos_tot, before_pnl_matured_pos_tot);
    assert_eq!(
        group.assets[0].raw_oracle_target_price,
        before_asset_raw_target
    );
    assert_eq!(
        group.assets[0].effective_price,
        before_asset_effective_price
    );
    assert_eq!(group.assets[0].oi_eff_long_q, before_asset_oi_long);
    assert_eq!(group.assets[0].oi_eff_short_q, before_asset_oi_short);
    assert_eq!(account.capital, before_capital);
    assert_eq!(account.pnl, before_pnl);
    assert_eq!(account.reserved_pnl, before_reserved_pnl);
    assert_eq!(account.fee_credits, before_fee_credits);
    assert_eq!(account.last_fee_slot, before_last_fee_slot);
    assert_eq!(account.active_bitmap, before_active_bitmap);
    assert_eq!(account.health_cert.valid, before_health_valid);
    assert_eq!(account.legs[0].active, before_leg_active);
    assert_eq!(account.legs[0].side, before_leg_side);
    assert_eq!(account.legs[0].basis_pos_q, before_leg_basis);
    assert_eq!(account.legs[0].a_basis, before_leg_a_basis);
    assert_eq!(account.legs[0].k_snap, before_leg_k_snap);
    assert_eq!(account.legs[0].f_snap, before_leg_f_snap);
    assert_eq!(account.stale_state, before_stale);
    assert_eq!(account.b_stale_state, before_b_stale);
}

#[kani::proof]
#[kani::unwind(40)]
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
#[kani::unwind(40)]
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
#[kani::unwind(40)]
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
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v16_global_cross_margin_positive_leg_supports_other_leg_maintenance_without_b_domain() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(2, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut opp0 = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [9; 32], owner));
    let mut opp1 = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [10; 32], owner));
    group.deposit_not_atomic(&mut account, 1).unwrap();
    group.vault = group.vault.checked_add(3).unwrap();
    group
        .attach_leg(&mut account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut account, 1, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut opp0, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    group
        .attach_leg(&mut opp1, 1, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    group.assets[0].k_long = -2 * ADL_ONE as i128;
    group.assets[1].k_long = 3 * ADL_ONE as i128;
    group
        .add_fresh_counterparty_backing_not_atomic(3, 2 * BOUND_SCALE, 10)
        .unwrap();

    let cert = group
        .full_account_refresh(&mut account, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    kani::cover!(
        cert.certified_liq_deficit == 0,
        "v16 positive leg support covers other-leg maintenance"
    );
    assert_eq!(account.pnl, 2);
    assert_eq!(account.capital, 0);
    assert_eq!(group.c_tot, 0);
    assert_eq!(
        group.source_credit[0].fresh_reserved_backing_num,
        BOUND_SCALE
    );
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
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

fn assert_full_refresh_settles_and_scores_two_active_assets(capital_units: u128) {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(2, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut opp0 = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [9; 32], owner));
    let mut opp1 = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [10; 32], owner));
    if capital_units != 0 {
        group
            .deposit_not_atomic(&mut account, capital_units)
            .unwrap();
    }

    group
        .attach_leg(&mut account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut account, 1, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut opp0, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    group
        .attach_leg(&mut opp1, 1, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();

    group.assets[0].k_long = ADL_ONE as i128;
    group.assets[1].k_long = -2 * (ADL_ONE as i128);
    let prices = {
        let mut out = [1u64; V16_MAX_PORTFOLIO_ASSETS_N];
        out[0] = 7;
        out[1] = 11;
        out
    };
    let expected_loss0 = risk_notional_ceil(POS_SCALE, prices[0]).unwrap();
    let expected_loss1 = risk_notional_ceil(POS_SCALE, prices[1]).unwrap();

    let cert = group.full_account_refresh(&mut account, &prices).unwrap();

    assert_eq!(account.active_bitmap, bitmap(&[0, 1]));
    let expected_pnl = if capital_units == 0 { -2 } else { 0 };
    let expected_capital = capital_units.saturating_sub(2);
    assert_eq!(account.pnl, expected_pnl);
    assert_eq!(account.capital, expected_capital);
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
    assert_eq!(cert.certified_equity, capital_units as i128 - 2);
    assert_eq!(cert.active_bitmap_at_cert, bitmap(&[0, 1]));
    assert_eq!(group.validate_account_shape(&account), Ok(()));
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v16_full_refresh_settles_two_assets_with_negative_equity() {
    assert_full_refresh_settles_and_scores_two_active_assets(0);
    kani::cover!(true, "v16 two-asset refresh covers negative equity");
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v16_full_refresh_settles_two_assets_with_zero_equity() {
    assert_full_refresh_settles_and_scores_two_active_assets(2);
    kani::cover!(true, "v16 two-asset refresh covers zero equity");
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v16_full_refresh_settles_and_scores_two_active_assets() {
    assert_full_refresh_settles_and_scores_two_active_assets(20);
    kani::cover!(true, "v16 two-asset refresh covers positive equity");
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_full_refresh_clears_stale_certificate() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    group.mark_account_stale(&mut account).unwrap();
    assert_eq!(group.stale_certificate_count, 1);
    group
        .full_account_refresh(&mut account, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    kani::cover!(
        !account.stale_state,
        "v16 stale account refresh clears stale state"
    );
    assert!(!account.stale_state);
    assert_eq!(group.stale_certificate_count, 0);
    assert_eq!(group.ensure_favorable_action_allowed(&account), Ok(()));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_b_stale_blocks_refresh_and_favorable_actions() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    group
        .full_account_refresh(&mut account, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(group.ensure_favorable_action_allowed(&account), Ok(()));

    group.mark_account_b_stale(&mut account).unwrap();
    kani::cover!(
        account.b_stale_state && !account.health_cert.valid,
        "v16 b-stale invalidates prior health certificate"
    );

    assert_eq!(
        group.full_account_refresh(&mut account, &[1; V16_MAX_PORTFOLIO_ASSETS_N]),
        Err(V16Error::BStale)
    );
    assert_eq!(
        group.ensure_favorable_action_allowed(&account),
        Err(V16Error::LockActive)
    );
    assert!(account.b_stale_state);
    assert_eq!(group.b_stale_account_count, 1);
}

#[kani::proof]
#[kani::unwind(60)]
#[kani::solver(cadical)]
fn proof_v16_b_stale_trade_preflight_rolls_back_partial_side_effects() {
    let (market, account_id, owner) = concrete_ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 1);
    cfg.public_b_chunk_atoms = 1;
    let mut group = MarketGroupV16::new(market, cfg).unwrap();
    let mut long = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut short = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut long, 100).unwrap();
    group.deposit_not_atomic(&mut short, 100).unwrap();
    group.attach_leg(&mut long, 0, SideV16::Long, 1).unwrap();
    group.assets[0].b_long_num = 2;

    let before_vault = group.vault;
    let before_c_tot = group.c_tot;
    let before_insurance = group.insurance;
    let before_pnl_pos_tot = group.pnl_pos_tot;
    let before_pnl_matured_pos_tot = group.pnl_matured_pos_tot;
    let before_b_stale_count = group.b_stale_account_count;
    let before_asset = group.assets[0];
    let before_long_capital = long.capital;
    let before_long_pnl = long.pnl;
    let before_long_bitmap = long.active_bitmap;
    let before_long_b_stale = long.b_stale_state;
    let before_long_health_valid = long.health_cert.valid;
    let before_long_leg = long.legs[0];
    let before_short_capital = short.capital;
    let before_short_pnl = short.pnl;
    let before_short_bitmap = short.active_bitmap;
    let before_short_b_stale = short.b_stale_state;
    let before_short_health_valid = short.health_cert.valid;
    let before_short_leg = short.legs[0];
    let result = group.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV16 {
            asset_index: 0,
            size_q: 1,
            exec_price: 1,
            fee_bps: 0,
        },
        &[1; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(
        before_asset.b_long_num > before_long_leg.b_snap,
        "v16 trade preflight reaches partial B-stale side effect"
    );
    assert_eq!(result, Err(V16Error::BStale));
    assert_eq!(group.vault, before_vault);
    assert_eq!(group.c_tot, before_c_tot);
    assert_eq!(group.insurance, before_insurance);
    assert_eq!(group.pnl_pos_tot, before_pnl_pos_tot);
    assert_eq!(group.pnl_matured_pos_tot, before_pnl_matured_pos_tot);
    assert_eq!(group.b_stale_account_count, before_b_stale_count);
    assert_eq!(group.assets[0].b_long_num, before_asset.b_long_num);
    assert_eq!(group.assets[0].b_short_num, before_asset.b_short_num);
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
    assert_eq!(long.capital, before_long_capital);
    assert_eq!(long.pnl, before_long_pnl);
    assert_eq!(long.active_bitmap, before_long_bitmap);
    assert_eq!(long.b_stale_state, before_long_b_stale);
    assert_eq!(long.health_cert.valid, before_long_health_valid);
    assert_eq!(long.legs[0].active, before_long_leg.active);
    assert_eq!(long.legs[0].basis_pos_q, before_long_leg.basis_pos_q);
    assert_eq!(long.legs[0].b_snap, before_long_leg.b_snap);
    assert_eq!(long.legs[0].b_stale, before_long_leg.b_stale);
    assert_eq!(short.capital, before_short_capital);
    assert_eq!(short.pnl, before_short_pnl);
    assert_eq!(short.active_bitmap, before_short_bitmap);
    assert_eq!(short.b_stale_state, before_short_b_stale);
    assert_eq!(short.health_cert.valid, before_short_health_valid);
    assert_eq!(short.legs[0].active, before_short_leg.active);
    assert_eq!(short.legs[0].basis_pos_q, before_short_leg.basis_pos_q);
    assert_eq!(short.legs[0].b_snap, before_short_leg.b_snap);
    assert_eq!(short.legs[0].b_stale, before_short_leg.b_stale);
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v16_deposit_into_stale_or_b_stale_account_does_not_unlock_favorable_actions() {
    let stale_case: bool = kani::any();
    let deposit_units: u8 = kani::any();
    kani::assume(deposit_units > 0);
    kani::assume(deposit_units <= 20);
    let deposit = deposit_units as u128;
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));

    group
        .full_account_refresh(&mut account, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    if stale_case {
        group.mark_account_stale(&mut account).unwrap();
    } else {
        group.mark_account_b_stale(&mut account).unwrap();
    }
    let stale_before = group.stale_certificate_count;
    let b_stale_before = group.b_stale_account_count;

    group.deposit_not_atomic(&mut account, deposit).unwrap();

    kani::cover!(stale_case, "v16 deposit into stale account reachable");
    kani::cover!(!stale_case, "v16 deposit into B-stale account reachable");
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
#[kani::unwind(40)]
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
#[kani::unwind(40)]
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
#[kani::unwind(40)]
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
#[kani::unwind(60)]
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
#[kani::unwind(40)]
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
#[kani::unwind(40)]
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
#[kani::unwind(60)]
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

    let out = group
        .forfeit_recovery_leg_not_atomic(&mut account, 0, 1)
        .unwrap();

    kani::cover!(
        out.positive_pnl_forfeited > 0,
        "v16 dead-leg positive K/F delta is forfeited"
    );
    assert!(out.detached);
    assert_eq!(out.positive_pnl_forfeited, 3);
    assert_eq!(account.pnl, 0);
    assert_eq!(group.pnl_pos_tot, 0);
    assert_eq!(account.active_bitmap, bitmap(&[]));
    assert_eq!(group.assets[0].oi_eff_long_q, 0);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v16_dead_leg_forfeit_partial_b_progress_does_not_detach() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.mode = MarketModeV16::Recovery;
    group.attach_leg(&mut account, 0, SideV16::Long, 1).unwrap();
    group.assets[0].b_long_num = 2;

    let out = group
        .forfeit_recovery_leg_not_atomic(&mut account, 0, 1)
        .unwrap();

    kani::cover!(
        !out.detached && account.legs[0].b_stale,
        "v16 dead-leg forfeit partial B progress before detach"
    );
    assert!(!out.detached);
    assert_eq!(out.loss_settled, 0);
    assert_eq!(out.principal_used, 0);
    assert_eq!(out.insurance_used, 0);
    assert_eq!(out.residual_booked, 0);
    assert_eq!(account.legs[0].b_snap, 1);
    assert!(account.legs[0].b_stale);
    assert!(account.b_stale_state);
    assert!(account.legs[0].active);
    assert_eq!(group.assets[0].oi_eff_long_q, 1);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v16_dead_leg_forfeit_books_loss_to_opposing_domain_only() {
    let loss_units: u8 = kani::any();
    kani::assume(loss_units > 0);
    kani::assume(loss_units <= 4);
    let loss = loss_units as u128;

    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut opposing =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [10; 32], owner));
    group.mode = MarketModeV16::Recovery;
    group
        .attach_leg(&mut account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    group.assets[0].mode_long = SideModeV16::DrainOnly;
    group.assets[0].k_long = -((loss as i128) * ADL_ONE as i128);
    let b_long_before = group.assets[0].b_long_num;
    let b_short_before = group.assets[0].b_short_num;

    let out = group
        .forfeit_recovery_leg_not_atomic(&mut account, 0, loss)
        .unwrap();

    kani::cover!(
        out.residual_booked > 0,
        "v16 dead-leg negative K/F delta books durable opposing-domain loss"
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
        group.pending_domain_loss_barrier_count(0, SideV16::Short),
        Ok(0)
    );
    assert!(account.close_progress.finalized);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v16_dead_leg_forfeit_haircuts_positive_support_when_junior_impaired() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut opposing =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [11; 32], owner));
    group.mode = MarketModeV16::Recovery;
    group
        .attach_leg(&mut account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    group.assets[0].mode_long = SideModeV16::DrainOnly;
    group.assets[0].k_long = -(100 * ADL_ONE as i128);
    account.pnl = 100;
    group.pnl_pos_tot = 100;
    set_junior_bound(&mut group, 100);
    group.vault = 50;

    let out = group
        .forfeit_recovery_leg_not_atomic(&mut account, 0, 50)
        .unwrap();

    kani::cover!(
        out.support_consumed == 50 && out.junior_face_burned == 100,
        "v16 impaired positive support burns full face for haircut value"
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
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_fee_sync_uses_wide_product_and_drops_uncollectible_tail() {
    let capital: u8 = kani::any();
    kani::assume(capital > 0);
    kani::assume(capital <= 20);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group
        .deposit_not_atomic(&mut account, capital as u128)
        .unwrap();

    let charged = group
        .sync_account_fee_to_slot_not_atomic(&mut account, 2, u128::MAX)
        .unwrap();

    kani::cover!(
        charged == capital as u128,
        "v16 fee sync wide-product cap path charges available principal"
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
fn proof_v16_non_deficit_public_paths_do_not_decrease_insurance() {
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
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.capital = capital;
    group.c_tot = capital;
    group.insurance = insurance;
    group.vault = capital + insurance;
    let insurance_before = group.insurance;

    match case {
        0 => {
            group.deposit_not_atomic(&mut account, amount).unwrap();
            kani::cover!(amount > 0, "v16 deposit non-deficit insurance boundary");
            assert_eq!(group.insurance, insurance_before);
        }
        1 => {
            group
                .withdraw_not_atomic(&mut account, amount, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
                .unwrap();
            kani::cover!(amount > 0, "v16 withdraw non-deficit insurance boundary");
            assert_eq!(group.insurance, insurance_before);
        }
        2 => {
            let charged = group
                .charge_account_fee_not_atomic(&mut account, requested_fee)
                .unwrap();
            kani::cover!(
                requested_fee > 0,
                "v16 fee charge can increase but not decrease insurance"
            );
            assert_eq!(group.insurance, insurance_before + charged);
        }
        3 => {
            let profit = 3u128;
            account.pnl = profit as i128;
            group.pnl_pos_tot = profit;
            set_junior_bound(&mut group, profit);
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
            kani::cover!(true, "v16 released pnl conversion preserves insurance");
            assert_eq!(group.insurance, insurance_before);
        }
        _ => {
            group.resolve_market_not_atomic(1).unwrap();
            let outcome = group.close_resolved_account_not_atomic(&mut account, 0);
            kani::cover!(true, "v16 non-deficit resolved close preserves insurance");
            assert_eq!(
                outcome,
                Ok(ResolvedCloseOutcomeV16::Closed { payout: capital })
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
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_equity_active_accrual_requires_protective_progress() {
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

    let result = group.accrue_asset_to_not_atomic(0, 1, 2, 0, false);
    assert_eq!(result, Err(V16Error::NonProgress));
    assert_eq!(group.slot_last, 0);

    let ok = group.accrue_asset_to_not_atomic(0, 1, 2, 0, true);
    assert!(ok.is_ok());
    assert_eq!(group.slot_last, 1);
}

#[kani::proof]
#[kani::unwind(40)]
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
        group.h_lock_lane(Some(&account), false),
        Ok(HLockLaneV16::HMax)
    );
}

#[kani::proof]
#[kani::unwind(40)]
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
#[kani::unwind(40)]
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
    let stale_count: u16 = kani::any();
    let b_stale_count: u16 = kani::any();
    let negative_count: u16 = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
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
            PermissionlessCrankRequestV16 {
                now_slot: 0,
                asset_index: 0,
                effective_price: 1,
                funding_rate_e9: 0,
                action: PermissionlessCrankActionV16::Refresh,
            },
            &[1; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    kani::cover!(
        stale_count > 0 || b_stale_count > 0 || negative_count > 0,
        "v16 permissionless hinted progress ignores unrelated global account counters"
    );
    assert_eq!(out, PermissionlessProgressOutcomeV16::AccountCurrent);
    assert!(account.health_cert.valid);
    assert_eq!(group.materialized_portfolio_count, before_materialized);
    assert_eq!(group.stale_certificate_count, before_stale);
    assert_eq!(group.b_stale_account_count, before_b_stale);
    assert_eq!(group.negative_pnl_account_count, before_negative);
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v16_permissionless_refresh_can_advance_one_equity_active_segment() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut long = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut short = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group
        .attach_leg(&mut long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut short, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();

    let out = group
        .permissionless_crank_not_atomic(
            &mut long,
            PermissionlessCrankRequestV16 {
                now_slot: 3,
                asset_index: 0,
                effective_price: 2,
                funding_rate_e9: 0,
                action: PermissionlessCrankActionV16::Refresh,
            },
            &[1; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    kani::cover!(
        group.loss_stale_active && group.assets[0].slot_last == 1,
        "v16 permissionless refresh commits bounded equity-active segment"
    );
    assert_eq!(out, PermissionlessProgressOutcomeV16::AccountCurrent);
    assert_eq!(group.assets[0].slot_last, 1);
    assert_eq!(group.slot_last, 1);
    assert_eq!(group.current_slot, 3);
    assert!(group.loss_stale_active);
    assert_eq!(group.assets[0].effective_price, 2);
    assert_eq!(group.assets[0].k_long, ADL_ONE as i128);
    assert_eq!(group.assets[0].k_short, -(ADL_ONE as i128));
    assert_eq!(group.assets[0].oi_eff_long_q, POS_SCALE);
    assert_eq!(group.assets[0].oi_eff_short_q, POS_SCALE);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v16_permissionless_refresh_returns_partial_b_progress_without_accrual() {
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
    let outcome = group.permissionless_crank_not_atomic(
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
        !larger_target,
        "v16 permissionless refresh partial B target two"
    );
    kani::cover!(
        larger_target,
        "v16 permissionless refresh partial B target three"
    );
    assert!(matches!(
        outcome,
        Ok(PermissionlessProgressOutcomeV16::AccountBChunk(_))
    ));
    assert!(account.legs[0].b_stale);
    assert!(account.legs[0].b_snap > 0);
    assert!(account.legs[0].b_snap < group.assets[0].b_long_num);
    assert_eq!(group.slot_last, 0);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_permissionless_flat_refresh_is_not_protective_for_equity_active_accrual() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(2, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 1).unwrap();
    group.assets[0].oi_eff_long_q = 1;
    group.assets[0].oi_eff_short_q = 1;
    let before_asset = group.assets[0];
    let before_slot = group.slot_last;

    let outcome = group.permissionless_crank_not_atomic(
        &mut account,
        PermissionlessCrankRequestV16 {
            now_slot: 1,
            asset_index: 0,
            effective_price: 2,
            funding_rate_e9: 0,
            action: PermissionlessCrankActionV16::Refresh,
        },
        &[1; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(
        outcome == Err(V16Error::NonProgress),
        "v16 flat refresh is not protective for exposed asset accrual"
    );
    assert_eq!(outcome, Err(V16Error::NonProgress));
    assert_eq!(group.assets[0], before_asset);
    assert_eq!(group.slot_last, before_slot);
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v16_permissionless_cross_asset_liquidation_is_not_protective_for_equity_active_accrual() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(2, 0, 1)).unwrap();
    group.assets[0].oi_eff_long_q = 1;
    group.assets[0].oi_eff_short_q = 1;
    let before_asset = group.assets[0];
    let before_slot = group.slot_last;

    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.attach_leg(&mut account, 1, SideV16::Long, 1).unwrap();

    let outcome = group.permissionless_crank_not_atomic(
        &mut account,
        PermissionlessCrankRequestV16 {
            now_slot: 1,
            asset_index: 0,
            effective_price: 2,
            funding_rate_e9: 0,
            action: PermissionlessCrankActionV16::Liquidate(LiquidationRequestV16 {
                asset_index: 1,
                close_q: 1,
                fee_bps: 0,
            }),
        },
        &[1; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(
        outcome == Err(V16Error::NonProgress),
        "v16 cross-asset liquidation is not protective for exposed asset accrual"
    );
    assert_eq!(outcome, Err(V16Error::NonProgress));
    assert_eq!(group.assets[0], before_asset);
    assert_eq!(group.slot_last, before_slot);
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v16_worst_case_hinted_progress_actions_are_total_and_bounded() {
    let case: u8 = kani::any();
    kani::assume(case < 4);
    let (market, account_id, owner) = concrete_ids();
    let base_req = PermissionlessCrankRequestV16 {
        now_slot: 0,
        asset_index: 0,
        effective_price: 1,
        funding_rate_e9: 0,
        action: PermissionlessCrankActionV16::Refresh,
    };

    match case {
        0 => {
            let mut group =
                MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
            let mut account =
                PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
            group.deposit_not_atomic(&mut account, 1).unwrap();
            let outcome = group.permissionless_crank_not_atomic(
                &mut account,
                base_req,
                &[1; V16_MAX_PORTFOLIO_ASSETS_N],
            );
            kani::cover!(true, "v16 hinted refresh-current branch reachable");
            assert_eq!(
                outcome,
                Ok(PermissionlessProgressOutcomeV16::AccountCurrent)
            );
            assert!(account.health_cert.valid);
        }
        1 => {
            let mut cfg = V16Config::public_user_fund(1, 0, 1);
            cfg.public_b_chunk_atoms = 1;
            let mut group = MarketGroupV16::new(market, cfg).unwrap();
            let mut account =
                PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
            group.attach_leg(&mut account, 0, SideV16::Long, 1).unwrap();
            group.assets[0].b_long_num = 2;
            let outcome = group.permissionless_crank_not_atomic(
                &mut account,
                PermissionlessCrankRequestV16 {
                    action: PermissionlessCrankActionV16::SettleB { asset_index: 0 },
                    ..base_req
                },
                &[1; V16_MAX_PORTFOLIO_ASSETS_N],
            );
            kani::cover!(true, "v16 hinted settle-B branch reachable");
            match outcome {
                Ok(PermissionlessProgressOutcomeV16::AccountBChunk(chunk)) => {
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
                MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
            let mut account =
                PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
            group
                .attach_leg(&mut account, 0, SideV16::Long, POS_SCALE as i128)
                .unwrap();
            let _opposite =
                attach_opposite_for_live_oi(&mut group, 0, SideV16::Long, POS_SCALE, 99);
            let outcome = group.permissionless_crank_not_atomic(
                &mut account,
                PermissionlessCrankRequestV16 {
                    action: PermissionlessCrankActionV16::Liquidate(LiquidationRequestV16 {
                        asset_index: 0,
                        close_q: POS_SCALE,
                        fee_bps: 0,
                    }),
                    ..base_req
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
        _ => {
            let mut group =
                MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
            let mut account =
                PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
            let reason = PermissionlessRecoveryReasonV16::BelowProgressFloor;
            let outcome = group.permissionless_crank_not_atomic(
                &mut account,
                PermissionlessCrankRequestV16 {
                    action: PermissionlessCrankActionV16::Recover(reason),
                    ..base_req
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
    }
}

fn assert_permissionless_crank_liquidation_books_bankruptcy_and_advances_accrual(
    loss_atoms: u128,
    insurance_atoms: u128,
) {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut victim =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut opposite = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [9; 32], owner));
    group.attach_leg(&mut victim, 0, SideV16::Long, 1).unwrap();
    group
        .attach_leg(&mut opposite, 0, SideV16::Short, -1)
        .unwrap();
    group.vault = insurance_atoms;
    group.insurance = insurance_atoms;
    victim.pnl = -(loss_atoms as i128);
    group.negative_pnl_account_count = 1;

    let out = group
        .permissionless_crank_not_atomic(
            &mut victim,
            PermissionlessCrankRequestV16 {
                now_slot: 1,
                asset_index: 0,
                effective_price: 1,
                funding_rate_e9: 0,
                action: PermissionlessCrankActionV16::Liquidate(LiquidationRequestV16 {
                    asset_index: 0,
                    close_q: 1,
                    fee_bps: 0,
                }),
            },
            &[1; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    let expected_insurance_used = loss_atoms.min(insurance_atoms);

    assert_eq!(out, PermissionlessProgressOutcomeV16::AccountCurrent);
    assert_eq!(group.vault, insurance_atoms);
    assert_eq!(group.insurance, insurance_atoms - expected_insurance_used);
    assert_eq!(victim.pnl, 0);
    assert_eq!(victim.active_bitmap, bitmap(&[]));
    assert_eq!(group.negative_pnl_account_count, 0);
    assert_eq!(group.assets[0].oi_eff_long_q, 0);
    assert_eq!(group.assets[0].oi_eff_short_q, 0);
    assert_eq!(group.slot_last, 1);
    assert_eq!(group.current_slot, 1);
    assert!(group.bankruptcy_hlock_active);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(95)]
#[kani::solver(cadical)]
fn proof_v16_permissionless_crank_liquidation_fully_insured_advances_accrual() {
    assert_permissionless_crank_liquidation_books_bankruptcy_and_advances_accrual(2, 3);
    kani::cover!(
        true,
        "v16 permissionless crank liquidation fully insured path"
    );
}

#[kani::proof]
#[kani::unwind(95)]
#[kani::solver(cadical)]
fn proof_v16_permissionless_crank_liquidation_insurance_plus_residual_advances_accrual() {
    assert_permissionless_crank_liquidation_books_bankruptcy_and_advances_accrual(3, 1);
    kani::cover!(
        true,
        "v16 permissionless crank liquidation insurance plus residual path"
    );
}

#[kani::proof]
#[kani::unwind(95)]
#[kani::solver(cadical)]
fn proof_v16_permissionless_crank_liquidation_uninsured_residual_advances_accrual() {
    assert_permissionless_crank_liquidation_books_bankruptcy_and_advances_accrual(2, 0);
    kani::cover!(
        true,
        "v16 permissionless crank liquidation uninsured residual path"
    );
}

#[kani::proof]
#[kani::unwind(40)]
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
#[kani::unwind(40)]
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
#[kani::unwind(40)]
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
        },
        &[1; V16_MAX_PORTFOLIO_ASSETS_N],
    );
    assert_eq!(result, Err(V16Error::InvalidConfig));
    assert_eq!(long.active_bitmap, bitmap(&[]));
    assert_eq!(short.active_bitmap, bitmap(&[]));
}

#[kani::proof]
#[kani::unwind(60)]
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
#[kani::unwind(60)]
#[kani::solver(cadical)]
fn proof_v16_risk_increasing_trade_requires_initial_health_before_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut underfunded_long =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut funded_short =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut funded_short, 10_000).unwrap();
    let before_group = group;
    let before_long = underfunded_long;
    let before_short = funded_short;

    let result = group.execute_trade_with_fee_not_atomic(
        &mut underfunded_long,
        &mut funded_short,
        TradeRequestV16 {
            asset_index: 0,
            size_q: 1,
            exec_price: 100,
            fee_bps: 0,
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
#[kani::unwind(50)]
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
    let before_group = group;
    let before_long = long;
    let before_short = short;

    let result = group.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV16 {
            asset_index: 0,
            size_q: 1,
            exec_price: 1,
            fee_bps: 0,
        },
        &[1; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(
        long.legs[1].b_stale,
        "v16 trade hint with toxic unhinted active leg reachable"
    );
    assert_eq!(result, Err(V16Error::BStale));
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
    assert_eq!(
        group.assets[1].oi_eff_long_q,
        before_group.assets[1].oi_eff_long_q
    );
    assert_eq!(
        group.assets[1].oi_eff_short_q,
        before_group.assets[1].oi_eff_short_q
    );
    assert_eq!(long.capital, before_long.capital);
    assert_eq!(long.pnl, before_long.pnl);
    assert_eq!(long.active_bitmap, before_long.active_bitmap);
    assert_eq!(long.legs[0], before_long.legs[0]);
    assert_eq!(long.legs[1], before_long.legs[1]);
    assert_eq!(short.capital, before_short.capital);
    assert_eq!(short.pnl, before_short.pnl);
    assert_eq!(short.active_bitmap, before_short.active_bitmap);
    assert_eq!(short.legs[0], before_short.legs[0]);
}

#[kani::proof]
#[kani::unwind(70)]
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
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_hlock_allows_risk_increasing_trade_with_principal_margin() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut long = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut short = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut long, 100).unwrap();
    group.deposit_not_atomic(&mut short, 100).unwrap();
    group.threshold_stress_active = true;

    let result = group.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV16 {
            asset_index: 0,
            size_q: 1,
            exec_price: 1,
            fee_bps: 0,
        },
        &[1; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(
        result.is_ok(),
        "v16 h-lock risk-increasing trade principal-only margin lane reachable"
    );
    assert!(result.is_ok());
    assert_eq!(long.active_bitmap, bitmap(&[0]));
    assert_eq!(short.active_bitmap, bitmap(&[0]));
    assert_eq!(long.legs[0].basis_pos_q, 1);
    assert_eq!(short.legs[0].basis_pos_q, -1);
    assert_eq!(group.assets[0].oi_eff_long_q, 1);
    assert_eq!(group.assets[0].oi_eff_short_q, 1);
    assert_eq!(group.insurance, 0);
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v16_loss_stale_blocks_risk_increasing_trade_before_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut long = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut short = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut long, 100).unwrap();
    group.deposit_not_atomic(&mut short, 100).unwrap();
    group.loss_stale_active = true;

    let before_vault = group.vault;
    let before_c_tot = group.c_tot;
    let before_insurance = group.insurance;
    let before_long_capital = long.capital;
    let before_short_capital = short.capital;
    let result = group.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV16 {
            asset_index: 0,
            size_q: 1,
            exec_price: 1,
            fee_bps: 0,
        },
        &[1; V16_MAX_PORTFOLIO_ASSETS_N],
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
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v16_hlock_risk_increasing_trade_rejects_positive_credit_dependency_without_mutation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut long = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut short = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    long.pnl = 10;
    short.pnl = 10;
    group.pnl_pos_tot = 20;
    set_junior_bound(&mut group, 20);
    group.vault = 20;
    group.threshold_stress_active = true;

    let before_vault = group.vault;
    let before_insurance = group.insurance;
    let before_c_tot = group.c_tot;
    let before_pnl_pos_tot = group.pnl_pos_tot;
    let before_pnl_pos_bound_tot = group.pnl_pos_bound_tot;
    let before_long_active = long.active_bitmap;
    let before_short_active = short.active_bitmap;
    let before_long_pnl = long.pnl;
    let before_short_pnl = short.pnl;
    let result = group.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV16 {
            asset_index: 0,
            size_q: 1,
            exec_price: 1,
            fee_bps: 0,
        },
        &[1; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(
        result == Err(V16Error::LockActive),
        "v16 h-lock risk-increasing positive-credit dependency rejection reachable"
    );
    assert_eq!(result, Err(V16Error::LockActive));
    assert_eq!(group.vault, before_vault);
    assert_eq!(group.insurance, before_insurance);
    assert_eq!(group.c_tot, before_c_tot);
    assert_eq!(group.pnl_pos_tot, before_pnl_pos_tot);
    assert_eq!(group.pnl_pos_bound_tot, before_pnl_pos_bound_tot);
    assert_eq!(long.active_bitmap, before_long_active);
    assert_eq!(short.active_bitmap, before_short_active);
    assert_eq!(long.pnl, before_long_pnl);
    assert_eq!(short.pnl, before_short_pnl);
    assert_eq!(group.assets[0].oi_eff_long_q, 0);
    assert_eq!(group.assets[0].oi_eff_short_q, 0);
}

#[kani::proof]
#[kani::unwind(40)]
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
        },
        &[1; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(result, Err(V16Error::LockActive));
    assert_eq!(long.active_bitmap, bitmap(&[]));
    assert_eq!(short.active_bitmap, bitmap(&[]));
    assert_eq!(group.insurance, 0);
}

#[kani::proof]
#[kani::unwind(50)]
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
    group.threshold_stress_active = true;

    let result = group.execute_trade_with_fee_not_atomic(
        &mut reducing_short,
        &mut reducing_long,
        TradeRequestV16 {
            asset_index: 0,
            size_q: 5,
            exec_price: 1,
            fee_bps: 0,
        },
        &[1; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    assert!(result.is_ok());
    assert_eq!(reducing_short.legs[0].basis_pos_q, -5);
    assert_eq!(reducing_long.legs[0].basis_pos_q, 5);
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v16_hlock_withdraw_uses_no_positive_credit_lane() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 20).unwrap();
    group
        .attach_leg(&mut account, 0, SideV16::Long, 10)
        .unwrap();
    account.pnl = 100;
    group.pnl_pos_tot = 100;
    set_junior_bound(&mut group, 100);
    group.threshold_stress_active = true;

    let result =
        group.withdraw_not_atomic(&mut account, 11, &[1_000_000; V16_MAX_PORTFOLIO_ASSETS_N]);

    assert_eq!(result, Err(V16Error::InvalidConfig));
}

#[kani::proof]
#[kani::unwind(70)]
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
#[kani::unwind(50)]
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
#[kani::unwind(90)]
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
#[kani::unwind(120)]
#[kani::solver(cadical)]
fn proof_v16_source_backed_open_conversion_rejects_for_configured_domain_1() {
    assert_v16_source_backed_open_conversion_rejects_for_configured_domain(1);
    kani::cover!(true, "v16 source exposure domain 1 covered");
}

#[kani::proof]
#[kani::unwind(120)]
#[kani::solver(cadical)]
fn proof_v16_source_backed_open_conversion_rejects_for_configured_domain_2() {
    assert_v16_source_backed_open_conversion_rejects_for_configured_domain(2);
    kani::cover!(true, "v16 source exposure domain 2 covered");
}

#[kani::proof]
#[kani::unwind(120)]
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
    group
        .full_account_refresh(&mut account, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    if resolved_mode {
        group.resolve_market_not_atomic(1).unwrap();
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

    let result = group.convert_released_pnl_to_capital_not_atomic(&mut account);

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
#[kani::unwind(50)]
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
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v16_loss_stale_blocks_nonflat_withdrawal() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 100).unwrap();
    group
        .attach_leg(&mut account, 0, SideV16::Long, 10)
        .unwrap();
    group.loss_stale_active = true;

    let result = group.withdraw_not_atomic(&mut account, 10, &[1; V16_MAX_PORTFOLIO_ASSETS_N]);

    assert_eq!(result, Err(V16Error::LockActive));
}

#[kani::proof]
#[kani::unwind(40)]
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
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_resolved_receipt_tracks_paid_effective_and_bound_refinement_topup() {
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

    let first = group.close_resolved_account_not_atomic(&mut account, 0);

    kani::cover!(
        extra_num == 1,
        "v16 resolved receipt top-up covers minimum scaled remainder"
    );
    kani::cover!(
        account.resolved_payout_receipt.present
            && group
                .resolved_payout_ledger
                .terminal_claim_bound_unreceipted_num
                == extra_num as u128,
        "v16 resolved receipt leaves only scaled unreceipted remainder"
    );
    assert_eq!(first, Ok(ResolvedCloseOutcomeV16::Closed { payout: 0 }));
    assert_eq!(
        account.resolved_payout_receipt.terminal_positive_claim_face,
        1
    );
    assert_eq!(account.resolved_payout_receipt.paid_effective, 0);
    assert_eq!(
        group
            .resolved_payout_ledger
            .terminal_claim_exact_receipts_num,
        BOUND_SCALE
    );
    assert_eq!(
        group
            .resolved_payout_ledger
            .terminal_claim_bound_unreceipted_num,
        extra_num as u128
    );

    group
        .refine_resolved_unreceipted_bound_not_atomic(extra_num as u128)
        .unwrap();
    let topup = group.claim_resolved_payout_topup_not_atomic(&mut account);

    kani::cover!(
        account.resolved_payout_receipt.finalized,
        "v16 resolved receipt finalizes after bound refinement top-up"
    );
    assert_eq!(topup, Ok(1));
    assert_eq!(account.resolved_payout_receipt.paid_effective, 1);
    assert!(account.resolved_payout_receipt.finalized);
    assert_eq!(group.vault, 0);
}

#[kani::proof]
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v16_unfinalized_resolved_receipt_blocks_account_close_until_topup() {
    let extra_num: u8 = kani::any();
    kani::assume((1..=3).contains(&extra_num));
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.create_portfolio_account(&account).unwrap();
    group.vault = 1;
    account.pnl = 1;
    group.pnl_pos_tot = 1;
    group.pnl_pos_bound_tot_num = BOUND_SCALE + extra_num as u128;
    group.pnl_pos_bound_tot = 2;
    group.resolve_market_not_atomic(1).unwrap();

    let first = group.close_resolved_account_not_atomic(&mut account, 0);

    kani::cover!(
        first == Ok(ResolvedCloseOutcomeV16::Closed { payout: 0 })
            && account.resolved_payout_receipt.present
            && !account.resolved_payout_receipt.finalized,
        "v16 partial resolved receipt blocks account close before top-up"
    );
    assert_eq!(first, Ok(ResolvedCloseOutcomeV16::Closed { payout: 0 }));
    assert!(account.resolved_payout_receipt.present);
    assert!(!account.resolved_payout_receipt.finalized);
    assert_eq!(
        group.close_portfolio_account(&account),
        Err(V16Error::LockActive)
    );
    assert_eq!(group.materialized_portfolio_count, 1);

    group
        .refine_resolved_unreceipted_bound_not_atomic(extra_num as u128)
        .unwrap();
    let topup = group.claim_resolved_payout_topup_not_atomic(&mut account);

    assert_eq!(topup, Ok(1));
    assert!(account.resolved_payout_receipt.finalized);
    assert_eq!(group.close_portfolio_account(&account), Ok(()));
    assert_eq!(group.materialized_portfolio_count, 0);
}

#[kani::proof]
#[kani::unwind(40)]
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
#[kani::unwind(50)]
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
#[kani::unwind(45)]
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
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v16_pending_domain_barrier_does_not_freeze_unrelated_positive_credit() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(2, 0, 1)).unwrap();
    let mut profitable =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut opposite =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [77; 32], owner));
    group.deposit_not_atomic(&mut profitable, 100).unwrap();
    group.deposit_not_atomic(&mut opposite, 100).unwrap();
    group
        .attach_leg(&mut profitable, 1, SideV16::Long, 1)
        .unwrap();
    group
        .attach_leg(&mut opposite, 1, SideV16::Short, -1)
        .unwrap();
    profitable.pnl = 5;
    group.pnl_pos_tot = 5;
    group.pnl_matured_pos_tot = 5;
    set_junior_bound(&mut group, 5);
    group.vault = group.c_tot + 5;
    group.pending_domain_loss_barriers[1] = 1;
    group
        .full_account_refresh(&mut profitable, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    let result = group.convert_released_pnl_to_capital_not_atomic(&mut profitable);

    kani::cover!(
        result == Ok(5),
        "v16 unrelated-domain positive-credit conversion remains reachable"
    );
    assert_eq!(result, Ok(5));
    assert_eq!(profitable.pnl, 0);
    assert_eq!(profitable.capital, 105);
    assert_eq!(group.c_tot, 205);
    assert_eq!(group.pending_domain_loss_barriers[1], 1);
    assert_eq!(group.assets[1].oi_eff_long_q, 1);
    assert_eq!(group.assets[1].oi_eff_short_q, 1);
    assert_eq!(group.assert_public_invariants(), Ok(()));
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
#[kani::unwind(45)]
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
#[kani::unwind(40)]
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
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v16_bankrupt_liquidation_consumes_insurance_before_social_loss() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut opposing = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group.vault = 4;
    group.insurance = 4;
    account.pnl = -9;
    group.negative_pnl_account_count = 1;
    group.attach_leg(&mut account, 0, SideV16::Long, 1).unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV16::Short, -1)
        .unwrap();

    let out = group
        .liquidate_account_not_atomic(
            &mut account,
            LiquidationRequestV16 {
                asset_index: 0,
                close_q: 1,
                fee_bps: 0,
            },
            &[1; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(out.insurance_used, 4);
    assert_eq!(out.residual_booked, 5);
    assert_eq!(out.explicit_loss, 0);
    assert_eq!(group.vault, 4);
    assert_eq!(group.insurance, 0);
    assert_eq!(account.pnl, 0);
    assert_eq!(account.active_bitmap, bitmap(&[]));
}

fn assert_domain_insurance_budget_caps_bankruptcy_spend(domain_budget: u128) {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut opposing = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group.vault = 4;
    group.insurance = 4;
    group.insurance_domain_budget = [0; V16_DOMAIN_COUNT];
    group.insurance_domain_budget[1] = domain_budget;
    account.pnl = -9;
    group.negative_pnl_account_count = 1;
    group.attach_leg(&mut account, 0, SideV16::Long, 1).unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV16::Short, -1)
        .unwrap();

    let out = group
        .liquidate_account_not_atomic(
            &mut account,
            LiquidationRequestV16 {
                asset_index: 0,
                close_q: 1,
                fee_bps: 0,
            },
            &[1; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(out.insurance_used, domain_budget);
    assert_eq!(out.residual_booked, 9 - domain_budget);
    assert_eq!(out.explicit_loss, 0);
    assert_eq!(group.insurance, 4 - domain_budget);
    assert_eq!(group.insurance_domain_spent[1], domain_budget);
    assert_eq!(group.insurance_domain_spent[0], 0);
    assert_eq!(account.pnl, 0);
    assert_eq!(account.active_bitmap, bitmap(&[]));
}

#[kani::proof]
#[kani::unwind(55)]
#[kani::solver(cadical)]
fn proof_v16_domain_insurance_budget_zero_caps_bankruptcy_spend() {
    assert_domain_insurance_budget_caps_bankruptcy_spend(0);
    kani::cover!(true, "v16 domain insurance proof covers zero budget");
}

#[kani::proof]
#[kani::unwind(55)]
#[kani::solver(cadical)]
fn proof_v16_domain_insurance_budget_one_caps_bankruptcy_spend() {
    assert_domain_insurance_budget_caps_bankruptcy_spend(1);
    kani::cover!(true, "v16 domain insurance proof covers one atom budget");
}

#[kani::proof]
#[kani::unwind(55)]
#[kani::solver(cadical)]
fn proof_v16_domain_insurance_budget_two_caps_bankruptcy_spend() {
    assert_domain_insurance_budget_caps_bankruptcy_spend(2);
    kani::cover!(true, "v16 domain insurance proof covers two atom budget");
}

#[kani::proof]
#[kani::unwind(55)]
#[kani::solver(cadical)]
fn proof_v16_domain_insurance_budget_three_caps_bankruptcy_spend() {
    assert_domain_insurance_budget_caps_bankruptcy_spend(3);
    kani::cover!(true, "v16 domain insurance proof covers three atom budget");
}

#[kani::proof]
#[kani::unwind(55)]
#[kani::solver(cadical)]
fn proof_v16_domain_insurance_budget_full_caps_bankruptcy_spend() {
    assert_domain_insurance_budget_caps_bankruptcy_spend(4);
    kani::cover!(true, "v16 domain insurance proof covers full local budget");
}

#[kani::proof]
#[kani::unwind(60)]
#[kani::solver(cadical)]
fn proof_v16_long_liquidation_residual_charges_short_domain() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut bankrupt =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut opposing = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group.vault = 4;
    group.insurance = 4;
    group.insurance_domain_budget = [0; V16_DOMAIN_COUNT];
    group.insurance_domain_budget[1] = 3;
    bankrupt.pnl = -5;
    group.negative_pnl_account_count = 1;
    group
        .attach_leg(&mut bankrupt, 0, SideV16::Long, 1)
        .unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV16::Short, -1)
        .unwrap();

    let out = group
        .liquidate_account_not_atomic(
            &mut bankrupt,
            LiquidationRequestV16 {
                asset_index: 0,
                close_q: 1,
                fee_bps: 0,
            },
            &[1; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    kani::cover!(true, "v16 long liquidation charges short domain");
    assert_eq!(out.insurance_used, 3);
    assert_eq!(out.residual_booked, 2);
    assert_eq!(group.insurance_domain_spent[1], 3);
    assert_eq!(group.insurance_domain_spent[0], 0);
    assert_eq!(group.insurance, 1);
    assert_eq!(bankrupt.pnl, 0);
    assert_eq!(bankrupt.active_bitmap, bitmap(&[]));
}

#[kani::proof]
#[kani::unwind(60)]
#[kani::solver(cadical)]
fn proof_v16_short_liquidation_residual_charges_long_domain() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut bankrupt =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut opposing = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group.vault = 4;
    group.insurance = 4;
    group.insurance_domain_budget = [0; V16_DOMAIN_COUNT];
    group.insurance_domain_budget[0] = 3;
    bankrupt.pnl = -5;
    group.negative_pnl_account_count = 1;
    group
        .attach_leg(&mut bankrupt, 0, SideV16::Short, -1)
        .unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV16::Long, 1)
        .unwrap();

    let out = group
        .liquidate_account_not_atomic(
            &mut bankrupt,
            LiquidationRequestV16 {
                asset_index: 0,
                close_q: 1,
                fee_bps: 0,
            },
            &[1; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    kani::cover!(true, "v16 short liquidation charges long domain");
    assert_eq!(out.insurance_used, 3);
    assert_eq!(out.residual_booked, 2);
    assert_eq!(group.insurance_domain_spent[0], 3);
    assert_eq!(group.insurance_domain_spent[1], 0);
    assert_eq!(group.insurance, 1);
    assert_eq!(bankrupt.pnl, 0);
    assert_eq!(bankrupt.active_bitmap, bitmap(&[]));
}

#[kani::proof]
#[kani::unwind(60)]
#[kani::solver(cadical)]
fn proof_v16_bad_asset_cannot_spend_unrelated_domain_insurance_budget() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut bankrupt =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut opposing = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [9; 32], owner));
    group.vault = 1;
    group.insurance = 1;
    group.insurance_domain_budget = [0; V16_DOMAIN_COUNT];
    group.insurance_domain_budget[0] = 1;
    bankrupt.pnl = -1;
    group.negative_pnl_account_count = 1;
    group
        .attach_leg(&mut bankrupt, 0, SideV16::Long, 1)
        .unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV16::Short, -1)
        .unwrap();

    let out = group
        .liquidate_account_not_atomic(
            &mut bankrupt,
            LiquidationRequestV16 {
                asset_index: 0,
                close_q: 1,
                fee_bps: 0,
            },
            &[1; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    kani::cover!(
        out.residual_booked != 0,
        "v16 unrelated insurance budget leaves bad-asset residual on domain"
    );
    assert_eq!(out.insurance_used, 0);
    assert_eq!(out.residual_booked, 1);
    assert_eq!(group.insurance, 1);
    assert_eq!(group.insurance_domain_spent[0], 0);
    assert_eq!(group.insurance_domain_spent[1], 0);
    assert_eq!(
        group.pending_domain_loss_barrier_count(0, SideV16::Short),
        Ok(0)
    );
    assert_eq!(bankrupt.pnl, 0);
    assert_eq!(bankrupt.active_bitmap, bitmap(&[]));
}

fn assert_bankrupt_liquidation_cannot_free_exposure_before_residual_durable(residual: i128) {
    let (market, account_id, owner) = concrete_ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 1);
    cfg.public_b_chunk_atoms = 1;
    let mut group = MarketGroupV16::new(market, cfg).unwrap();
    let mut bankrupt =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut opposing = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));

    group
        .attach_leg(&mut bankrupt, 0, SideV16::Long, 4)
        .unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV16::Short, -4)
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
        LiquidationRequestV16 {
            asset_index: 0,
            close_q: 4,
            fee_bps: 0,
        },
        &[1; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(
        result == Err(V16Error::RecoveryRequired),
        "v16 partial residual recovery path reachable"
    );
    assert_eq!(result, Err(V16Error::RecoveryRequired));
    assert_eq!(
        group.recovery_reason,
        Some(PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress)
    );
    assert_eq!(bankrupt.active_bitmap, before_bitmap);
    assert_eq!(bankrupt.legs[0].basis_pos_q, before_basis);
    assert_eq!(bankrupt.pnl, before_pnl);
    assert_eq!(group.assets[0].b_short_num, before_b_short);
}

#[kani::proof]
#[kani::unwind(60)]
#[kani::solver(cadical)]
fn proof_v16_bankrupt_liquidation_cannot_free_exposure_before_two_atom_residual_durable() {
    assert_bankrupt_liquidation_cannot_free_exposure_before_residual_durable(-2);
    kani::cover!(true, "v16 residual durability proof covers two atoms");
}

#[kani::proof]
#[kani::unwind(60)]
#[kani::solver(cadical)]
fn proof_v16_bankrupt_liquidation_cannot_free_exposure_before_three_atom_residual_durable() {
    assert_bankrupt_liquidation_cannot_free_exposure_before_residual_durable(-3);
    kani::cover!(true, "v16 residual durability proof covers three atoms");
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
    let mut opposing = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));

    group.vault = insurance;
    group.insurance = insurance;
    account.pnl = -5;
    group.negative_pnl_account_count = 1;
    group.attach_leg(&mut account, 0, SideV16::Long, 1).unwrap();
    group
        .attach_leg(&mut opposing, 0, SideV16::Short, -1)
        .unwrap();

    let out = group
        .liquidate_account_not_atomic(
            &mut account,
            LiquidationRequestV16 {
                asset_index: 0,
                close_q: 1,
                fee_bps: 0,
            },
            &[1; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(out.fee_charged, 0);
    assert_eq!(out.insurance_used, insurance);
    assert_eq!(group.insurance, 0);
    assert_eq!(out.residual_booked, 5 - insurance);
    assert_eq!(out.explicit_loss, 0);
    assert_eq!(account.pnl, 0);
    assert_eq!(account.active_bitmap, bitmap(&[]));
}

#[kani::proof]
#[kani::unwind(55)]
#[kani::solver(cadical)]
fn proof_v16_bankrupt_liquidation_excludes_fee_from_residual_with_zero_insurance() {
    assert_bankrupt_liquidation_excludes_fee_from_residual_and_spends_insurance_once(0);
    kani::cover!(
        true,
        "v16 bankrupt liquidation zero-insurance path reachable"
    );
}

#[kani::proof]
#[kani::unwind(55)]
#[kani::solver(cadical)]
fn proof_v16_bankrupt_liquidation_spends_one_insurance_atom_once() {
    assert_bankrupt_liquidation_excludes_fee_from_residual_and_spends_insurance_once(1);
    kani::cover!(
        true,
        "v16 bankrupt liquidation one-insurance path reachable"
    );
}

#[kani::proof]
#[kani::unwind(55)]
#[kani::solver(cadical)]
fn proof_v16_bankrupt_liquidation_spends_two_insurance_atoms_once() {
    assert_bankrupt_liquidation_excludes_fee_from_residual_and_spends_insurance_once(2);
    kani::cover!(
        true,
        "v16 bankrupt liquidation partial-insurance path reachable"
    );
}

#[kani::proof]
#[kani::unwind(50)]
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

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_b_residual_booking_makes_durable_progress_or_fails_closed() {
    let residual_units: u8 = kani::any();
    kani::assume(residual_units <= 4);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group
        .attach_leg(&mut account, 0, SideV16::Short, -1)
        .unwrap();

    let before_b = group.assets[0].b_short_num;
    let residual = residual_units as u128;
    let result =
        group.book_bankruptcy_residual_chunk_for_account(&mut account, 0, SideV16::Long, residual);
    if residual == 0 {
        assert_eq!(result.unwrap().remaining_after, 0);
        assert_eq!(group.assets[0].b_short_num, before_b);
    } else {
        let out = result.unwrap();
        kani::cover!(out.booked_loss > 0, "v16 residual B booking reachable");
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
#[kani::unwind(40)]
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
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v16_pending_domain_barrier_blocks_participants_until_residual_finalized() {
    let (market, account_id, owner) = concrete_ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 1);
    cfg.public_b_chunk_atoms = 1;
    let mut group = MarketGroupV16::new(market, cfg).unwrap();
    let mut bankrupt =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut participant =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    let mut joiner = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [5; 32], owner));

    group
        .attach_leg(&mut participant, 0, SideV16::Short, -10)
        .unwrap();
    let first = group
        .book_bankruptcy_residual_chunk_for_account(&mut bankrupt, 0, SideV16::Long, 2)
        .unwrap();
    kani::cover!(
        first.booked_loss == 1,
        "v16 pending domain barrier partial B booking reachable"
    );
    assert_eq!(first.booked_loss, 1);
    assert!(matches!(
        group.pending_domain_loss_barrier_count(0, SideV16::Short),
        Ok(1)
    ));
    assert!(matches!(
        group.clear_leg(&mut participant, 0),
        Err(V16Error::LockActive)
    ));
    assert!(matches!(
        group.attach_leg(&mut joiner, 0, SideV16::Short, -1),
        Err(V16Error::LockActive)
    ));
    assert!(matches!(
        group.h_lock_lane(Some(&participant), false),
        Ok(HLockLaneV16::HMax)
    ));

    let second = group
        .book_bankruptcy_residual_chunk_for_account(&mut bankrupt, 0, SideV16::Long, 2)
        .unwrap();
    assert_eq!(second.booked_loss, 1);
    assert!(bankrupt.close_progress.finalized);
    assert!(matches!(
        group.pending_domain_loss_barrier_count(0, SideV16::Short),
        Ok(0)
    ));
    assert!(matches!(
        group.clear_leg(&mut participant, 0),
        Err(V16Error::Stale)
    ));
    let b_first = group
        .settle_account_b_chunk(&mut participant, 0, u128::MAX)
        .unwrap();
    assert_eq!(b_first.remaining_after, 1);
    let b_second = group
        .settle_account_b_chunk(&mut participant, 0, u128::MAX)
        .unwrap();
    assert_eq!(b_second.remaining_after, 0);
    assert!(matches!(group.clear_leg(&mut participant, 0), Ok(())));
}

#[kani::proof]
#[kani::unwind(70)]
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
#[kani::unwind(24)]
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
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v16_pending_domain_barrier_allows_rebalance_reduction_with_weight_obligation_preserved() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut participant =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut counterparty =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [5; 32], owner));

    group.deposit_not_atomic(&mut participant, 100).unwrap();
    group
        .attach_leg(&mut participant, 0, SideV16::Short, -10)
        .unwrap();
    group
        .attach_leg(&mut counterparty, 0, SideV16::Long, 10)
        .unwrap();
    group.pending_domain_loss_barriers[1] = 1;
    kani::cover!(
        group.pending_domain_loss_barrier_count(0, SideV16::Short) == Ok(1),
        "v16 pending domain barrier with rebalance risk reduction reachable"
    );

    let before_weight_short = group.assets[0].loss_weight_sum_short;
    let before_barrier = group.pending_domain_loss_barriers[1];
    let before_participant_loss_weight = participant.legs[0].loss_weight;

    let result = group.rebalance_reduce_position_not_atomic(
        &mut participant,
        RebalanceRequestV16 {
            asset_index: 0,
            reduce_q: 5,
        },
        &[POS_SCALE as u64; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    assert!(result.is_ok());
    assert_eq!(group.assets[0].oi_eff_long_q, 5);
    assert_eq!(group.assets[0].oi_eff_short_q, 5);
    assert_eq!(group.assets[0].loss_weight_sum_short, before_weight_short);
    assert_eq!(group.pending_domain_loss_barriers[1], before_barrier);
    assert_eq!(participant.legs[0].basis_pos_q, -5);
    assert_eq!(
        participant.legs[0].loss_weight,
        before_participant_loss_weight
    );
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v16_pending_domain_barrier_allows_trade_reduction_with_weight_obligation_preserved() {
    let (market, _, owner) = concrete_ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 1);
    cfg.public_b_chunk_atoms = 1;
    let mut group = MarketGroupV16::new(market, cfg).unwrap();
    let mut participant =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    let mut counterparty =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [5; 32], owner));

    group.deposit_not_atomic(&mut participant, 100).unwrap();
    group.deposit_not_atomic(&mut counterparty, 100).unwrap();
    group
        .attach_leg(&mut participant, 0, SideV16::Short, -10)
        .unwrap();
    group
        .attach_leg(&mut counterparty, 0, SideV16::Long, 10)
        .unwrap();
    group.pending_domain_loss_barriers[1] = 1;
    kani::cover!(
        group.pending_domain_loss_barrier_count(0, SideV16::Short) == Ok(1),
        "v16 pending domain barrier with trade risk reduction reachable"
    );

    let before_weight_short = group.assets[0].loss_weight_sum_short;
    let before_barrier = group.pending_domain_loss_barriers[1];
    let before_participant_loss_weight = participant.legs[0].loss_weight;

    let result = group.execute_trade_with_fee_not_atomic(
        &mut participant,
        &mut counterparty,
        TradeRequestV16 {
            asset_index: 0,
            size_q: 5,
            exec_price: POS_SCALE as u64,
            fee_bps: 0,
        },
        &[POS_SCALE as u64; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    assert!(result.is_ok());
    assert_eq!(group.assets[0].oi_eff_long_q, 5);
    assert_eq!(group.assets[0].oi_eff_short_q, 5);
    assert_eq!(group.assets[0].loss_weight_sum_short, before_weight_short);
    assert_eq!(group.pending_domain_loss_barriers[1], before_barrier);
    assert_eq!(participant.legs[0].basis_pos_q, -5);
    assert_eq!(
        participant.legs[0].loss_weight,
        before_participant_loss_weight
    );
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v16_pending_domain_barrier_allows_full_trade_exit_as_flat_weight_obligation() {
    let (market, _, owner) = concrete_ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 1);
    cfg.public_b_chunk_atoms = 1;
    let mut group = MarketGroupV16::new(market, cfg).unwrap();
    let mut participant =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    let mut counterparty =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [5; 32], owner));

    group.deposit_not_atomic(&mut participant, 100).unwrap();
    group.deposit_not_atomic(&mut counterparty, 100).unwrap();
    group
        .attach_leg(&mut participant, 0, SideV16::Short, -10)
        .unwrap();
    group
        .attach_leg(&mut counterparty, 0, SideV16::Long, 10)
        .unwrap();
    group.pending_domain_loss_barriers[1] = 1;
    kani::cover!(
        group.pending_domain_loss_barrier_count(0, SideV16::Short) == Ok(1),
        "v16 pending domain barrier with full trade exit reachable"
    );

    let before_vault = group.vault;
    let before_insurance = group.insurance;
    let before_c_tot = group.c_tot;
    let before_weight_short = group.assets[0].loss_weight_sum_short;
    let before_barrier = group.pending_domain_loss_barriers[1];
    let before_participant_capital = participant.capital;
    let before_participant_pnl = participant.pnl;
    let before_participant_fee_credits = participant.fee_credits;
    let before_participant_bitmap = participant.active_bitmap;
    let before_participant_loss_weight = participant.legs[0].loss_weight;
    let before_counterparty_capital = counterparty.capital;
    let before_counterparty_pnl = counterparty.pnl;
    let before_counterparty_fee_credits = counterparty.fee_credits;
    let result = group.execute_trade_with_fee_not_atomic(
        &mut participant,
        &mut counterparty,
        TradeRequestV16 {
            asset_index: 0,
            size_q: 10,
            exec_price: POS_SCALE as u64,
            fee_bps: 0,
        },
        &[POS_SCALE as u64; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    assert!(result.is_ok());
    assert_eq!(group.vault, before_vault);
    assert_eq!(group.insurance, before_insurance);
    assert_eq!(group.c_tot, before_c_tot);
    assert_eq!(group.assets[0].oi_eff_long_q, 0);
    assert_eq!(group.assets[0].oi_eff_short_q, 0);
    assert_eq!(group.assets[0].loss_weight_sum_short, before_weight_short);
    assert_eq!(group.pending_domain_loss_barriers[1], before_barrier);
    assert_eq!(participant.capital, before_participant_capital);
    assert_eq!(participant.pnl, before_participant_pnl);
    assert_eq!(participant.fee_credits, before_participant_fee_credits);
    assert_eq!(participant.active_bitmap, before_participant_bitmap);
    assert_eq!(participant.legs[0].basis_pos_q, 0);
    assert_eq!(
        participant.legs[0].loss_weight,
        before_participant_loss_weight
    );
    assert_eq!(participant.legs[0].side, SideV16::Short);
    assert!(participant.legs[0].active);
    assert_eq!(counterparty.capital, before_counterparty_capital);
    assert_eq!(counterparty.pnl, before_counterparty_pnl);
    assert_eq!(counterparty.fee_credits, before_counterparty_fee_credits);
    assert_eq!(counterparty.active_bitmap, bitmap(&[]));
    assert_eq!(counterparty.legs[0].basis_pos_q, 0);
    assert_eq!(counterparty.legs[0].loss_weight, 0);
    assert!(!counterparty.legs[0].active);
    assert_eq!(
        group.clear_leg(&mut participant, 0),
        Err(V16Error::LockActive)
    );
    group.pending_domain_loss_barriers[1] = 0;
    assert!(group.clear_leg(&mut participant, 0).is_ok());
    assert_eq!(group.assets[0].loss_weight_sum_short, 0);
    assert_eq!(group.assets[0].stored_pos_count_short, 0);
    assert_eq!(group.assets[0].pending_obligation_count_short, 0);
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v16_pending_obligation_blocks_side_reset_until_clear() {
    let (market, _, owner) = concrete_ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 1);
    cfg.public_b_chunk_atoms = 1;
    let mut group = MarketGroupV16::new(market, cfg).unwrap();
    let mut participant =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    let mut counterparty =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [5; 32], owner));

    group.deposit_not_atomic(&mut participant, 100).unwrap();
    group.deposit_not_atomic(&mut counterparty, 100).unwrap();
    group
        .attach_leg(&mut participant, 0, SideV16::Short, -10)
        .unwrap();
    group
        .attach_leg(&mut counterparty, 0, SideV16::Long, 10)
        .unwrap();
    group.pending_domain_loss_barriers[1] = 1;
    group
        .execute_trade_with_fee_not_atomic(
            &mut participant,
            &mut counterparty,
            TradeRequestV16 {
                asset_index: 0,
                size_q: 10,
                exec_price: POS_SCALE as u64,
                fee_bps: 0,
            },
            &[POS_SCALE as u64; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();
    group.pending_domain_loss_barriers[1] = 0;
    kani::cover!(
        group.assets[0].pending_obligation_count_short == 1 && group.assets[0].oi_eff_short_q == 0,
        "v16 flat pending obligation before side reset reachable"
    );

    let before_weight = group.assets[0].loss_weight_sum_short;
    let before_count = group.assets[0].pending_obligation_count_short;
    let before_epoch = group.assets[0].epoch_short;
    let before_mode = group.assets[0].mode_short;
    let before_risk_epoch = group.risk_epoch;
    let reset_while_obligated = group.begin_full_drain_reset(0, SideV16::Short);

    assert_eq!(reset_while_obligated, Err(V16Error::LockActive));
    assert_eq!(group.assets[0].loss_weight_sum_short, before_weight);
    assert_eq!(group.assets[0].pending_obligation_count_short, before_count);
    assert_eq!(group.assets[0].epoch_short, before_epoch);
    assert_eq!(group.assets[0].mode_short, before_mode);
    assert_eq!(group.risk_epoch, before_risk_epoch);

    assert!(group.clear_leg(&mut participant, 0).is_ok());
    assert_eq!(group.assets[0].pending_obligation_count_short, 0);
    assert!(group.begin_full_drain_reset(0, SideV16::Short).is_ok());
    assert_eq!(group.assets[0].mode_short, SideModeV16::ResetPending);
}

#[kani::proof]
#[kani::unwind(70)]
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
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v16_pending_domain_barrier_allows_rebalance_full_exit_as_flat_weight_obligation() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut participant =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut counterparty =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [5; 32], owner));

    group.deposit_not_atomic(&mut participant, 100).unwrap();
    group
        .attach_leg(&mut participant, 0, SideV16::Short, -10)
        .unwrap();
    group
        .attach_leg(&mut counterparty, 0, SideV16::Long, 10)
        .unwrap();
    group
        .full_account_refresh(
            &mut participant,
            &[POS_SCALE as u64; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();
    group.pending_domain_loss_barriers[1] = 1;
    kani::cover!(
        group.pending_domain_loss_barrier_count(0, SideV16::Short) == Ok(1),
        "v16 pending domain barrier with rebalance escape attempt reachable"
    );

    let before_vault = group.vault;
    let before_insurance = group.insurance;
    let before_c_tot = group.c_tot;
    let before_weight_short = group.assets[0].loss_weight_sum_short;
    let before_stored_short = group.assets[0].stored_pos_count_short;
    let before_barrier = group.pending_domain_loss_barriers[1];
    let before_capital = participant.capital;
    let before_pnl = participant.pnl;
    let before_bitmap = participant.active_bitmap;
    let before_loss_weight = participant.legs[0].loss_weight;
    let result = group.rebalance_reduce_position_not_atomic(
        &mut participant,
        RebalanceRequestV16 {
            asset_index: 0,
            reduce_q: 10,
        },
        &[POS_SCALE as u64; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    assert!(result.is_ok());
    assert_eq!(group.vault, before_vault);
    assert_eq!(group.insurance, before_insurance);
    assert_eq!(group.c_tot, before_c_tot);
    assert_eq!(group.assets[0].oi_eff_short_q, 0);
    assert_eq!(group.assets[0].loss_weight_sum_short, before_weight_short);
    assert_eq!(group.assets[0].stored_pos_count_short, before_stored_short);
    assert_eq!(group.pending_domain_loss_barriers[1], before_barrier);
    assert_eq!(participant.capital, before_capital);
    assert_eq!(participant.pnl, before_pnl);
    assert_eq!(participant.active_bitmap, before_bitmap);
    assert_eq!(participant.legs[0].basis_pos_q, 0);
    assert_eq!(participant.legs[0].loss_weight, before_loss_weight);
    assert_eq!(
        group.clear_leg(&mut participant, 0),
        Err(V16Error::LockActive)
    );
    group.pending_domain_loss_barriers[1] = 0;
    assert!(group.clear_leg(&mut participant, 0).is_ok());
    assert_eq!(group.assets[0].loss_weight_sum_short, 0);
    assert_eq!(group.assets[0].stored_pos_count_short, 0);
    assert_eq!(group.assets[0].pending_obligation_count_short, 0);
}

#[kani::proof]
#[kani::unwind(60)]
#[kani::solver(cadical)]
fn proof_v16_new_close_cannot_overwrite_active_finalized_close_ledger() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(2, 0, 1)).unwrap();
    let mut bankrupt =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut opposing = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group
        .attach_leg(&mut bankrupt, 1, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut opposing, 1, SideV16::Short, -(POS_SCALE as i128))
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
    group.assets[1].k_long = -(100 * ADL_ONE as i128);
    let before_ledger = bankrupt.close_progress;
    let before_b_short = group.assets[1].b_short_num;

    let result = group.liquidate_account_not_atomic(
        &mut bankrupt,
        LiquidationRequestV16 {
            asset_index: 1,
            close_q: POS_SCALE,
            fee_bps: 0,
        },
        &[1; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(
        result == Err(V16Error::LockActive),
        "v16 active finalized close ledger blocks new close id"
    );
    assert_eq!(result, Err(V16Error::LockActive));
    assert_eq!(bankrupt.close_progress, before_ledger);
    assert_eq!(group.assets[1].b_short_num, before_b_short);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_cure_and_cancel_close_releases_barrier_and_escrow_before_irreversible_progress() {
    let prior_escrow_raw: u8 = kani::any();
    let optional_deposit_raw: u8 = kani::any();
    let prior_escrow = (prior_escrow_raw % 3) as u128;
    let optional_deposit = ((optional_deposit_raw % 3) as u128) + 1;
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

    let result = group.cure_and_cancel_close_not_atomic(
        &mut account,
        optional_deposit,
        &[100; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(
        prior_escrow != 0,
        "v16 cure cancel releases existing cancel escrow"
    );
    kani::cover!(
        optional_deposit != 0,
        "v16 cure cancel deposits fresh escrow"
    );
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
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_cure_and_cancel_rejects_irreversible_progress_before_deposit_mutation() {
    let progress_case: u8 = kani::any();
    kani::assume(progress_case < 6);
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
        4 => ledger.quantity_adl_applied_q = 1,
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

    let before_barrier = group.pending_domain_loss_barriers[1];
    let before_vault = group.vault;
    let before_c_tot = group.c_tot;
    let before_capital = account.capital;
    let before_escrow = account.cancel_deposit_escrow;
    let before_ledger = account.close_progress;
    let result =
        group.cure_and_cancel_close_not_atomic(&mut account, 3, &[100; V16_MAX_PORTFOLIO_ASSETS_N]);

    kani::cover!(
        progress_case == 0,
        "v16 cure cancel rejects support progress"
    );
    kani::cover!(
        progress_case == 1,
        "v16 cure cancel rejects insurance progress"
    );
    kani::cover!(progress_case == 2, "v16 cure cancel rejects b progress");
    kani::cover!(
        progress_case == 3,
        "v16 cure cancel rejects explicit loss progress"
    );
    kani::cover!(
        progress_case == 4,
        "v16 cure cancel rejects quantity adl progress"
    );
    kani::cover!(progress_case == 5, "v16 cure cancel rejects drift progress");
    assert!(result.is_err());
    assert_eq!(group.pending_domain_loss_barriers[1], before_barrier);
    assert_eq!(group.vault, before_vault);
    assert_eq!(group.c_tot, before_c_tot);
    assert_eq!(account.capital, before_capital);
    assert_eq!(account.cancel_deposit_escrow, before_escrow);
    assert_eq!(account.close_progress, before_ledger);
}

#[kani::proof]
#[kani::unwind(60)]
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
    let mut participant =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [44; 32], owner));
    group
        .attach_leg(&mut participant, 0, SideV16::Short, -10)
        .unwrap();

    let first =
        group.book_bankruptcy_residual_chunk_for_account(&mut bankrupt, 0, SideV16::Long, 2);
    kani::cover!(
        matches!(first, Ok(out) if out.booked_loss == 1),
        "v16 first close chunk starts configured-lifetime ledger"
    );
    assert!(matches!(first, Ok(out) if out.booked_loss == 1));
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
    let second =
        group.book_bankruptcy_residual_chunk_for_account(&mut bankrupt, 0, SideV16::Long, 2);
    kani::cover!(
        matches!(second, Ok(out) if out.booked_loss == 1),
        "v16 close continuation finalizes without refreshing lifetime"
    );
    assert!(matches!(second, Ok(out) if out.booked_loss == 1));
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
#[kani::unwind(50)]
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
#[kani::unwind(50)]
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
#[kani::unwind(50)]
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
#[kani::unwind(50)]
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
#[kani::unwind(50)]
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
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v16_invalid_trade_request_rejects_before_any_mutation() {
    assert_invalid_trade_reverts(TradeRequestV16 {
        asset_index: 1,
        size_q: POS_SCALE,
        exec_price: 100,
        fee_bps: 0,
    });
    assert_invalid_trade_reverts(TradeRequestV16 {
        asset_index: 0,
        size_q: 0,
        exec_price: 100,
        fee_bps: 0,
    });
    assert_invalid_trade_reverts(TradeRequestV16 {
        asset_index: 0,
        size_q: POS_SCALE,
        exec_price: 0,
        fee_bps: 0,
    });
    assert_invalid_trade_reverts(TradeRequestV16 {
        asset_index: 0,
        size_q: POS_SCALE,
        exec_price: 100,
        fee_bps: 11,
    });
}

fn assert_invalid_trade_reverts(request: TradeRequestV16) {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    group.config.max_trading_fee_bps = 10;
    let mut long = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut short = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    group.deposit_not_atomic(&mut long, 1_000).unwrap();
    group.deposit_not_atomic(&mut short, 1_000).unwrap();
    let before_group = group;
    let before_long = long;
    let before_short = short;

    let result = group.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        request,
        &[100; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(result, Err(V16Error::InvalidConfig));
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
    assert_eq!(long.capital, before_long.capital);
    assert_eq!(long.pnl, before_long.pnl);
    assert_eq!(long.active_bitmap, before_long.active_bitmap);
    assert_eq!(long.legs[0], before_long.legs[0]);
    assert_eq!(short.capital, before_short.capital);
    assert_eq!(short.pnl, before_short.pnl);
    assert_eq!(short.active_bitmap, before_short.active_bitmap);
    assert_eq!(short.legs[0], before_short.legs[0]);
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v16_price_accrual_refresh_matches_eager_mark_pnl() {
    assert_price_accrual_refresh_matches_eager_mark_pnl(101, 1, -1);
    assert_price_accrual_refresh_matches_eager_mark_pnl(99, -1, 1);
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v16_same_epoch_full_refresh_is_idempotent_after_price_up_settlement() {
    assert_same_epoch_refresh_idempotent_after_kf_settlement(101, 1);
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v16_same_epoch_full_refresh_is_idempotent_after_price_down_settlement() {
    assert_same_epoch_refresh_idempotent_after_kf_settlement(99, -1);
}

#[kani::proof]
#[kani::unwind(80)]
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
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v16_positive_funding_accrual_writes_f_ledger_sign_and_floor() {
    assert_funding_accrual_writes_f_ledger_sign_and_floor(1, -(ADL_ONE as i128), ADL_ONE as i128);
    kani::cover!(true, "v16 positive funding ledger sign and floor covered");
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v16_negative_funding_accrual_writes_f_ledger_sign_and_floor() {
    assert_funding_accrual_writes_f_ledger_sign_and_floor(-1, ADL_ONE as i128, -(ADL_ONE as i128));
    kani::cover!(true, "v16 negative funding ledger sign and floor covered");
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v16_positive_funding_refreshes_long_loss() {
    assert_funding_refresh_side_matches_sign_and_floor(1, true, -1);
    kani::cover!(true, "v16 positive funding refreshes long loss");
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v16_positive_funding_refreshes_short_gain() {
    assert_funding_refresh_side_matches_sign_and_floor(1, false, 1);
    kani::cover!(true, "v16 positive funding refreshes short gain");
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v16_negative_funding_refreshes_long_gain() {
    assert_funding_refresh_side_matches_sign_and_floor(-1, true, 1);
    kani::cover!(true, "v16 negative funding refreshes long gain");
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v16_negative_funding_refreshes_short_loss() {
    assert_funding_refresh_side_matches_sign_and_floor(-1, false, -1);
    kani::cover!(true, "v16 negative funding refreshes short loss");
}

#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_v16_funding_accrual_requires_bilateral_exposure() {
    let (market, account_id, owner) = concrete_ids();
    let mut long_only = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    long_only.config.max_price_move_bps_per_slot = 9_999;
    long_only.config.max_abs_funding_e9_per_slot = 1;
    long_only.assets[0].effective_price = 1_000_000_000;
    long_only.assets[0].fund_px_last = 1_000_000_000;
    long_only.assets[0].raw_oracle_target_price = 1_000_000_000;
    let mut long = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    long_only
        .attach_leg(&mut long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let long_before = long_only.assets[0];

    let out = long_only.accrue_asset_to_not_atomic(0, 1, 1_000_000_000, 1, false);
    kani::cover!(
        long_only.assets[0].oi_eff_long_q != 0 && long_only.assets[0].oi_eff_short_q == 0,
        "v16 funding rejects long-only exposure"
    );

    assert!(out.is_err());
    assert_eq!(long_only.assets[0].f_long_num, long_before.f_long_num);
    assert_eq!(long_only.assets[0].f_short_num, long_before.f_short_num);
    assert_eq!(long_only.funding_epoch, 0);

    let mut short_only = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    short_only.config.max_price_move_bps_per_slot = 9_999;
    short_only.config.max_abs_funding_e9_per_slot = 1;
    short_only.assets[0].effective_price = 1_000_000_000;
    short_only.assets[0].fund_px_last = 1_000_000_000;
    short_only.assets[0].raw_oracle_target_price = 1_000_000_000;
    let mut short = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    short_only
        .attach_leg(&mut short, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    let short_before = short_only.assets[0];

    let out = short_only.accrue_asset_to_not_atomic(0, 1, 1_000_000_000, 1, false);
    kani::cover!(
        short_only.assets[0].oi_eff_short_q != 0 && short_only.assets[0].oi_eff_long_q == 0,
        "v16 funding rejects short-only exposure"
    );

    assert!(out.is_err());
    assert_eq!(short_only.assets[0].f_long_num, short_before.f_long_num);
    assert_eq!(short_only.assets[0].f_short_num, short_before.f_short_num);
    assert_eq!(short_only.funding_epoch, 0);
}

#[kani::proof]
#[kani::unwind(50)]
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
#[kani::unwind(50)]
#[kani::solver(cadical)]
fn proof_v16_permissionless_crank_accepts_configured_funding_rate_boundaries() {
    let positive_rate: bool = kani::any();
    let (market, account_id, owner) = concrete_ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 1);
    cfg.max_price_move_bps_per_slot = 9_999;
    cfg.max_abs_funding_e9_per_slot = 1;
    let mut group = MarketGroupV16::new(market, cfg).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let supplied_rate = if positive_rate { 1 } else { -1 };

    let out = group
        .permissionless_crank_not_atomic(
            &mut account,
            PermissionlessCrankRequestV16 {
                now_slot: 1,
                asset_index: 0,
                effective_price: 1,
                funding_rate_e9: supplied_rate,
                action: PermissionlessCrankActionV16::Refresh,
            },
            &[1; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    kani::cover!(
        positive_rate && supplied_rate == group.config.max_abs_funding_e9_per_slot as i128,
        "v16 permissionless crank accepts positive funding boundary"
    );
    kani::cover!(
        !positive_rate && supplied_rate == -(group.config.max_abs_funding_e9_per_slot as i128),
        "v16 permissionless crank accepts negative funding boundary"
    );
    assert_eq!(out, PermissionlessProgressOutcomeV16::AccountCurrent);
    assert_eq!(group.current_slot, 1);
    assert_eq!(group.slot_last, 1);
    assert_eq!(group.funding_epoch, 0);
}

#[kani::proof]
#[kani::unwind(100)]
#[kani::solver(cadical)]
fn proof_v16_per_asset_slot_last_prevents_cross_asset_accrual_aliasing() {
    let (market, _, _) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(2, 0, 1)).unwrap();
    let owner = [3; 32];
    let mut a0_long = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [31; 32], owner));
    let mut a0_short =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [32; 32], owner));
    let mut a1_long = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [33; 32], owner));
    let mut a1_short =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [34; 32], owner));
    group
        .attach_leg(&mut a0_long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut a0_short, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    group
        .attach_leg(&mut a1_long, 1, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    group
        .attach_leg(&mut a1_short, 1, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    let mut i = 0;
    while i < 2 {
        group.assets[i].effective_price = 100;
        group.assets[i].fund_px_last = 100;
        group.assets[i].raw_oracle_target_price = 100;
        i += 1;
    }

    let asset1_initial = group.assets[1];
    let first = group.accrue_asset_to_not_atomic(0, 1, 101, 0, true);
    let asset0_after_first = group.assets[0];
    let asset1_slot_before = group.assets[1].slot_last;
    assert_eq!(
        group.assets[1], asset1_initial,
        "asset 0 accrual must not mutate asset 1 state"
    );
    let second = group.accrue_asset_to_not_atomic(1, 1, 101, 0, true);

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
#[kani::unwind(70)]
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
#[kani::unwind(70)]
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
#[kani::unwind(70)]
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
    if funding_rate_e9 > 0 {
        group.assets[0].f_long_num = -(ADL_ONE as i128);
        group.assets[0].f_short_num = ADL_ONE as i128;
    } else {
        group.assets[0].f_long_num = ADL_ONE as i128;
        group.assets[0].f_short_num = -(ADL_ONE as i128);
    }
    group.funding_epoch = 1;
    let refreshed = if refresh_long {
        group
            .full_account_refresh(&mut long, &[1_000_000_000; V16_MAX_PORTFOLIO_ASSETS_N])
            .unwrap();
        long
    } else {
        group
            .full_account_refresh(&mut short, &[1_000_000_000; V16_MAX_PORTFOLIO_ASSETS_N])
            .unwrap();
        short
    };

    assert_eq!(refreshed.pnl, expected_pnl);
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(40)]
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
#[kani::unwind(60)]
#[kani::solver(cadical)]
fn proof_v16_partial_liquidation_can_reduce_risk_without_forcing_full_close() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 10).unwrap();
    group
        .attach_leg(&mut account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite_for_live_oi(&mut group, 0, SideV16::Long, POS_SCALE, 93);

    let out = group
        .liquidate_account_not_atomic(
            &mut account,
            LiquidationRequestV16 {
                asset_index: 0,
                close_q: POS_SCALE / 2,
                fee_bps: 0,
            },
            &[100; V16_MAX_PORTFOLIO_ASSETS_N],
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
    group.assets[0].k_long = -(100 * ADL_ONE as i128);
    let before_b_short = group.assets[0].b_short_num;
    let before_basis = bankrupt.legs[0].basis_pos_q;
    let before_bitmap = bankrupt.active_bitmap;
    let before_b_loss_booked = bankrupt.close_progress.b_loss_booked;

    let result = group.liquidate_account_not_atomic(
        &mut bankrupt,
        LiquidationRequestV16 {
            asset_index: 0,
            close_q: POS_SCALE / 2,
            fee_bps: 0,
        },
        &[1; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    kani::cover!(
        result == Err(V16Error::RecoveryRequired),
        "v16 partial liquidation residual routes to recovery before B booking"
    );
    assert_eq!(result, Err(V16Error::RecoveryRequired));
    assert_eq!(
        group.recovery_reason,
        Some(PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress)
    );
    assert_eq!(group.assets[0].b_short_num, before_b_short);
    assert_eq!(bankrupt.close_progress.b_loss_booked, before_b_loss_booked);
    assert_eq!(bankrupt.legs[0].basis_pos_q, before_basis);
    assert_eq!(bankrupt.active_bitmap, before_bitmap);
}

#[kani::proof]
#[kani::unwind(45)]
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
#[kani::unwind(40)]
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
        .deposit_not_atomic(&mut account, capital as u128)
        .unwrap();

    let charged = group
        .charge_account_fee_not_atomic(&mut account, 40)
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
    assert_eq!(group.assert_public_invariants(), Ok(()));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v16_resolved_active_position_close_returns_progress_without_payout() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.deposit_not_atomic(&mut account, 7).unwrap();
    group.attach_leg(&mut account, 0, SideV16::Long, 1).unwrap();
    group.resolve_market_not_atomic(1).unwrap();
    let before_vault = group.vault;
    let before_c_tot = group.c_tot;

    let outcome = group.close_resolved_account_not_atomic(&mut account, 0);

    assert_eq!(outcome, Ok(ResolvedCloseOutcomeV16::ProgressOnly));
    assert!(!percolator::active_bitmap_is_empty(account.active_bitmap));
    assert_eq!(account.capital, 7);
    assert_eq!(group.vault, before_vault);
    assert_eq!(group.c_tot, before_c_tot);
}
