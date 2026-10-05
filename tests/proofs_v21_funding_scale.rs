#![cfg(kani)]
//! fix/v21-funding-scale: counter kernels behind the hidden-loss bound. The bound's
//! arithmetic soundness against real settlement is covered by
//! `tests/v21_funding_scale_solvency.rs` (proptest, exact per-leg floor formula).

use percolator::v16::{
    kani_mark_kf_stale_cohorts, kani_settle_kf_laggard, kani_settle_kf_stale_cohort,
    kani_track_kf_cohort_drift, AssetStateV16, SideV16,
};

/// Track-then-mark (the only order production uses) keeps the laggards a subset of the
/// stale cohort, never starts a generation after the KF epoch, rotates exactly when the
/// previous generation has no laggard left, and resets the stale weight with the cohort.
#[kani::proof]
#[kani::unwind(2)]
#[kani::solver(cadical)]
fn proof_v21_drift_track_then_mark_keeps_laggards_inside_the_cohort() {
    let stored: u64 = kani::any();
    let stale: u64 = kani::any();
    let laggard: u64 = kani::any();
    let kf_epoch: u64 = kani::any();
    let gen_epoch: u64 = kani::any();
    let cohort_epoch: u64 = kani::any();
    let gen: u128 = kani::any();
    let prior: u128 = kani::any();
    let stale_weight: u128 = kani::any();
    let weight_sum: u128 = kani::any();
    let adverse: u128 = kani::any();
    let changed: bool = kani::any();
    kani::assume(laggard <= stale && stale <= stored);
    kani::assume(gen_epoch <= kf_epoch);
    kani::assume(!changed || cohort_epoch > kf_epoch);

    let mut asset = AssetStateV16::default();
    asset.stored_pos_count_long = stored;
    asset.stale_account_count_long = stale;
    asset.kf_gen_laggard_count_long = laggard;
    asset.kf_epoch_long = kf_epoch;
    asset.kf_gen_epoch_long = gen_epoch;
    asset.kf_drift_gen_long = gen;
    asset.kf_drift_prior_long = prior;
    asset.kf_stale_weight_long = stale_weight;
    asset.loss_weight_sum_long = weight_sum;

    let tracked = kani_track_kf_cohort_drift(asset, changed, false, adverse, 0).unwrap();
    let marked = kani_mark_kf_stale_cohorts(tracked, changed, false, cohort_epoch).unwrap();

    assert!(marked.kf_gen_laggard_count_long <= marked.stale_account_count_long);
    assert!(marked.kf_gen_epoch_long <= marked.kf_epoch_long);
    // The short side is untouched.
    assert_eq!(marked.kf_gen_laggard_count_short, 0);
    assert_eq!(marked.kf_drift_gen_short, 0);
    if !changed {
        assert_eq!(marked.kf_gen_laggard_count_long, laggard);
        assert_eq!(marked.kf_drift_gen_long, gen);
        assert_eq!(marked.kf_stale_weight_long, stale_weight);
        return;
    }
    assert_eq!(marked.kf_stale_weight_long, weight_sum);
    if laggard == 0 {
        kani::cover!(stale > 0, "rotation inherits a non-empty laggard set");
        assert_eq!(marked.kf_gen_epoch_long, kf_epoch);
        assert_eq!(marked.kf_gen_laggard_count_long, stale);
        assert_eq!(marked.kf_gen_laggard_weight_long, stale_weight);
        assert_eq!(marked.kf_drift_prior_long, gen);
        assert_eq!(marked.kf_drift_gen_long, adverse);
    } else {
        kani::cover!(true, "an open generation keeps accumulating");
        assert_eq!(marked.kf_gen_epoch_long, gen_epoch);
        assert_eq!(marked.kf_gen_laggard_count_long, laggard);
        assert_eq!(marked.kf_drift_prior_long, prior);
        assert_eq!(marked.kf_drift_gen_long, gen.saturating_add(adverse));
        assert!(marked.kf_drift_gen_long >= gen);
    }
}

/// Settling a leg (laggard discharge, then the cohort discharge, as production composes
/// them) keeps laggards inside the cohort and never underflows when the counters describe
/// the leg (a leg older than the generation start is counted as a laggard, and a leg older
/// than the KF epoch is counted as stale).
#[kani::proof]
#[kani::unwind(2)]
#[kani::solver(cadical)]
fn proof_v21_settlement_discharges_laggard_with_its_cohort_member() {
    let stored: u64 = kani::any();
    let stale: u64 = kani::any();
    let laggard: u64 = kani::any();
    let kf_epoch: u64 = kani::any();
    let gen_epoch: u64 = kani::any();
    let snap: u64 = kani::any();
    let stale_weight: u128 = kani::any();
    let laggard_weight: u128 = kani::any();
    let leg_weight: u128 = kani::any();
    kani::assume(laggard <= stale && stale <= stored);
    kani::assume(gen_epoch <= kf_epoch);
    kani::assume(snap <= kf_epoch);
    // The counters describe this leg.
    kani::assume(snap >= gen_epoch || laggard >= 1);
    kani::assume(snap >= kf_epoch || stale >= 1);
    // Laggards are stale; a stale laggard leaves both sets, so the counters must leave room
    // for a laggard-free stale leg only when one exists.
    kani::assume(!(snap >= gen_epoch && snap < kf_epoch) || stale > laggard);

    let mut asset = AssetStateV16::default();
    asset.stored_pos_count_long = stored;
    asset.stale_account_count_long = stale;
    asset.kf_gen_laggard_count_long = laggard;
    asset.kf_epoch_long = kf_epoch;
    asset.kf_gen_epoch_long = gen_epoch;
    asset.kf_stale_weight_long = stale_weight;
    asset.kf_gen_laggard_weight_long = laggard_weight;

    let lagged = kani_settle_kf_laggard(asset, SideV16::Long, snap, leg_weight).unwrap();
    let (settled, epoch) = kani_settle_kf_stale_cohort(lagged, SideV16::Long, snap).unwrap();

    assert_eq!(epoch, kf_epoch);
    assert!(settled.kf_gen_laggard_count_long <= settled.stale_account_count_long);
    assert_eq!(
        settled.kf_gen_laggard_count_long,
        if snap < gen_epoch { laggard - 1 } else { laggard }
    );
    assert_eq!(
        settled.stale_account_count_long,
        if snap < kf_epoch { stale - 1 } else { stale }
    );
    assert!(settled.kf_stale_weight_long <= stale_weight);
    assert!(settled.kf_gen_laggard_weight_long <= laggard_weight);
    kani::cover!(snap < gen_epoch, "a laggard settles");
    kani::cover!(snap >= gen_epoch && snap < kf_epoch, "a non-laggard stale leg settles");
    kani::cover!(snap == kf_epoch, "a current leg is a no-op");
}
