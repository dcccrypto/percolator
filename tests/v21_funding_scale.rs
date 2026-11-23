//! fix/v21-funding-scale: a K/F cohort no longer forces every positioned account to be
//! refreshed before a risk-increasing trade, PROVIDED the worst-case loss the still-stale
//! legs could recognize is covered by the insurance that would absorb it.
//!
//! Baseline (Toly 92ed4a1a, spec.md 4 "Risk-increasing trades remain blocked until both
//! affected side cohorts are empty"): any K or F change resets `stale_account_count` to the
//! side's stored-position count and every risk-increasing trade on the asset fails until all
//! of them settle. With funding > 0 every accrual is a change, so an opening order needs a
//! refresh of EVERY positioned account in its own slot -- O(N) CU in one transaction.
//!
//! These tests pin: the exact cover boundary, the fallbacks (no cover, unaccrued asset,
//! price travel beyond cover), that reductions never needed cover, that the bound is sound
//! against the real settlement, and that insurance cannot be withdrawn out from under it.

use percolator::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, TradeRequestV16, V16Config, V16Error, V16PodU64,
};
use percolator::{POS_SCALE, SOCIAL_WEIGHT_SCALE};

const PRICE: u64 = 1_000_000;
const RATE_E9: i128 = 10_000; // 10 price units of funding per slot at PRICE
const TRADER_CAPITAL: u128 = 1_000_000_000;
const MAKER_CAPITAL: u128 = 1_000_000_000_000;
const LONG_DOMAIN: usize = 0;
const SHORT_DOMAIN: usize = 1;

type Slab = (MarketGroupV16HeaderAccount, Vec<Market<u64>>);

fn market(max_price_move_bps_per_slot: u64) -> Slab {
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    cfg.max_abs_funding_e9_per_slot = RATE_E9 as u64;
    cfg.max_price_move_bps_per_slot = max_price_move_bps_per_slot;
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut markets = vec![Market::new(0, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, PRICE, 1)
        .unwrap();
    (header, markets)
}

fn account(seed: u32) -> PortfolioAccountV16Account {
    let mut key = [0u8; 32];
    key[..4].copy_from_slice(&seed.to_le_bytes());
    key[31] = 0xA5;
    let header =
        ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new([1; 32], key, [3; 32]));
    let mut account = PortfolioAccountV16Account::default();
    account.init_empty_in_place(header).unwrap();
    account
}

fn deposit(slab: &mut Slab, acct: &mut PortfolioAccountV16Account, amount: u128) {
    let mut m = MarketGroupV16ViewMut::new(&mut slab.0, &mut slab.1);
    let mut a = PortfolioV16ViewMut::new(acct);
    m.deposit_not_atomic(&mut a, amount).unwrap();
}

/// Positive `size` opens/extends `long` long against `short`.
fn trade(
    slab: &mut Slab,
    long: &mut PortfolioAccountV16Account,
    short: &mut PortfolioAccountV16Account,
    size: u128,
    exec_price: u64,
) -> Result<(), V16Error> {
    let mut m = MarketGroupV16ViewMut::new(&mut slab.0, &mut slab.1);
    let mut l = PortfolioV16ViewMut::new(long);
    let mut s = PortfolioV16ViewMut::new(short);
    m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
        &mut l,
        &mut s,
        TradeRequestV16 {
            asset_index: 0,
            size_q: i128::try_from(size).unwrap(),
            exec_price,
            fee_bps: 0,
        },
        true,
    )
    .map(|_| ())
}

fn accrue(slab: &mut Slab, slot: u64, price: u64, rate: i128) {
    let mut m = MarketGroupV16ViewMut::new(&mut slab.0, &mut slab.1);
    m.accrue_asset_to_not_atomic(0, slot, price, rate, true).unwrap();
    m.markets[0].engine.asset.raw_oracle_target_price = V16PodU64::new(price);
}

fn refresh(slab: &mut Slab, acct: &mut PortfolioAccountV16Account) {
    let mut m = MarketGroupV16ViewMut::new(&mut slab.0, &mut slab.1);
    let mut a = PortfolioV16ViewMut::new(acct);
    m.full_account_refresh_not_atomic(&mut a).unwrap();
}

fn insure(slab: &mut Slab, domain: usize, amount: u128) {
    let mut m = MarketGroupV16ViewMut::new(&mut slab.0, &mut slab.1);
    m.deposit_domain_insurance_not_atomic(domain, amount).unwrap();
}

fn asset(slab: &Slab) -> percolator::AssetStateV16 {
    slab.1[0].engine.asset.try_to_runtime().unwrap()
}

fn drift_long(slab: &Slab) -> percolator::KfDriftSideV16 {
    slab.1[0].engine.kf_drift_long.to_runtime()
}

fn validate(slab: &mut Slab, accounts: &mut [&mut PortfolioAccountV16Account]) {
    let m = MarketGroupV16ViewMut::new(&mut slab.0, &mut slab.1);
    m.validate_shape().unwrap();
    for a in accounts.iter_mut() {
        PortfolioV16ViewMut::new(a).validate_with_market(&m.as_view()).unwrap();
    }
}

/// `n` traders each long one unit against one maker short; then a funding accrual opens a
/// cohort on both sides.
fn funded_cohort(n: usize) -> (Slab, PortfolioAccountV16Account, Vec<PortfolioAccountV16Account>) {
    let mut slab = market(9_000);
    let mut maker = account(0);
    deposit(&mut slab, &mut maker, MAKER_CAPITAL);
    let mut traders = Vec::with_capacity(n);
    for i in 0..n {
        let mut t = account(1 + i as u32);
        deposit(&mut slab, &mut t, TRADER_CAPITAL);
        trade(&mut slab, &mut t, &mut maker, POS_SCALE, PRICE).unwrap();
        traders.push(t);
    }
    accrue(&mut slab, 2, PRICE, RATE_E9);
    let a = asset(&slab);
    assert_eq!(a.stale_account_count_long, n as u64, "funding re-stales every long");
    assert_eq!(a.stale_account_count_short, 1, "and the maker");
    (slab, maker, traders)
}

/// Each long leg owes 10 atoms of funding (basis 1e6 * dF 10*ADL_ONE / (ADL_ONE * POS_SCALE)).
/// The bound is ceil(W * drift / (SWS * POS_SCALE)) + 2 per stale leg.
fn expected_long_bound(n: u128) -> u128 {
    let weight = n * POS_SCALE; // loss_weight = basis * SWS / a_basis, a_basis = ADL_ONE = SWS
    let drift = 10 * SOCIAL_WEIGHT_SCALE; // funding_index_delta (10) scaled by a_long (ADL_ONE)
    (weight * drift).div_ceil(SOCIAL_WEIGHT_SCALE * POS_SCALE) + 2 * n
}

#[test]
fn v21_baseline_cohort_blocks_entrant_without_cover() {
    let (mut slab, mut maker, mut traders) = funded_cohort(60);
    let mut entrant = account(10_000);
    deposit(&mut slab, &mut entrant, TRADER_CAPITAL);
    assert_eq!(
        trade(&mut slab, &mut entrant, &mut maker, POS_SCALE, PRICE),
        Err(V16Error::LossStale),
        "no insurance: the baseline all-accounts gate still applies"
    );
    let mut all: Vec<&mut PortfolioAccountV16Account> = traders.iter_mut().collect();
    all.push(&mut maker);
    all.push(&mut entrant);
    validate(&mut slab, &mut all);
}

#[test]
fn v21_insurance_cover_admits_entrant_at_exact_boundary_without_refreshing_60_accounts() {
    let n = 60u128;
    let bound = expected_long_bound(n);
    assert_eq!(bound, 720);

    // One atom short of the bound: refused.
    let (mut slab, mut maker, _traders) = funded_cohort(n as usize);
    insure(&mut slab, SHORT_DOMAIN, bound - 1);
    let mut entrant = account(10_000);
    deposit(&mut slab, &mut entrant, TRADER_CAPITAL);
    assert_eq!(
        trade(&mut slab, &mut entrant, &mut maker, POS_SCALE, PRICE),
        Err(V16Error::LossStale)
    );

    // Exactly the bound: admitted, and nobody else had to settle.
    let (mut slab, mut maker, mut traders) = funded_cohort(n as usize);
    insure(&mut slab, SHORT_DOMAIN, bound);
    let mut entrant = account(10_000);
    deposit(&mut slab, &mut entrant, TRADER_CAPITAL);
    trade(&mut slab, &mut entrant, &mut maker, POS_SCALE, PRICE)
        .expect("hidden K/F loss fully insured: the entrant opens without a global refresh");
    let a = asset(&slab);
    assert_eq!(a.stale_account_count_long, n as u64, "the 60 longs are still stale");
    assert_eq!(a.stale_account_count_short, 0, "the maker was settled by its own trade");
    assert_eq!(a.stored_pos_count_long, n as u64 + 1);

    // The bound is sound: settling the cohort recognizes exactly 10 atoms per long.
    let mut recognized = 0u128;
    for t in traders.iter_mut() {
        let before = t.capital.get() as i128 + t.pnl.get();
        refresh(&mut slab, t);
        let after = t.capital.get() as i128 + t.pnl.get();
        recognized += (before - after) as u128;
    }
    assert_eq!(recognized, 600);
    assert!(recognized <= bound);
    let a = asset(&slab);
    assert_eq!(a.stale_account_count_long, 0);
    assert_eq!(drift_long(&slab).laggard_count, 0);
    let mut all: Vec<&mut PortfolioAccountV16Account> = traders.iter_mut().collect();
    all.push(&mut maker);
    all.push(&mut entrant);
    validate(&mut slab, &mut all);
}

#[test]
fn v21_cover_never_relaxes_the_unaccrued_clause() {
    let (mut slab, mut maker, _traders) = funded_cohort(8);
    insure(&mut slab, SHORT_DOMAIN, 1_000_000);
    insure(&mut slab, LONG_DOMAIN, 1_000_000);
    // The market clock moves on but the asset has not accrued to it: index travel since
    // `slot_last` is unknown, so no cover can be computed.
    slab.0.current_slot = V16PodU64::new(3);
    let mut entrant = account(10_000);
    deposit(&mut slab, &mut entrant, TRADER_CAPITAL);
    assert_eq!(
        trade(&mut slab, &mut entrant, &mut maker, POS_SCALE, PRICE),
        Err(V16Error::LossStale)
    );
    // Accrue to the clock and it opens.
    accrue(&mut slab, 3, PRICE, RATE_E9);
    trade(&mut slab, &mut entrant, &mut maker, POS_SCALE, PRICE).unwrap();
}

#[test]
fn v21_price_travel_beyond_cover_falls_back_to_baseline() {
    let (mut slab, mut maker, mut traders) = funded_cohort(20);
    // Funding-only drift is covered...
    insure(&mut slab, SHORT_DOMAIN, 1_000);
    insure(&mut slab, LONG_DOMAIN, 1_000);
    // ...but a 5% price drop adds 50_000 atoms of potential loss per long.
    accrue(&mut slab, 3, PRICE - PRICE / 20, RATE_E9);
    let mut entrant = account(10_000);
    deposit(&mut slab, &mut entrant, TRADER_CAPITAL);
    assert_eq!(
        trade(&mut slab, &mut entrant, &mut maker, POS_SCALE, PRICE - PRICE / 20),
        Err(V16Error::LossStale)
    );
    // Settle everyone (the baseline path) and the same order lands.
    for t in traders.iter_mut() {
        refresh(&mut slab, t);
    }
    trade(&mut slab, &mut entrant, &mut maker, POS_SCALE, PRICE - PRICE / 20).unwrap();
}

#[test]
fn v21_reductions_never_needed_cover() {
    let (mut slab, mut maker, mut traders) = funded_cohort(10);
    // No insurance at all: a long closing half its position is risk-reducing for both.
    let t = &mut traders[3];
    {
        let mut m = MarketGroupV16ViewMut::new(&mut slab.0, &mut slab.1);
        let mut s = PortfolioV16ViewMut::new(&mut maker);
        let mut l = PortfolioV16ViewMut::new(t);
        m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut s,
            &mut l,
            TradeRequestV16 {
                asset_index: 0,
                size_q: i128::try_from(POS_SCALE / 2).unwrap(),
                exec_price: PRICE,
                fee_bps: 0,
            },
            true,
        )
        .expect("risk-reducing trades were never gated on loss-stale");
    }
}

#[test]
fn v21_insurance_withdrawal_cannot_dip_into_hidden_loss_cover() {
    let (mut slab, mut maker, mut traders) = funded_cohort(60);
    let bound = expected_long_bound(60);
    insure(&mut slab, SHORT_DOMAIN, bound + 100);
    {
        let mut m = MarketGroupV16ViewMut::new(&mut slab.0, &mut slab.1);
        assert_eq!(m.domain_insurance_withdraw_capacity(SHORT_DOMAIN).unwrap(), 100);
        assert_eq!(
            m.withdraw_domain_insurance_not_atomic(SHORT_DOMAIN, 101),
            Err(V16Error::LockActive)
        );
        m.withdraw_domain_insurance_not_atomic(SHORT_DOMAIN, 100).unwrap();
    }
    // Once the cohort settles the reservation is released.
    refresh(&mut slab, &mut maker);
    for t in traders.iter_mut() {
        refresh(&mut slab, t);
    }
    let m = MarketGroupV16ViewMut::new(&mut slab.0, &mut slab.1);
    assert_eq!(m.domain_insurance_withdraw_capacity(SHORT_DOMAIN).unwrap(), bound);
}

#[test]
fn v21_generation_rotation_tracks_laggards_across_repeated_funding_accruals() {
    // Ten slots of funding with a keeper that sweeps a few accounts per slot: the bound
    // must cover the oldest snapshot, and rotate once every account has moved past the
    // generation start.
    let n = 12usize;
    let (mut slab, mut maker, mut traders) = funded_cohort(n);
    let mut next = 0usize;
    for slot in 3..=12u64 {
        accrue(&mut slab, slot, PRICE, RATE_E9);
        for _ in 0..3 {
            refresh(&mut slab, &mut traders[next % n]);
            next += 1;
        }
        let a = asset(&slab);
        let d = drift_long(&slab);
        assert!(d.laggard_count <= a.stale_account_count_long);
        assert!(d.gen_epoch <= a.kf_epoch_long);
    }
    // Real hidden loss = sum over stale longs of 10 atoms per slot since its snapshot.
    let mut hidden = 0u128;
    let a = asset(&slab);
    for t in traders.iter() {
        let leg = t.legs[0].try_to_runtime().unwrap();
        if leg.kf_epoch_snap < a.kf_epoch_long {
            hidden += 10 * u128::from(a.slot_last - leg.kf_epoch_snap);
        }
    }
    let d = drift_long(&slab);
    let den = SOCIAL_WEIGHT_SCALE * POS_SCALE;
    let prior_term = if d.laggard_count == 0 {
        0
    } else {
        (d.laggard_weight * d.drift_prior).div_ceil(den)
    };
    let bound = (d.stale_weight * d.drift_gen).div_ceil(den)
        + prior_term
        + 2 * u128::from(a.stale_account_count_long);
    // The stale weight is exactly one unit of weight per stale long.
    assert_eq!(d.stale_weight, u128::from(a.stale_account_count_long) * POS_SCALE);
    assert_eq!(d.laggard_weight, u128::from(d.laggard_count) * POS_SCALE);
    assert!(hidden > 0);
    assert!(
        hidden <= bound,
        "bound {bound} must cover the real unsettled loss {hidden}"
    );
    // Cover it and an entrant opens with 9 of 12 longs still stale.
    insure(&mut slab, SHORT_DOMAIN, bound);
    let mut entrant = account(10_000);
    deposit(&mut slab, &mut entrant, TRADER_CAPITAL);
    trade(&mut slab, &mut entrant, &mut maker, POS_SCALE, PRICE).unwrap();
    let mut all: Vec<&mut PortfolioAccountV16Account> = traders.iter_mut().collect();
    all.push(&mut maker);
    all.push(&mut entrant);
    validate(&mut slab, &mut all);
}
