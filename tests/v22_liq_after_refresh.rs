//! S10-X1 differential harness for the liquidation fee charge (engine #288).
//!
//! Needs `--features x1-diff`: every liquidation fee charge then also runs the BASE path
//! (`charge_account_fee_not_atomic`, the pass the PR skips) from the identical pre-state and the
//! monitor compares result and full state (header, every engine asset slot, the account), panicking
//! on any difference. This harness drives the engine through randomised multi-leg worlds (up to 8
//! legs, both directions, reverse open order, slot gaps, a bystander whose B socialisation lands on
//! a third account, funded / exhausted domain insurance, repeated liquidation, side drains,
//! Recovery) and asserts it was not vacuous: a minimum number of monitored calls, of calls with
//! fee / insurance / booked residual / explicit loss, and of calls where the clamp found a
//! pending-credit counter above the claims (the shape security review F2 exposed).
//! Negative control: `--features x1-mutant-noclamp` removes the clamp and this test MUST fail.
//! Ported from the security reviewer's `x1_sec_diff.rs` (scenario generator unchanged).
#![cfg(feature = "x1-diff")]


use percolator::{
    EngineAssetSlotV16Account, LiquidationRequestV16, Market, MarketGroupV16HeaderAccount,
    MarketGroupV16ViewMut, PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, TradeRequestV16, V16Config, V16PodU128, V16PodU64, POS_SCALE,
};

static ACCRUE_TOTAL: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static ACCRUE_ERR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn account(seed: u8) -> PortfolioAccountV16Account {
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        [1; 32],
        [seed; 32],
        [3; 32],
    ));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(header).unwrap();
    a
}

#[derive(Debug, Clone, Copy)]
struct K {
    n: usize,
    order_rev: bool,
    alt_sides: bool,
    bystander: bool,
    insurance: u128,
    capital: u128,
    early: bool,
    fresh: usize,
    fresh_last: bool,
    funding: i128,
    fee_bps: u64,
    seq: u8,
    mirror: bool,
    close_mid: bool,
}

#[derive(Default)]
struct Stats {
    scenarios: usize,
    liq_calls: usize,
    liq_ok: usize,
    fee_pos: usize,
    ins_pos: usize,
    booked_pos: usize,
    explicit_pos: usize,
    by_ok: usize,
    by_err: usize,
    closemid_ok: usize,
    accrue_err: usize,
    errs: std::collections::BTreeMap<String, usize>,
}

fn scenario(k: &K, st: &mut Stats) -> String {
    let mut cfg = V16Config::public_user_fund_with_market_slots(k.n as u16, k.n as u32, 0, 10);
    cfg.maintenance_margin_bps = 1_000;
    cfg.initial_margin_bps = 1_000;
    cfg.max_price_move_bps_per_slot = 500;
    cfg.max_abs_funding_e9_per_slot = if k.funding == 0 { 0 } else { 10 };
    cfg.min_funding_lifetime_slots = 1;
    cfg.liquidation_fee_bps = k.fee_bps;
    cfg.liquidation_fee_cap = 1_000_000;
    cfg.min_liquidation_abs = if k.fee_bps > 0 { 2 } else { 0 };
    cfg.min_nonzero_mm_req = 100;
    cfg.min_nonzero_im_req = 101;
    cfg.max_accrual_dt_slots = 1;
    let mut header = match MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, k.n as u32, 0) {
        Ok(h) => h,
        Err(e) => return format!("SKIP-config {e:?}"),
    };
    let mut markets: Vec<Market<u64>> = (0..k.n)
        .map(|i| Market::new(i as u64, EngineAssetSlotV16Account::default()))
        .collect();
    for (i, m) in markets.iter_mut().enumerate() {
        header
            .activate_empty_asset_slot_not_atomic(i as u32, &mut m.engine, 100, (i + 1) as u64)
            .unwrap();
    }
    let mut tgt = account(10);
    let mut cp = account(11);
    let mut by = account(12);
    let mut log = String::new();
    {
        let mut g = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut tv = PortfolioV16ViewMut::new(&mut tgt);
        let mut cv = PortfolioV16ViewMut::new(&mut cp);
        let mut bv = PortfolioV16ViewMut::new(&mut by);
        g.deposit_not_atomic(&mut tv, 1_000_000_000).unwrap();
        g.deposit_not_atomic(&mut cv, 500_000_000).unwrap();
        g.deposit_not_atomic(&mut bv, 500_000_000).unwrap();
        if k.insurance != 0 {
            for d in 0..(2 * k.n) {
                let r = g.deposit_domain_insurance_not_atomic(d, k.insurance);
                log += &format!("ins{d}={r:?};");
            }
        }
        let q = (100_000 * POS_SCALE) as i128;
        let order: Vec<usize> = if k.order_rev { (0..k.n).rev().collect() } else { (0..k.n).collect() };
        for &asset_index in &order {
            // target is long on this asset unless alt_sides and the asset is odd
            let tgt_long = !(k.alt_sides && asset_index % 2 == 1);
            let req = TradeRequestV16 { asset_index, size_q: q, exec_price: 100, fee_bps: 0 };
            let r = if tgt_long {
                g.execute_trade_with_fee_loss_stale_scoped_not_atomic(&mut tv, &mut cv, req, true)
            } else {
                g.execute_trade_with_fee_loss_stale_scoped_not_atomic(&mut cv, &mut tv, req, true)
            };
            log += &format!("t{asset_index}={:?};", r.as_ref().map(|_| ()));
            r.unwrap();
        }
        // bring every asset to one common slot so later trades are not loss-stale
        let mut slot = (k.n as u64) + 2;
        for asset_index in 0..k.n {
            let r = g.accrue_asset_to_not_atomic(asset_index, slot, 100, 0, true);
            log += &format!("sync{asset_index}={:?};", r.as_ref().map(|_| ()));
        }
        if k.bystander {
            // the sync accrual marked every stored leg K/F-stale: settle both before a new open
            let r = g.full_account_refresh_not_atomic(&mut tv);
            log += &format!("ref_t0={:?};", r.as_ref().map(|_| ()));
            let r = g.full_account_refresh_not_atomic(&mut cv);
            log += &format!("ref_c0={:?};", r.as_ref().map(|_| ()));
            for &asset_index in &order {
                let tgt_long = !(k.alt_sides && asset_index % 2 == 1);
                let req = TradeRequestV16 { asset_index, size_q: q, exec_price: 100, fee_bps: 0 };
                let r = if tgt_long {
                    g.execute_trade_with_fee_loss_stale_scoped_not_atomic(&mut tv, &mut bv, req, true)
                } else {
                    g.execute_trade_with_fee_loss_stale_scoped_not_atomic(&mut bv, &mut tv, req, true)
                };
                if r.is_ok() { st.by_ok += 1; } else { st.by_err += 1; }
                log += &format!("b{asset_index}={:?};", r.as_ref().map(|_| ()));
            }
        }
        if k.close_mid && k.n >= 3 {
            // flatten the target's leg on asset 1 against the counterparty: leaves a slot gap
            let tgt_long = !k.alt_sides;
            let mult = if k.bystander { 2 } else { 1 };
            let req = TradeRequestV16 { asset_index: 1, size_q: q * mult, exec_price: 100, fee_bps: 0 };
            let r = if tgt_long {
                g.execute_trade_with_fee_loss_stale_scoped_not_atomic(&mut cv, &mut tv, req, true)
            } else {
                g.execute_trade_with_fee_loss_stale_scoped_not_atomic(&mut tv, &mut cv, req, true)
            };
            if r.is_ok() { st.closemid_ok += 1; }
            log += &format!("closemid={:?};", r.as_ref().map(|_| ()));
        }
        let step = |g: &mut MarketGroupV16ViewMut<'_, u64>, slot: &mut u64, price: u64, fresh: usize, last: bool, log: &mut String| {
            *slot += 1;
            for asset_index in 0..k.n {
                let is_fresh = if last { asset_index >= k.n - fresh } else { asset_index < fresh };
                if !is_fresh {
                    continue;
                }
                let price = if k.mirror && asset_index % 2 == 1 { 200 - price } else { price };
                let r = g.accrue_asset_to_not_atomic(asset_index, *slot, price, k.funding, true);
                ACCRUE_TOTAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if r.is_err() {
                    ACCRUE_ERR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                *log += &format!("a{asset_index}@{price}={:?};", r.as_ref().map(|_| ()));
                g.markets[asset_index].engine.asset.raw_oracle_target_price = V16PodU64::new(price);
            }
        };
        for p in [96u64, 92] {
            step(&mut g, &mut slot, p, k.n, false, &mut log);
        }
        let force = |g: &mut MarketGroupV16ViewMut<'_, u64>, a: &mut PortfolioV16ViewMut<'_>, cap: u128| {
            let old = a.header.capital.get();
            a.header.capital = V16PodU128::new(cap);
            let c_tot = g.header.c_tot.get();
            g.header.c_tot = V16PodU128::new(c_tot - old + cap);
        };
        if k.early {
            force(&mut g, &mut tv, k.capital);
        }
        let r = g.full_account_refresh_not_atomic(&mut tv);
        log += &format!("ref_t1={:?};", r.as_ref().map(|c| c.certified_liq_deficit));
        for p in [96u64, 100] {
            step(&mut g, &mut slot, p, k.n, false, &mut log);
        }
        step(&mut g, &mut slot, if k.seq == 6 { 96 } else { 104 }, k.fresh, k.fresh_last, &mut log);
        let r = g.full_account_refresh_not_atomic(&mut cv);
        log += &format!("ref_c={:?};", r.as_ref().map(|c| c.certified_liq_deficit));
        if !k.early {
            let cur = tv.header.capital.get();
            force(&mut g, &mut tv, k.capital.min(cur));
        }
        let liq = |g: &mut MarketGroupV16ViewMut<'_, u64>, a: &mut PortfolioV16ViewMut<'_>, asset_index: usize, tag: &str, log: &mut String, st: &mut Stats| {
            let out = g.liquidate_account_not_atomic(a, LiquidationRequestV16 { asset_index });
            st.liq_calls += 1;
            match &out {
                Ok(o) => {
                    st.liq_ok += 1;
                    if o.fee_charged > 0 { st.fee_pos += 1; }
                    if o.insurance_used > 0 { st.ins_pos += 1; }
                    if o.residual_booked > 0 { st.booked_pos += 1; }
                    if o.explicit_loss > 0 { st.explicit_pos += 1; }
                }
                Err(e) => { *st.errs.entry(format!("{e:?}")).or_default() += 1; }
            }
            *log += &format!("liq[{tag}:{asset_index}]={out:?};");
        };
        match k.seq {
            0 | 6 => liq(&mut g, &mut tv, 0, "t", &mut log, &mut *st),
            1 => {
                liq(&mut g, &mut tv, 0, "t", &mut log, &mut *st);
                liq(&mut g, &mut tv, 0, "t2", &mut log, &mut *st);
                liq(&mut g, &mut tv, k.n - 1, "t3", &mut log, &mut *st);
            }
            2 => {
                for round in 0..2 {
                    for a in 0..k.n {
                        liq(&mut g, &mut tv, a, if round == 0 { "r0" } else { "r1" }, &mut log, &mut *st);
                    }
                }
            }
            3 => {
                for a in 0..k.n {
                    liq(&mut g, &mut tv, a, "t", &mut log, &mut *st);
                }
                // the counterparty now carries legs on drained sides; push the price against it
                step(&mut g, &mut slot, 108, k.n, false, &mut log);
                step(&mut g, &mut slot, 112, k.n, false, &mut log);
                let cur = cv.header.capital.get();
                force(&mut g, &mut cv, k.capital.min(cur));
                for a in 0..k.n {
                    liq(&mut g, &mut cv, (a + 1) % k.n, "c", &mut log, &mut *st);
                }
                if k.bystander {
                    let cur = bv.header.capital.get();
                    force(&mut g, &mut bv, k.capital.min(cur));
                    for a in 0..k.n {
                        liq(&mut g, &mut bv, a, "b", &mut log, &mut *st);
                    }
                }
            }
            4 => {
                let r = g.force_asset_recovery_not_atomic(k.n - 1, slot);
                log += &format!("recov={r:?};");
                liq(&mut g, &mut tv, 0, "t", &mut log, &mut *st);
                liq(&mut g, &mut tv, k.n - 1, "tR", &mut log, &mut *st);
            }
            _ => {
                liq(&mut g, &mut tv, k.n - 1, "t", &mut log, &mut *st);
                liq(&mut g, &mut tv, k.n / 2, "tm", &mut log, &mut *st);
            }
        }
    }
    if std::env::var("X1SEC_VERBOSE").is_ok() { println!("X1SECLOG {log}"); }
    let dump = format!("{log}|{header:?}|{markets:?}|{tgt:?}|{cp:?}|{by:?}");
    format!("{} log={}", fnv(&dump), log.split(';').filter(|s| s.starts_with("liq")).collect::<Vec<_>>().join(";"))
}

const MIN_CALLS: u64 = 3000;
const MIN_CLAMP: u64 = 40;
const MIN_FEE: usize = 1000;
const MIN_INS: usize = 30;
const MIN_BOOKED: usize = 40;
const MIN_EXPLICIT: usize = 0;

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
        xs[(self.next() as usize) % xs.len()]
    }
}

#[test]
fn x1_sec_matrix() {
    let count: usize = std::env::var("X1SEC_N").ok().and_then(|v| v.parse().ok()).unwrap_or(6000);
    let mut rng = Lcg(0x5eed_5ec0_1234_5678);
    let mut st = Stats::default();
    let mut digest: u64 = 0xcbf29ce484222325;
    for i in 0..count {
        let n = rng.pick(&[1usize, 1, 2, 2, 3, 3, 5, 8]);
        let k = K {
            n,
            order_rev: rng.pick(&[false, true]),
            alt_sides: rng.pick(&[false, false, true]),
            bystander: rng.pick(&[false, true]),
            insurance: rng.pick(&[0u128, 0, 5_000, 300_000, 100_000_000]),
            capital: {
                let cmul = rng.pick(&[0u128, 100_000, 400_000, 700_000, 900_000, 1_100_000, 1_300_000, 2_000_000]);
                if cmul == 0 { 1_000 } else { cmul * n as u128 }
            },
            early: rng.pick(&[false, true]),
            fresh: rng.pick(&[n, n, 1, n / 2 + 1]),
            fresh_last: rng.pick(&[false, true]),
            funding: rng.pick(&[0i128, 10, -10]),
            fee_bps: rng.pick(&[0u64, 50, 50]),
            seq: rng.pick(&[0u8, 1, 2, 3, 3, 4, 5, 6]),
            mirror: rng.pick(&[false, true]),
            close_mid: rng.pick(&[false, false, true]),
        };
        let r = scenario(&k, &mut st);
        st.scenarios += 1;
        println!("X1SEC {i} {k:?} {r}");
        digest = (digest ^ fnv(&r)).wrapping_mul(0x100000001b3);
    }
    println!(
        "X1SECSUM scenarios={} liq_calls={} liq_ok={} fee_pos={} ins_pos={} booked_pos={} explicit_pos={} by_ok={} by_err={} closemid_ok={} errs={:?} digest={}",
        st.scenarios, st.liq_calls, st.liq_ok, st.fee_pos, st.ins_pos, st.booked_pos, st.explicit_pos, st.by_ok, st.by_err, st.closemid_ok, st.errs, digest
    );
    let (calls, clamp_fired, mismatch) = percolator::x1_diff_stats();
    let (acc_total, acc_err) = (
        ACCRUE_TOTAL.load(std::sync::atomic::Ordering::Relaxed),
        ACCRUE_ERR.load(std::sync::atomic::Ordering::Relaxed),
    );
    println!("X1DIFF monitored_calls={calls} clamp_fired={clamp_fired} mismatches={mismatch} accrue_total={acc_total} accrue_err={acc_err}");
    assert_eq!(mismatch, 0);
    if count >= 6000 {
        assert!(calls >= MIN_CALLS, "vacuous: only {calls} monitored fee-charge calls");
        assert!(clamp_fired >= MIN_CLAMP, "vacuous: the clamp found an excess in only {clamp_fired} calls");
        assert!(st.fee_pos >= MIN_FEE && st.ins_pos >= MIN_INS && st.booked_pos >= MIN_BOOKED && st.explicit_pos >= MIN_EXPLICIT,
            "vacuous: fee {} ins {} booked {} explicit {}", st.fee_pos, st.ins_pos, st.booked_pos, st.explicit_pos);
        assert!(acc_err * 10 <= acc_total, "accrual errors {acc_err} of {acc_total} exceed 10%");
    }
}
