//! Section 8 — External audit fix proofs
//!
//! Formal verification of fixes for confirmed external audit findings:
//! 1. attach_effective_position epoch_snap canonical zero (spec §2.4)
//! 2. add_user/add_lp materialized_account_count rollback on alloc_slot failure
//! 3. is_above_maintenance_margin / is_above_initial_margin eff==0 special case (spec §9.1)
//! 4. fee_debt_sweep checked_add (defensive, invariant-guaranteed safe)

#![cfg(kani)]

mod common;
use common::*;

// ############################################################################
// FIX 1: epoch_snap canonical zero on position zero-out (spec §2.4)
// ############################################################################

/// After attach_effective_position(idx, 0), epoch_snap MUST be 0 regardless
/// of prior position side. Spec §2.4: canonical zero-position defaults.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_epoch_snap_zero_on_position_zeroout() {
    let mut engine = RiskEngine::new(zero_fee_params());

    let idx = engine.add_user(0).unwrap() as usize;
    engine.deposit(idx as u16, 1_000_000, DEFAULT_ORACLE, DEFAULT_SLOT).unwrap();

    // Set up non-trivial ADL epoch state
    engine.adl_epoch_long = 5;
    engine.adl_epoch_short = 7;

    // Symbolic initial side: positive (long) or negative (short) basis
    let side_long: bool = kani::any();
    let basis: u32 = kani::any();
    kani::assume(basis >= 1 && basis <= 10 * POS_SCALE as u32);

    let signed_basis = if side_long { basis as i128 } else { -(basis as i128) };

    // Use set_position_basis_q to correctly track stored_pos_count.
    // Set epoch mismatch to skip the phantom dust U256 path
    // (irrelevant to the epoch_snap fix).
    engine.set_position_basis_q(idx, signed_basis);
    engine.accounts[idx].adl_a_basis = ADL_ONE;
    engine.accounts[idx].adl_k_snap = 0;
    // Epoch mismatch: snap=0 != epoch_long=5 / epoch_short=7
    engine.accounts[idx].adl_epoch_snap = 0;

    // Zero out the position
    engine.attach_effective_position(idx, 0);

    // Spec §2.4: all canonical zero-position defaults
    assert!(engine.accounts[idx].position_basis_q == 0, "basis must be zero");
    assert!(engine.accounts[idx].adl_a_basis == ADL_ONE, "a_basis must be ADL_ONE");
    assert!(engine.accounts[idx].adl_k_snap == 0, "k_snap must be zero");
    assert!(engine.accounts[idx].adl_epoch_snap == 0, "epoch_snap must be zero per §2.4");
}

/// Verify that attaching a nonzero position correctly picks up the
/// current side epoch (not zero).
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_epoch_snap_correct_on_nonzero_attach() {
    let mut engine = RiskEngine::new(zero_fee_params());

    let idx = engine.add_user(0).unwrap() as usize;
    engine.deposit(idx as u16, 1_000_000, DEFAULT_ORACLE, DEFAULT_SLOT).unwrap();

    engine.adl_epoch_long = 3;
    engine.adl_epoch_short = 9;

    let side_long: bool = kani::any();
    let basis: u32 = kani::any();
    kani::assume(basis >= 1 && basis <= 100 * POS_SCALE as u32);

    let new_eff = if side_long { basis as i128 } else { -(basis as i128) };

    engine.attach_effective_position(idx, new_eff);

    if side_long {
        assert!(engine.accounts[idx].adl_epoch_snap == engine.adl_epoch_long);
        assert!(engine.accounts[idx].adl_a_basis == engine.adl_mult_long);
        assert!(engine.accounts[idx].adl_k_snap == engine.adl_coeff_long);
    } else {
        assert!(engine.accounts[idx].adl_epoch_snap == engine.adl_epoch_short);
        assert!(engine.accounts[idx].adl_a_basis == engine.adl_mult_short);
        assert!(engine.accounts[idx].adl_k_snap == engine.adl_coeff_short);
    }
}

// ############################################################################
// FIX 2: materialized_account_count rollback on alloc_slot failure
// ############################################################################

/// If alloc_slot fails in add_user, materialized_account_count must be
/// rolled back to its pre-call value.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_add_user_count_rollback_on_alloc_failure() {
    let mut engine = RiskEngine::new(zero_fee_params());

    // Fill all slots so alloc_slot will fail
    for i in 0..MAX_ACCOUNTS {
        engine.accounts[i].account_id = 1; // mark as used
    }
    engine.num_used_accounts = MAX_ACCOUNTS as u16;
    engine.materialized_account_count = 0; // but count is low (simulating inconsistency path)

    let count_before = engine.materialized_account_count;

    let result = engine.add_user(0);
    assert!(result.is_err(), "add_user must fail when all slots are full");
    assert!(
        engine.materialized_account_count == count_before,
        "materialized_account_count must be rolled back on failure"
    );
}

/// If alloc_slot fails in add_lp, materialized_account_count must be
/// rolled back to its pre-call value.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_add_lp_count_rollback_on_alloc_failure() {
    let mut engine = RiskEngine::new(zero_fee_params());

    // Fill all slots so alloc_slot will fail
    for i in 0..MAX_ACCOUNTS {
        engine.accounts[i].account_id = 1;
    }
    engine.num_used_accounts = MAX_ACCOUNTS as u16;
    engine.materialized_account_count = 0;

    let count_before = engine.materialized_account_count;

    let result = engine.add_lp([0; 32], [0; 32], 0);
    assert!(result.is_err(), "add_lp must fail when all slots are full");
    assert!(
        engine.materialized_account_count == count_before,
        "materialized_account_count must be rolled back on failure"
    );
}

// ############################################################################
// FIX 3: margin requirement is zero when effective position is zero (§9.1)
// ############################################################################

/// A flat account (eff==0) with any nonnegative equity must be maintenance-healthy.
/// Before the fix, min_nonzero_mm_req created a false requirement for flat accounts.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_flat_account_maintenance_healthy() {
    let mut engine = RiskEngine::new(zero_fee_params());

    let idx = engine.add_user(0).unwrap();
    let capital: u32 = kani::any();
    kani::assume(capital >= 1 && capital <= 10_000_000);

    engine.deposit(idx, capital as u128, DEFAULT_ORACLE, DEFAULT_SLOT).unwrap();

    // Account is flat (no position)
    assert!(engine.effective_pos_q(idx as usize) == 0);

    // With any positive capital and no position, account MUST be maintenance-healthy
    // Spec §9.1: MM_req = 0 when eff == 0
    let healthy = engine.is_above_maintenance_margin(
        &engine.accounts[idx as usize].clone(),
        idx as usize,
        DEFAULT_ORACLE,
    );
    assert!(healthy, "flat account with positive capital must be maintenance-healthy");
}

/// A flat account (eff==0) with any nonnegative equity must be initial-margin healthy.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_flat_account_initial_margin_healthy() {
    let mut engine = RiskEngine::new(zero_fee_params());

    let idx = engine.add_user(0).unwrap();
    let capital: u32 = kani::any();
    kani::assume(capital >= 1 && capital <= 10_000_000);

    engine.deposit(idx, capital as u128, DEFAULT_ORACLE, DEFAULT_SLOT).unwrap();

    assert!(engine.effective_pos_q(idx as usize) == 0);

    let healthy = engine.is_above_initial_margin(
        &engine.accounts[idx as usize].clone(),
        idx as usize,
        DEFAULT_ORACLE,
    );
    assert!(healthy, "flat account with positive capital must be initial-margin healthy");
}

/// A flat account with zero equity must NOT be maintenance-healthy.
/// Spec §9.1: Eq_net > 0 (since MM_req = 0 for flat), so Eq_net = 0 fails.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_flat_zero_equity_not_maintenance_healthy() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = engine.add_user(0).unwrap();
    // No deposit, capital = 0, pnl = 0 → equity = 0

    assert!(engine.effective_pos_q(idx as usize) == 0);

    let healthy = engine.is_above_maintenance_margin(
        &engine.accounts[idx as usize].clone(),
        idx as usize,
        DEFAULT_ORACLE,
    );
    // Eq_net = 0, MM_req = 0, 0 > 0 is false → not healthy
    assert!(!healthy, "flat account with zero equity is NOT maintenance-healthy");
}

// ############################################################################
// FIX 4: fee_debt_sweep uses checked_add (invariant: pay <= |fee_credits|)
// ############################################################################

/// fee_debt_sweep: after sweep, fee_credits is closer to zero and
/// insurance fund increases by exactly pay. Symbolic capital and debt.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_fee_debt_sweep_checked_arithmetic() {
    let mut engine = RiskEngine::new(zero_fee_params());

    let idx = engine.add_user(0).unwrap() as usize;
    let capital: u32 = kani::any();
    let debt: u32 = kani::any();
    kani::assume(capital >= 1 && capital <= 10_000_000);
    kani::assume(debt >= 1 && debt <= 10_000_000);

    // Set up capital
    engine.deposit(idx as u16, capital as u128, DEFAULT_ORACLE, DEFAULT_SLOT).unwrap();

    // Set fee debt (negative fee_credits)
    engine.accounts[idx].fee_credits = I128::new(-(debt as i128));

    let cap_before = engine.accounts[idx].capital.get();
    let fc_before = engine.accounts[idx].fee_credits.get();
    let ins_before = engine.insurance_fund.balance.get();

    engine.fee_debt_sweep(idx);

    let cap_after = engine.accounts[idx].capital.get();
    let fc_after = engine.accounts[idx].fee_credits.get();
    let ins_after = engine.insurance_fund.balance.get();

    let pay = core::cmp::min(debt as u128, capital as u128);

    // Capital decreases by pay
    assert!(cap_after == cap_before - pay);
    // fee_credits increases by pay (moves toward zero)
    assert!(fc_after == fc_before + pay as i128);
    // Insurance increases by pay
    assert!(ins_after == ins_before + pay);
    // fee_credits is still <= 0
    assert!(fc_after <= 0);
    // Conservation: total capital moved from account to insurance
    assert!(engine.check_conservation());
}

// ############################################################################
// FIX 5: keeper_crank always uses FullClose (no partial hint griefing)
// ############################################################################

/// keeper_crank with a syntactically valid ExactPartial hint must not revert.
/// The hint is ignored — keeper_crank always uses FullClose internally.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_keeper_crank_ignores_partial_hint() {
    let mut engine = RiskEngine::new(default_params());

    // Create two accounts for a trade
    let a = engine.add_user(1000).unwrap();
    let b = engine.add_user(1000).unwrap();

    engine.deposit(a, 50_000, DEFAULT_ORACLE, DEFAULT_SLOT).unwrap();
    engine.deposit(b, 50_000, DEFAULT_ORACLE, DEFAULT_SLOT).unwrap();

    // Open a position: a long, b short
    let size = 100 * POS_SCALE as i128;
    engine.execute_trade(a, b, DEFAULT_ORACLE, DEFAULT_SLOT, size, DEFAULT_ORACLE).unwrap();

    // Crash oracle to make 'a' liquidatable
    let crash_oracle = 500u64;

    // Submit a syntactically valid but economically invalid partial hint
    // (tiny close that won't restore health)
    let bad_hint = Some(LiquidationPolicy::ExactPartial(POS_SCALE as u128));

    // keeper_crank must NOT revert — it ignores the partial hint and uses FullClose
    let candidates = [(a, bad_hint)];
    let result = engine.keeper_crank(DEFAULT_SLOT + 1, crash_oracle, &candidates, 10);
    assert!(result.is_ok(), "keeper_crank must not revert on bad partial hint");
}

// ############################################################################
// FIX 6: liquidate_at_oracle rejects missing accounts before touch
// ############################################################################

/// liquidate_at_oracle on a missing account must return Ok(false) without
/// mutating market state (no accrue_market_to side effects).
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_liquidate_missing_account_no_market_mutation() {
    let mut engine = RiskEngine::new(zero_fee_params());

    let slot_before = engine.current_slot;
    let oracle_before = engine.last_oracle_price;

    // Call liquidate on an unused slot
    let result = engine.liquidate_at_oracle(0, DEFAULT_SLOT, DEFAULT_ORACLE, LiquidationPolicy::FullClose);
    assert!(matches!(result, Ok(false)), "must return Ok(false) for missing account");

    // Market state must not have been mutated
    assert!(engine.current_slot == slot_before, "current_slot must not change");
    assert!(engine.last_oracle_price == oracle_before, "last_oracle_price must not change");
}

// ############################################################################
// FIX 7: config validation — max_accounts <= MAX_ACCOUNTS
// ############################################################################

/// new() with max_accounts > MAX_ACCOUNTS must panic.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
#[kani::should_panic]
fn proof_config_rejects_oversized_max_accounts() {
    let mut params = zero_fee_params();
    params.max_accounts = (MAX_ACCOUNTS as u64) + 1;
    let _engine = RiskEngine::new(params);
}

/// new() with max_accounts == 0 must panic.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
#[kani::should_panic]
fn proof_config_rejects_zero_max_accounts() {
    let mut params = zero_fee_params();
    params.max_accounts = 0;
    let _engine = RiskEngine::new(params);
}

/// new() with BPS > 10_000 must panic.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
#[kani::should_panic]
fn proof_config_rejects_invalid_bps() {
    let mut params = zero_fee_params();
    params.initial_margin_bps = 10_001;
    let _engine = RiskEngine::new(params);
}
