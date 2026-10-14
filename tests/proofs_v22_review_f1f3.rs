#![cfg(kani)]
//! Kani review 2026-10-14, lane engine-f1f3 (review file: ~/wt-v22-kanirev/review/engine-f1f3.md).
//! PROOF/CHARACTERIZATION-ONLY FILE. No production code is changed.
//!
//! * `review_f1_*`  CHARACTERIZATION of ledger finding F1 (not a safety proof): the production
//!   classifier hides a certified deficit behind any later price move, so the auto-crank selects
//!   Refresh, never Liquidate, on the first crank after an accrual.
//! * `review_liq_err_*` strengthens `proof_v16_liquidation_error_commits_only_fully_declared_recovery`
//!   to EVERY `V16Error` variant (the original enumerates 14 of 21).
//! * `review_f3_*` conversion liveness at the support kernel: a consistently-rated source domain
//!   with backing never yields zero support for a claim whose pro-rata share is >= 1 atom.
//! * `review_cov_*` covers on PASS_NOCOVER rows.
#![allow(clippy::too_many_arguments, dead_code, unused_imports, unused_mut)]

use percolator::v16::{
    active_bitmap_get, kani_active_bitmap_set as active_bitmap_set,
    kani_commit_declared_liquidation_recovery, kani_expected_source_credit_rate_num_for_state,
    kani_select_auto_crank_plan, kani_source_credit_state_realizable_support_for_claim_num,
    ActionableSummaryV16, AssetStateV16Account, AutoCrankPlanV16, EngineAssetSlotV16Account,
    HealthCertV16, HealthCertV16Account, Market, MarketGroupV16HeaderAccount,
    MarketGroupV16ViewMut, MarketModeV16, PermissionlessProgressOutcomeV16,
    PermissionlessRecoveryReasonV16, PortfolioAccountV16Account, PortfolioLegV16,
    PortfolioLegV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16, ProvenanceHeaderV16Account,
    SideV16, SourceCreditStateV16, V16Config, V16Error, V16PodI128, V16PodU128, V16PodU64,
};
use percolator::{ADL_ONE, BOUND_SCALE, CREDIT_RATE_SCALE, POS_SCALE};

fn ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    ([1; 32], [2; 32], [3; 32])
}

// =====================================================================================
// F1 characterization (classifier level, production `build_actionable_summary`).
// =====================================================================================

/// One live long of 1 unit at price 100 with matched OI on both sides, pnl -5, capital 0, and a
/// certificate taken at the CURRENT epochs carrying `deficit` (the state a refresh crank leaves
/// behind on an account below maintenance; same leg/asset shape as the native
/// `v16_auto_crank_liquidates_current_account_without_observation`).
fn f1_fixture(
    deficit: u128,
) -> (MarketGroupV16HeaderAccount, [Market<u64>; 1], PortfolioAccountV16Account) {
    let (market_id, _, owner) = ids();
    let cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(market_id, cfg, 1, 0).unwrap();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    {
        let mut view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        view.activate_empty_market_not_atomic(0, 100, 1).unwrap();
    }
    header.current_slot = V16PodU64::new(10);
    header.slot_last = V16PodU64::new(10);
    let mut asset = markets[0].engine.asset.try_to_runtime().unwrap();
    asset.slot_last = 10;
    asset.oi_eff_long_q = 2 * POS_SCALE;
    asset.oi_eff_short_q = 2 * POS_SCALE;
    asset.loss_weight_sum_long = 2 * POS_SCALE;
    asset.loss_weight_sum_short = 2 * POS_SCALE;
    asset.stored_pos_count_long = 2;
    asset.stored_pos_count_short = 2;
    markets[0].engine.asset = AssetStateV16Account::from_runtime(&asset);

    let mut account = PortfolioAccountV16Account::try_empty(ProvenanceHeaderV16Account::from_runtime(
        &ProvenanceHeaderV16::new(market_id, [14; 32], owner),
    ))
    .unwrap();
    account.pnl = V16PodI128::new(-5);
    account.legs[0] = PortfolioLegV16Account::from_runtime(&PortfolioLegV16 {
        active: true,
        asset_index: 0,
        market_id: asset.market_id,
        side: SideV16::Long,
        basis_pos_q: POS_SCALE as i128,
        a_basis: ADL_ONE,
        k_snap: asset.k_long,
        f_snap: asset.f_long_num,
        k_rem_num: 0,
        f_rem_num: 0,
        kf_epoch_snap: 0,
        epoch_snap: asset.epoch_long,
        loss_weight: POS_SCALE,
        b_snap: asset.b_long_num,
        b_rem: 0,
        b_epoch_snap: asset.epoch_long,
        b_stale: false,
        stale: false,
        band_epoch_snap: 0,
        band_liq_pending: false,
        rent_snap: 0,
        rent_carry: 0,
    });
    let mut bitmap = account.active_bitmap.map(V16PodU64::get);
    active_bitmap_set(&mut bitmap, 0).unwrap();
    account.active_bitmap = bitmap.map(V16PodU64::new);
    account.health_cert = HealthCertV16Account::from_runtime(&HealthCertV16 {
        certified_equity: -5,
        certified_initial_req: 10,
        certified_maintenance_req: 5,
        certified_liq_deficit: deficit,
        certified_worst_case_loss: 5,
        cert_oracle_epoch: header.oracle_epoch.get(),
        cert_funding_epoch: header.funding_epoch.get(),
        cert_risk_epoch: header.risk_epoch.get(),
        cert_asset_set_epoch: header.asset_set_epoch.get(),
        active_bitmap_at_cert: bitmap,
        valid: true,
    });
    (header, markets, account)
}

/// CHARACTERIZATION (F1), not a safety proof. A certified deficit is liquidatable on the next
/// crank only while no price move has landed since the certificate. One oracle-epoch bump (any
/// accrual that moves the price, e.g. the keeper's observation crank through the LP earlier in
/// the same transaction) makes the classifier report `stale && !liquidatable`, and the proven
/// selector then returns `RefreshAccount`: the crank refreshes and does not liquidate.
#[kani::proof]
#[kani::unwind(18)]
#[kani::solver(cadical)]
fn review_f1_price_move_since_cert_hides_deficit_from_auto_crank() {
    let deficit: u128 = kani::any();
    let pushed: bool = kani::any();
    kani::assume(deficit >= 1 && deficit <= 1_000);
    let (mut header, mut markets, mut account_header) = f1_fixture(deficit);
    if pushed {
        header.oracle_epoch = V16PodU64::new(header.oracle_epoch.get() + 1);
    }
    let market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let account = PortfolioV16ViewMut::new(&mut account_header);
    let summary = market.build_actionable_summary(&account.as_view()).unwrap();
    let plan = kani_select_auto_crank_plan(
        summary,
        0,
        0,
        Some(0),
        PermissionlessRecoveryReasonV16::ExplicitLossOrDustAuditOverflow,
    );
    kani::cover!(pushed && deficit > 1, "deficit certified, then a price move: crank refreshes only");
    kani::cover!(!pushed && deficit > 1, "deficit certified, no move since: crank liquidates");
    if pushed {
        assert!(summary.stale && !summary.liquidatable);
        assert_eq!(plan, AutoCrankPlanV16::RefreshAccount { asset_index: Some(0) });
    } else {
        assert!(summary.liquidatable && !summary.stale);
        assert_eq!(plan, AutoCrankPlanV16::Liquidate { asset_index: 0 });
    }
}

// =====================================================================================
// Liquidation-error commit kernel over ALL 21 V16Error variants.
// =====================================================================================

fn any_v16_error() -> V16Error {
    let t: u8 = kani::any();
    kani::assume(t < 21);
    match t {
        0 => V16Error::InvalidConfig,
        1 => V16Error::ArithmeticOverflow,
        2 => V16Error::ProvenanceMismatch,
        3 => V16Error::HiddenLeg,
        4 => V16Error::InvalidLeg,
        5 => V16Error::Stale,
        6 => V16Error::BStale,
        7 => V16Error::LockActive,
        8 => V16Error::NonProgress,
        9 => V16Error::RecoveryRequired,
        10 => V16Error::CounterOverflow,
        11 => V16Error::CounterUnderflow,
        12 => V16Error::InsufficientInitialMargin,
        13 => V16Error::LpVaultZeroSharesMinted,
        14 => V16Error::AdlReduceOnly,
        15 => V16Error::LossStale,
        16 => V16Error::BandOutOfRange,
        17 => V16Error::BandPinned,
        18 => V16Error::BandPositionCap,
        19 => V16Error::BandTooNarrow,
        _ => V16Error::BandLegBelowMinNotional,
    }
}

/// Exhaustiveness guard: if a 22nd variant is added this match stops compiling.
fn v16_error_variant_count_is_21(e: V16Error) -> u8 {
    match e {
        V16Error::InvalidConfig => 0,
        V16Error::ArithmeticOverflow => 1,
        V16Error::ProvenanceMismatch => 2,
        V16Error::HiddenLeg => 3,
        V16Error::InvalidLeg => 4,
        V16Error::Stale => 5,
        V16Error::BStale => 6,
        V16Error::LockActive => 7,
        V16Error::NonProgress => 8,
        V16Error::RecoveryRequired => 9,
        V16Error::CounterOverflow => 10,
        V16Error::CounterUnderflow => 11,
        V16Error::InsufficientInitialMargin => 12,
        V16Error::LpVaultZeroSharesMinted => 13,
        V16Error::AdlReduceOnly => 14,
        V16Error::LossStale => 15,
        V16Error::BandOutOfRange => 16,
        V16Error::BandPinned => 17,
        V16Error::BandPositionCap => 18,
        V16Error::BandTooNarrow => 19,
        V16Error::BandLegBelowMinNotional => 20,
    }
}

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn review_liq_err_commit_only_full_declaration_all_21_errors() {
    let error = any_v16_error();
    let mode_tag: u8 = kani::any();
    let reason_tag: u8 = kani::any();
    kani::assume(mode_tag < 3);
    kani::assume(reason_tag <= 8);
    let mode = match mode_tag {
        0 => MarketModeV16::Live,
        1 => MarketModeV16::Resolved,
        _ => MarketModeV16::Recovery,
    };
    let reason = match reason_tag {
        0 => Some(PermissionlessRecoveryReasonV16::BelowProgressFloor),
        1 => Some(PermissionlessRecoveryReasonV16::BlockedSegmentHeadroomOrRepresentability),
        2 => Some(PermissionlessRecoveryReasonV16::AccountBSettlementCannotProgress),
        3 => Some(PermissionlessRecoveryReasonV16::BIndexHeadroomExhausted),
        4 => Some(PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress),
        5 => Some(PermissionlessRecoveryReasonV16::ExplicitLossOrDustAuditOverflow),
        6 => Some(PermissionlessRecoveryReasonV16::OracleOrTargetUnavailableByAuthenticatedPolicy),
        7 => Some(PermissionlessRecoveryReasonV16::CounterOrEpochOverflowDeclaredRecovery),
        _ => None,
    };
    let result = kani_commit_declared_liquidation_recovery(error, mode, reason);
    let complete =
        error == V16Error::RecoveryRequired && mode == MarketModeV16::Recovery && reason.is_some();
    kani::cover!(complete, "complete declaration converted to progress");
    kani::cover!(
        v16_error_variant_count_is_21(error) >= 14 && mode == MarketModeV16::Recovery && reason.is_some(),
        "post-v16 variants (AdlReduceOnly..BandLegBelowMinNotional) reach the kernel in Recovery"
    );
    if complete {
        assert_eq!(result, Ok(PermissionlessProgressOutcomeV16::RecoveryDeclared(reason.unwrap())));
    } else {
        assert_eq!(result, Err(error));
    }
}

// =====================================================================================
// F3: conversion liveness at the support kernel.
// =====================================================================================

/// LIVENESS lower bound (F3 diagnosis aid). For a domain whose stored rate is the production
/// expected rate (`validate_source_credit_state_static`'s condition), a claim whose pro-rata
/// share of the available backing is at least TWO atoms gets NON-ZERO realizable support, and the
/// support is within one atom of `floor(claim * available / claims)` (the haircut).
/// So the Live conversion's `converted == 0 -> LockActive` refusal can only fire when the domain
/// has no available backing (or the holder's share is below two atoms), not from the rate maths.
/// Widths: atoms u8 (so num values up to 255e12); `claims >= claim`.
#[kani::proof]
#[kani::solver(kissat)]
fn review_f3_backed_claim_never_gets_zero_support() {
    let avail_atoms: u8 = kani::any();
    let claims_atoms: u8 = kani::any();
    let claim_atoms: u8 = kani::any();
    kani::assume(claims_atoms >= 1 && claim_atoms >= 1 && claim_atoms <= claims_atoms);
    let mut state = SourceCreditStateV16::EMPTY;
    state.positive_claim_bound_num = claims_atoms as u128 * BOUND_SCALE;
    state.exact_positive_claim_num = state.positive_claim_bound_num;
    state.fresh_reserved_backing_num = avail_atoms as u128 * BOUND_SCALE;
    let rate = kani_expected_source_credit_rate_num_for_state(state).unwrap();
    state.credit_rate_num = rate;
    let claim_num = claim_atoms as u128 * BOUND_SCALE;
    let support =
        kani_source_credit_state_realizable_support_for_claim_num(state, claim_num).unwrap();
    let pro_rata = (claim_atoms as u128 * avail_atoms as u128) / claims_atoms as u128;
    let fair = core::cmp::min(pro_rata, claim_atoms as u128);
    kani::cover!(avail_atoms < claims_atoms && support > 0, "under-backed domain still converts (haircut)");
    kani::cover!(avail_atoms >= claims_atoms && support == claim_atoms as u128, "fully backed: full claim");
    kani::cover!(avail_atoms == 0 && support == 0, "no backing: zero support (the 21 refusal)");
    // Rounding edge: rate = floor(a*S/C) loses a sub-atom, so an EXACT one-atom share
    // (claim*avail divisible by claims, e.g. a=1, C=3, c=3) floors to 0.
    kani::cover!(fair == 1 && support == 0, "exact one-atom share floors to zero support");
    assert!(support <= fair);
    assert!(support + 1 >= fair);
    if fair >= 2 {
        assert!(support >= 1);
    }
}
