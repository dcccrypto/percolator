//! fix/v21-funding-scale: adversarial bookkeeping harness, ported from the security review
//! (Sentinel, `ledger/security-review-v21-funding-scale-2026-10-05.md`,
//! `~/wt-sec-fundscale/eng/tests/sec_fundscale_adv.rs`). Kept as a PERMANENT engine test (M1):
//! after EVERY step of random two-sided worlds (thin peers, partial closes, zigzags with funding
//! sign flips, slides, crank cascades, insurance 0..600M per domain) it asserts that the stale
//! count, stale weight, laggard count and laggard weight equal the exact sums over the real legs,
//! and that the real hidden K/F loss never exceeds the bound. Any accrual site or merge that
//! marks a cohort without updating the drift tracker fails here.
//!
//! F6 (funding rounding) regression: `sec_f6_funding_rounding_low_price` (inverted from the
//! reviewer's reproduction: it now pins the INTENDED rate).
//! Sentinel adversarial harness (review only).
#![allow(dead_code, unused_imports)]
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

// v2.2 combination: the whole adversarial harness can run with holding-fee rent > 0
// (Wave B). Thread-local so the rent test cannot leak into the rent-off sweeps.
thread_local! { static RENT_E9: std::cell::Cell<u64> = const { std::cell::Cell::new(0) }; }
fn rent_e9() -> u64 { RENT_E9.with(|c| c.get()) }

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
        let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
        cfg.max_abs_funding_e9_per_slot = cap;
        cfg.rent_max_e9_per_slot = rent_e9();
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
        if m.accrue_asset_to_with_rent_not_atomic(0, slot, new, rate, rent_e9(), rent_e9() / 2, true).is_err() {
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
        if m.accrue_asset_path_with_rent_to_not_atomic(0, now, target, &steps, rent_e9(), rent_e9() / 2, true).is_err() {
            self.header = h;
            self.markets = mk;
            return false;
        }
        // keep the book tradable: the next segment re-targets from wherever the path ended
        m.markets[0].engine.asset.raw_oracle_target_price = V16PodU64::new(price);
        self.slot = now;
        true
    }

    /// Rent-ONLY canonical path accrual: `n` one-slot steps at the SAME price with zero funding, so
    /// only the rent index moves (the path-accrual mark site; `accrue` above drives the
    /// single-step site).
    fn accrue_path_rent_only(&mut self, n: u8) -> bool {
        let a = self.asset();
        let price = a.effective_price;
        let n = n.clamp(1, 2) as u64;
        let steps: Vec<percolator::AccrualStepV16> = (0..n)
            .map(|_| percolator::AccrualStepV16 {
                effective_price: price,
                funding_rate_e9: 0,
                price_move_remainder_before_bps_num: 0,
                price_move_remainder_after_bps_num: 0,
            })
            .collect();
        let now = self.slot + n;
        let (h, mk) = (self.header, self.markets.clone());
        let mut m = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        if m.accrue_asset_path_with_rent_to_not_atomic(0, now, price, &steps, rent_e9(), rent_e9() / 2, true).is_err() {
            self.header = h;
            self.markets = mk;
            return false;
        }
        m.markets[0].engine.asset.raw_oracle_target_price = V16PodU64::new(price);
        self.slot = now;
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



fn all_accounts<'a>(w: &'a World, extra: &'a [PortfolioAccountV16Account]) -> Vec<&'a PortfolioAccountV16Account> {
    w.traders.iter().chain(core::iter::once(&w.maker)).chain(extra.iter()).collect()
}

/// Exact bookkeeping + bound check, per side, over every account.
fn check_state(w: &World, extra: &[PortfolioAccountV16Account], tag: &str) -> Result<(), String> {
    let asset = w.asset();
    if asset.mode_long != percolator::SideModeV16::Normal || asset.mode_short != percolator::SideModeV16::Normal {
        return Ok(());
    }
    for side in [SideV16::Long, SideV16::Short] {
        let (epoch, stale, d) = match side {
            SideV16::Long => (asset.kf_epoch_long, asset.stale_account_count_long, w.markets[0].engine.kf_drift_long.to_runtime()),
            SideV16::Short => (asset.kf_epoch_short, asset.stale_account_count_short, w.markets[0].engine.kf_drift_short.to_runtime()),
        };
        let (mut sw, mut lc, mut lw, mut real) = (0u128, 0u64, 0u128, 0u128);
        let mut n_stale = 0u64;
        for a in all_accounts(w, extra) {
            if a.active_bitmap[0].get() & 1 == 0 { continue; }
            let leg = a.legs[0].try_to_runtime().unwrap();
            if leg.side != side || leg.kf_epoch_snap >= epoch { continue; }
            n_stale += 1;
            sw += leg.loss_weight;
            if leg.kf_epoch_snap < d.gen_epoch { lc += 1; lw += leg.loss_weight; }
            if let Some((_, l)) = stale_leg_loss(&asset, a) { real += l; }
        }
        if n_stale != stale { return Err(format!("{tag}: {side:?} stale count {stale} != actual {n_stale}")); }
        if stale > 0 {
            if sw != d.stale_weight { return Err(format!("{tag}: {side:?} stale_weight {} != exact {sw}", d.stale_weight)); }
            if lc != d.laggard_count { return Err(format!("{tag}: {side:?} laggard_count {} != exact {lc}", d.laggard_count)); }
            if lw != d.laggard_weight { return Err(format!("{tag}: {side:?} laggard_weight {} != exact {lw}", d.laggard_weight)); }
        }
        let b = bound(w, side);
        if real > b { return Err(format!("{tag}: {side:?} REAL hidden loss {real} > bound {b}")); }
    }
    Ok(())
}

#[derive(Clone, Debug)]
enum Op2 {
    Accrue(i64, i128),
    Zig(i64, u8),          // oscillation: up/down n times with funding sign flips
    Refresh(usize),
    Crank(usize),
    Reduce(usize, u8),     // partial close of account i vs rich maker (percent)
    Open(bool, u128),
    AddEntrant(usize, u128),
    CrankAll,
    Slide(bool, u8),
    /// canonical path accrual: (target dp bps, steps, funding sign flips per step)
    Path(i64, u8, bool),
}

fn op2() -> impl Strategy<Value = Op2> {
    prop_oneof![
        4 => ((-(MOVE_CAP_BPS as i64))..=(MOVE_CAP_BPS as i64), -RATE_E9..=RATE_E9).prop_map(|(d, r)| Op2::Accrue(d, r)),
        2 => ((1i64..=MOVE_CAP_BPS as i64), 1u8..6).prop_map(|(d, n)| Op2::Zig(d, n)),
        3 => (0usize..64).prop_map(Op2::Refresh),
        1 => (0usize..64).prop_map(Op2::Crank),
        2 => (0usize..64, 10u8..100).prop_map(|(i, p)| Op2::Reduce(i, p)),
        3 => (any::<bool>(), 1u128..=3).prop_map(|(l, u)| Op2::Open(l, u)),
        1 => (0usize..64, 1u128..=2).prop_map(|(i, u)| Op2::AddEntrant(i, u)),
        1 => Just(Op2::CrankAll),
        3 => (any::<bool>(), 4u8..14).prop_map(|(u, n)| Op2::Slide(u, n)),
        4 => (-(8 * MOVE_CAP_BPS as i64)..=(8 * MOVE_CAP_BPS as i64), 1u8..=8, any::<bool>()).prop_map(|(d, n, f)| Op2::Path(d, n, f)),
    ]
}

#[derive(Default, Debug)]
struct Stats2 { admissions: u64, p3: u64, hidden_def: u64, reduces: u64, rotations_seen: u64, negpnl_skip: u64, paths: u64 }

fn run2(pairs: Vec<(u128, u128, u128)>, ins: (u128, u128), ops: Vec<Op2>, st: &mut Stats2) -> Result<(), String> {
    let mut w = World::new_pairs(&pairs, ins.0, ins.1);
    let mut entrants: Vec<PortfolioAccountV16Account> = Vec::new();
    let mut seed = 10_000u32;
    for (step, op) in ops.into_iter().enumerate() {
        let tag = format!("step {step} {op:?}");
        match op {
            Op2::Accrue(d, r) => { w.accrue(d, r); }
            Op2::Zig(d, n) => {
                for k in 0..(2 * n as usize) {
                    let up = k % 2 == 0;
                    w.accrue(if up { d } else { -d }, if up { -RATE_E9 } else { RATE_E9 });
                }
            }
            Op2::Refresh(i) => {
                let n = w.traders.len();
                let mut t = w.traders[i % n];
                w.refresh(&mut t);
                w.traders[i % n] = t;
            }
            Op2::Crank(i) => {
                let n = w.traders.len();
                let mut t = w.traders[i % n];
                w.crank(&mut t);
                w.traders[i % n] = t;
            }
            Op2::CrankAll => { w.cascade(&mut entrants); }
            Op2::Path(d, n, flip) => { if w.accrue_path(d, n, RATE_E9, flip) { st.paths += 1; } }
            Op2::Slide(up, n) => {
                for _ in 0..n {
                    if up { w.accrue(MOVE_CAP_BPS as i64, -RATE_E9); } else { w.accrue(-(MOVE_CAP_BPS as i64), RATE_E9); }
                }
            }
            Op2::Reduce(i, pct) => {
                let n = w.traders.len();
                let mut t = w.traders[i % n];
                if t.active_bitmap[0].get() & 1 != 0 {
                    let leg = t.legs[0].try_to_runtime().unwrap();
                    let q = leg.basis_pos_q.unsigned_abs();
                    let sz = (q * pct as u128 / 100).max(POS_SCALE / 1000);
                    if sz < q {
                        let mut maker = w.maker;
                        let ok = if leg.side == SideV16::Long { w.trade(&mut maker, &mut t, sz) } else { w.trade(&mut t, &mut maker, sz) }.is_ok();
                        if ok { w.maker = maker; st.reduces += 1; }
                    }
                }
                w.traders[i % n] = t;
            }
            Op2::Open(long, units) => {
                let mut e = account(seed); seed += 1;
                w.deposit(&mut e, (units * u128::from(w.price())) / 8 + 1);
                let mut maker = w.maker;
                let before = w.clone();
                let ok = if long { w.trade(&mut e, &mut maker, units * POS_SCALE) } else { w.trade(&mut maker, &mut e, units * POS_SCALE) }.is_ok();
                if ok {
                    w.maker = maker;
                    let a = before.asset();
                    if a.stale_account_count_long + a.stale_account_count_short != 0 {
                        st.admissions += 1;
                        p3(&w, &e, st)?;
                    }
                    entrants.push(e);
                }
            }
            Op2::AddEntrant(i, units) => {
                if entrants.is_empty() { continue; }
                let n = entrants.len();
                let mut e = entrants[i % n];
                let mut maker = w.maker;
                let long = e.legs[0].try_to_runtime().map(|l| l.side == SideV16::Long).unwrap_or(true);
                w.deposit(&mut e, (units * u128::from(w.price())) / 8 + 1);
                let before = w.clone();
                let ok = if long { w.trade(&mut e, &mut maker, units * POS_SCALE) } else { w.trade(&mut maker, &mut e, units * POS_SCALE) }.is_ok();
                if ok {
                    w.maker = maker;
                    let a = before.asset();
                    if a.stale_account_count_long + a.stale_account_count_short != 0 {
                        st.admissions += 1;
                        p3(&w, &e, st)?;
                    }
                }
                entrants[i % n] = e;
            }
        }
        check_state(&w, &entrants, &tag)?;
    }
    w.cascade(&mut entrants);
    w.validate(&mut entrants);
    Ok(())
}

fn p3(w: &World, entrant: &PortfolioAccountV16Account, st: &mut Stats2) -> Result<(), String> {
    if w.header.negative_pnl_account_count.get() != 0 { st.negpnl_skip += 1; return Ok(()); }
    let asset = w.asset();
    let mut deficit = false;
    for a in w.traders.iter().chain(core::iter::once(&w.maker)) {
        if let Some((_, loss)) = stale_leg_loss(&asset, a) { deficit |= equity(a) < loss as i128; }
    }
    if deficit { st.hidden_def += 1; }
    let before = equity(entrant);
    let (b_long, b_short) = (asset.b_long_num, asset.b_short_num);
    let mut w2 = w.clone();
    let mut e2 = [*entrant];
    w2.cascade(&mut e2);
    let a2 = w2.asset();
    if (a2.b_long_num, a2.b_short_num) != (b_long, b_short) { return Err(format!("P3: B moved {:?}->{:?}", (b_long,b_short),(a2.b_long_num,a2.b_short_num))); }
    if equity(&e2[0]) != before { return Err(format!("P3: entrant equity {before} -> {}", equity(&e2[0]))); }
    st.p3 += 1;
    Ok(())
}

fn pair() -> impl Strategy<Value = (u128, u128, u128)> {
    (1u128..=3, 105u128..=400, 105u128..=400).prop_map(|(u, a, b)| (u * u128::from(PRICE) * a / 1_000, u * u128::from(PRICE) * b / 1_000, u))
}

fn ins() -> impl Strategy<Value = u128> {
    prop_oneof![Just(0u128), 1u128..2_000, 2_000u128..2_000_000, 2_000_000u128..60_000_000, 60_000_000u128..600_000_000]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: std::env::var("PROPTEST_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(64), max_shrink_iters: 2000, .. ProptestConfig::default() })]
    #[test]
    fn sec_adv_two_sided_world(
        pairs in prop::collection::vec(pair(), 3..14),
        il in ins(), is in ins(),
        ops in prop::collection::vec(op2(), 6..60),
    ) {
        let mut st = Stats2::default();
        let r = run2(pairs, (il, is), ops, &mut st);
        prop_assert!(r.is_ok(), "{:?}", r);
    }
}

#[test]
fn sec_adv_sweep_stats() {
    use proptest::strategy::ValueTree;
    use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
    let seed: u8 = std::env::var("SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(11);
    let n: usize = std::env::var("WORLDS").ok().and_then(|v| v.parse().ok()).unwrap_or(500);
    let mut runner = TestRunner::new_with_rng(Config::default(), TestRng::from_seed(RngAlgorithm::ChaCha, &[seed; 32]));
    let strat = (prop::collection::vec(pair(), 3..14), ins(), ins(), prop::collection::vec(op2(), 6..80));
    let mut st = Stats2::default();
    let mut fails = 0;
    for _ in 0..n {
        let (p, il, is, ops) = strat.new_tree(&mut runner).unwrap().current();
        if let Err(e) = run2(p, (il, is), ops, &mut st) { fails += 1; eprintln!("FAIL: {e}"); }
    }
    eprintln!("adv sweep seed {seed}: worlds {n} fails {fails} {st:?}");
    assert_eq!(fails, 0);
    // non-vacuity: both accrual routes, relaxed admissions and real hidden deficits occurred
    assert!(st.paths > 100 && st.admissions > 50 && st.hidden_def > 10, "{st:?}");
}


/// T6: a stale cohort whose drift tail is zero (slab grown in place from the old layout, or any
/// path that zeroes `kf_drift_*` while `stale_account_count_*` > 0) makes the bound collapse to
/// `2*stale`: relaxed admission with almost no cover although hidden bankruptcies exist.
#[test]
fn sec_t6_zero_tail_with_live_cohort_undercounts() {
    let traders: Vec<(u128, u128, u128)> = (0..6).map(|_| (105_000u128, 10_000_000u128, 1u128)).collect();
    let mut w = World::new_pairs(&traders, 0, 0);
    for _ in 0..7 { assert!(w.accrue(-200, RATE_E9)); } // longs slide ~13%, nobody settled
    let asset = w.asset();
    let real_long: u128 = w.traders.iter().filter_map(|a| stale_leg_loss(&asset, a)).filter(|(s, _)| *s == SideV16::Long).map(|(_, l)| l).sum();
    let honest = bound(&w, SideV16::Long);
    eprintln!("T6 real long hidden loss {real_long}, tracked bound {honest}");
    // fund the cover and the counterparties first (so the setup itself is shape-valid), then
    // simulate "zero tail with live cohort"
    let cover = 2 * u128::from(asset.stale_account_count_long) + 1;
    {
        let mut m = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
        m.deposit_domain_insurance_not_atomic(1, cover).unwrap();
        m.deposit_domain_insurance_not_atomic(0, cover).unwrap();
    }
    let mut cp = account(50_000); w.deposit(&mut cp, 10_000_000);
    let mut e = account(50_001); w.deposit(&mut e, 300_000);
    w.markets[0].engine.kf_drift_long = Default::default();
    w.markets[0].engine.kf_drift_short = Default::default();
    let b = bound(&w, SideV16::Long);
    eprintln!("T6 bound after tail zeroed {b} (stale count {})", w.asset().stale_account_count_long);
    assert!(b < real_long, "the unchecked formula collapses below the real hidden loss");
    assert!(cover > b, "the cover would satisfy the collapsed bound");
    // S1 shape check: the audit-scan walk rejects the corrupted tracker outright
    if cfg!(feature = "audit-scan") {
        let mut header = w.header;
        let mut markets = w.markets.clone();
        let m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        assert_eq!(m.validate_shape(), Err(percolator::V16Error::InvalidConfig));
    }
    let r = w.trade(&mut cp, &mut e, 2 * POS_SCALE);
    eprintln!("T6 entrant with {cover} atoms of cover vs {real_long} real hidden loss: {:?}", r);
    // Security review S1 regression: the engine must fail CLOSED on a tracker that does not
    // describe the live cohort (stale_weight < stale), whatever the cover.
    assert!(
        r == Err(percolator::V16Error::LossStale)
            || (cfg!(feature = "audit-scan") && r == Err(percolator::V16Error::InvalidConfig)),
        "a zeroed drift tail under a live cohort must never admit (S1): {r:?}"
    );
    // and the domain withdrawal reservation blocks everything on that side
    let mut header = w.header;
    let mut markets = w.markets.clone();
    let m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    assert_eq!(m.domain_insurance_withdraw_capacity(1).unwrap(), 0, "S1: withdrawal blocked");
}

/// Security review S2: outside `Normal` the drift weights can under-count (a drain reset zeroes
/// `loss_weight_sum` under stored prior-epoch legs), so the insurance-withdrawal reservation
/// must not trust the bound there: the absorbing domain is fully reserved while that side has
/// stale legs.
#[test]
fn sec_s2_withdraw_reservation_blocks_outside_normal_mode() {
    let traders: Vec<(u128, u128, u128)> = (0..4).map(|_| (900_000u128, 10_000_000u128, 1u128)).collect();
    let mut w = World::new_pairs(&traders, 0, 0);
    assert!(w.accrue(-50, RATE_E9));
    {
        let mut m = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
        m.deposit_domain_insurance_not_atomic(1, 50_000_000).unwrap();
    }
    let asset = w.asset();
    assert!(asset.stale_account_count_long > 0);
    let cap_normal = {
        let mut header = w.header;
        let mut markets = w.markets.clone();
        let m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        m.domain_insurance_withdraw_capacity(1).unwrap()
    };
    assert!(cap_normal > 0, "Normal mode: cover above the bound is withdrawable");
    // same state with the long side outside Normal (as after a drain reset)
    let mut a = asset;
    a.mode_long = percolator::SideModeV16::ResetPending;
    w.markets[0].engine.asset = percolator::AssetStateV16Account::from_runtime(&a);
    let mut header = w.header;
    let mut markets = w.markets.clone();
    let m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    assert_eq!(m.domain_insurance_withdraw_capacity(1).unwrap(), 0, "S2: fully reserved outside Normal");
}

/// F6 regression (security review F6, reproduction inverted). Before: per-accrual funding was
/// floored to whole price units, so at price e6 = 30,000 and the v2.1 cap (111e-9/slot) positive
/// funding was 0 for 49 slots and ONE slot of negative funding charged a short 1,000 atoms
/// (intended 3.33). Now F integrates the exact numerator: equal and opposite, path-independent
/// (49 one-slot calls == 49 x one slot exactly; the remainder is identically zero because
/// ADL_ONE % FUNDING_DEN == 0 and A == ADL_ONE in Normal), and the payer pays the intended rate.
#[test]
fn sec_f6_funding_rounding_low_price() {
    let px = 30_000u64;
    let mut w = World::new_pairs_at(&[(10_000_000_000, 10_000_000_000, 1_000)], 0, 0, px, 111);
    let f0 = (w.asset().f_long_num, w.asset().f_short_num);
    let mut l = w.traders[0];
    let le0 = equity(&l);
    for _ in 0..49 { assert!(w.accrue(0, 111)); }
    let a = w.asset();
    let per_slot = 111i128 * px as i128 * (ADL_ONE / 1_000_000_000) as i128; // exact index units
    assert_eq!(a.f_long_num - f0.0, -49 * per_slot, "longs pay exactly, every slot (no floor to 0)");
    assert_eq!(a.f_short_num - f0.1, 49 * per_slot, "shorts receive exactly the same");
    assert!(w.refresh(&mut l));
    let paid = le0 - equity(&l);
    let intended = 49.0 * 111.0 * 1_000.0 * 30_000.0 / 1e9; // 163.17 atoms
    assert!((paid as f64 - intended).abs() <= 1.0, "long paid {paid}, intended {intended:.2}");
    // one slot of negative funding: shorts pay ~3.33 atoms, not a full price unit (1,000)
    let mut sh = w.traders[1];
    assert!(w.refresh(&mut sh));
    let se0 = equity(&sh);
    assert!(w.accrue(0, -111));
    assert!(w.refresh(&mut sh));
    let paid = se0 - equity(&sh);
    let intended1 = 111.0 * 1_000.0 * 30_000.0 / 1e9;
    assert!((paid as f64 - intended1).abs() <= 1.0, "short paid {paid} in one slot, intended {intended1:.2}");
}


/// v2.2 combination gate: the same exact-bookkeeping + bound-covers-real-loss sweep with
/// holding-fee rent charged on every accrual (asymmetric: long = R, short = R/2). A rent move
/// marks cohorts stale but must add no adverse K/F travel; `check_state` asserts exact
/// stale/laggard counts and weights and `bound >= real hidden loss` after EVERY step.
#[test]
fn sec_adv_sweep_stats_rent_positive() {
    for rent in [2_000u64, 9_000u64] {
        RENT_E9.with(|c| c.set(rent));
        use proptest::strategy::ValueTree;
        use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
        let seed: u8 = std::env::var("SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(33);
        let n: usize = std::env::var("WORLDS").ok().and_then(|v| v.parse().ok()).unwrap_or(400);
        let mut runner = TestRunner::new_with_rng(Config::default(), TestRng::from_seed(RngAlgorithm::ChaCha, &[seed; 32]));
        let strat = (prop::collection::vec(pair(), 3..14), ins(), ins(), prop::collection::vec(op2(), 6..80));
        let mut st = Stats2::default();
        let mut fails = 0;
        for _ in 0..n {
            let (p, il, is, ops) = strat.new_tree(&mut runner).unwrap().current();
            if let Err(e) = run2(p, (il, is), ops, &mut st) { fails += 1; eprintln!("FAIL(rent {rent}): {e}"); }
        }
        eprintln!("rent {rent} sweep seed {seed}: worlds {n} fails {fails} {st:?}");
        assert_eq!(fails, 0);
        assert!(st.paths > 50 && st.admissions > 20 && st.hidden_def > 5, "{st:?}");
        // rent actually accrued: a fresh world with rent on shows a non-zero rent index after slides
        let mut w = World::new_pairs(&[(105_000u128, 10_000_000u128, 1u128); 3], 0, 0);
        for _ in 0..4 { assert!(w.accrue(10, RATE_E9)); }
        let a = w.asset();
        assert!(a.rent_index_long_num != 0 && a.rent_index_short_num != 0, "rent must accrue (non-vacuity)");
    }
    RENT_E9.with(|c| c.set(0));
}


/// Rent-ONLY accrual (no price move, no funding): the rent index alone marks the cohorts stale.
/// The tracker must follow (exact bookkeeping) with zero adverse travel. Discriminates the merge
/// hunk that passes the rent-aware `changed` flags into `track_kf_drift`.
#[test]
fn sec_adv_rent_only_accrual_keeps_the_tracker_exact() {
    RENT_E9.with(|c| c.set(5_000));
    let traders: Vec<(u128, u128, u128)> = (0..5).map(|_| (105_000u128, 10_000_000u128, 1u128)).collect();
    let mut w = World::new_pairs(&traders, 0, 0);
    for step in 0..6 {
        assert!(w.accrue(0, 0), "rent-only accrue {step}");
        check_state(&w, &[], &format!("rent-only {step}")).unwrap();
        assert!(w.accrue_path(0, 2, 0, false) || true);
        check_state(&w, &[], &format!("rent-only path {step}")).unwrap();
    }
    let a = w.asset();
    assert!(a.rent_index_long_num != 0, "rent accrued");
    assert!(a.stale_account_count_long + a.stale_account_count_short > 0, "rent alone marked a cohort stale");
    RENT_E9.with(|c| c.set(0));
}


/// The PATH-accrual mark site (`accrue_asset_path_with_rent_to_not_atomic`): rent-only steps (same
/// price, zero funding) must keep the K/F tracker exact. Discriminates the merge hunk that passes
/// the rent-aware `changed` flags into `track_kf_drift` at that site (negative control in the
/// ledger: dropping the rent flags there makes this test fail).
#[test]
fn sec_adv_rent_only_path_accrual_keeps_the_tracker_exact() {
    RENT_E9.with(|c| c.set(5_000));
    let traders: Vec<(u128, u128, u128)> = (0..5).map(|_| (105_000u128, 10_000_000u128, 1u128)).collect();
    let mut w = World::new_pairs(&traders, 0, 0);
    for step in 0..6 {
        assert!(w.accrue_path_rent_only(2), "rent-only path accrue {step} must be accepted (non-vacuity)");
        check_state(&w, &[], &format!("rent-only path {step}")).unwrap();
    }
    let a = w.asset();
    assert!(a.rent_index_long_num != 0, "rent accrued on the path route");
    assert!(a.stale_account_count_long + a.stale_account_count_short > 0, "rent alone marked a cohort stale on the path route");
    RENT_E9.with(|c| c.set(0));
}
