
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


fn crash_cfg(cfg: V16Config, dip_a0: bool, insure: &[(usize, u128)]) -> Option<W2> {
    let q = POS_SCALE;
    let mut w = W2::new(cfg, &[260_000, 50_000_000, 50_000_000], 2);
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

fn cfg_chunk(chunk: u128) -> V16Config {
    let mut c = band_cfg(2);
    c.public_b_chunk_atoms = chunk;
    c
}

fn dump(w: &W2, tag: &str) {
    let a = &w.accounts[0];
    println!("[{tag}] mode={} pnl={} cap={} lock={} legs={:?} negcnt={} hlock={} pnl_pos_tot={} ins={} vault={} c_tot={} b_short=({},{}) wsum=({},{})",
        w.header.mode, a.pnl.get(), a.capital.get(), a.liquidation_lock, w.legs(0),
        w.header.negative_pnl_account_count.get(), w.header.bankruptcy_hlock_active, w.header.pnl_pos_tot.get(),
        w.header.insurance.get(), w.header.vault.get(), w.header.c_tot.get(),
        w.asset(0).b_short_num, w.asset(1).b_short_num, w.asset(0).loss_weight_sum_short, w.asset(1).loss_weight_sum_short);
}

fn conserved(w: &W2) -> bool { w.header.vault.get() >= w.header.c_tot.get() + w.header.insurance.get() }

/// Probe: attributed a1 deficit, partial capacity via small chunk, partial insurance; both close orders.
#[test]
fn r4_partial_charge_accounting() {
    for order in 0..2 {
      for &chunk in &[1u128, 7, 50, 333, 334, 335, 336, 430, 433, 434, 435, 436, 437, 440, 1_000, 1_000_000_000_000] {
        for &ins in &[0u128, 100, 10_000] {
            let Some(mut w) = crash_cfg(cfg_chunk(chunk), false, &[(DOMAIN_A1_SHORT, ins)]) else { println!("order{order} chunk{chunk} ins{ins}: no latch"); continue };
            let d0 = -w.accounts[0].pnl.get();
            let ins0 = w.header.insurance.get();
            let b0 = (w.asset(0).b_short_num, w.asset(1).b_short_num);
            let ws = (w.asset(0).loss_weight_sum_short, w.asset(1).loss_weight_sum_short);
            for _ in 0..3 { for i in [1usize,2] { let _ = w.refresh(i); } }
            let wp0 = (w.accounts[1].pnl.get(), w.accounts[2].pnl.get());
            if chunk==1000 && ins==0 && order==0 { println!("ASSET1 before {:?}", w.asset(1)); println!("ACCT2 before {:?}", w.accounts[2].legs[0].try_to_runtime().unwrap()); }
            let seq: [usize; 2] = if order == 0 { [1, 0] } else { [0, 1] };
            let mut reported = (0u128, 0u128, 0u128);
            let mut log = String::new();
            for &ai in &seq {
                for _try in 0..6 {
                    let r = close(&mut w, ai);
                    match &r { Ok(o) => { reported.0 += o.insurance_used; reported.1 += o.residual_booked; reported.2 += o.explicit_loss; log += &format!(" close a{ai} ok(ins={},bk={},ex={})", o.insurance_used, o.residual_booked, o.explicit_loss); break; }
                               Err(e) => { log += &format!(" close a{ai} {e:?}"); break; } }
                }
                assert!(conserved(&w), "conservation after close a{ai}");
            }
            for _ in 0..3 { for i in [1usize,2] { let _ = w.refresh(i); } }
            let wp1 = (w.accounts[1].pnl.get(), w.accounts[2].pnl.get());
            println!("   winners pnl before {wp0:?} after {wp1:?} loss_a0={} loss_a1={}", wp0.0-wp1.0, wp0.1-wp1.1);

            if chunk==1000 && ins==0 && order==0 { println!("ASSET1 after {:?}", w.asset(1)); println!("ACCT2 after {:?} {:?}", w.accounts[2].legs[0].try_to_runtime().unwrap(), w.legs(2)); }
            let ins_used = ins0 - w.header.insurance.get();
            let db1 = w.asset(1).b_short_num - b0.1; let db0 = w.asset(0).b_short_num - b0.0;
            let dbe = w.asset(1).b_epoch_start_short_num - 0 + db1;
            let booked_atoms_est = dbe * ws.1 / 1_000_000_000_000_000_000_000u128; assert_eq!(booked_atoms_est, reported.1 + reported.2, "B on winners (epoch-start index) equals reported booked");
            println!("order{order} chunk{chunk} ins{ins}: D0={d0} reported={reported:?} ins_used={ins_used} dB(a0,a1)=({db0},{db1}) ws1={} est={booked_atoms_est} final pnl={} mode={} negcnt={} {log}",
                ws.1, w.accounts[0].pnl.get(), w.header.mode, w.header.negative_pnl_account_count.get());
            assert_eq!(w.header.mode, 0, "no Recovery");
            assert!(ins_used <= ins.min(d0 as u128));
            assert_eq!(db0, 0, "domain a0 untouched");
            assert!(w.accounts[0].pnl.get() == 0 || w.legs(0).iter().any(|l| l.1));
        }
      }
    }
}

#[test]
fn r4_winner_realization_probe() {
    let mut w = crash_cfg(cfg_chunk(1_000_000_000_000), false, &[(DOMAIN_A1_SHORT, 100)]).unwrap();
    let _ = close(&mut w, 1); let _ = close(&mut w, 0);
    dump(&w, "after closes");
    for i in 1..3 { for k in 0..4 { let r = w.refresh(i); println!("refresh acct{i} #{k}: {:?} pnl={} cap={} legs={:?}", r.as_ref().map(|c| c.certified_liq_deficit).map_err(|e| format!("{e:?}")), w.accounts[i].pnl.get(), w.accounts[i].capital.get(), w.legs(i)); } }
    dump(&w, "after refresh");
}

fn three_leg(chunk: u128, ins: &[(usize, u128)]) -> Option<W2> {
    let q = POS_SCALE;
    let n = 3usize;
    let mut c = band_cfg(n as u16);
    c.public_b_chunk_atoms = chunk;
    let mut w = W2::new(c, &[390_000, 50_000_000, 50_000_000, 50_000_000], n);
    for i in 0..n { w.trade(i, 0, i + 1, q).ok()?; }
    w.now += 5; let _ = w.refresh(0);
    for _ in 0..120 {
        w.now += 1;
        for i in 0..n { w.step_price(i, 1); }
        let _ = w.refresh(0);
        if w.accounts[0].pnl.get() < 0 && w.accounts[0].capital.get() == 0 && w.accounts[0].liquidation_lock != 0 { break; }
    }
    if !(w.accounts[0].pnl.get() < 0 && w.accounts[0].capital.get() == 0) { return None; }
    for &(d, a) in ins { w.market(|m| m.deposit_domain_insurance_not_atomic(d, a)).ok()?; }
    Some(w)
}

fn perms3() -> Vec<[usize;3]> { vec![[0,1,2],[0,2,1],[1,0,2],[1,2,0],[2,0,1],[2,1,0]] }

/// Sweep: every close order x insurance x chunk x dip; never Recovery, always conserved, terminates flat with pnl 0 / counter 0.
#[test]
fn r4_recovery_sweep_two_and_three_assets() {
    let mut runs = 0; let mut fails = Vec::new();
    for &chunk in &[1u128, 100, 435, 1_000_000_000_000] {
        for ins_cfg in 0..5usize {
            let insv: Vec<(usize,u128)> = match ins_cfg { 0 => vec![], 1 => (0..4).map(|d|(d,100)).collect(), 2 => (0..4).map(|d|(d,10_000)).collect(), 3 => vec![(1,100),(3,10_000)], _ => vec![(0,10_000),(2,10_000)] };
            for dip in [false, true] {
              for order in [[0usize,1],[1,0]] {
                let Some(mut w) = crash_cfg(cfg_chunk(chunk), dip, &insv) else { continue };
                runs += 1;
                for &ai in &order { for _ in 0..3 { if close(&mut w, ai).is_ok() { break; } } assert!(conserved(&w)); }
                for _ in 0..4 { let _ = w.refresh(0); }
                let ok = w.header.mode == 0 && w.accounts[0].pnl.get() == 0 && w.header.negative_pnl_account_count.get() == 0;
                if !ok { fails.push(format!("2asset chunk{chunk} ins{ins_cfg} dip{dip} order{order:?}: mode={} pnl={} negcnt={} legs={:?}", w.header.mode, w.accounts[0].pnl.get(), w.header.negative_pnl_account_count.get(), w.legs(0))); }
              }
            }
            for p in perms3() {
                let Some(mut w) = three_leg(chunk, &insv) else { continue };
                runs += 1;
                for &ai in &p { for _ in 0..3 { if close(&mut w, ai).is_ok() { break; } } assert!(conserved(&w)); }
                for _ in 0..4 { let _ = w.refresh(0); }
                let ok = w.header.mode == 0 && w.accounts[0].pnl.get() == 0 && w.header.negative_pnl_account_count.get() == 0;
                if !ok { fails.push(format!("3asset chunk{chunk} ins{ins_cfg} order{p:?}: mode={} pnl={} negcnt={} legs={:?}", w.header.mode, w.accounts[0].pnl.get(), w.header.negative_pnl_account_count.get(), w.legs(0))); }
            }
        }
    }
    println!("RUNS={runs} FAILS={}", fails.len());
    for f in &fails { println!("FAIL {f}"); }
    assert!(runs > 100);
    assert!(fails.is_empty());
}

/// Trade-route sweep: bankrupt locked account closes both legs by trades (reduce-only) with its counterparties.
#[test]
fn r4_trade_route_sweep() {
    let mut runs = 0; let mut fails = Vec::new();
    for &chunk in &[1u128, 100, 435, 1_000_000_000_000] {
        for ins_cfg in 0..4usize {
            let insv: Vec<(usize,u128)> = match ins_cfg { 0 => vec![], 1 => (0..4).map(|d|(d,100)).collect(), 2 => (0..4).map(|d|(d,10_000)).collect(), _ => vec![(3,100)] };
            for dip in [false, true] {
              for order in [[0usize,1],[1,0]] {
                for split in [false, true] {
                let Some(mut w) = crash_cfg(cfg_chunk(chunk), dip, &insv) else { continue };
                runs += 1;
                let d0 = -w.accounts[0].pnl.get();
                let ins0 = w.header.insurance.get();
                let q = POS_SCALE;
                let mut errs = String::new();
                for &ai in &order {
                    let cp = ai + 1; // counterparty account (a0 -> acct1, a1 -> acct2)
                    let parts: Vec<u128> = if split { vec![q/2, q - q/2] } else { vec![q] };
                    for p in parts { let r = w.trade(ai, cp, 0, p); if let Err(e) = r { errs += &format!(" trade a{ai} {p}: {e:?};"); } }
                    assert!(conserved(&w));
                }
                for _ in 0..4 { let _ = w.refresh(0); let _ = w.with(0, |m,a| m.permissionless_auto_crank_not_atomic(a, percolator::AutoCrankWorkV16 { now_slot: m.header.current_slot.get(), observations: &[], resolved_close_fee_rate_per_slot: 0 })); }
                let ins_used = ins0 - w.header.insurance.get();
                let ok = w.header.mode == 0 && w.accounts[0].pnl.get() == 0 && w.header.negative_pnl_account_count.get() == 0 && ins_used <= d0 as u128;
                if !ok { fails.push(format!("chunk{chunk} ins{ins_cfg} dip{dip} order{order:?} split{split}: mode={} pnl={} negcnt={} ins_used={ins_used} d0={d0} legs={:?} errs={errs}", w.header.mode, w.accounts[0].pnl.get(), w.header.negative_pnl_account_count.get(), w.legs(0))); }
                }
              }
            }
        }
    }
    println!("TRADE RUNS={runs} FAILS={}", fails.len());
    for f in fails.iter().take(25) { println!("FAIL {f}"); }
    assert!(runs > 100);
}

/// Stale record after a PARTIAL pre-charge: deficit remainder on a1's record, then a fresh loss on a0 is realized.
#[test]
fn r4_stale_record_after_partial_charge() {
    for &(chunk, ins1) in &[(100u128, 50u128), (100, 0), (1_000_000_000_000, 0)] {
        let Some(mut w) = crash_cfg(cfg_chunk(chunk), false, &[(DOMAIN_A0_SHORT, 5_000), (DOMAIN_A1_SHORT, ins1)]) else { println!("nolatch"); continue };
        let ia0 = w.market(|m| m.domain_insurance_withdraw_capacity(DOMAIN_A0_SHORT).unwrap());
        let ia1 = w.market(|m| m.domain_insurance_withdraw_capacity(DOMAIN_A1_SHORT).unwrap());
        println!("== chunk{chunk} ins1={ins1}: pnl={} prov={} ins domains a0={ia0} a1={ia1}", w.accounts[0].pnl.get(), w.accounts[0].loss_provenance.get());
        let r = close(&mut w, 1);
        println!("close a1: {:?}", r.as_ref().map(|o| (o.insurance_used, o.residual_booked)).map_err(|e| format!("{e:?}")));
        dump(&w, "after a1 close");
        println!("prov={}", w.accounts[0].loss_provenance.get());
        // now a0 crashes: realize loss on a0
        let mut crashed = false;
        for _ in 0..80 { w.now += 1; w.step_price(0, 1); let _ = w.refresh(0); if w.accounts[0].pnl.get() < -300 { crashed = true; break; } }
        println!("a0 crashed={crashed} pnl={} prov={} cap={}", w.accounts[0].pnl.get(), w.accounts[0].loss_provenance.get(), w.accounts[0].capital.get());
        let pnl_before = w.accounts[0].pnl.get();
        let i0 = w.market(|m| m.domain_insurance_withdraw_capacity(DOMAIN_A0_SHORT).unwrap());
        let i1 = w.market(|m| m.domain_insurance_withdraw_capacity(DOMAIN_A1_SHORT).unwrap());
        let r = close(&mut w, 0);
        let j0 = w.market(|m| m.domain_insurance_withdraw_capacity(DOMAIN_A0_SHORT).unwrap());
        let j1 = w.market(|m| m.domain_insurance_withdraw_capacity(DOMAIN_A1_SHORT).unwrap());
        println!("close a0: {:?}; pnl {pnl_before}->{}; ins a0 {i0}->{j0}; ins a1 {i1}->{j1}; mode={}", r.as_ref().map(|o| (o.insurance_used, o.residual_booked)).map_err(|e| format!("{e:?}")), w.accounts[0].pnl.get(), w.header.mode);
        assert_eq!(w.header.mode, 0);
        assert!(conserved(&w));
    }
}

fn crash_extra(cfg: V16Config, insure: &[(usize, u128)]) -> Option<W2> {
    let q = POS_SCALE;
    let mut w = W2::new(cfg, &[260_000, 50_000_000, 50_000_000, 50_000_000, 50_000_000], 2);
    w.trade(0, 0, 1, q).ok()?;
    w.trade(1, 0, 2, q).ok()?;
    w.trade(1, 3, 4, 2 * q).ok()?; // honest pair on a1 so the winner side survives L's close
    w.refresh(0).ok()?;
    for _ in 0..3 { w.now += 1; w.step_price(1, P0 + P0 / 20); w.step_price(0, P0); w.refresh(0).ok()?; }
    let mut hit = false;
    for _ in 0..80 {
        w.now += 1; w.step_price(0, P0); w.step_price(1, 1); let _ = w.refresh(0);
        if w.accounts[0].pnl.get() < 0 && w.accounts[0].liquidation_lock != 0 { hit = true; break; }
    }
    if !hit || w.accounts[0].capital.get() != 0 { println!("no latch extra pnl={} cap={}", w.accounts[0].pnl.get(), w.accounts[0].capital.get()); return None; }
    for &(d, a) in insure { w.market(|m| m.deposit_domain_insurance_not_atomic(d, a)).ok()?; }
    Some(w)
}

#[test]
fn r4_second_chunk_with_surviving_winners() {
    for &chunk in &[50u128, 100, 200, 1_000_000_000_000] {
      for &ins in &[0u128, 100] {
        for order in [[0usize,1],[1,0]] {
            let Some(mut w) = crash_extra(cfg_chunk(chunk), &[(DOMAIN_A1_SHORT, ins)]) else { continue };
            let d0 = -w.accounts[0].pnl.get();
            let ins0 = w.header.insurance.get();
            let b0 = w.asset(1).b_short_num + w.asset(1).b_epoch_start_short_num;
            let ws0 = w.asset(1).loss_weight_sum_short;
            let mut rep = (0u128,0u128,0u128);
            for &ai in &order { match close(&mut w, ai) { Ok(o) => { rep.0+=o.insurance_used; rep.1+=o.residual_booked; rep.2+=o.explicit_loss; } Err(e) => println!("  close a{ai} err {e:?}") } assert!(conserved(&w)); }
            for _ in 0..3 { let _ = w.refresh(0); }
            let a = w.asset(1);
            let b1 = a.b_short_num + a.b_epoch_start_short_num;
            let booked_idx = (b1 - b0) * ws0 / 1_000_000_000_000_000_000_000u128;
            println!("chunk{chunk} ins{ins} order{order:?}: D0={d0} reported={rep:?} ins_used={} idx_booked~{booked_idx} ws0={ws0} ws_now={} mode={} pnl={} negcnt={} modeShort={:?}", ins0 - w.header.insurance.get(), a.loss_weight_sum_short, w.header.mode, w.accounts[0].pnl.get(), w.header.negative_pnl_account_count.get(), a.mode_short);
            assert!(rep.0 + rep.1 + rep.2 <= d0 as u128, "charged more than the deficit");
            assert_eq!(w.header.mode, 0);
        }
      }
    }
}

/// Req 27 + honest reporting with surviving winners and a binding chunk: SHOULD hold, does not on f343d568.
#[test]
fn r4_order_independence_and_reporting_with_surviving_winners() {
    let mut bad = vec![];
    for &chunk in &[50u128, 100, 200] {
        let mut res = vec![];
        for order in [[0usize,1],[1,0]] {
            let mut w = crash_extra(cfg_chunk(chunk), &[]).unwrap();
            let b0 = w.asset(1).b_short_num + w.asset(1).b_epoch_start_short_num; let ws0 = w.asset(1).loss_weight_sum_short;
            let mut rep = 0u128;
            for &ai in &order { let o = close(&mut w, ai).unwrap(); rep += o.insurance_used + o.residual_booked + o.explicit_loss; }
            let a = w.asset(1);
            let idx = (a.b_short_num + a.b_epoch_start_short_num - b0) * ws0 / 1_000_000_000_000_000_000_000u128;
            res.push((rep, idx));
        }
        println!("chunk{chunk}: order[0,1] (reported,actual)={:?} order[1,0]={:?}", res[0], res[1]);
        if res[0].0 + 1 < res[0].1 { bad.push(format!("chunk{chunk} reported {} < actual {}", res[0].0, res[0].1)); }
        if res[0].1 != res[1].1 { bad.push(format!("chunk{chunk} order-dependent {} vs {}", res[0].1, res[1].1)); }
    }
    assert!(bad.is_empty(), "{bad:?}");
}
