//! Liveness of liquidation when `unilateral_close_capacity` caps the close (P45, follow-up to P44).
//!
//! P44's reviewer flagged an untested path: when `unilateral_close_capacity` (src/v16.rs:21910,
//! kernel :1358, `account_effective_abs.min(oi_eff_long_q).min(oi_eff_short_q)`) caps the
//! close below the leg, can liquidation fail to make progress and strand an unhealthy account?
//!
//! HOW THE CAP BINDS. A leg's effective quantity is CEILed (`kernel_adl_effective_quantity_ceil`,
//! src/v16.rs:2443) while the side aggregate is FLOOR-rescaled by every unilateral close
//! (`reduce_matching_open_interest_for_unilateral_close`, :21950). After a haircut the side's
//! last leg can therefore exceed `oi_eff` by one atom (the GH#290 gap, see
//! `effective_abs_quantity_for_detach`, :2496). The fixture below produces it with two longs
//! (or shorts) and a weak counterparty liquidated unilaterally first: eff(b) = oi + 1 once `a`
//! is gone. Liquidating `b` then gets `close_budget = min(request, eff, oi) = eff - 1`
//! (`liquidate_account_inner_not_atomic`, :22271-22275).
//!
//! WHAT THE SECOND FEE CALL IS. The goal named "the second fee call can return NonProgress
//! (:22455)". In this tree the fee call is `charge_account_fee_after_full_refresh_not_atomic`
//! at :22471; its body (:24905-24965) has no NonProgress exit (only LockActive, BStale and
//! counter errors). The NonProgress exits of liquidation are :22211 (zero deficit, the
//! "healthy" refusal) and :22273-22275 (`close_budget == 0`). So the path is reached as
//! follows: capped close (this file) -> next call.
//!
//! RESULTS (see each test):
//!  * non-bankrupt (equity >= 0, full close requested): the capped close of eff-1 succeeds, the
//!    last 1-atom leg is absorbed by the detach rule, the account is flat, no Recovery. The
//!    second call is InvalidLeg (flat), never a `close_budget == 0` NonProgress. Progress.
//!  * bankrupt (equity < 0): the capped close leaves 1 atom of open risk beside uncovered loss,
//!    :22424 declares Recovery (ActiveBankruptCloseCannotProgress) and returns RecoveryRequired;
//!    every later call is LockActive. STUCK in Live. Pinned as `#[ignore]` tests below.
//!  * Toly (b97e1746, src/v16.rs:13420 `liquidate_account_not_atomic`): NO capacity cap at all;
//!    close_q = min(request, basis). So there is no `close_budget == 0` NonProgress and no
//!    capped-bankrupt Recovery. Upstream has no ceil-effective-quantity either (it closes the
//!    stored basis), so the one-atom gap that triggers the cap here is specific to this fork.
//!    Upstream instead calls `liquidation_close_would_leave_uncovered_loss_with_open_risk`
//!    (:13472) for the same Recovery rule, which a full-basis close never trips.
//!    (Read-only comparison, upstream not run.)
//!
//! Every test name contains `nonprogress_liveness`.
use percolator::*;

const P0: u64 = 1_000;

fn account(seed: u8) -> PortfolioAccountV16Account {
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        [1; 32], [seed; 32], [3; 32],
    ));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(header).unwrap();
    a
}

struct W {
    header: MarketGroupV16HeaderAccount,
    markets: Vec<Market<u64>>,
    accts: Vec<PortfolioAccountV16Account>,
    slot: u64,
}

fn build(caps: &[u128]) -> W {
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    cfg.maintenance_margin_bps = 1_000;
    cfg.initial_margin_bps = 1_000;
    cfg.max_price_move_bps_per_slot = 500;
    cfg.liquidation_fee_bps = 50;
    cfg.liquidation_fee_cap = 1_000_000;
    cfg.min_liquidation_abs = 0;
    cfg.min_nonzero_mm_req = 100;
    cfg.min_nonzero_im_req = 101;
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut markets = vec![Market::new(0u64, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, P0, 1)
        .unwrap();
    let mut accts: Vec<_> = (0..caps.len()).map(|i| account(10 + i as u8)).collect();
    {
        let mut g = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        for (a, c) in accts.iter_mut().zip(caps) {
            let mut v = PortfolioV16ViewMut::new(a);
            g.deposit_not_atomic(&mut v, *c).unwrap();
        }
    }
    W {
        header,
        markets,
        accts,
        slot: 2,
    }
}

impl W {
    fn trade(&mut self, long: usize, short: usize, q: u128) {
        let mut g = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let (a, b) = two(&mut self.accts, long, short);
        let mut lv = PortfolioV16ViewMut::new(a);
        let mut sv = PortfolioV16ViewMut::new(b);
        g.execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut lv,
            &mut sv,
            TradeRequestV16 {
                asset_index: 0,
                size_q: q as i128,
                exec_price: P0,
                fee_bps: 0,
            },
            true,
        )
        .unwrap_or_else(|e| panic!("trade {long} {short} {q}: {e:?}"));
    }
    fn move_price(&mut self, target: u64) {
        let mut g = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let mut price = g.markets[0]
            .engine
            .asset
            .try_to_runtime()
            .unwrap()
            .effective_price;
        while price != target {
            let step = (price * 500 / 10_000).max(1);
            price = if target > price {
                (price + step).min(target)
            } else {
                price.saturating_sub(step).max(target)
            };
            self.slot += 1;
            g.accrue_asset_to_not_atomic(0, self.slot, price, 0, true)
                .unwrap();
            g.markets[0].engine.asset.raw_oracle_target_price = V16PodU64::new(price);
        }
    }
    fn liq(&mut self, i: usize) -> Result<LiquidationOutcomeV16, V16Error> {
        let mut g = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let mut v = PortfolioV16ViewMut::new(&mut self.accts[i]);
        g.liquidate_account_not_atomic(&mut v, LiquidationRequestV16 { asset_index: 0 })
    }
    fn refresh(&mut self, i: usize) -> HealthCertV16 {
        let mut g = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let mut v = PortfolioV16ViewMut::new(&mut self.accts[i]);
        g.full_account_refresh_not_atomic(&mut v).unwrap()
    }
}
#[allow(non_snake_case)]
fn V16_eff(a: AssetStateV16, leg: PortfolioLegV16) -> u128 {
    let cur = if leg.basis_pos_q > 0 {
        a.a_long
    } else {
        a.a_short
    };
    (leg.basis_pos_q.unsigned_abs() * cur).div_ceil(leg.a_basis)
}
fn two(
    v: &mut [PortfolioAccountV16Account],
    i: usize,
    j: usize,
) -> (
    &mut PortfolioAccountV16Account,
    &mut PortfolioAccountV16Account,
) {
    assert!(i != j);
    if i < j {
        let (a, b) = v.split_at_mut(j);
        (&mut a[i], &mut b[0])
    } else {
        let (a, b) = v.split_at_mut(i);
        (&mut b[0], &mut a[j])
    }
}

const FLAT_GAP_CAPITAL_A: u128 = 4_000;

/// Two same-side legs a, b (target side) with a weak counterparty liquidated unilaterally
/// first, then the price walked back until a and b are underwater. `tgt_long` mirrors the whole
/// scenario (price p -> 2000 - p). `lag_bps` puts the wrapper-owned raw target that far behind
/// the effective price on the target's adverse side (non-zero target lag).
/// Returns the world with `a` already fully liquidated and `b` as the side's last leg.
fn setup(tgt_long: bool, price: u64, lag_bps: u64, cap_b: u128) -> W {
    let (q1, q2, qs1) = (33_333_333u128, 66_666_667u128, 20_000_003u128);
    let mut w = build(&[FLAT_GAP_CAPITAL_A, cap_b, 2_500, 100_000_000]);
    let (a, b, weak, strong) = (0, 1, 2, 3);
    let t = |w: &mut W, l: usize, s: usize, q: u128| {
        if tgt_long {
            w.trade(l, s, q)
        } else {
            w.trade(s, l, q)
        }
    };
    t(&mut w, a, weak, qs1);
    t(&mut w, a, strong, q1 - qs1);
    t(&mut w, b, strong, q2);
    let px = |p: u64| if tgt_long { p } else { 2_000 - p };
    w.move_price(px(1_120));
    assert_eq!(
        w.liq(weak).map(|o| o.closed_q),
        Ok(qs1),
        "weak counterparty liquidated unilaterally"
    );
    w.move_price(px(price));
    if lag_bps != 0 {
        let eff = w.markets[0]
            .engine
            .asset
            .try_to_runtime()
            .unwrap()
            .effective_price;
        let raw = if tgt_long {
            eff * (10_000 - lag_bps) / 10_000
        } else {
            eff * (10_000 + lag_bps) / 10_000
        };
        w.markets[0].engine.asset.raw_oracle_target_price = V16PodU64::new(raw);
        assert_ne!(raw, eff, "target lag is non-zero");
    }
    w.refresh(a);
    w.refresh(b);
    // a is liquidated first and fully, which detaches its CEILed quantity from the side oi.
    let ra = w.liq(a).unwrap();
    assert_eq!(ra.closed_q, 26_666_666, "a is fully closed");
    assert!(w.is_flat(a));
    w
}

impl W {
    fn is_flat(&self, i: usize) -> bool {
        !self.accts[i].legs[0].try_to_runtime().unwrap().active
    }
    /// (eff of b's leg, oi of its own side, oi of the opposite side)
    fn gap(&self, i: usize) -> (u128, u128, u128) {
        let a = self.markets[0].engine.asset.try_to_runtime().unwrap();
        let leg = self.accts[i].legs[0].try_to_runtime().unwrap();
        (V16_eff(a, leg), a.oi_eff_long_q, a.oi_eff_short_q)
    }
    fn assert_sound(&mut self) {
        let v = self.header.vault.get();
        let c = self.header.c_tot.get();
        let i = self.header.insurance.get();
        assert!(v >= c + i, "conservation broken: V {v} < C_tot {c} + I {i}");
        let g = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        g.validate_shape().unwrap();
    }
}

/// b: equity ~0 but not bankrupt, so the engine requests a FULL close; the cap clips it to eff-1.
fn capped_nonbankrupt_close_progresses(tgt_long: bool, lag_bps: u64) {
    let mut w = setup(tgt_long, 820, lag_bps, 8_000);
    let (eff, oi_l, oi_s) = w.gap(1);
    assert_eq!(oi_l, oi_s);
    assert_eq!(
        eff,
        oi_l + 1,
        "the cap binds: eff {eff} > oi {oi_l} (CEIL leg vs FLOOR aggregate)"
    );
    let cert = w.refresh(1);
    assert!(
        cert.certified_liq_deficit > 0 && cert.certified_equity >= 0,
        "{cert:?}"
    );

    let out = w
        .liq(1)
        .expect("a capped, non-bankrupt close must still liquidate");
    assert_eq!(
        out.closed_q,
        eff - 1,
        "close budget = min(request, eff, oi) = eff - 1"
    );
    assert_eq!(
        out.insurance_used + out.residual_booked + out.explicit_loss,
        0
    );
    // The 1-atom remainder is not stranded: the book is empty, the side was reset, and the
    // next touch (refresh) settles the prior-epoch leg away.
    let (_, oi_l, oi_s) = w.gap(1);
    assert_eq!((oi_l, oi_s), (0, 0));
    w.refresh(1);
    assert!(w.is_flat(1), "no dust leg left behind");
    assert_eq!(w.header.bankruptcy_hlock_active, 0);
    // Across further slots nothing is stuck: a flat account is refused as InvalidLeg (never a
    // `close_budget == 0` NonProgress) and the market stays Live.
    for _ in 0..3 {
        w.slot += 1;
        assert_eq!(w.liq(1).unwrap_err(), V16Error::InvalidLeg);
    }
    w.assert_sound();
}

#[test]
fn nonprogress_liveness_capped_nonbankrupt_close_progresses_long_no_lag() {
    capped_nonbankrupt_close_progresses(true, 0);
}
#[test]
fn nonprogress_liveness_capped_nonbankrupt_close_progresses_long_with_lag() {
    capped_nonbankrupt_close_progresses(true, 300);
}
#[test]
fn nonprogress_liveness_capped_nonbankrupt_close_progresses_short_no_lag() {
    capped_nonbankrupt_close_progresses(false, 0);
}
#[test]
fn nonprogress_liveness_capped_nonbankrupt_close_progresses_short_with_lag() {
    capped_nonbankrupt_close_progresses(false, 300);
}

/// Control: when the cap does NOT bind (no gap), a healed account's next call is the plain
/// zero-deficit NonProgress (src/v16.rs:22211): refused, nothing charged, not stuck.
#[test]
fn nonprogress_liveness_uncapped_partial_then_second_call_is_healthy_refusal() {
    let mut w = build(&[100_000_000, 17_000, 100_000_000]);
    w.trade(1, 2, 66_666_667);
    w.move_price(820);
    let (eff, oi_l, oi_s) = w.gap(1);
    assert!(eff <= oi_l.min(oi_s), "no gap: the cap does not bind");
    let first = w.liq(1).unwrap();
    assert!(first.closed_q > 0 && first.closed_q < eff, "{first:?}");
    let cap0 = w.accts[1].capital.get();
    assert_eq!(w.refresh(1).certified_liq_deficit, 0);
    for _ in 0..3 {
        w.slot += 1;
        assert_eq!(w.liq(1).unwrap_err(), V16Error::NonProgress);
    }
    assert_eq!(w.accts[1].capital.get(), cap0, "no second fee");
    w.assert_sound();
}

/// b is BANKRUPT (equity < 0): the full close is requested, the cap clips it to eff-1, one atom
/// of open risk remains beside uncovered loss, and src/v16.rs:22424-22431 declares Recovery
/// (ActiveBankruptCloseCannotProgress). From then on every liquidation is LockActive (:22190ff
/// Live gate) so b can never be liquidated in the live market, for the sake of ONE atom of
/// rounding. Desired: the capped bankrupt close completes (the last leg absorbed as the
/// detach rule does for non-bankrupt accounts, :2496) and the loss is charged normally.
/// Toly b97e1746 has no cap (close_q = min(request, basis), :13420-13470), so this path does
/// not exist there; it is specific to this fork's CEIL-eff + capacity cap.
fn capped_bankrupt_close_progresses(tgt_long: bool, lag_bps: u64) {
    let mut w = setup(tgt_long, 800, lag_bps, 7_500);
    let (eff, oi_l, _) = w.gap(1);
    assert_eq!(eff, oi_l + 1, "the cap binds");
    assert!(w.refresh(1).certified_equity < 0, "bankrupt");
    let mut last = Err(V16Error::NonProgress);
    for _ in 0..4 {
        last = w.liq(1);
        if last.is_ok() {
            let mut g = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
            let mut v = PortfolioV16ViewMut::new(&mut w.accts[1]);
            let _ = g.full_account_refresh_not_atomic(&mut v);
        }
        if w.is_flat(1) {
            break;
        }
        w.slot += 1;
    }
    assert!(
        w.is_flat(1),
        "liquidation must make progress across slots; got {last:?} (market stuck in Recovery)"
    );
}

#[test]
#[ignore = "BUG src/v16.rs:22424 (cap src/v16.rs:22271): capped bankrupt close of the last long leg declares Recovery for a 1-atom gap, then LockActive forever; Toly b97e1746 has no cap so no such path"]
fn nonprogress_liveness_capped_bankrupt_close_progresses_long_no_lag() {
    capped_bankrupt_close_progresses(true, 0);
}
#[test]
#[ignore = "BUG src/v16.rs:22424 (cap src/v16.rs:22271): same as long/no-lag, with non-zero target lag; Toly b97e1746 has no cap"]
fn nonprogress_liveness_capped_bankrupt_close_progresses_long_with_lag() {
    capped_bankrupt_close_progresses(true, 300);
}
#[test]
#[ignore = "BUG src/v16.rs:22424 (cap src/v16.rs:22271): short side mirror; Toly b97e1746 has no cap"]
fn nonprogress_liveness_capped_bankrupt_close_progresses_short_no_lag() {
    capped_bankrupt_close_progresses(false, 0);
}
#[test]
#[ignore = "BUG src/v16.rs:22424 (cap src/v16.rs:22271): short side, non-zero target lag; Toly b97e1746 has no cap"]
fn nonprogress_liveness_capped_bankrupt_close_progresses_short_with_lag() {
    capped_bankrupt_close_progresses(false, 300);
}
