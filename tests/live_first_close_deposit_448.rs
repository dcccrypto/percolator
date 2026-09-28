//! GH dcccrypto/percolator-prog#448 — the reporter's CORRECTED "Live-first" sequence
//! (Live close opens + books a chunk -> Live deposit -> resolve -> CloseResolved),
//! driven through real mutators only. Nothing here hand-writes a ledger, a barrier,
//! `pnl` or `capital`.
//!
//! #448's claim: principal settlement drives `pnl` to 0 WITHOUT advancing the close
//! ledger, `settle_resolved_bankruptcy_negative_pnl` early-returns at `pnl >= 0`, so
//! the ledger is left `active && !finalized` with `residual_remaining > 0` and the
//! (asset, side) domain-loss barrier is held forever (`cure_and_cancel_close` refused
//! because a chunk was already booked; no recovery in Resolved).
//!
//! That was true of the baseline the issue was filed against (engine `1bea6692`). It
//! is NOT true of the pinned/deployed engine: F-02 (`ad463472`, "credit a mid-close
//! principal settlement to the close ledger") makes the single principal-settlement
//! funnel (`settle_negative_pnl_from_principal_core_not_atomic`) call
//! `credit_close_progress_principal_settlement`, which books the payment against the
//! ledger, finalizes it at zero residual and releases `pending_domain_loss_barrier_*`.
//!
//! Two reachability facts are pinned alongside, measured rather than asserted:
//!
//! * The reporter's step 1 as written — a Live `LiquidateAtOracle` that books ONE chunk
//!   and leaves a residual — does not happen: `preflight_liquidation_residual_durability`
//!   makes Live liquidation all-or-nothing (the residual is absorbed in one chunk, or
//!   the call fails with `RecoveryRequired` and the wrapper transaction reverts).
//! * The #448 precondition IS still reachable another way: a terminal risk-reducing
//!   trade leaves a pending close ledger (`begin_terminal_trade_residual_if_needed`),
//!   and a Live permissionless `AdvanceClose` crank with `public_b_chunk_atoms` below the
//!   residual books one chunk and leaves the rest. That is the fixture used below.
//!
//! Negative control (measured): deleting the one `credit_close_progress_principal_settlement`
//! call in `settle_negative_pnl_from_principal_core_not_atomic` makes the stranding
//! tests fail with `residual_remaining > 0, finalized = false, barrier = 1`.

use percolator::{
    active_bitmap_is_empty, v16_domain_count_for_market_slots, AutoCrankPlanV16, AutoCrankWorkV16,
    EngineAssetSlotV16Account, LiquidationRequestV16, Market, MarketGroupV16HeaderAccount,
    MarketGroupV16ViewMut, PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, SideV16, TradeRequestV16, V16Config, V16Error, V16PodU128,
    V16PodU64, POS_SCALE,
};

const SIZE_Q: u128 = 10 * POS_SCALE;
/// Residual the terminal trade leaves on the short's close ledger.
const R: u128 = 250;
/// Chunk cap strictly below `R`, so the first Live `AdvanceClose` books ONE chunk and
/// leaves a residual: the #448 "multi-chunk, irreversible progress" precondition.
const CHUNK_CAP: u128 = 100;

fn ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    ([1; 32], [2; 32], [3; 32])
}

fn market_fixture(init_price: u64) -> (MarketGroupV16HeaderAccount, Vec<Market<u64>>) {
    let (market_id, _, _) = ids();
    let cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(market_id, cfg, 1, 0).unwrap();
    let mut markets = vec![Market::new(0u64, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, init_price, 1)
        .unwrap();
    {
        let view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        view.validate_shape().unwrap();
    }
    header.config.maintenance_margin_bps = V16PodU64::new(1_000);
    header.config.initial_margin_bps = V16PodU64::new(1_000);
    header.config.max_price_move_bps_per_slot = V16PodU64::new(500);
    header.config.max_accrual_dt_slots = V16PodU64::new(1);
    header.config.min_funding_lifetime_slots = V16PodU64::new(1);
    (header, markets)
}

fn account_fixture(seed: u8) -> PortfolioAccountV16Account {
    let (market_id, _, owner) = ids();
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        market_id, [seed; 32], owner,
    ));
    let _ = v16_domain_count_for_market_slots(1).unwrap();
    let mut account = PortfolioAccountV16Account::default();
    account.init_empty_in_place(header).unwrap();
    account
}

fn signed_q(q: u128) -> i128 {
    i128::try_from(q).unwrap()
}

fn open_and_walk(
    market: &mut MarketGroupV16ViewMut<'_, u64>,
    long: &mut PortfolioV16ViewMut<'_>,
    short: &mut PortfolioV16ViewMut<'_>,
    long_dep: u128,
    top: u64,
) {
    market.deposit_not_atomic(long, long_dep).unwrap();
    market.deposit_not_atomic(short, 250).unwrap();
    market
        .execute_trade_with_fee_loss_stale_scoped_not_atomic(
            long,
            short,
            TradeRequestV16 {
                asset_index: 0,
                size_q: signed_q(SIZE_Q),
                exec_price: 100,
                fee_bps: 0,
            },
            true,
        )
        .unwrap();
    for (offset, price) in (105u64..=top).step_by(5).enumerate() {
        let slot = 2 + offset as u64;
        market
            .set_asset_raw_oracle_target_not_atomic(0, price)
            .unwrap();
        market
            .accrue_asset_to_not_atomic(0, slot, price, 0, true)
            .unwrap();
    }
}

fn barrier_long(market: &MarketGroupV16ViewMut<'_, u64>) -> u64 {
    market.markets[0]
        .engine
        .pending_domain_loss_barrier_long
        .get()
}

/// Live, and the short carries an ACTIVE close ledger that has already booked one
/// chunk (irreversible progress) and still holds a residual and the barrier. Returns
/// the account's remaining `|pnl|`.
fn live_multi_chunk_close_with_booked_chunk(
    market: &mut MarketGroupV16ViewMut<'_, u64>,
    long: &mut PortfolioV16ViewMut<'_>,
    short: &mut PortfolioV16ViewMut<'_>,
) -> u128 {
    open_and_walk(market, long, short, 1_000, 150);
    // Terminal risk-reducing trade: the short goes flat owing R with capital 0.
    market
        .execute_trade_with_fee_loss_stale_scoped_not_atomic(
            long,
            short,
            TradeRequestV16 {
                asset_index: 0,
                size_q: -signed_q(SIZE_Q),
                exec_price: 150,
                fee_bps: 0,
            },
            true,
        )
        .expect("risk-reducing final trade must remain available");
    let l = short.header.close_progress.try_to_runtime().unwrap();
    assert!(l.active && !l.finalized && l.residual_remaining == R && l.b_loss_booked == 0);
    assert_eq!(barrier_long(market), 1);

    // One Live permissionless AdvanceClose under the chunk cap: books ONE chunk.
    market.header.config.public_b_chunk_atoms = V16PodU128::new(CHUNK_CAP);
    let res = market
        .permissionless_auto_crank_not_atomic(
            short,
            AutoCrankWorkV16 {
                now_slot: 11,
                observations: &[],
                resolved_close_fee_rate_per_slot: 0,
            },
        )
        .expect("the pending close has a permissionless Live continuation");
    assert_eq!(res.selected, AutoCrankPlanV16::AdvanceClose);

    let l = short.header.close_progress.try_to_runtime().unwrap();
    println!(
        "Live multi-chunk close: pnl={} capital={} ledger={{active:{} finalized:{} gross:{} \
         b_loss_booked:{} residual:{}}} barrier_long={}",
        short.header.pnl.get(),
        short.header.capital.get(),
        l.active,
        l.finalized,
        l.gross_loss_at_close_start,
        l.b_loss_booked,
        l.residual_remaining,
        barrier_long(market)
    );
    assert!(active_bitmap_is_empty(
        short.header.active_bitmap.map(V16PodU64::get)
    ));
    assert_eq!(short.header.capital.get(), 0);
    assert!(l.active && !l.finalized && !l.canceled);
    assert_eq!(l.domain_side, SideV16::Long);
    assert_eq!(
        l.b_loss_booked, CHUNK_CAP,
        "one chunk booked: irreversible progress, so cure_and_cancel_close is closed"
    );
    assert_eq!(l.residual_remaining, R - CHUNK_CAP, "genuinely multi-chunk");
    assert_eq!(
        short.header.pnl.get(),
        -((R - CHUNK_CAP) as i128),
        "ledger residual tracks the account's remaining loss before the deposit"
    );
    assert_eq!(barrier_long(market), 1, "domain-loss barrier still held");
    R - CHUNK_CAP
}

fn assert_finalized_and_released(
    market: &MarketGroupV16ViewMut<'_, u64>,
    short: &PortfolioV16ViewMut<'_>,
    tag: &str,
) {
    let l = short.header.close_progress.try_to_runtime().unwrap();
    println!(
        "{tag}: pnl={} capital={} ledger={{active:{} finalized:{} gross:{} b_loss_booked:{} \
         residual:{}}} barrier_long={}",
        short.header.pnl.get(),
        short.header.capital.get(),
        l.active,
        l.finalized,
        l.gross_loss_at_close_start,
        l.b_loss_booked,
        l.residual_remaining,
        barrier_long(market)
    );
    assert_eq!(
        short.header.pnl.get(),
        0,
        "{tag}: the debt is fully settled"
    );
    assert_eq!(
        l.residual_remaining, 0,
        "{tag}: #448 — the ledger must not be left claiming a residual nobody owes"
    );
    assert!(l.finalized, "{tag}: #448 — the close must finalize");
    assert!(
        l.active && !l.canceled,
        "{tag}: finalized-inert is what close_slot_available() accepts, so the next \
         bankruptcy in this domain can open its close"
    );
    assert_eq!(
        barrier_long(market),
        0,
        "{tag}: #448 — the (asset 0, Long) domain-loss barrier must be released"
    );
}

/// The exact #448 sequence: Live close with a booked chunk -> Live deposit of `|pnl|`
/// -> ResolveMarket -> CloseResolved.
#[test]
fn live_first_448_deposit_then_resolve_then_close_finalizes_and_releases_barrier() {
    let (mut header, mut markets) = market_fixture(100);
    let mut long_h = account_fixture(61);
    let mut short_h = account_fixture(62);
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut long = PortfolioV16ViewMut::new(&mut long_h);
    let mut short = PortfolioV16ViewMut::new(&mut short_h);

    let owed = live_multi_chunk_close_with_booked_chunk(&mut market, &mut long, &mut short);

    // 2. Live deposit of exactly |pnl|: accepted (deposit has no close-state gate).
    market
        .deposit_not_atomic(&mut short, owed)
        .expect("Live deposit onto an account with an open close ledger is accepted");
    // 3. Resolve — not blocked by the held barrier.
    market
        .resolve_market_not_atomic(12)
        .expect("resolve proceeds");
    // 4. CloseResolved.
    market
        .close_resolved_account_not_atomic(&mut short, 0)
        .expect("CloseResolved succeeds");

    assert_finalized_and_released(&market, &short, "deposit->resolve->CloseResolved");
    market.validate_shape().unwrap();
    short.validate_with_market(&market.as_view()).unwrap();
}

/// Same Live deposit, but the close is continued in Live by the permissionless crank
/// instead of resolving: principal settlement must finalize it there too.
#[test]
fn live_first_448_deposit_then_live_crank_finalizes_and_releases_barrier() {
    let (mut header, mut markets) = market_fixture(100);
    let mut long_h = account_fixture(63);
    let mut short_h = account_fixture(64);
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut long = PortfolioV16ViewMut::new(&mut long_h);
    let mut short = PortfolioV16ViewMut::new(&mut short_h);

    let owed = live_multi_chunk_close_with_booked_chunk(&mut market, &mut long, &mut short);
    market.deposit_not_atomic(&mut short, owed).unwrap();
    market
        .permissionless_auto_crank_not_atomic(
            &mut short,
            AutoCrankWorkV16 {
                now_slot: 12,
                observations: &[],
                resolved_close_fee_rate_per_slot: 0,
            },
        )
        .expect("Live crank continues the close");

    assert_finalized_and_released(&market, &short, "deposit->Live crank");
    market.validate_shape().unwrap();
}

/// A deposit SMALLER than the remaining loss: the ledger must be credited exactly the
/// principal paid (never over-credited), and the resolved close then books the rest
/// chunk by chunk and finalizes.
#[test]
fn live_first_448_partial_deposit_is_credited_exactly_and_the_close_still_finalizes() {
    let (mut header, mut markets) = market_fixture(100);
    let mut long_h = account_fixture(65);
    let mut short_h = account_fixture(66);
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut long = PortfolioV16ViewMut::new(&mut long_h);
    let mut short = PortfolioV16ViewMut::new(&mut short_h);

    let owed = live_multi_chunk_close_with_booked_chunk(&mut market, &mut long, &mut short);
    let partial = owed / 3;
    market.deposit_not_atomic(&mut short, partial).unwrap();
    market.resolve_market_not_atomic(12).unwrap();

    let mut calls = 0;
    loop {
        calls += 1;
        assert!(calls <= 16, "resolved close must terminate");
        market
            .close_resolved_account_not_atomic(&mut short, 0)
            .expect("CloseResolved progresses");
        let l = short.header.close_progress.try_to_runtime().unwrap();
        if calls == 1 {
            // After the first call the principal has been credited: gross shrank by
            // exactly `partial`, and nothing else moved except one more chunk.
            assert_eq!(l.gross_loss_at_close_start, R - partial);
        }
        if l.finalized {
            break;
        }
    }
    assert_finalized_and_released(&market, &short, "partial deposit");
    let l = short.header.close_progress.try_to_runtime().unwrap();
    assert_eq!(
        l.b_loss_booked,
        R - partial,
        "the loss side is charged exactly what the debtor did not pay"
    );
}

/// Reachability pin for the reporter's step 1 AS WRITTEN: a Live liquidation whose
/// residual exceeds one chunk does not leave a half-booked ledger behind — it is
/// refused with `RecoveryRequired` (all-or-nothing), and with the cap lifted the same
/// liquidation absorbs the whole residual in one call and finalizes.
#[test]
fn live_liquidation_is_all_or_nothing_it_never_leaves_a_multi_chunk_residual() {
    for (cap, expect_ok) in [(20u128, false), (u64::MAX as u128, true)] {
        let (mut header, mut markets) = market_fixture(100);
        let mut long_h = account_fixture(71);
        let mut short_h = account_fixture(72);
        let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut long = PortfolioV16ViewMut::new(&mut long_h);
        let mut short = PortfolioV16ViewMut::new(&mut short_h);
        open_and_walk(&mut market, &mut long, &mut short, 2_000, 140);
        market.header.config.public_b_chunk_atoms = V16PodU128::new(cap);
        let r = market
            .liquidate_account_not_atomic(&mut short, LiquidationRequestV16 { asset_index: 0 });
        println!("liquidation cap={cap} -> {r:?}");
        if expect_ok {
            r.expect("uncapped liquidation succeeds");
            let l = short.header.close_progress.try_to_runtime().unwrap();
            assert!(l.finalized && l.residual_remaining == 0);
            assert_eq!(barrier_long(&market), 0);
        } else {
            assert_eq!(r.unwrap_err(), V16Error::RecoveryRequired);
        }
    }
}
