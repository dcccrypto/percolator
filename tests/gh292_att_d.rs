
use percolator::{AutoCrankWorkV16, 
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
const DOM_A1_SHORT: usize = 3;

fn hl(w: &W2) -> u8 { w.header.bankruptcy_hlock_active }
fn cons(w: &W2, tag: &str) {
    let h = &w.header;
    let senior = h.c_tot.get() + h.insurance.get();
    println!("  [{tag}] vault={} c_tot={} ins={} pnl_pos_tot={} neg_cnt={} hlock={} mode={}",
        h.vault.get(), h.c_tot.get(), h.insurance.get(), h.pnl_pos_tot.get(),
        h.negative_pnl_account_count.get(), h.bankruptcy_hlock_active, h.mode);
    assert!(h.vault.get() >= senior, "[{tag}] vault < c_tot + insurance: DRAIN");
}

/// n_win winners (accounts 2..) short a1 vs loser acct0 long a1 (and acct0 long a0 vs acct1).
/// a1 rises a bit, then crashes to 1: loser goes bankrupt, locked, flat after two closes.
fn scenario(n_win: usize, ins: u128, win_cap: u128) -> Option<W2> { scenario_d(n_win, ins, win_cap, 260_000, 0) }
fn scenario_d(n_win: usize, ins: u128, win_cap: u128, loser_dep: u128, extra_depth: usize) -> Option<W2> {
    let q = POS_SCALE;
    let mut deps = vec![loser_dep, 50_000_000];
    for _ in 0..n_win { deps.push(win_cap); }
    let mut w = W2::new(band_cfg(2), &deps, 2);
    w.trade(0, 0, 1, q).ok()?;
    // loser long q on a1 split across winners
    for k in 0..n_win { w.trade(1, 0, 2 + k, q / n_win as u128).ok()?; }
    w.refresh(0).ok()?;
    for _ in 0..3 { w.now += 1; w.step_price(1, P0 + P0 / 20); w.step_price(0, P0); w.refresh(0).ok()?; }
    let mut hit = false;
    for _ in 0..80 {
        w.now += 1; w.step_price(0, P0); w.step_price(1, 1);
        let _ = w.refresh(0);
        if w.accounts[0].pnl.get() < 0 && w.accounts[0].liquidation_lock != 0 { hit = true; break; }
    }
    if hit { for _ in 0..extra_depth { w.now += 1; w.step_price(0, P0); w.step_price(1, 1); let _ = w.refresh(0); } }
    if !hit || w.accounts[0].capital.get() != 0 { println!("no latch pnl={} cap={}", w.accounts[0].pnl.get(), w.accounts[0].capital.get()); return None; }
    if ins > 0 { w.market(|m| m.deposit_domain_insurance_not_atomic(DOM_A1_SHORT, ins)).ok()?; }
    Some(w)
}

fn winner_cash_out(w: &mut W2, i: usize) -> (u128, u128) {
    // close position via trade vs a fresh counterparty is not available; try convert + withdraw
    let _ = w.refresh(i);
    let conv = w.with(i, |m, a| m.convert_released_pnl_to_capital_not_atomic(a));
    let cap = w.accounts[i].capital.get();
    let wd = w.with(i, |m, a| m.withdraw_not_atomic(a, cap));
    println!("  winner {i}: pnl={} convert={:?} cap_before_wd={} withdraw={:?} legs={:?}", w.accounts[i].pnl.get(), conv, cap, wd, w.legs(i));
    (cap, conv.unwrap_or(0))
}

/// T1: after write-off, do winners hold unbacked claims, and does a winner's cash-out stay within backing?
#[test]
fn att_d1_conservation_after_writeoff() {
    let mut w = scenario(1, 0, 50_000_000).expect("latch");
    cons(&w, "pre");
    println!("loser pnl={} winner pnl={} cap={}", w.accounts[0].pnl.get(), w.accounts[2].pnl.get(), w.accounts[2].capital.get());
    let _ = close(&mut w, 0); let r = close(&mut w, 1);
    println!("final close {:?} acct0 pnl={}", r.as_ref().map(|o| o.closed_q).map_err(|e| format!("{e:?}")), w.accounts[0].pnl.get());
    cons(&w, "post-writeoff");
    let w2_pnl = w.accounts[2].pnl.get();
    let vault_before = w.header.vault.get();
    println!("winner2 pnl={} capital={} legs={:?} src_claims", w2_pnl, w.accounts[2].capital.get(), w.legs(2));
    let cap0 = w.accounts[2].capital.get();
    let _ = w.refresh(2);
    let (cap, conv) = winner_cash_out(&mut w, 2);
    cons(&w, "post-cashout");
    println!("winner capital before {cap0}, converted {conv}, vault {vault_before} -> {}", w.header.vault.get());
    // winner's total take must not exceed what the loser put in: loser deposit 260_000
    assert!(w.header.vault.get() >= w.header.c_tot.get() + w.header.insurance.get());
}

/// T2: insurance in the loss domain is 100M; the deficit is attributable (a1-only crash). Who pays?
#[test]
fn att_d2_insurance_never_used_money() {
    for depth in [0usize, 10, 30] {
        let mut w = scenario_d(1, 100_000_000, 50_000_000, 260_000, depth).expect("latch");
        let ins0 = w.header.insurance.get();
        let deficit = -w.accounts[0].pnl.get();
        let _ = close(&mut w, 0); let r = close(&mut w, 1);
        let used = r.as_ref().map(|o| o.insurance_used).unwrap_or(0);
        let _ = w.refresh(2);
        let face = w.accounts[2].pnl.get();
        let (cap, conv) = winner_cash_out(&mut w, 2);
        cons(&w, "t2");
        println!("T2 depth={depth} deficit={deficit} insurance {ins0}->{} used={used} winner pnl_face_before_cashout={face} converted={conv} winner_shortfall~{}", w.header.insurance.get(), deficit);
        // b-prime-2: the deficit is attributable, so the domain's insurance pays it (d wrote it off).
        assert_eq!(ins0 - w.header.insurance.get(), deficit as u128, "attributable loss charged to insurance");
    }
}

/// T3: dormant winner keeps the group hlock armed after the write-off (latch reappearance).
#[test]
fn att_d3_dormant_winner_keeps_hlock() {
    let mut w = scenario(1, 0, 50_000_000).expect("latch");
    // winner realises its claim into pnl (any touch does this) BEFORE the last close
    let _ = w.refresh(2);
    println!("winner pnl before close = {} pnl_pos_tot={}", w.accounts[2].pnl.get(), w.header.pnl_pos_tot.get());
    let _ = close(&mut w, 0); let _ = close(&mut w, 1);
    cons(&w, "after-writeoff");
    // run permissionless cranks on every account + try_clear
    for _ in 0..4 {
        for i in 0..3 {
            let slot = w.header.current_slot.get();
            let _ = w.with(i, |m, a| m.permissionless_auto_crank_not_atomic(a, AutoCrankWorkV16 { now_slot: slot, observations: &[], resolved_close_fee_rate_per_slot: 0 }));
        }
        let _ = w.market(|m| m.try_clear_bankruptcy_hlock_not_atomic());
    }
    cons(&w, "after-cranks");
    println!("hlock after all permissionless steps = {} (dormant winner never converts)", hl(&w));
    // b-prime-2: the deficit is resolved and nothing new was marked; the dormant winner's claim is the
    // pre-existing claim term. Once it converts (flat, owner action) the byte clears.
    assert_eq!(w.header.negative_pnl_account_count.get(), 0);
    for _ in 0..3 {
        w.now += 1;
        for i in 1..3usize {
            let _ = w.refresh(i);
            let _ = w.with(i, |m, a| m.convert_released_pnl_to_capital_not_atomic(a));
        }
    }
    let _ = w.market(|m| m.try_clear_bankruptcy_hlock_not_atomic());
    assert_eq!(hl(&w), 0, "latch persists after the dormant winner converted");
}

/// T4: two co-winners; cash-out order must not change payouts (req 27) and sum <= backing.
#[test]
fn att_d4_cowinner_order() {
    let mut res = vec![];
    for order in [[2usize, 3], [3, 2]] {
        let mut w = scenario_d(2, 0, 50_000_000, 260_000, 20).expect("latch");
        let d = -w.accounts[0].pnl.get();
        let _ = close(&mut w, 0); let _ = close(&mut w, 1);
        let _ = w.refresh(2); let _ = w.refresh(3);
        let mut out = vec![];
        for &i in &order { let pre = w.header.vault.get(); winner_cash_out(&mut w, i); out.push((i, pre - w.header.vault.get())); }
        cons(&w, "t4");
        out.sort();
        println!("T4 deficit={d} order {:?} withdrawn {:?}", order, out);
        res.push(out);
    }
    assert_eq!(res[0], res[1], "payout depends on cash-out order");
}

/// T5: busy market. An UNRELATED healthy winner on asset 0 (accounts 3 long / 4 short) holds positive pnl.
/// After the write-off (which arms the UNATTRIBUTED hlock) and the real loss-domain winner cashing out,
/// does the group hlock ever clear without that unrelated winner acting?
#[test]
fn att_d5_unrelated_winner_keeps_group_hlock() {
    let q = POS_SCALE;
    let mut w = W2::new(band_cfg(2), &[260_000, 50_000_000, 50_000_000, 50_000_000, 50_000_000], 2);
    w.trade(0, 0, 1, q).unwrap();
    w.trade(1, 0, 2, q).unwrap();
    w.refresh(0).unwrap();
    for _ in 0..3 { w.now += 1; w.step_price(1, P0 + P0 / 20); w.step_price(0, P0); w.refresh(0).unwrap(); }
    // unrelated pair on a0 (accounts 3 long, 4 short)
    w.trade(0, 3, 4, q).unwrap();
    let mut hit = false;
    for _ in 0..80 {
        w.now += 1; w.step_price(0, P0 + P0 / 50); w.step_price(1, 1);
        let _ = w.refresh(0);
        if w.accounts[0].pnl.get() < 0 && w.accounts[0].liquidation_lock != 0 { hit = true; break; }
    }
    if !hit || w.accounts[0].capital.get() != 0 { panic!("no latch pnl={} cap={}", w.accounts[0].pnl.get(), w.accounts[0].capital.get()); }
    let _ = w.refresh(3);
    println!("unrelated winner 3 pnl={} (a0 domain)", w.accounts[3].pnl.get());
    let hl_pre = hl(&w);
    let _ = close(&mut w, 0); let _ = close(&mut w, 1);
    println!("loser pnl after closes {} neg_cnt {}", w.accounts[0].pnl.get(), w.header.negative_pnl_account_count.get());
    cons(&w, "post-writeoff");
    winner_cash_out(&mut w, 2);
    for _ in 0..3 { let _ = w.market(|m| m.try_clear_bankruptcy_hlock_not_atomic()); }
    cons(&w, "after loss-domain winner cashed out");
    // b-prime-2: the deficit is gone and the byte is no wider than before the closes. An unrelated
    // winner with an open leg keeps the pre-existing global claim term armed (not write-off specific).
    assert_eq!(w.header.negative_pnl_account_count.get(), 0);
    assert_eq!(w.accounts[0].pnl.get(), 0);
    assert!(hl(&w) == 0 || hl(&w) == hl_pre, "write-off changed the h-lock byte: {} -> {}", hl_pre, hl(&w));
}

/// T6: steering. Same economic deficit on asset 1, same 100M domain insurance. Loser with ONLY the a1 leg
/// (no cross-margin lock) vs loser that also holds an a0 leg (locked). Who pays?
#[test]
fn att_d6_shape_steers_payer() {
    for two_legs in [false, true] {
        let q = POS_SCALE;
        let mut w = W2::new(band_cfg(2), &[260_000, 50_000_000, 50_000_000], 2);
        if two_legs { w.trade(0, 0, 1, q).unwrap(); }
        w.trade(1, 0, 2, q).unwrap();
        w.refresh(0).unwrap();
        for _ in 0..3 { w.now += 1; w.step_price(1, P0 + P0 / 20); w.step_price(0, P0); w.refresh(0).unwrap(); }
        w.market(|m| m.deposit_domain_insurance_not_atomic(DOM_A1_SHORT, 100_000_000)).unwrap();
        for _ in 0..40 { w.now += 1; w.step_price(0, P0); w.step_price(1, 1); let _ = w.refresh(0);
            if w.accounts[0].pnl.get() < 0 && w.accounts[0].capital.get() == 0 { break; } }
        let pnl0 = w.accounts[0].pnl.get();
        let ins0 = w.header.insurance.get();
        let mut used = 0u128;
        for ai in [0usize, 1] { if let Ok(o) = close(&mut w, ai) { used += o.insurance_used; } }
        println!("T6 two_legs={two_legs} loser pnl before close={pnl0} lock={} insurance {ins0}->{} used={used} pnl after={} mode={}", w.accounts[0].liquidation_lock, w.header.insurance.get(), w.accounts[0].pnl.get(), w.header.mode);
    }
}

/// T7: double write-off / reuse: after the write-off the same account re-deposits, re-trades, goes bankrupt again.
#[test]
fn att_d7_reuse_after_writeoff_counters_stay_consistent() {
    let mut w = scenario(1, 0, 50_000_000).expect("latch");
    let _ = close(&mut w, 0); let _ = close(&mut w, 1);
    // second write-off attempts must be no-ops / errors
    let r = close(&mut w, 1);
    println!("second close: {:?}", r.map(|o| o.closed_q).map_err(|e| format!("{e:?}")));
    let _ = w.refresh(0);
    assert_eq!(w.header.negative_pnl_account_count.get(), 0);
    // reuse
    w.with(0, |m, a| m.deposit_not_atomic(a, 300_000)).unwrap();
    println!("reuse: pnl={} cap={} lock={} neg_cnt={}", w.accounts[0].pnl.get(), w.accounts[0].capital.get(), w.accounts[0].liquidation_lock, w.header.negative_pnl_account_count.get());
    cons(&w, "reuse");
    assert_eq!(w.accounts[0].pnl.get(), 0);
    assert_eq!(w.accounts[0].capital.get(), 300_000, "deposit must not be eaten by the written-off deficit");
}
