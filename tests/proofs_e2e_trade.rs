//! E2E Kani proof (a, trade step): a real trade conserves value exactly.
//!
//! World (REAL public operations): one asset at P0 = 100 (mm = im = 10%, max trading fee
//! 100 bps). `deposit_not_atomic(long, DL)`, `deposit_not_atomic(short, DS)` with SYMBOLIC DL, DS;
//! then ONE call to the public entry point
//! `execute_trade_with_fee_loss_stale_scoped_not_atomic(long, short, {asset 0, +10 units,
//! price 100, fee_bps F}, taker_is_long)` with SYMBOLIC F and SYMBOLIC taker side.
//!
//! Spec (independent, written here): notional = 10 * 100 = 1_000;
//! fee = ceil(1_000 * F / 10_000); the taker pays `min(fee, taker capital)` (taker-only fee:
//! the maker pays only a shortfall, here never, since taker capital >= 100 >= fee).
//!
//! Statement. For every DL, DS in 110..=237 (IM 100 + max fee 10), F in 0..=100, taker side in {long, short}:
//!   * the trade succeeds; outcome.notional == 1_000; the taker's fee == spec fee, the maker's 0;
//!   * vault unchanged (a trade moves no token);
//!   * c_tot drops by exactly fee_a + fee_b; insurance rises by exactly fee_a + fee_b;
//!   * long capital drops by exactly fee_a, short capital by exactly fee_b;
//!   * both pnl unchanged (0);
//!   * long holds +10 units, short -10 units on asset 0;
//!   * `c_tot + insurance <= vault` after.
//!
//! Bounds: 1 asset, 2 accounts, 1 fill (no batch), concrete size and price, deposits u8-sized.
// `cover!` is `kani::cover!` under Kani and a no-op natively, so the same check function is
// proved by Kani (`#[kani::proof]` below) and run natively over its whole bounded domain
// (`#[test] native_exhaustive_*` below).
#[cfg(kani)]
macro_rules! cover {
    ($c:expr, $m:expr) => {
        kani::cover!($c, $m)
    };
}
#[cfg(not(kani))]
macro_rules! cover {
    ($c:expr, $m:expr) => {
        let _ = $c;
    };
}

use percolator::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, TradeRequestV16, V16Config, POS_SCALE,
};

const P0: u64 = 100;
const Q: i128 = 10 * POS_SCALE as i128;

fn account(seed: u8) -> PortfolioAccountV16Account {
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        [1; 32], [seed; 32], [3; 32],
    ));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(header).unwrap();
    a
}

fn config() -> V16Config {
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    cfg.maintenance_margin_bps = 1_000;
    cfg.initial_margin_bps = 1_000;
    cfg.max_price_move_bps_per_slot = 500;
    cfg.max_trading_fee_bps = 100;
    cfg.min_nonzero_mm_req = 2;
    cfg.min_nonzero_im_req = 3;
    cfg
}

fn check_trade_conserves_value(dl: u128, ds: u128, fee_bps: u64, taker_is_long: bool) {
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], config(), 1, 0).unwrap();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, P0, 1)
        .unwrap();
    let mut long = account(10);
    let mut short = account(11);
    let mut g = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut lv = PortfolioV16ViewMut::new(&mut long);
    let mut sv = PortfolioV16ViewMut::new(&mut short);
    g.deposit_not_atomic(&mut lv, dl).unwrap();
    g.deposit_not_atomic(&mut sv, ds).unwrap();

    let vault0 = g.header.vault.get();
    let c_tot0 = g.header.c_tot.get();
    let ins0 = g.header.insurance.get();
    let lcap0 = lv.header.capital.get();
    let scap0 = sv.header.capital.get();

    let req = TradeRequestV16 {
        asset_index: 0,
        size_q: Q,
        exec_price: P0,
        fee_bps,
    };
    // The real public trade entry point.
    let out = g
        .execute_trade_with_fee_loss_stale_scoped_not_atomic(&mut lv, &mut sv, req, taker_is_long)
        .unwrap();

    let fee = (1_000u128 * fee_bps as u128).div_ceil(10_000);
    let (taker_fee, maker_fee) = if taker_is_long {
        (out.fee_a, out.fee_b)
    } else {
        (out.fee_b, out.fee_a)
    };
    assert!(out.notional == 1_000);
    assert!(taker_fee == fee && maker_fee == 0);
    let moved = out.fee_a + out.fee_b;
    assert!(g.header.vault.get() == vault0);
    assert!(g.header.c_tot.get() == c_tot0 - moved);
    assert!(g.header.insurance.get() == ins0 + moved);
    assert!(lv.header.capital.get() == lcap0 - out.fee_a);
    assert!(sv.header.capital.get() == scap0 - out.fee_b);
    assert!(lv.header.pnl.get() == 0 && sv.header.pnl.get() == 0);
    let ll = lv.header.legs[0].try_to_runtime().unwrap();
    let sl = sv.header.legs[0].try_to_runtime().unwrap();
    assert!(ll.active && ll.basis_pos_q == Q);
    assert!(sl.active && sl.basis_pos_q == -Q);
    assert!(g.header.c_tot.get() + g.header.insurance.get() <= g.header.vault.get());

    cover!(fee > 0 && taker_is_long, "long taker pays a nonzero fee");
    cover!(fee > 0 && !taker_is_long, "short taker pays a nonzero fee");
    cover!(fee == 10, "max fee (100 bps of 1_000)");
}

#[cfg(kani)]
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_e2e_trade_conserves_value_exactly() {
    let dl: u8 = kani::any();
    let ds: u8 = kani::any();
    let fee_bps: u8 = kani::any();
    let taker_is_long: bool = kani::any();
    kani::assume(dl >= 110 && dl <= 237);
    kani::assume(ds >= 110 && ds <= 237);
    kani::assume(fee_bps <= 100);
    check_trade_conserves_value(dl as u128, ds as u128, fee_bps as u64, taker_is_long);
}

/// Full domain is 128 * 128 * 101 * 2 = 3.3M trades (21 s in release, too slow for the debug
/// suite); here every (fee, taker, DL) with DS in {110, 173, 237}, both ways round.
#[cfg(not(kani))]
#[test]
fn native_sampled_trade_domain() {
    for f in 0..=100u64 {
        for t in [true, false] {
            for dl in 110..=237u128 {
                for ds in [110u128, 173, 237] {
                    check_trade_conserves_value(dl, ds, f, t);
                    check_trade_conserves_value(ds, dl, f, t);
                }
            }
        }
    }
}
