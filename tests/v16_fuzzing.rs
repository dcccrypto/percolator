#![cfg(feature = "fuzz")]

use percolator::{
    EngineAssetSlotV16Account, LiquidationRequestV16, Market, MarketGroupV16HeaderAccount,
    MarketGroupV16ViewMut, PermissionlessCrankActionV16, PermissionlessCrankRequestV16,
    PermissionlessRecoveryReasonV16, PortfolioAccountV16Account, PortfolioLegV16, PortfolioV16View,
    PortfolioV16ViewMut, ProvenanceHeaderV16, ProvenanceHeaderV16Account, TradeRequestV16,
    V16Config, V16Error, V16PodU128, MAX_VAULT_TVL,
};
use percolator::{BOUND_SCALE, POS_SCALE, PUBLIC_B_CHUNK_ATOMS, SOCIAL_LOSS_DEN};
use proptest::prelude::*;

fn ids() -> ([u8; 32], [u8; 32], [u8; 32], [u8; 32]) {
    ([1; 32], [2; 32], [3; 32], [4; 32])
}

fn source_claim_num(account: &PortfolioAccountV16Account, domain: usize) -> u128 {
    account
        .source_domains
        .iter()
        .find(|source| {
            source.source_claim_market_id.get() != 0 && source.domain.get() as usize == domain
        })
        .map(|source| source.source_claim_bound_num.get())
        .unwrap_or(0)
}

// A burn walks the account's source domains in order. Emptying an earlier domain
// makes the table SPARSE, and resolving each slot by scanning for its domain then
// disagrees with the caller's index, refusing a later domain that is still funded.
// Reading the slot in place keeps the traversal correct. (upstream efa7e6f4)
#[test]
fn v16_source_claim_burn_crosses_a_just_emptied_sparse_slot() {
    let (market_id, _, _, owner) = ids();
    let cfg = V16Config::public_user_fund_with_market_slots(2, 2, 0, 10);
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(market_id, cfg, 2, 0).unwrap();
    let mut markets = vec![
        Market::new(0u64, EngineAssetSlotV16Account::default()),
        Market::new(1u64, EngineAssetSlotV16Account::default()),
    ];
    for (asset_index, market) in markets.iter_mut().enumerate() {
        header
            .activate_empty_asset_slot_not_atomic(
                asset_index as u32,
                &mut market.engine,
                100,
                (asset_index + 1) as u64,
            )
            .unwrap();
    }
    let mut account_header = PortfolioAccountV16Account::default();
    account_header
        .init_empty_in_place(ProvenanceHeaderV16Account::from_runtime(
            &ProvenanceHeaderV16::new(market_id, [11; 32], owner),
        ))
        .unwrap();

    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header);
    market
        .add_account_source_positive_pnl_not_atomic(&mut account, 1, 100)
        .unwrap();
    market
        .add_account_source_positive_pnl_not_atomic(&mut account, 3, 100)
        .unwrap();

    market.kani_set_account_pnl(&mut account, 0).unwrap();
    assert_eq!(account.header.pnl.get(), 0);
    assert_eq!(source_claim_num(account.header, 1), 0);
    assert_eq!(source_claim_num(account.header, 3), 0);
}

struct TwoDomainBSettlement {
    other_claim_num: u128,
    target_claim_num: u128,
    target_lien_num: u128,
    target_valid_backing_num: u128,
    target_valid_insurance_num: u128,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TargetLienBacking {
    None,
    Counterparty,
    Insurance,
}

fn settle_two_domain_b_loss(
    target_claim: u128,
    other_claim: u128,
    loss: u128,
    target_lien: u128,
    target_lien_backing: TargetLienBacking,
) -> TwoDomainBSettlement {
    let (market_id, _, _, owner) = ids();
    let cfg = V16Config::public_user_fund_with_market_slots(2, 2, 0, 10);
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(market_id, cfg, 2, 0).unwrap();
    let mut markets = vec![
        Market::new(0u64, EngineAssetSlotV16Account::default()),
        Market::new(1u64, EngineAssetSlotV16Account::default()),
    ];
    for (asset_index, market) in markets.iter_mut().enumerate() {
        header
            .activate_empty_asset_slot_not_atomic(
                asset_index as u32,
                &mut market.engine,
                100,
                (asset_index + 1) as u64,
            )
            .unwrap();
    }
    let mut long_header = PortfolioAccountV16Account::default();
    long_header
        .init_empty_in_place(ProvenanceHeaderV16Account::from_runtime(
            &ProvenanceHeaderV16::new(market_id, [9; 32], owner),
        ))
        .unwrap();
    let mut short_header = PortfolioAccountV16Account::default();
    short_header
        .init_empty_in_place(ProvenanceHeaderV16Account::from_runtime(
            &ProvenanceHeaderV16::new(market_id, [10; 32], owner),
        ))
        .unwrap();

    let loss_weight = {
        let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut long = PortfolioV16ViewMut::new(&mut long_header);
        let mut short = PortfolioV16ViewMut::new(&mut short_header);
        market.deposit_not_atomic(&mut long, 10_000).unwrap();
        market.deposit_not_atomic(&mut short, 10_000).unwrap();
        match target_lien_backing {
            TargetLienBacking::None => {}
            TargetLienBacking::Counterparty => market
                .deposit_fresh_counterparty_backing_not_atomic(3, 1_000, 100)
                .unwrap(),
            TargetLienBacking::Insurance => {
                market
                    .deposit_domain_insurance_not_atomic(3, 1_000)
                    .unwrap();
                market
                    .reserve_insurance_credit_not_atomic(3, 1_000 * BOUND_SCALE)
                    .unwrap();
            }
        }
        market
            .execute_trade_with_fee_loss_stale_scoped_not_atomic(
                &mut long,
                &mut short,
                TradeRequestV16 {
                    asset_index: 1,
                    size_q: POS_SCALE as i128,
                    exec_price: 100,
                    fee_bps: 0,
                },
                true,
            )
            .unwrap();
        market
            .add_account_source_positive_pnl_not_atomic(&mut long, 1, other_claim)
            .unwrap();
        market
            .add_account_source_positive_pnl_not_atomic(&mut long, 3, target_claim)
            .unwrap();
        if target_lien != 0 {
            long.header.health_cert.certified_initial_req =
                V16PodU128::new(long.header.capital.get() + target_lien);
            long.header.health_cert.valid = 1;
            market
                .kani_create_initial_margin_source_lien_if_needed(&mut long)
                .unwrap();
            assert_eq!(
                source_claim_num(&*long.header, 3),
                target_claim * BOUND_SCALE
            );
            let target_source = long
                .header
                .source_domains
                .iter()
                .find(|source| {
                    source.source_claim_market_id.get() != 0 && source.domain.get() as usize == 3
                })
                .expect("target-domain claim");
            assert_eq!(
                target_source.source_claim_liened_num.get(),
                target_lien * BOUND_SCALE
            );
        }
        long.header
            .legs
            .iter()
            .find_map(|leg| {
                let leg = leg.try_to_runtime().ok()?;
                (leg.active && leg.asset_index == 1).then_some(leg.loss_weight)
            })
            .unwrap()
    };
    let delta_b = loss
        .checked_mul(SOCIAL_LOSS_DEN)
        .unwrap()
        .checked_div(loss_weight)
        .unwrap();
    markets[1].engine.asset.b_long_num = V16PodU128::new(delta_b);

    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut long = PortfolioV16ViewMut::new(&mut long_header);
    let outcome = market
        .kani_settle_account_b_chunk(&mut long, 1, delta_b)
        .unwrap();
    assert_eq!(outcome.loss, loss);
    let target_source = long_header
        .source_domains
        .iter()
        .find(|source| {
            source.source_claim_market_id.get() != 0 && source.domain.get() as usize == 3
        })
        .copied()
        .unwrap_or_default();
    TwoDomainBSettlement {
        other_claim_num: source_claim_num(&long_header, 1),
        target_claim_num: source_claim_num(&long_header, 3),
        target_lien_num: target_source.source_claim_liened_num.get(),
        target_valid_backing_num: markets[1]
            .engine
            .backing_short
            .try_to_runtime()
            .unwrap()
            .valid_liened_backing_num,
        target_valid_insurance_num: markets[1]
            .engine
            .insurance_reservation_short
            .try_to_runtime()
            .unwrap()
            .valid_liened_insurance_num,
    }
}

#[test]
fn v16_b_loss_burns_the_matching_source_domain_before_sparse_slot_order() {
    let after = settle_two_domain_b_loss(100, 100, 50, 0, TargetLienBacking::None);
    assert_eq!(after.other_claim_num, 100 * BOUND_SCALE);
    assert_eq!(after.target_claim_num, 50 * BOUND_SCALE);
}

#[test]
fn v16_b_loss_falls_back_to_cross_margin_claims_after_its_domain_is_exhausted() {
    let after = settle_two_domain_b_loss(20, 100, 50, 0, TargetLienBacking::None);
    assert_eq!(after.target_claim_num, 0);
    assert_eq!(after.other_claim_num, 70 * BOUND_SCALE);
}

#[test]
fn v16_b_loss_preserves_the_unburned_part_of_a_matching_source_lien() {
    let after = settle_two_domain_b_loss(100, 100, 50, 80, TargetLienBacking::Counterparty);
    assert_eq!(after.other_claim_num, 100 * BOUND_SCALE);
    assert_eq!(after.target_claim_num, 50 * BOUND_SCALE);
    assert_eq!(after.target_lien_num, 50 * BOUND_SCALE);
    assert_eq!(after.target_valid_backing_num, 50 * BOUND_SCALE);
}

#[test]
fn v16_b_loss_preserves_the_unburned_part_of_a_matching_insurance_lien() {
    let after = settle_two_domain_b_loss(100, 100, 50, 80, TargetLienBacking::Insurance);
    assert_eq!(after.other_claim_num, 100 * BOUND_SCALE);
    assert_eq!(after.target_claim_num, 50 * BOUND_SCALE);
    assert_eq!(after.target_lien_num, 50 * BOUND_SCALE);
    assert_eq!(after.target_valid_backing_num, 0);
    assert_eq!(after.target_valid_insurance_num, 50 * BOUND_SCALE);
}

// Multi-step B settlement, driven for real: a bankrupt residual is booked onto the winning
// side's B index, then the winner settles it under a 1-atom caller budget. Each step moves at
// most one loss atom, so settlement takes several steps (the old fuzzer reached this with a
// one-atom config chunk) and a zero budget with a B delta outstanding asks for Recovery.
#[test]
fn v16_b_settlement_under_a_small_budget_takes_several_steps_and_a_zero_budget_recovers() {
    const RESIDUAL: u128 = 40;
    let (mut header, mut markets) = fuzz_group();
    let (_, a_id, b_id, _) = ids();
    let mut loser = fuzz_account(a_id);
    let mut winner = fuzz_account(b_id);
    {
        let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut l = PortfolioV16ViewMut::new(&mut loser);
        let mut w = PortfolioV16ViewMut::new(&mut winner);
        market.deposit_not_atomic(&mut l, 1_000).unwrap();
        market.deposit_not_atomic(&mut w, 1_000).unwrap();
        market
            .execute_trade_with_fee_loss_stale_scoped_not_atomic(
                &mut l,
                &mut w,
                TradeRequestV16 {
                    asset_index: 0,
                    size_q: POS_SCALE as i128,
                    exec_price: 1,
                    fee_bps: 0,
                },
                true,
            )
            .unwrap();
    }
    // The long loser goes bankrupt; the short winner carries the loss weight.
    loser.pnl = percolator::V16PodI128::new(-(RESIDUAL as i128));
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let booked = {
        let mut l = PortfolioV16ViewMut::new(&mut loser);
        let (booked, explicit) = market
            .kani_book_bankruptcy_residual_chunk_for_account_core(
                &mut l,
                0,
                percolator::SideV16::Long,
                RESIDUAL,
            )
            .unwrap();
        booked + explicit
    };
    assert!(booked > 1, "the residual must be large enough to need several 1-atom steps");

    let mut w = PortfolioV16ViewMut::new(&mut winner);
    // A zero budget cannot settle anything while B has moved: Recovery, not a loop.
    assert_eq!(
        market.kani_settle_account_b_chunk(&mut w, 0, 0).unwrap_err(),
        V16Error::RecoveryRequired
    );
    let pnl_before = w.header.pnl.get();
    let mut steps = 0u32;
    let mut total_loss = 0u128;
    let mut saw_partial = false;
    loop {
        let chunk = market.kani_settle_account_b_chunk(&mut w, 0, 1).unwrap();
        steps += 1;
        total_loss += chunk.loss;
        assert!(chunk.loss <= 1, "a 1-atom budget caps one step at one loss atom");
        if chunk.remaining_after != 0 {
            saw_partial = true;
        } else {
            break;
        }
        assert!(steps < 10_000, "settlement must terminate");
    }
    assert!(steps > 1 && saw_partial, "B settlement must have taken more than one step");
    assert!(total_loss <= booked);
    assert_eq!(pnl_before - w.header.pnl.get(), total_loss as i128);
}

fn fuzz_group() -> (MarketGroupV16HeaderAccount, Vec<Market<u64>>) {
    let (market_id, _, _, _) = ids();
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    cfg.max_trading_fee_bps = 10;
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(market_id, cfg, 1, 0).unwrap();
    let mut markets = vec![Market::new(0u64, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, 1, 1)
        .unwrap();
    (header, markets)
}

fn fuzz_account(account_id: [u8; 32]) -> PortfolioAccountV16Account {
    let (market_id, _, _, owner) = ids();
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        market_id, account_id, owner,
    ));
    let mut account = PortfolioAccountV16Account::default();
    account.init_empty_in_place(header).unwrap();
    account
}

// upstream 0976a303: the B-settlement budget is LOSS ATOMS, not a B-index delta. The
// public config's default chunk (MAX_VAULT_TVL) must clear a large index delta in one
// step, which a same-numbered index-delta budget could not. Replaces this fork's
// hand-rolled spec test, which set the budget itself and so skipped the config check.
#[test]
fn v16_b_settlement_budget_is_loss_atoms_not_index_delta() {
    const TARGET_B: u128 = 107_486_458_947_473_684_210_526_315;
    const LOSS_WEIGHT: u128 = 19_000_000;

    let (market_id, _, _, _) = ids();
    let cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    assert_eq!(PUBLIC_B_CHUNK_ATOMS, MAX_VAULT_TVL);
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(market_id, cfg, 1, 0).unwrap();
    let mut markets = vec![Market::new(0u64, EngineAssetSlotV16Account::default())];
    let market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let leg = PortfolioLegV16 {
        active: true,
        loss_weight: LOSS_WEIGHT,
        ..PortfolioLegV16::default()
    };
    let full_loss = LOSS_WEIGHT.checked_mul(TARGET_B).unwrap() / SOCIAL_LOSS_DEN;
    assert!(full_loss > 0 && full_loss < MAX_VAULT_TVL);

    let chunk = market
        .kani_account_b_settlement_chunk_from_leg(leg, TARGET_B, MAX_VAULT_TVL)
        .unwrap();

    assert_eq!(chunk.delta_b, TARGET_B);
    assert_eq!(chunk.loss, full_loss);
    assert_eq!(chunk.remaining_after, 0);
}

fn assert_fuzz_invariants(
    header: &mut MarketGroupV16HeaderAccount,
    markets: &mut [Market<u64>],
    account_a: &PortfolioAccountV16Account,
    account_b: &PortfolioAccountV16Account,
) {
    let market = MarketGroupV16ViewMut::new(header, markets);
    assert_eq!(market.validate_shape(), Ok(()));
    assert_eq!(
        PortfolioV16View::new(account_a).validate_with_market(&market.as_view()),
        Ok(())
    );
    assert_eq!(
        PortfolioV16View::new(account_b).validate_with_market(&market.as_view()),
        Ok(())
    );
    assert_eq!(
        market.header.c_tot.get(),
        account_a.capital.get() + account_b.capital.get()
    );
    assert!(market.header.vault.get() >= market.header.c_tot.get() + market.header.insurance.get());
    let positive_pnl = [account_a.pnl.get(), account_b.pnl.get()]
        .into_iter()
        .filter(|pnl| *pnl > 0)
        .map(|pnl| pnl as u128)
        .sum::<u128>();
    assert_eq!(market.header.pnl_pos_tot.get(), positive_pnl);
}

#[allow(clippy::too_many_arguments)]
fn run_with_svm_rollback(
    header: &mut MarketGroupV16HeaderAccount,
    markets: &mut Vec<Market<u64>>,
    account_a: &mut PortfolioAccountV16Account,
    account_b: &mut PortfolioAccountV16Account,
    result: Result<(), V16Error>,
    before: (
        MarketGroupV16HeaderAccount,
        Vec<Market<u64>>,
        PortfolioAccountV16Account,
        PortfolioAccountV16Account,
    ),
) {
    if result.is_err() {
        *header = before.0;
        *markets = before.1;
        *account_a = before.2;
        *account_b = before.3;
    }
    assert_fuzz_invariants(header, markets, account_a, account_b);
}

#[allow(clippy::too_many_arguments)]
fn apply_fuzz_action(
    header: &mut MarketGroupV16HeaderAccount,
    markets: &mut Vec<Market<u64>>,
    account_a: &mut PortfolioAccountV16Account,
    account_b: &mut PortfolioAccountV16Account,
    selector: u8,
    amount_seed: u16,
) {
    let before = (*header, markets.clone(), *account_a, *account_b);
    let target_a = (selector & 0x8) == 0;
    let amount = (amount_seed as u128) % 128;
    let result = match selector % 13 {
        0 => {
            let mut market = MarketGroupV16ViewMut::new(header, markets);
            if target_a {
                let mut account = PortfolioV16ViewMut::new(account_a);
                market.deposit_not_atomic(&mut account, amount)
            } else {
                let mut account = PortfolioV16ViewMut::new(account_b);
                market.deposit_not_atomic(&mut account, amount)
            }
        }
        1 => {
            let mut market = MarketGroupV16ViewMut::new(header, markets);
            if target_a {
                let mut account = PortfolioV16ViewMut::new(account_a);
                market.withdraw_not_atomic(&mut account, amount)
            } else {
                let mut account = PortfolioV16ViewMut::new(account_b);
                market.withdraw_not_atomic(&mut account, amount)
            }
        }
        2 => {
            let fee_slot = header.current_slot.get().saturating_add(1);
            let mut market = MarketGroupV16ViewMut::new(header, markets);
            if target_a {
                let mut account = PortfolioV16ViewMut::new(account_a);
                market
                    .sync_account_fee_to_slot_not_atomic(&mut account, fee_slot, amount)
                    .map(|_| ())
            } else {
                let mut account = PortfolioV16ViewMut::new(account_b);
                market
                    .sync_account_fee_to_slot_not_atomic(&mut account, fee_slot, amount)
                    .map(|_| ())
            }
        }
        3 => {
            let mut market = MarketGroupV16ViewMut::new(header, markets);
            if target_a {
                let mut account = PortfolioV16ViewMut::new(account_a);
                market
                    .full_account_refresh_not_atomic(&mut account)
                    .map(|_| ())
            } else {
                let mut account = PortfolioV16ViewMut::new(account_b);
                market
                    .full_account_refresh_not_atomic(&mut account)
                    .map(|_| ())
            }
        }
        4 => {
            let mut market = MarketGroupV16ViewMut::new(header, markets);
            let mut long_account = PortfolioV16ViewMut::new(account_a);
            let mut short_account = PortfolioV16ViewMut::new(account_b);
            market
                .execute_trade_with_fee_loss_stale_scoped_not_atomic(
                    &mut long_account,
                    &mut short_account,
                    TradeRequestV16 {
                        asset_index: 0,
                        size_q: i128::try_from(1 + (amount % 4)).unwrap(),
                        exec_price: 1,
                        fee_bps: (amount_seed as u64) % 11,
                    },
                    amount_seed % 2 == 0,
                )
                .map(|_| ())
        }
        5 => {
            let mut market = MarketGroupV16ViewMut::new(header, markets);
            if target_a {
                let mut account = PortfolioV16ViewMut::new(account_a);
                market
                    .permissionless_crank_not_atomic(
                        &mut account,
                        PermissionlessCrankRequestV16 {
                            now_slot: market.header.current_slot.get().saturating_add(1),
                            asset_index: 0,
                            effective_price: 1 + ((amount_seed as u64) & 1),
                            funding_rate_e9: 0,
                            action: PermissionlessCrankActionV16::Refresh,
                        },
                    )
                    .map(|_| ())
            } else {
                let mut account = PortfolioV16ViewMut::new(account_b);
                market
                    .permissionless_crank_not_atomic(
                        &mut account,
                        PermissionlessCrankRequestV16 {
                            now_slot: market.header.current_slot.get().saturating_add(1),
                            asset_index: 0,
                            effective_price: 1 + ((amount_seed as u64) & 1),
                            funding_rate_e9: 0,
                            action: PermissionlessCrankActionV16::Refresh,
                        },
                    )
                    .map(|_| ())
            }
        }
        6 => {
            let mut market = MarketGroupV16ViewMut::new(header, markets);
            if target_a {
                let mut account = PortfolioV16ViewMut::new(account_a);
                market
                    .sync_account_fee_to_slot_not_atomic(
                        &mut account,
                        market.header.current_slot.get(),
                        amount,
                    )
                    .map(|_| ())
            } else {
                let mut account = PortfolioV16ViewMut::new(account_b);
                market
                    .sync_account_fee_to_slot_not_atomic(
                        &mut account,
                        market.header.current_slot.get(),
                        amount,
                    )
                    .map(|_| ())
            }
        }
        7 => {
            let mut market = MarketGroupV16ViewMut::new(header, markets);
            if target_a {
                let mut account = PortfolioV16ViewMut::new(account_a);
                market.convert_released_pnl_to_capital_not_atomic(&mut account)
            } else {
                let mut account = PortfolioV16ViewMut::new(account_b);
                market.convert_released_pnl_to_capital_not_atomic(&mut account)
            }
            .map(|_| ())
        }
        8 => {
            let mut market = MarketGroupV16ViewMut::new(header, markets);
            if target_a {
                let mut account = PortfolioV16ViewMut::new(account_a);
                market
                    .liquidate_account_not_atomic(
                        &mut account,
                        LiquidationRequestV16 { asset_index: 0 },
                    )
                    .map(|_| ())
            } else {
                let mut account = PortfolioV16ViewMut::new(account_b);
                market
                    .liquidate_account_not_atomic(
                        &mut account,
                        LiquidationRequestV16 { asset_index: 0 },
                    )
                    .map(|_| ())
            }
        }
        9 => {
            let mut market = MarketGroupV16ViewMut::new(header, markets);
            if target_a {
                let mut account = PortfolioV16ViewMut::new(account_a);
                market
                    .permissionless_crank_not_atomic(
                        &mut account,
                        PermissionlessCrankRequestV16 {
                            now_slot: market.header.current_slot.get(),
                            asset_index: 0,
                            effective_price: 1,
                            funding_rate_e9: 0,
                            action: PermissionlessCrankActionV16::Recover(
                                PermissionlessRecoveryReasonV16::ExplicitLossOrDustAuditOverflow,
                            ),
                        },
                    )
                    .map(|_| ())
            } else {
                let mut account = PortfolioV16ViewMut::new(account_b);
                market
                    .permissionless_crank_not_atomic(
                        &mut account,
                        PermissionlessCrankRequestV16 {
                            now_slot: market.header.current_slot.get(),
                            asset_index: 0,
                            effective_price: 1,
                            funding_rate_e9: 0,
                            action: PermissionlessCrankActionV16::Recover(
                                PermissionlessRecoveryReasonV16::ExplicitLossOrDustAuditOverflow,
                            ),
                        },
                    )
                    .map(|_| ())
            }
        }
        10 => {
            let mut market = MarketGroupV16ViewMut::new(header, markets);
            market
                .resolve_market_not_atomic(market.header.current_slot.get())
                .map(|_| ())
        }
        11 => {
            let mut market = MarketGroupV16ViewMut::new(header, markets);
            if target_a {
                let mut account = PortfolioV16ViewMut::new(account_a);
                market
                    .close_resolved_account_not_atomic(&mut account, 0)
                    .map(|_| ())
            } else {
                let mut account = PortfolioV16ViewMut::new(account_b);
                market
                    .close_resolved_account_not_atomic(&mut account, 0)
                    .map(|_| ())
            }
        }
        _ => {
            // The public B chunk is the constant PUBLIC_B_CHUNK_ATOMS, so a public path settles a
            // leg's B loss in one step. A caller's loss-atom budget still caps one step; settle with
            // a tiny budget (0..=3 atoms) to reach multi-step settlement and the RecoveryRequired
            // branches of kernel_b_settlement_chunk.
            let budget = amount % 4;
            let mut market = MarketGroupV16ViewMut::new(header, markets);
            if target_a {
                let mut account = PortfolioV16ViewMut::new(account_a);
                market
                    .kani_settle_account_b_chunk(&mut account, 0, budget)
                    .map(|_| ())
            } else {
                let mut account = PortfolioV16ViewMut::new(account_b);
                market
                    .kani_settle_account_b_chunk(&mut account, 0, budget)
                    .map(|_| ())
            }
        }
    };

    run_with_svm_rollback(header, markets, account_a, account_b, result, before);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn v16_fuzz_public_live_view_actions_preserve_conservation_under_svm_rollback(
        actions in prop::collection::vec((0u8..16, 0u16..512), 1..80)
    ) {
        let (mut header, mut markets) = fuzz_group();
        let (_, a_id, b_id, _) = ids();
        let mut account_a = fuzz_account(a_id);
        let mut account_b = fuzz_account(b_id);
        {
            let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
            let mut a = PortfolioV16ViewMut::new(&mut account_a);
            let mut b = PortfolioV16ViewMut::new(&mut account_b);
            market.deposit_not_atomic(&mut a, 1_000).unwrap();
            market.deposit_not_atomic(&mut b, 1_000).unwrap();
        }
        assert_fuzz_invariants(
            &mut header,
            &mut markets,
            &account_a,
            &account_b,
        );

        for (selector, amount_seed) in actions {
            apply_fuzz_action(
                &mut header,
                &mut markets,
                &mut account_a,
                &mut account_b,
                selector,
                amount_seed,
            );
        }
    }
}

// INV-077 fee-sync skip (2026-11-23). A fee sync whose side-effect settlement has nothing to settle
// does not run it, writes the settlement's one other effect (the market's loss-stale summary, from
// the asset of its last entry) and keeps the current certificate with the charge taken from its
// equity. Differential: at every fee sync of a random three-asset history, the same state with only
// the target's certificate validity cleared runs the full settlement; every byte of the market and
// both accounts must match except the target's certificate. A kept certificate must be current, equal
// to the certificate held before the sync with the charge taken from its equity, and a full refresh
// of the post-state must succeed and differ from a refresh of the state before by the charge alone.
fn fee_skip_group() -> (MarketGroupV16HeaderAccount, Vec<Market<u64>>) {
    let (market_id, _, _, _) = ids();
    let mut cfg = V16Config::public_user_fund_with_market_slots(3, 3, 0, 10);
    cfg.max_trading_fee_bps = 10;
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(market_id, cfg, 3, 0).unwrap();
    let mut markets = (0..3u64)
        .map(|i| Market::new(i, EngineAssetSlotV16Account::default()))
        .collect::<Vec<_>>();
    for i in 0..3usize {
        header
            .activate_empty_asset_slot_not_atomic(
                i as u32,
                &mut markets[i].engine,
                100,
                (i + 1) as u64,
            )
            .unwrap();
    }
    (header, markets)
}

type FeeSkipState = (
    MarketGroupV16HeaderAccount,
    Vec<Market<u64>>,
    PortfolioAccountV16Account,
    PortfolioAccountV16Account,
);

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn v16_fuzz_fee_sync_skip_matches_the_settlement_except_the_certificate(
        actions in prop::collection::vec((0u8..6, 0usize..3, any::<bool>(), 0u16..1024), 1..60)
    ) {
        let (mut header, mut markets) = fee_skip_group();
        let (_, a_id, b_id, _) = ids();
        let mut account_a = fuzz_account(a_id);
        let mut account_b = fuzz_account(b_id);
        {
            let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
            let mut a = PortfolioV16ViewMut::new(&mut account_a);
            let mut b = PortfolioV16ViewMut::new(&mut account_b);
            market.deposit_not_atomic(&mut a, 100_000).unwrap();
            // B is thin, so fee charges can leave it below maintenance.
            market.deposit_not_atomic(&mut b, 250).unwrap();
        }
        let mut skips = 0usize;
        let mut syncs = 0usize;
        let mut refreshable = 0usize;
        let mut exact = 0usize;
        let mut exact_positive = 0usize;
        let mut deficit_voids = 0usize;
        for (op, asset, target_a, seed) in actions {
            let before: FeeSkipState = (header, markets.clone(), account_a, account_b);
            let result: Result<(), V16Error> = match op {
                // A trade of one or two units at the asset's effective price.
                0 => {
                    let price = markets[asset].engine.asset.effective_price.get();
                    let size_q = (POS_SCALE as i128) * (1 + (seed % 2) as i128);
                    let size_q = if target_a { size_q } else { -size_q };
                    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
                    let mut a = PortfolioV16ViewMut::new(&mut account_a);
                    let mut b = PortfolioV16ViewMut::new(&mut account_b);
                    market
                        .execute_trade_with_fee_loss_stale_scoped_not_atomic(
                            &mut a,
                            &mut b,
                            TradeRequestV16 { asset_index: asset, size_q, exec_price: price, fee_bps: 0 },
                            true,
                        )
                        .map(|_| ())
                }
                // An accrual that moves the price and pays funding: epochs move.
                1 => {
                    let price = 90 + (seed % 21) as u64;
                    let funding = (seed % 7) as i128 - 3;
                    let now = header.current_slot.get() + 1 + (seed % 3) as u64;
                    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
                    market
                        .accrue_asset_to_not_atomic(asset, now, price, funding, true)
                        .map(|_| ())
                }
                // An accrual at the same price without funding: no epoch moves.
                2 => {
                    let price = markets[asset].engine.asset.effective_price.get();
                    let now = header.current_slot.get() + 1 + (seed % 3) as u64;
                    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
                    market
                        .accrue_asset_to_not_atomic(asset, now, price, 0, true)
                        .map(|_| ())
                }
                // A full refresh: a current certificate again.
                3 => {
                    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
                    let account = if target_a { &mut account_a } else { &mut account_b };
                    let mut view = PortfolioV16ViewMut::new(account);
                    market.full_account_refresh_not_atomic(&mut view).map(|_| ())
                }
                // A fee sync, compared with the settlement on the same state.
                _ => {
                    syncs += 1;
                    let rate = (seed % 50) as u128;
                    let now = header.current_slot.get();
                    let last_fee_before = if target_a { account_a } else { account_b }.last_fee_slot.get();
                    let mut control: FeeSkipState = (header, markets.clone(), account_a, account_b);
                    if target_a {
                        control.2.health_cert.valid = 0;
                    } else {
                        control.3.health_cert.valid = 0;
                    }
                    let sync = |state: &mut FeeSkipState| {
                        let (h, m, a, b) = state;
                        let mut market = MarketGroupV16ViewMut::new(h, m);
                        let mut view = PortfolioV16ViewMut::new(if target_a { a } else { b });
                        market.sync_account_fee_to_slot_not_atomic(&mut view, now, rate)
                    };
                    let mut main: FeeSkipState = (header, markets.clone(), account_a, account_b);
                    let main_result = sync(&mut main);
                    let control_result = sync(&mut control);
                    prop_assert_eq!(main_result, control_result, "the same outcome");
                    if main_result.is_ok() {
                        let kept = if target_a { main.2 } else { main.3 }.health_cert.try_to_runtime().unwrap();
                        prop_assert_eq!(bytemuck::bytes_of(&main.0), bytemuck::bytes_of(&control.0));
                        prop_assert_eq!(&main.1, &control.1);
                        let (mut main_a, mut main_b, mut control_a, mut control_b) = (main.2, main.3, control.2, control.3);
                        // Only a valid certificate differs from the control's by design; a void one
                        // must be the held certificate with its validity cleared, as the settlement
                        // leaves it.
                        if kept.valid {
                            if target_a {
                                main_a.health_cert = Default::default();
                                control_a.health_cert = Default::default();
                            } else {
                                main_b.health_cert = Default::default();
                                control_b.health_cert = Default::default();
                            }
                        }
                        prop_assert_eq!(bytemuck::bytes_of(&main_a), bytemuck::bytes_of(&control_a));
                        prop_assert_eq!(bytemuck::bytes_of(&main_b), bytemuck::bytes_of(&control_b));
                        let control_cert = if target_a { control.2 } else { control.3 }.health_cert.try_to_runtime().unwrap();
                        let target_after = if target_a { main.2 } else { main.3 };
                        let advanced = target_after.last_fee_slot.get() > last_fee_before;
                        let held = if target_a { before.2 } else { before.3 }.health_cert.try_to_runtime().unwrap();
                        // The skip is taken only by an account with legs (a Live, non-flat sync). A
                        // flat account settles nothing in either case, and a zero charge leaves its
                        // certificate as it was, whatever its backing has done since.
                        let nonflat_before = if target_a { before.2 } else { before.3 }
                            .active_bitmap
                            .iter()
                            .any(|word| word.get() != 0);
                        if advanced && kept.valid && nonflat_before {
                            prop_assert_eq!(kept.certified_liq_deficit, 0, "only a healthy certificate is kept");
                        }
                        let held_current = held.valid
                            && held.cert_oracle_epoch == before.0.oracle_epoch.get()
                            && held.cert_funding_epoch == before.0.funding_epoch.get()
                            && held.cert_risk_epoch == before.0.risk_epoch.get()
                            && held.cert_asset_set_epoch == before.0.asset_set_epoch.get();
                        if advanced && !kept.valid && held_current && nonflat_before {
                            let charged = *main_result.as_ref().unwrap();
                            let equity = held.certified_equity - charged as i128;
                            let deficit = if equity < 0 {
                                equity.unsigned_abs()
                            } else {
                                held.certified_maintenance_req.saturating_sub(equity as u128)
                            };
                            if deficit != 0 {
                                deficit_voids += 1;
                            }
                        }
                        // A sync whose anchor did not advance returns before touching the account;
                        // one with legs that did advance and left a valid certificate took the skip.
                        if advanced && kept.valid && nonflat_before {
                            skips += 1;
                            let charged = main_result.unwrap();
                            prop_assert!(!control_cert.valid, "the settlement voids the certificate");
                            // The kept certificate is current for the post-state...
                            prop_assert_eq!(kept.cert_oracle_epoch, main.0.oracle_epoch.get());
                            prop_assert_eq!(kept.cert_funding_epoch, main.0.funding_epoch.get());
                            prop_assert_eq!(kept.cert_risk_epoch, main.0.risk_epoch.get());
                            prop_assert_eq!(kept.cert_asset_set_epoch, main.0.asset_set_epoch.get());
                            prop_assert_eq!(kept.active_bitmap_at_cert, target_after.active_bitmap.map(|w| w.get()));
                            // ...and is the certificate held before the sync with the charge taken
                            // from its equity (the capital charge is the sync's only write that
                            // equity reads).
                            let mut expected = held;
                            expected.certified_equity = held.certified_equity - charged as i128;
                            expected.certified_liq_deficit = if expected.certified_equity < 0 {
                                expected.certified_equity.unsigned_abs()
                            } else {
                                held.certified_maintenance_req.saturating_sub(expected.certified_equity as u128)
                            };
                            prop_assert_eq!(kept, expected);
                            // A full refresh of the post-state succeeds (a certificate that counts
                            // lapsed backing, which a refresh refuses, is not kept), as does one of
                            // the state before the sync, and the two differ by the charge alone; so
                            // when the held certificate was exact, so is the kept one. (No backing
                            // here: a refresh collects no utilization fee.)
                            let refresh = |state: &mut FeeSkipState| {
                                let (h, m, a, b) = state;
                                let mut market = MarketGroupV16ViewMut::new(h, m);
                                let mut view = PortfolioV16ViewMut::new(if target_a { a } else { b });
                                market.full_account_refresh_not_atomic(&mut view)
                            };
                            let mut probe_before: FeeSkipState = (before.0, before.1.clone(), before.2, before.3);
                            let mut probe_after: FeeSkipState = (main.0, main.1.clone(), main.2, main.3);
                            let refreshed_before = refresh(&mut probe_before);
                            let refreshed_after = refresh(&mut probe_after);
                            prop_assert!(refreshed_after.is_ok(), "the kept certificate's post-state refreshes: {:?}", refreshed_after);
                            match (refreshed_before, refreshed_after) {
                                (Ok(cert_before), Ok(cert_after)) => {
                                    let mut charged_before = cert_before;
                                    charged_before.certified_equity = cert_before.certified_equity - charged as i128;
                                    charged_before.certified_liq_deficit = if charged_before.certified_equity < 0 {
                                        charged_before.certified_equity.unsigned_abs()
                                    } else {
                                        cert_before.certified_maintenance_req.saturating_sub(charged_before.certified_equity as u128)
                                    };
                                    prop_assert_eq!(cert_after, charged_before);
                                    refreshable += 1;
                                    if held == cert_before {
                                        exact += 1;
                                        // Equity's positive-PnL branch reads the source claims' support.
                                        if target_after.pnl.get() > 0
                                            && target_after.source_domains.iter().any(|d| d.source_claim_bound_num.get() != 0)
                                        {
                                            exact_positive += 1;
                                        }
                                        prop_assert_eq!(kept, cert_after);
                                    }
                                }
                                (b, a) => prop_assert!(false, "refresh before {:?}, after {:?}", b, a),
                            }
                        }
                        header = main.0;
                        markets = main.1;
                        account_a = main.2;
                        account_b = main.3;
                    }
                    Ok(())
                }
            };
            if result.is_err() {
                header = before.0;
                markets = before.1;
                account_a = before.2;
                account_b = before.3;
            }
        }
        if syncs > 0 {
            eprintln!("FEESKIP syncs={syncs} skips={skips} refreshable={refreshable} exact={exact} exact_positive={exact_positive} deficit_voids={deficit_voids}");
        }
    }
}
