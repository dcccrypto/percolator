#![cfg(kani)]

use percolator::v16::{
    active_bitmap_set, kani_add_open_interest_for_new_position,
    kani_apply_backing_provider_earnings_withdraw, kani_apply_backing_utilization_fee_charge,
    kani_apply_resolved_payout_receipt_payment, kani_expected_source_credit_rate_num_for_state,
    kani_liquidation_close_would_leave_uncovered_loss_with_open_risk,
    kani_validate_positive_pnl_source_attribution, AssetLifecycleV16, AssetStateV16,
    AssetStateV16Account, BackingBucketStatusV16, BackingBucketV16, BackingBucketV16Account,
    CloseProgressLedgerV16, EngineAssetSlotV16Account, HealthCertV16, HealthCertV16Account,
    InsuranceCreditReservationV16, InsuranceCreditReservationV16Account, Market,
    MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, PermissionlessCrankActionV16,
    PermissionlessCrankRequestV16, PermissionlessProgressOutcomeV16,
    PermissionlessRecoveryReasonV16, PortfolioAccountV16Account, PortfolioLegV16,
    PortfolioLegV16Account, PortfolioSourceDomainV16Account, PortfolioV16ViewMut,
    ProvenanceHeaderV16, ProvenanceHeaderV16Account, ResolvedPayoutLedgerV16,
    ResolvedPayoutLedgerV16Account, ResolvedPayoutReceiptV16, ResolvedPayoutReceiptV16Account,
    SideV16, SourceCreditStateV16, SourceCreditStateV16Account, TokenValueClassV16,
    TokenValueFlowProofV16, V16Config, V16Error, V16PodI128, V16PodU128, V16PodU64,
    V16_EMPTY_ACTIVE_BITMAP,
};
use percolator::{ADL_ONE, BOUND_SCALE, CREDIT_RATE_SCALE, MAX_ACCOUNT_NOTIONAL, POS_SCALE};

fn ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    ([1; 32], [2; 32], [3; 32])
}

fn empty_account_fixture(
    market_id: [u8; 32],
    account_tag: u8,
) -> (
    PortfolioAccountV16Account,
    [PortfolioSourceDomainV16Account; 2],
) {
    let mut account_id = [0u8; 32];
    account_id[0] = account_tag;
    let mut owner = [0u8; 32];
    owner[0] = account_tag;
    let account_header =
        PortfolioAccountV16Account::try_empty(ProvenanceHeaderV16Account::from_runtime(
            &ProvenanceHeaderV16::new(market_id, account_id, owner),
        ))
        .unwrap();
    let source_domains = [PortfolioSourceDomainV16Account::default(); 2];
    (account_header, source_domains)
}

fn one_market_view_fixture() -> (
    MarketGroupV16HeaderAccount,
    [Market<u64>; 1],
    PortfolioAccountV16Account,
    [PortfolioSourceDomainV16Account; 2],
) {
    let (market_id, _, _) = ids();
    let cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(market_id, cfg, 1, 0).unwrap();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    {
        let mut view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        view.activate_empty_market_not_atomic(0, 100, 1).unwrap();
    }
    let (account_header, source_domains) = empty_account_fixture(market_id, 2);
    (header, markets, account_header, source_domains)
}

#[kani::proof]
#[kani::unwind(48)]
#[kani::solver(cadical)]
fn proof_v16_view_deposit_preserves_c_tot_vault_capital_sum() {
    let amount_raw: u16 = kani::any();
    kani::assume(amount_raw <= 1_000);
    let amount = amount_raw as u128;
    let (mut header, mut markets, mut account_header, mut source_domains) =
        one_market_view_fixture();
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);

    market.deposit_not_atomic(&mut account, amount).unwrap();

    kani::cover!(amount > 0, "view deposit covers nonzero amount");
    assert_eq!(account.header.capital.get(), amount);
    assert_eq!(market.header.c_tot.get(), amount);
    assert_eq!(market.header.vault.get(), amount);
    assert_eq!(market.validate_shape(), Ok(()));
}

#[kani::proof]
#[kani::unwind(48)]
#[kani::solver(cadical)]
fn proof_v16_view_overwithdraw_rejects() {
    let (mut header, mut markets, mut account_header, mut source_domains) =
        one_market_view_fixture();
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    market.deposit_not_atomic(&mut account, 3).unwrap();

    let result = market.withdraw_not_atomic(&mut account, 4);

    kani::cover!(
        result == Err(V16Error::LockActive),
        "view overwithdraw lock branch reachable"
    );
    assert_eq!(result, Err(V16Error::LockActive));
}

#[kani::proof]
#[kani::unwind(48)]
#[kani::solver(cadical)]
fn proof_v16_view_withdraw_reduces_vault_ctot_and_capital_equally() {
    let amount_raw: u8 = kani::any();
    kani::assume((1..=5).contains(&amount_raw));
    let amount = amount_raw as u128;
    let (mut header, mut markets, mut account_header, mut source_domains) =
        one_market_view_fixture();
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    market.deposit_not_atomic(&mut account, 10).unwrap();
    let vault_before = market.header.vault.get();
    let c_tot_before = market.header.c_tot.get();
    let insurance_before = market.header.insurance.get();
    let capital_before = account.header.capital.get();

    market.withdraw_not_atomic(&mut account, amount).unwrap();

    kani::cover!(amount > 1, "successful withdraw covers nontrivial amount");
    assert_eq!(market.header.vault.get(), vault_before - amount);
    assert_eq!(market.header.c_tot.get(), c_tot_before - amount);
    assert_eq!(account.header.capital.get(), capital_before - amount);
    assert_eq!(market.header.insurance.get(), insurance_before);
    assert_eq!(market.validate_shape(), Ok(()));
    assert_eq!(account.validate_with_market(&market.as_view()), Ok(()));
}

#[kani::proof]
#[kani::unwind(48)]
#[kani::solver(cadical)]
fn proof_v16_recovery_mode_blocks_withdraw() {
    let (mut header, mut markets, mut account_header, mut source_domains) =
        one_market_view_fixture();
    header.mode = 2;
    header.vault = V16PodU128::new(10);
    header.c_tot = V16PodU128::new(10);
    account_header.capital = V16PodU128::new(10);

    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    let result = market.withdraw_not_atomic(&mut account, 1);

    kani::cover!(
        result == Err(V16Error::LockActive),
        "recovery mode blocks ordinary withdraw"
    );
    assert_eq!(result, Err(V16Error::LockActive));
}

#[kani::proof]
#[kani::unwind(32)]
#[kani::solver(cadical)]
fn proof_v16_public_resolve_market_is_value_neutral_and_clears_loss_stale() {
    let resolved_slot_raw: u8 = kani::any();
    kani::assume((1..=10).contains(&resolved_slot_raw));
    let resolved_slot = resolved_slot_raw as u64;
    let (mut header, mut markets, _, _) = one_market_view_fixture();
    header.vault = V16PodU128::new(7);
    header.c_tot = V16PodU128::new(3);
    header.insurance = V16PodU128::new(4);
    header.loss_stale_active = 1;
    header.current_slot = V16PodU64::new(1);
    header.slot_last = V16PodU64::new(1);
    let vault_before = header.vault;
    let c_tot_before = header.c_tot;
    let insurance_before = header.insurance;
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    market.resolve_market_not_atomic(resolved_slot).unwrap();

    kani::cover!(
        resolved_slot > 1,
        "resolved market transition covers future authenticated slot"
    );
    assert_eq!(market.header.mode, 1);
    assert_eq!(market.header.resolved_slot.get(), resolved_slot);
    assert_eq!(market.header.current_slot.get(), resolved_slot);
    assert_eq!(market.header.loss_stale_active, 0);
    assert_eq!(market.header.vault, vault_before);
    assert_eq!(market.header.c_tot, c_tot_before);
    assert_eq!(market.header.insurance, insurance_before);
    assert_eq!(market.validate_shape(), Ok(()));
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v16_open_source_claim_exposure_blocks_convert() {
    let (mut header, mut markets, mut account_header, mut source_domains) =
        one_market_view_fixture();
    let market_id = markets[0].engine.asset.market_id.get();
    let face_num = 10u128 * BOUND_SCALE;
    let mut bitmap = account_header.active_bitmap.map(V16PodU64::get);
    active_bitmap_set(&mut bitmap, 0).unwrap();
    let leg = PortfolioLegV16 {
        active: true,
        asset_index: 0,
        market_id,
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
    account_header.legs[0] = PortfolioLegV16Account::from_runtime(&leg);
    account_header.active_bitmap = bitmap.map(V16PodU64::new);
    account_header.pnl = V16PodI128::new(10);
    account_header.health_cert = HealthCertV16Account::from_runtime(&HealthCertV16 {
        certified_equity: 100,
        certified_initial_req: 1,
        certified_maintenance_req: 1,
        certified_liq_deficit: 0,
        certified_worst_case_loss: 1,
        cert_oracle_epoch: header.oracle_epoch.get(),
        cert_funding_epoch: header.funding_epoch.get(),
        cert_risk_epoch: header.risk_epoch.get(),
        cert_asset_set_epoch: header.asset_set_epoch.get(),
        active_bitmap_at_cert: bitmap,
        valid: true,
    });
    markets[0].engine.source_credit_short =
        SourceCreditStateV16Account::from_runtime(&SourceCreditStateV16 {
            positive_claim_bound_num: face_num,
            exact_positive_claim_num: face_num,
            credit_rate_num: 0,
            ..SourceCreditStateV16::EMPTY
        });
    source_domains[1].source_claim_market_id = V16PodU64::new(market_id);
    source_domains[1].source_claim_bound_num = V16PodU128::new(face_num);
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);

    let result = market.convert_released_pnl_to_capital_not_atomic(&mut account);

    kani::cover!(
        result == Err(V16Error::LockActive),
        "active source-claim exposure reaches convert guard"
    );
    assert_eq!(result, Err(V16Error::LockActive));
}

#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn proof_v16_view_trade_position_delta_preserves_oi_symmetry() {
    let size_units_raw: u8 = kani::any();
    let loss_weight_raw: u8 = kani::any();
    kani::assume((1..=4).contains(&size_units_raw));
    kani::assume((1..=4).contains(&loss_weight_raw));
    let size_q = size_units_raw as u128 * POS_SCALE;
    let loss_weight = loss_weight_raw as u128 * POS_SCALE;
    let mut asset = AssetStateV16::default();
    let before = asset;

    kani_add_open_interest_for_new_position(&mut asset, SideV16::Long, size_q, loss_weight)
        .unwrap();
    kani_add_open_interest_for_new_position(&mut asset, SideV16::Short, size_q, loss_weight)
        .unwrap();

    kani::cover!(
        size_units_raw > 1 && loss_weight_raw > 1,
        "trade open-interest accounting covers nontrivial size and weight"
    );
    assert_eq!(asset.oi_eff_long_q, size_q);
    assert_eq!(asset.oi_eff_short_q, size_q);
    assert_eq!(asset.loss_weight_sum_long, loss_weight);
    assert_eq!(asset.loss_weight_sum_short, loss_weight);
    assert_eq!(asset.stored_pos_count_long, 1);
    assert_eq!(asset.stored_pos_count_short, 1);
    assert_eq!(asset.market_id, before.market_id);
    assert_eq!(asset.effective_price, before.effective_price);
    assert_eq!(asset.k_long, before.k_long);
    assert_eq!(asset.k_short, before.k_short);
    assert_eq!(asset.f_long_num, before.f_long_num);
    assert_eq!(asset.f_short_num, before.f_short_num);
    assert_eq!(asset.b_long_num, before.b_long_num);
    assert_eq!(asset.b_short_num, before.b_short_num);
}

#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn proof_v16_liquidation_cannot_leave_uncovered_loss_with_other_open_risk() {
    let mut two_leg_bitmap = V16_EMPTY_ACTIVE_BITMAP;
    active_bitmap_set(&mut two_leg_bitmap, 0).unwrap();
    active_bitmap_set(&mut two_leg_bitmap, 1).unwrap();
    let mut single_leg_bitmap = V16_EMPTY_ACTIVE_BITMAP;
    active_bitmap_set(&mut single_leg_bitmap, 0).unwrap();

    let full_close_with_other_risk =
        kani_liquidation_close_would_leave_uncovered_loss_with_open_risk(
            -5,
            1,
            two_leg_bitmap,
            0,
            10,
            10,
        )
        .unwrap();
    let partial_close_with_other_risk =
        kani_liquidation_close_would_leave_uncovered_loss_with_open_risk(
            -5,
            1,
            two_leg_bitmap,
            0,
            5,
            10,
        )
        .unwrap();
    let full_close_without_other_risk =
        kani_liquidation_close_would_leave_uncovered_loss_with_open_risk(
            -5,
            1,
            single_leg_bitmap,
            0,
            10,
            10,
        )
        .unwrap();
    let covered_loss_with_other_risk =
        kani_liquidation_close_would_leave_uncovered_loss_with_open_risk(
            -5,
            5,
            two_leg_bitmap,
            0,
            10,
            10,
        )
        .unwrap();

    kani::cover!(
        full_close_with_other_risk && partial_close_with_other_risk,
        "liquidation guard detects uncovered loss with remaining open risk"
    );
    assert!(full_close_with_other_risk);
    assert!(partial_close_with_other_risk);
    assert!(!full_close_without_other_risk);
    assert!(!covered_loss_with_other_risk);
}

#[kani::proof]
#[kani::unwind(32)]
#[kani::solver(cadical)]
fn proof_v16_trade_fee_helper_moves_capital_to_insurance_only() {
    let capital_raw: u8 = kani::any();
    let fee_raw: u8 = kani::any();
    kani::assume(capital_raw <= 10);
    kani::assume(fee_raw <= 10);
    let capital = capital_raw as u128;
    let requested_fee = fee_raw as u128;
    let expected = capital.min(requested_fee);
    let (mut header, mut markets, mut account_header, mut source_domains) =
        one_market_view_fixture();
    header.vault = V16PodU128::new(100 + capital);
    header.c_tot = V16PodU128::new(capital);
    header.insurance = V16PodU128::new(100);
    account_header.capital = V16PodU128::new(capital);
    account_header.pnl = V16PodI128::new(0);
    let vault_before = header.vault.get();
    let senior_before = header.c_tot.get() + header.insurance.get();

    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    let charged = market
        .kani_charge_account_fee_current_not_atomic(&mut account, requested_fee)
        .unwrap();

    kani::cover!(
        capital > 0 && requested_fee > capital,
        "trade fee helper covers capped fee collection"
    );
    kani::cover!(
        capital > 0 && requested_fee <= capital && requested_fee > 0,
        "trade fee helper covers full requested fee collection"
    );
    assert_eq!(charged, expected);
    assert_eq!(market.header.vault.get(), vault_before);
    assert_eq!(
        market.header.c_tot.get() + market.header.insurance.get(),
        senior_before
    );
    assert_eq!(account.header.capital.get(), capital - expected);
    assert_eq!(market.header.c_tot.get(), capital - expected);
    assert_eq!(market.header.insurance.get(), 100 + expected);
}

#[kani::proof]
#[kani::unwind(32)]
#[kani::solver(cadical)]
fn proof_v16_trade_fee_helper_does_not_charge_negative_pnl_account() {
    let requested_fee_raw: u8 = kani::any();
    kani::assume(requested_fee_raw <= 10);
    let requested_fee = requested_fee_raw as u128;
    let (mut header, mut markets, mut account_header, mut source_domains) =
        one_market_view_fixture();
    header.vault = V16PodU128::new(110);
    header.c_tot = V16PodU128::new(10);
    header.insurance = V16PodU128::new(100);
    account_header.capital = V16PodU128::new(10);
    account_header.pnl = V16PodI128::new(-1);
    let vault_before = header.vault;
    let c_tot_before = header.c_tot;
    let insurance_before = header.insurance;
    let capital_before = account_header.capital;

    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    let charged = market
        .kani_charge_account_fee_current_not_atomic(&mut account, requested_fee)
        .unwrap();

    kani::cover!(
        requested_fee > 0,
        "negative-PnL account reaches no-fee guard with requested fee"
    );
    assert_eq!(charged, 0);
    assert_eq!(market.header.vault, vault_before);
    assert_eq!(market.header.c_tot, c_tot_before);
    assert_eq!(market.header.insurance, insurance_before);
    assert_eq!(account.header.capital, capital_before);
    assert_eq!(account.header.pnl.get(), -1);
}

#[kani::proof]
#[kani::unwind(48)]
#[kani::solver(cadical)]
fn proof_v16_negative_pnl_settlement_consumes_principal_before_residual() {
    let capital_raw: u8 = kani::any();
    let loss_raw: u8 = kani::any();
    kani::assume(capital_raw <= 10);
    kani::assume((1..=10).contains(&loss_raw));
    let capital = capital_raw as u128;
    let loss = loss_raw as u128;
    let paid_expected = capital.min(loss);
    let (mut header, mut markets, mut account_header, mut source_domains) =
        one_market_view_fixture();
    header.vault = V16PodU128::new(capital);
    header.c_tot = V16PodU128::new(capital);
    header.negative_pnl_account_count = V16PodU64::new(1);
    account_header.capital = V16PodU128::new(capital);
    account_header.pnl = V16PodI128::new(-(loss as i128));
    let vault_before = header.vault.get();

    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    let paid = market
        .settle_negative_pnl_from_principal_not_atomic(&mut account)
        .unwrap();

    kani::cover!(
        capital > 0 && capital < loss,
        "principal settlement covers residual bankruptcy branch"
    );
    kani::cover!(
        capital >= loss,
        "principal settlement covers fully paid realized loss"
    );
    assert_eq!(paid, paid_expected);
    assert_eq!(market.header.vault.get(), vault_before);
    assert_eq!(market.header.c_tot.get(), capital - paid_expected);
    assert_eq!(account.header.capital.get(), capital - paid_expected);
    assert_eq!(
        account.header.pnl.get(),
        -(loss as i128) + paid_expected as i128
    );
    if paid_expected < loss {
        assert_eq!(market.header.bankruptcy_hlock_active, 1);
        assert_eq!(market.header.negative_pnl_account_count.get(), 1);
    } else {
        assert_eq!(market.header.negative_pnl_account_count.get(), 0);
    }
}

#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn proof_v16_backing_utilization_fee_never_charges_negative_pnl_account() {
    let capital_raw: u8 = kani::any();
    let fee_raw: u8 = kani::any();
    let earnings_raw: u8 = kani::any();
    kani::assume(capital_raw <= 10);
    kani::assume(fee_raw <= 10);
    kani::assume(earnings_raw <= 10);
    let capital = capital_raw as u128;
    let fee = fee_raw as u128;
    let earnings = earnings_raw as u128;
    let group_c_tot = capital;

    let (charged, next_capital, next_c_tot, next_earnings) =
        kani_apply_backing_utilization_fee_charge(capital, group_c_tot, earnings, -1, fee).unwrap();

    kani::cover!(
        fee > 0 && capital > 0,
        "negative-PnL backing utilization fee reaches no-charge guard"
    );
    assert_eq!(charged, 0);
    assert_eq!(next_capital, capital);
    assert_eq!(next_c_tot, group_c_tot);
    assert_eq!(next_earnings, earnings);
}

#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn proof_v16_backing_utilization_fee_is_capped_by_capital_and_conserves_ctot_to_earnings() {
    let capital_raw: u8 = kani::any();
    let fee_raw: u8 = kani::any();
    let earnings_raw: u8 = kani::any();
    kani::assume(capital_raw <= 10);
    kani::assume(fee_raw <= 10);
    kani::assume(earnings_raw <= 10);
    let capital = capital_raw as u128;
    let fee = fee_raw as u128;
    let earnings = earnings_raw as u128;
    let group_c_tot = capital;
    let expected = capital.min(fee);

    let (charged, next_capital, next_c_tot, next_earnings) =
        kani_apply_backing_utilization_fee_charge(capital, group_c_tot, earnings, 0, fee).unwrap();

    kani::cover!(
        fee > capital && capital > 0,
        "backing utilization fee covers capital-capped collection"
    );
    kani::cover!(
        fee <= capital && fee > 0,
        "backing utilization fee covers full requested collection"
    );
    assert_eq!(charged, expected);
    assert_eq!(next_capital, capital - expected);
    assert_eq!(next_c_tot, group_c_tot - expected);
    assert_eq!(next_earnings, earnings + expected);
    assert_eq!(next_c_tot + next_earnings, group_c_tot + earnings);
}

#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn proof_v16_backing_provider_earnings_withdraw_cannot_exceed_earnings() {
    let vault_raw: u8 = kani::any();
    let earnings_raw: u8 = kani::any();
    let amount_raw: u8 = kani::any();
    kani::assume(vault_raw <= 20);
    kani::assume(earnings_raw <= vault_raw);
    kani::assume(amount_raw <= 20);
    let vault = vault_raw as u128;
    let earnings = earnings_raw as u128;
    let amount = amount_raw as u128;
    let result = kani_apply_backing_provider_earnings_withdraw(vault, earnings, amount);

    if amount <= earnings {
        let (next_vault, next_earnings) = result.unwrap();
        kani::cover!(
            amount > 0 && amount < earnings,
            "provider earnings withdraw covers partial earned payout"
        );
        assert_eq!(next_vault, vault - amount);
        assert_eq!(next_earnings, earnings - amount);
    } else {
        kani::cover!(
            amount > earnings,
            "provider earnings withdraw rejects over-withdraw"
        );
        assert_eq!(result, Err(V16Error::CounterUnderflow));
    }
}

#[kani::proof]
#[kani::unwind(48)]
#[kani::solver(cadical)]
fn proof_v16_public_backing_provider_earnings_withdraw_debits_only_earned_vault() {
    let (mut header, mut markets, _, _) = one_market_view_fixture();
    let market_id = markets[0].engine.asset.market_id.get();
    header.vault = V16PodU128::new(5);
    markets[0].engine.backing_long = BackingBucketV16Account::from_runtime(&BackingBucketV16 {
        market_id,
        utilization_fee_earnings: 5,
        status: BackingBucketStatusV16::Expired,
        ..BackingBucketV16::EMPTY
    });
    let c_tot_before = header.c_tot;
    let insurance_before = header.insurance;
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    market
        .withdraw_backing_provider_earnings_not_atomic(0, 3)
        .unwrap();
    let bucket = market.markets[0]
        .engine
        .backing_long
        .try_to_runtime()
        .unwrap();

    kani::cover!(
        bucket.utilization_fee_earnings == 2,
        "public backing earnings withdraw is nontrivial"
    );
    assert_eq!(market.header.vault.get(), 2);
    assert_eq!(bucket.utilization_fee_earnings, 2);
    assert_eq!(market.header.c_tot, c_tot_before);
    assert_eq!(market.header.insurance, insurance_before);
    assert_eq!(market.validate_shape(), Ok(()));
}

#[kani::proof]
#[kani::unwind(64)]
#[kani::solver(cadical)]
fn proof_v16_reused_asset_slot_rejects_stale_market_id_leg() {
    let (mut header, mut markets, mut account_header, mut source_domains) =
        one_market_view_fixture();
    let leg = PortfolioLegV16 {
        active: true,
        asset_index: 0,
        market_id: 999,
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
    account_header.legs[0] = percolator::v16::PortfolioLegV16Account::from_runtime(&leg);
    let mut bitmap = account_header.active_bitmap.map(V16PodU64::get);
    active_bitmap_set(&mut bitmap, 0).unwrap();
    account_header.active_bitmap = bitmap.map(V16PodU64::new);

    let market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let account = PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    let result = account.as_view().validate_with_market(&market.as_view());

    kani::cover!(
        result == Err(V16Error::HiddenLeg),
        "stale market_id leg is rejected after asset slot reuse"
    );
    assert_eq!(result, Err(V16Error::HiddenLeg));
}

#[kani::proof]
#[kani::unwind(64)]
#[kani::solver(cadical)]
fn proof_v16_duplicate_asset_legs_reject_before_double_counting_support() {
    let (mut header, mut markets, mut account_header, mut source_domains) =
        one_market_view_fixture();
    let long_leg = PortfolioLegV16 {
        active: true,
        asset_index: 0,
        market_id: 1,
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
    let short_leg = PortfolioLegV16 {
        side: SideV16::Short,
        basis_pos_q: -(POS_SCALE as i128),
        ..long_leg
    };
    account_header.legs[0] = percolator::v16::PortfolioLegV16Account::from_runtime(&long_leg);
    account_header.legs[1] = percolator::v16::PortfolioLegV16Account::from_runtime(&short_leg);
    let mut bitmap = account_header.active_bitmap.map(V16PodU64::get);
    active_bitmap_set(&mut bitmap, 0).unwrap();
    active_bitmap_set(&mut bitmap, 1).unwrap();
    account_header.active_bitmap = bitmap.map(V16PodU64::new);

    let market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let account = PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    let result = account.as_view().validate_with_market(&market.as_view());

    kani::cover!(
        result == Err(V16Error::HiddenLeg),
        "duplicate active asset legs are rejected"
    );
    assert_eq!(result, Err(V16Error::HiddenLeg));
}

#[kani::proof]
#[kani::unwind(48)]
#[kani::solver(cadical)]
fn proof_v16_mark_asset_drain_only_is_value_neutral_and_epoch_scoped() {
    let (mut header, mut markets, _, _) = one_market_view_fixture();
    header.vault = V16PodU128::new(10);
    header.c_tot = V16PodU128::new(7);
    header.insurance = V16PodU128::new(3);
    let vault_before = header.vault;
    let c_tot_before = header.c_tot;
    let insurance_before = header.insurance;
    let asset_set_epoch_before = header.asset_set_epoch.get();
    let risk_epoch_before = header.risk_epoch.get();

    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    market.mark_asset_drain_only_not_atomic(0).unwrap();
    let asset = market.markets[0].engine.asset.try_to_runtime().unwrap();

    kani::cover!(
        asset.lifecycle == AssetLifecycleV16::DrainOnly,
        "active asset can enter drain-only without value movement"
    );
    assert_eq!(asset.lifecycle, AssetLifecycleV16::DrainOnly);
    assert_eq!(market.header.vault, vault_before);
    assert_eq!(market.header.c_tot, c_tot_before);
    assert_eq!(market.header.insurance, insurance_before);
    assert_eq!(
        market.header.asset_set_epoch.get(),
        asset_set_epoch_before + 1
    );
    assert_eq!(market.header.risk_epoch.get(), risk_epoch_before + 1);
    assert_eq!(market.validate_shape(), Ok(()));
}

#[kani::proof]
#[kani::unwind(48)]
#[kani::solver(cadical)]
fn proof_v16_retire_nonempty_asset_rejects() {
    let (mut header, mut markets, _, _) = one_market_view_fixture();
    let mut asset = markets[0].engine.asset.try_to_runtime().unwrap();
    asset.oi_eff_long_q = POS_SCALE;
    asset.stored_pos_count_long = 1;
    asset.loss_weight_sum_long = POS_SCALE;
    markets[0].engine.asset = AssetStateV16Account::from_runtime(&asset);

    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let result = market.retire_empty_asset_not_atomic(0, 10);

    kani::cover!(
        result == Err(V16Error::LockActive),
        "nonempty asset retirement reaches fail-closed guard"
    );
    assert_eq!(result, Err(V16Error::LockActive));
}

#[kani::proof]
#[kani::unwind(48)]
#[kani::solver(cadical)]
fn proof_v16_retire_empty_asset_is_value_neutral_and_epoch_scoped() {
    let (mut header, mut markets, _, _) = one_market_view_fixture();
    header.vault = V16PodU128::new(10);
    header.c_tot = V16PodU128::new(7);
    header.insurance = V16PodU128::new(3);
    let vault_before = header.vault;
    let c_tot_before = header.c_tot;
    let insurance_before = header.insurance;
    let asset_set_epoch_before = header.asset_set_epoch.get();
    let risk_epoch_before = header.risk_epoch.get();

    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    market.retire_empty_asset_not_atomic(0, 10).unwrap();
    let asset = market.markets[0].engine.asset.try_to_runtime().unwrap();

    kani::cover!(
        asset.lifecycle == AssetLifecycleV16::Retired,
        "empty asset can retire without moving value"
    );
    assert_eq!(asset.lifecycle, AssetLifecycleV16::Retired);
    assert_eq!(asset.retired_slot, 10);
    assert_eq!(market.header.current_slot.get(), 10);
    assert_eq!(market.header.vault, vault_before);
    assert_eq!(market.header.c_tot, c_tot_before);
    assert_eq!(market.header.insurance, insurance_before);
    assert_eq!(
        market.header.asset_set_epoch.get(),
        asset_set_epoch_before + 1
    );
    assert_eq!(market.header.risk_epoch.get(), risk_epoch_before + 1);
    assert_eq!(market.validate_shape(), Ok(()));
}

#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn proof_v16_positive_pnl_requires_full_source_claim_attribution() {
    let pnl_raw: u8 = kani::any();
    let missing_raw: u8 = kani::any();
    kani::assume((1..=10).contains(&pnl_raw));
    kani::assume((1..=10).contains(&missing_raw));
    let pnl = pnl_raw as i128;
    let required = pnl_raw as u128 * BOUND_SCALE;
    let missing = (missing_raw as u128).min(required);
    let insufficient = required - missing;

    let ok = kani_validate_positive_pnl_source_attribution(pnl, required);
    let err = kani_validate_positive_pnl_source_attribution(pnl, insufficient);
    let non_positive = kani_validate_positive_pnl_source_attribution(-pnl, 0);

    kani::cover!(
        insufficient < required,
        "positive PnL source attribution rejects under-attributed claim bounds"
    );
    assert_eq!(ok, Ok(()));
    assert_eq!(err, Err(V16Error::InvalidLeg));
    assert_eq!(non_positive, Ok(()));
}

#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn proof_v16_source_credit_rate_never_exceeds_available_backing_ratio() {
    let claim_atoms_raw: u8 = kani::any();
    let backing_atoms_raw: u8 = kani::any();
    kani::assume((1..=10).contains(&claim_atoms_raw));
    kani::assume(backing_atoms_raw <= 20);
    let claim_num = claim_atoms_raw as u128 * BOUND_SCALE;
    let backing_num = backing_atoms_raw as u128 * BOUND_SCALE;
    let state = SourceCreditStateV16 {
        positive_claim_bound_num: claim_num,
        exact_positive_claim_num: claim_num,
        fresh_reserved_backing_num: backing_num,
        ..SourceCreditStateV16::EMPTY
    };

    let rate = kani_expected_source_credit_rate_num_for_state(state).unwrap();
    let usable_num = claim_num * rate / CREDIT_RATE_SCALE;

    kani::cover!(
        backing_num < claim_num,
        "source credit rate proof covers haircut branch"
    );
    kani::cover!(
        backing_num >= claim_num,
        "source credit rate proof covers full-credit branch"
    );
    assert!(rate <= CREDIT_RATE_SCALE);
    assert!(usable_num <= backing_num);
    if backing_num >= claim_num {
        assert_eq!(rate, CREDIT_RATE_SCALE);
    }
}

#[kani::proof]
#[kani::unwind(48)]
#[kani::solver(cadical)]
fn proof_v16_positive_kf_delta_creates_source_claim_bound() {
    let delta_raw: u8 = kani::any();
    kani::assume((1..=10).contains(&delta_raw));
    let delta = delta_raw as i128;
    let delta_num = delta_raw as u128 * BOUND_SCALE;
    let (mut header, mut markets, mut account_header, mut source_domains) =
        one_market_view_fixture();
    account_header.pnl = V16PodI128::new(0);

    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    let (support_consumed, junior_face_burned) = market
        .kani_apply_signed_kf_delta_to_pnl(&mut account, delta, Some(1))
        .unwrap();

    kani::cover!(
        delta > 1,
        "positive K/F settlement creates nontrivial source-attributed claim"
    );
    assert_eq!(support_consumed, 0);
    assert_eq!(junior_face_burned, 0);
    assert_eq!(account.header.pnl.get(), delta);
    assert_eq!(
        account.source_domains[1].source_claim_bound_num.get(),
        delta_num
    );
    assert_eq!(
        market.markets[0]
            .engine
            .source_credit_short
            .positive_claim_bound_num
            .get(),
        delta_num
    );
    assert_eq!(
        market.markets[0]
            .engine
            .source_credit_short
            .exact_positive_claim_num
            .get(),
        delta_num
    );
    assert_eq!(market.header.pnl_pos_tot.get(), delta as u128);
    assert_eq!(market.header.pnl_pos_bound_tot_num.get(), delta_num);
}

#[kani::proof]
#[kani::unwind(48)]
#[kani::solver(cadical)]
fn proof_v16_live_positive_kf_delta_without_source_rejects() {
    let delta_raw: u8 = kani::any();
    kani::assume((1..=10).contains(&delta_raw));
    let delta = delta_raw as i128;
    let (mut header, mut markets, mut account_header, mut source_domains) =
        one_market_view_fixture();
    account_header.pnl = V16PodI128::new(0);

    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    let result = market.kani_apply_signed_kf_delta_to_pnl(&mut account, delta, None);

    kani::cover!(
        delta > 1,
        "live positive K/F delta without source reaches fail-closed guard"
    );
    assert_eq!(result, Err(V16Error::InvalidLeg));
}

#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn proof_v16_resolved_receipt_payment_cannot_exceed_terminal_claim() {
    let terminal_raw: u8 = kani::any();
    let paid_raw: u8 = kani::any();
    kani::assume((1..=10).contains(&terminal_raw));
    kani::assume(paid_raw <= terminal_raw);
    let terminal = terminal_raw as u128;
    let paid = paid_raw as u128;
    let receipt = ResolvedPayoutReceiptV16 {
        present: true,
        prior_bound_contribution_num: terminal * BOUND_SCALE,
        live_released_face_at_receipt: terminal,
        terminal_positive_claim_face: terminal,
        paid_effective: paid,
        finalized: paid == terminal,
    };
    let remaining = terminal - paid;
    let ok_payment = kani_apply_resolved_payout_receipt_payment(receipt, remaining).unwrap();
    let overpay = kani_apply_resolved_payout_receipt_payment(receipt, remaining + 1);

    kani::cover!(
        paid < terminal && remaining > 0,
        "resolved receipt proof covers non-final receipt topup"
    );
    assert_eq!(ok_payment.paid_effective, terminal);
    assert!(ok_payment.finalized);
    assert_eq!(overpay, Err(V16Error::InvalidLeg));
}

#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_v16_public_resolved_payout_topup_pays_min_claimable_and_vault() {
    let claimable_raw: u8 = kani::any();
    let vault_raw: u8 = kani::any();
    kani::assume((1..=5).contains(&claimable_raw));
    kani::assume(vault_raw <= 5);
    let claimable = claimable_raw as u128;
    let vault = vault_raw as u128;
    let paid_before = 2u128;
    let terminal = paid_before + claimable;
    let payout = claimable.min(vault);
    let (mut header, mut markets, mut account_header, mut source_domains) =
        one_market_view_fixture();
    header.mode = 1;
    header.vault = V16PodU128::new(vault);
    header.payout_snapshot_captured = 1;
    header.resolved_payout_ledger =
        ResolvedPayoutLedgerV16Account::from_runtime(&ResolvedPayoutLedgerV16 {
            snapshot_residual: terminal,
            terminal_claim_exact_receipts_num: terminal * BOUND_SCALE,
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
            prior_bound_contribution_num: terminal * BOUND_SCALE,
            live_released_face_at_receipt: 0,
            terminal_positive_claim_face: terminal,
            paid_effective: paid_before,
            finalized: false,
        });
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);

    let paid = market
        .claim_resolved_payout_topup_not_atomic(&mut account)
        .unwrap();
    let receipt = account
        .header
        .resolved_payout_receipt
        .try_to_runtime()
        .unwrap();

    kani::cover!(payout > 0, "resolved payout topup pays a nonzero amount");
    kani::cover!(
        payout < claimable,
        "resolved payout topup is capped by vault"
    );
    kani::cover!(
        payout == claimable,
        "resolved payout topup can fully pay claimable amount"
    );
    assert_eq!(paid, payout);
    assert_eq!(market.header.vault.get(), vault - payout);
    assert_eq!(receipt.paid_effective, paid_before + payout);
    assert_eq!(receipt.finalized, payout == claimable);
    assert_eq!(market.validate_shape(), Ok(()));
    assert_eq!(account.validate_with_market(&market.as_view()), Ok(()));
}

#[kani::proof]
#[kani::unwind(48)]
#[kani::solver(cadical)]
fn proof_v16_expired_close_progress_declares_recovery_without_value_mutation() {
    let (mut header, mut markets, _, _) = one_market_view_fixture();
    header.current_slot = V16PodU64::new(11);
    header.vault = V16PodU128::new(10);
    header.c_tot = V16PodU128::new(7);
    header.insurance = V16PodU128::new(3);
    let vault_before = header.vault;
    let c_tot_before = header.c_tot;
    let insurance_before = header.insurance;
    let ledger = CloseProgressLedgerV16 {
        active: true,
        finalized: false,
        canceled: false,
        close_id: 1,
        asset_index: 0,
        market_id: 1,
        domain_side: SideV16::Long,
        gross_loss_at_close_start: 5,
        drift_reference_slot: 0,
        max_close_slot: 10,
        residual_remaining: 5,
        ..CloseProgressLedgerV16::EMPTY
    };

    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let result = market.kani_ensure_close_progress_not_expired(ledger);

    kani::cover!(
        result == Err(V16Error::RecoveryRequired),
        "expired live close progress declares recovery"
    );
    assert_eq!(result, Err(V16Error::RecoveryRequired));
    assert_eq!(market.header.mode, 2);
    assert_eq!(
        market.header.recovery_reason.try_to_runtime().unwrap(),
        Some(PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress)
    );
    assert_eq!(market.header.vault, vault_before);
    assert_eq!(market.header.c_tot, c_tot_before);
    assert_eq!(market.header.insurance, insurance_before);
}

#[kani::proof]
#[kani::unwind(48)]
#[kani::solver(cadical)]
fn proof_v16_permissionless_recovery_crank_is_accounting_neutral() {
    let (mut header, mut markets, mut account_header, mut source_domains) =
        one_market_view_fixture();
    header.vault = V16PodU128::new(10);
    header.c_tot = V16PodU128::new(7);
    header.insurance = V16PodU128::new(3);
    account_header.capital = V16PodU128::new(7);
    let vault_before = header.vault;
    let c_tot_before = header.c_tot;
    let insurance_before = header.insurance;
    let capital_before = account_header.capital;
    let pnl_before = account_header.pnl;

    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    let outcome = market
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

    kani::cover!(
        matches!(
            outcome,
            PermissionlessProgressOutcomeV16::RecoveryDeclared(
                PermissionlessRecoveryReasonV16::ExplicitLossOrDustAuditOverflow
            )
        ),
        "permissionless recovery crank reaches recovery declaration"
    );
    assert_eq!(
        outcome,
        PermissionlessProgressOutcomeV16::RecoveryDeclared(
            PermissionlessRecoveryReasonV16::ExplicitLossOrDustAuditOverflow
        )
    );
    assert_eq!(market.header.mode, 2);
    assert_eq!(
        market.header.recovery_reason.try_to_runtime().unwrap(),
        Some(PermissionlessRecoveryReasonV16::ExplicitLossOrDustAuditOverflow)
    );
    assert_eq!(market.header.vault, vault_before);
    assert_eq!(market.header.c_tot, c_tot_before);
    assert_eq!(market.header.insurance, insurance_before);
    assert_eq!(account.header.capital, capital_before);
    assert_eq!(account.header.pnl, pnl_before);
}

#[kani::proof]
#[kani::unwind(48)]
#[kani::solver(cadical)]
fn proof_v16_view_fee_sync_settles_negative_pnl_before_fee() {
    let (mut header, mut markets, mut account_header, mut source_domains) =
        one_market_view_fixture();
    header.vault = V16PodU128::new(100);
    header.c_tot = V16PodU128::new(100);
    header.negative_pnl_account_count = V16PodU64::new(1);
    header.current_slot = V16PodU64::new(10);
    header.slot_last = V16PodU64::new(10);
    account_header.capital = V16PodU128::new(100);
    account_header.pnl = V16PodI128::new(-40);
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);

    let charged = market
        .sync_account_fee_to_slot_not_atomic(&mut account, 10, 10)
        .unwrap();

    kani::cover!(
        charged == 60 && account.header.pnl.get() == 0,
        "view fee sync settles realized loss before fee"
    );
    assert_eq!(charged, 60);
    assert_eq!(account.header.pnl.get(), 0);
    assert_eq!(account.header.capital.get(), 0);
    assert_eq!(market.header.c_tot.get(), 0);
    assert_eq!(market.header.insurance.get(), 60);
    assert_eq!(market.header.vault.get(), 100);
}

#[kani::proof]
#[kani::unwind(48)]
#[kani::solver(cadical)]
fn proof_v16_view_domain_budget_caps_bankruptcy_insurance_spend() {
    let budget_raw: u8 = kani::any();
    kani::assume(budget_raw <= 5);
    let budget = budget_raw as u128;
    let (mut header, mut markets, mut account_header, mut source_domains) =
        one_market_view_fixture();
    header.vault = V16PodU128::new(10);
    header.insurance = V16PodU128::new(10);
    header.negative_pnl_account_count = V16PodU64::new(1);
    markets[0].engine.insurance_domain_budget_short = V16PodU128::new(budget);
    account_header.pnl = V16PodI128::new(-5);
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);

    let used = market
        .kani_consume_domain_insurance_for_negative_pnl(0, SideV16::Long, &mut account)
        .unwrap();

    kani::cover!(budget == 0 && used == 0, "zero domain budget spend branch");
    kani::cover!(
        budget > 0 && used == budget,
        "positive domain budget spend branch"
    );
    assert_eq!(used, budget);
    assert_eq!(market.header.insurance.get(), 10 - budget);
    assert_eq!(
        market.markets[0].engine.insurance_domain_spent_short.get(),
        budget
    );
    assert_eq!(account.header.pnl.get(), -5 + budget as i128);
}

fn run_funding_target_sign_case(positive_funding: bool) -> (i128, i128, i128) {
    let (mut header, mut markets, _, _) = one_market_view_fixture();
    if positive_funding {
        markets[0].engine.asset.f_long_num = V16PodI128::new(-(ADL_ONE as i128));
        markets[0].engine.asset.f_short_num = V16PodI128::new(ADL_ONE as i128);
    } else {
        markets[0].engine.asset.f_long_num = V16PodI128::new(ADL_ONE as i128);
        markets[0].engine.asset.f_short_num = V16PodI128::new(-(ADL_ONE as i128));
    }
    let leg = PortfolioLegV16 {
        active: true,
        asset_index: 0,
        market_id: 1,
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
    let market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    market.kani_leg_kf_delta_for_settlement(leg).unwrap()
}

#[kani::proof]
#[kani::unwind(48)]
#[kani::solver(cadical)]
fn proof_v16_view_positive_funding_charges_long_side() {
    let (k_now, f_now, net) = run_funding_target_sign_case(true);
    kani::cover!(
        k_now == 0 && f_now == -(ADL_ONE as i128) && net == -1,
        "positive funding charges long"
    );
    assert_eq!(k_now, 0);
    assert_eq!(f_now, -(ADL_ONE as i128));
    assert_eq!(net, -1);
}

#[kani::proof]
#[kani::unwind(48)]
#[kani::solver(cadical)]
fn proof_v16_view_negative_funding_pays_long_side() {
    let (k_now, f_now, net) = run_funding_target_sign_case(false);
    kani::cover!(
        k_now == 0 && f_now == ADL_ONE as i128 && net == 1,
        "negative funding pays long"
    );
    assert_eq!(k_now, 0);
    assert_eq!(f_now, ADL_ONE as i128);
    assert_eq!(net, 1);
}

#[kani::proof]
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

#[kani::proof]
#[kani::unwind(48)]
#[kani::solver(cadical)]
fn proof_v16_public_counterparty_lien_release_restores_unliened_backing_without_value_movement() {
    let amount_raw: u8 = kani::any();
    kani::assume((1..=5).contains(&amount_raw));
    let amount = amount_raw as u128 * BOUND_SCALE;
    let (mut header, mut markets, _, _) = one_market_view_fixture();
    let market_id = markets[0].engine.asset.market_id.get();
    markets[0].engine.backing_long = BackingBucketV16Account::from_runtime(&BackingBucketV16 {
        market_id,
        fresh_unliened_backing_num: amount,
        valid_liened_backing_num: amount,
        expiry_slot: 10,
        status: BackingBucketStatusV16::Fresh,
        ..BackingBucketV16::EMPTY
    });
    markets[0].engine.source_credit_long =
        SourceCreditStateV16Account::from_runtime(&SourceCreditStateV16 {
            fresh_reserved_backing_num: amount * 2,
            valid_liened_backing_num: amount,
            credit_rate_num: CREDIT_RATE_SCALE,
            ..SourceCreditStateV16::EMPTY
        });
    let vault_before = header.vault;
    let c_tot_before = header.c_tot;
    let insurance_before = header.insurance;
    let risk_epoch_before = header.risk_epoch.get();
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    market
        .release_source_credit_lien_from_counterparty_not_atomic(0, amount)
        .unwrap();
    let after_release_bucket = market.markets[0]
        .engine
        .backing_long
        .try_to_runtime()
        .unwrap();
    let after_release_source = market.markets[0]
        .engine
        .source_credit_long
        .try_to_runtime()
        .unwrap();

    kani::cover!(
        amount_raw > 1,
        "public counterparty lien release is nontrivial"
    );
    assert_eq!(after_release_bucket.status, BackingBucketStatusV16::Fresh);
    assert_eq!(after_release_bucket.fresh_unliened_backing_num, amount * 2);
    assert_eq!(after_release_bucket.valid_liened_backing_num, 0);
    assert_eq!(after_release_source.fresh_reserved_backing_num, amount * 2);
    assert_eq!(after_release_source.valid_liened_backing_num, 0);
    assert_eq!(market.header.vault, vault_before);
    assert_eq!(market.header.c_tot, c_tot_before);
    assert_eq!(market.header.insurance, insurance_before);
    assert!(market.header.risk_epoch.get() > risk_epoch_before);
    assert_eq!(market.validate_shape(), Ok(()));
}

#[kani::proof]
#[kani::unwind(48)]
#[kani::solver(cadical)]
fn proof_v16_public_counterparty_lien_consume_creates_receivable_without_value_movement() {
    let amount_raw: u8 = kani::any();
    kani::assume((1..=5).contains(&amount_raw));
    let amount = amount_raw as u128 * BOUND_SCALE;
    let (mut header, mut markets, _, _) = one_market_view_fixture();
    let market_id = markets[0].engine.asset.market_id.get();
    markets[0].engine.backing_long = BackingBucketV16Account::from_runtime(&BackingBucketV16 {
        market_id,
        valid_liened_backing_num: amount,
        expiry_slot: 10,
        status: BackingBucketStatusV16::Fresh,
        ..BackingBucketV16::EMPTY
    });
    markets[0].engine.source_credit_long =
        SourceCreditStateV16Account::from_runtime(&SourceCreditStateV16 {
            fresh_reserved_backing_num: amount,
            valid_liened_backing_num: amount,
            credit_rate_num: CREDIT_RATE_SCALE,
            ..SourceCreditStateV16::EMPTY
        });
    let vault_before = header.vault;
    let c_tot_before = header.c_tot;
    let insurance_before = header.insurance;
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    market
        .consume_source_credit_lien_from_counterparty_not_atomic(0, amount)
        .unwrap();
    let bucket = market.markets[0]
        .engine
        .backing_long
        .try_to_runtime()
        .unwrap();
    let source = market.markets[0]
        .engine
        .source_credit_long
        .try_to_runtime()
        .unwrap();

    kani::cover!(
        amount_raw > 1,
        "public counterparty lien consume is nontrivial"
    );
    assert_eq!(bucket.status, BackingBucketStatusV16::Expired);
    assert_eq!(bucket.fresh_unliened_backing_num, 0);
    assert_eq!(bucket.valid_liened_backing_num, 0);
    assert_eq!(bucket.consumed_liened_backing_num, amount);
    assert_eq!(source.fresh_reserved_backing_num, 0);
    assert_eq!(source.valid_liened_backing_num, 0);
    assert_eq!(source.spent_backing_num, amount);
    assert_eq!(source.provider_receivable_num, amount);
    assert_eq!(market.header.vault, vault_before);
    assert_eq!(market.header.c_tot, c_tot_before);
    assert_eq!(market.header.insurance, insurance_before);
    assert_eq!(market.validate_shape(), Ok(()));
}

#[kani::proof]
#[kani::unwind(48)]
#[kani::solver(cadical)]
fn proof_v16_public_insurance_lien_consume_spends_only_its_domain_budget() {
    let atoms = 3u128;
    let amount = atoms * BOUND_SCALE;
    let (mut header, mut markets, _, _) = one_market_view_fixture();
    header.vault = V16PodU128::new(atoms);
    header.insurance = V16PodU128::new(atoms);
    markets[0].engine.insurance_domain_budget_long = V16PodU128::new(atoms);
    markets[0].engine.insurance_reservation_long =
        InsuranceCreditReservationV16Account::from_runtime(&InsuranceCreditReservationV16 {
            insurance_credit_reserved_num: amount,
            valid_liened_insurance_num: amount,
            ..InsuranceCreditReservationV16::EMPTY
        });
    markets[0].engine.source_credit_long =
        SourceCreditStateV16Account::from_runtime(&SourceCreditStateV16 {
            insurance_credit_reserved_num: amount,
            valid_liened_insurance_num: amount,
            credit_rate_num: CREDIT_RATE_SCALE,
            ..SourceCreditStateV16::EMPTY
        });
    let vault_before = header.vault;
    let c_tot_before = header.c_tot;
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    market
        .consume_source_credit_lien_from_insurance_not_atomic(0, amount)
        .unwrap();
    let reservation = market.markets[0]
        .engine
        .insurance_reservation_long
        .try_to_runtime()
        .unwrap();
    let source = market.markets[0]
        .engine
        .source_credit_long
        .try_to_runtime()
        .unwrap();

    kani::cover!(atoms > 1, "public insurance lien consume is nontrivial");
    assert_eq!(reservation.insurance_credit_reserved_num, 0);
    assert_eq!(reservation.valid_liened_insurance_num, 0);
    assert_eq!(reservation.consumed_insurance_num, amount);
    assert_eq!(source.insurance_credit_reserved_num, 0);
    assert_eq!(source.valid_liened_insurance_num, 0);
    assert_eq!(
        market.markets[0].engine.insurance_domain_spent_long.get(),
        atoms
    );
    assert_eq!(market.header.insurance.get(), 0);
    assert_eq!(market.header.vault, vault_before);
    assert_eq!(market.header.c_tot, c_tot_before);
    assert_eq!(market.validate_shape(), Ok(()));
}

#[kani::proof]
#[kani::unwind(32)]
#[kani::solver(cadical)]
fn proof_v16_public_insurance_reserve_rejects_unfunded_domain() {
    let amount_raw: u8 = kani::any();
    kani::assume((1..=5).contains(&amount_raw));
    let amount = amount_raw as u128 * BOUND_SCALE;
    let (mut header, mut markets, _, _) = one_market_view_fixture();
    header.vault = V16PodU128::new(10);
    header.insurance = V16PodU128::new(10);
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    let result = market.reserve_insurance_credit_not_atomic(0, amount);

    kani::cover!(
        result == Err(V16Error::LockActive),
        "unfunded domain insurance reservation reaches isolation guard"
    );
    assert_eq!(result, Err(V16Error::LockActive));
}

#[kani::proof]
#[kani::unwind(48)]
#[kani::solver(cadical)]
fn proof_v16_public_insurance_reserve_encumbers_budget_without_value_movement() {
    let atoms = 3u128;
    let amount = atoms * BOUND_SCALE;
    let (mut header, mut markets, _, _) = one_market_view_fixture();
    header.vault = V16PodU128::new(atoms);
    header.insurance = V16PodU128::new(atoms);
    markets[0].engine.insurance_domain_budget_long = V16PodU128::new(atoms);
    let vault_before = header.vault;
    let c_tot_before = header.c_tot;
    let insurance_before = header.insurance;
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    market
        .reserve_insurance_credit_not_atomic(0, amount)
        .unwrap();
    let reservation = market.markets[0]
        .engine
        .insurance_reservation_long
        .try_to_runtime()
        .unwrap();
    let source = market.markets[0]
        .engine
        .source_credit_long
        .try_to_runtime()
        .unwrap();

    kani::cover!(
        atoms > 1,
        "funded domain insurance reservation is nontrivial"
    );
    assert_eq!(reservation.insurance_credit_reserved_num, amount);
    assert_eq!(reservation.valid_liened_insurance_num, 0);
    assert_eq!(source.insurance_credit_reserved_num, amount);
    assert_eq!(source.valid_liened_insurance_num, 0);
    assert_eq!(market.header.vault, vault_before);
    assert_eq!(market.header.c_tot, c_tot_before);
    assert_eq!(market.header.insurance, insurance_before);
    assert_eq!(market.validate_shape(), Ok(()));
}

#[kani::proof]
#[kani::unwind(16)]
#[kani::solver(cadical)]
fn proof_v16_insurance_lien_split_consume_spends_exact_reserved_atoms() {
    let first_raw: u8 = kani::any();
    let second_raw: u8 = kani::any();
    kani::assume((1..=5).contains(&first_raw));
    kani::assume((1..=5).contains(&second_raw));
    let first_atoms = first_raw as u128;
    let second_atoms = second_raw as u128;
    let first_num = first_atoms * BOUND_SCALE;
    let second_num = second_atoms * BOUND_SCALE;
    let total_num = first_num + second_num;
    let total_atoms = first_atoms + second_atoms;
    let reservation = InsuranceCreditReservationV16 {
        insurance_credit_reserved_num: total_num,
        valid_liened_insurance_num: total_num,
        ..InsuranceCreditReservationV16::EMPTY
    };
    let source = SourceCreditStateV16 {
        insurance_credit_reserved_num: total_num,
        valid_liened_insurance_num: total_num,
        credit_rate_num: CREDIT_RATE_SCALE,
        ..SourceCreditStateV16::EMPTY
    };

    let (reservation, source, spent, insurance) =
        MarketGroupV16ViewMut::<u64>::kani_prepare_insurance_lien_consume_delta(
            reservation,
            source,
            0,
            total_atoms,
            first_num,
        )
        .unwrap();
    let (reservation, source, spent, insurance) =
        MarketGroupV16ViewMut::<u64>::kani_prepare_insurance_lien_consume_delta(
            reservation,
            source,
            spent,
            insurance,
            second_num,
        )
        .unwrap();

    kani::cover!(
        first_atoms > 1 && second_atoms > 1,
        "split aligned insurance-lien consumption is nontrivial"
    );
    assert_eq!(spent, total_atoms);
    assert_eq!(insurance, 0);
    assert_eq!(reservation.insurance_credit_reserved_num, 0);
    assert_eq!(reservation.valid_liened_insurance_num, 0);
    assert_eq!(reservation.consumed_insurance_num, total_num);
    assert_eq!(source.insurance_credit_reserved_num, 0);
    assert_eq!(source.valid_liened_insurance_num, 0);
}

#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn proof_v16_insurance_lien_fractional_consume_rejects() {
    let atoms_raw: u8 = kani::any();
    kani::assume((1..=5).contains(&atoms_raw));
    let available_num = (atoms_raw as u128 + 1) * BOUND_SCALE;
    let fractional_num = (atoms_raw as u128 * BOUND_SCALE) + 1;
    let reservation = InsuranceCreditReservationV16 {
        insurance_credit_reserved_num: available_num,
        valid_liened_insurance_num: available_num,
        ..InsuranceCreditReservationV16::EMPTY
    };
    let source = SourceCreditStateV16 {
        insurance_credit_reserved_num: available_num,
        valid_liened_insurance_num: available_num,
        credit_rate_num: CREDIT_RATE_SCALE,
        ..SourceCreditStateV16::EMPTY
    };

    let result = MarketGroupV16ViewMut::<u64>::kani_prepare_insurance_lien_consume_delta(
        reservation,
        source,
        0,
        atoms_raw as u128 + 1,
        fractional_num,
    );

    kani::cover!(
        fractional_num > BOUND_SCALE,
        "fractional insurance-lien consume reaches alignment guard"
    );
    assert_eq!(result, Err(V16Error::InvalidConfig));
}

#[kani::proof]
#[kani::unwind(16)]
#[kani::solver(cadical)]
fn proof_v16_expired_counterparty_backing_bucket_accepts_receivable_refill() {
    let amount_raw: u8 = kani::any();
    let receivable_raw: u8 = kani::any();
    kani::assume((1..=5).contains(&amount_raw));
    kani::assume((1..=5).contains(&receivable_raw));
    let amount = amount_raw as u128;
    let receivable = receivable_raw as u128;
    let bucket = BackingBucketV16 {
        market_id: 1,
        consumed_liened_backing_num: receivable,
        expiry_slot: 4,
        status: BackingBucketStatusV16::Expired,
        ..BackingBucketV16::EMPTY
    };
    let source = SourceCreditStateV16 {
        spent_backing_num: receivable,
        provider_receivable_num: receivable,
        credit_rate_num: CREDIT_RATE_SCALE,
        ..SourceCreditStateV16::EMPTY
    };

    let (next_bucket, next_source) =
        MarketGroupV16ViewMut::<u64>::kani_prepare_counterparty_backing_add_delta(
            bucket, source, amount, 10, 20,
        )
        .unwrap();
    let refill = amount.min(receivable);

    kani::cover!(amount < receivable, "partial expired-bucket refill");
    kani::cover!(amount >= receivable, "complete expired-bucket refill");
    assert_eq!(next_bucket.status, BackingBucketStatusV16::Fresh);
    assert_eq!(next_bucket.expiry_slot, 20);
    assert_eq!(next_bucket.consumed_liened_backing_num, receivable - refill);
    assert_eq!(next_source.provider_receivable_num, receivable - refill);
    assert_eq!(next_bucket.fresh_unliened_backing_num, amount);
    assert_eq!(next_source.fresh_reserved_backing_num, amount);
}

#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn proof_v16_source_credit_lien_face_and_backing_use_scaled_units() {
    let effective_raw: u8 = kani::any();
    let divisor_raw: u8 = kani::any();
    kani::assume((1..=5).contains(&effective_raw));
    kani::assume((1..=5).contains(&divisor_raw));
    let effective = effective_raw as u128;
    let divisor = divisor_raw as u128;
    let rate = CREDIT_RATE_SCALE / divisor;

    let (required_face_num, required_backing_num) =
        MarketGroupV16ViewMut::<u64>::kani_source_credit_lien_amounts_for_effective(
            effective, rate,
        )
        .unwrap();
    let realized_scaled = required_face_num.checked_mul(rate).unwrap() / CREDIT_RATE_SCALE;

    kani::cover!(
        divisor == 1 && effective > 1,
        "full-rate source lien sizing branch"
    );
    kani::cover!(
        divisor > 1 && required_face_num > required_backing_num,
        "partial-rate source lien sizing branch"
    );
    assert_eq!(required_backing_num, effective * BOUND_SCALE);
    if rate == CREDIT_RATE_SCALE {
        assert_eq!(required_face_num, required_backing_num);
    }
    assert!(required_face_num >= required_backing_num);
    assert!(realized_scaled >= required_backing_num);
}

#[kani::proof]
#[kani::unwind(16)]
#[kani::solver(cadical)]
fn proof_v16_counterparty_credit_consumption_reports_atoms_not_scaled_backing() {
    let effective_raw: u8 = kani::any();
    kani::assume((1..=5).contains(&effective_raw));
    let effective = effective_raw as u128;
    let (required_face_num, backing_num) =
        MarketGroupV16ViewMut::<u64>::kani_source_credit_lien_amounts_for_effective(
            effective,
            CREDIT_RATE_SCALE,
        )
        .unwrap();
    let source_credit = SourceCreditStateV16 {
        positive_claim_bound_num: required_face_num,
        exact_positive_claim_num: required_face_num,
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
    let (backing_after_create, source_after_create) =
        MarketGroupV16ViewMut::<u64>::kani_prepare_counterparty_lien_create_delta(
            backing_bucket,
            source_credit,
            0,
            backing_num,
        )
        .unwrap();
    let (backing_after_consume, source_after_consume) =
        MarketGroupV16ViewMut::<u64>::kani_prepare_counterparty_lien_consume_delta(
            backing_after_create,
            source_after_create,
            backing_num,
        )
        .unwrap();
    let cure_atoms =
        MarketGroupV16ViewMut::<u64>::kani_counterparty_cure_atoms_from_scaled_backing(backing_num)
            .unwrap();

    kani::cover!(
        effective > 1,
        "counterparty source-credit consume uses nontrivial atom value"
    );
    assert_eq!(required_face_num, backing_num);
    assert_eq!(backing_num, effective * BOUND_SCALE);
    assert_eq!(cure_atoms, effective);
    assert_ne!(cure_atoms, backing_num);
    assert_eq!(backing_after_consume.fresh_unliened_backing_num, 0);
    assert_eq!(backing_after_consume.valid_liened_backing_num, 0);
    assert_eq!(
        backing_after_consume.consumed_liened_backing_num,
        backing_num
    );
    assert_eq!(source_after_consume.fresh_reserved_backing_num, 0);
    assert_eq!(source_after_consume.valid_liened_backing_num, 0);
    assert_eq!(source_after_consume.spent_backing_num, backing_num);
    assert_eq!(source_after_consume.provider_receivable_num, backing_num);
}

#[kani::proof]
#[kani::unwind(24)]
#[kani::solver(cadical)]
fn proof_v16_counterparty_source_credit_support_does_not_debit_vault_or_insurance() {
    let amount_raw: u8 = kani::any();
    kani::assume((1..=5).contains(&amount_raw));
    let amount = amount_raw as u128;
    let vault_before: u128 = kani::any();
    kani::assume(vault_before <= 1_000_000);

    let proof = TokenValueFlowProofV16::support_to_account_capital(
        amount,
        amount,
        0,
        0,
        vault_before,
        vault_before,
    )
    .unwrap();

    kani::cover!(
        amount > 1,
        "counterparty-backed source credit support mints account capital without insurance spend"
    );
    assert_eq!(proof.vault_after, vault_before);
    assert_eq!(proof.external_quote_in, 0);
    assert_eq!(proof.external_quote_out, 0);
    assert_eq!(
        proof.debits[TokenValueClassV16::AccountCapital as usize],
        amount
    );
    assert_eq!(
        proof.credits[TokenValueClassV16::CloseCounterpartyCreditConsumed as usize],
        amount
    );
    assert_eq!(
        proof.credits[TokenValueClassV16::CloseInsuranceSpent as usize],
        0
    );
    assert_eq!(
        proof.debits[TokenValueClassV16::InsuranceCapital as usize],
        0
    );
    assert_eq!(proof.validate(), Ok(()));
}

#[kani::proof]
#[kani::unwind(24)]
#[kani::solver(cadical)]
fn proof_v16_counterparty_source_credit_support_is_prebacked_by_realized_capital() {
    let amount_raw: u8 = kani::any();
    kani::assume((1..=5).contains(&amount_raw));
    let amount = amount_raw as u128;
    let c_tot_before: u128 = kani::any();
    kani::assume(amount <= c_tot_before && c_tot_before <= 1_000_000);
    let vault = c_tot_before;

    let reserve_proof =
        TokenValueFlowProofV16::account_capital_to_realized_loss(amount, vault, vault).unwrap();
    let c_tot_after_reserve = c_tot_before - amount;

    let support_proof =
        TokenValueFlowProofV16::support_to_account_capital(amount, amount, 0, 0, vault, vault)
            .unwrap();
    let c_tot_after_support = c_tot_after_reserve + amount;

    kani::cover!(
        amount > 1 && c_tot_before > amount,
        "counterparty support is backed by a prior nontrivial capital reservation"
    );
    assert_eq!(
        reserve_proof.debits[TokenValueClassV16::AccountCapital as usize],
        amount
    );
    assert_eq!(
        reserve_proof.credits[TokenValueClassV16::ExplicitBackedLoss as usize],
        amount
    );
    assert_eq!(
        support_proof.credits[TokenValueClassV16::CloseCounterpartyCreditConsumed as usize],
        amount
    );
    assert_eq!(
        support_proof.debits[TokenValueClassV16::AccountCapital as usize],
        amount
    );
    assert_eq!(reserve_proof.validate(), Ok(()));
    assert_eq!(support_proof.validate(), Ok(()));
    assert_eq!(c_tot_after_support, c_tot_before);
    assert_eq!(reserve_proof.vault_after, vault);
    assert_eq!(support_proof.vault_after, vault);
}

#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn proof_v16_nontrivial_public_profile_satisfies_symbolic_mm_envelope() {
    let x_raw: u16 = kani::any();

    kani::assume((1..=4_096).contains(&x_raw));

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

    let x = x_raw as u128;

    kani::cover!(
        x > 64,
        "nontrivial accepted config covers interior notionals beyond endpoint checks"
    );
    assert!(x <= MAX_ACCOUNT_NOTIONAL);
    assert_eq!(cfg.kani_solvency_envelope_holds_for_notional(x), Ok(true));
}
