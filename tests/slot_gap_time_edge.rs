//! Time edge cases for the v2.2 risk engine (job P24, tests only).
//!
//! Every engine entry point that takes a slot/clock and accrues over elapsed time:
//! `accrue_asset_to_not_atomic` (funding, capped per call by `max_accrual_dt_slots`),
//! `sync_account_fee_to_slot_not_atomic` (maintenance fee, anchored to the loss-currentness
//! slot), `resolve_market_not_atomic` / `advance_resolved_slot_not_atomic` (resolved clock) and
//! `band_pin_expired_at` (config-off path only). Cases here are the ones the funding / fees / band-rent / crank edge
//! files do not hold: slot 0, slot regression, gaps up to `u64::MAX`, catch-up in capped
//! segments, the fee anchor on a stale asset, and the resolved fee cap.
//!
//! Direct state writes are named in their test (`try_accrue` copies the accrued price into
//! `raw_oracle_target_price`, which is wrapper state).

use percolator::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, TradeRequestV16, V16Config, V16Error, V16PodU64,
};
use percolator::{ADL_ONE, POS_SCALE};

const FUNDING_DEN: u128 = 1_000_000_000;
const MAX_RATE: u64 = 1_000;
const MAX_DT: u64 = 50;
const CAPITAL: u128 = 1_000_000_000_000;
const PRICE: u64 = 1_000_000;

type Slab = (MarketGroupV16HeaderAccount, Vec<Market<u64>>);

fn market(init_slot: u64) -> Slab {
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    cfg.max_abs_funding_e9_per_slot = MAX_RATE;
    cfg.max_accrual_dt_slots = MAX_DT;
    cfg.min_funding_lifetime_slots = MAX_DT;
    cfg.max_price_move_bps_per_slot = 10;
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut markets = vec![Market::new(0, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, PRICE, init_slot)
        .unwrap();
    (header, markets)
}

fn account(seed: u32) -> PortfolioAccountV16Account {
    let mut key = [0u8; 32];
    key[..4].copy_from_slice(&seed.to_le_bytes());
    key[31] = 0x4F;
    let h =
        ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new([1; 32], key, [3; 32]));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(h).unwrap();
    a
}

fn funded(slab: &mut Slab, seed: u32) -> PortfolioAccountV16Account {
    let mut a = account(seed);
    let mut m = MarketGroupV16ViewMut::new(&mut slab.0, &mut slab.1);
    m.deposit_not_atomic(&mut PortfolioV16ViewMut::new(&mut a), CAPITAL)
        .unwrap();
    a
}

fn trade(
    slab: &mut Slab,
    long: &mut PortfolioAccountV16Account,
    short: &mut PortfolioAccountV16Account,
    units: u128,
) {
    let mut m = MarketGroupV16ViewMut::new(&mut slab.0, &mut slab.1);
    m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
        &mut PortfolioV16ViewMut::new(long),
        &mut PortfolioV16ViewMut::new(short),
        TradeRequestV16 {
            asset_index: 0,
            size_q: (units * POS_SCALE) as i128,
            exec_price: PRICE,
            fee_bps: 0,
        },
        true,
    )
    .unwrap();
}

fn try_accrue(slab: &mut Slab, slot: u64, rate: i128) -> Result<(), V16Error> {
    let mut m = MarketGroupV16ViewMut::new(&mut slab.0, &mut slab.1);
    m.accrue_asset_to_not_atomic(0, slot, PRICE, rate, true)?;
    m.markets[0].engine.asset.raw_oracle_target_price = V16PodU64::new(PRICE);
    Ok(())
}

fn asset(slab: &Slab) -> percolator::AssetStateV16 {
    slab.1[0].engine.asset.try_to_runtime().unwrap()
}

fn validate(slab: &mut Slab, accts: &mut [&mut PortfolioAccountV16Account]) {
    let m = MarketGroupV16ViewMut::new(&mut slab.0, &mut slab.1);
    m.validate_shape().unwrap();
    for a in accts.iter_mut() {
        PortfolioV16ViewMut::new(a)
            .validate_with_market(&m.as_view())
            .unwrap();
    }
}

fn book(init_slot: u64) -> (Slab, PortfolioAccountV16Account, PortfolioAccountV16Account) {
    let mut s = market(init_slot);
    let mut long = funded(&mut s, 1);
    let mut short = funded(&mut s, 2);
    trade(&mut s, &mut long, &mut short, 10);
    (s, long, short)
}

/// |F| after `dt` slots at `rate` when A == ADL_ONE.
fn exact_f(rate: i128, dt: u64) -> i128 {
    (rate.unsigned_abs() * dt as u128 * PRICE as u128 * (ADL_ONE / FUNDING_DEN)) as i128
}

// ---------------------------------------------------------------------------------------------
// accrue_asset_to_not_atomic: slot 0, regression, gaps
// ---------------------------------------------------------------------------------------------

/// A clock that runs backwards is refused and the asset is left byte-identical.
#[test]
fn accrue_to_an_earlier_slot_is_refused_and_atomic() {
    let (mut s, _l, _s) = book(1);
    try_accrue(&mut s, 10, 0).unwrap();
    let before = s.1[0].engine.asset;
    let hdr_slot = (s.0.slot_last, s.0.current_slot);
    for slot in [9, 1, 0] {
        assert_eq!(try_accrue(&mut s, slot, 500), Err(V16Error::InvalidConfig));
        assert_eq!(s.1[0].engine.asset, before, "slot {slot} mutated state");
    }
    assert_eq!((s.0.slot_last, s.0.current_slot), hdr_slot);
}

/// Slot 0 on a market activated at slot 1 is a regression, not a free pass.
#[test]
fn accrue_at_slot_zero_on_fresh_market_is_refused() {
    let mut s = market(1);
    let before = s.1[0].engine.asset;
    assert_eq!(try_accrue(&mut s, 0, 0), Err(V16Error::InvalidConfig));
    assert_eq!(s.1[0].engine.asset, before);
}

/// A market activated at slot 0 accepts slot-0 accrual as a no-op and moves on normally.
#[test]
fn accrue_at_slot_zero_on_market_born_at_zero_is_noop() {
    let mut s = market(0);
    assert_eq!(asset(&s).slot_last, 0);
    try_accrue(&mut s, 0, 0).unwrap();
    assert_eq!(asset(&s).slot_last, 0);
    try_accrue(&mut s, 7, 0).unwrap();
    assert_eq!(asset(&s).slot_last, 7);
    validate(&mut s, &mut []);
}

/// A gap of `u64::MAX` neither overflows nor panics: one call commits at most
/// `max_accrual_dt_slots`, and F moves by exactly that segment's funding.
#[test]
fn accrue_u64_max_gap_commits_only_one_capped_segment() {
    let (mut s, mut long, mut short) = book(1);
    try_accrue(&mut s, u64::MAX, 400).unwrap();
    let a = asset(&s);
    assert_eq!(a.slot_last, 1 + MAX_DT);
    assert_eq!(s.0.slot_last.get(), 1 + MAX_DT);
    let f = exact_f(400, MAX_DT);
    assert_eq!((a.f_long_num, a.f_short_num), (-f, f));
    validate(&mut s, &mut [&mut long, &mut short]);
}

/// Gap of exactly `max_dt`, `max_dt + 1` and `max_dt - 1`: the committed segment is
/// `min(gap, max_dt)`, never more.
#[test]
fn accrue_gap_around_the_cap_boundary() {
    for (gap, committed) in [
        (MAX_DT - 1, MAX_DT - 1),
        (MAX_DT, MAX_DT),
        (MAX_DT + 1, MAX_DT),
    ] {
        let (mut s, _l, _s) = book(1);
        try_accrue(&mut s, 1 + gap, 300).unwrap();
        let a = asset(&s);
        assert_eq!(a.slot_last, 1 + committed, "gap {gap}");
        assert_eq!(a.f_short_num, exact_f(300, committed), "gap {gap}");
        assert_eq!(a.f_long_num, -exact_f(300, committed), "gap {gap}");
    }
}

/// Catching up a long gap by repeated calls to the same target slot accrues each capped
/// segment once: F is the sum of the segments, it conserves (long + short == 0 at A == ADL_ONE),
/// and once caught up a further call at the same slot is a no-op.
#[test]
fn accrue_repeated_calls_to_one_target_catch_up_in_capped_segments() {
    let (mut s, mut long, mut short) = book(1);
    let target = 1 + 3 * MAX_DT + 7;
    let mut prev = asset(&s).slot_last;
    let mut calls = 0;
    while asset(&s).slot_last < target {
        try_accrue(&mut s, target, 250).unwrap();
        let now = asset(&s).slot_last;
        assert!(now > prev && now - prev <= MAX_DT);
        prev = now;
        calls += 1;
        assert!(calls <= 10, "catch-up did not converge");
    }
    assert_eq!(calls, 4);
    let a = asset(&s);
    assert_eq!(a.slot_last, target);
    let f = exact_f(250, target - 1);
    assert_eq!((a.f_long_num, a.f_short_num), (-f, f));
    let frozen = s.1[0].engine.asset;
    try_accrue(&mut s, target, 250).unwrap();
    try_accrue(&mut s, target, -250).unwrap();
    assert_eq!(s.1[0].engine.asset, frozen);
    validate(&mut s, &mut [&mut long, &mut short]);
}

/// Funding settled after four capped catch-up calls toward a ~1M-slot target is zero-sum between the two counterparties.
#[test]
fn accrue_gap_funding_settlement_is_zero_sum() {
    let (mut s, mut long, mut short) = book(1);
    let total = |l: &PortfolioAccountV16Account, sh: &PortfolioAccountV16Account| {
        (l.capital.get() as i128 + l.pnl.get()) + (sh.capital.get() as i128 + sh.pnl.get())
    };
    let before = total(&long, &short);
    for _ in 0..4 {
        try_accrue(&mut s, 1_000_000, 700).unwrap();
    }
    for a in [&mut long, &mut short] {
        let mut m = MarketGroupV16ViewMut::new(&mut s.0, &mut s.1);
        m.full_account_refresh_not_atomic(&mut PortfolioV16ViewMut::new(a))
            .unwrap();
    }
    let after = total(&long, &short);
    // Rounding may leave dust with the engine, never create value for the pair.
    assert!(
        after <= before && before - after <= 2,
        "{before} -> {after}"
    );
    // Positive rate: the long pays, the short receives (loss may settle into capital).
    let eq = |a: &PortfolioAccountV16Account| a.capital.get() as i128 + a.pnl.get();
    assert!(eq(&long) < CAPITAL as i128 && eq(&short) > CAPITAL as i128);
    validate(&mut s, &mut [&mut long, &mut short]);
}

// ---------------------------------------------------------------------------------------------
// sync_account_fee_to_slot_not_atomic
// ---------------------------------------------------------------------------------------------

/// Slot 0 on a fresh account: nothing to charge, cursor stays put.
#[test]
fn fee_sync_at_slot_zero_charges_nothing() {
    let mut s = market(0);
    let mut a = funded(&mut s, 1);
    let mut m = MarketGroupV16ViewMut::new(&mut s.0, &mut s.1);
    let mut v = PortfolioV16ViewMut::new(&mut a);
    let cursor = v.header.last_fee_slot.get();
    assert_eq!(
        m.sync_account_fee_to_slot_not_atomic(&mut v, 0, 1_000),
        Ok(0)
    );
    assert_eq!(v.header.last_fee_slot.get(), cursor);
    assert_eq!(v.header.capital.get(), CAPITAL);
}

/// A zero rate over a huge gap charges nothing but still moves the cursor to now.
#[test]
fn fee_sync_zero_rate_huge_gap_moves_cursor_only() {
    let mut s = market(1);
    let mut a = funded(&mut s, 1);
    let mut m = MarketGroupV16ViewMut::new(&mut s.0, &mut s.1);
    let mut v = PortfolioV16ViewMut::new(&mut a);
    assert_eq!(
        m.sync_account_fee_to_slot_not_atomic(&mut v, u64::MAX, 0),
        Ok(0)
    );
    assert_eq!(v.header.last_fee_slot.get(), u64::MAX);
    assert_eq!(v.header.capital.get(), CAPITAL);
}

/// Splitting a fee period at any midpoint charges the same total as one sync, and capital plus
/// insurance is conserved.
#[test]
fn fee_sync_is_additive_across_midpoints_and_conserves_value() {
    for mid in [0u64, 1, 2, 49, 99, 100] {
        let mut s = market(1);
        let mut a = funded(&mut s, 1);
        let mut m = MarketGroupV16ViewMut::new(&mut s.0, &mut s.1);
        let mut v = PortfolioV16ViewMut::new(&mut a);
        let c0 = v.header.capital.get();
        let ins0 = m.header.insurance.get();
        let start = v.header.last_fee_slot.get();
        let mut charged = 0;
        if mid != 0 {
            charged += m
                .sync_account_fee_to_slot_not_atomic(&mut v, start + mid, 7)
                .unwrap();
        }
        charged += m
            .sync_account_fee_to_slot_not_atomic(&mut v, start + 100, 7)
            .unwrap();
        assert_eq!(charged, 700, "mid {mid}");
        assert_eq!(v.header.capital.get(), c0 - 700);
        assert_eq!(m.header.insurance.get(), ins0 + 700);
        m.validate_shape().unwrap();
    }
}

/// With an open position on an asset whose clock lags (`slot_last` behind the caller's slot),
/// the fee is only charged up to the asset's slot, and the rest once it catches up (checked below). Charging to
/// the caller's slot would bill time the market has not priced yet.
#[test]
fn fee_sync_with_open_position_stops_at_stale_asset_slot() {
    let (mut s, mut long, mut short) = book(1);
    try_accrue(&mut s, 20, 0).unwrap();
    let asset_slot = asset(&s).slot_last;
    assert_eq!(asset_slot, 20);
    let start = long.last_fee_slot.get();
    let (c0, ins0) = (long.capital.get(), s.0.insurance.get());
    let mut m = MarketGroupV16ViewMut::new(&mut s.0, &mut s.1);
    let first = m
        .sync_account_fee_to_slot_not_atomic(&mut PortfolioV16ViewMut::new(&mut long), 500, 3)
        .unwrap();
    assert_eq!(long.last_fee_slot.get(), asset_slot);
    assert_eq!(first, 3 * (asset_slot - start) as u128);
    // Same call again: anchor has not moved, nothing more to charge.
    let again = m
        .sync_account_fee_to_slot_not_atomic(&mut PortfolioV16ViewMut::new(&mut long), 500, 3)
        .unwrap();
    assert_eq!(again, 0);
    assert_eq!(long.capital.get(), c0 - first);
    assert_eq!(m.header.insurance.get(), ins0 + first);
    m.validate_shape().unwrap();
    // The asset catches up by one capped segment; the next sync bills exactly that remainder.
    try_accrue(&mut s, 20 + MAX_DT, 0).unwrap();
    let mut m = MarketGroupV16ViewMut::new(&mut s.0, &mut s.1);
    let rest = m
        .sync_account_fee_to_slot_not_atomic(&mut PortfolioV16ViewMut::new(&mut long), 500, 3)
        .unwrap();
    assert_eq!(rest, 3 * MAX_DT as u128);
    assert_eq!(long.last_fee_slot.get(), 20 + MAX_DT);
    validate(&mut s, &mut [&mut long, &mut short]);
}

// ---------------------------------------------------------------------------------------------
// Resolved clock
// ---------------------------------------------------------------------------------------------

/// Resolution cannot be dated before the committed slot; the current slot itself and
/// `u64::MAX` are both fine, and a refused resolve leaves the market live.
#[test]
fn resolve_slot_bounds() {
    let mut s = market(5);
    let mut m = MarketGroupV16ViewMut::new(&mut s.0, &mut s.1);
    let cur = m.header.current_slot.get();
    assert!(cur > 1);
    for bad in [0, cur - 1] {
        assert_eq!(m.resolve_market_not_atomic(bad), Err(V16Error::Stale));
    }
    assert_eq!(m.header.resolved_slot.get(), 0);
    assert_eq!(
        m.header.mode, 0,
        "refused resolve must leave the market Live"
    );
    m.resolve_market_not_atomic(cur).unwrap();
    assert_eq!(m.header.resolved_slot.get(), cur);

    let mut s2 = market(5);
    let mut m2 = MarketGroupV16ViewMut::new(&mut s2.0, &mut s2.1);
    m2.resolve_market_not_atomic(u64::MAX).unwrap();
    assert_eq!(m2.header.current_slot.get(), u64::MAX);
    assert_eq!(m2.header.resolved_slot.get(), u64::MAX);
}

/// Resolved clock: the same slot again is accepted, `u64::MAX` is accepted, and after that any
/// smaller slot is Stale. `resolved_slot` itself never moves and the vault is untouched.
#[test]
fn advance_resolved_slot_repeat_max_and_regress() {
    let mut s = market(1);
    let _a = funded(&mut s, 1);
    let mut m = MarketGroupV16ViewMut::new(&mut s.0, &mut s.1);
    let vault = m.header.vault.get();
    m.resolve_market_not_atomic(10).unwrap();
    m.advance_resolved_slot_not_atomic(10).unwrap();
    m.advance_resolved_slot_not_atomic(10).unwrap();
    m.advance_resolved_slot_not_atomic(u64::MAX).unwrap();
    m.advance_resolved_slot_not_atomic(u64::MAX).unwrap();
    assert_eq!(
        m.advance_resolved_slot_not_atomic(u64::MAX - 1),
        Err(V16Error::Stale)
    );
    assert_eq!(m.advance_resolved_slot_not_atomic(0), Err(V16Error::Stale));
    assert_eq!(m.header.current_slot.get(), u64::MAX);
    assert_eq!(m.header.resolved_slot.get(), 10);
    assert_eq!(m.header.vault.get(), vault);
    m.validate_shape().unwrap();
}

/// After resolution the maintenance fee stops at `resolved_slot`: a sync at a far later slot
/// charges only up to resolution, and the cursor lands on `resolved_slot`.
#[test]
fn fee_sync_after_resolution_is_capped_at_resolved_slot() {
    let mut s = market(1);
    let mut a = funded(&mut s, 1);
    let mut m = MarketGroupV16ViewMut::new(&mut s.0, &mut s.1);
    let mut v = PortfolioV16ViewMut::new(&mut a);
    let start = v.header.last_fee_slot.get();
    m.resolve_market_not_atomic(start + 40).unwrap();
    m.advance_resolved_slot_not_atomic(start + 1_000_000)
        .unwrap();
    let charged = m
        .sync_account_fee_to_slot_not_atomic(&mut v, start + 1_000_000, 2)
        .unwrap();
    assert_eq!(charged, 80);
    assert_eq!(v.header.last_fee_slot.get(), start + 40);
    assert_eq!(
        m.sync_account_fee_to_slot_not_atomic(&mut v, start + 2_000_000, 2),
        Ok(0)
    );
    assert_eq!(v.header.capital.get(), CAPITAL - 80);
    m.validate_shape().unwrap();
}

// ---------------------------------------------------------------------------------------------
// band_pin_expired_at
// ---------------------------------------------------------------------------------------------

/// With the band off there is nothing to pin: false at slot 0 and at `u64::MAX`.
#[test]
fn band_pin_expired_is_false_when_band_is_off() {
    let mut s = market(1);
    let v = MarketGroupV16ViewMut::new(&mut s.0, &mut s.1);
    assert_eq!(v.band_pin_expired_at(0), Ok(false));
    assert_eq!(v.band_pin_expired_at(1), Ok(false));
    assert_eq!(v.band_pin_expired_at(u64::MAX), Ok(false));
}
