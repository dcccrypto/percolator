#![cfg(kani)]
//! fix/v21-funding-scale: counter kernels behind the hidden-loss bound. The bound's
//! arithmetic soundness against real settlement is covered by
//! `tests/v21_funding_scale_solvency.rs` (proptest, exact per-leg floor formula).

use percolator::v16::{
    kani_mark_kf_stale_cohorts, kani_settle_kf_laggard, kani_settle_kf_stale_cohort,
    kani_track_kf_side_drift, AssetStateV16, KfDriftSideV16, SideV16,
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
    let kf_epoch: u64 = kani::any();
    let cohort_epoch: u64 = kani::any();
    let weight_sum: u128 = kani::any();
    let adverse: u128 = kani::any();
    let changed: bool = kani::any();
    let d = KfDriftSideV16 {
        gen_epoch: kani::any(),
        laggard_count: kani::any(),
        drift_gen: kani::any(),
        drift_prior: kani::any(),
        stale_weight: kani::any(),
        laggard_weight: kani::any(),
    };
    kani::assume(d.laggard_count <= stale && stale <= stored);
    kani::assume(d.gen_epoch <= kf_epoch);
    kani::assume(!changed || cohort_epoch > kf_epoch);

    let mut asset = AssetStateV16::default();
    asset.stored_pos_count_long = stored;
    asset.stale_account_count_long = stale;
    asset.kf_epoch_long = kf_epoch;
    asset.loss_weight_sum_long = weight_sum;

    let t = kani_track_kf_side_drift(d, changed, kf_epoch, stale, weight_sum, adverse);
    let marked = kani_mark_kf_stale_cohorts(asset, changed, false, cohort_epoch).unwrap();

    assert!(t.laggard_count <= marked.stale_account_count_long);
    assert!(t.gen_epoch <= marked.kf_epoch_long);
    if !changed {
        assert_eq!(t, d);
        return;
    }
    assert_eq!(t.stale_weight, weight_sum);
    if d.laggard_count == 0 {
        kani::cover!(stale > 0, "rotation inherits a non-empty laggard set");
        assert_eq!(t.gen_epoch, kf_epoch);
        assert_eq!(t.laggard_count, stale);
        assert_eq!(t.laggard_weight, d.stale_weight);
        assert_eq!(t.drift_prior, d.drift_gen);
        assert_eq!(t.drift_gen, adverse);
    } else {
        kani::cover!(true, "an open generation keeps accumulating");
        assert_eq!(t.gen_epoch, d.gen_epoch);
        assert_eq!(t.laggard_count, d.laggard_count);
        assert_eq!(t.laggard_weight, d.laggard_weight);
        assert_eq!(t.drift_prior, d.drift_prior);
        assert_eq!(t.drift_gen, d.drift_gen.saturating_add(adverse));
        assert!(t.drift_gen >= d.drift_gen);
    }
}

/// Settling a leg (laggard discharge, then the cohort discharge, as production composes
/// them) keeps laggards inside the cohort and never underflows when the counters describe
/// the leg (a leg older than the generation start is counted as a laggard, a leg older than
/// the KF epoch is counted as stale, and a stale non-laggard leg leaves room above laggards).
#[kani::proof]
#[kani::unwind(2)]
#[kani::solver(cadical)]
fn proof_v21_settlement_discharges_laggard_with_its_cohort_member() {
    let stored: u64 = kani::any();
    let stale: u64 = kani::any();
    let kf_epoch: u64 = kani::any();
    let snap: u64 = kani::any();
    let leg_weight: u128 = kani::any();
    let d = KfDriftSideV16 {
        gen_epoch: kani::any(),
        laggard_count: kani::any(),
        drift_gen: kani::any(),
        drift_prior: kani::any(),
        stale_weight: kani::any(),
        laggard_weight: kani::any(),
    };
    kani::assume(d.laggard_count <= stale && stale <= stored);
    kani::assume(d.gen_epoch <= kf_epoch);
    kani::assume(snap <= kf_epoch);
    kani::assume(snap >= d.gen_epoch || d.laggard_count >= 1);
    kani::assume(snap >= kf_epoch || stale >= 1);
    kani::assume(!(snap >= d.gen_epoch && snap < kf_epoch) || stale > d.laggard_count);

    let mut asset = AssetStateV16::default();
    asset.stored_pos_count_long = stored;
    asset.stale_account_count_long = stale;
    asset.kf_epoch_long = kf_epoch;

    let t = kani_settle_kf_laggard(d, kf_epoch, snap, leg_weight).unwrap();
    let (settled, epoch) = kani_settle_kf_stale_cohort(asset, SideV16::Long, snap).unwrap();

    assert_eq!(epoch, kf_epoch);
    assert!(t.laggard_count <= settled.stale_account_count_long);
    assert_eq!(
        t.laggard_count,
        if snap < d.gen_epoch { d.laggard_count - 1 } else { d.laggard_count }
    );
    assert_eq!(
        settled.stale_account_count_long,
        if snap < kf_epoch { stale - 1 } else { stale }
    );
    assert!(t.stale_weight <= d.stale_weight);
    assert!(t.laggard_weight <= d.laggard_weight);
    assert_eq!((t.gen_epoch, t.drift_gen, t.drift_prior), (d.gen_epoch, d.drift_gen, d.drift_prior));
    kani::cover!(snap < d.gen_epoch, "a laggard settles");
    kani::cover!(snap >= d.gen_epoch && snap < kf_epoch, "a non-laggard stale leg settles");
    kani::cover!(snap == kf_epoch, "a current leg is a no-op");
}
