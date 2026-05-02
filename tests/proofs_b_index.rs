//! B-index bankruptcy socialization spec-model proofs.
//!
//! These proofs are small symbolic models for the v12.20.6 B-index rules.
//! They are intentionally independent from the current v12.19 engine structs:
//! the production engine has not yet implemented B-index state.

#![cfg(kani)]

const MODEL_DEN: u32 = 16;
const MODEL_MAX_B: u32 = 31;
const MODEL_MAX_ACCOUNT_B_LOSS: u32 = 7;
const MODEL_MAX_PUBLIC_WORK: u8 = 8;
const MODEL_PNL_MIN: i16 = -31;
const MODEL_PNL_MAX: i16 = 31;

#[derive(Clone, Copy)]
struct CloseModel {
    active: bool,
    phase: u8,
    close_price: u8,
    close_slot: u8,
    fee_obligation: u8,
    liq_epoch: u8,
    opp_epoch: u8,
    q_close: u8,
    residual_remaining: u8,
    b_chunks_booked: u8,
    account_b_chunks_booked: u8,
    account_b_current: bool,
}

#[derive(Clone, Copy)]
struct ClaimModel {
    vault: u16,
    c_tot: u16,
    insurance: u16,
    pnl_pos_tot: u16,
    explicit_unallocated: u16,
    residual_remaining: u16,
    account_cleared: bool,
}

#[derive(Clone, Copy)]
struct AccountBModel {
    b_snap: u8,
    b_target: u8,
    weight: u8,
    b_rem: u8,
}

#[derive(Clone, Copy)]
struct RecoveryInputModel {
    ordinary_progress_possible: bool,
    authenticated_price_available: bool,
    authenticated_price: u8,
    p_last: u8,
    fallback_enabled: bool,
    caller_omitted_or_corrupted_proof: bool,
}

fn continue_active_close(
    state: CloseModel,
    supplied_price: u8,
    supplied_slot: u8,
    supplied_fee: u8,
) -> Option<CloseModel> {
    if !state.active {
        return None;
    }
    if supplied_price != state.close_price
        || supplied_slot != state.close_slot
        || supplied_fee != state.fee_obligation
    {
        return None;
    }

    let mut next = state;
    if next.phase < 6 {
        next.phase += 1;
    }
    Some(next)
}

fn active_close_next_outcome(
    state: CloseModel,
    max_residual_chunks: u8,
    max_account_chunks: u8,
) -> u8 {
    // 0 = reject/no active close, 1 = bounded progress, 2 = recovery required.
    if !state.active {
        return 0;
    }
    if !state.account_b_current && state.account_b_chunks_booked >= max_account_chunks {
        return 2;
    }
    if state.residual_remaining > 0 && state.b_chunks_booked >= max_residual_chunks {
        return 2;
    }
    1
}

fn max_representable_chunk(h: u32, w: u32, rem: u32) -> u32 {
    let max_scaled = (h + 1) * w - 1;
    if rem > max_scaled {
        0
    } else {
        (max_scaled - rem) / MODEL_DEN
    }
}

fn engine_residual_chunk(residual: u8, public_budget: u8, max_chunk_by_b: u8) -> u8 {
    let engine_chunk = core::cmp::min(residual, max_chunk_by_b);
    if engine_chunk > public_budget {
        public_budget
    } else {
        engine_chunk
    }
}

fn max_account_b_delta(loss_limit: u32, w: u32, rem: u32) -> u32 {
    let max_num = (loss_limit + 1) * MODEL_DEN - 1;
    if rem > max_num {
        0
    } else {
        (max_num - rem) / w
    }
}

fn settle_account_b_chunk(
    state: AccountBModel,
    loss_limit: u8,
    delta_budget: u8,
) -> Option<(AccountBModel, u8, u8)> {
    if state.b_snap >= state.b_target {
        return None;
    }
    if state.weight == 0 || (state.weight as u32) > MODEL_DEN {
        return None;
    }
    if (state.b_rem as u32) >= MODEL_DEN || delta_budget == 0 {
        return None;
    }

    let remaining = (state.b_target - state.b_snap) as u32;
    let max_delta = max_account_b_delta(loss_limit as u32, state.weight as u32, state.b_rem as u32);
    let delta = core::cmp::min(remaining, core::cmp::min(max_delta, delta_budget as u32));
    if delta == 0 {
        return None;
    }

    let num = state.b_rem as u32 + (state.weight as u32) * delta;
    let loss = num / MODEL_DEN;
    let b_rem = num % MODEL_DEN;
    let next = AccountBModel {
        b_snap: state.b_snap + delta as u8,
        b_target: state.b_target,
        weight: state.weight,
        b_rem: b_rem as u8,
    };
    Some((next, loss as u8, delta as u8))
}

fn settle_account_b_chunk_with_combined_pnl_guard(
    state: AccountBModel,
    loss_limit: u8,
    delta_budget: u8,
    pnl_before: i8,
    kf_delta: i8,
) -> Option<(AccountBModel, i16)> {
    let (next, b_loss, _) = settle_account_b_chunk(state, loss_limit, delta_budget)?;
    let pnl_after = pnl_before as i16 + kf_delta as i16 - b_loss as i16;
    if !(MODEL_PNL_MIN..=MODEL_PNL_MAX).contains(&pnl_after) {
        return None;
    }
    Some((next, pnl_after))
}

fn user_value_allowed_after_b(state: AccountBModel) -> bool {
    state.b_snap == state.b_target
}

fn flush_remainder_to_dust(rem: u8, dust: u8) -> (u8, u8, u8) {
    let total = rem as u16 + dust as u16;
    let whole = total / MODEL_DEN as u16;
    let new_dust = total % MODEL_DEN as u16;
    (0, new_dust as u8, whole as u8)
}

fn b_booking_allowed(certified_weight_sum: bool, known_zero_effective_member: bool) -> bool {
    certified_weight_sum && !known_zero_effective_member
}

fn atomic_position_close(
    basis_live: bool,
    oi: u8,
    weight_sum: u8,
    q_close: u8,
    account_weight: u8,
) -> Option<(bool, u8, u8)> {
    if !basis_live || q_close == 0 || q_close > oi || account_weight > weight_sum {
        return None;
    }
    Some((false, oi - q_close, weight_sum - account_weight))
}

fn record_explicit_loss_then_clear(mut state: ClaimModel) -> Option<ClaimModel> {
    if state.residual_remaining == 0 {
        return None;
    }
    state.explicit_unallocated = state
        .explicit_unallocated
        .checked_add(state.residual_remaining)?;
    state.residual_remaining = 0;
    state.account_cleared = true;
    Some(state)
}

fn valid_liveness_profile(profile: u8, recovery_enabled: bool, impossible_proof: bool) -> bool {
    // 0 = BestEffort, 1 = CrankForward. Other tags are invalid.
    match profile {
        0 => true,
        1 => recovery_enabled || impossible_proof,
        _ => false,
    }
}

fn valid_public_user_fund_profile(
    profile: u8,
    recovery_enabled: bool,
    bankrupt_close_chunks: u8,
    account_b_chunks: u8,
    public_b_chunk_atoms: u8,
) -> bool {
    // 1 = CrankForward. Public user-fund markets cannot use BestEffort.
    profile == 1
        && recovery_enabled
        && bankrupt_close_chunks > 0
        && account_b_chunks > 0
        && (public_b_chunk_atoms as u32) >= MODEL_DEN
}

fn effective_h_lock(admit_h_min: u8, admit_h_max: u8, hmax_effective_active: bool) -> u8 {
    if hmax_effective_active {
        admit_h_max
    } else {
        admit_h_min
    }
}

fn bankruptcy_residual_excluding_fee_shortfall(
    bankruptcy_loss: u8,
    liquidation_fee_shortfall: u8,
) -> (u8, u8) {
    // Residual socialization includes only bankruptcy loss; fee shortfall is dropped/forgiven.
    (bankruptcy_loss, liquidation_fee_shortfall)
}

fn recovery_price(input: RecoveryInputModel) -> Option<u8> {
    if input.ordinary_progress_possible || input.p_last == 0 {
        return None;
    }
    if input.authenticated_price_available {
        if input.authenticated_price == 0 {
            None
        } else {
            Some(input.authenticated_price)
        }
    } else if input.fallback_enabled && !input.caller_omitted_or_corrupted_proof {
        Some(input.p_last)
    } else {
        None
    }
}

fn public_instruction_work_allowed(work_items: u8) -> bool {
    work_items <= MODEL_MAX_PUBLIC_WORK
}

#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn proof_active_bankrupt_close_uses_frozen_economics() {
    let state = CloseModel {
        active: true,
        phase: kani::any(),
        close_price: kani::any(),
        close_slot: kani::any(),
        fee_obligation: kani::any(),
        liq_epoch: kani::any(),
        opp_epoch: kani::any(),
        q_close: kani::any(),
        residual_remaining: kani::any(),
        b_chunks_booked: kani::any(),
        account_b_chunks_booked: kani::any(),
        account_b_current: kani::any(),
    };
    kani::assume(state.phase <= 6);
    kani::assume(state.close_price > 0);
    kani::assume(state.q_close > 0);

    let supplied_price: u8 = kani::any();
    let supplied_slot: u8 = kani::any();
    let supplied_fee: u8 = kani::any();

    let result = continue_active_close(state, supplied_price, supplied_slot, supplied_fee);
    if let Some(next) = result {
        assert!(supplied_price == state.close_price);
        assert!(supplied_slot == state.close_slot);
        assert!(supplied_fee == state.fee_obligation);
        assert!(next.close_price == state.close_price);
        assert!(next.close_slot == state.close_slot);
        assert!(next.fee_obligation == state.fee_obligation);
        assert!(next.liq_epoch == state.liq_epoch);
        assert!(next.opp_epoch == state.opp_epoch);
        assert!(next.q_close == state.q_close);
        assert!(next.residual_remaining == state.residual_remaining);
        assert!(next.b_chunks_booked == state.b_chunks_booked);
        assert!(next.account_b_chunks_booked == state.account_b_chunks_booked);
        assert!(next.account_b_current == state.account_b_current);
        assert!(next.phase >= state.phase);
    }

    kani::cover!(
        continue_active_close(
            state,
            state.close_price,
            state.close_slot,
            state.fee_obligation
        )
        .is_some(),
        "valid continuation using frozen economics is reachable"
    );
}

#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn proof_active_bankrupt_close_chunk_caps_force_recovery_not_infinite_progress() {
    let state = CloseModel {
        active: true,
        phase: kani::any(),
        close_price: kani::any(),
        close_slot: kani::any(),
        fee_obligation: kani::any(),
        liq_epoch: kani::any(),
        opp_epoch: kani::any(),
        q_close: kani::any(),
        residual_remaining: kani::any(),
        b_chunks_booked: kani::any(),
        account_b_chunks_booked: kani::any(),
        account_b_current: kani::any(),
    };
    let max_residual_chunks: u8 = kani::any();
    let max_account_chunks: u8 = kani::any();

    if state.residual_remaining > 0 && state.b_chunks_booked >= max_residual_chunks {
        assert!(
            active_close_next_outcome(state, max_residual_chunks, max_account_chunks) == 2,
            "residual chunk cap must route active close to recovery"
        );
    }
    if !state.account_b_current && state.account_b_chunks_booked >= max_account_chunks {
        assert!(
            active_close_next_outcome(state, max_residual_chunks, max_account_chunks) == 2,
            "account-local B chunk cap must route active close to recovery"
        );
    }
}

#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn proof_residual_b_booking_uses_engine_determined_chunk_not_caller_smaller() {
    let residual: u8 = kani::any();
    let public_budget: u8 = kani::any();
    let max_chunk_by_b: u8 = kani::any();
    let caller_chunk: u8 = kani::any();

    kani::assume(residual > 0);
    kani::assume(public_budget > 0);
    kani::assume(max_chunk_by_b > 0);

    let expected = engine_residual_chunk(residual, public_budget, max_chunk_by_b);
    kani::assume(expected > 0);
    let accepted = caller_chunk == expected;

    if caller_chunk < expected {
        assert!(
            !accepted,
            "caller-selected smaller residual chunk must not be accepted"
        );
    }
    if accepted {
        assert!(caller_chunk == expected);
    }
}

#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn proof_b_booking_chunk_conserves_scaled_loss_and_bounds_b() {
    let b_before: u8 = kani::any();
    let w_raw: u8 = kani::any();
    let rem_raw: u8 = kani::any();
    let residual: u8 = kani::any();
    let budget: u8 = kani::any();

    kani::assume((b_before as u32) <= MODEL_MAX_B);
    kani::assume(w_raw > 0 && (w_raw as u32) <= MODEL_DEN);
    kani::assume((rem_raw as u32) < w_raw as u32);
    kani::assume(residual > 0 && budget > 0);

    let h = MODEL_MAX_B - b_before as u32;
    let w = w_raw as u32;
    let rem = rem_raw as u32;
    let max_chunk = max_representable_chunk(h, w, rem);
    kani::assume(max_chunk > 0);

    let chunk = core::cmp::min(core::cmp::min(residual as u32, budget as u32), max_chunk);
    kani::assume(chunk > 0);

    let scaled = chunk * MODEL_DEN + rem;
    let delta_b = scaled / w;
    let new_rem = scaled % w;
    let b_after = b_before as u32 + delta_b;

    assert!(delta_b > 0);
    assert!(b_after <= MODEL_MAX_B);
    assert!(new_rem < w);
    assert!(delta_b * w + new_rem - rem == chunk * MODEL_DEN);
}

#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn proof_account_b_settlement_chunk_respects_loss_limit_and_advances_locally() {
    let state = AccountBModel {
        b_snap: kani::any(),
        b_target: kani::any(),
        weight: kani::any(),
        b_rem: kani::any(),
    };
    let loss_limit: u8 = kani::any();
    let delta_budget: u8 = kani::any();

    kani::assume((state.b_target as u32) <= MODEL_MAX_B);
    kani::assume(state.b_snap < state.b_target);
    kani::assume(state.weight > 0 && (state.weight as u32) <= MODEL_DEN);
    kani::assume((state.b_rem as u32) < MODEL_DEN);
    kani::assume((loss_limit as u32) <= MODEL_MAX_ACCOUNT_B_LOSS);
    kani::assume(delta_budget > 0);

    if let Some((next, loss, delta)) = settle_account_b_chunk(state, loss_limit, delta_budget) {
        assert!(delta > 0);
        assert!(next.b_snap > state.b_snap);
        assert!(next.b_snap <= state.b_target);
        assert!(next.b_target == state.b_target);
        assert!(next.weight == state.weight);
        assert!((next.b_rem as u32) < MODEL_DEN);
        assert!((loss as u32) <= loss_limit as u32);
        assert!(
            (loss as u32) * MODEL_DEN + next.b_rem as u32
                == state.b_rem as u32 + (state.weight as u32) * (delta as u32),
            "account-local chunk must conserve scaled B settlement"
        );
    }

    kani::cover!(
        settle_account_b_chunk(
            AccountBModel {
                b_snap: 0,
                b_target: 3,
                weight: 5,
                b_rem: 2,
            },
            1,
            3,
        )
        .is_some(),
        "nontrivial account-local B chunk is reachable"
    );
}

#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn proof_account_b_chunk_commit_requires_combined_pnl_candidate_fits() {
    let state = AccountBModel {
        b_snap: kani::any(),
        b_target: kani::any(),
        weight: kani::any(),
        b_rem: kani::any(),
    };
    let loss_limit: u8 = kani::any();
    let delta_budget: u8 = kani::any();
    let pnl_before: i8 = kani::any();
    let kf_delta: i8 = kani::any();

    kani::assume((state.b_target as u32) <= MODEL_MAX_B);
    kani::assume(state.b_snap < state.b_target);
    kani::assume(state.weight > 0 && (state.weight as u32) <= MODEL_DEN);
    kani::assume((state.b_rem as u32) < MODEL_DEN);
    kani::assume((loss_limit as u32) <= MODEL_MAX_ACCOUNT_B_LOSS);
    kani::assume(delta_budget > 0);

    if let Some((next, pnl_after)) = settle_account_b_chunk_with_combined_pnl_guard(
        state,
        loss_limit,
        delta_budget,
        pnl_before,
        kf_delta,
    ) {
        assert!(next.b_snap > state.b_snap);
        assert!((MODEL_PNL_MIN..=MODEL_PNL_MAX).contains(&pnl_after));
    }

    kani::cover!(
        settle_account_b_chunk_with_combined_pnl_guard(
            AccountBModel {
                b_snap: 0,
                b_target: 2,
                weight: 5,
                b_rem: 0,
            },
            1,
            2,
            0,
            0,
        )
        .is_some(),
        "nontrivial combined B+K/F settlement write is reachable"
    );
}

#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn proof_account_b_partial_settlement_blocks_user_value_until_current() {
    let state = AccountBModel {
        b_snap: kani::any(),
        b_target: kani::any(),
        weight: kani::any(),
        b_rem: kani::any(),
    };
    let loss_limit: u8 = kani::any();
    let delta_budget: u8 = kani::any();

    kani::assume((state.b_target as u32) <= MODEL_MAX_B);
    kani::assume(state.b_snap < state.b_target);
    kani::assume(state.weight > 0 && (state.weight as u32) <= MODEL_DEN);
    kani::assume((state.b_rem as u32) < MODEL_DEN);
    kani::assume((loss_limit as u32) <= MODEL_MAX_ACCOUNT_B_LOSS);

    if let Some((next, _, _)) = settle_account_b_chunk(state, loss_limit, delta_budget) {
        if next.b_snap < next.b_target {
            assert!(
                !user_value_allowed_after_b(next),
                "partial B settlement must keep user value/risk-increase actions blocked"
            );
        }
    }
}

#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn proof_account_b_zero_delta_routes_to_progress_required_or_recovery() {
    let state = AccountBModel {
        b_snap: kani::any(),
        b_target: kani::any(),
        weight: kani::any(),
        b_rem: kani::any(),
    };
    let loss_limit: u8 = kani::any();

    kani::assume((state.b_target as u32) <= MODEL_MAX_B);
    kani::assume(state.b_snap < state.b_target);
    kani::assume(state.weight > 0 && (state.weight as u32) <= MODEL_DEN);
    kani::assume((state.b_rem as u32) < MODEL_DEN);
    kani::assume((loss_limit as u32) <= MODEL_MAX_ACCOUNT_B_LOSS);

    let no_budget = settle_account_b_chunk(state, loss_limit, 0);
    assert!(
        no_budget.is_none(),
        "zero account-B delta budget must not pretend to settle progress"
    );
}

#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn proof_loose_b_chunk_formula_has_overflow_counterexample() {
    let h = 1u32;
    let w = 10u32;
    let rem = 9u32;
    let loose_chunk = (h * w + rem) / MODEL_DEN;
    let correct_chunk = max_representable_chunk(h, w, rem);
    let delta_b_from_loose = (loose_chunk * MODEL_DEN + rem) / w;

    assert!(loose_chunk > correct_chunk);
    assert!(delta_b_from_loose > h);
}

#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn proof_healthy_hmin_zero_lane_is_not_raised_to_cfg_min() {
    let admit_h_max: u8 = kani::any();
    kani::assume(admit_h_max > 0);

    assert!(effective_h_lock(0, admit_h_max, false) == 0);
    assert!(effective_h_lock(0, admit_h_max, true) == admit_h_max);
}

#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn proof_liquidation_fee_shortfall_is_not_socialized_through_b() {
    let bankruptcy_loss: u8 = kani::any();
    let liquidation_fee_shortfall: u8 = kani::any();

    let (residual_to_socialize, dropped_fee) =
        bankruptcy_residual_excluding_fee_shortfall(bankruptcy_loss, liquidation_fee_shortfall);

    assert!(residual_to_socialize == bankruptcy_loss);
    assert!(dropped_fee == liquidation_fee_shortfall);
}

#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn proof_recovery_price_uses_authenticated_price_or_non_forced_p_last_fallback() {
    let input = RecoveryInputModel {
        ordinary_progress_possible: kani::any(),
        authenticated_price_available: kani::any(),
        authenticated_price: kani::any(),
        p_last: kani::any(),
        fallback_enabled: kani::any(),
        caller_omitted_or_corrupted_proof: kani::any(),
    };

    let price = recovery_price(input);

    if input.ordinary_progress_possible || input.p_last == 0 {
        assert!(price.is_none());
    }
    if input.authenticated_price_available
        && input.authenticated_price > 0
        && !input.ordinary_progress_possible
        && input.p_last > 0
    {
        assert!(price == Some(input.authenticated_price));
    }
    if !input.authenticated_price_available
        && input.caller_omitted_or_corrupted_proof
        && !input.ordinary_progress_possible
    {
        assert!(
            price.is_none(),
            "caller proof omission/corruption must not force P_last fallback"
        );
    }
    if let Some(p) = price {
        assert!(p > 0);
        assert!(p == input.authenticated_price || p == input.p_last);
    }
}

#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn proof_public_instruction_work_is_bounded_not_whole_market() {
    let work_items: u8 = kani::any();
    let accepted = public_instruction_work_allowed(work_items);

    if accepted {
        assert!(work_items <= MODEL_MAX_PUBLIC_WORK);
    }
    if work_items > MODEL_MAX_PUBLIC_WORK {
        assert!(!accepted);
    }
}

#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn proof_weight_change_flushes_global_b_remainder() {
    let rem: u8 = kani::any();
    let dust: u8 = kani::any();
    kani::assume((rem as u32) < MODEL_DEN);
    kani::assume((dust as u32) < MODEL_DEN);

    let (new_rem, new_dust, whole_loss) = flush_remainder_to_dust(rem, dust);

    assert!(new_rem == 0);
    assert!((new_dust as u32) < MODEL_DEN);
    assert!(
        rem as u32 + dust as u32 == new_dust as u32 + whole_loss as u32 * MODEL_DEN,
        "scaled remainder value must be conserved across weight-sum changes"
    );
}

#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn proof_zero_effective_member_blocks_future_b_booking() {
    let certified_weight_sum: bool = kani::any();
    let known_zero_effective_member: bool = kani::any();

    let allowed = b_booking_allowed(certified_weight_sum, known_zero_effective_member);

    if known_zero_effective_member {
        assert!(
            !allowed,
            "future B booking must not include known zero-effective accounts"
        );
    }
    if allowed {
        assert!(certified_weight_sum);
        assert!(!known_zero_effective_member);
    }
}

#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn proof_position_closed_phase_preserves_public_exposure_invariants() {
    let basis_live: bool = kani::any();
    let oi: u8 = kani::any();
    let weight_sum: u8 = kani::any();
    let q_close: u8 = kani::any();
    let account_weight: u8 = kani::any();

    let result = atomic_position_close(basis_live, oi, weight_sum, q_close, account_weight);
    if let Some((basis_after, oi_after, weight_sum_after)) = result {
        assert!(!basis_after);
        assert!(q_close > 0);
        assert!(q_close <= oi);
        assert!(account_weight <= weight_sum);
        assert!(oi_after == oi - q_close);
        assert!(weight_sum_after == weight_sum - account_weight);
    }

    kani::cover!(
        atomic_position_close(true, 2, 2, 1, 1).is_some(),
        "nontrivial atomic close step is reachable"
    );
}

#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn proof_explicit_unallocated_loss_is_durable_before_clear_and_not_payout_capacity() {
    let state = ClaimModel {
        vault: kani::any(),
        c_tot: kani::any(),
        insurance: kani::any(),
        pnl_pos_tot: kani::any(),
        explicit_unallocated: kani::any(),
        residual_remaining: kani::any(),
        account_cleared: false,
    };
    kani::assume(state.residual_remaining > 0);
    kani::assume(state.explicit_unallocated <= u16::MAX - state.residual_remaining);
    kani::assume(state.vault >= state.c_tot);
    kani::assume(state.vault - state.c_tot >= state.insurance);

    let residual_before = state.vault - state.c_tot - state.insurance;
    let result = record_explicit_loss_then_clear(state);

    if let Some(next) = result {
        let residual_after = next.vault - next.c_tot - next.insurance;
        assert!(next.account_cleared);
        assert!(next.residual_remaining == 0);
        assert!(next.explicit_unallocated - state.explicit_unallocated == state.residual_remaining);
        assert!(next.vault == state.vault);
        assert!(next.c_tot == state.c_tot);
        assert!(next.insurance == state.insurance);
        assert!(next.pnl_pos_tot == state.pnl_pos_tot);
        assert!(
            residual_after == residual_before,
            "explicit loss recording must not increase payout residual"
        );
    }
}

#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn proof_crankforward_requires_recovery_or_impossibility_proof() {
    let recovery_enabled: bool = kani::any();
    let impossible_proof: bool = kani::any();

    let ok = valid_liveness_profile(1, recovery_enabled, impossible_proof);

    assert!(ok == (recovery_enabled || impossible_proof));
    assert!(!valid_liveness_profile(1, false, false));
    assert!(valid_liveness_profile(1, true, false));
    assert!(valid_liveness_profile(1, false, true));
    assert!(valid_liveness_profile(0, false, false));
    assert!(!valid_liveness_profile(2, true, true));
}

#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn proof_public_user_fund_profile_requires_crankforward_recovery_and_chunk_caps() {
    let profile: u8 = kani::any();
    let recovery_enabled: bool = kani::any();
    let bankrupt_close_chunks: u8 = kani::any();
    let account_b_chunks: u8 = kani::any();
    let public_b_chunk_atoms: u8 = kani::any();

    let ok = valid_public_user_fund_profile(
        profile,
        recovery_enabled,
        bankrupt_close_chunks,
        account_b_chunks,
        public_b_chunk_atoms,
    );

    if ok {
        assert!(profile == 1);
        assert!(recovery_enabled);
        assert!(bankrupt_close_chunks > 0);
        assert!(account_b_chunks > 0);
        assert!((public_b_chunk_atoms as u32) >= MODEL_DEN);
    }
    assert!(!valid_public_user_fund_profile(
        0,
        true,
        1,
        1,
        MODEL_DEN as u8
    ));
    assert!(!valid_public_user_fund_profile(
        1,
        false,
        1,
        1,
        MODEL_DEN as u8
    ));
    assert!(!valid_public_user_fund_profile(
        1,
        true,
        0,
        1,
        MODEL_DEN as u8
    ));
    assert!(!valid_public_user_fund_profile(
        1,
        true,
        1,
        0,
        MODEL_DEN as u8
    ));
    assert!(!valid_public_user_fund_profile(
        1,
        true,
        1,
        1,
        (MODEL_DEN - 1) as u8
    ));
}
