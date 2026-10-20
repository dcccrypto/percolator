//! kani v22-final: proof-only shims for the band / rent harnesses (`tests/proofs_v22_band.rs`).
//!
//! `cfg(kani)` child module of `v16` (hooked at the very end of `src/v16.rs`, so no
//! production line moves). Every shim is a one-line forward to the REAL production
//! function; nothing here re-implements behaviour. The module itself is private, so every
//! shim is an inherent `pub fn` on a public type (inherent methods are visible wherever the
//! type is). Names carry the `kani_bnd_` prefix so they cannot collide with the sibling
//! shim module (`kani_v22_shims.rs`).
#![allow(dead_code)]
use super::*;

impl AssetStateV16 {
    /// `V16Core::kernel_band_attach` (`src/v16.rs:1483`).
    pub fn kani_bnd_band_attach(
        self,
        side: SideV16,
        band_bps: u64,
        band_max_positions_per_side: u64,
    ) -> V16Result<AssetStateV16> {
        V16Core::kernel_band_attach(self, side, band_bps, band_max_positions_per_side)
    }

    /// `V16Core::kernel_band_detach` (`:1518`).
    pub fn kani_bnd_band_detach(self, leg: PortfolioLegV16) -> V16Result<AssetStateV16> {
        V16Core::kernel_band_detach(self, leg)
    }

    /// `V16Core::kernel_band_certify_leg` (`:1566`).
    pub fn kani_bnd_band_certify_leg(
        self,
        leg: PortfolioLegV16,
        healthy: bool,
    ) -> V16Result<(AssetStateV16, PortfolioLegV16)> {
        V16Core::kernel_band_certify_leg(self, leg, healthy)
    }

    /// `V16Core::kernel_band_reanchor_ready` (`:1613`).
    pub fn kani_bnd_band_reanchor_ready(self, barrier_long: u64, barrier_short: u64) -> bool {
        V16Core::kernel_band_reanchor_ready(self, barrier_long, barrier_short)
    }

    /// `V16Core::kernel_band_reanchor` (`:1631`).
    pub fn kani_bnd_band_reanchor(self, window_start_slot: u64) -> V16Result<AssetStateV16> {
        V16Core::kernel_band_reanchor(self, window_start_slot)
    }

    /// `V16Core::kernel_attach_leg` (`:1398`).
    pub fn kani_bnd_attach_leg(
        self,
        side: SideV16,
        basis_pos_q: i128,
        loss_weight: u128,
        asset_index: u32,
        band_bps: u64,
        band_max_positions_per_side: u64,
    ) -> V16Result<(AssetStateV16, PortfolioLegV16)> {
        V16Core::kernel_attach_leg(
            self,
            side,
            basis_pos_q,
            loss_weight,
            asset_index,
            band_bps,
            band_max_positions_per_side,
        )
    }

    /// `V16Core::kernel_clear_leg` (`:1784`).
    pub fn kani_bnd_clear_leg(
        self,
        leg: PortfolioLegV16,
        clear_effective_oi_q: u128,
    ) -> V16Result<AssetStateV16> {
        V16Core::kernel_clear_leg(leg, self, clear_effective_oi_q)
    }

    /// `V16Core::effective_abs_quantity_for_leg` (`:2147`).
    pub fn kani_bnd_effective_abs_q(self, leg: PortfolioLegV16) -> V16Result<u128> {
        V16Core::effective_abs_quantity_for_leg(self, leg)
    }
}

impl V16Config {
    /// `V16Config::validate_band_safety_law` (`:5056`).
    pub fn kani_bnd_validate_band_safety_law(&self, rate_e9: u128) -> V16Result<()> {
        self.validate_band_safety_law(rate_e9)
    }

    /// `V16Config::solvency_envelope_holds_for_notional` (`:4897`).
    pub fn kani_bnd_envelope_holds(
        &self,
        n: u128,
        loss_budget_num: u128,
        loss_budget_den: u128,
        price_budget_bps: u128,
    ) -> V16Result<bool> {
        self.solvency_envelope_holds_for_notional(n, loss_budget_num, loss_budget_den, price_budget_bps)
    }

    /// `V16Config::maintenance_requirement_for_notional` (`:4891`).
    pub fn kani_bnd_mm_req(&self, n: u128) -> V16Result<u128> {
        self.maintenance_requirement_for_notional(n)
    }
}

impl<'a, T> MarketGroupV16ViewMut<'a, T> {
    /// `settle_leg_rent_not_atomic` (`:15691`).
    pub fn kani_bnd_settle_leg_rent(
        &mut self,
        account: &mut PortfolioV16ViewMut<'_>,
        asset: AssetStateV16,
        leg: &mut PortfolioLegV16,
    ) -> V16Result<u128> {
        self.settle_leg_rent_not_atomic(account, asset, leg)
    }

    /// `band_prepare_accrual` (`:16777`): returns the (possibly re-anchored) asset and
    /// whether the band gate is active (the gate type itself is private).
    pub fn kani_bnd_band_prepare_accrual(
        &self,
        asset_index: usize,
        asset: AssetStateV16,
        segment_end_slot: u64,
    ) -> V16Result<(AssetStateV16, bool)> {
        let config = self.header.config.try_to_runtime_shape()?;
        let (asset, gate) = self.band_prepare_accrual(asset_index, asset, &config, segment_end_slot)?;
        Ok((asset, gate.active))
    }

    /// `require_band_trade_shape` (`:18387`).
    #[allow(clippy::too_many_arguments)]
    pub fn kani_bnd_require_band_trade_shape(
        &self,
        long_account: &PortfolioV16ViewMut<'_>,
        short_account: &PortfolioV16ViewMut<'_>,
        requests: &[TradeRequestV16],
        before: &[(Option<SideV16>, Option<SideV16>)],
        exempt_long: bool,
        exempt_short: bool,
    ) -> V16Result<()> {
        self.require_band_trade_shape(
            long_account,
            short_account,
            requests,
            before,
            exempt_long,
            exempt_short,
        )
    }
}
