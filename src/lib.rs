//! Percolator risk engine — v16.
//!
//! v16 keeps the account-local engine surface and adds source-domain realizable
//! credit accounting so positive PnL cannot be used beyond proven source-domain
//! backing.

#![no_std]
#![deny(unsafe_code)]

extern crate alloc;

#[cfg(kani)]
extern crate kani;

pub const POS_SCALE: u128 = 1_000_000;
pub const ADL_ONE: u128 = 1_000_000_000_000_000;
pub const MIN_A_SIDE: u128 = 100_000_000_000_000;
pub const MAX_ORACLE_PRICE: u64 = 1_000_000_000_000;
pub const FUNDING_DEN: u128 = 1_000_000_000;
pub const STRESS_CONSUMPTION_SCALE: u128 = 1_000_000_000;
pub const SOCIAL_WEIGHT_SCALE: u128 = ADL_ONE;
pub const SOCIAL_LOSS_DEN: u128 = 1_000_000_000_000_000_000_000;
pub const SUPPORT_WEIGHT_SCALE: u128 = 1_000_000;
pub const FULL_SUPPORT_WEIGHT: u128 = SUPPORT_WEIGHT_SCALE;
pub const BOUND_SCALE: u128 = 1_000_000_000_000;
pub const CREDIT_RATE_SCALE: u128 = 1_000_000_000_000;
pub const MAX_VAULT_TVL: u128 = 10_000_000_000_000_000;
pub const MAX_POSITION_ABS_Q: u128 = 100_000_000_000_000;
pub const MAX_ACCOUNT_NOTIONAL: u128 = 100_000_000_000_000_000_000;
pub const MAX_TRADE_SIZE_Q: u128 = MAX_POSITION_ABS_Q;
pub const MAX_OI_SIDE_Q: u128 = 100_000_000_000_000;
pub const MAX_TRADING_FEE_BPS: u64 = 10_000;
pub const MAX_MARGIN_BPS: u64 = 10_000;
pub const MAX_LIQUIDATION_FEE_BPS: u64 = 10_000;
pub const MAX_PROTOCOL_FEE_ABS: u128 = 1_000_000_000_000_000_000_000_000_000_000_000_000;
pub const MAX_WARMUP_SLOTS: u64 = u64::MAX;
pub const MAX_RESOLVE_PRICE_DEVIATION_BPS: u64 = 10_000;
pub const MAX_RECOVERY_FALLBACK_DEVIATION_BPS: u64 = MAX_RESOLVE_PRICE_DEVIATION_BPS;

pub mod v16;
#[cfg(kani)]
pub mod wide_math;
#[cfg(not(kani))]
mod wide_math;

pub use v16::*;
