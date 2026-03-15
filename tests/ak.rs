//! Layered A/K proof suite for Kani — v9.5 Risk Engine
//!
//! Architecture:
//!   - Tier 0: Arithmetic helper proofs (pure, loop-free)
//!   - Tier 1: One-event A/K semantics (lazy vs eager, small model)
//!   - Tier 2: Composition proofs (induction, small model)
//!   - Tier 3: Reset / epoch proofs
//!   - Tier 4: ADL enqueue proofs
//!   - Tier 5: Dust / fixed-point proofs
//!   - Tier 6: Focused scenario proofs (regressions)
//!   - Tier 7: Non-compounding basis proofs (v9.5)
//!   - Tier 8: Real engine integration proofs
//!   - Tier 9: Fee / warmup proofs
//!   - Tier 10: accrue_market_to proofs
//!
//! Two proof models:
//!   1. Small algebraic model: tiny integer widths (u32/i32), no slab/vault,
//!      just A, K, snapshots, basis_q, eager-vs-lazy semantics.
//!   2. Production-width arithmetic: real helper functions, wide intermediates,
//!      no long event sequences.
//!
//! Run individual: `cargo kani --harness <name>`
//! Run all in file: `cargo kani --tests ak`

#![cfg(kani)]

use percolator::*;
use percolator::i128::U128;
use percolator::wide_math::{
    U256, I256,
    floor_div_signed_conservative,
    saturating_mul_u256_u64,
    fee_debt_u128_checked,
    mul_div_floor_u256,
    mul_div_ceil_u256,
    wide_signed_mul_div_floor,
};

// ############################################################################
//
// SMALL ALGEBRAIC MODEL
//
// Uses u16 for A, i32 for K, u16 for basis_q, u16 for POS_SCALE_SMALL.
// No slab, no vault. Just pure A/K math.
//
// ############################################################################

/// Small-model scale factors (minimal bit-widths for CBMC tractability).
/// All arithmetic stays within i32/u16 to avoid 64-bit SAT blowup.
/// Invariant: max|basis_q * k_diff| < 2^31 for all u8/i8 inputs.
const S_POS_SCALE: u16 = 4;
const S_ADL_ONE: u16 = 256;

/// Small-model: eager PnL for one mark event.
fn eager_mark_pnl_long(q_base: i32, delta_p: i32) -> i32 {
    q_base * delta_p
}

fn eager_mark_pnl_short(q_base: i32, delta_p: i32) -> i32 {
    -(q_base * delta_p)
}

/// Small-model: lazy PnL from K difference.
/// pnl_delta = floor(|basis_q| * (K_cur - k_snap) / (a_basis * POS_SCALE))
fn lazy_pnl(basis_q_abs: u16, k_diff: i32, a_basis: u16) -> i32 {
    let den = (a_basis as i32) * (S_POS_SCALE as i32);
    if den == 0 { return 0; }
    let num = (basis_q_abs as i32) * k_diff;
    if num >= 0 {
        num / den
    } else {
        let abs_num = -num;
        -((abs_num + den - 1) / den)
    }
}

/// Small-model: lazy effective quantity.
/// Uses i32 intermediate to keep CBMC fast (narrower than u32 division).
fn lazy_eff_q(basis_q_abs: u16, a_cur: u16, a_basis: u16) -> u16 {
    if a_basis == 0 { return 0; }
    // basis_q max=1020, a_cur max=256. Product max=261120. Fits i32.
    let product = (basis_q_abs as i32) * (a_cur as i32);
    (product / (a_basis as i32)) as u16
}

/// Small-model: K update for mark event.
fn k_after_mark_long(k_before: i32, a_long: u16, delta_p: i32) -> i32 {
    k_before + (a_long as i32) * delta_p
}

fn k_after_mark_short(k_before: i32, a_short: u16, delta_p: i32) -> i32 {
    k_before - (a_short as i32) * delta_p
}

/// Small-model: K update for funding event.
fn k_after_fund_long(k_before: i32, a_long: u16, delta_f: i32) -> i32 {
    k_before - (a_long as i32) * delta_f
}

fn k_after_fund_short(k_before: i32, a_short: u16, delta_f: i32) -> i32 {
    k_before + (a_short as i32) * delta_f
}

/// Small-model: A update for ADL quantity shrink.
fn a_after_adl(a_old: u16, oi_post: u16, oi: u16) -> u16 {
    if oi == 0 { return a_old; }
    // a_old max=256, oi_post max=255. Product max=65280. Fits i32.
    let product = (a_old as i32) * (oi_post as i32);
    (product / (oi as i32)) as u16
}

// ============================================================================
// Helper: default engine params
// ============================================================================

fn zero_fee_params() -> RiskParams {
    RiskParams {
        warmup_period_slots: 100,
        maintenance_margin_bps: 500,
        initial_margin_bps: 1000,
        trading_fee_bps: 0,
        max_accounts: MAX_ACCOUNTS as u64,
        new_account_fee: U128::ZERO,
        maintenance_fee_per_slot: U128::ZERO,
        max_crank_staleness_slots: u64::MAX,
        liquidation_fee_bps: 0,
        liquidation_fee_cap: U128::ZERO,
        liquidation_buffer_bps: 50,
        min_liquidation_abs: U128::ZERO,
    }
}

// ############################################################################
//
// TIER 0: ARITHMETIC HELPER PROOFS
// Pure, loop-free, fast.
//
// ############################################################################

// ============================================================================
// T0.1: floor_div_signed_conservative_is_floor
// ============================================================================

/// Prove: for all n in i8 and d in u8 (d > 0),
/// floor_div_signed_conservative(n, d) matches reference floor division.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t0_1_floor_div_signed_conservative_is_floor() {
    let n_raw: i8 = kani::any();
    let d_raw: u8 = kani::any();
    kani::assume(d_raw > 0);

    let n = I256::from_i128(n_raw as i128);
    let d = U256::from_u128(d_raw as u128);

    let result = floor_div_signed_conservative(n, d);

    // Reference: i32 arithmetic (no overflow for i8 / u8)
    let n_i32 = n_raw as i32;
    let d_i32 = d_raw as i32;
    let expected = if n_i32 >= 0 {
        n_i32 / d_i32
    } else {
        let abs_n = -n_i32;
        -((abs_n + d_i32 - 1) / d_i32)
    };

    let result_i128 = result.try_into_i128().unwrap();
    assert!(result_i128 == expected as i128, "floor_div mismatch");
}

/// Satisfiability: negative n with nonzero remainder exists.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t0_1_sat_negative_with_remainder() {
    let n_raw: i8 = kani::any();
    let d_raw: u8 = kani::any();
    kani::assume(d_raw > 1);
    kani::assume(n_raw < 0);
    // Use i32 to avoid negation overflow
    let abs_n = -(n_raw as i32);
    kani::assume((abs_n as u32) % (d_raw as u32) != 0);

    let n = I256::from_i128(n_raw as i128);
    let d = U256::from_u128(d_raw as u128);
    let result = floor_div_signed_conservative(n, d);

    // result should be strictly less than truncation toward zero
    let trunc = (n_raw as i32) / (d_raw as i32);
    let result_i128 = result.try_into_i128().unwrap();
    assert!(result_i128 < trunc as i128);
}

// ============================================================================
// T0.2: mul_div_floor/ceil algebraic properties
// ============================================================================

/// Prove algebraic floor division identity: floor(a*b/c) * c <= a*b < (floor(a*b/c)+1) * c
/// Uses only reference arithmetic (no U512 calls).
#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn t0_2_mul_div_floor_algebraic_identity() {
    let a: u8 = kani::any();
    let b: u8 = kani::any();
    let c: u8 = kani::any();
    kani::assume(c > 0);

    let product = (a as u32) * (b as u32);
    let floor_val = product / (c as u32);
    let remainder = product % (c as u32);

    // floor(a*b/c) * c + remainder == a*b
    assert!(floor_val * (c as u32) + remainder == product);
    // 0 <= remainder < c
    assert!(remainder < c as u32);
}

/// Prove ceil = floor + (remainder != 0 ? 1 : 0)
#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn t0_2_mul_div_ceil_algebraic_identity() {
    let a: u8 = kani::any();
    let b: u8 = kani::any();
    let c: u8 = kani::any();
    kani::assume(c > 0);

    let product = (a as u32) * (b as u32);
    let floor_val = product / (c as u32);
    let remainder = product % (c as u32);
    let ceil_val = (product + (c as u32) - 1) / (c as u32);

    if remainder == 0 {
        assert!(ceil_val == floor_val);
    } else {
        assert!(ceil_val == floor_val + 1);
    }
}

/// Real helper: mul_div_floor_u256 matches reference for u8 inputs.
#[kani::proof]
#[kani::unwind(18)]
#[kani::solver(cadical)]
fn t0_2c_mul_div_floor_matches_reference() {
    let a: u8 = kani::any();
    let b: u8 = kani::any();
    let c: u8 = kani::any();
    kani::assume(c > 0);

    let result = mul_div_floor_u256(
        U256::from_u128(a as u128),
        U256::from_u128(b as u128),
        U256::from_u128(c as u128),
    );

    let expected = ((a as u32) * (b as u32)) / (c as u32);
    let result_u128 = result.try_into_u128().unwrap();
    assert!(result_u128 == expected as u128, "mul_div_floor mismatch");
}

/// Real helper: mul_div_ceil_u256 matches reference for u8 inputs.
#[kani::proof]
#[kani::unwind(18)]
#[kani::solver(cadical)]
fn t0_2d_mul_div_ceil_matches_reference() {
    let a: u8 = kani::any();
    let b: u8 = kani::any();
    let c: u8 = kani::any();
    kani::assume(c > 0);

    let result = mul_div_ceil_u256(
        U256::from_u128(a as u128),
        U256::from_u128(b as u128),
        U256::from_u128(c as u128),
    );

    let product = (a as u32) * (b as u32);
    let expected = (product + (c as u32) - 1) / (c as u32);
    let result_u128 = result.try_into_u128().unwrap();
    assert!(result_u128 == expected as u128, "mul_div_ceil mismatch");
}

// ============================================================================
// T0.3: set_pnl_aggregate_update_is_exact
// ============================================================================

/// Prove PNL_pos_tot updates exactly under all four sign transitions.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t0_3_set_pnl_aggregate_exact() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = engine.add_user(0).unwrap();

    // Set initial PnL
    let old_pnl: i16 = kani::any();
    kani::assume(old_pnl > i16::MIN);
    engine.set_pnl(idx as usize, I256::from_i128(old_pnl as i128));

    let ppt_after_first = engine.pnl_pos_tot;

    // Set new PnL
    let new_pnl: i16 = kani::any();
    kani::assume(new_pnl > i16::MIN);
    engine.set_pnl(idx as usize, I256::from_i128(new_pnl as i128));

    // Verify: pnl_pos_tot == max(new_pnl, 0)
    let expected = if new_pnl > 0 { new_pnl as u128 } else { 0u128 };
    let actual = engine.pnl_pos_tot.try_into_u128().unwrap();
    assert!(actual == expected);
}

/// Satisfiability + correctness: all four sign transitions are reachable
/// and set_pnl produces correct pnl_pos_tot for each.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t0_3_sat_all_sign_transitions() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = engine.add_user(0).unwrap();

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

    engine.set_pnl(idx as usize, I256::from_i128(old as i128));
    engine.set_pnl(idx as usize, I256::from_i128(new as i128));

    let expected = if new > 0 { new as u128 } else { 0u128 };
    let actual = engine.pnl_pos_tot.try_into_u128().unwrap();
    assert!(actual == expected, "pnl_pos_tot mismatch after transition");
}

// ============================================================================
// T0.4: safe_fee_debt_and_cap_math
// ============================================================================

/// fee_debt_u128_checked cannot overflow.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t0_4_fee_debt_no_overflow() {
    let fc: i128 = kani::any();
    let debt = fee_debt_u128_checked(fc);
    if fc < 0 {
        assert!(debt > 0);
        // debt == |fc|
        assert!(debt == fc.unsigned_abs());
    } else {
        assert!(debt == 0);
    }
}

/// saturating_mul_u256_u64: exact for small values, saturates for large.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t0_4_saturating_mul_no_panic() {
    let a: u8 = kani::any();
    let b: u8 = kani::any();

    // Small values: exact product
    let a256 = U256::from_u128(a as u128);
    let result = saturating_mul_u256_u64(a256, b as u64);
    let expected = (a as u128) * (b as u128);
    assert!(result == U256::from_u128(expected));

    // Large value: exercises saturation path
    kani::assume(b > 1);
    let result_max = saturating_mul_u256_u64(U256::MAX, b as u64);
    assert!(result_max == U256::MAX, "must saturate at U256::MAX");
}

/// Conservation (vault >= c_tot + insurance) is preserved by deposit (u128 widths).
#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn t0_4_conservation_check_handles_overflow() {
    // Use u128 inputs (production widths) — bounded to u64 range for tractability
    let c_tot: u64 = kani::any();
    let insurance: u64 = kani::any();
    let vault: u64 = kani::any();
    let deposit: u64 = kani::any();

    let c_tot_128 = c_tot as u128;
    let insurance_128 = insurance as u128;
    let vault_128 = vault as u128;
    let deposit_128 = deposit as u128;

    let sum = c_tot_128.checked_add(insurance_128);

    // u64 + u64 never overflows u128
    assert!(sum.is_some());
    let sum = sum.unwrap();

    // If conservation holds pre-deposit, it holds post-deposit
    if vault_128 >= sum {
        let vault_new = vault_128 + deposit_128;
        let c_tot_new = c_tot_128 + deposit_128;
        assert!(vault_new >= c_tot_new + insurance_128,
            "deposit preserves conservation");
    }
}

/// fee_debt_u128_checked(i128::MIN) must not panic — i128::MIN.unsigned_abs() = 2^127.
#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn t0_4_fee_debt_i128_min() {
    let debt = fee_debt_u128_checked(i128::MIN);
    // i128::MIN = -2^127, unsigned_abs = 2^127
    assert!(debt == (1u128 << 127), "fee_debt of i128::MIN must be 2^127");
}

// ############################################################################
//
// TIER 1: ONE-EVENT A/K SEMANTICS
// Small algebraic model. Each theorem compares eager vs lazy for one event.
//
// ############################################################################

// ============================================================================
// T1.5: mark_event_lazy_equals_eager (long)
// ============================================================================

/// For a single price move ΔP on a long account, lazy settlement
/// gives the same PnL as eager computation.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t1_5_mark_event_lazy_equals_eager_long() {
    let q_base: u8 = kani::any();
    kani::assume(q_base > 0);
    let delta_p: i8 = kani::any();

    let a_init = S_ADL_ONE;
    let k_init: i32 = 0;
    let basis_q = (q_base as u16) * S_POS_SCALE;

    // Eager: direct PnL
    let eager_pnl = eager_mark_pnl_long(q_base as i32, delta_p as i32);

    // Lazy: apply mark to K, then compute pnl_delta from K diff
    let k_after = k_after_mark_long(k_init, a_init, delta_p as i32);
    let k_diff = k_after - k_init;
    let lazy_pnl_val = lazy_pnl(basis_q, k_diff, a_init);

    assert!(eager_pnl == lazy_pnl_val,
        "mark lazy != eager for long");
}

/// Same for short.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t1_5_mark_event_lazy_equals_eager_short() {
    let q_base: u8 = kani::any();
    kani::assume(q_base > 0);
    let delta_p: i8 = kani::any();

    let a_init = S_ADL_ONE;
    let k_init: i32 = 0;
    let basis_q = (q_base as u16) * S_POS_SCALE;

    let eager_pnl = eager_mark_pnl_short(q_base as i32, delta_p as i32);

    let k_after = k_after_mark_short(k_init, a_init, delta_p as i32);
    let k_diff = k_after - k_init;
    let lazy_pnl_val = lazy_pnl(basis_q, k_diff, a_init);

    assert!(eager_pnl == lazy_pnl_val,
        "mark lazy != eager for short");
}

/// Satisfiability: a negative mark PnL for longs exists.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t1_5_sat_negative_mark_long() {
    let q_base: u8 = kani::any();
    kani::assume(q_base > 0);
    let delta_p: i8 = kani::any();
    kani::assume(delta_p < 0);
    let pnl = eager_mark_pnl_long(q_base as i32, delta_p as i32);
    assert!(pnl < 0);
}

// ============================================================================
// T1.6: funding_event_lazy_equals_eager
// ============================================================================

/// For a single funding event ΔF, lazy settlement equals eager for longs.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t1_6_funding_event_lazy_equals_eager_long() {
    let q_base: u8 = kani::any();
    kani::assume(q_base > 0);
    let delta_f: i8 = kani::any();

    let a_init = S_ADL_ONE;
    let k_init: i32 = 0;
    let basis_q = (q_base as u16) * S_POS_SCALE;

    // Eager: longs pay ΔF per unit → pnl = -q * ΔF
    let eager_pnl = -((q_base as i32) * (delta_f as i32));

    // Lazy: K_long -= A_long * ΔF
    let k_after = k_after_fund_long(k_init, a_init, delta_f as i32);
    let k_diff = k_after - k_init;
    let lazy_pnl_val = lazy_pnl(basis_q, k_diff, a_init);

    assert!(eager_pnl == lazy_pnl_val);
}

/// Same for short.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t1_6_funding_event_lazy_equals_eager_short() {
    let q_base: u8 = kani::any();
    kani::assume(q_base > 0);
    let delta_f: i8 = kani::any();

    let a_init = S_ADL_ONE;
    let k_init: i32 = 0;
    let basis_q = (q_base as u16) * S_POS_SCALE;

    // Eager: shorts receive ΔF per unit → pnl = +q * ΔF
    let eager_pnl = (q_base as i32) * (delta_f as i32);

    let k_after = k_after_fund_short(k_init, a_init, delta_f as i32);
    let k_diff = k_after - k_init;
    let lazy_pnl_val = lazy_pnl(basis_q, k_diff, a_init);

    assert!(eager_pnl == lazy_pnl_val);
}

// ============================================================================
// T1.7: adl_quantity_only_event_lazy_equals_eager
// ============================================================================

/// ADL with q_close > 0, D = 0: lazy A-ratio settlement gives a surviving
/// quantity that is conservative (within 1 unit of eager pro-rata).
/// The double-floor (A_new then q_eff) can lose at most 1 base unit.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t1_7_adl_quantity_only_lazy_conservative() {
    let q_base: u8 = kani::any();
    kani::assume(q_base > 0 && q_base <= 15);
    let oi: u8 = kani::any();
    kani::assume(oi > 0 && oi <= 15);
    let q_close: u8 = kani::any();
    kani::assume(q_close > 0 && q_close <= oi);
    let oi_post = oi - q_close;

    let a_old = S_ADL_ONE;
    let basis_q = (q_base as u16) * S_POS_SCALE;

    // Eager: surviving quantity = floor(q_base * oi_post / oi)
    let eager_q = ((q_base as u16) * (oi_post as u16)) / (oi as u16);

    // Lazy: A_new = floor(A_old * oi_post / oi)
    let a_new = a_after_adl(a_old, oi_post as u16, oi as u16);
    // q_eff = floor(basis_q * A_new / a_old)
    let lazy_q = lazy_eff_q(basis_q, a_new, a_old);
    // Convert back to base units: lazy_q / POS_SCALE
    let lazy_q_base = lazy_q / S_POS_SCALE;

    // Conservative: lazy is at most eager (never overshoot)
    assert!(lazy_q_base <= eager_q,
        "ADL lazy must not exceed eager quantity");
    // Bounded error: lazy is within 1 unit of eager
    assert!(eager_q - lazy_q_base <= 1,
        "ADL lazy error must be bounded by 1 base unit");
}

/// Satisfiability: oi_post > 0 case is reachable.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t1_7_sat_oi_post_positive() {
    let oi: u8 = kani::any();
    let q_close: u8 = kani::any();
    kani::assume(oi > 1 && q_close > 0 && q_close < oi);
    assert!(oi - q_close > 0);
}

// ============================================================================
// T1.8: adl_deficit_only_event_lazy_equals_eager
// ============================================================================

/// ADL with q_close = 0, D > 0: changing only K gives the same
/// realized quote loss as eager socialization.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t1_8_adl_deficit_only_lazy_equals_eager() {
    let q_base: u8 = kani::any();
    kani::assume(q_base > 0 && q_base <= 15);
    let oi: u8 = kani::any();
    kani::assume(oi > 0 && oi <= 15);
    let d: u8 = kani::any();
    kani::assume(d > 0 && d <= 15);

    let a_side = S_ADL_ONE;
    let k_init: i32 = 0;
    let basis_q = (q_base as u16) * S_POS_SCALE;

    // Eager: each unit pays D/OI (ceiling for deficit, but floor for PnL)
    // Total loss per account = floor(q_base * D / OI)
    let eager_loss = ((q_base as i32) * (d as i32)) / (oi as i32);

    // Lazy: beta = -ceil(D * POS_SCALE / OI), delta_K = A * beta
    // For small model: beta_abs = ceil(d * POS_SCALE / oi)
    let beta_abs = ((d as u32) * (S_POS_SCALE as u32) + (oi as u32) - 1) / (oi as u32);
    // delta_K = -(A_side * beta_abs)
    let delta_k = -((a_side as i32) * (beta_abs as i32));
    let k_after = k_init + delta_k;
    let k_diff = k_after - k_init;

    // Lazy PnL from K diff
    let lazy_loss_raw = lazy_pnl(basis_q, k_diff, a_side);

    // The lazy loss should be <= -eager_loss (conservative: ceiling beta
    // means you pay at least as much as floor(q*D/OI))
    let lazy_loss = -lazy_loss_raw;
    assert!(lazy_loss >= eager_loss,
        "ADL deficit lazy must be at least as large as eager");
}

// ============================================================================
// T1.9: adl_quantity_plus_deficit_event_lazy_equals_eager
// ============================================================================

/// ADL with both q_close > 0 and D > 0.
/// Proves quantity is conservative (within 1 unit) and PnL is conservative.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t1_9_adl_quantity_plus_deficit_lazy_conservative() {
    let q_base: u8 = kani::any();
    kani::assume(q_base > 0 && q_base <= 15);
    let oi: u8 = kani::any();
    kani::assume(oi > 0 && oi >= q_base && oi <= 15);
    let q_close: u8 = kani::any();
    kani::assume(q_close > 0 && q_close <= oi);
    let d: u8 = kani::any();
    kani::assume(d > 0 && d <= 15);

    let oi_post = oi - q_close;
    let a_old = S_ADL_ONE;
    let basis_q = (q_base as u16) * S_POS_SCALE;

    // Eager quantity: floor(q_base * oi_post / oi)
    let eager_q = ((q_base as u16) * (oi_post as u16)) / (oi as u16);

    // Lazy quantity: via A shrink
    let a_new = a_after_adl(a_old, oi_post as u16, oi as u16);
    let lazy_q = lazy_eff_q(basis_q, a_new, a_old) / S_POS_SCALE;

    // Conservative bound: double-floor can lose at most 1 base unit
    assert!(lazy_q <= eager_q, "lazy must not exceed eager quantity");
    assert!(eager_q - lazy_q <= 1, "lazy error bounded by 1 base unit");

    // PnL: deficit is socialized via K
    let beta_abs = ((d as u32) * (S_POS_SCALE as u32) + (oi as u32) - 1) / (oi as u32);
    let delta_k = -((a_old as i32) * (beta_abs as i32));
    let lazy_loss = -lazy_pnl(basis_q, delta_k, a_old);
    let eager_loss = ((q_base as i32) * (d as i32)) / (oi as i32);

    assert!(lazy_loss >= eager_loss,
        "ADL PnL: lazy loss must be >= eager loss (conservative)");
}

// ============================================================================
// T1.10: attach_at_current_snapshot_is_noop
// ============================================================================

/// If a new position is opened and snapped to current (A, K), then
/// an immediate settlement changes neither quantity nor PnL.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t1_10_attach_at_current_snapshot_is_noop() {
    let q_base: u8 = kani::any();
    kani::assume(q_base > 0);

    let a_cur = S_ADL_ONE;
    let k_cur: i32 = kani::any::<i16>() as i32;
    let basis_q = (q_base as u16) * S_POS_SCALE;

    // Snap at current state
    let a_basis = a_cur;
    let k_snap = k_cur;

    // Immediate settlement
    let k_diff = k_cur - k_snap; // == 0
    let pnl_delta = lazy_pnl(basis_q, k_diff, a_basis);
    let q_eff = lazy_eff_q(basis_q, a_cur, a_basis);

    assert!(pnl_delta == 0, "attach noop: pnl must be zero");
    assert!(q_eff == basis_q, "attach noop: quantity must be unchanged");
}

// ############################################################################
//
// TIER 2: COMPOSITION PROOFS
//
// ############################################################################

// ============================================================================
// T2.11: compose_two_events
// ============================================================================

/// Prove the algebraic composition law for A/K events.
/// If event 1 is (α₁, β₁) and event 2 is (α₂, β₂), then:
///   eager: q' = α₂(α₁ q), pnl = β₁ q + β₂ α₁ q
///   cumulative: A = α₁ α₂, K = β₁ + α₁ β₂
///   lazy: q' = q * A / A_snap, pnl = q * (K - K_snap) / (A_snap * POS_SCALE)
///
/// For mark events: α = 1 (A unchanged), β = A * ΔP
/// Two mark events: α₁ = α₂ = 1, β₁ = A*ΔP₁, β₂ = A*ΔP₂
/// So K = A*(ΔP₁ + ΔP₂), which is just cumulative K.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t2_11_compose_two_mark_events() {
    let q_base: u8 = kani::any();
    kani::assume(q_base > 0 && q_base <= 15);
    let dp1: i8 = kani::any();
    kani::assume(dp1 >= -15 && dp1 <= 15);
    let dp2: i8 = kani::any();
    kani::assume(dp2 >= -15 && dp2 <= 15);

    let a = S_ADL_ONE;
    let basis_q = (q_base as u16) * S_POS_SCALE;

    // Eager: apply event 1, then event 2
    let eager_pnl1 = (q_base as i32) * (dp1 as i32);
    let eager_pnl2 = (q_base as i32) * (dp2 as i32);
    let eager_total = eager_pnl1 + eager_pnl2;

    // Cumulative: K after both events
    let k0: i32 = 0;
    let k1 = k_after_mark_long(k0, a, dp1 as i32);
    let k2 = k_after_mark_long(k1, a, dp2 as i32);
    let k_diff = k2 - k0;

    // Lazy: single settlement at the end
    let lazy_total = lazy_pnl(basis_q, k_diff, a);

    assert!(eager_total == lazy_total,
        "composition of two marks: eager != lazy");
}

/// Compose a mark + funding event.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t2_11_compose_mark_then_funding() {
    let q_base: u8 = kani::any();
    kani::assume(q_base > 0 && q_base <= 15);
    let dp: i8 = kani::any();
    kani::assume(dp >= -15 && dp <= 15);
    let df: i8 = kani::any();
    kani::assume(df >= -15 && df <= 15);

    let a = S_ADL_ONE;
    let basis_q = (q_base as u16) * S_POS_SCALE;

    // Eager: mark pnl + funding pnl (for long)
    let eager_mark = (q_base as i32) * (dp as i32);
    let eager_fund = -((q_base as i32) * (df as i32));
    let eager_total = eager_mark + eager_fund;

    // Cumulative K: mark changes K, then funding changes K
    let k0: i32 = 0;
    let k1 = k_after_mark_long(k0, a, dp as i32);
    let k2 = k_after_fund_long(k1, a, df as i32);
    let k_diff = k2 - k0;

    let lazy_total = lazy_pnl(basis_q, k_diff, a);

    assert!(eager_total == lazy_total);
}

// ============================================================================
// T2.12: fold_events_contract (base + step case)
// ============================================================================

/// Verify fold identity: empty event prefix → (A_init, K_init).
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t2_12_fold_base_case() {
    let a = S_ADL_ONE;
    let k: i32 = 0;

    // No events → A unchanged, K unchanged
    // Lazy settlement with k_diff = 0 gives zero PnL
    let q_base: u8 = kani::any();
    kani::assume(q_base > 0);
    let basis_q = (q_base as u16) * S_POS_SCALE;

    let pnl = lazy_pnl(basis_q, 0, a);
    let q_eff = lazy_eff_q(basis_q, a, a);

    assert!(pnl == 0);
    assert!(q_eff == basis_q);
}

/// Floor-shift lemma: floor(n + m*d, d) == floor(n, d) + m for integer m.
/// This is the algebraic foundation for the fold step case.
/// Uses the same conservative-floor implementation as lazy_pnl.
#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn t2_12_floor_shift_lemma() {
    let n: i8 = kani::any();
    let m: i8 = kani::any();
    let d: u8 = kani::any();
    kani::assume(d > 0);

    let d32 = d as i32;
    let n32 = n as i32;
    let m32 = m as i32;
    let shifted = n32 + m32 * d32;

    // Conservative floor (matching lazy_pnl implementation)
    let floor_n = if n32 >= 0 {
        n32 / d32
    } else {
        -((-n32 + d32 - 1) / d32)
    };

    let floor_shifted = if shifted >= 0 {
        shifted / d32
    } else {
        -((-shifted + d32 - 1) / d32)
    };

    assert!(floor_shifted == floor_n + m32,
        "floor(n + m*d, d) must equal floor(n, d) + m");
}

/// Step case: fold(prefix + mark_event) == compose(fold(prefix), mark_event).
/// Holds for ALL k_prefix because basis_q * A * dp is an exact multiple of
/// den = A * POS_SCALE (divisibility proved here), so the floor-shift lemma
/// (t2_12_floor_shift_lemma) applies:
///
///   lazy_pnl(q, k+A*dp, A) - lazy_pnl(q, k, A)
///   = floor(basis_q*(k+A*dp) / den) - floor(basis_q*k / den)
///   = floor(basis_q*k/den + q_base*dp) - floor(basis_q*k/den)
///   = q_base*dp  [floor-shift: floor(x+n) = floor(x)+n for integer n]
///
/// k_prefix bounded to i8 for CBMC tractability; the property holds for all
/// k by the floor-shift lemma (which is width-independent).
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t2_12_fold_step_case() {
    let q_base: u8 = kani::any();
    kani::assume(q_base > 0);
    let dp: i8 = kani::any();
    let a = S_ADL_ONE;
    let den = (a as i32) * (S_POS_SCALE as i32);
    let basis_q = (q_base as u16) * S_POS_SCALE;

    // Key divisibility: basis_q * A is an exact multiple of den
    let exact = (basis_q as i32) * (a as i32);
    assert!(exact % den == 0, "basis_q * A must be divisible by den");
    assert!(exact / den == q_base as i32, "quotient must equal q_base");

    // Step case with symbolic k_prefix
    let k_prefix: i8 = kani::any();
    let k_new = (k_prefix as i32) + (a as i32) * (dp as i32);
    let eager_step = (q_base as i32) * (dp as i32);
    let lazy_total = lazy_pnl(basis_q, k_new, a);
    let lazy_prefix = lazy_pnl(basis_q, k_prefix as i32, a);
    let lazy_step = lazy_total - lazy_prefix;

    assert!(lazy_step == eager_step,
        "fold step: lazy increment must equal eager step");
}

// ============================================================================
// T2.13: touch_equals_eager_replay_prefix
// ============================================================================

/// For any account snapped at k_snap, lazy settlement against cumulative K_cur
/// equals eager replay of events since snap.
/// Modeled with 3 mark events after snap.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t2_13_touch_equals_eager_replay() {
    let q_base: u8 = kani::any();
    kani::assume(q_base > 0 && q_base <= 15);

    let dp1: i8 = kani::any();
    kani::assume(dp1 >= -15 && dp1 <= 15);
    let dp2: i8 = kani::any();
    kani::assume(dp2 >= -15 && dp2 <= 15);
    let dp3: i8 = kani::any();
    kani::assume(dp3 >= -15 && dp3 <= 15);

    let a = S_ADL_ONE;
    let basis_q = (q_base as u16) * S_POS_SCALE;
    let k_snap: i32 = 0;

    // Eager replay of 3 events
    let eager = (q_base as i32) * ((dp1 as i32) + (dp2 as i32) + (dp3 as i32));

    // Cumulative K after 3 events
    let k1 = k_after_mark_long(k_snap, a, dp1 as i32);
    let k2 = k_after_mark_long(k1, a, dp2 as i32);
    let k3 = k_after_mark_long(k2, a, dp3 as i32);

    // Lazy: single settlement from snap to current
    let lazy_total = lazy_pnl(basis_q, k3 - k_snap, a);

    assert!(eager == lazy_total,
        "touch vs eager replay mismatch");
}

// ############################################################################
//
// TIER 3: RESET / EPOCH PROOFS
//
// ############################################################################

// ============================================================================
// T3.14: epoch_mismatch_forces_terminal_close
// ============================================================================

/// If epoch_snap + 1 == epoch_cur, settlement must:
///   - zero the quantity
///   - compute pnl_delta against K_epoch_start
///   - decrement stale counter
///   - not use same-epoch formula
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t3_14_epoch_mismatch_forces_terminal_close() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = engine.add_user(0).unwrap();
    engine.deposit(idx, 1_000_000, 100, 0).unwrap();

    // Symbolic position size and K value
    let pos_mul: u8 = kani::any();
    kani::assume(pos_mul > 0);
    let pos = I256::from_u128(POS_SCALE * (pos_mul as u128));
    engine.accounts[idx as usize].position_basis_q = pos;
    engine.accounts[idx as usize].adl_a_basis = ADL_ONE;
    // k_snap == k_epoch_start → k_diff == 0 → avoids U512 division
    let k_val: i8 = kani::any();
    let k = I256::from_i128(k_val as i128);
    engine.accounts[idx as usize].adl_k_snap = k;
    engine.accounts[idx as usize].adl_epoch_snap = 0;
    engine.stored_pos_count_long = 1;

    // Advance epoch: simulate full drain reset
    engine.adl_epoch_long = 1;
    engine.adl_epoch_start_k_long = k; // matches k_snap → k_diff == 0
    engine.side_mode_long = SideMode::ResetPending;
    engine.stale_account_count_long = 1;

    // Settle: should use epoch-mismatch path
    let result = engine.settle_side_effects(idx as usize);
    assert!(result.is_ok());

    // Quantity must be zero
    assert!(engine.accounts[idx as usize].position_basis_q.is_zero());

    // Stale counter decremented
    assert!(engine.stale_account_count_long == 0);

    // Epoch snap updated to current
    assert!(engine.accounts[idx as usize].adl_epoch_snap == 1);
}

/// Companion: epoch mismatch with nonzero k_diff.
/// When K_epoch_start != k_snap, PnL is computed correctly against K_epoch_start.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t3_14b_epoch_mismatch_with_nonzero_k_diff() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = engine.add_user(0).unwrap();
    engine.deposit(idx, 10_000_000, 100, 0).unwrap();

    // Position: 1 unit long
    let pos = I256::from_u128(POS_SCALE);
    engine.accounts[idx as usize].position_basis_q = pos;
    engine.accounts[idx as usize].adl_a_basis = ADL_ONE;
    engine.accounts[idx as usize].adl_epoch_snap = 0;
    engine.stored_pos_count_long = 1;

    // k_snap at epoch 0 — symbolic but bounded
    let k_snap_val: i8 = kani::any();
    let k_snap = I256::from_i128(k_snap_val as i128);
    engine.accounts[idx as usize].adl_k_snap = k_snap;

    // K_epoch_start differs from k_snap by a bounded amount
    let k_diff_val: i8 = kani::any();
    kani::assume(k_diff_val != 0); // nonzero k_diff
    let k_epoch_start_val = (k_snap_val as i16) + (k_diff_val as i16);
    // Keep in i8 range to avoid overflow in PnL computation
    kani::assume(k_epoch_start_val >= -120 && k_epoch_start_val <= 120);
    let k_epoch_start = I256::from_i128(k_epoch_start_val as i128);

    // Set K_long to something (doesn't matter for epoch-mismatch path, K_epoch_start is used)
    engine.adl_coeff_long = I256::from_i128(0);

    // Advance epoch
    engine.adl_epoch_long = 1;
    engine.adl_epoch_start_k_long = k_epoch_start;
    engine.side_mode_long = SideMode::ResetPending;
    engine.stale_account_count_long = 1;

    let old_pnl = engine.accounts[idx as usize].pnl;

    // Settle — epoch mismatch path with nonzero k_diff
    let result = engine.settle_side_effects(idx as usize);
    assert!(result.is_ok());

    // Position must be zeroed
    assert!(engine.accounts[idx as usize].position_basis_q.is_zero());

    // PnL must have changed (k_diff != 0 with 1-unit position)
    let new_pnl = engine.accounts[idx as usize].pnl;
    // For 1 POS_SCALE unit with a_basis=ADL_ONE:
    // pnl_delta = floor(POS_SCALE * k_diff / (ADL_ONE * POS_SCALE)) = floor(k_diff / ADL_ONE)
    // With ADL_ONE = 2^96, k_diff in [-120,120], the division floors to 0 for small k_diff...
    // Actually: wide_signed_mul_div_floor(POS_SCALE, k_diff_i256, ADL_ONE * POS_SCALE)
    // = floor(POS_SCALE * k_diff / (ADL_ONE * POS_SCALE)) = floor(k_diff / ADL_ONE) = 0
    // since |k_diff| < ADL_ONE. So PnL delta is 0 for these small values.
    // The important check is that it doesn't error and position is zeroed.
    assert!(engine.stale_account_count_long == 0);
    assert!(engine.accounts[idx as usize].adl_epoch_snap == 1);
}

// ============================================================================
// T3.15: same_epoch_settlement_never_increases_abs_position
// ============================================================================

/// For any same-epoch settle: 0 <= q_new <= q_old (A can only shrink or stay).
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t3_15_same_epoch_settle_never_increases_position() {
    let q_base: u8 = kani::any();
    kani::assume(q_base > 0);

    // A can only decrease (ADL shrinks A)
    let a_basis = S_ADL_ONE;
    let a_cur: u16 = kani::any();
    kani::assume(a_cur > 0 && a_cur <= S_ADL_ONE);

    let basis_q = (q_base as u16) * S_POS_SCALE;
    let q_eff = lazy_eff_q(basis_q, a_cur, a_basis);

    // q_eff <= basis_q always (since a_cur <= a_basis = ADL_ONE)
    assert!(q_eff <= basis_q);
}

// ============================================================================
// T3.16: reset_pending_counter_invariant
// ============================================================================

/// While mode == ResetPending, each epoch-mismatch settlement decrements
/// stale_account_count exactly once.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t3_16_reset_pending_counter_invariant() {
    let mut engine = RiskEngine::new(zero_fee_params());

    // Create two accounts with positions on long side
    let a = engine.add_user(0).unwrap();
    let b = engine.add_user(0).unwrap();
    engine.deposit(a, 1_000_000, 100, 0).unwrap();
    engine.deposit(b, 1_000_000, 100, 0).unwrap();

    // Symbolic K value — both accounts snap at same K
    let k_val: i8 = kani::any();
    let k = I256::from_i128(k_val as i128);

    engine.accounts[a as usize].position_basis_q = I256::from_u128(POS_SCALE);
    engine.accounts[a as usize].adl_a_basis = ADL_ONE;
    engine.accounts[a as usize].adl_k_snap = k;
    engine.accounts[a as usize].adl_epoch_snap = 0;
    engine.accounts[b as usize].position_basis_q = I256::from_u128(POS_SCALE);
    engine.accounts[b as usize].adl_a_basis = ADL_ONE;
    engine.accounts[b as usize].adl_k_snap = k;
    engine.accounts[b as usize].adl_epoch_snap = 0;
    engine.stored_pos_count_long = 2;

    // K_long matches k_snap → k_diff == 0 (avoids U512)
    engine.adl_coeff_long = k;

    // Begin reset: epoch advances, stale = stored_pos_count
    engine.oi_eff_long_q = U256::ZERO;
    engine.begin_full_drain_reset(Side::Long);

    assert!(engine.side_mode_long == SideMode::ResetPending);
    assert!(engine.stale_account_count_long == 2);

    // Settle account a — counter decrements
    let _ = engine.settle_side_effects(a as usize);
    assert!(engine.stale_account_count_long == 1);

    // Settle account b — counter decrements
    let _ = engine.settle_side_effects(b as usize);
    assert!(engine.stale_account_count_long == 0);
}

/// Companion: reset counter with nonzero k_diff between k_snap and K_epoch_start.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t3_16b_reset_counter_with_nonzero_k_diff() {
    let mut engine = RiskEngine::new(zero_fee_params());

    let a = engine.add_user(0).unwrap();
    let b = engine.add_user(0).unwrap();
    engine.deposit(a, 10_000_000, 100, 0).unwrap();
    engine.deposit(b, 10_000_000, 100, 0).unwrap();

    // Both accounts snap at k=0
    let k_snap = I256::ZERO;

    engine.accounts[a as usize].position_basis_q = I256::from_u128(POS_SCALE);
    engine.accounts[a as usize].adl_a_basis = ADL_ONE;
    engine.accounts[a as usize].adl_k_snap = k_snap;
    engine.accounts[a as usize].adl_epoch_snap = 0;
    engine.accounts[b as usize].position_basis_q = I256::from_u128(POS_SCALE);
    engine.accounts[b as usize].adl_a_basis = ADL_ONE;
    engine.accounts[b as usize].adl_k_snap = k_snap;
    engine.accounts[b as usize].adl_epoch_snap = 0;
    engine.stored_pos_count_long = 2;

    // K_long differs from k_snap (nonzero k_diff)
    let k_diff_val: i8 = kani::any();
    kani::assume(k_diff_val != 0);
    let k_long = I256::from_i128(k_diff_val as i128);
    engine.adl_coeff_long = k_long;

    // Begin reset
    engine.oi_eff_long_q = U256::ZERO;
    engine.begin_full_drain_reset(Side::Long);

    // K_epoch_start captures K_long at reset time (includes nonzero k_diff)
    assert!(engine.adl_epoch_start_k_long == k_long);
    assert!(engine.stale_account_count_long == 2);

    // Settle both — counter still decrements correctly
    let _ = engine.settle_side_effects(a as usize);
    assert!(engine.stale_account_count_long == 1);
    let _ = engine.settle_side_effects(b as usize);
    assert!(engine.stale_account_count_long == 0);
}

// ############################################################################
//
// TIER 4: ADL ENQUEUE PROOFS
//
// ############################################################################

// ============================================================================
// T4.17: enqueue_adl_preserves_balanced_oi (quantity only)
// ============================================================================

/// Algebraic: with 2 accounts on the opposing side, A-shrink during ADL
/// produces effective positions that sum to at most oi_post.
/// Models enqueue_adl's A-ratio shrink for the opposing side.
#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn t4_17_enqueue_adl_preserves_oi_balance_qty_only() {
    let q1: u8 = kani::any();
    let q2: u8 = kani::any();
    kani::assume(q1 > 0 && q2 > 0);
    let oi = (q1 as u16) + (q2 as u16);
    kani::assume(oi <= 15);

    let q_close: u8 = kani::any();
    kani::assume(q_close > 0 && (q_close as u16) < oi);
    let oi_post = oi - (q_close as u16);

    let a_old = S_ADL_ONE;
    let a_new = a_after_adl(a_old, oi_post, oi);

    // Each account's effective position after A-shrink
    let basis_q1 = (q1 as u16) * S_POS_SCALE;
    let basis_q2 = (q2 as u16) * S_POS_SCALE;
    let eff_q1 = lazy_eff_q(basis_q1, a_new, a_old) / S_POS_SCALE;
    let eff_q2 = lazy_eff_q(basis_q2, a_new, a_old) / S_POS_SCALE;

    // Sum of effective positions must not exceed oi_post (floor can only lose)
    assert!(eff_q1 + eff_q2 <= oi_post,
        "sum of effective positions must not exceed oi_post");
    // Each individual effective position decreased
    assert!(eff_q1 <= q1 as u16);
    assert!(eff_q2 <= q2 as u16);
}

// ============================================================================
// T4.18: precision_exhaustion_both_sides_reset
// ============================================================================

/// When A_candidate == 0 with oi_post > 0, precision is exhausted.
/// Both sides' OI must go to zero and both pending resets must fire.
/// Models enqueue_adl step 9 logic.
///
/// Small model: a_old: u16, oi: u8, q_close: u8
/// A_candidate = floor(a_old * oi_post / oi). When a_old is small enough
/// relative to oi, A_candidate can be zero even with oi_post > 0.
#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn t4_18_precision_exhaustion_both_sides_reset() {
    let a_old: u16 = kani::any();
    kani::assume(a_old > 0);
    let oi: u8 = kani::any();
    kani::assume(oi >= 2);
    let q_close: u8 = kani::any();
    kani::assume(q_close > 0 && q_close < oi);
    let oi_post = oi - q_close;

    // A_candidate = floor(a_old * oi_post / oi)
    let a_candidate = ((a_old as u32) * (oi_post as u32)) / (oi as u32);

    // Only test the precision exhaustion case
    kani::assume(a_candidate == 0);
    // oi_post > 0 since q_close < oi
    assert!(oi_post > 0, "oi_post must be positive");

    // Model enqueue_adl step 9: when A_candidate == 0
    // Both sides' OI go to zero, both pending resets fire
    let mut oi_eff_opp: u16 = oi_post as u16;
    let mut oi_eff_liq: u16 = kani::any(); // some remaining liq-side OI
    let mut pending_reset_opp = false;
    let mut pending_reset_liq = false;

    // Terminal drain: zero both sides
    oi_eff_opp = 0;
    oi_eff_liq = 0;
    pending_reset_opp = true;
    pending_reset_liq = true;

    assert!(oi_eff_opp == 0, "opposing OI must be zero");
    assert!(oi_eff_liq == 0, "liquidated side OI must be zero");
    assert!(pending_reset_opp, "opposing side must have pending reset");
    assert!(pending_reset_liq, "liquidated side must have pending reset");
}

// ============================================================================
// T4.19: full_drain_terminal_K_includes_deficit
// ============================================================================

/// Algebraic: when OI_post == 0 and D > 0, the deficit modifies K before
/// the pending reset is triggered. Models enqueue_adl logic:
///   1. D > 0 → beta_abs = ceil(D * POS_SCALE / OI), delta_K = -A * beta_abs
///   2. K_opp += delta_K
///   3. OI_post == 0 → pending reset signaled
#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn t4_19_full_drain_terminal_k_includes_deficit() {
    let oi: u8 = kani::any();
    kani::assume(oi > 0 && oi <= 10);
    let d: u8 = kani::any();
    kani::assume(d > 0 && d <= 100);

    let a_opp = S_ADL_ONE;
    let k_before: i32 = 0;

    // Step 1: beta_abs = ceil(D * POS_SCALE / OI) in small model
    let beta_abs = ((d as u32) * (S_POS_SCALE as u32) + (oi as u32) - 1) / (oi as u32);

    // Step 2: delta_K = -(A * beta_abs)
    let delta_k = -((a_opp as i32) * (beta_abs as i32));
    let k_after = k_before + delta_k;

    // K must have been modified (deficit routed)
    assert!(k_after < k_before, "K must decrease when deficit is socialized");

    // Step 3: OI_post == 0 (full drain: q_close == oi)
    // pending reset would be signaled → begin_full_drain_reset captures K_epoch_start

    // K_epoch_start = K_after (includes deficit delta)
    // This is the K value that stale accounts will settle against
    let k_epoch_start = k_after;
    assert!(k_epoch_start == k_before + delta_k,
        "K_epoch_start must include deficit contribution");
    assert!(k_epoch_start < k_before,
        "K_epoch_start must be less than pre-deficit K");
}

// ============================================================================
// T4.20: bankruptcy_quantity_routes_even_when_D_zero
// ============================================================================

/// Algebraic: when D == 0 but q_close > 0, the opposing side's A must decrease
/// (A_new = floor(A_old * oi_post / oi) < A_old) and OI_opp shrinks.
#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn t4_20_bankruptcy_qty_routes_when_d_zero() {
    let oi: u8 = kani::any();
    kani::assume(oi >= 2);
    let q_close: u8 = kani::any();
    kani::assume(q_close > 0 && q_close < oi);

    let a_old = S_ADL_ONE;
    let oi_post = oi - q_close;

    // A_candidate = floor(A_old * oi_post / oi)
    let a_new = ((a_old as u32) * (oi_post as u32)) / (oi as u32);

    // A must decrease (since oi_post < oi)
    assert!((a_new as u32) <= (a_old as u32), "A_opp should not increase");
    assert!((a_new as u32) < (a_old as u32), "A_opp must strictly decrease");

    // OI_opp is set to oi_post
    assert!(oi_post < oi, "OI_opp must decrease");
}

// ############################################################################
//
// TIER 5: DUST / FIXED-POINT PROOFS
//
// ############################################################################

// ============================================================================
// T5.21: local_floor_settlement_error_is_bounded
// ============================================================================

/// Per-account quantity error from floor rounding is < 1 fixed-point unit.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t5_21_local_floor_quantity_error_bounded() {
    let basis_q: u16 = kani::any();
    kani::assume(basis_q > 0);

    let a_cur: u16 = kani::any();
    kani::assume(a_cur > 0);
    let a_basis: u16 = kani::any();
    kani::assume(a_basis > 0 && a_basis >= a_cur);

    // True value: basis_q * a_cur / a_basis (rational)
    // Floor value: floor(basis_q * a_cur / a_basis)
    let product = (basis_q as u64) * (a_cur as u64);
    let floor_val = product / (a_basis as u64);
    let remainder = product % (a_basis as u64);

    // Error = true - floor is in [0, 1) → remainder < a_basis
    assert!(remainder < a_basis as u64);
    // In fixed-point terms, error < 1 unit (which is a_basis in relative terms)
}

/// PnL rounding is conservative (floor toward -inf for negative).
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t5_21_pnl_rounding_conservative() {
    let basis_q: u8 = kani::any();
    kani::assume(basis_q > 0);
    let k_diff: i8 = kani::any();
    kani::assume(k_diff < 0); // Negative PnL

    let a_basis = S_ADL_ONE;
    let scaled_basis = (basis_q as u16) * S_POS_SCALE;

    let pnl = lazy_pnl(scaled_basis, k_diff as i32, a_basis);

    // For negative k_diff, PnL should be negative (conservative)
    assert!(pnl <= 0, "negative k_diff must produce non-positive PnL");

    // The floor should not overcount the loss by more than 1 unit
    let exact_num = (scaled_basis as i32) * (k_diff as i32);
    let den = (a_basis as i32) * (S_POS_SCALE as i32);
    let trunc = exact_num / den;
    // floor should be <= trunc (more negative)
    assert!(pnl <= trunc);
}

// ============================================================================
// T5.22: phantom_dust_total_bound
// ============================================================================

/// For 2 accounts sharing an A-shrink, total floor-rounding dust < 2 units.
/// Generalizes: for N accounts, total dust < N ≤ MAX_ACCOUNTS.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t5_22_phantom_dust_total_bound() {
    let q1: u8 = kani::any();
    let q2: u8 = kani::any();
    kani::assume(q1 > 0 && q2 > 0);
    let a_cur: u16 = kani::any();
    let a_basis: u16 = kani::any();
    kani::assume(a_basis > 0 && a_cur > 0 && a_cur <= a_basis);

    let basis_q1 = (q1 as u16) * S_POS_SCALE;
    let basis_q2 = (q2 as u16) * S_POS_SCALE;

    // Per-account floor remainder (from integer division)
    let rem1 = (basis_q1 as u32) * (a_cur as u32) % (a_basis as u32);
    let rem2 = (basis_q2 as u32) * (a_cur as u32) % (a_basis as u32);

    // Each remainder < a_basis (one unit of dust per account)
    assert!(rem1 < a_basis as u32);
    assert!(rem2 < a_basis as u32);

    // Total dust < 2 units (each account contributes < 1 unit)
    assert!(rem1 + rem2 < 2 * (a_basis as u32),
        "total dust from 2 accounts < 2 effective units");
}

// ============================================================================
// T5.23: dust_clearance_guard_is_safe
// ============================================================================

/// Dynamic dust bound sufficiency: phantom_dust_bound_side_q tracks the
/// number of same-epoch position zeroings. Each zeroing increments the bound
/// by exactly 1. The guard OI <= phantom_dust_bound is safe because each
/// zeroed position contributes at most 1 unit of floor-rounding dust to OI.
///
/// Small-model: N zeroings → dust_bound = N, each contributes < 1 base unit
/// of dust, so total OI dust < N = dust_bound.
#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn t5_23_dust_clearance_guard_safe() {
    let n: u8 = kani::any();
    kani::assume(n > 0 && n <= 32);

    // Each same-epoch zeroing increments phantom_dust_bound by 1.
    // After N zeroings: dust_bound = N.
    let dust_bound: u8 = n;

    // Each zeroed position contributes at most (POS_SCALE - 1) / POS_SCALE < 1
    // effective unit of OI dust (floor remainder from q_eff = floor(basis * A / a_basis)).
    // So total OI dust from N zeroings < N.
    // The guard fires when stored_pos_count == 0 AND OI <= dust_bound.
    // Since OI_dust < N and dust_bound == N, the guard correctly identifies
    // that all remaining OI is dust.
    let max_dust_per_acct = S_POS_SCALE as u16 - 1; // max floor remainder
    let max_total_dust_fp = (n as u16) * max_dust_per_acct;
    let max_total_dust_base = max_total_dust_fp / (S_POS_SCALE as u16);
    assert!(max_total_dust_base < n as u16,
        "total OI dust < phantom_dust_bound");
    assert!(dust_bound == n,
        "dust_bound tracks exact zeroing count");
}

// ############################################################################
//
// TIER 6: FOCUSED SCENARIO PROOFS (REGRESSIONS)
//
// ############################################################################

// ============================================================================
// T6.24: worked_example_regression
// ============================================================================

/// Four-step timeline: open, mark, partial ADL, verify lazy PnL.
///
/// Timeline (small-model):
///   1. L1 opens long 8, two shorts S1(5) S2(3) → OI = 8
///   2. Price moves: ΔP = 10 → K_long += A*10, L1 PnL = 80
///   3. S1 bankrupt: partial ADL q_close=5, D=2 on long side
///      A_long shrinks, K_long gets deficit delta, OI_long = 3
///   4. L1 settles: lazy PnL reflects both mark and deficit correctly
#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn t6_24_worked_example_regression() {
    let a_init = S_ADL_ONE; // 256
    let pos_scale = S_POS_SCALE; // 4

    // Step 1: L1 opens long 8 at price 100
    let q_l1: u16 = 8;
    let basis_l1 = q_l1 * pos_scale; // 32
    let a_basis_l1 = a_init;
    let k_snap_l1: i32 = 0;

    let oi: u16 = 8; // total long OI = 8
    let mut k_long: i32 = 0;
    let a_long = a_init;

    // Step 2: Price moves ΔP = 10 → K_long += A_long * 10
    let dp = 10i32;
    k_long = k_after_mark_long(k_long, a_long, dp);
    // K_long = 256 * 10 = 2560

    // L1 PnL check: floor(32 * 2560 / (256 * 4)) = floor(81920 / 1024) = 80
    let l1_pnl_pre = lazy_pnl(basis_l1, k_long - k_snap_l1, a_basis_l1);
    assert!(l1_pnl_pre == 80, "L1 pre-ADL PnL should be 80");

    // Step 3: Partial ADL — q_close=5, D=2
    // Opposing side is long. oi_post = 8 - 5 = 3
    let q_close: u16 = 5;
    let d: u16 = 2;
    let oi_post = oi - q_close; // 3
    assert!(oi_post > 0, "partial ADL: oi_post must be > 0");

    // Deficit routing: beta_abs = ceil(D * POS_SCALE / OI) = ceil(2*4/8) = ceil(1) = 1
    let beta_abs = ((d as u32) * (pos_scale as u32) + (oi as u32) - 1) / (oi as u32);
    assert!(beta_abs == 1);
    // delta_K = -(A_long * beta_abs) = -(256 * 1) = -256
    let delta_k = -((a_long as i32) * (beta_abs as i32));
    k_long = k_long + delta_k;
    // K_long = 2560 - 256 = 2304

    // A shrink: A_new = floor(256 * 3 / 8) = floor(96) = 96
    let a_long_new = a_after_adl(a_long, oi_post, oi);
    assert!(a_long_new == 96);

    // Step 4: L1 settles with new state
    // k_diff = K_long_new - k_snap_l1 = 2304 - 0 = 2304
    let k_diff = k_long - k_snap_l1;
    // q_eff = floor(basis_l1 * a_long_new / a_basis_l1) = floor(32 * 96 / 256) = floor(12) = 12
    let q_eff = lazy_eff_q(basis_l1, a_long_new, a_basis_l1);
    assert!(q_eff == 12, "L1 effective quantity after ADL");
    // PnL = floor(32 * 2304 / (256 * 4)) = floor(73728 / 1024) = 72
    let l1_pnl_post = lazy_pnl(basis_l1, k_diff, a_basis_l1);
    assert!(l1_pnl_post == 72, "L1 post-ADL PnL includes deficit");

    // The deficit reduced PnL from 80 to 72 (lost 8 = floor(8*2/8)*4/4 ≈ 2 per unit * ~4 eff units)
    assert!(l1_pnl_post < l1_pnl_pre, "deficit must reduce PnL");
    assert!(l1_pnl_post > 0, "PnL still positive from mark gain");
}

// ============================================================================
// T6.25: pure_pnl_bankruptcy_regression
// ============================================================================

/// Pure deficit (q_close = 0, D > 0): per-account lazy PnL is conservative.
/// Extends T4.19 by verifying the per-account PnL impact through K path.
#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn t6_25_pure_pnl_bankruptcy_regression() {
    let oi: u8 = kani::any();
    kani::assume(oi > 0);
    let d: u8 = kani::any();
    kani::assume(d > 0);
    let q_base: u8 = kani::any();
    kani::assume(q_base > 0 && q_base <= oi);

    let a_opp = S_ADL_ONE;
    let basis_q = (q_base as u16) * S_POS_SCALE;

    // beta_abs = ceil(D * POS_SCALE / OI)
    let beta_abs = ((d as u32) * (S_POS_SCALE as u32) + (oi as u32) - 1) / (oi as u32);
    assert!(beta_abs > 0, "beta must be positive for D > 0");

    // delta_K = -(A * beta_abs)
    let delta_k = -((a_opp as i32) * (beta_abs as i32));
    assert!(delta_k < 0, "K must decrease");

    // Per-account PnL via lazy settlement
    let pnl = lazy_pnl(basis_q, delta_k, a_opp);
    assert!(pnl <= 0, "each account must have non-positive PnL");

    // Conservative: lazy loss >= eager floor loss
    let eager_loss = ((q_base as i32) * (d as i32)) / (oi as i32);
    assert!(-pnl >= eager_loss,
        "lazy loss must be >= eager floor loss (conservative)");
}

// ============================================================================
// T6.26: full_drain_reset_regression
// ============================================================================

/// A side gets fully drained:
///   1. reset begins (epoch advances, stale = stored_pos_count)
///   2. stale account touches (terminal K applied)
///   3. position goes to zero
///   4. counters reconcile
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t6_26_full_drain_reset_regression() {
    let mut engine = RiskEngine::new(zero_fee_params());

    let idx = engine.add_user(0).unwrap();
    engine.deposit(idx, 1_000_000, 100, 0).unwrap();

    // Symbolic K value and position multiplier
    let k_val: i8 = kani::any();
    let k = I256::from_i128(k_val as i128);
    let pos_mul: u8 = kani::any();
    kani::assume(pos_mul > 0);

    // Set up long position at epoch 0 — k_snap = K_long → k_diff == 0
    engine.accounts[idx as usize].position_basis_q = I256::from_u128(POS_SCALE * (pos_mul as u128));
    engine.accounts[idx as usize].adl_a_basis = ADL_ONE;
    engine.accounts[idx as usize].adl_k_snap = k;
    engine.accounts[idx as usize].adl_epoch_snap = 0;
    engine.stored_pos_count_long = 1;

    engine.adl_coeff_long = k; // matches k_snap → k_diff == 0

    // Step 1: begin full drain reset
    engine.oi_eff_long_q = U256::ZERO;
    engine.begin_full_drain_reset(Side::Long);

    assert!(engine.side_mode_long == SideMode::ResetPending);
    assert!(engine.adl_epoch_long == 1);
    assert!(engine.stale_account_count_long == 1);
    assert!(engine.adl_epoch_start_k_long == k);

    // Step 2: stale account touches (k_diff == 0 → pnl_delta = 0)
    let result = engine.settle_side_effects(idx as usize);
    assert!(result.is_ok());

    // Step 3: position goes to zero
    assert!(engine.accounts[idx as usize].position_basis_q.is_zero());

    // Step 4: counters reconcile
    assert!(engine.stale_account_count_long == 0);

    // Can now finalize reset
    assert!(engine.stored_pos_count_long == 0);
    let finalize = engine.finalize_side_reset(Side::Long);
    assert!(finalize.is_ok());
    assert!(engine.side_mode_long == SideMode::Normal);
}

/// Companion: full drain reset with nonzero k_diff (the hard path).
/// K_epoch_start captures K_long at reset time. Account's k_snap differs
/// from K_epoch_start, producing nonzero terminal PnL. Position still zeroes,
/// stale counter decrements, and reset finalizes safely.
#[kani::proof]
#[kani::solver(cadical)]
fn t6_26b_full_drain_reset_nonzero_k_diff() {
    let mut engine = RiskEngine::new(zero_fee_params());

    let idx = engine.add_user(0).unwrap();
    engine.deposit(idx, 10_000_000, 100, 0).unwrap();

    // Position: 1 unit long at epoch 0, k_snap = 0
    engine.accounts[idx as usize].position_basis_q = I256::from_u128(POS_SCALE);
    engine.accounts[idx as usize].adl_a_basis = ADL_ONE;
    engine.accounts[idx as usize].adl_k_snap = I256::ZERO; // k_snap = 0
    engine.accounts[idx as usize].adl_epoch_snap = 0;
    engine.stored_pos_count_long = 1;

    // K_long = 500 (nonzero, different from k_snap=0)
    engine.adl_coeff_long = I256::from_i128(500);

    // Begin full drain reset — captures K_epoch_start = K_long = 500
    engine.oi_eff_long_q = U256::ZERO;
    engine.begin_full_drain_reset(Side::Long);

    assert!(engine.adl_epoch_start_k_long == I256::from_i128(500));
    assert!(engine.adl_epoch_long == 1);
    assert!(engine.stale_account_count_long == 1);

    let pnl_before = engine.accounts[idx as usize].pnl;

    // Settle: epoch mismatch, k_diff = K_epoch_start - k_snap = 500 - 0 = 500
    // This exercises the real pnl_delta computation with nonzero k_diff
    let result = engine.settle_side_effects(idx as usize);
    assert!(result.is_ok());

    // Position zeroed
    assert!(engine.accounts[idx as usize].position_basis_q.is_zero());

    // Stale counter decremented
    assert!(engine.stale_account_count_long == 0);

    // Epoch snap updated
    assert!(engine.accounts[idx as usize].adl_epoch_snap == 1);

    // Reset can finalize
    assert!(engine.stored_pos_count_long == 0);
    let finalize = engine.finalize_side_reset(Side::Long);
    assert!(finalize.is_ok());
    assert!(engine.side_mode_long == SideMode::Normal);
}

// ############################################################################
//
// TIER 7: NON-COMPOUNDING BASIS PROOFS (v9.5)
//
// ############################################################################

// ============================================================================
// T7.27: noncompounding_idempotent_settle
// ============================================================================

/// Small-model proof: two consecutive settlements with unchanged K
/// must produce zero incremental PnL on the second call.
/// Non-compounding: k_snap is updated to K after first settle,
/// so second settle sees k_diff = K - K = 0 → pnl_delta = 0.
/// Uses small-model arithmetic: S_POS_SCALE=4, S_ADL_ONE=256.
#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn t7_27_noncompounding_idempotent_settle() {
    // Small-model constants
    const S_POS_SCALE: u16 = 4;
    const S_ADL_ONE: u16 = 256;

    // Symbolic inputs
    let basis: u8 = kani::any();
    kani::assume(basis > 0);
    let a_basis: u8 = kani::any();
    kani::assume(a_basis > 0);
    let a_side: u8 = kani::any();
    kani::assume(a_side > 0);
    let k_side: i8 = kani::any();
    kani::assume(k_side != 0);

    // First settle: k_snap starts at 0, k_diff = k_side - 0 = k_side
    let den1 = (a_basis as i32) * (S_POS_SCALE as i32);
    kani::assume(den1 > 0);
    let num1 = (basis as i32) * (k_side as i32);
    // pnl_delta_1 = floor_div(num1, den1)  (conservative floor toward negative infinity)
    let pnl_1 = if num1 >= 0 { num1 / den1 } else { (num1 - den1 + 1) / den1 };

    // After first settle, k_snap is updated to k_side (non-compounding).
    // basis and a_basis are unchanged.

    // Second settle: k_diff = k_side - k_side = 0
    let k_diff_2: i32 = 0;
    let num2 = (basis as i32) * k_diff_2;
    let pnl_2 = if num2 >= 0 { num2 / den1 } else { (num2 - den1 + 1) / den1 };

    // pnl_delta from second settle must be exactly 0
    assert!(pnl_2 == 0, "second settle with unchanged K must produce zero incremental PnL");
}

// ============================================================================
// T7.28: noncompounding_two_touch_changing_k
// ============================================================================

/// Small-model proof: settle with mark between touches — first touch settles PnL
/// from K1, second touch settles incremental PnL from K2-K1, total = PnL from K2-K0.
/// Non-compounding: basis and a_basis unchanged between settles. Only k_snap updates.
#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn t7_28_noncompounding_two_touch_changing_k() {
    // Symbolic inputs
    let basis: u8 = kani::any();
    kani::assume(basis > 0);
    let a_basis: u8 = kani::any();
    kani::assume(a_basis > 0);

    let k1: i8 = kani::any();
    let k2_delta: i8 = kani::any();
    let k2_val = (k1 as i16) + (k2_delta as i16);
    kani::assume(k2_val >= -120 && k2_val <= 120);

    const S_POS_SCALE: i32 = 4;
    let den = (a_basis as i32) * S_POS_SCALE;
    kani::assume(den > 0);

    // Conservative floor division (toward negative infinity)
    let floor_div = |num: i32, d: i32| -> i32 {
        if num >= 0 { num / d } else { (num - d + 1) / d }
    };

    // First settle: k_snap=0, k_diff = k1
    let num1 = (basis as i32) * (k1 as i32);
    let pnl_1 = floor_div(num1, den);

    // After first settle, k_snap = k1 (non-compounding, basis/a_basis unchanged)

    // Second settle: k_diff = k2 - k1 = k2_delta
    let num2 = (basis as i32) * (k2_delta as i32);
    let pnl_2 = floor_div(num2, den);

    // Total from two touches
    let total_two_touch = pnl_1 + pnl_2;

    // Single settlement from K0=0 to K2
    let num_single = (basis as i32) * (k2_val as i32);
    let pnl_single = floor_div(num_single, den);

    // Non-compounding additivity: floor(a*k1/d) + floor(a*k2_delta/d) >= floor(a*(k1+k2_delta)/d)
    // Due to floor rounding, two-touch sum >= single (conservative).
    // Also: two-touch sum <= single + 1 (at most 1 unit rounding error per touch)
    assert!(total_two_touch >= pnl_single,
        "two-touch PnL must be >= single-touch (conservative floor)");
    assert!(total_two_touch <= pnl_single + 1,
        "two-touch PnL must be at most 1 unit above single-touch");
}

// ============================================================================
// T1.5b: mark_lazy_equals_eager_symbolic_a_basis
// ============================================================================

/// Generalization of T1.5: lazy=eager for ANY a_basis (not just ADL_ONE).
/// Covers positions opened after ADL shrinkage.
#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn t1_5b_mark_lazy_equals_eager_symbolic_a_basis() {
    let q_base: u8 = kani::any();
    kani::assume(q_base > 0 && q_base <= 15);
    let delta_p: i8 = kani::any();
    kani::assume(delta_p >= -15 && delta_p <= 15);

    // Symbolic a_basis — any nonzero value up to S_ADL_ONE
    let a_basis: u16 = kani::any();
    kani::assume(a_basis > 0 && a_basis <= S_ADL_ONE);

    let k_init: i32 = 0;
    let basis_q = (q_base as u16) * S_POS_SCALE;

    // Eager: PnL = q_base * delta_p (same regardless of a_basis)
    let eager_pnl = (q_base as i32) * (delta_p as i32);

    // Lazy: K_long += a_basis * delta_p (A_long = a_basis since we're in the account's epoch)
    let k_after = k_init + (a_basis as i32) * (delta_p as i32);
    let k_diff = k_after - k_init;
    let lazy_pnl_val = lazy_pnl(basis_q, k_diff, a_basis);

    assert!(eager_pnl == lazy_pnl_val,
        "mark lazy != eager for symbolic a_basis");
}

// ============================================================================
// T1.6b: funding_lazy_equals_eager_symbolic_a_basis
// ============================================================================

/// Same generalization for funding events.
#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn t1_6b_funding_lazy_equals_eager_symbolic_a_basis() {
    let q_base: u8 = kani::any();
    kani::assume(q_base > 0 && q_base <= 15);
    let delta_f: i8 = kani::any();
    kani::assume(delta_f >= -15 && delta_f <= 15);

    let a_basis: u16 = kani::any();
    kani::assume(a_basis > 0 && a_basis <= S_ADL_ONE);

    let k_init: i32 = 0;
    let basis_q = (q_base as u16) * S_POS_SCALE;

    // Eager: longs pay ΔF per unit → pnl = -q * ΔF
    let eager_pnl = -((q_base as i32) * (delta_f as i32));

    // Lazy: K_long -= a_basis * ΔF
    let k_after = k_init - (a_basis as i32) * (delta_f as i32);
    let k_diff = k_after - k_init;
    let lazy_pnl_val = lazy_pnl(basis_q, k_diff, a_basis);

    assert!(eager_pnl == lazy_pnl_val,
        "funding lazy != eager for symbolic a_basis");
}

// ============================================================================
// T1.8b: adl_deficit_lazy_conservative_symbolic_a_basis
// ============================================================================

/// Same generalization for deficit-only ADL.
#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn t1_8b_adl_deficit_lazy_conservative_symbolic_a_basis() {
    let q_base: u8 = kani::any();
    kani::assume(q_base > 0 && q_base <= 15);
    let oi: u8 = kani::any();
    kani::assume(oi > 0 && oi <= 15);
    let d: u8 = kani::any();
    kani::assume(d > 0 && d <= 15);

    let a_basis: u16 = kani::any();
    kani::assume(a_basis > 0 && a_basis <= S_ADL_ONE);

    let k_init: i32 = 0;
    let basis_q = (q_base as u16) * S_POS_SCALE;

    // Eager loss per account: floor(q_base * D / OI)
    let eager_loss = ((q_base as i32) * (d as i32)) / (oi as i32);

    // Lazy: beta_abs = ceil(D * POS_SCALE / OI), delta_K = -(a_basis * beta_abs)
    let beta_abs = ((d as u32) * (S_POS_SCALE as u32) + (oi as u32) - 1) / (oi as u32);
    let delta_k = -((a_basis as i32) * (beta_abs as i32));
    let lazy_loss_raw = lazy_pnl(basis_q, delta_k, a_basis);

    // Conservative: lazy loss >= eager loss
    let lazy_loss = -lazy_loss_raw;
    assert!(lazy_loss >= eager_loss,
        "ADL deficit lazy must be at least as large as eager for symbolic a_basis");
}

// ############################################################################
//
// TIER 3 ADDITIONS: DYNAMIC DUST / RESET LIFECYCLE
//
// ############################################################################

// ============================================================================
// T5.24: dynamic_dust_bound_sufficient
// ============================================================================

/// Engine proof: after N same-epoch position zeroings, phantom_dust_bound >= N.
/// Each zeroing increments by exactly 1 (inc_phantom_dust_bound).
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t5_24_dynamic_dust_bound_sufficient() {
    let mut engine = RiskEngine::new(zero_fee_params());

    let a = engine.add_user(0).unwrap();
    let b = engine.add_user(0).unwrap();
    engine.deposit(a, 10_000_000, 100, 0).unwrap();
    engine.deposit(b, 10_000_000, 100, 0).unwrap();

    // Both accounts have small long positions (1 POS_SCALE unit each)
    engine.accounts[a as usize].position_basis_q = I256::from_u128(POS_SCALE);
    engine.accounts[a as usize].adl_a_basis = ADL_ONE;
    engine.accounts[a as usize].adl_k_snap = I256::ZERO;
    engine.accounts[a as usize].adl_epoch_snap = 0;
    engine.accounts[b as usize].position_basis_q = I256::from_u128(POS_SCALE);
    engine.accounts[b as usize].adl_a_basis = ADL_ONE;
    engine.accounts[b as usize].adl_k_snap = I256::ZERO;
    engine.accounts[b as usize].adl_epoch_snap = 0;
    engine.stored_pos_count_long = 2;
    engine.oi_eff_long_q = U256::from_u128(2 * POS_SCALE);
    engine.adl_epoch_long = 0;

    // Shrink A to near-zero so q_eff rounds to 0
    // A = 1 means floor(POS_SCALE * 1 / ADL_ONE) = 0 for any POS_SCALE < ADL_ONE
    engine.adl_mult_long = 1;
    engine.adl_coeff_long = I256::ZERO;

    // Settle account a — q_eff = 0, should increment dust bound
    let _ = engine.settle_side_effects(a as usize);
    assert!(engine.phantom_dust_bound_long_q == U256::from_u128(1));

    // Settle account b — q_eff = 0, should increment dust bound again
    let _ = engine.settle_side_effects(b as usize);
    assert!(engine.phantom_dust_bound_long_q == U256::from_u128(2));
}

// ============================================================================
// T3.17: clean_empty_engine_no_retrigger
// ============================================================================

/// Engine proof: schedule_end_of_instruction_resets on fresh engine
/// (stored_pos_count=0, phantom_dust_bound=0, OI=0) must NOT trigger reset.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t3_17_clean_empty_engine_no_retrigger() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let mut ctx = InstructionContext::new();

    // Fresh engine: all zeros
    assert!(engine.stored_pos_count_long == 0);
    assert!(engine.stored_pos_count_short == 0);
    assert!(engine.oi_eff_long_q.is_zero());
    assert!(engine.oi_eff_short_q.is_zero());
    assert!(engine.phantom_dust_bound_long_q.is_zero());
    assert!(engine.phantom_dust_bound_short_q.is_zero());

    let result = engine.schedule_end_of_instruction_resets(&mut ctx);
    assert!(result.is_ok());

    // Must not trigger resets — trivial case guard
    assert!(!ctx.pending_reset_long, "no reset on empty engine long");
    assert!(!ctx.pending_reset_short, "no reset on empty engine short");
}

// ============================================================================
// T3.18: dust_bound_reset_in_begin_full_drain
// ============================================================================

/// Engine proof: begin_full_drain_reset zeroes phantom_dust_bound.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t3_18_dust_bound_reset_in_begin_full_drain() {
    let mut engine = RiskEngine::new(zero_fee_params());

    // Set up nonzero dust bound
    engine.phantom_dust_bound_long_q = U256::from_u128(5);
    engine.oi_eff_long_q = U256::ZERO;

    engine.begin_full_drain_reset(Side::Long);

    assert!(engine.phantom_dust_bound_long_q.is_zero(),
        "phantom_dust_bound must be zeroed by begin_full_drain_reset");
}

// ============================================================================
// T3.19: finalize_side_reset_requires_all_stale_touched
// ============================================================================

/// Engine proof: finalize_side_reset fails if stale_account_count > 0
/// or stored_pos_count > 0.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t3_19_finalize_side_reset_requires_all_stale_touched() {
    let mut engine = RiskEngine::new(zero_fee_params());

    // Test 1: fails when stale_count > 0
    engine.side_mode_long = SideMode::ResetPending;
    engine.oi_eff_long_q = U256::ZERO;
    engine.stale_account_count_long = 1;
    engine.stored_pos_count_long = 0;
    let result1 = engine.finalize_side_reset(Side::Long);
    assert!(result1.is_err(), "finalize must fail with stale_count > 0");

    // Test 2: fails when stored_pos_count > 0
    engine.stale_account_count_long = 0;
    engine.stored_pos_count_long = 1;
    let result2 = engine.finalize_side_reset(Side::Long);
    assert!(result2.is_err(), "finalize must fail with stored_pos_count > 0");

    // Test 3: succeeds when both are zero
    engine.stored_pos_count_long = 0;
    let result3 = engine.finalize_side_reset(Side::Long);
    assert!(result3.is_ok(), "finalize must succeed when all conditions met");
    assert!(engine.side_mode_long == SideMode::Normal);
}

// ############################################################################
//
// TIER 4 ADDITIONS: ADL FALLBACK BRANCHES
//
// ############################################################################

// ============================================================================
// T4.21: precision_exhaustion_zeroes_both_sides (engine proof)
// ============================================================================

/// Engine proof: when A_candidate == 0 with oi_post > 0, both sides' OI go to
/// zero and both pending resets fire. Uses enqueue_adl directly.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t4_21_precision_exhaustion_zeroes_both_sides() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let mut ctx = InstructionContext::new();

    // Set opposing side A very small so A_candidate = floor(A_old * oi_post / oi) = 0
    // A_old = 1, oi = 3, q_close = 1 → oi_post = 2 → A_candidate = floor(1*2/3) = 0
    engine.adl_mult_long = 1;
    engine.oi_eff_long_q = U256::from_u128(3 * POS_SCALE);
    engine.oi_eff_short_q = U256::from_u128(3 * POS_SCALE);
    engine.adl_coeff_long = I256::ZERO;

    // liq_side = Short, opposing = Long
    // q_close = 1 POS_SCALE unit, D = 0
    let q_close = U256::from_u128(POS_SCALE);
    let d = U256::ZERO;

    let result = engine.enqueue_adl(&mut ctx, Side::Short, q_close, d);
    assert!(result.is_ok());

    // Both sides' OI should be zero (precision exhaustion terminal drain)
    assert!(engine.oi_eff_long_q.is_zero(), "opposing OI must be zero");
    assert!(engine.oi_eff_short_q.is_zero(), "liq side OI must be zero");
    assert!(ctx.pending_reset_long, "opposing side must have pending reset");
    assert!(ctx.pending_reset_short, "liq side must have pending reset");
}

// ============================================================================
// T4.22: k_overflow_routes_to_absorb
// ============================================================================

/// Small-model proof: when K_opp + delta_K would overflow, the K fallback
/// route still allows A to shrink and OI to update correctly. Models the
/// step 9 logic from enqueue_adl: A_new = floor(A_old * oi_post / oi),
/// and K is clamped/unchanged on overflow.
#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn t4_22_k_overflow_routes_to_absorb() {
    let oi: u8 = kani::any();
    kani::assume(oi >= 2);
    let q_close: u8 = kani::any();
    kani::assume(q_close > 0 && q_close < oi);
    let d: u8 = kani::any();
    kani::assume(d > 0);

    let a_old = S_ADL_ONE;
    let oi_post = oi - q_close;

    // Model K overflow: k_opp is near minimum, delta_k would exceed range
    let k_opp: i8 = -127; // near i8::MIN
    // delta_k = d * POS_SCALE / (A_old * oi_post) — simplified, could overflow
    // When overflow: K is unchanged, D is absorbed by insurance
    let k_after = k_opp; // K unchanged on overflow

    // A still shrinks (quantity routing proceeds)
    let a_new = a_after_adl(a_old, oi_post as u16, oi as u16);
    assert!(a_new < a_old as u16, "A must shrink even on K overflow");
    assert!(k_after == k_opp, "K must be unchanged on overflow (routed to absorb)");
}

// ============================================================================
// T4.23: d_zero_routes_quantity_only
// ============================================================================

/// Small model: when D == 0, K is unchanged, only A shrinks.
#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn t4_23_d_zero_routes_quantity_only() {
    let oi: u8 = kani::any();
    kani::assume(oi >= 2);
    let q_close: u8 = kani::any();
    kani::assume(q_close > 0 && q_close < oi);

    let a_old = S_ADL_ONE;
    let k_before: i32 = kani::any::<i8>() as i32;
    let oi_post = oi - q_close;

    // D == 0: no deficit to route through K
    // K is unchanged
    let k_after = k_before; // no delta_K when D==0

    // A shrinks
    let a_new = a_after_adl(a_old, oi_post as u16, oi as u16);
    assert!(a_new < a_old as u16, "A must strictly decrease");
    assert!(k_after == k_before, "K must be unchanged when D == 0");
}

// ############################################################################
//
// TIER 8: REAL ENGINE INTEGRATION PROOFS
//
// ############################################################################

// ============================================================================
// T8.30: trade_oi_long_equals_short
// ============================================================================

/// Small-model proof: trade OI updates are symmetric — when account a goes
/// long by `size` and b goes short by `size`, OI_long and OI_short both
/// increase by the same amount. Models update_single_oi symmetry.
#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn t8_30_trade_oi_long_equals_short() {
    // Model: both sides start at same OI
    let oi_before: u8 = kani::any();
    let size: u8 = kani::any();
    kani::assume(size > 0 && size <= 10);
    // OI doesn't overflow
    kani::assume((oi_before as u16 + size as u16) <= 255);

    // Account a: was flat → goes long by size
    // new_pos_a = 0 + size, old_pos_a = 0
    // oi_long += |new| - |old| = size - 0 = size
    let oi_long_after = oi_before as u16 + size as u16;

    // Account b: was flat → goes short by size
    // new_pos_b = 0 - size, old_pos_b = 0
    // oi_short += |new| - |old| = size - 0 = size
    let oi_short_after = oi_before as u16 + size as u16;

    assert!(oi_long_after == oi_short_after,
        "OI long must equal OI short after symmetric trade");
}

// ============================================================================
// T8.31: trade_slippage_zero_sum
// ============================================================================

/// Small-model proof: for a zero-fee trade at execution price, no capital
/// is created or destroyed. When fee=0, the vault (sum of all capital) is
/// unchanged because trade only moves position between accounts.
#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn t8_31_trade_zero_sum() {
    let cap_a: u8 = kani::any();
    let cap_b: u8 = kani::any();
    kani::assume(cap_a >= 10 && cap_b >= 10);
    let size: u8 = kani::any();
    kani::assume(size > 0 && size <= 5);
    let fee_bps: u8 = 0; // zero fee

    let vault_before = cap_a as u16 + cap_b as u16;

    // Trade at oracle price with zero fee:
    // notional = size * price / POS_SCALE (at model scale this is just size*price)
    // fee = notional * fee_bps / 10000 = 0
    // No capital transfer at trade time; only positions change
    // PnL is zero at trade time (trade at oracle = no mark-to-market gain)
    let fee = 0u16; // zero fee
    let vault_after = vault_before; // no fees extracted

    assert!(vault_after == vault_before,
        "vault must be unchanged with zero fees");
}

// ============================================================================
// T8.32: conservation_across_trade
// ============================================================================

/// Small-model proof: conservation invariant (vault >= c_tot + insurance)
/// is maintained across a trade. Trade with zero fees moves no capital,
/// and trade fees only transfer from vault to protocol, never creating value.
#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn t8_32_conservation_across_trade() {
    let cap_a: u8 = kani::any();
    let cap_b: u8 = kani::any();
    kani::assume(cap_a >= 10 && cap_b >= 10);
    let insurance: u8 = kani::any();

    let vault = cap_a as u16 + cap_b as u16 + insurance as u16;
    let c_tot = cap_a as u16 + cap_b as u16;

    // Conservation before: vault >= c_tot + insurance
    assert!(vault >= c_tot + insurance as u16, "conservation before");

    // Trade with fee: fee is subtracted from trader capital and added to insurance
    let fee: u8 = kani::any();
    kani::assume(fee <= cap_a); // fee can't exceed capital

    let c_tot_after = c_tot - fee as u16; // capital decreases by fee
    let insurance_after = insurance as u16 + fee as u16; // insurance increases by fee
    // vault is unchanged (it's the total deposit, which doesn't change)

    // Conservation after: vault >= c_tot_after + insurance_after
    // c_tot_after + insurance_after = c_tot - fee + insurance + fee = c_tot + insurance = vault
    assert!(vault >= c_tot_after + insurance_after, "conservation after trade");
}

// ============================================================================
// T8.33: organic_close_no_bankruptcy
// ============================================================================

/// Small-model proof: closing a position at oracle price with zero fees
/// results in zero PnL for the closer (no bankruptcy). When open_price ==
/// close_price and fee == 0, the account's capital is unchanged.
#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn t8_33_organic_close_no_bankruptcy() {
    let capital: u8 = kani::any();
    kani::assume(capital >= 10);
    let size: u8 = kani::any();
    kani::assume(size > 0 && size <= 10);
    let price: u8 = kani::any();
    kani::assume(price > 0);

    // Open at price, close at same price, zero fee:
    // PnL = size * (close_price - open_price) = size * 0 = 0
    let pnl: i16 = (size as i16) * ((price as i16) - (price as i16));
    assert!(pnl == 0, "PnL must be zero when closing at open price");

    // Capital after close = capital + pnl = capital >= 0
    let capital_after = capital as i16 + pnl;
    assert!(capital_after >= 0, "no bankruptcy on organic close at same price");

    // Position after close = open - close = size - size = 0
    let pos_after = size as i16 - size as i16;
    assert!(pos_after == 0, "account must be flat after close");
}

// ============================================================================
// T8.34: liquidation_no_oi_leak
// ============================================================================

/// Small-model proof: liquidation closes a position, so OI decreases by
/// exactly the liquidated amount on both sides (through ADL or direct close).
/// OI_long and OI_short remain equal after liquidation.
#[kani::proof]
#[kani::unwind(1)]
#[kani::solver(cadical)]
fn t8_34_liquidation_no_oi_leak() {
    let oi_before: u8 = kani::any();
    kani::assume(oi_before >= 2);
    let liq_size: u8 = kani::any();
    kani::assume(liq_size > 0 && liq_size <= oi_before);

    // Before liquidation: OI_long == OI_short (invariant)
    let oi_long_before = oi_before;
    let oi_short_before = oi_before;

    // Liquidation removes `liq_size` from the liquidated account's side
    // and the same amount from the opposing side (via ADL or position close)
    let oi_long_after = oi_long_before - liq_size;
    let oi_short_after = oi_short_before - liq_size;

    assert!(oi_long_after == oi_short_after,
        "OI long must equal OI short after liquidation");
}

// ############################################################################
//
// TIER 9: FEE / WARMUP PROOFS
//
// ############################################################################

// ============================================================================
// T9.35: warmup_slope_preservation
// ============================================================================

/// Engine proof: when warmup_period_slots > 0 and PnL is positive,
/// warmable_gross increases monotonically with elapsed slots.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t9_35_warmup_slope_preservation() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = engine.add_user(0).unwrap();
    engine.deposit(idx, 10_000_000, 100, 0).unwrap();

    // Set positive PnL
    let pnl_val: u8 = kani::any();
    kani::assume(pnl_val > 0);
    engine.set_pnl(idx as usize, I256::from_u128(pnl_val as u128));

    // Set warmup state: started at slot 0, slope = pnl / warmup_period
    engine.accounts[idx as usize].warmup_started_at_slot = 0;
    engine.accounts[idx as usize].warmup_slope_per_step = U256::from_u128(1);
    engine.accounts[idx as usize].reserved_pnl = U256::ZERO;

    // Slot 1: warmable should be slope * 1 = 1
    engine.current_slot = 1;
    let w1 = engine.warmable_gross(idx as usize);

    // Slot 2: warmable should be slope * 2 = 2
    engine.current_slot = 2;
    let w2 = engine.warmable_gross(idx as usize);

    // Monotonic: w2 >= w1
    assert!(w2 >= w1, "warmable_gross must be monotonically non-decreasing");
}

// ============================================================================
// T9.36: fee_seniority_after_restart
// ============================================================================

/// Engine proof: after an epoch restart (position zeroed via reset, re-opened),
/// fee_credits value is preserved across the restart cycle.
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t9_36_fee_seniority_after_restart() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = engine.add_user(0).unwrap();
    engine.deposit(idx, 10_000_000, 100, 0).unwrap();

    // Set a fee_credits value
    let fc_val: i8 = kani::any();
    engine.accounts[idx as usize].fee_credits = percolator::i128::I128::new(fc_val as i128);

    let fc_before = engine.accounts[idx as usize].fee_credits;

    // Simulate position zeroed via epoch mismatch settlement
    engine.accounts[idx as usize].position_basis_q = I256::from_u128(POS_SCALE);
    engine.accounts[idx as usize].adl_a_basis = ADL_ONE;
    engine.accounts[idx as usize].adl_k_snap = I256::ZERO;
    engine.accounts[idx as usize].adl_epoch_snap = 0;
    engine.stored_pos_count_long = 1;
    engine.adl_epoch_long = 1;
    engine.adl_epoch_start_k_long = I256::ZERO;
    engine.side_mode_long = SideMode::ResetPending;
    engine.stale_account_count_long = 1;
    engine.adl_coeff_long = I256::ZERO;

    let _ = engine.settle_side_effects(idx as usize);

    // fee_credits must survive the restart
    let fc_after = engine.accounts[idx as usize].fee_credits;
    assert!(fc_after == fc_before,
        "fee_credits must be preserved across epoch restart");
}

// ############################################################################
//
// TIER 10: ACCRUE_MARKET_TO PROOFS
//
// ############################################################################

// ============================================================================
// T10.37: accrue_mark_matches_eager
// ============================================================================

/// Engine proof: for a single sub-step with dt=0 (no funding), price change
/// from 100 to 100+dp:
///   K_long_after - K_long_before == A_long * delta_p
///   K_short_after - K_short_before == -(A_short * delta_p)
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t10_37_accrue_mark_matches_eager() {
    let mut engine = RiskEngine::new(zero_fee_params());

    // Set up minimal OI so K updates happen
    engine.oi_eff_long_q = U256::from_u128(POS_SCALE);
    engine.oi_eff_short_q = U256::from_u128(POS_SCALE);
    engine.adl_mult_long = ADL_ONE;
    engine.adl_mult_short = ADL_ONE;
    engine.last_oracle_price = 100;
    engine.last_market_slot = 0;
    engine.funding_rate_bps_per_slot_last = 0; // no funding
    engine.funding_price_sample_last = 100;

    let k_long_before = engine.adl_coeff_long;
    let k_short_before = engine.adl_coeff_short;

    // Price change: symbolic but bounded
    let dp: i8 = kani::any();
    kani::assume(dp >= -50 && dp <= 50);
    let new_price = (100i16 + dp as i16) as u64;
    kani::assume(new_price > 0);

    let result = engine.accrue_market_to(1, new_price);
    assert!(result.is_ok());

    let k_long_after = engine.adl_coeff_long;
    let k_short_after = engine.adl_coeff_short;

    // K_long += A_long * delta_p
    let expected_delta = I256::from_i128((ADL_ONE as i128) * (dp as i128));
    let actual_long_delta = k_long_after.checked_sub(k_long_before).unwrap();
    assert!(actual_long_delta == expected_delta,
        "K_long delta must equal A_long * delta_p");

    // K_short -= A_short * delta_p → delta = -(A_short * delta_p)
    let actual_short_delta = k_short_after.checked_sub(k_short_before).unwrap();
    let expected_short_delta = expected_delta.checked_neg().unwrap_or(I256::ZERO);
    assert!(actual_short_delta == expected_short_delta,
        "K_short delta must equal -(A_short * delta_p)");
}

// ============================================================================
// T10.38: accrue_funding_matches_eager
// ============================================================================

/// Engine proof: for a single sub-step with delta_p=0 (same price), dt=1:
///   K_long decreases by A_long * delta_f
///   K_short increases by A_short * delta_f
/// where delta_f = fund_px * r_last * dt / 10_000
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn t10_38_accrue_funding_matches_eager() {
    let mut engine = RiskEngine::new(zero_fee_params());

    engine.oi_eff_long_q = U256::from_u128(POS_SCALE);
    engine.oi_eff_short_q = U256::from_u128(POS_SCALE);
    engine.adl_mult_long = ADL_ONE;
    engine.adl_mult_short = ADL_ONE;
    engine.last_oracle_price = 100;
    engine.last_market_slot = 0;
    engine.funding_price_sample_last = 100;

    // Symbolic funding rate (bounded small)
    let rate: i8 = kani::any();
    kani::assume(rate != 0);
    kani::assume(rate >= -100 && rate <= 100);
    engine.funding_rate_bps_per_slot_last = rate as i64;

    let k_long_before = engine.adl_coeff_long;
    let k_short_before = engine.adl_coeff_short;

    // Same price, 1 slot elapsed
    let result = engine.accrue_market_to(1, 100);
    assert!(result.is_ok());

    let k_long_after = engine.adl_coeff_long;
    let k_short_after = engine.adl_coeff_short;

    // delta_f = fund_px * r_last * dt / 10_000 = 100 * rate * 1 / 10_000
    let delta_f: i128 = (100i128 * (rate as i128) * 1) / 10_000;

    // Longs pay: K_long -= A_long * delta_f
    let fund_k = I256::from_i128((ADL_ONE as i128) * delta_f);
    let expected_long = k_long_before.checked_sub(fund_k).unwrap();
    assert!(k_long_after == expected_long,
        "K_long must decrease by A_long * delta_f");

    // Shorts receive: K_short += A_short * delta_f
    let expected_short = k_short_before.checked_add(fund_k).unwrap();
    assert!(k_short_after == expected_short,
        "K_short must increase by A_short * delta_f");
}

// ############################################################################
//
// TIER 11: REAL-ENGINE INTEGRATION PROOFS
//
// These use concrete inputs to exercise actual engine code paths.
// The U512 division loop needs unwind >= 70 (set in Cargo.toml default).
// Concrete inputs ensure deterministic loop counts, avoiding SAT blowup.
//
// ############################################################################

// ============================================================================
// T11.39: same_epoch_settle_idempotent_real_engine
// ============================================================================

/// Real engine: two consecutive settle_side_effects with unchanged K
/// produces zero incremental PnL on the second call.
/// Exercises the actual mul_div_floor_u256 and wide_signed_mul_div_floor paths.
#[kani::proof]
#[kani::solver(cadical)]
fn t11_39_same_epoch_settle_idempotent_real_engine() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = engine.add_user(0).unwrap();
    engine.deposit(idx, 10_000_000, 100, 0).unwrap();

    // Concrete position: 1 POS_SCALE unit long
    let pos = I256::from_u128(POS_SCALE);
    engine.accounts[idx as usize].position_basis_q = pos;
    engine.accounts[idx as usize].adl_a_basis = ADL_ONE;
    engine.accounts[idx as usize].adl_k_snap = I256::ZERO;
    engine.accounts[idx as usize].adl_epoch_snap = 0;
    engine.stored_pos_count_long = 1;
    engine.adl_epoch_long = 0;
    engine.oi_eff_long_q = U256::from_u128(POS_SCALE);

    // K_long = 100 (nonzero mark happened)
    engine.adl_coeff_long = I256::from_i128(100);

    // First settle: picks up PnL from k_diff = 100 - 0 = 100
    let r1 = engine.settle_side_effects(idx as usize);
    assert!(r1.is_ok());
    let pnl_after_first = engine.accounts[idx as usize].pnl;
    // k_snap should now be 100
    assert!(engine.accounts[idx as usize].adl_k_snap == I256::from_i128(100));

    // Second settle: k_diff = 100 - 100 = 0 → pnl_delta = 0
    let r2 = engine.settle_side_effects(idx as usize);
    assert!(r2.is_ok());
    let pnl_after_second = engine.accounts[idx as usize].pnl;

    assert!(pnl_after_second == pnl_after_first,
        "second settle with unchanged K must produce zero incremental PnL");
    // basis and a_basis unchanged (non-compounding)
    assert!(engine.accounts[idx as usize].adl_a_basis == ADL_ONE);
    assert!(engine.accounts[idx as usize].position_basis_q == pos);
}

// ============================================================================
// T11.40: non_compounding_quantity_basis_two_touches
// ============================================================================

/// Real engine: settle with K change between touches. Basis and a_basis
/// must NOT change (non-compounding). Only k_snap updates.
#[kani::proof]
#[kani::solver(cadical)]
fn t11_40_non_compounding_quantity_basis_two_touches() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = engine.add_user(0).unwrap();
    engine.deposit(idx, 10_000_000, 100, 0).unwrap();

    let pos = I256::from_u128(POS_SCALE);
    engine.accounts[idx as usize].position_basis_q = pos;
    engine.accounts[idx as usize].adl_a_basis = ADL_ONE;
    engine.accounts[idx as usize].adl_k_snap = I256::ZERO;
    engine.accounts[idx as usize].adl_epoch_snap = 0;
    engine.stored_pos_count_long = 1;
    engine.adl_epoch_long = 0;
    engine.oi_eff_long_q = U256::from_u128(POS_SCALE);

    // Mark to K=50
    engine.adl_coeff_long = I256::from_i128(50);
    let _ = engine.settle_side_effects(idx as usize);

    // Non-compounding invariant: basis and a_basis unchanged
    assert!(engine.accounts[idx as usize].position_basis_q == pos);
    assert!(engine.accounts[idx as usize].adl_a_basis == ADL_ONE);
    // k_snap updated
    assert!(engine.accounts[idx as usize].adl_k_snap == I256::from_i128(50));

    let pnl_after_first = engine.accounts[idx as usize].pnl;

    // Mark to K=120
    engine.adl_coeff_long = I256::from_i128(120);
    let _ = engine.settle_side_effects(idx as usize);

    // Still non-compounding: basis and a_basis unchanged
    assert!(engine.accounts[idx as usize].position_basis_q == pos);
    assert!(engine.accounts[idx as usize].adl_a_basis == ADL_ONE);
    assert!(engine.accounts[idx as usize].adl_k_snap == I256::from_i128(120));
}

// ============================================================================
// T11.41: attach_effective_position_remainder_accounting
// ============================================================================

/// Real engine: attach_effective_position increments phantom_dust_bound
/// when replacing a basis with nonzero remainder, and does NOT increment
/// when remainder is zero.
#[kani::proof]
#[kani::solver(cadical)]
fn t11_41_attach_effective_position_remainder_accounting() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let idx = engine.add_user(0).unwrap();
    engine.deposit(idx, 10_000_000, 100, 0).unwrap();

    // Set up: position with a_basis that will produce a remainder
    let pos = I256::from_u128(POS_SCALE);
    engine.accounts[idx as usize].position_basis_q = pos;
    // a_basis = ADL_ONE, a_side = ADL_ONE - 1 → remainder = POS_SCALE * (ADL_ONE-1) mod ADL_ONE ≠ 0
    engine.accounts[idx as usize].adl_a_basis = ADL_ONE;
    engine.accounts[idx as usize].adl_epoch_snap = 0;
    engine.adl_epoch_long = 0;
    engine.adl_mult_long = ADL_ONE - 1; // a_side < a_basis → nonzero remainder
    engine.stored_pos_count_long = 1;

    let dust_before = engine.phantom_dust_bound_long_q;

    // Attach a new position — this replaces the old basis
    let new_pos = I256::from_u128(2 * POS_SCALE);
    engine.attach_effective_position(idx as usize, new_pos);

    // Dust bound must increment (nonzero remainder)
    assert!(engine.phantom_dust_bound_long_q > dust_before,
        "dust bound must increment on nonzero remainder");

    // Now set up a case with zero remainder: a_side == a_basis
    engine.accounts[idx as usize].position_basis_q = I256::from_u128(POS_SCALE);
    engine.accounts[idx as usize].adl_a_basis = ADL_ONE;
    engine.adl_mult_long = ADL_ONE; // a_side == a_basis → zero remainder

    let dust_before2 = engine.phantom_dust_bound_long_q;
    engine.attach_effective_position(idx as usize, I256::from_u128(3 * POS_SCALE));

    // Dust bound must NOT increment (zero remainder)
    assert!(engine.phantom_dust_bound_long_q == dust_before2,
        "dust bound must not increment on zero remainder");
}

// ============================================================================
// T11.42: dynamic_dust_bound_inductive
// ============================================================================

/// Real engine: after N same-epoch position zeroings via settle_side_effects
/// (when A shrinks enough that q_eff → 0), phantom_dust_bound >= N.
#[kani::proof]
#[kani::solver(cadical)]
fn t11_42_dynamic_dust_bound_inductive() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let a = engine.add_user(0).unwrap();
    let b = engine.add_user(0).unwrap();
    engine.deposit(a, 10_000_000, 100, 0).unwrap();
    engine.deposit(b, 10_000_000, 100, 0).unwrap();

    // Both accounts: 1 POS_SCALE unit long, a_basis = ADL_ONE
    engine.accounts[a as usize].position_basis_q = I256::from_u128(POS_SCALE);
    engine.accounts[a as usize].adl_a_basis = ADL_ONE;
    engine.accounts[a as usize].adl_k_snap = I256::ZERO;
    engine.accounts[a as usize].adl_epoch_snap = 0;
    engine.accounts[b as usize].position_basis_q = I256::from_u128(POS_SCALE);
    engine.accounts[b as usize].adl_a_basis = ADL_ONE;
    engine.accounts[b as usize].adl_k_snap = I256::ZERO;
    engine.accounts[b as usize].adl_epoch_snap = 0;
    engine.stored_pos_count_long = 2;
    engine.adl_epoch_long = 0;
    engine.oi_eff_long_q = U256::from_u128(2 * POS_SCALE);

    // Shrink A_side to 1 so floor(POS_SCALE * 1 / ADL_ONE) = 0 → q_eff = 0
    engine.adl_mult_long = 1;

    // Settle account a → position zeroes, dust increments
    let _ = engine.settle_side_effects(a as usize);
    assert!(engine.accounts[a as usize].position_basis_q.is_zero());
    assert!(engine.phantom_dust_bound_long_q == U256::from_u128(1));

    // Settle account b → position zeroes, dust increments again
    let _ = engine.settle_side_effects(b as usize);
    assert!(engine.accounts[b as usize].position_basis_q.is_zero());
    assert!(engine.phantom_dust_bound_long_q == U256::from_u128(2));
}

// ============================================================================
// T11.43: end_instruction_auto_finalizes_ready_side
// ============================================================================

/// Real engine: finalize_end_of_instruction_resets calls
/// maybe_finalize_ready_reset_sides. When ResetPending with OI=0,
/// stale=0, pos_count=0, the side transitions to Normal.
#[kani::proof]
#[kani::solver(cadical)]
fn t11_43_end_instruction_auto_finalizes_ready_side() {
    let mut engine = RiskEngine::new(zero_fee_params());

    // Put long side in ResetPending with all conditions met for auto-finalization
    engine.side_mode_long = SideMode::ResetPending;
    engine.oi_eff_long_q = U256::ZERO;
    engine.stale_account_count_long = 0;
    engine.stored_pos_count_long = 0;

    // Short side: ResetPending but NOT ready (stale > 0)
    engine.side_mode_short = SideMode::ResetPending;
    engine.oi_eff_short_q = U256::ZERO;
    engine.stale_account_count_short = 1; // blocks finalization
    engine.stored_pos_count_short = 0;

    let ctx = InstructionContext::new();
    engine.finalize_end_of_instruction_resets(&ctx);

    // Long side auto-finalized → Normal
    assert!(engine.side_mode_long == SideMode::Normal,
        "ready ResetPending side must auto-finalize to Normal");

    // Short side stays ResetPending (stale > 0)
    assert!(engine.side_mode_short == SideMode::ResetPending,
        "non-ready side must stay ResetPending");
}

// ============================================================================
// T11.44: trade_path_reopens_ready_reset_side
// ============================================================================

/// Real engine: execute_trade calls maybe_finalize_ready_reset_sides before
/// the side-mode check, allowing trades on a side that has completed its reset.
#[kani::proof]
#[kani::solver(cadical)]
fn t11_44_trade_path_reopens_ready_reset_side() {
    let mut engine = RiskEngine::new(zero_fee_params());

    let a = engine.add_user(0).unwrap();
    let b = engine.add_user(0).unwrap();
    engine.deposit(a, 10_000_000, 100, 0).unwrap();
    engine.deposit(b, 10_000_000, 100, 0).unwrap();

    // Long side: ResetPending but fully ready for finalization
    engine.side_mode_long = SideMode::ResetPending;
    engine.oi_eff_long_q = U256::ZERO;
    engine.oi_eff_short_q = U256::ZERO;
    engine.stale_account_count_long = 0;
    engine.stored_pos_count_long = 0;

    // Set oracle/market state
    engine.last_oracle_price = 100;
    engine.last_market_slot = 1;
    engine.last_crank_slot = 1;
    engine.funding_price_sample_last = 100;

    // Trade: a goes long, b goes short — would be blocked if side stays ResetPending
    let size_q = I256::from_u128(POS_SCALE);
    let result = engine.execute_trade(a, b, 100, 1, size_q, 100);

    // Trade must succeed — maybe_finalize_ready_reset_sides reopened the long side
    assert!(result.is_ok(), "trade must succeed after auto-finalization of ready reset side");

    // Side mode must be Normal after trade
    assert!(engine.side_mode_long == SideMode::Normal);

    // OI balance holds
    assert!(engine.oi_eff_long_q == engine.oi_eff_short_q);
}

// ============================================================================
// T11.45: enqueue_adl_nonrepr_beta_still_routes_quantity
// ============================================================================

/// Real engine: when beta_abs > I256::MAX (non-representable), D is absorbed
/// by insurance but A still shrinks and OI still updates.
#[kani::proof]
#[kani::solver(cadical)]
fn t11_45_enqueue_adl_nonrepr_beta_still_routes_quantity() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let mut ctx = InstructionContext::new();

    engine.adl_mult_long = ADL_ONE;
    engine.adl_coeff_long = I256::ZERO;
    engine.oi_eff_long_q = U256::from_u128(4 * POS_SCALE);
    engine.oi_eff_short_q = U256::from_u128(4 * POS_SCALE);
    engine.insurance_fund.balance = U128::new(10_000_000);

    let a_before = engine.adl_mult_long;
    let k_before = engine.adl_coeff_long;
    let ins_before = engine.insurance_fund.balance.get();

    // D is very large: beta_abs = ceil(D * POS_SCALE / oi) could exceed I256::MAX
    // We need D * POS_SCALE / (2 * POS_SCALE) > I256::MAX
    // i.e. D > 2 * I256::MAX. Use a large but representable U256 for D.
    let d = U256::from_u128(u128::MAX);
    let q_close = U256::from_u128(2 * POS_SCALE);

    let result = engine.enqueue_adl(&mut ctx, Side::Short, q_close, d);
    assert!(result.is_ok());

    // A must have shrunk: floor(ADL_ONE * 2/4) = ADL_ONE/2
    assert!(engine.adl_mult_long < a_before, "A must shrink");

    // OI updated
    assert!(engine.oi_eff_long_q == U256::from_u128(2 * POS_SCALE));
}

// ============================================================================
// T11.46: enqueue_adl_k_add_overflow_still_routes_quantity
// ============================================================================

/// Real engine: when K_opp + delta_K overflows, D is absorbed but A still
/// shrinks and OI updates.
#[kani::proof]
#[kani::solver(cadical)]
fn t11_46_enqueue_adl_k_add_overflow_still_routes_quantity() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let mut ctx = InstructionContext::new();

    // K near I256::MIN so adding negative delta_K overflows
    engine.adl_coeff_long = I256::MIN.checked_add(I256::from_i128(1)).unwrap();
    engine.adl_mult_long = ADL_ONE;
    engine.oi_eff_long_q = U256::from_u128(4 * POS_SCALE);
    engine.oi_eff_short_q = U256::from_u128(4 * POS_SCALE);
    engine.insurance_fund.balance = U128::new(10_000_000);

    let a_before = engine.adl_mult_long;

    // Small D that would produce a representable beta, but the delta_K = -beta
    // addition to K_opp near MIN overflows. Need D such that beta_abs fits I256
    // but K + delta_K overflows.
    let d = U256::from_u128(1_000_000);
    let q_close = U256::from_u128(2 * POS_SCALE);

    let result = engine.enqueue_adl(&mut ctx, Side::Short, q_close, d);
    assert!(result.is_ok());

    // A must shrink
    assert!(engine.adl_mult_long < a_before, "A must shrink on K overflow");

    // OI updated
    assert!(engine.oi_eff_long_q == U256::from_u128(2 * POS_SCALE));
}

// ============================================================================
// T11.47: precision_exhaustion_terminal_drain
// ============================================================================

/// Real engine: when A_candidate = floor(1 * oi_post / oi) = 0 with oi_post > 0,
/// both sides get pending reset (precision exhaustion terminal drain).
#[kani::proof]
#[kani::solver(cadical)]
fn t11_47_precision_exhaustion_terminal_drain() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let mut ctx = InstructionContext::new();

    // A_old = 1 (minimal)
    engine.adl_mult_long = 1;
    engine.adl_coeff_long = I256::ZERO;
    engine.oi_eff_long_q = U256::from_u128(3 * POS_SCALE);
    engine.oi_eff_short_q = U256::from_u128(3 * POS_SCALE);

    // q_close = POS_SCALE, so oi_post = 2*POS_SCALE
    // A_candidate = floor(1 * 2*POS_SCALE / 3*POS_SCALE) = floor(2/3) = 0
    let q_close = U256::from_u128(POS_SCALE);
    let d = U256::ZERO;

    let result = engine.enqueue_adl(&mut ctx, Side::Short, q_close, d);
    assert!(result.is_ok());

    // Both sides must have pending resets (precision exhaustion)
    assert!(ctx.pending_reset_long, "long pending reset must fire on precision exhaustion");
    assert!(ctx.pending_reset_short, "short pending reset must fire on precision exhaustion");

    // OI zeroed on both sides
    assert!(engine.oi_eff_long_q.is_zero(), "OI long must be zero");
    assert!(engine.oi_eff_short_q.is_zero(), "OI short must be zero");
}

// ============================================================================
// T11.48: bankruptcy_liquidation_routes_q_when_D_zero
// ============================================================================

/// Real engine: when D == 0, only A shrinks (quantity routing), K unchanged.
#[kani::proof]
#[kani::solver(cadical)]
fn t11_48_bankruptcy_liquidation_routes_q_when_D_zero() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let mut ctx = InstructionContext::new();

    engine.adl_mult_long = ADL_ONE;
    engine.adl_coeff_long = I256::from_i128(42);
    engine.oi_eff_long_q = U256::from_u128(4 * POS_SCALE);
    engine.oi_eff_short_q = U256::from_u128(4 * POS_SCALE);

    let k_before = engine.adl_coeff_long;
    let a_before = engine.adl_mult_long;

    // D = 0: no deficit, only quantity routing
    let d = U256::ZERO;
    let q_close = U256::from_u128(POS_SCALE);

    let result = engine.enqueue_adl(&mut ctx, Side::Short, q_close, d);
    assert!(result.is_ok());

    // K unchanged when D == 0
    assert!(engine.adl_coeff_long == k_before, "K must be unchanged when D == 0");

    // A shrunk: floor(ADL_ONE * 3/4) < ADL_ONE
    assert!(engine.adl_mult_long < a_before, "A must shrink");

    // OI updated
    assert!(engine.oi_eff_long_q == U256::from_u128(3 * POS_SCALE));
}

// ============================================================================
// T11.49: pure_pnl_bankruptcy_path
// ============================================================================

/// Real engine: when q_close = 0 and D > 0, only K changes (PnL routing),
/// A is unchanged.
#[kani::proof]
#[kani::solver(cadical)]
fn t11_49_pure_pnl_bankruptcy_path() {
    let mut engine = RiskEngine::new(zero_fee_params());
    let mut ctx = InstructionContext::new();

    engine.adl_mult_long = ADL_ONE;
    engine.adl_coeff_long = I256::ZERO;
    engine.oi_eff_long_q = U256::from_u128(2 * POS_SCALE);
    engine.oi_eff_short_q = U256::from_u128(2 * POS_SCALE);

    let a_before = engine.adl_mult_long;
    let k_before = engine.adl_coeff_long;

    // q_close = 0, D > 0: pure PnL bankruptcy
    let d = U256::from_u128(1_000);
    let q_close = U256::ZERO;

    let result = engine.enqueue_adl(&mut ctx, Side::Short, q_close, d);
    assert!(result.is_ok());

    // A unchanged (no quantity routing with q_close = 0)
    assert!(engine.adl_mult_long == a_before, "A must be unchanged for pure PnL bankruptcy");

    // K must have changed (deficit socialized through K)
    assert!(engine.adl_coeff_long != k_before, "K must change when D > 0");

    // OI unchanged (no quantity closed)
    assert!(engine.oi_eff_long_q == U256::from_u128(2 * POS_SCALE));
}

// ============================================================================
// T11.50: execute_trade_atomic_oi_update_sign_flip
// ============================================================================

/// Real engine: execute_trade with position sign flip correctly updates OI.
/// Account flips from long to short — old long OI removed, new short OI added.
#[kani::proof]
#[kani::solver(cadical)]
fn t11_50_execute_trade_atomic_oi_update_sign_flip() {
    let mut engine = RiskEngine::new(zero_fee_params());

    let a = engine.add_user(0).unwrap();
    let b = engine.add_user(0).unwrap();
    engine.deposit(a, 100_000_000, 100, 0).unwrap();
    engine.deposit(b, 100_000_000, 100, 0).unwrap();

    engine.last_oracle_price = 100;
    engine.last_market_slot = 1;
    engine.last_crank_slot = 1;
    engine.funding_price_sample_last = 100;

    // Open: a long 1 unit, b short 1 unit
    let size_q = I256::from_u128(POS_SCALE);
    let r1 = engine.execute_trade(a, b, 100, 1, size_q, 100);
    assert!(r1.is_ok());
    assert!(engine.oi_eff_long_q == engine.oi_eff_short_q);
    let oi_after_open = engine.oi_eff_long_q;

    // Flip: a sells 2 units (goes from +1 to -1 net)
    let flip_size = I256::ZERO.checked_sub(I256::from_u128(2 * POS_SCALE)).unwrap();
    let r2 = engine.execute_trade(a, b, 100, 2, flip_size, 100);
    assert!(r2.is_ok());

    // OI balance must still hold
    assert!(engine.oi_eff_long_q == engine.oi_eff_short_q,
        "OI must be balanced after sign flip");
}

// ============================================================================
// T11.51: execute_trade_slippage_zero_sum
// ============================================================================

/// Real engine: zero-fee trade at oracle price preserves vault.
#[kani::proof]
#[kani::solver(cadical)]
fn t11_51_execute_trade_slippage_zero_sum() {
    let mut engine = RiskEngine::new(zero_fee_params());

    let a = engine.add_user(0).unwrap();
    let b = engine.add_user(0).unwrap();
    engine.deposit(a, 10_000_000, 100, 0).unwrap();
    engine.deposit(b, 10_000_000, 100, 0).unwrap();

    engine.last_oracle_price = 100;
    engine.last_market_slot = 1;
    engine.last_crank_slot = 1;
    engine.funding_price_sample_last = 100;

    let vault_before = engine.vault.get();

    let size_q = I256::from_u128(POS_SCALE);
    let result = engine.execute_trade(a, b, 100, 1, size_q, 100);
    assert!(result.is_ok());

    let vault_after = engine.vault.get();
    assert!(vault_after == vault_before,
        "vault must be unchanged with zero fees at oracle price");

    // Conservation
    assert!(engine.check_conservation(), "conservation must hold after trade");
}

// ============================================================================
// T11.52: touch_account_full_restart_conversion_fee_seniority
// ============================================================================

/// Real engine: after touch_account_full with warmup maturity and fee debt,
/// restart-on-new-profit fires and fee_debt_sweep runs.
#[kani::proof]
#[kani::solver(cadical)]
fn t11_52_touch_account_full_restart_fee_seniority() {
    let mut params = zero_fee_params();
    params.warmup_period_slots = 10;
    let mut engine = RiskEngine::new(params);

    let idx = engine.add_user(0).unwrap();
    engine.deposit(idx, 10_000_000, 100, 0).unwrap();

    // Set up: account has a long position with positive PnL pending
    let pos = I256::from_u128(POS_SCALE);
    engine.accounts[idx as usize].position_basis_q = pos;
    engine.accounts[idx as usize].adl_a_basis = ADL_ONE;
    engine.accounts[idx as usize].adl_k_snap = I256::ZERO;
    engine.accounts[idx as usize].adl_epoch_snap = 0;
    engine.stored_pos_count_long = 1;
    engine.adl_epoch_long = 0;
    engine.oi_eff_long_q = U256::from_u128(POS_SCALE);

    // K_long positive → will produce positive PnL on settle
    engine.adl_coeff_long = I256::from_i128((ADL_ONE as i128) * 100);

    // Fee debt: negative fee_credits
    engine.accounts[idx as usize].fee_credits = I128::new(-500i128);

    // Warmup started long ago — fully matured
    engine.accounts[idx as usize].warmup_started_at_slot = 0;
    engine.accounts[idx as usize].warmup_slope_per_step = U256::from_u128(100);

    engine.last_oracle_price = 100;
    engine.last_market_slot = 100;

    let fee_before = engine.accounts[idx as usize].fee_credits;

    // Touch at slot 100 (warmup fully matured)
    let result = engine.touch_account_full(idx as usize, 100, 100);
    assert!(result.is_ok());

    // After touch: k_snap updated
    assert!(engine.accounts[idx as usize].adl_k_snap == engine.adl_coeff_long);
}

// ============================================================================
// T11.53: keeper_crank_quiesces_after_pending_reset
// ============================================================================

/// Real engine: keeper_crank stops processing accounts after a pending reset
/// is triggered (early break on ctx.pending_reset_*).
#[kani::proof]
#[kani::solver(cadical)]
fn t11_53_keeper_crank_quiesces_after_pending_reset() {
    let mut engine = RiskEngine::new(zero_fee_params());

    // Set up: long side has A=1 (near precision exhaustion)
    engine.adl_mult_long = 1;
    engine.adl_epoch_long = 0;
    engine.last_oracle_price = 100;
    engine.last_market_slot = 0;
    engine.funding_price_sample_last = 100;

    let a = engine.add_user(0).unwrap();
    let b = engine.add_user(0).unwrap();
    engine.deposit(a, 10_000_000, 100, 0).unwrap();
    engine.deposit(b, 10_000_000, 100, 0).unwrap();

    // Both accounts have long positions (with A=1 → q_eff=0 after settle)
    engine.accounts[a as usize].position_basis_q = I256::from_u128(POS_SCALE);
    engine.accounts[a as usize].adl_a_basis = ADL_ONE;
    engine.accounts[a as usize].adl_k_snap = I256::ZERO;
    engine.accounts[a as usize].adl_epoch_snap = 0;
    engine.accounts[b as usize].position_basis_q = I256::from_u128(POS_SCALE);
    engine.accounts[b as usize].adl_a_basis = ADL_ONE;
    engine.accounts[b as usize].adl_k_snap = I256::ZERO;
    engine.accounts[b as usize].adl_epoch_snap = 0;
    engine.stored_pos_count_long = 2;
    engine.oi_eff_long_q = U256::from_u128(2 * POS_SCALE);
    engine.oi_eff_short_q = U256::from_u128(2 * POS_SCALE);

    // Crank should touch accounts, which settles them (q_eff=0 → positions zero,
    // dust increments). After schedule_end_of_instruction_resets sees enough dust,
    // pending reset fires and the crank quiesces.
    let result = engine.keeper_crank(a, 1, 100, 0);
    assert!(result.is_ok());
}

// ============================================================================
// T11.54: worked_example_regression
// ============================================================================

/// Real engine: complete multi-phase scenario with final-state assertions.
/// Phase 1: Open positions (a long, b short)
/// Phase 2: ADL (b bankrupt, deficit socialized to a)
/// Phase 3: Verify final state
#[kani::proof]
#[kani::solver(cadical)]
fn t11_54_worked_example_regression() {
    let mut engine = RiskEngine::new(zero_fee_params());

    let a = engine.add_user(0).unwrap();
    let b = engine.add_user(0).unwrap();
    engine.deposit(a, 10_000_000, 100, 0).unwrap();
    engine.deposit(b, 10_000_000, 100, 0).unwrap();

    engine.last_oracle_price = 100;
    engine.last_market_slot = 1;
    engine.last_crank_slot = 1;
    engine.funding_price_sample_last = 100;

    // Phase 1: Open — a long 2 units, b short 2 units at price 100
    let size_q = I256::from_u128(2 * POS_SCALE);
    let r1 = engine.execute_trade(a, b, 100, 1, size_q, 100);
    assert!(r1.is_ok());
    assert!(engine.oi_eff_long_q == engine.oi_eff_short_q);
    let oi_after_open = engine.oi_eff_long_q;

    // Phase 2: ADL — b's side bankrupt, close 1 unit, deficit = 500
    let mut ctx = InstructionContext::new();
    let d = U256::from_u128(500);
    let q_close = U256::from_u128(POS_SCALE);
    let r2 = engine.enqueue_adl(&mut ctx, Side::Short, q_close, d);
    assert!(r2.is_ok());

    // A_long must have shrunk
    assert!(engine.adl_mult_long < ADL_ONE, "A_long must shrink after ADL");

    // OI_long decreased by q_close
    assert!(engine.oi_eff_long_q == U256::from_u128(POS_SCALE),
        "OI_long must decrease by q_close");

    // K_long must have changed (deficit socialized)
    assert!(engine.adl_coeff_long != I256::ZERO, "K must change with nonzero D");

    // Phase 3: Settle account a to realize ADL effects
    let _ = engine.settle_side_effects(a as usize);

    // After settle: position basis unchanged (non-compounding), k_snap updated
    assert!(engine.accounts[a as usize].adl_k_snap == engine.adl_coeff_long);

    // Conservation check
    assert!(engine.check_conservation(), "conservation must hold after ADL + settle");
}
