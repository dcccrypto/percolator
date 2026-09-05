//! GH#457 — the ATTACH writer must contribute its A-SCALED share of `oi_eff`.
//!
//! `add_open_interest_for_new_position` added the RAW basis to an A-scaled
//! accumulator. On a fresh side (`a == ADL_ONE`) that happens to be right; on a
//! side already scaled by a prior unilateral close it is not, because per-leg
//! rounding does not sum to the rounding of the sum. `oi_eff` then lags
//! `floor(a · Σloss_weight / SOCIAL_WEIGHT_SCALE)`, `clear_leg` subtracts the full
//! aggregate contribution, and the counter underflows — `CounterUnderflow` on a
//! LEGITIMATE close. Liquidation routes through the same `clear_leg`, so the
//! position cannot be forced out either.
//!
//! The drift is invisible to production: `validate_shape` checks
//! `oi_eff_long == oi_eff_short`, and both sides drift together.
//!
//! Adapted from the reporter's PoC. Only real mutators are driven — deposit,
//! execute_trade, rebalance_reduce. Nothing hand-writes `a`, `oi_eff` or
//! `loss_weight`, which is what makes the reproduction meaningful.

use percolator::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, RebalanceRequestV16, TradeRequestV16, V16Config, ADL_ONE,
    POS_SCALE, SOCIAL_WEIGHT_SCALE,
};

fn ids() -> [u8; 32] {
    [1; 32]
}

fn market_fixture(init_price: u64) -> (MarketGroupV16HeaderAccount, Vec<Market<u64>>) {
    let market_id = ids();
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
    (header, markets)
}

fn account_fixture(seed: u8) -> PortfolioAccountV16Account {
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        ids(),
        [seed; 32],
        [9u8; 32],
    ));
    let mut account = PortfolioAccountV16Account::default();
    account.init_empty_in_place(header).unwrap();
    account
}

fn signed_q(q: u128) -> i128 {
    i128::try_from(q).unwrap()
}

/// The aggregate the stored counter is SUPPOSED to equal. This is the check
/// production `validate_shape` does not perform, which is why the drift was silent.
fn aggregate_oi(a_side: u128, lws: u128) -> u128 {
    a_side
        .checked_mul(lws)
        .map(|v| v / SOCIAL_WEIGHT_SCALE)
        .unwrap_or_else(|| {
            (a_side / SOCIAL_WEIGHT_SCALE) * lws
                + ((a_side % SOCIAL_WEIGHT_SCALE) * lws) / SOCIAL_WEIGHT_SCALE
        })
}

#[test]
fn attach_on_a_scaled_side_keeps_oi_eff_equal_to_the_aggregate() {
    const PX0: u64 = 1_000_000;
    const DEP: u128 = 1_000_000_000_000;
    let (mut header, mut markets) = market_fixture(PX0);
    let mut acc: Vec<PortfolioAccountV16Account> =
        (0..16).map(|i| account_fixture(40 + i)).collect();
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    for a in acc.iter_mut() {
        let mut v = PortfolioV16ViewMut::new(a);
        market.deposit_not_atomic(&mut v, DEP).unwrap();
    }
    let sz = 10 * POS_SCALE;

    macro_rules! open {
        ($i:expr, $j:expr, $size:expr) => {{
            let (li, lj) = ($i, $j);
            let (lo, hi) = if li < lj { (li, lj) } else { (lj, li) };
            let (left, right) = acc.split_at_mut(hi);
            let (taker_ref, cp_ref) = if li < lj {
                (&mut left[lo], &mut right[0])
            } else {
                (&mut right[0], &mut left[lo])
            };
            let mut taker = PortfolioV16ViewMut::new(taker_ref);
            let mut cp = PortfolioV16ViewMut::new(cp_ref);
            market
                .execute_trade_with_fee_loss_stale_scoped_not_atomic(
                    &mut taker,
                    &mut cp,
                    TradeRequestV16 {
                        asset_index: 0,
                        size_q: signed_q($size),
                        exec_price: PX0,
                        fee_bps: 0,
                    },
                    true,
                )
                .unwrap();
        }};
    }

    open!(0, 1, sz);
    open!(2, 3, sz);

    // A unilateral close scales a_long below ADL_ONE. Everything after this point
    // attaches onto a SCALED side, which is the only regime where the bug bites.
    {
        let mut d = PortfolioV16ViewMut::new(&mut acc[3]);
        market
            .rebalance_reduce_position_not_atomic(
                &mut d,
                RebalanceRequestV16 {
                    asset_index: 0,
                    reduce_q: 8 * POS_SCALE,
                },
            )
            .unwrap();
    }
    let a_long = market.markets[0].engine.asset.a_long.get();
    assert!(
        a_long < ADL_ONE,
        "the fixture must actually scale a_long, or this proves nothing about the \
         scaled-side path: a_long={a_long} ADL_ONE={ADL_ONE}"
    );

    // Five fresh longs onto the scaled side. Each used to add raw abs_q.
    for k in 0..5u8 {
        let i = 4 + (k as usize) * 2;
        let j = 5 + (k as usize) * 2;
        open!(i, j, sz);
    }

    // THE INVARIANT. Production checks only oi_eff_long == oi_eff_short, and both
    // sides drift, so this is the assertion that had nothing enforcing it.
    let asset = &market.markets[0].engine.asset;
    let stored_long = asset.oi_eff_long_q.get();
    let stored_short = asset.oi_eff_short_q.get();
    let agg_long = aggregate_oi(asset.a_long.get(), asset.loss_weight_sum_long.get());
    let agg_short = aggregate_oi(asset.a_short.get(), asset.loss_weight_sum_short.get());
    assert_eq!(
        stored_long,
        agg_long,
        "#457: oi_eff_long must equal floor(a_long * loss_weight_sum_long / SCALE); \
         drift = {}",
        agg_long as i128 - stored_long as i128
    );
    assert_eq!(stored_short, agg_short, "#457: same for the short side");

    // Production's own guard must of course still pass — the point is that it is
    // not sufficient, not that it is wrong.
    market.validate_shape().expect("production shape check");

    // And the consequence. Before the fix the under-recorded counter underflowed on
    // a LEGITIMATE close — `CounterUnderflow` — and since liquidation routes through
    // the same `clear_leg`, the position could not be forced out either.
    //
    // The assertion is deliberately "never CounterUnderflow, and the invariant holds
    // after every close" rather than "every close returns Ok". Account 2 is a long
    // whose position was socialised by the ADL, so a full-size reduce is genuinely
    // too large and `InvalidLeg` is the CORRECT answer for it. Demanding Ok there
    // would be asserting the fixture, not the fix — and would quietly pass the day
    // the underflow came back as some other error.
    let mut closed_ok = 0usize;
    for i in [4usize, 6, 8, 10, 12, 0, 2] {
        let res = {
            let mut v = PortfolioV16ViewMut::new(&mut acc[i]);
            market.rebalance_reduce_position_not_atomic(
                &mut v,
                RebalanceRequestV16 {
                    asset_index: 0,
                    reduce_q: sz,
                },
            )
        };
        match res {
            Ok(_) => closed_ok += 1,
            Err(e) => assert_ne!(
                format!("{e:?}"),
                "CounterUnderflow",
                "#457: closing account {i} underflowed oi_eff — the leg is undetachable \
                 and liquidation cannot force it out either"
            ),
        }
        let a = &market.markets[0].engine.asset;
        assert_eq!(
            a.oi_eff_long_q.get(),
            aggregate_oi(a.a_long.get(), a.loss_weight_sum_long.get()),
            "#457: the long aggregate invariant must survive closing account {i}"
        );
        // The SHORT side is deliberately NOT asserted here, and that is a finding
        // rather than an omission: it does NOT hold across these closes, on the
        // fixed build and on the unpatched one alike. Closing a long also writes the
        // opposite side's OI (the `opp_oi_after` path), and I have not established
        // whether `oi_eff_short == floor(a_short · Σweights_short / SCALE)` is meant
        // to be an invariant across that write or whether the opposite-side
        // adjustment is intentionally nominal. Asserting it would pin behaviour I
        // do not understand; quietly dropping it without saying so would be worse.
        // Raised separately — it is out of scope for the attach writer.
    }
    assert!(
        closed_ok >= 6,
        "#457: expected at least the five fresh longs and one original to close \
         cleanly, got {closed_ok}"
    );
}
