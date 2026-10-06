//! v2.2 Phase 4 items 1 + 2: pure arithmetic for the per-epoch price band and the
//! holding-fee rent index.
//!
//! Everything here is a pure integer function with no engine state, so the
//! production code and the Kani / proptest harnesses call exactly the same
//! primitive (design `phase4-design-2026-10-05.md` §0.3, §1.4, §2.4).
//!
//! # Band (item 1)
//!
//! An asset with `band_bps = d > 0` carries an anchor price `A`. Every exposed
//! accrual must feed a price inside `[lo(A), hi(A)]`:
//!
//! ```text
//! lo(A) = ceil (A * (10_000 - d) / 10_000)
//! hi(A) = floor(A * (10_000 + d) / 10_000)        (capped at MAX_ORACLE_PRICE)
//! ```
//!
//! `lo` rounds up and `hi` rounds down, so the integer band is never wider than
//! the real one. The worst adverse move a leg can see between two consecutive
//! certifications is `G = ceil((10_000 + d)^2 / (10_000 - d)) - 10_000` bps of
//! the price at its last certification (proof sketch in the design §1.1 and in
//! `docs/v22-band-rent-kani-design.md`).
//!
//! # Rent (item 2)
//!
//! Each side keeps a monotone index `R_side` in `price * rate_e9 * slots` units.
//! A leg with effective size `q` owes `floor(q * (R_side - snap) / (POS_SCALE *
//! 10^9))` collateral atoms: floor rounding never overcharges.

use crate::wide_math::{mul_div_floor_u256_with_rem, U256};
use crate::{MAX_ORACLE_PRICE, POS_SCALE};

/// Upper bound on `band_bps` (20%). Wider bands are refused at InitMarket: with
/// `d > 2000` the two-epoch worst move `G` exceeds 7,000 bps and no sane
/// maintenance margin satisfies the Band Safety Law anyway.
pub const MAX_BAND_BPS: u64 = 2_000;

/// Sanity ceiling on the per-slot rent rate (1e-5 of notional per slot), on the
/// same scale as `max_abs_funding_e9_per_slot`'s 10_000 bound. The real bound is
/// the Band Safety Law / §1.6 envelope, which prices the rent into maintenance.
pub const MAX_RENT_E9_PER_SLOT: u64 = 10_000;

/// Rent index denominator: `POS_SCALE * 10^9`.
pub const RENT_INDEX_DEN: u128 = POS_SCALE * 1_000_000_000;

const BPS: u128 = 10_000;

/// Errors from the pure band / rent helpers. The engine maps them to `V16Error`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BandRentError {
    /// An input is outside its documented domain (zero / over-cap price, `d`
    /// outside `1..=MAX_BAND_BPS`, ...).
    InvalidInput,
    /// A checked add overflowed (index growth). Callers fail closed.
    Overflow,
}

/// `(lo, hi)` of the band around `anchor`. `band_bps` must be in
/// `1..=MAX_BAND_BPS` and `anchor` in `1..=MAX_ORACLE_PRICE`.
///
/// Guarantees: `1 <= lo <= anchor <= hi <= MAX_ORACLE_PRICE`.
pub fn band_bounds(anchor: u64, band_bps: u64) -> Result<(u64, u64), BandRentError> {
    if anchor == 0 || anchor > MAX_ORACLE_PRICE || band_bps == 0 || band_bps > MAX_BAND_BPS {
        return Err(BandRentError::InvalidInput);
    }
    let a = anchor as u128;
    let d = band_bps as u128;
    // anchor <= 1e12 and (BPS + d) <= 12_000, so the products fit in u128 easily.
    let lo_num = a * (BPS - d);
    let lo = lo_num / BPS + u128::from(!lo_num.is_multiple_of(BPS));
    let hi = (a * (BPS + d)) / BPS;
    let hi = hi.min(MAX_ORACLE_PRICE as u128);
    // lo >= 1: a >= 1 and BPS - d >= 8_000, so lo_num > 0 and the ceiling is >= 1.
    Ok((lo as u64, hi as u64))
}

/// Review E-M1: largest per-side position cap a band market may configure (the
/// design's `cfg_max_active_positions_per_side`). The keeper must certify every
/// positioned leg each epoch, so this bounds the sweep at `2 * cap` refreshes.
pub const BAND_MAX_POSITIONS_PER_SIDE: u64 = 256;
/// Review E-M2: floor on `E` (`band_max_epoch_slots`). ~1 minute at 400 ms slots:
/// one missed keeper pass must not pin the book.
pub const BAND_MIN_EPOCH_SLOTS: u64 = 150;
/// Review E-M2: `Pmax >= BAND_MIN_PIN_EPOCHS * E`: a pinned book gets at least
/// this many epochs of keeper time before `BandPinExpired` opens recovery.
pub const BAND_MIN_PIN_EPOCHS: u64 = 8;
/// Review E-L1: minimum band width `hi - lo` in price ticks. Below it the
/// integer band degenerates (at d = 130 every price <= 76 has `lo == hi`).
pub const MIN_BAND_WIDTH_TICKS: u64 = 32;

/// True iff the band around `anchor` is at least `MIN_BAND_WIDTH_TICKS` wide.
pub fn band_width_ok(anchor: u64, band_bps: u64) -> Result<bool, BandRentError> {
    let (lo, hi) = band_bounds(anchor, band_bps)?;
    Ok(hi - lo >= MIN_BAND_WIDTH_TICKS)
}

/// True iff `price` lies inside the band around `anchor`.
pub fn price_in_band(price: u64, anchor: u64, band_bps: u64) -> Result<bool, BandRentError> {
    let (lo, hi) = band_bounds(anchor, band_bps)?;
    Ok(lo <= price && price <= hi)
}

/// Clamps `price` into the band around `anchor`. Never overshoots: the result is
/// `price` itself when inside the band, otherwise the nearer edge.
pub fn clamp_to_band(price: u64, anchor: u64, band_bps: u64) -> Result<u64, BandRentError> {
    let (lo, hi) = band_bounds(anchor, band_bps)?;
    Ok(price.clamp(lo, hi))
}

/// `G = ceil((10_000 + d)^2 / (10_000 - d)) - 10_000`: the worst adverse move, in
/// bps of the price at a leg's last certification, before the leg must be
/// certified again or liquidated (two consecutive bands, see I-B6).
pub fn band_worst_adverse_bps(band_bps: u64) -> Result<u64, BandRentError> {
    if band_bps == 0 || band_bps > MAX_BAND_BPS {
        return Err(BandRentError::InvalidInput);
    }
    let d = band_bps as u128;
    let num = (BPS + d) * (BPS + d);
    let den = BPS - d;
    let ratio_bps = num / den + u128::from(!num.is_multiple_of(den));
    Ok((ratio_bps - BPS) as u64)
}

/// Is the duration pin in force for an accrual whose segment ends at
/// `segment_end_slot`, given the anchor slot that governs the segment?
///
/// The epoch's loss-accruing window is `[anchor_slot, anchor_slot + E]`. Past it,
/// accruals must be no-move (I-B4). A segment that ends before the anchor (cannot
/// happen with a monotone clock) is treated as inside the window.
pub fn band_duration_pinned(segment_end_slot: u64, anchor_slot: u64, max_epoch_slots: u64) -> bool {
    segment_end_slot.saturating_sub(anchor_slot) > max_epoch_slots
}

/// D-1 (spec divergence, NOT yet founder-approved). THE single code path that
/// feeds an unchanged price while the raw target differs.
///
/// Spec §1.7 / §9.6 forbids advancing `slot_last` by feeding the unchanged price
/// "merely to bypass the lag". The band does it on purpose, in two cases only:
///
/// 1. **Edge pin.** The capped staircase step would leave the band, so the price
///    is clamped to the band edge. Once at the edge, further steps feed the edge
///    (== `P_last`) until the book is certified and the anchor advances.
/// 2. **Duration pin.** The epoch's window `[anchor_slot, anchor_slot + E]` has
///    elapsed, so the accrual must be no-move (`price == P_last`, funding and
///    rent 0) until the book is certified.
///
/// Why it is not a *silent* bypass: the raw target stays stored and visible, the
/// §9.7 target/effective lag gates stay armed (risk-increasing trades and
/// extraction are refused), and a pin is bounded by `band_max_pin_slots`, after
/// which `BandPinExpired` recovery takes over.
///
/// If the founder rejects D-1, this function is the place to change: returning an
/// error for a pinned step (instead of `current`) restores the strict §9.6
/// behaviour at the cost of bricking refresh while pinned (design §1.1).
///
/// Inputs: `current` (= `P_last`), `capped_step` (the canonical cap-law step
/// toward the target), the anchor and `d`, and whether the duration pin is in
/// force. Returns the price to feed and whether the step was pinned (fed the
/// unchanged price / an edge instead of the cap-law step).
pub fn band_d1_pinned_step(
    current: u64,
    capped_step: u64,
    anchor: u64,
    band_bps: u64,
    duration_pinned: bool,
) -> Result<(u64, bool), BandRentError> {
    if duration_pinned {
        return Ok((current, capped_step != current));
    }
    let clamped = clamp_to_band(capped_step, anchor, band_bps)?;
    Ok((clamped, clamped != capped_step))
}

/// One side's rent-index increment for an accrual of `dt` slots at `price`:
/// `price * rate_e9 * dt`, fail-closed on overflow (I-R2). Zero when the rate,
/// `dt` or `price` is zero.
pub fn rent_index_delta(price: u64, rate_e9_per_slot: u64, dt: u64) -> Result<u128, BandRentError> {
    (price as u128)
        .checked_mul(rate_e9_per_slot as u128)
        .and_then(|v| v.checked_mul(dt as u128))
        .ok_or(BandRentError::Overflow)
}

/// Rent owed by a leg of effective size `abs_q` whose snapshot is `snap` when the
/// side index is `index`, with the leg's sub-atom carry `carry` (< RENT_INDEX_DEN):
///
/// ```text
/// total = abs_q * (index - snap) + carry
/// due   = floor(total / RENT_INDEX_DEN)      carry' = total mod RENT_INDEX_DEN
/// ```
///
/// Carrying the remainder makes rent exact under any settle schedule: the atoms
/// charged over many settles equal one settle over the whole interval (a keeper
/// that certifies every epoch cannot round a small position's rent to zero, and
/// splitting a settle never over- or under-charges by more than the final carry).
/// The carry is < 1 atom, so it stays meaningful across a resize of the leg.
///
/// `snap > index` or `carry >= RENT_INDEX_DEN` is impossible for a well-formed leg
/// and fails closed. A due above `u128::MAX` saturates (the charge is capped at
/// capital anyway).
pub fn rent_due_with_carry(
    abs_q: u128,
    index: u128,
    snap: u128,
    carry: u64,
) -> Result<(u128, u64), BandRentError> {
    if snap > index || carry as u128 >= RENT_INDEX_DEN {
        return Err(BandRentError::InvalidInput);
    }
    let delta = index - snap;
    if let Some(total) = abs_q
        .checked_mul(delta)
        .and_then(|v| v.checked_add(carry as u128))
    {
        return Ok((total / RENT_INDEX_DEN, (total % RENT_INDEX_DEN) as u64));
    }
    let product = U256::from_u128(abs_q)
        .checked_mul(U256::from_u128(delta))
        .and_then(|v| v.checked_add(U256::from_u128(carry as u128)))
        .ok_or(BandRentError::Overflow)?;
    let (q, r) = mul_div_floor_u256_with_rem(product, U256::ONE, U256::from_u128(RENT_INDEX_DEN));
    let carry_out = r.try_into_u128().ok_or(BandRentError::Overflow)? as u64;
    Ok((q.try_into_u128().unwrap_or(u128::MAX), carry_out))
}

/// Carry-free form: `floor(abs_q * (index - snap) / RENT_INDEX_DEN)`.
pub fn rent_due_atoms(abs_q: u128, index: u128, snap: u128) -> Result<u128, BandRentError> {
    rent_due_with_carry(abs_q, index, snap, 0).map(|(due, _)| due)
}

/// The chargeable rent for an account: `min(due, capital - max(-pnl, 0))`.
///
/// Capital that an unsettled loss already owns is never taken for rent, so rent
/// stays junior to losses (the same seniority the N1 fee waiver protects); time
/// the account cannot pay for is forgone, never booked as debt (I-R7).
pub fn rent_chargeable_atoms(due: u128, capital: u128, pnl: i128) -> u128 {
    let owned_by_loss = if pnl < 0 { pnl.unsigned_abs() } else { 0 };
    due.min(capital.saturating_sub(owned_by_loss))
}
