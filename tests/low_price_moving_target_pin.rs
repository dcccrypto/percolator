//! A target that changes on every accrual pins a low mark: the first step after a target change
//! must start from remainder 0, and at a low anchor `anchor * cap < MAX_MARGIN_BPS` the step
//! floors to 0, so the mark never moves while the oracle keeps reporting a price far away.
use percolator::{
    canonical_accrual_price_step_v16, AccrualStepV16, AssetStateV16, EngineAssetSlotV16Account,
    Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, PortfolioAccountV16Account,
    PortfolioV16ViewMut, ProvenanceHeaderV16, ProvenanceHeaderV16Account, TradeRequestV16,
    V16Config, POS_SCALE,
};

const P0: u64 = 50;
const CAP_BPS: u64 = 100;

fn account(seed: u8) -> PortfolioAccountV16Account {
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        [1; 32], [seed; 32], [3; 32],
    ));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(header).unwrap();
    a
}

fn asset(markets: &[Market<u64>]) -> AssetStateV16 {
    markets[0].engine.asset.try_to_runtime().unwrap()
}

fn exposed_low_price_asset() -> (MarketGroupV16HeaderAccount, Vec<Market<u64>>) {
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    cfg.max_price_move_bps_per_slot = CAP_BPS;
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut markets = vec![Market::new(0, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, P0, 1)
        .unwrap();
    let (mut l, mut s) = (account(10), account(11));
    {
        let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut lv = PortfolioV16ViewMut::new(&mut l);
        let mut sv = PortfolioV16ViewMut::new(&mut s);
        m.deposit_not_atomic(&mut lv, 1_000_000_000).unwrap();
        m.deposit_not_atomic(&mut sv, 1_000_000_000).unwrap();
        m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut lv,
            &mut sv,
            TradeRequestV16 {
                asset_index: 0,
                size_q: POS_SCALE as i128,
                exec_price: P0,
                fee_bps: 0,
            },
            true,
        )
        .unwrap();
    }
    let a = asset(&markets);
    assert!(a.oi_eff_long_q != 0 && a.oi_eff_short_q != 0);
    (header, markets)
}

/// One-slot accruals toward `target(i)`. The first step of each path starts from the carried
/// remainder unless the target changed and is now on the other side of the mark (or there was
/// no target yet), in which case it starts from 0. Returns the mark after `slots` slots.
fn run(target: impl Fn(u64) -> u64, slots: u64) -> u64 {
    let (mut header, mut markets) = exposed_low_price_asset();
    let mut carried = 0u16;
    for i in 0..slots {
        let a = asset(&markets);
        let t = target(i);
        let changed = a.raw_oracle_target_price != t;
        let anchor = if changed {
            a.effective_price
        } else {
            a.fund_px_last
        };
        let old = a.raw_oracle_target_price;
        let mark = a.effective_price;
        let same_side = old != 0 && ((old > mark && t > mark) || (old < mark && t < mark));
        let before = if changed && !same_side { 0 } else { carried };
        let (p, after) =
            canonical_accrual_price_step_v16(a.effective_price, t, anchor, CAP_BPS, true, before)
                .unwrap();
        let step = AccrualStepV16 {
            effective_price: p,
            funding_rate_e9: 0,
            price_move_remainder_before_bps_num: before,
            price_move_remainder_after_bps_num: after,
        };
        let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        m.accrue_asset_path_to_not_atomic(0, a.slot_last + 1, t, &[step], true)
            .unwrap();
        carried = after;
    }
    asset(&markets).effective_price
}

#[test]
fn fixed_target_moves_a_low_mark() {
    // 50 * 100 bps = 5000 / 10_000 per slot: one atom every two slots, as the carry intends
    let mark = run(|_| 10, 40);
    assert!(mark < P0, "fixed target: mark {mark}");
}

#[test]
fn moving_target_must_still_move_a_low_mark() {
    // the oracle reports 10 or 11 on alternate slots: same direction, 80% below the mark
    let mark = run(|i| 10 + (i % 2), 400);
    assert!(
        mark < P0,
        "target moved every slot for 400 slots, mark still {mark}"
    );
}

#[test]
fn a_reversed_target_still_restarts_the_carry() {
    let (mut header, mut markets) = exposed_low_price_asset();
    // one step down toward 10 leaves a carry of 5000
    let a = asset(&markets);
    let (p, r) = canonical_accrual_price_step_v16(P0, 10, P0, CAP_BPS, true, 0).unwrap();
    assert_eq!((p, r), (P0, 5_000));
    {
        let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        m.accrue_asset_path_to_not_atomic(
            0,
            a.slot_last + 1,
            10,
            &[AccrualStepV16 {
                effective_price: p,
                funding_rate_e9: 0,
                price_move_remainder_before_bps_num: 0,
                price_move_remainder_after_bps_num: r,
            }],
            true,
        )
        .unwrap();
    }
    // the target flips above the mark: the carry from the downward move must not be reused
    let a = asset(&markets);
    let (p, r2) = canonical_accrual_price_step_v16(P0, 90, P0, CAP_BPS, true, r).unwrap();
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let refused = m.accrue_asset_path_to_not_atomic(
        0,
        a.slot_last + 1,
        90,
        &[AccrualStepV16 {
            effective_price: p,
            funding_rate_e9: 0,
            price_move_remainder_before_bps_num: r,
            price_move_remainder_after_bps_num: r2,
        }],
        true,
    );
    assert!(
        refused.is_err(),
        "carry reused across a reversal: {refused:?}"
    );
}
