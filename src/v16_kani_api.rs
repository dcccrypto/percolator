//! `#[cfg(kani)]` test-facing wrapper API for the v16 engine.
//!
//! NON-PRODUCTION: compiled only under cfg(kani). These thin wrappers expose
//! private engine fns/methods to the Kani proof suite (tests/proofs_v16.rs).
//! Kept out of v16.rs to minimise the production audit surface; re-exported
//! from v16 so existing `percolator::v16::kani_*` paths keep resolving.
#![allow(unused_imports, clippy::too_many_arguments)]

use super::*;
use crate::wide_math::{U256};

pub fn kani_apply_backing_utilization_fee_charge(
    account_capital: u128,
    group_c_tot: u128,
    bucket_earnings: u128,
    account_pnl: i128,
    requested_fee: u128,
) -> V16Result<(u128, u128, u128, u128)> {
    apply_backing_utilization_fee_charge(
        account_capital,
        group_c_tot,
        bucket_earnings,
        account_pnl,
        requested_fee,
    )
}

pub fn kani_apply_backing_provider_earnings_withdraw(
    vault: u128,
    bucket_earnings: u128,
    amount: u128,
) -> V16Result<(u128, u128)> {
    apply_backing_provider_earnings_withdraw(vault, bucket_earnings, amount)
}

pub fn kani_health_cert_after_capital_debit(
    cert: HealthCertV16,
    amount: u128,
) -> V16Result<HealthCertV16> {
    health_cert_after_capital_debit(cert, amount)
}

pub fn kani_active_bitmap_set(
    bitmap: &mut V16ActiveBitmap,
    leg_slot_index: usize,
) -> V16Result<()> {
    active_bitmap_set(bitmap, leg_slot_index)
}

pub fn kani_liquidation_close_would_leave_uncovered_loss_with_open_risk(
    pnl: i128,
    capital: u128,
    active_bitmap: V16ActiveBitmap,
    leg_slot_index: usize,
    close_q: u128,
    leg_abs_q: u128,
) -> V16Result<bool> {
    liquidation_close_would_leave_uncovered_loss_with_open_risk(
        pnl,
        capital,
        active_bitmap,
        leg_slot_index,
        close_q,
        leg_abs_q,
    )
}

pub fn kani_add_open_interest_for_new_position(
    asset: &mut AssetStateV16,
    side: SideV16,
    abs_q: u128,
    loss_weight: u128,
) -> V16Result<()> {
    add_open_interest_for_new_position(asset, side, abs_q, loss_weight)
}

pub fn kani_validate_positive_pnl_source_attribution(
    pnl: i128,
    source_claim_sum_num: u128,
) -> V16Result<()> {
    V16Core::validate_positive_pnl_source_attribution(pnl, source_claim_sum_num)
}

pub fn kani_expected_source_credit_rate_num_for_state(
    state: SourceCreditStateV16,
) -> V16Result<u128> {
    V16Core::expected_source_credit_rate_num_for_state(state)
}

pub fn kani_available_backing_num_for_source_credit_state(
    state: SourceCreditStateV16,
) -> V16Result<u128> {
    V16Core::available_backing_num_for_source_credit_state(state)
}

pub fn kani_loss_stale_trade_scope_allowed(
    market_loss_stale_active: bool,
    trade_asset_loss_stale: bool,
    long_account_loss_stale_exposed: bool,
    short_account_loss_stale_exposed: bool,
) -> bool {
    V16Core::loss_stale_trade_scope_allowed(
        market_loss_stale_active,
        trade_asset_loss_stale,
        long_account_loss_stale_exposed,
        short_account_loss_stale_exposed,
    )
}

pub fn kani_prepare_asset_recovery_transition(
    asset: AssetStateV16,
    asset_set_epoch: u64,
    risk_epoch: u64,
) -> V16Result<(AssetStateV16, u64, u64)> {
    V16Core::prepare_asset_recovery_transition(asset, asset_set_epoch, risk_epoch)
}

pub fn kani_source_credit_state_realizable_support_for_face(
    state: SourceCreditStateV16,
    face_claim: u128,
) -> V16Result<u128> {
    V16Core::source_credit_state_realizable_support_for_face(state, face_claim)
}

pub fn kani_backing_utilization_rate_e9_for_source_state(
    config: V16Config,
    source: SourceCreditStateV16,
) -> V16Result<u64> {
    V16Core::backing_utilization_rate_e9_for_source_state(config, source)
}

pub fn kani_backing_utilization_fee_quote_atoms_for_lien(
    config: V16Config,
    source: SourceCreditStateV16,
    lien_backing_num: u128,
    from_slot: u64,
    to_slot: u64,
) -> V16Result<u128> {
    V16Core::backing_utilization_fee_quote_atoms_for_lien(
        config,
        source,
        lien_backing_num,
        from_slot,
        to_slot,
    )
}

pub fn kani_target_effective_lag_adverse_delta(
    side: SideV16,
    effective_price: u64,
    raw_target_price: u64,
) -> u64 {
    V16Core::target_effective_lag_adverse_delta(side, effective_price, raw_target_price)
}

pub fn kani_health_requirements_from_base_and_target_lag(
    base_initial: u128,
    base_maintenance: u128,
    risk_notional: u128,
    target_lag_penalty: u128,
) -> V16Result<(u128, u128, u128)> {
    V16Core::health_requirements_from_base_and_target_lag(
        base_initial,
        base_maintenance,
        risk_notional,
        target_lag_penalty,
    )
}

pub fn kani_apply_resolved_payout_receipt_payment(
    receipt: ResolvedPayoutReceiptV16,
    actual_resolved_paid: u128,
) -> V16Result<ResolvedPayoutReceiptV16> {
    apply_resolved_payout_receipt_payment(receipt, actual_resolved_paid)
}

pub fn kani_risk_notional_ceil(abs_pos_q: u128, price: u64) -> V16Result<u128> {
    risk_notional_ceil(abs_pos_q, price)
}

pub fn kani_position_delta_increases_risk(current: i128, delta_q: i128) -> V16Result<bool> {
    position_delta_increases_risk(current, delta_q)
}

pub fn kani_trade_preflight_risk_gate(
    risk_increasing: bool,
    asset_loss_stale: bool,
    target_effective_lag: bool,
    touches_pending_domain_barrier: bool,
) -> V16Result<()> {
    trade_preflight_risk_gate(
        risk_increasing,
        asset_loss_stale,
        target_effective_lag,
        touches_pending_domain_barrier,
    )
}

pub fn kani_trade_notional_floor(size_q: u128, exec_price: u64) -> V16Result<u128> {
    trade_notional_floor(size_q, exec_price)
}

pub fn kani_checked_fee_bps(notional: u128, fee_bps: u64) -> V16Result<u128> {
    checked_fee_bps(notional, fee_bps)
}

pub fn kani_adjust_u128(current: u128, old: u128, new: u128) -> V16Result<u128> {
    adjust_u128(current, old, new)
}

pub fn kani_pending_domain_loss_barrier_blocks_position_change(
    touches_barrier: bool,
    current: i128,
    next: i128,
) -> bool {
    pending_domain_loss_barrier_blocks_position_change(touches_barrier, current, next)
}

pub fn kani_scaled_adl_delta_fast(
    abs_basis_q: u128,
    a_basis: u128,
    then: i128,
    now: i128,
) -> Option<i128> {
    scaled_adl_delta_fast(abs_basis_q, a_basis, then, now)
}

impl V16Config {
        pub fn kani_solvency_envelope_holds_for_notional(&self, n: u128) -> V16Result<bool> {
        self.validate_funding_headroom(self.max_accrual_dt_slots)?;
        self.validate_funding_headroom(self.min_funding_lifetime_slots)?;
        let price_budget_bps = (self.max_price_move_bps_per_slot as u128)
            .checked_mul(self.max_accrual_dt_slots as u128)
            .ok_or(V16Error::InvalidConfig)?;
        let funding_budget_num = (self.max_abs_funding_e9_per_slot as u128)
            .checked_mul(self.max_accrual_dt_slots as u128)
            .and_then(|v| v.checked_mul(10_000))
            .ok_or(V16Error::InvalidConfig)?;
        let loss_budget_num = price_budget_bps
            .checked_mul(FUNDING_DEN)
            .and_then(|v| v.checked_add(funding_budget_num))
            .ok_or(V16Error::InvalidConfig)?;
        let loss_budget_den = 10_000u128
            .checked_mul(FUNDING_DEN)
            .ok_or(V16Error::InvalidConfig)?;
        self.solvency_envelope_holds_for_notional(
            n,
            loss_budget_num,
            loss_budget_den,
            price_budget_bps,
        )
    }

}

impl<'a> PortfolioV16View<'a> {
        pub fn kani_source_domain_slot(&self, domain: usize) -> V16Result<Option<usize>> {
        self.source_domain_slot(domain)
    }

        pub fn kani_source_domain(&self, domain: usize) -> V16Result<PortfolioSourceDomainV16Account> {
        self.source_domain(domain)
    }

        pub fn kani_validate_source_credit_shape_with_market<T>(
        &self,
        market: &MarketGroupV16View<'_, T>,
    ) -> V16Result<()> {
        self.validate_source_credit_shape_with_market(market)
    }

        pub fn kani_active_leg_slot_for_asset(&self, asset_index: usize) -> V16Result<Option<usize>> {
        self.active_leg_slot_for_asset(asset_index)
    }

}

impl<'a> PortfolioV16ViewMut<'a> {
        pub fn kani_source_domain_slot_or_insert(&mut self, domain: usize) -> V16Result<usize> {
        self.source_domain_slot_or_insert(domain)
    }

}

impl MarketGroupV16HeaderAccount {
        pub fn kani_dynamic_asset_slot_stride<T: MarketWrapperPod>() -> usize {
        Self::dynamic_asset_slot_stride::<T>()
    }

        pub fn kani_validate_dynamic_market_slots_len(
        supplied_len: usize,
        capacity: usize,
        configured_market_slots: usize,
    ) -> V16Result<()> {
        Self::validate_dynamic_market_slots_len_static(
            supplied_len,
            capacity,
            configured_market_slots,
        )
    }

    #[cfg(kani)]
        pub fn kani_validate_dynamic_market_slot_shape_at<S: MarketSlotV16View>(
        &self,
        slot_index: usize,
        slot: &S,
    ) -> V16Result<()> {
        self.validate_dynamic_market_slot_shape_at(slot_index, slot)
    }

}

impl<'a, T> MarketGroupV16ViewMut<'a, T> {
        pub fn kani_residual(&self) -> u128 {
        self.residual()
    }

        pub fn kani_domain_asset_side(&self, domain: usize) -> V16Result<(usize, SideV16)> {
        self.domain_asset_side(domain)
    }

        pub fn kani_insurance_domain_index(
        &self,
        asset_index: usize,
        side: SideV16,
    ) -> V16Result<usize> {
        self.insurance_domain_index(asset_index, side)
    }

        pub fn kani_backing_bucket_for_domain(&self, domain: usize) -> V16Result<BackingBucketV16> {
        self.backing_bucket_for_domain(domain)
    }

        pub fn kani_validate_source_domain_ledger_current(&self, domain: usize) -> V16Result<()> {
        self.validate_source_domain_ledger_current(domain)
    }

        pub fn kani_credit_backing_provider_earnings_delta(
        vault: u128,
        c_tot: u128,
        insurance: u128,
        earnings_total: u128,
        bucket_earnings: u128,
        amount: u128,
    ) -> V16Result<(u128, u128)> {
        Self::credit_backing_provider_earnings_delta(
            vault,
            c_tot,
            insurance,
            earnings_total,
            bucket_earnings,
            amount,
        )
    }

        pub fn kani_burn_impaired_account_source_claim_fields(
        account: &mut PortfolioV16ViewMut<'_>,
        slot: usize,
        burn_num: u128,
    ) -> V16Result<(u128, u128)> {
        Self::burn_impaired_account_source_claim_fields(account, slot, burn_num)
    }

        pub fn kani_set_domain_insurance_spent_delta(
        total_remaining: u128,
        insurance: u128,
        budget: u128,
        old_spent: u128,
        new_spent: u128,
    ) -> V16Result<u128> {
        Self::set_domain_insurance_spent_delta(
            total_remaining,
            insurance,
            budget,
            old_spent,
            new_spent,
        )
    }

        pub fn kani_set_domain_insurance_budget_delta(
        total_remaining: u128,
        insurance_limit: u128,
        old_budget: u128,
        spent: u128,
        new_budget: u128,
    ) -> V16Result<u128> {
        Self::set_domain_insurance_budget_delta(
            total_remaining,
            insurance_limit,
            old_budget,
            spent,
            new_budget,
        )
    }

        pub fn kani_withdraw_domain_insurance_delta(
        vault: u128,
        insurance: u128,
        source_reserved_atoms: u128,
        budget: u128,
        spent: u128,
        domain_reserved_atoms: u128,
        amount: u128,
    ) -> V16Result<(u128, u128, u128)> {
        Self::withdraw_domain_insurance_delta(
            vault,
            insurance,
            source_reserved_atoms,
            budget,
            spent,
            domain_reserved_atoms,
            amount,
        )
    }

        pub fn kani_credit_account_from_insurance_delta(
        insurance: u128,
        budget_remaining: u128,
        c_tot: u128,
        capital: u128,
        amount: u128,
    ) -> V16Result<(u128, u128, u128)> {
        Self::credit_account_from_insurance_delta(
            insurance,
            budget_remaining,
            c_tot,
            capital,
            amount,
        )
    }

        pub fn kani_consume_domain_insurance_for_negative_pnl(
        &mut self,
        asset_index: usize,
        bankrupt_side: SideV16,
        account: &mut PortfolioV16ViewMut<'_>,
    ) -> V16Result<u128> {
        self.consume_domain_insurance_for_negative_pnl(asset_index, bankrupt_side, account)
    }

        pub fn kani_preflight_liquidation_residual_durability(
        &mut self,
        asset_index: usize,
        bankrupt_side: SideV16,
        account: &PortfolioV16View<'_>,
    ) -> V16Result<()> {
        self.preflight_liquidation_residual_durability(asset_index, bankrupt_side, account)
    }

        pub fn kani_apply_counterparty_source_credit_lien_delta(
        source: &mut PortfolioSourceDomainV16Account,
        required_face_num: u128,
        required_backing_num: u128,
        effective_credit: u128,
        current_slot: u64,
    ) -> V16Result<()> {
        Self::apply_account_source_credit_lien_delta(
            source,
            SourceCreditBackingSourceV16::Counterparty,
            required_face_num,
            required_backing_num,
            effective_credit,
            current_slot,
        )
    }

        pub fn kani_prepare_counterparty_lien_create_delta(
        bucket: BackingBucketV16,
        source: SourceCreditStateV16,
        current_slot: u64,
        amount: u128,
    ) -> V16Result<(BackingBucketV16, SourceCreditStateV16)> {
        V16Core::prepare_counterparty_lien_create_delta(bucket, source, current_slot, amount)
    }

        pub fn kani_prepare_counterparty_lien_consume_delta(
        bucket: BackingBucketV16,
        source: SourceCreditStateV16,
        amount: u128,
    ) -> V16Result<(BackingBucketV16, SourceCreditStateV16)> {
        V16Core::prepare_counterparty_lien_consume_delta(bucket, source, amount)
    }

        pub fn kani_prepare_counterparty_lien_terminal_release_delta(
        bucket: BackingBucketV16,
        source: SourceCreditStateV16,
        amount: u128,
    ) -> V16Result<(BackingBucketV16, SourceCreditStateV16)> {
        V16Core::prepare_counterparty_lien_terminal_release_delta(bucket, source, amount)
    }

        pub fn kani_prepare_counterparty_backing_add_delta(
        bucket: BackingBucketV16,
        source: SourceCreditStateV16,
        amount: u128,
        current_slot: u64,
        expiry_slot: u64,
    ) -> V16Result<(BackingBucketV16, SourceCreditStateV16)> {
        V16Core::prepare_counterparty_backing_add_delta(
            bucket,
            source,
            amount,
            current_slot,
            expiry_slot,
        )
    }

        pub fn kani_prepare_counterparty_backing_withdraw_delta(
        bucket: BackingBucketV16,
        source: SourceCreditStateV16,
        amount: u128,
    ) -> V16Result<(BackingBucketV16, SourceCreditStateV16)> {
        V16Core::prepare_counterparty_backing_withdraw_delta(bucket, source, amount)
    }

        pub fn kani_source_credit_lien_amounts_for_effective(
        effective_credit: u128,
        credit_rate_num: u128,
    ) -> V16Result<(u128, u128)> {
        V16Core::source_credit_lien_amounts_for_effective(effective_credit, credit_rate_num)
    }

        pub fn kani_counterparty_cure_atoms_from_scaled_backing(amount: u128) -> V16Result<u128> {
        V16Core::validate_bound_num_atom_aligned(amount)?;
        Ok(amount / BOUND_SCALE)
    }

        pub fn kani_prepare_insurance_lien_consume_delta(
        reservation: InsuranceCreditReservationV16,
        source: SourceCreditStateV16,
        domain_spent: u128,
        insurance: u128,
        amount: u128,
    ) -> V16Result<(
        InsuranceCreditReservationV16,
        SourceCreditStateV16,
        u128,
        u128,
    )> {
        V16Core::prepare_insurance_lien_consume_delta(
            reservation,
            source,
            domain_spent,
            insurance,
            amount,
        )
    }

        pub fn kani_prepare_insurance_lien_terminal_release_delta(
        reservation: InsuranceCreditReservationV16,
        source: SourceCreditStateV16,
        amount: u128,
    ) -> V16Result<(InsuranceCreditReservationV16, SourceCreditStateV16)> {
        V16Core::prepare_insurance_lien_terminal_release_delta(reservation, source, amount)
    }

        pub fn kani_apply_insurance_lien_consume_domain_delta(
        &mut self,
        domain: usize,
        amount: u128,
    ) -> V16Result<()> {
        self.domain_asset_side(domain)?;
        if amount == 0 {
            return Ok(());
        }
        let (reservation, source, next_domain_spent, next_insurance) =
            V16Core::prepare_insurance_lien_consume_delta(
                self.insurance_reservation_for_domain(domain)?,
                self.source_credit_for_domain(domain)?,
                self.domain_insurance_budget_spent(domain)?.1,
                self.header.insurance.get(),
                amount,
            )?;
        let spend_atoms = self
            .header
            .insurance
            .get()
            .checked_sub(next_insurance)
            .ok_or(V16Error::CounterUnderflow)?;
        let vault_before = self.header.vault.get();
        let (source, next_risk_epoch) = V16Core::prepare_source_credit_domain_recompute_for_epoch(
            source,
            self.header.risk_epoch.get(),
        )?;
        TokenValueFlowProofV16::validate_insurance_to_close_insurance_spent(
            spend_atoms,
            vault_before,
            self.header.vault.get(),
        )?;
        self.set_insurance_reservation_for_domain(domain, reservation)?;
        self.set_source_credit_for_domain(domain, source)?;
        self.header.insurance = V16PodU128::new(next_insurance);
        self.set_domain_insurance_spent_core(domain, next_domain_spent)?;
        self.header.risk_epoch = V16PodU64::new(next_risk_epoch);
        Ok(())
    }

        pub fn kani_create_initial_margin_source_lien_if_needed(
        &mut self,
        account: &mut PortfolioV16ViewMut<'_>,
    ) -> V16Result<()> {
        self.create_initial_margin_source_lien_if_needed(account)
    }

        pub fn kani_transfer_account_residual_reward_credit(
        trader: &mut PortfolioV16ViewMut<'_>,
        lp: &mut PortfolioV16ViewMut<'_>,
        principal_atoms: u128,
    ) -> V16Result<u128> {
        Self::transfer_account_residual_reward_credit(trader, lp, principal_atoms)
    }

        pub fn kani_set_account_pnl(
        &mut self,
        account: &mut PortfolioV16ViewMut<'_>,
        new_pnl: i128,
    ) -> V16Result<()> {
        self.set_account_pnl(account, new_pnl)
    }

        pub fn kani_apply_signed_kf_delta_to_pnl(
        &mut self,
        account: &mut PortfolioV16ViewMut<'_>,
        delta: i128,
        source_domain: Option<usize>,
    ) -> V16Result<(u128, u128)> {
        let out = self.apply_signed_kf_delta_to_pnl(account, delta, source_domain)?;
        Ok((out.support_consumed, out.junior_face_burned))
    }

        pub fn kani_account_unliened_source_realizable_support(
        &self,
        account: &PortfolioV16View<'_>,
        face_claim: u128,
    ) -> V16Result<u128> {
        self.account_unliened_source_realizable_support(account, face_claim)
    }

        pub fn kani_reserve_new_capital_backed_loss_for_source_domain_not_atomic(
        &mut self,
        account: &mut PortfolioV16ViewMut<'_>,
        domain: usize,
        negative_before: u128,
        negative_after: u128,
    ) -> V16Result<()> {
        self.reserve_new_capital_backed_loss_for_source_domain_not_atomic(
            account,
            domain,
            negative_before,
            negative_after,
        )
    }

    #[cfg(kani)]
        pub fn kani_leg_kf_delta_for_settlement(
        &self,
        leg: PortfolioLegV16,
    ) -> V16Result<(i128, i128, i128)> {
        self.leg_kf_delta_for_settlement(leg)
    }

        pub fn kani_collect_account_backing_utilization_fee_for_domain_not_atomic(
        &mut self,
        account: &mut PortfolioV16ViewMut<'_>,
        domain: usize,
    ) -> V16Result<u128> {
        self.collect_account_backing_utilization_fee_for_domain_not_atomic(account, domain)
    }

        pub fn kani_asset_restart_next_counters(
        next_market_id_before: u64,
        activation_count_before: u64,
        asset_set_epoch_before: u64,
        risk_epoch_before: u64,
    ) -> V16Result<(u64, u64, u64, u64)> {
        Self::asset_restart_next_counters(
            next_market_id_before,
            activation_count_before,
            asset_set_epoch_before,
            risk_epoch_before,
        )
    }

        pub fn kani_restarted_asset_slot_preserving_insurance_budget(
        old_slot: &EngineAssetSlotV16Account,
        market_id: u64,
        authenticated_price: u64,
        now_slot: u64,
    ) -> EngineAssetSlotV16Account {
        Self::restarted_asset_slot_preserving_insurance_budget(
            old_slot,
            market_id,
            authenticated_price,
            now_slot,
        )
    }

        pub fn kani_canonical_retired_asset_slot(
        old_asset: AssetStateV16,
    ) -> EngineAssetSlotV16Account {
        Self::canonical_retired_asset_slot(old_asset)
    }

        pub fn kani_convert_source_claim_exposure_guard(
        &self,
        account: &PortfolioV16View<'_>,
    ) -> V16Result<bool> {
        Ok(Self::account_has_source_claims(account)?
            && self.account_has_active_source_claim_exposure(account)?)
    }

        pub fn kani_position_change_touches_pending_domain_loss_barrier(
        &self,
        asset_index: usize,
        current: i128,
        next: i128,
    ) -> V16Result<bool> {
        self.position_change_touches_pending_domain_loss_barrier(asset_index, current, next)
    }

        pub fn kani_h_lock_lane(
        &self,
        account: Option<&PortfolioV16View<'_>>,
        instruction_bankruptcy_candidate: bool,
    ) -> V16Result<HLockLaneV16> {
        self.h_lock_lane(account, instruction_bankruptcy_candidate)
    }

        pub fn kani_can_ignore_unrelated_loss_stale_for_trade(
        &self,
        long_account: &PortfolioV16View<'_>,
        short_account: &PortfolioV16View<'_>,
        asset_index: usize,
    ) -> V16Result<bool> {
        self.can_ignore_unrelated_loss_stale_for_trade(long_account, short_account, asset_index)
    }

        pub fn kani_trade_signed_size_deltas(size_q: i128) -> V16Result<(u128, i128, i128)> {
        Self::trade_signed_size_deltas(size_q)
    }

        pub fn kani_ensure_close_progress_not_expired(
        &mut self,
        ledger: CloseProgressLedgerV16,
    ) -> V16Result<()> {
        self.ensure_close_progress_not_expired(ledger)
    }

        pub fn kani_bankruptcy_residual_single_step_capacity(
        &self,
        asset_index: usize,
        bankrupt_side: SideV16,
        residual_remaining: u128,
    ) -> V16Result<u128> {
        self.bankruptcy_residual_single_step_capacity(
            asset_index,
            bankrupt_side,
            residual_remaining,
        )
    }

        pub fn kani_book_bankruptcy_residual_chunk_internal(
        &mut self,
        asset_index: usize,
        bankrupt_side: SideV16,
        residual_remaining: u128,
    ) -> V16Result<BResidualBookingOutcomeV16> {
        self.book_bankruptcy_residual_chunk_internal(asset_index, bankrupt_side, residual_remaining)
    }

        pub fn kani_apply_bankruptcy_residual_chunk_to_loss_side(
        asset: &mut AssetStateV16,
        opp: SideV16,
        engine_chunk: u128,
        residual_remaining: u128,
    ) -> V16Result<Option<BResidualBookingOutcomeV16>> {
        Self::apply_bankruptcy_residual_chunk_to_loss_side(
            asset,
            opp,
            engine_chunk,
            residual_remaining,
        )
    }

        pub fn kani_ensure_initial_margin(account: &PortfolioV16View<'_>) -> V16Result<()> {
        Self::ensure_initial_margin(account)
    }

        pub fn kani_ensure_no_positive_credit_initial_margin(
        account: &PortfolioV16View<'_>,
    ) -> V16Result<()> {
        Self::ensure_no_positive_credit_initial_margin(account)
    }

        pub fn kani_apply_trade_after_refresh_not_atomic(
        &mut self,
        long_account: &mut PortfolioV16ViewMut<'_>,
        short_account: &mut PortfolioV16ViewMut<'_>,
        request: TradeRequestV16,
        recertify_after_fill: bool,
    ) -> V16Result<(u128, u128, u128, bool)> {
        let out = self.apply_trade_after_refresh_not_atomic(
            long_account,
            short_account,
            request,
            recertify_after_fill,
        )?;
        Ok((out.fee_a, out.fee_b, out.notional, out.risk_increasing))
    }

        pub fn kani_accumulate_batch_trade_apply(
        outcome: &mut BatchTradeOutcomeV16,
        risk_increasing: &mut bool,
        long_has_source_claims: &mut bool,
        short_has_source_claims: &mut bool,
        fee_a: u128,
        fee_b: u128,
        notional: u128,
        applied_risk_increasing: bool,
        applied_long_has_source_claims: bool,
        applied_short_has_source_claims: bool,
    ) -> V16Result<()> {
        Self::accumulate_batch_trade_apply(
            outcome,
            risk_increasing,
            long_has_source_claims,
            short_has_source_claims,
            TradeApplyOutcomeV16 {
                fee_a,
                fee_b,
                notional,
                risk_increasing: applied_risk_increasing,
                long_has_source_claims: applied_long_has_source_claims,
                short_has_source_claims: applied_short_has_source_claims,
            },
        )
    }

        pub fn kani_charge_account_fee_current_not_atomic(
        &mut self,
        account: &mut PortfolioV16ViewMut<'_>,
        requested_fee: u128,
    ) -> V16Result<u128> {
        self.charge_account_fee_current_not_atomic(account, requested_fee)
    }

        pub fn kani_settle_negative_pnl_from_principal_core_not_atomic(
        &mut self,
        account: &mut PortfolioV16ViewMut<'_>,
    ) -> V16Result<u128> {
        self.settle_negative_pnl_from_principal_core_not_atomic(account)
    }

        pub fn kani_resolved_receipt_claimable_against_ledger(
        receipt: ResolvedPayoutReceiptV16,
        ledger: ResolvedPayoutLedgerV16,
    ) -> V16Result<u128> {
        Self::resolved_receipt_claimable_against_ledger(receipt, ledger)
    }

        pub fn kani_realize_source_backed_claims_for_resolved_close_not_atomic(
        &mut self,
        account: &mut PortfolioV16ViewMut<'_>,
    ) -> V16Result<u128> {
        self.realize_source_backed_claims_for_resolved_close_not_atomic(account)
    }

        pub fn kani_claim_resolved_payout_topup_core_not_atomic(
        &mut self,
        account: &mut PortfolioV16ViewMut<'_>,
    ) -> V16Result<u128> {
        self.claim_resolved_payout_topup_core_not_atomic(account)
    }


    pub fn kani_begin_close_progress_ledger(
        &mut self,
        account: &mut PortfolioV16ViewMut<'_>,
        asset_index: usize,
        domain_side: SideV16,
        gross_loss: u128,
    ) -> V16Result<()> {
        self.begin_close_progress_ledger(account, asset_index, domain_side, gross_loss)
    }
}

impl PortfolioSourceDomainV16Account {
        pub fn kani_is_sparse_tail_default(self) -> bool {
        self.is_sparse_tail_default()
    }

}

pub fn kani_margin_requirement(
    notional: u128,
    margin_bps: u64,
    min_nonzero_req: u128,
) -> V16Result<u128> {
    margin_requirement(notional, margin_bps, min_nonzero_req)
}

