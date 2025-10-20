//! Execute cross-slab order - v0 main instruction

use crate::state::{Portfolio, Vault};
use percolator_common::*;
use pinocchio::{account_info::AccountInfo, msg, pubkey::Pubkey};

/// Slab split - how much to execute on each slab
#[derive(Debug, Clone, Copy)]
pub struct SlabSplit {
    /// Slab account pubkey
    pub slab_id: Pubkey,
    /// Quantity to execute on this slab (1e6 scale)
    pub qty: i64,
    /// Side (0 = buy, 1 = sell)
    pub side: u8,
    /// Limit price (1e6 scale)
    pub limit_px: i64,
}

/// Process execute cross-slab order (v0 main instruction)
///
/// This is the core v0 instruction that proves portfolio netting.
/// Router reads QuoteCache from multiple slabs, splits the order,
/// CPIs to each slab's commit_fill, aggregates receipts, and
/// updates portfolio with net exposure.
///
/// # Arguments
/// * `portfolio` - User's portfolio account
/// * `user` - User pubkey (signer)
/// * `vault` - Collateral vault
/// * `slab_accounts` - Array of slab accounts to execute on
/// * `receipt_accounts` - Array of receipt PDAs (one per slab)
/// * `splits` - How to split the order across slabs
///
/// # Returns
/// * Updates portfolio with net exposures
/// * Checks margin on net exposure (capital efficiency!)
/// * All-or-nothing atomicity
pub fn process_execute_cross_slab(
    portfolio: &mut Portfolio,
    user: &Pubkey,
    vault: &mut Vault,
    slab_accounts: &[AccountInfo],
    receipt_accounts: &[AccountInfo],
    splits: &[SlabSplit],
) -> Result<(), PercolatorError> {
    // Verify portfolio belongs to user
    if &portfolio.user != user {
        msg!("Error: Portfolio does not belong to user");
        return Err(PercolatorError::InvalidPortfolio);
    }

    // Verify we have matching number of slabs and receipts
    if slab_accounts.len() != receipt_accounts.len() || slab_accounts.len() != splits.len() {
        msg!("Error: Mismatched slab/receipt/split counts");
        return Err(PercolatorError::InvalidInstruction);
    }

    // Phase 1: Read QuoteCache from each slab (v0 - skip validation for now)
    // In production, we'd validate seqno consistency here (TOCTOU safety)

    // Phase 2: CPI to each slab's commit_fill
    // For v0, we'll stub out actual CPI and simulate the fills
    // Real CPI will be added when we wire up slab program ID
    msg!("Executing fills on slabs");

    // Phase 3: Aggregate fills and update portfolio
    // For each split, update the portfolio exposure
    for (i, split) in splits.iter().enumerate() {
        // In v0, assume fill is successful
        let filled_qty = split.qty;

        // Update portfolio exposure for this slab/instrument
        // For v0, we'll use slab index and instrument 0 (simplified)
        let slab_idx = i as u16;
        let instrument_idx = 0u16;

        // Get current exposure
        let current_exposure = portfolio.get_exposure(slab_idx, instrument_idx);

        // Update based on side: Buy = add qty, Sell = subtract qty
        let new_exposure = if split.side == 0 {
            // Buy
            current_exposure + filled_qty
        } else {
            // Sell
            current_exposure - filled_qty
        };

        portfolio.update_exposure(slab_idx, instrument_idx, new_exposure);
    }

    // Phase 4: Calculate IM on net exposure (THE CAPITAL EFFICIENCY PROOF!)
    // For v0, use simplified margin calculation:
    // - Calculate net exposure across all slabs for same instrument
    // - IM = abs(net_exposure) * notional_value * imr_factor
    let net_exposure = calculate_net_exposure(portfolio);
    let im_required = calculate_initial_margin(net_exposure, splits);

    msg!("Calculated margin on net exposure");

    portfolio.update_margin(im_required, im_required / 2); // MM = IM / 2 for v0

    // Phase 5: Check if portfolio has sufficient margin
    // For v0, we assume equity is managed separately via vault
    // In production, this would check vault.equity >= portfolio.im
    if !portfolio.has_sufficient_margin() {
        msg!("Error: Insufficient margin");
        return Err(PercolatorError::PortfolioInsufficientMargin);
    }

    let _ = vault; // Will be used in production for equity checks
    let _ = receipt_accounts; // Will be used for real CPI

    msg!("ExecuteCrossSlab completed successfully");
    Ok(())
}

/// Calculate net exposure across all slabs for the same instrument (v0 simplified)
fn calculate_net_exposure(portfolio: &Portfolio) -> i64 {
    // For v0, sum all exposures (assuming same instrument across slabs)
    let mut net = 0i64;
    for i in 0..portfolio.exposure_count as usize {
        net += portfolio.exposures[i].2;
    }
    net
}

/// Calculate initial margin requirement (v0 simplified)
fn calculate_initial_margin(net_exposure: i64, splits: &[SlabSplit]) -> u128 {
    // For v0, simplified: IM = abs(net_exposure) * avg_price * 0.1 (10% IMR)
    if splits.is_empty() {
        return 0;
    }

    let abs_exposure = net_exposure.abs() as u128;
    let avg_price = splits[0].limit_px as u128; // Use first split price

    // IM = abs(net_exposure) * price * 0.1 / 1e6 (scale factor)
    // For v0 proof: if net_exposure = 0, IM = 0!
    (abs_exposure * avg_price * 10) / (100 * 1_000_000)
}
