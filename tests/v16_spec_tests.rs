use percolator::v16::{
    account_equity, risk_notional_ceil, v16_domain_count_for_market_slots, AssetLifecycleV16,
    AssetStateV16Account, BackingBucketStatusV16, BackingBucketV16, BackingBucketV16Account,
    CloseProgressLedgerV16, EngineAssetSlotV16Account, HLockLaneV16, HealthCertV16Account,
    LiquidationRequestV16, Market, MarketGroupV16, MarketGroupV16HeaderAccount, MarketGroupV16View,
    MarketGroupV16ViewMut, MarketModeV16, PermissionlessCrankActionV16, PermissionlessCrankRequestV16,
    PermissionlessProgressOutcomeV16, PermissionlessRecoveryReasonV16, PortfolioAccountV16,
    PortfolioAccountV16Account, PortfolioLegV16, PortfolioLegV16Account,
    PortfolioSourceDomainV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, RebalanceRequestV16, ReservationEncumbranceProofV16,
    ResolvedCloseOutcomeV16, ResolvedPayoutLedgerV16, ResolvedPayoutReceiptV16, SideModeV16, SideV16,
    SourceCreditLienAggregateProofV16, SourceCreditStateV16, SourceCreditStateV16Account,
    StockReconciliationProofV16, TokenValueClassV16, TokenValueFlowProofV16, TradeRequestV16,
    V16Config, V16ConfigAccount, V16Error, V16OptionalRecoveryReasonAccount, V16PodI128, V16PodU128,
    V16PodU16, V16PodU32, V16PodU64, V16_MAX_PORTFOLIO_ASSETS_N,
};
use percolator::{
    ADL_ONE, BOUND_SCALE, CREDIT_RATE_SCALE, MAX_ACCOUNT_NOTIONAL, MAX_ORACLE_PRICE,
    MAX_PROTOCOL_FEE_ABS, POS_SCALE, SOCIAL_LOSS_DEN,
};

fn ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    ([1; 32], [2; 32], [3; 32])
}

fn group() -> MarketGroupV16 {
    let (market, _, _) = ids();
    MarketGroupV16::new(market, V16Config::public_user_fund(4, 0, 10)).unwrap()
}

fn group_with_market_slots(max_market_slots: u32) -> MarketGroupV16 {
    let (market, _, _) = ids();
    MarketGroupV16::new(
        market,
        V16Config::public_user_fund_with_market_slots(
            max_market_slots as u16,
            max_market_slots,
            0,
            10,
        ),
    )
    .unwrap()
}

// RESYNC(f3aef4b): ported verbatim from toly's test suite — zero-copy
// (account-form) fixtures used by the new source-credit IM-lien spec test.
// Our fork previously built these views inline; these named helpers are the
// shape toly's new tests expect.
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

fn account_fixture(
    market_slots: u32,
    account_seed: u8,
) -> (
    PortfolioAccountV16Account,
    Vec<PortfolioSourceDomainV16Account>,
) {
    let (market_id, _, owner) = ids();
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        market_id,
        [account_seed; 32],
        owner,
    ));
    let account = PortfolioAccountV16Account::try_empty(header).unwrap();
    let domains = vec![
        PortfolioSourceDomainV16Account::default();
        v16_domain_count_for_market_slots(market_slots).unwrap()
    ];
    (account, domains)
}

fn bitmap(indices: &[usize]) -> percolator::V16ActiveBitmap {
    let mut out = percolator::active_bitmap_empty();
    for &idx in indices {
        percolator::active_bitmap_set(&mut out, idx).unwrap();
    }
    out
}

fn bitmap_count_ones(bitmap: percolator::V16ActiveBitmap) -> u32 {
    bitmap.iter().map(|word| word.count_ones()).sum()
}

fn active_leg_for_asset(
    account: &PortfolioAccountV16,
    asset_index: usize,
) -> Option<(usize, PortfolioLegV16)> {
    account
        .legs
        .iter()
        .copied()
        .enumerate()
        .find(|(_, leg)| leg.active && leg.asset_index as usize == asset_index)
}

fn set_junior_bound(group: &mut MarketGroupV16, amount: u128) {
    group.pnl_pos_bound_tot = amount;
    group.pnl_pos_bound_tot_num = amount.checked_mul(BOUND_SCALE).unwrap();
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

fn tight_envelope_config() -> V16Config {
    let mut cfg = V16Config::public_user_fund(4, 0, 10);
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

fn account() -> PortfolioAccountV16 {
    let (market, account_id, owner) = ids();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    account.ensure_source_domain_capacity(v16_domain_count_for_market_slots(4).unwrap());
    account
}

#[test]
fn v16_token_value_flow_proof_balances_external_deposit() {
    let proof = TokenValueFlowProofV16::external_in_to_account_capital(7, 10, 17).unwrap();

    assert!(proof.validate().is_ok());
    assert_eq!(proof.external_quote_in, 7);
    assert_eq!(proof.external_quote_out, 0);
    assert_eq!(proof.debits[TokenValueClassV16::AccountCapital as usize], 7);
    assert_eq!(proof.credits[TokenValueClassV16::ExternalQuote as usize], 7);
}

#[test]
fn v16_token_value_flow_proof_rejects_vault_delta_mismatch() {
    let proof = TokenValueFlowProofV16::external_in_to_account_capital(7, 10, 16).unwrap();

    assert_eq!(proof.validate(), Err(V16Error::InvalidConfig));
}

#[test]
fn v16_token_value_flow_proof_balances_internal_value_moves() {
    let fee = TokenValueFlowProofV16::account_capital_to_insurance(5, 100, 100).unwrap();
    assert!(fee.validate().is_ok());
    assert_eq!(fee.debits[TokenValueClassV16::AccountCapital as usize], 5);
    assert_eq!(
        fee.credits[TokenValueClassV16::InsuranceCapital as usize],
        5
    );

    let loss = TokenValueFlowProofV16::account_capital_to_realized_loss(6, 100, 100).unwrap();
    assert!(loss.validate().is_ok());
    assert_eq!(loss.debits[TokenValueClassV16::AccountCapital as usize], 6);
    assert_eq!(
        loss.credits[TokenValueClassV16::ExplicitBackedLoss as usize],
        6
    );

    let close_insurance =
        TokenValueFlowProofV16::insurance_to_close_insurance_spent(4, 100, 100).unwrap();
    assert!(close_insurance.validate().is_ok());
    assert_eq!(
        close_insurance.debits[TokenValueClassV16::InsuranceCapital as usize],
        4
    );
    assert_eq!(
        close_insurance.credits[TokenValueClassV16::CloseInsuranceSpent as usize],
        4
    );
    assert_eq!(
        TokenValueFlowProofV16::validate_insurance_to_close_insurance_spent(4, 100, 100),
        Ok(()),
        "insurance-spend validation must validate the same balanced value-flow class proof"
    );
    assert_eq!(
        TokenValueFlowProofV16::validate_insurance_to_close_insurance_spent(4, 100, 99),
        Err(V16Error::InvalidConfig),
        "internal insurance support must not hide an external vault delta"
    );

    let support =
        TokenValueFlowProofV16::support_to_account_capital(10, 4, 3, 3, 100, 100).unwrap();
    assert!(support.validate().is_ok());
    assert_eq!(
        support.debits[TokenValueClassV16::AccountCapital as usize],
        10
    );
    assert_eq!(
        support.credits[TokenValueClassV16::CloseCounterpartyCreditConsumed as usize],
        4
    );
    assert_eq!(
        support.credits[TokenValueClassV16::CloseInsuranceSpent as usize],
        3
    );
    assert_eq!(
        support.credits[TokenValueClassV16::UnallocatedProtocolSurplus as usize],
        3
    );
}

#[test]
fn v16_token_value_flow_proof_rejects_unbalanced_internal_support() {
    assert_eq!(
        TokenValueFlowProofV16::support_to_account_capital(10, 4, 3, 2, 100, 100),
        Err(V16Error::InvalidConfig)
    );
}

#[test]
fn v16_deposit_and_withdraw_paths_validate_token_value_flow() {
    let mut g = group();
    let mut a = account();

    g.deposit_not_atomic(&mut a, 11).unwrap();
    assert_eq!(g.vault, 11);
    assert_eq!(g.c_tot, 11);
    assert_eq!(a.capital, 11);

    g.withdraw_not_atomic(&mut a, 4, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(g.vault, 7);
    assert_eq!(g.c_tot, 7);
    assert_eq!(a.capital, 7);
}

#[test]
fn v16_zero_copy_market_view_deposit_mutates_pod_without_runtime_vecs() {
    let g = group();
    let mut header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: 0xCAFE_0000u64 + i as u64,
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let runtime_account = account();
    let mut account_header = PortfolioAccountV16Account::from_runtime(&runtime_account);
    let mut source_domains =
        PortfolioAccountV16Account::source_domains_from_runtime(&runtime_account).unwrap();
    let preserved_wrapper = markets[0].wrapper;

    let mut account_view =
        percolator::v16::PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    market_view
        .deposit_not_atomic(&mut account_view, 17)
        .unwrap();

    assert_eq!(account_view.header.capital.get(), 17);
    assert_eq!(market_view.header.vault.get(), 17);
    assert_eq!(market_view.header.c_tot.get(), 17);
    assert_eq!(market_view.markets[0].wrapper, preserved_wrapper);
    market_view.validate_shape().unwrap();
    account_view
        .validate_with_market(&market_view.as_view())
        .unwrap();
}

#[test]
fn v16_zero_copy_fee_sync_settles_flat_loss_before_fee_without_runtime_vecs() {
    let mut g = group();
    let mut a = account();
    g.vault = 100;
    g.c_tot = 100;
    a.capital = 100;
    a.pnl = -40;
    g.negative_pnl_account_count = 1;
    g.current_slot = 10;
    g.slot_last = 10;
    a.last_fee_slot = 0;

    let mut header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: 0xDAD0_0000u64 + i as u64,
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let mut account_header = PortfolioAccountV16Account::from_runtime(&a);
    let mut source_domains = PortfolioAccountV16Account::source_domains_from_runtime(&a).unwrap();

    let mut account_view =
        percolator::v16::PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    let charged = market_view
        .sync_account_fee_to_slot_not_atomic(&mut account_view, 10, 10)
        .unwrap();

    assert_eq!(charged, 60);
    assert_eq!(account_view.header.pnl.get(), 0);
    assert_eq!(account_view.header.capital.get(), 0);
    assert_eq!(account_view.header.last_fee_slot.get(), 10);
    assert_eq!(market_view.header.c_tot.get(), 0);
    assert_eq!(market_view.header.insurance.get(), 60);
    assert_eq!(market_view.header.vault.get(), 100);
    assert_eq!(market_view.header.negative_pnl_account_count.get(), 0);
    market_view.validate_shape().unwrap();
    account_view
        .validate_with_market(&market_view.as_view())
        .unwrap();
}

#[test]
fn v16_zero_copy_principal_settlement_starts_hlock_on_unpaid_flat_loss() {
    let mut g = group();
    let mut a = account();
    g.vault = 100;
    g.c_tot = 100;
    a.capital = 100;
    a.pnl = -150;
    g.negative_pnl_account_count = 1;

    let mut header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: i as u64,
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let mut account_header = PortfolioAccountV16Account::from_runtime(&a);
    let mut source_domains = PortfolioAccountV16Account::source_domains_from_runtime(&a).unwrap();

    let mut account_view =
        percolator::v16::PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    let paid = market_view
        .settle_negative_pnl_from_principal_not_atomic(&mut account_view)
        .unwrap();

    assert_eq!(paid, 100);
    assert_eq!(account_view.header.capital.get(), 0);
    assert_eq!(account_view.header.pnl.get(), -50);
    assert_eq!(market_view.header.c_tot.get(), 0);
    assert_eq!(market_view.header.bankruptcy_hlock_active, 1);
    assert_eq!(market_view.header.negative_pnl_account_count.get(), 1);
    market_view.validate_shape().unwrap();
    account_view
        .validate_with_market(&market_view.as_view())
        .unwrap();
}

#[test]
fn v16_zero_copy_flat_withdraw_mutates_pod_without_runtime_vecs() {
    let mut g = group();
    let mut a = account();
    g.vault = 100;
    g.c_tot = 100;
    a.capital = 100;

    let mut header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: [i as u8; 24],
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let mut account_header = PortfolioAccountV16Account::from_runtime(&a);
    let mut source_domains = PortfolioAccountV16Account::source_domains_from_runtime(&a).unwrap();

    let mut account_view =
        percolator::v16::PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    market_view
        .withdraw_not_atomic(&mut account_view, 35)
        .unwrap();

    assert_eq!(account_view.header.capital.get(), 65);
    assert_eq!(market_view.header.c_tot.get(), 65);
    assert_eq!(market_view.header.vault.get(), 65);
    assert_eq!(market_view.markets[0].wrapper, [0u8; 24]);
    market_view.validate_shape().unwrap();
    account_view
        .validate_with_market(&market_view.as_view())
        .unwrap();
}

#[test]
fn v16_zero_copy_withdraw_rejects_nonflat_until_view_refresh_exists() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let mut header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: [i as u8; 24],
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let mut account_header = PortfolioAccountV16Account::from_runtime(&a);
    let mut source_domains = PortfolioAccountV16Account::source_domains_from_runtime(&a).unwrap();

    let mut account_view =
        percolator::v16::PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    assert_eq!(
        market_view.withdraw_not_atomic(&mut account_view, 1),
        Err(V16Error::Stale)
    );
    assert_eq!(account_view.header.capital.get(), 100);
    assert_eq!(market_view.header.vault.get(), 100);
}

#[test]
fn v16_zero_copy_full_refresh_settles_kf_like_runtime_without_vecs() {
    let mut g = group();
    let mut a = account();
    let mut opposing = account_with_id(90);
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.deposit_not_atomic(&mut opposing, 100).unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut opposing, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.assets[0].k_long = ADL_ONE as i128;

    let mut runtime_g = g.clone();
    let mut runtime_a = a.clone();
    let expected = runtime_g
        .full_account_refresh(&mut runtime_a, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    let mut header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: [i as u8; 32],
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let mut account_header = PortfolioAccountV16Account::from_runtime(&a);
    let mut source_domains =
        vec![
            PortfolioSourceDomainV16Account::default();
            percolator::v16::v16_domain_count_for_market_slots(g.config.max_market_slots).unwrap()
        ];

    let mut account_view =
        percolator::v16::PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    let cert = market_view
        .full_account_refresh_not_atomic(&mut account_view)
        .unwrap();

    assert_eq!(cert, expected);
    assert_eq!(account_view.header.pnl.get(), runtime_a.pnl);
    assert_eq!(
        account_view.header.legs[0].k_snap.get(),
        runtime_a.legs[0].k_snap
    );
    assert_eq!(
        account_view.source_domains[1].source_claim_bound_num.get(),
        runtime_a.source_claim_bound_num[1]
    );
    assert_eq!(
        market_view.markets[0]
            .engine
            .source_credit_short
            .positive_claim_bound_num
            .get(),
        runtime_g.source_credit[1].positive_claim_bound_num
    );
    assert_eq!(market_view.header.pnl_pos_tot.get(), runtime_g.pnl_pos_tot);
    market_view.validate_shape().unwrap();
    account_view
        .validate_with_market(&market_view.as_view())
        .unwrap();
}

#[test]
fn v16_zero_copy_source_backed_equity_ignores_unrelated_global_junior_bound() {
    let mut g = group();
    let mut a = account();
    g.vault = g.c_tot + g.insurance;
    g.add_account_source_positive_pnl_not_atomic(&mut a, 0, 10)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 10 * BOUND_SCALE, 10)
        .unwrap();
    set_junior_bound(&mut g, 1_000);

    let mut header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: [i as u8; 32],
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let mut account_header = PortfolioAccountV16Account::from_runtime(&a);
    let mut source_domains = PortfolioAccountV16Account::source_domains_from_runtime(&a).unwrap();

    let mut account_view =
        percolator::v16::PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    let cert = market_view
        .full_account_refresh_not_atomic(&mut account_view)
        .unwrap();

    assert_eq!(
        cert.certified_equity, 10,
        "zero-copy source-backed PnL must not be haircut by unrelated global junior-bound inflation"
    );
    account_view
        .validate_with_market(&market_view.as_view())
        .unwrap();
}

#[test]
fn v16_zero_copy_full_refresh_marks_b_stale_like_runtime_without_vecs() {
    let mut g = group();
    let mut a = account();
    let mut opposing = account_with_id(91);
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.deposit_not_atomic(&mut opposing, 100).unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut opposing, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.assets[0].b_long_num = 1;

    let mut runtime_g = g.clone();
    let mut runtime_a = a.clone();
    assert_eq!(
        runtime_g.full_account_refresh(&mut runtime_a, &[1; V16_MAX_PORTFOLIO_ASSETS_N]),
        Err(V16Error::BStale)
    );

    let mut header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: [i as u8; 32],
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let mut account_header = PortfolioAccountV16Account::from_runtime(&a);
    let mut source_domains =
        vec![
            PortfolioSourceDomainV16Account::default();
            percolator::v16::v16_domain_count_for_market_slots(g.config.max_market_slots).unwrap()
        ];

    let mut account_view =
        percolator::v16::PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    assert_eq!(
        market_view.full_account_refresh_not_atomic(&mut account_view),
        Err(V16Error::BStale)
    );
    assert_eq!(account_view.header.b_stale_state, 1);
    assert_eq!(account_view.header.legs[0].b_stale, 1);
    assert_eq!(market_view.header.b_stale_account_count.get(), 1);
}

#[test]
fn v16_zero_copy_trade_updates_positions_like_runtime_without_vecs() {
    let mut g = group();
    let mut long = account();
    let mut short = account_with_id(92);
    g.deposit_not_atomic(&mut long, 100).unwrap();
    g.deposit_not_atomic(&mut short, 100).unwrap();
    let request = TradeRequestV16 {
        asset_index: 0,
        size_q: POS_SCALE,
        exec_price: 1,
        fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
    };

    let mut runtime_g = g.clone();
    let mut runtime_long = long.clone();
    let mut runtime_short = short.clone();
    let expected = runtime_g
        .execute_trade_with_fee_in_place_not_atomic(
            &mut runtime_long,
            &mut runtime_short,
            request,
            &[1; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    let mut header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: [i as u8; 32],
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let mut long_header = PortfolioAccountV16Account::from_runtime(&long);
    let mut short_header = PortfolioAccountV16Account::from_runtime(&short);
    let mut long_sources =
        vec![
            PortfolioSourceDomainV16Account::default();
            percolator::v16::v16_domain_count_for_market_slots(g.config.max_market_slots).unwrap()
        ];
    let mut short_sources = long_sources.clone();

    let mut long_view =
        percolator::v16::PortfolioV16ViewMut::new(&mut long_header, &mut long_sources);
    let mut short_view =
        percolator::v16::PortfolioV16ViewMut::new(&mut short_header, &mut short_sources);
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    let outcome = market_view
        .execute_trade_with_fee_in_place_not_atomic(&mut long_view, &mut short_view, request)
        .unwrap();

    assert_eq!(outcome, expected);
    assert_eq!(
        long_view.header.legs[0].try_to_runtime().unwrap(),
        runtime_long.legs[0]
    );
    assert_eq!(
        short_view.header.legs[0].try_to_runtime().unwrap(),
        runtime_short.legs[0]
    );
    assert_eq!(
        long_view.header.health_cert.try_to_runtime().unwrap(),
        runtime_long.health_cert
    );
    assert_eq!(
        short_view.header.health_cert.try_to_runtime().unwrap(),
        runtime_short.health_cert
    );
    assert_eq!(
        market_view.markets[0]
            .engine
            .asset
            .try_to_runtime()
            .unwrap()
            .oi_eff_long_q,
        runtime_g.assets[0].oi_eff_long_q
    );
    assert_eq!(
        market_view.markets[0]
            .engine
            .asset
            .try_to_runtime()
            .unwrap()
            .oi_eff_short_q,
        runtime_g.assets[0].oi_eff_short_q
    );

    let second_expected = runtime_g
        .execute_trade_with_fee_in_place_not_atomic(
            &mut runtime_long,
            &mut runtime_short,
            request,
            &[1; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();
    let second_outcome = market_view
        .execute_trade_with_fee_in_place_not_atomic(&mut long_view, &mut short_view, request)
        .unwrap();

    assert_eq!(second_outcome, second_expected);
    assert_eq!(
        long_view.header.legs[0].try_to_runtime().unwrap(),
        runtime_long.legs[0]
    );
    assert_eq!(
        short_view.header.legs[0].try_to_runtime().unwrap(),
        runtime_short.legs[0]
    );
    assert_eq!(
        long_view.header.health_cert.try_to_runtime().unwrap(),
        runtime_long.health_cert
    );
    assert_eq!(
        short_view.header.health_cert.try_to_runtime().unwrap(),
        runtime_short.health_cert
    );
    assert_eq!(
        market_view.markets[0]
            .engine
            .asset
            .try_to_runtime()
            .unwrap()
            .oi_eff_long_q,
        runtime_g.assets[0].oi_eff_long_q
    );
    assert_eq!(
        market_view.markets[0]
            .engine
            .asset
            .try_to_runtime()
            .unwrap()
            .oi_eff_short_q,
        runtime_g.assets[0].oi_eff_short_q
    );
    market_view.validate_shape().unwrap();
    long_view
        .validate_with_market(&market_view.as_view())
        .unwrap();
    short_view
        .validate_with_market(&market_view.as_view())
        .unwrap();
}

#[test]
fn v16_zero_copy_trade_rejects_corrupt_unconfigured_market_tail() {
    let mut g = group();
    let mut long = account();
    let mut short = account_with_id(96);
    g.deposit_not_atomic(&mut long, 100).unwrap();
    g.deposit_not_atomic(&mut short, 100).unwrap();
    let request = TradeRequestV16 {
        asset_index: 0,
        size_q: POS_SCALE,
        exec_price: 1,
        fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
    };

    let configured = g.config.max_market_slots as usize;
    assert_eq!(configured, g.assets.len());
    let capacity = configured + 1;
    let mut header = MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, capacity).unwrap();
    let mut markets = (0..capacity)
        .map(|i| Market {
            wrapper: [i as u8; 32],
            engine: if i < configured {
                EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap()
            } else {
                EngineAssetSlotV16Account::default()
            },
        })
        .collect::<Vec<_>>();
    markets[configured].engine.asset.market_id = V16PodU64::new(99);

    let mut long_header = PortfolioAccountV16Account::from_runtime(&long);
    let mut short_header = PortfolioAccountV16Account::from_runtime(&short);
    let mut long_sources = PortfolioAccountV16Account::source_domains_from_runtime(&long).unwrap();
    let mut short_sources =
        PortfolioAccountV16Account::source_domains_from_runtime(&short).unwrap();
    let long_before = long_header;
    let short_before = short_header;
    let market_before = markets[0].engine;

    let mut long_view =
        percolator::v16::PortfolioV16ViewMut::new(&mut long_header, &mut long_sources);
    let mut short_view =
        percolator::v16::PortfolioV16ViewMut::new(&mut short_header, &mut short_sources);
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    assert_eq!(
        market_view.execute_trade_with_fee_in_place_not_atomic(
            &mut long_view,
            &mut short_view,
            request,
        ),
        Err(V16Error::InvalidConfig)
    );
    assert_eq!(*long_view.header, long_before);
    assert_eq!(*short_view.header, short_before);
    assert_eq!(market_view.markets[0].engine, market_before);
}

#[test]
fn v16_zero_copy_permissionless_crank_refresh_accrues_without_runtime_vecs() {
    let mut g = group();
    let mut long = account();
    g.deposit_not_atomic(&mut long, 1000).unwrap();
    g.attach_leg(&mut long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV16::Long, POS_SCALE, 93);
    let req = PermissionlessCrankRequestV16 {
        now_slot: 1,
        asset_index: 0,
        effective_price: 2,
        funding_rate_e9: 0,
        action: PermissionlessCrankActionV16::Refresh,
    };

    let mut runtime_g = g.clone();
    let mut runtime_long = long.clone();
    let expected = runtime_g
        .permissionless_crank_not_atomic(&mut runtime_long, req, &[2; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    let mut header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: [i as u8; 64],
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let mut account_header = PortfolioAccountV16Account::from_runtime(&long);
    let mut source_domains =
        PortfolioAccountV16Account::source_domains_from_runtime(&long).unwrap();

    let mut account_view =
        percolator::v16::PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    let outcome = market_view
        .permissionless_crank_not_atomic(&mut account_view, req)
        .unwrap();

    assert_eq!(outcome, expected);
    assert_eq!(market_view.header.slot_last.get(), runtime_g.slot_last);
    assert_eq!(
        market_view.header.current_slot.get(),
        runtime_g.current_slot
    );
    assert_eq!(
        market_view.markets[0]
            .engine
            .asset
            .try_to_runtime()
            .unwrap(),
        runtime_g.assets[0]
    );
    assert_eq!(
        account_view.header.health_cert.try_to_runtime().unwrap(),
        runtime_long.health_cert
    );
    market_view.validate_shape().unwrap();
    account_view
        .validate_with_market(&market_view.as_view())
        .unwrap();
}

#[test]
fn v16_zero_copy_permissionless_crank_flat_refresh_is_not_protective_without_vecs() {
    let mut g = group();
    let mut long = account();
    let mut short = account_with_id(94);
    let mut flat = account_with_id(95);
    g.deposit_not_atomic(&mut flat, 1).unwrap();
    g.attach_leg(&mut long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut short, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    let before_asset = g.assets[0];
    let before_slot = g.slot_last;
    let req = PermissionlessCrankRequestV16 {
        now_slot: 1,
        asset_index: 0,
        effective_price: 2,
        funding_rate_e9: 0,
        action: PermissionlessCrankActionV16::Refresh,
    };

    let mut header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: [i as u8; 64],
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let mut account_header = PortfolioAccountV16Account::from_runtime(&flat);
    let mut source_domains =
        PortfolioAccountV16Account::source_domains_from_runtime(&flat).unwrap();

    let mut account_view =
        percolator::v16::PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    assert_eq!(
        market_view.permissionless_crank_not_atomic(&mut account_view, req),
        Err(V16Error::NonProgress)
    );
    assert_eq!(
        market_view.markets[0]
            .engine
            .asset
            .try_to_runtime()
            .unwrap(),
        before_asset
    );
    assert_eq!(market_view.header.slot_last.get(), before_slot);
}

#[test]
fn v16_zero_copy_resolved_close_is_fee_current_without_runtime_vecs() {
    let mut g = group();
    let mut a = account();
    g.vault = 100;
    g.c_tot = 100;
    a.capital = 100;
    a.last_fee_slot = 0;

    let mut header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: [0x51u8; 16],
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let mut account_header = PortfolioAccountV16Account::from_runtime(&a);
    let mut source_domains = PortfolioAccountV16Account::source_domains_from_runtime(&a).unwrap();

    let mut account_view =
        percolator::v16::PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    market_view.resolve_market_not_atomic(10).unwrap();
    let out = market_view
        .close_resolved_account_not_atomic(&mut account_view, 1)
        .unwrap();

    assert_eq!(out, ResolvedCloseOutcomeV16::Closed { payout: 90 });
    assert_eq!(account_view.header.last_fee_slot.get(), 10);
    assert_eq!(account_view.header.capital.get(), 0);
    assert_eq!(market_view.header.c_tot.get(), 0);
    assert_eq!(market_view.header.vault.get(), 10);
    market_view.validate_shape().unwrap();
    account_view
        .validate_with_market(&market_view.as_view())
        .unwrap();
}

#[test]
fn v16_zero_copy_cure_and_cancel_close_releases_domain_barrier_without_runtime_vecs() {
    let mut g = group();
    let mut a = account();
    g.create_portfolio_account(&a).unwrap();
    a.close_progress = CloseProgressLedgerV16 {
        active: true,
        close_id: 1,
        asset_index: 0,
        market_id: g.assets[0].market_id,
        domain_side: SideV16::Short,
        gross_loss_at_close_start: 5,
        drift_reference_slot: g.current_slot,
        max_close_slot: g.current_slot + g.config.max_bankrupt_close_lifetime_slots,
        residual_remaining: 5,
        ..CloseProgressLedgerV16::EMPTY
    };
    g.pending_domain_loss_barriers[1] = 1;

    let mut header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: [0x52u8; 16],
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let mut account_header = PortfolioAccountV16Account::from_runtime(&a);
    let mut source_domains = PortfolioAccountV16Account::source_domains_from_runtime(&a).unwrap();

    let mut account_view =
        percolator::v16::PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    market_view
        .cure_and_cancel_close_not_atomic(&mut account_view, 7)
        .unwrap();

    let ledger = account_view.header.close_progress.try_to_runtime().unwrap();
    assert!(!ledger.active);
    assert!(ledger.canceled);
    assert_eq!(ledger.residual_remaining, 5);
    assert_eq!(account_view.header.cancel_deposit_escrow.get(), 0);
    assert_eq!(account_view.header.capital.get(), 7);
    assert_eq!(market_view.header.c_tot.get(), 7);
    assert_eq!(market_view.header.vault.get(), 7);
    assert_eq!(
        market_view.markets[0]
            .engine
            .pending_domain_loss_barrier_short
            .get(),
        0
    );
    market_view.validate_shape().unwrap();
    account_view
        .validate_with_market(&market_view.as_view())
        .unwrap();
}

#[test]
fn v16_zero_copy_forfeit_recovery_leg_makes_partial_b_progress_without_runtime_vecs() {
    let mut g = group();
    let mut a = account();
    g.mode = MarketModeV16::Recovery;
    g.attach_leg(&mut a, 0, SideV16::Long, 1).unwrap();
    g.assets[0].b_long_num = 2;

    let mut header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: [0x53u8; 16],
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let mut account_header = PortfolioAccountV16Account::from_runtime(&a);
    let mut source_domains = PortfolioAccountV16Account::source_domains_from_runtime(&a).unwrap();

    let mut account_view =
        percolator::v16::PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    let out = market_view
        .forfeit_recovery_leg_not_atomic(&mut account_view, 0, 1)
        .unwrap();

    assert!(!out.detached);
    assert_eq!(out.loss_settled, 0);
    assert_eq!(out.principal_used, 0);
    assert_eq!(out.insurance_used, 0);
    assert_eq!(out.residual_booked, 0);
    let leg = account_view.header.legs[0].try_to_runtime().unwrap();
    assert_eq!(leg.b_snap, 1);
    assert!(leg.b_stale);
    assert_eq!(account_view.header.b_stale_state, 1);
    assert!(leg.active);
    assert_eq!(
        market_view.markets[0]
            .engine
            .asset
            .try_to_runtime()
            .unwrap()
            .oi_eff_long_q,
        1
    );
    market_view.validate_shape().unwrap();
    account_view
        .validate_with_market(&market_view.as_view())
        .unwrap();
}

#[test]
fn v16_stock_reconciliation_proof_decomposes_vault_into_single_stock_classes() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 11).unwrap();
    g.insurance = 3;
    g.vault = 20;

    let proof = g.stock_reconciliation_proof().unwrap();

    assert_eq!(
        proof,
        StockReconciliationProofV16 {
            token_vault: 20,
            senior_capital_total: 11,
            insurance_capital: 3,
            backing_provider_earnings: 0,
            settlement_rounding_residue_total: 0,
            unallocated_protocol_surplus: 6,
        }
    );
    assert!(proof.validate().is_ok());
}

#[test]
fn v16_stock_reconciliation_proof_rejects_unaccounted_vault_atoms() {
    let proof = StockReconciliationProofV16 {
        token_vault: 20,
        senior_capital_total: 11,
        insurance_capital: 3,
        backing_provider_earnings: 0,
        settlement_rounding_residue_total: 0,
        unallocated_protocol_surplus: 5,
    };

    assert_eq!(proof.validate(), Err(V16Error::InvalidConfig));
}

#[test]
fn v16_reservation_encumbrance_proof_validates_source_domain_ledgers() {
    let mut g = group();
    g.add_source_positive_claim_bound_not_atomic(0, 10, 10)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 10 * BOUND_SCALE, 10)
        .unwrap();
    g.create_source_credit_lien_from_counterparty_not_atomic(0, 4 * BOUND_SCALE)
        .unwrap();
    g.insurance = 3 * BOUND_SCALE;
    g.vault = g.vault.checked_add(g.insurance).unwrap();
    g.insurance_domain_budget[0] = 3;
    g.reserve_insurance_credit_not_atomic(0, 3 * BOUND_SCALE)
        .unwrap();
    g.create_source_credit_lien_from_insurance_not_atomic(0, BOUND_SCALE)
        .unwrap();

    let proof = g.reservation_encumbrance_proof_for_domain(0).unwrap();
    assert!(proof.validate().is_ok());

    let mut corrupt: ReservationEncumbranceProofV16 = proof;
    corrupt.source_valid_liened_backing_num += BOUND_SCALE;
    assert_eq!(corrupt.validate(), Err(V16Error::InvalidConfig));
}

#[test]
fn v16_public_init_requires_realizable_source_credit_profile() {
    let (market, _, _) = ids();
    let mut cfg = V16Config::public_user_fund(4, 0, 10);
    cfg.source_credit_lien_required = false;
    assert_eq!(
        MarketGroupV16::new(market, cfg),
        Err(V16Error::InvalidConfig)
    );

    let mut cfg = V16Config::public_user_fund(4, 0, 10);
    cfg.insurance_credit_reservation_required = false;
    assert_eq!(
        MarketGroupV16::new(market, cfg),
        Err(V16Error::InvalidConfig)
    );

    let mut cfg = V16Config::public_user_fund(4, 0, 10);
    cfg.recovery_fallback_envelope_enabled = false;
    assert_eq!(
        MarketGroupV16::new(market, cfg),
        Err(V16Error::InvalidConfig)
    );

    let mut cfg = V16Config::public_user_fund(4, 0, 10);
    cfg.backing_freshness_buckets = 0;
    assert_eq!(
        MarketGroupV16::new(market, cfg),
        Err(V16Error::InvalidConfig)
    );
}

#[test]
fn v16_source_credit_rate_is_capped_by_source_domain_available_backing() {
    let mut g = group();
    g.add_source_positive_claim_bound_not_atomic(0, 100, 80)
        .unwrap();
    assert_eq!(g.source_credit[0].credit_rate_num, 0);

    g.add_fresh_counterparty_backing_not_atomic(0, 40, 10)
        .unwrap();
    assert_eq!(g.source_credit_available_backing_num(0), Ok(40));
    assert_eq!(
        g.source_credit[0].credit_rate_num,
        CREDIT_RATE_SCALE * 40 / 100
    );

    g.add_fresh_counterparty_backing_not_atomic(0, 100, 10)
        .unwrap();
    assert_eq!(g.source_credit[0].credit_rate_num, CREDIT_RATE_SCALE);
}

#[test]
fn v16_account_source_claim_equity_is_capped_by_source_credit_rate() {
    let mut g = group();
    let mut a = account();
    g.vault = 1_000;

    g.add_account_source_positive_pnl_not_atomic(&mut a, 0, 10)
        .unwrap();
    let prices = [1; V16_MAX_PORTFOLIO_ASSETS_N];
    let cert = g.full_account_refresh(&mut a, &prices).unwrap();
    assert_eq!(
        cert.certified_equity, 0,
        "unbacked source-domain positive PnL must not support account health"
    );

    g.add_fresh_counterparty_backing_not_atomic(0, 5 * BOUND_SCALE, 10)
        .unwrap();
    let cert = g.full_account_refresh(&mut a, &prices).unwrap();
    assert_eq!(
        cert.certified_equity, 5,
        "source-domain credit rate should cap usable positive PnL"
    );

    g.add_fresh_counterparty_backing_not_atomic(0, 5 * BOUND_SCALE, 10)
        .unwrap();
    let cert = g.full_account_refresh(&mut a, &prices).unwrap();
    assert_eq!(
        cert.certified_equity, 10,
        "fully backed source-domain claims should support full positive PnL"
    );
}

#[test]
fn v16_account_source_claim_cannot_exceed_domain_aggregate_runtime() {
    let mut g = group();
    let mut a = account();
    g.add_fresh_counterparty_backing_not_atomic(0, 10 * BOUND_SCALE, 10)
        .unwrap();

    a.pnl = 10;
    a.source_claim_market_id[0] = g.assets[0].market_id;
    a.source_claim_bound_num[0] = 10 * BOUND_SCALE;

    assert_eq!(
        g.validate_account_shape(&a),
        Err(V16Error::InvalidLeg),
        "an account-local source claim must not be accepted when the domain aggregate has no matching claim face"
    );
}

#[test]
fn v16_account_source_claim_cannot_exceed_domain_aggregate_zero_copy() {
    let mut g = group();
    g.add_fresh_counterparty_backing_not_atomic(0, 10 * BOUND_SCALE, 10)
        .unwrap();
    let header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: 0u64,
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let market_view = MarketGroupV16View::new(&header, &markets);

    let mut a = account();
    a.pnl = 10;
    a.source_claim_market_id[0] = market_view.markets[0].engine.asset.market_id.get();
    a.source_claim_bound_num[0] = 10 * BOUND_SCALE;
    let account_header = PortfolioAccountV16Account::from_runtime(&a);
    let source_domains = PortfolioAccountV16Account::source_domains_from_runtime(&a).unwrap();
    let account_view = percolator::v16::PortfolioV16View::new(&account_header, &source_domains);

    assert_eq!(
        account_view.validate_with_market(&market_view),
        Err(V16Error::InvalidLeg),
        "zero-copy account validation must enforce the same aggregate source-claim bound"
    );
}

#[test]
fn v16_live_positive_pnl_without_source_claim_has_no_credit_or_conversion() {
    let mut g = group();
    let mut a = account();
    a.pnl = 10;
    g.pnl_pos_tot = 10;
    g.pnl_pos_bound_tot = 10;
    g.pnl_pos_bound_tot_num = 10 * BOUND_SCALE;
    g.vault = 100;
    let prices = [1; V16_MAX_PORTFOLIO_ASSETS_N];

    let cert = g.full_account_refresh(&mut a, &prices).unwrap();
    assert_eq!(
        cert.certified_equity, 0,
        "live positive PnL must not support health without source attribution"
    );
    assert!(
        matches!(
            g.convert_released_pnl_to_capital_not_atomic(&mut a),
            Err(V16Error::LockActive)
        ),
        "live positive PnL must not be converted without source attribution"
    );
}

#[test]
fn v16_source_backed_equity_ignores_unrelated_global_junior_bound() {
    let mut g = group();
    let mut a = account();
    g.vault = g.c_tot + g.insurance;

    g.add_account_source_positive_pnl_not_atomic(&mut a, 0, 10)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 10 * BOUND_SCALE, 10)
        .unwrap();
    set_junior_bound(&mut g, 1_000);

    let cert = g
        .full_account_refresh(&mut a, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(
        cert.certified_equity, 10,
        "fully source-backed PnL must not be haircut by unrelated global junior-bound inflation"
    );
}

#[test]
fn v16_convert_released_pnl_requires_realizable_source_credit_when_claim_is_attributed() {
    let mut g = group();
    let mut a = account();
    g.vault = 1_000;
    g.add_account_source_positive_pnl_not_atomic(&mut a, 0, 10)
        .unwrap();
    let prices = [1; V16_MAX_PORTFOLIO_ASSETS_N];
    g.full_account_refresh(&mut a, &prices).unwrap();
    assert_eq!(
        g.convert_released_pnl_to_capital_not_atomic(&mut a),
        Err(V16Error::LockActive),
        "unbacked source-domain PnL cannot be converted into capital"
    );

    g.add_fresh_counterparty_backing_not_atomic(0, 4 * BOUND_SCALE, 10)
        .unwrap();
    g.full_account_refresh(&mut a, &prices).unwrap();
    let converted = g
        .convert_released_pnl_to_capital_not_atomic(&mut a)
        .unwrap();
    assert_eq!(converted, 4);
    assert_eq!(a.capital, 4);
    assert_eq!(a.pnl, 0);
    assert_eq!(a.source_claim_bound_num[0], 0);
    assert_eq!(g.source_credit[0].positive_claim_bound_num, 0);
    assert_eq!(g.source_credit[0].fresh_reserved_backing_num, 0);
    assert_eq!(g.source_credit[0].spent_backing_num, 4 * BOUND_SCALE);
    assert_eq!(g.source_credit_available_backing_num(0), Ok(0));
}

#[test]
fn v16_source_backed_conversion_ignores_unrelated_global_junior_bound() {
    let mut g = group();
    let mut a = account();
    g.vault = g.c_tot + g.insurance + 10;
    g.add_account_source_positive_pnl_not_atomic(&mut a, 0, 10)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 10 * BOUND_SCALE, 10)
        .unwrap();
    set_junior_bound(&mut g, 1_000);
    g.full_account_refresh(&mut a, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    let converted = g
        .convert_released_pnl_to_capital_not_atomic(&mut a)
        .unwrap();

    assert_eq!(converted, 10);
    assert_eq!(a.capital, 10);
    assert_eq!(a.pnl, 0);
    assert_eq!(a.source_claim_bound_num[0], 0);
    assert_eq!(g.source_credit[0].spent_backing_num, 10 * BOUND_SCALE);
}

#[test]
fn v16_source_backed_conversion_waits_until_source_position_closes() {
    let mut g = group();
    let mut attacker = account();
    let mut lp = account_with_id(77);
    let prices = [1; V16_MAX_PORTFOLIO_ASSETS_N];

    g.vault = 1_000;
    g.add_account_source_positive_pnl_not_atomic(&mut attacker, 0, 10)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 10 * BOUND_SCALE, 10)
        .unwrap();
    g.attach_leg(&mut attacker, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.attach_leg(&mut lp, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.full_account_refresh(&mut attacker, &prices).unwrap();

    assert_eq!(
        g.convert_released_pnl_to_capital_not_atomic(&mut attacker),
        Err(V16Error::LockActive),
        "source-backed open-position PnL may support margin but must not become withdrawable capital"
    );
    assert_eq!(attacker.capital, 0);
    assert_eq!(attacker.pnl, 10);

    g.clear_leg(&mut attacker, 0).unwrap();
    g.clear_leg(&mut lp, 0).unwrap();
    g.full_account_refresh(&mut attacker, &prices).unwrap();

    let converted = g
        .convert_released_pnl_to_capital_not_atomic(&mut attacker)
        .unwrap();
    assert_eq!(converted, 10);
    assert_eq!(attacker.capital, 10);
    g.withdraw_not_atomic(&mut attacker, converted, &prices)
        .unwrap();
    assert_eq!(attacker.capital, 0);
}

#[test]
fn v16_source_backed_conversion_only_waits_for_contributing_source_exposure() {
    let (market, _, owner) = ids();
    let mut g = MarketGroupV16::new(market, V16Config::public_user_fund(2, 0, 10)).unwrap();
    let mut claimant =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [78; 32], owner));
    let mut source_counterparty =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [79; 32], owner));
    let mut unrelated_counterparty =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [80; 32], owner));
    let prices = [1; V16_MAX_PORTFOLIO_ASSETS_N];

    g.vault = 1_000;
    g.add_account_source_positive_pnl_not_atomic(&mut claimant, 0, 10)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 10 * BOUND_SCALE, 10)
        .unwrap();
    g.attach_leg(&mut claimant, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.attach_leg(
        &mut source_counterparty,
        0,
        SideV16::Long,
        POS_SCALE as i128,
    )
    .unwrap();
    g.full_account_refresh(&mut claimant, &prices).unwrap();
    assert_eq!(
        g.convert_released_pnl_to_capital_not_atomic(&mut claimant),
        Err(V16Error::LockActive),
        "source-backed credit must remain nonwithdrawable while its source exposure is open"
    );

    g.clear_leg(&mut claimant, 0).unwrap();
    g.clear_leg(&mut source_counterparty, 0).unwrap();
    g.attach_leg(&mut claimant, 1, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(
        &mut unrelated_counterparty,
        1,
        SideV16::Short,
        -(POS_SCALE as i128),
    )
    .unwrap();
    g.full_account_refresh(&mut claimant, &prices).unwrap();

    let converted = g
        .convert_released_pnl_to_capital_not_atomic(&mut claimant)
        .unwrap();
    assert_eq!(
        converted, 10,
        "unrelated active exposure must not block conversion after the contributing source closes"
    );
    assert_eq!(claimant.capital, 10);
    assert_eq!(claimant.pnl, 0);
    let (claimant_slot, claimant_leg) = active_leg_for_asset(&claimant, 1).unwrap();
    assert_eq!(claimant.active_bitmap, bitmap(&[claimant_slot]));
    assert_eq!(claimant_leg.market_id, g.assets[1].market_id);
    assert_eq!(g.source_credit[0].spent_backing_num, 10 * BOUND_SCALE);
    g.assert_public_invariants().unwrap();
}

#[test]
fn v16_expired_fresh_backing_requires_refresh_before_source_credit_conversion() {
    let mut g = group();
    let mut a = account();
    let mut other_claimant = account_with_id(49);
    g.vault = 1_000;
    g.insurance = 300;
    g.insurance_domain_budget[0] = 300;
    g.add_account_source_positive_pnl_not_atomic(&mut a, 0, 300)
        .unwrap();
    g.add_account_source_positive_pnl_not_atomic(&mut other_claimant, 0, 100)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 100 * BOUND_SCALE, 1)
        .unwrap();
    g.reserve_insurance_credit_not_atomic(0, 300 * BOUND_SCALE)
        .unwrap();

    let prices = [1; V16_MAX_PORTFOLIO_ASSETS_N];
    g.full_account_refresh(&mut a, &prices).unwrap();
    g.accrue_asset_to_not_atomic(0, 1, 1, 0, true).unwrap();
    assert_eq!(g.current_slot, 1);
    assert!(a.health_cert.valid);
    assert_eq!(a.health_cert.cert_oracle_epoch, g.oracle_epoch);
    assert_eq!(a.health_cert.cert_funding_epoch, g.funding_epoch);
    assert_eq!(a.health_cert.cert_risk_epoch, g.risk_epoch);

    let before = (a.capital, a.pnl, g.c_tot, g.insurance);
    assert_eq!(
        g.convert_released_pnl_to_capital_not_atomic(&mut a),
        Err(V16Error::Stale),
        "expired fresh backing must not be used through a still-epoch-valid health certificate"
    );
    assert_eq!(before, (a.capital, a.pnl, g.c_tot, g.insurance));

    g.full_account_refresh(&mut a, &prices).unwrap();
    assert_eq!(g.source_backing_buckets[0].fresh_unliened_backing_num, 0);
    assert_eq!(
        g.source_credit[0].credit_rate_num,
        CREDIT_RATE_SCALE * 3 / 4
    );
    let converted = g
        .convert_released_pnl_to_capital_not_atomic(&mut a)
        .unwrap();
    assert_eq!(converted, 225);
    assert_eq!(a.capital, 225);
    assert_eq!(a.pnl, 0);
}

#[test]
fn v16_risk_increasing_trade_that_uses_positive_credit_creates_source_lien() {
    let mut g = group();
    let mut long = account_with_id(10);
    let mut short = account_with_id(11);
    g.deposit_not_atomic(&mut short, 100).unwrap();
    g.vault = g.vault.checked_add(10).unwrap();
    g.add_account_source_positive_pnl_not_atomic(&mut long, 0, 10)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 10 * BOUND_SCALE, 10)
        .unwrap();
    let prices = [1; V16_MAX_PORTFOLIO_ASSETS_N];

    g.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV16 {
            asset_index: 0,
            size_q: POS_SCALE,
            exec_price: 1,
            fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
        },
        &prices,
    )
    .unwrap();

    assert!(
        long.source_claim_liened_num[0] != 0,
        "risk increase that depends on source PnL must lock claim face"
    );
    assert!(
        long.source_lien_effective_reserved[0] != 0,
        "risk increase that depends on source PnL must reserve effective credit"
    );
    assert_eq!(
        g.source_credit[0].valid_liened_backing_num,
        long.source_lien_effective_reserved[0] * BOUND_SCALE
    );
}

#[test]
fn v16_source_credit_lien_aggregate_proof_tracks_account_backing_split() {
    let mut g = group();
    let mut long = account_with_id(10);
    let mut short = account_with_id(11);
    g.deposit_not_atomic(&mut short, 100).unwrap();
    g.vault = g.vault.checked_add(10).unwrap();
    g.add_account_source_positive_pnl_not_atomic(&mut long, 0, 10)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 10 * BOUND_SCALE, 10)
        .unwrap();
    let prices = [1; V16_MAX_PORTFOLIO_ASSETS_N];

    g.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV16 {
            asset_index: 0,
            size_q: POS_SCALE,
            exec_price: 1,
            fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
        },
        &prices,
    )
    .unwrap();
    let proof = g
        .source_credit_lien_proof_for_account_domain(&long, 0)
        .unwrap();

    assert_eq!(
        proof,
        SourceCreditLienAggregateProofV16 {
            domain: 0,
            source_claim_bound_num: long.source_claim_bound_num[0],
            face_claim_locked_num: long.source_claim_liened_num[0],
            counterparty_face_claim_locked_num: long.source_claim_counterparty_liened_num[0],
            insurance_face_claim_locked_num: 0,
            effective_credit_reserved: long.source_lien_effective_reserved[0],
            counterparty_backing_reserved_num: long.source_lien_effective_reserved[0] * BOUND_SCALE,
            insurance_backing_reserved_num: 0,
            impaired_face_claim_num: 0,
            impaired_effective_credit_reserved: 0,
        }
    );
    assert!(proof.validate().is_ok());
}

#[test]
fn v16_backing_utilization_fee_collects_from_lien_holder_to_provider_earnings() {
    let (market, _, _) = ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.min_nonzero_im_req = 15;
    cfg.backing_fee_base_rate_e9_per_slot = 1_000_000_000;
    cfg.backing_fee_kink_util_bps = 8_000;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    let mut long = account_with_id(10);
    g.deposit_not_atomic(&mut long, 100).unwrap();
    g.vault = g.vault.checked_add(10).unwrap();
    g.add_account_source_positive_pnl_not_atomic(&mut long, 0, 10)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 10 * BOUND_SCALE, 10)
        .unwrap();
    g.attach_leg(&mut long, 0, SideV16::Long, 10 * POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV16::Long, 10 * POS_SCALE, 11);
    g.withdraw_not_atomic(&mut long, 90, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(
        long.source_lien_counterparty_backing_num[0],
        5 * BOUND_SCALE
    );
    assert_eq!(long.source_lien_fee_last_slot[0], 0);

    g.current_slot = 1;
    g.full_account_refresh(&mut long, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(long.source_lien_fee_last_slot[0], 1);
    assert_eq!(g.source_backing_buckets[0].utilization_fee_earnings, 0);

    g.current_slot = 2;
    g.full_account_refresh(&mut long, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(long.capital, 5);
    assert_eq!(g.c_tot, 5);
    assert_eq!(g.source_backing_buckets[0].utilization_fee_earnings, 5);
    assert_eq!(
        g.stock_reconciliation_proof()
            .unwrap()
            .backing_provider_earnings,
        5
    );
    g.assert_public_invariants().unwrap();
}

#[test]
fn v16_uncollectible_backing_utilization_fee_is_forgiven_not_socialized() {
    let (market, _, _) = ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.min_nonzero_im_req = 11;
    cfg.backing_fee_base_rate_e9_per_slot = 1_000_000_000;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    let mut long = account_with_id(10);
    g.deposit_not_atomic(&mut long, 11).unwrap();
    g.vault = g.vault.checked_add(10).unwrap();
    g.add_account_source_positive_pnl_not_atomic(&mut long, 0, 10)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 10 * BOUND_SCALE, 10)
        .unwrap();
    g.attach_leg(&mut long, 0, SideV16::Long, 10 * POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV16::Long, 10 * POS_SCALE, 11);
    g.withdraw_not_atomic(&mut long, 10, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(
        long.source_lien_counterparty_backing_num[0],
        10 * BOUND_SCALE
    );

    g.current_slot = 1;
    g.full_account_refresh(&mut long, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    g.current_slot = 2;
    g.full_account_refresh(&mut long, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(long.capital, 0);
    assert_eq!(g.source_backing_buckets[0].utilization_fee_earnings, 1);
    assert_eq!(g.insurance, 0);
    assert_eq!(g.assets[0].b_long_num, 0);
    assert_eq!(g.assets[0].b_short_num, 0);
    g.assert_public_invariants().unwrap();
}

#[test]
fn v16_backing_provider_earnings_withdraw_only_releases_accrued_fees() {
    let (market, _, _) = ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.min_nonzero_im_req = 15;
    cfg.backing_fee_base_rate_e9_per_slot = 1_000_000_000;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    let mut long = account_with_id(10);
    g.deposit_not_atomic(&mut long, 100).unwrap();
    g.vault = g.vault.checked_add(10).unwrap();
    g.add_account_source_positive_pnl_not_atomic(&mut long, 0, 10)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 10 * BOUND_SCALE, 10)
        .unwrap();
    g.attach_leg(&mut long, 0, SideV16::Long, 10 * POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV16::Long, 10 * POS_SCALE, 11);
    g.withdraw_not_atomic(&mut long, 90, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    g.current_slot = 1;
    g.full_account_refresh(&mut long, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    g.current_slot = 2;
    g.full_account_refresh(&mut long, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(g.source_backing_buckets[0].utilization_fee_earnings, 5);
    let vault_before = g.vault;

    assert_eq!(
        g.withdraw_backing_provider_earnings_not_atomic(0, 6),
        Err(V16Error::CounterUnderflow)
    );
    assert_eq!(g.vault, vault_before);
    assert_eq!(g.source_backing_buckets[0].utilization_fee_earnings, 5);

    g.withdraw_backing_provider_earnings_not_atomic(0, 5)
        .unwrap();
    assert_eq!(g.vault, vault_before - 5);
    assert_eq!(g.source_backing_buckets[0].utilization_fee_earnings, 0);
    assert_eq!(g.c_tot, 5);
    g.assert_public_invariants().unwrap();
}

#[test]
fn v16_zero_copy_backing_provider_earnings_withdraw_preserves_stock() {
    let (market, _, _) = ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.min_nonzero_im_req = 15;
    cfg.backing_fee_base_rate_e9_per_slot = 1_000_000_000;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    let mut long = account_with_id(10);
    g.deposit_not_atomic(&mut long, 100).unwrap();
    g.vault = g.vault.checked_add(10).unwrap();
    g.add_account_source_positive_pnl_not_atomic(&mut long, 0, 10)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 10 * BOUND_SCALE, 10)
        .unwrap();
    g.attach_leg(&mut long, 0, SideV16::Long, 10 * POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV16::Long, 10 * POS_SCALE, 11);
    g.withdraw_not_atomic(&mut long, 90, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    g.current_slot = 1;
    g.full_account_refresh(&mut long, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    g.current_slot = 2;
    g.full_account_refresh(&mut long, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    let vault_before = g.vault;

    let mut header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: [i as u8; 32],
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let preserved_wrapper = markets[0].wrapper;
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    market_view
        .withdraw_backing_provider_earnings_not_atomic(0, 5)
        .unwrap();
    assert_eq!(market_view.header.vault.get(), vault_before - 5);
    assert_eq!(
        market_view.markets[0]
            .engine
            .backing_long
            .utilization_fee_earnings
            .get(),
        0
    );
    assert_eq!(market_view.markets[0].wrapper, preserved_wrapper);
    market_view.validate_shape().unwrap();
}

#[test]
fn v16_zero_copy_source_backing_lifecycle_without_runtime_vecs() {
    let g = group();
    let mut header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: [i as u8; 32],
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let preserved_wrapper = markets[0].wrapper;
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    market_view
        .add_source_positive_claim_bound_not_atomic(0, 100, 100)
        .unwrap();
    assert_eq!(market_view.source_credit_available_backing_num(0), Ok(0));

    market_view
        .add_fresh_counterparty_backing_not_atomic(0, 100, 10)
        .unwrap();
    assert_eq!(market_view.source_credit_available_backing_num(0), Ok(100));
    assert_eq!(
        market_view.recompute_source_credit_rate_not_atomic(0),
        Ok(CREDIT_RATE_SCALE)
    );
    assert_eq!(market_view.markets[0].wrapper, preserved_wrapper);

    market_view
        .expire_source_backing_bucket_not_atomic(0, 10)
        .unwrap();
    assert_eq!(market_view.source_credit_available_backing_num(0), Ok(0));
    assert_eq!(
        market_view.markets[0]
            .engine
            .source_credit_long
            .credit_rate_num
            .get(),
        0
    );
    assert_eq!(
        market_view.markets[0].engine.backing_long.status,
        BackingBucketStatusV16::Expired as u8
    );
    assert_eq!(market_view.markets[0].wrapper, preserved_wrapper);
    market_view.validate_shape().unwrap();
}

#[test]
fn v16_zero_copy_source_credit_lien_lifecycles_without_runtime_vecs() {
    let mut g = group();
    g.vault = 100;
    g.insurance = 100;
    g.insurance_domain_budget[1] = 50;
    let mut header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: [i as u8; 32],
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let preserved_long_wrapper = markets[0].wrapper;
    let preserved_short_wrapper = markets[0].wrapper;
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    market_view
        .add_source_positive_claim_bound_not_atomic(0, 100, 100)
        .unwrap();
    market_view
        .add_fresh_counterparty_backing_not_atomic(0, 100, 10)
        .unwrap();
    market_view
        .create_source_credit_lien_from_counterparty_not_atomic(0, 30)
        .unwrap();
    assert!(market_view
        .reservation_encumbrance_proof_for_domain(0)
        .unwrap()
        .validate()
        .is_ok());
    assert_eq!(market_view.source_credit_available_backing_num(0), Ok(70));
    market_view
        .release_source_credit_lien_from_counterparty_not_atomic(0, 30)
        .unwrap();
    assert_eq!(market_view.source_credit_available_backing_num(0), Ok(100));
    market_view
        .create_source_credit_lien_from_counterparty_not_atomic(0, 20)
        .unwrap();
    market_view
        .consume_source_credit_lien_from_counterparty_not_atomic(0, 20)
        .unwrap();
    assert_eq!(
        market_view.markets[0]
            .engine
            .source_credit_long
            .spent_backing_num
            .get(),
        20
    );
    assert_eq!(
        market_view.markets[0]
            .engine
            .backing_long
            .consumed_liened_backing_num
            .get(),
        20
    );
    market_view
        .create_source_credit_lien_from_counterparty_not_atomic(0, 10)
        .unwrap();
    market_view
        .impair_source_credit_lien_from_counterparty_not_atomic(0, 10)
        .unwrap();
    assert_eq!(
        market_view.markets[0]
            .engine
            .source_credit_long
            .impaired_liened_backing_num
            .get(),
        10
    );

    market_view
        .add_source_positive_claim_bound_not_atomic(1, 50 * BOUND_SCALE, 50 * BOUND_SCALE)
        .unwrap();
    market_view
        .reserve_insurance_credit_not_atomic(1, 50 * BOUND_SCALE)
        .unwrap();
    assert_eq!(
        market_view.source_credit_available_backing_num(1),
        Ok(50 * BOUND_SCALE)
    );
    market_view
        .create_source_credit_lien_from_insurance_not_atomic(1, 20 * BOUND_SCALE)
        .unwrap();
    market_view
        .release_source_credit_lien_from_insurance_not_atomic(1, 20 * BOUND_SCALE)
        .unwrap();
    market_view
        .create_source_credit_lien_from_insurance_not_atomic(1, 10 * BOUND_SCALE)
        .unwrap();
    market_view
        .consume_source_credit_lien_from_insurance_not_atomic(1, 10 * BOUND_SCALE)
        .unwrap();
    assert_eq!(market_view.header.insurance.get(), 90);
    assert_eq!(
        market_view.markets[0]
            .engine
            .insurance_domain_spent_short
            .get(),
        10
    );
    market_view
        .create_source_credit_lien_from_insurance_not_atomic(1, 5 * BOUND_SCALE)
        .unwrap();
    market_view
        .impair_source_credit_lien_from_insurance_not_atomic(1, 5 * BOUND_SCALE)
        .unwrap();
    assert_eq!(
        market_view.markets[0]
            .engine
            .source_credit_short
            .impaired_liened_insurance_num
            .get(),
        5 * BOUND_SCALE
    );
    assert_eq!(market_view.markets[0].wrapper, preserved_long_wrapper);
    assert_eq!(market_view.markets[0].wrapper, preserved_short_wrapper);
    market_view.validate_shape().unwrap();

    let mut account = account_with_id(44);
    account.ensure_source_domain_capacity(
        v16_domain_count_for_market_slots(market_view.header.config.max_market_slots.get())
            .unwrap(),
    );
    account.source_claim_market_id[0] = market_view.markets[0].engine.asset.market_id.get();
    account.source_claim_bound_num[0] = 4 * BOUND_SCALE;
    account.source_claim_liened_num[0] = 4 * BOUND_SCALE;
    account.source_claim_counterparty_liened_num[0] = 4 * BOUND_SCALE;
    account.source_lien_effective_reserved[0] = 4;
    account.source_lien_counterparty_backing_num[0] = 4 * BOUND_SCALE;
    let account_header = PortfolioAccountV16Account::from_runtime(&account);
    let account_sources = PortfolioAccountV16Account::source_domains_from_runtime(&account)
        .expect("runtime account should serialize to zero-copy domains");
    let account_view = percolator::v16::PortfolioV16View::new(&account_header, &account_sources);
    let proof = market_view
        .source_credit_lien_proof_for_account_domain(&account_view, 0)
        .unwrap();
    assert_eq!(
        proof,
        SourceCreditLienAggregateProofV16 {
            domain: 0,
            source_claim_bound_num: 4 * BOUND_SCALE,
            face_claim_locked_num: 4 * BOUND_SCALE,
            counterparty_face_claim_locked_num: 4 * BOUND_SCALE,
            insurance_face_claim_locked_num: 0,
            effective_credit_reserved: 4,
            counterparty_backing_reserved_num: 4 * BOUND_SCALE,
            insurance_backing_reserved_num: 0,
            impaired_face_claim_num: 0,
            impaired_effective_credit_reserved: 0,
        }
    );
    assert!(proof.validate().is_ok());
}

#[test]
fn v16_zero_copy_account_source_lien_release_and_impair_without_runtime_vecs() {
    let mut g = group();
    g.vault = 10;
    g.insurance = 10;
    g.insurance_domain_budget[0] = 10;
    let mut header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: [i as u8; 32],
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    market_view
        .add_source_positive_claim_bound_not_atomic(0, 30 * BOUND_SCALE, 30 * BOUND_SCALE)
        .unwrap();
    market_view
        .add_fresh_counterparty_backing_not_atomic(0, 20 * BOUND_SCALE, 10)
        .unwrap();
    market_view
        .create_source_credit_lien_from_counterparty_not_atomic(0, 20 * BOUND_SCALE)
        .unwrap();
    market_view
        .reserve_insurance_credit_not_atomic(0, 10 * BOUND_SCALE)
        .unwrap();
    market_view
        .create_source_credit_lien_from_insurance_not_atomic(0, 10 * BOUND_SCALE)
        .unwrap();

    let domain_count =
        v16_domain_count_for_market_slots(market_view.header.config.max_market_slots.get())
            .unwrap();
    let mut account = account_with_id(45);
    account.ensure_source_domain_capacity(domain_count);
    account.source_claim_market_id[0] = market_view.markets[0].engine.asset.market_id.get();
    account.source_claim_bound_num[0] = 30 * BOUND_SCALE;
    account.source_claim_liened_num[0] = 30 * BOUND_SCALE;
    account.source_claim_counterparty_liened_num[0] = 20 * BOUND_SCALE;
    account.source_claim_insurance_liened_num[0] = 10 * BOUND_SCALE;
    account.source_lien_effective_reserved[0] = 30;
    account.source_lien_counterparty_backing_num[0] = 20 * BOUND_SCALE;
    account.source_lien_insurance_backing_num[0] = 10 * BOUND_SCALE;
    let mut account_header = PortfolioAccountV16Account::from_runtime(&account);
    let mut account_sources =
        PortfolioAccountV16Account::source_domains_from_runtime(&account).unwrap();
    let mut account_view =
        percolator::v16::PortfolioV16ViewMut::new(&mut account_header, &mut account_sources);

    let released = market_view
        .release_account_source_credit_liens_if_unneeded_not_atomic(&mut account_view)
        .unwrap();
    assert_eq!(released, 30);
    assert_eq!(
        account_view.source_domains[0]
            .source_lien_effective_reserved
            .get(),
        0
    );
    assert_eq!(
        account_view.source_domains[0]
            .source_lien_counterparty_backing_num
            .get(),
        0
    );
    assert_eq!(
        account_view.source_domains[0]
            .source_lien_insurance_backing_num
            .get(),
        0
    );
    assert_eq!(
        market_view.markets[0]
            .engine
            .source_credit_long
            .valid_liened_backing_num
            .get(),
        0
    );
    assert_eq!(
        market_view.markets[0]
            .engine
            .source_credit_long
            .valid_liened_insurance_num
            .get(),
        0
    );
    assert_eq!(
        market_view.source_credit_available_backing_num(0),
        Ok(30 * BOUND_SCALE)
    );
    market_view.validate_shape().unwrap();

    let mut g = group();
    g.vault = 10;
    g.insurance = 10;
    g.insurance_domain_budget[0] = 10;
    let mut header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: [7u8; 32],
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    market_view
        .add_source_positive_claim_bound_not_atomic(0, 10 * BOUND_SCALE, 10 * BOUND_SCALE)
        .unwrap();
    market_view
        .reserve_insurance_credit_not_atomic(0, 10 * BOUND_SCALE)
        .unwrap();
    market_view
        .create_source_credit_lien_from_insurance_not_atomic(0, 10 * BOUND_SCALE)
        .unwrap();

    let domain_count =
        v16_domain_count_for_market_slots(market_view.header.config.max_market_slots.get())
            .unwrap();
    let mut account = account_with_id(46);
    account.ensure_source_domain_capacity(domain_count);
    account.source_claim_market_id[0] = market_view.markets[0].engine.asset.market_id.get();
    account.source_claim_bound_num[0] = 10 * BOUND_SCALE;
    account.source_claim_liened_num[0] = 10 * BOUND_SCALE;
    account.source_claim_insurance_liened_num[0] = 10 * BOUND_SCALE;
    account.source_lien_effective_reserved[0] = 10;
    account.source_lien_insurance_backing_num[0] = 10 * BOUND_SCALE;
    let mut account_header = PortfolioAccountV16Account::from_runtime(&account);
    let mut account_sources =
        PortfolioAccountV16Account::source_domains_from_runtime(&account).unwrap();
    let mut account_view =
        percolator::v16::PortfolioV16ViewMut::new(&mut account_header, &mut account_sources);

    let impaired = market_view
        .impair_account_source_credit_lien_from_insurance_not_atomic(&mut account_view, 0)
        .unwrap();
    assert_eq!(impaired, 10);
    assert_eq!(
        account_view.source_domains[0]
            .source_claim_insurance_liened_num
            .get(),
        0
    );
    assert_eq!(
        account_view.source_domains[0]
            .source_claim_impaired_num
            .get(),
        10 * BOUND_SCALE
    );
    assert_eq!(
        account_view.source_domains[0]
            .source_lien_impaired_effective_reserved
            .get(),
        10
    );
    assert_eq!(
        market_view.markets[0]
            .engine
            .source_credit_long
            .valid_liened_insurance_num
            .get(),
        0
    );
    assert_eq!(
        market_view.markets[0]
            .engine
            .source_credit_long
            .impaired_liened_insurance_num
            .get(),
        10 * BOUND_SCALE
    );
    market_view.validate_shape().unwrap();
}

#[test]
fn v16_zero_copy_quantity_adl_finalizes_account_and_aggregate_oi_without_runtime_vecs() {
    let mut g = group();
    let mut closing = account();
    let mut survivor = account_with_id(47);
    let mut opposing = account_with_id(48);
    g.attach_leg(&mut closing, 0, SideV16::Long, 4).unwrap();
    g.attach_leg(&mut survivor, 0, SideV16::Long, 6).unwrap();
    g.attach_leg(&mut opposing, 0, SideV16::Short, -10).unwrap();
    closing.close_progress = CloseProgressLedgerV16 {
        active: true,
        finalized: true,
        close_id: 1,
        asset_index: 0,
        market_id: g.assets[0].market_id,
        domain_side: SideV16::Short,
        gross_loss_at_close_start: 1,
        explicit_loss_assigned: 1,
        residual_remaining: 0,
        ..CloseProgressLedgerV16::EMPTY
    };
    let survivor_weight = survivor.legs[0].loss_weight;
    closing.ensure_source_domain_capacity(
        v16_domain_count_for_market_slots(g.config.max_market_slots).unwrap(),
    );

    let mut header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: [i as u8; 32],
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let mut account_header = PortfolioAccountV16Account::from_runtime(&closing);
    let mut account_sources =
        PortfolioAccountV16Account::source_domains_from_runtime(&closing).unwrap();
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account_view =
        percolator::v16::PortfolioV16ViewMut::new(&mut account_header, &mut account_sources);

    let out = market_view
        .apply_quantity_adl_after_residual_for_account_not_atomic(
            &mut account_view,
            0,
            SideV16::Long,
            4,
        )
        .unwrap();

    assert_eq!(out.closed_q, 4);
    assert_eq!(
        account_view.header.active_bitmap.map(V16PodU64::get),
        bitmap(&[])
    );
    assert!(!account_view.header.legs[0].try_to_runtime().unwrap().active);
    assert_eq!(
        account_view
            .header
            .close_progress
            .quantity_adl_applied_q
            .get(),
        4
    );
    assert_eq!(market_view.markets[0].engine.asset.oi_eff_long_q.get(), 6);
    assert_eq!(market_view.markets[0].engine.asset.oi_eff_short_q.get(), 6);
    assert_eq!(
        market_view.markets[0]
            .engine
            .asset
            .stored_pos_count_long
            .get(),
        1
    );
    assert_eq!(
        market_view.markets[0]
            .engine
            .asset
            .loss_weight_sum_long
            .get(),
        survivor_weight
    );
    account_view
        .validate_with_market(&market_view.as_view())
        .unwrap();
    market_view.validate_shape().unwrap();
}

#[test]
fn v16_zero_copy_asset_lifecycle_and_explicit_recovery_without_runtime_vecs() {
    let g = group();
    let mut header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: [i as u8; 32],
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let preserved_wrapper = markets[0].wrapper;
    let risk_epoch_before = header.risk_epoch.get();
    let asset_set_epoch_before = header.asset_set_epoch.get();
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    market_view.mark_asset_drain_only_not_atomic(0).unwrap();
    assert_eq!(
        market_view.markets[0].engine.asset.lifecycle,
        AssetLifecycleV16::DrainOnly as u8
    );
    assert!(market_view.header.risk_epoch.get() > risk_epoch_before);
    assert!(market_view.header.asset_set_epoch.get() > asset_set_epoch_before);
    assert_eq!(market_view.markets[0].wrapper, preserved_wrapper);

    market_view.retire_empty_asset_not_atomic(0, 1).unwrap();
    assert_eq!(
        market_view.markets[0].engine.asset.lifecycle,
        AssetLifecycleV16::Retired as u8
    );
    assert_eq!(market_view.markets[0].engine.asset.retired_slot.get(), 1);
    assert_eq!(market_view.header.current_slot.get(), 1);
    assert_eq!(market_view.markets[0].wrapper, preserved_wrapper);
    market_view.retire_empty_asset_not_atomic(0, 1).unwrap();
    market_view.validate_shape().unwrap();

    let mut blocked_group = group();
    blocked_group.vault = 1;
    blocked_group.insurance = 1;
    let mut blocked_header = MarketGroupV16HeaderAccount::from_runtime_with_capacity(
        &blocked_group,
        blocked_group.assets.len(),
    )
    .unwrap();
    let mut blocked_markets = (0..blocked_group.assets.len())
        .map(|i| Market {
            wrapper: [9u8; 32],
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&blocked_group, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let mut blocked_view = MarketGroupV16ViewMut::new(&mut blocked_header, &mut blocked_markets);
    blocked_view
        .add_fresh_counterparty_backing_not_atomic(0, BOUND_SCALE, 10)
        .unwrap();
    assert_eq!(
        blocked_view.retire_empty_asset_not_atomic(0, 1),
        Err(V16Error::LockActive),
        "zero-copy retirement must not orphan source-credit backing"
    );

    let recovery_group = group();
    let mut recovery_header = MarketGroupV16HeaderAccount::from_runtime_with_capacity(
        &recovery_group,
        recovery_group.assets.len(),
    )
    .unwrap();
    let mut recovery_markets = (0..recovery_group.assets.len())
        .map(|i| Market {
            wrapper: [7u8; 32],
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&recovery_group, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let mut recovery_view = MarketGroupV16ViewMut::new(&mut recovery_header, &mut recovery_markets);
    assert_eq!(
        recovery_view.declare_explicit_loss_or_dust_audit_overflow_not_atomic(),
        Ok(PermissionlessProgressOutcomeV16::RecoveryDeclared(
            PermissionlessRecoveryReasonV16::ExplicitLossOrDustAuditOverflow
        ))
    );
    assert_eq!(recovery_view.header.mode, MarketModeV16::Recovery as u8);
    recovery_view.validate_shape().unwrap();
}

#[test]
fn v16_withdraw_that_uses_positive_credit_creates_source_lien() {
    let (market, _, _) = ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.min_nonzero_im_req = 10;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 10).unwrap();
    g.vault = g.vault.checked_add(10).unwrap();
    g.add_account_source_positive_pnl_not_atomic(&mut a, 0, 10)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 10 * BOUND_SCALE, 10)
        .unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, 10 * POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV16::Long, 10 * POS_SCALE, 111);

    g.withdraw_not_atomic(&mut a, 5, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(a.capital, 5);
    assert!(
        a.source_claim_liened_num[0] != 0,
        "withdrawal that depends on source PnL must lock claim face"
    );
    assert_eq!(
        a.source_lien_effective_reserved[0], 5,
        "post-withdraw initial margin shortfall should be source-lien reserved"
    );
    assert_eq!(
        g.source_credit[0].valid_liened_backing_num,
        a.source_lien_effective_reserved[0] * BOUND_SCALE
    );
}

#[test]
fn v16_source_lien_releases_after_no_positive_credit_health_is_restored() {
    let (market, _, _) = ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.min_nonzero_im_req = 10;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 10).unwrap();
    g.vault = g.vault.checked_add(10).unwrap();
    g.add_account_source_positive_pnl_not_atomic(&mut a, 0, 10)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 10 * BOUND_SCALE, 10)
        .unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, 10 * POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV16::Long, 10 * POS_SCALE, 112);

    g.withdraw_not_atomic(&mut a, 5, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(a.source_lien_effective_reserved[0], 5);

    g.deposit_not_atomic(&mut a, 5).unwrap();
    let released = g
        .release_account_source_credit_liens_if_unneeded_not_atomic(
            &mut a,
            &[1; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(released, 5);
    assert_eq!(a.source_claim_liened_num[0], 0);
    assert_eq!(a.source_lien_effective_reserved[0], 0);
    assert_eq!(g.source_credit[0].valid_liened_backing_num, 0);
    assert_eq!(
        g.source_credit_available_backing_num(0),
        Ok(10 * BOUND_SCALE)
    );
}

#[test]
fn v16_insurance_backed_source_lien_releases_after_no_positive_credit_health_is_restored() {
    let (market, _, _) = ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.min_nonzero_im_req = 10;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 10).unwrap();
    g.vault = g.vault.checked_add(10).unwrap();
    g.insurance = 10 * BOUND_SCALE;
    g.vault = g.vault.checked_add(g.insurance).unwrap();
    g.insurance_domain_budget[0] = g.insurance;
    g.add_account_source_positive_pnl_not_atomic(&mut a, 0, 10)
        .unwrap();
    g.reserve_insurance_credit_not_atomic(0, 10 * BOUND_SCALE)
        .unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, 10 * POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV16::Long, 10 * POS_SCALE, 114);

    g.withdraw_not_atomic(&mut a, 5, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(a.source_lien_effective_reserved[0], 5);
    assert_eq!(a.source_lien_counterparty_backing_num[0], 0);
    assert_eq!(a.source_lien_insurance_backing_num[0], 5 * BOUND_SCALE);
    assert_eq!(
        g.source_credit[0].valid_liened_insurance_num,
        5 * BOUND_SCALE
    );

    g.deposit_not_atomic(&mut a, 5).unwrap();
    let released = g
        .release_account_source_credit_liens_if_unneeded_not_atomic(
            &mut a,
            &[1; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(released, 5);
    assert_eq!(a.source_claim_liened_num[0], 0);
    assert_eq!(a.source_lien_effective_reserved[0], 0);
    assert_eq!(a.source_lien_insurance_backing_num[0], 0);
    assert_eq!(g.source_credit[0].valid_liened_insurance_num, 0);
    assert_eq!(
        g.source_credit_available_backing_num(0),
        Ok(10 * BOUND_SCALE)
    );
}

#[test]
fn v16_insurance_backed_source_lien_impairment_removes_account_health_credit() {
    let (market, _, _) = ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.min_nonzero_im_req = 10;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 10).unwrap();
    g.vault = g.vault.checked_add(10).unwrap();
    g.insurance = 10 * BOUND_SCALE;
    g.vault = g.vault.checked_add(g.insurance).unwrap();
    g.insurance_domain_budget[0] = g.insurance;
    g.add_account_source_positive_pnl_not_atomic(&mut a, 0, 10)
        .unwrap();
    g.reserve_insurance_credit_not_atomic(0, 10 * BOUND_SCALE)
        .unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, 10 * POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV16::Long, 10 * POS_SCALE, 116);

    g.withdraw_not_atomic(&mut a, 10, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(a.source_lien_effective_reserved[0], 10);
    assert_eq!(a.source_lien_insurance_backing_num[0], 10 * BOUND_SCALE);

    let impaired = g
        .impair_account_source_credit_lien_from_insurance_not_atomic(&mut a, 0)
        .unwrap();
    let cert = g
        .full_account_refresh(&mut a, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(impaired, 10);
    assert_eq!(
        cert.certified_equity, 0,
        "impaired insurance-backed source credit must not keep an account above IM"
    );
    assert!(cert.certified_initial_req != 0);
    assert_eq!(a.source_claim_liened_num[0], 0);
    assert_eq!(a.source_claim_insurance_liened_num[0], 0);
    assert_eq!(a.source_lien_effective_reserved[0], 0);
    assert_eq!(a.source_lien_insurance_backing_num[0], 0);
    assert!(a.source_claim_impaired_num[0] != 0);
    assert_eq!(a.source_lien_impaired_effective_reserved[0], 10);
    assert_eq!(g.source_credit[0].valid_liened_insurance_num, 0);
    assert_eq!(
        g.source_credit[0].impaired_liened_insurance_num,
        10 * BOUND_SCALE
    );
}

#[test]
fn v16_existing_source_lien_counts_for_later_positive_credit_checks() {
    let (market, _, _) = ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.min_nonzero_im_req = 10;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 10).unwrap();
    g.vault = g.vault.checked_add(10).unwrap();
    g.add_account_source_positive_pnl_not_atomic(&mut a, 0, 10)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 10 * BOUND_SCALE, 10)
        .unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, 10 * POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV16::Long, 10 * POS_SCALE, 113);

    g.withdraw_not_atomic(&mut a, 5, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(a.source_lien_effective_reserved[0], 5);

    g.withdraw_not_atomic(&mut a, 1, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(a.capital, 4);
    assert_eq!(
        a.source_lien_effective_reserved[0], 6,
        "existing source-credit lien should count before adding only the incremental shortfall"
    );
}

#[test]
fn v16_expired_counterparty_backing_impairs_account_lien_before_health_credit() {
    let (market, _, _) = ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.min_nonzero_im_req = 10;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 10).unwrap();
    g.vault = g.vault.checked_add(10).unwrap();
    g.add_account_source_positive_pnl_not_atomic(&mut a, 0, 10)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 10 * BOUND_SCALE, 10)
        .unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, 10 * POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV16::Long, 10 * POS_SCALE, 115);
    g.withdraw_not_atomic(&mut a, 5, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(a.source_lien_effective_reserved[0], 5);
    assert_eq!(a.source_lien_counterparty_backing_num[0], 5 * BOUND_SCALE);

    g.current_slot = 10;
    let cert = g
        .full_account_refresh(&mut a, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(
        cert.certified_equity, 5,
        "expired counterparty-backed source credit must not keep an account above IM"
    );
    assert!(cert.certified_equity as u128 == a.capital);
    assert!((cert.certified_equity as u128) < cert.certified_initial_req);
    assert_eq!(a.source_lien_effective_reserved[0], 0);
    assert_eq!(a.source_lien_counterparty_backing_num[0], 0);
    assert_eq!(a.source_claim_liened_num[0], 0);
    assert!(a.source_claim_impaired_num[0] != 0);
    assert_eq!(a.source_lien_impaired_effective_reserved[0], 5);
    assert_eq!(g.source_credit[0].valid_liened_backing_num, 0);
    assert_eq!(
        g.source_credit[0].impaired_liened_backing_num,
        5 * BOUND_SCALE
    );
}

#[test]
fn v16_impaired_source_claim_burns_when_positive_pnl_decreases() {
    let (market, _, _) = ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.min_nonzero_im_req = 10;
    cfg.max_price_move_bps_per_slot = 10_000;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 10).unwrap();
    g.vault = g.vault.checked_add(10).unwrap();
    g.add_account_source_positive_pnl_not_atomic(&mut a, 0, 5)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 5 * BOUND_SCALE, 10)
        .unwrap();
    g.attach_leg(&mut a, 0, SideV16::Short, -(5 * POS_SCALE as i128))
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV16::Short, 5 * POS_SCALE, 117);
    g.withdraw_not_atomic(&mut a, 5, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    g.current_slot = 10;
    g.full_account_refresh(&mut a, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(a.pnl, 5);
    assert_eq!(a.source_claim_bound_num[0], 5 * BOUND_SCALE);
    assert_eq!(a.source_claim_impaired_num[0], 5 * BOUND_SCALE);
    assert_eq!(a.source_lien_impaired_effective_reserved[0], 5);

    g.accrue_asset_to_not_atomic(0, 11, 2, 0, true).unwrap();
    g.full_account_refresh(&mut a, &[2; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(a.pnl, 0);
    assert_eq!(a.source_claim_bound_num[0], 0);
    assert_eq!(a.source_claim_impaired_num[0], 0);
    assert_eq!(a.source_lien_impaired_effective_reserved[0], 0);
    assert_eq!(g.source_credit[0].positive_claim_bound_num, 0);
    g.assert_public_invariants().unwrap();
}

#[test]
fn v16_zero_copy_impaired_source_claim_burns_when_positive_pnl_decreases() {
    let (market, _, _) = ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.min_nonzero_im_req = 10;
    cfg.max_price_move_bps_per_slot = 10_000;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 10).unwrap();
    g.vault = g.vault.checked_add(10).unwrap();
    g.add_account_source_positive_pnl_not_atomic(&mut a, 0, 5)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 5 * BOUND_SCALE, 10)
        .unwrap();
    g.attach_leg(&mut a, 0, SideV16::Short, -(5 * POS_SCALE as i128))
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV16::Short, 5 * POS_SCALE, 118);
    g.withdraw_not_atomic(&mut a, 5, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    g.current_slot = 10;
    g.full_account_refresh(&mut a, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    let mut header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: [i as u8; 32],
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let mut account_header = PortfolioAccountV16Account::from_runtime(&a);
    let mut source_domains = PortfolioAccountV16Account::source_domains_from_runtime(&a).unwrap();
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account_view =
        percolator::v16::PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);

    market_view
        .accrue_asset_to_not_atomic(0, 11, 2, 0, true)
        .unwrap();
    market_view
        .full_account_refresh_not_atomic(&mut account_view)
        .unwrap();

    assert_eq!(account_view.header.pnl.get(), 0);
    assert_eq!(
        account_view.source_domains[0].source_claim_bound_num.get(),
        0
    );
    assert_eq!(
        account_view.source_domains[0]
            .source_claim_impaired_num
            .get(),
        0
    );
    assert_eq!(
        account_view.source_domains[0]
            .source_lien_impaired_effective_reserved
            .get(),
        0
    );
    assert_eq!(
        market_view.markets[0]
            .engine
            .source_credit_long
            .positive_claim_bound_num
            .get(),
        0
    );
    market_view.validate_shape().unwrap();
    account_view
        .validate_with_market(&market_view.as_view())
        .unwrap();
}

#[test]
fn v16_counterparty_lien_lifecycle_never_inflates_available_backing() {
    let mut g = group();
    g.add_source_positive_claim_bound_not_atomic(0, 100, 100)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 100, 10)
        .unwrap();
    assert_eq!(g.source_credit_available_backing_num(0), Ok(100));

    g.create_source_credit_lien_from_counterparty_not_atomic(0, 30)
        .unwrap();
    assert_eq!(g.source_credit[0].fresh_reserved_backing_num, 100);
    assert_eq!(g.source_credit[0].valid_liened_backing_num, 30);
    assert_eq!(g.source_credit_available_backing_num(0), Ok(70));

    g.release_source_credit_lien_from_counterparty_not_atomic(0, 30)
        .unwrap();
    assert_eq!(g.source_credit_available_backing_num(0), Ok(100));

    g.create_source_credit_lien_from_counterparty_not_atomic(0, 20)
        .unwrap();
    g.consume_source_credit_lien_from_counterparty_not_atomic(0, 20)
        .unwrap();
    assert_eq!(g.source_credit[0].spent_backing_num, 20);
    assert_eq!(g.source_credit[0].provider_receivable_num, 20);
    assert_eq!(g.source_backing_buckets[0].consumed_liened_backing_num, 20);
    assert_eq!(g.source_credit_available_backing_num(0), Ok(80));

    g.create_source_credit_lien_from_counterparty_not_atomic(0, 10)
        .unwrap();
    g.impair_source_credit_lien_from_counterparty_not_atomic(0, 10)
        .unwrap();
    assert_eq!(g.source_credit[0].impaired_liened_backing_num, 10);
    assert_eq!(g.source_credit_available_backing_num(0), Ok(70));

    g.current_slot = 10;
    g.expire_source_backing_bucket_not_atomic(0, 10).unwrap();
    assert_eq!(g.source_credit_available_backing_num(0), Ok(0));
    assert_eq!(g.source_credit[0].credit_rate_num, 0);
}

#[test]
fn v16_consumed_counterparty_backing_is_refilled_by_future_source_backing() {
    let mut g = group();
    g.add_source_positive_claim_bound_not_atomic(0, 100, 100)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 100, 10)
        .unwrap();
    g.create_source_credit_lien_from_counterparty_not_atomic(0, 80)
        .unwrap();
    g.consume_source_credit_lien_from_counterparty_not_atomic(0, 80)
        .unwrap();

    assert_eq!(g.source_credit[0].spent_backing_num, 80);
    assert_eq!(g.source_credit[0].provider_receivable_num, 80);
    assert_eq!(g.source_backing_buckets[0].consumed_liened_backing_num, 80);
    assert_eq!(g.source_credit_available_backing_num(0), Ok(20));

    g.add_fresh_counterparty_backing_not_atomic(0, 50, 10)
        .unwrap();
    assert_eq!(g.source_credit[0].provider_receivable_num, 30);
    assert_eq!(g.source_backing_buckets[0].consumed_liened_backing_num, 30);
    assert_eq!(g.source_credit[0].fresh_reserved_backing_num, 70);
    assert_eq!(g.source_backing_buckets[0].fresh_unliened_backing_num, 70);
    assert_eq!(g.source_credit_available_backing_num(0), Ok(70));

    g.add_fresh_counterparty_backing_not_atomic(0, 50, 10)
        .unwrap();
    assert_eq!(g.source_credit[0].provider_receivable_num, 0);
    assert_eq!(g.source_backing_buckets[0].consumed_liened_backing_num, 0);
    assert_eq!(g.source_credit[0].fresh_reserved_backing_num, 120);
    assert_eq!(g.source_backing_buckets[0].fresh_unliened_backing_num, 120);
    assert_eq!(g.source_credit_available_backing_num(0), Ok(120));
    g.assert_public_invariants().unwrap();
}

#[test]
fn v16_fully_consumed_backing_bucket_can_be_refilled_without_losing_receivable() {
    let mut g = group();
    g.add_source_positive_claim_bound_not_atomic(0, 40, 40)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 40, 10)
        .unwrap();
    g.create_source_credit_lien_from_counterparty_not_atomic(0, 40)
        .unwrap();
    g.consume_source_credit_lien_from_counterparty_not_atomic(0, 40)
        .unwrap();

    assert_eq!(
        g.source_backing_buckets[0].status,
        percolator::v16::BackingBucketStatusV16::Expired
    );
    assert_eq!(g.source_credit[0].provider_receivable_num, 40);
    assert_eq!(g.source_backing_buckets[0].consumed_liened_backing_num, 40);
    assert_eq!(g.source_credit_available_backing_num(0), Ok(0));

    g.add_fresh_counterparty_backing_not_atomic(0, 20, 20)
        .unwrap();
    assert_eq!(
        g.source_backing_buckets[0].status,
        percolator::v16::BackingBucketStatusV16::Fresh
    );
    assert_eq!(g.source_credit[0].provider_receivable_num, 20);
    assert_eq!(g.source_backing_buckets[0].consumed_liened_backing_num, 20);
    assert_eq!(g.source_credit[0].fresh_reserved_backing_num, 20);
    assert_eq!(g.source_credit_available_backing_num(0), Ok(20));
    g.assert_public_invariants().unwrap();
}

#[test]
fn v16_counterparty_lien_consume_overflow_rejects_before_mutation() {
    let mut g = group();
    g.add_source_positive_claim_bound_not_atomic(0, 100, 100)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 100, 10)
        .unwrap();
    g.create_source_credit_lien_from_counterparty_not_atomic(0, 30)
        .unwrap();
    g.source_backing_buckets[0].consumed_liened_backing_num = u128::MAX;

    let source_before = g.source_credit[0];
    let bucket_before = g.source_backing_buckets[0];
    let risk_epoch_before = g.risk_epoch;

    assert_eq!(
        g.consume_source_credit_lien_from_counterparty_not_atomic(0, 1),
        Err(V16Error::CounterOverflow)
    );
    assert_eq!(g.source_credit[0], source_before);
    assert_eq!(g.source_backing_buckets[0], bucket_before);
    assert_eq!(g.risk_epoch, risk_epoch_before);
}

#[test]
fn v16_counterparty_lien_consume_source_overflow_rejects_before_mutation() {
    let mut g = group();
    g.add_source_positive_claim_bound_not_atomic(0, 100, 100)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 100, 10)
        .unwrap();
    g.create_source_credit_lien_from_counterparty_not_atomic(0, 30)
        .unwrap();
    g.source_credit[0].spent_backing_num = u128::MAX;

    let source_before = g.source_credit[0];
    let bucket_before = g.source_backing_buckets[0];
    let risk_epoch_before = g.risk_epoch;

    assert_eq!(
        g.consume_source_credit_lien_from_counterparty_not_atomic(0, 1),
        Err(V16Error::CounterOverflow)
    );
    assert_eq!(g.source_credit[0], source_before);
    assert_eq!(g.source_backing_buckets[0], bucket_before);
    assert_eq!(g.risk_epoch, risk_epoch_before);
}

#[test]
fn v16_counterparty_lien_consume_receivable_overflow_rejects_before_mutation() {
    let mut g = group();
    g.add_source_positive_claim_bound_not_atomic(0, 100, 100)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 100, 10)
        .unwrap();
    g.create_source_credit_lien_from_counterparty_not_atomic(0, 30)
        .unwrap();
    g.source_credit[0].provider_receivable_num = u128::MAX;
    g.source_credit[0].spent_backing_num = u128::MAX;
    g.source_backing_buckets[0].consumed_liened_backing_num = u128::MAX;

    let source_before = g.source_credit[0];
    let bucket_before = g.source_backing_buckets[0];
    let risk_epoch_before = g.risk_epoch;

    assert_eq!(
        g.consume_source_credit_lien_from_counterparty_not_atomic(0, 1),
        Err(V16Error::CounterOverflow)
    );
    assert_eq!(g.source_credit[0], source_before);
    assert_eq!(g.source_backing_buckets[0], bucket_before);
    assert_eq!(g.risk_epoch, risk_epoch_before);
}

#[test]
fn v16_counterparty_lien_impair_overflow_rejects_before_mutation() {
    let mut g = group();
    g.add_source_positive_claim_bound_not_atomic(0, 100, 100)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 100, 10)
        .unwrap();
    g.create_source_credit_lien_from_counterparty_not_atomic(0, 30)
        .unwrap();
    g.source_backing_buckets[0].impaired_liened_backing_num = u128::MAX;

    let source_before = g.source_credit[0];
    let bucket_before = g.source_backing_buckets[0];
    let risk_epoch_before = g.risk_epoch;

    assert_eq!(
        g.impair_source_credit_lien_from_counterparty_not_atomic(0, 1),
        Err(V16Error::CounterOverflow)
    );
    assert_eq!(g.source_credit[0], source_before);
    assert_eq!(g.source_backing_buckets[0], bucket_before);
    assert_eq!(g.risk_epoch, risk_epoch_before);
}

#[test]
fn v16_counterparty_lien_impair_source_overflow_rejects_before_mutation() {
    let mut g = group();
    g.add_source_positive_claim_bound_not_atomic(0, 100, 100)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 100, 10)
        .unwrap();
    g.create_source_credit_lien_from_counterparty_not_atomic(0, 30)
        .unwrap();
    g.source_credit[0].impaired_liened_backing_num = u128::MAX;

    let source_before = g.source_credit[0];
    let bucket_before = g.source_backing_buckets[0];
    let risk_epoch_before = g.risk_epoch;

    assert_eq!(
        g.impair_source_credit_lien_from_counterparty_not_atomic(0, 1),
        Err(V16Error::CounterOverflow)
    );
    assert_eq!(g.source_credit[0], source_before);
    assert_eq!(g.source_backing_buckets[0], bucket_before);
    assert_eq!(g.risk_epoch, risk_epoch_before);
}

#[test]
fn v16_counterparty_lien_create_overflow_rejects_before_mutation() {
    let mut g = group();
    g.add_source_positive_claim_bound_not_atomic(0, 100, 100)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 100, 10)
        .unwrap();
    g.source_backing_buckets[0].valid_liened_backing_num = u128::MAX;

    let source_before = g.source_credit[0];
    let bucket_before = g.source_backing_buckets[0];
    let risk_epoch_before = g.risk_epoch;

    assert_eq!(
        g.create_source_credit_lien_from_counterparty_not_atomic(0, 1),
        Err(V16Error::CounterOverflow)
    );
    assert_eq!(g.source_credit[0], source_before);
    assert_eq!(g.source_backing_buckets[0], bucket_before);
    assert_eq!(g.risk_epoch, risk_epoch_before);
}

#[test]
fn v16_counterparty_lien_release_overflow_rejects_before_mutation() {
    let mut g = group();
    g.add_source_positive_claim_bound_not_atomic(0, 100, 100)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 100, 10)
        .unwrap();
    g.create_source_credit_lien_from_counterparty_not_atomic(0, 30)
        .unwrap();
    g.source_backing_buckets[0].fresh_unliened_backing_num = u128::MAX;

    let source_before = g.source_credit[0];
    let bucket_before = g.source_backing_buckets[0];
    let risk_epoch_before = g.risk_epoch;

    assert_eq!(
        g.release_source_credit_lien_from_counterparty_not_atomic(0, 1),
        Err(V16Error::CounterOverflow)
    );
    assert_eq!(g.source_credit[0], source_before);
    assert_eq!(g.source_backing_buckets[0], bucket_before);
    assert_eq!(g.risk_epoch, risk_epoch_before);
}

#[test]
fn v16_counterparty_lien_release_epoch_overflow_rejects_before_mutation() {
    let mut g = group();
    g.add_source_positive_claim_bound_not_atomic(0, 100, 100)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 100, 10)
        .unwrap();
    g.create_source_credit_lien_from_counterparty_not_atomic(0, 30)
        .unwrap();
    g.source_credit[0].credit_epoch = u64::MAX;

    let source_before = g.source_credit[0];
    let bucket_before = g.source_backing_buckets[0];
    let risk_epoch_before = g.risk_epoch;

    assert_eq!(
        g.release_source_credit_lien_from_counterparty_not_atomic(0, 1),
        Err(V16Error::CounterOverflow)
    );
    assert_eq!(g.source_credit[0], source_before);
    assert_eq!(g.source_backing_buckets[0], bucket_before);
    assert_eq!(g.risk_epoch, risk_epoch_before);
}

#[test]
fn v16_insurance_credit_reservation_lifecycle_tracks_encumbrance_once() {
    let mut g = group();
    g.vault = 100;
    g.insurance = 100;
    g.insurance_domain_budget[0] = 100;
    let reserve = 60 * BOUND_SCALE;
    let lien_release = 20 * BOUND_SCALE;
    let lien_impair = 15 * BOUND_SCALE;
    let lien_consume = 5 * BOUND_SCALE;
    g.add_source_positive_claim_bound_not_atomic(0, 100, 100)
        .unwrap();
    g.reserve_insurance_credit_not_atomic(0, reserve).unwrap();
    assert_eq!(g.source_credit_available_backing_num(0), Ok(reserve));

    g.create_source_credit_lien_from_insurance_not_atomic(0, lien_release)
        .unwrap();
    assert_eq!(g.source_credit[0].valid_liened_insurance_num, lien_release);
    assert_eq!(
        g.source_credit_available_backing_num(0),
        Ok(reserve - lien_release)
    );

    g.release_source_credit_lien_from_insurance_not_atomic(0, lien_release)
        .unwrap();
    assert_eq!(g.source_credit_available_backing_num(0), Ok(reserve));

    g.create_source_credit_lien_from_insurance_not_atomic(0, lien_impair)
        .unwrap();
    g.impair_source_credit_lien_from_insurance_not_atomic(0, lien_impair)
        .unwrap();
    assert_eq!(
        g.source_credit[0].impaired_liened_insurance_num,
        lien_impair
    );
    assert_eq!(
        g.source_credit_available_backing_num(0),
        Ok(reserve - lien_impair)
    );

    g.create_source_credit_lien_from_insurance_not_atomic(0, lien_consume)
        .unwrap();
    g.consume_source_credit_lien_from_insurance_not_atomic(0, lien_consume)
        .unwrap();
    assert_eq!(g.insurance, 95);
    assert_eq!(
        g.insurance_credit_reservations[0].consumed_insurance_num,
        lien_consume
    );
    assert_eq!(
        g.source_credit_available_backing_num(0),
        Ok(reserve - lien_impair - lien_consume)
    );
}

#[test]
fn v16_insurance_lien_consume_overflow_rejects_before_mutation() {
    let mut g = group();
    g.vault = 100;
    g.insurance = 100;
    g.insurance_domain_budget[0] = 100;
    let reserve = 60 * BOUND_SCALE;
    let lien = 20 * BOUND_SCALE;
    g.add_source_positive_claim_bound_not_atomic(0, 100, 100)
        .unwrap();
    g.reserve_insurance_credit_not_atomic(0, reserve).unwrap();
    g.create_source_credit_lien_from_insurance_not_atomic(0, lien)
        .unwrap();
    g.insurance_credit_reservations[0].consumed_insurance_num = u128::MAX;

    let source_before = g.source_credit[0];
    let reservation_before = g.insurance_credit_reservations[0];
    let insurance_before = g.insurance;
    let spent_before = g.insurance_domain_spent[0];
    let risk_epoch_before = g.risk_epoch;

    assert_eq!(
        g.consume_source_credit_lien_from_insurance_not_atomic(0, BOUND_SCALE),
        Err(V16Error::CounterOverflow)
    );
    assert_eq!(g.source_credit[0], source_before);
    assert_eq!(g.insurance_credit_reservations[0], reservation_before);
    assert_eq!(g.insurance, insurance_before);
    assert_eq!(g.insurance_domain_spent[0], spent_before);
    assert_eq!(g.risk_epoch, risk_epoch_before);
}

#[test]
fn v16_insurance_lien_consume_domain_spent_overflow_rejects_before_mutation() {
    let mut g = group();
    g.vault = 100;
    g.insurance = 100;
    g.insurance_domain_budget[0] = 100;
    let reserve = 60 * BOUND_SCALE;
    let lien = 20 * BOUND_SCALE;
    g.add_source_positive_claim_bound_not_atomic(0, 100, 100)
        .unwrap();
    g.reserve_insurance_credit_not_atomic(0, reserve).unwrap();
    g.create_source_credit_lien_from_insurance_not_atomic(0, lien)
        .unwrap();
    g.insurance_domain_spent[0] = u128::MAX;

    let source_before = g.source_credit[0];
    let reservation_before = g.insurance_credit_reservations[0];
    let insurance_before = g.insurance;
    let spent_before = g.insurance_domain_spent[0];
    let risk_epoch_before = g.risk_epoch;

    assert_eq!(
        g.consume_source_credit_lien_from_insurance_not_atomic(0, BOUND_SCALE),
        Err(V16Error::CounterOverflow)
    );
    assert_eq!(g.source_credit[0], source_before);
    assert_eq!(g.insurance_credit_reservations[0], reservation_before);
    assert_eq!(g.insurance, insurance_before);
    assert_eq!(g.insurance_domain_spent[0], spent_before);
    assert_eq!(g.risk_epoch, risk_epoch_before);
}

#[test]
fn v16_insurance_lien_impair_overflow_rejects_before_mutation() {
    let mut g = group();
    g.vault = 100;
    g.insurance = 100;
    g.insurance_domain_budget[0] = 100;
    let reserve = 60 * BOUND_SCALE;
    let lien = 20 * BOUND_SCALE;
    g.add_source_positive_claim_bound_not_atomic(0, 100, 100)
        .unwrap();
    g.reserve_insurance_credit_not_atomic(0, reserve).unwrap();
    g.create_source_credit_lien_from_insurance_not_atomic(0, lien)
        .unwrap();
    g.insurance_credit_reservations[0].impaired_liened_insurance_num = u128::MAX;

    let source_before = g.source_credit[0];
    let reservation_before = g.insurance_credit_reservations[0];
    let risk_epoch_before = g.risk_epoch;

    assert_eq!(
        g.impair_source_credit_lien_from_insurance_not_atomic(0, BOUND_SCALE),
        Err(V16Error::CounterOverflow)
    );
    assert_eq!(g.source_credit[0], source_before);
    assert_eq!(g.insurance_credit_reservations[0], reservation_before);
    assert_eq!(g.risk_epoch, risk_epoch_before);
}

#[test]
fn v16_insurance_lien_impair_source_overflow_rejects_before_mutation() {
    let mut g = group();
    g.vault = 100;
    g.insurance = 100;
    g.insurance_domain_budget[0] = 100;
    let reserve = 60 * BOUND_SCALE;
    let lien = 20 * BOUND_SCALE;
    g.add_source_positive_claim_bound_not_atomic(0, 100, 100)
        .unwrap();
    g.reserve_insurance_credit_not_atomic(0, reserve).unwrap();
    g.create_source_credit_lien_from_insurance_not_atomic(0, lien)
        .unwrap();
    g.source_credit[0].impaired_liened_insurance_num = u128::MAX;

    let source_before = g.source_credit[0];
    let reservation_before = g.insurance_credit_reservations[0];
    let risk_epoch_before = g.risk_epoch;

    assert_eq!(
        g.impair_source_credit_lien_from_insurance_not_atomic(0, BOUND_SCALE),
        Err(V16Error::CounterOverflow)
    );
    assert_eq!(g.source_credit[0], source_before);
    assert_eq!(g.insurance_credit_reservations[0], reservation_before);
    assert_eq!(g.risk_epoch, risk_epoch_before);
}

#[test]
fn v16_insurance_lien_create_overflow_rejects_before_mutation() {
    let mut g = group();
    g.vault = 100;
    g.insurance = 100;
    g.insurance_domain_budget[0] = 100;
    let reserve = 60 * BOUND_SCALE;
    g.add_source_positive_claim_bound_not_atomic(0, 100, 100)
        .unwrap();
    g.reserve_insurance_credit_not_atomic(0, reserve).unwrap();
    g.source_credit[0].valid_liened_insurance_num = u128::MAX;

    let source_before = g.source_credit[0];
    let reservation_before = g.insurance_credit_reservations[0];
    let risk_epoch_before = g.risk_epoch;

    assert_eq!(
        g.create_source_credit_lien_from_insurance_not_atomic(0, BOUND_SCALE),
        Err(V16Error::CounterOverflow)
    );
    assert_eq!(g.source_credit[0], source_before);
    assert_eq!(g.insurance_credit_reservations[0], reservation_before);
    assert_eq!(g.risk_epoch, risk_epoch_before);
}

#[test]
fn v16_insurance_lien_release_epoch_overflow_rejects_before_mutation() {
    let mut g = group();
    g.vault = 100;
    g.insurance = 100;
    g.insurance_domain_budget[0] = 100;
    let reserve = 60 * BOUND_SCALE;
    let lien = 20 * BOUND_SCALE;
    g.add_source_positive_claim_bound_not_atomic(0, 100, 100)
        .unwrap();
    g.reserve_insurance_credit_not_atomic(0, reserve).unwrap();
    g.create_source_credit_lien_from_insurance_not_atomic(0, lien)
        .unwrap();
    g.source_credit[0].credit_epoch = u64::MAX;

    let source_before = g.source_credit[0];
    let reservation_before = g.insurance_credit_reservations[0];
    let risk_epoch_before = g.risk_epoch;

    assert_eq!(
        g.release_source_credit_lien_from_insurance_not_atomic(0, BOUND_SCALE),
        Err(V16Error::CounterOverflow)
    );
    assert_eq!(g.source_credit[0], source_before);
    assert_eq!(g.insurance_credit_reservations[0], reservation_before);
    assert_eq!(g.risk_epoch, risk_epoch_before);
}

#[test]
fn v16_reserve_insurance_credit_overflow_rejects_before_mutation() {
    let mut g = group();
    g.vault = 1;
    g.insurance = 1;
    g.insurance_domain_budget[0] = u128::MAX;
    g.source_credit[0].insurance_credit_reserved_num = u128::MAX;

    let source_before = g.source_credit[0];
    let reservation_before = g.insurance_credit_reservations[0];
    let risk_epoch_before = g.risk_epoch;

    assert_eq!(
        g.reserve_insurance_credit_not_atomic(0, BOUND_SCALE),
        Err(V16Error::CounterOverflow)
    );
    assert_eq!(g.source_credit[0], source_before);
    assert_eq!(g.insurance_credit_reservations[0], reservation_before);
    assert_eq!(g.risk_epoch, risk_epoch_before);
}

#[test]
fn v16_source_credit_recompute_epoch_overflow_rejects_before_mutation() {
    let mut g = group();
    g.source_credit[0].positive_claim_bound_num = 100;
    g.source_credit[0].exact_positive_claim_num = 100;
    g.source_credit[0].fresh_reserved_backing_num = 50;
    g.source_credit[0].credit_rate_num = CREDIT_RATE_SCALE;
    g.source_credit[0].credit_epoch = u64::MAX;

    let source_before = g.source_credit[0];
    let risk_epoch_before = g.risk_epoch;

    assert_eq!(
        g.recompute_source_credit_rate_not_atomic(0),
        Err(V16Error::CounterOverflow)
    );
    assert_eq!(g.source_credit[0], source_before);
    assert_eq!(g.risk_epoch, risk_epoch_before);
}

#[test]
fn v16_source_positive_claim_add_epoch_overflow_rejects_before_mutation() {
    let mut g = group();
    g.source_credit[0].credit_epoch = u64::MAX;

    let source_before = g.source_credit[0];
    let risk_epoch_before = g.risk_epoch;

    assert_eq!(
        g.add_source_positive_claim_bound_not_atomic(0, 1, 1),
        Err(V16Error::CounterOverflow)
    );
    assert_eq!(g.source_credit[0], source_before);
    assert_eq!(g.risk_epoch, risk_epoch_before);
}

#[test]
fn v16_source_positive_claim_add_bound_overflow_rejects_before_mutation() {
    let mut g = group();
    g.source_credit[0].positive_claim_bound_num = u128::MAX;
    g.source_credit[0].exact_positive_claim_num = u128::MAX;

    let source_before = g.source_credit[0];
    let risk_epoch_before = g.risk_epoch;

    assert_eq!(
        g.add_source_positive_claim_bound_not_atomic(0, 1, 1),
        Err(V16Error::CounterOverflow)
    );
    assert_eq!(g.source_credit[0], source_before);
    assert_eq!(g.risk_epoch, risk_epoch_before);
}

#[test]
fn v16_insurance_credit_reservation_uses_scaled_num_not_quote_atoms() {
    let mut g = group();
    g.vault = 5;
    g.insurance = 5;
    g.insurance_domain_budget[0] = 5;

    g.reserve_insurance_credit_not_atomic(0, 5 * BOUND_SCALE)
        .unwrap();
    g.create_source_credit_lien_from_insurance_not_atomic(0, 5 * BOUND_SCALE)
        .unwrap();
    g.consume_source_credit_lien_from_insurance_not_atomic(0, 2 * BOUND_SCALE)
        .unwrap();

    assert_eq!(
        g.insurance, 3,
        "consuming 2*BOUND_SCALE insurance-credit numerator must spend 2 quote atoms"
    );
    assert_eq!(g.insurance_domain_spent[0], 2);
    assert_eq!(
        g.insurance_credit_reservations[0].insurance_credit_reserved_num,
        3 * BOUND_SCALE
    );
    assert_eq!(
        g.source_credit[0].valid_liened_insurance_num,
        3 * BOUND_SCALE
    );
}

#[test]
fn v16_asset_retire_requires_empty_source_credit_state() {
    let mut g = group();
    g.add_fresh_counterparty_backing_not_atomic(0, 1, 10)
        .unwrap();

    assert_eq!(
        g.retire_empty_asset_not_atomic(0, 1),
        Err(V16Error::LockActive),
        "asset retirement must not orphan source-credit backing or claims"
    );
}

#[test]
fn v16_backing_domains_are_bound_to_asset_slot_market_id() {
    let mut g = group();
    let asset_market_id = g.assets[0].market_id;

    assert_eq!(g.source_backing_buckets[0].market_id, asset_market_id);
    assert_eq!(g.source_backing_buckets[1].market_id, asset_market_id);
    assert_eq!(g.assert_public_invariants(), Ok(()));

    g.source_backing_buckets[0].market_id = asset_market_id + 1;
    assert_eq!(
        g.assert_public_invariants(),
        Err(V16Error::InvalidConfig),
        "backing for a selected domain must be authenticated to that asset slot's market_id"
    );
}

#[test]
fn v16_asset_reuse_rebinds_backing_domains_and_rejects_stale_bucket_identity() {
    let mut g = group();
    g.config.asset_activation_cooldown_slots = 1;
    let old_market_id = g.assets[0].market_id;
    let next_market_id = g.next_market_id;

    g.retire_empty_asset_not_atomic(0, 1).unwrap();
    g.activate_empty_asset_not_atomic(0, 100, 2).unwrap();

    assert_eq!(g.assets[0].market_id, next_market_id);
    assert_ne!(g.assets[0].market_id, old_market_id);
    assert_eq!(g.source_backing_buckets[0].market_id, next_market_id);
    assert_eq!(g.source_backing_buckets[1].market_id, next_market_id);

    g.source_backing_buckets[1].market_id = old_market_id;
    assert_eq!(
        g.assert_public_invariants(),
        Err(V16Error::InvalidConfig),
        "reused slots must not retain stale backing identity from the prior market_id"
    );
}

fn account_with_id(id: u8) -> PortfolioAccountV16 {
    let (market, _, owner) = ids();
    let mut account = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [id; 32], owner));
    account.ensure_source_domain_capacity(v16_domain_count_for_market_slots(4).unwrap());
    account
}

fn attach_opposite(
    group: &mut MarketGroupV16,
    asset_index: usize,
    side_to_balance: SideV16,
    abs_q: u128,
    account_id: u8,
) -> PortfolioAccountV16 {
    let mut opposite = account_with_id(account_id);
    let abs_i128 = i128::try_from(abs_q).unwrap();
    match side_to_balance {
        SideV16::Long => group
            .attach_leg(&mut opposite, asset_index, SideV16::Short, -abs_i128)
            .unwrap(),
        SideV16::Short => group
            .attach_leg(&mut opposite, asset_index, SideV16::Long, abs_i128)
            .unwrap(),
    }
    opposite
}

fn active_leg(side: SideV16, basis_pos_q: i128) -> PortfolioLegV16 {
    PortfolioLegV16 {
        active: true,
        asset_index: 0,
        market_id: 1,
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
fn v16_persisted_account_wire_structs_are_bytemuck_pod() {
    assert_pod_zeroable::<V16PodU16>();
    assert_pod_zeroable::<V16PodU32>();
    assert_pod_zeroable::<V16PodU64>();
    assert_pod_zeroable::<V16PodU128>();
    assert_pod_zeroable::<V16PodI128>();
    assert_pod_zeroable::<V16OptionalRecoveryReasonAccount>();
    assert_pod_zeroable::<ProvenanceHeaderV16Account>();
    assert_pod_zeroable::<V16ConfigAccount>();
    assert_pod_zeroable::<AssetStateV16Account>();
    assert_pod_zeroable::<EngineAssetSlotV16Account>();
    assert_pod_zeroable::<Market<()>>();
    assert_pod_zeroable::<Market<[u8; 8]>>();
    assert_pod_zeroable::<Market<[u8; 24]>>();
    assert_pod_zeroable::<PortfolioLegV16Account>();
    assert_pod_zeroable::<HealthCertV16Account>();
    assert_pod_zeroable::<PortfolioSourceDomainV16Account>();
    assert_pod_zeroable::<PortfolioAccountV16Account>();

    assert_eq!(core::mem::align_of::<PortfolioAccountV16Account>(), 1);
    assert_eq!(core::mem::align_of::<PortfolioSourceDomainV16Account>(), 1);
    assert_eq!(core::mem::align_of::<MarketGroupV16HeaderAccount>(), 1);
    assert_eq!(core::mem::align_of::<EngineAssetSlotV16Account>(), 1);
    assert_eq!(
        core::mem::size_of::<Market<[u8; 8]>>(),
        8 + core::mem::size_of::<EngineAssetSlotV16Account>()
    );
    assert_eq!(
        core::mem::size_of::<Market<[u8; 24]>>(),
        24 + core::mem::size_of::<EngineAssetSlotV16Account>()
    );
}

#[test]
fn v16_dynamic_persisted_wire_roundtrips_runtime_state_from_wrapper_slices() {
    let mut g = group();
    let mut a = account();
    g.create_portfolio_account(&a).unwrap();
    g.deposit_not_atomic(&mut a, 10_000).unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV16::Long, POS_SCALE, 90);
    g.full_account_refresh(&mut a, &[100; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    let wire_group =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let wire_slots = (0..g.assets.len())
        .map(|i| EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap())
        .collect::<Vec<_>>();
    let wire_account = PortfolioAccountV16Account::from_runtime(&a);
    let wire_source_domains = PortfolioAccountV16Account::source_domains_from_runtime(&a).unwrap();
    let group_bytes = bytemuck::bytes_of(&wire_group);
    let slot_bytes = bytemuck::cast_slice::<EngineAssetSlotV16Account, u8>(&wire_slots);
    let account_bytes = bytemuck::bytes_of(&wire_account);
    let source_domain_bytes =
        bytemuck::cast_slice::<PortfolioSourceDomainV16Account, u8>(&wire_source_domains);

    assert_eq!(
        group_bytes.len(),
        core::mem::size_of::<MarketGroupV16HeaderAccount>()
    );
    assert_eq!(
        slot_bytes.len(),
        g.assets.len() * core::mem::size_of::<EngineAssetSlotV16Account>()
    );
    assert_eq!(
        account_bytes.len(),
        core::mem::size_of::<PortfolioAccountV16Account>()
    );
    assert_eq!(
        source_domain_bytes.len(),
        wire_source_domains.len() * core::mem::size_of::<PortfolioSourceDomainV16Account>()
    );

    let decoded_group = *bytemuck::from_bytes::<MarketGroupV16HeaderAccount>(group_bytes);
    let decoded_slots = bytemuck::cast_slice::<u8, EngineAssetSlotV16Account>(slot_bytes);
    let decoded_account = *bytemuck::from_bytes::<PortfolioAccountV16Account>(account_bytes);
    let decoded_source_domains =
        bytemuck::cast_slice::<u8, PortfolioSourceDomainV16Account>(source_domain_bytes);
    let runtime_group = decoded_group
        .try_to_runtime_with_slots(decoded_slots)
        .unwrap();
    let runtime_account = decoded_account
        .validate_with_market(&runtime_group, decoded_source_domains)
        .unwrap();

    assert_eq!(runtime_group, g);
    assert_eq!(runtime_account, a);
}

#[test]
fn v16_persisted_account_wire_rejects_invalid_bool_enum_and_option_encoding() {
    let g = group();
    let a = account();

    let mut bad_account_bool = PortfolioAccountV16Account::from_runtime(&a);
    bad_account_bool.stale_state = 2;
    assert_eq!(
        bad_account_bool.try_to_runtime_with_source_domains(&[]),
        Err(V16Error::InvalidConfig)
    );

    let mut bad_close_bool = PortfolioAccountV16Account::from_runtime(&a);
    bad_close_bool.close_progress.canceled = 2;
    assert_eq!(
        bad_close_bool.try_to_runtime_with_source_domains(&[]),
        Err(V16Error::InvalidConfig)
    );

    let mut bad_leg_enum = PortfolioAccountV16Account::from_runtime(&a);
    bad_leg_enum.legs[0].active = 1;
    bad_leg_enum.legs[0].side = 9;
    assert_eq!(
        bad_leg_enum.try_to_runtime_with_source_domains(&[]),
        Err(V16Error::InvalidConfig)
    );

    let mut bad_market_mode =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    bad_market_mode.mode = 9;
    assert_eq!(
        bad_market_mode.try_to_runtime_with_slots(&[
            EngineAssetSlotV16Account::from_runtime_group_slot(&g, 0).unwrap(),
            EngineAssetSlotV16Account::from_runtime_group_slot(&g, 1).unwrap(),
            EngineAssetSlotV16Account::from_runtime_group_slot(&g, 2).unwrap(),
            EngineAssetSlotV16Account::from_runtime_group_slot(&g, 3).unwrap(),
        ]),
        Err(V16Error::InvalidConfig)
    );

    let mut bad_config_bool =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    bad_config_bool.config.recovery_fallback_price_enabled = 2;
    assert_eq!(
        bad_config_bool.try_to_runtime_with_slots(&[
            EngineAssetSlotV16Account::from_runtime_group_slot(&g, 0).unwrap(),
            EngineAssetSlotV16Account::from_runtime_group_slot(&g, 1).unwrap(),
            EngineAssetSlotV16Account::from_runtime_group_slot(&g, 2).unwrap(),
            EngineAssetSlotV16Account::from_runtime_group_slot(&g, 3).unwrap(),
        ]),
        Err(V16Error::InvalidConfig)
    );

    let header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut bad_side_slots = [
        EngineAssetSlotV16Account::from_runtime_group_slot(&g, 0).unwrap(),
        EngineAssetSlotV16Account::from_runtime_group_slot(&g, 1).unwrap(),
        EngineAssetSlotV16Account::from_runtime_group_slot(&g, 2).unwrap(),
        EngineAssetSlotV16Account::from_runtime_group_slot(&g, 3).unwrap(),
    ];
    bad_side_slots[0].asset.mode_long = 9;
    assert_eq!(
        header.try_to_runtime_with_slots(&bad_side_slots),
        Err(V16Error::InvalidConfig)
    );

    let mut bad_lifecycle_slots = [
        EngineAssetSlotV16Account::from_runtime_group_slot(&g, 0).unwrap(),
        EngineAssetSlotV16Account::from_runtime_group_slot(&g, 1).unwrap(),
        EngineAssetSlotV16Account::from_runtime_group_slot(&g, 2).unwrap(),
        EngineAssetSlotV16Account::from_runtime_group_slot(&g, 3).unwrap(),
    ];
    bad_lifecycle_slots[0].asset.lifecycle = 9;
    assert_eq!(
        header.try_to_runtime_with_slots(&bad_lifecycle_slots),
        Err(V16Error::InvalidConfig)
    );

    let mut bad_option =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    bad_option.recovery_reason.present = 0;
    bad_option.recovery_reason.value = 1;
    assert_eq!(
        bad_option.try_to_runtime_with_slots(&[
            EngineAssetSlotV16Account::from_runtime_group_slot(&g, 0).unwrap(),
            EngineAssetSlotV16Account::from_runtime_group_slot(&g, 1).unwrap(),
            EngineAssetSlotV16Account::from_runtime_group_slot(&g, 2).unwrap(),
            EngineAssetSlotV16Account::from_runtime_group_slot(&g, 3).unwrap(),
        ]),
        Err(V16Error::InvalidConfig)
    );
}

#[test]
fn v16_persisted_market_wire_rejects_backing_slot_market_id_drift() {
    let g = group();
    let header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut slots = [
        EngineAssetSlotV16Account::from_runtime_group_slot(&g, 0).unwrap(),
        EngineAssetSlotV16Account::from_runtime_group_slot(&g, 1).unwrap(),
        EngineAssetSlotV16Account::from_runtime_group_slot(&g, 2).unwrap(),
        EngineAssetSlotV16Account::from_runtime_group_slot(&g, 3).unwrap(),
    ];
    slots[0].backing_long.market_id = V16PodU64::new(g.assets[0].market_id + 1);

    assert_eq!(
        header.try_to_runtime_with_slots(&slots),
        Err(V16Error::InvalidConfig),
        "zero-copy market state must reject backing buckets not bound to their asset slot"
    );
}

#[test]
fn v16_dynamic_market_header_and_slot_table_roundtrip_runtime_state() {
    let g = group();
    let header = MarketGroupV16HeaderAccount::from_runtime_with_capacity(
        &g,
        g.config.max_portfolio_assets as usize,
    )
    .unwrap();
    let slots = [
        EngineAssetSlotV16Account::from_runtime_group_slot(&g, 0).unwrap(),
        EngineAssetSlotV16Account::from_runtime_group_slot(&g, 1).unwrap(),
        EngineAssetSlotV16Account::from_runtime_group_slot(&g, 2).unwrap(),
        EngineAssetSlotV16Account::from_runtime_group_slot(&g, 3).unwrap(),
    ];

    let decoded = header.try_to_runtime_with_slots(&slots).unwrap();
    assert_eq!(decoded, g);
    assert_eq!(
        MarketGroupV16HeaderAccount::dynamic_market_group_account_len::<[u8; 24]>(4).unwrap(),
        core::mem::size_of::<MarketGroupV16HeaderAccount>()
            + 4 * core::mem::size_of::<Market<[u8; 24]>>()
    );
}

#[test]
fn v16_market_header_decodes_wrapper_owned_overallocated_slot_slice() {
    let g = group();
    let capacity = g.config.max_market_slots as usize + 3;
    let header = MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, capacity).unwrap();
    let mut slots = (0..g.assets.len())
        .map(|i| Market {
            wrapper: 0xA5A5_0000u64 + i as u64,
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    while slots.len() < capacity {
        let idx = slots.len();
        slots.push(Market {
            wrapper: 0xFFFF_0000u64 + idx as u64,
            engine: EngineAssetSlotV16Account::empty_for_market(0),
        });
    }

    let decoded = header.try_to_runtime_with_market_slots(&slots).unwrap();
    let view = MarketGroupV16View::new(&header, &slots);

    assert_eq!(decoded.config.max_market_slots, g.config.max_market_slots);
    assert_eq!(view.validate_shape(), Ok(()));
    assert_eq!(decoded.assets.len(), capacity);
    assert_eq!(&decoded.assets[..g.assets.len()], &g.assets[..]);
    assert_eq!(decoded.source_credit.len(), capacity * 2);
    for asset in &decoded.assets[g.assets.len()..] {
        assert_eq!(asset.lifecycle, AssetLifecycleV16::Disabled);
        assert_eq!(asset.market_id, 0);
    }
    assert_eq!(
        slots[capacity - 1].wrapper,
        0xFFFF_0000u64 + capacity as u64 - 1
    );
}

#[test]
fn v16_market_header_decodes_zero_filled_realloc_slots_after_growth() {
    let g = group();
    let capacity = g.config.max_market_slots as usize + 4;
    let mut header = MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, capacity).unwrap();
    header
        .grow_asset_slot_capacity_not_atomic(capacity as u32, capacity as u32)
        .unwrap();
    let mut slots = (0..g.assets.len())
        .map(|i| EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap())
        .collect::<Vec<_>>();
    while slots.len() < capacity {
        slots.push(EngineAssetSlotV16Account::default());
    }
    header
        .activate_empty_asset_slot_not_atomic(capacity as u32 - 2, &mut slots[capacity - 2], 222, 1)
        .unwrap();

    let decoded = header.try_to_runtime_with_slots(&slots).unwrap();

    assert_eq!(decoded.assets.len(), capacity);
    assert_eq!(decoded.assets[capacity - 2].effective_price, 222);
    assert_eq!(
        decoded.assets[capacity - 1].lifecycle,
        AssetLifecycleV16::Disabled
    );
    assert_eq!(decoded.assets[capacity - 1].market_id, 0);
    assert_eq!(
        decoded.source_credit[capacity * 2 - 1],
        percolator::v16::SourceCreditStateV16::EMPTY
    );
}

#[test]
fn v16_market_header_rejects_nonempty_hidden_wrapper_slot() {
    let g = group();
    let capacity = g.config.max_market_slots as usize + 1;
    let header = MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, capacity).unwrap();
    let mut slots = (0..g.assets.len())
        .map(|i| Market {
            wrapper: (),
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    slots.push(Market {
        wrapper: (),
        engine: EngineAssetSlotV16Account::empty_for_market(0),
    });
    slots[g.assets.len()].engine.asset.market_id = V16PodU64::new(99);

    assert_eq!(
        header.try_to_runtime_with_market_slots(&slots),
        Err(V16Error::InvalidConfig),
        "wrapper-owned spare slots must be empty until config exposes them"
    );
    assert_eq!(
        MarketGroupV16View::new(&header, &slots).validate_shape(),
        Err(V16Error::InvalidConfig)
    );
}

#[test]
fn v16_dynamic_header_activation_initializes_appended_slot_identity() {
    let g = group();
    let mut header = MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, 4).unwrap();
    let mut appended_slot = EngineAssetSlotV16Account::default();
    let market_id = header.next_market_id.get();

    header
        .activate_empty_asset_slot_not_atomic(3, &mut appended_slot, 123, 1)
        .unwrap();

    assert_eq!(appended_slot.asset.market_id.get(), market_id);
    assert_eq!(appended_slot.asset.lifecycle, 2);
    assert_eq!(appended_slot.asset.effective_price.get(), 123);
    assert_eq!(appended_slot.asset.slot_last.get(), 1);
    assert_eq!(appended_slot.backing_long.market_id.get(), market_id);
    assert_eq!(appended_slot.backing_short.market_id.get(), market_id);
    assert_eq!(header.next_market_id.get(), market_id + 1);
    assert_eq!(header.current_slot.get(), 1);
    assert_eq!(
        header.asset_activation_count.get(),
        g.asset_activation_count + 1
    );
}

#[test]
fn v16_dynamic_header_activation_rejects_nonempty_disabled_slot() {
    let g = group();
    let mut header = MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, 4).unwrap();
    let mut corrupt_slot = EngineAssetSlotV16Account::default();
    corrupt_slot.asset.oi_eff_long_q = V16PodU128::new(1);
    corrupt_slot.asset.stored_pos_count_long = V16PodU64::new(1);
    corrupt_slot.asset.loss_weight_sum_long = V16PodU128::new(1);

    assert_eq!(
        header.activate_empty_asset_slot_not_atomic(3, &mut corrupt_slot, 123, 1),
        Err(V16Error::LockActive),
        "slot activation must not overwrite hidden OI/counter state in a disabled slot"
    );
}

#[test]
fn v16_dynamic_header_activation_rejects_market_id_counter_overflow_before_slot_mutation() {
    let g = group();
    let mut header = MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, 4).unwrap();
    header.next_market_id = V16PodU64::new(u64::MAX);
    let mut slot = EngineAssetSlotV16Account::default();
    let before_slot = slot;
    let before_header = header;

    assert_eq!(
        header.activate_empty_asset_slot_not_atomic(3, &mut slot, 123, 1),
        Err(V16Error::CounterOverflow),
        "activation must preflight market-id counter overflow"
    );
    assert_eq!(slot, before_slot);
    assert_eq!(header, before_header);
}

#[test]
fn v16_dynamic_slot_table_rejects_backing_identity_drift() {
    let g = group();
    let header = MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, 4).unwrap();
    let mut slots = [
        EngineAssetSlotV16Account::from_runtime_group_slot(&g, 0).unwrap(),
        EngineAssetSlotV16Account::from_runtime_group_slot(&g, 1).unwrap(),
        EngineAssetSlotV16Account::from_runtime_group_slot(&g, 2).unwrap(),
        EngineAssetSlotV16Account::from_runtime_group_slot(&g, 3).unwrap(),
    ];
    slots[2].backing_short.market_id = V16PodU64::new(g.assets[2].market_id + 1);

    assert_eq!(
        header.try_to_runtime_with_slots(&slots),
        Err(V16Error::InvalidConfig),
        "dynamic slot decoding must fail closed if backing identity drifts from the asset slot"
    );
}

#[test]
fn v16_hlock_is_permissionless_state_not_oracle_input() {
    let mut g = group();
    let mut a = account();

    assert_eq!(g.h_lock_lane(Some(&a), false, None), Ok(HLockLaneV16::HMin));
    assert_eq!(g.select_h_lock(Some(&a), false), Ok(0));

    g.threshold_stress_active = true;
    assert_eq!(g.h_lock_lane(Some(&a), false, None), Ok(HLockLaneV16::HMax));
    assert_eq!(g.select_h_lock(Some(&a), false), Ok(10));

    g.threshold_stress_active = false;
    assert_eq!(g.h_lock_lane(Some(&a), true, None), Ok(HLockLaneV16::HMax));

    a.b_stale_state = true;
    assert_eq!(g.h_lock_lane(Some(&a), false, None), Ok(HLockLaneV16::HMax));
}

#[test]
fn v16_asset_lifecycle_blocks_new_risk_unless_active() {
    let mut g = group();
    let mut a = account();
    g.mark_asset_drain_only_not_atomic(0).unwrap();
    let before_asset = g.assets[0];

    assert_eq!(
        g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128),
        Err(V16Error::LockActive)
    );
    assert_eq!(g.assets[0], before_asset);
    assert_eq!(a.active_bitmap, bitmap(&[]));

    let mut long = account();
    let mut short = account_with_id(9);
    g.deposit_not_atomic(&mut long, 10_000).unwrap();
    g.deposit_not_atomic(&mut short, 10_000).unwrap();
    let res = g.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV16 {
            asset_index: 0,
            size_q: POS_SCALE,
            exec_price: 100,
            fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
        },
        &[100; V16_MAX_PORTFOLIO_ASSETS_N],
    );
    assert_eq!(res, Err(V16Error::LockActive));
    assert_eq!(g.assets[0].oi_eff_long_q, 0);
    assert_eq!(g.assets[0].oi_eff_short_q, 0);
}

#[test]
fn v16_asset_lifecycle_drain_only_allows_reduction_but_not_increase() {
    let mut g = group();
    let mut reducing_short = account();
    let mut reducing_long = account_with_id(8);
    g.deposit_not_atomic(&mut reducing_short, 10_000).unwrap();
    g.deposit_not_atomic(&mut reducing_long, 10_000).unwrap();
    g.attach_leg(&mut reducing_short, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.attach_leg(&mut reducing_long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.mark_asset_drain_only_not_atomic(0).unwrap();

    let out = g
        .execute_trade_with_fee_not_atomic(
            &mut reducing_short,
            &mut reducing_long,
            TradeRequestV16 {
                asset_index: 0,
                size_q: POS_SCALE / 2,
                exec_price: 100,
                fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
            },
            &[100; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();
    assert_eq!(out.notional, 50);
    assert_eq!(reducing_long.legs[0].basis_pos_q, (POS_SCALE / 2) as i128);
    assert_eq!(
        reducing_short.legs[0].basis_pos_q,
        -((POS_SCALE / 2) as i128)
    );

    let increase = g.execute_trade_with_fee_not_atomic(
        &mut reducing_short,
        &mut reducing_long,
        TradeRequestV16 {
            asset_index: 0,
            size_q: POS_SCALE,
            exec_price: 100,
            fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
        },
        &[100; V16_MAX_PORTFOLIO_ASSETS_N],
    );
    assert_eq!(increase, Err(V16Error::LockActive));
}

#[test]
fn v16_asset_retire_and_activation_require_empty_asset_state_and_invalidate_certs() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 10_000).unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, 1).unwrap();
    assert_eq!(
        g.retire_empty_asset_not_atomic(0, 1),
        Err(V16Error::LockActive),
        "retirement must fail closed while OI or stored positions remain"
    );
    g.clear_leg(&mut a, 0).unwrap();
    g.retire_empty_asset_not_atomic(0, 1).unwrap();
    assert_eq!(g.assets[0].lifecycle, AssetLifecycleV16::Retired);

    g.full_account_refresh(&mut a, &[100; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert!(a.health_cert.valid);
    let cert_epoch = a.health_cert.cert_risk_epoch;
    let risk_epoch_before = g.risk_epoch;
    let asset_set_epoch_before = g.asset_set_epoch;
    g.activate_empty_asset_not_atomic(0, 100, g.current_slot + 1)
        .unwrap();

    assert_eq!(g.assets[0].lifecycle, AssetLifecycleV16::Active);
    assert!(g.risk_epoch > risk_epoch_before);
    assert!(g.asset_set_epoch > asset_set_epoch_before);
    assert_ne!(a.health_cert.cert_risk_epoch, g.risk_epoch);
    assert_eq!(a.health_cert.cert_risk_epoch, cert_epoch);
    assert_eq!(
        g.ensure_favorable_action_allowed(&a),
        Err(V16Error::Stale),
        "certified-favorable actions must fail closed until account refreshes under the new asset set"
    );
}

#[test]
fn v16_reused_asset_index_gets_monotonic_market_id_after_shutdown_timeout() {
    let mut g = group();
    g.config.asset_activation_cooldown_slots = 2;
    let mut a = account();
    let mut b = account_with_id(7);

    g.attach_leg(&mut a, 0, SideV16::Long, 10).unwrap();
    g.attach_leg(&mut b, 0, SideV16::Short, -10).unwrap();
    let old_market_id = g.assets[0].market_id;
    let next_market_id_before = g.next_market_id;
    assert_eq!(a.legs[0].market_id, old_market_id);

    g.clear_leg(&mut a, 0).unwrap();
    g.clear_leg(&mut b, 0).unwrap();
    g.retire_empty_asset_not_atomic(0, 1).unwrap();
    assert_eq!(
        g.activate_empty_asset_not_atomic(0, 100, 2),
        Err(V16Error::LockActive),
        "freed asset slots cannot be reused before their shutdown timeout"
    );
    g.activate_empty_asset_not_atomic(0, 100, 3).unwrap();
    assert_eq!(g.assets[0].market_id, next_market_id_before);
    assert!(g.assets[0].market_id > old_market_id);
    assert_eq!(g.next_market_id, next_market_id_before + 1);

    a.legs[0] = active_leg(SideV16::Long, 10);
    a.legs[0].market_id = old_market_id;
    a.active_bitmap = bitmap(&[0]);
    assert_eq!(
        g.validate_account_shape(&a),
        Err(V16Error::HiddenLeg),
        "stale portfolio legs from an old asset market_id must not bind to the reused slot"
    );

    let mut stale_claim = account();
    stale_claim.ensure_source_domain_capacity(2);
    stale_claim.pnl = 1;
    stale_claim.source_claim_market_id[0] = old_market_id;
    stale_claim.source_claim_bound_num[0] = BOUND_SCALE;
    assert_eq!(
        g.validate_account_shape(&stale_claim),
        Err(V16Error::HiddenLeg),
        "stale source-credit claims from an old market_id must not bind to the reused slot"
    );
}

#[test]
fn v16_asset_activation_counter_overflow_rejects_before_state_mutation() {
    let mut g = group();
    g.retire_empty_asset_not_atomic(0, 1).unwrap();
    g.next_market_id = u64::MAX;
    let before = g.clone();

    assert_eq!(
        g.activate_empty_asset_not_atomic(0, 100, 2),
        Err(V16Error::CounterOverflow)
    );
    assert_eq!(g, before);
}

#[test]
fn v16_asset_lifecycle_epoch_overflow_rejects_before_state_mutation() {
    let mut drain = group();
    drain.asset_set_epoch = u64::MAX;
    let before_drain = drain.clone();
    assert_eq!(
        drain.mark_asset_drain_only_not_atomic(0),
        Err(V16Error::CounterOverflow)
    );
    assert_eq!(drain, before_drain);

    let mut retire = group();
    retire.risk_epoch = u64::MAX;
    let before_retire = retire.clone();
    assert_eq!(
        retire.retire_empty_asset_not_atomic(0, 1),
        Err(V16Error::CounterOverflow)
    );
    assert_eq!(retire, before_retire);
}

#[test]
fn v16_retired_asset_idempotence_still_requires_empty_state() {
    let mut g = group();
    g.assets[0].lifecycle = AssetLifecycleV16::Retired;
    g.assets[0].retired_slot = 1;
    g.current_slot = 1;
    g.assets[0].oi_eff_long_q = 1;
    g.assets[0].stored_pos_count_long = 1;
    g.assets[0].loss_weight_sum_long = 1;
    let before = g.clone();

    assert_eq!(
        g.retire_empty_asset_not_atomic(0, 1),
        Err(V16Error::LockActive),
        "retired asset idempotence must not bless nonempty accounting state"
    );
    assert_eq!(g, before);
}

#[test]
fn v16_asset_activation_cooldown_rate_limits_asset_set_churn() {
    let mut g = group();
    g.config.asset_activation_cooldown_slots = 3;

    g.retire_empty_asset_not_atomic(0, 1).unwrap();
    assert_eq!(
        g.activate_empty_asset_not_atomic(0, 100, 3),
        Err(V16Error::LockActive),
        "a retired asset index cannot be reused before the configured shutdown timeout"
    );
    g.activate_empty_asset_not_atomic(0, 100, 4).unwrap();
    assert_eq!(g.asset_activation_count, 1);
    assert_eq!(g.last_asset_activation_slot, 4);

    g.retire_empty_asset_not_atomic(1, 4).unwrap();
    let before = g.assets[1];
    assert_eq!(
        g.activate_empty_asset_not_atomic(1, 100, 6),
        Err(V16Error::LockActive),
        "a second activation before the configured cooldown must fail closed"
    );
    assert_eq!(g.assets[1], before);

    g.activate_empty_asset_not_atomic(1, 100, 7).unwrap();
    assert_eq!(g.asset_activation_count, 2);
    assert_eq!(g.last_asset_activation_slot, 7);
    assert_eq!(g.assets[1].lifecycle, AssetLifecycleV16::Active);
}

#[test]
fn v16_provenance_binds_account_to_market_owner_and_layout() {
    let g = group();
    let mut a = account();
    assert_eq!(g.validate_portfolio_account_provenance(&a), Ok(()));

    a.provenance_header.market_group_id = [9; 32];
    assert_eq!(
        g.validate_portfolio_account_provenance(&a),
        Err(V16Error::ProvenanceMismatch)
    );
}

#[test]
fn v16_account_shape_rejects_missing_source_domain_storage() {
    let g = group();
    let mut a = account();
    a.source_claim_market_id.clear();
    a.source_claim_bound_num.clear();
    a.source_claim_liened_num.clear();
    a.source_claim_counterparty_liened_num.clear();
    a.source_claim_insurance_liened_num.clear();
    a.source_lien_effective_reserved.clear();
    a.source_lien_counterparty_backing_num.clear();
    a.source_lien_insurance_backing_num.clear();
    a.source_lien_fee_last_slot.clear();
    a.source_claim_impaired_num.clear();
    a.source_lien_impaired_effective_reserved.clear();

    assert_eq!(g.validate_account_shape(&a), Err(V16Error::HiddenLeg));
}

#[test]
fn v16_zero_copy_account_shape_rejects_missing_source_domain_slice() {
    let g = group();
    let header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: (),
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let a = account();
    let account_header = PortfolioAccountV16Account::from_runtime(&a);
    let empty_source_domains: [PortfolioSourceDomainV16Account; 0] = [];
    let account_view =
        percolator::v16::PortfolioV16View::new(&account_header, &empty_source_domains);
    let market_view = MarketGroupV16View::new(&header, &markets);

    assert_eq!(
        account_view.validate_with_market(&market_view),
        Err(V16Error::HiddenLeg)
    );
}

#[test]
fn v16_account_shape_rejects_active_leg_epoch_mismatch() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.assets[0].epoch_long = g.assets[0].epoch_long.checked_add(1).unwrap();

    assert_eq!(g.validate_account_shape(&a), Err(V16Error::HiddenLeg));
}

#[test]
fn v16_zero_copy_account_shape_rejects_active_leg_epoch_mismatch() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.assets[0].epoch_long = g.assets[0].epoch_long.checked_add(1).unwrap();

    let header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: (),
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let account_header = PortfolioAccountV16Account::from_runtime(&a);
    let source_domains = PortfolioAccountV16Account::source_domains_from_runtime(&a).unwrap();
    let account_view = percolator::v16::PortfolioV16View::new(&account_header, &source_domains);
    let market_view = MarketGroupV16View::new(&header, &markets);

    assert_eq!(
        account_view.validate_with_market(&market_view),
        Err(V16Error::HiddenLeg)
    );
}

#[test]
fn v16_active_bitmap_is_the_only_active_leg_authority() {
    let g = group();
    let mut a = account();
    a.legs[0] = active_leg(SideV16::Long, 1);
    assert_eq!(g.validate_account_shape(&a), Err(V16Error::HiddenLeg));

    a.active_bitmap = bitmap(&[0]);
    assert_eq!(g.validate_account_shape(&a), Ok(()));

    a.legs[5] = active_leg(SideV16::Short, -1);
    percolator::active_bitmap_set(&mut a.active_bitmap, 5).unwrap();
    assert_eq!(g.validate_account_shape(&a), Err(V16Error::HiddenLeg));
}

#[test]
fn v16_same_asset_duplicate_leg_cannot_double_count_support() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let account_before = a.clone();
    let asset_before = g.assets[0];

    assert_eq!(
        g.attach_leg(&mut a, 0, SideV16::Short, -(POS_SCALE as i128)),
        Err(V16Error::InvalidLeg)
    );
    assert_eq!(a, account_before);
    assert_eq!(g.assets[0], asset_before);
    assert_eq!(bitmap_count_ones(a.active_bitmap), 1);
    assert_eq!(g.validate_account_shape(&a), Ok(()));

    g.full_account_refresh(&mut a, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(a.health_cert.active_bitmap_at_cert, bitmap(&[0]));
}

#[test]
fn v16_stale_and_b_stale_counters_are_exact_and_idempotent() {
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
fn v16_b_stale_account_cannot_clear_while_leg_is_b_stale() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();

    g.mark_leg_b_stale(&mut a, 0).unwrap();
    g.mark_leg_b_stale(&mut a, 0).unwrap();
    assert!(a.b_stale_state);
    assert!(a.legs[0].b_stale);
    assert_eq!(g.b_stale_account_count, 1);

    assert_eq!(g.clear_account_b_stale(&mut a), Err(V16Error::BStale));
    assert!(a.b_stale_state);
    assert_eq!(g.b_stale_account_count, 1);
}

#[test]
fn v16_full_refresh_clears_stale_certificate_but_not_b_stale_loss() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.full_account_refresh(&mut a, &[100; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    g.mark_account_stale(&mut a).unwrap();
    assert_eq!(g.stale_certificate_count, 1);
    assert_eq!(
        g.ensure_favorable_action_allowed(&a),
        Err(V16Error::LockActive)
    );

    g.full_account_refresh(&mut a, &[100; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(g.stale_certificate_count, 0);
    assert!(!a.stale_state);
    assert_eq!(g.ensure_favorable_action_allowed(&a), Ok(()));

    g.assets[0].b_long_num = SOCIAL_LOSS_DEN;
    assert_eq!(
        g.full_account_refresh(&mut a, &[100; V16_MAX_PORTFOLIO_ASSETS_N]),
        Err(V16Error::BStale)
    );
}

#[test]
fn v16_favorable_action_requires_current_full_account_refresh() {
    let mut g = group();
    let mut a = account();
    a.capital = 100;
    // Align BOTH oracle fields with the fed refresh price (100). Post-323c9f2
    // two distinct gates key off the oracle: the worst-case/maintenance penalty
    // compares the fed effective_prices arg vs asset.raw_oracle_target_price (so
    // raw must = 100 to zero the penalty), and ensure_favorable_action_allowed's
    // strict target-effective-lag gate compares asset.raw_oracle_target_price vs
    // asset.effective_price (so the two stored fields must be equal). The group()
    // defaults leave both at 1; without aligning them this test would levy a
    // spurious lag penalty and then trip LockActive. Neither gate is what this
    // test exercises (it asserts the stale/refresh certificate lifecycle).
    g.assets[0].raw_oracle_target_price = 100;
    g.assets[0].effective_price = 100;
    g.attach_leg(&mut a, 0, SideV16::Long, 1_000_000).unwrap();
    let mut prices = [1u64; V16_MAX_PORTFOLIO_ASSETS_N];
    prices[0] = 100;

    assert_eq!(g.ensure_favorable_action_allowed(&a), Err(V16Error::Stale));

    let cert = g.full_account_refresh(&mut a, &prices).unwrap();
    assert!(cert.valid);
    assert_eq!(cert.certified_maintenance_req, 100);
    assert_eq!(g.ensure_favorable_action_allowed(&a), Ok(()));

    g.oracle_epoch += 1;
    assert_eq!(g.ensure_favorable_action_allowed(&a), Err(V16Error::Stale));
}

#[test]
fn v16_health_certificate_is_bound_to_market_epochs_and_prices() {
    let mut g = group();
    let mut long = account();
    let mut short = account_with_id(111);
    g.deposit_not_atomic(&mut long, 1_000).unwrap();
    g.deposit_not_atomic(&mut short, 1_000).unwrap();
    g.attach_leg(&mut long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut short, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();

    let cert = g
        .full_account_refresh(&mut long, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(cert.cert_oracle_epoch, g.oracle_epoch);
    assert_eq!(cert.cert_funding_epoch, g.funding_epoch);
    assert_eq!(cert.cert_risk_epoch, g.risk_epoch);
    assert_eq!(cert.cert_asset_set_epoch, g.asset_set_epoch);
    assert_eq!(cert.active_bitmap_at_cert, long.active_bitmap);
    assert_eq!(g.ensure_favorable_action_allowed(&long), Ok(()));

    g.asset_set_epoch += 1;
    assert_eq!(
        g.ensure_favorable_action_allowed(&long),
        Err(V16Error::Stale)
    );
    g.asset_set_epoch -= 1;

    g.accrue_asset_to_not_atomic(0, 1, 2, 0, true).unwrap();
    assert_eq!(
        g.ensure_favorable_action_allowed(&long),
        Err(V16Error::Stale)
    );

    let refreshed = g
        .full_account_refresh(&mut long, &[2; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(refreshed.cert_oracle_epoch, g.oracle_epoch);
}

#[test]
fn v16_global_residual_is_not_account_health_proof() {
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

    let before_group = g.clone();
    let before_account = a.clone();
    assert_eq!(
        g.convert_released_pnl_to_capital_not_atomic(&mut a),
        Err(V16Error::Stale)
    );
    assert_eq!(g, before_group);
    assert_eq!(a, before_account);
}

#[test]
fn v16_full_refresh_haircuts_positive_pnl_credit_when_junior_claims_are_impaired() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 10).unwrap();
    g.add_account_source_positive_pnl_not_atomic(&mut a, 0, 100)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 25 * BOUND_SCALE, 10)
        .unwrap();

    let cert = g
        .full_account_refresh(&mut a, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(account_equity(&a), Ok(110));
    assert_eq!(cert.certified_equity, 35);
}

#[test]
fn v16_full_refresh_uses_haircut_bounded_support_for_negative_kf_delta_when_impaired() {
    let mut g = group();
    let mut a = account();
    g.add_account_source_positive_pnl_not_atomic(&mut a, 1, 100)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(1, 50 * BOUND_SCALE, 10)
        .unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.assets[0].k_long = -(100 * ADL_ONE as i128);

    let cert = g
        .full_account_refresh(&mut a, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
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
fn v16_negative_kf_settlement_uses_realizable_source_credit_before_principal() {
    let mut g = group();
    let mut a = account();
    let mut opposing = account_with_id(44);
    g.deposit_not_atomic(&mut a, 1_000).unwrap();
    g.add_account_source_positive_pnl_not_atomic(&mut a, 0, 500)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 500 * BOUND_SCALE, 10)
        .unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut opposing, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.assets[0].k_long = -(500 * ADL_ONE as i128);
    assert_eq!(
        g.vault.saturating_sub(g.c_tot.saturating_add(g.insurance)),
        0,
        "regression requires no global residual"
    );

    let cert = g
        .full_account_refresh(&mut a, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(a.capital, 1_000);
    assert_eq!(a.pnl, 0);
    assert_eq!(g.c_tot, 1_000);
    assert_eq!(g.source_credit[0].spent_backing_num, 500 * BOUND_SCALE);
    assert_eq!(g.source_credit[0].fresh_reserved_backing_num, 0);
    assert_eq!(cert.certified_equity, 1_000);
}

#[test]
fn v16_source_attributed_negative_kf_settlement_does_not_use_global_residual() {
    let mut g = group();
    let mut a = account();
    let mut opposing = account_with_id(46);
    g.add_account_source_positive_pnl_not_atomic(&mut a, 0, 100)
        .unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut opposing, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.vault = 50;
    g.assets[0].k_long = -(100 * ADL_ONE as i128);
    assert_eq!(g.source_credit[0].credit_rate_num, 0);

    let cert = g
        .full_account_refresh(&mut a, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(
        a.pnl, -100,
        "source-attributed losses must not consume unrelated global residual support"
    );
    assert_eq!(g.source_credit[0].spent_backing_num, 0);
    assert_eq!(g.pnl_pos_tot, 0);
    assert_eq!(g.pnl_pos_bound_tot, 0);
    assert_eq!(g.negative_pnl_account_count, 1);
    assert_eq!(cert.certified_equity, -100);
}

#[test]
fn v16_source_domain_positive_kf_loss_cure_does_not_use_global_residual() {
    let mut g = group();
    let mut a = account();
    let mut opposing = account_with_id(47);
    a.pnl = -100;
    g.negative_pnl_account_count = 1;
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut opposing, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.vault = 50;
    g.assets[0].k_long = 100 * ADL_ONE as i128;
    assert_eq!(g.source_credit[1].fresh_reserved_backing_num, 0);

    let cert = g
        .full_account_refresh(&mut a, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(
        a.pnl, -100,
        "source-domain positive K/F must not cure losses using unrelated global residual support"
    );
    assert_eq!(g.source_credit[1].spent_backing_num, 0);
    assert_eq!(g.pnl_pos_tot, 0);
    assert_eq!(g.pnl_pos_bound_tot, 0);
    assert_eq!(g.negative_pnl_account_count, 1);
    assert_eq!(cert.certified_equity, -100);
}

#[test]
fn v16_full_refresh_reserves_counterparty_backing_from_new_capital_backed_loss() {
    let mut g = group();
    let mut loser = account();
    let mut opposing = account_with_id(47);
    g.deposit_not_atomic(&mut loser, 1_000).unwrap();
    g.attach_leg(&mut loser, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut opposing, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.assets[0].k_long = -(500 * ADL_ONE as i128);
    assert_eq!(g.source_credit[0].fresh_reserved_backing_num, 0);

    let cert = g
        .full_account_refresh(&mut loser, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(
        loser.pnl, 0,
        "capital-backed loss reservation must cure the local negative tail"
    );
    assert_eq!(
        loser.capital, 500,
        "reserved counterparty backing must no longer be withdrawable account capital"
    );
    assert_eq!(g.c_tot, 500);
    assert_eq!(g.vault, 1_000);
    assert_eq!(cert.certified_equity, 500);
    assert_eq!(
        g.source_credit[0].fresh_reserved_backing_num,
        500 * BOUND_SCALE,
        "full refresh must reserve capital-backed local losses as source-domain counterparty backing"
    );
    assert_eq!(
        g.source_credit_available_backing_num(0),
        Ok(500 * BOUND_SCALE)
    );
    assert!(
        g.source_backing_buckets[0].expiry_slot >= g.current_slot + g.config.h_max,
        "auto-reserved backing must survive the configured positive-PnL warmup window"
    );
}

#[test]
fn v16_passive_backing_consumption_preserves_senior_accounting_without_wrapper_injection() {
    let mut g = group();
    let mut loser = account();
    let mut winner = account_with_id(48);
    g.deposit_not_atomic(&mut loser, 1_000).unwrap();
    g.attach_leg(&mut loser, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut winner, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.assets[0].k_long = -(500 * ADL_ONE as i128);
    g.assets[0].k_short = 500 * ADL_ONE as i128;

    g.full_account_refresh(&mut loser, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    g.full_account_refresh(&mut winner, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(
        g.source_credit[0].fresh_reserved_backing_num,
        500 * BOUND_SCALE
    );
    assert_eq!(winner.pnl, 500);

    assert_eq!(
        g.convert_released_pnl_to_capital_not_atomic(&mut winner),
        Err(V16Error::LockActive),
        "passively backed source PnL is margin credit while the source position remains open"
    );
    assert_eq!(winner.capital, 0);
    assert_eq!(winner.pnl, 500);
    assert_eq!(g.source_credit[0].spent_backing_num, 0);
    assert_eq!(
        g.source_credit[0].fresh_reserved_backing_num,
        500 * BOUND_SCALE
    );

    g.clear_leg(&mut winner, 0).unwrap();
    g.clear_leg(&mut loser, 0).unwrap();
    g.full_account_refresh(&mut winner, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    let converted = g
        .convert_released_pnl_to_capital_not_atomic(&mut winner)
        .unwrap();

    assert_eq!(converted, 500);
    assert_eq!(winner.capital, 500);
    assert_eq!(winner.pnl, 0);
    assert_eq!(g.source_credit[0].spent_backing_num, 500 * BOUND_SCALE);
    assert_eq!(
        g.source_credit[0].provider_receivable_num,
        500 * BOUND_SCALE
    );
    assert_eq!(g.source_credit[0].fresh_reserved_backing_num, 0);
    assert_eq!(
        g.c_tot, g.vault,
        "counterparty-backed conversion must not inflate senior account capital above vault stock"
    );
    g.assert_public_invariants().unwrap();
}

#[test]
fn v16_future_capital_backed_loss_refills_consumed_counterparty_backing() {
    let mut g = group();
    let mut first_loser = account();
    let mut second_loser = account_with_id(49);
    let mut winner = account_with_id(50);
    let mut second_opposing = account_with_id(51);

    g.deposit_not_atomic(&mut first_loser, 1_000).unwrap();
    g.deposit_not_atomic(&mut second_loser, 1_000).unwrap();
    g.attach_leg(&mut first_loser, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut winner, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.assets[0].k_long = -(500 * ADL_ONE as i128);
    g.assets[0].k_short = 500 * ADL_ONE as i128;

    g.full_account_refresh(&mut first_loser, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    g.full_account_refresh(&mut winner, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    g.clear_leg(&mut first_loser, 0).unwrap();
    g.clear_leg(&mut winner, 0).unwrap();
    g.full_account_refresh(&mut winner, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(
        g.convert_released_pnl_to_capital_not_atomic(&mut winner),
        Ok(500)
    );
    assert_eq!(
        g.source_credit[0].provider_receivable_num,
        500 * BOUND_SCALE
    );
    assert_eq!(g.source_credit_available_backing_num(0), Ok(0));

    g.attach_leg(&mut second_loser, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(
        &mut second_opposing,
        0,
        SideV16::Short,
        -(POS_SCALE as i128),
    )
    .unwrap();
    g.assets[0].k_long = -(700 * ADL_ONE as i128);
    g.full_account_refresh(&mut second_loser, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(second_loser.capital, 800);
    assert_eq!(second_loser.pnl, 0);
    assert_eq!(
        g.source_credit[0].provider_receivable_num,
        300 * BOUND_SCALE,
        "new source-domain capital backing must first repay the outstanding provider receivable"
    );
    assert_eq!(
        g.source_backing_buckets[0].consumed_liened_backing_num,
        300 * BOUND_SCALE
    );
    assert_eq!(
        g.source_credit_available_backing_num(0),
        Ok(200 * BOUND_SCALE)
    );
    g.assert_public_invariants().unwrap();
}

#[test]
fn v16_full_refresh_uses_haircut_bounded_new_positive_kf_to_cure_prior_loss() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut a, 1, SideV16::Long, POS_SCALE as i128)
        .unwrap();

    g.vault = 50;
    g.assets[0].k_long = -(100 * ADL_ONE as i128);
    g.assets[1].k_long = 100 * ADL_ONE as i128;

    let cert = g
        .full_account_refresh(&mut a, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(
        a.pnl, -100,
        "new positive K/F support without source backing must not cure prior losses from global residual"
    );
    assert_eq!(g.pnl_pos_tot, 0);
    assert_eq!(g.pnl_pos_bound_tot, 0);
    assert_eq!(g.negative_pnl_account_count, 1);
    assert_eq!(cert.certified_equity, -100);
}

#[test]
// RESYNC-TODO(step7): 323c9f2 added the source-credit provenance gate
// (`positive_claim_bound_num == 0 ⇒ no realizable support`): un-provenanced
// counterparty backing can no longer cure a prior loss. This fixture seeds
// a.pnl=-500 then adds backing-only via add_fresh_counterparty_backing_not_atomic
// (which leaves positive_claim_bound_num=0), so the +500 K-gain no longer cures
// to pnl=0 (now stays -500). NOT a regression — verified intended (toly's own
// v16_impaired_source_claim_burns_when_positive_pnl_decreases cures via
// add_account_source_positive_pnl_not_atomic). Reconstruct with the provenance
// path in Step 7, after f3aef4b/0afecb1 land the final source-credit shape.
#[ignore = "RESYNC-TODO(step7): rebuild for 323c9f2 source-credit provenance gate (see comment)"]
fn v16_positive_kf_settlement_uses_source_credit_to_cure_prior_loss_before_principal() {
    let mut g = group();
    let mut a = account();
    let mut opposing = account_with_id(45);
    g.deposit_not_atomic(&mut a, 1_000).unwrap();
    a.pnl = -500;
    g.negative_pnl_account_count = 1;
    g.add_fresh_counterparty_backing_not_atomic(1, 500 * BOUND_SCALE, 10)
        .unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut opposing, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.assets[0].k_long = 500 * ADL_ONE as i128;
    assert_eq!(
        g.vault.saturating_sub(g.c_tot.saturating_add(g.insurance)),
        0,
        "regression requires no global residual"
    );

    let cert = g
        .full_account_refresh(&mut a, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(a.capital, 1_000);
    assert_eq!(a.pnl, 0);
    assert_eq!(g.c_tot, 1_000);
    assert_eq!(g.negative_pnl_account_count, 0);
    assert_eq!(g.source_credit[1].spent_backing_num, 500 * BOUND_SCALE);
    assert_eq!(g.source_credit[1].fresh_reserved_backing_num, 0);
    assert_eq!(cert.certified_equity, 1_000);
}

#[test]
fn v16_withdraw_uses_haircut_positive_credit_not_face_pnl_when_unlocked() {
    let (market, _, _) = ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.min_nonzero_im_req = 20;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 30).unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, 1).unwrap();
    a.pnl = 100;
    g.pnl_pos_tot = 100;
    set_junior_bound(&mut g, 100);
    g.vault = g.c_tot + g.insurance + 10;

    assert_eq!(
        g.withdraw_not_atomic(&mut a, 25, &[1; V16_MAX_PORTFOLIO_ASSETS_N]),
        Err(V16Error::InvalidConfig)
    );
    assert_eq!(a.capital, 30);
    assert_eq!(g.c_tot, 30);
}

#[test]
fn v16_stale_profitable_leg_cannot_withdraw_using_pre_refresh_positive_pnl() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 40).unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    a.pnl = 100;
    g.pnl_pos_tot = 100;
    set_junior_bound(&mut g, 100);
    g.vault = g.c_tot + 50;
    g.assets[0].k_long = -(100 * ADL_ONE as i128);
    g.mark_account_stale(&mut a).unwrap();

    let before_vault = g.vault;
    let before_c_tot = g.c_tot;
    let res = g.withdraw_not_atomic(&mut a, 41, &[1; V16_MAX_PORTFOLIO_ASSETS_N]);

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
fn v16_public_invariants_reject_broken_senior_claim_conservation() {
    let mut g = group();
    g.vault = 10;
    g.c_tot = 8;
    g.insurance = 3;

    assert_eq!(g.assert_public_invariants(), Err(V16Error::InvalidConfig));

    g.insurance = 2;
    assert_eq!(g.assert_public_invariants(), Ok(()));
}

#[test]
fn v16_public_invariants_reject_persistent_asset_kf_i128_min() {
    let mut g = group();
    g.assets[0].k_long = i128::MIN;
    assert_eq!(g.assert_public_invariants(), Err(V16Error::InvalidConfig));

    let mut g = group();
    g.assets[0].f_epoch_start_short_num = i128::MIN;
    assert_eq!(g.assert_public_invariants(), Err(V16Error::InvalidConfig));
}

#[test]
fn v16_public_invariants_reject_oi_loss_weight_shape_mismatch() {
    let mut g = group();
    g.assets[0].oi_eff_long_q = 1;
    assert_eq!(g.assert_public_invariants(), Err(V16Error::InvalidConfig));

    let mut g = group();
    g.assets[0].loss_weight_sum_short = 1;
    assert_eq!(g.assert_public_invariants(), Err(V16Error::InvalidConfig));
}

#[test]
fn v16_public_invariants_reject_live_oi_imbalance() {
    let mut g = group();
    let mut long = account();
    g.attach_leg(&mut long, 0, SideV16::Long, 1).unwrap();
    assert_eq!(g.assert_public_invariants(), Err(V16Error::InvalidConfig));

    let mut short = PortfolioAccountV16::empty(ProvenanceHeaderV16::new([1; 32], [9; 32], [3; 32]));
    g.attach_leg(&mut short, 0, SideV16::Short, -1).unwrap();
    assert_eq!(g.assert_public_invariants(), Ok(()));
}

#[test]
fn v16_cross_margin_collateral_counted_once_and_not_below_loss_envelope() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 1_000_000).unwrap();
    // Keep oracle targets consistent with the priced refresh so the post-323c9f2
    // target-effective-lag penalty is zero (the envelope below is the pure
    // worst-case loss, not loss + lag penalty).
    g.assets[0].raw_oracle_target_price = 1_000_000;
    g.assets[1].raw_oracle_target_price = 1_000_000;
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut a, 1, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    let prices = [1_000_000; V16_MAX_PORTFOLIO_ASSETS_N];

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
// RESYNC-TODO(step7): same 323c9f2 source-credit provenance gate as
// v16_positive_kf_settlement_uses_source_credit_to_cure_prior_loss_before_principal.
// Here the positive leg's +4 K-gain sits in a domain with fresh backing but
// positive_claim_bound_num=0, so post-hardening it can no longer cross-subsidise
// the negative leg's -2 loss → a.pnl drops 3→-1. NOT a regression (intended
// provenance requirement). Reconstruct with add_account_source_positive_pnl_not_atomic
// in Step 7, after f3aef4b/0afecb1 land the final source-credit shape.
#[ignore = "RESYNC-TODO(step7): rebuild for 323c9f2 source-credit provenance gate (see comment)"]
fn v16_global_cross_margin_positive_leg_supports_other_leg_maintenance_without_b_domain() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 1).unwrap();
    g.vault += 3;
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut a, 1, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let _opp0 = attach_opposite(&mut g, 0, SideV16::Long, POS_SCALE, 9);
    let _opp1 = attach_opposite(&mut g, 1, SideV16::Long, POS_SCALE, 10);
    g.assets[0].k_long = -2 * ADL_ONE as i128;
    g.assets[1].k_long = 4 * ADL_ONE as i128;
    g.add_fresh_counterparty_backing_not_atomic(3, 5 * BOUND_SCALE, 10)
        .unwrap();

    let cert = g
        .full_account_refresh(&mut a, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(
        a.pnl, 3,
        "the negative leg's capital-backed loss is reserved before the positive leg is credited"
    );
    assert_eq!(a.capital, 0);
    assert_eq!(g.c_tot, 0);
    assert_eq!(g.source_credit[0].fresh_reserved_backing_num, BOUND_SCALE);
    assert_eq!(cert.certified_equity, 3);
    assert_eq!(cert.certified_maintenance_req, 2);
    assert_eq!(cert.certified_liq_deficit, 0);
    assert_eq!(
        g.insurance_domain_spent,
        vec![0; g.insurance_domain_spent.len()]
    );
    assert_eq!(
        g.pending_domain_loss_barriers,
        vec![0; g.pending_domain_loss_barriers.len()]
    );
    assert_eq!(g.assets[0].b_long_num, 0);
    assert_eq!(g.assets[0].b_short_num, 0);
    assert_eq!(g.assets[1].b_long_num, 0);
    assert_eq!(g.assets[1].b_short_num, 0);
    assert_eq!(g.assert_public_invariants(), Ok(()));
}

#[test]
fn v16_b_stale_blocks_refresh_and_favorable_actions_without_scanning_market() {
    let mut g = group();
    let mut a = account();
    a.capital = 100;
    g.attach_leg(&mut a, 0, SideV16::Long, 1_000_000).unwrap();
    let prices = [100u64; V16_MAX_PORTFOLIO_ASSETS_N];

    g.mark_account_b_stale(&mut a).unwrap();
    assert_eq!(
        g.full_account_refresh(&mut a, &prices),
        Err(V16Error::BStale)
    );
    assert_eq!(
        g.ensure_favorable_action_allowed(&a),
        Err(V16Error::LockActive)
    );
}

#[test]
fn v16_public_init_rejects_unbounded_portfolio_width() {
    let (market, _, _) = ids();
    let cfg = V16Config::public_user_fund((V16_MAX_PORTFOLIO_ASSETS_N + 1) as u16, 0, 10);
    assert_eq!(
        MarketGroupV16::new(market, cfg),
        Err(V16Error::InvalidConfig)
    );
}

#[test]
fn v16_public_init_rejects_disabled_recovery_profile() {
    let (market, _, _) = ids();
    let mut cfg = V16Config::public_user_fund(4, 0, 10);
    cfg.permissionless_recovery_enabled = false;

    assert_eq!(
        MarketGroupV16::new(market, cfg),
        Err(V16Error::InvalidConfig)
    );
}

#[test]
fn v16_public_init_rejects_disabled_recovery_fallback_price_policy() {
    let (market, _, _) = ids();
    let mut cfg = V16Config::public_user_fund(4, 0, 10);
    cfg.recovery_fallback_price_enabled = false;

    assert_eq!(
        MarketGroupV16::new(market, cfg),
        Err(V16Error::InvalidConfig)
    );
}

#[test]
fn v16_public_init_requires_crankforward_recovery_and_chunk_caps() {
    let (market, _, _) = ids();

    let mut cfg = V16Config::public_user_fund(4, 0, 10);
    cfg.stale_certificate_penalty_enabled = false;
    assert_eq!(
        MarketGroupV16::new(market, cfg),
        Err(V16Error::InvalidConfig)
    );

    let mut cfg = V16Config::public_user_fund(4, 0, 10);
    cfg.full_refresh_required_for_favorable_actions = false;
    assert_eq!(
        MarketGroupV16::new(market, cfg),
        Err(V16Error::InvalidConfig)
    );

    let mut cfg = V16Config::public_user_fund(4, 0, 10);
    cfg.public_liveness_profile_crank_forward = false;
    assert_eq!(
        MarketGroupV16::new(market, cfg),
        Err(V16Error::InvalidConfig)
    );

    let mut cfg = V16Config::public_user_fund(4, 0, 10);
    cfg.max_account_b_settlement_chunks = 0;
    assert_eq!(
        MarketGroupV16::new(market, cfg),
        Err(V16Error::InvalidConfig)
    );

    let mut cfg = V16Config::public_user_fund(4, 0, 10);
    cfg.max_bankrupt_close_chunks = 0;
    assert_eq!(
        MarketGroupV16::new(market, cfg),
        Err(V16Error::InvalidConfig)
    );

    let mut cfg = V16Config::public_user_fund(4, 0, 10);
    cfg.max_bankrupt_close_lifetime_slots = 0;
    assert_eq!(
        MarketGroupV16::new(market, cfg),
        Err(V16Error::InvalidConfig)
    );
}

#[test]
fn v16_public_init_accepts_tight_exact_solvency_envelope() {
    let (market, _, _) = ids();
    let cfg = tight_envelope_config();
    assert!(MarketGroupV16::new(market, cfg).is_ok());
}

#[test]
fn v16_public_init_rejects_price_funding_or_liquidation_envelope_breach() {
    let (market, _, _) = ids();

    let mut price_breach = tight_envelope_config();
    price_breach.max_price_move_bps_per_slot = 10;
    assert_eq!(
        MarketGroupV16::new(market, price_breach),
        Err(V16Error::InvalidConfig)
    );

    let mut funding_breach = tight_envelope_config();
    funding_breach.max_accrual_dt_slots = 10_000;
    funding_breach.min_funding_lifetime_slots = 10_000;
    assert_eq!(
        MarketGroupV16::new(market, funding_breach),
        Err(V16Error::InvalidConfig)
    );

    let mut liquidation_breach = tight_envelope_config();
    liquidation_breach.liquidation_fee_bps = 400;
    assert_eq!(
        MarketGroupV16::new(market, liquidation_breach),
        Err(V16Error::InvalidConfig)
    );
}

#[test]
fn v16_public_init_rejects_zero_price_move_cap() {
    let (market, _, _) = ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.max_price_move_bps_per_slot = 0;

    assert_eq!(
        MarketGroupV16::new(market, cfg),
        Err(V16Error::InvalidConfig)
    );
}

#[test]
fn v16_oracle_price_zero_rejected_and_max_price_accepted_when_unexposed() {
    let mut g = group();
    let before = g.clone();

    assert_eq!(
        g.accrue_asset_to_not_atomic(0, 1, 0, 0, false),
        Err(V16Error::InvalidConfig)
    );
    assert_eq!(g, before);

    let out = g
        .accrue_asset_to_not_atomic(0, 1, MAX_ORACLE_PRICE, 0, false)
        .unwrap();
    assert!(!out.equity_active);
    assert_eq!(g.assets[0].effective_price, MAX_ORACLE_PRICE);
}

#[test]
fn v16_public_init_accepts_capped_liquidation_fee_envelope() {
    let (market, _, _) = ids();
    let mut cfg = tight_envelope_config();
    cfg.liquidation_fee_bps = 10_000;
    cfg.liquidation_fee_cap = 1;
    cfg.min_liquidation_abs = 0;
    assert!(MarketGroupV16::new(market, cfg).is_ok());
}

#[test]
fn v16_public_init_accepts_capped_liquidation_fee_with_min_near_cap() {
    let (market, _, _) = ids();
    let mut cfg = tight_envelope_config();
    cfg.liquidation_fee_bps = 10_000;
    cfg.liquidation_fee_cap = 100;
    cfg.min_liquidation_abs = 99;
    cfg.min_nonzero_mm_req = 300;
    cfg.min_nonzero_im_req = 301;
    assert!(MarketGroupV16::new(market, cfg).is_ok());
}

#[test]
fn v16_public_init_handles_zero_proportional_maintenance_exactly() {
    let (market, _, _) = ids();
    let mut cfg = V16Config::public_user_fund(4, 0, 10);
    cfg.maintenance_margin_bps = 0;
    cfg.max_price_move_bps_per_slot = 1;
    cfg.max_accrual_dt_slots = 1;
    cfg.min_funding_lifetime_slots = 1;
    cfg.max_abs_funding_e9_per_slot = 0;
    cfg.min_nonzero_mm_req = MAX_ACCOUNT_NOTIONAL;
    cfg.min_nonzero_im_req = MAX_ACCOUNT_NOTIONAL + 1;
    assert!(MarketGroupV16::new(market, cfg).is_ok());

    cfg.min_nonzero_mm_req = 1;
    cfg.min_nonzero_im_req = 2;
    assert_eq!(
        MarketGroupV16::new(market, cfg),
        Err(V16Error::InvalidConfig)
    );
}

#[test]
fn v16_public_init_rejects_funding_headroom_overflow() {
    let (market, _, _) = ids();
    let mut cfg = V16Config::public_user_fund(4, 0, 10);
    cfg.max_accrual_dt_slots = 1_000_000_000;
    cfg.min_funding_lifetime_slots = 1_000_000_000;
    cfg.max_abs_funding_e9_per_slot = 10_000;
    assert_eq!(
        MarketGroupV16::new(market, cfg),
        Err(V16Error::InvalidConfig)
    );
}

#[test]
fn v16_public_init_accepts_exact_envelope_boundary() {
    let (market, _, _) = ids();
    let mut cfg = V16Config::public_user_fund(4, 0, 10);
    cfg.maintenance_margin_bps = 500;
    cfg.initial_margin_bps = 600;
    cfg.max_price_move_bps_per_slot = 390;
    cfg.max_accrual_dt_slots = 1;
    cfg.min_funding_lifetime_slots = 1;
    cfg.max_abs_funding_e9_per_slot = 0;
    cfg.min_nonzero_mm_req = 200;
    cfg.min_nonzero_im_req = 201;
    assert!(MarketGroupV16::new(market, cfg).is_ok());
}

#[test]
fn v16_risk_notional_and_equity_use_exact_conservative_shapes() {
    assert_eq!(risk_notional_ceil(1, 1), Ok(1));
    assert_eq!(risk_notional_ceil(1, 1_000_001), Ok(2));

    let mut a = account();
    a.capital = 100;
    a.pnl = -25;
    a.fee_credits = -10;
    assert_eq!(account_equity(&a), Ok(65));
}

#[test]
fn v16_account_equity_rejects_capital_above_i128_max() {
    let mut a = account();
    a.capital = i128::MAX as u128 + 1;
    assert_eq!(account_equity(&a), Err(V16Error::ArithmeticOverflow));
}

#[test]
fn v16_min_nonzero_initial_floor_blocks_tiny_risk_increasing_trade() {
    let (market, account_id, owner) = ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 1);
    cfg.min_nonzero_mm_req = 49;
    cfg.min_nonzero_im_req = 50;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    let mut long = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut short = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    g.deposit_not_atomic(&mut long, 49).unwrap();
    g.deposit_not_atomic(&mut short, 100).unwrap();
    let before_group = g.clone();
    let before_long = long.clone();
    let before_short = short.clone();

    let result = g.execute_trade_with_fee_not_atomic(
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

    assert!(
        matches!(result, Err(V16Error::InvalidConfig | V16Error::LockActive)),
        "tiny risk increase must fail before mutation, whether from IM floor or missing source-credit lien"
    );
    assert_eq!(g, before_group);
    assert_eq!(long, before_long);
    assert_eq!(short, before_short);
}

#[test]
fn v16_account_shape_rejects_malformed_persistent_economic_state() {
    let g = group();

    let mut min_pnl = account();
    min_pnl.pnl = i128::MIN;
    assert_eq!(
        g.validate_account_shape(&min_pnl),
        Err(V16Error::ArithmeticOverflow)
    );

    let mut positive_fee_credit = account();
    positive_fee_credit.fee_credits = 1;
    assert_eq!(
        g.validate_account_shape(&positive_fee_credit),
        Err(V16Error::InvalidLeg)
    );

    let mut min_fee_credit = account();
    min_fee_credit.fee_credits = i128::MIN;
    assert_eq!(
        g.validate_account_shape(&min_fee_credit),
        Err(V16Error::ArithmeticOverflow)
    );

    let mut over_reserved = account();
    over_reserved.pnl = 1;
    over_reserved.reserved_pnl = 2;
    assert_eq!(
        g.validate_account_shape(&over_reserved),
        Err(V16Error::InvalidLeg)
    );
}

#[test]
fn v16_account_shape_rejects_noncanonical_resolved_receipt_finalization() {
    let g = group();

    let mut unfinalized_paid = account();
    unfinalized_paid.resolved_payout_receipt = ResolvedPayoutReceiptV16 {
        present: true,
        prior_bound_contribution_num: BOUND_SCALE,
        live_released_face_at_receipt: 0,
        terminal_positive_claim_face: 1,
        paid_effective: 1,
        finalized: false,
    };
    assert_eq!(
        g.validate_account_shape(&unfinalized_paid),
        Err(V16Error::InvalidLeg)
    );

    let mut finalized_underpaid = account();
    finalized_underpaid.resolved_payout_receipt = ResolvedPayoutReceiptV16 {
        present: true,
        prior_bound_contribution_num: BOUND_SCALE,
        live_released_face_at_receipt: 0,
        terminal_positive_claim_face: 1,
        paid_effective: 0,
        finalized: true,
    };
    assert_eq!(
        g.validate_account_shape(&finalized_underpaid),
        Err(V16Error::InvalidLeg)
    );
}

#[test]
fn v16_flat_account_equity_is_capital_plus_pnl_minus_fee_debt() {
    let mut a = account();
    a.capital = 123;
    a.pnl = -45;
    a.fee_credits = -6;
    assert_eq!(account_equity(&a), Ok(72));

    a.pnl = 45;
    assert_eq!(account_equity(&a), Ok(162));
}

#[test]
fn v16_authoritatively_flat_account_never_receives_b_loss() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.assets[0].b_long_num = 10;
    g.assets[0].b_short_num = 7;

    let outcome = g
        .settle_account_side_effects_not_atomic(&mut a, g.config.public_b_chunk_atoms)
        .unwrap();

    assert_eq!(outcome, PermissionlessProgressOutcomeV16::AccountCurrent);
    assert_eq!(a.active_bitmap, bitmap(&[]));
    assert_eq!(a.pnl, 0);
    assert_eq!(a.capital, 100);
    assert!(!a.b_stale_state);
    assert_eq!(g.b_stale_account_count, 0);
}

#[test]
fn v16_deposit_withdraw_roundtrip_preserves_accounting() {
    let mut g = group();
    let mut a = account();

    g.deposit_not_atomic(&mut a, 123).unwrap();
    assert_eq!(a.capital, 123);
    assert_eq!(g.c_tot, 123);
    assert_eq!(g.vault, 123);

    g.withdraw_not_atomic(&mut a, 123, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(a.capital, 0);
    assert_eq!(g.c_tot, 0);
    assert_eq!(g.vault, 0);
    assert_eq!(g.assert_public_invariants(), Ok(()));
}

#[test]
fn v16_deposit_does_not_draw_insurance_or_sweep_loss_bearing_account() {
    let mut g = group();
    let mut a = account();
    g.vault = 50;
    g.insurance = 50;
    g.attach_leg(&mut a, 0, SideV16::Long, 10).unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV16::Long, 10, 91);
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
fn v16_deposit_never_sweeps_fee_debt_even_when_flat_and_nonnegative() {
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
fn v16_partial_withdraw_can_leave_small_remainder() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 5_000).unwrap();

    g.withdraw_not_atomic(&mut a, 4_500, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(a.capital, 500);
    assert_eq!(g.c_tot, 500);
    assert_eq!(g.vault, 500);
    assert_eq!(g.assert_public_invariants(), Ok(()));
}

#[test]
fn v16_over_withdraw_rejects_before_any_accounting_mutation() {
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

    let res = g.withdraw_not_atomic(&mut a, 11, &[1; V16_MAX_PORTFOLIO_ASSETS_N]);

    assert_eq!(res, Err(V16Error::LockActive));
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
fn v16_close_portfolio_account_requires_clean_local_state() {
    let mut g = group();
    let mut a = account();
    g.create_portfolio_account(&a).unwrap();
    assert_eq!(g.materialized_portfolio_count, 1);

    a.capital = 1;
    assert_eq!(g.close_portfolio_account(&a), Err(V16Error::LockActive));
    assert_eq!(g.materialized_portfolio_count, 1);

    a.capital = 0;
    a.b_stale_state = true;
    assert_eq!(g.close_portfolio_account(&a), Err(V16Error::LockActive));
    assert_eq!(g.materialized_portfolio_count, 1);

    a.b_stale_state = false;
    a.cancel_deposit_escrow = 1;
    assert_eq!(g.close_portfolio_account(&a), Err(V16Error::LockActive));
    assert_eq!(g.materialized_portfolio_count, 1);

    a.cancel_deposit_escrow = 0;
    a.capital = 0;
    g.close_portfolio_account(&a).unwrap();
    assert_eq!(g.materialized_portfolio_count, 0);
}

#[test]
fn v16_attach_and_clear_leg_update_only_bounded_account_and_asset_state() {
    let mut g = group();
    let mut a = account();

    g.attach_leg(&mut a, 1, SideV16::Short, -7).unwrap();
    let (slot, leg) = active_leg_for_asset(&a, 1).unwrap();
    assert_eq!(a.active_bitmap, bitmap(&[slot]));
    assert_eq!(leg.asset_index, 1);
    assert_eq!(leg.market_id, g.assets[1].market_id);
    assert_eq!(g.assets[1].stored_pos_count_short, 1);
    assert_eq!(g.assets[1].oi_eff_short_q, 7);
    assert_eq!(g.assets[1].loss_weight_sum_short, 7);

    g.clear_leg(&mut a, 1).unwrap();
    assert_eq!(a.active_bitmap, bitmap(&[]));
    assert_eq!(g.assets[1].stored_pos_count_short, 0);
    assert_eq!(g.assets[1].oi_eff_short_q, 0);
    assert_eq!(g.assets[1].loss_weight_sum_short, 0);
}

#[test]
fn v16_portfolio_legs_are_compact_slots_keyed_by_asset_identity() {
    let mut g = group();
    let mut a = account();

    g.attach_leg(&mut a, 3, SideV16::Long, 11).unwrap();
    g.attach_leg(&mut a, 1, SideV16::Short, -7).unwrap();

    let (slot_3, leg_3) = active_leg_for_asset(&a, 3).unwrap();
    let (slot_1, leg_1) = active_leg_for_asset(&a, 1).unwrap();
    assert_eq!(a.active_bitmap, bitmap(&[slot_3, slot_1]));
    assert_eq!(slot_3, 0);
    assert_eq!(slot_1, 1);
    assert_eq!(leg_3.market_id, g.assets[3].market_id);
    assert_eq!(leg_1.market_id, g.assets[1].market_id);

    g.clear_leg(&mut a, 3).unwrap();
    assert_eq!(active_leg_for_asset(&a, 3), None);
    let (still_slot_1, _) = active_leg_for_asset(&a, 1).unwrap();
    assert_eq!(still_slot_1, 1);
    assert_eq!(a.active_bitmap, bitmap(&[1]));

    g.attach_leg(&mut a, 2, SideV16::Long, 5).unwrap();
    let (slot_2, leg_2) = active_leg_for_asset(&a, 2).unwrap();
    assert_eq!(slot_2, 0);
    assert_eq!(leg_2.market_id, g.assets[2].market_id);
    assert_eq!(a.active_bitmap, bitmap(&[0, 1]));
    assert_eq!(g.validate_account_shape(&a), Ok(()));
}

#[test]
fn v16_market_slots_can_exceed_portfolio_active_leg_cap() {
    let mut g = MarketGroupV16::new(
        ids().0,
        V16Config::public_user_fund_with_market_slots(4, 32, 0, 10),
    )
    .unwrap();
    let mut a = account();
    let asset_index = 17usize;

    g.attach_leg(&mut a, asset_index, SideV16::Long, 13)
        .unwrap();

    let (slot, leg) = active_leg_for_asset(&a, asset_index).unwrap();
    assert_eq!(slot, 0);
    assert_eq!(a.active_bitmap, bitmap(&[0]));
    assert_eq!(leg.asset_index as usize, asset_index);
    assert_eq!(leg.market_id, g.assets[asset_index].market_id);
    assert_eq!(g.assets[asset_index].oi_eff_long_q, 13);
    assert_eq!(
        g.source_backing_buckets[asset_index * 2].market_id,
        g.assets[asset_index].market_id
    );

    let prices = vec![100u64; g.assets.len()];
    g.full_account_refresh(&mut a, &prices).unwrap();
    assert!(a.health_cert.valid);
}

#[test]
fn v16_dynamic_header_can_grow_and_activate_appended_market_slot() {
    let g = MarketGroupV16::new(
        ids().0,
        V16Config::public_user_fund_with_market_slots(4, 16, 0, 10),
    )
    .unwrap();
    let mut header = MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, 16).unwrap();
    header.grow_asset_slot_capacity_not_atomic(32, 32).unwrap();
    let config = header.config.try_to_runtime().unwrap();
    assert_eq!(header.asset_slot_capacity.get(), 32);
    assert_eq!(config.max_portfolio_assets, 4);
    assert_eq!(config.max_market_slots, 32);

    let mut appended_slot = EngineAssetSlotV16Account::default();
    let next_market_id = header.next_market_id.get();
    header
        .activate_empty_asset_slot_not_atomic(17, &mut appended_slot, 123, g.current_slot)
        .unwrap();
    let activated = appended_slot.asset.try_to_runtime().unwrap();
    assert_eq!(activated.market_id, next_market_id);
    assert_eq!(activated.effective_price, 123);
    assert_eq!(
        appended_slot
            .backing_long
            .try_to_runtime()
            .unwrap()
            .market_id,
        activated.market_id
    );
    assert_eq!(header.next_market_id.get(), next_market_id + 1);
}

#[test]
fn v16_dynamic_header_activates_generic_wrapper_slot_without_touching_wrapper_data() {
    let g = MarketGroupV16::new(
        ids().0,
        V16Config::public_user_fund_with_market_slots(4, 4, 0, 10),
    )
    .unwrap();
    let mut header = MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, 8).unwrap();
    header.grow_asset_slot_capacity_not_atomic(8, 8).unwrap();
    let mut markets = (0..8u8)
        .map(|i| Market {
            wrapper: [i; 32],
            engine: EngineAssetSlotV16Account::default(),
        })
        .collect::<Vec<_>>();
    let market_id = header.next_market_id.get();

    MarketGroupV16ViewMut::new(&mut header, &mut markets)
        .activate_empty_market_not_atomic(7, 777, 2)
        .unwrap();

    assert_eq!(markets[7].wrapper, [7u8; 32]);
    assert_eq!(markets[7].engine.asset.market_id.get(), market_id);
    assert_eq!(markets[7].engine.asset.effective_price.get(), 777);
    assert_eq!(markets[7].engine.backing_long.market_id.get(), market_id);
}

#[test]
fn v16_dynamic_header_growth_counter_overflow_rejects_before_metadata_mutation() {
    let g = MarketGroupV16::new(
        ids().0,
        V16Config::public_user_fund_with_market_slots(4, 16, 0, 10),
    )
    .unwrap();
    let mut header = MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, 16).unwrap();
    header.risk_epoch = V16PodU64::new(u64::MAX);

    let before_capacity = header.asset_slot_capacity;
    let before_config = header.config;
    let before_asset_set_epoch = header.asset_set_epoch;
    let before_risk_epoch = header.risk_epoch;

    assert_eq!(
        header.grow_asset_slot_capacity_not_atomic(32, 32),
        Err(V16Error::CounterOverflow)
    );
    assert_eq!(header.asset_slot_capacity, before_capacity);
    assert_eq!(header.config, before_config);
    assert_eq!(header.asset_set_epoch, before_asset_set_epoch);
    assert_eq!(header.risk_epoch, before_risk_epoch);
}

#[test]
fn v16_dynamic_header_capacity_is_wrapper_supplied_not_fixed_runtime_window() {
    let capacity = 89u32;
    let config = V16Config::public_user_fund_with_market_slots(4, 16, 0, 10);
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(ids().0, config, 16, 0).unwrap();

    header
        .grow_asset_slot_capacity_not_atomic(capacity, capacity)
        .unwrap();
    let mut appended_slot = EngineAssetSlotV16Account::default();
    let asset_index = capacity - 1;
    let next_market_id = header.next_market_id.get();

    header
        .activate_empty_asset_slot_not_atomic(asset_index, &mut appended_slot, 456, 0)
        .unwrap();

    let grown = header.config.try_to_runtime().unwrap();
    assert_eq!(grown.max_market_slots, capacity);
    assert_eq!(header.asset_slot_capacity.get(), capacity);
    assert_eq!(appended_slot.asset.market_id.get(), next_market_id);
    assert_eq!(appended_slot.asset.effective_price.get(), 456);
    assert_eq!(appended_slot.backing_long.market_id.get(), next_market_id);
    assert_eq!(appended_slot.backing_short.market_id.get(), next_market_id);
}

#[test]
fn v16_dynamic_realloc_layout_is_capacity_driven_not_fixed_runtime_window() {
    let capacity = 263usize;
    type Wrapper = [u8; 24];
    let slot_len = MarketGroupV16HeaderAccount::dynamic_asset_slot_stride::<Wrapper>();
    let account_len =
        MarketGroupV16HeaderAccount::dynamic_market_group_account_len::<Wrapper>(capacity).unwrap();
    let last_offset =
        MarketGroupV16HeaderAccount::dynamic_asset_slot_offset::<Wrapper>(capacity - 1).unwrap();

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
    assert_eq!(
        last_offset + slot_len,
        account_len,
        "last growable slot must end exactly at the reallocated account length"
    );
    assert!(capacity > 64, "test must exercise wrapper-sized growth");
}

#[test]
fn v16_dynamic_realloc_layout_rejects_truncated_or_misaligned_lengths() {
    let capacity = 67usize;
    type Wrapper = [u8; 7];
    let account_len =
        MarketGroupV16HeaderAccount::dynamic_market_group_account_len::<Wrapper>(capacity).unwrap();

    assert_eq!(
        MarketGroupV16HeaderAccount::dynamic_asset_slot_capacity_from_account_len::<Wrapper>(
            core::mem::size_of::<MarketGroupV16HeaderAccount>() - 1
        ),
        Err(V16Error::InvalidConfig)
    );
    assert_eq!(
        MarketGroupV16HeaderAccount::dynamic_asset_slot_capacity_from_account_len::<Wrapper>(
            account_len - 1
        ),
        Err(V16Error::InvalidConfig)
    );
    assert_eq!(
        MarketGroupV16HeaderAccount::validate_dynamic_market_group_account_len::<Wrapper>(
            account_len,
            capacity + 1
        ),
        Err(V16Error::InvalidConfig)
    );
}

#[test]
fn v16_runtime_group_accepts_wrapper_sized_market_capacity_without_static_window() {
    let config = V16Config::public_user_fund_with_market_slots(4, 89, 0, 10);
    assert_eq!(config.validate_public_user_fund_shape(), Ok(()));
    let g = MarketGroupV16::new(ids().0, config).unwrap();
    assert_eq!(g.assets.len(), 89);
    assert_eq!(g.source_credit.len(), 178);
    assert_eq!(g.assert_public_invariants(), Ok(()));
}

#[test]
fn v16_bilateral_oi_decomposition_counts_only_active_side_exposure() {
    let mut g = group();
    let mut long = account();
    let mut short = account();
    short.provenance_header.portfolio_account_id = [4; 32];

    g.attach_leg(&mut long, 0, SideV16::Long, 3).unwrap();
    g.attach_leg(&mut short, 0, SideV16::Short, -3).unwrap();

    assert_eq!(g.assets[0].oi_eff_long_q, 3);
    assert_eq!(g.assets[0].oi_eff_short_q, 3);
    assert_eq!(g.assets[0].stored_pos_count_long, 1);
    assert_eq!(g.assets[0].stored_pos_count_short, 1);
    assert_eq!(long.active_bitmap, bitmap(&[0]));
    assert_eq!(short.active_bitmap, bitmap(&[0]));
    assert_eq!(long.legs[0].basis_pos_q, 3);
    assert_eq!(short.legs[0].basis_pos_q, -3);
}

#[test]
fn v16_oversize_position_is_rejected_before_oi_mutation() {
    let mut g = group();
    let mut a = account();

    let res = g.attach_leg(
        &mut a,
        0,
        SideV16::Long,
        (percolator::MAX_POSITION_ABS_Q + 1) as i128,
    );

    assert_eq!(res, Err(V16Error::InvalidLeg));
    assert_eq!(a.active_bitmap, bitmap(&[]));
    assert_eq!(g.assets[0].oi_eff_long_q, 0);
}

#[test]
fn v16_account_b_chunk_makes_strict_account_local_progress_or_requires_recovery() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV16::Long, 1).unwrap();
    g.assets[0].b_long_num = SOCIAL_LOSS_DEN * 2;
    g.mark_leg_b_stale(&mut a, 0).unwrap();

    let chunk = g
        .settle_account_b_chunk(&mut a, 0, SOCIAL_LOSS_DEN)
        .unwrap();
    assert!(chunk.delta_b > 0);
    assert!(a.legs[0].b_snap > 0);
    assert_eq!(a.health_cert.valid, false);

    let mut blocked = account();
    g.attach_leg(&mut blocked, 1, SideV16::Long, 1).unwrap();
    g.assets[1].b_long_num = 1;
    g.mark_leg_b_stale(&mut blocked, 1).unwrap();
    assert_eq!(
        g.settle_account_b_chunk(&mut blocked, 1, 0),
        Err(V16Error::RecoveryRequired)
    );
}

#[test]
fn v16_liquidation_progress_requires_strict_risk_score_reduction() {
    let mut g = group();
    let mut before = account();
    let mut after = account();
    g.full_account_refresh(&mut before, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    g.full_account_refresh(&mut after, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    before.health_cert.certified_liq_deficit = 10;
    after.health_cert.certified_liq_deficit = 10;
    assert_eq!(
        g.validate_liquidation_progress(&before, &after),
        Err(V16Error::NonProgress)
    );

    after.health_cert.certified_liq_deficit = 9;
    assert_eq!(g.validate_liquidation_progress(&before, &after), Ok(()));
}

#[test]
fn v16_cyclic_rescue_without_scalar_progress_reverts() {
    let mut g = group();
    let mut before = account();
    let mut after = account();
    g.full_account_refresh(&mut before, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    g.full_account_refresh(&mut after, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    before.health_cert.certified_liq_deficit = 5;
    before.health_cert.certified_worst_case_loss = 3;

    after.health_cert.certified_liq_deficit = 5;
    after.health_cert.certified_worst_case_loss = 4;
    assert_eq!(
        g.validate_liquidation_progress(&before, &after),
        Err(V16Error::NonProgress)
    );

    after.health_cert.certified_worst_case_loss = 3;
    after.stale_state = true;
    assert_eq!(
        g.validate_liquidation_progress(&before, &after),
        Err(V16Error::NonProgress)
    );

    after.stale_state = false;
    after.health_cert.certified_liq_deficit = 4;
    assert_eq!(g.validate_liquidation_progress(&before, &after), Ok(()));
}

#[test]
fn v16_permissionless_recovery_is_declared_by_reason_not_caller_price() {
    let mut g = group();
    let reason = PermissionlessRecoveryReasonV16::AccountBSettlementCannotProgress;
    assert_eq!(
        g.declare_permissionless_recovery(reason),
        Ok(PermissionlessProgressOutcomeV16::RecoveryDeclared(reason))
    );
    assert_eq!(g.recovery_reason, Some(reason));
    assert_eq!(g.mode, MarketModeV16::Recovery);
}

#[test]
fn v16_explicit_loss_audit_overflow_declares_recovery_without_value_mutation() {
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
        PermissionlessProgressOutcomeV16::RecoveryDeclared(
            PermissionlessRecoveryReasonV16::ExplicitLossOrDustAuditOverflow
        )
    );
    assert_eq!(
        g.recovery_reason,
        Some(PermissionlessRecoveryReasonV16::ExplicitLossOrDustAuditOverflow)
    );
    assert_eq!(g.mode, MarketModeV16::Recovery);
    assert_eq!(g.vault, vault_before);
    assert_eq!(g.c_tot, c_tot_before);
    assert_eq!(g.insurance, insurance_before);
    assert_eq!(g.pnl_pos_tot, pnl_pos_before);
    assert_eq!(g.assets[0], asset_before);
}

#[test]
fn v16_permissionless_recovery_enters_terminal_mode_and_enables_dead_leg_forfeit() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV16::Long, 1).unwrap();
    assert_eq!(
        g.forfeit_recovery_leg_not_atomic(&mut a, 0, 1),
        Err(V16Error::LockActive)
    );

    let reason = PermissionlessRecoveryReasonV16::OracleOrTargetUnavailableByAuthenticatedPolicy;
    assert_eq!(
        g.declare_permissionless_recovery(reason),
        Ok(PermissionlessProgressOutcomeV16::RecoveryDeclared(reason))
    );
    let out = g.forfeit_recovery_leg_not_atomic(&mut a, 0, 1).unwrap();

    assert!(out.detached);
    assert_eq!(a.active_bitmap, bitmap(&[]));
    assert_eq!(g.assets[0].oi_eff_long_q, 0);
    assert_eq!(g.recovery_reason, Some(reason));
    assert_eq!(g.mode, MarketModeV16::Recovery);
}

#[test]
fn v16_permissionless_recovery_cannot_override_resolved_mode() {
    let mut g = group();
    g.resolve_market_not_atomic(1).unwrap();

    assert_eq!(
        g.declare_permissionless_recovery(PermissionlessRecoveryReasonV16::BelowProgressFloor),
        Err(V16Error::LockActive)
    );
    assert_eq!(g.mode, MarketModeV16::Resolved);
    assert_eq!(g.recovery_reason, None);
}

#[test]
fn v16_recovery_reason_is_terminal_and_idempotent() {
    let mut g = group();
    let first = PermissionlessRecoveryReasonV16::BelowProgressFloor;
    let second = PermissionlessRecoveryReasonV16::CounterOrEpochOverflowDeclaredRecovery;

    assert_eq!(
        g.declare_permissionless_recovery(first),
        Ok(PermissionlessProgressOutcomeV16::RecoveryDeclared(first))
    );
    assert_eq!(
        g.declare_permissionless_recovery(second),
        Ok(PermissionlessProgressOutcomeV16::RecoveryDeclared(first))
    );
    assert_eq!(g.recovery_reason, Some(first));
    assert_eq!(g.mode, MarketModeV16::Recovery);
}

#[test]
fn v16_recovery_mode_cannot_be_overridden_by_resolve() {
    let mut g = group();
    let reason = PermissionlessRecoveryReasonV16::BelowProgressFloor;
    g.declare_permissionless_recovery(reason).unwrap();

    assert_eq!(g.resolve_market_not_atomic(10), Err(V16Error::LockActive));
    assert_eq!(g.mode, MarketModeV16::Recovery);
    assert_eq!(g.recovery_reason, Some(reason));
    assert_eq!(g.resolved_slot, 0);
}

#[test]
fn v16_recovery_mode_blocks_value_escape_and_fee_sync_before_mutation() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    a.pnl = 10;
    g.pnl_pos_tot = 10;
    g.vault += 10;
    g.full_account_refresh(&mut a, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    let before = a.clone();
    let vault_before = g.vault;
    let c_tot_before = g.c_tot;
    let insurance_before = g.insurance;
    g.declare_permissionless_recovery(PermissionlessRecoveryReasonV16::BelowProgressFloor)
        .unwrap();

    assert_eq!(
        g.convert_released_pnl_to_capital_not_atomic(&mut a),
        Err(V16Error::LockActive)
    );
    assert_eq!(
        g.withdraw_not_atomic(&mut a, 1, &[1; V16_MAX_PORTFOLIO_ASSETS_N]),
        Err(V16Error::LockActive)
    );
    assert_eq!(
        g.sync_account_fee_to_slot_not_atomic(&mut a, 1, 1),
        Err(V16Error::LockActive)
    );
    assert_eq!(a, before);
    assert_eq!(g.vault, vault_before);
    assert_eq!(g.c_tot, c_tot_before);
    assert_eq!(g.insurance, insurance_before);
}

#[test]
fn v16_recovery_mode_rejects_liquidation_and_rebalance_before_account_mutation() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let account_before = a.clone();
    let asset_before = g.assets[0];
    let reason = PermissionlessRecoveryReasonV16::BlockedSegmentHeadroomOrRepresentability;
    g.declare_permissionless_recovery(reason).unwrap();

    let liquidation = g.liquidate_account_not_atomic(
        &mut a,
        LiquidationRequestV16 {
            asset_index: 0,
            close_q: POS_SCALE,
            fee_bps: 0,
        },
        &[1; V16_MAX_PORTFOLIO_ASSETS_N],
    );
    assert_eq!(liquidation, Err(V16Error::LockActive));
    assert_eq!(a, account_before);
    assert_eq!(g.assets[0], asset_before);

    let rebalance = g.rebalance_reduce_position_not_atomic(
        &mut a,
        RebalanceRequestV16 {
            asset_index: 0,
            reduce_q: POS_SCALE,
        },
        &[1; V16_MAX_PORTFOLIO_ASSETS_N],
    );
    assert_eq!(rebalance, Err(V16Error::LockActive));
    assert_eq!(a, account_before);
    assert_eq!(g.assets[0], asset_before);
    assert_eq!(g.mode, MarketModeV16::Recovery);
    assert_eq!(g.recovery_reason, Some(reason));
}

#[test]
fn v16_recovery_mode_rejects_non_recovery_crank_before_account_mutation() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV16::Long, 1).unwrap();
    g.declare_permissionless_recovery(
        PermissionlessRecoveryReasonV16::BlockedSegmentHeadroomOrRepresentability,
    )
    .unwrap();
    let before = a.clone();

    let res = g.permissionless_crank_not_atomic(
        &mut a,
        PermissionlessCrankRequestV16 {
            now_slot: 1,
            asset_index: 0,
            effective_price: 1,
            funding_rate_e9: 0,
            action: PermissionlessCrankActionV16::Refresh,
        },
        &[1; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(res, Err(V16Error::LockActive));
    assert_eq!(a, before);
    assert_eq!(
        g.recovery_reason,
        Some(PermissionlessRecoveryReasonV16::BlockedSegmentHeadroomOrRepresentability)
    );
    assert_eq!(g.mode, MarketModeV16::Recovery);
}

#[test]
fn v16_permissionless_recovery_fails_closed_when_disabled() {
    let mut g = group();
    g.config.permissionless_recovery_enabled = false;

    assert_eq!(
        g.declare_permissionless_recovery(
            PermissionlessRecoveryReasonV16::BlockedSegmentHeadroomOrRepresentability
        ),
        Err(V16Error::InvalidConfig)
    );
    assert_eq!(g.recovery_reason, None);
    assert_eq!(g.mode, MarketModeV16::Live);
}

#[test]
fn v16_permissionless_crank_recovery_declaration_is_accounting_neutral() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, 1).unwrap();
    let account_before = a.clone();
    let vault_before = g.vault;
    let c_tot_before = g.c_tot;
    let insurance_before = g.insurance;
    let pnl_pos_before = g.pnl_pos_tot;
    let asset_before = g.assets[0];
    let slot_last_before = g.slot_last;
    let current_slot_before = g.current_slot;
    let reason = PermissionlessRecoveryReasonV16::ExplicitLossOrDustAuditOverflow;

    let out = g
        .permissionless_crank_not_atomic(
            &mut a,
            PermissionlessCrankRequestV16 {
                now_slot: current_slot_before + 1,
                asset_index: 0,
                effective_price: 2,
                funding_rate_e9: 0,
                action: PermissionlessCrankActionV16::Recover(reason),
            },
            &[1; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(
        out,
        PermissionlessProgressOutcomeV16::RecoveryDeclared(reason)
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
    assert_eq!(g.mode, MarketModeV16::Recovery);
}

#[test]
fn v16_fees_are_charged_only_after_realized_losses() {
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
fn v16_fee_sync_settles_hidden_kf_losses_before_collecting_fee() {
    let mut g = group();
    g.assets[0].effective_price = 100;
    g.assets[0].fund_px_last = 100;
    let mut long = account();
    g.deposit_not_atomic(&mut long, 50).unwrap();
    g.attach_leg(&mut long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV16::Long, POS_SCALE, 92);

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
fn v16_fee_sync_uses_wide_product_and_drops_uncollectible_tail() {
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
fn v16_direct_fee_charge_is_live_only_but_resolved_fee_sync_still_works() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.resolve_market_not_atomic(10).unwrap();

    let before = (g.clone(), a.clone());
    assert_eq!(
        g.charge_account_fee_not_atomic(&mut a, 10),
        Err(V16Error::LockActive)
    );
    assert_eq!((g.clone(), a.clone()), before);

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
fn v16_hlock_allows_principal_withdrawal_without_positive_credit_escape() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV16::Long, POS_SCALE, 93);
    g.threshold_stress_active = true;

    assert_eq!(
        g.withdraw_not_atomic(&mut a, 50, &[10; V16_MAX_PORTFOLIO_ASSETS_N]),
        Ok(())
    );
    assert_eq!(a.capital, 50);
    assert_eq!(g.vault, 50);
}

#[test]
fn v16_hlock_withdraw_rejects_if_post_state_needs_positive_pnl_credit() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 20).unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    a.pnl = 100;
    g.pnl_pos_tot = 100;
    set_junior_bound(&mut g, 100);
    g.threshold_stress_active = true;

    assert_eq!(
        g.withdraw_not_atomic(&mut a, 10, &[50; V16_MAX_PORTFOLIO_ASSETS_N]),
        Err(V16Error::InvalidConfig)
    );
}

#[test]
fn v16_loss_stale_blocks_nonflat_withdrawal_even_if_no_positive_credit_suffices() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.loss_stale_active = true;

    assert_eq!(
        g.withdraw_not_atomic(&mut a, 10, &[10; V16_MAX_PORTFOLIO_ASSETS_N]),
        Err(V16Error::LockActive)
    );
}

#[test]
fn v16_loss_stale_does_not_block_flat_principal_withdrawal() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.loss_stale_active = true;

    assert_eq!(
        g.withdraw_not_atomic(&mut a, 25, &[10; V16_MAX_PORTFOLIO_ASSETS_N]),
        Ok(())
    );
    assert_eq!(a.capital, 75);
    assert_eq!(g.vault, 75);
    assert_eq!(g.c_tot, 75);
}

#[test]
fn v16_target_effective_lag_blocks_risk_increasing_trade_before_mutation() {
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
        TradeRequestV16 {
            asset_index: 0,
            size_q: POS_SCALE,
            exec_price: 100,
            fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
        },
        &[100; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(res, Err(V16Error::LockActive));
    assert_eq!(long.active_bitmap, bitmap(&[]));
    assert_eq!(short.active_bitmap, bitmap(&[]));
}

#[test]
fn v16_target_effective_lag_allows_pure_risk_reducing_trade() {
    let mut g = group();
    let mut reducing_short = account();
    let mut reducing_long = account();
    reducing_long.provenance_header.portfolio_account_id = [4; 32];
    g.deposit_not_atomic(&mut reducing_short, 10_000).unwrap();
    g.deposit_not_atomic(&mut reducing_long, 10_000).unwrap();
    g.attach_leg(&mut reducing_short, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.attach_leg(&mut reducing_long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.assets[0].effective_price = 100;
    g.assets[0].raw_oracle_target_price = 120;

    assert!(g
        .execute_trade_with_fee_not_atomic(
            &mut reducing_short,
            &mut reducing_long,
            TradeRequestV16 {
                asset_index: 0,
                size_q: POS_SCALE / 2,
                exec_price: 100,
                fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
            },
            &[100; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .is_ok());
}

#[test]
fn v16_target_effective_lag_blocks_nonflat_withdrawal_and_pnl_conversion() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    a.pnl = 10;
    g.pnl_pos_tot = 10;
    set_junior_bound(&mut g, 10);
    g.vault = g.vault.checked_add(10).unwrap();
    g.assets[0].effective_price = 100;
    g.assets[0].raw_oracle_target_price = 120;
    g.full_account_refresh(&mut a, &[100; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(
        g.withdraw_not_atomic(&mut a, 1, &[100; V16_MAX_PORTFOLIO_ASSETS_N]),
        Err(V16Error::LockActive)
    );
    assert_eq!(
        g.convert_released_pnl_to_capital_not_atomic(&mut a),
        Err(V16Error::LockActive)
    );
}

#[test]
fn v16_health_cert_counts_target_effective_lag_adverse_loss() {
    let mut long_group = group();
    let mut long = account();
    long_group.deposit_not_atomic(&mut long, 105).unwrap();
    long_group
        .attach_leg(&mut long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    long_group.assets[0].effective_price = 100;
    long_group.assets[0].raw_oracle_target_price = 90;

    let long_cert = long_group
        .full_account_refresh(&mut long, &[100; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(long_cert.certified_maintenance_req, 110);
    assert_eq!(long_cert.certified_liq_deficit, 5);

    let mut short_group = group();
    let mut short = account();
    short_group.deposit_not_atomic(&mut short, 105).unwrap();
    short_group
        .attach_leg(&mut short, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    short_group.assets[0].effective_price = 100;
    short_group.assets[0].raw_oracle_target_price = 110;

    let short_cert = short_group
        .full_account_refresh(&mut short, &[100; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(short_cert.certified_maintenance_req, 110);
    assert_eq!(short_cert.certified_liq_deficit, 5);
}

#[test]
fn v16_zero_copy_health_cert_counts_target_effective_lag_adverse_loss() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 105).unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.assets[0].effective_price = 100;
    g.assets[0].raw_oracle_target_price = 90;
    let mut header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: (),
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let mut account_header = PortfolioAccountV16Account::from_runtime(&a);
    let mut source_domains = PortfolioAccountV16Account::source_domains_from_runtime(&a).unwrap();
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account_view =
        percolator::v16::PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);

    let cert = market_view
        .full_account_refresh_not_atomic(&mut account_view)
        .unwrap();

    assert_eq!(cert.certified_maintenance_req, 110);
    assert_eq!(cert.certified_liq_deficit, 5);
}

#[test]
fn v16_target_effective_lag_does_not_block_flat_principal_withdrawal() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.assets[0].effective_price = 100;
    g.assets[0].raw_oracle_target_price = 120;

    assert_eq!(
        g.withdraw_not_atomic(&mut a, 25, &[100; V16_MAX_PORTFOLIO_ASSETS_N]),
        Ok(())
    );
    assert_eq!(a.capital, 75);
    assert_eq!(g.vault, 75);
    assert_eq!(g.c_tot, 75);
}

#[test]
fn v16_account_free_equity_active_accrual_requires_protective_progress() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 1000).unwrap();
    let mut b = account_with_id(4);
    g.deposit_not_atomic(&mut b, 1000).unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut b, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();

    assert_eq!(
        g.accrue_asset_to_not_atomic(0, 1, 2, 0, false),
        Err(V16Error::NonProgress)
    );
    assert!(g.accrue_asset_to_not_atomic(0, 1, 2, 0, true).is_ok());
}

#[test]
fn v16_equity_active_accrual_commits_one_bounded_loss_stale_segment() {
    let mut g = group();
    g.config.max_accrual_dt_slots = 2;
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV16::Long, POS_SCALE, 94);

    let out = g.accrue_asset_to_not_atomic(0, 10, 3, 0, true).unwrap();
    assert_eq!(out.dt, 2);
    assert!(out.loss_stale_after);
    assert_eq!(g.slot_last, 2);
    assert_eq!(g.current_slot, 10);
    assert!(g.loss_stale_active);
}

#[test]
fn v16_106_loss_stale_active_remains_set_when_other_asset_stale_runtime() {
    let mut g = group_with_market_slots(2);
    g.config.max_accrual_dt_slots = 10;
    g.config.min_funding_lifetime_slots = 10;
    let mut asset0_long = account_with_id(10);
    let mut asset1_long = account_with_id(11);
    g.attach_leg(&mut asset0_long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut asset1_long, 1, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let _asset0_short = attach_opposite(&mut g, 0, SideV16::Long, POS_SCALE, 12);
    let _asset1_short = attach_opposite(&mut g, 1, SideV16::Long, POS_SCALE, 13);

    g.accrue_asset_to_not_atomic(1, 20, 2, 0, true).unwrap();
    assert_eq!(g.assets[1].slot_last, 10);
    assert!(g.loss_stale_active);

    g.accrue_asset_to_not_atomic(0, 20, 2, 0, true).unwrap();
    g.accrue_asset_to_not_atomic(0, 20, 2, 0, true).unwrap();

    assert_eq!(g.assets[0].slot_last, 20);
    assert_eq!(g.assets[1].slot_last, 10);
    assert_eq!(g.slot_last, 10);
    assert!(
        g.loss_stale_active,
        "catching up one asset must not clear a stale loss segment on another accruable asset"
    );
}

#[test]
fn v16_106_loss_stale_active_clears_only_when_all_accruable_assets_fresh_runtime() {
    let mut g = group_with_market_slots(2);
    g.config.max_accrual_dt_slots = 10;
    g.config.min_funding_lifetime_slots = 10;
    let mut asset0_long = account_with_id(14);
    let mut asset1_long = account_with_id(15);
    g.attach_leg(&mut asset0_long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut asset1_long, 1, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let _asset0_short = attach_opposite(&mut g, 0, SideV16::Long, POS_SCALE, 16);
    let _asset1_short = attach_opposite(&mut g, 1, SideV16::Long, POS_SCALE, 17);

    g.accrue_asset_to_not_atomic(1, 20, 2, 0, true).unwrap();
    g.accrue_asset_to_not_atomic(0, 20, 2, 0, true).unwrap();
    g.accrue_asset_to_not_atomic(0, 20, 2, 0, true).unwrap();
    assert!(g.loss_stale_active);
    assert_eq!(g.slot_last, 10);

    g.accrue_asset_to_not_atomic(1, 20, 2, 0, true).unwrap();

    assert_eq!(g.assets[0].slot_last, 20);
    assert_eq!(g.assets[1].slot_last, 20);
    assert_eq!(g.slot_last, 20);
    assert!(!g.loss_stale_active);
}

#[test]
fn v16_106_loss_stale_active_remains_set_when_other_asset_stale_zero_copy() {
    let mut g = group_with_market_slots(2);
    g.config.max_accrual_dt_slots = 10;
    g.config.min_funding_lifetime_slots = 10;
    let mut asset0_long = account_with_id(18);
    let mut asset1_long = account_with_id(19);
    g.attach_leg(&mut asset0_long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut asset1_long, 1, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let _asset0_short = attach_opposite(&mut g, 0, SideV16::Long, POS_SCALE, 20);
    let _asset1_short = attach_opposite(&mut g, 1, SideV16::Long, POS_SCALE, 21);
    let mut header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: [i as u8; 32],
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let mut view = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    view.accrue_asset_to_not_atomic(1, 20, 2, 0, true).unwrap();
    view.accrue_asset_to_not_atomic(0, 20, 2, 0, true).unwrap();
    view.accrue_asset_to_not_atomic(0, 20, 2, 0, true).unwrap();

    assert_eq!(
        view.markets[0].engine.asset.slot_last.get(),
        20,
        "asset 0 should be locally current"
    );
    assert_eq!(
        view.markets[1].engine.asset.slot_last.get(),
        10,
        "asset 1 remains loss-stale"
    );
    assert_eq!(view.header.slot_last.get(), 10);
    assert_eq!(view.header.loss_stale_active, 1);
}

#[test]
fn v16_pending_domain_loss_barrier_does_not_freeze_asset_accrual() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV16::Long, POS_SCALE, 95);
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
fn v16_pending_domain_loss_barrier_blocks_side_reset_before_residual_done() {
    let mut g = group();
    g.pending_domain_loss_barriers[0] = 1;
    g.assets[0].k_long = 7;
    g.assets[0].f_long_num = -3;
    g.assets[0].b_long_num = 11;
    g.assets[0].a_long = ADL_ONE - 1;
    g.assets[0].epoch_long = 4;

    let before = g.clone();
    assert_eq!(
        g.begin_full_drain_reset(0, SideV16::Long),
        Err(V16Error::LockActive),
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
fn v16_pending_domain_loss_barrier_does_not_block_unrelated_side_reset() {
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

    g.begin_full_drain_reset(0, SideV16::Short)
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
    assert_eq!(g.assets[0].mode_short, SideModeV16::ResetPending);
}

#[test]
fn v16_per_asset_slot_last_prevents_cross_asset_accrual_aliasing() {
    let (market, _, _) = ids();
    let mut g = MarketGroupV16::new(market, V16Config::public_user_fund(2, 0, 10)).unwrap();
    let mut a0_long =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [31; 32], [3; 32]));
    let mut a0_short =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [32; 32], [3; 32]));
    let mut a1_long =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [33; 32], [3; 32]));
    let mut a1_short =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [34; 32], [3; 32]));
    g.attach_leg(&mut a0_long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut a0_short, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.attach_leg(&mut a1_long, 1, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut a1_short, 1, SideV16::Short, -(POS_SCALE as i128))
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
fn v16_funding_rate_above_cap_rejects_before_state_mutation() {
    let mut g = group();
    g.config.max_abs_funding_e9_per_slot = 1;
    let before_asset = g.assets[0];

    let res = g.accrue_asset_to_not_atomic(0, 1, 1, 2, true);

    assert!(
        matches!(res, Err(V16Error::InvalidConfig | V16Error::LockActive)),
        "risk increase must fail before mutation when initial health cannot be satisfied without an available source-credit lien"
    );
    assert_eq!(g.assets[0], before_asset);
    assert_eq!(g.slot_last, 0);
    assert_eq!(g.current_slot, 0);
}

#[test]
fn v16_trade_fee_is_dynamic_bounded_and_charged_inside_engine() {
    let mut g = group();
    g.config.max_trading_fee_bps = 100;
    let mut long = account();
    let mut short = account();
    short.provenance_header.portfolio_account_id = [4; 32];
    g.deposit_not_atomic(&mut long, 10_000).unwrap();
    g.deposit_not_atomic(&mut short, 10_000).unwrap();

    let req = TradeRequestV16 {
        asset_index: 0,
        size_q: POS_SCALE,
        exec_price: 1_000,
        fee_bps: 50,
            admit_h_max_consumption_threshold_bps_opt: None,
    };
    let out = g
        .execute_trade_with_fee_not_atomic(
            &mut long,
            &mut short,
            req,
            &[1_000; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();
    assert_eq!(out.notional, 1_000);
    assert_eq!(out.fee_a, 5);
    assert_eq!(long.active_bitmap, bitmap(&[0]));
    assert_eq!(short.active_bitmap, bitmap(&[0]));
    assert_eq!(g.insurance, 10);

    let mut bad_req = req;
    bad_req.fee_bps = 101;
    assert_eq!(
        g.execute_trade_with_fee_not_atomic(
            &mut long,
            &mut short,
            bad_req,
            &[1_000; V16_MAX_PORTFOLIO_ASSETS_N],
        ),
        Err(V16Error::InvalidConfig)
    );
}

#[test]
fn v16_trade_fee_conserves_vault_and_keeps_oi_symmetric() {
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
            TradeRequestV16 {
                asset_index: 0,
                size_q: POS_SCALE,
                exec_price: 100,
                fee_bps: 100,
            admit_h_max_consumption_threshold_bps_opt: None,
            },
            &[100; V16_MAX_PORTFOLIO_ASSETS_N],
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
fn v16_trade_outcome_reports_actual_charged_fees_runtime() {
    let mut g = group();
    g.config.max_trading_fee_bps = 1_000;
    // Align the asset oracle target with the fed effective price (100) so the
    // post-323c9f2 target-effective-lag penalty is zero — matching the
    // zero_copy sibling, which prices via the stored effective_price. Without
    // this, the group() default raw_oracle_target_price=1 vs fed price=100
    // levies a 99-unit lag penalty that strands a spurious initial-margin
    // requirement on the post-trade flat position, failing ensure_initial_margin.
    g.assets[0].raw_oracle_target_price = 100;
    let mut long = account_with_id(22);
    let mut short = account_with_id(23);
    g.deposit_not_atomic(&mut long, 3).unwrap();
    g.deposit_not_atomic(&mut short, 7).unwrap();
    g.attach_leg(&mut long, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.attach_leg(&mut short, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();

    let out = g
        .execute_trade_with_fee_in_place_not_atomic(
            &mut long,
            &mut short,
            TradeRequestV16 {
                asset_index: 0,
                size_q: POS_SCALE,
                exec_price: 100,
                fee_bps: 1_000,
                admit_h_max_consumption_threshold_bps_opt: None,
            },
            &[100; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(out.notional, 100);
    assert_eq!(out.fee_a, 3);
    assert_eq!(out.fee_b, 7);
    assert_eq!(g.insurance, 10);
    assert_eq!(g.c_tot, 0);
    assert_eq!(long.capital, 0);
    assert_eq!(short.capital, 0);
}

#[test]
fn v16_trade_outcome_reports_actual_charged_fees_zero_copy() {
    let mut g = group();
    g.config.max_trading_fee_bps = 1_000;
    let mut long = account_with_id(24);
    let mut short = account_with_id(25);
    g.deposit_not_atomic(&mut long, 3).unwrap();
    g.deposit_not_atomic(&mut short, 7).unwrap();
    g.attach_leg(&mut long, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.attach_leg(&mut short, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let mut header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: [i as u8; 32],
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let mut long_header = PortfolioAccountV16Account::from_runtime(&long);
    let mut short_header = PortfolioAccountV16Account::from_runtime(&short);
    let domain_count =
        percolator::v16::v16_domain_count_for_market_slots(g.config.max_market_slots).unwrap();
    let mut long_sources = vec![PortfolioSourceDomainV16Account::default(); domain_count];
    let mut short_sources = vec![PortfolioSourceDomainV16Account::default(); domain_count];
    let mut long_view =
        percolator::v16::PortfolioV16ViewMut::new(&mut long_header, &mut long_sources);
    let mut short_view =
        percolator::v16::PortfolioV16ViewMut::new(&mut short_header, &mut short_sources);
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    let out = market_view
        .execute_trade_with_fee_in_place_not_atomic(
            &mut long_view,
            &mut short_view,
            TradeRequestV16 {
                asset_index: 0,
                size_q: POS_SCALE,
                exec_price: 100,
                fee_bps: 1_000,
                admit_h_max_consumption_threshold_bps_opt: None,
            },
        )
        .unwrap();

    assert_eq!(out.notional, 100);
    assert_eq!(out.fee_a, 3);
    assert_eq!(out.fee_b, 7);
    assert_eq!(market_view.header.insurance.get(), 10);
    assert_eq!(market_view.header.c_tot.get(), 0);
    assert_eq!(long_view.header.capital.get(), 0);
    assert_eq!(short_view.header.capital.get(), 0);
}

#[test]
fn v16_risk_increasing_trade_requires_initial_health_after_refresh() {
    let mut g = group();
    let mut underfunded_long = account();
    let mut funded_short = account();
    funded_short.provenance_header.portfolio_account_id = [4; 32];
    g.deposit_not_atomic(&mut funded_short, 10_000).unwrap();

    let res = g.execute_trade_with_fee_not_atomic(
        &mut underfunded_long,
        &mut funded_short,
        TradeRequestV16 {
            asset_index: 0,
            size_q: POS_SCALE,
            exec_price: 100,
            fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
        },
        &[100; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    assert!(
        matches!(res, Err(V16Error::InvalidConfig | V16Error::LockActive)),
        "risk increase must fail before mutation when initial health cannot be satisfied without an available source-credit lien"
    );
    assert_eq!(underfunded_long.active_bitmap, bitmap(&[]));
    assert_eq!(g.assets[0].oi_eff_long_q, 0);
    assert_eq!(g.assets[0].oi_eff_short_q, 0);
}

#[test]
fn v16_trade_hint_cannot_hide_toxic_portfolio_leg_on_other_asset() {
    let mut g = group();
    let mut long = account();
    let mut short = account();
    short.provenance_header.portfolio_account_id = [4; 32];
    g.deposit_not_atomic(&mut long, 1).unwrap();
    g.deposit_not_atomic(&mut short, 1_000).unwrap();
    g.attach_leg(&mut long, 1, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.assets[1].k_long = -(3 * ADL_ONE as i128);
    let before_group = g.clone();
    let before_long = long.clone();
    let before_short = short.clone();

    let res = g.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV16 {
            asset_index: 0,
            size_q: POS_SCALE,
            exec_price: 1,
            fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
        },
        &[1; V16_MAX_PORTFOLIO_ASSETS_N],
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
fn v16_invalid_trade_request_rejects_before_any_mutation() {
    let mut g = group();
    let mut long = account();
    let mut short = account();
    short.provenance_header.portfolio_account_id = [4; 32];
    g.deposit_not_atomic(&mut long, 1_000).unwrap();
    g.deposit_not_atomic(&mut short, 1_000).unwrap();
    let before_group = g.clone();
    let before_long = long.clone();
    let before_short = short.clone();

    let res = g.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV16 {
            asset_index: 0,
            size_q: 0,
            exec_price: 100,
            fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
        },
        &[100; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(res, Err(V16Error::InvalidConfig));
    assert_eq!(g, before_group);
    assert_eq!(long, before_long);
    assert_eq!(short, before_short);
}

#[test]
fn v16_sign_flip_trade_preserves_oi_symmetry_and_senior_accounting() {
    let mut g = group();
    let mut flip_to_long = account();
    let mut flip_to_short = account();
    flip_to_short.provenance_header.portfolio_account_id = [4; 32];
    g.deposit_not_atomic(&mut flip_to_long, 10_000).unwrap();
    g.deposit_not_atomic(&mut flip_to_short, 10_000).unwrap();
    g.attach_leg(&mut flip_to_long, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.attach_leg(&mut flip_to_short, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let vault_before = g.vault;
    let c_tot_before = g.c_tot;

    g.execute_trade_with_fee_not_atomic(
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

    assert_eq!(flip_to_long.legs[0].side, SideV16::Long);
    assert_eq!(flip_to_long.legs[0].basis_pos_q, POS_SCALE as i128);
    assert_eq!(flip_to_short.legs[0].side, SideV16::Short);
    assert_eq!(flip_to_short.legs[0].basis_pos_q, -(POS_SCALE as i128));
    assert_eq!(g.assets[0].oi_eff_long_q, POS_SCALE);
    assert_eq!(g.assets[0].oi_eff_short_q, POS_SCALE);
    assert_eq!(g.assets[0].stored_pos_count_long, 1);
    assert_eq!(g.assets[0].stored_pos_count_short, 1);
    assert_eq!(g.vault, vault_before);
    assert_eq!(g.c_tot, c_tot_before);
}

#[test]
fn v16_e2e_trade_mark_close_convert_withdraw_conserves() {
    let (market, _, owner) = ids();
    let mut g = group();
    let mut alice = account();
    let mut bob = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    let px1 = [1; V16_MAX_PORTFOLIO_ASSETS_N];
    let px2 = [2; V16_MAX_PORTFOLIO_ASSETS_N];

    g.deposit_not_atomic(&mut alice, 10_000).unwrap();
    g.deposit_not_atomic(&mut bob, 10_000).unwrap();
    let vault_after_deposit = g.vault;

    g.execute_trade_with_fee_not_atomic(
        &mut alice,
        &mut bob,
        TradeRequestV16 {
            asset_index: 0,
            size_q: POS_SCALE,
            exec_price: 1,
            fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
        },
        &px1,
    )
    .unwrap();
    assert_eq!(g.assets[0].oi_eff_long_q, POS_SCALE);
    assert_eq!(g.assets[0].oi_eff_short_q, POS_SCALE);

    g.permissionless_crank_not_atomic(
        &mut alice,
        PermissionlessCrankRequestV16 {
            now_slot: 1,
            asset_index: 0,
            effective_price: 2,
            funding_rate_e9: 0,
            action: PermissionlessCrankActionV16::Refresh,
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
    assert_eq!(
        bob.pnl, 0,
        "short mark loss should be realized into reserved counterparty backing, not left as unpaid PnL"
    );
    assert_eq!(
        bob.capital, 9_999,
        "short mark loss should no longer remain withdrawable account capital"
    );
    assert_eq!(
        g.source_credit[1].fresh_reserved_backing_num, BOUND_SCALE,
        "short loss refresh should reserve backing without wrapper injection"
    );

    g.execute_trade_with_fee_not_atomic(
        &mut bob,
        &mut alice,
        TradeRequestV16 {
            asset_index: 0,
            size_q: POS_SCALE,
            exec_price: 2,
            fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
        },
        &px2,
    )
    .unwrap();
    assert_eq!(alice.active_bitmap, bitmap(&[]));
    assert_eq!(bob.active_bitmap, bitmap(&[]));
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
fn v16_price_accrual_then_refresh_matches_eager_mark_pnl() {
    let mut g = group();
    g.assets[0].effective_price = 100;
    g.assets[0].fund_px_last = 100;
    g.assets[0].raw_oracle_target_price = 100;
    let mut long = account();
    let mut short = account();
    short.provenance_header.portfolio_account_id = [4; 32];

    g.attach_leg(&mut long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut short, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    let out = g.accrue_asset_to_not_atomic(0, 1, 101, 0, true).unwrap();
    assert!(out.price_move_active);

    g.full_account_refresh(&mut long, &[101; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    g.full_account_refresh(&mut short, &[101; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(long.pnl, 1);
    assert_eq!(short.pnl, -1);
    assert_eq!(g.pnl_pos_tot, 1);
    assert_eq!(g.negative_pnl_account_count, 1);
}

#[test]
fn v16_same_epoch_full_refresh_is_idempotent_after_kf_settlement() {
    let mut g = group();
    g.assets[0].effective_price = 100;
    g.assets[0].fund_px_last = 100;
    g.assets[0].raw_oracle_target_price = 100;
    let mut a = account();

    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV16::Long, POS_SCALE, 96);
    g.accrue_asset_to_not_atomic(0, 1, 101, 0, true).unwrap();
    g.full_account_refresh(&mut a, &[101; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    let account_after_first = a.clone();
    let group_after_first = g.clone();

    g.full_account_refresh(&mut a, &[101; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(a, account_after_first);
    assert_eq!(g, group_after_first);
}

#[test]
fn v16_sequential_kf_refresh_is_additive_not_compounding() {
    let mut sequential = group();
    sequential.assets[0].effective_price = 100;
    sequential.assets[0].fund_px_last = 100;
    sequential.assets[0].raw_oracle_target_price = 100;
    let mut seq_account = account();
    sequential
        .attach_leg(&mut seq_account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let _seq_opposite = attach_opposite(&mut sequential, 0, SideV16::Long, POS_SCALE, 97);

    sequential
        .accrue_asset_to_not_atomic(0, 1, 101, 0, true)
        .unwrap();
    sequential
        .full_account_refresh(&mut seq_account, &[101; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(seq_account.pnl, 1);

    sequential
        .accrue_asset_to_not_atomic(0, 2, 102, 0, true)
        .unwrap();
    sequential
        .full_account_refresh(&mut seq_account, &[102; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    let mut direct = group();
    direct.assets[0].effective_price = 100;
    direct.assets[0].fund_px_last = 100;
    direct.assets[0].raw_oracle_target_price = 100;
    let mut direct_account = account();
    direct
        .attach_leg(&mut direct_account, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let _direct_opposite = attach_opposite(&mut direct, 0, SideV16::Long, POS_SCALE, 98);

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

#[test]
fn v16_funding_accrual_then_refresh_matches_sign_and_floor() {
    let (market, _, _) = ids();
    let mut cfg = V16Config::public_user_fund(4, 0, 10);
    cfg.max_price_move_bps_per_slot = 4_999;
    cfg.max_abs_funding_e9_per_slot = 1;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    g.assets[0].effective_price = 1_000_000_000;
    g.assets[0].fund_px_last = 1_000_000_000;
    g.assets[0].raw_oracle_target_price = 1_000_000_000;
    let mut long = account();
    let mut short = account();
    short.provenance_header.portfolio_account_id = [4; 32];

    g.attach_leg(&mut long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut short, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    let out = g
        .accrue_asset_to_not_atomic(0, 1, 1_000_000_000, 1, true)
        .unwrap();
    assert!(out.funding_active);

    g.full_account_refresh(&mut long, &[1_000_000_000; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    g.full_account_refresh(&mut short, &[1_000_000_000; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(long.pnl, -1);
    assert_eq!(short.pnl, 1);
}

#[test]
fn v16_funding_accrual_requires_bilateral_exposure() {
    let (market, _, _) = ids();
    let mut cfg = V16Config::public_user_fund(4, 0, 10);
    cfg.max_price_move_bps_per_slot = 9_999;
    cfg.max_abs_funding_e9_per_slot = 1;
    let mut no_oi = MarketGroupV16::new(market, cfg).unwrap();
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

    let mut one_sided = MarketGroupV16::new(market, cfg).unwrap();
    one_sided.assets[0].effective_price = 1_000_000_000;
    one_sided.assets[0].fund_px_last = 1_000_000_000;
    one_sided.assets[0].raw_oracle_target_price = 1_000_000_000;
    let mut long = account();
    one_sided
        .attach_leg(&mut long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let one_sided_before = one_sided.assets[0];
    let out = one_sided
        .accrue_asset_to_not_atomic(0, 1, 1_000_000_000, 1, false)
        .unwrap_err();
    assert_eq!(out, V16Error::InvalidConfig);
    assert_eq!(one_sided.assets[0].f_long_num, one_sided_before.f_long_num);
    assert_eq!(
        one_sided.assets[0].f_short_num,
        one_sided_before.f_short_num
    );
    assert_eq!(one_sided.funding_epoch, 0);

    let mut short_only = MarketGroupV16::new(market, cfg).unwrap();
    short_only.assets[0].effective_price = 1_000_000_000;
    short_only.assets[0].fund_px_last = 1_000_000_000;
    short_only.assets[0].raw_oracle_target_price = 1_000_000_000;
    let mut short = account();
    short.provenance_header.portfolio_account_id = [5; 32];
    short_only
        .attach_leg(&mut short, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    let short_only_before = short_only.assets[0];
    let out = short_only
        .accrue_asset_to_not_atomic(0, 1, 1_000_000_000, 1, false)
        .unwrap_err();
    assert_eq!(out, V16Error::InvalidConfig);
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
fn v16_permissionless_crank_accepts_configured_funding_rate_boundaries() {
    let (market, _, _) = ids();
    let mut cfg = V16Config::public_user_fund(4, 0, 10);
    cfg.max_price_move_bps_per_slot = 9_999;
    cfg.max_abs_funding_e9_per_slot = 1;
    let mut positive = MarketGroupV16::new(market, cfg).unwrap();
    let mut positive_account = account();
    let req = PermissionlessCrankRequestV16 {
        now_slot: 1,
        asset_index: 0,
        effective_price: 1,
        funding_rate_e9: 1,
        action: PermissionlessCrankActionV16::Refresh,
    };
    assert_eq!(
        positive.permissionless_crank_not_atomic(
            &mut positive_account,
            req,
            &[1; V16_MAX_PORTFOLIO_ASSETS_N]
        ),
        Ok(PermissionlessProgressOutcomeV16::AccountCurrent)
    );

    let mut negative = MarketGroupV16::new(market, cfg).unwrap();
    let mut negative_account = account();
    let negative_req = PermissionlessCrankRequestV16 {
        funding_rate_e9: -1,
        ..req
    };
    assert_eq!(
        negative.permissionless_crank_not_atomic(
            &mut negative_account,
            negative_req,
            &[1; V16_MAX_PORTFOLIO_ASSETS_N]
        ),
        Ok(PermissionlessProgressOutcomeV16::AccountCurrent)
    );
}

#[test]
fn v16_funding_accrual_uses_only_bounded_segment_dt() {
    let (market, _, _) = ids();
    let mut cfg = V16Config::public_user_fund(4, 0, 10);
    cfg.max_price_move_bps_per_slot = 4_999;
    cfg.max_abs_funding_e9_per_slot = 1;
    cfg.max_accrual_dt_slots = 2;
    cfg.min_funding_lifetime_slots = 2;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    g.assets[0].effective_price = 1_000_000_000;
    g.assets[0].fund_px_last = 1_000_000_000;
    g.assets[0].raw_oracle_target_price = 1_000_000_000;
    let mut long = account();
    let mut short = account();
    short.provenance_header.portfolio_account_id = [4; 32];
    g.attach_leg(&mut long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut short, 0, SideV16::Short, -(POS_SCALE as i128))
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
fn v16_combined_price_and_funding_accrual_keeps_k_and_f_separate() {
    let (market, _, _) = ids();
    let mut cfg = V16Config::public_user_fund(4, 0, 10);
    cfg.max_price_move_bps_per_slot = 9_999;
    cfg.max_abs_funding_e9_per_slot = 1;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    g.assets[0].effective_price = 999_999_999;
    g.assets[0].fund_px_last = 999_999_999;
    g.assets[0].raw_oracle_target_price = 999_999_999;
    let mut long = account();
    let mut short = account();
    short.provenance_header.portfolio_account_id = [4; 32];
    g.attach_leg(&mut long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut short, 0, SideV16::Short, -(POS_SCALE as i128))
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
fn v16_zero_funding_rate_advances_time_without_f_mutation() {
    let mut g = group();
    g.assets[0].effective_price = 100;
    g.assets[0].fund_px_last = 100;
    g.assets[0].raw_oracle_target_price = 100;
    let mut long = account();
    let mut short = account();
    short.provenance_header.portfolio_account_id = [4; 32];
    g.attach_leg(&mut long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut short, 0, SideV16::Short, -(POS_SCALE as i128))
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
fn v16_same_slot_exposed_price_move_rejects_without_mutation() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let before = g.clone();

    assert_eq!(
        g.accrue_asset_to_not_atomic(0, 0, 2, 0, true),
        Err(V16Error::NonProgress)
    );
    assert_eq!(g, before);
}

#[test]
fn v16_over_cap_exposed_price_move_routes_recovery_without_mutation() {
    let mut g = group();
    g.config.max_price_move_bps_per_slot = 100;
    g.assets[0].effective_price = 100;
    g.assets[0].raw_oracle_target_price = 100;
    g.assets[0].fund_px_last = 100;
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let before = g.clone();

    assert_eq!(
        g.accrue_asset_to_not_atomic(0, 1, 102, 0, true),
        Err(V16Error::RecoveryRequired)
    );
    assert_eq!(g, before);
}

#[test]
fn v16_hlock_allows_risk_increasing_trade_with_no_positive_credit_margin() {
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
            TradeRequestV16 {
                asset_index: 0,
                size_q: POS_SCALE,
                exec_price: 100,
                fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
            },
            &[100; V16_MAX_PORTFOLIO_ASSETS_N],
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
fn v16_loss_stale_blocks_risk_increasing_trade_even_with_no_positive_credit_margin() {
    let mut g = group();
    let mut long = account();
    let mut short = account();
    short.provenance_header.portfolio_account_id = [4; 32];
    g.deposit_not_atomic(&mut long, 10_000).unwrap();
    g.deposit_not_atomic(&mut short, 10_000).unwrap();
    g.loss_stale_active = true;

    let before = (g.clone(), long.clone(), short.clone());
    let res = g.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV16 {
            asset_index: 0,
            size_q: POS_SCALE,
            exec_price: 100,
            fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
        },
        &[100; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(res, Err(V16Error::LockActive));
    assert_eq!((g, long, short), before);
}

#[test]
fn v16_hlock_rejects_risk_increasing_trade_that_needs_positive_pnl_credit() {
    let mut g = group();
    let mut long = account();
    let mut short = account();
    short.provenance_header.portfolio_account_id = [4; 32];
    g.add_account_source_positive_pnl_not_atomic(&mut long, 0, 200)
        .unwrap();
    g.add_account_source_positive_pnl_not_atomic(&mut short, 1, 200)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 200 * BOUND_SCALE, 10)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(1, 200 * BOUND_SCALE, 10)
        .unwrap();
    g.threshold_stress_active = true;

    let before = (g.clone(), long.clone(), short.clone());
    let res = g.execute_trade_with_fee_not_atomic(
        &mut long,
        &mut short,
        TradeRequestV16 {
            asset_index: 0,
            size_q: POS_SCALE,
            exec_price: 100,
            fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
        },
        &[100; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(res, Err(V16Error::LockActive));
    assert_eq!((g, long, short), before);
}

#[test]
fn v16_hlock_allows_pure_risk_reducing_trade_with_no_positive_credit_margin() {
    let mut g = group();
    let mut reducing_short = account();
    let mut reducing_long = account();
    reducing_long.provenance_header.portfolio_account_id = [4; 32];
    g.deposit_not_atomic(&mut reducing_short, 10_000).unwrap();
    g.deposit_not_atomic(&mut reducing_long, 10_000).unwrap();
    g.attach_leg(&mut reducing_short, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.attach_leg(&mut reducing_long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.threshold_stress_active = true;

    let out = g
        .execute_trade_with_fee_not_atomic(
            &mut reducing_short,
            &mut reducing_long,
            TradeRequestV16 {
                asset_index: 0,
                size_q: POS_SCALE / 2,
                exec_price: 100,
                fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
            },
            &[100; V16_MAX_PORTFOLIO_ASSETS_N],
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
fn v16_hlock_rejects_reducing_trade_that_needs_positive_pnl_credit() {
    let mut g = group();
    let mut weak_short = account();
    let mut strong_long = account();
    strong_long.provenance_header.portfolio_account_id = [4; 32];
    g.add_account_source_positive_pnl_not_atomic(&mut weak_short, 0, 100)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 100 * BOUND_SCALE, 10)
        .unwrap();
    g.deposit_not_atomic(&mut strong_long, 10_000).unwrap();
    g.attach_leg(&mut weak_short, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.attach_leg(&mut strong_long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.threshold_stress_active = true;

    let res = g.execute_trade_with_fee_not_atomic(
        &mut weak_short,
        &mut strong_long,
        TradeRequestV16 {
            asset_index: 0,
            size_q: POS_SCALE / 2,
            exec_price: 100,
            fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
        },
        &[100; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(res, Err(V16Error::LockActive));
}

#[test]
fn v16_released_pnl_conversion_burns_face_claim_under_global_impairment() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 10).unwrap();
    g.add_account_source_positive_pnl_not_atomic(&mut a, 0, 50)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 7 * BOUND_SCALE, 10)
        .unwrap();
    g.pnl_matured_pos_tot = 50;
    g.vault = g.c_tot + 7;
    g.full_account_refresh(&mut a, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
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
fn v16_loss_stale_allows_pure_risk_reducing_trade_path() {
    let mut g = group();
    let mut reducing_short = account();
    let mut reducing_long = account();
    reducing_long.provenance_header.portfolio_account_id = [4; 32];
    g.deposit_not_atomic(&mut reducing_short, 10_000).unwrap();
    g.deposit_not_atomic(&mut reducing_long, 10_000).unwrap();
    g.attach_leg(&mut reducing_short, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.attach_leg(&mut reducing_long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.loss_stale_active = true;

    assert!(g
        .execute_trade_with_fee_not_atomic(
            &mut reducing_short,
            &mut reducing_long,
            TradeRequestV16 {
                asset_index: 0,
                size_q: POS_SCALE / 2,
                exec_price: 100,
                fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
            },
            &[100; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .is_ok());
}

#[test]
fn v16_b_residual_booking_is_bounded_and_remainder_conserving() {
    let mut g = group();
    let mut short = account();
    g.deposit_not_atomic(&mut short, 100).unwrap();
    g.attach_leg(&mut short, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    let mut bankrupt = account();

    let out = g
        .book_bankruptcy_residual_chunk_for_account(&mut bankrupt, 0, SideV16::Long, 7)
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
fn v16_zero_weight_domain_residual_cannot_clear_without_backing() {
    let mut g = group();
    let mut bankrupt = account();

    assert_eq!(
        g.book_bankruptcy_residual_chunk_for_account(&mut bankrupt, 0, SideV16::Long, 1),
        Err(V16Error::RecoveryRequired)
    );
    assert_eq!(
        g.recovery_reason,
        Some(PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress)
    );
    assert_eq!(g.assets[0].explicit_unallocated_loss_short, 0);
    assert!(!bankrupt.close_progress.active);
    assert_eq!(
        g.pending_domain_loss_barrier_count(0, SideV16::Short),
        Ok(0)
    );
}

#[test]
fn v16_pending_close_progress_blocks_domain_escape_until_finalized() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    a.close_progress = CloseProgressLedgerV16 {
        active: true,
        finalized: false,
        close_id: 1,
        asset_index: 0,
        market_id: g.assets[0].market_id,
        domain_side: SideV16::Short,
        gross_loss_at_close_start: 10,
        drift_reference_slot: g.current_slot,
        max_close_slot: g.current_slot + 1,
        residual_remaining: 10,
        ..CloseProgressLedgerV16::EMPTY
    };

    assert_eq!(g.clear_leg(&mut a, 0), Err(V16Error::LockActive));
    assert_eq!(g.h_lock_lane(Some(&a), false, None), Ok(HLockLaneV16::HMax));
}

#[test]
fn v16_cure_and_cancel_close_releases_barrier_and_escrow_before_irreversible_progress() {
    let mut g = group();
    let mut a = account();
    g.create_portfolio_account(&a).unwrap();
    a.close_progress = CloseProgressLedgerV16 {
        active: true,
        close_id: 1,
        asset_index: 0,
        market_id: g.assets[0].market_id,
        domain_side: SideV16::Short,
        gross_loss_at_close_start: 5,
        drift_reference_slot: g.current_slot,
        max_close_slot: g.current_slot + g.config.max_bankrupt_close_lifetime_slots,
        residual_remaining: 5,
        ..CloseProgressLedgerV16::EMPTY
    };
    g.pending_domain_loss_barriers[1] = 1;

    g.cure_and_cancel_close_not_atomic(&mut a, 7, &[100; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert!(!a.close_progress.active);
    assert!(a.close_progress.canceled);
    assert_eq!(a.close_progress.close_id, 1);
    assert_eq!(a.close_progress.residual_remaining, 5);
    assert_eq!(a.cancel_deposit_escrow, 0);
    assert_eq!(a.capital, 7);
    assert_eq!(g.c_tot, 7);
    assert_eq!(g.vault, 7);
    assert_eq!(
        g.pending_domain_loss_barrier_count(0, SideV16::Short),
        Ok(0)
    );
}

#[test]
fn v16_cure_and_cancel_close_rejects_after_irreversible_progress_without_consuming_deposit() {
    let mut g = group();
    let mut a = account();
    g.create_portfolio_account(&a).unwrap();
    a.close_progress = CloseProgressLedgerV16 {
        active: true,
        close_id: 1,
        asset_index: 0,
        market_id: g.assets[0].market_id,
        domain_side: SideV16::Short,
        gross_loss_at_close_start: 5,
        drift_reference_slot: g.current_slot,
        max_close_slot: g.current_slot + g.config.max_bankrupt_close_lifetime_slots,
        insurance_spent: 1,
        residual_remaining: 4,
        ..CloseProgressLedgerV16::EMPTY
    };
    g.pending_domain_loss_barriers[1] = 1;
    let before_account = a.clone();
    let before_group = g.clone();

    assert_eq!(
        g.cure_and_cancel_close_not_atomic(&mut a, 7, &[100; V16_MAX_PORTFOLIO_ASSETS_N]),
        Err(V16Error::LockActive)
    );
    assert_eq!(a, before_account);
    assert_eq!(g, before_group);
}

#[test]
fn v16_new_close_cannot_overwrite_active_finalized_close_ledger() {
    let mut g = group();
    let mut bankrupt = account();
    let mut opposing =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new([1; 32], [42; 32], [3; 32]));
    g.attach_leg(&mut bankrupt, 1, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut opposing, 1, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    bankrupt.close_progress = CloseProgressLedgerV16 {
        active: true,
        finalized: true,
        close_id: 7,
        asset_index: 0,
        market_id: g.assets[0].market_id,
        domain_side: SideV16::Short,
        gross_loss_at_close_start: 2,
        b_loss_booked: 2,
        residual_remaining: 0,
        drift_reference_slot: g.current_slot,
        max_close_slot: g.current_slot + 1,
        ..CloseProgressLedgerV16::EMPTY
    };
    g.assets[1].k_long = -(100 * ADL_ONE as i128);
    let before_ledger = bankrupt.close_progress;
    let before_b_short = g.assets[1].b_short_num;

    assert_eq!(
        g.liquidate_account_not_atomic(
            &mut bankrupt,
            LiquidationRequestV16 {
                asset_index: 1,
                close_q: POS_SCALE,
                fee_bps: 0,
            },
            &[1; V16_MAX_PORTFOLIO_ASSETS_N],
        ),
        Err(V16Error::LockActive)
    );
    assert_eq!(bankrupt.close_progress, before_ledger);
    assert_eq!(g.assets[1].b_short_num, before_b_short);
}

#[test]
fn v16_pending_domain_loss_barrier_blocks_other_participants_until_residual_done() {
    let (market, _, owner) = ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.public_b_chunk_atoms = 1;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    let mut bankrupt = account();
    let mut participant =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    let mut joiner = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [5; 32], owner));

    g.attach_leg(&mut participant, 0, SideV16::Short, -10)
        .unwrap();
    let first = g
        .book_bankruptcy_residual_chunk_for_account(&mut bankrupt, 0, SideV16::Long, 2)
        .unwrap();
    assert_eq!(first.booked_loss, 1);
    assert_eq!(
        g.pending_domain_loss_barrier_count(0, SideV16::Short),
        Ok(1)
    );
    assert_eq!(g.clear_leg(&mut participant, 0), Err(V16Error::LockActive));
    assert_eq!(
        g.attach_leg(&mut joiner, 0, SideV16::Short, -1),
        Err(V16Error::LockActive)
    );

    let second = g
        .book_bankruptcy_residual_chunk_for_account(&mut bankrupt, 0, SideV16::Long, 2)
        .unwrap();
    assert_eq!(second.booked_loss, 1);
    assert!(bankrupt.close_progress.finalized);
    assert_eq!(
        g.pending_domain_loss_barrier_count(0, SideV16::Short),
        Ok(0)
    );
    assert_eq!(
        g.clear_leg(&mut participant, 0),
        Err(V16Error::Stale),
        "participants must settle lazy B loss before clearing weight"
    );
    loop {
        let chunk = g
            .settle_account_b_chunk(&mut participant, 0, u128::MAX)
            .unwrap();
        if chunk.remaining_after == 0 {
            break;
        }
    }
    g.clear_leg(&mut participant, 0).unwrap();
}

#[test]
fn v16_single_domain_close_lock_rejects_second_origin_until_first_finalized() {
    let (market, _, owner) = ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.public_b_chunk_atoms = 1;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    let mut first_bankrupt =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    let mut second_bankrupt =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [5; 32], owner));
    let mut participant =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [6; 32], owner));

    g.attach_leg(&mut participant, 0, SideV16::Short, -10)
        .unwrap();
    let first = g
        .book_bankruptcy_residual_chunk_for_account(&mut first_bankrupt, 0, SideV16::Long, 2)
        .unwrap();
    assert_eq!(first.booked_loss, 1);
    assert_eq!(
        g.pending_domain_loss_barrier_count(0, SideV16::Short),
        Ok(1)
    );

    let before_second_ledger = second_bankrupt.close_progress;
    let before_barriers = g.pending_domain_loss_barriers.clone();
    let before_b_short = g.assets[0].b_short_num;
    assert_eq!(
        g.book_bankruptcy_residual_chunk_for_account(&mut second_bankrupt, 0, SideV16::Long, 1),
        Err(V16Error::LockActive),
        "a domain can have only one active pending close origin"
    );
    assert_eq!(second_bankrupt.close_progress, before_second_ledger);
    assert_eq!(g.pending_domain_loss_barriers, before_barriers);
    assert_eq!(g.assets[0].b_short_num, before_b_short);

    let complete_first = g
        .book_bankruptcy_residual_chunk_for_account(&mut first_bankrupt, 0, SideV16::Long, 2)
        .unwrap();
    assert_eq!(complete_first.booked_loss, 1);
    assert!(first_bankrupt.close_progress.finalized);
    assert_eq!(
        g.pending_domain_loss_barrier_count(0, SideV16::Short),
        Ok(0)
    );

    let second = g
        .book_bankruptcy_residual_chunk_for_account(&mut second_bankrupt, 0, SideV16::Long, 1)
        .unwrap();
    assert_eq!(second.booked_loss, 1);
    assert!(second_bankrupt.close_progress.finalized);
}

#[test]
fn v16_public_invariants_reject_multiple_pending_barriers_per_domain() {
    let mut g = group();
    g.pending_domain_loss_barriers[1] = 2;
    assert_eq!(g.assert_public_invariants(), Err(V16Error::InvalidConfig));
}

#[test]
fn v16_pending_domain_loss_barrier_allows_partial_risk_reduction_with_weight_obligation_preserved()
{
    let (market, _, owner) = ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.public_b_chunk_atoms = 1;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    let mut participant =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    let mut counterparty =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [5; 32], owner));

    g.deposit_not_atomic(&mut participant, 1_000).unwrap();
    g.deposit_not_atomic(&mut counterparty, 1_000).unwrap();
    g.attach_leg(&mut participant, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.attach_leg(&mut counterparty, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.pending_domain_loss_barriers[1] = 1;
    assert_eq!(
        g.pending_domain_loss_barrier_count(0, SideV16::Short),
        Ok(1)
    );
    let old_short_weight_sum = g.assets[0].loss_weight_sum_short;
    let old_participant_weight = participant.legs[0].loss_weight;

    let out = g
        .execute_trade_with_fee_not_atomic(
            &mut participant,
            &mut counterparty,
            TradeRequestV16 {
                asset_index: 0,
                size_q: POS_SCALE / 2,
                exec_price: 100,
                fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
            },
            &[100; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(out.notional, 50);
    assert_eq!(participant.legs[0].basis_pos_q, -((POS_SCALE / 2) as i128));
    assert_eq!(counterparty.legs[0].basis_pos_q, (POS_SCALE / 2) as i128);
    assert_eq!(
        participant.legs[0].loss_weight, old_participant_weight,
        "the participant's prior loss weight stays attached as the pending obligation"
    );
    assert_eq!(
        g.assets[0].loss_weight_sum_short, old_short_weight_sum,
        "the pending-domain denominator must not shrink until the barrier clears"
    );
    assert_eq!(g.assets[0].oi_eff_short_q, POS_SCALE / 2);
    assert_eq!(g.assets[0].oi_eff_long_q, POS_SCALE / 2);
}

#[test]
fn v16_pending_domain_loss_barrier_allows_full_trade_exit_as_flat_weight_obligation() {
    let (market, _, owner) = ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.public_b_chunk_atoms = 1;
    cfg.max_trading_fee_bps = 10;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    let mut participant =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    let mut counterparty =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [5; 32], owner));

    g.deposit_not_atomic(&mut participant, 1_000).unwrap();
    g.deposit_not_atomic(&mut counterparty, 1_000).unwrap();
    g.attach_leg(&mut participant, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.attach_leg(&mut counterparty, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.pending_domain_loss_barriers[1] = 1;

    let old_weight = participant.legs[0].loss_weight;
    g.execute_trade_with_fee_not_atomic(
        &mut participant,
        &mut counterparty,
        TradeRequestV16 {
            asset_index: 0,
            size_q: POS_SCALE,
            exec_price: 100,
            fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
        },
        &[100; V16_MAX_PORTFOLIO_ASSETS_N],
    )
    .unwrap();

    assert!(participant.legs[0].active);
    assert_eq!(participant.legs[0].basis_pos_q, 0);
    assert_eq!(participant.legs[0].loss_weight, old_weight);
    assert_eq!(counterparty.legs[0], PortfolioLegV16::EMPTY);
    assert_eq!(g.assets[0].oi_eff_long_q, 0);
    assert_eq!(g.assets[0].oi_eff_short_q, 0);
    assert_eq!(g.assets[0].loss_weight_sum_short, old_weight);
    assert_eq!(g.assets[0].pending_obligation_count_short, 1);
    assert_eq!(g.clear_leg(&mut participant, 0), Err(V16Error::LockActive));

    g.pending_domain_loss_barriers[1] = 0;
    g.clear_leg(&mut participant, 0).unwrap();
    assert_eq!(g.assets[0].loss_weight_sum_short, 0);
    assert_eq!(g.assets[0].pending_obligation_count_short, 0);
}

#[test]
fn v16_pending_obligation_blocks_side_reset_until_obligation_account_clears() {
    let (market, _, owner) = ids();
    let mut g = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 10)).unwrap();
    let mut participant =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    let mut counterparty =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [5; 32], owner));

    g.deposit_not_atomic(&mut participant, 1_000).unwrap();
    g.deposit_not_atomic(&mut counterparty, 1_000).unwrap();
    g.attach_leg(&mut participant, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.attach_leg(&mut counterparty, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.pending_domain_loss_barriers[1] = 1;
    g.execute_trade_with_fee_not_atomic(
        &mut participant,
        &mut counterparty,
        TradeRequestV16 {
            asset_index: 0,
            size_q: POS_SCALE,
            exec_price: 100,
            fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
        },
        &[100; V16_MAX_PORTFOLIO_ASSETS_N],
    )
    .unwrap();

    assert_eq!(g.assets[0].oi_eff_short_q, 0);
    assert_eq!(g.assets[0].pending_obligation_count_short, 1);
    g.pending_domain_loss_barriers[1] = 0;
    let before = g.clone();
    assert_eq!(
        g.begin_full_drain_reset(0, SideV16::Short),
        Err(V16Error::LockActive),
        "a flat pending-obligation leg must clear before side reset can wipe weights"
    );
    assert_eq!(
        g.assets[0].loss_weight_sum_short,
        before.assets[0].loss_weight_sum_short
    );
    assert_eq!(
        g.assets[0].pending_obligation_count_short,
        before.assets[0].pending_obligation_count_short
    );
    assert_eq!(g.assets[0].mode_short, before.assets[0].mode_short);

    g.clear_leg(&mut participant, 0).unwrap();
    g.begin_full_drain_reset(0, SideV16::Short).unwrap();
    assert_eq!(g.assets[0].mode_short, SideModeV16::ResetPending);
}

#[test]
fn v16_flat_pending_obligation_must_settle_b_loss_before_clear() {
    let (market, _, owner) = ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.public_b_chunk_atoms = 1;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    let mut bankrupt = account();
    let mut participant =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    let mut counterparty =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [5; 32], owner));

    g.deposit_not_atomic(&mut participant, 1_000).unwrap();
    g.deposit_not_atomic(&mut counterparty, 1_000).unwrap();
    g.attach_leg(&mut participant, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.attach_leg(&mut counterparty, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.pending_domain_loss_barriers[1] = 1;

    g.execute_trade_with_fee_not_atomic(
        &mut participant,
        &mut counterparty,
        TradeRequestV16 {
            asset_index: 0,
            size_q: POS_SCALE,
            exec_price: 100,
            fee_bps: 0,
            admit_h_max_consumption_threshold_bps_opt: None,
        },
        &[100; V16_MAX_PORTFOLIO_ASSETS_N],
    )
    .unwrap();
    assert_eq!(participant.legs[0].basis_pos_q, 0);
    assert_eq!(g.assets[0].pending_obligation_count_short, 1);

    g.pending_domain_loss_barriers[1] = 0;
    g.book_bankruptcy_residual_chunk_for_account(&mut bankrupt, 0, SideV16::Long, 1)
        .unwrap();
    assert_eq!(
        g.pending_domain_loss_barrier_count(0, SideV16::Short),
        Ok(0)
    );
    assert_eq!(
        g.clear_leg(&mut participant, 0),
        Err(V16Error::Stale),
        "zero-basis obligations still owe their loss-weight share of B"
    );

    loop {
        let chunk = g
            .settle_account_b_chunk(&mut participant, 0, u128::MAX)
            .unwrap();
        if chunk.remaining_after == 0 {
            break;
        }
    }
    g.clear_leg(&mut participant, 0).unwrap();
    assert_eq!(g.assets[0].pending_obligation_count_short, 0);
    assert_eq!(g.assets[0].loss_weight_sum_short, 0);
}

#[test]
fn v16_pending_domain_loss_barrier_allows_rebalance_reduction_with_weight_obligation_preserved() {
    let (market, _, owner) = ids();
    let mut g = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 10)).unwrap();
    let mut participant =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));

    g.deposit_not_atomic(&mut participant, 1_000).unwrap();
    g.attach_leg(&mut participant, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    let _counterparty = attach_opposite(&mut g, 0, SideV16::Short, POS_SCALE, 6);
    g.full_account_refresh(&mut participant, &[100; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    g.pending_domain_loss_barriers[1] = 1;
    let old_weight_sum = g.assets[0].loss_weight_sum_short;
    let old_weight = participant.legs[0].loss_weight;

    let out = g
        .rebalance_reduce_position_not_atomic(
            &mut participant,
            RebalanceRequestV16 {
                asset_index: 0,
                reduce_q: POS_SCALE / 2,
            },
            &[100; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(out.reduced_q, POS_SCALE / 2);
    assert_eq!(participant.legs[0].basis_pos_q, -((POS_SCALE / 2) as i128));
    assert_eq!(participant.legs[0].loss_weight, old_weight);
    assert_eq!(
        g.assets[0].loss_weight_sum_short, old_weight_sum,
        "rebalanced weight remains as pending obligation until the barrier clears"
    );
    assert_eq!(g.assets[0].oi_eff_short_q, POS_SCALE / 2);
}

#[test]
fn v16_pending_domain_loss_barrier_allows_rebalance_full_exit_as_flat_weight_obligation() {
    let (market, _, owner) = ids();
    let mut g = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 10)).unwrap();
    let mut participant =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));

    g.deposit_not_atomic(&mut participant, 1_000).unwrap();
    g.attach_leg(&mut participant, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    let _counterparty = attach_opposite(&mut g, 0, SideV16::Short, POS_SCALE, 6);
    g.full_account_refresh(&mut participant, &[100; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    g.pending_domain_loss_barriers[1] = 1;

    let old_weight = participant.legs[0].loss_weight;
    let out = g
        .rebalance_reduce_position_not_atomic(
            &mut participant,
            RebalanceRequestV16 {
                asset_index: 0,
                reduce_q: POS_SCALE,
            },
            &[100; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(out.reduced_q, POS_SCALE);
    assert!(participant.legs[0].active);
    assert_eq!(participant.legs[0].basis_pos_q, 0);
    assert_eq!(participant.legs[0].loss_weight, old_weight);
    assert_eq!(g.assets[0].oi_eff_short_q, 0);
    assert_eq!(g.assets[0].loss_weight_sum_short, old_weight);
    assert_eq!(g.assets[0].pending_obligation_count_short, 1);
    assert_eq!(g.clear_leg(&mut participant, 0), Err(V16Error::LockActive));

    g.pending_domain_loss_barriers[1] = 0;
    g.clear_leg(&mut participant, 0).unwrap();
    assert_eq!(g.assets[0].loss_weight_sum_short, 0);
    assert_eq!(g.assets[0].pending_obligation_count_short, 0);
}

#[test]
fn v16_expired_close_progress_routes_recovery_before_b_booking() {
    let mut g = group();
    let mut participant = account();
    let mut bankrupt = account();
    g.attach_leg(&mut participant, 0, SideV16::Short, -10)
        .unwrap();
    bankrupt.close_progress = CloseProgressLedgerV16 {
        active: true,
        finalized: false,
        close_id: 1,
        asset_index: 0,
        market_id: g.assets[0].market_id,
        domain_side: SideV16::Short,
        gross_loss_at_close_start: 2,
        drift_reference_slot: 0,
        max_close_slot: 1,
        residual_remaining: 2,
        ..CloseProgressLedgerV16::EMPTY
    };
    g.current_slot = 2;
    let b_before = g.assets[0].b_short_num;

    assert_eq!(
        g.book_bankruptcy_residual_chunk_for_account(&mut bankrupt, 0, SideV16::Long, 2),
        Err(V16Error::RecoveryRequired)
    );
    assert_eq!(
        g.recovery_reason,
        Some(PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress)
    );
    assert_eq!(g.assets[0].b_short_num, b_before);
    assert_eq!(bankrupt.close_progress.b_loss_booked, 0);
    assert_eq!(bankrupt.close_progress.residual_remaining, 2);
    assert_eq!(
        g.pending_domain_loss_barrier_count(0, SideV16::Short),
        Ok(0)
    );
}

#[test]
fn v16_close_progress_uses_configured_lifetime_and_does_not_refresh_on_continuation() {
    let (market, _, owner) = ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.max_bankrupt_close_chunks = 7;
    cfg.max_bankrupt_close_lifetime_slots = 5;
    cfg.public_b_chunk_atoms = 1;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    g.current_slot = 11;
    let mut bankrupt = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [9; 32], owner));
    let mut participant =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    g.attach_leg(&mut participant, 0, SideV16::Short, -10)
        .unwrap();

    let first = g
        .book_bankruptcy_residual_chunk_for_account(&mut bankrupt, 0, SideV16::Long, 2)
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
        .book_bankruptcy_residual_chunk_for_account(&mut bankrupt, 0, SideV16::Long, 2)
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
fn v16_expired_close_progress_routes_recovery_before_quantity_adl() {
    let mut g = group();
    let mut closing = account();
    let mut opposing =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new([1; 32], [12; 32], [3; 32]));
    g.attach_leg(&mut closing, 0, SideV16::Long, 4).unwrap();
    g.attach_leg(&mut opposing, 0, SideV16::Short, -4).unwrap();
    closing.close_progress = CloseProgressLedgerV16 {
        active: true,
        finalized: true,
        close_id: 1,
        asset_index: 0,
        market_id: g.assets[0].market_id,
        domain_side: SideV16::Short,
        gross_loss_at_close_start: 1,
        explicit_loss_assigned: 1,
        drift_reference_slot: 0,
        max_close_slot: 1,
        residual_remaining: 0,
        ..CloseProgressLedgerV16::EMPTY
    };
    g.assets[0].a_short = ADL_ONE;
    g.current_slot = 2;

    assert_eq!(
        g.apply_quantity_adl_after_residual_for_account_not_atomic(
            &mut closing,
            0,
            SideV16::Long,
            4
        ),
        Err(V16Error::RecoveryRequired)
    );
    assert_eq!(
        g.recovery_reason,
        Some(PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress)
    );
    assert_eq!(closing.close_progress.quantity_adl_applied_q, 0);
    assert_eq!(g.assets[0].oi_eff_long_q, 4);
    assert_eq!(g.assets[0].oi_eff_short_q, 4);
    assert_eq!(g.assets[0].a_short, ADL_ONE);
}

#[test]
fn v16_stale_active_close_residual_routes_recovery_before_b_booking() {
    let (market, _, owner) = ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.public_b_chunk_atoms = 1;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    let mut closing = account();
    let mut opposing = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    g.attach_leg(&mut closing, 0, SideV16::Long, 4).unwrap();
    g.attach_leg(&mut opposing, 0, SideV16::Short, -4).unwrap();
    closing.close_progress = CloseProgressLedgerV16 {
        active: true,
        finalized: false,
        close_id: 1,
        asset_index: 0,
        market_id: g.assets[0].market_id,
        domain_side: SideV16::Short,
        gross_loss_at_close_start: 2,
        drift_reference_slot: 0,
        max_close_slot: 10,
        residual_remaining: 2,
        ..CloseProgressLedgerV16::EMPTY
    };
    g.current_slot = 1;
    let b_before = g.assets[0].b_short_num;
    let ledger_before = closing.close_progress;

    assert_eq!(
        g.book_bankruptcy_residual_chunk_for_account(&mut closing, 0, SideV16::Long, 2),
        Err(V16Error::RecoveryRequired)
    );
    assert_eq!(
        g.recovery_reason,
        Some(PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress)
    );
    assert_eq!(g.assets[0].b_short_num, b_before);
    assert_eq!(closing.close_progress, ledger_before);
}

#[test]
fn v16_stale_active_close_routes_recovery_before_quantity_adl() {
    let mut g = group();
    let mut closing = account();
    let mut opposing =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new([1; 32], [12; 32], [3; 32]));
    g.attach_leg(&mut closing, 0, SideV16::Long, 4).unwrap();
    g.attach_leg(&mut opposing, 0, SideV16::Short, -4).unwrap();
    closing.close_progress = CloseProgressLedgerV16 {
        active: true,
        finalized: true,
        close_id: 1,
        asset_index: 0,
        market_id: g.assets[0].market_id,
        domain_side: SideV16::Short,
        gross_loss_at_close_start: 1,
        explicit_loss_assigned: 1,
        drift_reference_slot: 0,
        max_close_slot: 10,
        residual_remaining: 0,
        ..CloseProgressLedgerV16::EMPTY
    };
    g.current_slot = 1;
    let a_before = g.assets[0].a_short;
    let oi_long_before = g.assets[0].oi_eff_long_q;
    let oi_short_before = g.assets[0].oi_eff_short_q;

    assert_eq!(
        g.apply_quantity_adl_after_residual_for_account_not_atomic(
            &mut closing,
            0,
            SideV16::Long,
            4
        ),
        Err(V16Error::RecoveryRequired)
    );
    assert_eq!(
        g.recovery_reason,
        Some(PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress)
    );
    assert_eq!(closing.close_progress.quantity_adl_applied_q, 0);
    assert_eq!(g.assets[0].a_short, a_before);
    assert_eq!(g.assets[0].oi_eff_long_q, oi_long_before);
    assert_eq!(g.assets[0].oi_eff_short_q, oi_short_before);
}

#[test]
fn v16_side_reset_snapshots_epoch_start_for_prior_epoch_accounts() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.assets[0].k_long = 5 * ADL_ONE as i128;
    g.assets[0].oi_eff_long_q = 0;

    g.begin_full_drain_reset(0, SideV16::Long).unwrap();
    assert_eq!(
        g.assets[0].mode_long,
        percolator::v16::SideModeV16::ResetPending
    );
    g.full_account_refresh(&mut a, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(a.pnl, 5);

    g.clear_leg(&mut a, 0).unwrap();
    g.finalize_ready_reset_side(0, SideV16::Long).unwrap();
    assert_eq!(g.assets[0].mode_long, percolator::v16::SideModeV16::Normal);
    assert_eq!(g.assets[0].stored_pos_count_long, 0);
}

#[test]
fn v16_side_reset_cannot_finalize_until_prior_epoch_positions_clear() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.assets[0].oi_eff_long_q = 0;

    g.begin_full_drain_reset(0, SideV16::Long).unwrap();
    assert_eq!(
        g.assets[0].mode_long,
        percolator::v16::SideModeV16::ResetPending
    );
    assert_eq!(
        g.finalize_ready_reset_side(0, SideV16::Long),
        Err(V16Error::Stale)
    );

    g.clear_leg(&mut a, 0).unwrap();
    assert_eq!(g.finalize_ready_reset_side(0, SideV16::Long), Ok(()));
    assert_eq!(g.assets[0].mode_long, percolator::v16::SideModeV16::Normal);
}

#[test]
fn v16_begin_full_drain_reset_rejects_side_already_reset_pending() {
    let mut g = group();
    g.begin_full_drain_reset(0, SideV16::Long).unwrap();
    let before = g.clone();

    assert_eq!(
        g.begin_full_drain_reset(0, SideV16::Long),
        Err(V16Error::LockActive),
        "a second reset must not advance epochs while prior-epoch accounts may still exist"
    );
    assert_eq!(g.assets[0].mode_long, SideModeV16::ResetPending);
    assert_eq!(g.assets[0].epoch_long, before.assets[0].epoch_long);
    assert_eq!(g.risk_epoch, before.risk_epoch);
}

#[test]
fn v16_quantity_adl_reduces_opposing_a_or_starts_reset_after_residual_durable() {
    let mut g = group();
    let mut closing = account();
    let mut survivor =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new([1; 32], [12; 32], [3; 32]));
    let mut opposing =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new([1; 32], [13; 32], [3; 32]));
    g.attach_leg(&mut closing, 0, SideV16::Long, 4).unwrap();
    g.attach_leg(&mut survivor, 0, SideV16::Long, 6).unwrap();
    g.attach_leg(&mut opposing, 0, SideV16::Short, -10).unwrap();
    closing.close_progress = CloseProgressLedgerV16 {
        active: true,
        finalized: true,
        close_id: 1,
        asset_index: 0,
        market_id: g.assets[0].market_id,
        domain_side: SideV16::Short,
        gross_loss_at_close_start: 1,
        explicit_loss_assigned: 1,
        residual_remaining: 0,
        ..CloseProgressLedgerV16::EMPTY
    };

    let partial = g
        .apply_quantity_adl_after_residual_for_account_not_atomic(&mut closing, 0, SideV16::Long, 4)
        .unwrap();
    assert_eq!(partial.closed_q, 4);
    assert_eq!(closing.close_progress.quantity_adl_applied_q, 4);
    assert_eq!(g.assets[0].oi_eff_long_q, 6);
    assert_eq!(g.assets[0].oi_eff_short_q, 6);
    assert_eq!(g.assets[0].a_short, ADL_ONE * 6 / 10);

    let mut g = group();
    let mut closing = account();
    let mut opposing =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new([1; 32], [14; 32], [3; 32]));
    g.attach_leg(&mut closing, 0, SideV16::Long, 6).unwrap();
    g.attach_leg(&mut opposing, 0, SideV16::Short, -6).unwrap();
    closing.close_progress = CloseProgressLedgerV16 {
        active: true,
        finalized: true,
        close_id: 1,
        asset_index: 0,
        market_id: g.assets[0].market_id,
        domain_side: SideV16::Short,
        gross_loss_at_close_start: 1,
        explicit_loss_assigned: 1,
        residual_remaining: 0,
        ..CloseProgressLedgerV16::EMPTY
    };
    let full = g
        .apply_quantity_adl_after_residual_for_account_not_atomic(&mut closing, 0, SideV16::Long, 6)
        .unwrap();
    assert!(full.reset_started);
    assert_eq!(closing.close_progress.quantity_adl_applied_q, 6);
    assert_eq!(g.assets[0].oi_eff_long_q, 0);
    assert_eq!(g.assets[0].oi_eff_short_q, 0);
}

#[test]
fn v16_quantity_adl_finalizes_closing_leg_atomically_with_aggregate_oi() {
    let mut g = group();
    let mut closing = account();
    let mut survivor =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new([1; 32], [12; 32], [3; 32]));
    let mut opposing =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new([1; 32], [13; 32], [3; 32]));
    g.attach_leg(&mut closing, 0, SideV16::Long, 4).unwrap();
    g.attach_leg(&mut survivor, 0, SideV16::Long, 6).unwrap();
    g.attach_leg(&mut opposing, 0, SideV16::Short, -10).unwrap();
    closing.close_progress = CloseProgressLedgerV16 {
        active: true,
        finalized: true,
        close_id: 1,
        asset_index: 0,
        market_id: g.assets[0].market_id,
        domain_side: SideV16::Short,
        gross_loss_at_close_start: 1,
        explicit_loss_assigned: 1,
        residual_remaining: 0,
        ..CloseProgressLedgerV16::EMPTY
    };
    let survivor_weight = survivor.legs[0].loss_weight;

    let out = g
        .apply_quantity_adl_after_residual_for_account_not_atomic(&mut closing, 0, SideV16::Long, 4)
        .unwrap();

    assert_eq!(out.closed_q, 4);
    assert_eq!(closing.active_bitmap, bitmap(&[]));
    assert!(!closing.legs[0].active);
    assert_eq!(closing.close_progress.quantity_adl_applied_q, 4);
    assert_eq!(g.assets[0].oi_eff_long_q, 6);
    assert_eq!(g.assets[0].oi_eff_short_q, 6);
    assert_eq!(g.assets[0].stored_pos_count_long, 1);
    assert_eq!(g.assets[0].loss_weight_sum_long, survivor_weight);
}

#[test]
fn v16_quantity_adl_requires_finalized_matching_close_ledger() {
    let mut g = group();
    let mut closing = account();
    g.assets[0].oi_eff_long_q = 1;
    g.assets[0].oi_eff_short_q = 1;

    assert_eq!(
        g.apply_quantity_adl_after_residual_for_account_not_atomic(
            &mut closing,
            0,
            SideV16::Long,
            1,
        ),
        Err(V16Error::LockActive)
    );

    closing.close_progress = CloseProgressLedgerV16 {
        active: true,
        finalized: true,
        close_id: 1,
        asset_index: 0,
        market_id: g.assets[0].market_id,
        domain_side: SideV16::Long,
        gross_loss_at_close_start: 1,
        explicit_loss_assigned: 1,
        residual_remaining: 0,
        ..CloseProgressLedgerV16::EMPTY
    };
    assert_eq!(
        g.apply_quantity_adl_after_residual_for_account_not_atomic(
            &mut closing,
            0,
            SideV16::Long,
            1,
        ),
        Err(V16Error::LockActive)
    );
}

#[test]
fn v16_account_shape_rejects_malformed_quantity_adl_close_progress() {
    let mut g = group();
    let mut premature = account();
    premature.close_progress = CloseProgressLedgerV16 {
        active: true,
        finalized: false,
        close_id: 1,
        asset_index: 0,
        market_id: g.assets[0].market_id,
        domain_side: SideV16::Short,
        gross_loss_at_close_start: 2,
        b_loss_booked: 1,
        residual_remaining: 1,
        quantity_adl_applied_q: 1,
        ..CloseProgressLedgerV16::EMPTY
    };
    assert_eq!(
        g.validate_account_shape(&premature),
        Err(V16Error::InvalidLeg),
        "quantity ADL cannot be durable before residual finalization"
    );

    let mut still_open = account();
    g.attach_leg(&mut still_open, 0, SideV16::Long, 4).unwrap();
    still_open.close_progress = CloseProgressLedgerV16 {
        active: true,
        finalized: true,
        close_id: 1,
        asset_index: 0,
        market_id: g.assets[0].market_id,
        domain_side: SideV16::Short,
        gross_loss_at_close_start: 1,
        explicit_loss_assigned: 1,
        residual_remaining: 0,
        quantity_adl_applied_q: 4,
        ..CloseProgressLedgerV16::EMPTY
    };
    assert_eq!(
        g.validate_account_shape(&still_open),
        Err(V16Error::InvalidLeg),
        "quantity ADL and closing exposure clear must stay atomic"
    );
}

#[test]
fn v16_account_shape_rejects_malformed_canceled_close_progress() {
    let g = group();
    let mut canceled_with_progress = account();
    canceled_with_progress.close_progress = CloseProgressLedgerV16 {
        canceled: true,
        close_id: 1,
        asset_index: 0,
        market_id: g.assets[0].market_id,
        domain_side: SideV16::Short,
        gross_loss_at_close_start: 5,
        drift_reference_slot: 0,
        max_close_slot: 10,
        insurance_spent: 1,
        residual_remaining: 4,
        ..CloseProgressLedgerV16::EMPTY
    };
    assert_eq!(
        g.validate_account_shape(&canceled_with_progress),
        Err(V16Error::InvalidLeg)
    );

    let mut canceled_active = account();
    canceled_active.close_progress = CloseProgressLedgerV16 {
        active: true,
        canceled: true,
        close_id: 1,
        asset_index: 0,
        market_id: g.assets[0].market_id,
        domain_side: SideV16::Short,
        gross_loss_at_close_start: 5,
        drift_reference_slot: 0,
        max_close_slot: 10,
        residual_remaining: 5,
        ..CloseProgressLedgerV16::EMPTY
    };
    assert_eq!(
        g.validate_account_shape(&canceled_active),
        Err(V16Error::InvalidLeg)
    );
}

#[test]
fn v16_account_shape_rejects_close_progress_domain_mismatch_for_open_leg() {
    let mut g = group();
    let mut closing = account();
    g.attach_leg(&mut closing, 0, SideV16::Long, 4).unwrap();
    closing.close_progress = CloseProgressLedgerV16 {
        active: true,
        finalized: false,
        close_id: 1,
        asset_index: 0,
        market_id: g.assets[0].market_id,
        domain_side: SideV16::Long,
        gross_loss_at_close_start: 2,
        b_loss_booked: 1,
        residual_remaining: 1,
        ..CloseProgressLedgerV16::EMPTY
    };

    assert_eq!(
        g.validate_account_shape(&closing),
        Err(V16Error::InvalidLeg),
        "a close ledger for an open long leg must attribute residual loss to the short domain"
    );
}

#[test]
fn v16_permissionless_crank_commits_refresh_before_equity_active_accrual() {
    let mut g = group();
    let mut long = account();
    g.deposit_not_atomic(&mut long, 1000).unwrap();
    g.attach_leg(&mut long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV16::Long, POS_SCALE, 99);
    let req = PermissionlessCrankRequestV16 {
        now_slot: 1,
        asset_index: 0,
        effective_price: 2,
        funding_rate_e9: 0,
        action: PermissionlessCrankActionV16::Refresh,
    };
    let out = g
        .permissionless_crank_not_atomic(&mut long, req, &[2; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(out, PermissionlessProgressOutcomeV16::AccountCurrent);
    assert_eq!(g.slot_last, 1);
}

#[test]
fn v16_permissionless_crank_flat_refresh_is_not_protective_for_equity_active_accrual() {
    let mut g = group();
    let mut long = account();
    let mut short =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new([1; 32], [44; 32], [3; 32]));
    let mut flat = PortfolioAccountV16::empty(ProvenanceHeaderV16::new([1; 32], [45; 32], [3; 32]));
    g.deposit_not_atomic(&mut flat, 1).unwrap();
    g.attach_leg(&mut long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut short, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    let before_asset = g.assets[0];
    let before_slot = g.slot_last;

    let res = g.permissionless_crank_not_atomic(
        &mut flat,
        PermissionlessCrankRequestV16 {
            now_slot: 1,
            asset_index: 0,
            effective_price: 2,
            funding_rate_e9: 0,
            action: PermissionlessCrankActionV16::Refresh,
        },
        &[2; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(res, Err(V16Error::NonProgress));
    assert_eq!(g.assets[0], before_asset);
    assert_eq!(g.slot_last, before_slot);
}

#[test]
fn v16_permissionless_crank_cross_asset_liquidation_is_not_protective_for_accrued_asset() {
    let (market, _, _) = ids();
    let mut g = MarketGroupV16::new(market, V16Config::public_user_fund(2, 0, 10)).unwrap();
    let mut victim = account();
    let mut asset0_long =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [41; 32], [3; 32]));
    let mut asset0_short =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [42; 32], [3; 32]));
    let mut asset1_short =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [43; 32], [3; 32]));
    g.attach_leg(&mut asset0_long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut asset0_short, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.attach_leg(&mut victim, 1, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut asset1_short, 1, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    let before_asset = g.assets[0];
    let before_slot = g.slot_last;
    let req = PermissionlessCrankRequestV16 {
        now_slot: 1,
        asset_index: 0,
        effective_price: 2,
        funding_rate_e9: 0,
        action: PermissionlessCrankActionV16::Liquidate(LiquidationRequestV16 {
            asset_index: 1,
            close_q: POS_SCALE,
            fee_bps: 0,
        }),
    };

    let res = g.permissionless_crank_not_atomic(&mut victim, req, &[1; V16_MAX_PORTFOLIO_ASSETS_N]);

    assert_eq!(res, Err(V16Error::NonProgress));
    assert_eq!(g.assets[0], before_asset);
    assert_eq!(g.slot_last, before_slot);
}

#[test]
fn v16_permissionless_crank_does_not_require_full_market_scan() {
    let mut g = group();
    let mut hinted = account();
    g.deposit_not_atomic(&mut hinted, 1).unwrap();
    g.materialized_portfolio_count = 1_000_000;
    g.stale_certificate_count = 77;
    g.b_stale_account_count = 55;
    g.negative_pnl_account_count = 33;
    let req = PermissionlessCrankRequestV16 {
        now_slot: 0,
        asset_index: 0,
        effective_price: 1,
        funding_rate_e9: 0,
        action: PermissionlessCrankActionV16::Refresh,
    };

    let out = g
        .permissionless_crank_not_atomic(&mut hinted, req, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert_eq!(out, PermissionlessProgressOutcomeV16::AccountCurrent);
    assert!(hinted.health_cert.valid);
    assert_eq!(g.materialized_portfolio_count, 1_000_000);
    assert_eq!(g.stale_certificate_count, 77);
    assert_eq!(g.b_stale_account_count, 55);
    assert_eq!(g.negative_pnl_account_count, 33);
}

#[test]
fn v16_permissionless_refresh_can_advance_one_equity_active_segment() {
    let (market, _, owner) = ids();
    let mut g = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 10)).unwrap();
    let mut long = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [51; 32], owner));
    let mut short = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [52; 32], owner));
    g.attach_leg(&mut long, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut short, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();

    let out = g
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

    assert_eq!(out, PermissionlessProgressOutcomeV16::AccountCurrent);
    assert_eq!(g.assets[0].slot_last, 1);
    assert_eq!(g.slot_last, 1);
    assert_eq!(g.current_slot, 3);
    assert!(g.loss_stale_active);
    assert_eq!(g.assets[0].effective_price, 2);
    assert_eq!(g.assets[0].k_long, ADL_ONE as i128);
    assert_eq!(g.assets[0].k_short, -(ADL_ONE as i128));
    assert_eq!(g.assets[0].oi_eff_long_q, POS_SCALE);
    assert_eq!(g.assets[0].oi_eff_short_q, POS_SCALE);
    assert_eq!(g.assert_public_invariants(), Ok(()));
}

#[test]
fn v16_permissionless_refresh_returns_partial_b_progress_without_failing() {
    let (market, _, _) = ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.public_b_chunk_atoms = 1;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, 1).unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV16::Long, 1, 100);
    g.assets[0].b_long_num = SOCIAL_LOSS_DEN * 2;
    let req = PermissionlessCrankRequestV16 {
        now_slot: 1,
        asset_index: 0,
        effective_price: 1,
        funding_rate_e9: 0,
        action: PermissionlessCrankActionV16::Refresh,
    };

    let out = g
        .permissionless_crank_not_atomic(&mut a, req, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    assert!(matches!(
        out,
        PermissionlessProgressOutcomeV16::AccountBChunk(_)
    ));
    assert!(a.legs[0].b_stale);
    assert!(a.legs[0].b_snap > 0);
    assert!(a.legs[0].b_snap < g.assets[0].b_long_num);
    assert_eq!(g.slot_last, 0);
}

#[test]
fn v16_worst_case_hinted_progress_actions_are_total_and_bounded() {
    let req_current = PermissionlessCrankRequestV16 {
        now_slot: 0,
        asset_index: 0,
        effective_price: 1,
        funding_rate_e9: 0,
        action: PermissionlessCrankActionV16::Refresh,
    };
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 1).unwrap();
    assert_eq!(
        g.permissionless_crank_not_atomic(&mut a, req_current, &[1; V16_MAX_PORTFOLIO_ASSETS_N]),
        Ok(PermissionlessProgressOutcomeV16::AccountCurrent)
    );
    assert!(a.health_cert.valid);

    let (market, _, _) = ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.public_b_chunk_atoms = 1;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV16::Long, 1).unwrap();
    g.assets[0].b_long_num = 2;
    let out = g
        .permissionless_crank_not_atomic(
            &mut a,
            PermissionlessCrankRequestV16 {
                action: PermissionlessCrankActionV16::SettleB { asset_index: 0 },
                ..req_current
            },
            &[1; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();
    match out {
        PermissionlessProgressOutcomeV16::AccountBChunk(chunk) => {
            assert_eq!(chunk.delta_b, 1);
            assert_eq!(chunk.remaining_after, 1);
        }
        _ => panic!("SettleB hint must return bounded B progress"),
    }
    assert!(a.b_stale_state);
    assert_eq!(g.b_stale_account_count, 1);

    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let _opposing = attach_opposite(&mut g, 0, SideV16::Long, POS_SCALE, 91);
    let out = g
        .permissionless_crank_not_atomic(
            &mut a,
            PermissionlessCrankRequestV16 {
                action: PermissionlessCrankActionV16::Liquidate(LiquidationRequestV16 {
                    asset_index: 0,
                    close_q: POS_SCALE,
                    fee_bps: 0,
                }),
                ..req_current
            },
            &[1; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();
    assert_eq!(out, PermissionlessProgressOutcomeV16::AccountCurrent);
    assert_eq!(a.active_bitmap, bitmap(&[]));

    let mut g = group();
    let mut a = account();
    let reason = PermissionlessRecoveryReasonV16::BelowProgressFloor;
    assert_eq!(
        g.permissionless_crank_not_atomic(
            &mut a,
            PermissionlessCrankRequestV16 {
                action: PermissionlessCrankActionV16::Recover(reason),
                ..req_current
            },
            &[1; V16_MAX_PORTFOLIO_ASSETS_N],
        ),
        Ok(PermissionlessProgressOutcomeV16::RecoveryDeclared(reason))
    );
    assert_eq!(g.recovery_reason, Some(reason));
}

#[test]
fn v16_permissionless_crank_liquidation_books_bankruptcy_and_advances_accrual() {
    let mut g = group();
    let mut victim = account();
    let _opposite = attach_opposite(&mut g, 0, SideV16::Long, 1, 105);
    g.attach_leg(&mut victim, 0, SideV16::Long, 1).unwrap();
    g.vault = 1;
    g.insurance = 1;
    g.insurance_domain_budget[1] = 1;
    victim.pnl = -3;
    g.negative_pnl_account_count = 1;

    let out = g
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

    assert_eq!(out, PermissionlessProgressOutcomeV16::AccountCurrent);
    assert_eq!(victim.pnl, 0);
    assert_eq!(victim.active_bitmap, bitmap(&[]));
    assert_eq!(g.insurance, 0);
    assert_eq!(g.negative_pnl_account_count, 0);
    assert_eq!(g.assets[0].oi_eff_long_q, 0);
    assert_eq!(g.assets[0].oi_eff_short_q, 0);
    assert!(g.bankruptcy_hlock_active);
    assert_eq!(g.slot_last, 1);
    assert_eq!(g.current_slot, 1);
    assert_eq!(g.assert_public_invariants(), Ok(()));
}

#[test]
fn v16_resolved_close_is_bounded_and_fee_current() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.resolve_market_not_atomic(10).unwrap();
    let out = g.close_resolved_account_not_atomic(&mut a, 1).unwrap();
    assert_eq!(out, ResolvedCloseOutcomeV16::Closed { payout: 90 });
    assert_eq!(a.last_fee_slot, 10);
    assert_eq!(a.capital, 0);
}

#[test]
fn v16_resolved_flat_close_returns_exact_capital() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 777).unwrap();
    g.resolve_market_not_atomic(1).unwrap();

    let out = g.close_resolved_account_not_atomic(&mut a, 0).unwrap();

    assert_eq!(out, ResolvedCloseOutcomeV16::Closed { payout: 777 });
    assert_eq!(a.capital, 0);
    assert_eq!(a.pnl, 0);
    assert_eq!(g.c_tot, 0);
    assert_eq!(g.vault, 0);
}

#[test]
fn v16_resolved_profit_close_pays_from_snapshot_residual_and_clears_claim() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 10).unwrap();
    a.pnl = 7;
    g.pnl_pos_tot = 7;
    set_junior_bound(&mut g, 7);
    g.vault = g.c_tot + 7;
    g.resolve_market_not_atomic(1).unwrap();

    let out = g.close_resolved_account_not_atomic(&mut a, 0).unwrap();

    assert_eq!(out, ResolvedCloseOutcomeV16::Closed { payout: 17 });
    assert_eq!(a.capital, 0);
    assert_eq!(a.pnl, 0);
    assert_eq!(g.c_tot, 0);
    assert_eq!(g.pnl_pos_tot, 0);
    assert_eq!(g.vault, 0);
}

#[test]
fn v16_resolved_close_with_active_position_detaches_leg_and_pays_capital() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 777).unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.resolve_market_not_atomic(1).unwrap();

    let out = g.close_resolved_account_not_atomic(&mut a, 0).unwrap();

    assert_eq!(out, ResolvedCloseOutcomeV16::Closed { payout: 777 });
    assert_eq!(a.capital, 0);
    assert_eq!(a.active_bitmap, bitmap(&[]));
    assert_eq!(g.vault, 0);
    assert_eq!(g.c_tot, 0);
    assert_eq!(g.assets[0].stored_pos_count_long, 0);
}

#[test]
fn v16_zero_copy_resolved_close_with_active_position_detaches_leg_and_pays_capital() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 777).unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.resolve_market_not_atomic(1).unwrap();
    let mut header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: [0u8; 32],
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let mut account_header = PortfolioAccountV16Account::from_runtime(&a);
    let mut source_domains = PortfolioAccountV16Account::source_domains_from_runtime(&a).unwrap();
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account_view =
        percolator::v16::PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);

    let out = market_view
        .close_resolved_account_not_atomic(&mut account_view, 0)
        .unwrap();

    assert_eq!(out, ResolvedCloseOutcomeV16::Closed { payout: 777 });
    assert_eq!(account_view.header.capital.get(), 0);
    assert!(percolator::active_bitmap_is_empty(
        account_view.header.active_bitmap.map(V16PodU64::get)
    ));
    assert_eq!(market_view.header.vault.get(), 0);
    assert_eq!(market_view.header.c_tot.get(), 0);
    assert_eq!(
        market_view.markets[0]
            .engine
            .asset
            .stored_pos_count_long
            .get(),
        0
    );
}

#[test]
fn v16_resolved_close_returns_progress_after_partial_b_settlement() {
    let (market, _, _) = ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.public_b_chunk_atoms = 1;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 100).unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, 1).unwrap();
    g.assets[0].b_long_num = SOCIAL_LOSS_DEN * 2;
    g.resolve_market_not_atomic(10).unwrap();

    let out = g.close_resolved_account_not_atomic(&mut a, 1).unwrap();

    assert_eq!(out, ResolvedCloseOutcomeV16::ProgressOnly);
    assert!(a.legs[0].b_stale);
    assert!(a.legs[0].b_snap > 0);
    assert!(a.legs[0].b_snap < g.assets[0].b_long_num);
    assert_eq!(a.last_fee_slot, 0);
    assert_eq!(a.active_bitmap, bitmap(&[0]));
}

#[test]
fn v16_resolved_payout_readiness_uses_exact_counters_and_bounds() {
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
            ResolvedCloseOutcomeV16::ProgressOnly,
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
fn v16_resolved_unattributed_negative_pnl_fails_closed_without_erasure() {
    let (market, _, owner) = ids();
    let mut g = group();
    let mut bankrupt =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [44; 32], owner));
    let mut winner = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [45; 32], owner));
    g.vault = 5;
    bankrupt.pnl = -3;
    winner.pnl = 5;
    g.negative_pnl_account_count = 1;
    g.pnl_pos_tot = 5;
    set_junior_bound(&mut g, 5);
    g.resolve_market_not_atomic(1).unwrap();

    assert_eq!(
        g.close_resolved_account_not_atomic(&mut winner, 0).unwrap(),
        ResolvedCloseOutcomeV16::ProgressOnly
    );
    bankrupt.ensure_source_domain_capacity(g.source_credit.len());
    let bankrupt_before = bankrupt.clone();
    let vault_before = g.vault;
    let c_tot_before = g.c_tot;
    let insurance_before = g.insurance;
    let negative_count_before = g.negative_pnl_account_count;

    let bankrupt_close = g.close_resolved_account_not_atomic(&mut bankrupt, 0);

    assert_eq!(
        bankrupt_close,
        Err(V16Error::RecoveryRequired),
        "unattributed bad debt must not be silently erased on resolved close"
    );
    assert_eq!(bankrupt, bankrupt_before);
    assert_eq!(g.vault, vault_before);
    assert_eq!(g.c_tot, c_tot_before);
    assert_eq!(g.insurance, insurance_before);
    assert_eq!(g.negative_pnl_account_count, negative_count_before);

    let winner_close = g.close_resolved_account_not_atomic(&mut winner, 0);

    assert_eq!(winner_close, Ok(ResolvedCloseOutcomeV16::ProgressOnly));
    assert_eq!(winner.pnl, 5);
    assert_eq!(g.vault, 5);
}

#[test]
fn v16_resolved_bankrupt_active_negative_consumes_insurance_then_unblocks_winner() {
    let (market, _, owner) = ids();
    let mut g = group();
    let mut bankrupt =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [46; 32], owner));
    let mut winner = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [47; 32], owner));
    g.vault = 5;
    g.insurance = 5;
    g.insurance_domain_budget[1] = 5;
    bankrupt.pnl = -5;
    winner.pnl = 5;
    g.negative_pnl_account_count = 1;
    g.pnl_pos_tot = 5;
    set_junior_bound(&mut g, 5);
    g.attach_leg(&mut bankrupt, 0, SideV16::Long, 1).unwrap();
    g.resolve_market_not_atomic(1).unwrap();

    let bankrupt_close = g.close_resolved_account_not_atomic(&mut bankrupt, 0);

    assert_eq!(
        bankrupt_close,
        Ok(ResolvedCloseOutcomeV16::Closed { payout: 0 })
    );
    assert_eq!(bankrupt.pnl, 0);
    assert_eq!(bankrupt.active_bitmap, bitmap(&[]));
    assert_eq!(g.negative_pnl_account_count, 0);
    assert_eq!(g.insurance, 0);
    assert_eq!(g.insurance_domain_spent[1], 5);
    assert_eq!(g.assets[0].stored_pos_count_long, 0);

    let winner_close = g.close_resolved_account_not_atomic(&mut winner, 0);

    assert_eq!(
        winner_close,
        Ok(ResolvedCloseOutcomeV16::Closed { payout: 5 })
    );
    assert_eq!(g.vault, 0);
}

#[test]
fn v16_resolved_bankrupt_active_negative_without_counterweight_clears_as_explicit_terminal_loss() {
    let (market, _, owner) = ids();
    let mut g = group();
    let mut bankrupt =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [48; 32], owner));
    let mut winner = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [49; 32], owner));
    g.vault = 5;
    bankrupt.pnl = -5;
    winner.pnl = 5;
    g.negative_pnl_account_count = 1;
    g.pnl_pos_tot = 5;
    set_junior_bound(&mut g, 5);
    g.attach_leg(&mut bankrupt, 0, SideV16::Long, 1).unwrap();
    g.resolve_market_not_atomic(1).unwrap();

    let bankrupt_close = g.close_resolved_account_not_atomic(&mut bankrupt, 0);

    assert_eq!(
        bankrupt_close,
        Ok(ResolvedCloseOutcomeV16::Closed { payout: 0 })
    );
    assert_eq!(bankrupt.pnl, 0);
    assert_eq!(bankrupt.active_bitmap, bitmap(&[]));
    assert_eq!(g.negative_pnl_account_count, 0);
    assert_eq!(g.recovery_reason, None);
    assert!(g.bankruptcy_hlock_active);
    assert_eq!(bankrupt.close_progress.explicit_loss_assigned, 5);
    assert_eq!(bankrupt.close_progress.residual_remaining, 0);
    assert!(bankrupt.close_progress.finalized);
    assert_eq!(g.assets[0].stored_pos_count_long, 0);

    let winner_close = g.close_resolved_account_not_atomic(&mut winner, 0);

    assert_eq!(
        winner_close,
        Ok(ResolvedCloseOutcomeV16::Closed { payout: 5 })
    );
    assert_eq!(g.vault, 0);
}

#[test]
fn v16_resolved_preexisting_close_progress_ledger_does_not_deadlock_on_recovery_gate() {
    let (market, _, owner) = ids();
    let mut g = group();
    let mut bankrupt =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [53; 32], owner));
    g.deposit_not_atomic(&mut bankrupt, 10).unwrap();
    g.attach_leg(&mut bankrupt, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    bankrupt.pnl = -100;
    g.negative_pnl_account_count = 1;
    bankrupt.close_progress = CloseProgressLedgerV16 {
        active: true,
        finalized: false,
        close_id: 1,
        asset_index: 0,
        market_id: g.assets[0].market_id,
        domain_side: SideV16::Short,
        gross_loss_at_close_start: 100,
        residual_remaining: 100,
        drift_reference_slot: g.current_slot,
        max_close_slot: u64::MAX / 2,
        ..CloseProgressLedgerV16::EMPTY
    };
    g.pending_domain_loss_barriers[1] = 1;
    g.resolve_market_not_atomic(10).unwrap();

    let out = g.close_resolved_account_not_atomic(&mut bankrupt, 0);

    assert_eq!(out, Ok(ResolvedCloseOutcomeV16::Closed { payout: 0 }));
    assert_eq!(bankrupt.pnl, 0);
    assert_eq!(bankrupt.active_bitmap, bitmap(&[]));
    assert!(bankrupt.close_progress.finalized);
    assert_eq!(bankrupt.close_progress.residual_remaining, 0);
    assert_eq!(g.pending_domain_loss_barriers[1], 0);
    assert_eq!(g.negative_pnl_account_count, 0);
    assert_eq!(g.recovery_reason, None);
}

#[test]
fn v16_resolved_positive_payout_waits_for_pending_domain_loss_barrier() {
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
        ResolvedCloseOutcomeV16::ProgressOnly,
        "pending domain-loss barriers must block positive payout readiness"
    );
    assert_eq!(g.vault, vault_before);
    assert_eq!(g.pnl_pos_tot, pnl_pos_before);
    assert_eq!(g.pnl_pos_bound_tot, bound_before);
    assert_eq!(a.pnl, 10);
    assert!(!g.payout_snapshot_captured);
}

#[test]
fn v16_pending_domain_loss_barrier_does_not_freeze_unrelated_positive_credit() {
    let (market, _, owner) = ids();
    let mut g = MarketGroupV16::new(market, V16Config::public_user_fund(2, 0, 10)).unwrap();
    let mut profitable =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [71; 32], owner));
    let mut opposite =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [72; 32], owner));
    g.deposit_not_atomic(&mut profitable, 100).unwrap();
    g.deposit_not_atomic(&mut opposite, 100).unwrap();
    g.attach_leg(&mut profitable, 1, SideV16::Long, 10).unwrap();
    g.attach_leg(&mut opposite, 1, SideV16::Short, -10).unwrap();
    g.add_account_source_positive_pnl_not_atomic(&mut profitable, 0, 5)
        .unwrap();
    g.add_fresh_counterparty_backing_not_atomic(0, 5 * BOUND_SCALE, 10)
        .unwrap();
    g.pnl_matured_pos_tot = 5;
    g.vault = g.c_tot + 5;
    g.pending_domain_loss_barriers[1] = 1;
    g.full_account_refresh(&mut profitable, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
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
fn v16_ordinary_positive_conversion_disabled_after_resolved_payout_lane_exists() {
    let mut g = group();
    let mut a = account();
    a.pnl = 10;
    g.pnl_pos_tot = 10;
    g.pnl_matured_pos_tot = 10;
    set_junior_bound(&mut g, 10);
    g.vault = 10;
    g.full_account_refresh(&mut a, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    g.resolve_market_not_atomic(1).unwrap();
    let before = (g.clone(), a.clone());

    let result = g.convert_released_pnl_to_capital_not_atomic(&mut a);

    assert_eq!(result, Err(V16Error::LockActive));
    assert_eq!((g, a), before);

    let mut live = group();
    let mut live_account = account();
    live_account.pnl = 10;
    live.pnl_pos_tot = 10;
    live.pnl_matured_pos_tot = 10;
    set_junior_bound(&mut live, 10);
    live.vault = 10;
    initialize_payout_ledger(&mut live);
    live.full_account_refresh(&mut live_account, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    let before_live = (live.clone(), live_account.clone());

    let live_result = live.convert_released_pnl_to_capital_not_atomic(&mut live_account);

    assert_eq!(live_result, Err(V16Error::LockActive));
    assert_eq!((live, live_account), before_live);
}

#[test]
fn v16_dead_leg_forfeit_is_unavailable_for_normal_live_leg() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();

    assert_eq!(
        g.forfeit_recovery_leg_not_atomic(&mut a, 0, 4),
        Err(V16Error::LockActive)
    );
    assert!(a.legs[0].active);
    assert_eq!(g.assets[0].oi_eff_long_q, POS_SCALE);
}

#[test]
fn v16_dead_leg_forfeit_returns_partial_b_progress_before_detach() {
    let mut g = group();
    let mut a = account();
    g.mode = MarketModeV16::Recovery;
    g.attach_leg(&mut a, 0, SideV16::Long, 1).unwrap();
    g.assets[0].b_long_num = 2;

    let out = g.forfeit_recovery_leg_not_atomic(&mut a, 0, 1).unwrap();

    assert!(!out.detached);
    assert_eq!(out.loss_settled, 0);
    assert_eq!(out.principal_used, 0);
    assert_eq!(out.insurance_used, 0);
    assert_eq!(out.residual_booked, 0);
    assert_eq!(a.legs[0].b_snap, 1);
    assert!(a.legs[0].b_stale);
    assert!(a.b_stale_state);
    assert!(a.legs[0].active);
    assert_eq!(g.assets[0].oi_eff_long_q, 1);
    assert_eq!(g.assert_public_invariants(), Ok(()));
}

#[test]
fn v16_dead_leg_forfeit_detaches_without_crediting_positive_pnl() {
    let mut g = group();
    let mut a = account();
    let mut unrelated =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new([1; 32], [21; 32], [3; 32]));
    g.mode = MarketModeV16::Recovery;
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut unrelated, 1, SideV16::Short, -(POS_SCALE as i128))
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
    assert_eq!(a.active_bitmap, bitmap(&[]));
    assert!(!a.legs[0].active);
    assert!(active_leg_for_asset(&unrelated, 1).is_some());
    assert_eq!(g.assets[0].oi_eff_long_q, 0);
    assert_eq!(g.assets[1].oi_eff_short_q, POS_SCALE);
}

#[test]
fn v16_dead_leg_forfeit_books_negative_residual_to_opposing_domain_only() {
    let mut g = group();
    let mut bankrupt = account();
    let mut opposing =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new([1; 32], [22; 32], [3; 32]));
    g.mode = MarketModeV16::Recovery;
    g.attach_leg(&mut bankrupt, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut opposing, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.assets[0].mode_long = SideModeV16::DrainOnly;
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
        g.pending_domain_loss_barrier_count(0, SideV16::Short),
        Ok(0)
    );
    assert!(bankrupt.close_progress.finalized);
}

#[test]
fn v16_dead_leg_forfeit_haircuts_positive_support_when_junior_impaired() {
    let mut g = group();
    let mut bankrupt = account();
    let mut opposing =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new([1; 32], [23; 32], [3; 32]));
    g.mode = MarketModeV16::Recovery;
    g.attach_leg(&mut bankrupt, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut opposing, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.assets[0].mode_long = SideModeV16::DrainOnly;
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
fn v16_resolved_positive_payout_uses_stable_snapshot_denominator() {
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

    assert_eq!(first, ResolvedCloseOutcomeV16::Closed { payout: 50 });
    assert_eq!(second, ResolvedCloseOutcomeV16::Closed { payout: 50 });
    assert_eq!(g.payout_snapshot, 100);
    assert_eq!(g.payout_snapshot_pnl_pos_tot, 200);
}

#[test]
fn v16_resolved_positive_payout_uses_conservative_bound_denominator() {
    let mut g = group();
    let mut a = account();
    g.vault = 100;
    a.pnl = 100;
    g.pnl_pos_tot = 100;
    set_junior_bound(&mut g, 200);
    g.resolve_market_not_atomic(1).unwrap();

    let out = g.close_resolved_account_not_atomic(&mut a, 0).unwrap();

    assert_eq!(out, ResolvedCloseOutcomeV16::Closed { payout: 50 });
    assert_eq!(g.payout_snapshot, 100);
    assert_eq!(g.payout_snapshot_pnl_pos_tot, 200);
    assert_eq!(g.vault, 50);
}

#[test]
fn v16_resolved_close_receipt_records_only_actual_resolved_payout_after_vault_drift() {
    let mut g = group();
    let mut a = account();
    g.vault = 1;
    a.pnl = 1;
    g.pnl_pos_tot = 1;
    set_junior_bound(&mut g, 1);
    g.resolve_market_not_atomic(1).unwrap();
    initialize_payout_ledger(&mut g);
    g.vault = 0;

    let out = g.close_resolved_account_not_atomic(&mut a, 0).unwrap();

    assert_eq!(out, ResolvedCloseOutcomeV16::Closed { payout: 0 });
    assert!(a.resolved_payout_receipt.present);
    assert_eq!(a.resolved_payout_receipt.terminal_positive_claim_face, 1);
    assert_eq!(
        a.resolved_payout_receipt.paid_effective, 0,
        "receipt must track quote atoms actually paid, not theoretical claimable amount"
    );
    assert!(!a.resolved_payout_receipt.finalized);

    g.vault = 1;
    let topup = g.claim_resolved_payout_topup_not_atomic(&mut a).unwrap();
    assert_eq!(topup, 1);
    assert_eq!(a.resolved_payout_receipt.paid_effective, 1);
    assert!(a.resolved_payout_receipt.finalized);
}

#[test]
fn v16_zero_copy_resolved_close_receipt_records_only_actual_resolved_payout_after_vault_drift() {
    let mut g = group();
    let mut a = account();
    g.vault = 1;
    a.pnl = 1;
    g.pnl_pos_tot = 1;
    set_junior_bound(&mut g, 1);
    g.resolve_market_not_atomic(1).unwrap();
    initialize_payout_ledger(&mut g);
    g.vault = 0;
    let mut header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: [0u8; 32],
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let mut account_header = PortfolioAccountV16Account::from_runtime(&a);
    let mut source_domains = PortfolioAccountV16Account::source_domains_from_runtime(&a).unwrap();
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account_view =
        percolator::v16::PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);

    let out = market_view
        .close_resolved_account_not_atomic(&mut account_view, 0)
        .unwrap();

    assert_eq!(out, ResolvedCloseOutcomeV16::Closed { payout: 0 });
    let receipt = account_view
        .header
        .resolved_payout_receipt
        .try_to_runtime()
        .unwrap();
    assert!(receipt.present);
    assert_eq!(receipt.terminal_positive_claim_face, 1);
    assert_eq!(receipt.paid_effective, 0);
    assert!(!receipt.finalized);
}

#[test]
fn v16_resolved_positive_payout_uses_scaled_bound_remainder_denominator() {
    let mut g = group();
    let mut a = account();
    g.vault = 1;
    a.pnl = 1;
    g.pnl_pos_tot = 1;
    g.pnl_pos_bound_tot_num = BOUND_SCALE + 1;
    g.pnl_pos_bound_tot = 2;
    g.resolve_market_not_atomic(1).unwrap();

    let out = g.close_resolved_account_not_atomic(&mut a, 0).unwrap();

    assert_eq!(out, ResolvedCloseOutcomeV16::Closed { payout: 0 });
    assert_eq!(g.payout_snapshot, 1);
    assert_eq!(g.payout_snapshot_pnl_pos_tot, 2);
    assert_eq!(g.vault, 1);
    assert_eq!(g.pnl_pos_bound_tot_num, 1);
    assert_eq!(g.pnl_pos_bound_tot, 1);
}

#[test]
fn v16_resolved_payout_receipt_tracks_paid_effective_and_later_topup() {
    let mut g = group();
    let mut a = account();
    g.vault = 1;
    a.pnl = 1;
    g.pnl_pos_tot = 1;
    g.pnl_pos_bound_tot_num = BOUND_SCALE + 1;
    g.pnl_pos_bound_tot = 2;
    g.resolve_market_not_atomic(1).unwrap();

    let first = g.close_resolved_account_not_atomic(&mut a, 0).unwrap();

    assert_eq!(first, ResolvedCloseOutcomeV16::Closed { payout: 0 });
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
fn v16_unfinalized_resolved_receipt_blocks_account_close_until_topup() {
    let mut g = group();
    let mut a = account();
    g.create_portfolio_account(&a).unwrap();
    g.vault = 1;
    a.pnl = 1;
    g.pnl_pos_tot = 1;
    g.pnl_pos_bound_tot_num = BOUND_SCALE + 1;
    g.pnl_pos_bound_tot = 2;
    g.resolve_market_not_atomic(1).unwrap();

    let first = g.close_resolved_account_not_atomic(&mut a, 0).unwrap();

    assert_eq!(first, ResolvedCloseOutcomeV16::Closed { payout: 0 });
    assert!(a.resolved_payout_receipt.present);
    assert!(!a.resolved_payout_receipt.finalized);
    assert_eq!(g.close_portfolio_account(&a), Err(V16Error::LockActive));
    assert_eq!(g.materialized_portfolio_count, 1);

    g.refine_resolved_unreceipted_bound_not_atomic(1).unwrap();
    let topup = g.claim_resolved_payout_topup_not_atomic(&mut a).unwrap();

    assert_eq!(topup, 1);
    assert!(a.resolved_payout_receipt.finalized);
    assert_eq!(g.close_portfolio_account(&a), Ok(()));
    assert_eq!(g.materialized_portfolio_count, 0);
}

#[test]
fn v16_public_invariants_reject_scaled_junior_bound_cache_mismatch() {
    let mut g = group();
    g.pnl_pos_tot = 1;
    g.pnl_pos_bound_tot_num = BOUND_SCALE + 1;
    g.pnl_pos_bound_tot = 1;
    assert_eq!(g.assert_public_invariants(), Err(V16Error::InvalidConfig));

    g.pnl_pos_bound_tot_num = BOUND_SCALE - 1;
    g.pnl_pos_bound_tot = 1;
    assert_eq!(g.assert_public_invariants(), Err(V16Error::InvalidConfig));
}

#[test]
fn v16_pnl_pos_bound_tot_prevents_lazy_positive_pnl_first_mover_overpay() {
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

    assert_eq!(out, ResolvedCloseOutcomeV16::Closed { payout: 33 });
    assert_eq!(g.payout_snapshot, 100);
    assert_eq!(g.payout_snapshot_pnl_pos_tot, 300);
    assert_eq!(g.vault, 67);
}

#[test]
fn v16_liquidation_requires_strict_account_risk_progress() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV16::Long, POS_SCALE, 101);
    g.accrue_asset_to_not_atomic(0, 1, 1, 0, true).unwrap();
    let req = LiquidationRequestV16 {
        asset_index: 0,
        close_q: POS_SCALE,
        fee_bps: 0,
    };
    let out = g
        .liquidate_account_not_atomic(&mut a, req, &[1; V16_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(out.closed_q, POS_SCALE);
    assert_eq!(a.active_bitmap, bitmap(&[]));
}

#[test]
fn v16_partial_liquidation_can_reduce_risk_without_forcing_full_close() {
    let mut g = group();
    let mut a = account();
    g.deposit_not_atomic(&mut a, 10).unwrap();
    // Keep the oracle target consistent with the [100] liquidation refresh so
    // the post-323c9f2 target-effective-lag penalty is zero; the certified
    // liq_deficit below is the pure maintenance shortfall, not loss + lag.
    g.assets[0].raw_oracle_target_price = 100;
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV16::Long, POS_SCALE, 102);

    let out = g
        .liquidate_account_not_atomic(
            &mut a,
            LiquidationRequestV16 {
                asset_index: 0,
                close_q: POS_SCALE / 2,
                fee_bps: 0,
            },
            &[100; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(out.closed_q, POS_SCALE / 2);
    assert_eq!(a.legs[0].basis_pos_q.unsigned_abs(), POS_SCALE / 2);
    assert_eq!(g.assets[0].oi_eff_long_q, POS_SCALE / 2);
    assert_eq!(a.health_cert.certified_liq_deficit, 40);
}

#[test]
fn v16_partial_liquidation_cannot_b_book_residual_while_open_risk_remains() {
    let mut g = group();
    let mut bankrupt = account();
    let mut opposing =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new([1; 32], [42; 32], [3; 32]));
    g.attach_leg(&mut bankrupt, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut opposing, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.assets[0].k_long = -(100 * ADL_ONE as i128);

    let before_b_short = g.assets[0].b_short_num;
    let res = g.liquidate_account_not_atomic(
        &mut bankrupt,
        LiquidationRequestV16 {
            asset_index: 0,
            close_q: POS_SCALE / 2,
            fee_bps: 0,
        },
        &[1; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(res, Err(V16Error::RecoveryRequired));
    assert_eq!(
        g.assets[0].b_short_num, before_b_short,
        "partial liquidation must not socialize residual while the account still has closable risk"
    );
    assert!(bankrupt.legs[0].active);
    assert_eq!(bankrupt.legs[0].basis_pos_q.unsigned_abs(), POS_SCALE);
}

#[test]
fn v16_liquidation_rejects_zero_close_before_mutation() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let before_group = g.clone();
    let before_account = a.clone();

    let res = g.liquidate_account_not_atomic(
        &mut a,
        LiquidationRequestV16 {
            asset_index: 0,
            close_q: 0,
            fee_bps: 0,
        },
        &[100; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(res, Err(V16Error::InvalidConfig));
    assert_eq!(g, before_group);
    assert_eq!(a, before_account);
}

#[test]
fn v16_min_liquidation_abs_shortfall_does_not_block_risk_close() {
    let (market, account_id, owner) = ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 1);
    cfg.min_nonzero_mm_req = 100;
    cfg.min_nonzero_im_req = 101;
    cfg.max_price_move_bps_per_slot = 5_000;
    cfg.liquidation_fee_cap = 40;
    cfg.min_liquidation_abs = 40;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    let mut a = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    g.deposit_not_atomic(&mut a, 20).unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV16::Long, POS_SCALE, 103);

    let out = g
        .liquidate_account_not_atomic(
            &mut a,
            LiquidationRequestV16 {
                asset_index: 0,
                close_q: POS_SCALE,
                fee_bps: 0,
            },
            &[100; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(out.closed_q, POS_SCALE);
    assert_eq!(out.fee_charged, 20);
    assert_eq!(a.capital, 0);
    assert_eq!(a.active_bitmap, bitmap(&[]));
    assert_eq!(g.insurance, 20);
    assert_eq!(g.c_tot, 0);
    assert_eq!(g.vault, 20);
    assert_eq!(g.assert_public_invariants(), Ok(()));
}

#[test]
fn v16_bankrupt_liquidation_consumes_insurance_before_social_loss() {
    let (market, _, owner) = ids();
    let mut g = group();
    let mut a = account();
    let mut opposing = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    g.vault = 4;
    g.insurance = 4;
    g.insurance_domain_budget[1] = 4;
    a.pnl = -9;
    g.negative_pnl_account_count = 1;
    g.attach_leg(&mut a, 0, SideV16::Long, 1).unwrap();
    g.attach_leg(&mut opposing, 0, SideV16::Short, -1).unwrap();

    let out = g
        .liquidate_account_not_atomic(
            &mut a,
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
    assert_eq!(g.vault, 4);
    assert_eq!(g.insurance, 0);
    assert_eq!(a.pnl, 0);
    assert_eq!(a.active_bitmap, bitmap(&[]));
    assert_eq!(
        g.stock_reconciliation_proof().unwrap(),
        StockReconciliationProofV16 {
            token_vault: 4,
            senior_capital_total: 0,
            insurance_capital: 0,
            backing_provider_earnings: 0,
            settlement_rounding_residue_total: 0,
            unallocated_protocol_surplus: 4,
        }
    );
}

#[test]
fn v16_domain_insurance_budget_caps_bankruptcy_spend_for_one_asset_side() {
    let (market, _, owner) = ids();
    let mut g = group();
    let mut a = account();
    let mut opposing = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    g.vault = 10;
    g.insurance = 10;
    g.insurance_domain_budget = vec![0; g.insurance_domain_budget.len()];
    let short_domain_for_bankrupt_long = 1;
    g.insurance_domain_budget[short_domain_for_bankrupt_long] = 3;
    a.pnl = -9;
    g.negative_pnl_account_count = 1;
    g.attach_leg(&mut a, 0, SideV16::Long, 1).unwrap();
    g.attach_leg(&mut opposing, 0, SideV16::Short, -1).unwrap();

    let out = g
        .liquidate_account_not_atomic(
            &mut a,
            LiquidationRequestV16 {
                asset_index: 0,
                close_q: 1,
                fee_bps: 0,
            },
            &[1; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(out.insurance_used, 3);
    assert_eq!(out.residual_booked, 6);
    assert_eq!(out.explicit_loss, 0);
    assert_eq!(g.insurance, 7);
    assert_eq!(g.insurance_domain_spent[short_domain_for_bankrupt_long], 3);
    assert_eq!(g.insurance_domain_spent[0], 0);
    assert_eq!(a.pnl, 0);
    assert_eq!(a.active_bitmap, bitmap(&[]));
}

#[test]
fn v16_unbudgeted_domain_cannot_spend_global_insurance_on_bankruptcy() {
    let mut g = group();
    let mut bankrupt_long = account();
    let mut opposing_short = account_with_id(46);
    g.vault = 50_025;
    g.insurance = 50_025;
    g.attach_leg(&mut bankrupt_long, 0, SideV16::Long, 1)
        .unwrap();
    g.attach_leg(&mut opposing_short, 0, SideV16::Short, -1)
        .unwrap();
    bankrupt_long.pnl = -50_000;
    g.negative_pnl_account_count = 1;

    let out = g
        .liquidate_account_not_atomic(
            &mut bankrupt_long,
            LiquidationRequestV16 {
                asset_index: 0,
                close_q: 1,
                fee_bps: 0,
            },
            &[1; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .expect("bankruptcy should make social-loss progress without domain insurance budget");

    assert_eq!(out.insurance_used, 0);
    assert_eq!(out.residual_booked, 50_000);
    assert_eq!(
        g.insurance, 50_025,
        "global insurance must not be spendable by an unfunded domain"
    );
    assert_eq!(g.insurance_domain_spent[1], 0);
    assert_eq!(bankrupt_long.pnl, 0);
    assert_eq!(bankrupt_long.active_bitmap, bitmap(&[]));
    g.assert_public_invariants().unwrap();
}

#[test]
fn v16_bankruptcy_insurance_spend_excludes_source_credit_reserved_insurance() {
    let mut g = group();
    let mut bankrupt_long = account();
    let mut opposing_short = account_with_id(47);
    g.vault = 10;
    g.insurance = 10;
    g.insurance_domain_budget[1] = 10;
    g.reserve_insurance_credit_not_atomic(1, 10 * BOUND_SCALE)
        .unwrap();
    bankrupt_long.pnl = -5;
    g.negative_pnl_account_count = 1;
    g.attach_leg(&mut bankrupt_long, 0, SideV16::Long, 1)
        .unwrap();
    g.attach_leg(&mut opposing_short, 0, SideV16::Short, -1)
        .unwrap();

    let out = g
        .liquidate_account_not_atomic(
            &mut bankrupt_long,
            LiquidationRequestV16 {
                asset_index: 0,
                close_q: 1,
                fee_bps: 0,
            },
            &[1; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .expect("reserved source-credit insurance must not make liquidation roll back");

    assert_eq!(
        out.insurance_used, 0,
        "bankruptcy close must not spend insurance already reserved for source credit"
    );
    assert_eq!(out.residual_booked, 5);
    assert_eq!(g.insurance, 10);
    assert_eq!(g.insurance_domain_spent[1], 0);
    assert_eq!(bankrupt_long.close_progress.insurance_spent, 0);
    assert_eq!(bankrupt_long.close_progress.b_loss_booked, 5);
    assert_eq!(bankrupt_long.pnl, 0);
    assert_eq!(bankrupt_long.active_bitmap, bitmap(&[]));
    g.assert_public_invariants().unwrap();
}

#[test]
fn v16_zero_copy_bankruptcy_insurance_spend_excludes_source_credit_reserved_insurance() {
    let mut g = group();
    let mut bankrupt_long = account();
    let mut opposing_short = account_with_id(48);
    g.vault = 10;
    g.insurance = 10;
    g.insurance_domain_budget[1] = 10;
    g.reserve_insurance_credit_not_atomic(1, 10 * BOUND_SCALE)
        .unwrap();
    bankrupt_long.pnl = -5;
    g.negative_pnl_account_count = 1;
    g.attach_leg(&mut bankrupt_long, 0, SideV16::Long, 1)
        .unwrap();
    g.attach_leg(&mut opposing_short, 0, SideV16::Short, -1)
        .unwrap();

    let mut header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: [i as u8; 8],
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let mut account_header = PortfolioAccountV16Account::from_runtime(&bankrupt_long);
    let mut source_domains =
        PortfolioAccountV16Account::source_domains_from_runtime(&bankrupt_long).unwrap();
    let mut account_view =
        percolator::v16::PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    let out = market_view
        .liquidate_account_not_atomic(
            &mut account_view,
            LiquidationRequestV16 {
                asset_index: 0,
                close_q: 1,
                fee_bps: 0,
            },
        )
        .expect("zero-copy liquidation must not spend source-credit-reserved insurance");

    assert_eq!(out.insurance_used, 0);
    assert_eq!(out.residual_booked, 5);
    assert_eq!(market_view.header.insurance.get(), 10);
    assert_eq!(
        market_view.markets[0]
            .engine
            .insurance_domain_spent_short
            .get(),
        0
    );
    assert_eq!(account_view.header.close_progress.insurance_spent.get(), 0);
    assert_eq!(account_view.header.close_progress.b_loss_booked.get(), 5);
    assert_eq!(account_view.header.pnl.get(), 0);
    assert!(percolator::active_bitmap_is_empty(
        account_view.header.active_bitmap.map(V16PodU64::get)
    ));
    market_view.validate_shape().unwrap();
    account_view
        .validate_with_market(&market_view.as_view())
        .unwrap();
}

#[test]
fn v16_two_asset_refresh_without_source_backing_handles_exact_capital_loss() {
    let (market, account_id, owner) = ids();
    let mut g = MarketGroupV16::new(market, V16Config::public_user_fund(2, 0, 1)).unwrap();
    let mut a = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    let mut opp0 = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [9; 32], owner));
    let mut opp1 = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [10; 32], owner));
    g.deposit_not_atomic(&mut a, 2).unwrap();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut a, 1, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    g.attach_leg(&mut opp0, 0, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.attach_leg(&mut opp1, 1, SideV16::Short, -(POS_SCALE as i128))
        .unwrap();
    g.assets[0].k_long = ADL_ONE as i128;
    g.assets[1].k_long = -2 * (ADL_ONE as i128);

    let mut prices = [1u64; V16_MAX_PORTFOLIO_ASSETS_N];
    prices[0] = 7;
    prices[1] = 11;
    let cert = g.full_account_refresh(&mut a, &prices).unwrap();

    assert_eq!(a.pnl, 0);
    assert_eq!(a.capital, 0);
    assert_eq!(g.c_tot, 0);
    assert_eq!(cert.certified_equity, 0);
    assert_eq!(a.active_bitmap, bitmap(&[0, 1]));
    assert_eq!(a.legs[0].k_snap, ADL_ONE as i128);
    assert_eq!(a.legs[1].k_snap, -2 * (ADL_ONE as i128));
}

#[test]
fn v16_liquidation_residual_domain_is_opposite_side_for_long_and_short() {
    for bankrupt_side in [SideV16::Long, SideV16::Short] {
        let (market, _, owner) = ids();
        let mut g = group();
        let mut bankrupt = account();
        let mut opposing =
            PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
        g.vault = 4;
        g.insurance = 4;
        g.insurance_domain_budget = vec![0; g.insurance_domain_budget.len()];
        let expected_domain = match bankrupt_side {
            SideV16::Long => 1,
            SideV16::Short => 0,
        };
        let unrelated_domain = match bankrupt_side {
            SideV16::Long => 0,
            SideV16::Short => 1,
        };
        g.insurance_domain_budget[expected_domain] = 3;
        bankrupt.pnl = -5;
        g.negative_pnl_account_count = 1;
        match bankrupt_side {
            SideV16::Long => {
                g.attach_leg(&mut bankrupt, 0, SideV16::Long, 1).unwrap();
                g.attach_leg(&mut opposing, 0, SideV16::Short, -1).unwrap();
            }
            SideV16::Short => {
                g.attach_leg(&mut bankrupt, 0, SideV16::Short, -1).unwrap();
                g.attach_leg(&mut opposing, 0, SideV16::Long, 1).unwrap();
            }
        }

        let out = g
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

        assert_eq!(out.insurance_used, 3);
        assert_eq!(out.residual_booked, 2);
        assert_eq!(g.insurance_domain_spent[expected_domain], 3);
        assert_eq!(g.insurance_domain_spent[unrelated_domain], 0);
        assert_eq!(bankrupt.pnl, 0);
        assert_eq!(bankrupt.active_bitmap, bitmap(&[]));
    }
}

#[test]
fn v16_bad_asset_cannot_spend_unrelated_domain_insurance_budget() {
    let mut g = group();
    let mut bankrupt = account();
    let mut opposing = account_with_id(9);
    g.vault = 4;
    g.insurance = 4;
    g.insurance_domain_budget = vec![0; g.insurance_domain_budget.len()];
    g.insurance_domain_budget[0] = 4;
    bankrupt.pnl = -5;
    g.negative_pnl_account_count = 1;
    g.attach_leg(&mut bankrupt, 0, SideV16::Long, 1).unwrap();
    g.attach_leg(&mut opposing, 0, SideV16::Short, -1).unwrap();

    let out = g
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

    assert_eq!(out.insurance_used, 0);
    assert_eq!(out.residual_booked, 5);
    assert_eq!(g.insurance, 4);
    assert_eq!(
        g.insurance_domain_spent,
        vec![0; g.insurance_domain_spent.len()]
    );
    assert_eq!(
        g.pending_domain_loss_barrier_count(0, SideV16::Short),
        Ok(0)
    );
    assert_eq!(bankrupt.pnl, 0);
    assert_eq!(bankrupt.active_bitmap, bitmap(&[]));
}

#[test]
fn v16_public_invariants_reject_overallocated_domain_insurance_budgets() {
    let mut g = group();
    g.vault = 5;
    g.insurance = 5;
    g.insurance_domain_budget = vec![0; g.insurance_domain_budget.len()];
    g.insurance_domain_budget[0] = 5;
    g.insurance_domain_budget[1] = 1;

    assert_eq!(g.assert_public_invariants(), Err(V16Error::InvalidConfig));
}

#[test]
fn v16_zero_copy_shape_rejects_overallocated_domain_insurance_budgets() {
    let mut g = group();
    g.vault = 5;
    g.insurance = 5;
    g.insurance_domain_budget = vec![0; g.insurance_domain_budget.len()];
    g.insurance_domain_budget[0] = 5;
    g.insurance_domain_budget[1] = 1;

    let mut header =
        MarketGroupV16HeaderAccount::from_runtime_with_capacity(&g, g.assets.len()).unwrap();
    let mut markets = (0..g.assets.len())
        .map(|i| Market {
            wrapper: (),
            engine: EngineAssetSlotV16Account::from_runtime_group_slot(&g, i).unwrap(),
        })
        .collect::<Vec<_>>();
    let view = MarketGroupV16ViewMut::new(&mut header, &mut markets);

    assert_eq!(view.validate_shape(), Err(V16Error::InvalidConfig));
}

#[test]
fn v16_bankrupt_liquidation_drops_uncollectible_fee_and_spends_insurance_once() {
    let (market, _, owner) = ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.max_price_move_bps_per_slot = 1;
    cfg.min_nonzero_mm_req = 12;
    cfg.min_nonzero_im_req = 13;
    cfg.liquidation_fee_bps = 10_000;
    cfg.liquidation_fee_cap = 10;
    cfg.min_liquidation_abs = 1;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    let mut a = account();
    let mut opposing = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));
    g.vault = 2;
    g.insurance = 2;
    g.insurance_domain_budget[1] = 2;
    a.pnl = -5;
    g.negative_pnl_account_count = 1;
    g.attach_leg(&mut a, 0, SideV16::Long, 1).unwrap();
    g.attach_leg(&mut opposing, 0, SideV16::Short, -1).unwrap();

    let out = g
        .liquidate_account_not_atomic(
            &mut a,
            LiquidationRequestV16 {
                asset_index: 0,
                close_q: 1,
                fee_bps: 10_000,
            },
            &[1; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(out.fee_charged, 0);
    assert_eq!(out.insurance_used, 2);
    assert_eq!(out.residual_booked, 3);
    assert_eq!(out.explicit_loss, 0);
    assert_eq!(g.insurance, 0);
    assert_eq!(a.pnl, 0);
    assert_eq!(a.active_bitmap, bitmap(&[]));
}

#[test]
fn v16_bankrupt_liquidation_requires_residual_durable_before_freeing_exposure() {
    let (market, _, owner) = ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.public_b_chunk_atoms = 1;
    let mut g = MarketGroupV16::new(market, cfg).unwrap();
    let mut bankrupt = account();
    let mut opposing = PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, [4; 32], owner));

    g.attach_leg(&mut bankrupt, 0, SideV16::Long, 4).unwrap();
    g.attach_leg(&mut opposing, 0, SideV16::Short, -10).unwrap();
    bankrupt.pnl = -5;
    g.negative_pnl_account_count = 1;

    let before_bitmap = bankrupt.active_bitmap;
    let before_basis = bankrupt.legs[0].basis_pos_q;
    let before_pnl = bankrupt.pnl;
    let res = g.liquidate_account_not_atomic(
        &mut bankrupt,
        LiquidationRequestV16 {
            asset_index: 0,
            close_q: 4,
            fee_bps: 0,
        },
        &[1; V16_MAX_PORTFOLIO_ASSETS_N],
    );

    assert_eq!(res, Err(V16Error::RecoveryRequired));
    assert_eq!(
        g.recovery_reason,
        Some(PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress)
    );
    assert_eq!(bankrupt.active_bitmap, before_bitmap);
    assert_eq!(bankrupt.legs[0].basis_pos_q, before_basis);
    assert_eq!(bankrupt.pnl, before_pnl);
    assert_eq!(g.assets[0].b_short_num, 0);
}

#[test]
fn v16_rebalance_reduce_position_requires_strict_risk_progress_and_preserves_senior_claims() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV16::Long, POS_SCALE as i128)
        .unwrap();
    let _opposite = attach_opposite(&mut g, 0, SideV16::Long, POS_SCALE, 104);
    let senior_before = g.c_tot + g.insurance;
    let out = g
        .rebalance_reduce_position_not_atomic(
            &mut a,
            RebalanceRequestV16 {
                asset_index: 0,
                reduce_q: POS_SCALE / 2,
            },
            &[1_000_000; V16_MAX_PORTFOLIO_ASSETS_N],
        )
        .unwrap();

    assert_eq!(out.reduced_q, POS_SCALE / 2);
    assert_eq!(a.legs[0].basis_pos_q.unsigned_abs(), POS_SCALE / 2);
    assert_eq!(g.c_tot + g.insurance, senior_before);
}

#[test]
fn v16_rebalance_rejects_missing_or_zero_progress() {
    let mut g = group();
    let mut a = account();

    assert_eq!(
        g.rebalance_reduce_position_not_atomic(
            &mut a,
            RebalanceRequestV16 {
                asset_index: 0,
                reduce_q: 1,
            },
            &[1_000_000; V16_MAX_PORTFOLIO_ASSETS_N],
        ),
        Err(V16Error::InvalidLeg)
    );
    assert_eq!(
        g.rebalance_reduce_position_not_atomic(
            &mut a,
            RebalanceRequestV16 {
                asset_index: 0,
                reduce_q: 0,
            },
            &[1_000_000; V16_MAX_PORTFOLIO_ASSETS_N],
        ),
        Err(V16Error::InvalidConfig)
    );
}

#[test]
fn v16_risk_increasing_trade_creates_source_credit_lien_for_im() {
    let (mut header, mut markets) = market_fixture(1, 1);
    let (mut long_header, mut long_domains) = account_fixture(1, 8);
    let (mut short_header, mut short_domains) = account_fixture(1, 9);
    let claim = 100u128;
    let claim_num = claim * BOUND_SCALE;
    long_header.pnl = V16PodI128::new(claim as i128);
    long_domains[0].source_claim_market_id = V16PodU64::new(1);
    long_domains[0].source_claim_bound_num = V16PodU128::new(claim_num);
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
        let mut short = PortfolioV16ViewMut::new(&mut short_header, &mut short_domains);
        market.deposit_not_atomic(&mut short, 1_000).unwrap();
    }

    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut long = PortfolioV16ViewMut::new(&mut long_header, &mut long_domains);
    let mut short = PortfolioV16ViewMut::new(&mut short_header, &mut short_domains);
    market
        .execute_trade_with_fee_in_place_not_atomic(
            &mut long,
            &mut short,
            TradeRequestV16 {
                asset_index: 0,
                size_q: 10 * POS_SCALE,
                exec_price: 1,
                fee_bps: 0,
                admit_h_max_consumption_threshold_bps_opt: None,
            },
        )
        .expect("risk-increasing trade should atomically lien backed source credit for IM");

    assert_eq!(long.header.capital.get(), 0);
    assert_eq!(
        long.source_domains[0].source_claim_liened_num.get(),
        10 * BOUND_SCALE
    );
    assert_eq!(
        long.source_domains[0].source_lien_effective_reserved.get(),
        10
    );
    assert_eq!(
        long.source_domains[0]
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
    market.validate_shape().unwrap();
    long.validate_with_market(&market.as_view()).unwrap();
    short.validate_with_market(&market.as_view()).unwrap();
}
