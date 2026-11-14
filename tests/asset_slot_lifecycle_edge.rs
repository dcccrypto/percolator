//! Edge-case tests for the v2.2 market/asset slot lifecycle: activate an empty slot, retire,
//! canonicalize a retired slot, deregister an empty portfolio, and slot capacity from account
//! length. Tests only; no product code is touched.
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]
use percolator::{
    AssetLifecycleV16, EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount,
    MarketGroupV16ViewMut, PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, V16Config, V16Error, V16PodU128, MAX_ORACLE_PRICE, MAX_VAULT_TVL,
};

const PRICE: u64 = 1_000;

type Hdr = MarketGroupV16HeaderAccount;

fn empty_group(
    max_slots: u32,
    capacity: u32,
) -> (Hdr, Vec<Market<u64>>) {
    let cfg = V16Config::public_user_fund_with_market_slots(max_slots as u16, max_slots, 0, 10);
    let header = Hdr::new_dynamic([1; 32], cfg, capacity, 0).unwrap();
    let markets = (0..capacity as u64)
        .map(|i| Market::new(i, EngineAssetSlotV16Account::default()))
        .collect();
    (header, markets)
}

fn account(seed: u8) -> PortfolioAccountV16Account {
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        [1; 32],
        [seed; 32],
        [3; 32],
    ));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(header).unwrap();
    a
}

fn lifecycle(m: &Market<u64>) -> AssetLifecycleV16 {
    m.engine.asset.try_to_runtime().unwrap().lifecycle
}

fn money(h: &Hdr) -> (u128, u128, u128) {
    (h.vault.get(), h.c_tot.get(), h.insurance.get())
}

// ---------------------------------------------------------------------------
// slot capacity from account length / slot offset
// ---------------------------------------------------------------------------

#[test]
fn capacity_from_len_header_only_is_zero() {
    let header_len = Hdr::dynamic_market_group_account_len::<()>(0).unwrap();
    assert_eq!(
        Hdr::dynamic_asset_slot_capacity_from_account_len::<()>(header_len),
        Ok(0)
    );
}

#[test]
fn capacity_from_len_shorter_than_header_is_rejected() {
    let header_len = Hdr::dynamic_market_group_account_len::<()>(0).unwrap();
    assert_eq!(
        Hdr::dynamic_asset_slot_capacity_from_account_len::<()>(0),
        Err(V16Error::InvalidConfig)
    );
    assert_eq!(
        Hdr::dynamic_asset_slot_capacity_from_account_len::<()>(header_len - 1),
        Err(V16Error::InvalidConfig)
    );
}

#[test]
fn capacity_from_len_round_trips_exact_lengths() {
    for cap in [0usize, 1, 2, 3, 7, 64, 1_000] {
        let len = Hdr::dynamic_market_group_account_len::<()>(cap).unwrap();
        assert_eq!(
            Hdr::dynamic_asset_slot_capacity_from_account_len::<()>(len),
            Ok(cap),
            "cap {cap}"
        );
        assert_eq!(
            Hdr::validate_dynamic_market_group_account_len::<()>(len, cap),
            Ok(())
        );
    }
}

#[test]
fn capacity_from_len_rejects_partial_trailing_slot() {
    // One byte short of the next whole slot, and one byte past a whole slot.
    let l1 = Hdr::dynamic_market_group_account_len::<()>(1).unwrap();
    let l2 = Hdr::dynamic_market_group_account_len::<()>(2).unwrap();
    let stride = l2 - l1;
    for len in [l1 + 1, l1 + stride - 1, l2 + 1] {
        assert_eq!(
            Hdr::dynamic_asset_slot_capacity_from_account_len::<()>(len),
            Err(V16Error::InvalidConfig),
            "len {len}"
        );
    }
    // ...but the next whole slot is accepted again.
    assert_eq!(
        Hdr::dynamic_asset_slot_capacity_from_account_len::<()>(l1 + stride),
        Ok(2)
    );
}

#[test]
fn capacity_from_len_huge_account_len_does_not_overflow() {
    // usize::MAX is not a whole number of slots past the header (or is, and then it is huge);
    // either way it must not panic.
    let r = Hdr::dynamic_asset_slot_capacity_from_account_len::<()>(usize::MAX);
    match r {
        Ok(cap) => {
            let back = Hdr::dynamic_market_group_account_len::<()>(cap).unwrap();
            assert_eq!(back, usize::MAX);
        }
        Err(e) => assert_eq!(e, V16Error::InvalidConfig),
    }
}

#[test]
fn validate_account_len_rejects_off_by_one_both_ways() {
    let len = Hdr::dynamic_market_group_account_len::<()>(5).unwrap();
    assert_eq!(
        Hdr::validate_dynamic_market_group_account_len::<()>(len - 1, 5),
        Err(V16Error::InvalidConfig)
    );
    assert_eq!(
        Hdr::validate_dynamic_market_group_account_len::<()>(len + 1, 5),
        Err(V16Error::InvalidConfig)
    );
    assert_eq!(
        Hdr::validate_dynamic_market_group_account_len::<()>(len, 4),
        Err(V16Error::InvalidConfig)
    );
}

#[test]
fn slot_offset_is_header_plus_stride_times_index() {
    let l0 = Hdr::dynamic_market_group_account_len::<()>(0).unwrap();
    let l1 = Hdr::dynamic_market_group_account_len::<()>(1).unwrap();
    let stride = l1 - l0;
    assert!(stride > 0);
    assert_eq!(Hdr::dynamic_asset_slot_offset::<()>(0), Ok(l0));
    for i in [1usize, 2, 9, 1_000] {
        assert_eq!(Hdr::dynamic_asset_slot_offset::<()>(i), Ok(l0 + i * stride));
        // slot i ends exactly where an account of capacity i+1 ends
        assert_eq!(
            Hdr::dynamic_asset_slot_offset::<()>(i).unwrap() + stride,
            Hdr::dynamic_market_group_account_len::<()>(i + 1).unwrap()
        );
    }
}

#[test]
fn slot_offset_and_len_overflow_are_errors_not_wraps() {
    assert_eq!(
        Hdr::dynamic_asset_slot_offset::<()>(usize::MAX),
        Err(V16Error::ArithmeticOverflow)
    );
    assert_eq!(
        Hdr::dynamic_market_group_account_len::<()>(usize::MAX),
        Err(V16Error::ArithmeticOverflow)
    );
    // index whose product fits but whose sum with the header does not
    let l0 = Hdr::dynamic_market_group_account_len::<()>(0).unwrap();
    let l1 = Hdr::dynamic_market_group_account_len::<()>(1).unwrap();
    let stride = l1 - l0;
    let top = usize::MAX / stride;
    let expected = top.checked_mul(stride).and_then(|p| p.checked_add(l0));
    match expected {
        Some(v) => assert_eq!(Hdr::dynamic_asset_slot_offset::<()>(top), Ok(v)),
        None => assert_eq!(
            Hdr::dynamic_asset_slot_offset::<()>(top),
            Err(V16Error::ArithmeticOverflow)
        ),
    }
    // one slot more than that cannot fit
    assert_eq!(
        Hdr::dynamic_asset_slot_offset::<()>(top + 1),
        Err(V16Error::ArithmeticOverflow)
    );
}

// ---------------------------------------------------------------------------
// activate_empty_asset_slot_not_atomic: bounds and argument edges
// ---------------------------------------------------------------------------

#[test]
fn activate_rejects_index_at_or_past_capacity() {
    let (mut h, mut m) = empty_group(2, 2);
    assert_eq!(
        h.activate_empty_asset_slot_not_atomic(2, &mut m[0].engine, PRICE, 1),
        Err(V16Error::InvalidLeg)
    );
    assert_eq!(
        h.activate_empty_asset_slot_not_atomic(u32::MAX, &mut m[0].engine, PRICE, 1),
        Err(V16Error::InvalidLeg)
    );
    assert_eq!(h.asset_activation_count.get(), 0);
}

#[test]
fn activate_rejects_index_past_configured_slots_even_with_spare_capacity() {
    // capacity 4 but only 2 configured market slots: indexes 2 and 3 are physically there
    // but not usable.
    let (mut h, mut m) = empty_group(2, 4);
    assert_eq!(
        h.activate_empty_asset_slot_not_atomic(2, &mut m[2].engine, PRICE, 1),
        Err(V16Error::InvalidLeg)
    );
    assert_eq!(lifecycle(&m[2]), AssetLifecycleV16::Disabled);
    h.activate_empty_asset_slot_not_atomic(1, &mut m[1].engine, PRICE, 1)
        .unwrap();
}

#[test]
fn activate_price_bounds() {
    let (mut h, mut m) = empty_group(1, 1);
    for bad in [0u64, MAX_ORACLE_PRICE + 1, u64::MAX] {
        assert_eq!(
            h.activate_empty_asset_slot_not_atomic(0, &mut m[0].engine, bad, 1),
            Err(V16Error::InvalidConfig),
            "price {bad}"
        );
    }
    assert_eq!(lifecycle(&m[0]), AssetLifecycleV16::Disabled);
    h.activate_empty_asset_slot_not_atomic(0, &mut m[0].engine, MAX_ORACLE_PRICE, 1)
        .unwrap();
    assert_eq!(m[0].engine.asset.effective_price.get(), MAX_ORACLE_PRICE);

    let (mut h, mut m) = empty_group(1, 1);
    h.activate_empty_asset_slot_not_atomic(0, &mut m[0].engine, 1, 1)
        .unwrap();
    assert_eq!(m[0].engine.asset.effective_price.get(), 1);
}

#[test]
fn activate_first_slot_at_slot_zero_is_allowed() {
    let (mut h, mut m) = empty_group(1, 1);
    h.activate_empty_asset_slot_not_atomic(0, &mut m[0].engine, PRICE, 0)
        .unwrap();
    assert_eq!(lifecycle(&m[0]), AssetLifecycleV16::Active);
    assert_eq!(h.current_slot.get(), 0);
    assert_eq!(h.last_asset_activation_slot.get(), 0);
    assert_eq!(h.asset_activation_count.get(), 1);
}

#[test]
fn activate_rejects_time_going_backwards() {
    let (mut h, mut m) = empty_group(2, 2);
    h.activate_empty_asset_slot_not_atomic(0, &mut m[0].engine, PRICE, 10)
        .unwrap();
    assert_eq!(
        h.activate_empty_asset_slot_not_atomic(1, &mut m[1].engine, PRICE, 9),
        Err(V16Error::InvalidConfig)
    );
    assert_eq!(lifecycle(&m[1]), AssetLifecycleV16::Disabled);
}

#[test]
fn activate_cooldown_boundary_is_exact() {
    // cooldown is 1 slot: same slot is locked, next slot is fine.
    let (mut h, mut m) = empty_group(3, 3);
    let cooldown = h.config.try_to_runtime_shape().unwrap().asset_activation_cooldown_slots;
    assert!(cooldown >= 1);
    h.activate_empty_asset_slot_not_atomic(0, &mut m[0].engine, PRICE, 5)
        .unwrap();
    assert_eq!(
        h.activate_empty_asset_slot_not_atomic(1, &mut m[1].engine, PRICE, 5 + cooldown - 1),
        Err(V16Error::LockActive)
    );
    h.activate_empty_asset_slot_not_atomic(1, &mut m[1].engine, PRICE, 5 + cooldown)
        .unwrap();
}

// ---------------------------------------------------------------------------
// activate: repeated calls, atomicity of failures, counters, conservation
// ---------------------------------------------------------------------------

#[test]
fn activate_twice_on_same_slot_is_rejected_and_changes_nothing() {
    let (mut h, mut m) = empty_group(1, 1);
    h.activate_empty_asset_slot_not_atomic(0, &mut m[0].engine, PRICE, 1)
        .unwrap();
    let h_before = h;
    let s_before = m[0].engine;
    assert_eq!(
        h.activate_empty_asset_slot_not_atomic(0, &mut m[0].engine, PRICE * 2, 100),
        Err(V16Error::LockActive)
    );
    assert_eq!(h, h_before);
    assert_eq!(m[0].engine, s_before);
}

#[test]
fn failed_activation_leaves_header_and_slot_untouched() {
    let (mut h, mut m) = empty_group(2, 2);
    h.activate_empty_asset_slot_not_atomic(0, &mut m[0].engine, PRICE, 3)
        .unwrap();
    let h_before = h;
    let s_before = m[1].engine;
    // cooldown, price, time, and index failures
    let _ = h.activate_empty_asset_slot_not_atomic(1, &mut m[1].engine, PRICE, 3);
    let _ = h.activate_empty_asset_slot_not_atomic(1, &mut m[1].engine, 0, 9);
    let _ = h.activate_empty_asset_slot_not_atomic(1, &mut m[1].engine, PRICE, 2);
    let _ = h.activate_empty_asset_slot_not_atomic(7, &mut m[1].engine, PRICE, 9);
    assert_eq!(h, h_before);
    assert_eq!(m[1].engine, s_before);
}

#[test]
fn activation_counters_and_market_ids_are_strictly_monotonic() {
    let n = 4u32;
    let (mut h, mut m) = empty_group(n, n);
    let mut ids = Vec::new();
    let (mut set_epoch, mut risk_epoch) = (h.asset_set_epoch.get(), h.risk_epoch.get());
    for i in 0..n as usize {
        h.activate_empty_asset_slot_not_atomic(i as u32, &mut m[i].engine, PRICE, 1 + i as u64)
            .unwrap();
        ids.push(m[i].engine.asset.market_id.get());
        assert_eq!(h.asset_activation_count.get(), i as u64 + 1);
        assert_eq!(h.asset_set_epoch.get(), set_epoch + 1);
        assert_eq!(h.risk_epoch.get(), risk_epoch + 1);
        set_epoch += 1;
        risk_epoch += 1;
        assert_eq!(h.current_slot.get(), 1 + i as u64);
        assert_eq!(h.last_asset_activation_slot.get(), 1 + i as u64);
    }
    assert!(ids.windows(2).all(|w| w[0] < w[1]), "ids {ids:?}");
    assert!(ids.iter().all(|&id| id != 0));
    assert_eq!(h.next_market_id.get(), ids[ids.len() - 1] + 1);
}

#[test]
fn activation_in_any_order_is_allowed_and_conserves_value() {
    let (mut h, mut m) = empty_group(3, 3);
    let before = money(&h);
    for (k, i) in [2usize, 0, 1].into_iter().enumerate() {
        h.activate_empty_asset_slot_not_atomic(i as u32, &mut m[i].engine, PRICE, 1 + k as u64)
            .unwrap();
    }
    assert_eq!(money(&h), before);
    assert!(m.iter().all(|x| lifecycle(x) == AssetLifecycleV16::Active));
    MarketGroupV16ViewMut::new(&mut h, &mut m).validate_shape().unwrap();
}

#[test]
fn activated_slot_starts_with_zero_budgets_and_empty_state() {
    let (mut h, mut m) = empty_group(1, 1);
    h.activate_empty_asset_slot_not_atomic(0, &mut m[0].engine, PRICE, 1)
        .unwrap();
    let s = &m[0].engine;
    assert_eq!(s.insurance_domain_budget_long.get(), 0);
    assert_eq!(s.insurance_domain_budget_short.get(), 0);
    assert_eq!(s.insurance_domain_spent_long.get(), 0);
    assert_eq!(s.insurance_domain_spent_short.get(), 0);
    let a = s.asset.try_to_runtime().unwrap();
    assert_eq!(a.effective_price, PRICE);
    assert_eq!(a.slot_last, 1);
    assert_eq!(a.market_id, h.next_market_id.get() - 1);
}

// ---------------------------------------------------------------------------
// retire, reuse, cooldown, ordering
// ---------------------------------------------------------------------------

#[test]
fn retire_then_reactivate_respects_cooldown_boundary() {
    let (mut h, mut m) = empty_group(1, 1);
    h.activate_empty_asset_slot_not_atomic(0, &mut m[0].engine, PRICE, 1)
        .unwrap();
    let old_id = m[0].engine.asset.market_id.get();
    MarketGroupV16ViewMut::new(&mut h, &mut m)
        .retire_empty_asset_not_atomic(0, 10)
        .unwrap();
    assert_eq!(lifecycle(&m[0]), AssetLifecycleV16::Retired);
    assert_eq!(m[0].engine.asset.try_to_runtime().unwrap().retired_slot, 10);
    // same slot as retirement: cooldown not elapsed
    assert_eq!(
        h.activate_empty_asset_slot_not_atomic(0, &mut m[0].engine, PRICE, 10),
        Err(V16Error::LockActive)
    );
    assert_eq!(lifecycle(&m[0]), AssetLifecycleV16::Retired);
    h.activate_empty_asset_slot_not_atomic(0, &mut m[0].engine, PRICE, 11)
        .unwrap();
    assert_eq!(lifecycle(&m[0]), AssetLifecycleV16::Active);
    assert!(m[0].engine.asset.market_id.get() > old_id);
}

#[test]
fn retire_is_idempotent_and_keeps_first_retired_slot() {
    let (mut h, mut m) = empty_group(1, 1);
    h.activate_empty_asset_slot_not_atomic(0, &mut m[0].engine, PRICE, 1)
        .unwrap();
    let mut v = MarketGroupV16ViewMut::new(&mut h, &mut m);
    v.retire_empty_asset_not_atomic(0, 5).unwrap();
    let epoch = v.header.asset_set_epoch.get();
    v.retire_empty_asset_not_atomic(0, 9).unwrap();
    assert_eq!(v.header.asset_set_epoch.get(), epoch, "second retire must not bump epoch");
    assert_eq!(
        v.markets[0].engine.asset.try_to_runtime().unwrap().retired_slot,
        5
    );
}

#[test]
fn retire_rejects_stale_time_and_out_of_range_index() {
    let (mut h, mut m) = empty_group(2, 2);
    h.activate_empty_asset_slot_not_atomic(0, &mut m[0].engine, PRICE, 8)
        .unwrap();
    let mut v = MarketGroupV16ViewMut::new(&mut h, &mut m);
    assert_eq!(v.retire_empty_asset_not_atomic(0, 7), Err(V16Error::Stale));
    assert_eq!(v.retire_empty_asset_not_atomic(2, 9), Err(V16Error::InvalidLeg));
    assert_eq!(v.retire_empty_asset_not_atomic(usize::MAX, 9), Err(V16Error::InvalidLeg));
    // retire at exactly current_slot is fine
    v.retire_empty_asset_not_atomic(0, 8).unwrap();
}

#[test]
fn retire_never_activated_slot_is_rejected() {
    let (mut h, mut m) = empty_group(1, 1);
    let mut v = MarketGroupV16ViewMut::new(&mut h, &mut m);
    assert_eq!(v.retire_empty_asset_not_atomic(0, 1), Err(V16Error::LockActive));
}

#[test]
fn retire_conserves_value_and_bumps_epochs_once() {
    let (mut h, mut m) = empty_group(2, 2);
    for i in 0..2usize {
        h.activate_empty_asset_slot_not_atomic(i as u32, &mut m[i].engine, PRICE, 1 + i as u64)
            .unwrap();
    }
    let before = money(&h);
    let (se, re) = (h.asset_set_epoch.get(), h.risk_epoch.get());
    let mut v = MarketGroupV16ViewMut::new(&mut h, &mut m);
    v.retire_empty_asset_not_atomic(1, 5).unwrap();
    assert_eq!(money(v.header), before);
    assert_eq!(v.header.asset_set_epoch.get(), se + 1);
    assert_eq!(v.header.risk_epoch.get(), re + 1);
    assert_eq!(v.header.current_slot.get(), 5);
    // the other slot is untouched
    assert_eq!(lifecycle(&v.markets[0]), AssetLifecycleV16::Active);
    v.validate_shape().unwrap();
}

#[test]
fn retire_reuse_cycles_never_reuse_a_market_id() {
    let (mut h, mut m) = empty_group(1, 1);
    let mut seen = Vec::new();
    let mut slot = 1u64;
    for _ in 0..6 {
        h.activate_empty_asset_slot_not_atomic(0, &mut m[0].engine, PRICE, slot)
            .unwrap();
        let id = m[0].engine.asset.market_id.get();
        assert!(!seen.contains(&id), "market id {id} reused");
        seen.push(id);
        slot += 1;
        MarketGroupV16ViewMut::new(&mut h, &mut m)
            .retire_empty_asset_not_atomic(0, slot)
            .unwrap();
        slot += 1;
    }
    MarketGroupV16ViewMut::new(&mut h, &mut m).validate_shape().unwrap();
}

// ---------------------------------------------------------------------------
// canonicalize_retired_empty_asset_slot_not_atomic
// ---------------------------------------------------------------------------

#[test]
fn canonicalize_rejects_active_and_never_activated_slots() {
    let (mut h, mut m) = empty_group(2, 2);
    h.activate_empty_asset_slot_not_atomic(0, &mut m[0].engine, PRICE, 1)
        .unwrap();
    let mut v = MarketGroupV16ViewMut::new(&mut h, &mut m);
    assert_eq!(
        v.canonicalize_retired_empty_asset_slot_not_atomic(0),
        Err(V16Error::LockActive)
    );
    assert_eq!(
        v.canonicalize_retired_empty_asset_slot_not_atomic(1),
        Err(V16Error::LockActive)
    );
    assert_eq!(
        v.canonicalize_retired_empty_asset_slot_not_atomic(2),
        Err(V16Error::InvalidLeg)
    );
}

#[test]
fn canonicalize_retired_slot_is_idempotent_and_value_neutral() {
    let (mut h, mut m) = empty_group(1, 1);
    h.activate_empty_asset_slot_not_atomic(0, &mut m[0].engine, PRICE, 1)
        .unwrap();
    let mut v = MarketGroupV16ViewMut::new(&mut h, &mut m);
    v.retire_empty_asset_not_atomic(0, 4).unwrap();
    let money_before = money(v.header);
    let old = v.markets[0].engine.asset.try_to_runtime().unwrap();
    v.canonicalize_retired_empty_asset_slot_not_atomic(0).unwrap();
    let once = v.markets[0].engine;
    v.canonicalize_retired_empty_asset_slot_not_atomic(0).unwrap();
    assert_eq!(v.markets[0].engine, once, "second canonicalize must be a no-op");
    let a = v.markets[0].engine.asset.try_to_runtime().unwrap();
    assert_eq!(a.lifecycle, AssetLifecycleV16::Retired);
    assert_eq!(a.market_id, old.market_id);
    assert_eq!(a.retired_slot, old.retired_slot);
    assert_eq!(money(v.header), money_before);
    v.validate_shape().unwrap();
}

#[test]
fn canonicalize_rejects_slot_with_unspent_budget_and_changes_nothing() {
    let (mut h, mut m) = empty_group(1, 1);
    h.activate_empty_asset_slot_not_atomic(0, &mut m[0].engine, PRICE, 1)
        .unwrap();
    MarketGroupV16ViewMut::new(&mut h, &mut m)
        .retire_empty_asset_not_atomic(0, 4)
        .unwrap();
    for (long, short, spent_l, spent_s) in
        [(1u128, 0u128, 0u128, 0u128), (0, 1, 0, 0), (0, 0, 1, 0), (0, 0, 0, 1)]
    {
        let mut slot = m[0].engine;
        slot.insurance_domain_budget_long = V16PodU128::new(long);
        slot.insurance_domain_budget_short = V16PodU128::new(short);
        slot.insurance_domain_spent_long = V16PodU128::new(spent_l);
        slot.insurance_domain_spent_short = V16PodU128::new(spent_s);
        let mut mm = vec![Market::new(0, slot)];
        let mut hh = h;
        let mut v = MarketGroupV16ViewMut::new(&mut hh, &mut mm);
        assert_eq!(
            v.canonicalize_retired_empty_asset_slot_not_atomic(0),
            Err(V16Error::LockActive),
            "budget {long}/{short} spent {spent_l}/{spent_s}"
        );
        assert_eq!(mm[0].engine, slot);
    }
}

#[test]
fn canonicalized_slot_can_be_reactivated_after_cooldown() {
    let (mut h, mut m) = empty_group(1, 1);
    h.activate_empty_asset_slot_not_atomic(0, &mut m[0].engine, PRICE, 1)
        .unwrap();
    {
        let mut v = MarketGroupV16ViewMut::new(&mut h, &mut m);
        v.retire_empty_asset_not_atomic(0, 4).unwrap();
        v.canonicalize_retired_empty_asset_slot_not_atomic(0).unwrap();
    }
    assert_eq!(
        h.activate_empty_asset_slot_not_atomic(0, &mut m[0].engine, PRICE, 4),
        Err(V16Error::LockActive)
    );
    h.activate_empty_asset_slot_not_atomic(0, &mut m[0].engine, PRICE, 5)
        .unwrap();
    assert_eq!(lifecycle(&m[0]), AssetLifecycleV16::Active);
}

// ---------------------------------------------------------------------------
// deregister_empty_materialized_portfolio_not_atomic
// ---------------------------------------------------------------------------

fn one_market() -> (Hdr, Vec<Market<u64>>) {
    let (mut h, mut m) = empty_group(1, 1);
    h.activate_empty_asset_slot_not_atomic(0, &mut m[0].engine, PRICE, 1)
        .unwrap();
    (h, m)
}

#[test]
fn deregister_with_zero_registered_underflows_cleanly() {
    let (mut h, mut m) = one_market();
    let mut acct = account(1);
    let av = PortfolioV16ViewMut::new(&mut acct);
    let mut v = MarketGroupV16ViewMut::new(&mut h, &mut m);
    assert_eq!(
        v.deregister_empty_materialized_portfolio_not_atomic(&av.as_view()),
        Err(V16Error::CounterUnderflow)
    );
    assert_eq!(v.header.materialized_portfolio_count.get(), 0);
}

#[test]
fn register_deregister_round_trip_and_repeat_counts() {
    let (mut h, mut m) = one_market();
    let mut a1 = account(1);
    let mut a2 = account(2);
    let v1 = PortfolioV16ViewMut::new(&mut a1);
    let v2 = PortfolioV16ViewMut::new(&mut a2);
    let mut v = MarketGroupV16ViewMut::new(&mut h, &mut m);
    let before = money(v.header);
    v.register_empty_materialized_portfolio_not_atomic(&v1.as_view()).unwrap();
    v.register_empty_materialized_portfolio_not_atomic(&v2.as_view()).unwrap();
    assert_eq!(v.header.materialized_portfolio_count.get(), 2);
    v.deregister_empty_materialized_portfolio_not_atomic(&v1.as_view()).unwrap();
    assert_eq!(v.header.materialized_portfolio_count.get(), 1);
    v.deregister_empty_materialized_portfolio_not_atomic(&v2.as_view()).unwrap();
    assert_eq!(v.header.materialized_portfolio_count.get(), 0);
    // one more is an underflow, and the count stays at zero
    assert_eq!(
        v.deregister_empty_materialized_portfolio_not_atomic(&v2.as_view()),
        Err(V16Error::CounterUnderflow)
    );
    assert_eq!(v.header.materialized_portfolio_count.get(), 0);
    assert_eq!(money(v.header), before);
}

#[test]
fn deregister_rejects_account_holding_capital_and_conserves_value() {
    let (mut h, mut m) = one_market();
    let mut acct = account(1);
    {
        let mut v = MarketGroupV16ViewMut::new(&mut h, &mut m);
        let mut av = PortfolioV16ViewMut::new(&mut acct);
        v.register_empty_materialized_portfolio_not_atomic(&av.as_view()).unwrap();
        v.deposit_not_atomic(&mut av, 1).unwrap();
    }
    let before = money(&h);
    let mut v = MarketGroupV16ViewMut::new(&mut h, &mut m);
    let av = PortfolioV16ViewMut::new(&mut acct);
    assert_eq!(
        v.deregister_empty_materialized_portfolio_not_atomic(&av.as_view()),
        Err(V16Error::LockActive)
    );
    assert_eq!(v.header.materialized_portfolio_count.get(), 1);
    assert_eq!(money(v.header), before);
    assert_eq!(before.0, 1);
}

#[test]
fn register_rejects_non_empty_account() {
    let (mut h, mut m) = one_market();
    let mut acct = account(1);
    let mut v = MarketGroupV16ViewMut::new(&mut h, &mut m);
    let mut av = PortfolioV16ViewMut::new(&mut acct);
    v.deposit_not_atomic(&mut av, 5).unwrap();
    assert_eq!(
        v.register_empty_materialized_portfolio_not_atomic(&av.as_view()),
        Err(V16Error::LockActive)
    );
    assert_eq!(v.header.materialized_portfolio_count.get(), 0);
}

// ---------------------------------------------------------------------------
// BUG (kept as an ignored failing test): a slot retired at slot 0 can never be reused.
// ---------------------------------------------------------------------------

/// `retire_empty_asset_not_atomic` (src/v16.rs:26692) accepts now_slot = 0 and stores
/// `retired_slot = 0`. Both `activate_empty_market_slot_not_atomic` (src/v16.rs:9745, the
/// `previous_asset.retired_slot == 0` check) and
/// `canonicalize_retired_empty_asset_slot_not_atomic` (src/v16.rs:26911) use `retired_slot == 0`
/// to mean "not a valid retired slot" and return LockActive. So a slot that is activated and
/// retired while the group clock is still 0 stays Retired forever, however late the caller is.
#[test]
#[ignore = "BUG: slot retired at slot 0 is stuck (retired_slot == 0 is treated as invalid)"]
fn slot_retired_at_slot_zero_can_be_reactivated_later() {
    let (mut h, mut m) = empty_group(1, 1);
    h.activate_empty_asset_slot_not_atomic(0, &mut m[0].engine, PRICE, 0)
        .unwrap();
    MarketGroupV16ViewMut::new(&mut h, &mut m)
        .retire_empty_asset_not_atomic(0, 0)
        .unwrap();
    assert_eq!(m[0].engine.asset.try_to_runtime().unwrap().retired_slot, 0);
    assert_eq!(
        MarketGroupV16ViewMut::new(&mut h, &mut m)
            .canonicalize_retired_empty_asset_slot_not_atomic(0),
        Ok(())
    );
    assert_eq!(
        h.activate_empty_asset_slot_not_atomic(0, &mut m[0].engine, PRICE, 1_000_000),
        Ok(())
    );
}
