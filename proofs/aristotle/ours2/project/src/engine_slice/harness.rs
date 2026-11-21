//! NOT engine code. The engine closes one account per call and the caller picks the order;
//! `close_all` loops over `order` so that "any number of accounts, any order" is a statement
//! about one function. `close_one` and `topup_one` build the two views and call one engine
//! function each.

use super::*;

/// One call of the engine's close tail (`close_resolved_account_tail_not_atomic`, whose body is
/// src/v16.rs:26643-26702) on one account. The tail always ends in `Closed { payout }` or `Err`.
pub fn close_one(
    market: &mut MarketGroupV16HeaderAccount,
    account: &mut PortfolioAccountV16Account,
) -> V16Result<u128> {
    let mut market_view = MarketGroupV16ViewMut { header: market };
    let mut account_view = PortfolioV16ViewMut { header: account };
    match market_view.close_resolved_account_tail_not_atomic(&mut account_view)? {
        ResolvedCloseOutcomeV16::Closed { payout } => Ok(payout),
        ResolvedCloseOutcomeV16::ProgressOnly => Err(V16Error::NonProgress),
    }
}

/// One call of the engine's `claim_resolved_payout_topup_core_not_atomic` (src/v16.rs:26492-26525).
pub fn topup_one(
    market: &mut MarketGroupV16HeaderAccount,
    account: &mut PortfolioAccountV16Account,
) -> V16Result<u128> {
    let mut market_view = MarketGroupV16ViewMut { header: market };
    let mut account_view = PortfolioV16ViewMut { header: account };
    market_view.claim_resolved_payout_topup_core_not_atomic(&mut account_view)
}

/// Closes `accounts[order[0]]`, `accounts[order[1]]`, ... in that order and returns the sum of
/// the payouts. An index outside the slice gives `Err(InvalidLeg)`.
pub fn close_all(
    market: &mut MarketGroupV16HeaderAccount,
    accounts: &mut [PortfolioAccountV16Account],
    order: &[usize],
) -> V16Result<u128> {
    let mut total: u128 = 0;
    let mut k = 0usize;
    while k < order.len() {
        let i = order[k];
        if i >= accounts.len() {
            return Err(V16Error::InvalidLeg);
        }
        let paid = close_one(market, &mut accounts[i])?;
        total = total
            .checked_add(paid)
            .ok_or(V16Error::ArithmeticOverflow)?;
        k += 1;
    }
    Ok(total)
}
