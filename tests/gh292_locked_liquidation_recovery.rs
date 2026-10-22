//! GH#292: the cross-margin `liquidation_lock` branch of `liquidate_account_not_atomic`.
//! Can an account be whittled to ZERO legs while still carrying a deficit, so that
//! `negative_pnl_account_count` never returns to 0 and the market-wide bankruptcy
//! h-lock can never be cleared?
//!
//! Reproduction fixture and the first test are by @0x-SquidSol (engine PR #293, issue #292).
//! #293 escalated that state to group-wide Recovery. This file instead pins LIVE RESOLUTION:
//! the close that leaves a locked account flat resolves its deficit in the live market
//! (principal -> domain insurance -> B-booking onto the loss-bearing side), and declares
//! Recovery only when that cannot cover it in one step.

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



/// Insurance domain charged when the bankrupt LONG on asset 1 is liquidated flat:
/// (asset 1, opposite side = Short) = 2 * 1 + 1.
const DOMAIN_A1_SHORT: usize = 3;

struct Latched {
    w: W2,
    /// The deficit the final close must resolve (`-pnl`, capital is 0).
    deficit: u128,
}

/// Squid's fixture: account 0 (cross-margin long a0+a1) is driven into bad debt with the
/// unattributed-loss lock set, then its a0 leg is liquidated reduce-only. One leg (a1) remains,
/// so the NEXT liquidation (a1) is the close that leaves it flat.
fn latched(chunk: Option<u128>, insure_a1_short: u128) -> Latched {
    let q = POS_SCALE;
    let mut cfg = band_cfg(2);
    if let Some(c) = chunk {
        cfg.public_b_chunk_atoms = c;
    }
    let mut w = W2::new(cfg, &[260_000, 50_000_000, 50_000_000], 2);
    w.trade(0, 0, 1, q).expect("open a0");
    w.trade(1, 0, 2, q).expect("open a1");
    w.refresh(0).unwrap();
    let mut latched = false;
    for _ in 0..24 {
        w.now += 1;
        w.step_price(0, 1);
        w.step_price(1, 1);
        let _ = w.refresh(0);
        if w.accounts[0].pnl.get() < 0 && w.accounts[0].liquidation_lock != 0 {
            latched = true;
            break;
        }
    }
    assert!(latched, "precondition: bad debt with the lock set");
    assert_eq!(w.accounts[0].capital.get(), 0, "precondition: capital exhausted");
    if insure_a1_short != 0 {
        w.market(|m| m.deposit_domain_insurance_not_atomic(DOMAIN_A1_SHORT, insure_a1_short))
            .unwrap();
    }
    let first = w
        .with(0, |m, a| {
            m.liquidate_account_not_atomic(a, LiquidationRequestV16 { asset_index: 0 })
        })
        .expect("a locked account with open risk remaining still liquidates, reduce-only");
    assert_eq!(first.insurance_used, 0, "reduce-only close consumes no insurance");
    assert_eq!(first.residual_booked, 0, "reduce-only close books no residual");
    assert_eq!(first.fee_charged, 0, "reduce-only close charges no fee");
    assert_eq!(w.header.mode, 0, "no recovery while open risk remains");
    assert_ne!(w.accounts[0].liquidation_lock, 0, "still locked with a leg open");
    assert_eq!(w.legs(0).iter().filter(|(_, act)| *act).count(), 1, "one leg left");
    let deficit = w.accounts[0].pnl.get().unsigned_abs();
    assert!(deficit > 0 && w.accounts[0].capital.get() == 0);
    Latched { w, deficit }
}

fn final_close(l: &mut Latched) -> Result<LiquidationOutcomeV16, V16Error> {
    l.w.with(0, |m, a| {
        m.liquidate_account_not_atomic(a, LiquidationRequestV16 { asset_index: 1 })
    })
}

struct Snap {
    vault: u128,
    insurance: u128,
    c_tot: u128,
    neg: u32,
    b_long: u128,
    b_short: u128,
    a0_b_short: u128,
    a0_b_long: u128,
    pnl_pos_tot: u128,
}

/// Total B charged on a side across epochs. A full close can start a reset (epoch start takes
/// the live index), so the booking is visible in `b_epoch_start` as well as `b_num`.
fn b_total(w: &W2, ai: usize) -> (u128, u128) {
    let a = w.asset(ai);
    (a.b_long_num + a.b_epoch_start_long_num, a.b_short_num + a.b_epoch_start_short_num)
}

fn snap(w: &W2) -> Snap {
    Snap {
        vault: w.header.vault.get(),
        insurance: w.header.insurance.get(),
        c_tot: w.header.c_tot.get(),
        neg: w.header.negative_pnl_account_count.get() as u32,
        b_long: b_total(w, 1).0,
        b_short: b_total(w, 1).1,
        a0_b_short: b_total(w, 0).1,
        a0_b_long: b_total(w, 0).0,
        pnl_pos_tot: w.header.pnl_pos_tot.get(),
    }
}

fn assert_resolved_cleanly(l: &Latched, before: &Snap, out: &LiquidationOutcomeV16) {
    let w = &l.w;
    assert_eq!(w.accounts[0].pnl.get(), 0, "deficit fully resolved in place");
    assert_eq!(w.accounts[0].liquidation_lock, 0, "lock clears with the deficit");
    assert_eq!(w.header.negative_pnl_account_count.get(), 0, "counter released");
    assert_eq!(before.neg, 1, "precondition: counter was armed");
    assert_eq!(w.header.mode, 0, "market stays Live");
    assert!(w.header.recovery_reason.try_to_runtime().unwrap().is_none());
    assert!(w.legs(0).iter().all(|(_, act)| !*act), "account is flat");
    assert_eq!(out.explicit_loss, 0, "Live never writes a loss off unbacked");
    // Conservation: no token moved; insurance fell by exactly what was used; deficit partition.
    assert_eq!(w.header.vault.get(), before.vault, "vault unchanged");
    assert_eq!(
        w.header.insurance.get() + out.insurance_used,
        before.insurance + out.fee_charged,
        "insurance moves only by used insurance (and any fee credited)"
    );
    assert!(w.header.vault.get() >= w.header.c_tot.get() + w.header.insurance.get());
    assert_eq!(
        out.insurance_used + out.residual_booked + (before.c_tot - w.header.c_tot.get()),
        l.deficit,
        "deficit = principal paid + insurance + booked residual (no remainder lost)"
    );
    // Other-domain isolation: asset 0 B indices untouched.
    assert_eq!(b_total(w, 0), (before.a0_b_long, before.a0_b_short));
    // Winners are not haircut through anything but the charged domain's B index; the long side
    // of the charged asset (the bankrupt's own side) is untouched.
    assert_eq!(b_total(w, 1).0, before.b_long);
    let _ = before.pnl_pos_tot;
}

/// Squid's reproduction (GH#292), adapted: the close that leaves the locked account flat now
/// RESOLVES the deficit in the live market instead of stranding it (base) or winding the group
/// down (#293). FAILS ON BASE e59aa6a6 and on #293.
#[test]
fn gh292_locked_flat_close_resolves_the_deficit_in_the_live_market() {
    let mut l = latched(None, 0);
    let before = snap(&l.w);
    assert_eq!(before.neg, 1, "precondition: the counter is armed");
    let out = final_close(&mut l).expect("the flat close must resolve, not escalate or strand");
    assert!(out.closed_q > 0);
    assert_resolved_cleanly(&l, &before, &out);
    // No insurance anywhere: the whole deficit is B-booked onto the short side of a1.
    assert_eq!(out.insurance_used, 0);
    assert_eq!(out.residual_booked, l.deficit);
    assert!(b_total(&l.w, 1).1 > before.b_short, "B index of the loss-bearing side moved");
}

/// Stage 1: domain insurance alone covers the deficit; nobody is haircut.
#[test]
fn gh292_stage_insurance_only_cover_books_nothing() {
    let probe = latched(None, 0).deficit;
    let mut l = latched(None, probe + 1_000);
    let before = snap(&l.w);
    let out = final_close(&mut l).expect("insurance covers it");
    assert_resolved_cleanly(&l, &before, &out);
    assert_eq!(out.insurance_used, l.deficit);
    assert_eq!(out.residual_booked, 0, "insurance is exhausted BEFORE any B-booking");
    assert_eq!(b_total(&l.w, 1).1, before.b_short, "no winner haircut at all");
    assert_eq!(l.w.header.insurance.get(), before.insurance - l.deficit);
}

/// Stage 2: insurance is exhausted first, the remainder is B-booked (vault-LP market: junior,
/// then Earn seniors, via the vault LP's reduced value).
#[test]
fn gh292_stage_insurance_then_b_booking_in_order() {
    let probe = latched(None, 0).deficit;
    let seed = probe / 3;
    assert!(seed > 0 && seed < probe);
    let mut l = latched(None, seed);
    let before = snap(&l.w);
    let out = final_close(&mut l).expect("insurance + B covers it");
    assert_resolved_cleanly(&l, &before, &out);
    assert_eq!(out.insurance_used, seed, "ALL available insurance is used first");
    assert_eq!(out.residual_booked, l.deficit - seed, "only the remainder is booked");
    assert_eq!(l.w.header.insurance.get(), before.insurance - seed);
    assert!(b_total(&l.w, 1).1 > before.b_short);
}

/// Winners of the OTHER asset are never touched, and the whole booking lands on the charged
/// domain's loss-weight holders (in a vault-LP market that is the vault LP). The single
/// opposing holder's weight is POS_SCALE (1e6) and SOCIAL_LOSS_DEN is 1e21, so a booking of `x`
/// atoms moves the B index by exactly `x * 1e15`.
#[test]
fn gh292_only_the_charged_domains_holder_bears_the_booking() {
    let mut l = latched(None, 0);
    l.w.refresh(1).unwrap();
    let other_before = (l.w.accounts[1].pnl.get(), l.w.accounts[1].capital.get());
    let b_before = b_total(&l.w, 1).1;
    let w_sum = l.w.asset(1).loss_weight_sum_short;
    assert_eq!(w_sum, POS_SCALE, "precondition: one opposing holder at weight POS_SCALE");
    let out = final_close(&mut l).expect("resolves");
    l.w.refresh(1).unwrap();
    assert_eq!((l.w.accounts[1].pnl.get(), l.w.accounts[1].capital.get()), other_before, "a0 winner untouched");
    assert_eq!(out.residual_booked, l.deficit, "precondition: the whole deficit was booked (non-vacuous)");
    let delta_b = b_total(&l.w, 1).1 - b_before;
    assert_eq!(delta_b, out.residual_booked * 1_000_000_000_000_000, "B index moved by exactly the booking");
    assert_eq!(b_total(&l.w, 0), (0, 0), "the other asset's domain is untouched");
}

/// Stage 3: nothing can cover it in one step -> Recovery (and only then). Forces the
/// single-step B capacity below the residual with a one-atom chunk.
#[test]
fn gh292_stage_uncoverable_declares_recovery() {
    let mut l = latched(Some(1), 0);
    assert!(l.deficit > 1);
    let out = final_close(&mut l);
    assert!(
        matches!(out, Err(V16Error::RecoveryRequired)),
        "an uncoverable deficit must escalate, got {:?}",
        out.as_ref().map(|o| o.closed_q).map_err(|e| format!("{e:?}"))
    );
    assert_eq!(l.w.header.mode, 2, "Recovery declared");
    assert!(l.w.header.recovery_reason.try_to_runtime().unwrap().is_some());
}

/// A deficit that insurance + the chunk can cover never reaches Recovery even when the chunk is
/// small: insurance shrinks the residual below the chunk (no new path to Recovery for dust).
#[test]
fn gh292_small_residual_after_insurance_does_not_escalate() {
    let probe = latched(None, 0).deficit;
    let chunk = 5u128;
    let seed = probe - chunk; // leaves exactly `chunk` for B
    let mut l = latched(Some(chunk), seed);
    let before = snap(&l.w);
    let out = final_close(&mut l).expect("residual == chunk is bookable in one step");
    assert_resolved_cleanly(&l, &before, &out);
    assert_eq!(out.insurance_used, seed);
    assert_eq!(out.residual_booked, chunk);
}

/// The open-risk half is unchanged: locked with legs remaining after the close stays reduce-only.
/// (Exercised inside `latched`; here the invariant is checked on its own with a negative control
/// that the snapshot did not move.)
#[test]
fn gh292_locked_close_with_open_risk_remaining_charges_nothing() {
    let q = POS_SCALE;
    let mut w = W2::new(band_cfg(2), &[260_000, 50_000_000, 50_000_000], 2);
    w.trade(0, 0, 1, q).unwrap();
    w.trade(1, 0, 2, q).unwrap();
    w.refresh(0).unwrap();
    for _ in 0..24 {
        w.now += 1;
        w.step_price(0, 1);
        w.step_price(1, 1);
        let _ = w.refresh(0);
        if w.accounts[0].pnl.get() < 0 && w.accounts[0].liquidation_lock != 0 {
            break;
        }
    }
    assert!(w.accounts[0].liquidation_lock != 0);
    let ins = w.header.insurance.get();
    let b = (b_total(&w, 0).1, b_total(&w, 1).1);
    let pnl = w.accounts[0].pnl.get();
    let out = w
        .with(0, |m, a| m.liquidate_account_not_atomic(a, LiquidationRequestV16 { asset_index: 0 }))
        .unwrap();
    assert_eq!((out.insurance_used, out.residual_booked, out.fee_charged), (0, 0, 0));
    assert_eq!(w.header.insurance.get(), ins);
    assert_eq!(b, (b_total(&w, 0).1, b_total(&w, 1).1));
    assert_eq!(w.accounts[0].pnl.get(), pnl, "deficit untouched while risk remains");
    assert_ne!(w.accounts[0].liquidation_lock, 0);
}

/// Negative control: the single-asset (unlocked) path is unchanged. Passes before and after.
#[test]
fn gh292_single_asset_bankruptcy_still_resolves_in_place() {
    let q = POS_SCALE;
    let mut w = W2::new(band_cfg(1), &[110_000, 50_000_000], 1);
    w.trade(0, 0, 1, q).expect("open a0");
    w.refresh(0).unwrap();
    let mut liquidatable = false;
    for _ in 0..40 {
        w.now += 1;
        w.step_price(0, 1);
        if let Ok(c) = w.refresh(0) {
            if c.certified_liq_deficit != 0 {
                liquidatable = true;
                break;
            }
        }
    }
    assert!(liquidatable, "precondition: the control account must breach maintenance");
    assert_eq!(w.accounts[0].liquidation_lock, 0, "a single-leg account never takes the locked branch");
    let out = w.with(0, |m, a| {
        m.liquidate_account_not_atomic(a, LiquidationRequestV16 { asset_index: 0 })
    });
    assert!(out.is_ok(), "got {:?}", out.as_ref().map(|o| o.closed_q).map_err(|e| format!("{e:?}")));
}

// ---------------------------------------------------------------------------------------------
// Randomized invariant sim: many (deposit, insurance seed, chunk) states, every invariant above.
// ---------------------------------------------------------------------------------------------
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, .. ProptestConfig::default() })]

    #[test]
    fn gh292_sim_locked_flat_close_invariants(
        deposit in 200_000u128..330_000,
        seed_pct in 0u32..=150,          // insurance seed as % of the deficit
        chunk_sel in 0u8..4,             // 0 = unbounded, else a small chunk
    ) {
        // Probe the deficit for this deposit first (same deterministic path).
        let probe = latched_with_deposit(deposit, None, 0);
        let Some(probe) = probe else { return Ok(()); };
        let deficit = probe.deficit;
        let seed = deficit * seed_pct as u128 / 100;
        let chunk = match chunk_sel { 0 => None, 1 => Some(1u128), 2 => Some(deficit / 2 + 1), _ => Some(deficit) };
        let Some(mut l) = latched_with_deposit(deposit, chunk, seed) else { return Ok(()); };
        prop_assert_eq!(l.deficit, deficit);
        let before = snap(&l.w);
        let after_ins = deficit - seed.min(deficit);
        let chunk_cap = chunk.unwrap_or(u128::MAX);
        let must_recover = after_ins > chunk_cap;
        let out = final_close(&mut l);
        match out {
            Ok(o) => {
                prop_assert!(!must_recover, "covered state must not need Recovery");
                prop_assert_eq!(l.w.accounts[0].pnl.get(), 0);
                prop_assert_eq!(l.w.accounts[0].liquidation_lock, 0);
                prop_assert_eq!(l.w.header.negative_pnl_account_count.get(), 0);
                prop_assert_eq!(l.w.header.mode, 0);
                prop_assert_eq!(l.w.header.vault.get(), before.vault);
                prop_assert_eq!(o.insurance_used, seed.min(deficit));
                prop_assert_eq!(o.residual_booked, after_ins);
                prop_assert_eq!(o.explicit_loss, 0);
                // ordering: booking only after insurance is exhausted
                prop_assert!(o.residual_booked == 0 || o.insurance_used == seed.min(deficit));
                prop_assert_eq!(l.w.header.insurance.get() + o.insurance_used, before.insurance + o.fee_charged);
                prop_assert!(l.w.header.vault.get() >= l.w.header.c_tot.get() + l.w.header.insurance.get());
                prop_assert_eq!(b_total(&l.w, 0), (before.a0_b_long, before.a0_b_short));
            }
            Err(V16Error::RecoveryRequired) => {
                prop_assert!(must_recover, "Recovery only when single-step capacity cannot cover");
                prop_assert_eq!(l.w.header.mode, 2);
            }
            Err(e) => prop_assert!(false, "unexpected error {:?}", e),
        }
    }
}

fn latched_with_deposit(deposit: u128, chunk: Option<u128>, insure: u128) -> Option<Latched> {
    std::panic::catch_unwind(|| {
        let q = POS_SCALE;
        let mut cfg = band_cfg(2);
        if let Some(c) = chunk {
            cfg.public_b_chunk_atoms = c;
        }
        let mut w = W2::new(cfg, &[deposit, 50_000_000, 50_000_000], 2);
        w.trade(0, 0, 1, q).ok()?;
        w.trade(1, 0, 2, q).ok()?;
        w.refresh(0).ok()?;
        let mut hit = false;
        for _ in 0..24 {
            w.now += 1;
            w.step_price(0, 1);
            w.step_price(1, 1);
            let _ = w.refresh(0);
            if w.accounts[0].pnl.get() < 0 && w.accounts[0].liquidation_lock != 0 {
                hit = true;
                break;
            }
        }
        if !hit || w.accounts[0].capital.get() != 0 {
            return None;
        }
        if insure != 0 {
            w.market(|m| m.deposit_domain_insurance_not_atomic(DOMAIN_A1_SHORT, insure)).ok()?;
        }
        w.with(0, |m, a| m.liquidate_account_not_atomic(a, LiquidationRequestV16 { asset_index: 0 })).ok()?;
        if w.accounts[0].liquidation_lock == 0 {
            return None;
        }
        let deficit = w.accounts[0].pnl.get().unsigned_abs();
        if deficit == 0 {
            return None;
        }
        Some(Latched { w, deficit })
    })
    .ok()
    .flatten()
}



/// Non-vacuity guard for the sim: a deterministic sweep over the same state space must actually
/// reach the latched state many times AND exercise every outcome (resolved with insurance only,
/// insurance + B, B only, and Recovery), asserting the invariants on each.
#[test]
fn gh292_sim_sweep_covers_every_waterfall_outcome() {
    let (mut ran, mut ins_only, mut ins_b, mut b_only, mut recovery) = (0u32, 0u32, 0u32, 0u32, 0u32);
    for deposit in (200_000u128..=330_000).step_by(10_000) {
        let Some(probe) = latched_with_deposit(deposit, None, 0) else { continue };
        let deficit = probe.deficit;
        for seed_pct in [0u32, 25, 50, 100, 150] {
            for chunk_sel in 0u8..4 {
                let seed = deficit * seed_pct as u128 / 100;
                let chunk = match chunk_sel { 0 => None, 1 => Some(1u128), 2 => Some(deficit / 2 + 1), _ => Some(deficit) };
                let Some(mut l) = latched_with_deposit(deposit, chunk, seed) else { continue };
                assert_eq!(l.deficit, deficit);
                ran += 1;
                let before = snap(&l.w);
                let used = seed.min(deficit);
                let must_recover = deficit - used > chunk.unwrap_or(u128::MAX);
                match final_close(&mut l) {
                    Ok(o) => {
                        assert!(!must_recover);
                        assert_resolved_cleanly(&l, &before, &o);
                        assert_eq!((o.insurance_used, o.residual_booked), (used, deficit - used));
                        match (o.insurance_used > 0, o.residual_booked > 0) {
                            (true, false) => ins_only += 1,
                            (true, true) => ins_b += 1,
                            (false, true) => b_only += 1,
                            (false, false) => unreachable!("deficit > 0"),
                        }
                    }
                    Err(V16Error::RecoveryRequired) => {
                        assert!(must_recover);
                        assert_eq!(l.w.header.mode, 2);
                        recovery += 1;
                    }
                    Err(e) => panic!("unexpected {e:?}"),
                }
            }
        }
    }
    println!("sweep ran={ran} ins_only={ins_only} ins_b={ins_b} b_only={b_only} recovery={recovery}");
    assert!(ran >= 40 && ins_only > 0 && ins_b > 0 && b_only > 0 && recovery > 0, "sim must be non-vacuous");
}
