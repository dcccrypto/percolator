//! Issue #172 (reopen): after adf5cb64 a gain that reaches an account with negative PnL is still
//! settled differently depending on which account the keeper cranks first.
//!
//! spec.md, K/F settlement: "A nonzero uncovered tail MUST NOT both burn all retained positive
//! face and then be subtracted again. Permissionless settlement order therefore cannot change the
//! account's terminal value merely by changing which account is refreshed first." Requirement 27:
//! "liquidation order, support allocation, insurance allocation, credit-lien consumption, and
//! residual attribution MUST be deterministic and independent of caller ordering."
//!
//! Every case below builds one state and settles it twice, changing only which portfolio
//! `permissionless_auto_crank_not_atomic` reaches first. Each run records every account's
//! equity (capital + pnl), unowned value (vault minus C_tot, insurance, provider earnings and
//! fresh backing: what no account or provider can ever withdraw) and the backing domains. The
//! two runs must agree. No fees, no funding, one asset, engine API only.
//!
//! The mechanism (src/v16.rs `apply_signed_kf_delta_to_pnl`, `pnl < 0` branch): when the loser is
//! liquidated first, the winner's share of the residual arrives via SettleBChunk as negative pnl
//! with capital untouched. The winner's gain is then treated as a cure for that "debt": it
//! consumes the loser-paid backing, and because the share exceeds the backing
//! (`remaining_loss != 0`) the whole incoming face is burned and the rest is taken from capital.
//! Nobody is owed the residual share, so the consumed backing becomes unowned. Winner-first, the
//! same share nets against the winner's face and nothing is stranded.

use percolator::{
    AutoCrankObservationV16, AutoCrankPlanV16, AutoCrankWorkV16, EngineAssetSlotV16Account, Market,
    MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, PortfolioAccountV16Account,
    PortfolioV16ViewMut, ProvenanceHeaderV16, ProvenanceHeaderV16Account, TradeRequestV16,
    V16Config, BOUND_SCALE, POS_SCALE,
};

const START_PRICE: u64 = 10_000;

#[derive(Clone, Copy)]
struct Case {
    initial_margin_bps: u64,
    maintenance_margin_bps: u64,
    price_cap_bps: u64,
    loser_deposit: u128,
    winner_deposit: u128,
    units: i128,
    /// Winner is long (price rises) or short (price falls).
    winner_long: bool,
    /// Capped one-slot moves against the loser; nobody is cranked in between.
    steps: u32,
    /// Fresh counterparty backing deposited by a provider: (domain, amount).
    provider: Option<(usize, u128)>,
}

#[derive(Debug, PartialEq, Eq)]
struct DomainState {
    claims: u128,
    fresh_reserved: u128,
    spent: u128,
    provider_receivable: u128,
    fresh_unliened: u128,
}

#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    winner_equity: i128,
    loser_equity: i128,
    unowned: u128,
    domains: [DomainState; 2],
    price: u64,
}

fn account(seed: u8) -> PortfolioAccountV16Account {
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        [1; 32], [seed; 32], [3; 32],
    ));
    let mut account = PortfolioAccountV16Account::default();
    account.init_empty_in_place(header).unwrap();
    account
}

fn crank_to_fixpoint(
    market: &mut MarketGroupV16ViewMut<'_, u64>,
    account: &mut PortfolioV16ViewMut<'_>,
    slot: u64,
    price: u64,
) {
    for _ in 0..32 {
        let result = market
            .permissionless_auto_crank_not_atomic(
                account,
                AutoCrankWorkV16 {
                    now_slot: slot,
                    observations: &[AutoCrankObservationV16 {
                        asset_index: 0,
                        effective_price: price,
                        funding_rate_e9: 0,
                    }],
                    resolved_close_fee_rate_per_slot: 0,
                },
            )
            .unwrap();
        if result.selected == AutoCrankPlanV16::NoAction {
            return;
        }
    }
    panic!("crank reached no fixpoint");
}

fn domains(market: &MarketGroupV16ViewMut<'_, u64>) -> [DomainState; 2] {
    let engine = &market.markets[0].engine;
    [
        (engine.source_credit_long, engine.backing_long),
        (engine.source_credit_short, engine.backing_short),
    ]
    .map(|(credit, bucket)| {
        let credit = credit.try_to_runtime().unwrap();
        let bucket = bucket.try_to_runtime().unwrap();
        DomainState {
            claims: credit.positive_claim_bound_num / BOUND_SCALE,
            fresh_reserved: credit.fresh_reserved_backing_num / BOUND_SCALE,
            spent: credit.spent_backing_num / BOUND_SCALE,
            provider_receivable: credit.provider_receivable_num / BOUND_SCALE,
            fresh_unliened: bucket.fresh_unliened_backing_num / BOUND_SCALE,
        }
    })
}

fn run(case: Case, winner_first: bool) -> Outcome {
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    cfg.initial_margin_bps = case.initial_margin_bps;
    cfg.maintenance_margin_bps = case.maintenance_margin_bps;
    cfg.max_price_move_bps_per_slot = case.price_cap_bps;
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut markets = vec![Market::new(0, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, START_PRICE, 1)
        .unwrap();
    let mut winner_account = account(1);
    let mut loser_account = account(2);
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut winner = PortfolioV16ViewMut::new(&mut winner_account);
    let mut loser = PortfolioV16ViewMut::new(&mut loser_account);

    if let Some((domain, amount)) = case.provider {
        market
            .deposit_fresh_counterparty_backing_not_atomic(domain, amount, 1_000_000)
            .unwrap();
    }
    market
        .deposit_not_atomic(&mut winner, case.winner_deposit)
        .unwrap();
    market
        .deposit_not_atomic(&mut loser, case.loser_deposit)
        .unwrap();
    let size_q = case.units * POS_SCALE as i128;
    market
        .execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut winner,
            &mut loser,
            TradeRequestV16 {
                asset_index: 0,
                size_q: if case.winner_long { size_q } else { -size_q },
                exec_price: START_PRICE,
                fee_bps: 0,
            },
            true,
        )
        .unwrap();

    let mut price = START_PRICE;
    let mut slot = 1;
    for _ in 0..case.steps {
        let step = price * case.price_cap_bps / 10_000;
        price = if case.winner_long {
            price + step
        } else {
            price - step
        };
        slot += 1;
        market
            .set_asset_raw_oracle_target_not_atomic(0, price)
            .unwrap();
        market
            .accrue_asset_to_not_atomic(0, slot, price, 0, true)
            .unwrap();
    }

    if winner_first {
        crank_to_fixpoint(&mut market, &mut winner, slot, price);
        crank_to_fixpoint(&mut market, &mut loser, slot, price);
    } else {
        crank_to_fixpoint(&mut market, &mut loser, slot, price);
        crank_to_fixpoint(&mut market, &mut winner, slot, price);
    }
    // Settle everyone again until nothing moves, so neither run is left mid-way.
    for _ in 0..2 {
        crank_to_fixpoint(&mut market, &mut winner, slot, price);
        crank_to_fixpoint(&mut market, &mut loser, slot, price);
    }

    let owned = market
        .header
        .c_tot
        .get()
        .checked_add(market.header.insurance.get())
        .and_then(|v| v.checked_add(market.header.backing_provider_earnings_total.get()))
        .and_then(|v| {
            v.checked_add(market.header.source_fresh_backing_total_num.get() / BOUND_SCALE)
        })
        .unwrap();
    Outcome {
        winner_equity: winner.header.capital.get() as i128 + winner.header.pnl.get(),
        loser_equity: loser.header.capital.get() as i128 + loser.header.pnl.get(),
        unowned: market.header.vault.get() - owned,
        domains: domains(&market),
        price,
    }
}

/// Runs both orders; asserts the winner-first run is the expected exact result, then that the
/// loser-first run (what a keeper's auto-crank does) matches it.
fn assert_order_independent(case: Case, expected_winner_equity: i128) {
    let control = run(case, true);
    let attack = run(case, false);
    println!("winner first: {control:?}");
    println!("loser first:  {attack:?}");
    assert_eq!(control.price, attack.price);
    // Allow one atom of conservative rounding in the claim face.
    assert!(
        (control.winner_equity - expected_winner_equity).abs() <= 1,
        "control must be exact"
    );
    assert_eq!(control.unowned, 0, "control strands nothing");
    assert_eq!(
        attack.winner_equity, control.winner_equity,
        "crank order changed the winner's terminal value"
    );
    assert_eq!(
        attack.unowned, control.unowned,
        "crank order stranded value no account can withdraw"
    );
    assert_eq!(
        attack.domains, control.domains,
        "crank order changed backing consumption"
    );
}

/// Control: the loser is liquidated but stays solvent (no bankruptcy residual), so no B share
/// reaches the winner and both orders agree. This shows the harness measures what the cases below
/// claim.
#[test]
fn issue172_control_solvent_liquidation_is_order_independent() {
    let case = Case {
        initial_margin_bps: 1_000,
        maintenance_margin_bps: 500,
        price_cap_bps: 400,
        loser_deposit: 10_000,
        winner_deposit: 100_000,
        units: 9,
        winner_long: true,
        steps: 2,
        provider: None,
    };
    let control = run(case, true);
    let attack = run(case, false);
    println!("winner first: {control:?}");
    println!("loser first:  {attack:?}");
    assert_eq!(control.unowned, 0);
    assert_eq!(attack, control);
}

/// The solvent-winner case from the #172 comment of 2026-11-05 (one bankruptcy, stale winner),
/// reported fixed by #212. Equity agrees and resolution repairs the final payout, but while Live
/// the loser-first cure still consumes 5,272 of the loser-paid backing for a share nobody is owed
/// (booked as a provider receivable with no provider), leaving the winner's 10,001 claim backed
/// by 4,728 instead of 10,000. Requirement 27: credit-lien consumption MUST be independent of
/// caller ordering.
#[test]
fn issue172_small_default_consumes_unowed_backing_when_the_loser_is_cranked_first() {
    let case = Case {
        initial_margin_bps: 1_000,
        maintenance_margin_bps: 500,
        price_cap_bps: 400,
        loser_deposit: 10_000,
        winner_deposit: 100_000,
        units: 9,
        winner_long: true,
        steps: 4,
        provider: None,
    };
    assert_order_independent(case, 100_000 + 10_000);
}

/// The residual share exceeds the loser-paid backing. Winner first: 1,597,135 (both deposits,
/// the loser is wiped out). Loser first: the winner is charged the shortfall a second time and
/// the difference is stranded in the vault.
#[test]
fn issue172_winner_pays_the_default_twice_when_the_loser_is_cranked_first() {
    let case = Case {
        initial_margin_bps: 5_000,
        maintenance_margin_bps: 2_500,
        price_cap_bps: 2_000,
        loser_deposit: 692_730,
        winner_deposit: 904_405,
        units: 85,
        winner_long: true,
        steps: 6,
        provider: None,
    };
    assert_order_independent(case, 904_405 + 692_730);
}

/// Same mechanism with a backing provider on the loser's domain: loser first, the cure draws the
/// provider's backing for a share nobody is owed. Winner equity is unchanged; the provider's
/// backing is consumed and the value is stranded.
#[test]
fn issue172_provider_pays_the_default_when_the_loser_is_cranked_first() {
    let case = Case {
        initial_margin_bps: 2_000,
        maintenance_margin_bps: 1_000,
        price_cap_bps: 500,
        loser_deposit: 200_000,
        winner_deposit: 1_000_000,
        units: 100,
        winner_long: false,
        steps: 6,
        // Domain 0 = asset 0 long side, where the long loser's payments back short gains.
        provider: Some((0, 78_464)),
    };
    assert_order_independent(case, 1_000_000 + 200_000);
}
