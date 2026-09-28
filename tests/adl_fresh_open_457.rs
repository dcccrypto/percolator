//! dcccrypto/percolator-prog#457 regression: a fresh open on a side whose A was
//! scaled by a prior unilateral close must not be able to drift `oi_eff` and
//! later strand a co-side holder behind `CounterUnderflow`.
//!
//! Two adopted upstream commits close it, both in the pinned/deployed engine
//! `c141d47f`:
//!   * 3427c613 (upstream 6ae709e0 "Prevent ADL basis reissue"): any
//!     risk-increasing change -- attach, flip, enlarge -- is refused with
//!     `LockActive` while either side's A != ADL_ONE, so the drift source (the
//!     raw-`abs_q` attach on a scaled side) is unreachable. Every attach runs at
//!     A == ADL_ONE, where raw abs_q equals the aggregate contribution exactly.
//!   * 1ca3a65d (upstream 6f36972d "post-ADL effective position accounting"):
//!     clear/resize/retain subtract the leg's effective quantity
//!     ceil(raw * A / a_basis) instead of a nominal basis.
//!
//! The fixture is the reporter's sequence, driven only through real mutators:
//! two 10-lot pairs, one short owner-rebalances 8 lots away (quantity ADL
//! scales a_long to 0.6), then five fresh 10-lot longs are attempted.
use percolator::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, RebalanceRequestV16, TradeRequestV16, V16Config, V16Error,
};
use percolator::{ADL_ONE, POS_SCALE};

const PX0: u64 = 1_000_000;
const DEP: u128 = 1_000_000_000_000;

fn market_fixture() -> (MarketGroupV16HeaderAccount, Vec<Market<u64>>) {
    let cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut markets = vec![Market::new(0u64, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, PX0, 1)
        .unwrap();
    (header, markets)
}

fn account_fixture(seed: u8) -> PortfolioAccountV16Account {
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        [1; 32], [seed; 32], [9u8; 32],
    ));
    let mut account = PortfolioAccountV16Account::default();
    account.init_empty_in_place(header).unwrap();
    account
}

fn trade(
    market: &mut MarketGroupV16ViewMut<'_, u64>,
    acc: &mut [PortfolioAccountV16Account],
    taker: usize,
    maker: usize,
    size_q: i128,
) -> Result<(), V16Error> {
    assert_ne!(taker, maker);
    let (lo, hi) = (taker.min(maker), taker.max(maker));
    let (left, right) = acc.split_at_mut(hi);
    let (t, m) = if taker < maker {
        (&mut left[lo], &mut right[0])
    } else {
        (&mut right[0], &mut left[lo])
    };
    let mut t = PortfolioV16ViewMut::new(t);
    let mut m = PortfolioV16ViewMut::new(m);
    market
        .execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut t,
            &mut m,
            TradeRequestV16 {
                asset_index: 0,
                size_q,
                exec_price: PX0,
                fee_bps: 0,
            },
            true,
        )
        .map(|_| ())
}

fn reduce(
    market: &mut MarketGroupV16ViewMut<'_, u64>,
    acc: &mut PortfolioAccountV16Account,
    reduce_q: u128,
) -> Result<(), V16Error> {
    let mut v = PortfolioV16ViewMut::new(acc);
    market
        .rebalance_reduce_position_not_atomic(
            &mut v,
            RebalanceRequestV16 {
                asset_index: 0,
                reduce_q,
            },
        )
        .map(|_| ())
}

/// (oi_long, oi_short, lws_long, lws_short, count_long, count_short)
fn side_state(market: &MarketGroupV16ViewMut<'_, u64>) -> (u128, u128, u128, u128, u64, u64) {
    let a = &market.markets[0].engine.asset;
    (
        a.oi_eff_long_q.get(),
        a.oi_eff_short_q.get(),
        a.loss_weight_sum_long.get(),
        a.loss_weight_sum_short.get(),
        a.stored_pos_count_long.get(),
        a.stored_pos_count_short.get(),
    )
}

#[test]
fn fresh_open_on_adl_scaled_side_is_refused_and_co_side_unwinds_cleanly() {
    let (mut header, mut markets) = market_fixture();
    let mut acc: Vec<PortfolioAccountV16Account> =
        (0..16).map(|i| account_fixture(40 + i)).collect();
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    for a in acc.iter_mut() {
        let mut v = PortfolioV16ViewMut::new(a);
        market.deposit_not_atomic(&mut v, DEP).unwrap();
    }
    let sz = 10 * POS_SCALE;
    let szi = i128::try_from(sz).unwrap();

    trade(&mut market, &mut acc, 0, 1, szi).unwrap();
    trade(&mut market, &mut acc, 2, 3, szi).unwrap();
    reduce(&mut market, &mut acc[3], 8 * POS_SCALE).unwrap();
    let a_long = market.markets[0].engine.asset.a_long.get();
    assert!(
        a_long < ADL_ONE,
        "fixture must scale a_long or it proves nothing: a_long={a_long}"
    );
    market.validate_shape().unwrap();
    let before = side_state(&market);
    assert_eq!(before.0, before.1, "oi_eff pair must stay balanced");

    // The #457 drift source: fresh 10-lot longs (and shorts) on the scaled
    // market. Each must be refused atomically -- no OI, weight or count write.
    for k in 0..5usize {
        let (i, j) = (4 + 2 * k, 5 + 2 * k);
        assert_eq!(
            trade(&mut market, &mut acc, i, j, szi),
            Err(V16Error::LockActive),
            "fresh long #{k} on an A-scaled side must be refused"
        );
        assert_eq!(
            trade(&mut market, &mut acc, i, j, -szi),
            Err(V16Error::LockActive),
            "fresh short #{k} on an A-scaled market must be refused"
        );
        assert_eq!(side_state(&market), before, "refused open wrote side state");
    }
    // Enlarging an existing leg is the other way to add basis at a scaled A.
    assert_eq!(
        trade(&mut market, &mut acc, 0, 4, szi),
        Err(V16Error::LockActive),
        "enlarging a surviving long must be refused while A is scaled"
    );
    assert_eq!(side_state(&market), before);

    // Unwind the co-side longs -- the positions #457 showed stranded. No close
    // may hit CounterUnderflow and the pair must stay balanced and shape-valid.
    for i in [0usize, 2usize] {
        let r = reduce(&mut market, &mut acc[i], sz);
        assert_ne!(
            r,
            Err(V16Error::CounterUnderflow),
            "close of original long acct {i} bricked by CounterUnderflow"
        );
        market.validate_shape().unwrap();
        let s = side_state(&market);
        assert_eq!(s.0, s.1, "oi_eff pair unbalanced after closing acct {i}");
    }
    let s = side_state(&market);
    assert_eq!(
        (s.0, s.1),
        (0, 0),
        "every long was closed, so no effective OI may remain on either side"
    );
}
