//! Funding edge cases for the v2.2 risk engine (job P4, tests only).
//!
//! Spec §5.3 (spec.md:1093-1138): funding is active only when the rate is nonzero, BOTH sides
//! carry effective OI, and `fund_px_last > 0`; it moves `F_long` down and `F_short` up by
//! `A_side * fund_num_total` (spec F units carry FUNDING_DEN; the engine stores
//! `A_side * fund_num_total / FUNDING_DEN`, payer rounded up and receiver rounded down when A is
//! below ADL_ONE, and the tests follow the engine's units). Settlement realizes
//! `basis * (F_now - F_snap) / (a_basis * POS_SCALE)` with
//! a persistent remainder, and `record_account_funding_flow` books the gross funding leg of
//! every settlement into the account's per-side paid/received counters.
//!
//! Tests drive the public engine API (accrue, trade, refresh). Direct state writes, each one
//! named in its test: `try_accrue` sets `raw_oracle_target_price` to the accrued price after
//! every accrual, playing the wrapper's role (the oracle target is wrapper state); the
//! one-sided-OI, asymmetric-A, overflow and epoch tests set one or two asset fields to reach a
//! state (ADL-shrunk A, exhausted OI, near-limit F) that no short public sequence produces.

use percolator::{
    AssetStateV16Account, EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount,
    MarketGroupV16ViewMut, PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, SideModeV16, TradeRequestV16, V16Config, V16Error, V16PodU64,
};
use percolator::{ADL_ONE, MIN_A_SIDE, POS_SCALE};

const FUNDING_DEN: u128 = 1_000_000_000;
const MAX_RATE: u64 = 1_000;
const MAX_DT: u64 = 50;
const CAPITAL: u128 = 1_000_000_000_000;

type Slab = (MarketGroupV16HeaderAccount, Vec<Market<u64>>);

fn market(price: u64) -> Slab {
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    cfg.max_abs_funding_e9_per_slot = MAX_RATE;
    cfg.max_accrual_dt_slots = MAX_DT;
    cfg.min_funding_lifetime_slots = MAX_DT;
    cfg.max_price_move_bps_per_slot = 10;
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut markets = vec![Market::new(0, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, price, 1)
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

/// `long` buys `size_q` from `short` at `price` (negative size reverses the direction).
fn trade(
    slab: &mut Slab,
    long: &mut PortfolioAccountV16Account,
    short: &mut PortfolioAccountV16Account,
    size_q: i128,
    price: u64,
) {
    let mut m = MarketGroupV16ViewMut::new(&mut slab.0, &mut slab.1);
    m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
        &mut PortfolioV16ViewMut::new(long),
        &mut PortfolioV16ViewMut::new(short),
        TradeRequestV16 {
            asset_index: 0,
            size_q,
            exec_price: price,
            fee_bps: 0,
        },
        true,
    )
    .unwrap();
}

fn try_accrue(
    slab: &mut Slab,
    slot: u64,
    price: u64,
    rate: i128,
    protective: bool,
) -> Result<(), V16Error> {
    let mut m = MarketGroupV16ViewMut::new(&mut slab.0, &mut slab.1);
    m.accrue_asset_to_not_atomic(0, slot, price, rate, protective)?;
    m.markets[0].engine.asset.raw_oracle_target_price = V16PodU64::new(price);
    Ok(())
}

fn accrue(slab: &mut Slab, slot: u64, price: u64, rate: i128) {
    try_accrue(slab, slot, price, rate, true).unwrap();
}

fn refresh(slab: &mut Slab, a: &mut PortfolioAccountV16Account) {
    let mut m = MarketGroupV16ViewMut::new(&mut slab.0, &mut slab.1);
    m.full_account_refresh_not_atomic(&mut PortfolioV16ViewMut::new(a))
        .unwrap();
}

fn asset(slab: &Slab) -> percolator::AssetStateV16 {
    slab.1[0].engine.asset.try_to_runtime().unwrap()
}

fn equity(a: &PortfolioAccountV16Account) -> i128 {
    a.capital.get() as i128 + a.pnl.get()
}

/// (long paid, long received, short paid, short received)
fn flows(a: &PortfolioAccountV16Account) -> (u128, u128, u128, u128) {
    (
        a.funding_long_paid_atoms_total.get(),
        a.funding_long_received_atoms_total.get(),
        a.funding_short_paid_atoms_total.get(),
        a.funding_short_received_atoms_total.get(),
    )
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

/// Balanced book: one long and one short of `units` at `price`, opened at slot 1.
fn book(price: u64, units: u128) -> (Slab, PortfolioAccountV16Account, PortfolioAccountV16Account) {
    let mut s = market(price);
    let mut long = funded(&mut s, 1);
    let mut short = funded(&mut s, 2);
    trade(
        &mut s,
        &mut long,
        &mut short,
        (units * POS_SCALE) as i128,
        price,
    );
    (s, long, short)
}

/// Exact F index magnitude for a side with A == ADL_ONE: |rate| * dt * price * ADL_ONE / 1e9.
fn exact_f(rate: i128, dt: u64, price: u64) -> i128 {
    (rate.unsigned_abs() * dt as u128 * price as u128 * (ADL_ONE / FUNDING_DEN)) as i128
}

// ---------------------------------------------------------------------------------------------
// Accrual gating (spec §5.3 `funding_active`)
// ---------------------------------------------------------------------------------------------

/// An empty book has no funding to charge: a nonzero rate leaves F alone, needs no protective
/// progress, and still advances the clock.
#[test]
fn funding_empty_book_rate_is_inert_and_clock_advances() {
    let mut s = market(1_000_000);
    try_accrue(&mut s, 10, 1_000_000, MAX_RATE as i128, false).unwrap();
    let a = asset(&s);
    assert_eq!((a.f_long_num, a.f_short_num), (0, 0));
    assert_eq!(a.slot_last, 10);
    validate(&mut s, &mut []);
}

/// A zero rate on a positioned book moves no F and is not "equity active", so it does not need
/// protective progress (lenient: a keeper can always tick the clock forward).
#[test]
fn funding_zero_rate_on_positioned_book_moves_nothing() {
    let (mut s, mut long, mut short) = book(1_000_000, 10);
    try_accrue(&mut s, 30, 1_000_000, 0, false).unwrap();
    let a = asset(&s);
    assert_eq!((a.f_long_num, a.f_short_num), (0, 0));
    assert_eq!(a.slot_last, 30);
    refresh(&mut s, &mut long);
    refresh(&mut s, &mut short);
    assert_eq!(flows(&long), (0, 0, 0, 0));
    assert_eq!(flows(&short), (0, 0, 0, 0));
    validate(&mut s, &mut [&mut long, &mut short]);
}

/// A one-sided book (effective OI on one side only, as ADL can leave) must not accrue funding:
/// there is nobody to pay or receive. Spec: `funding_active` needs BOTH OI_eff sides nonzero.
/// With no price move it is not equity-active either, so it needs no protective progress.
#[test]
fn funding_one_sided_oi_does_not_accrue() {
    for empty_side in [0, 1] {
        let (mut s, _long, _short) = book(1_000_000, 10);
        let mut a = asset(&s);
        if empty_side == 0 {
            a.oi_eff_long_q = 0;
        } else {
            a.oi_eff_short_q = 0;
        }
        s.1[0].engine.asset = AssetStateV16Account::from_runtime(&a);
        for rate in [MAX_RATE as i128, -(MAX_RATE as i128)] {
            let before = asset(&s);
            let slot = before.slot_last + 5;
            try_accrue(&mut s, slot, 1_000_000, rate, false).unwrap();
            let after = asset(&s);
            assert_eq!(
                after.f_long_num, before.f_long_num,
                "side {empty_side} rate {rate}"
            );
            assert_eq!(
                after.f_short_num, before.f_short_num,
                "side {empty_side} rate {rate}"
            );
            assert_eq!(after.slot_last, slot);
        }
    }
}

/// Same-slot accrual (dt == 0) with a nonzero rate is a no-op, not an error.
#[test]
fn funding_same_slot_accrual_is_noop() {
    let (mut s, _long, _short) = book(1_000_000, 10);
    accrue(&mut s, 5, 1_000_000, 500);
    let before = asset(&s);
    accrue(&mut s, 5, 1_000_000, 500);
    accrue(&mut s, 5, 1_000_000, -500);
    let after = asset(&s);
    assert_eq!(
        (after.f_long_num, after.f_short_num, after.slot_last),
        (before.f_long_num, before.f_short_num, before.slot_last)
    );
}

/// The rate bound is inclusive: |rate| == cap is accepted for both signs, |rate| == cap + 1 is
/// refused, and a refused accrual leaves the asset byte-identical.
#[test]
fn funding_rate_cap_is_inclusive_and_refusal_is_atomic() {
    let (mut s, _long, _short) = book(1_000_000, 10);
    for over in [MAX_RATE as i128 + 1, -(MAX_RATE as i128) - 1, i128::MIN + 1] {
        let before = s.1[0].engine.asset;
        let slot = asset(&s).slot_last + 1;
        assert_eq!(
            try_accrue(&mut s, slot, 1_000_000, over, true),
            Err(V16Error::InvalidConfig),
            "rate {over}"
        );
        assert_eq!(
            s.1[0].engine.asset, before,
            "refused accrual mutated state (rate {over})"
        );
    }
    let start = asset(&s).slot_last;
    accrue(&mut s, start + 1, 1_000_000, MAX_RATE as i128);
    accrue(&mut s, start + 2, 1_000_000, -(MAX_RATE as i128));
    let a = asset(&s);
    // +cap then -cap over one slot each at a constant price cancels exactly.
    assert_eq!((a.f_long_num, a.f_short_num), (0, 0));
}

/// Active funding without protective progress is refused with NonProgress and leaves the asset
/// unchanged (spec: equity-active accrual needs a committed protective step).
#[test]
fn funding_active_without_protective_progress_is_refused_atomically() {
    let (mut s, _long, _short) = book(1_000_000, 10);
    let before = s.1[0].engine.asset;
    assert_eq!(
        try_accrue(&mut s, 4, 1_000_000, 100, false),
        Err(V16Error::NonProgress)
    );
    assert_eq!(s.1[0].engine.asset, before);
}

// ---------------------------------------------------------------------------------------------
// Accrual arithmetic
// ---------------------------------------------------------------------------------------------

/// With A == ADL_ONE the F move is exact and equal-and-opposite: |dF| = rate*dt*price*1e6.
#[test]
fn funding_index_move_is_exact_and_opposite() {
    for &(price, rate, dt) in &[
        (1u64, 1i128, 1u64),
        (1, -1, 1),
        (3_086, 111, 13),
        (1_000_000, -(MAX_RATE as i128), MAX_DT),
        (percolator::MAX_ORACLE_PRICE, MAX_RATE as i128, MAX_DT),
    ] {
        let (mut s, _l, _s) = book(price, 1);
        let start = asset(&s).slot_last;
        accrue(&mut s, start + dt, price, rate);
        let a = asset(&s);
        let want = exact_f(rate, dt, price);
        let (want_long, want_short) = if rate > 0 {
            (-want, want)
        } else {
            (want, -want)
        };
        assert_eq!(
            (a.f_long_num, a.f_short_num),
            (want_long, want_short),
            "price {price} rate {rate} dt {dt}"
        );
    }
}

/// A gap longer than `max_accrual_dt_slots` is charged one capped segment per call: the clock
/// moves by exactly the cap and F by exactly one cap's worth, never the whole gap at once.
/// Spec §5.3 text says to *require* `dt <= cfg_max_accrual_dt_slots`; the engine (like
/// upstream) clamps to one segment instead, which is the lenient reading pinned here.
#[test]
fn funding_long_gap_is_charged_one_capped_segment_per_call() {
    let price = 1_000_000;
    let (mut s, _l, _s) = book(price, 10);
    let start = asset(&s).slot_last;
    accrue(&mut s, start + 10 * MAX_DT, price, 300);
    let a = asset(&s);
    assert_eq!(a.slot_last, start + MAX_DT);
    assert_eq!(a.f_long_num, -exact_f(300, MAX_DT, price));
    // Catching up the rest takes nine more calls and lands on the same F as per-segment calls.
    for _ in 0..9 {
        accrue(&mut s, start + 10 * MAX_DT, price, 300);
    }
    let a = asset(&s);
    assert_eq!(a.slot_last, start + 10 * MAX_DT);
    assert_eq!(a.f_long_num, -exact_f(300, 10 * MAX_DT, price));
    assert_eq!(a.f_short_num, exact_f(300, 10 * MAX_DT, price));
}

/// Accrual is cadence-invariant at a constant price: one k-slot segment and k one-slot
/// segments give the same F (no partitioning gain for a keeper choosing when to crank).
#[test]
fn funding_split_accrual_equals_single_segment() {
    for &(price, rate) in &[(7u64, 3i128), (3_086, -111), (1_000_000, 999)] {
        let (mut one, _a, _b) = book(price, 5);
        let (mut many, _c, _d) = book(price, 5);
        let start = asset(&one).slot_last;
        accrue(&mut one, start + 37, price, rate);
        for k in 1..=37 {
            accrue(&mut many, start + k, price, rate);
        }
        let (x, y) = (asset(&one), asset(&many));
        assert_eq!(
            (x.f_long_num, x.f_short_num),
            (y.f_long_num, y.f_short_num),
            "price {price} rate {rate}"
        );
    }
}

/// With a shrunken A on the paying side, the payer's index moves by ceil(n*A/1e9) and the
/// receiver's by floor(n*A/1e9): rounding never creates value (payer >= receiver per unit of A).
#[test]
fn funding_asymmetric_a_rounds_payer_up_receiver_down() {
    let price = 3_086u64;
    for &a_long in &[MIN_A_SIDE, ADL_ONE / 3, ADL_ONE - 1] {
        for &rate in &[111i128, -111] {
            let (mut s, _l, _s) = book(price, 10);
            let mut a = asset(&s);
            a.a_long = a_long;
            s.1[0].engine.asset = AssetStateV16Account::from_runtime(&a);
            let start = a.slot_last;
            accrue(&mut s, start + 3, price, rate);
            let after = asset(&s);
            let n = rate.unsigned_abs() * 3 * price as u128;
            let floor_long = n * a_long / FUNDING_DEN;
            let ceil_long = (n * a_long).div_ceil(FUNDING_DEN);
            let exact_short = n * (ADL_ONE / FUNDING_DEN);
            if rate > 0 {
                // longs pay: ceil on the long side, short (A == ADL_ONE) exact.
                assert_eq!(after.f_long_num, -(ceil_long as i128), "a_long {a_long}");
                assert_eq!(after.f_short_num, exact_short as i128);
            } else {
                // shorts pay: long side receives floor.
                assert_eq!(after.f_long_num, floor_long as i128, "a_long {a_long}");
                assert_eq!(after.f_short_num, -(exact_short as i128));
            }
            // Per unit of effective position (|F| / A) the payer never pays less than the
            // receiver gets; compared exactly by cross-multiplying.
            let long_scaled = after.f_long_num.unsigned_abs() * ADL_ONE;
            let short_scaled = after.f_short_num.unsigned_abs() * a_long;
            if rate > 0 {
                assert!(long_scaled >= short_scaled, "payer (long) below receiver");
            } else {
                assert!(short_scaled >= long_scaled, "payer (short) below receiver");
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Settlement and `record_account_funding_flow`
// ---------------------------------------------------------------------------------------------

/// Positive rate: the long's paid counter equals its equity loss and the short's received
/// counter equals its gain; the other two counters stay zero. Negative rate mirrors exactly.
#[test]
fn funding_flow_counters_match_settled_equity_both_signs() {
    let price = 1_000_000;
    for &rate in &[250i128, -250] {
        let (mut s, mut long, mut short) = book(price, 7);
        let (l0, s0) = (equity(&long), equity(&short));
        let start = asset(&s).slot_last;
        for k in 1..=20 {
            accrue(&mut s, start + k, price, rate);
        }
        refresh(&mut s, &mut long);
        refresh(&mut s, &mut short);
        let (dl, ds) = (equity(&long) - l0, equity(&short) - s0);
        // 7 units * 250e-9 * 1e6 * 20 slots = 35 atoms each way.
        let want = 35i128;
        if rate > 0 {
            assert_eq!((dl, ds), (-want, want));
            assert_eq!(flows(&long), (want as u128, 0, 0, 0));
            assert_eq!(flows(&short), (0, 0, 0, want as u128));
        } else {
            assert_eq!((dl, ds), (want, -want));
            assert_eq!(flows(&long), (0, want as u128, 0, 0));
            assert_eq!(flows(&short), (0, 0, want as u128, 0));
        }
        validate(&mut s, &mut [&mut long, &mut short]);
    }
}

/// Settling every slot and settling once at the end give the same equity and the same
/// counters, even when one slot of funding is a fraction of an atom (the F remainder carries).
#[test]
fn funding_settlement_cadence_invariant_with_sub_atom_slots() {
    // 1 unit * 3e-9 * 77 price = 2.31e-7 atoms per slot. The long's first per-slot settle
    // floors to -1 and the Euclidean remainder then absorbs later slots; without the carry
    // the per-slot path would charge 49 atoms instead of 1.
    let (price, rate, units, slots) = (77u64, 3i128, 1u128, 49u64);
    let (mut a, mut al, mut as_) = book(price, units);
    let (mut b, mut bl, mut bs) = book(price, units);
    let start = asset(&a).slot_last;
    for k in 1..=slots {
        accrue(&mut a, start + k, price, rate);
        accrue(&mut b, start + k, price, rate);
        refresh(&mut b, &mut bl);
        refresh(&mut b, &mut bs);
    }
    refresh(&mut a, &mut al);
    refresh(&mut a, &mut as_);
    assert_eq!(equity(&al), equity(&bl));
    assert_eq!(equity(&as_), equity(&bs));
    assert_eq!(flows(&al), flows(&bl));
    assert_eq!(flows(&as_), flows(&bs));
    // And with a larger position, where per-slot amounts are fractional but the total is not.
    let (price, rate, units, slots) = (3_086u64, 111i128, 1_000u128, 40u64);
    let (mut a, mut al, _as) = book(price, units);
    let (mut b, mut bl, _bs) = book(price, units);
    let start = asset(&a).slot_last;
    for k in 1..=slots {
        accrue(&mut a, start + k, price, rate);
        accrue(&mut b, start + k, price, rate);
        refresh(&mut b, &mut bl);
    }
    refresh(&mut a, &mut al);
    // intended: 1000 * 111e-9 * 3086 * 40 = 13.70 atoms -> long pays ceil-ish 14 (floor of -13.7)
    assert_eq!(equity(&al), equity(&bl));
    assert_eq!(flows(&al), flows(&bl));
    assert_eq!(flows(&al).0, 14);
}

/// A second refresh with no new accrual settles nothing: no equity change, no counter change.
#[test]
fn funding_refresh_is_idempotent() {
    let (mut s, mut long, mut short) = book(1_000_000, 3);
    let start = asset(&s).slot_last;
    accrue(&mut s, start + 9, 1_000_000, 400);
    refresh(&mut s, &mut long);
    refresh(&mut s, &mut short);
    let snap = (equity(&long), equity(&short), flows(&long), flows(&short));
    refresh(&mut s, &mut long);
    refresh(&mut s, &mut short);
    assert_eq!(
        snap,
        (equity(&long), equity(&short), flows(&long), flows(&short))
    );
}

/// A leg opened after funding has accrued does not pay the funding that came before it.
#[test]
fn funding_late_entrant_pays_only_from_entry() {
    let price = 1_000_000;
    let (mut s, mut long, mut short) = book(price, 4);
    let start = asset(&s).slot_last;
    for k in 1..=10 {
        accrue(&mut s, start + k, price, 500);
    }
    // settle the existing book first so its counters are final before the late trade
    refresh(&mut s, &mut long);
    refresh(&mut s, &mut short);
    let mut late_long = funded(&mut s, 3);
    let mut late_short = funded(&mut s, 4);
    trade(
        &mut s,
        &mut late_long,
        &mut late_short,
        (2 * POS_SCALE) as i128,
        price,
    );
    let e0 = equity(&late_long);
    for k in 11..=15 {
        accrue(&mut s, start + k, price, 500);
    }
    refresh(&mut s, &mut late_long);
    // 2 units * 500e-9 * 1e6 * 5 slots = 5 atoms
    assert_eq!(e0 - equity(&late_long), 5);
    assert_eq!(flows(&late_long), (5, 0, 0, 0));
    // the original long paid 4 * 500e-9 * 1e6 * 10 = 20 then 4 * 0.5 * 5 = 10
    refresh(&mut s, &mut long);
    assert_eq!(flows(&long), (30, 0, 0, 0));
}

/// Counters are booked per side HELD: a long that flips short keeps its long-paid history and
/// then accrues short-received under the same positive rate. No cross-side bleed.
#[test]
fn funding_flow_counters_follow_side_across_flip() {
    let price = 1_000_000;
    let (mut s, mut a, mut b) = book(price, 5);
    let start = asset(&s).slot_last;
    for k in 1..=4 {
        accrue(&mut s, start + k, price, 100);
    }
    // a: long 5 -> short 5 (sells 10 to b)
    refresh(&mut s, &mut a);
    refresh(&mut s, &mut b);
    trade(&mut s, &mut b, &mut a, (10 * POS_SCALE) as i128, price);
    for k in 5..=8 {
        accrue(&mut s, start + k, price, 100);
    }
    refresh(&mut s, &mut a);
    refresh(&mut s, &mut b);
    // each phase: 5 units * 100e-9 * 1e6 * 4 = 2 atoms
    // a: paid 2 as a long, then received 2 as a short.
    assert_eq!(flows(&a), (2, 0, 0, 2));
    // b: received 2 as a short, then paid 2 as a long.
    assert_eq!(flows(&b), (2, 0, 0, 2));
    validate(&mut s, &mut [&mut a, &mut b]);
}

/// The counters record the FUNDING leg of a settlement only, gross of any mark-to-market gain:
/// a long whose price gain exceeds its funding still books the funding as paid.
#[test]
fn funding_counter_is_gross_of_mark_to_market() {
    let price = 1_000_000;
    let (mut s, mut long, mut short) = book(price, 10);
    let start = asset(&s).slot_last;
    let e0 = equity(&long);
    // price +0.1% per slot (10 bps cap) for 5 slots, funding 200e-9 per slot
    let mut px = price;
    for k in 1..=5 {
        px += px / 1_000;
        accrue(&mut s, start + k, px, 200);
    }
    refresh(&mut s, &mut long);
    refresh(&mut s, &mut short);
    assert!(equity(&long) > e0, "long should be up on the price move");
    let (paid, recv, sp, sr) = flows(&long);
    assert!(paid > 0 && recv == 0 && sp == 0 && sr == 0);
    // sum 10 * 200e-9 * px_k over the new segment prices. Pricing at the previous price
    // (spec text's fund_px_last) gives the same rounded totals here, so this test does not
    // pin either price choice.
    let mut want = 0u128;
    let mut p = price;
    let mut num = 0u128;
    for _ in 0..5 {
        p += p / 1_000;
        num += 10 * 200 * p as u128;
    }
    want += num / FUNDING_DEN + u128::from(num % FUNDING_DEN != 0);
    assert_eq!(paid, want);
    assert_eq!(flows(&short).3, num / FUNDING_DEN);
}

// ---------------------------------------------------------------------------------------------
// Epoch change
// ---------------------------------------------------------------------------------------------

/// Epoch change through the engine's own full-drain reset. Two longs share one short. Once the
/// long side's effective OI is exhausted (what a socializing ADL leaves behind; set directly
/// here, the only hand-made step), the first long's refresh runs `begin_full_drain_reset`: the
/// live F moves to the epoch-start slot and the side epoch bumps. Each old-epoch long then
/// settles funding against the epoch-start F, exactly what accrued before the reset, and
/// nothing that the new epoch's F carries.
#[test]
fn funding_stale_epoch_leg_settles_against_epoch_start() {
    let price = 1_000_000;
    let mut s = market(price);
    let mut l1 = funded(&mut s, 1);
    let mut l2 = funded(&mut s, 2);
    let mut short = funded(&mut s, 3);
    trade(&mut s, &mut l1, &mut short, (6 * POS_SCALE) as i128, price);
    trade(&mut s, &mut l2, &mut short, (4 * POS_SCALE) as i128, price);
    let start = asset(&s).slot_last;
    for k in 1..=10 {
        accrue(&mut s, start + k, price, 300);
    }
    let before = asset(&s);
    let f_at_reset = before.f_long_num;
    assert_eq!(f_at_reset, -exact_f(300, 10, price));
    let mut a = before;
    a.oi_eff_long_q = 0;
    s.1[0].engine.asset = AssetStateV16Account::from_runtime(&a);

    refresh(&mut s, &mut l1);
    let reset = asset(&s);
    assert_eq!(reset.mode_long, SideModeV16::ResetPending);
    assert_eq!(reset.epoch_long, before.epoch_long + 1);
    assert_eq!(reset.f_epoch_start_long_num, f_at_reset);
    assert_eq!(reset.f_long_num, 0);
    // 6 units * 300e-9 * 1e6 * 10 = 18 atoms.
    assert_eq!(flows(&l1), (18, 0, 0, 0));

    // Negative-control probe: plant a live F the engine cannot reach (a ResetPending side has
    // no effective OI, so its F does not move) worth thousands of atoms. An old-epoch leg must
    // ignore it. The unplanted live F (0) would already catch a live-F read; the plant also
    // catches a bug that adds live F on top of the epoch-start F.
    let mut a = asset(&s);
    a.f_long_num = -1_000 * exact_f(300, 10, price);
    s.1[0].engine.asset = AssetStateV16Account::from_runtime(&a);
    let e0 = equity(&l2);
    refresh(&mut s, &mut l2);
    // 4 units * 300e-9 * 1e6 * 10 = 12 atoms.
    assert_eq!(flows(&l2), (12, 0, 0, 0));
    assert_eq!(e0 - equity(&l2), 12);
    // Restore the reachable live F (0) and check the engine's own invariants still hold.
    let mut a = asset(&s);
    a.f_long_num = 0;
    s.1[0].engine.asset = AssetStateV16Account::from_runtime(&a);
    validate(&mut s, &mut [&mut l1, &mut l2, &mut short]);
}

// ---------------------------------------------------------------------------------------------
// Path accrual and overflow
// ---------------------------------------------------------------------------------------------

/// The canonical multi-step path (one slot per step, per-step rates) lands on the same F as the
/// same rates fed one slot at a time through the single-segment accrual, including sign changes.
#[test]
fn funding_path_accrual_matches_per_slot_accrual() {
    let price = 3_086u64;
    let rates: Vec<i128> = (0..20)
        .map(|k: i128| {
            if k % 3 == 0 {
                -(k * 37 % 1_000)
            } else {
                k * 53 % 1_000
            }
        })
        .collect();
    let (mut path, _a, _b) = book(price, 9);
    let (mut single, _c, _d) = book(price, 9);
    let start = asset(&path).slot_last;
    let steps: Vec<percolator::AccrualStepV16> = rates
        .iter()
        .map(|&r| percolator::AccrualStepV16 {
            effective_price: price,
            funding_rate_e9: r,
            price_move_remainder_before_bps_num: 0,
            price_move_remainder_after_bps_num: 0,
        })
        .collect();
    {
        let mut m = MarketGroupV16ViewMut::new(&mut path.0, &mut path.1);
        m.accrue_asset_path_to_not_atomic(0, start + rates.len() as u64, price, &steps, true)
            .unwrap();
    }
    for (k, &r) in rates.iter().enumerate() {
        accrue(&mut single, start + 1 + k as u64, price, r);
    }
    let (p, s) = (asset(&path), asset(&single));
    assert_eq!((p.f_long_num, p.f_short_num), (s.f_long_num, s.f_short_num));
    assert_eq!(p.slot_last, s.slot_last);
    let net: i128 = rates.iter().sum();
    assert_eq!(p.f_long_num, -net * 3_086 * (ADL_ONE / FUNDING_DEN) as i128);
}

/// F overflow fails conservatively (spec §5.3 "Persistent K/F overflow fails conservatively"):
/// an accrual that would push F past the i128 range is refused and leaves the asset untouched.
#[test]
fn funding_index_overflow_is_refused_atomically() {
    let price = 1_000_000;
    for &rate in &[MAX_RATE as i128, -(MAX_RATE as i128)] {
        let (mut s, _l, _s) = book(price, 1);
        let mut a = asset(&s);
        // One slot at the cap moves |F| by 1_000 * 1e6 * 1e6 = 1e15; park the payer side just
        // inside the bound so the next move crosses it.
        let near = i128::MIN + 1 + 10;
        if rate > 0 {
            a.f_long_num = near;
        } else {
            a.f_short_num = near;
        }
        s.1[0].engine.asset = AssetStateV16Account::from_runtime(&a);
        let before = s.1[0].engine.asset;
        let slot = a.slot_last + 1;
        assert_eq!(
            try_accrue(&mut s, slot, price, rate, true),
            Err(V16Error::ArithmeticOverflow),
            "rate {rate}: overflowing accrual must be refused by the F overflow guard"
        );
        assert_eq!(
            s.1[0].engine.asset, before,
            "rate {rate}: refused accrual mutated state"
        );
    }
}

/// A flip trade with unsettled funding (no refresh first) must settle the old side's funding
/// under the OLD side before the leg re-attaches on the new side: the pre-flip funding shows up
/// as long-paid / short-received, never on the post-flip side's counters. The flip is
/// allowed without a prior refresh (lenient).
#[test]
fn funding_flip_without_refresh_books_old_side() {
    let price = 1_000_000;
    let (mut s, mut a, mut b) = book(price, 50);
    let start = asset(&s).slot_last;
    for k in 1..=4 {
        accrue(&mut s, start + k, price, 100);
    }
    // a: long 50 -> short 50 (sells 100 to b), no refresh beforehand.
    let mut m = MarketGroupV16ViewMut::new(&mut s.0, &mut s.1);
    let res = m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
        &mut PortfolioV16ViewMut::new(&mut b),
        &mut PortfolioV16ViewMut::new(&mut a),
        TradeRequestV16 {
            asset_index: 0,
            size_q: (100 * POS_SCALE) as i128,
            exec_price: price,
            fee_bps: 0,
        },
        true,
    );
    drop(m);
    // Lenient: the flip is allowed without a prior refresh.
    res.expect("flip with unsettled funding is allowed");
    // 50 units * 100e-9 * 1e6 * 4 = 20 atoms, booked on the side held when it accrued.
    assert_eq!(flows(&a), (20, 0, 0, 0));
    assert_eq!(flows(&b), (0, 0, 0, 20));
    validate(&mut s, &mut [&mut a, &mut b]);
}
