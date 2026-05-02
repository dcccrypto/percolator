//! B-index bankruptcy socialization spec-model proofs.
//!
//! These proofs are small symbolic models for the v12.20.1 B-index rules.
//! They are intentionally independent from the current v12.19 engine structs:
//! the production engine has not yet implemented B-index state.

#![cfg(kani)]

const MODEL_DEN: u32 = 16;
const MODEL_MAX_B: u32 = 31;

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

fn max_representable_chunk(h: u32, w: u32, rem: u32) -> u32 {
    let max_scaled = (h + 1) * w - 1;
    if rem > max_scaled {
        0
    } else {
        (max_scaled - rem) / MODEL_DEN
    }
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
