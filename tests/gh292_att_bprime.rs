
use percolator::{
    AssetStateV16Account, EngineAssetSlotV16Account, LiquidationOutcomeV16, LiquidationRequestV16,
    Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, PortfolioAccountV16Account,
    PortfolioV16ViewMut, ProvenanceHeaderV16, ProvenanceHeaderV16Account, TradeRequestV16,
    V16Config, V16Error, POS_SCALE,
};

const P0: u64 = 1_000_000;
const MARKET_ID: [u8; 32] = [7; 32];
const OWNER: [u8; 32] = [9; 32];

fn band_cfg(slots: u16) -> V16Config {
    let mut c = V16Config::public_user_fund_with_market_slots(slots, slots as u32, 0, 10);
    c.maintenance_margin_bps = 500;
    c.initial_margin_bps = 1_000;
    c.min_nonzero_mm_req = 10_000;
    c.min_nonzero_im_req = 20_000;
    c.liquidation_fee_bps = 50;
    c.liquidation_fee_cap = 1_000_000_000_000;
    c.min_liquidation_abs = 0;
    c.max_abs_funding_e9_per_slot = 111;
    c.max_price_move_bps_per_slot = 100;
    c.max_accrual_dt_slots = 3;
    c.min_funding_lifetime_slots = 3;
    c.max_trading_fee_bps = 100;
    c.rent_max_e9_per_slot = 23;
    c.max_bankrupt_close_lifetime_slots = 1_000_000;
    c
}

struct W2 {
    header: MarketGroupV16HeaderAccount,
    markets: Vec<Market<u64>>,
    accounts: Vec<PortfolioAccountV16Account>,
    now: u64,
}

impl W2 {
    fn new(cfg: V16Config, deposits: &[u128], n: usize) -> Self {
        cfg.validate_public_user_fund().expect("cfg validates");
        let mut header =
            MarketGroupV16HeaderAccount::new_dynamic(MARKET_ID, cfg, n as u32, 0).unwrap();
        let mut markets: Vec<Market<u64>> = (0..n)
            .map(|i| Market::new(i as u64, EngineAssetSlotV16Account::default()))
            .collect();
        for i in 0..n {
            header
                .activate_empty_asset_slot_not_atomic(
                    i as u32,
                    &mut markets[i].engine,
                    P0,
                    (i + 1) as u64,
                )
                .unwrap();
        }
        let mut accounts = Vec::new();
        for i in 0..deposits.len() {
            let mut k = [1u8; 32];
            k[0] = i as u8;
            k[1] = 1;
            let prov = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
                MARKET_ID, k, OWNER,
            ));
            let mut a = PortfolioAccountV16Account::default();
            a.init_empty_in_place(prov).unwrap();
            accounts.push(a);
        }
        let mut w = Self { header, markets, accounts, now: 1 };
        for (i, d) in deposits.iter().enumerate() {
            w.with(i, |m, a| m.deposit_not_atomic(a, *d)).unwrap();
        }
        w
    }

    fn with<R>(
        &mut self,
        i: usize,
        f: impl FnOnce(&mut MarketGroupV16ViewMut<'_, u64>, &mut PortfolioV16ViewMut<'_>) -> R,
    ) -> R {
        let mut m = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let mut a = PortfolioV16ViewMut::new(&mut self.accounts[i]);
        f(&mut m, &mut a)
    }

    fn market<R>(&mut self, f: impl FnOnce(&mut MarketGroupV16ViewMut<'_, u64>) -> R) -> R {
        let mut m = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        f(&mut m)
    }

    fn asset(&self, i: usize) -> percolator::AssetStateV16 {
        self.markets[i].engine.asset.try_to_runtime().unwrap()
    }

    fn trade(&mut self, ai: usize, long: usize, short: usize, q: u128) -> Result<(), V16Error> {
        let price = self.asset(ai).effective_price;
        let (lo, hi) = if long < short { (long, short) } else { (short, long) };
        let (left, right) = self.accounts.split_at_mut(hi);
        let (a_lo, a_hi) = (&mut left[lo], &mut right[0]);
        let (l, s) = if long < short { (a_lo, a_hi) } else { (a_hi, a_lo) };
        let mut m = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let mut lv = PortfolioV16ViewMut::new(l);
        let mut sv = PortfolioV16ViewMut::new(s);
        m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut lv,
            &mut sv,
            TradeRequestV16 { asset_index: ai, size_q: q as i128, exec_price: price, fee_bps: 0 },
            true,
        )
        .map(|_| ())
    }

    fn step_price(&mut self, ai: usize, target: u64) {
        self.market(|m| m.set_asset_raw_oracle_target_not_atomic(ai, target)).unwrap();
        let a = self.asset(ai);
        let cfg = self.header.config.try_to_runtime_shape().unwrap();
        let dt = (self.now - a.slot_last).min(cfg.max_accrual_dt_slots).max(1);
        let p = a.effective_price;
        let exposed = a.oi_eff_long_q != 0 || a.oi_eff_short_q != 0;
        let capped = if !exposed {
            target
        } else {
            let md =
                (p as u128 * cfg.max_price_move_bps_per_slot as u128 * dt as u128 / 10_000) as u64;
            if target > p { p + md.min(target - p) } else { p - md.min(p - target) }
        };
        let now = self.now;
        let r = self.market(|m| m.accrue_asset_to_not_atomic(ai, now, capped, 0, true));
        if r.is_err() {
            println!("    accrue a{ai} err {r:?}");
        }
        let mut a = self.asset(ai);
        a.raw_oracle_target_price = capped;
        self.markets[ai].engine.asset = AssetStateV16Account::from_runtime(&a);
    }

    fn refresh(&mut self, i: usize) -> Result<percolator::HealthCertV16, V16Error> {
        let mut last = Err(V16Error::Stale);
        for _ in 0..16 {
            last = self.with(i, |m, a| m.full_account_refresh_not_atomic(a));
            if last != Err(V16Error::Stale) && last != Err(V16Error::BStale) {
                break;
            }
        }
        last
    }

    fn legs(&self, i: usize) -> Vec<(u32, bool)> {
        (0..4)
            .map(|s| {
                let l = self.accounts[i].legs[s].try_to_runtime().unwrap();
                (l.asset_index, l.active)
            })
            .collect()
    }
}




const DOMAIN_A0_SHORT: usize = 1;
const DOMAIN_A1_SHORT: usize = 3;

fn close(w: &mut W2, ai: usize) -> Result<LiquidationOutcomeV16, V16Error> {
    w.with(0, |m, a| m.liquidate_account_not_atomic(a, LiquidationRequestV16 { asset_index: ai }))
}

/// account0 long a0 (vs acct1) and long a1 (vs acct2). `dip_a0`: a0 dips one step and account 0
/// refreshes (realizing a small a0 loss netted against a1 gains) before a1 crashes alone.
fn crash_a1_only(dip_a0: bool, insure: &[(usize, u128)]) -> Option<W2> {
    let q = POS_SCALE;
    let mut w = W2::new(band_cfg(2), &[260_000, 50_000_000, 50_000_000], 2);
    w.trade(0, 0, 1, q).ok()?;
    w.trade(1, 0, 2, q).ok()?;
    w.refresh(0).ok()?;
    // phase 1: a1 rises a little (account 0 gains), a0 optionally dips and is refreshed
    for _ in 0..3 {
        w.now += 1;
        w.step_price(1, P0 + P0 / 20);
        w.step_price(0, if dip_a0 { P0 - P0 / 100 } else { P0 });
        w.refresh(0).ok()?;
    }
    // phase 2: a0 returns to P0 (no more a0 loss), a1 crashes
    let mut hit = false;
    for _ in 0..80 {
        w.now += 1;
        w.step_price(0, P0);
        w.step_price(1, 1);
        let _ = w.refresh(0);
        if w.accounts[0].pnl.get() < 0 && w.accounts[0].liquidation_lock != 0 {
            hit = true;
            break;
        }
    }
    if !hit || w.accounts[0].capital.get() != 0 {
        println!("no latch: pnl={} cap={} lock={}", w.accounts[0].pnl.get(), w.accounts[0].capital.get(), w.accounts[0].liquidation_lock);
        return None;
    }
    for &(d, amt) in insure {
        w.market(|m| m.deposit_domain_insurance_not_atomic(d, amt)).ok()?;
    }
    Some(w)
}


fn dump(tag:&str,w:&W2){
    println!("[{tag}] vault={} c_tot={} ins={} pnl_pos_tot={} neg_cnt={} hlock={} mode={}",w.header.vault.get(),w.header.c_tot.get(),w.header.insurance.get(),w.header.pnl_pos_tot.get(),w.header.negative_pnl_account_count.get(),w.header.bankruptcy_hlock_active,w.header.mode);
    for (i,a) in w.accounts.iter().enumerate(){println!("   acct{i}: cap={} pnl={} lock={} legs={:?}",a.capital.get(),a.pnl.get(),a.liquidation_lock,w.legs(i));}
}
#[test]
fn att_probe_dip_vs_control(){
    for dip in [false,true]{
        let mut w=crash_a1_only(dip,&[(DOMAIN_A1_SHORT,100_000_000)]).expect("latch");
        dump(&format!("dip={dip} pre"),&w);
        let r0=close(&mut w,0); let r1=close(&mut w,1);
        println!("close a0 {:?} a1 {:?}",r0.as_ref().map(|o|o.insurance_used).map_err(|e|format!("{e:?}")),r1.as_ref().map(|o|o.insurance_used).map_err(|e|format!("{e:?}")));
        dump(&format!("dip={dip} post"),&w);
    }
}

fn crash_a1_only4(dip_a0: bool, insure: &[(usize, u128)]) -> Option<W2> {
    let q = POS_SCALE;
    let mut w = W2::new(band_cfg(2), &[260_000, 50_000_000, 50_000_000, 50_000_000], 2);
    w.trade(0, 0, 1, q).ok()?;
    w.trade(1, 0, 2, q).ok()?;
    w.refresh(0).ok()?;
    for _ in 0..3 {
        w.now += 1;
        w.step_price(1, P0 + P0 / 20);
        w.step_price(0, if dip_a0 { P0 - P0 / 100 } else { P0 });
        w.refresh(0).ok()?;
    }
    let mut hit = false;
    for _ in 0..80 {
        w.now += 1;
        w.step_price(0, P0);
        w.step_price(1, 1);
        let _ = w.refresh(0);
        if w.accounts[0].pnl.get() < 0 && w.accounts[0].liquidation_lock != 0 { hit = true; break; }
    }
    if !hit || w.accounts[0].capital.get() != 0 { return None; }
    for &(d, amt) in insure { w.market(|m| m.deposit_domain_insurance_not_atomic(d, amt)).ok()?; }
    Some(w)
}

/// winners exit: settle, close legs against fresh acct 3, convert pnl, withdraw everything.
fn exit_winners(w: &mut W2, tag: &str) -> Vec<(usize, u128, u128)> {
    let mut out = vec![];
    for round in 0..3 { w.now += 1; for i in [1usize, 2, 3] { let r = w.refresh(i); if round == 2 { println!("   refresh a{i}: {:?}", r.map(|_| ()).map_err(|e| format!("{e:?}"))); } } }
    dump(&format!("{tag} winners settled"), w);
    // close acct1 short a0 (buy from acct3 selling) and acct2 short a1
    let r = w.trade(0, 1, 3, POS_SCALE); println!("   close a1/a0 via trade {:?}", r);
    let r = w.trade(1, 2, 3, POS_SCALE); println!("   close a2/a1 via trade {:?}", r);
    for round in 0..3 { w.now += 1; for i in [1usize, 2, 3] { let _ = w.refresh(i); } let _ = round; }
    for i in [1usize, 2, 3] {
        let c = w.with(i, |m, a| m.convert_released_pnl_to_capital_not_atomic(a));
        println!("   convert a{i}: {:?} pnl={} cap={}", c.as_ref().map_err(|e| format!("{e:?}")), w.accounts[i].pnl.get(), w.accounts[i].capital.get());
    }
    dump(&format!("{tag} after convert"), w);
    for i in [1usize, 2, 3] {
        let cap = w.accounts[i].capital.get();
        let before = cap;
        let r = w.with(i, |m, a| m.withdraw_not_atomic(a, cap));
        println!("   withdraw a{i} {cap}: {:?}", r.as_ref().map_err(|e| format!("{e:?}")));
        out.push((i, before, w.accounts[i].capital.get()));
    }
    dump(&format!("{tag} after withdraw"), w);
    out
}

#[test]
fn att_conservation_and_drain_dip_vs_control() {
    for dip in [false, true] {
        let mut w = crash_a1_only4(dip, &[(DOMAIN_A1_SHORT, 100_000_000)]).expect("latch");
        let v0 = w.header.vault.get();
        let _ = close(&mut w, 0); let _ = close(&mut w, 1);
        let _ = exit_winners(&mut w, &format!("dip={dip}"));
        let (v, c, i) = (w.header.vault.get(), w.header.c_tot.get(), w.header.insurance.get());
        println!("RESULT dip={dip} vault0={v0} vault={v} c_tot={c} ins={i} slack={}", v as i128 - c as i128 - i as i128);
        assert!(v >= c + i, "conservation broken: vault {v} < c_tot {c} + ins {i}");
    }
}

/// permanent a0 dip (a0 shorts = acct1 win), a1 crash. `a0_final` = a0 price during crash phase.
fn crash_perm(a0_final: u64, insure: &[(usize, u128)]) -> Option<W2> {
    let q = POS_SCALE;
    let mut w = W2::new(band_cfg(2), &[260_000, 50_000_000, 50_000_000, 50_000_000], 2);
    w.trade(0, 0, 1, q).ok()?;
    w.trade(1, 0, 2, q).ok()?;
    w.refresh(0).ok()?;
    for _ in 0..3 {
        w.now += 1;
        w.step_price(1, P0 + P0 / 20);
        w.step_price(0, P0 - P0 / 100);
        w.refresh(0).ok()?;
    }
    let mut hit = false;
    for _ in 0..80 {
        w.now += 1;
        w.step_price(0, a0_final);
        w.step_price(1, 1);
        let _ = w.refresh(0);
        if w.accounts[0].pnl.get() < 0 && w.accounts[0].liquidation_lock != 0 { hit = true; break; }
    }
    if !hit || w.accounts[0].capital.get() != 0 { println!("nohit pnl={} cap={}", w.accounts[0].pnl.get(), w.accounts[0].capital.get()); return None; }
    for &(d, amt) in insure { w.market(|m| m.deposit_domain_insurance_not_atomic(d, amt)).ok()?; }
    Some(w)
}

/// Cross-domain socialization: does the a1 deficit haircut an INNOCENT a0 winner (acct1)?
#[test]
fn att_cross_domain_socialization_of_write_off() {
    let mut res = vec![];
    for (name, insured, order) in [("insured-a1 a1first", true, [1usize,2]), ("insured-a1 a2first", true, [2usize,1]), ("no-insurance a2first", false, [2usize,1])] {
        let ins: Vec<(usize, u128)> = if insured { vec![(DOMAIN_A1_SHORT, 100_000_000)] } else { vec![] };
        let mut w = crash_perm(P0 - P0 / 100, &ins).expect("latch");
        dump(&format!("{name} pre"), &w);
        let r0 = close(&mut w, 0); let r1 = close(&mut w, 1);
        println!("{name}: close a0 {:?} a1 {:?} mode {}", r0.as_ref().map(|o| o.insurance_used).map_err(|e| format!("{e:?}")), r1.as_ref().map(|o| o.insurance_used).map_err(|e| format!("{e:?}")), w.header.mode);
        for round in 0..3 { w.now += 1; for i in [1usize, 2, 3] { let _ = w.refresh(i); } let _ = round; }
        dump(&format!("{name} winners settled"), &w);
        let mut conv = vec![];
        for i in order.iter().copied() {
            let pnl0 = w.accounts[i].pnl.get();
            for _ in 0..3 { w.now += 1; for j in [1usize, 2, 3] { let _ = w.refresh(j); } }
            let cr = w.with(i, |m, a| m.convert_released_pnl_to_capital_not_atomic(a));
            if let Err(e) = &cr { println!("   convert a{i} err {e:?}"); }
            conv.push((i, pnl0, cr.unwrap_or(0)));
        }
        println!("   leftover pnl a1={} a2={} vault-c-ins={}", w.accounts[1].pnl.get(), w.accounts[2].pnl.get(), w.header.vault.get() as i128 - w.header.c_tot.get() as i128 - w.header.insurance.get() as i128);
        println!("RESULT {name}: (acct, claim, converted) = {:?} ins={}", conv, w.header.insurance.get());
        res.push((name, conv));
        assert!(w.header.vault.get() >= w.header.c_tot.get() + w.header.insurance.get());
    }
}

/// Two winners in the SAME loss domain (acct2 vs bankrupt acct0, acct3 vs acct4). The deficit is
/// written off: is the shortfall pro rata, or does the first converter drain it (withdraw before haircut)?
fn two_winner_world() -> Option<W2> {
    let q = POS_SCALE;
    let mut w = W2::new(band_cfg(2), &[260_000, 50_000_000, 50_000_000, 50_000_000, 50_000_000], 2);
    w.trade(0, 0, 1, q).ok()?;
    w.trade(1, 0, 2, q).ok()?;
    w.trade(1, 4, 3, q).ok()?; // acct4 long a1, acct3 short a1
    w.refresh(0).ok()?;
    for _ in 0..3 {
        w.now += 1;
        w.step_price(1, P0 + P0 / 20);
        w.step_price(0, P0 - P0 / 100);
        w.refresh(0).ok()?;
    }
    let mut hit = false;
    for _ in 0..80 {
        w.now += 1;
        w.step_price(0, P0);
        w.step_price(1, 1);
        let _ = w.refresh(0);
        if w.accounts[0].pnl.get() < 0 && w.accounts[0].liquidation_lock != 0 { hit = true; break; }
    }
    if !hit || w.accounts[0].capital.get() != 0 { println!("nohit"); return None; }
    Some(w)
}

#[test]
fn att_same_domain_first_converter_vs_prorata() {
    for order in [[2usize, 3], [3, 2]] {
        let mut w = two_winner_world().expect("world");
        let _ = close(&mut w, 0); let r = close(&mut w, 1);
        println!("close a1: {:?} mode {}", r.map(|o| o.insurance_used), w.header.mode);
        for _ in 0..3 { w.now += 1; for i in 1..5usize { let _ = w.refresh(i); } }
        println!("   close2 {:?} close3 {:?}", w.trade(1, 2, 4, POS_SCALE), w.trade(1, 3, 1, POS_SCALE));
        for _ in 0..4 { w.now += 1; for i in 1..5usize { let _ = w.refresh(i); } }
        dump(&format!("order {order:?} settled"), &w);
        let claims: Vec<i128> = (0..5).map(|i| w.accounts[i].pnl.get()).collect();
        let mut got = vec![];
        for &i in &order {
            for _ in 0..3 { w.now += 1; for j in 1..5usize { let _ = w.refresh(j); } }
            let c = w.with(i, |m, a| m.convert_released_pnl_to_capital_not_atomic(a));
            got.push((i, claims[i], c.map_err(|e| format!("{e:?}"))));
        }
        println!("RESULT order {order:?}: {got:?}  residual={}", w.header.vault.get() as i128 - w.header.c_tot.get() as i128 - w.header.insurance.get() as i128);
        assert!(w.header.vault.get() >= w.header.c_tot.get() + w.header.insurance.get());
    }
}

#[test]
fn att_same_domain_split_counterparties_prorata() {
    for order in [[2usize, 3], [3, 2]] {
        let q = POS_SCALE;
        let mut w = W2::new(band_cfg(2), &[260_000, 50_000_000, 50_000_000, 50_000_000], 2);
        w.trade(0, 0, 1, q).unwrap();
        w.trade(1, 0, 2, q / 2).unwrap();
        w.trade(1, 0, 3, q / 2).unwrap();
        w.refresh(0).unwrap();
        for _ in 0..3 { w.now += 1; w.step_price(1, P0 + P0 / 20); w.step_price(0, P0 - P0 / 100); w.refresh(0).unwrap(); }
        let mut hit = false;
        for _ in 0..80 {
            w.now += 1; w.step_price(0, P0); w.step_price(1, 1); let _ = w.refresh(0);
            if w.accounts[0].pnl.get() < 0 && w.accounts[0].liquidation_lock != 0 { hit = true; break; }
        }
        assert!(hit);
        let _ = close(&mut w, 0); let r = close(&mut w, 1);
        println!("close a1 {:?} mode {}", r.map(|o| o.insurance_used), w.header.mode);
        let mut got = vec![];
        for _ in 0..3 { w.now += 1; for i in 1..4usize { let _ = w.refresh(i); } }
        dump(&format!("order {order:?} settled"), &w);
        let claims: Vec<i128> = (0..4).map(|i| w.accounts[i].pnl.get()).collect();
        for &i in &order {
            for _ in 0..3 { w.now += 1; for j in 1..4usize { let _ = w.refresh(j); } }
            let c = w.with(i, |m, a| m.convert_released_pnl_to_capital_not_atomic(a));
            got.push((i, claims[i], c.map_err(|e| format!("{e:?}"))));
        }
        println!("RESULT order {order:?}: {got:?} left pnl a2={} a3={} resid={}", w.accounts[2].pnl.get(), w.accounts[3].pnl.get(), w.header.vault.get() as i128 - w.header.c_tot.get() as i128 - w.header.insurance.get() as i128);
        assert!(w.header.vault.get() >= w.header.c_tot.get() + w.header.insurance.get());
    }
}

/// ATTRIBUTABLE loss (control world: record = a1 short domain, 100M insurance in that domain).
/// Liquidation charges 435 to insurance. Closing the same account by TRADES instead: written off?
#[test]
fn att_attributable_loss_escapes_insurance_via_trade_close() {
    let mut w = crash_a1_only(false, &[(DOMAIN_A1_SHORT, 100_000_000)]).expect("latch");
    let ins0 = w.header.insurance.get();
    let r_a0 = w.trade(0, 1, 0, POS_SCALE);
    let r_a1 = w.trade(1, 2, 0, POS_SCALE);
    println!("trade-close a0 {:?}  a1 {:?}", r_a0, r_a1);
    for _ in 0..4 { w.now += 1; let _ = w.refresh(0); }
    dump("after trade closes", &w);
    println!("RESULT trade-close: insurance {} -> {} (liquidation control charged 435), pnl={} mode={}", ins0, w.header.insurance.get(), w.accounts[0].pnl.get(), w.header.mode);
    // b-prime-2 (a): an attributable, insured loss closed by trades is charged to its domain's
    // insurance exactly like the liquidation control (435), not written off.
    assert_eq!(ins0 - w.header.insurance.get(), 435, "attributable loss must be charged to insurance");
    assert_eq!(w.accounts[0].pnl.get(), 0);
    assert_eq!(w.header.mode, 0);
}

/// Write-off marks the hlock UNATTRIBUTED, so its claim term is the GLOBAL pnl_pos_tot==0. An idle
/// winner in an UNRELATED domain (acct1: short a0, +10000, open leg) then holds the group h-lock
/// after the deficit itself is gone. Control: same crash but attributable (liquidation charge)
/// and no a0 winner -> clears.
#[test]
fn att_writeoff_rearms_global_claim_term_latch() {
    // permanent a0 dip: acct1 (short a0) holds positive pnl with an OPEN leg; a1 crashes.
    let mut w = crash_perm(P0 - P0 / 100, &[(DOMAIN_A1_SHORT, 100_000_000)]).expect("latch");
    for _ in 0..2 { w.now += 1; let _ = w.refresh(1); }
    let wire_before = w.header.bankruptcy_hlock_active;
    let _ = close(&mut w, 0);
    let r = close(&mut w, 1);
    assert_eq!(w.header.bankruptcy_hlock_active, wire_before, "b-prime-2: the write-off marks nothing new on the h-lock");
    println!("final close {:?} mode {}", r.map(|o| o.insurance_used), w.header.mode);
    for _ in 0..6 { w.now += 1; for i in 1..4usize { let _ = w.refresh(i); } let _ = w.market(|m| m.try_clear_bankruptcy_hlock_not_atomic()); }
    dump("write-off world, after many cranks", &w);
    let hl = w.header.bankruptcy_hlock_active;
    let cleared = w.market(|m| m.try_clear_bankruptcy_hlock_not_atomic()).unwrap();
    println!("RESULT hlock={} cleared={} neg_cnt={} pnl_pos_tot={}", hl, cleared, w.header.negative_pnl_account_count.get(), w.header.pnl_pos_tot.get());
    // try an insurance withdrawal on the UNRELATED domain 1 / the insured domain 3
    let r3 = w.market(|m| m.withdraw_domain_insurance_not_atomic(DOMAIN_A1_SHORT, 1));
    println!("withdraw_domain_insurance(3,1) -> {:?}", r3);
    // The global claim term (pnl_pos_tot == 0) is pre-existing P2b-L1 behaviour for every multi-asset
    // bankruptcy (the byte was already 1 before the write-off). It is NOT permanent: the idle flat
    // winners can convert (permissionless refresh + owner convert), and then the h-lock clears.
    if !cleared {
        for _ in 0..3 {
            w.now += 1;
            for i in 1..4usize {
                let _ = w.refresh(i);
                let _ = w.with(i, |m, a| m.convert_released_pnl_to_capital_not_atomic(a));
            }
        }
    }
    let cleared_after = w.market(|m| m.try_clear_bankruptcy_hlock_not_atomic()).unwrap();
    assert!(cleared_after, "hlock must clear once the idle unrelated winners convert (pnl_pos_tot={})", w.header.pnl_pos_tot.get());
}

/// Honest open winner in an unrelated market (acct4 short a0 vs acct5 long a0, a0 stays 1% down):
/// it can never convert while its leg is open, so after ANY write-off the group hlock stays armed.
#[test]
fn att_writeoff_hlock_pinned_by_honest_open_winner() {
    let q = POS_SCALE;
    let mut w = W2::new(band_cfg(2), &[260_000, 50_000_000, 50_000_000, 50_000_000, 50_000_000, 50_000_000], 2);
    w.trade(0, 5, 4, q).unwrap(); // acct5 long a0, acct4 short a0
    w.now += 1; w.step_price(0, P0); w.step_price(1, P0); for i in [4usize, 5] { let _ = w.refresh(i); }
    let r = w.trade(0, 0, 1, q); println!("t2 {r:?}"); r.unwrap();
    let r = w.trade(1, 0, 2, q); println!("t3 {r:?}"); r.unwrap();
    let _ = w.refresh(0);
    for _ in 0..3 { w.now += 1; w.step_price(1, P0 + P0 / 20); w.step_price(0, P0 - P0 / 100); let _ = w.refresh(0); }
    let mut hit = false;
    for _ in 0..80 {
        w.now += 1; w.step_price(0, P0 - P0 / 100); w.step_price(1, 1); let _ = w.refresh(0);
        if w.accounts[0].pnl.get() < 0 && w.accounts[0].liquidation_lock != 0 { hit = true; break; }
    }
    assert!(hit);
    w.market(|m| m.deposit_domain_insurance_not_atomic(DOMAIN_A1_SHORT, 100_000_000)).unwrap();
    for _ in 0..2 { w.now += 1; let _ = w.refresh(4); let _ = w.refresh(5); }
    println!("acct4 pnl={} open={:?}", w.accounts[4].pnl.get(), w.legs(4));
    println!("pre-close hlock={} pnl_pos_tot={}", w.header.bankruptcy_hlock_active, w.header.pnl_pos_tot.get());
    let hl_pre = w.header.bankruptcy_hlock_active;
    let _ = close(&mut w, 0); let r = close(&mut w, 1);
    println!("final close {:?} ins={}", r.map(|o| o.insurance_used), w.header.insurance.get());
    for _ in 0..6 { w.now += 1; for i in 1..6usize { let _ = w.refresh(i); } }
    let cl = w.market(|m| m.try_clear_bankruptcy_hlock_not_atomic()).unwrap();
    dump("honest open winner", &w);
    let conv = w.with(4, |m, a| m.convert_released_pnl_to_capital_not_atomic(a));
    println!("RESULT hlock={} cleared={} acct4 convert={:?}", w.header.bankruptcy_hlock_active, cl, conv.map_err(|e| format!("{e:?}")));
    // KNOWN, PRE-EXISTING, NOT FIXED HERE (claim-term over-lock, issue (ii)): an honest winner with an
    // OPEN leg cannot convert (LockActive) and its reducing trade is refused (AdlReduceOnly in this
    // fixture), so it keeps the global claim term armed. The byte was already 1 BEFORE the
    // write-off (set by the original multi-asset deficit event), so the write-off changes nothing:
    // assert exactly that, plus that the deficit itself is gone and nothing else blocks the clear.
    assert_eq!(w.header.negative_pnl_account_count.get(), 0);
    assert_eq!(w.header.bankruptcy_hlock_active, hl_pre, "write-off must not change the h-lock byte");
    assert_ne!(w.header.pnl_pos_tot.get(), 0, "the only remaining blocker is the winner's claim");
}

/// Req 27 on the UNATTRIBUTABLE path + double write-off.
#[test]
fn att_order_independence_and_double_writeoff_unattributable() {
    let mut a = crash_a1_only(true, &[(DOMAIN_A1_SHORT, 100_000_000)]).expect("latch");
    let mut b = crash_a1_only(true, &[(DOMAIN_A1_SHORT, 100_000_000)]).expect("latch");
    let ra = (close(&mut a, 0).map(|o| o.insurance_used).map_err(|e| format!("{e:?}")), close(&mut a, 1).map(|o| o.insurance_used).map_err(|e| format!("{e:?}")));
    let rb = (close(&mut b, 1).map(|o| o.insurance_used).map_err(|e| format!("{e:?}")), close(&mut b, 0).map(|o| o.insurance_used).map_err(|e| format!("{e:?}")));
    println!("order 0-1 {:?} {:?}\norder 1-0 {:?} {:?}", ra, snapshot(&a), rb, snapshot(&b));
    assert_eq!(snapshot(&a), snapshot(&b), "req 27: close order changed the outcome");
    assert_eq!((a.accounts[0].pnl.get(), a.header.negative_pnl_account_count.get()), (b.accounts[0].pnl.get(), b.header.negative_pnl_account_count.get()));
    // double write-off: repeat everything; nothing may move
    let s0 = snapshot(&a);
    let pnl_pos = a.header.pnl_pos_tot.get();
    for ai in [0, 1] { assert!(close(&mut a, ai).is_err(), "second close must fail"); }
    for _ in 0..3 { a.now += 1; let _ = a.refresh(0); let _ = a.market(|m| m.try_clear_bankruptcy_hlock_not_atomic()); }
    assert_eq!(s0, snapshot(&a));
    assert_eq!(pnl_pos, a.header.pnl_pos_tot.get());
    assert_eq!(a.header.negative_pnl_account_count.get(), 0);
}

/// Permissionless liveness (req 35): the write-off must be reachable by a keeper that does NOT own the account
/// and by plain refresh on the trade-path flat state (no liquidation possible).
#[test]
fn att_trade_close_flat_deficit_resolved_by_refresh_only() {
    let mut w = crash_a1_only(true, &[]).expect("latch");
    let r_a0 = w.trade(0, 1, 0, POS_SCALE);
    let r_a1 = w.trade(1, 2, 0, POS_SCALE);
    println!("trade closes {r_a0:?} {r_a1:?}");
    for _ in 0..3 { w.now += 1; let _ = w.refresh(0); }
    dump("trade-closed unattributable", &w);
    assert_eq!(w.accounts[0].pnl.get(), 0);
    assert_eq!(w.header.negative_pnl_account_count.get(), 0);
}

fn snapshot(w: &W2) -> (u128, u128, u128, u128, u128, u128, i128, i128, u128) {
    (w.header.vault.get(), w.header.c_tot.get(), w.header.insurance.get(),
     w.asset(0).b_short_num, w.asset(1).b_short_num, w.header.mode as u128,
     w.accounts[0].pnl.get(), w.accounts[2].pnl.get(), w.header.pnl_pos_tot.get())
}

/// d6 adapted: shape steering. single leg vs two legs (no dip) vs two legs + a0 dip. Who pays the same a1 deficit?
#[test]
fn att_d6_shape_steers_payer() {
    for (two_legs, dip) in [(false, false), (true, false), (true, true)] {
        let q = POS_SCALE;
        let mut w = W2::new(band_cfg(2), &[260_000, 50_000_000, 50_000_000], 2);
        if two_legs { w.trade(0, 0, 1, q).unwrap(); }
        w.trade(1, 0, 2, q).unwrap();
        w.refresh(0).unwrap();
        for _ in 0..3 { w.now += 1; w.step_price(1, P0 + P0 / 20); w.step_price(0, if dip { P0 - P0 / 100 } else { P0 }); w.refresh(0).unwrap(); }
        w.market(|m| m.deposit_domain_insurance_not_atomic(DOMAIN_A1_SHORT, 100_000_000)).unwrap();
        for _ in 0..40 { w.now += 1; w.step_price(0, P0); w.step_price(1, 1); let _ = w.refresh(0);
            if w.accounts[0].pnl.get() < 0 && w.accounts[0].capital.get() == 0 { break; } }
        let ins0 = w.header.insurance.get();
        let mut used = 0u128;
        for ai in [0usize, 1] { if let Ok(o) = close(&mut w, ai) { used += o.insurance_used; } }
        println!("D6 two_legs={two_legs} dip={dip} insurance {ins0}->{} used={used} pnl={} mode={}", w.header.insurance.get(), w.accounts[0].pnl.get(), w.header.mode);
    }
}
