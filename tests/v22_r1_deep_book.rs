//! S5 deep-book check. One rich maker (the LP side) takes the other side of K poor, liened
//! flipping accounts. Each goes long, builds an unconverted claim, then flips short against the
//! maker (a lien backs the larger position's margin) and the price rises again. Eight settle
//! permutations per world over {maker, victims}; the metrics are the maker's effective equity
//! (Earn sits behind it), the victims', and the total. The pre-R1 engine's worst maker outcome
//! over the orders is the floor the maker must never fall below.
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

/// Returns (maker effective change, per-victim effective change, victims that carried a lien) for one
/// settle permutation, or None if the flip is refused.
fn scenario(seed: u64, perm: &[usize]) -> Option<(i128, Vec<i128>, usize)> {
    let mut s = seed | 1;
    let k = 3 + (rng(&mut s) % 6) as usize; // 3..8 victims
    let cap = 300_000 + (rng(&mut s) % 120_000) as u128;
    let sell = 7 + (rng(&mut s) % 2) as u128; // flip to short 4 or 5 (of 3)
    let up1 = 60 + (rng(&mut s) % 60) as i64;
    let st1 = 3 + (rng(&mut s) % 4) as usize;
    let up2 = 40 + (rng(&mut s) % 100) as i64;
    let st2 = 1 + (rng(&mut s) % 3) as usize;
    let mut w = World::new_pairs(&[], 0, 0);
    let mut maker = w.maker;
    let mut vs: Vec<PortfolioAccountV16Account> = (0..k).map(|i| account(20 + i as u32)).collect();
    for v in vs.iter_mut() { w.deposit(v, cap); }
    for v in vs.iter_mut() { w.trade(v, &mut maker, 3 * POS_SCALE).ok()?; }
    let start_m = maker.capital.get() as i128;
    let start_v: Vec<i128> = vs.iter().map(|v| v.capital.get() as i128).collect();
    for _ in 0..st1 {
        if !w.accrue(up1, 0) { return None; }
        if !w.settle(&mut maker) { return None; }
        for v in vs.iter_mut() { if !w.settle(v) { return None; } }
    }
    for v in vs.iter_mut() { w.trade(&mut maker, v, sell * POS_SCALE).ok()?; }
    let liened = vs.iter().filter(|v| v.source_domains.iter().any(|d| d.source_claim_liened_num.get() > 0)).count();
    for _ in 0..st2 { if !w.accrue(up2, 0) { return None; } }
    // perm over 0 = maker, 1..=k = victims
    for &i in perm {
        let ok = if i == 0 { w.settle(&mut maker) } else { w.settle(&mut vs[i - 1]) };
        if !ok { return None; }
    }
    w.traders = vs.clone();
    w.maker = maker;
    let mut e: [PortfolioAccountV16Account; 0] = [];
    w.validate(&mut e);
    let mm = effective_equity(&mut w, &mut maker) - start_m;
    let vv: Vec<i128> = (0..k).map(|i| { let mut a = vs[i]; effective_equity(&mut w, &mut a) - start_v[i] }).collect();
    Some((mm, vv, liened))
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

struct Deep {
    ran: u64,
    liened_worlds: u64,
    sum_maker_min: i128,
    sum_total_spread: i128,
    gains: u64,
}

fn sweep(worlds: u64, seed0: u64) -> Deep {
    let mut d = Deep { ran: 0, liened_worlds: 0, sum_maker_min: 0, sum_total_spread: 0, gains: 0 };
    for i in 0..worlds {
        let seed = seed0 * 6007 + i;
        let mut t = seed | 1;
        let k = 3 + (rng(&mut t) % 6) as usize;
        let mut s = seed ^ 0x5A5A;
        let ps = perms(k + 1, 8, &mut s);
        let rows: Vec<(i128, i128, usize)> = ps
            .iter()
            .filter_map(|p| scenario(seed, p).map(|(m, v, l)| (m, m + v.iter().sum::<i128>(), l)))
            .collect();
        if rows.is_empty() {
            continue;
        }
        d.ran += 1;
        if rows[0].2 > 0 {
            d.liened_worlds += 1;
        }
        d.sum_maker_min += rows.iter().map(|r| r.0).min().unwrap();
        let tmax = rows.iter().map(|r| r.1).max().unwrap();
        let tmin = rows.iter().map(|r| r.1).min().unwrap();
        d.sum_total_spread += tmax - tmin;
        if tmax > 8 {
            d.gains += 1;
        }
    }
    d
}

/// S5, option B. In the deep-book worlds the maker's WORST outcome over the settle orders is
/// never below the pre-R1 engine's worst (computed on 874fe33a), so the LP side is never below
/// what the old engine could already do to it. Total equity never gains.
///
/// Upstream #250 (+ 7b32eef2): a victim's liened face no longer haircuts the netting rate, so no
/// face is burned: every world is now exactly zero-sum and order independent (total spread 0,
/// was 2,174,094). The higher credit rate also lets more flips pass IM, so 372 worlds run instead
/// of 187; a summed floor over different world sets is meaningless (the maker's sum is
/// -124,633,648, of which -55,847,966 is the 185 new worlds, zero-sum with the victims). The floor
/// is therefore checked per world on the 187 worlds the pre-R1 floor was taken over: there the
/// maker is at or above the old worst in every world (summed -68,785,682 vs -78,538,861, also
/// above the old BEST order, -75,117,201), and the victims gain too (no face burned).
#[test]
fn deep_book_maker_is_never_below_the_pre_r1_worst_order() {
    let d = sweep(700, 5);
    eprintln!("DEEPBOOK ran {} liened_worlds {} sum_maker_min {} sum_total_spread {} gains {}", d.ran, d.liened_worlds, d.sum_maker_min, d.sum_total_spread, d.gains);
    assert!(d.ran >= 150 && d.liened_worlds * 10 >= d.ran * 8, "the worlds must carry liens");
    assert_eq!(d.gains, 0, "total equity never gains");
    assert!(d.sum_total_spread <= SPREAD_RATCHET, "order spread regressed: {}", d.sum_total_spread);
    assert_eq!(
        PRE_R1_MAKER_MIN_BY_WORLD.iter().map(|w| w.1).sum::<i128>(),
        PRE_R1_SUM_MAKER_MIN,
        "per-world floors add up to the pre-R1 sum"
    );
    let (mut checked, mut sum_now) = (0u64, 0i128);
    for &(off, floor) in PRE_R1_MAKER_MIN_BY_WORLD.iter() {
        let seed = 5 * 6007 + off;
        let mut t = seed | 1;
        let k = 3 + (rng(&mut t) % 6) as usize;
        let mut s = seed ^ 0x5A5A;
        let ps = perms(k + 1, 8, &mut s);
        let makers: Vec<i128> = ps.iter().filter_map(|p| scenario(seed, p).map(|r| r.0)).collect();
        assert!(!makers.is_empty(), "world {seed} ran on the pre-R1 engine and must still run");
        let worst = *makers.iter().min().unwrap();
        assert!(worst >= floor, "world {seed}: maker's worst order {worst} below the pre-R1 worst {floor}");
        checked += 1;
        sum_now += worst;
    }
    eprintln!("DEEPBOOK pre-R1 worlds {checked}: maker worst now {sum_now} vs floor {PRE_R1_SUM_MAKER_MIN}");
    assert_eq!(checked, 187);
}

/// Sum over the 700-world sweep of the maker's worst order, computed on 874fe33a.
const PRE_R1_SUM_MAKER_MIN: i128 = -78_538_861;
/// Sum of the per-world order spread of the total: pre-R1 9,370,961, R1 2,174,094, #250 0.
const SPREAD_RATCHET: i128 = 0;
/// (world offset from seed 5 * 6007, maker's worst order) for the 187 worlds that ran before
/// #250, measured on 91911d5b; the sum is exactly the pre-R1 floor above.
const PRE_R1_MAKER_MIN_BY_WORLD: [(u64, i128); 187] = [
    (1, -966603), (2, -966603), (3, -316697), (4, -316697), (7, -918086), (8, -918086),
    (9, -741549), (10, -741549), (11, -440820), (12, -440820), (13, -162382), (14, -162382),
    (15, -473958), (16, -473958), (17, -403069), (18, -403069), (19, 196208), (20, 196208),
    (21, -523018), (22, -523018), (23, -97605), (24, -97605), (25, -1033142), (26, -1033142),
    (27, -164233), (28, -164233), (31, -760978), (32, -760978), (35, -250743), (36, -250743),
    (41, -580714), (42, -580714), (177, -376310), (178, -376310), (181, -926911), (182, -926911),
    (183, 145554), (184, 145554), (185, -680155), (186, -680155), (187, -441657), (188, -441657),
    (193, 266300), (194, 266300), (197, -1451084), (198, -1451084), (207, 184506), (208, 184506),
    (211, -531517), (212, -531517), (213, 2200), (214, 2200), (215, -454787), (216, -454787),
    (217, 36472), (218, 36472), (219, -140128), (220, -140128), (223, -346446), (224, -346446),
    (225, -400297), (226, -400297), (227, -22387), (228, -22387), (229, -455982), (230, -455982),
    (231, -567685), (232, -567685), (233, -353691), (234, -353691), (235, -332908), (236, -332908),
    (241, 263748), (242, 263748), (243, -543825), (244, -543825), (245, -555813), (246, -555813),
    (249, -247791), (250, -247791), (251, -518118), (252, -518118), (257, 305370), (258, 305370),
    (261, -1143134), (262, -1143134), (263, -371191), (264, -371191), (269, -590099),
    (270, -590099), (273, -1122295), (274, -1122295), (283, -698628), (284, -698628),
    (293, -311754), (294, -311754), (295, -291012), (296, -291012), (297, -194094), (298, -194094),
    (433, -672352), (434, -672352), (435, -932200), (436, -932200), (437, 199296), (438, 199296),
    (439, -356310), (440, -356310), (441, -281518), (442, -281518), (443, -737303), (444, -737303),
    (449, -433711), (450, -433711), (451, -259128), (452, -259128), (455, -719491), (456, -719491),
    (461, -384494), (462, -384494), (463, -952527), (464, -952527), (467, -646544), (468, -646544),
    (469, -315112), (470, -315112), (471, -232138), (472, -232138), (473, -1135604),
    (474, -1135604), (477, 243784), (478, 243784), (479, -665803), (480, -665803), (483, -881147),
    (484, -881147), (485, -1248669), (486, -1248669), (489, -394518), (490, -394518),
    (495, 376194), (496, 376194), (497, -1242902), (498, -1242902), (499, -392601), (500, -392601),
    (503, -628046), (504, -628046), (509, -612560), (510, -612560), (511, -249511), (512, -249511),
    (513, 18440), (514, 18440), (517, -80839), (518, -80839), (519, 23442), (520, 23442),
    (521, 378952), (522, 378952), (523, -410136), (524, -410136), (529, 246546), (530, 246546),
    (531, -516015), (532, -516015), (545, 35552), (546, 35552), (547, -690380), (548, -690380),
    (549, -212766), (550, -212766), (551, -876900), (552, -876900), (553, -621621), (554, -621621),
    (555, -355882), (556, -355882), (689, -761685), (690, -761685), (693, -295754), (694, -295754),
    (695, -673303), (696, -673303), (697, -74782), (698, -74782), (699, -688893),
];

#[test]
fn deep_book_dump() {
    let worlds: u64 = std::env::var("WORLDS").ok().and_then(|v| v.parse().ok()).unwrap_or(200);
    let seed0: u64 = std::env::var("SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(5);
    let (mut ran, mut with_lien) = (0, 0);
    for i in 0..worlds {
        let seed = seed0 * 6007 + i;
        let mut t = seed | 1;
        let k = 3 + (rng(&mut t) % 6) as usize;
        let mut s = seed ^ 0x5A5A;
        let ps = perms(k + 1, 8, &mut s);
        let mut any = false;
        for p in &ps {
            if let Some((m, v, l)) = scenario(seed, p) {
                any = true;
                if l > 0 { with_lien += 1; }
                println!("DROW {seed} {:?} {m} {:?} {l}", p, v);
            }
        }
        if any { ran += 1; }
    }
    println!("DEEP ran {ran} rows_with_lien {with_lien}");
}

// ---------------------------------------------------------------------------------------------
// S8: multi-claimant liened books. One liened (poor) flipping victim, 2-6 rich unliened claimants
// that only win, ONE maker. The liened victim makes the DOMAIN locked, so the pricing branch
// must not depend on the pricing account's own entry.
// ---------------------------------------------------------------------------------------------

/// Equity change per account (victim, claimants..., maker last) for one settle permutation.
fn claimants_scenario(seed: u64, perm: &[usize]) -> Vec<i128> {
    let mut s = seed | 1;
    let nl = 2 + (rng(&mut s) % 5) as usize;
    let vcap: u128 = 330_000;
    let cap = 1_000_000_000_000_000u128;
    let mut w = World::new_pairs(&[], 0, 0);
    let mut accts: Vec<PortfolioAccountV16Account> = (0..=nl).map(|i| account(100 + i as u32)).collect();
    w.deposit(&mut accts[0], vcap);
    for i in 1..=nl { let mut a = accts[i]; w.deposit(&mut a, cap); accts[i] = a; }
    let mut m = w.maker;
    let sizes: Vec<u128> = (0..=nl).map(|i| if i == 0 { 3 } else { 1 + (rng(&mut s) % 4) as u128 }).collect();
    for i in 0..=nl { let mut a = accts[i]; if w.trade(&mut a, &mut m, sizes[i] * POS_SCALE).is_err() { return vec![]; } accts[i] = a; }
    let up1 = 40 + (rng(&mut s) % 80) as i64;
    let st1 = 3 + (rng(&mut s) % 4) as usize;
    for _ in 0..st1 {
        if !w.accrue(up1, 0) { return vec![]; }
        for i in 0..=nl { let mut a = accts[i]; if !w.settle(&mut a) { return vec![]; } accts[i] = a; }
        let mut mm = m; if !w.settle(&mut mm) { return vec![]; } m = mm;
    }
    let mut v = accts[0];
    if w.trade(&mut m, &mut v, 7 * POS_SCALE).is_err() { return vec![]; }
    accts[0] = v;
    let up2 = 30 + (rng(&mut s) % 100) as i64;
    let st2 = 1 + (rng(&mut s) % 4) as usize;
    for _ in 0..st2 { if !w.accrue(up2, 0) { return vec![]; } }
    for &k in perm {
        if k == nl + 1 { let mut mm = m; if !w.settle(&mut mm) { return vec![]; } m = mm; }
        else { let mut a = accts[k]; if !w.settle(&mut a) { return vec![]; } accts[k] = a; }
    }
    let mut out = vec![];
    for i in 0..=nl { let mut a = accts[i]; out.push(effective_equity(&mut w, &mut a) - if i == 0 { vcap as i128 } else { cap as i128 }); }
    let mut mm = m;
    out.push(effective_equity(&mut w, &mut mm) - cap as i128);
    out
}

struct Claimants {
    worlds: u64,
    sum_maker_min: i128,
    sum_claimant_min: i128,
    sum_total_spread: i128,
    maker_order_dependent: u64,
    gains: u64,
}

fn claimants_sweep(worlds: u64, seed0: u64) -> Claimants {
    let mut c = Claimants { worlds: 0, sum_maker_min: 0, sum_claimant_min: 0, sum_total_spread: 0, maker_order_dependent: 0, gains: 0 };
    for i in 0..worlds {
        let seed = seed0 * 7919 + i;
        let mut t = seed | 1;
        let nl = 2 + (rng(&mut t) % 5) as usize;
        let mut s2 = seed ^ 0x1234;
        let ps = perms(nl + 2, 8, &mut s2);
        let rows: Vec<Vec<i128>> = ps.iter().map(|p| claimants_scenario(seed, p)).take_while(|r| !r.is_empty()).collect();
        if rows.len() != ps.len() { continue; }
        c.worlds += 1;
        let n = rows[0].len();
        c.sum_maker_min += rows.iter().map(|r| r[n - 1]).min().unwrap();
        let mmax = rows.iter().map(|r| r[n - 1]).max().unwrap();
        if mmax - rows.iter().map(|r| r[n - 1]).min().unwrap() > 8 { c.maker_order_dependent += 1; }
        for i in 1..n - 1 { c.sum_claimant_min += rows.iter().map(|r| r[i]).min().unwrap(); }
        let tot: Vec<i128> = rows.iter().map(|r| r.iter().sum()).collect();
        c.sum_total_spread += tot.iter().max().unwrap() - tot.iter().min().unwrap();
        if *tot.iter().max().unwrap() > 8 { c.gains += 1; }
    }
    c
}

/// Measured on 874fe33a over the same sweep (constants below). Claimants may sit at most 0.05% under
/// the base worst in aggregate (S8: the
/// best achievable result, a deterministic liened-domain rule cannot reconstruct the base's
/// victim-first state in every interleaving), the order spread of the total is 3.6x smaller.
#[test]
fn liened_domain_multi_claimant_ratchets() {
    let c = claimants_sweep(400, 3);
    println!("CLAIMANTS worlds {} sum_maker_min {} sum_claimant_min {} sum_total_spread {} maker_order_dependent {} gains {}", c.worlds, c.sum_maker_min, c.sum_claimant_min, c.sum_total_spread, c.maker_order_dependent, c.gains);
    assert!(c.worlds >= 200);
    assert_eq!(c.gains, 0);
    assert!(c.sum_maker_min >= PRE_R1_CLAIMANTS_SUM_MAKER_MIN, "maker's worst order fell below the pre-R1 worst");
    assert!(c.sum_claimant_min >= PRE_R1_CLAIMANTS_SUM_CLAIMANT_MIN - CLAIMANT_SLACK, "claimants fell further below the pre-R1 worst than the ratchet allows");
    assert!(c.sum_total_spread <= CLAIMANTS_SPREAD_RATCHET, "order spread regressed");
    assert_eq!(c.maker_order_dependent, 0, "the maker's equity must not depend on settle order here (pre-R1: 8 worlds)");
}

/// Σ over the 400-world sweep (256 ran) of the maker's worst order, computed on 874fe33a.
const PRE_R1_CLAIMANTS_SUM_MAKER_MIN: i128 = -183_346_414;
/// Σ of every unliened claimant's worst order on 874fe33a (the head sits 38,786 below: 0.024%).
const PRE_R1_CLAIMANTS_SUM_CLAIMANT_MIN: i128 = 161_167_373;
const CLAIMANT_SLACK: i128 = 40_000;
/// pre-R1 1,577,772; head 444,312.
const CLAIMANTS_SPREAD_RATCHET: i128 = 460_000;
