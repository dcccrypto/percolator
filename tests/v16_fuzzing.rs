#![cfg(feature = "fuzz")]

use percolator::v16::{
    LiquidationRequestV16, MarketGroupV16, PermissionlessCrankActionV16,
    PermissionlessCrankRequestV16, PortfolioAccountV16, ProvenanceHeaderV16, TradeRequestV16,
    V16Config, V16Error, V16_MAX_PORTFOLIO_ASSETS_N,
};
use proptest::prelude::*;

fn ids() -> ([u8; 32], [u8; 32], [u8; 32], [u8; 32]) {
    ([1; 32], [2; 32], [3; 32], [4; 32])
}

fn fuzz_group() -> MarketGroupV16 {
    let (market, _, _, _) = ids();
    let mut cfg = V16Config::public_user_fund(1, 0, 10);
    cfg.max_trading_fee_bps = 10;
    cfg.public_b_chunk_atoms = 1;
    MarketGroupV16::new(market, cfg).unwrap()
}

fn fuzz_accounts() -> (PortfolioAccountV16, PortfolioAccountV16) {
    let (market, a_id, b_id, owner) = ids();
    (
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, a_id, owner)),
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, b_id, owner)),
    )
}

fn prices(price: u64) -> [u64; V16_MAX_PORTFOLIO_ASSETS_N] {
    [price; V16_MAX_PORTFOLIO_ASSETS_N]
}

fn assert_fuzz_invariants(
    group: &MarketGroupV16,
    a: &PortfolioAccountV16,
    b: &PortfolioAccountV16,
) {
    assert_eq!(group.assert_public_invariants(), Ok(()));
    assert_eq!(group.validate_account_shape(a), Ok(()));
    assert_eq!(group.validate_account_shape(b), Ok(()));
    assert_eq!(group.c_tot, a.capital + b.capital);

    let positive_pnl = [a.pnl, b.pnl]
        .into_iter()
        .filter(|pnl| *pnl > 0)
        .map(|pnl| pnl as u128)
        .sum::<u128>();
    assert_eq!(group.pnl_pos_tot, positive_pnl);
}

fn run_with_svm_rollback(
    group: &mut MarketGroupV16,
    a: &mut PortfolioAccountV16,
    b: &mut PortfolioAccountV16,
    result: Result<(), V16Error>,
    before: (MarketGroupV16, PortfolioAccountV16, PortfolioAccountV16),
) {
    if result.is_err() {
        *group = before.0;
        *a = before.1;
        *b = before.2;
    }
    assert_fuzz_invariants(group, a, b);
}

fn apply_fuzz_action(
    group: &mut MarketGroupV16,
    a: &mut PortfolioAccountV16,
    b: &mut PortfolioAccountV16,
    selector: u8,
    amount_seed: u16,
) {
    let before = (group.clone(), a.clone(), b.clone());
    let target_a = (selector & 0x8) == 0;
    let price = 1 + ((amount_seed as u64) & 1);
    let effective_prices = prices(price);
    let amount = (amount_seed as u128) % 128;
    let result = match selector % 8 {
        0 => {
            if target_a {
                group.deposit_not_atomic(a, amount)
            } else {
                group.deposit_not_atomic(b, amount)
            }
        }
        1 => {
            if target_a {
                group.withdraw_not_atomic(a, amount, &effective_prices)
            } else {
                group.withdraw_not_atomic(b, amount, &effective_prices)
            }
        }
        2 => {
            if target_a {
                group.charge_account_fee_not_atomic(a, amount).map(|_| ())
            } else {
                group.charge_account_fee_not_atomic(b, amount).map(|_| ())
            }
        }
        3 => {
            if target_a {
                group.full_account_refresh(a, &effective_prices).map(|_| ())
            } else {
                group.full_account_refresh(b, &effective_prices).map(|_| ())
            }
        }
        4 => group
            .execute_trade_with_fee_not_atomic(
                a,
                b,
                TradeRequestV16 {
                    asset_index: 0,
                    size_q: 1 + (amount % 4),
                    exec_price: price,
                    fee_bps: (amount_seed as u64) % 11,
                },
                &effective_prices,
            )
            .map(|_| ()),
        5 => {
            if target_a {
                group
                    .permissionless_crank_not_atomic(
                        a,
                        PermissionlessCrankRequestV16 {
                            now_slot: group.current_slot.saturating_add(1),
                            asset_index: 0,
                            effective_price: price,
                            funding_rate_e9: 0,
                            action: PermissionlessCrankActionV16::Refresh,
                        },
                        &effective_prices,
                    )
                    .map(|_| ())
            } else {
                group
                    .permissionless_crank_not_atomic(
                        b,
                        PermissionlessCrankRequestV16 {
                            now_slot: group.current_slot.saturating_add(1),
                            asset_index: 0,
                            effective_price: price,
                            funding_rate_e9: 0,
                            action: PermissionlessCrankActionV16::Refresh,
                        },
                        &effective_prices,
                    )
                    .map(|_| ())
            }
        }
        6 => {
            let req = LiquidationRequestV16 {
                asset_index: 0,
                close_q: 1 + (amount % 4),
                fee_bps: (amount_seed as u64) % 11,
            };
            if target_a {
                group.liquidate_account_not_atomic(a, req, &effective_prices)
            } else {
                group.liquidate_account_not_atomic(b, req, &effective_prices)
            }
            .map(|_| ())
        }
        _ => if target_a {
            group.convert_released_pnl_to_capital_not_atomic(a)
        } else {
            group.convert_released_pnl_to_capital_not_atomic(b)
        }
        .map(|_| ()),
    };

    run_with_svm_rollback(group, a, b, result, before);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn v16_fuzz_public_live_actions_preserve_conservation_under_svm_rollback(
        actions in prop::collection::vec((0u8..16, 0u16..512), 1..80)
    ) {
        let mut group = fuzz_group();
        let (mut a, mut b) = fuzz_accounts();
        group.deposit_not_atomic(&mut a, 1_000).unwrap();
        group.deposit_not_atomic(&mut b, 1_000).unwrap();
        assert_fuzz_invariants(&group, &a, &b);

        for (selector, amount_seed) in actions {
            apply_fuzz_action(&mut group, &mut a, &mut b, selector, amount_seed);
        }
    }
}
