//! R1 round 2 (security review S1): multi-claimant settle-order harness, from the reviewer's
//! `sec_r1_multi.rs`. 2-4 long/short pairs build unconverted claims in one source domain
//! (price rises), the victim (long 0) flips short, the price rises again: the victim is now a
//! loser in the SAME domain its old claim sits in, the other longs are winners, and every
//! other short is a loser whose loss is still unbooked. Eight settle permutations per world;
//! the metric is terminal EFFECTIVE (certified) equity per account, `h_max` 100_000 (no
//! bucket lapse).
//!
//! Property measured, not assumed: with solvent losers (`short_cap == cap`) the harness shows
//! ZERO order-dependent worlds. With bankrupt losers (`short_cap` far below the loss) it does
//! NOT: a victim that nets before a bankrupt loser settles prices that loser's credit as
//! backed, and it will not be. That residual is bounded and ratcheted below; do not describe the
//! property as order independent without that qualifier.
#![allow(dead_code, unused_imports, unused_mut, clippy::needless_range_loop, clippy::type_complexity)]
// Sentinel multi-claimant R1 harness
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
    /// Peer pairs: a thin long and a thin short trade each other (no rich counterparty), so
    /// BOTH sides carry thin, bankruptcy-prone legs. `maker` stays rich for entrants/reductions.
    fn new_pairs(pairs: &[(u128, u128, u128)], ins_long: u128, ins_short: u128) -> Self {
        Self::new_pairs_at(pairs, ins_long, ins_short, PRICE, RATE_E9 as u64)
    }
    fn new_pairs_at(pairs: &[(u128, u128, u128)], ins_long: u128, ins_short: u128, px: u64, cap: u64) -> Self {
        let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 100_000);
        cfg.max_abs_funding_e9_per_slot = cap;
        cfg.max_price_move_bps_per_slot = MOVE_CAP_BPS;
        cfg.initial_margin_bps = 1_000;
        cfg.maintenance_margin_bps = 500;
        // multi-slot segments and canonical paths (the wrapper's production accrual route)
        cfg.max_accrual_dt_slots = 2;
        cfg.min_funding_lifetime_slots = 2;
        let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
        let mut markets = vec![Market::new(0, EngineAssetSlotV16Account::default())];
        header
            .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, px, 1)
            .unwrap();
        let mut w = World { header, markets, maker: account(0), traders: Vec::new(), slot: 1 };
        let mut maker = w.maker;
        w.deposit(&mut maker, 1_000_000_000_000_000);
        w.maker = maker;
        for (i, &(cl, cs, units)) in pairs.iter().enumerate() {
            let mut l = account(1 + 2 * i as u32);
            let mut s = account(2 + 2 * i as u32);
            w.deposit(&mut l, cl);
            w.deposit(&mut s, cs);
            w.trade(&mut l, &mut s, units * POS_SCALE).expect("fresh pair opens");
            w.traders.push(l);
            w.traders.push(s);
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
        let a0 = *acct;
        if m.deposit_not_atomic(&mut PortfolioV16ViewMut::new(acct), amount).is_err() { *acct = a0; }
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

    /// Canonical multi-step path accrual (`accrue_asset_path_to_not_atomic`), the route the
    /// wrapper's crank and TradeCpi use: `n` one-slot steps toward a target `dp_bps` away, with
    /// the funding sign optionally flipping every step.
    fn accrue_path(&mut self, dp_bps: i64, n: u8, rate: i128, flip: bool) -> bool {
        let a = self.asset();
        let cur = a.effective_price;
        let target = (cur as i128 + cur as i128 * dp_bps as i128 / 10_000).max(1) as u64;
        let anchor = if a.raw_oracle_target_price != target { a.effective_price } else { a.fund_px_last };
        let exposed = a.oi_eff_long_q != 0 || a.oi_eff_short_q != 0;
        let n = n.clamp(1, 2) as u64;
        let mut price = cur;
        let mut rem = 0u16;
        let mut steps = Vec::new();
        for i in 0..n {
            let Ok((p, r)) = percolator::canonical_accrual_price_step_v16(price, target, anchor, MOVE_CAP_BPS, exposed, rem) else {
                return false;
            };
            steps.push(percolator::AccrualStepV16 {
                effective_price: p,
                funding_rate_e9: if flip && i % 2 == 1 { -rate } else { rate },
                price_move_remainder_before_bps_num: rem,
                price_move_remainder_after_bps_num: r,
            });
            price = p;
            rem = r;
        }
        let now = self.slot + n;
        let (h, mk) = (self.header, self.markets.clone());
        let mut m = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        if m.accrue_asset_path_to_not_atomic(0, now, target, &steps, true).is_err() {
            self.header = h;
            self.markets = mk;
            return false;
        }
        // keep the book tradable: the next segment re-targets from wherever the path ended
        m.markets[0].engine.asset.raw_oracle_target_price = V16PodU64::new(price);
        self.slot = now;
        true
    }

    fn settle(&mut self, acct: &mut PortfolioAccountV16Account) -> bool {
        let slot = self.slot;
        {
            let mut m = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
            for d in 0..2 { let _ = m.expire_source_backing_bucket_not_atomic(d, slot); }
        }
        self.refresh(acct)
    }

    fn refresh(&mut self, acct: &mut PortfolioAccountV16Account) -> bool {
        let (h, mk, a0) = (self.header, self.markets.clone(), *acct);
        let mut m = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let r = m.full_account_refresh_not_atomic(&mut PortfolioV16ViewMut::new(acct));
        let ok = r.is_ok();
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
}



fn rng(s: &mut u64) -> u64 { *s ^= *s << 13; *s ^= *s >> 7; *s ^= *s << 17; *s }

fn effective_equity(w: &mut World, a: &mut PortfolioAccountV16Account) -> i128 {
    assert!(w.refresh(a), "refresh for certification");
    let cert = a.health_cert.try_to_runtime().expect("cert decodes");
    assert!(cert.valid);
    cert.certified_equity
}

/// n pairs (long_i, short_i). Claimants build claims in the short-loss domain (price rises make
/// shorts lose, longs win: longs hold claims). Then victim V=long_0 flips short; the price rises
/// again: V is now a loser in the SAME domain its old claim sits in, other longs are winners.
/// Returns per-account effective equity for the given settle permutation (and total capital).
#[derive(Clone, Copy)]
enum Shape {
    /// every short has capital `.0`
    Uniform(u128),
    /// pair 0 (the victim's) is solvent; every OTHER pair's short has capital `.0` (bankrupt losers)
    Adversarial(u128),
}

impl Shape {
    fn short_cap(self, pair: usize) -> u128 {
        match self {
            Shape::Uniform(c) => c,
            Shape::Adversarial(c) => if pair == 0 { CAP } else { c },
        }
    }
}

/// S7: reconcile the pending-credit counter with the claim stock. After every settle: for each
/// source domain the counter's positive part never exceeds the claims actually held by the
/// accounts in that domain (a credit that is not in the stock cannot be pending). At quiescence
/// (every account settled) nothing is in flight: the counter is at most the floor/ceil atoms of
/// the settlements (it drifts negative by netted-against-debt credits and rounding, never
/// positive).
fn reconcile_pending(w: &World, accounts: &[PortfolioAccountV16Account], quiescent: bool) {
    for d in 0..2usize {
        let stock: u128 = accounts
            .iter()
            .flat_map(|a| a.source_domains.iter())
            .filter(|sd| sd.is_occupied() && sd.domain.get() as usize == d)
            .map(|sd| sd.source_claim_bound_num.get())
            .sum();
        let slot = &w.markets[0].engine;
        let pend = if d == 0 { slot.kf_pending_credit_long.get() } else { slot.kf_pending_credit_short.get() };
        assert!(pend <= stock as i128, "domain {d}: pending credit {pend} exceeds the claim stock {stock}");
        if quiescent {
            assert!(pend <= accounts.len() as i128 * 1_000_000_000_000, "domain {d}: stale positive pending credit {pend} at quiescence");
        }
    }
}

/// `resolved`: the market is resolved with the loss unsettled and the accounts are CLOSED in the
/// permutation order (payout - initial capital) instead of settled in Live.
fn scenario(seed: u64, perm: &[usize], shape: Shape, resolved: bool) -> (Vec<i128>, i128) {
    let mut s = seed | 1;
    let npairs = 2 + (rng(&mut s) % 3) as usize; // 2..4 pairs
    let cap = 1_000_000_000_000_000u128;
    let units: Vec<u128> = (0..npairs).map(|_| 1 + (rng(&mut s) % 4) as u128).collect();
    let pairs: Vec<(u128, u128, u128)> = units.iter().enumerate().map(|(i, u)| (cap, shape.short_cap(i), *u)).collect();
    let mut w = World::new_pairs(&pairs, 0, 0);
    let n = w.traders.len();
    let mut tr = w.traders.clone();
    let up1 = 20 + (rng(&mut s) % 100) as i64;
    let st1 = 2 + (rng(&mut s) % 8) as usize;
    for _ in 0..st1 {
        assert!(w.accrue(up1, 0));
        for k in 0..n {
            let mut a = tr[k];
            assert!(w.settle(&mut a));
            tr[k] = a;
            reconcile_pending(&w, &tr, false);
        }
        reconcile_pending(&w, &tr, true);
    }
    // victim = trader 0 (long). flip: counterparty short_0 (trader 1) buys 2x victim size.
    let flip = 2 * units[0] * POS_SCALE;
    let (mut v, mut c) = (tr[0], tr[1]);
    if w.trade(&mut c, &mut v, flip).is_err() { return (vec![], 0); }
    tr[0] = v; tr[1] = c;
    // optionally more unconverted claimants flip too (random second victim)
    let up2 = 20 + (rng(&mut s) % 120) as i64;
    let st2 = 1 + (rng(&mut s) % 5) as usize;
    for _ in 0..st2 { if !w.accrue(up2, 0) { return (vec![], 0); } }
    let init = |k: usize| -> i128 { if k % 2 == 1 { shape.short_cap(k / 2) as i128 } else { cap as i128 } };
    if resolved {
        let slot = w.slot + 1;
        let mut m = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
        m.resolve_market_not_atomic(slot).unwrap();
        let mut paid = vec![0u128; n];
        let mut closed = vec![false; n];
        for _ in 0..60 {
            for &k in perm {
                if closed[k] { continue; }
                for d in 0..2 { let _ = m.expire_source_backing_bucket_not_atomic(d, slot); }
                if let Ok(percolator::ResolvedCloseOutcomeV16::Closed { payout }) = m.close_resolved_account_not_atomic(&mut PortfolioV16ViewMut::new(&mut tr[k]), 0) { paid[k] = payout; closed[k] = true; }
            }
            if closed.iter().all(|c| *c) { break; }
        }
        if !closed.iter().all(|c| *c) { return (vec![], 0); }
        return ((0..n).map(|k| paid[k] as i128 - init(k)).collect(), 0);
    }
    for &k in perm {
        let mut a = tr[k];
        if !w.settle(&mut a) { return (vec![], 0); }
        tr[k] = a;
        reconcile_pending(&w, &tr, false);
    }
    reconcile_pending(&w, &tr, true);
    w.traders = tr.clone();
    let mut e: [PortfolioAccountV16Account; 0] = [];
    w.validate(&mut e);
    let mut out = vec![];
    for (k, a) in tr.iter_mut().enumerate().take(n) { out.push(effective_equity(&mut w, a) - init(k)); }
    (out, 0)
}

fn perms(n: usize, count: usize, s: &mut u64) -> Vec<Vec<usize>> {
    let mut out = vec![(0..n).collect::<Vec<_>>(), (0..n).rev().collect::<Vec<_>>()];
    while out.len() < count {
        let mut p: Vec<usize> = (0..n).collect();
        for i in (1..n).rev() { let j = (rng(s) % (i as u64 + 1)) as usize; p.swap(i, j); }
        out.push(p);
    }
    out
}


struct Outcome {
    ran: u64,
    order_dependent: u64,
    worst_gap: i128,
    gains: u64,
    worst_total: i128,
}

fn sweep(worlds: u64, seed0: u64, shape: Shape, resolved: bool) -> Outcome {
    let (mut ran, mut orderdep, mut worst, mut gain, mut worst_total) = (0, 0, 0i128, 0, 0i128);
    for i in 0..worlds {
        let seed = seed0 * 7919 + i;
        let mut s = seed ^ 0xABCDEF;
        let mut t = seed | 1;
        let np = 2 + (rng(&mut t) % 3) as usize;
        let ps = perms(2 * np, 8, &mut s);
        let base = scenario(seed, &ps[0], shape, resolved);
        if base.0.is_empty() {
            continue;
        }
        ran += 1;
        let mut dep = false;
        let tot: i128 = base.0.iter().sum();
        if tot > 0 {
            gain += 1;
        }
        worst_total = worst_total.min(tot);
        for p in &ps[1..] {
            let r = scenario(seed, p, shape, resolved);
            if r.0.is_empty() {
                continue;
            }
            let tot2: i128 = r.0.iter().sum();
            if tot2 > 0 {
                gain += 1;
            }
            worst_total = worst_total.min(tot2);
            let d = r.0.iter().zip(&base.0).map(|(a, b)| (a - b).abs()).max().unwrap();
            if d > 8 {
                dep = true;
            }
            worst = worst.max(d);
        }
        if dep {
            orderdep += 1;
        }
    }
    Outcome { ran, order_dependent: orderdep, worst_gap: worst, gains: gain, worst_total }
}

const CAP: u128 = 1_000_000_000_000_000;

/// Solvent losers: the settle order must not matter.
#[test]
fn r1_multi_claimant_solvent_losers_are_settle_order_independent() {
    let worlds: u64 = std::env::var("WORLDS").ok().and_then(|v| v.parse().ok()).unwrap_or(400);
    for seed0 in [3u64, 11] {
        let o = sweep(worlds, seed0, Shape::Uniform(CAP), false);
        eprintln!("R1MULTI solvent seed {seed0}: ran {} order_dependent {} worst_gap {} gains {} worst_total {}", o.ran, o.order_dependent, o.worst_gap, o.gains, o.worst_total);
        assert_eq!(o.ran, worlds);
        assert_eq!(o.gains, 0, "never a gain");
        assert_eq!(o.worst_total, 0, "solvent: the pair is zero-sum in every order");
        assert_eq!(o.order_dependent, 0, "seed {seed0}: {} worlds depend on settle order (worst gap {})", o.order_dependent, o.worst_gap);
    }
}

/// The measured residual, ratcheted. `(name, shape, resolved, seed, worlds, max order-dependent
/// worlds, max per-account gap, pre-R1 gap for reference)`. Measured on 3b77d43c+round 3 (the
/// pre-R1 engine, same worlds: Live 100% order dependent everywhere, gaps 50,769 / 144,500 /
/// 144,500). Resolved close is order dependent in the pre-R1 engine too (the receipt/bucket-expiry
/// order of the resolved payout, spec "DISTRIBUTION CHARACTERIZATION"); R1 only lowers the gap.
/// Tighten when it improves; do not call any of this order independent.
#[test]
fn r1_multi_claimant_residual_ratchets() {
    const CASES: &[(&str, Shape, bool, u64, u64, u64, i128, i128)] = &[
        ("live bankrupt losers", Shape::Uniform(420_000), false, 3, 800, 14, 12_200, 50_769),
        ("live bankrupt losers", Shape::Uniform(420_000), false, 7, 800, 26, 21_800, 67_323),
        ("live adversarial (victim solvent, other losers bankrupt)", Shape::Adversarial(420_000), false, 3, 800, 39, 63_100, 144_500),
        ("live adversarial", Shape::Adversarial(420_000), false, 7, 800, 38, 21_800, 128_035),
        ("resolved solvent", Shape::Uniform(CAP), true, 3, 800, 800, 116_400, 144_500),
        ("resolved bankrupt losers", Shape::Uniform(420_000), true, 3, 800, 515, 169_800, 169_790),
        ("resolved adversarial", Shape::Adversarial(420_000), true, 3, 800, 800, 466_700, 466_652),
    ];
    for &(name, shape, resolved, seed0, worlds, max_dep, max_gap, _pre) in CASES {
        let o = sweep(worlds, seed0, shape, resolved);
        eprintln!("R1MULTI {name} seed {seed0}: ran {} order_dependent {} worst_gap {} gains {} worst_total {}", o.ran, o.order_dependent, o.worst_gap, o.gains, o.worst_total);
        assert!(o.ran > 400, "{name}: too few worlds ran");
        assert_eq!(o.gains, 0, "{name}: never a gain");
        assert!(o.order_dependent <= max_dep, "{name}: order-dependent worlds regressed: {} > {max_dep}", o.order_dependent);
        assert!(o.worst_gap <= max_gap, "{name}: worst gap regressed: {} > {max_gap}", o.worst_gap);
    }
}

#[test]
fn r1_multi_measure() {
    if std::env::var("MEASURE").is_err() { return; }
    let worlds: u64 = std::env::var("WORLDS").ok().and_then(|v| v.parse().ok()).unwrap_or(800);
    for (name, shape, resolved) in [
        ("live solvent", Shape::Uniform(CAP), false),
        ("live bankrupt 420k", Shape::Uniform(420_000), false),
        ("live adversarial 420k", Shape::Adversarial(420_000), false),
        ("resolved solvent", Shape::Uniform(CAP), true),
        ("resolved bankrupt 420k", Shape::Uniform(420_000), true),
        ("resolved adversarial 420k", Shape::Adversarial(420_000), true),
    ] {
        for seed0 in [3u64, 7] {
            let o = sweep(worlds, seed0, shape, resolved);
            println!("MULTI {name} seed {seed0}: ran {} order_dependent {} worst_gap {} gains {} worst_total {}", o.ran, o.order_dependent, o.worst_gap, o.gains, o.worst_total);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// S8 mixed domain: a liened poor flipper (pair 0: capital 330,000, sells 7 of 3) AND a rich,
// UNliened flipper (pair 1) net losses in the same loser domain. With the pricing branch chosen
// on the DOMAIN's lien state the rich flipper is priced like everyone else; with the branch
// chosen on the account's own entry it would be priced neutral. Measured against the pre-R1
// engine (same 300-seed sweep, 173 worlds ran): see the constants.
// ---------------------------------------------------------------------------------------------
fn scenario_mixed(seed: u64, perm: &[usize]) -> Vec<i128> {
    let mut s = seed | 1;
    let npairs = 2 + (rng(&mut s) % 3) as usize;
    let cap = CAP;
    let mut units: Vec<u128> = (0..npairs).map(|_| 1 + (rng(&mut s) % 4) as u128).collect();
    units[0] = 3;
    let vcap = 330_000u128;
    let pairs: Vec<(u128, u128, u128)> = units.iter().enumerate().map(|(i, u)| (if i == 0 { vcap } else { cap }, cap, *u)).collect();
    let mut w = World::new_pairs(&pairs, 0, 0);
    let n = w.traders.len();
    let mut tr = w.traders.clone();
    let up1 = 20 + (rng(&mut s) % 100) as i64;
    let st1 = 2 + (rng(&mut s) % 8) as usize;
    for _ in 0..st1 {
        assert!(w.accrue(up1, 0));
        for k in 0..n { let mut a = tr[k]; assert!(w.settle(&mut a)); tr[k] = a; }
    }
    let (mut v, mut c) = (tr[0], tr[1]);
    if w.trade(&mut c, &mut v, 7 * POS_SCALE).is_err() { return vec![]; }
    tr[0] = v; tr[1] = c;
    if n >= 4 {
        let (mut v2, mut c2) = (tr[2], tr[3]);
        if w.trade(&mut c2, &mut v2, 2 * units[1] * POS_SCALE).is_err() { return vec![]; }
        tr[2] = v2; tr[3] = c2;
    }
    let up2 = 20 + (rng(&mut s) % 120) as i64;
    let st2 = 1 + (rng(&mut s) % 5) as usize;
    for _ in 0..st2 { if !w.accrue(up2, 0) { return vec![]; } }
    for &k in perm { let mut a = tr[k]; if !w.settle(&mut a) { return vec![]; } tr[k] = a; }
    w.traders = tr.clone();
    let mut e: [PortfolioAccountV16Account; 0] = [];
    w.validate(&mut e);
    let mut out = vec![];
    for k in 0..n { let mut a = tr[k]; out.push(effective_equity(&mut w, &mut a) - if k == 0 { vcap as i128 } else { cap as i128 }); }
    out
}

#[test]
fn mixed_liened_and_rich_flippers_ratchets() {
    let (mut worlds, mut sum_worst, mut spread, mut gains, mut rich_worst) = (0u64, 0i128, 0i128, 0u64, 0i128);
    let mut floors = R1_ACCOUNT_WORST_BY_WORLD.iter();
    for i in 0..300u64 {
        let seed = 3 * 7919 + i;
        let mut t = seed | 1;
        let np = 2 + (rng(&mut t) % 3) as usize;
        let mut s2 = seed ^ 0xABCDEF;
        let ps = perms(2 * np, 8, &mut s2);
        let rows: Vec<Vec<i128>> = ps.iter().map(|p| scenario_mixed(seed, p)).take_while(|r| !r.is_empty()).collect();
        if rows.len() != ps.len() { continue; }
        worlds += 1;
        // #250 must leave every account at or above its R1 (91911d5b) worst order.
        let (off, floor) = floors.next().expect("same 173 worlds as 91911d5b");
        assert_eq!(seed, 3 * 7919 + off, "world set changed");
        for k in 0..rows[0].len() {
            let worst = rows.iter().map(|r| r[k]).min().unwrap();
            assert!(worst >= floor[k], "world {seed} account {k}: {worst} below the R1 worst {}", floor[k]);
        }
        if rows[0].len() > 3 { rich_worst += rows.iter().map(|r| r[2]).min().unwrap(); }
        for k in 0..rows[0].len() { sum_worst += rows.iter().map(|r| r[k]).min().unwrap(); }
        let tot: Vec<i128> = rows.iter().map(|r| r.iter().sum()).collect();
        spread += tot.iter().max().unwrap() - tot.iter().min().unwrap();
        if *tot.iter().max().unwrap() > 8 { gains += 1; }
    }
    println!("MIXED worlds {worlds} sum_account_worst {sum_worst} total_spread {spread} gains {gains} rich_flipper_worst {rich_worst}");
    assert_eq!(worlds, 173);
    assert_eq!(gains, 0);
    // pre-R1 engine: sum of per-account worst -14,995,914, total spread 4,160,314 (all 173 worlds
    // order dependent). R1 at 91911d5b: -11,221,444 / 1,690,883 (the stored rate still
    // understated unliened claims while a lien sat in the domain).
    // Upstream #250 (+ 7b32eef2): a lien's face leaves the rate denominator, so nothing is
    // burned. Every world is exactly zero-sum and order independent, and every account is at or
    // above its 91911d5b worst order in all 173 worlds (losing shorts lose less, longs gain more;
    // asserted per account above against R1_ACCOUNT_WORST_BY_WORLD).
    assert_eq!(sum_worst, 0, "per-account worst orders regressed: {sum_worst}");
    assert_eq!(spread, 0, "order spread regressed: {spread}");
    // The rich UNliened flipper in the locked domain: pre-R1 worst 8,599,318 summed, R1 10.77M on
    // the domain's protective branch (10,773,008 at 91911d5b; 11.31M neutral). With #250 it is
    // 15,427,446: its full mark-to-market, not a windfall, since every world is zero-sum and its
    // counterparties are above their old worst (the extra is face that is no longer burned).
    // NOTE: these worlds no longer tell the S8 branches apart. Choosing the branch on the
    // account's own entry gives the same numbers, because valid-lien face is out of the stored
    // rate's denominator. The branches still differ for impaired face, which this fixture has none of.
    assert!(rich_worst >= 8_599_318, "rich flipper below the pre-R1 worst: {rich_worst}");
    assert_eq!(rich_worst, 15_427_446, "rich flipper pricing moved: {rich_worst}");
}

/// (world offset from seed 3 * 7919, each account's worst order) on 91911d5b, the 173 worlds.
const R1_ACCOUNT_WORST_BY_WORLD: [(u64, &[i128]); 173] = [
    (1, &[131255, -152853, 173963, -217825, 66588, -77157, 66588, -77157]),
    (2, &[131039, -152853, 174915, -217825, 66588, -77157, 66588, -77157]),
    (9, &[225916, -255348, 285776, -346110]), (10, &[225771, -255369, 285776, -346131]),
    (13, &[124773, -145338, 172281, -211278, 143031, -161512, 71515, -80756]),
    (14, &[124800, -145525, 172334, -211465, 142520, -161512, 71260, -80756]),
    (15, &[277795, -305089, 183280, -208714, 434177, -485016]),
    (16, &[277795, -305073, 183249, -208706, 434391, -485016]),
    (17, &[111701, -135883, 138715, -191042]), (18, &[111954, -135883, 136951, -191042]),
    (21, &[186375, -212740, 231823, -289338]), (22, &[186215, -212740, 231823, -289338]),
    (23, &[-7217, 4027, -20115, -12542]), (24, &[-7217, 4027, -20115, -12542]),
    (27, &[83297, -95275, 44999, -65909, 31064, -41316, 62128, -82632]),
    (28, &[83297, -95289, 45038, -65916, 31033, -41316, 62067, -82632]),
    (29, &[18157, -27619, 18232, -62340]), (30, &[18157, -28891, 18232, -63612]),
    (33, &[113236, -137810, 166945, -208186, 242402, -273075]),
    (34, &[113443, -137669, 166945, -208045, 242867, -273075]),
    (37, &[-5796, 3597, -3107, -26764, 216124, -216124]),
    (38, &[-5796, 3597, -3107, -26764, 216124, -216124]),
    (39, &[142027, -167922, 89409, -116832, 122243, -149386]),
    (40, &[142027, -167927, 89230, -116834, 122222, -149386]),
    (41, &[106572, -125060, 159611, -191753, 318158, -349084, 159079, -174542]),
    (42, &[106492, -125036, 160150, -191729, 318257, -349084, 159128, -174542]),
    (45, &[97913, -116431, 139327, -174171, 262450, -294736]),
    (46, &[97845, -117059, 140143, -174799, 259544, -294736]),
    (51, &[146725, -169367, 147269, -180202, 217279, -250131]),
    (52, &[146665, -169319, 146501, -180167, 217518, -250131]),
    (53, &[158701, -201534, 54757, -73033, 93786, -111474, 93786, -111474]),
    (54, &[158613, -200972, 55049, -72893, 94536, -111474, 94536, -111474]),
    (57, &[69011, -98131, 14997, -34498]), (58, &[69011, -98468, 14735, -34583]),
    (65, &[182349, -213433, 66255, -78980, 116339, -128432, 465356, -513728]),
    (66, &[182143, -214260, 65532, -79186, 115315, -128432, 461263, -513728]),
    (67, &[-17066, 16588, -16086, -8220, 209424, -209424]),
    (68, &[-17066, 16588, -16086, -8220, 209424, -209424]),
    (69, &[21862, -38736, 11401, -21138, 136540, -143506, 136540, -143506]),
    (70, &[21862, -41135, 11427, -21738, 133222, -143506, 133222, -143506]),
    (71, &[-10952, 4223, 11166, -29640, 310424, -310424, 232818, -232818]),
    (72, &[-10952, 4223, 11166, -29640, 310424, -310424, 232818, -232818]),
    (75, &[-26163, 3883, -5153, -41760]), (76, &[-26163, 3883, -5153, -41760]),
    (77, &[95652, -133882, 33533, -49836, 138651, -169022]),
    (78, &[95652, -133965, 32961, -49857, 138467, -169022]),
    (79, &[17743, -22391, 16412, -38601, 175881, -184224]),
    (80, &[17743, -21949, 16412, -38270, 177079, -184224]),
    (81, &[-67633, 63081, -16723, 7469, 147758, -147758, 221637, -221637]),
    (82, &[-67633, 63081, -14040, 7469, 147758, -147758, 221637, -221637]),
    (85, &[-68663, 61248, -13859, 5248, 257280, -257280, 257280, -257280]),
    (86, &[-68663, 61248, -13859, 5248, 257280, -257280, 257280, -257280]),
    (89, &[50813, -74524, 6703, -27312]), (90, &[50813, -75578, 6817, -27575]),
    (93, &[74190, -96376, 23592, -35511, 143233, -174012]),
    (94, &[74190, -97659, 24092, -35832, 138710, -174012]),
    (95, &[105017, -152477, 122463, -175889]), (96, &[105094, -153940, 116450, -176986]),
    (97, &[-10179, 5973, -2561, -6668, 175866, -175866, 117244, -117244]),
    (98, &[-10179, 5973, -2561, -6668, 175866, -175866, 117244, -117244]),
    (99, &[125334, -152210, 110840, -157728]), (100, &[125394, -152048, 111657, -157606]),
    (105, &[-200347, 197063, -62460, 42090]), (106, &[-200347, 197063, -62460, 42090]),
    (115, &[64043, -81898, 80085, -102243, 210015, -230163, 210015, -230163]),
    (116, &[64043, -80290, 80236, -101037, 214165, -230163, 214165, -230163]),
    (117, &[57830, -73720, 4655, -25717]), (118, &[57830, -73720, 4655, -25717]),
    (119, &[77213, -82523, 67422, -85874, 89924, -107997, 119898, -143996]),
    (120, &[77213, -82511, 67560, -85865, 90006, -107997, 120008, -143996]),
    (123, &[137248, -159175, 136236, -169401, 203482, -235599]),
    (124, &[137388, -159151, 137044, -169383, 203603, -235599]),
    (129, &[1249, -12620, 6959, -13703, 69850, -71253, 279403, -285012]),
    (130, &[1249, -10332, 4509, -13131, 71253, -71253, 285012, -285012]),
    (133, &[-20816, 14383, -16505, -4056]), (134, &[-20816, 14383, -16505, -4056]),
    (135, &[49522, -58760, 56378, -71504, 184767, -202056, 138575, -151542]),
    (136, &[49522, -58602, 55281, -71385, 185339, -202056, 139004, -151542]),
    (137, &[-6811, -21718, -4257, -18453]), (138, &[-6811, -21718, -4257, -18453]),
    (143, &[94554, -112601, 95126, -123105, 218792, -255156]),
    (144, &[94554, -113236, 95126, -123581, 215487, -255156]),
    (145, &[45043, -63050, 18162, -27184, 119927, -132136, 179890, -198204]),
    (146, &[45043, -62190, 18112, -26969, 121321, -132136, 181982, -198204]),
    (147, &[204055, -243802, 216206, -264882, 117683, -133241]),
    (148, &[203758, -244686, 213418, -265545, 116448, -133241]),
    (149, &[70875, -123588, 29514, -49474]), (150, &[70875, -130534, 27248, -51211]),
    (153, &[64683, -78872, 7036, -27052]), (154, &[64683, -78727, 7016, -27016]),
    (155, &[-111121, 102107, -91922, 49773]), (156, &[-111121, 102107, -91922, 49773]),
    (157, &[204650, -234916, 68767, -82676, 291303, -332220, 97101, -110740]),
    (158, &[204748, -234841, 69083, -82657, 291717, -332220, 97239, -110740]),
    (159, &[17057, -37487, 32501, -69498]), (160, &[17057, -37701, 32446, -69659]),
    (165, &[-105706, 75408, -28154, 5188]), (166, &[-105706, 75408, -28154, 5188]),
    (167, &[137559, -166884, 145665, -183803, 171084, -194924]),
    (168, &[137597, -167243, 145458, -184072, 170177, -194924]),
    (169, &[109274, -182009, 43245, -69449]), (170, &[109274, -184504, 41366, -70073]),
    (171, &[-98139, 78412, -64323, 18600]), (172, &[-98139, 78412, -64323, 18600]),
    (173, &[102855, -121473, 34568, -44163, 235175, -271588, 117587, -135794]),
    (174, &[102833, -121356, 34668, -44134, 235800, -271588, 117900, -135794]),
    (175, &[70868, -114300, 114776, -158865]), (176, &[70868, -129774, 103960, -170470]),
    (177, &[-76573, 71870, -18869, 9604, 306048, -306048, 76512, -76512]),
    (178, &[-76573, 71870, -18869, 9604, 306048, -306048, 76512, -76512]),
    (179, &[72018, -106259, 105230, -166666]), (180, &[72018, -103122, 114311, -163529]),
    (185, &[43156, -56739, 19814, -43743, 33321, -41908]),
    (186, &[43156, -56632, 18299, -43689, 33420, -41908]), (189, &[-74715, 65183, -48874, 15360]),
    (190, &[-74715, 65183, -48874, 15360]),
    (191, &[-29326, 13809, 19005, -55396, 373683, -373683]),
    (192, &[-29326, 13809, 19005, -55396, 373683, -373683]),
    (193, &[-66539, 44557, -24042, -7136, 114090, -114090]),
    (194, &[-66539, 44557, -24042, -7136, 114090, -114090]),
    (195, &[114509, -127444, 156814, -183700, 244412, -270812, 183309, -203109]),
    (196, &[114571, -127535, 156814, -183791, 243875, -270812, 182906, -203109]),
    (205, &[-82901, 80726, -34953, 25372, 145300, -145300, 217950, -217950]),
    (206, &[-82901, 80726, -41771, 25372, 145300, -145300, 217950, -217950]),
    (209, &[51584, -64123, 15685, -46475]), (210, &[51584, -64123, 15798, -46475]),
    (213, &[81946, -109208, 43032, -76476]), (214, &[81946, -108802, 43032, -76273]),
    (215, &[53268, -80333, 102583, -144376]), (216, &[53268, -82891, 102583, -146934]),
    (219, &[69686, -79602, 91157, -116997, 40774, -46731, 163096, -186924]),
    (220, &[69686, -79665, 90333, -117060, 40695, -46731, 162780, -186924]),
    (223, &[83481, -133223, 154930, -229876]), (224, &[83481, -123307, 165021, -219960]),
    (225, &[-19892, 15345, -12219, -8520, 181530, -181530]),
    (226, &[-19892, 15345, -12219, -8520, 181530, -181530]),
    (227, &[110136, -145604, 179885, -231894, 209034, -236712]),
    (228, &[110126, -143822, 176656, -230112, 212322, -236712]),
    (233, &[9057, -15801, -11235, -22825]), (234, &[9057, -15119, -11235, -22484]),
    (237, &[-22460, 8925, 8112, -25666, 430724, -430724, 107681, -107681]),
    (238, &[-22460, 8925, 8112, -25666, 430724, -430724, 107681, -107681]),
    (239, &[82835, -102776, 134425, -168082, 166094, -181504, 332188, -363008]),
    (240, &[82835, -101785, 135594, -167091, 167861, -181504, 335722, -363008]),
    (247, &[208007, -231991, 284292, -327623, 300754, -332409]),
    (248, &[207695, -232348, 282340, -327980, 298801, -332409]),
    (249, &[8133, -29132, 16433, -42808, 91564, -91564]),
    (250, &[8133, -29132, 16433, -42808, 91564, -91564]),
    (251, &[3046, -16529, 46195, -76568, 302808, -302808]),
    (252, &[3046, -16529, 46195, -76568, 302808, -302808]),
    (253, &[71082, -95100, 42784, -69231, 42717, -54524]),
    (254, &[71082, -95271, 42784, -69316, 42509, -54524]),
    (257, &[144585, -169643, 83796, -115025, 48703, -64224]),
    (258, &[144555, -169640, 83880, -115024, 48718, -64224]),
    (263, &[67237, -105037, 112278, -174593]), (264, &[67237, -105037, 112278, -174593]),
    (265, &[3570, -10314, -11988, -21120]), (266, &[3570, -10314, -11988, -21120]),
    (271, &[44948, -61387, 54665, -97636]), (272, &[44948, -59411, 57806, -95660]),
    (273, &[69088, -112481, 56768, -90065]), (274, &[69088, -110241, 56768, -88945]),
    (275, &[109947, -131423, 130330, -182921]), (276, &[109947, -131205, 131560, -182703]),
    (277, &[7396, -26603, 28205, -47538, 339531, -339531]),
    (278, &[7396, -28566, 27895, -48520, 335805, -339531]),
    (283, &[164964, -208642, 241705, -314075]), (284, &[164964, -208642, 241705, -314075]),
    (285, &[12075, -22162, 9967, -29599, 174703, -179733]),
    (286, &[12075, -20766, 9967, -28901, 177445, -179733]),
    (297, &[126764, -174011, 83316, -125618]), (298, &[127106, -172484, 85522, -124854]),
    (299, &[97763, -130210, 194035, -234658, 285167, -307698, 427750, -461547]),
];
