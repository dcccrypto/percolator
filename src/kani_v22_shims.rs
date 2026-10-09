//! kani v22-final: proof-only shims (cfg(kani) child module of v16; never in a program build).
//!
//! Every function here forwards to ONE private production function, with no logic of its own,
//! so the harnesses in `tests/proofs_v22_final.rs` prove the production code. They hang off
//! `MarketGroupV16ViewMut` (a pub type) because this module is private: associated functions
//! without `self` are the free-function shims (`MarketGroupV16ViewMut::<u64>::kani_v22_..`).
#![allow(dead_code)]
use super::*;

impl<'a, T> MarketGroupV16ViewMut<'a, T> {
    // ---- S10 ------------------------------------------------------------------------------
    /// E-S10-11: the retry hook (`s10_retry_for_unpositioned_asset`).
    pub fn kani_v22_s10_retry(&mut self, asset_index: usize, s10_budget: &mut u8) -> V16Result<()> {
        self.s10_retry_for_unpositioned_asset(asset_index, s10_budget)
    }

    /// E-S10-14: the bucket setter (wholly-empty mirror reset).
    pub fn kani_v22_set_backing_bucket_for_domain(
        &mut self,
        domain: usize,
        bucket: BackingBucketV16,
    ) -> V16Result<()> {
        self.set_backing_bucket_for_domain(domain, bucket)
    }

    /// Fixture helper: the canonical claim-bound -> amount conversion (`amount_from_bound_num`).
    pub fn kani_v22_amount_from_bound_num(bound_num: u128) -> V16Result<u128> {
        V16Core::amount_from_bound_num(bound_num)
    }

    /// Rate-model twin helper: the shape half of the expected-rate function.
    pub fn kani_v22_source_credit_shape_static(state: SourceCreditStateV16) -> V16Result<()> {
        V16Core::validate_source_credit_state_shape_static(state)
    }

    // ---- X1 / S9 --------------------------------------------------------------------------
    /// E-X1-5: the only pricing reader of `kf_pending_credit_*`.
    pub fn kani_v22_kf_pending_credit_num(&self, domain: usize) -> V16Result<u128> {
        self.kf_pending_credit_num(domain)
    }

    /// E-X1-1: the new fee charge on the liquidation path.
    pub fn kani_v22_charge_fee_after_full_refresh(
        &mut self,
        account: &mut PortfolioV16ViewMut<'_>,
        requested_fee: u128,
    ) -> V16Result<u128> {
        self.charge_account_fee_after_full_refresh_not_atomic(account, requested_fee)
    }

    /// E-X1-1: the reference fee charge (kept `#[allow(dead_code)]` in production for this proof).
    pub fn kani_v22_charge_fee_reference(
        &mut self,
        account: &mut PortfolioV16ViewMut<'_>,
        requested_fee: u128,
    ) -> V16Result<u128> {
        self.charge_account_fee_not_atomic(account, requested_fee)
    }

    /// E-S9-3: the S9 protective credit rate.
    pub fn kani_v22_source_credit_protective_rate(
        state: SourceCreditStateV16,
        pending_num: u128,
    ) -> V16Result<u128> {
        V16Core::source_credit_protective_rate(state, pending_num)
    }

    // ---- cap ------------------------------------------------------------------------------
    /// E-CAP-1 (rev 2.1 5b): batch-length refusal site `:22230` (the fork batch exists only with `fork-facade`).
    #[cfg(feature = "fork-facade")]
    pub fn kani_v22_fork_batch_after_tail_validation(
        &mut self,
        long_account: &mut PortfolioV16ViewMut<'_>,
        short_account: &mut PortfolioV16ViewMut<'_>,
        requests: &[TradeRequestV16],
        threshold_bps_opt: Option<u128>,
        taker_is_long_account: bool,
    ) -> V16Result<BatchTradeOutcomeV16> {
        self.fork_execute_batch_after_tail_validation_with_threshold_not_atomic(
            long_account,
            short_account,
            requests,
            threshold_bps_opt,
            taker_is_long_account,
        )
    }

    /// E-CAP-1 (rev 2.1 5b): batch-length refusal site `:22346`.
    pub fn kani_v22_batch_with_fee_after_tail_validation(
        &mut self,
        long_account: &mut PortfolioV16ViewMut<'_>,
        short_account: &mut PortfolioV16ViewMut<'_>,
        requests: &[TradeRequestV16],
        taker_is_long_account: bool,
    ) -> V16Result<BatchTradeOutcomeV16> {
        self.execute_batch_with_fee_after_tail_validation_not_atomic(
            long_account,
            short_account,
            requests,
            taker_is_long_account,
        )
    }

    // ---- ADL / leg remainders --------------------------------------------------------------
    /// E-ADL-1, E-REM-4: `V16Core::kernel_attach_leg`.
    #[allow(clippy::too_many_arguments)]
    pub fn kani_v22_attach_leg(
        asset: AssetStateV16,
        side: SideV16,
        basis_pos_q: i128,
        loss_weight: u128,
        asset_index_u32: u32,
        band_bps: u64,
        band_max_positions_per_side: u64,
    ) -> V16Result<(AssetStateV16, PortfolioLegV16)> {
        V16Core::kernel_attach_leg(
            asset,
            side,
            basis_pos_q,
            loss_weight,
            asset_index_u32,
            band_bps,
            band_max_positions_per_side,
        )
    }

    /// E-ADL-1, E-REM-4: `V16Core::kernel_resize_leg_same_side`.
    #[allow(clippy::too_many_arguments)]
    pub fn kani_v22_resize_leg_same_side(
        leg: PortfolioLegV16,
        asset: AssetStateV16,
        new_signed: i128,
        new_weight: u128,
        preserve_pending_obligation_weight: bool,
        old_effective_abs: u128,
        new_effective_abs: u128,
    ) -> V16Result<(PortfolioLegV16, AssetStateV16)> {
        V16Core::kernel_resize_leg_same_side(
            leg,
            asset,
            new_signed,
            new_weight,
            preserve_pending_obligation_weight,
            old_effective_abs,
            new_effective_abs,
        )
    }

    /// E-REM-1/2: the K/F fast path `scaled_adl_delta_with_carry_fast`.
    pub fn kani_v22_scaled_adl_delta_with_carry_fast(
        abs_basis_q: u128,
        a_basis: u128,
        then: i128,
        now: i128,
        carry: u128,
    ) -> Option<(i128, u128)> {
        scaled_adl_delta_with_carry_fast(abs_basis_q, a_basis, then, now, carry)
    }

    /// E-REM-3: the 7-tuple settlement producer (k_now, f_now, k_delta, f_delta, k_rem, f_rem, net).
    pub fn kani_v22_leg_kf_components(
        asset: AssetStateV16,
        leg: PortfolioLegV16,
    ) -> V16Result<(i128, i128, i128, i128, u128, u128, i128)> {
        Self::leg_kf_delta_components_for_settlement_from_asset(asset, leg)
    }

    /// E-REM-6 (#277): `V16Core::kernel_kf_hidden_loss_bound`.
    pub fn kani_v22_kf_hidden_loss_bound(stale: u64, drift: KfDriftSideV16) -> Option<u128> {
        V16Core::kernel_kf_hidden_loss_bound(stale, drift)
    }
}
