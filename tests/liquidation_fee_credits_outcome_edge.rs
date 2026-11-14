//! Liquidation outcomes with a NON-ZERO liquidation fee, and what happens to `fee_credits`.
//!
//! Follow-up to #352 (capital + pnl only). Here we read the `LiquidationOutcomeV16` itself and
//! check the whole ledger: where the fee goes, that it never exceeds what the account has left,
//! that `fee_credits` never moves or goes negative, partial vs full close fees, conservation of
//! value across vault / insurance / accounts, and that cranking again does not charge twice.
//!
//! Config is the valid non-zero-fee one found in hedged_refresh_fee_loss.rs (#349): MM 5%,
//! `min_nonzero_mm_req = 2`, `min_nonzero_im_req = 3`, fee at most 243 bps.
//!
//! Setup: account `a` long 100 units of asset 0 at 1_000 with `DEPOSIT` capital; counterparty `b`
//! (huge capital) is short. The price falls by the 1%/slot cap until `a` is under maintenance.
//! Test names start with `liq_fee_credits_`.
#![allow(dead_code)]

use percolator::{
    EngineAssetSlotV16Account, LiquidationOutcomeV16, LiquidationRequestV16, Market,
    MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, PermissionlessCrankActionV16,
    PermissionlessCrankRequestV16, PortfolioAccountV16Account, PortfolioV16ViewMut,
    ProvenanceHeaderV16, ProvenanceHeaderV16Account, TradeRequestV16, V16Config, V16Error,
    V16PodU64, POS_SCALE,
};

const P0: u64 = 1_000;
const Q: u128 = 100 * POS_SCALE;
const BIG: u128 = 1_000_000_000;
const DEPOSIT: u128 = 20_000;

fn config(fee_bps: u64, cap: u128, min_abs: u128) -> V16Config {
    let mut cfg = V16Config::public_user_fund_with_market_slots(2, 2, 0, 10);
    cfg.maintenance_margin_bps = 500;
    cfg.initial_margin_bps = 1_000;
    cfg.max_price_move_bps_per_slot = 100;
    cfg.liquidation_fee_bps = fee_bps;
    cfg.liquidation_fee_cap = cap;
    cfg.min_liquidation_abs = min_abs;
    cfg.min_nonzero_mm_req = 2;
    cfg.min_nonzero_im_req = 3;
    cfg
}

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
}

impl W {
    fn with(cfg: V16Config) -> Result<Self, V16Error> {
        let n = 2usize;
        let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, n as u32, 0)?;
        let mut markets: Vec<Market<u64>> =
            (0..n as u64).map(|i| Market::new(i, EngineAssetSlotV16Account::default())).collect();
        for i in 0..n {
            header
                .activate_empty_asset_slot_not_atomic(i as u32, &mut markets[i].engine, P0, 1 + 2 * i as u64)
                .unwrap();
        }
        Ok(W { header, markets, slot: 2 * n as u64 - 1 })
    }
    fn view(&mut self) -> MarketGroupV16ViewMut<'_, u64> {
        MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets)
    }
    fn price(&self) -> u64 {
        self.markets[0].engine.asset.effective_price.get()
    }
    fn deposit(&mut self, a: &mut PortfolioAccountV16Account, amt: u128) {
        self.view().deposit_not_atomic(&mut PortfolioV16ViewMut::new(a), amt).unwrap();
    }
    fn trade(&mut self, long: &mut PortfolioAccountV16Account, short: &mut PortfolioAccountV16Account, size: i128) {
        let p = self.price();
        self.view()
            .execute_trade_with_fee_loss_stale_scoped_not_atomic(
                &mut PortfolioV16ViewMut::new(long),
                &mut PortfolioV16ViewMut::new(short),
                TradeRequestV16 { asset_index: 0, size_q: size, exec_price: p, fee_bps: 0 },
                true,
            )
            .expect("trade");
    }
    fn tick(&mut self, new: u64) {
        self.slot += 1;
        let slot = self.slot;
        for j in 0..2 {
            let px = if j == 0 { new } else { self.markets[j].engine.asset.effective_price.get() };
            let mut m = self.view();
            m.accrue_asset_to_not_atomic(j, slot, px, 0, true).expect("accrue");
            m.markets[j].engine.asset.raw_oracle_target_price = V16PodU64::new(px);
        }
    }
    fn move_to(&mut self, target: u64) {
        while self.price() != target {
            let p = self.price();
            let s = (p * 100 / 10_000).max(1);
            self.tick(if target > p { (p + s).min(target) } else { p.saturating_sub(s).max(target) });
        }
    }
    fn refresh(&mut self, a: &mut PortfolioAccountV16Account) -> percolator::HealthCertV16 {
        let slot = self.slot;
        for _ in 0..64 {
            let r = self.view().full_account_refresh_not_atomic(&mut PortfolioV16ViewMut::new(a));
            match r {
                Err(V16Error::BStale) => {
                    for i in 0..2 {
                        let price = self.markets[i].engine.asset.effective_price.get();
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
                r => return r.expect("refresh"),
            }
        }
        panic!("refresh did not converge");
    }
    fn liquidate(&mut self, a: &mut PortfolioAccountV16Account) -> Result<LiquidationOutcomeV16, V16Error> {
        self.view()
            .liquidate_account_not_atomic(&mut PortfolioV16ViewMut::new(a), LiquidationRequestV16 { asset_index: 0 })
    }
    fn sound(&mut self, accts: &mut [&mut PortfolioAccountV16Account]) {
        let (v, c, i) = (self.header.vault.get(), self.header.c_tot.get(), self.header.insurance.get());
        assert!(v >= c + i, "conservation broken: V {v} < C_tot {c} + I {i}");
        let g = self.view();
        g.validate_shape().unwrap();
        for a in accts.iter_mut() {
            PortfolioV16ViewMut::new(a).validate_with_market(&g.as_view()).unwrap();
        }
    }
}

fn pos(a: &PortfolioAccountV16Account) -> i128 {
    a.legs
        .iter()
        .find(|l| l.active != 0 && l.asset_index.get() == 0)
        .map(|l| l.basis_pos_q.get())
        .unwrap_or(0)
}

/// Snapshot of every pool the fee could move through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Pools {
    vault: u128,
    c_tot: u128,
    insurance: u128,
    a_cap: u128,
    a_pnl: i128,
    a_fc: i128,
    b_cap: u128,
    b_pnl: i128,
    b_fc: i128,
}

fn pools(w: &W, a: &PortfolioAccountV16Account, b: &PortfolioAccountV16Account) -> Pools {
    Pools {
        vault: w.header.vault.get(),
        c_tot: w.header.c_tot.get(),
        insurance: w.header.insurance.get(),
        a_cap: a.capital.get(),
        a_pnl: a.pnl.get(),
        a_fc: a.fee_credits.get(),
        b_cap: b.capital.get(),
        b_pnl: b.pnl.get(),
        b_fc: b.fee_credits.get(),
    }
}

/// a long, b short, price moved to `px`, `a` refreshed (liquidatable or not).
fn setup(cfg: V16Config, deposit: u128, px: u64) -> (W, PortfolioAccountV16Account, PortfolioAccountV16Account) {
    let mut w = W::with(cfg).expect("config");
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, deposit);
    w.deposit(&mut b, BIG);
    w.trade(&mut a, &mut b, Q as i128);
    w.move_to(px);
    (w, a, b)
}

const FEES: [u64; 3] = [50, 100, 243];
/// 840..810: liquidatable with capital left. 800: capital exactly gone. 790, 700: insolvent.
const SOLVENT_PX: [u64; 6] = [840, 835, 830, 820, 815, 810];

fn expected_fee(closed_q: u128, px: u64, bps: u64, cap: u128, capital: u128) -> u128 {
    let notional = (closed_q * px as u128 + POS_SCALE - 1) / POS_SCALE;
    let raw = (notional * bps as u128 + 9_999) / 10_000;
    raw.min(cap).min(capital)
}

/// The fee goes to insurance and nowhere else: account capital falls by exactly `fee_charged`,
/// insurance rises by it, the vault does not move, the counterparty is untouched, and the
/// outcome reports no insurance use / residual / loss.
#[test]
fn liq_fee_credits_fee_goes_to_insurance_and_nowhere_else() {
    for fee in FEES {
        for px in SOLVENT_PX {
            let (mut w, mut a, mut b) = setup(config(fee, 1_000_000, 0), DEPOSIT, px);
            w.refresh(&mut a);
            let p0 = pools(&w, &a, &b);
            let o = w.liquidate(&mut a).expect("liquidate");
            let p1 = pools(&w, &a, &b);
            let ctx = format!("fee {fee} px {px} {o:?}");
            assert!(o.fee_charged > 0, "{ctx}");
            assert_eq!(p0.a_cap - p1.a_cap, o.fee_charged, "{ctx}: capital drop is the fee");
            assert_eq!(p1.insurance - p0.insurance, o.fee_charged, "{ctx}: insurance gain is the fee");
            assert_eq!(p0.c_tot - p1.c_tot, o.fee_charged, "{ctx}");
            assert_eq!(p0.vault, p1.vault, "{ctx}: vault unchanged");
            assert_eq!((p1.b_cap, p1.b_pnl), (p0.b_cap, p0.b_pnl), "{ctx}: counterparty untouched");
            assert_eq!((p1.a_pnl, p1.a_fc, p1.b_fc), (0, 0, 0), "{ctx}");
            assert_eq!((o.insurance_used, o.residual_booked, o.explicit_loss), (0, 0, 0), "{ctx}");
            w.sound(&mut [&mut a, &mut b]);
        }
    }
}

/// The charged fee equals ceil(ceil(closed_q * px / POS_SCALE) * bps / 10_000), limited by the
/// account's capital (and the cap). Includes the case where the fee swallows all of the capital.
#[test]
fn liq_fee_credits_fee_matches_formula_and_never_exceeds_capital() {
    for fee in FEES {
        for px in SOLVENT_PX {
            let (mut w, mut a, _b) = setup(config(fee, 1_000_000, 0), DEPOSIT, px);
            w.refresh(&mut a);
            let cap0 = a.capital.get();
            let o = w.liquidate(&mut a).unwrap();
            assert_eq!(o.fee_charged, expected_fee(o.closed_q, px, fee, 1_000_000, cap0), "fee {fee} px {px}");
            assert!(o.fee_charged <= cap0, "fee {fee} px {px}");
        }
    }
    // 243 bps at 810: the whole remaining 1_000 of capital goes to the fee on a full close
    let (mut w, mut a, _b) = setup(config(243, 1_000_000, 0), DEPOSIT, 810);
    w.refresh(&mut a);
    let o = w.liquidate(&mut a).unwrap();
    assert_eq!((o.closed_q, o.fee_charged, a.capital.get(), pos(&a)), (Q, 1_000, 0, 0));
}

/// Sweep every price from just under maintenance down past bankruptcy, three fees: the fee never
/// exceeds the capital the account had, capital never goes negative (u128) and `fee_credits`
/// stays at 0, never positive (minted) and never negative. Nothing in the engine writes
/// `fee_credits` on a liquidation, so the fee can only come out of capital.
#[test]
fn liq_fee_credits_sweep_fee_bounded_and_fee_credits_untouched() {
    for fee in FEES {
        let mut liquidated = 0;
        for px in (700..=840u64).step_by(5) {
            let (mut w, mut a, mut b) = setup(config(fee, 1_000_000, 0), DEPOSIT, px);
            w.refresh(&mut a);
            let p0 = pools(&w, &a, &b);
            let Ok(o) = w.liquidate(&mut a) else { continue };
            liquidated += 1;
            let p1 = pools(&w, &a, &b);
            let ctx = format!("fee {fee} px {px} {o:?}");
            assert!(o.fee_charged <= p0.a_cap, "{ctx}");
            assert!(p1.a_cap <= p0.a_cap && p1.insurance >= p0.insurance, "{ctx}");
            assert_eq!((p0.a_fc, p1.a_fc, p0.b_fc, p1.b_fc), (0, 0, 0, 0), "{ctx}");
            assert!(p1.a_pnl >= 0, "{ctx}: nothing negative left after a full resolve or partial close");
            assert!(p1.vault >= p1.c_tot + p1.insurance, "{ctx}");
            assert_eq!(p1.vault, p0.vault, "{ctx}: liquidation never moves the vault");
            w.sound(&mut [&mut a, &mut b]);
        }
        assert!(liquidated >= 25, "fee {fee}: only {liquidated} prices liquidated");
    }
}

/// An account that is already insolvent (pnl below zero after the principal is used) is charged
/// no fee: `charge_account_fee_current_not_atomic` (src/v16.rs:24760) returns 0 for pnl < 0, and
/// at px 800 the capital is exactly gone, so there is nothing to take. The loss is booked and the
/// insurance fund is not touched by the fee.
#[test]
fn liq_fee_credits_insolvent_account_pays_no_fee() {
    for fee in FEES {
        for (px, residual) in [(800u64, 0u128), (790, 1_000), (700, 10_000)] {
            let (mut w, mut a, mut b) = setup(config(fee, 1_000_000, 0), DEPOSIT, px);
            w.refresh(&mut a);
            let p0 = pools(&w, &a, &b);
            let o = w.liquidate(&mut a).unwrap();
            let p1 = pools(&w, &a, &b);
            assert_eq!(o.fee_charged, 0, "fee {fee} px {px}");
            assert_eq!(o.residual_booked, residual, "fee {fee} px {px}");
            assert_eq!(o.closed_q, Q, "fee {fee} px {px}: full close");
            assert_eq!(p1.insurance, p0.insurance, "fee {fee} px {px}");
            assert_eq!((p1.a_cap, p1.a_fc, pos(&a)), (0, 0, 0), "fee {fee} px {px}");
            w.sound(&mut [&mut a, &mut b]);
        }
    }
}

/// The cap bounds the fee on a partial close, and a bigger cap changes only the fee.
#[test]
fn liq_fee_credits_cap_limits_the_fee() {
    let (mut w, mut a, _b) = setup(config(243, 50, 0), DEPOSIT, 830);
    w.refresh(&mut a);
    let o = w.liquidate(&mut a).unwrap();
    assert_eq!(o.fee_charged, 50);
    assert_eq!(w.header.insurance.get(), 50);
    assert_eq!(a.capital.get(), 3_000 - 50);
    // cap 0 turns the fee off entirely
    let (mut w, mut a, _b) = setup(config(243, 0, 0), DEPOSIT, 830);
    w.refresh(&mut a);
    let o = w.liquidate(&mut a).unwrap();
    assert_eq!((o.fee_charged, w.header.insurance.get(), a.capital.get()), (0, 0, 3_000));
}

/// Partial close vs full close: the fee is on the closed notional, so it grows with closed
/// quantity. At fee 0 the engine closes less than at 243 bps (the fee is part of how much it has
/// to close), and a full close at 800 or below carries no fee.
#[test]
fn liq_fee_credits_partial_vs_full_close_fee() {
    let mut last_closed = 0u128;
    let mut last_fee = 0u128;
    for px in [840u64, 835, 830, 825, 820] {
        let (mut w, mut a, _b) = setup(config(50, 1_000_000, 0), DEPOSIT, px);
        w.refresh(&mut a);
        let o = w.liquidate(&mut a).unwrap();
        assert!(o.closed_q < Q, "px {px}: partial");
        assert!(o.closed_q > last_closed && o.fee_charged > last_fee, "px {px}: deeper = more closed, more fee");
        last_closed = o.closed_q;
        last_fee = o.fee_charged;
        assert_ne!(pos(&a), 0);
    }
    for px in [840u64, 830, 820] {
        let q_at = |fee| {
            let (mut w, mut a, _b) = setup(config(fee, 1_000_000, 0), DEPOSIT, px);
            w.refresh(&mut a);
            w.liquidate(&mut a).unwrap().closed_q
        };
        assert!(q_at(0) < q_at(50) && q_at(50) < q_at(243), "px {px}");
    }
    // full close with fee: the whole position goes at 810 / 243 bps
    let (mut w, mut a, _b) = setup(config(243, 1_000_000, 0), DEPOSIT, 810);
    w.refresh(&mut a);
    assert_eq!(w.liquidate(&mut a).unwrap().closed_q, Q);
}

/// `min_liquidation_abs` needs a bigger `min_nonzero_mm_req` to validate (floor 2 rejects
/// min_abs >= 2; floor 100 takes up to 50 at 50 bps). With floor 100 / min 50: a PARTIAL close
/// is never charged less than the minimum (the engine closes more to get there; the refusal at
/// src/v16.rs:28954 is the backstop), and above the minimum the raw fee is charged.
#[test]
fn liq_fee_credits_min_liquidation_abs_floors_partial_fee() {
    let cfg = |min_abs| {
        let mut c = config(50, 1_000_000, min_abs);
        c.min_nonzero_mm_req = 100;
        c.min_nonzero_im_req = 101;
        c
    };
    assert_eq!(config(50, 1_000_000, 2).validate_public_user_fund(), Err(V16Error::InvalidConfig));
    assert_eq!(cfg(50).validate_public_user_fund(), Ok(()));
    // px 840: with min 0 the engine closes 5_309_524 and pays 23. With min 50 it sizes the
    // partial close up so the fee reaches the minimum (11_666_667 closed, fee 50) instead of
    // refusing, and it never charges below the minimum on a partial close.
    let (mut w, mut a, b) = setup(cfg(0), DEPOSIT, 840);
    w.refresh(&mut a);
    let p0 = pools(&w, &a, &b);
    let o0 = w.liquidate(&mut a).unwrap();
    assert_eq!((o0.closed_q, o0.fee_charged), (5_309_524, 23));
    let (mut w, mut a, _b) = setup(cfg(50), DEPOSIT, 840);
    w.refresh(&mut a);
    let o = w.liquidate(&mut a).unwrap();
    assert_eq!((o.closed_q, o.fee_charged), (11_666_667, 50));
    assert!(o.closed_q < Q && o.fee_charged >= 50);
    assert_eq!(a.capital.get(), DEPOSIT - 16_000 - 50);
    // px 810 is still a partial close; its raw fee 339 is above the minimum, so the raw fee is charged
    let (mut w, mut a, _b) = setup(cfg(50), DEPOSIT, 810);
    w.refresh(&mut a);
    let o = w.liquidate(&mut a).unwrap();
    assert_eq!((o.closed_q, o.fee_charged), (83_679_013, 339));
}

/// Cranking again must not charge again. After a partial liquidation the account is back above
/// maintenance: a second direct liquidation, the crank's Liquidate action (twice) and repeated
/// refreshes leave every pool exactly as it was.
#[test]
fn liq_fee_credits_repeated_cranks_do_not_charge_twice() {
    for fee in FEES {
        for px in [840u64, 830, 820] {
            let (mut w, mut a, mut b) = setup(config(fee, 1_000_000, 0), DEPOSIT, px);
            w.refresh(&mut a);
            let o = w.liquidate(&mut a).unwrap();
            assert!(o.fee_charged > 0 && o.closed_q < Q);
            let p1 = pools(&w, &a, &b);
            let ctx = format!("fee {fee} px {px}");
            assert_eq!(w.liquidate(&mut a), Err(V16Error::NonProgress), "{ctx}");
            let slot = w.slot;
            for _ in 0..2 {
                let r = w.view().permissionless_crank_not_atomic(
                    &mut PortfolioV16ViewMut::new(&mut a),
                    PermissionlessCrankRequestV16 {
                        now_slot: slot,
                        asset_index: 0,
                        effective_price: px,
                        funding_rate_e9: 0,
                        action: PermissionlessCrankActionV16::Liquidate(LiquidationRequestV16 { asset_index: 0 }),
                    },
                );
                assert!(r.is_err(), "{ctx}: {r:?}");
            }
            for _ in 0..3 {
                let cert = w.refresh(&mut a);
                assert_eq!(cert.certified_liq_deficit, 0, "{ctx}: healthy after the partial close");
            }
            assert_eq!(pools(&w, &a, &b), p1, "{ctx}: nothing moved");
            assert_eq!(w.header.insurance.get(), o.fee_charged, "{ctx}: fee charged once");
            w.refresh(&mut b);
            w.sound(&mut [&mut a, &mut b]);
        }
    }
}

/// The crank's Liquidate action reaches the same fee as the direct call.
#[test]
fn liq_fee_credits_crank_liquidate_matches_direct_call() {
    for fee in FEES {
        let direct = {
            let (mut w, mut a, _b) = setup(config(fee, 1_000_000, 0), DEPOSIT, 830);
            w.refresh(&mut a);
            w.liquidate(&mut a).unwrap()
        };
        let (mut w, mut a, mut b) = setup(config(fee, 1_000_000, 0), DEPOSIT, 830);
        let slot = w.slot;
        let ins0 = w.header.insurance.get();
        w.view()
            .permissionless_crank_not_atomic(
                &mut PortfolioV16ViewMut::new(&mut a),
                PermissionlessCrankRequestV16 {
                    now_slot: slot,
                    asset_index: 0,
                    effective_price: 830,
                    funding_rate_e9: 0,
                    action: PermissionlessCrankActionV16::Liquidate(LiquidationRequestV16 { asset_index: 0 }),
                },
            )
            .expect("crank liquidate");
        assert_eq!(w.header.insurance.get() - ins0, direct.fee_charged, "fee {fee}");
        assert_eq!(a.capital.get(), 3_000 - direct.fee_charged, "fee {fee}");
        assert_eq!(a.fee_credits.get(), 0);
        w.sound(&mut [&mut a, &mut b]);
    }
}

/// Matched liquidation against the counterparty (the wrapper's vault-LP path): same fee, same
/// destination, and the counterparty's own `fee_credits` and capital are not charged.
#[test]
fn liq_fee_credits_matched_close_charges_same_fee_to_insurance() {
    for fee in FEES {
        for px in [840u64, 830, 820] {
            let direct = {
                let (mut w, mut a, _b) = setup(config(fee, 1_000_000, 0), DEPOSIT, px);
                w.refresh(&mut a);
                w.liquidate(&mut a).unwrap()
            };
            let (mut w, mut a, mut b) = setup(config(fee, 1_000_000, 0), DEPOSIT, px);
            w.refresh(&mut a);
            w.refresh(&mut b);
            let p0 = pools(&w, &a, &b);
            let o = w
                .view()
                .liquidate_account_against_counterparty_not_atomic(
                    &mut PortfolioV16ViewMut::new(&mut a),
                    &mut PortfolioV16ViewMut::new(&mut b),
                    LiquidationRequestV16 { asset_index: 0 },
                )
                .unwrap();
            let p1 = pools(&w, &a, &b);
            let ctx = format!("fee {fee} px {px}");
            assert_eq!(o, direct, "{ctx}: same outcome as the unilateral close");
            assert_eq!(p1.insurance - p0.insurance, o.fee_charged, "{ctx}");
            assert_eq!(p0.a_cap - p1.a_cap, o.fee_charged, "{ctx}");
            assert_eq!((p1.b_cap, p1.b_pnl, p1.b_fc), (p0.b_cap, p0.b_pnl, 0), "{ctx}");
            assert_eq!(p1.vault, p0.vault, "{ctx}");
            w.sound(&mut [&mut a, &mut b]);
        }
    }
}

/// Conservation across vault, insurance, trader and counterparty for a whole episode (trade,
/// fall, liquidate, refresh both sides): the vault only changes by the deposits, insurance gets
/// exactly the fees, and total claims (capital + insurance + settled pnl) never exceed the vault.
#[test]
fn liq_fee_credits_value_is_conserved_across_the_episode() {
    for fee in FEES {
        for px in [840u64, 830, 820, 810, 790] {
            let (mut w, mut a, mut b) = setup(config(fee, 1_000_000, 0), DEPOSIT, px);
            assert_eq!(w.header.vault.get(), DEPOSIT + BIG);
            w.refresh(&mut a);
            let o = w.liquidate(&mut a).unwrap();
            w.refresh(&mut b);
            w.refresh(&mut a);
            let (v, c, i) = (w.header.vault.get(), w.header.c_tot.get(), w.header.insurance.get());
            let ctx = format!("fee {fee} px {px}");
            assert_eq!(v, DEPOSIT + BIG, "{ctx}: no value enters or leaves");
            assert_eq!(i, o.fee_charged, "{ctx}: insurance is the fee");
            assert_eq!(c, a.capital.get() + b.capital.get(), "{ctx}: C_tot is the two capitals");
            assert!(c + i <= v, "{ctx}");
            assert!(a.pnl.get() >= 0 && a.fee_credits.get() == 0 && b.fee_credits.get() == 0, "{ctx}");
            // trader value lost to the fee equals insurance gained (price loss goes to b, not to the fee)
            assert_eq!(a.capital.get() as i128 + a.pnl.get(), 0.max(DEPOSIT as i128 - (1_000 - px) as i128 * 100) - o.fee_charged as i128, "{ctx}");
            w.sound(&mut [&mut a, &mut b]);
        }
    }
}

/// Same invariants for the SHORT side (price rises). `a` is short, `b` long.
#[test]
fn liq_fee_credits_short_side_fee_goes_to_insurance() {
    for fee in FEES {
        let mut liquidated = 0;
        let mut fee_paid = 0;
        for px in (1_145..=1_230u64).step_by(5) {
            let mut w = W::with(config(fee, 1_000_000, 0)).unwrap();
            let (mut a, mut b) = (account(1), account(2));
            w.deposit(&mut a, DEPOSIT);
            w.deposit(&mut b, BIG);
            w.trade(&mut b, &mut a, Q as i128);
            w.move_to(px);
            w.refresh(&mut a);
            let p0 = pools(&w, &a, &b);
            let Ok(o) = w.liquidate(&mut a) else { continue };
            liquidated += 1;
            let p1 = pools(&w, &a, &b);
            let ctx = format!("fee {fee} px {px} {o:?}");
            assert_eq!(p0.a_cap - p1.a_cap, o.fee_charged, "{ctx}");
            assert_eq!(p1.insurance - p0.insurance, o.fee_charged, "{ctx}");
            assert!(o.fee_charged <= p0.a_cap, "{ctx}");
            assert_eq!((p1.a_fc, p1.b_fc, p1.vault), (0, 0, p0.vault), "{ctx}");
            if o.fee_charged > 0 {
                fee_paid += 1;
            }
            w.sound(&mut [&mut a, &mut b]);
        }
        assert!(liquidated >= 10 && fee_paid >= 8, "fee {fee}: {liquidated} liquidated, {fee_paid} paid");
    }
}
