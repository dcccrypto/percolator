//! Upstream aeyakovenko/percolator-prog#457 at engine level (from the reviewer's `sec_i457.rs`):
//! actor 0 long 4,500 on 1,000 capital, actor 1 long 2,000, actor 2 short 1,500, a rich maker on
//! the other side, no fees or funding, 10% IM, price 1.00 -> 1.14 -> 0.855 (accrued in steps of
//! at most 1.9%, nobody settled), and peak cranks on {nobody, everyone, actor 0, actors 0+1+2}.
//! A NON-flipping long's gain reverses: its loss books into the long domain while its claim sits
//! in the short-loser domain (a cross-domain netting) at the stored partial credit rate. No
//! position flip is involved. Before R1 round 2 the fourth run destroyed 700 USD (actor 0 ended
//! with pnl -137.313 and the maker/others were not paid); after it all four runs are exact.
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

fn eqf(a: &PortfolioAccountV16Account) -> i128 { a.capital.get() as i128 + a.pnl.get() }

/// returns (changes in face equity actors 0..=3, actor0 (capital,pnl), vault-c_tot-ins vs pnl_pos)
fn run(at_peak: &[usize]) -> ([i128; 4], (i128, i128), [i128; 4], i128) {
    let mut w = World::new_pairs(&[], 0, 0);
    let mut acts = [account(10), account(11), account(12), w.maker];
    let caps = [1_000 * U, 1_000 * U, 1_000 * U, 20_000 * U];
    for i in 0..3 { let mut a = acts[i]; w.deposit(&mut a, caps[i]); acts[i] = a; }
    // maker keeps its 1e15 (rich, solvent) -- its capital does not matter, it pays in full
    let (mut a0, mut a1, mut a2, mut m) = (acts[0], acts[1], acts[2], acts[3]);
    w.trade(&mut a0, &mut m, 4_500 * POS_SCALE).expect("a0 long");
    w.trade(&mut a1, &mut m, 2_000 * POS_SCALE).expect("a1 long");
    w.trade(&mut m, &mut a2, 1_500 * POS_SCALE).expect("a2 short");
    let mut acts = [a0, a1, a2, m];
    let start: Vec<i128> = acts.iter().map(eqf).collect();
    // 1.00 -> 1.14: 7 steps of +2% approx then settle target
    let mut price_steps = |w: &mut World, target: u64| {
        let mut guard = 0;
        while w.price() != target && guard < 200 {
            guard += 1;
            let cur = w.price() as i128;
            let want = target as i128;
            let step_bps = (((want - cur) * 10_000) / cur).clamp(-190, 190) as i64;
            let step_bps = if step_bps == 0 { if want > cur { 1 } else { -1 } } else { step_bps };
            assert!(w.accrue(step_bps, 0));
        }
    };
    price_steps(&mut w, 1_140_000);
    for &i in at_peak { let mut a = acts[i]; assert!(w.settle(&mut a)); acts[i] = a; }
    price_steps(&mut w, 855_000);
    for i in 0..4 { let mut a = acts[i]; assert!(w.settle(&mut a)); acts[i] = a; }
    w.traders = acts[..3].to_vec();
    w.maker = acts[3];
    let mut ch = [0i128; 4];
    for i in 0..4 { ch[i] = eqf(&acts[i]) - start[i]; }
    let mut eff = [0i128; 4];
    for i in 0..4 { let mut a = acts[i]; eff[i] = effective_equity(&mut w, &mut a) - start[i] ; }
    let sum: i128 = ch.iter().sum();
    (ch, (acts[0].capital.get() as i128, acts[0].pnl.get()), eff, sum)
}

#[test]
fn i457_every_peak_crank_pattern_gives_the_exact_result() {
    // changes in capital+pnl (micro-USD): the same in all four runs, summing to zero
    const EXPECT: [i128; 4] = [-652_698_000, -290_088_000, 217_566_000, 725_220_000];
    for (name, peak) in [("nobody", vec![]), ("everyone", vec![0, 1, 2, 3]), ("actor0 only", vec![0]), ("0,1,2 not maker", vec![0, 1, 2])] {
        let (ch, a0, eff, sum) = run(&peak);
        assert_eq!(ch, EXPECT, "{name}: face-equity change");
        assert_eq!(eff, EXPECT, "{name}: effective equity equals face equity");
        assert_eq!(sum, 0, "{name}: nothing destroyed");
        assert_eq!(a0, (347_302_000, 0), "{name}: actor 0 ends with capital and no negative pnl");
    }
}

fn run_resolved(peak_cranks: &[usize], close_order: &[usize], reverse_first: bool) -> Vec<u128> {
    let mut w = World::new_pairs(&[], 0, 0);
    let mut acts = [account(10), account(11), account(12), w.maker];
    for i in 0..3 { let mut a = acts[i]; w.deposit(&mut a, 1_000 * U); acts[i] = a; }
    let (mut a0, mut a1, mut a2, mut m) = (acts[0], acts[1], acts[2], acts[3]);
    w.trade(&mut a0, &mut m, 4_500 * POS_SCALE).unwrap();
    w.trade(&mut a1, &mut m, 2_000 * POS_SCALE).unwrap();
    w.trade(&mut m, &mut a2, 1_500 * POS_SCALE).unwrap();
    let mut acts = [a0, a1, a2, m];
    let steps = |w: &mut World, target: u64| { let mut g = 0; while w.price() != target && g < 200 { g += 1; let cur = w.price() as i128; let b = (((target as i128 - cur) * 10_000) / cur).clamp(-190, 190) as i64; let b = if b == 0 { if target as i128 > cur { 1 } else { -1 } } else { b }; assert!(w.accrue(b, 0)); } };
    steps(&mut w, 1_140_000);
    for &i in peak_cranks { let mut a = acts[i]; assert!(w.settle(&mut a)); acts[i] = a; }
    if reverse_first { steps(&mut w, 855_000); }
    let slot = w.slot + 1;
    let mut m = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
    m.resolve_market_not_atomic(slot).unwrap();
    let mut paid = vec![0u128; 4];
    let mut closed = [false; 4];
    for _ in 0..60 {
        for &i in close_order {
            if closed[i] { continue; }
            for d in 0..2 { let _ = m.expire_source_backing_bucket_not_atomic(d, slot); }
            if let Ok(percolator::ResolvedCloseOutcomeV16::Closed { payout }) = m.close_resolved_account_not_atomic(&mut PortfolioV16ViewMut::new(&mut acts[i]), 0) { paid[i] = payout; closed[i] = true; }
        }
        if closed.iter().all(|c| *c) { break; }
    }
    assert!(closed.iter().all(|c| *c), "not all closed {closed:?}");
    paid
}

/// All 16 crank masks over {actor 0, 1, 2, maker} at the peak: the FACE equity (capital + pnl) is
/// exact in every one (the pre-R1 engine is wrong in 4 of them, e.g. {0,1,2}: -700.000 destroyed),
/// actor 0 ends with capital 347.302 and no negative pnl, and nothing is destroyed.
///
/// The EFFECTIVE (certified, haircut) equity is exact in 9 masks and NOT in the 7 where a
/// loser-side account (actor 2 or the maker) is cranked at the peak while the longs are not (S10):
/// the backing its realised loss left in the short-loser domain has no claimant (the longs' peak
/// gain reversed before they settled), so the claims the reversal creates in the other domain are
/// only partly backed. Those 7 results are pinned so any change to that class is deliberate.
#[test]
fn i457_all_sixteen_crank_masks_are_exact_in_face_terms() {
    const FACE: [i128; 4] = [-652_698_000, -290_088_000, 217_566_000, 725_220_000];
    // masks whose effective equity differs from the face equity: (mask, actor 2 eff, maker eff)
    const STRANDED: [(u32, i128, i128); 7] = [
        (4, 139_677_423, 593_108_576),
        (8, 124_859_950, 117_926_049),
        (9, 208_295_395, 664_490_604),
        (10, 161_942_370, 360_843_629),
        (12, 7_565_999, 25_219_999),
        (13, 152_950_615, 509_835_384),
        (14, 72_181_384, 240_604_615),
    ];
    for mask in 0u32..16 {
        let peak: Vec<usize> = (0..4).filter(|i| mask >> i & 1 == 1).collect();
        let (ch, a0, eff, sum) = run(&peak);
        assert_eq!(ch, FACE, "mask {mask}: face equity");
        assert_eq!(sum, 0, "mask {mask}: nothing destroyed in face terms");
        assert_eq!(a0, (347_302_000, 0), "mask {mask}: actor 0 capital and pnl");
        match STRANDED.iter().find(|x| x.0 == mask) {
            None => assert_eq!(eff, FACE, "mask {mask}: effective equity exact"),
            Some(&(_, a2, mk)) => {
                assert_eq!((eff[0], eff[1]), (FACE[0], FACE[1]), "mask {mask}: longs exact");
                assert_eq!((eff[2], eff[3]), (a2, mk), "mask {mask}: pinned stranded-backing class");
                assert!(eff[2] + eff[3] < FACE[2] + FACE[3], "mask {mask}: effective is below face, never above");
            }
        }
    }
}

#[test]
fn i457_resolved_close_order_does_not_change_payouts() {
    // Resolved uses the same pending-credit pricing (only `booked` is 0 outside Live).
    for (nm, peak) in [("none", vec![]), ("0,1,2", vec![0usize, 1, 2])] {
        for rf in [true, false] {
            let a = run_resolved(&peak, &[0, 1, 2, 3], rf);
            let b = run_resolved(&peak, &[3, 2, 1, 0], rf);
            assert_eq!(a, b, "peak {nm} reverse_first {rf}: close order changes payouts");
            assert_eq!(a.iter().sum::<u128>(), 1_000_003_000_000_000, "conserved");
        }
    }
}

/// S10 is permanent, not a Live liquidity haircut: at Resolved the stranded backing does not come
/// back to the loser (it has no claimant and no junior claim covers it). Pinned for the maker-only
/// and actor-2-only peak cranks; the all-cranked and nobody-cranked payouts are the ideal.
#[test]
fn i457_resolved_stranded_backing_is_not_returned() {
    let ideal = vec![347_302_000u128, 709_912_000, 1_217_566_000, 1_000_000_725_220_000];
    assert_eq!(run_resolved(&[], &[0, 1, 2, 3], true), ideal);
    assert_eq!(run_resolved(&[0, 1, 2, 3], &[0, 1, 2, 3], true), ideal);
    assert_eq!(run_resolved(&[3], &[0, 1, 2, 3], true), vec![347_302_000, 709_912_000, 1_124_859_950, 1_000_000_117_926_049]);
    assert_eq!(run_resolved(&[2], &[0, 1, 2, 3], true), vec![347_302_000, 709_912_000, 1_139_677_423, 1_000_000_593_108_576]);
}
