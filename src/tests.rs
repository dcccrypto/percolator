use super::*;

const E6: u64 = 1_000_000;
const ORACLE_100K: u64 = 100_000 * E6;
const ONE_BASE: i128 = 1_000_000; // 1.0 base unit if base is 1e6-scaled

fn params_for_tests() -> RiskParams {
    RiskParams {
        warmup_period_slots: 1000,
        maintenance_margin_bps: 0,
        initial_margin_bps: 0,
        trading_fee_bps: 0,
        max_accounts: MAX_ACCOUNTS as u64,
        new_account_fee: U128::new(0),
        risk_reduction_threshold: U128::new(0),

        maintenance_fee_per_slot: U128::new(0),
        max_crank_staleness_slots: u64::MAX,

        liquidation_fee_bps: 0,
        liquidation_fee_cap: U128::new(0),

        liquidation_buffer_bps: 0,
        min_liquidation_abs: U128::new(0),
    }
}

struct PriceBelowOracleMatcher;
impl MatchingEngine for PriceBelowOracleMatcher {
    fn execute_match(
        &self,
        _lp_program: &[u8; 32],
        _lp_context: &[u8; 32],
        _lp_account_id: u64,
        oracle_price: u64,
        size: i128,
    ) -> Result<TradeExecution> {
        // Execute $1k below oracle
        let exec_price = oracle_price - (1_000 * E6);
        Ok(TradeExecution { price: exec_price, size })
    }
}

struct OppositeSignMatcher;
impl MatchingEngine for OppositeSignMatcher {
    fn execute_match(
        &self,
        _lp_program: &[u8; 32],
        _lp_context: &[u8; 32],
        _lp_account_id: u64,
        oracle_price: u64,
        size: i128,
    ) -> Result<TradeExecution> {
        Ok(TradeExecution {
            price: oracle_price,
            size: -size,
        })
    }
}

struct OversizeFillMatcher;
impl MatchingEngine for OversizeFillMatcher {
    fn execute_match(
        &self,
        _lp_program: &[u8; 32],
        _lp_context: &[u8; 32],
        _lp_account_id: u64,
        oracle_price: u64,
        size: i128,
    ) -> Result<TradeExecution> {
        Ok(TradeExecution {
            price: oracle_price,
            size: size.checked_mul(2).unwrap(),
        })
    }
}

#[test]
fn test_execute_trade_sets_current_slot_and_resets_warmup_start() {
    let mut engine = RiskEngine::new(params_for_tests());

    let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();
    let user_idx = engine.add_user(0).unwrap();

    // Fund both so margin checks pass (maint=0 still requires equity > 0)
    engine.deposit(lp_idx, 1_000_000_000_000, 1).unwrap();
    engine.deposit(user_idx, 1_000_000_000_000, 1).unwrap();

    let matcher = PriceBelowOracleMatcher;

    // Trade at now_slot = 100
    engine
        .execute_trade(
            &matcher,
            lp_idx,
            user_idx,
            100,
            ORACLE_100K,
            ONE_BASE,
        )
        .unwrap();

    assert_eq!(engine.current_slot, 100);
    assert_eq!(
        engine.accounts[user_idx as usize].warmup_started_at_slot,
        100
    );
    assert_eq!(engine.accounts[lp_idx as usize].warmup_started_at_slot, 100);
}

#[test]
fn test_execute_trade_rejects_matcher_opposite_sign() {
    let mut engine = RiskEngine::new(params_for_tests());

    let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();
    let user_idx = engine.add_user(0).unwrap();

    engine.deposit(lp_idx, 1_000_000_000_000, 1).unwrap();
    engine.deposit(user_idx, 1_000_000_000_000, 1).unwrap();

    let matcher = OppositeSignMatcher;

    let res = engine.execute_trade(
        &matcher,
        lp_idx,
        user_idx,
        10,
        ORACLE_100K,
        ONE_BASE,
    );

    assert_eq!(res, Err(RiskError::InvalidMatchingEngine));
}

#[test]
fn test_execute_trade_rejects_matcher_oversize_fill() {
    let mut engine = RiskEngine::new(params_for_tests());

    let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();
    let user_idx = engine.add_user(0).unwrap();

    engine.deposit(lp_idx, 1_000_000_000_000, 1).unwrap();
    engine.deposit(user_idx, 1_000_000_000_000, 1).unwrap();

    let matcher = OversizeFillMatcher;

    let res = engine.execute_trade(
        &matcher,
        lp_idx,
        user_idx,
        10,
        ORACLE_100K,
        ONE_BASE,
    );

    assert_eq!(res, Err(RiskError::InvalidMatchingEngine));
}

#[test]
fn test_check_conservation_fails_on_mark_overflow() {
    let mut engine = RiskEngine::new(params_for_tests());
    let user_idx = engine.add_user(0).unwrap();

    // Corrupt the account to force mark_pnl overflow inside check_conservation
    engine.accounts[user_idx as usize].position_size = I128::new(i128::MAX);
    engine.accounts[user_idx as usize].entry_price = MAX_ORACLE_PRICE;
    engine.accounts[user_idx as usize].pnl = I128::ZERO;
    engine.accounts[user_idx as usize].capital = U128::ZERO;

    engine.vault = U128::ZERO;
    engine.insurance_fund.balance = U128::ZERO;
    engine.loss_accum = U128::ZERO;

    assert!(!engine.check_conservation(1));
}

#[test]
fn test_cross_lp_close_no_pnl_teleport_simple() {
    let mut engine = RiskEngine::new(params_for_tests());

    let lp1 = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();
    let lp2 = engine.add_lp([3u8; 32], [4u8; 32], 0).unwrap();
    let user = engine.add_user(0).unwrap();

    // LP1 must be able to absorb -10k*E6 loss and still have equity > 0
    engine.deposit(lp1, 50_000 * (E6 as u128), 1).unwrap();
    engine.deposit(lp2, 50_000 * (E6 as u128), 1).unwrap();
    engine.deposit(user, 50_000 * (E6 as u128), 1).unwrap();

    // Trade 1: user opens +1 at 90k while oracle=100k => user +10k, LP1 -10k
    struct P90kMatcher;
    impl MatchingEngine for P90kMatcher {
        fn execute_match(
            &self,
            _lp_program: &[u8; 32],
            _lp_context: &[u8; 32],
            _lp_account_id: u64,
            oracle_price: u64,
            size: i128,
        ) -> Result<TradeExecution> {
            Ok(TradeExecution {
                price: oracle_price - (10_000 * E6),
                size,
            })
        }
    }

    // Trade 2: user closes with LP2 at oracle price => trade_pnl = 0 (no teleport)
    struct AtOracleMatcher;
    impl MatchingEngine for AtOracleMatcher {
        fn execute_match(
            &self,
            _lp_program: &[u8; 32],
            _lp_context: &[u8; 32],
            _lp_account_id: u64,
            oracle_price: u64,
            size: i128,
        ) -> Result<TradeExecution> {
            Ok(TradeExecution {
                price: oracle_price,
                size,
            })
        }
    }

    engine
        .execute_trade(&P90kMatcher, lp1, user, 100, ORACLE_100K, ONE_BASE)
        .unwrap();
    engine
        .execute_trade(&AtOracleMatcher, lp2, user, 101, ORACLE_100K, -ONE_BASE)
        .unwrap();

    // User is flat
    assert_eq!(engine.accounts[user as usize].position_size.get(), 0);

    // PnL stays with LP1 (the LP that gave the user a better-than-oracle fill).
    // settle_warmup_to_capital immediately settles negative PnL against capital,
    // so LP1's pnl field is 0 and capital is reduced by 10k*E6.
    // Some of the user's PnL may have partially settled to capital via warmup
    // during trade 2 (correct behavior: settle matured warmup before slope reset).
    let ten_k_e6: u128 = (10_000 * E6) as u128;
    let user_pnl = engine.accounts[user as usize].pnl.get() as u128;
    let user_cap = engine.accounts[user as usize].capital.get();
    let initial_cap = 50_000 * (E6 as u128);
    // Total user value (pnl + capital) must equal initial_capital + 10k profit
    assert_eq!(user_pnl + user_cap, initial_cap + ten_k_e6,
        "user total value must be initial_capital + trade profit");
    assert_eq!(engine.accounts[lp1 as usize].pnl.get(), 0);
    assert_eq!(engine.accounts[lp1 as usize].capital.get(), initial_cap - ten_k_e6);
    // LP2 must be unaffected (no teleportation)
    assert_eq!(engine.accounts[lp2 as usize].pnl.get(), 0);
    assert_eq!(engine.accounts[lp2 as usize].capital.get(), initial_cap);

    // Conservation must still hold
    assert!(engine.check_conservation(ORACLE_100K));
}

#[test]
fn test_idle_user_drains_and_gc_closes() {
    let mut params = params_for_tests();
    // 1 unit per slot maintenance fee
    params.maintenance_fee_per_slot = U128::new(1);
    let mut engine = RiskEngine::new(params);

    let user_idx = engine.add_user(0).unwrap();
    // Deposit 10 units of capital
    engine.deposit(user_idx, 10, 1).unwrap();

    assert!(engine.is_used(user_idx as usize));

    // Advance 1000 slots and crank — fee drains 1/slot * 1000 = 1000 >> 10 capital
    let outcome = engine
        .keeper_crank(user_idx, 1001, ORACLE_100K, 0, false)
        .unwrap();

    // Account should have been drained to 0 capital
    // The crank settles fees and then GC sweeps dust
    assert_eq!(outcome.num_gc_closed, 1, "expected GC to close the drained account");
    assert!(!engine.is_used(user_idx as usize), "account should be freed");
}

#[test]
fn test_dust_stale_funding_gc() {
    let mut engine = RiskEngine::new(params_for_tests());

    let user_idx = engine.add_user(0).unwrap();

    // Zero out the account: no capital, no position, no pnl
    engine.accounts[user_idx as usize].capital = U128::ZERO;
    engine.accounts[user_idx as usize].pnl = I128::ZERO;
    engine.accounts[user_idx as usize].position_size = I128::ZERO;
    engine.accounts[user_idx as usize].reserved_pnl = 0;

    // Set a stale funding_index (different from global)
    engine.accounts[user_idx as usize].funding_index = I128::new(999);
    // Global funding index is 0 (default)
    assert_ne!(
        engine.accounts[user_idx as usize].funding_index,
        engine.funding_index_qpb_e6
    );

    assert!(engine.is_used(user_idx as usize));

    // Crank should snap funding and GC the dust account
    let outcome = engine
        .keeper_crank(user_idx, 10, ORACLE_100K, 0, false)
        .unwrap();

    assert_eq!(outcome.num_gc_closed, 1, "expected GC to close stale-funding dust");
    assert!(!engine.is_used(user_idx as usize), "account should be freed");
}

#[test]
fn test_dust_negative_fee_credits_gc() {
    let mut engine = RiskEngine::new(params_for_tests());

    let user_idx = engine.add_user(0).unwrap();

    // Zero out the account
    engine.accounts[user_idx as usize].capital = U128::ZERO;
    engine.accounts[user_idx as usize].pnl = I128::ZERO;
    engine.accounts[user_idx as usize].position_size = I128::ZERO;
    engine.accounts[user_idx as usize].reserved_pnl = 0;
    // Set negative fee_credits (fee debt)
    engine.accounts[user_idx as usize].fee_credits = I128::new(-123);

    assert!(engine.is_used(user_idx as usize));

    // Crank should GC this account — negative fee_credits doesn't block GC
    let outcome = engine
        .keeper_crank(user_idx, 10, ORACLE_100K, 0, false)
        .unwrap();

    assert_eq!(outcome.num_gc_closed, 1, "expected GC to close account with negative fee_credits");
    assert!(!engine.is_used(user_idx as usize), "account should be freed");
}

#[test]
fn test_lp_never_gc() {
    let mut params = params_for_tests();
    params.maintenance_fee_per_slot = U128::new(1);
    let mut engine = RiskEngine::new(params);

    let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();

    // Zero out the LP account to make it look like dust
    engine.accounts[lp_idx as usize].capital = U128::ZERO;
    engine.accounts[lp_idx as usize].pnl = I128::ZERO;
    engine.accounts[lp_idx as usize].position_size = I128::ZERO;
    engine.accounts[lp_idx as usize].reserved_pnl = 0;

    assert!(engine.is_used(lp_idx as usize));

    // Crank many times — LP should never be GC'd
    for slot in 1..=10 {
        let outcome = engine
            .keeper_crank(lp_idx, slot * 100, ORACLE_100K, 0, false)
            .unwrap();
        assert_eq!(outcome.num_gc_closed, 0, "LP must not be garbage collected (slot {})", slot * 100);
    }

    assert!(engine.is_used(lp_idx as usize), "LP account must still exist");
}

#[test]
fn test_maintenance_fee_paid_from_fee_credits_is_coupon_not_revenue() {
    let mut params = params_for_tests();
    params.maintenance_fee_per_slot = U128::new(10);
    let mut engine = RiskEngine::new(params);

    let user_idx = engine.add_user(0).unwrap();
    engine.deposit(user_idx, 1_000_000, 1).unwrap();

    // Add 100 fee credits (test-only helper — no vault/insurance)
    engine.add_fee_credits(user_idx, 100).unwrap();
    assert_eq!(engine.accounts[user_idx as usize].fee_credits.get(), 100);

    let rev_before = engine.insurance_fund.fee_revenue.get();
    let bal_before = engine.insurance_fund.balance.get();

    // Settle maintenance: dt=5, fee_per_slot=10, due=50
    // All 50 should come from fee_credits (coupon: no insurance booking)
    engine
        .settle_maintenance_fee(user_idx, 6, ORACLE_100K)
        .unwrap();

    assert_eq!(
        engine.accounts[user_idx as usize].fee_credits.get(),
        50,
        "fee_credits should decrease by 50"
    );
    // Coupon semantics: spending credits does NOT touch insurance.
    // Insurance was already paid when credits were granted.
    assert_eq!(
        engine.insurance_fund.fee_revenue.get() - rev_before,
        0,
        "insurance fee_revenue must NOT change (coupon semantics)"
    );
    assert_eq!(
        engine.insurance_fund.balance.get() - bal_before,
        0,
        "insurance balance must NOT change (coupon semantics)"
    );
}

#[test]
fn test_maintenance_fee_splits_credits_coupon_capital_to_insurance() {
    let mut params = params_for_tests();
    params.maintenance_fee_per_slot = U128::new(10);
    let mut engine = RiskEngine::new(params);

    let user_idx = engine.add_user(0).unwrap();
    // deposit at slot 1: dt=1 from slot 0, fee=10. Paid from deposit.
    // capital = 50 - 10 = 40.
    engine.deposit(user_idx, 50, 1).unwrap();
    assert_eq!(engine.accounts[user_idx as usize].capital.get(), 40);

    // Add 30 fee credits (test-only)
    engine.add_fee_credits(user_idx, 30).unwrap();

    let rev_before = engine.insurance_fund.fee_revenue.get();

    // Settle maintenance: dt=10, fee_per_slot=10, due=100
    // credits pays 30, capital pays 40 (all it has), leftover 30 unpaid
    engine
        .settle_maintenance_fee(user_idx, 11, ORACLE_100K)
        .unwrap();

    let rev_increase = engine.insurance_fund.fee_revenue.get() - rev_before;
    let cap_after = engine.accounts[user_idx as usize].capital.get();

    assert_eq!(rev_increase, 40, "insurance revenue should be 40 (capital only; credits are coupon)");
    assert_eq!(cap_after, 0, "capital should be fully drained");
    // fee_credits should be -30 (100 due - 30 credits - 40 capital = 30 unpaid debt)
    assert_eq!(
        engine.accounts[user_idx as usize].fee_credits.get(),
        -30,
        "fee_credits should reflect unpaid debt"
    );
}

#[test]
fn test_deposit_fee_credits_updates_vault_and_insurance() {
    let mut engine = RiskEngine::new(params_for_tests());
    let user_idx = engine.add_user(0).unwrap();

    let vault_before = engine.vault.get();
    let ins_before = engine.insurance_fund.balance.get();
    let rev_before = engine.insurance_fund.fee_revenue.get();

    engine.deposit_fee_credits(user_idx, 500, 10).unwrap();

    assert_eq!(engine.vault.get() - vault_before, 500, "vault must increase");
    assert_eq!(engine.insurance_fund.balance.get() - ins_before, 500, "insurance balance must increase");
    assert_eq!(engine.insurance_fund.fee_revenue.get() - rev_before, 500, "insurance fee_revenue must increase");
    assert_eq!(engine.accounts[user_idx as usize].fee_credits.get(), 500, "fee_credits must increase");
}

#[test]
fn test_warmup_matured_not_lost_on_trade() {
    let mut params = params_for_tests();
    params.warmup_period_slots = 100;
    params.max_crank_staleness_slots = u64::MAX;
    let mut engine = RiskEngine::new(params);

    let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();
    let user_idx = engine.add_user(0).unwrap();

    // Fund both generously
    engine.deposit(lp_idx, 1_000_000_000, 1).unwrap();
    engine.deposit(user_idx, 1_000_000_000, 1).unwrap();

    // Provide warmup budget: the warmup budget system requires losses or
    // spendable insurance to fund positive PnL settlement. Seed insurance
    // so the warmup budget allows settlement.
    engine.insurance_fund.balance = engine.insurance_fund.balance + 1_000_000;

    // Give user positive PnL and set warmup started far in the past
    engine.accounts[user_idx as usize].pnl = I128::new(10_000);
    engine.accounts[user_idx as usize].warmup_started_at_slot = 1;
    // slope = max(1, 10000/100) = 100
    engine.accounts[user_idx as usize].warmup_slope_per_step = U128::new(100);

    let cap_before = engine.accounts[user_idx as usize].capital.get();

    // Execute a tiny trade at slot 200 (elapsed from slot 1 = 199 slots, cap = 100*199 = 19900 > 10000)
    struct AtOracleMatcher;
    impl MatchingEngine for AtOracleMatcher {
        fn execute_match(
            &self,
            _lp_program: &[u8; 32],
            _lp_context: &[u8; 32],
            _lp_account_id: u64,
            oracle_price: u64,
            size: i128,
        ) -> Result<TradeExecution> {
            Ok(TradeExecution { price: oracle_price, size })
        }
    }

    engine
        .execute_trade(&AtOracleMatcher, lp_idx, user_idx, 200, ORACLE_100K, ONE_BASE)
        .unwrap();

    let cap_after = engine.accounts[user_idx as usize].capital.get();

    // Capital must have increased by the matured warmup amount (10_000 PnL settled to capital)
    assert!(
        cap_after > cap_before,
        "capital must increase from matured warmup: before={}, after={}",
        cap_before,
        cap_after
    );
    assert!(
        cap_after >= cap_before + 10_000,
        "capital should have increased by at least 10000 (matured warmup): before={}, after={}",
        cap_before,
        cap_after
    );
}

#[test]
fn test_abandoned_with_stale_last_fee_slot_eventually_closed() {
    let mut params = params_for_tests();
    params.maintenance_fee_per_slot = U128::new(1);
    let mut engine = RiskEngine::new(params);

    let user_idx = engine.add_user(0).unwrap();
    // Small deposit
    engine.deposit(user_idx, 5, 1).unwrap();

    assert!(engine.is_used(user_idx as usize));

    // Don't call any user ops. Run crank at a slot far ahead.
    // First crank: drains the account via fee settlement
    let _ = engine
        .keeper_crank(user_idx, 10_000, ORACLE_100K, 0, false)
        .unwrap();

    // Second crank: GC scan should pick up the dust
    let outcome = engine
        .keeper_crank(user_idx, 10_001, ORACLE_100K, 0, false)
        .unwrap();

    // The account must be closed by now (across both cranks)
    assert!(
        !engine.is_used(user_idx as usize),
        "abandoned account with stale last_fee_slot must eventually be GC'd"
    );
    // At least one of the two cranks should have GC'd it
    // (first crank drains capital to 0, GC might close it there already)
}
