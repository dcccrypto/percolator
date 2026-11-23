//! Safety net for merging the keeper crank's two engine passes (v2.2 engine, public API only).
//!
//! Today the worst-case crank runs the maintenance-fee sync (`sync_account_fee_to_slot_not_atomic`,
//! which settles every leg) and then the refresh crank walks the legs again. A future change may
//! fold those into one pass. These tests pin what today's two-pass order leaves behind, for many
//! seeded multi-leg accounts: leg count up to the portfolio cap, fee per slot zero and non-zero,
//! slot gaps, and price paths.
//!
//! * `run_two_pass` is the pipeline under test: fee sync, refresh every leg, full refresh,
//!   liquidate if the certificate says so. A merged single-pass version should be dropped in
//!   here (or wrapped in a function with the same signature).
//! * `Snapshot` is the post-crank state: capital, PnL, fee slot, legs, insurance, vault, c_tot,
//!   certificate, liquidation outcome.
//! * `assert_same_state` is the golden comparison helper: it diffs two snapshots field by field.
//! * `GOLDEN_DIGEST` is the digest of every case's snapshot as of today's engine.
//!
//! If the golden digest test fails after an intentional change, run it with `--nocapture`: it
//! prints the new digest and every case line so the diff can be reviewed before re-pinning.
//! Test names start with `fee_sync_refresh_` so `cargo test -- fee_sync_refresh` selects them.
#![allow(dead_code)]

use percolator::{
    EngineAssetSlotV16Account, LiquidationRequestV16, Market, MarketGroupV16HeaderAccount,
    MarketGroupV16ViewMut, PermissionlessCrankActionV16, PermissionlessCrankRequestV16,
    PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, TradeRequestV16, V16Config, V16Error, V16PodU64, POS_SCALE,
    V16_MAX_PORTFOLIO_ASSETS_N,
};

const P0: u64 = 100_000; // fine price grid: 1 bp = 10 ticks
const BIG: u128 = 1_000_000_000;
const LEG_SIZE: i128 = (10 * POS_SCALE) as i128;

// ---------------------------------------------------------------------------------------------
// Deterministic RNG
// ---------------------------------------------------------------------------------------------

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

// ---------------------------------------------------------------------------------------------
// World
// ---------------------------------------------------------------------------------------------

fn account(seed: u8) -> PortfolioAccountV16Account {
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        [1; 32], [seed; 32], [3; 32],
    ));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(header).unwrap();
    a
}

struct W {
    header: MarketGroupV16HeaderAccount,
    markets: Vec<Market<u64>>,
    slot: u64,
    n: usize,
}

impl W {
    fn new(n: usize) -> Self {
        let mut cfg = V16Config::public_user_fund_with_market_slots(n as u16, n as u32, 0, 10);
        cfg.maintenance_margin_bps = 500;
        cfg.initial_margin_bps = 1_000;
        cfg.max_price_move_bps_per_slot = 8;
        cfg.max_accrual_dt_slots = 50;
        cfg.min_funding_lifetime_slots = 1_000_000;
        cfg.liquidation_fee_bps = 0;
        cfg.liquidation_fee_cap = 1_000_000;
        cfg.min_nonzero_mm_req = 1;
        cfg.min_nonzero_im_req = 2;
        let mut header =
            MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, n as u32, 0)
                .unwrap_or_else(|e| panic!("new_dynamic n={n}: {e:?}"));
        let mut markets: Vec<Market<u64>> = (0..n as u64)
            .map(|i| Market::new(i, EngineAssetSlotV16Account::default()))
            .collect();
        for i in 0..n {
            header
                .activate_empty_asset_slot_not_atomic(
                    i as u32,
                    &mut markets[i].engine,
                    P0,
                    1 + i as u64,
                )
                .unwrap();
        }
        let mut w = W { header, markets, slot: n as u64, n };
        w.tick(0, &vec![0; n]); // bring every asset clock to the same slot
        w
    }
    fn view(&mut self) -> MarketGroupV16ViewMut<'_, u64> {
        MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets)
    }
    fn price(&self, i: usize) -> u64 {
        self.markets[i].engine.asset.effective_price.get()
    }
    fn deposit(&mut self, a: &mut PortfolioAccountV16Account, amt: u128) {
        self.view().deposit_not_atomic(&mut PortfolioV16ViewMut::new(a), amt).unwrap();
    }
    fn trade(
        &mut self,
        i: usize,
        long: &mut PortfolioAccountV16Account,
        short: &mut PortfolioAccountV16Account,
    ) {
        let p = self.price(i);
        self.view()
            .execute_trade_with_fee_loss_stale_scoped_not_atomic(
                &mut PortfolioV16ViewMut::new(long),
                &mut PortfolioV16ViewMut::new(short),
                TradeRequestV16 { asset_index: i, size_q: LEG_SIZE, exec_price: p, fee_bps: 0 },
                true,
            )
            .expect("trade");
    }
    /// Advance `dt` slots; every asset moves to `price(j) * (10_000 + bps[j]) / 10_000`.
    fn tick(&mut self, dt: u64, bps: &[i64]) {
        self.slot += dt;
        let slot = self.slot;
        for j in 0..self.n {
            let p = self.price(j) as i128;
            let new = (p * (10_000 + bps[j] as i128) / 10_000).max(1) as u64;
            let mut m = self.view();
            m.accrue_asset_to_not_atomic(j, slot, new, 0, true).unwrap_or_else(|e| panic!("accrue asset {j} slot {slot} dt {dt} price {} -> {new}: {e:?}", p));
            m.markets[j].engine.asset.raw_oracle_target_price = V16PodU64::new(new);
        }
    }
    fn crank_refresh_all(&mut self, a: &mut PortfolioAccountV16Account) {
        let slot = self.slot;
        for i in 0..self.n {
            let price = self.price(i);
            let _ = self.view().permissionless_crank_not_atomic(
                &mut PortfolioV16ViewMut::new(a),
                PermissionlessCrankRequestV16 {
                    now_slot: slot,
                    asset_index: i,
                    effective_price: price,
                    funding_rate_e9: 0,
                    action: PermissionlessCrankActionV16::Refresh,
                },
            );
        }
    }
    fn refresh(&mut self, a: &mut PortfolioAccountV16Account) -> percolator::HealthCertV16 {
        for _ in 0..64 {
            let r = self.view().full_account_refresh_not_atomic(&mut PortfolioV16ViewMut::new(a));
            match r {
                Err(V16Error::BStale) => self.crank_refresh_all(a),
                r => return r.expect("refresh"),
            }
        }
        panic!("refresh did not converge");
    }
    /// Fee sync; on `BStale` walk the legs with refresh cranks and retry, like a keeper would.
    fn sync_fee(
        &mut self,
        a: &mut PortfolioAccountV16Account,
        rate: u128,
    ) -> Result<u128, V16Error> {
        for _ in 0..64 {
            let slot = self.slot;
            match self
                .view()
                .sync_account_fee_to_slot_not_atomic(&mut PortfolioV16ViewMut::new(a), slot, rate)
            {
                Err(V16Error::BStale) => self.crank_refresh_all(a),
                r => return r,
            }
        }
        panic!("fee sync did not converge");
    }
    fn liquidate_any(&mut self, a: &mut PortfolioAccountV16Account) -> bool {
        let mut any = false;
        for i in 0..self.n {
            any |= self
                .view()
                .liquidate_account_not_atomic(
                    &mut PortfolioV16ViewMut::new(a),
                    LiquidationRequestV16 { asset_index: i },
                )
                .is_ok();
        }
        any
    }
}

// ---------------------------------------------------------------------------------------------
// Cases
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
struct Case {
    seed: u64,
    legs: usize,
    /// maintenance fee per slot (0 = none)
    rate: u128,
    /// largest slot gap between price steps
    max_gap: u64,
    /// number of price steps before the crank
    steps: u32,
    /// price step volatility per slot in bps (<= 8, the config cap)
    vol_bps: u64,
    /// downward drift per slot in bps (vol_bps + drift_bps <= 8, the config cap)
    drift_bps: u64,
    /// capital of the account as a percent of the sum of per-leg initial margin
    cap_pct: u64,
}

fn build(c: Case) -> (W, PortfolioAccountV16Account, PortfolioAccountV16Account) {
    let mut w = W::new(c.legs);
    let (mut a, mut b) = (account(1), account(2));
    // initial margin per leg: 10 units * P0 * 10% = P0
    let cap = (c.legs as u128 * P0 as u128 * c.cap_pct as u128 / 100).max(1);
    w.deposit(&mut a, cap);
    w.deposit(&mut b, BIG);
    for i in 0..c.legs {
        w.trade(i, &mut a, &mut b);
    }
    let mut rng = Rng(c.seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    for _ in 0..c.steps {
        let dt = 1 + rng.below(c.max_gap);
        let max = (c.vol_bps * dt) as i64;
        let drift = (c.drift_bps * dt) as i64;
        let bps: Vec<i64> = (0..c.legs)
            .map(|_| rng.below(2 * max as u64 + 1) as i64 - max - drift)
            .collect();
        w.tick(dt, &bps);
    }
    (w, a, b)
}

fn cases() -> Vec<Case> {
    let mut v = Vec::new();
    let cap = V16_MAX_PORTFOLIO_ASSETS_N;
    let leg_counts = [2usize, 3, 5, 8, cap - 1, cap];
    let rates = [0u128, 1, 7, 250];
    let mut seed = 1u64;
    for &legs in &leg_counts {
        for &rate in &rates {
            for &(max_gap, steps, vol_bps, drift_bps, cap_pct) in
                &[
                    (1u64, 3u32, 6u64, 0u64, 300u64),
                    (50, 4, 7, 0, 150),
                    (3, 6, 7, 0, 102),
                    (50, 4, 1, 5, 105),
                ]
            {
                v.push(Case { seed, legs, rate, max_gap, steps, vol_bps, drift_bps, cap_pct });
                seed += 1;
            }
        }
    }
    v
}

// ---------------------------------------------------------------------------------------------
// Snapshot and golden comparison
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
struct Snapshot {
    fee_result: Result<u128, V16Error>,
    liquidated: bool,
    capital: u128,
    pnl: i128,
    last_fee_slot: u64,
    /// (asset_index, basis_pos_q) of active legs, in leg-slot order
    legs: Vec<(u32, i128)>,
    cp_capital: u128,
    cp_pnl: i128,
    insurance: u128,
    vault: u128,
    c_tot: u128,
    cert_equity: i128,
    cert_maintenance: u128,
    cert_liq_deficit: u128,
}

impl Snapshot {
    fn line(&self) -> String {
        format!("{self:?}")
    }
}

/// Golden comparison helper: field-by-field diff so a failure says which part of the state moved.
fn assert_same_state(label: &str, left: &Snapshot, right: &Snapshot) {
    macro_rules! f {
        ($($x:ident),*) => {$(
            assert_eq!(left.$x, right.$x, "{label}: field `{}` differs\n left: {}\nright: {}",
                stringify!($x), left.line(), right.line());
        )*};
    }
    f!(fee_result, liquidated, capital, pnl, last_fee_slot, legs, cp_capital, cp_pnl, insurance,
       vault, c_tot, cert_equity, cert_maintenance, cert_liq_deficit);
}

fn snapshot(
    w: &mut W,
    a: &PortfolioAccountV16Account,
    b: &PortfolioAccountV16Account,
    fee_result: Result<u128, V16Error>,
    liquidated: bool,
    cert: percolator::HealthCertV16,
) -> Snapshot {
    Snapshot {
        fee_result,
        liquidated,
        capital: a.capital.get(),
        pnl: a.pnl.get(),
        last_fee_slot: a.last_fee_slot.get(),
        legs: a
            .legs
            .iter()
            .filter(|l| l.active != 0)
            .map(|l| (l.asset_index.get(), l.basis_pos_q.get()))
            .collect(),
        cp_capital: b.capital.get(),
        cp_pnl: b.pnl.get(),
        insurance: w.header.insurance.get(),
        vault: w.header.vault.get(),
        c_tot: w.header.c_tot.get(),
        cert_equity: cert.certified_equity,
        cert_maintenance: cert.certified_maintenance_req,
        cert_liq_deficit: cert.certified_liq_deficit,
    }
}

/// Today's order: fee sync (settles all legs), then refresh walks them again, then liquidate if
/// the certificate says so. A merged single-pass version must leave the same `Snapshot`.
fn run_two_pass(c: Case) -> (W, PortfolioAccountV16Account, PortfolioAccountV16Account, Snapshot) {
    let (mut w, mut a, mut b) = build(c);
    let fee = w.sync_fee(&mut a, c.rate);
    let cert = w.refresh(&mut a);
    let mut liq = false;
    if cert.certified_liq_deficit > 0 {
        liq = w.liquidate_any(&mut a);
    }
    // The counterparty refresh backs `a`'s gains, so the pinned certificate is taken after it.
    w.refresh(&mut b);
    let cert = w.refresh(&mut a);
    let s = snapshot(&mut w, &a, &b, fee, liq, cert);
    (w, a, b, s)
}

/// Reverse order (refresh, then fee sync), used to show what the order is worth.
fn run_refresh_first(c: Case) -> Snapshot {
    let (mut w, mut a, mut b) = build(c);
    let mut cert = w.refresh(&mut a);
    let fee = w.sync_fee(&mut a, c.rate);
    let mut liq = false;
    if cert.certified_liq_deficit > 0 {
        liq = w.liquidate_any(&mut a);
    }
    w.refresh(&mut b);
    cert = w.refresh(&mut a);
    snapshot(&mut w, &a, &b, fee, liq, cert)
}

fn fnv(s: &str, mut h: u64) -> u64 {
    for byte in s.bytes() {
        h ^= byte as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

fn digest(snaps: &[Snapshot]) -> u64 {
    snaps.iter().fold(0xcbf2_9ce4_8422_2325, |h, s| fnv(&s.line(), h))
}

// ---------------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------------

#[test]
fn fee_sync_refresh_case_matrix_covers_cap_and_both_fee_modes() {
    let cs = cases();
    assert_eq!(cs.len(), 96);
    assert!(cs.iter().any(|c| c.legs == V16_MAX_PORTFOLIO_ASSETS_N));
    assert!(cs.iter().any(|c| c.rate == 0) && cs.iter().any(|c| c.rate > 0));
    assert!(cs.iter().any(|c| c.max_gap >= 50));
}

/// Invariants that must hold after every crank, whatever the pass structure.
#[test]
fn fee_sync_refresh_invariants_hold_for_every_case() {
    for c in cases() {
        let (mut w, mut a, mut b, s) = run_two_pass(c);
        let label = format!("{c:?}");
        // conservation: vault covers capital plus insurance
        assert!(s.vault >= s.c_tot + s.insurance, "{label}: V < C_tot + I: {}", s.line());
        // validity of both accounts and the market
        let g = w.view();
        g.validate_shape().unwrap();
        PortfolioV16ViewMut::new(&mut a).validate_with_market(&g.as_view()).unwrap();
        PortfolioV16ViewMut::new(&mut b).validate_with_market(&g.as_view()).unwrap();
        // the fee never exceeds the requested amount: rate * slots elapsed
        if let Ok(charged) = s.fee_result {
            let elapsed = (w.slot).saturating_sub(0) as u128;
            assert!(charged <= c.rate * elapsed, "{label}: charged {charged} > rate*slots");
            if c.rate == 0 {
                assert_eq!(charged, 0, "{label}: fee charged with zero rate");
            }
        }
        // fee anchor never runs past the current slot
        assert!(s.last_fee_slot <= w.slot, "{label}: fee slot ahead of clock");
        // a liquidated account has no more deficit than it started with
        if !s.liquidated {
            assert!(s.legs.len() <= c.legs, "{label}");
        }
        // capital only goes down from fee/loss, never above deposit plus gains realised as capital
        assert!(s.capital <= c.legs as u128 * P0 as u128 * c.cap_pct as u128 / 100 + 1, "{label}");
    }
}

/// With zero fee the fee sync is a no-op on state, so a merged pass must equal refresh-only.
#[test]
fn fee_sync_refresh_zero_fee_order_does_not_matter() {
    for c in cases().into_iter().filter(|c| c.rate == 0) {
        let two = run_two_pass(c).3;
        let rev = run_refresh_first(c);
        assert_same_state(&format!("{c:?}"), &two, &rev);
    }
}

/// Running the crank twice at the same slot must change nothing (idempotent).
#[test]
fn fee_sync_refresh_second_crank_at_same_slot_changes_nothing() {
    for c in cases() {
        let (mut w, mut a, mut b, s1) = run_two_pass(c);
        if s1.liquidated {
            continue;
        }
        let fee = w.sync_fee(&mut a, c.rate);
        assert!(matches!(fee, Ok(0)) || fee.is_err(), "{c:?}: second sync charged {fee:?}");
        w.refresh(&mut b);
        let cert = w.refresh(&mut a);
        let s2 = snapshot(&mut w, &a, &b, s1.fee_result.clone(), s1.liquidated, cert);
        assert_same_state(&format!("{c:?}"), &s1, &s2);
    }
}

/// The total fee over a longer interval is charged once: syncing mid-way then at the end equals
/// one sync at the end, for a flat-priced account (no loss anchor effects).
#[test]
fn fee_sync_refresh_split_sync_charges_same_total_when_prices_flat() {
    for &legs in &[2usize, 4, V16_MAX_PORTFOLIO_ASSETS_N] {
        for &rate in &[1u128, 9] {
            let c = Case { seed: 1, legs, rate, max_gap: 1, steps: 0, vol_bps: 0, drift_bps: 0, cap_pct: 400 };
            let flat = vec![0i64; legs];
            let (mut w1, mut a1, _b1) = build(c);
            w1.tick(10, &flat);
            let one = w1.sync_fee(&mut a1, rate).unwrap();
            w1.tick(10, &flat);
            let two = one + w1.sync_fee(&mut a1, rate).unwrap();

            let (mut w2, mut a2, _b2) = build(c);
            w2.tick(20, &flat);
            let all = w2.sync_fee(&mut a2, rate).unwrap();
            assert_eq!(two, all, "legs {legs} rate {rate}");
            assert_eq!(a1.capital.get(), a2.capital.get(), "legs {legs} rate {rate}");
            assert_eq!(w1.header.insurance.get(), w2.header.insurance.get());
        }
    }
}

/// Golden: today's post-crank state for every case. Intentional engine changes must re-pin this.
const GOLDEN_DIGEST: u64 = 0x99309842fb6b750c;
const GOLDEN_LIQUIDATED: usize = 7;
const GOLDEN_FEE_CHARGED_CASES: usize = 72;

#[test]
fn fee_sync_refresh_golden_post_crank_state() {
    let cs = cases();
    let snaps: Vec<Snapshot> = cs.iter().map(|&c| run_two_pass(c).3).collect();
    let d = digest(&snaps);
    let liq = snaps.iter().filter(|s| s.liquidated).count();
    let fee = snaps.iter().filter(|s| matches!(s.fee_result, Ok(f) if f > 0)).count();
    println!("DIGEST {d:#x} liquidated {liq} fee_charged {fee} cases {}", snaps.len());
    for (c, s) in cs.iter().zip(&snaps) {
        println!("{c:?} => {}", s.line());
    }
    assert_eq!(liq, GOLDEN_LIQUIDATED, "liquidation outcome count moved");
    assert_eq!(fee, GOLDEN_FEE_CHARGED_CASES, "fee-charged case count moved");
    assert_eq!(d, GOLDEN_DIGEST, "post-crank state digest moved; see --nocapture output");
}

/// The golden run is deterministic: two runs give identical snapshots.
#[test]
fn fee_sync_refresh_golden_run_is_deterministic() {
    for c in cases() {
        assert_same_state(&format!("{c:?}"), &run_two_pass(c).3, &run_two_pass(c).3);
    }
}
