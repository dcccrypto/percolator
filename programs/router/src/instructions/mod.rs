/// Router instruction handlers (v0 minimal)

pub mod initialize;
pub mod initialize_portfolio;
pub mod deposit;
pub mod withdraw;
pub mod execute_cross_slab;
pub mod liquidate_user;

pub use initialize::*;
pub use initialize_portfolio::*;
pub use deposit::*;
pub use withdraw::*;
pub use execute_cross_slab::*;
pub use liquidate_user::*;

/// Instruction discriminator (v0 minimal)
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouterInstruction {
    /// Initialize router registry
    Initialize = 0,
    /// Initialize user portfolio
    InitializePortfolio = 1,
    /// Deposit collateral to vault
    Deposit = 2,
    /// Withdraw collateral from vault
    Withdraw = 3,
    /// Execute cross-slab order (v0 main instruction)
    ExecuteCrossSlab = 4,
    /// Liquidate user positions (reduce-only)
    LiquidateUser = 5,
}

// Note: Instruction dispatching is handled in entrypoint.rs
// The functions in this module are called from the entrypoint after
// account deserialization and validation.
