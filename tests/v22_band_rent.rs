//! v2.2 Phase 4 items 1 + 2: per-epoch price band and holding-fee rent.
//!
//! Engine-level tests for design `phase4-design-2026-10-05.md` §1.3 / §2.3:
//! band math (I-B1, I-B6), census (I-B2), re-anchor (I-B3), duration pin (I-B4),
//! the Band Safety Law (I-B5) under an adversarial mark and keeper schedule,
//! band-off inertness (I-B7), and rent (I-R1..I-R7).
//!
//! Negative controls (mutants) are listed next to each test and are applied by
//! file copy, never by `git stash`; see `docs/v22-band-rent-kani-design.md`.

use percolator::band_rent::{
    band_bounds, band_d1_pinned_step, band_duration_pinned, band_worst_adverse_bps, clamp_to_band,
    price_in_band, rent_chargeable_atoms, rent_due_atoms, rent_index_delta, MAX_BAND_BPS,
    RENT_INDEX_DEN,
};
use percolator::{
    AutoCrankPlanV16, AutoCrankWorkV16, EngineAssetSlotV16Account, LiquidationRequestV16, Market,
    MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, PermissionlessRecoveryReasonV16,
    PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, TradeRequestV16, V16Config, V16Error,
};
use percolator::{MAX_ORACLE_PRICE, POS_SCALE};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const P0: u64 = 1_000_000;
const MARKET_ID: [u8; 32] = [7; 32];
const OWNER: [u8; 32] = [9; 32];

/// A 10x band market: MMR 5%, IMR 10%, liquidation fee 50 bps, funding <= 111 e9,
/// rent <= 23 e9 (~50 bps/day), cap 100 bps/slot over <= 3 slots, d = 130 bps,
/// E = 600 slots, Pmax = 9,000 slots (the design's 10x preset).
fn band_cfg(band_bps: u64) -> V16Config {
    let mut c = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    c.maintenance_margin_bps = 500;
    c.initial_margin_bps = 1_000;
    // Floors large enough that integer rounding of loss + fee cannot exceed the
    // maintenance requirement at small notionals (the exact §1.6 law checks it).
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
    // Counterparty backing stays fresh for the whole test (freshness horizon =
    // max(max_accrual_dt, h_max, max_bankrupt_close_lifetime)); with the 10-slot
    // default a winner's claim lapses and its refresh reports
    // SourceBackingExpired every time, so it could never be certified.
    c.max_bankrupt_close_lifetime_slots = 1_000_000;
    c.band_bps = band_bps;
    if band_bps != 0 {
        c.band_max_epoch_slots = 600;
        c.band_max_pin_slots = 9_000;
        c.band_max_positions_per_side = percolator::band_rent::BAND_MAX_POSITIONS_PER_SIDE;
        // Engine minimum (the wrapper floors it at whole tokens): fixture trades stay valid.
        c.band_min_leg_notional = 1;
    }
    c
}

struct World {
    header: MarketGroupV16HeaderAccount,
    markets: Vec<Market<u64>>,
    accounts: Vec<PortfolioAccountV16Account>,
    now: u64,
}

impl World {
    fn new(cfg: V16Config, n_accounts: usize, deposit: u128) -> Self {
        Self::new_at(cfg, n_accounts, deposit, P0)
    }

    /// A World launched at `P0` whose (still empty) asset is then moved to `price` by a state
    /// poke, bypassing the N-2 genesis rule: for tests that need a book near the width floor.
    fn new_poked_at(cfg: V16Config, n_accounts: usize, deposit: u128, price: u64) -> Self {
        let mut w = Self::new(cfg, n_accounts, deposit);
        let mut a = w.asset();
        a.effective_price = price;
        a.raw_oracle_target_price = price;
        a.fund_px_last = price;
        a.band_anchor_price = price;
        w.markets[0].engine.asset = percolator::AssetStateV16Account::from_runtime(&a);
        w
    }

    fn new_at(cfg: V16Config, n_accounts: usize, deposit: u128, price: u64) -> Self {
        cfg.validate_public_user_fund()
            .expect("fixture config validates");
        let mut header = MarketGroupV16HeaderAccount::new_dynamic(MARKET_ID, cfg, 1, 0).unwrap();
        let mut markets = vec![Market::new(0, EngineAssetSlotV16Account::default())];
        header
            .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, price, 1)
            .unwrap();
        let mut accounts = Vec::new();
        for i in 0..n_accounts {
            let prov = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
                MARKET_ID,
                {
                    // > 255 accounts (re-review cap tests): two key bytes.
                    let mut k = [1u8; 32];
                    k[0] = (i % 256) as u8;
                    k[1] = (i / 256) as u8 + 1;
                    k
                },
                OWNER,
            ));
            let mut a = PortfolioAccountV16Account::default();
            a.init_empty_in_place(prov).unwrap();
            accounts.push(a);
        }
        let mut w = Self {
            header,
            markets,
            accounts,
            now: 1,
        };
        for i in 0..n_accounts {
            w.with(i, |m, a| m.deposit_not_atomic(a, deposit)).unwrap();
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

    fn asset(&self) -> percolator::AssetStateV16 {
        self.markets[0].engine.asset.try_to_runtime().unwrap()
    }

    fn trade(&mut self, long: usize, short: usize, q: u128) -> Result<(), V16Error> {
        assert_ne!(long, short);
        let price = self.asset().effective_price;
        let (lo, hi) = if long < short {
            (long, short)
        } else {
            (short, long)
        };
        let (left, right) = self.accounts.split_at_mut(hi);
        let (a_lo, a_hi) = (&mut left[lo], &mut right[0]);
        let (l, s) = if long < short {
            (a_lo, a_hi)
        } else {
            (a_hi, a_lo)
        };
        let mut m = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let mut lv = PortfolioV16ViewMut::new(l);
        let mut sv = PortfolioV16ViewMut::new(s);
        m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut lv,
            &mut sv,
            TradeRequestV16 {
                asset_index: 0,
                size_q: q as i128,
                exec_price: price,
                fee_bps: 0,
            },
            true,
        )
        .map(|_| ())
    }

    /// The wrapper's price recipe for a single-segment accrual to `self.now`:
    /// capped staircase toward the target, then the band (edge clamp / duration
    /// pin) against the anchor the engine will use after any re-anchor.
    fn wrapper_price(&self) -> (u64, bool) {
        let a = self.asset();
        let cfg = self.header.config.try_to_runtime_shape().unwrap();
        let dt = (self.now - a.slot_last).min(cfg.max_accrual_dt_slots);
        let target = a.raw_oracle_target_price;
        let p = a.effective_price;
        let exposed = a.oi_eff_long_q != 0 || a.oi_eff_short_q != 0;
        let capped = if !exposed {
            target
        } else {
            let max_delta =
                (p as u128 * cfg.max_price_move_bps_per_slot as u128 * dt as u128 / 10_000) as u64;
            if target > p {
                p + max_delta.min(target - p)
            } else {
                p - max_delta.min(p - target)
            }
        };
        let mut header = self.header;
        let mut markets = self.markets.clone();
        let m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        match m.band_accrual_preview(0, a.slot_last + dt).unwrap() {
            None => (capped, false),
            Some((anchor, d, governing, e)) => {
                let pinned = band_duration_pinned(a.slot_last + dt, governing, e);
                let (price, _) = band_d1_pinned_step(p, capped, anchor, d, pinned).unwrap();
                (price, pinned)
            }
        }
    }

    fn accrue(&mut self, funding: i128, rent_long: u64, rent_short: u64) -> Result<(), V16Error> {
        let (price, pinned) = self.wrapper_price();
        let (f, rl, rs) = if pinned {
            (0, 0, 0)
        } else {
            (funding, rent_long, rent_short)
        };
        let now = self.now;
        self.market(|m| m.accrue_asset_to_with_rent_not_atomic(0, now, price, f, rl, rs, true))
            .map(|_| ())
    }

    fn set_target(&mut self, target: u64) {
        self.market(|m| m.set_asset_raw_oracle_target_not_atomic(0, target))
            .unwrap();
    }

    /// A keeper refresh. `Stale` from a full refresh is the engine reporting that
    /// it expired one lapsed source-backing bucket (committed progress); the
    /// keeper simply retries, exactly as the auto-crank treats
    /// `SourceBackingExpired` as progress.
    fn refresh(&mut self, i: usize) -> Result<percolator::HealthCertV16, V16Error> {
        let mut last = Err(V16Error::Stale);
        for _ in 0..8 {
            last = self.with(i, |m, a| m.full_account_refresh_not_atomic(a));
            if last != Err(V16Error::Stale) {
                break;
            }
        }
        last
    }

    fn positioned(&self, i: usize) -> bool {
        self.accounts[i].legs[0].try_to_runtime().unwrap().active
    }

    /// I-B2: the band counters equal the leg census over every portfolio.
    fn assert_census(&self) {
        let a = self.asset();
        let (mut unc_l, mut unc_s, mut lp_l, mut lp_s, mut n_l, mut n_s) = (0, 0, 0, 0, 0, 0);
        for acct in &self.accounts {
            let leg = acct.legs[0].try_to_runtime().unwrap();
            if !leg.active {
                continue;
            }
            let long = leg.side == percolator::SideV16::Long;
            if long {
                n_l += 1
            } else {
                n_s += 1
            }
            if a.band_epoch != 0 && leg.band_epoch_snap < a.band_epoch {
                if long {
                    unc_l += 1
                } else {
                    unc_s += 1
                }
            }
            if leg.band_liq_pending {
                if long {
                    lp_l += 1
                } else {
                    lp_s += 1
                }
            }
            assert!(leg.band_epoch_snap <= a.band_epoch);
        }
        assert_eq!(
            (a.stored_pos_count_long, a.stored_pos_count_short),
            (n_l, n_s)
        );
        assert_eq!(
            (a.band_uncertified_long, a.band_uncertified_short),
            (unc_l, unc_s),
            "uncertified cohort census (I-B2)"
        );
        assert_eq!(
            (a.band_liq_pending_long, a.band_liq_pending_short),
            (lp_l, lp_s),
            "liq-pending cohort census (I-B2)"
        );
    }

    fn assert_conservation(&self) {
        let v = self.header.vault.get();
        let c = self.header.c_tot.get();
        let i = self.header.insurance.get();
        assert!(v >= c + i, "V >= C_tot + I violated: {v} < {c} + {i}");
    }
}

// ---------------------------------------------------------------------------
// Pure band math (I-B1, I-B6) — bounded exhaustive + full-width proptest
// ---------------------------------------------------------------------------

#[test]
fn band_bounds_round_inward_and_contain_anchor() {
    for anchor in [1u64, 2, 3, 99, 100, 101, 9_999, 1_000_000, MAX_ORACLE_PRICE] {
        for d in [1u64, 7, 130, 300, 999, MAX_BAND_BPS] {
            let (lo, hi) = band_bounds(anchor, d).unwrap();
            assert!(1 <= lo && lo <= anchor && anchor <= hi && hi <= MAX_ORACLE_PRICE);
            // Inward rounding: lo * 1e4 >= anchor * (1e4 - d), hi * 1e4 <= anchor * (1e4 + d).
            assert!(lo as u128 * 10_000 >= anchor as u128 * (10_000 - d as u128));
            assert!(hi as u128 * 10_000 <= anchor as u128 * (10_000 + d as u128));
        }
    }
    assert!(band_bounds(0, 130).is_err());
    assert!(band_bounds(1, 0).is_err());
    assert!(band_bounds(1, MAX_BAND_BPS + 1).is_err());
    assert!(band_bounds(MAX_ORACLE_PRICE + 1, 130).is_err());
}

#[test]
fn band_worst_adverse_matches_design_table() {
    // design §1.1 worked examples: G(130) = 397, G(300) = 938, G(60) = 182.
    assert_eq!(band_worst_adverse_bps(130).unwrap(), 397);
    assert_eq!(band_worst_adverse_bps(300).unwrap(), 938);
    assert_eq!(band_worst_adverse_bps(60).unwrap(), 182);
    // Mutant "G = 2d" would give 260 / 600 / 120 and fail here.
}

/// I-B6, exhaustive on a small domain: for every d, anchor A, A' in band(A),
/// p1 in band(A), p3 in band(A'), the adverse move |p3 - p1| / p1 <= G (both
/// directions). Uses the real `band_bounds` / `band_worst_adverse_bps`.
#[test]
fn band_g_bounds_every_two_epoch_path_exhaustive_small_domain() {
    let mut checked = 0u64;
    for d in [1u64, 13, 60, 130, 300, 1_000, MAX_BAND_BPS] {
        let g = band_worst_adverse_bps(d).unwrap() as u128;
        for a in (100u64..=2_000).step_by(37) {
            let (lo, hi) = band_bounds(a, d).unwrap();
            for a2 in [lo, (lo + hi) / 2, hi] {
                let (lo2, hi2) = band_bounds(a2, d).unwrap();
                for p1 in [lo, a, hi] {
                    for p3 in [lo2, a2, hi2] {
                        let mv = p1.abs_diff(p3) as u128;
                        assert!(
                            mv * 10_000 <= g * p1 as u128,
                            "d={d} A={a} A'={a2} p1={p1} p3={p3}: move {mv} > G={g}"
                        );
                        checked += 1;
                    }
                }
            }
        }
    }
    assert!(checked > 5_000, "non-vacuity");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2_000))]

    /// I-B6 at full price width.
    #[test]
    fn band_g_bounds_two_epoch_path_full_width(
        d in 1u64..=MAX_BAND_BPS,
        a in 1u64..=MAX_ORACLE_PRICE,
        f2 in 0u64..=10_000, f1 in 0u64..=10_000, f3 in 0u64..=10_000,
    ) {
        let g = band_worst_adverse_bps(d).unwrap() as u128;
        let (lo, hi) = band_bounds(a, d).unwrap();
        let pick = |lo: u64, hi: u64, f: u64| lo + ((hi - lo) as u128 * f as u128 / 10_000) as u64;
        let a2 = pick(lo, hi, f2);
        let p1 = pick(lo, hi, f1);
        let (lo2, hi2) = band_bounds(a2, d).unwrap();
        let p3 = pick(lo2, hi2, f3);
        prop_assert!((p1.abs_diff(p3) as u128) * 10_000 <= g * p1 as u128);
    }

    /// clamp_to_band: in range, idempotent, never overshoots, identity inside.
    #[test]
    fn band_clamp_in_range_full_width(
        d in 1u64..=MAX_BAND_BPS, a in 1u64..=MAX_ORACLE_PRICE, p in 1u64..=MAX_ORACLE_PRICE,
    ) {
        let (lo, hi) = band_bounds(a, d).unwrap();
        let c = clamp_to_band(p, a, d).unwrap();
        prop_assert!(lo <= c && c <= hi);
        prop_assert_eq!(clamp_to_band(c, a, d).unwrap(), c);
        if price_in_band(p, a, d).unwrap() { prop_assert_eq!(c, p); }
        if p > hi { prop_assert_eq!(c, hi); }
        if p < lo { prop_assert_eq!(c, lo); }
    }

    /// D-1 path: a duration-pinned step is exactly P_last; an edge step is the
    /// edge; otherwise the cap-law step passes through unchanged.
    #[test]
    fn band_d1_step_semantics(
        d in 1u64..=MAX_BAND_BPS, a in 1u64..=MAX_ORACLE_PRICE / 2,
        cur_f in 0u64..=10_000, step in 1u64..=MAX_ORACLE_PRICE, pinned in any::<bool>(),
    ) {
        let (lo, hi) = band_bounds(a, d).unwrap();
        let cur = lo + ((hi - lo) as u128 * cur_f as u128 / 10_000) as u64;
        let (p, adjusted) = band_d1_pinned_step(cur, step, a, d, pinned).unwrap();
        if pinned {
            prop_assert_eq!(p, cur);
            prop_assert_eq!(adjusted, step != cur);
        } else {
            prop_assert_eq!(p, step.clamp(lo, hi));
            prop_assert_eq!(adjusted, p != step);
        }
    }

    /// I-R4 / floor: rent due is floor-exact, additive-subadditive (splitting a
    /// settle never charges more than one settle), zero for zero delta.
    #[test]
    fn rent_due_floor_and_split_never_overcharges_full_width(
        q in 0u128..=100_000_000_000_000u128,
        r0 in 0u128..=u64::MAX as u128, d1 in 0u128..=u64::MAX as u128, d2 in 0u128..=u64::MAX as u128,
    ) {
        let r1 = r0 + d1;
        let r2 = r1 + d2;
        let whole = rent_due_atoms(q, r2, r0).unwrap();
        let split = rent_due_atoms(q, r1, r0).unwrap() + rent_due_atoms(q, r2, r1).unwrap();
        prop_assert!(split <= whole, "splitting a settle must never overcharge");
        prop_assert!(whole <= split + 1, "floor loses at most one atom per extra settle");
        // exact floor
        let exact = q.checked_mul(r2 - r0).map(|v| v / RENT_INDEX_DEN);
        if let Some(e) = exact { prop_assert_eq!(whole, e); }
        prop_assert_eq!(rent_due_atoms(q, r2, r2).unwrap(), 0);
    }

    /// I-R7: rent never takes capital that an unsettled loss owns, never exceeds due.
    #[test]
    fn rent_charge_is_loss_junior(
        due in any::<u128>(), capital in any::<u128>(), pnl in (i128::MIN + 1)..=i128::MAX,
    ) {
        let c = rent_chargeable_atoms(due, capital, pnl);
        prop_assert!(c <= due && c <= capital);
        if pnl < 0 {
            prop_assert!(capital - c >= pnl.unsigned_abs().min(capital));
        }
    }

    /// I-R2: index growth is exact and overflow fails closed.
    #[test]
    fn rent_index_delta_exact_or_fails_closed(p in any::<u64>(), r in any::<u64>(), dt in any::<u64>()) {
        let expect = (p as u128).checked_mul(r as u128).and_then(|v| v.checked_mul(dt as u128));
        match rent_index_delta(p, r, dt) {
            Ok(v) => prop_assert_eq!(Some(v), expect),
            Err(_) => prop_assert!(expect.is_none()),
        }
    }
}

// ---------------------------------------------------------------------------
// Band Safety Law at InitMarket (I-B5 precondition)
// ---------------------------------------------------------------------------

#[test]
fn bsl_accepts_design_presets_and_rejects_wider_bands() {
    // 10x preset (d = 130) validates; the law refuses bands whose G + fee exceeds MMR.
    assert!(band_cfg(130).validate_public_user_fund().is_ok());
    let max_ok = (1..=MAX_BAND_BPS)
        .take_while(|&d| band_cfg(d).validate_public_user_fund().is_ok())
        .last()
        .unwrap();
    // G(d) + ~52 bps fee + funding/rent must stay <= 500 bps: the exact maximum.
    assert!(
        max_ok >= 130,
        "design preset must validate (max_ok = {max_ok})"
    );
    let g = band_worst_adverse_bps(max_ok).unwrap();
    let g_next = band_worst_adverse_bps(max_ok + 1).unwrap();
    assert!(
        g + 50 <= 500 && g_next + 50 > 440,
        "boundary is where G + fee meets MMR: {max_ok}"
    );
    assert!(band_cfg(max_ok + 1).validate_public_user_fund().is_err());
    // Mutant "G = 2d" would accept d up to ~225 here: the assert on max_ok + 1 goes red.
}

#[test]
fn band_config_shape_rules() {
    let mut c = band_cfg(130);
    c.max_market_slots = 2;
    c.max_portfolio_assets = 2;
    assert_eq!(
        c.validate_public_user_fund(),
        Err(V16Error::InvalidConfig),
        "band is single-asset"
    );
    let mut c = band_cfg(130);
    c.band_max_epoch_slots = 0;
    assert!(c.validate_public_user_fund().is_err(), "E >= 1");
    let mut c = band_cfg(130);
    c.band_max_pin_slots = c.band_max_epoch_slots - 1;
    assert!(c.validate_public_user_fund().is_err(), "Pmax >= E");
    let mut c = band_cfg(0);
    c.band_max_epoch_slots = 600;
    assert!(
        c.validate_public_user_fund().is_err(),
        "off band has zero clocks"
    );
    let mut c = band_cfg(130);
    c.rent_max_e9_per_slot = percolator::band_rent::MAX_RENT_E9_PER_SLOT + 1;
    assert!(c.validate_public_user_fund().is_err(), "rent ceiling");
    let mut c = band_cfg(130);
    c.band_bps = MAX_BAND_BPS + 1;
    assert!(c.validate_public_user_fund().is_err());
}

/// Review E-M1 / E-M2: the per-side position cap and the E / Pmax floors are
/// config-shape rules, each boundary pinned on both sides.
#[test]
fn band_config_floors_and_position_cap() {
    use percolator::band_rent::{
        BAND_MAX_POSITIONS_PER_SIDE, BAND_MIN_EPOCH_SLOTS, BAND_MIN_PIN_EPOCHS,
    };
    let ok = |c: &V16Config| c.validate_public_user_fund().is_ok();
    let mut c = band_cfg(130);
    c.band_max_epoch_slots = BAND_MIN_EPOCH_SLOTS;
    c.band_max_pin_slots = BAND_MIN_EPOCH_SLOTS * BAND_MIN_PIN_EPOCHS;
    assert!(ok(&c), "E and Pmax exactly at their floors");
    c.band_max_epoch_slots = BAND_MIN_EPOCH_SLOTS - 1;
    c.band_max_pin_slots = 20 * BAND_MIN_EPOCH_SLOTS;
    assert!(
        !ok(&c),
        "E below the floor (E = 1 pinned on one missed sweep)"
    );
    let mut c = band_cfg(130);
    c.band_max_pin_slots = c.band_max_epoch_slots * BAND_MIN_PIN_EPOCHS - 1;
    assert!(!ok(&c), "Pmax below 8E");
    let mut c = band_cfg(130);
    c.band_max_epoch_slots = 1;
    c.band_max_pin_slots = 1;
    assert!(!ok(&c), "the review's E = 1, Pmax = 1 example");
    let mut c = band_cfg(130);
    c.band_max_positions_per_side = 0;
    assert!(!ok(&c), "band on needs a cap");
    c.band_max_positions_per_side = BAND_MAX_POSITIONS_PER_SIDE + 1;
    assert!(!ok(&c), "cap above 256");
    c.band_max_positions_per_side = 1;
    assert!(ok(&c));
    let mut c = band_cfg(0);
    c.band_max_positions_per_side = 1;
    assert!(!ok(&c), "band off encodes cap 0");
}

/// Review E-M1: dust legs cannot hold the epoch. With a per-side cap K the
/// (K+1)-th leg on a side is refused, so the keeper's per-epoch sweep is at most
/// 2K refreshes and the anchor advances once they are done.
#[test]
fn band_position_cap_bounds_the_sweep_so_dust_cannot_hold_the_epoch() {
    const K: u64 = 4;
    let mut cfg = band_cfg(130);
    cfg.band_max_positions_per_side = K;
    let n = 2 * (K as usize + 2);
    let mut w = World::new(cfg, n, 10_000_000);
    // K dust pairs fill both sides to the cap ...
    for k in 0..K as usize {
        w.trade(2 * k, 2 * k + 1, POS_SCALE / 1_000)
            .expect("open within cap");
    }
    let a = w.asset();
    assert_eq!((a.stored_pos_count_long, a.stored_pos_count_short), (K, K));
    // ... and the (K+1)-th leg on either side is refused, state untouched.
    let before = (w.header, w.markets.clone(), w.accounts.clone());
    let k = K as usize;
    assert_eq!(
        w.trade(2 * k, 2 * k + 1, POS_SCALE / 1_000),
        Err(V16Error::BandPositionCap)
    );
    assert_eq!((w.header, w.markets.clone()), (before.0, before.1));
    // Adding to an EXISTING leg is not a new position: still allowed.
    w.trade(0, 1, POS_SCALE / 1_000)
        .expect("increase an existing leg");
    // The epoch advances after one bounded sweep of every positioned leg.
    w.set_target(2 * P0);
    w.now += 3;
    w.accrue(0, 0, 0).unwrap();
    let e = w.asset().band_epoch;
    for _ in 0..5 {
        w.now += 3;
        w.accrue(0, 0, 0).unwrap();
    }
    assert_eq!(
        w.asset().band_epoch,
        e,
        "uncertified dust holds the epoch ..."
    );
    let mut swept = 0;
    for i in 0..n {
        if w.positioned(i) {
            w.refresh(i).unwrap();
            swept += 1;
        }
    }
    assert!(
        swept as u64 <= 2 * K,
        "... for at most 2K refreshes ({swept})"
    );
    w.now += 3;
    w.accrue(0, 0, 0).unwrap();
    assert_eq!(w.asset().band_epoch, e + 1, "... and then advances");
    // Closing a leg frees its slot for a new account.
    let q = w.accounts[0].legs[0]
        .try_to_runtime()
        .unwrap()
        .basis_pos_q
        .unsigned_abs();
    w.trade(1, 0, q).ok();
    w.assert_census();
}

/// Review E-L1: the band can never be (near) zero-width. Genesis below the width
/// floor is refused, an asset whose anchor has drifted below it accepts no new
/// exposure, and re-anchoring never lands on a too-narrow band.
#[test]
fn band_never_zero_width() {
    use percolator::band_rent::{band_width_ok, MIN_BAND_WIDTH_TICKS};
    // Every anchor the width rule accepts has hi - lo >= 32 (exhaustive over small prices).
    for d in [1u64, 130, 145, MAX_BAND_BPS] {
        for a in 1..20_000u64 {
            let (lo, hi) = band_bounds(a, d).unwrap();
            assert_eq!(
                band_width_ok(a, d).unwrap(),
                hi - lo >= MIN_BAND_WIDTH_TICKS
            );
            if band_width_ok(a, d).unwrap() {
                assert!(lo < a && a < hi, "a width-ok band moves both ways");
            }
        }
    }
    // Genesis at a tiny price is refused.
    let mut asset = percolator::AssetStateV16::default();
    assert_eq!(
        percolator::band_initialize_asset(&mut asset, 130, 76, 1),
        Err(V16Error::BandTooNarrow)
    );
    assert!(percolator::band_initialize_asset(&mut asset, 130, P0, 1).is_ok());
    // Re-anchoring never lands on a too-narrow band: walk a certified book down from
    // 1,300 ticks (width 32 at d = 130) with the keeper certifying every epoch. The
    // anchor follows down to 1,236 (band [1220, 1252]); 1,220 would be 30 wide, so
    // the anchor stays and the book pins at the edge (recovery is BandPinExpired).
    let mut w = World::new_poked_at(band_cfg(130), 2, 10_000_000, 1_300);
    w.trade(0, 1, 1_000 * POS_SCALE).unwrap();
    w.set_target(1);
    let mut anchors = vec![];
    for _ in 0..40 {
        w.now += 3;
        w.accrue(0, 0, 0).unwrap();
        let a = w.asset();
        assert!(
            band_width_ok(a.band_anchor_price, 130).unwrap(),
            "anchor {} has a band narrower than {MIN_BAND_WIDTH_TICKS}",
            a.band_anchor_price
        );
        anchors.push(a.band_anchor_price);
        for i in 0..2 {
            w.refresh(i).unwrap();
        }
    }
    let a = w.asset();
    assert_eq!(
        a.band_anchor_price, 1_236,
        "followed down to the last wide-enough anchor"
    );
    assert_eq!(a.effective_price, 1_220, "pinned at lo of the last band");
    assert!(
        anchors.contains(&1_284),
        "the anchor did advance before the floor"
    );
    // A live asset whose anchor sits below the floor refuses new exposure.
    let mut w = World::new(band_cfg(130), 2, 10_000_000);
    w.markets[0].engine.asset.band_anchor_price = percolator::V16PodU64::new(76);
    assert_eq!(w.trade(0, 1, POS_SCALE), Err(V16Error::BandTooNarrow));
}

// ---------------------------------------------------------------------------
// Engine flows: census, re-anchor, pin, liquidation pending
// ---------------------------------------------------------------------------

fn open_book(w: &mut World, pairs: usize, q: u128) {
    for k in 0..pairs {
        w.trade(2 * k, 2 * k + 1, q).expect("open pair");
    }
}

#[test]
fn band_activation_initialises_epoch_one_at_the_price() {
    let w = World::new(band_cfg(130), 2, 1_000_000);
    let a = w.asset();
    assert_eq!(
        (a.band_epoch, a.band_anchor_price, a.band_anchor_slot),
        (1, P0, 1)
    );
    let w0 = World::new(band_cfg(0), 2, 1_000_000);
    let a0 = w0.asset();
    assert_eq!(
        (a0.band_epoch, a0.band_anchor_price, a0.band_anchor_slot),
        (0, 0, 0),
        "I-B7"
    );
}

#[test]
fn band_trade_certifies_new_legs_and_census_holds() {
    let mut w = World::new(band_cfg(130), 4, 10_000_000);
    open_book(&mut w, 2, 5 * POS_SCALE);
    w.assert_census();
    let a = w.asset();
    assert_eq!(a.stored_pos_count_long, 2);
    assert_eq!(
        (a.band_uncertified_long, a.band_uncertified_short),
        (0, 0),
        "an IM-approved open is a C-event (b)"
    );
}

#[test]
fn band_out_of_band_accrual_rejected_before_mutation() {
    let mut w = World::new(band_cfg(130), 2, 10_000_000);
    open_book(&mut w, 1, 5 * POS_SCALE);
    w.now = 2;
    let before_header = w.header;
    let before_markets = w.markets.clone();
    let (_, hi) = band_bounds(P0, 130).unwrap();
    // Within the per-slot cap (100 bps) but outside the band (130 bps)? Use 2 slots
    // so the cap allows 200 bps; hi + 1 is then cap-legal but band-illegal.
    w.now = 3;
    let r = w.market(|m| m.accrue_asset_to_not_atomic(0, 3, hi + 1, 0, true));
    assert_eq!(r, Err(V16Error::BandOutOfRange));
    assert_eq!(w.header, before_header, "header untouched (I-B1)");
    assert_eq!(
        w.markets[0].engine, before_markets[0].engine,
        "asset untouched (I-B1)"
    );
    // The edge itself is accepted.
    assert!(w
        .market(|m| m.accrue_asset_to_not_atomic(0, 3, hi, 0, true))
        .is_ok());
}

#[test]
fn band_reanchor_waits_for_every_leg_then_advances_to_p_last() {
    let mut w = World::new(band_cfg(130), 4, 10_000_000);
    open_book(&mut w, 2, 5 * POS_SCALE);
    w.set_target(2 * P0);
    // epoch 1 -> first accrual: everything certified at open, so the anchor
    // advances at this accrual (A := P_last = P0, e = 2) and the price climbs.
    w.now = 4;
    w.accrue(0, 0, 0).unwrap();
    let a = w.asset();
    assert_eq!(a.band_epoch, 2);
    assert_eq!(a.band_anchor_price, P0);
    assert_eq!((a.band_uncertified_long, a.band_uncertified_short), (2, 2));
    // Climb to the edge; partially certify; the anchor must not move (I-B3).
    for _ in 0..5 {
        w.now += 3;
        w.accrue(0, 0, 0).unwrap();
    }
    let (_, hi) = band_bounds(P0, 130).unwrap();
    assert_eq!(w.asset().effective_price, hi, "edge pin");
    assert!(
        w.asset().band_pin_since_slot != 0,
        "pin clock started at the edge"
    );
    for i in 0..3 {
        w.refresh(i).unwrap();
    }
    w.now += 3;
    w.accrue(0, 0, 0).unwrap();
    assert_eq!(
        w.asset().band_epoch,
        2,
        "one uncertified leg blocks the advance"
    );
    assert_eq!(w.asset().effective_price, hi);
    w.refresh(3).unwrap();
    w.assert_census();
    w.now += 3;
    w.accrue(0, 0, 0).unwrap();
    let a = w.asset();
    assert_eq!(a.band_epoch, 3, "advance once the whole book is certified");
    assert_eq!(a.band_anchor_price, hi, "A' = P_last");
    assert!(
        a.effective_price > hi,
        "the staircase continues in the new band"
    );
    w.assert_census();
    // Mutant "re-anchor without the uncertified == 0 check" turns the epoch-2 assert red.
}

#[test]
fn band_duration_pin_forces_no_move_accruals() {
    let mut w = World::new(band_cfg(130), 2, 10_000_000);
    open_book(&mut w, 1, 5 * POS_SCALE);
    w.set_target(2 * P0);
    w.now = 2;
    w.accrue(0, 0, 0).unwrap(); // advances to e=2, legs uncertified
    let anchor_slot = w.asset().band_anchor_slot;
    // Nobody certifies. The pin is measured on the engine clock (the clock funding
    // and rent are charged on), which advances <= max_accrual_dt per accrual:
    // walk it to the end of the window E = 600 with ordinary accruals.
    while w.asset().slot_last + 3 <= anchor_slot + 600 {
        w.now += 3;
        w.accrue(0, 0, 0).unwrap();
    }
    w.now += 3;
    let now = w.now;
    let p_last = w.asset().effective_price;
    assert!(
        w.asset().slot_last + 3 > anchor_slot + 600,
        "next segment crosses the window"
    );
    // Any moving accrual is refused (I-B4) ...
    // (P_last sits at the upper edge, so probe one atom below it: in band.)
    let r = w.market(|m| m.accrue_asset_to_not_atomic(0, now, p_last - 1, 0, true));
    assert_eq!(r, Err(V16Error::BandPinned));
    let r = w.market(|m| m.accrue_asset_to_not_atomic(0, now, p_last, 7, true));
    assert_eq!(r, Err(V16Error::BandPinned), "funding is zero while pinned");
    let r = w.market(|m| m.accrue_asset_to_with_rent_not_atomic(0, now, p_last, 0, 5, 0, true));
    assert_eq!(r, Err(V16Error::BandPinned), "rent is zero while pinned");
    // ... and the no-move accrual is accepted (D-1) and starts the pin clock.
    let rent_before = w.asset().rent_index_long_num;
    w.market(|m| m.accrue_asset_to_not_atomic(0, now, p_last, 0, true))
        .unwrap();
    assert_eq!(w.asset().effective_price, p_last);
    assert_eq!(w.asset().rent_index_long_num, rent_before, "I-R6");
    assert!(w.asset().band_pin_since_slot != 0);
    // Certify the book. The next accrual re-anchors (A' = P_last, e = 3, the new
    // window opens at its segment end) but its own interval still belongs to the
    // expired window, so it is no-move; the accrual after that moves again.
    w.refresh(0).unwrap();
    w.refresh(1).unwrap();
    w.now += 3;
    w.accrue(0, 0, 0).unwrap();
    assert_eq!(w.asset().band_epoch, 3);
    assert_eq!(w.asset().band_anchor_price, p_last);
    assert_eq!(
        w.asset().effective_price,
        p_last,
        "re-anchoring accrual is governed by the old window"
    );
    w.now += 3;
    w.accrue(0, 0, 0).unwrap();
    assert!(w.asset().effective_price > p_last, "the new epoch moves");
    // Mutant "no duration pin" lets the p_last - 1 accrual through.
}

#[test]
fn band_liq_pending_blocks_advance_until_liquidated_then_no_bad_debt() {
    let mut w = World::new(band_cfg(130), 4, 1_000_000);
    // Account 0 is near-IM long; 1 is a deep short; 2/3 are a well-funded pair.
    w.trade(0, 1, 9 * POS_SCALE).unwrap();
    w.set_target(P0 / 2);
    let mut liquidated = false;
    let mut saw_pending = false;
    for _ in 0..400 {
        w.now += 3;
        w.accrue(0, 0, 0).unwrap();
        for i in 0..2 {
            if !w.positioned(i) {
                continue;
            }
            let cert = w.refresh(i).unwrap();
            assert!(cert.certified_equity >= 0, "loss <= capital (I-B5)");
            if cert.certified_liq_deficit != 0 {
                saw_pending = true;
                assert!(
                    w.accounts[i].legs[0]
                        .try_to_runtime()
                        .unwrap()
                        .band_liq_pending
                );
                let e = w.asset().band_epoch;
                // The pending leg blocks the advance even though it is the only one.
                w.now += 3;
                w.accrue(0, 0, 0).unwrap();
                assert_eq!(w.asset().band_epoch, e, "liq-pending blocks (I-B3)");
                let out = w
                    .with(i, |m, a| {
                        m.liquidate_account_not_atomic(a, LiquidationRequestV16 { asset_index: 0 })
                    })
                    .unwrap();
                assert_eq!(
                    (out.insurance_used, out.residual_booked, out.explicit_loss),
                    (0, 0, 0),
                    "BSL: liquidation with D == 0"
                );
                liquidated = true;
            }
        }
        w.assert_census();
        w.assert_conservation();
        assert_eq!(
            w.header.bankruptcy_hlock_active, 0,
            "no hlock from a band market"
        );
        if liquidated {
            break;
        }
    }
    assert!(
        saw_pending && liquidated,
        "non-vacuity: the scenario reached a liquidation"
    );
    // Mutant "re-anchor without the liq_pending == 0 check" turns the blocks assert red.
}

/// The liquidation-pending cohort's own job (design §1.1, "mandatory per-epoch
/// liquidation"): a leg that was certified healthy EARLIER in the epoch and then
/// turns liquidatable blocks the advance in that same epoch, even though every
/// leg is certified. (For the loss <= capital bound alone the uncertified cohort
/// suffices one epoch later; this is the stricter, design-mandated timing.)
#[test]
fn band_liq_pending_blocks_advance_even_after_the_leg_was_certified() {
    let mut w = World::new(band_cfg(130), 4, 1_000_000);
    // 0: near-IM long (the victim). 1: its short. 2/3: a second pair whose long
    // (2) the keeper certifies last, so the epoch stays open while price moves.
    w.trade(0, 1, 9 * POS_SCALE).unwrap();
    w.trade(2, 3, POS_SCALE).unwrap();
    w.set_target(P0 / 2);
    let mut isolated = false;
    for _ in 0..400 {
        // Certify everyone except 2, then move the price inside the epoch.
        for i in [0usize, 1, 3] {
            if w.positioned(i) {
                w.refresh(i).unwrap();
            }
        }
        let e = w.asset().band_epoch;
        let c0 = w.accounts[0].legs[0].try_to_runtime().unwrap();
        w.now += 3;
        w.accrue(0, 0, 0).unwrap();
        assert_eq!(w.asset().band_epoch, e, "2 is uncertified: no advance");
        if !w.positioned(0) {
            break;
        }
        let cert = w.refresh(0).unwrap();
        if cert.certified_liq_deficit != 0 && c0.band_epoch_snap == e {
            // 0 was certified in e and is liquidatable in e. Certify 2: now no leg
            // is uncertified, only 0 is liquidation-pending. The advance must wait.
            w.refresh(2).unwrap();
            let a = w.asset();
            assert_eq!((a.band_uncertified_long, a.band_uncertified_short), (0, 0));
            assert_eq!(a.band_liq_pending_long, 1);
            w.now += 3;
            w.accrue(0, 0, 0).unwrap();
            assert_eq!(
                w.asset().band_epoch,
                e,
                "a liquidation-pending leg blocks the advance"
            );
            let out = w
                .with(0, |m, a| {
                    m.liquidate_account_not_atomic(a, LiquidationRequestV16 { asset_index: 0 })
                })
                .unwrap();
            assert_eq!(
                (out.insurance_used, out.residual_booked, out.explicit_loss),
                (0, 0, 0)
            );
            w.assert_census();
            isolated = true;
            break;
        }
        w.refresh(2).unwrap();
        w.now += 3;
        w.accrue(0, 0, 0).unwrap();
    }
    assert!(
        isolated,
        "non-vacuity: reached a certified-then-liquidatable leg in one epoch"
    );
    // Mutant M1 "re-anchor without the liq_pending check" advances here.
}

#[test]
fn band_pin_expired_declares_recovery_after_pmax() {
    let mut w = World::new(band_cfg(130), 2, 10_000_000);
    open_book(&mut w, 1, 5 * POS_SCALE);
    w.set_target(2 * P0);
    w.now = 4;
    w.accrue(0, 0, 0).unwrap();
    // Keeper gone: only no-move / edge accruals land. Walk the clock past Pmax.
    let mut guard = 0;
    while w.now < 4 + 600 + 9_000 + 10 {
        w.now += 3;
        w.accrue(0, 0, 0).unwrap();
        guard += 1;
        assert!(guard < 10_000);
    }
    let since = w.asset().band_pin_since_slot;
    assert!(since != 0 && w.now - since > 9_000);
    let now = w.now;
    let res = w
        .with(0, |m, a| {
            m.permissionless_auto_crank_not_atomic(
                a,
                AutoCrankWorkV16 {
                    now_slot: now,
                    observations: &[],
                    resolved_close_fee_rate_per_slot: 0,
                },
            )
        })
        .unwrap();
    assert_eq!(
        res.selected,
        AutoCrankPlanV16::DeclareRecovery {
            reason: PermissionlessRecoveryReasonV16::BandPinExpired
        }
    );
    assert_eq!(
        w.header.mode, 2,
        "market left Live through permissionless recovery"
    );
}

#[test]
fn band_auto_crank_refreshes_uncertified_leg_with_current_cert() {
    let mut w = World::new(band_cfg(130), 2, 10_000_000);
    open_book(&mut w, 1, 5 * POS_SCALE);
    // A no-move accrual re-anchors (all certified) without changing the price, so
    // the certificate stays current; the leg is still band-uncertified work.
    w.now = 2;
    w.accrue(0, 0, 0).unwrap();
    assert_eq!(w.asset().band_epoch, 2);
    let now = w.now;
    let res = w
        .with(0, |m, a| {
            m.permissionless_auto_crank_not_atomic(
                a,
                AutoCrankWorkV16 {
                    now_slot: now,
                    observations: &[],
                    resolved_close_fee_rate_per_slot: 0,
                },
            )
        })
        .unwrap();
    assert!(matches!(
        res.selected,
        AutoCrankPlanV16::RefreshAccount { .. }
    ));
    assert_eq!(
        w.accounts[0].legs[0]
            .try_to_runtime()
            .unwrap()
            .band_epoch_snap,
        2,
        "the crank certified the leg"
    );
    w.assert_census();
}

#[test]
fn band_off_keeps_every_band_field_zero_through_a_full_lifecycle() {
    // I-B7: on a band-off market nothing touches the band state.
    let mut w = World::new(band_cfg(0), 4, 1_000_000);
    w.trade(0, 1, 9 * POS_SCALE).unwrap();
    w.trade(2, 3, POS_SCALE).unwrap();
    w.set_target(P0 / 2);
    for _ in 0..50 {
        w.now += 3;
        w.accrue(0, 0, 0).unwrap();
        for i in 0..4 {
            if w.positioned(i) {
                // A band-off crash can bankrupt accounts (the very thing the band
                // prevents); failures are fine here, only band inertness is tested.
                if let Ok(c) = w.refresh(i) {
                    if c.certified_liq_deficit != 0 {
                        let _ = w.with(i, |m, a| {
                            m.liquidate_account_not_atomic(
                                a,
                                LiquidationRequestV16 { asset_index: 0 },
                            )
                        });
                    }
                }
            }
        }
        let a = w.asset();
        assert_eq!(
            (
                a.band_epoch,
                a.band_anchor_price,
                a.band_anchor_slot,
                a.band_pin_since_slot
            ),
            (0, 0, 0, 0)
        );
        assert_eq!(
            (
                a.band_uncertified_long,
                a.band_uncertified_short,
                a.band_liq_pending_long,
                a.band_liq_pending_short
            ),
            (0, 0, 0, 0)
        );
        for acct in &w.accounts {
            let leg = acct.legs[0].try_to_runtime().unwrap();
            assert_eq!((leg.band_epoch_snap, leg.band_liq_pending), (0, false));
        }
    }
}

/// Fail closed: a band-configured market whose asset was never armed (a wrapper that builds
/// genesis state without `band_initialize_asset`) must refuse to accrue rather than run as if
/// the band were off. Found by the BPF suite (the wrapper genesis path bypassed activation).
#[test]
fn band_unarmed_asset_on_a_band_market_fails_closed() {
    let mut w = World::new(band_cfg(130), 2, 10_000_000);
    let mut a = w.asset();
    a.band_epoch = 0;
    a.band_anchor_price = 0;
    a.band_anchor_slot = 0;
    w.markets[0].engine.asset = percolator::AssetStateV16Account::from_runtime(&a);
    w.now = 2;
    let r = w.market(|m| m.accrue_asset_to_not_atomic(0, 2, P0, 0, true));
    assert_eq!(r, Err(V16Error::InvalidConfig));
    // Control: the armed asset accrues.
    let mut w = World::new(band_cfg(130), 2, 10_000_000);
    w.now = 2;
    assert!(w
        .market(|m| m.accrue_asset_to_not_atomic(0, 2, P0, 0, true))
        .is_ok());
}

/// A rent move invalidates certificates like funding (the auto-crank then settles it).
#[test]
fn rent_move_invalidates_certificates_like_funding() {
    let mut w = World::new(band_cfg(130), 2, 10_000_000);
    open_book(&mut w, 1, 10 * POS_SCALE);
    let fe0 = w.header.funding_epoch.get();
    w.now += 3;
    w.accrue(0, 20, 0).unwrap();
    assert!(w.asset().rent_index_long_num > 0);
    assert!(
        w.header.funding_epoch.get() > fe0,
        "rent bumps the funding epoch"
    );
    // Control: a no-rent, no-funding accrual does not.
    let fe1 = w.header.funding_epoch.get();
    w.now += 3;
    w.accrue(0, 0, 0).unwrap();
    assert_eq!(w.header.funding_epoch.get(), fe1);
}

// ---------------------------------------------------------------------------
// Rent (item 2)
// ---------------------------------------------------------------------------

#[test]
fn rent_accrues_settles_floor_exact_routes_and_conserves() {
    let mut w = World::new(band_cfg(130), 3, 10_000_000);
    open_book(&mut w, 1, 10 * POS_SCALE);
    let cap0 = w.accounts[0].capital.get();
    let ins0 = w.header.insurance.get();
    // 100 slots of rent at 20 e9/slot on the long side only.
    let mut slots = 0u64;
    while slots < 99 {
        w.now += 3;
        slots += 3;
        w.accrue(0, 20, 0).unwrap();
    }
    let a = w.asset();
    let expect_index = P0 as u128 * 20 * slots as u128;
    assert_eq!(
        a.rent_index_long_num, expect_index,
        "index = price * rate * dt"
    );
    assert_eq!(
        a.rent_index_short_num, 0,
        "the short side pays nothing at rate 0"
    );
    w.refresh(0).unwrap();
    let due = (10 * POS_SCALE) * expect_index / RENT_INDEX_DEN;
    assert_eq!(cap0 - w.accounts[0].capital.get(), due, "floor-exact due");
    assert_eq!(w.header.insurance.get() - ins0, due);
    assert_eq!(w.asset().rent_unrouted_atoms, due);
    // I-R4: settling again in the same slot charges 0.
    let cap1 = w.accounts[0].capital.get();
    w.refresh(0).unwrap();
    assert_eq!(w.accounts[0].capital.get(), cap1);
    // I_free: the insurance surplus withdrawal cannot take the LP's unrouted rent.
    let r = w.market(|m| m.withdraw_insurance_surplus_not_atomic(1));
    assert_eq!(r, Err(V16Error::LockActive));
    // Routing to the LP (account 2) moves exactly the claim (I-R3).
    let lp_cap = w.accounts[2].capital.get();
    let x = w
        .with(2, |m, a| m.route_rent_to_account_not_atomic(0, a))
        .unwrap();
    assert_eq!(x, due);
    assert_eq!(w.accounts[2].capital.get() - lp_cap, due);
    assert_eq!(w.header.insurance.get(), ins0);
    assert_eq!(w.asset().rent_unrouted_atoms, 0);
    w.assert_conservation();
    // Mutants: "ceil instead of floor" breaks the due equality; "routing without
    // zeroing rent_unrouted" breaks the last-but-one assert.
}

/// A relabel of insurance into a reserved domain budget (the wrapper's fee-crediting paths
/// once did this to rent atoms) cannot erase the LP's claim: the route moves what is
/// unreserved now and keeps the rest as a claim, capped at the insurance physically there.
#[test]
fn rent_route_never_erases_the_claim_on_a_relabel() {
    let mut w = World::new(band_cfg(130), 3, 10_000_000);
    open_book(&mut w, 1, 10 * POS_SCALE);
    for _ in 0..33 {
        w.now += 3;
        w.accrue(0, 20, 0).unwrap();
    }
    w.refresh(0).unwrap();
    let claim = w.asset().rent_unrouted_atoms;
    assert!(claim > 0);
    // Relabel every unbudgeted insurance atom into domain 0's budget.
    let ins = w.header.insurance.get();
    w.market(|m| m.credit_domain_insurance_budget_not_atomic(0, ins))
        .unwrap();
    let x = w
        .with(2, |m, a| m.route_rent_to_account_not_atomic(0, a))
        .unwrap();
    assert_eq!(x, 0, "nothing unreserved to route");
    assert_eq!(
        w.asset().rent_unrouted_atoms,
        claim,
        "the claim survives the relabel"
    );
    w.assert_conservation();
}

#[test]
fn rent_new_leg_pays_nothing_for_prior_time() {
    let mut w = World::new(band_cfg(130), 4, 10_000_000);
    open_book(&mut w, 1, 10 * POS_SCALE);
    for _ in 0..20 {
        w.now += 3;
        w.accrue(0, 20, 20).unwrap();
    }
    // Certify the book so the late trade is not refused for staleness.
    for i in 0..2 {
        w.refresh(i).unwrap();
    }
    let idx = w.asset().rent_index_long_num;
    assert!(idx > 0);
    w.trade(2, 3, 10 * POS_SCALE).unwrap();
    let leg = w.accounts[2].legs[0].try_to_runtime().unwrap();
    assert_eq!(leg.rent_snap, idx, "I-R5: snapshot at attach");
    let cap = w.accounts[2].capital.get();
    w.refresh(2).unwrap();
    assert_eq!(
        w.accounts[2].capital.get(),
        cap,
        "no charge for time before attach"
    );
    // Mutant "snap not reset at attach" charges the whole prior index here.
}

#[test]
fn rent_hedged_lockout_pays_at_least_the_floor_bound() {
    // A delta-neutral both-side lock-out pays 2 * floor(N * P * rent * T) - 2 atoms.
    let mut w = World::new(band_cfg(130), 4, 100_000_000);
    let n = 50 * POS_SCALE;
    w.trade(0, 1, n).unwrap(); // lock long via 0
    w.trade(3, 2, n).unwrap(); // lock short via 2
    let caps: Vec<u128> = (0..4).map(|i| w.accounts[i].capital.get()).collect();
    let mut t = 0u64;
    while t < 300 {
        w.now += 3;
        t += 3;
        w.accrue(0, 23, 23).unwrap();
        for i in 0..4 {
            w.refresh(i).unwrap();
        }
    }
    let paid_long = caps[0] - w.accounts[0].capital.get();
    let paid_short = caps[2] - w.accounts[2].capital.get();
    let floor = n * P0 as u128 * 23 * t as u128 / RENT_INDEX_DEN;
    // With the sub-atom carry, 100 settles charge exactly what one settle would.
    assert_eq!((paid_long, paid_short), (floor, floor));
    // Mutant "drop the carry" pays 3 * 100 = 300 < 345 here.
}

#[test]
fn rent_is_junior_to_an_unsettled_loss() {
    let mut w = World::new(band_cfg(0), 2, 1_000_000);
    w.trade(0, 1, 9 * POS_SCALE).unwrap();
    // The long loses ~ all equity while rent accrues.
    w.set_target(P0 * 905 / 1000);
    for _ in 0..40 {
        w.now += 3;
        w.accrue(0, 23, 0).unwrap();
    }
    w.refresh(0).unwrap();
    let acct = &w.accounts[0];
    let pnl = acct.pnl.get();
    let cap = acct.capital.get();
    assert!(
        pnl >= 0 || cap >= pnl.unsigned_abs(),
        "rent never eats loss-owned capital"
    );
    w.assert_conservation();
}

// ---------------------------------------------------------------------------
// I-B5 under an adversarial mark and keeper: the malicious-mark simulation
// ---------------------------------------------------------------------------

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

#[derive(Default, Debug)]
struct Cover {
    liquidations: u64,
    reanchors: u64,
    pinned_accruals: u64,
    edge_pins: u64,
    opens: u64,
}

/// One adversarial run. The mark authority picks targets (jumps up to +-100%),
/// the keeper refreshes in a random order with random delays and may skip
/// liquidations for a while, traders open and close at random sizes near IM.
/// Asserts after every step: census (I-B2), conservation, no hlock, every
/// certificate has equity >= 0, every liquidation has D == 0 (I-B5).
fn adversarial_run(seed: u64, band_bps: u64, cover: &mut Cover) {
    let n = 6;
    let mut w = World::new(band_cfg(band_bps), n, 2_000_000);
    let mut rng = Rng(seed | 1);
    // Initial book near IM.
    let _ = w.trade(0, 1, 18 * POS_SCALE);
    let _ = w.trade(3, 2, 18 * POS_SCALE);
    for step in 0..300 {
        match rng.below(10) {
            0 | 1 => {
                // malicious mark: jump up to +-100% of the current target
                let t = w.asset().raw_oracle_target_price as u128;
                let pct = rng.below(200) as u128;
                let nt = (t * (pct + 1) / 100).clamp(1, MAX_ORACLE_PRICE as u128) as u64;
                w.set_target(nt);
            }
            2..=5 => {
                w.now += 1 + rng.below(30);
                let e = w.asset().band_epoch;
                let before = w.asset().effective_price;
                w.accrue(0, 0, 0)
                    .unwrap_or_else(|err| panic!("seed {seed} step {step}: accrue {err:?}"));
                let a = w.asset();
                if a.band_epoch > e {
                    cover.reanchors += 1;
                }
                if a.band_pin_since_slot != 0 {
                    if a.effective_price == before {
                        cover.pinned_accruals += 1;
                    } else {
                        cover.edge_pins += 1;
                    }
                }
            }
            6 | 7 => {
                // keeper: refresh a random subset (may leave someone uncertified)
                for i in 0..n {
                    if w.positioned(i) && rng.below(3) != 0 {
                        let snapshot = (w.header, w.markets.clone(), w.accounts.clone());
                        match w.refresh(i) {
                            Ok(cert) => {
                                assert!(cert.certified_equity >= 0, "seed {seed}: loss > capital")
                            }
                            Err(_) => (w.header, w.markets, w.accounts) = snapshot,
                        }
                    }
                }
            }
            8 => {
                // liquidator (may be slow): liquidate liquidatable accounts
                for i in 0..n {
                    let cert = w.accounts[i].health_cert.try_to_runtime().unwrap();
                    if w.positioned(i) && cert.valid && cert.certified_liq_deficit != 0 {
                        let snapshot = (w.header, w.markets.clone(), w.accounts.clone());
                        match w.with(i, |m, a| {
                            m.liquidate_account_not_atomic(
                                a,
                                LiquidationRequestV16 { asset_index: 0 },
                            )
                        }) {
                            Ok(out) => {
                                assert_eq!(
                                    (out.insurance_used, out.residual_booked, out.explicit_loss),
                                    (0, 0, 0),
                                    "seed {seed} step {step}: bad debt on a band market"
                                );
                                cover.liquidations += 1;
                            }
                            Err(_) => (w.header, w.markets, w.accounts) = snapshot,
                        }
                    }
                }
            }
            _ => {
                // trader: open or close at a random size (fallible: IM, staleness)
                let i = rng.below(n as u64) as usize;
                let mut j = rng.below(n as u64) as usize;
                if j == i {
                    j = (j + 1) % n;
                }
                let q = (1 + rng.below(18)) as u128 * POS_SCALE;
                let snapshot = (w.header, w.markets.clone(), w.accounts.clone());
                match w.trade(i, j, q) {
                    Ok(()) => cover.opens += 1,
                    Err(_) => (w.header, w.markets, w.accounts) = snapshot,
                }
            }
        }
        w.assert_census();
        w.assert_conservation();
        assert_eq!(
            w.header.bankruptcy_hlock_active, 0,
            "seed {seed} step {step}: hlock set"
        );
        let a = w.asset();
        let (lo, hi) = band_bounds(a.band_anchor_price, band_bps).unwrap();
        if a.stored_pos_count_long + a.stored_pos_count_short != 0 {
            assert!(
                lo <= a.effective_price && a.effective_price <= hi,
                "I-B1: P_last in band"
            );
        }
    }
}

/// The widest band the InitMarket validator accepts for the fixture tier.
fn widest_valid_band() -> u64 {
    (1..=MAX_BAND_BPS)
        .take_while(|&d| band_cfg(d).validate_public_user_fund().is_ok())
        .last()
        .unwrap()
}

#[test]
fn band_malicious_mark_never_creates_bad_debt() {
    let mut cover = Cover::default();
    // The design preset, and the widest band the BSL validator accepts: if the
    // law were too loose (mutant "G = 2d"), the wide run would find bad debt.
    let d_max = widest_valid_band();
    eprintln!("widest valid band for the 10x fixture: d = {d_max}");
    for seed in 1..=120u64 {
        let seed = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        adversarial_run(seed, 130, &mut cover);
        adversarial_run(seed ^ 0xA5A5, d_max, &mut cover);
    }
    eprintln!("cover: {cover:?}");
    // Kani-style cover properties: a run that never liquidates proves nothing.
    assert!(cover.liquidations > 0, "{cover:?}");
    assert!(cover.reanchors > 0, "{cover:?}");
    assert!(cover.pinned_accruals + cover.edge_pins > 0, "{cover:?}");
    assert!(cover.opens > 0, "{cover:?}");
}

// ---------------------------------------------------------------------------
// Liveness: catch-up after a 30% gap on a 10x market
// ---------------------------------------------------------------------------

fn catch_up(portfolios: usize, dust: usize) -> (u64, u64) {
    let n = portfolios + dust;
    let mut w = World::new(band_cfg(130), n, 10_000_000);
    for k in 0..portfolios / 2 {
        w.trade(2 * k, 2 * k + 1, 5 * POS_SCALE).unwrap();
    }
    // Dust Sybils: one-lot positions against account 0's counterparty side.
    for k in 0..dust / 2 {
        let a = portfolios + 2 * k;
        w.trade(a, a + 1, 1).unwrap();
    }
    let target = P0 * 13 / 10;
    w.set_target(target);
    let start = w.now;
    let mut sweeps = 0u64;
    while w.asset().effective_price != target {
        // One keeper sweep: accrue, then refresh every positioned portfolio.
        w.now += 3;
        w.accrue(0, 0, 0).unwrap();
        for i in 0..n {
            if w.positioned(i) {
                if let Err(e) = w.refresh(i) {
                    panic!(
                        "sweep {sweeps} refresh {i}: {e:?} asset {:?} leg {:?}",
                        w.asset(),
                        w.accounts[i].legs[0].try_to_runtime().unwrap()
                    );
                }
            }
        }
        sweeps += 1;
        assert!(sweeps < 10_000, "catch-up must terminate");
    }
    (w.asset().band_epoch, w.now - start)
}

#[test]
fn band_liveness_30pct_gap_10x_catch_up() {
    let (epochs, slots) = catch_up(4, 0);
    let (epochs_sybil, slots_sybil) = catch_up(4, 20);
    eprintln!(
        "30% gap @ d=130: {epochs} epochs / {slots} slots (4 portfolios); \
         {epochs_sybil} epochs / {slots_sybil} slots (+20 dust Sybils)"
    );
    // ln(1.3) / ln(1.013) ~ 20.3 epochs; the cap (100 bps/slot) is not binding.
    assert!((20..=26).contains(&epochs), "{epochs}");
    // Sybils add sweep WORK (transactions per sweep), not epochs: the epoch count
    // is the same, and each sweep refreshes more portfolios.
    assert_eq!(epochs, epochs_sybil);
}

/// v2.2 CU fast path: `PortfolioLegV16Account::try_to_runtime` recognises the
/// empty-leg encoding with one byte compare. The constant must be exactly the
/// encoding of `PortfolioLegV16::EMPTY`, and every other byte pattern must keep
/// the full decode (and its errors).
#[test]
fn v22_empty_leg_fast_path_is_exactly_the_empty_encoding() {
    use percolator::{
        PortfolioLegV16, PortfolioLegV16Account, V16Error, PORTFOLIO_LEG_V16_EMPTY_ACCOUNT,
    };
    let encoded = PortfolioLegV16Account::from_runtime(&PortfolioLegV16::EMPTY);
    assert_eq!(encoded, PORTFOLIO_LEG_V16_EMPTY_ACCOUNT);
    assert!(encoded.is_empty_encoding());
    assert_eq!(encoded.try_to_runtime().unwrap(), PortfolioLegV16::EMPTY);
    // Every single-byte perturbation leaves the fast path and decodes exactly as before:
    // an inactive leg with any stray byte is a hidden leg or an invalid encoding.
    let base = bytemuck::bytes_of(&PORTFOLIO_LEG_V16_EMPTY_ACCOUNT).to_vec();
    for i in 0..base.len() {
        for v in [1u8, 2, 0x80, 0xff] {
            if base[i] == v {
                continue;
            }
            let mut bytes = base.clone();
            bytes[i] = v;
            let leg: PortfolioLegV16Account = *bytemuck::from_bytes(&bytes);
            assert!(
                !leg.is_empty_encoding(),
                "byte {i}={v} must leave the fast path"
            );
            match leg.try_to_runtime() {
                Ok(out) => assert!(
                    out.active,
                    "byte {i}={v}: an Ok decode must be an active leg"
                ),
                Err(e) => assert!(
                    matches!(
                        e,
                        V16Error::HiddenLeg | V16Error::InvalidConfig | V16Error::InvalidLeg
                    ),
                    "byte {i}={v}: unexpected {e:?}"
                ),
            }
        }
    }
}

// ===========================================================================
// SENTINEL adversarial additions (review of PR #279). Tests named sec_*.
// ===========================================================================

fn sec_cfg(mmr: u64, fee: u64, fund: u64, rent: u64, e: u64, d: u64) -> V16Config {
    let mut c = band_cfg(d);
    c.maintenance_margin_bps = mmr;
    c.initial_margin_bps = 2 * mmr;
    c.liquidation_fee_bps = fee;
    c.max_abs_funding_e9_per_slot = fund;
    c.rent_max_e9_per_slot = rent;
    if d != 0 {
        c.band_max_epoch_slots = e;
        c.band_max_pin_slots = 20 * e;
    }
    c
}

/// widest d accepted by the BSL validator for this config shape, or None.
fn sec_widest(mmr: u64, fee: u64, fund: u64, rent: u64, e: u64) -> Option<u64> {
    let mut best = None;
    for d in 1..=MAX_BAND_BPS {
        if sec_cfg(mmr, fee, fund, rent, e, d)
            .validate_public_user_fund()
            .is_ok()
        {
            best = Some(d);
        } else if best.is_some() {
            break;
        }
    }
    best
}

#[derive(Default, Debug)]
struct SecStats {
    cfgs: u64,
    liqs: u64,
    min_slack: Option<i128>,
    reanchors: u64,
    errs: Vec<String>,
}

/// Directed worst-case adversary AT the BSL boundary: widest valid d, max funding and max
/// rent every non-pinned accrual, all accounts at ~max leverage, the keeper certifies the
/// victim LAST in each epoch and the liquidation is delayed to the end of the window.
fn sec_run(cfg: V16Config, seed: u64, stats: &mut SecStats) {
    let e = cfg.band_max_epoch_slots;
    let fund = cfg.max_abs_funding_e9_per_slot as i128;
    let rent = cfg.rent_max_e9_per_slot;
    let n = 8usize;
    let mut w = World::new(cfg, n, 2_000_000);
    let mut rng = Rng(seed | 1);
    let mut dir_down = rng.below(2) == 0;
    for epoch in 0..80u64 {
        // (re)open the book: pairs at near max leverage (q chosen so IM ~ capital).
        if epoch % 5 == 0 {
            for k in 0..n / 2 {
                let q = (15 + rng.below(5)) as u128 * POS_SCALE + rng.below(1_000_000) as u128;
                let (a, b) = if rng.below(2) == 0 {
                    (2 * k, 2 * k + 1)
                } else {
                    (2 * k + 1, 2 * k)
                };
                let snap = (w.header, w.markets.clone(), w.accounts.clone());
                if w.trade(a, b, q).is_err() {
                    (w.header, w.markets, w.accounts) = snap;
                }
            }
        }
        if rng.below(4) == 0 {
            dir_down = !dir_down;
        }
        let t = w.asset().raw_oracle_target_price;
        w.set_target(if dir_down {
            (t / 3).max(1)
        } else {
            t.saturating_mul(3).min(MAX_ORACLE_PRICE)
        });
        // accrue up to (just under) the window with adverse funding + max rent
        let steps = (e / 3).saturating_sub(1).max(1);
        for _ in 0..steps {
            w.now += 3;
            let f = if rng.below(2) == 0 { fund } else { -fund };
            let before = w.asset().band_epoch;
            if let Err(err) = w.accrue(f, rent, rent) {
                stats.errs.push(format!("accrue {err:?}"));
                return;
            }
            if w.asset().band_epoch > before {
                stats.reanchors += 1;
            }
        }
        // keeper: certify every positioned account, victims (the ones at/near MM) last
        let mut order: Vec<usize> = (0..n).filter(|&i| w.positioned(i)).collect();
        order.sort_by_key(|&i| {
            let c = w.accounts[i].health_cert.try_to_runtime().unwrap();
            std::cmp::Reverse(
                c.certified_maintenance_req
                    .saturating_sub(c.certified_equity.max(0) as u128),
            )
        });
        order.reverse();
        for i in order {
            let snap = (w.header, w.markets.clone(), w.accounts.clone());
            match w.refresh(i) {
                Ok(cert) => {
                    assert!(
                        cert.certified_equity >= 0,
                        "loss > capital, seed {seed} epoch {epoch}"
                    );
                    if cert.certified_liq_deficit != 0 {
                        // delayed liquidation, then assert D == 0 and equity >= fee
                        let snap2 = (w.header, w.markets.clone(), w.accounts.clone());
                        match w.with(i, |m, a| {
                            m.liquidate_account_not_atomic(
                                a,
                                LiquidationRequestV16 { asset_index: 0 },
                            )
                        }) {
                            Ok(out) => {
                                assert_eq!(
                                    (out.insurance_used, out.residual_booked, out.explicit_loss),
                                    (0, 0, 0),
                                    "BAD DEBT seed {seed} epoch {epoch}"
                                );
                                let slack = cert.certified_equity - out.fee_charged as i128;
                                stats.min_slack =
                                    Some(stats.min_slack.map_or(slack, |m| m.min(slack)));
                                assert!(
                                    slack >= 0,
                                    "BSL: equity {} < fee {} seed {seed}",
                                    cert.certified_equity,
                                    out.fee_charged
                                );
                                stats.liqs += 1;
                            }
                            Err(err) => {
                                stats.errs.push(format!("liq {err:?}"));
                                (w.header, w.markets, w.accounts) = snap2;
                            }
                        }
                    }
                }
                Err(err) => {
                    stats.errs.push(format!("refresh {err:?}"));
                    (w.header, w.markets, w.accounts) = snap;
                }
            }
        }
        w.assert_census();
        w.assert_conservation();
        assert_eq!(
            w.header.bankruptcy_hlock_active, 0,
            "hlock set seed {seed} epoch {epoch}"
        );
    }
}

#[test]
fn sec_bsl_boundary_adversary_with_max_funding_rent_and_delayed_liquidation() {
    let mut stats = SecStats::default();
    for &mmr in &[250u64, 500, 1000] {
        for &fee in &[10u64, 50, 100] {
            for &fund in &[0u64, 111, 2_000, 10_000] {
                for &rent in &[0u64, 23, 1_000, 10_000] {
                    // v2.2 review E-M2: E >= 150 (was 30 / 300 before the floor).
                    for &e in &[150u64, 300] {
                        let Some(d) = sec_widest(mmr, fee, fund, rent, e) else {
                            continue;
                        };
                        stats.cfgs += 1;
                        for seed in 1..=3u64 {
                            sec_run(
                                sec_cfg(mmr, fee, fund, rent, e, d),
                                seed.wrapping_mul(0x9E37_79B9_7F4A_7C15)
                                    ^ (mmr << 8)
                                    ^ fee
                                    ^ (fund << 20)
                                    ^ (rent << 32)
                                    ^ e,
                                &mut stats,
                            );
                        }
                    }
                }
            }
        }
    }
    let mut kinds: std::collections::BTreeMap<String, u64> = Default::default();
    for e in &stats.errs {
        *kinds.entry(e.clone()).or_default() += 1;
    }
    eprintln!(
        "SEC cfgs={} liqs={} min_slack={:?} reanchors={} errs={:?}",
        stats.cfgs, stats.liqs, stats.min_slack, stats.reanchors, kinds
    );
    assert!(stats.cfgs > 20 && stats.liqs > 50, "vacuous: {stats:?}");
}

/// Empty-leg fast path: EVERY single-byte mutation of the empty encoding must still be
/// rejected by the full decode (HiddenLeg / bad bool / bad side / invalid active leg).
#[test]
fn sec_empty_leg_fast_path_no_single_byte_bypass() {
    use percolator::{PortfolioLegV16, PortfolioLegV16Account, PORTFOLIO_LEG_V16_EMPTY_ACCOUNT};
    let base = bytemuck::bytes_of(&PORTFOLIO_LEG_V16_EMPTY_ACCOUNT).to_vec();
    assert_eq!(
        PortfolioLegV16Account::from_runtime(&PortfolioLegV16::EMPTY),
        PORTFOLIO_LEG_V16_EMPTY_ACCOUNT
    );
    assert_eq!(
        PORTFOLIO_LEG_V16_EMPTY_ACCOUNT.try_to_runtime(),
        Ok(PortfolioLegV16::EMPTY)
    );
    let mut n = 0u64;
    for i in 0..base.len() {
        for v in 0..=255u8 {
            if v == base[i] {
                continue;
            }
            let mut b = base.clone();
            b[i] = v;
            let leg: PortfolioLegV16Account = *bytemuck::from_bytes(&b);
            assert!(!leg.is_empty_encoding());
            let r = leg.try_to_runtime();
            assert!(r.is_err(), "byte {i}={v} decoded Ok: {r:?}");
            n += 1;
        }
    }
    // random multi-byte mutations: result is Ok only if it round-trips to a canonical leg
    let mut rng = Rng(0xDEAD_BEEF_1234_5678);
    for _ in 0..200_000 {
        let mut b = base.clone();
        for _ in 0..(1 + rng.below(4)) {
            let i = rng.below(b.len() as u64) as usize;
            b[i] = rng.below(256) as u8;
        }
        let leg: PortfolioLegV16Account = *bytemuck::from_bytes(&b);
        if let Ok(rt) = leg.try_to_runtime() {
            // accepted => canonical: re-encoding gives the identical bytes (no hidden state)
            assert_eq!(
                bytemuck::bytes_of(&PortfolioLegV16Account::from_runtime(&rt)),
                &b[..],
                "non-canonical leg accepted"
            );
        }
    }
    eprintln!("SEC empty-leg: {n} single-byte mutations all rejected");
}

/// Engine layer does NOT itself refuse risk-increasing trades while the asset is pinned
/// (the lag gate is the wrapper's). Documents the reliance (reported in the review).
#[test]
fn sec_engine_refuses_open_while_pinned_lag_gate_is_in_the_engine() {
    let mut w = World::new(band_cfg(130), 4, 10_000_000);
    w.trade(0, 1, 5 * POS_SCALE).unwrap();
    w.set_target(P0 * 2);
    w.now += 3;
    w.accrue(0, 0, 0).unwrap();
    // refresh nobody: the anchor cannot advance, price is pinned at hi(A)
    let a = w.asset();
    let (_, hi) = band_bounds(a.band_anchor_price, 130).unwrap();
    assert_eq!(a.effective_price, hi);
    assert!(a.band_pin_since_slot != 0);
    let r = w.trade(2, 3, 5 * POS_SCALE);
    eprintln!("SEC engine open while pinned (target 2x, P_last at hi): {r:?}");
}

/// The G-lunge (design §1.1 proof sketch's worst case). A blocker leg that is liq-pending
/// (underwater at the victim's favourable edge, never refreshed again) holds epoch e open
/// while the victim opens at lo(A_e) at the minimum initial margin (IM = MM + 1 bp); the
/// price runs lo(A_e) -> hi(A_e); the blocker is then certified, the anchor advances, and
/// the price runs on to hi(A_{e+1}) (3d), with max funding + rent throughout. The victim is
/// certified last. Must stay D == 0 and equity >= liquidation fee.
fn sec_lunge(
    mmr: u64,
    imr: u64,
    fee: u64,
    fund: u64,
    rent: u64,
    e: u64,
    d: u64,
    long_victim: bool,
) -> Result<Option<i128>, String> {
    let mut cfg = sec_cfg(mmr, fee, fund, rent, e, d);
    cfg.initial_margin_bps = imr;
    if cfg.validate_public_user_fund().is_err() {
        return Err("cfg".into());
    }
    let mut w = World::new(cfg, 4, 2_000_000);
    let max_q = |w: &mut World, l: usize, s: usize| -> u128 {
        let (mut qlo, mut qhi, mut best) = (1u128, 4_000u128 * POS_SCALE, 0u128);
        while qlo <= qhi {
            let q = (qlo + qhi) / 2;
            let snap = (w.header, w.markets.clone(), w.accounts.clone());
            let ok = w.trade(l, s, q).is_ok();
            (w.header, w.markets, w.accounts) = snap;
            if ok {
                best = q;
                qlo = q + 1
            } else {
                qhi = q - 1
            }
        }
        best
    };
    // blockers: 2 long / 3 short at P0, max size (IM = MM + 1 bp => underwater after a 1bp+ move)
    let qb = max_q(&mut w, 2, 3);
    if qb == 0 {
        return Err("qb".into());
    }
    w.trade(2, 3, qb)
        .map_err(|e| format!("open blockers {e:?}"))?;
    w.now += 3;
    w.accrue(0, 0, 0).map_err(|e| format!("reanchor {e:?}"))?;
    let a = w.asset();
    let (lo, hi) = band_bounds(a.band_anchor_price, d).unwrap();
    let (fav_edge, adv_edge, blocker, other) = if long_victim {
        (hi, lo, 3usize, 2usize)
    } else {
        (lo, hi, 2usize, 3usize)
    };
    // favourable edge, then keeper refreshes the blockers: `blocker` goes liq-pending
    w.set_target(fav_edge);
    for _ in 0..3 {
        w.now += 3;
        w.accrue(0, 0, 0).map_err(|e| format!("to fav {e:?}"))?;
    }
    if w.asset().effective_price != fav_edge {
        return Err("not at fav edge".into());
    }
    for i in [blocker, other] {
        w.refresh(i)
            .map_err(|e| format!("refresh blockers {e:?}"))?;
    }
    let pend = if blocker == 2 {
        w.asset().band_liq_pending_long
    } else {
        w.asset().band_liq_pending_short
    };
    if pend == 0 {
        return Err("blocker not liq-pending".into());
    }
    // victim opens at the favourable edge at the thinnest margin
    let (vl, vs) = if long_victim { (0, 1) } else { (1, 0) };
    let qv = max_q(&mut w, vl, vs);
    if qv == 0 {
        return Err("qv".into());
    }
    w.trade(vl, vs, qv)
        .map_err(|e| format!("open victim {e:?}"))?;
    // lunge to the adverse edge of band(A_e), blocker never refreshed meanwhile
    w.set_target(if long_victim {
        1
    } else {
        MAX_ORACLE_PRICE.min(a.band_anchor_price * 4)
    });
    let f = if long_victim {
        fund as i128
    } else {
        -(fund as i128)
    };
    let mut guard = 0;
    while w.asset().effective_price != adv_edge {
        w.now += 3;
        w.accrue(f, rent, rent)
            .map_err(|e| format!("lunge1 {e:?}"))?;
        guard += 1;
        if guard > 200 {
            return Err("no edge".into());
        }
    }
    let e1 = w.asset().band_epoch;
    // epoch e exhausted: certify blocker + other (everyone but the victim) => anchor may advance
    for i in [blocker, other] {
        if !w.positioned(i) {
            continue;
        }
        let cert = w.refresh(i).map_err(|e| format!("cert blockers {e:?}"))?;
        if cert.certified_liq_deficit != 0 {
            let out = w
                .with(i, |m, a| {
                    m.liquidate_account_not_atomic(a, LiquidationRequestV16 { asset_index: 0 })
                })
                .map_err(|e| format!("liq blocker {e:?}"))?;
            assert_eq!((out.insurance_used, out.residual_booked, out.explicit_loss), (0, 0, 0),
                "BAD DEBT liquidating the blocker mmr={mmr} fee={fee} fund={fund} rent={rent} e={e} d={d}");
            assert!(
                cert.certified_equity - out.fee_charged as i128 >= 0,
                "BSL violated on blocker"
            );
        }
    }
    // second leg: epoch e+1 (victim uncertified), run to hi(A_{e+1}) just before the window ends
    let steps = (e / 3).saturating_sub(2).max(2);
    for _ in 0..steps {
        w.now += 3;
        w.accrue(f, rent, rent)
            .map_err(|e| format!("lunge2 {e:?}"))?;
    }
    if w.asset().band_epoch <= e1 {
        let a = w.asset();
        if std::env::var("SEC_DBG").is_ok() {
            eprintln!(
                "DBG long={long_victim} unc=({},{}) lp=({},{}) pin={} snaps={:?}",
                a.band_uncertified_long,
                a.band_uncertified_short,
                a.band_liq_pending_long,
                a.band_liq_pending_short,
                a.band_pin_since_slot,
                (0..4)
                    .map(|i| {
                        let l = w.accounts[i].legs[0].try_to_runtime().unwrap();
                        (l.active, l.band_epoch_snap, l.band_liq_pending)
                    })
                    .collect::<Vec<_>>()
            );
        }
        return Err("anchor did not advance".into());
    }
    let cert = w
        .refresh(vl.min(vs).min(if long_victim { 0 } else { 0 }))
        .map_err(|e| format!("refresh victim {e:?}"))?;
    assert!(cert.certified_equity >= 0, "LOSS > CAPITAL in lunge mmr={mmr} imr={imr} fee={fee} fund={fund} rent={rent} e={e} d={d} long={long_victim}");
    let mut slack = None;
    if cert.certified_liq_deficit != 0 {
        let out = w
            .with(0, |m, a| {
                m.liquidate_account_not_atomic(a, LiquidationRequestV16 { asset_index: 0 })
            })
            .map_err(|e| format!("liq {e:?}"))?;
        assert_eq!((out.insurance_used, out.residual_booked, out.explicit_loss), (0, 0, 0),
            "BAD DEBT in lunge mmr={mmr} imr={imr} fee={fee} fund={fund} rent={rent} e={e} d={d} long={long_victim}");
        let sl = cert.certified_equity - out.fee_charged as i128;
        assert!(
            sl >= 0,
            "BSL violated: eq {} fee {}",
            cert.certified_equity,
            out.fee_charged
        );
        slack = Some(sl);
    }
    Ok(slack)
}

#[test]
fn sec_g_lunge_at_thinnest_initial_margin() {
    let (mut ran, mut liq, mut min_slack) = (0u64, 0u64, i128::MAX);
    let mut why: std::collections::BTreeMap<String, u64> = Default::default();
    for &mmr in &[250u64, 500, 1000] {
        for &fee in &[10u64, 50, 100] {
            for &fund in &[0u64, 2_000, 10_000] {
                for &rent in &[0u64, 1_000, 10_000] {
                    for &e in &[150u64, 300] {
                        let Some(d) = sec_widest(mmr, fee, fund, rent, e) else {
                            continue;
                        };
                        for &long_victim in &[true, false] {
                            match sec_lunge(mmr, mmr + 1, fee, fund, rent, e, d, long_victim) {
                                Ok(s) => {
                                    ran += 1;
                                    if let Some(s) = s {
                                        liq += 1;
                                        min_slack = min_slack.min(s);
                                    }
                                }
                                Err(r) => *why.entry(r).or_default() += 1,
                            }
                        }
                    }
                }
            }
        }
    }
    eprintln!("SEC lunge ran={ran} liquidated={liq} min_slack={min_slack} skipped={why:?}");
    assert!(ran > 20, "vacuous: ran={ran} {why:?}");
}

/// Free-option open at a pinned stale price. A liq-pending, settled blocker (long, underwater
/// at the pinned lo edge) holds the epoch; the true target is far below; everyone is settled so
/// the engine's LossStale gate is clear. Can a FRESH account open a SHORT at the stale-high price?
#[test]
fn sec_open_at_pinned_stale_price_with_settled_book() {
    let mut w = World::new(band_cfg(130), 6, 2_000_000);
    // blocker pair at max leverage (2 long, 3 short)
    let (mut qlo, mut qhi, mut best) = (1u128, 4_000u128 * POS_SCALE, 0u128);
    while qlo <= qhi {
        let q = (qlo + qhi) / 2;
        let snap = (w.header, w.markets.clone(), w.accounts.clone());
        let ok = w.trade(2, 3, q).is_ok();
        (w.header, w.markets, w.accounts) = snap;
        if ok {
            best = q;
            qlo = q + 1
        } else {
            qhi = q - 1
        }
    }
    w.trade(2, 3, best).unwrap();
    w.now += 3;
    w.accrue(0, 0, 0).unwrap(); // epoch opens, A = P0
                                // crash target far below: price walks to lo(A) and pins (blocker cannot be certified healthy)
    w.set_target(1);
    for _ in 0..3 {
        w.now += 3;
        w.accrue(0, 0, 0).unwrap();
    }
    let a = w.asset();
    let (lo, _) = band_bounds(a.band_anchor_price, 130).unwrap();
    assert_eq!(
        a.effective_price, lo,
        "pinned at lo(A) while the true target is ~0"
    );
    // everyone settles; blocker (long) turns liq-pending; shorts certify healthy
    for i in [2usize, 3] {
        w.refresh(i).unwrap();
    }
    let a = w.asset();
    assert!(a.band_liq_pending_long >= 1, "blocker is liq-pending");
    assert!(
        a.band_pin_since_slot != 0,
        "pin clock running; target {} vs P_last {}",
        a.raw_oracle_target_price,
        a.effective_price
    );
    // fresh account 0 (short) vs fresh account 1 (long) open AT THE STALE-HIGH price
    let r = w.trade(1, 0, 10 * POS_SCALE);
    eprintln!(
        "SEC OPEN AT PINNED STALE PRICE (target={}, P_last={}): {:?}",
        a.raw_oracle_target_price, a.effective_price, r
    );
    // control: same state but the target equals P_last (no lag): does the open pass?
    let mut w2 = World {
        header: w.header,
        markets: w.markets.clone(),
        accounts: w.accounts.clone(),
        now: w.now,
    };
    let p_last = w2.asset().effective_price;
    w2.set_target(p_last);
    let r2 = w2.trade(1, 0, 10 * POS_SCALE);
    eprintln!("SEC CONTROL same state, target==P_last: {:?}", r2);
}

/// Review E-L1 regression (kept from the review): the raw integer band does collapse
/// below ~77 ticks at d = 130. `band_never_zero_width` proves no anchor in that zone is
/// ever accepted for genesis, new exposure or a re-anchor.
#[test]
fn sec_band_collapses_at_tiny_prices() {
    // At d = 130 the integer band has zero width below ~77 ticks: the mark can never move.
    let mut dead = vec![];
    for a in 1..200u64 {
        let (lo, hi) = band_bounds(a, 130).unwrap();
        if lo == a && hi == a {
            dead.push(a);
        }
    }
    eprintln!(
        "SEC dead-band prices at d=130: {}..={} ({} values)",
        dead.first().unwrap(),
        dead.last().unwrap(),
        dead.len()
    );
    assert!(!dead.is_empty());
    // downward-only dead zone: lowest price reachable from a starting anchor by repeated lo()
    let mut p = 10_000u64;
    while band_bounds(p, 130).unwrap().0 < p {
        p = band_bounds(p, 130).unwrap().0;
    }
    eprintln!("SEC price floor reachable by walking down at d=130: {p}");
}
// ---------------------------------------------------------------------------
// Re-review of 30b2ec20: cap griefing, narrow-band trap
// ---------------------------------------------------------------------------

#[test]
fn sec2_dust_fills_the_position_cap_and_blocks_honest_opens() {
    let cap = percolator::band_rent::BAND_MAX_POSITIONS_PER_SIDE as usize;
    let n = 2 * cap + 2;
    let mut w = World::new(band_cfg(130), n, 2_000_000);
    for k in 0..cap {
        w.trade(2 * k, 2 * k + 1, 1)
            .unwrap_or_else(|e| panic!("dust pair {k}: {e:?}"));
    }
    let a = w.asset();
    assert_eq!(
        (a.stored_pos_count_long, a.stored_pos_count_short),
        (cap as u64, cap as u64)
    );
    // honest pair is refused on BOTH sides
    let snap = (w.header, w.markets.clone(), w.accounts.clone());
    let r = w.trade(2 * cap, 2 * cap + 1, 5 * POS_SCALE);
    eprintln!("SEC2 honest open with {cap} dust legs per side: {r:?}");
    assert_eq!(r, Err(V16Error::BandPositionCap));
    (w.header, w.markets, w.accounts) = snap;
    // an existing dust leg can still GROW (no attach), and a close frees a slot
    w.trade(0, 1, 1).expect("existing leg grows");
    w.trade(1, 0, 2)
        .expect("attacker closes pair 0 (both legs)");
    // one slot per side is free again: the honest pair now lands
    w.trade(2 * cap, 2 * cap + 1, 5 * POS_SCALE)
        .expect("slot freed");
    // sweep cost per epoch: every positioned leg must certify before the anchor advances
    w.now += 3;
    w.accrue(0, 0, 0).unwrap();
    let a = w.asset();
    eprintln!(
        "SEC2 uncertified after reanchor: long {} short {}",
        a.band_uncertified_long, a.band_uncertified_short
    );
    assert!(a.band_uncertified_long as usize >= cap && a.band_uncertified_short as usize >= cap);
}

#[test]
fn sec2_price_floor_trap_near_the_narrow_band_threshold() {
    let mut w = World::new(band_cfg(130), 2, 50_000_000);
    w.trade(0, 1, 5 * POS_SCALE).unwrap();
    w.set_target(1);
    let mut last = w.asset().effective_price;
    let mut still = 0;
    for _ in 0..20_000 {
        w.now += 3;
        w.accrue(0, 0, 0).unwrap();
        for i in 0..2 {
            if w.positioned(i) {
                let _ = w.refresh(i);
            }
        }
        let p = w.asset().effective_price;
        if p == last {
            still += 1
        } else {
            still = 0
        }
        last = p;
        if still > 400 {
            break;
        }
    }
    let a = w.asset();
    let (lo, hi) = band_bounds(a.band_anchor_price, 130).unwrap();
    eprintln!("SEC2 target=1: P_last={} anchor={} band=[{lo},{hi}] width={} epoch={} pin_since={} liq_pending=({},{}) positioned={:?}",
        a.effective_price, a.band_anchor_price, hi - lo, a.band_epoch, a.band_pin_since_slot,
        a.band_liq_pending_long, a.band_liq_pending_short, (w.positioned(0), w.positioned(1)));
    // the price stops where the next anchor would be narrower than MIN_BAND_WIDTH_TICKS
    assert!(
        a.effective_price > 1_000,
        "price froze far above the true target 1: {}",
        a.effective_price
    );
}

/// Re-review N-1 regression: with a minimum leg notional the cap can no longer be filled with
/// dust. A sub-floor open is refused (`BandLegBelowMinNotional`), a partial reduce that would
/// leave a sub-floor leg is refused while the full close lands, and filling the 256-per-side
/// cap now locks at least `2 * 256 * min_notional * IMR` of real margin.
#[test]
fn sec2_cap_grief() {
    const MIN: u64 = 100_000_000; // 100 whole tokens of a 6-decimal collateral
    let mut cfg = band_cfg(130);
    cfg.band_min_leg_notional = MIN;
    let cap = percolator::band_rent::BAND_MAX_POSITIONS_PER_SIDE;
    let mut w = World::new(cfg, 6, 2_000_000_000);
    // The reviewer's attack: one-atom pairs. Refused. (Engine `_not_atomic` calls may leave
    // partial state on Err; the wrapper's transaction reverts it, so restore the snapshot.)
    let before = (w.header, w.markets.clone(), w.accounts.clone());
    assert_eq!(w.trade(0, 1, 1), Err(V16Error::BandLegBelowMinNotional));
    (w.header, w.markets, w.accounts) = before.clone();
    assert_eq!(
        w.trade(0, 1, (MIN as u128 * POS_SCALE / P0 as u128) - 1),
        Err(V16Error::BandLegBelowMinNotional)
    );
    (w.header, w.markets, w.accounts) = before;
    // Exactly the floor is accepted.
    let q_min = MIN as u128 * POS_SCALE / P0 as u128;
    w.trade(0, 1, q_min).expect("a leg at the floor");
    w.trade(2, 3, 3 * q_min).expect("a bigger leg");
    // A partial reduce that would leave dust is refused; reducing to >= floor or to 0 lands.
    let snap = (w.header, w.markets.clone(), w.accounts.clone());
    assert_eq!(
        w.trade(3, 2, 3 * q_min - 1),
        Err(V16Error::BandLegBelowMinNotional)
    );
    (w.header, w.markets, w.accounts) = snap;
    w.trade(3, 2, q_min).expect("reduce to 2x the floor");
    w.trade(1, 0, q_min)
        .expect("full close of a floor-sized leg");
    assert!(!w.positioned(0) && !w.positioned(1));
    w.assert_census();
    // Cost of filling the cap: every slot-holding leg locks at least min * IMR.
    let imr = w.header.config.initial_margin_bps.get() as u128;
    let lower_bound = 2 * cap as u128 * MIN as u128 * imr / 10_000;
    eprintln!("SEC2 cap-fill now locks >= {lower_bound} atoms of margin (was ~0)");
    assert!(
        lower_bound >= 2 * cap as u128 * 10_000_000,
        "at least 10 whole tokens per slot"
    );
}

/// Re-review N-2: genesis rule. `band_min_wide_anchor` is the exact first anchor of a run of
/// width-ok anchors (checked against a brute-force scan), and genesis needs 100x it.
#[test]
fn band_genesis_price_rule() {
    use percolator::band_rent::{
        band_genesis_price_ok, band_min_wide_anchor, band_width_ok, BAND_GENESIS_FLOOR_MULTIPLE,
    };
    for d in [1u64, 7, 60, 100, 130, 145, 250, 1_000, MAX_BAND_BPS] {
        let min = band_min_wide_anchor(d).unwrap().unwrap();
        // brute force: every anchor from `min` for 10k ticks is width-ok, `min - 1` is not
        assert!(
            !band_width_ok(min - 1, d).unwrap(),
            "d={d}: {} is already wide",
            min - 1
        );
        for a in min..min + 10_000 {
            assert!(
                band_width_ok(a, d).unwrap(),
                "d={d}: {a} >= min {min} is narrow"
            );
        }
        let g = min * BAND_GENESIS_FLOOR_MULTIPLE;
        assert!(band_genesis_price_ok(g, d).unwrap());
        assert!(!band_genesis_price_ok(g - 1, d).unwrap());
    }
    assert_eq!(band_min_wide_anchor(130).unwrap(), Some(1_231));
    // The engine's own activation refuses a band genesis below the rule.
    let mut header =
        MarketGroupV16HeaderAccount::new_dynamic(MARKET_ID, band_cfg(130), 1, 0).unwrap();
    let mut markets = vec![Market::new(0, EngineAssetSlotV16Account::default())];
    assert_eq!(
        header.activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, 123_099, 1),
        Err(V16Error::BandTooNarrow)
    );
    assert!(header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, 123_100, 1)
        .is_ok());
}

/// Re-review N-1 config: a band market needs a nonzero minimum leg notional, an off-band
/// market encodes 0.
#[test]
fn band_config_min_leg_notional_shape() {
    let mut c = band_cfg(130);
    c.band_min_leg_notional = 0;
    assert!(c.validate_public_user_fund().is_err());
    let mut c = band_cfg(0);
    c.band_min_leg_notional = 1;
    assert!(c.validate_public_user_fund().is_err());
}
