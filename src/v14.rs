//! v14 account-local risk engine.
//!
//! This module implements the v14 slab-free engine surface: authenticated
//! portfolio accounts, bounded per-account refresh, lazy A/K/F/B settlement,
//! loss-senior fee handling, account-local cranks, residual B booking, dynamic
//! trade fees, liquidation progress checks, and resolved account close.

use crate::wide_math::{
    checked_mul_div_ceil_u256, floor_div_signed_conservative_i128, mul_div_floor_u256_with_rem,
    wide_mul_div_floor_u128, wide_signed_mul_div_floor_from_k_pair, U256,
};
use crate::{
    ADL_ONE, FUNDING_DEN, MAX_ACCOUNT_NOTIONAL, MAX_MARGIN_BPS, MAX_ORACLE_PRICE,
    MAX_POSITION_ABS_Q, MAX_PROTOCOL_FEE_ABS, MAX_TRADE_SIZE_Q, MAX_VAULT_TVL, MIN_A_SIDE,
    POS_SCALE, SOCIAL_LOSS_DEN, SOCIAL_WEIGHT_SCALE,
};

pub const V14_MAX_PORTFOLIO_ASSETS_N: usize = 16;
pub const V14_DOMAIN_COUNT: usize = V14_MAX_PORTFOLIO_ASSETS_N * 2;
pub const V14_LAYOUT_DISCRIMINATOR: u16 = 14;
pub const V14_ACCOUNT_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum V14Error {
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

pub type V14Result<T> = core::result::Result<T, V14Error>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HLockLaneV14 {
    HMin,
    HMax,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SideV14 {
    Long,
    Short,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SideModeV14 {
    Normal,
    DrainOnly,
    ResetPending,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarketModeV14 {
    Live,
    Resolved,
    Recovery,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionlessRecoveryReasonV14 {
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
pub struct ProvenanceHeaderV14 {
    pub market_group_id: [u8; 32],
    pub portfolio_account_id: [u8; 32],
    pub owner: [u8; 32],
    pub version: u16,
    pub layout_discriminator: u16,
}

impl ProvenanceHeaderV14 {
    pub const fn new(
        market_group_id: [u8; 32],
        portfolio_account_id: [u8; 32],
        owner: [u8; 32],
    ) -> Self {
        Self {
            market_group_id,
            portfolio_account_id,
            owner,
            version: V14_ACCOUNT_VERSION,
            layout_discriminator: V14_LAYOUT_DISCRIMINATOR,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct V14Config {
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
    pub public_b_chunk_atoms: u128,
    pub permissionless_recovery_enabled: bool,
    pub stale_certificate_penalty_enabled: bool,
    pub full_refresh_required_for_favorable_actions: bool,
    pub public_liveness_profile_crank_forward: bool,
}

impl V14Config {
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
            public_b_chunk_atoms: MAX_VAULT_TVL,
            permissionless_recovery_enabled: true,
            stale_certificate_penalty_enabled: true,
            full_refresh_required_for_favorable_actions: true,
            public_liveness_profile_crank_forward: true,
        }
    }

    fn ceil_div_u256_to_u128(n: U256, d: U256) -> V14Result<u128> {
        if d.is_zero() {
            return Err(V14Error::InvalidConfig);
        }
        let q = n.checked_div(d).ok_or(V14Error::InvalidConfig)?;
        let r = n.checked_rem(d).ok_or(V14Error::InvalidConfig)?;
        let q = if r.is_zero() {
            q
        } else {
            q.checked_add(U256::ONE).ok_or(V14Error::InvalidConfig)?
        };
        q.try_into_u128().ok_or(V14Error::InvalidConfig)
    }

    fn checked_mul_div_ceil_to_u128(a: u128, b: u128, d: u128) -> V14Result<u128> {
        checked_mul_div_ceil_u256(U256::from_u128(a), U256::from_u128(b), U256::from_u128(d))
            .and_then(|v| v.try_into_u128())
            .ok_or(V14Error::InvalidConfig)
    }

    fn solvency_envelope_total_for_notional(
        &self,
        n: u128,
        loss_budget_num: u128,
        loss_budget_den: u128,
        price_budget_bps: u128,
    ) -> V14Result<u128> {
        let loss = Self::checked_mul_div_ceil_to_u128(n, loss_budget_num, loss_budget_den)?;

        let worst_liq_multiplier = 10_000u128
            .checked_add(price_budget_bps)
            .ok_or(V14Error::InvalidConfig)?;
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

        loss.checked_add(liq_fee).ok_or(V14Error::InvalidConfig)
    }

    fn maintenance_requirement_for_notional(&self, n: u128) -> V14Result<u128> {
        let mm_prop = U256::from_u128(n)
            .checked_mul(U256::from_u128(self.maintenance_margin_bps as u128))
            .and_then(|v| v.checked_div(U256::from_u128(10_000)))
            .and_then(|v| v.try_into_u128())
            .ok_or(V14Error::InvalidConfig)?;
        Ok(core::cmp::max(mm_prop, self.min_nonzero_mm_req))
    }

    fn solvency_envelope_holds_for_notional(
        &self,
        n: u128,
        loss_budget_num: u128,
        loss_budget_den: u128,
        price_budget_bps: u128,
    ) -> V14Result<bool> {
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
    ) -> V14Result<bool> {
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
    ) -> V14Result<()> {
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
            steps = steps.checked_add(1).ok_or(V14Error::InvalidConfig)?;
            if steps > MAX_SOLVENCY_STEPS {
                return Err(V14Error::InvalidConfig);
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
                        return Err(V14Error::InvalidConfig);
                    }
                    if n == range_hi {
                        break;
                    }
                    n = n.checked_add(1).ok_or(V14Error::InvalidConfig)?;
                }
                continue;
            }

            let mid = range_lo + (range_hi - range_lo) / 2;
            if len + 2 > MAX_SOLVENCY_INTERVALS {
                return Err(V14Error::InvalidConfig);
            }
            stack[len] = (mid.checked_add(1).ok_or(V14Error::InvalidConfig)?, range_hi);
            stack[len + 1] = (range_lo, mid);
            len += 2;
        }

        Ok(())
    }

    fn validate_funding_headroom(&self, slots: u64) -> V14Result<()> {
        let max_signed = U256::from_u128(i128::MAX as u128);
        let headroom = U256::from_u128(ADL_ONE)
            .checked_mul(U256::from_u128(MAX_ORACLE_PRICE as u128))
            .and_then(|v| v.checked_mul(U256::from_u128(self.max_abs_funding_e9_per_slot as u128)))
            .and_then(|v| v.checked_mul(U256::from_u128(slots as u128)))
            .ok_or(V14Error::InvalidConfig)?;
        if headroom <= max_signed {
            Ok(())
        } else {
            Err(V14Error::InvalidConfig)
        }
    }

    fn validate_exact_solvency_envelope(&self) -> V14Result<()> {
        let price_budget_fast = (self.max_price_move_bps_per_slot as u128)
            .checked_mul(self.max_accrual_dt_slots as u128)
            .ok_or(V14Error::InvalidConfig)?;
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
            .ok_or(V14Error::InvalidConfig)?;
        let funding_budget_num = rate
            .checked_mul(dt)
            .and_then(|v| v.checked_mul(ten_thousand))
            .ok_or(V14Error::InvalidConfig)?;
        let loss_budget_num_wide = U256::from_u128(price_budget_bps)
            .checked_mul(funding_den)
            .and_then(|v| v.checked_add(funding_budget_num))
            .ok_or(V14Error::InvalidConfig)?;
        let loss_budget_den_wide = ten_thousand
            .checked_mul(funding_den)
            .ok_or(V14Error::InvalidConfig)?;

        let funding_budget_bps_ceil = Self::ceil_div_u256_to_u128(funding_budget_num, funding_den)?;
        let loss_budget_bps_ceil = price_budget_bps
            .checked_add(funding_budget_bps_ceil)
            .ok_or(V14Error::InvalidConfig)?;
        let worst_liq_budget_bps_ceil = Self::ceil_div_u256_to_u128(
            U256::from_u128(
                10_000u128
                    .checked_add(price_budget_bps)
                    .ok_or(V14Error::InvalidConfig)?,
            )
            .checked_mul(U256::from_u128(self.liquidation_fee_bps as u128))
            .ok_or(V14Error::InvalidConfig)?,
            ten_thousand,
        )?;
        let linear_budget_bps = loss_budget_bps_ceil
            .checked_add(worst_liq_budget_bps_ceil)
            .ok_or(V14Error::InvalidConfig)?;

        if self.maintenance_margin_bps == 10_000
            && loss_budget_bps_ceil == 10_000
            && worst_liq_budget_bps_ceil == 0
            && self.min_liquidation_abs == 0
        {
            return Ok(());
        }

        let loss_budget_num = loss_budget_num_wide
            .try_into_u128()
            .ok_or(V14Error::InvalidConfig)?;
        let loss_budget_den = loss_budget_den_wide
            .try_into_u128()
            .ok_or(V14Error::InvalidConfig)?;
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
            return Err(V14Error::InvalidConfig);
        }

        let floor_region_max = U256::from_u128(
            self.min_nonzero_mm_req
                .checked_add(1)
                .ok_or(V14Error::InvalidConfig)?,
        )
        .checked_mul(ten_thousand)
        .and_then(|v| v.checked_sub(U256::ONE))
        .and_then(|v| v.checked_div(U256::from_u128(self.maintenance_margin_bps as u128)))
        .and_then(|v| v.try_into_u128())
        .ok_or(V14Error::InvalidConfig)?;
        let floor_region_end = core::cmp::min(floor_region_max, domain_max);
        if floor_region_end != 0
            && !self.solvency_envelope_holds_for_notional(
                floor_region_end,
                loss_budget_num,
                loss_budget_den,
                price_budget_bps,
            )?
        {
            return Err(V14Error::InvalidConfig);
        }
        if floor_region_max >= domain_max {
            return Ok(());
        }

        let exact_start = floor_region_end
            .checked_add(1)
            .ok_or(V14Error::InvalidConfig)?;

        if linear_budget_bps < self.maintenance_margin_bps as u128 {
            let slope_gap = (self.maintenance_margin_bps as u128) - linear_budget_bps;
            let tail_for_linear = Self::ceil_div_u256_to_u128(
                U256::from_u128(3 * 10_000),
                U256::from_u128(slope_gap),
            )?;

            let loss_gap = (self.maintenance_margin_bps as u128)
                .checked_sub(loss_budget_bps_ceil)
                .ok_or(V14Error::InvalidConfig)?;
            let floor_fee_slack = self
                .min_liquidation_abs
                .checked_add(2)
                .ok_or(V14Error::InvalidConfig)?;
            let tail_for_fee_floor = Self::ceil_div_u256_to_u128(
                U256::from_u128(floor_fee_slack)
                    .checked_mul(ten_thousand)
                    .ok_or(V14Error::InvalidConfig)?,
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
            .ok_or(V14Error::InvalidConfig)?;
        let exact_tail = Self::ceil_div_u256_to_u128(
            U256::from_u128(capped_fee_slack)
                .checked_mul(ten_thousand)
                .ok_or(V14Error::InvalidConfig)?,
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

    pub fn validate_public_user_fund(&self) -> V14Result<()> {
        if self.max_portfolio_assets == 0
            || self.max_portfolio_assets as usize > V14_MAX_PORTFOLIO_ASSETS_N
        {
            return Err(V14Error::InvalidConfig);
        }
        if self.h_max == 0 || self.h_min > self.h_max {
            return Err(V14Error::InvalidConfig);
        }
        if self.min_nonzero_mm_req == 0 || self.min_nonzero_mm_req >= self.min_nonzero_im_req {
            return Err(V14Error::InvalidConfig);
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
            || self.public_b_chunk_atoms == 0
        {
            return Err(V14Error::InvalidConfig);
        }
        if !self.permissionless_recovery_enabled
            || !self.stale_certificate_penalty_enabled
            || !self.full_refresh_required_for_favorable_actions
            || !self.public_liveness_profile_crank_forward
        {
            return Err(V14Error::InvalidConfig);
        }
        self.validate_exact_solvency_envelope()
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssetStateV14 {
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
    pub mode_long: SideModeV14,
    pub mode_short: SideModeV14,
}

impl Default for AssetStateV14 {
    fn default() -> Self {
        Self {
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
            mode_long: SideModeV14::Normal,
            mode_short: SideModeV14::Normal,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PortfolioLegV14 {
    pub active: bool,
    pub side: SideV14,
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

impl PortfolioLegV14 {
    pub const EMPTY: Self = Self {
        active: false,
        side: SideV14::Long,
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

impl Default for PortfolioLegV14 {
    fn default() -> Self {
        Self::EMPTY
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct HealthCertV14 {
    pub certified_equity: i128,
    pub certified_initial_req: u128,
    pub certified_maintenance_req: u128,
    pub certified_liq_deficit: u128,
    pub certified_worst_case_loss: u128,
    pub cert_oracle_epoch: u64,
    pub cert_funding_epoch: u64,
    pub cert_risk_epoch: u64,
    pub active_bitmap_at_cert: u32,
    pub valid: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CloseProgressLedgerV14 {
    pub active: bool,
    pub finalized: bool,
    pub close_id: u64,
    pub asset_index: u8,
    pub domain_side: SideV14,
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

impl CloseProgressLedgerV14 {
    pub const EMPTY: Self = Self {
        active: false,
        finalized: false,
        close_id: 0,
        asset_index: 0,
        domain_side: SideV14::Long,
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
        self.active && !self.finalized && self.residual_remaining != 0
    }
}

impl Default for CloseProgressLedgerV14 {
    fn default() -> Self {
        Self::EMPTY
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PortfolioAccountV14 {
    pub provenance_header: ProvenanceHeaderV14,
    pub owner: [u8; 32],
    pub capital: u128,
    pub pnl: i128,
    pub reserved_pnl: u128,
    pub fee_credits: i128,
    pub last_fee_slot: u64,
    pub active_bitmap: u32,
    pub legs: [PortfolioLegV14; V14_MAX_PORTFOLIO_ASSETS_N],
    pub health_cert: HealthCertV14,
    pub stale_state: bool,
    pub b_stale_state: bool,
    pub rebalance_lock: bool,
    pub liquidation_lock: bool,
    pub close_progress: CloseProgressLedgerV14,
}

impl PortfolioAccountV14 {
    pub const fn empty(header: ProvenanceHeaderV14) -> Self {
        Self {
            provenance_header: header,
            owner: header.owner,
            capital: 0,
            pnl: 0,
            reserved_pnl: 0,
            fee_credits: 0,
            last_fee_slot: 0,
            active_bitmap: 0,
            legs: [PortfolioLegV14::EMPTY; V14_MAX_PORTFOLIO_ASSETS_N],
            health_cert: HealthCertV14 {
                certified_equity: 0,
                certified_initial_req: 0,
                certified_maintenance_req: 0,
                certified_liq_deficit: 0,
                certified_worst_case_loss: 0,
                cert_oracle_epoch: 0,
                cert_funding_epoch: 0,
                cert_risk_epoch: 0,
                active_bitmap_at_cert: 0,
                valid: false,
            },
            stale_state: false,
            b_stale_state: false,
            rebalance_lock: false,
            liquidation_lock: false,
            close_progress: CloseProgressLedgerV14::EMPTY,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarketGroupV14 {
    pub market_group_id: [u8; 32],
    pub config: V14Config,
    pub vault: u128,
    pub insurance: u128,
    pub c_tot: u128,
    pub pnl_pos_tot: u128,
    pub pnl_pos_bound_tot: u128,
    pub pnl_matured_pos_tot: u128,
    pub insurance_domain_budget: [u128; V14_DOMAIN_COUNT],
    pub insurance_domain_spent: [u128; V14_DOMAIN_COUNT],
    pub pending_domain_loss_barriers: [u64; V14_DOMAIN_COUNT],
    pub materialized_portfolio_count: u64,
    pub stale_certificate_count: u64,
    pub b_stale_account_count: u64,
    pub negative_pnl_account_count: u64,
    pub risk_epoch: u64,
    pub oracle_epoch: u64,
    pub funding_epoch: u64,
    pub slot_last: u64,
    pub current_slot: u64,
    pub assets: [AssetStateV14; V14_MAX_PORTFOLIO_ASSETS_N],
    pub bankruptcy_hlock_active: bool,
    pub threshold_stress_active: bool,
    pub active_bankrupt_close_present: bool,
    pub loss_stale_active: bool,
    pub recovery_reason: Option<PermissionlessRecoveryReasonV14>,
    pub mode: MarketModeV14,
    pub resolved_slot: u64,
    pub payout_snapshot: u128,
    pub payout_snapshot_pnl_pos_tot: u128,
    pub payout_snapshot_captured: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccrueAssetOutcomeV14 {
    pub dt: u64,
    pub price_move_active: bool,
    pub funding_active: bool,
    pub equity_active: bool,
    pub loss_stale_after: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TradeRequestV14 {
    pub asset_index: usize,
    pub size_q: u128,
    pub exec_price: u64,
    pub fee_bps: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TradeOutcomeV14 {
    pub fee_a: u128,
    pub fee_b: u128,
    pub notional: u128,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiquidationRequestV14 {
    pub asset_index: usize,
    pub close_q: u128,
    pub fee_bps: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiquidationOutcomeV14 {
    pub closed_q: u128,
    pub insurance_used: u128,
    pub residual_booked: u128,
    pub explicit_loss: u128,
    pub fee_charged: u128,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeadLegForfeitOutcomeV14 {
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
struct SupportLossApplicationV14 {
    support_consumed: u128,
    junior_face_burned: u128,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RebalanceRequestV14 {
    pub asset_index: usize,
    pub reduce_q: u128,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RebalanceOutcomeV14 {
    pub reduced_q: u128,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BResidualBookingOutcomeV14 {
    pub booked_loss: u128,
    pub explicit_loss: u128,
    pub delta_b: u128,
    pub remaining_after: u128,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuantityAdlOutcomeV14 {
    pub closed_q: u128,
    pub opposite_a_after: u128,
    pub reset_started: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionlessCrankActionV14 {
    Refresh,
    SettleB { asset_index: usize },
    Liquidate(LiquidationRequestV14),
    Recover(PermissionlessRecoveryReasonV14),
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PermissionlessCrankRequestV14 {
    pub now_slot: u64,
    pub asset_index: usize,
    pub effective_price: u64,
    pub funding_rate_e9: i128,
    pub action: PermissionlessCrankActionV14,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolvedCloseOutcomeV14 {
    ProgressOnly,
    Closed { payout: u128 },
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
pub struct V14PodU16 {
    pub bytes: [u8; 2],
}

impl V14PodU16 {
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
pub struct V14PodU32 {
    pub bytes: [u8; 4],
}

impl V14PodU32 {
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
pub struct V14PodU64 {
    pub bytes: [u8; 8],
}

impl V14PodU64 {
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
pub struct V14PodU128 {
    pub bytes: [u8; 16],
}

impl V14PodU128 {
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
pub struct V14PodI128 {
    pub bytes: [u8; 16],
}

impl V14PodI128 {
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
pub struct V14OptionalRecoveryReasonAccount {
    pub present: u8,
    pub value: u8,
}

impl V14OptionalRecoveryReasonAccount {
    pub fn from_runtime(value: Option<PermissionlessRecoveryReasonV14>) -> Self {
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

    pub fn try_to_runtime(self) -> V14Result<Option<PermissionlessRecoveryReasonV14>> {
        match self.present {
            0 if self.value == 0 => Ok(None),
            1 => Ok(Some(decode_recovery_reason(self.value)?)),
            _ => Err(V14Error::InvalidConfig),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
pub struct ProvenanceHeaderV14Account {
    pub market_group_id: [u8; 32],
    pub portfolio_account_id: [u8; 32],
    pub owner: [u8; 32],
    pub version: V14PodU16,
    pub layout_discriminator: V14PodU16,
}

impl ProvenanceHeaderV14Account {
    pub fn from_runtime(value: &ProvenanceHeaderV14) -> Self {
        Self {
            market_group_id: value.market_group_id,
            portfolio_account_id: value.portfolio_account_id,
            owner: value.owner,
            version: V14PodU16::new(value.version),
            layout_discriminator: V14PodU16::new(value.layout_discriminator),
        }
    }

    pub fn try_to_runtime(&self) -> V14Result<ProvenanceHeaderV14> {
        let out = ProvenanceHeaderV14 {
            market_group_id: self.market_group_id,
            portfolio_account_id: self.portfolio_account_id,
            owner: self.owner,
            version: self.version.get(),
            layout_discriminator: self.layout_discriminator.get(),
        };
        if out.version != V14_ACCOUNT_VERSION
            || out.layout_discriminator != V14_LAYOUT_DISCRIMINATOR
        {
            return Err(V14Error::ProvenanceMismatch);
        }
        Ok(out)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
pub struct V14ConfigAccount {
    pub max_portfolio_assets: u8,
    pub min_nonzero_mm_req: V14PodU128,
    pub min_nonzero_im_req: V14PodU128,
    pub h_min: V14PodU64,
    pub h_max: V14PodU64,
    pub maintenance_margin_bps: V14PodU64,
    pub initial_margin_bps: V14PodU64,
    pub max_trading_fee_bps: V14PodU64,
    pub liquidation_fee_bps: V14PodU64,
    pub liquidation_fee_cap: V14PodU128,
    pub min_liquidation_abs: V14PodU128,
    pub max_accrual_dt_slots: V14PodU64,
    pub max_abs_funding_e9_per_slot: V14PodU64,
    pub min_funding_lifetime_slots: V14PodU64,
    pub max_price_move_bps_per_slot: V14PodU64,
    pub max_account_b_settlement_chunks: V14PodU64,
    pub max_bankrupt_close_chunks: V14PodU64,
    pub public_b_chunk_atoms: V14PodU128,
    pub permissionless_recovery_enabled: u8,
    pub stale_certificate_penalty_enabled: u8,
    pub full_refresh_required_for_favorable_actions: u8,
    pub public_liveness_profile_crank_forward: u8,
}

impl V14ConfigAccount {
    pub fn from_runtime(value: &V14Config) -> Self {
        Self {
            max_portfolio_assets: value.max_portfolio_assets,
            min_nonzero_mm_req: V14PodU128::new(value.min_nonzero_mm_req),
            min_nonzero_im_req: V14PodU128::new(value.min_nonzero_im_req),
            h_min: V14PodU64::new(value.h_min),
            h_max: V14PodU64::new(value.h_max),
            maintenance_margin_bps: V14PodU64::new(value.maintenance_margin_bps),
            initial_margin_bps: V14PodU64::new(value.initial_margin_bps),
            max_trading_fee_bps: V14PodU64::new(value.max_trading_fee_bps),
            liquidation_fee_bps: V14PodU64::new(value.liquidation_fee_bps),
            liquidation_fee_cap: V14PodU128::new(value.liquidation_fee_cap),
            min_liquidation_abs: V14PodU128::new(value.min_liquidation_abs),
            max_accrual_dt_slots: V14PodU64::new(value.max_accrual_dt_slots),
            max_abs_funding_e9_per_slot: V14PodU64::new(value.max_abs_funding_e9_per_slot),
            min_funding_lifetime_slots: V14PodU64::new(value.min_funding_lifetime_slots),
            max_price_move_bps_per_slot: V14PodU64::new(value.max_price_move_bps_per_slot),
            max_account_b_settlement_chunks: V14PodU64::new(value.max_account_b_settlement_chunks),
            max_bankrupt_close_chunks: V14PodU64::new(value.max_bankrupt_close_chunks),
            public_b_chunk_atoms: V14PodU128::new(value.public_b_chunk_atoms),
            permissionless_recovery_enabled: encode_bool(value.permissionless_recovery_enabled),
            stale_certificate_penalty_enabled: encode_bool(value.stale_certificate_penalty_enabled),
            full_refresh_required_for_favorable_actions: encode_bool(
                value.full_refresh_required_for_favorable_actions,
            ),
            public_liveness_profile_crank_forward: encode_bool(
                value.public_liveness_profile_crank_forward,
            ),
        }
    }

    pub fn try_to_runtime(&self) -> V14Result<V14Config> {
        let out = V14Config {
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
            public_b_chunk_atoms: self.public_b_chunk_atoms.get(),
            permissionless_recovery_enabled: decode_bool(self.permissionless_recovery_enabled)?,
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
pub struct AssetStateV14Account {
    pub raw_oracle_target_price: V14PodU64,
    pub effective_price: V14PodU64,
    pub fund_px_last: V14PodU64,
    pub slot_last: V14PodU64,
    pub a_long: V14PodU128,
    pub a_short: V14PodU128,
    pub k_long: V14PodI128,
    pub k_short: V14PodI128,
    pub f_long_num: V14PodI128,
    pub f_short_num: V14PodI128,
    pub k_epoch_start_long: V14PodI128,
    pub k_epoch_start_short: V14PodI128,
    pub f_epoch_start_long_num: V14PodI128,
    pub f_epoch_start_short_num: V14PodI128,
    pub b_long_num: V14PodU128,
    pub b_short_num: V14PodU128,
    pub b_epoch_start_long_num: V14PodU128,
    pub b_epoch_start_short_num: V14PodU128,
    pub oi_eff_long_q: V14PodU128,
    pub oi_eff_short_q: V14PodU128,
    pub stored_pos_count_long: V14PodU64,
    pub stored_pos_count_short: V14PodU64,
    pub stale_account_count_long: V14PodU64,
    pub stale_account_count_short: V14PodU64,
    pub loss_weight_sum_long: V14PodU128,
    pub loss_weight_sum_short: V14PodU128,
    pub social_loss_remainder_long_num: V14PodU128,
    pub social_loss_remainder_short_num: V14PodU128,
    pub social_loss_dust_long_num: V14PodU128,
    pub social_loss_dust_short_num: V14PodU128,
    pub explicit_unallocated_loss_long: V14PodU128,
    pub explicit_unallocated_loss_short: V14PodU128,
    pub epoch_long: V14PodU64,
    pub epoch_short: V14PodU64,
    pub mode_long: u8,
    pub mode_short: u8,
}

impl AssetStateV14Account {
    pub fn from_runtime(value: &AssetStateV14) -> Self {
        Self {
            raw_oracle_target_price: V14PodU64::new(value.raw_oracle_target_price),
            effective_price: V14PodU64::new(value.effective_price),
            fund_px_last: V14PodU64::new(value.fund_px_last),
            slot_last: V14PodU64::new(value.slot_last),
            a_long: V14PodU128::new(value.a_long),
            a_short: V14PodU128::new(value.a_short),
            k_long: V14PodI128::new(value.k_long),
            k_short: V14PodI128::new(value.k_short),
            f_long_num: V14PodI128::new(value.f_long_num),
            f_short_num: V14PodI128::new(value.f_short_num),
            k_epoch_start_long: V14PodI128::new(value.k_epoch_start_long),
            k_epoch_start_short: V14PodI128::new(value.k_epoch_start_short),
            f_epoch_start_long_num: V14PodI128::new(value.f_epoch_start_long_num),
            f_epoch_start_short_num: V14PodI128::new(value.f_epoch_start_short_num),
            b_long_num: V14PodU128::new(value.b_long_num),
            b_short_num: V14PodU128::new(value.b_short_num),
            b_epoch_start_long_num: V14PodU128::new(value.b_epoch_start_long_num),
            b_epoch_start_short_num: V14PodU128::new(value.b_epoch_start_short_num),
            oi_eff_long_q: V14PodU128::new(value.oi_eff_long_q),
            oi_eff_short_q: V14PodU128::new(value.oi_eff_short_q),
            stored_pos_count_long: V14PodU64::new(value.stored_pos_count_long),
            stored_pos_count_short: V14PodU64::new(value.stored_pos_count_short),
            stale_account_count_long: V14PodU64::new(value.stale_account_count_long),
            stale_account_count_short: V14PodU64::new(value.stale_account_count_short),
            loss_weight_sum_long: V14PodU128::new(value.loss_weight_sum_long),
            loss_weight_sum_short: V14PodU128::new(value.loss_weight_sum_short),
            social_loss_remainder_long_num: V14PodU128::new(value.social_loss_remainder_long_num),
            social_loss_remainder_short_num: V14PodU128::new(value.social_loss_remainder_short_num),
            social_loss_dust_long_num: V14PodU128::new(value.social_loss_dust_long_num),
            social_loss_dust_short_num: V14PodU128::new(value.social_loss_dust_short_num),
            explicit_unallocated_loss_long: V14PodU128::new(value.explicit_unallocated_loss_long),
            explicit_unallocated_loss_short: V14PodU128::new(value.explicit_unallocated_loss_short),
            epoch_long: V14PodU64::new(value.epoch_long),
            epoch_short: V14PodU64::new(value.epoch_short),
            mode_long: encode_side_mode(value.mode_long),
            mode_short: encode_side_mode(value.mode_short),
        }
    }

    pub fn try_to_runtime(&self) -> V14Result<AssetStateV14> {
        let out = AssetStateV14 {
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
pub struct PortfolioLegV14Account {
    pub active: u8,
    pub side: u8,
    pub basis_pos_q: V14PodI128,
    pub a_basis: V14PodU128,
    pub k_snap: V14PodI128,
    pub f_snap: V14PodI128,
    pub epoch_snap: V14PodU64,
    pub loss_weight: V14PodU128,
    pub b_snap: V14PodU128,
    pub b_rem: V14PodU128,
    pub b_epoch_snap: V14PodU64,
    pub b_stale: u8,
    pub stale: u8,
}

impl PortfolioLegV14Account {
    pub fn from_runtime(value: &PortfolioLegV14) -> Self {
        Self {
            active: encode_bool(value.active),
            side: encode_side(value.side),
            basis_pos_q: V14PodI128::new(value.basis_pos_q),
            a_basis: V14PodU128::new(value.a_basis),
            k_snap: V14PodI128::new(value.k_snap),
            f_snap: V14PodI128::new(value.f_snap),
            epoch_snap: V14PodU64::new(value.epoch_snap),
            loss_weight: V14PodU128::new(value.loss_weight),
            b_snap: V14PodU128::new(value.b_snap),
            b_rem: V14PodU128::new(value.b_rem),
            b_epoch_snap: V14PodU64::new(value.b_epoch_snap),
            b_stale: encode_bool(value.b_stale),
            stale: encode_bool(value.stale),
        }
    }

    pub fn try_to_runtime(&self) -> V14Result<PortfolioLegV14> {
        let out = PortfolioLegV14 {
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
        } else if out != PortfolioLegV14::EMPTY {
            return Err(V14Error::HiddenLeg);
        }
        Ok(out)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
pub struct HealthCertV14Account {
    pub certified_equity: V14PodI128,
    pub certified_initial_req: V14PodU128,
    pub certified_maintenance_req: V14PodU128,
    pub certified_liq_deficit: V14PodU128,
    pub certified_worst_case_loss: V14PodU128,
    pub cert_oracle_epoch: V14PodU64,
    pub cert_funding_epoch: V14PodU64,
    pub cert_risk_epoch: V14PodU64,
    pub active_bitmap_at_cert: V14PodU32,
    pub valid: u8,
}

impl HealthCertV14Account {
    pub fn from_runtime(value: &HealthCertV14) -> Self {
        Self {
            certified_equity: V14PodI128::new(value.certified_equity),
            certified_initial_req: V14PodU128::new(value.certified_initial_req),
            certified_maintenance_req: V14PodU128::new(value.certified_maintenance_req),
            certified_liq_deficit: V14PodU128::new(value.certified_liq_deficit),
            certified_worst_case_loss: V14PodU128::new(value.certified_worst_case_loss),
            cert_oracle_epoch: V14PodU64::new(value.cert_oracle_epoch),
            cert_funding_epoch: V14PodU64::new(value.cert_funding_epoch),
            cert_risk_epoch: V14PodU64::new(value.cert_risk_epoch),
            active_bitmap_at_cert: V14PodU32::new(value.active_bitmap_at_cert),
            valid: encode_bool(value.valid),
        }
    }

    pub fn try_to_runtime(&self) -> V14Result<HealthCertV14> {
        let out = HealthCertV14 {
            certified_equity: self.certified_equity.get(),
            certified_initial_req: self.certified_initial_req.get(),
            certified_maintenance_req: self.certified_maintenance_req.get(),
            certified_liq_deficit: self.certified_liq_deficit.get(),
            certified_worst_case_loss: self.certified_worst_case_loss.get(),
            cert_oracle_epoch: self.cert_oracle_epoch.get(),
            cert_funding_epoch: self.cert_funding_epoch.get(),
            cert_risk_epoch: self.cert_risk_epoch.get(),
            active_bitmap_at_cert: self.active_bitmap_at_cert.get(),
            valid: decode_bool(self.valid)?,
        };
        validate_non_min_i128(out.certified_equity)?;
        Ok(out)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
pub struct CloseProgressLedgerV14Account {
    pub active: u8,
    pub finalized: u8,
    pub close_id: V14PodU64,
    pub asset_index: u8,
    pub domain_side: u8,
    pub gross_loss_at_close_start: V14PodU128,
    pub drift_reference_slot: V14PodU64,
    pub max_close_slot: V14PodU64,
    pub support_consumed: V14PodU128,
    pub junior_face_burned: V14PodU128,
    pub insurance_spent: V14PodU128,
    pub b_loss_booked: V14PodU128,
    pub explicit_loss_assigned: V14PodU128,
    pub quantity_adl_applied_q: V14PodU128,
    pub drift_consumed: V14PodU128,
    pub residual_remaining: V14PodU128,
}

impl CloseProgressLedgerV14Account {
    pub fn from_runtime(value: &CloseProgressLedgerV14) -> Self {
        Self {
            active: encode_bool(value.active),
            finalized: encode_bool(value.finalized),
            close_id: V14PodU64::new(value.close_id),
            asset_index: value.asset_index,
            domain_side: encode_side(value.domain_side),
            gross_loss_at_close_start: V14PodU128::new(value.gross_loss_at_close_start),
            drift_reference_slot: V14PodU64::new(value.drift_reference_slot),
            max_close_slot: V14PodU64::new(value.max_close_slot),
            support_consumed: V14PodU128::new(value.support_consumed),
            junior_face_burned: V14PodU128::new(value.junior_face_burned),
            insurance_spent: V14PodU128::new(value.insurance_spent),
            b_loss_booked: V14PodU128::new(value.b_loss_booked),
            explicit_loss_assigned: V14PodU128::new(value.explicit_loss_assigned),
            quantity_adl_applied_q: V14PodU128::new(value.quantity_adl_applied_q),
            drift_consumed: V14PodU128::new(value.drift_consumed),
            residual_remaining: V14PodU128::new(value.residual_remaining),
        }
    }

    pub fn try_to_runtime(&self) -> V14Result<CloseProgressLedgerV14> {
        Ok(CloseProgressLedgerV14 {
            active: decode_bool(self.active)?,
            finalized: decode_bool(self.finalized)?,
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
pub struct PortfolioAccountV14Account {
    pub provenance_header: ProvenanceHeaderV14Account,
    pub owner: [u8; 32],
    pub capital: V14PodU128,
    pub pnl: V14PodI128,
    pub reserved_pnl: V14PodU128,
    pub fee_credits: V14PodI128,
    pub last_fee_slot: V14PodU64,
    pub active_bitmap: V14PodU32,
    pub legs: [PortfolioLegV14Account; V14_MAX_PORTFOLIO_ASSETS_N],
    pub health_cert: HealthCertV14Account,
    pub stale_state: u8,
    pub b_stale_state: u8,
    pub rebalance_lock: u8,
    pub liquidation_lock: u8,
    pub close_progress: CloseProgressLedgerV14Account,
}

impl PortfolioAccountV14Account {
    #[cfg(not(target_os = "solana"))]
    pub fn try_empty(header: ProvenanceHeaderV14Account) -> V14Result<Self> {
        Ok(Self::from_runtime(&PortfolioAccountV14::empty(
            header.try_to_runtime()?,
        )))
    }

    #[cfg(not(target_os = "solana"))]
    pub fn from_runtime(value: &PortfolioAccountV14) -> Self {
        let mut legs = [PortfolioLegV14Account::default(); V14_MAX_PORTFOLIO_ASSETS_N];
        let mut i = 0;
        while i < V14_MAX_PORTFOLIO_ASSETS_N {
            legs[i] = PortfolioLegV14Account::from_runtime(&value.legs[i]);
            i += 1;
        }
        Self {
            provenance_header: ProvenanceHeaderV14Account::from_runtime(&value.provenance_header),
            owner: value.owner,
            capital: V14PodU128::new(value.capital),
            pnl: V14PodI128::new(value.pnl),
            reserved_pnl: V14PodU128::new(value.reserved_pnl),
            fee_credits: V14PodI128::new(value.fee_credits),
            last_fee_slot: V14PodU64::new(value.last_fee_slot),
            active_bitmap: V14PodU32::new(value.active_bitmap),
            legs,
            health_cert: HealthCertV14Account::from_runtime(&value.health_cert),
            stale_state: encode_bool(value.stale_state),
            b_stale_state: encode_bool(value.b_stale_state),
            rebalance_lock: encode_bool(value.rebalance_lock),
            liquidation_lock: encode_bool(value.liquidation_lock),
            close_progress: CloseProgressLedgerV14Account::from_runtime(&value.close_progress),
        }
    }

    #[cfg(not(target_os = "solana"))]
    pub fn try_to_runtime(&self) -> V14Result<PortfolioAccountV14> {
        let mut legs = [PortfolioLegV14::EMPTY; V14_MAX_PORTFOLIO_ASSETS_N];
        let mut i = 0;
        while i < V14_MAX_PORTFOLIO_ASSETS_N {
            legs[i] = self.legs[i].try_to_runtime()?;
            i += 1;
        }
        let out = PortfolioAccountV14 {
            provenance_header: self.provenance_header.try_to_runtime()?,
            owner: self.owner,
            capital: self.capital.get(),
            pnl: self.pnl.get(),
            reserved_pnl: self.reserved_pnl.get(),
            fee_credits: self.fee_credits.get(),
            last_fee_slot: self.last_fee_slot.get(),
            active_bitmap: self.active_bitmap.get(),
            legs,
            health_cert: self.health_cert.try_to_runtime()?,
            stale_state: decode_bool(self.stale_state)?,
            b_stale_state: decode_bool(self.b_stale_state)?,
            rebalance_lock: decode_bool(self.rebalance_lock)?,
            liquidation_lock: decode_bool(self.liquidation_lock)?,
            close_progress: self.close_progress.try_to_runtime()?,
        };
        if out.provenance_header.owner != out.owner {
            return Err(V14Error::ProvenanceMismatch);
        }
        validate_non_min_i128(out.pnl)?;
        validate_fee_credits(out.fee_credits)?;
        if out.reserved_pnl > out.pnl.max(0) as u128 {
            return Err(V14Error::InvalidLeg);
        }
        Ok(out)
    }

    #[cfg(not(target_os = "solana"))]
    pub fn validate_with_market(&self, market: &MarketGroupV14) -> V14Result<PortfolioAccountV14> {
        let out = self.try_to_runtime()?;
        market.validate_account_shape(&out)?;
        Ok(out)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
pub struct MarketGroupV14Account {
    pub market_group_id: [u8; 32],
    pub config: V14ConfigAccount,
    pub vault: V14PodU128,
    pub insurance: V14PodU128,
    pub c_tot: V14PodU128,
    pub pnl_pos_tot: V14PodU128,
    pub pnl_pos_bound_tot: V14PodU128,
    pub pnl_matured_pos_tot: V14PodU128,
    pub insurance_domain_budget: [V14PodU128; V14_DOMAIN_COUNT],
    pub insurance_domain_spent: [V14PodU128; V14_DOMAIN_COUNT],
    pub pending_domain_loss_barriers: [V14PodU64; V14_DOMAIN_COUNT],
    pub materialized_portfolio_count: V14PodU64,
    pub stale_certificate_count: V14PodU64,
    pub b_stale_account_count: V14PodU64,
    pub negative_pnl_account_count: V14PodU64,
    pub risk_epoch: V14PodU64,
    pub oracle_epoch: V14PodU64,
    pub funding_epoch: V14PodU64,
    pub slot_last: V14PodU64,
    pub current_slot: V14PodU64,
    pub assets: [AssetStateV14Account; V14_MAX_PORTFOLIO_ASSETS_N],
    pub bankruptcy_hlock_active: u8,
    pub threshold_stress_active: u8,
    pub active_bankrupt_close_present: u8,
    pub loss_stale_active: u8,
    pub recovery_reason: V14OptionalRecoveryReasonAccount,
    pub mode: u8,
    pub resolved_slot: V14PodU64,
    pub payout_snapshot: V14PodU128,
    pub payout_snapshot_pnl_pos_tot: V14PodU128,
    pub payout_snapshot_captured: u8,
}

impl MarketGroupV14Account {
    #[cfg(not(target_os = "solana"))]
    pub fn from_runtime(value: &MarketGroupV14) -> Self {
        let mut assets = [AssetStateV14Account::default(); V14_MAX_PORTFOLIO_ASSETS_N];
        let mut i = 0;
        while i < V14_MAX_PORTFOLIO_ASSETS_N {
            assets[i] = AssetStateV14Account::from_runtime(&value.assets[i]);
            i += 1;
        }
        let mut insurance_domain_budget = [V14PodU128::default(); V14_DOMAIN_COUNT];
        let mut insurance_domain_spent = [V14PodU128::default(); V14_DOMAIN_COUNT];
        let mut pending_domain_loss_barriers = [V14PodU64::default(); V14_DOMAIN_COUNT];
        let mut d = 0;
        while d < V14_DOMAIN_COUNT {
            insurance_domain_budget[d] = V14PodU128::new(value.insurance_domain_budget[d]);
            insurance_domain_spent[d] = V14PodU128::new(value.insurance_domain_spent[d]);
            pending_domain_loss_barriers[d] = V14PodU64::new(value.pending_domain_loss_barriers[d]);
            d += 1;
        }
        Self {
            market_group_id: value.market_group_id,
            config: V14ConfigAccount::from_runtime(&value.config),
            vault: V14PodU128::new(value.vault),
            insurance: V14PodU128::new(value.insurance),
            c_tot: V14PodU128::new(value.c_tot),
            pnl_pos_tot: V14PodU128::new(value.pnl_pos_tot),
            pnl_pos_bound_tot: V14PodU128::new(value.pnl_pos_bound_tot),
            pnl_matured_pos_tot: V14PodU128::new(value.pnl_matured_pos_tot),
            insurance_domain_budget,
            insurance_domain_spent,
            pending_domain_loss_barriers,
            materialized_portfolio_count: V14PodU64::new(value.materialized_portfolio_count),
            stale_certificate_count: V14PodU64::new(value.stale_certificate_count),
            b_stale_account_count: V14PodU64::new(value.b_stale_account_count),
            negative_pnl_account_count: V14PodU64::new(value.negative_pnl_account_count),
            risk_epoch: V14PodU64::new(value.risk_epoch),
            oracle_epoch: V14PodU64::new(value.oracle_epoch),
            funding_epoch: V14PodU64::new(value.funding_epoch),
            slot_last: V14PodU64::new(value.slot_last),
            current_slot: V14PodU64::new(value.current_slot),
            assets,
            bankruptcy_hlock_active: encode_bool(value.bankruptcy_hlock_active),
            threshold_stress_active: encode_bool(value.threshold_stress_active),
            active_bankrupt_close_present: encode_bool(value.active_bankrupt_close_present),
            loss_stale_active: encode_bool(value.loss_stale_active),
            recovery_reason: V14OptionalRecoveryReasonAccount::from_runtime(value.recovery_reason),
            mode: encode_market_mode(value.mode),
            resolved_slot: V14PodU64::new(value.resolved_slot),
            payout_snapshot: V14PodU128::new(value.payout_snapshot),
            payout_snapshot_pnl_pos_tot: V14PodU128::new(value.payout_snapshot_pnl_pos_tot),
            payout_snapshot_captured: encode_bool(value.payout_snapshot_captured),
        }
    }

    #[cfg(not(target_os = "solana"))]
    pub fn try_to_runtime(&self) -> V14Result<MarketGroupV14> {
        let mut assets = [AssetStateV14::default(); V14_MAX_PORTFOLIO_ASSETS_N];
        let mut i = 0;
        while i < V14_MAX_PORTFOLIO_ASSETS_N {
            assets[i] = self.assets[i].try_to_runtime()?;
            i += 1;
        }
        let mut insurance_domain_budget = [0u128; V14_DOMAIN_COUNT];
        let mut insurance_domain_spent = [0u128; V14_DOMAIN_COUNT];
        let mut pending_domain_loss_barriers = [0u64; V14_DOMAIN_COUNT];
        let mut d = 0;
        while d < V14_DOMAIN_COUNT {
            insurance_domain_budget[d] = self.insurance_domain_budget[d].get();
            insurance_domain_spent[d] = self.insurance_domain_spent[d].get();
            pending_domain_loss_barriers[d] = self.pending_domain_loss_barriers[d].get();
            d += 1;
        }
        let out = MarketGroupV14 {
            market_group_id: self.market_group_id,
            config: self.config.try_to_runtime()?,
            vault: self.vault.get(),
            insurance: self.insurance.get(),
            c_tot: self.c_tot.get(),
            pnl_pos_tot: self.pnl_pos_tot.get(),
            pnl_pos_bound_tot: self.pnl_pos_bound_tot.get(),
            pnl_matured_pos_tot: self.pnl_matured_pos_tot.get(),
            insurance_domain_budget,
            insurance_domain_spent,
            pending_domain_loss_barriers,
            materialized_portfolio_count: self.materialized_portfolio_count.get(),
            stale_certificate_count: self.stale_certificate_count.get(),
            b_stale_account_count: self.b_stale_account_count.get(),
            negative_pnl_account_count: self.negative_pnl_account_count.get(),
            risk_epoch: self.risk_epoch.get(),
            oracle_epoch: self.oracle_epoch.get(),
            funding_epoch: self.funding_epoch.get(),
            slot_last: self.slot_last.get(),
            current_slot: self.current_slot.get(),
            assets,
            bankruptcy_hlock_active: decode_bool(self.bankruptcy_hlock_active)?,
            threshold_stress_active: decode_bool(self.threshold_stress_active)?,
            active_bankrupt_close_present: decode_bool(self.active_bankrupt_close_present)?,
            loss_stale_active: decode_bool(self.loss_stale_active)?,
            recovery_reason: self.recovery_reason.try_to_runtime()?,
            mode: decode_market_mode(self.mode)?,
            resolved_slot: self.resolved_slot.get(),
            payout_snapshot: self.payout_snapshot.get(),
            payout_snapshot_pnl_pos_tot: self.payout_snapshot_pnl_pos_tot.get(),
            payout_snapshot_captured: decode_bool(self.payout_snapshot_captured)?,
        };
        out.assert_public_invariants()?;
        Ok(out)
    }

    #[cfg(not(target_os = "solana"))]
    pub fn validate(&self) -> V14Result<MarketGroupV14> {
        self.try_to_runtime()
    }
}

impl MarketGroupV14 {
    #[cfg(not(target_os = "solana"))]
    pub fn new(market_group_id: [u8; 32], config: V14Config) -> V14Result<Self> {
        config.validate_public_user_fund()?;
        Ok(Self {
            market_group_id,
            config,
            vault: 0,
            insurance: 0,
            c_tot: 0,
            pnl_pos_tot: 0,
            pnl_pos_bound_tot: 0,
            pnl_matured_pos_tot: 0,
            insurance_domain_budget: [MAX_VAULT_TVL; V14_DOMAIN_COUNT],
            insurance_domain_spent: [0; V14_DOMAIN_COUNT],
            pending_domain_loss_barriers: [0; V14_DOMAIN_COUNT],
            materialized_portfolio_count: 0,
            stale_certificate_count: 0,
            b_stale_account_count: 0,
            negative_pnl_account_count: 0,
            risk_epoch: 0,
            oracle_epoch: 0,
            funding_epoch: 0,
            slot_last: 0,
            current_slot: 0,
            assets: [AssetStateV14::default(); V14_MAX_PORTFOLIO_ASSETS_N],
            bankruptcy_hlock_active: false,
            threshold_stress_active: false,
            active_bankrupt_close_present: false,
            loss_stale_active: false,
            recovery_reason: None,
            mode: MarketModeV14::Live,
            resolved_slot: 0,
            payout_snapshot: 0,
            payout_snapshot_pnl_pos_tot: 0,
            payout_snapshot_captured: false,
        })
    }

    pub fn validate_portfolio_account_provenance(
        &self,
        account: &PortfolioAccountV14,
    ) -> V14Result<()> {
        let h = account.provenance_header;
        if h.market_group_id != self.market_group_id
            || h.owner != account.owner
            || h.version != V14_ACCOUNT_VERSION
            || h.layout_discriminator != V14_LAYOUT_DISCRIMINATOR
        {
            return Err(V14Error::ProvenanceMismatch);
        }
        Ok(())
    }

    fn validate_close_progress_ledger(&self, ledger: CloseProgressLedgerV14) -> V14Result<()> {
        if !ledger.active {
            if ledger != CloseProgressLedgerV14::EMPTY {
                return Err(V14Error::InvalidLeg);
            }
            return Ok(());
        }
        if ledger.close_id == 0
            || ledger.asset_index as usize >= self.config.max_portfolio_assets as usize
            || ledger.drift_reference_slot > ledger.max_close_slot
            || ledger.max_close_slot < ledger.drift_reference_slot
            || ledger.support_consumed > ledger.junior_face_burned
        {
            return Err(V14Error::InvalidLeg);
        }
        let progress = ledger
            .support_consumed
            .checked_add(ledger.insurance_spent)
            .and_then(|v| v.checked_add(ledger.b_loss_booked))
            .and_then(|v| v.checked_add(ledger.explicit_loss_assigned))
            .ok_or(V14Error::ArithmeticOverflow)?;
        let total_loss = ledger
            .gross_loss_at_close_start
            .checked_add(ledger.drift_consumed)
            .ok_or(V14Error::ArithmeticOverflow)?;
        if progress > total_loss || ledger.residual_remaining != total_loss - progress {
            return Err(V14Error::InvalidLeg);
        }
        if ledger.finalized && ledger.residual_remaining != 0 {
            return Err(V14Error::InvalidLeg);
        }
        if ledger.quantity_adl_applied_q != 0
            && (!ledger.finalized || ledger.residual_remaining != 0)
        {
            return Err(V14Error::InvalidLeg);
        }
        Ok(())
    }

    pub fn validate_account_shape(&self, account: &PortfolioAccountV14) -> V14Result<()> {
        self.validate_portfolio_account_provenance(account)?;
        validate_non_min_i128(account.pnl)?;
        validate_fee_credits(account.fee_credits)?;
        if account.reserved_pnl > account.pnl.max(0) as u128 {
            return Err(V14Error::InvalidLeg);
        }
        self.validate_close_progress_ledger(account.close_progress)?;

        let n = self.config.max_portfolio_assets as usize;
        for i in 0..V14_MAX_PORTFOLIO_ASSETS_N {
            let bit = ((account.active_bitmap >> i) & 1) != 0;
            let leg = account.legs[i];
            if i >= n {
                if bit || leg != PortfolioLegV14::default() {
                    return Err(V14Error::HiddenLeg);
                }
                continue;
            }

            if bit != leg.active {
                return Err(V14Error::HiddenLeg);
            }
            if !leg.active {
                if leg != PortfolioLegV14::EMPTY {
                    return Err(V14Error::HiddenLeg);
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
                    return Err(V14Error::InvalidLeg);
                }
            }
        }
        if account.close_progress.quantity_adl_applied_q != 0 {
            let i = account.close_progress.asset_index as usize;
            if i >= n || account.legs[i].active {
                return Err(V14Error::InvalidLeg);
            }
        }
        Ok(())
    }

    pub fn create_portfolio_account(&mut self, account: &PortfolioAccountV14) -> V14Result<()> {
        self.validate_account_shape(account)?;
        self.materialized_portfolio_count = self
            .materialized_portfolio_count
            .checked_add(1)
            .ok_or(V14Error::CounterOverflow)?;
        Ok(())
    }

    pub fn close_portfolio_account(&mut self, account: &PortfolioAccountV14) -> V14Result<()> {
        self.validate_account_shape(account)?;
        if account.active_bitmap != 0
            || account.capital != 0
            || account.pnl != 0
            || account.reserved_pnl != 0
            || account.fee_credits != 0
            || account.stale_state
            || account.b_stale_state
            || account.close_progress.active
        {
            return Err(V14Error::LockActive);
        }
        self.materialized_portfolio_count = self
            .materialized_portfolio_count
            .checked_sub(1)
            .ok_or(V14Error::CounterUnderflow)?;
        Ok(())
    }

    pub fn deposit_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV14,
        amount: u128,
    ) -> V14Result<()> {
        self.validate_account_shape(account)?;
        if amount == 0 {
            return Ok(());
        }
        account.capital = account
            .capital
            .checked_add(amount)
            .ok_or(V14Error::ArithmeticOverflow)?;
        self.c_tot = self
            .c_tot
            .checked_add(amount)
            .ok_or(V14Error::ArithmeticOverflow)?;
        self.vault = self
            .vault
            .checked_add(amount)
            .ok_or(V14Error::ArithmeticOverflow)?;
        account.health_cert.valid = false;
        self.assert_public_invariants()
    }

    pub fn settle_negative_pnl_from_principal(
        &mut self,
        account: &mut PortfolioAccountV14,
    ) -> V14Result<u128> {
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
        account.capital -= paid;
        self.c_tot = self
            .c_tot
            .checked_sub(paid)
            .ok_or(V14Error::CounterUnderflow)?;
        let paid_i128 = i128::try_from(paid).map_err(|_| V14Error::ArithmeticOverflow)?;
        let new_pnl = account
            .pnl
            .checked_add(paid_i128)
            .ok_or(V14Error::ArithmeticOverflow)?;
        self.set_account_pnl(account, new_pnl)?;
        if account.pnl < 0 {
            self.bankruptcy_hlock_active = true;
        }
        account.health_cert.valid = false;
        self.assert_public_invariants()?;
        Ok(paid)
    }

    pub fn charge_account_fee_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV14,
        requested_fee: u128,
    ) -> V14Result<u128> {
        if self.mode != MarketModeV14::Live {
            return Err(V14Error::LockActive);
        }
        self.charge_account_fee_after_loss_settlement(account, requested_fee)
    }

    fn charge_account_fee_after_loss_settlement(
        &mut self,
        account: &mut PortfolioAccountV14,
        requested_fee: u128,
    ) -> V14Result<u128> {
        self.settle_account_side_effects_not_atomic(account, self.config.public_b_chunk_atoms)?;
        if account.b_stale_state || has_b_stale_leg(account) {
            return Err(V14Error::BStale);
        }
        self.settle_negative_pnl_from_principal(account)?;
        if requested_fee == 0 || account.pnl < 0 {
            return Ok(0);
        }
        let charged = requested_fee.min(account.capital);
        if charged == 0 {
            return Ok(0);
        }
        account.capital -= charged;
        self.c_tot = self
            .c_tot
            .checked_sub(charged)
            .ok_or(V14Error::CounterUnderflow)?;
        self.insurance = self
            .insurance
            .checked_add(charged)
            .ok_or(V14Error::ArithmeticOverflow)?;
        account.health_cert.valid = false;
        self.assert_public_invariants()?;
        Ok(charged)
    }

    pub fn sync_account_fee_to_slot_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV14,
        now_slot: u64,
        fee_rate_per_slot: u128,
    ) -> V14Result<u128> {
        self.validate_account_shape(account)?;
        if now_slot < account.last_fee_slot {
            return Err(V14Error::Stale);
        }
        let nonflat = account.active_bitmap != 0;
        let fee_anchor = if self.mode == MarketModeV14::Live && nonflat && now_slot > self.slot_last
        {
            self.slot_last
        } else if self.mode == MarketModeV14::Resolved {
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
            .ok_or(V14Error::ArithmeticOverflow)?;
        let requested_fee = raw_fee.try_into_u128().unwrap_or(u128::MAX);
        let charged = self.charge_account_fee_after_loss_settlement(account, requested_fee)?;
        account.last_fee_slot = fee_anchor;
        Ok(charged)
    }

    pub fn convert_released_pnl_to_capital_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV14,
    ) -> V14Result<u128> {
        self.ensure_favorable_action_allowed(account)?;
        let pos = account.pnl.max(0) as u128;
        let released = if self.mode == MarketModeV14::Resolved {
            pos
        } else {
            pos.saturating_sub(account.reserved_pnl)
        };
        if released == 0 {
            return Ok(0);
        }
        let residual = self.residual();
        let junior_bound = self.junior_claim_bound();
        let converted = self.haircut_effective_support(released, residual, junior_bound)?;
        if converted == 0 {
            return Err(V14Error::LockActive);
        }
        let face_burn = self.face_claim_to_burn_for_support(converted, residual, junior_bound)?;
        let face_i128 = i128::try_from(face_burn).map_err(|_| V14Error::ArithmeticOverflow)?;
        let new_pnl = account
            .pnl
            .checked_sub(face_i128)
            .ok_or(V14Error::ArithmeticOverflow)?;
        self.set_account_pnl(account, new_pnl)?;
        account.capital = account
            .capital
            .checked_add(converted)
            .ok_or(V14Error::ArithmeticOverflow)?;
        self.c_tot = self
            .c_tot
            .checked_add(converted)
            .ok_or(V14Error::ArithmeticOverflow)?;
        self.pnl_matured_pos_tot = self.pnl_matured_pos_tot.saturating_sub(face_burn);
        account.health_cert.valid = false;
        self.assert_public_invariants()?;
        Ok(converted)
    }

    pub fn withdraw_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV14,
        amount: u128,
        effective_prices: &[u64; V14_MAX_PORTFOLIO_ASSETS_N],
    ) -> V14Result<()> {
        if amount == 0 {
            return Ok(());
        }
        self.settle_account_side_effects_not_atomic(account, self.config.public_b_chunk_atoms)?;
        self.full_account_refresh(account, effective_prices)?;
        let locked = self.h_lock_lane(Some(account), false)? == HLockLaneV14::HMax;
        if self.loss_stale_active && account.active_bitmap != 0 {
            return Err(V14Error::LockActive);
        }
        if self.account_has_target_effective_lag(account)? && account.active_bitmap != 0 {
            return Err(V14Error::LockActive);
        }
        self.settle_negative_pnl_from_principal(account)?;
        if account.pnl < 0 || amount > account.capital {
            return Err(V14Error::LockActive);
        }
        let post_capital = account.capital - amount;
        let equity_after = if locked {
            account_no_positive_credit_equity_with_capital(account, post_capital)?
        } else {
            self.account_haircut_equity_with_capital(account, post_capital)?
        };
        if equity_after < 0 {
            return Err(V14Error::InvalidConfig);
        }
        let equity_after_u = equity_after as u128;
        if equity_after_u < account.health_cert.certified_initial_req {
            return Err(V14Error::InvalidConfig);
        }
        account.capital = post_capital;
        self.c_tot = self
            .c_tot
            .checked_sub(amount)
            .ok_or(V14Error::CounterUnderflow)?;
        self.vault = self
            .vault
            .checked_sub(amount)
            .ok_or(V14Error::CounterUnderflow)?;
        account.health_cert.valid = false;
        self.assert_public_invariants()
    }

    pub fn mark_account_stale(&mut self, account: &mut PortfolioAccountV14) -> V14Result<()> {
        self.validate_portfolio_account_provenance(account)?;
        if !account.stale_state {
            account.stale_state = true;
            account.health_cert.valid = false;
            self.stale_certificate_count = self
                .stale_certificate_count
                .checked_add(1)
                .ok_or(V14Error::CounterOverflow)?;
        }
        Ok(())
    }

    pub fn clear_account_stale(&mut self, account: &mut PortfolioAccountV14) -> V14Result<()> {
        self.validate_portfolio_account_provenance(account)?;
        if account.stale_state {
            account.stale_state = false;
            self.stale_certificate_count = self
                .stale_certificate_count
                .checked_sub(1)
                .ok_or(V14Error::CounterUnderflow)?;
        }
        Ok(())
    }

    pub fn mark_account_b_stale(&mut self, account: &mut PortfolioAccountV14) -> V14Result<()> {
        self.validate_portfolio_account_provenance(account)?;
        if !account.b_stale_state {
            account.b_stale_state = true;
            account.health_cert.valid = false;
            self.b_stale_account_count = self
                .b_stale_account_count
                .checked_add(1)
                .ok_or(V14Error::CounterOverflow)?;
        }
        Ok(())
    }

    pub fn clear_account_b_stale(&mut self, account: &mut PortfolioAccountV14) -> V14Result<()> {
        self.validate_portfolio_account_provenance(account)?;
        if has_b_stale_leg(account) {
            return Err(V14Error::BStale);
        }
        if account.b_stale_state {
            account.b_stale_state = false;
            self.b_stale_account_count = self
                .b_stale_account_count
                .checked_sub(1)
                .ok_or(V14Error::CounterUnderflow)?;
        }
        Ok(())
    }

    pub fn attach_leg(
        &mut self,
        account: &mut PortfolioAccountV14,
        asset_index: usize,
        side: SideV14,
        basis_pos_q: i128,
    ) -> V14Result<()> {
        self.validate_portfolio_account_provenance(account)?;
        if asset_index >= self.config.max_portfolio_assets as usize {
            return Err(V14Error::InvalidLeg);
        }
        if self.has_pending_domain_loss_barrier(asset_index, side)? {
            return Err(V14Error::LockActive);
        }
        if account.legs[asset_index].active || ((account.active_bitmap >> asset_index) & 1) != 0 {
            return Err(V14Error::InvalidLeg);
        }
        validate_basis(basis_pos_q)?;

        let asset = self.assets[asset_index];
        let (a_basis, k_snap, f_snap, b_snap, epoch_snap) = match side {
            SideV14::Long => (
                asset.a_long,
                asset.k_long,
                asset.f_long_num,
                asset.b_long_num,
                asset.epoch_long,
            ),
            SideV14::Short => (
                asset.a_short,
                asset.k_short,
                asset.f_short_num,
                asset.b_short_num,
                asset.epoch_short,
            ),
        };
        if !(MIN_A_SIDE..=ADL_ONE).contains(&a_basis) {
            return Err(V14Error::InvalidLeg);
        }
        let loss_weight = loss_weight_for_basis(basis_pos_q.unsigned_abs(), a_basis)?;
        if loss_weight == 0 {
            return Err(V14Error::InvalidLeg);
        }

        let asset = &mut self.assets[asset_index];
        match side {
            SideV14::Long => {
                asset.stored_pos_count_long = asset
                    .stored_pos_count_long
                    .checked_add(1)
                    .ok_or(V14Error::CounterOverflow)?;
                asset.oi_eff_long_q = asset
                    .oi_eff_long_q
                    .checked_add(basis_pos_q.unsigned_abs())
                    .ok_or(V14Error::ArithmeticOverflow)?;
                asset.loss_weight_sum_long = asset
                    .loss_weight_sum_long
                    .checked_add(loss_weight)
                    .ok_or(V14Error::ArithmeticOverflow)?;
            }
            SideV14::Short => {
                asset.stored_pos_count_short = asset
                    .stored_pos_count_short
                    .checked_add(1)
                    .ok_or(V14Error::CounterOverflow)?;
                asset.oi_eff_short_q = asset
                    .oi_eff_short_q
                    .checked_add(basis_pos_q.unsigned_abs())
                    .ok_or(V14Error::ArithmeticOverflow)?;
                asset.loss_weight_sum_short = asset
                    .loss_weight_sum_short
                    .checked_add(loss_weight)
                    .ok_or(V14Error::ArithmeticOverflow)?;
            }
        }
        account.legs[asset_index] = PortfolioLegV14 {
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
        account: &mut PortfolioAccountV14,
        asset_index: usize,
    ) -> V14Result<()> {
        self.validate_account_shape(account)?;
        if asset_index >= self.config.max_portfolio_assets as usize {
            return Err(V14Error::InvalidLeg);
        }
        let leg = account.legs[asset_index];
        if !leg.active || leg.b_stale || leg.stale {
            return Err(V14Error::InvalidLeg);
        }
        if account.close_progress.has_pending_residual() {
            return Err(V14Error::LockActive);
        }
        if self.has_pending_domain_loss_barrier(asset_index, leg.side)? {
            return Err(V14Error::LockActive);
        }
        let asset = &mut self.assets[asset_index];
        let prior_reset_epoch = match leg.side {
            SideV14::Long => {
                asset.mode_long == SideModeV14::ResetPending
                    && leg.epoch_snap.checked_add(1) == Some(asset.epoch_long)
            }
            SideV14::Short => {
                asset.mode_short == SideModeV14::ResetPending
                    && leg.epoch_snap.checked_add(1) == Some(asset.epoch_short)
            }
        };
        match leg.side {
            SideV14::Long => {
                asset.stored_pos_count_long = asset
                    .stored_pos_count_long
                    .checked_sub(1)
                    .ok_or(V14Error::CounterUnderflow)?;
                if !prior_reset_epoch {
                    asset.oi_eff_long_q = asset
                        .oi_eff_long_q
                        .checked_sub(leg.basis_pos_q.unsigned_abs())
                        .ok_or(V14Error::CounterUnderflow)?;
                    asset.loss_weight_sum_long = asset
                        .loss_weight_sum_long
                        .checked_sub(leg.loss_weight)
                        .ok_or(V14Error::CounterUnderflow)?;
                }
            }
            SideV14::Short => {
                asset.stored_pos_count_short = asset
                    .stored_pos_count_short
                    .checked_sub(1)
                    .ok_or(V14Error::CounterUnderflow)?;
                if !prior_reset_epoch {
                    asset.oi_eff_short_q = asset
                        .oi_eff_short_q
                        .checked_sub(leg.basis_pos_q.unsigned_abs())
                        .ok_or(V14Error::CounterUnderflow)?;
                    asset.loss_weight_sum_short = asset
                        .loss_weight_sum_short
                        .checked_sub(leg.loss_weight)
                        .ok_or(V14Error::CounterUnderflow)?;
                }
            }
        }
        account.legs[asset_index] = PortfolioLegV14::EMPTY;
        account.active_bitmap &= !(1u32 << asset_index);
        account.health_cert.valid = false;
        self.validate_account_shape(account)
    }

    pub fn mark_leg_b_stale(
        &mut self,
        account: &mut PortfolioAccountV14,
        asset_index: usize,
    ) -> V14Result<()> {
        self.validate_account_shape(account)?;
        if asset_index >= self.config.max_portfolio_assets as usize
            || !account.legs[asset_index].active
        {
            return Err(V14Error::InvalidLeg);
        }
        account.legs[asset_index].b_stale = true;
        self.mark_account_b_stale(account)
    }

    pub fn h_lock_lane(
        &self,
        account: Option<&PortfolioAccountV14>,
        instruction_bankruptcy_candidate: bool,
    ) -> V14Result<HLockLaneV14> {
        if let Some(account) = account {
            self.validate_portfolio_account_provenance(account)?;
            if account.stale_state || account.b_stale_state {
                return Ok(HLockLaneV14::HMax);
            }
            if account.close_progress.has_pending_residual() {
                return Ok(HLockLaneV14::HMax);
            }
            if self.account_touches_pending_domain_loss_barrier(account)? {
                return Ok(HLockLaneV14::HMax);
            }
        }

        if self.threshold_stress_active
            || self.bankruptcy_hlock_active
            || instruction_bankruptcy_candidate
            || self.loss_stale_active
            || self.active_bankrupt_close_present
        {
            return Ok(HLockLaneV14::HMax);
        }

        Ok(HLockLaneV14::HMin)
    }

    pub fn select_h_lock(
        &self,
        account: Option<&PortfolioAccountV14>,
        instruction_bankruptcy_candidate: bool,
    ) -> V14Result<u64> {
        match self.h_lock_lane(account, instruction_bankruptcy_candidate)? {
            HLockLaneV14::HMin => Ok(self.config.h_min),
            HLockLaneV14::HMax => Ok(self.config.h_max),
        }
    }

    fn asset_has_target_effective_lag(&self, asset_index: usize) -> V14Result<bool> {
        if asset_index >= self.config.max_portfolio_assets as usize {
            return Err(V14Error::InvalidLeg);
        }
        let asset = self.assets[asset_index];
        Ok(asset.raw_oracle_target_price != asset.effective_price)
    }

    fn account_has_target_effective_lag(&self, account: &PortfolioAccountV14) -> V14Result<bool> {
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
        account: &mut PortfolioAccountV14,
        effective_prices: &[u64; V14_MAX_PORTFOLIO_ASSETS_N],
    ) -> V14Result<HealthCertV14> {
        self.validate_account_shape(account)?;
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
            return Err(V14Error::BStale);
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
                return Err(V14Error::InvalidConfig);
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
                .ok_or(V14Error::ArithmeticOverflow)?;
            maintenance_req = maintenance_req
                .checked_add(leg_maintenance)
                .ok_or(V14Error::ArithmeticOverflow)?;
            worst_case_loss = worst_case_loss
                .checked_add(risk_notional)
                .ok_or(V14Error::ArithmeticOverflow)?;
        }

        let equity = self.account_haircut_equity(account)?;
        let certified_liq_deficit = if equity < 0 {
            equity.unsigned_abs()
        } else {
            let e = equity as u128;
            maintenance_req.saturating_sub(e)
        };
        let cert = HealthCertV14 {
            certified_equity: equity,
            certified_initial_req: initial_req,
            certified_maintenance_req: maintenance_req,
            certified_liq_deficit,
            certified_worst_case_loss: worst_case_loss,
            cert_oracle_epoch: self.oracle_epoch,
            cert_funding_epoch: self.funding_epoch,
            cert_risk_epoch: self.risk_epoch,
            active_bitmap_at_cert: account.active_bitmap,
            valid: true,
        };
        account.health_cert = cert;
        Ok(cert)
    }

    pub fn ensure_favorable_action_allowed(&self, account: &PortfolioAccountV14) -> V14Result<()> {
        self.validate_account_shape(account)?;
        if self.h_lock_lane(Some(account), false)? == HLockLaneV14::HMax {
            return Err(V14Error::LockActive);
        }
        if !account.health_cert.valid
            || account.health_cert.cert_oracle_epoch != self.oracle_epoch
            || account.health_cert.cert_funding_epoch != self.funding_epoch
            || account.health_cert.cert_risk_epoch != self.risk_epoch
            || account.health_cert.active_bitmap_at_cert != account.active_bitmap
        {
            return Err(V14Error::Stale);
        }
        if self.account_has_target_effective_lag(account)? {
            return Err(V14Error::LockActive);
        }
        Ok(())
    }

    pub fn account_b_settlement_chunk(
        &self,
        account: &PortfolioAccountV14,
        asset_index: usize,
        endpoint_delta_budget: u128,
    ) -> V14Result<AccountBSettlementChunkV14> {
        self.validate_account_shape(account)?;
        if asset_index >= self.config.max_portfolio_assets as usize {
            return Err(V14Error::InvalidLeg);
        }
        let leg = account.legs[asset_index];
        if !leg.active {
            return Err(V14Error::InvalidLeg);
        }
        let target = self.b_target_for_leg(asset_index, leg)?;
        if target < leg.b_snap {
            return Err(V14Error::RecoveryRequired);
        }
        let b_remaining = target - leg.b_snap;
        if b_remaining == 0 {
            return Ok(AccountBSettlementChunkV14 {
                delta_b: 0,
                loss: 0,
                new_remainder: leg.b_rem,
                remaining_after: 0,
            });
        }
        if leg.loss_weight == 0 || endpoint_delta_budget == 0 {
            return Err(V14Error::RecoveryRequired);
        }

        let limit = self.config.public_b_chunk_atoms;
        let max_num = limit
            .checked_add(1)
            .and_then(|v| v.checked_mul(SOCIAL_LOSS_DEN))
            .and_then(|v| v.checked_sub(1))
            .ok_or(V14Error::ArithmeticOverflow)?;
        if leg.b_rem > max_num {
            return Err(V14Error::RecoveryRequired);
        }
        let max_delta_by_loss = (max_num - leg.b_rem) / leg.loss_weight;
        let delta_b = b_remaining
            .min(max_delta_by_loss)
            .min(endpoint_delta_budget);
        if delta_b == 0 {
            return Err(V14Error::RecoveryRequired);
        }
        let num = leg
            .loss_weight
            .checked_mul(delta_b)
            .and_then(|v| v.checked_add(leg.b_rem))
            .ok_or(V14Error::ArithmeticOverflow)?;
        let loss = num / SOCIAL_LOSS_DEN;
        let new_remainder = num % SOCIAL_LOSS_DEN;
        Ok(AccountBSettlementChunkV14 {
            delta_b,
            loss,
            new_remainder,
            remaining_after: b_remaining - delta_b,
        })
    }

    pub fn settle_account_b_chunk(
        &mut self,
        account: &mut PortfolioAccountV14,
        asset_index: usize,
        endpoint_delta_budget: u128,
    ) -> V14Result<AccountBSettlementChunkV14> {
        let chunk = self.account_b_settlement_chunk(account, asset_index, endpoint_delta_budget)?;
        if chunk.delta_b == 0 {
            if !has_b_stale_leg(account) {
                self.clear_account_b_stale(account)?;
            }
            return Ok(chunk);
        }
        let old_pnl = account.pnl;
        let loss_i128 = i128::try_from(chunk.loss).map_err(|_| V14Error::ArithmeticOverflow)?;
        let new_pnl = old_pnl
            .checked_sub(loss_i128)
            .ok_or(V14Error::ArithmeticOverflow)?;

        {
            let leg = &mut account.legs[asset_index];
            leg.b_snap = leg
                .b_snap
                .checked_add(chunk.delta_b)
                .ok_or(V14Error::ArithmeticOverflow)?;
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
        account: &mut PortfolioAccountV14,
        b_delta_budget: u128,
    ) -> V14Result<PermissionlessProgressOutcomeV14> {
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
                    return Ok(PermissionlessProgressOutcomeV14::AccountBChunk(chunk));
                }
            }
        }
        self.settle_negative_pnl_from_principal(account)?;
        account.health_cert.valid = false;
        Ok(PermissionlessProgressOutcomeV14::AccountCurrent)
    }

    pub fn accrue_asset_to_not_atomic(
        &mut self,
        asset_index: usize,
        now_slot: u64,
        effective_price: u64,
        funding_rate_e9: i128,
        protective_progress_committed: bool,
    ) -> V14Result<AccrueAssetOutcomeV14> {
        if self.mode != MarketModeV14::Live {
            return Err(V14Error::LockActive);
        }
        if asset_index >= self.config.max_portfolio_assets as usize
            || effective_price == 0
            || effective_price > MAX_ORACLE_PRICE
            || funding_rate_e9.unsigned_abs() > self.config.max_abs_funding_e9_per_slot as u128
            || now_slot < self.current_slot
            || now_slot < self.assets[asset_index].slot_last
        {
            return Err(V14Error::InvalidConfig);
        }
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
                return Err(V14Error::NonProgress);
            }
            let price_diff = effective_price.abs_diff(old.effective_price) as u128;
            let lhs = price_diff
                .checked_mul(MAX_MARGIN_BPS as u128)
                .ok_or(V14Error::ArithmeticOverflow)?;
            let rhs = (self.config.max_price_move_bps_per_slot as u128)
                .checked_mul(segment_dt as u128)
                .and_then(|v| v.checked_mul(old.effective_price as u128))
                .ok_or(V14Error::ArithmeticOverflow)?;
            if lhs > rhs {
                return Err(V14Error::RecoveryRequired);
            }
            if !protective_progress_committed {
                return Err(V14Error::NonProgress);
            }
        }

        let price_delta = effective_price as i128 - old.effective_price as i128;
        let k_delta = checked_i128_mul(price_delta, ADL_ONE as i128)?;
        let funding_delta = if funding_active {
            let n = funding_rate_e9
                .checked_mul(segment_dt as i128)
                .and_then(|v| v.checked_mul(effective_price as i128))
                .ok_or(V14Error::ArithmeticOverflow)?;
            floor_div_signed_conservative_i128(n, FUNDING_DEN)
                .checked_mul(ADL_ONE as i128)
                .ok_or(V14Error::ArithmeticOverflow)?
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
            .ok_or(V14Error::ArithmeticOverflow)?;
        self.current_slot = now_slot;
        self.slot_last = asset.slot_last;
        self.loss_stale_active = asset.slot_last < now_slot;
        if price_move_active {
            self.oracle_epoch = self
                .oracle_epoch
                .checked_add(1)
                .ok_or(V14Error::CounterOverflow)?;
        }
        if funding_active {
            self.funding_epoch = self
                .funding_epoch
                .checked_add(1)
                .ok_or(V14Error::CounterOverflow)?;
        }
        self.assert_public_invariants()?;
        Ok(AccrueAssetOutcomeV14 {
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
        long_account: &mut PortfolioAccountV14,
        short_account: &mut PortfolioAccountV14,
        request: TradeRequestV14,
        effective_prices: &[u64; V14_MAX_PORTFOLIO_ASSETS_N],
    ) -> V14Result<TradeOutcomeV14> {
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
        long_account: &mut PortfolioAccountV14,
        short_account: &mut PortfolioAccountV14,
        request: TradeRequestV14,
        effective_prices: &[u64; V14_MAX_PORTFOLIO_ASSETS_N],
    ) -> V14Result<TradeOutcomeV14> {
        self.execute_trade_with_fee_inner(long_account, short_account, request, effective_prices)
    }

    fn execute_trade_with_fee_inner(
        &mut self,
        long_account: &mut PortfolioAccountV14,
        short_account: &mut PortfolioAccountV14,
        request: TradeRequestV14,
        effective_prices: &[u64; V14_MAX_PORTFOLIO_ASSETS_N],
    ) -> V14Result<TradeOutcomeV14> {
        if request.asset_index >= self.config.max_portfolio_assets as usize
            || request.size_q == 0
            || request.size_q > MAX_TRADE_SIZE_Q
            || request.exec_price == 0
            || request.exec_price > MAX_ORACLE_PRICE
            || request.fee_bps > self.config.max_trading_fee_bps
        {
            return Err(V14Error::InvalidConfig);
        }
        if self.mode != MarketModeV14::Live {
            return Err(V14Error::LockActive);
        }
        self.settle_account_side_effects_not_atomic(
            long_account,
            self.config.public_b_chunk_atoms,
        )?;
        self.settle_account_side_effects_not_atomic(
            short_account,
            self.config.public_b_chunk_atoms,
        )?;
        self.full_account_refresh(long_account, effective_prices)?;
        self.full_account_refresh(short_account, effective_prices)?;

        let long_delta =
            i128::try_from(request.size_q).map_err(|_| V14Error::ArithmeticOverflow)?;
        let short_delta = long_delta
            .checked_neg()
            .ok_or(V14Error::ArithmeticOverflow)?;
        let locked = self.h_lock_lane(Some(long_account), false)? == HLockLaneV14::HMax
            || self.h_lock_lane(Some(short_account), false)? == HLockLaneV14::HMax;
        let risk_increasing =
            position_delta_increases_risk(long_account, request.asset_index, long_delta)?
                || position_delta_increases_risk(short_account, request.asset_index, short_delta)?;
        let target_effective_lag = self.asset_has_target_effective_lag(request.asset_index)?;
        if risk_increasing && (locked || target_effective_lag) {
            return Err(V14Error::LockActive);
        }

        let notional = trade_notional_floor(request.size_q, request.exec_price)?;
        let fee = checked_fee_bps(notional, request.fee_bps)?;
        self.charge_account_fee_not_atomic(long_account, fee)?;
        self.charge_account_fee_not_atomic(short_account, fee)?;
        self.apply_position_delta(long_account, request.asset_index, long_delta)?;
        self.apply_position_delta(short_account, request.asset_index, short_delta)?;
        self.full_account_refresh(long_account, effective_prices)?;
        self.full_account_refresh(short_account, effective_prices)?;
        ensure_initial_margin(long_account)?;
        ensure_initial_margin(short_account)?;
        if locked {
            ensure_no_positive_credit_initial_margin(long_account)?;
            ensure_no_positive_credit_initial_margin(short_account)?;
        }
        self.assert_public_invariants()?;
        Ok(TradeOutcomeV14 {
            fee_a: fee,
            fee_b: fee,
            notional,
        })
    }

    pub fn liquidate_account_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV14,
        request: LiquidationRequestV14,
        effective_prices: &[u64; V14_MAX_PORTFOLIO_ASSETS_N],
    ) -> V14Result<LiquidationOutcomeV14> {
        if request.asset_index >= self.config.max_portfolio_assets as usize
            || request.close_q == 0
            || request.fee_bps
                > self
                    .config
                    .liquidation_fee_bps
                    .max(self.config.max_trading_fee_bps)
        {
            return Err(V14Error::InvalidConfig);
        }
        self.settle_account_side_effects_not_atomic(account, self.config.public_b_chunk_atoms)?;
        self.full_account_refresh(account, effective_prices)?;
        if account.health_cert.certified_liq_deficit == 0 {
            return Err(V14Error::NonProgress);
        }
        let before = *account;
        let leg = account.legs[request.asset_index];
        if !leg.active {
            return Err(V14Error::InvalidLeg);
        }
        let close_q = request.close_q.min(leg.basis_pos_q.unsigned_abs());
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
                PermissionlessRecoveryReasonV14::ActiveBankruptCloseCannotProgress,
            )?;
            return Err(V14Error::RecoveryRequired);
        }
        self.preflight_liquidation_residual_durability(request.asset_index, leg.side, account)?;
        let fee_notional = risk_notional_ceil(close_q, effective_prices[request.asset_index])?;
        let fee = checked_fee_bps(fee_notional, request.fee_bps)?
            .max(self.config.min_liquidation_abs)
            .min(self.config.liquidation_fee_cap);
        let charged_fee = self.charge_account_fee_not_atomic(account, fee)?;
        self.reduce_position(account, request.asset_index, close_q)?;
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
                .ok_or(V14Error::ArithmeticOverflow)?
                .min(residual);
            let cleared_i128 = i128::try_from(cleared).map_err(|_| V14Error::ArithmeticOverflow)?;
            self.set_account_pnl(
                account,
                account
                    .pnl
                    .checked_add(cleared_i128)
                    .ok_or(V14Error::ArithmeticOverflow)?,
            )?;
            self.bankruptcy_hlock_active = true;
        }
        self.full_account_refresh(account, effective_prices)?;
        self.validate_liquidation_progress(&before, account)?;
        self.assert_public_invariants()?;
        Ok(LiquidationOutcomeV14 {
            closed_q: close_q,
            insurance_used,
            residual_booked: booked,
            explicit_loss: explicit,
            fee_charged: charged_fee,
        })
    }

    pub fn forfeit_recovery_leg_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV14,
        asset_index: usize,
        b_delta_budget: u128,
    ) -> V14Result<DeadLegForfeitOutcomeV14> {
        self.validate_account_shape(account)?;
        if asset_index >= self.config.max_portfolio_assets as usize || b_delta_budget == 0 {
            return Err(V14Error::InvalidLeg);
        }
        let leg = account.legs[asset_index];
        if !leg.active {
            return Err(V14Error::InvalidLeg);
        }
        if !self.leg_is_dead_for_forfeit(asset_index, leg.side)? {
            return Err(V14Error::LockActive);
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
                .ok_or(V14Error::ArithmeticOverflow)?;
            if chunk.remaining_after != 0 {
                return Ok(DeadLegForfeitOutcomeV14 {
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
            .ok_or(V14Error::ArithmeticOverflow)?;
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
                .ok_or(V14Error::ArithmeticOverflow)?
                .min(residual);
            let cleared_i128 = i128::try_from(cleared).map_err(|_| V14Error::ArithmeticOverflow)?;
            self.set_account_pnl(
                account,
                account
                    .pnl
                    .checked_add(cleared_i128)
                    .ok_or(V14Error::ArithmeticOverflow)?,
            )?;
        }

        let detached = account.pnl >= 0 && !account.close_progress.has_pending_residual();
        if detached {
            self.clear_leg(account, asset_index)?;
        }

        self.assert_public_invariants()?;
        Ok(DeadLegForfeitOutcomeV14 {
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
        account: &mut PortfolioAccountV14,
        request: RebalanceRequestV14,
        effective_prices: &[u64; V14_MAX_PORTFOLIO_ASSETS_N],
    ) -> V14Result<RebalanceOutcomeV14> {
        if request.asset_index >= self.config.max_portfolio_assets as usize || request.reduce_q == 0
        {
            return Err(V14Error::InvalidConfig);
        }
        self.settle_account_side_effects_not_atomic(account, self.config.public_b_chunk_atoms)?;
        self.full_account_refresh(account, effective_prices)?;
        let before = *account;
        let leg = account.legs[request.asset_index];
        if !leg.active {
            return Err(V14Error::InvalidLeg);
        }
        let reduce_q = request.reduce_q.min(leg.basis_pos_q.unsigned_abs());
        if reduce_q == 0 {
            return Err(V14Error::NonProgress);
        }
        self.reduce_position(account, request.asset_index, reduce_q)?;
        self.settle_negative_pnl_from_principal(account)?;
        self.full_account_refresh(account, effective_prices)?;
        self.validate_liquidation_progress(&before, account)?;
        self.assert_public_invariants()?;
        Ok(RebalanceOutcomeV14 {
            reduced_q: reduce_q,
        })
    }

    pub fn permissionless_crank_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV14,
        request: PermissionlessCrankRequestV14,
        effective_prices: &[u64; V14_MAX_PORTFOLIO_ASSETS_N],
    ) -> V14Result<PermissionlessProgressOutcomeV14> {
        self.validate_account_shape(account)?;
        let protective_progress = match request.action {
            PermissionlessCrankActionV14::Refresh => {
                if let PermissionlessProgressOutcomeV14::AccountBChunk(out) = self
                    .settle_account_side_effects_not_atomic(
                        account,
                        self.config.public_b_chunk_atoms,
                    )?
                {
                    self.assert_public_invariants()?;
                    return Ok(PermissionlessProgressOutcomeV14::AccountBChunk(out));
                }
                self.full_account_refresh(account, effective_prices)?;
                true
            }
            PermissionlessCrankActionV14::SettleB { asset_index } => {
                let out = self.settle_account_b_chunk(
                    account,
                    asset_index,
                    self.config.public_b_chunk_atoms,
                )?;
                return Ok(PermissionlessProgressOutcomeV14::AccountBChunk(out));
            }
            PermissionlessCrankActionV14::Liquidate(liq) => {
                self.liquidate_account_not_atomic(account, liq, effective_prices)?;
                true
            }
            PermissionlessCrankActionV14::Recover(reason) => {
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
        Ok(PermissionlessProgressOutcomeV14::AccountCurrent)
    }

    pub fn resolve_market_not_atomic(&mut self, resolved_slot: u64) -> V14Result<()> {
        if resolved_slot < self.current_slot {
            return Err(V14Error::Stale);
        }
        self.mode = MarketModeV14::Resolved;
        self.resolved_slot = resolved_slot;
        self.current_slot = resolved_slot;
        self.loss_stale_active = false;
        self.assert_public_invariants()
    }

    pub fn close_resolved_account_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV14,
        fee_rate_per_slot: u128,
    ) -> V14Result<ResolvedCloseOutcomeV14> {
        if self.mode != MarketModeV14::Resolved {
            return Err(V14Error::LockActive);
        }
        if let PermissionlessProgressOutcomeV14::AccountBChunk(_) =
            self.settle_account_side_effects_not_atomic(account, self.config.public_b_chunk_atoms)?
        {
            self.assert_public_invariants()?;
            return Ok(ResolvedCloseOutcomeV14::ProgressOnly);
        }
        self.sync_account_fee_to_slot_not_atomic(account, self.resolved_slot, fee_rate_per_slot)?;
        self.settle_negative_pnl_from_principal(account)?;
        if account.pnl < 0 {
            self.declare_permissionless_recovery(
                PermissionlessRecoveryReasonV14::ActiveBankruptCloseCannotProgress,
            )?;
            self.assert_public_invariants()?;
            return Ok(ResolvedCloseOutcomeV14::ProgressOnly);
        }
        if account.active_bitmap != 0
            || account.pnl < 0
            || account.b_stale_state
            || account.stale_state
        {
            return Ok(ResolvedCloseOutcomeV14::ProgressOnly);
        }
        if account.pnl > 0 && !self.resolved_positive_payout_ready() {
            return Ok(ResolvedCloseOutcomeV14::ProgressOnly);
        }
        if !self.payout_snapshot_captured {
            self.payout_snapshot = self.residual();
            self.payout_snapshot_pnl_pos_tot = self.junior_claim_bound();
            self.payout_snapshot_captured = true;
        }
        let pnl_payout = if account.pnl > 0 && self.payout_snapshot_pnl_pos_tot != 0 {
            wide_mul_div_floor_u128(
                account.pnl as u128,
                self.payout_snapshot,
                self.payout_snapshot_pnl_pos_tot,
            )
        } else {
            0
        };
        let payout = account
            .capital
            .checked_add(pnl_payout)
            .ok_or(V14Error::ArithmeticOverflow)?
            .min(self.vault);
        self.vault = self
            .vault
            .checked_sub(payout)
            .ok_or(V14Error::CounterUnderflow)?;
        self.c_tot = self.c_tot.saturating_sub(account.capital.min(self.c_tot));
        self.set_account_pnl(account, 0)?;
        account.capital = 0;
        account.reserved_pnl = 0;
        account.fee_credits = 0;
        account.health_cert.valid = false;
        self.assert_public_invariants()?;
        Ok(ResolvedCloseOutcomeV14::Closed { payout })
    }

    fn begin_close_progress_ledger(
        &mut self,
        account: &mut PortfolioAccountV14,
        asset_index: usize,
        domain_side: SideV14,
        gross_loss: u128,
    ) -> V14Result<()> {
        self.validate_account_shape(account)?;
        if gross_loss == 0 {
            return Ok(());
        }
        if account.close_progress.active {
            return Err(V14Error::LockActive);
        }
        let close_id = account.close_progress.close_id.saturating_add(1).max(1);
        let close_span = self
            .config
            .max_accrual_dt_slots
            .checked_mul(self.config.max_bankrupt_close_chunks)
            .ok_or(V14Error::ArithmeticOverflow)?;
        let ledger = CloseProgressLedgerV14 {
            active: true,
            finalized: false,
            close_id,
            asset_index: u8::try_from(asset_index).map_err(|_| V14Error::InvalidLeg)?,
            domain_side,
            gross_loss_at_close_start: gross_loss,
            drift_reference_slot: self.current_slot,
            max_close_slot: self
                .current_slot
                .checked_add(close_span)
                .ok_or(V14Error::ArithmeticOverflow)?,
            residual_remaining: gross_loss,
            ..CloseProgressLedgerV14::EMPTY
        };
        self.validate_close_progress_ledger(ledger)?;
        let domain = self.insurance_domain_index(asset_index, domain_side)?;
        self.pending_domain_loss_barriers[domain] = self.pending_domain_loss_barriers[domain]
            .checked_add(1)
            .ok_or(V14Error::CounterOverflow)?;
        account.close_progress = ledger;
        Ok(())
    }

    fn advance_close_progress_ledger(
        &mut self,
        account: &mut PortfolioAccountV14,
        support_consumed: u128,
        junior_face_burned: u128,
        insurance_spent: u128,
        b_loss_booked: u128,
        explicit_loss_assigned: u128,
    ) -> V14Result<()> {
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
            return Err(V14Error::LockActive);
        }
        ledger.support_consumed = ledger
            .support_consumed
            .checked_add(support_consumed)
            .ok_or(V14Error::ArithmeticOverflow)?;
        ledger.junior_face_burned = ledger
            .junior_face_burned
            .checked_add(junior_face_burned)
            .ok_or(V14Error::ArithmeticOverflow)?;
        ledger.insurance_spent = ledger
            .insurance_spent
            .checked_add(insurance_spent)
            .ok_or(V14Error::ArithmeticOverflow)?;
        ledger.b_loss_booked = ledger
            .b_loss_booked
            .checked_add(b_loss_booked)
            .ok_or(V14Error::ArithmeticOverflow)?;
        ledger.explicit_loss_assigned = ledger
            .explicit_loss_assigned
            .checked_add(explicit_loss_assigned)
            .ok_or(V14Error::ArithmeticOverflow)?;
        let total_loss = ledger
            .gross_loss_at_close_start
            .checked_add(ledger.drift_consumed)
            .ok_or(V14Error::ArithmeticOverflow)?;
        let progress = ledger
            .support_consumed
            .checked_add(ledger.insurance_spent)
            .and_then(|v| v.checked_add(ledger.b_loss_booked))
            .and_then(|v| v.checked_add(ledger.explicit_loss_assigned))
            .ok_or(V14Error::ArithmeticOverflow)?;
        if progress > total_loss {
            return Err(V14Error::ArithmeticOverflow);
        }
        ledger.residual_remaining = total_loss - progress;
        if ledger.residual_remaining == 0 {
            ledger.finalized = true;
        }
        self.validate_close_progress_ledger(ledger)?;
        if was_pending && !ledger.has_pending_residual() {
            self.pending_domain_loss_barriers[domain] = self.pending_domain_loss_barriers[domain]
                .checked_sub(1)
                .ok_or(V14Error::CounterUnderflow)?;
        }
        account.close_progress = ledger;
        account.health_cert.valid = false;
        Ok(())
    }

    fn advance_close_progress_quantity_adl(
        &mut self,
        account: &mut PortfolioAccountV14,
        quantity_adl_applied_q: u128,
    ) -> V14Result<()> {
        if quantity_adl_applied_q == 0 {
            return Err(V14Error::NonProgress);
        }
        let mut ledger = account.close_progress;
        self.ensure_close_progress_not_expired(ledger)?;
        if !ledger.active || !ledger.finalized || ledger.residual_remaining != 0 {
            return Err(V14Error::LockActive);
        }
        if ledger.quantity_adl_applied_q != 0 {
            return Err(V14Error::LockActive);
        }
        ledger.quantity_adl_applied_q = quantity_adl_applied_q;
        self.validate_close_progress_ledger(ledger)?;
        account.close_progress = ledger;
        account.health_cert.valid = false;
        Ok(())
    }

    pub fn book_bankruptcy_residual_chunk_for_account(
        &mut self,
        account: &mut PortfolioAccountV14,
        asset_index: usize,
        bankrupt_side: SideV14,
        residual_remaining: u128,
    ) -> V14Result<BResidualBookingOutcomeV14> {
        self.validate_account_shape(account)?;
        if residual_remaining == 0 {
            return Ok(BResidualBookingOutcomeV14 {
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
                    PermissionlessRecoveryReasonV14::ActiveBankruptCloseCannotProgress,
                )?;
                return Err(V14Error::RecoveryRequired);
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
            return Err(V14Error::LockActive);
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
        bankrupt_side: SideV14,
        residual_remaining: u128,
    ) -> V14Result<BResidualBookingOutcomeV14> {
        if asset_index >= self.config.max_portfolio_assets as usize {
            return Err(V14Error::InvalidLeg);
        }
        if residual_remaining == 0 {
            return Ok(BResidualBookingOutcomeV14 {
                booked_loss: 0,
                explicit_loss: 0,
                delta_b: 0,
                remaining_after: 0,
            });
        }
        let opp = opposite_side(bankrupt_side);
        let asset = self.assets[asset_index];
        let (b_now, weight_sum, rem) = match opp {
            SideV14::Long => (
                asset.b_long_num,
                asset.loss_weight_sum_long,
                asset.social_loss_remainder_long_num,
            ),
            SideV14::Short => (
                asset.b_short_num,
                asset.loss_weight_sum_short,
                asset.social_loss_remainder_short_num,
            ),
        };
        if weight_sum == 0 {
            self.declare_permissionless_recovery(
                PermissionlessRecoveryReasonV14::ActiveBankruptCloseCannotProgress,
            )?;
            return Err(V14Error::RecoveryRequired);
        }
        let engine_chunk = self.bankruptcy_residual_single_step_capacity(
            asset_index,
            bankrupt_side,
            residual_remaining,
        )?;
        if engine_chunk == 0 {
            self.declare_permissionless_recovery(
                PermissionlessRecoveryReasonV14::BIndexHeadroomExhausted,
            )?;
            return Err(V14Error::RecoveryRequired);
        }
        let numerator = engine_chunk
            .checked_mul(SOCIAL_LOSS_DEN)
            .and_then(|v| v.checked_add(rem))
            .ok_or(V14Error::ArithmeticOverflow)?;
        let delta_b = numerator / weight_sum;
        let new_rem = numerator % weight_sum;
        if delta_b == 0 || b_now.checked_add(delta_b).is_none() {
            self.declare_permissionless_recovery(
                PermissionlessRecoveryReasonV14::BIndexHeadroomExhausted,
            )?;
            return Err(V14Error::RecoveryRequired);
        }
        let asset = &mut self.assets[asset_index];
        match opp {
            SideV14::Long => {
                asset.b_long_num = asset
                    .b_long_num
                    .checked_add(delta_b)
                    .ok_or(V14Error::ArithmeticOverflow)?;
                asset.social_loss_remainder_long_num = new_rem;
            }
            SideV14::Short => {
                asset.b_short_num = asset
                    .b_short_num
                    .checked_add(delta_b)
                    .ok_or(V14Error::ArithmeticOverflow)?;
                asset.social_loss_remainder_short_num = new_rem;
            }
        }
        self.bankruptcy_hlock_active = true;
        Ok(BResidualBookingOutcomeV14 {
            booked_loss: engine_chunk,
            explicit_loss: 0,
            delta_b,
            remaining_after: residual_remaining - engine_chunk,
        })
    }

    pub fn apply_quantity_adl_after_residual_for_account_not_atomic(
        &mut self,
        account: &mut PortfolioAccountV14,
        asset_index: usize,
        bankrupt_side: SideV14,
        close_q: u128,
    ) -> V14Result<QuantityAdlOutcomeV14> {
        self.validate_account_shape(account)?;
        let ledger = account.close_progress;
        let leg = if asset_index < self.config.max_portfolio_assets as usize {
            account.legs[asset_index]
        } else {
            return Err(V14Error::InvalidLeg);
        };
        if !ledger.active
            || !ledger.finalized
            || ledger.residual_remaining != 0
            || ledger.asset_index as usize != asset_index
            || ledger.domain_side != opposite_side(bankrupt_side)
        {
            return Err(V14Error::LockActive);
        }
        if !leg.active
            || leg.stale
            || leg.b_stale
            || leg.side != bankrupt_side
            || close_q != leg.basis_pos_q.unsigned_abs()
        {
            return Err(V14Error::InvalidLeg);
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
        account: &mut PortfolioAccountV14,
        asset_index: usize,
        leg: PortfolioLegV14,
    ) -> V14Result<()> {
        if asset_index >= self.config.max_portfolio_assets as usize
            || !leg.active
            || leg.stale
            || leg.b_stale
            || account.legs[asset_index] != leg
        {
            return Err(V14Error::InvalidLeg);
        }

        let asset = &mut self.assets[asset_index];
        let prior_reset_epoch = match leg.side {
            SideV14::Long => {
                asset.mode_long == SideModeV14::ResetPending
                    && leg.epoch_snap.checked_add(1) == Some(asset.epoch_long)
            }
            SideV14::Short => {
                asset.mode_short == SideModeV14::ResetPending
                    && leg.epoch_snap.checked_add(1) == Some(asset.epoch_short)
            }
        };
        match leg.side {
            SideV14::Long => {
                asset.stored_pos_count_long = asset
                    .stored_pos_count_long
                    .checked_sub(1)
                    .ok_or(V14Error::CounterUnderflow)?;
                if !prior_reset_epoch {
                    asset.loss_weight_sum_long = asset
                        .loss_weight_sum_long
                        .checked_sub(leg.loss_weight)
                        .ok_or(V14Error::CounterUnderflow)?;
                }
            }
            SideV14::Short => {
                asset.stored_pos_count_short = asset
                    .stored_pos_count_short
                    .checked_sub(1)
                    .ok_or(V14Error::CounterUnderflow)?;
                if !prior_reset_epoch {
                    asset.loss_weight_sum_short = asset
                        .loss_weight_sum_short
                        .checked_sub(leg.loss_weight)
                        .ok_or(V14Error::CounterUnderflow)?;
                }
            }
        }
        account.legs[asset_index] = PortfolioLegV14::EMPTY;
        account.active_bitmap &= !(1u32 << asset_index);
        account.health_cert.valid = false;
        self.validate_account_shape(account)
    }

    fn ensure_close_progress_not_expired(
        &mut self,
        ledger: CloseProgressLedgerV14,
    ) -> V14Result<()> {
        if ledger.active && self.current_slot > ledger.max_close_slot {
            self.declare_permissionless_recovery(
                PermissionlessRecoveryReasonV14::ActiveBankruptCloseCannotProgress,
            )?;
            return Err(V14Error::RecoveryRequired);
        }
        Ok(())
    }

    fn ensure_open_close_snapshot_current_or_recovery(
        &mut self,
        account: &PortfolioAccountV14,
        ledger: CloseProgressLedgerV14,
    ) -> V14Result<()> {
        if !ledger.active {
            return Ok(());
        }
        let asset_index = ledger.asset_index as usize;
        if asset_index < self.config.max_portfolio_assets as usize
            && account.legs[asset_index].active
            && self.current_slot > ledger.drift_reference_slot
        {
            self.declare_permissionless_recovery(
                PermissionlessRecoveryReasonV14::ActiveBankruptCloseCannotProgress,
            )?;
            return Err(V14Error::RecoveryRequired);
        }
        Ok(())
    }

    fn apply_quantity_adl_after_residual_internal(
        &mut self,
        asset_index: usize,
        bankrupt_side: SideV14,
        close_q: u128,
    ) -> V14Result<QuantityAdlOutcomeV14> {
        if asset_index >= self.config.max_portfolio_assets as usize || close_q == 0 {
            return Err(V14Error::InvalidLeg);
        }
        let opp = opposite_side(bankrupt_side);
        let asset = self.assets[asset_index];
        let (liq_oi_before, opp_oi_before, opp_a_before) = match (bankrupt_side, opp) {
            (SideV14::Long, SideV14::Short) => {
                (asset.oi_eff_long_q, asset.oi_eff_short_q, asset.a_short)
            }
            (SideV14::Short, SideV14::Long) => {
                (asset.oi_eff_short_q, asset.oi_eff_long_q, asset.a_long)
            }
            _ => unreachable!(),
        };
        if close_q > liq_oi_before || close_q > opp_oi_before {
            return Err(V14Error::InvalidLeg);
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
                SideV14::Long => asset.oi_eff_long_q = final_liq_oi_after,
                SideV14::Short => asset.oi_eff_short_q = final_liq_oi_after,
            }
            match opp {
                SideV14::Long => {
                    asset.oi_eff_long_q = final_opp_oi_after;
                    asset.a_long =
                        opposite_a_after.max(if final_opp_oi_after == 0 { ADL_ONE } else { 1 });
                    if final_opp_oi_after != 0 && asset.a_long < MIN_A_SIDE {
                        asset.mode_long = SideModeV14::DrainOnly;
                    }
                }
                SideV14::Short => {
                    asset.oi_eff_short_q = final_opp_oi_after;
                    asset.a_short =
                        opposite_a_after.max(if final_opp_oi_after == 0 { ADL_ONE } else { 1 });
                    if final_opp_oi_after != 0 && asset.a_short < MIN_A_SIDE {
                        asset.mode_short = SideModeV14::DrainOnly;
                    }
                }
            }
        }

        if final_liq_oi_after == 0 {
            self.begin_full_drain_reset(asset_index, bankrupt_side)?;
            reset_started = true;
        }
        if final_opp_oi_after == 0 {
            self.begin_full_drain_reset(asset_index, opp)?;
            reset_started = true;
        }
        self.assert_public_invariants()?;
        Ok(QuantityAdlOutcomeV14 {
            closed_q: close_q,
            opposite_a_after,
            reset_started,
        })
    }

    pub fn begin_full_drain_reset(&mut self, asset_index: usize, side: SideV14) -> V14Result<()> {
        if self.active_bankrupt_close_present
            || asset_index >= self.config.max_portfolio_assets as usize
        {
            return Err(V14Error::LockActive);
        }
        let asset = &mut self.assets[asset_index];
        match side {
            SideV14::Long => {
                if asset.oi_eff_long_q != 0 {
                    return Err(V14Error::InvalidLeg);
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
                    .ok_or(V14Error::CounterOverflow)?;
                asset.mode_long = SideModeV14::ResetPending;
            }
            SideV14::Short => {
                if asset.oi_eff_short_q != 0 {
                    return Err(V14Error::InvalidLeg);
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
                    .ok_or(V14Error::CounterOverflow)?;
                asset.mode_short = SideModeV14::ResetPending;
            }
        }
        self.risk_epoch = self
            .risk_epoch
            .checked_add(1)
            .ok_or(V14Error::CounterOverflow)?;
        self.assert_public_invariants()
    }

    pub fn finalize_ready_reset_side(
        &mut self,
        asset_index: usize,
        side: SideV14,
    ) -> V14Result<()> {
        if asset_index >= self.config.max_portfolio_assets as usize {
            return Err(V14Error::InvalidLeg);
        }
        let asset = &mut self.assets[asset_index];
        match side {
            SideV14::Long => {
                if asset.mode_long != SideModeV14::ResetPending {
                    return Ok(());
                }
                if asset.stored_pos_count_long != 0 || asset.stale_account_count_long != 0 {
                    return Err(V14Error::Stale);
                }
                asset.mode_long = SideModeV14::Normal;
            }
            SideV14::Short => {
                if asset.mode_short != SideModeV14::ResetPending {
                    return Ok(());
                }
                if asset.stored_pos_count_short != 0 || asset.stale_account_count_short != 0 {
                    return Err(V14Error::Stale);
                }
                asset.mode_short = SideModeV14::Normal;
            }
        }
        self.assert_public_invariants()
    }

    pub fn risk_score(&self, account: &PortfolioAccountV14) -> V14Result<RiskScoreV14> {
        self.validate_account_shape(account)?;
        if !account.health_cert.valid {
            return Err(V14Error::Stale);
        }
        Ok(RiskScoreV14 {
            certified_liq_deficit: account.health_cert.certified_liq_deficit,
            unsettled_b_loss_bound: account_b_loss_bound(account)?,
            stale_loss_bound: if account.stale_state { 1 } else { 0 },
            gross_risk_notional: account.health_cert.certified_worst_case_loss,
            active_leg_count: account.active_bitmap.count_ones(),
        })
    }

    pub fn validate_liquidation_progress(
        &self,
        before: &PortfolioAccountV14,
        after: &PortfolioAccountV14,
    ) -> V14Result<()> {
        let before_score = self.risk_score(before)?;
        let after_score = self.risk_score(after)?;
        if after_score.strictly_reduces_from(before_score)
            || after_score.certified_liq_deficit < before_score.certified_liq_deficit
        {
            Ok(())
        } else {
            Err(V14Error::NonProgress)
        }
    }

    pub fn declare_permissionless_recovery(
        &mut self,
        reason: PermissionlessRecoveryReasonV14,
    ) -> V14Result<PermissionlessProgressOutcomeV14> {
        if !self.config.permissionless_recovery_enabled {
            return Err(V14Error::InvalidConfig);
        }
        self.recovery_reason = Some(reason);
        Ok(PermissionlessProgressOutcomeV14::RecoveryDeclared(reason))
    }

    pub fn assert_public_invariants(&self) -> V14Result<()> {
        if self.vault > MAX_VAULT_TVL {
            return Err(V14Error::InvalidConfig);
        }
        let senior = self
            .c_tot
            .checked_add(self.insurance)
            .ok_or(V14Error::ArithmeticOverflow)?;
        if self.c_tot > self.vault || self.insurance > self.vault || senior > self.vault {
            return Err(V14Error::InvalidConfig);
        }
        if self.pnl_matured_pos_tot > self.pnl_pos_tot {
            return Err(V14Error::InvalidConfig);
        }
        if self.pnl_pos_bound_tot < self.pnl_pos_tot {
            return Err(V14Error::InvalidConfig);
        }
        if self.slot_last > self.current_slot {
            return Err(V14Error::InvalidConfig);
        }
        let mut d = 0;
        while d < V14_DOMAIN_COUNT {
            if self.insurance_domain_spent[d] > self.insurance_domain_budget[d] {
                return Err(V14Error::InvalidConfig);
            }
            if d >= self.config.max_portfolio_assets as usize * 2
                && self.pending_domain_loss_barriers[d] != 0
            {
                return Err(V14Error::InvalidConfig);
            }
            d += 1;
        }
        for i in 0..self.config.max_portfolio_assets as usize {
            let asset = self.assets[i];
            if asset.effective_price == 0
                || asset.effective_price > MAX_ORACLE_PRICE
                || asset.raw_oracle_target_price == 0
                || asset.raw_oracle_target_price > MAX_ORACLE_PRICE
                || asset.fund_px_last == 0
                || asset.fund_px_last > MAX_ORACLE_PRICE
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
                || asset.loss_weight_sum_long > SOCIAL_LOSS_DEN
                || asset.loss_weight_sum_short > SOCIAL_LOSS_DEN
                || asset.social_loss_remainder_long_num >= SOCIAL_LOSS_DEN
                || asset.social_loss_remainder_short_num >= SOCIAL_LOSS_DEN
                || asset.social_loss_dust_long_num >= SOCIAL_LOSS_DEN
                || asset.social_loss_dust_short_num >= SOCIAL_LOSS_DEN
            {
                return Err(V14Error::InvalidConfig);
            }
        }
        Ok(())
    }

    fn b_target_for_leg(&self, asset_index: usize, leg: PortfolioLegV14) -> V14Result<u128> {
        let asset = self.assets[asset_index];
        let (current_b, epoch_start_b, side_epoch, mode) = match leg.side {
            SideV14::Long => (
                asset.b_long_num,
                asset.b_epoch_start_long_num,
                asset.epoch_long,
                asset.mode_long,
            ),
            SideV14::Short => (
                asset.b_short_num,
                asset.b_epoch_start_short_num,
                asset.epoch_short,
                asset.mode_short,
            ),
        };
        if leg.b_epoch_snap == side_epoch {
            Ok(current_b)
        } else if mode == SideModeV14::ResetPending
            && leg.b_epoch_snap.checked_add(1) == Some(side_epoch)
        {
            Ok(epoch_start_b)
        } else {
            Err(V14Error::InvalidLeg)
        }
    }

    fn side_mode_for(&self, asset_index: usize, side: SideV14) -> V14Result<SideModeV14> {
        if asset_index >= self.config.max_portfolio_assets as usize {
            return Err(V14Error::InvalidLeg);
        }
        let asset = self.assets[asset_index];
        Ok(match side {
            SideV14::Long => asset.mode_long,
            SideV14::Short => asset.mode_short,
        })
    }

    fn leg_is_dead_for_forfeit(&self, asset_index: usize, side: SideV14) -> V14Result<bool> {
        let side_mode = self.side_mode_for(asset_index, side)?;
        Ok(self.mode == MarketModeV14::Recovery
            || matches!(
                side_mode,
                SideModeV14::DrainOnly | SideModeV14::ResetPending
            ))
    }

    fn kf_target_for_leg(
        &self,
        asset_index: usize,
        leg: PortfolioLegV14,
    ) -> V14Result<(i128, i128)> {
        let asset = self.assets[asset_index];
        let (current_k, current_f, epoch_start_k, epoch_start_f, side_epoch, mode) = match leg.side
        {
            SideV14::Long => (
                asset.k_long,
                asset.f_long_num,
                asset.k_epoch_start_long,
                asset.f_epoch_start_long_num,
                asset.epoch_long,
                asset.mode_long,
            ),
            SideV14::Short => (
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
        } else if mode == SideModeV14::ResetPending
            && leg.epoch_snap.checked_add(1) == Some(side_epoch)
        {
            Ok((epoch_start_k, epoch_start_f))
        } else {
            Err(V14Error::InvalidLeg)
        }
    }

    fn residual(&self) -> u128 {
        self.vault
            .saturating_sub(self.c_tot.saturating_add(self.insurance))
    }

    fn junior_claim_bound(&self) -> u128 {
        self.pnl_pos_bound_tot.max(self.pnl_pos_tot)
    }

    fn haircut_effective_support(
        &self,
        face_claim: u128,
        residual: u128,
        junior_bound: u128,
    ) -> V14Result<u128> {
        if face_claim == 0 || residual == 0 || junior_bound == 0 {
            return Ok(0);
        }
        if residual >= junior_bound {
            return Ok(face_claim);
        }
        Ok(wide_mul_div_floor_u128(face_claim, residual, junior_bound))
    }

    fn account_haircut_equity(&self, account: &PortfolioAccountV14) -> V14Result<i128> {
        self.account_haircut_equity_with_capital(account, account.capital)
    }

    fn account_haircut_equity_with_capital(
        &self,
        account: &PortfolioAccountV14,
        capital_override: u128,
    ) -> V14Result<i128> {
        validate_non_min_i128(account.pnl)?;
        validate_fee_credits(account.fee_credits)?;
        let capital = i128::try_from(capital_override).map_err(|_| V14Error::ArithmeticOverflow)?;
        let fee_debt =
            i128::try_from(fee_debt_u128(account)?).map_err(|_| V14Error::ArithmeticOverflow)?;
        let positive_support = self.haircut_effective_support(
            account.pnl.max(0) as u128,
            self.residual(),
            self.junior_claim_bound(),
        )?;
        let positive_support_i128 =
            i128::try_from(positive_support).map_err(|_| V14Error::ArithmeticOverflow)?;
        capital
            .checked_add(account.pnl.min(0))
            .and_then(|v| v.checked_add(positive_support_i128))
            .and_then(|v| v.checked_sub(fee_debt))
            .ok_or(V14Error::ArithmeticOverflow)
    }

    fn face_claim_to_burn_for_support(
        &self,
        effective_support: u128,
        residual: u128,
        junior_bound: u128,
    ) -> V14Result<u128> {
        if effective_support == 0 {
            return Ok(0);
        }
        if residual == 0 || junior_bound == 0 {
            return Err(V14Error::LockActive);
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
        .ok_or(V14Error::ArithmeticOverflow)
    }

    fn apply_haircut_bounded_close_loss_to_pnl(
        &mut self,
        account: &mut PortfolioAccountV14,
        loss_abs: u128,
    ) -> V14Result<SupportLossApplicationV14> {
        if loss_abs == 0 {
            return Ok(SupportLossApplicationV14 {
                support_consumed: 0,
                junior_face_burned: 0,
            });
        }

        let old_positive_face = account.pnl.max(0) as u128;
        if old_positive_face == 0 {
            let loss_i128 = i128::try_from(loss_abs).map_err(|_| V14Error::ArithmeticOverflow)?;
            let new_pnl = account
                .pnl
                .checked_sub(loss_i128)
                .ok_or(V14Error::ArithmeticOverflow)?;
            self.set_account_pnl(account, new_pnl)?;
            return Ok(SupportLossApplicationV14 {
                support_consumed: 0,
                junior_face_burned: 0,
            });
        }

        let residual = self.residual();
        let junior_bound = self.junior_claim_bound();
        let effective_available =
            self.haircut_effective_support(old_positive_face, residual, junior_bound)?;
        let support_consumed = effective_available.min(loss_abs);
        let remaining_loss = loss_abs
            .checked_sub(support_consumed)
            .ok_or(V14Error::ArithmeticOverflow)?;
        let mut junior_face_burned = if support_consumed == 0 {
            0
        } else {
            self.face_claim_to_burn_for_support(support_consumed, residual, junior_bound)?
        };
        if remaining_loss != 0 {
            junior_face_burned = old_positive_face;
        }
        if junior_face_burned > old_positive_face {
            return Err(V14Error::ArithmeticOverflow);
        }

        let retained_face = old_positive_face
            .checked_sub(junior_face_burned)
            .ok_or(V14Error::ArithmeticOverflow)?;
        let retained_i128 =
            i128::try_from(retained_face).map_err(|_| V14Error::ArithmeticOverflow)?;
        let remaining_i128 =
            i128::try_from(remaining_loss).map_err(|_| V14Error::ArithmeticOverflow)?;
        let new_pnl = retained_i128
            .checked_sub(remaining_i128)
            .ok_or(V14Error::ArithmeticOverflow)?;
        account.reserved_pnl = account.reserved_pnl.min(new_pnl.max(0) as u128);
        self.set_account_pnl(account, new_pnl)?;

        Ok(SupportLossApplicationV14 {
            support_consumed,
            junior_face_burned,
        })
    }

    fn apply_signed_kf_delta_to_pnl(
        &mut self,
        account: &mut PortfolioAccountV14,
        delta: i128,
    ) -> V14Result<SupportLossApplicationV14> {
        validate_non_min_i128(delta)?;
        if delta == 0 {
            return Ok(SupportLossApplicationV14 {
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
                .ok_or(V14Error::ArithmeticOverflow)?;
            self.set_account_pnl(account, new_pnl)?;
            return Ok(SupportLossApplicationV14 {
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
            .ok_or(V14Error::ArithmeticOverflow)?;
        let effective_available =
            self.haircut_effective_support(new_face_support, residual, junior_bound)?;
        let support_consumed = effective_available.min(old_loss);
        let remaining_loss = old_loss
            .checked_sub(support_consumed)
            .ok_or(V14Error::ArithmeticOverflow)?;
        let mut junior_face_burned = if support_consumed == 0 {
            0
        } else {
            self.face_claim_to_burn_for_support(support_consumed, residual, junior_bound)?
        };
        if remaining_loss != 0 {
            junior_face_burned = new_face_support;
        }
        if junior_face_burned > new_face_support {
            return Err(V14Error::ArithmeticOverflow);
        }

        let retained_face = new_face_support
            .checked_sub(junior_face_burned)
            .ok_or(V14Error::ArithmeticOverflow)?;
        let retained_i128 =
            i128::try_from(retained_face).map_err(|_| V14Error::ArithmeticOverflow)?;
        let remaining_i128 =
            i128::try_from(remaining_loss).map_err(|_| V14Error::ArithmeticOverflow)?;
        let new_pnl = retained_i128
            .checked_sub(remaining_i128)
            .ok_or(V14Error::ArithmeticOverflow)?;
        self.set_account_pnl(account, new_pnl)?;
        Ok(SupportLossApplicationV14 {
            support_consumed,
            junior_face_burned,
        })
    }

    fn insurance_domain_index(&self, asset_index: usize, side: SideV14) -> V14Result<usize> {
        if asset_index >= self.config.max_portfolio_assets as usize {
            return Err(V14Error::InvalidLeg);
        }
        let domain = asset_index
            .checked_mul(2)
            .and_then(|v| v.checked_add(encode_side(side) as usize))
            .ok_or(V14Error::ArithmeticOverflow)?;
        if domain >= V14_DOMAIN_COUNT {
            return Err(V14Error::InvalidLeg);
        }
        Ok(domain)
    }

    pub fn pending_domain_loss_barrier_count(
        &self,
        asset_index: usize,
        side: SideV14,
    ) -> V14Result<u64> {
        let domain = self.insurance_domain_index(asset_index, side)?;
        Ok(self.pending_domain_loss_barriers[domain])
    }

    fn has_pending_domain_loss_barrier(
        &self,
        asset_index: usize,
        side: SideV14,
    ) -> V14Result<bool> {
        Ok(self.pending_domain_loss_barrier_count(asset_index, side)? != 0)
    }

    fn account_touches_pending_domain_loss_barrier(
        &self,
        account: &PortfolioAccountV14,
    ) -> V14Result<bool> {
        let limit = core::cmp::min(
            self.config.max_portfolio_assets as usize,
            V14_MAX_PORTFOLIO_ASSETS_N,
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

    fn available_domain_insurance(&self, domain: usize) -> u128 {
        if domain >= V14_DOMAIN_COUNT {
            return 0;
        }
        let budget_remaining = self.insurance_domain_budget[domain]
            .saturating_sub(self.insurance_domain_spent[domain]);
        self.insurance.min(budget_remaining)
    }

    fn consume_domain_insurance_for_negative_pnl(
        &mut self,
        asset_index: usize,
        bankrupt_side: SideV14,
        account: &mut PortfolioAccountV14,
    ) -> V14Result<u128> {
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
        self.insurance = self
            .insurance
            .checked_sub(used)
            .ok_or(V14Error::CounterUnderflow)?;
        self.insurance_domain_spent[domain] = self.insurance_domain_spent[domain]
            .checked_add(used)
            .ok_or(V14Error::ArithmeticOverflow)?;
        let used_i128 = i128::try_from(used).map_err(|_| V14Error::ArithmeticOverflow)?;
        let new_pnl = account
            .pnl
            .checked_add(used_i128)
            .ok_or(V14Error::ArithmeticOverflow)?;
        self.set_account_pnl(account, new_pnl)?;
        account.health_cert.valid = false;
        Ok(used)
    }

    fn preflight_liquidation_residual_durability(
        &mut self,
        asset_index: usize,
        bankrupt_side: SideV14,
        account: &PortfolioAccountV14,
    ) -> V14Result<()> {
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
                PermissionlessRecoveryReasonV14::ActiveBankruptCloseCannotProgress,
            )?;
            return Err(V14Error::RecoveryRequired);
        }
        Ok(())
    }

    fn bankruptcy_residual_single_step_capacity(
        &self,
        asset_index: usize,
        bankrupt_side: SideV14,
        residual_remaining: u128,
    ) -> V14Result<u128> {
        if asset_index >= self.config.max_portfolio_assets as usize {
            return Err(V14Error::InvalidLeg);
        }
        if residual_remaining == 0 {
            return Ok(0);
        }

        let opp = opposite_side(bankrupt_side);
        let asset = self.assets[asset_index];
        let (b_now, weight_sum, rem) = match opp {
            SideV14::Long => (
                asset.b_long_num,
                asset.loss_weight_sum_long,
                asset.social_loss_remainder_long_num,
            ),
            SideV14::Short => (
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
            .ok_or(V14Error::ArithmeticOverflow)?;
        let max_scaled = headroom_plus_one
            .checked_mul(U256::from_u128(weight_sum))
            .and_then(|v| v.checked_sub(U256::ONE))
            .ok_or(V14Error::ArithmeticOverflow)?;
        if U256::from_u128(rem) > max_scaled {
            return Ok(0);
        }
        let max_chunk_by_b_wide = max_scaled
            .checked_sub(U256::from_u128(rem))
            .and_then(|v| v.checked_div(U256::from_u128(SOCIAL_LOSS_DEN)))
            .ok_or(V14Error::ArithmeticOverflow)?;
        let max_chunk_by_b = max_chunk_by_b_wide
            .try_into_u128()
            .unwrap_or(residual_remaining);
        Ok(residual_remaining
            .min(max_chunk_by_b)
            .min(self.config.public_b_chunk_atoms))
    }

    fn resolved_positive_payout_ready(&self) -> bool {
        if self.active_bankrupt_close_present
            || self.b_stale_account_count != 0
            || self.stale_certificate_count != 0
            || self.negative_pnl_account_count != 0
        {
            return false;
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
        account: &mut PortfolioAccountV14,
        asset_index: usize,
    ) -> V14Result<()> {
        let leg = account.legs[asset_index];
        if !leg.active {
            return Ok(());
        }
        let (k_now, f_now) = self.kf_target_for_leg(asset_index, leg)?;
        let den = leg
            .a_basis
            .checked_mul(POS_SCALE)
            .ok_or(V14Error::ArithmeticOverflow)?;
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
            .ok_or(V14Error::ArithmeticOverflow)?;
        validate_non_min_i128(net)?;
        if net != 0 {
            self.apply_signed_kf_delta_to_pnl(account, net)?;
        }
        account.legs[asset_index].k_snap = k_now;
        account.legs[asset_index].f_snap = f_now;
        account.health_cert.valid = false;
        Ok(())
    }

    fn settle_forfeited_leg_kf_effects(
        &mut self,
        account: &mut PortfolioAccountV14,
        asset_index: usize,
    ) -> V14Result<(u128, u128, u128, u128)> {
        let leg = account.legs[asset_index];
        if !leg.active {
            return Ok((0, 0, 0, 0));
        }
        let (k_now, f_now) = self.kf_target_for_leg(asset_index, leg)?;
        let den = leg
            .a_basis
            .checked_mul(POS_SCALE)
            .ok_or(V14Error::ArithmeticOverflow)?;
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
            .ok_or(V14Error::ArithmeticOverflow)?;
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
        account: &mut PortfolioAccountV14,
        asset_index: usize,
        delta_q: i128,
    ) -> V14Result<()> {
        if delta_q == 0 {
            return Ok(());
        }
        if asset_index >= self.config.max_portfolio_assets as usize {
            return Err(V14Error::InvalidLeg);
        }
        self.settle_leg_kf_effects(account, asset_index)?;
        let current = signed_position(account.legs[asset_index]);
        let new = current
            .checked_add(delta_q)
            .ok_or(V14Error::ArithmeticOverflow)?;
        validate_basis_or_zero(new)?;
        if current == 0 {
            let side = if new > 0 {
                SideV14::Long
            } else {
                SideV14::Short
            };
            return self.attach_leg(account, asset_index, side, new);
        }
        if new == 0 {
            return self.clear_leg(account, asset_index);
        }
        if current.signum() != new.signum() {
            self.clear_leg(account, asset_index)?;
            let side = if new > 0 {
                SideV14::Long
            } else {
                SideV14::Short
            };
            return self.attach_leg(account, asset_index, side, new);
        }

        let old_leg = account.legs[asset_index];
        let old_abs = old_leg.basis_pos_q.unsigned_abs();
        let new_abs = new.unsigned_abs();
        let new_weight = loss_weight_for_basis(new_abs, old_leg.a_basis)?;
        let asset = &mut self.assets[asset_index];
        match old_leg.side {
            SideV14::Long => {
                asset.oi_eff_long_q = adjust_u128(asset.oi_eff_long_q, old_abs, new_abs)?;
                asset.loss_weight_sum_long =
                    adjust_u128(asset.loss_weight_sum_long, old_leg.loss_weight, new_weight)?;
            }
            SideV14::Short => {
                asset.oi_eff_short_q = adjust_u128(asset.oi_eff_short_q, old_abs, new_abs)?;
                asset.loss_weight_sum_short =
                    adjust_u128(asset.loss_weight_sum_short, old_leg.loss_weight, new_weight)?;
            }
        }
        account.legs[asset_index].basis_pos_q = new;
        account.legs[asset_index].loss_weight = new_weight;
        account.health_cert.valid = false;
        self.validate_account_shape(account)
    }

    fn reduce_position(
        &mut self,
        account: &mut PortfolioAccountV14,
        asset_index: usize,
        close_q: u128,
    ) -> V14Result<()> {
        if close_q == 0 {
            return Ok(());
        }
        let leg = account.legs[asset_index];
        if !leg.active {
            return Err(V14Error::InvalidLeg);
        }
        let close_i128 = i128::try_from(close_q).map_err(|_| V14Error::ArithmeticOverflow)?;
        let delta = match leg.side {
            SideV14::Long => close_i128
                .checked_neg()
                .ok_or(V14Error::ArithmeticOverflow)?,
            SideV14::Short => close_i128,
        };
        self.apply_position_delta(account, asset_index, delta)
    }

    fn set_account_pnl(
        &mut self,
        account: &mut PortfolioAccountV14,
        new_pnl: i128,
    ) -> V14Result<()> {
        validate_non_min_i128(new_pnl)?;
        let old_pos = account.pnl.max(0) as u128;
        let new_pos = new_pnl.max(0) as u128;
        if new_pos >= old_pos {
            self.pnl_pos_tot = self
                .pnl_pos_tot
                .checked_add(new_pos - old_pos)
                .ok_or(V14Error::ArithmeticOverflow)?;
            self.pnl_pos_bound_tot = self
                .pnl_pos_bound_tot
                .checked_add(new_pos - old_pos)
                .ok_or(V14Error::ArithmeticOverflow)?;
        } else {
            let decrease = old_pos - new_pos;
            self.pnl_pos_tot = self
                .pnl_pos_tot
                .checked_sub(decrease)
                .ok_or(V14Error::CounterUnderflow)?;
            self.pnl_pos_bound_tot = self.pnl_pos_bound_tot.saturating_sub(decrease);
            if self.pnl_pos_bound_tot < self.pnl_pos_tot {
                self.pnl_pos_bound_tot = self.pnl_pos_tot;
            }
            self.pnl_matured_pos_tot = self.pnl_matured_pos_tot.min(self.pnl_pos_tot);
        }

        let old_negative = account.pnl < 0;
        let new_negative = new_pnl < 0;
        match (old_negative, new_negative) {
            (false, true) => {
                self.negative_pnl_account_count = self
                    .negative_pnl_account_count
                    .checked_add(1)
                    .ok_or(V14Error::CounterOverflow)?;
            }
            (true, false) => {
                self.negative_pnl_account_count = self
                    .negative_pnl_account_count
                    .checked_sub(1)
                    .ok_or(V14Error::CounterUnderflow)?;
            }
            _ => {}
        }
        account.pnl = new_pnl;
        Ok(())
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccountBSettlementChunkV14 {
    pub delta_b: u128,
    pub loss: u128,
    pub new_remainder: u128,
    pub remaining_after: u128,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RiskScoreV14 {
    pub certified_liq_deficit: u128,
    pub unsettled_b_loss_bound: u128,
    pub stale_loss_bound: u128,
    pub gross_risk_notional: u128,
    pub active_leg_count: u32,
}

impl RiskScoreV14 {
    pub fn strictly_reduces_from(self, before: Self) -> bool {
        self < before
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionlessProgressOutcomeV14 {
    AccountCurrent,
    AccountBChunk(AccountBSettlementChunkV14),
    ResidualBooked(BResidualBookingOutcomeV14),
    RecoveryDeclared(PermissionlessRecoveryReasonV14),
}

pub fn risk_notional_ceil(abs_pos_q: u128, price: u64) -> V14Result<u128> {
    if abs_pos_q == 0 {
        return Ok(0);
    }
    checked_mul_div_ceil_u256(
        U256::from_u128(abs_pos_q),
        U256::from_u128(price as u128),
        U256::from_u128(POS_SCALE),
    )
    .and_then(|v| v.try_into_u128())
    .ok_or(V14Error::ArithmeticOverflow)
}

pub fn account_equity(account: &PortfolioAccountV14) -> V14Result<i128> {
    validate_non_min_i128(account.pnl)?;
    validate_fee_credits(account.fee_credits)?;
    let capital = i128::try_from(account.capital).map_err(|_| V14Error::ArithmeticOverflow)?;
    let fee_debt =
        i128::try_from(fee_debt_u128(account)?).map_err(|_| V14Error::ArithmeticOverflow)?;
    capital
        .checked_add(account.pnl)
        .and_then(|v| v.checked_sub(fee_debt))
        .ok_or(V14Error::ArithmeticOverflow)
}

fn account_no_positive_credit_equity(account: &PortfolioAccountV14) -> V14Result<i128> {
    validate_non_min_i128(account.pnl)?;
    validate_fee_credits(account.fee_credits)?;
    let capital = i128::try_from(account.capital).map_err(|_| V14Error::ArithmeticOverflow)?;
    let fee_debt =
        i128::try_from(fee_debt_u128(account)?).map_err(|_| V14Error::ArithmeticOverflow)?;
    capital
        .checked_add(account.pnl.min(0))
        .and_then(|v| v.checked_sub(fee_debt))
        .ok_or(V14Error::ArithmeticOverflow)
}

fn account_no_positive_credit_equity_with_capital(
    account: &PortfolioAccountV14,
    capital_override: u128,
) -> V14Result<i128> {
    validate_non_min_i128(account.pnl)?;
    validate_fee_credits(account.fee_credits)?;
    let capital = i128::try_from(capital_override).map_err(|_| V14Error::ArithmeticOverflow)?;
    let fee_debt =
        i128::try_from(fee_debt_u128(account)?).map_err(|_| V14Error::ArithmeticOverflow)?;
    capital
        .checked_add(account.pnl.min(0))
        .and_then(|v| v.checked_sub(fee_debt))
        .ok_or(V14Error::ArithmeticOverflow)
}

fn ensure_initial_margin(account: &PortfolioAccountV14) -> V14Result<()> {
    if !account.health_cert.valid {
        return Err(V14Error::Stale);
    }
    let equity = account.health_cert.certified_equity;
    if equity < 0 || (equity as u128) < account.health_cert.certified_initial_req {
        return Err(V14Error::InvalidConfig);
    }
    Ok(())
}

fn ensure_no_positive_credit_initial_margin(account: &PortfolioAccountV14) -> V14Result<()> {
    let equity = account_no_positive_credit_equity(account)?;
    if equity < 0 || (equity as u128) < account.health_cert.certified_initial_req {
        return Err(V14Error::LockActive);
    }
    Ok(())
}

fn position_delta_increases_risk(
    account: &PortfolioAccountV14,
    asset_index: usize,
    delta_q: i128,
) -> V14Result<bool> {
    let current = signed_position(account.legs[asset_index]);
    let next = current
        .checked_add(delta_q)
        .ok_or(V14Error::ArithmeticOverflow)?;
    validate_basis_or_zero(next)?;
    Ok(next.unsigned_abs() > current.unsigned_abs())
}

fn margin_requirement(notional: u128, bps: u64, floor: u128) -> V14Result<u128> {
    if notional == 0 {
        return Ok(0);
    }
    let raw = wide_mul_div_floor_u128(notional, bps as u128, MAX_MARGIN_BPS as u128);
    Ok(raw.max(floor))
}

fn trade_notional_floor(size_q: u128, exec_price: u64) -> V14Result<u128> {
    if size_q == 0 {
        return Ok(0);
    }
    let (q, _) = mul_div_floor_u256_with_rem(
        U256::from_u128(size_q),
        U256::from_u128(exec_price as u128),
        U256::from_u128(POS_SCALE),
    );
    q.try_into_u128().ok_or(V14Error::ArithmeticOverflow)
}

fn checked_fee_bps(notional: u128, fee_bps: u64) -> V14Result<u128> {
    if notional == 0 || fee_bps == 0 {
        return Ok(0);
    }
    checked_mul_div_ceil_u256(
        U256::from_u128(notional),
        U256::from_u128(fee_bps as u128),
        U256::from_u128(MAX_MARGIN_BPS as u128),
    )
    .and_then(|v| v.try_into_u128())
    .ok_or(V14Error::ArithmeticOverflow)
}

fn checked_i128_mul(a: i128, b: i128) -> V14Result<i128> {
    let out = a.checked_mul(b).ok_or(V14Error::ArithmeticOverflow)?;
    validate_non_min_i128(out)?;
    Ok(out)
}

fn add_non_min_i128(a: i128, b: i128) -> V14Result<i128> {
    let out = a.checked_add(b).ok_or(V14Error::ArithmeticOverflow)?;
    validate_non_min_i128(out)?;
    Ok(out)
}

fn adjust_u128(current: u128, old: u128, new: u128) -> V14Result<u128> {
    if new >= old {
        current
            .checked_add(new - old)
            .ok_or(V14Error::ArithmeticOverflow)
    } else {
        current
            .checked_sub(old - new)
            .ok_or(V14Error::CounterUnderflow)
    }
}

fn encode_bool(value: bool) -> u8 {
    if value {
        1
    } else {
        0
    }
}

fn decode_bool(value: u8) -> V14Result<bool> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(V14Error::InvalidConfig),
    }
}

fn encode_side(value: SideV14) -> u8 {
    match value {
        SideV14::Long => 0,
        SideV14::Short => 1,
    }
}

fn decode_side(value: u8) -> V14Result<SideV14> {
    match value {
        0 => Ok(SideV14::Long),
        1 => Ok(SideV14::Short),
        _ => Err(V14Error::InvalidConfig),
    }
}

fn encode_side_mode(value: SideModeV14) -> u8 {
    match value {
        SideModeV14::Normal => 0,
        SideModeV14::DrainOnly => 1,
        SideModeV14::ResetPending => 2,
    }
}

fn decode_side_mode(value: u8) -> V14Result<SideModeV14> {
    match value {
        0 => Ok(SideModeV14::Normal),
        1 => Ok(SideModeV14::DrainOnly),
        2 => Ok(SideModeV14::ResetPending),
        _ => Err(V14Error::InvalidConfig),
    }
}

fn encode_market_mode(value: MarketModeV14) -> u8 {
    match value {
        MarketModeV14::Live => 0,
        MarketModeV14::Resolved => 1,
        MarketModeV14::Recovery => 2,
    }
}

fn decode_market_mode(value: u8) -> V14Result<MarketModeV14> {
    match value {
        0 => Ok(MarketModeV14::Live),
        1 => Ok(MarketModeV14::Resolved),
        2 => Ok(MarketModeV14::Recovery),
        _ => Err(V14Error::InvalidConfig),
    }
}

fn encode_recovery_reason(value: PermissionlessRecoveryReasonV14) -> u8 {
    match value {
        PermissionlessRecoveryReasonV14::BelowProgressFloor => 0,
        PermissionlessRecoveryReasonV14::BlockedSegmentHeadroomOrRepresentability => 1,
        PermissionlessRecoveryReasonV14::AccountBSettlementCannotProgress => 2,
        PermissionlessRecoveryReasonV14::BIndexHeadroomExhausted => 3,
        PermissionlessRecoveryReasonV14::ActiveBankruptCloseCannotProgress => 4,
        PermissionlessRecoveryReasonV14::ExplicitLossOrDustAuditOverflow => 5,
        PermissionlessRecoveryReasonV14::OracleOrTargetUnavailableByAuthenticatedPolicy => 6,
        PermissionlessRecoveryReasonV14::CounterOrEpochOverflowDeclaredRecovery => 7,
    }
}

fn decode_recovery_reason(value: u8) -> V14Result<PermissionlessRecoveryReasonV14> {
    match value {
        0 => Ok(PermissionlessRecoveryReasonV14::BelowProgressFloor),
        1 => Ok(PermissionlessRecoveryReasonV14::BlockedSegmentHeadroomOrRepresentability),
        2 => Ok(PermissionlessRecoveryReasonV14::AccountBSettlementCannotProgress),
        3 => Ok(PermissionlessRecoveryReasonV14::BIndexHeadroomExhausted),
        4 => Ok(PermissionlessRecoveryReasonV14::ActiveBankruptCloseCannotProgress),
        5 => Ok(PermissionlessRecoveryReasonV14::ExplicitLossOrDustAuditOverflow),
        6 => Ok(PermissionlessRecoveryReasonV14::OracleOrTargetUnavailableByAuthenticatedPolicy),
        7 => Ok(PermissionlessRecoveryReasonV14::CounterOrEpochOverflowDeclaredRecovery),
        _ => Err(V14Error::InvalidConfig),
    }
}

fn validate_basis_or_zero(basis_pos_q: i128) -> V14Result<()> {
    if basis_pos_q == 0 {
        Ok(())
    } else {
        validate_basis(basis_pos_q)
    }
}

fn signed_position(leg: PortfolioLegV14) -> i128 {
    if !leg.active {
        0
    } else {
        match leg.side {
            SideV14::Long => leg.basis_pos_q.unsigned_abs() as i128,
            SideV14::Short => -(leg.basis_pos_q.unsigned_abs() as i128),
        }
    }
}

fn opposite_side(side: SideV14) -> SideV14 {
    match side {
        SideV14::Long => SideV14::Short,
        SideV14::Short => SideV14::Long,
    }
}

fn quarantine_remainder(remainder: &mut u128, dust: &mut u128) -> V14Result<()> {
    if *remainder == 0 {
        return Ok(());
    }
    let new_dust = dust
        .checked_add(*remainder)
        .ok_or(V14Error::ArithmeticOverflow)?;
    if new_dust >= SOCIAL_LOSS_DEN {
        return Err(V14Error::RecoveryRequired);
    }
    *dust = new_dust;
    *remainder = 0;
    Ok(())
}

fn validate_non_min_i128(v: i128) -> V14Result<()> {
    if v == i128::MIN {
        return Err(V14Error::ArithmeticOverflow);
    }
    Ok(())
}

fn validate_fee_credits(v: i128) -> V14Result<()> {
    validate_non_min_i128(v)?;
    if v > 0 {
        return Err(V14Error::InvalidLeg);
    }
    Ok(())
}

fn fee_debt_u128(account: &PortfolioAccountV14) -> V14Result<u128> {
    validate_fee_credits(account.fee_credits)?;
    Ok(account.fee_credits.unsigned_abs())
}

fn validate_basis(basis_pos_q: i128) -> V14Result<()> {
    if basis_pos_q == 0
        || basis_pos_q == i128::MIN
        || basis_pos_q.unsigned_abs() > MAX_POSITION_ABS_Q
    {
        return Err(V14Error::InvalidLeg);
    }
    Ok(())
}

fn validate_active_leg(leg: PortfolioLegV14) -> V14Result<()> {
    validate_basis(leg.basis_pos_q)?;
    validate_non_min_i128(leg.k_snap)?;
    validate_non_min_i128(leg.f_snap)?;
    if !(MIN_A_SIDE..=ADL_ONE).contains(&leg.a_basis)
        || leg.loss_weight == 0
        || leg.loss_weight != loss_weight_for_basis(leg.basis_pos_q.unsigned_abs(), leg.a_basis)?
        || leg.b_rem >= SOCIAL_LOSS_DEN
        || leg.b_epoch_snap != leg.epoch_snap
    {
        return Err(V14Error::InvalidLeg);
    }
    Ok(())
}

fn loss_weight_for_basis(abs_basis_q: u128, a_basis: u128) -> V14Result<u128> {
    if a_basis == 0 {
        return Err(V14Error::InvalidLeg);
    }
    checked_mul_div_ceil_u256(
        U256::from_u128(abs_basis_q),
        U256::from_u128(SOCIAL_WEIGHT_SCALE),
        U256::from_u128(a_basis),
    )
    .and_then(|v| v.try_into_u128())
    .ok_or(V14Error::ArithmeticOverflow)
}

fn has_b_stale_leg(account: &PortfolioAccountV14) -> bool {
    account.legs.iter().any(|leg| leg.active && leg.b_stale)
}

fn account_b_loss_bound(account: &PortfolioAccountV14) -> V14Result<u128> {
    let mut bound = 0u128;
    for leg in account.legs.iter() {
        if leg.active && leg.b_stale {
            bound = bound
                .checked_add(leg.loss_weight)
                .ok_or(V14Error::ArithmeticOverflow)?;
        }
    }
    Ok(bound)
}
