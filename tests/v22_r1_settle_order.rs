//! R1 (security review `security-review-v22-leg-remainders-2026-10-06.md`, finding R1):
//! account equity must not depend on WHICH ACCOUNT a permissionless cranker settles first.
//!
//! Mechanism. A loss netted against the loser's own positive claim burns that claim's face at
//! the source domain's credit rate. When the loss domain is the same domain the claim sits in,
//! and a winning counterparty has already been credited, the rate is transiently `< 1` (the
//! winner's claim is in `positive_claim_bound_num`, the loser's loss that backs it is not yet
//! booked). The burn then costs `loss / r` face, the booking lands, the rate returns to 1, and
//! the surplus face is gone. Round 1 priced the burn at the victim's own post-booking rate;
//! round 2 (`source_credit_netting_rate`, fed by the per-domain `kf_pending_credit`) also removes
//! the credit of OTHER winners whose losers have not settled. Measured, not assumed: the
//! scenarios here are order independent; bankrupt losers in a multi-claimant domain are not
//! (see `v22_r1_multi_claimant.rs`).
//!
//! Test shape: two traders; `L` earns a long claim, flips short, the price rises, and the new
//! winner `S` is settled BEFORE or AFTER the victim `L`. Effective equity (the certified
//! haircut equity, `capital + realizable support`) must be identical in both orders and the
//! pair must be zero-sum up to the floor atoms. All markets here use a freshness horizon far
//! longer than the world (`h_max` 100_000), which isolates this mechanism from backing-bucket
//! lapse (an intended, separate decay of unconverted claims).
#![allow(dead_code, unused_imports, clippy::type_complexity)]
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


const INIT: i128 = 2_000_000_000_000_000;

/// Effective equity the engine itself certifies: `capital + realizable (haircut) support of the
/// positive PnL`. Read from a fresh health certificate, so it uses no test-only engine API.
fn effective_equity(w: &mut World, a: &mut PortfolioAccountV16Account) -> i128 {
    assert!(w.refresh(a), "refresh for certification");
    let cert = a.health_cert.try_to_runtime().expect("cert decodes");
    assert!(cert.valid, "certificate valid after refresh");
    cert.certified_equity
}

/// `L` builds a long claim (`steps1` slots of `up1` bps), flips short (trade), the price rises
/// (`steps2` slots of `up2` bps) and the winner `S` / victim `L` are settled in the given order.
/// Returns (L effective equity, S effective equity) after a final certification of both.
fn scenario(winner_first: bool, up1: i64, steps1: usize, up2: i64, steps2: usize) -> (i128, i128) {
    let mut w = World::new_pairs(&[(1_000_000_000_000_000, 1_000_000_000_000_000, 3)], 0, 0);
    let mut l = w.traders[0];
    let mut s = w.traders[1];
    for _ in 0..steps1 {
        assert!(w.accrue(up1, 0));
        assert!(w.settle(&mut l) && w.settle(&mut s));
    }
    assert!(l.pnl.get() > 0, "the victim holds a source-backed claim");
    // L flips from long 3 to short 3 (S buys 6 from L)
    w.trade(&mut s, &mut l, 6 * POS_SCALE).expect("flip");
    for _ in 0..steps2 {
        assert!(w.accrue(up2, 0));
    }
    if winner_first {
        assert!(w.settle(&mut s) && w.settle(&mut l));
    } else {
        assert!(w.settle(&mut l) && w.settle(&mut s));
    }
    let mut e: [PortfolioAccountV16Account; 0] = [];
    w.traders[0] = l;
    w.traders[1] = s;
    w.validate(&mut e);
    let el = effective_equity(&mut w, &mut l);
    let es = effective_equity(&mut w, &mut s);
    (el - 1_000_000_000_000_000, es - 1_000_000_000_000_000)
}

const SCENARIOS: &[(i64, usize, i64, usize)] = &[
    (100, 5, 100, 2),  // claim 5.0%, loss 2.0%
    (100, 5, 150, 3),  // loss exhausts the claim and reaches capital
    (100, 10, 100, 4),
    (50, 20, 100, 5),
    (100, 5, 100, 5),
    (50, 2, 50, 2),    // 1% / 1%
];

#[test]
fn r1_netting_is_independent_of_which_account_is_settled_first() {
    for &(up1, s1, up2, s2) in SCENARIOS {
        let wf = scenario(true, up1, s1, up2, s2);
        let lf = scenario(false, up1, s1, up2, s2);
        assert_eq!(
            wf, lf,
            "claim {up1}x{s1} loss {up2}x{s2}: effective equity (victim, winner) differs by settle order: winner-first {wf:?} victim-first {lf:?}"
        );
        // zero-sum: floor atoms only (at most a few)
        assert!(
            (wf.0 + wf.1).abs() <= 4,
            "claim {up1}x{s1} loss {up2}x{s2}: pair destroyed {} atoms",
            wf.0 + wf.1
        );
    }
}

/// A non-flipping victim (claim in the OTHER domain than the one its loss books into) is not
/// affected by the order either, before or after the fix: guards that the fix only reprices the
/// loss domain's own claims.
#[test]
fn r1_cross_domain_victim_is_order_independent() {
    for &(up1, s1, dn, s2) in &[(100i64, 5usize, -100i64, 2usize), (100, 5, -150, 3), (50, 10, -100, 5)] {
        let run = |winner_first: bool| {
            let mut w = World::new_pairs(&[(1_000_000_000_000_000, 1_000_000_000_000_000, 3)], 0, 0);
            let mut l = w.traders[0];
            let mut s = w.traders[1];
            for _ in 0..s1 {
                assert!(w.accrue(up1, 0));
                assert!(w.settle(&mut l) && w.settle(&mut s));
            }
            // L stays long and now loses as the price falls; the winner is S (short)
            for _ in 0..s2 {
                assert!(w.accrue(dn, 0));
            }
            if winner_first {
                assert!(w.settle(&mut s) && w.settle(&mut l));
            } else {
                assert!(w.settle(&mut l) && w.settle(&mut s));
            }
            let (el, es) = (effective_equity(&mut w, &mut l), effective_equity(&mut w, &mut s));
            (el - 1_000_000_000_000_000, es - 1_000_000_000_000_000)
        };
        let (wf, lf) = (run(true), run(false));
        assert_eq!(wf, lf, "cross-domain case {up1}x{s1} then {dn}x{s2}");
        assert!((wf.0 + wf.1).abs() <= 4, "cross-domain pair destroyed {}", wf.0 + wf.1);
    }
}

// ---------------------------------------------------------------------------------------------
// Differential: random price/funding/trade worlds, per-slot settling vs lazy settling, and both
// account orders. Legs are bit-identical across cadences (v22 leg remainders); with R1 fixed the
// effective equity is too, up to the floor atoms.
// ---------------------------------------------------------------------------------------------

fn rng(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}

#[derive(Clone, Copy, Debug)]
enum Op {
    Acc(i64, i128),
    Path(i64, u8, i128, bool),
    Open(u128),
    Rev(u128),
}

fn gen_ops(seed: u64, n: usize) -> Vec<Op> {
    let mut s = seed | 1;
    (0..n)
        .map(|_| {
            let r = rng(&mut s) % 10;
            let dp = (rng(&mut s) % 61) as i64 - 30;
            let rate = (rng(&mut s) % (2 * RATE_E9 as u64 + 1)) as i128 - RATE_E9;
            let sz = 1 + (rng(&mut s) % 4_999_999) as u128 / (1 + rng(&mut s) as u128 % 3);
            match r {
                0..=3 => Op::Acc(dp, rate),
                4..=5 => Op::Path(dp, 1 + (rng(&mut s) % 2) as u8, rate, rng(&mut s).is_multiple_of(2)),
                6..=7 => Op::Open(sz),
                _ => Op::Rev(sz * (1 + rng(&mut s) as u128 % 3)),
            }
        })
        .collect()
}

fn settle_pair(w: &mut World, l: &mut PortfolioAccountV16Account, s: &mut PortfolioAccountV16Account, rev: bool) {
    if rev {
        w.settle(s);
        w.settle(l);
    } else {
        w.settle(l);
        w.settle(s);
    }
}

/// Final (effective equities, leg tuples) of one arm.
type ArmResult = ([i128; 2], [(i128, i128, u128, u128); 2]);
fn run_arm(ops: &[Op], every_slot: bool, rev: bool) -> ArmResult {
    let mut w = World::new_pairs(&[(1_000_000_000_000_000, 1_000_000_000_000_000, 3)], 0, 0);
    let mut l = w.traders[0];
    let mut s = w.traders[1];
    for op in ops {
        match *op {
            Op::Acc(dp, rate) => {
                if w.accrue(dp, rate) && every_slot {
                    settle_pair(&mut w, &mut l, &mut s, rev);
                }
            }
            Op::Path(dp, n, rate, flip) => {
                if w.accrue_path(dp, n, rate, flip) && every_slot {
                    settle_pair(&mut w, &mut l, &mut s, rev);
                }
            }
            Op::Open(sz) => {
                settle_pair(&mut w, &mut l, &mut s, rev);
                let _ = w.trade(&mut l, &mut s, sz);
            }
            Op::Rev(sz) => {
                settle_pair(&mut w, &mut l, &mut s, rev);
                let _ = w.trade(&mut s, &mut l, sz);
            }
        }
        w.traders[0] = l;
        w.traders[1] = s;
        let mut e: [PortfolioAccountV16Account; 0] = [];
        w.validate(&mut e);
    }
    settle_pair(&mut w, &mut l, &mut s, rev);
    w.traders[0] = l;
    w.traders[1] = s;
    let eq = [effective_equity(&mut w, &mut l) - 1_000_000_000_000_000, effective_equity(&mut w, &mut s) - 1_000_000_000_000_000];
    let leg = |a: &PortfolioAccountV16Account| {
        let r = a.legs[0].try_to_runtime().unwrap();
        (r.k_snap, r.f_snap, r.k_rem_num, r.f_rem_num)
    };
    (eq, [leg(&l), leg(&s)])
}

#[test]
fn r1_effective_equity_is_cadence_and_order_invariant() {
    let worlds: u64 = std::env::var("WORLDS").ok().and_then(|v| v.parse().ok()).unwrap_or(400);
    let (mut worst_acct, mut worst_pair, mut worst_seed) = (0i128, 0i128, 0u64);
    for i in 0..worlds {
        let seed = 5u64.wrapping_mul(1_000_003).wrapping_add(i);
        let ops = gen_ops(seed, 60);
        let arms = [run_arm(&ops, true, false), run_arm(&ops, false, false), run_arm(&ops, true, true), run_arm(&ops, false, true)];
        for a in &arms[1..] {
            assert_eq!(arms[0].1, a.1, "seed {seed}: legs must be bit-identical across cadence and order");
        }
        for (k, a) in arms.iter().enumerate() {
            let pair = a.0[0] + a.0[1];
            // never a gain, and the pair loses only the floor atoms of the settlements
            assert!(pair <= 0, "seed {seed} arm {k}: pair gained {pair}");
            if pair < worst_pair {
                worst_pair = pair;
                worst_seed = seed;
            }
            for acct in 0..2 {
                let d = (a.0[acct] - arms[0].0[acct]).abs();
                if d > worst_acct {
                    worst_acct = d;
                }
            }
        }
    }
    eprintln!("R1DIFF worlds {worlds}: worst per-account equity gap across arms {worst_acct}, worst pair deficit {worst_pair} (seed {worst_seed})");
    assert!(worst_acct <= 8, "effective equity differs by cadence/order by up to {worst_acct} atoms");
    assert!(worst_pair >= -64, "pair deficit {worst_pair} atoms exceeds the floor budget");
}

/// S2: a victim whose claim is partly LIENED (the lien backs margin for its flipped, larger
/// position; reached through the public trade flow with a poor account). The liened part cannot
/// be consumed, so the tail of the loss eats retained face 1:1 and is not booked as support.
/// Returns (victim, winner) effective equity and the victim's liened claim num after the flip.
fn liened_scenario(winner_first: bool, cap: u128, sell: u128, up2: i64, steps2: usize) -> (i128, i128, u128) {
    let mut w = World::new_pairs(&[(cap, 1_000_000_000_000_000, 3)], 0, 0);
    let mut l = w.traders[0];
    let mut s = w.traders[1];
    for _ in 0..5 {
        assert!(w.accrue(100, 0));
        assert!(w.settle(&mut l) && w.settle(&mut s));
    }
    w.trade(&mut s, &mut l, sell * POS_SCALE).expect("flip");
    let liened: u128 = l.source_domains.iter().map(|d| d.source_claim_liened_num.get()).sum();
    for _ in 0..steps2 {
        assert!(w.accrue(up2, 0));
    }
    let show = |tag: &str, a: &PortfolioAccountV16Account| {
        if std::env::var("LDBG").is_ok() {
            let bound: u128 = a.source_domains.iter().map(|d| d.source_claim_bound_num.get()).sum();
            let li: u128 = a.source_domains.iter().map(|d| d.source_claim_liened_num.get()).sum();
            eprintln!("  {tag}: cap {} pnl {} reserved {} bound {} liened {}", a.capital.get() as i128 - cap as i128, a.pnl.get(), a.reserved_pnl.get(), bound / 1_000_000_000_000, li / 1_000_000_000_000);
        }
    };
    show("pre-settle victim", &l);
    if winner_first {
        assert!(w.settle(&mut s) && w.settle(&mut l));
    } else {
        assert!(w.settle(&mut l) && w.settle(&mut s));
    }
    show("post victim", &l);
    show("post winner", &s);
    let mut e: [PortfolioAccountV16Account; 0] = [];
    w.traders[0] = l;
    w.traders[1] = s;
    w.validate(&mut e);
    let el = effective_equity(&mut w, &mut l) - cap as i128;
    let es = effective_equity(&mut w, &mut s) - 1_000_000_000_000_000;
    (el, es, liened)
}

#[test]
fn r1_liened_victim_is_order_independent_and_pinned() {
    let mut saw_lien = 0;
    let mut order_dep = vec![];
    for &(cap, sell, up2, st2) in &[(330_000u128, 7u128, 100i64, 1usize), (330_000, 7, 100, 2), (330_000, 7, 50, 3), (300_000, 7, 100, 2), (400_000, 8, 100, 2), (400_000, 7, 150, 2)] {
        let wf = liened_scenario(true, cap, sell, up2, st2);
        let lf = liened_scenario(false, cap, sell, up2, st2);
        if wf.2 > 0 {
            saw_lien += 1;
        }
        eprintln!("R1LIEN cap {cap} sell {sell} {up2}x{st2}: liened {} winner-first (victim {}, winner {}) victim-first (victim {}, winner {})", wf.2, wf.0, wf.1, lf.0, lf.1);
        for r in [wf, lf] {
            assert!(r.0 + r.1 <= 4, "pair gained {} atoms", r.0 + r.1);
        }
        if wf != lf {
            order_dep.push((cap, sell, up2, st2));
        }
        let idx = [(330_000u128, 7u128, 100i64, 1usize), (330_000, 7, 100, 2), (330_000, 7, 50, 3), (300_000, 7, 100, 2), (400_000, 8, 100, 2), (400_000, 7, 150, 2)]
            .iter()
            .position(|x| *x == (cap, sell, up2, st2))
            .unwrap();
        // S5 (option B, protect the counterparty): equal to the pre-R1 engine's LOSER-FIRST result,
        // which is the base's WORST order for the counterparty in all six scenarios.
        const PINNED: [(i128, i128); 6] = [
            (73_993, -124_395),
            (31_533, -68_530),
            (52_661, -98_213),
            (42_863, -79_939),
            (24_834, -49_238),
            (8_282, -25_966),
        ];
        // the pre-R1 engine's worst counterparty equity over both orders (874fe33a)
        const BASE_WORST_COUNTERPARTY: [i128; 6] = [-124_395, -68_530, -98_213, -79_939, -49_238, -25_966];
        assert!(wf.1 >= BASE_WORST_COUNTERPARTY[idx], "counterparty below the pre-R1 worst order");
        assert_eq!((wf.0, wf.1), PINNED[idx], "liened victim: pinned result moved");
    }
    assert!(order_dep.is_empty(), "liened victim: settle order changes effective equity in {order_dep:?}");
    assert!(saw_lien >= 3, "the scenarios must actually carry a lien");
}

fn liened_scenario_try(winner_first: bool, cap: u128, sell: u128, up2: i64, steps2: usize) -> Option<(i128, i128)> {
    let mut w = World::new_pairs(&[(cap, 1_000_000_000_000_000, 3)], 0, 0);
    let mut l = w.traders[0];
    let mut s = w.traders[1];
    for _ in 0..5 {
        assert!(w.accrue(100, 0));
        assert!(w.settle(&mut l) && w.settle(&mut s));
    }
    w.trade(&mut s, &mut l, sell * POS_SCALE).ok()?;
    for _ in 0..steps2 {
        if !w.accrue(up2, 0) { return None; }
    }
    let ok = if winner_first { w.settle(&mut s) && w.settle(&mut l) } else { w.settle(&mut l) && w.settle(&mut s) };
    if !ok { return None; }
    let el = effective_equity(&mut w, &mut l) - cap as i128;
    let es = effective_equity(&mut w, &mut s) - 1_000_000_000_000_000;
    Some((el, es))
}

/// A victim whose loss outruns its capital and face (bad debt). The bad debt never books, so the
/// neutral rate must not assume it does (it would over-credit the victim). Both orders agree and
/// the numbers are pinned: dropping the bad-debt term moves the victim by ~+20,000 atoms.
#[test]
fn r1_bankrupt_victim_is_order_independent_and_not_over_credited() {
    // (victim, winner) effective equity, pinned from the verified neutral-rate run. Dropping the
    // bad-debt term (assuming the whole loss books) moves these by tens of thousands of atoms.
    const PINNED: [(i128, i128); 5] = [
        (-414_343, 329_999),
        (-605_611, 329_999),
        (-804_603, 329_999),
        (-771_596, 399_999),
        (-1_279_126, 399_999),
    ];
    for (n, &(cap, sell, up2, st2)) in [(330_000u128, 7u128, 200i64, 6usize), (330_000, 7, 200, 8), (330_000, 7, 200, 10), (400_000, 8, 200, 8), (400_000, 8, 200, 12)].iter().enumerate() {
        let wf = liened_scenario_try(true, cap, sell, up2, st2).expect("scenario runs");
        let lf = liened_scenario_try(false, cap, sell, up2, st2).expect("scenario runs");
        eprintln!("R1BANKRUPT cap {cap} sell {sell} {up2}x{st2}: {wf:?} {lf:?}");
        assert_eq!(wf, lf, "bankrupt victim: settle order changes effective equity");
        assert_eq!(wf, PINNED[n], "bankrupt victim: pinned result moved");
        assert!(wf.0 + wf.1 <= 4, "pair gained {}", wf.0 + wf.1);
    }
}

/// Resolved and forfeit paths keep the stored rate: the same flip scenario, but the market is
/// resolved with the loss still unsettled and both accounts are closed in either order. The
/// payouts must equal the pre-R1 engine's (pinned below; computed on 874fe33a). Forcing the Live
/// booking predicate true changes them.
fn resolved_payouts(winner_first: bool) -> (u128, u128) {
    let mut w = World::new_pairs(&[(1_000_000_000_000_000, 1_000_000_000_000_000, 3)], 0, 0);
    let mut l = w.traders[0];
    let mut s = w.traders[1];
    for _ in 0..5 {
        assert!(w.accrue(100, 0));
        assert!(w.settle(&mut l) && w.settle(&mut s));
    }
    w.trade(&mut s, &mut l, 6 * POS_SCALE).expect("flip");
    for _ in 0..3 {
        assert!(w.accrue(100, 0));
    }
    let slot = w.slot + 1;
    let mut m = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
    m.resolve_market_not_atomic(slot).unwrap();
    let mut paid = [0u128; 2];
    let mut closed = [false; 2];
    let order: [usize; 2] = if winner_first { [1, 0] } else { [0, 1] };
    for _round in 0..40 {
        for &i in &order {
            if closed[i] {
                continue;
            }
            let a = if i == 0 { &mut l } else { &mut s };
            for d in 0..2 {
                let _ = m.expire_source_backing_bucket_not_atomic(d, slot);
            }
            if let Ok(percolator::ResolvedCloseOutcomeV16::Closed { payout }) =
                m.close_resolved_account_not_atomic(&mut PortfolioV16ViewMut::new(a), 0)
            {
                paid[i] = payout;
                closed[i] = true;
            }
        }
        if closed[0] && closed[1] {
            break;
        }
    }
    assert!(closed[0] && closed[1], "both accounts close");
    (paid[0], paid[1])
}

#[test]
fn r1_resolved_close_is_unchanged() {
    let wf = resolved_payouts(true);
    let lf = resolved_payouts(false);
    eprintln!("R1RESOLVED winner-first {wf:?} victim-first {lf:?}");
    // pre-R1 values (874fe33a). Resolved close is itself order dependent (pre-existing, not R1);
    // this test only pins that the R1 change does not touch it.
    assert_eq!(wf, (999_999_999_998_674, 999_999_999_942_508));
    assert_eq!(lf, (1_000_000_000_057_492, 999_999_999_942_508));
}

/// A NON-liened victim whose loss outruns its capital and face. This is the class the neutral
/// rate (not the S5 protective rate) prices, so it pins the booked-amount, bad-debt and extra-claims
/// terms. Order dependence REMAINS here by construction: victim-first sees a fully backed domain
/// (stored rate 1) while winner-first sees the in-flight credit, and the bad debt will not book.
/// The victim is never worse than the pre-R1 engine in either order and the counterparty is untouched.
#[test]
fn r1_nonliened_bankrupt_victim_is_pinned_and_no_worse_than_pre_r1() {
    // (cap, sell, up2, st2) -> (fix winner-first, fix victim-first, pre-R1 winner-first, pre-R1 victim-first)
    const CASES: [((u128, u128, i64, usize), (i128, i128, i128, i128)); 4] = [
        ((330_000, 5, 200, 14), (-553_486, -518_502, -643_128, -518_502)),
        ((330_000, 5, 200, 18), (-807_740, -747_130, -877_922, -747_130)),
        ((330_000, 5, 200, 12), (-428_054, -410_804, -531_165, -410_804)),
        ((300_000, 6, 200, 10), (-580_525, -537_447, -662_711, -537_447)),
    ];
    for &((cap, sell, up2, st2), (fwf, flf, bwf, blf)) in &CASES {
        let wf = liened_scenario_try(true, cap, sell, up2, st2).expect("scenario runs");
        let lf = liened_scenario_try(false, cap, sell, up2, st2).expect("scenario runs");
        assert_eq!((wf.0, lf.0), (fwf, flf), "pinned neutral-rate result moved: {cap} {sell} {up2}x{st2}");
        assert!(wf.0 >= bwf && lf.0 >= blf, "victim worse than the pre-R1 engine");
        assert_eq!((wf.1, lf.1), (cap as i128 - 1, cap as i128 - 1), "the counterparty is untouched");
    }
}
