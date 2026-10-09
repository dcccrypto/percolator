//! fix/v21-funding-scale solvency proptest.
//!
//! Random worlds: thinly margined traders against one maker, random price paths (capped)
//! and random funding, a keeper that refreshes random subsets, random liquidation cranks,
//! insurance from zero to generous, and entrants trying to open at every step.
//!
//! Whenever a risk-increasing trade is ADMITTED while other legs are still K/F-stale (the
//! relaxed path), we check, on clones of the post-trade world:
//!   P1 bound soundness: for each side, the exact K/F loss the stale legs would recognize
//!      (recomputed leg-by-leg with the engine's floor formula) is <= the hidden-loss bound,
//!      which an independent re-implementation of the bound also reproduces;
//!   P2 cover: that bound is <= the insurance available to the domain that absorbs it;
//!   P3 no socialization onto the entrant: settling AND liquidating every account at the
//!      admission price (the engine's own auto-crank, full waterfall) leaves the entrant's
//!      equity untouched and books no B (no deficit reaches socialization), whenever the
//!      market had no already-recognized deficit at admission;
//!   P4 conservation: vault >= c_tot + insurance and every shape check holds after the
//!      cascade.
//! A control re-runs P3 with the insurance stripped from the clone, to show the property is
//! carried by the cover and is not vacuous.

use percolator::{
    AutoCrankWorkV16, EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount,
    MarketGroupV16ViewMut, PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, SideV16, TradeRequestV16, V16Config, V16PodU128, V16PodU64,
};
use percolator::{ADL_ONE, POS_SCALE, SOCIAL_WEIGHT_SCALE};
use proptest::prelude::*;

const PRICE: u64 = 1_000_000;
const RATE_E9: i128 = 10_000;
const MOVE_CAP_BPS: u64 = 200;

#[derive(Clone)]
struct World {
    header: MarketGroupV16HeaderAccount,
    markets: Vec<Market<u64>>,
    maker: PortfolioAccountV16Account,
    traders: Vec<PortfolioAccountV16Account>,
    slot: u64,
}

fn account(seed: u32) -> PortfolioAccountV16Account {
    let mut key = [0u8; 32];
    key[..4].copy_from_slice(&seed.to_le_bytes());
    key[31] = 0x5A;
    let header =
        ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new([1; 32], key, [3; 32]));
    let mut account = PortfolioAccountV16Account::default();
    account.init_empty_in_place(header).unwrap();
    account
}

impl World {
    fn new(trader_capitals: &[(u128, bool, u128)], ins_long: u128, ins_short: u128) -> Self {
        let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
        cfg.max_abs_funding_e9_per_slot = RATE_E9 as u64;
        cfg.max_price_move_bps_per_slot = MOVE_CAP_BPS;
        cfg.initial_margin_bps = 1_000;
        cfg.maintenance_margin_bps = 500;
        let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
        let mut markets = vec![Market::new(0, EngineAssetSlotV16Account::default())];
        header
            .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, PRICE, 1)
            .unwrap();
        let mut w = World {
            header,
            markets,
            maker: account(0),
            traders: Vec::new(),
            slot: 1,
        };
        let mut maker = w.maker;
        w.deposit(&mut maker, 1_000_000_000_000_000);
        w.maker = maker;
        for (i, &(capital, long, units)) in trader_capitals.iter().enumerate() {
            let mut t = account(1 + i as u32);
            w.deposit(&mut t, capital);
            let mut maker = w.maker;
            let size = units * POS_SCALE;
            let r = if long {
                w.trade(&mut t, &mut maker, size)
            } else {
                w.trade(&mut maker, &mut t, size)
            };
            r.expect("initial fills are on a fresh, current market");
            w.maker = maker;
            w.traders.push(t);
        }
        {
            let mut m = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
            if ins_long != 0 {
                m.deposit_domain_insurance_not_atomic(0, ins_long).unwrap();
            }
            if ins_short != 0 {
                m.deposit_domain_insurance_not_atomic(1, ins_short).unwrap();
            }
        }
        w
    }

    fn deposit(&mut self, acct: &mut PortfolioAccountV16Account, amount: u128) {
        let mut m = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        m.deposit_not_atomic(&mut PortfolioV16ViewMut::new(acct), amount)
            .unwrap();
    }

    fn price(&self) -> u64 {
        self.markets[0].engine.asset.effective_price.get()
    }

    fn asset(&self) -> percolator::AssetStateV16 {
        self.markets[0].engine.asset.try_to_runtime().unwrap()
    }

    /// Engine calls assume SVM rollback on `Err`; emulate it for the market and accounts.
    fn trade(
        &mut self,
        long: &mut PortfolioAccountV16Account,
        short: &mut PortfolioAccountV16Account,
        size: u128,
    ) -> Result<(), percolator::V16Error> {
        let (h, mk, l0, s0) = (self.header, self.markets.clone(), *long, *short);
        let r = self.trade_raw(long, short, size);
        if r.is_err() {
            self.header = h;
            self.markets = mk;
            *long = l0;
            *short = s0;
        }
        r
    }

    fn trade_raw(
        &mut self,
        long: &mut PortfolioAccountV16Account,
        short: &mut PortfolioAccountV16Account,
        size: u128,
    ) -> Result<(), percolator::V16Error> {
        let price = self.price();
        let mut m = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut PortfolioV16ViewMut::new(long),
            &mut PortfolioV16ViewMut::new(short),
            TradeRequestV16 {
                asset_index: 0,
                size_q: i128::try_from(size).unwrap(),
                exec_price: price,
                fee_bps: 0,
            },
            true,
        )
        .map(|_| ())
    }

    fn accrue(&mut self, dp_bps: i64, rate: i128) -> bool {
        let old = self.price() as i128;
        let new = (old + old * dp_bps as i128 / 10_000).max(1) as u64;
        let slot = self.slot + 1;
        let (h, mk) = (self.header, self.markets.clone());
        let mut m = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        if m.accrue_asset_to_not_atomic(0, slot, new, rate, true).is_err() {
            self.header = h;
            self.markets = mk;
            return false;
        }
        m.markets[0].engine.asset.raw_oracle_target_price = V16PodU64::new(new);
        self.slot = slot;
        true
    }

    fn crank(&mut self, acct: &mut PortfolioAccountV16Account) -> bool {
        let (h, mk, a0) = (self.header, self.markets.clone(), *acct);
        let ok = self.crank_raw(acct);
        if !ok {
            self.header = h;
            self.markets = mk;
            *acct = a0;
        }
        ok
    }

    fn crank_raw(&mut self, acct: &mut PortfolioAccountV16Account) -> bool {
        let slot = self.slot;
        let mut m = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        m.permissionless_auto_crank_not_atomic(
            &mut PortfolioV16ViewMut::new(acct),
            AutoCrankWorkV16 {
                now_slot: slot,
                observations: &[],
                resolved_close_fee_rate_per_slot: 0,
            },
        )
        .is_ok()
    }

    fn refresh(&mut self, acct: &mut PortfolioAccountV16Account) -> bool {
        let (h, mk, a0) = (self.header, self.markets.clone(), *acct);
        let mut m = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let ok = m
            .full_account_refresh_not_atomic(&mut PortfolioV16ViewMut::new(acct))
            .is_ok();
        if !ok {
            self.header = h;
            self.markets = mk;
            *acct = a0;
        }
        ok
    }

    fn validate(&mut self, extra: &mut [PortfolioAccountV16Account]) {
        let m = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        m.validate_shape().unwrap();
        PortfolioV16ViewMut::new(&mut self.maker)
            .validate_with_market(&m.as_view())
            .unwrap();
        for (i, t) in self.traders.iter_mut().chain(extra.iter_mut()).enumerate() {
            if let Err(e) = PortfolioV16ViewMut::new(t).validate_with_market(&m.as_view()) {
                let leg = t.legs[0].try_to_runtime();
                let a = m.markets[0].engine.asset.try_to_runtime().unwrap();
                panic!(
                    "account {i} invalid vs market: {e:?}\n leg {leg:?}\n asset modes {:?}/{:?} epochs {}/{} kf {}/{} stale {}/{} stored {}/{} oi {}/{} lifecycle {:?} mode {:?}",
                    a.mode_long, a.mode_short, a.epoch_long, a.epoch_short, a.kf_epoch_long, a.kf_epoch_short,
                    a.stale_account_count_long, a.stale_account_count_short, a.stored_pos_count_long, a.stored_pos_count_short,
                    a.oi_eff_long_q, a.oi_eff_short_q, a.lifecycle, self.header.mode
                );
            }
        }
        let vault = self.header.vault.get();
        let senior = self.header.c_tot.get() + self.header.insurance.get();
        assert!(vault >= senior, "P4: vault {vault} < c_tot + insurance {senior}");
    }

    /// Crank every account at the current slot until nothing makes progress.
    fn cascade(&mut self, extra: &mut [PortfolioAccountV16Account]) {
        for _round in 0..6 {
            let mut progressed = false;
            let mut maker = self.maker;
            for _ in 0..4 {
                if !self.crank(&mut maker) {
                    break;
                }
                progressed = true;
            }
            self.maker = maker;
            for i in 0..self.traders.len() {
                let mut t = self.traders[i];
                for _ in 0..4 {
                    if !self.crank(&mut t) {
                        break;
                    }
                    progressed = true;
                }
                self.traders[i] = t;
            }
            for e in extra.iter_mut() {
                for _ in 0..4 {
                    if !self.crank(e) {
                        break;
                    }
                    progressed = true;
                }
            }
            if !progressed {
                break;
            }
        }
    }
}

fn equity(a: &PortfolioAccountV16Account) -> i128 {
    a.capital.get() as i128 + a.pnl.get()
}

fn floor_div(n: i128, d: i128) -> i128 {
    let q = n / d;
    if (n % d != 0) && ((n < 0) != (d < 0)) {
        q - 1
    } else {
        q
    }
}

/// Exact K/F loss a stale leg would recognize at settlement (the engine's floor formula).
fn stale_leg_loss(asset: &percolator::AssetStateV16, a: &PortfolioAccountV16Account) -> Option<(SideV16, u128)> {
    let bitmap = a.active_bitmap[0].get();
    if bitmap & 1 == 0 {
        return None;
    }
    let leg = a.legs[0].try_to_runtime().unwrap();
    let (k, f, epoch) = match leg.side {
        SideV16::Long => (asset.k_long, asset.f_long_num, asset.kf_epoch_long),
        SideV16::Short => (asset.k_short, asset.f_short_num, asset.kf_epoch_short),
    };
    if leg.kf_epoch_snap >= epoch {
        return None;
    }
    let basis = leg.basis_pos_q.unsigned_abs() as i128;
    let den = (leg.a_basis * POS_SCALE) as i128;
    // per-leg persistent remainders (upstream a74b81b2) carry into the settlement numerators
    let net = floor_div(leg.k_rem_num as i128 + basis * (k - leg.k_snap), den)
        + floor_div(leg.f_rem_num as i128 + basis * (f - leg.f_snap), den);
    Some((leg.side, if net < 0 { net.unsigned_abs() } else { 0 }))
}

/// Independent re-implementation of the hidden-loss bound.
fn bound(w: &World, side: SideV16) -> u128 {
    let asset = w.asset();
    let (stale, d) = match side {
        SideV16::Long => (asset.stale_account_count_long, w.markets[0].engine.kf_drift_long.to_runtime()),
        SideV16::Short => (asset.stale_account_count_short, w.markets[0].engine.kf_drift_short.to_runtime()),
    };
    if stale == 0 {
        return 0;
    }
    let den = SOCIAL_WEIGHT_SCALE * POS_SCALE;
    // weights <= 1e21; drift here stays far below 1e17, so u128 products are exact.
    let g = d.stale_weight.checked_mul(d.drift_gen).expect("test worlds stay in u128").div_ceil(den);
    let p = if d.laggard_count == 0 {
        0
    } else {
        d.laggard_weight.checked_mul(d.drift_prior).expect("test worlds stay in u128").div_ceil(den)
    };
    g + p + 2 * u128::from(stale)
}

fn domain_available(w: &World, domain: usize) -> u128 {
    let mut header = w.header;
    let mut markets = w.markets.clone();
    let m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    m.domain_insurance_budget_remaining(domain)
        .unwrap()
        .min(w.header.insurance.get())
}

#[derive(Clone, Debug)]
enum Op {
    Accrue(i64, i128),
    /// Six capped slots one way with funding against the losing side: makes thin accounts
    /// go underwater without anyone settling them.
    Slide(bool),
    Refresh(usize),
    Crank(usize),
    Open(bool, u128),
    Add(usize, u128),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => ((-(MOVE_CAP_BPS as i64))..=(MOVE_CAP_BPS as i64), -RATE_E9..=RATE_E9)
            .prop_map(|(d, r)| Op::Accrue(d, r)),
        2 => (0usize..64).prop_map(Op::Refresh),
        2 => any::<bool>().prop_map(Op::Slide),
        1 => (0usize..64).prop_map(Op::Crank),
        3 => (any::<bool>(), 1u128..=3).prop_map(|(l, u)| Op::Open(l, u)),
        1 => (0usize..64, 1u128..=2).prop_map(|(i, u)| Op::Add(i, u)),
    ]
}

#[derive(Default, Debug)]
struct Stats {
    relaxed_admissions: u64,
    hidden_deficit_admissions: u64,
    p3_checked: u64,
    control_harm: u64,
}

fn check_admission(w: &World, entrant: &PortfolioAccountV16Account, stats: &mut Stats) {
    let asset = w.asset();
    // P1: soundness of the bound, per side, against the exact settlement of every stale leg.
    let mut real = [0u128; 2];
    for a in w.traders.iter().chain(core::iter::once(&w.maker)).chain(core::iter::once(entrant)) {
        if let Some((side, loss)) = stale_leg_loss(&asset, a) {
            real[(side == SideV16::Short) as usize] += loss;
        }
    }
    let mut deficit = false;
    for a in w.traders.iter().chain(core::iter::once(&w.maker)) {
        if let Some((_, loss)) = stale_leg_loss(&asset, a) {
            deficit |= equity(a) < loss as i128;
        }
    }
    if deficit {
        stats.hidden_deficit_admissions += 1;
    }
    for (i, side) in [SideV16::Long, SideV16::Short].into_iter().enumerate() {
        let b = bound(w, side);
        let mut header = w.header;
        let mut markets = w.markets.clone();
        let m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        assert_eq!(
            m.domain_insurance_withdraw_capacity(if side == SideV16::Long { 1 } else { 0 })
                .unwrap()
                .saturating_add(b)
                >= domain_available(w, if side == SideV16::Long { 1 } else { 0 }).min(w.header.vault.get()),
            true
        );
        assert!(real[i] <= b, "P1: side {side:?} real hidden loss {} > bound {b}", real[i]);
        // P2: the absorbing domain covers it.
        let absorbing = if side == SideV16::Long { 1 } else { 0 };
        assert!(
            b <= domain_available(w, absorbing),
            "P2: bound {b} exceeds available insurance {}",
            domain_available(w, absorbing)
        );
    }
    // P3 only when nothing was already recognized-but-unbooked at admission.
    if w.header.negative_pnl_account_count.get() != 0 {
        return;
    }
    let before = equity(entrant);
    let (b_long, b_short) = (asset.b_long_num, asset.b_short_num);
    let mut w2 = w.clone();
    let mut e2 = [*entrant];
    w2.cascade(&mut e2);
    let a2 = w2.asset();
    assert_eq!(
        (a2.b_long_num, a2.b_short_num),
        (b_long, b_short),
        "P3: a pre-admission deficit reached B-socialization"
    );
    assert_eq!(equity(&e2[0]), before, "P3: entrant charged for pre-entry loss");
    w2.validate(&mut e2);
    stats.p3_checked += 1;

    // Control: the same cascade with the cover stripped. Not asserted (most worlds have no
    // deficit at all); counted to show P3 is carried by the cover when one exists.
    let mut w3 = w.clone();
    w3.header.vault = V16PodU128::new(w3.header.vault.get() - w3.header.insurance.get());
    w3.header.insurance = V16PodU128::new(0);
    w3.header.insurance_domain_budget_remaining_total = V16PodU128::new(0);
    w3.markets[0].engine.insurance_domain_budget_long = V16PodU128::new(0);
    w3.markets[0].engine.insurance_domain_budget_short = V16PodU128::new(0);
    w3.markets[0].engine.insurance_domain_spent_long = V16PodU128::new(0);
    w3.markets[0].engine.insurance_domain_spent_short = V16PodU128::new(0);
    let mut e3 = [*entrant];
    w3.cascade(&mut e3);
    let a3 = w3.asset();
    if (a3.b_long_num, a3.b_short_num) != (b_long, b_short) || equity(&e3[0]) != before {
        stats.control_harm += 1;
    }
}

fn run(
    traders: Vec<(u128, bool, u128)>,
    ins: (u128, u128),
    ops: Vec<Op>,
    stats: &mut Stats,
) {
    let mut w = World::new(&traders, ins.0, ins.1);
    let mut entrants: Vec<PortfolioAccountV16Account> = Vec::new();
    let mut seed = 10_000u32;
    for op in ops {
        let op = match op {
            Op::Add(_, units) if entrants.is_empty() => Op::Open(true, units),
            other => other,
        };
        match op {
            Op::Accrue(d, r) => {
                w.accrue(d, r);
            }
            Op::Slide(up) => {
                for _ in 0..6 {
                    // Up hurts shorts; funding sign follows (positive = longs pay).
                    if up {
                        w.accrue(MOVE_CAP_BPS as i64, -RATE_E9);
                    } else {
                        w.accrue(-(MOVE_CAP_BPS as i64), RATE_E9);
                    }
                }
            }
            Op::Refresh(i) => {
                let n = w.traders.len();
                let mut t = w.traders[i % n];
                w.refresh(&mut t);
                w.traders[i % n] = t;
            }
            Op::Crank(i) => {
                let n = w.traders.len();
                let mut t = w.traders[i % n];
                w.crank(&mut t);
                w.traders[i % n] = t;
            }
            Op::Open(long, units) => {
                let mut e = account(seed);
                seed += 1;
                // Enough for the 10% IM without positive credit, never more than ~12%.
                w.deposit(&mut e, (units * u128::from(w.price())) / 8 + 1);
                let mut maker = w.maker;
                let ok = if long {
                    w.trade(&mut e, &mut maker, units * POS_SCALE)
                } else {
                    w.trade(&mut maker, &mut e, units * POS_SCALE)
                }
                .is_ok();
                if ok {
                    w.maker = maker;
                    let a = w.asset();
                    if a.stale_account_count_long + a.stale_account_count_short != 0 {
                        stats.relaxed_admissions += 1;
                        check_admission(&w, &e, stats);
                    }
                    entrants.push(e);
                }
            }
            Op::Add(i, units) => {
                let n = entrants.len();
                let mut e = entrants[i % n];
                let mut maker = w.maker;
                let long = e.legs[0].try_to_runtime().map(|l| l.side == SideV16::Long).unwrap_or(true);
                w.deposit(&mut e, (units * u128::from(w.price())) / 8 + 1);
                let ok = if long {
                    w.trade(&mut e, &mut maker, units * POS_SCALE)
                } else {
                    w.trade(&mut maker, &mut e, units * POS_SCALE)
                }
                .is_ok();
                if ok {
                    w.maker = maker;
                    let a = w.asset();
                    if a.stale_account_count_long + a.stale_account_count_short != 0 {
                        stats.relaxed_admissions += 1;
                        check_admission(&w, &e, stats);
                    }
                }
                entrants[i % n] = e;
            }
        }
    }
    // Settle everyone and check conservation.
    w.cascade(&mut entrants);
    w.validate(&mut entrants);
    let _ = ADL_ONE;
}

fn trader() -> impl Strategy<Value = (u128, bool, u128)> {
    // capital between ~10% and ~40% of notional at PRICE: thin accounts go bankrupt on a
    // few capped adverse slots, fat ones never do.
    (1u128..=3, any::<bool>(), 105u128..=400).prop_map(|(units, long, pct10)| {
        (units * u128::from(PRICE) * pct10 / 1_000, long, units)
    })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: std::env::var("PROPTEST_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(96), .. ProptestConfig::default() })]
    #[test]
    fn v21_relaxed_admission_never_exposes_entrants_to_pre_entry_loss(
        traders in prop::collection::vec(trader(), 4..24),
        ins_long in prop_oneof![Just(0u128), 1u128..2_000, 2_000u128..2_000_000, 2_000_000u128..60_000_000],
        ins_short in prop_oneof![Just(0u128), 1u128..2_000, 2_000u128..2_000_000, 2_000_000u128..60_000_000],
        ops in prop::collection::vec(op(), 4..40),
    ) {
        let mut stats = Stats::default();
        run(traders, (ins_long, ins_short), ops, &mut stats);
    }
}

/// Deterministic sweep with a summary, so the run proves the relaxed path and the P3
/// cascade were actually exercised (non-vacuity) and that the control finds harm.
#[test]
fn v21_relaxed_admission_sweep_is_not_vacuous() {
    use proptest::strategy::ValueTree;
    use proptest::test_runner::{Config, TestRng, RngAlgorithm, TestRunner};
    let mut runner = TestRunner::new_with_rng(
        Config::default(),
        TestRng::from_seed(RngAlgorithm::ChaCha, &[7u8; 32]),
    );
    let strat = (
        prop::collection::vec(trader(), 4..24),
        prop_oneof![Just(0u128), 1u128..2_000, 2_000u128..2_000_000, 2_000_000u128..60_000_000],
        prop_oneof![Just(0u128), 1u128..2_000, 2_000u128..2_000_000, 2_000_000u128..60_000_000],
        prop::collection::vec(op(), 4..40),
    );
    let mut stats = Stats::default();
    for _ in 0..400 {
        let (t, il, is, ops) = strat.new_tree(&mut runner).unwrap().current();
        run(t, (il, is), ops, &mut stats);
    }
    eprintln!("v21 sweep: {stats:?}");
    assert!(stats.relaxed_admissions > 50, "relaxed path barely exercised: {stats:?}");
    assert!(stats.p3_checked > 20, "P3 cascade barely exercised: {stats:?}");
    // Random worlds that reach a hidden deficit at admission must exist, and the P3 control
    // (cover stripped) must find harm in some of them; the deterministic
    // `v21_hidden_deficit_is_absorbed_by_cover_not_by_entrant` pins one such world exactly.
    assert!(stats.hidden_deficit_admissions > 0, "no admission ever had a hidden deficit: {stats:?}");
    assert!(stats.control_harm > 0, "control never found harm: P3 would be vacuous: {stats:?}");
}

/// Deterministic: thin longs go underwater on a capped price slide nobody has refreshed. The
/// hidden deficit is real (control: strip the insurance and the same cascade B-charges the
/// short entrant), and with the cover in place the entrant opens and pays nothing for it.
#[test]
fn v21_hidden_deficit_is_absorbed_by_cover_not_by_entrant() {
    // 6 thin longs (10.5% of notional), 2 fat longs; the maker is short against all of them.
    let traders: Vec<(u128, bool, u128)> = (0..8)
        .map(|i| if i < 6 { (105_000, true, 1) } else { (900_000, true, 1) })
        .collect();
    let mut w = World::new(&traders, 0, 0);
    // Slide 2% per slot for 7 slots with positive funding (longs pay): ~13% adverse.
    for _ in 0..7 {
        assert!(w.accrue(-200, RATE_E9));
    }
    let asset = w.asset();
    let mut hidden_deficit = 0i128;
    for t in w.traders.iter() {
        let (_, loss) = stale_leg_loss(&asset, t).unwrap();
        let after = equity(t) - loss as i128;
        if after < 0 {
            hidden_deficit -= after;
        }
    }
    assert!(hidden_deficit > 0, "the slide must leave unrecognized bankruptcies");
    let b_long = bound(&w, SideV16::Long);
    assert!(b_long > 0);

    // The maker's short is stale too but only gained: its bound is the floor allowance.
    let b_short = bound(&w, SideV16::Short);
    assert_eq!(b_short, 2, "favourable travel adds nothing beyond rounding");
    // Insure the domain that absorbs long bankruptcies (the short domain, 1) to the bound,
    // and the long domain for the maker's rounding allowance.
    {
        let mut m = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
        m.deposit_domain_insurance_not_atomic(1, b_long).unwrap();
        m.deposit_domain_insurance_not_atomic(0, b_short).unwrap();
    }
    // A short entrant opens against a fresh long counterparty: neither has stale legs, the
    // eight longs stay stale.
    let mut cp = account(50_000);
    w.deposit(&mut cp, 10_000_000);
    let mut entrant = account(50_001);
    w.deposit(&mut entrant, 300_000);
    w.trade(&mut cp, &mut entrant, 2 * POS_SCALE)
        .expect("covered: the short entrant opens without settling the eight stale longs");
    assert_eq!(w.asset().stale_account_count_long, 8);

    let mut stats = Stats::default();
    check_admission(&w, &entrant, &mut stats);
    assert_eq!(stats.p3_checked, 1, "P3 must run on this world");
    assert_eq!(stats.control_harm, 1, "control: without the cover the entrant is charged");

    // Same world, one atom less cover: refused.
    let mut w2 = World::new(&traders, 0, 0);
    for _ in 0..7 {
        assert!(w2.accrue(-200, RATE_E9));
    }
    {
        let mut m = MarketGroupV16ViewMut::new(&mut w2.header, &mut w2.markets);
        m.deposit_domain_insurance_not_atomic(1, b_long - 1).unwrap();
        m.deposit_domain_insurance_not_atomic(0, b_short).unwrap();
    }
    let mut cp = account(50_000);
    w2.deposit(&mut cp, 10_000_000);
    let mut entrant = account(50_001);
    w2.deposit(&mut entrant, 300_000);
    assert_eq!(
        w2.trade(&mut cp, &mut entrant, 2 * POS_SCALE),
        Err(percolator::V16Error::LossStale)
    );
}
