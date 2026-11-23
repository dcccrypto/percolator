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
/// The public B chunk: the most collateral atoms one B-settlement step or one
/// bankruptcy-residual booking step moves. A constant, not a market setting: a market
/// creator who could choose it could make it one atom, so that every social-loss
/// settlement crawls, or so large that every step overflows. No collateral amount
/// exceeds the largest vault, so this chunk never splits a settlement that fits in it.
pub const PUBLIC_B_CHUNK_ATOMS: u128 = MAX_VAULT_TVL;
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

// S10-X1: the dev-only differential monitor and its negative control must never reach an on-chain build.
#[cfg(all(target_os = "solana", any(feature = "x1-diff", feature = "x1-mutant-noclamp")))]
compile_error!("features `x1-diff` / `x1-mutant-noclamp` are test-only and must not be built for target_os = \"solana\"");

/// v2.2 Phase 4 items 1 + 2: pure band / rent arithmetic (see module docs).
pub mod band_rent;
// Under `fuzz` the module is public too, so fuzz-gated tests can reach the kernel
// helpers by path (upstream #250 `tests/v16_lien_claim_rate.rs` uses `percolator::v16::*`).
#[cfg(any(kani, feature = "fuzz"))]
pub mod v16;
#[cfg(not(any(kani, feature = "fuzz")))]
mod v16;
#[cfg(kani)]
pub mod wide_math;
#[cfg(all(not(kani), feature = "fork-facade"))]
pub mod wide_math;
#[cfg(all(not(kani), not(feature = "fork-facade")))]
mod wide_math;

#[cfg(kani)]
pub use v16::*;
#[cfg(all(not(kani), feature = "x1-diff"))]
pub use v16::x1_diff_stats;
#[cfg(not(kani))]
pub use v16::{
    adjust_slot_provider_principal, S10_MAX_MOVES_PER_INSTRUCTION, S10_MIN_MOVE_ATOMS, S10_PROTECT_FULL_PROVIDER_PRINCIPAL,
    active_bitmap_count_ones, active_bitmap_empty, active_bitmap_get, active_bitmap_is_empty,
    auto_crank_plan_requires_caller_observation, backing_domain_fee_split_for_lien_delta_num,
    band_initialize_asset, bankruptcy_hlock_domain_mask, bankruptcy_hlock_is_active,
    bankruptcy_hlock_is_unattributed, bankruptcy_hlock_mark_domain,
    bankruptcy_hlock_mark_unattributed, canonical_accrual_price_step_v16,
    canonical_band_accrual_price_step_v16, repay_pnl_postconditions_hold, v16_domain_count_for_market_slots,
    v16_domain_pair_for_asset_index, validate_bankruptcy_hlock_wire, AccrualStepV16,
    // #299 (2b) loss provenance wire + attribution rules
    decode_loss_provenance, effective_loss_provenance, encode_loss_provenance,
    liquidation_residual_charge, loss_provenance_after_pnl, LiquidationResidualChargeV16,
    LossProvenanceV16, LOSS_PROVENANCE_NONE, LOSS_PROVENANCE_UNATTRIBUTABLE,
    AccrueAssetOutcomeV16, ActionableSummaryV16, AdlWindDownBoundV16, AdlWindDownOutcomeV16,
    AdlWindDownRequestV16, KfDriftSideV16, KfDriftSideV16Account, AssetLifecycleV16, AssetStateV16, AssetStateV16Account,
    AutoCrankObservationV16, AutoCrankOutcomeV16, AutoCrankPlanV16, AutoCrankResultV16,
    AutoCrankWorkV16, BackingBucketStatusV16, BackingBucketV16, BackingBucketV16Account,
    BackingDomainFeeSplitV16, BandStepContextV16, BatchTradeOutcomeV16, CloseProgressLedgerV16,
    CloseProgressLedgerV16Account, DeadLegForfeitOutcomeV16, EngineAssetSlotV16Account,
    HealthCertV16, HealthCertV16Account, InsuranceCreditReservationV16,
    InsuranceCreditReservationV16Account, LiquidationOutcomeV16, LiquidationRequestV16, Market,
    MarketGroupV16HeaderAccount, MarketGroupV16View, MarketGroupV16ViewMut, MarketModeV16,
    MarketSlotV16View, MarketSlotV16ViewMut, PermissionlessCrankActionV16,
    PermissionlessCrankRequestV16, PermissionlessProgressOutcomeV16,
    PermissionlessRecoveryReasonV16, PortfolioAccountV16Account, PortfolioLegV16,
    PortfolioLegV16Account, PortfolioSourceDomainV16Account, PortfolioV16View, PortfolioV16ViewMut,
    ProvenanceHeaderV16, ProvenanceHeaderV16Account, RebalanceOutcomeV16, RebalanceRequestV16,
    ResolvedCloseOutcomeV16, ResolvedPayoutLedgerV16, ResolvedPayoutLedgerV16Account,
    ResolvedPayoutReceiptV16, ResolvedPayoutReceiptV16Account, SideModeV16, SideV16,
    SourceCreditStateV16, SourceCreditStateV16Account, TerminalSlabOutcomeV16, TradeRequestV16,
    V16ActiveBitmap, V16Config, V16ConfigAccount, V16Error, V16OptionalRecoveryReasonAccount,
    V16PodI128, V16PodU128, V16PodU16, V16PodU32, V16PodU64, V16Result,
    BANKRUPTCY_HLOCK_ACTIVE_BIT, BANKRUPTCY_HLOCK_MAX_ATTRIBUTED_DOMAINS,
    PORTFOLIO_LEG_V16_EMPTY_ACCOUNT, PORTFOLIO_SOURCE_DOMAIN_CAP,
    PROVIDER_PROTECTION_OBSERVED_ASSET_CAP, PROVIDER_PROTECTION_OBSERVED_MAX_ASSET_INDEX,
    TERMINAL_SLAB_SCAN_ASSETS_PER_CALL, V16_ACCOUNT_VERSION, V16_EMPTY_ACTIVE_BITMAP,
    V16_LAYOUT_DISCRIMINATOR, V16_MAX_ACCRUAL_PATH_STEPS, V16_MAX_PORTFOLIO_ASSETS_N,
};

// kani_active_bitmap_set is gated #[cfg(any(kani, test, feature="fork-facade"))]
// in v16.rs; re-export it under fork-facade so the wrapper tests can call it.
#[cfg(all(not(kani), feature = "fork-facade"))]
pub use v16::kani_active_bitmap_set;

// Loss-weight u128 path vs. the U512 computation it replaced (differential test only).
#[cfg(all(not(kani), feature = "fuzz"))]
pub use v16::{loss_weight_for_basis_for_test, loss_weight_for_basis_wide_reference_for_test};

// ADL effective-quantity kernels for the fuzz targets (kani builds get them from the blanket re-export).
#[cfg(all(not(kani), feature = "fuzz"))]
pub use v16::{
    kani_adl_effective_quantity_ceil, kani_funding_index_deltas,
    kani_raw_basis_after_adl_reduction, kani_raw_basis_for_adl_effective_quantity,
    kani_side_adl_index, kani_sync_side_adl_index,
};

// #449 (upstream c030aa40): the audit scan's per-side A-index predicate, for
// tests/adl_side_index_449.rs.
#[cfg(all(not(kani), feature = "fuzz", feature = "audit-scan"))]
pub use v16::kani_side_adl_totals_are_coherent;

// Bounded source-credit mul-div kernels + fused claim-burn deltas for the
// rounding-residue differential fuzz target (upstream 4c4dfb20).
#[cfg(all(not(kani), feature = "fuzz"))]
pub use v16::{
    kani_mul_div_ceil_u128_or_wide, kani_mul_div_ceil_u128_wide_reference,
    kani_mul_div_floor_u128_or_wide, kani_mul_div_floor_u128_wide_reference,
    kani_prepare_source_credit_domain_recompute_for_epoch,
    kani_prepare_source_credit_domain_recompute_for_epoch_steps,
    kani_prepare_source_positive_claim_burn_delta,
    kani_source_credit_state_realizable_support_for_claim_num,
};

// Source-credit rate u128 path vs. the U256 computation it replaced (differential test only).
#[cfg(all(not(kani), feature = "fuzz"))]
pub use v16::{source_credit_rate_num_for_test, source_credit_rate_num_wide_reference_for_test};

// v17 fork-facade re-exports — present only when the fork-facade feature is enabled (the wrapper
// opts in on its engine dep). Keeps the production frozen surface minimal by default. Under kani the
// blanket `pub use v16::*` above already covers these.
#[cfg(all(not(kani), feature = "fork-facade"))]
pub use v16::fork_facade;
#[cfg(all(not(kani), feature = "fork-facade"))]
pub use v16::lp_vault;
#[cfg(all(not(kani), feature = "fork-facade"))]
pub use v16::FeePolicyUpdateV16;
#[cfg(all(not(kani), feature = "fork-facade"))]
pub use v16::STRESS_ENVELOPE_TRIGGER_BPS_E9;
// Matrix row 38: wide_math is made pub mod under fork-facade (see mod declaration above)
// so the wrapper can use percolator::wide_math::U256 in expected_source_credit_rate_num.
// The module is private by default (frozen surface); fork-facade elevates it to pub.

// Source-domain all-zero tail check (test only).
#[cfg(all(not(kani), feature = "fuzz"))]
pub use v16::source_domain_tail_is_zero_for_test;

// Atom-alignment check without u128 modulo vs. `% BOUND_SCALE` (differential test only).
#[cfg(all(not(kani), feature = "fuzz"))]
pub use v16::bound_num_is_atom_aligned_for_test;

// Raw-byte emptiness of a decoded leg slot vs. PortfolioLegV16::is_empty (differential test only).
#[cfg(all(not(kani), feature = "fuzz"))]
pub use v16::decoded_leg_slot_is_empty_for_test;

// Source-domain compaction skip predicate vs. the full compaction (differential test only).
#[cfg(all(not(kani), feature = "fuzz"))]
pub use v16::{compact_source_domain_table_for_test, source_domain_table_already_compact_for_test};

// u128 mul-div cancelled-factor shortcut (differential test only).
#[cfg(all(not(kani), feature = "fuzz"))]
pub use v16::mul_div_cancelled_factor_for_test;

// PositionSlotMapV16::from_account vs. its pre-change loop (differential test only).
#[cfg(all(not(kani), feature = "fuzz"))]
pub use v16::position_slot_map_from_account_vs_reference_for_test;
