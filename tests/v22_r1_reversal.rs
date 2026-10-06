//! Randomised NON-flipping reversal worlds (the #457 shape): poor and rich longs, one or two
//! shorts, a rich maker on the other side of all of them, a random subset cranked at the price
//! peak, then the reversal below the entry, then the accounts settled in a permutation.
//!
//! * S9: the pending-credit counter never exceeds the claim stock, after every settle.
//! * S10: equity is NOT cadence invariant here (the same in the pre-R1 engine, which deviates in
//!   more worlds). The deviation is exactly the backing a loser's REALISED peak loss left in a
//!   domain with no claimant (the unsettled winners' gain reversed before they settled), and
//!   it falls on the loser side (the maker, mostly). Characterised and ratcheted, not fixed.
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
        let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 6_480_000);
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

/// S10, characterised: against the same world with EVERYONE cranked at the peak, a random-mask
/// world never ends above it, and whatever it falls short by is exactly the backing left stranded
/// in a domain with no claimant (so the value is not lost by the engine, it is unowned). The loss
/// falls on the loser side. The deviating-world count is ratcheted (pre-R1: 450 of 600).
#[test]
fn reversal_cadence_deviation_equals_stranded_backing() {
    let (mut ran, mut dev, mut eq_strand, mut above, mut longs_lose) = (0u64, 0u64, 0u64, 0u64, 0u64);
    let (mut maker_loss, mut short_loss, mut tot_loss) = (0i128, 0i128, 0i128);
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
        if d.iter().any(|x| x.abs() > 8) {
            dev += 1;
            let tot: i128 = d.iter().sum();
            tot_loss += tot;
            if (tot + r.stranded).abs() <= 64 { eq_strand += 1; }
            if d[..nl].iter().any(|x| *x < -8) { longs_lose += 1; }
            short_loss += d[nl..n].iter().sum::<i128>();
            maker_loss += d[n];
        }
    }
    println!("REVCAD ran {ran} deviating {dev} deficit==stranded {eq_strand} above-ideal {above} worlds-where-a-long-loses {longs_lose} total {tot_loss} maker {maker_loss} shorts {short_loss}");
    assert!(ran >= 550);
    assert_eq!(above, 0, "no account ever ends above the all-cranked ideal");
    assert_eq!(longs_lose, 0, "the winners-side longs never lose to this");
    assert_eq!(eq_strand, dev, "every deviation equals the stranded backing");
    assert!(dev <= 260, "deviating worlds regressed: {dev} (pre-R1 450)");
    assert!(maker_loss + short_loss == tot_loss, "the loss is on the maker and the shorts only");
}
