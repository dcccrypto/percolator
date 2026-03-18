//! Kani proofs for two-phase barrier scan properties (spec §A2).

#![cfg(kani)]

mod common;
use common::*;

// ############################################################################
// BARRIER SCAN PROOFS
// ############################################################################

/// Proof 1: capture_barrier_snapshot returns exact engine state fields.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_barrier_snapshot_matches_engine() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = engine.add_user(0).unwrap();
    engine.deposit(idx, 1_000_000, DEFAULT_ORACLE, DEFAULT_SLOT).unwrap();

    let snap = engine.capture_barrier_snapshot(DEFAULT_SLOT, DEFAULT_ORACLE);

    assert!(snap.oracle_price_b == DEFAULT_ORACLE);
    assert!(snap.current_slot_b == DEFAULT_SLOT);
    assert!(snap.a_long_b == engine.adl_mult_long);
    assert!(snap.a_short_b == engine.adl_mult_short);
    assert!(snap.k_long_b == engine.adl_coeff_long);
    assert!(snap.k_short_b == engine.adl_coeff_short);
    assert!(snap.epoch_long_b == engine.adl_epoch_long);
    assert!(snap.epoch_short_b == engine.adl_epoch_short);
    assert!(snap.k_epoch_start_long_b == engine.adl_epoch_start_k_long);
    assert!(snap.k_epoch_start_short_b == engine.adl_epoch_start_k_short);
    assert!(snap.mode_long_b == engine.side_mode_long);
    assert!(snap.mode_short_b == engine.side_mode_short);
    assert!(snap.oi_eff_long_b == engine.oi_eff_long_q);
    assert!(snap.oi_eff_short_b == engine.oi_eff_short_q);
    assert!(snap.maintenance_margin_bps == engine.params.maintenance_margin_bps);
    assert!(snap.maintenance_fee_per_slot == engine.params.maintenance_fee_per_slot.get());
}

/// Proof 2: If full touch makes account liquidatable at barrier, preview never
/// returns Safe. Bounded symbolic inputs.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_preview_no_false_negative() {
    let mut engine = RiskEngine::new(zero_fee_params());

    let a = engine.add_user(0).unwrap();
    let b = engine.add_user(0).unwrap();

    let deposit_a: u16 = kani::any();
    kani::assume(deposit_a >= 100 && deposit_a <= 50_000);
    engine.deposit(a, deposit_a as u128, DEFAULT_ORACLE, DEFAULT_SLOT).unwrap();
    engine.deposit(b, 1_000_000, DEFAULT_ORACLE, DEFAULT_SLOT).unwrap();

    // Open a position: small bounded size
    let size_raw: u8 = kani::any();
    kani::assume(size_raw >= 1 && size_raw <= 10);
    let size_q = (size_raw as i128) * (POS_SCALE as i128);

    // Execute trade (may fail IM check — that's fine, we skip)
    if engine.execute_trade(a, b, DEFAULT_ORACLE, DEFAULT_SLOT, size_q, DEFAULT_ORACLE).is_err() {
        return;
    }

    // Symbolic oracle for barrier scan
    let oracle2: u16 = kani::any();
    kani::assume(oracle2 >= 1 && oracle2 <= 5000);
    let scan_slot = DEFAULT_SLOT + 1;

    if engine.accrue_market_to(scan_slot, oracle2 as u64).is_err() {
        return;
    }

    let barrier = engine.capture_barrier_snapshot(scan_slot, oracle2 as u64);
    let class_a = engine.preview_account_at_barrier(a as usize, &barrier);

    // Run full touch on a clone
    let mut verify = engine.clone();
    if verify.touch_account_full(a as usize, oracle2 as u64, scan_slot).is_err() {
        // Touch failed — preview must not return Safe for open position
        if verify.accounts[a as usize].position_basis_q != 0 {
            assert!(class_a != ReviewClass::Safe);
        }
        return;
    }

    let eff = verify.effective_pos_q(a as usize);
    if eff != 0 {
        let above_mm = verify.is_above_maintenance_margin(
            &verify.accounts[a as usize], a as usize, oracle2 as u64,
        );
        if !above_mm {
            // Account IS liquidatable → preview must NOT return Safe
            assert!(class_a != ReviewClass::Safe, "no-false-negative violated");
        }
    }
}

/// Proof 3: Epoch mismatch never returns Safe.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_preview_epoch_mismatch_not_safe() {
    let mut engine = RiskEngine::new(zero_fee_params());

    let a = engine.add_user(0).unwrap();
    engine.deposit(a, 1_000_000, DEFAULT_ORACLE, DEFAULT_SLOT).unwrap();

    // Manually set up an open position with epoch mismatch
    let basis: i8 = kani::any();
    kani::assume(basis != 0);
    engine.accounts[a as usize].position_basis_q = basis as i128;
    if basis > 0 {
        engine.stored_pos_count_long += 1;
    } else {
        engine.stored_pos_count_short += 1;
    }
    engine.accounts[a as usize].adl_a_basis = ADL_ONE;
    engine.accounts[a as usize].adl_epoch_snap = 0;

    // Set side epoch to 1 (mismatch with snap=0)
    engine.adl_epoch_long = 1;
    engine.adl_epoch_short = 1;

    // Test both ResetPending and non-ResetPending modes
    let mode_val: u8 = kani::any();
    kani::assume(mode_val <= 2);
    let mode = match mode_val {
        0 => SideMode::Normal,
        1 => SideMode::DrainOnly,
        _ => SideMode::ResetPending,
    };
    engine.side_mode_long = mode;
    engine.side_mode_short = mode;

    let barrier = engine.capture_barrier_snapshot(DEFAULT_SLOT, DEFAULT_ORACLE);
    let class = engine.preview_account_at_barrier(a as usize, &barrier);

    // Epoch mismatch must never be Safe
    assert!(class != ReviewClass::Safe, "epoch mismatch must never be Safe");
}

/// Proof 4: Preview fee UB is an upper bound on actual maintenance fee.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_preview_fee_ub_is_upper_bound() {
    let params = RiskParams {
        warmup_period_slots: 100,
        maintenance_margin_bps: 500,
        initial_margin_bps: 1000,
        trading_fee_bps: 0,
        max_accounts: MAX_ACCOUNTS as u64,
        new_account_fee: U128::ZERO,
        maintenance_fee_per_slot: U128::new(10),
        max_crank_staleness_slots: u64::MAX,
        liquidation_fee_bps: 0,
        liquidation_fee_cap: U128::ZERO,
        liquidation_buffer_bps: 50,
        min_liquidation_abs: U128::ZERO,
    };
    let mut engine = RiskEngine::new(params);
    let a = engine.add_user(0).unwrap();

    let deposit: u16 = kani::any();
    kani::assume(deposit >= 100);
    engine.deposit(a, deposit as u128, DEFAULT_ORACLE, DEFAULT_SLOT).unwrap();
    engine.accounts[a as usize].last_fee_slot = DEFAULT_SLOT;
    engine.last_oracle_price = DEFAULT_ORACLE;
    engine.last_market_slot = DEFAULT_SLOT;

    let dt: u8 = kani::any();
    kani::assume(dt >= 1 && dt <= 200);
    let check_slot = DEFAULT_SLOT + dt as u64;

    let barrier = engine.capture_barrier_snapshot(check_slot, DEFAULT_ORACLE);
    let fee_ub = engine.preview_account_local_fee_debt_ub(a as usize, &barrier);

    // fee_ub should not be None for reasonable dt
    if let Some(ub) = fee_ub {
        // After touch, get actual fee debt
        let mut verify = engine.clone();
        if verify.touch_account_full(a as usize, DEFAULT_ORACLE, check_slot).is_ok() {
            let actual_debt = fee_debt_u128_checked(verify.accounts[a as usize].fee_credits.get());
            assert!(ub >= actual_debt, "fee UB must be >= actual fee debt");
        }
    }
}

/// Proof 5: Flat account with negative PnL → ReviewCleanup.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_preview_flat_negative_cleanup() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let a = engine.add_user(0).unwrap();
    engine.deposit(a, 1_000_000, DEFAULT_ORACLE, DEFAULT_SLOT).unwrap();

    let pnl: i16 = kani::any();
    kani::assume(pnl < 0 && pnl > i16::MIN);
    engine.set_pnl(a as usize, pnl as i128);

    let barrier = engine.capture_barrier_snapshot(DEFAULT_SLOT, DEFAULT_ORACLE);
    let class = engine.preview_account_at_barrier(a as usize, &barrier);

    // Flat (basis == 0) with negative PnL → ReviewCleanup
    assert!(class == ReviewClass::ReviewCleanup);
}

/// Proof 6: OI balance after keeper_barrier_wave.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_barrier_wave_oi_balance() {
    let mut engine = RiskEngine::new(zero_fee_params());

    let a = engine.add_user(0).unwrap();
    let b = engine.add_user(0).unwrap();
    engine.deposit(a, 500_000, DEFAULT_ORACLE, DEFAULT_SLOT).unwrap();
    engine.deposit(b, 500_000, DEFAULT_ORACLE, DEFAULT_SLOT).unwrap();

    let size_raw: u8 = kani::any();
    kani::assume(size_raw >= 1 && size_raw <= 5);
    let size_q = (size_raw as i128) * (POS_SCALE as i128);

    if engine.execute_trade(a, b, DEFAULT_ORACLE, DEFAULT_SLOT, size_q, DEFAULT_ORACLE).is_err() {
        return;
    }

    let scan_window: [u16; 2] = [a, b];
    let slot2 = DEFAULT_SLOT + 1;

    let oracle2: u16 = kani::any();
    kani::assume(oracle2 >= 100 && oracle2 <= 5000);

    if engine.keeper_barrier_wave(a, slot2, oracle2 as u64, 0, &scan_window, 10).is_ok() {
        assert!(engine.oi_eff_long_q == engine.oi_eff_short_q, "OI must balance");
    }
}

/// Proof 7: Conservation after keeper_barrier_wave.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_barrier_wave_conservation() {
    let mut engine = RiskEngine::new(zero_fee_params());

    let a = engine.add_user(0).unwrap();
    let b = engine.add_user(0).unwrap();
    engine.deposit(a, 500_000, DEFAULT_ORACLE, DEFAULT_SLOT).unwrap();
    engine.deposit(b, 500_000, DEFAULT_ORACLE, DEFAULT_SLOT).unwrap();

    let size_raw: u8 = kani::any();
    kani::assume(size_raw >= 1 && size_raw <= 5);
    let size_q = (size_raw as i128) * (POS_SCALE as i128);

    if engine.execute_trade(a, b, DEFAULT_ORACLE, DEFAULT_SLOT, size_q, DEFAULT_ORACLE).is_err() {
        return;
    }

    assert!(engine.check_conservation(), "pre-wave conservation");

    let scan_window: [u16; 2] = [a, b];
    let slot2 = DEFAULT_SLOT + 1;

    if engine.keeper_barrier_wave(a, slot2, DEFAULT_ORACLE, 0, &scan_window, 10).is_ok() {
        assert!(engine.check_conservation(), "post-wave conservation");
    }
}
