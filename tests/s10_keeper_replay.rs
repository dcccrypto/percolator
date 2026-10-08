//! S10 live-exposure replay: the REAL keeper settle pattern over oscillating price paths.
//! Measures how much the LP (maker) ends below the "nobody settled mid-path" ideal (= the
//! all-cranked ideal, see finding-r1-equity-cadence Round 4) per policy, in atoms and bps of OI.
//! Env: POLICY=lp_alone|cohort|sweep|lp_alone_sweep CYCLE (ticks per recovery cycle) NTR SWEEP_CAP
//!      LEGS LEGLEN AMPBPS (per-leg move, bps) SEEDS PATHS
#![allow(dead_code, unused_imports, unused_mut, clippy::needless_range_loop, clippy::type_complexity)]
use proptest as _;
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
        let mut m = MarketGroupV16ViewMut::new_crank(&mut self.header, &mut self.markets);
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

fn rng(s: &mut u64) -> u64 { *s ^= *s << 13; *s ^= *s >> 7; *s ^= *s << 17; *s }
fn envn(k: &str, d: u64) -> u64 { std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d) }

fn effective_equity(w: &mut World, a: &mut PortfolioAccountV16Account) -> i128 {
    assert!(w.refresh(a), "refresh for certification");
    let cert = a.health_cert.try_to_runtime().expect("cert decodes");
    assert!(cert.valid);
    cert.certified_equity
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Policy {
    /// no mid-path settle at all (the ideal; lazy)
    Ideal,
    /// recovery cranker: every CYCLE ticks, ALL traders then the LP, same K
    Cohort,
    /// VaultLpCranker: the LP alone after EVERY tick; the cohort only every CYCLE ticks
    LpAlone,
    /// v2.2 sweep: every CYCLE ticks the LP then the SWEEP_CAP heaviest traders (heaviest first,
    /// the same set each time: the heaviest are stale again after every move)
    Sweep,
    /// v2.2 sweep plus the vault-LP per-tick crank
    LpAloneSweep,
    /// v2.2 sweep as shipped in PR #147: every CYCLE ticks the LP is settled and the next SWEEP_CAP
    /// stale traders of a heaviest-first ROUND (each portfolio once per round, then the round
    /// restarts), so a trader is settled every ceil(NTR / SWEEP_CAP) cycles
    SweepRound,
    /// SweepRound plus the vault-LP per-tick crank
    LpAloneSweepRound,
    /// every tick every account (LP included) settles independently with probability RANDP per mille
    Random,
}

struct Out {
    dbg: String,
    lp: i128,
    traders: Vec<i128>,
    oi: u128,
    stranded: i128,
    ran: bool,
}

/// One path. Traders: NTR accounts, random side, heavy-tailed size; the rich maker takes the
/// other side of everything. Price path: LEGS legs of LEGLEN ticks, each moving AMP bps (+/-
/// alternating, amplitude jittered), one `accrue` per tick (<= 190 bps per tick).
fn run(seed: u64, policy: Policy, cycle: u64) -> Out {
    let none = Out { dbg: String::new(), lp: 0, traders: vec![], oi: 0, stranded: 0, ran: false };
    let mut s = seed | 1;
    let ntr = envn("NTR", 12) as usize;
    let sweep_cap = envn("SWEEP_CAP", 10) as usize;
    let legs = envn("LEGS", 6);
    let leglen = envn("LEGLEN", 10);
    let amp = envn("AMPBPS", 300) as i64;
    let mut w = World::new_pairs(&[], 0, 0);
    let mut accts: Vec<PortfolioAccountV16Account> = (0..ntr).map(|i| account(500 + i as u32)).collect();
    let mut sizes = vec![0u128; ntr];
    let mut sides = vec![true; ntr];
    let trader_cap = envn("TCAP", 4_000_000_000) as u128 * if std::env::var("PATHFILE").is_ok() { 25 } else { 1 };
    for i in 0..ntr {
        let mut a = accts[i];
        w.deposit(&mut a, trader_cap);
        accts[i] = a;
    }
    let mut m = w.maker;
    let mut oi = 0u128;
    for i in 0..ntr {
        // heavy tail: 1,1,2,2,3,5,8 units
        let u = [1u128, 1, 2, 2, 3, 5, 8][(rng(&mut s) % 7) as usize];
        let long = rng(&mut s) % 2 == 0;
        let mut a = accts[i];
        let sz = u * 100 * POS_SCALE / 4;
        let r = if long { w.trade(&mut a, &mut m, sz) } else { w.trade(&mut m, &mut a, sz) };
        if r.is_err() { return none; }
        accts[i] = a;
        sizes[i] = sz;
        sides[i] = long;
        oi += u * 100 * U / 4 * 4 / 4; // notional atoms, price 1e6 x size/POS_SCALE
    }
    let start: Vec<i128> = accts.iter().map(|a| a.capital.get() as i128 + a.pnl.get()).collect();
    let start_m = m.capital.get() as i128 + m.pnl.get();
    // heaviest first order of the cohort
    let mut by_weight: Vec<usize> = (0..ntr).collect();
    by_weight.sort_by(|a, b| sizes[*b].cmp(&sizes[*a]));
    // per-tick price targets: a real mark window (PATHFILE) or a synthetic zigzag
    let tpm = envn("TPM", 8) as usize; // ticks per minute of the real path
    let targets: Vec<u64> = if let Ok(f) = std::env::var("PATHFILE") {
        let txt = std::fs::read_to_string(&f).expect("pathfile");
        let closes: Vec<f64> = txt.lines().filter_map(|l| l.trim().parse::<f64>().ok()).filter(|c| *c > 0.0).collect();
        let win = envn("WINMIN", 180) as usize;
        let nwin = (closes.len().saturating_sub(1)) / win;
        let k = (((seed - 1) / 977) as usize).saturating_sub(1) % nwin.max(1);
        let base = closes[k * win];
        let p0 = w.price() as f64;
        let mut out = vec![];
        for m_i in 0..win {
            let (c0, c1) = (closes[k * win + m_i], closes[k * win + m_i + 1]);
            for t in 1..=tpm {
                let c = c0 + (c1 - c0) * t as f64 / tpm as f64;
                out.push((p0 * c / base).round().max(1.0) as u64);
            }
        }
        out
    } else {
        let mut out = vec![];
        let mut px = w.price() as i128;
        for l in 0..legs {
            let a = amp * (50 + (rng(&mut s) % 100) as i64) / 100;
            let dir = if l % 2 == 0 { 1 } else { -1 };
            let tgt = px + px * (dir * a) as i128 / 10_000;
            for t in 1..=leglen {
                out.push((px + (tgt - px) * t as i128 / leglen as i128) as u64);
            }
            px = tgt;
        }
        out
    };
    let mut tick = 0u64;
    let mut queue: Vec<usize> = vec![];
    for tgt in targets {
        // walk to the tick's target (<= 190 bps per accrual, as the engine's move cap requires)
        let mut g = 0;
        while w.price() != tgt && g < 400 {
            g += 1;
            let cur = w.price() as i128;
            let mut b = ((tgt as i128 - cur) * 10_000) / cur;
            b = b.clamp(-190, 190);
            if b == 0 { b = if tgt as i128 > cur { 1 } else { -1 }; }
            if !w.accrue(b as i64, 0) { return none; }
        }
        tick += 1;
        let trace = std::env::var("TRACE").ok().and_then(|v| v.parse::<u64>().ok()) == Some(seed);
        let lp_each_tick = matches!(policy, Policy::LpAlone | Policy::LpAloneSweep | Policy::LpAloneSweepRound);
        if lp_each_tick {
            let mut mm = m;
            if !w.settle(&mut mm) { return none; }
            m = mm;
            if trace { println!("T{tick} LP    {} lp_cap {} lp_pnl {}", dom_dbg(&w), m.capital.get(), m.pnl.get()); }
        }
        // user trades: a trade settles its two parties (the trader and the LP) at the K of this tick
        let tradep = envn("TRADEP", 0);
        if tradep > 0 && policy != Policy::Ideal && rng(&mut s) % 1000 < tradep {
            let i = (rng(&mut s) % ntr as u64) as usize;
            let mut a = accts[i];
            if !w.settle(&mut a) { return none; }
            accts[i] = a;
            let mut mm = m;
            if !w.settle(&mut mm) { return none; }
            m = mm;
        }
        if policy == Policy::Random {
            let q = envn("RANDP", 250);
            for i in 0..ntr { if rng(&mut s) % 1000 < q { let mut a = accts[i]; let f0 = a.capital.get() as i128 + a.pnl.get(); if !w.settle(&mut a) { return none; } accts[i] = a; if trace { println!("T{tick} tr{i} long={} {} face {} -> {}", sides[i], dom_dbg(&w), f0, a.capital.get() as i128 + a.pnl.get()); } } }
            if rng(&mut s) % 1000 < q { let mut mm = m; let f0 = m.capital.get() as i128 + m.pnl.get(); if !w.settle(&mut mm) { return none; } m = mm; if trace { println!("T{tick} LP {} face {} -> {}", dom_dbg(&w), f0, m.capital.get() as i128 + m.pnl.get()); } }
        }
        if (tick + seed) % cycle == 0 {
            if trace { println!("T{tick} COHORT"); }
            match policy {
                Policy::Ideal | Policy::Random => {}
                Policy::Cohort | Policy::LpAlone => {
                    for i in 0..ntr { let mut a = accts[i]; if !w.settle(&mut a) { return none; } accts[i] = a; if trace { println!("T{tick}  tr{i}  {} cap {} pnl {}", dom_dbg(&w), accts[i].capital.get(), accts[i].pnl.get()); } }
                    let mut mm = m;
                    if !w.settle(&mut mm) { return none; }
                    m = mm;
                    if trace { println!("T{tick}  LP    {}", dom_dbg(&w)); }
                }
                Policy::Sweep | Policy::LpAloneSweep => {
                    let mut mm = m;
                    if !w.settle(&mut mm) { return none; }
                    m = mm;
                    if trace { println!("T{tick}  LP    {} lp_cap {} pnl {}", dom_dbg(&w), m.capital.get(), m.pnl.get()); }
                    for &i in by_weight.iter().take(sweep_cap) { let mut a = accts[i]; if !w.settle(&mut a) { return none; } accts[i] = a; if trace { println!("T{tick}  tr{i} side_long={} {} cap {} pnl {}", sides[i], dom_dbg(&w), accts[i].capital.get(), accts[i].pnl.get()); } }
                }
                Policy::SweepRound | Policy::LpAloneSweepRound => {
                    let mut mm = m;
                    if !w.settle(&mut mm) { return none; }
                    m = mm;
                    if queue.is_empty() { queue = by_weight.clone(); queue.reverse(); }
                    for _ in 0..sweep_cap {
                        let Some(i) = queue.pop() else { break };
                        let mut a = accts[i];
                        if !w.settle(&mut a) { return none; }
                        accts[i] = a;
                    }
                }
            }
        }
    }
    // end: everyone settles once (traders, then the maker) = the final lazy settle
    if std::env::var("TRACE").ok().and_then(|v| v.parse::<u64>().ok()) == Some(seed) { println!("END {}", dom_dbg(&w)); }
    for i in 0..ntr { let mut a = accts[i]; if !w.settle(&mut a) { return none; } accts[i] = a; }
    let mut mm = m;
    if !w.settle(&mut mm) { return none; }
    m = mm;
    let sl = &w.markets[0].engine;
    let g = |a: &percolator::SourceCreditStateV16Account| (a.positive_claim_bound_num.get() as i128 / 1_000_000_000_000, a.fresh_reserved_backing_num.get() as i128 / 1_000_000_000_000);
    let (c0, b0) = g(&sl.source_credit_long);
    let (c1, b1) = g(&sl.source_credit_short);
    let stranded = (b0 - c0).max(0) + (b1 - c1).max(0);
    let mut traders = vec![];
    for i in 0..ntr { let mut a = accts[i]; traders.push(effective_equity(&mut w, &mut a) - start[i]); }
    let mut mm2 = m;
    let lp = effective_equity(&mut w, &mut mm2) - start_m;
    let dbg = format!("sides {:?} sizes {:?} dom0(long) claims {} backing {} dom1(short) claims {} backing {} traders {:?} lp {}", sides, sizes.iter().map(|x| x / (POS_SCALE/4)).collect::<Vec<_>>(), c0, b0, c1, b1, traders, lp);
    Out { dbg, lp, traders, oi, stranded, ran: true }
}

fn dom_dbg(w: &World) -> String {
    let sl = &w.markets[0].engine;
    let f = |a: &percolator::SourceCreditStateV16Account| (a.positive_claim_bound_num.get() as i128 / 1_000_000_000_000, a.fresh_reserved_backing_num.get() as i128 / 1_000_000_000_000);
    let (c0, b0) = f(&sl.source_credit_long);
    let (c1, b1) = f(&sl.source_credit_short);
    format!("px {} | long-dom claims {} backing {} pend {} | short-dom claims {} backing {} pend {}", w.price(), c0, b0, sl.kf_pending_credit_long.get() / 1_000_000_000_000, c1, b1, sl.kf_pending_credit_short.get() / 1_000_000_000_000)
}

fn pol(name: &str) -> Policy {
    match name { "cohort" => Policy::Cohort, "lp_alone" => Policy::LpAlone, "sweep" => Policy::Sweep, "lp_alone_sweep" => Policy::LpAloneSweep, "sweep_round" => Policy::SweepRound, "random" => Policy::Random, "lp_alone_sweep_round" => Policy::LpAloneSweepRound, _ => Policy::Cohort }
}

struct Meas { ran: u64, dev: u64, above: u64, sum_lp: i128, sum_tr: i128, worst_lp: i128, sum_str: i128, sum_oi: u128 }

fn measure(name: &str, paths: u64, cycle: u64) -> Meas {
    let p = pol(name);
    let mut m = Meas { ran: 0, dev: 0, above: 0, sum_lp: 0, sum_tr: 0, worst_lp: 0, sum_str: 0, sum_oi: 0 };
    for i in 0..paths {
        let seed = 977 * (i + 1) + envn("SEED", 1);
        let ideal = run(seed, Policy::Ideal, cycle);
        let r = run(seed, p, cycle);
        if !(ideal.ran && r.ran) { continue; }
        m.ran += 1;
        let d_lp = r.lp - ideal.lp;
        let d_tr: i128 = r.traders.iter().zip(&ideal.traders).map(|(a, b)| a - b).sum();
        if std::env::var("DUMP").is_ok() { println!("W {seed} {} {}", d_lp, r.traders.iter().zip(&ideal.traders).map(|(a, b)| (a - b).to_string()).collect::<Vec<_>>().join(" ")); }
        if std::env::var("DUMP2").is_ok() { let tot = d_lp + d_tr; println!("X {seed} total {tot} stranded {} {}", r.stranded, r.dbg.split(" traders").next().unwrap_or("")); }
        if d_lp.abs() > 8 { m.dev += 1; if std::env::var("DEBUG").is_ok() && m.dev <= 3 { println!("DBG seed {seed}\n ideal: {}\n actual: {}", ideal.dbg, r.dbg); } }
        if d_lp > 8 || r.traders.iter().zip(&ideal.traders).any(|(a, b)| a - b > 8) { m.above += 1; }
        m.sum_lp += d_lp; m.sum_tr += d_tr; m.sum_oi += ideal.oi; m.sum_str += r.stranded;
        m.worst_lp = m.worst_lp.min(d_lp);
    }
    m
}

/// Measurement harness (env driven, see the module header). `cargo test --release --test
/// s10_keeper_replay -- --ignored --nocapture`.
#[test]
#[ignore = "measurement harness, driven by environment variables"]
fn s10_keeper_replay() {
    let paths = envn("PATHS", 200);
    let cycle = envn("CYCLE", 4);
    let name = std::env::var("POLICY").unwrap_or_else(|_| "lp_alone".into());
    let m = measure(&name, paths, cycle);
    let bps = if m.sum_oi == 0 { 0.0 } else { m.sum_lp as f64 * 10_000.0 / m.sum_oi as f64 };
    let winmin = envn("WINMIN", 180) as f64;
    let per_day = if std::env::var("PATHFILE").is_ok() { bps * 1440.0 / winmin } else { f64::NAN };
    println!("S10DAY policy={name} lp_loss_bps_of_oi_per_day={per_day:.3} traders_delta_bps_of_oi_per_window={:.3}", m.sum_tr as f64 * 10_000.0 / m.sum_oi.max(1) as f64);
    println!(
        "S10REPLAY policy={name} cycle={cycle} ran={} lp_deviating={} above_ideal={} sum_lp_delta={} sum_traders_delta={} worst_lp={} sum_stranded={} sum_oi={} lp_loss_bps_of_oi_per_path={bps:.3}",
        m.ran, m.dev, m.above, m.sum_lp, m.sum_tr, m.worst_lp, m.sum_str, m.sum_oi
    );
    assert!(m.ran > 0);
}

/// The live keeper patterns over synthetic zigzag paths (12 traders, 6 legs of 10 ticks, 300 bps),
/// measured against the same world with nobody settled mid-path. Pre-fix (88a58606): the LP is
/// settled alone every tick -> 112 of 150 paths leak, -13.5 bps of OI per path; the v2.2 sweep
/// (10 heaviest per cycle) 33 paths, -2.2 bps. After the S10 fix the LP-alone pattern is exact.
/// The partial-cohort sweeps keep an R1-class residual (claims burned at a transient rate), far
/// smaller, ratcheted. Nobody ever ends above the ideal.
#[test]
fn s10_keeper_settle_patterns_do_not_leak_to_the_ideal() {
    for k in ["NTR", "LEGS", "LEGLEN", "AMPBPS", "TRADEP", "PATHFILE", "SWEEP_CAP"] { std::env::remove_var(k); }
    let cohort = measure("cohort", 150, 4);
    assert_eq!((cohort.ran, cohort.dev, cohort.above, cohort.sum_lp), (150, 0, 0, 0), "recovery cohort refresh");
    let lp_alone = measure("lp_alone", 150, 4);
    assert_eq!(lp_alone.ran, 150);
    assert_eq!((lp_alone.dev, lp_alone.above), (0, 0), "LP settled alone every tick, cohort every cycle");
    assert_eq!(lp_alone.sum_lp, 0);
    for (name, max_dev, max_loss) in [("sweep", 25u64, 3_000_000i128), ("lp_alone_sweep", 30, 3_000_000)] {
        let m = measure(name, 150, 4);
        println!("S10SWEEP {name} dev {} sum_lp {} sum_tr {}", m.dev, m.sum_lp, m.sum_tr);
        assert_eq!(m.above, 0, "{name}: nobody above the ideal");
        assert!(m.dev <= max_dev, "{name}: deviating paths {} > {max_dev}", m.dev);
        assert!(m.sum_lp > -max_loss, "{name}: LP deficit {}", m.sum_lp);
    }
    // V1 pin: worlds where the pre-fix engine is exact and an unguarded move (no stale-count
    // guard) strands the LP or a trader (113337: LP -976k, 135806: trader -181k). Random-cadence,
    // 3 traders, 3 legs of 6 ticks, 150 per mille per tick.
    std::env::set_var("NTR", "3"); std::env::set_var("LEGS", "3"); std::env::set_var("LEGLEN", "6");
    std::env::set_var("AMPBPS", "300"); std::env::set_var("RANDP", "150");
    for seed in [113_337u64, 135_806] {
        let ideal = run(seed, Policy::Ideal, 4);
        let r = run(seed, Policy::Random, 4);
        assert!(ideal.ran && r.ran, "world {seed} runs");
        assert_eq!(r.lp, ideal.lp, "world {seed}: LP exact");
        assert_eq!(r.traders, ideal.traders, "world {seed}: traders exact");
    }
    for k in ["NTR", "LEGS", "LEGLEN", "AMPBPS", "RANDP"] { std::env::remove_var(k); }
}
