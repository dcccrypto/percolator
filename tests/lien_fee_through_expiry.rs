//! A counterparty lien pays the provider a utilization fee for every slot its backing was live
//! (src/v16.rs `collect_account_backing_utilization_fee_for_domain_not_atomic` clamps the fee
//! interval at `bucket.expiry_slot`: "Rent stops at the bucket's own expiry"). The fee earned up
//! to expiry must not depend on whether a keeper happened to touch the account before the bucket
//! lapsed.
//!
//! Each case builds one liened position from the same state and runs it twice: once with a keeper
//! crank before expiry, once with the first crank after expiry. Both runs must credit the provider
//! the fee for the same live interval.

use percolator::{
    AutoCrankObservationV16, AutoCrankPlanV16, AutoCrankWorkV16, BackingBucketStatusV16,
    BackingBucketV16, BackingBucketV16Account, EngineAssetSlotV16Account, Market,
    MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, PortfolioAccountV16Account,
    PortfolioV16ViewMut, ProvenanceHeaderV16, ProvenanceHeaderV16Account, SourceCreditStateV16,
    SourceCreditStateV16Account, TradeRequestV16, V16Config, V16PodI128, V16PodU128, V16PodU32,
    V16PodU64, BOUND_SCALE, CREDIT_RATE_SCALE, POS_SCALE,
};

const PRICE: u64 = 1;
const EXPIRY: u64 = 100;
const RATE_E9: u64 = 1_000_000; // 0.1% of the liened backing per slot
const CLAIM: u128 = 1_000_000;

fn account(seed: u8) -> PortfolioAccountV16Account {
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        [1; 32], [seed; 32], [3; 32],
    ));
    let mut account = PortfolioAccountV16Account::default();
    account.init_empty_in_place(header).unwrap();
    account
}

fn crank(
    market: &mut MarketGroupV16ViewMut<'_, u64>,
    account: &mut PortfolioV16ViewMut<'_>,
    slot: u64,
) {
    for _ in 0..32 {
        let result = market
            .permissionless_auto_crank_not_atomic(
                account,
                AutoCrankWorkV16 {
                    now_slot: slot,
                    observations: &[AutoCrankObservationV16 {
                        asset_index: 0,
                        effective_price: PRICE,
                        funding_rate_e9: 0,
                    }],
                    resolved_close_fee_rate_per_slot: 0,
                },
            )
            .unwrap();
        if result.selected == AutoCrankPlanV16::NoAction {
            return;
        }
    }
    panic!("crank reached no fixpoint");
}

struct Outcome {
    /// Provider fee earnings on the liened domain at the end.
    earnings: u128,
    /// Fee the lien owes for its live interval [lien slot, expiry).
    owed: u128,
    bucket_status: BackingBucketStatusV16,
}

/// `touch_before_expiry`: a keeper cranks the liened account at this slot (before `EXPIRY`).
fn run(touch_before_expiry: Option<u64>, end_slot: u64) -> Outcome {
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    cfg.backing_fee_base_rate_e9_per_slot = RATE_E9;
    cfg.backing_fee_slope_at_kink_e9_per_slot = 0;
    cfg.backing_fee_slope_above_kink_e9_per_slot = 0;
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut markets = vec![Market::new(0, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, PRICE, 1)
        .unwrap();

    // A long account holding a source-backed gain on domain 0, fully backed by a provider's fresh
    // bucket that lapses at EXPIRY (same shape as v16_spec_tests' IM-lien fixtures).
    let mut long_account = account(1);
    let mut short_account = account(2);
    let claim_num = CLAIM * BOUND_SCALE;
    long_account.pnl = V16PodI128::new(CLAIM as i128);
    long_account.source_domains[0].domain = V16PodU32::new(0);
    long_account.source_domains[0].source_claim_market_id = V16PodU64::new(1);
    long_account.source_domains[0].source_claim_bound_num = V16PodU128::new(claim_num);
    header.pnl_pos_tot = V16PodU128::new(CLAIM);
    header.pnl_pos_bound_tot_num = V16PodU128::new(claim_num);
    header.pnl_pos_bound_tot = V16PodU128::new(CLAIM);
    header.source_claim_bound_total_num = V16PodU128::new(claim_num);
    header.source_fresh_backing_total_num = V16PodU128::new(claim_num);
    header.vault = V16PodU128::new(CLAIM);
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
        expiry_slot: EXPIRY,
        status: BackingBucketStatusV16::Fresh,
        ..BackingBucketV16::EMPTY
    });

    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut long = PortfolioV16ViewMut::new(&mut long_account);
    let mut short = PortfolioV16ViewMut::new(&mut short_account);
    market.deposit_not_atomic(&mut short, 10 * CLAIM).unwrap();
    // Risk-increasing trade margined by the backed gain: mints the counterparty lien.
    market
        .execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut long,
            &mut short,
            TradeRequestV16 {
                asset_index: 0,
                size_q: (100_000 * POS_SCALE) as i128,
                exec_price: PRICE,
                fee_bps: 0,
            },
            true,
        )
        .unwrap();
    // Capital to pay the fee from, so the fee is never capped by what the account holds.
    market.deposit_not_atomic(&mut long, CLAIM).unwrap();

    let lien_num = long.header.source_domains[0]
        .source_lien_counterparty_backing_num
        .get();
    let lien_slot = long.header.source_domains[0]
        .source_lien_fee_last_slot
        .get();
    assert_ne!(lien_num, 0, "fixture must hold a counterparty lien");
    assert!(lien_slot != 0 && lien_slot < EXPIRY);
    // Base rate only (zero slopes), so the rate is constant while the lien is live.
    let owed =
        lien_num * RATE_E9 as u128 * (EXPIRY - lien_slot) as u128 / (1_000_000_000 * BOUND_SCALE);
    assert!(owed > 0);

    if let Some(slot) = touch_before_expiry {
        assert!(slot < EXPIRY);
        market
            .accrue_asset_to_not_atomic(0, slot, PRICE, 0, true)
            .unwrap();
        crank(&mut market, &mut long, slot);
    }
    market
        .accrue_asset_to_not_atomic(0, end_slot, PRICE, 0, true)
        .unwrap();
    crank(&mut market, &mut long, end_slot);
    crank(&mut market, &mut short, end_slot);
    market.validate_shape().unwrap();

    let bucket = market.markets[0]
        .engine
        .backing_long
        .try_to_runtime()
        .unwrap();
    Outcome {
        // Aggregate of every bucket's utilization_fee_earnings (validated by validate_shape).
        earnings: market.header.backing_provider_earnings_total.get(),
        owed,
        bucket_status: bucket.status,
    }
}

/// Control: a keeper touches the account at the slot before expiry, so only one slot of the live
/// interval is left uncollected when the bucket lapses.
#[test]
fn lien_fee_is_collected_while_the_bucket_is_live() {
    let out = run(Some(EXPIRY - 1), EXPIRY + 50);
    println!(
        "touched at {}: earnings {} of {} owed",
        EXPIRY - 1,
        out.earnings,
        out.owed
    );
    assert_ne!(
        out.bucket_status,
        BackingBucketStatusV16::Fresh,
        "the bucket lapsed"
    );
    let one_slot = out.owed / (EXPIRY - 2) as u128 + 1;
    assert!(
        out.earnings + one_slot >= out.owed,
        "control must collect the live interval"
    );
}

/// The fee earned up to expiry must reach the provider whether or not the account was touched
/// before the bucket lapsed.
///
/// KNOWN RED on this engine and on upstream master b22bef7d: untouched, the provider collects 0 of
/// 9,900; touched at slot 50, 4,900 of 9,900. Every expiry path runs
/// `prepare_counterparty_backing_expiry_delta` (which zeroes the domain's
/// `valid_liened_backing_num`) before the account's fee is collected, and
/// `backing_utilization_rate_e9_for_source_state` returns 0 once that is 0, so the
/// `fee_through_slot = min(now, expiry_slot)` clamp never prices the live interval. Ignored until
/// the fix is agreed upstream (the post-expiry rate is a design choice):
/// `cargo test --features fuzz --test lien_fee_through_expiry -- --ignored`.
#[test]
#[ignore = "known red: lien fee earned before expiry is lost once the bucket expires first"]
fn lien_fee_earned_before_expiry_survives_expiry() {
    let touched = run(Some(EXPIRY / 2), EXPIRY + 50);
    let untouched = run(None, EXPIRY + 50);
    println!(
        "touched at {}: earnings {} of {} owed",
        EXPIRY / 2,
        touched.earnings,
        touched.owed
    );
    println!(
        "untouched:      earnings {} of {} owed",
        untouched.earnings, untouched.owed
    );
    assert_ne!(
        untouched.bucket_status,
        BackingBucketStatusV16::Fresh,
        "the bucket lapsed"
    );
    assert!(
        untouched.earnings + 1 >= untouched.owed,
        "provider lost the fee earned while its backing was live: {} of {} collected",
        untouched.earnings,
        untouched.owed
    );
    assert!(
        touched.earnings + 1 >= touched.owed,
        "provider lost the fee earned after the last touch: {} of {} collected",
        touched.earnings,
        touched.owed
    );
}
