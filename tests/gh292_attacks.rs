
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

#[test]
fn atk_control_a1_only_loss_is_attributed_and_resolved() {
    let mut w = crash_a1_only(false, &[(DOMAIN_A1_SHORT, 100_000_000)]).expect("latch");
    let r0 = close(&mut w, 0);
    let r1 = close(&mut w, 1);
    println!("control: a0 close {:?}, a1 close {:?}, mode {}", r0.as_ref().map(|o| o.insurance_used).map_err(|e| format!("{e:?}")), r1.as_ref().map(|o| o.insurance_used).map_err(|e| format!("{e:?}")), w.header.mode);
    assert_eq!(w.header.mode, 0, "control must stay Live");
    assert_eq!(w.accounts[0].pnl.get(), 0);
}

/// Owner-steerable Recovery: a small a0 dip realized while account holds a1 gains makes the later
/// a1-only crash unattributable. Expect (by req 5) attributed to a1 and resolved live.
#[test]
fn atk_stale_a0_dip_forces_recovery_on_a1_only_deficit() {
    let mut w = crash_a1_only(true, &[(DOMAIN_A1_SHORT, 100_000_000)]).expect("latch");
    let _ = close(&mut w, 0);
    let r = close(&mut w, 1);
    println!("dip: final close {:?}, mode {}", r.as_ref().map(|o| o.insurance_used).map_err(|e| format!("{e:?}")), w.header.mode);
    assert_eq!(w.header.mode, 0, "a1-only deficit should not push the group into Recovery");
}

#[test]
fn atk_double_liquidation_does_not_recharge() {
    let mut w = crash_a1_only(false, &[(DOMAIN_A1_SHORT, 100_000_000)]).expect("latch");
    let _ = close(&mut w, 0);
    let _ = close(&mut w, 1);
    let ins = w.header.insurance.get();
    let (v, c) = (w.header.vault.get(), w.header.c_tot.get());
    let b = (w.asset(0).b_short_num, w.asset(1).b_short_num);
    for ai in [0, 1] {
        let r = close(&mut w, ai);
        assert!(r.is_err(), "second liquidation of resolved account must fail, got ok");
    }
    assert_eq!((ins, v, c, b), (w.header.insurance.get(), w.header.vault.get(), w.header.c_tot.get(), (w.asset(0).b_short_num, w.asset(1).b_short_num)));
    assert!(w.header.vault.get() >= w.header.c_tot.get() + w.header.insurance.get());
}

fn snapshot(w: &W2) -> (u128, u128, u128, u128, u128, u128) {
    (w.header.vault.get(), w.header.c_tot.get(), w.header.insurance.get(),
     w.asset(0).b_short_num, w.asset(1).b_short_num, w.header.mode as u128)
}

/// Req 27: close order must not change who pays.
#[test]
fn atk_close_order_changes_payer() {
    let mut a = crash_a1_only(false, &[(DOMAIN_A1_SHORT, 100_000_000)]).expect("latch");
    let mut b = crash_a1_only(false, &[(DOMAIN_A1_SHORT, 100_000_000)]).expect("latch");
    let ra = (close(&mut a, 0).map(|o| o.insurance_used), close(&mut a, 1).map(|o| o.insurance_used));
    let rb = (close(&mut b, 1).map(|o| o.insurance_used), close(&mut b, 0).map(|o| o.insurance_used));
    println!("order0-1 {:?} {:?}\norder1-0 {:?} {:?}", ra, snapshot(&a), rb, snapshot(&b));
    assert_eq!(snapshot(&a), snapshot(&b));
}

/// Free option: after the charge at a1's full close, the a0 leg survives with pnl 0 / capital 0.
#[test]
fn atk_surviving_leg_after_charge_is_zero_equity() {
    let mut w = crash_a1_only(false, &[(DOMAIN_A1_SHORT, 100_000_000)]).expect("latch");
    let r = close(&mut w, 1);
    println!("close a1 first: {:?}", r.as_ref().map(|o| o.insurance_used));
    let acct = &w.accounts[0];
    println!("after: pnl={} cap={} lock={} legs={:?}", acct.pnl.get(), acct.capital.get(), acct.liquidation_lock, w.legs(0));
    // price of a0 moves up one step: account (zero capital) keeps the upside?
    for _ in 0..3 {
        w.now += 1;
        w.step_price(0, P0 + P0 / 50);
        let _ = w.refresh(0);
    }
    println!("a0 up: pnl={} cap={}", w.accounts[0].pnl.get(), w.accounts[0].capital.get());
    // and down: who pays?
    let before = snapshot(&w);
    for _ in 0..40 {
        w.now += 1;
        w.step_price(0, 1);
        let _ = w.refresh(0);
    }
    let r = close(&mut w, 0);
    println!("a0 crash then close: {:?} pnl={} before={:?} after={:?}", r.as_ref().map(|o| o.insurance_used).map_err(|e| format!("{e:?}")), w.accounts[0].pnl.get(), before, snapshot(&w));
}

/// Pooled fallback: dip variant with ample insurance in EVERY domain: still Recovery?
#[test]
fn atk_unattributable_with_all_domains_insured_goes_recovery_not_pooled() {
    let ins: Vec<(usize, u128)> = (0..4).map(|d| (d, 100_000_000)).collect();
    let mut w = crash_a1_only(true, &ins).expect("latch");
    let _ = close(&mut w, 0);
    let r = close(&mut w, 1);
    println!("all-insured unattributable: {:?} mode {} ins={}", r.as_ref().map(|o| o.insurance_used).map_err(|e| format!("{e:?}")), w.header.mode, w.header.insurance.get());
    assert_eq!(w.header.mode, 0);
}

/// 3-leg account, every asset crashes: how does a keeper's per-leg liquidation sequence end?
#[test]
fn atk_three_leg_all_crash_liquidation_sequence() {
    let q = POS_SCALE;
    let n = 3usize;
    let mut w = W2::new(band_cfg(n as u16), &[390_000, 50_000_000, 50_000_000, 50_000_000], n);
    for i in 0..n { let r = w.trade(i, 0, i + 1, q); println!("trade {i}: {r:?}"); r.unwrap(); }
    w.now += 5; let _ = w.refresh(0);
    for _ in 0..120 {
        w.now += 1;
        for i in 0..n { w.step_price(i, 1); }
        let _ = w.refresh(0);
        if w.accounts[0].pnl.get() < 0 && w.accounts[0].capital.get() == 0 { break; }
    }
    println!("state: pnl={} cap={} lock={}", w.accounts[0].pnl.get(), w.accounts[0].capital.get(), w.accounts[0].liquidation_lock);
    for round in 0..8 {
        for ai in 0..n {
            let r = close(&mut w, ai);
            println!("round {round} close a{ai}: {:?} mode={} legs={:?} pnl={}", r.as_ref().map(|o| (o.closed_q, o.insurance_used)).map_err(|e| format!("{e:?}")), w.header.mode, w.legs(0), w.accounts[0].pnl.get());
        }
    }
}
