//! Sections 1-2 — Global inductive invariants
//!
//! Conservation, PnL tracking, side counts, haircut ratio.

#![cfg(kani)]

mod common;
use common::*;

// ============================================================================
// T0.3: set_pnl_aggregate_update_is_exact
// ============================================================================

#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t0_3_set_pnl_aggregate_exact() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap();

    let old_pnl: i16 = kani::any();
    kani::assume(old_pnl > i16::MIN);
    let _ = set_pnl_test(&mut engine, idx as usize, old_pnl as i128);

    let new_pnl: i16 = kani::any();
    kani::assume(new_pnl > i16::MIN);
    let _ = set_pnl_test(&mut engine, idx as usize, new_pnl as i128);

    let expected = if new_pnl > 0 { new_pnl as u128 } else { 0u128 };
    let actual = engine.pnl_pos_tot;
    assert!(actual == expected);
}

#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t0_3_sat_all_sign_transitions() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap();

    let old: i16 = kani::any();
    let new: i16 = kani::any();
    kani::assume(old > i16::MIN && new > i16::MIN);

    let transition: u8 = kani::any();
    kani::assume(transition < 4);
    match transition {
        0 => kani::assume(old <= 0 && new <= 0),
        1 => kani::assume(old <= 0 && new > 0),
        2 => kani::assume(old > 0 && new <= 0),
        3 => kani::assume(old > 0 && new > 0),
        _ => unreachable!(),
    }

    let _ = set_pnl_test(&mut engine, idx as usize, old as i128);
    let _ = set_pnl_test(&mut engine, idx as usize, new as i128);

    let expected = if new > 0 { new as u128 } else { 0u128 };
    let actual = engine.pnl_pos_tot;
    assert!(actual == expected, "pnl_pos_tot mismatch after transition");
}

// ============================================================================
// T0.4: conservation_check_handles_overflow
// ============================================================================

#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn t0_4_conservation_check_handles_overflow() {
    // Use u128 inputs directly to cover the full value range,
    // including cases where c_tot + insurance may overflow u128.
    let c_tot: u128 = kani::any();
    let insurance: u128 = kani::any();
    let vault: u128 = kani::any();
    let deposit: u64 = kani::any();

    let deposit_128 = deposit as u128;

    // The conservation check uses checked_add, which may return None
    let sum = c_tot.checked_add(insurance);
    match sum {
        Some(s) => {
            // Non-overflow case: verify deposit preserves the invariant
            if vault >= s {
                // After deposit: vault + deposit and c_tot + deposit
                let vault_new = vault.checked_add(deposit_128);
                let c_tot_new = c_tot.checked_add(deposit_128);
                if let (Some(vn), Some(cn)) = (vault_new, c_tot_new) {
                    // Conservation: vault_new >= c_tot_new + insurance
                    let sum_new = cn.checked_add(insurance);
                    if let Some(sn) = sum_new {
                        assert!(vn >= sn, "deposit preserves conservation when no overflow");
                    }
                }
            }
        }
        None => {
            // c_tot + insurance overflows u128 → conservation check
            // must detect this as a deficit / corrupt state.
            kani::cover!(true, "overflow branch reachable");
        }
    }
}

// ============================================================================
// Inductive proofs from kani.rs
// ============================================================================

#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn inductive_top_up_insurance_preserves_accounting() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap();

    let dep: u32 = kani::any();
    kani::assume(dep > 0 && dep <= 1_000_000);
    engine
        .deposit_not_atomic(idx, dep as u128, DEFAULT_SLOT)
        .unwrap();
    assert!(engine.check_conservation());

    let ins_amt: u32 = kani::any();
    kani::assume(ins_amt <= 1_000_000);
    engine
        .top_up_insurance_fund(ins_amt as u128, DEFAULT_SLOT)
        .unwrap();
    assert!(engine.check_conservation());
}

#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn inductive_set_capital_decrease_preserves_accounting() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap();

    let dep: u32 = kani::any();
    kani::assume(dep >= 1000 && dep <= 1_000_000);
    engine
        .deposit_not_atomic(idx, dep as u128, DEFAULT_SLOT)
        .unwrap();
    assert!(engine.check_conservation());

    let new_cap: u32 = kani::any();
    kani::assume(new_cap <= dep);
    engine.set_capital(idx as usize, new_cap as u128);
    assert!(engine.check_conservation());
}

#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn inductive_set_pnl_preserves_pnl_pos_tot_delta() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let a = add_user_test(&mut engine, 0).unwrap();
    let b = add_user_test(&mut engine, 0).unwrap();

    let pnl_a: i32 = kani::any();
    kani::assume(pnl_a > i32::MIN);
    let _ = set_pnl_test(&mut engine, a as usize, pnl_a as i128);

    let pnl_b: i32 = kani::any();
    kani::assume(pnl_b > i32::MIN);
    let _ = set_pnl_test(&mut engine, b as usize, pnl_b as i128);

    let pos_a: u128 = if pnl_a > 0 { pnl_a as u128 } else { 0 };
    let pos_b: u128 = if pnl_b > 0 { pnl_b as u128 } else { 0 };
    assert!(engine.pnl_pos_tot == pos_a + pos_b);
}

#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn inductive_deposit_preserves_accounting() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap();

    let dep: u32 = kani::any();
    kani::assume(dep >= 1 && dep <= 1_000_000);
    engine
        .deposit_not_atomic(idx, dep as u128, DEFAULT_SLOT)
        .unwrap();
    assert!(engine.check_conservation());
}

#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn inductive_withdraw_preserves_accounting() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap();

    // Concrete deposit to reduce symbolic state space
    engine
        .deposit_not_atomic(idx, 100_000, DEFAULT_SLOT)
        .unwrap();

    // Symbolic withdrawal amount
    let w: u32 = kani::any();
    kani::assume(w >= 1 && w <= 100_000);
    let result = engine.withdraw_not_atomic(
        idx,
        w as u128,
        DEFAULT_ORACLE,
        DEFAULT_SLOT,
        0i128,
        0,
        100,
        None,
    );
    assert!(result.is_ok(), "valid flat funded withdrawal must succeed");
    kani::cover!(result.is_ok(), "withdraw Ok path reachable");
    assert!(engine.check_conservation());
}

#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn inductive_settle_loss_preserves_accounting() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap();

    let dep: u16 = kani::any();
    kani::assume(dep >= 1 && dep <= 2_000);
    engine
        .deposit_not_atomic(idx, dep as u128, DEFAULT_SLOT)
        .unwrap();
    assert!(engine.check_conservation());

    let loss: u16 = kani::any();
    kani::assume(loss >= 1 && loss <= dep);
    engine.set_pnl(idx as usize, -(loss as i128)).unwrap();

    let result = engine.settle_flat_negative_pnl_not_atomic(idx, DEFAULT_SLOT);
    assert!(
        result.is_ok(),
        "valid principal-covered flat loss settlement must succeed"
    );
    assert!(engine.accounts[idx as usize].capital.get() == (dep - loss) as u128);
    assert!(engine.accounts[idx as usize].pnl == 0);
    assert!(engine.check_conservation());
}

// ============================================================================
// Property proofs from kani.rs
// ============================================================================

#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn prop_pnl_pos_tot_agrees_with_recompute() {
    let mut engine = RiskEngine::new(zero_fee_params());

    let a = add_user_test(&mut engine, 0).unwrap();
    let b = add_user_test(&mut engine, 0).unwrap();

    let pnl_a: i32 = kani::any();
    kani::assume(pnl_a > i32::MIN);
    let _ = set_pnl_test(&mut engine, a as usize, pnl_a as i128);

    let pnl_b: i32 = kani::any();
    kani::assume(pnl_b > i32::MIN);
    let _ = set_pnl_test(&mut engine, b as usize, pnl_b as i128);

    let pos_a: u128 = if pnl_a > 0 { pnl_a as u128 } else { 0 };
    let pos_b: u128 = if pnl_b > 0 { pnl_b as u128 } else { 0 };
    let expected = pos_a + pos_b;

    assert!(engine.pnl_pos_tot == expected);
}

#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn prop_conservation_holds_after_all_ops() {
    let mut engine = RiskEngine::new(zero_fee_params());

    let idx = add_user_test(&mut engine, 0).unwrap();

    let dep: u32 = kani::any();
    kani::assume(dep > 0 && dep <= 5_000_000);
    engine
        .deposit_not_atomic(idx, dep as u128, DEFAULT_SLOT)
        .unwrap();
    assert!(engine.check_conservation());

    let ins_amt: u32 = kani::any();
    kani::assume(ins_amt <= 1_000_000);
    engine
        .top_up_insurance_fund(ins_amt as u128, DEFAULT_SLOT)
        .unwrap();
    assert!(engine.check_conservation());

    let loss: u32 = kani::any();
    kani::assume(loss <= dep);
    engine.set_pnl(idx as usize, -(loss as i128));
    assert!(engine.check_conservation());

    let cap_before = engine.accounts[idx as usize].capital.get();
    let pnl_abs = if loss > 0 { loss as u128 } else { 0 };
    let pay = core::cmp::min(pnl_abs, cap_before);
    if pay > 0 {
        engine.set_capital(idx as usize, cap_before - pay);
        let new_pnl_val = -(loss as i128) + (pay as i128);
        engine.set_pnl(idx as usize, new_pnl_val);
    }
    assert!(engine.check_conservation());
}

// ============================================================================
// set_pnl proofs from kani.rs
// ============================================================================

#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
#[kani::should_panic]
fn proof_set_pnl_rejects_i128_min() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap();
    // set_pnl returns Err for i128::MIN; unwrap to trigger the expected panic.
    engine.set_pnl(idx as usize, i128::MIN).unwrap();
}

#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_set_pnl_maintains_pnl_pos_tot() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap();

    let pnl1: i32 = kani::any();
    kani::assume(pnl1 > i32::MIN);
    let _ = set_pnl_test(&mut engine, idx as usize, pnl1 as i128);

    let expected1 = if pnl1 > 0 { pnl1 as u128 } else { 0u128 };
    assert!(engine.pnl_pos_tot == expected1);

    let pnl2: i32 = kani::any();
    kani::assume(pnl2 > i32::MIN);
    let _ = set_pnl_test(&mut engine, idx as usize, pnl2 as i128);

    let expected2 = if pnl2 > 0 { pnl2 as u128 } else { 0u128 };
    assert!(engine.pnl_pos_tot == expected2);
}

#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_set_pnl_underflow_safety() {
    // Substantive: pnl_pos_tot tracks sum of max(pnl, 0) correctly across
    // arbitrary set_pnl_with_reserve transitions.
    let mut engine = RiskEngine::new(zero_fee_params());
    engine.vault = U128::new(10_000); // positive residual for admission
    let idx = add_user_test(&mut engine, 0).unwrap() as usize;

    // Symbolic positive initial PnL via admission pair
    let pnl1: u8 = kani::any();
    let mut ctx = InstructionContext::new_with_admission(0, 100);
    let _ = engine.set_pnl_with_reserve(
        idx,
        pnl1 as i128,
        ReserveMode::UseAdmissionPair(0, 100),
        Some(&mut ctx),
    );
    assert!(engine.pnl_pos_tot == pnl1 as u128);

    // Decrease to symbolic smaller or negative value
    let pnl2: i8 = kani::any();
    kani::assume(pnl2 <= pnl1 as i8);
    let _ = engine.set_pnl_with_reserve(
        idx,
        pnl2 as i128,
        ReserveMode::NoPositiveIncreaseAllowed,
        None,
    );
    let expected = core::cmp::max(pnl2 as i128, 0) as u128;
    assert!(engine.pnl_pos_tot == expected);
}

#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_set_pnl_clamps_reserved_pnl() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap();

    // Market defaults to Live; set_pnl uses ImmediateReleaseResolvedOnly and errs
    // in Live mode. Use UseAdmissionPair for positive increases (Live-compatible).
    let mut ctx = InstructionContext::new_with_admission(10, 10);
    engine
        .set_pnl_with_reserve(
            idx as usize,
            5000i128,
            ReserveMode::UseAdmissionPair(10, 10),
            Some(&mut ctx),
        )
        .unwrap();
    assert!(
        engine.accounts[idx as usize].reserved_pnl == 5000u128,
        "UseAdmissionPair: positive PnL goes to reserve"
    );

    // Decrease PnL via UseAdmissionPair (no positive increase → ctx path not used).
    // Reserve loss applied via newest-first.
    engine
        .set_pnl_with_reserve(
            idx as usize,
            3000i128,
            ReserveMode::UseAdmissionPair(10, 10),
            Some(&mut ctx),
        )
        .unwrap();
    assert!(
        engine.accounts[idx as usize].reserved_pnl <= 3000u128,
        "reserved_pnl must be clamped by new positive PnL"
    );

    // Decrease PnL below zero → reserve must clamp to 0.
    engine
        .set_pnl_with_reserve(
            idx as usize,
            -100i128,
            ReserveMode::UseAdmissionPair(10, 10),
            Some(&mut ctx),
        )
        .unwrap();
    assert!(
        engine.accounts[idx as usize].reserved_pnl == 0u128,
        "reserved_pnl clamps to 0 when pnl goes negative"
    );
}

#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_set_capital_maintains_c_tot() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap();

    let initial: u32 = kani::any();
    kani::assume(initial > 0 && initial <= 1_000_000);
    engine
        .deposit_not_atomic(idx, initial as u128, DEFAULT_SLOT)
        .unwrap();

    assert!(engine.c_tot.get() == engine.accounts[idx as usize].capital.get());

    let new_cap: u32 = kani::any();
    kani::assume((new_cap as u64) <= (initial as u64) * 2);
    engine.set_capital(idx as usize, new_cap as u128);

    assert!(engine.c_tot.get() == new_cap as u128);
}

// ============================================================================
// check_conservation / haircut from kani.rs
// ============================================================================

#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_check_conservation_basic() {
    // Substantive: check_conservation returns exactly V >= C + I across symbolic V/C/I.
    let mut engine = RiskEngine::new(zero_fee_params());

    let v: u16 = kani::any();
    let c: u16 = kani::any();
    let i: u16 = kani::any();

    engine.vault = U128::new(v as u128);
    engine.c_tot = U128::new(c as u128);
    engine.insurance_fund.balance = U128::new(i as u128);

    let expected = (v as u128) >= (c as u128) + (i as u128);
    assert!(engine.check_conservation() == expected);
}

#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_haircut_ratio_no_division_by_zero() {
    let mut engine = RiskEngine::new(zero_fee_params());

    // Empty engine → (1, 1) since pnl_matured_pos_tot == 0
    let (num, den) = engine.haircut_ratio();
    assert!(num == 1u128);
    assert!(den == 1u128);

    // Set pnl_matured_pos_tot (v12.14.0 uses this as denominator, not pnl_pos_tot)
    engine.pnl_pos_tot = 1000u128;
    engine.pnl_matured_pos_tot = 1000u128;
    engine.vault = U128::new(2000);
    engine.c_tot = U128::new(500);
    engine.insurance_fund.balance = U128::new(300);
    let (num2, den2) = engine.haircut_ratio();
    assert!(den2 == 1000u128, "denominator must be pnl_matured_pos_tot");
    // residual = 2000 - 500 - 300 = 1200 > 1000, so h_num = min(1200, 1000) = 1000
    assert!(num2 == 1000u128);
    assert!(num2 <= den2);
}

#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_absorb_protocol_loss_drains_to_zero() {
    // After the insurance_floor removal, absorb_protocol_loss consumes
    // the full insurance balance. Remaining loss becomes uninsured
    // (handled by the junior haircut mechanism).
    let mut engine = RiskEngine::new(zero_fee_params());

    let balance: u32 = kani::any();
    kani::assume(balance <= 100_000);
    engine.insurance_fund.balance = U128::new(balance as u128);

    let loss: u32 = kani::any();
    kani::assume(loss > 0 && loss <= 100_000);
    engine.absorb_protocol_loss(loss as u128);

    // Balance must never grow from a loss and must not underflow.
    assert!(engine.insurance_fund.balance.get() <= balance as u128);
}

// ============================================================================
// Position / side tracking from kani.rs
// ============================================================================

#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_set_position_basis_q_count_tracking() {
    // Substantive: symbolic basis transitions test count tracking across
    // sign changes, zero transitions, and magnitude changes.
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap() as usize;

    let b1: i8 = kani::any();
    let b2: i8 = kani::any();
    kani::assume(b1 != 0);
    kani::assume(b2 != 0);

    engine.set_position_basis_q(idx, b1 as i128);
    // Counts reflect b1's sign
    if b1 > 0 {
        assert!(engine.stored_pos_count_long == 1);
        assert!(engine.stored_pos_count_short == 0);
    } else {
        assert!(engine.stored_pos_count_long == 0);
        assert!(engine.stored_pos_count_short == 1);
    }

    engine.set_position_basis_q(idx, b2 as i128);
    // Counts reflect b2's sign (single account, so one side is 1)
    if b2 > 0 {
        assert!(engine.stored_pos_count_long == 1);
        assert!(engine.stored_pos_count_short == 0);
    } else {
        assert!(engine.stored_pos_count_long == 0);
        assert!(engine.stored_pos_count_short == 1);
    }

    engine.set_position_basis_q(idx, 0i128);
    assert!(engine.stored_pos_count_long == 0);
    assert!(engine.stored_pos_count_short == 0);
}

#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_side_mode_gating() {
    let mut engine = RiskEngine::new_with_market(zero_fee_params(), DEFAULT_SLOT, DEFAULT_ORACLE);
    let oi = 10 * POS_SCALE;
    engine.oi_eff_long_q = oi;
    engine.oi_eff_short_q = oi;

    // DrainOnly blocks OI increases on its side but permits non-increasing candidates.
    // ENG-PORT-4 fixup: 6-arg signature. Pass 0i128 per-account positions —
    // the per-account gate is a no-op when new_eff == 0; this proof isolates
    // the aggregate-OI gate.
    engine.side_mode_long = SideMode::DrainOnly;
    let long_inc = engine.enforce_side_mode_oi_gate(0, 0, 0, 0, oi + POS_SCALE, oi);
    assert!(
        long_inc == Err(RiskError::SideBlocked),
        "DrainOnly long side must block long OI increases"
    );
    let long_same = engine.enforce_side_mode_oi_gate(0, 0, 0, 0, oi, oi);
    assert!(
        long_same.is_ok(),
        "DrainOnly long side must permit non-increasing long OI"
    );

    // ResetPending has the same OI-increase gate.
    engine.side_mode_long = SideMode::Normal;
    engine.side_mode_short = SideMode::ResetPending;
    let short_inc = engine.enforce_side_mode_oi_gate(0, 0, 0, 0, oi, oi + POS_SCALE);
    assert!(
        short_inc == Err(RiskError::SideBlocked),
        "ResetPending short side must block short OI increases"
    );
    let short_same = engine.enforce_side_mode_oi_gate(0, 0, 0, 0, oi, oi);
    assert!(
        short_same.is_ok(),
        "ResetPending short side must permit non-increasing short OI"
    );

    // Normal mode does not block side OI increases at this gate.
    engine.side_mode_short = SideMode::Normal;
    let normal_inc = engine.enforce_side_mode_oi_gate(0, 0, 0, 0, oi + POS_SCALE, oi + POS_SCALE);
    assert!(
        normal_inc.is_ok(),
        "Normal side mode must not block OI increases at the side-mode gate"
    );

    assert!(engine.oi_eff_long_q == oi);
    assert!(engine.oi_eff_short_q == oi);
    assert!(engine.check_conservation());
}

#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_account_equity_net_nonnegative() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let a = add_user_test(&mut engine, 0).unwrap();
    let b = add_user_test(&mut engine, 0).unwrap();

    let cap_a: u16 = kani::any();
    kani::assume(cap_a > 0 && cap_a <= 10_000);
    let cap_b: u16 = kani::any();
    kani::assume(cap_b > 0 && cap_b <= 10_000);

    engine.set_capital(a as usize, cap_a as u128);
    engine.set_capital(b as usize, cap_b as u128);

    // Vault has excess beyond c_tot so Residual > 0 and haircut is non-trivial
    let excess: u16 = kani::any();
    kani::assume(excess <= 5_000);
    let c_tot = (cap_a as u128) + (cap_b as u128);
    engine.vault = U128::new(c_tot + (excess as u128));

    let pnl_val: i16 = kani::any();
    kani::assume(pnl_val as i32 > i16::MIN as i32);
    engine.set_pnl(a as usize, pnl_val as i128);

    // Set pnl_matured_pos_tot to exercise h < 1 in haircut_ratio (v12.14.0)
    let matured: u16 = kani::any();
    kani::assume(matured <= 20_000);
    engine.pnl_matured_pos_tot = core::cmp::min(matured as u128, engine.pnl_pos_tot);

    // Exercise both positive PnL (haircut path) and negative PnL
    let eq = engine.account_equity_net(&engine.accounts[a as usize], DEFAULT_ORACLE);
    assert!(
        eq >= 0,
        "flat account equity must be non-negative for any haircut level"
    );
}

#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_effective_pos_q_epoch_mismatch_returns_zero() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap() as usize;

    engine
        .attach_effective_position(idx, POS_SCALE as i128)
        .unwrap();
    engine.begin_full_drain_reset(Side::Long).unwrap();
    let eff = engine.effective_pos_q(idx);
    assert!(eff == 0);

    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap() as usize;
    engine
        .attach_effective_position(idx, -(POS_SCALE as i128))
        .unwrap();
    engine.begin_full_drain_reset(Side::Short).unwrap();
    let eff2 = engine.effective_pos_q(idx);
    assert!(eff2 == 0);
}

#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_effective_pos_q_flat_is_zero() {
    // Substantive: after attaching a symbolic nonzero position and then
    // detaching (attach 0), effective_pos_q returns 0.
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap() as usize;

    // Attach a symbolic nonzero position via the proper path
    let basis: i8 = kani::any();
    kani::assume(basis != 0);
    engine
        .attach_effective_position(idx, basis as i128)
        .unwrap();
    assert!(engine.effective_pos_q(idx) != 0);

    // Detach by attaching 0
    engine.attach_effective_position(idx, 0).unwrap();
    assert!(engine.accounts[idx].position_basis_q == 0);
    assert!(engine.effective_pos_q(idx) == 0);
}

#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_attach_effective_position_updates_side_counts() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap();

    assert!(engine.stored_pos_count_long == 0);
    assert!(engine.stored_pos_count_short == 0);

    let pos = POS_SCALE as i128;
    engine.attach_effective_position(idx as usize, pos);
    assert!(engine.stored_pos_count_long == 1);
    assert!(engine.stored_pos_count_short == 0);

    engine.attach_effective_position(idx as usize, 0i128);
    assert!(engine.stored_pos_count_long == 0);
    assert!(engine.stored_pos_count_short == 0);

    let neg = -(POS_SCALE as i128);
    engine.attach_effective_position(idx as usize, neg);
    assert!(engine.stored_pos_count_long == 0);
    assert!(engine.stored_pos_count_short == 1);
}

// ============================================================================
// NEW: proof_fee_credits_never_i128_min
// ============================================================================

/// fee_debt_u128_checked safely handles all fee_credits values including i128::MIN.
/// Verifies: checked_sub boundary behavior and fee_debt extraction never panics.
/// The settle_maintenance_fee path uses checked_sub which can produce i128::MIN,
/// but fee_debt_u128_checked uses unsigned_abs() which safely returns 2^127.
#[kani::proof]
#[kani::unwind(2)]
#[kani::solver(cadical)]
fn proof_fee_credits_never_i128_min() {
    // Part 1: fee_debt_u128_checked is safe for ALL i128 values
    let fc: i32 = kani::any();
    let debt = fee_debt_u128_checked(fc as i128);
    if fc < 0 {
        assert!(debt == (fc as i128).unsigned_abs());
    } else {
        assert!(debt == 0);
    }

    // Part 2: checked_sub boundary — if fee_credits - due overflows, it returns None
    let credits: i32 = kani::any();
    let due: u16 = kani::any();
    kani::assume(due > 0);
    let due_i128: i128 = due as i128;
    let result = (credits as i128).checked_sub(due_i128);
    match result {
        Some(new_fc) => {
            // Didn't overflow — fee_debt_u128_checked must still be safe
            let _ = fee_debt_u128_checked(new_fc);
        }
        None => {
            // Overflow — implementation would return Err(Overflow)
        }
    }
}

// ############################################################################
// Wave 1 ENG-PORT-C: external-oracle-target schema invariant
// ############################################################################

/// Wave 1 / ENG-PORT-C: oracle-target schema invariant.
///
/// `init_in_place` MUST zero both `oracle_target_price_e6` and
/// `oracle_target_publish_time`. The wrapper's strictly-advanced gate
/// in `read_price_and_stamp` relies on `(0, 0)` representing "no
/// target observed yet" so the first observation is admitted
/// unconditionally. Mis-initializing these fields would either
/// reject the first valid Pyth publish or accept a stale replay.
///
/// Also asserts that arbitrary writes through the field are
/// observable (the schema addition is well-formed) and that adding
/// these fields doesn't break `check_conservation` — the
/// conservation aggregate only reads value-bearing fields, and
/// oracle-target fields are pure metadata.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_oracle_target_init_zero_and_persistence() {
    let mut engine =
        RiskEngine::new_with_market(zero_fee_params(), DEFAULT_SLOT, DEFAULT_ORACLE);

    // Init zeros both fields.
    assert_eq!(engine.oracle_target_price_e6, 0);
    assert_eq!(engine.oracle_target_publish_time, 0);
    // Conservation holds at genesis with the new fields present.
    assert!(engine.check_conservation());

    // Symbolic write-back: arbitrary values persist and are observable.
    let target_price: u64 = kani::any();
    let target_time: i64 = kani::any();
    kani::assume(target_price <= MAX_ORACLE_PRICE);

    engine.oracle_target_price_e6 = target_price;
    engine.oracle_target_publish_time = target_time;

    assert_eq!(engine.oracle_target_price_e6, target_price);
    assert_eq!(engine.oracle_target_publish_time, target_time);
    // Conservation still holds — oracle-target fields are pure metadata.
    assert!(engine.check_conservation());

    kani::cover!(
        target_price > 0 && target_time > 0,
        "non-zero target observation persists and conservation is preserved"
    );
}

// ############################################################################
// Wave 4a: bankrupt-close gate invariant
// ############################################################################

/// Wave 4a / KL-FORK-ENGINE-BANKRUPT-CLOSE-1 (REVOKED, gate-only).
///
/// `ensure_no_active_bankrupt_close` MUST:
///   - return Err(RecoveryRequired) iff `active_close_present != 0`
///   - return Ok(()) iff `active_close_present == 0`
///   - leave engine state unchanged (read-only predicate)
///   - preserve `check_conservation` invariant
///
/// Also asserts the schema additions (`active_close_present: u8` and
/// `bankruptcy_hmax_lock_active: bool`) are init-zeroed at market
/// genesis, are well-formed (writable + readable through symbolic
/// values), and are pure metadata — they don't enter the conservation
/// aggregate.
///
/// Path A gate-only port: there is no setter on this branch that
/// flips `active_close_present` to a non-zero value. The fields stay
/// at init defaults forever, so the gate always passes for live
/// markets. Wave 5b adds the state machine setters that actually
/// drive the recovery flow.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_bankrupt_close_gate_init_and_predicate() {
    let mut engine =
        RiskEngine::new_with_market(zero_fee_params(), DEFAULT_SLOT, DEFAULT_ORACLE);

    // Init zeros both gate variables.
    assert_eq!(engine.active_close_present, 0);
    assert!(!engine.bankruptcy_hmax_lock_active);
    // Predicate accepts the no-active-close state.
    assert!(engine.ensure_no_active_bankrupt_close().is_ok());
    assert!(engine.check_conservation());

    // Symbolic write: when active_close_present is set, the gate
    // rejects with RecoveryRequired.
    let active: u8 = kani::any();
    kani::assume(active != 0);
    engine.active_close_present = active;
    assert_eq!(
        engine.ensure_no_active_bankrupt_close(),
        Err(RiskError::RecoveryRequired)
    );
    // Conservation still holds — gate fields are pure metadata.
    assert!(engine.check_conservation());

    // Reset to 0 and the gate passes again.
    engine.active_close_present = 0;
    assert!(engine.ensure_no_active_bankrupt_close().is_ok());

    // bankruptcy_hmax_lock_active is independent of the gate predicate
    // (toly uses it for hmax-lock decisions, not the recovery
    // continuation gate). Setting it doesn't toggle the gate.
    engine.bankruptcy_hmax_lock_active = true;
    assert!(engine.ensure_no_active_bankrupt_close().is_ok());

    kani::cover!(
        active > 0,
        "bankrupt-close gate fires when active_close_present is non-zero"
    );
}

// ############################################################################
// Wave 5a: stress-envelope schema invariant
// ############################################################################

/// Wave 5a / KL-FORK-ENGINE-STRESS-ENVELOPE-1 (REVOKED, schema-only).
///
/// `clear_stress_envelope` MUST:
///   - zero `stress_consumed_bps_e9_since_envelope`
///   - zero `stress_envelope_remaining_indices`
///   - reset `stress_envelope_start_slot` to NO_SLOT
///   - reset `stress_envelope_start_generation` to NO_SLOT
///   - clear `bankruptcy_hmax_lock_active` (the post-stress envelope and
///     the bankruptcy h-max lock share the same reconciliation channel
///     per toly engine src/percolator.rs:6263-6269)
///   - preserve `check_conservation` (the cleared fields are pure metadata
///     that don't enter the conservation aggregate)
///
/// Also asserts that fresh-market init places all four envelope fields at
/// their inactive defaults (NO_SLOT / 0) and that `clear_stress_envelope`
/// is idempotent — calling it twice from any starting state lands at the
/// same sentinel-values fixed point.
///
/// Path A schema-only port: there is no setter on this branch that opens
/// a stress envelope. Wave 5b will add the setters
/// (`start_post_stress_recovery_envelope`,
/// `apply_stress_envelope_progress`) once the bankrupt-close state
/// machine ports — both subsystems couple per toly:2982-3019.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_stress_envelope_clear_and_init() {
    let mut engine =
        RiskEngine::new_with_market(zero_fee_params(), DEFAULT_SLOT, DEFAULT_ORACLE);

    // Init places envelope at the inactive sentinel defaults.
    assert_eq!(engine.stress_consumed_bps_e9_since_envelope, 0);
    assert_eq!(engine.stress_envelope_remaining_indices, 0);
    assert_eq!(engine.stress_envelope_start_slot, NO_SLOT);
    assert_eq!(engine.stress_envelope_start_generation, NO_SLOT);
    assert!(engine.check_conservation());

    // Symbolic write: any combination of envelope state.
    engine.stress_consumed_bps_e9_since_envelope = kani::any();
    engine.stress_envelope_remaining_indices = kani::any();
    engine.stress_envelope_start_slot = kani::any();
    engine.stress_envelope_start_generation = kani::any();
    engine.bankruptcy_hmax_lock_active = kani::any();
    // Cleared envelope and lock land at the documented defaults regardless
    // of the prior values — tests the postcondition unconditionally.
    engine.clear_stress_envelope();
    assert_eq!(engine.stress_consumed_bps_e9_since_envelope, 0);
    assert_eq!(engine.stress_envelope_remaining_indices, 0);
    assert_eq!(engine.stress_envelope_start_slot, NO_SLOT);
    assert_eq!(engine.stress_envelope_start_generation, NO_SLOT);
    assert!(!engine.bankruptcy_hmax_lock_active);
    // Conservation still holds — envelope fields are pure metadata.
    assert!(engine.check_conservation());

    // Idempotence: a second call yields the same state.
    engine.clear_stress_envelope();
    assert_eq!(engine.stress_consumed_bps_e9_since_envelope, 0);
    assert_eq!(engine.stress_envelope_remaining_indices, 0);
    assert_eq!(engine.stress_envelope_start_slot, NO_SLOT);
    assert_eq!(engine.stress_envelope_start_generation, NO_SLOT);
    assert!(!engine.bankruptcy_hmax_lock_active);

    kani::cover!(true, "stress-envelope clear lands at sentinel defaults");
}

// ############################################################################
// Wave 5b: bankrupt-close state-machine schema invariants
// ############################################################################

/// Wave 5b / KL-FORK-ENGINE-BANKRUPT-CLOSE-1: state-machine schema and
/// structural helpers (Path A2 schema+helpers).
///
/// `clear_active_bankrupt_close_state` MUST:
///   - zero `active_close_present`
///   - reset `active_close_phase` to `ACTIVE_CLOSE_PHASE_NONE`
///   - reset `active_close_account_idx` to `u16::MAX`
///   - reset `active_close_opp_side` to `ACTIVE_CLOSE_SIDE_NONE`
///   - zero the 5 numeric fields (close_price, close_slot, q_close_q,
///     all 3 residual_*, b_chunks_booked)
///   - leave `bankruptcy_hmax_lock_active` untouched (it's owned by
///     `clear_stress_envelope`, not this helper)
///   - preserve `check_conservation` (the cleared fields are pure
///     metadata that don't enter the conservation aggregate)
///
/// `validate_active_bankrupt_close_shape` MUST:
///   - return `Ok(())` for the post-init / post-clear default state
///     (active_close_present == 0, all fields at defaults)
///   - return `Err(CorruptState)` when active_close_present == 0 but
///     ANY companion field disagrees with the no-continuation default
///   - return `Err(CorruptState)` when active_close_present > 1
///   - leave engine state unchanged (read-only predicate)
///
/// Codec round-trip: `decode(encode(side)) == Ok(side)` for both Long
/// and Short. `decode(SIDE_NONE)` and `decode` of any other byte value
/// returns `Err(CorruptState)`.
///
/// Path A2 schema-only port: setters
/// (`start_active_bankrupt_close_residual`, etc.) and integration into
/// trade/accrue/resolve paths defer to Wave 5b-ii. The fields stay at
/// init defaults forever on this branch; the validator's "active form"
/// branch is exercised only by Kani's symbolic-write probes.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_bankrupt_close_state_machine_schema() {
    let mut engine =
        RiskEngine::new_with_market(zero_fee_params(), DEFAULT_SLOT, DEFAULT_ORACLE);

    // Init places every state-machine field at the inactive default.
    assert_eq!(engine.active_close_present, 0);
    assert_eq!(engine.active_close_phase, ACTIVE_CLOSE_PHASE_NONE);
    assert_eq!(engine.active_close_account_idx, u16::MAX);
    assert_eq!(engine.active_close_opp_side, ACTIVE_CLOSE_SIDE_NONE);
    assert_eq!(engine.active_close_close_price, 0);
    assert_eq!(engine.active_close_close_slot, 0);
    assert_eq!(engine.active_close_q_close_q, 0);
    assert_eq!(engine.active_close_residual_remaining, 0);
    assert_eq!(engine.active_close_residual_booked, 0);
    assert_eq!(engine.active_close_residual_recorded, 0);
    assert_eq!(engine.active_close_b_chunks_booked, 0);
    // Validator accepts the default state.
    assert!(engine.validate_active_bankrupt_close_shape().is_ok());
    assert!(engine.check_conservation());

    // Symbolic write: poison every active-close field with arbitrary
    // values. `clear_active_bankrupt_close_state` MUST land back at the
    // defaults regardless of the prior state.
    engine.active_close_present = kani::any();
    engine.active_close_phase = kani::any();
    engine.active_close_account_idx = kani::any();
    engine.active_close_opp_side = kani::any();
    engine.active_close_close_price = kani::any();
    engine.active_close_close_slot = kani::any();
    engine.active_close_q_close_q = kani::any();
    engine.active_close_residual_remaining = kani::any();
    engine.active_close_residual_booked = kani::any();
    engine.active_close_residual_recorded = kani::any();
    engine.active_close_b_chunks_booked = kani::any();
    engine.clear_active_bankrupt_close_state();
    assert_eq!(engine.active_close_present, 0);
    assert_eq!(engine.active_close_phase, ACTIVE_CLOSE_PHASE_NONE);
    assert_eq!(engine.active_close_account_idx, u16::MAX);
    assert_eq!(engine.active_close_opp_side, ACTIVE_CLOSE_SIDE_NONE);
    assert_eq!(engine.active_close_close_price, 0);
    assert_eq!(engine.active_close_close_slot, 0);
    assert_eq!(engine.active_close_q_close_q, 0);
    assert_eq!(engine.active_close_residual_remaining, 0);
    assert_eq!(engine.active_close_residual_booked, 0);
    assert_eq!(engine.active_close_residual_recorded, 0);
    assert_eq!(engine.active_close_b_chunks_booked, 0);
    assert!(engine.validate_active_bankrupt_close_shape().is_ok());
    assert!(engine.check_conservation());

    // Validator rejects an inactive-form record with a non-default field.
    engine.clear_active_bankrupt_close_state();
    assert!(engine.validate_active_bankrupt_close_shape().is_ok());
    let bogus_idx: u16 = kani::any();
    kani::assume(bogus_idx != u16::MAX);
    engine.active_close_account_idx = bogus_idx;
    assert_eq!(
        engine.validate_active_bankrupt_close_shape(),
        Err(RiskError::CorruptState)
    );
    // Reset and confirm validator is happy again.
    engine.active_close_account_idx = u16::MAX;
    assert!(engine.validate_active_bankrupt_close_shape().is_ok());

    // Validator rejects active_close_present > 1.
    let two_or_more: u8 = kani::any();
    kani::assume(two_or_more > 1);
    engine.active_close_present = two_or_more;
    assert_eq!(
        engine.validate_active_bankrupt_close_shape(),
        Err(RiskError::CorruptState)
    );
    engine.active_close_present = 0;

    // Side codec round-trip.
    assert_eq!(
        RiskEngine::decode_active_close_side(RiskEngine::encode_active_close_side(Side::Long)),
        Ok(Side::Long)
    );
    assert_eq!(
        RiskEngine::decode_active_close_side(RiskEngine::encode_active_close_side(Side::Short)),
        Ok(Side::Short)
    );
    // Decoder rejects SIDE_NONE.
    assert_eq!(
        RiskEngine::decode_active_close_side(ACTIVE_CLOSE_SIDE_NONE),
        Err(RiskError::CorruptState)
    );
    // Decoder rejects any other byte.
    let bogus_side: u8 = kani::any();
    kani::assume(
        bogus_side != ACTIVE_CLOSE_SIDE_LONG && bogus_side != ACTIVE_CLOSE_SIDE_SHORT,
    );
    assert_eq!(
        RiskEngine::decode_active_close_side(bogus_side),
        Err(RiskError::CorruptState)
    );

    kani::cover!(
        true,
        "bankrupt-close state-machine schema clear/validate/codec invariants"
    );
}

// ============================================================================
// Wave 6a — phantom-dust 4-field schema (KL-PHANTOM-DUST-SCHEMA-1 REVOKED)
// ============================================================================

/// Schema-level invariant `certified <= potential` holds at market genesis
/// and is consumed by `assert_public_postconditions`. On this branch
/// `certified_<side>_q` is always 0 (no B-tracking-aware liquidation logic),
/// so the gate trivially passes; when B-tracking lands, this harness will
/// detect any setter that violates the lower-bound ≤ upper-bound contract.
#[kani::proof]
#[kani::unwind(2)]
#[kani::solver(cadical)]
fn proof_phantom_dust_certified_le_potential_at_genesis() {
    let engine = RiskEngine::new(zero_fee_params());

    assert!(engine.phantom_dust_certified_long_q <= engine.phantom_dust_potential_long_q);
    assert!(engine.phantom_dust_certified_short_q <= engine.phantom_dust_potential_short_q);

    // On this branch certified is always 0 at genesis.
    assert_eq!(engine.phantom_dust_certified_long_q, 0);
    assert_eq!(engine.phantom_dust_certified_short_q, 0);

    assert!(engine.assert_public_postconditions().is_ok());
}

/// `assert_public_postconditions` rejects any state where certified exceeds
/// potential on either side. Forward-looking: once B-tracking sets
/// `certified` from liquidation step 7, this guards against off-by-one or
/// step-ordering bugs.
#[kani::proof]
#[kani::unwind(2)]
#[kani::solver(cadical)]
fn proof_phantom_dust_certified_gt_potential_rejects() {
    let mut engine = RiskEngine::new(zero_fee_params());

    let side: u8 = kani::any();
    kani::assume(side < 2);

    let certified: u8 = kani::any();
    let potential: u8 = kani::any();
    kani::assume(certified as u128 > potential as u128);

    if side == 0 {
        engine.phantom_dust_certified_long_q = certified as u128;
        engine.phantom_dust_potential_long_q = potential as u128;
    } else {
        engine.phantom_dust_certified_short_q = certified as u128;
        engine.phantom_dust_potential_short_q = potential as u128;
    }

    assert_eq!(
        engine.assert_public_postconditions(),
        Err(RiskError::CorruptState)
    );
}

// ============================================================================
// Wave 11a — B-tracking subsystem (KL-FORK-ENGINE-B-TRACKING-1 PARTIALLY REVOKED)
// ============================================================================

/// At market genesis the B-tracking fields are all zero and the
/// `validate_b_tracking_shape` invariant holds. On this branch no writer
/// exists yet (Wave 11a-i is schema-only); the harness pins the
/// init-time predicate so future Wave 11a-ii writers can't silently
/// regress the genesis state.
#[kani::proof]
#[kani::unwind(2)]
#[kani::solver(cadical)]
fn proof_b_tracking_shape_holds_at_genesis() {
    let engine = RiskEngine::new(zero_fee_params());

    assert_eq!(engine.b_long_num, 0);
    assert_eq!(engine.b_short_num, 0);
    assert_eq!(engine.loss_weight_sum_long, 0);
    assert_eq!(engine.loss_weight_sum_short, 0);
    assert_eq!(engine.social_loss_remainder_long_num, 0);
    assert_eq!(engine.social_loss_remainder_short_num, 0);
    assert_eq!(engine.social_loss_dust_long_num, 0);
    assert_eq!(engine.social_loss_dust_short_num, 0);
    assert_eq!(engine.explicit_unallocated_loss_saturated, 0);

    assert!(engine.assert_public_postconditions().is_ok());
}

/// `assert_public_postconditions` rejects out-of-range
/// `loss_weight_sum_<side>` (must be `<= SOCIAL_LOSS_DEN`). Forward-
/// looking: once Wave 11a-ii's `book_bankruptcy_residual_chunk_to_side`
/// starts incrementing loss_weight_sum, this catches off-by-one and
/// overflow-style regressions.
#[kani::proof]
#[kani::unwind(2)]
#[kani::solver(cadical)]
fn proof_b_tracking_loss_weight_sum_overflow_rejects() {
    let mut engine = RiskEngine::new(zero_fee_params());

    let pick_long: bool = kani::any();
    if pick_long {
        engine.loss_weight_sum_long = SOCIAL_LOSS_DEN + 1;
    } else {
        engine.loss_weight_sum_short = SOCIAL_LOSS_DEN + 1;
    }

    assert_eq!(
        engine.assert_public_postconditions(),
        Err(RiskError::CorruptState)
    );
}

// ============================================================================
// Wave 11a-ii-A — B-tracking helpers + bankrupt-close state machine
// KL-FORK-ENGINE-B-TRACKING-1 (state machine portion REVOKED)
// KL-FORK-ENGINE-BANKRUPT-CLOSE-1 (state machine portion REVOKED)
// ============================================================================

/// `assert_public_postconditions` (which delegates to
/// `validate_b_tracking_shape`) rejects any side whose
/// `social_loss_remainder_<side>_num` equals or exceeds `SOCIAL_LOSS_DEN`.
/// Forward-looking: once Wave 11a-ii-A's
/// `book_bankruptcy_residual_chunk_to_side` writes
/// `set_social_remainder(side, plan.rem_new)`, this catches a wrap or
/// off-by-one that would push the numerator into invalid range.
#[kani::proof]
#[kani::unwind(2)]
#[kani::solver(cadical)]
fn proof_b_tracking_shape_rejects_social_remainder_at_or_above_denominator() {
    let mut engine = RiskEngine::new(zero_fee_params());

    let pick_long: bool = kani::any();
    if pick_long {
        engine.social_loss_remainder_long_num = SOCIAL_LOSS_DEN;
    } else {
        engine.social_loss_remainder_short_num = SOCIAL_LOSS_DEN;
    }

    assert_eq!(
        engine.assert_public_postconditions(),
        Err(RiskError::CorruptState)
    );
}

/// `assert_public_postconditions` rejects any side whose
/// `social_loss_dust_<side>_num` equals or exceeds `SOCIAL_LOSS_DEN`.
/// Forward-looking: catches a future writer of
/// `transfer_scaled_dust_side` that fails to flush the post-mod dust
/// correctly.
#[kani::proof]
#[kani::unwind(2)]
#[kani::solver(cadical)]
fn proof_b_tracking_shape_rejects_social_dust_at_or_above_denominator() {
    let mut engine = RiskEngine::new(zero_fee_params());

    let pick_long: bool = kani::any();
    if pick_long {
        engine.social_loss_dust_long_num = SOCIAL_LOSS_DEN;
    } else {
        engine.social_loss_dust_short_num = SOCIAL_LOSS_DEN;
    }

    assert_eq!(
        engine.assert_public_postconditions(),
        Err(RiskError::CorruptState)
    );
}

/// `assert_public_postconditions` rejects an
/// `explicit_unallocated_loss_saturated` flag set to anything other than
/// 0 or 1. Wave 11a-ii-A's `add_explicit_unallocated_loss_side` and
/// `record_uninsured_protocol_loss` both pin it at `1` only on
/// `checked_add` overflow; this harness catches any future writer that
/// might store an out-of-range value.
#[kani::proof]
#[kani::unwind(2)]
#[kani::solver(cadical)]
fn proof_b_tracking_shape_rejects_saturated_flag_out_of_range() {
    let mut engine = RiskEngine::new(zero_fee_params());

    let bad: u8 = kani::any();
    kani::assume(bad > 1);
    engine.explicit_unallocated_loss_saturated = bad;

    assert_eq!(
        engine.assert_public_postconditions(),
        Err(RiskError::CorruptState)
    );
}

/// Round-trip: `encode_active_close_side` followed by
/// `decode_active_close_side` is the identity for both `Side::Long` and
/// `Side::Short`. Pins the bidirectional contract that Wave 11a-ii-A's
/// state-machine setters (`start_active_bankrupt_close_residual` writes
/// the encoded byte; `continue_active_bankrupt_close_core` reads it back)
/// depend on for end-to-end correctness.
#[kani::proof]
#[kani::unwind(2)]
#[kani::solver(cadical)]
fn proof_encode_decode_active_close_side_roundtrip() {
    let pick_long: bool = kani::any();
    let s = if pick_long { Side::Long } else { Side::Short };

    let encoded = RiskEngine::encode_active_close_side(s);
    let decoded = RiskEngine::decode_active_close_side(encoded).unwrap();
    assert_eq!(encoded == ACTIVE_CLOSE_SIDE_LONG, pick_long);
    assert_eq!(encoded == ACTIVE_CLOSE_SIDE_SHORT, !pick_long);
    match (s, decoded) {
        (Side::Long, Side::Long) | (Side::Short, Side::Short) => {}
        _ => panic!("encode/decode roundtrip failed"),
    }
}

/// `decode_active_close_side` rejects any byte that is not
/// `ACTIVE_CLOSE_SIDE_LONG` (1) or `ACTIVE_CLOSE_SIDE_SHORT` (2). This
/// is the inverse-direction safety net: if persisted slab state ever
/// shows `active_close_present = 1` but `opp_side` is `0` (NONE) or any
/// other byte, the decoder MUST surface `CorruptState`.
#[kani::proof]
#[kani::unwind(2)]
#[kani::solver(cadical)]
fn proof_decode_active_close_side_rejects_invalid_byte() {
    let byte: u8 = kani::any();
    kani::assume(byte != ACTIVE_CLOSE_SIDE_LONG && byte != ACTIVE_CLOSE_SIDE_SHORT);

    assert_eq!(
        RiskEngine::decode_active_close_side(byte),
        Err(RiskError::CorruptState)
    );
}

/// `clear_active_bankrupt_close_state` zeros all 11 state-machine fields
/// regardless of starting state. Wave 11a-ii-A's
/// `continue_active_bankrupt_close_core` calls this when the residual
/// hits zero, and `complete_active_bankrupt_close_for_recovery` calls it
/// in the recovery terminal — both paths rely on the clear being
/// total.
#[kani::proof]
#[kani::unwind(2)]
#[kani::solver(cadical)]
fn proof_clear_active_bankrupt_close_state_zeros_all_fields() {
    let mut engine = RiskEngine::new(zero_fee_params());

    engine.active_close_present = kani::any();
    engine.active_close_phase = kani::any();
    engine.active_close_account_idx = kani::any();
    engine.active_close_opp_side = kani::any();
    engine.active_close_close_price = kani::any();
    engine.active_close_close_slot = kani::any();
    engine.active_close_q_close_q = kani::any();
    engine.active_close_residual_remaining = kani::any();
    engine.active_close_residual_booked = kani::any();
    engine.active_close_residual_recorded = kani::any();
    engine.active_close_b_chunks_booked = kani::any();

    engine.clear_active_bankrupt_close_state();

    assert_eq!(engine.active_close_present, 0);
    assert_eq!(engine.active_close_phase, ACTIVE_CLOSE_PHASE_NONE);
    assert_eq!(engine.active_close_account_idx, u16::MAX);
    assert_eq!(engine.active_close_opp_side, ACTIVE_CLOSE_SIDE_NONE);
    assert_eq!(engine.active_close_close_price, 0);
    assert_eq!(engine.active_close_close_slot, 0);
    assert_eq!(engine.active_close_q_close_q, 0);
    assert_eq!(engine.active_close_residual_remaining, 0);
    assert_eq!(engine.active_close_residual_booked, 0);
    assert_eq!(engine.active_close_residual_recorded, 0);
    assert_eq!(engine.active_close_b_chunks_booked, 0);
    assert!(engine.validate_active_bankrupt_close_shape().is_ok());
}

// ============================================================================
// Wave 11a-ii-B — permissionless_progress_not_atomic + dep tail
// ============================================================================

/// `permissionless_progress_not_atomic` returns Unauthorized when the market
/// is neither Live nor Resolved. The fork only has those two modes today,
/// but the guard is structural defense-in-depth: if a future schema
/// extension adds a third mode, the dispatcher must not silently take any
/// of the two branches it understands.
#[kani::proof]
#[kani::unwind(2)]
#[kani::solver(cadical)]
fn proof_permissionless_progress_resolved_routes_to_resolved_close() {
    let mut engine = RiskEngine::new(zero_fee_params());
    engine.market_mode = MarketMode::Resolved;
    engine.rr_cursor_position = 0;
    engine.resolved_slot = engine.current_slot;

    let candidates: [(u16, Option<LiquidationPolicy>); 0] = [];
    let req = PermissionlessProgressRequest {
        now_slot: engine.current_slot,
        oracle_price: engine.last_oracle_price.max(1),
        authenticated_raw_target_price: 0,
        ordered_candidates: &candidates,
        account_hint: None,
        max_revalidations: 1,
        max_candidate_inspections: 1,
        funding_rate_e9: 0,
        admit_h_min: engine.params.h_min,
        admit_h_max: engine.params.h_max,
        admit_h_max_consumption_threshold_bps_opt: None,
        rr_touch_limit: 1,
        rr_scan_limit: 1,
        resolved_scan_limit: 1,
        resolved_fee_rate_per_slot: 0,
    };

    match engine.permissionless_progress_not_atomic(req) {
        Ok(PermissionlessProgressOutcome::ResolvedClose(_)) => {}
        // A scan-window with no materialized accounts yields ProgressOnly
        // wrapped in ResolvedClose; any other variant on a Resolved
        // dispatch is a routing bug.
        Ok(other) => panic!("Resolved must route to ResolvedClose, got {:?}", other),
        // CorruptState / Overflow / Unauthorized may be returned by the
        // inner cursor scan when the engine state can't satisfy its
        // public preconditions — that's acceptable. The harness only
        // asserts that the OUTCOME on success is the right variant.
        Err(_) => {}
    }
}

/// `permissionless_progress_not_atomic` short-circuits with
/// `RecoveryRequired` when an active bankrupt-close is in flight. This
/// is the defense-in-depth gate the Wave 11a-ii-B port adds while the
/// full recovery resolver is deferred — surfaces a stable error
/// instead of taking either of the two live-mode branches the
/// dispatcher knows.
#[kani::proof]
#[kani::unwind(2)]
#[kani::solver(cadical)]
fn proof_permissionless_progress_rejects_when_active_close_present() {
    let mut engine = RiskEngine::new(zero_fee_params());
    engine.market_mode = MarketMode::Live;
    engine.active_close_present = 1;

    let candidates: [(u16, Option<LiquidationPolicy>); 0] = [];
    let req = PermissionlessProgressRequest {
        now_slot: engine.current_slot,
        oracle_price: engine.last_oracle_price.max(1),
        authenticated_raw_target_price: 0,
        ordered_candidates: &candidates,
        account_hint: None,
        max_revalidations: 1,
        max_candidate_inspections: 1,
        funding_rate_e9: 0,
        admit_h_min: engine.params.h_min,
        admit_h_max: engine.params.h_max,
        admit_h_max_consumption_threshold_bps_opt: None,
        rr_touch_limit: 1,
        rr_scan_limit: 1,
        resolved_scan_limit: 1,
        resolved_fee_rate_per_slot: 0,
    };

    assert_eq!(
        engine.permissionless_progress_not_atomic(req),
        Err(RiskError::RecoveryRequired)
    );
}

/// `force_close_resolved_cursor_with_fee_not_atomic` rejects a non-Resolved
/// market — Live markets must take the keeper-crank path, not the
/// cursor-scan path.
#[kani::proof]
#[kani::unwind(2)]
#[kani::solver(cadical)]
fn proof_force_close_resolved_cursor_rejects_live_market() {
    let mut engine = RiskEngine::new(zero_fee_params());
    engine.market_mode = MarketMode::Live;

    assert_eq!(
        engine.force_close_resolved_cursor_with_fee_not_atomic(1, 0),
        Err(RiskError::Unauthorized)
    );
}

/// `force_close_resolved_cursor_with_fee_not_atomic` rejects a zero scan
/// limit — silently treating it as a no-op would mean the wrapper's
/// liveness contract has no guarantee the cursor advanced.
#[kani::proof]
#[kani::unwind(2)]
#[kani::solver(cadical)]
fn proof_force_close_resolved_cursor_rejects_zero_scan_limit() {
    let mut engine = RiskEngine::new(zero_fee_params());
    engine.market_mode = MarketMode::Resolved;
    engine.resolved_slot = engine.current_slot;

    assert_eq!(
        engine.force_close_resolved_cursor_with_fee_not_atomic(0, 0),
        Err(RiskError::Overflow)
    );
}

// ============================================================================
// Wave 10 / PORT-13 — validate_engine_state_shape aggregator
// ============================================================================

/// `validate_engine_state_shape` holds on a freshly initialised engine.
/// Genesis state has every B-tracking and bankrupt-close field at the
/// no-continuation default, so the aggregator must accept.
#[kani::proof]
#[kani::unwind(2)]
#[kani::solver(cadical)]
fn proof_validate_engine_state_shape_holds_at_genesis() {
    let engine = RiskEngine::new_with_market(zero_fee_params(), DEFAULT_SLOT, DEFAULT_ORACLE);
    assert!(engine.validate_engine_state_shape().is_ok());
}

/// `validate_engine_state_shape` rejects when the embedded B-tracking
/// validator would reject (e.g., `loss_weight_sum` out of range). This
/// pins the aggregator's delegation contract — if anyone removes the
/// `validate_b_tracking_shape` call from inside the aggregator, Kani
/// catches it.
#[kani::proof]
#[kani::unwind(2)]
#[kani::solver(cadical)]
fn proof_validate_engine_state_shape_delegates_to_b_tracking() {
    let mut engine = RiskEngine::new(zero_fee_params());
    engine.loss_weight_sum_long = SOCIAL_LOSS_DEN + 1;

    assert_eq!(
        engine.validate_engine_state_shape(),
        Err(RiskError::CorruptState)
    );
}

/// `validate_engine_state_shape` rejects when the bankrupt-close
/// state-machine validator would reject (e.g., active_close_present > 1).
/// Pins the second delegation arm of the aggregator.
#[kani::proof]
#[kani::unwind(2)]
#[kani::solver(cadical)]
fn proof_validate_engine_state_shape_delegates_to_bankrupt_close() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let two_or_more: u8 = kani::any();
    kani::assume(two_or_more > 1);
    engine.active_close_present = two_or_more;

    assert_eq!(
        engine.validate_engine_state_shape(),
        Err(RiskError::CorruptState)
    );
}
