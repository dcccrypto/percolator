//! Section 7 — Liveness, progress, no-deadlock
//!
//! Auto-finalization, trade reopening, ADL fallback routes,
//! precision exhaustion, crank quiescence, drain-only progress.

#![cfg(kani)]

mod common;
use common::*;

#[derive(Clone, Copy)]
struct HonestCrankWorkModel {
    catchup_lag: u8,
    cursor_remaining: u8,
    stress_remaining: u8,
    reset_stale_remaining: u8,
    b_remaining: u8,
    close_chunk_remaining: u8,
    resolved_blockers: u8,
    segment_blocked: bool,
}

#[derive(Clone, Copy)]
struct HonestCrankProfileModel {
    scan_budget: u8,
    touch_budget: u8,
    b_budget: u8,
    close_budget: u8,
    resolved_budget: u8,
    recovery_enabled: bool,
}

#[derive(Clone, Copy)]
struct HonestCrankStepModel {
    outcome: u8,
    next: HonestCrankWorkModel,
}

const MODEL_OUTCOME_STUCK: u8 = 0;
const MODEL_OUTCOME_QUIESCENT: u8 = 1;
const MODEL_OUTCOME_PROGRESS: u8 = 2;
const MODEL_OUTCOME_RECOVERY: u8 = 3;

fn honest_crank_valid_public_profile(profile: HonestCrankProfileModel) -> bool {
    profile.scan_budget > 0
        && profile.touch_budget > 0
        && profile.b_budget > 0
        && profile.close_budget > 0
        && profile.resolved_budget > 0
        && profile.recovery_enabled
}

fn honest_crank_work_sum(work: HonestCrankWorkModel) -> u16 {
    work.catchup_lag as u16
        + work.cursor_remaining as u16
        + work.stress_remaining as u16
        + work.reset_stale_remaining as u16
        + work.b_remaining as u16
        + work.close_chunk_remaining as u16
        + work.resolved_blockers as u16
}

fn honest_crank_step_model(
    work: HonestCrankWorkModel,
    profile: HonestCrankProfileModel,
) -> HonestCrankStepModel {
    let mut next = work;
    if work.segment_blocked {
        return HonestCrankStepModel {
            outcome: if profile.recovery_enabled {
                MODEL_OUTCOME_RECOVERY
            } else {
                MODEL_OUTCOME_STUCK
            },
            next,
        };
    }

    if work.catchup_lag > 0 {
        if profile.scan_budget == 0 || profile.touch_budget == 0 {
            return HonestCrankStepModel {
                outcome: MODEL_OUTCOME_STUCK,
                next,
            };
        }
        next.catchup_lag -= 1;
        if next.cursor_remaining > 0 {
            next.cursor_remaining = next.cursor_remaining.saturating_sub(profile.scan_budget);
        }
        return HonestCrankStepModel {
            outcome: MODEL_OUTCOME_PROGRESS,
            next,
        };
    }
    if work.cursor_remaining > 0 {
        if profile.scan_budget == 0 {
            return HonestCrankStepModel {
                outcome: MODEL_OUTCOME_STUCK,
                next,
            };
        }
        next.cursor_remaining = next.cursor_remaining.saturating_sub(profile.scan_budget);
        return HonestCrankStepModel {
            outcome: MODEL_OUTCOME_PROGRESS,
            next,
        };
    }
    if work.stress_remaining > 0 {
        if profile.scan_budget == 0 {
            return HonestCrankStepModel {
                outcome: MODEL_OUTCOME_STUCK,
                next,
            };
        }
        next.stress_remaining = next.stress_remaining.saturating_sub(profile.scan_budget);
        return HonestCrankStepModel {
            outcome: MODEL_OUTCOME_PROGRESS,
            next,
        };
    }
    if work.reset_stale_remaining > 0 {
        if profile.touch_budget == 0 {
            return HonestCrankStepModel {
                outcome: MODEL_OUTCOME_STUCK,
                next,
            };
        }
        next.reset_stale_remaining = next
            .reset_stale_remaining
            .saturating_sub(profile.touch_budget);
        return HonestCrankStepModel {
            outcome: MODEL_OUTCOME_PROGRESS,
            next,
        };
    }
    if work.b_remaining > 0 {
        if profile.b_budget == 0 {
            return HonestCrankStepModel {
                outcome: MODEL_OUTCOME_STUCK,
                next,
            };
        }
        next.b_remaining = next.b_remaining.saturating_sub(profile.b_budget);
        return HonestCrankStepModel {
            outcome: MODEL_OUTCOME_PROGRESS,
            next,
        };
    }
    if work.close_chunk_remaining > 0 {
        if profile.close_budget == 0 {
            return HonestCrankStepModel {
                outcome: MODEL_OUTCOME_STUCK,
                next,
            };
        }
        next.close_chunk_remaining = next
            .close_chunk_remaining
            .saturating_sub(profile.close_budget);
        return HonestCrankStepModel {
            outcome: MODEL_OUTCOME_PROGRESS,
            next,
        };
    }
    if work.resolved_blockers > 0 {
        if profile.resolved_budget == 0 {
            return HonestCrankStepModel {
                outcome: MODEL_OUTCOME_STUCK,
                next,
            };
        }
        next.resolved_blockers = next
            .resolved_blockers
            .saturating_sub(profile.resolved_budget);
        return HonestCrankStepModel {
            outcome: MODEL_OUTCOME_PROGRESS,
            next,
        };
    }

    HonestCrankStepModel {
        outcome: MODEL_OUTCOME_QUIESCENT,
        next,
    }
}

// ============================================================================
// T11.43: end_instruction_auto_finalizes_ready_side
// ============================================================================

#[kani::proof]
#[kani::solver(cadical)]
fn t11_43_end_instruction_auto_finalizes_ready_side() {
    let mut engine = RiskEngine::new(zero_fee_params());

    engine.side_mode_long = SideMode::ResetPending;
    engine.oi_eff_long_q = 0u128;
    engine.stale_account_count_long = 0;
    engine.stored_pos_count_long = 0;

    engine.side_mode_short = SideMode::ResetPending;
    engine.oi_eff_short_q = 0u128;
    engine.stale_account_count_short = 1;
    engine.stored_pos_count_short = 0;

    let ctx = InstructionContext::new();
    engine.finalize_end_of_instruction_resets(&ctx);

    assert!(
        engine.side_mode_long == SideMode::Normal,
        "ready ResetPending side must auto-finalize to Normal"
    );
    assert!(
        engine.side_mode_short == SideMode::ResetPending,
        "non-ready side must stay ResetPending"
    );
}

// ============================================================================
// T11.44: trade_path_reopens_ready_reset_side
// ============================================================================

#[kani::proof]
#[kani::solver(cadical)]
fn t11_44_trade_path_reopens_ready_reset_side() {
    let mut engine = RiskEngine::new(zero_fee_params());

    engine.side_mode_long = SideMode::ResetPending;
    engine.oi_eff_long_q = 0u128;
    engine.oi_eff_short_q = 0u128;
    engine.stale_account_count_long = 0;
    engine.stored_pos_count_long = 0;

    let size_q = POS_SCALE as i128;
    let old_a = 0i128;
    let old_b = 0i128;
    let new_a = size_q;
    let new_b = -size_q;
    let (oi_long_after, oi_short_after) = engine
        .bilateral_oi_after(&old_a, &new_a, &old_b, &new_b)
        .unwrap();

    assert!(
        engine
            .enforce_side_mode_oi_gate(old_a, new_a, old_b, new_b, oi_long_after, oi_short_after)
            .is_err(),
        "ready ResetPending side must block OI increase before preflight finalization"
    );

    engine.maybe_finalize_ready_reset_sides();

    assert!(engine.side_mode_long == SideMode::Normal);
    assert!(
        engine
            .enforce_side_mode_oi_gate(old_a, new_a, old_b, new_b, oi_long_after, oi_short_after)
            .is_ok(),
        "trade preflight must reopen a fully ready ResetPending side before OI gating"
    );
    assert!(oi_long_after == oi_short_after);
}

// ============================================================================
// T11.45: try_negate_u256_correctness
// ============================================================================
// NOTE: try_negate_u256_to_i256 has been removed from the engine after the
// migration to native 128-bit types. This test is preserved as a pure
// wide_math test using U256/I256 types that still exist for transient math.

// (Test removed — function no longer exists in the public API)

// ============================================================================
// T11.46: enqueue_adl_residual_booking_still_routes_quantity
// ============================================================================

#[kani::proof]
#[kani::solver(cadical)]
fn t11_46_enqueue_adl_k_add_overflow_still_routes_quantity() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let mut ctx = InstructionContext::new();

    engine.adl_coeff_long = i128::MIN + 1;
    engine.adl_mult_long = POS_SCALE;
    engine.oi_eff_long_q = 4 * POS_SCALE;
    engine.oi_eff_short_q = 4 * POS_SCALE;
    engine.insurance_fund.balance = U128::new(10_000_000);
    engine.stored_pos_count_long = 1;

    let k_before = engine.adl_coeff_long;
    let a_before = engine.adl_mult_long;
    let ins_before = engine.insurance_fund.balance.get();

    let d = 1_000_000u128;
    let q_close = 2 * POS_SCALE;

    let result = engine.enqueue_adl(&mut ctx, Side::Short, q_close, d);
    assert!(result.is_ok());

    // v12.20.6: bankruptcy residuals do not write K. Even near a K boundary,
    // quantity routing remains live and insurance is consumed first.
    assert!(
        engine.adl_coeff_long == k_before,
        "K_opp must not be modified by bankruptcy residual booking"
    );
    // A must shrink (quantity was still routed)
    assert!(
        engine.adl_mult_long < a_before,
        "A must shrink on K overflow"
    );
    // OI must decrease by q_close
    assert!(engine.oi_eff_long_q == 2 * POS_SCALE);
    // Insurance fund must decrease by D's insurance-covered prefix.
    assert!(
        engine.insurance_fund.balance.get() < ins_before,
        "insurance fund must decrease through insurance-first deficit coverage"
    );
}

// ============================================================================
// T11.47: precision_exhaustion_terminal_drain
// ============================================================================

#[kani::proof]
#[kani::solver(cadical)]
fn t11_47_precision_exhaustion_terminal_drain() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let mut ctx = InstructionContext::new();

    engine.adl_mult_long = 1;
    engine.adl_coeff_long = 0i128;
    engine.oi_eff_long_q = 3 * POS_SCALE;
    engine.oi_eff_short_q = 3 * POS_SCALE;
    engine.stored_pos_count_long = 1;

    let q_close = POS_SCALE;
    let d = 0u128;

    let result = engine.enqueue_adl(&mut ctx, Side::Short, q_close, d);
    assert!(result.is_ok());

    assert!(ctx.pending_reset_long);
    assert!(ctx.pending_reset_short);
    assert!(engine.oi_eff_long_q == 0);
    assert!(engine.oi_eff_short_q == 0);
}

// ============================================================================
// T11.48: bankruptcy_liquidation_routes_q_when_D_zero
// ============================================================================

#[kani::proof]
#[kani::solver(cadical)]
fn t11_48_bankruptcy_liquidation_routes_q_when_D_zero() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let mut ctx = InstructionContext::new();

    engine.adl_mult_long = POS_SCALE;
    engine.adl_coeff_long = 42i128;
    engine.oi_eff_long_q = 4 * POS_SCALE;
    engine.oi_eff_short_q = 4 * POS_SCALE;
    engine.stored_pos_count_long = 1;

    let k_before = engine.adl_coeff_long;
    let a_before = engine.adl_mult_long;

    let d = 0u128;
    let q_close = POS_SCALE;

    let result = engine.enqueue_adl(&mut ctx, Side::Short, q_close, d);
    assert!(result.is_ok());

    assert!(
        engine.adl_coeff_long == k_before,
        "K must be unchanged when D == 0"
    );
    assert!(engine.adl_mult_long < a_before, "A must shrink");
    assert!(engine.oi_eff_long_q == 3 * POS_SCALE);
}

// ============================================================================
// T11.49: pure_pnl_bankruptcy_path
// ============================================================================

#[kani::proof]
#[kani::solver(cadical)]
fn t11_49_pure_pnl_bankruptcy_path() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let mut ctx = InstructionContext::new();

    engine.adl_mult_long = POS_SCALE;
    engine.adl_coeff_long = 0i128;
    engine.oi_eff_long_q = 2 * POS_SCALE;
    engine.oi_eff_short_q = 2 * POS_SCALE;
    engine.stored_pos_count_long = 1;

    let a_before = engine.adl_mult_long;
    let k_before = engine.adl_coeff_long;
    let audit_before = engine.explicit_unallocated_loss_long.get();

    let d = 1_000u128;
    let q_close = 0u128;

    let result = engine.enqueue_adl(&mut ctx, Side::Short, q_close, d);
    assert!(result.is_ok());

    assert!(
        engine.adl_mult_long == a_before,
        "A must be unchanged for pure PnL bankruptcy"
    );
    assert!(
        engine.adl_coeff_long == k_before,
        "pure residual bankruptcy must not mutate K"
    );
    assert!(
        engine.explicit_unallocated_loss_long.get() > audit_before,
        "with no certified B weight, pure residual bankruptcy is durably audited"
    );
    assert!(engine.oi_eff_long_q == 2 * POS_SCALE);
}

// ============================================================================
// T11.53: keeper_crank_quiesces_after_pending_reset
// ============================================================================

#[kani::proof]
#[kani::solver(cadical)]
fn t11_53_keeper_crank_quiesces_after_pending_reset() {
    let mut engine = RiskEngine::new(zero_fee_params());

    engine.last_oracle_price = 100;
    engine.last_market_slot = 0;
    engine.adl_mult_long = ADL_ONE;
    engine.adl_mult_short = ADL_ONE;
    engine.adl_epoch_long = 0;
    engine.adl_epoch_short = 0;

    let a = add_user_test(&mut engine, 0).unwrap();
    let b = add_user_test(&mut engine, 0).unwrap();
    let c = add_user_test(&mut engine, 0).unwrap();

    // a: long POS_SCALE (entire long side OI), tiny capital → deeply underwater
    engine.deposit_not_atomic(a, 1, 0).unwrap();
    engine.accounts[a as usize].position_basis_q = POS_SCALE as i128;
    engine.accounts[a as usize].adl_a_basis = ADL_ONE;
    engine.accounts[a as usize].adl_k_snap = 0i128;
    engine.accounts[a as usize].adl_epoch_snap = 0;

    // b: short POS_SCALE, well-funded
    engine.deposit_not_atomic(b, 10_000_000, 0).unwrap();
    engine.accounts[b as usize].position_basis_q = -(POS_SCALE as i128);
    engine.accounts[b as usize].adl_a_basis = ADL_ONE;
    engine.accounts[b as usize].adl_k_snap = 0i128;
    engine.accounts[b as usize].adl_epoch_snap = 0;

    // c: NO position, just capital (should NOT be touched after pending reset)
    engine.deposit_not_atomic(c, 10_000_000, 0).unwrap();

    // BALANCED OI: 1 long (a) = PS, 1 short (b) = PS
    engine.stored_pos_count_long = 1;
    engine.stored_pos_count_short = 1;
    engine.oi_eff_long_q = POS_SCALE;
    engine.oi_eff_short_q = POS_SCALE;

    // Set K_long very negative → account a is deeply underwater
    engine.adl_coeff_long = -((ADL_ONE as i128) * 1000);

    let c_cap_before = engine.accounts[c as usize].capital.get();
    let c_pnl_before = engine.accounts[c as usize].pnl;

    let result = engine.keeper_crank_not_atomic(
        1,
        100,
        &[(a, Some(LiquidationPolicy::FullClose))],
        1,
        0i128,
        0,
        100,
        None,
        0,
    );
    assert!(result.is_ok());

    assert!(
        engine.accounts[c as usize].capital.get() == c_cap_before,
        "c's capital must not change — crank must quiesce after pending reset"
    );
    assert!(
        engine.accounts[c as usize].pnl == c_pnl_before,
        "c's PnL must not change — crank must quiesce after pending reset"
    );
}

// ============================================================================
// proof_drain_only_to_reset_progress
// ============================================================================

#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_drain_only_to_reset_progress() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let mut ctx = InstructionContext::new();

    // Long side: DrainOnly, OI = 0
    engine.side_mode_long = SideMode::DrainOnly;
    engine.oi_eff_long_q = 0u128;
    engine.oi_eff_short_q = 0u128;
    engine.stored_pos_count_long = 0;
    // Short side still has stored positions and zero OI. Under v12.19.53 this
    // must also schedule reset: stored current-epoch positions cannot remain
    // live on a zero-OI side.
    engine.stored_pos_count_short = 1;

    let result = engine.schedule_end_of_instruction_resets(&mut ctx);
    assert!(result.is_ok());

    // §5.7.D must fire for the DrainOnly long side
    assert!(
        ctx.pending_reset_long,
        "DrainOnly side with OI=0 must schedule reset via §5.7.D"
    );
    assert!(
        ctx.pending_reset_short,
        "stored positions with zero OI must schedule reset to avoid noncanonical live state"
    );
}

// ============================================================================
// proof_keeper_reset_lifecycle_last_stale_triggers_finalize
// ============================================================================

#[kani::proof]
#[kani::solver(cadical)]
fn proof_keeper_reset_lifecycle_last_stale_triggers_finalize() {
    let mut engine = RiskEngine::new_with_market(zero_fee_params(), 0, 100);

    engine.adl_mult_long = ADL_ONE;
    engine.adl_epoch_long = 1; // new epoch after the reset started
    engine.adl_epoch_short = 0;

    let a = add_user_test(&mut engine, 0).unwrap();

    // a: the last stale long account — has a position from epoch 0 (stale)
    engine
        .set_position_basis_q(a as usize, POS_SCALE as i128)
        .unwrap();
    engine.accounts[a as usize].adl_a_basis = ADL_ONE;
    engine.accounts[a as usize].adl_k_snap = 0i128;
    engine.accounts[a as usize].adl_epoch_snap = 0; // mismatches adl_epoch_long=1

    // Long side: ResetPending, 1 stale account remaining, OI=0
    engine.side_mode_long = SideMode::ResetPending;
    engine.stale_account_count_long = 1;

    assert!(engine.side_mode_long == SideMode::ResetPending);
    assert!(engine.stale_account_count_long == 1);
    assert!(engine.stored_pos_count_long == 1);
    assert!(
        engine.effective_pos_q(a as usize) == 0,
        "stale reset-pending positions have no current-market effective OI"
    );

    let mut ctx = InstructionContext::new_with_admission(0, 100);
    engine
        .touch_account_live_local(a as usize, &mut ctx)
        .unwrap();
    assert!(
        engine.stale_account_count_long == 0,
        "touching the last stale account must clear the stale counter"
    );
    assert!(
        engine.stored_pos_count_long == 0,
        "touching the last stale account must remove the stale stored position"
    );
    assert!(
        engine.accounts[a as usize].position_basis_q == 0,
        "stale reset settlement must flatten the stale account"
    );
    assert!(
        engine.side_mode_long == SideMode::ResetPending,
        "touch alone must not finalize the reset before end-of-instruction"
    );

    engine
        .finalize_touched_accounts_post_live(&mut ctx)
        .unwrap();
    engine.schedule_end_of_instruction_resets(&mut ctx).unwrap();
    engine.finalize_end_of_instruction_resets(&ctx).unwrap();

    assert!(
        engine.side_mode_long == SideMode::Normal,
        "touching last stale account must finalize ResetPending → Normal (spec property #26)"
    );
    assert!(engine.stale_account_count_long == 0);
    assert!(engine.stored_pos_count_long == 0);
}

// ============================================================================
// proof_phase2_missing_slot_scan_progress_or_rate_limited_boundary
// ============================================================================

#[kani::proof]
#[kani::unwind(6)]
#[kani::solver(cadical)]
fn proof_phase2_missing_slot_scan_progress_or_rate_limited_boundary() {
    let max_accounts: u8 = kani::any();
    let cursor: u8 = kani::any();
    let rr_scan_limit: u8 = kani::any();
    let rr_touch_limit: u8 = kani::any();
    let wrap_allowed: bool = kani::any();

    kani::assume((1..=4).contains(&max_accounts));
    kani::assume(cursor < max_accounts);
    kani::assume((1..=4).contains(&rr_scan_limit));
    kani::assume((1..=4).contains(&rr_touch_limit));

    let mut engine =
        RiskEngine::new_with_market(small_zero_fee_params(max_accounts as u64), 0, 100);
    engine.rr_cursor_position = cursor as u64;

    let out = engine
        .phase2_scan_outcome(
            max_accounts as u64,
            rr_touch_limit as u64,
            rr_scan_limit as u64,
            false,
            wrap_allowed,
            false,
        )
        .unwrap();

    let blocked_by_slot_rate = !wrap_allowed && cursor == max_accounts - 1;
    if blocked_by_slot_rate {
        assert_eq!(
            out.inspected, 0,
            "same-slot generation boundary must not pretend to scan progress"
        );
        assert_eq!(
            out.next_cursor, cursor as u64,
            "same-slot generation boundary must leave cursor unchanged"
        );
        assert!(!out.wrapped);
    } else {
        assert!(
            out.inspected > 0,
            "permissionless Phase 2 must authenticate at least one missing slot when not boundary-limited"
        );
        assert!(
            out.next_cursor != cursor as u64 || out.wrapped,
            "authenticated missing-slot scan must advance cursor state"
        );
    }

    assert_eq!(
        out.touched, 0,
        "empty-slot progress must not consume touched-account capacity"
    );
    assert!(out.inspected <= rr_scan_limit as u64);
    assert!(out.inspected <= max_accounts as u64);
    kani::cover!(
        !blocked_by_slot_rate && out.inspected > 0 && out.touched == 0,
        "missing-slot cursor progress branch is reachable"
    );
    kani::cover!(
        blocked_by_slot_rate && out.inspected == 0 && out.next_cursor == cursor as u64,
        "slot-rate boundary branch is reachable"
    );
}

// ============================================================================
// proof_honest_crank_rank_progress_or_recovery
// ============================================================================

#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn proof_honest_crank_rank_progress_or_recovery() {
    let work = HonestCrankWorkModel {
        catchup_lag: kani::any(),
        cursor_remaining: kani::any(),
        stress_remaining: kani::any(),
        reset_stale_remaining: kani::any(),
        b_remaining: kani::any(),
        close_chunk_remaining: kani::any(),
        resolved_blockers: kani::any(),
        segment_blocked: kani::any(),
    };
    let profile = HonestCrankProfileModel {
        scan_budget: kani::any(),
        touch_budget: kani::any(),
        b_budget: kani::any(),
        close_budget: kani::any(),
        resolved_budget: kani::any(),
        recovery_enabled: kani::any(),
    };

    kani::assume(work.catchup_lag <= 4);
    kani::assume(work.cursor_remaining <= 4);
    kani::assume(work.stress_remaining <= 4);
    kani::assume(work.reset_stale_remaining <= 4);
    kani::assume(work.b_remaining <= 4);
    kani::assume(work.close_chunk_remaining <= 4);
    kani::assume(work.resolved_blockers <= 4);
    kani::assume(profile.scan_budget <= 4);
    kani::assume(profile.touch_budget <= 4);
    kani::assume(profile.b_budget <= 4);
    kani::assume(profile.close_budget <= 4);
    kani::assume(profile.resolved_budget <= 4);
    kani::assume(honest_crank_valid_public_profile(profile));

    let before = honest_crank_work_sum(work);
    let step = honest_crank_step_model(work, profile);

    assert!(
        step.outcome != MODEL_OUTCOME_STUCK,
        "valid one-honest-cranker profile must not produce a stuck transition"
    );
    if step.outcome == MODEL_OUTCOME_PROGRESS {
        assert!(
            honest_crank_work_sum(step.next) < before,
            "progress transition must reduce the public liveness rank"
        );
    } else if step.outcome == MODEL_OUTCOME_RECOVERY {
        assert!(
            work.segment_blocked && profile.recovery_enabled,
            "recovery is only the explicit route for blocked ordinary progress"
        );
    } else {
        assert_eq!(
            before, 0,
            "quiescent outcome is only valid when no public work remains"
        );
    }

    kani::cover!(
        step.outcome == MODEL_OUTCOME_PROGRESS && honest_crank_work_sum(step.next) < before,
        "rank-decreasing progress branch is reachable"
    );
    kani::cover!(
        step.outcome == MODEL_OUTCOME_RECOVERY && work.segment_blocked,
        "recovery-required branch is reachable"
    );
    kani::cover!(
        step.outcome == MODEL_OUTCOME_QUIESCENT && before == 0,
        "quiescent branch is reachable"
    );
}

// ============================================================================
// proof_unilateral_empty_orphan_reset
// ============================================================================

#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_unilateral_empty_orphan_reset() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let mut ctx = InstructionContext::new();

    // Long side: no stored positions, but has orphan residual OI.
    engine.stored_pos_count_long = 0;
    // Short side: still has stored positions
    engine.stored_pos_count_short = 2;

    // Potential dust records uncertain floor slack; it is not a certified
    // OI-clearance allowance. One-empty-side residual OI must still make
    // progress through explicit orphan-exposure reset.
    let dust: u128 = kani::any();
    kani::assume(dust > 0);
    kani::assume(dust <= 100);
    engine.phantom_dust_potential_long_q = dust;
    engine.phantom_dust_potential_short_q = dust;
    engine.oi_eff_long_q = dust;
    engine.oi_eff_short_q = dust; // balanced (required by spec)

    let result = engine.schedule_end_of_instruction_resets(&mut ctx);
    assert!(result.is_ok());

    // One-empty-side residual OI schedules reset on both sides.
    assert!(
        ctx.pending_reset_long,
        "unilateral-empty side with residual OI must schedule reset"
    );
    assert!(
        ctx.pending_reset_short,
        "opposite side must also get reset for bilateral consistency"
    );
    // OI must be zeroed
    assert!(
        engine.oi_eff_long_q == 0,
        "OI must be zeroed after dust clearance"
    );
    assert!(
        engine.oi_eff_short_q == 0,
        "OI must be zeroed after dust clearance"
    );
    assert_eq!(
        engine.phantom_dust_potential_long_q, 0,
        "potential dust must be consumed by orphan reset"
    );
    assert_eq!(
        engine.phantom_dust_potential_short_q, 0,
        "potential dust must be consumed by orphan reset"
    );
}

// ############################################################################
// Full ADL pipeline integration: trade → liquidation → ADL → reset → reopen
// ############################################################################

/// End-to-end ADL lifecycle: two accounts hold a valid bilateral position,
/// ADL socializes a deficit, end-of-instruction resets fire, stale accounts
/// settle out, and a later balanced position can reopen the market.
/// Verifies OI_eff_long == OI_eff_short is maintained throughout.
#[kani::proof]
#[kani::unwind(70)]
#[kani::solver(cadical)]
fn proof_adl_pipeline_trade_liquidate_reopen() {
    let mut engine = RiskEngine::new_with_market(zero_fee_params(), DEFAULT_SLOT, DEFAULT_ORACLE);

    let a = add_user_test(&mut engine, 0).unwrap();
    let b = add_user_test(&mut engine, 0).unwrap();
    let c = add_user_test(&mut engine, 0).unwrap();
    engine.deposit_not_atomic(a, 100_000, DEFAULT_SLOT).unwrap();
    engine.deposit_not_atomic(b, 500_000, DEFAULT_SLOT).unwrap();
    engine.deposit_not_atomic(c, 500_000, DEFAULT_SLOT).unwrap();

    let size = 3 * POS_SCALE;
    engine
        .attach_effective_position(a as usize, size as i128)
        .unwrap();
    engine
        .attach_effective_position(b as usize, -(size as i128))
        .unwrap();
    engine.oi_eff_long_q = size;
    engine.oi_eff_short_q = size;
    assert!(
        engine.oi_eff_long_q == engine.oi_eff_short_q,
        "OI must balance after trade"
    );
    assert!(engine.check_conservation());

    let mut ctx = InstructionContext::new();
    let k_short_before = engine.adl_coeff_short;
    let b_short_before = engine.b_short_num;
    let result = engine.enqueue_adl(&mut ctx, Side::Long, size, 1_000u128);
    assert!(result.is_ok(), "ADL enqueue must succeed for balanced OI");
    assert!(
        engine.oi_eff_long_q == engine.oi_eff_short_q,
        "OI must balance after liquidation+ADL"
    );
    assert!(engine.oi_eff_long_q == 0, "full ADL close drains long OI");
    assert!(engine.oi_eff_short_q == 0, "full ADL close drains short OI");
    assert!(
        ctx.pending_reset_long,
        "ADL full drain must schedule long reset"
    );
    assert!(
        ctx.pending_reset_short,
        "ADL full drain must schedule short reset"
    );
    assert!(
        engine.adl_coeff_short == k_short_before,
        "bankruptcy residual must not mutate opposing short side K"
    );
    assert!(
        engine.b_short_num > b_short_before,
        "deficit must be booked to the opposing short side B"
    );
    assert!(engine.check_conservation());

    let reset_result = engine.finalize_end_of_instruction_resets(&ctx);
    assert!(reset_result.is_ok(), "pending ADL resets must finalize");
    assert!(engine.side_mode_long == SideMode::ResetPending);
    assert!(engine.side_mode_short == SideMode::ResetPending);
    assert!(engine.stale_account_count_long == 1);
    assert!(engine.stale_account_count_short == 1);

    let mut settle_ctx = InstructionContext::new_with_admission(0, 100);
    engine
        .settle_side_effects_live(a as usize, &mut settle_ctx)
        .unwrap();
    engine
        .settle_side_effects_live(b as usize, &mut settle_ctx)
        .unwrap();
    engine
        .finalize_end_of_instruction_resets(&InstructionContext::new())
        .unwrap();
    assert!(engine.side_mode_long == SideMode::Normal);
    assert!(engine.side_mode_short == SideMode::Normal);
    assert!(engine.stored_pos_count_long == 0);
    assert!(engine.stored_pos_count_short == 0);

    let new_size = POS_SCALE;
    engine
        .attach_effective_position(c as usize, new_size as i128)
        .unwrap();
    engine
        .attach_effective_position(b as usize, -(new_size as i128))
        .unwrap();
    engine.oi_eff_long_q = new_size;
    engine.oi_eff_short_q = new_size;
    assert!(
        engine.oi_eff_long_q == engine.oi_eff_short_q,
        "OI must balance after reopen attempt"
    );
    assert!(
        engine.check_conservation(),
        "conservation after full pipeline"
    );
    kani::cover!(
        engine.side_mode_long == SideMode::Normal
            && engine.side_mode_short == SideMode::Normal
            && engine.oi_eff_long_q == new_size,
        "post-ADL market reopens with balanced OI"
    );
}
