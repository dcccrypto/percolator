//! Per-domain insurance budget edge tests (tests only, no product code).
//!
//! Covers credit_domain_insurance_budget(s)_not_atomic, domain_insurance_budget_remaining,
//! domain_insurance_withdraw_capacity, deposit_domain_insurance_not_atomic,
//! withdraw_domain_insurance_not_atomic and credit_account_from_insurance_not_atomic.
//! Domain index = 2 * asset_slot + side (0 long, 1 short).

use percolator::{
    v16_domain_count_for_market_slots, EngineAssetSlotV16Account, Market,
    MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, PortfolioAccountV16Account,
    PortfolioV16ViewMut, ProvenanceHeaderV16, ProvenanceHeaderV16Account, V16Config, V16Error,
    V16PodU128,
};

fn market_fixture(slots: u32) -> (MarketGroupV16HeaderAccount, Vec<Market<u64>>) {
    let cfg = V16Config::public_user_fund_with_market_slots(slots as u16, slots, 0, 10);
    let mut header =
        MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, slots, 0).unwrap();
    let mut markets = (0..slots)
        .map(|i| Market::new(i as u64, EngineAssetSlotV16Account::default()))
        .collect::<Vec<_>>();
    for i in 0..slots as usize {
        header
            .activate_empty_asset_slot_not_atomic(i as u32, &mut markets[i].engine, 100, (i + 1) as u64)
            .unwrap();
    }
    {
        let view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        view.validate_shape().unwrap();
    }
    (header, markets)
}

fn account_fixture(slots: u32, seed: u8) -> PortfolioAccountV16Account {
    let header =
        ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new([1; 32], [seed; 32], [3; 32]));
    let _ = v16_domain_count_for_market_slots(slots).unwrap();
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(header).unwrap();
    a
}

/// Total value the vault must cover: vault == insurance + c_tot (no trades in these tests).
fn assert_conserved(m: &MarketGroupV16ViewMut<'_, u64>) {
    assert_eq!(
        m.header.vault.get(),
        m.header.insurance.get() + m.header.c_tot.get(),
        "vault must equal insurance + capital"
    );
    assert!(
        m.header.insurance_domain_budget_remaining_total.get() <= m.header.insurance.get(),
        "sum of domain budgets must stay backed by insurance"
    );
    assert_eq!(m.validate_shape(), Ok(()));
}

// ---------------------------------------------------------------- deposit ---

#[test]
fn domain_insurance_deposit_zero_is_a_noop() {
    let (mut h, mut ms) = market_fixture(1);
    let mut m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
    m.deposit_domain_insurance_not_atomic(0, 0).unwrap();
    assert_eq!(m.header.vault.get(), 0);
    assert_eq!(m.header.insurance.get(), 0);
    assert_eq!(m.domain_insurance_budget_remaining(0), Ok(0));
    assert_conserved(&m);
}

#[test]
fn domain_insurance_deposit_credits_only_its_domain() {
    let (mut h, mut ms) = market_fixture(2);
    let mut m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
    m.deposit_domain_insurance_not_atomic(1, 40).unwrap();
    assert_eq!(m.domain_insurance_budget_remaining(1), Ok(40));
    for d in [0usize, 2, 3] {
        assert_eq!(m.domain_insurance_budget_remaining(d), Ok(0), "domain {d}");
    }
    assert_eq!(m.header.vault.get(), 40);
    assert_eq!(m.header.insurance.get(), 40);
    assert_eq!(m.header.insurance_domain_budget_remaining_total.get(), 40);
    assert_conserved(&m);
}

#[test]
fn domain_insurance_deposit_repeated_accumulates_exactly() {
    let (mut h, mut ms) = market_fixture(1);
    let mut m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
    let mut sum = 0u128;
    for amt in [1u128, 2, 3, 5, 8, 13] {
        m.deposit_domain_insurance_not_atomic(0, amt).unwrap();
        sum += amt;
        assert_eq!(m.domain_insurance_budget_remaining(0), Ok(sum));
        assert_eq!(m.header.vault.get(), sum);
        assert_conserved(&m);
    }
}

#[test]
fn domain_insurance_deposit_to_out_of_range_domain_is_rejected_and_changes_nothing() {
    let (mut h, mut ms) = market_fixture(1);
    let mut m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
    assert!(m.deposit_domain_insurance_not_atomic(2, 5).is_err());
    assert!(m.deposit_domain_insurance_not_atomic(usize::MAX, 5).is_err());
    assert_eq!(m.header.vault.get(), 0);
    assert_eq!(m.header.insurance.get(), 0);
    assert_conserved(&m);
}

#[test]
fn domain_insurance_deposit_u128_max_fails_without_state_change() {
    let (mut h, mut ms) = market_fixture(1);
    let mut m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
    m.deposit_domain_insurance_not_atomic(0, 10).unwrap();
    assert!(m.deposit_domain_insurance_not_atomic(0, u128::MAX).is_err());
    assert_eq!(m.header.vault.get(), 10);
    assert_eq!(m.header.insurance.get(), 10);
    assert_eq!(m.domain_insurance_budget_remaining(0), Ok(10));
    assert_conserved(&m);
}

#[test]
fn domain_insurance_deposit_past_vault_tvl_cap_is_rejected() {
    // The cap is enforced by validate_shape at the end of the call, after the writes
    // (the "_not_atomic" contract: the caller discards the market on Err). So we only
    // pin that the call fails and that the over-cap market is never reported valid.
    let (mut h, mut ms) = market_fixture(1);
    let mut m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
    m.deposit_domain_insurance_not_atomic(0, 10).unwrap();
    assert!(m
        .deposit_domain_insurance_not_atomic(0, percolator::MAX_VAULT_TVL)
        .is_err());
    assert!(m.validate_shape().is_err());
}

// exactly at the cap is allowed
#[test]
fn domain_insurance_deposit_exactly_to_vault_tvl_cap_is_accepted() {
    let (mut h, mut ms) = market_fixture(1);
    let mut m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
    m.deposit_domain_insurance_not_atomic(0, percolator::MAX_VAULT_TVL).unwrap();
    assert_eq!(m.header.vault.get(), percolator::MAX_VAULT_TVL);
    assert_eq!(
        m.domain_insurance_withdraw_capacity(0),
        Ok(percolator::MAX_VAULT_TVL)
    );
    assert_conserved(&m);
}

// ----------------------------------------------------------------- credit ---

#[test]
fn domain_insurance_credit_empty_list_is_rejected() {
    let (mut h, mut ms) = market_fixture(1);
    let mut m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
    assert_eq!(
        m.credit_domain_insurance_budgets_not_atomic(&[]),
        Err(V16Error::InvalidConfig)
    );
}

#[test]
fn domain_insurance_credit_zero_is_a_noop() {
    let (mut h, mut ms) = market_fixture(1);
    h.vault = V16PodU128::new(5);
    h.insurance = V16PodU128::new(5);
    let mut m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
    m.credit_domain_insurance_budget_not_atomic(0, 0).unwrap();
    assert_eq!(m.domain_insurance_budget_remaining(0), Ok(0));
    assert_eq!(m.header.insurance_domain_budget_remaining_total.get(), 0);
    assert_eq!(m.header.vault.get(), 5);
    assert_eq!(m.header.insurance.get(), 5);
}

#[test]
fn domain_insurance_credit_exactly_all_insurance_is_allowed_one_more_is_not() {
    let (mut h, mut ms) = market_fixture(1);
    h.vault = V16PodU128::new(10);
    h.insurance = V16PodU128::new(10);
    let mut m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
    m.credit_domain_insurance_budget_not_atomic(0, 10).unwrap();
    assert_eq!(m.domain_insurance_budget_remaining(0), Ok(10));
    assert_eq!(
        m.credit_domain_insurance_budget_not_atomic(1, 1),
        Err(V16Error::LockActive),
        "budgets may never exceed total insurance"
    );
    assert_eq!(m.domain_insurance_budget_remaining(1), Ok(0));
    assert_eq!(m.header.insurance_domain_budget_remaining_total.get(), 10);
}

#[test]
fn domain_insurance_credit_one_over_insurance_is_rejected_with_no_change() {
    let (mut h, mut ms) = market_fixture(1);
    h.vault = V16PodU128::new(10);
    h.insurance = V16PodU128::new(10);
    let mut m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
    assert_eq!(
        m.credit_domain_insurance_budget_not_atomic(0, 11),
        Err(V16Error::LockActive)
    );
    assert_eq!(m.domain_insurance_budget_remaining(0), Ok(0));
    assert_eq!(m.header.insurance_domain_budget_remaining_total.get(), 0);
}

#[test]
fn domain_insurance_credit_u128_max_reports_error_not_wrap() {
    let (mut h, mut ms) = market_fixture(1);
    h.vault = V16PodU128::new(10);
    h.insurance = V16PodU128::new(10);
    let mut m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
    m.credit_domain_insurance_budget_not_atomic(0, 4).unwrap();
    // 4 + MAX overflows u128: must be an error, never a wrapped small budget.
    assert!(m.credit_domain_insurance_budget_not_atomic(0, u128::MAX).is_err());
    assert_eq!(m.domain_insurance_budget_remaining(0), Ok(4));
    assert_eq!(m.header.insurance_domain_budget_remaining_total.get(), 4);
}

#[test]
fn domain_insurance_credit_leaves_vault_and_insurance_untouched() {
    let (mut h, mut ms) = market_fixture(2);
    h.vault = V16PodU128::new(30);
    h.insurance = V16PodU128::new(30);
    let mut m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
    m.credit_domain_insurance_budgets_not_atomic(&[(0, 10), (3, 20)]).unwrap();
    assert_eq!(m.header.vault.get(), 30);
    assert_eq!(m.header.insurance.get(), 30);
    assert_eq!(m.header.insurance_domain_budget_remaining_total.get(), 30);
    assert_conserved(&m);
}

#[test]
fn domain_insurance_credit_batch_order_does_not_change_the_result() {
    let run = |order: &[(usize, u128)]| {
        let (mut h, mut ms) = market_fixture(2);
        h.vault = V16PodU128::new(20);
        h.insurance = V16PodU128::new(20);
        let mut m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
        m.credit_domain_insurance_budgets_not_atomic(order).unwrap();
        (0..4).map(|d| m.domain_insurance_budget_remaining(d).unwrap()).collect::<Vec<_>>()
    };
    let a = run(&[(0, 3), (1, 4), (0, 5), (3, 8)]);
    let b = run(&[(3, 8), (0, 5), (1, 4), (0, 3)]);
    assert_eq!(a, b);
    assert_eq!(a, vec![8, 4, 0, 8]);
}

#[test]
fn domain_insurance_credit_batch_equals_sequence_of_single_credits() {
    let (mut h1, mut m1s) = market_fixture(2);
    h1.vault = V16PodU128::new(20);
    h1.insurance = V16PodU128::new(20);
    let (mut h2, mut m2s) = market_fixture(2);
    h2.vault = V16PodU128::new(20);
    h2.insurance = V16PodU128::new(20);
    let list = [(1usize, 2u128), (1, 3), (2, 7), (0, 1)];
    let mut a = MarketGroupV16ViewMut::new(&mut h1, &mut m1s);
    let mut b = MarketGroupV16ViewMut::new(&mut h2, &mut m2s);
    a.credit_domain_insurance_budgets_not_atomic(&list).unwrap();
    for &(d, x) in &list {
        b.credit_domain_insurance_budget_not_atomic(d, x).unwrap();
    }
    for d in 0..4 {
        assert_eq!(a.domain_insurance_budget_remaining(d), b.domain_insurance_budget_remaining(d));
    }
    assert_eq!(
        a.header.insurance_domain_budget_remaining_total.get(),
        b.header.insurance_domain_budget_remaining_total.get()
    );
}

#[test]
fn domain_insurance_credit_batch_over_insurance_in_aggregate_is_rejected() {
    // Each entry alone fits (6 <= 10) but the two together (12) do not.
    let (mut h, mut ms) = market_fixture(1);
    h.vault = V16PodU128::new(10);
    h.insurance = V16PodU128::new(10);
    let mut m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
    assert_eq!(
        m.credit_domain_insurance_budgets_not_atomic(&[(0, 6), (1, 6)]),
        Err(V16Error::LockActive)
    );
}

#[test]
fn domain_insurance_credit_to_out_of_range_domain_is_rejected() {
    let (mut h, mut ms) = market_fixture(1);
    h.vault = V16PodU128::new(10);
    h.insurance = V16PodU128::new(10);
    let mut m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
    assert!(m.credit_domain_insurance_budget_not_atomic(2, 1).is_err());
    assert_eq!(m.header.insurance_domain_budget_remaining_total.get(), 0);
}

// ------------------------------------------------------ remaining/capacity ---

#[test]
fn domain_insurance_remaining_and_capacity_on_fresh_domain_are_zero() {
    let (mut h, mut ms) = market_fixture(1);
    let m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
    assert_eq!(m.domain_insurance_budget_remaining(0), Ok(0));
    assert_eq!(m.domain_insurance_withdraw_capacity(0), Ok(0));
    assert!(m.domain_insurance_budget_remaining(2).is_err());
    assert!(m.domain_insurance_withdraw_capacity(2).is_err());
}

#[test]
fn domain_insurance_capacity_equals_budget_when_budget_insurance_and_vault_agree() {
    let (mut h, mut ms) = market_fixture(1);
    let mut m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
    m.deposit_domain_insurance_not_atomic(0, 10).unwrap();
    assert_eq!(m.domain_insurance_withdraw_capacity(0), Ok(10));
    // an untouched sibling domain has no capacity
    assert_eq!(m.domain_insurance_withdraw_capacity(1), Ok(0));
}

#[test]
fn domain_insurance_capacity_ignores_unbudgeted_surplus() {
    // Domain 0 has budget 4 backed by 10 insurance. The 6 unbudgeted atoms add no capacity to any domain.
    let (mut h, mut ms) = market_fixture(1);
    h.vault = V16PodU128::new(10);
    h.insurance = V16PodU128::new(10);
    let mut m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
    m.credit_domain_insurance_budget_not_atomic(0, 4).unwrap();
    assert_eq!(m.domain_insurance_withdraw_capacity(0), Ok(4));
    assert_eq!(m.domain_insurance_withdraw_capacity(1), Ok(0));
}

// --------------------------------------------------------------- withdraw ---

#[test]
fn domain_insurance_withdraw_exactly_capacity_works_one_more_fails() {
    let (mut h, mut ms) = market_fixture(1);
    let mut m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
    m.deposit_domain_insurance_not_atomic(0, 10).unwrap();
    let cap = m.domain_insurance_withdraw_capacity(0).unwrap();
    assert_eq!(
        m.withdraw_domain_insurance_not_atomic(0, cap + 1),
        Err(V16Error::LockActive)
    );
    m.withdraw_domain_insurance_not_atomic(0, cap).unwrap();
    assert_eq!(m.header.vault.get(), 0);
    assert_eq!(m.header.insurance.get(), 0);
    assert_eq!(m.domain_insurance_budget_remaining(0), Ok(0));
    assert_eq!(m.domain_insurance_withdraw_capacity(0), Ok(0));
    assert_conserved(&m);
}

#[test]
fn domain_insurance_withdraw_zero_changes_nothing() {
    let (mut h, mut ms) = market_fixture(1);
    let mut m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
    m.deposit_domain_insurance_not_atomic(0, 10).unwrap();
    m.withdraw_domain_insurance_not_atomic(0, 0).unwrap();
    assert_eq!(m.header.vault.get(), 10);
    assert_eq!(m.domain_insurance_budget_remaining(0), Ok(10));
    assert_conserved(&m);
}

#[test]
fn domain_insurance_withdraw_cannot_dip_into_another_domains_budget() {
    let (mut h, mut ms) = market_fixture(1);
    let mut m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
    m.deposit_domain_insurance_not_atomic(0, 10).unwrap();
    m.deposit_domain_insurance_not_atomic(1, 5).unwrap();
    assert_eq!(
        m.withdraw_domain_insurance_not_atomic(1, 6),
        Err(V16Error::LockActive)
    );
    m.withdraw_domain_insurance_not_atomic(1, 5).unwrap();
    assert_eq!(m.domain_insurance_budget_remaining(0), Ok(10));
    assert_eq!(m.header.insurance.get(), 10);
    assert_conserved(&m);
}

#[test]
fn domain_insurance_deposit_withdraw_repeated_round_trips_return_to_zero() {
    let (mut h, mut ms) = market_fixture(1);
    let mut m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
    for amt in [1u128, 7, 1_000_003, 2] {
        m.deposit_domain_insurance_not_atomic(0, amt).unwrap();
        m.withdraw_domain_insurance_not_atomic(0, amt).unwrap();
        assert_eq!(m.header.vault.get(), 0);
        assert_eq!(m.header.insurance.get(), 0);
        assert_eq!(m.domain_insurance_budget_remaining(0), Ok(0));
        assert_conserved(&m);
    }
}

// -------------------------------------------- credit account from insurance ---

#[test]
fn domain_insurance_account_credit_zero_is_a_noop_even_with_no_insurance() {
    let (mut h, mut ms) = market_fixture(1);
    let mut ah = account_fixture(1, 9);
    let mut m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
    let mut a = PortfolioV16ViewMut::new(&mut ah);
    m.credit_account_from_insurance_not_atomic(&mut a, 0, 0).unwrap();
    assert_eq!(a.header.capital.get(), 0);
    assert_eq!(m.header.c_tot.get(), 0);
}

#[test]
fn domain_insurance_account_credit_with_no_insurance_underflows_cleanly() {
    let (mut h, mut ms) = market_fixture(1);
    let mut ah = account_fixture(1, 9);
    let mut m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
    let mut a = PortfolioV16ViewMut::new(&mut ah);
    assert_eq!(
        m.credit_account_from_insurance_not_atomic(&mut a, 1, 0),
        Err(V16Error::CounterUnderflow)
    );
    assert_eq!(a.header.capital.get(), 0);
    assert_eq!(m.header.c_tot.get(), 0);
}

#[test]
fn domain_insurance_account_credit_exactly_unbudgeted_surplus_then_one_more_fails() {
    // insurance 10, budget 4 -> 6 free to pay out, the 7th atom is reserved.
    let (mut h, mut ms) = market_fixture(1);
    h.vault = V16PodU128::new(10);
    h.insurance = V16PodU128::new(10);
    let mut ah = account_fixture(1, 9);
    let mut m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
    let mut a = PortfolioV16ViewMut::new(&mut ah);
    m.credit_domain_insurance_budget_not_atomic(0, 4).unwrap();
    assert_eq!(
        m.credit_account_from_insurance_not_atomic(&mut a, 7, 0),
        Err(V16Error::LockActive)
    );
    assert_eq!(m.header.insurance.get(), 10);
    m.credit_account_from_insurance_not_atomic(&mut a, 6, 0).unwrap();
    assert_eq!(a.header.capital.get(), 6);
    assert_eq!(m.header.c_tot.get(), 6);
    assert_eq!(m.header.insurance.get(), 4);
    assert_eq!(m.header.vault.get(), 10, "vault is unchanged: value only moves insurance -> capital");
    assert_eq!(m.domain_insurance_budget_remaining(0), Ok(4), "budget untouched by reward credit");
    assert_conserved(&m);
    assert_eq!(
        m.credit_account_from_insurance_not_atomic(&mut a, 1, 0),
        Err(V16Error::LockActive)
    );
}

#[test]
fn domain_insurance_account_credit_repeated_small_credits_equal_one_big_credit() {
    let (mut h, mut ms) = market_fixture(1);
    h.vault = V16PodU128::new(100);
    h.insurance = V16PodU128::new(100);
    let mut ah = account_fixture(1, 9);
    let mut m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
    let mut a = PortfolioV16ViewMut::new(&mut ah);
    for _ in 0..100 {
        m.credit_account_from_insurance_not_atomic(&mut a, 1, 0).unwrap();
    }
    assert_eq!(a.header.capital.get(), 100);
    assert_eq!(m.header.insurance.get(), 0);
    assert_eq!(m.header.c_tot.get(), 100);
    assert_conserved(&m);
}

#[test]
fn domain_insurance_account_credit_u128_max_is_rejected() {
    let (mut h, mut ms) = market_fixture(1);
    h.vault = V16PodU128::new(10);
    h.insurance = V16PodU128::new(10);
    let mut ah = account_fixture(1, 9);
    let mut m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
    let mut a = PortfolioV16ViewMut::new(&mut ah);
    assert!(m.credit_account_from_insurance_not_atomic(&mut a, u128::MAX, 0).is_err());
    assert_eq!(m.header.insurance.get(), 10);
    assert_eq!(a.header.capital.get(), 0);
}

#[test]
fn domain_insurance_account_credit_additional_reserved_boundary() {
    let (mut h, mut ms) = market_fixture(1);
    h.vault = V16PodU128::new(10);
    h.insurance = V16PodU128::new(10);
    let mut ah = account_fixture(1, 9);
    let mut m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
    let mut a = PortfolioV16ViewMut::new(&mut ah);
    // reserved u128::MAX saturates; nothing can be paid out.
    assert_eq!(
        m.credit_account_from_insurance_not_atomic(&mut a, 1, u128::MAX),
        Err(V16Error::LockActive)
    );
    // reserved == insurance: zero free.
    assert_eq!(
        m.credit_account_from_insurance_not_atomic(&mut a, 1, 10),
        Err(V16Error::LockActive)
    );
    // reserved 9: exactly 1 free.
    m.credit_account_from_insurance_not_atomic(&mut a, 1, 9).unwrap();
    assert_eq!(m.header.insurance.get(), 9);
}

#[test]
fn domain_insurance_account_credit_cannot_touch_deposited_budget_so_full_withdraw_succeeds() {
    // Deposit 10 to domain 0 (budget 10). Reward credit cannot touch it (all reserved),
    // so the depositor can always take it back in full.
    let (mut h, mut ms) = market_fixture(1);
    let mut ah = account_fixture(1, 9);
    let mut m = MarketGroupV16ViewMut::new(&mut h, &mut ms);
    let mut a = PortfolioV16ViewMut::new(&mut ah);
    m.deposit_domain_insurance_not_atomic(0, 10).unwrap();
    assert_eq!(
        m.credit_account_from_insurance_not_atomic(&mut a, 1, 0),
        Err(V16Error::LockActive)
    );
    m.withdraw_domain_insurance_not_atomic(0, 10).unwrap();
    assert_conserved(&m);
}
