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
#![allow(dead_code, unused_imports)]
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
fn scenario(seed: u64, perm: &[usize], scap: u128) -> (Vec<i128>, i128) {
    let mut s = seed | 1;
    let npairs = 2 + (rng(&mut s) % 3) as usize; // 2..4 pairs
    let cap = 1_000_000_000_000_000u128;
    let units: Vec<u128> = (0..npairs).map(|_| 1 + (rng(&mut s) % 4) as u128).collect();
    let pairs: Vec<(u128, u128, u128)> = units.iter().map(|u| (cap, scap, *u)).collect();
    let mut w = World::new_pairs(&pairs, 0, 0);
    let n = w.traders.len();
    let mut tr = w.traders.clone();
    let up1 = 20 + (rng(&mut s) % 100) as i64;
    let st1 = 2 + (rng(&mut s) % 8) as usize;
    for _ in 0..st1 {
        assert!(w.accrue(up1, 0));
        for a in tr.iter_mut().take(n) { assert!(w.settle(a)); }
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
    for &k in perm { let mut a = tr[k]; if !w.settle(&mut a) { return (vec![], 0); } tr[k] = a; }
    w.traders = tr.clone();
    let mut e: [PortfolioAccountV16Account; 0] = [];
    w.validate(&mut e);
    let mut out = vec![];
    for (k, a) in tr.iter_mut().enumerate().take(n) { out.push(effective_equity(&mut w, a) - if k % 2 == 1 { scap as i128 } else { cap as i128 }); }
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

fn sweep(worlds: u64, seed0: u64, scap: u128) -> Outcome {
    let (mut ran, mut orderdep, mut worst, mut gain, mut worst_total) = (0, 0, 0i128, 0, 0i128);
    for i in 0..worlds {
        let seed = seed0 * 7919 + i;
        let mut s = seed ^ 0xABCDEF;
        let mut t = seed | 1;
        let np = 2 + (rng(&mut t) % 3) as usize;
        let ps = perms(2 * np, 8, &mut s);
        let base = scenario(seed, &ps[0], scap);
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
            let r = scenario(seed, p, scap);
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
        let o = sweep(worlds, seed0, CAP);
        eprintln!("R1MULTI solvent seed {seed0}: ran {} order_dependent {} worst_gap {} gains {} worst_total {}", o.ran, o.order_dependent, o.worst_gap, o.gains, o.worst_total);
        assert_eq!(o.ran, worlds);
        assert_eq!(o.gains, 0, "never a gain");
        assert_eq!(o.worst_total, 0, "solvent: the pair is zero-sum in every order");
        assert_eq!(o.order_dependent, 0, "seed {seed0}: {} worlds depend on settle order (worst gap {})", o.order_dependent, o.worst_gap);
    }
}

/// Bankrupt losers (loser capital 420_000 against a much larger loss): the residual. Ratchet:
/// measured at `seed 3, 1500 worlds`: 26 of 954 order dependent, worst gap 29,681 (the pre-R1
/// engine: 954 of 954, 56,479; round 1: 100% / 33,142). Tighten these when it improves.
#[test]
fn r1_multi_claimant_bankrupt_losers_residual_is_bounded() {
    let o = sweep(1500, 3, 420_000);
    eprintln!("R1MULTI bankrupt: ran {} order_dependent {} worst_gap {} gains {} worst_total {}", o.ran, o.order_dependent, o.worst_gap, o.gains, o.worst_total);
    assert!(o.ran > 900);
    assert_eq!(o.gains, 0, "never a gain, bankrupt or not");
    assert!(o.order_dependent <= 30, "order-dependent worlds regressed: {}", o.order_dependent);
    assert!(o.worst_gap <= 30_000, "worst per-account order gap regressed: {}", o.worst_gap);
}
