//! Randomised NON-flipping reversal worlds (the #457 shape): poor and rich longs, one or two
//! shorts, a rich maker on the other side of all of them, a random subset cranked at the price
//! peak, then the reversal below the entry, then the accounts settled in a permutation.
//!
//! * S9: the pending-credit counter never exceeds the claim stock, after every settle.
//! * S10 (fixed): equity WAS not cadence invariant here (pre-R1 engine: 450 of 600 worlds
//!   deviate, #282 round 4: 260). The deviation was exactly the backing a loser's REALISED peak loss
//!   left in a domain with no claimant (the unsettled winners' gain reversed before they settled),
//!   and it fell on the loser side (the maker, mostly). The unclaimed-backing rebalance
//!   (`rebalance_unclaimed_backing_across_asset_domains_not_atomic`) closes it: 0 of 600 deviate.
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
        Self::new_pairs_hmax(pairs, ins_long, ins_short, px, cap, 6_480_000)
    }
    fn new_pairs_hmax(pairs: &[(u128, u128, u128)], ins_long: u128, ins_short: u128, px: u64, cap: u64, h_max: u64) -> Self {
        let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, h_max);
        cfg.max_abs_funding_e9_per_slot = cap;
        cfg.max_price_move_bps_per_slot = MOVE_CAP_BPS;
        cfg.initial_margin_bps = 1_000;
        cfg.maintenance_margin_bps = 500;
        // multi-slot segments and canonical paths (the wrapper's production accrual route)
        cfg.max_accrual_dt_slots = 2;
        cfg.min_funding_lifetime_slots = 10_000_000;
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





const U: u128 = 1_000_000;

fn effective_equity(w: &mut World, a: &mut PortfolioAccountV16Account) -> i128 {
    assert!(w.refresh(a), "refresh for certification");
    let cert = a.health_cert.try_to_runtime().expect("cert decodes");
    assert!(cert.valid);
    cert.certified_equity
}

fn rng(s: &mut u64) -> u64 { *s ^= *s << 13; *s ^= *s >> 7; *s ^= *s << 17; *s }

fn perms(n: usize, count: usize, s: &mut u64) -> Vec<Vec<usize>> {
    let mut out = vec![(0..n).collect::<Vec<_>>(), (0..n).rev().collect::<Vec<_>>()];
    while out.len() < count {
        let mut p: Vec<usize> = (0..n).collect();
        for i in (1..n).rev() { let j = (rng(s) % (i as u64 + 1)) as usize; p.swap(i, j); }
        out.push(p);
    }
    out
}

/// Pending-credit counter vs claim stock, per source domain. Returns the number of violations.
fn pend_violations(w: &World, accts: &[PortfolioAccountV16Account], m: &PortfolioAccountV16Account) -> u64 {
    let mut v = 0;
    for dd in 0..2usize {
        let stock: u128 = accts.iter().chain(std::iter::once(m)).flat_map(|a| a.source_domains.iter())
            .filter(|sd| sd.is_occupied() && sd.domain.get() as usize == dd)
            .map(|sd| sd.source_claim_bound_num.get()).sum();
        let slot = &w.markets[0].engine;
        let pend = if dd == 0 { slot.kf_pending_credit_long.get() } else { slot.kf_pending_credit_short.get() };
        if pend > stock as i128 { v += 1; }
    }
    v
}

struct Rev {
    /// equity change per account (longs..., shorts..., maker last), None if the world did not run
    eq: Vec<i128>,
    /// backing in excess of claims, summed over both domains, at the end
    stranded: i128,
    pend_checks: u64,
    pend_violations: u64,
}

/// `mask`: bit i = account i (maker = bit n) cranked at the peak.
fn reversal(seed: u64, perm: &[usize], forced_mask: Option<u64>) -> Option<Rev> {
    let mut s = seed | 1;
    let nl = 2 + (rng(&mut s) % 3) as usize;
    let ns = 1 + (rng(&mut s) % 2) as usize;
    let n = nl + ns;
    let cap = 1_000_000_000_000_000u128;
    let poor = 1_000_000_000u128;
    let mut w = World::new_pairs(&[], 0, 0);
    let mut accts: Vec<PortfolioAccountV16Account> = (0..n).map(|i| account(200 + i as u32)).collect();
    let mut caps = vec![];
    for i in 0..n {
        let c = if i == 0 { poor } else if i < nl && rng(&mut s).is_multiple_of(2) { poor * 2 } else { cap };
        caps.push(c);
        let mut a = accts[i]; w.deposit(&mut a, c); accts[i] = a;
    }
    let mut m = w.maker;
    for i in 0..n {
        let u = 1 + (rng(&mut s) % 6) as u128;
        let mut a = accts[i];
        let r = if i < nl { w.trade(&mut a, &mut m, u * 400 * POS_SCALE / 4) } else { w.trade(&mut m, &mut a, u * 100 * POS_SCALE / 4) };
        r.ok()?;
        accts[i] = a;
    }
    let start: Vec<i128> = accts.iter().map(|a| a.capital.get() as i128 + a.pnl.get()).collect();
    let steps = |w: &mut World, target: u64| -> bool {
        let mut g = 0;
        while w.price() != target && g < 200 {
            g += 1;
            let cur = w.price() as i128;
            let b = (((target as i128 - cur) * 10_000) / cur).clamp(-190, 190) as i64;
            let b = if b == 0 { if target as i128 > cur { 1 } else { -1 } } else { b };
            if !w.accrue(b, 0) { return false; }
        }
        true
    };
    if !steps(&mut w, 1_000_000 + 80_000 + (rng(&mut s) % 100_000)) { return None; }
    let mask = rng(&mut s) % (1 << (n + 1));
    let mask = forced_mask.unwrap_or(mask);
    let (mut checks, mut viol) = (0u64, 0u64);
    for i in 0..n { if mask >> i & 1 == 1 { let mut a = accts[i]; if !w.settle(&mut a) { return None; } accts[i] = a; } }
    if mask >> n & 1 == 1 { let mut mm = m; if !w.settle(&mut mm) { return None; } m = mm; }
    checks += 2; viol += pend_violations(&w, &accts, &m);
    let low = 700_000 + (rng(&mut s) % 200_000);
    if !steps(&mut w, low) { return None; }
    for &k in perm {
        if k == n { let mut mm = m; if !w.settle(&mut mm) { return None; } m = mm; }
        else { let mut a = accts[k]; if !w.settle(&mut a) { return None; } accts[k] = a; }
        checks += 2; viol += pend_violations(&w, &accts, &m);
    }
    let sl = &w.markets[0].engine;
    let g = |a: &percolator::SourceCreditStateV16Account| (a.positive_claim_bound_num.get() as i128 / 1_000_000_000_000, a.fresh_reserved_backing_num.get() as i128 / 1_000_000_000_000);
    let (c0, b0) = g(&sl.source_credit_long);
    let (c1, b1) = g(&sl.source_credit_short);
    let stranded = (b0 - c0).max(0) + (b1 - c1).max(0);
    let mut eq = vec![];
    for i in 0..n { let mut a = accts[i]; eq.push(effective_equity(&mut w, &mut a) - start[i]); }
    let mut mm = m;
    eq.push(effective_equity(&mut w, &mut mm) - cap as i128);
    Some(Rev { eq, stranded, pend_checks: checks, pend_violations: viol })
}

fn shape(seed: u64) -> (usize, usize) {
    let mut t = seed | 1;
    let nl = 2 + (rng(&mut t) % 3) as usize;
    let ns = 1 + (rng(&mut t) % 2) as usize;
    (nl, ns)
}

/// S9: the counter never exceeds the claim stock, in any world, mask and settle order.
#[test]
fn reversal_pending_credit_never_exceeds_the_claim_stock() {
    let (mut checks, mut viol, mut ran) = (0u64, 0u64, 0u64);
    for i in 0..400u64 {
        let seed = 3u64 * 7919 + i;
        let (nl, ns) = shape(seed);
        let mut s2 = seed ^ 0x777;
        for p in perms(nl + ns + 1, 8, &mut s2) {
            let Some(r) = reversal(seed, &p, None) else { break };
            ran += 1;
            checks += r.pend_checks; viol += r.pend_violations;
            assert!(r.eq.iter().sum::<i128>() <= 8, "never a gain");
        }
    }
    println!("REVPEND runs {ran} checks {checks} violations {viol}");
    assert!(ran > 1000);
    assert_eq!(viol, 0, "pending credit exceeded the claim stock {viol} times in {checks} checks");
}

/// S10, fixed: against the same world with EVERYONE cranked at the peak, a random-mask world ends
/// at the same EFFECTIVE equity for every account (0 of 600 worlds deviate; #282 round 4: 260, the
/// pre-R1 engine: 450), never above it, and no backing is left stranded.
#[test]
fn reversal_cadence_is_exact_in_effective_equity() {
    let (mut ran, mut dev, mut above, mut stranded_worlds) = (0u64, 0u64, 0u64, 0u64);
    for i in 0..600u64 {
        let seed = 3u64 * 7919 + i;
        let (nl, ns) = shape(seed);
        let n = nl + ns;
        let perm: Vec<usize> = (0..=n).collect();
        let Some(r) = reversal(seed, &perm, None) else { continue };
        let Some(ideal) = reversal(seed, &perm, Some((1u64 << (n + 1)) - 1)) else { continue };
        ran += 1;
        let d: Vec<i128> = r.eq.iter().zip(&ideal.eq).map(|(a, b)| a - b).collect();
        if d.iter().any(|x| *x > 8) { above += 1; }
        if d.iter().any(|x| x.abs() > 8) { dev += 1; }
        if r.stranded > 8 { stranded_worlds += 1; }
    }
    println!("REVCAD ran {ran} deviating {dev} above-ideal {above} worlds-with-stranded-backing {stranded_worlds}");
    assert!(ran >= 550);
    assert_eq!(above, 0, "no account ever ends above the all-cranked ideal");
    assert_eq!(dev, 0, "every world ends at the all-cranked ideal in effective equity");
    assert_eq!(stranded_worlds, 0, "no backing is left in excess of claims");
}

/// Every settle order of every crank mask ends at the ideal too (the harness above fixes the
/// settle order to 0..=n; this walks 6 random orders per world for 150 worlds).
#[test]
fn reversal_cadence_is_exact_for_every_settle_order() {
    let (mut ran, mut dev) = (0u64, 0u64);
    for i in 0..150u64 {
        let seed = 3u64 * 7919 + 4_000 + i;
        let (nl, ns) = shape(seed);
        let n = nl + ns;
        let all = (1u64 << (n + 1)) - 1;
        let mut s2 = seed ^ 0x999;
        let Some(ideal) = reversal(seed, &(0..=n).collect::<Vec<_>>(), Some(all)) else { continue };
        for p in perms(n + 1, 6, &mut s2) {
            let Some(r) = reversal(seed, &p, None) else { break };
            ran += 1;
            if r.eq.iter().zip(&ideal.eq).any(|(a, b)| (a - b).abs() > 8) { dev += 1; }
        }
    }
    println!("REVORDER ran {ran} deviating {dev}");
    assert!(ran > 500);
    assert_eq!(dev, 0, "settle order changed an account's effective equity");
}

/// The rebalance moves backing only into a SHORTFALL. A loser that realised its loss while its
/// winners are unsettled leaves backing in its own domain; with no claim anywhere to cover it must
/// stay exactly where it was booked (the winners may still claim it).
#[test]
fn unclaimed_backing_stays_put_without_a_shortfall() {
    let mut w = World::new_pairs(&[], 0, 0);
    let mut long = account(300);
    let mut maker = w.maker;
    w.deposit(&mut long, 1_000_000_000_000_000);
    w.trade(&mut long, &mut maker, 400 * POS_SCALE).expect("long opens against the maker");
    assert!(w.accrue(150, 0) && w.accrue(150, 0) && w.accrue(150, 0));
    // the maker (the loser, short) settles alone: its loss books as backing in the SHORT domain
    assert!(w.settle(&mut maker));
    let sl = &w.markets[0].engine;
    let (sc, ss) = (sl.source_credit_long.try_to_runtime().unwrap(), sl.source_credit_short.try_to_runtime().unwrap());
    assert!(ss.fresh_reserved_backing_num > 0, "the maker's realised loss is backing");
    assert_eq!(ss.positive_claim_bound_num, 0, "the unsettled long has no claim yet");
    assert_eq!((sc.fresh_reserved_backing_num, sc.positive_claim_bound_num), (0, 0), "nothing moved to the long domain");
    // and the long, settling now, claims it in full
    assert!(w.settle(&mut long));
    let sl = &w.markets[0].engine;
    let ss = sl.source_credit_short.try_to_runtime().unwrap();
    assert!(ss.positive_claim_bound_num > 0 && ss.positive_claim_bound_num <= ss.fresh_reserved_backing_num);
}


// ---------------------------------------------------------------------------------------------
// Provider ring-fence (security review F1, repros P and R) and the two guards of the move.
// ---------------------------------------------------------------------------------------------

fn fresh(w: &World, d: usize) -> u128 {
    let sl = &w.markets[0].engine;
    let b = if d == 0 { sl.backing_long } else { sl.backing_short };
    b.try_to_runtime().unwrap().fresh_unliened_backing_num / 1_000_000_000_000
}

fn counter(w: &World, d: usize) -> u128 {
    let sl = &w.markets[0].engine;
    (if d == 0 { sl.loss_booked_unclaimed_long.get() } else { sl.loss_booked_unclaimed_short.get() }) / 1_000_000_000_000
}

/// Repro P: provider principal (50,000,000 atoms in the LONG domain) must not move when a long
/// winner settles before its loser (the ordinary keeper order). Before the ring-fence the winner
/// pulled 18,270,000 of it into the short domain and the provider could withdraw only 31.73M.
#[test]
fn provider_principal_never_moves_when_a_winner_settles_first() {
    let mut w = World::new_pairs(&[], 0, 0);
    let mut t = account(300);
    let mut maker = w.maker;
    w.deposit(&mut t, 1_000_000_000_000_000);
    w.trade(&mut t, &mut maker, 400 * POS_SCALE).expect("open");
    let prov: u128 = 50_000_000;
    {
        let mut m = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
        m.deposit_fresh_counterparty_backing_not_atomic(0, prov, u64::MAX / 2).unwrap();
    }
    assert_eq!((fresh(&w, 0), counter(&w, 0)), (prov, 0), "a deposit is not loser-booked backing");
    for _ in 0..3 { assert!(w.accrue(150, 0)); }
    assert!(w.settle(&mut t)); // the winner settles first
    assert_eq!(fresh(&w, 0), prov, "provider principal stayed in its domain");
    assert_eq!(fresh(&w, 1), 0);
    // the maker pays: its cash is loser-booked backing in the short domain, claimed by the winner
    assert!(w.settle(&mut maker));
    assert_eq!(fresh(&w, 0), prov);
    assert_eq!(counter(&w, 1), fresh(&w, 1), "everything in the short bucket is loser-booked");
    let mut m = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
    m.withdraw_fresh_counterparty_backing_not_atomic(0, prov).expect("the provider withdraws all of its principal");
}

/// Repro R: the same in a Resolved market, both close orders: the provider recovers all 50M.
#[test]
fn provider_principal_is_fully_recoverable_after_resolved_close() {
    for order in [[0usize, 1], [1, 0]] {
        let mut w = World::new_pairs(&[], 0, 0);
        let mut t = account(300);
        let mut maker = w.maker;
        w.deposit(&mut t, 1_000_000_000_000_000);
        w.trade(&mut t, &mut maker, 400 * POS_SCALE).expect("open");
        let prov: u128 = 50_000_000;
        { let mut m = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets); m.deposit_fresh_counterparty_backing_not_atomic(0, prov, u64::MAX / 2).unwrap(); }
        for _ in 0..3 { assert!(w.accrue(150, 0)); }
        assert!(w.settle(&mut t));
        let slot = w.slot + 1;
        let mut m = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
        m.resolve_market_not_atomic(slot).unwrap();
        let mut accts = [t, maker];
        let mut paid = [0u128; 2];
        let mut closed = [false; 2];
        for _ in 0..60 {
            for &i in &order {
                if closed[i] { continue; }
                for d in 0..2 { let _ = m.expire_source_backing_bucket_not_atomic(d, slot); }
                if let Ok(percolator::ResolvedCloseOutcomeV16::Closed { payout }) = m.close_resolved_account_not_atomic(&mut PortfolioV16ViewMut::new(&mut accts[i]), 0) { paid[i] = payout; closed[i] = true; }
            }
            if closed.iter().all(|c| *c) { break; }
        }
        assert!(closed.iter().all(|c| *c), "order {order:?}: both close");
        assert_eq!(paid, [1_000_000_018_271_200, 999_999_981_728_800], "order {order:?}: payouts are the base engine's");
        m.withdraw_fresh_counterparty_backing_not_atomic(0, prov).expect("provider recovers its whole principal");
        assert_eq!(m.header.vault.get(), 0, "order {order:?}: nothing left in the vault");
    }
}

/// The counter never exceeds fresh unliened backing, through settlements, moves and withdrawals
/// (`validate_shape` also enforces it); 300 reversal worlds with 20M of provider backing in each
/// domain, every account settled in every order, checked after every settle.
#[test]
fn loss_booked_counter_never_exceeds_fresh_backing() {
    let (mut checks, mut moved_worlds) = (0u64, 0u64);
    for i in 0..300u64 {
        let seed = 3u64 * 7919 + 9_000 + i;
        let (nl, ns) = shape(seed);
        let n = nl + ns;
        let mut w = World::new_pairs(&[], 0, 0);
        let mut accts: Vec<PortfolioAccountV16Account> = (0..n).map(|k| account(700 + k as u32)).collect();
        let mut s = seed | 1;
        for k in 0..n { let mut a = accts[k]; w.deposit(&mut a, 1_000_000_000_000_000); accts[k] = a; }
        let mut m = w.maker;
        for k in 0..n {
            let u = 1 + (rng(&mut s) % 6) as u128;
            let mut a = accts[k];
            let r = if k < nl { w.trade(&mut a, &mut m, u * 100 * POS_SCALE) } else { w.trade(&mut m, &mut a, u * 100 * POS_SCALE) };
            if r.is_err() { break; }
            accts[k] = a;
        }
        {
            let mut mg = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
            mg.deposit_fresh_counterparty_backing_not_atomic(0, 20_000_000, u64::MAX / 2).unwrap();
            mg.deposit_fresh_counterparty_backing_not_atomic(1, 20_000_000, u64::MAX / 2).unwrap();
        }
        let steps = |w: &mut World, target: u64| { let mut g = 0; while w.price() != target && g < 200 { g += 1; let cur = w.price() as i128; let b = (((target as i128 - cur) * 10_000) / cur).clamp(-190, 190) as i64; let b = if b == 0 { if target as i128 > cur { 1 } else { -1 } } else { b }; if !w.accrue(b, 0) { return false; } } true };
        if !steps(&mut w, 1_080_000 + rng(&mut s) % 100_000) { continue; }
        let mask = rng(&mut s) % (1 << (n + 1));
        let chk = |w: &World, checks: &mut u64| {
            for d in 0..2 { assert!(counter(w, d) <= fresh(w, d) , "counter above fresh backing"); }
            *checks += 1;
        };
        for k in 0..n { if mask >> k & 1 == 1 { let mut a = accts[k]; if !w.settle(&mut a) { break; } accts[k] = a; chk(&w, &mut checks); } }
        if mask >> n & 1 == 1 { let mut mm = m; if w.settle(&mut mm) { m = mm; } chk(&w, &mut checks); }
        if !steps(&mut w, 700_000 + rng(&mut s) % 200_000) { continue; }
        let before = (fresh(&w, 0), fresh(&w, 1));
        for k in 0..=n {
            if k == n { let mut mm = m; if w.settle(&mut mm) { m = mm; } } else { let mut a = accts[k]; if w.settle(&mut a) { accts[k] = a; } }
            chk(&w, &mut checks);
        }
        if (fresh(&w, 0), fresh(&w, 1)) != before { moved_worlds += 1; }
        let mut mg = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
        mg.validate_shape().unwrap();
    }
    println!("RINGCHK checks {checks} worlds-where-fresh-backing-changed {moved_worlds}");
    assert!(checks > 1_000);
}

/// A world where the maker (the loser, short) settled alone at the peak and the price then fell
/// below entry; the longs are settled; only the maker's recovery settle is left. Returns the world,
/// the maker and the longs. `h_max` sets the backing horizon.
fn stranded_world(h_max: u64) -> (World, PortfolioAccountV16Account, PortfolioAccountV16Account) {
    let mut w = World::new_pairs_hmax(&[], 0, 0, PRICE, RATE_E9 as u64, h_max);
    let mut long = account(300);
    let mut maker = w.maker;
    w.deposit(&mut long, 1_000_000_000_000_000);
    w.trade(&mut long, &mut maker, 400 * POS_SCALE).expect("open");
    for _ in 0..3 { assert!(w.accrue(150, 0)); }
    assert!(w.settle(&mut maker)); // the loser realises its peak loss alone
    for _ in 0..8 { assert!(w.accrue(-150, 0)); }
    assert!(w.refresh(&mut long)); // the winner's net is a loss: no claim ever (refresh: no expiry sweep)
    (w, maker, long)
}

/// Guard 1 (source expiry): a Fresh source bucket past its expiry must neither move nor make the
/// settlement fail (`prepare_counterparty_backing_withdraw_delta` returns LockActive on it).
#[test]
fn the_move_skips_an_expired_source_bucket_and_the_settle_still_succeeds() {
    let (mut w, mut maker, _long) = stranded_world(5);
    let sl = &w.markets[0].engine;
    let b = sl.backing_short.try_to_runtime().unwrap();
    assert_eq!(b.status, percolator::BackingBucketStatusV16::Fresh);
    assert!(b.expiry_slot <= w.slot, "the source bucket's horizon has passed (expiry {} slot {})", b.expiry_slot, w.slot);
    let before = fresh(&w, 1);
    assert!(before > 0 && counter(&w, 1) > 0);
    assert!(w.refresh(&mut maker), "the recovery settle must succeed with a lapsed source bucket");
    assert_eq!(fresh(&w, 1), before, "nothing moved out of a lapsed bucket");
}

/// The same world with a live horizon moves the stranded backing, so the control above is not vacuous.
#[test]
fn the_move_fires_in_the_same_world_with_a_live_source_bucket() {
    let (mut w, mut maker, _long) = stranded_world(6_480_000);
    let before = fresh(&w, 1);
    assert!(before > 0);
    // literal atoms (not read from the state under test): the maker's peak loss 18,271,200 sits
    // in the short bucket, the longs' final loss 29,362,400 in the long bucket
    assert_eq!((fresh(&w, 1), fresh(&w, 0), counter(&w, 1), counter(&w, 0)), (18_271_200, 29_362_400, 18_271_200, 29_362_400));
    assert!(w.refresh(&mut maker));
    // the maker's recovery claim is 47,633,600 = 18,271,200 + 29,362,400: all of the stranded
    // backing moved, the claim is backed exactly, nothing is left claimant-less
    assert_eq!((fresh(&w, 1), fresh(&w, 0), counter(&w, 1), counter(&w, 0)), (0, 47_633_600, 0, 47_633_600));
    let sl = &w.markets[0].engine;
    assert_eq!(sl.source_credit_long.try_to_runtime().unwrap().positive_claim_bound_num, 47_633_600 * 1_000_000_000_000);
    assert_eq!(sl.source_credit_short.try_to_runtime().unwrap().positive_claim_bound_num, 0);
    assert!(fresh(&w, 1) < before, "stranded loser-booked backing moved to cover the recovery claim");
    assert!(fresh(&w, 0) > 0);
    assert_eq!(counter(&w, 0), fresh(&w, 0), "what moved is loser-booked backing in its new domain");
}

/// Guard 2 (destination acceptance): a destination bucket that is Fresh but past its expiry does
/// not accept a booking (`prepare_counterparty_backing_add_delta` would return LockActive); the
/// settlement must still succeed and nothing may move.
#[test]
fn the_move_skips_a_destination_that_does_not_accept_backing() {
    let (mut w, mut maker, _long) = stranded_world(6_480_000);
    // the longs' own loss sits in the long bucket (the destination); let its horizon pass while it
    // keeps status Fresh (a lapse is only a status change once something runs the expiry sweep)
    let mut bk = w.markets[0].engine.backing_long.try_to_runtime().unwrap();
    assert_eq!(bk.status, percolator::BackingBucketStatusV16::Fresh);
    bk.expiry_slot = 1;
    w.markets[0].engine.backing_long = percolator::BackingBucketV16Account::from_runtime(&bk);
    // a claim shortfall in the destination (as the maker's recovery would leave), and the move
    // driven directly: the lapsed bucket makes the claimant's own settle Stale, the guard is the move's
    let mut sc = w.markets[0].engine.source_credit_long.try_to_runtime().unwrap();
    let claims = sc.fresh_reserved_backing_num + 5_000_000 * 1_000_000_000_000;
    sc.positive_claim_bound_num = claims;
    sc.exact_positive_claim_num = claims;
    w.markets[0].engine.source_credit_long = percolator::SourceCreditStateV16Account::from_runtime(&sc);
    let _ = &mut maker;
    let before = (fresh(&w, 0), fresh(&w, 1));
    {
        let mut m = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
        m.rebalance_unclaimed_backing_for_test_not_atomic(0).expect("the move must not fail the caller");
    }
    assert_eq!((fresh(&w, 0), fresh(&w, 1)), before, "nothing moved into a destination that cannot accept it");
}

/// Ring-fence in the SOURCE domain with the V1 guard satisfied: 20M of provider principal sits in
/// the short bucket next to the maker's loser-booked backing; when the recovery claim triggers the
/// move, only the loser-booked part may go, and the provider can still withdraw all of its 20M.
#[test]
fn the_move_takes_only_loser_booked_backing_from_a_bucket_that_also_holds_provider_principal() {
    let mut w = World::new_pairs(&[], 0, 0);
    let mut long = account(300);
    let mut maker = w.maker;
    w.deposit(&mut long, 1_000_000_000_000_000);
    w.trade(&mut long, &mut maker, 400 * POS_SCALE).expect("open");
    let prov: u128 = 20_000_000;
    {
        let mut m = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
        m.deposit_fresh_counterparty_backing_not_atomic(1, prov, u64::MAX / 2).unwrap();
    }
    for _ in 0..3 { assert!(w.accrue(150, 0)); }
    assert!(w.settle(&mut maker));
    let booked = counter(&w, 1);
    assert!(booked > 0);
    assert_eq!(fresh(&w, 1), prov + booked, "provider principal and loser-booked backing share the bucket");
    for _ in 0..8 { assert!(w.accrue(-150, 0)); }
    assert!(w.settle(&mut long));
    assert!(w.settle(&mut maker)); // the recovery claim triggers the move
    assert!(fresh(&w, 1) >= prov, "provider principal stayed ({} < {prov})", fresh(&w, 1));
    // a far larger shortfall in the long domain asks for more than the loser-booked part holds
    let mut sc = w.markets[0].engine.source_credit_long.try_to_runtime().unwrap();
    let claims = sc.fresh_reserved_backing_num + 40_000_000 * 1_000_000_000_000;
    sc.positive_claim_bound_num = claims;
    sc.exact_positive_claim_num = claims;
    sc.credit_rate_num = (sc.fresh_reserved_backing_num as u128) * percolator::CREDIT_RATE_SCALE / claims;
    w.markets[0].engine.source_credit_long = percolator::SourceCreditStateV16Account::from_runtime(&sc);
    {
        let mut m = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
        m.rebalance_unclaimed_backing_for_test_not_atomic(0).unwrap();
    }
    assert!(fresh(&w, 1) >= prov, "an oversized shortfall still leaves the provider's principal ({} < {prov})", fresh(&w, 1));
    assert!(counter(&w, 1) <= fresh(&w, 1) - prov);
    let mut m = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
    m.withdraw_fresh_counterparty_backing_not_atomic(1, prov).expect("the provider withdraws all of its principal");
}
