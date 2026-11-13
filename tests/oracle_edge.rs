//! Oracle edge cases for the v2.2 risk engine (J-20261113-P5, tests only).
//!
//! Spec anchors (spec.md):
//!   §1.7  wrapper-fed effective price vs raw oracle target, the clamp law, same-slot rule,
//!         unexposed books may take the raw target directly, target/effective divergence gates;
//!   §5.3  `accrue_market_to`: dt bound, the exact per-slot price cap, mark once, epoch bumps;
//!   §12   wrapper rules 6 and 9 (separate raw target, capped staircase, no cap-violating jump).
//!
//! Every test drives the public zero-copy view. Rejected oracle updates and accruals assert
//! that the market (header + asset slots) is byte-for-byte unchanged, so a refused oracle
//! update cannot leave a half-applied mark behind. Rejected trades and cranks assert the
//! error and the mark only.
//!
//! The canonical path (`accrue_asset_path_to_not_atomic`) moves linearly from a per-target
//! anchor (`fund_px_last`), by design (see the doc on `canonical_accrual_price_step_v16`), not
//! from each call's `P_last` as the §1.7 clamp law is written. It stays inside the §5.3 envelope
//! cumulatively per target (per call the carried sub-atom remainder may add up to one atom, by
//! design, to avoid the §1.7 CatchupRequired pin); downward a later call can move more than
//! `cap * P_last * dt`. The tests below pin the
//! first-call envelope only and say so; the one real gap found is in
//! `oracle_path_caller_chosen_remainder_cannot_beat_the_carried_path` (ignored, see there).

use percolator::{
    canonical_accrual_price_step_v16, AccrualStepV16, AssetLifecycleV16, AssetStateV16,
    AutoCrankObservationV16, AutoCrankPlanV16, AutoCrankWorkV16, EngineAssetSlotV16Account,
    Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, PortfolioAccountV16Account,
    PermissionlessCrankActionV16, PermissionlessCrankRequestV16, PortfolioV16ViewMut,
    ProvenanceHeaderV16, ProvenanceHeaderV16Account, TradeRequestV16,
    V16Config, V16Error, MAX_ORACLE_PRICE, POS_SCALE,
};

type Header = MarketGroupV16HeaderAccount;
type Markets = Vec<Market<u64>>;

const P0: u64 = 1_000_000;
/// 1% of price per slot.
const CAP_BPS: u64 = 100;
const MAX_DT: u64 = 10;
const FUNDING_MAX: u64 = 10_000;

// ---------------------------------------------------------------- fixtures --

fn ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    ([1; 32], [2; 32], [3; 32])
}

fn oracle_cfg(slots: u32) -> V16Config {
    let max_portfolio_assets = slots.min(percolator::V16_MAX_PORTFOLIO_ASSETS_N as u32) as u16;
    let mut cfg = V16Config::public_user_fund_with_market_slots(max_portfolio_assets, slots, 0, 10);
    cfg.max_accrual_dt_slots = MAX_DT;
    cfg.min_funding_lifetime_slots = MAX_DT;
    cfg.max_abs_funding_e9_per_slot = FUNDING_MAX;
    cfg.max_price_move_bps_per_slot = CAP_BPS;
    cfg
}

fn market(slots: u32, price: u64) -> (Header, Markets) {
    let (market_id, _, _) = ids();
    let mut header = Header::new_dynamic(market_id, oracle_cfg(slots), slots, 0).unwrap();
    let mut markets = (0..slots)
        .map(|i| Market::new(i as u64, EngineAssetSlotV16Account::default()))
        .collect::<Vec<_>>();
    for (i, m) in markets.iter_mut().enumerate() {
        header
            .activate_empty_asset_slot_not_atomic(i as u32, &mut m.engine, price, (i + 1) as u64)
            .unwrap();
    }
    MarketGroupV16ViewMut::new(&mut header, &mut markets)
        .validate_shape()
        .unwrap();
    (header, markets)
}

fn account(seed: u8) -> PortfolioAccountV16Account {
    let (market_id, _, owner) = ids();
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        market_id,
        [seed; 32],
        owner,
    ));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(header).unwrap();
    a
}

fn asset(markets: &Markets, i: usize) -> AssetStateV16 {
    markets[i].engine.asset.try_to_runtime().unwrap()
}

fn deposit(h: &mut Header, m: &mut Markets, a: &mut PortfolioAccountV16Account, amt: u128) {
    let mut market = MarketGroupV16ViewMut::new(h, m);
    let mut v = PortfolioV16ViewMut::new(a);
    market.deposit_not_atomic(&mut v, amt).unwrap();
}

fn trade(
    h: &mut Header,
    m: &mut Markets,
    long: &mut PortfolioAccountV16Account,
    short: &mut PortfolioAccountV16Account,
    asset_index: usize,
    size_q: i128,
    price: u64,
) -> Result<(), V16Error> {
    let mut market = MarketGroupV16ViewMut::new(h, m);
    let mut l = PortfolioV16ViewMut::new(long);
    let mut s = PortfolioV16ViewMut::new(short);
    market
        .execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut l,
            &mut s,
            TradeRequestV16 {
                asset_index,
                size_q,
                exec_price: price,
                fee_bps: 0,
            },
            true,
        )
        .map(|_| ())
}

/// A market whose asset 0 (of `slots`) carries one matched 1-unit long/short pair at `P0`.
fn exposed(slots: u32) -> (Header, Markets, PortfolioAccountV16Account, PortfolioAccountV16Account) {
    let (mut h, mut m) = market(slots, P0);
    let mut l = account(40);
    let mut s = account(41);
    deposit(&mut h, &mut m, &mut l, 10_000_000);
    deposit(&mut h, &mut m, &mut s, 10_000_000);
    trade(&mut h, &mut m, &mut l, &mut s, 0, POS_SCALE as i128, P0).unwrap();
    let a = asset(&m, 0);
    assert!(a.oi_eff_long_q != 0 && a.oi_eff_short_q != 0, "fixture must be exposed");
    (h, m, l, s)
}

fn accrue(
    h: &mut Header,
    m: &mut Markets,
    i: usize,
    slot: u64,
    price: u64,
    funding: i128,
) -> Result<percolator::AccrueAssetOutcomeV16, V16Error> {
    MarketGroupV16ViewMut::new(h, m).accrue_asset_to_not_atomic(i, slot, price, funding, true)
}

fn set_targets(h: &mut Header, m: &mut Markets, targets: &[(usize, u64)]) -> Result<(), V16Error> {
    MarketGroupV16ViewMut::new(h, m).set_asset_raw_oracle_targets_not_atomic(targets)
}

/// The canonical one-call path the wrapper must build for `asset_index` (no band on these
/// fixtures), starting from committed state, exactly as the engine re-derives it.
fn canonical_path(m: &Markets, i: usize, now_slot: u64, target: u64) -> Vec<AccrualStepV16> {
    let a = asset(m, i);
    let steps = (now_slot - a.slot_last).min(MAX_DT)
        .min(percolator::V16_MAX_ACCRUAL_PATH_STEPS as u64);
    let target_changed = a.raw_oracle_target_price != target;
    let anchor = if target_changed { a.effective_price } else { a.fund_px_last };
    let exposed = a.oi_eff_long_q != 0 || a.oi_eff_short_q != 0;
    let mut price = a.effective_price;
    // The engine requires the first step's carried remainder to be 0 on a new target; on an
    // unchanged target the caller carries its own remainder, which these tests restart at 0.
    let mut rem = 0u16;
    (0..steps)
        .map(|_| {
            let before = rem;
            (price, rem) =
                canonical_accrual_price_step_v16(price, target, anchor, CAP_BPS, exposed, rem)
                    .unwrap();
            AccrualStepV16 {
                effective_price: price,
                funding_rate_e9: 0,
                price_move_remainder_before_bps_num: before,
                price_move_remainder_after_bps_num: rem,
            }
        })
        .collect()
}

fn accrue_path(
    h: &mut Header,
    m: &mut Markets,
    i: usize,
    now_slot: u64,
    target: u64,
    steps: &[AccrualStepV16],
) -> Result<percolator::AccrueAssetOutcomeV16, V16Error> {
    MarketGroupV16ViewMut::new(h, m).accrue_asset_path_to_not_atomic(i, now_slot, target, steps, true)
}

// ======================================================= raw oracle target ===

#[test]
fn oracle_target_zero_is_rejected_without_mutation() {
    let (mut h, mut m) = market(2, P0);
    let (h0, m0) = (h, m.clone());
    assert_eq!(set_targets(&mut h, &mut m, &[(0, 0)]), Err(V16Error::InvalidConfig));
    assert_eq!((h, &m), (h0, &m0));
}

#[test]
fn oracle_target_max_price_boundary_accepts_max_and_rejects_max_plus_one() {
    let (mut h, mut m) = market(1, P0);
    let epoch = h.oracle_epoch.get();
    let (h0, m0) = (h, m.clone());
    assert_eq!(
        set_targets(&mut h, &mut m, &[(0, MAX_ORACLE_PRICE + 1)]),
        Err(V16Error::InvalidConfig)
    );
    assert_eq!((h, &m), (h0, &m0));
    set_targets(&mut h, &mut m, &[(0, MAX_ORACLE_PRICE)]).unwrap();
    assert_eq!(asset(&m, 0).raw_oracle_target_price, MAX_ORACLE_PRICE);
    assert_eq!(asset(&m, 0).effective_price, P0, "a target push never moves the mark");
    assert_eq!(h.oracle_epoch.get(), epoch + 1);
}

#[test]
fn oracle_target_batch_is_all_or_nothing_when_a_later_entry_is_invalid() {
    let (mut h, mut m) = market(3, P0);
    let (h0, m0) = (h, m.clone());
    // A valid change first, then each kind of bad entry: the whole batch must be refused and
    // the first (valid) entry must not have been written.
    for bad in [(1usize, 0u64), (1, MAX_ORACLE_PRICE + 1), (7, P0 + 1), (0, P0 + 2)] {
        let res = set_targets(&mut h, &mut m, &[(0, P0 + 1), bad]);
        // An unconfigured index is an invalid leg reference; bad prices / duplicates are
        // invalid config. Either way the batch is refused as a whole.
        let want = if bad.0 == 7 { V16Error::InvalidLeg } else { V16Error::InvalidConfig };
        assert_eq!(res, Err(want), "bad entry {bad:?}");
        assert_eq!((h, &m), (h0, &m0), "bad entry {bad:?} left a partial write");
    }
}

#[test]
fn oracle_target_empty_batch_is_rejected() {
    let (mut h, mut m) = market(1, P0);
    let (h0, m0) = (h, m.clone());
    assert_eq!(set_targets(&mut h, &mut m, &[]), Err(V16Error::InvalidConfig));
    assert_eq!((h, &m), (h0, &m0));
}

#[test]
fn oracle_target_mixed_batch_bumps_epoch_once_and_unchanged_batch_not_at_all() {
    let (mut h, mut m) = market(3, P0);
    let e0 = h.oracle_epoch.get();
    // Two of three entries equal the stored target (P0 after activation); one changes.
    set_targets(&mut h, &mut m, &[(0, P0), (1, P0 + 5), (2, P0)]).unwrap();
    assert_eq!(h.oracle_epoch.get(), e0 + 1);
    // Re-pushing the same observation is a no-op for the epoch (certs stay valid).
    set_targets(&mut h, &mut m, &[(2, P0), (1, P0 + 5), (0, P0)]).unwrap();
    assert_eq!(h.oracle_epoch.get(), e0 + 1);
}

#[test]
fn oracle_target_push_leaves_mark_kf_slot_and_funding_anchor_untouched() {
    let (mut h, mut m, _l, _s) = exposed(1);
    let before = asset(&m, 0);
    let (vault, c_tot, ins) = (h.vault.get(), h.c_tot.get(), h.insurance.get());
    set_targets(&mut h, &mut m, &[(0, P0 * 2)]).unwrap();
    let after = asset(&m, 0);
    assert_eq!(after.raw_oracle_target_price, P0 * 2);
    let mut expect = before;
    expect.raw_oracle_target_price = P0 * 2;
    assert_eq!(after, expect, "only the raw target field may change");
    assert_eq!((h.vault.get(), h.c_tot.get(), h.insurance.get()), (vault, c_tot, ins));
}

#[test]
fn oracle_target_on_recovery_asset_is_locked_and_drain_only_is_pushable() {
    let (mut h, mut m) = market(2, P0);
    let now = h.current_slot.get();
    MarketGroupV16ViewMut::new(&mut h, &mut m)
        .force_asset_recovery_not_atomic(1, now)
        .unwrap();
    assert_eq!(asset(&m, 1).lifecycle, AssetLifecycleV16::Recovery);
    let (h0, m0) = (h, m.clone());
    assert_eq!(set_targets(&mut h, &mut m, &[(1, P0 + 1)]), Err(V16Error::LockActive));
    // Also inside a batch with a valid entry first: no partial write.
    assert_eq!(
        set_targets(&mut h, &mut m, &[(0, P0 + 1), (1, P0 + 1)]),
        Err(V16Error::LockActive)
    );
    assert_eq!((h, &m), (h0, &m0));

    MarketGroupV16ViewMut::new(&mut h, &mut m)
        .mark_asset_drain_only_not_atomic(0)
        .unwrap();
    set_targets(&mut h, &mut m, &[(0, P0 + 1)])
        .expect("a DrainOnly asset still needs fresh marks to wind down");
    assert_eq!(asset(&m, 0).raw_oracle_target_price, P0 + 1);
}

#[test]
fn oracle_target_after_resolution_is_refused() {
    let (mut h, mut m) = market(1, P0);
    MarketGroupV16ViewMut::new(&mut h, &mut m)
        .resolve_market_not_atomic(5)
        .unwrap();
    let (h0, m0) = (h, m.clone());
    assert_eq!(set_targets(&mut h, &mut m, &[(0, P0 + 1)]), Err(V16Error::InvalidConfig));
    assert_eq!((h, &m), (h0, &m0));
}

// ============================================================== stale marks ===

#[test]
fn stale_mark_same_slot_price_move_on_exposed_book_is_non_progress() {
    let (mut h, mut m, _l, _s) = exposed(1);
    let slot = asset(&m, 0).slot_last;
    let (h0, m0) = (h, m.clone());
    // §1.7: same-slot exposed cranks MUST pass P_last.
    assert_eq!(accrue(&mut h, &mut m, 0, slot, P0 + 1, 0), Err(V16Error::NonProgress));
    assert_eq!((h, &m), (h0, &m0));
    // NEGATIVE CONTROL: the unchanged price at the same slot is fine and bumps no epoch.
    let out = accrue(&mut h, &mut m, 0, slot, P0, 0).unwrap();
    assert!(!out.price_move_active && out.dt == 0);
    assert_eq!(h.oracle_epoch.get(), h0.oracle_epoch.get());
}

#[test]
fn stale_mark_cap_boundary_is_exact() {
    // §5.3: abs(p - P_last) * 10_000 <= cap * dt * P_last, checked before any mutation.
    for dt in [1u64, 3, MAX_DT] {
        let max_delta = P0 * CAP_BPS * dt / 10_000;
        for up in [true, false] {
            let (mut h, mut m, _l, _s) = exposed(1);
            let slot = asset(&m, 0).slot_last + dt;
            let over = if up { P0 + max_delta + 1 } else { P0 - max_delta - 1 };
            let at = if up { P0 + max_delta } else { P0 - max_delta };
            let (h0, m0) = (h, m.clone());
            assert_eq!(
                accrue(&mut h, &mut m, 0, slot, over, 0),
                Err(V16Error::RecoveryRequired),
                "dt={dt} up={up}: one atom over the cap"
            );
            assert_eq!((h, &m), (h0, &m0), "refused accrual mutated state");
            let out = accrue(&mut h, &mut m, 0, slot, at, 0).unwrap();
            assert!(out.price_move_active);
            assert_eq!(asset(&m, 0).effective_price, at);
            assert_eq!(asset(&m, 0).slot_last, slot);
        }
    }
}

#[test]
fn stale_mark_unexposed_book_may_take_the_raw_target_in_one_jump() {
    // §1.7: with both OI sides zero no live position can lose equity, so the raw target may be
    // fed directly -- even a jump far beyond the per-slot cap, and even at dt == 0.
    let (mut h, mut m) = market(1, P0);
    let slot = asset(&m, 0).slot_last;
    let out = accrue(&mut h, &mut m, 0, slot, P0 * 50, 0).unwrap();
    assert!(!out.price_move_active && !out.equity_active);
    assert_eq!(asset(&m, 0).effective_price, P0 * 50);
    let out = accrue(&mut h, &mut m, 0, slot + 1, 1, 0).unwrap();
    assert!(!out.price_move_active);
    assert_eq!(asset(&m, 0).effective_price, 1);
}

#[test]
fn stale_mark_gap_beyond_max_accrual_dt_commits_one_bounded_segment_and_stays_loss_stale() {
    let (mut h, mut m, _l, _s) = exposed(1);
    let start = asset(&m, 0).slot_last;
    let now = start + 3 * MAX_DT + 4;
    // Engine design (lenient, not spec text: §5.3 says dt <= max_accrual_dt for an active
    // move): a longer gap commits one MAX_DT segment, and the cap is measured on that segment,
    // not on the full gap -- a move only reachable over the full gap is refused.
    let full_gap_move = P0 + P0 * CAP_BPS * (now - start) / 10_000;
    let (h0, m0) = (h, m.clone());
    assert_eq!(
        accrue(&mut h, &mut m, 0, now, full_gap_move, 0),
        Err(V16Error::RecoveryRequired)
    );
    assert_eq!((h, &m), (h0, &m0));

    // The full one-segment cap is accepted (on a twin, so the K move does not leave stale
    // settlement cohorts behind on the market used for the catch-up below).
    let seg_move = P0 + P0 * CAP_BPS * MAX_DT / 10_000;
    let (mut h2, mut m2) = (h, m.clone());
    let out = accrue(&mut h2, &mut m2, 0, now, seg_move, 0).unwrap();
    assert_eq!(out.dt, MAX_DT);
    assert_eq!(asset(&m2, 0).effective_price, seg_move);

    // Unchanged price: one bounded segment per call, loss-stale until fully caught up.
    let out = accrue(&mut h, &mut m, 0, now, P0, 0).unwrap();
    assert_eq!(out.dt, MAX_DT);
    assert_eq!(asset(&m, 0).slot_last, start + MAX_DT);
    assert!(out.loss_stale_after, "the asset is still behind the clock");
    assert_eq!(h.loss_stale_active, 1);
    assert_eq!(h.current_slot.get(), now);
    let mut guard = 0;
    while asset(&m, 0).slot_last < now {
        let out = accrue(&mut h, &mut m, 0, now, P0, 0).unwrap();
        assert!(out.dt <= MAX_DT);
        guard += 1;
        assert!(guard < 10);
    }
    assert_eq!(guard, 3, "34 slots = 10 + 10 + 10 + 4");
    assert_eq!(h.loss_stale_active, 0, "caught up -> no longer loss-stale");
}

#[test]
fn stale_mark_backwards_slot_and_invalid_prices_are_rejected_without_mutation() {
    let (mut h, mut m, _l, _s) = exposed(1);
    let s0 = asset(&m, 0).slot_last + 4;
    accrue(&mut h, &mut m, 0, s0, P0, 0).unwrap();
    let (h0, m0) = (h, m.clone());
    assert_eq!(accrue(&mut h, &mut m, 0, s0 - 1, P0, 0), Err(V16Error::InvalidConfig));
    assert_eq!(accrue(&mut h, &mut m, 0, s0 + 1, 0, 0), Err(V16Error::InvalidConfig));
    assert_eq!(
        accrue(&mut h, &mut m, 0, s0 + 1, MAX_ORACLE_PRICE + 1, 0),
        Err(V16Error::InvalidConfig)
    );
    assert_eq!(
        accrue(&mut h, &mut m, 0, s0 + 1, P0, FUNDING_MAX as i128 + 1),
        Err(V16Error::InvalidConfig)
    );
    assert_eq!((h, &m), (h0, &m0));
}

#[test]
fn stale_mark_price_move_without_committed_progress_is_refused() {
    let (mut h, mut m, _l, _s) = exposed(1);
    let slot = asset(&m, 0).slot_last + 1;
    let (h0, m0) = (h, m.clone());
    let res = MarketGroupV16ViewMut::new(&mut h, &mut m)
        .accrue_asset_to_not_atomic(0, slot, P0 + 1, 0, false);
    assert_eq!(res, Err(V16Error::NonProgress));
    assert_eq!((h, &m), (h0, &m0));
}

#[test]
fn stale_mark_epochs_price_move_bumps_oracle_epoch_funding_only_does_not() {
    let (mut h, mut m, _l, _s) = exposed(1);
    let (o0, f0) = (h.oracle_epoch.get(), h.funding_epoch.get());
    let s0 = asset(&m, 0).slot_last;
    let out = accrue(&mut h, &mut m, 0, s0 + 1, P0, 1_000).unwrap();
    assert!(out.funding_active && !out.price_move_active);
    assert_eq!((h.oracle_epoch.get(), h.funding_epoch.get()), (o0, f0 + 1));
    let out = accrue(&mut h, &mut m, 0, s0 + 2, P0 + 1, 0).unwrap();
    assert!(out.price_move_active && !out.funding_active);
    assert_eq!((h.oracle_epoch.get(), h.funding_epoch.get()), (o0 + 1, f0 + 1));
}

#[test]
fn stale_mark_exposed_book_reaches_max_oracle_price_and_rejects_above() {
    // Exposed book sitting at the top of the price domain: the cap check and K arithmetic
    // must neither overflow nor let the mark escape MAX_ORACLE_PRICE.
    let (mut h, mut m) = market(1, MAX_ORACLE_PRICE - 1);
    let mut l = account(50);
    let mut s = account(51);
    deposit(&mut h, &mut m, &mut l, 1_000_000_000_000_000);
    deposit(&mut h, &mut m, &mut s, 1_000_000_000_000_000);
    trade(&mut h, &mut m, &mut l, &mut s, 0, POS_SCALE as i128, MAX_ORACLE_PRICE - 1).unwrap();
    let s0 = asset(&m, 0).slot_last;
    accrue(&mut h, &mut m, 0, s0 + 1, MAX_ORACLE_PRICE, 0).unwrap();
    assert_eq!(asset(&m, 0).effective_price, MAX_ORACLE_PRICE);
    assert_eq!(
        accrue(&mut h, &mut m, 0, s0 + 2, MAX_ORACLE_PRICE + 1, 0),
        Err(V16Error::InvalidConfig)
    );
    MarketGroupV16ViewMut::new(&mut h, &mut m).validate_shape().unwrap();
}

// ======================================================= canonical path ===

#[test]
fn oracle_path_wrong_step_count_and_off_canonical_price_are_rejected_without_mutation() {
    let (mut h, mut m, _l, _s) = exposed(1);
    let target = P0 * 2;
    let now = asset(&m, 0).slot_last + 4;
    let steps = canonical_path(&m, 0, now, target);
    assert_eq!(steps.len(), 4);
    let (h0, m0) = (h, m.clone());

    assert_eq!(
        accrue_path(&mut h, &mut m, 0, now, target, &steps[..3]),
        Err(V16Error::InvalidConfig),
        "a short path would let the caller pick its own partition"
    );
    let mut nudged = steps.clone();
    nudged[2].effective_price += 1;
    assert_eq!(
        accrue_path(&mut h, &mut m, 0, now, target, &nudged),
        Err(V16Error::InvalidConfig),
        "an off-canonical price is refused"
    );
    let mut bad_rem = steps.clone();
    bad_rem[0].price_move_remainder_before_bps_num = 1;
    assert_eq!(
        accrue_path(&mut h, &mut m, 0, now, target, &bad_rem),
        Err(V16Error::InvalidConfig),
        "a new target restarts the carried remainder at 0"
    );
    assert_eq!((h, &m), (h0, &m0));

    accrue_path(&mut h, &mut m, 0, now, target, &steps).unwrap();
    assert_eq!(asset(&m, 0).effective_price, steps[3].effective_price);
    assert_eq!(asset(&m, 0).raw_oracle_target_price, target);
}

#[test]
fn oracle_path_zero_steps_records_the_new_target_and_bumps_the_oracle_epoch_once() {
    let (mut h, mut m, _l, _s) = exposed(1);
    let slot = asset(&m, 0).slot_last;
    let e0 = h.oracle_epoch.get();
    let out = accrue_path(&mut h, &mut m, 0, slot, P0 + 7, &[]).unwrap();
    assert_eq!(out.dt, 0);
    assert!(!out.price_move_active);
    let a = asset(&m, 0);
    assert_eq!((a.raw_oracle_target_price, a.effective_price), (P0 + 7, P0));
    assert_eq!(h.oracle_epoch.get(), e0 + 1);
    // Same target again with no steps: nothing new observed, no epoch bump.
    accrue_path(&mut h, &mut m, 0, slot, P0 + 7, &[]).unwrap();
    assert_eq!(h.oracle_epoch.get(), e0 + 1);
}

#[test]
fn oracle_path_low_price_sub_atom_carry_makes_progress_instead_of_pinning() {
    // At P = 50 and 1%/slot the per-step allowance is half an atom. Without the carried
    // remainder the mark would be pinned forever below an exposed target (§1.7 CatchupRequired).
    let (mut h, mut m) = market(1, 50);
    let mut l = account(60);
    let mut s = account(61);
    deposit(&mut h, &mut m, &mut l, 1_000_000);
    deposit(&mut h, &mut m, &mut s, 1_000_000);
    trade(&mut h, &mut m, &mut l, &mut s, 0, POS_SCALE as i128, 50).unwrap();
    let now = asset(&m, 0).slot_last + MAX_DT;
    let steps = canonical_path(&m, 0, now, 60);
    accrue_path(&mut h, &mut m, 0, now, 60, &steps).unwrap();
    assert_eq!(asset(&m, 0).effective_price, 55, "10 slots x 0.5 atom = 5 atoms");
    assert!(steps.iter().any(|s| s.price_move_remainder_after_bps_num != 0));
}

#[test]
fn oracle_path_never_overshoots_and_settles_on_the_target() {
    for target in [P0 + 25_000, P0 - 25_000] {
        let (mut h, mut m, _l, _s) = exposed(1);
        let now = asset(&m, 0).slot_last + MAX_DT;
        let steps = canonical_path(&m, 0, now, target);
        let mut prev = P0;
        for st in &steps {
            let p = st.effective_price;
            assert!(p.abs_diff(target) <= prev.abs_diff(target), "monotone toward target");
            assert!(
                p.abs_diff(prev) <= P0 * CAP_BPS / 10_000,
                "each step within the anchor-linear allowance (anchor = P0)"
            );
            prev = p;
        }
        accrue_path(&mut h, &mut m, 0, now, target, &steps).unwrap();
        assert_eq!(asset(&m, 0).effective_price, target, "2.5% reached within 10 slots");
        // Once caught up, the funding anchor is reset to the target and lag is gone.
        assert_eq!(asset(&m, 0).fund_px_last, target);
    }
}

#[test]
fn oracle_path_partition_does_not_change_the_endpoint() {
    // Canonical-path claim: transaction boundaries chosen by the caller cannot change price
    // movement. One 8-slot call vs. 3 + 5 slot calls under the same raw target.
    let target = P0 / 2;
    let (mut h1, mut m1, _l1, _s1) = exposed(1);
    let start = asset(&m1, 0).slot_last;
    let steps = canonical_path(&m1, 0, start + 8, target);
    accrue_path(&mut h1, &mut m1, 0, start + 8, target, &steps).unwrap();

    let (mut h2, mut m2, _l2, _s2) = exposed(1);
    let first = canonical_path(&m2, 0, start + 3, target);
    accrue_path(&mut h2, &mut m2, 0, start + 3, target, &first).unwrap();
    let second = canonical_path(&m2, 0, start + 8, target);
    accrue_path(&mut h2, &mut m2, 0, start + 8, target, &second).unwrap();

    let (a1, a2) = (asset(&m1, 0), asset(&m2, 0));
    assert_eq!(a1.effective_price, a2.effective_price);
    assert_eq!((a1.k_long, a1.k_short), (a2.k_long, a2.k_short));
    assert_eq!(a1.slot_last, a2.slot_last);
}

#[test]
fn oracle_path_endpoint_stays_inside_the_single_segment_cap_envelope() {
    // First call after a target change (anchor == P_last): a downward canonical path steps
    // linearly from its anchor, so an individual late step can exceed `cap * P_last`, but the
    // call's cumulative move is still what one §5.3 accrual over the same dt allows from the
    // starting P_last -- checked on a twin market. (Later calls keep the old anchor; see the
    // module doc.)
    let target = P0 / 2;
    let (mut h, mut m, _l, _s) = exposed(1);
    let start = asset(&m, 0).slot_last;
    let now = start + MAX_DT;
    let steps = canonical_path(&m, 0, now, target);
    accrue_path(&mut h, &mut m, 0, now, target, &steps).unwrap();
    let end = asset(&m, 0).effective_price;
    assert!(P0.abs_diff(end) * 10_000 <= CAP_BPS * MAX_DT * P0);

    let (mut h2, mut m2, _l2, _s2) = exposed(1);
    accrue(&mut h2, &mut m2, 0, now, end, 0).expect("single §5.3 accrual reaches the same endpoint");
    assert_eq!(asset(&m2, 0).k_long, asset(&m, 0).k_long, "same mark-to-market");
}

// ================================================= loss-stale / lag gates ===

#[test]
fn lag_blocks_risk_increase_until_the_path_catches_up() {
    let (mut h, mut m, mut l, mut s) = exposed(1);
    let mut l2 = account(42);
    let mut s2 = account(43);
    deposit(&mut h, &mut m, &mut l2, 10_000_000);
    deposit(&mut h, &mut m, &mut s2, 10_000_000);
    let target = P0 + 15_000;
    set_targets(&mut h, &mut m, &[(0, target)]).unwrap();
    assert_eq!(
        trade(&mut h, &mut m, &mut l2, &mut s2, 0, POS_SCALE as i128, P0),
        Err(V16Error::LockActive),
        "§1.7: risk-increasing trades are refused during target/effective divergence"
    );
    // A pure reduction is never blocked by lag (lenient for traders closing out).
    trade(&mut h, &mut m, &mut s, &mut l, 0, (POS_SCALE / 2) as i128, P0)
        .expect("reducing during lag stays open");

    let now = asset(&m, 0).slot_last + 2;
    let steps = canonical_path(&m, 0, now, target);
    accrue_path(&mut h, &mut m, 0, now, target, &steps).unwrap();
    assert_eq!(asset(&m, 0).effective_price, target);
    // The mark moved, so the existing legs form a stale settlement cohort: a new risk
    // increase still waits for them (loss-stale), and a keeper refresh of each clears it.
    assert_eq!(
        trade(&mut h, &mut m, &mut l2, &mut s2, 0, POS_SCALE as i128, target),
        Err(V16Error::LossStale)
    );
    refresh(&mut h, &mut m, &mut l);
    refresh(&mut h, &mut m, &mut s);
    trade(&mut h, &mut m, &mut l2, &mut s2, 0, POS_SCALE as i128, target)
        .expect("caught up and settled: the gate lifts");
}

/// Keeper Refresh crank at the committed mark, until the account is current.
fn refresh(h: &mut Header, m: &mut Markets, a: &mut PortfolioAccountV16Account) {
    let now_slot = h.current_slot.get();
    let price = asset(m, 0).effective_price;
    for _ in 0..64 {
        let mut market = MarketGroupV16ViewMut::new(h, m);
        let mut v = PortfolioV16ViewMut::new(a);
        market
            .permissionless_crank_not_atomic(
                &mut v,
                PermissionlessCrankRequestV16 {
                    now_slot,
                    asset_index: 0,
                    effective_price: price,
                    funding_rate_e9: 0,
                    action: PermissionlessCrankActionV16::Refresh,
                },
            )
            .expect("permissionless Refresh crank");
        if a.b_stale_state == 0 && a.stale_state == 0 {
            return;
        }
    }
    panic!("refresh did not converge");
}

#[test]
fn lag_on_one_asset_does_not_block_a_trade_on_another() {
    let (mut h, mut m, _l, _s) = exposed(2);
    set_targets(&mut h, &mut m, &[(0, P0 + 9)]).unwrap();
    let mut a = account(70);
    let mut b = account(71);
    deposit(&mut h, &mut m, &mut a, 10_000_000);
    deposit(&mut h, &mut m, &mut b, 10_000_000);
    trade(&mut h, &mut m, &mut a, &mut b, 1, POS_SCALE as i128, P0)
        .expect("asset 1 is current; asset 0's lag is unrelated");
}

#[test]
fn loss_stale_summary_flag_cleared_by_another_asset_does_not_unblock_the_stale_one() {
    // The header loss-stale flag only summarizes the last-touched asset. Accruing a fresh asset
    // clears it while asset 0 is still behind the clock; the per-asset gate must still refuse a
    // risk increase on asset 0 (conflicting summary vs. local state).
    let (mut h, mut m, mut l, mut s) = exposed(2);
    let start = asset(&m, 0).slot_last;
    let now = start + MAX_DT + 3;
    // Advance asset 0 one bounded segment: still stale.
    accrue(&mut h, &mut m, 0, now, P0, 0).unwrap();
    assert_eq!(h.loss_stale_active, 1);
    // Asset 1 is empty; accruing it to `now` clears the summary flag.
    accrue(&mut h, &mut m, 1, now, P0, 0).unwrap();
    assert_eq!(h.loss_stale_active, 0, "summary reflects only the touched asset");
    assert!(asset(&m, 0).slot_last < h.current_slot.get());
    assert_eq!(
        trade(&mut h, &mut m, &mut l, &mut s, 0, POS_SCALE as i128, P0),
        Err(V16Error::LossStale),
        "asset 0 is still loss-stale"
    );
    // NEGATIVE CONTROL: a reduction on the stale asset stays allowed.
    trade(&mut h, &mut m, &mut s, &mut l, 0, (POS_SCALE / 2) as i128, P0)
        .expect("reductions are never loss-stale gated");
}

#[test]
fn loss_stale_clears_once_the_lagging_asset_catches_up() {
    let (mut h, mut m, mut l, mut s) = exposed(2);
    // Another asset's accrual already moved the authoritative clock forward.
    let ahead = asset(&m, 0).slot_last + 4;
    accrue(&mut h, &mut m, 1, ahead, P0, 0).unwrap();
    assert_eq!(h.current_slot.get(), ahead);
    assert_eq!(
        trade(&mut h, &mut m, &mut l, &mut s, 0, POS_SCALE as i128, P0),
        Err(V16Error::LossStale)
    );
    let now = h.current_slot.get();
    accrue(&mut h, &mut m, 0, now, P0, 0).unwrap();
    trade(&mut h, &mut m, &mut l, &mut s, 0, POS_SCALE as i128, P0)
        .expect("asset current again: the gate lifts");
}

// ================================================= auto-crank observations ===

fn refresh_crank(
    h: &mut Header,
    m: &mut Markets,
    a: &mut PortfolioAccountV16Account,
    now_slot: u64,
    observations: &[AutoCrankObservationV16],
) -> Result<percolator::AutoCrankResultV16, V16Error> {
    let mut market = MarketGroupV16ViewMut::new(h, m);
    let mut v = PortfolioV16ViewMut::new(a);
    market.permissionless_auto_crank_not_atomic(
        &mut v,
        AutoCrankWorkV16 {
            now_slot,
            observations,
            resolved_close_fee_rate_per_slot: 0,
        },
    )
}

/// Exposed market where a funding-only accrual one slot later has made the long's certificate
/// stale, so the engine itself selects `RefreshAccount { Some(0) }` for it. Returns `now` = the
/// next slot.
fn needs_refresh(slots: u32) -> (Header, Markets, PortfolioAccountV16Account, u64) {
    let (mut h, mut m, l, _s) = exposed(slots);
    let s0 = asset(&m, 0).slot_last;
    accrue(&mut h, &mut m, 0, s0 + 1, P0, 1_000).unwrap();
    (h, m, l, s0 + 2)
}

#[test]
fn observation_missing_for_the_selected_asset_falls_back_to_the_committed_mark() {
    let (mut h, mut m, mut l, now) = needs_refresh(2);
    // Only an observation for an unrelated asset is supplied.
    let obs = [AutoCrankObservationV16 {
        asset_index: 1,
        effective_price: P0 * 3,
        funding_rate_e9: 0,
    }];
    let res = refresh_crank(&mut h, &mut m, &mut l, now, &obs).unwrap();
    assert_eq!(res.selected, AutoCrankPlanV16::RefreshAccount { asset_index: Some(0) });
    assert_eq!(asset(&m, 0).effective_price, P0, "no fresh price -> committed mark");
    assert_eq!(asset(&m, 0).slot_last, now, "the committed mark still advances the clock");
    assert_eq!(asset(&m, 1).effective_price, P0, "the unrelated observation is not applied");
}

#[test]
fn observation_conflicting_duplicates_never_apply_a_cap_violating_price() {
    let (mut h, mut m, mut l, now) = needs_refresh(1);
    let ok = AutoCrankObservationV16 {
        asset_index: 0,
        effective_price: P0 + 100,
        funding_rate_e9: 0,
    };
    let wild = AutoCrankObservationV16 {
        asset_index: 0,
        effective_price: P0 * 2,
        funding_rate_e9: 0,
    };
    // Whatever the engine does with a conflicting pair, the cap-violating price never lands.
    let (h0, m0, l0) = (h, m.clone(), l);
    if refresh_crank(&mut h, &mut m, &mut l, now, &[ok, wild]).is_ok() {
        let p = asset(&m, 0).effective_price;
        assert!(p.abs_diff(P0) * 10_000 <= CAP_BPS * P0, "applied price within the cap");
    }
    assert_ne!(asset(&m, 0).effective_price, P0 * 2);

    // Reversed order: a conflicting later observation cannot launder the wild one.
    let (mut h, mut m, mut l) = (h0, m0.clone(), l0);
    let res = refresh_crank(&mut h, &mut m, &mut l, now, &[wild, ok]);
    assert_ne!(asset(&m, 0).effective_price, P0 * 2);
    if res.is_ok() {
        let p = asset(&m, 0).effective_price;
        assert!(p.abs_diff(P0) * 10_000 <= CAP_BPS * P0, "applied price within the cap");
    } else {
        assert_eq!(asset(&m, 0).effective_price, P0, "a refused crank commits no mark");
    }
}

#[test]
fn loss_stale_lagging_asset_catch_up_below_the_global_clock_keeps_the_gate() {
    // Asset 1 has already advanced the market clock. Asset 0 may catch up to an EARLIER slot
    // (the clock stays monotonic), but it is still behind that clock, so a risk increase on it
    // stays refused. The header summary is computed at the accrual's own endpoint and is
    // only a hint; the trade gate re-derives staleness per asset against `current_slot`.
    let (mut h, mut m, mut l, mut s) = exposed(2);
    let start = asset(&m, 0).slot_last;
    accrue(&mut h, &mut m, 1, start + 8, P0, 0).unwrap();
    assert_eq!(h.current_slot.get(), start + 8);
    accrue(&mut h, &mut m, 0, start + 5, P0, 0).expect("a lagging asset may catch up partway");
    assert_eq!(h.current_slot.get(), start + 8, "global clock never moves backwards");
    assert_eq!(asset(&m, 0).slot_last, start + 5);
    assert_eq!(
        trade(&mut h, &mut m, &mut l, &mut s, 0, POS_SCALE as i128, P0),
        Err(V16Error::LossStale)
    );
    accrue(&mut h, &mut m, 0, start + 8, P0, 0).unwrap();
    trade(&mut h, &mut m, &mut l, &mut s, 0, POS_SCALE as i128, P0)
        .expect("asset 0 now current with the clock");
}

/// One canonical step built from a caller-chosen carried remainder.
fn step_with_remainder(price: u64, target: u64, anchor: u64, before: u16) -> AccrualStepV16 {
    let (next, after) =
        canonical_accrual_price_step_v16(price, target, anchor, CAP_BPS, true, before).unwrap();
    AccrualStepV16 {
        effective_price: next,
        funding_rate_e9: 0,
        price_move_remainder_before_bps_num: before,
        price_move_remainder_after_bps_num: after,
    }
}

#[test]
#[ignore = "known engine gap, reported in the PR: the carried sub-atom remainder is caller-supplied \
            and not persisted, so a path caller can exceed the §5.3 cap at low prices"]
fn oracle_path_caller_chosen_remainder_cannot_beat_the_carried_path() {
    // P = 50, 1%/slot: the honest allowance is half an atom per slot. Over 10 slots both the
    // honestly carried canonical path and the cumulative §5.3 envelope
    // (`50 * 100 * 10 / 10_000 = 5`) allow +5 atoms.
    //
    // The engine checks a call's first-step remainder only when the target CHANGES (must be 0)
    // and otherwise only that it is < 10_000; nothing ties it to the previous call's
    // `remainder_after`, because no remainder is stored in asset state. A caller who restarts
    // each one-slot call with remainder 9_999 gets a full atom every slot: 2x the cap.
    let (mut h, mut m) = market(1, 50);
    let mut l = account(80);
    let mut s = account(81);
    deposit(&mut h, &mut m, &mut l, 1_000_000);
    deposit(&mut h, &mut m, &mut s, 1_000_000);
    trade(&mut h, &mut m, &mut l, &mut s, 0, POS_SCALE as i128, 50).unwrap();
    let target = 60;
    let start = asset(&m, 0).slot_last;

    // Slot 1: the target change itself (remainder must start at 0; the engine enforces this).
    let first = step_with_remainder(50, target, 50, 0);
    accrue_path(&mut h, &mut m, 0, start + 1, target, &[first]).unwrap();
    // Slots 2..=10: same target, each call claims a 9_999 carried remainder. Every call is
    // accepted today. A fixed engine may refuse such a step instead; that is a pass too.
    for k in 2..=10u64 {
        let a = asset(&m, 0);
        let step = step_with_remainder(a.effective_price, target, a.fund_px_last, 9_999);
        if accrue_path(&mut h, &mut m, 0, start + k, target, &[step]).is_err() {
            break;
        }
    }
    let reached = asset(&m, 0).effective_price;
    let spec_max = 50 + 50 * CAP_BPS * 10 / 10_000;
    assert!(
        reached <= spec_max,
        "10 slots at 1%/slot from 50 may reach at most {spec_max}; reached {reached}"
    );
}
