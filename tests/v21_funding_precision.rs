//! fix/v21-funding-precision: the F index moves by the exact funding amount, sign-symmetric.
//!
//! Before: `funding_index_delta = floor_conservative(rate * dt * price / 1e9)` in whole price units,
//! then scaled by A. Per-slot accrual (the canonical path is one slot per step) therefore paid NO
//! positive funding on any asset whose `rate * price < 1e9` (every live devnet market under ~$9 at
//! the v2.1 cap) and charged a FULL price unit per slot for negative funding (up to ~1.8M x the
//! intended rate on the cheapest live market). These tests pin: long/short symmetry, sign
//! mirroring, the intended rate at low prices through real accruals and settlement, and a negative
//! control showing the same assertions reject the old formula.

use percolator::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, TradeRequestV16, V16Config, V16PodU64,
};
use percolator::{ADL_ONE, POS_SCALE};
use proptest::prelude::*;

const FUNDING_DEN: i128 = 1_000_000_000;

type Slab = (MarketGroupV16HeaderAccount, Vec<Market<u64>>);

fn market(price: u64, max_rate: u64) -> Slab {
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    cfg.max_abs_funding_e9_per_slot = max_rate;
    cfg.max_price_move_bps_per_slot = 100;
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut markets = vec![Market::new(0, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, price, 1)
        .unwrap();
    (header, markets)
}

fn account(seed: u32) -> PortfolioAccountV16Account {
    let mut key = [0u8; 32];
    key[..4].copy_from_slice(&seed.to_le_bytes());
    key[31] = 0x77;
    let h = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new([1; 32], key, [3; 32]));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(h).unwrap();
    a
}

fn equity(a: &PortfolioAccountV16Account) -> i128 {
    a.capital.get() as i128 + a.pnl.get()
}

/// One long and one short of `units` each at `price`; accrue `slots` one-slot segments at `rate`;
/// settle both; return (long equity change, short equity change, final F long, final F short).
fn run(price: u64, rate: i128, units: u128, slots: u64) -> (i128, i128, i128, i128) {
    let mut s = market(price, rate.unsigned_abs() as u64);
    let mut long = account(1);
    let mut short = account(2);
    {
        let mut m = MarketGroupV16ViewMut::new(&mut s.0, &mut s.1);
        m.deposit_not_atomic(&mut PortfolioV16ViewMut::new(&mut long), 1_000_000_000_000)
            .unwrap();
        m.deposit_not_atomic(&mut PortfolioV16ViewMut::new(&mut short), 1_000_000_000_000)
            .unwrap();
        m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut PortfolioV16ViewMut::new(&mut long),
            &mut PortfolioV16ViewMut::new(&mut short),
            TradeRequestV16 {
                asset_index: 0,
                size_q: i128::try_from(units * POS_SCALE).unwrap(),
                exec_price: price,
                fee_bps: 0,
            },
            true,
        )
        .unwrap();
    }
    let (l0, s0) = (equity(&long), equity(&short));
    for slot in 2..2 + slots {
        let mut m = MarketGroupV16ViewMut::new(&mut s.0, &mut s.1);
        m.accrue_asset_to_not_atomic(0, slot, price, rate, true).unwrap();
        m.markets[0].engine.asset.raw_oracle_target_price = V16PodU64::new(price);
    }
    {
        let mut m = MarketGroupV16ViewMut::new(&mut s.0, &mut s.1);
        m.full_account_refresh_not_atomic(&mut PortfolioV16ViewMut::new(&mut long))
            .unwrap();
        m.full_account_refresh_not_atomic(&mut PortfolioV16ViewMut::new(&mut short))
            .unwrap();
        m.validate_shape().unwrap();
    }
    let a = s.1[0].engine.asset.try_to_runtime().unwrap();
    (equity(&long) - l0, equity(&short) - s0, a.f_long_num, a.f_short_num)
}

/// Intended funding in quote atoms for `units` over `slots` at `rate`, `price`: units * rate *
/// price * slots / 1e9 (positive = longs pay).
fn intended(price: u64, rate: i128, units: u128, slots: u64) -> f64 {
    units as f64 * rate as f64 * price as f64 * slots as f64 / FUNDING_DEN as f64
}

/// The assertions a correct engine must pass for one scenario.
fn check(price: u64, rate: i128, units: u128, slots: u64, got: (i128, i128, i128, i128)) -> Result<(), String> {
    let (dl, ds, fl, fs) = got;
    let want = intended(price, rate, units, slots);
    // Equal and opposite index on a balanced book.
    if fl != -fs {
        return Err(format!("F not symmetric: {fl} vs {fs}"));
    }
    // Longs pay (rate > 0) or receive (rate < 0) the intended amount, within one atom of
    // settlement floor; shorts mirror; nobody receives more than the payer paid.
    if (dl as f64 + want).abs() > 1.0 || (ds as f64 - want).abs() > 1.0 {
        return Err(format!("paid/received {dl}/{ds}, intended {want:.3}"));
    }
    if dl + ds > 0 {
        return Err(format!("value created: {dl} + {ds}"));
    }
    Ok(())
}

/// The OLD per-slot formula (whole price units floored before A), for the negative control.
fn old_f_after(price: u64, rate: i128, slots: u64) -> (i128, i128) {
    let n = rate * price as i128;
    let q = n.div_euclid(FUNDING_DEN); // floor toward -inf == floor_div_signed_conservative
    let a = ADL_ONE as i128;
    (-(q * a) * slots as i128, q * a * slots as i128)
}

#[test]
fn v21_low_price_funding_pays_the_intended_rate_both_signs() {
    // Live devnet prices (e6) at the v2.1 cap (0.10%/h = 111e-9 per slot) and at 10% of it.
    for &price in &[5u64, 3_086, 9_738, 157_172, 18_327_119] {
        for &rate in &[111i128, -111, 11, -11] {
            let (units, slots) = (1_000u128, 9_000u64); // one hour
            let got = run(price, rate, units, slots);
            check(price, rate, units, slots, got)
                .unwrap_or_else(|e| panic!("price {price} rate {rate}: {e}"));
        }
    }
}

/// Long/short parity at v2.2 Wave A per-LOT prices (`lot_exp`: a lot of 10^k tokens, launch
/// floor $10/lot = 1e7 e6) and at the raw token prices those lots come from: the long pays what the
/// short receives (within the per-leg settlement floor), at the intended rate, for both signs.
#[test]
fn v21_funding_parity_with_lot_exp_prices() {
    // (token price e6, lot_exp): PENGU-like $0.0097 x 10^4, PUTIN-like $0.000005 x 10^7, $10 floor.
    for &(token_px, lot_exp) in &[(9_738u64, 4u32), (5, 7), (10_000_000, 0), (3_086, 4)] {
        let lot_px = token_px * 10u64.pow(lot_exp);
        for &px in &[token_px, lot_px] {
            for &rate in &[111i128, -111, 11, -11] {
                let (units, slots) = (1_000u128, 900u64);
                let got = run(px, rate, units, slots);
                check(px, rate, units, slots, got)
                    .unwrap_or_else(|e| panic!("price {px} (lot_exp {lot_exp}) rate {rate}: {e}"));
                // parity: |long paid| and |short received| differ by at most the two settlement floors
                assert!((got.0 + got.1).abs() <= 2, "parity at {px}: {} vs {}", got.0, got.1);
            }
        }
    }
}

#[test]
fn v21_funding_sign_mirrors_exactly() {
    for &price in &[3_086u64, 1_000_000] {
        let pos = run(price, 111, 1_000, 50);
        let neg = run(price, -111, 1_000, 50);
        assert_eq!((pos.0, pos.1), (neg.1, neg.0), "price {price}: long under +r == short under -r");
        assert_eq!((pos.2, pos.3), (neg.3, neg.2), "price {price}: F mirrors");
    }
}

/// Negative control: the same assertions REJECT the old floored formula at a low price, for both
/// signs (positive funding vanishes, negative funding is overcharged by ~3,000x).
#[test]
fn v21_negative_control_old_formula_fails_the_same_checks() {
    let (price, units, slots) = (3_086u64, 1_000u128, 9_000u64);
    for &rate in &[111i128, -111] {
        let (fl, fs) = old_f_after(price, rate, slots);
        // settle one long/short leg of `units` against the old F (a_basis = ADL_ONE)
        let settle = |f: i128| (units as i128 * POS_SCALE as i128 * f).div_euclid(ADL_ONE as i128 * POS_SCALE as i128);
        let got = (settle(fl), settle(fs), fl, fs);
        assert!(
            check(price, rate, units, slots, got).is_err(),
            "the checks must catch the old formula at rate {rate}: {got:?}"
        );
    }
    // and the new engine passes them on the same scenario
    for &rate in &[111i128, -111] {
        check(price, rate, units, slots, run(price, rate, units, slots)).unwrap();
    }
}

#[cfg(feature = "fuzz")]
proptest! {
    /// Kernel-level: sign mirroring, each side within one index unit of exact, payer never below
    /// receiver per unit of A (no value from rounding), for any rate, price and asymmetric A.
    #[test]
    fn v21_funding_index_deltas_symmetric_exact_and_conservative(
        rate in -1_000_000i128..=1_000_000,
        dt in 1u64..=20,
        price in 1u64..=1_000_000_000_000,
        a_long in 1_000_000u128..=ADL_ONE,
        a_short in 1_000_000u128..=ADL_ONE,
    ) {
        use percolator::kani_funding_index_deltas as k;
        let n = rate * dt as i128 * price as i128;
        let (fl, fs) = k(n, a_long, a_short).unwrap();
        let (ml, ms) = k(-n, a_short, a_long).unwrap();
        // mirror: flipping the sign and swapping A swaps the sides (long under +n == short under -n)
        prop_assert_eq!((fl, fs), (ms, ml));
        let exact = |a: u128| n.unsigned_abs() as f64 * a as f64 / FUNDING_DEN as f64;
        if n == 0 {
            prop_assert_eq!((fl, fs), (0, 0));
        } else {
            let (payer, pa, recv, ra) = if n > 0 { (-fl, a_long, fs, a_short) } else { (-fs, a_short, fl, a_long) };
            prop_assert!(payer >= 0 && recv >= 0);
            prop_assert!((payer as f64 - exact(pa)) < 1.0 + 1e-6 * exact(pa) && (payer as f64) >= exact(pa) - 1e-6 * exact(pa));
            prop_assert!((exact(ra) - recv as f64) < 1.0 + 1e-6 * exact(ra) && (recv as f64) <= exact(ra) + 1e-6 * exact(ra));
            // payer / a_payer >= recv / a_recv, exactly (cross-multiplied in u256-free i128 range)
            prop_assert!((payer as u128).checked_mul(ra).map_or(true, |l| (recv as u128).checked_mul(pa).map_or(false, |r| l >= r)));
            if a_long == a_short && a_long == ADL_ONE {
                prop_assert_eq!(payer, recv);
                prop_assert_eq!(payer as u128, n.unsigned_abs() * (ADL_ONE / FUNDING_DEN as u128));
            }
        }
    }
}
