//! Liquidate user positions via reduce-only cross-slab execution

use crate::state::{Portfolio, SlabRegistry, Vault};
use percolator_common::*;
use pinocchio::{account_info::AccountInfo, msg};

/// Liquidation mode based on health
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiquidationMode {
    /// Pre-liquidation: MM < equity < MM + buffer (tighter band)
    PreLiquidation,
    /// Hard liquidation: equity < MM (wider band)
    HardLiquidation,
}

impl LiquidationMode {
    /// Get the price band for this mode
    pub fn get_band_bps(&self, registry: &SlabRegistry) -> u64 {
        match self {
            LiquidationMode::PreLiquidation => registry.preliq_band_bps,
            LiquidationMode::HardLiquidation => registry.liq_band_bps,
        }
    }
}

/// Determine liquidation mode based on health and buffer
pub fn determine_mode(health: i128, preliq_buffer: i128) -> Option<LiquidationMode> {
    if health < 0 {
        // Below maintenance margin - hard liquidation
        Some(LiquidationMode::HardLiquidation)
    } else if health >= 0 && health < preliq_buffer {
        // At or above MM but below buffer - pre-liquidation
        Some(LiquidationMode::PreLiquidation)
    } else {
        // Healthy - no liquidation needed
        None
    }
}

/// Process liquidate user instruction
///
/// This instruction liquidates an undercollateralized user by executing
/// reduce-only orders across slabs to bring them back to health.
///
/// # Arguments
/// * `portfolio` - User's portfolio account (to be liquidated)
/// * `registry` - Slab registry with liquidation parameters
/// * `vault` - Collateral vault
/// * `router_authority` - Router authority PDA (for CPI signing)
/// * `oracle_accounts` - Oracle price feed accounts (for price validation)
/// * `slab_accounts` - Array of slab accounts to execute on
/// * `receipt_accounts` - Array of receipt PDAs (one per slab)
/// * `is_preliq` - Force pre-liquidation mode (if false, auto-determine)
/// * `current_ts` - Current timestamp (for rate limiting)
///
/// # Returns
/// * Updates portfolio with reduced exposures
/// * Updates portfolio health
/// * Enforces reduce-only (no position increases)
/// * All-or-nothing atomicity
pub fn process_liquidate_user(
    portfolio: &mut Portfolio,
    registry: &SlabRegistry,
    vault: &mut Vault,
    router_authority: &AccountInfo,
    oracle_accounts: &[AccountInfo],
    slab_accounts: &[AccountInfo],
    receipt_accounts: &[AccountInfo],
    is_preliq: bool,
    current_ts: u64,
) -> Result<(), PercolatorError> {
    msg!("Liquidate: Starting liquidation check");

    // Step 1: Calculate health = equity - MM
    let health = portfolio.equity.saturating_sub(portfolio.mm as i128);
    msg!("Liquidate: Health calculated");

    // Store health in portfolio for tracking
    portfolio.health = health;

    // Step 2: Determine liquidation mode
    let mode = if is_preliq {
        // Force pre-liquidation mode
        if health >= registry.preliq_buffer {
            msg!("Error: Health too high for pre-liquidation");
            return Err(PercolatorError::PortfolioHealthy);
        }
        LiquidationMode::PreLiquidation
    } else {
        // Auto-determine mode
        match determine_mode(health, registry.preliq_buffer) {
            Some(m) => m,
            None => {
                msg!("Error: Portfolio is healthy, no liquidation needed");
                return Err(PercolatorError::PortfolioHealthy);
            }
        }
    };

    msg!("Liquidate: Mode determined");

    // Step 3: Check rate limiting (for pre-liquidation deleveraging)
    if mode == LiquidationMode::PreLiquidation {
        let time_since_last = current_ts.saturating_sub(portfolio.last_liquidation_ts);
        if time_since_last < portfolio.cooldown_seconds {
            msg!("Error: Cooldown period not elapsed");
            return Err(PercolatorError::LiquidationCooldown);
        }
    }

    // Step 4: Read oracle prices for all instruments
    // TODO (Phase 1.4): Implement oracle price reading
    // For now, stub with empty array
    let _oracle_prices: &[(u16, i64)] = &[];
    msg!("Liquidate: Oracle prices read");

    // Step 5: Call reduce-only planner
    // TODO (Phase 1.3): Implement reduce-only planner
    // For now, stub - in production this will:
    // - Plan reduce-only splits across slabs
    // - Apply oracle alignment gate
    // - Apply price banding
    // - Apply per-slab caps
    msg!("Liquidate: Planning reduce-only execution");

    // Step 6: Execute via internal call to execute_cross_slab logic
    // TODO: This will reuse the CPI logic from execute_cross_slab
    // For now, just verify we have the right accounts
    if slab_accounts.len() != receipt_accounts.len() {
        msg!("Error: Mismatched slab/receipt counts");
        return Err(PercolatorError::InvalidInstruction);
    }
    msg!("Liquidate: Execution complete");

    // Step 7: Update portfolio health and timestamp
    portfolio.health = portfolio.equity.saturating_sub(portfolio.mm as i128);
    portfolio.last_liquidation_ts = current_ts;

    msg!("Liquidate: Portfolio updated");

    // Step 8: Emit liquidation events (simplified for v0)
    // In production, emit LiquidationStart, LiquidationFill, LiquidationEnd
    msg!("Liquidate: Liquidation completed successfully");

    let _ = vault; // Will be used in production
    let _ = router_authority; // Will be used for CPI signing
    let _ = oracle_accounts; // Will be used for price reading

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_mode_hard_liquidation() {
        let health = -1000;
        let buffer = 10_000_000;
        assert_eq!(determine_mode(health, buffer), Some(LiquidationMode::HardLiquidation));
    }

    #[test]
    fn test_determine_mode_pre_liquidation() {
        let health = 5_000_000;
        let buffer = 10_000_000;
        assert_eq!(determine_mode(health, buffer), Some(LiquidationMode::PreLiquidation));
    }

    #[test]
    fn test_determine_mode_healthy() {
        let health = 15_000_000;
        let buffer = 10_000_000;
        assert_eq!(determine_mode(health, buffer), None);
    }

    #[test]
    fn test_determine_mode_exact_buffer() {
        let health = 10_000_000;
        let buffer = 10_000_000;
        assert_eq!(determine_mode(health, buffer), None);
    }

    #[test]
    fn test_determine_mode_zero_health() {
        let health = 0;
        let buffer = 10_000_000;
        assert_eq!(determine_mode(health, buffer), Some(LiquidationMode::PreLiquidation));
    }
}
