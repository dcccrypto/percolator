//! Per-leg K/F settlement remainders (port of upstream a74b81b2,
//! `av/codex/fix-funding-crank-cadence-20260716`).
//!
//! Before: each settle floored `basis * dK / den` and `basis * dF / den` separately and dropped
//! the fraction, so settling a leg N times could cost it up to 2N atoms versus settling once
//! (never a gain): anyone could grind a victim by refreshing it every slot. Now each leg carries
//! `k_rem_num` / `f_rem_num` in `[0, a_basis * POS_SCALE)`, so the cumulative PnL and the final
//! remainder are the same for ANY partition of the same K/F interval.
//!
//! The accruals are identical in both arms; only the SETTLEMENT cadence differs. The account that
//! is refreshed at the varying cadence is the one LOSING value (the grief target: a floor can only
//! cost it); the winner is settled once at the end in both arms, because a winner's own repeated
//! refreshes also walk the source-backing bucket lifecycle (fresh -> expired after the funding
//! lifetime), which is a separate mechanism. `v22_winner_and_loser_..._inside_one_backing_window`
//! varies both sides' cadence inside one bucket lifetime.

use percolator::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, TradeRequestV16, V16Config, V16Error, V16PodU128, V16PodU64,
};
use percolator::{ADL_ONE, POS_SCALE};

type Slab = (MarketGroupV16HeaderAccount, Vec<Market<u64>>);

fn market(price: u64, max_rate: u64) -> Slab {
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    cfg.max_abs_funding_e9_per_slot = max_rate;
    cfg.max_accrual_dt_slots = 10;
    cfg.min_funding_lifetime_slots = 10;
    cfg.max_price_move_bps_per_slot = 1;
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
    key[31] = 0x3C;
    let h = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new([1; 32], key, [3; 32]));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(h).unwrap();
    a
}

fn equity(a: &PortfolioAccountV16Account) -> i128 {
    a.capital.get() as i128 + a.pnl.get()
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
struct Outcome {
    long_delta: i128,
    short_delta: i128,
    long_rem: (u128, u128),
    short_rem: (u128, u128),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Who {
    Long,
    Short,
    Both,
}

/// `steps`: per-slot (price, funding rate). `who` is settled after every `every`-th slot; both
/// legs are always settled after the last one (loser first).
fn run(basis_q: u128, price0: u64, steps: &[(u64, i128)], every: usize, who: Who) -> Outcome {
    let max_rate = steps.iter().map(|s| s.1.unsigned_abs() as u64).max().unwrap_or(0);
    let mut s = market(price0, max_rate);
    let mut long = account(1);
    let mut short = account(2);
    {
        let mut m = MarketGroupV16ViewMut::new(&mut s.0, &mut s.1);
        m.deposit_not_atomic(&mut PortfolioV16ViewMut::new(&mut long), 1_000_000_000_000).unwrap();
        m.deposit_not_atomic(&mut PortfolioV16ViewMut::new(&mut short), 1_000_000_000_000).unwrap();
        m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut PortfolioV16ViewMut::new(&mut long),
            &mut PortfolioV16ViewMut::new(&mut short),
            TradeRequestV16 { asset_index: 0, size_q: basis_q as i128, exec_price: price0, fee_bps: 0 },
            true,
        )
        .unwrap();
    }
    let (l0, s0) = (equity(&long), equity(&short));
    for (i, &(price, rate)) in steps.iter().enumerate() {
        let mut m = MarketGroupV16ViewMut::new(&mut s.0, &mut s.1);
        m.accrue_asset_to_not_atomic(0, 2 + i as u64, price, rate, true).unwrap();
        m.markets[0].engine.asset.raw_oracle_target_price = V16PodU64::new(price);
        let last = i + 1 == steps.len();
        if (i + 1) % every == 0 || last {
            // keeper's permissionless expiry crank: a lapsed source-backing bucket must be retired
            // before the domain can take new realized backing (refuses when nothing has lapsed)
            for domain in 0..2 {
                match m.expire_source_backing_bucket_not_atomic(domain, 2 + i as u64) {
                    // `Stale` is the crank's only refusal here: no Fresh bucket has lapsed
                    Ok(()) | Err(V16Error::Stale) => {}
                    Err(e) => panic!("expiry crank: {e:?}"),
                }
            }
            let (first, second) = if who == Who::Short { (&mut short, &mut long) } else { (&mut long, &mut short) };
            m.full_account_refresh_not_atomic(&mut PortfolioV16ViewMut::new(first)).unwrap();
            if last || who == Who::Both {
                m.full_account_refresh_not_atomic(&mut PortfolioV16ViewMut::new(second)).unwrap();
            }
        }
    }
    {
        let m = MarketGroupV16ViewMut::new(&mut s.0, &mut s.1);
        m.validate_shape().unwrap();
        PortfolioV16ViewMut::new(&mut long).validate_with_market(&m.as_view()).unwrap();
        PortfolioV16ViewMut::new(&mut short).validate_with_market(&m.as_view()).unwrap();
    }
    let (ll, sl) = (long.legs[0].try_to_runtime().unwrap(), short.legs[0].try_to_runtime().unwrap());
    Outcome {
        long_delta: equity(&long) - l0,
        short_delta: equity(&short) - s0,
        long_rem: (ll.k_rem_num, ll.f_rem_num),
        short_rem: (sl.k_rem_num, sl.f_rem_num),
    }
}

/// The OLD rule for one side: sum of per-settle floors with no carry.
fn old_fragmented(basis_q: u128, deltas: &[i128]) -> i128 {
    let den = (ADL_ONE * POS_SCALE) as i128;
    deltas.iter().map(|d| (basis_q as i128 * d).div_euclid(den)).sum()
}

#[test]
fn v22_fractional_funding_settles_the_same_for_any_refresh_cadence() {
    // 7.000001 units at price 0.003086, v2.1 cap: 0.0024 atoms of funding per slot.
    let (basis, price, n) = (7_000_001u128, 3_086u64, 500usize);
    for &rate in &[111i128, -111] {
        let payer = if rate > 0 { Who::Long } else { Who::Short };
        let steps: Vec<(u64, i128)> = vec![(price, rate); n];
        let once = run(basis, price, &steps, n, payer);
        for every in [1usize, 2, 3, 7, 50, 499] {
            assert_eq!(run(basis, price, &steps, every, payer), once, "rate {rate}, settle every {every}");
        }
        // exact total: 7.000001 * 111 * 3086 * 500 / 1e9 = 1.1989.. atoms; payer pays 2, receiver gets 1
        let exact = basis as f64 / 1e6 * 111.0 * price as f64 * n as f64 / 1e9;
        let (paid, recv) = if rate > 0 { (once.long_delta, once.short_delta) } else { (once.short_delta, once.long_delta) };
        assert_eq!(paid, -(exact.ceil() as i128));
        assert_eq!(recv, exact.floor() as i128);
        // the pair never gains, and loses less than one atom in TOTAL, whatever the cadence
        assert!(paid + recv <= 0 && paid + recv >= -1);
    }
}

#[test]
fn v22_fractional_price_pnl_settles_the_same_for_any_refresh_cadence() {
    // 0.001337 units; the price walks by 1..=7 e6 units per slot, so every step's K pnl is a
    // fraction of an atom. Falling price: the long is the loser; rising: the short.
    let (basis, price0, n) = (1_337u128, 1_000_000u64, 200usize);
    for (dir, loser) in [(-1i64, Who::Long), (1, Who::Short)] {
        let mut p = price0 as i64;
        let steps: Vec<(u64, i128)> = (0..n)
            .map(|i| {
                p += dir * ((i as i64 % 7) + 1);
                (p as u64, 0i128)
            })
            .collect();
        let once = run(basis, price0, &steps, n, loser);
        for every in [1usize, 2, 5, 13, 199] {
            assert_eq!(run(basis, price0, &steps, every, loser), once, "dir {dir}, settle every {every}");
        }
        let sum = once.long_delta + once.short_delta;
        assert!(sum <= 0 && sum >= -1, "pair total {sum}");
        assert_ne!(once.long_delta, 0);
    }
}

/// Both legs settled at the varying cadence, inside one source-backing window (10 slots here).
#[test]
fn v22_winner_and_loser_settle_the_same_inside_one_backing_window() {
    let (basis, price, n) = (1_000_000_001u128, 3_086u64, 10usize);
    for &rate in &[111i128, -111] {
        let steps: Vec<(u64, i128)> = vec![(price, rate); n];
        let once = run(basis, price, &steps, n, Who::Both);
        for every in [1usize, 2, 3, 9] {
            assert_eq!(run(basis, price, &steps, every, Who::Both), once, "rate {rate}, every {every}");
        }
        assert_eq!(once.long_delta + once.short_delta, -1);
    }
}

/// Negative control: the old per-settle floors ARE cadence-dependent on these exact numbers, and
/// the engine now matches the single-settle value, not the old fragmented sum.
#[test]
fn v22_negative_control_old_per_settle_floor_was_cadence_dependent() {
    let (basis, price, n, rate) = (7_000_001u128, 3_086u64, 500usize, 111i128);
    // per-slot F delta on the long side, in index units (exact funding, A = ADL_ONE)
    let d = -(rate * price as i128 * (ADL_ONE / 1_000_000_000) as i128);
    let den = (ADL_ONE * POS_SCALE) as i128;
    let single = (basis as i128 * d * n as i128).div_euclid(den);
    let fragmented_old = old_fragmented(basis, &vec![d; n]);
    assert_eq!(single, -2);
    assert_eq!(fragmented_old, -(n as i128), "old rule: one atom lost on EVERY settle");
    let steps: Vec<(u64, i128)> = vec![(price, rate); n];
    assert_eq!(run(basis, price, &steps, 1, Who::Long).long_delta, single, "engine == single settle");
}

#[test]
fn v22_leg_remainder_must_stay_below_its_denominator() {
    let mut s = market(1_000_000, 0);
    let mut long = account(1);
    let mut short = account(2);
    let mut m = MarketGroupV16ViewMut::new(&mut s.0, &mut s.1);
    m.deposit_not_atomic(&mut PortfolioV16ViewMut::new(&mut long), 1_000_000_000).unwrap();
    m.deposit_not_atomic(&mut PortfolioV16ViewMut::new(&mut short), 1_000_000_000).unwrap();
    m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
        &mut PortfolioV16ViewMut::new(&mut long),
        &mut PortfolioV16ViewMut::new(&mut short),
        TradeRequestV16 { asset_index: 0, size_q: POS_SCALE as i128, exec_price: 1_000_000, fee_bps: 0 },
        true,
    )
    .unwrap();
    PortfolioV16ViewMut::new(&mut long).validate_with_market(&m.as_view()).unwrap();
    let den = ADL_ONE * POS_SCALE;
    long.legs[0].k_rem_num = V16PodU128::new(den - 1);
    PortfolioV16ViewMut::new(&mut long).validate_with_market(&m.as_view()).unwrap();
    long.legs[0].k_rem_num = V16PodU128::new(den);
    assert_eq!(
        PortfolioV16ViewMut::new(&mut long).validate_with_market(&m.as_view()),
        Err(V16Error::InvalidLeg)
    );
    long.legs[0].k_rem_num = V16PodU128::new(0);
    long.legs[0].f_rem_num = V16PodU128::new(den);
    assert_eq!(
        PortfolioV16ViewMut::new(&mut long).validate_with_market(&m.as_view()),
        Err(V16Error::InvalidLeg)
    );
}

/// Layout pin (ledger/v22-allocations.md): the remainders sit right after `f_snap`.
#[test]
fn v22_leg_layout_is_pinned() {
    use core::mem::{offset_of, size_of};
    use percolator::{PortfolioLegV16Account, V16_LAYOUT_DISCRIMINATOR};
    assert_eq!(offset_of!(PortfolioLegV16Account, f_snap), 62);
    assert_eq!(offset_of!(PortfolioLegV16Account, k_rem_num), 78);
    assert_eq!(offset_of!(PortfolioLegV16Account, f_rem_num), 94);
    assert_eq!(offset_of!(PortfolioLegV16Account, kf_epoch_snap), 110);
    assert_eq!(size_of::<PortfolioLegV16Account>(), 184);
    assert_eq!(size_of::<PortfolioAccountV16Account>(), 9931);
    assert_eq!(V16_LAYOUT_DISCRIMINATOR, 20);
}
