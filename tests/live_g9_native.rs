//! v22kani LIVE (2026-10-20), audit gap G9: liquidation LIVENESS, native characterisation.
//!
//! Supporting evidence for the Kani harnesses in `src/kani_live_1020.rs` (not a substitute).
//! Every test drives the production zero-copy view through public engine entries only
//! (deposit, trade, accrue, the permissionless crank, `liquidate_account_not_atomic`); no
//! engine field is written except where a test says so explicitly.
//!
//!   S1  sweep: a below-maintenance account (production certificate, `certified_liq_deficit != 0`)
//!       is liquidated by the public auto-crank with progress, repeatedly, until it is at or above
//!       maintenance or flat; an account at or above maintenance is refused (`NonProgress`).
//!   R1  head-of-line replay: the auto-crank always liquidates the FIRST liquidatable leg; when that
//!       leg's asset side carries a pending domain-loss barrier the whole crank is refused
//!       (`LockActive`) although another leg of the same account is liquidatable.
#![allow(clippy::too_many_arguments)]

use percolator::{
    AutoCrankObservationV16, AutoCrankOutcomeV16, AutoCrankPlanV16, AutoCrankWorkV16,
    EngineAssetSlotV16Account, LiquidationRequestV16, Market, MarketGroupV16HeaderAccount,
    MarketGroupV16ViewMut, PermissionlessCrankActionV16, PermissionlessCrankRequestV16,
    PermissionlessProgressOutcomeV16, PortfolioAccountV16Account, PortfolioV16ViewMut,
    ProvenanceHeaderV16, ProvenanceHeaderV16Account, TradeRequestV16, V16Config, V16Error,
    V16PodU64,
};

/// initial price; one UNIT of position is UNIT_Q (notional 10_000 at P0). Moves are 4.9% per slot
/// (inside `max_price_move_bps_per_slot = 500` after integer rounding).
const P0: u64 = 1_000_000;
const UNIT_Q: i128 = 10_000;

fn down(p: u64) -> u64 { p - p * 49 / 1000 }
fn up(p: u64) -> u64 { p + p * 49 / 1000 }

type Header = MarketGroupV16HeaderAccount;
type Markets = Vec<Market<u64>>;

fn account(seed: u8) -> PortfolioAccountV16Account {
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        [1; 32],
        [seed; 32],
        [3; 32],
    ));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(header).unwrap();
    a
}

#[derive(Clone, Copy, Debug)]
struct Cfg {
    n: usize,
    fee_bps: u64,
    min_liq_abs: u128,
    lifetime: u64,
}

fn market(c: Cfg) -> Option<(Header, Markets)> {
    let mut cfg = V16Config::public_user_fund_with_market_slots(c.n as u16, c.n as u32, 0, 10);
    cfg.maintenance_margin_bps = 1_000;
    cfg.initial_margin_bps = 1_000;
    cfg.max_price_move_bps_per_slot = 500;
    cfg.max_abs_funding_e9_per_slot = 0;
    cfg.min_funding_lifetime_slots = 1;
    cfg.liquidation_fee_bps = c.fee_bps;
    cfg.liquidation_fee_cap = 1_000_000;
    cfg.min_liquidation_abs = c.min_liq_abs;
    cfg.min_nonzero_mm_req = 100;
    cfg.min_nonzero_im_req = 101;
    cfg.max_accrual_dt_slots = 1;
    cfg.max_bankrupt_close_lifetime_slots = c.lifetime;
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, c.n as u32, 0).ok()?;
    let mut markets: Markets = (0..c.n)
        .map(|i| Market::new(i as u64, EngineAssetSlotV16Account::default()))
        .collect();
    for (i, m) in markets.iter_mut().enumerate() {
        header
            .activate_empty_asset_slot_not_atomic(i as u32, &mut m.engine, P0, (i + 1) as u64)
            .unwrap();
    }
    {
        // bring every asset to one common slot (empty book: no equity activity)
        let mut g = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let target = c.n as u64 + 1;
        for i in 0..c.n {
            while g.markets[i].engine.asset.slot_last.get() < target {
                let next = g.markets[i].engine.asset.slot_last.get() + 1;
                g.accrue_asset_to_not_atomic(i, next, P0, 0, true).unwrap();
            }
        }
    }
    Some((header, markets))
}

/// Advance every asset one slot to the given prices.
fn tick(h: &mut Header, m: &mut Markets, prices: &[u64]) -> Result<(), V16Error> {
    let slot = h.current_slot.get() + 1;
    for (i, &p) in prices.iter().enumerate() {
        accrue(h, m, i, slot, p)?;
    }
    Ok(())
}

fn deposit(h: &mut Header, m: &mut Markets, a: &mut PortfolioAccountV16Account, amt: u128) {
    let mut g = MarketGroupV16ViewMut::new(h, m);
    let mut v = PortfolioV16ViewMut::new(a);
    g.deposit_not_atomic(&mut v, amt).unwrap();
}

fn trade(
    h: &mut Header,
    m: &mut Markets,
    long: &mut PortfolioAccountV16Account,
    short: &mut PortfolioAccountV16Account,
    asset_index: usize,
    size_q: i128,
    price: u64,
) -> Result<(), V16Error> {
    let mut g = MarketGroupV16ViewMut::new(h, m);
    let mut lv = PortfolioV16ViewMut::new(long);
    let mut sv = PortfolioV16ViewMut::new(short);
    g.execute_trade_with_fee_loss_stale_scoped_not_atomic(
        &mut lv,
        &mut sv,
        TradeRequestV16 { asset_index, size_q, exec_price: price, fee_bps: 0 },
        true,
    )
    .map(|_| ())
}

fn accrue(h: &mut Header, m: &mut Markets, asset_index: usize, slot: u64, price: u64) -> Result<(), V16Error> {
    let mut g = MarketGroupV16ViewMut::new(h, m);
    g.accrue_asset_to_not_atomic(asset_index, slot, price, 0, true)?;
    g.markets[asset_index].engine.asset.raw_oracle_target_price = V16PodU64::new(price);
    Ok(())
}

fn price_of(m: &Markets, i: usize) -> u64 {
    m[i].engine.asset.effective_price.get()
}

/// Public refresh crank on `asset_index`, repeated while the account is b-stale (the keeper loop).
fn refresh(h: &mut Header, m: &mut Markets, a: &mut PortfolioAccountV16Account, asset_index: usize) -> Result<(), V16Error> {
    let now_slot = h.current_slot.get();
    let price = price_of(m, asset_index);
    for _ in 0..64 {
        let mut g = MarketGroupV16ViewMut::new(h, m);
        let mut v = PortfolioV16ViewMut::new(a);
        g.permissionless_crank_not_atomic(
            &mut v,
            PermissionlessCrankRequestV16 {
                now_slot,
                asset_index,
                effective_price: price,
                funding_rate_e9: 0,
                action: PermissionlessCrankActionV16::Refresh,
            },
        )?;
        if a.b_stale_state == 0 && a.stale_state == 0 {
            break;
        }
    }
    Ok(())
}

fn auto_crank(h: &mut Header, m: &mut Markets, a: &mut PortfolioAccountV16Account) -> Result<percolator::AutoCrankResultV16, V16Error> {
    let now_slot = h.current_slot.get();
    let obs: Vec<AutoCrankObservationV16> = (0..m.len())
        .map(|i| AutoCrankObservationV16 { asset_index: i, effective_price: price_of(m, i), funding_rate_e9: 0 })
        .collect();
    let mut g = MarketGroupV16ViewMut::new(h, m);
    let mut v = PortfolioV16ViewMut::new(a);
    g.permissionless_auto_crank_not_atomic(
        &mut v,
        AutoCrankWorkV16 { now_slot, observations: &obs, resolved_close_fee_rate_per_slot: 0 },
    )
}

fn cert(a: &PortfolioAccountV16Account) -> percolator::HealthCertV16 {
    a.health_cert.try_to_runtime().unwrap()
}

fn abs_basis_total(a: &PortfolioAccountV16Account) -> u128 {
    a.legs
        .iter()
        .map(|l| l.try_to_runtime().unwrap())
        .filter(|l| l.active)
        .map(|l| l.basis_pos_q.unsigned_abs())
        .sum()
}

fn active_legs(a: &PortfolioAccountV16Account) -> usize {
    a.legs.iter().filter(|l| l.active != 0).count()
}

fn mode(h: &Header) -> u8 {
    h.mode
}

#[derive(Default, Debug)]
struct Stats {
    scenarios: usize,
    below: usize,
    healthy_refused: usize,
    liq_steps: usize,
    partial_steps: usize,
    full_closes: usize,
    to_healthy: usize,
    to_flat: usize,
    recovery_declared: usize,
    insurance_scenarios: usize,
    bankrupt: usize,
    unexpected: Vec<String>,
}

/// S1: single-asset sweep. Long target vs a deep counterparty; the price falls `drops` x 5%;
/// capital, size, fee policy and insurance vary. Whenever the production certificate says
/// below maintenance, the public auto-crank must select Liquidate and return Ok with progress,
/// and repeated cranks must end at or above maintenance, flat, or in a declared Recovery.
#[test]
fn g9_s1_below_maintenance_is_always_liquidated_by_the_public_crank() {
    let mut st = Stats::default();
    for &(fee_bps, min_abs) in &[(0u64, 0u128), (50, 2), (100, 500)] {
        for &capital in &[1_500u128, 3_000, 6_000, 12_000, 25_000] {
            for &units in &[1u128, 2, 5] {
                for drops in 1u64..=6 {
                    for &short_target in &[false, true] {
                        for &insurance in &[0u128, 1_000_000] {
                            run_s1(Cfg { n: 1, fee_bps, min_liq_abs: min_abs, lifetime: 100 }, capital, units, drops, short_target, insurance, &mut st);
                        }
                    }
                }
            }
        }
    }
    eprintln!("G9-S1 stats: {st:?}");
    assert!(st.unexpected.is_empty(), "unexpected outcomes: {:#?}", st.unexpected);
    // non-vacuity: every interesting branch was reached
    assert!(st.below >= 50, "too few below-maintenance scenarios: {}", st.below);
    assert!(st.partial_steps > 0 && st.full_closes > 0 && st.to_healthy > 0 && st.to_flat > 0);
    assert!(st.healthy_refused > 0);
    assert!(st.insurance_scenarios > 0 && st.bankrupt > 0);
}

fn run_s1(c: Cfg, capital: u128, units: u128, drops: u64, short_target: bool, insurance: u128, st: &mut Stats) {
    let Some((mut h, mut m)) = market(c) else { return };
    st.scenarios += 1;
    let mut t = account(10);
    let mut cp = account(11);
    deposit(&mut h, &mut m, &mut t, capital);
    deposit(&mut h, &mut m, &mut cp, 1_000_000_000);
    if insurance != 0 {
        let mut g = MarketGroupV16ViewMut::new(&mut h, &mut m);
        for d in 0..2 {
            let _ = g.deposit_domain_insurance_not_atomic(d, insurance);
        }
    }
    let q = units as i128 * UNIT_Q; // notional = units * 10_000 at P0
    let opened = if short_target {
        trade(&mut h, &mut m, &mut cp, &mut t, 0, q, P0)
    } else {
        trade(&mut h, &mut m, &mut t, &mut cp, 0, q, P0)
    };
    if opened.is_err() {
        return; // below initial margin at open: not a liquidation scenario
    }
    let mut price = P0;
    for _ in 0..drops {
        price = if short_target { up(price) } else { down(price) };
        tick(&mut h, &mut m, &[price]).unwrap();
    }
    if let Err(e) = refresh(&mut h, &mut m, &mut t, 0) {
        st.unexpected.push(format!("refresh failed {e:?} c={capital} u={units} d={drops} s={short_target}"));
        return;
    }
    let c0 = cert(&t);
    if c0.certified_liq_deficit == 0 {
        // converse: at or above maintenance the liquidation entry refuses with NonProgress
        let before = (t.capital.get(), abs_basis_total(&t));
        let r = {
            let mut g = MarketGroupV16ViewMut::new(&mut h, &mut m);
            let mut v = PortfolioV16ViewMut::new(&mut t);
            g.liquidate_account_not_atomic(&mut v, LiquidationRequestV16 { asset_index: 0 })
        };
        if r != Err(V16Error::NonProgress) {
            st.unexpected.push(format!("healthy account not refused NonProgress: {r:?}"));
        }
        // the engine's refresh inside the call may re-certify, never move value or the position
        if (t.capital.get(), abs_basis_total(&t)) != before {
            st.unexpected.push("healthy refusal moved capital or position".into());
        }
        st.healthy_refused += 1;
        return;
    }
    st.below += 1;
    if c0.certified_equity < 0 {
        st.bankrupt += 1;
    }
    if insurance != 0 {
        st.insurance_scenarios += 1;
    }
    for step in 0..16 {
        let cb = cert(&t);
        if active_legs(&t) == 0 {
            st.to_flat += 1;
            return;
        }
        if cb.certified_liq_deficit == 0 {
            st.to_healthy += 1;
            return;
        }
        let pos_before = abs_basis_total(&t);
        let r = auto_crank(&mut h, &mut m, &mut t);
        match r {
            Ok(res) => {
                match (res.selected, res.outcome) {
                    (AutoCrankPlanV16::Liquidate { .. }, AutoCrankOutcomeV16::Progressed(PermissionlessProgressOutcomeV16::RecoveryDeclared(_))) => {
                        st.recovery_declared += 1;
                        assert_eq!(mode(&h), 2, "RecoveryDeclared must leave the market in Recovery");
                        return;
                    }
                    (AutoCrankPlanV16::Liquidate { .. }, AutoCrankOutcomeV16::Progressed(_)) => {
                        st.liq_steps += 1;
                        let pos_after = abs_basis_total(&t);
                        if pos_after >= pos_before {
                            st.unexpected.push(format!("liquidation without position progress step={step} {pos_before}->{pos_after}"));
                            return;
                        }
                        if pos_after == 0 { st.full_closes += 1 } else { st.partial_steps += 1 }
                    }
                    (AutoCrankPlanV16::AdvanceClose, _) | (AutoCrankPlanV16::RefreshAccount { .. }, _) | (AutoCrankPlanV16::SettleBChunk { .. }, _) => {
                        // bounded housekeeping the selector ranks above Liquidate (pending close / stale / B)
                    }
                    other => {
                        st.unexpected.push(format!("below maintenance but crank chose {other:?} c={capital} u={units} d={drops} s={short_target}"));
                        return;
                    }
                }
            }
            Err(e) => {
                st.unexpected.push(format!("crank refused a below-maintenance account: {e:?} c={capital} u={units} d={drops} s={short_target} ins={insurance} fee={}/{}", c.fee_bps, c.min_liq_abs));
                return;
            }
        }
        if mode(&h) != 0 {
            st.recovery_declared += 1;
            return;
        }
        // the next crank needs a current certificate; the keeper refreshes (no price move here)
        if t.stale_state != 0 || t.b_stale_state != 0 || !cert(&t).valid {
            if active_legs(&t) == 0 {
                st.to_flat += 1;
                return;
            }
            if let Err(e) = refresh(&mut h, &mut m, &mut t, 0) {
                st.unexpected.push(format!("refresh after liquidation failed {e:?}"));
                return;
            }
        }
    }
    st.unexpected.push(format!("no terminal state after 16 cranks c={capital} u={units} d={drops} s={short_target}"));
}

// ---------------------------------------------------------------------------------------------
// R1: head-of-line blocking by a pending domain-loss barrier on the FIRST liquidatable leg.
// ---------------------------------------------------------------------------------------------

struct World2 {
    h: Header,
    m: Markets,
    t: PortfolioAccountV16Account,
    s2: PortfolioAccountV16Account,
}

/// Two assets. Target T: long on asset 0 (leg slot 0) and long on asset 1 (slot 1). A bankrupt
/// short S2 on asset 0 closes its only leg through a terminal trade, which raises the LONG barrier
/// on asset 0 (`begin_terminal_trade_residual_if_needed`, same sequence as f04_regression's
/// `drive_to_the_barrier`). Asset 1 then falls so T is below maintenance.
fn r1_world() -> World2 {
    r1_world_with(5)
}

fn r1_world_with(asset1_drops: usize) -> World2 {
    let c = Cfg { n: 2, fee_bps: 0, min_liq_abs: 0, lifetime: 1_000 };
    let (mut h, mut m) = market(c).expect("config");
    let mut t = account(10);
    let mut cp = account(11);
    let mut s2 = account(12);
    let mut l2 = account(13);
    deposit(&mut h, &mut m, &mut t, 4_000);
    deposit(&mut h, &mut m, &mut cp, 1_000_000_000);
    deposit(&mut h, &mut m, &mut s2, 1_200);
    deposit(&mut h, &mut m, &mut l2, 1_000_000_000);
    let unit = UNIT_Q; // notional 10_000 at P0
    // S2 short 1 unit on asset 0 against L2 (S2 has ~12% margin)
    trade(&mut h, &mut m, &mut l2, &mut s2, 0, unit, P0).unwrap();
    // T long 1 unit on asset 0 (vs CP), then long 2 units on asset 1 (vs CP)
    trade(&mut h, &mut m, &mut t, &mut cp, 0, unit, P0).unwrap();
    trade(&mut h, &mut m, &mut t, &mut cp, 1, 2 * unit, P0).unwrap();
    // asset 0 rallies 4 x 4.9%: S2 is bankrupt (loss ~2_100 > capital 1_200); T's asset-0 leg gains
    let mut p0 = P0;
    for _ in 0..4 {
        p0 = up(p0);
        tick(&mut h, &mut m, &[p0, P0]).unwrap();
    }
    // S2's terminal trade: buys back its short from L2 at the mark -> residual, LONG barrier on asset 0
    trade(&mut h, &mut m, &mut s2, &mut l2, 0, unit, p0).expect("S2 terminal close");
    assert!(s2.close_progress.try_to_runtime().unwrap().residual_remaining > 0, "S2 holds a pending bankrupt close");
    assert_eq!(m[0].engine.pending_domain_loss_barrier_long.get(), 1, "the long barrier on asset 0 is up");
    assert_eq!(m[0].engine.pending_domain_loss_barrier_short.get(), 0);
    // asset 1 falls 5 x 4.9% (T loses ~4_300 on 2 units of 10_000 notional -> below maintenance)
    let mut p1 = P0;
    for _ in 0..asset1_drops {
        p1 = down(p1);
        tick(&mut h, &mut m, &[p0, p1]).unwrap();
    }
    refresh(&mut h, &mut m, &mut t, 0).unwrap();
    World2 { h, m, t, s2 }
}

#[test]
fn g9_r1_barrier_on_the_first_leg_no_longer_blocks_the_crank_when_another_leg_is_liquidatable() {
    let mut w = r1_world();
    let c0 = cert(&w.t);
    eprintln!("R1 cert: equity={} mm={} deficit={}", c0.certified_equity, c0.certified_maintenance_req, c0.certified_liq_deficit);
    assert!(c0.valid && c0.certified_liq_deficit != 0, "T must be below maintenance");
    assert_eq!(w.m[0].engine.pending_domain_loss_barrier_long.get(), 1, "barrier still pending");
    let legs: Vec<_> = w.t.legs.iter().map(|l| l.try_to_runtime().unwrap()).filter(|l| l.active).collect();
    assert_eq!(legs.len(), 2);
    assert_eq!(legs[0].asset_index, 0, "slot 0 holds the asset-0 (barrier) leg");

    // The public crank (the ONLY liquidation entry the wrapper exposes, tag 5) selects
    // Liquidate { asset 0 } and is refused.
    // Liquidate { asset 0 } was refused (LockActive) on 1c96c22f. Since F-LIVE-1 (v22live) the
    // selector prefers the first liquidatable leg NOT behind a barrier: Liquidate { asset 1 }.
    let snapshot = (w.h, w.m.clone(), w.t);
    let r = auto_crank(&mut w.h, &mut w.m, &mut w.t);
    eprintln!("R1 auto-crank: {r:?}");
    assert_eq!(
        r.map(|x| x.selected),
        Ok(AutoCrankPlanV16::Liquidate { asset_index: 1 }),
        "crank liquidates the unblocked leg"
    );
    assert_eq!(w.m[0].engine.pending_domain_loss_barrier_long.get(), 1, "the barrier is untouched");

    // The same account IS liquidatable on its other leg: the engine kernel accepts asset 1.
    let (mut h, mut m, mut t) = snapshot;
    let before = abs_basis_total(&t);
    let r = {
        let mut g = MarketGroupV16ViewMut::new(&mut h, &mut m);
        let mut v = PortfolioV16ViewMut::new(&mut t);
        g.liquidate_account_not_atomic(&mut v, LiquidationRequestV16 { asset_index: 1 })
    };
    eprintln!("R1 direct liquidate asset 1: {r:?}");
    assert!(r.is_ok(), "the asset-1 leg is liquidatable: {r:?}");
    assert!(abs_basis_total(&t) < before);

    // How the block ends: someone must crank the BANKRUPT account S2 (its AdvanceClose books the
    // residual and drops the barrier); only then does T's crank liquidate. If that close cannot
    // progress, the barrier stays until `max_close_slot` and the market goes to Recovery.
    let mut w = r1_world();
    let r = auto_crank(&mut w.h, &mut w.m, &mut w.s2);
    eprintln!("R1 crank S2 (bankrupt close owner): {:?}", r.as_ref().map(|x| x.selected));
    eprintln!("R1 barrier after S2 crank: long={}", w.m[0].engine.pending_domain_loss_barrier_long.get());
    if w.m[0].engine.pending_domain_loss_barrier_long.get() == 0 {
        refresh(&mut w.h, &mut w.m, &mut w.t, 0).unwrap();
        let before = abs_basis_total(&w.t);
        let r = auto_crank(&mut w.h, &mut w.m, &mut w.t);
        eprintln!("R1 crank T after the barrier cleared: {:?}", r.as_ref().map(|x| x.selected));
        assert!(matches!(r, Ok(ref x) if matches!(x.selected, AutoCrankPlanV16::Liquidate { .. })));
        assert!(abs_basis_total(&w.t) < before);
    }
}

/// R1 control: same world without the barrier (S2 solvent): the crank liquidates.
#[test]
fn g9_r1_control_without_barrier_the_crank_liquidates() {
    let c = Cfg { n: 2, fee_bps: 0, min_liq_abs: 0, lifetime: 1_000 };
    let (mut h, mut m) = market(c).expect("config");
    let mut t = account(10);
    let mut cp = account(11);
    deposit(&mut h, &mut m, &mut t, 4_000);
    deposit(&mut h, &mut m, &mut cp, 1_000_000_000);
    let unit = UNIT_Q;
    trade(&mut h, &mut m, &mut t, &mut cp, 0, unit, P0).unwrap();
    trade(&mut h, &mut m, &mut t, &mut cp, 1, 2 * unit, P0).unwrap();
    let mut p0 = P0;
    for _ in 0..4 {
        p0 = up(p0);
        tick(&mut h, &mut m, &[p0, P0]).unwrap();
    }
    let mut p1 = P0;
    for _ in 0..5 {
        p1 = down(p1);
        tick(&mut h, &mut m, &[p0, p1]).unwrap();
    }
    refresh(&mut h, &mut m, &mut t, 0).unwrap();
    assert!(cert(&t).certified_liq_deficit != 0);
    let before = abs_basis_total(&t);
    let r = auto_crank(&mut h, &mut m, &mut t).expect("crank liquidates");
    assert!(matches!(r.selected, AutoCrankPlanV16::Liquidate { asset_index: 0 }));
    assert!(abs_basis_total(&t) < before);
}

// ---------------------------------------------------------------------------------------------
// E7 native twin: the exact fixture of the Kani harness `kani_live_e7_liquidate_progresses_below_
// maintenance` (src/kani_live_1020.rs), with the symbolic deposit / insurance replaced by a grid.
// It validates the fixture (so the single Kani run is not spent on a fixture bug) and prints the
// branch each point takes.
// ---------------------------------------------------------------------------------------------

const E7_INS_DOMAIN: usize = 1;

fn e7_fixture(capital: u128, insurance: u128) -> Option<(Header, Markets, PortfolioAccountV16Account)> {
    let (mut h, mut m) = market(Cfg { n: 1, fee_bps: 50, min_liq_abs: 2, lifetime: 100 }).expect("E7 config");
    let mut t = account(10);
    let mut cp = account(11);
    deposit(&mut h, &mut m, &mut t, capital);
    deposit(&mut h, &mut m, &mut cp, 1_000_000);
    {
        let mut g = MarketGroupV16ViewMut::new(&mut h, &mut m);
        g.deposit_domain_insurance_not_atomic(E7_INS_DOMAIN, insurance).ok()?;
    }
    trade(&mut h, &mut m, &mut t, &mut cp, 0, 2 * UNIT_Q, P0).ok()?;
    let mut p = P0;
    for _ in 0..3 {
        p = down(p);
        tick(&mut h, &mut m, &[p]).unwrap();
    }
    // ONE public refresh crank, as in the Kani harness, must leave the account current
    {
        let now_slot = h.current_slot.get();
        let mut g = MarketGroupV16ViewMut::new(&mut h, &mut m);
        let mut v = PortfolioV16ViewMut::new(&mut t);
        g.permissionless_crank_not_atomic(
            &mut v,
            PermissionlessCrankRequestV16 { now_slot, asset_index: 0, effective_price: p, funding_rate_e9: 0, action: PermissionlessCrankActionV16::Refresh },
        )
        .unwrap();
    }
    assert!(cert(&t).valid && t.stale_state == 0 && t.b_stale_state == 0, "one refresh must suffice");
    Some((h, m, t))
}

#[test]
fn g9_e7_native_twin_of_the_kani_fixture() {
    let mut seen = std::collections::BTreeSet::new();
    for &capital in &[1_000u128, 2_000, 2_001, 2_500, 2_798, 2_799, 2_800, 3_000, 3_500, 4_000, 4_400, 4_519, 4_520, 4_600, 6_000] {
        for &ins in &[0u128, 300, 5_000] {
            let Some((mut h, mut m, mut t)) = e7_fixture(capital, ins) else {
                seen.insert("open refused (below IM)");
                continue;
            };
            let c0 = cert(&t);
            let pos0 = abs_basis_total(&t);
            let r = {
                let mut g = MarketGroupV16ViewMut::new(&mut h, &mut m);
                let mut v = PortfolioV16ViewMut::new(&mut t);
                g.liquidate_account_not_atomic(&mut v, LiquidationRequestV16 { asset_index: 0 })
            };
            let c1 = cert(&t);
            eprintln!(
                "E7 cap={capital} ins={ins}: eq0={} mm0={} def0={} -> {r:?} legs={} def1={} valid1={}",
                c0.certified_equity, c0.certified_maintenance_req, c0.certified_liq_deficit,
                active_legs(&t), c1.certified_liq_deficit, c1.valid
            );
            if c0.certified_liq_deficit == 0 {
                assert_eq!(r, Err(V16Error::NonProgress));
                seen.insert("healthy refused");
                continue;
            }
            let out = r.expect("below maintenance must liquidate");
            assert!(out.closed_q >= 1 && abs_basis_total(&t) < pos0);
            assert!(active_legs(&t) == 0 || c1.certified_liq_deficit == 0);
            if active_legs(&t) != 0 { seen.insert("partial"); } else { seen.insert("full"); }
            if c0.certified_equity < 0 { seen.insert("bankrupt"); }
            if out.insurance_used > 0 { seen.insert("insurance"); }
            if out.residual_booked + out.explicit_loss > 0 { seen.insert("residual"); }
        }
    }
    eprintln!("E7 branches: {seen:?}");
    for b in ["healthy refused", "partial", "full", "bankrupt", "insurance", "residual"] {
        assert!(seen.contains(b), "fixture never reaches {b}");
    }
}

// ---------------------------------------------------------------------------------------------
// RED replays (assert the LIVENESS property; FAIL on the head 1c96c22f). `#[ignore]` keeps the
// suite green; run with `cargo test --release --test live_g9_native -- --ignored`.
// ---------------------------------------------------------------------------------------------

/// F-LIVE-1: a below-maintenance account with a liquidatable leg must be liquidated by the public
/// crank. On the head the crank picks the FIRST flagged leg (asset 0, behind a domain-loss barrier)
/// and returns LockActive although the asset-1 leg is liquidatable.
#[test]
fn g9_f_live_1_red_crank_liquidates_a_below_maintenance_account_with_a_liquidatable_leg() {
    let mut w = r1_world();
    assert!(cert(&w.t).certified_liq_deficit != 0, "precondition: below maintenance");
    let before = abs_basis_total(&w.t);
    let r = auto_crank(&mut w.h, &mut w.m, &mut w.t);
    assert!(
        matches!(r, Ok(ref x) if matches!(x.selected, AutoCrankPlanV16::Liquidate { .. })),
        "public crank did not liquidate a below-maintenance account: {r:?}"
    );
    assert!(abs_basis_total(&w.t) < before);
}

// ---------------------------------------------------------------------------------------------
// LIVEFIX (v22live, 2026-10-20): independent F-LIVE-1 repros, public entries only.
// R2 reaches the state differently from R1: three assets, the barrier is on the SHORT side
// (raised by a bankrupt LONG's terminal sale on a falling asset 0), the target T is short asset 0
// (its winning leg, slot 0) and long assets 1 and 2 (losing, slots 1 and 2).
// ---------------------------------------------------------------------------------------------

struct World3 {
    h: Header,
    m: Markets,
    t: PortfolioAccountV16Account,
    l2: PortfolioAccountV16Account,
}

fn r2_world() -> World3 {
    let c = Cfg { n: 3, fee_bps: 0, min_liq_abs: 0, lifetime: 1_000 };
    let (mut h, mut m) = market(c).expect("config");
    let mut t = account(20);
    let mut cp = account(21);
    let mut l2 = account(22);
    let mut ss = account(23);
    deposit(&mut h, &mut m, &mut t, 6_000);
    deposit(&mut h, &mut m, &mut cp, 1_000_000_000);
    deposit(&mut h, &mut m, &mut l2, 1_200);
    deposit(&mut h, &mut m, &mut ss, 1_000_000_000);
    let unit = UNIT_Q;
    // L2 long 1 unit on asset 0 against SS
    trade(&mut h, &mut m, &mut l2, &mut ss, 0, unit, P0).unwrap();
    // T short 1 unit on asset 0 (vs CP), long 1 unit on asset 1 and 2 units on asset 2 (vs CP)
    trade(&mut h, &mut m, &mut cp, &mut t, 0, unit, P0).unwrap();
    trade(&mut h, &mut m, &mut t, &mut cp, 1, unit, P0).unwrap();
    trade(&mut h, &mut m, &mut t, &mut cp, 2, 2 * unit, P0).unwrap();
    // asset 0 falls 4 x 4.9%: L2 (long, capital 1_200) is bankrupt; T's short gains
    let mut p0 = P0;
    for _ in 0..4 {
        p0 = down(p0);
        tick(&mut h, &mut m, &[p0, P0, P0]).unwrap();
    }
    // L2's terminal trade: sells its long back to SS at the mark -> residual, SHORT barrier on asset 0
    trade(&mut h, &mut m, &mut ss, &mut l2, 0, unit, p0).expect("L2 terminal close");
    assert!(l2.close_progress.try_to_runtime().unwrap().residual_remaining > 0, "L2 holds a pending bankrupt close");
    assert_eq!(m[0].engine.pending_domain_loss_barrier_short.get(), 1, "the short barrier on asset 0 is up");
    assert_eq!(m[0].engine.pending_domain_loss_barrier_long.get(), 0);
    // assets 1 and 2 fall 5 x 4.9%: T is below maintenance
    let (mut p1, mut p2) = (P0, P0);
    for _ in 0..5 {
        p1 = down(p1);
        p2 = down(p2);
        tick(&mut h, &mut m, &[p0, p1, p2]).unwrap();
    }
    refresh(&mut h, &mut m, &mut t, 0).unwrap();
    World3 { h, m, t, l2 }
}

fn leg_basis_for_asset(a: &PortfolioAccountV16Account, asset: u32) -> i128 {
    a.legs
        .iter()
        .map(|l| l.try_to_runtime().unwrap())
        .filter(|l| l.active && l.asset_index == asset)
        .map(|l| l.basis_pos_q)
        .sum()
}

/// R2 (F-LIVE-1, mirrored side, three legs): on 1c96c22f the crank selects asset 0 and returns
/// LockActive; with the fix it liquidates asset 1 (the first leg the kernel accepts), leaves the
/// barrier leg and the barrier untouched, and repeated cranks keep making progress.
#[test]
fn g9_livefix_r2_short_barrier_three_legs_crank_liquidates_the_first_unblocked_leg() {
    let mut w = r2_world();
    let c0 = cert(&w.t);
    eprintln!("R2 cert: equity={} mm={} deficit={}", c0.certified_equity, c0.certified_maintenance_req, c0.certified_liq_deficit);
    assert!(c0.valid && c0.certified_liq_deficit != 0, "T must be below maintenance");
    let legs: Vec<_> = w.t.legs.iter().map(|l| l.try_to_runtime().unwrap()).filter(|l| l.active).collect();
    assert_eq!(legs.len(), 3);
    assert_eq!(legs[0].asset_index, 0, "slot 0 holds the asset-0 (barrier) leg");
    // the kernel itself refuses the barrier leg (the barrier is intentional and stays)
    {
        let mut probe = (w.h, w.m.clone(), w.t);
        let mut g = MarketGroupV16ViewMut::new(&mut probe.0, &mut probe.1);
        let mut v = PortfolioV16ViewMut::new(&mut probe.2);
        assert_eq!(
            g.liquidate_account_not_atomic(&mut v, LiquidationRequestV16 { asset_index: 0 }).map(|_| ()),
            Err(V16Error::LockActive),
            "a direct liquidation of the barrier leg is still refused"
        );
    }
    let basis0 = leg_basis_for_asset(&w.t, 0);
    let basis1 = leg_basis_for_asset(&w.t, 1);
    let vault_before = w.h.vault.get();
    let r = auto_crank(&mut w.h, &mut w.m, &mut w.t);
    eprintln!("R2 auto-crank: {r:?}");
    assert!(
        matches!(r, Ok(ref x) if x.selected == AutoCrankPlanV16::Liquidate { asset_index: 1 }
            && matches!(x.outcome, AutoCrankOutcomeV16::Progressed(_))),
        "public crank must liquidate the first unblocked leg: {r:?}"
    );
    assert_eq!(leg_basis_for_asset(&w.t, 0), basis0, "the barrier leg is not touched");
    assert!(leg_basis_for_asset(&w.t, 1).unsigned_abs() < basis1.unsigned_abs(), "asset-1 leg reduced");
    assert_eq!(w.m[0].engine.pending_domain_loss_barrier_short.get(), 1, "the barrier stays up");
    assert_eq!(w.h.vault.get(), vault_before, "no external value moves (fee 0)");
    // keep cranking (keeper loop) until T is healthy or only the barrier leg is left
    for _ in 0..8 {
        if cert(&w.t).valid && cert(&w.t).certified_liq_deficit == 0 {
            break;
        }
        if !cert(&w.t).valid || w.t.stale_state != 0 || w.t.b_stale_state != 0 {
            refresh(&mut w.h, &mut w.m, &mut w.t, 1).unwrap();
            continue;
        }
        let r = auto_crank(&mut w.h, &mut w.m, &mut w.t);
        eprintln!("R2 next crank: {:?}", r.as_ref().map(|x| x.selected));
        if leg_basis_for_asset(&w.t, 1) == 0 && leg_basis_for_asset(&w.t, 2) == 0 {
            break;
        }
        assert!(r.is_ok(), "a later crank on an unblocked leg must not be refused: {r:?}");
    }
    let _ = &w.l2;
}

/// R2 control: the same world without L2's bankruptcy (no barrier): selection is unchanged
/// (slot 0, asset 0), exactly as on 1c96c22f.
#[test]
fn g9_livefix_r2_control_without_barrier_selection_is_unchanged() {
    let c = Cfg { n: 3, fee_bps: 0, min_liq_abs: 0, lifetime: 1_000 };
    let (mut h, mut m) = market(c).expect("config");
    let mut t = account(20);
    let mut cp = account(21);
    deposit(&mut h, &mut m, &mut t, 6_000);
    deposit(&mut h, &mut m, &mut cp, 1_000_000_000);
    let unit = UNIT_Q;
    trade(&mut h, &mut m, &mut cp, &mut t, 0, unit, P0).unwrap();
    trade(&mut h, &mut m, &mut t, &mut cp, 1, unit, P0).unwrap();
    trade(&mut h, &mut m, &mut t, &mut cp, 2, 2 * unit, P0).unwrap();
    let mut p0 = P0;
    for _ in 0..4 {
        p0 = down(p0);
        tick(&mut h, &mut m, &[p0, P0, P0]).unwrap();
    }
    let (mut p1, mut p2) = (P0, P0);
    for _ in 0..5 {
        p1 = down(p1);
        p2 = down(p2);
        tick(&mut h, &mut m, &[p0, p1, p2]).unwrap();
    }
    refresh(&mut h, &mut m, &mut t, 0).unwrap();
    assert!(cert(&t).certified_liq_deficit != 0);
    let r = auto_crank(&mut h, &mut m, &mut t).expect("crank liquidates");
    assert_eq!(r.selected, AutoCrankPlanV16::Liquidate { asset_index: 0 }, "first liquidatable leg, as before");
}

/// R3 fallback: when EVERY liquidatable leg is behind a barrier the selector falls back to the
/// first liquidatable leg and the crank is refused (LockActive) with no state change, exactly as on
/// 1c96c22f (the barrier's loss ordering is preserved). T holds only the asset-0 long of R1, and
/// asset 0 reverses after the barrier went up.
#[test]
fn g9_livefix_r3_all_legs_behind_barriers_still_refused_without_state_change() {
    let c = Cfg { n: 2, fee_bps: 0, min_liq_abs: 0, lifetime: 1_000 };
    let (mut h, mut m) = market(c).expect("config");
    let mut t = account(30);
    let mut cp = account(31);
    let mut s2 = account(32);
    let mut l2 = account(33);
    deposit(&mut h, &mut m, &mut t, 1_500);
    deposit(&mut h, &mut m, &mut cp, 1_000_000_000);
    deposit(&mut h, &mut m, &mut s2, 1_200);
    deposit(&mut h, &mut m, &mut l2, 1_000_000_000);
    let unit = UNIT_Q;
    trade(&mut h, &mut m, &mut l2, &mut s2, 0, unit, P0).unwrap();
    trade(&mut h, &mut m, &mut t, &mut cp, 0, unit, P0).unwrap();
    let mut p0 = P0;
    for _ in 0..4 {
        p0 = up(p0);
        tick(&mut h, &mut m, &[p0, P0]).unwrap();
    }
    trade(&mut h, &mut m, &mut s2, &mut l2, 0, unit, p0).expect("S2 terminal close");
    assert_eq!(m[0].engine.pending_domain_loss_barrier_long.get(), 1);
    // asset 0 reverses hard: T's only leg (long, behind the long barrier) goes below maintenance
    for _ in 0..8 {
        p0 = down(p0);
        tick(&mut h, &mut m, &[p0, P0]).unwrap();
    }
    refresh(&mut h, &mut m, &mut t, 0).unwrap();
    let c0 = cert(&t);
    eprintln!("R3 cert: equity={} mm={} deficit={}", c0.certified_equity, c0.certified_maintenance_req, c0.certified_liq_deficit);
    assert!(c0.valid && c0.certified_liq_deficit != 0, "T must be below maintenance");
    assert_eq!(m[0].engine.pending_domain_loss_barrier_long.get(), 1, "barrier still pending");
    let before = (h, m.clone(), t);
    let r = auto_crank(&mut h, &mut m, &mut t);
    eprintln!("R3 auto-crank: {r:?}");
    assert_eq!(r.map(|x| x.selected), Err(V16Error::LockActive), "unchanged refusal on the barrier leg");
    // no state change on the refusal, so the SVM rollback is a no-op (compare raw bytes)
    assert_eq!(bytes_of(&before.2), bytes_of(&t), "account unchanged");
    assert_eq!(bytes_of(&before.0), bytes_of(&h), "header unchanged");
    for i in 0..m.len() {
        assert_eq!(bytes_of(&before.1[i].engine), bytes_of(&m[i].engine), "asset {i} unchanged");
    }
}

fn bytes_of<T: Copy>(v: &T) -> Vec<u8> {
    let p = v as *const T as *const u8;
    // SAFETY: plain-old-data engine account structs; only read for equality in a test.
    unsafe { std::slice::from_raw_parts(p, std::mem::size_of::<T>()) }.to_vec()
}

/// R4 (security review LIVEFIX, finding 1): a BANKRUPT two-leg account (capital 0, pnl < 0)
/// whose slot-0 leg is behind a barrier. On 1c96c22f the crank returned LockActive (no progress
/// until the barrier cleared). With the fix it selects the unblocked asset-1 leg. The engine's
/// multi-asset-deficit path (row #18, `liquidation_lock`) applies: the leg is reduced without
/// consuming insurance or booking residual against the surviving asset; no Recovery is declared,
/// the barrier leg and the barrier are untouched.
#[test]
fn g9_livefix_r4_bankrupt_account_with_barrier_leg_is_reduced_on_its_unblocked_leg() {
    let mut w = r1_world_with(12);
    let c0 = cert(&w.t);
    eprintln!("R4 cert: equity={} mm={} deficit={} capital={} pnl={} lock={}", c0.certified_equity, c0.certified_maintenance_req, c0.certified_liq_deficit, w.t.capital.get(), w.t.pnl.get(), w.t.liquidation_lock);
    assert!(c0.valid && c0.certified_equity < 0, "T must be bankrupt");
    assert_eq!(w.m[0].engine.pending_domain_loss_barrier_long.get(), 1, "barrier still pending");
    let basis0 = leg_basis_for_asset(&w.t, 0);
    let basis1 = leg_basis_for_asset(&w.t, 1);
    let insurance_before = w.h.insurance.get();
    let r = auto_crank(&mut w.h, &mut w.m, &mut w.t);
    eprintln!("R4 auto-crank: {r:?} lock={}", w.t.liquidation_lock);
    let r = r.expect("the crank makes progress");
    assert_eq!(r.selected, AutoCrankPlanV16::Liquidate { asset_index: 1 });
    assert!(matches!(r.outcome, AutoCrankOutcomeV16::Progressed(_)));
    assert_eq!(mode(&w.h), 0, "no Recovery declared");
    assert!(leg_basis_for_asset(&w.t, 1).unsigned_abs() < basis1.unsigned_abs(), "asset-1 leg reduced");
    assert_eq!(leg_basis_for_asset(&w.t, 0), basis0, "the barrier leg is not touched");
    assert_eq!(w.m[0].engine.pending_domain_loss_barrier_long.get(), 1, "the barrier stays up");
    assert_eq!(w.h.insurance.get(), insurance_before, "no insurance consumed on the lock path");
}
