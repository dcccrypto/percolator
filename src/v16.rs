//! v16 account-local risk engine.
//!
//! This module implements the v16 slab-free engine surface: authenticated
//! portfolio accounts, bounded per-account refresh, lazy A/K/F/B settlement,
//! source-domain realizable credit, loss-senior fee handling, account-local
//! cranks, residual B booking, dynamic trade fees, liquidation progress checks,
//! and resolved account close.

use crate::wide_math::{
    checked_mul_div_ceil_u256, floor_div_signed_conservative_i128, mul_div_floor_u256_with_rem,
    wide_mul_div_floor_u128, wide_signed_mul_div_floor_from_k_pair, U256,
};
use crate::{
    ADL_ONE, BOUND_SCALE, CREDIT_RATE_SCALE, FUNDING_DEN, MAX_ACCOUNT_NOTIONAL, MAX_MARGIN_BPS,
    MAX_ORACLE_PRICE, MAX_POSITION_ABS_Q, MAX_PROTOCOL_FEE_ABS,
    MAX_RECOVERY_FALLBACK_DEVIATION_BPS, MAX_TRADE_SIZE_Q, MAX_VAULT_TVL, MIN_A_SIDE, POS_SCALE,
    SOCIAL_LOSS_DEN, SOCIAL_WEIGHT_SCALE,
};

pub const V16_MAX_PORTFOLIO_ASSETS_N: usize = 16;
pub const V16_DOMAIN_COUNT: usize = V16_MAX_PORTFOLIO_ASSETS_N * 2;
pub const V16_BACKING_BUCKETS_PER_DOMAIN: usize = 1;
pub const V16_LAYOUT_DISCRIMINATOR: u16 = 16;
pub const V16_ACCOUNT_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum V16Error {
    InvalidConfig,
    ArithmeticOverflow,
    ProvenanceMismatch,
    HiddenLeg,
    InvalidLeg,
    Stale,
    BStale,
    LockActive,
    NonProgress,
    RecoveryRequired,
    CounterOverflow,
    CounterUnderflow,
}

pub type V16Result<T> = core::result::Result<T, V16Error>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HLockLaneV16 {
    HMin,
    HMax,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SideV16 {
    Long,
    Short,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SideModeV16 {
    Normal,
    DrainOnly,
    ResetPending,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetLifecycleV16 {
    Disabled,
    PendingActivation,
    Active,
    DrainOnly,
    Retired,
    Recovery,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarketModeV16 {
    Live,
    Resolved,
    Recovery,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackingBucketStatusV16 {
    Empty,
    Fresh,
    Expired,
    Impaired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SourceCreditBackingSourceV16 {
    Counterparty,
    Insurance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionlessRecoveryReasonV16 {
    BelowProgressFloor,
    BlockedSegmentHeadroomOrRepresentability,
    AccountBSettlementCannotProgress,
    BIndexHeadroomExhausted,
    ActiveBankruptCloseCannotProgress,
    ExplicitLossOrDustAuditOverflow,
    OracleOrTargetUnavailableByAuthenticatedPolicy,
    CounterOrEpochOverflowDeclaredRecovery,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProvenanceHeaderV16 {
    pub market_group_id: [u8; 32],
    pub portfolio_account_id: [u8; 32],
    pub owner: [u8; 32],
    pub version: u16,
    pub layout_discriminator: u16,
}

impl ProvenanceHeaderV16 {
    pub const fn new(
        market_group_id: [u8; 32],
        portfolio_account_id: [u8; 32],
        owner: [u8; 32],
    ) -> Self {
        Self {
            market_group_id,
            portfolio_account_id,
            owner,
            version: V16_ACCOUNT_VERSION,
            layout_discriminator: V16_LAYOUT_DISCRIMINATOR,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct V16Config {
    pub max_portfolio_assets: u8,
    pub min_nonzero_mm_req: u128,
    pub min_nonzero_im_req: u128,
    pub h_min: u64,
    pub h_max: u64,
    pub maintenance_margin_bps: u64,
    pub initial_margin_bps: u64,
    pub max_trading_fee_bps: u64,
    pub liquidation_fee_bps: u64,
    pub liquidation_fee_cap: u128,
    pub min_liquidation_abs: u128,
    pub max_accrual_dt_slots: u64,
    pub max_abs_funding_e9_per_slot: u64,
    pub min_funding_lifetime_slots: u64,
    pub max_price_move_bps_per_slot: u64,
    pub max_account_b_settlement_chunks: u64,
    pub max_bankrupt_close_chunks: u64,
    pub max_bankrupt_close_lifetime_slots: u64,
    pub asset_activation_cooldown_slots: u64,
    pub public_b_chunk_atoms: u128,
    pub max_recovery_fallback_deviation_bps: u64,
    pub backing_freshness_buckets: u8,
    pub margin_mode_realizable_full_shared_cross_margin: bool,
    pub source_credit_lien_required: bool,
    pub insurance_credit_reservation_required: bool,
    pub permissionless_recovery_enabled: bool,
    pub recovery_fallback_price_enabled: bool,
    pub recovery_fallback_envelope_enabled: bool,
    pub credit_lien_revalidation_required: bool,
    pub stale_certificate_penalty_enabled: bool,
    pub full_refresh_required_for_favorable_actions: bool,
    pub public_liveness_profile_crank_forward: bool,
}

impl V16Config {
    pub const fn public_user_fund(max_portfolio_assets: u8, h_min: u64, h_max: u64) -> Self {
        Self {
            max_portfolio_assets,
            min_nonzero_mm_req: 1,
            min_nonzero_im_req: 2,
            h_min,
            h_max,
            maintenance_margin_bps: 10_000,
            initial_margin_bps: 10_000,
            max_trading_fee_bps: 0,
            liquidation_fee_bps: 0,
            liquidation_fee_cap: 0,
            min_liquidation_abs: 0,
            max_accrual_dt_slots: 1,
            max_abs_funding_e9_per_slot: 0,
            min_funding_lifetime_slots: 1,
            max_price_move_bps_per_slot: 10_000,
            max_account_b_settlement_chunks: 1,
            max_bankrupt_close_chunks: 1,
            max_bankrupt_close_lifetime_slots: 1,
            asset_activation_cooldown_slots: 1,
            public_b_chunk_atoms: MAX_VAULT_TVL,
            max_recovery_fallback_deviation_bps: MAX_RECOVERY_FALLBACK_DEVIATION_BPS,
            backing_freshness_buckets: V16_BACKING_BUCKETS_PER_DOMAIN as u8,
            margin_mode_realizable_full_shared_cross_margin: true,
            source_credit_lien_required: true,
            insurance_credit_reservation_required: true,
            permissionless_recovery_enabled: true,
            recovery_fallback_price_enabled: true,
            recovery_fallback_envelope_enabled: true,
            credit_lien_revalidation_required: true,
            stale_certificate_penalty_enabled: true,
            full_refresh_required_for_favorable_actions: true,
            public_liveness_profile_crank_forward: true,
        }
    }

    fn ceil_div_u256_to_u128(n: U256, d: U256) -> V16Result<u128> {
        if d.is_zero() {
            return Err(V16Error::InvalidConfig);
        }
        let q = n.checked_div(d).ok_or(V16Error::InvalidConfig)?;
        let r = n.checked_rem(d).ok_or(V16Error::InvalidConfig)?;
        let q = if r.is_zero() {
            q
        } else {
            q.checked_add(U256::ONE).ok_or(V16Error::InvalidConfig)?
        };
        q.try_into_u128().ok_or(V16Error::InvalidConfig)
    }

    fn checked_mul_div_ceil_to_u128(a: u128, b: u128, d: u128) -> V16Result<u128> {
        checked_mul_div_ceil_u256(U256::from_u128(a), U256::from_u128(b), U256::from_u128(d))
            .and_then(|v| v.try_into_u128())
            .ok_or(V16Error::InvalidConfig)
    }

    fn solvency_envelope_total_for_notional(
        &self,
        n: u128,
        loss_budget_num: u128,
        loss_budget_den: u128,
        price_budget_bps: u128,
    ) -> V16Result<u128> {
        let loss = Self::checked_mul_div_ceil_to_u128(n, loss_budget_num, loss_budget_den)?;

        let worst_liq_multiplier = 10_000u128
            .checked_add(price_budget_bps)
            .ok_or(V16Error::InvalidConfig)?;
        let worst_liq_notional =
            Self::checked_mul_div_ceil_to_u128(n, worst_liq_multiplier, 10_000)?;
        let liq_fee_raw = Self::checked_mul_div_ceil_to_u128(
            worst_liq_notional,
            self.liquidation_fee_bps as u128,
            10_000,
        )?;
        let liq_fee = core::cmp::min(
            core::cmp::max(liq_fee_raw, self.min_liquidation_abs),
            self.liquidation_fee_cap,
        );

        loss.checked_add(liq_fee).ok_or(V16Error::InvalidConfig)
    }

    fn maintenance_requirement_for_notional(&self, n: u128) -> V16Result<u128> {
        let mm_prop = U256::from_u128(n)
            .checked_mul(U256::from_u128(self.maintenance_margin_bps as u128))
            .and_then(|v| v.checked_div(U256::from_u128(10_000)))
            .and_then(|v| v.try_into_u128())
            .ok_or(V16Error::InvalidConfig)?;
        Ok(core::cmp::max(mm_prop, self.min_nonzero_mm_req))
    }

    fn solvency_envelope_holds_for_notional(
        &self,
        n: u128,
        loss_budget_num: u128,
        loss_budget_den: u128,
        price_budget_bps: u128,
    ) -> V16Result<bool> {
        let total = self.solvency_envelope_total_for_notional(
            n,
            loss_budget_num,
            loss_budget_den,
            price_budget_bps,
        )?;
        let mm_req = self.maintenance_requirement_for_notional(n)?;
        Ok(total <= mm_req)
    }

    fn solvency_envelope_interval_certifies(
        &self,
        lo: u128,
        hi: u128,
        loss_budget_num: u128,
        loss_budget_den: u128,
        price_budget_bps: u128,
    ) -> V16Result<bool> {
        let total_hi = self.solvency_envelope_total_for_notional(
            hi,
            loss_budget_num,
            loss_budget_den,
            price_budget_bps,
        )?;
        let mm_lo = self.maintenance_requirement_for_notional(lo)?;
        Ok(total_hi <= mm_lo)
    }

    fn validate_solvency_envelope_range(
        &self,
        lo: u128,
        hi: u128,
        loss_budget_num: u128,
        loss_budget_den: u128,
        price_budget_bps: u128,
    ) -> V16Result<()> {
        if lo > hi {
            return Ok(());
        }

        const MAX_SOLVENCY_INTERVALS: usize = 96;
        const MAX_SOLVENCY_STEPS: usize = 4096;
        const EXACT_CHUNK: u128 = 64;

        let mut stack = [(0u128, 0u128); MAX_SOLVENCY_INTERVALS];
        let mut len = 1usize;
        let mut steps = 0usize;
        stack[0] = (lo, hi);

        while len != 0 {
            steps = steps.checked_add(1).ok_or(V16Error::InvalidConfig)?;
            if steps > MAX_SOLVENCY_STEPS {
                return Err(V16Error::InvalidConfig);
            }

            len -= 1;
            let (range_lo, range_hi) = stack[len];

            if self.solvency_envelope_interval_certifies(
                range_lo,
                range_hi,
                loss_budget_num,
                loss_budget_den,
                price_budget_bps,
            )? {
                continue;
            }

            if range_hi == range_lo || range_hi - range_lo <= EXACT_CHUNK {
                let mut n = range_lo;
                loop {
                    if !self.solvency_envelope_holds_for_notional(
                        n,
                        loss_budget_num,
                        loss_budget_den,
                        price_budget_bps,
                    )? {
                        return Err(V16Error::InvalidConfig);
                    }
                    if n == range_hi {
                        break;
                    }
                    n = n.checked_add(1).ok_or(V16Error::InvalidConfig)?;
                }
                continue;
            }

            let mid = range_lo + (range_hi - range_lo) / 2;
            if len + 2 > MAX_SOLVENCY_INTERVALS {
                return Err(V16Error::InvalidConfig);
            }
            stack[len] = (mid.checked_add(1).ok_or(V16Error::InvalidConfig)?, range_hi);
            stack[len + 1] = (range_lo, mid);
            len += 2;
        }

        Ok(())
    }

    fn validate_funding_headroom(&self, slots: u64) -> V16Result<()> {
        let max_signed = U256::from_u128(i128::MAX as u128);
        let headroom = U256::from_u128(ADL_ONE)
            .checked_mul(U256::from_u128(MAX_ORACLE_PRICE as u128))
            .and_then(|v| v.checked_mul(U256::from_u128(self.max_abs_funding_e9_per_slot as u128)))
            .and_then(|v| v.checked_mul(U256::from_u128(slots as u128)))
            .ok_or(V16Error::InvalidConfig)?;
        if headroom <= max_signed {
            Ok(())
        } else {
            Err(V16Error::InvalidConfig)
        }
    }

    fn validate_exact_solvency_envelope(&self) -> V16Result<()> {
        let price_budget_fast = (self.max_price_move_bps_per_slot as u128)
            .checked_mul(self.max_accrual_dt_slots as u128)
            .ok_or(V16Error::InvalidConfig)?;
        if self.maintenance_margin_bps == 10_000
            && price_budget_fast <= 10_000
            && self.max_abs_funding_e9_per_slot == 0
            && self.liquidation_fee_bps == 0
            && self.min_liquidation_abs == 0
        {
            return Ok(());
        }

        self.validate_funding_headroom(self.max_accrual_dt_slots)?;
        self.validate_funding_headroom(self.min_funding_lifetime_slots)?;

        let move_cap = U256::from_u128(self.max_price_move_bps_per_slot as u128);
        let dt = U256::from_u128(self.max_accrual_dt_slots as u128);
        let rate = U256::from_u128(self.max_abs_funding_e9_per_slot as u128);
        let ten_thousand = U256::from_u128(10_000);
        let funding_den = U256::from_u128(FUNDING_DEN);

        let price_budget_bps = move_cap
            .checked_mul(dt)
            .and_then(|v| v.try_into_u128())
            .ok_or(V16Error::InvalidConfig)?;
        let funding_budget_num = rate
            .checked_mul(dt)
            .and_then(|v| v.checked_mul(ten_thousand))
            .ok_or(V16Error::InvalidConfig)?;
        let loss_budget_num_wide = U256::from_u128(price_budget_bps)
            .checked_mul(funding_den)
            .and_then(|v| v.checked_add(funding_budget_num))
            .ok_or(V16Error::InvalidConfig)?;
        let loss_budget_den_wide = ten_thousand
            .checked_mul(funding_den)
            .ok_or(V16Error::InvalidConfig)?;

        let funding_budget_bps_ceil = Self::ceil_div_u256_to_u128(funding_budget_num, funding_den)?;
        let loss_budget_bps_ceil = price_budget_bps
            .checked_add(funding_budget_bps_ceil)
            .ok_or(V16Error::InvalidConfig)?;
        let worst_liq_budget_bps_ceil = Self::ceil_div_u256_to_u128(
            U256::from_u128(
                10_000u128
                    .checked_add(price_budget_bps)
                    .ok_or(V16Error::InvalidConfig)?,
            )
            .checked_mul(U256::from_u128(self.liquidation_fee_bps as u128))
            .ok_or(V16Error::InvalidConfig)?,
            ten_thousand,
        )?;
        let linear_budget_bps = loss_budget_bps_ceil
            .checked_add(worst_liq_budget_bps_ceil)
            .ok_or(V16Error::InvalidConfig)?;

        if self.maintenance_margin_bps == 10_000
            && loss_budget_bps_ceil == 10_000
            && worst_liq_budget_bps_ceil == 0
            && self.min_liquidation_abs == 0
        {
            return Ok(());
        }

        let loss_budget_num = loss_budget_num_wide
            .try_into_u128()
            .ok_or(V16Error::InvalidConfig)?;
        let loss_budget_den = loss_budget_den_wide
            .try_into_u128()
            .ok_or(V16Error::InvalidConfig)?;
        let domain_max = MAX_ACCOUNT_NOTIONAL;

        if self.maintenance_margin_bps == 0 {
            if self.solvency_envelope_holds_for_notional(
                domain_max,
                loss_budget_num,
                loss_budget_den,
                price_budget_bps,
            )? {
                return Ok(());
            }
            return Err(V16Error::InvalidConfig);
        }

        let floor_region_max = U256::from_u128(
            self.min_nonzero_mm_req
                .checked_add(1)
                .ok_or(V16Error::InvalidConfig)?,
        )
        .checked_mul(ten_thousand)
        .and_then(|v| v.checked_sub(U256::ONE))
        .and_then(|v| v.checked_div(U256::from_u128(self.maintenance_margin_bps as u128)))
        .and_then(|v| v.try_into_u128())
        .ok_or(V16Error::InvalidConfig)?;
        let floor_region_end = core::cmp::min(floor_region_max, domain_max);
        if floor_region_end != 0
            && !self.solvency_envelope_holds_for_notional(
                floor_region_end,
                loss_budget_num,
                loss_budget_den,
                price_budget_bps,
            )?
        {
            return Err(V16Error::InvalidConfig);
        }
        if floor_region_max >= domain_max {
            return Ok(());
        }

        let exact_start = floor_region_end
            .checked_add(1)
            .ok_or(V16Error::InvalidConfig)?;

        if linear_budget_bps < self.maintenance_margin_bps as u128 {
            let slope_gap = (self.maintenance_margin_bps as u128) - linear_budget_bps;
            let tail_for_linear = Self::ceil_div_u256_to_u128(
                U256::from_u128(3 * 10_000),
                U256::from_u128(slope_gap),
            )?;

            let loss_gap = (self.maintenance_margin_bps as u128)
                .checked_sub(loss_budget_bps_ceil)
                .ok_or(V16Error::InvalidConfig)?;
            let floor_fee_slack = self
                .min_liquidation_abs
                .checked_add(2)
                .ok_or(V16Error::InvalidConfig)?;
            let tail_for_fee_floor = Self::ceil_div_u256_to_u128(
                U256::from_u128(floor_fee_slack)
                    .checked_mul(ten_thousand)
                    .ok_or(V16Error::InvalidConfig)?,
                U256::from_u128(loss_gap),
            )?;

            let exact_tail = core::cmp::max(tail_for_linear, tail_for_fee_floor);
            if exact_tail <= exact_start {
                return Ok(());
            }
            let exact_end = core::cmp::min(exact_tail.saturating_sub(1), domain_max);
            return self.validate_solvency_envelope_range(
                exact_start,
                exact_end,
                loss_budget_num,
                loss_budget_den,
                price_budget_bps,
            );
        }

        if loss_budget_bps_ceil >= self.maintenance_margin_bps as u128 {
            return self.validate_solvency_envelope_range(
                exact_start,
                domain_max,
                loss_budget_num,
                loss_budget_den,
                price_budget_bps,
            );
        }

        let slope_gap = (self.maintenance_margin_bps as u128) - loss_budget_bps_ceil;
        let capped_fee_slack = self
            .liquidation_fee_cap
            .checked_add(3)
            .ok_or(V16Error::InvalidConfig)?;
        let exact_tail = Self::ceil_div_u256_to_u128(
            U256::from_u128(capped_fee_slack)
                .checked_mul(ten_thousand)
                .ok_or(V16Error::InvalidConfig)?,
            U256::from_u128(slope_gap),
        )?;

        if exact_tail <= exact_start {
            return Ok(());
        }

        let exact_end = core::cmp::min(exact_tail.saturating_sub(1), domain_max);
        self.validate_solvency_envelope_range(
            exact_start,
            exact_end,
            loss_budget_num,
            loss_budget_den,
            price_budget_bps,
        )
    }

    pub fn validate_public_user_fund(&self) -> V16Result<()> {
        if self.max_portfolio_assets == 0
            || self.max_portfolio_assets as usize > V16_MAX_PORTFOLIO_ASSETS_N
        {
            return Err(V16Error::InvalidConfig);
        }
        if self.h_max == 0 || self.h_min > self.h_max {
            return Err(V16Error::InvalidConfig);
        }
        if self.min_nonzero_mm_req == 0 || self.min_nonzero_mm_req >= self.min_nonzero_im_req {
            return Err(V16Error::InvalidConfig);
        }
        if self.maintenance_margin_bps > self.initial_margin_bps
            || self.initial_margin_bps > MAX_MARGIN_BPS
            || self.max_trading_fee_bps > MAX_MARGIN_BPS
            || self.liquidation_fee_bps > MAX_MARGIN_BPS
            || self.min_liquidation_abs > self.liquidation_fee_cap
            || self.liquidation_fee_cap > MAX_PROTOCOL_FEE_ABS
            || self.max_accrual_dt_slots == 0
            || self.min_funding_lifetime_slots < self.max_accrual_dt_slots
            || self.max_abs_funding_e9_per_slot > 10_000
            || self.max_price_move_bps_per_slot == 0
            || self.max_account_b_settlement_chunks == 0
            || self.max_bankrupt_close_chunks == 0
            || self.max_bankrupt_close_lifetime_slots == 0
            || self.asset_activation_cooldown_slots == 0
            || self.public_b_chunk_atoms == 0
            || self.max_recovery_fallback_deviation_bps > MAX_RECOVERY_FALLBACK_DEVIATION_BPS
            || self.backing_freshness_buckets == 0
            || self.backing_freshness_buckets as usize > V16_BACKING_BUCKETS_PER_DOMAIN
        {
            return Err(V16Error::InvalidConfig);
        }
        if !self.margin_mode_realizable_full_shared_cross_margin
            || !self.source_credit_lien_required
            || !self.insurance_credit_reservation_required
            || !self.permissionless_recovery_enabled
            || !self.recovery_fallback_price_enabled
            || !self.recovery_fallback_envelope_enabled
            || !self.credit_lien_revalidation_required
            || !self.stale_certificate_penalty_enabled
            || !self.full_refresh_required_for_favorable_actions
            || !self.public_liveness_profile_crank_forward
        {
            return Err(V16Error::InvalidConfig);
        }
        self.validate_exact_solvency_envelope()
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssetStateV16 {
    pub lifecycle: AssetLifecycleV16,
    pub raw_oracle_target_price: u64,
    pub effective_price: u64,
    pub fund_px_last: u64,
    pub slot_last: u64,
    pub a_long: u128,
    pub a_short: u128,
    pub k_long: i128,
    pub k_short: i128,
    pub f_long_num: i128,
    pub f_short_num: i128,
    pub k_epoch_start_long: i128,
    pub k_epoch_start_short: i128,
    pub f_epoch_start_long_num: i128,
    pub f_epoch_start_short_num: i128,
    pub b_long_num: u128,
    pub b_short_num: u128,
    pub b_epoch_start_long_num: u128,
    pub b_epoch_start_short_num: u128,
    pub oi_eff_long_q: u128,
    pub oi_eff_short_q: u128,
    pub stored_pos_count_long: u64,
    pub stored_pos_count_short: u64,
    pub stale_account_count_long: u64,
    pub stale_account_count_short: u64,
    pub pending_obligation_count_long: u64,
    pub pending_obligation_count_short: u64,
    pub loss_weight_sum_long: u128,
    pub loss_weight_sum_short: u128,
    pub social_loss_remainder_long_num: u128,
    pub social_loss_remainder_short_num: u128,
    pub social_loss_dust_long_num: u128,
    pub social_loss_dust_short_num: u128,
    pub explicit_unallocated_loss_long: u128,
    pub explicit_unallocated_loss_short: u128,
    pub epoch_long: u64,
    pub epoch_short: u64,
    pub mode_long: SideModeV16,
    pub mode_short: SideModeV16,
}

impl Default for AssetStateV16 {
    fn default() -> Self {
        Self {
            lifecycle: AssetLifecycleV16::Active,
            raw_oracle_target_price: 1,
            effective_price: 1,
            fund_px_last: 1,
            slot_last: 0,
            a_long: ADL_ONE,
            a_short: ADL_ONE,
            k_long: 0,
            k_short: 0,
            f_long_num: 0,
            f_short_num: 0,
            k_epoch_start_long: 0,
            k_epoch_start_short: 0,
            f_epoch_start_long_num: 0,
            f_epoch_start_short_num: 0,
            b_long_num: 0,
            b_short_num: 0,
            b_epoch_start_long_num: 0,
            b_epoch_start_short_num: 0,
            oi_eff_long_q: 0,
            oi_eff_short_q: 0,
            stored_pos_count_long: 0,
            stored_pos_count_short: 0,
            stale_account_count_long: 0,
            stale_account_count_short: 0,
            pending_obligation_count_long: 0,
            pending_obligation_count_short: 0,
            loss_weight_sum_long: 0,
            loss_weight_sum_short: 0,
            social_loss_remainder_long_num: 0,
            social_loss_remainder_short_num: 0,
            social_loss_dust_long_num: 0,
            social_loss_dust_short_num: 0,
            explicit_unallocated_loss_long: 0,
            explicit_unallocated_loss_short: 0,
            epoch_long: 0,
            epoch_short: 0,
            mode_long: SideModeV16::Normal,
            mode_short: SideModeV16::Normal,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceCreditStateV16 {
    pub positive_claim_bound_num: u128,
    pub exact_positive_claim_num: u128,
    pub fresh_reserved_backing_num: u128,
    pub spent_backing_num: u128,
    pub valid_liened_backing_num: u128,
    pub impaired_liened_backing_num: u128,
    pub insurance_credit_reserved_num: u128,
    pub valid_liened_insurance_num: u128,
    pub impaired_liened_insurance_num: u128,
    pub credit_rate_num: u128,
    pub credit_epoch: u64,
}

impl SourceCreditStateV16 {
    pub const EMPTY: Self = Self {
        positive_claim_bound_num: 0,
        exact_positive_claim_num: 0,
        fresh_reserved_backing_num: 0,
        spent_backing_num: 0,
        valid_liened_backing_num: 0,
        impaired_liened_backing_num: 0,
        insurance_credit_reserved_num: 0,
        valid_liened_insurance_num: 0,
        impaired_liened_insurance_num: 0,
        credit_rate_num: CREDIT_RATE_SCALE,
        credit_epoch: 0,
    };
}

impl Default for SourceCreditStateV16 {
    fn default() -> Self {
        Self::EMPTY
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BackingBucketV16 {
    pub fresh_unliened_backing_num: u128,
    pub valid_liened_backing_num: u128,
    pub consumed_liened_backing_num: u128,
    pub impaired_liened_backing_num: u128,
    pub expiry_slot: u64,
    pub status: BackingBucketStatusV16,
}

impl BackingBucketV16 {
    pub const EMPTY: Self = Self {
        fresh_unliened_backing_num: 0,
        valid_liened_backing_num: 0,
        consumed_liened_backing_num: 0,
        impaired_liened_backing_num: 0,
        expiry_slot: 0,
        status: BackingBucketStatusV16::Empty,
    };
}

impl Default for BackingBucketV16 {
    fn default() -> Self {
        Self::EMPTY
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InsuranceCreditReservationV16 {
    pub insurance_credit_reserved_num: u128,
    pub valid_liened_insurance_num: u128,
    pub impaired_liened_insurance_num: u128,
    pub consumed_insurance_num: u128,
    pub source_credit_epoch: u64,
}

impl InsuranceCreditReservationV16 {
    pub const EMPTY: Self = Self {
        insurance_credit_reserved_num: 0,
        valid_liened_insurance_num: 0,
        impaired_liened_insurance_num: 0,
        consumed_insurance_num: 0,
        source_credit_epoch: 0,
    };
}

impl Default for InsuranceCreditReservationV16 {
    fn default() -> Self {
        Self::EMPTY
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PortfolioLegV16 {
    pub active: bool,
    pub side: SideV16,
    pub basis_pos_q: i128,
    pub a_basis: u128,
    pub k_snap: i128,
    pub f_snap: i128,
    pub epoch_snap: u64,
    pub loss_weight: u128,
    pub b_snap: u128,
    pub b_rem: u128,
    pub b_epoch_snap: u64,
    pub b_stale: bool,
    pub stale: bool,
}

impl PortfolioLegV16 {
    pub const EMPTY: Self = Self {
        active: false,
        side: SideV16::Long,
        basis_pos_q: 0,
        a_basis: ADL_ONE,
        k_snap: 0,
        f_snap: 0,
        epoch_snap: 0,
        loss_weight: 0,
        b_snap: 0,
        b_rem: 0,
        b_epoch_snap: 0,
        b_stale: false,
        stale: false,
    };
}

impl Default for PortfolioLegV16 {
    fn default() -> Self {
        Self::EMPTY
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct HealthCertV16 {
    pub certified_equity: i128,
    pub certified_initial_req: u128,
    pub certified_maintenance_req: u128,
    pub certified_liq_deficit: u128,
    pub certified_worst_case_loss: u128,
    pub cert_oracle_epoch: u64,
    pub cert_funding_epoch: u64,
    pub cert_risk_epoch: u64,
    pub cert_asset_set_epoch: u64,
    pub active_bitmap_at_cert: u32,
    pub valid: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CloseProgressLedgerV16 {
    pub active: bool,
    pub finalized: bool,
    pub canceled: bool,
    pub close_id: u64,
    pub asset_index: u8,
    pub domain_side: SideV16,
    pub gross_loss_at_close_start: u128,
    pub drift_reference_slot: u64,
    pub max_close_slot: u64,
    pub support_consumed: u128,
    pub junior_face_burned: u128,
    pub insurance_spent: u128,
    pub b_loss_booked: u128,
    pub explicit_loss_assigned: u128,
    pub quantity_adl_applied_q: u128,
    pub drift_consumed: u128,
    pub residual_remaining: u128,
}

impl CloseProgressLedgerV16 {
    pub const EMPTY: Self = Self {
        active: false,
        finalized: false,
        canceled: false,
        close_id: 0,
        asset_index: 0,
        domain_side: SideV16::Long,
        gross_loss_at_close_start: 0,
        drift_reference_slot: 0,
        max_close_slot: 0,
        support_consumed: 0,
        junior_face_burned: 0,
        insurance_spent: 0,
        b_loss_booked: 0,
        explicit_loss_assigned: 0,
        quantity_adl_applied_q: 0,
        drift_consumed: 0,
        residual_remaining: 0,
    };

    pub fn has_pending_residual(self) -> bool {
        self.active && !self.finalized && !self.canceled && self.residual_remaining != 0
    }

    pub fn has_irreversible_progress(self) -> bool {
        self.support_consumed != 0
            || self.junior_face_burned != 0
            || self.insurance_spent != 0
            || self.b_loss_booked != 0
            || self.explicit_loss_assigned != 0
            || self.quantity_adl_applied_q != 0
            || self.drift_consumed != 0
    }
}

impl Default for CloseProgressLedgerV16 {
    fn default() -> Self {
        Self::EMPTY
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedPayoutLedgerV16 {
    pub snapshot_residual: u128,
    pub terminal_claim_exact_receipts_num: u128,
    pub terminal_claim_bound_unreceipted_num: u128,
    pub current_payout_rate_num: u128,
    pub current_payout_rate_den: u128,
    pub snapshot_slot: u64,
    pub payout_halted: bool,
    pub finalized: bool,
}

impl ResolvedPayoutLedgerV16 {
    pub const EMPTY: Self = Self {
        snapshot_residual: 0,
        terminal_claim_exact_receipts_num: 0,
        terminal_claim_bound_unreceipted_num: 0,
        current_payout_rate_num: 0,
        current_payout_rate_den: 0,
        snapshot_slot: 0,
        payout_halted: false,
        finalized: false,
    };
}

impl Default for ResolvedPayoutLedgerV16 {
    fn default() -> Self {
        Self::EMPTY
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedPayoutReceiptV16 {
    pub present: bool,
    pub prior_bound_contribution_num: u128,
    pub live_released_face_at_receipt: u128,
    pub terminal_positive_claim_face: u128,
    pub paid_effective: u128,
    pub finalized: bool,
}

impl ResolvedPayoutReceiptV16 {
    pub const EMPTY: Self = Self {
        present: false,
        prior_bound_contribution_num: 0,
        live_released_face_at_receipt: 0,
        terminal_positive_claim_face: 0,
        paid_effective: 0,
        finalized: false,
    };
}

impl Default for ResolvedPayoutReceiptV16 {
    fn default() -> Self {
        Self::EMPTY
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PortfolioAccountV16 {
    pub provenance_header: ProvenanceHeaderV16,
    pub owner: [u8; 32],
    pub capital: u128,
    pub pnl: i128,
    pub reserved_pnl: u128,
    pub source_claim_bound_num: [u128; V16_DOMAIN_COUNT],
    pub source_claim_liened_num: [u128; V16_DOMAIN_COUNT],
    pub source_claim_counterparty_liened_num: [u128; V16_DOMAIN_COUNT],
    pub source_claim_insurance_liened_num: [u128; V16_DOMAIN_COUNT],
    pub source_lien_effective_reserved: [u128; V16_DOMAIN_COUNT],
    pub source_lien_counterparty_backing_num: [u128; V16_DOMAIN_COUNT],
    pub source_lien_insurance_backing_num: [u128; V16_DOMAIN_COUNT],
    pub source_claim_impaired_num: [u128; V16_DOMAIN_COUNT],
    pub source_lien_impaired_effective_reserved: [u128; V16_DOMAIN_COUNT],
    pub fee_credits: i128,
    pub cancel_deposit_escrow: u128,
    pub last_fee_slot: u64,
    pub active_bitmap: u32,
    pub legs: [PortfolioLegV16; V16_MAX_PORTFOLIO_ASSETS_N],
    pub health_cert: HealthCertV16,
    pub stale_state: bool,
    pub b_stale_state: bool,
    pub rebalance_lock: bool,
    pub liquidation_lock: bool,
    pub close_progress: CloseProgressLedgerV16,
    pub resolved_payout_receipt: ResolvedPayoutReceiptV16,
}

impl PortfolioAccountV16 {
    pub const fn empty(header: ProvenanceHeaderV16) -> Self {
        Self {
            provenance_header: header,
            owner: header.owner,
            capital: 0,
            pnl: 0,
            reserved_pnl: 0,
            source_claim_bound_num: [0; V16_DOMAIN_COUNT],
            source_claim_liened_num: [0; V16_DOMAIN_COUNT],
            source_claim_counterparty_liened_num: [0; V16_DOMAIN_COUNT],
            source_claim_insurance_liened_num: [0; V16_DOMAIN_COUNT],
            source_lien_effective_reserved: [0; V16_DOMAIN_COUNT],
            source_lien_counterparty_backing_num: [0; V16_DOMAIN_COUNT],
            source_lien_insurance_backing_num: [0; V16_DOMAIN_COUNT],
            source_claim_impaired_num: [0; V16_DOMAIN_COUNT],
            source_lien_impaired_effective_reserved: [0; V16_DOMAIN_COUNT],
            fee_credits: 0,
            cancel_deposit_escrow: 0,
            last_fee_slot: 0,
            active_bitmap: 0,
            legs: [PortfolioLegV16::EMPTY; V16_MAX_PORTFOLIO_ASSETS_N],
            health_cert: HealthCertV16 {
                certified_equity: 0,
                certified_initial_req: 0,
                certified_maintenance_req: 0,
                certified_liq_deficit: 0,
                certified_worst_case_loss: 0,
                cert_oracle_epoch: 0,
                cert_funding_epoch: 0,
                cert_risk_epoch: 0,
                cert_asset_set_epoch: 0,
                active_bitmap_at_cert: 0,
                valid: false,
            },
            stale_state: false,
            b_stale_state: false,
            rebalance_lock: false,
            liquidation_lock: false,
            close_progress: CloseProgressLedgerV16::EMPTY,
            resolved_payout_receipt: ResolvedPayoutReceiptV16::EMPTY,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarketGroupV16 {
    pub market_group_id: [u8; 32],
    pub config: V16Config,
    pub vault: u128,
    pub insurance: u128,
    pub c_tot: u128,
    pub pnl_pos_tot: u128,
    pub pnl_pos_bound_tot_num: u128,
    pub pnl_pos_bound_tot: u128,
    pub pnl_matured_pos_tot: u128,
    pub insurance_domain_budget: [u128; V16_DOMAIN_COUNT],
    pub insurance_domain_spent: [u128; V16_DOMAIN_COUNT],
    pub pending_domain_loss_barriers: [u64; V16_DOMAIN_COUNT],
    pub source_credit: [SourceCreditStateV16; V16_DOMAIN_COUNT],
    pub source_backing_buckets: [BackingBucketV16; V16_DOMAIN_COUNT],
    pub insurance_credit_reservations: [InsuranceCreditReservationV16; V16_DOMAIN_COUNT],
    pub materialized_portfolio_count: u64,
    pub stale_certificate_count: u64,
    pub b_stale_account_count: u64,
    pub negative_pnl_account_count: u64,
    pub risk_epoch: u64,
    pub asset_set_epoch: u64,
    pub asset_activation_count: u64,
    pub last_asset_activation_slot: u64,
    pub oracle_epoch: u64,
    pub funding_epoch: u64,
    pub slot_last: u64,
    pub current_slot: u64,
    pub assets: [AssetStateV16; V16_MAX_PORTFOLIO_ASSETS_N],
    pub bankruptcy_hlock_active: bool,
    pub threshold_stress_active: bool,
    pub loss_stale_active: bool,
    pub recovery_reason: Option<PermissionlessRecoveryReasonV16>,
    pub mode: MarketModeV16,
    pub resolved_slot: u64,
    pub payout_snapshot: u128,
    pub payout_snapshot_pnl_pos_tot: u128,
    pub payout_snapshot_captured: bool,
    pub resolved_payout_ledger: ResolvedPayoutLedgerV16,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccrueAssetOutcomeV16 {
    pub dt: u64,
    pub price_move_active: bool,
    pub funding_active: bool,
    pub equity_active: bool,
    pub loss_stale_after: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TradeRequestV16 {
    pub asset_index: usize,
    pub size_q: u128,
    pub exec_price: u64,
    pub fee_bps: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TradeOutcomeV16 {
    pub fee_a: u128,
    pub fee_b: u128,
    pub notional: u128,
}

pub const V16_TOKEN_VALUE_CLASS_COUNT: usize = 17;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenValueClassV16 {
    TokenVault = 0,
    SeniorCapital = 1,
    InsuranceCapital = 2,
    AccountCapital = 3,
    CloseSupportConsumed = 4,
    CloseInsuranceSpent = 5,
    CloseCounterpartyCreditConsumed = 6,
    BResidualBooked = 7,
    PendingObligationEscrow = 8,
    PendingObligationCredit = 9,
    ExplicitBackedLoss = 10,
    SettlementRoundingResidue = 11,
    CancelDepositEscrow = 12,
    ResolvedPayoutPaid = 13,
    ProtocolFeePaid = 14,
    ExternalQuote = 15,
    UnallocatedProtocolSurplus = 16,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenValueFlowProofV16 {
    pub debits: [u128; V16_TOKEN_VALUE_CLASS_COUNT],
    pub credits: [u128; V16_TOKEN_VALUE_CLASS_COUNT],
    pub external_quote_in: u128,
    pub external_quote_out: u128,
    pub vault_before: u128,
    pub vault_after: u128,
}

impl TokenValueFlowProofV16 {
    pub const fn empty(vault_before: u128, vault_after: u128) -> Self {
        Self {
            debits: [0; V16_TOKEN_VALUE_CLASS_COUNT],
            credits: [0; V16_TOKEN_VALUE_CLASS_COUNT],
            external_quote_in: 0,
            external_quote_out: 0,
            vault_before,
            vault_after,
        }
    }

    pub fn external_in_to_account_capital(
        amount: u128,
        vault_before: u128,
        vault_after: u128,
    ) -> V16Result<Self> {
        let mut proof = Self::empty(vault_before, vault_after);
        proof.external_quote_in = amount;
        proof.credit(TokenValueClassV16::ExternalQuote, amount)?;
        proof.debit(TokenValueClassV16::AccountCapital, amount)?;
        Ok(proof)
    }

    pub fn account_capital_to_external_out(
        amount: u128,
        vault_before: u128,
        vault_after: u128,
    ) -> V16Result<Self> {
        let mut proof = Self::empty(vault_before, vault_after);
        proof.external_quote_out = amount;
        proof.debit(TokenValueClassV16::AccountCapital, amount)?;
        proof.credit(TokenValueClassV16::ExternalQuote, amount)?;
        Ok(proof)
    }

    pub fn close_cure_to_account_capital(
        optional_external_deposit: u128,
        cancel_deposit_escrow: u128,
        capital_credit: u128,
        vault_before: u128,
        vault_after: u128,
    ) -> V16Result<Self> {
        let expected_credit = optional_external_deposit
            .checked_add(cancel_deposit_escrow)
            .ok_or(V16Error::ArithmeticOverflow)?;
        if expected_credit != capital_credit {
            return Err(V16Error::InvalidConfig);
        }
        let mut proof = Self::empty(vault_before, vault_after);
        proof.external_quote_in = optional_external_deposit;
        proof.credit(TokenValueClassV16::ExternalQuote, optional_external_deposit)?;
        proof.credit(
            TokenValueClassV16::CancelDepositEscrow,
            cancel_deposit_escrow,
        )?;
        proof.debit(TokenValueClassV16::AccountCapital, capital_credit)?;
        Ok(proof)
    }

    pub fn account_capital_to_insurance(
        amount: u128,
        vault_before: u128,
        vault_after: u128,
    ) -> V16Result<Self> {
        let mut proof = Self::empty(vault_before, vault_after);
        proof.debit(TokenValueClassV16::AccountCapital, amount)?;
        proof.credit(TokenValueClassV16::InsuranceCapital, amount)?;
        Ok(proof)
    }

    pub fn account_capital_to_realized_loss(
        amount: u128,
        vault_before: u128,
        vault_after: u128,
    ) -> V16Result<Self> {
        let mut proof = Self::empty(vault_before, vault_after);
        proof.debit(TokenValueClassV16::AccountCapital, amount)?;
        proof.credit(TokenValueClassV16::ExplicitBackedLoss, amount)?;
        Ok(proof)
    }

    pub fn insurance_to_close_insurance_spent(
        amount: u128,
        vault_before: u128,
        vault_after: u128,
    ) -> V16Result<Self> {
        let mut proof = Self::empty(vault_before, vault_after);
        proof.debit(TokenValueClassV16::InsuranceCapital, amount)?;
        proof.credit(TokenValueClassV16::CloseInsuranceSpent, amount)?;
        Ok(proof)
    }

    pub fn validate_insurance_to_close_insurance_spent(
        amount: u128,
        vault_before: u128,
        vault_after: u128,
    ) -> V16Result<()> {
        if vault_before != vault_after {
            return Err(V16Error::InvalidConfig);
        }
        let _ = amount;
        Ok(())
    }

    pub fn support_to_account_capital(
        account_capital_credit: u128,
        counterparty_credit_consumed: u128,
        insurance_credit_consumed: u128,
        protocol_surplus_consumed: u128,
        vault_before: u128,
        vault_after: u128,
    ) -> V16Result<Self> {
        let source_total = counterparty_credit_consumed
            .checked_add(insurance_credit_consumed)
            .and_then(|v| v.checked_add(protocol_surplus_consumed))
            .ok_or(V16Error::ArithmeticOverflow)?;
        if source_total != account_capital_credit {
            return Err(V16Error::InvalidConfig);
        }
        let mut proof = Self::empty(vault_before, vault_after);
        proof.credit(
            TokenValueClassV16::CloseCounterpartyCreditConsumed,
            counterparty_credit_consumed,
        )?;
        proof.credit(
            TokenValueClassV16::CloseInsuranceSpent,
            insurance_credit_consumed,
        )?;
        proof.credit(
            TokenValueClassV16::UnallocatedProtocolSurplus,
            protocol_surplus_consumed,
        )?;
        proof.debit(TokenValueClassV16::AccountCapital, account_capital_credit)?;
        Ok(proof)
    }

    pub fn capital_and_resolved_payout_to_external_out(
        capital_paid: u128,
        resolved_payout_paid: u128,
        total_external_out: u128,
        vault_before: u128,
        vault_after: u128,
    ) -> V16Result<Self> {
        let total_source = capital_paid
            .checked_add(resolved_payout_paid)
            .ok_or(V16Error::ArithmeticOverflow)?;
        if total_source != total_external_out {
            return Err(V16Error::InvalidConfig);
        }
        let mut proof = Self::empty(vault_before, vault_after);
        proof.external_quote_out = total_external_out;
        proof.debit(TokenValueClassV16::AccountCapital, capital_paid)?;
        proof.debit(TokenValueClassV16::ResolvedPayoutPaid, resolved_payout_paid)?;
        proof.credit(TokenValueClassV16::ExternalQuote, total_external_out)?;
        Ok(proof)
    }

    pub fn debit(&mut self, class: TokenValueClassV16, amount: u128) -> V16Result<()> {
        let idx = class as usize;
        self.debits[idx] = self.debits[idx]
            .checked_add(amount)
            .ok_or(V16Error::ArithmeticOverflow)?;
        Ok(())
    }

    pub fn credit(&mut self, class: TokenValueClassV16, amount: u128) -> V16Result<()> {
        let idx = class as usize;
        self.credits[idx] = self.credits[idx]
            .checked_add(amount)
            .ok_or(V16Error::ArithmeticOverflow)?;
        Ok(())
    }

    pub fn validate(&self) -> V16Result<()> {
        let mut total_debits = 0u128;
        let mut total_credits = 0u128;
        let mut i = 0;
        while i < V16_TOKEN_VALUE_CLASS_COUNT {
            total_debits = total_debits
                .checked_add(self.debits[i])
                .ok_or(V16Error::ArithmeticOverflow)?;
            total_credits = total_credits
                .checked_add(self.credits[i])
                .ok_or(V16Error::ArithmeticOverflow)?;
            i += 1;
        }
        if total_debits != total_credits {
            return Err(V16Error::InvalidConfig);
        }

        if self.vault_after >= self.vault_before {
            let vault_delta = self.vault_after - self.vault_before;
            if self.external_quote_in < self.external_quote_out
                || self.external_quote_in - self.external_quote_out != vault_delta
            {
                return Err(V16Error::InvalidConfig);
            }
        } else {
            let vault_delta = self.vault_before - self.vault_after;
            if self.external_quote_out < self.external_quote_in
                || self.external_quote_out - self.external_quote_in != vault_delta
            {
                return Err(V16Error::InvalidConfig);
            }
        }
        Ok(())
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReservationEncumbranceProofV16 {
    pub domain: u16,
    pub exact_positive_claim_num: u128,
    pub positive_claim_bound_num: u128,
    pub source_fresh_reserved_backing_num: u128,
    pub bucket_fresh_unliened_backing_num: u128,
    pub bucket_valid_liened_backing_num: u128,
    pub source_valid_liened_backing_num: u128,
    pub source_impaired_liened_backing_num: u128,
    pub bucket_impaired_liened_backing_num: u128,
    pub source_insurance_credit_reserved_num: u128,
    pub reservation_insurance_credit_reserved_num: u128,
    pub source_valid_liened_insurance_num: u128,
    pub reservation_valid_liened_insurance_num: u128,
    pub source_impaired_liened_insurance_num: u128,
    pub reservation_impaired_liened_insurance_num: u128,
    pub source_credit_rate_num: u128,
}

impl ReservationEncumbranceProofV16 {
    pub fn validate(&self) -> V16Result<()> {
        let fresh_reserved = self
            .bucket_fresh_unliened_backing_num
            .checked_add(self.bucket_valid_liened_backing_num)
            .ok_or(V16Error::ArithmeticOverflow)?;
        if self.source_fresh_reserved_backing_num != fresh_reserved
            || self.source_valid_liened_backing_num != self.bucket_valid_liened_backing_num
            || self.source_impaired_liened_backing_num != self.bucket_impaired_liened_backing_num
            || self.source_insurance_credit_reserved_num
                != self.reservation_insurance_credit_reserved_num
            || self.source_valid_liened_insurance_num != self.reservation_valid_liened_insurance_num
            || self.source_impaired_liened_insurance_num
                != self.reservation_impaired_liened_insurance_num
        {
            return Err(V16Error::InvalidConfig);
        }
        let insurance_encumbered = self
            .reservation_valid_liened_insurance_num
            .checked_add(self.reservation_impaired_liened_insurance_num)
            .ok_or(V16Error::ArithmeticOverflow)?;
        if self.reservation_insurance_credit_reserved_num < insurance_encumbered {
            return Err(V16Error::InvalidConfig);
        }
        let source = SourceCreditStateV16 {
            exact_positive_claim_num: self.exact_positive_claim_num,
            positive_claim_bound_num: self.positive_claim_bound_num,
            fresh_reserved_backing_num: self.source_fresh_reserved_backing_num,
            valid_liened_backing_num: self.source_valid_liened_backing_num,
            impaired_liened_backing_num: self.source_impaired_liened_backing_num,
            spent_backing_num: 0,
            insurance_credit_reserved_num: self.source_insurance_credit_reserved_num,
            valid_liened_insurance_num: self.source_valid_liened_insurance_num,
            impaired_liened_insurance_num: self.source_impaired_liened_insurance_num,
            credit_rate_num: self.source_credit_rate_num,
            credit_epoch: 0,
        };
        MarketGroupV16::validate_source_credit_state_static(source)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StockReconciliationProofV16 {
    pub token_vault: u128,
    pub senior_capital_total: u128,
    pub insurance_capital: u128,
    pub settlement_rounding_residue_total: u128,
    pub unallocated_protocol_surplus: u128,
}

impl StockReconciliationProofV16 {
    pub fn validate(&self) -> V16Result<()> {
        let accounted = self
            .senior_capital_total
            .checked_add(self.insurance_capital)
            .and_then(|v| v.checked_add(self.settlement_rounding_residue_total))
            .and_then(|v| v.checked_add(self.unallocated_protocol_surplus))
            .ok_or(V16Error::ArithmeticOverflow)?;
        if accounted != self.token_vault {
            return Err(V16Error::InvalidConfig);
        }
        Ok(())
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceCreditLienAggregateProofV16 {
    pub domain: u16,
    pub source_claim_bound_num: u128,
    pub face_claim_locked_num: u128,
    pub counterparty_face_claim_locked_num: u128,
    pub insurance_face_claim_locked_num: u128,
    pub effective_credit_reserved: u128,
    pub counterparty_backing_reserved_num: u128,
    pub insurance_backing_reserved_num: u128,
    pub impaired_face_claim_num: u128,
    pub impaired_effective_credit_reserved: u128,
}

impl SourceCreditLienAggregateProofV16 {
    pub fn validate(&self) -> V16Result<()> {
        let backing_face = self
            .counterparty_face_claim_locked_num
            .checked_add(self.insurance_face_claim_locked_num)
            .ok_or(V16Error::ArithmeticOverflow)?;
        if backing_face != self.face_claim_locked_num {
            return Err(V16Error::InvalidLeg);
        }
        let locked_or_impaired = self
            .face_claim_locked_num
            .checked_add(self.impaired_face_claim_num)
            .ok_or(V16Error::ArithmeticOverflow)?;
        if locked_or_impaired > self.source_claim_bound_num {
            return Err(V16Error::InvalidLeg);
        }
        if self.effective_credit_reserved
            > MarketGroupV16::amount_from_bound_num(self.face_claim_locked_num)?
        {
            return Err(V16Error::InvalidLeg);
        }
        if self.counterparty_backing_reserved_num % BOUND_SCALE != 0
            || self.insurance_backing_reserved_num % BOUND_SCALE != 0
        {
            return Err(V16Error::InvalidLeg);
        }
        let backing_num = self
            .counterparty_backing_reserved_num
            .checked_add(self.insurance_backing_reserved_num)
            .ok_or(V16Error::ArithmeticOverflow)?;
        let expected_backing_num = self
            .effective_credit_reserved
            .checked_mul(BOUND_SCALE)
            .ok_or(V16Error::ArithmeticOverflow)?;
        if backing_num != expected_backing_num {
            return Err(V16Error::InvalidLeg);
        }
        if self.impaired_effective_credit_reserved != 0 && self.impaired_face_claim_num == 0 {
            return Err(V16Error::InvalidLeg);
        }
        Ok(())
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiquidationRequestV16 {
    pub asset_index: usize,
    pub close_q: u128,
    pub fee_bps: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiquidationOutcomeV16 {
    pub closed_q: u128,
    pub insurance_used: u128,
    pub residual_booked: u128,
    pub explicit_loss: u128,
    pub fee_charged: u128,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeadLegForfeitOutcomeV16 {
    pub detached: bool,
    pub positive_pnl_forfeited: u128,
    pub loss_settled: u128,
    pub support_consumed: u128,
    pub junior_face_burned: u128,
    pub principal_used: u128,
    pub insurance_used: u128,
    pub residual_booked: u128,
    pub explicit_loss: u128,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SupportLossApplicationV16 {
    support_consumed: u128,
    junior_face_burned: u128,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SourceCreditConsumptionV16 {
    face_burn: u128,
    counterparty_credit_consumed: u128,
    insurance_credit_consumed: u128,
    domain_effective_consumed: [u128; V16_DOMAIN_COUNT],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RebalanceRequestV16 {
    pub asset_index: usize,
    pub reduce_q: u128,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RebalanceOutcomeV16 {
    pub reduced_q: u128,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BResidualBookingOutcomeV16 {
    pub booked_loss: u128,
    pub explicit_loss: u128,
    pub delta_b: u128,
    pub remaining_after: u128,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuantityAdlOutcomeV16 {
    pub closed_q: u128,
    pub opposite_a_after: u128,
    pub reset_started: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionlessCrankActionV16 {
    Refresh,
    SettleB { asset_index: usize },
    Liquidate(LiquidationRequestV16),
    Recover(PermissionlessRecoveryReasonV16),
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PermissionlessCrankRequestV16 {
    pub now_slot: u64,
    pub asset_index: usize,
    pub effective_price: u64,
    pub funding_rate_e9: i128,
    pub action: PermissionlessCrankActionV16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolvedCloseOutcomeV16 {
    ProgressOnly,
    Closed { payout: u128 },
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
pub struct V16PodU16 {
    pub bytes: [u8; 2],
}

impl V16PodU16 {
    pub fn new(value: u16) -> Self {
        Self {
            bytes: value.to_le_bytes(),
        }
    }

    pub fn get(self) -> u16 {
        u16::from_le_bytes(self.bytes)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
pub struct V16PodU32 {
    pub bytes: [u8; 4],
}

impl V16PodU32 {
    pub fn new(value: u32) -> Self {
        Self {
            bytes: value.to_le_bytes(),
        }
    }

    pub fn get(self) -> u32 {
        u32::from_le_bytes(self.bytes)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
pub struct V16PodU64 {
    pub bytes: [u8; 8],
}

impl V16PodU64 {
    pub fn new(value: u64) -> Self {
        Self {
            bytes: value.to_le_bytes(),
        }
    }

    pub fn get(self) -> u64 {
        u64::from_le_bytes(self.bytes)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
pub struct V16PodU128 {
    pub bytes: [u8; 16],
}

impl V16PodU128 {
    pub fn new(value: u128) -> Self {
        Self {
            bytes: value.to_le_bytes(),
        }
    }

    pub fn get(self) -> u128 {
        u128::from_le_bytes(self.bytes)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
pub struct V16PodI128 {
    pub bytes: [u8; 16],
}

impl V16PodI128 {
    pub fn new(value: i128) -> Self {
        Self {
            bytes: value.to_le_bytes(),
        }
    }

    pub fn get(self) -> i128 {
        i128::from_le_bytes(self.bytes)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
pub struct V16OptionalRecoveryReasonAccount {
    pub present: u8,
    pub value: u8,
}

impl V16OptionalRecoveryReasonAccount {
    pub fn from_runtime(value: Option<PermissionlessRecoveryReasonV16>) -> Self {
        match value {
            Some(reason) => Self {
                present: 1,
                value: encode_recovery_reason(reason),
            },
            None => Self {
                present: 0,
                value: 0,
            },
        }
    }

    pub fn try_to_runtime(self) -> V16Result<Option<PermissionlessRecoveryReasonV16>> {
        match self.present {
            0 if self.value == 0 => Ok(None),
            1 => Ok(Some(decode_recovery_reason(self.value)?)),
            _ => Err(V16Error::InvalidConfig),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
pub struct ProvenanceHeaderV16Account {
    pub market_group_id: [u8; 32],
    pub portfolio_account_id: [u8; 32],
    pub owner: [u8; 32],
    pub version: V16PodU16,
    pub layout_discriminator: V16PodU16,
}

impl ProvenanceHeaderV16Account {
    pub fn from_runtime(value: &ProvenanceHeaderV16) -> Self {
        Self {
            market_group_id: value.market_group_id,
            portfolio_account_id: value.portfolio_account_id,
            owner: value.owner,
            version: V16PodU16::new(value.version),
            layout_discriminator: V16PodU16::new(value.layout_discriminator),
        }
    }

    pub fn try_to_runtime(&self) -> V16Result<ProvenanceHeaderV16> {
        let out = ProvenanceHeaderV16 {
            market_group_id: self.market_group_id,
            portfolio_account_id: self.portfolio_account_id,
            owner: self.owner,
            version: self.version.get(),
            layout_discriminator: self.layout_discriminator.get(),
        };
        if out.version != V16_ACCOUNT_VERSION
            || out.layout_discriminator != V16_LAYOUT_DISCRIMINATOR
        {
            return Err(V16Error::ProvenanceMismatch);
        }
        Ok(out)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
pub struct V16ConfigAccount {
    pub max_portfolio_assets: u8,
    pub min_nonzero_mm_req: V16PodU128,
    pub min_nonzero_im_req: V16PodU128,
    pub h_min: V16PodU64,
    pub h_max: V16PodU64,
    pub maintenance_margin_bps: V16PodU64,
    pub initial_margin_bps: V16PodU64,
    pub max_trading_fee_bps: V16PodU64,
    pub liquidation_fee_bps: V16PodU64,
    pub liquidation_fee_cap: V16PodU128,
    pub min_liquidation_abs: V16PodU128,
    pub max_accrual_dt_slots: V16PodU64,
    pub max_abs_funding_e9_per_slot: V16PodU64,
    pub min_funding_lifetime_slots: V16PodU64,
    pub max_price_move_bps_per_slot: V16PodU64,
    pub max_account_b_settlement_chunks: V16PodU64,
    pub max_bankrupt_close_chunks: V16PodU64,
    pub max_bankrupt_close_lifetime_slots: V16PodU64,
    pub asset_activation_cooldown_slots: V16PodU64,
    pub public_b_chunk_atoms: V16PodU128,
    pub max_recovery_fallback_deviation_bps: V16PodU64,
    pub backing_freshness_buckets: u8,
    pub margin_mode_realizable_full_shared_cross_margin: u8,
    pub source_credit_lien_required: u8,
    pub insurance_credit_reservation_required: u8,
    pub permissionless_recovery_enabled: u8,
    pub recovery_fallback_price_enabled: u8,
    pub recovery_fallback_envelope_enabled: u8,
    pub credit_lien_revalidation_required: u8,
    pub stale_certificate_penalty_enabled: u8,
    pub full_refresh_required_for_favorable_actions: u8,
    pub public_liveness_profile_crank_forward: u8,
}

impl V16ConfigAccount {
    pub fn from_runtime(value: &V16Config) -> Self {
        Self {
            max_portfolio_assets: value.max_portfolio_assets,
            min_nonzero_mm_req: V16PodU128::new(value.min_nonzero_mm_req),
            min_nonzero_im_req: V16PodU128::new(value.min_nonzero_im_req),
            h_min: V16PodU64::new(value.h_min),
            h_max: V16PodU64::new(value.h_max),
            maintenance_margin_bps: V16PodU64::new(value.maintenance_margin_bps),
            initial_margin_bps: V16PodU64::new(value.initial_margin_bps),
            max_trading_fee_bps: V16PodU64::new(value.max_trading_fee_bps),
            liquidation_fee_bps: V16PodU64::new(value.liquidation_fee_bps),
            liquidation_fee_cap: V16PodU128::new(value.liquidation_fee_cap),
            min_liquidation_abs: V16PodU128::new(value.min_liquidation_abs),
            max_accrual_dt_slots: V16PodU64::new(value.max_accrual_dt_slots),
            max_abs_funding_e9_per_slot: V16PodU64::new(value.max_abs_funding_e9_per_slot),
            min_funding_lifetime_slots: V16PodU64::new(value.min_funding_lifetime_slots),
            max_price_move_bps_per_slot: V16PodU64::new(value.max_price_move_bps_per_slot),
            max_account_b_settlement_chunks: V16PodU64::new(value.max_account_b_settlement_chunks),
            max_bankrupt_close_chunks: V16PodU64::new(value.max_bankrupt_close_chunks),
            max_bankrupt_close_lifetime_slots: V16PodU64::new(
                value.max_bankrupt_close_lifetime_slots,
            ),
            asset_activation_cooldown_slots: V16PodU64::new(value.asset_activation_cooldown_slots),
            public_b_chunk_atoms: V16PodU128::new(value.public_b_chunk_atoms),
            max_recovery_fallback_deviation_bps: V16PodU64::new(
                value.max_recovery_fallback_deviation_bps,
            ),
            backing_freshness_buckets: value.backing_freshness_buckets,
            margin_mode_realizable_full_shared_cross_margin: encode_bool(
                value.margin_mode_realizable_full_shared_cross_margin,
            ),
            source_credit_lien_required: encode_bool(value.source_credit_lien_required),
            insurance_credit_reservation_required: encode_bool(
                value.insurance_credit_reservation_required,
            ),
            permissionless_recovery_enabled: encode_bool(value.permissionless_recovery_enabled),
            recovery_fallback_price_enabled: encode_bool(value.recovery_fallback_price_enabled),
            recovery_fallback_envelope_enabled: encode_bool(
                value.recovery_fallback_envelope_enabled,
            ),
            credit_lien_revalidation_required: encode_bool(value.credit_lien_revalidation_required),
            stale_certificate_penalty_enabled: encode_bool(value.stale_certificate_penalty_enabled),
            full_refresh_required_for_favorable_actions: encode_bool(
                value.full_refresh_required_for_favorable_actions,
            ),
            public_liveness_profile_crank_forward: encode_bool(
                value.public_liveness_profile_crank_forward,
            ),
        }
    }

    pub fn try_to_runtime(&self) -> V16Result<V16Config> {
        let out = V16Config {
            max_portfolio_assets: self.max_portfolio_assets,
            min_nonzero_mm_req: self.min_nonzero_mm_req.get(),
            min_nonzero_im_req: self.min_nonzero_im_req.get(),
            h_min: self.h_min.get(),
            h_max: self.h_max.get(),
            maintenance_margin_bps: self.maintenance_margin_bps.get(),
            initial_margin_bps: self.initial_margin_bps.get(),
            max_trading_fee_bps: self.max_trading_fee_bps.get(),
            liquidation_fee_bps: self.liquidation_fee_bps.get(),
            liquidation_fee_cap: self.liquidation_fee_cap.get(),
            min_liquidation_abs: self.min_liquidation_abs.get(),
            max_accrual_dt_slots: self.max_accrual_dt_slots.get(),
            max_abs_funding_e9_per_slot: self.max_abs_funding_e9_per_slot.get(),
            min_funding_lifetime_slots: self.min_funding_lifetime_slots.get(),
            max_price_move_bps_per_slot: self.max_price_move_bps_per_slot.get(),
            max_account_b_settlement_chunks: self.max_account_b_settlement_chunks.get(),
            max_bankrupt_close_chunks: self.max_bankrupt_close_chunks.get(),
            max_bankrupt_close_lifetime_slots: self.max_bankrupt_close_lifetime_slots.get(),
            asset_activation_cooldown_slots: self.asset_activation_cooldown_slots.get(),
            public_b_chunk_atoms: self.public_b_chunk_atoms.get(),
            max_recovery_fallback_deviation_bps: self.max_recovery_fallback_deviation_bps.get(),
            backing_freshness_buckets: self.backing_freshness_buckets,
            margin_mode_realizable_full_shared_cross_margin: decode_bool(
                self.margin_mode_realizable_full_shared_cross_margin,
            )?,
            source_credit_lien_required: decode_bool(self.source_credit_lien_required)?,
            insurance_credit_reservation_required: decode_bool(
                self.insurance_credit_reservation_required,
            )?,
            permissionless_recovery_enabled: decode_bool(self.permissionless_recovery_enabled)?,
            recovery_fallback_price_enabled: decode_bool(self.recovery_fallback_price_enabled)?,
            recovery_fallback_envelope_enabled: decode_bool(
                self.recovery_fallback_envelope_enabled,
            )?,
            credit_lien_revalidation_required: decode_bool(self.credit_lien_revalidation_required)?,
            stale_certificate_penalty_enabled: decode_bool(self.stale_certificate_penalty_enabled)?,
            full_refresh_required_for_favorable_actions: decode_bool(
                self.full_refresh_required_for_favorable_actions,
            )?,
            public_liveness_profile_crank_forward: decode_bool(
                self.public_liveness_profile_crank_forward,
            )?,
        };
        out.validate_public_user_fund()?;
        Ok(out)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
pub struct SourceCreditStateV16Account {
    pub positive_claim_bound_num: V16PodU128,
    pub exact_positive_claim_num: V16PodU128,
    pub fresh_reserved_backing_num: V16PodU128,
    pub spent_backing_num: V16PodU128,
    pub valid_liened_backing_num: V16PodU128,
    pub impaired_liened_backing_num: V16PodU128,
    pub insurance_credit_reserved_num: V16PodU128,
    pub valid_liened_insurance_num: V16PodU128,
    pub impaired_liened_insurance_num: V16PodU128,
    pub credit_rate_num: V16PodU128,
    pub credit_epoch: V16PodU64,
}

impl SourceCreditStateV16Account {
    pub fn from_runtime(value: &SourceCreditStateV16) -> Self {
        Self {
            positive_claim_bound_num: V16PodU128::new(value.positive_claim_bound_num),
            exact_positive_claim_num: V16PodU128::new(value.exact_positive_claim_num),
            fresh_reserved_backing_num: V16PodU128::new(value.fresh_reserved_backing_num),
            spent_backing_num: V16PodU128::new(value.spent_backing_num),
            valid_liened_backing_num: V16PodU128::new(value.valid_liened_backing_num),
            impaired_liened_backing_num: V16PodU128::new(value.impaired_liened_backing_num),
            insurance_credit_reserved_num: V16PodU128::new(value.insurance_credit_reserved_num),
            valid_liened_insurance_num: V16PodU128::new(value.valid_liened_insurance_num),
            impaired_liened_insurance_num: V16PodU128::new(value.impaired_liened_insurance_num),
            credit_rate_num: V16PodU128::new(value.credit_rate_num),
            credit_epoch: V16PodU64::new(value.credit_epoch),
        }
    }

    pub fn try_to_runtime(&self) -> V16Result<SourceCreditStateV16> {
        let out = SourceCreditStateV16 {
            positive_claim_bound_num: self.positive_claim_bound_num.get(),
            exact_positive_claim_num: self.exact_positive_claim_num.get(),
            fresh_reserved_backing_num: self.fresh_reserved_backing_num.get(),
            spent_backing_num: self.spent_backing_num.get(),
            valid_liened_backing_num: self.valid_liened_backing_num.get(),
            impaired_liened_backing_num: self.impaired_liened_backing_num.get(),
            insurance_credit_reserved_num: self.insurance_credit_reserved_num.get(),
            valid_liened_insurance_num: self.valid_liened_insurance_num.get(),
            impaired_liened_insurance_num: self.impaired_liened_insurance_num.get(),
            credit_rate_num: self.credit_rate_num.get(),
            credit_epoch: self.credit_epoch.get(),
        };
        MarketGroupV16::validate_source_credit_state_static(out)?;
        Ok(out)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
pub struct BackingBucketV16Account {
    pub fresh_unliened_backing_num: V16PodU128,
    pub valid_liened_backing_num: V16PodU128,
    pub consumed_liened_backing_num: V16PodU128,
    pub impaired_liened_backing_num: V16PodU128,
    pub expiry_slot: V16PodU64,
    pub status: u8,
}

impl BackingBucketV16Account {
    pub fn from_runtime(value: &BackingBucketV16) -> Self {
        Self {
            fresh_unliened_backing_num: V16PodU128::new(value.fresh_unliened_backing_num),
            valid_liened_backing_num: V16PodU128::new(value.valid_liened_backing_num),
            consumed_liened_backing_num: V16PodU128::new(value.consumed_liened_backing_num),
            impaired_liened_backing_num: V16PodU128::new(value.impaired_liened_backing_num),
            expiry_slot: V16PodU64::new(value.expiry_slot),
            status: encode_backing_bucket_status(value.status),
        }
    }

    pub fn try_to_runtime(&self) -> V16Result<BackingBucketV16> {
        let out = BackingBucketV16 {
            fresh_unliened_backing_num: self.fresh_unliened_backing_num.get(),
            valid_liened_backing_num: self.valid_liened_backing_num.get(),
            consumed_liened_backing_num: self.consumed_liened_backing_num.get(),
            impaired_liened_backing_num: self.impaired_liened_backing_num.get(),
            expiry_slot: self.expiry_slot.get(),
            status: decode_backing_bucket_status(self.status)?,
        };
        MarketGroupV16::validate_backing_bucket_static(out)?;
        Ok(out)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
pub struct InsuranceCreditReservationV16Account {
    pub insurance_credit_reserved_num: V16PodU128,
    pub valid_liened_insurance_num: V16PodU128,
    pub impaired_liened_insurance_num: V16PodU128,
    pub consumed_insurance_num: V16PodU128,
    pub source_credit_epoch: V16PodU64,
}

impl InsuranceCreditReservationV16Account {
    pub fn from_runtime(value: &InsuranceCreditReservationV16) -> Self {
        Self {
            insurance_credit_reserved_num: V16PodU128::new(value.insurance_credit_reserved_num),
            valid_liened_insurance_num: V16PodU128::new(value.valid_liened_insurance_num),
            impaired_liened_insurance_num: V16PodU128::new(value.impaired_liened_insurance_num),
            consumed_insurance_num: V16PodU128::new(value.consumed_insurance_num),
            source_credit_epoch: V16PodU64::new(value.source_credit_epoch),
        }
    }

    pub fn try_to_runtime(&self) -> V16Result<InsuranceCreditReservationV16> {
        let out = InsuranceCreditReservationV16 {
            insurance_credit_reserved_num: self.insurance_credit_reserved_num.get(),
            valid_liened_insurance_num: self.valid_liened_insurance_num.get(),
            impaired_liened_insurance_num: self.impaired_liened_insurance_num.get(),
            consumed_insurance_num: self.consumed_insurance_num.get(),
            source_credit_epoch: self.source_credit_epoch.get(),
        };
        MarketGroupV16::validate_insurance_reservation_static(out)?;
        Ok(out)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
pub struct AssetStateV16Account {
    pub lifecycle: u8,
    pub raw_oracle_target_price: V16PodU64,
    pub effective_price: V16PodU64,
    pub fund_px_last: V16PodU64,
    pub slot_last: V16PodU64,
    pub a_long: V16PodU128,
    pub a_short: V16PodU128,
    pub k_long: V16PodI128,
    pub k_short: V16PodI128,
    pub f_long_num: V16PodI128,
    pub f_short_num: V16PodI128,
    pub k_epoch_start_long: V16PodI128,
    pub k_epoch_start_short: V16PodI128,
    pub f_epoch_start_long_num: V16PodI128,
    pub f_epoch_start_short_num: V16PodI128,
    pub b_long_num: V16PodU128,
    pub b_short_num: V16PodU128,
    pub b_epoch_start_long_num: V16PodU128,
    pub b_epoch_start_short_num: V16PodU128,
    pub oi_eff_long_q: V16PodU128,
    pub oi_eff_short_q: V16PodU128,
    pub stored_pos_count_long: V16PodU64,
    pub stored_pos_count_short: V16PodU64,
    pub stale_account_count_long: V16PodU64,
    pub stale_account_count_short: V16PodU64,
    pub pending_obligation_count_long: V16PodU64,
    pub pending_obligation_count_short: V16PodU64,
    pub loss_weight_sum_long: V16PodU128,
    pub loss_weight_sum_short: V16PodU128,
    pub social_loss_remainder_long_num: V16PodU128,
    pub social_loss_remainder_short_num: V16PodU128,
    pub social_loss_dust_long_num: V16PodU128,
    pub social_loss_dust_short_num: V16PodU128,
    pub explicit_unallocated_loss_long: V16PodU128,
    pub explicit_unallocated_loss_short: V16PodU128,
    pub epoch_long: V16PodU64,
    pub epoch_short: V16PodU64,
    pub mode_long: u8,
    pub mode_short: u8,
}

impl AssetStateV16Account {
    pub fn from_runtime(value: &AssetStateV16) -> Self {
        Self {
            lifecycle: encode_asset_lifecycle(value.lifecycle),
            raw_oracle_target_price: V16PodU64::new(value.raw_oracle_target_price),
            effective_price: V16PodU64::new(value.effective_price),
            fund_px_last: V16PodU64::new(value.fund_px_last),
            slot_last: V16PodU64::new(value.slot_last),
            a_long: V16PodU128::new(value.a_long),
            a_short: V16PodU128::new(value.a_short),
            k_long: V16PodI128::new(value.k_long),
            k_short: V16PodI128::new(value.k_short),
            f_long_num: V16PodI128::new(value.f_long_num),
            f_short_num: V16PodI128::new(value.f_short_num),
            k_epoch_start_long: V16PodI128::new(value.k_epoch_start_long),
            k_epoch_start_short: V16PodI128::new(value.k_epoch_start_short),
            f_epoch_start_long_num: V16PodI128::new(value.f_epoch_start_long_num),
            f_epoch_start_short_num: V16PodI128::new(value.f_epoch_start_short_num),
            b_long_num: V16PodU128::new(value.b_long_num),
            b_short_num: V16PodU128::new(value.b_short_num),
            b_epoch_start_long_num: V16PodU128::new(value.b_epoch_start_long_num),
            b_epoch_start_short_num: V16PodU128::new(value.b_epoch_start_short_num),
            oi_eff_long_q: V16PodU128::new(value.oi_eff_long_q),
            oi_eff_short_q: V16PodU128::new(value.oi_eff_short_q),
            stored_pos_count_long: V16PodU64::new(value.stored_pos_count_long),
            stored_pos_count_short: V16PodU64::new(value.stored_pos_count_short),
            stale_account_count_long: V16PodU64::new(value.stale_account_count_long),
            stale_account_count_short: V16PodU64::new(value.stale_account_count_short),
            pending_obligation_count_long: V16PodU64::new(value.pending_obligation_count_long),
            pending_obligation_count_short: V16PodU64::new(value.pending_obligation_count_short),
            loss_weight_sum_long: V16PodU128::new(value.loss_weight_sum_long),
            loss_weight_sum_short: V16PodU128::new(value.loss_weight_sum_short),
            social_loss_remainder_long_num: V16PodU128::new(value.social_loss_remainder_long_num),
            social_loss_remainder_short_num: V16PodU128::new(value.social_loss_remainder_short_num),
            social_loss_dust_long_num: V16PodU128::new(value.social_loss_dust_long_num),
            social_loss_dust_short_num: V16PodU128::new(value.social_loss_dust_short_num),
            explicit_unallocated_loss_long: V16PodU128::new(value.explicit_unallocated_loss_long),
            explicit_unallocated_loss_short: V16PodU128::new(value.explicit_unallocated_loss_short),
            epoch_long: V16PodU64::new(value.epoch_long),
            epoch_short: V16PodU64::new(value.epoch_short),
            mode_long: encode_side_mode(value.mode_long),
            mode_short: encode_side_mode(value.mode_short),
        }
    }

    pub fn try_to_runtime(&self) -> V16Result<AssetStateV16> {
        let out = AssetStateV16 {
            lifecycle: decode_asset_lifecycle(self.lifecycle)?,
            raw_oracle_target_price: self.raw_oracle_target_price.get(),
            effective_price: self.effective_price.get(),
            fund_px_last: self.fund_px_last.get(),
            slot_last: self.slot_last.get(),
            a_long: self.a_long.get(),
            a_short: self.a_short.get(),
            k_long: self.k_long.get(),
            k_short: self.k_short.get(),
            f_long_num: self.f_long_num.get(),
            f_short_num: self.f_short_num.get(),
            k_epoch_start_long: self.k_epoch_start_long.get(),
            k_epoch_start_short: self.k_epoch_start_short.get(),
            f_epoch_start_long_num: self.f_epoch_start_long_num.get(),
            f_epoch_start_short_num: self.f_epoch_start_short_num.get(),
            b_long_num: self.b_long_num.get(),
            b_short_num: self.b_short_num.get(),
            b_epoch_start_long_num: self.b_epoch_start_long_num.get(),
            b_epoch_start_short_num: self.b_epoch_start_short_num.get(),
            oi_eff_long_q: self.oi_eff_long_q.get(),
            oi_eff_short_q: self.oi_eff_short_q.get(),
            stored_pos_count_long: self.stored_pos_count_long.get(),
            stored_pos_count_short: self.stored_pos_count_short.get(),
            stale_account_count_long: self.stale_account_count_long.get(),
            stale_account_count_short: self.stale_account_count_short.get(),
            pending_obligation_count_long: self.pending_obligation_count_long.get(),
            pending_obligation_count_short: self.pending_obligation_count_short.get(),
            loss_weight_sum_long: self.loss_weight_sum_long.get(),
            loss_weight_sum_short: self.loss_weight_sum_short.get(),
            social_loss_remainder_long_num: self.social_loss_remainder_long_num.get(),
            social_loss_remainder_short_num: self.social_loss_remainder_short_num.get(),
            social_loss_dust_long_num: self.social_loss_dust_long_num.get(),
            social_loss_dust_short_num: self.social_loss_dust_short_num.get(),
            explicit_unallocated_loss_long: self.explicit_unallocated_loss_long.get(),
            explicit_unallocated_loss_short: self.explicit_unallocated_loss_short.get(),
            epoch_long: self.epoch_long.get(),
            epoch_short: self.epoch_short.get(),
            mode_long: decode_side_mode(self.mode_long)?,
            mode_short: decode_side_mode(self.mode_short)?,
        };
        validate_non_min_i128(out.k_long)?;
        validate_non_min_i128(out.k_short)?;
        validate_non_min_i128(out.f_long_num)?;
        validate_non_min_i128(out.f_short_num)?;
        validate_non_min_i128(out.k_epoch_start_long)?;
        validate_non_min_i128(out.k_epoch_start_short)?;
        validate_non_min_i128(out.f_epoch_start_long_num)?;
        validate_non_min_i128(out.f_epoch_start_short_num)?;
        Ok(out)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
pub struct PortfolioLegV16Account {
    pub active: u8,
    pub side: u8,
    pub basis_pos_q: V16PodI128,
    pub a_basis: V16PodU128,
    pub k_snap: V16PodI128,
    pub f_snap: V16PodI128,
    pub epoch_snap: V16PodU64,
    pub loss_weight: V16PodU128,
    pub b_snap: V16PodU128,
    pub b_rem: V16PodU128,
    pub b_epoch_snap: V16PodU64,
    pub b_stale: u8,
    pub stale: u8,
}

impl PortfolioLegV16Account {
    pub fn from_runtime(value: &PortfolioLegV16) -> Self {
        Self {
            active: encode_bool(value.active),
            side: encode_side(value.side),
            basis_pos_q: V16PodI128::new(value.basis_pos_q),
            a_basis: V16PodU128::new(value.a_basis),
            k_snap: V16PodI128::new(value.k_snap),
            f_snap: V16PodI128::new(value.f_snap),
            epoch_snap: V16PodU64::new(value.epoch_snap),
            loss_weight: V16PodU128::new(value.loss_weight),
            b_snap: V16PodU128::new(value.b_snap),
            b_rem: V16PodU128::new(value.b_rem),
            b_epoch_snap: V16PodU64::new(value.b_epoch_snap),
            b_stale: encode_bool(value.b_stale),
            stale: encode_bool(value.stale),
        }
    }

    pub fn try_to_runtime(&self) -> V16Result<PortfolioLegV16> {
        let out = PortfolioLegV16 {
            active: decode_bool(self.active)?,
            side: decode_side(self.side)?,
            basis_pos_q: self.basis_pos_q.get(),
            a_basis: self.a_basis.get(),
            k_snap: self.k_snap.get(),
            f_snap: self.f_snap.get(),
            epoch_snap: self.epoch_snap.get(),
            loss_weight: self.loss_weight.get(),
            b_snap: self.b_snap.get(),
            b_rem: self.b_rem.get(),
            b_epoch_snap: self.b_epoch_snap.get(),
            b_stale: decode_bool(self.b_stale)?,
            stale: decode_bool(self.stale)?,
        };
        if out.active {
            validate_active_leg(out)?;
        } else if out != PortfolioLegV16::EMPTY {
            return Err(V16Error::HiddenLeg);
        }
        Ok(out)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
pub struct HealthCertV16Account {
    pub certified_equity: V16PodI128,
    pub certified_initial_req: V16PodU128,
    pub certified_maintenance_req: V16PodU128,
    pub certified_liq_deficit: V16PodU128,
    pub certified_worst_case_loss: V16PodU128,
    pub cert_oracle_epoch: V16PodU64,
    pub cert_funding_epoch: V16PodU64,
    pub cert_risk_epoch: V16PodU64,
    pub cert_asset_set_epoch: V16PodU64,
    pub active_bitmap_at_cert: V16PodU32,
    pub valid: u8,
}

impl HealthCertV16Account {
    pub fn from_runtime(value: &HealthCertV16) -> Self {
        Self {
            certified_equity: V16PodI128::new(value.certified_equity),
            certified_initial_req: V16PodU128::new(value.certified_initial_req),
            certified_maintenance_req: V16PodU128::new(value.certified_maintenance_req),
            certified_liq_deficit: V16PodU128::new(value.certified_liq_deficit),
            certified_worst_case_loss: V16PodU128::new(value.certified_worst_case_loss),
            cert_oracle_epoch: V16PodU64::new(value.cert_oracle_epoch),
            cert_funding_epoch: V16PodU64::new(value.cert_funding_epoch),
            cert_risk_epoch: V16PodU64::new(value.cert_risk_epoch),
            cert_asset_set_epoch: V16PodU64::new(value.cert_asset_set_epoch),
            active_bitmap_at_cert: V16PodU32::new(value.active_bitmap_at_cert),
            valid: encode_bool(value.valid),
        }
    }

    pub fn try_to_runtime(&self) -> V16Result<HealthCertV16> {
        let out = HealthCertV16 {
            certified_equity: self.certified_equity.get(),
            certified_initial_req: self.certified_initial_req.get(),
            certified_maintenance_req: self.certified_maintenance_req.get(),
            certified_liq_deficit: self.certified_liq_deficit.get(),
            certified_worst_case_loss: self.certified_worst_case_loss.get(),
            cert_oracle_epoch: self.cert_oracle_epoch.get(),
            cert_funding_epoch: self.cert_funding_epoch.get(),
            cert_risk_epoch: self.cert_risk_epoch.get(),
            cert_asset_set_epoch: self.cert_asset_set_epoch.get(),
            active_bitmap_at_cert: self.active_bitmap_at_cert.get(),
            valid: decode_bool(self.valid)?,
        };
        validate_non_min_i128(out.certified_equity)?;
        Ok(out)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
pub struct CloseProgressLedgerV16Account {
    pub active: u8,
    pub finalized: u8,
    pub canceled: u8,
    pub close_id: V16PodU64,
    pub asset_index: u8,
    pub domain_side: u8,
    pub gross_loss_at_close_start: V16PodU128,
    pub drift_reference_slot: V16PodU64,
    pub max_close_slot: V16PodU64,
    pub support_consumed: V16PodU128,
    pub junior_face_burned: V16PodU128,
    pub insurance_spent: V16PodU128,
    pub b_loss_booked: V16PodU128,
    pub explicit_loss_assigned: V16PodU128,
    pub quantity_adl_applied_q: V16PodU128,
    pub drift_consumed: V16PodU128,
    pub residual_remaining: V16PodU128,
}

impl CloseProgressLedgerV16Account {
    pub fn from_runtime(value: &CloseProgressLedgerV16) -> Self {
        Self {
            active: encode_bool(value.active),
            finalized: encode_bool(value.finalized),
            canceled: encode_bool(value.canceled),
            close_id: V16PodU64::new(value.close_id),
            asset_index: value.asset_index,
            domain_side: encode_side(value.domain_side),
            gross_loss_at_close_start: V16PodU128::new(value.gross_loss_at_close_start),
            drift_reference_slot: V16PodU64::new(value.drift_reference_slot),
            max_close_slot: V16PodU64::new(value.max_close_slot),
            support_consumed: V16PodU128::new(value.support_consumed),
            junior_face_burned: V16PodU128::new(value.junior_face_burned),
            insurance_spent: V16PodU128::new(value.insurance_spent),
            b_loss_booked: V16PodU128::new(value.b_loss_booked),
            explicit_loss_assigned: V16PodU128::new(value.explicit_loss_assigned),
            quantity_adl_applied_q: V16PodU128::new(value.quantity_adl_applied_q),
            drift_consumed: V16PodU128::new(value.drift_consumed),
            residual_remaining: V16PodU128::new(value.residual_remaining),
        }
    }

    pub fn try_to_runtime(&self) -> V16Result<CloseProgressLedgerV16> {
        Ok(CloseProgressLedgerV16 {
            active: decode_bool(self.active)?,
            finalized: decode_bool(self.finalized)?,
            canceled: decode_bool(self.canceled)?,
            close_id: self.close_id.get(),
            asset_index: self.asset_index,
            domain_side: decode_side(self.domain_side)?,
            gross_loss_at_close_start: self.gross_loss_at_close_start.get(),
            drift_reference_slot: self.drift_reference_slot.get(),
            max_close_slot: self.max_close_slot.get(),
            support_consumed: self.support_consumed.get(),
            junior_face_burned: self.junior_face_burned.get(),
            insurance_spent: self.insurance_spent.get(),
            b_loss_booked: self.b_loss_booked.get(),
            explicit_loss_assigned: self.explicit_loss_assigned.get(),
            quantity_adl_applied_q: self.quantity_adl_applied_q.get(),
            drift_consumed: self.drift_consumed.get(),
            residual_remaining: self.residual_remaining.get(),
        })
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
pub struct ResolvedPayoutLedgerV16Account {
    pub snapshot_residual: V16PodU128,
    pub terminal_claim_exact_receipts_num: V16PodU128,
    pub terminal_claim_bound_unreceipted_num: V16PodU128,
    pub current_payout_rate_num: V16PodU128,
    pub current_payout_rate_den: V16PodU128,
    pub snapshot_slot: V16PodU64,
    pub payout_halted: u8,
    pub finalized: u8,
}

impl ResolvedPayoutLedgerV16Account {
    pub fn from_runtime(value: &ResolvedPayoutLedgerV16) -> Self {
        Self {
            snapshot_residual: V16PodU128::new(value.snapshot_residual),
            terminal_claim_exact_receipts_num: V16PodU128::new(
                value.terminal_claim_exact_receipts_num,
            ),
            terminal_claim_bound_unreceipted_num: V16PodU128::new(
                value.terminal_claim_bound_unreceipted_num,
            ),
            current_payout_rate_num: V16PodU128::new(value.current_payout_rate_num),
            current_payout_rate_den: V16PodU128::new(value.current_payout_rate_den),
            snapshot_slot: V16PodU64::new(value.snapshot_slot),
            payout_halted: encode_bool(value.payout_halted),
            finalized: encode_bool(value.finalized),
        }
    }

    pub fn try_to_runtime(&self) -> V16Result<ResolvedPayoutLedgerV16> {
        Ok(ResolvedPayoutLedgerV16 {
            snapshot_residual: self.snapshot_residual.get(),
            terminal_claim_exact_receipts_num: self.terminal_claim_exact_receipts_num.get(),
            terminal_claim_bound_unreceipted_num: self.terminal_claim_bound_unreceipted_num.get(),
            current_payout_rate_num: self.current_payout_rate_num.get(),
            current_payout_rate_den: self.current_payout_rate_den.get(),
            snapshot_slot: self.snapshot_slot.get(),
            payout_halted: decode_bool(self.payout_halted)?,
            finalized: decode_bool(self.finalized)?,
        })
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
pub struct ResolvedPayoutReceiptV16Account {
    pub prior_bound_contribution_num: V16PodU128,
    pub live_released_face_at_receipt: V16PodU128,
    pub terminal_positive_claim_face: V16PodU128,
    pub paid_effective: V16PodU128,
    pub present: u8,
    pub finalized: u8,
}

impl ResolvedPayoutReceiptV16Account {
    pub fn from_runtime(value: &ResolvedPayoutReceiptV16) -> Self {
        Self {
            prior_bound_contribution_num: V16PodU128::new(value.prior_bound_contribution_num),
            live_released_face_at_receipt: V16PodU128::new(value.live_released_face_at_receipt),
            terminal_positive_claim_face: V16PodU128::new(value.terminal_positive_claim_face),
            paid_effective: V16PodU128::new(value.paid_effective),
            present: encode_bool(value.present),
            finalized: encode_bool(value.finalized),
        }
    }

    pub fn try_to_runtime(&self) -> V16Result<ResolvedPayoutReceiptV16> {
        Ok(ResolvedPayoutReceiptV16 {
            present: decode_bool(self.present)?,
            prior_bound_contribution_num: self.prior_bound_contribution_num.get(),
            live_released_face_at_receipt: self.live_released_face_at_receipt.get(),
            terminal_positive_claim_face: self.terminal_positive_claim_face.get(),
            paid_effective: self.paid_effective.get(),
            finalized: decode_bool(self.finalized)?,
        })
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
pub struct PortfolioAccountV16Account {
    pub provenance_header: ProvenanceHeaderV16Account,
    pub owner: [u8; 32],
    pub capital: V16PodU128,
    pub pnl: V16PodI128,
    pub reserved_pnl: V16PodU128,
    pub source_claim_bound_num: [V16PodU128; V16_DOMAIN_COUNT],
    pub source_claim_liened_num: [V16PodU128; V16_DOMAIN_COUNT],
    pub source_claim_counterparty_liened_num: [V16PodU128; V16_DOMAIN_COUNT],
    pub source_claim_insurance_liened_num: [V16PodU128; V16_DOMAIN_COUNT],
    pub source_lien_effective_reserved: [V16PodU128; V16_DOMAIN_COUNT],
    pub source_lien_counterparty_backing_num: [V16PodU128; V16_DOMAIN_COUNT],
    pub source_lien_insurance_backing_num: [V16PodU128; V16_DOMAIN_COUNT],
    pub source_claim_impaired_num: [V16PodU128; V16_DOMAIN_COUNT],
    pub source_lien_impaired_effective_reserved: [V16PodU128; V16_DOMAIN_COUNT],
    pub fee_credits: V16PodI128,
    pub cancel_deposit_escrow: V16PodU128,
    pub last_fee_slot: V16PodU64,
    pub active_bitmap: V16PodU32,
    pub legs: [PortfolioLegV16Account; V16_MAX_PORTFOLIO_ASSETS_N],
    pub health_cert: HealthCertV16Account,
    pub stale_state: u8,
    pub b_stale_state: u8,
    pub rebalance_lock: u8,
    pub liquidation_lock: u8,
    pub close_progress: CloseProgressLedgerV16Account,
    pub resolved_payout_receipt: ResolvedPayoutReceiptV16Account,
}

impl PortfolioAccountV16Account {
    #[cfg(not(target_os = "solana"))]
    pub fn try_empty(header: ProvenanceHeaderV16Account) -> V16Result<Self> {
        Ok(Self::from_runtime(&PortfolioAccountV16::empty(
            header.try_to_runtime()?,
        )))
    }

    #[cfg(not(target_os = "solana"))]
    pub fn from_runtime(value: &PortfolioAccountV16) -> Self {
        let mut legs = [PortfolioLegV16Account::default(); V16_MAX_PORTFOLIO_ASSETS_N];
        let mut i = 0;
        while i < V16_MAX_PORTFOLIO_ASSETS_N {
            legs[i] = PortfolioLegV16Account::from_runtime(&value.legs[i]);
            i += 1;
        }
        Self {
            provenance_header: ProvenanceHeaderV16Account::from_runtime(&value.provenance_header),
            owner: value.owner,
            capital: V16PodU128::new(value.capital),
            pnl: V16PodI128::new(value.pnl),
            reserved_pnl: V16PodU128::new(value.reserved_pnl),
            source_claim_bound_num: value.source_claim_bound_num.map(V16PodU128::new),
            source_claim_liened_num: value.source_claim_liened_num.map(V16PodU128::new),
            source_claim_counterparty_liened_num: value
                .source_claim_counterparty_liened_num
                .map(V16PodU128::new),
            source_claim_insurance_liened_num: value
                .source_claim_insurance_liened_num
                .map(V16PodU128::new),
            source_lien_effective_reserved: value
                .source_lien_effective_reserved
                .map(V16PodU128::new),
            source_lien_counterparty_backing_num: value
                .source_lien_counterparty_backing_num
                .map(V16PodU128::new),
            source_lien_insurance_backing_num: value
                .source_lien_insurance_backing_num
                .map(V16PodU128::new),
            source_claim_impaired_num: value.source_claim_impaired_num.map(V16PodU128::new),
            source_lien_impaired_effective_reserved: value
                .source_lien_impaired_effective_reserved
                .map(V16PodU128::new),
            fee_credits: V16PodI128::new(value.fee_credits),
            cancel_deposit_escrow: V16PodU128::new(value.cancel_deposit_escrow),
            last_fee_slot: V16PodU64::new(value.last_fee_slot),
            active_bitmap: V16PodU32::new(value.active_bitmap),
            legs,
            health_cert: HealthCertV16Account::from_runtime(&value.health_cert),
            stale_state: encode_bool(value.stale_state),
            b_stale_state: encode_bool(value.b_stale_state),
            rebalance_lock: encode_bool(value.rebalance_lock),
            liquidation_lock: encode_bool(value.liquidation_lock),
            close_progress: CloseProgressLedgerV16Account::from_runtime(&value.close_progress),
            resolved_payout_receipt: ResolvedPayoutReceiptV16Account::from_runtime(
                &value.resolved_payout_receipt,
            ),
        }
    }

    #[cfg(not(target_os = "solana"))]
    pub fn try_to_runtime(&self) -> V16Result<PortfolioAccountV16> {
        let mut legs = [PortfolioLegV16::EMPTY; V16_MAX_PORTFOLIO_ASSETS_N];
        let mut i = 0;
        while i < V16_MAX_PORTFOLIO_ASSETS_N {
            legs[i] = self.legs[i].try_to_runtime()?;
            i += 1;
        }
        let out = PortfolioAccountV16 {
            provenance_header: self.provenance_header.try_to_runtime()?,
            owner: self.owner,
            capital: self.capital.get(),
            pnl: self.pnl.get(),
            reserved_pnl: self.reserved_pnl.get(),
            source_claim_bound_num: self.source_claim_bound_num.map(|v| v.get()),
            source_claim_liened_num: self.source_claim_liened_num.map(|v| v.get()),
            source_claim_counterparty_liened_num: self
                .source_claim_counterparty_liened_num
                .map(|v| v.get()),
            source_claim_insurance_liened_num: self
                .source_claim_insurance_liened_num
                .map(|v| v.get()),
            source_lien_effective_reserved: self.source_lien_effective_reserved.map(|v| v.get()),
            source_lien_counterparty_backing_num: self
                .source_lien_counterparty_backing_num
                .map(|v| v.get()),
            source_lien_insurance_backing_num: self
                .source_lien_insurance_backing_num
                .map(|v| v.get()),
            source_claim_impaired_num: self.source_claim_impaired_num.map(|v| v.get()),
            source_lien_impaired_effective_reserved: self
                .source_lien_impaired_effective_reserved
                .map(|v| v.get()),
            fee_credits: self.fee_credits.get(),
            cancel_deposit_escrow: self.cancel_deposit_escrow.get(),
            last_fee_slot: self.last_fee_slot.get(),
            active_bitmap: self.active_bitmap.get(),
            legs,
            health_cert: self.health_cert.try_to_runtime()?,
            stale_state: decode_bool(self.stale_state)?,
            b_stale_state: decode_bool(self.b_stale_state)?,
            rebalance_lock: decode_bool(self.rebalance_lock)?,
            liquidation_lock: decode_bool(self.liquidation_lock)?,
            close_progress: self.close_progress.try_to_runtime()?,
            resolved_payout_receipt: self.resolved_payout_receipt.try_to_runtime()?,
        };
        if out.provenance_header.owner != out.owner {
            return Err(V16Error::ProvenanceMismatch);
        }
        validate_non_min_i128(out.pnl)?;
        validate_fee_credits(out.fee_credits)?;
        if out.reserved_pnl > out.pnl.max(0) as u128 {
            return Err(V16Error::InvalidLeg);
        }
        let mut d = 0;
        while d < V16_DOMAIN_COUNT {
            let locked = out.source_claim_liened_num[d]
                .checked_add(out.source_claim_impaired_num[d])
                .ok_or(V16Error::ArithmeticOverflow)?;
            if locked > out.source_claim_bound_num[d] {
                return Err(V16Error::InvalidLeg);
            }
            let backing_source_claim = out.source_claim_counterparty_liened_num[d]
                .checked_add(out.source_claim_insurance_liened_num[d])
                .ok_or(V16Error::ArithmeticOverflow)?;
            if backing_source_claim != out.source_claim_liened_num[d] {
                return Err(V16Error::InvalidLeg);
            }
            if out.source_lien_effective_reserved[d]
                > MarketGroupV16::amount_from_bound_num(out.source_claim_liened_num[d])?
            {
                return Err(V16Error::InvalidLeg);
            }
            let lien_backing_num = out.source_lien_counterparty_backing_num[d]
                .checked_add(out.source_lien_insurance_backing_num[d])
                .ok_or(V16Error::ArithmeticOverflow)?;
            if out.source_lien_counterparty_backing_num[d] % BOUND_SCALE != 0
                || out.source_lien_insurance_backing_num[d] % BOUND_SCALE != 0
            {
                return Err(V16Error::InvalidLeg);
            }
            let expected_backing_num = out.source_lien_effective_reserved[d]
                .checked_mul(BOUND_SCALE)
                .ok_or(V16Error::ArithmeticOverflow)?;
            if lien_backing_num != expected_backing_num {
                return Err(V16Error::InvalidLeg);
            }
            d += 1;
        }
        let source_claim_sum_num = MarketGroupV16::account_source_claim_bound_sum_num_static(&out)?;
        if source_claim_sum_num != 0 {
            let required = MarketGroupV16::bound_num_from_amount(out.pnl.max(0) as u128)?;
            if source_claim_sum_num < required {
                return Err(V16Error::InvalidLeg);
            }
        }
        Ok(out)
    }

    #[cfg(not(target_os = "solana"))]
    pub fn validate_with_market(&self, market: &MarketGroupV16) -> V16Result<PortfolioAccountV16> {
        let out = self.try_to_runtime()?;
        market.validate_account_shape(&out)?;
        Ok(out)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
pub struct MarketGroupV16Account {
    pub market_group_id: [u8; 32],
    pub config: V16ConfigAccount,
    pub vault: V16PodU128,
    pub insurance: V16PodU128,
    pub c_tot: V16PodU128,
    pub pnl_pos_tot: V16PodU128,
    pub pnl_pos_bound_tot_num: V16PodU128,
    pub pnl_pos_bound_tot: V16PodU128,
    pub pnl_matured_pos_tot: V16PodU128,
    pub insurance_domain_budget: [V16PodU128; V16_DOMAIN_COUNT],
    pub insurance_domain_spent: [V16PodU128; V16_DOMAIN_COUNT],
    pub pending_domain_loss_barriers: [V16PodU64; V16_DOMAIN_COUNT],
    pub source_credit: [SourceCreditStateV16Account; V16_DOMAIN_COUNT],
    pub source_backing_buckets: [BackingBucketV16Account; V16_DOMAIN_COUNT],
    pub insurance_credit_reservations: [InsuranceCreditReservationV16Account; V16_DOMAIN_COUNT],
    pub materialized_portfolio_count: V16PodU64,
    pub stale_certificate_count: V16PodU64,
    pub b_stale_account_count: V16PodU64,
    pub negative_pnl_account_count: V16PodU64,
    pub risk_epoch: V16PodU64,
    pub asset_set_epoch: V16PodU64,
    pub asset_activation_count: V16PodU64,
    pub last_asset_activation_slot: V16PodU64,
    pub oracle_epoch: V16PodU64,
    pub funding_epoch: V16PodU64,
    pub slot_last: V16PodU64,
    pub current_slot: V16PodU64,
    pub assets: [AssetStateV16Account; V16_MAX_PORTFOLIO_ASSETS_N],
    pub bankruptcy_hlock_active: u8,
    pub threshold_stress_active: u8,
    pub loss_stale_active: u8,
    pub recovery_reason: V16OptionalRecoveryReasonAccount,
    pub mode: u8,
    pub resolved_slot: V16PodU64,
    pub payout_snapshot: V16PodU128,
    pub payout_snapshot_pnl_pos_tot: V16PodU128,
    pub payout_snapshot_captured: u8,
    pub resolved_payout_ledger: ResolvedPayoutLedgerV16Account,
}

impl MarketGroupV16Account {
    #[cfg(not(target_os = "solana"))]
    pub fn from_runtime(value: &MarketGroupV16) -> Self {
        let mut assets = [AssetStateV16Account::default(); V16_MAX_PORTFOLIO_ASSETS_N];
        let mut i = 0;
        while i < V16_MAX_PORTFOLIO_ASSETS_N {
            assets[i] = AssetStateV16Account::from_runtime(&value.assets[i]);
            i += 1;
        }
        let mut insurance_domain_budget = [V16PodU128::default(); V16_DOMAIN_COUNT];
        let mut insurance_domain_spent = [V16PodU128::default(); V16_DOMAIN_COUNT];
        let mut pending_domain_loss_barriers = [V16PodU64::default(); V16_DOMAIN_COUNT];
        let mut source_credit = [SourceCreditStateV16Account::default(); V16_DOMAIN_COUNT];
        let mut source_backing_buckets = [BackingBucketV16Account::default(); V16_DOMAIN_COUNT];
        let mut insurance_credit_reservations =
            [InsuranceCreditReservationV16Account::default(); V16_DOMAIN_COUNT];
        let mut d = 0;
        while d < V16_DOMAIN_COUNT {
            insurance_domain_budget[d] = V16PodU128::new(value.insurance_domain_budget[d]);
            insurance_domain_spent[d] = V16PodU128::new(value.insurance_domain_spent[d]);
            pending_domain_loss_barriers[d] = V16PodU64::new(value.pending_domain_loss_barriers[d]);
            source_credit[d] = SourceCreditStateV16Account::from_runtime(&value.source_credit[d]);
            source_backing_buckets[d] =
                BackingBucketV16Account::from_runtime(&value.source_backing_buckets[d]);
            insurance_credit_reservations[d] = InsuranceCreditReservationV16Account::from_runtime(
                &value.insurance_credit_reservations[d],
            );
            d += 1;
        }
        Self {
            market_group_id: value.market_group_id,
            config: V16ConfigAccount::from_runtime(&value.config),
            vault: V16PodU128::new(value.vault),
            insurance: V16PodU128::new(value.insurance),
            c_tot: V16PodU128::new(value.c_tot),
            pnl_pos_tot: V16PodU128::new(value.pnl_pos_tot),
            pnl_pos_bound_tot_num: V16PodU128::new(value.pnl_pos_bound_tot_num),
            pnl_pos_bound_tot: V16PodU128::new(value.pnl_pos_bound_tot),
            pnl_matured_pos_tot: V16PodU128::new(value.pnl_matured_pos_tot),
            insurance_domain_budget,
            insurance_domain_spent,
            pending_domain_loss_barriers,
            source_credit,
            source_backing_buckets,
            insurance_credit_reservations,
            materialized_portfolio_count: V16PodU64::new(value.materialized_portfolio_count),
            stale_certificate_count: V16PodU64::new(value.stale_certificate_count),
            b_stale_account_count: V16PodU64::new(value.b_stale_account_count),
            negative_pnl_account_count: V16PodU64::new(value.negative_pnl_account_count),
            risk_epoch: V16PodU64::new(value.risk_epoch),
            asset_set_epoch: V16PodU64::new(value.asset_set_epoch),
            asset_activation_count: V16PodU64::new(value.asset_activation_count),
            last_asset_activation_slot: V16PodU64::new(value.last_asset_activation_slot),
            oracle_epoch: V16PodU64::new(value.oracle_epoch),
            funding_epoch: V16PodU64::new(value.funding_epoch),
            slot_last: V16PodU64::new(value.slot_last),
            current_slot: V16PodU64::new(value.current_slot),
            assets,
            bankruptcy_hlock_active: encode_bool(value.bankruptcy_hlock_active),
            threshold_stress_active: encode_bool(value.threshold_stress_active),
            loss_stale_active: encode_bool(value.loss_stale_active),
            recovery_reason: V16OptionalRecoveryReasonAccount::from_runtime(value.recovery_reason),
            mode: encode_market_mode(value.mode),
            resolved_slot: V16PodU64::new(value.resolved_slot),
            payout_snapshot: V16PodU128::new(value.payout_snapshot),
            payout_snapshot_pnl_pos_tot: V16PodU128::new(value.payout_snapshot_pnl_pos_tot),
            payout_snapshot_captured: encode_bool(value.payout_snapshot_captured),
            resolved_payout_ledger: ResolvedPayoutLedgerV16Account::from_runtime(
                &value.resolved_payout_ledger,
            ),
        }
    }

    #[cfg(not(target_os = "solana"))]
    pub fn try_to_runtime(&self) -> V16Result<MarketGroupV16> {
        let mut assets = [AssetStateV16::default(); V16_MAX_PORTFOLIO_ASSETS_N];
        let mut i = 0;
        while i < V16_MAX_PORTFOLIO_ASSETS_N {
            assets[i] = self.assets[i].try_to_runtime()?;
            i += 1;
        }
        let mut insurance_domain_budget = [0u128; V16_DOMAIN_COUNT];
        let mut insurance_domain_spent = [0u128; V16_DOMAIN_COUNT];
        let mut pending_domain_loss_barriers = [0u64; V16_DOMAIN_COUNT];
        let mut source_credit = [SourceCreditStateV16::EMPTY; V16_DOMAIN_COUNT];
        let mut source_backing_buckets = [BackingBucketV16::EMPTY; V16_DOMAIN_COUNT];
        let mut insurance_credit_reservations =
            [InsuranceCreditReservationV16::EMPTY; V16_DOMAIN_COUNT];
        let mut d = 0;
        while d < V16_DOMAIN_COUNT {
            insurance_domain_budget[d] = self.insurance_domain_budget[d].get();
            insurance_domain_spent[d] = self.insurance_domain_spent[d].get();
            pending_domain_loss_barriers[d] = self.pending_domain_loss_barriers[d].get();
            source_credit[d] = self.source_credit[d].try_to_runtime()?;
            source_backing_buckets[d] = self.source_backing_buckets[d].try_to_runtime()?;
            insurance_credit_reservations[d] =
                self.insurance_credit_reservations[d].try_to_runtime()?;
            d += 1;
        }
        let out = MarketGroupV16 {
            market_group_id: self.market_group_id,
            config: self.config.try_to_runtime()?,
            vault: self.vault.get(),
            insurance: self.insurance.get(),
            c_tot: self.c_tot.get(),
            pnl_pos_tot: self.pnl_pos_tot.get(),
            pnl_pos_bound_tot_num: self.pnl_pos_bound_tot_num.get(),
            pnl_pos_bound_tot: self.pnl_pos_bound_tot.get(),
            pnl_matured_pos_tot: self.pnl_matured_pos_tot.get(),
            insurance_domain_budget,
            insurance_domain_spent,
            pending_domain_loss_barriers,
            source_credit,
            source_backing_buckets,
            insurance_credit_reservations,
            materialized_portfolio_count: self.materialized_portfolio_count.get(),
            stale_certificate_count: self.stale_certificate_count.get(),
            b_stale_account_count: self.b_stale_account_count.get(),
            negative_pnl_account_count: self.negative_pnl_account_count.get(),
            risk_epoch: self.risk_epoch.get(),
            asset_set_epoch: self.asset_set_epoch.get(),
            asset_activation_count: self.asset_activation_count.get(),
            last_asset_activation_slot: self.last_asset_activation_slot.get(),
            oracle_epoch: self.oracle_epoch.get(),
            funding_epoch: self.funding_epoch.get(),
            slot_last: self.slot_last.get(),
            current_slot: self.current_slot.get(),
            assets,
            bankruptcy_hlock_active: decode_bool(self.bankruptcy_hlock_active)?,
            threshold_stress_active: decode_bool(self.threshold_stress_active)?,
            loss_stale_active: decode_bool(self.loss_stale_active)?,
            recovery_reason: self.recovery_reason.try_to_runtime()?,
            mode: decode_market_mode(self.mode)?,
            resolved_slot: self.resolved_slot.get(),
            payout_snapshot: self.payout_snapshot.get(),
            payout_snapshot_pnl_pos_tot: self.payout_snapshot_pnl_pos_tot.get(),
            payout_snapshot_captured: decode_bool(self.payout_snapshot_captured)?,
            resolved_payout_ledger: self.resolved_payout_ledger.try_to_runtime()?,
        };
        out.assert_public_invariants()?;
        Ok(out)
    }

    #[cfg(not(target_os = "solana"))]
    pub fn validate(&self) -> V16Result<MarketGroupV16> {
        self.try_to_runtime()
    }
}

impl MarketGroupV16 {
    #[cfg(not(target_os = "solana"))]
    pub fn new(market_group_id: [u8; 32], config: V16Config) -> V16Result<Self> {
        config.validate_public_user_fund()?;
        Ok(Self {
            market_group_id,
            config,
            vault: 0,
            insurance: 0,
            c_tot: 0,
            pnl_pos_tot: 0,
            pnl_pos_bound_tot_num: 0,
            pnl_pos_bound_tot: 0,
            pnl_matured_pos_tot: 0,
            insurance_domain_budget: [MAX_VAULT_TVL; V16_DOMAIN_COUNT],
            insurance_domain_spent: [0; V16_DOMAIN_COUNT],
            pending_domain_loss_barriers: [0; V16_DOMAIN_COUNT],
            source_credit: [SourceCreditStateV16::EMPTY; V16_DOMAIN_COUNT],
            source_backing_buckets: [BackingBucketV16::EMPTY; V16_DOMAIN_COUNT],
            insurance_credit_reservations: [InsuranceCreditReservationV16::EMPTY; V16_DOMAIN_COUNT],
            materialized_portfolio_count: 0,
            stale_certificate_count: 0,
            b_stale_account_count: 0,
            negative_pnl_account_count: 0,
            risk_epoch: 0,
            asset_set_epoch: 0,
            asset_activation_count: 0,
            last_asset_activation_slot: 0,
            oracle_epoch: 0,
            funding_epoch: 0,
            slot_last: 0,
            current_slot: 0,
            assets: [AssetStateV16::default(); V16_MAX_PORTFOLIO_ASSETS_N],
            bankruptcy_hlock_active: false,
            threshold_stress_active: false,
            loss_stale_active: false,
            recovery_reason: None,
            mode: MarketModeV16::Live,
            resolved_slot: 0,
            payout_snapshot: 0,
            payout_snapshot_pnl_pos_tot: 0,
            payout_snapshot_captured: false,
            resolved_payout_ledger: ResolvedPayoutLedgerV16::EMPTY,
        })
    }

    pub fn validate_portfolio_account_provenance(
        &self,
        account: &PortfolioAccountV16,
    ) -> V16Result<()> {
        let h = account.provenance_header;
        if h.market_group_id != self.market_group_id
            || h.owner != account.owner
            || h.version != V16_ACCOUNT_VERSION
            || h.layout_discriminator != V16_LAYOUT_DISCRIMINATOR
        {
            return Err(V16Error::ProvenanceMismatch);
        }
        Ok(())
    }

    fn validate_close_progress_ledger(&self, ledger: CloseProgressLedgerV16) -> V16Result<()> {
        if ledger.canceled {
            if ledger.active
                || ledger.finalized
                || ledger.close_id == 0
                || ledger.asset_index as usize >= self.config.max_portfolio_assets as usize
                || ledger.drift_reference_slot > ledger.max_close_slot
                || ledger.has_irreversible_progress()
                || ledger.residual_remaining != ledger.gross_loss_at_close_start
            {
                return Err(V16Error::InvalidLeg);
            }
            return Ok(());
        }
        if !ledger.active {
            if ledger != CloseProgressLedgerV16::EMPTY {
                return Err(V16Error::InvalidLeg);
            }
            return Ok(());
        }
        if ledger.close_id == 0
            || ledger.asset_index as usize >= self.config.max_portfolio_assets as usize
            || ledger.drift_reference_slot > ledger.max_close_slot
            || ledger.max_close_slot < ledger.drift_reference_slot
            || ledger.support_consumed > ledger.junior_face_burned
            || ledger.canceled
        {
            return Err(V16Error::InvalidLeg);
        }
        let progress = ledger
            .support_consumed
            .checked_add(ledger.insurance_spent)
            .and_then(|v| v.checked_add(ledger.b_loss_booked))
            .and_then(|v| v.checked_add(ledger.explicit_loss_assigned))
            .ok_or(V16Error::ArithmeticOverflow)?;
        let total_loss = ledger
            .gross_loss_at_close_start
            .checked_add(ledger.drift_consumed)
            .ok_or(V16Error::ArithmeticOverflow)?;
        if progress > total_loss || ledger.residual_remaining != total_loss - progress {
            return Err(V16Error::InvalidLeg);
        }
        if ledger.finalized && ledger.residual_remaining != 0 {
            return Err(V16Error::InvalidLeg);
        }
        if ledger.quantity_adl_applied_q != 0
            && (!ledger.finalized || ledger.residual_remaining != 0)
        {
            return Err(V16Error::InvalidLeg);
        }
        Ok(())
    }

    pub fn validate_account_shape(&self, account: &PortfolioAccountV16) -> V16Result<()> {
        self.validate_portfolio_account_provenance(account)?;
        validate_non_min_i128(account.pnl)?;
        validate_fee_credits(account.fee_credits)?;
        if account.reserved_pnl > account.pnl.max(0) as u128 {
            return Err(V16Error::InvalidLeg);
        }
        self.validate_account_source_credit_shape(account)?;
        let source_claim_sum_num = Self::account_source_claim_bound_sum_num_static(account)?;
        if source_claim_sum_num != 0 {
            let required = Self::bound_num_from_amount(account.pnl.max(0) as u128)?;
            if source_claim_sum_num < required {
                return Err(V16Error::InvalidLeg);
            }
        }
        self.validate_close_progress_ledger(account.close_progress)?;
        self.validate_resolved_payout_receipt(account.resolved_payout_receipt)?;

        let n = self.config.max_portfolio_assets as usize;
        for i in 0..V16_MAX_PORTFOLIO_ASSETS_N {
            let bit = ((account.active_bitmap >> i) & 1) != 0;
            let leg = account.legs[i];
            if i >= n {
                if bit || leg != PortfolioLegV16::default() {
                    return Err(V16Error::HiddenLeg);
                }
                continue;
            }

            if bit != leg.active {
                return Err(V16Error::HiddenLeg);
            }
            if !leg.active {
                if leg != PortfolioLegV16::EMPTY {
                    return Err(V16Error::HiddenLeg);
                }
            } else {
                validate_active_leg(leg)?;
            }
        }
        if account.close_progress.active {
            let i = account.close_progress.asset_index as usize;
            if i < n {
                let leg = account.legs[i];
                if leg.active && account.close_progress.domain_side != opposite_side(leg.side) {
                    return Err(V16Error::InvalidLeg);
                }
            }
        }
        if account.close_progress.quantity_adl_applied_q != 0 {
            let i = account.close_progress.asset_index as usize;
            if i >= n || account.legs[i].active {
                return Err(V16Error::InvalidLeg);
            }
        }
        Ok(())
    }

    fn validate_resolved_payout_receipt(&self, receipt: ResolvedPayoutReceiptV16) -> V16Result<()> {
        if !receipt.present {
            if receipt != ResolvedPayoutReceiptV16::EMPTY {
                return Err(V16Error::InvalidLeg);
            }
            return Ok(());
        }
        let exact_num = Self::bound_num_from_amount(receipt.terminal_positive_claim_face)?;
        if exact_num > receipt.prior_bound_contribution_num {
            return Err(V16Error::InvalidLeg);
        }
        if receipt.paid_effective > receipt.terminal_positive_claim_face {
            return Err(V16Error::InvalidLeg);
        }
        if receipt.finalized != (receipt.paid_effective == receipt.terminal_positive_claim_face) {
            return Err(V16Error::InvalidLeg);
        }
        Ok(())
    }

    pub fn create_portfolio_account(&mut self, account: &PortfolioAccountV16) -> V16Result<()> {
        self.validate_account_shape(account)?;
        self.materialized_portfolio_count = self
            .materialized_portfolio_count
            .checked_add(1)
            .ok_or(V16Error::CounterOverflow)?;
        Ok(())
    }

    fn validate_account_source_credit_shape(&self, account: &PortfolioAccountV16) -> V16Result<()> {
        let configured_domains = self.config.max_portfolio_assets as usize * 2;
        let mut d = 0;
        while d < V16_DOMAIN_COUNT {
            let zero_source_domain = account.source_claim_bound_num[d] == 0
                && account.source_claim_liened_num[d] == 0
                && account.source_claim_counterparty_liened_num[d] == 0
                && account.source_claim_insurance_liened_num[d] == 0
                && account.source_lien_effective_reserved[d] == 0
                && account.source_lien_counterparty_backing_num[d] == 0
                && account.source_lien_insurance_backing_num[d] == 0
                && account.source_claim_impaired_num[d] == 0
                && account.source_lien_impaired_effective_reserved[d] == 0;
            if zero_source_domain {
                d += 1;
                continue;
            }
            if d >= configured_domains {
                return Err(V16Error::HiddenLeg);
            }
            self.source_credit_lien_proof_for_account_domain(account, d)?
                .validate()?;
            let locked = account.source_claim_liened_num[d]
                .checked_add(account.source_claim_impaired_num[d])
                .ok_or(V16Error::ArithmeticOverflow)?;
            if locked > account.source_claim_bound_num[d] {
                return Err(V16Error::InvalidLeg);
            }
            let backing_source_claim = account.source_claim_counterparty_liened_num[d]
                .checked_add(account.source_claim_insurance_liened_num[d])
                .ok_or(V16Error::ArithmeticOverflow)?;
            if backing_source_claim != account.source_claim_liened_num[d] {
                return Err(V16Error::InvalidLeg);
            }
            if account.source_lien_effective_reserved[d]
                > Self::amount_from_bound_num(account.source_claim_liened_num[d])?
            {
                return Err(V16Error::InvalidLeg);
            }
            let lien_backing_num = account.source_lien_counterparty_backing_num[d]
                .checked_add(account.source_lien_insurance_backing_num[d])
                .ok_or(V16Error::ArithmeticOverflow)?;
            if account.source_lien_counterparty_backing_num[d] % BOUND_SCALE != 0
                || account.source_lien_insurance_backing_num[d] % BOUND_SCALE != 0
            {
                return Err(V16Error::InvalidLeg);
            }
            let expected_backing_num = account.source_lien_effective_reserved[d]
                .checked_mul(BOUND_SCALE)
                .ok_or(V16Error::ArithmeticOverflow)?;
            if lien_backing_num != expected_backing_num {
                return Err(V16Error::InvalidLeg);
            }
            if account.source_lien_impaired_effective_reserved[d] != 0
                && account.source_claim_impaired_num[d] == 0
            {
                return Err(V16Error::InvalidLeg);
            }
            d += 1;
        }
        Ok(())
    }

    fn account_has_active_source_claim_exposure(
        &self,
        account: &PortfolioAccountV16,
    ) -> V16Result<bool> {
        let mut d = 0;
        while d < V16_DOMAIN_COUNT {
            if account.source_claim_bound_num[d] != 0
                && self.account_has_active_exposure_for_source_domain(account, d)?
            {
                return Ok(true);
            }
            d += 1;
        }
        Ok(false)
    }

    pub fn source_credit_lien_proof_for_account_domain(
        &self,
        account: &PortfolioAccountV16,
        domain: usize,
    ) -> V16Result<SourceCreditLienAggregateProofV16> {
        if domain >= V16_DOMAIN_COUNT {
            return Err(V16Error::InvalidLeg);
        }
        Ok(SourceCreditLienAggregateProofV16 {
            domain: u16::try_from(domain).map_err(|_| V16Error::ArithmeticOverflow)?,
            source_claim_bound_num: account.source_claim_bound_num[domain],
            face_claim_locked_num: account.source_claim_liened_num[domain],
            counterparty_face_claim_locked_num: account.source_claim_counterparty_liened_num
                [domain],
            insurance_face_claim_locked_num: account.source_claim_insurance_liened_num[domain],
            effective_credit_reserved: account.source_lien_effective_reserved[domain],
            counterparty_backing_reserved_num: account.source_lien_counterparty_backing_num[domain],
            insurance_backing_reserved_num: account.source_lien_insurance_backing_num[domain],
            impaired_face_claim_num: account.source_claim_impaired_num[domain],
            impaired_effective_credit_reserved: account.source_lien_impaired_effective_reserved
                [domain],
        })
    }

    pub fn close_portfolio_account(&mut self, account: &PortfolioAccountV16) -> V16Result<()> {
        self.validate_account_shape(account)?;
        if account.active_bitmap != 0
            || account.capital != 0
            || account.pnl != 0
            || account.reserved_pnl != 0
            || account.fee_credits != 0
            || account.cancel_deposit_escrow != 0
            || account.stale_state
            || account.b_stale_state
            || account.close_progress.active
            || Self::account_source_claim_bound_sum_num_static(account)? != 0
            || (account.resolved_payout_receipt.present
                && !account.resolved_payout_receipt.finalized)
        {
            return Err(V16Error::LockActive);
        }
        self.materialized_portfolio_count = self
            .materialized_portfolio_count
            .checked_sub(1)
            .ok_or(V16Error::CounterUnderflow)?;
        Ok(())
    }

    pub fn deposit_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV16,
        amount: u128,
    ) -> V16Result<()> {
        self.validate_account_shape(account)?;
        if amount == 0 {
            return Ok(());
        }
        let vault_before = self.vault;
        account.capital = account
            .capital
            .checked_add(amount)
            .ok_or(V16Error::ArithmeticOverflow)?;
        self.c_tot = self
            .c_tot
            .checked_add(amount)
            .ok_or(V16Error::ArithmeticOverflow)?;
        self.vault = self
            .vault
            .checked_add(amount)
            .ok_or(V16Error::ArithmeticOverflow)?;
        TokenValueFlowProofV16::external_in_to_account_capital(amount, vault_before, self.vault)?
            .validate()?;
        account.health_cert.valid = false;
        self.assert_public_invariants()
    }

    pub fn cure_and_cancel_close_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV16,
        optional_deposit: u128,
        effective_prices: &[u64; V16_MAX_PORTFOLIO_ASSETS_N],
    ) -> V16Result<()> {
        self.validate_account_shape(account)?;
        let ledger = account.close_progress;
        if !ledger.active
            || ledger.finalized
            || ledger.canceled
            || ledger.has_irreversible_progress()
            || ledger.residual_remaining != ledger.gross_loss_at_close_start
        {
            return Err(V16Error::LockActive);
        }
        let domain =
            self.insurance_domain_index(ledger.asset_index as usize, ledger.domain_side)?;
        if self.pending_domain_loss_barriers[domain] == 0 {
            return Err(V16Error::LockActive);
        }
        let vault_before = self.vault;
        let escrow_before = account.cancel_deposit_escrow;
        let escrow_total = account
            .cancel_deposit_escrow
            .checked_add(optional_deposit)
            .ok_or(V16Error::ArithmeticOverflow)?;
        let new_vault = self
            .vault
            .checked_add(optional_deposit)
            .ok_or(V16Error::ArithmeticOverflow)?;
        if new_vault > MAX_VAULT_TVL {
            return Err(V16Error::InvalidConfig);
        }
        let new_capital = account
            .capital
            .checked_add(escrow_total)
            .ok_or(V16Error::ArithmeticOverflow)?;
        let new_c_tot = self
            .c_tot
            .checked_add(escrow_total)
            .ok_or(V16Error::ArithmeticOverflow)?;

        let cert = self.full_account_refresh(account, effective_prices)?;
        let escrow_i128 = i128::try_from(escrow_total).map_err(|_| V16Error::ArithmeticOverflow)?;
        let cured_equity = cert
            .certified_equity
            .checked_add(escrow_i128)
            .ok_or(V16Error::ArithmeticOverflow)?;
        if cured_equity < 0 || (cured_equity as u128) < cert.certified_initial_req {
            return Err(V16Error::InvalidConfig);
        }

        self.vault = new_vault;
        self.c_tot = new_c_tot;
        account.capital = new_capital;
        account.cancel_deposit_escrow = 0;
        self.pending_domain_loss_barriers[domain] = self.pending_domain_loss_barriers[domain]
            .checked_sub(1)
            .ok_or(V16Error::CounterUnderflow)?;
        account.close_progress = CloseProgressLedgerV16 {
            active: false,
            finalized: false,
            canceled: true,
            ..ledger
        };
        TokenValueFlowProofV16::close_cure_to_account_capital(
            optional_deposit,
            escrow_before,
            escrow_total,
            vault_before,
            self.vault,
        )?
        .validate()?;
        account.health_cert.valid = false;
        self.validate_account_shape(account)?;
        self.assert_public_invariants()
    }

    pub fn settle_negative_pnl_from_principal(
        &mut self,
        account: &mut PortfolioAccountV16,
    ) -> V16Result<u128> {
        self.validate_account_shape(account)?;
        if account.pnl >= 0 {
            return Ok(0);
        }
        let loss = account.pnl.unsigned_abs();
        let paid = account.capital.min(loss);
        if paid == 0 {
            self.bankruptcy_hlock_active = true;
            return Ok(0);
        }
        let vault_before = self.vault;
        account.capital -= paid;
        self.c_tot = self
            .c_tot
            .checked_sub(paid)
            .ok_or(V16Error::CounterUnderflow)?;
        let paid_i128 = i128::try_from(paid).map_err(|_| V16Error::ArithmeticOverflow)?;
        let new_pnl = account
            .pnl
            .checked_add(paid_i128)
            .ok_or(V16Error::ArithmeticOverflow)?;
        self.set_account_pnl(account, new_pnl)?;
        if account.pnl < 0 {
            self.bankruptcy_hlock_active = true;
        }
        TokenValueFlowProofV16::account_capital_to_realized_loss(paid, vault_before, self.vault)?
            .validate()?;
        account.health_cert.valid = false;
        self.assert_public_invariants()?;
        Ok(paid)
    }

    pub fn charge_account_fee_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV16,
        requested_fee: u128,
    ) -> V16Result<u128> {
        if self.mode != MarketModeV16::Live {
            return Err(V16Error::LockActive);
        }
        self.charge_account_fee_after_loss_settlement(account, requested_fee)
    }

    fn charge_account_fee_after_loss_settlement(
        &mut self,
        account: &mut PortfolioAccountV16,
        requested_fee: u128,
    ) -> V16Result<u128> {
        self.settle_account_side_effects_not_atomic(account, self.config.public_b_chunk_atoms)?;
        if account.b_stale_state || has_b_stale_leg(account) {
            return Err(V16Error::BStale);
        }
        self.settle_negative_pnl_from_principal(account)?;
        if requested_fee == 0 || account.pnl < 0 {
            return Ok(0);
        }
        let charged = requested_fee.min(account.capital);
        if charged == 0 {
            return Ok(0);
        }
        let vault_before = self.vault;
        account.capital -= charged;
        self.c_tot = self
            .c_tot
            .checked_sub(charged)
            .ok_or(V16Error::CounterUnderflow)?;
        self.insurance = self
            .insurance
            .checked_add(charged)
            .ok_or(V16Error::ArithmeticOverflow)?;
        TokenValueFlowProofV16::account_capital_to_insurance(charged, vault_before, self.vault)?
            .validate()?;
        account.health_cert.valid = false;
        self.assert_public_invariants()?;
        Ok(charged)
    }

    fn charge_account_fee_current_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV16,
        requested_fee: u128,
    ) -> V16Result<u128> {
        if requested_fee == 0 || account.pnl < 0 {
            return Ok(0);
        }
        let charged = requested_fee.min(account.capital);
        if charged == 0 {
            return Ok(0);
        }
        let vault_before = self.vault;
        account.capital -= charged;
        self.c_tot = self
            .c_tot
            .checked_sub(charged)
            .ok_or(V16Error::CounterUnderflow)?;
        self.insurance = self
            .insurance
            .checked_add(charged)
            .ok_or(V16Error::ArithmeticOverflow)?;
        TokenValueFlowProofV16::account_capital_to_insurance(charged, vault_before, self.vault)?
            .validate()?;
        account.health_cert.valid = false;
        Ok(charged)
    }

    fn recertify_account_after_source_lien_change(
        &self,
        account: &mut PortfolioAccountV16,
    ) -> V16Result<HealthCertV16> {
        let existing = account.health_cert;
        if existing.active_bitmap_at_cert != account.active_bitmap {
            return Err(V16Error::Stale);
        }
        let equity = self.account_haircut_equity(account)?;
        let certified_liq_deficit = if equity < 0 {
            equity.unsigned_abs()
        } else {
            let e = equity as u128;
            existing.certified_maintenance_req.saturating_sub(e)
        };
        let cert = HealthCertV16 {
            certified_equity: equity,
            certified_initial_req: existing.certified_initial_req,
            certified_maintenance_req: existing.certified_maintenance_req,
            certified_liq_deficit,
            certified_worst_case_loss: existing.certified_worst_case_loss,
            cert_oracle_epoch: self.oracle_epoch,
            cert_funding_epoch: self.funding_epoch,
            cert_risk_epoch: self.risk_epoch,
            cert_asset_set_epoch: self.asset_set_epoch,
            active_bitmap_at_cert: account.active_bitmap,
            valid: true,
        };
        account.health_cert = cert;
        self.validate_account_shape(account)?;
        Ok(cert)
    }

    fn recertify_account_after_trade_delta(
        &self,
        account: &mut PortfolioAccountV16,
        asset_index: usize,
        old_abs_q: u128,
        price: u64,
    ) -> V16Result<HealthCertV16> {
        if asset_index >= self.config.max_portfolio_assets as usize
            || price == 0
            || price > MAX_ORACLE_PRICE
        {
            return Err(V16Error::InvalidConfig);
        }
        let existing = account.health_cert;
        let new_abs_q = signed_position(account.legs[asset_index]).unsigned_abs();
        let old_notional = risk_notional_ceil(old_abs_q, price)?;
        let new_notional = risk_notional_ceil(new_abs_q, price)?;
        let old_initial = margin_requirement(
            old_notional,
            self.config.initial_margin_bps,
            self.config.min_nonzero_im_req,
        )?;
        let old_maintenance = margin_requirement(
            old_notional,
            self.config.maintenance_margin_bps,
            self.config.min_nonzero_mm_req,
        )?;
        let new_initial = margin_requirement(
            new_notional,
            self.config.initial_margin_bps,
            self.config.min_nonzero_im_req,
        )?;
        let new_maintenance = margin_requirement(
            new_notional,
            self.config.maintenance_margin_bps,
            self.config.min_nonzero_mm_req,
        )?;
        let initial_req = existing
            .certified_initial_req
            .checked_sub(old_initial)
            .and_then(|v| v.checked_add(new_initial))
            .ok_or(V16Error::ArithmeticOverflow)?;
        let maintenance_req = existing
            .certified_maintenance_req
            .checked_sub(old_maintenance)
            .and_then(|v| v.checked_add(new_maintenance))
            .ok_or(V16Error::ArithmeticOverflow)?;
        let worst_case_loss = existing
            .certified_worst_case_loss
            .checked_sub(old_notional)
            .and_then(|v| v.checked_add(new_notional))
            .ok_or(V16Error::ArithmeticOverflow)?;
        let equity = self.account_haircut_equity(account)?;
        let certified_liq_deficit = if equity < 0 {
            equity.unsigned_abs()
        } else {
            let e = equity as u128;
            maintenance_req.saturating_sub(e)
        };
        let cert = HealthCertV16 {
            certified_equity: equity,
            certified_initial_req: initial_req,
            certified_maintenance_req: maintenance_req,
            certified_liq_deficit,
            certified_worst_case_loss: worst_case_loss,
            cert_oracle_epoch: self.oracle_epoch,
            cert_funding_epoch: self.funding_epoch,
            cert_risk_epoch: self.risk_epoch,
            cert_asset_set_epoch: self.asset_set_epoch,
            active_bitmap_at_cert: account.active_bitmap,
            valid: true,
        };
        account.health_cert = cert;
        self.validate_account_shape(account)?;
        Ok(cert)
    }

    pub fn sync_account_fee_to_slot_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV16,
        now_slot: u64,
        fee_rate_per_slot: u128,
    ) -> V16Result<u128> {
        self.validate_account_shape(account)?;
        if matches!(self.mode, MarketModeV16::Recovery) {
            return Err(V16Error::LockActive);
        }
        if now_slot < account.last_fee_slot {
            return Err(V16Error::Stale);
        }
        let nonflat = account.active_bitmap != 0;
        let fee_anchor = if self.mode == MarketModeV16::Live && nonflat && now_slot > self.slot_last
        {
            self.slot_last
        } else if self.mode == MarketModeV16::Resolved {
            self.resolved_slot
        } else {
            now_slot
        };
        if fee_anchor <= account.last_fee_slot {
            return Ok(0);
        }
        let dt = fee_anchor - account.last_fee_slot;
        let raw_fee = U256::from_u128(fee_rate_per_slot)
            .checked_mul(U256::from_u64(dt))
            .ok_or(V16Error::ArithmeticOverflow)?;
        let requested_fee = raw_fee.try_into_u128().unwrap_or(u128::MAX);
        let charged = self.charge_account_fee_after_loss_settlement(account, requested_fee)?;
        account.last_fee_slot = fee_anchor;
        Ok(charged)
    }

    pub fn convert_released_pnl_to_capital_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV16,
    ) -> V16Result<u128> {
        self.validate_account_shape(account)?;
        if self.mode != MarketModeV16::Live || self.payout_snapshot_captured {
            return Err(V16Error::LockActive);
        }
        self.ensure_favorable_action_allowed(account)?;
        let pos = account.pnl.max(0) as u128;
        let released = pos.saturating_sub(account.reserved_pnl);
        if released == 0 {
            return Ok(0);
        }
        if Self::account_has_source_claims(account)?
            && self.account_has_active_source_claim_exposure(account)?
        {
            return Err(V16Error::LockActive);
        }
        let residual = self.residual();
        let junior_bound = self.junior_claim_bound();
        let global_support = self.haircut_effective_support(released, residual, junior_bound)?;
        let converted = if Self::account_has_source_claims(account)? {
            global_support.min(self.account_source_realizable_support(account, released)?)
        } else {
            global_support
        };
        if converted == 0 {
            return Err(V16Error::LockActive);
        }
        let vault_before = self.vault;
        let consumption = if Self::account_has_source_claims(account)? {
            self.create_and_consume_account_source_credit_for_effective_not_atomic(
                account, converted,
            )?
        } else {
            SourceCreditConsumptionV16 {
                face_burn: self.face_claim_to_burn_for_support(
                    converted,
                    residual,
                    junior_bound,
                )?,
                counterparty_credit_consumed: 0,
                insurance_credit_consumed: 0,
                domain_effective_consumed: [0; V16_DOMAIN_COUNT],
            }
        };
        let face_i128 =
            i128::try_from(consumption.face_burn).map_err(|_| V16Error::ArithmeticOverflow)?;
        let new_pnl = account
            .pnl
            .checked_sub(face_i128)
            .ok_or(V16Error::ArithmeticOverflow)?;
        self.set_account_pnl(account, new_pnl)?;
        account.capital = account
            .capital
            .checked_add(converted)
            .ok_or(V16Error::ArithmeticOverflow)?;
        self.c_tot = self
            .c_tot
            .checked_add(converted)
            .ok_or(V16Error::ArithmeticOverflow)?;
        self.pnl_matured_pos_tot = self
            .pnl_matured_pos_tot
            .saturating_sub(consumption.face_burn);
        let protocol_surplus_consumed = converted
            .checked_sub(consumption.counterparty_credit_consumed)
            .and_then(|v| v.checked_sub(consumption.insurance_credit_consumed))
            .ok_or(V16Error::CounterUnderflow)?;
        TokenValueFlowProofV16::support_to_account_capital(
            converted,
            consumption.counterparty_credit_consumed,
            consumption.insurance_credit_consumed,
            protocol_surplus_consumed,
            vault_before,
            self.vault,
        )?
        .validate()?;
        account.health_cert.valid = false;
        self.assert_public_invariants()?;
        Ok(converted)
    }

    pub fn withdraw_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV16,
        amount: u128,
        effective_prices: &[u64; V16_MAX_PORTFOLIO_ASSETS_N],
    ) -> V16Result<()> {
        if amount == 0 {
            return Ok(());
        }
        if self.mode != MarketModeV16::Live {
            return Err(V16Error::LockActive);
        }
        self.settle_account_side_effects_not_atomic(account, self.config.public_b_chunk_atoms)?;
        self.full_account_refresh(account, effective_prices)?;
        let locked = self.h_lock_lane(Some(account), false)? == HLockLaneV16::HMax;
        if self.loss_stale_active && account.active_bitmap != 0 {
            return Err(V16Error::LockActive);
        }
        if self.account_has_target_effective_lag(account)? && account.active_bitmap != 0 {
            return Err(V16Error::LockActive);
        }
        self.settle_negative_pnl_from_principal(account)?;
        if account.pnl < 0 || amount > account.capital {
            return Err(V16Error::LockActive);
        }
        let post_capital = account.capital - amount;
        let initial_req = account.health_cert.certified_initial_req;
        if !locked && Self::account_has_source_claims(account)? {
            self.create_initial_margin_source_lien_with_capital_if_needed(account, post_capital)?;
        }
        let equity_after = if locked {
            account_no_positive_credit_equity_with_capital(account, post_capital)?
        } else {
            self.account_haircut_equity_with_capital(account, post_capital)?
        };
        if equity_after < 0 {
            return Err(V16Error::InvalidConfig);
        }
        let equity_after_u = equity_after as u128;
        if equity_after_u < initial_req {
            return Err(V16Error::InvalidConfig);
        }
        let vault_before = self.vault;
        account.capital = post_capital;
        self.c_tot = self
            .c_tot
            .checked_sub(amount)
            .ok_or(V16Error::CounterUnderflow)?;
        self.vault = self
            .vault
            .checked_sub(amount)
            .ok_or(V16Error::CounterUnderflow)?;
        TokenValueFlowProofV16::account_capital_to_external_out(amount, vault_before, self.vault)?
            .validate()?;
        account.health_cert.valid = false;
        self.assert_public_invariants()
    }

    pub fn release_account_source_credit_liens_if_unneeded_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV16,
        effective_prices: &[u64; V16_MAX_PORTFOLIO_ASSETS_N],
    ) -> V16Result<u128> {
        if self.mode != MarketModeV16::Live {
            return Err(V16Error::LockActive);
        }
        self.settle_account_side_effects_not_atomic(account, self.config.public_b_chunk_atoms)?;
        self.full_account_refresh(account, effective_prices)?;
        let no_positive = account_no_positive_credit_equity(account)?;
        if no_positive < 0 || (no_positive as u128) < account.health_cert.certified_initial_req {
            return Err(V16Error::LockActive);
        }

        let mut released_effective = 0u128;
        let mut d = 0;
        while d < V16_DOMAIN_COUNT {
            let effective = account.source_lien_effective_reserved[d];
            let counterparty_backing = account.source_lien_counterparty_backing_num[d];
            let insurance_backing = account.source_lien_insurance_backing_num[d];
            if counterparty_backing != 0 {
                self.release_source_credit_lien_from_counterparty_not_atomic(
                    d,
                    counterparty_backing,
                )?;
            }
            if insurance_backing != 0 {
                self.release_source_credit_lien_from_insurance_not_atomic(d, insurance_backing)?;
            }
            if effective != 0 {
                released_effective = released_effective
                    .checked_add(effective)
                    .ok_or(V16Error::ArithmeticOverflow)?;
                account.source_claim_liened_num[d] = 0;
                account.source_claim_counterparty_liened_num[d] = 0;
                account.source_claim_insurance_liened_num[d] = 0;
                account.source_lien_effective_reserved[d] = 0;
                account.source_lien_counterparty_backing_num[d] = 0;
                account.source_lien_insurance_backing_num[d] = 0;
            }
            d += 1;
        }
        account.health_cert.valid = false;
        self.validate_account_shape(account)?;
        self.assert_public_invariants()?;
        Ok(released_effective)
    }

    pub fn impair_account_source_credit_lien_from_insurance_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV16,
        domain: usize,
    ) -> V16Result<u128> {
        self.validate_source_domain_index(domain)?;
        self.validate_account_shape(account)?;
        let insurance_backing = account.source_lien_insurance_backing_num[domain];
        if insurance_backing == 0 {
            return Ok(0);
        }
        let effective = insurance_backing / BOUND_SCALE;
        let face = account.source_claim_insurance_liened_num[domain];
        if effective == 0 || face == 0 {
            return Err(V16Error::InvalidLeg);
        }

        self.impair_source_credit_lien_from_insurance_not_atomic(domain, insurance_backing)?;
        account.source_claim_insurance_liened_num[domain] = 0;
        account.source_claim_liened_num[domain] = account.source_claim_liened_num[domain]
            .checked_sub(face)
            .ok_or(V16Error::CounterUnderflow)?;
        account.source_claim_impaired_num[domain] = account.source_claim_impaired_num[domain]
            .checked_add(face)
            .ok_or(V16Error::CounterOverflow)?;
        account.source_lien_insurance_backing_num[domain] = 0;
        account.source_lien_effective_reserved[domain] = account.source_lien_effective_reserved
            [domain]
            .checked_sub(effective)
            .ok_or(V16Error::CounterUnderflow)?;
        account.source_lien_impaired_effective_reserved[domain] = account
            .source_lien_impaired_effective_reserved[domain]
            .checked_add(effective)
            .ok_or(V16Error::CounterOverflow)?;
        account.health_cert.valid = false;
        self.validate_account_shape(account)?;
        self.assert_public_invariants()?;
        Ok(effective)
    }

    fn reconcile_account_source_credit_liens_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV16,
    ) -> V16Result<u128> {
        let configured_domains = self.config.max_portfolio_assets as usize * 2;
        let mut impaired_effective = 0u128;
        let mut d = 0;
        while d < configured_domains {
            let bucket = self.source_backing_buckets[d];
            if bucket.status == BackingBucketStatusV16::Fresh
                && bucket.expiry_slot != 0
                && self.current_slot >= bucket.expiry_slot
            {
                self.expire_source_backing_bucket_not_atomic(d, self.current_slot)?;
            }

            let counterparty_backing = account.source_lien_counterparty_backing_num[d];
            if counterparty_backing != 0
                && self.source_backing_buckets[d].status != BackingBucketStatusV16::Fresh
            {
                let effective = counterparty_backing / BOUND_SCALE;
                let face = account.source_claim_counterparty_liened_num[d];
                if effective == 0 || face == 0 {
                    return Err(V16Error::InvalidLeg);
                }
                account.source_claim_counterparty_liened_num[d] = 0;
                account.source_claim_liened_num[d] = account.source_claim_liened_num[d]
                    .checked_sub(face)
                    .ok_or(V16Error::CounterUnderflow)?;
                account.source_claim_impaired_num[d] = account.source_claim_impaired_num[d]
                    .checked_add(face)
                    .ok_or(V16Error::CounterOverflow)?;
                account.source_lien_counterparty_backing_num[d] = 0;
                account.source_lien_effective_reserved[d] = account.source_lien_effective_reserved
                    [d]
                    .checked_sub(effective)
                    .ok_or(V16Error::CounterUnderflow)?;
                account.source_lien_impaired_effective_reserved[d] = account
                    .source_lien_impaired_effective_reserved[d]
                    .checked_add(effective)
                    .ok_or(V16Error::CounterOverflow)?;
                impaired_effective = impaired_effective
                    .checked_add(effective)
                    .ok_or(V16Error::CounterOverflow)?;
            }
            d += 1;
        }
        if impaired_effective != 0 {
            account.health_cert.valid = false;
            self.validate_account_shape(account)?;
        }
        Ok(impaired_effective)
    }

    pub fn mark_account_stale(&mut self, account: &mut PortfolioAccountV16) -> V16Result<()> {
        self.validate_portfolio_account_provenance(account)?;
        if !account.stale_state {
            account.stale_state = true;
            account.health_cert.valid = false;
            self.stale_certificate_count = self
                .stale_certificate_count
                .checked_add(1)
                .ok_or(V16Error::CounterOverflow)?;
        }
        Ok(())
    }

    pub fn clear_account_stale(&mut self, account: &mut PortfolioAccountV16) -> V16Result<()> {
        self.validate_portfolio_account_provenance(account)?;
        if account.stale_state {
            account.stale_state = false;
            self.stale_certificate_count = self
                .stale_certificate_count
                .checked_sub(1)
                .ok_or(V16Error::CounterUnderflow)?;
        }
        Ok(())
    }

    pub fn mark_account_b_stale(&mut self, account: &mut PortfolioAccountV16) -> V16Result<()> {
        self.validate_portfolio_account_provenance(account)?;
        if !account.b_stale_state {
            account.b_stale_state = true;
            account.health_cert.valid = false;
            self.b_stale_account_count = self
                .b_stale_account_count
                .checked_add(1)
                .ok_or(V16Error::CounterOverflow)?;
        }
        Ok(())
    }

    pub fn clear_account_b_stale(&mut self, account: &mut PortfolioAccountV16) -> V16Result<()> {
        self.validate_portfolio_account_provenance(account)?;
        if has_b_stale_leg(account) {
            return Err(V16Error::BStale);
        }
        if account.b_stale_state {
            account.b_stale_state = false;
            self.b_stale_account_count = self
                .b_stale_account_count
                .checked_sub(1)
                .ok_or(V16Error::CounterUnderflow)?;
        }
        Ok(())
    }

    pub fn mark_asset_drain_only_not_atomic(&mut self, asset_index: usize) -> V16Result<()> {
        self.validate_configured_asset_index(asset_index)?;
        if self.mode != MarketModeV16::Live {
            return Err(V16Error::LockActive);
        }
        match self.assets[asset_index].lifecycle {
            AssetLifecycleV16::Active => {
                self.assets[asset_index].lifecycle = AssetLifecycleV16::DrainOnly;
                self.bump_asset_set_epoch()?;
                self.assert_public_invariants()
            }
            AssetLifecycleV16::DrainOnly => Ok(()),
            _ => Err(V16Error::LockActive),
        }
    }

    pub fn retire_empty_asset_not_atomic(&mut self, asset_index: usize) -> V16Result<()> {
        self.validate_configured_asset_index(asset_index)?;
        match self.assets[asset_index].lifecycle {
            AssetLifecycleV16::Active
            | AssetLifecycleV16::DrainOnly
            | AssetLifecycleV16::Recovery => {
                self.require_empty_asset_lifecycle_state(asset_index)?;
                self.assets[asset_index].lifecycle = AssetLifecycleV16::Retired;
                self.bump_asset_set_epoch()?;
                self.assert_public_invariants()
            }
            AssetLifecycleV16::Retired => {
                self.require_empty_asset_lifecycle_state(asset_index)?;
                self.assert_public_invariants()
            }
            _ => Err(V16Error::LockActive),
        }
    }

    pub fn activate_empty_asset_not_atomic(
        &mut self,
        asset_index: usize,
        authenticated_price: u64,
        now_slot: u64,
    ) -> V16Result<()> {
        self.validate_configured_asset_index(asset_index)?;
        if self.mode != MarketModeV16::Live {
            return Err(V16Error::LockActive);
        }
        if authenticated_price == 0
            || authenticated_price > MAX_ORACLE_PRICE
            || now_slot < self.current_slot
        {
            return Err(V16Error::InvalidConfig);
        }
        if self.asset_activation_count != 0 {
            let elapsed = now_slot
                .checked_sub(self.last_asset_activation_slot)
                .ok_or(V16Error::Stale)?;
            if elapsed < self.config.asset_activation_cooldown_slots {
                return Err(V16Error::LockActive);
            }
        }
        match self.assets[asset_index].lifecycle {
            AssetLifecycleV16::Disabled | AssetLifecycleV16::Retired => {}
            _ => return Err(V16Error::LockActive),
        }
        self.config.validate_public_user_fund()?;
        self.require_empty_asset_lifecycle_state(asset_index)?;
        let mut asset = AssetStateV16::default();
        asset.lifecycle = AssetLifecycleV16::Active;
        asset.raw_oracle_target_price = authenticated_price;
        asset.effective_price = authenticated_price;
        asset.fund_px_last = authenticated_price;
        asset.slot_last = now_slot;
        self.assets[asset_index] = asset;
        self.current_slot = now_slot;
        self.asset_activation_count = self
            .asset_activation_count
            .checked_add(1)
            .ok_or(V16Error::CounterOverflow)?;
        self.last_asset_activation_slot = now_slot;
        self.bump_asset_set_epoch()?;
        self.assert_public_invariants()
    }

    pub fn attach_leg(
        &mut self,
        account: &mut PortfolioAccountV16,
        asset_index: usize,
        side: SideV16,
        basis_pos_q: i128,
    ) -> V16Result<()> {
        self.validate_portfolio_account_provenance(account)?;
        if asset_index >= self.config.max_portfolio_assets as usize {
            return Err(V16Error::InvalidLeg);
        }
        self.require_asset_active_for_risk_increase(asset_index)?;
        if self.has_pending_domain_loss_barrier(asset_index, side)? {
            return Err(V16Error::LockActive);
        }
        if account.legs[asset_index].active || ((account.active_bitmap >> asset_index) & 1) != 0 {
            return Err(V16Error::InvalidLeg);
        }
        validate_basis(basis_pos_q)?;

        let asset = self.assets[asset_index];
        let (a_basis, k_snap, f_snap, b_snap, epoch_snap) = match side {
            SideV16::Long => (
                asset.a_long,
                asset.k_long,
                asset.f_long_num,
                asset.b_long_num,
                asset.epoch_long,
            ),
            SideV16::Short => (
                asset.a_short,
                asset.k_short,
                asset.f_short_num,
                asset.b_short_num,
                asset.epoch_short,
            ),
        };
        if !(MIN_A_SIDE..=ADL_ONE).contains(&a_basis) {
            return Err(V16Error::InvalidLeg);
        }
        let loss_weight = loss_weight_for_basis(basis_pos_q.unsigned_abs(), a_basis)?;
        if loss_weight == 0 {
            return Err(V16Error::InvalidLeg);
        }

        let asset = &mut self.assets[asset_index];
        match side {
            SideV16::Long => {
                asset.stored_pos_count_long = asset
                    .stored_pos_count_long
                    .checked_add(1)
                    .ok_or(V16Error::CounterOverflow)?;
                asset.oi_eff_long_q = asset
                    .oi_eff_long_q
                    .checked_add(basis_pos_q.unsigned_abs())
                    .ok_or(V16Error::ArithmeticOverflow)?;
                asset.loss_weight_sum_long = asset
                    .loss_weight_sum_long
                    .checked_add(loss_weight)
                    .ok_or(V16Error::ArithmeticOverflow)?;
            }
            SideV16::Short => {
                asset.stored_pos_count_short = asset
                    .stored_pos_count_short
                    .checked_add(1)
                    .ok_or(V16Error::CounterOverflow)?;
                asset.oi_eff_short_q = asset
                    .oi_eff_short_q
                    .checked_add(basis_pos_q.unsigned_abs())
                    .ok_or(V16Error::ArithmeticOverflow)?;
                asset.loss_weight_sum_short = asset
                    .loss_weight_sum_short
                    .checked_add(loss_weight)
                    .ok_or(V16Error::ArithmeticOverflow)?;
            }
        }
        account.legs[asset_index] = PortfolioLegV16 {
            active: true,
            side,
            basis_pos_q,
            a_basis,
            k_snap,
            f_snap,
            epoch_snap,
            loss_weight,
            b_snap,
            b_rem: 0,
            b_epoch_snap: epoch_snap,
            b_stale: false,
            stale: false,
        };
        account.active_bitmap |= 1u32 << asset_index;
        account.health_cert.valid = false;
        self.validate_account_shape(account)
    }

    pub fn clear_leg(
        &mut self,
        account: &mut PortfolioAccountV16,
        asset_index: usize,
    ) -> V16Result<()> {
        self.validate_account_shape(account)?;
        if asset_index >= self.config.max_portfolio_assets as usize {
            return Err(V16Error::InvalidLeg);
        }
        let leg = account.legs[asset_index];
        if !leg.active || leg.b_stale || leg.stale {
            return Err(V16Error::InvalidLeg);
        }
        if account.close_progress.has_pending_residual() {
            return Err(V16Error::LockActive);
        }
        if self.has_pending_domain_loss_barrier(asset_index, leg.side)? {
            return Err(V16Error::LockActive);
        }
        let (k_target, f_target) = self.kf_target_for_leg(asset_index, leg)?;
        if k_target != leg.k_snap || f_target != leg.f_snap {
            return Err(V16Error::Stale);
        }
        if self.b_target_for_leg(asset_index, leg)? != leg.b_snap {
            return Err(V16Error::Stale);
        }
        let asset_snapshot = self.assets[asset_index];
        let prior_reset_epoch = match leg.side {
            SideV16::Long => {
                asset_snapshot.mode_long == SideModeV16::ResetPending
                    && leg.epoch_snap.checked_add(1) == Some(asset_snapshot.epoch_long)
            }
            SideV16::Short => {
                asset_snapshot.mode_short == SideModeV16::ResetPending
                    && leg.epoch_snap.checked_add(1) == Some(asset_snapshot.epoch_short)
            }
        };
        let dust_after_clear = if !prior_reset_epoch && leg.b_rem != 0 {
            let current_dust = match leg.side {
                SideV16::Long => asset_snapshot.social_loss_dust_long_num,
                SideV16::Short => asset_snapshot.social_loss_dust_short_num,
            };
            let new_dust = current_dust
                .checked_add(leg.b_rem)
                .ok_or(V16Error::ArithmeticOverflow)?;
            if new_dust >= SOCIAL_LOSS_DEN {
                return Err(V16Error::RecoveryRequired);
            }
            Some(new_dust)
        } else {
            None
        };
        let asset = &mut self.assets[asset_index];
        match leg.side {
            SideV16::Long => {
                asset.stored_pos_count_long = asset
                    .stored_pos_count_long
                    .checked_sub(1)
                    .ok_or(V16Error::CounterUnderflow)?;
                if leg.basis_pos_q == 0 && leg.loss_weight != 0 {
                    asset.pending_obligation_count_long = asset
                        .pending_obligation_count_long
                        .checked_sub(1)
                        .ok_or(V16Error::CounterUnderflow)?;
                }
                if !prior_reset_epoch {
                    if let Some(new_dust) = dust_after_clear {
                        asset.social_loss_dust_long_num = new_dust;
                    }
                    asset.oi_eff_long_q = asset
                        .oi_eff_long_q
                        .checked_sub(leg.basis_pos_q.unsigned_abs())
                        .ok_or(V16Error::CounterUnderflow)?;
                    asset.loss_weight_sum_long = asset
                        .loss_weight_sum_long
                        .checked_sub(leg.loss_weight)
                        .ok_or(V16Error::CounterUnderflow)?;
                }
            }
            SideV16::Short => {
                asset.stored_pos_count_short = asset
                    .stored_pos_count_short
                    .checked_sub(1)
                    .ok_or(V16Error::CounterUnderflow)?;
                if leg.basis_pos_q == 0 && leg.loss_weight != 0 {
                    asset.pending_obligation_count_short = asset
                        .pending_obligation_count_short
                        .checked_sub(1)
                        .ok_or(V16Error::CounterUnderflow)?;
                }
                if !prior_reset_epoch {
                    if let Some(new_dust) = dust_after_clear {
                        asset.social_loss_dust_short_num = new_dust;
                    }
                    asset.oi_eff_short_q = asset
                        .oi_eff_short_q
                        .checked_sub(leg.basis_pos_q.unsigned_abs())
                        .ok_or(V16Error::CounterUnderflow)?;
                    asset.loss_weight_sum_short = asset
                        .loss_weight_sum_short
                        .checked_sub(leg.loss_weight)
                        .ok_or(V16Error::CounterUnderflow)?;
                }
            }
        }
        account.legs[asset_index] = PortfolioLegV16::EMPTY;
        account.active_bitmap &= !(1u32 << asset_index);
        account.health_cert.valid = false;
        self.validate_account_shape(account)
    }

    pub fn mark_leg_b_stale(
        &mut self,
        account: &mut PortfolioAccountV16,
        asset_index: usize,
    ) -> V16Result<()> {
        self.validate_account_shape(account)?;
        if asset_index >= self.config.max_portfolio_assets as usize
            || !account.legs[asset_index].active
        {
            return Err(V16Error::InvalidLeg);
        }
        account.legs[asset_index].b_stale = true;
        self.mark_account_b_stale(account)
    }

    pub fn h_lock_lane(
        &self,
        account: Option<&PortfolioAccountV16>,
        instruction_bankruptcy_candidate: bool,
    ) -> V16Result<HLockLaneV16> {
        if let Some(account) = account {
            self.validate_portfolio_account_provenance(account)?;
            if account.stale_state || account.b_stale_state {
                return Ok(HLockLaneV16::HMax);
            }
            if account.close_progress.has_pending_residual() {
                return Ok(HLockLaneV16::HMax);
            }
            if self.account_touches_pending_domain_loss_barrier(account)? {
                return Ok(HLockLaneV16::HMax);
            }
        }

        if self.threshold_stress_active
            || self.bankruptcy_hlock_active
            || self.mode == MarketModeV16::Recovery
            || instruction_bankruptcy_candidate
            || self.loss_stale_active
        {
            return Ok(HLockLaneV16::HMax);
        }

        Ok(HLockLaneV16::HMin)
    }

    pub fn select_h_lock(
        &self,
        account: Option<&PortfolioAccountV16>,
        instruction_bankruptcy_candidate: bool,
    ) -> V16Result<u64> {
        match self.h_lock_lane(account, instruction_bankruptcy_candidate)? {
            HLockLaneV16::HMin => Ok(self.config.h_min),
            HLockLaneV16::HMax => Ok(self.config.h_max),
        }
    }

    fn asset_has_target_effective_lag(&self, asset_index: usize) -> V16Result<bool> {
        if asset_index >= self.config.max_portfolio_assets as usize {
            return Err(V16Error::InvalidLeg);
        }
        let asset = self.assets[asset_index];
        Ok(asset.raw_oracle_target_price != asset.effective_price)
    }

    fn account_has_target_effective_lag(&self, account: &PortfolioAccountV16) -> V16Result<bool> {
        self.validate_account_shape(account)?;
        for i in 0..self.config.max_portfolio_assets as usize {
            if account.legs[i].active && self.asset_has_target_effective_lag(i)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn full_account_refresh(
        &mut self,
        account: &mut PortfolioAccountV16,
        effective_prices: &[u64; V16_MAX_PORTFOLIO_ASSETS_N],
    ) -> V16Result<HealthCertV16> {
        self.validate_account_shape(account)?;
        self.reconcile_account_source_credit_liens_not_atomic(account)?;
        let n = self.config.max_portfolio_assets as usize;
        for i in 0..n {
            if !account.legs[i].active {
                continue;
            }
            self.settle_leg_kf_effects(account, i)?;
            if self.b_target_for_leg(i, account.legs[i])? > account.legs[i].b_snap {
                self.mark_leg_b_stale(account, i)?;
            }
        }
        if account.b_stale_state {
            return Err(V16Error::BStale);
        }
        if account.stale_state {
            self.clear_account_stale(account)?;
        }

        let mut initial_req = 0u128;
        let mut maintenance_req = 0u128;
        let mut worst_case_loss = 0u128;
        for i in 0..n {
            if !account.legs[i].active {
                continue;
            }
            let price = effective_prices[i];
            if price == 0 || price > MAX_ORACLE_PRICE {
                return Err(V16Error::InvalidConfig);
            }
            let risk_notional =
                risk_notional_ceil(account.legs[i].basis_pos_q.unsigned_abs(), price)?;
            let leg_initial = margin_requirement(
                risk_notional,
                self.config.initial_margin_bps,
                self.config.min_nonzero_im_req,
            )?;
            let leg_maintenance = margin_requirement(
                risk_notional,
                self.config.maintenance_margin_bps,
                self.config.min_nonzero_mm_req,
            )?;
            initial_req = initial_req
                .checked_add(leg_initial)
                .ok_or(V16Error::ArithmeticOverflow)?;
            maintenance_req = maintenance_req
                .checked_add(leg_maintenance)
                .ok_or(V16Error::ArithmeticOverflow)?;
            worst_case_loss = worst_case_loss
                .checked_add(risk_notional)
                .ok_or(V16Error::ArithmeticOverflow)?;
        }

        let equity = self.account_haircut_equity(account)?;
        let certified_liq_deficit = if equity < 0 {
            equity.unsigned_abs()
        } else {
            let e = equity as u128;
            maintenance_req.saturating_sub(e)
        };
        let cert = HealthCertV16 {
            certified_equity: equity,
            certified_initial_req: initial_req,
            certified_maintenance_req: maintenance_req,
            certified_liq_deficit,
            certified_worst_case_loss: worst_case_loss,
            cert_oracle_epoch: self.oracle_epoch,
            cert_funding_epoch: self.funding_epoch,
            cert_risk_epoch: self.risk_epoch,
            cert_asset_set_epoch: self.asset_set_epoch,
            active_bitmap_at_cert: account.active_bitmap,
            valid: true,
        };
        account.health_cert = cert;
        self.validate_account_shape(account)?;
        Ok(cert)
    }

    pub fn ensure_favorable_action_allowed(&self, account: &PortfolioAccountV16) -> V16Result<()> {
        self.validate_account_shape(account)?;
        if self.h_lock_lane(Some(account), false)? == HLockLaneV16::HMax {
            return Err(V16Error::LockActive);
        }
        if !account.health_cert.valid
            || account.health_cert.cert_oracle_epoch != self.oracle_epoch
            || account.health_cert.cert_funding_epoch != self.funding_epoch
            || account.health_cert.cert_risk_epoch != self.risk_epoch
            || account.health_cert.cert_asset_set_epoch != self.asset_set_epoch
            || account.health_cert.active_bitmap_at_cert != account.active_bitmap
        {
            return Err(V16Error::Stale);
        }
        if self.account_has_target_effective_lag(account)? {
            return Err(V16Error::LockActive);
        }
        Ok(())
    }

    pub fn account_b_settlement_chunk(
        &self,
        account: &PortfolioAccountV16,
        asset_index: usize,
        endpoint_delta_budget: u128,
    ) -> V16Result<AccountBSettlementChunkV16> {
        self.validate_account_shape(account)?;
        if asset_index >= self.config.max_portfolio_assets as usize {
            return Err(V16Error::InvalidLeg);
        }
        let leg = account.legs[asset_index];
        if !leg.active {
            return Err(V16Error::InvalidLeg);
        }
        let target = self.b_target_for_leg(asset_index, leg)?;
        if target < leg.b_snap {
            return Err(V16Error::RecoveryRequired);
        }
        let b_remaining = target - leg.b_snap;
        if b_remaining == 0 {
            return Ok(AccountBSettlementChunkV16 {
                delta_b: 0,
                loss: 0,
                new_remainder: leg.b_rem,
                remaining_after: 0,
            });
        }
        if leg.loss_weight == 0 || endpoint_delta_budget == 0 {
            return Err(V16Error::RecoveryRequired);
        }

        let limit = self.config.public_b_chunk_atoms;
        let max_num = limit
            .checked_add(1)
            .and_then(|v| v.checked_mul(SOCIAL_LOSS_DEN))
            .and_then(|v| v.checked_sub(1))
            .ok_or(V16Error::ArithmeticOverflow)?;
        if leg.b_rem > max_num {
            return Err(V16Error::RecoveryRequired);
        }
        let max_delta_by_loss = (max_num - leg.b_rem) / leg.loss_weight;
        let delta_b = b_remaining
            .min(max_delta_by_loss)
            .min(endpoint_delta_budget);
        if delta_b == 0 {
            return Err(V16Error::RecoveryRequired);
        }
        let num = leg
            .loss_weight
            .checked_mul(delta_b)
            .and_then(|v| v.checked_add(leg.b_rem))
            .ok_or(V16Error::ArithmeticOverflow)?;
        let loss = num / SOCIAL_LOSS_DEN;
        let new_remainder = num % SOCIAL_LOSS_DEN;
        Ok(AccountBSettlementChunkV16 {
            delta_b,
            loss,
            new_remainder,
            remaining_after: b_remaining - delta_b,
        })
    }

    pub fn settle_account_b_chunk(
        &mut self,
        account: &mut PortfolioAccountV16,
        asset_index: usize,
        endpoint_delta_budget: u128,
    ) -> V16Result<AccountBSettlementChunkV16> {
        let chunk = self.account_b_settlement_chunk(account, asset_index, endpoint_delta_budget)?;
        if chunk.delta_b == 0 {
            if !has_b_stale_leg(account) {
                self.clear_account_b_stale(account)?;
            }
            return Ok(chunk);
        }
        let old_pnl = account.pnl;
        let loss_i128 = i128::try_from(chunk.loss).map_err(|_| V16Error::ArithmeticOverflow)?;
        let new_pnl = old_pnl
            .checked_sub(loss_i128)
            .ok_or(V16Error::ArithmeticOverflow)?;

        {
            let leg = &mut account.legs[asset_index];
            leg.b_snap = leg
                .b_snap
                .checked_add(chunk.delta_b)
                .ok_or(V16Error::ArithmeticOverflow)?;
            leg.b_rem = chunk.new_remainder;
            leg.b_stale = chunk.remaining_after != 0;
        }
        self.set_account_pnl(account, new_pnl)?;
        if chunk.remaining_after != 0 {
            self.mark_account_b_stale(account)?;
        } else if !has_b_stale_leg(account) {
            self.clear_account_b_stale(account)?;
        }
        account.health_cert.valid = false;
        self.validate_account_shape(account)?;
        Ok(chunk)
    }

    pub fn settle_account_side_effects_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV16,
        b_delta_budget: u128,
    ) -> V16Result<PermissionlessProgressOutcomeV16> {
        self.validate_account_shape(account)?;
        let n = self.config.max_portfolio_assets as usize;
        for i in 0..n {
            if !account.legs[i].active {
                continue;
            }
            self.settle_leg_kf_effects(account, i)?;
            let target = self.b_target_for_leg(i, account.legs[i])?;
            if target > account.legs[i].b_snap {
                self.mark_leg_b_stale(account, i)?;
                let chunk = self.settle_account_b_chunk(account, i, b_delta_budget)?;
                if chunk.remaining_after != 0 {
                    return Ok(PermissionlessProgressOutcomeV16::AccountBChunk(chunk));
                }
            }
        }
        self.settle_negative_pnl_from_principal(account)?;
        account.health_cert.valid = false;
        Ok(PermissionlessProgressOutcomeV16::AccountCurrent)
    }

    fn settle_account_for_position_action_and_refresh_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV16,
        effective_prices: &[u64; V16_MAX_PORTFOLIO_ASSETS_N],
    ) -> V16Result<HealthCertV16> {
        self.validate_account_shape(account)?;
        self.reconcile_account_source_credit_liens_not_atomic(account)?;
        let n = self.config.max_portfolio_assets as usize;
        let mut initial_req = 0u128;
        let mut maintenance_req = 0u128;
        let mut worst_case_loss = 0u128;
        for i in 0..n {
            if !account.legs[i].active {
                continue;
            }
            self.settle_leg_kf_effects(account, i)?;
            if self.b_target_for_leg(i, account.legs[i])? > account.legs[i].b_snap {
                self.mark_leg_b_stale(account, i)?;
                let chunk = self.settle_account_b_chunk(
                    account,
                    i,
                    self.config.public_b_chunk_atoms,
                )?;
                if chunk.remaining_after != 0 {
                    return Err(V16Error::BStale);
                }
            }
            let price = effective_prices[i];
            if price == 0 || price > MAX_ORACLE_PRICE {
                return Err(V16Error::InvalidConfig);
            }
            let risk_notional =
                risk_notional_ceil(account.legs[i].basis_pos_q.unsigned_abs(), price)?;
            let leg_initial = margin_requirement(
                risk_notional,
                self.config.initial_margin_bps,
                self.config.min_nonzero_im_req,
            )?;
            let leg_maintenance = margin_requirement(
                risk_notional,
                self.config.maintenance_margin_bps,
                self.config.min_nonzero_mm_req,
            )?;
            initial_req = initial_req
                .checked_add(leg_initial)
                .ok_or(V16Error::ArithmeticOverflow)?;
            maintenance_req = maintenance_req
                .checked_add(leg_maintenance)
                .ok_or(V16Error::ArithmeticOverflow)?;
            worst_case_loss = worst_case_loss
                .checked_add(risk_notional)
                .ok_or(V16Error::ArithmeticOverflow)?;
        }
        if account.b_stale_state || has_b_stale_leg(account) {
            return Err(V16Error::BStale);
        }
        if account.stale_state {
            self.clear_account_stale(account)?;
        }
        self.settle_negative_pnl_from_principal(account)?;
        let equity = self.account_haircut_equity(account)?;
        let certified_liq_deficit = if equity < 0 {
            equity.unsigned_abs()
        } else {
            let e = equity as u128;
            maintenance_req.saturating_sub(e)
        };
        let cert = HealthCertV16 {
            certified_equity: equity,
            certified_initial_req: initial_req,
            certified_maintenance_req: maintenance_req,
            certified_liq_deficit,
            certified_worst_case_loss: worst_case_loss,
            cert_oracle_epoch: self.oracle_epoch,
            cert_funding_epoch: self.funding_epoch,
            cert_risk_epoch: self.risk_epoch,
            cert_asset_set_epoch: self.asset_set_epoch,
            active_bitmap_at_cert: account.active_bitmap,
            valid: true,
        };
        account.health_cert = cert;
        self.validate_account_shape(account)?;
        Ok(cert)
    }

    pub fn accrue_asset_to_not_atomic(
        &mut self,
        asset_index: usize,
        now_slot: u64,
        effective_price: u64,
        funding_rate_e9: i128,
        protective_progress_committed: bool,
    ) -> V16Result<AccrueAssetOutcomeV16> {
        if self.mode != MarketModeV16::Live {
            return Err(V16Error::LockActive);
        }
        if asset_index >= self.config.max_portfolio_assets as usize
            || effective_price == 0
            || effective_price > MAX_ORACLE_PRICE
            || funding_rate_e9.unsigned_abs() > self.config.max_abs_funding_e9_per_slot as u128
            || now_slot < self.current_slot
            || now_slot < self.assets[asset_index].slot_last
        {
            return Err(V16Error::InvalidConfig);
        }
        self.require_asset_accruable(asset_index)?;
        let dt_total = now_slot - self.assets[asset_index].slot_last;
        let segment_dt = if dt_total > self.config.max_accrual_dt_slots {
            self.config.max_accrual_dt_slots
        } else {
            dt_total
        };
        let old = self.assets[asset_index];
        let exposed = old.oi_eff_long_q != 0 || old.oi_eff_short_q != 0;
        let balanced_exposure = old.oi_eff_long_q != 0 && old.oi_eff_short_q != 0;
        let price_move_active = effective_price != old.effective_price && exposed;
        let funding_active =
            segment_dt > 0 && funding_rate_e9 != 0 && balanced_exposure && old.fund_px_last > 0;
        let equity_active = price_move_active || funding_active;
        if equity_active {
            if segment_dt == 0 {
                return Err(V16Error::NonProgress);
            }
            let price_diff = effective_price.abs_diff(old.effective_price) as u128;
            let lhs = price_diff
                .checked_mul(MAX_MARGIN_BPS as u128)
                .ok_or(V16Error::ArithmeticOverflow)?;
            let rhs = (self.config.max_price_move_bps_per_slot as u128)
                .checked_mul(segment_dt as u128)
                .and_then(|v| v.checked_mul(old.effective_price as u128))
                .ok_or(V16Error::ArithmeticOverflow)?;
            if lhs > rhs {
                return Err(V16Error::RecoveryRequired);
            }
            if !protective_progress_committed {
                return Err(V16Error::NonProgress);
            }
        }

        let price_delta = effective_price as i128 - old.effective_price as i128;
        let k_delta = checked_i128_mul(price_delta, ADL_ONE as i128)?;
        let funding_delta = if funding_active {
            let n = funding_rate_e9
                .checked_mul(segment_dt as i128)
                .and_then(|v| v.checked_mul(effective_price as i128))
                .ok_or(V16Error::ArithmeticOverflow)?;
            floor_div_signed_conservative_i128(n, FUNDING_DEN)
                .checked_mul(ADL_ONE as i128)
                .ok_or(V16Error::ArithmeticOverflow)?
        } else {
            0
        };

        let asset = &mut self.assets[asset_index];
        asset.k_long = add_non_min_i128(asset.k_long, k_delta)?;
        asset.k_short = add_non_min_i128(asset.k_short, -k_delta)?;
        asset.f_long_num = add_non_min_i128(asset.f_long_num, -funding_delta)?;
        asset.f_short_num = add_non_min_i128(asset.f_short_num, funding_delta)?;
        asset.effective_price = effective_price;
        asset.fund_px_last = effective_price;
        asset.slot_last = asset
            .slot_last
            .checked_add(segment_dt)
            .ok_or(V16Error::ArithmeticOverflow)?;
        self.current_slot = now_slot;
        self.slot_last = asset.slot_last;
        self.loss_stale_active = asset.slot_last < now_slot;
        if price_move_active {
            self.oracle_epoch = self
                .oracle_epoch
                .checked_add(1)
                .ok_or(V16Error::CounterOverflow)?;
        }
        if funding_active {
            self.funding_epoch = self
                .funding_epoch
                .checked_add(1)
                .ok_or(V16Error::CounterOverflow)?;
        }
        self.assert_public_invariants()?;
        Ok(AccrueAssetOutcomeV16 {
            dt: segment_dt,
            price_move_active,
            funding_active,
            equity_active,
            loss_stale_after: self.loss_stale_active,
        })
    }

    #[cfg(not(target_os = "solana"))]
    pub fn execute_trade_with_fee_not_atomic(
        &mut self,
        long_account: &mut PortfolioAccountV16,
        short_account: &mut PortfolioAccountV16,
        request: TradeRequestV16,
        effective_prices: &[u64; V16_MAX_PORTFOLIO_ASSETS_N],
    ) -> V16Result<TradeOutcomeV16> {
        let mut staged_group = *self;
        let mut staged_long = *long_account;
        let mut staged_short = *short_account;
        let outcome = staged_group.execute_trade_with_fee_inner(
            &mut staged_long,
            &mut staged_short,
            request,
            effective_prices,
        )?;
        *self = staged_group;
        *long_account = staged_long;
        *short_account = staged_short;
        Ok(outcome)
    }

    pub fn execute_trade_with_fee_in_place_not_atomic(
        &mut self,
        long_account: &mut PortfolioAccountV16,
        short_account: &mut PortfolioAccountV16,
        request: TradeRequestV16,
        effective_prices: &[u64; V16_MAX_PORTFOLIO_ASSETS_N],
    ) -> V16Result<TradeOutcomeV16> {
        self.execute_trade_with_fee_inner(long_account, short_account, request, effective_prices)
    }

    fn execute_trade_with_fee_inner(
        &mut self,
        long_account: &mut PortfolioAccountV16,
        short_account: &mut PortfolioAccountV16,
        request: TradeRequestV16,
        effective_prices: &[u64; V16_MAX_PORTFOLIO_ASSETS_N],
    ) -> V16Result<TradeOutcomeV16> {
        if request.asset_index >= self.config.max_portfolio_assets as usize
            || request.size_q == 0
            || request.size_q > MAX_TRADE_SIZE_Q
            || request.exec_price == 0
            || request.exec_price > MAX_ORACLE_PRICE
            || request.fee_bps > self.config.max_trading_fee_bps
        {
            return Err(V16Error::InvalidConfig);
        }
        if self.mode != MarketModeV16::Live {
            return Err(V16Error::LockActive);
        }
        self.settle_account_for_position_action_and_refresh_not_atomic(
            long_account,
            effective_prices,
        )?;
        self.settle_account_for_position_action_and_refresh_not_atomic(
            short_account,
            effective_prices,
        )?;

        let long_delta =
            i128::try_from(request.size_q).map_err(|_| V16Error::ArithmeticOverflow)?;
        let short_delta = long_delta
            .checked_neg()
            .ok_or(V16Error::ArithmeticOverflow)?;
        let locked = self.h_lock_lane(Some(long_account), false)? == HLockLaneV16::HMax
            || self.h_lock_lane(Some(short_account), false)? == HLockLaneV16::HMax;
        let risk_increasing =
            position_delta_increases_risk(long_account, request.asset_index, long_delta)?
                || position_delta_increases_risk(short_account, request.asset_index, short_delta)?;
        let target_effective_lag = self.asset_has_target_effective_lag(request.asset_index)?;
        let touches_pending_domain_barrier =
            self.position_delta_blocked_by_pending_domain_loss_barrier(
                long_account,
                request.asset_index,
                long_delta,
            )? || self.position_delta_blocked_by_pending_domain_loss_barrier(
                short_account,
                request.asset_index,
                short_delta,
            )?;
        if touches_pending_domain_barrier {
            return Err(V16Error::LockActive);
        }
        if risk_increasing && (self.loss_stale_active || target_effective_lag) {
            return Err(V16Error::LockActive);
        }
        if risk_increasing {
            self.require_asset_active_for_risk_increase(request.asset_index)?;
        }

        let notional = trade_notional_floor(request.size_q, request.exec_price)?;
        let fee = checked_fee_bps(notional, request.fee_bps)?;
        let long_old_abs = signed_position(long_account.legs[request.asset_index]).unsigned_abs();
        let short_old_abs =
            signed_position(short_account.legs[request.asset_index]).unsigned_abs();
        self.charge_account_fee_current_not_atomic(long_account, fee)?;
        self.charge_account_fee_current_not_atomic(short_account, fee)?;
        self.apply_position_delta(long_account, request.asset_index, long_delta)?;
        self.apply_position_delta(short_account, request.asset_index, short_delta)?;
        self.recertify_account_after_trade_delta(
            long_account,
            request.asset_index,
            long_old_abs,
            effective_prices[request.asset_index],
        )?;
        self.recertify_account_after_trade_delta(
            short_account,
            request.asset_index,
            short_old_abs,
            effective_prices[request.asset_index],
        )?;
        if risk_increasing && !locked {
            self.create_initial_margin_source_lien_if_needed(long_account)?;
            self.create_initial_margin_source_lien_if_needed(short_account)?;
            self.recertify_account_after_source_lien_change(long_account)?;
            self.recertify_account_after_source_lien_change(short_account)?;
        }
        ensure_initial_margin(long_account)?;
        ensure_initial_margin(short_account)?;
        if locked {
            ensure_no_positive_credit_initial_margin(long_account)?;
            ensure_no_positive_credit_initial_margin(short_account)?;
        }
        self.assert_public_invariants()?;
        Ok(TradeOutcomeV16 {
            fee_a: fee,
            fee_b: fee,
            notional,
        })
    }

    pub fn liquidate_account_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV16,
        request: LiquidationRequestV16,
        effective_prices: &[u64; V16_MAX_PORTFOLIO_ASSETS_N],
    ) -> V16Result<LiquidationOutcomeV16> {
        if self.mode != MarketModeV16::Live {
            return Err(V16Error::LockActive);
        }
        if request.asset_index >= self.config.max_portfolio_assets as usize
            || request.close_q == 0
            || request.fee_bps
                > self
                    .config
                    .liquidation_fee_bps
                    .max(self.config.max_trading_fee_bps)
        {
            return Err(V16Error::InvalidConfig);
        }
        self.require_asset_live_reducible(request.asset_index)?;
        self.settle_account_side_effects_not_atomic(account, self.config.public_b_chunk_atoms)?;
        self.full_account_refresh(account, effective_prices)?;
        if account.health_cert.certified_liq_deficit == 0 {
            return Err(V16Error::NonProgress);
        }
        let before_score = self.risk_score_unchecked(account)?;
        let leg = account.legs[request.asset_index];
        if !leg.active {
            return Err(V16Error::InvalidLeg);
        }
        let close_q = request.close_q.min(leg.basis_pos_q.unsigned_abs());
        let close_i128 = i128::try_from(close_q).map_err(|_| V16Error::ArithmeticOverflow)?;
        let close_delta = match leg.side {
            SideV16::Long => close_i128
                .checked_neg()
                .ok_or(V16Error::ArithmeticOverflow)?,
            SideV16::Short => close_i128,
        };
        if self.position_delta_touches_pending_domain_loss_barrier(
            account,
            request.asset_index,
            close_delta,
        )? {
            return Err(V16Error::LockActive);
        }
        let uncovered_loss_after_principal = if account.pnl < 0 {
            account.pnl.unsigned_abs().saturating_sub(account.capital)
        } else {
            0
        };
        let remaining_active_bitmap = if close_q == leg.basis_pos_q.unsigned_abs() {
            account.active_bitmap & !(1u32 << request.asset_index)
        } else {
            account.active_bitmap
        };
        if uncovered_loss_after_principal != 0 && remaining_active_bitmap != 0 {
            self.declare_permissionless_recovery(
                PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress,
            )?;
            return Err(V16Error::RecoveryRequired);
        }
        self.preflight_liquidation_residual_durability(request.asset_index, leg.side, account)?;
        let fee_notional = risk_notional_ceil(close_q, effective_prices[request.asset_index])?;
        let fee = checked_fee_bps(fee_notional, request.fee_bps)?
            .max(self.config.min_liquidation_abs)
            .min(self.config.liquidation_fee_cap);
        let charged_fee = self.charge_account_fee_not_atomic(account, fee)?;
        self.settle_negative_pnl_from_principal(account)?;
        let gross_bankruptcy_residual = if account.pnl < 0 {
            account.pnl.unsigned_abs()
        } else {
            0
        };
        if gross_bankruptcy_residual != 0 {
            self.begin_close_progress_ledger(
                account,
                request.asset_index,
                opposite_side(leg.side),
                gross_bankruptcy_residual,
            )?;
        }
        let insurance_used =
            self.consume_domain_insurance_for_negative_pnl(request.asset_index, leg.side, account)?;
        if insurance_used != 0 {
            self.advance_close_progress_ledger(account, 0, 0, insurance_used, 0, 0)?;
        }
        let residual = if account.pnl < 0 {
            account.pnl.unsigned_abs()
        } else {
            0
        };
        let mut booked = 0u128;
        let mut explicit = 0u128;
        if residual != 0 {
            let bankrupt_side = leg.side;
            let outcome = self.book_bankruptcy_residual_chunk_for_account(
                account,
                request.asset_index,
                bankrupt_side,
                residual,
            )?;
            booked = outcome.booked_loss;
            explicit = outcome.explicit_loss;
            let cleared = booked
                .checked_add(explicit)
                .ok_or(V16Error::ArithmeticOverflow)?
                .min(residual);
            let cleared_i128 = i128::try_from(cleared).map_err(|_| V16Error::ArithmeticOverflow)?;
            self.set_account_pnl(
                account,
                account
                    .pnl
                    .checked_add(cleared_i128)
                    .ok_or(V16Error::ArithmeticOverflow)?,
            )?;
            self.bankruptcy_hlock_active = true;
        }
        self.reduce_position(account, request.asset_index, close_q)?;
        self.full_account_refresh(account, effective_prices)?;
        self.validate_liquidation_progress_from_score(before_score, account)?;
        self.assert_public_invariants()?;
        Ok(LiquidationOutcomeV16 {
            closed_q: close_q,
            insurance_used,
            residual_booked: booked,
            explicit_loss: explicit,
            fee_charged: charged_fee,
        })
    }

    pub fn forfeit_recovery_leg_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV16,
        asset_index: usize,
        b_delta_budget: u128,
    ) -> V16Result<DeadLegForfeitOutcomeV16> {
        self.validate_account_shape(account)?;
        if asset_index >= self.config.max_portfolio_assets as usize || b_delta_budget == 0 {
            return Err(V16Error::InvalidLeg);
        }
        let leg = account.legs[asset_index];
        if !leg.active {
            return Err(V16Error::InvalidLeg);
        }
        if !self.leg_is_dead_for_forfeit(asset_index, leg.side)? {
            return Err(V16Error::LockActive);
        }

        let (loss_settled, positive_pnl_forfeited, support_consumed, junior_face_burned) =
            self.settle_forfeited_leg_kf_effects(account, asset_index)?;

        let mut total_loss_settled = loss_settled;
        if self.b_target_for_leg(asset_index, account.legs[asset_index])?
            > account.legs[asset_index].b_snap
        {
            self.mark_leg_b_stale(account, asset_index)?;
            let chunk = self.settle_account_b_chunk(account, asset_index, b_delta_budget)?;
            total_loss_settled = total_loss_settled
                .checked_add(chunk.loss)
                .ok_or(V16Error::ArithmeticOverflow)?;
            if chunk.remaining_after != 0 {
                return Ok(DeadLegForfeitOutcomeV16 {
                    detached: false,
                    positive_pnl_forfeited,
                    loss_settled: total_loss_settled,
                    support_consumed,
                    junior_face_burned,
                    principal_used: 0,
                    insurance_used: 0,
                    residual_booked: 0,
                    explicit_loss: 0,
                });
            }
        }

        let principal_used = self.settle_negative_pnl_from_principal(account)?;
        let bankruptcy_residual_after_principal = if account.pnl < 0 {
            account.pnl.unsigned_abs()
        } else {
            0
        };
        let gross_close_loss = bankruptcy_residual_after_principal
            .checked_add(support_consumed)
            .ok_or(V16Error::ArithmeticOverflow)?;
        if gross_close_loss != 0 {
            self.begin_close_progress_ledger(
                account,
                asset_index,
                opposite_side(leg.side),
                gross_close_loss,
            )?;
            if support_consumed != 0 {
                self.advance_close_progress_ledger(
                    account,
                    support_consumed,
                    junior_face_burned,
                    0,
                    0,
                    0,
                )?;
            }
        }

        let insurance_used =
            self.consume_domain_insurance_for_negative_pnl(asset_index, leg.side, account)?;
        if insurance_used != 0 {
            self.advance_close_progress_ledger(account, 0, 0, insurance_used, 0, 0)?;
        }

        let residual = if account.pnl < 0 {
            account.pnl.unsigned_abs()
        } else {
            0
        };
        let mut residual_booked = 0u128;
        let mut explicit_loss = 0u128;
        if residual != 0 {
            let outcome = self.book_bankruptcy_residual_chunk_for_account(
                account,
                asset_index,
                leg.side,
                residual,
            )?;
            residual_booked = outcome.booked_loss;
            explicit_loss = outcome.explicit_loss;
            let cleared = residual_booked
                .checked_add(explicit_loss)
                .ok_or(V16Error::ArithmeticOverflow)?
                .min(residual);
            let cleared_i128 = i128::try_from(cleared).map_err(|_| V16Error::ArithmeticOverflow)?;
            self.set_account_pnl(
                account,
                account
                    .pnl
                    .checked_add(cleared_i128)
                    .ok_or(V16Error::ArithmeticOverflow)?,
            )?;
        }

        let detached = account.pnl >= 0 && !account.close_progress.has_pending_residual();
        if detached {
            self.clear_leg(account, asset_index)?;
        }

        self.assert_public_invariants()?;
        Ok(DeadLegForfeitOutcomeV16 {
            detached,
            positive_pnl_forfeited,
            loss_settled: total_loss_settled,
            support_consumed,
            junior_face_burned,
            principal_used,
            insurance_used,
            residual_booked,
            explicit_loss,
        })
    }

    pub fn rebalance_reduce_position_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV16,
        request: RebalanceRequestV16,
        effective_prices: &[u64; V16_MAX_PORTFOLIO_ASSETS_N],
    ) -> V16Result<RebalanceOutcomeV16> {
        if self.mode != MarketModeV16::Live {
            return Err(V16Error::LockActive);
        }
        if request.asset_index >= self.config.max_portfolio_assets as usize || request.reduce_q == 0
        {
            return Err(V16Error::InvalidConfig);
        }
        self.require_asset_live_reducible(request.asset_index)?;
        self.settle_account_side_effects_not_atomic(account, self.config.public_b_chunk_atoms)?;
        self.full_account_refresh(account, effective_prices)?;
        let before_score = self.risk_score_unchecked(account)?;
        let leg = account.legs[request.asset_index];
        if !leg.active {
            return Err(V16Error::InvalidLeg);
        }
        let reduce_q = request.reduce_q.min(leg.basis_pos_q.unsigned_abs());
        if reduce_q == 0 {
            return Err(V16Error::NonProgress);
        }
        let reduce_i128 = i128::try_from(reduce_q).map_err(|_| V16Error::ArithmeticOverflow)?;
        let reduce_delta = match leg.side {
            SideV16::Long => reduce_i128
                .checked_neg()
                .ok_or(V16Error::ArithmeticOverflow)?,
            SideV16::Short => reduce_i128,
        };
        if self.position_delta_blocked_by_pending_domain_loss_barrier(
            account,
            request.asset_index,
            reduce_delta,
        )? {
            return Err(V16Error::LockActive);
        }
        self.reduce_position(account, request.asset_index, reduce_q)?;
        self.settle_negative_pnl_from_principal(account)?;
        self.full_account_refresh(account, effective_prices)?;
        self.validate_liquidation_progress_from_score(before_score, account)?;
        self.assert_public_invariants()?;
        Ok(RebalanceOutcomeV16 {
            reduced_q: reduce_q,
        })
    }

    pub fn permissionless_crank_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV16,
        request: PermissionlessCrankRequestV16,
        effective_prices: &[u64; V16_MAX_PORTFOLIO_ASSETS_N],
    ) -> V16Result<PermissionlessProgressOutcomeV16> {
        self.validate_account_shape(account)?;
        if self.mode != MarketModeV16::Live
            && !matches!(request.action, PermissionlessCrankActionV16::Recover(_))
        {
            return Err(V16Error::LockActive);
        }
        let protective_progress = match request.action {
            PermissionlessCrankActionV16::Refresh => {
                let touches_accrued_asset = request.asset_index
                    < self.config.max_portfolio_assets as usize
                    && account.legs[request.asset_index].active;
                if let PermissionlessProgressOutcomeV16::AccountBChunk(out) = self
                    .settle_account_side_effects_not_atomic(
                        account,
                        self.config.public_b_chunk_atoms,
                    )?
                {
                    self.assert_public_invariants()?;
                    return Ok(PermissionlessProgressOutcomeV16::AccountBChunk(out));
                }
                self.full_account_refresh(account, effective_prices)?;
                touches_accrued_asset
            }
            PermissionlessCrankActionV16::SettleB { asset_index } => {
                let out = self.settle_account_b_chunk(
                    account,
                    asset_index,
                    self.config.public_b_chunk_atoms,
                )?;
                return Ok(PermissionlessProgressOutcomeV16::AccountBChunk(out));
            }
            PermissionlessCrankActionV16::Liquidate(liq) => {
                let liquidated_asset_index = liq.asset_index;
                self.liquidate_account_not_atomic(account, liq, effective_prices)?;
                liquidated_asset_index == request.asset_index
            }
            PermissionlessCrankActionV16::Recover(reason) => {
                return self.declare_permissionless_recovery(reason);
            }
        };
        self.accrue_asset_to_not_atomic(
            request.asset_index,
            request.now_slot,
            request.effective_price,
            request.funding_rate_e9,
            protective_progress,
        )?;
        Ok(PermissionlessProgressOutcomeV16::AccountCurrent)
    }

    pub fn resolve_market_not_atomic(&mut self, resolved_slot: u64) -> V16Result<()> {
        if self.mode == MarketModeV16::Recovery {
            return Err(V16Error::LockActive);
        }
        if resolved_slot < self.current_slot {
            return Err(V16Error::Stale);
        }
        self.mode = MarketModeV16::Resolved;
        self.resolved_slot = resolved_slot;
        self.current_slot = resolved_slot;
        self.loss_stale_active = false;
        self.assert_public_invariants()
    }

    pub fn close_resolved_account_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV16,
        fee_rate_per_slot: u128,
    ) -> V16Result<ResolvedCloseOutcomeV16> {
        if self.mode != MarketModeV16::Resolved {
            return Err(V16Error::LockActive);
        }
        if let PermissionlessProgressOutcomeV16::AccountBChunk(_) =
            self.settle_account_side_effects_not_atomic(account, self.config.public_b_chunk_atoms)?
        {
            self.assert_public_invariants()?;
            return Ok(ResolvedCloseOutcomeV16::ProgressOnly);
        }
        self.sync_account_fee_to_slot_not_atomic(account, self.resolved_slot, fee_rate_per_slot)?;
        self.settle_negative_pnl_from_principal(account)?;
        if account.pnl < 0 {
            self.declare_permissionless_recovery(
                PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress,
            )?;
            self.assert_public_invariants()?;
            return Ok(ResolvedCloseOutcomeV16::ProgressOnly);
        }
        if account.active_bitmap != 0
            || account.pnl < 0
            || account.b_stale_state
            || account.stale_state
        {
            return Ok(ResolvedCloseOutcomeV16::ProgressOnly);
        }
        if account.pnl > 0 && !self.resolved_positive_payout_ready() {
            return Ok(ResolvedCloseOutcomeV16::ProgressOnly);
        }
        let pnl_payout = if account.pnl > 0 || account.resolved_payout_receipt.present {
            self.create_resolved_payout_receipt_if_needed(account)?;
            let claimable = self.resolved_receipt_claimable_now(account.resolved_payout_receipt)?;
            account.resolved_payout_receipt.paid_effective = account
                .resolved_payout_receipt
                .paid_effective
                .checked_add(claimable)
                .ok_or(V16Error::ArithmeticOverflow)?;
            if account.resolved_payout_receipt.paid_effective
                == account.resolved_payout_receipt.terminal_positive_claim_face
            {
                account.resolved_payout_receipt.finalized = true;
            }
            claimable
        } else {
            0
        };
        let payout = account
            .capital
            .checked_add(pnl_payout)
            .ok_or(V16Error::ArithmeticOverflow)?
            .min(self.vault);
        let capital_paid = account.capital.min(payout);
        let resolved_paid = payout
            .checked_sub(capital_paid)
            .ok_or(V16Error::CounterUnderflow)?;
        let vault_before = self.vault;
        self.vault = self
            .vault
            .checked_sub(payout)
            .ok_or(V16Error::CounterUnderflow)?;
        self.c_tot = self.c_tot.saturating_sub(account.capital.min(self.c_tot));
        self.set_account_pnl(account, 0)?;
        account.capital = 0;
        account.reserved_pnl = 0;
        account.fee_credits = 0;
        account.health_cert.valid = false;
        TokenValueFlowProofV16::capital_and_resolved_payout_to_external_out(
            capital_paid,
            resolved_paid,
            payout,
            vault_before,
            self.vault,
        )?
        .validate()?;
        self.assert_public_invariants()?;
        Ok(ResolvedCloseOutcomeV16::Closed { payout })
    }

    pub fn claim_resolved_payout_topup_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV16,
    ) -> V16Result<u128> {
        self.validate_account_shape(account)?;
        if self.mode != MarketModeV16::Resolved || !self.payout_snapshot_captured {
            return Err(V16Error::LockActive);
        }
        let claimable = self.resolved_receipt_claimable_now(account.resolved_payout_receipt)?;
        if claimable == 0 {
            return Ok(0);
        }
        let payout = claimable.min(self.vault);
        account.resolved_payout_receipt.paid_effective = account
            .resolved_payout_receipt
            .paid_effective
            .checked_add(payout)
            .ok_or(V16Error::ArithmeticOverflow)?;
        if account.resolved_payout_receipt.paid_effective
            == account.resolved_payout_receipt.terminal_positive_claim_face
        {
            account.resolved_payout_receipt.finalized = true;
        }
        let vault_before = self.vault;
        self.vault = self
            .vault
            .checked_sub(payout)
            .ok_or(V16Error::CounterUnderflow)?;
        TokenValueFlowProofV16::capital_and_resolved_payout_to_external_out(
            0,
            payout,
            payout,
            vault_before,
            self.vault,
        )?
        .validate()?;
        self.assert_public_invariants()?;
        Ok(payout)
    }

    pub fn refine_resolved_unreceipted_bound_not_atomic(
        &mut self,
        decrease_num: u128,
    ) -> V16Result<()> {
        if self.mode != MarketModeV16::Resolved || !self.payout_snapshot_captured {
            return Err(V16Error::LockActive);
        }
        let old_num = self.resolved_payout_ledger.current_payout_rate_num;
        let old_den = self.resolved_payout_ledger.current_payout_rate_den;
        self.resolved_payout_ledger
            .terminal_claim_bound_unreceipted_num = self
            .resolved_payout_ledger
            .terminal_claim_bound_unreceipted_num
            .checked_sub(decrease_num)
            .ok_or(V16Error::CounterUnderflow)?;
        self.recompute_resolved_payout_rate()?;
        if !fraction_ge(
            self.resolved_payout_ledger.current_payout_rate_num,
            self.resolved_payout_ledger.current_payout_rate_den,
            old_num,
            old_den,
        )? {
            return Err(V16Error::InvalidConfig);
        }
        self.assert_public_invariants()
    }

    fn begin_close_progress_ledger(
        &mut self,
        account: &mut PortfolioAccountV16,
        asset_index: usize,
        domain_side: SideV16,
        gross_loss: u128,
    ) -> V16Result<()> {
        self.validate_account_shape(account)?;
        if gross_loss == 0 {
            return Ok(());
        }
        if account.close_progress.active {
            return Err(V16Error::LockActive);
        }
        let domain = self.insurance_domain_index(asset_index, domain_side)?;
        if self.pending_domain_loss_barriers[domain] != 0 {
            return Err(V16Error::LockActive);
        }
        let close_id = account.close_progress.close_id.saturating_add(1).max(1);
        let ledger = CloseProgressLedgerV16 {
            active: true,
            finalized: false,
            close_id,
            asset_index: u8::try_from(asset_index).map_err(|_| V16Error::InvalidLeg)?,
            domain_side,
            gross_loss_at_close_start: gross_loss,
            drift_reference_slot: self.current_slot,
            max_close_slot: self
                .current_slot
                .checked_add(self.config.max_bankrupt_close_lifetime_slots)
                .ok_or(V16Error::ArithmeticOverflow)?,
            residual_remaining: gross_loss,
            ..CloseProgressLedgerV16::EMPTY
        };
        self.validate_close_progress_ledger(ledger)?;
        self.pending_domain_loss_barriers[domain] = self.pending_domain_loss_barriers[domain]
            .checked_add(1)
            .ok_or(V16Error::CounterOverflow)?;
        account.close_progress = ledger;
        Ok(())
    }

    fn advance_close_progress_ledger(
        &mut self,
        account: &mut PortfolioAccountV16,
        support_consumed: u128,
        junior_face_burned: u128,
        insurance_spent: u128,
        b_loss_booked: u128,
        explicit_loss_assigned: u128,
    ) -> V16Result<()> {
        if support_consumed == 0
            && junior_face_burned == 0
            && insurance_spent == 0
            && b_loss_booked == 0
            && explicit_loss_assigned == 0
        {
            return Ok(());
        }
        let mut ledger = account.close_progress;
        self.ensure_close_progress_not_expired(ledger)?;
        let was_pending = ledger.has_pending_residual();
        let domain =
            self.insurance_domain_index(ledger.asset_index as usize, ledger.domain_side)?;
        if !ledger.active || ledger.finalized {
            return Err(V16Error::LockActive);
        }
        ledger.support_consumed = ledger
            .support_consumed
            .checked_add(support_consumed)
            .ok_or(V16Error::ArithmeticOverflow)?;
        ledger.junior_face_burned = ledger
            .junior_face_burned
            .checked_add(junior_face_burned)
            .ok_or(V16Error::ArithmeticOverflow)?;
        ledger.insurance_spent = ledger
            .insurance_spent
            .checked_add(insurance_spent)
            .ok_or(V16Error::ArithmeticOverflow)?;
        ledger.b_loss_booked = ledger
            .b_loss_booked
            .checked_add(b_loss_booked)
            .ok_or(V16Error::ArithmeticOverflow)?;
        ledger.explicit_loss_assigned = ledger
            .explicit_loss_assigned
            .checked_add(explicit_loss_assigned)
            .ok_or(V16Error::ArithmeticOverflow)?;
        let total_loss = ledger
            .gross_loss_at_close_start
            .checked_add(ledger.drift_consumed)
            .ok_or(V16Error::ArithmeticOverflow)?;
        let progress = ledger
            .support_consumed
            .checked_add(ledger.insurance_spent)
            .and_then(|v| v.checked_add(ledger.b_loss_booked))
            .and_then(|v| v.checked_add(ledger.explicit_loss_assigned))
            .ok_or(V16Error::ArithmeticOverflow)?;
        if progress > total_loss {
            return Err(V16Error::ArithmeticOverflow);
        }
        ledger.residual_remaining = total_loss - progress;
        if ledger.residual_remaining == 0 {
            ledger.finalized = true;
        }
        self.validate_close_progress_ledger(ledger)?;
        if was_pending && !ledger.has_pending_residual() {
            self.pending_domain_loss_barriers[domain] = self.pending_domain_loss_barriers[domain]
                .checked_sub(1)
                .ok_or(V16Error::CounterUnderflow)?;
        }
        account.close_progress = ledger;
        account.health_cert.valid = false;
        Ok(())
    }

    fn advance_close_progress_quantity_adl(
        &mut self,
        account: &mut PortfolioAccountV16,
        quantity_adl_applied_q: u128,
    ) -> V16Result<()> {
        if quantity_adl_applied_q == 0 {
            return Err(V16Error::NonProgress);
        }
        let mut ledger = account.close_progress;
        self.ensure_close_progress_not_expired(ledger)?;
        if !ledger.active || !ledger.finalized || ledger.residual_remaining != 0 {
            return Err(V16Error::LockActive);
        }
        if ledger.quantity_adl_applied_q != 0 {
            return Err(V16Error::LockActive);
        }
        ledger.quantity_adl_applied_q = quantity_adl_applied_q;
        self.validate_close_progress_ledger(ledger)?;
        account.close_progress = ledger;
        account.health_cert.valid = false;
        Ok(())
    }

    pub fn book_bankruptcy_residual_chunk_for_account(
        &mut self,
        account: &mut PortfolioAccountV16,
        asset_index: usize,
        bankrupt_side: SideV16,
        residual_remaining: u128,
    ) -> V16Result<BResidualBookingOutcomeV16> {
        self.validate_account_shape(account)?;
        if residual_remaining == 0 {
            return Ok(BResidualBookingOutcomeV16 {
                booked_loss: 0,
                explicit_loss: 0,
                delta_b: 0,
                remaining_after: 0,
            });
        }
        let domain_side = opposite_side(bankrupt_side);
        if !account.close_progress.active {
            if self.bankruptcy_residual_single_step_capacity(
                asset_index,
                bankrupt_side,
                residual_remaining,
            )? == 0
            {
                self.declare_permissionless_recovery(
                    PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress,
                )?;
                return Err(V16Error::RecoveryRequired);
            }
            self.begin_close_progress_ledger(
                account,
                asset_index,
                domain_side,
                residual_remaining,
            )?;
        }
        self.ensure_close_progress_not_expired(account.close_progress)?;
        let ledger = account.close_progress;
        if ledger.asset_index as usize != asset_index || ledger.domain_side != domain_side {
            return Err(V16Error::LockActive);
        }
        self.ensure_open_close_snapshot_current_or_recovery(account, ledger)?;
        let residual_to_book = ledger.residual_remaining;
        let outcome = self.book_bankruptcy_residual_chunk_internal(
            asset_index,
            bankrupt_side,
            residual_to_book,
        )?;
        self.advance_close_progress_ledger(
            account,
            0,
            0,
            0,
            outcome.booked_loss,
            outcome.explicit_loss,
        )?;
        Ok(outcome)
    }

    fn book_bankruptcy_residual_chunk_internal(
        &mut self,
        asset_index: usize,
        bankrupt_side: SideV16,
        residual_remaining: u128,
    ) -> V16Result<BResidualBookingOutcomeV16> {
        if asset_index >= self.config.max_portfolio_assets as usize {
            return Err(V16Error::InvalidLeg);
        }
        if residual_remaining == 0 {
            return Ok(BResidualBookingOutcomeV16 {
                booked_loss: 0,
                explicit_loss: 0,
                delta_b: 0,
                remaining_after: 0,
            });
        }
        let opp = opposite_side(bankrupt_side);
        let asset = self.assets[asset_index];
        let (b_now, weight_sum, rem) = match opp {
            SideV16::Long => (
                asset.b_long_num,
                asset.loss_weight_sum_long,
                asset.social_loss_remainder_long_num,
            ),
            SideV16::Short => (
                asset.b_short_num,
                asset.loss_weight_sum_short,
                asset.social_loss_remainder_short_num,
            ),
        };
        if weight_sum == 0 {
            self.declare_permissionless_recovery(
                PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress,
            )?;
            return Err(V16Error::RecoveryRequired);
        }
        let engine_chunk = self.bankruptcy_residual_single_step_capacity(
            asset_index,
            bankrupt_side,
            residual_remaining,
        )?;
        if engine_chunk == 0 {
            self.declare_permissionless_recovery(
                PermissionlessRecoveryReasonV16::BIndexHeadroomExhausted,
            )?;
            return Err(V16Error::RecoveryRequired);
        }
        let numerator = engine_chunk
            .checked_mul(SOCIAL_LOSS_DEN)
            .and_then(|v| v.checked_add(rem))
            .ok_or(V16Error::ArithmeticOverflow)?;
        let delta_b = numerator / weight_sum;
        let new_rem = numerator % weight_sum;
        if delta_b == 0 || b_now.checked_add(delta_b).is_none() {
            self.declare_permissionless_recovery(
                PermissionlessRecoveryReasonV16::BIndexHeadroomExhausted,
            )?;
            return Err(V16Error::RecoveryRequired);
        }
        let asset = &mut self.assets[asset_index];
        match opp {
            SideV16::Long => {
                asset.b_long_num = asset
                    .b_long_num
                    .checked_add(delta_b)
                    .ok_or(V16Error::ArithmeticOverflow)?;
                asset.social_loss_remainder_long_num = new_rem;
            }
            SideV16::Short => {
                asset.b_short_num = asset
                    .b_short_num
                    .checked_add(delta_b)
                    .ok_or(V16Error::ArithmeticOverflow)?;
                asset.social_loss_remainder_short_num = new_rem;
            }
        }
        self.bankruptcy_hlock_active = true;
        Ok(BResidualBookingOutcomeV16 {
            booked_loss: engine_chunk,
            explicit_loss: 0,
            delta_b,
            remaining_after: residual_remaining - engine_chunk,
        })
    }

    pub fn apply_quantity_adl_after_residual_for_account_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV16,
        asset_index: usize,
        bankrupt_side: SideV16,
        close_q: u128,
    ) -> V16Result<QuantityAdlOutcomeV16> {
        self.validate_account_shape(account)?;
        let ledger = account.close_progress;
        let leg = if asset_index < self.config.max_portfolio_assets as usize {
            account.legs[asset_index]
        } else {
            return Err(V16Error::InvalidLeg);
        };
        if !ledger.active
            || !ledger.finalized
            || ledger.residual_remaining != 0
            || ledger.asset_index as usize != asset_index
            || ledger.domain_side != opposite_side(bankrupt_side)
        {
            return Err(V16Error::LockActive);
        }
        if !leg.active
            || leg.stale
            || leg.b_stale
            || leg.side != bankrupt_side
            || close_q != leg.basis_pos_q.unsigned_abs()
        {
            return Err(V16Error::InvalidLeg);
        }
        self.ensure_close_progress_not_expired(ledger)?;
        self.ensure_open_close_snapshot_current_or_recovery(account, ledger)?;
        let out =
            self.apply_quantity_adl_after_residual_internal(asset_index, bankrupt_side, close_q)?;
        self.advance_close_progress_quantity_adl(account, out.closed_q)?;
        self.clear_leg_after_quantity_adl(account, asset_index, leg)?;
        self.assert_public_invariants()?;
        Ok(out)
    }

    fn clear_leg_after_quantity_adl(
        &mut self,
        account: &mut PortfolioAccountV16,
        asset_index: usize,
        leg: PortfolioLegV16,
    ) -> V16Result<()> {
        if asset_index >= self.config.max_portfolio_assets as usize
            || !leg.active
            || leg.stale
            || leg.b_stale
            || account.legs[asset_index] != leg
        {
            return Err(V16Error::InvalidLeg);
        }

        let asset = &mut self.assets[asset_index];
        let prior_reset_epoch = match leg.side {
            SideV16::Long => {
                asset.mode_long == SideModeV16::ResetPending
                    && leg.epoch_snap.checked_add(1) == Some(asset.epoch_long)
            }
            SideV16::Short => {
                asset.mode_short == SideModeV16::ResetPending
                    && leg.epoch_snap.checked_add(1) == Some(asset.epoch_short)
            }
        };
        match leg.side {
            SideV16::Long => {
                asset.stored_pos_count_long = asset
                    .stored_pos_count_long
                    .checked_sub(1)
                    .ok_or(V16Error::CounterUnderflow)?;
                if !prior_reset_epoch {
                    asset.loss_weight_sum_long = asset
                        .loss_weight_sum_long
                        .checked_sub(leg.loss_weight)
                        .ok_or(V16Error::CounterUnderflow)?;
                }
            }
            SideV16::Short => {
                asset.stored_pos_count_short = asset
                    .stored_pos_count_short
                    .checked_sub(1)
                    .ok_or(V16Error::CounterUnderflow)?;
                if !prior_reset_epoch {
                    asset.loss_weight_sum_short = asset
                        .loss_weight_sum_short
                        .checked_sub(leg.loss_weight)
                        .ok_or(V16Error::CounterUnderflow)?;
                }
            }
        }
        account.legs[asset_index] = PortfolioLegV16::EMPTY;
        account.active_bitmap &= !(1u32 << asset_index);
        account.health_cert.valid = false;
        self.validate_account_shape(account)
    }

    fn ensure_close_progress_not_expired(
        &mut self,
        ledger: CloseProgressLedgerV16,
    ) -> V16Result<()> {
        if ledger.active && self.current_slot > ledger.max_close_slot {
            self.declare_permissionless_recovery(
                PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress,
            )?;
            return Err(V16Error::RecoveryRequired);
        }
        Ok(())
    }

    fn ensure_open_close_snapshot_current_or_recovery(
        &mut self,
        account: &PortfolioAccountV16,
        ledger: CloseProgressLedgerV16,
    ) -> V16Result<()> {
        if !ledger.active {
            return Ok(());
        }
        let asset_index = ledger.asset_index as usize;
        if asset_index < self.config.max_portfolio_assets as usize
            && account.legs[asset_index].active
            && self.current_slot > ledger.drift_reference_slot
        {
            self.declare_permissionless_recovery(
                PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress,
            )?;
            return Err(V16Error::RecoveryRequired);
        }
        Ok(())
    }

    fn apply_quantity_adl_after_residual_internal(
        &mut self,
        asset_index: usize,
        bankrupt_side: SideV16,
        close_q: u128,
    ) -> V16Result<QuantityAdlOutcomeV16> {
        if asset_index >= self.config.max_portfolio_assets as usize || close_q == 0 {
            return Err(V16Error::InvalidLeg);
        }
        let opp = opposite_side(bankrupt_side);
        let asset = self.assets[asset_index];
        let (liq_oi_before, opp_oi_before, opp_a_before) = match (bankrupt_side, opp) {
            (SideV16::Long, SideV16::Short) => {
                (asset.oi_eff_long_q, asset.oi_eff_short_q, asset.a_short)
            }
            (SideV16::Short, SideV16::Long) => {
                (asset.oi_eff_short_q, asset.oi_eff_long_q, asset.a_long)
            }
            _ => unreachable!(),
        };
        if close_q > liq_oi_before || close_q > opp_oi_before {
            return Err(V16Error::InvalidLeg);
        }
        let liq_oi_after = liq_oi_before - close_q;
        let opp_oi_after = opp_oi_before - close_q;
        let mut reset_started = false;
        let mut opposite_a_after = if opp_oi_after == 0 {
            ADL_ONE
        } else {
            wide_mul_div_floor_u128(opp_a_before, opp_oi_after, opp_oi_before)
        };

        let force_full_reset = opp_oi_after != 0 && opposite_a_after == 0;
        let final_liq_oi_after = if force_full_reset { 0 } else { liq_oi_after };
        let final_opp_oi_after = if force_full_reset { 0 } else { opp_oi_after };
        if force_full_reset {
            opposite_a_after = ADL_ONE;
        }

        {
            let asset = &mut self.assets[asset_index];
            match bankrupt_side {
                SideV16::Long => asset.oi_eff_long_q = final_liq_oi_after,
                SideV16::Short => asset.oi_eff_short_q = final_liq_oi_after,
            }
            match opp {
                SideV16::Long => {
                    asset.oi_eff_long_q = final_opp_oi_after;
                    asset.a_long =
                        opposite_a_after.max(if final_opp_oi_after == 0 { ADL_ONE } else { 1 });
                    if final_opp_oi_after != 0 && asset.a_long < MIN_A_SIDE {
                        asset.mode_long = SideModeV16::DrainOnly;
                    }
                }
                SideV16::Short => {
                    asset.oi_eff_short_q = final_opp_oi_after;
                    asset.a_short =
                        opposite_a_after.max(if final_opp_oi_after == 0 { ADL_ONE } else { 1 });
                    if final_opp_oi_after != 0 && asset.a_short < MIN_A_SIDE {
                        asset.mode_short = SideModeV16::DrainOnly;
                    }
                }
            }
        }

        if final_liq_oi_after == 0 {
            self.begin_full_drain_reset_inner(asset_index, bankrupt_side)?;
            reset_started = true;
        }
        if final_opp_oi_after == 0 {
            self.begin_full_drain_reset_inner(asset_index, opp)?;
            reset_started = true;
        }
        self.assert_public_invariants()?;
        Ok(QuantityAdlOutcomeV16 {
            closed_q: close_q,
            opposite_a_after,
            reset_started,
        })
    }

    pub fn begin_full_drain_reset(&mut self, asset_index: usize, side: SideV16) -> V16Result<()> {
        self.begin_full_drain_reset_inner(asset_index, side)?;
        self.assert_public_invariants()
    }

    fn begin_full_drain_reset_inner(&mut self, asset_index: usize, side: SideV16) -> V16Result<()> {
        if asset_index >= self.config.max_portfolio_assets as usize {
            return Err(V16Error::LockActive);
        }
        if self.has_pending_domain_loss_barrier(asset_index, side)? {
            return Err(V16Error::LockActive);
        }
        let asset = &mut self.assets[asset_index];
        match side {
            SideV16::Long => {
                if asset.mode_long == SideModeV16::ResetPending {
                    return Err(V16Error::LockActive);
                }
                if asset.oi_eff_long_q != 0 {
                    return Err(V16Error::InvalidLeg);
                }
                if asset.pending_obligation_count_long != 0 {
                    return Err(V16Error::LockActive);
                }
                quarantine_remainder(
                    &mut asset.social_loss_remainder_long_num,
                    &mut asset.social_loss_dust_long_num,
                )?;
                asset.k_epoch_start_long = asset.k_long;
                asset.f_epoch_start_long_num = asset.f_long_num;
                asset.b_epoch_start_long_num = asset.b_long_num;
                asset.k_long = 0;
                asset.f_long_num = 0;
                asset.b_long_num = 0;
                asset.loss_weight_sum_long = 0;
                asset.a_long = ADL_ONE;
                asset.epoch_long = asset
                    .epoch_long
                    .checked_add(1)
                    .ok_or(V16Error::CounterOverflow)?;
                asset.mode_long = SideModeV16::ResetPending;
            }
            SideV16::Short => {
                if asset.mode_short == SideModeV16::ResetPending {
                    return Err(V16Error::LockActive);
                }
                if asset.oi_eff_short_q != 0 {
                    return Err(V16Error::InvalidLeg);
                }
                if asset.pending_obligation_count_short != 0 {
                    return Err(V16Error::LockActive);
                }
                quarantine_remainder(
                    &mut asset.social_loss_remainder_short_num,
                    &mut asset.social_loss_dust_short_num,
                )?;
                asset.k_epoch_start_short = asset.k_short;
                asset.f_epoch_start_short_num = asset.f_short_num;
                asset.b_epoch_start_short_num = asset.b_short_num;
                asset.k_short = 0;
                asset.f_short_num = 0;
                asset.b_short_num = 0;
                asset.loss_weight_sum_short = 0;
                asset.a_short = ADL_ONE;
                asset.epoch_short = asset
                    .epoch_short
                    .checked_add(1)
                    .ok_or(V16Error::CounterOverflow)?;
                asset.mode_short = SideModeV16::ResetPending;
            }
        }
        self.risk_epoch = self
            .risk_epoch
            .checked_add(1)
            .ok_or(V16Error::CounterOverflow)?;
        Ok(())
    }

    pub fn finalize_ready_reset_side(
        &mut self,
        asset_index: usize,
        side: SideV16,
    ) -> V16Result<()> {
        if asset_index >= self.config.max_portfolio_assets as usize {
            return Err(V16Error::InvalidLeg);
        }
        let asset = &mut self.assets[asset_index];
        match side {
            SideV16::Long => {
                if asset.mode_long != SideModeV16::ResetPending {
                    return Ok(());
                }
                if asset.stored_pos_count_long != 0 || asset.stale_account_count_long != 0 {
                    return Err(V16Error::Stale);
                }
                asset.mode_long = SideModeV16::Normal;
            }
            SideV16::Short => {
                if asset.mode_short != SideModeV16::ResetPending {
                    return Ok(());
                }
                if asset.stored_pos_count_short != 0 || asset.stale_account_count_short != 0 {
                    return Err(V16Error::Stale);
                }
                asset.mode_short = SideModeV16::Normal;
            }
        }
        self.assert_public_invariants()
    }

    pub fn risk_score(&self, account: &PortfolioAccountV16) -> V16Result<RiskScoreV16> {
        self.validate_account_shape(account)?;
        self.risk_score_unchecked(account)
    }

    fn risk_score_unchecked(&self, account: &PortfolioAccountV16) -> V16Result<RiskScoreV16> {
        if !account.health_cert.valid {
            return Err(V16Error::Stale);
        }
        Ok(RiskScoreV16 {
            certified_liq_deficit: account.health_cert.certified_liq_deficit,
            unsettled_b_loss_bound: account_b_loss_bound(account)?,
            stale_loss_bound: if account.stale_state { 1 } else { 0 },
            gross_risk_notional: account.health_cert.certified_worst_case_loss,
            active_leg_count: account.active_bitmap.count_ones(),
        })
    }

    pub fn validate_liquidation_progress(
        &self,
        before: &PortfolioAccountV16,
        after: &PortfolioAccountV16,
    ) -> V16Result<()> {
        self.validate_liquidation_progress_from_score(self.risk_score(before)?, after)
    }

    #[inline(never)]
    fn validate_liquidation_progress_from_score(
        &self,
        before_score: RiskScoreV16,
        after: &PortfolioAccountV16,
    ) -> V16Result<()> {
        let after_score = self.risk_score_unchecked(after)?;
        if after_score.strictly_reduces_from(before_score)
            || after_score.certified_liq_deficit < before_score.certified_liq_deficit
        {
            Ok(())
        } else {
            Err(V16Error::NonProgress)
        }
    }

    pub fn declare_permissionless_recovery(
        &mut self,
        reason: PermissionlessRecoveryReasonV16,
    ) -> V16Result<PermissionlessProgressOutcomeV16> {
        if !self.config.permissionless_recovery_enabled {
            return Err(V16Error::InvalidConfig);
        }
        if self.mode == MarketModeV16::Resolved {
            return Err(V16Error::LockActive);
        }
        if let Some(existing_reason) = self.recovery_reason {
            return Ok(PermissionlessProgressOutcomeV16::RecoveryDeclared(
                existing_reason,
            ));
        }
        self.mode = MarketModeV16::Recovery;
        self.recovery_reason = Some(reason);
        Ok(PermissionlessProgressOutcomeV16::RecoveryDeclared(reason))
    }

    pub fn declare_explicit_loss_or_dust_audit_overflow_not_atomic(
        &mut self,
    ) -> V16Result<PermissionlessProgressOutcomeV16> {
        self.declare_permissionless_recovery(
            PermissionlessRecoveryReasonV16::ExplicitLossOrDustAuditOverflow,
        )
    }

    pub fn stock_reconciliation_proof(&self) -> V16Result<StockReconciliationProofV16> {
        let senior = self
            .c_tot
            .checked_add(self.insurance)
            .ok_or(V16Error::ArithmeticOverflow)?;
        if senior > self.vault {
            return Err(V16Error::InvalidConfig);
        }
        Ok(StockReconciliationProofV16 {
            token_vault: self.vault,
            senior_capital_total: self.c_tot,
            insurance_capital: self.insurance,
            settlement_rounding_residue_total: 0,
            unallocated_protocol_surplus: self.vault - senior,
        })
    }

    pub fn assert_public_invariants(&self) -> V16Result<()> {
        if self.vault > MAX_VAULT_TVL {
            return Err(V16Error::InvalidConfig);
        }
        self.validate_resolved_payout_ledger()?;
        let senior = self
            .c_tot
            .checked_add(self.insurance)
            .ok_or(V16Error::ArithmeticOverflow)?;
        if self.c_tot > self.vault || self.insurance > self.vault || senior > self.vault {
            return Err(V16Error::InvalidConfig);
        }
        self.stock_reconciliation_proof()?.validate()?;
        if self.pnl_matured_pos_tot > self.pnl_pos_tot {
            return Err(V16Error::InvalidConfig);
        }
        let derived_bound = Self::amount_from_bound_num(self.pnl_pos_bound_tot_num)?;
        if self.pnl_pos_bound_tot < self.pnl_pos_tot {
            return Err(V16Error::InvalidConfig);
        }
        if self.pnl_pos_bound_tot != derived_bound {
            return Err(V16Error::InvalidConfig);
        }
        let exact_bound_num = Self::bound_num_from_amount(self.pnl_pos_tot)?;
        if self.pnl_pos_bound_tot_num < exact_bound_num {
            return Err(V16Error::InvalidConfig);
        }
        if self.slot_last > self.current_slot {
            return Err(V16Error::InvalidConfig);
        }
        let mut live_source_credit_insurance_atoms = 0u128;
        if self.asset_activation_count == 0 {
            if self.last_asset_activation_slot != 0 {
                return Err(V16Error::InvalidConfig);
            }
        } else if self.last_asset_activation_slot > self.current_slot {
            return Err(V16Error::InvalidConfig);
        }
        let mut d = 0;
        while d < V16_DOMAIN_COUNT {
            if self.insurance_domain_spent[d] > self.insurance_domain_budget[d] {
                return Err(V16Error::InvalidConfig);
            }
            if self.pending_domain_loss_barriers[d] > 1 {
                return Err(V16Error::InvalidConfig);
            }
            if d >= self.config.max_portfolio_assets as usize * 2
                && self.pending_domain_loss_barriers[d] != 0
            {
                return Err(V16Error::InvalidConfig);
            }
            if d >= self.config.max_portfolio_assets as usize * 2 {
                if self.source_credit[d] != SourceCreditStateV16::EMPTY
                    || self.source_backing_buckets[d] != BackingBucketV16::EMPTY
                    || self.insurance_credit_reservations[d] != InsuranceCreditReservationV16::EMPTY
                {
                    return Err(V16Error::InvalidConfig);
                }
            } else {
                self.validate_source_domain_ledger(d)?;
                let reserved_atoms = Self::amount_from_bound_num(
                    self.source_credit[d].insurance_credit_reserved_num,
                )?;
                live_source_credit_insurance_atoms = live_source_credit_insurance_atoms
                    .checked_add(reserved_atoms)
                    .ok_or(V16Error::ArithmeticOverflow)?;
                if self.insurance_domain_spent[d]
                    .checked_add(reserved_atoms)
                    .ok_or(V16Error::ArithmeticOverflow)?
                    > self.insurance_domain_budget[d]
                {
                    return Err(V16Error::InvalidConfig);
                }
            }
            d += 1;
        }
        if live_source_credit_insurance_atoms > self.insurance {
            return Err(V16Error::InvalidConfig);
        }
        for i in 0..self.config.max_portfolio_assets as usize {
            let asset = self.assets[i];
            let requires_price = matches!(
                asset.lifecycle,
                AssetLifecycleV16::Active
                    | AssetLifecycleV16::DrainOnly
                    | AssetLifecycleV16::Recovery
            );
            if (requires_price
                && (asset.effective_price == 0
                    || asset.effective_price > MAX_ORACLE_PRICE
                    || asset.raw_oracle_target_price == 0
                    || asset.raw_oracle_target_price > MAX_ORACLE_PRICE
                    || asset.fund_px_last == 0
                    || asset.fund_px_last > MAX_ORACLE_PRICE))
                || asset.slot_last > self.current_slot
                || asset.k_long == i128::MIN
                || asset.k_short == i128::MIN
                || asset.f_long_num == i128::MIN
                || asset.f_short_num == i128::MIN
                || asset.k_epoch_start_long == i128::MIN
                || asset.k_epoch_start_short == i128::MIN
                || asset.f_epoch_start_long_num == i128::MIN
                || asset.f_epoch_start_short_num == i128::MIN
                || asset.oi_eff_long_q > crate::MAX_OI_SIDE_Q
                || asset.oi_eff_short_q > crate::MAX_OI_SIDE_Q
                || (self.mode == MarketModeV16::Live && asset.oi_eff_long_q != asset.oi_eff_short_q)
                || asset.loss_weight_sum_long > SOCIAL_LOSS_DEN
                || asset.loss_weight_sum_short > SOCIAL_LOSS_DEN
                || (asset.oi_eff_long_q != 0 && asset.loss_weight_sum_long == 0)
                || (asset.oi_eff_short_q != 0 && asset.loss_weight_sum_short == 0)
                || (asset.loss_weight_sum_long != 0 && asset.stored_pos_count_long == 0)
                || (asset.loss_weight_sum_short != 0 && asset.stored_pos_count_short == 0)
                || asset.pending_obligation_count_long > asset.stored_pos_count_long
                || asset.pending_obligation_count_short > asset.stored_pos_count_short
                || (asset.pending_obligation_count_long != 0 && asset.loss_weight_sum_long == 0)
                || (asset.pending_obligation_count_short != 0 && asset.loss_weight_sum_short == 0)
                || asset.social_loss_remainder_long_num >= SOCIAL_LOSS_DEN
                || asset.social_loss_remainder_short_num >= SOCIAL_LOSS_DEN
                || asset.social_loss_dust_long_num >= SOCIAL_LOSS_DEN
                || asset.social_loss_dust_short_num >= SOCIAL_LOSS_DEN
            {
                return Err(V16Error::InvalidConfig);
            }
            if matches!(
                asset.lifecycle,
                AssetLifecycleV16::Disabled
                    | AssetLifecycleV16::PendingActivation
                    | AssetLifecycleV16::Retired
            ) {
                self.require_empty_asset_lifecycle_state(i)?;
            }
        }
        Ok(())
    }

    pub fn source_credit_available_backing_num(&self, domain: usize) -> V16Result<u128> {
        self.validate_source_domain_index(domain)?;
        Self::available_backing_num_for_source_credit_state(self.source_credit[domain])
    }

    pub fn recompute_source_credit_rate_not_atomic(&mut self, domain: usize) -> V16Result<u128> {
        self.validate_source_domain_index(domain)?;
        let rate = Self::expected_source_credit_rate_num_for_state(self.source_credit[domain])?;
        self.source_credit[domain].credit_rate_num = rate;
        self.source_credit[domain].credit_epoch = self.source_credit[domain]
            .credit_epoch
            .checked_add(1)
            .ok_or(V16Error::CounterOverflow)?;
        self.risk_epoch = self
            .risk_epoch
            .checked_add(1)
            .ok_or(V16Error::CounterOverflow)?;
        self.assert_public_invariants()?;
        Ok(rate)
    }

    pub fn add_source_positive_claim_bound_not_atomic(
        &mut self,
        domain: usize,
        claim_bound_num: u128,
        exact_claim_num: u128,
    ) -> V16Result<()> {
        self.validate_source_domain_index(domain)?;
        if exact_claim_num > claim_bound_num {
            return Err(V16Error::InvalidConfig);
        }
        self.source_credit[domain].positive_claim_bound_num = self.source_credit[domain]
            .positive_claim_bound_num
            .checked_add(claim_bound_num)
            .ok_or(V16Error::CounterOverflow)?;
        self.source_credit[domain].exact_positive_claim_num = self.source_credit[domain]
            .exact_positive_claim_num
            .checked_add(exact_claim_num)
            .ok_or(V16Error::CounterOverflow)?;
        self.refresh_source_credit_domain_after_mutation(domain)
    }

    pub fn add_account_source_positive_pnl_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV16,
        domain: usize,
        amount: u128,
    ) -> V16Result<()> {
        self.validate_account_shape(account)?;
        self.validate_source_domain_index(domain)?;
        if amount == 0 {
            return Ok(());
        }
        let delta = i128::try_from(amount).map_err(|_| V16Error::ArithmeticOverflow)?;
        let new_pnl = account
            .pnl
            .checked_add(delta)
            .ok_or(V16Error::ArithmeticOverflow)?;
        self.set_account_pnl_with_source(account, new_pnl, domain)?;
        account.health_cert.valid = false;
        self.assert_public_invariants()
    }

    pub fn add_fresh_counterparty_backing_not_atomic(
        &mut self,
        domain: usize,
        amount: u128,
        expiry_slot: u64,
    ) -> V16Result<()> {
        self.add_fresh_counterparty_backing_unchecked(domain, amount, expiry_slot)?;
        self.reservation_encumbrance_proof_for_domain(domain)?
            .validate()?;
        self.assert_public_invariants()
    }

    fn add_fresh_counterparty_backing_unchecked(
        &mut self,
        domain: usize,
        amount: u128,
        expiry_slot: u64,
    ) -> V16Result<()> {
        self.validate_source_domain_index(domain)?;
        if amount == 0 || expiry_slot <= self.current_slot {
            return Err(V16Error::InvalidConfig);
        }
        let bucket = &mut self.source_backing_buckets[domain];
        match bucket.status {
            BackingBucketStatusV16::Empty => {
                bucket.status = BackingBucketStatusV16::Fresh;
                bucket.expiry_slot = expiry_slot;
            }
            BackingBucketStatusV16::Fresh if bucket.expiry_slot == expiry_slot => {}
            _ => return Err(V16Error::LockActive),
        }
        bucket.fresh_unliened_backing_num = bucket
            .fresh_unliened_backing_num
            .checked_add(amount)
            .ok_or(V16Error::CounterOverflow)?;
        self.source_credit[domain].fresh_reserved_backing_num = self.source_credit[domain]
            .fresh_reserved_backing_num
            .checked_add(amount)
            .ok_or(V16Error::CounterOverflow)?;
        self.recompute_source_credit_domain_after_mutation(domain)
    }

    fn fresh_counterparty_backing_expiry_slot(&self, domain: usize) -> V16Result<u64> {
        self.validate_source_domain_index(domain)?;
        let bucket = self.source_backing_buckets[domain];
        if bucket.status == BackingBucketStatusV16::Fresh && bucket.expiry_slot > self.current_slot
        {
            return Ok(bucket.expiry_slot);
        }
        let freshness_horizon = self
            .config
            .max_accrual_dt_slots
            .max(self.config.h_max)
            .max(self.config.max_bankrupt_close_lifetime_slots)
            .max(1);
        self.current_slot
            .checked_add(freshness_horizon)
            .ok_or(V16Error::CounterOverflow)
    }

    fn reserve_new_capital_backed_loss_for_source_domain_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV16,
        domain: usize,
        negative_before: u128,
        negative_after: u128,
    ) -> V16Result<()> {
        self.validate_source_domain_index(domain)?;
        let new_negative_loss = negative_after.saturating_sub(negative_before);
        if new_negative_loss == 0 {
            return Ok(());
        }
        let capital_not_already_encumbered = account.capital.saturating_sub(negative_before);
        let backing = new_negative_loss.min(capital_not_already_encumbered);
        if backing == 0 {
            return Ok(());
        }
        let backing_num = backing
            .checked_mul(BOUND_SCALE)
            .ok_or(V16Error::ArithmeticOverflow)?;
        let vault_before = self.vault;
        account.capital = account
            .capital
            .checked_sub(backing)
            .ok_or(V16Error::CounterUnderflow)?;
        self.c_tot = self
            .c_tot
            .checked_sub(backing)
            .ok_or(V16Error::CounterUnderflow)?;
        let backing_i128 = i128::try_from(backing).map_err(|_| V16Error::ArithmeticOverflow)?;
        let new_pnl = account
            .pnl
            .checked_add(backing_i128)
            .ok_or(V16Error::ArithmeticOverflow)?;
        self.set_account_pnl(account, new_pnl)?;
        TokenValueFlowProofV16::account_capital_to_realized_loss(
            backing,
            vault_before,
            self.vault,
        )?
        .validate()?;
        let expiry_slot = self.fresh_counterparty_backing_expiry_slot(domain)?;
        self.add_fresh_counterparty_backing_unchecked(domain, backing_num, expiry_slot)?;
        account.health_cert.valid = false;
        Ok(())
    }

    pub fn create_source_credit_lien_from_counterparty_not_atomic(
        &mut self,
        domain: usize,
        amount: u128,
    ) -> V16Result<()> {
        self.validate_source_domain_index(domain)?;
        if amount == 0 {
            return Ok(());
        }
        let bucket = &mut self.source_backing_buckets[domain];
        if bucket.status != BackingBucketStatusV16::Fresh
            || bucket.expiry_slot <= self.current_slot
            || bucket.fresh_unliened_backing_num < amount
        {
            return Err(V16Error::LockActive);
        }
        bucket.fresh_unliened_backing_num -= amount;
        bucket.valid_liened_backing_num = bucket
            .valid_liened_backing_num
            .checked_add(amount)
            .ok_or(V16Error::CounterOverflow)?;
        self.source_credit[domain].valid_liened_backing_num = self.source_credit[domain]
            .valid_liened_backing_num
            .checked_add(amount)
            .ok_or(V16Error::CounterOverflow)?;
        self.refresh_source_credit_domain_after_mutation(domain)
    }

    pub fn release_source_credit_lien_from_counterparty_not_atomic(
        &mut self,
        domain: usize,
        amount: u128,
    ) -> V16Result<()> {
        self.validate_source_domain_index(domain)?;
        if amount == 0 {
            return Ok(());
        }
        let bucket = &mut self.source_backing_buckets[domain];
        if bucket.status != BackingBucketStatusV16::Fresh
            || bucket.expiry_slot <= self.current_slot
            || bucket.valid_liened_backing_num < amount
            || self.source_credit[domain].valid_liened_backing_num < amount
        {
            return Err(V16Error::CounterUnderflow);
        }
        bucket.valid_liened_backing_num -= amount;
        bucket.fresh_unliened_backing_num = bucket
            .fresh_unliened_backing_num
            .checked_add(amount)
            .ok_or(V16Error::CounterOverflow)?;
        self.source_credit[domain].valid_liened_backing_num -= amount;
        self.refresh_source_credit_domain_after_mutation(domain)
    }

    pub fn consume_source_credit_lien_from_counterparty_not_atomic(
        &mut self,
        domain: usize,
        amount: u128,
    ) -> V16Result<()> {
        self.validate_source_domain_index(domain)?;
        if amount == 0 {
            return Ok(());
        }
        let bucket = &mut self.source_backing_buckets[domain];
        if bucket.valid_liened_backing_num < amount
            || self.source_credit[domain].valid_liened_backing_num < amount
            || self.source_credit[domain].fresh_reserved_backing_num < amount
        {
            return Err(V16Error::CounterUnderflow);
        }
        bucket.valid_liened_backing_num -= amount;
        bucket.consumed_liened_backing_num = bucket
            .consumed_liened_backing_num
            .checked_add(amount)
            .ok_or(V16Error::CounterOverflow)?;
        if bucket.fresh_unliened_backing_num == 0
            && bucket.valid_liened_backing_num == 0
            && bucket.impaired_liened_backing_num == 0
        {
            bucket.status = BackingBucketStatusV16::Expired;
        }
        self.source_credit[domain].valid_liened_backing_num -= amount;
        self.source_credit[domain].fresh_reserved_backing_num -= amount;
        self.source_credit[domain].spent_backing_num = self.source_credit[domain]
            .spent_backing_num
            .checked_add(amount)
            .ok_or(V16Error::CounterOverflow)?;
        self.refresh_source_credit_domain_after_mutation(domain)
    }

    pub fn impair_source_credit_lien_from_counterparty_not_atomic(
        &mut self,
        domain: usize,
        amount: u128,
    ) -> V16Result<()> {
        self.validate_source_domain_index(domain)?;
        if amount == 0 {
            return Ok(());
        }
        let bucket = &mut self.source_backing_buckets[domain];
        if bucket.valid_liened_backing_num < amount
            || self.source_credit[domain].valid_liened_backing_num < amount
            || self.source_credit[domain].fresh_reserved_backing_num < amount
        {
            return Err(V16Error::CounterUnderflow);
        }
        bucket.valid_liened_backing_num -= amount;
        bucket.impaired_liened_backing_num = bucket
            .impaired_liened_backing_num
            .checked_add(amount)
            .ok_or(V16Error::CounterOverflow)?;
        if bucket.valid_liened_backing_num == 0 && bucket.fresh_unliened_backing_num == 0 {
            bucket.status = BackingBucketStatusV16::Impaired;
        }
        self.source_credit[domain].valid_liened_backing_num -= amount;
        self.source_credit[domain].fresh_reserved_backing_num -= amount;
        self.source_credit[domain].impaired_liened_backing_num = self.source_credit[domain]
            .impaired_liened_backing_num
            .checked_add(amount)
            .ok_or(V16Error::CounterOverflow)?;
        self.refresh_source_credit_domain_after_mutation(domain)
    }

    pub fn expire_source_backing_bucket_not_atomic(
        &mut self,
        domain: usize,
        now_slot: u64,
    ) -> V16Result<()> {
        self.validate_source_domain_index(domain)?;
        let bucket = &mut self.source_backing_buckets[domain];
        if bucket.status != BackingBucketStatusV16::Fresh || now_slot < bucket.expiry_slot {
            return Err(V16Error::Stale);
        }
        let expired_unliened = bucket.fresh_unliened_backing_num;
        let expired_liened = bucket.valid_liened_backing_num;
        let expired_total = expired_unliened
            .checked_add(expired_liened)
            .ok_or(V16Error::ArithmeticOverflow)?;
        if self.source_credit[domain].fresh_reserved_backing_num < expired_total
            || self.source_credit[domain].valid_liened_backing_num < expired_liened
        {
            return Err(V16Error::CounterUnderflow);
        }
        self.source_credit[domain].fresh_reserved_backing_num -= expired_total;
        self.source_credit[domain].valid_liened_backing_num -= expired_liened;
        self.source_credit[domain].impaired_liened_backing_num = self.source_credit[domain]
            .impaired_liened_backing_num
            .checked_add(expired_liened)
            .ok_or(V16Error::CounterOverflow)?;
        bucket.fresh_unliened_backing_num = 0;
        bucket.valid_liened_backing_num = 0;
        bucket.impaired_liened_backing_num = bucket
            .impaired_liened_backing_num
            .checked_add(expired_liened)
            .ok_or(V16Error::CounterOverflow)?;
        bucket.status = if expired_liened == 0 && bucket.impaired_liened_backing_num == 0 {
            BackingBucketStatusV16::Expired
        } else {
            BackingBucketStatusV16::Impaired
        };
        self.refresh_source_credit_domain_after_mutation(domain)
    }

    pub fn reserve_insurance_credit_not_atomic(
        &mut self,
        domain: usize,
        amount: u128,
    ) -> V16Result<()> {
        self.validate_source_domain_index(domain)?;
        if amount == 0 {
            return Ok(());
        }
        let new_reserved = self.insurance_credit_reservations[domain]
            .insurance_credit_reserved_num
            .checked_add(amount)
            .ok_or(V16Error::CounterOverflow)?;
        let mut live_source_credit_insurance_atoms = 0u128;
        let mut d = 0;
        while d < self.config.max_portfolio_assets as usize * 2 {
            let reserved_num = if d == domain {
                new_reserved
            } else {
                self.insurance_credit_reservations[d].insurance_credit_reserved_num
            };
            let reserved_atoms = Self::amount_from_bound_num(reserved_num)?;
            live_source_credit_insurance_atoms = live_source_credit_insurance_atoms
                .checked_add(reserved_atoms)
                .ok_or(V16Error::ArithmeticOverflow)?;
            d += 1;
        }
        let domain_reserved_atoms = Self::amount_from_bound_num(new_reserved)?;
        if live_source_credit_insurance_atoms > self.insurance
            || self.insurance_domain_spent[domain]
                .checked_add(domain_reserved_atoms)
                .ok_or(V16Error::ArithmeticOverflow)?
                > self.insurance_domain_budget[domain]
        {
            return Err(V16Error::LockActive);
        }
        self.insurance_credit_reservations[domain].insurance_credit_reserved_num = new_reserved;
        self.insurance_credit_reservations[domain].source_credit_epoch =
            self.source_credit[domain].credit_epoch;
        self.source_credit[domain].insurance_credit_reserved_num = self.source_credit[domain]
            .insurance_credit_reserved_num
            .checked_add(amount)
            .ok_or(V16Error::CounterOverflow)?;
        self.refresh_source_credit_domain_after_mutation(domain)
    }

    pub fn create_source_credit_lien_from_insurance_not_atomic(
        &mut self,
        domain: usize,
        amount: u128,
    ) -> V16Result<()> {
        self.validate_source_domain_index(domain)?;
        if amount == 0 {
            return Ok(());
        }
        let reservation = &mut self.insurance_credit_reservations[domain];
        let encumbered = reservation
            .valid_liened_insurance_num
            .checked_add(reservation.impaired_liened_insurance_num)
            .ok_or(V16Error::ArithmeticOverflow)?;
        let free_reserved = reservation
            .insurance_credit_reserved_num
            .checked_sub(encumbered)
            .ok_or(V16Error::CounterUnderflow)?;
        if free_reserved < amount {
            return Err(V16Error::LockActive);
        }
        reservation.valid_liened_insurance_num = reservation
            .valid_liened_insurance_num
            .checked_add(amount)
            .ok_or(V16Error::CounterOverflow)?;
        self.source_credit[domain].valid_liened_insurance_num = self.source_credit[domain]
            .valid_liened_insurance_num
            .checked_add(amount)
            .ok_or(V16Error::CounterOverflow)?;
        self.refresh_source_credit_domain_after_mutation(domain)
    }

    fn source_claim_unliened_num(account: &PortfolioAccountV16, domain: usize) -> V16Result<u128> {
        let locked = account.source_claim_liened_num[domain]
            .checked_add(account.source_claim_impaired_num[domain])
            .ok_or(V16Error::ArithmeticOverflow)?;
        account.source_claim_bound_num[domain]
            .checked_sub(locked)
            .ok_or(V16Error::CounterUnderflow)
    }

    fn valid_source_lien_effective_reserved_sum(account: &PortfolioAccountV16) -> V16Result<u128> {
        let mut sum = 0u128;
        let mut d = 0;
        while d < V16_DOMAIN_COUNT {
            sum = sum
                .checked_add(account.source_lien_effective_reserved[d])
                .ok_or(V16Error::ArithmeticOverflow)?;
            d += 1;
        }
        Ok(sum)
    }

    fn incremental_initial_margin_source_credit_needed(
        account: &PortfolioAccountV16,
        no_positive_equity: i128,
    ) -> V16Result<u128> {
        let req = account.health_cert.certified_initial_req;
        let existing_lien = Self::valid_source_lien_effective_reserved_sum(account)?;
        if no_positive_equity >= 0 {
            let covered = (no_positive_equity as u128)
                .checked_add(existing_lien)
                .ok_or(V16Error::ArithmeticOverflow)?;
            return Ok(req.saturating_sub(covered));
        }
        let need_before_lien = req
            .checked_add(no_positive_equity.unsigned_abs())
            .ok_or(V16Error::ArithmeticOverflow)?;
        Ok(need_before_lien.saturating_sub(existing_lien))
    }

    fn create_source_credit_lien_backing_not_atomic(
        &mut self,
        domain: usize,
        backing_num: u128,
    ) -> V16Result<SourceCreditBackingSourceV16> {
        self.validate_source_domain_index(domain)?;
        if backing_num == 0 {
            return Err(V16Error::InvalidConfig);
        }
        let bucket = self.source_backing_buckets[domain];
        if bucket.status == BackingBucketStatusV16::Fresh
            && bucket.expiry_slot > self.current_slot
            && bucket.fresh_unliened_backing_num >= backing_num
        {
            self.create_source_credit_lien_from_counterparty_not_atomic(domain, backing_num)?;
            return Ok(SourceCreditBackingSourceV16::Counterparty);
        }
        let reservation = self.insurance_credit_reservations[domain];
        let encumbered = reservation
            .valid_liened_insurance_num
            .checked_add(reservation.impaired_liened_insurance_num)
            .ok_or(V16Error::ArithmeticOverflow)?;
        if reservation
            .insurance_credit_reserved_num
            .checked_sub(encumbered)
            .ok_or(V16Error::CounterUnderflow)?
            >= backing_num
        {
            self.create_source_credit_lien_from_insurance_not_atomic(domain, backing_num)?;
            return Ok(SourceCreditBackingSourceV16::Insurance);
        }
        Err(V16Error::LockActive)
    }

    fn create_account_source_credit_lien_for_effective_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV16,
        domain: usize,
        effective_credit: u128,
    ) -> V16Result<()> {
        self.validate_account_shape(account)?;
        self.validate_source_domain_index(domain)?;
        if effective_credit == 0 {
            return Ok(());
        }
        self.validate_source_domain_ledger_current(domain)?;
        let rate = self.source_credit[domain].credit_rate_num;
        if rate == 0 {
            return Err(V16Error::LockActive);
        }
        let required_face_num = checked_mul_div_ceil_u256(
            U256::from_u128(
                effective_credit
                    .checked_mul(BOUND_SCALE)
                    .ok_or(V16Error::ArithmeticOverflow)?,
            ),
            U256::from_u128(CREDIT_RATE_SCALE),
            U256::from_u128(rate),
        )
        .and_then(|v| v.try_into_u128())
        .ok_or(V16Error::ArithmeticOverflow)?;
        let required_backing_num = effective_credit
            .checked_mul(BOUND_SCALE)
            .ok_or(V16Error::ArithmeticOverflow)?;
        if Self::source_claim_unliened_num(account, domain)? < required_face_num {
            return Err(V16Error::LockActive);
        }
        let backing_source =
            self.create_source_credit_lien_backing_not_atomic(domain, required_backing_num)?;
        account.source_claim_liened_num[domain] = account.source_claim_liened_num[domain]
            .checked_add(required_face_num)
            .ok_or(V16Error::ArithmeticOverflow)?;
        account.source_lien_effective_reserved[domain] = account.source_lien_effective_reserved
            [domain]
            .checked_add(effective_credit)
            .ok_or(V16Error::ArithmeticOverflow)?;
        match backing_source {
            SourceCreditBackingSourceV16::Counterparty => {
                account.source_claim_counterparty_liened_num[domain] = account
                    .source_claim_counterparty_liened_num[domain]
                    .checked_add(required_face_num)
                    .ok_or(V16Error::ArithmeticOverflow)?;
                account.source_lien_counterparty_backing_num[domain] = account
                    .source_lien_counterparty_backing_num[domain]
                    .checked_add(required_backing_num)
                    .ok_or(V16Error::ArithmeticOverflow)?;
            }
            SourceCreditBackingSourceV16::Insurance => {
                account.source_claim_insurance_liened_num[domain] = account
                    .source_claim_insurance_liened_num[domain]
                    .checked_add(required_face_num)
                    .ok_or(V16Error::ArithmeticOverflow)?;
                account.source_lien_insurance_backing_num[domain] = account
                    .source_lien_insurance_backing_num[domain]
                    .checked_add(required_backing_num)
                    .ok_or(V16Error::ArithmeticOverflow)?;
            }
        }
        account.health_cert.valid = false;
        self.validate_account_shape(account)?;
        Ok(())
    }

    fn create_account_source_credit_lien_for_effective_any_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV16,
        effective_credit: u128,
    ) -> V16Result<()> {
        self.validate_account_shape(account)?;
        let mut remaining = effective_credit;
        let mut d = 0;
        while d < V16_DOMAIN_COUNT && remaining != 0 {
            if d >= self.config.max_portfolio_assets as usize * 2 {
                break;
            }
            let rate = self.source_credit[d].credit_rate_num;
            let unliened = Self::source_claim_unliened_num(account, d)?;
            if rate != 0 && unliened != 0 {
                self.validate_source_domain_ledger_current(d)?;
                let soft_num = U256::from_u128(unliened)
                    .checked_mul(U256::from_u128(rate))
                    .and_then(|v| v.checked_div(U256::from_u128(CREDIT_RATE_SCALE)))
                    .and_then(|v| v.try_into_u128())
                    .ok_or(V16Error::ArithmeticOverflow)?;
                let by_claim = soft_num / BOUND_SCALE;
                let by_backing = self.source_credit_available_backing_num(d)? / BOUND_SCALE;
                let take = remaining.min(by_claim).min(by_backing);
                if take != 0 {
                    self.create_account_source_credit_lien_for_effective_not_atomic(
                        account, d, take,
                    )?;
                    remaining -= take;
                }
            }
            d += 1;
        }
        if remaining != 0 {
            return Err(V16Error::LockActive);
        }
        Ok(())
    }

    fn create_initial_margin_source_lien_if_needed(
        &mut self,
        account: &mut PortfolioAccountV16,
    ) -> V16Result<()> {
        if !account.health_cert.valid {
            return Err(V16Error::Stale);
        }
        let no_positive = account_no_positive_credit_equity(account)?;
        let required_credit =
            Self::incremental_initial_margin_source_credit_needed(account, no_positive)?;
        if required_credit == 0 {
            return Ok(());
        }
        self.create_account_source_credit_lien_for_effective_any_not_atomic(
            account,
            required_credit,
        )
    }

    fn create_initial_margin_source_lien_with_capital_if_needed(
        &mut self,
        account: &mut PortfolioAccountV16,
        capital_override: u128,
    ) -> V16Result<()> {
        if !account.health_cert.valid {
            return Err(V16Error::Stale);
        }
        let no_positive =
            account_no_positive_credit_equity_with_capital(account, capital_override)?;
        let required_credit =
            Self::incremental_initial_margin_source_credit_needed(account, no_positive)?;
        if required_credit == 0 {
            return Ok(());
        }
        self.create_account_source_credit_lien_for_effective_any_not_atomic(
            account,
            required_credit,
        )
    }

    fn create_and_consume_account_source_credit_for_effective_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV16,
        effective_credit: u128,
    ) -> V16Result<SourceCreditConsumptionV16> {
        self.validate_account_shape(account)?;
        if effective_credit == 0 {
            return Ok(SourceCreditConsumptionV16 {
                face_burn: 0,
                counterparty_credit_consumed: 0,
                insurance_credit_consumed: 0,
                domain_effective_consumed: [0; V16_DOMAIN_COUNT],
            });
        }
        let mut remaining = effective_credit;
        let mut face_burn_num = 0u128;
        let mut counterparty_credit_consumed = 0u128;
        let mut insurance_credit_consumed = 0u128;
        let mut domain_effective_consumed = [0u128; V16_DOMAIN_COUNT];
        let mut d = 0;
        while d < V16_DOMAIN_COUNT && remaining != 0 {
            if d >= self.config.max_portfolio_assets as usize * 2 {
                break;
            }
            let rate = self.source_credit[d].credit_rate_num;
            let unliened = Self::source_claim_unliened_num(account, d)?;
            if rate != 0 && unliened != 0 {
                self.validate_source_domain_ledger_current(d)?;
                let soft_num = U256::from_u128(unliened)
                    .checked_mul(U256::from_u128(rate))
                    .and_then(|v| v.checked_div(U256::from_u128(CREDIT_RATE_SCALE)))
                    .and_then(|v| v.try_into_u128())
                    .ok_or(V16Error::ArithmeticOverflow)?;
                let by_claim = soft_num / BOUND_SCALE;
                let by_backing = self.source_credit_available_backing_num(d)? / BOUND_SCALE;
                let take = remaining.min(by_claim).min(by_backing);
                if take != 0 {
                    let face_num = checked_mul_div_ceil_u256(
                        U256::from_u128(
                            take.checked_mul(BOUND_SCALE)
                                .ok_or(V16Error::ArithmeticOverflow)?,
                        ),
                        U256::from_u128(CREDIT_RATE_SCALE),
                        U256::from_u128(rate),
                    )
                    .and_then(|v| v.try_into_u128())
                    .ok_or(V16Error::ArithmeticOverflow)?;
                    let backing_num = take
                        .checked_mul(BOUND_SCALE)
                        .ok_or(V16Error::ArithmeticOverflow)?;
                    if self.source_backing_buckets[d].status == BackingBucketStatusV16::Fresh
                        && self.source_backing_buckets[d].expiry_slot > self.current_slot
                        && self.source_backing_buckets[d].fresh_unliened_backing_num >= backing_num
                    {
                        self.create_source_credit_lien_from_counterparty_not_atomic(
                            d,
                            backing_num,
                        )?;
                        self.consume_source_credit_lien_from_counterparty_not_atomic(
                            d,
                            backing_num,
                        )?;
                        counterparty_credit_consumed = counterparty_credit_consumed
                            .checked_add(take)
                            .ok_or(V16Error::ArithmeticOverflow)?;
                    } else {
                        self.create_source_credit_lien_from_insurance_not_atomic(d, backing_num)?;
                        self.consume_source_credit_lien_from_insurance_not_atomic(d, backing_num)?;
                        insurance_credit_consumed = insurance_credit_consumed
                            .checked_add(take)
                            .ok_or(V16Error::ArithmeticOverflow)?;
                    }
                    face_burn_num = face_burn_num
                        .checked_add(face_num)
                        .ok_or(V16Error::ArithmeticOverflow)?;
                    domain_effective_consumed[d] = domain_effective_consumed[d]
                        .checked_add(take)
                        .ok_or(V16Error::ArithmeticOverflow)?;
                    remaining -= take;
                }
            }
            d += 1;
        }
        if remaining != 0 {
            return Err(V16Error::LockActive);
        }
        Ok(SourceCreditConsumptionV16 {
            face_burn: Self::amount_from_bound_num(face_burn_num)?,
            counterparty_credit_consumed,
            insurance_credit_consumed,
            domain_effective_consumed,
        })
    }

    pub fn release_source_credit_lien_from_insurance_not_atomic(
        &mut self,
        domain: usize,
        amount: u128,
    ) -> V16Result<()> {
        self.validate_source_domain_index(domain)?;
        if amount == 0 {
            return Ok(());
        }
        let reservation = &mut self.insurance_credit_reservations[domain];
        if reservation.valid_liened_insurance_num < amount
            || self.source_credit[domain].valid_liened_insurance_num < amount
        {
            return Err(V16Error::CounterUnderflow);
        }
        reservation.valid_liened_insurance_num -= amount;
        self.source_credit[domain].valid_liened_insurance_num -= amount;
        self.refresh_source_credit_domain_after_mutation(domain)
    }

    pub fn consume_source_credit_lien_from_insurance_not_atomic(
        &mut self,
        domain: usize,
        amount: u128,
    ) -> V16Result<()> {
        self.validate_source_domain_index(domain)?;
        if amount == 0 {
            return Ok(());
        }
        let spend_atoms = Self::amount_from_bound_num(amount)?;
        let reservation = &mut self.insurance_credit_reservations[domain];
        if reservation.valid_liened_insurance_num < amount
            || reservation.insurance_credit_reserved_num < amount
            || self.source_credit[domain].valid_liened_insurance_num < amount
            || self.source_credit[domain].insurance_credit_reserved_num < amount
            || self.insurance < spend_atoms
        {
            return Err(V16Error::CounterUnderflow);
        }
        let vault_before = self.vault;
        reservation.valid_liened_insurance_num -= amount;
        reservation.insurance_credit_reserved_num -= amount;
        reservation.consumed_insurance_num = reservation
            .consumed_insurance_num
            .checked_add(amount)
            .ok_or(V16Error::CounterOverflow)?;
        self.source_credit[domain].valid_liened_insurance_num -= amount;
        self.source_credit[domain].insurance_credit_reserved_num -= amount;
        self.insurance -= spend_atoms;
        self.insurance_domain_spent[domain] = self.insurance_domain_spent[domain]
            .checked_add(spend_atoms)
            .ok_or(V16Error::CounterOverflow)?;
        TokenValueFlowProofV16::validate_insurance_to_close_insurance_spent(
            spend_atoms,
            vault_before,
            self.vault,
        )?;
        self.refresh_source_credit_domain_after_mutation(domain)
    }

    pub fn impair_source_credit_lien_from_insurance_not_atomic(
        &mut self,
        domain: usize,
        amount: u128,
    ) -> V16Result<()> {
        self.validate_source_domain_index(domain)?;
        if amount == 0 {
            return Ok(());
        }
        let reservation = &mut self.insurance_credit_reservations[domain];
        if reservation.valid_liened_insurance_num < amount
            || self.source_credit[domain].valid_liened_insurance_num < amount
        {
            return Err(V16Error::CounterUnderflow);
        }
        reservation.valid_liened_insurance_num -= amount;
        reservation.impaired_liened_insurance_num = reservation
            .impaired_liened_insurance_num
            .checked_add(amount)
            .ok_or(V16Error::CounterOverflow)?;
        self.source_credit[domain].valid_liened_insurance_num -= amount;
        self.source_credit[domain].impaired_liened_insurance_num = self.source_credit[domain]
            .impaired_liened_insurance_num
            .checked_add(amount)
            .ok_or(V16Error::CounterOverflow)?;
        self.refresh_source_credit_domain_after_mutation(domain)
    }

    fn refresh_source_credit_domain_after_mutation(&mut self, domain: usize) -> V16Result<()> {
        self.recompute_source_credit_domain_after_mutation(domain)?;
        self.reservation_encumbrance_proof_for_domain(domain)?
            .validate()?;
        self.assert_public_invariants()
    }

    fn recompute_source_credit_domain_after_mutation(&mut self, domain: usize) -> V16Result<()> {
        let rate = Self::expected_source_credit_rate_num_for_state(self.source_credit[domain])?;
        self.source_credit[domain].credit_rate_num = rate;
        self.source_credit[domain].credit_epoch = self.source_credit[domain]
            .credit_epoch
            .checked_add(1)
            .ok_or(V16Error::CounterOverflow)?;
        self.risk_epoch = self
            .risk_epoch
            .checked_add(1)
            .ok_or(V16Error::CounterOverflow)?;
        Ok(())
    }

    pub fn reservation_encumbrance_proof_for_domain(
        &self,
        domain: usize,
    ) -> V16Result<ReservationEncumbranceProofV16> {
        self.validate_source_domain_index(domain)?;
        let source = self.source_credit[domain];
        let bucket = self.source_backing_buckets[domain];
        let reservation = self.insurance_credit_reservations[domain];
        Ok(ReservationEncumbranceProofV16 {
            domain: domain as u16,
            exact_positive_claim_num: source.exact_positive_claim_num,
            positive_claim_bound_num: source.positive_claim_bound_num,
            source_fresh_reserved_backing_num: source.fresh_reserved_backing_num,
            bucket_fresh_unliened_backing_num: bucket.fresh_unliened_backing_num,
            bucket_valid_liened_backing_num: bucket.valid_liened_backing_num,
            source_valid_liened_backing_num: source.valid_liened_backing_num,
            source_impaired_liened_backing_num: source.impaired_liened_backing_num,
            bucket_impaired_liened_backing_num: bucket.impaired_liened_backing_num,
            source_insurance_credit_reserved_num: source.insurance_credit_reserved_num,
            reservation_insurance_credit_reserved_num: reservation.insurance_credit_reserved_num,
            source_valid_liened_insurance_num: source.valid_liened_insurance_num,
            reservation_valid_liened_insurance_num: reservation.valid_liened_insurance_num,
            source_impaired_liened_insurance_num: source.impaired_liened_insurance_num,
            reservation_impaired_liened_insurance_num: reservation.impaired_liened_insurance_num,
            source_credit_rate_num: source.credit_rate_num,
        })
    }

    fn validate_source_domain_index(&self, domain: usize) -> V16Result<()> {
        if domain >= self.config.max_portfolio_assets as usize * 2 {
            return Err(V16Error::InvalidLeg);
        }
        Ok(())
    }

    fn available_backing_num_for_source_credit_state(
        state: SourceCreditStateV16,
    ) -> V16Result<u128> {
        if state.fresh_reserved_backing_num < state.valid_liened_backing_num {
            return Err(V16Error::InvalidConfig);
        }
        let insurance_encumbered = state
            .valid_liened_insurance_num
            .checked_add(state.impaired_liened_insurance_num)
            .ok_or(V16Error::ArithmeticOverflow)?;
        if state.insurance_credit_reserved_num < insurance_encumbered {
            return Err(V16Error::InvalidConfig);
        }
        let counterparty_available =
            state.fresh_reserved_backing_num - state.valid_liened_backing_num;
        let insurance_available = state.insurance_credit_reserved_num - insurance_encumbered;
        counterparty_available
            .checked_add(insurance_available)
            .ok_or(V16Error::ArithmeticOverflow)
    }

    fn expected_source_credit_rate_num_for_state(state: SourceCreditStateV16) -> V16Result<u128> {
        Self::validate_source_credit_state_shape_static(state)?;
        if state.positive_claim_bound_num == 0 {
            return Ok(CREDIT_RATE_SCALE);
        }
        let available = Self::available_backing_num_for_source_credit_state(state)?;
        let rate = U256::from_u128(available)
            .checked_mul(U256::from_u128(CREDIT_RATE_SCALE))
            .and_then(|v| v.checked_div(U256::from_u128(state.positive_claim_bound_num)))
            .and_then(|v| v.try_into_u128())
            .ok_or(V16Error::ArithmeticOverflow)?;
        Ok(core::cmp::min(rate, CREDIT_RATE_SCALE))
    }

    fn validate_source_credit_state_shape_static(state: SourceCreditStateV16) -> V16Result<()> {
        if state.exact_positive_claim_num > state.positive_claim_bound_num
            || state.credit_rate_num > CREDIT_RATE_SCALE
        {
            return Err(V16Error::InvalidConfig);
        }
        Self::available_backing_num_for_source_credit_state(state).map(|_| ())
    }

    fn validate_source_credit_state_static(state: SourceCreditStateV16) -> V16Result<()> {
        Self::validate_source_credit_state_shape_static(state)?;
        let expected = Self::expected_source_credit_rate_num_for_state(state)?;
        if state.credit_rate_num != expected {
            return Err(V16Error::InvalidConfig);
        }
        Ok(())
    }

    fn validate_backing_bucket_static(bucket: BackingBucketV16) -> V16Result<()> {
        match bucket.status {
            BackingBucketStatusV16::Empty => {
                if bucket != BackingBucketV16::EMPTY {
                    return Err(V16Error::InvalidConfig);
                }
            }
            BackingBucketStatusV16::Fresh => {
                if bucket.expiry_slot == 0
                    || bucket
                        .fresh_unliened_backing_num
                        .checked_add(bucket.valid_liened_backing_num)
                        .ok_or(V16Error::ArithmeticOverflow)?
                        == 0
                {
                    return Err(V16Error::InvalidConfig);
                }
            }
            BackingBucketStatusV16::Expired => {
                if bucket.fresh_unliened_backing_num != 0
                    || bucket.valid_liened_backing_num != 0
                    || bucket.impaired_liened_backing_num != 0
                {
                    return Err(V16Error::InvalidConfig);
                }
            }
            BackingBucketStatusV16::Impaired => {
                if bucket.fresh_unliened_backing_num != 0
                    || bucket.valid_liened_backing_num != 0
                    || bucket.impaired_liened_backing_num == 0
                {
                    return Err(V16Error::InvalidConfig);
                }
            }
        }
        Ok(())
    }

    fn validate_insurance_reservation_static(
        reservation: InsuranceCreditReservationV16,
    ) -> V16Result<()> {
        let encumbered = reservation
            .valid_liened_insurance_num
            .checked_add(reservation.impaired_liened_insurance_num)
            .ok_or(V16Error::ArithmeticOverflow)?;
        if reservation.insurance_credit_reserved_num < encumbered {
            return Err(V16Error::InvalidConfig);
        }
        Ok(())
    }

    fn validate_source_domain_ledger(&self, domain: usize) -> V16Result<()> {
        let source = self.source_credit[domain];
        let bucket = self.source_backing_buckets[domain];
        let reservation = self.insurance_credit_reservations[domain];
        Self::validate_source_credit_state_static(source)?;
        Self::validate_backing_bucket_static(bucket)?;
        Self::validate_insurance_reservation_static(reservation)?;
        let fresh_reserved = bucket
            .fresh_unliened_backing_num
            .checked_add(bucket.valid_liened_backing_num)
            .ok_or(V16Error::ArithmeticOverflow)?;
        if source.fresh_reserved_backing_num != fresh_reserved
            || source.valid_liened_backing_num != bucket.valid_liened_backing_num
            || source.impaired_liened_backing_num != bucket.impaired_liened_backing_num
            || source.insurance_credit_reserved_num != reservation.insurance_credit_reserved_num
            || source.valid_liened_insurance_num != reservation.valid_liened_insurance_num
            || source.impaired_liened_insurance_num != reservation.impaired_liened_insurance_num
        {
            return Err(V16Error::InvalidConfig);
        }
        Ok(())
    }

    fn validate_source_domain_ledger_current(&self, domain: usize) -> V16Result<()> {
        self.validate_source_domain_ledger(domain)?;
        let bucket = self.source_backing_buckets[domain];
        if bucket.status == BackingBucketStatusV16::Fresh && bucket.expiry_slot <= self.current_slot
        {
            return Err(V16Error::Stale);
        }
        Ok(())
    }

    fn validate_resolved_payout_ledger(&self) -> V16Result<()> {
        let ledger = self.resolved_payout_ledger;
        if !self.payout_snapshot_captured {
            if ledger != ResolvedPayoutLedgerV16::EMPTY {
                return Err(V16Error::InvalidConfig);
            }
            return Ok(());
        }
        let total_bound_num = ledger
            .terminal_claim_exact_receipts_num
            .checked_add(ledger.terminal_claim_bound_unreceipted_num)
            .ok_or(V16Error::ArithmeticOverflow)?;
        let (expected_num, expected_den) = if total_bound_num == 0 {
            (1, 1)
        } else {
            let capped_snapshot_num = ledger
                .snapshot_residual
                .checked_mul(BOUND_SCALE)
                .ok_or(V16Error::ArithmeticOverflow)?
                .min(total_bound_num);
            (capped_snapshot_num, total_bound_num)
        };
        if ledger.current_payout_rate_num != expected_num
            || ledger.current_payout_rate_den != expected_den
            || ledger.snapshot_residual != self.payout_snapshot
            || ledger.current_payout_rate_den == 0
            || ledger.snapshot_slot > self.current_slot.max(self.resolved_slot)
        {
            return Err(V16Error::InvalidConfig);
        }
        Ok(())
    }

    fn b_target_for_leg(&self, asset_index: usize, leg: PortfolioLegV16) -> V16Result<u128> {
        let asset = self.assets[asset_index];
        let (current_b, epoch_start_b, side_epoch, mode) = match leg.side {
            SideV16::Long => (
                asset.b_long_num,
                asset.b_epoch_start_long_num,
                asset.epoch_long,
                asset.mode_long,
            ),
            SideV16::Short => (
                asset.b_short_num,
                asset.b_epoch_start_short_num,
                asset.epoch_short,
                asset.mode_short,
            ),
        };
        if leg.b_epoch_snap == side_epoch {
            Ok(current_b)
        } else if mode == SideModeV16::ResetPending
            && leg.b_epoch_snap.checked_add(1) == Some(side_epoch)
        {
            Ok(epoch_start_b)
        } else {
            Err(V16Error::InvalidLeg)
        }
    }

    fn side_mode_for(&self, asset_index: usize, side: SideV16) -> V16Result<SideModeV16> {
        if asset_index >= self.config.max_portfolio_assets as usize {
            return Err(V16Error::InvalidLeg);
        }
        let asset = self.assets[asset_index];
        Ok(match side {
            SideV16::Long => asset.mode_long,
            SideV16::Short => asset.mode_short,
        })
    }

    fn validate_configured_asset_index(&self, asset_index: usize) -> V16Result<()> {
        if asset_index >= self.config.max_portfolio_assets as usize {
            return Err(V16Error::InvalidLeg);
        }
        Ok(())
    }

    fn bump_asset_set_epoch(&mut self) -> V16Result<()> {
        self.asset_set_epoch = self
            .asset_set_epoch
            .checked_add(1)
            .ok_or(V16Error::CounterOverflow)?;
        self.risk_epoch = self
            .risk_epoch
            .checked_add(1)
            .ok_or(V16Error::CounterOverflow)?;
        Ok(())
    }

    fn require_asset_active_for_risk_increase(&self, asset_index: usize) -> V16Result<()> {
        self.validate_configured_asset_index(asset_index)?;
        if self.assets[asset_index].lifecycle != AssetLifecycleV16::Active {
            return Err(V16Error::LockActive);
        }
        Ok(())
    }

    fn require_asset_accruable(&self, asset_index: usize) -> V16Result<()> {
        self.validate_configured_asset_index(asset_index)?;
        match self.assets[asset_index].lifecycle {
            AssetLifecycleV16::Active | AssetLifecycleV16::DrainOnly => Ok(()),
            _ => Err(V16Error::LockActive),
        }
    }

    fn require_asset_live_reducible(&self, asset_index: usize) -> V16Result<()> {
        self.validate_configured_asset_index(asset_index)?;
        match self.assets[asset_index].lifecycle {
            AssetLifecycleV16::Active | AssetLifecycleV16::DrainOnly => Ok(()),
            _ => Err(V16Error::LockActive),
        }
    }

    fn require_empty_asset_lifecycle_state(&self, asset_index: usize) -> V16Result<()> {
        self.validate_configured_asset_index(asset_index)?;
        let asset = self.assets[asset_index];
        let long_domain = self.insurance_domain_index(asset_index, SideV16::Long)?;
        let short_domain = self.insurance_domain_index(asset_index, SideV16::Short)?;
        if self.pending_domain_loss_barriers[long_domain] != 0
            || self.pending_domain_loss_barriers[short_domain] != 0
            || asset.mode_long != SideModeV16::Normal
            || asset.mode_short != SideModeV16::Normal
            || asset.a_long != ADL_ONE
            || asset.a_short != ADL_ONE
            || asset.k_long != 0
            || asset.k_short != 0
            || asset.f_long_num != 0
            || asset.f_short_num != 0
            || asset.k_epoch_start_long != 0
            || asset.k_epoch_start_short != 0
            || asset.f_epoch_start_long_num != 0
            || asset.f_epoch_start_short_num != 0
            || asset.b_long_num != 0
            || asset.b_short_num != 0
            || asset.b_epoch_start_long_num != 0
            || asset.b_epoch_start_short_num != 0
            || asset.oi_eff_long_q != 0
            || asset.oi_eff_short_q != 0
            || asset.stored_pos_count_long != 0
            || asset.stored_pos_count_short != 0
            || asset.stale_account_count_long != 0
            || asset.stale_account_count_short != 0
            || asset.pending_obligation_count_long != 0
            || asset.pending_obligation_count_short != 0
            || asset.loss_weight_sum_long != 0
            || asset.loss_weight_sum_short != 0
            || asset.social_loss_remainder_long_num != 0
            || asset.social_loss_remainder_short_num != 0
            || asset.social_loss_dust_long_num != 0
            || asset.social_loss_dust_short_num != 0
            || asset.explicit_unallocated_loss_long != 0
            || asset.explicit_unallocated_loss_short != 0
            || self.insurance_domain_spent[long_domain] != 0
            || self.insurance_domain_spent[short_domain] != 0
            || self.source_credit[long_domain] != SourceCreditStateV16::EMPTY
            || self.source_credit[short_domain] != SourceCreditStateV16::EMPTY
            || self.source_backing_buckets[long_domain] != BackingBucketV16::EMPTY
            || self.source_backing_buckets[short_domain] != BackingBucketV16::EMPTY
            || self.insurance_credit_reservations[long_domain]
                != InsuranceCreditReservationV16::EMPTY
            || self.insurance_credit_reservations[short_domain]
                != InsuranceCreditReservationV16::EMPTY
        {
            return Err(V16Error::LockActive);
        }
        Ok(())
    }

    fn leg_is_dead_for_forfeit(&self, asset_index: usize, side: SideV16) -> V16Result<bool> {
        let side_mode = self.side_mode_for(asset_index, side)?;
        let asset_lifecycle = self.assets[asset_index].lifecycle;
        Ok(self.mode == MarketModeV16::Recovery
            || asset_lifecycle == AssetLifecycleV16::Recovery
            || matches!(
                side_mode,
                SideModeV16::DrainOnly | SideModeV16::ResetPending
            ))
    }

    fn kf_target_for_leg(
        &self,
        asset_index: usize,
        leg: PortfolioLegV16,
    ) -> V16Result<(i128, i128)> {
        let asset = self.assets[asset_index];
        let (current_k, current_f, epoch_start_k, epoch_start_f, side_epoch, mode) = match leg.side
        {
            SideV16::Long => (
                asset.k_long,
                asset.f_long_num,
                asset.k_epoch_start_long,
                asset.f_epoch_start_long_num,
                asset.epoch_long,
                asset.mode_long,
            ),
            SideV16::Short => (
                asset.k_short,
                asset.f_short_num,
                asset.k_epoch_start_short,
                asset.f_epoch_start_short_num,
                asset.epoch_short,
                asset.mode_short,
            ),
        };
        if leg.epoch_snap == side_epoch {
            Ok((current_k, current_f))
        } else if mode == SideModeV16::ResetPending
            && leg.epoch_snap.checked_add(1) == Some(side_epoch)
        {
            Ok((epoch_start_k, epoch_start_f))
        } else {
            Err(V16Error::InvalidLeg)
        }
    }

    fn residual(&self) -> u128 {
        self.vault
            .saturating_sub(self.c_tot.saturating_add(self.insurance))
    }

    fn amount_from_bound_num(bound_num: u128) -> V16Result<u128> {
        let whole = bound_num / BOUND_SCALE;
        let rem = bound_num % BOUND_SCALE;
        if rem == 0 {
            Ok(whole)
        } else {
            whole.checked_add(1).ok_or(V16Error::ArithmeticOverflow)
        }
    }

    fn bound_num_from_amount(amount: u128) -> V16Result<u128> {
        amount
            .checked_mul(BOUND_SCALE)
            .ok_or(V16Error::ArithmeticOverflow)
    }

    fn account_source_claim_bound_sum_num_static(account: &PortfolioAccountV16) -> V16Result<u128> {
        let mut sum = 0u128;
        let mut d = 0;
        while d < V16_DOMAIN_COUNT {
            sum = sum
                .checked_add(account.source_claim_bound_num[d])
                .ok_or(V16Error::ArithmeticOverflow)?;
            d += 1;
        }
        Ok(sum)
    }

    fn account_has_source_claims(account: &PortfolioAccountV16) -> V16Result<bool> {
        Ok(Self::account_source_claim_bound_sum_num_static(account)? != 0)
    }

    fn account_source_realizable_support(
        &self,
        account: &PortfolioAccountV16,
        face_claim: u128,
    ) -> V16Result<u128> {
        if face_claim == 0 {
            return Ok(0);
        }
        let mut remaining_num = Self::bound_num_from_amount(face_claim)?;
        let mut support_num = U256::ZERO;
        let mut d = 0;
        while d < V16_DOMAIN_COUNT && remaining_num != 0 {
            let locked = account.source_claim_liened_num[d]
                .checked_add(account.source_claim_impaired_num[d])
                .ok_or(V16Error::ArithmeticOverflow)?;
            if locked > account.source_claim_bound_num[d] {
                return Err(V16Error::InvalidLeg);
            }
            let valid_lien_effective_num = account.source_lien_effective_reserved[d]
                .checked_mul(BOUND_SCALE)
                .ok_or(V16Error::ArithmeticOverflow)?
                .min(remaining_num);
            if valid_lien_effective_num != 0 {
                support_num = support_num
                    .checked_add(U256::from_u128(valid_lien_effective_num))
                    .ok_or(V16Error::ArithmeticOverflow)?;
                remaining_num -= valid_lien_effective_num;
            }
            let claim_num = account.source_claim_bound_num[d]
                .checked_sub(locked)
                .ok_or(V16Error::CounterUnderflow)?
                .min(remaining_num);
            if claim_num != 0 {
                self.validate_source_domain_ledger_current(d)?;
                let credited_num = U256::from_u128(claim_num)
                    .checked_mul(U256::from_u128(self.source_credit[d].credit_rate_num))
                    .and_then(|v| v.checked_div(U256::from_u128(CREDIT_RATE_SCALE)))
                    .ok_or(V16Error::ArithmeticOverflow)?;
                support_num = support_num
                    .checked_add(credited_num)
                    .ok_or(V16Error::ArithmeticOverflow)?;
                remaining_num -= claim_num;
            }
            d += 1;
        }
        support_num
            .checked_div(U256::from_u128(BOUND_SCALE))
            .and_then(|v| v.try_into_u128())
            .ok_or(V16Error::ArithmeticOverflow)
    }

    fn account_unliened_source_realizable_support(
        &self,
        account: &PortfolioAccountV16,
        face_claim: u128,
    ) -> V16Result<u128> {
        if face_claim == 0 {
            return Ok(0);
        }
        let mut remaining_num = Self::bound_num_from_amount(face_claim)?;
        let mut support_num = U256::ZERO;
        let mut d = 0;
        while d < V16_DOMAIN_COUNT && remaining_num != 0 {
            let claim_num = Self::source_claim_unliened_num(account, d)?.min(remaining_num);
            if claim_num != 0 {
                self.validate_source_domain_ledger_current(d)?;
                let credited_num = U256::from_u128(claim_num)
                    .checked_mul(U256::from_u128(self.source_credit[d].credit_rate_num))
                    .and_then(|v| v.checked_div(U256::from_u128(CREDIT_RATE_SCALE)))
                    .ok_or(V16Error::ArithmeticOverflow)?;
                support_num = support_num
                    .checked_add(credited_num)
                    .ok_or(V16Error::ArithmeticOverflow)?;
                remaining_num -= claim_num;
            }
            d += 1;
        }
        support_num
            .checked_div(U256::from_u128(BOUND_SCALE))
            .and_then(|v| v.try_into_u128())
            .ok_or(V16Error::ArithmeticOverflow)
    }

    fn source_domain_realizable_support_for_face(
        &self,
        domain: usize,
        face_claim: u128,
    ) -> V16Result<u128> {
        self.validate_source_domain_index(domain)?;
        if face_claim == 0 {
            return Ok(0);
        }
        self.validate_source_domain_ledger_current(domain)?;
        let face_num = Self::bound_num_from_amount(face_claim)?;
        let credited_num = U256::from_u128(face_num)
            .checked_mul(U256::from_u128(self.source_credit[domain].credit_rate_num))
            .and_then(|v| v.checked_div(U256::from_u128(CREDIT_RATE_SCALE)))
            .and_then(|v| v.try_into_u128())
            .ok_or(V16Error::ArithmeticOverflow)?;
        let by_claim = credited_num / BOUND_SCALE;
        let by_backing = self.source_credit_available_backing_num(domain)? / BOUND_SCALE;
        Ok(by_claim.min(by_backing))
    }

    fn consume_source_domain_credit_for_effective_not_atomic(
        &mut self,
        domain: usize,
        effective_credit: u128,
    ) -> V16Result<SourceCreditConsumptionV16> {
        self.validate_source_domain_index(domain)?;
        if effective_credit == 0 {
            return Ok(SourceCreditConsumptionV16 {
                face_burn: 0,
                counterparty_credit_consumed: 0,
                insurance_credit_consumed: 0,
                domain_effective_consumed: [0; V16_DOMAIN_COUNT],
            });
        }
        self.validate_source_domain_ledger_current(domain)?;
        let rate = self.source_credit[domain].credit_rate_num;
        if rate == 0 {
            return Err(V16Error::LockActive);
        }
        let required_face_num = checked_mul_div_ceil_u256(
            U256::from_u128(
                effective_credit
                    .checked_mul(BOUND_SCALE)
                    .ok_or(V16Error::ArithmeticOverflow)?,
            ),
            U256::from_u128(CREDIT_RATE_SCALE),
            U256::from_u128(rate),
        )
        .and_then(|v| v.try_into_u128())
        .ok_or(V16Error::ArithmeticOverflow)?;
        let backing_num = effective_credit
            .checked_mul(BOUND_SCALE)
            .ok_or(V16Error::ArithmeticOverflow)?;
        if self.source_credit_available_backing_num(domain)? < backing_num {
            return Err(V16Error::LockActive);
        }
        let mut counterparty_credit_consumed = 0;
        let mut insurance_credit_consumed = 0;
        if self.source_backing_buckets[domain].status == BackingBucketStatusV16::Fresh
            && self.source_backing_buckets[domain].expiry_slot > self.current_slot
            && self.source_backing_buckets[domain].fresh_unliened_backing_num >= backing_num
        {
            self.create_source_credit_lien_from_counterparty_not_atomic(domain, backing_num)?;
            self.consume_source_credit_lien_from_counterparty_not_atomic(domain, backing_num)?;
            counterparty_credit_consumed = effective_credit;
        } else {
            self.create_source_credit_lien_from_insurance_not_atomic(domain, backing_num)?;
            self.consume_source_credit_lien_from_insurance_not_atomic(domain, backing_num)?;
            insurance_credit_consumed = effective_credit;
        }
        Ok(SourceCreditConsumptionV16 {
            face_burn: Self::amount_from_bound_num(required_face_num)?,
            counterparty_credit_consumed,
            insurance_credit_consumed,
            domain_effective_consumed: {
                let mut consumed = [0u128; V16_DOMAIN_COUNT];
                consumed[domain] = effective_credit;
                consumed
            },
        })
    }

    fn junior_claim_bound(&self) -> u128 {
        self.pnl_pos_bound_tot
    }

    fn recompute_resolved_payout_rate(&mut self) -> V16Result<()> {
        let total_bound_num = self
            .resolved_payout_ledger
            .terminal_claim_exact_receipts_num
            .checked_add(
                self.resolved_payout_ledger
                    .terminal_claim_bound_unreceipted_num,
            )
            .ok_or(V16Error::ArithmeticOverflow)?;
        if total_bound_num == 0 {
            self.resolved_payout_ledger.current_payout_rate_num = 1;
            self.resolved_payout_ledger.current_payout_rate_den = 1;
        } else {
            self.resolved_payout_ledger.current_payout_rate_num = self
                .resolved_payout_ledger
                .snapshot_residual
                .checked_mul(BOUND_SCALE)
                .ok_or(V16Error::ArithmeticOverflow)?
                .min(total_bound_num);
            self.resolved_payout_ledger.current_payout_rate_den = total_bound_num;
        }
        Ok(())
    }

    fn initialize_resolved_payout_ledger_if_needed(&mut self) -> V16Result<()> {
        if self.payout_snapshot_captured {
            return Ok(());
        }
        let snapshot_residual = self.residual();
        self.payout_snapshot = snapshot_residual;
        self.payout_snapshot_pnl_pos_tot = self.junior_claim_bound();
        self.payout_snapshot_captured = true;
        self.resolved_payout_ledger = ResolvedPayoutLedgerV16 {
            snapshot_residual,
            terminal_claim_exact_receipts_num: 0,
            terminal_claim_bound_unreceipted_num: self.pnl_pos_bound_tot_num,
            current_payout_rate_num: 0,
            current_payout_rate_den: 0,
            snapshot_slot: self.resolved_slot.max(self.current_slot),
            payout_halted: false,
            finalized: false,
        };
        self.recompute_resolved_payout_rate()
    }

    fn create_resolved_payout_receipt_if_needed(
        &mut self,
        account: &mut PortfolioAccountV16,
    ) -> V16Result<()> {
        if account.resolved_payout_receipt.present {
            return Ok(());
        }
        self.initialize_resolved_payout_ledger_if_needed()?;
        let terminal_positive_claim_face = account.pnl.max(0) as u128;
        let prior_bound_contribution_num =
            Self::bound_num_from_amount(terminal_positive_claim_face)?;
        if Self::bound_num_from_amount(terminal_positive_claim_face)? > prior_bound_contribution_num
            || prior_bound_contribution_num
                > self
                    .resolved_payout_ledger
                    .terminal_claim_bound_unreceipted_num
        {
            self.resolved_payout_ledger.payout_halted = true;
            return Err(V16Error::RecoveryRequired);
        }
        self.resolved_payout_ledger
            .terminal_claim_bound_unreceipted_num = self
            .resolved_payout_ledger
            .terminal_claim_bound_unreceipted_num
            .checked_sub(prior_bound_contribution_num)
            .ok_or(V16Error::CounterUnderflow)?;
        self.resolved_payout_ledger
            .terminal_claim_exact_receipts_num = self
            .resolved_payout_ledger
            .terminal_claim_exact_receipts_num
            .checked_add(Self::bound_num_from_amount(terminal_positive_claim_face)?)
            .ok_or(V16Error::ArithmeticOverflow)?;
        account.resolved_payout_receipt = ResolvedPayoutReceiptV16 {
            present: true,
            prior_bound_contribution_num,
            live_released_face_at_receipt: 0,
            terminal_positive_claim_face,
            paid_effective: 0,
            finalized: false,
        };
        self.recompute_resolved_payout_rate()
    }

    fn resolved_receipt_claimable_now(&self, receipt: ResolvedPayoutReceiptV16) -> V16Result<u128> {
        self.validate_resolved_payout_receipt(receipt)?;
        if !receipt.present {
            return Ok(0);
        }
        if self.resolved_payout_ledger.payout_halted {
            return Err(V16Error::RecoveryRequired);
        }
        let gross = wide_mul_div_floor_u128(
            receipt.terminal_positive_claim_face,
            self.resolved_payout_ledger.current_payout_rate_num,
            self.resolved_payout_ledger.current_payout_rate_den,
        );
        gross
            .checked_sub(receipt.paid_effective)
            .ok_or(V16Error::InvalidLeg)
    }

    fn haircut_effective_support(
        &self,
        face_claim: u128,
        residual: u128,
        junior_bound: u128,
    ) -> V16Result<u128> {
        if face_claim == 0 || residual == 0 || junior_bound == 0 {
            return Ok(0);
        }
        if residual >= junior_bound {
            return Ok(face_claim);
        }
        Ok(wide_mul_div_floor_u128(face_claim, residual, junior_bound))
    }

    fn account_haircut_equity(&self, account: &PortfolioAccountV16) -> V16Result<i128> {
        self.account_haircut_equity_with_capital(account, account.capital)
    }

    fn account_haircut_equity_with_capital(
        &self,
        account: &PortfolioAccountV16,
        capital_override: u128,
    ) -> V16Result<i128> {
        validate_non_min_i128(account.pnl)?;
        validate_fee_credits(account.fee_credits)?;
        let capital = i128::try_from(capital_override).map_err(|_| V16Error::ArithmeticOverflow)?;
        let fee_debt =
            i128::try_from(fee_debt_u128(account)?).map_err(|_| V16Error::ArithmeticOverflow)?;
        let positive_face = account.pnl.max(0) as u128;
        let global_support = self.haircut_effective_support(
            positive_face,
            self.residual(),
            self.junior_claim_bound(),
        )?;
        let positive_support = if Self::account_has_source_claims(account)? {
            global_support.min(self.account_source_realizable_support(account, positive_face)?)
        } else {
            global_support
        };
        let positive_support_i128 =
            i128::try_from(positive_support).map_err(|_| V16Error::ArithmeticOverflow)?;
        capital
            .checked_add(account.pnl.min(0))
            .and_then(|v| v.checked_add(positive_support_i128))
            .and_then(|v| v.checked_sub(fee_debt))
            .ok_or(V16Error::ArithmeticOverflow)
    }

    fn face_claim_to_burn_for_support(
        &self,
        effective_support: u128,
        residual: u128,
        junior_bound: u128,
    ) -> V16Result<u128> {
        if effective_support == 0 {
            return Ok(0);
        }
        if residual == 0 || junior_bound == 0 {
            return Err(V16Error::LockActive);
        }
        if residual >= junior_bound {
            return Ok(effective_support);
        }
        checked_mul_div_ceil_u256(
            U256::from_u128(effective_support),
            U256::from_u128(junior_bound),
            U256::from_u128(residual),
        )
        .and_then(|v| v.try_into_u128())
        .ok_or(V16Error::ArithmeticOverflow)
    }

    fn apply_haircut_bounded_close_loss_to_pnl(
        &mut self,
        account: &mut PortfolioAccountV16,
        loss_abs: u128,
    ) -> V16Result<SupportLossApplicationV16> {
        if loss_abs == 0 {
            return Ok(SupportLossApplicationV16 {
                support_consumed: 0,
                junior_face_burned: 0,
            });
        }

        let old_positive_face = account.pnl.max(0) as u128;
        if old_positive_face == 0 {
            let loss_i128 = i128::try_from(loss_abs).map_err(|_| V16Error::ArithmeticOverflow)?;
            let new_pnl = account
                .pnl
                .checked_sub(loss_i128)
                .ok_or(V16Error::ArithmeticOverflow)?;
            self.set_account_pnl(account, new_pnl)?;
            return Ok(SupportLossApplicationV16 {
                support_consumed: 0,
                junior_face_burned: 0,
            });
        }

        let has_source_claims = Self::account_has_source_claims(account)?;
        let residual = self.residual();
        let junior_bound = self.junior_claim_bound();
        let global_effective_available =
            self.haircut_effective_support(old_positive_face, residual, junior_bound)?;
        let mut source_support_selected = false;
        let effective_available = if has_source_claims {
            let source_effective_available =
                self.account_unliened_source_realizable_support(account, old_positive_face)?;
            if source_effective_available > global_effective_available {
                source_support_selected = true;
                source_effective_available
            } else {
                global_effective_available
            }
        } else {
            global_effective_available
        };
        let support_consumed = effective_available.min(loss_abs);
        let remaining_loss = loss_abs
            .checked_sub(support_consumed)
            .ok_or(V16Error::ArithmeticOverflow)?;
        let mut junior_face_burned = if source_support_selected {
            self.create_and_consume_account_source_credit_for_effective_not_atomic(
                account,
                support_consumed,
            )?
            .face_burn
            .min(old_positive_face)
        } else if support_consumed == 0 {
            0
        } else {
            self.face_claim_to_burn_for_support(support_consumed, residual, junior_bound)?
        };
        if remaining_loss != 0 {
            junior_face_burned = old_positive_face;
        }
        if junior_face_burned > old_positive_face {
            return Err(V16Error::ArithmeticOverflow);
        }

        let retained_face = old_positive_face
            .checked_sub(junior_face_burned)
            .ok_or(V16Error::ArithmeticOverflow)?;
        let retained_i128 =
            i128::try_from(retained_face).map_err(|_| V16Error::ArithmeticOverflow)?;
        let remaining_i128 =
            i128::try_from(remaining_loss).map_err(|_| V16Error::ArithmeticOverflow)?;
        let new_pnl = retained_i128
            .checked_sub(remaining_i128)
            .ok_or(V16Error::ArithmeticOverflow)?;
        account.reserved_pnl = account.reserved_pnl.min(new_pnl.max(0) as u128);
        self.set_account_pnl(account, new_pnl)?;

        Ok(SupportLossApplicationV16 {
            support_consumed,
            junior_face_burned,
        })
    }

    fn apply_signed_kf_delta_to_pnl(
        &mut self,
        account: &mut PortfolioAccountV16,
        delta: i128,
        source_domain: Option<usize>,
    ) -> V16Result<SupportLossApplicationV16> {
        validate_non_min_i128(delta)?;
        if delta == 0 {
            return Ok(SupportLossApplicationV16 {
                support_consumed: 0,
                junior_face_burned: 0,
            });
        }
        if delta < 0 {
            return self.apply_haircut_bounded_close_loss_to_pnl(account, delta.unsigned_abs());
        }
        if account.pnl >= 0 {
            let new_pnl = account
                .pnl
                .checked_add(delta)
                .ok_or(V16Error::ArithmeticOverflow)?;
            if let Some(domain) = source_domain {
                self.set_account_pnl_with_source(account, new_pnl, domain)?;
            } else {
                self.set_account_pnl(account, new_pnl)?;
            }
            return Ok(SupportLossApplicationV16 {
                support_consumed: 0,
                junior_face_burned: 0,
            });
        }

        let old_loss = account.pnl.unsigned_abs();
        let new_face_support = delta as u128;
        let residual = self.residual();
        let junior_bound = self
            .junior_claim_bound()
            .checked_add(new_face_support)
            .ok_or(V16Error::ArithmeticOverflow)?;
        let global_effective_available =
            self.haircut_effective_support(new_face_support, residual, junior_bound)?;
        let (effective_available, source_support_domain) = if let Some(domain) = source_domain {
            let source_effective_available =
                self.source_domain_realizable_support_for_face(domain, new_face_support)?;
            if source_effective_available > global_effective_available {
                (source_effective_available, Some(domain))
            } else {
                (global_effective_available, None)
            }
        } else {
            (global_effective_available, None)
        };
        let support_consumed = effective_available.min(old_loss);
        let remaining_loss = old_loss
            .checked_sub(support_consumed)
            .ok_or(V16Error::ArithmeticOverflow)?;
        let mut junior_face_burned = if let Some(domain) = source_support_domain {
            self.consume_source_domain_credit_for_effective_not_atomic(domain, support_consumed)?
                .face_burn
                .min(new_face_support)
        } else if support_consumed == 0 {
            0
        } else {
            self.face_claim_to_burn_for_support(support_consumed, residual, junior_bound)?
        };
        if remaining_loss != 0 {
            junior_face_burned = new_face_support;
        }
        if junior_face_burned > new_face_support {
            return Err(V16Error::ArithmeticOverflow);
        }

        let retained_face = new_face_support
            .checked_sub(junior_face_burned)
            .ok_or(V16Error::ArithmeticOverflow)?;
        let retained_i128 =
            i128::try_from(retained_face).map_err(|_| V16Error::ArithmeticOverflow)?;
        let remaining_i128 =
            i128::try_from(remaining_loss).map_err(|_| V16Error::ArithmeticOverflow)?;
        let new_pnl = retained_i128
            .checked_sub(remaining_i128)
            .ok_or(V16Error::ArithmeticOverflow)?;
        if new_pnl > 0 {
            if let Some(domain) = source_domain {
                self.set_account_pnl_with_source(account, new_pnl, domain)?;
            } else {
                self.set_account_pnl(account, new_pnl)?;
            }
        } else {
            self.set_account_pnl(account, new_pnl)?;
        }
        Ok(SupportLossApplicationV16 {
            support_consumed,
            junior_face_burned,
        })
    }

    fn insurance_domain_index(&self, asset_index: usize, side: SideV16) -> V16Result<usize> {
        if asset_index >= self.config.max_portfolio_assets as usize {
            return Err(V16Error::InvalidLeg);
        }
        let domain = asset_index
            .checked_mul(2)
            .and_then(|v| v.checked_add(encode_side(side) as usize))
            .ok_or(V16Error::ArithmeticOverflow)?;
        if domain >= V16_DOMAIN_COUNT {
            return Err(V16Error::InvalidLeg);
        }
        Ok(domain)
    }

    fn source_domain_asset_side(&self, domain: usize) -> V16Result<(usize, SideV16)> {
        self.validate_source_domain_index(domain)?;
        let asset_index = domain / 2;
        let source_side = decode_side((domain % 2) as u8)?;
        Ok((asset_index, source_side))
    }

    fn account_has_active_exposure_for_source_domain(
        &self,
        account: &PortfolioAccountV16,
        domain: usize,
    ) -> V16Result<bool> {
        let (asset_index, source_side) = self.source_domain_asset_side(domain)?;
        let leg = account.legs[asset_index];
        Ok(leg.active && opposite_side(leg.side) == source_side)
    }

    pub fn pending_domain_loss_barrier_count(
        &self,
        asset_index: usize,
        side: SideV16,
    ) -> V16Result<u64> {
        let domain = self.insurance_domain_index(asset_index, side)?;
        Ok(self.pending_domain_loss_barriers[domain])
    }

    fn has_pending_domain_loss_barrier(
        &self,
        asset_index: usize,
        side: SideV16,
    ) -> V16Result<bool> {
        Ok(self.pending_domain_loss_barrier_count(asset_index, side)? != 0)
    }

    fn account_touches_pending_domain_loss_barrier(
        &self,
        account: &PortfolioAccountV16,
    ) -> V16Result<bool> {
        let limit = core::cmp::min(
            self.config.max_portfolio_assets as usize,
            V16_MAX_PORTFOLIO_ASSETS_N,
        );
        let mut i = 0usize;
        while i < limit {
            let leg = account.legs[i];
            if leg.active && self.has_pending_domain_loss_barrier(i, leg.side)? {
                return Ok(true);
            }
            i += 1;
        }
        Ok(false)
    }

    fn position_delta_touches_pending_domain_loss_barrier(
        &self,
        account: &PortfolioAccountV16,
        asset_index: usize,
        delta_q: i128,
    ) -> V16Result<bool> {
        self.validate_account_shape(account)?;
        if delta_q == 0 {
            return Ok(false);
        }
        if asset_index >= self.config.max_portfolio_assets as usize {
            return Err(V16Error::InvalidLeg);
        }
        let current = signed_position(account.legs[asset_index]);
        let next = current
            .checked_add(delta_q)
            .ok_or(V16Error::ArithmeticOverflow)?;
        validate_basis_or_zero(next)?;
        if current != 0 {
            let current_side = if current > 0 {
                SideV16::Long
            } else {
                SideV16::Short
            };
            if self.has_pending_domain_loss_barrier(asset_index, current_side)? {
                return Ok(true);
            }
        }
        if next != 0 {
            let next_side = if next > 0 {
                SideV16::Long
            } else {
                SideV16::Short
            };
            if self.has_pending_domain_loss_barrier(asset_index, next_side)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn position_delta_blocked_by_pending_domain_loss_barrier(
        &self,
        account: &PortfolioAccountV16,
        asset_index: usize,
        delta_q: i128,
    ) -> V16Result<bool> {
        if !self.position_delta_touches_pending_domain_loss_barrier(
            account,
            asset_index,
            delta_q,
        )? {
            return Ok(false);
        }
        let current = signed_position(account.legs[asset_index]);
        let next = current
            .checked_add(delta_q)
            .ok_or(V16Error::ArithmeticOverflow)?;
        Ok(!same_side_risk_reduction_or_flat_obligation(current, next))
    }

    fn available_domain_insurance(&self, domain: usize) -> u128 {
        if domain >= V16_DOMAIN_COUNT {
            return 0;
        }
        let budget_remaining = self.insurance_domain_budget[domain]
            .saturating_sub(self.insurance_domain_spent[domain]);
        self.insurance.min(budget_remaining)
    }

    fn consume_domain_insurance_for_negative_pnl(
        &mut self,
        asset_index: usize,
        bankrupt_side: SideV16,
        account: &mut PortfolioAccountV16,
    ) -> V16Result<u128> {
        let domain = self.insurance_domain_index(asset_index, opposite_side(bankrupt_side))?;
        if account.pnl >= 0 {
            return Ok(0);
        }
        self.bankruptcy_hlock_active = true;
        let residual = account.pnl.unsigned_abs();
        let domain_available = self.available_domain_insurance(domain);
        let used = residual.min(domain_available);
        if used == 0 {
            return Ok(0);
        }
        let vault_before = self.vault;
        self.insurance = self
            .insurance
            .checked_sub(used)
            .ok_or(V16Error::CounterUnderflow)?;
        self.insurance_domain_spent[domain] = self.insurance_domain_spent[domain]
            .checked_add(used)
            .ok_or(V16Error::ArithmeticOverflow)?;
        let used_i128 = i128::try_from(used).map_err(|_| V16Error::ArithmeticOverflow)?;
        let new_pnl = account
            .pnl
            .checked_add(used_i128)
            .ok_or(V16Error::ArithmeticOverflow)?;
        self.set_account_pnl(account, new_pnl)?;
        TokenValueFlowProofV16::validate_insurance_to_close_insurance_spent(
            used,
            vault_before,
            self.vault,
        )?;
        account.health_cert.valid = false;
        Ok(used)
    }

    fn preflight_liquidation_residual_durability(
        &mut self,
        asset_index: usize,
        bankrupt_side: SideV16,
        account: &PortfolioAccountV16,
    ) -> V16Result<()> {
        let domain = self.insurance_domain_index(asset_index, opposite_side(bankrupt_side))?;
        let residual_after_principal_and_insurance = if account.pnl < 0 {
            account
                .pnl
                .unsigned_abs()
                .saturating_sub(account.capital)
                .saturating_sub(self.available_domain_insurance(domain))
        } else {
            0
        };
        if residual_after_principal_and_insurance == 0 {
            return Ok(());
        }
        let capacity = self.bankruptcy_residual_single_step_capacity(
            asset_index,
            bankrupt_side,
            residual_after_principal_and_insurance,
        )?;
        if capacity < residual_after_principal_and_insurance {
            self.declare_permissionless_recovery(
                PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress,
            )?;
            return Err(V16Error::RecoveryRequired);
        }
        Ok(())
    }

    fn bankruptcy_residual_single_step_capacity(
        &self,
        asset_index: usize,
        bankrupt_side: SideV16,
        residual_remaining: u128,
    ) -> V16Result<u128> {
        if asset_index >= self.config.max_portfolio_assets as usize {
            return Err(V16Error::InvalidLeg);
        }
        if residual_remaining == 0 {
            return Ok(0);
        }

        let opp = opposite_side(bankrupt_side);
        let asset = self.assets[asset_index];
        let (b_now, weight_sum, rem) = match opp {
            SideV16::Long => (
                asset.b_long_num,
                asset.loss_weight_sum_long,
                asset.social_loss_remainder_long_num,
            ),
            SideV16::Short => (
                asset.b_short_num,
                asset.loss_weight_sum_short,
                asset.social_loss_remainder_short_num,
            ),
        };
        if weight_sum == 0 {
            return Ok(0);
        }

        let candidate = residual_remaining.min(self.config.public_b_chunk_atoms);
        if candidate != 0 {
            if let Some(delta_b) = candidate
                .checked_mul(SOCIAL_LOSS_DEN)
                .and_then(|v| v.checked_add(rem))
                .map(|v| v / weight_sum)
            {
                if delta_b != 0 && b_now.checked_add(delta_b).is_some() {
                    return Ok(candidate);
                }
            }
        }

        let headroom_plus_one = U256::from_u128(u128::MAX - b_now)
            .checked_add(U256::ONE)
            .ok_or(V16Error::ArithmeticOverflow)?;
        let max_scaled = headroom_plus_one
            .checked_mul(U256::from_u128(weight_sum))
            .and_then(|v| v.checked_sub(U256::ONE))
            .ok_or(V16Error::ArithmeticOverflow)?;
        if U256::from_u128(rem) > max_scaled {
            return Ok(0);
        }
        let max_chunk_by_b_wide = max_scaled
            .checked_sub(U256::from_u128(rem))
            .and_then(|v| v.checked_div(U256::from_u128(SOCIAL_LOSS_DEN)))
            .ok_or(V16Error::ArithmeticOverflow)?;
        let max_chunk_by_b = max_chunk_by_b_wide
            .try_into_u128()
            .unwrap_or(residual_remaining);
        Ok(residual_remaining
            .min(max_chunk_by_b)
            .min(self.config.public_b_chunk_atoms))
    }

    fn resolved_positive_payout_ready(&self) -> bool {
        if self.b_stale_account_count != 0
            || self.stale_certificate_count != 0
            || self.negative_pnl_account_count != 0
        {
            return false;
        }
        let active_domains = self.config.max_portfolio_assets as usize * 2;
        let mut d = 0;
        while d < active_domains {
            if self.pending_domain_loss_barriers[d] != 0 {
                return false;
            }
            d += 1;
        }
        for i in 0..self.config.max_portfolio_assets as usize {
            let asset = self.assets[i];
            if asset.stored_pos_count_long != 0
                || asset.stored_pos_count_short != 0
                || asset.stale_account_count_long != 0
                || asset.stale_account_count_short != 0
            {
                return false;
            }
        }
        true
    }

    fn settle_leg_kf_effects(
        &mut self,
        account: &mut PortfolioAccountV16,
        asset_index: usize,
    ) -> V16Result<()> {
        let leg = account.legs[asset_index];
        if !leg.active {
            return Ok(());
        }
        let (k_now, f_now) = self.kf_target_for_leg(asset_index, leg)?;
        let den = leg
            .a_basis
            .checked_mul(POS_SCALE)
            .ok_or(V16Error::ArithmeticOverflow)?;
        let k_delta = wide_signed_mul_div_floor_from_k_pair(
            leg.basis_pos_q.unsigned_abs(),
            leg.k_snap,
            k_now,
            den,
        );
        let f_delta = wide_signed_mul_div_floor_from_k_pair(
            leg.basis_pos_q.unsigned_abs(),
            leg.f_snap,
            f_now,
            den,
        );
        let net = k_delta
            .checked_add(f_delta)
            .ok_or(V16Error::ArithmeticOverflow)?;
        validate_non_min_i128(net)?;
        if net != 0 {
            if net > 0 {
                let source_domain =
                    Some(self.insurance_domain_index(asset_index, opposite_side(leg.side))?);
                self.apply_signed_kf_delta_to_pnl(account, net, source_domain)?;
            } else {
                let negative_before = account.pnl.min(0).unsigned_abs();
                self.apply_signed_kf_delta_to_pnl(account, net, None)?;
                let negative_after = account.pnl.min(0).unsigned_abs();
                let loss_source_domain = self.insurance_domain_index(asset_index, leg.side)?;
                self.reserve_new_capital_backed_loss_for_source_domain_not_atomic(
                    account,
                    loss_source_domain,
                    negative_before,
                    negative_after,
                )?;
            }
        }
        account.legs[asset_index].k_snap = k_now;
        account.legs[asset_index].f_snap = f_now;
        account.health_cert.valid = false;
        Ok(())
    }

    fn settle_forfeited_leg_kf_effects(
        &mut self,
        account: &mut PortfolioAccountV16,
        asset_index: usize,
    ) -> V16Result<(u128, u128, u128, u128)> {
        let leg = account.legs[asset_index];
        if !leg.active {
            return Ok((0, 0, 0, 0));
        }
        let (k_now, f_now) = self.kf_target_for_leg(asset_index, leg)?;
        let den = leg
            .a_basis
            .checked_mul(POS_SCALE)
            .ok_or(V16Error::ArithmeticOverflow)?;
        let k_delta = wide_signed_mul_div_floor_from_k_pair(
            leg.basis_pos_q.unsigned_abs(),
            leg.k_snap,
            k_now,
            den,
        );
        let f_delta = wide_signed_mul_div_floor_from_k_pair(
            leg.basis_pos_q.unsigned_abs(),
            leg.f_snap,
            f_now,
            den,
        );
        let net = k_delta
            .checked_add(f_delta)
            .ok_or(V16Error::ArithmeticOverflow)?;
        validate_non_min_i128(net)?;

        let mut loss_settled = 0u128;
        let mut support_consumed = 0u128;
        let mut junior_face_burned = 0u128;
        let mut positive_pnl_forfeited = 0u128;
        if net < 0 {
            loss_settled = net.unsigned_abs();
            let support = self.apply_haircut_bounded_close_loss_to_pnl(account, loss_settled)?;
            support_consumed = support.support_consumed;
            junior_face_burned = support.junior_face_burned;
        } else {
            positive_pnl_forfeited = net as u128;
        }

        account.legs[asset_index].k_snap = k_now;
        account.legs[asset_index].f_snap = f_now;
        account.health_cert.valid = false;
        Ok((
            loss_settled,
            positive_pnl_forfeited,
            support_consumed,
            junior_face_burned,
        ))
    }

    fn apply_position_delta(
        &mut self,
        account: &mut PortfolioAccountV16,
        asset_index: usize,
        delta_q: i128,
    ) -> V16Result<()> {
        if delta_q == 0 {
            return Ok(());
        }
        if asset_index >= self.config.max_portfolio_assets as usize {
            return Err(V16Error::InvalidLeg);
        }
        if self.position_delta_blocked_by_pending_domain_loss_barrier(
            account,
            asset_index,
            delta_q,
        )? {
            return Err(V16Error::LockActive);
        }
        self.settle_leg_kf_effects(account, asset_index)?;
        let current = signed_position(account.legs[asset_index]);
        let new = current
            .checked_add(delta_q)
            .ok_or(V16Error::ArithmeticOverflow)?;
        validate_basis_or_zero(new)?;
        if current == 0 {
            let side = if new > 0 {
                SideV16::Long
            } else {
                SideV16::Short
            };
            return self.attach_leg(account, asset_index, side, new);
        }
        if new == 0 {
            let leg = account.legs[asset_index];
            if leg.active && self.has_pending_domain_loss_barrier(asset_index, leg.side)? {
                let old_abs = leg.basis_pos_q.unsigned_abs();
                let asset = &mut self.assets[asset_index];
                match leg.side {
                    SideV16::Long => {
                        asset.oi_eff_long_q = asset
                            .oi_eff_long_q
                            .checked_sub(old_abs)
                            .ok_or(V16Error::CounterUnderflow)?;
                        asset.pending_obligation_count_long = asset
                            .pending_obligation_count_long
                            .checked_add(1)
                            .ok_or(V16Error::CounterOverflow)?;
                    }
                    SideV16::Short => {
                        asset.oi_eff_short_q = asset
                            .oi_eff_short_q
                            .checked_sub(old_abs)
                            .ok_or(V16Error::CounterUnderflow)?;
                        asset.pending_obligation_count_short = asset
                            .pending_obligation_count_short
                            .checked_add(1)
                            .ok_or(V16Error::CounterOverflow)?;
                    }
                }
                account.legs[asset_index].basis_pos_q = 0;
                account.health_cert.valid = false;
                return self.validate_account_shape(account);
            }
            return self.clear_leg(account, asset_index);
        }
        if current.signum() != new.signum() {
            self.require_asset_active_for_risk_increase(asset_index)?;
            self.clear_leg(account, asset_index)?;
            let side = if new > 0 {
                SideV16::Long
            } else {
                SideV16::Short
            };
            return self.attach_leg(account, asset_index, side, new);
        }

        if new.unsigned_abs() > current.unsigned_abs() {
            self.require_asset_active_for_risk_increase(asset_index)?;
        }
        let old_leg = account.legs[asset_index];
        let old_abs = old_leg.basis_pos_q.unsigned_abs();
        let new_abs = new.unsigned_abs();
        let new_weight = loss_weight_for_basis(new_abs, old_leg.a_basis)?;
        let preserve_pending_obligation_weight =
            same_side_risk_reduction_or_flat_obligation(current, new)
                && self.has_pending_domain_loss_barrier(asset_index, old_leg.side)?;
        let asset = &mut self.assets[asset_index];
        match old_leg.side {
            SideV16::Long => {
                asset.oi_eff_long_q = adjust_u128(asset.oi_eff_long_q, old_abs, new_abs)?;
                if !preserve_pending_obligation_weight {
                    asset.loss_weight_sum_long =
                        adjust_u128(asset.loss_weight_sum_long, old_leg.loss_weight, new_weight)?;
                }
            }
            SideV16::Short => {
                asset.oi_eff_short_q = adjust_u128(asset.oi_eff_short_q, old_abs, new_abs)?;
                if !preserve_pending_obligation_weight {
                    asset.loss_weight_sum_short =
                        adjust_u128(asset.loss_weight_sum_short, old_leg.loss_weight, new_weight)?;
                }
            }
        }
        account.legs[asset_index].basis_pos_q = new;
        if !preserve_pending_obligation_weight {
            account.legs[asset_index].loss_weight = new_weight;
        }
        account.health_cert.valid = false;
        self.validate_account_shape(account)
    }

    fn reduce_position(
        &mut self,
        account: &mut PortfolioAccountV16,
        asset_index: usize,
        close_q: u128,
    ) -> V16Result<()> {
        if close_q == 0 {
            return Ok(());
        }
        let leg = account.legs[asset_index];
        if !leg.active {
            return Err(V16Error::InvalidLeg);
        }
        let close_i128 = i128::try_from(close_q).map_err(|_| V16Error::ArithmeticOverflow)?;
        let delta = match leg.side {
            SideV16::Long => close_i128
                .checked_neg()
                .ok_or(V16Error::ArithmeticOverflow)?,
            SideV16::Short => close_i128,
        };
        self.apply_position_delta(account, asset_index, delta)?;
        self.reduce_matching_open_interest_for_unilateral_close(asset_index, leg.side, close_q)
    }

    fn reduce_matching_open_interest_for_unilateral_close(
        &mut self,
        asset_index: usize,
        closed_side: SideV16,
        close_q: u128,
    ) -> V16Result<()> {
        if close_q == 0 {
            return Ok(());
        }
        let opp = opposite_side(closed_side);
        let asset = self.assets[asset_index];
        let (opp_oi_before, opp_a_before) = match opp {
            SideV16::Long => (asset.oi_eff_long_q, asset.a_long),
            SideV16::Short => (asset.oi_eff_short_q, asset.a_short),
        };
        if close_q > opp_oi_before {
            return Err(V16Error::InvalidLeg);
        }
        let opp_oi_after = opp_oi_before - close_q;
        let opp_a_after = if opp_oi_after == 0 {
            ADL_ONE
        } else {
            let candidate = wide_mul_div_floor_u128(opp_a_before, opp_oi_after, opp_oi_before);
            if candidate == 0 {
                self.declare_permissionless_recovery(
                    PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress,
                )?;
                return Err(V16Error::RecoveryRequired);
            }
            candidate
        };

        {
            let asset = &mut self.assets[asset_index];
            match opp {
                SideV16::Long => {
                    asset.oi_eff_long_q = opp_oi_after;
                    asset.a_long = opp_a_after;
                    if opp_oi_after != 0 && asset.a_long < MIN_A_SIDE {
                        asset.mode_long = SideModeV16::DrainOnly;
                    }
                }
                SideV16::Short => {
                    asset.oi_eff_short_q = opp_oi_after;
                    asset.a_short = opp_a_after;
                    if opp_oi_after != 0 && asset.a_short < MIN_A_SIDE {
                        asset.mode_short = SideModeV16::DrainOnly;
                    }
                }
            }
        }
        if opp_oi_after == 0 {
            self.begin_full_drain_reset_inner(asset_index, opp)?;
        }
        Ok(())
    }

    fn set_account_pnl(
        &mut self,
        account: &mut PortfolioAccountV16,
        new_pnl: i128,
    ) -> V16Result<()> {
        self.set_account_pnl_inner(account, new_pnl, None)
    }

    fn set_account_pnl_with_source(
        &mut self,
        account: &mut PortfolioAccountV16,
        new_pnl: i128,
        source_domain: usize,
    ) -> V16Result<()> {
        self.validate_source_domain_index(source_domain)?;
        self.set_account_pnl_inner(account, new_pnl, Some(source_domain))
    }

    fn set_account_pnl_inner(
        &mut self,
        account: &mut PortfolioAccountV16,
        new_pnl: i128,
        source_domain: Option<usize>,
    ) -> V16Result<()> {
        validate_non_min_i128(new_pnl)?;
        let old_pos = account.pnl.max(0) as u128;
        let new_pos = new_pnl.max(0) as u128;
        if new_pos >= old_pos {
            let increase_num = Self::bound_num_from_amount(new_pos - old_pos)?;
            self.pnl_pos_tot = self
                .pnl_pos_tot
                .checked_add(new_pos - old_pos)
                .ok_or(V16Error::ArithmeticOverflow)?;
            self.pnl_pos_bound_tot_num = self
                .pnl_pos_bound_tot_num
                .checked_add(increase_num)
                .ok_or(V16Error::ArithmeticOverflow)?;
            if increase_num != 0 {
                if let Some(domain) = source_domain {
                    account.source_claim_bound_num[domain] = account.source_claim_bound_num[domain]
                        .checked_add(increase_num)
                        .ok_or(V16Error::ArithmeticOverflow)?;
                    self.source_credit[domain].positive_claim_bound_num = self.source_credit
                        [domain]
                        .positive_claim_bound_num
                        .checked_add(increase_num)
                        .ok_or(V16Error::ArithmeticOverflow)?;
                    self.source_credit[domain].exact_positive_claim_num = self.source_credit
                        [domain]
                        .exact_positive_claim_num
                        .checked_add(increase_num)
                        .ok_or(V16Error::ArithmeticOverflow)?;
                    self.recompute_source_credit_domain_after_mutation(domain)?;
                }
            }
        } else {
            let decrease = old_pos - new_pos;
            let decrease_num = Self::bound_num_from_amount(decrease)?;
            self.burn_account_source_claim_bound_num(account, decrease_num)?;
            self.pnl_pos_tot = self
                .pnl_pos_tot
                .checked_sub(decrease)
                .ok_or(V16Error::CounterUnderflow)?;
            self.pnl_pos_bound_tot_num = self.pnl_pos_bound_tot_num.saturating_sub(decrease_num);
            let exact_min_num = Self::bound_num_from_amount(self.pnl_pos_tot)?;
            if self.pnl_pos_bound_tot_num < exact_min_num {
                self.pnl_pos_bound_tot_num = exact_min_num;
            }
            self.pnl_matured_pos_tot = self.pnl_matured_pos_tot.min(self.pnl_pos_tot);
        }
        self.pnl_pos_bound_tot = Self::amount_from_bound_num(self.pnl_pos_bound_tot_num)?;

        let old_negative = account.pnl < 0;
        let new_negative = new_pnl < 0;
        match (old_negative, new_negative) {
            (false, true) => {
                self.negative_pnl_account_count = self
                    .negative_pnl_account_count
                    .checked_add(1)
                    .ok_or(V16Error::CounterOverflow)?;
            }
            (true, false) => {
                self.negative_pnl_account_count = self
                    .negative_pnl_account_count
                    .checked_sub(1)
                    .ok_or(V16Error::CounterUnderflow)?;
            }
            _ => {}
        }
        account.pnl = new_pnl;
        Ok(())
    }

    fn burn_account_source_claim_bound_num(
        &mut self,
        account: &mut PortfolioAccountV16,
        mut burn_num: u128,
    ) -> V16Result<()> {
        if burn_num == 0 {
            return Ok(());
        }
        let account_claim_sum = Self::account_source_claim_bound_sum_num_static(account)?;
        if account_claim_sum == 0 {
            return Ok(());
        }
        if account_claim_sum < burn_num {
            return Err(V16Error::CounterUnderflow);
        }
        let mut d = 0;
        while d < V16_DOMAIN_COUNT && burn_num != 0 {
            let burnable = Self::source_claim_unliened_num(account, d)?;
            let burn = burnable.min(burn_num);
            if burn != 0 {
                account.source_claim_bound_num[d] -= burn;
                self.source_credit[d].positive_claim_bound_num = self.source_credit[d]
                    .positive_claim_bound_num
                    .checked_sub(burn)
                    .ok_or(V16Error::CounterUnderflow)?;
                self.source_credit[d].exact_positive_claim_num = self.source_credit[d]
                    .exact_positive_claim_num
                    .checked_sub(burn.min(self.source_credit[d].exact_positive_claim_num))
                    .ok_or(V16Error::CounterUnderflow)?;
                burn_num -= burn;
                self.recompute_source_credit_domain_after_mutation(d)?;
            }
            d += 1;
        }
        if burn_num != 0 {
            return Err(V16Error::LockActive);
        }
        Ok(())
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccountBSettlementChunkV16 {
    pub delta_b: u128,
    pub loss: u128,
    pub new_remainder: u128,
    pub remaining_after: u128,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RiskScoreV16 {
    pub certified_liq_deficit: u128,
    pub unsettled_b_loss_bound: u128,
    pub stale_loss_bound: u128,
    pub gross_risk_notional: u128,
    pub active_leg_count: u32,
}

impl RiskScoreV16 {
    pub fn strictly_reduces_from(self, before: Self) -> bool {
        self < before
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionlessProgressOutcomeV16 {
    AccountCurrent,
    AccountBChunk(AccountBSettlementChunkV16),
    ResidualBooked(BResidualBookingOutcomeV16),
    RecoveryDeclared(PermissionlessRecoveryReasonV16),
}

pub fn risk_notional_ceil(abs_pos_q: u128, price: u64) -> V16Result<u128> {
    if abs_pos_q == 0 {
        return Ok(0);
    }
    checked_mul_div_ceil_u256(
        U256::from_u128(abs_pos_q),
        U256::from_u128(price as u128),
        U256::from_u128(POS_SCALE),
    )
    .and_then(|v| v.try_into_u128())
    .ok_or(V16Error::ArithmeticOverflow)
}

pub fn account_equity(account: &PortfolioAccountV16) -> V16Result<i128> {
    validate_non_min_i128(account.pnl)?;
    validate_fee_credits(account.fee_credits)?;
    let capital = i128::try_from(account.capital).map_err(|_| V16Error::ArithmeticOverflow)?;
    let fee_debt =
        i128::try_from(fee_debt_u128(account)?).map_err(|_| V16Error::ArithmeticOverflow)?;
    capital
        .checked_add(account.pnl)
        .and_then(|v| v.checked_sub(fee_debt))
        .ok_or(V16Error::ArithmeticOverflow)
}

fn account_no_positive_credit_equity(account: &PortfolioAccountV16) -> V16Result<i128> {
    validate_non_min_i128(account.pnl)?;
    validate_fee_credits(account.fee_credits)?;
    let capital = i128::try_from(account.capital).map_err(|_| V16Error::ArithmeticOverflow)?;
    let fee_debt =
        i128::try_from(fee_debt_u128(account)?).map_err(|_| V16Error::ArithmeticOverflow)?;
    capital
        .checked_add(account.pnl.min(0))
        .and_then(|v| v.checked_sub(fee_debt))
        .ok_or(V16Error::ArithmeticOverflow)
}

fn account_no_positive_credit_equity_with_capital(
    account: &PortfolioAccountV16,
    capital_override: u128,
) -> V16Result<i128> {
    validate_non_min_i128(account.pnl)?;
    validate_fee_credits(account.fee_credits)?;
    let capital = i128::try_from(capital_override).map_err(|_| V16Error::ArithmeticOverflow)?;
    let fee_debt =
        i128::try_from(fee_debt_u128(account)?).map_err(|_| V16Error::ArithmeticOverflow)?;
    capital
        .checked_add(account.pnl.min(0))
        .and_then(|v| v.checked_sub(fee_debt))
        .ok_or(V16Error::ArithmeticOverflow)
}

fn ensure_initial_margin(account: &PortfolioAccountV16) -> V16Result<()> {
    if !account.health_cert.valid {
        return Err(V16Error::Stale);
    }
    let equity = account.health_cert.certified_equity;
    if equity < 0 || (equity as u128) < account.health_cert.certified_initial_req {
        return Err(V16Error::InvalidConfig);
    }
    Ok(())
}

fn ensure_no_positive_credit_initial_margin(account: &PortfolioAccountV16) -> V16Result<()> {
    let equity = account_no_positive_credit_equity(account)?;
    if equity < 0 || (equity as u128) < account.health_cert.certified_initial_req {
        return Err(V16Error::LockActive);
    }
    Ok(())
}

fn position_delta_increases_risk(
    account: &PortfolioAccountV16,
    asset_index: usize,
    delta_q: i128,
) -> V16Result<bool> {
    let current = signed_position(account.legs[asset_index]);
    let next = current
        .checked_add(delta_q)
        .ok_or(V16Error::ArithmeticOverflow)?;
    validate_basis_or_zero(next)?;
    Ok(next.unsigned_abs() > current.unsigned_abs())
}

fn margin_requirement(notional: u128, bps: u64, floor: u128) -> V16Result<u128> {
    if notional == 0 {
        return Ok(0);
    }
    let raw = wide_mul_div_floor_u128(notional, bps as u128, MAX_MARGIN_BPS as u128);
    Ok(raw.max(floor))
}

fn trade_notional_floor(size_q: u128, exec_price: u64) -> V16Result<u128> {
    if size_q == 0 {
        return Ok(0);
    }
    let (q, _) = mul_div_floor_u256_with_rem(
        U256::from_u128(size_q),
        U256::from_u128(exec_price as u128),
        U256::from_u128(POS_SCALE),
    );
    q.try_into_u128().ok_or(V16Error::ArithmeticOverflow)
}

fn checked_fee_bps(notional: u128, fee_bps: u64) -> V16Result<u128> {
    if notional == 0 || fee_bps == 0 {
        return Ok(0);
    }
    checked_mul_div_ceil_u256(
        U256::from_u128(notional),
        U256::from_u128(fee_bps as u128),
        U256::from_u128(MAX_MARGIN_BPS as u128),
    )
    .and_then(|v| v.try_into_u128())
    .ok_or(V16Error::ArithmeticOverflow)
}

fn checked_i128_mul(a: i128, b: i128) -> V16Result<i128> {
    let out = a.checked_mul(b).ok_or(V16Error::ArithmeticOverflow)?;
    validate_non_min_i128(out)?;
    Ok(out)
}

fn add_non_min_i128(a: i128, b: i128) -> V16Result<i128> {
    let out = a.checked_add(b).ok_or(V16Error::ArithmeticOverflow)?;
    validate_non_min_i128(out)?;
    Ok(out)
}

fn adjust_u128(current: u128, old: u128, new: u128) -> V16Result<u128> {
    if new >= old {
        current
            .checked_add(new - old)
            .ok_or(V16Error::ArithmeticOverflow)
    } else {
        current
            .checked_sub(old - new)
            .ok_or(V16Error::CounterUnderflow)
    }
}

fn encode_bool(value: bool) -> u8 {
    if value {
        1
    } else {
        0
    }
}

fn decode_bool(value: u8) -> V16Result<bool> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(V16Error::InvalidConfig),
    }
}

fn encode_side(value: SideV16) -> u8 {
    match value {
        SideV16::Long => 0,
        SideV16::Short => 1,
    }
}

fn decode_side(value: u8) -> V16Result<SideV16> {
    match value {
        0 => Ok(SideV16::Long),
        1 => Ok(SideV16::Short),
        _ => Err(V16Error::InvalidConfig),
    }
}

fn encode_side_mode(value: SideModeV16) -> u8 {
    match value {
        SideModeV16::Normal => 0,
        SideModeV16::DrainOnly => 1,
        SideModeV16::ResetPending => 2,
    }
}

fn decode_side_mode(value: u8) -> V16Result<SideModeV16> {
    match value {
        0 => Ok(SideModeV16::Normal),
        1 => Ok(SideModeV16::DrainOnly),
        2 => Ok(SideModeV16::ResetPending),
        _ => Err(V16Error::InvalidConfig),
    }
}

fn encode_asset_lifecycle(value: AssetLifecycleV16) -> u8 {
    match value {
        AssetLifecycleV16::Disabled => 0,
        AssetLifecycleV16::PendingActivation => 1,
        AssetLifecycleV16::Active => 2,
        AssetLifecycleV16::DrainOnly => 3,
        AssetLifecycleV16::Retired => 4,
        AssetLifecycleV16::Recovery => 5,
    }
}

fn decode_asset_lifecycle(value: u8) -> V16Result<AssetLifecycleV16> {
    match value {
        0 => Ok(AssetLifecycleV16::Disabled),
        1 => Ok(AssetLifecycleV16::PendingActivation),
        2 => Ok(AssetLifecycleV16::Active),
        3 => Ok(AssetLifecycleV16::DrainOnly),
        4 => Ok(AssetLifecycleV16::Retired),
        5 => Ok(AssetLifecycleV16::Recovery),
        _ => Err(V16Error::InvalidConfig),
    }
}

fn encode_market_mode(value: MarketModeV16) -> u8 {
    match value {
        MarketModeV16::Live => 0,
        MarketModeV16::Resolved => 1,
        MarketModeV16::Recovery => 2,
    }
}

fn decode_market_mode(value: u8) -> V16Result<MarketModeV16> {
    match value {
        0 => Ok(MarketModeV16::Live),
        1 => Ok(MarketModeV16::Resolved),
        2 => Ok(MarketModeV16::Recovery),
        _ => Err(V16Error::InvalidConfig),
    }
}

fn encode_backing_bucket_status(value: BackingBucketStatusV16) -> u8 {
    match value {
        BackingBucketStatusV16::Empty => 0,
        BackingBucketStatusV16::Fresh => 1,
        BackingBucketStatusV16::Expired => 2,
        BackingBucketStatusV16::Impaired => 3,
    }
}

fn decode_backing_bucket_status(value: u8) -> V16Result<BackingBucketStatusV16> {
    match value {
        0 => Ok(BackingBucketStatusV16::Empty),
        1 => Ok(BackingBucketStatusV16::Fresh),
        2 => Ok(BackingBucketStatusV16::Expired),
        3 => Ok(BackingBucketStatusV16::Impaired),
        _ => Err(V16Error::InvalidConfig),
    }
}

fn encode_recovery_reason(value: PermissionlessRecoveryReasonV16) -> u8 {
    match value {
        PermissionlessRecoveryReasonV16::BelowProgressFloor => 0,
        PermissionlessRecoveryReasonV16::BlockedSegmentHeadroomOrRepresentability => 1,
        PermissionlessRecoveryReasonV16::AccountBSettlementCannotProgress => 2,
        PermissionlessRecoveryReasonV16::BIndexHeadroomExhausted => 3,
        PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress => 4,
        PermissionlessRecoveryReasonV16::ExplicitLossOrDustAuditOverflow => 5,
        PermissionlessRecoveryReasonV16::OracleOrTargetUnavailableByAuthenticatedPolicy => 6,
        PermissionlessRecoveryReasonV16::CounterOrEpochOverflowDeclaredRecovery => 7,
    }
}

fn decode_recovery_reason(value: u8) -> V16Result<PermissionlessRecoveryReasonV16> {
    match value {
        0 => Ok(PermissionlessRecoveryReasonV16::BelowProgressFloor),
        1 => Ok(PermissionlessRecoveryReasonV16::BlockedSegmentHeadroomOrRepresentability),
        2 => Ok(PermissionlessRecoveryReasonV16::AccountBSettlementCannotProgress),
        3 => Ok(PermissionlessRecoveryReasonV16::BIndexHeadroomExhausted),
        4 => Ok(PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress),
        5 => Ok(PermissionlessRecoveryReasonV16::ExplicitLossOrDustAuditOverflow),
        6 => Ok(PermissionlessRecoveryReasonV16::OracleOrTargetUnavailableByAuthenticatedPolicy),
        7 => Ok(PermissionlessRecoveryReasonV16::CounterOrEpochOverflowDeclaredRecovery),
        _ => Err(V16Error::InvalidConfig),
    }
}

fn validate_basis_or_zero(basis_pos_q: i128) -> V16Result<()> {
    if basis_pos_q == 0 {
        Ok(())
    } else {
        validate_basis(basis_pos_q)
    }
}

fn signed_position(leg: PortfolioLegV16) -> i128 {
    if !leg.active {
        0
    } else {
        match leg.side {
            SideV16::Long => leg.basis_pos_q.unsigned_abs() as i128,
            SideV16::Short => -(leg.basis_pos_q.unsigned_abs() as i128),
        }
    }
}

fn opposite_side(side: SideV16) -> SideV16 {
    match side {
        SideV16::Long => SideV16::Short,
        SideV16::Short => SideV16::Long,
    }
}

fn fraction_ge(lhs_num: u128, lhs_den: u128, rhs_num: u128, rhs_den: u128) -> V16Result<bool> {
    if lhs_den == 0 || rhs_den == 0 {
        return Err(V16Error::InvalidConfig);
    }
    let lhs = U256::from_u128(lhs_num)
        .checked_mul(U256::from_u128(rhs_den))
        .ok_or(V16Error::ArithmeticOverflow)?;
    let rhs = U256::from_u128(rhs_num)
        .checked_mul(U256::from_u128(lhs_den))
        .ok_or(V16Error::ArithmeticOverflow)?;
    Ok(lhs >= rhs)
}

fn quarantine_remainder(remainder: &mut u128, dust: &mut u128) -> V16Result<()> {
    if *remainder == 0 {
        return Ok(());
    }
    let new_dust = dust
        .checked_add(*remainder)
        .ok_or(V16Error::ArithmeticOverflow)?;
    if new_dust >= SOCIAL_LOSS_DEN {
        return Err(V16Error::RecoveryRequired);
    }
    *dust = new_dust;
    *remainder = 0;
    Ok(())
}

fn validate_non_min_i128(v: i128) -> V16Result<()> {
    if v == i128::MIN {
        return Err(V16Error::ArithmeticOverflow);
    }
    Ok(())
}

fn validate_fee_credits(v: i128) -> V16Result<()> {
    validate_non_min_i128(v)?;
    if v > 0 {
        return Err(V16Error::InvalidLeg);
    }
    Ok(())
}

fn fee_debt_u128(account: &PortfolioAccountV16) -> V16Result<u128> {
    validate_fee_credits(account.fee_credits)?;
    Ok(account.fee_credits.unsigned_abs())
}

fn validate_basis(basis_pos_q: i128) -> V16Result<()> {
    if basis_pos_q == 0
        || basis_pos_q == i128::MIN
        || basis_pos_q.unsigned_abs() > MAX_POSITION_ABS_Q
    {
        return Err(V16Error::InvalidLeg);
    }
    Ok(())
}

fn validate_active_leg(leg: PortfolioLegV16) -> V16Result<()> {
    validate_non_min_i128(leg.k_snap)?;
    validate_non_min_i128(leg.f_snap)?;
    let current_loss_weight = if leg.basis_pos_q == 0 {
        0
    } else {
        validate_basis(leg.basis_pos_q)?;
        loss_weight_for_basis(leg.basis_pos_q.unsigned_abs(), leg.a_basis)?
    };
    if !(MIN_A_SIDE..=ADL_ONE).contains(&leg.a_basis)
        || leg.loss_weight == 0
        || leg.loss_weight < current_loss_weight
        || leg.loss_weight > SOCIAL_LOSS_DEN
        || leg.b_rem >= SOCIAL_LOSS_DEN
        || leg.b_epoch_snap != leg.epoch_snap
    {
        return Err(V16Error::InvalidLeg);
    }
    Ok(())
}

fn same_side_risk_reduction_or_flat_obligation(current: i128, next: i128) -> bool {
    current != 0
        && (next == 0 || current.signum() == next.signum())
        && next.unsigned_abs() < current.unsigned_abs()
}

fn loss_weight_for_basis(abs_basis_q: u128, a_basis: u128) -> V16Result<u128> {
    if a_basis == 0 {
        return Err(V16Error::InvalidLeg);
    }
    checked_mul_div_ceil_u256(
        U256::from_u128(abs_basis_q),
        U256::from_u128(SOCIAL_WEIGHT_SCALE),
        U256::from_u128(a_basis),
    )
    .and_then(|v| v.try_into_u128())
    .ok_or(V16Error::ArithmeticOverflow)
}

fn has_b_stale_leg(account: &PortfolioAccountV16) -> bool {
    account.legs.iter().any(|leg| leg.active && leg.b_stale)
}

fn account_b_loss_bound(account: &PortfolioAccountV16) -> V16Result<u128> {
    let mut bound = 0u128;
    for leg in account.legs.iter() {
        if leg.active && leg.b_stale {
            bound = bound
                .checked_add(leg.loss_weight)
                .ok_or(V16Error::ArithmeticOverflow)?;
        }
    }
    Ok(bound)
}
