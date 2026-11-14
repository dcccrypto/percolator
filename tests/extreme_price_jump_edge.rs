//! Extreme price jumps between cranks on the v2.2 engine (job P29, tests only).
//!
//! Memecoin-style moves: 100x up, 99% down, several jumps in a row, and a jump right after a
//! stale gap. The engine caps one accrual at `max_price_move_bps_per_slot * dt` once equity is
//! active, so a jump bigger than the cap must be refused (`RecoveryRequired`) and must leave the
//! state untouched; a jump inside the cap (a long enough slot gap, or a wide cap) must settle
//! with value conserved. Skips what oracle_edge / liquidations_edge / adl_insurance_edge hold.
//!
//! Every test name contains `jump` so `cargo test -- jump` selects the area.
#![allow(dead_code)]

use percolator::{
    EngineAssetSlotV16Account, LiquidationRequestV16, Market, MarketGroupV16HeaderAccount,
    MarketGroupV16ViewMut, PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    PermissionlessCrankActionV16, PermissionlessCrankRequestV16, ProvenanceHeaderV16Account, TradeRequestV16, V16Config, V16Error, V16PodU64, POS_SCALE,
};

const P0: u64 = 1_000_000;
const STEP_BPS: u64 = 300;
const MAX_DT: u64 = 2;
const Q: i128 = 10 * POS_SCALE as i128;

struct World {
    header: MarketGroupV16HeaderAccount,
    markets: Vec<Market<u64>>,
    long: PortfolioAccountV16Account,
    short: PortfolioAccountV16Account,
    slot: u64,
}

fn account(seed: u8) -> PortfolioAccountV16Account {
    let h = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        [1; 32], [seed; 32], [3; 32],
    ));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(h).unwrap();
    a
}

/// `move_bps` per slot, `max_dt` slots per accrual. Long and short each deposit `cap` and
/// trade `Q` at P0. Margin 10%.
fn world(move_bps: u64, max_dt: u64, cap: u128) -> World {
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 6_480_000);
    cfg.maintenance_margin_bps = 1_000;
    cfg.initial_margin_bps = 1_000;
    cfg.max_price_move_bps_per_slot = move_bps;
    cfg.max_accrual_dt_slots = max_dt;
    cfg.min_funding_lifetime_slots = max_dt;
    cfg.max_abs_funding_e9_per_slot = 0;
    cfg.liquidation_fee_bps = 0;
    cfg.min_nonzero_mm_req = 1;
    cfg.min_nonzero_im_req = 2;
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut markets = vec![Market::new(0u64, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, P0, 1)
        .unwrap();
    let (mut long, mut short) = (account(10), account(11));
    {
        let mut g = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut lv = PortfolioV16ViewMut::new(&mut long);
        let mut sv = PortfolioV16ViewMut::new(&mut short);
        g.deposit_not_atomic(&mut lv, cap).unwrap();
        g.deposit_not_atomic(&mut sv, cap).unwrap();
        g.execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut lv,
            &mut sv,
            TradeRequestV16 { asset_index: 0, size_q: Q, exec_price: P0, fee_bps: 0 },
            true,
        )
        .unwrap();
    }
    let slot = markets[0].engine.asset.slot_last.get();
    World { header, markets, long, short, slot }
}

impl World {
    fn price(&self) -> u64 {
        self.markets[0].engine.asset.effective_price.get()
    }
    /// accrue to `price` after `dt` more slots
    fn try_jump(&mut self, dt: u64, price: u64) -> Result<(), V16Error> {
        let slot = self.slot + dt;
        let mut g = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        g.accrue_asset_to_not_atomic(0, slot, price, 0, true)?;
        g.markets[0].engine.asset.raw_oracle_target_price = V16PodU64::new(price);
        self.slot = g.markets[0].engine.asset.slot_last.get();
        Ok(())
    }
    /// walk to `target` at the per-slot cap, one slot per step (a keeper cranking every slot)
    fn walk(&mut self, target: u64) {
        while self.price() != target {
            let p = self.price();
            let step = (p * STEP_BPS / 10_000).max(1);
            let next = if target > p { (p + step).min(target) } else { p.saturating_sub(step).max(target) };
            self.jump(1, next);
        }
    }
    fn jump(&mut self, dt: u64, price: u64) {
        self.try_jump(dt, price).expect("jump accrues");
    }
    fn refresh_both(&mut self) {
        let slot = self.slot;
        let mut g = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let price = g.markets[0].engine.asset.try_to_runtime().unwrap().effective_price;
        for acct in [&mut self.long, &mut self.short] {
            let mut v = PortfolioV16ViewMut::new(acct);
            // after a B booking the plain refresh refuses (BStale); a keeper runs the
            // permissionless Refresh crank until it certifies
            let mut done = false;
            for _ in 0..16 {
                match g.full_account_refresh_not_atomic(&mut v) {
                    Err(V16Error::BStale) => {
                        g.permissionless_crank_not_atomic(
                            &mut v,
                            PermissionlessCrankRequestV16 {
                                now_slot: slot,
                                asset_index: 0,
                                effective_price: price,
                                funding_rate_e9: 0,
                                action: PermissionlessCrankActionV16::Refresh,
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
    fn try_liq_long(&mut self) -> Result<(), V16Error> {
        let mut g = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        g.liquidate_account_not_atomic(
            &mut PortfolioV16ViewMut::new(&mut self.long),
            LiquidationRequestV16 { asset_index: 0 },
        )
        .map(|_| ())
    }
    fn try_liq_short(&mut self) -> Result<(), V16Error> {
        let mut g = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        g.liquidate_account_not_atomic(
            &mut PortfolioV16ViewMut::new(&mut self.short),
            LiquidationRequestV16 { asset_index: 0 },
        )
        .map(|_| ())
    }
    fn deposit_insurance(&mut self, domain: usize, amt: u128) {
        let mut g = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        g.deposit_domain_insurance_not_atomic(domain, amt).unwrap();
    }
    fn vault(&self) -> u128 { self.header.vault.get() }
    fn c_tot(&self) -> u128 { self.header.c_tot.get() }
    fn ins(&self) -> u128 { self.header.insurance.get() }
    fn eq(a: &PortfolioAccountV16Account) -> i128 {
        a.capital.get() as i128 + a.pnl.get()
    }
    /// V >= C_tot + I and the engine's own shape checks
    fn assert_sound(&mut self) {
        assert!(self.vault() >= self.c_tot() + self.ins(), "V < C + I");
        let g = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        g.validate_shape().unwrap();
    }
    /// settled equity of both accounts after refresh
    fn total(&mut self) -> i128 {
        self.refresh_both();
        Self::eq(&self.long) + Self::eq(&self.short)
    }
}

// ---------- the cap is the cap ----------

const BIG: u128 = 1_000_000_000_000;

#[test]
fn jump_100x_in_one_slot_is_refused_and_leaves_state_untouched() {
    let mut w = world(STEP_BPS, MAX_DT, BIG);
    let (slot, price) = (w.slot, w.price());
    let (v, c) = (w.vault(), w.c_tot());
    assert_eq!(w.try_jump(1, P0 * 100), Err(V16Error::RecoveryRequired));
    assert_eq!((w.price(), w.vault(), w.c_tot()), (price, v, c));
    assert_eq!(w.slot, slot);
    w.assert_sound();
}

#[test]
fn jump_99pct_down_in_one_slot_is_refused_and_leaves_state_untouched() {
    let mut w = world(STEP_BPS, MAX_DT, BIG);
    let price = w.price();
    assert_eq!(w.try_jump(1, P0 / 100), Err(V16Error::RecoveryRequired));
    assert_eq!(w.price(), price);
    w.assert_sound();
}

#[test]
fn jump_exactly_at_the_cap_passes_and_one_atom_past_is_refused() {
    // cap = 300 bps * 2 slots = 6% of P0
    let mut w = world(STEP_BPS, MAX_DT, BIG);
    let cap = P0 * STEP_BPS * MAX_DT / 10_000;
    assert_eq!(w.try_jump(MAX_DT, P0 + cap + 1), Err(V16Error::RecoveryRequired));
    w.jump(MAX_DT, P0 + cap);
    assert_eq!(w.price(), P0 + cap);
    w.assert_sound();
}

#[test]
fn jump_exactly_at_the_cap_down_passes_and_one_atom_past_is_refused() {
    let mut w = world(STEP_BPS, MAX_DT, BIG);
    let cap = P0 * STEP_BPS * MAX_DT / 10_000;
    assert_eq!(w.try_jump(MAX_DT, P0 - cap - 1), Err(V16Error::RecoveryRequired));
    w.jump(MAX_DT, P0 - cap);
    assert_eq!(w.price(), P0 - cap);
}

#[test]
fn jump_refused_retry_is_repeatable_and_does_not_creep() {
    let mut w = world(STEP_BPS, MAX_DT, BIG);
    for _ in 0..5 {
        assert_eq!(w.try_jump(1, P0 * 3), Err(V16Error::RecoveryRequired));
    }
    assert_eq!(w.price(), P0);
    w.jump(1, P0 + P0 * STEP_BPS / 10_000);
    w.assert_sound();
}

// ---------- jump after a stale period ----------

#[test]
fn jump_after_a_long_stale_gap_is_bounded_by_dt_not_by_wall_clock() {
    // a 1_000_000-slot gap must not license a 2x jump
    let mut w = world(STEP_BPS, MAX_DT, BIG);
    let r = w.try_jump(1_000_000, P0 * 2);
    assert!(r.is_err(), "a stale gap must not license a 2x jump: {r:?}");
    assert_eq!(w.price(), P0);
    w.assert_sound();
}

#[test]
fn jump_gap_one_past_max_dt_does_not_widen_the_cap() {
    let mut w = world(STEP_BPS, MAX_DT, BIG);
    let cap = P0 * STEP_BPS * MAX_DT / 10_000;
    // dt = MAX_DT + 1: the move allowed is still capped by MAX_DT, or the call is refused
    if w.try_jump(MAX_DT + 1, P0 + cap + P0 * STEP_BPS / 10_000).is_ok() {
        panic!("dt past max_accrual_dt_slots widened the price cap");
    }
    assert_eq!(w.price(), P0);
}

#[test]
fn jump_after_stale_gap_then_normal_step_still_works() {
    let mut w = world(STEP_BPS, MAX_DT, BIG);
    assert!(w.try_jump(500, P0 * 2).is_err());
    // the keeper catches up in capped steps
    w.walk(P0 * 2);
    assert_eq!(w.price(), P0 * 2);
    assert_eq!(w.total(), 2 * BIG as i128);
    w.assert_sound();
}

// ---------- several jumps in a row ----------

#[test]
fn jump_ladder_to_100x_in_capped_steps_conserves_value() {
    let mut w = world(STEP_BPS, MAX_DT, BIG);
    let before = w.total();
    let mut last = P0;
    for target in [P0 * 2, P0 * 10, P0 * 50, P0 * 100] {
        w.walk(target);
        w.assert_sound();
        let t = w.total();
        assert_eq!(t, before, "pair total drifted at price {target}");
        assert!(target > last);
        last = target;
    }
    // long gained, short lost
    assert!(World::eq(&w.long) > BIG as i128);
    assert!(World::eq(&w.short) < BIG as i128);
}

#[test]
fn jump_down_99pct_ladder_conserves_value() {
    let mut w = world(STEP_BPS, MAX_DT, BIG);
    let before = w.total();
    for target in [P0 / 2, P0 / 10, P0 / 50, P0 / 100] {
        w.walk(target);
        assert_eq!(w.total(), before, "pair total drifted at price {target}");
        w.assert_sound();
    }
    assert!(World::eq(&w.long) < BIG as i128);
}

#[test]
fn jump_up_then_down_back_to_start_returns_to_start_equity() {
    let mut w = world(STEP_BPS, MAX_DT, BIG);
    w.walk(P0 * 4);
    w.walk(P0);
    w.refresh_both();
    // round trip: rounding may cost at most a few atoms per leg, and never mints value
    let (l, s) = (World::eq(&w.long), World::eq(&w.short));
    assert!(l + s <= 2 * BIG as i128, "minted value: {l} + {s}");
    assert!((l - BIG as i128).abs() <= 8 && (s - BIG as i128).abs() <= 8, "{l} {s}");
    w.assert_sound();
}

#[test]
fn jump_oscillation_repeated_does_not_leak_or_mint_value() {
    let mut w = world(STEP_BPS, MAX_DT, BIG);
    let before = w.total();
    for _ in 0..10 {
        w.walk(P0 * 2);
        w.walk(P0 / 2);
    }
    let after = w.total();
    assert!(after <= before, "minted: {before} -> {after}");
    assert!(before - after <= 400, "rounding leak too large: {before} -> {after}");
    w.assert_sound();
}

#[test]
fn jump_mixed_sizes_in_a_row_work_but_the_same_total_in_one_call_is_refused() {
    let mut a = world(STEP_BPS, MAX_DT, BIG);
    let mut b = world(STEP_BPS, MAX_DT, BIG);
    let p1 = P0 + P0 * 3 / 100; // one slot, 3%
    let p2 = p1 + p1 * 6 / 100; // two slots, 6%
    a.jump(1, p1);
    a.jump(2, p2);
    assert_eq!(a.price(), p2);
    // the same end price in one 3-slot call is a 9.18% move over a capped 2-slot segment
    assert_eq!(b.try_jump(3, p2), Err(V16Error::RecoveryRequired));
    assert_eq!(b.price(), P0);
    a.assert_sound();
}

// ---------- liquidation / margin after a huge move ----------

#[test]
fn jump_100x_up_bankrupts_short_and_conservation_holds() {
    // short deposits 1.2x initial margin; 100x up is a loss of ~99x notional
    let mut w = world(STEP_BPS, MAX_DT, 1_200_000);
    w.walk(P0 * 100);
    w.assert_sound();
    let res = w.try_liq_short();
    assert!(res.is_ok(), "{res:?}");
    w.assert_sound();
    // the bankrupt short has no capital left; the vault still covers what is owed
    assert_eq!(w.short.capital.get(), 0);
}

#[test]
fn jump_99pct_down_bankrupts_long_and_conservation_holds() {
    let mut w = world(STEP_BPS, MAX_DT, 1_200_000);
    w.walk(P0 / 100);
    w.assert_sound();
    let res = w.try_liq_long();
    assert!(res.is_ok(), "{res:?}");
    w.assert_sound();
    assert_eq!(w.long.capital.get(), 0);
}

#[test]
fn jump_winner_cannot_withdraw_more_than_the_loser_could_fund() {
    // after a 100x run the short is bankrupt; the long's settled equity is capped by what exists
    let mut w = world(STEP_BPS, MAX_DT, 1_200_000);
    w.walk(P0 * 100);
    w.try_liq_short().unwrap();
    w.refresh_both();
    let eq_l = World::eq(&w.long);
    let backing = w.vault() as i128 - w.ins() as i128;
    assert!(eq_l <= backing + 1, "long equity {eq_l} exceeds vault minus insurance {backing}");
}

#[test]
fn jump_liquidating_twice_after_a_crash_does_not_double_charge() {
    let mut w = world(STEP_BPS, MAX_DT, 1_200_000);
    w.walk(P0 / 100);
    w.try_liq_long().unwrap();
    let (v, c, i) = (w.vault(), w.c_tot(), w.ins());
    let _ = w.try_liq_long();
    assert_eq!((w.vault(), w.c_tot(), w.ins()), (v, c, i));
    w.assert_sound();
}

#[test]
fn jump_insurance_spend_never_exceeds_its_balance_or_the_notional_lost() {
    let mut w = world(STEP_BPS, MAX_DT, 1_200_000);
    for d in 0..2 {
        w.deposit_insurance(d, 5_000_000);
    }
    let ins0 = w.ins();
    w.walk(P0 / 100);
    w.try_liq_long().unwrap();
    assert!(w.ins() <= ins0);
    assert!(ins0 - w.ins() <= 10 * P0 as u128, "spent more than the whole notional");
    w.assert_sound();
}

#[test]
fn jump_one_step_flips_a_healthy_account_to_liquidatable() {
    // 10% IM, deposit 1.2x IM = 12% of notional. Two 6% two-slot steps cross maintenance.
    let mut w = world(STEP_BPS, MAX_DT, 1_200_000);
    assert!(w.try_liq_long().is_err(), "healthy at start");
    w.jump(MAX_DT, P0 - P0 * 6 / 100);
    let p = w.price();
    w.jump(MAX_DT, p - p * 6 / 100);
    let r = w.try_liq_long();
    assert!(r.is_ok(), "{r:?}");
    w.assert_sound();
}

#[test]
fn jump_small_move_does_not_liquidate_a_healthy_account() {
    let mut w = world(STEP_BPS, MAX_DT, 2_000_000);
    w.jump(1, P0 - P0 / 100);
    assert!(w.try_liq_long().is_err());
    w.assert_sound();
}

#[test]
fn jump_trade_after_a_huge_move_is_priced_at_the_new_mark() {
    // after a 10x walk, a fresh pair opening at the new mark takes on no instant pnl
    let mut w = world(STEP_BPS, MAX_DT, BIG);
    w.walk(P0 * 10);
    w.refresh_both();
    let (mut a, mut b) = (account(20), account(21));
    let p = w.price();
    {
        let mut g = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
        let mut av = PortfolioV16ViewMut::new(&mut a);
        let mut bv = PortfolioV16ViewMut::new(&mut b);
        g.deposit_not_atomic(&mut av, BIG).unwrap();
        g.deposit_not_atomic(&mut bv, BIG).unwrap();
        g.execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut av, &mut bv,
            TradeRequestV16 { asset_index: 0, size_q: Q, exec_price: p, fee_bps: 0 },
            true,
        ).unwrap();
        g.full_account_refresh_not_atomic(&mut av).unwrap();
        g.full_account_refresh_not_atomic(&mut bv).unwrap();
    }
    assert_eq!(World::eq(&a), BIG as i128);
    assert_eq!(World::eq(&b), BIG as i128);
    w.assert_sound();
}

// ---------- zero / max boundaries ----------

#[test]
fn jump_down_walk_stalls_at_a_tiny_price_because_one_atom_exceeds_the_cap() {
    // observation, not a bug: at 3% per slot the smallest legal move is 1 atom, which is more
    // than 3% once the price is below ~34 atoms. The price then cannot walk further down.
    let mut w = world(STEP_BPS, MAX_DT, BIG);
    loop {
        let p = w.price();
        let step = (p * STEP_BPS / 10_000).max(1);
        match w.try_jump(1, p - step) {
            Ok(()) => {}
            Err(e) => {
                assert_eq!(e, V16Error::RecoveryRequired);
                assert!(p < 40 && p > 1, "stalled at {p}");
                break;
            }
        }
    }
    w.assert_sound();
    assert_eq!(w.total(), 2 * BIG as i128);
}

#[test]
fn jump_to_price_zero_is_rejected() {
    let mut w = world(STEP_BPS, MAX_DT, BIG);
    assert!(w.try_jump(1, 0).is_err());
    assert_eq!(w.price(), P0);
}

#[test]
fn jump_above_max_oracle_price_is_rejected() {
    let mut w = world(STEP_BPS, MAX_DT, BIG);
    assert!(w.try_jump(1, u64::MAX).is_err());
    assert_eq!(w.price(), P0);
    w.assert_sound();
}

#[test]
fn jump_with_no_open_interest_may_move_freely() {
    // with no positions there is no equity at risk, so the cap does not bind
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    cfg.max_price_move_bps_per_slot = 100;
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut markets = vec![Market::new(0u64, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, P0, 1)
        .unwrap();
    let mut g = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let r = g.accrue_asset_to_not_atomic(0, 2, P0 * 50, 0, true);
    assert!(r.is_ok(), "{r:?}");
    assert_eq!(g.markets[0].engine.asset.effective_price.get(), P0 * 50);
}

#[test]
fn jump_same_slot_price_change_is_refused() {
    let mut w = world(STEP_BPS, MAX_DT, BIG);
    assert!(w.try_jump(0, P0 + 1).is_err());
    assert_eq!(w.price(), P0);
}

// ---------- vault and insurance domains after a crash ----------

#[test]
fn jump_liquidation_after_a_crash_never_moves_the_vault() {
    // value only moves between accounts, insurance and the winner; nothing leaves the vault
    let mut w = world(STEP_BPS, MAX_DT, 1_200_000);
    w.walk(P0 / 100);
    let v = w.vault();
    w.try_liq_long().unwrap();
    assert_eq!(w.vault(), v);
    w.refresh_both();
    assert_eq!(w.vault(), v);
    w.assert_sound();
}

#[test]
fn jump_crash_spends_only_the_winners_side_domain() {
    // a long goes bankrupt: domain 1 (winners = shorts) pays; domain 0 must stay whole
    let mut w = world(STEP_BPS, MAX_DT, 1_200_000);
    w.deposit_insurance(0, 3_000_000);
    w.deposit_insurance(1, 3_000_000);
    w.walk(P0 / 100);
    w.try_liq_long().unwrap();
    let d0 = w.header.insurance.get();
    assert!(d0 <= 6_000_000);
    // domain 0 untouched: total spend can come only from domain 1's 3_000_000
    assert!(d0 >= 3_000_000, "spent {} which exceeds domain 1's balance", 6_000_000 - d0);
    w.assert_sound();
}

#[test]
fn jump_100x_up_with_insurance_in_the_short_losers_domain_conserves() {
    let mut w = world(STEP_BPS, MAX_DT, 1_200_000);
    w.deposit_insurance(0, 3_000_000);
    w.deposit_insurance(1, 3_000_000);
    w.walk(P0 * 100);
    w.try_liq_short().unwrap();
    assert!(w.ins() >= 3_000_000, "spent more than domain 0 holds");
    w.assert_sound();
    w.refresh_both();
    w.assert_sound();
}
