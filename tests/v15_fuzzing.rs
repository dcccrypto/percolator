#![cfg(feature = "fuzz")]

use percolator::v15::{
    LiquidationRequestV15, MarketGroupV15, PermissionlessCrankActionV15,
    PermissionlessCrankRequestV15, PortfolioAccountV15, ProvenanceHeaderV15, TradeRequestV15,
    V15Config, V15Error, V15_MAX_PORTFOLIO_ASSETS_N,
};
use proptest::prelude::*;

fn ids() -> ([u8; 32], [u8; 32], [u8; 32], [u8; 32]) {
    ([1; 32], [2; 32], [3; 32], [4; 32])
}

fn fuzz_group() -> MarketGroupV15 {
    let (market, _, _, _) = ids();
    let mut cfg = V15Config::public_user_fund(1, 0, 10);
    cfg.max_trading_fee_bps = 10;
    cfg.public_b_chunk_atoms = 1;
    MarketGroupV15::new(market, cfg).unwrap()
}

fn fuzz_accounts() -> (PortfolioAccountV15, PortfolioAccountV15) {
    let (market, a_id, b_id, owner) = ids();
    (
        PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, a_id, owner)),
        PortfolioAccountV15::empty(ProvenanceHeaderV15::new(market, b_id, owner)),
    )
}

fn prices(price: u64) -> [u64; V15_MAX_PORTFOLIO_ASSETS_N] {
    [price; V15_MAX_PORTFOLIO_ASSETS_N]
}

fn assert_fuzz_invariants(
    group: &MarketGroupV15,
    a: &PortfolioAccountV15,
    b: &PortfolioAccountV15,
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
    group: &mut MarketGroupV15,
    a: &mut PortfolioAccountV15,
    b: &mut PortfolioAccountV15,
    result: Result<(), V15Error>,
    before: (MarketGroupV15, PortfolioAccountV15, PortfolioAccountV15),
) {
    if result.is_err() {
        *group = before.0;
        *a = before.1;
        *b = before.2;
    }
    assert_fuzz_invariants(group, a, b);
}

fn apply_fuzz_action(
    group: &mut MarketGroupV15,
    a: &mut PortfolioAccountV15,
    b: &mut PortfolioAccountV15,
    selector: u8,
    amount_seed: u16,
) {
    let before = (*group, *a, *b);
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
                TradeRequestV15 {
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
                        PermissionlessCrankRequestV15 {
                            now_slot: group.current_slot.saturating_add(1),
                            asset_index: 0,
                            effective_price: price,
                            funding_rate_e9: 0,
                            action: PermissionlessCrankActionV15::Refresh,
                        },
                        &effective_prices,
                    )
                    .map(|_| ())
            } else {
                group
                    .permissionless_crank_not_atomic(
                        b,
                        PermissionlessCrankRequestV15 {
                            now_slot: group.current_slot.saturating_add(1),
                            asset_index: 0,
                            effective_price: price,
                            funding_rate_e9: 0,
                            action: PermissionlessCrankActionV15::Refresh,
                        },
                        &effective_prices,
                    )
                    .map(|_| ())
            }
        }
        6 => {
            let req = LiquidationRequestV15 {
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
    fn v15_fuzz_public_live_actions_preserve_conservation_under_svm_rollback(
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
