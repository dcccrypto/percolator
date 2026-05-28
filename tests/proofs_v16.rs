#![cfg(kani)]

use percolator::v16::{
    BackingBucketStatusV16, BackingBucketV16, EngineAssetSlotV16Account,
    InsuranceCreditReservationV16, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    PortfolioAccountV16Account, PortfolioLegV16, PortfolioSourceDomainV16Account,
    PortfolioV16ViewMut, ProvenanceHeaderV16, ProvenanceHeaderV16Account, SideV16,
    SourceCreditStateV16, TokenValueClassV16, TokenValueFlowProofV16, V16Config, V16Error,
    V16PodI128, V16PodU128, V16PodU64,
};
use percolator::{ADL_ONE, BOUND_SCALE, CREDIT_RATE_SCALE, POS_SCALE};

fn ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    ([1; 32], [2; 32], [3; 32])
}

fn one_market_view_fixture() -> (
    MarketGroupV16HeaderAccount,
    [Market<u64>; 1],
    PortfolioAccountV16Account,
    [PortfolioSourceDomainV16Account; 2],
) {
    let (market_id, account_id, owner) = ids();
    let cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(market_id, cfg, 1, 0).unwrap();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    {
        let mut view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        view.activate_empty_market_not_atomic(0, 100, 1).unwrap();
    }
    let account_header =
        PortfolioAccountV16Account::try_empty(ProvenanceHeaderV16Account::from_runtime(
            &ProvenanceHeaderV16::new(market_id, account_id, owner),
        ))
        .unwrap();
    let source_domains = [PortfolioSourceDomainV16Account::default(); 2];
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
fn proof_v16_view_overwithdraw_rejects_without_mutation() {
    let (mut header, mut markets, mut account_header, mut source_domains) =
        one_market_view_fixture();
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    market.deposit_not_atomic(&mut account, 3).unwrap();
    let before_vault = market.header.vault;
    let before_c_tot = market.header.c_tot;
    let before_capital = account.header.capital;

    let result = market.withdraw_not_atomic(&mut account, 4);

    kani::cover!(
        result == Err(V16Error::LockActive),
        "view overwithdraw lock branch reachable"
    );
    assert_eq!(result, Err(V16Error::LockActive));
    assert_eq!(market.header.vault, before_vault);
    assert_eq!(market.header.c_tot, before_c_tot);
    assert_eq!(account.header.capital, before_capital);
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
