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

    pub fn kani_clear_leg(
        &mut self,
        account: &mut PortfolioV16ViewMut<'_>,
        asset_index: usize,
    ) -> V16Result<()> {
        self.clear_leg(account, asset_index)
    }

    pub fn kani_attach_leg_at_slot(
        &mut self,
        account: &mut PortfolioV16ViewMut<'_>,
        asset_index: usize,
        side: SideV16,
        basis_pos_q: i128,
        leg_slot: usize,
    ) -> V16Result<()> {
        self.attach_leg_at_slot(account, asset_index, side, basis_pos_q, leg_slot)
    }
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

// ============ EXACT-FRAME EQUALITY HELPERS (E2E program) ============
// Loop-free-or-index-loop whole-struct equality (NEVER derived PartialEq:
// [u128; N] and large pod structs lower to builtin memcmp, which blows the
// unwind budget). Used by the exact-frame proofs: after an op, the ENTIRE
// state must equal the pre-state except the op's declared deltas.

pub fn kani_eq_v16_config_account(a: &V16ConfigAccount, b: &V16ConfigAccount) -> bool {
    a.max_portfolio_assets.get() == b.max_portfolio_assets.get()
        && a.max_market_slots.get() == b.max_market_slots.get()
        && a.min_nonzero_mm_req.get() == b.min_nonzero_mm_req.get()
        && a.min_nonzero_im_req.get() == b.min_nonzero_im_req.get()
        && a.h_min.get() == b.h_min.get()
        && a.h_max.get() == b.h_max.get()
        && a.maintenance_margin_bps.get() == b.maintenance_margin_bps.get()
        && a.initial_margin_bps.get() == b.initial_margin_bps.get()
        && a.max_trading_fee_bps.get() == b.max_trading_fee_bps.get()
        && a.liquidation_fee_bps.get() == b.liquidation_fee_bps.get()
        && a.liquidation_fee_cap.get() == b.liquidation_fee_cap.get()
        && a.min_liquidation_abs.get() == b.min_liquidation_abs.get()
        && a.max_accrual_dt_slots.get() == b.max_accrual_dt_slots.get()
        && a.max_abs_funding_e9_per_slot.get() == b.max_abs_funding_e9_per_slot.get()
        && a.min_funding_lifetime_slots.get() == b.min_funding_lifetime_slots.get()
        && a.max_price_move_bps_per_slot.get() == b.max_price_move_bps_per_slot.get()
        && a.max_account_b_settlement_chunks.get() == b.max_account_b_settlement_chunks.get()
        && a.max_bankrupt_close_chunks.get() == b.max_bankrupt_close_chunks.get()
        && a.max_bankrupt_close_lifetime_slots.get() == b.max_bankrupt_close_lifetime_slots.get()
        && a.asset_activation_cooldown_slots.get() == b.asset_activation_cooldown_slots.get()
        && a.public_b_chunk_atoms.get() == b.public_b_chunk_atoms.get()
        && a.max_recovery_fallback_deviation_bps.get() == b.max_recovery_fallback_deviation_bps.get()
        && a.backing_fee_base_rate_e9_per_slot.get() == b.backing_fee_base_rate_e9_per_slot.get()
        && a.backing_fee_kink_util_bps.get() == b.backing_fee_kink_util_bps.get()
        && a.backing_fee_slope_at_kink_e9_per_slot.get() == b.backing_fee_slope_at_kink_e9_per_slot.get()
        && a.backing_fee_slope_above_kink_e9_per_slot.get() == b.backing_fee_slope_above_kink_e9_per_slot.get()
        && a.backing_freshness_buckets == b.backing_freshness_buckets
        && a.margin_mode_realizable_full_shared_cross_margin == b.margin_mode_realizable_full_shared_cross_margin
        && a.source_credit_lien_required == b.source_credit_lien_required
        && a.insurance_credit_reservation_required == b.insurance_credit_reservation_required
        && a.permissionless_recovery_enabled == b.permissionless_recovery_enabled
        && a.recovery_fallback_price_enabled == b.recovery_fallback_price_enabled
        && a.recovery_fallback_envelope_enabled == b.recovery_fallback_envelope_enabled
        && a.credit_lien_revalidation_required == b.credit_lien_revalidation_required
        && a.stale_certificate_penalty_enabled == b.stale_certificate_penalty_enabled
        && a.full_refresh_required_for_favorable_actions == b.full_refresh_required_for_favorable_actions
        && a.public_liveness_profile_crank_forward == b.public_liveness_profile_crank_forward
}

pub fn kani_eq_v16_optional_recovery_reason_account(a: &V16OptionalRecoveryReasonAccount, b: &V16OptionalRecoveryReasonAccount) -> bool {
    a.present == b.present
        && a.value == b.value
}

pub fn kani_eq_resolved_payout_ledger_v16_account(a: &ResolvedPayoutLedgerV16Account, b: &ResolvedPayoutLedgerV16Account) -> bool {
    a.snapshot_residual.get() == b.snapshot_residual.get()
        && a.terminal_claim_exact_receipts_num.get() == b.terminal_claim_exact_receipts_num.get()
        && a.terminal_claim_bound_unreceipted_num.get() == b.terminal_claim_bound_unreceipted_num.get()
        && a.current_payout_rate_num.get() == b.current_payout_rate_num.get()
        && a.current_payout_rate_den.get() == b.current_payout_rate_den.get()
        && a.snapshot_slot.get() == b.snapshot_slot.get()
        && a.payout_halted == b.payout_halted
        && a.finalized == b.finalized
}

pub fn kani_eq_market_group_v16_header_account(a: &MarketGroupV16HeaderAccount, b: &MarketGroupV16HeaderAccount) -> bool {
    ({ let mut i = 0; let mut ok = true; while i < 32 { ok = ok && a.market_group_id[i] == b.market_group_id[i]; i += 1; } ok })
        && kani_eq_v16_config_account(&a.config, &b.config)
        && a.asset_slot_capacity.get() == b.asset_slot_capacity.get()
        && a.vault.get() == b.vault.get()
        && a.insurance.get() == b.insurance.get()
        && a.c_tot.get() == b.c_tot.get()
        && a.pnl_pos_tot.get() == b.pnl_pos_tot.get()
        && a.pnl_pos_bound_tot_num.get() == b.pnl_pos_bound_tot_num.get()
        && a.pnl_pos_bound_tot.get() == b.pnl_pos_bound_tot.get()
        && a.pnl_matured_pos_tot.get() == b.pnl_matured_pos_tot.get()
        && a.backing_provider_earnings_total.get() == b.backing_provider_earnings_total.get()
        && a.source_claim_bound_total_num.get() == b.source_claim_bound_total_num.get()
        && a.source_fresh_backing_total_num.get() == b.source_fresh_backing_total_num.get()
        && a.source_insurance_credit_reserved_total_atoms.get() == b.source_insurance_credit_reserved_total_atoms.get()
        && a.insurance_domain_budget_remaining_total.get() == b.insurance_domain_budget_remaining_total.get()
        && a.resolved_payout_blocker_count.get() == b.resolved_payout_blocker_count.get()
        && a.materialized_portfolio_count.get() == b.materialized_portfolio_count.get()
        && a.stale_certificate_count.get() == b.stale_certificate_count.get()
        && a.b_stale_account_count.get() == b.b_stale_account_count.get()
        && a.negative_pnl_account_count.get() == b.negative_pnl_account_count.get()
        && a.risk_epoch.get() == b.risk_epoch.get()
        && a.asset_set_epoch.get() == b.asset_set_epoch.get()
        && a.asset_activation_count.get() == b.asset_activation_count.get()
        && a.last_asset_activation_slot.get() == b.last_asset_activation_slot.get()
        && a.next_market_id.get() == b.next_market_id.get()
        && a.oracle_epoch.get() == b.oracle_epoch.get()
        && a.funding_epoch.get() == b.funding_epoch.get()
        && a.slot_last.get() == b.slot_last.get()
        && a.current_slot.get() == b.current_slot.get()
        && a.bankruptcy_hlock_active == b.bankruptcy_hlock_active
        && a.threshold_stress_active == b.threshold_stress_active
        && a.loss_stale_active == b.loss_stale_active
        && kani_eq_v16_optional_recovery_reason_account(&a.recovery_reason, &b.recovery_reason)
        && a.mode == b.mode
        && a.resolved_slot.get() == b.resolved_slot.get()
        && a.payout_snapshot.get() == b.payout_snapshot.get()
        && a.payout_snapshot_pnl_pos_tot.get() == b.payout_snapshot_pnl_pos_tot.get()
        && a.payout_snapshot_captured == b.payout_snapshot_captured
        && kani_eq_resolved_payout_ledger_v16_account(&a.resolved_payout_ledger, &b.resolved_payout_ledger)
}

pub fn kani_eq_asset_state_v16_account(a: &AssetStateV16Account, b: &AssetStateV16Account) -> bool {
    a.market_id.get() == b.market_id.get()
        && a.retired_slot.get() == b.retired_slot.get()
        && a.lifecycle == b.lifecycle
        && a.raw_oracle_target_price.get() == b.raw_oracle_target_price.get()
        && a.effective_price.get() == b.effective_price.get()
        && a.fund_px_last.get() == b.fund_px_last.get()
        && a.slot_last.get() == b.slot_last.get()
        && a.a_long.get() == b.a_long.get()
        && a.a_short.get() == b.a_short.get()
        && a.k_long.get() == b.k_long.get()
        && a.k_short.get() == b.k_short.get()
        && a.f_long_num.get() == b.f_long_num.get()
        && a.f_short_num.get() == b.f_short_num.get()
        && a.k_epoch_start_long.get() == b.k_epoch_start_long.get()
        && a.k_epoch_start_short.get() == b.k_epoch_start_short.get()
        && a.f_epoch_start_long_num.get() == b.f_epoch_start_long_num.get()
        && a.f_epoch_start_short_num.get() == b.f_epoch_start_short_num.get()
        && a.b_long_num.get() == b.b_long_num.get()
        && a.b_short_num.get() == b.b_short_num.get()
        && a.b_epoch_start_long_num.get() == b.b_epoch_start_long_num.get()
        && a.b_epoch_start_short_num.get() == b.b_epoch_start_short_num.get()
        && a.oi_eff_long_q.get() == b.oi_eff_long_q.get()
        && a.oi_eff_short_q.get() == b.oi_eff_short_q.get()
        && a.stored_pos_count_long.get() == b.stored_pos_count_long.get()
        && a.stored_pos_count_short.get() == b.stored_pos_count_short.get()
        && a.stale_account_count_long.get() == b.stale_account_count_long.get()
        && a.stale_account_count_short.get() == b.stale_account_count_short.get()
        && a.pending_obligation_count_long.get() == b.pending_obligation_count_long.get()
        && a.pending_obligation_count_short.get() == b.pending_obligation_count_short.get()
        && a.loss_weight_sum_long.get() == b.loss_weight_sum_long.get()
        && a.loss_weight_sum_short.get() == b.loss_weight_sum_short.get()
        && a.social_loss_remainder_long_num.get() == b.social_loss_remainder_long_num.get()
        && a.social_loss_remainder_short_num.get() == b.social_loss_remainder_short_num.get()
        && a.social_loss_dust_long_num.get() == b.social_loss_dust_long_num.get()
        && a.social_loss_dust_short_num.get() == b.social_loss_dust_short_num.get()
        && a.explicit_unallocated_loss_long.get() == b.explicit_unallocated_loss_long.get()
        && a.explicit_unallocated_loss_short.get() == b.explicit_unallocated_loss_short.get()
        && a.epoch_long.get() == b.epoch_long.get()
        && a.epoch_short.get() == b.epoch_short.get()
        && a.mode_long == b.mode_long
        && a.mode_short == b.mode_short
}

pub fn kani_eq_source_credit_state_v16_account(a: &SourceCreditStateV16Account, b: &SourceCreditStateV16Account) -> bool {
    a.positive_claim_bound_num.get() == b.positive_claim_bound_num.get()
        && a.exact_positive_claim_num.get() == b.exact_positive_claim_num.get()
        && a.fresh_reserved_backing_num.get() == b.fresh_reserved_backing_num.get()
        && a.spent_backing_num.get() == b.spent_backing_num.get()
        && a.provider_receivable_num.get() == b.provider_receivable_num.get()
        && a.valid_liened_backing_num.get() == b.valid_liened_backing_num.get()
        && a.impaired_liened_backing_num.get() == b.impaired_liened_backing_num.get()
        && a.insurance_credit_reserved_num.get() == b.insurance_credit_reserved_num.get()
        && a.valid_liened_insurance_num.get() == b.valid_liened_insurance_num.get()
        && a.impaired_liened_insurance_num.get() == b.impaired_liened_insurance_num.get()
        && a.credit_rate_num.get() == b.credit_rate_num.get()
        && a.credit_epoch.get() == b.credit_epoch.get()
}

pub fn kani_eq_backing_bucket_v16_account(a: &BackingBucketV16Account, b: &BackingBucketV16Account) -> bool {
    a.market_id.get() == b.market_id.get()
        && a.fresh_unliened_backing_num.get() == b.fresh_unliened_backing_num.get()
        && a.valid_liened_backing_num.get() == b.valid_liened_backing_num.get()
        && a.consumed_liened_backing_num.get() == b.consumed_liened_backing_num.get()
        && a.impaired_liened_backing_num.get() == b.impaired_liened_backing_num.get()
        && a.utilization_fee_earnings.get() == b.utilization_fee_earnings.get()
        && a.expiry_slot.get() == b.expiry_slot.get()
        && a.status == b.status
}

pub fn kani_eq_insurance_credit_reservation_v16_account(a: &InsuranceCreditReservationV16Account, b: &InsuranceCreditReservationV16Account) -> bool {
    a.insurance_credit_reserved_num.get() == b.insurance_credit_reserved_num.get()
        && a.valid_liened_insurance_num.get() == b.valid_liened_insurance_num.get()
        && a.impaired_liened_insurance_num.get() == b.impaired_liened_insurance_num.get()
        && a.consumed_insurance_num.get() == b.consumed_insurance_num.get()
        && a.source_credit_epoch.get() == b.source_credit_epoch.get()
}

pub fn kani_eq_engine_asset_slot_v16_account(a: &EngineAssetSlotV16Account, b: &EngineAssetSlotV16Account) -> bool {
    kani_eq_asset_state_v16_account(&a.asset, &b.asset)
        && a.insurance_domain_budget_long.get() == b.insurance_domain_budget_long.get()
        && a.insurance_domain_budget_short.get() == b.insurance_domain_budget_short.get()
        && a.insurance_domain_spent_long.get() == b.insurance_domain_spent_long.get()
        && a.insurance_domain_spent_short.get() == b.insurance_domain_spent_short.get()
        && a.pending_domain_loss_barrier_long.get() == b.pending_domain_loss_barrier_long.get()
        && a.pending_domain_loss_barrier_short.get() == b.pending_domain_loss_barrier_short.get()
        && kani_eq_source_credit_state_v16_account(&a.source_credit_long, &b.source_credit_long)
        && kani_eq_source_credit_state_v16_account(&a.source_credit_short, &b.source_credit_short)
        && kani_eq_backing_bucket_v16_account(&a.backing_long, &b.backing_long)
        && kani_eq_backing_bucket_v16_account(&a.backing_short, &b.backing_short)
        && kani_eq_insurance_credit_reservation_v16_account(&a.insurance_reservation_long, &b.insurance_reservation_long)
        && kani_eq_insurance_credit_reservation_v16_account(&a.insurance_reservation_short, &b.insurance_reservation_short)
}

pub fn kani_eq_provenance_header_v16_account(a: &ProvenanceHeaderV16Account, b: &ProvenanceHeaderV16Account) -> bool {
    ({ let mut i = 0; let mut ok = true; while i < 32 { ok = ok && a.market_group_id[i] == b.market_group_id[i]; i += 1; } ok })
        && ({ let mut i = 0; let mut ok = true; while i < 32 { ok = ok && a.portfolio_account_id[i] == b.portfolio_account_id[i]; i += 1; } ok })
        && ({ let mut i = 0; let mut ok = true; while i < 32 { ok = ok && a.owner[i] == b.owner[i]; i += 1; } ok })
        && a.version.get() == b.version.get()
        && a.layout_discriminator.get() == b.layout_discriminator.get()
}

pub fn kani_eq_portfolio_leg_v16_account(a: &PortfolioLegV16Account, b: &PortfolioLegV16Account) -> bool {
    a.active == b.active
        && a.asset_index.get() == b.asset_index.get()
        && a.market_id.get() == b.market_id.get()
        && a.side == b.side
        && a.basis_pos_q.get() == b.basis_pos_q.get()
        && a.a_basis.get() == b.a_basis.get()
        && a.k_snap.get() == b.k_snap.get()
        && a.f_snap.get() == b.f_snap.get()
        && a.epoch_snap.get() == b.epoch_snap.get()
        && a.loss_weight.get() == b.loss_weight.get()
        && a.b_snap.get() == b.b_snap.get()
        && a.b_rem.get() == b.b_rem.get()
        && a.b_epoch_snap.get() == b.b_epoch_snap.get()
        && a.b_stale == b.b_stale
        && a.stale == b.stale
}

pub fn kani_eq_portfolio_source_domain_v16_account(a: &PortfolioSourceDomainV16Account, b: &PortfolioSourceDomainV16Account) -> bool {
    a.domain.get() == b.domain.get()
        && a.source_claim_market_id.get() == b.source_claim_market_id.get()
        && a.source_claim_bound_num.get() == b.source_claim_bound_num.get()
        && a.source_claim_liened_num.get() == b.source_claim_liened_num.get()
        && a.source_claim_counterparty_liened_num.get() == b.source_claim_counterparty_liened_num.get()
        && a.source_claim_insurance_liened_num.get() == b.source_claim_insurance_liened_num.get()
        && a.source_lien_effective_reserved.get() == b.source_lien_effective_reserved.get()
        && a.source_lien_counterparty_backing_num.get() == b.source_lien_counterparty_backing_num.get()
        && a.source_lien_insurance_backing_num.get() == b.source_lien_insurance_backing_num.get()
        && a.source_lien_fee_last_slot.get() == b.source_lien_fee_last_slot.get()
        && a.source_claim_impaired_num.get() == b.source_claim_impaired_num.get()
        && a.source_lien_impaired_effective_reserved.get() == b.source_lien_impaired_effective_reserved.get()
        && a.source_lien_capital_at_risk_fee_revenue.get() == b.source_lien_capital_at_risk_fee_revenue.get()
        && a.source_lien_impaired_capital_at_risk_fee_revenue.get() == b.source_lien_impaired_capital_at_risk_fee_revenue.get()
}

pub fn kani_eq_health_cert_v16_account(a: &HealthCertV16Account, b: &HealthCertV16Account) -> bool {
    a.certified_equity.get() == b.certified_equity.get()
        && a.certified_initial_req.get() == b.certified_initial_req.get()
        && a.certified_maintenance_req.get() == b.certified_maintenance_req.get()
        && a.certified_liq_deficit.get() == b.certified_liq_deficit.get()
        && a.certified_worst_case_loss.get() == b.certified_worst_case_loss.get()
        && a.cert_oracle_epoch.get() == b.cert_oracle_epoch.get()
        && a.cert_funding_epoch.get() == b.cert_funding_epoch.get()
        && a.cert_risk_epoch.get() == b.cert_risk_epoch.get()
        && a.cert_asset_set_epoch.get() == b.cert_asset_set_epoch.get()
        && ({ let mut i = 0; let mut ok = true; while i < a.active_bitmap_at_cert.len() { ok = ok && a.active_bitmap_at_cert[i].get() == b.active_bitmap_at_cert[i].get(); i += 1; } ok })
        && a.valid == b.valid
}

pub fn kani_eq_close_progress_ledger_v16_account(a: &CloseProgressLedgerV16Account, b: &CloseProgressLedgerV16Account) -> bool {
    a.active == b.active
        && a.finalized == b.finalized
        && a.canceled == b.canceled
        && a.close_id.get() == b.close_id.get()
        && a.asset_index.get() == b.asset_index.get()
        && a.market_id.get() == b.market_id.get()
        && a.domain_side == b.domain_side
        && a.gross_loss_at_close_start.get() == b.gross_loss_at_close_start.get()
        && a.drift_reference_slot.get() == b.drift_reference_slot.get()
        && a.max_close_slot.get() == b.max_close_slot.get()
        && a.support_consumed.get() == b.support_consumed.get()
        && a.junior_face_burned.get() == b.junior_face_burned.get()
        && a.insurance_spent.get() == b.insurance_spent.get()
        && a.b_loss_booked.get() == b.b_loss_booked.get()
        && a.explicit_loss_assigned.get() == b.explicit_loss_assigned.get()
        && a.quantity_adl_applied_q.get() == b.quantity_adl_applied_q.get()
        && a.drift_consumed.get() == b.drift_consumed.get()
        && a.residual_remaining.get() == b.residual_remaining.get()
}

pub fn kani_eq_resolved_payout_receipt_v16_account(a: &ResolvedPayoutReceiptV16Account, b: &ResolvedPayoutReceiptV16Account) -> bool {
    a.prior_bound_contribution_num.get() == b.prior_bound_contribution_num.get()
        && a.live_released_face_at_receipt.get() == b.live_released_face_at_receipt.get()
        && a.terminal_positive_claim_face.get() == b.terminal_positive_claim_face.get()
        && a.paid_effective.get() == b.paid_effective.get()
        && a.present == b.present
        && a.finalized == b.finalized
}

pub fn kani_eq_portfolio_account_v16_account(a: &PortfolioAccountV16Account, b: &PortfolioAccountV16Account) -> bool {
    kani_eq_provenance_header_v16_account(&a.provenance_header, &b.provenance_header)
        && ({ let mut i = 0; let mut ok = true; while i < 32 { ok = ok && a.owner[i] == b.owner[i]; i += 1; } ok })
        && a.capital.get() == b.capital.get()
        && a.pnl.get() == b.pnl.get()
        && a.reserved_pnl.get() == b.reserved_pnl.get()
        && a.residual_crystallized_loss_atoms_total.get() == b.residual_crystallized_loss_atoms_total.get()
        && a.residual_spent_principal_atoms_total.get() == b.residual_spent_principal_atoms_total.get()
        && a.residual_received_atoms_total.get() == b.residual_received_atoms_total.get()
        && a.fee_credits.get() == b.fee_credits.get()
        && a.cancel_deposit_escrow.get() == b.cancel_deposit_escrow.get()
        && a.last_fee_slot.get() == b.last_fee_slot.get()
        && ({ let mut i = 0; let mut ok = true; while i < a.active_bitmap.len() { ok = ok && a.active_bitmap[i].get() == b.active_bitmap[i].get(); i += 1; } ok })
        && ({ let mut i = 0; let mut ok = true; while i < a.legs.len() { ok = ok && kani_eq_portfolio_leg_v16_account(&a.legs[i], &b.legs[i]); i += 1; } ok })
        && ({ let mut i = 0; let mut ok = true; while i < a.source_domains.len() { ok = ok && kani_eq_portfolio_source_domain_v16_account(&a.source_domains[i], &b.source_domains[i]); i += 1; } ok })
        && kani_eq_health_cert_v16_account(&a.health_cert, &b.health_cert)
        && a.stale_state == b.stale_state
        && a.b_stale_state == b.b_stale_state
        && a.rebalance_lock == b.rebalance_lock
        && a.liquidation_lock == b.liquidation_lock
        && kani_eq_close_progress_ledger_v16_account(&a.close_progress, &b.close_progress)
        && kani_eq_resolved_payout_receipt_v16_account(&a.resolved_payout_receipt, &b.resolved_payout_receipt)
}

