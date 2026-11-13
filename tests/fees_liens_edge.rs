//! Edge cases for the v2.2 engine's fees and liens (tests only).
//!
//! Four areas:
//!   * trade fee: ceil rounding, the per-call cap, value conservation, the GH#133 shortfall
//!     fallback in both orientations, per-leg rounding in a batch, the band maker-exempt entry;
//!   * account backing fee (`charge_account_backing_fee_not_atomic`): the no-op, refusal and
//!     expiry-boundary gates;
//!   * the lien fee split (`backing_domain_fee_split_for_lien_delta_num`) and the maintenance
//!     fee sync (`sync_account_fee_to_slot_not_atomic`);
//!   * counterparty lien fee accrual (floored-fraction carry) and lien release.
//!
//! Every test name carries `fee` or `lien` so the area count is
//! `cargo test --tests -- fee lien`.

use percolator::{
    backing_domain_fee_split_for_lien_delta_num, AutoCrankObservationV16, AutoCrankPlanV16,
    AutoCrankWorkV16, BackingBucketStatusV16, BackingBucketV16, BackingBucketV16Account,
    BackingDomainFeeSplitV16, EngineAssetSlotV16Account, HealthCertV16, HealthCertV16Account,
    Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, PortfolioAccountV16Account,
    PortfolioV16ViewMut, ProvenanceHeaderV16, ProvenanceHeaderV16Account, SourceCreditStateV16,
    SourceCreditStateV16Account, TradeRequestV16, V16Config, V16Error, V16PodI128, V16PodU128,
    V16PodU32, V16PodU64, BOUND_SCALE, CREDIT_RATE_SCALE, POS_SCALE, V16_EMPTY_ACTIVE_BITMAP,
};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

type Fixture = (MarketGroupV16HeaderAccount, Vec<Market<u64>>);

fn market(market_slots: u32, price: u64, max_trading_fee_bps: u64) -> Fixture {
    let mut cfg =
        V16Config::public_user_fund_with_market_slots(market_slots as u16, market_slots, 0, 10);
    cfg.max_trading_fee_bps = max_trading_fee_bps;
    let mut header =
        MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, market_slots, 0).unwrap();
    let mut markets = (0..market_slots)
        .map(|i| Market::new(i as u64, EngineAssetSlotV16Account::default()))
        .collect::<Vec<_>>();
    for (i, slot) in markets.iter_mut().enumerate() {
        header
            .activate_empty_asset_slot_not_atomic(i as u32, &mut slot.engine, price, (i + 1) as u64)
            .unwrap();
    }
    MarketGroupV16ViewMut::new(&mut header, &mut markets)
        .validate_shape()
        .unwrap();
    (header, markets)
}

fn account(seed: u8) -> PortfolioAccountV16Account {
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        [1; 32], [seed; 32], [3; 32],
    ));
    let mut account = PortfolioAccountV16Account::default();
    account.init_empty_in_place(header).unwrap();
    account
}

fn q(lots: u128) -> i128 {
    (lots * POS_SCALE) as i128
}

fn req(asset_index: usize, size_q: i128, exec_price: u64, fee_bps: u64) -> TradeRequestV16 {
    TradeRequestV16 {
        asset_index,
        size_q,
        exec_price,
        fee_bps,
    }
}

/// Two accounts funded with `long_cap` / `short_cap` on a fresh one-asset market.
fn funded_pair(
    price: u64,
    max_fee_bps: u64,
    long_cap: u128,
    short_cap: u128,
) -> (
    Fixture,
    PortfolioAccountV16Account,
    PortfolioAccountV16Account,
) {
    let (mut header, mut markets) = market(1, price, max_fee_bps);
    let mut long_account = account(11);
    let mut short_account = account(12);
    {
        let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut long = PortfolioV16ViewMut::new(&mut long_account);
        let mut short = PortfolioV16ViewMut::new(&mut short_account);
        m.deposit_not_atomic(&mut long, long_cap).unwrap();
        m.deposit_not_atomic(&mut short, short_cap).unwrap();
    }
    ((header, markets), long_account, short_account)
}

// ---------------------------------------------------------------------------------------------
// Trade fee
// ---------------------------------------------------------------------------------------------

/// Notional 100 at 1 bps is 0.01 of an atom; the fee rounds UP to 1 (never a free fill).
#[test]
fn fee_trade_fractional_bps_rounds_up_to_one_atom() {
    let ((mut header, mut markets), mut la, mut sa) = funded_pair(100, 10, 1_000, 1_000);
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut long = PortfolioV16ViewMut::new(&mut la);
    let mut short = PortfolioV16ViewMut::new(&mut sa);
    let out = m
        .execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut long,
            &mut short,
            req(0, q(1), 100, 1),
            true,
        )
        .unwrap();
    assert_eq!(out.notional, 100);
    assert_eq!((out.fee_a, out.fee_b), (1, 0));
    assert_eq!(long.header.capital.get(), 999);
    m.validate_shape().unwrap();
}

/// When notional * bps divides exactly by 10_000 there is no extra rounding atom.
#[test]
fn fee_trade_exact_bps_product_charges_no_extra_atom() {
    for (bps, expected) in [(1u64, 1u128), (3, 3), (10, 10)] {
        // 100 lots at price 100: notional 10_000, so fee = bps exactly.
        let ((mut header, mut markets), mut la, mut sa) = funded_pair(100, 10, 20_000, 20_000);
        let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut long = PortfolioV16ViewMut::new(&mut la);
        let mut short = PortfolioV16ViewMut::new(&mut sa);
        let out = m
            .execute_trade_with_fee_loss_stale_scoped_not_atomic(
                &mut long,
                &mut short,
                req(0, q(100), 100, bps),
                true,
            )
            .unwrap();
        assert_eq!(out.notional, 10_000);
        assert_eq!(out.fee_a, expected, "bps {bps}");
        assert_eq!(out.fee_b, 0);
    }
}

/// `fee_bps` equal to the configured cap is accepted; one bps over it is refused before any
/// value moves.
#[test]
fn fee_trade_at_cap_accepted_one_over_cap_rejected_without_charge() {
    let ((mut header, mut markets), mut la, mut sa) = funded_pair(100, 50, 1_000, 1_000);
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut long = PortfolioV16ViewMut::new(&mut la);
    let mut short = PortfolioV16ViewMut::new(&mut sa);
    let insurance_before = m.header.insurance.get();

    let err = m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
        &mut long,
        &mut short,
        req(0, q(1), 100, 51),
        true,
    );
    assert_eq!(err, Err(V16Error::InvalidConfig));
    assert_eq!(long.header.capital.get(), 1_000);
    assert_eq!(short.header.capital.get(), 1_000);
    assert_eq!(m.header.insurance.get(), insurance_before);

    let out = m
        .execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut long,
            &mut short,
            req(0, q(1), 100, 50),
            true,
        )
        .unwrap();
    // 100 * 50 / 10_000 = 0.5 -> 1.
    assert_eq!((out.fee_a, out.fee_b), (1, 0));
    m.validate_shape().unwrap();
}

/// The trade fee moves capital to insurance: vault unchanged, c_tot down by the fee,
/// insurance up by the fee.
#[test]
fn fee_trade_moves_capital_to_insurance_and_keeps_vault() {
    let ((mut header, mut markets), mut la, mut sa) = funded_pair(100, 1_000, 1_000, 1_000);
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut long = PortfolioV16ViewMut::new(&mut la);
    let mut short = PortfolioV16ViewMut::new(&mut sa);
    let (vault, c_tot, ins) = (
        m.header.vault.get(),
        m.header.c_tot.get(),
        m.header.insurance.get(),
    );
    let out = m
        .execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut long,
            &mut short,
            req(0, -q(2), 100, 1_000),
            false,
        )
        .unwrap();
    let fee = out.fee_a + out.fee_b;
    assert_eq!(fee, 20);
    assert_eq!(out.fee_b, 20, "short-side taker pays");
    assert_eq!(m.header.vault.get(), vault);
    assert_eq!(m.header.c_tot.get(), c_tot - fee);
    assert_eq!(m.header.insurance.get(), ins + fee);
    m.validate_shape().unwrap();
}

/// GH#133 shortfall: the taker can pay only part of the fee, the maker is asked for exactly the
/// remainder, and the two charges add up to the fee. Checked in both orientations.
fn shortfall_case(taker_is_long: bool) {
    // Both sides open 1 lot at price 100 fee-free (100% IM). The taker is funded thinly.
    let (long_cap, short_cap) = if taker_is_long {
        (103, 1_000)
    } else {
        (1_000, 103)
    };
    let ((mut header, mut markets), mut la, mut sa) = funded_pair(100, 1_000, long_cap, short_cap);
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut long = PortfolioV16ViewMut::new(&mut la);
    let mut short = PortfolioV16ViewMut::new(&mut sa);
    m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
        &mut long,
        &mut short,
        req(0, q(1), 100, 0),
        taker_is_long,
    )
    .unwrap();
    // A maintenance fee drains the taker to 3 atoms while the position is open.
    m.accrue_asset_to_not_atomic(0, 10, 100, 0, true).unwrap();
    let taker = if taker_is_long { &mut long } else { &mut short };
    // The fee runs to the asset's last accrued slot (the loss-currentness anchor).
    let anchor = m.markets[0].engine.asset.slot_last.get().min(10);
    let dt = anchor - taker.header.last_fee_slot.get();
    assert!(dt > 0 && 100 % dt == 0, "fee interval {dt}");
    assert_eq!(
        m.sync_account_fee_to_slot_not_atomic(taker, 10, 100 / dt as u128),
        Ok(100)
    );
    assert_eq!(taker.header.capital.get(), 3);
    let maker_cap_before = if taker_is_long {
        short.header.capital.get()
    } else {
        long.header.capital.get()
    };
    // Close the pair with a 10% fee (10 atoms): the taker pays its 3, the maker the other 7.
    let out = m
        .execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut long,
            &mut short,
            req(0, -q(1), 100, 1_000),
            taker_is_long,
        )
        .unwrap();
    let (taker_fee, maker_fee) = if taker_is_long {
        (out.fee_a, out.fee_b)
    } else {
        (out.fee_b, out.fee_a)
    };
    assert_eq!((taker_fee, maker_fee), (3, 7));
    let (taker_cap, maker_cap) = if taker_is_long {
        (long.header.capital.get(), short.header.capital.get())
    } else {
        (short.header.capital.get(), long.header.capital.get())
    };
    assert_eq!(taker_cap, 0);
    assert_eq!(maker_cap, maker_cap_before - 7);
    m.validate_shape().unwrap();
}

#[test]
fn fee_trade_shortfall_long_taker_maker_pays_only_remainder() {
    shortfall_case(true);
}

#[test]
fn fee_trade_shortfall_short_taker_maker_pays_only_remainder() {
    shortfall_case(false);
}

/// Each fill of a batch is rounded up on its own: two sub-atom legs cost 1 atom each, while one
/// trade of the same total size costs 1. This is the per-fill no-free-OI rule (upstream
/// 8f25aa5d), recorded so a change to it is visible.
#[test]
fn fee_batch_rounds_each_leg_up_separately() {
    let half = POS_SCALE / 200; // 0.005 lot at price 100 -> notional 0.5 atom
                                // A batch may carry at most one leg per portfolio asset, so use a two-asset market.
    let (mut header, mut markets) = market(2, 100, 1);
    let mut la = account(13);
    let mut sa = account(14);
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut long = PortfolioV16ViewMut::new(&mut la);
    let mut short = PortfolioV16ViewMut::new(&mut sa);
    m.deposit_not_atomic(&mut long, 1_000).unwrap();
    m.deposit_not_atomic(&mut short, 1_000).unwrap();
    let out = m
        .execute_batch_with_fee_loss_stale_scoped_not_atomic(
            &mut long,
            &mut short,
            &[req(0, half as i128, 100, 1), req(1, half as i128, 100, 1)],
            true,
        )
        .unwrap();
    assert_eq!(out.fill_count, 2);
    assert_eq!((out.fee_a, out.fee_b), (2, 0));

    let ((mut header, mut markets), mut la, mut sa) = funded_pair(100, 1, 1_000, 1_000);
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut long = PortfolioV16ViewMut::new(&mut la);
    let mut short = PortfolioV16ViewMut::new(&mut sa);
    let out = m
        .execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut long,
            &mut short,
            req(0, (2 * half) as i128, 100, 1),
            true,
        )
        .unwrap();
    assert_eq!((out.fee_a, out.fee_b), (1, 0));
}

/// The band maker-exempt entry exempts the maker from the leg-size band check only; the taker
/// still pays the trade fee and the maker still pays nothing.
#[test]
fn fee_band_maker_exempt_entry_still_charges_the_taker() {
    let ((mut header, mut markets), mut la, mut sa) = funded_pair(100, 1_000, 1_000, 1_000);
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut long = PortfolioV16ViewMut::new(&mut la);
    let mut short = PortfolioV16ViewMut::new(&mut sa);
    let out = m
        .execute_trade_band_maker_exempt_not_atomic(
            &mut long,
            &mut short,
            req(0, q(1), 100, 1_000),
            true,
        )
        .unwrap();
    assert_eq!((out.fee_a, out.fee_b), (10, 0));
    assert_eq!(short.header.capital.get(), 1_000);
}

// ---------------------------------------------------------------------------------------------
// Account backing fee (wrapper-priced, engine-applied)
// ---------------------------------------------------------------------------------------------

fn current_cert(
    m: &MarketGroupV16ViewMut<'_, u64>,
    equity: i128,
    im: u128,
) -> HealthCertV16Account {
    HealthCertV16Account::from_runtime(&HealthCertV16 {
        certified_equity: equity,
        certified_initial_req: im,
        certified_maintenance_req: im,
        cert_oracle_epoch: m.header.oracle_epoch.get(),
        cert_funding_epoch: m.header.funding_epoch.get(),
        cert_risk_epoch: m.header.risk_epoch.get(),
        cert_asset_set_epoch: m.header.asset_set_epoch.get(),
        active_bitmap_at_cert: V16_EMPTY_ACTIVE_BITMAP,
        valid: true,
        ..HealthCertV16::default()
    })
}

/// A market with a fresh counterparty bucket on domain 0 expiring at `expiry`, and an account
/// with `capital` and a current certificate (IM 0).
fn backing_fee_fixture(capital: u128, expiry: u64) -> (Fixture, PortfolioAccountV16Account) {
    let (mut header, mut markets) = market(1, 100, 0);
    let mut a = account(21);
    header.vault = V16PodU128::new(capital);
    header.c_tot = V16PodU128::new(capital);
    a.capital = V16PodU128::new(capital);
    {
        let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        m.deposit_fresh_counterparty_backing_not_atomic(0, 1, expiry)
            .unwrap();
        a.health_cert = current_cert(&m, capital as i128, 0);
    }
    ((header, markets), a)
}

/// A zero fee is a no-op that never inspects the account: it succeeds even with a stale
/// certificate and negative PnL (lenient, nothing to collect).
#[test]
fn backing_fee_zero_total_is_a_noop_even_on_stale_negative_account() {
    let ((mut header, mut markets), mut a) = backing_fee_fixture(100, 50);
    a.health_cert.valid = 0;
    a.pnl = V16PodI128::new(-1);
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut acct = PortfolioV16ViewMut::new(&mut a);
    assert_eq!(
        m.charge_account_backing_fee_not_atomic(&mut acct, 0, 0, 1, 0),
        Ok(0)
    );
    assert_eq!(acct.header.capital.get(), 100);
}

/// The account must cover the whole fee; there is no partial backing-fee charge.
#[test]
fn backing_fee_more_than_capital_is_refused_not_partially_charged() {
    let ((mut header, mut markets), mut a) = backing_fee_fixture(9, 50);
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut acct = PortfolioV16ViewMut::new(&mut a);
    assert_eq!(
        m.charge_account_backing_fee_not_atomic(&mut acct, 0, 6, 1, 4),
        Err(V16Error::LockActive)
    );
    assert_eq!(acct.header.capital.get(), 9);
    assert_eq!(m.header.c_tot.get(), 9);
    assert_eq!(m.header.insurance.get(), 0);
}

/// Exactly enough capital is enough: the account may pay down to zero when its IM is zero.
#[test]
fn backing_fee_equal_to_capital_is_charged_in_full() {
    let ((mut header, mut markets), mut a) = backing_fee_fixture(10, 50);
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut acct = PortfolioV16ViewMut::new(&mut a);
    assert_eq!(
        m.charge_account_backing_fee_not_atomic(&mut acct, 0, 6, 1, 4),
        Ok(10)
    );
    assert_eq!(acct.header.capital.get(), 0);
    assert_eq!(acct.header.health_cert.certified_equity.get(), 0);
    m.validate_shape().unwrap();
    acct.validate_with_market(&m.as_view()).unwrap();
}

/// A stale certificate refuses a nonzero backing fee (the account cannot be debited on a
/// statement that is not current).
#[test]
fn backing_fee_with_stale_certificate_is_refused() {
    let ((mut header, mut markets), mut a) = backing_fee_fixture(100, 50);
    a.health_cert.valid = 0;
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut acct = PortfolioV16ViewMut::new(&mut a);
    assert_eq!(
        m.charge_account_backing_fee_not_atomic(&mut acct, 0, 6, 1, 4),
        Err(V16Error::Stale)
    );
    assert_eq!(acct.header.capital.get(), 100);
}

/// An out-of-range domain is refused before anything is read or moved.
#[test]
fn backing_fee_unknown_domain_is_refused() {
    let ((mut header, mut markets), mut a) = backing_fee_fixture(100, 50);
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut acct = PortfolioV16ViewMut::new(&mut a);
    assert!(m
        .charge_account_backing_fee_not_atomic(&mut acct, 2, 6, 1, 4)
        .is_err());
    assert!(m
        .charge_account_backing_fee_not_atomic(&mut acct, 0, 6, 2, 4)
        .is_err());
    assert_eq!(acct.header.capital.get(), 100);
    assert_eq!(m.header.c_tot.get(), 100);
}

/// Fee after expiry: a provider fee is credited one slot before the bucket's expiry, and refused
/// at the expiry slot itself (the bucket is no longer live then).
#[test]
fn backing_fee_provider_share_refused_from_the_expiry_slot() {
    const EXPIRY: u64 = 50;
    // One slot before expiry: accepted, provider earnings credited.
    let ((mut header, mut markets), mut a) = backing_fee_fixture(100, EXPIRY);
    header.current_slot = V16PodU64::new(EXPIRY - 1);
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut acct = PortfolioV16ViewMut::new(&mut a);
    assert_eq!(
        m.charge_account_backing_fee_not_atomic(&mut acct, 0, 6, 1, 0),
        Ok(6)
    );
    assert_eq!(
        m.markets[0]
            .engine
            .backing_long
            .utilization_fee_earnings
            .get(),
        6
    );

    // At the expiry slot: refused.
    let ((mut header, mut markets), mut a) = backing_fee_fixture(100, EXPIRY);
    header.current_slot = V16PodU64::new(EXPIRY);
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut acct = PortfolioV16ViewMut::new(&mut a);
    assert_eq!(
        m.charge_account_backing_fee_not_atomic(&mut acct, 0, 6, 1, 0),
        Err(V16Error::LockActive)
    );
    assert_eq!(
        m.markets[0]
            .engine
            .backing_long
            .utilization_fee_earnings
            .get(),
        0,
        "no provider earnings after expiry"
    );
}

/// An insurance-only backing fee does not depend on the provider bucket and is still accepted
/// after the bucket's expiry.
#[test]
fn backing_fee_insurance_only_is_accepted_after_bucket_expiry() {
    const EXPIRY: u64 = 50;
    let ((mut header, mut markets), mut a) = backing_fee_fixture(100, EXPIRY);
    header.current_slot = V16PodU64::new(EXPIRY + 10);
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut acct = PortfolioV16ViewMut::new(&mut a);
    assert_eq!(
        m.charge_account_backing_fee_not_atomic(&mut acct, 0, 0, 1, 4),
        Ok(4)
    );
    assert_eq!(m.header.insurance.get(), 4);
    assert_eq!(acct.header.capital.get(), 96);
    m.validate_shape().unwrap();
}

// ---------------------------------------------------------------------------------------------
// Lien fee split and the maintenance fee sync
// ---------------------------------------------------------------------------------------------

/// The split rounds the total UP and the insurance share DOWN, so the provider keeps the
/// rounding atom and the parts always add up to the total.
#[test]
fn lien_fee_split_rounds_total_up_and_insurance_share_down() {
    // 1 atom at 1 bps: total 0.0001 -> 1; insurance half of 1 -> 0; provider 1.
    assert_eq!(
        backing_domain_fee_split_for_lien_delta_num(BOUND_SCALE, 1, 5_000),
        Ok(BackingDomainFeeSplitV16 {
            lien_delta_atoms: 1,
            total_fee: 1,
            provider_fee: 1,
            insurance_fee: 0,
        })
    );
    // 10_001 atoms at 1 bps: total 1.0001 -> 2; insurance 1/3 of 2 -> 0; provider 2.
    let s = backing_domain_fee_split_for_lien_delta_num(10_001 * BOUND_SCALE, 1, 3_333).unwrap();
    assert_eq!((s.total_fee, s.insurance_fee, s.provider_fee), (2, 0, 2));
    // Sweep: parts always sum to the total and insurance never exceeds its share.
    for atoms in [1u128, 7, 9_999, 10_000, 10_001, 123_457] {
        for fee_bps in [0u16, 1, 13, 10_000] {
            for share in [0u16, 1, 5_000, 9_999, 10_000] {
                let s = backing_domain_fee_split_for_lien_delta_num(
                    atoms * BOUND_SCALE,
                    fee_bps,
                    share,
                )
                .unwrap();
                assert_eq!(s.provider_fee + s.insurance_fee, s.total_fee);
                assert!(s.insurance_fee * 10_000 <= s.total_fee * share as u128);
                assert!(s.total_fee * 10_000 >= atoms * fee_bps as u128);
                assert!(s.total_fee * 10_000 < atoms * fee_bps as u128 + 10_000);
            }
        }
    }
}

/// A full insurance share leaves the provider nothing; a zero delta costs nothing.
#[test]
fn lien_fee_split_full_insurance_share_and_zero_delta() {
    let s = backing_domain_fee_split_for_lien_delta_num(10_000 * BOUND_SCALE, 25, 10_000).unwrap();
    assert_eq!((s.total_fee, s.insurance_fee, s.provider_fee), (25, 25, 0));
    assert_eq!(
        backing_domain_fee_split_for_lien_delta_num(0, 10_000, 10_000),
        Ok(BackingDomainFeeSplitV16::default())
    );
}

/// A sub-atom lien delta and out-of-range bps are refused.
#[test]
fn lien_fee_split_rejects_unaligned_delta_and_bps_over_100_percent() {
    assert_eq!(
        backing_domain_fee_split_for_lien_delta_num(BOUND_SCALE + 1, 1, 0),
        Err(V16Error::InvalidConfig)
    );
    assert_eq!(
        backing_domain_fee_split_for_lien_delta_num(BOUND_SCALE, 10_001, 0),
        Err(V16Error::InvalidConfig)
    );
    assert_eq!(
        backing_domain_fee_split_for_lien_delta_num(BOUND_SCALE, 1, 10_001),
        Err(V16Error::InvalidConfig)
    );
}

/// The maintenance fee cursor never runs backwards, and a second sync at the same slot charges
/// nothing.
#[test]
fn fee_sync_refuses_earlier_slot_and_charges_nothing_twice() {
    let ((mut header, mut markets), mut la, _) = funded_pair(100, 0, 1_000, 1_000);
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut a = PortfolioV16ViewMut::new(&mut la);
    assert_eq!(m.sync_account_fee_to_slot_not_atomic(&mut a, 10, 3), Ok(30));
    assert_eq!(a.header.last_fee_slot.get(), 10);
    assert_eq!(m.sync_account_fee_to_slot_not_atomic(&mut a, 10, 3), Ok(0));
    assert_eq!(
        m.sync_account_fee_to_slot_not_atomic(&mut a, 9, 3),
        Err(V16Error::Stale)
    );
    assert_eq!(a.header.capital.get(), 970);
    assert_eq!(m.header.insurance.get(), 30);
    m.validate_shape().unwrap();
}

/// A rate whose product with the interval overflows is clamped, and the account pays at most
/// its capital (lenient: never more than it holds, never an error).
#[test]
fn fee_sync_overflowing_rate_charges_at_most_capital() {
    let ((mut header, mut markets), mut la, _) = funded_pair(100, 0, 1_000, 1_000);
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut a = PortfolioV16ViewMut::new(&mut la);
    assert_eq!(
        m.sync_account_fee_to_slot_not_atomic(&mut a, u64::MAX, u128::MAX),
        Ok(1_000)
    );
    assert_eq!(a.header.capital.get(), 0);
    assert_eq!(a.header.last_fee_slot.get(), u64::MAX);
    m.validate_shape().unwrap();
}

// ---------------------------------------------------------------------------------------------
// Counterparty lien: fee accrual and release
// ---------------------------------------------------------------------------------------------

const LIEN_PRICE: u64 = 1;
const LIEN_EXPIRY: u64 = 1_000;
const LIEN_CLAIM: u128 = 1_000_000;
const LIEN_LOTS: u128 = 100_000;

fn lien_crank(m: &mut MarketGroupV16ViewMut<'_, u64>, a: &mut PortfolioV16ViewMut<'_>, slot: u64) {
    m.accrue_asset_to_not_atomic(0, slot, LIEN_PRICE, 0, true)
        .unwrap();
    for _ in 0..32 {
        let r = m
            .permissionless_auto_crank_not_atomic(
                a,
                AutoCrankWorkV16 {
                    now_slot: slot,
                    observations: &[AutoCrankObservationV16 {
                        asset_index: 0,
                        effective_price: LIEN_PRICE,
                        funding_rate_e9: 0,
                    }],
                    resolved_close_fee_rate_per_slot: 0,
                },
            )
            .unwrap();
        if r.selected == AutoCrankPlanV16::NoAction {
            return;
        }
    }
    panic!("crank reached no fixpoint");
}

/// A long account whose source-backed gain on domain 0 margins a risk-increasing trade, which
/// mints a counterparty lien on a fresh bucket expiring at `LIEN_EXPIRY`. Same shape as
/// tests/lien_fee_through_expiry.rs. The base backing fee rate is `rate_e9` per slot.
fn lien_fixture(
    rate_e9: u64,
    extra_capital: u128,
) -> (
    Fixture,
    PortfolioAccountV16Account,
    PortfolioAccountV16Account,
) {
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    cfg.backing_fee_base_rate_e9_per_slot = rate_e9;
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut markets = vec![Market::new(0, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, LIEN_PRICE, 1)
        .unwrap();
    let mut la = account(31);
    let mut sa = account(32);
    let claim_num = LIEN_CLAIM * BOUND_SCALE;
    la.pnl = V16PodI128::new(LIEN_CLAIM as i128);
    la.source_domains[0].domain = V16PodU32::new(0);
    la.source_domains[0].source_claim_market_id = V16PodU64::new(1);
    la.source_domains[0].source_claim_bound_num = V16PodU128::new(claim_num);
    header.pnl_pos_tot = V16PodU128::new(LIEN_CLAIM);
    header.pnl_pos_bound_tot_num = V16PodU128::new(claim_num);
    header.pnl_pos_bound_tot = V16PodU128::new(LIEN_CLAIM);
    header.source_claim_bound_total_num = V16PodU128::new(claim_num);
    header.source_fresh_backing_total_num = V16PodU128::new(claim_num);
    header.vault = V16PodU128::new(LIEN_CLAIM);
    markets[0].engine.source_credit_long =
        SourceCreditStateV16Account::from_runtime(&SourceCreditStateV16 {
            positive_claim_bound_num: claim_num,
            exact_positive_claim_num: claim_num,
            fresh_reserved_backing_num: claim_num,
            credit_rate_num: CREDIT_RATE_SCALE,
            ..SourceCreditStateV16::EMPTY
        });
    markets[0].engine.backing_long = BackingBucketV16Account::from_runtime(&BackingBucketV16 {
        market_id: 1,
        fresh_unliened_backing_num: claim_num,
        expiry_slot: LIEN_EXPIRY,
        status: BackingBucketStatusV16::Fresh,
        ..BackingBucketV16::EMPTY
    });
    {
        let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut long = PortfolioV16ViewMut::new(&mut la);
        let mut short = PortfolioV16ViewMut::new(&mut sa);
        m.deposit_not_atomic(&mut short, 10 * LIEN_CLAIM).unwrap();
        m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut long,
            &mut short,
            req(0, q(LIEN_LOTS), LIEN_PRICE, 0),
            true,
        )
        .unwrap();
        // Capital the lien fee is paid from. Below the position's IM (LIEN_LOTS atoms) the lien
        // is still needed; at or above it the lien is releasable.
        if extra_capital != 0 {
            m.deposit_not_atomic(&mut long, extra_capital).unwrap();
        }
    }
    assert_ne!(
        la.source_domains[0]
            .source_lien_counterparty_backing_num
            .get(),
        0,
        "fixture must hold a counterparty lien"
    );
    ((header, markets), la, sa)
}

fn lien_atoms(a: &PortfolioAccountV16Account) -> u128 {
    a.source_domains[0]
        .source_lien_counterparty_backing_num
        .get()
        / BOUND_SCALE
}

/// Closed-form lien fee over `dt` slots at the base rate (zero slopes): floor(L * r * dt / 1e9).
fn lien_owed(lien_atoms: u128, rate_e9: u64, dt: u64) -> u128 {
    lien_atoms * rate_e9 as u128 * dt as u128 / 1_000_000_000
}

/// Capital that leaves the lien needed: half the position's IM.
const NEEDED: u128 = LIEN_LOTS / 2;
/// Capital that covers the position's IM on its own, so the lien is releasable.
const COVERED: u128 = LIEN_CLAIM;

/// The lien fee for one slot floors to zero at a low rate. Refreshing every slot must still
/// collect it: the cursor does not advance on a zero quote, so the fraction carries forward and
/// the lien is never free (upstream f7ad3ee9). Per-slot refreshing collects no more than the
/// closed form, and loses less than one atom per collection. (The permissionless crank selects
/// no action on an account that is already current, so this drives the refresh directly.)
#[test]
fn lien_fee_sub_atom_per_slot_carries_until_it_is_collected() {
    // Probe the lien size with a zero rate, then pick a rate worth 0.4 atom per slot.
    let (_, probe, _) = lien_fixture(0, NEEDED);
    let atoms = lien_atoms(&probe);
    let rate_e9 = (400_000_000u128 / atoms) as u64;
    assert!(rate_e9 > 0);
    assert!(
        atoms * (rate_e9 as u128) < 1_000_000_000,
        "one slot must floor to zero"
    );

    let ((mut header, mut markets), mut la, _) = lien_fixture(rate_e9, NEEDED);
    let lien_before = la.source_domains[0]
        .source_lien_counterparty_backing_num
        .get();
    let start = la.source_domains[0].source_lien_fee_last_slot.get();
    assert_ne!(start, 0);
    let end = start + 50;
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut long = PortfolioV16ViewMut::new(&mut la);
    let mut collections = 0u128;
    let mut last = 0u128;
    for slot in start + 1..=end {
        // A full refresh certifies the account, and certifying collects the lien fee.
        m.accrue_asset_to_not_atomic(0, slot, LIEN_PRICE, 0, true)
            .unwrap();
        m.full_account_refresh_not_atomic(&mut long).unwrap();
        let earned = m.header.backing_provider_earnings_total.get();
        if earned != last {
            collections += 1;
            last = earned;
        }
    }
    m.validate_shape().unwrap();
    assert_eq!(
        long.header.source_domains[0]
            .source_lien_counterparty_backing_num
            .get(),
        lien_before,
        "the lien is still needed and still held"
    );
    let owed = lien_owed(atoms, rate_e9, end - start);
    let earned = m.header.backing_provider_earnings_total.get();
    println!(
        "lien {atoms} atoms, rate {rate_e9}e-9/slot: earned {earned} of {owed} in {collections} collections"
    );
    assert!(owed >= 10);
    assert!(collections >= 1, "the lien was free");
    assert!(earned <= owed, "charged more than the closed form");
    assert!(
        earned + collections >= owed,
        "lost a whole atom or more on some collection"
    );
}

/// FINDING (recorded, not fixed): when a collection does charge, the cursor jumps to the
/// collection slot and the floored fraction is dropped. Refreshing every slot at 1.9 atoms per
/// slot pays 1 atom per slot, so the provider collects 20 of the 38 owed over 20 slots. The
/// loss is under one atom per collection and always in the trader's favour; the spec leaves the
/// backing-fee schedule to wrapper policy (spec.md §2, "Backing fee schedules are
/// wrapper/product policy"), so this is recorded for review rather than tightened here.
#[test]
fn lien_fee_frequent_refresh_drops_the_floored_fraction_each_collection() {
    let (_, probe, _) = lien_fixture(0, NEEDED);
    let atoms = lien_atoms(&probe);
    let rate_e9 = (1_900_000_000u128 / atoms) as u64; // 1.9 atoms per slot
    assert_eq!(atoms * rate_e9 as u128, 1_900_000_000);
    let ((mut header, mut markets), mut la, _) = lien_fixture(rate_e9, NEEDED);
    let start = la.source_domains[0].source_lien_fee_last_slot.get();
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut long = PortfolioV16ViewMut::new(&mut la);
    for slot in start + 1..=start + 20 {
        m.accrue_asset_to_not_atomic(0, slot, LIEN_PRICE, 0, true)
            .unwrap();
        m.full_account_refresh_not_atomic(&mut long).unwrap();
    }
    assert_eq!(lien_owed(atoms, rate_e9, 20), 38);
    assert_eq!(
        m.header.backing_provider_earnings_total.get(),
        20,
        "one atom per slot: the 0.9 fraction is dropped at every collection"
    );
    m.validate_shape().unwrap();
}

/// One crank over a long interval charges exactly the closed form (no per-slot loss).
#[test]
fn lien_fee_single_collection_matches_closed_form() {
    let rate_e9 = 1_000; // 1e-6 per slot
    let ((mut header, mut markets), mut la, _) = lien_fixture(rate_e9, NEEDED);
    let atoms = lien_atoms(&la);
    let start = la.source_domains[0].source_lien_fee_last_slot.get();
    let end = start + 400;
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut long = PortfolioV16ViewMut::new(&mut la);
    let capital_before = long.header.capital.get();
    lien_crank(&mut m, &mut long, end);
    let owed = lien_owed(atoms, rate_e9, end - start);
    assert!(owed > 0);
    assert_eq!(m.header.backing_provider_earnings_total.get(), owed);
    assert_eq!(long.header.capital.get(), capital_before - owed);
    assert_eq!(
        long.header.source_domains[0]
            .source_lien_fee_last_slot
            .get(),
        end,
        "cursor advances to the collection slot"
    );
    m.validate_shape().unwrap();
}

/// While the lien still margins the open position (capital alone is below IM), release is
/// refused and the lien stays in place.
#[test]
fn lien_release_refused_while_the_lien_is_still_needed_for_margin() {
    let ((mut header, mut markets), mut la, _) = lien_fixture(0, NEEDED);
    let lien_before = la.source_domains[0]
        .source_lien_counterparty_backing_num
        .get();
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut long = PortfolioV16ViewMut::new(&mut la);
    assert_eq!(
        m.release_account_source_credit_liens_if_unneeded_not_atomic(&mut long),
        Err(V16Error::LockActive)
    );
    assert_eq!(
        long.header.source_domains[0]
            .source_lien_counterparty_backing_num
            .get(),
        lien_before
    );
}

/// Once capital alone covers IM the lien is released even with the position still open, and
/// the provider is first paid the lien fee for the live interval up to the release.
#[test]
fn lien_release_of_a_capital_covered_lien_pays_the_fee_first() {
    let rate_e9 = 1_000;
    let ((mut header, mut markets), mut la, _) = lien_fixture(rate_e9, COVERED);
    let atoms = lien_atoms(&la);
    let start = la.source_domains[0].source_lien_fee_last_slot.get();
    let release_slot = start + 300;
    assert!(release_slot < LIEN_EXPIRY);
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut long = PortfolioV16ViewMut::new(&mut la);
    m.accrue_asset_to_not_atomic(0, release_slot, LIEN_PRICE, 0, true)
        .unwrap();
    let released = m
        .release_account_source_credit_liens_if_unneeded_not_atomic(&mut long)
        .unwrap();
    assert_eq!(released, LIEN_LOTS);
    assert!(
        !percolator::active_bitmap_is_empty(long.header.active_bitmap.map(|w| w.get())),
        "the position is still open"
    );
    assert_eq!(
        long.header.source_domains[0]
            .source_lien_counterparty_backing_num
            .get(),
        0
    );
    let owed = lien_owed(atoms, rate_e9, release_slot - start);
    assert!(owed > 0);
    assert_eq!(m.header.backing_provider_earnings_total.get(), owed);
    let bucket = m.markets[0].engine.backing_long.try_to_runtime().unwrap();
    assert_eq!(bucket.valid_liened_backing_num, 0);
    assert_eq!(bucket.fresh_unliened_backing_num, LIEN_CLAIM * BOUND_SCALE);
    m.validate_shape().unwrap();
    long.validate_with_market(&m.as_view()).unwrap();
}

/// Release on an account that holds no lien is a successful no-op.
#[test]
fn lien_release_without_any_lien_is_a_noop() {
    let ((mut header, mut markets), mut la, _) = funded_pair(100, 0, 1_000, 1_000);
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut a = PortfolioV16ViewMut::new(&mut la);
    assert_eq!(
        m.release_account_source_credit_liens_if_unneeded_not_atomic(&mut a),
        Ok(0)
    );
    assert_eq!(a.header.capital.get(), 1_000);
    m.validate_shape().unwrap();
}

/// Closing the position before expiry and releasing the lien: the provider is paid the lien fee
/// for the live interval up to the release, the lien fields are cleared, and the backing goes
/// back to fresh unliened (it was still live).
#[test]
fn lien_release_after_close_collects_fee_and_restores_live_backing() {
    let rate_e9 = 1_000;
    let ((mut header, mut markets), mut la, mut sa) = lien_fixture(rate_e9, COVERED);
    let atoms = lien_atoms(&la);
    let start = la.source_domains[0].source_lien_fee_last_slot.get();
    let close_slot = start + 300;
    assert!(close_slot < LIEN_EXPIRY);
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut long = PortfolioV16ViewMut::new(&mut la);
    let mut short = PortfolioV16ViewMut::new(&mut sa);
    m.accrue_asset_to_not_atomic(0, close_slot, LIEN_PRICE, 0, true)
        .unwrap();
    m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
        &mut long,
        &mut short,
        req(0, -q(LIEN_LOTS), LIEN_PRICE, 0),
        true,
    )
    .unwrap();
    // The close may already have released the lien; release is then a no-op.
    m.release_account_source_credit_liens_if_unneeded_not_atomic(&mut long)
        .unwrap();
    let src = long.header.source_domains[0];
    assert_eq!(src.source_lien_counterparty_backing_num.get(), 0);
    assert_eq!(src.source_claim_liened_num.get(), 0);
    assert_eq!(src.source_lien_fee_last_slot.get(), 0);
    let bucket = m.markets[0].engine.backing_long.try_to_runtime().unwrap();
    assert_eq!(bucket.status, BackingBucketStatusV16::Fresh);
    assert_eq!(bucket.valid_liened_backing_num, 0);
    assert_eq!(bucket.fresh_unliened_backing_num, LIEN_CLAIM * BOUND_SCALE);
    let owed = lien_owed(atoms, rate_e9, close_slot - start);
    assert!(owed > 0);
    assert_eq!(
        m.header.backing_provider_earnings_total.get(),
        owed,
        "provider paid for the live interval up to the release"
    );
    m.validate_shape().unwrap();
    long.validate_with_market(&m.as_view()).unwrap();
}
