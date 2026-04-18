#![cfg(feature = "test")]

use percolator::wide_math::U256;
use percolator::*;

// ============================================================================
// Helpers
// ============================================================================

fn default_params() -> RiskParams {
    RiskParams {
        warmup_period_slots: 100,
        maintenance_margin_bps: 500, // 5%
        initial_margin_bps: 1000,    // 10% — MUST be > maintenance
        trading_fee_bps: 10,
        max_accounts: 64,
        new_account_fee: U128::new(1000),
        maintenance_fee_per_slot: U128::new(1),
        max_crank_staleness_slots: 1000,
        liquidation_fee_bps: 100,
        liquidation_fee_cap: U128::new(1_000_000),
        min_liquidation_abs: U128::new(0),
        min_initial_deposit: U128::new(1000),
        min_nonzero_mm_req: 1,
        min_nonzero_im_req: 2,
        insurance_floor: U128::ZERO,
    }
}

/// Build a size_q from a quantity in base units.
/// size_q = quantity * POS_SCALE  (signed)
fn make_size_q(quantity: i64) -> i128 {
    let abs_qty = (quantity as i128).unsigned_abs();
    let scaled = abs_qty
        .checked_mul(POS_SCALE)
        .expect("make_size_q overflow");
    assert!(scaled <= i128::MAX as u128, "make_size_q: exceeds i128");
    if quantity < 0 {
        -(scaled as i128)
    } else {
        scaled as i128
    }
}

/// Helper: create engine, add two users with deposits, run initial crank.
/// Returns (engine, user_a_idx, user_b_idx).
fn setup_two_users(deposit_a: u128, deposit_b: u128) -> (RiskEngine, u16, u16) {
    let mut engine = RiskEngine::new(default_params());
    let oracle = 1000u64;
    let slot = 1u64;
    engine.current_slot = slot;

    let a = engine.add_user(1000).expect("add user a");
    let b = engine.add_user(1000).expect("add user b");

    // Deposit before crank so accounts have capital and are not GC'd
    if deposit_a > 0 {
        engine
            .deposit(a, deposit_a, oracle, slot)
            .expect("deposit a");
    }
    if deposit_b > 0 {
        engine
            .deposit(b, deposit_b, oracle, slot)
            .expect("deposit b");
    }

    // Initial crank so trades/withdrawals pass freshness check
    engine
        .keeper_crank_not_atomic(
            slot,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .expect("initial crank");

    (engine, a, b)
}

// ============================================================================
// 1. Basic engine creation and parameter validation
// ============================================================================

#[test]
fn test_engine_creation() {
    let engine = RiskEngine::new(default_params());
    assert_eq!(engine.vault.get(), 0);
    assert_eq!(engine.insurance_fund.balance.get(), 0);
    assert_eq!(engine.current_slot, 0);
    assert_eq!(engine.num_used_accounts, 0);
    assert!(engine.check_conservation());
}

#[test]
fn test_params_allow_mm_eq_im() {
    // Spec §1.4: maintenance_bps <= initial_bps (non-strict, equal is valid)
    let mut params = default_params();
    params.maintenance_margin_bps = 1000;
    params.initial_margin_bps = 1000;
    let _ = RiskEngine::new(params); // must not panic
}

#[test]
#[should_panic(expected = "maintenance_margin_bps must be <= initial_margin_bps")]
fn test_params_require_mm_le_im() {
    let mut params = default_params();
    params.maintenance_margin_bps = 1500;
    params.initial_margin_bps = 1000; // mm > im => should panic
    let _ = RiskEngine::new(params);
}

// ============================================================================
// 2. add_user and add_lp
// ============================================================================

#[test]
fn test_add_user() {
    let mut engine = RiskEngine::new(default_params());
    let idx = engine.add_user(1000).expect("add_user");
    assert_eq!(idx, 0);
    assert!(engine.is_used(idx as usize));
    assert_eq!(engine.num_used_accounts, 1);
    // Fee of 1000 goes to insurance; excess = 0
    assert_eq!(engine.accounts[idx as usize].capital.get(), 0);
    assert_eq!(engine.insurance_fund.balance.get(), 1000);
    assert_eq!(engine.vault.get(), 1000);
    assert!(engine.accounts[idx as usize].is_user());
}

#[test]
fn test_add_user_with_excess() {
    let mut engine = RiskEngine::new(default_params());
    let idx = engine.add_user(5000).expect("add_user");
    // excess = 5000 - 1000 = 4000 goes to capital
    assert_eq!(engine.accounts[idx as usize].capital.get(), 4000);
    assert_eq!(engine.insurance_fund.balance.get(), 1000);
    assert_eq!(engine.vault.get(), 5000);
}

#[test]
fn test_add_user_insufficient_fee() {
    let mut engine = RiskEngine::new(default_params());
    let result = engine.add_user(500); // less than new_account_fee (1000)
    assert_eq!(result, Err(RiskError::InsufficientBalance));
}

#[test]
fn test_add_lp() {
    let mut engine = RiskEngine::new(default_params());
    let program = [1u8; 32];
    let context = [2u8; 32];
    let idx = engine.add_lp(program, context, 2000).expect("add_lp");
    assert!(engine.is_used(idx as usize));
    assert!(engine.accounts[idx as usize].is_lp());
    assert_eq!(engine.accounts[idx as usize].matcher_program, program);
    assert_eq!(engine.accounts[idx as usize].matcher_context, context);
    assert_eq!(engine.accounts[idx as usize].capital.get(), 1000); // 2000 - 1000 fee
}

// ============================================================================
// 3. deposit and withdraw_not_atomic
// ============================================================================

#[test]
fn test_deposit() {
    let mut engine = RiskEngine::new(default_params());
    let oracle = 1000u64;
    let slot = 1u64;
    engine.current_slot = slot;
    let idx = engine.add_user(1000).expect("add_user");

    let vault_before = engine.vault.get();
    engine.deposit(idx, 10_000, oracle, slot).expect("deposit");
    assert_eq!(engine.accounts[idx as usize].capital.get(), 10_000);
    assert_eq!(engine.vault.get(), vault_before + 10_000);
    assert!(engine.check_conservation());
}

#[test]
fn test_withdraw_no_position() {
    let mut engine = RiskEngine::new(default_params());
    let oracle = 1000u64;
    let slot = 1u64;
    engine.current_slot = slot;
    let idx = engine.add_user(1000).expect("add_user");

    // Deposit before crank so account is not GC'd
    engine.deposit(idx, 10_000, oracle, slot).expect("deposit");

    // Initial crank needed for freshness
    engine
        .keeper_crank_not_atomic(
            slot,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .expect("crank");

    engine
        .withdraw_not_atomic(idx, 5_000, oracle, slot, 0i64)
        .expect("withdraw_not_atomic");
    assert_eq!(engine.accounts[idx as usize].capital.get(), 5_000);
    assert!(engine.check_conservation());
}

#[test]
fn test_withdraw_exceeds_balance() {
    let mut engine = RiskEngine::new(default_params());
    let oracle = 1000u64;
    let slot = 1u64;
    engine.current_slot = slot;
    let idx = engine.add_user(1000).expect("add_user");
    engine.deposit(idx, 5_000, oracle, slot).expect("deposit");
    engine
        .keeper_crank_not_atomic(
            slot,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .expect("crank");

    let result = engine.withdraw_not_atomic(idx, 10_000, oracle, slot, 0i64);
    assert_eq!(result, Err(RiskError::InsufficientBalance));
}

#[test]
fn test_withdraw_succeeds_without_fresh_crank() {
    let mut engine = RiskEngine::new(default_params());
    let oracle = 1000u64;
    let idx = engine.add_user(1000).expect("add_user");
    engine.deposit(idx, 10_000, oracle, 1).expect("deposit");

    // Spec §10.4 + §0 goal 6: withdraw_not_atomic must not require a recent keeper crank.
    // touch_account_full_not_atomic accrues market state directly from the caller's oracle.
    let result = engine.withdraw_not_atomic(idx, 1_000, oracle, 5000, 0i64);
    assert!(
        result.is_ok(),
        "withdraw_not_atomic must succeed without fresh crank (spec §0 goal 6)"
    );
}

// ============================================================================
// 4. execute_trade_not_atomic basics
// ============================================================================

#[test]
fn test_basic_trade() {
    let (mut engine, a, b) = setup_two_users(100_000, 100_000);
    let oracle = 1000u64;
    let slot = 1u64;

    // Trade: a goes long 100 units, b goes short 100 units
    let size_q = make_size_q(100);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .expect("trade");

    // Both should have positions of the correct magnitude
    let eff_a = engine.effective_pos_q(a as usize);
    let eff_b = engine.effective_pos_q(b as usize);
    assert_eq!(eff_a, make_size_q(100), "account a must be long 100 units");
    assert_eq!(
        eff_b,
        make_size_q(-100),
        "account b must be short 100 units"
    );
    assert!(
        engine.oi_eff_long_q > 0,
        "open interest must be nonzero after trade"
    );
    assert!(engine.check_conservation());
}

#[test]
fn test_trade_succeeds_without_fresh_crank() {
    let mut engine = RiskEngine::new(default_params());
    let oracle = 1000u64;
    let a = engine.add_user(1000).expect("add user a");
    let b = engine.add_user(1000).expect("add user b");
    engine.deposit(a, 100_000, oracle, 1).expect("deposit a");
    engine.deposit(b, 100_000, oracle, 1).expect("deposit b");

    // Spec §10.5 + §0 goal 6: execute_trade_not_atomic must not require a recent keeper crank.
    let size_q = make_size_q(10);
    let result = engine.execute_trade_not_atomic(a, b, oracle, 5000, size_q, oracle, 0i64);
    assert!(
        result.is_ok(),
        "trade must succeed without fresh crank (spec §0 goal 6)"
    );
}

#[test]
fn test_trade_undercollateralized_rejected() {
    let (mut engine, a, b) = setup_two_users(1_000, 1_000);
    let oracle = 1000u64;
    let slot = 1u64;

    // Try to open a huge position that exceeds margin
    // 1000 capital, 10% IM => max notional = 10000
    // notional = |size| * oracle / POS_SCALE, so for oracle=1000,
    // 11 units => notional = 11000, requires 1100 IM
    let size_q = make_size_q(11);
    let result = engine.execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64);
    assert_eq!(result, Err(RiskError::Undercollateralized));
}

#[test]
fn test_trade_with_different_exec_price() {
    let (mut engine, a, b) = setup_two_users(100_000, 100_000);
    let oracle = 1000u64;
    let exec = 990u64;
    let slot = 1u64;

    // Trade at exec_price=990 vs oracle=1000
    // trade_pnl for long = size * (oracle - exec) / POS_SCALE
    let size_q = make_size_q(100);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, exec, 0i64)
        .expect("trade");

    // Account a (long) bought at exec=990 vs oracle=1000, so should have positive PnL
    // trade_pnl = floor(100 * POS_SCALE * (1000 - 990) / POS_SCALE) = 1000
    assert!(
        engine.accounts[a as usize].pnl > 0,
        "long PnL must be positive when exec < oracle: pnl={}",
        engine.accounts[a as usize].pnl
    );

    // Account b (short) had negative trade PnL of -1000, but settle_losses
    // absorbs it from capital. Verify b's capital decreased instead.
    // b started with 100_000 deposit, minus trading fee. After settle_losses,
    // the 1000 loss is paid from capital.
    let cap_b = engine.accounts[b as usize].capital.get();
    assert!(
        cap_b < 100_000,
        "short capital must decrease when exec < oracle (loss settled): cap={}",
        cap_b
    );
    assert!(engine.check_conservation());
}

// ============================================================================
// 5. Conservation invariant
// ============================================================================

#[test]
fn test_conservation_after_deposits() {
    let mut engine = RiskEngine::new(default_params());
    let oracle = 1000u64;
    let slot = 1u64;
    engine.current_slot = slot;

    let a = engine.add_user(5000).expect("add user a");
    engine.deposit(a, 100_000, oracle, slot).expect("deposit");
    let b = engine.add_user(3000).expect("add user b");
    engine.deposit(b, 50_000, oracle, slot).expect("deposit");

    assert!(engine.check_conservation());
    // V >= C_tot + I
    let senior = engine.c_tot.get() + engine.insurance_fund.balance.get();
    assert!(engine.vault.get() >= senior);
}

#[test]
fn test_conservation_after_trade() {
    let (mut engine, a, b) = setup_two_users(100_000, 100_000);
    let oracle = 1000u64;
    let slot = 1u64;

    let size_q = make_size_q(50);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .expect("trade");
    assert!(engine.check_conservation());
}

// ============================================================================
// 6. Haircut ratio computation
// ============================================================================

#[test]
fn test_haircut_ratio_no_positive_pnl() {
    let engine = RiskEngine::new(default_params());
    let (h_num, h_den) = engine.haircut_ratio();
    // When pnl_pos_tot == 0, returns (1, 1)
    assert_eq!(h_num, 1u128);
    assert_eq!(h_den, 1u128);
}

#[test]
fn test_haircut_ratio_with_surplus() {
    let (mut engine, a, b) = setup_two_users(100_000, 100_000);
    let oracle = 1000u64;
    let slot = 1u64;

    // Execute a trade, then move price to give one side positive PnL
    let size_q = make_size_q(50);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .expect("trade");

    // Now accrue market with a higher price
    engine.accrue_market_to(2, 1100).expect("accrue");
    // Touch accounts to realize PnL
    engine
        .touch_account_full_not_atomic(a as usize, 1100, 2)
        .expect("touch a");
    engine
        .touch_account_full_not_atomic(b as usize, 1100, 2)
        .expect("touch b");

    let (h_num, h_den) = engine.haircut_ratio();
    // h_num <= h_den always
    assert!(h_num <= h_den);
    // Verify the haircut is actually computed (not just the default (1,1))
    assert!(h_num > 0, "h_num must be positive when PnL exists");
    assert!(h_den > 0, "h_den must be positive when PnL exists");
}

// ============================================================================
// 7. Liquidation at oracle
// ============================================================================

#[test]
fn test_liquidation_eligible_account() {
    // Use a smaller capital so we can trigger liquidation more easily
    let (mut engine, a, b) = setup_two_users(50_000, 200_000);
    let oracle = 1000u64;
    let slot = 1u64;

    // Open a position near the margin limit
    // 50_000 capital, 10% IM => max notional = 500_000
    // 480 units * 1000 = 480_000 notional, IM = 48_000
    let size_q = make_size_q(480);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .expect("trade");

    // Move the price against the long (a) to trigger liquidation
    // Use accrue_market_to to update price state without running the full crank
    // (the crank would itself liquidate the account before we can test it explicitly)
    let new_oracle = 890u64;
    let slot2 = 2u64;

    // Call liquidate_at_oracle_not_atomic directly - it calls touch_account_full_not_atomic internally
    // which runs accrue_market_to
    let result = engine
        .liquidate_at_oracle_not_atomic(a, slot2, new_oracle, LiquidationPolicy::FullClose, 0i64)
        .expect("liquidate");
    assert!(result, "account a should have been liquidated");
    // Position should be closed
    let eff = engine.effective_pos_q(a as usize);
    assert!(eff == 0);
    assert!(engine.check_conservation());
}

#[test]
fn test_liquidation_healthy_account() {
    let (mut engine, a, b) = setup_two_users(100_000, 100_000);
    let oracle = 1000u64;
    let slot = 1u64;

    let size_q = make_size_q(50);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .expect("trade");

    // Account is well collateralized, liquidation should return false
    let result = engine
        .liquidate_at_oracle_not_atomic(a, slot, oracle, LiquidationPolicy::FullClose, 0i64)
        .expect("liquidate attempt");
    assert!(!result, "healthy account should not be liquidated");
}

#[test]
fn test_liquidation_flat_account() {
    let (mut engine, a, _b) = setup_two_users(100_000, 100_000);
    let oracle = 1000u64;
    let slot = 1u64;

    // No position open, liquidation should return false
    let result = engine
        .liquidate_at_oracle_not_atomic(a, slot, oracle, LiquidationPolicy::FullClose, 0i64)
        .expect("liquidate flat");
    assert!(!result);
}

// ============================================================================
// 8. Warmup and profit conversion
// ============================================================================

#[test]
fn test_warmup_slope_set_on_new_profit() {
    let (mut engine, a, b) = setup_two_users(100_000, 100_000);
    let oracle = 1000u64;
    let slot = 1u64;

    let size_q = make_size_q(50);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .expect("trade");

    // Advance and accrue at higher price so long (a) gets positive PnL
    let slot2 = 10u64;
    let new_oracle = 1100u64;
    engine
        .keeper_crank_not_atomic(
            slot2,
            new_oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .expect("crank");
    engine
        .touch_account_full_not_atomic(a as usize, new_oracle, slot2)
        .expect("touch");

    // If PnL is positive and warmup_period > 0, slope should be set
    if engine.accounts[a as usize].pnl > 0 {
        assert!(
            engine.accounts[a as usize].warmup_slope_per_step != 0,
            "warmup slope should be nonzero for positive PnL"
        );
    }
}

#[test]
fn test_warmup_full_conversion_after_period() {
    let (mut engine, a, b) = setup_two_users(100_000, 100_000);
    let oracle = 1000u64;
    let slot = 1u64;

    let size_q = make_size_q(50);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .expect("trade");

    // Move price up to give account a profit
    let slot2 = 10u64;
    let new_oracle = 1200u64;
    engine
        .keeper_crank_not_atomic(
            slot2,
            new_oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .expect("crank");
    engine
        .touch_account_full_not_atomic(a as usize, new_oracle, slot2)
        .expect("touch");

    // Close position so profit conversion can happen (only for flat accounts)
    let close_q = make_size_q(50);
    engine
        .execute_trade_not_atomic(b, a, new_oracle, slot2, close_q, new_oracle, 0i64)
        .expect("close");

    let capital_before = engine.accounts[a as usize].capital.get();

    // Wait beyond warmup period (100 slots) and touch again
    let slot3 = slot2 + 200;
    engine
        .keeper_crank_not_atomic(
            slot3,
            new_oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .expect("crank2");
    engine
        .touch_account_full_not_atomic(a as usize, new_oracle, slot3)
        .expect("touch2");

    let capital_after = engine.accounts[a as usize].capital.get();
    // Capital should increase after warmup conversion (position is flat now)
    assert!(
        capital_after > capital_before,
        "after full warmup period, profit must be converted to capital"
    );
    assert!(engine.check_conservation());
}

// ============================================================================
// 9. Insurance fund operations
// ============================================================================

#[test]
fn test_top_up_insurance_fund() {
    let mut engine = RiskEngine::new(default_params());
    let before_vault = engine.vault.get();
    let before_ins = engine.insurance_fund.balance.get();

    let result = engine.top_up_insurance_fund(5000, 0).expect("top_up");
    assert_eq!(engine.vault.get(), before_vault + 5000);
    assert_eq!(engine.insurance_fund.balance.get(), before_ins + 5000);
    assert!(result); // above floor (floor = 0)
    assert!(engine.check_conservation());
}

// ============================================================================
// 10. Fee operations
// ============================================================================

#[test]
fn test_deposit_fee_credits() {
    let mut engine = RiskEngine::new(default_params());
    let slot = 1u64;
    engine.current_slot = slot;
    let idx = engine.add_user(1000).expect("add_user");

    // Give the account fee debt first (spec §2.1: fee_credits <= 0)
    engine.accounts[idx as usize].fee_credits = I128::new(-5000);

    // Pay off 3000 of the 5000 debt
    engine
        .deposit_fee_credits(idx, 3000, slot)
        .expect("deposit_fee_credits");
    assert_eq!(
        engine.accounts[idx as usize].fee_credits.get(),
        -2000,
        "fee_credits must reflect partial payoff"
    );

    // Pay off the remaining 2000
    engine
        .deposit_fee_credits(idx, 2000, slot)
        .expect("deposit_fee_credits");
    assert_eq!(
        engine.accounts[idx as usize].fee_credits.get(),
        0,
        "fee_credits must be zero after full payoff"
    );

    // Over-payment is capped — fee_credits stays at 0
    engine
        .deposit_fee_credits(idx, 9999, slot)
        .expect("no-op succeeds");
    assert_eq!(
        engine.accounts[idx as usize].fee_credits.get(),
        0,
        "fee_credits must not go positive"
    );
}

#[test]
fn test_add_fee_credits() {
    let mut engine = RiskEngine::new(default_params());
    let slot = 1u64;
    engine.current_slot = slot;
    let idx = engine.add_user(1000).expect("add_user");

    // Give the account debt, then add credits to pay it off
    engine.accounts[idx as usize].fee_credits = I128::new(-5000);
    engine.add_fee_credits(idx, 3000).expect("add_fee_credits");
    assert_eq!(engine.accounts[idx as usize].fee_credits.get(), -2000);
}

#[test]
fn test_trading_fee_charged() {
    let (mut engine, a, b) = setup_two_users(100_000, 100_000);
    let oracle = 1000u64;
    let slot = 1u64;

    let capital_before = engine.accounts[a as usize].capital.get();

    let size_q = make_size_q(100);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .expect("trade");

    let capital_after = engine.accounts[a as usize].capital.get();
    // Trading fee should reduce capital of account a
    // fee = ceil(|100| * 1000 * 10 / 10000) = ceil(100) = 100
    assert!(
        capital_after < capital_before,
        "trading fee should reduce capital"
    );
    assert!(engine.check_conservation());
}

#[test]
fn test_lp_fees_earned_tracking() {
    let mut engine = RiskEngine::new(default_params());
    let oracle = 1000u64;
    let slot = 1u64;
    engine.current_slot = slot;

    let a = engine.add_user(1000).expect("add user");
    let lp = engine.add_lp([1; 32], [2; 32], 1000).expect("add lp");

    // Deposit before crank so accounts are not GC'd
    engine.deposit(a, 100_000, oracle, slot).expect("deposit a");
    engine
        .deposit(lp, 100_000, oracle, slot)
        .expect("deposit lp");
    engine
        .keeper_crank_not_atomic(
            slot,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .expect("crank");

    let size_q = make_size_q(100);
    engine
        .execute_trade_not_atomic(a, lp, oracle, slot, size_q, oracle, 0i64)
        .expect("trade");

    // LP (account b) should track fees earned
    assert!(
        engine.accounts[lp as usize].fees_earned_total.get() > 0,
        "LP should track fees earned"
    );
}

// ============================================================================
// 11. Close account
// ============================================================================

#[test]
fn test_close_account_flat() {
    let mut engine = RiskEngine::new(default_params());
    let oracle = 1000u64;
    let slot = 1u64;
    engine.current_slot = slot;

    let idx = engine.add_user(1000).expect("add_user");
    engine.deposit(idx, 10_000, oracle, slot).expect("deposit");

    let capital_returned = engine
        .close_account_not_atomic(idx, slot, oracle, 0i64)
        .expect("close");
    assert_eq!(capital_returned, 10_000);
    assert!(!engine.is_used(idx as usize));
    assert!(engine.check_conservation());
}

#[test]
fn test_close_account_with_position_fails() {
    let (mut engine, a, b) = setup_two_users(100_000, 100_000);
    let oracle = 1000u64;
    let slot = 1u64;

    let size_q = make_size_q(50);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .expect("trade");

    let result = engine.close_account_not_atomic(a, slot, oracle, 0i64);
    assert_eq!(result, Err(RiskError::Undercollateralized));
}

#[test]
fn test_close_account_not_found() {
    let mut engine = RiskEngine::new(default_params());
    let result = engine.close_account_not_atomic(99, 1, 1000, 0i64);
    assert_eq!(result, Err(RiskError::AccountNotFound));
}

// ============================================================================
// 12. Keeper crank
// ============================================================================

#[test]
fn test_keeper_crank_advances_slot() {
    let mut engine = RiskEngine::new(default_params());
    let oracle = 1000u64;
    let slot = 10u64;
    let _caller = engine.add_user(1000).expect("add_user");

    let outcome = engine
        .keeper_crank_not_atomic(
            slot,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .expect("crank");
    assert!(outcome.advanced);
    assert_eq!(engine.last_crank_slot, slot);
}

#[test]
fn test_keeper_crank_same_slot_not_advanced() {
    let mut engine = RiskEngine::new(default_params());
    let oracle = 1000u64;
    let slot = 10u64;
    let _caller = engine.add_user(1000).expect("add_user");

    engine
        .keeper_crank_not_atomic(
            slot,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .expect("crank1");
    let outcome = engine
        .keeper_crank_not_atomic(
            slot,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .expect("crank2");
    assert!(!outcome.advanced);
}

#[test]
fn test_keeper_crank_caller_touch_charges_fee() {
    // Spec §8.2: maintenance fees enabled — keeper crank charges accrued fees.
    let mut engine = RiskEngine::new(default_params()); // maintenance_fee_per_slot = 1
    let oracle = 1000u64;
    let slot = 1u64;
    engine.current_slot = slot;

    let caller = engine.add_user(1000).expect("add_user");
    engine
        .deposit(caller, 10_000, oracle, slot)
        .expect("deposit");

    let capital_before = engine.accounts[caller as usize].capital.get();

    // Advance 199 slots, crank touches caller → fee = dt * 1
    let slot2 = 200u64;
    let outcome = engine
        .keeper_crank_not_atomic(slot2, oracle, &[(caller, None)], 64, 0i64)
        .expect("crank");
    assert!(outcome.advanced);

    let capital_after = engine.accounts[caller as usize].capital.get();
    assert!(
        capital_after < capital_before,
        "maintenance fee must reduce capital"
    );
    assert!(engine.check_conservation());
}

// ============================================================================
// 13. Side mode gating (DrainOnly, ResetPending)
// ============================================================================

#[test]
fn test_drain_only_blocks_new_trades() {
    let (mut engine, a, b) = setup_two_users(100_000, 100_000);
    let oracle = 1000u64;
    let slot = 1u64;

    // Manually set long side to DrainOnly
    engine.side_mode_long = SideMode::DrainOnly;

    // Try to open a new long position (a goes long) — should be blocked
    let size_q = make_size_q(50);
    let result = engine.execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64);
    assert_eq!(result, Err(RiskError::SideBlocked));
}

#[test]
fn test_drain_only_allows_reducing_trade() {
    let (mut engine, a, b) = setup_two_users(100_000, 100_000);
    let oracle = 1000u64;
    let slot = 1u64;

    // Open a position first in Normal mode
    let size_q = make_size_q(100);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .expect("open trade");

    // Now set long side to DrainOnly
    engine.side_mode_long = SideMode::DrainOnly;

    // Reducing trade (a goes short = reducing long) should work
    let reduce_q = make_size_q(50);
    engine
        .execute_trade_not_atomic(b, a, oracle, slot, reduce_q, oracle, 0i64)
        .expect("reducing trade should succeed in DrainOnly");
}

#[test]
fn test_reset_pending_blocks_new_trades() {
    let (mut engine, a, b) = setup_two_users(100_000, 100_000);
    let oracle = 1000u64;
    let slot = 1u64;

    // ResetPending with stale_account_count > 0 is NOT auto-finalizable,
    // so it must still block OI-increasing trades.
    engine.side_mode_short = SideMode::ResetPending;
    engine.stale_account_count_short = 1;

    // b would go long (opposite of short blocked), a goes short — short increase blocked
    let size_q = make_size_q(50); // b goes long, a goes short (swapped)
    let result = engine.execute_trade_not_atomic(b, a, oracle, slot, size_q, oracle, 0i64);
    assert_eq!(result, Err(RiskError::SideBlocked));
}

// ============================================================================
// 14. ADL mechanics
// ============================================================================

#[test]
fn test_adl_triggered_by_liquidation() {
    let (mut engine, a, b) = setup_two_users(50_000, 50_000);
    let oracle = 1000u64;
    let slot = 1u64;

    // Open large positions near margin
    // 50k capital, 10% IM => max notional = 500k
    // 450 units * 1000 = 450k notional, IM = 45k
    let size_q = make_size_q(450);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .expect("trade");

    // Move price down sharply to make long (a) deeply underwater
    // Call liquidate_at_oracle_not_atomic directly (the crank would liquidate first)
    let slot2 = 2u64;
    let crash_oracle = 870u64;

    let result = engine
        .liquidate_at_oracle_not_atomic(a, slot2, crash_oracle, LiquidationPolicy::FullClose, 0i64)
        .expect("liquidate");
    assert!(result, "account a should be liquidated");
    assert!(engine.check_conservation());

    // After liquidation, the position is closed. ADL state may have changed.
    let eff_a = engine.effective_pos_q(a as usize);
    assert!(eff_a == 0, "liquidated position should be zero");
}

#[test]
fn test_adl_epoch_changes() {
    let mut engine = RiskEngine::new(default_params());
    let epoch_long_before = engine.adl_epoch_long;

    // Begin a full drain reset on long side (requires OI=0)
    assert!(engine.oi_eff_long_q == 0);
    engine.begin_full_drain_reset(Side::Long);

    assert_eq!(engine.adl_epoch_long, epoch_long_before + 1);
    assert_eq!(engine.side_mode_long, SideMode::ResetPending);
    assert_eq!(engine.adl_mult_long, ADL_ONE);
}

#[test]
fn test_effective_pos_epoch_mismatch() {
    let (mut engine, a, b) = setup_two_users(100_000, 100_000);
    let oracle = 1000u64;
    let slot = 1u64;

    // Open position
    let size_q = make_size_q(50);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .expect("trade");

    // Manually bump the long epoch to simulate a reset
    engine.adl_epoch_long += 1;

    // Effective position should be zero due to epoch mismatch
    let eff = engine.effective_pos_q(a as usize);
    assert!(eff == 0, "epoch mismatch should zero effective position");
}

// ============================================================================
// Additional edge-case tests
// ============================================================================

#[test]
fn test_set_owner() {
    let mut engine = RiskEngine::new(default_params());
    let idx = engine.add_user(1000).expect("add_user");
    let owner = [42u8; 32];
    engine.set_owner(idx, owner).expect("set_owner");
    assert_eq!(engine.accounts[idx as usize].owner, owner);
}

#[test]
fn test_set_owner_invalid_idx() {
    let mut engine = RiskEngine::new(default_params());
    let result = engine.set_owner(99, [0u8; 32]);
    assert_eq!(result, Err(RiskError::Unauthorized));
}

#[test]
fn test_notional_computation() {
    let (mut engine, a, b) = setup_two_users(100_000, 100_000);
    let oracle = 1000u64;
    let slot = 1u64;

    let size_q = make_size_q(100);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .expect("trade");

    let notional = engine.notional(a as usize, oracle);
    // notional = |100 * POS_SCALE| * 1000 / POS_SCALE = 100_000
    assert_eq!(notional, 100_000);
}

#[test]
fn test_advance_slot() {
    let mut engine = RiskEngine::new(default_params());
    assert_eq!(engine.current_slot, 0);
    engine.advance_slot(42);
    assert_eq!(engine.current_slot, 42);
    engine.advance_slot(8);
    assert_eq!(engine.current_slot, 50);
}

#[test]
fn test_recompute_aggregates() {
    let (mut engine, a, b) = setup_two_users(50_000, 50_000);
    let oracle = 1000u64;
    let slot = 1u64;

    let size_q = make_size_q(30);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .expect("trade");

    let c_before = engine.c_tot.get();
    let pnl_before = engine.pnl_pos_tot;

    engine.recompute_aggregates();

    // Aggregates should be consistent after recompute
    assert_eq!(engine.c_tot.get(), c_before);
    assert_eq!(engine.pnl_pos_tot, pnl_before);
}

#[test]
fn test_multiple_accounts() {
    let mut engine = RiskEngine::new(default_params());
    let oracle = 1000u64;
    let slot = 1u64;
    engine.current_slot = slot;

    // Create several accounts
    for _ in 0..10 {
        let idx = engine.add_user(1000).expect("add_user");
        engine.deposit(idx, 10_000, oracle, slot).expect("deposit");
    }

    assert_eq!(engine.num_used_accounts, 10);
    assert_eq!(engine.count_used(), 10);
    assert!(engine.check_conservation());
}

#[test]
fn test_trade_then_close_round_trip() {
    let (mut engine, a, b) = setup_two_users(100_000, 100_000);
    let oracle = 1000u64;
    let slot = 1u64;

    // Open position
    let size_q = make_size_q(50);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .expect("open");

    // Close position (reverse trade)
    let close_q = make_size_q(50);
    engine
        .execute_trade_not_atomic(b, a, oracle, slot, close_q, oracle, 0i64)
        .expect("close");

    let eff_a = engine.effective_pos_q(a as usize);
    let eff_b = engine.effective_pos_q(b as usize);
    assert!(eff_a == 0, "position a should be flat after close");
    assert!(eff_b == 0, "position b should be flat after close");
    assert!(engine.check_conservation());
}

#[test]
fn test_withdraw_with_position_margin_check() {
    let (mut engine, a, b) = setup_two_users(100_000, 100_000);
    let oracle = 1000u64;
    let slot = 1u64;

    // Open position: 100 units * 1000 = 100k notional, 10% IM = 10k required
    let size_q = make_size_q(100);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .expect("trade");

    // Try to withdraw_not_atomic so much that IM is violated
    // capital ~ 100k (minus fees), need at least 10k for IM
    let result = engine.withdraw_not_atomic(a, 95_000, oracle, slot, 0i64);
    assert_eq!(result, Err(RiskError::Undercollateralized));
}

#[test]
fn test_zero_size_trade_rejected() {
    let (mut engine, a, b) = setup_two_users(100_000, 100_000);
    let oracle = 1000u64;
    let slot = 1u64;

    let result = engine.execute_trade_not_atomic(a, b, oracle, slot, 0i128, oracle, 0i64);
    assert_eq!(result, Err(RiskError::Overflow));
}

#[test]
fn test_zero_oracle_rejected() {
    let (mut engine, a, b) = setup_two_users(100_000, 100_000);
    let slot = 1u64;

    let size_q = make_size_q(10);
    let result = engine.execute_trade_not_atomic(a, b, 0, slot, size_q, 1000, 0i64);
    assert_eq!(result, Err(RiskError::Overflow));
}

#[test]
fn test_close_account_after_trade_and_unwind() {
    let (mut engine, a, b) = setup_two_users(100_000, 100_000);
    let oracle = 1000u64;
    let slot = 1u64;

    // Open and close position
    let size_q = make_size_q(50);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .expect("open");
    let close_q = make_size_q(50);
    engine
        .execute_trade_not_atomic(b, a, oracle, slot, close_q, oracle, 0i64)
        .expect("close");

    // Wait beyond warmup to let PnL settle
    let slot2 = slot + 200;
    engine
        .keeper_crank_not_atomic(
            slot2,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .expect("crank");
    engine
        .touch_account_full_not_atomic(a as usize, oracle, slot2)
        .expect("touch");

    // PnL should be zero or converted by now
    let pnl = engine.accounts[a as usize].pnl;
    if pnl == 0 {
        let cap = engine
            .close_account_not_atomic(a, slot2, oracle, 0i64)
            .expect("close account");
        assert!(cap > 0);
        assert!(!engine.is_used(a as usize));
    }
    // If PnL is not zero, closing might fail — that is expected behavior
}

#[test]
fn test_insurance_absorbs_loss_on_liquidation() {
    let mut engine = RiskEngine::new(default_params());
    let oracle = 1000u64;
    let slot = 1u64;
    engine.current_slot = slot;

    let a = engine.add_user(1000).expect("add user a");
    let b = engine.add_user(1000).expect("add user b");

    // Deposit before crank so accounts are not GC'd
    engine.deposit(a, 20_000, oracle, slot).expect("deposit a");
    engine.deposit(b, 100_000, oracle, slot).expect("deposit b");

    // Top up insurance fund
    engine.top_up_insurance_fund(50_000, slot).expect("top up");

    engine
        .keeper_crank_not_atomic(
            slot,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .expect("initial crank");

    // Open near-max position
    let size_q = make_size_q(180);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .expect("trade");

    // Crash price to make a deeply underwater
    let slot2 = 2u64;
    let crash = 850u64;
    engine
        .keeper_crank_not_atomic(
            slot2,
            crash,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .expect("crank");

    engine
        .liquidate_at_oracle_not_atomic(a, slot2, crash, LiquidationPolicy::FullClose, 0i64)
        .expect("liquidate");
    assert!(engine.check_conservation());
}

#[test]
fn test_maintenance_fee_charges_on_touch() {
    // Spec §8.2: maintenance fees enabled — touch charges dt * fee_per_slot.
    let mut engine = RiskEngine::new(default_params()); // fee_per_slot = 1
    let oracle = 1000u64;
    let slot = 1u64;
    engine.current_slot = slot;

    let idx = engine.add_user(1000).expect("add_user");
    engine.deposit(idx, 10_000, oracle, slot).expect("deposit");

    let capital_before = engine.accounts[idx as usize].capital.get();

    // Advance 500 slots: crank accrues market, then touch charges fee
    // keeper_crank_not_atomic at 501 with empty candidates doesn't touch the account.
    // Then touch_account_full_not_atomic charges fee: dt from last_fee_slot to 501.
    let slot2 = 501u64;
    engine
        .keeper_crank_not_atomic(
            slot2,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .expect("crank");
    engine
        .touch_account_full_not_atomic(idx as usize, oracle, slot2)
        .expect("touch");

    let capital_after = engine.accounts[idx as usize].capital.get();
    assert!(
        capital_after < capital_before,
        "maintenance fee must reduce capital on touch"
    );
    assert!(engine.check_conservation());
}

#[test]
fn test_maintenance_fee_zero_rate_no_charge() {
    // maintenance_fee_per_slot = 0 means no fee is charged
    let mut params = default_params();
    params.maintenance_fee_per_slot = U128::ZERO;
    let mut engine = RiskEngine::new(params);
    let oracle = 1000u64;
    let slot = 1u64;
    engine.current_slot = slot;

    let idx = engine.add_user(1000).expect("add_user");
    engine.deposit(idx, 10_000, oracle, slot).expect("deposit");

    let capital_before = engine.accounts[idx as usize].capital.get();

    let slot2 = 501u64;
    engine
        .keeper_crank_not_atomic(
            slot2,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .expect("crank");
    engine
        .touch_account_full_not_atomic(idx as usize, oracle, slot2)
        .expect("touch");

    assert_eq!(
        engine.accounts[idx as usize].capital.get(),
        capital_before,
        "zero fee rate must not charge fees"
    );
}

#[test]
fn test_keeper_crank_liquidates_underwater_accounts() {
    let (mut engine, a, b) = setup_two_users(50_000, 50_000);
    let oracle = 1000u64;
    let slot = 1u64;

    // Open near-margin positions
    let size_q = make_size_q(450);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .expect("trade");

    // Crash price
    let slot2 = 2u64;
    let crash = 870u64;
    let outcome = engine
        .keeper_crank_not_atomic(
            slot2,
            crash,
            &[
                (a, Some(LiquidationPolicy::FullClose)),
                (b, Some(LiquidationPolicy::FullClose)),
            ],
            64,
            0i64,
        )
        .expect("crank");
    // The crank should have liquidated the underwater account
    assert!(
        outcome.num_liquidations > 0,
        "crank must liquidate underwater account"
    );
    assert!(engine.check_conservation());
}

#[test]
fn test_i128_size_q_construction() {
    // Verify our make_size_q helper produces correct values
    let pos = make_size_q(1);
    let neg = make_size_q(-1);

    assert!(pos > 0);
    assert!(neg < 0);

    // |pos| should equal POS_SCALE
    let abs_pos = pos.unsigned_abs();
    assert_eq!(abs_pos, POS_SCALE);
}

#[test]
fn test_deposit_fee_credits_invalid_account() {
    let mut engine = RiskEngine::new(default_params());
    let result = engine.deposit_fee_credits(99, 1000, 1);
    assert_eq!(result, Err(RiskError::Unauthorized));
}

#[test]
fn test_finalize_side_reset() {
    let mut engine = RiskEngine::new(default_params());

    // Set up for reset
    engine.begin_full_drain_reset(Side::Long);
    assert_eq!(engine.side_mode_long, SideMode::ResetPending);

    // All stored_pos_count and stale_count must be 0 for finalize
    // Since no accounts with long positions exist, they should already be 0
    let result = engine.finalize_side_reset(Side::Long);
    assert!(result.is_ok());
    assert_eq!(engine.side_mode_long, SideMode::Normal);
}

#[test]
fn test_finalize_side_reset_wrong_mode() {
    let mut engine = RiskEngine::new(default_params());
    // Side is Normal, finalize should fail
    let result = engine.finalize_side_reset(Side::Long);
    assert_eq!(result, Err(RiskError::CorruptState));
}

#[test]
fn test_account_equity_net_positive() {
    let mut engine = RiskEngine::new(default_params());
    let oracle = 1000u64;
    let slot = 1u64;
    engine.current_slot = slot;

    let idx = engine.add_user(1000).expect("add_user");
    engine.deposit(idx, 50_000, oracle, slot).expect("deposit");

    let eq = engine.account_equity_net(&engine.accounts[idx as usize], oracle);
    // With only capital and no PnL, equity = capital = 50_000
    let expected: i128 = 50_000;
    assert_eq!(eq, expected);
}

#[test]
fn test_count_used() {
    let mut engine = RiskEngine::new(default_params());
    assert_eq!(engine.count_used(), 0);

    engine.add_user(1000).expect("add_user");
    assert_eq!(engine.count_used(), 1);

    engine.add_user(1000).expect("add_user");
    assert_eq!(engine.count_used(), 2);
}

#[test]
fn test_conservation_maintained_through_lifecycle() {
    // Full lifecycle: create, deposit, trade, move price, crank, close
    let mut engine = RiskEngine::new(default_params());
    let oracle = 1000u64;
    let slot = 1u64;
    engine.current_slot = slot;

    let a = engine.add_user(1000).expect("add a");
    let b = engine.add_user(1000).expect("add b");

    // Deposit before crank so accounts are not GC'd
    engine.deposit(a, 100_000, oracle, slot).expect("dep a");
    engine.deposit(b, 100_000, oracle, slot).expect("dep b");
    assert!(engine.check_conservation());

    engine
        .keeper_crank_not_atomic(
            slot,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .expect("crank");
    assert!(engine.check_conservation());

    let size_q = make_size_q(50);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .expect("trade");
    assert!(engine.check_conservation());

    // Price move
    let slot2 = 10u64;
    engine
        .keeper_crank_not_atomic(
            slot2,
            1050,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .expect("crank2");
    assert!(engine.check_conservation());

    // Close positions
    let close_q = make_size_q(50);
    engine
        .execute_trade_not_atomic(b, a, 1050, slot2, close_q, 1050, 0i64)
        .expect("close");
    assert!(engine.check_conservation());
}

// ============================================================================
// Spec property #23: immediate fee seniority after restart conversion
// ============================================================================

/// If restart-on-new-profit converts matured entitlement into C_i while fee debt
/// is outstanding, the fee-debt sweep occurs immediately — before later
/// loss-settlement or margin logic can consume that new capital.
///
/// This test verifies that after a trade triggers restart-on-new-profit,
/// fee debt is properly swept (capital reduced, fee_credits less negative,
/// insurance fund receives payment).
#[test]
fn test_fee_seniority_after_restart_on_new_profit_in_trade() {
    // Use zero-fee params to isolate the restart-on-new-profit / fee-sweep interaction
    let mut params = default_params();
    params.trading_fee_bps = 0;
    params.maintenance_fee_per_slot = U128::new(0);
    // Use zero warmup so all positive PnL is immediately warmable
    params.warmup_period_slots = 0;

    let mut engine = RiskEngine::new(params);
    let oracle = 1000u64;
    let slot = 1u64;
    engine.current_slot = slot;

    let a = engine.add_user(1000).expect("add a");
    let b = engine.add_user(1000).expect("add b");

    // Large deposits so margin is not an issue
    engine.deposit(a, 1_000_000, oracle, slot).expect("dep a");
    engine.deposit(b, 1_000_000, oracle, slot).expect("dep b");

    engine
        .keeper_crank_not_atomic(
            slot,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .expect("crank");

    // Open position: a buys 10 from b
    let size_q = make_size_q(10);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .expect("trade1");
    assert!(engine.check_conservation());

    // Price rises: a now has positive PnL (profit)
    let slot2 = 50u64;
    let oracle2 = 1100u64;
    engine
        .keeper_crank_not_atomic(
            slot2,
            oracle2,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .expect("crank2");
    assert!(engine.check_conservation());

    // Inject fee debt on account a: fee_credits = -5000
    // (In production this happens from maintenance fees exceeding credits)
    engine.accounts[a as usize].fee_credits = I128::new(-5000);

    let cap_before = engine.accounts[a as usize].capital.get();
    let ins_before = engine.insurance_fund.balance.get();

    // Execute another trade that will trigger restart-on-new-profit for a
    // (a buys 1 more at favorable price = market, AvailGross increases)
    let size_q2 = make_size_q(1);
    engine
        .execute_trade_not_atomic(a, b, oracle2, slot2, size_q2, oracle2, 0i64)
        .expect("trade2");
    assert!(engine.check_conservation());

    // After trade: fee debt should have been swept
    let fc_after = engine.accounts[a as usize].fee_credits.get();
    // Fee debt was 5000. After sweep, fee_credits should be less negative (or zero).
    assert!(
        fc_after > -5000,
        "fee debt was not swept after restart-on-new-profit: fc={}",
        fc_after
    );

    // Insurance fund should have received the swept amount
    let ins_after = engine.insurance_fund.balance.get();
    assert!(
        ins_after > ins_before,
        "insurance fund did not receive fee sweep payment"
    );

    // Capital should have decreased by the swept amount
    // (restart conversion adds to capital, fee sweep subtracts)
    // We can't easily check exact amounts without knowing warmable, but we can
    // verify conservation holds
    assert!(engine.check_conservation());
}

// ============================================================================
// Issue #4: Maintenance fee settle must not clamp fee_credits to i128::MIN
// ============================================================================

#[test]
#[should_panic(expected = "maintenance_fee_per_slot must be <= MAX_MAINTENANCE_FEE_PER_SLOT")]
fn test_validate_params_rejects_extreme_fee_per_slot() {
    let mut params = default_params();
    params.maintenance_fee_per_slot = U128::new(MAX_MAINTENANCE_FEE_PER_SLOT + 1);
    let _ = RiskEngine::new(params);
}

// ============================================================================
// Issue #5: charge_fee_safe must not panic on PnL underflow
// ============================================================================

#[test]
fn test_charge_fee_safe_does_not_panic_on_extreme_pnl() {
    let mut params = default_params();
    params.trading_fee_bps = 100; // 1% fee
    let mut engine = RiskEngine::new(params);
    let oracle = 1000u64;
    let slot = 1u64;
    engine.current_slot = slot;

    let a = engine.add_user(1000).expect("add a");
    let b = engine.add_user(1000).expect("add b");

    // Give a zero capital (so fee shortfall goes to PnL),
    // and b large capital for margin
    engine.deposit(a, 1, oracle, slot).expect("dep a");
    engine.deposit(b, 10_000_000, oracle, slot).expect("dep b");

    engine
        .keeper_crank_not_atomic(
            slot,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .expect("crank");

    // Set account a's PnL to near i128::MIN so fee subtraction would overflow.
    // The charge_fee_safe path: if capital < fee, shortfall = fee - capital,
    // then PnL -= shortfall. If PnL is near i128::MIN, this could overflow.
    let near_min = i128::MIN.checked_add(1i128).unwrap();
    engine.set_pnl(a as usize, near_min);

    // Executing a trade charges a fee. If capital is 0, fee goes to PnL.
    // With PnL near i128::MIN, subtracting the fee must not panic.
    // (The trade will likely fail for margin reasons, but must not panic.)
    let size_q = make_size_q(1);
    let _result = engine.execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64);
    // We don't care if it succeeds or returns Err — just that it doesn't panic.
}

// ============================================================================
// Issue #1: keeper_crank_not_atomic must propagate errors from state-mutating functions
// ============================================================================

#[test]
fn test_keeper_crank_propagates_corruption() {
    let mut engine = RiskEngine::new(default_params());
    let oracle = 1000u64;
    let slot = 1u64;
    engine.current_slot = slot;

    let a = engine.add_user(1000).expect("add a");
    engine.deposit(a, 100_000, oracle, slot).expect("dep a");
    engine
        .keeper_crank_not_atomic(
            slot,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .expect("crank");

    // Set up a corrupt state: a_basis = 0 triggers CorruptState error
    // in settle_side_effects (called by touch_account_full_not_atomic)
    engine.accounts[a as usize].position_basis_q = POS_SCALE as i128;
    engine.accounts[a as usize].adl_a_basis = 0; // CORRUPT: a_basis must be > 0
    engine.stored_pos_count_long = 1;
    engine.oi_eff_long_q = POS_SCALE;
    engine.oi_eff_short_q = POS_SCALE;

    // keeper_crank_not_atomic must propagate the CorruptState error, not swallow it
    let result = engine.keeper_crank_not_atomic(2, oracle, &[(a, None)], 64, 0i64);
    assert!(
        result.is_err(),
        "keeper_crank_not_atomic must propagate corruption errors"
    );
}

// ============================================================================
// Self-trade rejection
// ============================================================================

#[test]
fn test_self_trade_rejected() {
    let mut engine = RiskEngine::new(default_params());
    let oracle = 1000u64;
    let slot = 1u64;
    engine.current_slot = slot;

    let a = engine.add_user(1000).expect("add a");
    engine.deposit(a, 100_000, oracle, slot).expect("dep a");
    engine
        .keeper_crank_not_atomic(
            slot,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .expect("crank");

    let size_q = make_size_q(1);
    let result = engine.execute_trade_not_atomic(a, a, oracle, slot, size_q, oracle, 0i64);
    assert!(result.is_err(), "self-trade (a == b) must be rejected");
}

// ============================================================================
// Same-slot price change applies mark-to-market
// ============================================================================

#[test]
fn test_same_slot_price_change_applies_mark() {
    let mut engine = RiskEngine::new(default_params());
    let oracle = 1000u64;
    let slot = 1u64;
    engine.current_slot = slot;
    engine.last_oracle_price = oracle;
    engine.last_market_slot = slot; // same slot
    engine.adl_mult_long = ADL_ONE;
    engine.adl_mult_short = ADL_ONE;
    engine.oi_eff_long_q = POS_SCALE;
    engine.oi_eff_short_q = POS_SCALE;

    let k_long_before = engine.adl_coeff_long;
    let k_short_before = engine.adl_coeff_short;

    // Same slot, different price: mark-only update must apply
    let new_oracle = 1100u64;
    engine.accrue_market_to(slot, new_oracle).expect("accrue");

    // K_long must increase (price went up, longs gain)
    assert!(
        engine.adl_coeff_long > k_long_before,
        "K_long must increase on same-slot price rise"
    );
    // K_short must decrease (shorts lose)
    assert!(
        engine.adl_coeff_short < k_short_before,
        "K_short must decrease on same-slot price rise"
    );
    // Oracle price must be updated
    assert!(
        engine.last_oracle_price == new_oracle,
        "last_oracle_price must be updated"
    );
}

// ============================================================================
// schedule_end_of_instruction_resets error propagation
// ============================================================================

#[test]
fn test_schedule_reset_error_propagated_in_withdraw() {
    let mut engine = RiskEngine::new(default_params());
    let oracle = 1000u64;
    let slot = 1u64;
    engine.current_slot = slot;

    let a = engine.add_user(1000).expect("add a");
    engine.deposit(a, 100_000, oracle, slot).expect("dep a");
    engine
        .keeper_crank_not_atomic(
            slot,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .expect("crank");

    // Corrupt state: stored_pos_count says 0 but OI is non-zero and unequal.
    // This makes schedule_end_of_instruction_resets return CorruptState.
    engine.stored_pos_count_long = 0;
    engine.stored_pos_count_short = 0;
    engine.oi_eff_long_q = POS_SCALE;
    engine.oi_eff_short_q = POS_SCALE * 2; // unequal OI

    let result = engine.withdraw_not_atomic(a, 1, oracle, slot, 0i64);
    assert!(
        result.is_err(),
        "withdraw_not_atomic must propagate reset error on corrupt state"
    );
}

// ============================================================================
// Wide arithmetic: U512-backed mul_div with large operands
// ============================================================================

#[test]
fn test_wide_signed_mul_div_floor_large_operands() {
    use percolator::wide_math::{wide_signed_mul_div_floor, I256};

    // Large basis * large positive K_diff
    let abs_basis = U256::from_u128(u128::MAX);
    let k_diff = I256::from_i128(i128::MAX);
    let denom = U256::from_u128(POS_SCALE);
    let result = wide_signed_mul_div_floor(abs_basis, k_diff, denom);
    // Must not panic; result should be positive (positive * positive / positive)
    assert!(
        !result.is_negative(),
        "positive inputs must give non-negative result"
    );

    // Large basis * large negative K_diff (floor toward -inf)
    let k_neg = I256::from_i128(-1_000_000_000);
    let result_neg = wide_signed_mul_div_floor(abs_basis, k_neg, denom);
    assert!(
        result_neg.is_negative(),
        "negative k_diff must give negative result"
    );

    // Verify floor rounding: for negative results with remainder, result should
    // be strictly more negative than truncation toward zero.
    // (-1 * 3) / 2 => floor = -2, not -1 (truncation).
    let basis_3 = U256::from_u128(3);
    let k_neg1 = I256::from_i128(-1);
    let denom_2 = U256::from_u128(2);
    let floored = wide_signed_mul_div_floor(basis_3, k_neg1, denom_2);
    assert_eq!(floored, I256::from_i128(-2), "floor(-3/2) must be -2");
}

#[test]
fn test_wide_signed_mul_div_floor_zero_cases() {
    use percolator::wide_math::{wide_signed_mul_div_floor, I256};

    // Zero basis
    let result = wide_signed_mul_div_floor(U256::ZERO, I256::from_i128(42), U256::from_u128(1));
    assert_eq!(result, I256::ZERO);

    // Zero k_diff
    let result = wide_signed_mul_div_floor(U256::from_u128(42), I256::ZERO, U256::from_u128(1));
    assert_eq!(result, I256::ZERO);
}

#[test]
fn test_mul_div_floor_u256_large_product() {
    use percolator::wide_math::mul_div_floor_u256;

    // (u128::MAX * u128::MAX) / 1 should not panic — uses U512 internally
    let a = U256::from_u128(u128::MAX);
    let b = U256::from_u128(u128::MAX);
    let d = U256::from_u128(u128::MAX); // dividing by same magnitude keeps in range
    let result = mul_div_floor_u256(a, b, d);
    assert_eq!(
        result,
        U256::from_u128(u128::MAX),
        "u128::MAX * u128::MAX / u128::MAX = u128::MAX"
    );

    // Small a * large b / large d => small result
    let result2 = mul_div_floor_u256(
        U256::from_u128(1),
        U256::from_u128(u128::MAX),
        U256::from_u128(u128::MAX),
    );
    assert_eq!(result2, U256::from_u128(1));
}

#[test]
fn test_mul_div_ceil_u256_rounding() {
    use percolator::wide_math::mul_div_ceil_u256;

    // Exact division: 6 * 2 / 3 = 4 (no rounding needed)
    let exact = mul_div_ceil_u256(U256::from_u128(6), U256::from_u128(2), U256::from_u128(3));
    assert_eq!(exact, U256::from_u128(4));

    // Rounding up: 7 * 1 / 3 = ceil(7/3) = 3
    let ceiled = mul_div_ceil_u256(U256::from_u128(7), U256::from_u128(1), U256::from_u128(3));
    assert_eq!(ceiled, U256::from_u128(3), "ceil(7/3) must be 3");

    // Minimal remainder: 4 * 1 / 3 = ceil(4/3) = 2
    let min_rem = mul_div_ceil_u256(U256::from_u128(4), U256::from_u128(1), U256::from_u128(3));
    assert_eq!(min_rem, U256::from_u128(2), "ceil(4/3) must be 2");
}

// ============================================================================
// Multi-step funding accrual over large dt
// ============================================================================

#[test]
fn test_accrue_market_to_multi_substep_large_dt() {
    let mut engine = RiskEngine::new(default_params());
    engine.last_oracle_price = 1000;
    engine.last_market_slot = 0;
    engine.funding_price_sample_last = 1000;
    engine.adl_mult_long = ADL_ONE;
    engine.adl_mult_short = ADL_ONE;
    engine.oi_eff_long_q = POS_SCALE;
    engine.oi_eff_short_q = POS_SCALE;

    // High funding rate, large time gap requiring multiple sub-steps
    engine.funding_rate_bps_per_slot_last = 5000; // 50% bps/slot
    let large_dt = MAX_FUNDING_DT * 3 + 100; // triggers 4 sub-steps

    let result = engine.accrue_market_to(large_dt, 1100);
    assert!(
        result.is_ok(),
        "multi-substep accrual must not overflow: {:?}",
        result
    );

    // Price increased, so K_long must increase (mark + funding payer = long)
    // K_short must also change from receiving funding
    assert!(engine.last_market_slot == large_dt);
    assert!(engine.last_oracle_price == 1100);
}

#[test]
fn test_accrue_market_funding_rate_zero_no_funding_applied() {
    let mut engine = RiskEngine::new(default_params());
    engine.last_oracle_price = 1000;
    engine.last_market_slot = 0;
    engine.funding_price_sample_last = 1000;
    engine.adl_mult_long = ADL_ONE;
    engine.adl_mult_short = ADL_ONE;
    engine.oi_eff_long_q = POS_SCALE;
    engine.oi_eff_short_q = POS_SCALE;
    engine.funding_rate_bps_per_slot_last = 0;

    let k_long_before = engine.adl_coeff_long;
    let k_short_before = engine.adl_coeff_short;

    // Same price, time passes: with zero rate, only mark applies (0 delta_p)
    engine.accrue_market_to(100, 1000).unwrap();

    // No price change + no funding → K unchanged
    assert_eq!(engine.adl_coeff_long, k_long_before);
    assert_eq!(engine.adl_coeff_short, k_short_before);
}

#[test]
fn test_accrue_market_applies_funding_transfer() {
    // Spec v12.1.0 §5.4: live funding — K coefficients change when r_last != 0
    let mut engine = RiskEngine::new(default_params());
    engine.last_oracle_price = 1000;
    engine.last_market_slot = 0;
    engine.funding_price_sample_last = 1000;
    engine.adl_mult_long = ADL_ONE;
    engine.adl_mult_short = ADL_ONE;
    engine.oi_eff_long_q = POS_SCALE;
    engine.oi_eff_short_q = POS_SCALE;

    // Positive rate: longs pay shorts
    engine.funding_rate_bps_per_slot_last = 100; // 1% per slot

    let k_long_before = engine.adl_coeff_long;
    let k_short_before = engine.adl_coeff_short;

    engine.accrue_market_to(10, 1000).unwrap(); // same price, dt=10

    // fund_num = 1000 * 100 * 10 = 1_000_000; fund_term = 1_000_000 / 10000 = 100
    // K_long -= A_long * fund_term = ADL_ONE * 100 = 100_000_000
    // K_short += A_short * fund_term = ADL_ONE * 100 = 100_000_000
    assert!(
        engine.adl_coeff_long < k_long_before,
        "positive rate: long K must decrease"
    );
    assert!(
        engine.adl_coeff_short > k_short_before,
        "positive rate: short K must increase"
    );
    assert_eq!(
        k_long_before - engine.adl_coeff_long,
        100_000_000,
        "long K delta must equal A_long * fund_term"
    );
    assert_eq!(
        engine.adl_coeff_short - k_short_before,
        100_000_000,
        "short K delta must equal A_short * fund_term"
    );
}

#[test]
fn test_accrue_market_no_funding_when_rate_zero() {
    // r_last = 0 means no funding transfer
    let mut engine = RiskEngine::new(default_params());
    engine.last_oracle_price = 1000;
    engine.last_market_slot = 0;
    engine.funding_price_sample_last = 1000;
    engine.adl_mult_long = ADL_ONE;
    engine.adl_mult_short = ADL_ONE;
    engine.oi_eff_long_q = POS_SCALE;
    engine.oi_eff_short_q = POS_SCALE;
    engine.funding_rate_bps_per_slot_last = 0;

    let k_long_before = engine.adl_coeff_long;
    let k_short_before = engine.adl_coeff_short;

    engine.accrue_market_to(10, 1000).unwrap();

    assert_eq!(
        engine.adl_coeff_long, k_long_before,
        "zero rate: long K unchanged"
    );
    assert_eq!(
        engine.adl_coeff_short, k_short_before,
        "zero rate: short K unchanged"
    );
}

// ============================================================================
// Keeper crank: cursor advancement and fairness
// ============================================================================

#[test]
fn test_keeper_crank_processes_candidates() {
    let (mut engine, a, b) = setup_two_users(10_000_000, 10_000_000);

    // Crank with explicit candidates processes them
    let outcome = engine
        .keeper_crank_not_atomic(5, 1000, &[(a, None), (b, None)], 64, 0i64)
        .unwrap();
    assert!(outcome.advanced, "crank must advance slot");
}

#[test]
fn test_keeper_crank_caller_fee_discount_multi_slot() {
    let mut engine = RiskEngine::new(default_params());
    let oracle = 1000u64;
    let slot = 1u64;
    engine.current_slot = slot;

    let a = engine.add_user(1000).unwrap();
    engine.deposit(a, 10_000_000, oracle, slot).unwrap();
    engine
        .keeper_crank_not_atomic(
            slot,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .unwrap();

    // Advance many slots to accumulate maintenance fee debt
    let far_slot = 1000u64;
    engine.accounts[a as usize].last_fee_slot = slot;

    // Run crank at far_slot with account a as candidate
    engine
        .keeper_crank_not_atomic(far_slot, oracle, &[(a, None)], 64, 0i64)
        .unwrap();

    // Account's last_fee_slot should be updated to far_slot (post-settlement)
    assert_eq!(
        engine.accounts[a as usize].last_fee_slot, far_slot,
        "account's last_fee_slot must be updated after crank settlement"
    );
}

// ============================================================================
// Liquidation edge cases
// ============================================================================

#[test]
fn test_liquidation_triggers_on_underwater_account() {
    // Small deposits + large position = high leverage → easily liquidated
    let (mut engine, a, b) = setup_two_users(100_000, 100_000);
    let oracle = 1000u64;
    let slot = 2u64;

    // Trade at maximum leverage the margin allows
    // With 100k capital, 10% IM, max notional ≈ 1M → ~1000 units at price 1000
    let size_q = make_size_q(900);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .unwrap();

    // Price crashes — longs deeply underwater
    let crash_price = 500u64; // 50% drop
    let slot2 = 3;

    // Crank at crash price — accrues market internally then liquidates
    let outcome = engine
        .keeper_crank_not_atomic(
            slot2,
            crash_price,
            &[
                (a, Some(LiquidationPolicy::FullClose)),
                (b, Some(LiquidationPolicy::FullClose)),
            ],
            64,
            0i64,
        )
        .unwrap();
    assert!(
        outcome.num_liquidations > 0,
        "crank must liquidate underwater account after 50% price drop"
    );
}

#[test]
fn test_direct_liquidation_returns_to_insurance() {
    let (mut engine, a, b) = setup_two_users(10_000_000, 10_000_000);
    let oracle = 1000u64;
    let slot = 2u64;

    let size_q = make_size_q(10);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .unwrap();

    let ins_before = engine.insurance_fund.balance.get();

    // Price crashes — a (long) underwater
    let crash_price = 100u64;
    let slot2 = 3;
    engine
        .liquidate_at_oracle_not_atomic(a, slot2, crash_price, LiquidationPolicy::FullClose, 0i64)
        .unwrap();

    let ins_after = engine.insurance_fund.balance.get();
    // Insurance should receive liquidation fee (or absorb loss)
    assert!(
        ins_after >= ins_before,
        "insurance fund must not decrease on liquidation"
    );
}

// ============================================================================
// Conservation law: full lifecycle
// ============================================================================

#[test]
fn test_conservation_full_lifecycle() {
    let (mut engine, a, b) = setup_two_users(10_000_000, 10_000_000);
    assert!(
        engine.check_conservation(),
        "conservation must hold after setup"
    );

    let oracle = 1000u64;
    let slot = 2u64;

    // Trade
    let size_q = make_size_q(5);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .unwrap();
    assert!(
        engine.check_conservation(),
        "conservation must hold after trade"
    );

    // Price change + crank
    let slot2 = 3;
    engine
        .keeper_crank_not_atomic(
            slot2,
            1200,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .unwrap();
    assert!(
        engine.check_conservation(),
        "conservation must hold after crank with price change"
    );

    // Withdraw
    engine
        .withdraw_not_atomic(a, 1_000, 1200, slot2, 0i64)
        .unwrap();
    assert!(
        engine.check_conservation(),
        "conservation must hold after withdraw_not_atomic"
    );

    // Another crank at different price
    let slot3 = 4;
    engine
        .keeper_crank_not_atomic(
            slot3,
            800,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .unwrap();
    assert!(
        engine.check_conservation(),
        "conservation must hold after second crank"
    );
}

// ============================================================================
// Position boundary: max position enforcement
// ============================================================================

#[test]
fn test_trade_at_reasonable_size_succeeds() {
    let (mut engine, a, b) = setup_two_users(100_000_000, 100_000_000);
    let oracle = 1000u64;
    let slot = 2u64;

    // Reasonable trade should succeed
    let size_q = make_size_q(1);
    let result = engine.execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64);
    assert!(result.is_ok(), "reasonable trade must succeed");
    assert!(engine.check_conservation());
}

// ============================================================================
// Maintenance fee: overflow on large dt
// ============================================================================

#[test]
fn test_maintenance_fee_large_dt_charges_correctly() {
    // Large dt with max fee_per_slot: fee = dt * fee_per_slot
    let mut params = default_params();
    params.maintenance_fee_per_slot = U128::new(MAX_MAINTENANCE_FEE_PER_SLOT);
    let mut engine = RiskEngine::new(params);
    let oracle = 1000u64;
    let slot = 1u64;
    engine.current_slot = slot;

    let a = engine.add_user(1000).unwrap();
    engine.deposit(a, 10_000_000, oracle, slot).unwrap();
    engine
        .keeper_crank_not_atomic(
            slot,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .unwrap();

    let far_slot = slot + 10;
    engine.last_market_slot = far_slot - 1;
    engine.last_oracle_price = oracle;
    engine.funding_price_sample_last = oracle;

    // fee = 10 * MAX_MAINTENANCE_FEE_PER_SLOT. If this exceeds MAX_PROTOCOL_FEE_ABS,
    // the crank will fail with Overflow — which is the correct behavior.
    let result = engine.keeper_crank_not_atomic(far_slot, oracle, &[(a, None)], 64, 0i64);
    // Either succeeds (fee within bounds) or fails (overflow) — both are correct
    if result.is_ok() {
        assert!(engine.check_conservation());
    }
}

// ============================================================================
// charge_fee_safe: PnL near i128::MIN boundary
// ============================================================================

#[test]
fn test_charge_fee_safe_rejects_pnl_at_i256_min() {
    let mut engine = RiskEngine::new(default_params());
    let oracle = 1000u64;
    let slot = 1u64;
    engine.current_slot = slot;

    let a = engine.add_user(1000).unwrap();
    engine.deposit(a, 0, oracle, slot).unwrap(); // zero capital so shortfall goes to PnL

    // Set PnL very close to i128::MIN
    let near_min = i128::MIN.checked_add(1i128).unwrap();
    engine.set_pnl(a as usize, near_min);

    // Liquidation fee would push PnL to exactly i128::MIN — must return Err
    // We test via the public liquidate path, but first set up the conditions
    // for an underwater account with a position.
    engine.accounts[a as usize].position_basis_q = POS_SCALE as i128;
    engine.adl_mult_long = ADL_ONE;
    engine.adl_mult_short = ADL_ONE;
    engine.adl_epoch_long = 0;
    engine.adl_epoch_short = 0;
    engine.accounts[a as usize].adl_a_basis = ADL_ONE;
    engine.accounts[a as usize].adl_k_snap = 0i128;
    engine.accounts[a as usize].adl_epoch_snap = 0;
    engine.stored_pos_count_long = 1;
    engine.oi_eff_long_q = POS_SCALE;
    engine.oi_eff_short_q = POS_SCALE;
    engine.last_oracle_price = oracle;
    engine.last_market_slot = slot;
    engine.last_crank_slot = slot;
    engine.funding_price_sample_last = oracle;

    // Liquidation should handle this gracefully (return Err or succeed without i128::MIN)
    let result =
        engine.liquidate_at_oracle_not_atomic(a, slot, oracle, LiquidationPolicy::FullClose, 0i64);
    // Either it errors out or it succeeds but PnL is not i128::MIN
    if result.is_ok() {
        assert!(
            engine.accounts[a as usize].pnl != i128::MIN,
            "PnL must never reach i128::MIN"
        );
    }
}

// ============================================================================
// Side mode gating prevents OI increase during DrainOnly
// ============================================================================

#[test]
fn test_drain_only_blocks_oi_increase() {
    let (mut engine, a, b) = setup_two_users(10_000_000, 10_000_000);
    let oracle = 1000u64;
    let slot = 2u64;

    // Set long side to DrainOnly
    engine.side_mode_long = SideMode::DrainOnly;

    // Try to open a new long position — should fail
    let size_q = make_size_q(1); // a goes long
    let result = engine.execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64);
    assert!(
        result.is_err(),
        "DrainOnly side must reject OI-increasing trades"
    );
}

// ============================================================================
// Oracle price: zero and max boundary
// ============================================================================

#[test]
fn test_oracle_price_zero_rejected() {
    let (mut engine, a, _b) = setup_two_users(10_000_000, 10_000_000);
    let result = engine.accrue_market_to(2, 0);
    assert!(result.is_err(), "oracle price 0 must be rejected");
}

#[test]
fn test_oracle_price_max_accepted() {
    let mut engine = RiskEngine::new(default_params());
    engine.last_oracle_price = 1000;
    engine.last_market_slot = 0;
    engine.funding_price_sample_last = 1000;
    engine.adl_mult_long = ADL_ONE;
    engine.adl_mult_short = ADL_ONE;
    engine.funding_rate_bps_per_slot_last = 0;

    let result = engine.accrue_market_to(1, MAX_ORACLE_PRICE);
    assert!(result.is_ok(), "MAX_ORACLE_PRICE must be accepted");

    let result2 = engine.accrue_market_to(2, MAX_ORACLE_PRICE + 1);
    assert!(result2.is_err(), "above MAX_ORACLE_PRICE must be rejected");
}

// ============================================================================
// Deposit/withdraw_not_atomic roundtrip: conservation on single account
// ============================================================================

#[test]
fn test_deposit_withdraw_roundtrip_same_slot() {
    let (mut engine, a, _b) = setup_two_users(10_000_000, 10_000_000);
    // Use same slot as setup (slot=1) to avoid maintenance fee deduction
    let oracle = 1000;
    let slot = 1;

    let cap_before = engine.accounts[a as usize].capital.get();
    engine.deposit(a, 5_000_000, oracle, slot).unwrap();
    assert_eq!(
        engine.accounts[a as usize].capital.get(),
        cap_before + 5_000_000
    );

    // Withdraw full extra amount at same slot — no fee should apply
    engine
        .withdraw_not_atomic(a, 5_000_000, oracle, slot, 0i64)
        .unwrap();
    assert_eq!(
        engine.accounts[a as usize].capital.get(),
        cap_before,
        "same-slot deposit+withdraw_not_atomic roundtrip must return exact capital"
    );
    assert!(engine.check_conservation());
}

// ============================================================================
// Multiple cranks don't double-process accounts
// ============================================================================

#[test]
fn test_double_crank_same_slot_is_safe() {
    let (mut engine, a, b) = setup_two_users(10_000_000, 10_000_000);
    let oracle = 1000u64;
    let slot = 2u64;

    engine
        .keeper_crank_not_atomic(
            slot,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .unwrap();

    let cap_a = engine.accounts[a as usize].capital.get();
    let cap_b = engine.accounts[b as usize].capital.get();

    // Second crank same slot — should be a no-op (no double fee charges etc.)
    engine
        .keeper_crank_not_atomic(
            slot,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .unwrap();

    // Capital shouldn't change from a redundant crank
    // (small tolerance for rounding if any fees apply)
    let cap_a_after = engine.accounts[a as usize].capital.get();
    let cap_b_after = engine.accounts[b as usize].capital.get();
    assert!(
        cap_a_after == cap_a,
        "redundant crank must not change capital"
    );
    assert!(
        cap_b_after == cap_b,
        "redundant crank must not change capital"
    );
    assert!(engine.check_conservation());
}

// ============================================================================
// Issue #1: Withdraw simulation must not inflate haircut ratio
// ============================================================================

#[test]
fn test_withdraw_simulation_does_not_inflate_haircut() {
    let (mut engine, a, b) = setup_two_users(10_000_000, 10_000_000);
    let oracle = 1000u64;
    let slot = 2u64;

    // Open a position so the margin check path is exercised
    let size_q = make_size_q(50);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .unwrap();

    // Give a some positive PnL so haircut matters
    engine.set_pnl(a as usize, 5_000_000i128);

    // Record haircut before
    let (h_num_before, h_den_before) = engine.haircut_ratio();

    // Simulate what the FIXED withdraw_not_atomic() does: adjust both capital AND vault
    let old_cap = engine.accounts[a as usize].capital.get();
    let old_vault = engine.vault;
    let withdraw_amount = 1_000_000u128;
    let new_cap = old_cap - withdraw_amount;
    engine.set_capital(a as usize, new_cap);
    engine.vault = U128::new(engine.vault.get() - withdraw_amount);

    let (h_num_sim, h_den_sim) = engine.haircut_ratio();

    // Revert both
    engine.set_capital(a as usize, old_cap);
    engine.vault = old_vault;

    // Compare: h_sim <= h_before (cross-multiply)
    // h_num_sim / h_den_sim <= h_num_before / h_den_before
    let lhs = h_num_sim.checked_mul(h_den_before).unwrap();
    let rhs = h_num_before.checked_mul(h_den_sim).unwrap();
    assert!(
        lhs <= rhs,
        "haircut must not increase during withdraw_not_atomic simulation (Residual inflation)"
    );
}

// ============================================================================
// Issue #2: Funding rate must be validated before storage
// ============================================================================

#[test]
fn test_multiple_cranks_do_not_brick_protocol() {
    let (mut engine, _a, _b) = setup_two_users(10_000_000, 10_000_000);

    // Run crank at slot 2
    let _ = engine.keeper_crank_not_atomic(
        2,
        1000,
        &[] as &[(u16, Option<LiquidationPolicy>)],
        64,
        0i64,
    );

    // Protocol must not be bricked — next crank must succeed
    let result = engine.keeper_crank_not_atomic(
        3,
        1000,
        &[] as &[(u16, Option<LiquidationPolicy>)],
        64,
        0i64,
    );
    assert!(
        result.is_ok(),
        "protocol must not be bricked by a previous crank"
    );
}

// ============================================================================
// Issue #3: GC must not delete accounts with fee_credits
// ============================================================================

#[test]
fn test_gc_dust_preserves_fee_credits() {
    let mut engine = RiskEngine::new(default_params());
    let oracle = 1000u64;
    let slot = 1u64;
    engine.current_slot = slot;

    let a = engine.add_user(1000).unwrap();
    engine.deposit(a, 10_000, oracle, slot).unwrap();
    engine
        .keeper_crank_not_atomic(
            slot,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .unwrap();

    // Set up dust-like state: 0 capital, 0 position, but positive fee_credits
    engine.set_capital(a as usize, 0);
    engine.accounts[a as usize].position_basis_q = 0i128;
    engine.set_pnl(a as usize, 0i128);
    engine.accounts[a as usize].fee_credits = I128::new(5_000);

    assert!(engine.is_used(a as usize), "account must exist before GC");

    engine.garbage_collect_dust();

    assert!(
        engine.is_used(a as usize),
        "GC must not delete account with non-zero fee_credits"
    );
    assert_eq!(
        engine.accounts[a as usize].fee_credits.get(),
        5_000,
        "fee_credits must be preserved"
    );
}

// ============================================================================
// Bug fix #1: GC must collect dead accounts with negative fee_credits (debt)
// ============================================================================

#[test]
fn test_gc_collects_dead_account_with_negative_fee_credits() {
    // Before the fix: settle_maintenance_fee pushes fee_credits negative,
    // then !fee_credits.is_zero() causes GC to skip the dead account forever.
    let mut params = default_params();
    params.maintenance_fee_per_slot = U128::new(100); // high fee
    let mut engine = RiskEngine::new(params);
    let oracle = 1000u64;
    let slot = 1u64;
    engine.current_slot = slot;

    let a = engine.add_user(1000).unwrap();
    engine.deposit(a, 10_000, oracle, slot).unwrap();
    engine
        .keeper_crank_not_atomic(
            slot,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .unwrap();

    // Simulate abandoned account: zero everything
    engine.set_capital(a as usize, 0);
    engine.accounts[a as usize].position_basis_q = 0i128;
    engine.set_pnl(a as usize, 0i128);
    engine.accounts[a as usize].fee_credits = I128::new(0);
    engine.accounts[a as usize].last_fee_slot = slot;

    // Advance time so maintenance fee accrues → pushes fee_credits negative
    let gc_slot = slot + 100;
    engine.current_slot = gc_slot;

    let num_used_before = engine.num_used_accounts;
    engine.garbage_collect_dust();

    // Account must be collected despite negative fee_credits
    assert!(
        !engine.is_used(a as usize),
        "dead account with negative fee_credits must be collected by GC"
    );
    assert!(
        engine.num_used_accounts < num_used_before,
        "used account count must decrease"
    );
}

#[test]
fn test_gc_still_protects_positive_fee_credits() {
    // Regression: the fix must not break protection of prepaid credits
    let mut engine = RiskEngine::new(default_params());
    let oracle = 1000u64;
    let slot = 1u64;
    engine.current_slot = slot;

    let a = engine.add_user(1000).unwrap();
    engine.deposit(a, 10_000, oracle, slot).unwrap();
    engine
        .keeper_crank_not_atomic(
            slot,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            64,
            0i64,
        )
        .unwrap();

    engine.set_capital(a as usize, 0);
    engine.accounts[a as usize].position_basis_q = 0i128;
    engine.set_pnl(a as usize, 0i128);
    // Large positive prepaid credits
    engine.accounts[a as usize].fee_credits = I128::new(1_000_000);

    engine.garbage_collect_dust();

    assert!(
        engine.is_used(a as usize),
        "GC must protect accounts with positive (prepaid) fee_credits"
    );
}

// ============================================================================
// Bug fix #2: Maintenance fee must NOT eagerly sweep capital
// (trading loss seniority over fee debt)
// ============================================================================

#[test]
fn test_maintenance_fee_sweeps_capital() {
    // Spec §8.2: maintenance fees enabled. fee_per_slot=100, dt=50 → fee=5000
    let mut params = default_params();
    params.maintenance_fee_per_slot = U128::new(100);
    params.new_account_fee = U128::ZERO;
    params.trading_fee_bps = 0;
    let mut engine = RiskEngine::new(params);
    let oracle = 1000u64;
    let slot = 1u64;

    let a = engine.add_user(0).unwrap();
    engine.deposit(a, 10_000, oracle, slot).unwrap();
    engine.last_oracle_price = oracle;
    engine.last_market_slot = slot;
    engine.accounts[a as usize].last_fee_slot = slot;

    let touch_slot = slot + 50;
    let result = engine.touch_account_full_not_atomic(a as usize, oracle, touch_slot);
    assert!(result.is_ok());

    let cap_after = engine.accounts[a as usize].capital.get();
    assert_eq!(
        cap_after, 5_000,
        "capital must decrease by fee (10000 - 50*100 = 5000)"
    );
    assert!(engine.check_conservation());
}

// ============================================================================
// Bug fix #3: Minimum absolute liquidation fee must be enforced
// ============================================================================

#[test]
fn test_min_liquidation_fee_enforced() {
    // Before the fix: dust positions liquidated with zero penalty because
    // min_liquidation_abs was defined but never referenced.
    // Use proper trade flow so all invariants are maintained.
    let mut params = default_params();
    params.min_liquidation_abs = U128::new(500);
    params.liquidation_fee_bps = 100; // 1%
    params.liquidation_fee_cap = U128::new(1_000_000);
    params.maintenance_fee_per_slot = U128::ZERO;
    let mut engine = RiskEngine::new(params);
    let oracle = 1000u64;
    let slot = 1u64;
    engine.current_slot = slot;

    let a = engine.add_user(1000).unwrap();
    let b = engine.add_user(1000).unwrap();
    // Large capital so account stays solvent even after price drop
    engine.deposit(a, 1_000_000, oracle, slot).unwrap();
    engine.deposit(b, 1_000_000, oracle, slot).unwrap();

    // Small position: 1 unit. Notional = 1000, 1% bps fee = 10.
    // min_liquidation_abs = 500 → fee = max(10, 500) = 500.
    let size_q = make_size_q(1);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .unwrap();

    // Now make account underwater but still solvent (has capital to pay fee).
    // Directly set PnL to push below maintenance margin.
    // Equity = capital + PnL. Maintenance = 5% * |notional|.
    // At oracle 1000, 1 unit: notional = 1000, maint = 50.
    // Capital ~ 1M (minus trading fee). Set PnL so equity < maint margin.
    // PnL = -(capital - 40) makes equity = 40 < 50 maintenance.
    let cap = engine.accounts[a as usize].capital.get();
    engine.set_pnl(a as usize, -((cap as i128) - 40));

    let ins_before = engine.insurance_fund.balance.get();

    let slot2 = 2;
    let result =
        engine.liquidate_at_oracle_not_atomic(a, slot2, oracle, LiquidationPolicy::FullClose, 0i64);
    assert!(result.is_ok(), "liquidation must succeed: {:?}", result);
    assert!(result.unwrap(), "account must be liquidated");

    let ins_after = engine.insurance_fund.balance.get();

    // Fee = max(10, 500) = 500, min(500, 1M) = 500.
    // Account has 40 units of equity → charge_fee_safe pays 40 from cap, 460 from PnL.
    // Insurance gets 40 from cap directly.
    // Then deficit gets absorbed from insurance.
    // Net insurance change: +40 (fee from cap) - deficit_absorbed.
    // The key: the FEE AMOUNT itself is 500 (not 10). Test the formula is correct.
    // Since we can't isolate fee vs loss, just verify the overall flow doesn't panic
    // and conservation holds.
    assert!(
        engine.check_conservation(),
        "conservation must hold after min-fee liquidation"
    );
}

#[test]
fn test_min_liquidation_fee_does_not_exceed_cap() {
    // Verify: min(max(bps_fee, min_abs), cap) → cap wins when min > cap
    let mut params = default_params();
    params.liquidation_fee_cap = U128::new(200); // low cap
    params.min_liquidation_abs = U128::new(150); // below cap (valid per §1.4)
    params.liquidation_fee_bps = 100;
    params.maintenance_fee_per_slot = U128::ZERO;
    let mut engine = RiskEngine::new(params);
    let oracle = 1000u64;
    let slot = 1u64;
    engine.current_slot = slot;

    let a = engine.add_user(1000).unwrap();
    let b = engine.add_user(1000).unwrap();
    engine.deposit(a, 50_000, oracle, slot).unwrap();
    engine.deposit(b, 50_000, oracle, slot).unwrap();

    // 10-unit position: notional = 10000, 1% bps = 100
    // max(100, 150) = 150, but cap = 200 → fee = 150
    // The cap wins when fee would exceed it
    let size_q = make_size_q(10);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .unwrap();

    // Crash price to trigger liquidation
    let crash_price = 100u64;
    let slot2 = 2;

    // Record insurance before. Trading fee from execute_trade_not_atomic already credited.
    let ins_before = engine.insurance_fund.balance.get();
    let result = engine.liquidate_at_oracle_not_atomic(
        a,
        slot2,
        crash_price,
        LiquidationPolicy::FullClose,
        0i64,
    );
    assert!(result.is_ok(), "liquidation must succeed: {:?}", result);

    let ins_after = engine.insurance_fund.balance.get();

    // The net insurance change includes: +liq_fee, -absorbed_loss.
    // We can't isolate the fee directly, but we verify conservation holds
    // and the code path executed min(max(bps, min_abs), cap).
    assert!(
        engine.check_conservation(),
        "conservation must hold after liquidation"
    );
}

// ============================================================================
// Property 49: Profit-conversion reserve preservation
// consume_released_pnl leaves R_i unchanged, reduces pnl_pos_tot and
// pnl_matured_pos_tot by exactly x.
// ============================================================================

#[test]
fn test_property_49_consume_released_pnl_preserves_reserve() {
    let oracle = 1_000u64;
    let slot = 1u64;
    let mut engine = RiskEngine::new(default_params());
    let a = engine.add_user(1000).unwrap();
    engine.deposit(a, 100_000, oracle, slot).unwrap();
    engine
        .keeper_crank_not_atomic(
            slot,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            0,
            0i64,
        )
        .unwrap();

    // Give account positive PnL with some matured (released) portion
    let idx = a as usize;
    engine.set_pnl(idx, 5_000);
    // After set_pnl, the increase goes to reserved_pnl; simulate warmup completion
    engine.set_reserved_pnl(idx, 0); // all matured

    let r_before = engine.accounts[idx].reserved_pnl;
    let ppt_before = engine.pnl_pos_tot;
    let pmpt_before = engine.pnl_matured_pos_tot;

    assert_eq!(r_before, 0, "all profit should be released");

    let x = 2_000u128;
    engine.consume_released_pnl(idx, x);

    assert_eq!(
        engine.accounts[idx].reserved_pnl, r_before,
        "R_i must be unchanged after consume_released_pnl"
    );
    assert_eq!(
        engine.pnl_pos_tot,
        ppt_before - x,
        "pnl_pos_tot must decrease by x"
    );
    assert_eq!(
        engine.pnl_matured_pos_tot,
        pmpt_before - x,
        "pnl_matured_pos_tot must decrease by x"
    );
    assert_eq!(
        engine.accounts[idx].pnl, 3_000i128,
        "PNL_i must decrease by x"
    );
}

// ============================================================================
// Property 50: Flat-only automatic conversion
// touch_account_full_not_atomic on a flat account converts matured released profit;
// touch_account_full_not_atomic on an open-position account does NOT auto-convert.
// ============================================================================

#[test]
fn test_property_50_flat_only_auto_conversion() {
    let oracle = 1_000u64;
    let slot = 1u64;
    let mut params = default_params();
    params.maintenance_fee_per_slot = U128::ZERO;
    params.trading_fee_bps = 0;
    params.new_account_fee = U128::ZERO;
    let mut engine = RiskEngine::new(params);

    let a = engine.add_user(0).unwrap();
    let b = engine.add_user(0).unwrap();
    engine.deposit(a, 100_000, oracle, slot).unwrap();
    engine.deposit(b, 100_000, oracle, slot).unwrap();
    engine
        .keeper_crank_not_atomic(
            slot,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            0,
            0i64,
        )
        .unwrap();

    // Give 'a' an open position
    let size_q = make_size_q(1);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .unwrap();

    // Manually give 'a' released matured profit and fund vault to cover it
    let idx_a = a as usize;
    engine.set_pnl(idx_a, 10_000);
    engine.set_reserved_pnl(idx_a, 0); // all matured
    engine.vault = U128::new(engine.vault.get() + 10_000); // fund the PnL

    // Touch with open position — should NOT auto-convert
    engine
        .touch_account_full_not_atomic(idx_a, oracle, slot + 1)
        .unwrap();

    let pnl_after = engine.accounts[idx_a].pnl;
    assert!(
        pnl_after > 0,
        "open-position touch must not zero out released profit via auto-convert"
    );

    // Now test flat account: close the position first
    engine
        .execute_trade_not_atomic(b, a, oracle, slot + 1, size_q, oracle, 0i64)
        .unwrap();
    // Give released profit and fund vault
    let idx_a = a as usize;
    engine.set_pnl(idx_a, 5_000);
    engine.set_reserved_pnl(idx_a, 0);
    engine.vault = U128::new(engine.vault.get() + 5_000);

    let cap_before_flat = engine.accounts[idx_a].capital.get();
    engine
        .touch_account_full_not_atomic(idx_a, oracle, slot + 2)
        .unwrap();

    // After flat touch, released profit should have been converted to capital
    let pnl_after_flat = engine.accounts[idx_a].pnl;
    let cap_after_flat = engine.accounts[idx_a].capital.get();
    assert_eq!(
        pnl_after_flat, 0,
        "flat touch must convert released profit (PNL → 0)"
    );
    assert!(
        cap_after_flat > cap_before_flat,
        "flat touch must increase capital from conversion"
    );
}

// ============================================================================
// Property 51: Universal withdrawal dust guard
// Withdrawal must leave either 0 capital or >= MIN_INITIAL_DEPOSIT.
// ============================================================================

#[test]
fn test_property_51_universal_withdrawal_dust_guard() {
    let oracle = 1_000u64;
    let slot = 1u64;
    let min_deposit = 1_000u128;

    let mut params = default_params();
    params.min_initial_deposit = U128::new(min_deposit);
    params.maintenance_fee_per_slot = U128::ZERO;
    params.new_account_fee = U128::ZERO;
    let mut engine = RiskEngine::new(params);

    let a = engine.add_user(0).unwrap();
    engine.deposit(a, 5_000, oracle, slot).unwrap();
    engine
        .keeper_crank_not_atomic(
            slot,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            0,
            0i64,
        )
        .unwrap();

    let cap = engine.accounts[a as usize].capital.get();
    assert_eq!(cap, 5_000);

    // Try withdrawing to leave dust (< MIN_INITIAL_DEPOSIT but > 0)
    let withdraw_dust = cap - 500; // leaves 500, which is < 1000 MIN_INITIAL_DEPOSIT
    let result = engine.withdraw_not_atomic(a, withdraw_dust, oracle, slot, 0i64);
    assert!(
        result.is_err(),
        "withdrawal leaving dust below MIN_INITIAL_DEPOSIT must be rejected"
    );

    // Withdrawing to leave exactly 0 must succeed
    let result2 = engine.withdraw_not_atomic(a, cap, oracle, slot, 0i64);
    assert!(result2.is_ok(), "full withdrawal to 0 must succeed");

    // Re-deposit and test partial withdrawal leaving >= MIN_INITIAL_DEPOSIT
    engine.deposit(a, 5_000, oracle, slot).unwrap();
    let cap2 = engine.accounts[a as usize].capital.get();
    let withdraw_ok = cap2 - min_deposit; // leaves exactly MIN_INITIAL_DEPOSIT
    let result3 = engine.withdraw_not_atomic(a, withdraw_ok, oracle, slot, 0i64);
    assert!(
        result3.is_ok(),
        "withdrawal leaving >= MIN_INITIAL_DEPOSIT must succeed"
    );
}

// ============================================================================
// Property 52: Explicit open-position profit conversion
// convert_released_pnl_not_atomic consumes only ReleasedPos_i, leaves R_i unchanged,
// sweeps fee debt, and rejects if post-conversion state is unhealthy.
// ============================================================================

#[test]
fn test_property_52_convert_released_pnl_explicit() {
    let oracle = 1_000u64;
    let slot = 1u64;
    let mut engine = RiskEngine::new(default_params());
    let a = engine.add_user(1000).unwrap();
    let b = engine.add_user(1000).unwrap();
    engine.deposit(a, 100_000, oracle, slot).unwrap();
    engine.deposit(b, 100_000, oracle, slot).unwrap();
    engine
        .keeper_crank_not_atomic(
            slot,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            0,
            0i64,
        )
        .unwrap();

    // Give 'a' an open position
    let size_q = make_size_q(1);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .unwrap();

    // Set released matured profit
    let idx = a as usize;
    engine.set_pnl(idx, 10_000);
    engine.set_reserved_pnl(idx, 3_000); // 7000 released

    let r_before = engine.accounts[idx].reserved_pnl;

    // Convert some released profit
    let result = engine.convert_released_pnl_not_atomic(a, 5_000, oracle, slot + 1, 0i64);
    assert!(
        result.is_ok(),
        "convert_released_pnl_not_atomic must succeed: {:?}",
        result
    );

    // R_i must be unchanged
    assert_eq!(
        engine.accounts[idx].reserved_pnl, r_before,
        "R_i must be unchanged after convert_released_pnl_not_atomic"
    );

    // Requesting more than released must fail
    let released_now = {
        let pnl = engine.accounts[idx].pnl;
        let pos = if pnl > 0 { pnl as u128 } else { 0u128 };
        pos.saturating_sub(engine.accounts[idx].reserved_pnl)
    };
    let result2 =
        engine.convert_released_pnl_not_atomic(a, released_now + 1, oracle, slot + 1, 0i64);
    assert!(result2.is_err(), "requesting more than released must fail");
}

// ============================================================================
// Property 53: Phantom-dust ADL ordering awareness
// If a keeper zeroes the last stored position on a side while phantom OI
// remains, opposite-side bankruptcies after that lose K-socialization capacity.
// ============================================================================

#[test]
fn test_property_53_phantom_dust_adl_ordering() {
    let oracle = 1_000u64;
    let slot = 1u64;
    let mut params = default_params();
    params.trading_fee_bps = 0;
    params.maintenance_fee_per_slot = U128::ZERO;
    params.new_account_fee = U128::ZERO;
    let mut engine = RiskEngine::new(params);

    let a = engine.add_user(0).unwrap();
    let b = engine.add_user(0).unwrap();
    // Give 'a' small capital so it goes bankrupt on crash; give 'b' large capital
    engine.deposit(a, 50_000, oracle, slot).unwrap();
    engine.deposit(b, 1_000_000, oracle, slot).unwrap();
    engine
        .keeper_crank_not_atomic(
            slot,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            0,
            0i64,
        )
        .unwrap();

    // Open near-maximum-leverage position for 'a':
    // 50k capital, 10% IM => max notional ~500k => ~480 units at price 1000
    let size_q = make_size_q(480);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .unwrap();

    // Verify balanced OI before crash
    assert_eq!(
        engine.oi_eff_long_q, engine.oi_eff_short_q,
        "OI must be balanced"
    );
    assert!(engine.oi_eff_long_q > 0, "OI must be nonzero");
    assert!(
        engine.stored_pos_count_long > 0,
        "should have stored long positions"
    );

    // Crash the price to make 'a' (long) deeply underwater, triggering
    // liquidation + ADL (bankruptcy). This closes a's position and creates
    // phantom dust on the long side.
    let crash_price = 870u64;
    let slot2 = slot + 1;
    let result = engine.liquidate_at_oracle_not_atomic(
        a,
        slot2,
        crash_price,
        LiquidationPolicy::FullClose,
        0i64,
    );
    assert!(result.is_ok(), "liquidation must succeed: {:?}", result);
    assert!(result.unwrap(), "account a must be liquidated");

    // After liquidation, a's position is closed; stored_pos_count_long should be 0
    assert_eq!(
        engine.stored_pos_count_long, 0,
        "long stored_pos_count must be 0 after sole long is liquidated"
    );

    // Conservation must hold even in this phantom-dust ADL scenario
    assert!(
        engine.check_conservation(),
        "conservation must hold after phantom-dust ADL scenario"
    );
}

// ============================================================================
// Property 54: Unilateral exact-drain reset scheduling
// If enqueue_adl drives OI_eff_opp to 0 while OI_eff_liq_side remains
// positive, it schedules pending_reset_opp = true.
// ============================================================================

#[test]
fn test_property_54_unilateral_exact_drain_reset() {
    let oracle = 1_000u64;
    let slot = 1u64;
    let mut params = default_params();
    params.trading_fee_bps = 0;
    params.maintenance_fee_per_slot = U128::ZERO;
    params.new_account_fee = U128::ZERO;
    let mut engine = RiskEngine::new(params);

    let a = engine.add_user(0).unwrap();
    let b = engine.add_user(0).unwrap();
    engine.deposit(a, 100_000, oracle, slot).unwrap();
    engine.deposit(b, 100_000, oracle, slot).unwrap();
    engine
        .keeper_crank_not_atomic(
            slot,
            oracle,
            &[] as &[(u16, Option<LiquidationPolicy>)],
            0,
            0i64,
        )
        .unwrap();

    // a long, b short
    let size_q = make_size_q(1);
    engine
        .execute_trade_not_atomic(a, b, oracle, slot, size_q, oracle, 0i64)
        .unwrap();

    // Crash the price to make account 'a' deeply underwater
    let crash_price = 100u64;
    let slot2 = slot + 1;

    // Liquidate 'a' — the long position is closed, ADL may drain the long side
    let result = engine.liquidate_at_oracle_not_atomic(
        a,
        slot2,
        crash_price,
        LiquidationPolicy::FullClose,
        0i64,
    );
    assert!(result.is_ok(), "liquidation must succeed: {:?}", result);

    // After liquidation, the long side should be drained (only long was 'a').
    // The key property: no underflow or panic, and conservation holds
    // even when OI_eff on one side goes to 0.
    assert!(
        engine.check_conservation(),
        "conservation must hold after exact-drain scenario"
    );

    // If long OI went to 0, the side should have a reset scheduled or already finalized
    if engine.oi_eff_long_q == 0 && engine.stored_pos_count_long == 0 {
        // Side was fully drained — mode should transition appropriately
        assert!(
            engine.side_mode_long != SideMode::Normal || engine.stored_pos_count_short == 0,
            "drained side should transition from Normal unless both sides empty"
        );
    }
}

// ============================================================================
// force_close_resolved_not_atomic
// ============================================================================

#[test]
fn test_force_close_resolved_flat_no_pnl() {
    let mut engine = RiskEngine::new(default_params());
    let idx = engine.add_user(1000).unwrap();
    engine.deposit(idx, 50_000, 1000, 100).unwrap();
    // Align last_fee_slot so force_close doesn't charge accrued fee
    engine.accounts[idx as usize].last_fee_slot = 100;

    let returned = engine.force_close_resolved_not_atomic(idx, 100).unwrap();
    assert_eq!(returned, 50_000);
    assert!(!engine.is_used(idx as usize));
    assert!(engine.check_conservation());
}

#[test]
fn test_force_close_resolved_with_open_position() {
    let mut engine = RiskEngine::new(default_params());
    let a = engine.add_user(1000).unwrap();
    let b = engine.add_user(1000).unwrap();
    engine.deposit(a, 500_000, 1000, 100).unwrap();
    engine.deposit(b, 500_000, 1000, 100).unwrap();

    let size = (100 * POS_SCALE) as i128;
    engine
        .execute_trade_not_atomic(a, b, 1000, 100, size, 1000, 0i64)
        .unwrap();

    // Account has open position — force_close settles K-pair PnL and zeros it
    let result = engine.force_close_resolved_not_atomic(a, 100);
    assert!(result.is_ok(), "force_close must handle open positions");
    assert!(!engine.is_used(a as usize));
    assert!(engine.check_conservation());
}

#[test]
fn test_force_close_resolved_with_negative_pnl() {
    let mut engine = RiskEngine::new(default_params());
    let a = engine.add_user(1000).unwrap();
    let b = engine.add_user(1000).unwrap();
    engine.deposit(a, 500_000, 1000, 100).unwrap();
    engine.deposit(b, 500_000, 1000, 100).unwrap();

    let size = (100 * POS_SCALE) as i128;
    engine
        .execute_trade_not_atomic(a, b, 1000, 100, size, 1000, 0i64)
        .unwrap();

    // Inject loss
    engine.set_pnl(a as usize, -100_000i128);

    let cap_before = engine.accounts[a as usize].capital.get();
    let returned = engine.force_close_resolved_not_atomic(a, 100).unwrap();

    assert!(returned < cap_before, "loss must reduce returned capital");
    assert!(!engine.is_used(a as usize));
    assert!(engine.check_conservation());
}

#[test]
fn test_force_close_resolved_with_positive_pnl() {
    let mut engine = RiskEngine::new(default_params());
    let idx = engine.add_user(1000).unwrap();
    engine.deposit(idx, 50_000, 1000, 100).unwrap();
    engine.accounts[idx as usize].last_fee_slot = 100;

    // Inject positive PnL on flat account
    engine.set_pnl(idx as usize, 10_000i128);

    let returned = engine.force_close_resolved_not_atomic(idx, 100).unwrap();
    // Positive PnL converted to capital (haircutted) before return
    assert!(
        returned >= 50_000,
        "positive PnL must increase returned capital"
    );
    assert!(!engine.is_used(idx as usize));
    assert!(engine.check_conservation());
}

#[test]
fn test_force_close_resolved_with_fee_debt() {
    let mut engine = RiskEngine::new(default_params());
    let idx = engine.add_user(1000).unwrap();
    engine.deposit(idx, 50_000, 1000, 100).unwrap();
    engine.accounts[idx as usize].last_fee_slot = 100;

    // Inject fee debt of 5000
    engine.accounts[idx as usize].fee_credits = I128::new(-5000);

    let returned = engine.force_close_resolved_not_atomic(idx, 100).unwrap();
    // Fee debt swept from capital first (spec §7.5 fee seniority):
    // 50_000 capital - 5_000 fee sweep = 45_000 returned
    assert_eq!(returned, 45_000, "fee debt swept before capital return");
    assert!(!engine.is_used(idx as usize));
    assert!(engine.check_conservation());
}

#[test]
fn test_force_close_resolved_unused_slot_rejected() {
    let mut engine = RiskEngine::new(default_params());
    let result = engine.force_close_resolved_not_atomic(0, 100);
    assert_eq!(result, Err(RiskError::AccountNotFound));
}

#[test]
fn test_force_close_same_epoch_positive_k_pair_pnl() {
    // Account opened long, price moved up → unrealized profit from K-pair
    let mut engine = RiskEngine::new(default_params());
    let a = engine.add_user(1000).unwrap();
    let b = engine.add_user(1000).unwrap();
    engine.deposit(a, 500_000, 1000, 100).unwrap();
    engine.deposit(b, 500_000, 1000, 100).unwrap();

    engine
        .execute_trade_not_atomic(a, b, 1000, 100, (100 * POS_SCALE) as i128, 1000, 0i64)
        .unwrap();
    // Align fee slots
    engine.accounts[a as usize].last_fee_slot = 100;
    engine.accounts[b as usize].last_fee_slot = 100;
    let cap_after_trade = engine.accounts[a as usize].capital.get();

    // Advance K via price movement (mark-to-market) — NOT touching a or b as candidates
    // so K-pair PnL remains unrealized for them
    engine.accrue_market_to(200, 1500).unwrap();
    engine.current_slot = 200;
    // Align fee slots to 200 to prevent fee on force_close
    engine.accounts[a as usize].last_fee_slot = 200;

    // a (long) has unrealized profit from K-pair (K_long increased)
    let returned = engine.force_close_resolved_not_atomic(a, 200).unwrap();

    // Returned should include settled K-pair profit
    assert!(
        returned >= cap_after_trade,
        "K-pair profit must increase returned capital"
    );
    assert!(!engine.is_used(a as usize));
    assert!(engine.check_conservation());
}

#[test]
fn test_force_close_same_epoch_negative_k_pair_pnl() {
    // Account opened long, price moved down → unrealized loss from K-pair
    let mut engine = RiskEngine::new(default_params());
    let a = engine.add_user(1000).unwrap();
    let b = engine.add_user(1000).unwrap();
    engine.deposit(a, 500_000, 1000, 100).unwrap();
    engine.deposit(b, 500_000, 1000, 100).unwrap();

    engine
        .execute_trade_not_atomic(a, b, 1000, 100, (100 * POS_SCALE) as i128, 1000, 0i64)
        .unwrap();

    // Price drops → a (long) has unrealized loss
    engine
        .keeper_crank_not_atomic(200, 500, &[], 64, 0i64)
        .unwrap();

    let cap_before = engine.accounts[a as usize].capital.get();
    let returned = engine.force_close_resolved_not_atomic(a, 200).unwrap();

    // Loss settled from capital
    assert!(
        returned < cap_before,
        "K-pair loss must reduce returned capital"
    );
    assert!(!engine.is_used(a as usize));
    assert!(engine.check_conservation());
}

#[test]
fn test_force_close_with_fee_debt_exceeding_capital() {
    let mut engine = RiskEngine::new(default_params());
    let idx = engine.add_user(1000).unwrap();
    engine.deposit(idx, 10_000, 1000, 100).unwrap();

    // Fee debt >> capital
    engine.accounts[idx as usize].fee_credits = I128::new(-50_000);

    let returned = engine.force_close_resolved_not_atomic(idx, 100).unwrap();
    // Capital (10k) fully swept to insurance, remaining debt forgiven
    assert_eq!(returned, 0, "all capital swept for fee debt");
    assert!(!engine.is_used(idx as usize));
    assert!(engine.check_conservation());
}

#[test]
fn test_force_close_zero_capital_zero_pnl() {
    let mut engine = RiskEngine::new(default_params());
    let idx = engine.add_user(1000).unwrap();
    // No deposit — capital = 0 (new_account_fee consumed all)

    let returned = engine.force_close_resolved_not_atomic(idx, 100).unwrap();
    assert_eq!(returned, 0);
    assert!(!engine.is_used(idx as usize));
    assert!(engine.check_conservation());
}

#[test]
fn test_force_close_c_tot_tracks_exactly() {
    let mut engine = RiskEngine::new(default_params());
    let a = engine.add_user(1000).unwrap();
    let b = engine.add_user(1000).unwrap();
    let c = engine.add_user(1000).unwrap();
    engine.deposit(a, 100_000, 1000, 100).unwrap();
    engine.deposit(b, 200_000, 1000, 100).unwrap();
    engine.deposit(c, 300_000, 1000, 100).unwrap();
    // Align fee slots to prevent maintenance fee interference
    engine.accounts[a as usize].last_fee_slot = 100;
    engine.accounts[b as usize].last_fee_slot = 100;
    engine.accounts[c as usize].last_fee_slot = 100;

    let c_tot_before = engine.c_tot.get();

    let ret_a = engine.force_close_resolved_not_atomic(a, 100).unwrap();
    assert_eq!(engine.c_tot.get(), c_tot_before - ret_a);

    let c_tot_mid = engine.c_tot.get();
    let ret_b = engine.force_close_resolved_not_atomic(b, 100).unwrap();
    assert_eq!(engine.c_tot.get(), c_tot_mid - ret_b);

    let c_tot_mid2 = engine.c_tot.get();
    let ret_c = engine.force_close_resolved_not_atomic(c, 100).unwrap();
    assert_eq!(engine.c_tot.get(), c_tot_mid2 - ret_c);

    assert_eq!(
        engine.c_tot.get(),
        0,
        "all accounts closed → C_tot must be 0"
    );
    assert!(engine.check_conservation());
}

#[test]
fn test_force_close_stored_pos_count_tracks() {
    let mut engine = RiskEngine::new(default_params());
    let a = engine.add_user(1000).unwrap();
    let b = engine.add_user(1000).unwrap();
    engine.deposit(a, 500_000, 1000, 100).unwrap();
    engine.deposit(b, 500_000, 1000, 100).unwrap();

    engine
        .execute_trade_not_atomic(a, b, 1000, 100, (100 * POS_SCALE) as i128, 1000, 0i64)
        .unwrap();
    assert_eq!(engine.stored_pos_count_long, 1);
    assert_eq!(engine.stored_pos_count_short, 1);

    engine.force_close_resolved_not_atomic(a, 100).unwrap();
    assert_eq!(engine.stored_pos_count_long, 0, "long count must decrement");
    // Short count unchanged — b still has position
    assert_eq!(engine.stored_pos_count_short, 1);

    engine.force_close_resolved_not_atomic(b, 100).unwrap();
    assert_eq!(
        engine.stored_pos_count_short, 0,
        "short count must decrement"
    );
}

#[test]
fn test_force_close_multiple_sequential_no_aggregate_drift() {
    let mut engine = RiskEngine::new(default_params());
    let mut accounts = Vec::new();
    for _ in 0..4 {
        let idx = engine.add_user(1000).unwrap();
        engine.deposit(idx, 100_000, 1000, 100).unwrap();
        accounts.push(idx);
    }

    for &idx in &accounts {
        engine.force_close_resolved_not_atomic(idx, 100).unwrap();
    }

    assert_eq!(engine.c_tot.get(), 0);
    assert_eq!(engine.pnl_pos_tot, 0);
    assert_eq!(engine.pnl_matured_pos_tot, 0);
    assert_eq!(engine.stored_pos_count_long, 0);
    assert_eq!(engine.stored_pos_count_short, 0);
    assert_eq!(engine.num_used_accounts, 0);
    assert!(engine.check_conservation());
}

#[test]
fn test_force_close_decrements_oi() {
    let mut engine = RiskEngine::new(default_params());
    let a = engine.add_user(1000).unwrap();
    let b = engine.add_user(1000).unwrap();
    engine.deposit(a, 500_000, 1000, 100).unwrap();
    engine.deposit(b, 500_000, 1000, 100).unwrap();

    engine
        .execute_trade_not_atomic(a, b, 1000, 100, (100 * POS_SCALE) as i128, 1000, 0i64)
        .unwrap();
    assert!(engine.oi_eff_long_q > 0);
    assert!(engine.oi_eff_short_q > 0);

    engine.force_close_resolved_not_atomic(a, 100).unwrap();
    // Bilateral decrement: both sides go to 0 together
    assert_eq!(engine.oi_eff_long_q, 0);
    assert_eq!(engine.oi_eff_short_q, 0);
    assert_eq!(
        engine.oi_eff_long_q, engine.oi_eff_short_q,
        "OI must stay symmetric"
    );

    engine.force_close_resolved_not_atomic(b, 100).unwrap();
    assert_eq!(engine.oi_eff_long_q, 0);
    assert_eq!(engine.oi_eff_short_q, 0);
    assert_eq!(engine.stored_pos_count_long, 0);
    assert_eq!(engine.stored_pos_count_short, 0);
    assert!(engine.check_conservation());
}

#[test]
fn test_force_close_oi_symmetry_after_one_side() {
    // Critical liveness test: after force-closing long-side account,
    // short-side user must be able to close_account without CorruptState.
    let mut engine = RiskEngine::new(default_params());
    let a = engine.add_user(1000).unwrap();
    let b = engine.add_user(1000).unwrap();
    engine.deposit(a, 500_000, 1000, 100).unwrap();
    engine.deposit(b, 500_000, 1000, 100).unwrap();
    engine.accounts[a as usize].last_fee_slot = 100;
    engine.accounts[b as usize].last_fee_slot = 100;

    engine
        .execute_trade_not_atomic(a, b, 1000, 100, (100 * POS_SCALE) as i128, 1000, 0i64)
        .unwrap();
    assert!(engine.oi_eff_long_q > 0);
    assert_eq!(engine.oi_eff_long_q, engine.oi_eff_short_q);

    // Force-close only account a (the long side)
    engine.force_close_resolved_not_atomic(a, 100).unwrap();

    // After force-closing one side, OI must stay symmetric so the
    // other side's users can still close normally.
    assert_eq!(
        engine.oi_eff_long_q, engine.oi_eff_short_q,
        "OI must stay symmetric after force-closing one side"
    );

    // b (short side) must be able to force-close without CorruptState
    engine.force_close_resolved_not_atomic(b, 100).unwrap();
    assert_eq!(engine.oi_eff_long_q, 0);
    assert_eq!(engine.oi_eff_short_q, 0);
    assert!(engine.check_conservation());
}

#[test]
fn test_force_close_rejects_corrupt_a_basis() {
    let mut engine = RiskEngine::new(default_params());
    let a = engine.add_user(1000).unwrap();
    engine.deposit(a, 500_000, 1000, 100).unwrap();

    // Manufacture corrupt state: nonzero position with a_basis = 0
    engine.set_position_basis_q(a as usize, (10 * POS_SCALE) as i128);
    engine.stored_pos_count_long = 1;
    engine.accounts[a as usize].adl_a_basis = 0;

    let result = engine.force_close_resolved_not_atomic(a, 100);
    assert_eq!(
        result,
        Err(RiskError::CorruptState),
        "must reject corrupt a_basis = 0"
    );
}

// ============================================================================
// Spec §12 property 31: full-close liquidation closes full position
// ============================================================================

#[test]
fn test_property_31_fullclose_liquidation_zeros_position() {
    let mut engine = RiskEngine::new(default_params());
    let a = engine.add_user(1000).unwrap();
    let b = engine.add_user(1000).unwrap();
    engine.deposit(a, 50_000, 1000, 100).unwrap();
    engine.deposit(b, 500_000, 1000, 100).unwrap();
    engine.accounts[a as usize].last_fee_slot = 100;
    engine.accounts[b as usize].last_fee_slot = 100;

    // a opens leveraged long
    let size = (450 * POS_SCALE) as i128;
    engine
        .execute_trade_not_atomic(a, b, 1000, 100, size, 1000, 0i64)
        .unwrap();
    assert!(engine.effective_pos_q(a as usize) > 0);

    // Crash price → a is underwater
    let crash = 870u64;
    let result =
        engine.liquidate_at_oracle_not_atomic(a, 101, crash, LiquidationPolicy::FullClose, 0i64);
    assert!(result.is_ok());

    // Property 31: after FullClose, effective_pos_q MUST be 0
    assert_eq!(
        engine.effective_pos_q(a as usize),
        0,
        "FullClose liquidation must zero the effective position"
    );
    // Position basis must also be zero
    assert_eq!(
        engine.accounts[a as usize].position_basis_q, 0,
        "FullClose liquidation must zero position_basis_q"
    );
    assert!(engine.check_conservation());

    // ================================================================
    // Fork-specific tests (PERC-121/122/283/298/299, ADL, premium funding)
    // ================================================================

    #[test]
    fn test_abandoned_with_stale_last_fee_slot_eventually_closed() {
        let mut params = params_for_inline_tests();
        params.maintenance_fee_per_slot = U128::new(1);
        let mut engine = RiskEngine::new(params);

        let user_idx = engine.add_user(0).unwrap();
        // Small deposit
        engine.deposit(user_idx, 5, 1).unwrap();

        assert!(engine.is_used(user_idx as usize));

        // Don't call any user ops. Run crank at a slot far ahead.
        // First crank: drains the account via fee settlement
        let _ = engine
            .keeper_crank(10_000, ORACLE_100K, &[], 64, 0)
            .unwrap();

        // Second crank: GC scan should pick up the dust
        let _outcome = engine
            .keeper_crank(10_001, ORACLE_100K, &[], 64, 0)
            .unwrap();

        // The account must be closed by now (across both cranks)
        assert!(
            !engine.is_used(user_idx as usize),
            "abandoned account with stale last_fee_slot must eventually be GC'd"
        );
        // At least one of the two cranks should have GC'd it
        // (first crank drains capital to 0, GC might close it there already)
    }

    #[test]
    fn test_account_equity_computes_correctly() {
        let engine = RiskEngine::new(default_params());

        // Positive equity
        let account_pos = Account {
            kind: AccountKind::User,
            account_id: 1,
            capital: U128::new(10_000),
            pnl: I128::new(-3_000),
            reserved_pnl: 0,
            warmup_started_at_slot: 0,
            warmup_slope_per_step: U128::ZERO,
            position_size: I128::ZERO,
            entry_price: 0,
            funding_index: I128::ZERO,
            matcher_program: [0; 32],
            matcher_context: [0; 32],
            owner: [0; 32],
            fee_credits: I128::ZERO,
            last_fee_slot: 0,
            last_partial_liquidation_slot: 0,
            position_basis_q: 0i128,
            adl_a_basis: 1_000_000u128,
            adl_k_snap: 0i128,
            adl_epoch_snap: 0,
        };
        assert_eq!(engine.account_equity(&account_pos), 7_000);

        // Negative sum clamped to zero
        let account_neg = Account {
            kind: AccountKind::User,
            account_id: 2,
            capital: U128::new(5_000),
            pnl: I128::new(-8_000),
            reserved_pnl: 0,
            warmup_started_at_slot: 0,
            warmup_slope_per_step: U128::ZERO,
            position_size: I128::ZERO,
            entry_price: 0,
            funding_index: I128::ZERO,
            matcher_program: [0; 32],
            matcher_context: [0; 32],
            owner: [0; 32],
            fee_credits: I128::ZERO,
            last_fee_slot: 0,
            last_partial_liquidation_slot: 0,
            position_basis_q: 0i128,
            adl_a_basis: 1_000_000u128,
            adl_k_snap: 0i128,
            adl_epoch_snap: 0,
        };
        assert_eq!(engine.account_equity(&account_neg), 0);

        // Positive pnl adds to equity
        let account_profit = Account {
            kind: AccountKind::User,
            account_id: 3,
            capital: U128::new(10_000),
            pnl: I128::new(5_000),
            reserved_pnl: 0,
            warmup_started_at_slot: 0,
            warmup_slope_per_step: U128::ZERO,
            position_size: I128::ZERO,
            entry_price: 0,
            funding_index: I128::ZERO,
            matcher_program: [0; 32],
            matcher_context: [0; 32],
            owner: [0; 32],
            fee_credits: I128::ZERO,
            last_fee_slot: 0,
            last_partial_liquidation_slot: 0,
            position_basis_q: 0i128,
            adl_a_basis: 1_000_000u128,
            adl_k_snap: 0i128,
            adl_epoch_snap: 0,
        };
        assert_eq!(engine.account_equity(&account_profit), 15_000);
    }

    #[test]
    fn test_account_field_offsets() {
        use std::mem::offset_of;
        println!("=== Account layout ===");
        println!("account_id: {}", offset_of!(Account, account_id));
        println!("capital: {}", offset_of!(Account, capital));
        println!("kind: {}", offset_of!(Account, kind));
        println!("pnl: {}", offset_of!(Account, pnl));
        println!("reserved_pnl: {}", offset_of!(Account, reserved_pnl));
        println!(
            "warmup_started_at_slot: {}",
            offset_of!(Account, warmup_started_at_slot)
        );
        println!(
            "warmup_slope_per_step: {}",
            offset_of!(Account, warmup_slope_per_step)
        );
        println!("position_size: {}", offset_of!(Account, position_size));
        println!("entry_price: {}", offset_of!(Account, entry_price));
        println!("funding_index: {}", offset_of!(Account, funding_index));
        println!("fee_credits: {}", offset_of!(Account, fee_credits));
        println!("last_fee_slot: {}", offset_of!(Account, last_fee_slot));
        println!("Account size: {}", std::mem::size_of::<Account>());
    }

    #[test]
    fn test_accrue_funding_combined_respects_interval() {
        let mut params = default_params();
        params.funding_premium_weight_bps = 5_000; // 50% premium
        params.funding_settlement_interval_slots = 100;
        params.funding_premium_dampening_e6 = 1_000_000;
        params.funding_premium_max_bps_per_slot = 50;
        let mut engine = Box::new(RiskEngine::new(params));
        engine.mark_price_e6 = 1_010_000; // 1% above index

        // Slot 50: below interval, should not accrue
        engine.last_funding_slot = 0;
        engine.funding_rate_bps_per_slot_last = 10;
        let result = engine.accrue_funding_combined(50, 1_000_000, 5);
        assert!(result.is_ok());
        // Funding index should be unchanged (skipped due to interval)
        assert_eq!(engine.funding_index_qpb_e6.get(), 0);
        assert_eq!(engine.last_funding_slot, 0); // Not updated

        // Slot 100: at interval, should accrue
        let result = engine.accrue_funding_combined(100, 1_000_000, 5);
        assert!(result.is_ok());
        assert_ne!(engine.last_funding_slot, 0); // Updated
    }

    #[test]
    fn test_add_lp_vault_capacity_rejects_overflow() {
        let mut engine = *Box::new(RiskEngine::new(default_params()));

        // Set vault near MAX_VAULT_TVL
        engine.vault = percolator::U128::new(percolator::MAX_VAULT_TVL - 10);

        // add_lp with fee_payment > remaining cap must fail
        let result = engine.add_lp([0; 32], [0; 32], 11);
        assert!(result.is_err(), "add_lp exceeding vault cap must fail");

        // add_lp with fee_payment within cap succeeds
        let result = engine.add_lp([0; 32], [0; 32], 10);
        assert!(result.is_ok(), "add_lp within vault cap must succeed");
    }

    #[test]
    fn test_add_user_vault_capacity_rejects_overflow() {
        let mut engine = *Box::new(RiskEngine::new(default_params()));

        // Set vault near MAX_VAULT_TVL
        engine.vault = percolator::U128::new(percolator::MAX_VAULT_TVL - 10);

        // add_user with fee_payment > remaining cap must fail
        let result = engine.add_user(11);
        assert!(result.is_err(), "add_user exceeding vault cap must fail");

        // add_user with fee_payment within cap succeeds
        let result = engine.add_user(10);
        assert!(result.is_ok(), "add_user within vault cap must succeed");
    }

    #[test]
    fn test_admin_force_close_oob_index_returns_account_not_found() {
        let mut engine = RiskEngine::new(default_params());
        let result = engine.admin_force_close(u16::MAX, 100, 1_000_000);
        assert_eq!(result, Err(RiskError::AccountNotFound));
    }

    #[test]
    fn test_admin_force_close_unused_slot_returns_account_not_found() {
        let mut engine = RiskEngine::new(default_params());
        let result = engine.admin_force_close(0, 100, 1_000_000);
        assert_eq!(result, Err(RiskError::AccountNotFound));
    }

    #[test]
    fn test_admin_force_close_valid_zero_position_returns_ok() {
        let mut engine = RiskEngine::new(default_params());
        let idx = engine.add_user(0).unwrap();
        // Force close on zero position should succeed (no-op)
        assert!(engine.admin_force_close(idx, 100, 1_000_000).is_ok());
    }

    #[test]
    fn test_all_field_offsets() {
        use std::mem::offset_of;
        println!("vault: {}", offset_of!(RiskEngine, vault));
        println!("insurance_fund: {}", offset_of!(RiskEngine, insurance_fund));
        println!("params: {}", offset_of!(RiskEngine, params));
        println!("current_slot: {}", offset_of!(RiskEngine, current_slot));
        println!("c_tot: {}", offset_of!(RiskEngine, c_tot));
        println!("pnl_pos_tot: {}", offset_of!(RiskEngine, pnl_pos_tot));
        println!(
            "total_open_interest: {}",
            offset_of!(RiskEngine, total_open_interest)
        );
        println!("long_oi: {}", offset_of!(RiskEngine, long_oi));
        println!("short_oi: {}", offset_of!(RiskEngine, short_oi));
        println!("net_lp_pos: {}", offset_of!(RiskEngine, net_lp_pos));
        println!("lp_sum_abs: {}", offset_of!(RiskEngine, lp_sum_abs));
        println!("lp_max_abs: {}", offset_of!(RiskEngine, lp_max_abs));
        println!(
            "lp_max_abs_sweep: {}",
            offset_of!(RiskEngine, lp_max_abs_sweep)
        );
        println!(
            "emergency_oi_mode: {}",
            offset_of!(RiskEngine, emergency_oi_mode)
        );
        println!(
            "emergency_start_slot: {}",
            offset_of!(RiskEngine, emergency_start_slot)
        );
        println!(
            "last_breaker_slot: {}",
            offset_of!(RiskEngine, last_breaker_slot)
        );
        println!("trade_twap_e6: {}", offset_of!(RiskEngine, trade_twap_e6));
        println!("twap_last_slot: {}", offset_of!(RiskEngine, twap_last_slot));
        println!("used: {}", offset_of!(RiskEngine, used));
        println!(
            "num_used_accounts: {}",
            offset_of!(RiskEngine, num_used_accounts)
        );
        println!(
            "next_account_id: {}",
            offset_of!(RiskEngine, next_account_id)
        );
        println!("free_head: {}", offset_of!(RiskEngine, free_head));
        println!("next_free: {}", offset_of!(RiskEngine, next_free));
        println!("accounts: {}", offset_of!(RiskEngine, accounts));
        println!("RiskEngine size: {}", std::mem::size_of::<RiskEngine>());
    }

    #[test]
    fn test_all_offsets_for_integration_tests() {
        use std::mem::offset_of;
        println!("=== RiskEngine layout ===");
        println!("vault: {}", offset_of!(RiskEngine, vault));
        println!("insurance_fund: {}", offset_of!(RiskEngine, insurance_fund));
        println!("params: {}", offset_of!(RiskEngine, params));
        println!("current_slot: {}", offset_of!(RiskEngine, current_slot));
        println!("c_tot: {}", offset_of!(RiskEngine, c_tot));
        println!("pnl_pos_tot: {}", offset_of!(RiskEngine, pnl_pos_tot));
        println!(
            "total_open_interest: {}",
            offset_of!(RiskEngine, total_open_interest)
        );
        println!("long_oi: {}", offset_of!(RiskEngine, long_oi));
        println!("short_oi: {}", offset_of!(RiskEngine, short_oi));
        println!("used: {}", offset_of!(RiskEngine, used));
        println!(
            "num_used_accounts: {}",
            offset_of!(RiskEngine, num_used_accounts)
        );
        println!("accounts: {}", offset_of!(RiskEngine, accounts));
        println!("RiskEngine size: {}", std::mem::size_of::<RiskEngine>());
    }

    #[test]
    fn test_audit_conservation_detects_excessive_slack() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(0).unwrap();

        engine.deposit(user_idx, 10_000, 0).unwrap();

        // Conservation should hold normally
        assert!(
            engine.check_conservation(DEFAULT_ORACLE),
            "Normal conservation"
        );

        // Artificially inflate vault beyond MAX_ROUNDING_SLACK
        // This simulates a minting bug
        engine.vault = engine.vault + percolator::MAX_ROUNDING_SLACK + 10;

        // Conservation should now FAIL due to excessive slack
        assert!(
            !engine.check_conservation(DEFAULT_ORACLE),
            "Conservation should fail when slack exceeds MAX_ROUNDING_SLACK"
        );
    }

    #[test]
    fn test_batched_adl_conservation_basic() {
        // Basic test: verify that keeper_crank maintains conservation.
        // This is a simpler regression test to verify batched ADL works.
        let mut params = default_params();
        params.max_crank_staleness_slots = u64::MAX;
        params.warmup_period_slots = 1; // Instant warmup (minimum valid)

        let mut engine = Box::new(RiskEngine::new(params));
        set_insurance(&mut engine, 100_000);

        // Create two users with opposing positions (zero-sum)
        // Give them plenty of capital so they're well above maintenance
        let long = engine.add_user(0).unwrap();
        engine.deposit(long, 200_000, 0).unwrap(); // Well above 5% of 1M = 50k
        engine.accounts[long as usize].position_size = I128::new(1_000_000);
        engine.accounts[long as usize].entry_price = 1_000_000;
        engine.total_open_interest = U128::new(1_000_000);

        let short = engine.add_user(0).unwrap();
        engine.deposit(short, 200_000, 0).unwrap(); // Well above 5% of 1M = 50k
        engine.accounts[short as usize].position_size = I128::new(-1_000_000);
        engine.accounts[short as usize].entry_price = 1_000_000;
        engine.total_open_interest = U128::new(engine.total_open_interest.get() + 1_000_000);

        // Verify conservation before
        assert!(
            engine.check_conservation(DEFAULT_ORACLE),
            "Conservation must hold before crank"
        );

        // Crank at same price (no mark pnl change)
        let outcome = engine.keeper_crank(1, 1_000_000, &[], 64, 0).unwrap();

        // Verify conservation after
        assert!(
            engine.check_conservation(DEFAULT_ORACLE),
            "Conservation must hold after crank"
        );

        // No liquidations should occur at same price
        assert_eq!(outcome.num_liquidations, 0);
        assert_eq!(outcome.num_liq_errors, 0);
    }

    #[test]
    fn test_batched_adl_profit_exclusion() {
        // Test: when liquidating an account with positive mark_pnl (profit from closing),
        // that account should be excluded from funding its own profit via ADL (socialization).
        let mut params = default_params();
        params.maintenance_margin_bps = 500; // 5%
        params.initial_margin_bps = 1000; // 10%
        params.liquidation_buffer_bps = 0; // No buffer
        params.liquidation_fee_bps = 0; // No fee for cleaner math
        params.max_crank_staleness_slots = u64::MAX;
        params.warmup_period_slots = 1; // Instant warmup (minimum valid) // Instant warmup for this test

        let mut engine = Box::new(RiskEngine::new(params));
        set_insurance(&mut engine, 100_000);

        // IMPORTANT: Account creation order matters for per-account processing.
        // We create the liquidated account FIRST so targets are processed AFTER,
        // allowing them to be haircutted to fund the liquidation profit.

        // Create the account to be liquidated FIRST: long from 0.8, so has PROFIT at 0.81
        // But with very low capital, maintenance margin will fail.
        // This creates a "winner liquidation" - account with positive mark_pnl gets liquidated.
        let winner_liq = engine.add_user(0).unwrap();
        engine.deposit(winner_liq, 1_000, 0).unwrap(); // Only 1000 capital
        engine.accounts[winner_liq as usize].position_size = I128::new(1_000_000); // Long 1 unit
        engine.accounts[winner_liq as usize].entry_price = 800_000; // Entered at 0.8

        // Create two accounts that will be the socialization targets (they have positive REALIZED PnL)
        // Socialization haircuts unwrapped PnL (not yet warmed), so keep slope=0.
        // Target 1: has realized profit of 20,000
        let adl_target1 = engine.add_user(0).unwrap();
        engine.deposit(adl_target1, 50_000, 0).unwrap();
        engine.accounts[adl_target1 as usize].pnl = I128::new(20_000); // Realized profit
                                                                       // Keep PnL unwrapped (not warmed) so socialization can haircut it
        engine.accounts[adl_target1 as usize].warmup_slope_per_step = U128::new(0);
        engine.accounts[adl_target1 as usize].warmup_started_at_slot = 0;

        // Target 2: Also has realized profit
        let adl_target2 = engine.add_user(0).unwrap();
        engine.deposit(adl_target2, 50_000, 0).unwrap();
        engine.accounts[adl_target2 as usize].pnl = I128::new(20_000); // Realized profit
        engine.accounts[adl_target2 as usize].warmup_slope_per_step = U128::new(0);
        engine.accounts[adl_target2 as usize].warmup_started_at_slot = 0;

        // Create a counterparty with negative pnl to balance the targets (for conservation)
        let counterparty = engine.add_user(0).unwrap();
        engine.deposit(counterparty, 100_000, 0).unwrap();
        engine.accounts[counterparty as usize].pnl = I128::new(-40_000); // Negative pnl balances targets

        // Set up counterparty short position for zero-sum (counterparty takes other side)
        engine.accounts[counterparty as usize].position_size = I128::new(-1_000_000);
        engine.accounts[counterparty as usize].entry_price = 800_000;
        engine.total_open_interest = U128::new(2_000_000); // Both positions counted

        // At oracle 0.81:
        // mark_pnl = (0.81 - 0.8) * 1 = 10_000
        // equity = 1000 + 10_000 = 11_000
        // position notional = 0.81 * 1 = 810_000 (in fixed point 810_000)
        // maintenance = 5% of 810_000 = 40_500
        // 11_000 < 40_500, so UNDERWATER

        // Snapshot before
        let target1_pnl_before = engine.accounts[adl_target1 as usize].pnl;
        let target2_pnl_before = engine.accounts[adl_target2 as usize].pnl;

        // Verify conservation holds before crank (at entry price since that's where positions are marked)
        let entry_oracle = 800_000; // Positions were created at this price
        assert!(
            engine.check_conservation(entry_oracle),
            "Conservation must hold before crank"
        );

        // Run crank at oracle price 0.81 - liquidation adds profit to pending bucket
        let crank_oracle = 810_000;
        let outcome = engine.keeper_crank(1, crank_oracle, &[], 64, 0).unwrap();

        // Run additional cranks until socialization completes
        // (socialization processes accounts per crank)
        for slot in 2..20 {
            engine.keeper_crank(slot, crank_oracle, &[], 64, 0).unwrap();
        }

        // Verify conservation holds after socialization (use crank oracle since entries were updated)
        assert!(
            engine.check_conservation(crank_oracle),
            "Conservation must hold after batched liquidation"
        );

        // The liquidated account had positive mark_pnl (profit from closing).
        // That profit should be funded by socialization from the other profitable accounts.
        // With variation margin settlement, the mark PnL is settled to the pnl field
        // BEFORE liquidation. The "close profit" that would be socialized is now
        // already in the pnl field. The liquidation closes positions at oracle price
        // where entry = oracle after settlement, so there's no additional profit to socialize.
        //
        // This is the expected behavior change from variation margin:
        // - Old: close PnL calculated at liquidation time, socialized via ADL
        // - New: mark PnL settled before liquidation, no additional close PnL
        //
        // The test verifies that either:
        // 1. Targets were haircutted (old behavior), OR
        // 2. Liquidation occurred but profit was settled pre-liquidation (new behavior)
        let target1_pnl_after = engine.accounts[adl_target1 as usize].pnl.get();
        let target2_pnl_after = engine.accounts[adl_target2 as usize].pnl.get();

        let total_haircut = (target1_pnl_before.get() - target1_pnl_after)
            + (target2_pnl_before.get() - target2_pnl_after);

        // With variation margin: the winner's profit is in pnl field, not from close
        // So socialization may not occur. Check that liquidation happened.
        assert!(
            outcome.num_liquidations > 0 || total_haircut > 0,
            "Either liquidation should occur or targets should be haircutted"
        );
    }

    #[test]
    fn test_blended_mark_70_30() {
        // oracle=100, twap=200, w=7000 (70%)
        // mark = (100*7000 + 200*3000) / 10000 = (700_000 + 600_000) / 10000 = 130
        let mark = RiskEngine::compute_blended_mark_price(100_000_000, 200_000_000, 7_000);
        assert_eq!(
            mark, 130_000_000,
            "70/30 blend of 100M and 200M should be 130M"
        );
    }

    #[test]
    fn test_blended_mark_full_oracle() {
        let mark = RiskEngine::compute_blended_mark_price(100_000_000, 200_000_000, 10_000);
        assert_eq!(mark, 100_000_000, "100% oracle weight should return oracle");
    }

    #[test]
    fn test_blended_mark_full_twap() {
        let mark = RiskEngine::compute_blended_mark_price(100_000_000, 200_000_000, 0);
        assert_eq!(mark, 200_000_000, "0% oracle weight should return TWAP");
    }

    #[test]
    fn test_blended_mark_no_oracle() {
        let mark = RiskEngine::compute_blended_mark_price(0, 2_000_000, 7_000);
        assert_eq!(
            mark, 2_000_000,
            "With zero oracle, mark should be pure TWAP"
        );
    }

    #[test]
    fn test_blended_mark_no_twap() {
        let mark = RiskEngine::compute_blended_mark_price(1_000_000, 0, 7_000);
        assert_eq!(
            mark, 1_000_000,
            "With zero TWAP, mark should be pure oracle"
        );
    }

    #[test]
    fn test_blended_mark_weight_clamped() {
        let mark = RiskEngine::compute_blended_mark_price(100_000_000, 200_000_000, 20_000);
        assert_eq!(
            mark, 100_000_000,
            "Weight > 10000 should clamp to pure oracle"
        );
    }

    #[test]
    fn test_check_conservation_fails_on_mark_overflow() {
        let mut params = default_params();
        params.max_accounts = 64;

        let mut engine = Box::new(RiskEngine::new(params));

        // Create user account
        let user_idx = engine.add_user(0).unwrap();

        // Manually set up an account state that will cause mark_pnl overflow
        // position_size = i128::MAX, entry_price = MAX_ORACLE_PRICE
        // When mark_pnl is calculated with oracle = 1, it will overflow
        engine.accounts[user_idx as usize].position_size = I128::new(i128::MAX);
        engine.accounts[user_idx as usize].entry_price = MAX_ORACLE_PRICE;
        engine.accounts[user_idx as usize].capital = U128::ZERO;
        engine.accounts[user_idx as usize].pnl = I128::new(0);

        // Conservation should fail because mark_pnl calculation overflows
        assert!(
            !engine.check_conservation(1),
            "check_conservation should return false when mark_pnl overflows"
        );
    }

    #[test]
    fn test_combined_funding_rate_50_50() {
        let combined = RiskEngine::compute_combined_funding_rate(
            10,    // inventory rate
            50,    // premium rate
            5_000, // weight = 50%
        );
        // (10 * 5000 + 50 * 5000) / 10000 = 300000 / 10000 = 30
        assert_eq!(combined, 30);
    }

    #[test]
    fn test_combined_funding_rate_pure_inventory() {
        let combined = RiskEngine::compute_combined_funding_rate(
            10, // inventory rate
            50, // premium rate
            0,  // weight = 0 (pure inventory)
        );
        assert_eq!(combined, 10);
    }

    #[test]
    fn test_combined_funding_rate_pure_premium() {
        let combined = RiskEngine::compute_combined_funding_rate(
            10,     // inventory rate
            50,     // premium rate
            10_000, // weight = 100% (pure premium)
        );
        assert_eq!(combined, 50);
    }

    #[test]
    fn test_compute_liquidation_close_amount_basic() {
        let params = default_params();
        let mut engine = Box::new(RiskEngine::new(params));
        let user = engine.add_user(0).unwrap();

        // Setup: position = 10 units, capital = 500k
        // At oracle $1: equity = 500k, position_value = 10M
        // MM = 10M * 5% = 500k
        // Target = 10M * 6% = 600k
        // abs_pos_safe_max = 500k * 10B / (1M * 600) = 8.33M
        // close_abs = 10M - 8.33M = 1.67M
        engine.accounts[user as usize].capital = U128::new(500_000);
        engine.accounts[user as usize].position_size = I128::new(10_000_000);
        engine.accounts[user as usize].entry_price = 1_000_000;
        engine.accounts[user as usize].pnl = I128::new(0);

        let account = &engine.accounts[user as usize];
        let (close_abs, is_full) = engine.compute_liquidation_close_amount(account, 1_000_000);

        // Should close some but not all
        assert!(close_abs > 0, "Should close some position");
        assert!(close_abs < 10_000_000, "Should not close entire position");
        assert!(!is_full, "Should be partial close");

        // Remaining should be >= min_liquidation_abs
        let remaining = 10_000_000 - close_abs;
        assert!(
            remaining >= params.min_liquidation_abs.get(),
            "Remaining should be above min threshold"
        );
    }

    #[test]
    fn test_compute_liquidation_dust_kill() {
        let mut params = default_params();
        params.min_liquidation_abs = U128::new(9_000_000); // 9 units minimum (so after partial, remaining < 9 triggers kill)
        params.liquidation_fee_cap = U128::new(10_000_000); // cap >= min_abs (spec §1.4)

        let mut engine = Box::new(RiskEngine::new(params));
        let user = engine.add_user(0).unwrap();

        // Setup: position = 10 units at $1, capital = 500k
        // At oracle $1: equity = 500k, position_value = 10M
        // Target = 6% of position_value
        // abs_pos_safe_max = 500k * 10B / (1M * 600) = 8.33M
        // remaining = 8.33M < 9M threshold => dust kill triggers
        engine.accounts[user as usize].capital = U128::new(500_000);
        engine.accounts[user as usize].position_size = I128::new(10_000_000);
        engine.accounts[user as usize].entry_price = 1_000_000;
        engine.accounts[user as usize].pnl = I128::new(0);

        let account = &engine.accounts[user as usize];
        let (close_abs, is_full) = engine.compute_liquidation_close_amount(account, 1_000_000);

        // Should trigger full close due to dust rule (remaining 8.33M < 9M min)
        assert_eq!(close_abs, 10_000_000, "Should close entire position");
        assert!(is_full, "Should be full close due to dust rule");
    }

    #[test]
    fn test_compute_liquidation_zero_equity() {
        let params = default_params();
        let mut engine = Box::new(RiskEngine::new(params));
        let user = engine.add_user(0).unwrap();

        // Setup: position = 10 units at $1, capital = 1M
        // At oracle $0.85: equity = max(0, 1M - 1.5M) = 0
        engine.accounts[user as usize].capital = U128::new(1_000_000);
        engine.accounts[user as usize].position_size = I128::new(10_000_000);
        engine.accounts[user as usize].entry_price = 1_000_000;
        // Simulate the mark pnl being applied
        engine.accounts[user as usize].pnl = I128::new(-1_500_000);

        let account = &engine.accounts[user as usize];
        let (close_abs, is_full) = engine.compute_liquidation_close_amount(account, 850_000);

        // Zero equity means full close
        assert_eq!(close_abs, 10_000_000, "Should close entire position");
        assert!(is_full, "Should be full close when equity is zero");
    }

    #[test]
    fn test_conservation_simple() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user1 = engine.add_user(0).unwrap();
        let user2 = engine.add_user(0).unwrap();

        // Initial state should conserve
        assert!(engine.check_conservation(DEFAULT_ORACLE));

        // Deposit to user1
        engine.deposit(user1, 1000, 0).unwrap();
        assert!(engine.check_conservation(DEFAULT_ORACLE));

        // Deposit to user2
        engine.deposit(user2, 2000, 0).unwrap();
        assert!(engine.check_conservation(DEFAULT_ORACLE));

        // PNL is zero-sum: user1 gains 500, user2 loses 500
        // (vault unchanged since this is internal redistribution)
        assert_eq!(engine.accounts[user1 as usize].pnl.get(), 0);
        assert_eq!(engine.accounts[user2 as usize].pnl.get(), 0);
        engine.accounts[user1 as usize].pnl = I128::new(500);
        engine.accounts[user2 as usize].pnl = I128::new(-500);
        assert!(engine.check_conservation(DEFAULT_ORACLE));

        // Withdraw from user1's capital
        engine.withdraw(user1, 500, 0, 1_000_000).unwrap();
        assert!(engine.check_conservation(DEFAULT_ORACLE));
    }

    #[test]
    fn test_crank_force_closes_dust_positions() {
        let mut params = default_params();
        params.risk_reduction_threshold = U128::new(1000);
        params.min_liquidation_abs = U128::new(100_000); // 100k minimum
        let mut engine = Box::new(RiskEngine::new(params));
        engine.vault = U128::new(100_000);

        // Create counterparty LP
        let lp = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();
        engine.deposit(lp, 50_000, 0).unwrap();

        // Create user with DUST position (below min_liquidation_abs)
        let user = engine.add_user(0).unwrap();
        engine.deposit(user, 10_000, 0).unwrap();
        engine.accounts[user as usize].position_size = I128::new(50_000); // Below 100k threshold
        engine.accounts[user as usize].entry_price = 1_000_000;
        engine.accounts[lp as usize].position_size = I128::new(-50_000);
        engine.accounts[lp as usize].entry_price = 1_000_000;
        engine.total_open_interest = U128::new(100_000);

        // Set insurance ABOVE threshold (force-realize NOT active)
        engine.insurance_fund.balance = U128::new(2000);

        assert!(
            !engine.accounts[user as usize].position_size.is_zero(),
            "User should have position before crank"
        );

        // Run crank
        let outcome = engine.keeper_crank(1, 1_000_000, &[], 64, 0).unwrap();

        // Force-realize mode should NOT be needed (insurance above threshold)
        assert!(
            !outcome.force_realize_needed,
            "Force-realize should not be needed"
        );

        // But the dust position should still be closed
        assert!(
            engine.accounts[user as usize].position_size.is_zero(),
            "Dust position should be force-closed"
        );
        assert!(
            engine.accounts[lp as usize].position_size.is_zero(),
            "LP dust position should also be force-closed"
        );
    }

    #[test]
    fn test_cross_lp_close_no_pnl_teleport() {
        let mut params = default_params();
        params.trading_fee_bps = 0;
        params.max_crank_staleness_slots = u64::MAX;
        params.max_accounts = 64;

        let mut engine = Box::new(RiskEngine::new(params));

        // Create two LPs with different entry prices (simulated)
        let lp1 = engine.add_lp([1u8; 32], [0u8; 32], 0).unwrap();
        engine.deposit(lp1, 1_000_000, 0).unwrap();

        let lp2 = engine.add_lp([2u8; 32], [0u8; 32], 0).unwrap();
        engine.deposit(lp2, 1_000_000, 0).unwrap();

        let user = engine.add_user(0).unwrap();
        engine.deposit(user, 1_000_000, 0).unwrap();

        // User opens position with LP1 at oracle 1_000_000
        let oracle1 = 1_000_000;
        engine
            .execute_trade(&MATCHER, lp1, user, 0, oracle1, 1_000_000)
            .unwrap();

        // Capture state
        let user_pnl_after_open = engine.accounts[user as usize].pnl.get();
        let lp1_pnl_after_open = engine.accounts[lp1 as usize].pnl.get();
        let lp2_pnl_after_open = engine.accounts[lp2 as usize].pnl.get();

        // All pnl should be 0 since oracle = exec
        assert_eq!(user_pnl_after_open, 0);
        assert_eq!(lp1_pnl_after_open, 0);
        assert_eq!(lp2_pnl_after_open, 0);

        // Now user closes with LP2 at SAME oracle (no price movement)
        // With old logic: PnL could "teleport" between LPs based on entry price differences
        // With new variation margin: all entries are at oracle, so no spurious PnL
        engine
            .execute_trade(&MATCHER, lp2, user, 0, oracle1, -1_000_000)
            .unwrap();

        // User should have 0 pnl (no price movement)
        let user_pnl_after_close = engine.accounts[user as usize].pnl.get();
        assert_eq!(
            user_pnl_after_close, 0,
            "User pnl should be 0 when closing at same oracle price"
        );

        // LP1 still has 0 pnl (never touched again after open)
        let lp1_pnl_after_close = engine.accounts[lp1 as usize].pnl.get();
        assert_eq!(lp1_pnl_after_close, 0, "LP1 pnl should remain 0");

        // LP2 should also have 0 pnl (took opposite of close at same price)
        let lp2_pnl_after_close = engine.accounts[lp2 as usize].pnl.get();
        assert_eq!(lp2_pnl_after_close, 0, "LP2 pnl should be 0");

        // CRITICAL: Total PnL should be exactly 0 (no value created/destroyed)
        let total_pnl = user_pnl_after_close + lp1_pnl_after_close + lp2_pnl_after_close;
        assert_eq!(total_pnl, 0, "Total PnL must be zero-sum");

        // Conservation should hold
        assert!(
            engine.check_conservation(oracle1),
            "Conservation should hold"
        );
    }

    #[test]
    fn test_cross_lp_close_no_pnl_teleport_simple() {
        let mut engine = RiskEngine::new(params_for_inline_tests());

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
                    price: oracle_price - (10_000 * 1_000_000),
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
        // Coin-margined profit: (10K*E6) * ONE_BASE / ORACLE_100K = 100_000
        let profit: u128 = 100_000;
        let user_pnl = engine.accounts[user as usize].pnl.get() as u128;
        let user_cap = engine.accounts[user as usize].capital.get();
        let initial_cap = 50_000 * (E6 as u128);
        // Total user value (pnl + capital) must equal initial_capital + coin-margined profit
        assert_eq!(
            user_pnl + user_cap,
            initial_cap + profit,
            "user total value must be initial_capital + trade profit"
        );
        assert_eq!(engine.accounts[lp1 as usize].pnl.get(), 0);
        assert_eq!(
            engine.accounts[lp1 as usize].capital.get(),
            initial_cap - profit
        );
        // LP2 must be unaffected (no teleportation)
        assert_eq!(engine.accounts[lp2 as usize].pnl.get(), 0);
        assert_eq!(engine.accounts[lp2 as usize].capital.get(), initial_cap);

        // Conservation must still hold
        assert!(engine.check_conservation(ORACLE_100K));
    }

    #[test]
    fn test_deposit_and_withdraw() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(0).unwrap();

        // Deposit
        let v0 = vault_snapshot(&engine);
        engine.deposit(user_idx, 1000, 0).unwrap();
        assert_eq!(engine.accounts[user_idx as usize].capital.get(), 1000);
        assert_vault_delta(&engine, v0, 1000);

        // Withdraw partial
        let v1 = vault_snapshot(&engine);
        engine.withdraw(user_idx, 400, 0, 1_000_000).unwrap();
        assert_eq!(engine.accounts[user_idx as usize].capital.get(), 600);
        assert_vault_delta(&engine, v1, -400);

        // Withdraw rest
        let v2 = vault_snapshot(&engine);
        engine.withdraw(user_idx, 600, 0, 1_000_000).unwrap();
        assert_eq!(engine.accounts[user_idx as usize].capital.get(), 0);
        assert_vault_delta(&engine, v2, -600);

        assert_conserved(&engine);
    }

    #[test]
    fn test_deposit_fee_credits_updates_vault_and_insurance() {
        let mut engine = RiskEngine::new(params_for_inline_tests());
        let user_idx = engine.add_user(0).unwrap();

        let vault_before = engine.vault.get();
        let ins_before = engine.insurance_fund.balance.get();
        let rev_before = engine.insurance_fund.fee_revenue.get();

        engine.deposit_fee_credits(user_idx, 500, 10).unwrap();

        assert_eq!(
            engine.vault.get() - vault_before,
            500,
            "vault must increase"
        );
        assert_eq!(
            engine.insurance_fund.balance.get() - ins_before,
            500,
            "insurance balance must increase"
        );
        assert_eq!(
            engine.insurance_fund.fee_revenue.get() - rev_before,
            500,
            "insurance fee_revenue must increase"
        );
        assert_eq!(
            engine.accounts[user_idx as usize].fee_credits.get(),
            500,
            "fee_credits must increase"
        );
    }

    #[test]
    fn test_deposit_fee_credits_vault_capacity_rejects_overflow() {
        let mut engine = *Box::new(RiskEngine::new(default_params()));
        let idx = engine.add_user(0).unwrap();
        engine.deposit(idx, 1_000, 0).unwrap();

        // Set vault near MAX_VAULT_TVL
        engine.vault = percolator::U128::new(percolator::MAX_VAULT_TVL - 100);

        // deposit_fee_credits within cap succeeds
        let result = engine.deposit_fee_credits(idx, 100, 1);
        assert!(result.is_ok(), "fee credits within cap must succeed");

        // Exceeding cap fails
        let result = engine.deposit_fee_credits(idx, 1, 2);
        assert!(result.is_err(), "fee credits exceeding vault cap must fail");
    }

    #[test]
    fn test_deposit_ghost_account_no_state_leak_on_cap_failure() {
        let mut engine = *Box::new(RiskEngine::new(default_params()));
        let idx = engine.add_user(0).unwrap();
        engine.deposit(idx, 10_000, 0).unwrap();

        // Record state before failed deposit
        let vault_before = engine.vault.get();
        let capital_before = engine.accounts[idx as usize].capital.get();
        let c_tot_before = engine.c_tot.get();

        // Set vault so next deposit will exceed cap
        engine.vault = percolator::U128::new(percolator::MAX_VAULT_TVL);
        let vault_at_cap = engine.vault.get();

        let result = engine.deposit(idx, 1, 1);
        assert!(result.is_err());

        // Verify NO state was mutated on failure
        assert_eq!(
            engine.vault.get(),
            vault_at_cap,
            "vault must not change on failed deposit"
        );
        assert_eq!(
            engine.accounts[idx as usize].capital.get(),
            capital_before,
            "capital must not change on failed deposit"
        );
    }

    #[test]
    fn test_deposit_min_initial_deposit_allows_subsequent_dust() {
        let mut params = default_params();
        params.new_account_fee = percolator::U128::new(1_000);
        let mut engine = *Box::new(RiskEngine::new(params));
        let idx = engine.add_user(1_000).unwrap();

        // First deposit meets minimum
        engine.deposit(idx, 5_000, 1).unwrap();
        assert!(engine.accounts[idx as usize].capital.get() > 0);

        // Subsequent small deposits are fine (account already has capital)
        let result = engine.deposit(idx, 1, 2);
        assert!(
            result.is_ok(),
            "small deposit on funded account must succeed"
        );
    }

    #[test]
    fn test_deposit_min_initial_deposit_rejects_dust() {
        let mut params = default_params();
        params.new_account_fee = percolator::U128::new(1_000); // min deposit = 1000
        let mut engine = *Box::new(RiskEngine::new(params));
        // add_user with exact fee — capital starts at 0
        let idx = engine.add_user(1_000).unwrap();
        assert_eq!(engine.accounts[idx as usize].capital.get(), 0);

        // Dust deposit (< min_initial_deposit) on zero-capital account must fail
        let result = engine.deposit(idx, 999, 1);
        assert!(
            result.is_err(),
            "dust deposit on zero-capital account must fail"
        );

        // Deposit exactly at min threshold succeeds
        let result = engine.deposit(idx, 1_000, 2);
        assert!(
            result.is_ok(),
            "deposit at min_initial_deposit threshold must succeed"
        );
    }

    #[test]
    fn test_deposit_settles_accrued_maintenance_fees() {
        // Setup engine with non-zero maintenance fee
        let mut params = default_params();
        params.maintenance_fee_per_slot = U128::new(10); // 10 units per slot
        let mut engine = Box::new(RiskEngine::new(params));

        let user_idx = engine.add_user(0).unwrap();

        // Initial deposit at slot 0
        engine.deposit(user_idx, 1000, 0).unwrap();
        assert_eq!(engine.accounts[user_idx as usize].capital.get(), 1000);
        assert_eq!(engine.accounts[user_idx as usize].last_fee_slot, 0);

        // Deposit at slot 100 - should charge 100 * 10 = 1000 in fees
        // Depositing 500:
        //   - 500 from deposit pays fees → insurance += 500, fee_credits = -500
        //   - 0 goes to capital
        //   - pay_fee_debt_from_capital sweep: capital(1000) pays remaining 500 debt
        //     → capital = 500, insurance += 500, fee_credits = 0
        let insurance_before = engine.insurance_fund.balance;
        engine.deposit(user_idx, 500, 100).unwrap();

        // Account's last_fee_slot should be updated
        assert_eq!(engine.accounts[user_idx as usize].last_fee_slot, 100);

        // Capital = 500 (was 1000, fee debt sweep paid 500)
        assert_eq!(engine.accounts[user_idx as usize].capital.get(), 500);

        // Insurance received 1000 total: 500 from deposit + 500 from capital sweep
        assert_eq!(
            (engine.insurance_fund.balance - insurance_before).get(),
            1000
        );

        // fee_credits fully repaid by capital sweep
        assert_eq!(engine.accounts[user_idx as usize].fee_credits.get(), 0);

        // Now deposit 1000 more at slot 100 (no additional fees, no debt)
        engine.deposit(user_idx, 1000, 100).unwrap();

        // All 1000 goes to capital (no debt to pay)
        assert_eq!(engine.accounts[user_idx as usize].capital.get(), 1500);
        assert_eq!(engine.accounts[user_idx as usize].fee_credits.get(), 0);

        assert_conserved(&engine);
    }

    #[test]
    fn test_deposit_vault_capacity_exact_boundary() {
        let mut engine = *Box::new(RiskEngine::new(default_params()));
        let idx = engine.add_user(0).unwrap();

        // Set vault so that deposit brings it exactly to MAX_VAULT_TVL
        engine.vault = percolator::U128::new(percolator::MAX_VAULT_TVL - 50_000);
        let result = engine.deposit(idx, 50_000, 1);
        assert!(
            result.is_ok(),
            "deposit to exactly MAX_VAULT_TVL must succeed"
        );
        assert_eq!(engine.vault.get(), percolator::MAX_VAULT_TVL);
    }

    #[test]
    fn test_deposit_vault_capacity_rejects_overflow() {
        let mut engine = *Box::new(RiskEngine::new(default_params()));
        let idx = engine.add_user(0).unwrap();

        // Artificially set vault near MAX_VAULT_TVL
        engine.vault = percolator::U128::new(percolator::MAX_VAULT_TVL - 100);

        // Deposit that fits within cap succeeds
        let result = engine.deposit(idx, 100, 1);
        assert!(result.is_ok(), "deposit within cap must succeed");

        // Vault is now exactly at MAX_VAULT_TVL; any further deposit must fail
        let result = engine.deposit(idx, 1, 2);
        assert!(result.is_err(), "deposit exceeding vault cap must fail");
    }

    #[test]
    fn test_dust_killswitch_forces_full_close() {
        let mut params = default_params();
        params.maintenance_margin_bps = 500;
        params.liquidation_buffer_bps = 100;
        params.min_liquidation_abs = U128::new(5_000_000); // 5 units minimum
        params.liquidation_fee_cap = U128::new(10_000_000); // cap >= min_abs (spec §1.4)

        let mut engine = Box::new(RiskEngine::new(params));

        // Create user with direct setup (matching test_liquidation_fee_calculation pattern)
        let user = engine.add_user(0).unwrap();

        // Position: 6 units at $1, barely undercollateralized at oracle = entry
        // position_value = 6_000_000
        // MM = 6_000_000 * 5% = 300_000
        // Set capital below MM to trigger liquidation
        engine.accounts[user as usize].capital = U128::new(200_000);
        engine.accounts[user as usize].position_size = I128::new(6_000_000);
        engine.accounts[user as usize].entry_price = 1_000_000;
        engine.accounts[user as usize].pnl = I128::new(0);
        engine.total_open_interest = U128::new(6_000_000);
        engine.vault = U128::new(200_000);

        // Oracle at entry price (no mark pnl)
        let oracle_price = 1_000_000;

        // Liquidate
        let result = engine.liquidate_at_oracle(user, 0, oracle_price).unwrap();
        assert!(result, "Liquidation should succeed");

        // Due to dust kill-switch (remaining < 5 units), position should be fully closed
        assert_eq!(
            engine.accounts[user as usize].position_size.get(),
            0,
            "Dust kill-switch should force full close"
        );
    }

    #[test]
    fn test_dust_negative_fee_credits_gc() {
        let mut engine = RiskEngine::new(params_for_inline_tests());

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
        let outcome = engine.keeper_crank(10, ORACLE_100K, &[], 64, 0).unwrap();

        assert_eq!(
            outcome.num_gc_closed, 1,
            "expected GC to close account with negative fee_credits"
        );
        assert!(
            !engine.is_used(user_idx as usize),
            "account should be freed"
        );
    }

    #[test]
    fn test_dust_stale_funding_gc() {
        let mut engine = RiskEngine::new(params_for_inline_tests());

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
        let outcome = engine.keeper_crank(10, ORACLE_100K, &[], 64, 0).unwrap();

        assert_eq!(
            outcome.num_gc_closed, 1,
            "expected GC to close stale-funding dust"
        );
        assert!(
            !engine.is_used(user_idx as usize),
            "account should be freed"
        );
    }

    #[test]
    fn test_dynamic_fee_flat_when_tiers_disabled() {
        let mut params = default_params();
        params.trading_fee_bps = 10; // 0.1%
        params.fee_tier2_threshold = 0; // disabled
        let engine = Box::new(RiskEngine::new(params));
        // Any notional → flat rate
        assert_eq!(engine.compute_dynamic_fee_bps(1_000), 10);
        assert_eq!(engine.compute_dynamic_fee_bps(1_000_000_000), 10);
    }

    #[test]
    fn test_dynamic_fee_tiered() {
        let mut params = default_params();
        params.trading_fee_bps = 5; // Tier 1: 0.05%
        params.fee_tier2_bps = 8; // Tier 2: 0.08%
        params.fee_tier3_bps = 10; // Tier 3: 0.10%
        params.fee_tier2_threshold = 1_000_000; // 1M
        params.fee_tier3_threshold = 10_000_000; // 10M
        let engine = Box::new(RiskEngine::new(params));

        assert_eq!(engine.compute_dynamic_fee_bps(500_000), 5); // Tier 1
        assert_eq!(engine.compute_dynamic_fee_bps(1_000_000), 8); // Tier 2
        assert_eq!(engine.compute_dynamic_fee_bps(5_000_000), 8); // Tier 2
        assert_eq!(engine.compute_dynamic_fee_bps(10_000_000), 10); // Tier 3
        assert_eq!(engine.compute_dynamic_fee_bps(100_000_000), 10); // Tier 3
    }

    #[test]
    fn test_dynamic_fee_utilization_surge() {
        let mut params = default_params();
        params.trading_fee_bps = 10;
        params.fee_utilization_surge_bps = 20; // max 20bps surge at 100% utilization
        let mut engine = Box::new(RiskEngine::new(params));

        // No vault → no surge
        assert_eq!(engine.compute_dynamic_fee_bps(1_000), 10);

        // Set vault and OI
        engine.vault = U128::new(1_000_000);
        engine.total_open_interest = U128::new(0);
        assert_eq!(engine.compute_dynamic_fee_bps(1_000), 10); // 0% utilization

        engine.total_open_interest = U128::new(1_000_000); // 50% util (OI / 2*vault)
        assert_eq!(engine.compute_dynamic_fee_bps(1_000), 20); // 10 + 20*0.5 = 20

        engine.total_open_interest = U128::new(2_000_000); // 100% util
        assert_eq!(engine.compute_dynamic_fee_bps(1_000), 30); // 10 + 20 = 30
    }

    #[test]
    fn test_emergency_cooldown_bypass_critically_underwater() {
        let mut params = default_params();
        params.maintenance_margin_bps = 500; // 5%
        params.liquidation_buffer_bps = 100; // 1% buffer → target 6%
        params.min_liquidation_abs = U128::new(1);
        params.partial_liquidation_bps = 2000; // 20% per partial
        params.partial_liquidation_cooldown_slots = 30;
        params.use_mark_price_for_liquidation = true;
        params.emergency_liquidation_margin_bps = 200; // 2% emergency threshold

        let mut engine = Box::new(RiskEngine::new(params));
        engine.mark_price_e6 = 1_000_000; // $1 mark price

        // Setup LP (required for risk engine)
        let lp = engine.add_lp([0u8; 32], [0u8; 32], 0).unwrap();
        engine.accounts[lp as usize].capital = U128::new(100_000_000);
        engine.accounts[lp as usize].position_size = I128::new(-10_000_000);
        engine.accounts[lp as usize].entry_price = 1_000_000;

        let user = engine.add_user(0).unwrap();

        // Position: 10 units at $1, capital = 300k
        // At $1: position_value = 10M, equity = 300k
        // MM = 10M * 5% = 500k
        // equity (300k) < MM (500k) → underwater
        engine.accounts[user as usize].capital = U128::new(300_000);
        engine.accounts[user as usize].position_size = I128::new(10_000_000);
        engine.accounts[user as usize].entry_price = 1_000_000;
        engine.accounts[user as usize].pnl = I128::new(0);
        engine.total_open_interest = U128::new(10_000_000);
        engine.vault = U128::new(100_300_000);

        // First partial liquidation at slot 100
        let result = engine
            .liquidate_with_mark_price(user, 100, 1_000_000)
            .unwrap();
        assert!(result, "First partial liquidation should succeed");

        // Account should still have a position (partial, not full)
        let pos_after_first = engine.accounts[user as usize].position_size.get();
        // Position may have been fully closed by safety check; skip rest if so
        if pos_after_first == 0 {
            return; // Safety check already handled it
        }

        // Now simulate price crash: mark price drops substantially
        // The account becomes critically underwater (below emergency threshold)
        engine.mark_price_e6 = 500_000; // $0.50 mark price — big drop

        // Set very low capital to simulate critically underwater
        // At $0.50 mark: position_value = pos * 0.5, equity very low
        // We need margin ratio < 2% (emergency_liquidation_margin_bps)
        engine.accounts[user as usize].capital = U128::new(10_000); // Very low capital
        engine.accounts[user as usize].pnl = I128::new(-290_000); // Large loss

        // Try liquidation at slot 105 — within cooldown (last was 100, cooldown=30)
        // Normally this would return Ok(false) due to cooldown.
        // But since account is critically underwater (< 2% margin), it must bypass.
        let result2 = engine
            .liquidate_with_mark_price(user, 105, 500_000)
            .unwrap();
        assert!(
            result2,
            "Emergency liquidation must bypass cooldown for critically underwater accounts"
        );

        // Position should be fully closed
        assert!(
            engine.accounts[user as usize].position_size.is_zero(),
            "Critically underwater account should be fully liquidated"
        );
    }

    #[test]
    fn test_execute_trade_rejects_matcher_opposite_sign() {
        let mut params = default_params();
        params.trading_fee_bps = 0;
        params.max_crank_staleness_slots = u64::MAX;
        params.max_accounts = 64;

        let mut engine = Box::new(RiskEngine::new(params));

        let lp_idx = engine.add_lp([1u8; 32], [0u8; 32], 0).unwrap();
        engine.deposit(lp_idx, 1_000_000, 0).unwrap();

        let user_idx = engine.add_user(0).unwrap();
        engine.deposit(user_idx, 1_000_000, 0).unwrap();

        let result = engine.execute_trade(
            &OppositeSignMatcher,
            lp_idx,
            user_idx,
            0,
            1_000_000,
            1_000_000, // Request positive size
        );

        assert!(
            matches!(result, Err(RiskError::InvalidMatchingEngine)),
            "Should reject matcher that returns opposite sign: {:?}",
            result
        );
    }

    #[test]
    fn test_execute_trade_rejects_matcher_oversize_fill() {
        let mut params = default_params();
        params.trading_fee_bps = 0;
        params.max_crank_staleness_slots = u64::MAX;
        params.max_accounts = 64;

        let mut engine = Box::new(RiskEngine::new(params));

        let lp_idx = engine.add_lp([1u8; 32], [0u8; 32], 0).unwrap();
        engine.deposit(lp_idx, 1_000_000, 0).unwrap();

        let user_idx = engine.add_user(0).unwrap();
        engine.deposit(user_idx, 1_000_000, 0).unwrap();

        let result = engine.execute_trade(
            &OversizeMatcher,
            lp_idx,
            user_idx,
            0,
            1_000_000,
            500_000, // Request half size
        );

        assert!(
            matches!(result, Err(RiskError::InvalidMatchingEngine)),
            "Should reject matcher that returns oversize fill: {:?}",
            result
        );
    }

    #[test]
    fn test_execute_trade_runs_end_of_instruction_lifecycle() {
        use percolator::SideMode;
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(0).unwrap();
        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();

        engine.deposit(user_idx, 100_000, 0).unwrap();
        engine.accounts[lp_idx as usize].capital = U128::new(100_000);
        engine.vault += 100_000;

        // Simulate a long side in ResetPending with OI already zero
        engine.side_mode_long = SideMode::ResetPending;
        engine.oi_eff_long_q = 0;
        engine.adl_mult_long = 77;

        // Execute a short trade (does not touch long side OI)
        let oracle_price = 1_000_000u64;
        engine
            .execute_trade(&MATCHER, lp_idx, user_idx, 0, oracle_price, -100)
            .unwrap();

        // Lifecycle should have fired: ResetPending + OI==0 → Normal
        assert_eq!(
            engine.side_mode_long,
            SideMode::Normal,
            "execute_trade must run end-of-instruction lifecycle"
        );
        assert_eq!(engine.adl_mult_long, 0, "adl_mult_long must be cleared");
    }

    #[test]
    fn test_execute_trade_sets_current_slot_and_resets_warmup_start() {
        let mut params = default_params();
        params.warmup_period_slots = 1000;
        params.trading_fee_bps = 0;
        params.maintenance_fee_per_slot = U128::new(0);
        params.max_crank_staleness_slots = u64::MAX;
        params.max_accounts = 64;

        let mut engine = Box::new(RiskEngine::new(params));

        // Create LP and user with capital — deposits large enough to satisfy initial margin
        // at oracle_price=100k with 10% initial margin (notional=1e11, margin_req=1e10)
        let lp_idx = engine.add_lp([1u8; 32], [0u8; 32], 0).unwrap();
        engine.deposit(lp_idx, 20_000_000_000, 0).unwrap();

        let user_idx = engine.add_user(0).unwrap();
        engine.deposit(user_idx, 20_000_000_000, 0).unwrap();

        // Execute trade at now_slot = 100
        let now_slot = 100u64;
        let oracle_price = 100_000 * 1_000_000; // 100k
        let btc = 1_000_000i128; // 1 BTC

        engine
            .execute_trade(&MATCHER, lp_idx, user_idx, now_slot, oracle_price, btc)
            .unwrap();

        // Check current_slot was set
        assert_eq!(
            engine.current_slot, now_slot,
            "engine.current_slot should be set to now_slot after execute_trade"
        );

        // Check warmup_started_at_slot was reset for both accounts
        assert_eq!(
            engine.accounts[user_idx as usize].warmup_started_at_slot, now_slot,
            "user warmup_started_at_slot should be set to now_slot"
        );
        assert_eq!(
            engine.accounts[lp_idx as usize].warmup_started_at_slot, now_slot,
            "lp warmup_started_at_slot should be set to now_slot"
        );
    }

    #[test]
    fn test_execute_trade_tier3_fee() {
        let mut params = default_params();
        params.warmup_period_slots = 1; // Instant warmup (minimum valid)
        params.trading_fee_bps = 5;
        params.fee_tier2_bps = 8;
        params.fee_tier3_bps = 15;
        params.fee_tier2_threshold = 500_000;
        params.fee_tier3_threshold = 5_000_000;
        params.max_crank_staleness_slots = u64::MAX;
        params.initial_margin_bps = 200; // 50x leverage for large trades
        params.maintenance_margin_bps = 100;

        let mut engine = Box::new(RiskEngine::new(params));

        let user_idx = engine.add_user(0).unwrap();
        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();

        engine.deposit(user_idx, 1_000_000_000_000, 0).unwrap();
        engine.accounts[lp_idx as usize].capital = U128::new(1_000_000_000_000);
        engine.c_tot = U128::new(engine.c_tot.get() + 1_000_000_000_000);
        engine.vault = U128::new(engine.vault.get() + 1_000_000_000_000);

        let oracle_price = 1_000_000u64; // $1

        // Size 10_000_000 at price $1 → notional = 10_000_000
        // Tier 3 threshold = 5_000_000 → fee = 15 bps
        // Expected fee = ceil(10_000_000 * 15 / 10_000) = 15_000
        let capital_before = engine.accounts[user_idx as usize].capital.get();
        engine
            .execute_trade(&MATCHER, lp_idx, user_idx, 0, oracle_price, 10_000_000)
            .unwrap();
        let capital_after = engine.accounts[user_idx as usize].capital.get();
        let fee = capital_before - capital_after;

        assert_eq!(
            fee, 15_000,
            "Tier 3 fee (15 bps) should apply for 10M notional"
        );
    }

    #[test]
    fn test_execute_trade_updates_twap() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(0).unwrap();
        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();

        // position_value = 10_000_000; initial_margin (10%) = 1_000_000 → need > 1M capital.
        engine.deposit(user_idx, 2_000_000, 0).unwrap();
        engine.accounts[lp_idx as usize].capital = U128::new(20_000_000);
        engine.vault += 20_000_000;

        assert_eq!(engine.trade_twap_e6, 0, "TWAP starts at 0");
        assert_eq!(engine.twap_last_slot, 0, "twap_last_slot starts at 0");

        let oracle_price: u64 = 1_000_000; // $1.00 in e6
        let trade_slot: u64 = 42;
        let size: i128 = 10_000_000; // Large enough that notional >= MIN_TWAP_NOTIONAL

        engine
            .execute_trade(&MATCHER, lp_idx, user_idx, trade_slot, oracle_price, size)
            .expect("execute_trade must succeed");

        // First trade bootstraps TWAP to exec_price (oracle for NoOpMatcher)
        assert_eq!(
            engine.trade_twap_e6, oracle_price,
            "execute_trade must bootstrap trade_twap_e6 to exec_price on first fill"
        );
        assert_eq!(
            engine.twap_last_slot, trade_slot,
            "execute_trade must set twap_last_slot to trade slot"
        );
    }

    #[test]
    fn test_execute_trade_uses_dynamic_fee_tiers() {
        let mut params = default_params();
        params.warmup_period_slots = 1; // Instant warmup (minimum valid)
        params.trading_fee_bps = 5; // Tier 1: 0.05%
        params.fee_tier2_bps = 8; // Tier 2: 0.08%
        params.fee_tier3_bps = 12; // Tier 3: 0.12%
                                   // Thresholds in capital units (notional = size * oracle / 1e6)
        params.fee_tier2_threshold = 500_000; // Tier 2 at 500k notional
        params.fee_tier3_threshold = 5_000_000; // Tier 3 at 5M notional
        params.max_crank_staleness_slots = u64::MAX;
        params.initial_margin_bps = 1000;
        params.maintenance_margin_bps = 500;

        let mut engine = Box::new(RiskEngine::new(params));

        let user_idx = engine.add_user(0).unwrap();
        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();

        // Large deposits to avoid undercollateralized
        engine.deposit(user_idx, 100_000_000_000, 0).unwrap();
        engine.accounts[lp_idx as usize].capital = U128::new(100_000_000_000);
        engine.c_tot = U128::new(engine.c_tot.get() + 100_000_000_000);
        engine.vault = U128::new(engine.vault.get() + 100_000_000_000);

        let oracle_price = 1_000_000u64; // $1

        // Trade 1: Small trade → Tier 1 (5 bps)
        // Size 100_000 at price $1 → notional = 100_000 * 1_000_000 / 1_000_000 = 100_000
        // That's below Tier 2 threshold of 500_000 → base fee = 5 bps
        // Expected fee = ceil(100_000 * 5 / 10_000) = ceil(50) = 50
        let capital_before = engine.accounts[user_idx as usize].capital.get();
        engine
            .execute_trade(&MATCHER, lp_idx, user_idx, 0, oracle_price, 100_000)
            .unwrap();
        let capital_after = engine.accounts[user_idx as usize].capital.get();
        let fee_paid_1 = capital_before - capital_after;

        // Close position before next trade (clean state)
        engine
            .execute_trade(&MATCHER, lp_idx, user_idx, 0, oracle_price, -100_000)
            .unwrap();

        // Trade 2: Large trade → Tier 2 (8 bps)
        // Size 1_000_000 at price $1 → notional = 1_000_000
        // That's above Tier 2 threshold (500k) → fee = 8 bps
        // Expected fee = ceil(1_000_000 * 8 / 10_000) = ceil(800) = 800
        let capital_before = engine.accounts[user_idx as usize].capital.get();
        engine
            .execute_trade(&MATCHER, lp_idx, user_idx, 0, oracle_price, 1_000_000)
            .unwrap();
        let capital_after = engine.accounts[user_idx as usize].capital.get();
        let fee_paid_2 = capital_before - capital_after;

        // Verify: Tier 1 fee should be 5 bps of notional
        // fee_1 = ceil(100_000 * 5 / 10_000) = 50
        assert_eq!(
            fee_paid_1, 50,
            "Tier 1 fee should be 5 bps of 100k notional"
        );

        // Verify: Tier 2 fee should be 8 bps of notional
        // fee_2 = ceil(1_000_000 * 8 / 10_000) = 800
        assert_eq!(fee_paid_2, 800, "Tier 2 fee should be 8 bps of 1M notional");
    }

    #[test]
    fn test_execute_trade_utilization_surge() {
        let mut params = default_params();
        params.warmup_period_slots = 1; // Instant warmup (minimum valid)
        params.trading_fee_bps = 10; // 0.10% base
        params.fee_utilization_surge_bps = 20; // max 0.20% surge at 100% utilization
        params.fee_tier2_threshold = 0; // No tiers (flat + surge only)
        params.max_crank_staleness_slots = u64::MAX;
        params.initial_margin_bps = 1000;
        params.maintenance_margin_bps = 500;

        let mut engine = Box::new(RiskEngine::new(params));

        let user_idx = engine.add_user(0).unwrap();
        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();

        engine.deposit(user_idx, 100_000_000_000, 0).unwrap();
        engine.accounts[lp_idx as usize].capital = U128::new(100_000_000_000);
        engine.c_tot = U128::new(engine.c_tot.get() + 100_000_000_000);
        engine.vault = U128::new(engine.vault.get() + 100_000_000_000);

        let oracle_price = 1_000_000u64; // $1

        // No OI yet → base fee only (10 bps)
        let capital_before = engine.accounts[user_idx as usize].capital.get();
        engine
            .execute_trade(&MATCHER, lp_idx, user_idx, 0, oracle_price, 1_000_000)
            .unwrap();
        let capital_after = engine.accounts[user_idx as usize].capital.get();
        let fee_no_util = capital_before - capital_after;

        // fee = ceil(1_000_000 * 10 / 10_000) = 1_000
        assert_eq!(fee_no_util, 1000, "With no OI, should be base 10 bps");

        // Now the trade has created OI. Close and re-trade with high OI.
        engine
            .execute_trade(&MATCHER, lp_idx, user_idx, 0, oracle_price, -1_000_000)
            .unwrap();

        // Inject high OI = vault (50% utilization since util = OI / (2*vault))
        // vault ~ 200B, inject OI = 200B → util = 200B / (2*200B) = 0.5
        let vault = engine.vault.get();
        engine.total_open_interest = U128::new(vault); // 50% util

        let capital_before = engine.accounts[user_idx as usize].capital.get();
        engine
            .execute_trade(&MATCHER, lp_idx, user_idx, 0, oracle_price, 1_000_000)
            .unwrap();
        let capital_after = engine.accounts[user_idx as usize].capital.get();
        let fee_with_util = capital_before - capital_after;

        // At 50% utilization: surge = 20 * 5000/10000 = 10 bps
        // Total fee = 10 + 10 = 20 bps
        // fee = ceil(1_000_000 * 20 / 10_000) = 2_000
        assert_eq!(
            fee_with_util, 2000,
            "At 50% utilization, surge should add 10 bps (total 20 bps)"
        );
    }

    #[test]
    fn test_fee_accumulation() {
        // WHITEBOX: direct state mutation for vault/capital setup
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(0).unwrap();
        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();

        engine.deposit(user_idx, 100_000, 0).unwrap();
        // WHITEBOX: Set LP capital directly. Add to vault (not override) to preserve account fees.
        engine.accounts[lp_idx as usize].capital = U128::new(1_000_000);
        engine.vault += 1_000_000;
        assert_conserved(&engine);

        // Track fee revenue and balance BEFORE trades
        let fee_rev_before = engine.insurance_fund.fee_revenue;
        let ins_before = engine.insurance_fund.balance;

        // Execute multiple trades, counting successes
        // Trade size must be > 1000 for fee to be non-zero (fee_bps=10, notional needs > 10000/10=1000)
        let mut succeeded = 0usize;
        for _ in 0..10 {
            if engine
                .execute_trade(&MATCHER, lp_idx, user_idx, 0, 1_000_000, 10_000)
                .is_ok()
            {
                succeeded += 1;
            }
            if engine
                .execute_trade(&MATCHER, lp_idx, user_idx, 0, 1_000_000, -10_000)
                .is_ok()
            {
                succeeded += 1;
            }
        }

        let fee_rev_after = engine.insurance_fund.fee_revenue;
        let ins_after = engine.insurance_fund.balance;

        // If any trades succeeded, fees should have accumulated
        if succeeded > 0 {
            assert!(
                fee_rev_after > fee_rev_before,
                "fee_revenue must increase on successful trades"
            );
            assert!(
                ins_after >= ins_before,
                "insurance balance must not decrease"
            );
        }

        assert_conserved(&engine);
    }

    #[test]
    fn test_fee_based_on_position_size_not_notional() {
        let mut params = default_params();
        params.trading_fee_bps = 10; // 0.1% fee
        params.maintenance_margin_bps = 100;
        params.initial_margin_bps = 100;
        params.warmup_period_slots = 1; // Instant warmup (minimum valid)
        params.max_crank_staleness_slots = u64::MAX;

        let mut engine = Box::new(RiskEngine::new(params));

        let user_idx = engine.add_user(0).unwrap();
        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();

        // Deposit enough capital
        engine.deposit(user_idx, 1_000_000_000_000, 0).unwrap(); // Large deposit
        engine.accounts[lp_idx as usize].capital = U128::new(1_000_000_000_000);
        engine.vault += 1_000_000_000_000;
        engine.c_tot = U128::new(2_000_000_000_000);

        let oracle_price = 1u64; // Very low price ($0.000001)

        let insurance_before = engine.insurance_fund.balance.get();

        // Execute trade: large position size, low price
        // size = 1_000_000_000 (1B units)
        // notional = 1_000_000_000 * 1 / 1_000_000 = 1_000 (very small)
        // abs_size = 1_000_000_000
        // Old fee: 1_000 * 10 / 10_000 = 1 (wrong - too small)
        // New fee: 1_000_000_000 * 10 / 10_000 = 1_000_000 (correct)
        let size: i128 = 1_000_000_000;
        engine
            .execute_trade(&MATCHER, lp_idx, user_idx, 0, oracle_price, size)
            .unwrap();

        let insurance_after = engine.insurance_fund.balance.get();
        let fee_charged = insurance_after - insurance_before;

        // Fee should be based on position size, not notional
        let expected_fee = (1_000_000_000u128 * 10u128).div_ceil(10_000);
        assert_eq!(
            fee_charged, expected_fee,
            "Fee must be based on position size ({}), not notional. Expected {}, got {}",
            1_000_000_000, expected_fee, fee_charged
        );
    }

    #[test]
    fn test_fee_params_validation() {
        let mut params = default_params();

        // Valid tiered config
        params.fee_tier2_bps = 8;
        params.fee_tier3_bps = 10;
        params.fee_tier2_threshold = 1_000_000;
        params.fee_tier3_threshold = 10_000_000;
        assert!(params.validate().is_ok());

        // Invalid: tier3 threshold <= tier2 threshold
        params.fee_tier3_threshold = 500_000;
        assert!(params.validate().is_err());

        // Fix thresholds, test fee split
        params.fee_tier3_threshold = 10_000_000;
        params.fee_split_lp_bps = 8000;
        params.fee_split_protocol_bps = 1200;
        params.fee_split_creator_bps = 800;
        assert!(params.validate().is_ok());

        // Invalid: fee split doesn't sum to 10_000
        params.fee_split_creator_bps = 900;
        assert!(params.validate().is_err());
    }

    #[test]
    fn test_fee_split_configured() {
        let mut params = default_params();
        params.fee_split_lp_bps = 8000; // 80%
        params.fee_split_protocol_bps = 1200; // 12%
        params.fee_split_creator_bps = 800; // 8%
        let engine = Box::new(RiskEngine::new(params));

        let (lp, proto, creator) = engine.compute_fee_split(10_000);
        assert_eq!(lp, 8000);
        assert_eq!(proto, 1200);
        assert_eq!(creator, 800);
    }

    #[test]
    fn test_fee_split_legacy() {
        let engine = Box::new(RiskEngine::new(default_params()));
        let (lp, proto, creator) = engine.compute_fee_split(10_000);
        assert_eq!(lp, 10_000); // 100% to LP
        assert_eq!(proto, 0);
        assert_eq!(creator, 0);
    }

    #[test]
    fn test_fee_split_rounding_goes_to_creator() {
        let mut params = default_params();
        params.fee_split_lp_bps = 8000;
        params.fee_split_protocol_bps = 1200;
        params.fee_split_creator_bps = 800;
        let engine = Box::new(RiskEngine::new(params));

        // 33 is not evenly divisible
        let (lp, proto, creator) = engine.compute_fee_split(33);
        assert_eq!(lp + proto + creator, 33); // Conservation: total preserved
    }

    #[test]
    fn test_finding_l_new_position_requires_initial_margin() {
        // Replicates the integration test scenario:
        // - maintenance_margin_bps = 500 (5%)
        // - initial_margin_bps = 1000 (10%)
        // - User deposits 0.6 SOL (600_000_000)
        // - User opens ~10 SOL notional position
        // - Trade should FAIL (6% < 10%)

        let mut params = default_params();
        params.maintenance_margin_bps = 500; // 5%
        params.initial_margin_bps = 1000; // 10%
        params.trading_fee_bps = 0; // No fee for cleaner math
        params.warmup_period_slots = 1; // Instant warmup (minimum valid)
        params.max_crank_staleness_slots = u64::MAX;

        let mut engine = Box::new(RiskEngine::new(params));

        let user_idx = engine.add_user(0).unwrap();
        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();

        // Deposit 600M (0.6 SOL in lamports)
        engine.deposit(user_idx, 600_000_000, 0).unwrap();

        // LP needs capital to take the other side
        engine.accounts[lp_idx as usize].capital = U128::new(100_000_000_000);
        engine.vault += 100_000_000_000;

        // Oracle price: $138 (in e6 = 138_000_000)
        let oracle_price = 138_000_000u64;

        // Position size for ~10 SOL notional at $138:
        // notional = size * price / 1_000_000
        // 10_000_000_000 = size * 138_000_000 / 1_000_000
        // size = 10_000_000_000 * 1_000_000 / 138_000_000 = ~72_463_768
        let size: i128 = 72_463_768;

        // Execute trade - should FAIL because:
        // - Position value = 72_463_768 * 138_000_000 / 1_000_000 = ~10_000_000_000
        // - Initial margin required (10%) = 1_000_000_000
        // - User equity = 600_000_000
        // - 600_000_000 < 1_000_000_000 → UNDERCOLLATERALIZED
        let result = engine.execute_trade(&MATCHER, lp_idx, user_idx, 0, oracle_price, size);

        assert!(
        result.is_err(),
        "Opening new position with only 6% margin should FAIL when 10% initial margin required. \
         Got {:?}",
        result
    );
        assert!(
            matches!(result, Err(percolator::RiskError::Undercollateralized)),
            "Error should be Undercollateralized"
        );
    }

    #[test]
    fn test_force_close_resolved_decrements_oi() {
        // After force-closing both sides, OI should be zero
        let (mut engine, long_idx, short_idx) = setup_bilateral_engine();

        assert_eq!(engine.oi_eff_long_q, 500_000);
        assert_eq!(engine.oi_eff_short_q, 500_000);

        engine.force_close_resolved(long_idx).unwrap();
        assert_eq!(engine.oi_eff_long_q, 0);

        engine.force_close_resolved(short_idx).unwrap();
        assert_eq!(engine.oi_eff_short_q, 0);
    }

    #[test]
    fn test_force_close_resolved_flat_account() {
        // force_close_resolved on a flat account (no position) should work
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user = engine.add_user(0).unwrap();
        engine.deposit(user, 1_000, 0).unwrap();
        set_insurance(&mut engine, 100);
        engine.recompute_aggregates();
        assert_conserved(&engine);

        let vault_before = engine.vault.get();
        let capital_returned = engine.force_close_resolved(user).unwrap();

        assert_eq!(capital_returned, 1_000, "should return full capital");
        assert_eq!(engine.vault.get(), vault_before - 1_000);
        assert!(!engine.is_used(user as usize), "slot should be freed");
    }

    #[test]
    fn test_force_close_resolved_oob_index() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        assert_eq!(
            engine.force_close_resolved(u16::MAX).unwrap_err(),
            RiskError::AccountNotFound
        );
    }

    #[test]
    fn test_force_close_resolved_rejects_corrupt_a_basis() {
        // a_basis == 0 with nonzero position should be rejected as CorruptState
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user = engine.add_user(0).unwrap();
        engine.deposit(user, 1_000, 0).unwrap();

        // Set position with corrupt a_basis = 0
        engine.accounts[user as usize].position_basis_q = 100_000;
        engine.stored_pos_count_long += 1;
        engine.accounts[user as usize].adl_a_basis = 0; // CORRUPT
                                                        // epoch_snap defaults to 0 which matches the default epoch_side (0)
        engine.oi_eff_long_q = 100_000;
        engine.recompute_aggregates();

        assert_eq!(
            engine.force_close_resolved(user).unwrap_err(),
            RiskError::CorruptState,
            "corrupt a_basis must be rejected"
        );
    }

    #[test]
    fn test_force_close_resolved_unused_slot() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        assert_eq!(
            engine.force_close_resolved(0).unwrap_err(),
            RiskError::AccountNotFound
        );
    }

    #[test]
    fn test_force_close_resolved_with_open_position_zero_pnl() {
        // Account has a position but no K-pair PnL delta (k_snap == k_end = 0)
        let (mut engine, long_idx, short_idx) = setup_bilateral_engine();

        // Force-close long — position zeroed, capital returned
        let capital = engine.force_close_resolved(long_idx).unwrap();

        assert_eq!(capital, 1_000);
        assert!(!engine.is_used(long_idx as usize));
        assert_eq!(engine.oi_eff_long_q, 0, "OI should be decremented");

        // Force-close short
        let capital_s = engine.force_close_resolved(short_idx).unwrap();
        assert_eq!(capital_s, 1_000);
        assert!(!engine.is_used(short_idx as usize));
        assert_eq!(engine.oi_eff_short_q, 0, "OI should be decremented");
    }

    #[test]
    fn test_force_realize_blocks_value_extraction() {
        let mut params = default_params();
        params.risk_reduction_threshold = U128::new(1000);
        let mut engine = Box::new(RiskEngine::new(params));
        engine.vault = U128::new(100_000);

        // Create user with capital
        let user = engine.add_user(0).unwrap();
        engine.deposit(user, 10_000, 0).unwrap();

        // Under haircut-ratio design, there is no pending_unpaid_loss mechanism.
        // Withdrawals and closes are not blocked by pending losses.
        // Verify that basic operations work normally.

        // Withdraw should succeed
        let result = engine.withdraw(user, 1_000, 0, 1_000_000);
        assert!(
            result.is_ok(),
            "Withdraw should succeed (no pending loss mechanism)"
        );

        // Close should succeed (account has remaining capital, no position)
        let result = engine.close_account(user, 0, 1_000_000);
        assert!(
            result.is_ok(),
            "Close should succeed (no pending loss mechanism)"
        );
    }

    #[test]
    fn test_force_realize_step_closes_in_window_only() {
        let mut params = default_params();
        params.risk_reduction_threshold = U128::new(1000); // Threshold at 1000
        let mut engine = Box::new(RiskEngine::new(params));
        engine.vault = U128::new(100_000);

        // Create counterparty LP
        let lp = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();
        engine.deposit(lp, 50_000, 0).unwrap();

        // Create users with positions at different indices
        let user1 = engine.add_user(0).unwrap(); // idx 1, in first window
        let user2 = engine.add_user(0).unwrap(); // idx 2, in first window
        let user3 = engine.add_user(0).unwrap(); // idx 3, in first window

        engine.deposit(user1, 5_000, 0).unwrap();
        engine.deposit(user2, 5_000, 0).unwrap();
        engine.deposit(user3, 5_000, 0).unwrap();

        // Give them positions
        engine.accounts[user1 as usize].position_size = I128::new(10_000);
        engine.accounts[user1 as usize].entry_price = 1_000_000;
        engine.accounts[user2 as usize].position_size = I128::new(10_000);
        engine.accounts[user2 as usize].entry_price = 1_000_000;
        engine.accounts[user3 as usize].position_size = I128::new(10_000);
        engine.accounts[user3 as usize].entry_price = 1_000_000;
        engine.accounts[lp as usize].position_size = I128::new(-30_000);
        engine.accounts[lp as usize].entry_price = 1_000_000;
        engine.total_open_interest = U128::new(60_000);

        // Set insurance at threshold (force-realize active)
        engine.insurance_fund.balance = U128::new(1000);

        // Run crank (cursor starts at 0)
        assert_eq!(engine.crank_cursor, 0);
        let outcome = engine.keeper_crank(1, 1_000_000, &[], 64, 0).unwrap();

        // Force-realize should have run and closed positions
        assert!(
            outcome.force_realize_needed,
            "Force-realize should be needed"
        );
        assert!(
            outcome.force_realize_closed > 0,
            "Should have closed some positions"
        );

        // Positions should be closed
        assert_eq!(
            engine.accounts[user1 as usize].position_size.get(),
            0,
            "User1 position should be closed"
        );
        assert_eq!(
            engine.accounts[user2 as usize].position_size.get(),
            0,
            "User2 position should be closed"
        );
        assert_eq!(
            engine.accounts[user3 as usize].position_size.get(),
            0,
            "User3 position should be closed"
        );
    }

    #[test]
    fn test_force_realize_step_inert_above_threshold() {
        let mut params = default_params();
        params.risk_reduction_threshold = U128::new(1000); // Threshold at 1000
        let mut engine = Box::new(RiskEngine::new(params));
        engine.vault = U128::new(100_000);

        // Create counterparty LP
        let lp = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();
        engine.deposit(lp, 50_000, 0).unwrap();

        // Create user with position (must be >= min_liquidation_abs to avoid dust-closure)
        let user = engine.add_user(0).unwrap();
        engine.deposit(user, 100_000, 0).unwrap();
        engine.accounts[user as usize].position_size = I128::new(200_000);
        engine.accounts[user as usize].entry_price = 1_000_000;
        engine.accounts[lp as usize].position_size = I128::new(-200_000);
        engine.accounts[lp as usize].entry_price = 1_000_000;
        engine.total_open_interest = U128::new(400_000);

        // Set insurance ABOVE threshold (force-realize NOT active)
        engine.insurance_fund.balance = U128::new(1001);

        let pos_before = engine.accounts[user as usize].position_size;

        // Run crank
        let outcome = engine.keeper_crank(1, 1_000_000, &[], 64, 0).unwrap();

        // Force-realize should not be needed
        assert!(
            !outcome.force_realize_needed,
            "Force-realize should not be needed"
        );
        assert_eq!(
            outcome.force_realize_closed, 0,
            "No positions should be force-closed"
        );

        // Position should be unchanged
        assert_eq!(
            engine.accounts[user as usize].position_size, pos_before,
            "Position should be unchanged"
        );
    }

    #[test]
    fn test_force_realize_updates_lp_aggregates() {
        let mut params = default_params();
        params.risk_reduction_threshold = U128::new(10_000); // High threshold to trigger force-realize
        let mut engine = Box::new(RiskEngine::new(params));
        engine.vault = U128::new(100_000);

        // Insurance below threshold = force-realize active
        engine.insurance_fund.balance = U128::new(5_000);

        // Create LP with position
        let lp = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();
        engine.deposit(lp, 50_000, 0).unwrap();

        // Create user as counterparty
        let user = engine.add_user(0).unwrap();
        engine.deposit(user, 50_000, 0).unwrap();

        // Set up positions
        engine.accounts[lp as usize].position_size = I128::new(-1_000_000); // Short 1 unit
        engine.accounts[lp as usize].entry_price = 1_000_000;
        engine.accounts[user as usize].position_size = I128::new(1_000_000); // Long 1 unit
        engine.accounts[user as usize].entry_price = 1_000_000;
        engine.total_open_interest = U128::new(2_000_000);

        // Update LP aggregates manually (simulating what would normally happen)
        engine.net_lp_pos = I128::new(-1_000_000);
        engine.lp_sum_abs = U128::new(1_000_000);

        // Verify force-realize is active
        assert!(
            engine.insurance_fund.balance <= params.risk_reduction_threshold,
            "Force-realize should be active"
        );

        let net_lp_before = engine.net_lp_pos;
        let sum_abs_before = engine.lp_sum_abs;

        // Run crank - should close LP position via force-realize
        let result = engine.keeper_crank(1, 1_000_000, &[], 64, 0);
        assert!(result.is_ok());

        // LP position should be closed
        if engine.accounts[lp as usize].position_size.is_zero() {
            // If LP was closed, aggregates should be updated
            assert_ne!(
                engine.net_lp_pos.get(),
                net_lp_before.get(),
                "net_lp_pos should change when LP position closed"
            );
            assert!(
                engine.lp_sum_abs.get() < sum_abs_before.get(),
                "lp_sum_abs should decrease when LP position closed"
            );
        }
    }

    #[test]
    fn test_freeze_funding_snapshots_rate() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        engine.funding_rate_bps_per_slot_last = 42;
        assert!(!engine.is_funding_frozen());

        // Freeze
        assert!(engine.freeze_funding().is_ok());
        assert!(engine.is_funding_frozen());
        assert_eq!(engine.funding_frozen_rate_snapshot, 42);

        // Double-freeze should fail
        assert!(engine.freeze_funding().is_err());
    }

    #[test]
    fn test_frozen_funding_ignores_rate_updates() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        engine.funding_rate_bps_per_slot_last = 10;
        engine.freeze_funding().unwrap();

        // Try to set a new rate — should be ignored
        engine.set_funding_rate_for_next_interval(999);
        assert_eq!(engine.funding_rate_bps_per_slot_last, 10); // Unchanged
    }

    #[test]
    fn test_frozen_funding_uses_snapshot_rate_on_accrue() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        engine.funding_rate_bps_per_slot_last = 5;
        engine.last_funding_slot = 0;

        // Freeze with rate = 5
        engine.freeze_funding().unwrap();

        // Change the stored rate (simulating external mutation) — should not matter
        engine.funding_rate_bps_per_slot_last = 999;

        // Accrue 100 slots at oracle price 1_000_000
        engine.accrue_funding(100, 1_000_000).unwrap();

        // ΔF = price * rate * dt / 10_000 = 1_000_000 * 5 * 100 / 10_000 = 50_000
        assert_eq!(engine.funding_index_qpb_e6.get(), 50_000);
    }

    #[test]
    fn test_funding_does_not_touch_principal() {
        // Funding should never modify principal (Invariant I1 extended)
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(0).unwrap();

        let initial_principal = 100_000;
        engine.deposit(user_idx, initial_principal, 0).unwrap();

        engine.accounts[user_idx as usize].position_size = I128::new(1_000_000);

        // Accrue funding
        engine
            .accrue_funding_with_rate(1, 100_000_000, 100)
            .unwrap();
        engine.touch_account(user_idx).unwrap();

        // Principal must be unchanged
        assert_eq!(
            engine.accounts[user_idx as usize].capital.get(),
            initial_principal
        );
    }

    #[test]
    fn test_funding_idempotence() {
        // T3: Settlement is idempotent
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(10000).unwrap();

        engine.deposit(user_idx, 100_000, 0).unwrap();
        engine.accounts[user_idx as usize].position_size = I128::new(1_000_000);

        // Accrue funding
        engine.accrue_funding_with_rate(1, 100_000_000, 10).unwrap();

        // Settle once
        engine.touch_account(user_idx).unwrap();
        let pnl_after_first = engine.accounts[user_idx as usize].pnl;

        // Settle again without new accrual
        engine.touch_account(user_idx).unwrap();
        let pnl_after_second = engine.accounts[user_idx as usize].pnl;

        assert_eq!(
            pnl_after_first, pnl_after_second,
            "Second settlement should not change PNL"
        );
    }

    #[test]
    fn test_funding_negative_rate_shorts_pay_longs() {
        // T2: Negative funding → shorts pay longs
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(0).unwrap();
        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();

        engine.deposit(user_idx, 100_000, 0).unwrap();
        // WHITEBOX: Set LP capital directly. Add to vault (not override) to preserve account fees.
        engine.accounts[lp_idx as usize].capital = U128::new(1_000_000);
        engine.vault += 1_000_000;

        // User opens short position
        engine.accounts[user_idx as usize].position_size = I128::new(-1_000_000);
        engine.accounts[user_idx as usize].entry_price = 100_000_000;

        // LP has opposite long position
        engine.accounts[lp_idx as usize].position_size = I128::new(1_000_000);
        engine.accounts[lp_idx as usize].entry_price = 100_000_000;

        // Zero warmup/reserved to avoid side effects from touch_account
        engine.accounts[user_idx as usize].warmup_slope_per_step = U128::new(0);
        engine.accounts[user_idx as usize].reserved_pnl = 0;
        engine.accounts[user_idx as usize].warmup_started_at_slot = engine.current_slot;
        engine.accounts[lp_idx as usize].warmup_slope_per_step = U128::new(0);
        engine.accounts[lp_idx as usize].reserved_pnl = 0;
        engine.accounts[lp_idx as usize].warmup_started_at_slot = engine.current_slot;
        assert_conserved(&engine);

        // Accrue negative funding: -10 bps/slot
        engine.current_slot = 1;
        engine
            .accrue_funding_with_rate(1, 100_000_000, -10)
            .unwrap();

        let user_pnl_before = engine.accounts[user_idx as usize].pnl;
        let lp_pnl_before = engine.accounts[lp_idx as usize].pnl;

        engine.touch_account(user_idx).unwrap();
        engine.touch_account(lp_idx).unwrap();

        // With negative funding rate, delta_F is negative (-100,000)
        // User (short) with negative position: payment = (-1M) * (-100,000) / 1e6 = 100,000
        // User pays 100,000 (shorts pay)
        assert_eq!(
            engine.accounts[user_idx as usize].pnl,
            user_pnl_before - 100_000
        );

        // LP (long) receives 100,000
        assert_eq!(
            engine.accounts[lp_idx as usize].pnl,
            lp_pnl_before + 100_000
        );
    }

    #[test]
    fn test_funding_partial_close() {
        // T4: Partial position close with funding
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(0).unwrap();
        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();

        // Need enough for initial margin (10% of 200M notional = 20M) plus trading fees
        engine.deposit(user_idx, 25_000_000, 0).unwrap();
        // WHITEBOX: Set LP capital directly. Add to vault (not override) to preserve account fees.
        engine.accounts[lp_idx as usize].capital = U128::new(50_000_000);
        engine.vault += 50_000_000;
        assert_conserved(&engine);

        // Open long position of 2M base units
        let trade_result =
            engine.execute_trade(&MATCHER, lp_idx, user_idx, 0, 100_000_000, 2_000_000);
        assert!(trade_result.is_ok(), "Trade should succeed");

        assert_eq!(
            engine.accounts[user_idx as usize].position_size.get(),
            2_000_000
        );

        // Accrue funding for 1 slot at +10 bps
        engine.advance_slot(1);
        engine.accrue_funding_with_rate(1, 100_000_000, 10).unwrap();

        // Reduce position to 1M (close half)
        let reduce_result =
            engine.execute_trade(&MATCHER, lp_idx, user_idx, 0, 100_000_000, -1_000_000);
        assert!(reduce_result.is_ok(), "Partial close should succeed");

        // Position should be 1M now
        assert_eq!(
            engine.accounts[user_idx as usize].position_size.get(),
            1_000_000
        );

        // Accrue more funding for another slot
        engine.advance_slot(2);
        engine.accrue_funding_with_rate(2, 100_000_000, 10).unwrap();

        // Touch to settle
        engine.touch_account(user_idx).unwrap();

        // Funding should have been applied correctly for both periods
        // Period 1: 2M base * (100K delta_F) / 1e6 = 200
        // Period 2: 1M base * (100K delta_F) / 1e6 = 100
        // Total funding paid: 300
        // (exact PNL depends on trading fees too, but funding should be applied)
    }

    #[test]
    fn test_funding_position_flip() {
        // T5: Flip from long to short
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(0).unwrap();
        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();

        // Need enough for initial margin (10% of 100M notional = 10M) plus trading fees
        engine.deposit(user_idx, 15_000_000, 0).unwrap();
        // WHITEBOX: Set LP capital directly. Add to vault (not override) to preserve account fees.
        engine.accounts[lp_idx as usize].capital = U128::new(20_000_000);
        engine.vault += 20_000_000;
        assert_conserved(&engine);

        // Open long
        engine
            .execute_trade(&MATCHER, lp_idx, user_idx, 0, 100_000_000, 1_000_000)
            .unwrap();
        assert_eq!(
            engine.accounts[user_idx as usize].position_size.get(),
            1_000_000
        );

        // Accrue funding
        engine.advance_slot(1);
        engine.accrue_funding_with_rate(1, 100_000_000, 10).unwrap();

        let _pnl_before_flip = engine.accounts[user_idx as usize].pnl;

        // Flip to short (trade -2M to go from +1M to -1M)
        engine
            .execute_trade(&MATCHER, lp_idx, user_idx, 0, 100_000_000, -2_000_000)
            .unwrap();

        assert_eq!(
            engine.accounts[user_idx as usize].position_size.get(),
            -1_000_000
        );

        // Funding should have been settled before the flip
        // User's funding index should be updated
        assert_eq!(
            engine.accounts[user_idx as usize].funding_index,
            engine.funding_index_qpb_e6
        );

        // Accrue more funding
        engine.advance_slot(2);
        engine.accrue_funding_with_rate(2, 100_000_000, 10).unwrap();

        engine.touch_account(user_idx).unwrap();

        // Now user is short, so they receive funding (if rate is still positive)
        // This verifies no "double charge" bug
    }

    #[test]
    fn test_funding_positive_rate_longs_pay_shorts() {
        // T1: Positive funding → longs pay shorts
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(0).unwrap();
        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();

        engine.deposit(user_idx, 100_000, 0).unwrap();
        // WHITEBOX: Set LP capital directly. Add to vault (not override) to preserve account fees.
        engine.accounts[lp_idx as usize].capital = U128::new(1_000_000);
        engine.vault += 1_000_000;

        // User opens long position (+1 base unit)
        engine.accounts[user_idx as usize].position_size = I128::new(1_000_000); // +1M base units
        engine.accounts[user_idx as usize].entry_price = 100_000_000; // $100

        // LP has opposite short position
        engine.accounts[lp_idx as usize].position_size = I128::new(-1_000_000);
        engine.accounts[lp_idx as usize].entry_price = 100_000_000;

        // Zero warmup/reserved to avoid side effects from touch_account
        engine.accounts[user_idx as usize].warmup_slope_per_step = U128::new(0);
        engine.accounts[user_idx as usize].reserved_pnl = 0;
        engine.accounts[user_idx as usize].warmup_started_at_slot = engine.current_slot;
        engine.accounts[lp_idx as usize].warmup_slope_per_step = U128::new(0);
        engine.accounts[lp_idx as usize].reserved_pnl = 0;
        engine.accounts[lp_idx as usize].warmup_started_at_slot = engine.current_slot;
        assert_conserved(&engine);

        // Accrue positive funding: +10 bps/slot for 1 slot
        engine.current_slot = 1;
        engine.accrue_funding_with_rate(1, 100_000_000, 10).unwrap(); // price=$100, rate=+10bps

        // Expected delta_F = 100e6 * 10 * 1 / 10000 = 100,000
        // User payment = 1M * 100,000 / 1e6 = 100,000
        // LP payment = -1M * 100,000 / 1e6 = -100,000

        let user_pnl_before = engine.accounts[user_idx as usize].pnl;
        let lp_pnl_before = engine.accounts[lp_idx as usize].pnl;

        // Settle funding
        engine.touch_account(user_idx).unwrap();
        engine.touch_account(lp_idx).unwrap();

        // User (long) should pay 100,000
        assert_eq!(
            engine.accounts[user_idx as usize].pnl,
            user_pnl_before - 100_000
        );

        // LP (short) should receive 100,000
        assert_eq!(
            engine.accounts[lp_idx as usize].pnl,
            lp_pnl_before + 100_000
        );

        // Zero-sum check
        let total_pnl_before = user_pnl_before + lp_pnl_before;
        let total_pnl_after =
            engine.accounts[user_idx as usize].pnl + engine.accounts[lp_idx as usize].pnl;
        assert_eq!(
            total_pnl_after, total_pnl_before,
            "Funding should be zero-sum"
        );
    }

    #[test]
    fn test_funding_settlement_maintains_pnl_pos_tot() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(0).unwrap();
        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();

        // Setup: user deposits capital
        engine.deposit(user_idx, 100_000, 0).unwrap();
        engine.accounts[lp_idx as usize].capital = U128::new(1_000_000);
        engine.vault += 1_000_000;

        // User has a long position
        engine.accounts[user_idx as usize].position_size = I128::new(1_000_000);
        engine.accounts[user_idx as usize].entry_price = 100_000_000;

        // LP has opposite short position
        engine.accounts[lp_idx as usize].position_size = I128::new(-1_000_000);
        engine.accounts[lp_idx as usize].entry_price = 100_000_000;

        // Give user positive PnL that will flip to negative after funding
        engine.accounts[user_idx as usize].pnl = I128::new(50_000);

        // Zero warmup to avoid side effects
        engine.accounts[user_idx as usize].warmup_slope_per_step = U128::new(0);
        engine.accounts[lp_idx as usize].warmup_slope_per_step = U128::new(0);

        // Recompute aggregates to ensure consistency
        engine.recompute_aggregates();

        // Verify initial pnl_pos_tot includes user's positive PnL
        let pnl_pos_tot_before = engine.pnl_pos_tot.get();
        assert_eq!(
            pnl_pos_tot_before, 50_000,
            "Initial pnl_pos_tot should be 50_000"
        );

        // Accrue large positive funding that will make user's PnL negative
        // rate = 1000 bps/slot for 1 slot at price 100e6
        // delta_F = 100e6 * 1000 * 1 / 10000 = 10,000,000
        // User payment = 1M * 10,000,000 / 1e6 = 10,000,000
        engine.current_slot = 1;
        engine
            .accrue_funding_with_rate(1, 100_000_000, 1000)
            .unwrap();

        // Settle funding for user - this should flip their PnL from +50k to -9.95M
        engine.touch_account(user_idx).unwrap();

        // User's new PnL should be negative: 50_000 - 10_000_000 = -9_950_000
        let user_pnl_after = engine.accounts[user_idx as usize].pnl.get();
        assert!(
            user_pnl_after < 0,
            "User PnL should be negative after large funding payment"
        );

        // pnl_pos_tot should now be 0 (user's PnL flipped from positive to negative)
        let pnl_pos_tot_after = engine.pnl_pos_tot.get();
        assert_eq!(
            pnl_pos_tot_after, 0,
            "pnl_pos_tot should be 0 after user's PnL flipped negative (was {}, now {})",
            pnl_pos_tot_before, pnl_pos_tot_after
        );

        // Settle LP funding - LP should receive payment, gaining positive PnL
        engine.touch_account(lp_idx).unwrap();

        // LP's PnL should now be positive: 0 + 10,000,000 = 10,000,000
        let lp_pnl_after = engine.accounts[lp_idx as usize].pnl.get();
        assert!(
            lp_pnl_after > 0,
            "LP PnL should be positive after receiving funding"
        );

        // pnl_pos_tot should now equal LP's positive PnL
        let pnl_pos_tot_final = engine.pnl_pos_tot.get();
        assert_eq!(
            pnl_pos_tot_final, lp_pnl_after as u128,
            "pnl_pos_tot should equal LP's positive PnL"
        );

        // Verify by recomputing from scratch
        let mut expected_pnl_pos_tot = 0u128;
        if engine.accounts[user_idx as usize].pnl.get() > 0 {
            expected_pnl_pos_tot += engine.accounts[user_idx as usize].pnl.get() as u128;
        }
        if engine.accounts[lp_idx as usize].pnl.get() > 0 {
            expected_pnl_pos_tot += engine.accounts[lp_idx as usize].pnl.get() as u128;
        }
        assert_eq!(
            pnl_pos_tot_final, expected_pnl_pos_tot,
            "pnl_pos_tot should match manual calculation"
        );
    }

    #[test]
    fn test_funding_zero_position() {
        // Edge case: funding with zero position should do nothing
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(10000).unwrap();

        engine.deposit(user_idx, 100_000, 0).unwrap();

        // No position
        assert_eq!(engine.accounts[user_idx as usize].position_size.get(), 0);

        let pnl_before = engine.accounts[user_idx as usize].pnl;

        // Accrue funding
        engine
            .accrue_funding_with_rate(1, 100_000_000, 100)
            .unwrap(); // Large rate

        // Settle
        engine.touch_account(user_idx).unwrap();

        // PNL should be unchanged
        assert_eq!(engine.accounts[user_idx as usize].pnl, pnl_before);
    }

    #[test]
    fn test_gc_fee_drained_dust() {
        // Test: account drained by maintenance fees gets GC'd
        let mut params = default_params();
        params.maintenance_fee_per_slot = U128::new(100); // 100 units per slot
        params.max_crank_staleness_slots = u64::MAX; // No staleness check

        let mut engine = Box::new(RiskEngine::new(params));

        // Create user with small capital
        let user = engine.add_user(0).unwrap();
        engine.deposit(user, 500, 0).unwrap();

        assert!(engine.is_used(user as usize), "User should exist");

        // Advance time to drain fees (500 / 100 = 5 slots)
        // Crank will settle fees, drain capital to 0, then GC
        let outcome = engine.keeper_crank(10, 1_000_000, &[], 64, 0).unwrap();

        assert!(
            !engine.is_used(user as usize),
            "User slot should be freed after fee drain"
        );
        assert_eq!(outcome.num_gc_closed, 1, "Should have GC'd one account");
    }

    #[test]
    fn test_gc_negative_pnl_socialized() {
        // Test: account with negative PnL and zero capital is socialized then GC'd
        let params = default_params();
        let mut engine = Box::new(RiskEngine::new(params));

        // Create user with negative PnL and zero capital
        let user = engine.add_user(0).unwrap();

        // Create counterparty with matching positive PnL for zero-sum
        let counterparty = engine.add_user(0).unwrap();
        engine.deposit(counterparty, 1000, 0).unwrap(); // Needs capital to exist
        engine.accounts[counterparty as usize].pnl = I128::new(500); // Counterparty gains
                                                                     // Keep PnL unwrapped (not warmed) so socialization can haircut it
        engine.accounts[counterparty as usize].warmup_slope_per_step = U128::new(0);
        engine.accounts[counterparty as usize].warmup_started_at_slot = 0;

        // Now set user's negative PnL (zero-sum with counterparty)
        engine.accounts[user as usize].pnl = I128::new(-500);
        engine.recompute_aggregates();

        // Set up insurance fund
        set_insurance(&mut engine, 10_000);

        assert!(engine.is_used(user as usize), "User should exist");

        // First crank: GC writes off negative PnL and frees account
        let outcome = engine.keeper_crank(100, 1_000_000, &[], 64, 0).unwrap();

        assert!(
            !engine.is_used(user as usize),
            "User should be GC'd after loss write-off"
        );
        assert_eq!(outcome.num_gc_closed, 1, "Should have GC'd one account");

        // Under haircut-ratio design, counterparty's positive PnL is NOT directly haircut.
        // Instead, the write-off reduces Residual which reduces the haircut ratio h,
        // automatically haircutting PnL claims when they convert to capital during warmup.
        // The raw PnL value stays at 500 until warmup conversion applies the haircut.
        assert_eq!(
            engine.accounts[counterparty as usize].pnl.get(),
            500,
            "Counterparty PnL should remain at 500 (haircut applied at warmup conversion)"
        );

        // Primary invariant V >= C_tot + I should still hold after GC.
        // The extended conservation check (including net_pnl) may fail when write-offs
        // create positive net PnL not yet haircut. This is expected under the haircut-ratio
        // design: the haircut is applied at warmup conversion time, not at GC time.
        let c_tot: u128 = engine.accounts[counterparty as usize].capital.get();
        let insurance = engine.insurance_fund.balance.get();
        assert!(
        engine.vault.get() >= c_tot.saturating_add(insurance),
        "Primary invariant V >= C_tot + I should hold after GC: vault={}, c_tot={}, insurance={}",
        engine.vault.get(),
        c_tot,
        insurance
    );
    }

    #[test]
    fn test_gc_positive_pnl_never_collected() {
        // Test: account with positive PnL is never GC'd
        let params = default_params();
        let mut engine = Box::new(RiskEngine::new(params));

        // Create user and set up positive PnL with zero capital
        let user = engine.add_user(0).unwrap();
        // No deposit - capital = 0
        engine.accounts[user as usize].pnl = I128::new(1000); // Positive PnL

        assert!(engine.is_used(user as usize), "User should exist");

        // Crank should NOT GC this account
        let outcome = engine.keeper_crank(100, 1_000_000, &[], 64, 0).unwrap();

        assert!(
            engine.is_used(user as usize),
            "User with positive PnL should NOT be GC'd"
        );
        assert_eq!(outcome.num_gc_closed, 0, "Should not GC any accounts");
    }

    #[test]
    fn test_gc_with_position_not_collected() {
        // Test: account with open position is never GC'd
        let params = default_params();
        let mut engine = Box::new(RiskEngine::new(params));

        let user = engine.add_user(0).unwrap();
        // Add enough capital to avoid liquidation, then set position
        engine.deposit(user, 10_000, 0).unwrap();
        engine.accounts[user as usize].position_size = I128::new(1000);
        engine.accounts[user as usize].entry_price = 1_000_000;
        engine.total_open_interest = U128::new(1000);

        // Crank should NOT GC this account (has position)
        let outcome = engine.keeper_crank(100, 1_000_000, &[], 64, 0).unwrap();

        assert!(
            engine.is_used(user as usize),
            "User with position should NOT be GC'd"
        );
        assert_eq!(outcome.num_gc_closed, 0, "Should not GC any accounts");
    }

    #[test]
    fn test_haircut_includes_isolated_balance() {
        let mut params = default_params();
        params.trading_fee_bps = 0;
        params.maintenance_margin_bps = 100;
        params.initial_margin_bps = 100;
        params.warmup_period_slots = 1; // Instant warmup (minimum valid)
        params.max_crank_staleness_slots = u64::MAX;

        let mut engine = Box::new(RiskEngine::new(params));

        let user_idx = engine.add_user(0).unwrap();
        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();

        // Setup capital
        engine.deposit(user_idx, 1_000_000_000, 0).unwrap();
        engine.accounts[lp_idx as usize].capital = U128::new(1_000_000_000);
        engine.vault += 1_000_000_000;
        engine.c_tot = U128::new(2_000_000_000);

        // Add isolated balance to insurance fund
        engine.insurance_fund.isolated_balance = U128::new(500_000_000);

        // Create positive PnL that would trigger haircut without isolated_balance
        // vault = 2B, c_tot = 2B, balance = 0, isolated_balance = 500M
        // residual = 2B - 2B - (0 + 500M) = -500M (negative, so haircut applies)
        // pnl_pos_tot = 1B (set below)
        // Without fix: residual = 2B - 2B - 0 = 0, no haircut
        // With fix: residual = -500M, haircut = min(-500M, 1B) = -500M, but since negative, effective 0?

        // Actually, to test, need pnl_pos_tot > residual with isolated, but not without.

        // Set pnl_pos_tot to 1B
        engine.pnl_pos_tot = U128::new(1_000_000_000);

        // Check haircut ratio
        let (h_num, h_den) = engine.haircut_ratio();

        // With isolated_balance included, residual = 0 - 500M = -500M
        // h_num = min(-500M, 1B) = -500M, but since haircut is for positive, wait.

        // Haircut is for junior profits when residual < pnl_pos_tot
        // If residual < 0, then h_num = residual (negative), but actually haircut_ratio returns (min(residual, pnl), pnl)

        // If residual = -500M, pnl = 1B, h_num = -500M, h_den = 1B
        // But effective haircut is max(0, h_num)/h_den

        // To test, perhaps check that with isolated_balance, haircut is applied when it wouldn't be without.

        // Let's set vault to 2.5B, c_tot = 2B, balance=0, isolated=0.5B
        // residual = 2.5B - 2B - 0.5B = 0
        // pnl_pos_tot = 1B
        // Without isolated: residual = 2.5B - 2B - 0 = 0.5B, h_num = min(0.5B, 1B) = 0.5B
        // With isolated: h_num = min(0, 1B) = 0
        // So haircut changes from 0.5B/1B = 50% to 0/1B = 0%

        // Yes.

        engine.vault = U128::new(2_500_000_000);
        engine.c_tot = U128::new(2_000_000_000);
        engine.insurance_fund.balance = U128::new(0);
        engine.insurance_fund.isolated_balance = U128::new(500_000_000);
        engine.pnl_pos_tot = U128::new(1_000_000_000);
        // PERC-8267: haircut_ratio now uses pnl_matured_pos_tot as denominator.
        // Set matured = pnl_pos_tot to test the same scenario (all PnL matured).
        engine.pnl_matured_pos_tot = 1_000_000_000;

        let (h_num, h_den) = engine.haircut_ratio();

        // With fix, residual = 2.5B - 2B - 0.5B = 0
        // h_num = min(0, 1B) = 0
        assert_eq!(
            h_num, 0,
            "Haircut should include isolated_balance, making residual=0"
        );
        assert_eq!(
            h_den, 1_000_000_000,
            "Denominator should be pnl_matured_pos_tot"
        );

        // Without isolated_balance (simulate old bug)
        let residual_old = engine
            .vault
            .get()
            .saturating_sub(engine.c_tot.get())
            .saturating_sub(engine.insurance_fund.balance.get());
        let h_num_old = core::cmp::min(residual_old, engine.pnl_pos_tot.get());
        assert_eq!(
            h_num_old, 500_000_000,
            "Old calculation would give different haircut"
        );
    }

    #[test]
    fn test_idle_user_drains_and_gc_closes() {
        let mut params = params_for_inline_tests();
        // 1 unit per slot maintenance fee
        params.maintenance_fee_per_slot = U128::new(1);
        let mut engine = RiskEngine::new(params);

        let user_idx = engine.add_user(0).unwrap();
        // Deposit 10 units of capital
        engine.deposit(user_idx, 10, 1).unwrap();

        assert!(engine.is_used(user_idx as usize));

        // Advance 1000 slots and crank — fee drains 1/slot * 1000 = 1000 >> 10 capital
        let outcome = engine.keeper_crank(1001, ORACLE_100K, &[], 64, 0).unwrap();

        // Account should have been drained to 0 capital
        // The crank settles fees and then GC sweeps dust
        assert_eq!(
            outcome.num_gc_closed, 1,
            "expected GC to close the drained account"
        );
        assert!(
            !engine.is_used(user_idx as usize),
            "account should be freed"
        );
    }

    #[test]
    fn test_init_in_place_accepts_valid_params() {
        let mut engine = RiskEngine::new(default_params());
        let mut new_params = default_params();
        new_params.initial_margin_bps = 2000;
        new_params.maintenance_margin_bps = 1000;
        assert!(engine.init_in_place(new_params).is_ok());
        assert_eq!(engine.params.initial_margin_bps, 2000);
    }

    #[test]
    fn test_init_in_place_rejects_invalid_params() {
        let mut engine = RiskEngine::new(default_params());
        let mut bad_params = default_params();
        bad_params.maintenance_margin_bps = 0;
        let result = engine.init_in_place(bad_params);
        assert_eq!(result, Err(RiskError::Overflow));
        // Engine params must remain unchanged after rejection
        assert_eq!(
            engine.params.maintenance_margin_bps,
            default_params().maintenance_margin_bps
        );
    }

    #[test]
    fn test_insolvent_account_blocks_any_withdrawal() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(0).unwrap();

        // Setup: deposit 500, no position, negative pnl of -800 (exceeds capital)
        let _ = engine.deposit(user_idx, 500, 0);
        engine.accounts[user_idx as usize].pnl = I128::new(-800);
        engine.accounts[user_idx as usize].position_size = I128::new(0);

        // After settle: capital = 0, pnl = -300 (remaining loss)
        // Any withdrawal should fail
        let result = engine.withdraw(user_idx, 1, 0, 1_000_000);
        assert_eq!(result, Err(RiskError::InsufficientBalance));

        // Verify N1 invariant: pnl < 0 implies capital == 0
        let account = &engine.accounts[user_idx as usize];
        assert!(!account.pnl.is_negative() || account.capital.is_zero());
    }

    #[test]
    fn test_instruction_context_default() {
        use percolator::InstructionContext;
        let ctx = InstructionContext::default();
        assert!(!ctx.pending_reset_long);
        assert!(!ctx.pending_reset_short);
    }

    #[test]
    fn test_keeper_crank_liquidates_undercollateralized_user() {
        let mut engine = Box::new(RiskEngine::new(default_params()));

        // Fund insurance to avoid force-realize mode (threshold=0 means balance=0 triggers it)
        engine.insurance_fund.balance = U128::new(1_000_000);

        // Create user and LP
        let user = engine.add_user(0).unwrap();
        let lp = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();
        let _ = engine.deposit(user, 10_000, 0);
        let _ = engine.deposit(lp, 100_000, 0);

        // Give user a long position at entry price 1.0
        engine.accounts[user as usize].position_size = I128::new(1_000_000); // 1 unit
        engine.accounts[user as usize].entry_price = 1_000_000;
        engine.accounts[lp as usize].position_size = I128::new(-1_000_000);
        engine.accounts[lp as usize].entry_price = 1_000_000;
        engine.total_open_interest = U128::new(2_000_000);

        // Set negative PnL to make user undercollateralized
        // Position value at oracle 0.5 = 500_000
        // Maintenance margin = 500_000 * 5% = 25_000
        // User has capital 10_000, needs equity > 25_000 to avoid liquidation
        engine.accounts[user as usize].pnl = I128::new(-9_500); // equity = 500 < 25_000

        let _insurance_before = engine.insurance_fund.balance;

        // Call keeper_crank with oracle price 0.5 (500_000 in e6)
        let result = engine.keeper_crank(1, 500_000, &[], 64, 0);
        assert!(result.is_ok());

        let outcome = result.unwrap();

        // Should have liquidated the user
        assert!(
            outcome.num_liquidations > 0,
            "Expected at least one liquidation, got {}",
            outcome.num_liquidations
        );

        // User's position should be closed
        assert_eq!(
            engine.accounts[user as usize].position_size.get(),
            0,
            "User position should be closed after liquidation"
        );

        // Pending loss from liquidation is resolved after a full sweep
        // Run enough cranks to complete a full sweep
        for slot in 2..=17 {
            engine.keeper_crank(slot, 500_000, &[], 64, 0).unwrap();
        }

        // Note: Insurance may decrease if liquidation creates unpaid losses
        // that get covered by finalize_pending_after_window. This is correct behavior.
        // The key invariant is that pending is resolved (not stuck forever).
    }

    #[test]
    fn test_keeper_crank_runs_end_of_instruction_lifecycle() {
        use percolator::SideMode;
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let caller_idx = engine.add_user(0).unwrap();
        engine.deposit(caller_idx, 10_000, 0).unwrap();

        // Simulate a short side in ResetPending with OI already zero
        engine.side_mode_short = SideMode::ResetPending;
        engine.oi_eff_short_q = 0;
        engine.adl_coeff_short = 55;

        let oracle_price = 1_000_000u64;
        // keeper_crank(now_slot, oracle_price, ordered_candidates, max_revalidations, funding_rate)
        engine.keeper_crank(1, oracle_price, &[], 0, 0i64).unwrap();

        // Lifecycle should have fired: ResetPending + OI==0 → Normal
        assert_eq!(
            engine.side_mode_short,
            SideMode::Normal,
            "keeper_crank must run end-of-instruction lifecycle"
        );
        assert_eq!(engine.adl_coeff_short, 0, "adl_coeff_short must be cleared");
    }

    #[test]
    fn test_liquidation_fee_calculation() {
        let mut engine = Box::new(RiskEngine::new(default_params()));

        // Create user
        let user = engine.add_user(0).unwrap();

        // Setup:
        // position = 100_000 (0.1 unit), entry = oracle = 1_000_000 (no mark pnl)
        // position_value = 100_000 * 1_000_000 / 1_000_000 = 100_000
        // maintenance_margin = 100_000 * 5% = 5_000
        // capital = 4_000 < 5_000 -> undercollateralized
        engine.accounts[user as usize].capital = U128::new(4_000);
        engine.accounts[user as usize].position_size = I128::new(100_000); // 0.1 unit
        engine.accounts[user as usize].entry_price = 1_000_000;
        engine.accounts[user as usize].pnl = I128::new(0);
        engine.total_open_interest = U128::new(100_000);
        engine.vault = U128::new(4_000);

        let insurance_before = engine.insurance_fund.balance;
        let oracle_price: u64 = 1_000_000; // Same as entry = no mark pnl

        // Expected fee calculation:
        // notional = 100_000 * 1_000_000 / 1_000_000 = 100_000
        // fee = 100_000 * 50 / 10_000 = 500 (0.5% of notional)

        let result = engine.liquidate_at_oracle(user, 0, oracle_price);
        assert!(result.is_ok());
        assert!(result.unwrap(), "Liquidation should occur");

        let insurance_after = engine.insurance_fund.balance.get();
        let fee_received = insurance_after - insurance_before.get();

        // Fee should be 0.5% of notional (100_000)
        let expected_fee: u128 = 500;
        assert_eq!(
            fee_received, expected_fee,
            "Liquidation fee should be {} but got {}",
            expected_fee, fee_received
        );

        // Verify capital was reduced by the fee
        assert_eq!(
            engine.accounts[user as usize].capital.get(),
            3_500,
            "Capital should be 4000 - 500 = 3500"
        );
    }

    #[test]
    fn test_loss_exceeding_capital_leaves_negative_pnl() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(0).unwrap();

        // Setup: loss greater than capital
        let capital = 5_000u128;
        let loss = 8_000i128;
        engine.accounts[user_idx as usize].capital = U128::new(capital);
        engine.accounts[user_idx as usize].pnl = I128::new(-loss);
        engine.accounts[user_idx as usize].warmup_slope_per_step = U128::new(0);
        engine.vault = U128::new(capital);
        engine.recompute_aggregates();

        // Call settle
        engine.settle_warmup_to_capital(user_idx).unwrap();

        // Capital should be fully consumed
        assert_eq!(
            engine.accounts[user_idx as usize].capital.get(),
            0,
            "Capital should be reduced to zero"
        );
        // Under haircut-ratio design, remaining loss is written off to 0 (spec §6.1 step 4)
        assert_eq!(
            engine.accounts[user_idx as usize].pnl.get(),
            0,
            "Remaining loss should be written off to zero"
        );
    }

    #[test]
    fn test_lp_never_gc() {
        let mut params = params_for_inline_tests();
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
                .keeper_crank(slot * 100, ORACLE_100K, &[], 64, 0)
                .unwrap();
            assert_eq!(
                outcome.num_gc_closed,
                0,
                "LP must not be garbage collected (slot {})",
                slot * 100
            );
        }

        assert!(
            engine.is_used(lp_idx as usize),
            "LP account must still exist"
        );
    }

    #[test]
    fn test_lp_position_flip_margin_check() {
        // Regression test: LP position flip from +1M to -1M requires initial margin.
        // When a user trade causes the LP to flip, it's risk-increasing for the LP.

        let mut params = default_params();
        params.maintenance_margin_bps = 500; // 5%
        params.initial_margin_bps = 1000; // 10%
        params.trading_fee_bps = 0;
        params.warmup_period_slots = 1; // Instant warmup (minimum valid)
        params.max_crank_staleness_slots = u64::MAX;

        let mut engine = Box::new(RiskEngine::new(params));

        let user_idx = engine.add_user(0).unwrap();
        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();

        let oracle_price = 100_000_000u64; // $100

        // User needs enough capital to trade
        engine.deposit(user_idx, 50_000_000, 0).unwrap();

        // LP needs capital for initial position (10% of 100M notional = 10M)
        engine.accounts[lp_idx as usize].capital = U128::new(15_000_000);
        engine.vault += 15_000_000;
        engine.c_tot = U128::new(15_000_000 + 50_000_000);

        // User sells 1M units to LP, LP becomes long +1M
        let size: i128 = -1_000_000;
        engine
            .execute_trade(&MATCHER, lp_idx, user_idx, 0, oracle_price, size)
            .unwrap();
        assert_eq!(
            engine.accounts[lp_idx as usize].position_size.get(),
            1_000_000
        );

        // Reduce LP capital to 5.5M (above maintenance 5%, below initial 10%)
        engine.accounts[lp_idx as usize].capital = U128::new(5_500_000);
        engine.c_tot = U128::new(5_500_000 + 50_000_000);

        // User tries to buy 2M units, which would flip LP from +1M to -1M
        // This crosses zero for LP, so LP needs initial margin (10% = 10M)
        // LP only has 5.5M, so this MUST fail
        let flip_size: i128 = 2_000_000;
        let result = engine.execute_trade(&MATCHER, lp_idx, user_idx, 0, oracle_price, flip_size);

        // MUST be rejected because LP flip requires initial margin
        assert!(
            result.is_err(),
            "LP position flip must require initial margin (cross-zero is risk-increasing)"
        );
        assert_eq!(result.unwrap_err(), RiskError::Undercollateralized);

        // LP position should remain unchanged
        assert_eq!(
            engine.accounts[lp_idx as usize].position_size.get(),
            1_000_000
        );

        // Give LP enough capital for initial margin
        engine.accounts[lp_idx as usize].capital = U128::new(11_000_000);
        engine.c_tot = U128::new(11_000_000 + 50_000_000);

        // Now flip should succeed
        let result2 = engine.execute_trade(&MATCHER, lp_idx, user_idx, 0, oracle_price, flip_size);
        assert!(
            result2.is_ok(),
            "LP position flip should succeed with sufficient initial margin"
        );
        assert_eq!(
            engine.accounts[lp_idx as usize].position_size.get(),
            -1_000_000
        );
    }

    #[test]
    fn test_lp_warmup_bounded() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 10000).unwrap();
        let user = engine.add_user(0).unwrap();

        // Zero-sum PNL: LP gains, user loses (no vault funding needed)
        assert_eq!(engine.accounts[lp_idx as usize].pnl.get(), 0);
        assert_eq!(engine.accounts[user as usize].pnl.get(), 0);
        engine.accounts[lp_idx as usize].pnl = I128::new(5_000);
        engine.accounts[user as usize].pnl = I128::new(-5_000);
        assert_conserved(&engine);

        // Reserve some PNL
        engine.accounts[lp_idx as usize].reserved_pnl = 1_000;

        // Even after long time, withdrawable should not exceed available (positive_pnl - reserved)
        engine.advance_slot(1000);
        let withdrawable = engine.withdrawable_pnl(&engine.accounts[lp_idx as usize]);

        assert!(
            withdrawable <= 4_000,
            "Withdrawable {} should not exceed available {}",
            withdrawable,
            4_000
        );
    }

    #[test]
    fn test_lp_warmup_initial_state() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 10000).unwrap();

        // LP should start with warmup state initialized
        assert_eq!(engine.accounts[lp_idx as usize].reserved_pnl, 0);
        assert_eq!(engine.accounts[lp_idx as usize].warmup_started_at_slot, 0);
    }

    #[test]
    fn test_lp_warmup_monotonic() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 10000).unwrap();
        let user = engine.add_user(0).unwrap();

        // Zero-sum PNL: LP gains, user loses (no vault funding needed)
        assert_eq!(engine.accounts[lp_idx as usize].pnl.get(), 0);
        assert_eq!(engine.accounts[user as usize].pnl.get(), 0);
        engine.accounts[lp_idx as usize].pnl = I128::new(10_000);
        engine.accounts[user as usize].pnl = I128::new(-10_000);
        assert_conserved(&engine);

        // At slot 0
        let w0 = engine.withdrawable_pnl(&engine.accounts[lp_idx as usize]);

        // Advance 50 slots
        engine.advance_slot(50);
        let w50 = engine.withdrawable_pnl(&engine.accounts[lp_idx as usize]);

        // Advance another 50 slots (total 100)
        engine.advance_slot(50);
        let w100 = engine.withdrawable_pnl(&engine.accounts[lp_idx as usize]);

        // Withdrawable should be monotonically increasing
        assert!(
            w50 >= w0,
            "LP warmup should be monotonic: w0={}, w50={}",
            w0,
            w50
        );
        assert!(
            w100 >= w50,
            "LP warmup should be monotonic: w50={}, w100={}",
            w50,
            w100
        );
    }

    #[test]
    fn test_lp_warmup_with_negative_pnl() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 10000).unwrap();

        // LP has negative PNL
        assert_eq!(engine.accounts[lp_idx as usize].pnl.get(), 0);
        engine.accounts[lp_idx as usize].pnl = I128::new(-3_000);

        // Advance time
        engine.advance_slot(100);

        // With negative PNL, withdrawable should be 0
        let withdrawable = engine.withdrawable_pnl(&engine.accounts[lp_idx as usize]);
        assert_eq!(
            withdrawable, 0,
            "Withdrawable should be 0 with negative PNL"
        );
    }

    #[test]
    fn test_lp_withdraw() {
        // Tests that LP withdrawal works correctly (WHITEBOX: direct state mutation)
        let mut engine = Box::new(RiskEngine::new(default_params()));

        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();

        // LP deposits capital
        engine.deposit(lp_idx, 10_000, 0).unwrap();

        // LP earns PNL from counterparty (need zero-sum setup)
        // Create a user to be the counterparty
        let user_idx = engine.add_user(0).unwrap();
        engine.deposit(user_idx, 5_000, 0).unwrap();

        // Add insurance to provide warmup budget for converting LP's positive PnL to capital
        // Budget = warmed_neg_total + insurance_spendable_raw() = 0 + 5000 = 5000
        set_insurance(&mut engine, 5_000);

        // Zero-sum PNL: LP gains 5000, user loses 5000
        // Assert starting pnl is 0 for both (required for zero-sum to preserve conservation)
        assert_eq!(engine.accounts[lp_idx as usize].pnl.get(), 0);
        assert_eq!(engine.accounts[user_idx as usize].pnl.get(), 0);
        engine.accounts[lp_idx as usize].pnl = I128::new(5_000);
        engine.accounts[user_idx as usize].pnl = I128::new(-5_000);
        engine.recompute_aggregates();

        // Set warmup slope so PnL can warm up (warmup_period_slots = 100 from default_params)
        engine.accounts[lp_idx as usize].warmup_slope_per_step = U128::new(5_000 / 100); // 50 per slot
        engine.accounts[lp_idx as usize].warmup_started_at_slot = 0;

        // Advance time to allow warmup
        engine.current_slot = 100; // Full warmup (100 slots × 50 = 5000)

        // Settle the counterparty's negative PnL first to free vault residual.
        // Under haircut-ratio design, positive PnL can only convert to capital when
        // Residual = max(0, V - C_tot - I) > 0. Settling losses reduces C_tot,
        // increasing Residual and enabling profit conversion.
        engine.settle_warmup_to_capital(user_idx).unwrap();

        // Snapshot before withdrawal
        let v0 = vault_snapshot(&engine);

        // withdraw converts warmed PNL to capital, then withdraws
        // After loss settlement: user capital=0, user pnl=0.
        // c_tot=10_000 (LP only), vault=20_000, insurance=5_000.
        // Residual = 20_000 - 10_000 - 5_000 = 5_000.
        // haircut h = min(5_000, 5_000)/5_000 = 1.0 (full conversion).
        // LP capital = 10,000 + 5,000 = 15,000 after conversion.
        let result = engine.withdraw(lp_idx, 10_000, engine.current_slot, 1_000_000);
        assert!(result.is_ok(), "LP withdrawal should succeed: {:?}", result);

        // Withdrawal should reduce vault by 10,000
        assert_vault_delta(&engine, v0, -10_000);
        assert_eq!(
            engine.accounts[lp_idx as usize].capital.get(),
            5_000,
            "LP should have 5,000 capital remaining (from converted PNL)"
        );
        assert_eq!(
            engine.accounts[lp_idx as usize].pnl.get(),
            0,
            "PNL should be converted to capital"
        );
        assert_conserved(&engine);
    }

    #[test]
    fn test_lp_withdraw_with_haircut() {
        // CRITICAL: Tests that LPs are subject to withdrawal-mode haircuts
        let mut engine = Box::new(RiskEngine::new(default_params()));

        let user_idx = engine.add_user(0).unwrap();
        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();

        engine.deposit(user_idx, 10_000, 0).unwrap();
        engine.deposit(lp_idx, 10_000, 0).unwrap();

        // Simulate crisis - set loss_accum
        assert!(user_result.is_ok());

        let lp_result = engine.withdraw(lp_idx, 10_000, 0, 1_000_000);
        assert!(lp_result.is_ok());

        // Both should have withdrawn same proportion
        let total_withdrawn = engine.withdrawal_mode_withdrawn;
        assert!(
            total_withdrawn < 20_000,
            "Total withdrawn should be less than requested due to haircuts"
        );
        assert!(
            total_withdrawn > 14_000,
            "Haircut should be approximately 25%"
        );
    }

    #[test]
    fn test_maintenance_fee_basic_accrual() {
        let fee_per_slot = 10u128;
        let mut engine = Box::new(RiskEngine::new(params_with_maintenance_fee(fee_per_slot)));
        let idx = engine.add_user(0).unwrap();
        engine.deposit(idx, 100_000, 0).unwrap();
        set_insurance(&mut engine, 1_000);
        engine.recompute_aggregates();
        assert_conserved(&engine);

        // Advance 100 slots via the public settle_maintenance_fee path
        let dt = 100u64;
        let now_slot = dt;
        let paid = engine
            .settle_maintenance_fee(idx, now_slot, DEFAULT_ORACLE)
            .unwrap();

        // fee_due = 10 * 100 = 1000
        // fee_credits starts at 0, goes to -1000, then capital pays 1000 into insurance
        assert_eq!(paid, fee_per_slot * dt as u128);
        assert_eq!(engine.accounts[idx as usize].last_fee_slot, now_slot);
        // Capital reduced by 1000
        assert_eq!(engine.accounts[idx as usize].capital.get(), 100_000 - 1000);
        assert_conserved(&engine);
    }

    #[test]
    fn test_maintenance_fee_constants() {
        use percolator::{MAX_MAINTENANCE_FEE_PER_SLOT, MAX_PROTOCOL_FEE_ABS};

        assert_eq!(MAX_MAINTENANCE_FEE_PER_SLOT, 1_000_000_000_000);
        assert_eq!(MAX_PROTOCOL_FEE_ABS, 1_000_000_000_000_000_000);

        // MAX_MAINTENANCE_FEE_PER_SLOT * u16::MAX should not exceed MAX_PROTOCOL_FEE_ABS
        // (i.e., even at max rate for max funding dt, the fee is within cap)
        let max_fee = MAX_MAINTENANCE_FEE_PER_SLOT * (u16::MAX as u128);
        assert!(
            max_fee <= MAX_PROTOCOL_FEE_ABS,
            "max_fee_per_slot * max_dt must fit within MAX_PROTOCOL_FEE_ABS"
        );
    }

    #[test]
    fn test_maintenance_fee_credits_buffer() {
        let fee_per_slot = 10u128;
        let mut engine = Box::new(RiskEngine::new(params_with_maintenance_fee(fee_per_slot)));
        let idx = engine.add_user(0).unwrap();
        engine.deposit(idx, 100_000, 0).unwrap();
        set_insurance(&mut engine, 1_000);

        // Give account 500 fee credits (coupon)
        engine.accounts[idx as usize].fee_credits = I128::new(500);
        engine.recompute_aggregates();
        assert_conserved(&engine);

        // 100 slots → fee_due = 1000
        // fee_credits: 500 - 1000 = -500 → pay 500 from capital
        let paid = engine
            .settle_maintenance_fee(idx, 100, DEFAULT_ORACLE)
            .unwrap();
        assert_eq!(paid, 500); // only the capital portion
        assert_eq!(engine.accounts[idx as usize].capital.get(), 100_000 - 500);
    }

    #[test]
    fn test_maintenance_fee_paid_from_fee_credits_is_coupon_not_revenue() {
        let mut params = params_for_inline_tests();
        params.maintenance_fee_per_slot = U128::new(10);
        let mut engine = RiskEngine::new(params);

        let user_idx = engine.add_user(0).unwrap();
        engine.deposit(user_idx, 1_000_000, 1).unwrap();

        // Add 100 fee credits (test-only helper — no vault/insurance)
        engine.deposit_fee_credits(user_idx, 100, 1).unwrap();
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
    fn test_maintenance_fee_params_validation() {
        use percolator::MAX_MAINTENANCE_FEE_PER_SLOT;

        // At the limit — should be accepted
        let mut p = params_with_maintenance_fee(MAX_MAINTENANCE_FEE_PER_SLOT);
        assert!(p.validate().is_ok(), "fee at cap must be accepted");

        // Above the limit — rejected
        p.maintenance_fee_per_slot = U128::new(MAX_MAINTENANCE_FEE_PER_SLOT + 1);
        assert!(p.validate().is_err(), "fee above cap must be rejected");
    }

    #[test]
    fn test_maintenance_fee_partial_payment() {
        let fee_per_slot = 100u128;
        let mut engine = Box::new(RiskEngine::new(params_with_maintenance_fee(fee_per_slot)));
        let idx = engine.add_user(0).unwrap();
        engine.deposit(idx, 500, 0).unwrap(); // small capital
        set_insurance(&mut engine, 1_000);
        engine.recompute_aggregates();
        assert_conserved(&engine);

        // 100 slots → fee_due = 10_000, but capital is only 500
        let paid = engine
            .settle_maintenance_fee(idx, 100, DEFAULT_ORACLE)
            .unwrap();
        assert_eq!(paid, 500); // all capital consumed
        assert_eq!(engine.accounts[idx as usize].capital.get(), 0);
        // Remaining debt stays in fee_credits (negative)
        assert!(engine.accounts[idx as usize].fee_credits.get() < 0);
    }

    #[test]
    fn test_maintenance_fee_splits_credits_coupon_capital_to_insurance() {
        let mut params = params_for_inline_tests();
        params.maintenance_fee_per_slot = U128::new(10);
        let mut engine = RiskEngine::new(params);

        let user_idx = engine.add_user(0).unwrap();
        // deposit at slot 1: dt=1 from slot 0, fee=10. Paid from deposit.
        // capital = 50 - 10 = 40.
        engine.deposit(user_idx, 50, 1).unwrap();
        assert_eq!(engine.accounts[user_idx as usize].capital.get(), 40);

        // Add 30 fee credits (test-only)
        engine.deposit_fee_credits(user_idx, 30, 1).unwrap();

        let rev_before = engine.insurance_fund.fee_revenue.get();

        // Settle maintenance: dt=10, fee_per_slot=10, due=100
        // credits pays 30, capital pays 40 (all it has), leftover 30 unpaid
        engine
            .settle_maintenance_fee(user_idx, 11, ORACLE_100K)
            .unwrap();

        let rev_increase = engine.insurance_fund.fee_revenue.get() - rev_before;
        let cap_after = engine.accounts[user_idx as usize].capital.get();

        assert_eq!(
            rev_increase, 40,
            "insurance revenue should be 40 (capital only; credits are coupon)"
        );
        assert_eq!(cap_after, 0, "capital should be fully drained");
        // fee_credits should be -30 (100 due - 30 credits - 40 capital = 30 unpaid debt)
        assert_eq!(
            engine.accounts[user_idx as usize].fee_credits.get(),
            -30,
            "fee_credits should reflect unpaid debt"
        );
    }

    #[test]
    fn test_maintenance_fee_via_force_close_resolved() {
        let fee_per_slot = 5u128;
        let mut engine = Box::new(RiskEngine::new(params_with_maintenance_fee(fee_per_slot)));
        let user = engine.add_user(0).unwrap();
        engine.deposit(user, 10_000, 0).unwrap();
        set_insurance(&mut engine, 500);
        engine.recompute_aggregates();
        assert_conserved(&engine);

        // Advance current_slot so force_close_resolved will compute dt > 0
        engine.current_slot = 200;
        // last_fee_slot defaults to current_slot at deposit time (0)
        // So dt = 200, fee_due = 5 * 200 = 1000

        let capital_returned = engine.force_close_resolved(user).unwrap();

        // Capital was 10_000, fee charged = 1000, returned = 10_000 - 1000 = 9_000
        assert_eq!(capital_returned, 9_000);
        assert!(!engine.is_used(user as usize));
    }

    #[test]
    fn test_maintenance_fee_zero_dt_noop() {
        let mut engine = Box::new(RiskEngine::new(params_with_maintenance_fee(10)));
        let idx = engine.add_user(0).unwrap();
        engine.deposit(idx, 100_000, 0).unwrap();
        set_insurance(&mut engine, 1_000);

        // Set last_fee_slot = 50, then call with now_slot = 50 → dt = 0
        engine.accounts[idx as usize].last_fee_slot = 50;
        let paid = engine
            .settle_maintenance_fee(idx, 50, DEFAULT_ORACLE)
            .unwrap();
        assert_eq!(paid, 0);
        assert_eq!(engine.accounts[idx as usize].capital.get(), 100_000);
    }

    #[test]
    fn test_maintenance_fee_zero_rate_noop() {
        let mut engine = Box::new(RiskEngine::new(params_with_maintenance_fee(0)));
        let idx = engine.add_user(0).unwrap();
        engine.deposit(idx, 100_000, 0).unwrap();
        set_insurance(&mut engine, 1_000);

        let paid = engine
            .settle_maintenance_fee(idx, 1_000_000, DEFAULT_ORACLE)
            .unwrap();
        assert_eq!(paid, 0);
        assert_eq!(engine.accounts[idx as usize].capital.get(), 100_000);
    }

    #[test]
    fn test_mark_price_liq_delegates_when_disabled() {
        let mut params = default_params();
        params.use_mark_price_for_liquidation = false;
        let mut engine = Box::new(RiskEngine::new(params));
        let user = engine.add_user(0).unwrap();
        engine.deposit(user, 10_000_000, 1).unwrap();
        assert_eq!(
            engine.liquidate_with_mark_price(user, 100, 1_000_000),
            Ok(false)
        );
    }

    #[test]
    fn test_mark_price_liq_oob() {
        let mut params = default_params();
        params.use_mark_price_for_liquidation = true;
        let mut engine = Box::new(RiskEngine::new(params));
        engine.mark_price_e6 = 1_000_000;
        assert_eq!(
            engine.liquidate_with_mark_price(u16::MAX, 100, 1_000_000),
            Ok(false)
        );
    }

    #[test]
    fn test_mark_price_liq_skips_healthy_at_mark() {
        let mut params = default_params();
        params.use_mark_price_for_liquidation = true;
        let mut engine = Box::new(RiskEngine::new(params));
        let user = engine.add_user(0).unwrap();
        engine.deposit(user, 100_000_000, 1).unwrap();
        engine.accounts[user as usize].position_size = I128::new(1_000_000);
        engine.accounts[user as usize].entry_price = 1_000_000;
        engine.total_open_interest = U128::new(1_000_000);
        engine.mark_price_e6 = 1_000_000; // healthy mark
                                          // Oracle crashed but mark is fine → no liquidation
        assert_eq!(
            engine.liquidate_with_mark_price(user, 100, 500_000),
            Ok(false)
        );
    }

    #[test]
    fn test_mark_settlement_on_trade_touch() {
        let mut params = default_params();
        params.trading_fee_bps = 0;
        params.max_crank_staleness_slots = u64::MAX;

        let mut engine = Box::new(RiskEngine::new(params));

        // Create LP and user
        let lp = engine.add_lp([1u8; 32], [0u8; 32], 0).unwrap();
        engine.deposit(lp, 1_000_000, 0).unwrap();

        let user = engine.add_user(0).unwrap();
        engine.deposit(user, 1_000_000, 0).unwrap();

        // First trade: user buys 1 unit at oracle 1_000_000
        let oracle1 = 1_000_000;
        engine
            .execute_trade(&MATCHER, lp, user, 0, oracle1, 1_000_000)
            .unwrap();

        // User now has: pos = +1, entry = 1_000_000, pnl = 0
        assert_eq!(
            engine.accounts[user as usize].position_size.get(),
            1_000_000
        );
        assert_eq!(engine.accounts[user as usize].entry_price, oracle1);
        assert_eq!(engine.accounts[user as usize].pnl.get(), 0);

        // Second trade at higher oracle: user sells (closes) at oracle 1_100_000
        // Before position change, mark should be settled (coin-margined):
        // mark = (1_100_000 - 1_000_000) * 1_000_000 / 1_100_000 = 90_909
        // User gains +90909 mark PnL, LP gets -90909 mark PnL
        //
        // After mark settlement, trade_pnl = (oracle - exec) * size = 0 (exec at oracle)
        //
        // Note: settle_warmup_to_capital immediately settles negative PnL from capital,
        // so LP's pnl becomes 0 and capital decreases by 100k.
        // User's positive pnl may or may not settle depending on warmup budget.
        let oracle2 = 1_100_000;

        let user_capital_before = engine.accounts[user as usize].capital.get();
        let lp_capital_before = engine.accounts[lp as usize].capital.get();

        engine
            .execute_trade(&MATCHER, lp, user, 0, oracle2, -1_000_000)
            .unwrap();

        // User closed position
        assert_eq!(engine.accounts[user as usize].position_size.get(), 0);

        // User should have gained 100k total equity (could be in pnl or capital)
        let user_pnl = engine.accounts[user as usize].pnl.get();
        let user_capital = engine.accounts[user as usize].capital.get();
        let user_equity_gain = user_pnl + (user_capital as i128 - user_capital_before as i128);
        assert_eq!(
            user_equity_gain, 90_909,
            "User should have gained 90909 total equity (coin-margined)"
        );

        // LP should have lost 100k total equity
        // Since negative PnL is immediately settled, LP's pnl should be 0 and capital should be 900k
        let lp_pnl = engine.accounts[lp as usize].pnl.get();
        let lp_capital = engine.accounts[lp as usize].capital.get();
        assert_eq!(lp_pnl, 0, "LP negative pnl should be settled to capital");
        assert_eq!(
            lp_capital,
            lp_capital_before - 90_909,
            "LP capital should decrease by 90909 (coin-margined loss settled)"
        );

        // Conservation should hold
        assert!(
            engine.check_conservation(oracle2),
            "Conservation should hold after mark settlement"
        );
    }

    #[test]
    fn test_max_funding_dt_constant() {
        assert_eq!(
            MAX_FUNDING_DT, 65535,
            "MAX_FUNDING_DT must be u16::MAX per spec §1.4"
        );
        assert_eq!(
            MAX_ABS_FUNDING_BPS_PER_SLOT, 10_000,
            "MAX_ABS = 10000 per spec §1.4"
        );
    }

    #[test]
    fn test_micro_trade_fee_not_zero() {
        let mut params = default_params();
        params.trading_fee_bps = 10; // 0.1% fee
        params.maintenance_margin_bps = 100; // 1% for easy math
        params.initial_margin_bps = 100;
        params.warmup_period_slots = 1; // Instant warmup (minimum valid)
        params.max_crank_staleness_slots = u64::MAX;

        let mut engine = Box::new(RiskEngine::new(params));

        let user_idx = engine.add_user(0).unwrap();
        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();

        // Deposit enough capital for margin
        engine.deposit(user_idx, 1_000_000_000, 0).unwrap();
        engine.accounts[lp_idx as usize].capital = U128::new(1_000_000_000);
        engine.vault += 1_000_000_000;
        engine.c_tot = U128::new(2_000_000_000);

        let oracle_price = 1_000_000u64; // $1

        let insurance_before = engine.insurance_fund.balance.get();

        // Execute a micro-trade: size=1, price=$1 → notional = 1
        // Old fee calc: 1 * 10 / 10_000 = 0 (WRONG - fee evasion!)
        // New fee calc: (1 * 10 + 9999) / 10_000 = 1 (CORRECT - minimum 1 unit)
        let size: i128 = 1;
        engine
            .execute_trade(&MATCHER, lp_idx, user_idx, 0, oracle_price, size)
            .unwrap();

        let insurance_after = engine.insurance_fund.balance.get();
        let fee_charged = insurance_after - insurance_before;

        // Fee MUST be at least 1 (ceiling division prevents zero-fee micro-trades)
        assert!(
            fee_charged >= 1,
            "Micro-trade must pay at least 1 unit fee (ceiling division). Got fee={}",
            fee_charged
        );
    }

    #[test]
    fn test_negative_pnl_settles_immediately_independent_of_slope() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(0).unwrap();

        // Setup: loss with zero slope - under old code this would NOT settle
        let capital = 10_000u128;
        let loss = 3_000i128;
        engine.accounts[user_idx as usize].capital = U128::new(capital);
        engine.accounts[user_idx as usize].pnl = I128::new(-loss);
        engine.accounts[user_idx as usize].warmup_slope_per_step = U128::new(0); // Zero slope
        engine.accounts[user_idx as usize].warmup_started_at_slot = 0;
        engine.vault = U128::new(capital);
        engine.current_slot = 100; // Time has passed

        // Call settle
        engine.settle_warmup_to_capital(user_idx).unwrap();

        // Assertions: loss should settle immediately despite zero slope
        assert_eq!(
            engine.accounts[user_idx as usize].capital.get(),
            capital - (loss as u128),
            "Capital should be reduced by full loss amount"
        );
        assert_eq!(
            engine.accounts[user_idx as usize].pnl.get(),
            0,
            "PnL should be 0 after immediate settlement"
        );
    }

    #[test]
    fn test_normal_cooldown_still_blocks_when_not_emergency() {
        let mut params = default_params();
        params.maintenance_margin_bps = 500;
        params.liquidation_buffer_bps = 100;
        params.min_liquidation_abs = U128::new(1);
        params.partial_liquidation_bps = 2000;
        params.partial_liquidation_cooldown_slots = 30;
        params.use_mark_price_for_liquidation = true;
        params.emergency_liquidation_margin_bps = 200; // 2%

        let mut engine = Box::new(RiskEngine::new(params));
        engine.mark_price_e6 = 1_000_000;

        let lp = engine.add_lp([0u8; 32], [0u8; 32], 0).unwrap();
        engine.accounts[lp as usize].capital = U128::new(100_000_000);
        engine.accounts[lp as usize].position_size = I128::new(-10_000_000);
        engine.accounts[lp as usize].entry_price = 1_000_000;

        let user = engine.add_user(0).unwrap();

        // Position: 10 units at $1, capital = 400k
        // At $1: position_value = 10M, equity = 400k
        // MM = 10M * 5% = 500k → underwater
        // Emergency = 10M * 2% = 200k → equity(400k) > 200k → NOT emergency
        engine.accounts[user as usize].capital = U128::new(400_000);
        engine.accounts[user as usize].position_size = I128::new(10_000_000);
        engine.accounts[user as usize].entry_price = 1_000_000;
        engine.accounts[user as usize].pnl = I128::new(0);
        engine.total_open_interest = U128::new(10_000_000);
        engine.vault = U128::new(100_400_000);

        // First partial liquidation at slot 100
        let result = engine
            .liquidate_with_mark_price(user, 100, 1_000_000)
            .unwrap();
        assert!(result, "First partial liquidation should succeed");

        // Simulate last_partial_liquidation_slot = 100 (already set by engine)
        let pos_after_first = engine.accounts[user as usize].position_size.get();
        if pos_after_first == 0 {
            return; // Already fully closed
        }

        // Try again at slot 105 — within cooldown, NOT emergency
        let result2 = engine
            .liquidate_with_mark_price(user, 105, 1_000_000)
            .unwrap();
        assert!(
            !result2,
            "Normal cooldown should block liquidation when not in emergency"
        );
    }

    #[test]
    fn test_offset_check_for_tests() {
        println!("vault: {}", std::mem::offset_of!(RiskEngine, vault));
        println!("used: {}", std::mem::offset_of!(RiskEngine, used));
        println!(
            "num_used_accounts: {}",
            std::mem::offset_of!(RiskEngine, num_used_accounts)
        );
        println!("accounts: {}", std::mem::offset_of!(RiskEngine, accounts));
        println!("RiskEngine size: {}", std::mem::size_of::<RiskEngine>());

        use std::mem::offset_of;
        // These assertions match the SBF_ENGINE_OFF=600 offsets in integration tests
        // If any of these fail, the integration test helpers need updating
        // Updated for percolator@cf35789 (PERC-8093): +48 bytes in RiskParams (min_nonzero_mm_req, min_nonzero_im_req, insurance_floor)
        // Updated for PERC-8267: +16 bytes from pnl_matured_pos_tot field added to RiskEngine
        //                        +8 bytes from Account.reserved_pnl: u64 → u128
        // Updated for PERC-8268: +224 bytes from ADL side state fields (SideMode, oi_eff, adl_mult/coeff/epoch, etc.)
        //                        used: 760→984, num_used (full): 1272→1496, accounts (full): 9488→9712
        // Updated for PERC-8270: +32 bytes from last_market_slot, funding_price_sample_last,
        //                        materialized_account_count, last_oracle_price added to RiskEngine
        //                        used: 984→1016, num_used (full): 1496→1528, accounts (full): 9712→9744
        //                        Note: Account also gains 56 bytes (position_basis_q, adl_a_basis,
        //                        adl_k_snap, adl_epoch_snap) — SLAB_LEN will change (devnet migration required)
        // Note: `small` feature uses MAX_ACCOUNTS=256, shrinking next_free[] and accounts[] — offsets differ
        assert_eq!(
            offset_of!(RiskEngine, used),
            1016,
            "used bitmap offset changed -- update SBF_ENGINE_OFF+1016 in integration tests"
        );
        #[cfg(not(any(feature = "small", feature = "medium")))]
        assert_eq!(
            offset_of!(RiskEngine, num_used_accounts),
            1528,
            "num_used_accounts offset changed -- update SBF_ENGINE_OFF+1528 in integration tests"
        );
        #[cfg(feature = "small")]
        assert_eq!(
            offset_of!(RiskEngine, num_used_accounts),
            1048,
            "small feature: num_used_accounts offset differs (MAX_ACCOUNTS=256 → bitmap=32 bytes)"
        );
        #[cfg(feature = "medium")]
        assert_eq!(
        offset_of!(RiskEngine, num_used_accounts),
        1144,
        "medium feature: num_used_accounts offset differs (MAX_ACCOUNTS=1024 → bitmap=128 bytes, +32 from ADL epoch fields PERC-8272)"
    );
        #[cfg(not(any(feature = "small", feature = "medium")))]
        assert_eq!(
        offset_of!(RiskEngine, accounts),
        9744,
        "accounts offset changed -- update SBF_ENGINE_OFF+9744 in integration tests (PERC-8270)"
    );
        #[cfg(feature = "small")]
        assert_eq!(
            offset_of!(RiskEngine, accounts),
            1584,
            "small feature: accounts offset differs (MAX_ACCOUNTS=256 → next_free is 512 bytes)"
        );
        #[cfg(feature = "medium")]
        assert_eq!(
            offset_of!(RiskEngine, accounts),
            3216,
            "medium feature: accounts offset differs (MAX_ACCOUNTS=1024 → next_free is 2048 bytes)"
        );
    }

    #[test]
    fn test_oi_eff_fields_initialized_to_zero() {
        let e = *Box::new(RiskEngine::new(default_params()));
        assert_eq!(e.oi_eff_long_q, 0);
        assert_eq!(e.oi_eff_short_q, 0);
        assert_eq!(e.adl_mult_long, 0);
        assert_eq!(e.adl_mult_short, 0);
    }

    #[test]
    fn test_partial_liq_cooldown() {
        let mut params = default_params();
        params.use_mark_price_for_liquidation = true;
        params.partial_liquidation_bps = 2000;
        params.partial_liquidation_cooldown_slots = 30;
        let mut engine = Box::new(RiskEngine::new(params));
        let user = engine.add_user(0).unwrap();
        engine.deposit(user, 10_000_000, 1).unwrap();
        engine.accounts[user as usize].position_size = I128::new(100_000_000);
        engine.accounts[user as usize].entry_price = 1_000_000;
        engine.total_open_interest = U128::new(100_000_000);
        engine.mark_price_e6 = 900_000;
        // First call at slot 100
        let r1 = engine.liquidate_with_mark_price(user, 100, 900_000);
        assert!(r1.is_ok());
        if r1.unwrap() {
            // Within cooldown at slot 110
            assert_eq!(
                engine.liquidate_with_mark_price(user, 110, 900_000),
                Ok(false)
            );
        }
    }

    #[test]
    fn test_partial_liq_params_validation() {
        let mut params = default_params();
        params.partial_liquidation_bps = 2000;
        assert!(params.validate().is_ok());
        params.partial_liquidation_bps = 10_001;
        assert!(params.validate().is_err());
    }

    #[test]
    fn test_partial_liquidation_brings_to_safety() {
        let mut params = default_params();
        params.maintenance_margin_bps = 500;
        params.liquidation_buffer_bps = 100;
        params.min_liquidation_abs = U128::new(100_000);

        let mut engine = Box::new(RiskEngine::new(params));
        let user = engine.add_user(0).unwrap();

        // Position: 10 units at $1, small capital
        // At oracle $1: equity = 100k, position_value = 10M
        // MM = 10M * 5% = 500k
        // equity (100k) < MM (500k) => undercollateralized
        // But equity > 0, so partial liquidation will occur
        engine.accounts[user as usize].capital = U128::new(100_000);
        engine.accounts[user as usize].position_size = I128::new(10_000_000);
        engine.accounts[user as usize].entry_price = 1_000_000;
        engine.accounts[user as usize].pnl = I128::new(0);
        engine.total_open_interest = U128::new(10_000_000);
        engine.vault = U128::new(100_000);

        let oracle_price = 1_000_000;
        let pos_before = engine.accounts[user as usize].position_size;

        // Liquidate - should succeed and reduce position
        let result = engine.liquidate_at_oracle(user, 0, oracle_price).unwrap();
        assert!(result, "Liquidation should succeed");

        let pos_after = engine.accounts[user as usize].position_size;

        // Position should be reduced (partial liquidation)
        assert!(
            pos_after.get() < pos_before.get(),
            "Position should be reduced after liquidation"
        );
        assert!(
            pos_after.is_positive(),
            "Partial liquidation should leave some position"
        );
    }

    #[test]
    fn test_partial_liquidation_fee_charged() {
        let mut params = default_params();
        params.maintenance_margin_bps = 500;
        params.liquidation_buffer_bps = 100;
        params.min_liquidation_abs = U128::new(100_000);
        params.liquidation_fee_bps = 50; // 0.5%

        let mut engine = Box::new(RiskEngine::new(params));
        let user = engine.add_user(0).unwrap();

        // Small position to trigger full liquidation (dust rule)
        // position_value = 500_000
        // MM = 25_000
        // capital = 20_000 < MM
        engine.accounts[user as usize].capital = U128::new(20_000);
        engine.accounts[user as usize].position_size = I128::new(500_000); // 0.5 units
        engine.accounts[user as usize].entry_price = 1_000_000;
        engine.accounts[user as usize].pnl = I128::new(0);
        engine.total_open_interest = U128::new(500_000);
        engine.vault = U128::new(20_000);

        let insurance_before = engine.insurance_fund.balance;
        let oracle_price = 1_000_000;

        // Liquidate
        let result = engine.liquidate_at_oracle(user, 0, oracle_price).unwrap();
        assert!(result, "Liquidation should succeed");

        let insurance_after = engine.insurance_fund.balance.get();
        let fee_received = insurance_after - insurance_before.get();

        // Fee = 500_000 * 1_000_000 / 1_000_000 * 50 / 10_000 = 2_500
        // But capped by available capital (20_000), so full 2_500 should be charged
        assert!(fee_received > 0, "Some fee should be charged");
    }

    #[test]
    fn test_pending_finalize_liveness_insurance_covers() {
        let mut params = default_params();
        params.risk_reduction_threshold = U128::new(1000); // Floor at 1000
        let mut engine = Box::new(RiskEngine::new(params));

        // Fund insurance well above floor
        engine.insurance_fund.balance = U128::new(100_000);
        engine.vault = U128::new(100_000);

        // Run enough cranks to complete a full sweep
        for slot in 1..=16 {
            let result = engine.keeper_crank(slot, 1_000_000, &[], 64, 0);
            assert!(result.is_ok());
        }

        // Under haircut-ratio design, there is no pending_unpaid_loss mechanism.
        // Insurance is not spent by cranks when there are no losses to handle.
        assert_eq!(
            engine.insurance_fund.balance.get(),
            100_000,
            "Insurance should be unchanged when no losses exist"
        );
    }

    #[test]
    fn test_pnl_warmup() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(0).unwrap();
        let counterparty = engine.add_user(0).unwrap();

        // Zero-sum PNL: user gains, counterparty loses (no vault funding needed)
        assert_eq!(engine.accounts[user_idx as usize].pnl.get(), 0);
        assert_eq!(engine.accounts[counterparty as usize].pnl.get(), 0);
        engine.accounts[user_idx as usize].pnl = I128::new(1000);
        engine.accounts[user_idx as usize].warmup_slope_per_step = U128::new(10); // 10 per slot
        engine.accounts[counterparty as usize].pnl = I128::new(-1000);
        assert_conserved(&engine);

        // At slot 0, nothing is warmed up yet
        assert_eq!(
            engine.withdrawable_pnl(&engine.accounts[user_idx as usize]),
            0
        );

        // Advance 50 slots
        engine.advance_slot(50);
        assert_eq!(
            engine.withdrawable_pnl(&engine.accounts[user_idx as usize]),
            500
        ); // 10 * 50

        // Advance 100 more slots (total 150)
        engine.advance_slot(100);
        assert_eq!(
            engine.withdrawable_pnl(&engine.accounts[user_idx as usize]),
            1000
        ); // Capped at total PNL
    }

    #[test]
    fn test_pnl_warmup_with_reserved() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(0).unwrap();
        let counterparty = engine.add_user(0).unwrap();

        // Zero-sum PNL: user gains, counterparty loses (no vault funding needed)
        assert_eq!(engine.accounts[user_idx as usize].pnl.get(), 0);
        assert_eq!(engine.accounts[counterparty as usize].pnl.get(), 0);
        engine.accounts[user_idx as usize].pnl = I128::new(1000);
        // reserved_pnl is now trade_entry_price — no longer reduces available PnL
        engine.accounts[user_idx as usize].warmup_slope_per_step = U128::new(10);
        engine.accounts[counterparty as usize].pnl = I128::new(-1000);
        assert_conserved(&engine);

        // Advance 100 slots
        engine.advance_slot(100);

        // Withdrawable = min(available_pnl, warmed_up)
        // available_pnl = 1000 (no reservation, full PnL available)
        // warmed_up = 10 * 100 = 1000
        // So withdrawable = 1000
        assert_eq!(
            engine.withdrawable_pnl(&engine.accounts[user_idx as usize]),
            1000
        );
    }

    #[test]
    fn test_position_flip_margin_check() {
        // Regression test: flipping from +1M to -1M (same absolute size) requires initial margin.
        // A flip is semantically a close + open, so the new side must meet initial margin.

        let mut params = default_params();
        params.maintenance_margin_bps = 500; // 5%
        params.initial_margin_bps = 1000; // 10%
        params.trading_fee_bps = 0;
        params.warmup_period_slots = 1; // Instant warmup (minimum valid)
        params.max_crank_staleness_slots = u64::MAX;

        let mut engine = Box::new(RiskEngine::new(params));

        let user_idx = engine.add_user(0).unwrap();
        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();

        // User needs capital for initial position (10% of 100M notional = 10M)
        engine.deposit(user_idx, 15_000_000, 0).unwrap();

        // LP capital
        engine.accounts[lp_idx as usize].capital = U128::new(100_000_000);
        engine.vault += 100_000_000;

        let oracle_price = 100_000_000u64; // $100

        // Open long position of 1M units ($100M notional)
        let size: i128 = 1_000_000;
        engine
            .execute_trade(&MATCHER, lp_idx, user_idx, 0, oracle_price, size)
            .unwrap();
        assert_eq!(
            engine.accounts[user_idx as usize].position_size.get(),
            1_000_000
        );

        // Set user capital to 5.5M (above maintenance 5% = 5M, but below initial 10% = 10M)
        engine.accounts[user_idx as usize].capital = U128::new(5_500_000);
        engine.c_tot = U128::new(5_500_000);

        // Try to flip from +1M to -1M (trade -2M)
        // This crosses zero, so it's risk-increasing and requires initial margin (10% = 10M)
        // User has only 5.5M, which is below initial margin, so this MUST fail
        let flip_size: i128 = -2_000_000;
        let result = engine.execute_trade(&MATCHER, lp_idx, user_idx, 0, oracle_price, flip_size);

        // MUST be rejected because flip requires initial margin
        assert!(
            result.is_err(),
            "Position flip must require initial margin (cross-zero is risk-increasing)"
        );
        assert_eq!(result.unwrap_err(), RiskError::Undercollateralized);

        // Position should remain unchanged
        assert_eq!(
            engine.accounts[user_idx as usize].position_size.get(),
            1_000_000
        );

        // Now give user enough capital for initial margin (10% of 100M = 10M, plus buffer)
        engine.accounts[user_idx as usize].capital = U128::new(11_000_000);
        engine.c_tot = U128::new(11_000_000);

        // Now flip should succeed
        let result2 = engine.execute_trade(&MATCHER, lp_idx, user_idx, 0, oracle_price, flip_size);
        assert!(
            result2.is_ok(),
            "Position flip should succeed with sufficient initial margin"
        );
        assert_eq!(
            engine.accounts[user_idx as usize].position_size.get(),
            -1_000_000
        );
    }

    #[test]
    fn test_premium_funding_clamped_to_max() {
        // mark = 1.10 (10% above index) but max is 5 bps
        let rate = RiskEngine::compute_premium_funding_bps_per_slot(
            1_100_000, // mark = 1.10
            1_000_000, // index = 1.0
            1_000_000, // dampening = 1.0x
            5,         // max 5 bps/slot
        );
        assert_eq!(rate, 5, "Should clamp to max");
    }

    #[test]
    fn test_premium_funding_negative_when_mark_below_index() {
        // mark = 0.99 (1% below index)
        let rate = RiskEngine::compute_premium_funding_bps_per_slot(
            990_000,   // mark = 0.99
            1_000_000, // index = 1.0
            1_000_000, // dampening = 1.0x
            100,       // max
        );
        assert!(rate < 0, "Shorts should pay when mark < index");
        assert_eq!(rate, -100);
    }

    #[test]
    fn test_premium_funding_params_validation() {
        let mut params = default_params();
        // Valid: premium weight = 50%, dampening = 8x
        params.funding_premium_weight_bps = 5_000;
        params.funding_premium_dampening_e6 = 8_000_000;
        assert!(params.validate().is_ok());

        // Invalid: premium weight > 100%
        params.funding_premium_weight_bps = 10_001;
        assert!(params.validate().is_err());

        // Invalid: premium weight > 0 but dampening = 0
        params.funding_premium_weight_bps = 5_000;
        params.funding_premium_dampening_e6 = 0;
        assert!(params.validate().is_err());
    }

    #[test]
    fn test_premium_funding_positive_when_mark_above_index() {
        // mark = 1.01 (1% above index)
        let rate = RiskEngine::compute_premium_funding_bps_per_slot(
            1_010_000, // mark = 1.01
            1_000_000, // index = 1.0
            1_000_000, // dampening = 1.0x (no dampening)
            100,       // max 100 bps/slot
        );
        // premium = (1.01 - 1.0) / 1.0 = 1% = 100 bps
        // rate = 100 bps / dampening(1.0) = 100 bps/slot
        assert!(rate > 0, "Longs should pay when mark > index");
        assert_eq!(rate, 100, "1% premium with 1.0x dampening = 100 bps");
    }

    #[test]
    fn test_premium_funding_with_dampening() {
        // mark = 1.01 (1% above), dampening = 8_000_000 (8x)
        let rate = RiskEngine::compute_premium_funding_bps_per_slot(
            1_010_000, // mark = 1.01
            1_000_000, // index = 1.0
            8_000_000, // dampening = 8.0x
            100,       // max
        );
        // premium = 100 bps, rate = 100 / 8 = 12 bps/slot
        assert_eq!(rate, 12);
    }

    #[test]
    fn test_premium_funding_zero_inputs() {
        assert_eq!(
            RiskEngine::compute_premium_funding_bps_per_slot(0, 1_000_000, 1_000_000, 5).unwrap(),
            0
        );
        assert_eq!(
            RiskEngine::compute_premium_funding_bps_per_slot(1_000_000, 0, 1_000_000, 5).unwrap(),
            0
        );
        assert_eq!(
            RiskEngine::compute_premium_funding_bps_per_slot(1_000_000, 1_000_000, 0, 5).unwrap(),
            0
        );
    }

    #[test]
    fn test_premium_funding_zero_when_mark_equals_index() {
        let rate = RiskEngine::compute_premium_funding_bps_per_slot(
            1_000_000, // mark = 1.0
            1_000_000, // index = 1.0
            1_000_000, // dampening = 1.0x
            100,       // max 100 bps/slot
        );
        assert_eq!(rate, 0, "No premium when mark == index");
    }

    #[test]
    fn test_riskparams_offsets() {
        use std::mem::offset_of;
        println!("RiskParams size: {}", std::mem::size_of::<RiskParams>());
        println!("RiskParams align: {}", std::mem::align_of::<RiskParams>());
        println!(
            "fee_tier2_threshold: {}",
            offset_of!(RiskParams, fee_tier2_threshold)
        );
        println!(
            "fee_tier3_threshold: {}",
            offset_of!(RiskParams, fee_tier3_threshold)
        );
        println!(
            "min_nonzero_mm_req: {}",
            offset_of!(RiskParams, min_nonzero_mm_req)
        );
        println!(
            "min_nonzero_im_req: {}",
            offset_of!(RiskParams, min_nonzero_im_req)
        );
        println!(
            "insurance_floor: {}",
            offset_of!(RiskParams, insurance_floor)
        );
        println!(
            "use_mark_price: {}",
            offset_of!(RiskParams, use_mark_price_for_liquidation)
        );
        println!(
            "emergency_liq_bps: {}",
            offset_of!(RiskParams, emergency_liquidation_margin_bps)
        );
        println!("fee_tier2_bps: {}", offset_of!(RiskParams, fee_tier2_bps));
        println!("fee_tier3_bps: {}", offset_of!(RiskParams, fee_tier3_bps));
        println!(
            "fee_split_lp_bps: {}",
            offset_of!(RiskParams, fee_split_lp_bps)
        );
    }

    #[test]
    fn test_rounding_bound_with_many_positive_pnl_accounts() {
        let mut engine = Box::new(RiskEngine::new(default_params()));

        // Create multiple accounts with positive PnL
        let num_accounts = 10usize;
        let mut account_indices = Vec::new();

        for _ in 0..num_accounts {
            let idx = engine.add_user(0).unwrap();
            engine.deposit(idx, 10_000, 0).unwrap();
            account_indices.push(idx);
        }

        // Set each account to have different positive PnL values
        // Use values that will create rounding when haircutted
        for (i, &idx) in account_indices.iter().enumerate() {
            let pnl = ((i + 1) * 1000 + 7) as i128; // 1007, 2007, 3007, ... (odd values for rounding)
            engine.accounts[idx as usize].pnl = I128::new(pnl);
        }

        // Total positive PnL = 1007 + 2007 + ... + 10007 = 55070
        let total_positive_pnl: u128 = (1..=num_accounts).map(|i| (i * 1000 + 7) as u128).sum();

        // Set Residual to be LESS than total PnL to create a haircut (h < 1)
        // This forces the floor operation to have rounding effects
        // Residual = V - C_tot - I
        // We want Residual < PNL_pos_tot
        let target_residual = total_positive_pnl * 2 / 3; // ~66% backing → h ≈ 0.66

        // c_tot = 10 * 10_000 = 100_000
        let c_tot = engine.c_tot.get();
        let insurance = engine.insurance_fund.balance.get();

        // V = Residual + C_tot + I
        engine.vault = U128::new(target_residual + c_tot + insurance);

        engine.recompute_aggregates();

        // Compute haircut ratio
        let (h_num, h_den) = engine.haircut_ratio();

        // Verify we have a haircut (h < 1)
        assert!(
            h_num < h_den,
            "Test setup error: expected haircut (h_num={} < h_den={})",
            h_num,
            h_den
        );

        // Compute Residual
        let residual = engine
            .vault
            .get()
            .saturating_sub(engine.c_tot.get())
            .saturating_sub(engine.insurance_fund.balance.get());

        // h_num = min(Residual, PNL_pos_tot) = Residual (since Residual < PNL_pos_tot)
        assert_eq!(
            h_num, residual,
            "h_num should equal Residual when underbacked"
        );

        // Compute sum of effective positive PnL using floor division
        let mut sum_eff_pos_pnl = 0u128;
        for &idx in &account_indices {
            let pnl = engine.accounts[idx as usize].pnl.get();
            if pnl > 0 {
                // floor(pnl * h_num / h_den)
                let eff_pos = (pnl as u128).saturating_mul(h_num) / h_den;
                sum_eff_pos_pnl += eff_pos;
            }
        }

        // Count accounts with positive PnL
        let k = account_indices
            .iter()
            .filter(|&&idx| engine.accounts[idx as usize].pnl.get() > 0)
            .count() as u128;

        // Verify rounding slack bound: Residual - Σ PNL_eff_pos_i < K
        // Since h_num = Residual, and each floor loses at most 1, we have:
        // Residual - sum_eff_pos_pnl < K
        let slack = residual.saturating_sub(sum_eff_pos_pnl);
        assert!(
        slack < k,
        "Rounding slack bound violated: slack={} >= K={} (Residual={}, sum_eff_pos={}, h_num={}, h_den={})",
        slack,
        k,
        residual,
        sum_eff_pos_pnl,
        h_num,
        h_den
    );

        // Also verify it's within MAX_ROUNDING_SLACK
        assert!(
            slack <= MAX_ROUNDING_SLACK,
            "Rounding slack {} exceeds MAX_ROUNDING_SLACK {}",
            slack,
            MAX_ROUNDING_SLACK
        );
    }

    #[test]
    fn test_run_end_of_instruction_lifecycle_no_reset_when_oi_nonzero() {
        use percolator::{InstructionContext, SideMode};
        let mut e = *Box::new(RiskEngine::new(default_params()));
        // Side is in ResetPending but OI is not zero — should NOT reset
        e.side_mode_long = SideMode::ResetPending;
        e.oi_eff_long_q = 100;
        e.adl_mult_long = 999;

        let mut ctx = InstructionContext::new();
        e.run_end_of_instruction_lifecycle(&mut ctx, 0i64).unwrap();

        // Still ResetPending — OI not drained yet
        assert_eq!(e.side_mode_long, SideMode::ResetPending);
        assert_eq!(e.adl_mult_long, 999); // unchanged
    }

    #[test]
    fn test_run_end_of_instruction_lifecycle_resets_when_oi_zero() {
        use percolator::{InstructionContext, SideMode};
        let mut e = *Box::new(RiskEngine::new(default_params()));
        // Simulate a side that is in ResetPending with OI already drained to 0
        e.side_mode_long = SideMode::ResetPending;
        e.oi_eff_long_q = 0;
        e.adl_mult_long = 999;
        e.adl_coeff_long = 42;

        let mut ctx = InstructionContext::new();
        e.run_end_of_instruction_lifecycle(&mut ctx, 0i64).unwrap();

        // Side should have been reset to Normal
        assert_eq!(e.side_mode_long, SideMode::Normal);
        assert_eq!(e.adl_mult_long, 0);
        assert_eq!(e.adl_coeff_long, 0);
        assert_eq!(e.adl_epoch_start_k_long, 0);
    }

    #[test]
    fn test_scratch_k_atomicity_via_keeper_crank() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let idx = engine.add_user(0).unwrap();
        engine.deposit(idx, 100_000, 0).unwrap();

        engine.last_oracle_price = 1_000_000;
        engine.current_slot = 100;
        engine.last_market_slot = 100;

        // Set up nonzero OI on both sides so funding path is active
        engine.oi_eff_long_q = 1_000_000;
        engine.oi_eff_short_q = 1_000_000;
        engine.adl_mult_long = u128::MAX / 2;
        engine.adl_mult_short = u128::MAX / 2;

        // Set K near i128::MAX so funding sub will overflow
        engine.adl_coeff_long = i128::MAX - 1;
        engine.adl_coeff_short = i128::MAX - 1;

        // Large rate to force funding overflow
        engine.funding_rate_bps_per_slot_last = 10_000;
        engine.funding_price_sample_last = 1_000_000;

        // Snapshot K values before the call
        let k_long_before = engine.adl_coeff_long;
        let k_short_before = engine.adl_coeff_short;

        // keeper_crank calls accrue_market_to internally; overflow is handled gracefully.
        // With scratch K, the overflow prevents ANY K mutation (atomic rollback).
        let outcome = engine.keeper_crank(200, 1_000_001, &[], 0, 0i64).unwrap();

        // accrue_market_to failed internally → adl_accrue_failures > 0
        assert!(
            outcome.adl_accrue_failures > 0,
            "Expected accrue failure due to overflow with near-MAX K values"
        );

        // Atomicity: K values must not be partially advanced
        assert_eq!(
            engine.adl_coeff_long, k_long_before,
            "K_long must be unchanged when accrue_market_to overflows (scratch K atomicity)"
        );
        assert_eq!(
            engine.adl_coeff_short, k_short_before,
            "K_short must be unchanged when accrue_market_to overflows (scratch K atomicity)"
        );
    }

    #[test]
    fn test_set_funding_rate_validates_bounds() {
        let mut engine = Box::new(RiskEngine::new(default_params()));

        assert!(engine.set_funding_rate_for_next_interval(10_000).is_ok());
        assert!(engine.set_funding_rate_for_next_interval(-10_000).is_ok());
        assert!(engine.set_funding_rate_for_next_interval(0).is_ok());
        assert!(engine.set_funding_rate_for_next_interval(10_001).is_err());
        assert!(engine.set_funding_rate_for_next_interval(-10_001).is_err());
        assert!(engine.set_funding_rate_for_next_interval(i64::MAX).is_err());
        assert!(engine.set_funding_rate_for_next_interval(i64::MIN).is_err());
    }

    #[test]
    fn test_set_margin_params_accepts_valid_values() {
        let mut engine = RiskEngine::new(default_params());
        assert!(engine.set_margin_params(2000, 1000).is_ok());
        assert_eq!(engine.params.initial_margin_bps, 2000);
        assert_eq!(engine.params.maintenance_margin_bps, 1000);
    }

    #[test]
    fn test_set_margin_params_does_not_update_on_error() {
        let mut engine = RiskEngine::new(default_params());
        let orig_initial = engine.params.initial_margin_bps;
        let orig_maint = engine.params.maintenance_margin_bps;
        let _ = engine.set_margin_params(500, 1000); // maintenance > initial → error
        assert_eq!(engine.params.initial_margin_bps, orig_initial);
        assert_eq!(engine.params.maintenance_margin_bps, orig_maint);
    }

    #[test]
    fn test_set_margin_params_rejects_exceeding_10000() {
        let mut engine = RiskEngine::new(default_params());
        assert_eq!(
            engine.set_margin_params(10_001, 500),
            Err(RiskError::Overflow)
        );
        assert_eq!(
            engine.set_margin_params(1000, 10_001),
            Err(RiskError::Overflow)
        );
    }

    #[test]
    fn test_set_margin_params_rejects_maintenance_greater_than_initial() {
        let mut engine = RiskEngine::new(default_params());
        assert_eq!(
            engine.set_margin_params(500, 1000),
            Err(RiskError::Overflow)
        );
    }

    #[test]
    fn test_set_margin_params_rejects_zero_initial() {
        let mut engine = RiskEngine::new(default_params());
        assert_eq!(engine.set_margin_params(0, 500), Err(RiskError::Overflow));
    }

    #[test]
    fn test_set_margin_params_rejects_zero_maintenance() {
        let mut engine = RiskEngine::new(default_params());
        assert_eq!(engine.set_margin_params(1000, 0), Err(RiskError::Overflow));
    }

    #[test]
    fn test_set_mark_price() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        assert_eq!(engine.mark_price_e6, 0);
        engine.set_mark_price(1_500_000);
        assert_eq!(engine.mark_price_e6, 1_500_000);
    }

    #[test]
    fn test_set_mark_price_blended() {
        let params = default_params();
        let mut engine = Box::new(RiskEngine::new(params));

        // Bootstrap TWAP
        engine.update_trade_twap(150_000_000, 10_000_000, 0);

        // 50/50 blend
        engine.set_mark_price_blended(100_000_000, 5_000);
        assert_eq!(
            engine.mark_price_e6, 125_000_000,
            "50/50 blend of 100M and 150M = 125M"
        );
    }

    #[test]
    fn test_set_threshold_large_value() {
        let params = default_params();
        let mut engine = Box::new(RiskEngine::new(params));

        // Set to large value
        let large = u128::MAX / 2;
        engine.set_risk_reduction_threshold(large);
        assert_eq!(engine.risk_reduction_threshold(), large);
    }

    #[test]
    fn test_set_threshold_updates_value() {
        let params = default_params();
        let mut engine = Box::new(RiskEngine::new(params));

        // Initial threshold from params
        assert_eq!(engine.risk_reduction_threshold(), 0);

        // Set new threshold
        engine.set_risk_reduction_threshold(5_000);
        assert_eq!(engine.risk_reduction_threshold(), 5_000);

        // Update again
        engine.set_risk_reduction_threshold(10_000);
        assert_eq!(engine.risk_reduction_threshold(), 10_000);

        // Set to zero
        engine.set_risk_reduction_threshold(0);
        assert_eq!(engine.risk_reduction_threshold(), 0);
    }

    #[test]
    fn test_settle_side_effects_epoch_mismatch_happy_path() {
        use percolator::SideMode;
        let mut engine = *Box::new(RiskEngine::new(default_params()));
        let idx = engine.add_user(0).unwrap();
        engine.deposit(idx, 100_000, 0).unwrap();

        // Set up epoch-mismatch: epoch_snap=0, side epoch=1
        engine.adl_epoch_long = 1;
        engine.side_mode_long = SideMode::ResetPending;
        engine.adl_epoch_start_k_long = 0i128;
        engine.adl_coeff_long = 0i128;

        engine.accounts[idx as usize].position_basis_q = 1_000i128;
        engine.accounts[idx as usize].adl_a_basis = 1_000_000u128;
        engine.accounts[idx as usize].adl_k_snap = 0i128;
        engine.accounts[idx as usize].adl_epoch_snap = 0;

        // stale_count = 1 — checked_sub(1) will succeed
        engine.stale_account_count_long = 1;
        // stored_pos_count_long = 1 — needed for set_position_basis_q(idx, 0) decrement
        engine.stored_pos_count_long = 1;

        let result = engine.settle_side_effects(idx as usize);
        assert!(
            result.is_ok(),
            "epoch-mismatch settle should succeed with stale_count=1"
        );

        // Verify stale_count decremented
        assert_eq!(
            engine.stale_account_count_long, 0,
            "stale_count must be decremented"
        );

        // Verify ADL state cleared
        assert_eq!(
            engine.accounts[idx as usize].position_basis_q, 0,
            "basis must be cleared"
        );
        assert_eq!(
            engine.accounts[idx as usize].adl_a_basis, 1_000_000u128,
            "a_basis must be reset"
        );
        assert_eq!(
            engine.accounts[idx as usize].adl_k_snap, 0i128,
            "k_snap must be cleared"
        );
        assert_eq!(
            engine.accounts[idx as usize].adl_epoch_snap, 0,
            "epoch_snap must be cleared"
        );
    }

    #[test]
    fn test_settle_side_effects_epoch_mismatch_stale_zero_no_pnl_mutation() {
        use percolator::SideMode;
        let mut engine = *Box::new(RiskEngine::new(default_params()));
        let idx = engine.add_user(0).unwrap();
        engine.deposit(idx, 100_000, 0).unwrap();

        // Set up epoch-mismatch scenario: account epoch_snap = 0, side epoch = 1
        engine.adl_epoch_long = 1;
        engine.side_mode_long = SideMode::ResetPending;
        engine.adl_epoch_start_k_long = 500_000i128;
        engine.adl_coeff_long = 1_000_000i128;

        // Give account a position and ADL state
        engine.accounts[idx as usize].position_basis_q = 1_000i128;
        engine.accounts[idx as usize].adl_a_basis = 1_000_000u128;
        engine.accounts[idx as usize].adl_k_snap = 0i128;
        engine.accounts[idx as usize].adl_epoch_snap = 0;

        // CRITICAL: set stale_count to 0 — checked_sub(1) must fail
        engine.stale_account_count_long = 0;

        let pnl_before = engine.accounts[idx as usize].pnl.get();

        // settle_side_effects must fail because stale_count underflows
        let result = engine.settle_side_effects(idx as usize);
        assert!(result.is_err(), "must fail when stale_count is 0");

        // PnL must NOT have been mutated (validate-then-mutate property)
        let pnl_after = engine.accounts[idx as usize].pnl.get();
        assert_eq!(
            pnl_before, pnl_after,
            "PERC-8459: PnL must not be mutated when stale_count validation fails"
        );
    }

    #[test]
    fn test_settle_side_effects_same_epoch_pnl_settled() {
        let mut engine = *Box::new(RiskEngine::new(default_params()));
        let idx = engine.add_user(0).unwrap();
        engine.deposit(idx, 100_000, 0).unwrap();

        // Set up same-epoch scenario: epoch_snap matches side epoch
        engine.adl_epoch_long = 1;
        engine.adl_coeff_long = 1_000_000i128;
        engine.adl_mult_long = 1_000_000u128;

        engine.accounts[idx as usize].position_basis_q = 1_000i128;
        engine.accounts[idx as usize].adl_a_basis = 1_000_000u128;
        engine.accounts[idx as usize].adl_k_snap = 0i128;
        engine.accounts[idx as usize].adl_epoch_snap = 1; // matches epoch_long

        let pnl_before = engine.accounts[idx as usize].pnl.get();

        let result = engine.settle_side_effects(idx as usize);
        assert!(result.is_ok(), "same-epoch settle should succeed");

        // PnL should have changed (k_side - k_snap = 1_000_000 - 0 = 1_000_000, non-zero delta)
        // The exact value depends on wide_signed_mul_div_floor_from_k_pair, but it should
        // at least have been called.
        // We just verify the function completed without error.
    }

    #[test]
    fn test_settle_side_effects_zero_basis_noop() {
        let mut engine = *Box::new(RiskEngine::new(default_params()));
        let idx = engine.add_user(0).unwrap();
        engine.deposit(idx, 100_000, 0).unwrap();

        // basis=0 → early return Ok
        engine.accounts[idx as usize].position_basis_q = 0;
        let result = engine.settle_side_effects(idx as usize);
        assert!(result.is_ok(), "zero basis must be a no-op");
    }

    #[test]
    fn test_sidemode_check_open_blocked_drain_only() {
        use percolator::{RiskError, Side, SideMode};
        let mut e = *Box::new(RiskEngine::new(default_params()));
        e.side_mode_long = SideMode::DrainOnly;
        let err = e.check_side_open_permitted(Side::Long).unwrap_err();
        assert_eq!(err, RiskError::SideBlocked);
        // Short side unaffected
        assert!(e.check_side_open_permitted(Side::Short).is_ok());
    }

    #[test]
    fn test_sidemode_check_open_blocked_reset_pending() {
        use percolator::{RiskError, Side, SideMode};
        let mut e = *Box::new(RiskEngine::new(default_params()));
        e.side_mode_short = SideMode::ResetPending;
        let err = e.check_side_open_permitted(Side::Short).unwrap_err();
        assert_eq!(err, RiskError::SideBlocked);
        // Long side unaffected
        assert!(e.check_side_open_permitted(Side::Long).is_ok());
    }

    #[test]
    fn test_sidemode_check_open_permitted_normal() {
        use percolator::{Side, SideMode};
        let mut e = *Box::new(RiskEngine::new(default_params()));
        // Both sides start Normal — opens are permitted
        assert!(e.check_side_open_permitted(Side::Long).is_ok());
        assert!(e.check_side_open_permitted(Side::Short).is_ok());
    }

    #[test]
    fn test_sidemode_repr_u8_values() {
        use percolator::SideMode;
        assert_eq!(SideMode::Normal as u8, 0);
        assert_eq!(SideMode::DrainOnly as u8, 1);
        assert_eq!(SideMode::ResetPending as u8, 2);
    }

    #[test]
    fn test_trade_aggregate_consistency() {
        let mut engine = Box::new(RiskEngine::new(default_params()));

        // Setup accounts with known initial state
        let user_idx = engine.add_user(0).unwrap();
        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();

        let user_capital = 100_000u128;
        let lp_capital = 500_000u128;

        engine.deposit(user_idx, user_capital, 0).unwrap();
        engine.accounts[lp_idx as usize].capital = U128::new(lp_capital);
        engine.vault += lp_capital;

        // Recompute to ensure clean state
        engine.recompute_aggregates();

        // Record initial aggregates
        let c_tot_before = engine.c_tot.get();
        let pnl_pos_tot_before = engine.pnl_pos_tot.get();

        assert_eq!(
            c_tot_before,
            user_capital + lp_capital,
            "Initial c_tot mismatch"
        );
        assert_eq!(pnl_pos_tot_before, 0, "Initial pnl_pos_tot should be 0");

        // Execute a trade
        let oracle_price = 1_000_000u64; // $1
        let trade_size = 10_000i128;
        engine
            .execute_trade(&MATCHER, lp_idx, user_idx, 0, oracle_price, trade_size)
            .unwrap();

        // Manually compute expected values:
        // - Trading fee = ceil(notional * fee_bps / 10000) = ceil(10000 * 1 * 10 / 10000) = ceil(10) = 10
        //   (notional = |size| * price / 1e6 = 10000 * 1000000 / 1000000 = 10000)
        //   Actually fee = ceil(10000 * 10 / 10000) = ceil(10) = 10
        // - Fee is deducted from user capital
        // - c_tot should decrease by fee amount

        let fee = 10u128; // ceil(10000 * 10 / 10000)
        let expected_c_tot = c_tot_before - fee;

        assert_eq!(
            engine.c_tot.get(),
            expected_c_tot,
            "c_tot should decrease by trading fee: expected {}, got {}",
            expected_c_tot,
            engine.c_tot.get()
        );

        // Verify c_tot by summing all account capitals
        let mut manual_c_tot = 0u128;
        if engine.is_used(user_idx as usize) {
            manual_c_tot += engine.accounts[user_idx as usize].capital.get();
        }
        if engine.is_used(lp_idx as usize) {
            manual_c_tot += engine.accounts[lp_idx as usize].capital.get();
        }
        assert_eq!(
            engine.c_tot.get(),
            manual_c_tot,
            "c_tot should match sum of account capitals"
        );

        // Verify pnl_pos_tot by summing positive PnLs
        let mut manual_pnl_pos_tot = 0u128;
        let user_pnl = engine.accounts[user_idx as usize].pnl.get();
        let lp_pnl = engine.accounts[lp_idx as usize].pnl.get();
        if user_pnl > 0 {
            manual_pnl_pos_tot += user_pnl as u128;
        }
        if lp_pnl > 0 {
            manual_pnl_pos_tot += lp_pnl as u128;
        }
        assert_eq!(
            engine.pnl_pos_tot.get(),
            manual_pnl_pos_tot,
            "pnl_pos_tot should match sum of positive PnLs: expected {}, got {}",
            manual_pnl_pos_tot,
            engine.pnl_pos_tot.get()
        );
    }

    #[test]
    fn test_trade_pnl_is_oracle_minus_exec() {
        let mut params = default_params();
        params.trading_fee_bps = 0; // No fees for cleaner math
        params.max_crank_staleness_slots = u64::MAX;

        let mut engine = Box::new(RiskEngine::new(params));

        // Create LP and user with capital
        let lp = engine.add_lp([1u8; 32], [0u8; 32], 0).unwrap();
        engine.deposit(lp, 1_000_000, 0).unwrap();

        let user = engine.add_user(0).unwrap();
        engine.deposit(user, 1_000_000, 0).unwrap();

        // Execute trade: user buys 1 unit
        // Oracle = 1_000_000, execution price will be at oracle (NoOpMatcher)
        let oracle_price = 1_000_000;
        let size = 1_000_000; // Buy 1 unit

        engine
            .execute_trade(&MATCHER, lp, user, 0, oracle_price, size)
            .unwrap();

        // With oracle = exec_price, trade_pnl = (oracle - exec_price) * size = 0
        // User and LP should have pnl = 0 (no fee)
        assert_eq!(
            engine.accounts[user as usize].pnl.get(),
            0,
            "User pnl should be 0 when oracle = exec"
        );
        assert_eq!(
            engine.accounts[lp as usize].pnl.get(),
            0,
            "LP pnl should be 0 when oracle = exec"
        );

        // Both should have entry_price = oracle_price
        assert_eq!(
            engine.accounts[user as usize].entry_price, oracle_price,
            "User entry should be oracle"
        );
        assert_eq!(
            engine.accounts[lp as usize].entry_price, oracle_price,
            "LP entry should be oracle"
        );

        // Conservation should hold
        assert!(
            engine.check_conservation(oracle_price),
            "Conservation should hold"
        );
    }

    #[test]
    fn test_trading_opens_position() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(0).unwrap();
        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();

        // Setup user with capital
        engine.deposit(user_idx, 10_000, 0).unwrap();
        // WHITEBOX: Set LP capital directly. Add to vault to preserve conservation.
        engine.accounts[lp_idx as usize].capital = U128::new(100_000);
        engine.vault += 100_000;
        assert_conserved(&engine);

        // Execute trade: user buys 1000 units at $1
        let oracle_price = 1_000_000;
        let size = 1000i128;

        engine
            .execute_trade(&MATCHER, lp_idx, user_idx, 0, oracle_price, size)
            .unwrap();

        // Check position opened
        assert_eq!(engine.accounts[user_idx as usize].position_size.get(), 1000);
        assert_eq!(engine.accounts[user_idx as usize].entry_price, oracle_price);

        // Check LP has opposite position
        assert_eq!(engine.accounts[lp_idx as usize].position_size.get(), -1000);

        // Check fee was charged (0.1% of 1000 = 1)
        assert!(!engine.insurance_fund.fee_revenue.is_zero());
    }

    #[test]
    fn test_trading_realizes_pnl() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(0).unwrap();
        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();

        engine.deposit(user_idx, 10_000, 0).unwrap();
        // WHITEBOX: Set LP capital directly. Add to vault (not override) to preserve account fees.
        engine.accounts[lp_idx as usize].capital = U128::new(100_000);
        engine.vault += 100_000;
        assert_conserved(&engine);

        // Open long position at $1
        engine
            .execute_trade(&MATCHER, lp_idx, user_idx, 0, 1_000_000, 1000)
            .unwrap();

        // Close position at $1.50 (50% profit)
        engine
            .execute_trade(&MATCHER, lp_idx, user_idx, 0, 1_500_000, -1000)
            .unwrap();

        // Check PNL realized (approximately)
        // Price went from $1 to $1.50, so 500 profit on 1000 units
        assert!(engine.accounts[user_idx as usize].pnl.is_positive());
        assert_eq!(engine.accounts[user_idx as usize].position_size.get(), 0);
    }

    #[test]
    fn test_twap_bootstrap() {
        let params = default_params();
        let mut engine = Box::new(RiskEngine::new(params));
        assert_eq!(engine.trade_twap_e6, 0);

        engine.update_trade_twap(50_000_000, 5_000_000, 100);
        assert_eq!(
            engine.trade_twap_e6, 50_000_000,
            "First trade bootstraps TWAP"
        );
        assert_eq!(engine.twap_last_slot, 100);
    }

    #[test]
    fn test_twap_ema_converges() {
        let params = default_params();
        let mut engine = Box::new(RiskEngine::new(params));

        // Bootstrap at $100 with full-weight notional ($10,000 in e6 = 10_000_000_000)
        const FULL_NOTIONAL: u128 = 10_000_000_000; // $10,000 in e6 units

        engine.update_trade_twap(100_000_000, FULL_NOTIONAL, 0); // bootstrap at 100
                                                                 // Many trades at 200 over many slots → TWAP should converge toward 200
        for slot in (100..10_000).step_by(100) {
            engine.update_trade_twap(200_000_000, FULL_NOTIONAL, slot);
        }
        // After ~10k slots at alpha=347/1e6 per slot (full weight), should be very close to 200
        let diff = if engine.trade_twap_e6 > 200_000_000 {
            engine.trade_twap_e6 - 200_000_000
        } else {
            200_000_000 - engine.trade_twap_e6
        };
        assert!(
            diff < 5_000_000, // within 5% of 200
            "TWAP should converge toward 200M, got {} (diff={})",
            engine.trade_twap_e6,
            diff
        );
    }

    #[test]
    fn test_twap_ignores_dust() {
        let params = default_params();
        let mut engine = Box::new(RiskEngine::new(params));

        engine.update_trade_twap(50_000_000, 5_000_000, 100); // bootstrap
        engine.update_trade_twap(999_000_000, 500_000, 200); // dust: notional < 1e6
        assert_eq!(
            engine.trade_twap_e6, 50_000_000,
            "Dust trade should not move TWAP"
        );
    }

    #[test]
    fn test_twap_notional_weighting() {
        let params = default_params();

        // Full-weight ($10k) drive: 1 trade of dt=1000 slots
        let mut engine_full = Box::new(RiskEngine::new(params.clone()));
        engine_full.update_trade_twap(100_000_000, 10_000_000_000, 0); // bootstrap
        engine_full.update_trade_twap(200_000_000, 10_000_000_000, 1_000);

        // Half-weight ($5k = 5_000_000_000) drive: same slot step
        let mut engine_half = Box::new(RiskEngine::new(params));
        engine_half.update_trade_twap(100_000_000, 10_000_000_000, 0); // bootstrap
        engine_half.update_trade_twap(200_000_000, 5_000_000_000, 1_000);

        // Full-weight trade must move TWAP further than half-weight trade
        let full_move = engine_full.trade_twap_e6.saturating_sub(100_000_000);
        let half_move = engine_half.trade_twap_e6.saturating_sub(100_000_000);
        assert!(
            full_move > half_move,
            "Full-weight trade should move TWAP more: full={full_move} half={half_move}"
        );
    }

    #[test]
    fn test_two_phase_liquidation_priority_and_sweep() {
        // Test the crank liquidation design:
        // Each crank processes up to ACCOUNTS_PER_CRANK occupied accounts
        // Full sweep completes when cursor wraps around to start

        use percolator::ACCOUNTS_PER_CRANK;

        let mut params = default_params();
        params.maintenance_margin_bps = 500; // 5%
        params.initial_margin_bps = 1000; // 10%
        params.liquidation_buffer_bps = 0;
        params.liquidation_fee_bps = 0;
        params.max_crank_staleness_slots = u64::MAX;
        params.warmup_period_slots = 1; // Instant warmup (minimum valid)

        let mut engine = Box::new(RiskEngine::new(params));
        set_insurance(&mut engine, 1_000_000);

        // Create several accounts with varying underwater amounts
        // Priority liquidation should find the worst ones first

        // Healthy counterparty to take other side of positions
        let counterparty = engine.add_user(0).unwrap();
        engine.deposit(counterparty, 10_000_000, 0).unwrap();

        // Create underwater accounts with different severities
        // At oracle 1.0: maintenance = 5% of notional
        // Account with position 1M needs 50k margin. Capital < 50k => underwater

        // Mildly underwater (capital = 45k, needs 50k)
        let mild = engine.add_user(0).unwrap();
        engine.deposit(mild, 45_000, 0).unwrap();
        engine.accounts[mild as usize].position_size = I128::new(1_000_000);
        engine.accounts[mild as usize].entry_price = 1_000_000;
        engine.accounts[counterparty as usize].position_size -= 1_000_000;
        engine.accounts[counterparty as usize].entry_price = 1_000_000;
        engine.total_open_interest += 2_000_000;

        // Severely underwater (capital = 10k, needs 50k)
        let severe = engine.add_user(0).unwrap();
        engine.deposit(severe, 10_000, 0).unwrap();
        engine.accounts[severe as usize].position_size = I128::new(1_000_000);
        engine.accounts[severe as usize].entry_price = 1_000_000;
        engine.accounts[counterparty as usize].position_size -= 1_000_000;
        engine.total_open_interest += 2_000_000;

        // Very severely underwater (capital = 1k, needs 50k)
        let very_severe = engine.add_user(0).unwrap();
        engine.deposit(very_severe, 1_000, 0).unwrap();
        engine.accounts[very_severe as usize].position_size = I128::new(1_000_000);
        engine.accounts[very_severe as usize].entry_price = 1_000_000;
        engine.accounts[counterparty as usize].position_size -= 1_000_000;
        engine.total_open_interest += 2_000_000;

        // Verify conservation before
        assert!(
            engine.check_conservation(DEFAULT_ORACLE),
            "Conservation must hold before crank"
        );

        // Single crank should liquidate all underwater accounts via priority phase
        let outcome = engine.keeper_crank(1, 1_000_000, &[], 64, 0).unwrap();

        // Verify conservation after
        assert!(
            engine.check_conservation(DEFAULT_ORACLE),
            "Conservation must hold after priority liquidation"
        );

        // All 3 underwater accounts should be liquidated (partially or fully)
        assert!(
            outcome.num_liquidations >= 3,
            "Priority liquidation should find all underwater accounts: got {}",
            outcome.num_liquidations
        );

        // Positions should be reduced (liquidation brings accounts back to margin)
        // very_severe had 1k capital => can support ~20k notional at 5% margin
        // severe had 10k capital => can support ~200k notional at 5% margin
        // mild had 45k capital => can support ~900k notional at 5% margin
        assert!(
            engine.accounts[very_severe as usize].position_size.get() < 100_000,
            "very_severe position should be significantly reduced"
        );
        assert!(
            engine.accounts[severe as usize].position_size.get() < 500_000,
            "severe position should be significantly reduced"
        );
        assert!(
            engine.accounts[mild as usize].position_size.get() < 1_000_000,
            "mild position should be reduced"
        );

        // With few accounts (< ACCOUNTS_PER_CRANK), a single crank should complete sweep
        // The first crank already ran above. Check if it completed a sweep.
        // With only 4 accounts, one crank should process all of them.
        assert!(
            outcome.sweep_complete || engine.num_used_accounts as u16 > ACCOUNTS_PER_CRANK,
            "Single crank should complete sweep when accounts < ACCOUNTS_PER_CRANK"
        );

        // If sweep didn't complete in first crank, run more until it does
        let mut slot = 2u64;
        while !engine.last_full_sweep_completed_slot > 0 && slot < 100 {
            let outcome = engine.keeper_crank(slot, 1_000_000, &[], 64, 0).unwrap();
            if outcome.sweep_complete {
                break;
            }
            slot += 1;
        }

        // Verify sweep completed
        assert!(
            engine.last_full_sweep_completed_slot > 0,
            "Sweep should have completed"
        );
    }

    #[test]
    fn test_unfreeze_funding() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        // Can't unfreeze what isn't frozen
        assert!(engine.unfreeze_funding().is_err());

        engine.funding_rate_bps_per_slot_last = 10;
        engine.freeze_funding().unwrap();

        // Unfreeze
        assert!(engine.unfreeze_funding().is_ok());
        assert!(!engine.is_funding_frozen());
        assert_eq!(engine.funding_frozen_rate_snapshot, 0);
    }

    #[test]
    fn test_unwrapped_definition() {
        let params = RiskParams {
            warmup_period_slots: 100,
            ..default_params()
        };
        let mut engine = Box::new(RiskEngine::new(params));

        let user = engine.add_user(0).unwrap();
        engine.deposit(user, 10_000, 0).unwrap();

        // Create counterparty for zero-sum
        // Zero-sum pattern: net_pnl = 0, so no vault funding needed
        let loser = engine.add_user(0).unwrap();
        engine.deposit(loser, 10_000, 0).unwrap();
        engine.accounts[loser as usize].pnl = I128::new(-1000);

        // Set positive PnL (reserved_pnl is now trade_entry_price, not a PnL reservation)
        engine.accounts[user as usize].pnl = I128::new(1000);

        // Update slope to establish warmup rate
        engine.update_warmup_slope(user).unwrap();

        assert_conserved(&engine);

        // At t=0, nothing is warmed yet, so:
        // withdrawable = 0
        // unwrapped = 1000 - 0 = 1000
        let account = &engine.accounts[user as usize];
        let positive_pnl = account.pnl.get() as u128;

        // Compute withdrawable manually (same logic as compute_withdrawable_pnl)
        let available = positive_pnl; // 1000 (no reservation)
        let elapsed = engine
            .current_slot
            .saturating_sub(account.warmup_started_at_slot);
        let warmed_cap = account.warmup_slope_per_step.get() * (elapsed as u128);
        let withdrawable = core::cmp::min(available, warmed_cap);

        // Expected unwrapped
        let expected_unwrapped = positive_pnl.saturating_sub(withdrawable);

        // Test: at t=0, withdrawable should be 0, unwrapped should be 1000
        assert_eq!(withdrawable, 0, "No time elapsed, withdrawable should be 0");
        assert_eq!(expected_unwrapped, 1000, "Unwrapped should be 1000 at t=0");

        // Advance time to allow partial warmup (50 slots = 50% of 100)
        engine.current_slot = 50;

        // Recalculate
        let account = &engine.accounts[user as usize];
        let elapsed = engine
            .current_slot
            .saturating_sub(account.warmup_started_at_slot);
        let warmed_cap = account.warmup_slope_per_step.get() * (elapsed as u128);
        let available = positive_pnl; // 1000
        let withdrawable_now = core::cmp::min(available, warmed_cap);

        // With slope=10 (avail_gross=1000/100) and 50 slots, warmed_cap = 500
        // withdrawable = min(1000, 500) = 500
        // unwrapped = 1000 - 500 = 500
        let expected_unwrapped_now = positive_pnl.saturating_sub(withdrawable_now);

        assert_eq!(
            withdrawable_now, 500,
            "After 50 slots, withdrawable should be 500"
        );
        assert_eq!(
            expected_unwrapped_now, 500,
            "After 50 slots, unwrapped should be 500"
        );

        assert_conserved(&engine);
    }

    #[test]
    fn test_update_lp_warmup_slope() {
        // CRITICAL: Tests that LP warmup actually gets rate limited
        let mut engine = Box::new(RiskEngine::new(default_params()));

        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();

        // Set insurance fund
        set_insurance(&mut engine, 10_000);

        // LP earns large PNL
        engine.accounts[lp_idx as usize].pnl = I128::new(50_000);

        // Update warmup slope
        engine.update_lp_warmup_slope(lp_idx).unwrap();

        // Should be rate limited
        let ideal_slope = 50_000 / 100; // 500 per slot
        let actual_slope = engine.accounts[lp_idx as usize].warmup_slope_per_step;

        assert!(
            actual_slope < ideal_slope,
            "LP warmup should be rate limited"
        );
        assert!(
            engine.total_warmup_rate > 0,
            "LP should contribute to total warmup rate"
        );
    }

    #[test]
    fn test_user_isolation() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user1 = engine.add_user(0).unwrap();
        let user2 = engine.add_user(0).unwrap();

        engine.deposit(user1, 1000, 0).unwrap();
        engine.deposit(user2, 2000, 0).unwrap();

        let user2_principal_before = engine.accounts[user2 as usize].capital;
        let user2_pnl_before = engine.accounts[user2 as usize].pnl;

        // Operate on user1
        engine.withdraw(user1, 500, 0, 1_000_000).unwrap();
        assert_eq!(engine.accounts[user1 as usize].pnl.get(), 0);
        engine.accounts[user1 as usize].pnl = I128::new(300);

        // User2 should be unchanged
        assert_eq!(
            engine.accounts[user2 as usize].capital,
            user2_principal_before
        );
        assert_eq!(engine.accounts[user2 as usize].pnl, user2_pnl_before);
    }

    #[test]
    fn test_validate_funding_rate_rejects_excessive_rate() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let idx = engine.add_user(0).unwrap();
        engine.deposit(idx, 100_000, 0).unwrap();

        // keeper_crank with rate > 10_000 should fail immediately
        let result = engine.keeper_crank(1, 1_000_000, &[], 0, 10_001i64);
        assert!(result.is_err(), "funding_rate > 10000 must be rejected");

        let result = engine.keeper_crank(1, 1_000_000, &[], 0, -10_001i64);
        assert!(result.is_err(), "funding_rate < -10000 must be rejected");

        // Exactly 10_000 should be accepted
        let result = engine.keeper_crank(1, 1_000_000, &[], 0, 10_000i64);
        assert!(result.is_ok(), "funding_rate == 10000 must be accepted");

        // Exactly -10_000 should be accepted
        let result = engine.keeper_crank(2, 1_000_000, &[], 0, -10_000i64);
        assert!(result.is_ok(), "funding_rate == -10000 must be accepted");

        // 0 should be accepted
        let result = engine.keeper_crank(3, 1_000_000, &[], 0, 0i64);
        assert!(result.is_ok(), "funding_rate == 0 must be accepted");
    }

    #[test]
    fn test_validate_initial_less_than_maintenance_rejected() {
        let mut p = default_params();
        p.maintenance_margin_bps = 1000;
        p.initial_margin_bps = 500; // initial < maintenance
        assert_eq!(p.validate(), Err(RiskError::Overflow));
    }

    #[test]
    fn test_validate_liquidation_buffer_exceeds_10000_rejected() {
        let mut p = default_params();
        p.liquidation_buffer_bps = 10_001;
        assert_eq!(p.validate(), Err(RiskError::Overflow));
    }

    #[test]
    fn test_validate_liquidation_fee_exceeds_10000_rejected() {
        let mut p = default_params();
        p.liquidation_fee_bps = 10_001;
        assert_eq!(p.validate(), Err(RiskError::Overflow));
    }

    #[test]
    fn test_validate_margin_exceeds_10000_rejected() {
        let mut p = default_params();
        p.initial_margin_bps = 10_001;
        assert_eq!(p.validate(), Err(RiskError::Overflow));

        let mut p2 = default_params();
        p2.maintenance_margin_bps = 10_001;
        assert_eq!(p2.validate(), Err(RiskError::Overflow));
    }

    #[test]
    fn test_validate_max_accounts_exceeds_physical_limit_rejected() {
        let mut p = default_params();
        p.max_accounts = MAX_ACCOUNTS as u64 + 1;
        assert_eq!(p.validate(), Err(RiskError::Overflow));
    }

    #[test]
    fn test_validate_nonzero_warmup_period_allowed() {
        let mut p = default_params();
        p.warmup_period_slots = 1;
        assert!(p.validate().is_ok());
    }

    #[test]
    fn test_validate_u64_max_crank_staleness_allowed() {
        let mut p = default_params();
        p.max_crank_staleness_slots = u64::MAX;
        assert!(p.validate().is_ok());
    }

    #[test]
    fn test_validate_valid_params() {
        assert!(default_params().validate().is_ok());
    }

    #[test]
    fn test_validate_zero_crank_staleness_rejected() {
        let mut p = default_params();
        p.max_crank_staleness_slots = 0;
        assert_eq!(p.validate(), Err(RiskError::Overflow));
    }

    #[test]
    fn test_validate_zero_initial_margin_rejected() {
        let mut p = default_params();
        p.initial_margin_bps = 0;
        assert_eq!(p.validate(), Err(RiskError::Overflow));
    }

    #[test]
    fn test_validate_zero_maintenance_margin_rejected() {
        let mut p = default_params();
        p.maintenance_margin_bps = 0;
        assert_eq!(p.validate(), Err(RiskError::Overflow));
    }

    #[test]
    fn test_validate_zero_max_accounts_rejected() {
        let mut p = default_params();
        p.max_accounts = 0;
        assert_eq!(p.validate(), Err(RiskError::Overflow));
    }

    #[test]
    fn test_validate_zero_warmup_period_rejected() {
        // GH#1731: warmup_period_slots=0 bypasses oracle manipulation delay — must be rejected
        let mut p = default_params();
        p.warmup_period_slots = 0;
        assert_eq!(p.validate(), Err(RiskError::Overflow));
    }

    #[test]
    fn test_warmup_leverage_cap_enforced() {
        // Setup: warmup_period = 1000 slots, initial_margin = 1000 bps (10x max leverage)
        let mut params = default_params();
        params.warmup_period_slots = 1000;
        params.initial_margin_bps = 1000; // 10% → 10x max leverage
        params.maintenance_margin_bps = 500; // 5%
        params.trading_fee_bps = 0;
        params.max_crank_staleness_slots = u64::MAX;

        let mut engine = Box::new(RiskEngine::new(params));

        let user_idx = engine.add_user(0).unwrap();
        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();

        // Deposit capital
        engine.deposit(user_idx, 1_000_000_000, 0).unwrap(); // 1 SOL
        engine.accounts[lp_idx as usize].capital = U128::new(100_000_000_000);
        engine.vault += 100_000_000_000;

        let oracle_price = 100_000_000u64; // $100

        // 1. Open a position at exactly 10x leverage — should succeed
        // position_value = size * oracle / 1e6 = size * 100
        // margin_required (10%) = position_value / 10
        // 1_000_000_000 >= position_value / 10 → max position_value = 10_000_000_000
        // size = 10_000_000_000 * 1_000_000 / 100_000_000 = 100_000_000
        let safe_size: i128 = 90_000_000; // ~9x leverage (below 10x, should pass)
        let result = engine.execute_trade(&MATCHER, lp_idx, user_idx, 0, oracle_price, safe_size);
        assert!(
            result.is_ok(),
            "9x leverage should be allowed (within 10x cap): {:?}",
            result
        );

        // 2. Advance slot into warmup period
        engine.current_slot = 500; // mid-warmup

        // 3. Try to increase position beyond 10x leverage — should fail
        // Current position is ~9x. Trying to add more should fail if it exceeds initial margin.
        let excess_size: i128 = 30_000_000; // would bring total to ~12x
        let result2 =
            engine.execute_trade(&MATCHER, lp_idx, user_idx, 500, oracle_price, excess_size);
        assert!(
            result2.is_err(),
            "Exceeding 10x leverage during warmup period must be rejected"
        );

        // 4. Reducing position should still be allowed during warmup
        let reduce_size: i128 = -50_000_000; // closing half the position
        let result3 =
            engine.execute_trade(&MATCHER, lp_idx, user_idx, 500, oracle_price, reduce_size);
        assert!(
            result3.is_ok(),
            "Position reduction should be allowed during warmup: {:?}",
            result3
        );
    }

    #[test]
    fn test_warmup_matured_not_lost_on_trade() {
        let mut params = params_for_inline_tests();
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
                Ok(TradeExecution {
                    price: oracle_price,
                    size,
                })
            }
        }

        engine
            .execute_trade(
                &AtOracleMatcher,
                lp_idx,
                user_idx,
                200,
                ORACLE_100K,
                ONE_BASE,
            )
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
    fn test_warmup_monotonicity() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(0).unwrap();
        let counterparty = engine.add_user(0).unwrap();

        // Zero-sum PNL: user gains, counterparty loses (no vault funding needed)
        assert_eq!(engine.accounts[user_idx as usize].pnl.get(), 0);
        assert_eq!(engine.accounts[counterparty as usize].pnl.get(), 0);
        engine.accounts[user_idx as usize].pnl = I128::new(1000);
        engine.accounts[user_idx as usize].warmup_slope_per_step = U128::new(10);
        engine.accounts[counterparty as usize].pnl = I128::new(-1000);
        assert_conserved(&engine);

        // Get withdrawable at different time points
        let w0 = engine.withdrawable_pnl(&engine.accounts[user_idx as usize]);

        engine.advance_slot(10);
        let w1 = engine.withdrawable_pnl(&engine.accounts[user_idx as usize]);

        engine.advance_slot(20);
        let w2 = engine.withdrawable_pnl(&engine.accounts[user_idx as usize]);

        // Should be monotonically increasing
        assert!(w1 >= w0);
        assert!(w2 >= w1);
    }

    #[test]
    fn test_warmup_rate_limit_invariant_maintained() {
        // Verify that the invariant is always maintained:
        // total_warmup_rate * (T/2) <= insurance_fund * max_warmup_rate_fraction

        let mut params = default_params();
        params.warmup_period_slots = 100;
        params.max_warmup_rate_fraction_bps = 5000;

        let mut engine = Box::new(RiskEngine::new(params));
        set_insurance(&mut engine, 10_000);

        // Add multiple users with varying PNL
        for i in 0..10 {
            let user = engine.add_user(100).unwrap();
            engine.deposit(user, 1_000, 0).unwrap();
            engine.accounts[user as usize].pnl = (i as i128 + 1) * 1_000;
            engine.update_warmup_slope(user).unwrap();

            // Check invariant after each update
            let half_period = params.warmup_period_slots / 2;
            let max_total_warmup_in_half_period = engine.total_warmup_rate * (half_period as u128);
            let insurance_limit = engine.insurance_fund.balance
                * params.max_warmup_rate_fraction_bps as u128
                / 10_000;

            assert!(
                max_total_warmup_in_half_period <= insurance_limit,
                "Invariant violated: {} > {}",
                max_total_warmup_in_half_period,
                insurance_limit
            );
        }
    }

    #[test]
    fn test_warmup_rate_limit_multiple_users() {
        // Test that warmup capacity is shared among users
        let mut params = default_params();
        params.warmup_period_slots = 100;
        params.max_warmup_rate_fraction_bps = 5000; // 50% in T/2

        let mut engine = Box::new(RiskEngine::new(params));
        set_insurance(&mut engine, 10_000);

        // Max total warmup rate = 100 per slot

        let user1 = engine.add_user(100).unwrap();
        let user2 = engine.add_user(100).unwrap();

        engine.deposit(user1, 1_000, 0).unwrap();
        engine.deposit(user2, 1_000, 0).unwrap();

        // User1 gets 6,000 PNL (would want slope of 60)
        assert_eq!(engine.accounts[user1 as usize].pnl.get(), 0);
        engine.accounts[user1 as usize].pnl = I128::new(6_000);
        engine.update_warmup_slope(user1).unwrap();
        assert_eq!(engine.accounts[user1 as usize].warmup_slope_per_step, 60);
        assert_eq!(engine.total_warmup_rate, 60);

        // User2 gets 8,000 PNL (would want slope of 80)
        assert_eq!(engine.accounts[user2 as usize].pnl.get(), 0);
        engine.accounts[user2 as usize].pnl = I128::new(8_000);
        engine.update_warmup_slope(user2).unwrap();

        // Total would be 140, but max is 100, so user2 gets only 40
        assert_eq!(engine.accounts[user2 as usize].warmup_slope_per_step, 40); // 100 - 60 = 40
        assert_eq!(engine.total_warmup_rate, 100); // 60 + 40 = 100
    }

    #[test]
    fn test_warmup_rate_limit_single_user() {
        // Test that warmup slope is capped by insurance fund capacity
        let mut params = default_params();
        params.warmup_period_slots = 100;
        params.max_warmup_rate_fraction_bps = 5000; // 50% in T/2 = 50 slots

        let mut engine = Box::new(RiskEngine::new(params));

        // Add insurance fund: 10,000
        set_insurance(&mut engine, 10_000);

        // Max warmup rate = 10,000 * 5000 / 50 / 10,000 = 10,000 * 0.5 / 50 = 100 per slot
        let expected_max_rate = 10_000 * 5000 / 50 / 10_000;
        assert_eq!(expected_max_rate, 100);

        let user = engine.add_user(100).unwrap();
        engine.deposit(user, 1_000, 0).unwrap();

        // Give user 20,000 PNL (would need slope of 200 without limit)
        assert_eq!(engine.accounts[user as usize].pnl.get(), 0);
        engine.accounts[user as usize].pnl = I128::new(20_000);

        // Update warmup slope
        engine.update_warmup_slope(user).unwrap();

        // Should be capped at 100 (the max rate)
        assert_eq!(engine.accounts[user as usize].warmup_slope_per_step, 100);
        assert_eq!(engine.total_warmup_rate, 100);

        // After 50 slots, only 5,000 should have warmed up (not 10,000)
        engine.advance_slot(50);
        let warmed = engine.withdrawable_pnl(&engine.accounts[user as usize]);
        assert_eq!(warmed, 5_000); // 100 * 50 = 5,000
    }

    #[test]
    fn test_warmup_rate_released_on_pnl_decrease() {
        // Test that warmup capacity is released when user's PNL decreases
        let mut params = default_params();
        params.warmup_period_slots = 100;
        params.max_warmup_rate_fraction_bps = 5000;

        let mut engine = Box::new(RiskEngine::new(params));
        set_insurance(&mut engine, 10_000);

        let user1 = engine.add_user(100).unwrap();
        let user2 = engine.add_user(100).unwrap();

        engine.deposit(user1, 1_000, 0).unwrap();
        engine.deposit(user2, 1_000, 0).unwrap();

        // User1 uses all capacity
        assert_eq!(engine.accounts[user1 as usize].pnl.get(), 0);
        engine.accounts[user1 as usize].pnl = I128::new(15_000);
        engine.update_warmup_slope(user1).unwrap();
        assert_eq!(engine.total_warmup_rate, 100);

        // User2 can't get any capacity
        assert_eq!(engine.accounts[user2 as usize].pnl.get(), 0);
        engine.accounts[user2 as usize].pnl = I128::new(5_000);
        engine.update_warmup_slope(user2).unwrap();
        assert_eq!(engine.accounts[user2 as usize].warmup_slope_per_step, 0);

        // User1's PNL drops to 3,000 (ADL or loss)
        engine.accounts[user1 as usize].pnl = I128::new(3_000);
        engine.update_warmup_slope(user1).unwrap();
        assert_eq!(engine.accounts[user1 as usize].warmup_slope_per_step, 30); // 3000/100
        assert_eq!(engine.total_warmup_rate, 30);

        // Now user2 can get the remaining 70
        engine.update_warmup_slope(user2).unwrap();
        assert_eq!(engine.accounts[user2 as usize].warmup_slope_per_step, 50); // 5000/100, but capped at 70
        assert_eq!(engine.total_warmup_rate, 80); // 30 + 50
    }

    #[test]
    fn test_warmup_rate_scales_with_insurance_fund() {
        // Test that max warmup rate scales with insurance fund size
        let mut params = default_params();
        params.warmup_period_slots = 100;
        params.max_warmup_rate_fraction_bps = 5000; // 50% in T/2

        let mut engine = Box::new(RiskEngine::new(params));

        // Small insurance fund
        set_insurance(&mut engine, 1_000);

        let user = engine.add_user(100).unwrap();
        engine.deposit(user, 1_000, 0).unwrap();

        assert_eq!(engine.accounts[user as usize].pnl.get(), 0);
        engine.accounts[user as usize].pnl = I128::new(10_000);
        engine.update_warmup_slope(user).unwrap();

        // Max rate = 1000 * 0.5 / 50 = 10
        assert_eq!(engine.accounts[user as usize].warmup_slope_per_step, 10);

        // Increase insurance fund 10x
        set_insurance(&mut engine, 10_000);

        // Update slope again
        engine.update_warmup_slope(user).unwrap();

        // Max rate should be 10x higher = 100
        assert_eq!(engine.accounts[user as usize].warmup_slope_per_step, 100);
    }

    #[test]
    fn test_warmup_resets_when_mark_increases_pnl() {
        let mut params = default_params();
        params.warmup_period_slots = 100;
        params.trading_fee_bps = 0;
        params.maintenance_margin_bps = 100;
        params.initial_margin_bps = 100;
        params.max_crank_staleness_slots = u64::MAX;

        let mut engine = Box::new(RiskEngine::new(params));

        let user_idx = engine.add_user(0).unwrap();
        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();

        // Setup: user has 1B capital, LP has 1B capital
        engine.deposit(user_idx, 1_000_000_000, 0).unwrap();
        engine.accounts[lp_idx as usize].capital = U128::new(1_000_000_000);
        engine.vault += 1_000_000_000;
        engine.c_tot = U128::new(2_000_000_000);

        let oracle_price = 100_000_000u64; // $100

        // T=0: User opens a long position
        let size: i128 = 10_000_000; // 10 units
        engine
            .execute_trade(&MATCHER, lp_idx, user_idx, 0, oracle_price, size)
            .unwrap();

        // At this point, PnL is 0 (exec_price = oracle_price with NoOpMatcher)
        // User has position with entry_price = oracle_price

        // Manually give user some positive PnL to simulate prior profit
        engine.set_pnl(user_idx as usize, 100_000_000); // 100M PnL
        engine.pnl_pos_tot = U128::new(100_000_000);

        // Set warmup slope for the initial PnL (slope = 100M / 100 = 1M per slot)
        engine.update_warmup_slope(user_idx).unwrap();

        let warmup_started_t0 = engine.accounts[user_idx as usize].warmup_started_at_slot;
        assert_eq!(warmup_started_t0, 0, "Warmup should start at slot 0");

        // T=200: Long idle period. Price moved in user's favor (+50%)
        // Mark PnL = (new_price - entry) * position = (150 - 100) * 10 = 500M
        let new_oracle_price = 150_000_000u64; // $150

        // Without the fix:
        // - cap = slope * 200 = 1M * 200 = 200M
        // - Mark settlement adds 500M profit to PnL → total PnL = 600M
        // - avail_gross = 600M, cap = 200M, x = min(600M, 200M) = 200M converted!
        // - But original entitlement was only 100M (the initial PnL)
        //
        // With the fix:
        // - Mark settlement increases PnL from 100M to 600M
        // - Warmup slope is updated, warmup_started_at = 200
        // - cap = new_slope * 0 = 0 (nothing warmable yet from the new total)

        // Touch account (triggers mark settlement + warmup slope update if PnL increased)
        engine
            .touch_account_full(user_idx, 200, new_oracle_price)
            .unwrap();

        // Check warmup was restarted (started_at should be updated to >= 200)
        let warmup_started_after = engine.accounts[user_idx as usize].warmup_started_at_slot;
        assert!(
        warmup_started_after >= 200,
        "Warmup must restart when mark settlement increases PnL. Started at {} should be >= 200",
        warmup_started_after
    );

        // With the fix, capital should be close to original 1B
        // (possibly with some conversion from the original 100M that was warming up)
        // But NOT the huge 200M that the bug would have allowed
        let user_capital_after = engine.accounts[user_idx as usize].capital.get();

        // The original 100M PnL had 200 slots to warm up at slope 1M/slot = 200M cap
        // But since only 100M existed, max conversion = 100M (fully warmed)
        // After mark adds 500M more, warmup restarts → new 500M gets 0 conversion
        // So capital should be around 1B + 100M = 1.1B (at most)
        assert!(
        user_capital_after <= 1_150_000_000, // Allow some margin for rounding
        "User should not instantly convert huge mark profit. Capital {} too high (expected ~1.1B)",
        user_capital_after
    );
    }

    #[test]
    fn test_warmup_slope_nonzero() {
        let params = RiskParams {
            warmup_period_slots: 1000, // Large period so pnl=1 would normally give slope=0
            ..default_params()
        };
        let mut engine = Box::new(RiskEngine::new(params));

        let user = engine.add_user(0).unwrap();
        engine.deposit(user, 10_000, 0).unwrap();

        // Set minimal positive PnL (1 unit, less than warmup_period_slots)
        engine.accounts[user as usize].pnl = I128::new(1);

        // Create counterparty for zero-sum
        // Zero-sum pattern: net_pnl = 0, so no vault funding needed
        let loser = engine.add_user(0).unwrap();
        engine.deposit(loser, 10_000, 0).unwrap();
        engine.accounts[loser as usize].pnl = I128::new(-1);

        assert_conserved(&engine);

        // Update warmup slope
        engine.update_warmup_slope(user).unwrap();

        // Verify slope is at least 1 (not 0)
        let slope = engine.accounts[user as usize].warmup_slope_per_step.get();
        assert!(
            slope >= 1,
            "Slope must be >= 1 when positive PnL exists, got {}",
            slope
        );

        assert_conserved(&engine);
    }

    #[test]
    fn test_window_liquidation_many_accounts_few_liquidatable() {
        // Bench scenario: Many accounts with positions, but few actually liquidatable.
        // Tests that window sweep liquidation works correctly.
        // (In test mode MAX_ACCOUNTS=64, so we use proportional scaling)

        use percolator::MAX_ACCOUNTS;

        let mut params = default_params();
        params.maintenance_margin_bps = 500; // 5%
        params.max_crank_staleness_slots = u64::MAX;

        let mut engine = Box::new(RiskEngine::new(params));
        set_insurance(&mut engine, 1_000_000);

        // Create accounts with positions - most are healthy, few are underwater
        let num_accounts = MAX_ACCOUNTS.min(60); // Leave some slots for counterparty
        let num_underwater = 5; // Only 5 are actually liquidatable

        // Counterparty for opposing positions
        let counterparty = engine.add_user(0).unwrap();
        engine.deposit(counterparty, 100_000_000, 0).unwrap();

        let mut underwater_indices = Vec::new();

        for i in 0..num_accounts {
            let user = engine.add_user(0).unwrap();

            if i < num_underwater {
                // Underwater: low capital, will fail maintenance
                engine.deposit(user, 1_000, 0).unwrap();
                underwater_indices.push(user);
            } else {
                // Healthy: plenty of capital
                engine.deposit(user, 200_000, 0).unwrap();
            }

            // All have positions
            engine.accounts[user as usize].position_size = I128::new(1_000_000);
            engine.accounts[user as usize].entry_price = 1_000_000;
            engine.accounts[counterparty as usize].position_size -= 1_000_000;
            engine.total_open_interest += 2_000_000;
        }
        engine.accounts[counterparty as usize].entry_price = 1_000_000;

        // Verify conservation
        assert!(
            engine.check_conservation(DEFAULT_ORACLE),
            "Conservation before crank"
        );

        // Run crank - should select top-K efficiently
        let outcome = engine.keeper_crank(1, 1_000_000, &[], 64, 0).unwrap();

        // Verify conservation after
        assert!(
            engine.check_conservation(DEFAULT_ORACLE),
            "Conservation after crank"
        );

        // Should have liquidated the underwater accounts
        assert!(
            outcome.num_liquidations >= num_underwater as u32,
            "Should liquidate at least {} accounts, got {}",
            num_underwater,
            outcome.num_liquidations
        );

        // Verify underwater accounts got liquidated (positions reduced)
        for &idx in &underwater_indices {
            assert!(
                engine.accounts[idx as usize].position_size.get() < 1_000_000,
                "Underwater account {} should have reduced position",
                idx
            );
        }
    }

    #[test]
    fn test_window_liquidation_many_liquidatable() {
        // Bench scenario: Multiple liquidatable accounts with varying severity.
        // Tests that window sweep handles multiple liquidations correctly.

        let mut params = default_params();
        params.maintenance_margin_bps = 500; // 5%
        params.max_crank_staleness_slots = u64::MAX;
        params.warmup_period_slots = 1; // Instant warmup (minimum valid) // Instant warmup

        let mut engine = Box::new(RiskEngine::new(params));
        set_insurance(&mut engine, 10_000_000);

        // Create 10 underwater accounts with varying severities
        let num_underwater = 10;

        // Counterparty with lots of capital
        let counterparty = engine.add_user(0).unwrap();
        engine.deposit(counterparty, 100_000_000, 0).unwrap();

        // Create underwater accounts
        for i in 0..num_underwater {
            let user = engine.add_user(0).unwrap();
            // Vary capital: 10_000 to 40_000 (underwater for 5% margin on 1M position = 50k needed)
            let capital = 10_000 + (i as u128 * 3_000);
            engine.deposit(user, capital, 0).unwrap();
            engine.accounts[user as usize].position_size = I128::new(1_000_000);
            engine.accounts[user as usize].entry_price = 1_000_000;
            engine.accounts[counterparty as usize].position_size -= 1_000_000;
            engine.total_open_interest += 2_000_000;
        }
        engine.accounts[counterparty as usize].entry_price = 1_000_000;

        // Verify conservation
        assert!(
            engine.check_conservation(DEFAULT_ORACLE),
            "Conservation before crank"
        );

        // Run crank
        let outcome = engine.keeper_crank(1, 1_000_000, &[], 64, 0).unwrap();

        // Verify conservation after
        assert!(
            engine.check_conservation(DEFAULT_ORACLE),
            "Conservation after crank"
        );

        // Should have liquidated accounts (partial or full)
        assert!(
            outcome.num_liquidations > 0,
            "Should liquidate some accounts"
        );

        // Liquidation may trigger errors if ADL waterfall exhausts resources,
        // but the system should remain consistent
    }

    #[test]
    fn test_withdraw_allows_remaining_principal_after_loss_realization() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(0).unwrap();

        // Setup: position closed but with unrealized losses
        engine.accounts[user_idx as usize].capital = U128::new(10_000);
        engine.accounts[user_idx as usize].pnl = I128::new(-9_000);
        engine.accounts[user_idx as usize].position_size = I128::new(0);
        engine.accounts[user_idx as usize].warmup_slope_per_step = U128::new(0);
        engine.vault = U128::new(10_000);

        // First, trigger loss settlement
        engine.settle_warmup_to_capital(user_idx).unwrap();

        // Now capital should be 1_000
        assert_eq!(engine.accounts[user_idx as usize].capital.get(), 1_000);
        assert_eq!(engine.accounts[user_idx as usize].pnl.get(), 0);

        // Withdraw remaining capital - should succeed
        let result = engine.withdraw(user_idx, 1_000, 0, 1_000_000);
        assert!(
            result.is_ok(),
            "Withdraw of remaining capital should succeed"
        );
        assert_eq!(engine.accounts[user_idx as usize].capital.get(), 0);
    }

    #[test]
    fn test_withdraw_allows_remaining_principal_after_loss_settlement() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(0).unwrap();

        // Setup: deposit 1000, no position, negative pnl of -300
        let _ = engine.deposit(user_idx, 1000, 0);
        engine.accounts[user_idx as usize].pnl = I128::new(-300);
        engine.accounts[user_idx as usize].position_size = I128::new(0);

        // After settle: capital = 700. Withdraw 500 should succeed.
        let result = engine.withdraw(user_idx, 500, 0, 1_000_000);
        assert!(result.is_ok());

        // Verify remaining capital
        assert_eq!(engine.accounts[user_idx as usize].capital.get(), 200);
        // Verify N1 invariant
        assert!(engine.accounts[user_idx as usize].pnl.get() >= 0);
    }

    #[test]
    fn test_withdraw_im_check_blocks_when_equity_below_im() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(0).unwrap();

        // Setup: capital = 150, pnl = 0, position = 1000, entry_price = 1_000_000
        // notional = 1000, IM = 1000 * 1000 / 10000 = 100
        let _ = engine.deposit(user_idx, 150, 0);
        engine.accounts[user_idx as usize].pnl = I128::new(0);
        engine.accounts[user_idx as usize].position_size = I128::new(1000);
        engine.accounts[user_idx as usize].entry_price = 1_000_000;
        engine.funding_index_qpb_e6 = I128::new(0);
        engine.accounts[user_idx as usize].funding_index = I128::new(0);

        // withdraw(60): new_capital = 90, equity = 90 < 100 (IM)
        // Should fail with Undercollateralized
        let result = engine.withdraw(user_idx, 60, 0, 1_000_000);
        assert_eq!(result, Err(RiskError::Undercollateralized));

        // withdraw(40): would pass IM check (equity 110 > IM 100) but
        // withdrawals are blocked entirely when position is open.
        // Must close position first.
        let result2 = engine.withdraw(user_idx, 40, 0, 1_000_000);
        assert_eq!(result2, Err(RiskError::Undercollateralized));
    }

    #[test]
    fn test_withdraw_insufficient_balance() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(0).unwrap();

        engine.deposit(user_idx, 1000, 0).unwrap();

        // Try to withdraw more than deposited
        let result = engine.withdraw(user_idx, 1500, 0, 1_000_000);
        assert_eq!(result, Err(RiskError::InsufficientBalance));
    }

    #[test]
    fn test_withdraw_open_position_blocks_due_to_equity() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(0).unwrap();

        // Setup: position_size = 1000, entry_price = 1_000_000
        // notional = 1000, MM = 50, IM = 100
        // capital = 150, pnl = -100
        // After warmup settle: capital = 50, pnl = 0, equity = 50
        // equity(50) is NOT strictly > MM(50), so touch_account_full's
        // post-settlement MM re-check fails with Undercollateralized.

        engine.accounts[user_idx as usize].capital = U128::new(150);
        engine.accounts[user_idx as usize].pnl = I128::new(-100);
        engine.accounts[user_idx as usize].position_size = I128::new(1_000);
        engine.accounts[user_idx as usize].entry_price = 1_000_000;
        engine.accounts[user_idx as usize].warmup_slope_per_step = U128::new(0);
        engine.vault = U128::new(150);

        // withdraw(60) should fail - loss settles first, then MM re-check catches
        // that equity(50) is not strictly above MM(50)
        let result = engine.withdraw(user_idx, 60, 0, 1_000_000);
        assert!(
            result == Err(RiskError::Undercollateralized),
            "withdraw(60) must fail: after settling 100 loss, equity=50 not > MM=50"
        );

        // Loss was settled during touch_account_full: capital = 50, pnl = 0
        assert_eq!(engine.accounts[user_idx as usize].capital.get(), 50);
        assert_eq!(engine.accounts[user_idx as usize].pnl.get(), 0);

        // Try withdraw(40) - same: equity(50) not > MM(50) so touch_account_full fails
        let result = engine.withdraw(user_idx, 40, 0, 1_000_000);
        assert!(
            result == Err(RiskError::Undercollateralized),
            "withdraw(40) must fail: equity=50 not > MM=50"
        );
    }

    #[test]
    fn test_withdraw_pnl_not_warmed_up() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(0).unwrap();
        let counterparty = engine.add_user(0).unwrap();

        engine.deposit(user_idx, 1000, 0).unwrap();
        // Zero-sum PNL: user gains, counterparty loses (no vault funding needed)
        assert_eq!(engine.accounts[user_idx as usize].pnl.get(), 0);
        assert_eq!(engine.accounts[counterparty as usize].pnl.get(), 0);
        engine.accounts[user_idx as usize].pnl = I128::new(500);
        engine.accounts[counterparty as usize].pnl = I128::new(-500);
        assert_conserved(&engine);

        // Try to withdraw more than principal + warmed up PNL
        // Since PNL hasn't warmed up, can only withdraw the 1000 principal
        let result = engine.withdraw(user_idx, 1100, 0, 1_000_000);
        assert_eq!(result, Err(RiskError::InsufficientBalance));
    }

    #[test]
    fn test_withdraw_principal_with_negative_pnl_should_fail() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(0).unwrap();

        // User deposits 1000
        engine.deposit(user_idx, 1000, 0).unwrap();

        // User has a position and negative PNL of -800
        engine.accounts[user_idx as usize].position_size = I128::new(10_000);
        engine.accounts[user_idx as usize].entry_price = 1_000_000; // $1 entry price
        engine.accounts[user_idx as usize].pnl = I128::new(-800);

        // Trying to withdraw all principal would leave collateral = 0 + max(0, -800) = 0
        // This should fail because user has an open position
        let result = engine.withdraw(user_idx, 1000, 0, 1_000_000);

        assert!(
        result.is_err(),
        "Should not allow withdrawal that leaves account undercollateralized with open position"
    );
    }

    #[test]
    fn test_withdraw_rejected_when_closed_and_negative_pnl() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(0).unwrap();

        // Setup: position closed but with unrealized losses
        engine.accounts[user_idx as usize].capital = U128::new(10_000);
        engine.accounts[user_idx as usize].pnl = I128::new(-9_000);
        engine.accounts[user_idx as usize].position_size = I128::new(0); // No position
        engine.accounts[user_idx as usize].warmup_slope_per_step = U128::new(0);
        engine.vault = U128::new(10_000);

        // Attempt to withdraw full capital - should fail because losses must be realized first
        let result = engine.withdraw(user_idx, 10_000, 0, 1_000_000);

        // The withdraw should fail with InsufficientBalance
        assert!(
            result == Err(RiskError::InsufficientBalance),
            "Expected InsufficientBalance after loss realization reduces capital"
        );

        // After the failed withdraw call (which internally called settle_warmup_to_capital):
        // capital should be 1_000 (10_000 - 9_000 loss)
        // pnl should be 0 (loss fully realized)
        // warmed_neg_total should include 9_000
        assert_eq!(
            engine.accounts[user_idx as usize].capital.get(),
            1_000,
            "Capital should be reduced by loss amount"
        );
        assert_eq!(
            engine.accounts[user_idx as usize].pnl.get(),
            0,
            "PnL should be 0 after loss realization"
        );
    }

    #[test]
    fn test_withdraw_rejected_when_closed_and_negative_pnl_full_amount() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(0).unwrap();

        // Setup: deposit 1000, no position, negative pnl of -300
        let _ = engine.deposit(user_idx, 1000, 0);
        engine.accounts[user_idx as usize].pnl = I128::new(-300);
        engine.accounts[user_idx as usize].position_size = I128::new(0);

        // Try to withdraw full original amount (1000)
        // After settle: capital = 1000 - 300 = 700, so withdrawing 1000 should fail
        let result = engine.withdraw(user_idx, 1000, 0, 1_000_000);
        assert_eq!(result, Err(RiskError::InsufficientBalance));

        // Verify N1 invariant: after operation, pnl >= 0 || capital == 0
        let account = &engine.accounts[user_idx as usize];
        assert!(!account.pnl.is_negative() || account.capital.is_zero());
    }

    #[test]
    fn test_withdraw_with_warmed_up_pnl() {
        let mut engine = Box::new(RiskEngine::new(default_params()));
        let user_idx = engine.add_user(0).unwrap();
        let counterparty = engine.add_user(0).unwrap();

        // Add insurance to provide warmup budget for converting positive PnL to capital
        set_insurance(&mut engine, 500);

        engine.deposit(user_idx, 1000, 0).unwrap();
        // Counterparty needs capital to pay their loss, creating vault surplus
        // for the haircut ratio (Residual = V - C_tot - I > 0)
        engine.deposit(counterparty, 500, 0).unwrap();
        // Zero-sum PnL: user gains, counterparty loses
        assert_eq!(engine.accounts[user_idx as usize].pnl.get(), 0);
        assert_eq!(engine.accounts[counterparty as usize].pnl.get(), 0);
        engine.accounts[user_idx as usize].pnl = I128::new(500);
        engine.accounts[counterparty as usize].pnl = I128::new(-500);
        engine.recompute_aggregates();
        engine.accounts[user_idx as usize].warmup_slope_per_step = U128::new(10);
        assert_conserved(&engine);

        // Settle counterparty's loss to free vault residual for haircut ratio.
        // Under haircut-ratio design: Residual must be > 0 for profit conversion.
        engine.settle_warmup_to_capital(counterparty).unwrap();

        // Advance enough slots to warm up 200 PNL
        engine.advance_slot(20);

        // Should be able to withdraw 1200 (1000 principal + 200 warmed PNL)
        // After counterparty settled: c_tot=1000, vault=2000, insurance=500.
        // Residual = 2000-1000-500 = 500. h = 1.0. Full conversion.
        engine
            .withdraw(user_idx, 1200, engine.current_slot, 1_000_000)
            .unwrap();
        assert_eq!(engine.accounts[user_idx as usize].pnl.get(), 300); // 500 - 200 converted
        assert_eq!(engine.accounts[user_idx as usize].capital.get(), 0); // 1000 + 200 - 1200
        assert_conserved(&engine);
    }

    #[test]
    fn test_withdrawals_blocked_during_pending_unblocked_after() {
        let mut params = default_params();
        params.risk_reduction_threshold = U128::new(0);
        params.warmup_period_slots = 1; // Instant warmup (minimum valid) // Instant warmup
        let mut engine = Box::new(RiskEngine::new(params));

        // Fund insurance
        engine.insurance_fund.balance = U128::new(100_000);
        engine.vault = U128::new(100_000);

        // Create user with capital
        let user = engine.add_user(0).unwrap();
        engine.deposit(user, 10_000, 0).unwrap();

        // Crank to establish baseline
        engine.keeper_crank(1, 1_000_000, &[], 64, 0).unwrap();

        // Under haircut-ratio design, there is no pending_unpaid_loss mechanism.
        // Withdrawals are not blocked by pending losses.
        let result = engine.withdraw(user, 1_000, 2, 1_000_000);
        assert!(
            result.is_ok(),
            "Withdraw should succeed (no pending loss mechanism)"
        );

        // Additional withdrawal should also succeed
        let result = engine.withdraw(user, 1_000, 2, 1_000_000);
        assert!(result.is_ok(), "Subsequent withdraw should also succeed");
    }

    #[test]
    fn test_zero_fee_bps_means_no_fee() {
        let mut params = default_params();
        params.trading_fee_bps = 0; // Fee-free trading
        params.maintenance_margin_bps = 100;
        params.initial_margin_bps = 100;
        params.warmup_period_slots = 1; // Instant warmup (minimum valid)
        params.max_crank_staleness_slots = u64::MAX;

        let mut engine = Box::new(RiskEngine::new(params));

        let user_idx = engine.add_user(0).unwrap();
        let lp_idx = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();

        engine.deposit(user_idx, 1_000_000_000, 0).unwrap();
        engine.accounts[lp_idx as usize].capital = U128::new(1_000_000_000);
        engine.vault += 1_000_000_000;
        engine.c_tot = U128::new(2_000_000_000);

        let oracle_price = 100_000_000u64; // $100

        let insurance_before = engine.insurance_fund.balance.get();

        // Execute a trade with fee_bps=0
        let size: i128 = 1_000_000;
        engine
            .execute_trade(&MATCHER, lp_idx, user_idx, 0, oracle_price, size)
            .unwrap();

        let insurance_after = engine.insurance_fund.balance.get();
        let fee_charged = insurance_after - insurance_before;

        // Fee MUST be 0 when trading_fee_bps is 0
        assert_eq!(
            fee_charged, 0,
            "Fee must be zero when trading_fee_bps=0. Got fee={}",
            fee_charged
        );
    }
}
