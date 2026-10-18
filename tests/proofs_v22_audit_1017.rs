#![cfg(kani)]
//! v2.2 full audit 2026-10-17, workstream K: engine value-flow conservation at the PUBLIC
//! deposit / withdraw entry points, full u128 width, no u8 box. The pre-existing view-level rows
//! (`proof_v16_view_overwithdraw_rejects`, `..._withdraw_reduces_vault_ctot_and_capital_equally`,
//! `..._deposit_preserves_c_tot_vault_capital_sum`, `..._nonflat_withdraw_rejects_before_value_exit`)
//! are NO-VERDICT-OOM: `validate_shape` / `validate_with_market` (whole-market audit scans) dominate.
//!
//! Strategy: the two validators are replaced by `Ok(())` stubs. This is SOUND for these claims:
//! the claims are about what a SUCCESSFUL call does, and an `Ok` stub only ADDS states (those the
//! real validators would have rejected) and removes no successful execution of the function body.
//! The validators are read-only (`&self`), so no mutation is dropped. The fixture state is the
//! engine's own constructor output plus symbolic numeric fields.
//!
//! Each harness has a negative result path with covers, so a stub that made the success path
//! unreachable would show up as an unsatisfied cover.
#![allow(dead_code, unused_imports, unused_mut)]

use percolator::v16::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16View,
    MarketGroupV16ViewMut, PortfolioAccountV16Account, PortfolioV16View, PortfolioV16ViewMut,
    ProvenanceHeaderV16, ProvenanceHeaderV16Account, V16Config, V16Error, V16PodI128, V16PodU128,
    V16Result,
};
use percolator::MAX_VAULT_TVL;

fn ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    ([1; 32], [2; 32], [3; 32])
}

fn empty_account_fixture(market_id: [u8; 32], account_tag: u8) -> PortfolioAccountV16Account {
    let mut account_id = [0u8; 32];
    account_id[0] = account_tag;
    let mut owner = [0u8; 32];
    owner[0] = account_tag;
    PortfolioAccountV16Account::try_empty(ProvenanceHeaderV16Account::from_runtime(
        &ProvenanceHeaderV16::new(market_id, account_id, owner),
    ))
    .unwrap()
}

fn one_market_view_fixture() -> (
    MarketGroupV16HeaderAccount,
    [Market<u64>; 1],
    PortfolioAccountV16Account,
) {
    let (market_id, _, _) = ids();
    let cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(market_id, cfg, 1, 0).unwrap();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    {
        let mut view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        view.activate_empty_market_not_atomic(0, 100, 1).unwrap();
    }
    let account_header = empty_account_fixture(market_id, 2);
    (header, markets, account_header)
}

fn account_validator_ok<T>(_s: &PortfolioV16View<'_>, _m: &MarketGroupV16View<'_, T>) -> V16Result<()> {
    Ok(())
}
fn market_shape_ok<T>(_s: &MarketGroupV16View<'_, T>) -> V16Result<()> {
    Ok(())
}

/// K-E1: a successful `withdraw_not_atomic` moves EXACTLY `amount` out of account capital, `c_tot` and
/// `vault` (all three by the same amount), never more than the account's capital, never touching
/// insurance or PnL; the vault-minus-senior slack (`vault - c_tot - insurance`) is unchanged
/// (nothing minted, nothing leaked). A refused over-withdraw changes nothing.
/// Full u128 width for capital / c_tot / vault / amount under the engine's own bounds
/// (`capital <= c_tot`, `c_tot + insurance <= vault <= MAX_VAULT_TVL`); pnl >= 0 (flat solvent
/// account; the negative-pnl settlement branch is a separate kernel).
#[kani::proof]
#[kani::unwind(70)]
#[kani::stub(percolator::v16::PortfolioV16View::validate_with_market, account_validator_ok)]
#[kani::stub(percolator::v16::MarketGroupV16View::validate_shape, market_shape_ok)]
fn kani_audit1017_e1_withdraw_exact_value_flow() {
    let capital: u128 = kani::any();
    let c_tot: u128 = kani::any();
    let insurance: u128 = kani::any();
    let vault: u128 = kani::any();
    let amount: u128 = kani::any();
    let pnl: i128 = kani::any();
    let fee_credits: i128 = kani::any();
    kani::assume(capital <= c_tot);
    kani::assume(c_tot.checked_add(insurance).is_some_and(|s| s <= vault));
    kani::assume(vault <= MAX_VAULT_TVL);
    kani::assume(pnl >= 0);

    let (mut header, mut markets, mut account_header) = one_market_view_fixture();
    header.c_tot = V16PodU128::new(c_tot);
    header.insurance = V16PodU128::new(insurance);
    header.vault = V16PodU128::new(vault);
    account_header.capital = V16PodU128::new(capital);
    account_header.pnl = V16PodI128::new(pnl);
    account_header.fee_credits = V16PodI128::new(fee_credits);
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header);

    let r = market.withdraw_not_atomic(&mut account, amount);

    match r {
        Ok(()) => {
            if amount == 0 {
                assert_eq!(account.header.capital.get(), capital);
                assert_eq!(market.header.vault.get(), vault);
                kani::cover!(true, "zero withdraw is a no-op");
            } else {
                assert!(amount <= capital);
                assert_eq!(account.header.capital.get(), capital - amount);
                assert_eq!(market.header.c_tot.get(), c_tot - amount);
                assert_eq!(market.header.vault.get(), vault - amount);
                assert_eq!(market.header.insurance.get(), insurance);
                assert_eq!(account.header.pnl.get(), pnl);
                // nothing minted, nothing leaked: slack over the senior claims is unchanged
                assert_eq!(market.header.vault.get() - market.header.c_tot.get(), vault - c_tot);
                assert!(market.header.vault.get() >= market.header.c_tot.get() + market.header.insurance.get());
                kani::cover!(amount > 0 && amount < capital, "partial withdraw succeeds");
                kani::cover!(amount > 0 && amount == capital, "full-capital withdraw succeeds");
                kani::cover!(insurance > 0 && vault > c_tot + insurance, "non-trivial insurance and slack");
            }
        }
        Err(_) => {
            if amount > capital {
                assert_eq!(r, Err(V16Error::LockActive));
                assert_eq!(account.header.capital.get(), capital);
                assert_eq!(market.header.c_tot.get(), c_tot);
                assert_eq!(market.header.vault.get(), vault);
                assert_eq!(market.header.insurance.get(), insurance);
                kani::cover!(capital > 0, "over-withdraw of a funded account refused unchanged");
                kani::cover!(capital == 0, "over-withdraw of an empty account refused unchanged");
            }
        }
    }
}

/// K-E2: a successful `deposit_not_atomic` adds EXACTLY `amount` to account capital, `c_tot` and
/// `vault`; insurance and PnL untouched; slack unchanged (no mint); overflow or a non-Live market
/// refuses without mutation.
#[kani::proof]
#[kani::unwind(70)]
#[kani::stub(percolator::v16::PortfolioV16View::validate_with_market, account_validator_ok)]
#[kani::stub(percolator::v16::MarketGroupV16View::validate_shape, market_shape_ok)]
fn kani_audit1017_e2_deposit_exact_value_flow() {
    let capital: u128 = kani::any();
    let c_tot: u128 = kani::any();
    let insurance: u128 = kani::any();
    let vault: u128 = kani::any();
    let amount: u128 = kani::any();
    kani::assume(capital <= c_tot);
    kani::assume(c_tot.checked_add(insurance).is_some_and(|s| s <= vault));
    kani::assume(vault <= MAX_VAULT_TVL);

    let (mut header, mut markets, mut account_header) = one_market_view_fixture();
    header.c_tot = V16PodU128::new(c_tot);
    header.insurance = V16PodU128::new(insurance);
    header.vault = V16PodU128::new(vault);
    account_header.capital = V16PodU128::new(capital);
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header);

    let r = market.deposit_not_atomic(&mut account, amount);

    match r {
        Ok(()) => {
            if amount == 0 {
                assert_eq!(account.header.capital.get(), capital);
                assert_eq!(market.header.vault.get(), vault);
                kani::cover!(true, "zero deposit is a no-op");
            } else {
                assert_eq!(account.header.capital.get(), capital + amount);
                assert_eq!(market.header.c_tot.get(), c_tot + amount);
                assert_eq!(market.header.vault.get(), vault + amount);
                assert_eq!(market.header.insurance.get(), insurance);
                assert_eq!(market.header.vault.get() - market.header.c_tot.get(), vault - c_tot);
                assert!(market.header.vault.get() >= market.header.c_tot.get() + market.header.insurance.get());
                kani::cover!(amount > 0 && capital > 0, "deposit into a funded account");
                kani::cover!(insurance > 0, "with insurance present");
            }
        }
        Err(_) => {
            // with the validators stubbed the ONLY refusal on a Live market is arithmetic overflow,
            // and it happens before any write: nothing moved.
            assert!(vault.checked_add(amount).is_none());
            assert_eq!(account.header.capital.get(), capital);
            assert_eq!(market.header.c_tot.get(), c_tot);
            assert_eq!(market.header.vault.get(), vault);
            kani::cover!(capital > 0, "overflowing deposit into a funded account refused unchanged");
        }
    }
    // completeness: Ok exactly when no counter overflows
    assert_eq!(r.is_ok(), vault.checked_add(amount).is_some());
}

/// K-E3: a Resolved close of a FLAT, non-negative-PnL account pays out exactly its capital, never more
/// than the vault holds, and moves vault and `c_tot` down by exactly that payout (insurance untouched).
/// Full u128 width. This upgrades `proof_v16_public_resolved_close_flat_account_pays_only_capital_and_vault`
/// (capital in 1..=5 on a u8) which has no wider-domain sibling. Same stub rule as E1/E2.
#[kani::proof]
#[kani::unwind(70)]
#[kani::stub(percolator::v16::PortfolioV16View::validate_with_market, account_validator_ok)]
#[kani::stub(percolator::v16::MarketGroupV16View::validate_shape, market_shape_ok)]
fn kani_audit1017_e3_resolved_close_pays_capital_and_never_more_than_vault() {
    use percolator::v16::{ResolvedCloseOutcomeV16, V16PodU64};
    let capital: u128 = kani::any();
    let c_tot: u128 = kani::any();
    let insurance: u128 = kani::any();
    let vault: u128 = kani::any();
    kani::assume(capital <= c_tot);
    kani::assume(c_tot.checked_add(insurance).is_some_and(|s| s <= vault));
    kani::assume(vault <= MAX_VAULT_TVL);

    let (mut header, mut markets, mut account_header) = one_market_view_fixture();
    header.mode = 1;
    header.current_slot = V16PodU64::new(2);
    header.resolved_slot = V16PodU64::new(2);
    header.vault = V16PodU128::new(vault);
    header.c_tot = V16PodU128::new(c_tot);
    header.insurance = V16PodU128::new(insurance);
    account_header.capital = V16PodU128::new(capital);
    account_header.pnl = V16PodI128::new(0);
    account_header.last_fee_slot = V16PodU64::new(2);
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header);

    let r = market.close_resolved_account_not_atomic(&mut account, 0);
    match r {
        Ok(ResolvedCloseOutcomeV16::Closed { payout }) => {
            assert!(payout == capital);
            assert!(payout <= vault);
            assert_eq!(market.header.vault.get(), vault - payout);
            assert_eq!(market.header.c_tot.get(), c_tot - capital);
            assert_eq!(market.header.insurance.get(), insurance);
            assert_eq!(account.header.capital.get(), 0);
            kani::cover!(payout > 0 && payout < vault, "positive payout with vault slack left");
            kani::cover!(payout > 0 && payout == vault, "payout drains the vault exactly");
            kani::cover!(insurance > 0, "insurance present and untouched");
        }
        _ => {
            kani::cover!(true, "other outcome (refusal / not closed)");
        }
    }
}
