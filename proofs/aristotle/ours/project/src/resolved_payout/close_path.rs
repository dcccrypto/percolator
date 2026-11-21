//! NOT engine code: a transcription over plain values of the value-moving part of the engine's
//! resolved close. Each function names the engine lines (src/v16.rs at c38b6b1e) it transcribes.
//! The engine keeps these fields in zero-copy account views (`self.header.*`, `account.header.*`);
//! here they are plain struct fields, and the encode/decode calls (`try_to_runtime`,
//! `from_runtime`, `V16PodU128::new/get`, `decode_bool`) are dropped. Every other difference is
//! marked `DIFF` and listed in aristotle/resolved_close_spec.md.

use super::*;

/// The market-header fields the path reads and writes
/// (`MarketGroupV16HeaderAccount`: `vault`, `c_tot`, `insurance`, `resolved_payout_ledger`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoolV16 {
    pub vault: u128,
    pub c_tot: u128,
    pub insurance: u128,
    pub ledger: ResolvedPayoutLedgerV16,
}

/// The portfolio-account fields the path reads and writes
/// (`PortfolioAccountV16Account` header: `capital`, `pnl`, `resolved_payout_receipt`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClaimantV16 {
    pub capital: u128,
    pub pnl: i128,
    pub receipt: ResolvedPayoutReceiptV16,
}

/// Transcribes `create_resolved_payout_receipt_if_needed`, src/v16.rs:25203-25245.
/// DIFF 1: the engine first calls `initialize_resolved_payout_ledger_if_needed` (25177-25201),
/// which captures the snapshot once; here the snapshot is already captured (it is the
/// precondition `Inv`).
pub fn create_resolved_payout_receipt_if_needed(
    pool: &mut PoolV16,
    account: &mut ClaimantV16,
) -> V16Result<()> {
    if account.receipt.present {
        return Ok(());
    }
    let terminal_positive_claim_face = account.pnl.max(0) as u128;
    let prior_bound_contribution_num =
        V16Core::bound_num_from_amount(terminal_positive_claim_face)?;
    let mut ledger = pool.ledger;
    if V16Core::bound_num_from_amount(terminal_positive_claim_face)?
        > prior_bound_contribution_num
        || prior_bound_contribution_num > ledger.terminal_claim_bound_unreceipted_num
    {
        ledger.payout_halted = true;
        pool.ledger = ledger;
        return Err(V16Error::RecoveryRequired);
    }
    ledger.terminal_claim_bound_unreceipted_num = ledger
        .terminal_claim_bound_unreceipted_num
        .checked_sub(prior_bound_contribution_num)
        .ok_or(V16Error::CounterUnderflow)?;
    ledger.terminal_claim_exact_receipts_num = ledger
        .terminal_claim_exact_receipts_num
        .checked_add(V16Core::bound_num_from_amount(
            terminal_positive_claim_face,
        )?)
        .ok_or(V16Error::ArithmeticOverflow)?;
    pool.ledger = ledger;
    account.receipt = ResolvedPayoutReceiptV16 {
        present: true,
        prior_bound_contribution_num,
        live_released_face_at_receipt: 0,
        terminal_positive_claim_face,
        paid_effective: 0,
        finalized: false,
    };
    // `self.recompute_resolved_payout_rate()`, src/v16.rs:25169-25175.
    pool.ledger = V16Core::kernel_recompute_resolved_payout_rate(pool.ledger)?;
    Ok(())
}

/// Transcribes the tail of `close_resolved_account_not_atomic`, src/v16.rs:26643-26691: from
/// `let mut payout_receipt = None;` to the writes of `vault`, `c_tot`, `pnl` and `capital`.
/// Returns the `payout` of `ResolvedCloseOutcomeV16::Closed { payout }`.
/// DIFF 2: the engine reaches this tail only for a flat account whose losses are settled; every
/// earlier exit returns `ProgressOnly` (26584-26642). Here the one part of that condition the
/// tail depends on, `pnl >= 0`, is an explicit guard.
/// DIFF 3: `clear_fully_diluted_resolved_receipt_if_terminal` (26672, body 26140-26160) is
/// omitted: it can only replace a receipt whose claimable amount is 0 by the empty receipt.
/// DIFF 4: omitted writes that move no value in this slice's state: `set_account_pnl(account, 0)`
/// is `account.pnl = 0` here (the engine also updates the positive-pnl aggregates),
/// `reserved_pnl`, `fee_credits`, `health_cert.valid`; and the self-checks
/// `TokenValueFlowProofV16::capital_and_resolved_payout_to_external_out(..).validate()`,
/// `validate_shape`, `validate_with_market` (26692-26701), which can only turn Ok into Err.
pub fn close_resolved_payout_tail(
    pool: &mut PoolV16,
    account: &mut ClaimantV16,
) -> V16Result<u128> {
    if account.pnl < 0 {
        return Err(V16Error::NonProgress);
    }
    let mut payout_receipt = None;
    let pnl_payout = if account.pnl > 0 || account.receipt.present {
        create_resolved_payout_receipt_if_needed(pool, account)?;
        let receipt = account.receipt;
        // `self.resolved_receipt_claimable_now(receipt)`, src/v16.rs:25247-25250.
        let claimable =
            MarketGroupV16ViewMut::resolved_receipt_claimable_against_ledger(receipt, pool.ledger)?;
        payout_receipt = Some(receipt);
        claimable
    } else {
        0
    };
    let account_capital = account.capital;
    let payout = account_capital
        .checked_add(pnl_payout)
        .ok_or(V16Error::ArithmeticOverflow)?
        .min(pool.vault);
    let capital_paid = account_capital.min(payout);
    let resolved_paid = payout
        .checked_sub(capital_paid)
        .ok_or(V16Error::CounterUnderflow)?;
    if let Some(mut receipt) = payout_receipt {
        receipt = apply_resolved_payout_receipt_payment(receipt, resolved_paid)?;
        account.receipt = receipt;
    }
    pool.vault = pool
        .vault
        .checked_sub(payout)
        .ok_or(V16Error::CounterUnderflow)?;
    pool.c_tot = pool
        .c_tot
        .saturating_sub(account_capital.min(pool.c_tot));
    account.pnl = 0;
    account.capital = 0;
    Ok(payout)
}

/// Transcribes `claim_resolved_payout_topup_core_not_atomic`, src/v16.rs:26492-26525.
/// DIFF 3 and DIFF 4 apply (the receipt clearing at 26501 and the flow self-check at 26516-26523).
pub fn claim_resolved_payout_topup_core(
    pool: &mut PoolV16,
    account: &mut ClaimantV16,
) -> V16Result<u128> {
    let mut receipt = account.receipt;
    let claimable =
        MarketGroupV16ViewMut::resolved_receipt_claimable_against_ledger(receipt, pool.ledger)?;
    if claimable == 0 {
        return Ok(0);
    }
    let payout = claimable.min(pool.vault);
    receipt = apply_resolved_payout_receipt_payment(receipt, payout)?;
    pool.vault = pool
        .vault
        .checked_sub(payout)
        .ok_or(V16Error::CounterUnderflow)?;
    account.receipt = receipt;
    Ok(payout)
}

/// NOT in the engine (the engine closes one account per call; the caller chooses the order).
/// Closes `accounts[order[0]]`, `accounts[order[1]]`, ... in that order and returns the sum of
/// the payouts. This is what makes "any number of accounts, any order" a statement about one
/// function.
pub fn close_all(
    pool: &mut PoolV16,
    accounts: &mut [ClaimantV16],
    order: &[usize],
) -> V16Result<u128> {
    let mut total: u128 = 0;
    let mut k = 0usize;
    while k < order.len() {
        let i = order[k];
        if i >= accounts.len() {
            return Err(V16Error::InvalidLeg);
        }
        let paid = close_resolved_payout_tail(pool, &mut accounts[i])?;
        total = total
            .checked_add(paid)
            .ok_or(V16Error::ArithmeticOverflow)?;
        k += 1;
    }
    Ok(total)
}
