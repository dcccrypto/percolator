//! Formally Verified Risk Engine for Perpetual DEX
//!
//! ⚠️ EDUCATIONAL USE ONLY - NOT PRODUCTION READY ⚠️
//!
//! This is an experimental research project for educational purposes only.
//! DO NOT use with real funds. Not independently audited. Not production ready.
//!
//! This module implements a formally verified risk engine that guarantees:
//! 1. User funds are safe against oracle manipulation attacks (within time window T)
//! 2. PNL warmup prevents instant withdrawal of manipulated profits
//! 3. ADL haircuts apply to unwrapped PNL first, protecting user principal
//! 4. Conservation of funds across all operations
//! 5. User isolation - one user's actions don't affect others
//!
//! All data structures are laid out in a single contiguous memory chunk,
//! suitable for a single Solana account.

#![no_std]
#![forbid(unsafe_code)]

#[cfg(kani)]
extern crate kani;

// ============================================================================
// Constants
// ============================================================================

// MAX_ACCOUNTS is feature-configured, not target-configured.
// This ensures x86 and SBF builds use the same sizes for a given feature set.
#[cfg(kani)]
pub const MAX_ACCOUNTS: usize = 4; // Small for fast formal verification (1 bitmap word, 4 bits)

#[cfg(all(feature = "test", not(kani)))]
pub const MAX_ACCOUNTS: usize = 64; // Micro: ~0.17 SOL rent

#[cfg(all(feature = "small", not(feature = "test"), not(kani)))]
pub const MAX_ACCOUNTS: usize = 256; // Small: ~0.68 SOL rent

#[cfg(all(
    feature = "medium",
    not(feature = "test"),
    not(feature = "small"),
    not(kani)
))]
pub const MAX_ACCOUNTS: usize = 1024; // Medium: ~2.7 SOL rent

#[cfg(all(
    not(kani),
    not(feature = "test"),
    not(feature = "small"),
    not(feature = "medium")
))]
pub const MAX_ACCOUNTS: usize = 4096; // Full: ~6.9 SOL rent

// Derived constants - all use size_of, no hardcoded values
pub const BITMAP_WORDS: usize = MAX_ACCOUNTS.div_ceil(64);
pub const MAX_ROUNDING_SLACK: u128 = MAX_ACCOUNTS as u128;

/// PERC-299: Number of consecutive stable slots before emergency OI mode clears.
pub const EMERGENCY_RECOVERY_SLOTS: u64 = 1000;
/// Mask for wrapping indices (MAX_ACCOUNTS must be power of 2)
const ACCOUNT_IDX_MASK: usize = MAX_ACCOUNTS - 1;

/// Maximum number of dust accounts to close per crank call.
/// Limits compute usage while still making progress on cleanup.
pub const GC_CLOSE_BUDGET: u32 = 32;

/// Number of occupied accounts to process per crank call.
/// When the system has fewer than this many accounts, one crank covers everything.
pub const ACCOUNTS_PER_CRANK: u16 = 256;

/// Hard liquidation budget per crank call (caps total work)
/// Set to 120 to keep worst-case crank CU under ~50% of Solana limit
pub const LIQ_BUDGET_PER_CRANK: u16 = 64;

/// Max number of force-realize closes per crank call.
/// Hard CU bound in force-realize mode. Liquidations are skipped when active.
pub const FORCE_REALIZE_BUDGET_PER_CRANK: u16 = 32;

/// Maximum oracle price (prevents overflow in mark_pnl calculations)
/// 10^15 allows prices up to $1B with 6 decimal places
pub const MAX_ORACLE_PRICE: u64 = 1_000_000_000_000_000;

/// MAX_FUNDING_DT: maximum sub-step size for funding transfer loop (spec §1.4).
pub const MAX_FUNDING_DT: u64 = u16::MAX as u64;

/// MAX_ABS_FUNDING_BPS_PER_SLOT: absolute bound on funding rate (spec §1.4).
pub const MAX_ABS_FUNDING_BPS_PER_SLOT: i64 = 10_000;

/// POS_SCALE = 1_000_000 (spec §1.2): position_basis_q is in micro-units.
/// notional = floor(|effective_pos_q| * oracle_price / POS_SCALE)
pub const POS_SCALE: u128 = 1_000_000;

/// Maximum absolute position size (prevents overflow in mark_pnl calculations)
/// 10^20 allows positions up to 100 billion units
/// Combined with MAX_ORACLE_PRICE, guarantees mark_pnl multiply won't overflow i128
pub const MAX_POSITION_ABS: u128 = 100_000_000_000_000_000_000;

// ============================================================================
// BPF-Safe 128-bit Types (see src/i128.rs)
// ============================================================================
pub mod i128;
pub use i128::{I128, U128};

// ============================================================================
// Wide 256-bit Arithmetic (see src/wide_math.rs)
// ============================================================================
pub mod wide_math;
use wide_math::{
    fee_debt_u128_checked, floor_div_signed_conservative_i128, mul_div_floor_u128,
    saturating_mul_u128_u64, wide_mul_div_floor_u128, wide_signed_mul_div_floor_from_k_pair,
};
pub use wide_math::{mul_div_floor_u128 as mul_div_floor_u128_pub, I256, U256};

// ============================================================================
// Core Data Structures
// ============================================================================

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountKind {
    User = 0,
    LP = 1,
}

/// Side mode for OI sides (spec §2.4)
///
/// Controls whether a given side (long/short) is accepting new positions or
/// draining as part of an ADL epoch reset.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SideMode {
    /// Normal operation — new positions accepted on this side
    Normal = 0,
    /// Drain-only — no new positions; existing ones may be closed
    DrainOnly = 1,
    /// Reset pending — waiting for OI to reach zero before resetting A/K coefficients
    ResetPending = 2,
}

/// Side of a position (Long = positive size, Short = negative size)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Long,
    Short,
}

/// Instruction-level context for deferred ADL reset scheduling (spec §5.7-5.8)
///
/// Passed through the instruction lifecycle so that `enqueue_adl` can mark
/// sides for pending reset without immediately mutating `side_mode_*`.
/// `run_end_of_instruction_lifecycle` finalises the resets at end-of-instruction.
pub struct InstructionContext {
    pub pending_reset_long: bool,
    pub pending_reset_short: bool,
}

impl InstructionContext {
    pub fn new() -> Self {
        Self {
            pending_reset_long: false,
            pending_reset_short: false,
        }
    }
}

impl Default for InstructionContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Unified account - can be user or LP
///
/// LPs are distinguished by having kind = LP and matcher_program/context set.
/// Users have kind = User and matcher arrays zeroed.
///
/// This unification ensures LPs receive the same risk management protections as users:
/// - PNL warmup
/// - ADL (Auto-Deleveraging)
/// - Liquidations
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Account {
    /// Unique account ID (monotonically increasing, never recycled)
    /// Note: Field order matches on-chain slab layout (account_id at offset 0)
    pub account_id: u64,

    // ========================================
    // Capital & PNL (universal)
    // ========================================
    /// Deposited capital (user principal or LP capital)
    /// NEVER reduced by ADL/socialization (Invariant I1)
    pub capital: U128,

    /// Account kind (User or LP)
    /// Note: Field is at offset 24 in on-chain layout, after capital
    pub kind: AccountKind,

    /// Realized PNL from trading (can be positive or negative)
    pub pnl: I128,

    /// Reserved positive PnL (R_i, spec §2.1): the portion of positive PnL still under warmup.
    /// Tracks how much of max(PNL_i, 0) is "reserved" (not yet matured/released).
    /// Invariant: 0 <= reserved_pnl <= max(PNL_i, 0)
    /// Migrated from u64 → u128 in PERC-8267 to match upstream spec §2.1.
    pub reserved_pnl: u128,

    // ========================================
    // Warmup (embedded, no separate struct)
    // ========================================
    /// Slot when warmup started
    pub warmup_started_at_slot: u64,

    /// Linear vesting rate per slot
    pub warmup_slope_per_step: U128,

    // ========================================
    // Position (universal)
    // ========================================
    /// Current position size (+ long, - short)
    pub position_size: I128,

    /// Last oracle mark price at which this account's position was settled (variation margin).
    /// NOT an average trade entry price.
    pub entry_price: u64,

    // ========================================
    // Funding (universal)
    // ========================================
    /// Funding index snapshot (quote per base, 1e6 scale)
    pub funding_index: I128,

    // ========================================
    // LP-specific (only meaningful for LP kind)
    // ========================================
    /// Matching engine program ID (zero for user accounts)
    pub matcher_program: [u8; 32],

    /// Matching engine context account (zero for user accounts)
    pub matcher_context: [u8; 32],

    // ========================================
    // Owner & Maintenance Fees (wrapper-related)
    // ========================================
    /// Owner pubkey (32 bytes, signature checks done by wrapper)
    pub owner: [u8; 32],

    /// Fee credits in capital units (can go negative if fees owed)
    pub fee_credits: I128,

    /// Last slot when maintenance fees were settled for this account
    pub last_fee_slot: u64,

    /// Last slot when a partial liquidation occurred (PERC-122 cooldown).
    pub last_partial_liquidation_slot: u64,

    // ===================================================
    // ADL per-account state (spec §2.1, T5: PERC-8270)
    // ===================================================
    /// Signed fixed-point base quantity basis (i128, spec §2.1)
    /// Replaces position_size in the two-phase crank model.
    pub position_basis_q: i128,

    /// Side multiplier snapshot at last explicit position attachment (u128)
    pub adl_a_basis: u128,

    /// K coefficient snapshot (i128)
    pub adl_k_snap: i128,

    /// Side epoch snapshot
    pub adl_epoch_snap: u64,
}

impl Account {
    /// Check if this account is an LP
    pub fn is_lp(&self) -> bool {
        matches!(self.kind, AccountKind::LP)
    }

    /// Check if this account is a regular user
    pub fn is_user(&self) -> bool {
        matches!(self.kind, AccountKind::User)
    }
}

/// Helper to create empty account
fn empty_account() -> Account {
    Account {
        account_id: 0,
        capital: U128::ZERO,
        kind: AccountKind::User,
        pnl: I128::ZERO,
        reserved_pnl: 0u128,
        warmup_started_at_slot: 0,
        warmup_slope_per_step: U128::ZERO,
        position_size: I128::ZERO,
        entry_price: 0,
        funding_index: I128::ZERO,
        matcher_program: [0; 32],
        matcher_context: [0; 32],
        owner: [0; 32],
        fee_credits: I128::ZERO,
        last_fee_slot: 0,
        last_partial_liquidation_slot: 0,
        position_basis_q: 0i128,
        adl_a_basis: 1_000_000u128, // ADL_ONE
        adl_k_snap: 0i128,
        adl_epoch_snap: 0,
    }
}

/// Insurance fund state
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InsuranceFund {
    /// Insurance fund balance (global pool)
    pub balance: U128,

    /// Accumulated fees from trades
    pub fee_revenue: U128,

    /// PERC-311: Balance incentive reserve.
    /// Funded by fee_to_balance_reserve_bps of trading fees.
    /// Pays rebates to traders who improve OI skew balance.
    pub balance_incentive_reserve: u64,

    /// Padding for 16-byte alignment.
    pub _rebate_pad: [u8; 8],

    /// PERC-306: Per-market isolated insurance balance.
    /// Drawn before global fund. Funded via FundMarketInsurance instruction.
    pub isolated_balance: U128,

    /// PERC-306: Insurance isolation BPS (max % of global fund this market can access).
    /// 0 = disabled (unlimited global access, legacy behavior).
    pub insurance_isolation_bps: u16,

    /// Padding for alignment
    pub _isolation_padding: [u8; 14],
}

/// Outcome from oracle_close_position_core helper
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClosedOutcome {
    /// Absolute position size that was closed
    pub abs_pos: u128,
    /// Mark PnL from closing at oracle price
    pub mark_pnl: i128,
    /// Capital before settlement
    pub cap_before: u128,
    /// Capital after settlement
    pub cap_after: u128,
    /// Whether a position was actually closed
    pub position_was_closed: bool,
}

/// Risk engine parameters
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RiskParams {
    /// Warmup period in slots (time T)
    pub warmup_period_slots: u64,

    /// Maintenance margin ratio in basis points (e.g., 500 = 5%)
    pub maintenance_margin_bps: u64,

    /// Initial margin ratio in basis points
    pub initial_margin_bps: u64,

    /// Trading fee in basis points
    pub trading_fee_bps: u64,

    /// Maximum number of accounts
    pub max_accounts: u64,

    /// Flat account creation fee (absolute amount in capital units)
    pub new_account_fee: U128,

    /// Insurance fund threshold for entering risk-reduction-only mode
    /// If insurance fund balance drops below this, risk-reduction mode activates
    pub risk_reduction_threshold: U128,

    // ========================================
    // Maintenance Fee Parameters
    // ========================================
    /// Maintenance fee per account per slot (in capital units)
    /// Engine is purely slot-native; any per-day conversion is wrapper/UI responsibility
    pub maintenance_fee_per_slot: U128,

    /// Maximum allowed staleness before crank is required (in slots)
    /// Set to u64::MAX to disable crank freshness check
    pub max_crank_staleness_slots: u64,

    /// Liquidation fee in basis points (e.g., 50 = 0.50%)
    /// Paid from liquidated account's capital into insurance fund
    pub liquidation_fee_bps: u64,

    /// Absolute cap on liquidation fee (in capital units)
    /// Prevents whales paying enormous fees
    pub liquidation_fee_cap: U128,

    // ========================================
    // Partial Liquidation Parameters
    // ========================================
    /// Buffer above maintenance margin (in basis points) to target after partial liquidation.
    /// E.g., if maintenance is 500 bps (5%) and buffer is 100 bps (1%), we target 6% margin.
    /// This prevents immediate re-liquidation from small price movements.
    pub liquidation_buffer_bps: u64,

    /// Minimum absolute position size after partial liquidation.
    /// If remaining position would be below this threshold, full liquidation occurs.
    /// Prevents dust positions that are uneconomical to maintain or re-liquidate.
    /// Denominated in base units (same scale as position_size.abs()).
    pub min_liquidation_abs: U128,

    // ========================================
    // Funding Rate Parameters (PERC-121)
    // ========================================
    /// Weight of premium component in funding rate (basis points, 0–10_000).
    /// 0 = premium-based funding disabled.
    pub funding_premium_weight_bps: u64,

    /// Funding settlement interval in slots.
    /// 0 = funding settlement disabled.
    pub funding_settlement_interval_slots: u64,

    /// Dampening factor for premium-based funding (fixed-point ×1e6).
    /// Must be non-zero when funding_premium_weight_bps > 0.
    pub funding_premium_dampening_e6: u64,

    /// Maximum absolute funding rate per slot (basis points).
    /// Caps the premium-based rate to prevent extreme funding.
    pub funding_premium_max_bps_per_slot: i64,

    // ========================================
    // Partial Liquidation Parameters (PERC-122)
    // ========================================
    /// Percentage of position to close per partial liquidation (bps, 0 = disabled).
    pub partial_liquidation_bps: u64,
    /// Cooldown slots between partial liquidations on the same account.
    pub partial_liquidation_cooldown_slots: u64,
    /// Use mark price (not oracle) for liquidation trigger.
    pub use_mark_price_for_liquidation: bool,
    /// Emergency liquidation margin threshold (bps). If an account's margin ratio
    /// falls below this level, cooldown is bypassed and full liquidation occurs
    /// immediately. Prevents bad debt from critically underwater accounts waiting
    /// on cooldown. Set to 0 to disable (defaults to maintenance_margin_bps / 2).
    /// Issue #300: must be < maintenance_margin_bps.
    pub emergency_liquidation_margin_bps: u64,

    // ========================================
    // Dynamic Fee Parameters (PERC-120)
    // ========================================
    /// Tier 2 trading fee in basis points (higher than base trading_fee_bps).
    pub fee_tier2_bps: u64,
    /// Tier 3 trading fee in basis points (highest tier).
    pub fee_tier3_bps: u64,
    /// Notional threshold for Tier 2 fees (in capital units, e6 scale).
    /// 0 = tiered fees disabled.
    pub fee_tier2_threshold: u128,
    /// Notional threshold for Tier 3 fees (in capital units, e6 scale).
    pub fee_tier3_threshold: u128,
    /// Fee split: LP vault share in basis points (0–10_000).
    pub fee_split_lp_bps: u64,
    /// Fee split: protocol treasury share in basis points.
    pub fee_split_protocol_bps: u64,
    /// Fee split: market creator share in basis points.
    /// Note: fee_split_lp_bps + fee_split_protocol_bps + fee_split_creator_bps must == 10_000.
    pub fee_split_creator_bps: u64,
    /// Utilization-based fee multiplier ceiling (bps above base). 0 = disabled.
    pub fee_utilization_surge_bps: u64,

    // ========================================
    // Margin Floors (spec §9.1)
    // ========================================
    /// Absolute floor for maintenance margin requirement.
    /// MM_req_i = max(proportional, min_nonzero_mm_req).
    /// Prevents microscopic positions from evading margin enforcement when
    /// proportional notional floors to zero. Set to 0 to disable.
    pub min_nonzero_mm_req: u128,
    /// Absolute floor for initial margin requirement.
    /// IM_req_i = max(proportional, min_nonzero_im_req).
    /// Must be strictly > min_nonzero_mm_req when both are non-zero.
    pub min_nonzero_im_req: u128,

    // ========================================
    // Insurance Fund Floor (spec §1.4)
    // ========================================
    /// Minimum insurance fund balance floor.
    /// Insurance fund draws stop at this floor (spec §1.4: 0 <= I_floor <= MAX_VAULT_TVL).
    /// Set to 0 to disable (default). Deployments can express nonzero insurance floors.
    pub insurance_floor: U128,
}

impl RiskParams {
    /// Validate that all parameters are within safe bounds.
    ///
    /// Returns `Err(RiskError::Overflow)` if any parameter violates a safety invariant.
    pub fn validate(&self) -> Result<()> {
        // Margins must be non-zero
        if self.maintenance_margin_bps == 0 || self.initial_margin_bps == 0 {
            return Err(RiskError::Overflow);
        }
        // Margins must not exceed 100%
        if self.initial_margin_bps > 10_000 || self.maintenance_margin_bps > 10_000 {
            return Err(RiskError::Overflow);
        }
        // Initial margin must be >= maintenance margin
        if self.initial_margin_bps < self.maintenance_margin_bps {
            return Err(RiskError::Overflow);
        }
        // Margin floors: if both non-zero, MM floor must be strictly less than IM floor (spec §9.1)
        if self.min_nonzero_mm_req > 0
            && self.min_nonzero_im_req > 0
            && self.min_nonzero_mm_req >= self.min_nonzero_im_req
        {
            return Err(RiskError::Overflow);
        }
        // max_accounts must be > 0 and within physical slab size
        if self.max_accounts == 0 || self.max_accounts > MAX_ACCOUNTS as u64 {
            return Err(RiskError::Overflow);
        }
        // warmup_period_slots must be non-zero — zero bypasses oracle manipulation delay (spec Goal 1)
        if self.warmup_period_slots == 0 {
            return Err(RiskError::Overflow);
        }
        // max_crank_staleness_slots must be non-zero (u64::MAX = disable mode is allowed)
        if self.max_crank_staleness_slots == 0 {
            return Err(RiskError::Overflow);
        }
        // Trading fee cannot exceed 100% (spec §1.4)
        if self.trading_fee_bps > 10_000 {
            return Err(RiskError::Overflow);
        }
        // Liquidation fee cannot exceed 100%
        if self.liquidation_fee_bps > 10_000 {
            return Err(RiskError::Overflow);
        }
        // Liquidation fee ordering: min_liquidation_abs <= liquidation_fee_cap (spec §1.4)
        if self.min_liquidation_abs.get() > self.liquidation_fee_cap.get() {
            return Err(RiskError::Overflow);
        }
        // Liquidation buffer cannot exceed 100%
        if self.liquidation_buffer_bps > 10_000 {
            return Err(RiskError::Overflow);
        }
        // Funding premium weight cannot exceed 100%
        if self.funding_premium_weight_bps > 10_000 {
            return Err(RiskError::Overflow);
        }
        // If funding premium is enabled, dampening must be non-zero
        if self.funding_premium_weight_bps > 0 && self.funding_premium_dampening_e6 == 0 {
            return Err(RiskError::Overflow);
        }
        if self.partial_liquidation_bps > 10_000 {
            return Err(RiskError::Overflow);
        }
        // Emergency margin must be less than maintenance margin (when set).
        // 0 = auto mode (uses maintenance_margin_bps / 2).
        if self.emergency_liquidation_margin_bps > 0
            && self.emergency_liquidation_margin_bps >= self.maintenance_margin_bps
        {
            return Err(RiskError::Overflow);
        }
        // Fee tiers must be monotonically increasing
        if self.fee_tier2_bps > 10_000 || self.fee_tier3_bps > 10_000 {
            return Err(RiskError::Overflow);
        }
        if self.fee_tier2_threshold > 0
            && self.fee_tier3_threshold > 0
            && self.fee_tier3_threshold <= self.fee_tier2_threshold
        {
            return Err(RiskError::Overflow);
        }
        // Fee split must sum to 10_000
        if self.fee_split_lp_bps > 0
            || self.fee_split_protocol_bps > 0
            || self.fee_split_creator_bps > 0
        {
            let total = self
                .fee_split_lp_bps
                .saturating_add(self.fee_split_protocol_bps)
                .saturating_add(self.fee_split_creator_bps);
            if total != 10_000 {
                return Err(RiskError::Overflow);
            }
        }
        // Insurance floor (spec §1.4: 0 <= I_floor <= MAX_ORACLE_PRICE * MAX_POSITION_ABS)
        // No separate MAX_VAULT_TVL constant in this fork; nonzero values are permissive.
        // Structural validity is enforced: insurance_floor must fit within U128.
        Ok(())
    }

    /// Effective emergency liquidation margin (bps).
    /// 0 = auto mode → maintenance_margin_bps / 2.
    #[inline]
    pub fn effective_emergency_margin_bps(&self) -> u64 {
        if self.emergency_liquidation_margin_bps > 0 {
            self.emergency_liquidation_margin_bps
        } else {
            self.maintenance_margin_bps / 2
        }
    }
}

/// Main risk engine state - fixed slab with bitmap
#[repr(C)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RiskEngine {
    /// Total vault balance (all deposited funds)
    pub vault: U128,

    /// Insurance fund
    pub insurance_fund: InsuranceFund,

    /// Risk parameters
    pub params: RiskParams,

    /// Current slot (for warmup calculations)
    pub current_slot: u64,

    /// Global funding index (quote per 1 base, scaled by 1e6)
    pub funding_index_qpb_e6: I128,

    /// Last slot when funding was accrued
    pub last_funding_slot: u64,

    /// Funding rate (bps per slot) in effect starting at last_funding_slot.
    /// This is the rate used for the interval [last_funding_slot, next_accrual).
    /// Anti-retroactivity: state changes at slot t can only affect funding for slots >= t.
    pub funding_rate_bps_per_slot_last: i64,

    /// Last slot used in accrue_market_to
    pub last_market_slot: u64,

    /// Funding price sample for anti-retroactivity
    pub funding_price_sample_last: u64,

    /// Materialized account count (spec §2.2)
    pub materialized_account_count: u64,

    /// Last oracle price seen (for delta_p computation in accrue_market_to)
    pub last_oracle_price: u64,

    // ========================================
    // Premium Funding State (PERC-121)
    // ========================================
    /// Current mark price (EMA-smoothed), scaled by 1e6.
    /// Updated by wrapper after oracle/mark price computation.
    pub mark_price_e6: u64,

    /// Whether funding rate is frozen (emergency freeze by admin).
    /// When true: accrue_funding still runs using the frozen rate snapshot,
    /// but no new rate is computed or applied.
    pub funding_frozen: bool,

    /// Snapshot of funding rate at freeze time (used while frozen).
    pub funding_frozen_rate_snapshot: i64,

    // ========================================
    // Keeper Crank Tracking
    // ========================================
    /// Last slot when keeper crank was executed
    pub last_crank_slot: u64,

    /// Maximum allowed staleness before crank is required (in slots)
    pub max_crank_staleness_slots: u64,

    // ========================================
    // Open Interest Tracking (O(1))
    // ========================================
    /// Total open interest = sum of abs(position_size) across all accounts
    /// This measures total risk exposure in the system.
    pub total_open_interest: U128,

    /// Long open interest = sum of position_size for all long positions (pos > 0)
    /// Maintained incrementally for O(1) OI skew computation (PERC-298).
    pub long_oi: U128,

    /// Short open interest = sum of abs(position_size) for all short positions (pos < 0)
    /// Maintained incrementally for O(1) OI skew computation (PERC-298).
    pub short_oi: U128,

    // ========================================
    // O(1) Aggregates (spec §2.2, §4)
    // ========================================
    /// Sum of all account capital: C_tot = Σ C_i
    /// Maintained incrementally via set_capital() helper.
    pub c_tot: U128,

    /// Sum of all positive PnL: PNL_pos_tot = Σ max(PNL_i, 0)
    /// Maintained incrementally via set_pnl() helper.
    pub pnl_pos_tot: U128,

    /// Sum of all matured (released) positive PnL: PNL_matured_pos_tot = Σ max(PNL_i - R_i, 0)
    /// "Matured" = positive PnL that is past warmup (released from reserve).
    /// Used as haircut denominator per spec §3.2 (v11.21+).
    /// Maintained by set_pnl, set_reserved_pnl, and consume_released_pnl helpers.
    pub pnl_matured_pos_tot: u128,

    // ========================================
    // Crank Cursors (bounded scan support)
    // ========================================
    /// Cursor for liquidation scan (wraps around MAX_ACCOUNTS)
    pub liq_cursor: u16,

    /// Cursor for garbage collection scan (wraps around MAX_ACCOUNTS)
    pub gc_cursor: u16,

    /// Slot when the current full sweep started (step 0 was executed)
    pub last_full_sweep_start_slot: u64,

    /// Slot when the last full sweep completed
    pub last_full_sweep_completed_slot: u64,

    /// Cursor: index where the next crank will start scanning
    pub crank_cursor: u16,

    /// Index where the current sweep started (for completion detection)
    pub sweep_start_idx: u16,

    // ========================================
    // Lifetime Counters (telemetry)
    // ========================================
    /// Total number of liquidations performed (lifetime)
    pub lifetime_liquidations: u64,

    /// Total number of force-realize closes performed (lifetime)
    pub lifetime_force_realize_closes: u64,

    // ========================================
    // ADL Side State (spec §2.4, T3: SideMode enum)
    // ========================================
    /// ADL A-coefficient for long side (socialized loss multiplier)
    pub adl_mult_long: u128,

    /// ADL A-coefficient for short side (socialized loss multiplier)
    pub adl_mult_short: u128,

    /// ADL K-coefficient for long side (epoch accumulator)
    pub adl_coeff_long: i128,

    /// ADL K-coefficient for short side (epoch accumulator)
    pub adl_coeff_short: i128,

    /// ADL epoch counter for long side
    pub adl_epoch_long: u64,

    /// ADL epoch counter for short side
    pub adl_epoch_short: u64,

    /// K value at start of current ADL epoch (long side)
    pub adl_epoch_start_k_long: i128,

    /// K value at start of current ADL epoch (short side)
    pub adl_epoch_start_k_short: i128,

    /// Effective OI for long side (in Q units, updated by enqueue_adl)
    pub oi_eff_long_q: u128,

    /// Effective OI for short side (in Q units, updated by enqueue_adl)
    pub oi_eff_short_q: u128,

    /// Side mode for long side (Normal / DrainOnly / ResetPending)
    pub side_mode_long: SideMode,

    /// Side mode for short side (Normal / DrainOnly / ResetPending)
    pub side_mode_short: SideMode,

    /// Count of stored (open) positions on long side
    pub stored_pos_count_long: u64,

    /// Count of stored (open) positions on short side
    pub stored_pos_count_short: u64,

    /// Count of stale accounts on long side (pending re-settlement)
    pub stale_account_count_long: u64,

    /// Count of stale accounts on short side (pending re-settlement)
    pub stale_account_count_short: u64,

    /// Dynamic phantom dust bound for long side (spec §4.6)
    pub phantom_dust_bound_long_q: u128,

    /// Dynamic phantom dust bound for short side (spec §4.6)
    pub phantom_dust_bound_short_q: u128,

    // ========================================
    // LP Aggregates (O(1) maintained for funding/threshold)
    // ========================================
    /// Net LP position: sum of position_size across all LP accounts
    /// Updated incrementally in execute_trade and close paths
    pub net_lp_pos: I128,

    /// Sum of abs(position_size) across all LP accounts
    /// Updated incrementally in execute_trade and close paths
    pub lp_sum_abs: U128,

    /// Max abs(position_size) across all LP accounts (monotone upper bound)
    /// Only increases; reset via bounded sweep at sweep completion
    pub lp_max_abs: U128,

    /// In-progress max abs for current sweep (reset at sweep start, committed at completion)
    pub lp_max_abs_sweep: U128,

    // ========================================
    // Volatility-Adjusted OI Cap (PERC-299)
    // ========================================
    /// When true, OI cap is halved due to circuit breaker trigger.
    /// Cleared when oracle is stable for EMERGENCY_RECOVERY_SLOTS consecutive slots.
    pub emergency_oi_mode: u8, // bool stored as u8 for repr(C) alignment

    /// Slot when emergency OI mode was activated (0 = never)
    pub emergency_start_slot: u64,

    /// Last slot when the circuit breaker fired (used for recovery tracking)
    pub last_breaker_slot: u64,

    // ========================================
    // Trade TWAP (PERC-118: Mark Price Blend)
    // ========================================
    /// EMA of trade execution prices (e6), updated on each fill.
    /// Used as the "impact mid price" component of the blended mark.
    pub trade_twap_e6: u64,

    /// Last slot when trade_twap_e6 was updated.
    pub twap_last_slot: u64,

    // ========================================
    // Slab Management
    // ========================================
    /// Occupancy bitmap (4096 bits = 64 u64 words)
    pub used: [u64; BITMAP_WORDS],

    /// Number of used accounts (O(1) counter, fixes H2: fee bypass TOCTOU)
    pub num_used_accounts: u16,

    /// Next account ID to assign (monotonically increasing, never recycled)
    pub next_account_id: u64,

    /// Freelist head (u16::MAX = none)
    pub free_head: u16,

    /// Freelist next pointers
    pub next_free: [u16; MAX_ACCOUNTS],

    /// Account slab (4096 accounts)
    pub accounts: [Account; MAX_ACCOUNTS],
}

// ============================================================================
// Error Types
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RiskError {
    /// Insufficient balance for operation
    InsufficientBalance,

    /// Account would become undercollateralized
    Undercollateralized,

    /// Unauthorized operation
    Unauthorized,

    /// Invalid matching engine
    InvalidMatchingEngine,

    /// PNL not yet warmed up
    PnlNotWarmedUp,

    /// Arithmetic overflow
    Overflow,

    /// Account not found
    AccountNotFound,

    /// Account is not an LP account
    NotAnLPAccount,

    /// Position size mismatch
    PositionSizeMismatch,

    /// Account kind mismatch
    AccountKindMismatch,

    /// Entry price must be positive when opening a position
    InvalidEntryPrice,

    /// Side is in DrainOnly or ResetPending mode — new positions blocked (spec §2.4)
    SideBlocked,

    /// Internal state is corrupt (invariant violation detected)
    CorruptState,
}

pub type Result<T> = core::result::Result<T, RiskError>;

/// Liquidation policy for keeper-supplied candidates (spec §10.6)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiquidationPolicy {
    /// Close the full effective position
    FullClose,
    /// Close exactly q_close_q units (validated against current state)
    ExactPartial(u128),
}

/// Outcome of a keeper crank operation
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrankOutcome {
    /// Whether the crank successfully advanced last_crank_slot
    pub advanced: bool,
    /// Slots forgiven for caller's maintenance (50% discount via time forgiveness)
    pub slots_forgiven: u64,
    /// Whether caller's maintenance fee settle succeeded (false if undercollateralized)
    pub caller_settle_ok: bool,
    /// Whether force-realize mode is active (insurance at/below threshold)
    pub force_realize_needed: bool,
    /// Whether panic_settle_all should be called (system in stress)
    pub panic_needed: bool,
    /// Number of accounts liquidated during this crank
    pub num_liquidations: u32,
    /// Number of liquidation errors (triggers risk_reduction_only)
    pub num_liq_errors: u16,
    /// Number of dust accounts garbage collected during this crank
    pub num_gc_closed: u32,
    /// Number of positions force-closed during this crank (when force_realize_needed)
    pub force_realize_closed: u16,
    /// Number of force-realize errors during this crank
    pub force_realize_errors: u16,
    /// Index where this crank stopped (next crank continues from here)
    pub last_cursor: u16,
    /// Whether this crank completed a full sweep of all accounts
    pub sweep_complete: bool,
    /// Number of times accrue_market_to failed during this crank (ADL coefficients went stale).
    /// Under normal conditions this is always 0. Non-zero values indicate extreme adl_mult +
    /// large price swing combinations that caused overflow inside accrue_market_to. No funds are
    /// lost — the ADL coefficients are simply not updated for this crank cycle — but observability
    /// of silent failures was previously zero. GH#1931.
    pub adl_accrue_failures: u8,
}

// ============================================================================
// Math Helpers (Saturating Arithmetic for Safety)
// ============================================================================

/// Determine which side a signed position is on.
/// Returns `None` for flat (zero) positions.
#[allow(dead_code)]
fn side_of_i128(v: i128) -> Option<Side> {
    if v == 0 {
        None
    } else if v > 0 {
        Some(Side::Long)
    } else {
        Some(Side::Short)
    }
}

/// Return the opposite side.
#[allow(dead_code)]
fn opposite_side(s: Side) -> Side {
    match s {
        Side::Long => Side::Short,
        Side::Short => Side::Long,
    }
}

/// Clamp i128 to max(v, 0) as u128 (upstream: i128_clamp_pos)
#[inline]
fn i128_clamp_pos(v: i128) -> u128 {
    if v > 0 {
        v as u128
    } else {
        0u128
    }
}

#[inline]
fn add_u128(a: u128, b: u128) -> u128 {
    a.saturating_add(b)
}

#[inline]
fn sub_u128(a: u128, b: u128) -> u128 {
    a.saturating_sub(b)
}

#[inline]
fn mul_u128(a: u128, b: u128) -> u128 {
    a.saturating_mul(b)
}

#[inline]
fn clamp_pos_i128(val: i128) -> u128 {
    if val > 0 {
        val as u128
    } else {
        0
    }
}

#[allow(dead_code)]
#[inline]
fn clamp_neg_i128(val: i128) -> u128 {
    if val < 0 {
        neg_i128_to_u128(val)
    } else {
        0
    }
}

/// Saturating absolute value for i128 (handles i128::MIN without overflow)
#[inline]
fn saturating_abs_i128(val: i128) -> i128 {
    if val == i128::MIN {
        i128::MAX
    } else {
        val.abs()
    }
}

/// Safely convert negative i128 to u128 (handles i128::MIN without overflow)
///
/// For i128::MIN, -i128::MIN would overflow because i128::MAX + 1 cannot be represented.
/// We handle this by returning (i128::MAX as u128) + 1 = 170141183460469231731687303715884105728.
#[inline]
fn neg_i128_to_u128(val: i128) -> u128 {
    debug_assert!(val < 0, "neg_i128_to_u128 called with non-negative value");
    if val == i128::MIN {
        (i128::MAX as u128) + 1
    } else {
        (-val) as u128
    }
}

/// Safely convert u128 to i128 with clamping (handles values > i128::MAX)
///
/// If x > i128::MAX, the cast would wrap to a negative value.
/// We clamp to i128::MAX instead to preserve correctness of margin checks.
#[inline]
fn u128_to_i128_clamped(x: u128) -> i128 {
    if x > i128::MAX as u128 {
        i128::MAX
    } else {
        x as i128
    }
}

// ============================================================================
// Matching Engine Trait
// ============================================================================

/// Result of a successful trade execution from the matching engine
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TradeExecution {
    /// Actual execution price (may differ from oracle/requested price)
    pub price: u64,
    /// Actual executed size (may be partial fill)
    pub size: i128,
}

/// Trait for pluggable matching engines
///
/// Implementers can provide custom order matching logic via CPI.
/// The matching engine is responsible for validating and executing trades
/// according to its own rules (CLOB, AMM, RFQ, etc).
pub trait MatchingEngine {
    /// Execute a trade between LP and user
    ///
    /// # Arguments
    /// * `lp_program` - The LP's matching engine program ID
    /// * `lp_context` - The LP's matching engine context account
    /// * `lp_account_id` - Unique ID of the LP account (never recycled)
    /// * `oracle_price` - Current oracle price for reference
    /// * `size` - Requested position size (positive = long, negative = short)
    ///
    /// # Returns
    /// * `Ok(TradeExecution)` with actual executed price and size
    /// * `Err(RiskError)` if the trade is rejected
    ///
    /// # Safety
    /// The matching engine MUST verify user authorization before approving trades.
    /// The risk engine will check solvency after the trade executes.
    fn execute_match(
        &self,
        lp_program: &[u8; 32],
        lp_context: &[u8; 32],
        lp_account_id: u64,
        oracle_price: u64,
        size: i128,
    ) -> Result<TradeExecution>;
}

/// No-op matching engine (for testing)
/// Returns the requested price and size as-is
pub struct NoOpMatcher;

impl MatchingEngine for NoOpMatcher {
    fn execute_match(
        &self,
        _lp_program: &[u8; 32],
        _lp_context: &[u8; 32],
        _lp_account_id: u64,
        oracle_price: u64,
        size: i128,
    ) -> Result<TradeExecution> {
        // Return requested price/size unchanged (no actual matching logic)
        Ok(TradeExecution {
            price: oracle_price,
            size,
        })
    }
}

// ============================================================================
// Core Implementation
// ============================================================================

impl RiskEngine {
    /// Create a new risk engine (stack-allocates the full struct - avoid in BPF!)
    ///
    /// WARNING: This allocates ~6MB on the stack at MAX_ACCOUNTS=4096.
    /// For Solana BPF programs, use `init_in_place` instead.
    ///
    /// # Panics
    /// Panics if `params` fails validation (see `RiskParams::validate`).
    ///
    /// Excluded from SBF builds to prevent linker stack-overflow warnings
    /// (128 KiB–2 MiB frame vs. the 4 KiB SBF limit). On-chain code must
    /// use `engine_mut()` + `init_in_place()` instead.
    #[cfg(not(target_os = "solana"))]
    pub fn new(params: RiskParams) -> Self {
        params.validate().expect("invalid RiskParams");
        let mut engine = Self {
            vault: U128::ZERO,
            insurance_fund: InsuranceFund {
                balance: U128::ZERO,
                fee_revenue: U128::ZERO,
                balance_incentive_reserve: 0,
                _rebate_pad: [0; 8],
                isolated_balance: U128::ZERO,
                insurance_isolation_bps: 0,
                _isolation_padding: [0u8; 14],
            },
            params,
            current_slot: 0,
            funding_index_qpb_e6: I128::ZERO,
            last_funding_slot: 0,
            funding_rate_bps_per_slot_last: 0,
            last_market_slot: 0,
            funding_price_sample_last: 0,
            materialized_account_count: 0,
            last_oracle_price: 0,
            mark_price_e6: 0,
            funding_frozen: false,
            funding_frozen_rate_snapshot: 0,
            last_crank_slot: 0,
            max_crank_staleness_slots: params.max_crank_staleness_slots,
            total_open_interest: U128::ZERO,
            long_oi: U128::ZERO,
            short_oi: U128::ZERO,
            c_tot: U128::ZERO,
            pnl_pos_tot: U128::ZERO,
            pnl_matured_pos_tot: 0u128,
            liq_cursor: 0,
            gc_cursor: 0,
            last_full_sweep_start_slot: 0,
            last_full_sweep_completed_slot: 0,
            crank_cursor: 0,
            sweep_start_idx: 0,
            lifetime_liquidations: 0,
            lifetime_force_realize_closes: 0,
            adl_mult_long: 0,
            adl_mult_short: 0,
            adl_coeff_long: 0,
            adl_coeff_short: 0,
            adl_epoch_long: 0,
            adl_epoch_short: 0,
            adl_epoch_start_k_long: 0,
            adl_epoch_start_k_short: 0,
            oi_eff_long_q: 0,
            oi_eff_short_q: 0,
            side_mode_long: SideMode::Normal,
            side_mode_short: SideMode::Normal,
            stored_pos_count_long: 0,
            stored_pos_count_short: 0,
            stale_account_count_long: 0,
            stale_account_count_short: 0,
            phantom_dust_bound_long_q: 0,
            phantom_dust_bound_short_q: 0,
            net_lp_pos: I128::ZERO,
            lp_sum_abs: U128::ZERO,
            lp_max_abs: U128::ZERO,
            lp_max_abs_sweep: U128::ZERO,
            emergency_oi_mode: 0,
            emergency_start_slot: 0,
            last_breaker_slot: 0,
            trade_twap_e6: 0,
            twap_last_slot: 0,
            used: [0; BITMAP_WORDS],
            num_used_accounts: 0,
            next_account_id: 0,
            free_head: 0,
            next_free: [0; MAX_ACCOUNTS],
            accounts: [empty_account(); MAX_ACCOUNTS],
        };

        // Initialize freelist: 0 -> 1 -> 2 -> ... -> 4095 -> NONE
        for i in 0..MAX_ACCOUNTS - 1 {
            engine.next_free[i] = (i + 1) as u16;
        }
        engine.next_free[MAX_ACCOUNTS - 1] = u16::MAX; // Sentinel

        engine
    }

    // ========================================
    // ADL Side State Helpers (T3: SideMode enum, spec §2.4)
    // ========================================

    /// Get the SideMode for a given side.
    #[inline]
    #[allow(dead_code)] // used by T8 ADL core (PERC-8273)
    fn get_side_mode(&self, s: Side) -> SideMode {
        match s {
            Side::Long => self.side_mode_long,
            Side::Short => self.side_mode_short,
        }
    }

    /// Set the SideMode for a given side.
    #[inline]
    #[allow(dead_code)] // used by T8 ADL core (PERC-8273)
    fn set_side_mode(&mut self, s: Side, m: SideMode) {
        match s {
            Side::Long => self.side_mode_long = m,
            Side::Short => self.side_mode_short = m,
        }
    }

    /// Get the effective OI (in Q units) for a given side.
    #[inline]
    #[allow(dead_code)] // used by T8 ADL core (PERC-8273)
    fn get_oi_eff(&self, s: Side) -> u128 {
        match s {
            Side::Long => self.oi_eff_long_q,
            Side::Short => self.oi_eff_short_q,
        }
    }

    /// Set the effective OI (in Q units) for a given side.
    #[inline]
    #[allow(dead_code)] // used by T8 ADL core (PERC-8273)
    fn set_oi_eff(&mut self, s: Side, v: u128) {
        match s {
            Side::Long => self.oi_eff_long_q = v,
            Side::Short => self.oi_eff_short_q = v,
        }
    }

    /// Check whether a new position open is permitted on the given side.
    /// Returns `Err(SideBlocked)` when side_mode is DrainOnly or ResetPending.
    #[inline]
    pub fn check_side_open_permitted(&self, s: Side) -> Result<()> {
        match self.get_side_mode(s) {
            SideMode::Normal => Ok(()),
            SideMode::DrainOnly | SideMode::ResetPending => Err(RiskError::SideBlocked),
        }
    }

    // ========================================
    // ADL per-account accessor helpers (T5: PERC-8270)
    // ========================================

    #[allow(dead_code)]
    fn get_a_side(&self, s: Side) -> u128 {
        match s {
            Side::Long => self.adl_mult_long,
            Side::Short => self.adl_mult_short,
        }
    }

    #[allow(dead_code)]
    fn get_k_side(&self, s: Side) -> i128 {
        match s {
            Side::Long => self.adl_coeff_long,
            Side::Short => self.adl_coeff_short,
        }
    }

    #[allow(dead_code)]
    fn get_epoch_side(&self, s: Side) -> u64 {
        match s {
            Side::Long => self.adl_epoch_long,
            Side::Short => self.adl_epoch_short,
        }
    }

    #[allow(dead_code)]
    fn get_k_epoch_start(&self, s: Side) -> i128 {
        match s {
            Side::Long => self.adl_epoch_start_k_long,
            Side::Short => self.adl_epoch_start_k_short,
        }
    }

    #[allow(dead_code)]
    fn get_stale_count(&self, s: Side) -> u64 {
        match s {
            Side::Long => self.stale_account_count_long,
            Side::Short => self.stale_account_count_short,
        }
    }

    #[allow(dead_code)]
    fn set_stale_count(&mut self, s: Side, v: u64) {
        match s {
            Side::Long => self.stale_account_count_long = v,
            Side::Short => self.stale_account_count_short = v,
        }
    }

    #[allow(dead_code)]
    fn inc_phantom_dust_bound(&mut self, s: Side) {
        match s {
            Side::Long => {
                self.phantom_dust_bound_long_q = self
                    .phantom_dust_bound_long_q
                    .checked_add(1u128)
                    .expect("phantom dust long overflow")
            }
            Side::Short => {
                self.phantom_dust_bound_short_q = self
                    .phantom_dust_bound_short_q
                    .checked_add(1u128)
                    .expect("phantom dust short overflow")
            }
        }
    }

    /// checked_u128_mul_i128: multiply u128 * i128, return i128 or Overflow
    #[allow(dead_code)]
    fn checked_u128_mul_i128(a: u128, b: i128) -> Result<i128> {
        if a == 0 || b == 0 {
            return Ok(0i128);
        }
        let negative = b < 0;
        let abs_b = if b == i128::MIN {
            return Err(RiskError::Overflow);
        } else {
            b.unsigned_abs()
        };
        let product = U256::from_u128(a)
            .checked_mul(U256::from_u128(abs_b))
            .ok_or(RiskError::Overflow)?;
        match product.try_into_u128() {
            Some(v) if v <= i128::MAX as u128 => {
                if negative {
                    Ok(-(v as i128))
                } else {
                    Ok(v as i128)
                }
            }
            _ => Err(RiskError::Overflow),
        }
    }

    // ========================================
    // Position basis helpers (T5: PERC-8270)
    // ========================================

    #[allow(dead_code)]
    fn set_position_basis_q(&mut self, idx: usize, new_basis: i128) {
        let old = self.accounts[idx].position_basis_q;
        let old_side = side_of_i128(old);
        let new_side = side_of_i128(new_basis);
        if let Some(s) = old_side {
            match s {
                Side::Long => {
                    self.stored_pos_count_long = self
                        .stored_pos_count_long
                        .checked_sub(1)
                        .expect("set_position_basis_q long underflow")
                }
                Side::Short => {
                    self.stored_pos_count_short = self
                        .stored_pos_count_short
                        .checked_sub(1)
                        .expect("set_position_basis_q short underflow")
                }
            }
        }
        if let Some(s) = new_side {
            match s {
                Side::Long => {
                    self.stored_pos_count_long = self
                        .stored_pos_count_long
                        .checked_add(1)
                        .expect("set_position_basis_q long overflow")
                }
                Side::Short => {
                    self.stored_pos_count_short = self
                        .stored_pos_count_short
                        .checked_add(1)
                        .expect("set_position_basis_q short overflow")
                }
            }
        }
        self.accounts[idx].position_basis_q = new_basis;
    }

    /// effective_pos_q (spec §5.2): compute effective position quantity for account idx.
    #[allow(dead_code)]
    pub fn effective_pos_q(&self, idx: usize) -> i128 {
        let basis = self.accounts[idx].position_basis_q;
        if basis == 0 {
            return 0i128;
        }
        let side = side_of_i128(basis).unwrap();
        let epoch_snap = self.accounts[idx].adl_epoch_snap;
        let epoch_side = self.get_epoch_side(side);
        if epoch_snap != epoch_side {
            return 0i128;
        }
        let a_side = self.get_a_side(side);
        let a_basis = self.accounts[idx].adl_a_basis;
        if a_basis == 0 {
            return 0i128;
        }
        let abs_basis = basis.unsigned_abs();
        let effective_abs = mul_div_floor_u128(abs_basis, a_side, a_basis);
        if basis < 0 {
            if effective_abs == 0 {
                0i128
            } else {
                assert!(
                    effective_abs <= i128::MAX as u128,
                    "effective_pos_q: overflow"
                );
                -(effective_abs as i128)
            }
        } else {
            assert!(
                effective_abs <= i128::MAX as u128,
                "effective_pos_q: overflow"
            );
            effective_abs as i128
        }
    }

    /// Schedule deferred ADL epoch resets for the end of this instruction.
    ///
    /// Sets `pending_reset_long/short` flags in `ctx` when the side is in
    /// ResetPending state, so `finalize_end_of_instruction_resets` can commit.
    fn schedule_end_of_instruction_resets(&self, ctx: &mut InstructionContext) -> Result<()> {
        if self.side_mode_long == SideMode::ResetPending {
            ctx.pending_reset_long = true;
        }
        if self.side_mode_short == SideMode::ResetPending {
            ctx.pending_reset_short = true;
        }
        Ok(())
    }

    /// Finalize deferred ADL epoch resets that were scheduled at end-of-instruction.
    ///
    /// If a side was in ResetPending and OI has reached zero, transition it back
    /// to Normal and reset A/K coefficients.
    fn finalize_end_of_instruction_resets(&mut self, ctx: &InstructionContext) {
        if ctx.pending_reset_long
            && self.side_mode_long == SideMode::ResetPending
            && self.oi_eff_long_q == 0
        {
            self.side_mode_long = SideMode::Normal;
            self.adl_mult_long = 0;
            self.adl_coeff_long = 0;
            self.adl_epoch_start_k_long = 0;
        }
        if ctx.pending_reset_short
            && self.side_mode_short == SideMode::ResetPending
            && self.oi_eff_short_q == 0
        {
            self.side_mode_short = SideMode::Normal;
            self.adl_mult_short = 0;
            self.adl_coeff_short = 0;
            self.adl_epoch_start_k_short = 0;
        }
    }

    /// Public entry-point for the end-of-instruction lifecycle (spec §5.7-5.8).
    ///
    /// Runs `schedule_end_of_instruction_resets` then
    /// `finalize_end_of_instruction_resets` in the canonical order.
    /// Callers must invoke this before returning from any instruction that
    /// may call `enqueue_adl`.
    pub fn run_end_of_instruction_lifecycle(
        &mut self,
        ctx: &mut InstructionContext,
        funding_rate: i64,
    ) -> Result<()> {
        Self::validate_funding_rate(funding_rate)?;
        self.schedule_end_of_instruction_resets(ctx)?;
        self.finalize_end_of_instruction_resets(ctx);
        self.recompute_r_last_from_final_state(funding_rate)?;
        Ok(())
    }

    // ========================================
    // PERC-299: Volatility-Adjusted OI Cap
    // ========================================

    /// Returns true if emergency OI mode is active.
    #[inline]
    pub fn is_emergency_oi_mode(&self) -> bool {
        self.emergency_oi_mode != 0
    }

    /// Activate emergency OI mode (halves effective OI cap).
    /// Called when circuit breaker fires.
    #[inline]
    pub fn enter_emergency_oi_mode(&mut self, current_slot: u64) {
        if self.emergency_oi_mode == 0 {
            self.emergency_start_slot = current_slot;
        }
        self.emergency_oi_mode = 1;
        self.last_breaker_slot = current_slot;
    }

    /// Check if oracle has been stable long enough to exit emergency mode.
    /// Call this on every crank/oracle update where the breaker did NOT fire.
    #[inline]
    pub fn check_emergency_recovery(&mut self, current_slot: u64) {
        if self.emergency_oi_mode != 0
            && current_slot
                >= self
                    .last_breaker_slot
                    .saturating_add(EMERGENCY_RECOVERY_SLOTS)
        {
            self.emergency_oi_mode = 0;
            self.emergency_start_slot = 0;
            self.last_breaker_slot = 0;
        }
    }

    /// Initialize a RiskEngine in place (zero-copy friendly).
    ///
    /// PREREQUISITE: The memory backing `self` MUST be zeroed before calling.
    /// This method only sets non-zero fields to avoid touching the entire ~6MB struct.
    ///
    /// This is the correct way to initialize RiskEngine in Solana BPF programs
    /// where stack space is limited to 4KB.
    pub fn init_in_place(&mut self, params: RiskParams) -> Result<()> {
        params.validate()?;

        // Set params (non-zero field)
        self.params = params;
        self.max_crank_staleness_slots = params.max_crank_staleness_slots;

        // Initialize freelist: 0 -> 1 -> 2 -> ... -> MAX_ACCOUNTS-1 -> NONE
        // All other fields are zero which is correct for:
        // - vault, insurance_fund, current_slot, funding_index, etc. = 0
        // - used bitmap = all zeros (no accounts in use)
        // - accounts = all zeros (equivalent to empty_account())
        // - free_head = 0 (first free slot is 0)
        for i in 0..MAX_ACCOUNTS - 1 {
            self.next_free[i] = (i + 1) as u16;
        }
        self.next_free[MAX_ACCOUNTS - 1] = u16::MAX; // Sentinel
        Ok(())
    }

    // ========================================
    // Bitmap Helpers
    // ========================================

    pub fn is_used(&self, idx: usize) -> bool {
        if idx >= MAX_ACCOUNTS {
            return false;
        }
        let w = idx >> 6;
        let b = idx & 63;
        ((self.used[w] >> b) & 1) == 1
    }

    fn set_used(&mut self, idx: usize) {
        let w = idx >> 6;
        let b = idx & 63;
        self.used[w] |= 1u64 << b;
    }

    fn clear_used(&mut self, idx: usize) {
        let w = idx >> 6;
        let b = idx & 63;
        self.used[w] &= !(1u64 << b);
    }

    #[allow(dead_code)]
    fn for_each_used_mut<F: FnMut(usize, &mut Account)>(&mut self, mut f: F) {
        for (block, word) in self.used.iter().copied().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                let idx = block * 64 + bit;
                w &= w - 1; // Clear lowest bit
                if idx >= MAX_ACCOUNTS {
                    continue; // Guard against stray high bits in bitmap
                }
                f(idx, &mut self.accounts[idx]);
            }
        }
    }

    fn for_each_used<F: FnMut(usize, &Account)>(&self, mut f: F) {
        for (block, word) in self.used.iter().copied().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                let idx = block * 64 + bit;
                w &= w - 1; // Clear lowest bit
                if idx >= MAX_ACCOUNTS {
                    continue; // Guard against stray high bits in bitmap
                }
                f(idx, &self.accounts[idx]);
            }
        }
    }

    // ========================================
    // O(1) Aggregate Helpers (spec §4)
    // ========================================

    /// set_pnl (spec §4.4): update PNL_i and maintain pnl_pos_tot + pnl_matured_pos_tot.
    ///
    /// Reserve-first semantics:
    ///   - If PnL increases: new profits go to reserve first (not yet matured).
    ///   - If PnL decreases: losses drain the released portion first, then reserve.
    ///
    /// All code paths that modify PnL MUST call this helper.
    #[inline]
    pub fn set_pnl(&mut self, idx: usize, new_pnl: i128) {
        let old_pnl = self.accounts[idx].pnl.get();
        let old_pos = if old_pnl > 0 { old_pnl as u128 } else { 0u128 };
        let old_r = self.accounts[idx].reserved_pnl;
        // released = max(PNL_i, 0) - R_i  (matured portion)
        let old_rel = old_pos.saturating_sub(old_r);

        let new_pos = if new_pnl > 0 { new_pnl as u128 } else { 0u128 };

        // Compute new reserve: reserve-first semantics.
        let new_r = if new_pos > old_pos {
            // Increase: new profits go to reserve (no change to released).
            let gain = new_pos - old_pos;
            old_r.saturating_add(gain).min(new_pos)
        } else {
            // Decrease or flat: losses drain released first, then reserve.
            let loss = old_pos.saturating_sub(new_pos);
            // Released portion absorbs loss first.
            let released_loss = loss.min(old_rel);
            let remaining_loss = loss.saturating_sub(released_loss);
            old_r.saturating_sub(remaining_loss).min(new_pos)
        };
        let new_rel = new_pos.saturating_sub(new_r);

        // Update pnl_pos_tot
        if new_pos > old_pos {
            let delta = new_pos - old_pos;
            self.pnl_pos_tot = U128::new(self.pnl_pos_tot.get().saturating_add(delta));
        } else if old_pos > new_pos {
            let delta = old_pos - new_pos;
            self.pnl_pos_tot = U128::new(self.pnl_pos_tot.get().saturating_sub(delta));
        }

        // Update pnl_matured_pos_tot
        if new_rel > old_rel {
            let delta = new_rel - old_rel;
            self.pnl_matured_pos_tot = self.pnl_matured_pos_tot.saturating_add(delta);
        } else if old_rel > new_rel {
            let delta = old_rel - new_rel;
            self.pnl_matured_pos_tot = self.pnl_matured_pos_tot.saturating_sub(delta);
        }

        // Write fields
        self.accounts[idx].pnl = I128::new(new_pnl);
        self.accounts[idx].reserved_pnl = new_r;
    }

    /// set_reserved_pnl (spec §4.3): update R_i and maintain pnl_matured_pos_tot.
    ///
    /// Used when warmup slope triggers partial release of reserves (R decreases → matured increases).
    /// Asserts: new_r <= max(PNL_i, 0) (R cannot exceed positive PnL).
    #[inline]
    pub fn set_reserved_pnl(&mut self, idx: usize, new_r: u128) {
        let pos = {
            let p = self.accounts[idx].pnl.get();
            if p > 0 {
                p as u128
            } else {
                0u128
            }
        };
        debug_assert!(
            new_r <= pos,
            "set_reserved_pnl: new_r ({}) > max(PNL_i, 0) ({})",
            new_r,
            pos
        );
        let new_r = new_r.min(pos); // clamp defensively

        let old_r = self.accounts[idx].reserved_pnl;
        let old_rel = pos.saturating_sub(old_r);
        let new_rel = pos.saturating_sub(new_r);

        // Update pnl_matured_pos_tot
        if new_rel > old_rel {
            let delta = new_rel - old_rel;
            self.pnl_matured_pos_tot = self.pnl_matured_pos_tot.saturating_add(delta);
        } else if old_rel > new_rel {
            let delta = old_rel - new_rel;
            self.pnl_matured_pos_tot = self.pnl_matured_pos_tot.saturating_sub(delta);
        }

        self.accounts[idx].reserved_pnl = new_r;
    }

    /// consume_released_pnl (spec §4.4.1): remove `x` matured released positive PnL from
    /// account without touching R_i. Used for profit-to-capital conversions.
    ///
    /// Caller must ensure x <= (max(PNL_i, 0) - R_i).
    #[inline]
    pub fn consume_released_pnl(&mut self, idx: usize, x: u128) {
        debug_assert!(x > 0, "consume_released_pnl: x must be > 0");
        let old_pos = {
            let p = self.accounts[idx].pnl.get();
            if p > 0 {
                p as u128
            } else {
                0u128
            }
        };
        let old_r = self.accounts[idx].reserved_pnl;
        let old_rel = old_pos.saturating_sub(old_r);
        debug_assert!(x <= old_rel, "consume_released_pnl: x > released portion");
        let x = x.min(old_rel); // clamp defensively

        // Update pnl_pos_tot
        self.pnl_pos_tot = U128::new(self.pnl_pos_tot.get().saturating_sub(x));
        // Update pnl_matured_pos_tot
        self.pnl_matured_pos_tot = self.pnl_matured_pos_tot.saturating_sub(x);

        // Reduce PNL_i by x (R_i unchanged)
        let x_i128 = x.min(i128::MAX as u128) as i128;
        let new_pnl = self.accounts[idx].pnl.get().saturating_sub(x_i128);
        self.accounts[idx].pnl = I128::new(new_pnl);
        // R_i stays unchanged; new released = (new_pos - old_r) which is now (old_rel - x)
    }

    /// Helper: set account capital and maintain c_tot aggregate (spec §4.1).
    #[inline]
    pub fn set_capital(&mut self, idx: usize, new_capital: u128) {
        let old = self.accounts[idx].capital.get();
        if new_capital >= old {
            self.c_tot = U128::new(self.c_tot.get().saturating_add(new_capital - old));
        } else {
            self.c_tot = U128::new(self.c_tot.get().saturating_sub(old - new_capital));
        }
        self.accounts[idx].capital = U128::new(new_capital);
    }

    // ========================================
    // Warmup & settlement helpers (T5: PERC-8270)
    // ========================================

    /// released_pos (spec §2.1): ReleasedPos_i = max(PNL_i, 0) - R_i
    #[allow(dead_code)]
    pub fn released_pos(&self, idx: usize) -> u128 {
        let pnl = self.accounts[idx].pnl.get();
        let pos_pnl = i128_clamp_pos(pnl);
        pos_pnl.saturating_sub(self.accounts[idx].reserved_pnl)
    }

    /// use_insurance_buffer (spec §4.11): deduct loss from insurance down to floor.
    #[allow(dead_code)]
    fn use_insurance_buffer(&mut self, loss: u128) -> u128 {
        if loss == 0 {
            return 0;
        }
        let ins_bal = self.insurance_fund.balance.get();
        let floor = self.params.insurance_floor.get();
        let available = ins_bal.saturating_sub(floor);
        let pay = core::cmp::min(loss, available);
        if pay > 0 {
            self.insurance_fund.balance = U128::new(ins_bal - pay);
        }
        loss - pay
    }

    /// absorb_protocol_loss (spec §4.11): use insurance buffer, remainder is implicit haircut.
    #[allow(dead_code)]
    fn absorb_protocol_loss(&mut self, loss: u128) {
        if loss == 0 {
            return;
        }
        let _rem = self.use_insurance_buffer(loss);
    }

    /// restart_warmup_after_reserve_increase (spec §4.9)
    #[allow(dead_code)]
    fn restart_warmup_after_reserve_increase(&mut self, idx: usize) {
        let t = self.params.warmup_period_slots;
        if t == 0 {
            self.set_reserved_pnl(idx, 0);
            self.accounts[idx].warmup_slope_per_step = U128::ZERO;
            self.accounts[idx].warmup_started_at_slot = self.current_slot;
            return;
        }
        let r = self.accounts[idx].reserved_pnl;
        if r == 0 {
            self.accounts[idx].warmup_slope_per_step = U128::ZERO;
            self.accounts[idx].warmup_started_at_slot = self.current_slot;
            return;
        }
        let base = r / (t as u128);
        let slope = if base == 0 { 1u128 } else { base };
        self.accounts[idx].warmup_slope_per_step = U128::new(slope);
        self.accounts[idx].warmup_started_at_slot = self.current_slot;
    }

    /// advance_profit_warmup (spec §4.9): advance warmup clock for account idx.
    #[allow(dead_code)]
    fn advance_profit_warmup(&mut self, idx: usize) {
        let r = self.accounts[idx].reserved_pnl;
        if r == 0 {
            self.accounts[idx].warmup_slope_per_step = U128::ZERO;
            self.accounts[idx].warmup_started_at_slot = self.current_slot;
            return;
        }
        let t = self.params.warmup_period_slots;
        if t == 0 {
            self.set_reserved_pnl(idx, 0);
            self.accounts[idx].warmup_slope_per_step = U128::ZERO;
            self.accounts[idx].warmup_started_at_slot = self.current_slot;
            return;
        }
        let elapsed = self
            .current_slot
            .saturating_sub(self.accounts[idx].warmup_started_at_slot);
        let cap = saturating_mul_u128_u64(self.accounts[idx].warmup_slope_per_step.get(), elapsed);
        let release = core::cmp::min(r, cap);
        if release > 0 {
            self.set_reserved_pnl(idx, r - release);
        }
        if self.accounts[idx].reserved_pnl == 0 {
            self.accounts[idx].warmup_slope_per_step = U128::ZERO;
        }
        self.accounts[idx].warmup_started_at_slot = self.current_slot;
    }

    /// settle_losses (spec §7.1): settle negative PnL from principal.
    #[allow(dead_code)]
    fn settle_losses(&mut self, idx: usize) {
        let pnl = self.accounts[idx].pnl.get();
        if pnl >= 0 {
            return;
        }
        assert!(pnl != i128::MIN, "settle_losses: i128::MIN");
        let need = pnl.unsigned_abs();
        let cap = self.accounts[idx].capital.get();
        let pay = core::cmp::min(need, cap);
        if pay > 0 {
            self.set_capital(idx, cap - pay);
            let pay_i128 = pay as i128;
            let new_pnl = pnl.checked_add(pay_i128).unwrap_or(0i128);
            self.set_pnl(idx, if new_pnl == i128::MIN { 0i128 } else { new_pnl });
        }
    }

    /// resolve_flat_negative (spec §7.3): for flat accounts with negative PnL.
    #[allow(dead_code)]
    fn resolve_flat_negative(&mut self, idx: usize) {
        let eff = self.effective_pos_q(idx);
        if eff != 0 {
            return;
        }
        let pnl = self.accounts[idx].pnl.get();
        if pnl < 0 {
            assert!(pnl != i128::MIN, "resolve_flat_negative: i128::MIN");
            let loss = pnl.unsigned_abs();
            self.absorb_protocol_loss(loss);
            self.set_pnl(idx, 0i128);
        }
    }

    /// do_profit_conversion (spec §7.4): convert matured released profit into principal.
    #[allow(dead_code)]
    fn do_profit_conversion(&mut self, idx: usize) {
        let x = self.released_pos(idx);
        if x == 0 {
            return;
        }
        let (h_num, h_den) = self.haircut_ratio();
        assert!(
            h_den > 0,
            "do_profit_conversion: h_den must be > 0 when x > 0"
        );
        let y: u128 = wide_mul_div_floor_u128(x, h_num, h_den);
        self.consume_released_pnl(idx, x);
        let new_cap = add_u128(self.accounts[idx].capital.get(), y);
        self.set_capital(idx, new_cap);
        if self.accounts[idx].reserved_pnl == 0 {
            self.accounts[idx].warmup_slope_per_step = U128::ZERO;
            self.accounts[idx].warmup_started_at_slot = self.current_slot;
        }
    }

    /// fee_debt_sweep (spec §7.5): after capital increase, sweep fee debt.
    #[allow(dead_code)]
    fn fee_debt_sweep(&mut self, idx: usize) {
        let fc = self.accounts[idx].fee_credits.get();
        let debt = fee_debt_u128_checked(fc);
        if debt == 0 {
            return;
        }
        let cap = self.accounts[idx].capital.get();
        let pay = core::cmp::min(debt, cap);
        if pay > 0 {
            self.set_capital(idx, cap - pay);
            let pay_i128 = core::cmp::min(pay, i128::MAX as u128) as i128;
            self.accounts[idx].fee_credits = I128::new(
                self.accounts[idx]
                    .fee_credits
                    .get()
                    .checked_add(pay_i128)
                    .expect("fee_debt_sweep overflow"),
            );
            self.insurance_fund.balance += pay;
        }
    }

    /// settle_maintenance_fee_internal (spec §8.2): update last_fee_slot for account.
    #[allow(dead_code)]
    fn settle_maintenance_fee_internal(&mut self, idx: usize, now_slot: u64) -> Result<()> {
        self.accounts[idx].last_fee_slot = now_slot;
        Ok(())
    }

    // ========================================
    // ADL settle / accrue helpers (T5: PERC-8270)
    // ========================================

    /// settle_side_effects (spec §5.3): settle A/K gains for account at current epoch.
    ///
    /// PERC-8459 (SYNC-02): Refactored to validate-then-mutate pattern.
    /// Phase 1: COMPUTE + VALIDATE — all arithmetic and validations complete before
    ///          any state mutation. If any validation fails, state is untouched.
    /// Phase 2: MUTATE — apply all state changes atomically after validation passes.
    #[allow(dead_code)]
    pub fn settle_side_effects(&mut self, idx: usize) -> Result<()> {
        let basis = self.accounts[idx].position_basis_q;
        if basis == 0 {
            return Ok(());
        }

        let side = side_of_i128(basis).unwrap();
        let epoch_snap = self.accounts[idx].adl_epoch_snap;
        let epoch_side = self.get_epoch_side(side);
        let a_basis = self.accounts[idx].adl_a_basis;
        if a_basis == 0 {
            return Err(RiskError::CorruptState);
        }
        let abs_basis = basis.unsigned_abs();

        if epoch_snap == epoch_side {
            // ── Phase 1: COMPUTE + VALIDATE (same-epoch branch) ──────────
            let a_side = self.get_a_side(side);
            let k_side = self.get_k_side(side);
            let k_snap = self.accounts[idx].adl_k_snap;
            let q_eff_new = mul_div_floor_u128(abs_basis, a_side, a_basis);
            let old_r = self.accounts[idx].reserved_pnl;
            let den = a_basis
                .checked_mul(1_000_000u128)
                .ok_or(RiskError::Overflow)?;
            let pnl_delta = wide_signed_mul_div_floor_from_k_pair(abs_basis, k_snap, k_side, den);
            let old_pnl = self.accounts[idx].pnl.get();
            let new_pnl = old_pnl.checked_add(pnl_delta).ok_or(RiskError::Overflow)?;
            if new_pnl == i128::MIN {
                return Err(RiskError::Overflow);
            }

            // ── Phase 2: MUTATE (same-epoch branch) ──────────────────────
            self.set_pnl(idx, new_pnl);
            if self.accounts[idx].reserved_pnl > old_r {
                self.restart_warmup_after_reserve_increase(idx);
            }
            if q_eff_new == 0 {
                self.inc_phantom_dust_bound(side);
                self.set_position_basis_q(idx, 0i128);
                self.accounts[idx].adl_a_basis = 1_000_000u128;
                self.accounts[idx].adl_k_snap = 0i128;
                self.accounts[idx].adl_epoch_snap = 0;
            } else {
                self.accounts[idx].adl_k_snap = k_side;
                self.accounts[idx].adl_epoch_snap = epoch_side;
            }
        } else {
            // ── Phase 1: COMPUTE + VALIDATE (epoch-mismatch branch) ──────
            let side_mode = self.get_side_mode(side);
            if side_mode != SideMode::ResetPending {
                return Err(RiskError::CorruptState);
            }
            if epoch_snap.checked_add(1) != Some(epoch_side) {
                return Err(RiskError::CorruptState);
            }
            let k_epoch_start = self.get_k_epoch_start(side);
            let k_snap = self.accounts[idx].adl_k_snap;
            let old_r = self.accounts[idx].reserved_pnl;
            let den = a_basis
                .checked_mul(1_000_000u128)
                .ok_or(RiskError::Overflow)?;
            let pnl_delta =
                wide_signed_mul_div_floor_from_k_pair(abs_basis, k_snap, k_epoch_start, den);
            let old_pnl = self.accounts[idx].pnl.get();
            let new_pnl = old_pnl.checked_add(pnl_delta).ok_or(RiskError::Overflow)?;
            if new_pnl == i128::MIN {
                return Err(RiskError::Overflow);
            }
            // Validate stale_count BEFORE any mutation (PERC-8459 fix)
            let old_stale = self.get_stale_count(side);
            let new_stale = old_stale.checked_sub(1).ok_or(RiskError::CorruptState)?;

            // ── Phase 2: MUTATE (epoch-mismatch branch) ──────────────────
            self.set_pnl(idx, new_pnl);
            if self.accounts[idx].reserved_pnl > old_r {
                self.restart_warmup_after_reserve_increase(idx);
            }
            self.set_position_basis_q(idx, 0i128);
            self.set_stale_count(side, new_stale);
            self.accounts[idx].adl_a_basis = 1_000_000u128;
            self.accounts[idx].adl_k_snap = 0i128;
            self.accounts[idx].adl_epoch_snap = 0;
        }
        Ok(())
    }

    /// accrue_market_to (spec §5.4): advance K/A coefficients for elapsed slots.
    /// Called once per keeper_crank invocation to update ADL market state.
    #[allow(dead_code)]
    fn accrue_market_to(&mut self, now_slot: u64, oracle_price: u64) -> Result<()> {
        if oracle_price == 0 || oracle_price > MAX_ORACLE_PRICE {
            return Err(RiskError::Overflow);
        }
        if now_slot < self.current_slot {
            return Err(RiskError::Overflow);
        }
        if now_slot < self.last_market_slot {
            return Err(RiskError::Overflow);
        }

        // Step 4: snapshot OI at start (fixed for all sub-steps per spec §5.4)
        let long_live = self.oi_eff_long_q != 0;
        let short_live = self.oi_eff_short_q != 0;

        let total_dt = now_slot.saturating_sub(self.last_market_slot);
        if total_dt == 0 && self.last_oracle_price == oracle_price {
            // Step 5: no change — set current_slot and return (spec §5.4)
            self.current_slot = now_slot;
            return Ok(());
        }

        // Use scratch K values for the entire mark + funding computation.
        // Only commit to engine state after ALL computations succeed.
        // This prevents partial K advancement on mid-function errors.
        let mut k_long = self.adl_coeff_long;
        let mut k_short = self.adl_coeff_short;

        // Step 5: Mark-to-market (once, spec §1.5 item 21)
        let current_price = self.last_oracle_price;
        let delta_p = (oracle_price as i128)
            .checked_sub(current_price as i128)
            .ok_or(RiskError::Overflow)?;
        if delta_p != 0 {
            if long_live {
                let dk = Self::checked_u128_mul_i128(self.adl_mult_long, delta_p)?;
                k_long = k_long.checked_add(dk).ok_or(RiskError::Overflow)?;
            }
            if short_live {
                let dk = Self::checked_u128_mul_i128(self.adl_mult_short, delta_p)?;
                k_short = k_short.checked_sub(dk).ok_or(RiskError::Overflow)?;
            }
        }

        // Step 6: Funding transfer via sub-stepping (spec v12.1.0 §5.4)
        let r_last = self.funding_rate_bps_per_slot_last;
        if r_last != 0 && total_dt > 0 && long_live && short_live {
            let fund_px_0 = self.funding_price_sample_last;
            if fund_px_0 > 0 {
                let mut dt_remaining = total_dt;
                while dt_remaining > 0 {
                    let dt_sub = core::cmp::min(dt_remaining, MAX_FUNDING_DT);
                    dt_remaining -= dt_sub;
                    let fund_num: i128 = (fund_px_0 as i128)
                        .checked_mul(r_last as i128)
                        .ok_or(RiskError::Overflow)?
                        .checked_mul(dt_sub as i128)
                        .ok_or(RiskError::Overflow)?;
                    let fund_term = floor_div_signed_conservative_i128(fund_num, 10_000u128);
                    if fund_term != 0 {
                        let dk_long = Self::checked_u128_mul_i128(self.adl_mult_long, fund_term)?;
                        k_long = k_long.checked_sub(dk_long).ok_or(RiskError::Overflow)?;
                        let dk_short = Self::checked_u128_mul_i128(self.adl_mult_short, fund_term)?;
                        k_short = k_short.checked_add(dk_short).ok_or(RiskError::Overflow)?;
                    }
                }
            }
        }

        // ALL computations succeeded — commit K values and synchronize state
        self.adl_coeff_long = k_long;
        self.adl_coeff_short = k_short;
        self.current_slot = now_slot;
        self.last_market_slot = now_slot;
        self.last_oracle_price = oracle_price;
        self.funding_price_sample_last = oracle_price;
        Ok(())
    }

    /// Pre-validate funding rate bound (called at top of each instruction,
    /// before any mutations, so bad rates never cause partial-mutation errors).
    fn validate_funding_rate(rate: i64) -> Result<()> {
        if rate.unsigned_abs() > MAX_ABS_FUNDING_BPS_PER_SLOT as u64 {
            return Err(RiskError::Overflow);
        }
        Ok(())
    }

    /// recompute_r_last_from_final_state (spec v12.1.0 §4.12).
    /// Stores the pre-validated funding rate for the next interval.
    #[allow(dead_code)]
    fn recompute_r_last_from_final_state(&mut self, externally_computed_rate: i64) -> Result<()> {
        // Rate already validated at instruction entry; belt-and-suspenders re-check.
        if externally_computed_rate.unsigned_abs() > MAX_ABS_FUNDING_BPS_PER_SLOT as u64 {
            return Err(RiskError::Overflow);
        }
        self.funding_rate_bps_per_slot_last = externally_computed_rate;
        Ok(())
    }

    /// Recompute c_tot, pnl_pos_tot, and pnl_matured_pos_tot from account data.
    /// For test use after direct state mutation.
    pub fn recompute_aggregates(&mut self) {
        let mut c_tot = 0u128;
        let mut pnl_pos_tot = 0u128;
        let mut pnl_matured_pos_tot = 0u128;
        for idx in 0..MAX_ACCOUNTS {
            if !self.is_used(idx) {
                continue;
            }
            let account = &self.accounts[idx];
            c_tot = c_tot.saturating_add(account.capital.get());
            let pnl = account.pnl.get();
            if pnl > 0 {
                let pos = pnl as u128;
                pnl_pos_tot = pnl_pos_tot.saturating_add(pos);
                let released = pos.saturating_sub(account.reserved_pnl);
                pnl_matured_pos_tot = pnl_matured_pos_tot.saturating_add(released);
            }
        }
        self.c_tot = U128::new(c_tot);
        self.pnl_pos_tot = U128::new(pnl_pos_tot);
        self.pnl_matured_pos_tot = pnl_matured_pos_tot;
    }

    /// Compute haircut ratio (h_num, h_den) per spec §3.2 (v11.21+).
    /// Uses pnl_matured_pos_tot as denominator: only matured/released PnL participates.
    /// h = min(Residual, PNL_matured_pos_tot) / PNL_matured_pos_tot
    ///   where Residual = max(0, V - C_tot - I).
    /// Returns (1, 1) when PNL_matured_pos_tot == 0 (no mature PnL to haircut).
    #[inline]
    pub fn haircut_ratio(&self) -> (u128, u128) {
        let pnl_matured = self.pnl_matured_pos_tot;
        if pnl_matured == 0 {
            return (1, 1);
        }
        let total_insurance =
            self.insurance_fund.balance.get() + self.insurance_fund.isolated_balance.get();
        let residual = self
            .vault
            .get()
            .saturating_sub(self.c_tot.get())
            .saturating_sub(total_insurance);
        let h_num = core::cmp::min(residual, pnl_matured);
        (h_num, pnl_matured)
    }

    /// Compute effective positive PnL after haircut for a given account PnL (spec §3.3).
    /// PNL_eff_pos_i = floor(max(PNL_i, 0) * h_num / h_den)
    #[inline]
    pub fn effective_pos_pnl(&self, pnl: i128) -> u128 {
        if pnl <= 0 {
            return 0;
        }
        let pos_pnl = pnl as u128;
        let (h_num, h_den) = self.haircut_ratio();
        if h_den == 0 {
            return pos_pnl;
        }
        // floor(pos_pnl * h_num / h_den)
        mul_u128(pos_pnl, h_num) / h_den
    }

    /// Compute effective realized equity per spec §3.3.
    /// Eq_real_i = max(0, C_i + min(PNL_i, 0) + PNL_eff_pos_i)
    #[inline]
    pub fn effective_equity(&self, account: &Account) -> u128 {
        let cap_i = u128_to_i128_clamped(account.capital.get());
        let neg_pnl = core::cmp::min(account.pnl.get(), 0);
        let eff_pos = self.effective_pos_pnl(account.pnl.get());
        let eq_i = cap_i
            .saturating_add(neg_pnl)
            .saturating_add(u128_to_i128_clamped(eff_pos));
        if eq_i > 0 {
            eq_i as u128
        } else {
            0
        }
    }

    // ========================================
    // Account Allocation
    // ========================================

    fn alloc_slot(&mut self) -> Result<u16> {
        if self.free_head == u16::MAX {
            return Err(RiskError::Overflow); // Slab full
        }
        let idx = self.free_head;
        self.free_head = self.next_free[idx as usize];
        self.set_used(idx as usize);
        // Increment O(1) counter atomically (fixes H2: TOCTOU fee bypass)
        self.num_used_accounts = self.num_used_accounts.saturating_add(1);
        Ok(idx)
    }

    /// Count used accounts
    #[allow(dead_code)]
    fn count_used(&self) -> u64 {
        let mut count = 0u64;
        self.for_each_used(|_, _| {
            count += 1;
        });
        count
    }

    // ========================================
    // Account Management
    // ========================================

    /// Add a new user account
    pub fn add_user(&mut self, fee_payment: u128) -> Result<u16> {
        // Use O(1) counter instead of O(N) count_used() (fixes H2: TOCTOU fee bypass)
        let used_count = self.num_used_accounts as u64;
        if used_count >= self.params.max_accounts {
            return Err(RiskError::Overflow);
        }

        // Flat fee (no scaling)
        let required_fee = self.params.new_account_fee.get();
        if fee_payment < required_fee {
            return Err(RiskError::InsufficientBalance);
        }

        // Bug #4 fix: Compute excess payment to credit to user capital
        let excess = fee_payment.saturating_sub(required_fee);

        // Pay fee to insurance (fee tokens are deposited into vault)
        // Account for FULL fee_payment in vault, not just required_fee
        self.vault += fee_payment;
        self.insurance_fund.balance += required_fee;
        self.insurance_fund.fee_revenue += required_fee;

        // Allocate slot and assign unique ID
        let idx = self.alloc_slot()?;
        let account_id = self.next_account_id;
        self.next_account_id = self.next_account_id.saturating_add(1);

        // Initialize account with excess credited to capital
        self.accounts[idx as usize] = Account {
            kind: AccountKind::User,
            account_id,
            capital: U128::new(excess), // Bug #4 fix: excess goes to user capital
            pnl: I128::ZERO,
            reserved_pnl: 0,
            warmup_started_at_slot: self.current_slot,
            warmup_slope_per_step: U128::ZERO,
            position_size: I128::ZERO,
            entry_price: 0,
            funding_index: self.funding_index_qpb_e6,
            matcher_program: [0; 32],
            matcher_context: [0; 32],
            owner: [0; 32],
            fee_credits: I128::ZERO,
            last_fee_slot: self.current_slot,
            last_partial_liquidation_slot: 0,
            position_basis_q: 0i128,
            adl_a_basis: 1_000_000u128,
            adl_k_snap: 0i128,
            adl_epoch_snap: 0,
        };

        // Maintain c_tot aggregate (account was created with capital = excess)
        if excess > 0 {
            self.c_tot = U128::new(self.c_tot.get().saturating_add(excess));
        }

        Ok(idx)
    }

    /// Add a new LP account
    pub fn add_lp(
        &mut self,
        matching_engine_program: [u8; 32],
        matching_engine_context: [u8; 32],
        fee_payment: u128,
    ) -> Result<u16> {
        // Use O(1) counter instead of O(N) count_used() (fixes H2: TOCTOU fee bypass)
        let used_count = self.num_used_accounts as u64;
        if used_count >= self.params.max_accounts {
            return Err(RiskError::Overflow);
        }

        // Flat fee (no scaling)
        let required_fee = self.params.new_account_fee.get();
        if fee_payment < required_fee {
            return Err(RiskError::InsufficientBalance);
        }

        // Bug #4 fix: Compute excess payment to credit to LP capital
        let excess = fee_payment.saturating_sub(required_fee);

        // Pay fee to insurance (fee tokens are deposited into vault)
        // Account for FULL fee_payment in vault, not just required_fee
        self.vault += fee_payment;
        self.insurance_fund.balance += required_fee;
        self.insurance_fund.fee_revenue += required_fee;

        // Allocate slot and assign unique ID
        let idx = self.alloc_slot()?;
        let account_id = self.next_account_id;
        self.next_account_id = self.next_account_id.saturating_add(1);

        // Initialize account with excess credited to capital
        self.accounts[idx as usize] = Account {
            kind: AccountKind::LP,
            account_id,
            capital: U128::new(excess), // Bug #4 fix: excess goes to LP capital
            pnl: I128::ZERO,
            reserved_pnl: 0,
            warmup_started_at_slot: self.current_slot,
            warmup_slope_per_step: U128::ZERO,
            position_size: I128::ZERO,
            entry_price: 0,
            funding_index: self.funding_index_qpb_e6,
            matcher_program: matching_engine_program,
            matcher_context: matching_engine_context,
            owner: [0; 32],
            fee_credits: I128::ZERO,
            last_fee_slot: self.current_slot,
            last_partial_liquidation_slot: 0,
            position_basis_q: 0i128,
            adl_a_basis: 1_000_000u128,
            adl_k_snap: 0i128,
            adl_epoch_snap: 0,
        };

        // Maintain c_tot aggregate (account was created with capital = excess)
        if excess > 0 {
            self.c_tot = U128::new(self.c_tot.get().saturating_add(excess));
        }

        Ok(idx)
    }

    // ========================================
    // Maintenance Fees
    // ========================================

    /// Settle maintenance fees for an account.
    ///
    /// Returns the fee amount due (for keeper rebate calculation).
    ///
    /// Algorithm:
    /// 1. Compute dt = now_slot - account.last_fee_slot
    /// 2. If dt == 0, return 0 (no-op)
    /// 3. Compute due = fee_per_slot * dt
    /// 4. Deduct from fee_credits; if negative, pay from capital to insurance
    /// 5. If position exists and below maintenance after fee, return Err
    pub fn settle_maintenance_fee(
        &mut self,
        idx: u16,
        now_slot: u64,
        oracle_price: u64,
    ) -> Result<u128> {
        if idx as usize >= MAX_ACCOUNTS || !self.is_used(idx as usize) {
            return Err(RiskError::Unauthorized);
        }

        // Calculate elapsed time
        let dt = now_slot.saturating_sub(self.accounts[idx as usize].last_fee_slot);
        if dt == 0 {
            return Ok(0);
        }

        // Calculate fee due (engine is purely slot-native)
        let due = self
            .params
            .maintenance_fee_per_slot
            .get()
            .saturating_mul(dt as u128);

        // Update last_fee_slot
        self.accounts[idx as usize].last_fee_slot = now_slot;

        // Deduct from fee_credits (coupon: no insurance booking here —
        // insurance was already paid when credits were granted)
        self.accounts[idx as usize].fee_credits = self.accounts[idx as usize]
            .fee_credits
            .saturating_sub(due as i128);

        // If fee_credits is negative, pay from capital using set_capital helper (spec §4.1)
        let mut paid_from_capital = 0u128;
        if self.accounts[idx as usize].fee_credits.is_negative() {
            let owed = neg_i128_to_u128(self.accounts[idx as usize].fee_credits.get());
            let current_cap = self.accounts[idx as usize].capital.get();
            let pay = core::cmp::min(owed, current_cap);

            // Use set_capital helper to maintain c_tot aggregate (spec §4.1)
            self.set_capital(idx as usize, current_cap.saturating_sub(pay));
            self.insurance_fund.balance += pay;
            self.insurance_fund.fee_revenue += pay;

            // Credit back what was paid
            self.accounts[idx as usize].fee_credits = self.accounts[idx as usize]
                .fee_credits
                .saturating_add(u128_to_i128_clamped(pay));
            paid_from_capital = pay;
        }

        // Check maintenance margin if account has a position (MTM check)
        if !self.accounts[idx as usize].position_size.is_zero() {
            let account_ref = &self.accounts[idx as usize];
            if !self.is_above_maintenance_margin_mtm(account_ref, oracle_price) {
                return Err(RiskError::Undercollateralized);
            }
        }

        Ok(paid_from_capital) // Return actual amount paid into insurance
    }

    /// Best-effort maintenance settle for crank paths.
    /// - Always advances last_fee_slot
    /// - Charges fees into insurance if possible
    /// - NEVER fails due to margin checks
    /// - Still returns Unauthorized if idx invalid
    fn settle_maintenance_fee_best_effort_for_crank(
        &mut self,
        idx: u16,
        now_slot: u64,
    ) -> Result<u128> {
        if idx as usize >= MAX_ACCOUNTS || !self.is_used(idx as usize) {
            return Err(RiskError::Unauthorized);
        }

        let dt = now_slot.saturating_sub(self.accounts[idx as usize].last_fee_slot);
        if dt == 0 {
            return Ok(0);
        }

        let due = self
            .params
            .maintenance_fee_per_slot
            .get()
            .saturating_mul(dt as u128);

        // Advance slot marker regardless
        self.accounts[idx as usize].last_fee_slot = now_slot;

        // Deduct from fee_credits (coupon: no insurance booking here —
        // insurance was already paid when credits were granted)
        self.accounts[idx as usize].fee_credits = self.accounts[idx as usize]
            .fee_credits
            .saturating_sub(due as i128);

        // If negative, pay what we can from capital using set_capital helper (spec §4.1)
        let mut paid_from_capital = 0u128;
        if self.accounts[idx as usize].fee_credits.is_negative() {
            let owed = neg_i128_to_u128(self.accounts[idx as usize].fee_credits.get());
            let current_cap = self.accounts[idx as usize].capital.get();
            let pay = core::cmp::min(owed, current_cap);

            // Use set_capital helper to maintain c_tot aggregate (spec §4.1)
            self.set_capital(idx as usize, current_cap.saturating_sub(pay));
            self.insurance_fund.balance += pay;
            self.insurance_fund.fee_revenue += pay;

            self.accounts[idx as usize].fee_credits = self.accounts[idx as usize]
                .fee_credits
                .saturating_add(u128_to_i128_clamped(pay));
            paid_from_capital = pay;
        }

        Ok(paid_from_capital) // Return actual amount paid into insurance
    }

    /// Best-effort warmup settlement for crank: settles any warmed positive PnL to capital.
    /// Silently ignores errors (e.g., account not found) since crank must not stall on
    /// individual account issues. Used to drain abandoned accounts' positive PnL over time.
    fn settle_warmup_to_capital_for_crank(&mut self, idx: u16) {
        // Ignore errors: crank is best-effort and must continue processing other accounts
        let _ = self.settle_warmup_to_capital(idx);
    }

    /// Pay down existing fee debt (negative fee_credits) using available capital.
    /// Does not advance last_fee_slot or charge new fees — just sweeps capital
    /// that became available (e.g. after warmup settlement) into insurance.
    /// Uses set_capital helper to maintain c_tot aggregate (spec §4.1).
    fn pay_fee_debt_from_capital(&mut self, idx: u16) {
        if self.accounts[idx as usize].fee_credits.is_negative()
            && !self.accounts[idx as usize].capital.is_zero()
        {
            let owed = neg_i128_to_u128(self.accounts[idx as usize].fee_credits.get());
            let current_cap = self.accounts[idx as usize].capital.get();
            let pay = core::cmp::min(owed, current_cap);
            if pay > 0 {
                // Use set_capital helper to maintain c_tot aggregate (spec §4.1)
                self.set_capital(idx as usize, current_cap.saturating_sub(pay));
                self.insurance_fund.balance += pay;
                self.insurance_fund.fee_revenue += pay;
                self.accounts[idx as usize].fee_credits = self.accounts[idx as usize]
                    .fee_credits
                    .saturating_add(u128_to_i128_clamped(pay));
            }
        }
    }

    /// Touch account for force-realize paths: settles funding, mark, and fees but
    /// uses best-effort fee settle that can't stall on margin checks.
    fn touch_account_for_force_realize(
        &mut self,
        idx: u16,
        now_slot: u64,
        oracle_price: u64,
    ) -> Result<()> {
        // Funding settle is required for correct pnl
        self.touch_account(idx)?;
        // Mark-to-market settlement (variation margin)
        self.settle_mark_to_oracle(idx, oracle_price)?;
        // Best-effort fees; never fails due to maintenance margin
        let _ = self.settle_maintenance_fee_best_effort_for_crank(idx, now_slot)?;
        Ok(())
    }

    /// Touch account for liquidation paths: settles funding, mark, and fees but
    /// uses best-effort fee settle since we're about to liquidate anyway.
    fn touch_account_for_liquidation(
        &mut self,
        idx: u16,
        now_slot: u64,
        oracle_price: u64,
    ) -> Result<()> {
        // Funding settle is required for correct pnl
        self.touch_account(idx)?;

        // Per spec §5.4: if mark settlement increases AvailGross, warmup must reset.
        // Capture old AvailGross before mark settlement.
        let old_avail_gross = {
            let pnl = self.accounts[idx as usize].pnl.get();
            if pnl > 0 {
                (pnl as u128).saturating_sub(self.accounts[idx as usize].reserved_pnl)
            } else {
                0
            }
        };

        // Best-effort mark-to-market (saturating — never wedges on extreme PnL)
        self.settle_mark_to_oracle_best_effort(idx, oracle_price)?;

        // If AvailGross increased, update warmup slope (restarts warmup timer)
        let new_avail_gross = {
            let pnl = self.accounts[idx as usize].pnl.get();
            if pnl > 0 {
                (pnl as u128).saturating_sub(self.accounts[idx as usize].reserved_pnl)
            } else {
                0
            }
        };
        if new_avail_gross > old_avail_gross {
            self.update_warmup_slope(idx)?;
        }

        // Best-effort fees; margin check would just block the liquidation we need to do
        let _ = self.settle_maintenance_fee_best_effort_for_crank(idx, now_slot)?;
        Ok(())
    }

    /// Set owner pubkey for an account
    pub fn set_owner(&mut self, idx: u16, owner: [u8; 32]) -> Result<()> {
        if idx as usize >= MAX_ACCOUNTS || !self.is_used(idx as usize) {
            return Err(RiskError::Unauthorized);
        }
        self.accounts[idx as usize].owner = owner;
        Ok(())
    }

    /// Pre-fund fee credits for an account.
    ///
    /// The wrapper must have already transferred `amount` tokens into the vault.
    /// This pre-pays future maintenance fees: vault increases, insurance receives
    /// the amount as revenue (since credits are a coupon — spending them later
    /// does NOT re-book into insurance), and the account's fee_credits balance
    /// increases by `amount`.
    pub fn deposit_fee_credits(&mut self, idx: u16, amount: u128, now_slot: u64) -> Result<()> {
        if idx as usize >= MAX_ACCOUNTS || !self.is_used(idx as usize) {
            return Err(RiskError::Unauthorized);
        }
        self.current_slot = now_slot;

        // Wrapper transferred tokens into vault
        self.vault += amount;

        // Pre-fund: insurance receives the amount now.
        // When credits are later spent during fee settlement, no further
        // insurance booking occurs (coupon semantics).
        self.insurance_fund.balance += amount;
        self.insurance_fund.fee_revenue += amount;

        // Credit the account
        self.accounts[idx as usize].fee_credits = self.accounts[idx as usize]
            .fee_credits
            .saturating_add(amount as i128);

        Ok(())
    }

    /// Add fee credits without vault/insurance accounting.
    /// Only for tests and Kani proofs — production code must use deposit_fee_credits.
    #[cfg(any(test, feature = "test", kani))]
    pub fn add_fee_credits(&mut self, idx: u16, amount: u128) -> Result<()> {
        if idx as usize >= MAX_ACCOUNTS || !self.is_used(idx as usize) {
            return Err(RiskError::Unauthorized);
        }
        self.accounts[idx as usize].fee_credits = self.accounts[idx as usize]
            .fee_credits
            .saturating_add(amount as i128);
        Ok(())
    }

    /// Set the risk reduction threshold (admin function).
    /// This controls when risk-reduction-only mode is triggered.
    #[inline]
    pub fn set_risk_reduction_threshold(&mut self, new_threshold: u128) {
        self.params.risk_reduction_threshold = U128::new(new_threshold);
    }

    /// Get the current risk reduction threshold.
    #[inline]
    pub fn risk_reduction_threshold(&self) -> u128 {
        self.params.risk_reduction_threshold.get()
    }

    /// Admin force-close: unconditionally close a position at oracle price.
    /// Skips margin checks — intended for emergency admin use only.
    /// Settles mark PnL first, then closes position.
    pub fn admin_force_close(&mut self, idx: u16, now_slot: u64, oracle_price: u64) -> Result<()> {
        // Bounds check: prevent OOB panic / DoS
        if (idx as usize) >= MAX_ACCOUNTS {
            return Err(RiskError::AccountNotFound);
        }
        // Existence check: account must be in use
        if !self.is_used(idx as usize) {
            return Err(RiskError::AccountNotFound);
        }
        self.current_slot = now_slot;
        if self.accounts[idx as usize].position_size.is_zero() {
            return Ok(());
        }
        // Settle funding + mark PnL before closing
        self.settle_mark_to_oracle_best_effort(idx, oracle_price)?;
        // Close position at oracle price
        self.oracle_close_position_core(idx, oracle_price)?;
        Ok(())
    }

    /// PERC-305: Auto-deleverage — surgically close or reduce a profitable position
    /// to bring `pnl_pos_tot` back within bounds.
    ///
    /// # Preconditions (caller must verify):
    /// - `pnl_pos_tot > pnl_cap` (the cap is exceeded)
    /// - Target account has positive effective PnL
    ///
    /// # Parameters
    /// - `idx`: account index to deleverage
    /// - `now_slot`: current slot for funding settlement
    /// - `oracle_price`: current oracle price (e6)
    /// - `excess`: `pnl_pos_tot - pnl_cap` (amount of PnL to remove)
    ///
    /// # Returns
    /// `Ok(closed_abs)` — the absolute position size that was closed.
    pub fn execute_adl(
        &mut self,
        idx: u16,
        now_slot: u64,
        oracle_price: u64,
        excess: u128,
    ) -> Result<u128> {
        if (idx as usize) >= MAX_ACCOUNTS {
            return Err(RiskError::AccountNotFound);
        }
        if !self.is_used(idx as usize) {
            return Err(RiskError::AccountNotFound);
        }
        self.current_slot = now_slot;

        let pos = self.accounts[idx as usize].position_size.get();
        if pos == 0 {
            return Err(RiskError::AccountNotFound);
        }

        // Settle funding + mark PnL before computing effective PnL
        self.settle_mark_to_oracle_best_effort(idx, oracle_price)?;

        let target_pnl = self.accounts[idx as usize].pnl.get();
        if target_pnl <= 0 {
            return Err(RiskError::Undercollateralized); // Target is not profitable
        }

        let target_positive_pnl = target_pnl as u128;
        let abs_pos = saturating_abs_i128(pos) as u128;

        let result = if target_positive_pnl <= excess || abs_pos == 0 {
            // Close entire position — not enough to cover all excess
            self.oracle_close_position_core(idx, oracle_price)?;
            abs_pos
        } else {
            // Partial close: close proportion = excess / target_positive_pnl
            let close_abs = abs_pos
                .checked_mul(excess)
                .map(|v| v / target_positive_pnl)
                .unwrap_or(abs_pos);
            let close_abs = core::cmp::max(close_abs, 1);

            self.oracle_close_position_slice_core(idx, oracle_price, close_abs)?;
            close_abs
        };

        // End-of-instruction lifecycle: finalize any deferred ADL epoch resets
        // that were triggered during this ADL execution (spec §5.7-5.8).
        // Use stored funding_rate_bps_per_slot_last — NOT 0i64 — to avoid
        // overwriting the funding rate with a stale zero (security issue: LOW).
        let mut ctx = InstructionContext::new();
        let stored_rate = self.funding_rate_bps_per_slot_last;
        self.run_end_of_instruction_lifecycle(&mut ctx, stored_rate)?;

        Ok(result)
    }

    /// Update initial and maintenance margin BPS. Admin only.
    pub fn set_margin_params(
        &mut self,
        initial_margin_bps: u64,
        maintenance_margin_bps: u64,
    ) -> Result<()> {
        if maintenance_margin_bps == 0 || initial_margin_bps == 0 {
            return Err(RiskError::Overflow);
        }
        if initial_margin_bps > 10_000 || maintenance_margin_bps > 10_000 {
            return Err(RiskError::Overflow);
        }
        if initial_margin_bps < maintenance_margin_bps {
            return Err(RiskError::Overflow);
        }
        self.params.initial_margin_bps = initial_margin_bps;
        self.params.maintenance_margin_bps = maintenance_margin_bps;
        Ok(())
    }

    /// Close an account and return its capital to the caller.
    ///
    /// Requirements:
    /// - Account must exist
    /// - Position must be zero (no open positions)
    /// - fee_credits >= 0 (no outstanding fees owed)
    /// - pnl must be 0 after settlement (positive pnl must be warmed up first)
    ///
    /// Returns Err(PnlNotWarmedUp) if pnl > 0 (user must wait for warmup).
    /// Returns Err(Undercollateralized) if pnl < 0 (shouldn't happen after settlement).
    /// Returns the capital amount on success.
    pub fn close_account(&mut self, idx: u16, now_slot: u64, oracle_price: u64) -> Result<u128> {
        // Update current_slot so warmup/bookkeeping progresses consistently
        self.current_slot = now_slot;

        if idx as usize >= MAX_ACCOUNTS || !self.is_used(idx as usize) {
            return Err(RiskError::AccountNotFound);
        }

        // Full settlement: funding + maintenance fees + warmup
        // This converts warmed pnl to capital and realizes negative pnl
        self.touch_account_full(idx, now_slot, oracle_price)?;

        // Position must be zero
        if !self.accounts[idx as usize].position_size.is_zero() {
            return Err(RiskError::Undercollateralized); // Has open position
        }

        // PnL must be zero BEFORE fee forgiveness to prevent in-memory state
        // mutation on the Err path (fee-debt evasion window — spec §10.6 ordering).
        // 1. Users can't bypass warmup by closing with positive unwarmed pnl
        // 2. Conservation is maintained (forfeiting pnl would create unbounded slack)
        // 3. Negative pnl after full settlement implies insolvency
        {
            let account = &self.accounts[idx as usize];
            if account.pnl.is_positive() {
                return Err(RiskError::PnlNotWarmedUp);
            }
            if account.pnl.is_negative() {
                return Err(RiskError::Undercollateralized);
            }
        }

        // Forgive any remaining fee debt (safe: position is zero, PnL is zero).
        // pay_fee_debt_from_capital (via touch_account_full above) already paid
        // what it could. Any remainder is uncollectable — forgive and proceed.
        if self.accounts[idx as usize].fee_credits.is_negative() {
            self.accounts[idx as usize].fee_credits = I128::ZERO;
        }

        let account = &self.accounts[idx as usize];

        let capital = account.capital;

        // Deduct from vault
        if capital > self.vault {
            return Err(RiskError::InsufficientBalance);
        }
        self.vault = self.vault - capital;

        // Decrement c_tot before freeing slot (free_slot zeroes account but doesn't update c_tot)
        self.set_capital(idx as usize, 0);

        // Free the slot
        self.free_slot(idx);

        Ok(capital.get())
    }

    // ========================================================================
    // force_close_resolved (resolved/frozen market path)
    // ========================================================================

    /// Force-close an account on a resolved market.
    ///
    /// Settles K-pair PnL, zeros position, settles losses, absorbs from
    /// insurance, converts profit (bypassing warmup), sweeps fee debt,
    /// forgives remainder, returns capital, frees slot.
    ///
    /// Skips accrue_market_to (market is frozen). Handles both same-epoch
    /// and epoch-mismatch accounts. For epoch-mismatch where the normal
    /// settle_side_effects would reject due to side mode, falls back to
    /// manual K-pair settlement using the same wide arithmetic.
    pub fn force_close_resolved(&mut self, idx: u16) -> Result<u128> {
        if idx as usize >= MAX_ACCOUNTS || !self.is_used(idx as usize) {
            return Err(RiskError::AccountNotFound);
        }

        let i = idx as usize;

        // Step 1: Settle K-pair PnL and zero position.
        // Uses validate-then-mutate: compute pnl_delta and validate all checked
        // ops BEFORE any mutation, preventing partial-mutation-on-error.
        // Does NOT call settle_side_effects (which interleaves mutations with
        // fallible checked_sub on stale_count).
        if self.accounts[i].position_basis_q != 0 {
            let basis = self.accounts[i].position_basis_q;
            let abs_basis = basis.unsigned_abs();
            let a_basis = self.accounts[i].adl_a_basis;
            let k_snap = self.accounts[i].adl_k_snap;
            let side = side_of_i128(basis).unwrap();
            let epoch_snap = self.accounts[i].adl_epoch_snap;
            let epoch_side = self.get_epoch_side(side);

            // Reject corrupt ADL state (a_basis must be > 0 for any position)
            if a_basis == 0 {
                return Err(RiskError::CorruptState);
            }

            // Phase 1: COMPUTE (no mutations)
            let k_end = if epoch_snap == epoch_side {
                self.get_k_side(side)
            } else {
                self.get_k_epoch_start(side)
            };
            let den = a_basis.checked_mul(POS_SCALE).ok_or(RiskError::Overflow)?;
            let pnl_delta = wide_signed_mul_div_floor_from_k_pair(abs_basis, k_snap, k_end, den);

            // Phase 1b: VALIDATE (check all fallible ops before mutating)
            let new_pnl = self.accounts[i]
                .pnl
                .get()
                .checked_add(pnl_delta)
                .ok_or(RiskError::Overflow)?;
            if new_pnl == i128::MIN {
                return Err(RiskError::Overflow);
            }
            // Validate OI decrement (computed before any mutation)
            let eff = self.effective_pos_q(i);
            if eff > 0 {
                self.oi_eff_long_q
                    .checked_sub(eff as u128)
                    .ok_or(RiskError::CorruptState)?;
            } else if eff < 0 {
                self.oi_eff_short_q
                    .checked_sub(eff.unsigned_abs())
                    .ok_or(RiskError::CorruptState)?;
            }

            if epoch_snap != epoch_side {
                // Validate epoch adjacency (same check as settle_side_effects
                // minus the ResetPending mode check, which is relaxed for
                // resolved markets where the side may be in any mode)
                if epoch_snap.checked_add(1) != Some(epoch_side) {
                    return Err(RiskError::CorruptState);
                }
                let old_stale = self.get_stale_count(side);
                if old_stale == 0 {
                    return Err(RiskError::CorruptState);
                }
            }

            // Phase 2: MUTATE (all validated, safe to commit)
            if pnl_delta != 0 {
                let old_r = self.accounts[i].reserved_pnl;
                self.set_pnl(i, new_pnl);
                if self.accounts[i].reserved_pnl > old_r {
                    self.restart_warmup_after_reserve_increase(i);
                }
            }

            // Decrement stale count (pre-validated above)
            if epoch_snap != epoch_side {
                let old_stale = self.get_stale_count(side);
                self.set_stale_count(side, old_stale - 1);
            }

            // Decrement OI (pre-validated above)
            if eff > 0 {
                self.oi_eff_long_q -= eff as u128;
            } else if eff < 0 {
                self.oi_eff_short_q -= eff.unsigned_abs();
            }

            // Zero position
            self.set_position_basis_q(i, 0);
            self.accounts[i].adl_a_basis = 1_000_000u128; // ADL_ONE
            self.accounts[i].adl_k_snap = 0;
            self.accounts[i].adl_epoch_snap = 0;
        }

        // Step 2: Settle losses from principal (senior to fees)
        self.settle_losses(i);

        // Step 3: Absorb any remaining flat negative PnL
        self.resolve_flat_negative(i);

        // Step 3b: Realize recurring maintenance fees (spec §8.2).
        // After losses and flat-negative absorption, matching touch_account_full
        // ordering where fees are junior to trading losses.
        self.settle_maintenance_fee_internal(i, self.current_slot)?;

        // Step 4: Convert positive PnL to capital (bypass warmup for resolved market).
        // Uses the same release-then-haircut order as do_profit_conversion and
        // convert_released_pnl. Sequential closers see progressively larger
        // pnl_matured_pos_tot denominators, which is the same behavior as normal
        // sequential profit conversion — this is inherent to the haircut model,
        // not a force_close-specific issue.
        if self.accounts[i].pnl.get() > 0 {
            // Release all reserves unconditionally (bypass warmup)
            self.set_reserved_pnl(i, 0);
            // Convert using post-release haircut
            let released = self.released_pos(i);
            if released > 0 {
                let (h_num, h_den) = self.haircut_ratio();
                let y = if h_den == 0 {
                    released
                } else {
                    wide_mul_div_floor_u128(released, h_num, h_den)
                };
                self.consume_released_pnl(i, released);
                let new_cap = add_u128(self.accounts[i].capital.get(), y);
                self.set_capital(i, new_cap);
            }
        }

        // Step 5: Sweep fee debt from capital
        self.fee_debt_sweep(i);

        // Step 6: Forgive any remaining fee debt
        if self.accounts[i].fee_credits.get() < 0 {
            self.accounts[i].fee_credits = I128::ZERO;
        }

        // Step 7: Return capital and free slot
        let capital = self.accounts[i].capital;
        if capital > self.vault {
            return Err(RiskError::InsufficientBalance);
        }
        self.vault = self.vault - capital;
        self.set_capital(i, 0);

        self.free_slot(idx);

        Ok(capital.get())
    }

    /// Free an account slot (internal helper).
    /// Clears the account, bitmap, and returns slot to freelist.
    /// Caller must ensure the account is safe to free (no capital, no positive pnl, etc).
    fn free_slot(&mut self, idx: u16) {
        self.accounts[idx as usize] = empty_account();
        self.clear_used(idx as usize);
        self.next_free[idx as usize] = self.free_head;
        self.free_head = idx;
        self.num_used_accounts = self.num_used_accounts.saturating_sub(1);
    }

    /// Garbage collect dust accounts.
    ///
    /// A "dust account" is a slot that can never pay out anything:
    /// - position_size == 0
    /// - capital == 0
    /// - reserved_pnl == 0
    /// - pnl <= 0
    ///
    /// Any remaining negative PnL is socialized via ADL waterfall before freeing.
    /// No token transfers occur - this is purely internal bookkeeping cleanup.
    ///
    /// Called at end of keeper_crank after liquidation/settlement has already run.
    ///
    /// Returns the number of accounts closed.
    pub fn garbage_collect_dust(&mut self) -> u32 {
        // Collect dust candidates: accounts with zero position, capital, reserved, and non-positive pnl
        let mut to_free: [u16; GC_CLOSE_BUDGET as usize] = [0; GC_CLOSE_BUDGET as usize];
        let mut num_to_free = 0usize;

        // Scan up to ACCOUNTS_PER_CRANK slots, capped to MAX_ACCOUNTS
        let max_scan = (ACCOUNTS_PER_CRANK as usize).min(MAX_ACCOUNTS);
        let start = self.gc_cursor as usize;

        let mut scanned: usize = 0;
        for offset in 0..max_scan {
            // Budget check
            if num_to_free >= GC_CLOSE_BUDGET as usize {
                break;
            }
            scanned = offset + 1;

            let idx = (start + offset) & ACCOUNT_IDX_MASK;

            // Check if slot is used via bitmap
            let block = idx >> 6;
            let bit = idx & 63;
            if (self.used[block] & (1u64 << bit)) == 0 {
                continue;
            }

            // NEVER garbage collect LP accounts - they are essential for market operation
            if self.accounts[idx].is_lp() {
                continue;
            }

            // Best-effort fee settle so accounts with tiny capital get drained in THIS sweep.
            let _ =
                self.settle_maintenance_fee_best_effort_for_crank(idx as u16, self.current_slot);

            // Dust predicate: must have zero position, reserved, and zero pnl.
            // Capital: reclaim when C_i == 0 OR 0 < C_i < MIN_INITIAL_DEPOSIT (spec §2.6).
            {
                let account = &self.accounts[idx];
                if !account.position_size.is_zero() {
                    continue;
                }
                // Spec §2.6: skip only if C_i >= MIN_INITIAL_DEPOSIT (not just nonzero).
                // Dust capital (0 < C_i < min_initial_deposit) is swept to insurance below.
                // Use new_account_fee as the minimum deposit floor (dcccrypto equivalent of min_initial_deposit)
                if account.capital.get() >= self.params.new_account_fee.get()
                    && !account.capital.is_zero()
                {
                    continue;
                }
                if account.reserved_pnl != 0 {
                    continue;
                }
                // Spec §2.6 requires PNL_i == 0 as a reclamation precondition.
                // Accounts with PNL != 0 need touch_account_full → §7.3 first.
                if account.pnl.get() != 0 {
                    continue;
                }
            }

            // Sweep dust capital into insurance fund before freeing (spec §2.6)
            let dust_cap = self.accounts[idx].capital.get();
            if dust_cap > 0 {
                self.set_capital(idx, 0);
                self.insurance_fund.balance =
                    U128::new(add_u128(self.insurance_fund.balance.get(), dust_cap));
            }

            // If flat, funding is irrelevant — snap to global so dust can be collected.
            // Position size is already confirmed zero above, so no unsettled funding value.
            if self.accounts[idx].funding_index != self.funding_index_qpb_e6 {
                self.accounts[idx].funding_index = self.funding_index_qpb_e6;
            }

            // Forgive uncollectible fee debt (spec §2.6)
            if self.accounts[idx].fee_credits.is_negative() {
                self.accounts[idx].fee_credits = I128::ZERO;
            }

            // Queue for freeing
            to_free[num_to_free] = idx as u16;
            num_to_free += 1;
        }

        // Advance cursor by actual number of offsets scanned, not max_scan.
        // Prevents skipping unscanned accounts on early budget break.
        self.gc_cursor = ((start + scanned) & ACCOUNT_IDX_MASK) as u16;

        // Free all collected dust accounts
        for slot in to_free.iter().take(num_to_free) {
            self.free_slot(*slot);
        }

        num_to_free as u32
    }

    // ========================================
    // Keeper Crank
    // ========================================

    /// Check if a fresh crank is required before state-changing operations.
    /// Returns Err if the crank is stale (too old).
    pub fn require_fresh_crank(&self, now_slot: u64) -> Result<()> {
        if now_slot.saturating_sub(self.last_crank_slot) > self.max_crank_staleness_slots {
            return Err(RiskError::Unauthorized); // NeedsCrank
        }
        Ok(())
    }

    /// Check if a full sweep started recently.
    /// For risk-increasing ops, we require a sweep to have STARTED recently.
    /// The priority-liquidation phase runs every crank, so once a sweep starts,
    /// the worst accounts are immediately addressed.
    pub fn require_recent_full_sweep(&self, now_slot: u64) -> Result<()> {
        if now_slot.saturating_sub(self.last_full_sweep_start_slot) > self.max_crank_staleness_slots
        {
            return Err(RiskError::Unauthorized); // SweepStale
        }
        Ok(())
    }

    /// Check if force-realize mode is active (insurance at or below threshold).
    /// When active, keeper_crank will run windowed force-realize steps.
    #[inline]
    fn force_realize_active(&self) -> bool {
        self.insurance_fund.balance <= self.params.risk_reduction_threshold
    }

    /// Keeper crank entrypoint - advances global state and performs maintenance.
    ///
    /// Returns CrankOutcome with flags indicating what happened.
    ///
    /// Behavior:
    /// 1. Accrue funding
    /// 2. Advance last_crank_slot if now_slot > last_crank_slot
    /// 3. Settle maintenance fees for caller (50% discount)
    /// 4. Process up to ACCOUNTS_PER_CRANK occupied accounts:
    ///    - Liquidation (if not in force-realize mode)
    ///    - Force-realize (if insurance at/below threshold)
    ///    - Socialization (haircut profits to cover losses)
    ///    - LP max tracking
    /// 5. Detect and finalize full sweep completion
    ///
    /// This is the single permissionless "do-the-right-thing" entrypoint.
    /// - Always attempts caller's maintenance settle with 50% discount (best-effort)
    /// - Only advances last_crank_slot when now_slot > last_crank_slot
    /// - Returns last_cursor: the index where this crank stopped
    /// - Returns sweep_complete: true if this crank completed a full sweep
    ///
    /// When the system has fewer than ACCOUNTS_PER_CRANK accounts, one crank
    /// covers all accounts and completes a full sweep.
    pub fn keeper_crank(
        &mut self,
        now_slot: u64,
        oracle_price: u64,
        ordered_candidates: &[(u16, Option<LiquidationPolicy>)],
        max_revalidations: u16,
        funding_rate: i64,
    ) -> Result<CrankOutcome> {
        Self::validate_funding_rate(funding_rate)?;

        // Validate oracle price bounds (prevents overflow in mark_pnl calculations)
        if oracle_price == 0 || oracle_price > MAX_ORACLE_PRICE {
            return Err(RiskError::Overflow);
        }

        // Create instruction context for deferred resets
        let mut ctx = InstructionContext {
            pending_reset_long: false,
            pending_reset_short: false,
        };

        // Accrue ADL market state.
        // Track silent failures for observability (GH#1931 / PERC-8296).
        let adl_accrue_failures: u8 = if self.accrue_market_to(now_slot, oracle_price).is_err() {
            1
        } else {
            0
        };

        // Update current_slot so warmup/bookkeeping progresses consistently
        self.current_slot = now_slot;

        // Check if we're advancing the global crank slot
        let advanced = now_slot > self.last_crank_slot;
        if advanced {
            self.last_crank_slot = now_slot;
        }

        let mut num_liquidations: u32 = 0;
        let mut num_liq_errors: u16 = 0;
        let mut liq_budget = LIQ_BUDGET_PER_CRANK;

        if !ordered_candidates.is_empty() {
            // === Two-phase keeper model: process ordered candidates ===
            let limit = core::cmp::min(ordered_candidates.len(), max_revalidations as usize);
            for &(candidate_idx, ref _policy) in &ordered_candidates[..limit] {
                let cidx = candidate_idx as usize;
                if cidx >= MAX_ACCOUNTS || !self.is_used(cidx) {
                    continue;
                }

                // Phase 1: settle side effects and warmup
                self.advance_profit_warmup(cidx);
                let _ = self.settle_side_effects(cidx); // best-effort
                self.settle_losses(cidx);

                let eff = self.effective_pos_q(cidx);
                let pnl = self.accounts[cidx].pnl.get();
                if eff == 0 && pnl < 0 {
                    self.resolve_flat_negative(cidx);
                }

                let _ = self.settle_maintenance_fee_internal(cidx, now_slot);

                let eff2 = self.effective_pos_q(cidx);
                if eff2 == 0 {
                    self.do_profit_conversion(cidx);
                }

                self.fee_debt_sweep(cidx);

                // Phase 2: liquidation (only when no pending resets)
                if !ctx.pending_reset_long
                    && !ctx.pending_reset_short
                    && liq_budget > 0
                    && !self.accounts[cidx].position_size.is_zero()
                {
                    match self.liquidate_at_oracle(candidate_idx, now_slot, oracle_price) {
                        Ok(true) => {
                            num_liquidations += 1;
                            liq_budget = liq_budget.saturating_sub(1);
                        }
                        Ok(false) => {}
                        Err(_) => {
                            num_liq_errors += 1;
                        }
                    }
                }
            }
        } else {
            // === Fallback: cursor-based scan (backward compat) ===
            let starting_new_sweep = self.crank_cursor == self.sweep_start_idx;
            if starting_new_sweep {
                self.last_full_sweep_start_slot = now_slot;
                self.lp_max_abs_sweep = U128::ZERO;
            }

            // Accrue funding using STORED rate (anti-retroactivity)
            self.accrue_funding(now_slot, oracle_price)?;
            self.set_funding_rate_for_next_interval(funding_rate)?;

            let force_realize_active = self.force_realize_active();
            let mut force_realize_closed: u16 = 0;
            let mut force_realize_errors: u16 = 0;
            let mut sweep_complete = false;
            let mut accounts_processed: u16 = 0;
            let mut force_realize_budget = FORCE_REALIZE_BUDGET_PER_CRANK;

            let mut idx = self.crank_cursor as usize;
            let mut slots_scanned: usize = 0;

            while accounts_processed < ACCOUNTS_PER_CRANK && slots_scanned < MAX_ACCOUNTS {
                slots_scanned += 1;
                let block = idx >> 6;
                let bit = idx & 63;
                let is_occupied = (self.used[block] & (1u64 << bit)) != 0;

                if is_occupied {
                    accounts_processed += 1;
                    let _ = self.settle_maintenance_fee_best_effort_for_crank(idx as u16, now_slot);
                    let _ = self.touch_account(idx as u16);
                    self.settle_warmup_to_capital_for_crank(idx as u16);

                    if !force_realize_active && liq_budget > 0 {
                        if !self.accounts[idx].position_size.is_zero() {
                            match self.liquidate_at_oracle(idx as u16, now_slot, oracle_price) {
                                Ok(true) => {
                                    num_liquidations += 1;
                                    liq_budget = liq_budget.saturating_sub(1);
                                }
                                Ok(false) => {}
                                Err(_) => {
                                    num_liq_errors += 1;
                                }
                            }
                        }
                        if !self.accounts[idx].position_size.is_zero() {
                            let equity = self
                                .account_equity_mtm_at_oracle(&self.accounts[idx], oracle_price);
                            let abs_pos = self.accounts[idx].position_size.unsigned_abs();
                            let is_dust = abs_pos < self.params.min_liquidation_abs.get();
                            if equity == 0 || is_dust {
                                let _ = self.touch_account_for_liquidation(
                                    idx as u16,
                                    now_slot,
                                    oracle_price,
                                );
                                let _ = self.oracle_close_position_core(idx as u16, oracle_price);
                                self.lifetime_force_realize_closes =
                                    self.lifetime_force_realize_closes.saturating_add(1);
                            }
                        }
                    }

                    if force_realize_active
                        && force_realize_budget > 0
                        && !self.accounts[idx].position_size.is_zero()
                    {
                        if self
                            .touch_account_for_force_realize(idx as u16, now_slot, oracle_price)
                            .is_ok()
                        {
                            if self
                                .oracle_close_position_core(idx as u16, oracle_price)
                                .is_ok()
                            {
                                force_realize_closed += 1;
                                force_realize_budget = force_realize_budget.saturating_sub(1);
                                self.lifetime_force_realize_closes =
                                    self.lifetime_force_realize_closes.saturating_add(1);
                            } else {
                                force_realize_errors += 1;
                            }
                        } else {
                            force_realize_errors += 1;
                        }
                    }

                    if self.accounts[idx].is_lp() {
                        let abs_pos = self.accounts[idx].position_size.unsigned_abs();
                        self.lp_max_abs_sweep = self.lp_max_abs_sweep.max(U128::new(abs_pos));
                    }
                }

                idx = (idx + 1) & ACCOUNT_IDX_MASK;
                if idx == self.sweep_start_idx as usize && slots_scanned > 0 {
                    sweep_complete = true;
                    break;
                }
            }

            self.crank_cursor = idx as u16;
            if sweep_complete {
                self.last_full_sweep_completed_slot = now_slot;
                self.lp_max_abs = self.lp_max_abs_sweep;
                self.sweep_start_idx = self.crank_cursor;
            }

            // End-of-instruction lifecycle for fallback path
            self.run_end_of_instruction_lifecycle(&mut ctx, funding_rate)?;

            let num_gc_closed = self.garbage_collect_dust();
            let force_realize_needed = self.force_realize_active();
            let panic_needed = false;

            return Ok(CrankOutcome {
                advanced,
                slots_forgiven: 0,
                caller_settle_ok: true,
                force_realize_needed,
                panic_needed,
                num_liquidations,
                num_liq_errors,
                num_gc_closed,
                force_realize_closed,
                force_realize_errors,
                last_cursor: self.crank_cursor,
                sweep_complete,
                adl_accrue_failures,
            });
        }

        // End-of-instruction lifecycle for two-phase path (single call — funding_rate from caller).
        // Previously there was a second call with 0i64 here (stale copy) — removed to prevent
        // overwriting funding_rate_bps_per_slot_last with zero (security issue: LOW).
        self.run_end_of_instruction_lifecycle(&mut ctx, funding_rate)?;

        let num_gc_closed = self.garbage_collect_dust();
        let force_realize_needed = self.force_realize_active();
        let panic_needed = false;

        Ok(CrankOutcome {
            advanced,
            slots_forgiven: 0,
            caller_settle_ok: true,
            force_realize_needed,
            panic_needed,
            num_liquidations,
            num_liq_errors,
            num_gc_closed,
            force_realize_closed: 0,
            force_realize_errors: 0,
            last_cursor: self.crank_cursor,
            sweep_complete: false,
            adl_accrue_failures,
        })
    }

    // ========================================
    // Liquidation
    // ========================================

    /// Compute mark PnL for a position at oracle price (pure helper, no side effects).
    /// Returns the PnL from closing the position at oracle price.
    /// - Longs: profit when oracle > entry
    /// - Shorts: profit when entry > oracle
    pub fn mark_pnl_for_position(pos: i128, entry: u64, oracle: u64) -> Result<i128> {
        if pos == 0 {
            return Ok(0);
        }

        let abs_pos = saturating_abs_i128(pos) as u128;

        let diff: i128 = if pos > 0 {
            // Long: profit when oracle > entry
            (oracle as i128).saturating_sub(entry as i128)
        } else {
            // Short: profit when entry > oracle
            (entry as i128).saturating_sub(oracle as i128)
        };

        // Coin-margined PnL: mark_pnl = diff * abs_pos / oracle
        // Dividing by oracle (instead of 1e6) gives PnL denominated in the
        // collateral token, which is correct for coin-margined perpetuals.
        diff.checked_mul(abs_pos as i128)
            .ok_or(RiskError::Overflow)?
            .checked_div(oracle as i128)
            .ok_or(RiskError::Overflow)
    }

    /// Compute how much position to close for liquidation (closed-form, single-pass).
    ///
    /// Returns (close_abs, is_full_close) where:
    /// - close_abs = absolute position size to close
    /// - is_full_close = true if this is a full position close (including dust kill-switch)
    ///
    /// ## Algorithm:
    /// 1. Compute target_bps = maintenance_margin_bps + liquidation_buffer_bps
    /// 2. Compute max safe remaining position: abs_pos_safe_max = floor(E_mtm * 10_000 * 1_000_000 / (P * target_bps))
    /// 3. close_abs = abs_pos - abs_pos_safe_max
    /// 4. If remaining position < min_liquidation_abs, do full close (dust kill-switch)
    ///
    /// Uses MTM equity (capital + realized_pnl + mark_pnl) for correct risk calculation.
    /// This is deterministic, requires no iteration, and guarantees single-pass liquidation.
    pub fn compute_liquidation_close_amount(
        &self,
        account: &Account,
        oracle_price: u64,
    ) -> (u128, bool) {
        let abs_pos = saturating_abs_i128(account.position_size.get()) as u128;
        if abs_pos == 0 {
            return (0, false);
        }

        // MTM equity at oracle price (fail-safe: overflow returns 0 = full liquidation)
        let equity = self.account_equity_mtm_at_oracle(account, oracle_price);

        // Target margin = maintenance + buffer (in basis points)
        let target_bps = self
            .params
            .maintenance_margin_bps
            .saturating_add(self.params.liquidation_buffer_bps);

        // Maximum safe remaining position (floor-safe calculation)
        // abs_pos_safe_max = floor(equity * 10_000 * 1_000_000 / (oracle_price * target_bps))
        // Rearranged to avoid intermediate overflow:
        // abs_pos_safe_max = floor(equity * 10_000_000_000 / (oracle_price * target_bps))
        let numerator = mul_u128(equity, 10_000_000_000);
        let denominator = mul_u128(oracle_price as u128, target_bps as u128);

        let mut abs_pos_safe_max = if denominator == 0 {
            0 // Edge case: full liquidation if no denominator
        } else {
            numerator / denominator
        };

        // Clamp to current position (can't have safe max > actual position)
        abs_pos_safe_max = core::cmp::min(abs_pos_safe_max, abs_pos);

        // Conservative rounding guard: subtract 1 unit to ensure we close slightly more
        // than mathematically required. This guarantees post-liquidation account is
        // strictly on the safe side of the inequality despite integer truncation.
        abs_pos_safe_max = abs_pos_safe_max.saturating_sub(1);

        // Required close amount
        let close_abs = abs_pos.saturating_sub(abs_pos_safe_max);

        // Dust kill-switch: if remaining position would be below min, do full close
        let remaining = abs_pos.saturating_sub(close_abs);
        if remaining < self.params.min_liquidation_abs.get() {
            return (abs_pos, true); // Full close
        }

        (close_abs, close_abs == abs_pos)
    }

    /// Core helper for closing a SLICE of a position at oracle price (partial liquidation).
    ///
    /// Similar to oracle_close_position_core but:
    /// - Only closes `close_abs` units of position (not the entire position)
    /// - Computes proportional mark_pnl for the closed slice
    /// - Entry price remains unchanged (correct for same-direction partial reduction)
    ///
    /// ## PnL Routing (same invariant as full close):
    /// - mark_pnl > 0 (profit) → backed by haircut ratio h (no ADL needed)
    /// - mark_pnl <= 0 (loss) → realized via settle_warmup_to_capital (capital path)
    /// - Residual negative PnL (capital exhausted) → written off via set_pnl(i, 0) (spec §6.1)
    ///
    /// ASSUMES: Caller has already called touch_account_full() on this account.
    fn oracle_close_position_slice_core(
        &mut self,
        idx: u16,
        oracle_price: u64,
        close_abs: u128,
    ) -> Result<ClosedOutcome> {
        let pos = self.accounts[idx as usize].position_size.get();
        let current_abs_pos = saturating_abs_i128(pos) as u128;

        if close_abs == 0 || current_abs_pos == 0 {
            return Ok(ClosedOutcome {
                abs_pos: 0,
                mark_pnl: 0,
                cap_before: self.accounts[idx as usize].capital.get(),
                cap_after: self.accounts[idx as usize].capital.get(),
                position_was_closed: false,
            });
        }

        if close_abs >= current_abs_pos {
            return self.oracle_close_position_core(idx, oracle_price);
        }

        let entry = self.accounts[idx as usize].entry_price;
        let cap_before = self.accounts[idx as usize].capital.get();

        let diff: i128 = if pos > 0 {
            (oracle_price as i128).saturating_sub(entry as i128)
        } else {
            (entry as i128).saturating_sub(oracle_price as i128)
        };

        let mark_pnl = match diff
            .checked_mul(close_abs as i128)
            .and_then(|v| v.checked_div(oracle_price as i128))
        {
            Some(pnl) => pnl,
            None => -u128_to_i128_clamped(cap_before),
        };

        // Apply mark PnL via set_pnl (maintains pnl_pos_tot aggregate)
        let new_pnl = self.accounts[idx as usize]
            .pnl
            .get()
            .saturating_add(mark_pnl);
        self.set_pnl(idx as usize, new_pnl);

        // Update position
        let new_abs_pos = current_abs_pos.saturating_sub(close_abs);
        self.accounts[idx as usize].position_size = if pos > 0 {
            I128::new(new_abs_pos as i128)
        } else {
            I128::new(-(new_abs_pos as i128))
        };

        // Update OI
        self.total_open_interest -= close_abs;
        // PERC-298: maintain per-side OI
        if pos > 0 {
            self.long_oi = self.long_oi.saturating_sub(close_abs);
        } else {
            self.short_oi = self.short_oi.saturating_sub(close_abs);
        }

        // Update LP aggregates if LP
        if self.accounts[idx as usize].is_lp() {
            let new_pos = self.accounts[idx as usize].position_size.get();
            self.net_lp_pos = self.net_lp_pos - pos + new_pos;
            self.lp_sum_abs -= close_abs;
        }

        // Settle warmup (loss settlement + profit conversion per spec §6)
        self.settle_warmup_to_capital(idx)?;

        // Write off residual negative PnL (capital exhausted) per spec §6.1
        if self.accounts[idx as usize].pnl.is_negative() {
            self.set_pnl(idx as usize, 0);
        }

        let cap_after = self.accounts[idx as usize].capital.get();

        Ok(ClosedOutcome {
            abs_pos: close_abs,
            mark_pnl,
            cap_before,
            cap_after,
            position_was_closed: true,
        })
    }

    /// Core helper for oracle-price full position close (spec §6).
    ///
    /// Applies mark PnL, closes position, settles warmup, writes off unpayable loss.
    /// No ADL needed — undercollateralization is reflected via haircut ratio h.
    ///
    /// ASSUMES: Caller has already called touch_account_full() on this account.
    fn oracle_close_position_core(&mut self, idx: u16, oracle_price: u64) -> Result<ClosedOutcome> {
        if self.accounts[idx as usize].position_size.is_zero() {
            return Ok(ClosedOutcome {
                abs_pos: 0,
                mark_pnl: 0,
                cap_before: self.accounts[idx as usize].capital.get(),
                cap_after: self.accounts[idx as usize].capital.get(),
                position_was_closed: false,
            });
        }

        let pos = self.accounts[idx as usize].position_size.get();
        let abs_pos = saturating_abs_i128(pos) as u128;
        let entry = self.accounts[idx as usize].entry_price;
        let cap_before = self.accounts[idx as usize].capital.get();

        let mark_pnl = match Self::mark_pnl_for_position(pos, entry, oracle_price) {
            Ok(pnl) => pnl,
            Err(_) => -u128_to_i128_clamped(cap_before),
        };

        // Apply mark PnL via set_pnl (maintains pnl_pos_tot aggregate)
        let new_pnl = self.accounts[idx as usize]
            .pnl
            .get()
            .saturating_add(mark_pnl);
        self.set_pnl(idx as usize, new_pnl);

        // Close position
        self.accounts[idx as usize].position_size = I128::ZERO;
        self.accounts[idx as usize].entry_price = oracle_price;

        // Update OI
        self.total_open_interest -= abs_pos;
        // PERC-298: maintain per-side OI
        if pos > 0 {
            self.long_oi = self.long_oi.saturating_sub(abs_pos);
        } else {
            self.short_oi = self.short_oi.saturating_sub(abs_pos);
        }

        // Update LP aggregates if LP
        if self.accounts[idx as usize].is_lp() {
            self.net_lp_pos -= pos;
            self.lp_sum_abs -= abs_pos;
        }

        // Settle warmup (loss settlement + profit conversion per spec §6)
        self.settle_warmup_to_capital(idx)?;

        // Write off residual negative PnL (capital exhausted) per spec §6.1
        if self.accounts[idx as usize].pnl.is_negative() {
            self.set_pnl(idx as usize, 0);
        }

        let cap_after = self.accounts[idx as usize].capital.get();

        Ok(ClosedOutcome {
            abs_pos,
            mark_pnl,
            cap_before,
            cap_after,
            position_was_closed: true,
        })
    }

    /// Liquidate a single account at oracle price if below maintenance margin.
    ///
    /// Returns Ok(true) if liquidation occurred, Ok(false) if not needed/possible.
    /// Per spec: close position, settle losses, write off unpayable PnL, charge fee.
    /// No ADL — haircut ratio h reflects any undercollateralization.
    pub fn liquidate_at_oracle(
        &mut self,
        idx: u16,
        now_slot: u64,
        oracle_price: u64,
    ) -> Result<bool> {
        self.current_slot = now_slot;

        if (idx as usize) >= MAX_ACCOUNTS || !self.is_used(idx as usize) {
            return Ok(false);
        }

        if oracle_price == 0 || oracle_price > MAX_ORACLE_PRICE {
            return Err(RiskError::Overflow);
        }

        if self.accounts[idx as usize].position_size.is_zero() {
            return Ok(false);
        }

        // Settle funding + mark-to-market + best-effort fees
        self.touch_account_for_liquidation(idx, now_slot, oracle_price)?;

        let account = &self.accounts[idx as usize];
        if self.is_above_maintenance_margin_mtm(account, oracle_price) {
            return Ok(false);
        }

        let (close_abs, is_full_close) =
            self.compute_liquidation_close_amount(account, oracle_price);

        if close_abs == 0 {
            return Ok(false);
        }

        // Close position (no ADL — losses written off in close helper)
        let mut outcome = if is_full_close {
            self.oracle_close_position_core(idx, oracle_price)?
        } else {
            match self.oracle_close_position_slice_core(idx, oracle_price, close_abs) {
                Ok(r) => r,
                Err(RiskError::Overflow) => self.oracle_close_position_core(idx, oracle_price)?,
                Err(e) => return Err(e),
            }
        };

        if !outcome.position_was_closed {
            return Ok(false);
        }

        // Safety check: if position remains and still below target, full close
        if !self.accounts[idx as usize].position_size.is_zero() {
            let target_bps = self
                .params
                .maintenance_margin_bps
                .saturating_add(self.params.liquidation_buffer_bps);
            if !self.is_above_margin_bps_mtm(&self.accounts[idx as usize], oracle_price, target_bps)
            {
                let fallback = self.oracle_close_position_core(idx, oracle_price)?;
                if fallback.position_was_closed {
                    outcome.abs_pos = outcome.abs_pos.saturating_add(fallback.abs_pos);
                }
            }
        }

        // Charge liquidation fee (from remaining capital → insurance)
        // Use ceiling division for consistency with trade fees
        let notional = mul_u128(outcome.abs_pos, oracle_price as u128) / 1_000_000;
        let fee_raw = if notional > 0 && self.params.liquidation_fee_bps > 0 {
            mul_u128(notional, self.params.liquidation_fee_bps as u128).div_ceil(10_000)
        } else {
            0
        };
        let fee = core::cmp::min(fee_raw, self.params.liquidation_fee_cap.get());
        let account_capital = self.accounts[idx as usize].capital.get();
        let pay = core::cmp::min(fee, account_capital);

        self.set_capital(idx as usize, account_capital.saturating_sub(pay));
        self.insurance_fund.balance = self
            .insurance_fund
            .balance
            .saturating_add_u128(U128::new(pay));
        self.insurance_fund.fee_revenue = self
            .insurance_fund
            .fee_revenue
            .saturating_add_u128(U128::new(pay));

        self.lifetime_liquidations = self.lifetime_liquidations.saturating_add(1);

        Ok(true)
    }

    // ========================================
    // Mark-Price Liquidation + Partial (PERC-122)
    // ========================================

    /// Liquidation with mark-price trigger and partial liquidation.
    ///
    /// - Trigger: check margin at mark_price_e6 (prevents oracle manipulation)
    /// - Settle: close position at oracle_price (actual market price)
    /// - Partial: close partial_liquidation_bps/10_000 of position with cooldown
    pub fn liquidate_with_mark_price(
        &mut self,
        idx: u16,
        now_slot: u64,
        oracle_price: u64,
    ) -> Result<bool> {
        if !self.params.use_mark_price_for_liquidation || self.mark_price_e6 == 0 {
            return self.liquidate_at_oracle(idx, now_slot, oracle_price);
        }

        self.current_slot = now_slot;

        if (idx as usize) >= MAX_ACCOUNTS || !self.is_used(idx as usize) {
            return Ok(false);
        }
        if oracle_price == 0 || oracle_price > MAX_ORACLE_PRICE {
            return Err(RiskError::Overflow);
        }
        if self.accounts[idx as usize].position_size.is_zero() {
            return Ok(false);
        }

        // Settle at oracle price
        self.touch_account_for_liquidation(idx, now_slot, oracle_price)?;

        // TRIGGER at mark price (not oracle)
        let mark_price = self.mark_price_e6;
        if self.is_above_maintenance_margin_mtm(&self.accounts[idx as usize], mark_price) {
            return Ok(false);
        }

        // Partial liquidation with cooldown
        let cooldown = self.params.partial_liquidation_cooldown_slots;
        let last_partial = self.accounts[idx as usize].last_partial_liquidation_slot;
        let can_partial = self.params.partial_liquidation_bps > 0
            && (cooldown == 0 || now_slot.saturating_sub(last_partial) >= cooldown);

        let account = &self.accounts[idx as usize];
        let pos_abs = saturating_abs_i128(account.position_size.get()) as u128;

        let (close_abs, is_full_close) = if can_partial {
            let batch = mul_u128(pos_abs, self.params.partial_liquidation_bps as u128) / 10_000;
            let batch = batch.max(self.params.min_liquidation_abs.get());
            // Issue #650: guarantee liveness — integer division can round batch to 0
            // when pos_abs < 10_000 / partial_liquidation_bps. Without this guard,
            // close_abs == 0 causes an early return (Ok(false)) and the account is
            // never liquidated. Enforce a minimum of 1 unit so we always make progress.
            let batch = if pos_abs > 0 { batch.max(1) } else { batch };
            if batch >= pos_abs {
                (pos_abs, true)
            } else {
                (batch, false)
            }
        } else if cooldown > 0 && now_slot.saturating_sub(last_partial) < cooldown {
            // Issue #300: Cooldown not elapsed — check if critically underwater.
            // If margin ratio is below emergency threshold, bypass cooldown
            // and do full liquidation to prevent bad debt accumulation.
            let emergency_bps = self.params.effective_emergency_margin_bps();
            if !self.is_above_margin_bps_mtm(
                &self.accounts[idx as usize],
                mark_price,
                emergency_bps,
            ) {
                // Critically underwater — bypass cooldown, full liquidation
                (pos_abs, true)
            } else {
                return Ok(false); // Normal cooldown — account is not in emergency
            }
        } else {
            (pos_abs, true)
        };

        if close_abs == 0 {
            return Ok(false);
        }

        // SETTLE at oracle price
        let mut outcome = if is_full_close {
            self.oracle_close_position_core(idx, oracle_price)?
        } else {
            match self.oracle_close_position_slice_core(idx, oracle_price, close_abs) {
                Ok(r) => r,
                Err(RiskError::Overflow) => self.oracle_close_position_core(idx, oracle_price)?,
                Err(e) => return Err(e),
            }
        };

        if !outcome.position_was_closed {
            return Ok(false);
        }

        if !is_full_close {
            self.accounts[idx as usize].last_partial_liquidation_slot = now_slot;
        }

        // Safety: if still below target at mark price, full close
        if !self.accounts[idx as usize].position_size.is_zero() {
            let target_bps = self
                .params
                .maintenance_margin_bps
                .saturating_add(self.params.liquidation_buffer_bps);
            if !self.is_above_margin_bps_mtm(&self.accounts[idx as usize], mark_price, target_bps) {
                let fallback = self.oracle_close_position_core(idx, oracle_price)?;
                if fallback.position_was_closed {
                    outcome.abs_pos = outcome.abs_pos.saturating_add(fallback.abs_pos);
                }
            }
        }

        // Liquidation fee
        let notional = mul_u128(outcome.abs_pos, oracle_price as u128) / 1_000_000;
        let fee_raw = if notional > 0 && self.params.liquidation_fee_bps > 0 {
            mul_u128(notional, self.params.liquidation_fee_bps as u128).div_ceil(10_000)
        } else {
            0
        };
        let fee = core::cmp::min(fee_raw, self.params.liquidation_fee_cap.get());
        let acap = self.accounts[idx as usize].capital.get();
        let pay = core::cmp::min(fee, acap);

        self.set_capital(idx as usize, acap.saturating_sub(pay));
        self.insurance_fund.balance = self
            .insurance_fund
            .balance
            .saturating_add_u128(U128::new(pay));
        self.insurance_fund.fee_revenue = self
            .insurance_fund
            .fee_revenue
            .saturating_add_u128(U128::new(pay));
        self.lifetime_liquidations = self.lifetime_liquidations.saturating_add(1);

        Ok(true)
    }

    // ========================================
    // Warmup
    // ========================================

    /// Calculate withdrawable PNL for an account after warmup
    pub fn withdrawable_pnl(&self, account: &Account) -> u128 {
        // Only positive PNL can be withdrawn
        let positive_pnl = clamp_pos_i128(account.pnl.get());

        // Available = positive PNL (reserved_pnl repurposed as trade entry price)
        let available_pnl = positive_pnl;

        let effective_slot = self.current_slot;

        // Calculate elapsed slots
        let elapsed_slots = effective_slot.saturating_sub(account.warmup_started_at_slot);

        // Calculate warmed up cap: slope * elapsed_slots
        let warmed_up_cap = mul_u128(account.warmup_slope_per_step.get(), elapsed_slots as u128);

        // Return minimum of available and warmed up
        core::cmp::min(available_pnl, warmed_up_cap)
    }

    /// Update warmup slope for an account
    /// NOTE: No warmup rate cap (removed for simplicity)
    pub fn update_warmup_slope(&mut self, idx: u16) -> Result<()> {
        if !self.is_used(idx as usize) {
            return Err(RiskError::AccountNotFound);
        }

        let account = &mut self.accounts[idx as usize];

        // Calculate available gross PnL: AvailGross_i = max(PNL_i, 0) (spec §5)
        let positive_pnl = clamp_pos_i128(account.pnl.get());
        let avail_gross = positive_pnl;

        // Calculate slope: avail_gross / warmup_period
        // Ensure slope >= 1 when avail_gross > 0 to prevent "zero forever" bug
        let slope = if self.params.warmup_period_slots > 0 {
            let base = avail_gross / (self.params.warmup_period_slots as u128);
            if avail_gross > 0 {
                core::cmp::max(1, base)
            } else {
                0
            }
        } else {
            avail_gross // Instant warmup if period is 0
        };

        // Verify slope >= 1 when available PnL exists
        #[cfg(any(test, kani))]
        debug_assert!(
            slope >= 1 || avail_gross == 0,
            "Warmup slope bug: slope {} with avail_gross {}",
            slope,
            avail_gross
        );

        // Update slope
        account.warmup_slope_per_step = U128::new(slope);

        account.warmup_started_at_slot = self.current_slot;

        Ok(())
    }

    // ========================================
    // Funding
    // ========================================

    /// Accrue funding globally in O(1) using the stored rate (anti-retroactivity).
    ///
    /// This uses `funding_rate_bps_per_slot_last` - the rate in effect since `last_funding_slot`.
    /// The rate for the NEXT interval is set separately via `set_funding_rate_for_next_interval`.
    ///
    /// Anti-retroactivity guarantee: state changes at slot t can only affect funding for slots >= t.
    pub fn accrue_funding(&mut self, now_slot: u64, oracle_price: u64) -> Result<()> {
        let dt = now_slot.saturating_sub(self.last_funding_slot);
        if dt == 0 {
            return Ok(());
        }

        // Input validation to prevent overflow
        if oracle_price == 0 || oracle_price > MAX_ORACLE_PRICE {
            return Err(RiskError::Overflow);
        }

        // Use the STORED rate (anti-retroactivity: rate was set at start of interval)
        // If frozen, use the snapshot rate (no drift from external rate changes)
        let funding_rate = if self.funding_frozen {
            self.funding_frozen_rate_snapshot
        } else {
            self.funding_rate_bps_per_slot_last
        };

        // Cap funding rate at 10000 bps (100%) per slot as sanity bound
        // Real-world funding rates should be much smaller (typically < 1 bps/slot)
        // Self-heal: if rate is corrupted (e.g., from a prior PushOraclePrice bug that wrote
        // a Unix timestamp into the funding rate field), reset to 0 and skip this accrual
        // rather than permanently bricking the market.
        if funding_rate.abs() > 10_000 {
            self.funding_rate_bps_per_slot_last = 0;
            self.last_funding_slot = now_slot;
            return Ok(());
        }

        if dt > 31_536_000 {
            return Err(RiskError::Overflow);
        }

        // Use checked math to prevent silent overflow
        let price = oracle_price as i128;
        let rate = funding_rate as i128;
        let dt_i = dt as i128;

        // ΔF = price × rate × dt / 10,000
        let delta = price
            .checked_mul(rate)
            .ok_or(RiskError::Overflow)?
            .checked_mul(dt_i)
            .ok_or(RiskError::Overflow)?
            .checked_div(10_000)
            .ok_or(RiskError::Overflow)?;

        self.funding_index_qpb_e6 = self
            .funding_index_qpb_e6
            .checked_add(delta)
            .ok_or(RiskError::Overflow)?;

        self.last_funding_slot = now_slot;
        Ok(())
    }

    /// Set the funding rate for the NEXT interval (anti-retroactivity).
    ///
    /// MUST be called AFTER `accrue_funding()` to ensure the old rate is applied to
    /// the elapsed interval before storing the new rate.
    ///
    /// This implements the "rate-change rule" from the spec: state changes at slot t
    /// can only affect funding for slots >= t.
    pub fn set_funding_rate_for_next_interval(&mut self, new_rate_bps_per_slot: i64) -> Result<()> {
        Self::validate_funding_rate(new_rate_bps_per_slot)?;
        // If funding is frozen, ignore rate updates (frozen rate snapshot is used instead)
        if self.funding_frozen {
            return Ok(());
        }
        self.funding_rate_bps_per_slot_last = new_rate_bps_per_slot;
        Ok(())
    }

    /// Convenience: Set rate then accrue in one call.
    ///
    /// This sets the rate for the interval being accrued, then accrues.
    /// For proper anti-retroactivity in production, the rate should be set at the
    /// START of an interval via `set_funding_rate_for_next_interval`, then accrued later.
    pub fn accrue_funding_with_rate(
        &mut self,
        now_slot: u64,
        oracle_price: u64,
        funding_rate_bps_per_slot: i64,
    ) -> Result<()> {
        self.set_funding_rate_for_next_interval(funding_rate_bps_per_slot)?;
        self.accrue_funding(now_slot, oracle_price)
    }

    // ========================================
    // Premium-based Funding (PERC-121)
    // ========================================

    /// Set the current mark price (EMA-smoothed). Called by wrapper after oracle update.
    pub fn set_mark_price(&mut self, mark_price_e6: u64) {
        self.mark_price_e6 = mark_price_e6;
    }

    /// Set mark price using blended formula: mark = blend(oracle, trade_twap).
    ///
    /// `oracle_weight_bps`: 10_000 = 100% oracle, 0 = 100% TWAP.
    /// Falls back to pure oracle when TWAP is zero (no trades yet).
    ///
    /// PERC-118: The trade TWAP acts as an on-chain "impact mid price" that
    /// anchors the mark to actual execution prices, making it resistant to
    /// oracle-only manipulation. The oracle component anchors mark to the
    /// external reference price.
    pub fn set_mark_price_blended(&mut self, oracle_e6: u64, oracle_weight_bps: u64) {
        let twap = self.trade_twap_e6;
        let mark = Self::compute_blended_mark_price(oracle_e6, twap, oracle_weight_bps);
        self.mark_price_e6 = mark;
    }

    /// Compute blended mark price from oracle and trade TWAP.
    ///
    /// Formula: mark = (oracle * w + twap * (10000 - w)) / 10000
    /// where w = oracle_weight_bps (clamped to 10000).
    ///
    /// If TWAP is zero (no trades), returns oracle price.
    /// If oracle is zero, returns TWAP (or 0 if both zero).
    pub fn compute_blended_mark_price(oracle_e6: u64, twap_e6: u64, oracle_weight_bps: u64) -> u64 {
        // Degenerate cases: use whichever is non-zero
        if twap_e6 == 0 {
            return oracle_e6;
        }
        if oracle_e6 == 0 {
            return twap_e6;
        }

        let w = oracle_weight_bps.min(10_000);
        let tw = 10_000u64.saturating_sub(w);

        // u128 arithmetic: max(oracle_e6) * 10_000 < 2^64 * 2^14 = 2^78, fits u128
        let blended = (oracle_e6 as u128)
            .saturating_mul(w as u128)
            .saturating_add((twap_e6 as u128).saturating_mul(tw as u128))
            / 10_000u128;

        blended.min(u64::MAX as u128) as u64
    }

    // ========================================
    // Trade TWAP Maintenance (PERC-118)
    // ========================================

    /// Update the trade execution price TWAP.
    ///
    /// Uses an exponential moving average weighted by both elapsed time and trade notional.
    /// Small trades (notional < MIN_TWAP_NOTIONAL) are ignored to prevent
    /// dust-trade manipulation of the TWAP. Trades up to FULL_WEIGHT_NOTIONAL
    /// receive proportionally increasing weight; trades at or above that cap
    /// receive full (1×) weight.
    ///
    /// Base alpha = 347 (out of 1_000_000) per slot ≈ 8-hour half-life at full weight.
    /// Effective alpha = min(base_alpha × dt_slots × notional_scale, 1_000_000)
    /// where notional_scale = min(notional, FULL_WEIGHT_NOTIONAL) / FULL_WEIGHT_NOTIONAL.
    ///
    /// This ensures large fills move the TWAP proportionally more than small fills,
    /// making the TWAP resistant to manipulation via many small trades.
    pub fn update_trade_twap(&mut self, exec_price_e6: u64, notional: u128, now_slot: u64) {
        // Minimum notional to affect TWAP (anti-dust: ~$1 at reasonable prices)
        const MIN_TWAP_NOTIONAL: u128 = 1_000_000; // 1e6 = $1 in e6 units
                                                   // Notional that receives full (1×) weight: $10,000 in e6 units.
                                                   // Trades below this are weighted proportionally (dust guard already removed <$1).
        const FULL_WEIGHT_NOTIONAL: u128 = 10_000_000_000; // 1e10 = $10,000 in e6 units

        if exec_price_e6 == 0 || notional < MIN_TWAP_NOTIONAL {
            return;
        }

        if self.trade_twap_e6 == 0 {
            // Bootstrap: first trade sets the TWAP directly
            self.trade_twap_e6 = exec_price_e6;
            self.twap_last_slot = now_slot;
            return;
        }

        // Time component: larger dt → faster convergence
        let dt = now_slot.saturating_sub(self.twap_last_slot).max(1);
        // TWAP_ALPHA_E6 = 347 per slot ≈ 8h half-life at full notional weight (matches oracle EMA)
        const TWAP_ALPHA_E6: u128 = 347;

        // Notional scale: 0..=1_000_000 (e6), capped at 1× for trades >= FULL_WEIGHT_NOTIONAL.
        // Smaller trades are weighted proportionally — a $100 trade gets 1% of full weight.
        let notional_scale_e6 =
            notional.min(FULL_WEIGHT_NOTIONAL) * 1_000_000 / FULL_WEIGHT_NOTIONAL;

        // eff_alpha = base_alpha_per_slot × dt × notional_scale, capped at 1.0
        let eff_alpha = (TWAP_ALPHA_E6
            .saturating_mul(dt as u128)
            .saturating_mul(notional_scale_e6)
            / 1_000_000u128)
            .min(1_000_000) as u64;
        let one_minus = 1_000_000u64.saturating_sub(eff_alpha);

        // EMA: new_twap = exec * alpha + old_twap * (1 - alpha)
        let new_twap = (exec_price_e6 as u128)
            .saturating_mul(eff_alpha as u128)
            .saturating_add((self.trade_twap_e6 as u128).saturating_mul(one_minus as u128))
            / 1_000_000u128;

        self.trade_twap_e6 = new_twap.min(u64::MAX as u128) as u64;
        self.twap_last_slot = now_slot;
    }

    /// Compute premium-based funding rate (bps per slot).
    ///
    /// premium = (mark_price - index_price) / index_price
    /// rate = premium * 10_000 / dampening_factor
    ///
    /// Sign convention: positive rate => longs pay shorts (mark > index means
    /// longs are paying a premium, so they should pay funding to push price down).
    ///
    /// Returns 0 if mark or index is zero, or if dampening is zero.
    pub fn compute_premium_funding_bps_per_slot(
        mark_price_e6: u64,
        index_price_e6: u64,
        dampening_e6: u64,
        max_bps_per_slot: i64,
    ) -> i64 {
        if mark_price_e6 == 0 || index_price_e6 == 0 || dampening_e6 == 0 {
            return 0;
        }

        // premium_bps = (mark - index) * 10_000 / index
        // Then divide by dampening to get per-slot rate.
        //
        // Use i128 to avoid overflow:
        // mark_price_e6 is u64 (~1.8e19 max), so i128 has plenty of room.
        let mark = mark_price_e6 as i128;
        let index = index_price_e6 as i128;
        let damp = dampening_e6 as i128;

        // premium_bps_e6 = (mark - index) * 10_000 * 1_000_000 / index
        // Then divide by dampening_e6:
        // rate = premium_bps_e6 / dampening_e6 / 1_000_000
        //      = (mark - index) * 10_000 / index / dampening_e6 * 1_000_000
        //
        // Simplify: rate = (mark - index) * 10_000_000_000 / (index * damp)
        let numerator = (mark - index)
            .checked_mul(10_000_000_000_i128) // 10_000 * 1e6
            .unwrap_or(0);
        let denominator = index.checked_mul(damp).unwrap_or(1).max(1); // never divide by 0

        let rate_unclamped = numerator / denominator;

        // Clamp to max_bps_per_slot
        let max_abs = max_bps_per_slot.unsigned_abs() as i128;
        let clamped = rate_unclamped.clamp(-max_abs, max_abs);

        clamped as i64
    }

    /// Compute the combined (blended) funding rate from inventory-based and premium-based.
    ///
    /// blended = (1 - weight) * inventory_rate + weight * premium_rate
    ///
    /// weight is `funding_premium_weight_bps` in basis points (0–10_000).
    pub fn compute_combined_funding_rate(
        inventory_rate_bps: i64,
        premium_rate_bps: i64,
        premium_weight_bps: u64,
    ) -> i64 {
        if premium_weight_bps == 0 {
            return inventory_rate_bps;
        }
        if premium_weight_bps >= 10_000 {
            return premium_rate_bps;
        }

        // blended = inv * (10000 - w) / 10000 + prem * w / 10000
        let inv = inventory_rate_bps as i128;
        let prem = premium_rate_bps as i128;
        let w = premium_weight_bps as i128;
        let inv_w = 10_000i128 - w;

        let blended = (inv * inv_w + prem * w) / 10_000;

        // Clamp to i64 range (should always fit given inputs are i64)
        blended.clamp(i64::MIN as i128, i64::MAX as i128) as i64
    }

    // ========================================
    // PERC-300: Adaptive Funding Rate
    // ========================================

    /// Compute adaptive funding rate based on OI skew.
    ///
    /// Formula: new_rate = clamp(prev_rate + skew * scale_bps, -max_bps, +max_bps)
    /// where skew = (long_oi - short_oi) / total_oi (range -1 to +1)
    ///
    /// When skew = 0 (balanced), rate unchanged (convergence at equilibrium).
    /// When longs dominate (skew > 0), rate increases (longs pay shorts).
    /// When shorts dominate (skew < 0), rate decreases (shorts pay longs).
    ///
    /// Returns the new adaptive funding rate in bps per slot.
    pub fn compute_adaptive_funding_rate(
        prev_rate_bps: i64,
        long_oi: u128,
        short_oi: u128,
        total_oi: u128,
        adaptive_scale_bps: u16,
        max_funding_bps: u64,
    ) -> i64 {
        // Always clamp to [-max_funding_bps, +max_funding_bps] — even when no
        // adjustment is possible. A previously-set rate may exceed current bounds
        // (e.g. if max was lowered), so skipping the clamp would let an out-of-
        // range value propagate and violate the invariant asserted by Kani proofs.
        let max = max_funding_bps as i128;
        let prev = prev_rate_bps as i128;

        if total_oi == 0 || adaptive_scale_bps == 0 {
            // No skew-delta to apply; just enforce bounds on the existing rate.
            return prev.clamp(-max, max) as i64;
        }

        // skew = (long_oi - short_oi) / total_oi, range [-1, 1]
        // delta = skew * adaptive_scale_bps
        // Using i128 to avoid overflow:
        // delta_bps = (long_oi - short_oi) * adaptive_scale_bps / total_oi
        let long = long_oi as i128;
        let short = short_oi as i128;
        let total = total_oi as i128;
        let scale = adaptive_scale_bps as i128;

        let delta_bps = ((long - short) * scale) / total;

        let new_rate = prev.saturating_add(delta_bps);

        // Clamp to [-max_funding_bps, +max_funding_bps]
        new_rate.clamp(-max, max) as i64
    }

    /// Freeze funding rate (emergency admin action).
    ///
    /// Snapshots the current rate so accrue_funding still applies it (no drift),
    /// but prevents any new rate computation from taking effect.
    pub fn freeze_funding(&mut self) -> Result<()> {
        if self.funding_frozen {
            return Err(RiskError::Overflow); // Already frozen
        }
        self.funding_frozen = true;
        self.funding_frozen_rate_snapshot = self.funding_rate_bps_per_slot_last;
        Ok(())
    }

    /// Unfreeze funding rate (admin).
    /// After unfreezing, the next crank can set a new rate.
    pub fn unfreeze_funding(&mut self) -> Result<()> {
        if !self.funding_frozen {
            return Err(RiskError::Overflow); // Not frozen
        }
        self.funding_frozen = false;
        self.funding_frozen_rate_snapshot = 0;
        Ok(())
    }

    /// Check whether funding is frozen.
    pub fn is_funding_frozen(&self) -> bool {
        self.funding_frozen
    }

    /// Settle funding for an account (lazy update).
    /// Uses set_pnl helper to maintain pnl_pos_tot aggregate (spec §4.2).
    /// Full funding accrual with combined rate (inventory + premium).
    ///
    /// 1. Respects settlement interval (batched accrual)
    /// 2. Accrues using the stored rate (anti-retroactivity)
    /// 3. Computes new combined rate for next interval
    /// 4. Stores the new rate
    pub fn accrue_funding_combined(
        &mut self,
        now_slot: u64,
        index_price_e6: u64,
        inventory_rate_bps: i64,
    ) -> Result<()> {
        let dt = now_slot.saturating_sub(self.last_funding_slot);
        let interval = self.params.funding_settlement_interval_slots;

        // If interval > 0, only accrue when enough slots have elapsed
        if interval > 0 && dt < interval {
            return Ok(());
        }

        // Step 1: Accrue using the STORED rate (anti-retroactivity)
        self.accrue_funding(now_slot, index_price_e6)?;

        // Step 2: Compute premium rate from current mark vs index
        let premium_rate = Self::compute_premium_funding_bps_per_slot(
            self.mark_price_e6,
            index_price_e6,
            self.params.funding_premium_dampening_e6,
            self.params.funding_premium_max_bps_per_slot,
        );

        // Step 3: Blend inventory and premium components
        let combined = Self::compute_combined_funding_rate(
            inventory_rate_bps,
            premium_rate,
            self.params.funding_premium_weight_bps,
        );

        // Step 4: Store for next interval (anti-retroactivity)
        self.set_funding_rate_for_next_interval(combined)?;

        Ok(())
    }

    fn settle_account_funding(&mut self, idx: usize) -> Result<()> {
        let global_fi = self.funding_index_qpb_e6;
        let account = &self.accounts[idx];
        let delta_f = global_fi
            .get()
            .checked_sub(account.funding_index.get())
            .ok_or(RiskError::Overflow)?;

        if delta_f != 0 && !account.position_size.is_zero() {
            // payment = position × ΔF / 1e6 (truncated toward zero for both payers and receivers)
            //
            // ZERO-SUM INVARIANT: For any two accounts with opposite positions (+delta / -delta),
            // raw_a = delta * ΔF and raw_b = -delta * ΔF. Rust integer division truncates toward
            // zero, so raw_b / 1e6 == -(raw_a / 1e6). The sum of both pnl changes is therefore
            // zero — funding is a pure transfer between counterparties, no capital created or
            // destroyed. Dust (remainder of raw mod 1e6) remains in the vault implicitly.
            //
            // Previously this used ceil(raw/1e6) for payers, which over-collected 1 quantum per
            // non-divisible payment and violated the Kani nightly_funding_zero_sum_across_accounts
            // proof (GitHub issue #909).
            let raw = account
                .position_size
                .get()
                .checked_mul(delta_f)
                .ok_or(RiskError::Overflow)?;

            // Symmetric truncation toward zero — preserves zero-sum invariant (PERC-492 / #909)
            let payment = raw.checked_div(1_000_000).ok_or(RiskError::Overflow)?;

            // Longs pay when funding positive: pnl -= payment
            // Use set_pnl helper to maintain pnl_pos_tot aggregate (spec §4.2)
            let new_pnl = self.accounts[idx]
                .pnl
                .get()
                .checked_sub(payment)
                .ok_or(RiskError::Overflow)?;
            self.set_pnl(idx, new_pnl);
        }

        self.accounts[idx].funding_index = global_fi;
        Ok(())
    }

    /// Touch an account (settle funding before operations)
    pub fn touch_account(&mut self, idx: u16) -> Result<()> {
        if !self.is_used(idx as usize) {
            return Err(RiskError::AccountNotFound);
        }

        self.settle_account_funding(idx as usize)
    }

    /// Settle mark-to-market PnL to the current oracle price (variation margin).
    ///
    /// This realizes all unrealized PnL at the given oracle price and resets
    /// entry_price = oracle_price. After calling this, mark_pnl_for_position
    /// will return 0 for this account at this oracle price.
    ///
    /// This makes positions fungible: any LP can close any user's position
    /// because PnL is settled to a common reference price.
    pub fn settle_mark_to_oracle(&mut self, idx: u16, oracle_price: u64) -> Result<()> {
        if idx as usize >= MAX_ACCOUNTS || !self.is_used(idx as usize) {
            return Err(RiskError::AccountNotFound);
        }

        if self.accounts[idx as usize].position_size.is_zero() {
            // No position: just set entry to oracle for determinism
            self.accounts[idx as usize].entry_price = oracle_price;
            return Ok(());
        }

        // Compute mark PnL at current oracle
        let mark = Self::mark_pnl_for_position(
            self.accounts[idx as usize].position_size.get(),
            self.accounts[idx as usize].entry_price,
            oracle_price,
        )?;

        // Realize the mark PnL via set_pnl (maintains pnl_pos_tot)
        let new_pnl = self.accounts[idx as usize]
            .pnl
            .get()
            .checked_add(mark)
            .ok_or(RiskError::Overflow)?;
        self.set_pnl(idx as usize, new_pnl);

        // Reset entry to oracle (mark PnL is now 0 at this price)
        self.accounts[idx as usize].entry_price = oracle_price;

        Ok(())
    }

    /// Best-effort mark-to-oracle settlement that uses saturating_add instead of
    /// checked_add, so it never fails on overflow.  This prevents the liquidation
    /// path from wedging on extreme mark PnL values.
    fn settle_mark_to_oracle_best_effort(&mut self, idx: u16, oracle_price: u64) -> Result<()> {
        if idx as usize >= MAX_ACCOUNTS || !self.is_used(idx as usize) {
            return Err(RiskError::AccountNotFound);
        }

        if self.accounts[idx as usize].position_size.is_zero() {
            self.accounts[idx as usize].entry_price = oracle_price;
            return Ok(());
        }

        // Compute mark PnL at current oracle
        let mark = Self::mark_pnl_for_position(
            self.accounts[idx as usize].position_size.get(),
            self.accounts[idx as usize].entry_price,
            oracle_price,
        )?;

        // Realize the mark PnL via set_pnl (saturating — never fails on overflow)
        let new_pnl = self.accounts[idx as usize].pnl.get().saturating_add(mark);
        self.set_pnl(idx as usize, new_pnl);

        // Reset entry to oracle (mark PnL is now 0 at this price)
        self.accounts[idx as usize].entry_price = oracle_price;

        Ok(())
    }

    /// Full account touch: funding + mark settlement + maintenance fees + warmup.
    /// This is the standard "lazy settlement" path called on every user operation.
    /// Triggers liquidation check if fees push account below maintenance margin.
    pub fn touch_account_full(&mut self, idx: u16, now_slot: u64, oracle_price: u64) -> Result<()> {
        // Update current_slot for consistent warmup/bookkeeping
        self.current_slot = now_slot;

        // 1. Settle funding
        self.touch_account(idx)?;

        // 2. Settle mark-to-market (variation margin)
        // Per spec §5.4: if AvailGross increases, warmup must restart.
        // Capture old AvailGross before mark settlement.
        let old_avail_gross = {
            let pnl = self.accounts[idx as usize].pnl.get();
            if pnl > 0 {
                pnl as u128
            } else {
                0
            }
        };
        self.settle_mark_to_oracle(idx, oracle_price)?;
        // If AvailGross increased, update warmup slope (restarts warmup timer)
        let new_avail_gross = {
            let pnl = self.accounts[idx as usize].pnl.get();
            if pnl > 0 {
                pnl as u128
            } else {
                0
            }
        };
        if new_avail_gross > old_avail_gross {
            self.update_warmup_slope(idx)?;
        }

        // 3. Settle maintenance fees (may trigger undercollateralized error)
        self.settle_maintenance_fee(idx, now_slot, oracle_price)?;

        // 4. Settle warmup (convert warmed PnL to capital, realize losses)
        self.settle_warmup_to_capital(idx)?;

        // 5. Sweep any fee debt from newly-available capital (warmup may
        //    have created capital that should pay outstanding fee debt)
        self.pay_fee_debt_from_capital(idx);

        // 6. Re-check maintenance margin after fee debt sweep
        if !self.accounts[idx as usize].position_size.is_zero()
            && !self.is_above_maintenance_margin_mtm(&self.accounts[idx as usize], oracle_price)
        {
            return Err(RiskError::Undercollateralized);
        }

        Ok(())
    }

    /// Minimal touch for crank liquidations: funding + maintenance only.
    /// Skips warmup settlement for performance - losses are handled inline
    /// by the deferred close helpers, positive warmup left for user ops.
    #[allow(dead_code)]
    fn touch_account_for_crank(
        &mut self,
        idx: u16,
        now_slot: u64,
        oracle_price: u64,
    ) -> Result<()> {
        // 1. Settle funding
        self.touch_account(idx)?;

        // 2. Settle maintenance fees (may trigger undercollateralized error)
        self.settle_maintenance_fee(idx, now_slot, oracle_price)?;

        // NOTE: No warmup settlement - handled inline for losses in close helpers
        Ok(())
    }

    // ========================================
    // Deposits and Withdrawals
    // ========================================

    /// Deposit funds to account.
    ///
    /// Settles any accrued maintenance fees from the deposit first,
    /// with the remainder added to capital. This ensures fee conservation
    /// (fees are never forgiven) and prevents stuck accounts.
    pub fn deposit(&mut self, idx: u16, amount: u128, now_slot: u64) -> Result<()> {
        // Update current_slot so warmup/bookkeeping progresses consistently
        self.current_slot = now_slot;

        if !self.is_used(idx as usize) {
            return Err(RiskError::AccountNotFound);
        }

        let account = &mut self.accounts[idx as usize];
        let mut deposit_remaining = amount;

        // Calculate and settle accrued fees
        let dt = now_slot.saturating_sub(account.last_fee_slot);
        if dt > 0 {
            let due = self
                .params
                .maintenance_fee_per_slot
                .get()
                .saturating_mul(dt as u128);
            account.last_fee_slot = now_slot;

            // Deduct from fee_credits (coupon: no insurance booking here —
            // insurance was already paid when credits were granted)
            account.fee_credits = account.fee_credits.saturating_sub(due as i128);
        }

        // Pay any owed fees from deposit first
        if account.fee_credits.is_negative() {
            let owed = neg_i128_to_u128(account.fee_credits.get());
            let pay = core::cmp::min(owed, deposit_remaining);

            deposit_remaining -= pay;
            self.insurance_fund.balance += pay;
            self.insurance_fund.fee_revenue += pay;

            // Credit back what was paid
            account.fee_credits = account
                .fee_credits
                .saturating_add(u128_to_i128_clamped(pay));
        }

        // Vault gets full deposit (tokens received)
        self.vault = U128::new(add_u128(self.vault.get(), amount));

        // Capital gets remainder after fees (via set_capital to maintain c_tot)
        let new_cap = add_u128(self.accounts[idx as usize].capital.get(), deposit_remaining);
        self.set_capital(idx as usize, new_cap);

        // Settle warmup after deposit (allows losses to be paid promptly if underwater)
        self.settle_warmup_to_capital(idx)?;

        // If any older fee debt remains, use capital to pay it now.
        self.pay_fee_debt_from_capital(idx);

        Ok(())
    }

    /// Withdraw capital from an account.
    /// Relies on Solana transaction atomicity: if this returns Err, the entire TX aborts.
    pub fn withdraw(
        &mut self,
        idx: u16,
        amount: u128,
        now_slot: u64,
        oracle_price: u64,
    ) -> Result<()> {
        // Update current_slot so warmup/bookkeeping progresses consistently
        self.current_slot = now_slot;

        // Validate oracle price bounds (prevents overflow in mark_pnl calculations)
        if oracle_price == 0 || oracle_price > MAX_ORACLE_PRICE {
            return Err(RiskError::Overflow);
        }

        // No require_fresh_crank: spec §10.4 does not gate withdraw on keeper
        // liveness. touch_account_full accrues market state directly, satisfying
        // spec §0 goal 6 (liveness — keeper downtime must not freeze user funds).

        // Validate account exists
        if !self.is_used(idx as usize) {
            return Err(RiskError::AccountNotFound);
        }

        // Full settlement: funding + maintenance fees + warmup
        self.touch_account_full(idx, now_slot, oracle_price)?;

        // Block withdrawal entirely if account has an open position.
        // Must close position first before withdrawing any capital.
        // This check is after settlement so funding/fees are applied first.
        if !self.accounts[idx as usize].position_size.is_zero() {
            return Err(RiskError::Undercollateralized);
        }

        // Read account state (scope the borrow)
        let (old_capital, pnl, position_size, entry_price, fee_credits) = {
            let account = &self.accounts[idx as usize];
            (
                account.capital,
                account.pnl,
                account.position_size,
                account.entry_price,
                account.fee_credits,
            )
        };

        // Check we have enough capital
        if old_capital.get() < amount {
            return Err(RiskError::InsufficientBalance);
        }

        // Calculate MTM equity after withdrawal with haircut (spec §3.3)
        // equity_mtm = max(0, new_capital + min(pnl, 0) + effective_pos_pnl(pnl) + mark_pnl)
        // Fail-safe: if mark_pnl overflows (corrupted entry_price/position_size), treat as 0 equity
        let new_capital = sub_u128(old_capital.get(), amount);
        let new_equity_mtm = {
            let eq =
                match Self::mark_pnl_for_position(position_size.get(), entry_price, oracle_price) {
                    Ok(mark_pnl) => {
                        let cap_i = u128_to_i128_clamped(new_capital);
                        let neg_pnl = core::cmp::min(pnl.get(), 0);
                        let eff_pos = self.effective_pos_pnl(pnl.get());
                        let new_eq_i = cap_i
                            .saturating_add(neg_pnl)
                            .saturating_add(u128_to_i128_clamped(eff_pos))
                            .saturating_add(mark_pnl);
                        if new_eq_i > 0 {
                            new_eq_i as u128
                        } else {
                            0
                        }
                    }
                    Err(_) => 0, // Overflow => worst-case equity => will fail margin check below
                };
            // Subtract fee debt (negative fee_credits = unpaid maintenance fees)
            let fee_debt = if fee_credits.is_negative() {
                neg_i128_to_u128(fee_credits.get())
            } else {
                0
            };
            eq.saturating_sub(fee_debt)
        };

        // If account has position, must maintain initial margin at ORACLE price (MTM check)
        // This prevents withdrawing to a state that's immediately liquidatable
        if !position_size.is_zero() {
            let position_notional = mul_u128(
                saturating_abs_i128(position_size.get()) as u128,
                oracle_price as u128,
            ) / 1_000_000;

            let initial_margin_required =
                mul_u128(position_notional, self.params.initial_margin_bps as u128) / 10_000;

            if new_equity_mtm < initial_margin_required {
                return Err(RiskError::Undercollateralized);
            }
        }

        // Commit the withdrawal (via set_capital to maintain c_tot)
        self.set_capital(idx as usize, new_capital);
        self.vault = U128::new(sub_u128(self.vault.get(), amount));

        // Post-withdrawal MTM maintenance margin check at oracle price
        // This is a safety belt to ensure we never leave an account in liquidatable state
        if !self.accounts[idx as usize].position_size.is_zero()
            && !self.is_above_maintenance_margin_mtm(&self.accounts[idx as usize], oracle_price)
        {
            // Revert the withdrawal (via set_capital to maintain c_tot)
            self.set_capital(idx as usize, old_capital.get());
            self.vault = U128::new(add_u128(self.vault.get(), amount));
            return Err(RiskError::Undercollateralized);
        }

        // Regression assert: after settle + withdraw, negative PnL should have been settled
        #[cfg(any(test, kani))]
        debug_assert!(
            !self.accounts[idx as usize].pnl.is_negative()
                || self.accounts[idx as usize].capital.is_zero(),
            "Withdraw: negative PnL must settle immediately"
        );

        Ok(())
    }

    // ========================================
    // Trading
    // ========================================

    // ========================================
    // Dynamic Fee Computation (PERC-120)
    // ========================================

    /// Compute the effective trading fee in basis points for a given notional.
    ///
    /// Uses tiered fee schedule if configured:
    /// - notional < tier2_threshold → trading_fee_bps (Tier 1)
    /// - notional < tier3_threshold → fee_tier2_bps (Tier 2)
    /// - notional >= tier3_threshold → fee_tier3_bps (Tier 3)
    ///
    /// If fee_tier2_threshold == 0, tiered fees are disabled (flat rate).
    ///
    /// Then applies utilization-based surge:
    /// - surge = fee_utilization_surge_bps * utilization_ratio
    /// - utilization_ratio = OI / (2 * vault), capped at 1.0
    /// - effective = base_tier_fee + surge
    pub fn compute_dynamic_fee_bps(&self, notional: u128) -> u64 {
        // Step 1: Determine tier fee
        let base_fee = if self.params.fee_tier2_threshold == 0 {
            // Tiered fees disabled → flat rate
            self.params.trading_fee_bps
        } else if notional >= self.params.fee_tier3_threshold && self.params.fee_tier3_threshold > 0
        {
            self.params.fee_tier3_bps
        } else if notional >= self.params.fee_tier2_threshold {
            self.params.fee_tier2_bps
        } else {
            self.params.trading_fee_bps
        };

        // Step 2: Utilization-based surge
        if self.params.fee_utilization_surge_bps == 0 {
            return base_fee;
        }

        let vault = self.vault.get();
        if vault == 0 {
            return base_fee;
        }

        let oi = self.total_open_interest.get();
        // utilization = OI / (2 * vault), capped at 1.0 (expressed as bps / 10_000)
        let vault_2x = vault.saturating_mul(2);
        let util_bps = if oi >= vault_2x {
            10_000u64 // Fully utilized
        } else {
            // (oi * 10_000 / vault_2x) as u64
            (oi.saturating_mul(10_000) / vault_2x.max(1)) as u64
        };

        // surge = fee_utilization_surge_bps * util_bps / 10_000
        let surge =
            (self.params.fee_utilization_surge_bps as u128 * util_bps as u128 / 10_000) as u64;

        base_fee.saturating_add(surge)
    }

    /// Compute the fee split for a given total fee amount.
    ///
    /// Returns (lp_share, protocol_share, creator_share).
    /// If fee split params are all 0, 100% goes to LP vault (legacy behavior).
    pub fn compute_fee_split(&self, total_fee: u128) -> (u128, u128, u128) {
        if self.params.fee_split_lp_bps == 0
            && self.params.fee_split_protocol_bps == 0
            && self.params.fee_split_creator_bps == 0
        {
            // Legacy: 100% to LP vault
            return (total_fee, 0, 0);
        }

        let lp = mul_u128(total_fee, self.params.fee_split_lp_bps as u128) / 10_000;
        let protocol = mul_u128(total_fee, self.params.fee_split_protocol_bps as u128) / 10_000;
        // Creator gets the remainder to avoid rounding loss
        let creator = total_fee.saturating_sub(lp).saturating_sub(protocol);

        (lp, protocol, creator)
    }

    pub fn account_equity(&self, account: &Account) -> u128 {
        let cap_i = u128_to_i128_clamped(account.capital.get());
        let eq_i = cap_i.saturating_add(account.pnl.get());
        if eq_i > 0 {
            eq_i as u128
        } else {
            0
        }
    }

    // ========================================================================
    // Margin helpers (spec §9.1) — ported from upstream T7
    // ========================================================================

    /// notional (spec §9.1): floor(|effective_pos_q| * oracle_price / POS_SCALE)
    pub fn notional(&self, idx: usize, oracle_price: u64) -> u128 {
        let eff = self.effective_pos_q(idx);
        if eff == 0 {
            return 0;
        }
        let abs_eff = eff.unsigned_abs();
        mul_div_floor_u128(abs_eff, oracle_price as u128, POS_SCALE)
    }

    /// account_equity_net (spec §3.4): max(0, Eq_maint_raw_i)
    pub fn account_equity_net(&self, account: &Account, _oracle_price: u64) -> i128 {
        let raw = self.account_equity_maint_raw(account);
        if raw < 0 {
            0i128
        } else {
            raw
        }
    }

    /// is_above_maintenance_margin (spec §9.1): Eq_net_i > MM_req_i
    pub fn is_above_maintenance_margin(
        &self,
        account: &Account,
        idx: usize,
        oracle_price: u64,
    ) -> bool {
        let eq_net = self.account_equity_net(account, oracle_price);
        let eff = self.effective_pos_q(idx);
        if eff == 0 {
            return eq_net > 0;
        }
        let not = self.notional(idx, oracle_price);
        let proportional =
            mul_div_floor_u128(not, self.params.maintenance_margin_bps as u128, 10_000);
        let mm_req = core::cmp::max(proportional, self.params.min_nonzero_mm_req);
        let mm_req_i128 = if mm_req > i128::MAX as u128 {
            i128::MAX
        } else {
            mm_req as i128
        };
        eq_net > mm_req_i128
    }

    /// is_above_maintenance_margin_from_notional: variant that accepts pre-computed
    /// notional (from the caller's `new_eff` param) instead of re-reading engine state.
    /// Used inside enforce_one_side_margin to avoid stale position_basis_q reads.
    fn is_above_maintenance_margin_from_notional(
        &self,
        account: &Account,
        notional: u128,
        oracle_price: u64,
    ) -> bool {
        let eq_net = self.account_equity_net(account, oracle_price);
        if notional == 0 {
            return eq_net > 0;
        }
        let proportional =
            mul_div_floor_u128(notional, self.params.maintenance_margin_bps as u128, 10_000);
        let mm_req = core::cmp::max(proportional, self.params.min_nonzero_mm_req);
        let mm_req_i128 = if mm_req > i128::MAX as u128 {
            i128::MAX
        } else {
            mm_req as i128
        };
        eq_net > mm_req_i128
    }

    /// is_above_initial_margin (spec §9.1): Eq_init_raw_i >= IM_req_i
    pub fn is_above_initial_margin(
        &self,
        account: &Account,
        idx: usize,
        oracle_price: u64,
    ) -> bool {
        let eq_init_raw = self.account_equity_init_raw(account);
        let eff = self.effective_pos_q(idx);
        if eff == 0 {
            return eq_init_raw >= 0;
        }
        let not = self.notional(idx, oracle_price);
        let proportional = mul_div_floor_u128(not, self.params.initial_margin_bps as u128, 10_000);
        let im_req = core::cmp::max(proportional, self.params.min_nonzero_im_req);
        let im_req_i128 = if im_req > i128::MAX as u128 {
            i128::MAX
        } else {
            im_req as i128
        };
        eq_init_raw >= im_req_i128
    }

    /// enforce_post_trade_margin (spec §10.5 step 29):
    /// Calls enforce_one_side_margin for both sides of a trade.
    /// `fee` is the trading fee charged to side `a` (user). Side `b` (LP) pays no fee
    /// — pass fee=0 for LP so the §9.2 exemption uses the correct fee-neutral buffer.
    #[allow(clippy::too_many_arguments)]
    pub fn enforce_post_trade_margin(
        &self,
        a: usize,
        b: usize,
        oracle_price: u64,
        old_eff_a: &i128,
        new_eff_a: &i128,
        old_eff_b: &i128,
        new_eff_b: &i128,
        buffer_pre_a: I256,
        buffer_pre_b: I256,
        fee: u128,
    ) -> Result<()> {
        // `a` is the user (fee payer); `b` is the LP (no fee charged — fee=0).
        self.enforce_one_side_margin(a, oracle_price, old_eff_a, new_eff_a, buffer_pre_a, fee)?;
        self.enforce_one_side_margin(b, oracle_price, old_eff_b, new_eff_b, buffer_pre_b, 0u128)?;
        Ok(())
    }

    /// enforce_one_side_margin (spec §10.5 step 29, v12.0.2 §9.2):
    /// After a trade, gate on initial margin (risk-increasing) or maintenance margin
    /// (risk-reducing). Strict-reducing trades that improve the fee-neutral buffer are
    /// exempted from liquidation (spec §9.2 exemption).
    pub fn enforce_one_side_margin_pub(
        &self,
        idx: usize,
        oracle_price: u64,
        old_eff: &i128,
        new_eff: &i128,
        buffer_pre: I256,
        fee: u128,
    ) -> Result<()> {
        self.enforce_one_side_margin(idx, oracle_price, old_eff, new_eff, buffer_pre, fee)
    }

    fn enforce_one_side_margin(
        &self,
        idx: usize,
        oracle_price: u64,
        old_eff: &i128,
        new_eff: &i128,
        buffer_pre: I256,
        fee: u128,
    ) -> Result<()> {
        if *new_eff == 0 {
            // v12.0.2 §10.5 step 29: flat-close guard — Eq_maint_raw_i >= 0.
            // Prevents flat exits with negative net wealth from fee debt.
            let maint_raw = self.account_equity_maint_raw_wide(&self.accounts[idx]);
            if maint_raw.is_negative() {
                return Err(RiskError::Undercollateralized);
            }
            return Ok(());
        }

        let abs_old: u128 = if *old_eff == 0 {
            0u128
        } else {
            old_eff.unsigned_abs()
        };
        let abs_new = new_eff.unsigned_abs();

        // Determine if risk-increasing (spec §9.2)
        let risk_increasing = abs_new > abs_old
            || (*old_eff > 0 && *new_eff < 0)
            || (*old_eff < 0 && *new_eff > 0)
            || *old_eff == 0;

        // Determine if strictly risk-reducing (spec §9.2)
        let strictly_reducing = *old_eff != 0
            && *new_eff != 0
            && ((*old_eff > 0 && *new_eff > 0) || (*old_eff < 0 && *new_eff < 0))
            && abs_new < abs_old;

        // NOTE: Notional is computed directly from `new_eff` param (not re-read from
        // engine state via effective_pos_q) to avoid stale position_basis_q reads after
        // execute_trade mutations. Security issue: HIGH — fix per PR#69 review.
        let notional_from_new_eff = if abs_new == 0 {
            0u128
        } else {
            mul_div_floor_u128(abs_new, oracle_price as u128, POS_SCALE)
        };

        if risk_increasing {
            // Require initial-margin healthy using Eq_init_raw_i
            // Uses is_above_initial_margin which reads equity from account state (capital/pnl)
            // and IM requirement computed from new_eff via notional_from_new_eff.
            let im_req = {
                let proportional = mul_div_floor_u128(
                    notional_from_new_eff,
                    self.params.initial_margin_bps as u128,
                    10_000,
                );
                core::cmp::max(proportional, self.params.min_nonzero_im_req)
            };
            let im_req_i128 = if im_req > i128::MAX as u128 {
                i128::MAX
            } else {
                im_req as i128
            };
            let eq_init_raw = self.account_equity_init_raw(&self.accounts[idx]);
            if eq_init_raw < im_req_i128 {
                return Err(RiskError::Undercollateralized);
            }
        } else if strictly_reducing {
            // v12.0.2 §10.5 step 29: strict risk-reducing exemption (fee-neutral).
            // Checked BEFORE maintenance-margin gate to avoid dead-code (security issue: MEDIUM).
            // Both conditions must hold in exact widened I256:
            // 1. Fee-neutral buffer improves: (Eq_maint_raw_post + fee) - MM_req_post > buffer_pre
            // 2. Fee-neutral shortfall does not worsen: min(Eq_maint_raw_post + fee, 0) >= min(Eq_maint_raw_pre, 0)
            let maint_raw_wide_post = self.account_equity_maint_raw_wide(&self.accounts[idx]);
            let fee_wide = I256::from_u128(fee);

            // Fee-neutral post equity and buffer — MM requirement uses new_eff (not stale state)
            let maint_raw_fee_neutral =
                maint_raw_wide_post.checked_add(fee_wide).expect("I256 add");
            let mm_req_post = {
                let proportional = mul_div_floor_u128(
                    notional_from_new_eff,
                    self.params.maintenance_margin_bps as u128,
                    10_000,
                );
                core::cmp::max(proportional, self.params.min_nonzero_mm_req)
            };
            let buffer_post_fee_neutral = maint_raw_fee_neutral
                .checked_sub(I256::from_u128(mm_req_post))
                .expect("I256 sub");

            // Recover pre-trade raw equity from buffer_pre + MM_req_pre (uses old_eff)
            let mm_req_pre = {
                let not_pre = if *old_eff == 0 {
                    0u128
                } else {
                    mul_div_floor_u128(old_eff.unsigned_abs(), oracle_price as u128, POS_SCALE)
                };
                core::cmp::max(
                    mul_div_floor_u128(not_pre, self.params.maintenance_margin_bps as u128, 10_000),
                    self.params.min_nonzero_mm_req,
                )
            };
            let maint_raw_pre = buffer_pre
                .checked_add(I256::from_u128(mm_req_pre))
                .expect("I256 add");

            // Condition 1: fee-neutral buffer strictly improves
            let cond1 = buffer_post_fee_neutral > buffer_pre;

            // Condition 2: fee-neutral shortfall below zero does not worsen
            // min(post + fee, 0) >= min(pre, 0)
            let zero = I256::from_i128(0);
            let shortfall_post = if maint_raw_fee_neutral < zero {
                maint_raw_fee_neutral
            } else {
                zero
            };
            let shortfall_pre = if maint_raw_pre < zero {
                maint_raw_pre
            } else {
                zero
            };
            let cond2 = shortfall_post >= shortfall_pre;

            if !(cond1 && cond2) {
                // Exemption conditions not met: fall through to maintenance check
                let mm_req_i128 = if mm_req_post > i128::MAX as u128 {
                    i128::MAX
                } else {
                    mm_req_post as i128
                };
                let eq_net = self.account_equity_net(&self.accounts[idx], oracle_price);
                if eq_net <= mm_req_i128 {
                    return Err(RiskError::Undercollateralized);
                }
            }
        } else if self.is_above_maintenance_margin_from_notional(
            &self.accounts[idx],
            notional_from_new_eff,
            oracle_price,
        ) {
            // Maintenance healthy: allow
        } else {
            return Err(RiskError::Undercollateralized);
        }
        Ok(())
    }

    /// Eq_maint_raw_i in exact I256 (spec §3.4 "transient widened signed type").
    ///
    /// Eq_maint_raw_i = C_i + PNL_i - FeeDebt_i
    ///
    /// MUST be used for strict before/after maintenance-buffer comparisons to
    /// avoid saturation masking real changes. No clamping.
    pub fn account_equity_maint_raw_wide(&self, account: &Account) -> I256 {
        let cap = I256::from_u128(account.capital.get());
        let pnl = I256::from_i128(account.pnl.get());
        let fee_debt = if account.fee_credits.is_negative() {
            I256::from_u128(neg_i128_to_u128(account.fee_credits.get()))
        } else {
            I256::ZERO
        };
        cap.checked_add(pnl)
            .expect("I256 add overflow: cap + pnl")
            .checked_sub(fee_debt)
            .expect("I256 sub overflow: - fee_debt")
    }

    /// Eq_maint_raw_i clamped to i128 (spec §3.4 saturation rule).
    /// Positive overflow → i128::MAX; negative overflow → i128::MIN + 1.
    pub fn account_equity_maint_raw(&self, account: &Account) -> i128 {
        let wide = self.account_equity_maint_raw_wide(account);
        match wide.try_into_i128() {
            Some(v) => v,
            None => {
                if wide.is_negative() {
                    i128::MIN + 1
                } else {
                    i128::MAX
                }
            }
        }
    }

    /// Eq_init_raw_i (spec §3.4): C_i + min(PNL_i, 0) + PNL_eff_matured_i - FeeDebt_i
    ///
    /// Uses haircutted matured PnL only — stricter than maintenance equity.
    /// Returns i128 with saturation on overflow per spec §3.4.
    pub fn account_equity_init_raw(&self, account: &Account) -> i128 {
        let cap = I256::from_u128(account.capital.get());
        let neg_pnl_val = if account.pnl.get() < 0 {
            account.pnl.get()
        } else {
            0i128
        };
        let neg_pnl = I256::from_i128(neg_pnl_val);
        // Effective matured PnL: apply haircut to the matured (released) portion only
        let released = {
            let pos = if account.pnl.get() > 0 {
                account.pnl.get() as u128
            } else {
                0u128
            };
            pos.saturating_sub(account.reserved_pnl)
        };
        let eff_matured = {
            let (h_num, h_den) = self.haircut_ratio();
            if h_den == 0 {
                released
            } else {
                mul_u128(released, h_num) / h_den
            }
        };
        let eff_mat_wide = I256::from_u128(eff_matured);
        let fee_debt = if account.fee_credits.is_negative() {
            I256::from_u128(neg_i128_to_u128(account.fee_credits.get()))
        } else {
            I256::ZERO
        };
        let sum = cap
            .checked_add(neg_pnl)
            .expect("I256 add overflow: cap + neg_pnl")
            .checked_add(eff_mat_wide)
            .expect("I256 add overflow: + eff_matured")
            .checked_sub(fee_debt)
            .expect("I256 sub overflow: - fee_debt");
        match sum.try_into_i128() {
            Some(v) => v,
            None => {
                if sum.is_negative() {
                    i128::MIN + 1
                } else {
                    i128::MAX
                }
            }
        }
    }

    /// Eq_init_net_i (spec §3.4): max(0, Eq_init_raw_i).
    pub fn account_equity_init_net(&self, account: &Account) -> i128 {
        let raw = self.account_equity_init_raw(account);
        if raw < 0 {
            0
        } else {
            raw
        }
    }

    /// Mark-to-market equity at oracle price with haircut (the ONLY correct equity for margin checks).
    /// equity_mtm = max(0, C_i + min(PNL_i, 0) + PNL_eff_pos_i + mark_pnl)
    /// where PNL_eff_pos_i = floor(max(PNL_i, 0) * h_num / h_den) per spec §3.3.
    ///
    /// FAIL-SAFE: On overflow, returns 0 (worst-case equity) to ensure liquidation
    /// can still trigger. This prevents overflow from blocking liquidation.
    pub fn account_equity_mtm_at_oracle(&self, account: &Account, oracle_price: u64) -> u128 {
        let mark = match Self::mark_pnl_for_position(
            account.position_size.get(),
            account.entry_price,
            oracle_price,
        ) {
            Ok(m) => m,
            Err(_) => return 0, // Overflow => worst-case equity
        };
        let cap_i = u128_to_i128_clamped(account.capital.get());
        let neg_pnl = core::cmp::min(account.pnl.get(), 0);
        let eff_pos = self.effective_pos_pnl(account.pnl.get());
        let eq_i = cap_i
            .saturating_add(neg_pnl)
            .saturating_add(u128_to_i128_clamped(eff_pos))
            .saturating_add(mark);
        let eq = if eq_i > 0 { eq_i as u128 } else { 0 };
        // Subtract fee debt (negative fee_credits = unpaid maintenance fees)
        let fee_debt = if account.fee_credits.is_negative() {
            neg_i128_to_u128(account.fee_credits.get())
        } else {
            0
        };
        eq.saturating_sub(fee_debt)
    }

    /// MTM margin check: is equity_mtm > required margin?
    /// This is the ONLY correct margin predicate for all risk checks.
    ///
    /// FAIL-SAFE: Returns false on any error (treat as below margin / liquidatable).
    pub fn is_above_margin_bps_mtm(&self, account: &Account, oracle_price: u64, bps: u64) -> bool {
        let equity = self.account_equity_mtm_at_oracle(account, oracle_price);

        // Position value at oracle price
        let position_value = mul_u128(
            saturating_abs_i128(account.position_size.get()) as u128,
            oracle_price as u128,
        ) / 1_000_000;

        // Price-based margin requirement
        let proportional = mul_u128(position_value, bps as u128) / 10_000;

        // Spec §9.1: apply absolute margin floor (maintenance floor applies to MTM check).
        let floor = self.params.min_nonzero_mm_req;
        let margin_required = core::cmp::max(proportional, floor);

        // Position-based margin requirement (coin-margined perps).
        // When oracle price is small, the price-based check undercounts.
        // This ensures correct margin regardless of price level.
        let pos_margin = mul_u128(
            saturating_abs_i128(account.position_size.get()) as u128,
            bps as u128,
        ) / 10_000;

        // Must pass BOTH checks: whichever requires more margin wins
        let effective_margin = if pos_margin > margin_required {
            pos_margin
        } else {
            margin_required
        };
        equity > effective_margin
    }

    /// MTM maintenance margin check (fail-safe: returns false on overflow)
    #[inline]
    pub fn is_above_maintenance_margin_mtm(&self, account: &Account, oracle_price: u64) -> bool {
        self.is_above_margin_bps_mtm(account, oracle_price, self.params.maintenance_margin_bps)
    }

    /// Cheap priority score for ranking liquidation candidates.
    /// Score = max(maint_required - equity, 0).
    /// Higher score = more urgent to liquidate.
    ///
    /// This is a ranking heuristic only - NOT authoritative.
    /// Real liquidation still calls touch_account_full() and checks margin properly.
    /// A "wrong" top-K pick is harmless: it just won't liquidate.
    #[inline]
    #[allow(dead_code)]
    fn liq_priority_score(&self, a: &Account, oracle_price: u64) -> u128 {
        if a.position_size.is_zero() {
            return 0;
        }

        // MTM equity (fail-safe: overflow returns 0, making account appear liquidatable)
        let equity = self.account_equity_mtm_at_oracle(a, oracle_price);

        let pos_value = mul_u128(
            saturating_abs_i128(a.position_size.get()) as u128,
            oracle_price as u128,
        ) / 1_000_000;

        let price_maint = mul_u128(pos_value, self.params.maintenance_margin_bps as u128) / 10_000;

        // Position-based margin (coin-margined perps)
        let pos_maint = mul_u128(
            saturating_abs_i128(a.position_size.get()) as u128,
            self.params.maintenance_margin_bps as u128,
        ) / 10_000;

        let maint = if pos_maint > price_maint {
            pos_maint
        } else {
            price_maint
        };

        maint.saturating_sub(equity)
    }

    /// Risk-reduction-only mode is entered when the system is in deficit. Warmups are frozen so pending PNL cannot become principal. Withdrawals of principal (capital) are allowed (subject to margin). Risk-increasing actions are blocked; only risk-reducing/neutral operations are allowed.
    /// Execute a trade between LP and user.
    /// Relies on Solana transaction atomicity: if this returns Err, the entire TX aborts.
    pub fn execute_trade<M: MatchingEngine>(
        &mut self,
        matcher: &M,
        lp_idx: u16,
        user_idx: u16,
        now_slot: u64,
        oracle_price: u64,
        size: i128,
    ) -> Result<()> {
        // Update current_slot so warmup/bookkeeping progresses consistently
        self.current_slot = now_slot;

        // No require_fresh_crank: spec §10.5 does not gate execute_trade on
        // keeper liveness. touch_account_full accrues market state directly,
        // satisfying spec §0 goal 6 (liveness without external action).

        // Validate indices
        if !self.is_used(lp_idx as usize) || !self.is_used(user_idx as usize) {
            return Err(RiskError::AccountNotFound);
        }

        // Validate oracle price bounds (prevents overflow in mark_pnl calculations)
        if oracle_price == 0 || oracle_price > MAX_ORACLE_PRICE {
            return Err(RiskError::Overflow);
        }

        // Validate requested size bounds
        if size == 0 || size == i128::MIN {
            return Err(RiskError::Overflow);
        }
        if saturating_abs_i128(size) as u128 > MAX_POSITION_ABS {
            return Err(RiskError::Overflow);
        }

        // Validate account kinds (using is_lp/is_user methods for SBF workaround)
        if !self.accounts[lp_idx as usize].is_lp() {
            return Err(RiskError::AccountKindMismatch);
        }
        if !self.accounts[user_idx as usize].is_user() {
            return Err(RiskError::AccountKindMismatch);
        }

        // Check if trade increases risk (absolute exposure for either party)
        let old_user_pos = self.accounts[user_idx as usize].position_size.get();
        let old_lp_pos = self.accounts[lp_idx as usize].position_size.get();
        let new_user_pos = old_user_pos.saturating_add(size);
        let new_lp_pos = old_lp_pos.saturating_sub(size);

        let user_inc = saturating_abs_i128(new_user_pos) > saturating_abs_i128(old_user_pos);
        let lp_inc = saturating_abs_i128(new_lp_pos) > saturating_abs_i128(old_lp_pos);

        if user_inc || lp_inc {
            // Risk-increasing: require recent full sweep
            self.require_recent_full_sweep(now_slot)?;
        }

        // Call matching engine
        let lp = &self.accounts[lp_idx as usize];
        let execution = matcher.execute_match(
            &lp.matcher_program,
            &lp.matcher_context,
            lp.account_id,
            oracle_price,
            size,
        )?;

        let exec_price = execution.price;
        let exec_size = execution.size;

        // Validate matcher output (trust boundary enforcement)
        // Price bounds
        if exec_price == 0 || exec_price > MAX_ORACLE_PRICE {
            return Err(RiskError::InvalidMatchingEngine);
        }

        // Size bounds
        if exec_size == 0 {
            // No fill: treat as no-op trade (no side effects, deterministic)
            return Ok(());
        }
        if exec_size == i128::MIN {
            return Err(RiskError::InvalidMatchingEngine);
        }
        if saturating_abs_i128(exec_size) as u128 > MAX_POSITION_ABS {
            return Err(RiskError::InvalidMatchingEngine);
        }

        // Must be same direction as requested
        if (exec_size > 0) != (size > 0) {
            return Err(RiskError::InvalidMatchingEngine);
        }

        // Must be partial fill at most (abs(exec) <= abs(request))
        if saturating_abs_i128(exec_size) > saturating_abs_i128(size) {
            return Err(RiskError::InvalidMatchingEngine);
        }

        // PERC-118: Update trade TWAP with execution price (volume-weighted EMA).
        // Uses the same EMA formula as the oracle mark: alpha controls how fast
        // the TWAP tracks recent fills. Notional-weighted to resist dust manipulation.
        {
            let notional =
                mul_u128(saturating_abs_i128(exec_size) as u128, exec_price as u128) / 1_000_000;
            self.update_trade_twap(exec_price, notional, now_slot);
        }

        // Settle funding, mark-to-market, and maintenance fees for both accounts
        // Mark settlement MUST happen before position changes (variation margin)
        // Note: warmup is settled at the END after trade PnL is generated
        self.touch_account(user_idx)?;
        self.touch_account(lp_idx)?;

        // Per spec §5.4: if AvailGross increases from mark settlement, warmup must restart.
        // Capture old AvailGross before mark settlement for both accounts.
        let user_old_avail = {
            let pnl = self.accounts[user_idx as usize].pnl.get();
            if pnl > 0 {
                pnl as u128
            } else {
                0
            }
        };
        let lp_old_avail = {
            let pnl = self.accounts[lp_idx as usize].pnl.get();
            if pnl > 0 {
                pnl as u128
            } else {
                0
            }
        };
        self.settle_mark_to_oracle(user_idx, oracle_price)?;
        self.settle_mark_to_oracle(lp_idx, oracle_price)?;
        // If AvailGross increased from mark settlement, update warmup slope (restarts warmup)
        let user_new_avail = {
            let pnl = self.accounts[user_idx as usize].pnl.get();
            if pnl > 0 {
                pnl as u128
            } else {
                0
            }
        };
        let lp_new_avail = {
            let pnl = self.accounts[lp_idx as usize].pnl.get();
            if pnl > 0 {
                pnl as u128
            } else {
                0
            }
        };
        if user_new_avail > user_old_avail {
            self.update_warmup_slope(user_idx)?;
        }
        if lp_new_avail > lp_old_avail {
            self.update_warmup_slope(lp_idx)?;
        }

        self.settle_maintenance_fee(user_idx, now_slot, oracle_price)?;
        self.settle_maintenance_fee(lp_idx, now_slot, oracle_price)?;

        // Calculate fee using dynamic fee model (tiered + utilization surge)
        // Falls back to flat trading_fee_bps when fee_tier2_threshold == 0
        let abs_size = saturating_abs_i128(exec_size) as u128;
        let notional = mul_u128(abs_size, exec_price as u128) / 1_000_000;
        let fee_bps = self.compute_dynamic_fee_bps(notional);
        let fee = if abs_size > 0 && fee_bps > 0 {
            // Ceiling division: ensures at least 1 atomic unit fee for any real trade
            mul_u128(abs_size, fee_bps as u128).div_ceil(10_000)
        } else {
            0
        };

        // Capture pre-trade effective positions and maintenance buffers for
        // enforce_post_trade_margin (spec §10.5 step 29 / T7).
        // Must be captured AFTER touch/settle (so PnL is current) but BEFORE
        // split_at_mut and position mutation.
        //
        // Use position_size as the source of truth for pre-trade effective position.
        // In our implementation, position_basis_q is not updated by execute_trade
        // (that is an upstream ADL-specific field), so effective_pos_q() returns 0
        // here. Using position_size directly gives the correct pre-trade position.
        // Security fix (HIGH from PR#69 review): ensures notional is computed from
        // the actual position, not from stale position_basis_q=0.
        let pre_eff_user = self.accounts[user_idx as usize].position_size.get();
        let pre_eff_lp = self.accounts[lp_idx as usize].position_size.get();
        let mm_req_pre_user: u128 = if pre_eff_user == 0 {
            0
        } else {
            let not =
                mul_div_floor_u128(pre_eff_user.unsigned_abs(), oracle_price as u128, POS_SCALE);
            core::cmp::max(
                mul_div_floor_u128(not, self.params.maintenance_margin_bps as u128, 10_000),
                self.params.min_nonzero_mm_req,
            )
        };
        let mm_req_pre_lp: u128 = if pre_eff_lp == 0 {
            0
        } else {
            let not =
                mul_div_floor_u128(pre_eff_lp.unsigned_abs(), oracle_price as u128, POS_SCALE);
            core::cmp::max(
                mul_div_floor_u128(not, self.params.maintenance_margin_bps as u128, 10_000),
                self.params.min_nonzero_mm_req,
            )
        };
        let buffer_pre_user = self
            .account_equity_maint_raw_wide(&self.accounts[user_idx as usize])
            .checked_sub(I256::from_u128(mm_req_pre_user))
            .expect("I256 sub");
        let buffer_pre_lp = self
            .account_equity_maint_raw_wide(&self.accounts[lp_idx as usize])
            .checked_sub(I256::from_u128(mm_req_pre_lp))
            .expect("I256 sub");

        // Access both accounts
        let (user, lp) = if user_idx < lp_idx {
            let (left, right) = self.accounts.split_at_mut(lp_idx as usize);
            (&mut left[user_idx as usize], &mut right[0])
        } else {
            let (left, right) = self.accounts.split_at_mut(user_idx as usize);
            (&mut right[0], &mut left[lp_idx as usize])
        };

        // Calculate new positions (checked math - overflow returns Err)
        let new_user_position = user
            .position_size
            .get()
            .checked_add(exec_size)
            .ok_or(RiskError::Overflow)?;
        let new_lp_position = lp
            .position_size
            .get()
            .checked_sub(exec_size)
            .ok_or(RiskError::Overflow)?;

        // Validate final position bounds (prevents overflow in mark_pnl calculations)
        if saturating_abs_i128(new_user_position) as u128 > MAX_POSITION_ABS
            || saturating_abs_i128(new_lp_position) as u128 > MAX_POSITION_ABS
        {
            return Err(RiskError::Overflow);
        }

        // Trade PnL = (oracle - exec_price) * exec_size (zero-sum between parties)
        // User gains if buying below oracle (exec_size > 0, oracle > exec_price)
        // LP gets opposite sign
        // Note: entry_price is already oracle_price after settle_mark_to_oracle
        let price_diff = (oracle_price as i128)
            .checked_sub(exec_price as i128)
            .ok_or(RiskError::Overflow)?;

        let trade_pnl = price_diff
            .checked_mul(exec_size)
            .ok_or(RiskError::Overflow)?
            .checked_div(oracle_price as i128)
            .ok_or(RiskError::Overflow)?;

        // Compute final PNL values (checked math - overflow returns Err)
        let new_user_pnl = user
            .pnl
            .get()
            .checked_add(trade_pnl)
            .ok_or(RiskError::Overflow)?;
        let new_lp_pnl = lp
            .pnl
            .get()
            .checked_sub(trade_pnl)
            .ok_or(RiskError::Overflow)?;

        // Deduct trading fee from user capital, not PnL (spec §8.1)
        let new_user_capital = user
            .capital
            .get()
            .checked_sub(fee)
            .ok_or(RiskError::InsufficientBalance)?;

        // Compute projected pnl_pos_tot AFTER trade PnL for fresh haircut in margin checks.
        // Can't call self.haircut_ratio() due to split_at_mut borrow on accounts;
        // inline the delta computation and haircut formula.
        let old_user_pnl_pos = if user.pnl.get() > 0 {
            user.pnl.get() as u128
        } else {
            0
        };
        let new_user_pnl_pos = if new_user_pnl > 0 {
            new_user_pnl as u128
        } else {
            0
        };
        let old_lp_pnl_pos = if lp.pnl.get() > 0 {
            lp.pnl.get() as u128
        } else {
            0
        };
        let new_lp_pnl_pos = if new_lp_pnl > 0 {
            new_lp_pnl as u128
        } else {
            0
        };

        // Recompute haircut using projected post-trade pnl_pos_tot (spec §3.3).
        // Fee moves C→I so Residual = V - C_tot - I is unchanged; only pnl_pos_tot changes.
        let projected_pnl_pos_tot = self
            .pnl_pos_tot
            .get()
            .saturating_add(new_user_pnl_pos)
            .saturating_sub(old_user_pnl_pos)
            .saturating_add(new_lp_pnl_pos)
            .saturating_sub(old_lp_pnl_pos);

        let (h_num, h_den) = if projected_pnl_pos_tot == 0 {
            (1u128, 1u128)
        } else {
            let total_insurance =
                self.insurance_fund.balance.get() + self.insurance_fund.isolated_balance.get();
            let residual = self
                .vault
                .get()
                .saturating_sub(self.c_tot.get())
                .saturating_sub(total_insurance);
            (
                core::cmp::min(residual, projected_pnl_pos_tot),
                projected_pnl_pos_tot,
            )
        };

        // Inline helper: compute effective positive PnL with post-trade haircut
        let eff_pos_pnl_inline = |pnl: i128| -> u128 {
            if pnl <= 0 {
                return 0;
            }
            let pos_pnl = pnl as u128;
            if h_den == 0 {
                return pos_pnl;
            }
            mul_u128(pos_pnl, h_num) / h_den
        };

        // Check user margin with haircut (spec §3.3, §10.4 step 7)
        // After settle_mark_to_oracle, entry_price = oracle_price, so mark_pnl = 0
        // Equity = max(0, new_capital + min(pnl, 0) + eff_pos_pnl)
        // Use initial margin if risk-increasing, maintenance margin otherwise
        if new_user_position != 0 {
            let user_cap_i = u128_to_i128_clamped(new_user_capital);
            let neg_pnl = core::cmp::min(new_user_pnl, 0);
            let eff_pos = eff_pos_pnl_inline(new_user_pnl);
            let user_eq_i = user_cap_i
                .saturating_add(neg_pnl)
                .saturating_add(u128_to_i128_clamped(eff_pos));
            let user_equity = if user_eq_i > 0 { user_eq_i as u128 } else { 0 };
            // Subtract fee debt (negative fee_credits = unpaid maintenance fees)
            let user_fee_debt = if user.fee_credits.is_negative() {
                neg_i128_to_u128(user.fee_credits.get())
            } else {
                0
            };
            let user_equity = user_equity.saturating_sub(user_fee_debt);
            let position_value = mul_u128(
                saturating_abs_i128(new_user_position) as u128,
                oracle_price as u128,
            ) / 1_000_000;
            // Risk-increasing if |new_pos| > |old_pos| OR position crosses zero (flip)
            // A flip is semantically a close + open, so the new side must meet initial margin
            let old_user_pos = user.position_size.get();
            let old_user_pos_abs = saturating_abs_i128(old_user_pos);
            let new_user_pos_abs = saturating_abs_i128(new_user_position);
            let user_crosses_zero = (old_user_pos > 0 && new_user_position < 0)
                || (old_user_pos < 0 && new_user_position > 0);
            let user_risk_increasing = new_user_pos_abs > old_user_pos_abs || user_crosses_zero;
            let margin_bps = if user_risk_increasing {
                self.params.initial_margin_bps
            } else {
                self.params.maintenance_margin_bps
            };
            let proportional_margin = mul_u128(position_value, margin_bps as u128) / 10_000;
            // Spec §9.1: apply absolute margin floor if position is non-flat.
            let floor = if user_risk_increasing {
                self.params.min_nonzero_im_req
            } else {
                self.params.min_nonzero_mm_req
            };
            let margin_required = core::cmp::max(proportional_margin, floor);
            if user_equity <= margin_required {
                return Err(RiskError::Undercollateralized);
            }

            // Position-based margin check (coin-margined perps).
            // When collateral and position are the same asset, the price-based
            // margin check above can undercount because price is small.
            // This check ensures: capital >= |position| * margin_bps / 10_000,
            // providing correct leverage limits regardless of oracle price.
            let pos_margin = mul_u128(
                saturating_abs_i128(new_user_position) as u128,
                margin_bps as u128,
            ) / 10_000;
            if new_user_capital < pos_margin {
                return Err(RiskError::Undercollateralized);
            }
        }

        // Check LP margin with haircut (spec §3.3, §10.4 step 7)
        // After settle_mark_to_oracle, entry_price = oracle_price, so mark_pnl = 0
        // Use initial margin if risk-increasing, maintenance margin otherwise
        if new_lp_position != 0 {
            let lp_cap_i = u128_to_i128_clamped(lp.capital.get());
            let neg_pnl = core::cmp::min(new_lp_pnl, 0);
            let eff_pos = eff_pos_pnl_inline(new_lp_pnl);
            let lp_eq_i = lp_cap_i
                .saturating_add(neg_pnl)
                .saturating_add(u128_to_i128_clamped(eff_pos));
            let lp_equity = if lp_eq_i > 0 { lp_eq_i as u128 } else { 0 };
            // Subtract fee debt (negative fee_credits = unpaid maintenance fees)
            let lp_fee_debt = if lp.fee_credits.is_negative() {
                neg_i128_to_u128(lp.fee_credits.get())
            } else {
                0
            };
            let lp_equity = lp_equity.saturating_sub(lp_fee_debt);
            let position_value = mul_u128(
                saturating_abs_i128(new_lp_position) as u128,
                oracle_price as u128,
            ) / 1_000_000;
            // Risk-increasing if |new_pos| > |old_pos| OR position crosses zero (flip)
            // A flip is semantically a close + open, so the new side must meet initial margin
            let old_lp_pos = lp.position_size.get();
            let old_lp_pos_abs = saturating_abs_i128(old_lp_pos);
            let new_lp_pos_abs = saturating_abs_i128(new_lp_position);
            let lp_crosses_zero =
                (old_lp_pos > 0 && new_lp_position < 0) || (old_lp_pos < 0 && new_lp_position > 0);
            let lp_risk_increasing = new_lp_pos_abs > old_lp_pos_abs || lp_crosses_zero;
            let margin_bps = if lp_risk_increasing {
                self.params.initial_margin_bps
            } else {
                self.params.maintenance_margin_bps
            };
            let proportional_margin = mul_u128(position_value, margin_bps as u128) / 10_000;
            // Spec §9.1: apply absolute margin floor for non-flat positions.
            let floor = if lp_risk_increasing {
                self.params.min_nonzero_im_req
            } else {
                self.params.min_nonzero_mm_req
            };
            let margin_required = core::cmp::max(proportional_margin, floor);
            if lp_equity <= margin_required {
                return Err(RiskError::Undercollateralized);
            }
        }

        // Commit all state changes
        self.insurance_fund.fee_revenue =
            U128::new(add_u128(self.insurance_fund.fee_revenue.get(), fee));
        self.insurance_fund.balance = U128::new(add_u128(self.insurance_fund.balance.get(), fee));

        // Credit fee to user's fee_credits (active traders earn credits that offset maintenance)
        user.fee_credits = user.fee_credits.saturating_add(fee as i128);

        // §4.3 Batch update exception: Direct field assignment for performance.
        // All aggregate deltas (old/new pnl_pos values) computed above before assignment;
        // aggregates (c_tot, pnl_pos_tot) updated atomically below.
        user.pnl = I128::new(new_user_pnl);
        // Save trade entry price when opening from flat (reserved_pnl = trade_entry_price)
        // Note: reserved_pnl is now u128; oracle_price is u64 — cast is safe.
        if user.position_size.is_zero() && new_user_position != 0 {
            user.reserved_pnl = oracle_price as u128;
        } else if new_user_position == 0 {
            user.reserved_pnl = 0u128; // Clear on close
        }
        // §INV PA1: Clamp reserved_pnl to max(pnl, 0) to maintain invariant.
        // Trade PnL may reduce pnl below reserved_pnl; without clamping,
        // valid_state() / canonical_inv() PA1 check fails (Kani finding).
        {
            let max_reserved: u128 = if new_user_pnl > 0 {
                new_user_pnl as u128
            } else {
                0
            };
            if user.reserved_pnl > max_reserved {
                user.reserved_pnl = max_reserved;
            }
        }
        // PA5 defense-in-depth: entry_price must be positive when position is non-zero
        if new_user_position != 0 && oracle_price == 0 {
            return Err(RiskError::InvalidEntryPrice);
        }
        user.position_size = I128::new(new_user_position);
        user.entry_price = oracle_price;
        // Commit fee deduction from user capital (spec §8.1)
        user.capital = U128::new(new_user_capital);

        lp.pnl = I128::new(new_lp_pnl);
        // Save trade entry price for LP as well
        if lp.position_size.is_zero() && new_lp_position != 0 {
            lp.reserved_pnl = oracle_price as u128;
        } else if new_lp_position == 0 {
            lp.reserved_pnl = 0u128;
        }
        // §INV PA1: Clamp reserved_pnl for LP as well
        {
            let max_reserved: u128 = if new_lp_pnl > 0 {
                new_lp_pnl as u128
            } else {
                0
            };
            if lp.reserved_pnl > max_reserved {
                lp.reserved_pnl = max_reserved;
            }
        }
        // PA5 defense-in-depth: entry_price must be positive when position is non-zero
        if new_lp_position != 0 && oracle_price == 0 {
            return Err(RiskError::InvalidEntryPrice);
        }
        lp.position_size = I128::new(new_lp_position);
        lp.entry_price = oracle_price;

        // §4.1, §4.2: Atomic aggregate maintenance after batch field assignments
        // Maintain c_tot: user capital decreased by fee
        self.c_tot = U128::new(self.c_tot.get().saturating_sub(fee));

        // Maintain pnl_pos_tot aggregate
        self.pnl_pos_tot = U128::new(
            self.pnl_pos_tot
                .get()
                .saturating_add(new_user_pnl_pos)
                .saturating_sub(old_user_pnl_pos)
                .saturating_add(new_lp_pnl_pos)
                .saturating_sub(old_lp_pnl_pos),
        );

        // Update total open interest tracking (O(1))
        // OI = sum of abs(position_size) across all accounts
        let old_oi =
            saturating_abs_i128(old_user_pos) as u128 + saturating_abs_i128(old_lp_pos) as u128;
        let new_oi = saturating_abs_i128(new_user_position) as u128
            + saturating_abs_i128(new_lp_position) as u128;
        if new_oi > old_oi {
            self.total_open_interest = self.total_open_interest.saturating_add(new_oi - old_oi);
        } else {
            self.total_open_interest = self.total_open_interest.saturating_sub(old_oi - new_oi);
        }

        // PERC-298: maintain per-side OI incrementally
        {
            // Helper: compute long/short OI contribution for a position
            fn long_short_oi(pos: i128) -> (u128, u128) {
                if pos > 0 {
                    (pos as u128, 0)
                } else {
                    (0, saturating_abs_i128(pos) as u128)
                }
            }
            let (old_user_long, old_user_short) = long_short_oi(old_user_pos);
            let (new_user_long, new_user_short) = long_short_oi(new_user_position);
            let (old_lp_long, old_lp_short) = long_short_oi(old_lp_pos);
            let (new_lp_long, new_lp_short) = long_short_oi(new_lp_position);

            let old_long = old_user_long + old_lp_long;
            let new_long = new_user_long + new_lp_long;
            if new_long > old_long {
                self.long_oi = self.long_oi.saturating_add(new_long - old_long);
            } else {
                self.long_oi = self.long_oi.saturating_sub(old_long - new_long);
            }

            let old_short = old_user_short + old_lp_short;
            let new_short = new_user_short + new_lp_short;
            if new_short > old_short {
                self.short_oi = self.short_oi.saturating_add(new_short - old_short);
            } else {
                self.short_oi = self.short_oi.saturating_sub(old_short - new_short);
            }
        }

        // Update LP aggregates for funding/threshold (O(1))
        let old_lp_abs = saturating_abs_i128(old_lp_pos) as u128;
        let new_lp_abs = saturating_abs_i128(new_lp_position) as u128;
        // net_lp_pos: delta = new - old
        self.net_lp_pos = self
            .net_lp_pos
            .saturating_sub(old_lp_pos)
            .saturating_add(new_lp_position);
        // lp_sum_abs: delta of abs values
        if new_lp_abs > old_lp_abs {
            self.lp_sum_abs = self.lp_sum_abs.saturating_add(new_lp_abs - old_lp_abs);
        } else {
            self.lp_sum_abs = self.lp_sum_abs.saturating_sub(old_lp_abs - new_lp_abs);
        }
        // lp_max_abs: monotone increase only (conservative upper bound)
        self.lp_max_abs = U128::new(self.lp_max_abs.get().max(new_lp_abs));

        // Two-pass settlement: losses first, then profits.
        // This ensures the loser's capital reduction increases Residual before
        // the winner's profit conversion reads the haircut ratio. Without this,
        // the winner's matured PnL can be haircutted to 0 because Residual
        // hasn't been increased by the loser's loss settlement yet (Finding G).
        self.settle_loss_only(user_idx)?;
        self.settle_loss_only(lp_idx)?;
        // Now Residual reflects realized losses; profit conversion uses correct h.
        self.settle_warmup_to_capital(user_idx)?;
        self.settle_warmup_to_capital(lp_idx)?;

        // Now recompute warmup slopes after PnL changes (resets started_at_slot)
        self.update_warmup_slope(user_idx)?;
        self.update_warmup_slope(lp_idx)?;

        // T7: Post-trade margin enforcement (spec §10.5 step 29, v12.0.2 §9.2).
        // Uses pre-captured positions (from position_size) and buffers.
        // new_eff = pre_eff ± exec_size (trades are zero-sum bilateral).
        // These match new_user_position / new_lp_position computed above.
        let new_eff_user = pre_eff_user
            .checked_add(exec_size)
            .ok_or(RiskError::Overflow)?;
        let new_eff_lp = pre_eff_lp
            .checked_sub(exec_size)
            .ok_or(RiskError::Overflow)?;
        self.enforce_post_trade_margin(
            user_idx as usize,
            lp_idx as usize,
            oracle_price,
            &pre_eff_user,
            &new_eff_user,
            &pre_eff_lp,
            &new_eff_lp,
            buffer_pre_user,
            buffer_pre_lp,
            fee,
        )?;

        // End-of-instruction lifecycle: finalize any deferred ADL epoch resets
        // that were scheduled during trade processing (spec §5.7-5.8).
        // Use stored funding_rate_bps_per_slot_last — NOT 0i64 — to avoid
        // overwriting the funding rate with a stale zero (security issue: LOW).
        let mut ctx = InstructionContext::new();
        let stored_rate = self.funding_rate_bps_per_slot_last;
        self.run_end_of_instruction_lifecycle(&mut ctx, stored_rate)?;

        Ok(())
    }
    /// Settle loss only (§6.1): negative PnL pays from capital immediately.
    /// If PnL still negative after capital exhausted, write off via set_pnl(i, 0).
    /// Used in two-pass settlement to ensure all losses are realized (increasing
    /// Residual) before any profit conversions use the haircut ratio.
    pub fn settle_loss_only(&mut self, idx: u16) -> Result<()> {
        if !self.is_used(idx as usize) {
            return Err(RiskError::AccountNotFound);
        }

        let pnl = self.accounts[idx as usize].pnl.get();
        if pnl < 0 {
            let need = neg_i128_to_u128(pnl);
            let capital = self.accounts[idx as usize].capital.get();
            let pay = core::cmp::min(need, capital);

            if pay > 0 {
                self.set_capital(idx as usize, capital - pay);
                self.set_pnl(idx as usize, pnl.saturating_add(u128_to_i128_clamped(pay)));
            }

            // Write off any remaining negative PnL (spec §6.1 step 4)
            if self.accounts[idx as usize].pnl.is_negative() {
                self.set_pnl(idx as usize, 0);
            }
        }

        Ok(())
    }

    /// Settle warmup: loss settlement + profit conversion per spec §6
    ///
    /// §6.1 Loss settlement: negative PnL pays from capital immediately.
    ///   If PnL still negative after capital exhausted, write off via set_pnl(i, 0).
    ///
    /// §6.2 Profit conversion: warmable gross profit converts to capital at haircut ratio h.
    ///   y = floor(x * h_num / h_den), where (h_num, h_den) is computed pre-conversion.
    pub fn settle_warmup_to_capital(&mut self, idx: u16) -> Result<()> {
        if !self.is_used(idx as usize) {
            return Err(RiskError::AccountNotFound);
        }

        // §6.1 Loss settlement (negative PnL → reduce capital immediately)
        let pnl = self.accounts[idx as usize].pnl.get();
        if pnl < 0 {
            let need = neg_i128_to_u128(pnl);
            let capital = self.accounts[idx as usize].capital.get();
            let pay = core::cmp::min(need, capital);

            if pay > 0 {
                self.set_capital(idx as usize, capital - pay);
                self.set_pnl(idx as usize, pnl.saturating_add(u128_to_i128_clamped(pay)));
            }

            // Write off any remaining negative PnL (spec §6.1 step 4)
            if self.accounts[idx as usize].pnl.is_negative() {
                self.set_pnl(idx as usize, 0);
            }
        }

        // §6.2 Profit conversion (warmup converts junior profit → protected principal)
        let pnl = self.accounts[idx as usize].pnl.get();
        if pnl > 0 {
            let positive_pnl = pnl as u128;
            let avail_gross = positive_pnl;

            // Compute warmable cap from slope and elapsed time (spec §5.3)
            let started_at = self.accounts[idx as usize].warmup_started_at_slot;
            let elapsed = self.current_slot.saturating_sub(started_at);
            let slope = self.accounts[idx as usize].warmup_slope_per_step.get();
            let cap = mul_u128(slope, elapsed as u128);

            let x = core::cmp::min(avail_gross, cap);

            if x > 0 {
                // Compute haircut ratio BEFORE modifying PnL/capital (spec §6.2)
                let (h_num, h_den) = self.haircut_ratio();
                let y = if h_den == 0 {
                    x
                } else {
                    mul_u128(x, h_num) / h_den
                };

                // Reduce junior profit claim by x
                self.set_pnl(idx as usize, pnl - (x as i128));
                // Increase protected principal by y
                let new_cap = add_u128(self.accounts[idx as usize].capital.get(), y);
                self.set_capital(idx as usize, new_cap);
            }

            // Advance warmup time base and update slope (spec §5.4)
            self.accounts[idx as usize].warmup_started_at_slot = self.current_slot;

            // Recompute warmup slope per spec §5.4
            let new_pnl = self.accounts[idx as usize].pnl.get();
            let new_avail = if new_pnl > 0 { new_pnl as u128 } else { 0 };
            let slope = if new_avail == 0 {
                0
            } else if self.params.warmup_period_slots > 0 {
                core::cmp::max(1, new_avail / (self.params.warmup_period_slots as u128))
            } else {
                new_avail
            };
            self.accounts[idx as usize].warmup_slope_per_step = U128::new(slope);
        }

        Ok(())
    }

    // Panic Settlement (Atomic Global Settle)
    // ========================================

    /// Top up insurance fund
    ///
    /// Adds tokens to both vault and insurance fund.
    /// Returns true if the top-up brings insurance above the risk reduction threshold.
    pub fn top_up_insurance_fund(&mut self, amount: u128) -> Result<bool> {
        // Add to vault
        self.vault = U128::new(add_u128(self.vault.get(), amount));

        // Add to insurance fund
        self.insurance_fund.balance =
            U128::new(add_u128(self.insurance_fund.balance.get(), amount));

        // Return whether we're now above the force-realize threshold
        let above_threshold = self.insurance_fund.balance > self.params.risk_reduction_threshold;
        Ok(above_threshold)
    }

    /// PERC-311: Fund the balance incentive reserve from trading fees.
    /// Called by wrapper after each trade's fee is computed.
    /// `fee_amount` is in engine units; `reserve_bps` is basis points of fee to reserve.
    pub fn fund_balance_reserve(&mut self, fee_amount: u128, reserve_bps: u16) {
        if reserve_bps == 0 || fee_amount == 0 {
            return;
        }
        let portion = fee_amount.saturating_mul(reserve_bps as u128) / 10_000;
        if portion > u64::MAX as u128 {
            return;
        }
        self.insurance_fund.balance_incentive_reserve = self
            .insurance_fund
            .balance_incentive_reserve
            .saturating_add(portion as u64);
    }

    /// PERC-311: Pay a skew-improvement rebate to a user.
    /// Returns the actual rebate paid (may be less than requested if reserve is low).
    ///
    /// `user_idx`: account to credit
    /// `rebate_amount`: requested rebate in engine units
    pub fn pay_skew_rebate(&mut self, user_idx: u16, rebate_amount: u64) -> u64 {
        if rebate_amount == 0 {
            return 0;
        }
        let reserve = self.insurance_fund.balance_incentive_reserve;
        let actual = core::cmp::min(rebate_amount, reserve);
        if actual == 0 {
            return 0;
        }
        self.insurance_fund.balance_incentive_reserve = reserve - actual;
        // Credit to user capital
        let old_cap = self.accounts[user_idx as usize].capital;
        self.accounts[user_idx as usize].capital = old_cap.saturating_add(actual as u128);
        // Update c_tot aggregate
        self.c_tot = U128::new(self.c_tot.get().saturating_add(actual as u128));
        actual
    }

    /// PERC-311: Compute whether a trade improves OI skew.
    /// Returns true if the user's new position reduces the absolute net LP position
    /// (i.e., the trade helps rebalance long/short OI).
    ///
    /// `net_lp_before`: net_lp_pos before trade
    /// `lp_delta`: LP's position change from this trade (negative of user's trade size)
    pub fn trade_improves_skew(net_lp_before: i128, lp_delta: i128) -> bool {
        let abs_before = net_lp_before.unsigned_abs();
        let net_lp_after = net_lp_before.saturating_add(lp_delta);
        let abs_after = net_lp_after.unsigned_abs();
        abs_after < abs_before
    }

    // ========================================
    // PERC-306: Per-Market Insurance Isolation
    // ========================================

    /// Fund the per-market isolated insurance balance.
    /// Tokens are already in the vault; this just credits the isolated pool.
    pub fn fund_market_insurance(&mut self, amount: u128) -> Result<()> {
        // Add to vault
        self.vault = U128::new(add_u128(self.vault.get(), amount));

        // Credit isolated balance (not the global insurance fund)
        self.insurance_fund.isolated_balance =
            U128::new(add_u128(self.insurance_fund.isolated_balance.get(), amount));

        Ok(())
    }

    /// Set insurance isolation BPS for this market's engine.
    pub fn set_insurance_isolation_bps(&mut self, bps: u16) {
        self.insurance_fund.insurance_isolation_bps = bps;
    }

    // ========================================
    // Utilities
    // ========================================

    /// Check conservation invariant (spec §3.1)
    ///
    /// Primary invariant: V >= C_tot + I
    ///
    /// Extended check: vault >= sum(capital) + sum(positive_pnl_clamped) + insurance
    /// with bounded rounding slack from funding/mark settlement.
    ///
    /// We also verify the full accounting identity including settled/unsettled PnL:
    /// vault >= sum(capital) + sum(settled_pnl + mark_pnl) + insurance
    /// The difference (slack) must be bounded by MAX_ROUNDING_SLACK.
    pub fn check_conservation(&self, oracle_price: u64) -> bool {
        let mut total_capital = 0u128;
        let mut net_pnl: i128 = 0;
        let mut net_mark: i128 = 0;
        let mut mark_ok = true;
        let global_index = self.funding_index_qpb_e6;

        self.for_each_used(|_idx, account| {
            total_capital = add_u128(total_capital, account.capital.get());

            // Compute "would-be settled" PNL for this account
            let mut settled_pnl = account.pnl.get();
            if !account.position_size.is_zero() {
                let delta_f = global_index
                    .get()
                    .saturating_sub(account.funding_index.get());
                if delta_f != 0 {
                    let raw = account.position_size.get().saturating_mul(delta_f);
                    // Use same symmetric truncation-toward-zero as settle_account_funding (PERC-492)
                    let payment = raw.saturating_div(1_000_000);
                    settled_pnl = settled_pnl.saturating_sub(payment);
                }

                match Self::mark_pnl_for_position(
                    account.position_size.get(),
                    account.entry_price,
                    oracle_price,
                ) {
                    Ok(mark) => {
                        net_mark = net_mark.saturating_add(mark);
                    }
                    Err(_) => {
                        mark_ok = false;
                    }
                }
            }
            net_pnl = net_pnl.saturating_add(settled_pnl);
        });

        if !mark_ok {
            return false;
        }

        // Conservation: vault >= C_tot + I + I_isolated (primary invariant)
        // PERC-306: Include isolated insurance balance in conservation check
        let total_insurance = self
            .insurance_fund
            .balance
            .get()
            .saturating_add(self.insurance_fund.isolated_balance.get());
        let primary = self.vault.get() >= total_capital.saturating_add(total_insurance);
        if !primary {
            return false;
        }

        // Extended: vault >= sum(capital) + sum(settled_pnl + mark_pnl) + insurance (global + isolated)
        let total_pnl = net_pnl.saturating_add(net_mark);
        let base = add_u128(total_capital, total_insurance);

        let expected = if total_pnl >= 0 {
            add_u128(base, total_pnl as u128)
        } else {
            base.saturating_sub(neg_i128_to_u128(total_pnl))
        };

        let actual = self.vault.get();

        if actual < expected {
            return false;
        }
        let slack = actual - expected;
        slack <= MAX_ROUNDING_SLACK
    }

    /// Advance to next slot (for testing warmup)
    pub fn advance_slot(&mut self, slots: u64) {
        self.current_slot = self.current_slot.saturating_add(slots);
    }
}

#[cfg(test)]
mod skew_rebate_tests {
    use super::*;

    /// Helper to run a closure on a thread with 8MB stack to avoid overflow
    /// from large RiskEngine (contains [Account; MAX_ACCOUNTS] on the stack).
    fn with_large_stack<F: FnOnce() + Send + 'static>(f: F) {
        extern crate std;
        let builder = std::thread::Builder::new().stack_size(8 * 1024 * 1024);
        let handle = builder.spawn(f).expect("failed to spawn thread");
        handle.join().expect("test thread panicked");
    }

    fn test_engine() -> RiskEngine {
        let params = RiskParams {
            warmup_period_slots: 10,
            maintenance_margin_bps: 500,
            initial_margin_bps: 1000,
            trading_fee_bps: 10,
            max_accounts: MAX_ACCOUNTS as u64,
            new_account_fee: U128::ZERO,
            risk_reduction_threshold: U128::ZERO,
            maintenance_fee_per_slot: U128::ZERO,
            max_crank_staleness_slots: u64::MAX,
            liquidation_fee_bps: 50,
            liquidation_fee_cap: U128::new(1_000_000),
            liquidation_buffer_bps: 100,
            min_liquidation_abs: U128::ZERO,
            funding_premium_weight_bps: 0,
            funding_settlement_interval_slots: 0,
            funding_premium_dampening_e6: 1_000_000,
            funding_premium_max_bps_per_slot: 5,
            partial_liquidation_bps: 0,
            partial_liquidation_cooldown_slots: 0,
            use_mark_price_for_liquidation: false,
            emergency_liquidation_margin_bps: 0,
            fee_tier2_bps: 0,
            fee_tier3_bps: 0,
            fee_tier2_threshold: 0,
            fee_tier3_threshold: 0,
            fee_split_lp_bps: 0,
            fee_split_protocol_bps: 0,
            fee_split_creator_bps: 10_000,
            fee_utilization_surge_bps: 0,
            min_nonzero_mm_req: 0,
            min_nonzero_im_req: 0,
            insurance_floor: U128::ZERO,
        };
        RiskEngine::new(params)
    }

    #[test]
    fn test_trade_improves_skew_reduces_net() {
        assert!(!RiskEngine::trade_improves_skew(-100, -10));
        assert!(RiskEngine::trade_improves_skew(-100, 10));
    }

    #[test]
    fn test_trade_improves_skew_from_zero() {
        assert!(!RiskEngine::trade_improves_skew(0, 10));
        assert!(!RiskEngine::trade_improves_skew(0, -10));
    }

    #[test]
    fn test_fund_balance_reserve() {
        with_large_stack(|| {
            let mut engine = test_engine();
            engine.fund_balance_reserve(10_000, 500);
            assert_eq!(engine.insurance_fund.balance_incentive_reserve, 500);
            engine.fund_balance_reserve(10_000, 0);
            assert_eq!(engine.insurance_fund.balance_incentive_reserve, 500);
        });
    }

    #[test]
    fn test_pay_skew_rebate_capped() {
        with_large_stack(|| {
            let mut engine = test_engine();
            engine.insurance_fund.balance_incentive_reserve = 100;
            let paid = engine.pay_skew_rebate(0, 200);
            assert_eq!(paid, 100);
            assert_eq!(engine.insurance_fund.balance_incentive_reserve, 0);
        });
    }

    #[test]
    fn test_pay_skew_rebate_exact() {
        with_large_stack(|| {
            let mut engine = test_engine();
            engine.insurance_fund.balance_incentive_reserve = 100;
            let paid = engine.pay_skew_rebate(0, 50);
            assert_eq!(paid, 50);
            assert_eq!(engine.insurance_fund.balance_incentive_reserve, 50);
        });
    }
}
