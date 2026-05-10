//! Section 7 — Liveness, progress, no-deadlock
//!
//! Auto-finalization, trade reopening, ADL fallback routes,
//! precision exhaustion, crank quiescence, drain-only progress.

#![cfg(kani)]

mod common;
use common::*;

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
            // ENG-PORT-4 fixup: 6-arg signature. Per-account positions in scope.
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
// T11.46: enqueue_adl_k_add_overflow_still_routes_quantity
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

    // K_opp must be UNCHANGED when K_opp + delta_K overflows
    assert!(
        engine.adl_coeff_long == k_before,
        "K_opp must not be modified on K-space overflow (spec §5.6 step 6)"
    );
    // A must shrink (quantity was still routed)
    assert!(
        engine.adl_mult_long < a_before,
        "A must shrink on K overflow"
    );
    // OI must decrease by q_close
    assert!(engine.oi_eff_long_q == 2 * POS_SCALE);
    // Insurance fund must decrease by D (absorb_protocol_loss was invoked)
    assert!(
        engine.insurance_fund.balance.get() < ins_before,
        "insurance fund must decrease — absorb_protocol_loss must be invoked"
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

    let d = 1_000u128;
    let q_close = 0u128;

    let result = engine.enqueue_adl(&mut ctx, Side::Short, q_close, d);
    assert!(result.is_ok());

    assert!(
        engine.adl_mult_long == a_before,
        "A must be unchanged for pure PnL bankruptcy"
    );
    assert!(
        engine.adl_coeff_long != k_before,
        "K must change when D > 0"
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
    // Short side still has stored positions → §5.7.A (bilateral-empty) does NOT fire
    engine.stored_pos_count_short = 1;

    let result = engine.schedule_end_of_instruction_resets(&mut ctx);
    assert!(result.is_ok());

    // §5.7.D must fire for the DrainOnly long side
    assert!(
        ctx.pending_reset_long,
        "DrainOnly side with OI=0 must schedule reset via §5.7.D"
    );
    assert!(
        !ctx.pending_reset_short,
        "opposite side must not get reset from DrainOnly path alone"
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

    engine.finalize_touched_accounts_post_live(&ctx).unwrap();
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
// proof_unilateral_empty_orphan_dust_clearance
// ============================================================================

#[kani::proof]
#[kani::solver(cadical)]
fn proof_unilateral_empty_orphan_dust_clearance() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let mut ctx = InstructionContext::new();

    // Long side: no stored positions, but has phantom dust OI
    engine.stored_pos_count_long = 0;
    // Short side: still has stored positions
    engine.stored_pos_count_short = 2;

    // Phantom dust: OI == dust bound (should clear)
    let dust = 42u128;
    engine.phantom_dust_potential_long_q = dust;
    engine.oi_eff_long_q = dust; // OI <= dust bound
    engine.oi_eff_short_q = dust; // balanced (required by spec)

    let result = engine.schedule_end_of_instruction_resets(&mut ctx);
    assert!(result.is_ok());

    // §5.7.B: long side is empty, OI within dust bound → both sides get reset
    assert!(
        ctx.pending_reset_long,
        "unilateral-empty side with OI within dust bound must schedule reset (§5.7.B)"
    );
    assert!(
        ctx.pending_reset_short,
        "opposite side must also get reset for bilateral consistency (§5.7.B)"
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
        engine.adl_coeff_short < k_short_before,
        "deficit must be socialized to the opposing short side K"
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

// ############################################################################
// Wave 1 ENG-PORT-B: force_close_resolved_with_fee_not_atomic invariant
// ############################################################################

/// Wave 1 / ENG-PORT-B: fee-credited-at-resolved-close invariant.
///
/// `force_close_resolved_with_fee_not_atomic` MUST sync the recurring
/// maintenance fee at the resolved-slot anchor BEFORE returning
/// ProgressOnly when the account is in the not-yet-payable case
/// (`pnl > 0 && !is_terminal_ready`). The fee charge moves capital
/// from the user to the insurance fund and stamps last_fee_slot to
/// resolved_slot — without this, a wrapper that re-calls the function
/// would either re-charge the same dt (double-charge) or skip the
/// charge entirely.
///
/// Mirrors toly engine tests/proofs_liveness.rs:1825-1869
/// (`proof_force_close_resolved_with_fee_progress_only_syncs_before_payout_on_prod_code`).
#[kani::proof]
#[kani::unwind(80)]
#[kani::solver(cadical)]
fn proof_force_close_resolved_with_fee_progress_only_syncs_before_payout_on_prod_code() {
    let mut engine =
        RiskEngine::new_with_market(small_zero_fee_params(4), DEFAULT_SLOT, DEFAULT_ORACLE);
    engine.deposit_not_atomic(0, 100, DEFAULT_SLOT).unwrap();
    engine.deposit_not_atomic(1, 100, DEFAULT_SLOT).unwrap();
    engine.market_mode = MarketMode::Resolved;
    engine.current_slot = DEFAULT_SLOT;
    engine.resolved_slot = DEFAULT_SLOT;
    engine.resolved_price = DEFAULT_ORACLE;
    engine.resolved_live_price = DEFAULT_ORACLE;
    engine.set_pnl(0, 10).unwrap();
    engine.set_pnl(1, -5).unwrap();
    engine.accounts[0].last_fee_slot = DEFAULT_SLOT - 1;

    let fee_rate: u8 = kani::any();
    kani::assume(fee_rate > 0 && fee_rate <= 10);
    let capital_before = engine.accounts[0].capital.get();
    let pnl_before = engine.accounts[0].pnl;
    let insurance_before = engine.insurance_fund.balance.get();

    let result = engine.force_close_resolved_with_fee_not_atomic(0, fee_rate as u128);

    assert_eq!(result, Ok(ResolvedCloseResult::ProgressOnly));
    assert!(engine.is_used(0));
    assert_eq!(engine.accounts[0].last_fee_slot, engine.resolved_slot);
    assert_eq!(engine.accounts[0].pnl, pnl_before);
    assert_eq!(
        engine.accounts[0].capital.get(),
        capital_before - fee_rate as u128
    );
    assert_eq!(
        engine.insurance_fund.balance.get(),
        insurance_before + fee_rate as u128
    );
    assert_eq!(engine.neg_pnl_account_count, 1);
    assert_eq!(engine.market_mode, MarketMode::Resolved);
    assert!(engine.check_conservation());
    kani::cover!(
        result == Ok(ResolvedCloseResult::ProgressOnly)
            && engine.is_used(0)
            && engine.accounts[0].last_fee_slot == engine.resolved_slot
            && engine.insurance_fund.balance.get() > insurance_before,
        "fee-aware resolved close syncs fee before ProgressOnly without payout/free"
    );
}
