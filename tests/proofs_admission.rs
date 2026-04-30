//! v12.18 admission-pair + sticky h_max + touch acceleration proofs (§4.7, §4.9)
//!
//! Proof groups:
//!   AH: Admission with pair + sticky rule (§4.7)
//!   AC: Acceleration on touch (§4.9)
//!   IN: Instruction-level invariants specific to v12.18

#![cfg(kani)]

mod common;
use common::*;

// ============================================================================
// AH-1: Single admission returns exactly admit_h_min or admit_h_max.
// ============================================================================

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn ah1_single_admission_range() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap();
    // Inject some vault/c_tot to make residual non-degenerate
    engine.vault = U128::new(1000);
    engine.c_tot = U128::new(500);

    let fresh: u8 = kani::any();
    kani::assume(fresh > 0);

    let admit_h_min: u8 = kani::any();
    let admit_h_max: u8 = kani::any();
    kani::assume(admit_h_min as u64 <= admit_h_max as u64);
    kani::assume(admit_h_max > 0);
    kani::assume(admit_h_max as u64 <= engine.params.h_max);

    let mut ctx = InstructionContext::new_with_admission(admit_h_min as u64, admit_h_max as u64);

    let h_eff = engine
        .admit_fresh_reserve_h_lock(
            idx as usize,
            fresh as u128,
            &mut ctx,
            admit_h_min as u64,
            admit_h_max as u64,
        )
        .unwrap();

    // Returned horizon is exactly one of the two inputs
    assert!(h_eff == admit_h_min as u64 || h_eff == admit_h_max as u64);

    // Admission law check
    let senior = engine.c_tot.get() + engine.insurance_fund.balance.get();
    let residual = engine.vault.get().saturating_sub(senior);
    let matured_plus_fresh = engine.pnl_matured_pos_tot.saturating_add(fresh as u128);
    if matured_plus_fresh <= residual {
        assert!(h_eff == admit_h_min as u64);
    } else {
        assert!(h_eff == admit_h_max as u64);
        assert!(ctx.is_h_max_sticky(idx));
    }
}

// ============================================================================
// AH-2: Sticky-H_max is absorbing. Once sticky, always returns admit_h_max.
// ============================================================================

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn ah2_sticky_is_absorbing() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap();
    engine.vault = U128::new(10_000); // plenty of residual — admission WOULD normally give h_min

    let admit_h_min: u8 = kani::any();
    let admit_h_max: u8 = kani::any();
    kani::assume((admit_h_min as u64) < (admit_h_max as u64)); // non-degenerate
    kani::assume(admit_h_max > 0);
    kani::assume(admit_h_max as u64 <= engine.params.h_max);

    let mut ctx = InstructionContext::new_with_admission(admit_h_min as u64, admit_h_max as u64);
    // Force idx into sticky set
    ctx.mark_h_max_sticky(idx);

    let fresh: u8 = kani::any();
    kani::assume(fresh > 0);

    let h_eff = engine
        .admit_fresh_reserve_h_lock(
            idx as usize,
            fresh as u128,
            &mut ctx,
            admit_h_min as u64,
            admit_h_max as u64,
        )
        .unwrap();

    // Sticky forces h_max regardless of residual
    assert!(h_eff == admit_h_max as u64);
    assert!(ctx.is_h_max_sticky(idx));
}

// ============================================================================
// AH-3: No under-admission (v12.18 core fix).
// After first admission forces h_max, second call on same account cannot
// return h_min even if current state would suggest it.
// ============================================================================

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn ah3_no_under_admission() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap();
    // Start constrained: residual = 0 so first fresh triggers h_max
    engine.vault = U128::new(100);
    engine.c_tot = U128::new(100);
    engine.pnl_matured_pos_tot = 0;

    let admit_h_min: u8 = kani::any();
    let admit_h_max: u8 = kani::any();
    kani::assume((admit_h_min as u64) < (admit_h_max as u64));
    kani::assume(admit_h_max > 0);
    kani::assume(admit_h_max as u64 <= engine.params.h_max);

    let mut ctx = InstructionContext::new_with_admission(admit_h_min as u64, admit_h_max as u64);

    // First admission: residual = 0, any positive fresh overflows → h_max
    let fresh1: u8 = kani::any();
    kani::assume(fresh1 > 0);
    let h1 = engine
        .admit_fresh_reserve_h_lock(
            idx as usize,
            fresh1 as u128,
            &mut ctx,
            admit_h_min as u64,
            admit_h_max as u64,
        )
        .unwrap();
    assert!(h1 == admit_h_max as u64);
    assert!(ctx.is_h_max_sticky(idx));

    // Simulate arbitrary state evolution: residual could grow huge
    engine.vault = U128::new(u128::MAX / 2);

    // Second admission: state now admits h_min, but sticky forces h_max
    let fresh2: u8 = kani::any();
    kani::assume(fresh2 > 0);
    let h2 = engine
        .admit_fresh_reserve_h_lock(
            idx as usize,
            fresh2 as u128,
            &mut ctx,
            admit_h_min as u64,
            admit_h_max as u64,
        )
        .unwrap();
    assert!(h2 == admit_h_max as u64);
}

// ============================================================================
// AH-4: h_min=0 admission preserves h=1 invariant.
// If admission returns 0 and caller instantly matures, residual still >= matured.
// ============================================================================

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn ah4_hmin_zero_preserves_h_equals_one() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap();

    // Small bounded values
    let v: u16 = kani::any();
    let ct: u16 = kani::any();
    kani::assume(ct as u128 <= v as u128);
    engine.vault = U128::new(v as u128);
    engine.c_tot = U128::new(ct as u128);
    let matured: u16 = kani::any();
    let residual = (v as u128).saturating_sub(ct as u128);
    kani::assume(matured as u128 <= residual); // precondition: h = 1
    engine.pnl_matured_pos_tot = matured as u128;
    engine.pnl_pos_tot = matured as u128;

    let admit_h_min = 0u64;
    let admit_h_max: u8 = kani::any();
    kani::assume(admit_h_max > 0);
    kani::assume(admit_h_max as u64 <= engine.params.h_max);
    let mut ctx = InstructionContext::new_with_admission(admit_h_min, admit_h_max as u64);

    let fresh: u8 = kani::any();
    kani::assume(fresh > 0);

    let h_eff = engine
        .admit_fresh_reserve_h_lock(
            idx as usize,
            fresh as u128,
            &mut ctx,
            admit_h_min,
            admit_h_max as u64,
        )
        .unwrap();

    if h_eff == 0 {
        // Simulate §4.8 clause 10: instant release
        let new_matured = engine.pnl_matured_pos_tot.saturating_add(fresh as u128);
        let senior = engine.c_tot.get() + engine.insurance_fund.balance.get();
        let new_residual = engine.vault.get().saturating_sub(senior);
        // h = 1 still holds
        assert!(new_matured <= new_residual);
    }
}

// ============================================================================
// AH-5: Cross-account sticky isolation.
// Sticky set for account a does NOT force h_max for account b.
// ============================================================================

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn ah5_cross_account_sticky_isolation() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let a = add_user_test(&mut engine, 0).unwrap();
    let b = add_user_test(&mut engine, 0).unwrap();
    // Healthy residual: admission would give h_min
    engine.vault = U128::new(10_000);
    engine.c_tot = U128::new(0);

    let admit_h_min: u8 = kani::any();
    let admit_h_max: u8 = kani::any();
    kani::assume((admit_h_min as u64) < (admit_h_max as u64));
    kani::assume(admit_h_max > 0);
    kani::assume(admit_h_max as u64 <= engine.params.h_max);

    let mut ctx = InstructionContext::new_with_admission(admit_h_min as u64, admit_h_max as u64);
    // Mark only a sticky
    ctx.mark_h_max_sticky(a);

    // Admission for b: should return h_min since b is NOT sticky
    let fresh_b: u8 = kani::any();
    kani::assume(fresh_b > 0);
    kani::assume(fresh_b as u128 <= 100); // stays under residual

    let h_b = engine
        .admit_fresh_reserve_h_lock(
            b as usize,
            fresh_b as u128,
            &mut ctx,
            admit_h_min as u64,
            admit_h_max as u64,
        )
        .unwrap();
    assert!(h_b == admit_h_min as u64);
    // b not sticky (h_min was returned)
    assert!(!ctx.is_h_max_sticky(b));
}

// ============================================================================
// AH-6: admit_h_min > 0 is a floor. Result is never below admit_h_min.
// ============================================================================

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn ah6_positive_hmin_floor() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap();

    let admit_h_min: u8 = kani::any();
    kani::assume(admit_h_min > 0);
    let admit_h_max: u8 = kani::any();
    kani::assume(admit_h_min as u64 <= admit_h_max as u64);
    kani::assume(admit_h_max as u64 <= engine.params.h_max);

    let mut ctx = InstructionContext::new_with_admission(admit_h_min as u64, admit_h_max as u64);

    let fresh: u8 = kani::any();
    kani::assume(fresh > 0);

    let h_eff = engine
        .admit_fresh_reserve_h_lock(
            idx as usize,
            fresh as u128,
            &mut ctx,
            admit_h_min as u64,
            admit_h_max as u64,
        )
        .unwrap();

    // Result >= admit_h_min (never below the floor)
    assert!(h_eff >= admit_h_min as u64);
}

// ============================================================================
// AC-1: Acceleration is all-or-nothing.
// ============================================================================

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn ac1_acceleration_all_or_nothing() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap() as usize;

    // Spec §4.9: validate a well-formed scheduled reserve bucket.
    let r: u8 = kani::any();
    kani::assume(r > 0);
    engine.accounts[idx].reserved_pnl = r as u128;
    engine.accounts[idx].pnl = r as i128;
    engine.pnl_pos_tot = r as u128;
    engine.accounts[idx].sched_present = 1;
    engine.accounts[idx].sched_remaining_q = r as u128;
    engine.accounts[idx].sched_anchor_q = r as u128;
    engine.accounts[idx].sched_horizon = 10;
    engine.accounts[idx].sched_start_slot = 0;

    let r_before = engine.accounts[idx].reserved_pnl;
    let matured_before = engine.pnl_matured_pos_tot;
    let sched_start_before = engine.accounts[idx].sched_start_slot;
    let sched_horizon_before = engine.accounts[idx].sched_horizon;

    // Valid accounting precondition: Residual_now = V - C_tot because I = 0.
    let v: u16 = kani::any();
    let ct: u16 = kani::any();
    kani::assume(ct <= v);
    engine.vault = U128::new(v as u128);
    engine.c_tot = U128::new(ct as u128);

    let residual = (v as u128) - (ct as u128);
    let expected_accelerated = r_before <= residual;
    kani::cover!(expected_accelerated, "spec acceleration branch reachable");
    kani::cover!(!expected_accelerated, "spec unchanged branch reachable");

    let ctx = InstructionContext::new_with_admission_and_threshold(0, 10, None);
    let result = engine.admit_outstanding_reserve_on_touch(idx, &ctx);
    assert!(result.is_ok(), "valid §4.9 pre-state must not reject");

    let r_after = engine.accounts[idx].reserved_pnl;
    let matured_after = engine.pnl_matured_pos_tot;

    if expected_accelerated {
        // Spec §4.9 step 2: all outstanding reserve matures atomically.
        assert!(matured_after == matured_before + r_before);
        assert!(r_after == 0);
        assert!(engine.accounts[idx].sched_present == 0);
        assert!(engine.accounts[idx].sched_remaining_q == 0);
        assert!(engine.accounts[idx].sched_anchor_q == 0);
        assert!(engine.accounts[idx].pending_present == 0);
        assert!(matured_after <= engine.pnl_pos_tot);
        let pos_pnl = if engine.accounts[idx].pnl > 0 {
            engine.accounts[idx].pnl as u128
        } else {
            0
        };
        assert!(r_after <= pos_pnl);
    } else {
        // Spec §4.9 step 3: inadmissible reserve remains byte-stable.
        assert!(matured_after == matured_before);
        assert!(r_after == r_before);
        assert!(engine.accounts[idx].sched_present == 1);
        assert!(engine.accounts[idx].sched_remaining_q == r_before);
        assert!(engine.accounts[idx].sched_anchor_q == r_before);
        assert!(engine.accounts[idx].sched_start_slot == sched_start_before);
        assert!(engine.accounts[idx].sched_horizon == sched_horizon_before);
        assert!(engine.accounts[idx].pending_present == 0);
    }
}

// ============================================================================
// AC-2: Acceleration fires iff state admits.
// ============================================================================

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn ac2_acceleration_fires_iff_admits() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap() as usize;

    let r: u8 = kani::any();
    let matured: u8 = kani::any();
    // Set up an account whose positive PnL is fully accounted for:
    //   pnl_pos_tot = matured + r (reserved portion)
    // This matches the normative admission precondition: after firing,
    // new_matured = matured + r must not exceed pnl_pos_tot (v12.18.1
    // added this check to admit_outstanding_reserve_on_touch).
    let pos_tot = (matured as u128).checked_add(r as u128);
    kani::assume(pos_tot.is_some());
    let pos_tot = pos_tot.unwrap();
    kani::assume(pos_tot <= i128::MAX as u128);

    engine.accounts[idx].reserved_pnl = r as u128;
    engine.accounts[idx].pnl = pos_tot as i128;
    engine.pnl_pos_tot = pos_tot;
    engine.pnl_matured_pos_tot = matured as u128;
    if r > 0 {
        engine.accounts[idx].sched_present = 1;
        engine.accounts[idx].sched_remaining_q = r as u128;
        engine.accounts[idx].sched_anchor_q = r as u128;
        engine.accounts[idx].sched_horizon = 10;
    }

    let v: u16 = kani::any();
    let ct: u16 = kani::any();
    engine.vault = U128::new(v as u128);
    engine.c_tot = U128::new(ct as u128);

    let r_before = engine.accounts[idx].reserved_pnl;
    // Engine's exact admission condition: residual uses checked_sub
    // (senior <= vault required) AND matured + r <= pnl_pos_tot
    // (guaranteed by our setup).
    let senior_ok = (ct as u128) <= (v as u128);
    let residual = (v as u128).saturating_sub(ct as u128);
    let admits =
        r_before > 0 && senior_ok && (matured as u128).saturating_add(r_before) <= residual;

    let ctx = InstructionContext::new_with_admission_and_threshold(0, 10, None);
    let _ = engine.admit_outstanding_reserve_on_touch(idx, &ctx);

    let r_after = engine.accounts[idx].reserved_pnl;
    let fired = r_after == 0 && r_before > 0;

    // Fired iff state admitted
    if admits {
        assert!(fired);
    } else {
        assert!(!fired || r_before == 0);
    }
}

// ============================================================================
// AC-4: Acceleration preserves conservation & matured monotonicity.
// ============================================================================

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn ac4_acceleration_conservation() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap() as usize;

    let r: u8 = kani::any();
    engine.accounts[idx].reserved_pnl = r as u128;
    engine.accounts[idx].pnl = r as i128;
    engine.pnl_pos_tot = r as u128;
    if r > 0 {
        engine.accounts[idx].sched_present = 1;
        engine.accounts[idx].sched_remaining_q = r as u128;
        engine.accounts[idx].sched_anchor_q = r as u128;
        engine.accounts[idx].sched_horizon = 10;
    }

    let v: u16 = kani::any();
    let ct: u16 = kani::any();
    kani::assume(ct as u128 <= v as u128); // conservation precondition
    engine.vault = U128::new(v as u128);
    engine.c_tot = U128::new(ct as u128);

    let matured_before = engine.pnl_matured_pos_tot;

    let ctx = InstructionContext::new_with_admission_and_threshold(0, 10, None);
    let _ = engine.admit_outstanding_reserve_on_touch(idx, &ctx);

    // Matured monotone non-decreasing
    assert!(engine.pnl_matured_pos_tot >= matured_before);
    // Matured <= total pos
    assert!(engine.pnl_matured_pos_tot <= engine.pnl_pos_tot);
    // Vault conservation (V doesn't change)
    assert!(engine.vault.get() == v as u128);
    // V >= C_tot + I
    let senior = engine.c_tot.get() + engine.insurance_fund.balance.get();
    assert!(engine.vault.get() >= senior);
}

// ============================================================================
// IN-1: No live bypass via ImmediateReleaseResolvedOnly.
// ============================================================================

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn in1_no_live_immediate_release() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap() as usize;
    // Live mode (default on new engine)

    let new_pnl: u8 = kani::any();
    kani::assume(new_pnl > 0);

    // Snapshot state before
    let pnl_before = engine.accounts[idx].pnl;
    let pnl_pos_before = engine.pnl_pos_tot;

    let result = engine.set_pnl_with_reserve(
        idx,
        new_pnl as i128,
        ReserveMode::ImmediateReleaseResolvedOnly,
        None,
    );

    // Must fail on Live
    assert!(result.is_err());
    // State unchanged
    assert!(engine.accounts[idx].pnl == pnl_before);
    assert!(engine.pnl_pos_tot == pnl_pos_before);
}

// ============================================================================
// AH-7 (strengthened): admit_fresh_reserve_h_lock returns Err when the
// sticky list is exhausted and the admission decision requires h_max.
//
// Prevents silent-drop regression: under the pre-item-5 code the discarded
// bool from mark_h_max_sticky meant a full sticky list would leave the
// account not-recorded, and a subsequent call could re-admit at h_min
// violating the sticky-h_max invariant.
// ============================================================================

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn ah7_sticky_bitmap_is_idempotent_and_never_capacity_bound() {
    // v12.19 rev6: sticky set is now a bitmap indexed by storage slot,
    // so capacity equals MAX_ACCOUNTS and cannot be exhausted by
    // marking distinct slots. Property: mark_h_max_sticky is idempotent
    // and returns true for any in-bounds idx regardless of pre-state.
    let mut ctx = InstructionContext::new_with_admission(0, 100);

    let idx: u16 = kani::any();
    kani::assume((idx as usize) < MAX_ACCOUNTS);

    // First mark sets the bit.
    assert!(ctx.mark_h_max_sticky(idx));
    assert!(ctx.is_h_max_sticky(idx));

    // Second mark is idempotent — still true.
    assert!(ctx.mark_h_max_sticky(idx));
    assert!(ctx.is_h_max_sticky(idx));

    // A different idx does not conflict.
    let other: u16 = kani::any();
    kani::assume((other as usize) < MAX_ACCOUNTS);
    kani::assume(other != idx);
    assert!(ctx.mark_h_max_sticky(other));
    assert!(ctx.is_h_max_sticky(other));
    // Original stays set.
    assert!(ctx.is_h_max_sticky(idx));
}

// ============================================================================
// AH-8 (strengthened): admit_fresh_reserve_h_lock fail-closed on broken
// V >= C_tot + I invariant.
//
// Previous saturating_sub would silently return residual=0 when V < senior;
// checked_sub now fails with CorruptState. This proof verifies the behavior.
// ============================================================================

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn ah8_broken_conservation_fails() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap();
    // Break the conservation invariant: V < C_tot + I.
    engine.vault = U128::new(10);
    engine.c_tot = U128::new(100);
    engine.insurance_fund.balance = U128::new(0);

    let mut ctx = InstructionContext::new_with_admission(0, 100);
    let fresh: u8 = kani::any();
    kani::assume(fresh > 0);

    let r = engine.admit_fresh_reserve_h_lock(idx as usize, fresh as u128, &mut ctx, 0u64, 100u64);
    // vault.checked_sub(senior) -> None -> Err(CorruptState).
    assert!(
        r.is_err(),
        "admission MUST refuse when V < C_tot + I (broken conservation)"
    );
}

// ============================================================================
// K-9: validate_admission_pair rejects admit_h_max == 0 (Bug 9)
// Prevents wrapper bypass of admission by passing (0, 0).
// ============================================================================

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn k9_admission_pair_rejects_zero_max() {
    let engine = RiskEngine::new(zero_fee_params());
    let admit_h_min: u8 = kani::any();
    let admit_h_max = 0u64;
    let r = RiskEngine::validate_admission_pair(admit_h_min as u64, admit_h_max, &engine.params);
    assert!(r.is_err());
}

// ============================================================================
// K-1: accrue_market_to rejects dt beyond cfg_max_accrual_dt_slots (Bug 1)
// ============================================================================

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn k1_accrue_rejects_dt_over_envelope() {
    // v12.19: the dt envelope only applies when funding is actually
    // active (rate != 0 AND both sides have OI AND fund_px_last > 0).
    // Idle / zero-rate / unilateral-OI markets can fast-forward past
    // the envelope — see `idle_market_can_fast_forward_beyond_max
    // _accrual_dt`. This proof checks the funding-active branch:
    // when funding WOULD accrue, dt > cfg_max_accrual_dt_slots MUST
    // be rejected.
    let mut engine = RiskEngine::new(zero_fee_params());
    engine.oi_eff_long_q = POS_SCALE;
    engine.oi_eff_short_q = POS_SCALE;
    engine.fund_px_last = 1; // required for funding_active
    let before_slot = engine.last_market_slot;
    let before_price = engine.last_oracle_price;

    // dt > cfg_max_accrual_dt_slots
    let over: u8 = kani::any();
    let now_slot = engine
        .last_market_slot
        .saturating_add(engine.params.max_accrual_dt_slots)
        .saturating_add((over as u64).saturating_add(1));
    let oracle: u8 = kani::any();
    kani::assume(oracle > 0);

    // Nonzero rate forces funding_active; envelope MUST apply.
    let r = engine.accrue_market_to(now_slot, oracle as u64, 1i128);
    assert!(r.is_err());
    // State unchanged
    assert!(engine.last_market_slot == before_slot);
    assert!(engine.last_oracle_price == before_price);
}

// ============================================================================
// K-2: resolve_market degenerate branch bypasses dt cap (Bug 2)
// ============================================================================

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn k2_resolve_degenerate_bypasses_dt_cap() {
    let mut engine = RiskEngine::new(zero_fee_params());
    // Force dormancy past the dt cap
    let dt_over = engine.params.max_accrual_dt_slots.saturating_add(1000);
    let now_slot = engine.last_market_slot.saturating_add(dt_over);
    kani::assume(now_slot >= engine.current_slot);

    // Degenerate branch: live_oracle = P_last, rate = 0, resolved == P_last (in-band)
    let live_price = engine.last_oracle_price;
    let resolved_price = live_price;
    let rate = 0i128;

    // v12.18.5: degenerate branch is explicitly selected, not value-detected.
    let r = engine.resolve_market_not_atomic(
        ResolveMode::Degenerate,
        resolved_price,
        live_price,
        now_slot,
        rate,
    );
    assert!(r.is_ok());
    assert!(engine.market_mode == MarketMode::Resolved);
}

// ============================================================================
// K-71: neg_pnl_account_count invariant
// After any sequence of set_pnl mutations, the counter equals the actual
// number of used accounts with pnl < 0.
// ============================================================================

#[kani::proof]
#[kani::unwind(6)]
#[kani::solver(cadical)]
fn k71_neg_pnl_count_tracks_actual() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let _a = add_user_test(&mut engine, 0).unwrap();
    let _b = add_user_test(&mut engine, 0).unwrap();

    // Apply arbitrary (small) pnl mutations. set_pnl uses ImmediateReleaseResolvedOnly
    // which only works for non-positive-crossing changes on Live, so restrict
    // to decreasing/negative pnl sequences which is exactly the counter-sensitive path.
    let p1: i8 = kani::any();
    let p2: i8 = kani::any();
    let _ =
        engine.set_pnl_with_reserve(0, p1 as i128, ReserveMode::NoPositiveIncreaseAllowed, None);
    let _ =
        engine.set_pnl_with_reserve(1, p2 as i128, ReserveMode::NoPositiveIncreaseAllowed, None);

    // Count actual negative-pnl used accounts
    let mut actual = 0u64;
    for i in 0..MAX_ACCOUNTS {
        if engine.is_used(i) && engine.accounts[i].pnl < 0 {
            actual += 1;
        }
    }
    assert!(engine.neg_pnl_account_count == actual);
}

// ============================================================================
// K-201 (strengthened): keeper_crank rejects max_revalidations > MAX_TOUCHED.
// Prevents silent-clamp regression (item 9): previously requests larger than
// the finalize budget were silently clamped; now they must return Err.
// ============================================================================

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn k201_keeper_crank_rejects_oversized_budget() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let _a = add_user_test(&mut engine, 0).unwrap();
    // Symbolic over-budget request
    let over: u8 = kani::any();
    kani::assume(over > 0);
    let req = (MAX_TOUCHED_PER_INSTRUCTION as u16).saturating_add(over as u16);

    let r = engine.keeper_crank_not_atomic(
        DEFAULT_SLOT,
        DEFAULT_ORACLE,
        &[],
        req,
        0i128,
        0,
        100,
        None,
        0,
    );
    assert!(
        r.is_err(),
        "max_revalidations > MAX_TOUCHED_PER_INSTRUCTION MUST reject, not clamp"
    );
}

// ============================================================================
// K-202 (strengthened): public postcondition fires on broken conservation.
// Exercises the defense-in-depth assert_public_postconditions (item 7).
// ============================================================================

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn k202_postcondition_detects_broken_conservation() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let _a = add_user_test(&mut engine, 0).unwrap();
    // Forcibly break conservation: inflate c_tot past vault.
    engine.c_tot = U128::new(10_000);
    engine.vault = U128::new(5_000);
    assert!(!engine.check_conservation());

    // Any public entrypoint must fail via postcondition check.
    let r = engine.keeper_crank_not_atomic(
        DEFAULT_SLOT,
        DEFAULT_ORACLE,
        &[],
        0,
        0i128,
        0,
        100,
        None,
        0,
    );
    assert!(
        r.is_err(),
        "broken conservation MUST surface as Err from a public entrypoint"
    );
}

// ============================================================================
// AC-5 (strengthened): admit_outstanding_reserve_on_touch is atomic on Err.
// If the pre-commit global-invariant check (new_matured > pnl_pos_tot)
// fires, no reserve bucket nor aggregate has been mutated.
// ============================================================================

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn ac5_admit_outstanding_atomic_on_err() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap() as usize;

    // Plenty of residual so admission chooses to accelerate.
    engine.vault = U128::new(10_000);
    engine.c_tot = U128::new(0);
    // Put the account in a state where acceleration would trigger but
    // pnl_matured_pos_tot + reserve_total > pnl_pos_tot (invariant break).
    let r: u8 = kani::any();
    kani::assume(r > 0);
    engine.accounts[idx].reserved_pnl = r as u128;
    engine.accounts[idx].pnl = r as i128;
    engine.pnl_pos_tot = r as u128; // exact; matured + r > r → must fail
    engine.pnl_matured_pos_tot = 1;
    engine.accounts[idx].sched_present = 1;
    engine.accounts[idx].sched_remaining_q = r as u128;
    engine.accounts[idx].sched_anchor_q = r as u128;
    engine.accounts[idx].sched_horizon = 10;

    // Snapshot
    let reserved_before = engine.accounts[idx].reserved_pnl;
    let sched_remaining_before = engine.accounts[idx].sched_remaining_q;
    let sched_present_before = engine.accounts[idx].sched_present;
    let matured_before = engine.pnl_matured_pos_tot;

    let ctx = InstructionContext::new_with_admission_and_threshold(0, 10, None);
    let result = engine.admit_outstanding_reserve_on_touch(idx, &ctx);

    // Deterministic setup: matured=1, reserve=r, pnl_pos_tot=r forces
    // new_matured = 1+r > pnl_pos_tot = r → invariant check returns Err.
    // Asserting Err unconditionally (not `if result.is_err()`) avoids
    // vacuous pass if the result were Ok.
    assert!(
        result.is_err(),
        "atomicity check MUST fire: new_matured > pnl_pos_tot"
    );
    // And state MUST be unchanged (validate-then-mutate contract).
    assert!(engine.accounts[idx].reserved_pnl == reserved_before);
    assert!(engine.accounts[idx].sched_remaining_q == sched_remaining_before);
    assert!(engine.accounts[idx].sched_present == sched_present_before);
    assert!(engine.pnl_matured_pos_tot == matured_before);
}

// ============================================================================
// AC-6: Outstanding reserve acceleration is policy-gated.
// ============================================================================

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn ac6_outstanding_acceleration_blocked_by_nonzero_hmin() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap() as usize;

    let r: u8 = kani::any();
    kani::assume(r > 0);
    let h_min: u8 = kani::any();
    kani::assume(h_min > 0);
    kani::assume((h_min as u64) <= engine.params.h_max);

    engine.vault = U128::new((r as u128) + 100);
    engine.c_tot = U128::new(0);
    engine.accounts[idx].reserved_pnl = r as u128;
    engine.accounts[idx].pnl = r as i128;
    engine.pnl_pos_tot = r as u128;
    engine.accounts[idx].sched_present = 1;
    engine.accounts[idx].sched_remaining_q = r as u128;
    engine.accounts[idx].sched_anchor_q = r as u128;
    engine.accounts[idx].sched_horizon = engine.params.h_max;

    let reserved_before = engine.accounts[idx].reserved_pnl;
    let matured_before = engine.pnl_matured_pos_tot;
    let sched_present_before = engine.accounts[idx].sched_present;
    let ctx = InstructionContext::new_with_admission_and_threshold(h_min as u64, 10, None);

    let result = engine.admit_outstanding_reserve_on_touch(idx, &ctx);
    assert!(result.is_ok(), "valid gated reserve state must not reject");
    assert!(
        engine.accounts[idx].reserved_pnl == reserved_before,
        "nonzero admit_h_min must block outstanding reserve acceleration"
    );
    assert!(engine.pnl_matured_pos_tot == matured_before);
    assert!(engine.accounts[idx].sched_present == sched_present_before);
}

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn ac7_outstanding_acceleration_blocked_by_active_threshold() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap() as usize;

    let r: u8 = kani::any();
    kani::assume(r > 0);
    let threshold: u8 = kani::any();
    kani::assume(threshold > 0);
    let consumed: u8 = kani::any();
    kani::assume(consumed >= threshold);

    engine.vault = U128::new((r as u128) + 100);
    engine.c_tot = U128::new(0);
    engine.accounts[idx].reserved_pnl = r as u128;
    engine.accounts[idx].pnl = r as i128;
    engine.pnl_pos_tot = r as u128;
    engine.accounts[idx].sched_present = 1;
    engine.accounts[idx].sched_remaining_q = r as u128;
    engine.accounts[idx].sched_anchor_q = r as u128;
    engine.accounts[idx].sched_horizon = engine.params.h_max;
    engine.stress_consumed_bps_e9_since_envelope = (consumed as u128) * STRESS_CONSUMPTION_SCALE;

    let reserved_before = engine.accounts[idx].reserved_pnl;
    let matured_before = engine.pnl_matured_pos_tot;
    let sched_present_before = engine.accounts[idx].sched_present;
    let ctx = InstructionContext::new_with_admission_and_threshold(0, 10, Some(threshold as u128));

    let result = engine.admit_outstanding_reserve_on_touch(idx, &ctx);
    assert!(result.is_ok(), "valid gated reserve state must not reject");
    assert!(
        engine.accounts[idx].reserved_pnl == reserved_before,
        "active threshold gate must block outstanding reserve acceleration"
    );
    assert!(engine.pnl_matured_pos_tot == matured_before);
    assert!(engine.accounts[idx].sched_present == sched_present_before);
}

// ============================================================================
// RS-1 (strengthened): reserve validation rejects reserved_pnl > max(pnl, 0).
// Prevents corrupt accounts with reserve exceeding positive PnL from being
// processed by downstream helpers.
// ============================================================================

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn rs1_validate_rejects_reserved_exceeding_pos_pnl() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap() as usize;

    // Set up a valid sched bucket but with reserved_pnl > pnl.
    let bad_reserve: u8 = kani::any();
    kani::assume(bad_reserve > 0);
    engine.accounts[idx].pnl = 0; // zero pnl
    engine.accounts[idx].reserved_pnl = bad_reserve as u128;
    engine.accounts[idx].sched_present = 1;
    engine.accounts[idx].sched_remaining_q = bad_reserve as u128;
    engine.accounts[idx].sched_anchor_q = bad_reserve as u128;
    engine.accounts[idx].sched_horizon = engine.params.h_max; // valid horizon

    // append_or_route validates shape at entry — MUST reject the corrupt state.
    let r = engine.append_or_route_new_reserve(idx, 100, 100, 10);
    assert!(
        r.is_err(),
        "reserved_pnl > max(pnl, 0) MUST be rejected (spec §2.1)"
    );
}

// ============================================================================
// RS-2 (strengthened): admit_outstanding_reserve_on_touch rejects bucket
// sum mismatch instead of laundering corruption into matured.
// Reviewer's Test A.
// ============================================================================

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn rs2_admit_outstanding_rejects_bucket_sum_mismatch() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap() as usize;

    // Healthy residual (would admit if state were valid).
    engine.vault = U128::new(10_000);
    engine.c_tot = U128::new(0);

    // Corrupt: reserved_pnl = 1 but sched_remaining_q = 10 (mismatch).
    engine.accounts[idx].pnl = 10;
    engine.pnl_pos_tot = 10;
    engine.accounts[idx].reserved_pnl = 1;
    engine.accounts[idx].sched_present = 1;
    engine.accounts[idx].sched_remaining_q = 10;
    engine.accounts[idx].sched_anchor_q = 10;
    engine.accounts[idx].sched_horizon = engine.params.h_max;

    let matured_before = engine.pnl_matured_pos_tot;
    let reserved_before = engine.accounts[idx].reserved_pnl;
    let sched_present_before = engine.accounts[idx].sched_present;

    let ctx = InstructionContext::new_with_admission_and_threshold(0, 10, None);
    let r = engine.admit_outstanding_reserve_on_touch(idx, &ctx);
    assert!(r.is_err(), "bucket-sum mismatch MUST reject");
    // No state change.
    assert!(engine.pnl_matured_pos_tot == matured_before);
    assert!(engine.accounts[idx].reserved_pnl == reserved_before);
    assert!(engine.accounts[idx].sched_present == sched_present_before);
}

// ============================================================================
// RS-3 (strengthened): apply_reserve_loss_newest_first rejects malformed
// queue state. Reviewer's Test D.
// ============================================================================

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn rs3_apply_reserve_loss_rejects_malformed_queue() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap() as usize;

    // Corrupt: sched_present=1 but reserved_pnl doesn't match queue sums.
    engine.accounts[idx].pnl = 10;
    engine.pnl_pos_tot = 10;
    engine.accounts[idx].reserved_pnl = 5;
    engine.accounts[idx].sched_present = 1;
    engine.accounts[idx].sched_remaining_q = 10; // mismatch: sum=10 != R=5
    engine.accounts[idx].sched_anchor_q = 10;
    engine.accounts[idx].sched_horizon = engine.params.h_max;

    let reserved_before = engine.accounts[idx].reserved_pnl;
    let sched_remaining_before = engine.accounts[idx].sched_remaining_q;

    let r = engine.apply_reserve_loss_newest_first(idx, 1);
    assert!(r.is_err(), "malformed queue MUST reject");
    // No state change.
    assert!(engine.accounts[idx].reserved_pnl == reserved_before);
    assert!(engine.accounts[idx].sched_remaining_q == sched_remaining_before);
}

// ============================================================================
// RS-4 (strengthened): advance_profit_warmup validates BEFORE pending→sched
// promotion. Pending fields with malformed horizon must fail before being
// copied into the scheduled bucket.
// ============================================================================

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn rs4_warmup_rejects_malformed_pending_before_promotion() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap() as usize;

    // Corrupt pending: horizon out of [h_min, h_max] range.
    engine.accounts[idx].pnl = 5;
    engine.pnl_pos_tot = 5;
    engine.accounts[idx].reserved_pnl = 5;
    engine.accounts[idx].pending_present = 1;
    engine.accounts[idx].pending_remaining_q = 5;
    engine.accounts[idx].pending_horizon = engine.params.h_max + 1; // OOB

    let r = engine.advance_profit_warmup(idx);
    assert!(
        r.is_err(),
        "malformed pending_horizon MUST reject before promotion"
    );
    // Pending must NOT have been promoted into sched.
    assert!(engine.accounts[idx].sched_present == 0);
    assert!(engine.accounts[idx].pending_present == 1);
}

// ============================================================================
// K-104: OI >= sum of effective positions per side
// ============================================================================

#[kani::proof]
#[kani::unwind(6)]
#[kani::solver(cadical)]
fn k104_oi_geq_sum_of_effective() {
    let mut engine = RiskEngine::new(zero_fee_params());
    // Fresh engine: both OI and per-account eff are 0
    let mut sum_long: u128 = 0;
    let mut sum_short: u128 = 0;
    for i in 0..MAX_ACCOUNTS {
        if engine.is_used(i) {
            let eff = engine.effective_pos_q(i);
            if eff > 0 {
                sum_long = sum_long.saturating_add(eff as u128);
            } else if eff < 0 {
                sum_short = sum_short.saturating_add(eff.unsigned_abs());
            }
        }
    }
    assert!(engine.oi_eff_long_q >= sum_long);
    assert!(engine.oi_eff_short_q >= sum_short);
    // Also verify bilateral invariant
    assert!(engine.oi_eff_long_q == engine.oi_eff_short_q);
    let _ = &mut engine; // avoid unused warning
}

// ============================================================================
// v12.19 admission-gate proofs (spec §4.7 step 2)
// Priority #3 from rev6 plan:
//   - gate_stress_lane: Some(t) + consumption>=t forces admit_h_max
//   - gate_none_recovers: None disables step 2 entirely
//   - gate_some_zero_rejected: Some(0) is invalid input
//   - gate_sticky_skips: sticky early-return bypasses step 2
// ============================================================================

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn v19_admit_gate_stress_lane_forces_h_max() {
    // Property 99: when threshold_opt = Some(threshold) and
    // stress_consumed_bps_e9_since_envelope >= threshold,
    // admit_fresh_reserve_h_lock returns admit_h_max regardless of any
    // choice of Residual_now and matured_plus_fresh.
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap();

    // Symbolic state.
    let fresh: u8 = kani::any();
    kani::assume(fresh > 0);

    // Symbolic vault/c_tot cover both residual-ample and residual-scarce cases.
    let vault: u8 = kani::any();
    let c_tot: u8 = kani::any();
    kani::assume(c_tot as u128 <= vault as u128);
    engine.vault = U128::new(vault as u128);
    engine.c_tot = U128::new(c_tot as u128);

    let threshold: u8 = kani::any();
    kani::assume(threshold > 0);
    let consumed: u8 = kani::any();
    kani::assume(consumed >= threshold);
    engine.stress_consumed_bps_e9_since_envelope = (consumed as u128) * STRESS_CONSUMPTION_SCALE;

    let admit_h_max: u64 = 50;
    let mut ctx = InstructionContext::new_with_admission_and_threshold(
        0,
        admit_h_max,
        Some(threshold as u128),
    );

    let h = engine
        .admit_fresh_reserve_h_lock(idx as usize, fresh as u128, &mut ctx, 0, admit_h_max)
        .unwrap();
    assert_eq!(
        h, admit_h_max,
        "consumption-threshold gate must force admit_h_max"
    );
}

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn v19_admit_gate_none_disables_step2() {
    // Property 101 first clause: None disables the gate. Result matches
    // pre-v12.19 behavior — determined solely by residual-scarcity check.
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap();

    let vault: u8 = kani::any();
    let c_tot: u8 = kani::any();
    kani::assume(c_tot as u128 <= vault as u128);
    engine.vault = U128::new(vault as u128);
    engine.c_tot = U128::new(c_tot as u128);

    let fresh: u8 = kani::any();
    kani::assume(fresh > 0);

    // Any consumption — gate is disabled so it cannot affect the outcome.
    engine.stress_consumed_bps_e9_since_envelope = kani::any();

    let admit_h_max: u64 = 50;
    let mut ctx = InstructionContext::new_with_admission_and_threshold(0, admit_h_max, None);

    let h = engine
        .admit_fresh_reserve_h_lock(idx as usize, fresh as u128, &mut ctx, 0, admit_h_max)
        .unwrap();

    // Expected result from pure residual lane.
    let senior = engine.c_tot.get() + engine.insurance_fund.balance.get();
    let residual = engine.vault.get().saturating_sub(senior);
    let matured_plus_fresh = engine.pnl_matured_pos_tot.saturating_add(fresh as u128);
    let expected = if matured_plus_fresh <= residual {
        0
    } else {
        admit_h_max
    };

    assert_eq!(
        h, expected,
        "None-threshold path must equal pure residual-scarcity lane"
    );
}

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn v19_admit_gate_some_zero_rejected() {
    // Property 101 second clause: Some(0) is invalid at validation time.
    let r = RiskEngine::validate_threshold_opt(Some(0));
    assert_eq!(r, Err(RiskError::Overflow));
    // None and any positive threshold accepted.
    assert!(RiskEngine::validate_threshold_opt(None).is_ok());
    let t: u128 = kani::any();
    kani::assume(t > 0);
    kani::assume(t <= u128::MAX / STRESS_CONSUMPTION_SCALE);
    assert!(RiskEngine::validate_threshold_opt(Some(t)).is_ok());
}

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn v19_admit_gate_sticky_early_return() {
    // Step 1 of §4.7: once an account is in h_max_sticky_accounts, the
    // function returns admit_h_max immediately regardless of step 2 or 3.
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = add_user_test(&mut engine, 0).unwrap();
    engine.vault = U128::new(100);

    let admit_h_max: u64 = 50;
    let mut ctx = InstructionContext::new_with_admission_and_threshold(0, admit_h_max, None);

    // Pre-populate sticky.
    assert!(ctx.mark_h_max_sticky(idx));

    let fresh: u8 = kani::any();
    kani::assume(fresh > 0);
    // Symbolic consumption / threshold — irrelevant due to sticky early-return.
    engine.stress_consumed_bps_e9_since_envelope = kani::any();

    let h = engine
        .admit_fresh_reserve_h_lock(idx as usize, fresh as u128, &mut ctx, 0, admit_h_max)
        .unwrap();
    assert_eq!(h, admit_h_max, "sticky must force admit_h_max");
}

// ============================================================================
// v12.19 consumption-accumulator proofs (spec §5.5 step 9a)
// Property 105: consumption is floor-rounded at scaled-bps precision.
// ============================================================================

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn v19_consumption_monotone_within_generation() {
    // Property 97: stress_consumed_bps_e9_since_envelope is monotone
    // nondecreasing within a generation. Two successive envelope-valid
    // accrue_market_to calls cannot decrement the accumulator; both
    // contribute floor(|ΔP| * 10_000 * STRESS_CONSUMPTION_SCALE / P_last) >= 0.
    let mut engine = RiskEngine::new(zero_fee_params());
    engine.oi_eff_long_q = 1_000_000;
    engine.oi_eff_short_q = 1_000_000;
    engine.last_oracle_price = 100_000;
    engine.fund_px_last = 100_000;
    engine.last_market_slot = 0;
    engine.adl_mult_long = ADL_ONE;
    engine.adl_mult_short = ADL_ONE;

    // Symbolic starting consumption.
    let start: u8 = kani::any();
    engine.stress_consumed_bps_e9_since_envelope = start as u128;
    let gen_start = engine.sweep_generation;

    // Symbolic price move within cap (max_price_move=4 bps/slot * dt=1
    // * P=100_000 = 400_000; LHS at abs_dp=40 is 400_000 = cap).
    let dp1: u8 = kani::any();
    kani::assume(dp1 <= 40);
    if dp1 > 0 {
        let _ = engine.accrue_market_to(1, 100_000 + dp1 as u64, 0);
    }
    let mid = engine.stress_consumed_bps_e9_since_envelope;

    // Second envelope-valid move within same generation.
    let dp2: u8 = kani::any();
    kani::assume(dp2 <= 40);
    // After first move, new P_last = 100_000 + dp1, new cap base = that,
    // new last_market_slot = 1 (if dp1>0). Use dt=1 again.
    if dp2 > 0 && engine.last_market_slot == 1 {
        let new_p = engine
            .last_oracle_price
            .checked_add(dp2 as u64)
            .unwrap_or(u64::MAX);
        let _ = engine.accrue_market_to(2, new_p, 0);
    }
    let after = engine.stress_consumed_bps_e9_since_envelope;

    // Monotone: neither call can decrement the accumulator.
    assert!(
        mid >= start as u128,
        "first accrual cannot decrement consumption"
    );
    assert!(after >= mid, "second accrual cannot decrement consumption");
    // Generation did not change (no Phase 2 wrap involved).
    assert_eq!(
        engine.sweep_generation, gen_start,
        "generation must be stable within a bounded-consumption interval"
    );
}

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn v19_consumption_floor_below_one_bp() {
    let mut engine = RiskEngine::new(zero_fee_params());
    engine.oi_eff_long_q = 1_000_000; // both sides live
    engine.oi_eff_short_q = 1_000_000;

    let p_last = 100_000u64;
    engine.last_oracle_price = p_last;
    engine.fund_px_last = p_last;
    engine.last_market_slot = 0;

    let abs_dp: u8 = kani::any();
    kani::assume(abs_dp > 0);
    kani::assume(abs_dp <= 40);

    let expected = (abs_dp as u128) * 10_000 * STRESS_CONSUMPTION_SCALE / (p_last as u128);
    let r = engine.accrue_market_to(1, p_last + abs_dp as u64, 0);
    assert!(r.is_ok());
    assert_eq!(
        engine.stress_consumed_bps_e9_since_envelope, expected,
        "consumption must use floor at scaled-bps precision"
    );
}

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn v19_funding_consumption_accumulates_scaled_bps() {
    let mut engine = RiskEngine::new(zero_fee_params());
    engine.oi_eff_long_q = 1_000_000;
    engine.oi_eff_short_q = 1_000_000;
    engine.last_oracle_price = DEFAULT_ORACLE;
    engine.fund_px_last = DEFAULT_ORACLE;
    engine.last_market_slot = 0;
    engine.adl_mult_long = ADL_ONE;
    engine.adl_mult_short = ADL_ONE;

    let rate: u8 = kani::any();
    kani::assume(rate > 0);
    kani::assume((rate as u64) <= engine.params.max_abs_funding_e9_per_slot);

    let r = engine.accrue_market_to(1, DEFAULT_ORACLE, rate as i128);
    assert!(r.is_ok());
    assert_eq!(
        engine.stress_consumed_bps_e9_since_envelope,
        (rate as u128) * 10_000u128,
        "funding stress must accumulate abs(rate_e9) * dt * 10_000"
    );
    assert_eq!(
        engine.stress_envelope_remaining_indices,
        engine.params.max_accounts
    );
}

// ============================================================================
// v12.19 cursor / generation state-machine proofs (spec §9.7 Phase 2)
// ============================================================================

#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn v19_rr_touch_zero_no_cursor_advance() {
    // Property 98: rr_touch_limit = 0 does not mutate cursor, generation,
    // or consumption accumulator.
    let mut engine = RiskEngine::new_with_market(zero_fee_params(), 1, DEFAULT_ORACLE);
    let cursor: u8 = kani::any();
    let generation_before: u8 = kani::any();
    let consumed_before: u8 = kani::any();
    kani::assume((cursor as u64) < engine.params.max_accounts);
    engine.rr_cursor_position = cursor as u64;
    engine.sweep_generation = generation_before as u64;
    if consumed_before > 0 {
        let max_accounts = engine.params.max_accounts;
        seed_active_stress_envelope(&mut engine, consumed_before as u128, 1, max_accounts);
    }

    let r = engine.keeper_crank_not_atomic(1, DEFAULT_ORACLE, &[], 0, 0, 1, 100, None, 0);
    assert!(r.is_ok());
    assert_eq!(engine.rr_cursor_position, cursor as u64);
    assert_eq!(engine.sweep_generation, generation_before as u64);
    assert_eq!(
        engine.stress_consumed_bps_e9_since_envelope,
        consumed_before as u128
    );
}

#[kani::proof]
#[kani::unwind(10)]
#[kani::solver(cadical)]
fn v19_greedy_phase2_model_respects_touch_budget_and_bounds() {
    // Small spec model for greedy Phase 2. It skips unused slots, touches at
    // most rr_touch_limit used slots, and never scans outside the sweep limit.
    let cursor: u8 = kani::any();
    let rr_touch_limit: u8 = kani::any();
    let used_mask: u8 = kani::any();
    let sweep_limit = 8u64;
    kani::assume((cursor as u64) < sweep_limit);
    kani::assume(rr_touch_limit <= 4);

    let mut i = cursor as u64;
    let mut touched = 0u64;
    while i < sweep_limit && touched < rr_touch_limit as u64 {
        let used = ((used_mask >> (i as u32)) & 1) != 0;
        if used {
            touched += 1;
        }
        i += 1;
    }
    let cursor_after = if i >= sweep_limit { 0 } else { i };

    assert!(touched <= rr_touch_limit as u64);
    assert!(i <= sweep_limit);
    assert!(cursor_after < sweep_limit);
    if rr_touch_limit == 0 {
        assert_eq!(cursor_after, cursor as u64);
        assert_eq!(touched, 0);
    }
}

#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn v19_same_slot_stress_wrap_defers_generation_reset() {
    let mut engine = RiskEngine::new_with_market(zero_fee_params(), 1, DEFAULT_ORACLE);
    let generation_before: u8 = kani::any();
    let consumed_before: u8 = kani::any();
    kani::assume(consumed_before > 0);

    engine.rr_cursor_position = engine.params.max_accounts - 1;
    engine.sweep_generation = generation_before as u64;
    seed_active_stress_envelope(&mut engine, consumed_before as u128, 1, 1);

    let r = engine.keeper_crank_not_atomic(1, DEFAULT_ORACLE, &[], 0, 0, 1, 100, None, 1);
    assert!(r.is_ok());
    assert_eq!(engine.rr_cursor_position, engine.params.max_accounts - 1);
    assert_eq!(engine.sweep_generation, generation_before as u64);
    assert_eq!(
        engine.stress_consumed_bps_e9_since_envelope,
        consumed_before as u128
    );
    assert_eq!(engine.stress_envelope_remaining_indices, 1);
}

#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn v19_stress_envelope_clear_requires_later_wrap() {
    let mut engine = RiskEngine::new_with_market(zero_fee_params(), 1, DEFAULT_ORACLE);
    let generation_before: u8 = kani::any();
    let consumed_before: u8 = kani::any();
    kani::assume(consumed_before > 0);

    engine.sweep_generation = generation_before as u64;
    seed_active_stress_envelope(&mut engine, consumed_before as u128, 1, 1);

    let no_wrap = engine.keeper_crank_not_atomic(2, DEFAULT_ORACLE, &[], 0, 0, 1, 100, None, 0);
    assert!(no_wrap.is_ok());
    assert_eq!(engine.sweep_generation, generation_before as u64);
    assert_eq!(
        engine.stress_consumed_bps_e9_since_envelope,
        consumed_before as u128
    );
    assert_eq!(engine.stress_envelope_remaining_indices, 1);

    engine.rr_cursor_position = engine.params.max_accounts - 1;
    let wrap = engine.keeper_crank_not_atomic(2, DEFAULT_ORACLE, &[], 0, 0, 1, 100, None, 1);
    assert!(wrap.is_ok());
    assert_eq!(engine.sweep_generation, generation_before as u64 + 1);
    assert_eq!(engine.stress_consumed_bps_e9_since_envelope, 0);
    assert_eq!(engine.stress_envelope_remaining_indices, 0);
    assert_eq!(engine.last_sweep_generation_advance_slot, 2);
}

#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn v19_generation_advances_at_most_once_per_slot() {
    let mut params = zero_fee_params();
    params.max_accounts = 2;
    params.max_active_positions_per_side = 2;
    let mut engine = RiskEngine::new_with_market(params, 1, DEFAULT_ORACLE);
    let generation_before: u8 = kani::any();
    engine.sweep_generation = generation_before as u64;
    engine.rr_cursor_position = 1;

    let first = engine.keeper_crank_not_atomic(1, DEFAULT_ORACLE, &[], 0, 0, 1, 100, None, 1);
    assert!(first.is_ok());
    let after_first = engine.sweep_generation;
    assert_eq!(after_first, generation_before as u64 + 1);

    let second = engine.keeper_crank_not_atomic(1, DEFAULT_ORACLE, &[], 0, 0, 1, 100, None, 1);
    assert!(second.is_ok());
    assert_eq!(engine.sweep_generation, after_first);
    assert_eq!(engine.last_sweep_generation_advance_slot, 1);
}

// ============================================================================
// v12.19 atomicity rollback proofs (spec §5.5 and §9.7 footer notes)
// Priority #6 from rev6 plan.
// ============================================================================

#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(cadical)]
fn v19_accrual_consumption_only_commits_on_success() {
    // Spec §5.5 step 9a footer: if a later leg of accrue_market_to fails
    // (e.g. K/F overflow), stress_consumed_bps_e9_since_envelope is NOT
    // incremented — it is committed only after all other state commits.
    //
    // Setup: dt=1 with a move large enough that consumed_this_step > 0
    // (so we can witness non-rollback as a bug), and K near i128::MAX so
    // the mark-to-market step overflows.
    let mut engine = RiskEngine::new(zero_fee_params());
    engine.oi_eff_long_q = 1_000_000;
    engine.oi_eff_short_q = 1_000_000;
    // P_last = 10_000. Move to 10_000 + 1 gives abs_dp*10_000 = 10_000,
    // floor(10_000 * 1e9 / 10_000) = 1e9 bps-e9 consumed. Cap at dt=1,
    // P=10_000 is 4 * 1 * 10_000 = 40_000 >= 10_000, so step 9 passes.
    engine.last_oracle_price = 10_000;
    engine.fund_px_last = 10_000;
    engine.last_market_slot = 0;
    // K near i128::MAX so mark delta = ADL_ONE * 1 = 1e15 overflows.
    engine.adl_coeff_long = i128::MAX - 1;
    engine.adl_mult_long = ADL_ONE;
    engine.adl_mult_short = ADL_ONE;

    // Prime consumption to a known non-trivial value so rollback is
    // observable (no accidental "0 + 0 = 0" trivial truth).
    seed_active_stress_envelope(&mut engine, 17, 0, 1);

    let consumed_before = engine.stress_consumed_bps_e9_since_envelope;
    let k_long_before = engine.adl_coeff_long;
    let p_last_before = engine.last_oracle_price;
    let slot_before = engine.last_market_slot;

    let r = engine.accrue_market_to(1, 10_001, 0);
    assert!(r.is_err(), "K overflow must reject the accrual");

    // All persistent state (including consumption) must have rolled back.
    assert_eq!(
        engine.stress_consumed_bps_e9_since_envelope, consumed_before,
        "stress consumption must roll back atomically with K/F commit"
    );
    assert_eq!(engine.adl_coeff_long, k_long_before);
    assert_eq!(engine.last_oracle_price, p_last_before);
    assert_eq!(engine.last_market_slot, slot_before);
}
