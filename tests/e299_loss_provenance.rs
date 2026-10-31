//! #299 path 2b: the account's loss provenance (`PortfolioAccountV16Account::loss_provenance`).
//! Pure attribution rules (record merge, gain reset, charge decision), the strict wire decode,
//! and the record the engine writes on a live account. Spec: upstream spec.md req 5 (residual is
//! charged only to the asset-side domain whose exposure generated it) and req 27 (residual
//! attribution is deterministic and independent of caller ordering).

use percolator::{
    decode_loss_provenance, effective_loss_provenance, encode_loss_provenance,
    liquidation_residual_charge, loss_provenance_after_pnl, AssetStateV16Account,
    EngineAssetSlotV16Account, LiquidationRequestV16, LiquidationResidualChargeV16,
    LossProvenanceV16, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, TradeRequestV16, V16Config, V16Error, V16PodU32,
    LOSS_PROVENANCE_NONE, LOSS_PROVENANCE_UNATTRIBUTABLE, POS_SCALE,
};
use proptest::prelude::*;
use LossProvenanceV16::{Domain, Unattributable};

// ---------------------------------------------------------------------------------------------
// Wire encoding
// ---------------------------------------------------------------------------------------------

#[test]
fn e299_provenance_wire_round_trips() {
    assert_eq!(decode_loss_provenance(LOSS_PROVENANCE_NONE), Ok(LossProvenanceV16::None));
    assert_eq!(decode_loss_provenance(LOSS_PROVENANCE_UNATTRIBUTABLE), Ok(Unattributable));
    assert_eq!(decode_loss_provenance(1), Ok(Domain(0)));
    for p in [LossProvenanceV16::None, Unattributable, Domain(0), Domain(7), Domain(31)] {
        assert_eq!(decode_loss_provenance(encode_loss_provenance(p).unwrap()), Ok(p));
    }
    // a domain that would collide with the sentinel is refused, never aliased
    assert!(encode_loss_provenance(Domain(u32::MAX as usize - 1)).is_err());
}

// ---------------------------------------------------------------------------------------------
// Record rules
// ---------------------------------------------------------------------------------------------

/// A single-leg account's write (the record may clear at pnl >= 0).
fn lp1(o: LossProvenanceV16, old: i128, new: i128, d: Option<usize>) -> LossProvenanceV16 {
    loss_provenance_after_pnl(o, old, new, d, false)
}

#[test]
fn e299_record_names_the_single_realizing_domain() {
    let p = lp1(LossProvenanceV16::None, 0, -10, Some(2));
    assert_eq!(p, Domain(2));
    // more of the same domain keeps it
    assert_eq!(lp1(p, -10, -30, Some(2)), Domain(2));
    // a loss from another domain makes it unattributable (no invented split)
    assert_eq!(lp1(p, -10, -30, Some(3)), Unattributable);
    // a loss that names no leg is unattributable
    assert_eq!(lp1(p, -10, -30, None), Unattributable);
}

#[test]
fn e299_gains_keep_the_record_while_negative_and_clear_it_at_zero() {
    let p = Domain(1);
    assert_eq!(lp1(p, -100, -40, Some(3)), Domain(1), "netting gain keeps it");
    assert_eq!(lp1(p, -100, -40, None), Domain(1));
    assert_eq!(lp1(p, -100, 0, None), LossProvenanceV16::None);
    assert_eq!(lp1(Unattributable, -100, 5, None), LossProvenanceV16::None);
    assert_eq!(lp1(Unattributable, -100, -1, None), Unattributable);
}

#[test]
fn e299_multi_leg_record_is_sticky_until_one_leg_and_pnl_nonnegative() {
    // multi-leg: a gain back to >= 0 does not clear; a later loss elsewhere is unattributable
    let p = loss_provenance_after_pnl(LossProvenanceV16::None, 0, -100, Some(0), true);
    let p = loss_provenance_after_pnl(p, -100, 50, None, true);
    assert_eq!(p, Domain(0), "multi-leg gain keeps the record");
    assert_eq!(loss_provenance_after_pnl(p, 50, -70, Some(4), true), Unattributable);
    // the first write with at most one leg and pnl >= 0 clears it
    assert_eq!(loss_provenance_after_pnl(p, 50, 60, None, false), LossProvenanceV16::None);
}

#[test]
fn e299_a_loss_absorbed_by_positive_face_is_still_recorded() {
    // pnl +15: domain 0 loses 10 (absorbed, pnl +5), then domain 1 loses 10 (pnl -5). The
    // deficit is not "domain 1's" just because it crossed zero: both realized it.
    let p = lp1(LossProvenanceV16::None, 15, 5, Some(0));
    assert_eq!(p, Domain(0));
    assert_eq!(lp1(p, 5, -5, Some(1)), Unattributable);
    // a non-loss decrease (released-pnl conversion, no leg) that leaves pnl >= 0 keeps the record
    assert_eq!(lp1(p, 5, 0, None), Domain(0));
}

#[test]
fn e299_no_record_on_a_negative_pnl_is_never_attributed() {
    assert_eq!(effective_loss_provenance(LossProvenanceV16::None, -1), Unattributable);
    assert_eq!(effective_loss_provenance(LossProvenanceV16::None, 0), LossProvenanceV16::None);
    // a later single-domain loss cannot launder an unknown earlier deficit into an attribution
    assert_eq!(lp1(LossProvenanceV16::None, -5, -9, Some(0)), Unattributable);
}

/// Apply a sequence of signed pnl deltas (loss domain per negative delta), returning the record.
fn replay(start: i128, steps: &[(i128, usize)], multi_leg: bool) -> LossProvenanceV16 {
    let (mut pnl, mut p) = (start, LossProvenanceV16::None);
    for &(delta, d) in steps {
        let next = pnl + delta;
        p = loss_provenance_after_pnl(p, pnl, next, if delta < 0 { Some(d) } else { None }, multi_leg);
        pnl = next;
    }
    p
}

fn shuffle<T: Clone>(v: &[T], seed: u64) -> Vec<T> {
    let mut out = v.to_vec();
    let mut x = seed | 1;
    for i in (1..out.len()).rev() {
        x ^= x << 13; x ^= x >> 7; x ^= x << 17;
        out.swap(i, (x % (i as u64 + 1)) as usize);
    }
    out
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 2048, .. ProptestConfig::default() })]

    /// Req 27: one refresh applies its negative entries before its positive ones; the record
    /// after the negative phase is the same for EVERY order of those entries (the merge is
    /// commutative and idempotent), and it is a Domain only when every entry shares it.
    #[test]
    fn e299_negative_phase_record_is_order_independent(
        start in -1_000i128..1_000,
        losses in prop::collection::vec((1i128..500, 0usize..4), 1..6),
        seed in any::<u64>(),
    ) {
        let steps: Vec<(i128, usize)> = losses.iter().map(|&(a, d)| (-a, d)).collect();
        let shuffled = shuffle(&steps, seed);
        let a = replay(start, &steps, false);
        let b = replay(start, &shuffled, false);
        prop_assert_eq!(a, b);
        let first = steps[0].1;
        let single = steps.iter().all(|s| s.1 == first);
        if start < 0 {
            // an unrecorded pre-existing deficit is never attributed
            prop_assert_eq!(a, Unattributable);
        } else if single {
            prop_assert_eq!(a, Domain(first));
        } else {
            prop_assert_eq!(a, Unattributable);
        }
    }

    /// Req 27 across refreshes (security review of 9dc1395c): while several legs are open, a
    /// caller can realize the legs' losses AND GAINS in any order (accrue one asset, refresh, the
    /// next, refresh...). The record is the same for every order of a mixed loss/gain sequence:
    /// Domain(d) iff every loss came from d, else Unattributable -- the gains (and the principal
    /// settlements they stand in for) never reset it mid-episode.
    #[test]
    fn e299_multi_leg_episode_record_is_order_independent_with_gains(
        start in 0i128..1_000,
        moves in prop::collection::vec((-500i128..500, 0usize..4), 1..8),
        seed in any::<u64>(),
    ) {
        let steps: Vec<(i128, usize)> = moves.into_iter().filter(|m| m.0 != 0).collect();
        prop_assume!(steps.iter().any(|m| m.0 < 0));
        let a = replay(start, &steps, true);
        let b = replay(start, &shuffle(&steps, seed), true);
        prop_assert_eq!(a, b);
        let losers: Vec<usize> = steps.iter().filter(|m| m.0 < 0).map(|m| m.1).collect();
        if losers.iter().all(|&d| d == losers[0]) {
            prop_assert_eq!(a, Domain(losers[0]));
        } else {
            prop_assert_eq!(a, Unattributable);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Charge decision (`liquidation_residual_charge`)
// ---------------------------------------------------------------------------------------------

#[test]
fn e299_charge_decision_table() {
    use LiquidationResidualChargeV16::{Charge, ReduceOnly, ResolveFlat};
    let none = LossProvenanceV16::None;
    for full in [false, true] {
        for flat in [false, true] {
            // unlocked: the closed leg's domain, whatever the record says (unchanged path)
            for p in [none, Unattributable, Domain(0), Domain(5)] {
                assert_eq!(liquidation_residual_charge(false, p, 3, full, flat), Charge(3));
            }
            // locked, unattributable or no record: never charged; ResolveFlat (never Recovery) once flat
            for p in [none, Unattributable] {
                let want = if flat { ResolveFlat } else { ReduceOnly };
                assert_eq!(liquidation_residual_charge(true, p, 3, full, flat), want);
            }
            // locked, attributed to the closed leg's domain: charged at its full close; flat -> ResolveFlat
            let want = if flat { ResolveFlat } else if full { Charge(3) } else { ReduceOnly };
            assert_eq!(liquidation_residual_charge(true, Domain(3), 3, full, flat), want);
            // locked, attributed elsewhere: the closed leg NEVER becomes the charged domain
            let want = if flat { ResolveFlat } else { ReduceOnly };
            assert_eq!(liquidation_residual_charge(true, Domain(0), 3, full, flat), want);
        }
    }
}

proptest! {
    /// T2a: for a locked account the charged domain never depends on which leg is closed.
    #[test]
    fn e299_locked_charge_never_names_the_closed_leg_unless_it_is_the_record(
        d in 0usize..32, closed in 0usize..32, full: bool, flat: bool,
    ) {
        if let LiquidationResidualChargeV16::Charge(c) =
            liquidation_residual_charge(true, Domain(d), closed, full, flat)
        {
            prop_assert_eq!(c, d);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Live state (harness as in tests/gh292_locked_liquidation_recovery.rs)
// ---------------------------------------------------------------------------------------------

const P0: u64 = 1_000_000;
const MARKET_ID: [u8; 32] = [7; 32];
const OWNER: [u8; 32] = [9; 32];

fn band_cfg(slots: u16) -> V16Config {
    let mut c = V16Config::public_user_fund_with_market_slots(slots, slots as u32, 0, 10);
    c.maintenance_margin_bps = 500;
    c.initial_margin_bps = 1_000;
    c.min_nonzero_mm_req = 10_000;
    c.min_nonzero_im_req = 20_000;
    c.liquidation_fee_bps = 50;
    c.liquidation_fee_cap = 1_000_000_000_000;
    c.min_liquidation_abs = 0;
    c.max_abs_funding_e9_per_slot = 111;
    c.max_price_move_bps_per_slot = 100;
    c.max_accrual_dt_slots = 3;
    c.min_funding_lifetime_slots = 3;
    c.max_trading_fee_bps = 100;
    c.rent_max_e9_per_slot = 23;
    c.max_bankrupt_close_lifetime_slots = 1_000_000;
    c
}

struct W2 {
    header: MarketGroupV16HeaderAccount,
    markets: Vec<Market<u64>>,
    accounts: Vec<PortfolioAccountV16Account>,
    now: u64,
}

impl W2 {
    fn new(cfg: V16Config, deposits: &[u128], n: usize) -> Self {
        cfg.validate_public_user_fund().expect("cfg validates");
        let mut header =
            MarketGroupV16HeaderAccount::new_dynamic(MARKET_ID, cfg, n as u32, 0).unwrap();
        let mut markets: Vec<Market<u64>> = (0..n)
            .map(|i| Market::new(i as u64, EngineAssetSlotV16Account::default()))
            .collect();
        for i in 0..n {
            header
                .activate_empty_asset_slot_not_atomic(
                    i as u32,
                    &mut markets[i].engine,
                    P0,
                    (i + 1) as u64,
                )
                .unwrap();
        }
        let mut accounts = Vec::new();
        for i in 0..deposits.len() {
            let mut k = [1u8; 32];
            k[0] = i as u8;
            k[1] = 1;
            let prov = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
                MARKET_ID, k, OWNER,
            ));
            let mut a = PortfolioAccountV16Account::default();
            a.init_empty_in_place(prov).unwrap();
            accounts.push(a);
        }
        let mut w = Self { header, markets, accounts, now: 1 };
        for (i, d) in deposits.iter().enumerate() {
            w.with(i, |m, a| m.deposit_not_atomic(a, *d)).unwrap();
        }
        w
    }

    fn with<R>(
        &mut self,
        i: usize,
        f: impl FnOnce(&mut MarketGroupV16ViewMut<'_, u64>, &mut PortfolioV16ViewMut<'_>) -> R,
    ) -> R {
        let mut m = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let mut a = PortfolioV16ViewMut::new(&mut self.accounts[i]);
        f(&mut m, &mut a)
    }

    fn market<R>(&mut self, f: impl FnOnce(&mut MarketGroupV16ViewMut<'_, u64>) -> R) -> R {
        let mut m = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        f(&mut m)
    }

    fn asset(&self, i: usize) -> percolator::AssetStateV16 {
        self.markets[i].engine.asset.try_to_runtime().unwrap()
    }

    fn trade(&mut self, ai: usize, long: usize, short: usize, q: u128) -> Result<(), V16Error> {
        let price = self.asset(ai).effective_price;
        let (lo, hi) = if long < short { (long, short) } else { (short, long) };
        let (left, right) = self.accounts.split_at_mut(hi);
        let (a_lo, a_hi) = (&mut left[lo], &mut right[0]);
        let (l, s) = if long < short { (a_lo, a_hi) } else { (a_hi, a_lo) };
        let mut m = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let mut lv = PortfolioV16ViewMut::new(l);
        let mut sv = PortfolioV16ViewMut::new(s);
        m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut lv,
            &mut sv,
            TradeRequestV16 { asset_index: ai, size_q: q as i128, exec_price: price, fee_bps: 0 },
            true,
        )
        .map(|_| ())
    }

    fn step_price(&mut self, ai: usize, target: u64) {
        self.market(|m| m.set_asset_raw_oracle_target_not_atomic(ai, target)).unwrap();
        let a = self.asset(ai);
        let cfg = self.header.config.try_to_runtime_shape().unwrap();
        let dt = (self.now - a.slot_last).min(cfg.max_accrual_dt_slots).max(1);
        let p = a.effective_price;
        let exposed = a.oi_eff_long_q != 0 || a.oi_eff_short_q != 0;
        let capped = if !exposed {
            target
        } else {
            let md =
                (p as u128 * cfg.max_price_move_bps_per_slot as u128 * dt as u128 / 10_000) as u64;
            if target > p { p + md.min(target - p) } else { p - md.min(p - target) }
        };
        let now = self.now;
        let r = self.market(|m| m.accrue_asset_to_not_atomic(ai, now, capped, 0, true));
        if r.is_err() {
            println!("    accrue a{ai} err {r:?}");
        }
        let mut a = self.asset(ai);
        a.raw_oracle_target_price = capped;
        self.markets[ai].engine.asset = AssetStateV16Account::from_runtime(&a);
    }

    fn refresh(&mut self, i: usize) -> Result<percolator::HealthCertV16, V16Error> {
        let mut last = Err(V16Error::Stale);
        for _ in 0..16 {
            last = self.with(i, |m, a| m.full_account_refresh_not_atomic(a));
            if last != Err(V16Error::Stale) && last != Err(V16Error::BStale) {
                break;
            }
        }
        last
    }

    fn legs(&self, i: usize) -> Vec<(u32, bool)> {
        (0..4)
            .map(|s| {
                let l = self.accounts[i].legs[s].try_to_runtime().unwrap();
                (l.asset_index, l.active)
            })
            .collect()
    }
}


fn record(w: &W2) -> LossProvenanceV16 {
    decode_loss_provenance(w.accounts[0].loss_provenance.get()).unwrap()
}

/// Account 0 long a0 + long a1; a0 falls, a1 falls too when `drop_a1`. Returns at bad debt + lock.
fn latched(drop_a1: bool) -> W2 {
    let mut w = W2::new(band_cfg(2), &[260_000, 50_000_000, 50_000_000], 2);
    w.trade(0, 0, 1, POS_SCALE).unwrap();
    w.trade(1, 0, 2, POS_SCALE).unwrap();
    w.refresh(0).unwrap();
    for _ in 0..60 {
        w.now += 1;
        w.step_price(0, 1);
        w.step_price(1, if drop_a1 { 1 } else { P0 });
        let _ = w.refresh(0);
        if w.accounts[0].pnl.get() < 0 && w.accounts[0].liquidation_lock != 0 {
            return w;
        }
        // two legs open: the record is sticky for the episode and only ever names a loser
        let r = record(&w);
        if drop_a1 {
            assert!(matches!(r, LossProvenanceV16::None | Domain(0) | Domain(2) | Unattributable));
        } else {
            assert!(matches!(r, LossProvenanceV16::None | Domain(0)), "{r:?}");
        }
    }
    panic!("precondition: bad debt with the lock set");
}

#[test]
fn e299_engine_records_the_realizing_domain_and_clears_it_on_resolution() {
    let mut w = latched(false);
    // a0 long lost: loss domain (asset 0, Long) = 0
    assert_eq!(record(&w), Domain(0));
    let out = w.with(0, |m, a| m.liquidate_account_not_atomic(a, LiquidationRequestV16 { asset_index: 0 }));
    assert!(out.is_ok(), "{out:?}");
    assert_eq!(w.accounts[0].pnl.get(), 0);
    assert_eq!(record(&w), LossProvenanceV16::None, "resolved: record cleared");
    assert_eq!(w.accounts[0].liquidation_lock, 0);
}

#[test]
fn e299_engine_marks_a_two_domain_deficit_unattributable() {
    let w = latched(true);
    assert_eq!(record(&w), Unattributable);
}

#[test]
fn e299_out_of_range_record_fails_closed() {
    let mut w = latched(false);
    // 2 configured assets -> 4 domains; domain 4 is out of range
    w.accounts[0].loss_provenance = V16PodU32::new(1 + 4);
    let before = (w.header, w.accounts[0]);
    let out = w.with(0, |m, a| m.liquidate_account_not_atomic(a, LiquidationRequestV16 { asset_index: 1 }));
    assert_eq!(out.map(|o| o.closed_q), Err(V16Error::InvalidLeg));
    assert_eq!((w.header, w.accounts[0]), before, "rejected before any mutation");
    // the in-range record is accepted again
    w.accounts[0].loss_provenance = V16PodU32::new(1 + 3);
    assert!(w.refresh(0).is_ok());
}

// ---------------------------------------------------------------------------------------------
// Cross-refresh realization order (security review of 9dc1395c): a cranker can accrue assets one
// at a time and refresh the account between them, so losses and gains of different legs are
// realized in an order the CALLER picks. The record must not depend on it (spec req 27).
// ---------------------------------------------------------------------------------------------

/// Account 0: long a0 (vs 1), short a1 (vs 2), long a2 (vs 3). `targets[k]` is asset k's price
/// target; each slot the assets are stepped and the account refreshed in `order`, one asset at a
/// time. Returns the record once the account is in bad debt (or after 80 slots).
fn record_after_crank_order(targets: [u64; 3], order: [usize; 3]) -> (LossProvenanceV16, i128) {
    let mut w = W2::new(band_cfg(3), &[400_000, 50_000_000, 50_000_000, 50_000_000], 3);
    w.trade(0, 0, 1, POS_SCALE).unwrap();
    w.trade(1, 2, 0, POS_SCALE).unwrap();
    w.trade(2, 0, 3, POS_SCALE).unwrap();
    w.refresh(0).unwrap();
    w.now = 3; // asset k is activated at slot k + 1
    for _ in 0..80 {
        w.now += 1;
        for &ai in &order {
            w.step_price(ai, targets[ai]);
            let _ = w.refresh(0);
        }
        if w.accounts[0].pnl.get() < 0 && w.accounts[0].capital.get() == 0 {
            break;
        }
    }
    (record(&w), w.accounts[0].pnl.get())
}

const ORDERS: [[usize; 3]; 6] = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];

/// Two legs lose (a0 long, a2 long fall) while one gains (a1 short, a1 falls): in EVERY crank order
/// the record is unattributable. Before the multi-leg-sticky rule the record named whichever loser
/// was realized after the last gain/principal reset, so the caller chose who paid.
#[test]
fn e299_crank_order_cannot_choose_the_payer_two_losers() {
    let targets = [P0 / 2, P0 / 2, P0 / 2];
    let mut seen = Vec::new();
    for order in ORDERS {
        let (p, pnl) = record_after_crank_order(targets, order);
        assert!(pnl < 0, "precondition {order:?}: bad debt reached");
        seen.push((order, p));
    }
    for (order, p) in &seen {
        assert_eq!(*p, Unattributable, "order {order:?} attributed a two-domain loss: {seen:?}");
    }
}

/// Only a0 loses (a1 short gains, a2 flat): every crank order attributes the deficit to a0's loss
/// domain (asset 0, Long) = 0, i.e. gains interleaved with the loss never change the payer.
#[test]
fn e299_crank_order_cannot_choose_the_payer_one_loser() {
    let targets = [1, P0 * 98 / 100, P0];
    for order in ORDERS {
        let (p, pnl) = record_after_crank_order(targets, order);
        assert!(pnl < 0, "precondition {order:?}: bad debt reached");
        assert_eq!(p, Domain(0), "order {order:?}");
    }
}
