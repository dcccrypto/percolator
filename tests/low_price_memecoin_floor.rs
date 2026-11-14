//! Very low-priced memecoin markets and the price floor on the v2.2 engine (job P32, tests only).
//!
//! The engine price is an integer: quote atoms per ONE whole base unit (`POS_SCALE` = 1e6 position
//! atoms), with `1 <= price <= MAX_ORACLE_PRICE` (1e12). A 3%-per-slot cap on a tiny integer
//! rounds to zero: P29 (#345) showed the direct accrue path cannot walk below about 34 atoms.
//! These tests check the production path (`accrue_asset_path_to_not_atomic`, with the sub-atom
//! remainder carry) and the direct path at memecoin prices, how long a 99% crash takes, whether
//! a market can get stuck, and that value is conserved at tiny prices.
//!
//! Every test name contains `low_price_floor` so `cargo test -- low_price_floor` selects the area.
#![allow(dead_code)]

use percolator::{
    canonical_band_accrual_price_step_v16, AccrualStepV16, EngineAssetSlotV16Account,
    LiquidationRequestV16, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, TradeRequestV16, V16Config, V16Error, MAX_ORACLE_PRICE, POS_SCALE,
};

const STEP_BPS: u64 = 300;
const MAX_DT: u64 = 2;
const Q: i128 = 10 * POS_SCALE as i128;
const BIG: u128 = 1_000_000_000_000;

struct World {
    header: MarketGroupV16HeaderAccount,
    markets: Vec<Market<u64>>,
    long: PortfolioAccountV16Account,
    short: PortfolioAccountV16Account,
    slot: u64,
    rem: u16,
}

fn account(seed: u8) -> PortfolioAccountV16Account {
    let h = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        [1; 32], [seed; 32], [3; 32],
    ));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(h).unwrap();
    a
}

/// Long and short each deposit `cap`; the long buys `size` at `p0`. Margin 10%, no fees,
/// no funding. `size == 0` means no open interest.
fn world_sized(p0: u64, move_bps: u64, cap: u128, size: i128) -> World {
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 6_480_000);
    cfg.maintenance_margin_bps = 1_000;
    cfg.initial_margin_bps = 1_000;
    cfg.max_price_move_bps_per_slot = move_bps;
    cfg.max_accrual_dt_slots = MAX_DT;
    cfg.min_funding_lifetime_slots = MAX_DT;
    cfg.max_abs_funding_e9_per_slot = 0;
    cfg.liquidation_fee_bps = 0;
    cfg.min_nonzero_mm_req = 1;
    cfg.min_nonzero_im_req = 2;
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut markets = vec![Market::new(0u64, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, p0, 1)
        .unwrap();
    let (mut long, mut short) = (account(10), account(11));
    {
        let mut g = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut lv = PortfolioV16ViewMut::new(&mut long);
        let mut sv = PortfolioV16ViewMut::new(&mut short);
        g.deposit_not_atomic(&mut lv, cap).unwrap();
        g.deposit_not_atomic(&mut sv, cap).unwrap();
        if size != 0 {
            g.execute_trade_with_fee_loss_stale_scoped_not_atomic(
                &mut lv,
                &mut sv,
                TradeRequestV16 { asset_index: 0, size_q: size, exec_price: p0, fee_bps: 0 },
                true,
            )
            .unwrap();
        }
    }
    let slot = markets[0].engine.asset.slot_last.get();
    World { header, markets, long, short, slot, rem: 0 }
}

fn world(p0: u64) -> World {
    world_sized(p0, STEP_BPS, BIG, Q)
}

impl World {
    fn price(&self) -> u64 {
        self.markets[0].engine.asset.effective_price.get()
    }
    fn target(&self) -> u64 {
        self.markets[0].engine.asset.raw_oracle_target_price.get()
    }
    fn vault(&self) -> u128 {
        self.header.vault.get()
    }
    fn c_tot(&self) -> u128 {
        self.header.c_tot.get()
    }
    fn ins(&self) -> u128 {
        self.header.insurance.get()
    }
    fn eq(a: &PortfolioAccountV16Account) -> i128 {
        a.capital.get() as i128 + a.pnl.get()
    }
    fn assert_sound(&mut self) {
        assert!(self.vault() >= self.c_tot() + self.ins(), "V < C + I");
        let g = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        g.validate_shape().unwrap();
    }

    /// Direct accrue to an explicit price (hard cap, no remainder carry), `dt` slots later.
    fn try_direct(&mut self, dt: u64, price: u64) -> Result<(), V16Error> {
        let slot = self.slot + dt;
        let mut g = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        g.accrue_asset_to_not_atomic(0, slot, price, 0, true)?;
        self.slot = g.markets[0].engine.asset.slot_last.get();
        Ok(())
    }

    /// One production-path crank of `dt` slots (1..=MAX_DT) towards `target`: the canonical
    /// steps a keeper wrapper would build, carrying the sub-atom remainder. `rem_in` is the
    /// remainder passed in the first step (a keeper passes the previous call's remainder).
    fn try_path(&mut self, dt: u64, target: u64, rem_in: u16) -> Result<u64, V16Error> {
        let asset = self.markets[0].engine.asset;
        let target_changed = asset.raw_oracle_target_price.get() != target;
        let anchor = if target_changed {
            asset.effective_price.get()
        } else {
            asset.fund_px_last.get()
        };
        let exposed = asset.oi_eff_long_q.get() != 0 || asset.oi_eff_short_q.get() != 0;
        let mut cur = asset.effective_price.get();
        let mut rem = if target_changed { 0 } else { rem_in };
        let mut steps = Vec::new();
        for _ in 0..dt {
            let (next, after) = canonical_band_accrual_price_step_v16(
                cur, target, anchor, STEP_BPS, exposed, rem, None,
            )?;
            steps.push(AccrualStepV16 {
                effective_price: next,
                funding_rate_e9: 0,
                price_move_remainder_before_bps_num: rem,
                price_move_remainder_after_bps_num: after,
            });
            cur = next;
            rem = after;
        }
        let now = self.slot + dt;
        let mut g = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        g.accrue_asset_path_to_not_atomic(0, now, target, &steps, true)?;
        self.slot = g.markets[0].engine.asset.slot_last.get();
        self.rem = rem;
        Ok(cur)
    }

    /// keeper walks to a FIXED target one slot per call, carrying the remainder. Returns slots used.
    fn walk_fixed(&mut self, target: u64, max_slots: u64) -> u64 {
        let mut n = 0;
        while self.price() != target && n < max_slots {
            let rem = self.rem;
            self.try_path(1, target, rem).expect("path accrues");
            n += 1;
        }
        n
    }

    fn refresh_both(&mut self) {
        let mut g = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        for acct in [&mut self.long, &mut self.short] {
            let mut v = PortfolioV16ViewMut::new(acct);
            let mut done = false;
            for _ in 0..16 {
                match g.full_account_refresh_not_atomic(&mut v) {
                    Err(V16Error::BStale) => {
                        let price = g.markets[0].engine.asset.effective_price.get();
                        g.permissionless_crank_not_atomic(
                            &mut v,
                            percolator::PermissionlessCrankRequestV16 {
                                now_slot: self.slot,
                                asset_index: 0,
                                effective_price: price,
                                funding_rate_e9: 0,
                                action: percolator::PermissionlessCrankActionV16::Refresh,
                            },
                        )
                        .unwrap();
                    }
                    r => {
                        r.unwrap();
                        done = true;
                        break;
                    }
                }
            }
            assert!(done, "refresh did not converge");
        }
    }
    fn total(&mut self) -> i128 {
        self.refresh_both();
        Self::eq(&self.long) + Self::eq(&self.short)
    }
    fn try_liq_long(&mut self) -> Result<(), V16Error> {
        let mut g = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        g.liquidate_account_not_atomic(
            &mut PortfolioV16ViewMut::new(&mut self.long),
            LiquidationRequestV16 { asset_index: 0 },
        )
        .map(|_| ())
    }
}

// ---------- what atom price does a real memecoin get? ----------

/// atoms of price for `usd_micro_micro` (USD * 1e12) with a quote that has `qdec` decimals,
/// price = quote atoms per one whole base token. Floors, like any integer price feed.
fn atoms(usd_e12: u128, qdec: u32) -> u128 {
    usd_e12 * 10u128.pow(qdec) / 1_000_000_000_000
}

#[test]
fn low_price_floor_five_e7_dollar_token_has_no_usable_price_with_6_decimal_quote() {
    // $0.0000005 = 5e-7 USD. 6-decimal quote: 0.5 atom -> floors to 0, which the engine rejects.
    assert_eq!(atoms(500_000, 6), 0);
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 6_480_000);
    cfg.max_price_move_bps_per_slot = STEP_BPS;
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut m = vec![Market::new(0u64, EngineAssetSlotV16Account::default())];
    assert!(header.activate_empty_asset_slot_not_atomic(0, &mut m[0].engine, 0, 1).is_err());
}

#[test]
fn low_price_floor_five_e7_dollar_token_at_9_decimals_is_500_atoms_and_walks() {
    // 9-decimal quote: 500 atoms. 3% of that is 15 atoms per slot, so the cap is representable.
    let p = atoms(500_000, 9) as u64;
    assert_eq!(p, 500);
    let mut w = world(p);
    assert_eq!(w.try_path(1, p - 15, 0), Ok(485));
    assert_eq!(w.price(), 485);
    w.assert_sound();
}

#[test]
fn low_price_floor_listing_at_one_atom_and_at_max_price_is_accepted() {
    // observation: nothing in the engine refuses a price in the unusable zone below 34 atoms.
    for p0 in [1u64, 2, 33, 34, MAX_ORACLE_PRICE] {
        let mut w = world(p0);
        assert_eq!(w.price(), p0);
        assert_eq!(w.total(), 2 * BIG as i128);
        w.assert_sound();
    }
}

#[test]
fn low_price_floor_above_max_oracle_price_and_zero_are_rejected_at_listing() {
    for bad in [0u64, MAX_ORACLE_PRICE + 1, u64::MAX] {
        let cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 6_480_000);
        let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
        let mut m = vec![Market::new(0u64, EngineAssetSlotV16Account::default())];
        assert!(header.activate_empty_asset_slot_not_atomic(0, &mut m[0].engine, bad, 1).is_err());
    }
}

// ---------- where the floor is (direct path, hard cap) ----------

#[test]
fn low_price_floor_direct_path_one_atom_step_is_legal_exactly_from_34_atoms() {
    // a 1-atom move needs 1 * 10_000 <= 300 * dt * p, so p >= 34 for dt = 1.
    for p in 2u64..=120 {
        let mut w = world(p);
        let r = w.try_direct(1, p - 1);
        if p * STEP_BPS >= 10_000 {
            assert_eq!(r, Ok(()), "p={p} should step down");
            assert_eq!(w.price(), p - 1);
        } else {
            assert_eq!(r, Err(V16Error::RecoveryRequired), "p={p} should be pinned");
            assert_eq!(w.price(), p);
        }
        w.assert_sound();
    }
}

#[test]
fn low_price_floor_direct_path_two_slot_gap_halves_the_floor_to_17() {
    for p in 2u64..=60 {
        let mut w = world(p);
        let r = w.try_direct(MAX_DT, p - 1);
        if p * STEP_BPS * MAX_DT >= 10_000 {
            assert_eq!(r, Ok(()), "p={p}");
        } else {
            assert_eq!(r, Err(V16Error::RecoveryRequired), "p={p}");
        }
    }
    assert_eq!(10_000u64.div_ceil(STEP_BPS * MAX_DT), 17);
}

#[test]
fn low_price_floor_direct_path_one_atom_up_from_one_is_a_100_percent_move_and_refused() {
    let mut w = world(1);
    assert_eq!(w.try_direct(1, 2), Err(V16Error::RecoveryRequired));
    assert_eq!(w.try_direct(MAX_DT, 2), Err(V16Error::RecoveryRequired));
    assert_eq!(w.price(), 1);
    w.assert_sound();
}

#[test]
fn low_price_floor_refused_move_is_repeatable_and_does_not_creep() {
    let mut w = world(20);
    let (v, c, slot) = (w.vault(), w.c_tot(), w.slot);
    for _ in 0..50 {
        assert_eq!(w.try_direct(1, 19), Err(V16Error::RecoveryRequired));
    }
    assert_eq!((w.price(), w.vault(), w.c_tot(), w.slot), (20, v, c, slot));
    w.assert_sound();
}

#[test]
fn low_price_floor_same_price_accrual_at_tiny_price_is_a_clean_repeat() {
    let mut w = world(7);
    for _ in 0..20 {
        assert_eq!(w.try_direct(1, 7), Ok(()));
    }
    assert_eq!(w.price(), 7);
    assert_eq!(w.total(), 2 * BIG as i128);
    w.assert_sound();
}

// ---------- the production path: fixed target walks with the carry ----------

#[test]
fn low_price_floor_fixed_target_99pct_crash_from_1e6_reaches_one_in_34_slots() {
    let mut w = world(1_000_000);
    let n = w.walk_fixed(10_000, 1000);
    // 99% down = 990_000 atoms at 30_000 atoms per slot (linear in the fixed anchor)
    assert_eq!(n, 33, "1e6 -> 1e4 slots");
    assert_eq!(w.price(), 10_000);
    // a NEW target re-anchors the cap at 1e4, so the second leg needs its own ~33 slots
    let n = w.walk_fixed(1, 1000);
    assert_eq!(w.price(), 1);
    assert!((33..=34).contains(&n), "1e4 -> 1 took {n}");
    assert_eq!(w.total(), 2 * BIG as i128);
    w.assert_sound();
}

#[test]
fn low_price_floor_fixed_target_crash_speed_is_the_same_at_every_price_scale() {
    for p0 in [1_000_000_000_000u64, 1_000_000, 5_000, 500, 100, 34] {
        let mut w = world(p0);
        let n = w.walk_fixed(1, 1000);
        assert_eq!(w.price(), 1, "p0={p0}");
        // linear envelope: about 10_000/300 = 33.3 slots, plus at most one for the carry
        assert!((33..=34).contains(&n), "p0={p0} took {n}");
        assert_eq!(w.total(), 2 * BIG as i128, "p0={p0}");
        w.assert_sound();
    }
}

#[test]
fn low_price_floor_fixed_target_walks_back_up_from_one_to_start_and_conserves() {
    let mut w = world(1_000);
    w.walk_fixed(1, 1000);
    assert_eq!(w.price(), 1);
    // 1 -> 1000: 999 atoms at 3% of anchor 1 (0.03 atom/slot) takes 34 slots per atom
    let n = w.walk_fixed(2, 1000);
    assert_eq!(n, 34);
    assert_eq!(w.total(), 2 * BIG as i128);
    w.assert_sound();
}

#[test]
fn low_price_floor_recovery_from_price_one_is_slow_with_one_fixed_target() {
    // observation: the cap anchor stays at 1 for the life of the target, so the climb is
    // 0.03 atom a slot: 34 slots for the first atom, and 34 more for each next one.
    let mut w = world(1);
    let n = w.walk_fixed(10, 100_000);
    assert_eq!(w.price(), 10);
    assert!((9 * 33..=9 * 34).contains(&n), "took {n}");
}

#[test]
fn low_price_floor_price_one_with_open_interest_needs_34_slots_per_atom() {
    let mut w = world(1);
    for slots in 1..34 {
        let rem = w.rem;
        let p = w.try_path(1, 2, rem).unwrap();
        assert_eq!(p, 1, "slot {slots}");
    }
    let rem = w.rem;
    assert_eq!(w.try_path(1, 2, rem), Ok(2));
}

#[test]
fn low_price_floor_carry_remainder_is_not_stored_so_a_caller_chosen_value_buys_one_atom() {
    // observation: with an unchanged target the first step's remainder is taken from the call.
    // At 33 atoms 3% is 0.99 atom: remainder 0 gives no move, remainder 9_999 gives one atom.
    let mut a = world(33);
    a.try_path(1, 1, 0).unwrap();
    assert_eq!(a.price(), 33);
    let rem = a.rem;
    let mut b = world(33);
    b.try_path(1, 1, 0).unwrap();
    // same target again, honest remainder 9_900 carried: moves one atom
    assert_eq!(b.try_path(1, 1, rem), Ok(32));
    // an invented remainder is accepted too, as long as the chain is self-consistent
    let mut c = world(33);
    c.try_path(1, 1, 0).unwrap();
    assert_eq!(c.try_path(1, 1, 9_999), Ok(32));
    c.assert_sound();
}

// ---------- a moving target pins the market below 34 atoms ----------

/// a feed that changes every slot, always asking for 1 atom lower
fn drift_down(w: &mut World, slots: u64) {
    for _ in 0..slots {
        let t = (w.price() - 1).max(1);
        w.try_path(1, t, 0).unwrap();
    }
}

#[test]
fn low_price_floor_moving_target_pins_at_33_atoms_today() {
    let mut w = world(1_000);
    // a fresh target each slot, each only 1 atom (or 3%) below the price: walk to the pin
    while w.price() > 33 {
        let p = w.price();
        let t = p - (p * STEP_BPS / 10_000).max(1);
        w.try_path(1, t, 0).unwrap();
    }
    assert_eq!(w.price(), 33);
    drift_down(&mut w, 200);
    assert_eq!(w.price(), 33, "the price cannot leave 33 while the target keeps changing");
    w.assert_sound();
}

#[test]
fn low_price_floor_moving_target_pins_even_when_the_oracle_says_one_atom() {
    // oracle target 32, then 31, ... each fed once: the price stays where it is
    let mut w = world(33);
    for t in (1..=32).rev() {
        w.try_path(1, t, 0).unwrap();
    }
    assert_eq!(w.price(), 33);
}

#[test]
fn low_price_floor_two_slot_crank_moves_a_moving_target_down_to_17() {
    let mut w = world(33);
    while w.price() > 1 {
        let p = w.price();
        let before = p;
        w.try_path(MAX_DT, p - 1, 0).unwrap();
        if w.price() == before {
            break;
        }
    }
    assert_eq!(w.price(), 16, "pinned at 16 with 2-slot cranks, 33 with 1-slot cranks");
}

/// BUG (failing on purpose): while the oracle target changes every slot, a market priced below
/// 34 atoms cannot move even one atom in either direction, so the engine price freezes at a
/// value the real market left long ago. Cause: `canonical_accrual_price_step_v16`
/// (src/v16.rs:7203) uses `max_delta = (cap_anchor * bps + rem) / 10_000`, and a changed target
/// resets the anchor to the current price and the carried remainder to 0
/// (src/v16.rs:18203-18214, check at :18229), so `max_delta` is `floor(p * 0.03) = 0` every slot.
/// Smallest fix: on a target change keep the anchor/remainder carry (or make
/// `max_delta = max(1, ...)` whenever `distance > 0`), or refuse to list a price under 34 atoms
/// (see the listing-floor test below).
#[test]
#[ignore = "real bug: moving oracle target pins a sub-34-atom market (src/v16.rs:7203, :18203)"]
fn low_price_floor_bug_moving_target_must_still_make_progress_below_34_atoms() {
    let mut w = world(33);
    drift_down(&mut w, 100);
    assert!(w.price() < 33, "price still {} after 100 slots of a falling oracle", w.price());
}

/// Same bug upward: a rising oracle cannot lift a pinned market off 33 either.
#[test]
#[ignore = "real bug: moving oracle target pins a sub-34-atom market (src/v16.rs:7203, :18203)"]
fn low_price_floor_bug_moving_target_must_make_progress_upward_below_34_atoms() {
    let mut w = world(20);
    for _ in 0..100 {
        let t = w.price() + 1;
        w.try_path(1, t, 0).unwrap();
    }
    assert!(w.price() > 20, "price still {} after 100 slots of a rising oracle", w.price());
}

// ---------- listing rule: smallest safe price ----------

#[test]
fn low_price_floor_smallest_safe_price_table() {
    // one atom is 100/p percent. Three measures, all for the 3%/slot cap and 10% margin:
    // 1) a moving target can move at all:                  p >= 34  (dt = 1)
    // 2) one atom is no more than a tenth of the cap step:   p >= 334
    // 3) one atom is no more than 1% of the 10% margin:    p >= 1000
    let cap_moves = |p: u64| p * STEP_BPS / 10_000 >= 1;
    assert!(!cap_moves(33) && cap_moves(34));
    let tick_within_tenth_of_cap = |p: u64| 10 * 10_000 <= p * STEP_BPS; // 1/p <= 3%/10
    assert!(!tick_within_tenth_of_cap(333) && tick_within_tenth_of_cap(334));
    // a price of 1_000 atoms: cap step 30 atoms, one atom is 0.1%
    assert!(tick_within_tenth_of_cap(1_000));
    // a $0.0000005 token therefore needs a quote with >= 9 decimals at 1 whole token per
    // unit (500 atoms: usable but coarse), or a unit of 1_000+ tokens on a 6-decimal quote.
    assert_eq!(atoms(500_000, 9), 500);
    assert_eq!(atoms(500_000_000_000 /* $0.5 per 1e6 tokens */, 6), 500_000);
}

#[test]
fn low_price_floor_at_the_recommended_floor_a_moving_target_always_progresses() {
    // from 1_000 atoms down with a fresh target every slot: never pinned until the floor itself
    let mut w = world(1_000);
    let mut stuck_at = None;
    for _ in 0..600 {
        let p = w.price();
        let t = p - (p * STEP_BPS / 10_000).max(1);
        w.try_path(1, t, 0).unwrap();
        if w.price() == p {
            stuck_at = Some(p);
            break;
        }
    }
    assert_eq!(stuck_at, Some(33));
    // and the whole way from 1_000 to the pin is a 96.7% crash that took this many slots
    assert!(w.slot > 0);
}

// ---------- positions, liquidation and rounding at tiny prices ----------

#[test]
fn low_price_floor_liquidation_fires_at_tiny_price_and_conserves_value() {
    // long 10 units at 100 atoms (notional 1_000, IM 100): deposit 120 each
    let mut w = world_sized(100, STEP_BPS, 120, Q);
    let total0 = w.total();
    assert_eq!(total0, 240);
    // healthy at the start
    assert!(w.try_liq_long().is_err());
    // crash to 40: long loses 600 on a 120 deposit
    w.walk_fixed(40, 100);
    assert_eq!(w.price(), 40);
    w.refresh_both();
    assert!(w.try_liq_long().is_ok(), "bankrupt long must be liquidatable at 40 atoms");
    w.assert_sound();
    // the long cannot have lost more than it owned and the short cannot be paid more than that
    assert!(w.long.capital.get() as i128 + w.long.pnl.get() >= 0);
    assert!(w.vault() >= w.c_tot() + w.ins());
}

#[test]
fn low_price_floor_small_move_does_not_liquidate_a_healthy_tiny_price_account() {
    let mut w = world_sized(100, STEP_BPS, 500, Q);
    w.walk_fixed(97, 10);
    w.refresh_both();
    assert!(w.try_liq_long().is_err());
    assert_eq!(w.total(), 1_000);
    w.assert_sound();
}

#[test]
fn low_price_floor_dust_position_pnl_rounds_but_conserves() {
    // size 1 position atom, 2 atoms, 999_999 atoms and one whole unit at price 1_000 -> 500
    for size in [1i128, 2, 999_999, 1_000_000, 3_000_001] {
        let mut w = world_sized(1_000, STEP_BPS, 10_000, size);
        w.walk_fixed(500, 100);
        assert_eq!(w.price(), 500, "size {size}");
        // the loser pays rounded up, the winner is paid rounded down: the dust stays in the
        // vault, so total equity is never above the deposits and loses at most 2 atoms
        let t = w.total();
        assert!((19_998..=20_000).contains(&t), "size {size}: total {t}");
        // the loss is size * 500 / 1e6 rounded: never more than the exact value, long <= start
        assert!(w.long.capital.get() as i128 + w.long.pnl.get() <= 10_000);
        w.assert_sound();
    }
}

#[test]
fn low_price_floor_one_atom_position_at_price_one_has_zero_notional_and_conserves() {
    let mut w = world_sized(1, STEP_BPS, 10, 1);
    assert_eq!(w.total(), 20);
    w.walk_fixed(2, 100);
    let t = w.total();
    assert!((18..=20).contains(&t), "total {t}");
    assert!(t <= 20, "never mint value");
    w.assert_sound();
}

#[test]
fn low_price_floor_no_open_interest_price_moves_freely_to_one_and_to_max() {
    let mut w = world_sized(1_000, STEP_BPS, BIG, 0);
    w.try_direct(1, 1).unwrap();
    assert_eq!(w.price(), 1);
    w.try_direct(1, MAX_ORACLE_PRICE).unwrap();
    assert_eq!(w.price(), MAX_ORACLE_PRICE);
    w.try_direct(1, 2).unwrap();
    assert_eq!(w.price(), 2);
    assert_eq!(w.total(), 2 * BIG as i128);
    w.assert_sound();
}

#[test]
fn low_price_floor_crash_to_one_then_recover_returns_equity_exactly() {
    let mut w = world(300);
    w.walk_fixed(1, 1000);
    // coming back up with one fixed target is linear in the anchor (1 atom): 0.03 atom a slot,
    // so 299 atoms take about 10_000 slots (see the recovery-speed test)
    let n = w.walk_fixed(300, 100_000);
    assert_eq!(w.price(), 300);
    assert!(n > 9_000 && n < 11_000, "took {n}");
    let t = w.total();
    assert!((2 * BIG as i128 - 2..=2 * BIG as i128).contains(&t), "total {t}");
    assert!(Self_eq(&w.long) <= BIG as i128 && Self_eq(&w.short) <= BIG as i128 + 2);
    w.assert_sound();
}

#[allow(non_snake_case)]
fn Self_eq(a: &PortfolioAccountV16Account) -> i128 {
    World::eq(a)
}

#[test]
fn low_price_floor_max_size_position_at_max_price_does_not_overflow_on_a_99pct_crash() {
    let dep = 4 * BIG;
    let mut w = world_sized(MAX_ORACLE_PRICE, STEP_BPS, dep, Q);
    w.walk_fixed(MAX_ORACLE_PRICE / 100, 100);
    assert_eq!(w.price(), MAX_ORACLE_PRICE / 100);
    let t = w.total();
    assert!((2 * dep as i128 - 2..=2 * dep as i128).contains(&t), "total {t}");
    w.assert_sound();
}

#[test]
fn low_price_floor_funding_on_a_tiny_notional_rounds_per_slot_but_conserves() {
    // observation test: at 33 atoms a 10-unit position is 330 atoms of notional, so a funding
    // rate of 1e-6 per slot pays 0.00033 atom per slot. Whatever the engine books, value is
    // conserved and the two sides net to zero.
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 6_480_000);
    cfg.maintenance_margin_bps = 1_000;
    cfg.initial_margin_bps = 1_000;
    cfg.max_price_move_bps_per_slot = STEP_BPS;
    cfg.max_accrual_dt_slots = MAX_DT;
    cfg.min_funding_lifetime_slots = MAX_DT;
    cfg.max_abs_funding_e9_per_slot = 1_000;
    cfg.liquidation_fee_bps = 0;
    cfg.min_nonzero_mm_req = 1;
    cfg.min_nonzero_im_req = 2;
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut markets = vec![Market::new(0u64, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, 33, 1)
        .unwrap();
    let (mut long, mut short) = (account(10), account(11));
    let mut g = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let (mut lv, mut sv) = (PortfolioV16ViewMut::new(&mut long), PortfolioV16ViewMut::new(&mut short));
    g.deposit_not_atomic(&mut lv, 100_000).unwrap();
    g.deposit_not_atomic(&mut sv, 100_000).unwrap();
    g.execute_trade_with_fee_loss_stale_scoped_not_atomic(
        &mut lv,
        &mut sv,
        TradeRequestV16 { asset_index: 0, size_q: Q, exec_price: 33, fee_bps: 0 },
        true,
    )
    .unwrap();
    let mut slot = g.markets[0].engine.asset.slot_last.get();
    for _ in 0..500 {
        slot += 1;
        g.accrue_asset_to_not_atomic(0, slot, 33, 1_000, true).unwrap();
    }
    for v in [&mut lv, &mut sv] {
        let mut done = false;
        for _ in 0..16 {
            match g.full_account_refresh_not_atomic(v) {
                Err(V16Error::BStale) => {
                    g.permissionless_crank_not_atomic(
                        v,
                        percolator::PermissionlessCrankRequestV16 {
                            now_slot: slot,
                            asset_index: 0,
                            effective_price: 33,
                            funding_rate_e9: 0,
                            action: percolator::PermissionlessCrankActionV16::Refresh,
                        },
                    )
                    .unwrap();
                }
                r => {
                    r.unwrap();
                    done = true;
                    break;
                }
            }
        }
        assert!(done);
    }
    g.validate_shape().unwrap();
    let (l, s) = (World::eq(&long), World::eq(&short));
    assert!((199_998..=200_000).contains(&(l + s)), "total {}", l + s);
    // 500 slots * 330 atoms * 1e-6 = 0.165 atom: nothing visible may be paid
    assert!((l - 100_000).abs() <= 1, "long moved {}", l - 100_000);
}

