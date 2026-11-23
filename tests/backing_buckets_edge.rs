//! Edge cases for counterparty backing deposits, provider earnings and bucket expiry (v2.2).
//!
//! Covers `deposit_fresh_counterparty_backing_not_atomic`,
//! `credit_backing_provider_earnings_not_atomic`, `expire_source_backing_bucket_not_atomic`
//! and `adjust_slot_provider_principal`. (`add_fresh_counterparty_backing_not_atomic` is
//! cfg(kani/fuzz) only; deposit goes through the same unchecked add, so it is covered via deposit.)
//!
//! Does not repeat `backing_double_claim_fuzz.rs` or `fees_liens_edge.rs`.
//! Run: `cargo test --test backing_buckets_edge`.

use percolator::{
    adjust_slot_provider_principal, BackingBucketStatusV16, BackingBucketV16, EngineAssetSlotV16Account,
    Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, V16Config, V16Error, V16PodU128,
    V16PodU64, BOUND_SCALE,
};

type Fixture = (MarketGroupV16HeaderAccount, Vec<Market<u64>>);

fn fixture() -> Fixture {
    let cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut markets = vec![Market::new(0u64, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, 100, 1)
        .unwrap();
    (header, markets)
}

fn bucket(markets: &[Market<u64>], domain: usize) -> BackingBucketV16 {
    let slot = &markets[0].engine;
    if domain == 0 {
        slot.backing_long.try_to_runtime().unwrap()
    } else {
        slot.backing_short.try_to_runtime().unwrap()
    }
}

fn principal(markets: &[Market<u64>], domain: usize) -> u128 {
    let slot = &markets[0].engine;
    if domain == 0 {
        slot.provider_principal_long.get()
    } else {
        slot.provider_principal_short.get()
    }
}

fn reserved(markets: &[Market<u64>], domain: usize) -> u128 {
    let slot = &markets[0].engine;
    if domain == 0 {
        slot.source_credit_long.fresh_reserved_backing_num.get()
    } else {
        slot.source_credit_short.fresh_reserved_backing_num.get()
    }
}

fn deposit(f: &mut Fixture, domain: usize, amount: u128, expiry: u64) -> Result<(), V16Error> {
    MarketGroupV16ViewMut::new(&mut f.0, &mut f.1)
        .deposit_fresh_counterparty_backing_not_atomic(domain, amount, expiry)
}

fn earn(f: &mut Fixture, domain: usize, amount: u128) -> Result<(), V16Error> {
    MarketGroupV16ViewMut::new(&mut f.0, &mut f.1).credit_backing_provider_earnings_not_atomic(domain, amount)
}

fn expire(f: &mut Fixture, domain: usize, now: u64) -> Result<(), V16Error> {
    MarketGroupV16ViewMut::new(&mut f.0, &mut f.1).expire_source_backing_bucket_not_atomic(domain, now)
}

/// Earnings are senior to backing principal (`validate_header_aggregate_totals`), so crediting
/// them needs vault atoms beyond the deposits. In production those come from account fees; here
/// we just add them to the vault.
fn fund_vault(f: &mut Fixture, extra: u128) {
    f.0.vault = V16PodU128::new(f.0.vault.get() + extra);
}

fn set_slot(f: &mut Fixture, slot: u64) {
    f.0.current_slot = V16PodU64::new(slot);
}

// ---------------------------------------------------------------- deposit

#[test]
fn backing_deposit_zero_is_invalid_and_changes_nothing() {
    let mut f = fixture();
    assert_eq!(deposit(&mut f, 0, 0, 100), Err(V16Error::InvalidConfig));
    assert_eq!(f.0.vault.get(), 0);
    assert_eq!(bucket(&f.1, 0).status, BackingBucketStatusV16::Empty);
    assert_eq!(principal(&f.1, 0), 0);
}

#[test]
fn backing_deposit_out_of_range_domain_is_invalid_leg() {
    let mut f = fixture();
    // one asset => domains 0 and 1 only
    assert_eq!(deposit(&mut f, 2, 5, 100), Err(V16Error::InvalidLeg));
    assert_eq!(deposit(&mut f, usize::MAX, 5, 100), Err(V16Error::InvalidLeg));
    assert_eq!(f.0.vault.get(), 0);
}

#[test]
fn backing_deposit_one_atom_scales_by_bound_scale() {
    let mut f = fixture();
    deposit(&mut f, 0, 1, 100).unwrap();
    let b = bucket(&f.1, 0);
    assert_eq!(b.status, BackingBucketStatusV16::Fresh);
    assert_eq!(b.expiry_slot, 100);
    assert_eq!(b.fresh_unliened_backing_num, BOUND_SCALE);
    assert_eq!(principal(&f.1, 0), BOUND_SCALE);
    assert_eq!(reserved(&f.1, 0), BOUND_SCALE);
    assert_eq!(f.0.vault.get(), 1);
    assert_eq!(f.0.source_fresh_backing_total_num.get(), BOUND_SCALE);
}

#[test]
fn backing_deposit_max_amount_overflows_on_scaling_and_leaves_state_clean() {
    let mut f = fixture();
    let r = deposit(&mut f, 0, u128::MAX, 100);
    assert_eq!(r, Err(V16Error::ArithmeticOverflow));
    // largest amount whose scaled value still fits is MAX / BOUND_SCALE; one above must fail
    let too_big = u128::MAX / BOUND_SCALE + 1;
    assert_eq!(deposit(&mut f, 0, too_big, 100), Err(V16Error::ArithmeticOverflow));
    assert_eq!(f.0.vault.get(), 0);
    assert_eq!(bucket(&f.1, 0).status, BackingBucketStatusV16::Empty);
    assert_eq!(principal(&f.1, 0), 0);
}

#[test]
fn backing_deposit_expiry_must_be_strictly_after_current_slot() {
    let mut f = fixture();
    set_slot(&mut f, 50);
    assert_eq!(deposit(&mut f, 0, 3, 49), Err(V16Error::InvalidConfig));
    assert_eq!(deposit(&mut f, 0, 3, 50), Err(V16Error::InvalidConfig));
    assert_eq!(f.0.vault.get(), 0);
    deposit(&mut f, 0, 3, 51).unwrap();
    assert_eq!(bucket(&f.1, 0).expiry_slot, 51);
    assert_eq!(f.0.vault.get(), 3);
}

#[test]
fn backing_deposit_repeated_same_expiry_accumulates_exactly() {
    let mut f = fixture();
    let mut total = 0u128;
    for amt in [1u128, 2, 7, 1_000_000, 1] {
        deposit(&mut f, 0, amt, 100).unwrap();
        total += amt;
        assert_eq!(f.0.vault.get(), total);
        assert_eq!(bucket(&f.1, 0).fresh_unliened_backing_num, total * BOUND_SCALE);
        assert_eq!(principal(&f.1, 0), total * BOUND_SCALE);
        assert_eq!(reserved(&f.1, 0), total * BOUND_SCALE);
        assert_eq!(f.0.source_fresh_backing_total_num.get(), total * BOUND_SCALE);
    }
}

#[test]
fn backing_deposit_different_expiry_on_live_bucket_is_refused() {
    let mut f = fixture();
    deposit(&mut f, 0, 5, 100).unwrap();
    assert_eq!(deposit(&mut f, 0, 5, 101), Err(V16Error::LockActive));
    assert_eq!(deposit(&mut f, 0, 5, 99), Err(V16Error::LockActive));
    assert_eq!(f.0.vault.get(), 5);
    assert_eq!(bucket(&f.1, 0).expiry_slot, 100);
}

#[test]
fn backing_deposit_long_and_short_domains_are_independent() {
    let mut f = fixture();
    deposit(&mut f, 0, 4, 100).unwrap();
    deposit(&mut f, 1, 9, 200).unwrap();
    assert_eq!(bucket(&f.1, 0).expiry_slot, 100);
    assert_eq!(bucket(&f.1, 1).expiry_slot, 200);
    assert_eq!(principal(&f.1, 0), 4 * BOUND_SCALE);
    assert_eq!(principal(&f.1, 1), 9 * BOUND_SCALE);
    assert_eq!(f.0.vault.get(), 13);
    assert_eq!(f.0.source_fresh_backing_total_num.get(), 13 * BOUND_SCALE);
}

#[test]
fn backing_deposit_vault_overflow_is_refused() {
    let mut f = fixture();
    f.0.vault = V16PodU128::new(u128::MAX);
    let r = deposit(&mut f, 0, 1, 100);
    assert!(r.is_err(), "vault at u128::MAX must not accept a deposit, got {r:?}");
}

// ---------------------------------------------------------------- earnings

#[test]
fn backing_earnings_zero_amount_is_noop_even_without_bucket() {
    let mut f = fixture();
    assert_eq!(earn(&mut f, 0, 0), Ok(()));
    assert_eq!(f.0.backing_provider_earnings_total.get(), 0);
}

#[test]
fn backing_earnings_bad_domain_is_invalid_leg_even_for_zero() {
    let mut f = fixture();
    assert_eq!(earn(&mut f, 5, 0), Err(V16Error::InvalidLeg));
    assert_eq!(earn(&mut f, 5, 1), Err(V16Error::InvalidLeg));
}

#[test]
fn backing_earnings_need_a_fresh_bucket() {
    let mut f = fixture();
    assert_eq!(earn(&mut f, 0, 1), Err(V16Error::LockActive));
    assert_eq!(f.0.backing_provider_earnings_total.get(), 0);
}

#[test]
fn backing_earnings_refused_at_expiry_slot_allowed_one_before() {
    let mut f = fixture();
    deposit(&mut f, 0, 10, 100).unwrap();
    fund_vault(&mut f, 100);
    // vault holds 10 atoms; nothing senior yet
    set_slot(&mut f, 99);
    earn(&mut f, 0, 1).unwrap();
    set_slot(&mut f, 100);
    assert_eq!(earn(&mut f, 0, 1), Err(V16Error::LockActive));
    assert_eq!(bucket(&f.1, 0).utilization_fee_earnings, 1);
}

#[test]
fn backing_earnings_exact_boundary_against_vault_less_backing() {
    // deposit 10 (backing 10 atoms), plus 10 spare atoms: vault = 20, so earnings <= 10 fit.
    let mut f = fixture();
    deposit(&mut f, 0, 10, 100).unwrap();
    fund_vault(&mut f, 10);
    earn(&mut f, 0, 10).unwrap();
    assert_eq!(f.0.backing_provider_earnings_total.get(), 10);
    assert_eq!(bucket(&f.1, 0).utilization_fee_earnings, 10);
    // one more atom would put earnings + backing above the vault
    assert!(earn(&mut f, 0, 1).is_err());
}

#[test]
fn backing_earnings_above_whole_vault_is_lock_active() {
    // the delta's own check (earnings alone vs vault) fires before the shape check
    let mut f = fixture();
    deposit(&mut f, 0, 10, 100).unwrap();
    assert_eq!(earn(&mut f, 0, 11), Err(V16Error::LockActive));
    assert_eq!(f.0.backing_provider_earnings_total.get(), 0);
}

#[test]
fn backing_earnings_cannot_eat_backing_principal() {
    // vault = 10 = backing only; even 1 atom of earnings would double-promise the backing
    let mut f = fixture();
    deposit(&mut f, 0, 10, 100).unwrap();
    assert_eq!(earn(&mut f, 0, 1), Err(V16Error::InvalidConfig));
}

#[test]
fn backing_earnings_accumulate_and_do_not_move_vault_or_principal() {
    let mut f = fixture();
    deposit(&mut f, 0, 100, 100).unwrap();
    fund_vault(&mut f, 100);
    let vault = f.0.vault.get();
    let p = principal(&f.1, 0);
    for amt in [1u128, 2, 3, 4] {
        earn(&mut f, 0, amt).unwrap();
    }
    assert_eq!(bucket(&f.1, 0).utilization_fee_earnings, 10);
    assert_eq!(f.0.backing_provider_earnings_total.get(), 10);
    assert_eq!(f.0.vault.get(), vault);
    assert_eq!(principal(&f.1, 0), p);
    assert_eq!(bucket(&f.1, 0).fresh_unliened_backing_num, 100 * BOUND_SCALE);
}

#[test]
fn backing_earnings_max_amount_overflows_cleanly() {
    let mut f = fixture();
    deposit(&mut f, 0, 10, 100).unwrap();
    fund_vault(&mut f, 100);
    earn(&mut f, 0, 1).unwrap();
    let r = earn(&mut f, 0, u128::MAX);
    assert_eq!(r, Err(V16Error::CounterOverflow));
    assert_eq!(f.0.backing_provider_earnings_total.get(), 1);
    assert_eq!(bucket(&f.1, 0).utilization_fee_earnings, 1);
}

#[test]
fn backing_earnings_are_per_domain() {
    let mut f = fixture();
    deposit(&mut f, 0, 10, 100).unwrap();
    deposit(&mut f, 1, 10, 100).unwrap();
    fund_vault(&mut f, 100);
    earn(&mut f, 0, 3).unwrap();
    earn(&mut f, 1, 4).unwrap();
    assert_eq!(bucket(&f.1, 0).utilization_fee_earnings, 3);
    assert_eq!(bucket(&f.1, 1).utilization_fee_earnings, 4);
    assert_eq!(f.0.backing_provider_earnings_total.get(), 7);
}

// ---------------------------------------------------------------- expiry

#[test]
fn backing_expire_before_expiry_slot_is_stale_and_changes_nothing() {
    let mut f = fixture();
    deposit(&mut f, 0, 6, 100).unwrap();
    assert_eq!(expire(&mut f, 0, 99), Err(V16Error::Stale));
    assert_eq!(bucket(&f.1, 0).status, BackingBucketStatusV16::Fresh);
    assert_eq!(bucket(&f.1, 0).fresh_unliened_backing_num, 6 * BOUND_SCALE);
}

#[test]
fn backing_expire_on_empty_bucket_is_stale() {
    let mut f = fixture();
    assert_eq!(expire(&mut f, 0, 1_000), Err(V16Error::Stale));
}

#[test]
fn backing_expire_bad_domain_errors() {
    let mut f = fixture();
    assert!(expire(&mut f, 9, 1_000).is_err());
}

#[test]
fn backing_expire_at_exact_expiry_slot_clears_bucket_and_keeps_vault() {
    let mut f = fixture();
    deposit(&mut f, 0, 6, 100).unwrap();
    expire(&mut f, 0, 100).unwrap();
    let b = bucket(&f.1, 0);
    assert_eq!(b.status, BackingBucketStatusV16::Expired);
    assert_eq!(b.fresh_unliened_backing_num, 0);
    assert_eq!(b.valid_liened_backing_num, 0);
    assert_eq!(reserved(&f.1, 0), 0);
    assert_eq!(f.0.source_fresh_backing_total_num.get(), 0);
    // value is not destroyed: the atoms stay in the vault
    assert_eq!(f.0.vault.get(), 6);
    // the provider-principal mirror resets with the empty bucket
    assert_eq!(principal(&f.1, 0), 0);
}

#[test]
fn backing_expire_twice_second_call_is_stale() {
    let mut f = fixture();
    deposit(&mut f, 0, 6, 100).unwrap();
    expire(&mut f, 0, 100).unwrap();
    assert_eq!(expire(&mut f, 0, 200), Err(V16Error::Stale));
    assert_eq!(f.0.vault.get(), 6);
}

#[test]
fn backing_expire_only_touches_its_own_domain() {
    let mut f = fixture();
    deposit(&mut f, 0, 6, 100).unwrap();
    deposit(&mut f, 1, 8, 100).unwrap();
    expire(&mut f, 0, 100).unwrap();
    assert_eq!(bucket(&f.1, 1).status, BackingBucketStatusV16::Fresh);
    assert_eq!(bucket(&f.1, 1).fresh_unliened_backing_num, 8 * BOUND_SCALE);
    assert_eq!(principal(&f.1, 1), 8 * BOUND_SCALE);
    assert_eq!(f.0.source_fresh_backing_total_num.get(), 8 * BOUND_SCALE);
    assert_eq!(f.0.vault.get(), 14);
}

#[test]
fn backing_redeposit_after_expiry_starts_a_new_fresh_bucket() {
    let mut f = fixture();
    deposit(&mut f, 0, 6, 100).unwrap();
    expire(&mut f, 0, 100).unwrap();
    set_slot(&mut f, 100);
    // a different expiry is fine once the old bucket is Expired
    deposit(&mut f, 0, 2, 300).unwrap();
    let b = bucket(&f.1, 0);
    assert_eq!(b.status, BackingBucketStatusV16::Fresh);
    assert_eq!(b.expiry_slot, 300);
    assert_eq!(b.fresh_unliened_backing_num, 2 * BOUND_SCALE);
    assert_eq!(principal(&f.1, 0), 2 * BOUND_SCALE);
    assert_eq!(f.0.vault.get(), 8);
}

#[test]
fn backing_earnings_refused_after_expiry() {
    let mut f = fixture();
    deposit(&mut f, 0, 6, 100).unwrap();
    expire(&mut f, 0, 100).unwrap();
    set_slot(&mut f, 100);
    assert_eq!(earn(&mut f, 0, 1), Err(V16Error::LockActive));
}

#[test]
fn backing_expire_after_earnings_keeps_earnings_total_consistent() {
    let mut f = fixture();
    deposit(&mut f, 0, 6, 100).unwrap();
    fund_vault(&mut f, 100);
    earn(&mut f, 0, 2).unwrap();
    expire(&mut f, 0, 100).unwrap();
    // header total must equal the sum over buckets (validate_shape checks it)
    MarketGroupV16ViewMut::new(&mut f.0, &mut f.1).validate_shape().unwrap();
    assert_eq!(
        f.0.backing_provider_earnings_total.get(),
        bucket(&f.1, 0).utilization_fee_earnings
    );
}

// ---------------------------------------------------------------- conservation

#[test]
fn backing_vault_equals_sum_of_deposits_through_full_cycle() {
    let mut f = fixture();
    let mut sum = 0u128;
    for (d, amt, exp) in [(0usize, 5u128, 100u64), (1, 7, 150), (0, 11, 100), (1, 3, 150)] {
        deposit(&mut f, d, amt, exp).unwrap();
        sum += amt;
        assert_eq!(f.0.vault.get(), sum);
        assert_eq!(
            f.0.source_fresh_backing_total_num.get(),
            bucket(&f.1, 0).fresh_unliened_backing_num + bucket(&f.1, 1).fresh_unliened_backing_num
        );
        MarketGroupV16ViewMut::new(&mut f.0, &mut f.1).validate_shape().unwrap();
    }
    fund_vault(&mut f, 4);
    sum += 4;
    earn(&mut f, 0, 4).unwrap();
    expire(&mut f, 0, 100).unwrap();
    expire(&mut f, 1, 150).unwrap();
    assert_eq!(f.0.vault.get(), sum);
    assert_eq!(f.0.source_fresh_backing_total_num.get(), 0);
    MarketGroupV16ViewMut::new(&mut f.0, &mut f.1).validate_shape().unwrap();
}

// ---------------------------------------------------------------- adjust_slot_provider_principal

#[test]
fn backing_adjust_principal_add_zero_is_noop() {
    let mut slot = EngineAssetSlotV16Account::default();
    adjust_slot_provider_principal(&mut slot, false, 0, true).unwrap();
    adjust_slot_provider_principal(&mut slot, true, 0, false).unwrap();
    assert_eq!(slot.provider_principal_long.get(), 0);
    assert_eq!(slot.provider_principal_short.get(), 0);
}

#[test]
fn backing_adjust_principal_sides_are_independent() {
    let mut slot = EngineAssetSlotV16Account::default();
    adjust_slot_provider_principal(&mut slot, false, 5, true).unwrap();
    adjust_slot_provider_principal(&mut slot, true, 9, true).unwrap();
    adjust_slot_provider_principal(&mut slot, false, 2, false).unwrap();
    assert_eq!(slot.provider_principal_long.get(), 3);
    assert_eq!(slot.provider_principal_short.get(), 9);
}

#[test]
fn backing_adjust_principal_remove_saturates_at_zero() {
    let mut slot = EngineAssetSlotV16Account::default();
    adjust_slot_provider_principal(&mut slot, false, 5, true).unwrap();
    adjust_slot_provider_principal(&mut slot, false, 6, false).unwrap();
    assert_eq!(slot.provider_principal_long.get(), 0);
    adjust_slot_provider_principal(&mut slot, false, u128::MAX, false).unwrap();
    assert_eq!(slot.provider_principal_long.get(), 0);
}

#[test]
fn backing_adjust_principal_add_overflow_errors_and_keeps_value() {
    let mut slot = EngineAssetSlotV16Account::default();
    adjust_slot_provider_principal(&mut slot, false, u128::MAX, true).unwrap();
    assert_eq!(
        adjust_slot_provider_principal(&mut slot, false, 1, true),
        Err(V16Error::ArithmeticOverflow)
    );
    assert_eq!(slot.provider_principal_long.get(), u128::MAX);
    // exact removal back down works
    adjust_slot_provider_principal(&mut slot, false, u128::MAX, false).unwrap();
    assert_eq!(slot.provider_principal_long.get(), 0);
}
