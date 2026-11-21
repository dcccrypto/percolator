//! E2E Kani proof (c): senior solvency after settlement through the REAL refresh path, with and
//! without bankruptcy.
//!
//! World (REAL public operations; the wrapper-owned raw oracle target is written onto the slot
//! after each accrual exactly as tests/liquidations_edge.rs does): one asset at P0 = 100
//! (mm = im = 10%, max move 5%/slot, no fees).
//!   1. `deposit_not_atomic(long, D)` with SYMBOLIC D; `deposit_not_atomic(short, 1_000)`;
//!   2. `execute_trade_with_fee_loss_stale_scoped_not_atomic`: long +10 units at 100;
//!   3. concrete price walk 100 -> 85 within the 5%/slot cap via `accrue_asset_to_not_atomic`
//!      (one call per slot; the long is NOT touched in between, so its loss accumulates);
//!   4. `full_account_refresh_not_atomic(long)` -- the real path that settles the K/F mark into
//!      pnl and then pays the negative pnl from principal
//!      (`settle_negative_pnl_from_principal_core_not_atomic`).
//!
//! Spec (independent, written here): loss = 10 * (100 - 85) = 150; paid = min(D, 150).
//! Statement. For every D in 100..=227:
//!   * the refresh succeeds;
//!   * long capital' == D - paid; long pnl' == paid - 150 (<= 0); short capital unchanged;
//!   * vault unchanged; insurance unchanged; c_tot' == c_tot - paid;
//!   * SENIOR SOLVENCY: c_tot' + insurance' <= vault', and the senior surplus
//!     vault' - c_tot' - insurance' == paid (exactly the principal moved toward the winner);
//!   * D < 150 (bankrupt): negative_pnl_account_count == 1 and the bankruptcy h-lock is active;
//!     D >= 150: negative_pnl_account_count == 0.
//!
//! Bounds: 1 asset, 2 accounts, 1 leg each, concrete trade and walk, D u8-sized.
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
    ProvenanceHeaderV16Account, TradeRequestV16, V16Config, V16PodU64, POS_SCALE,
};

const P0: u64 = 100;
const P1: u64 = 85;
const MOVE_BPS: u64 = 500;

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
    cfg.max_price_move_bps_per_slot = MOVE_BPS;
    cfg.min_nonzero_mm_req = 2;
    cfg.min_nonzero_im_req = 3;
    cfg
}

fn check_settle_preserves_senior_solvency(d: u128) {
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
    g.deposit_not_atomic(&mut lv, d).unwrap();
    g.deposit_not_atomic(&mut sv, 1_000).unwrap();
    let req = TradeRequestV16 {
        asset_index: 0,
        size_q: (10 * POS_SCALE) as i128,
        exec_price: P0,
        fee_bps: 0,
    };
    g.execute_trade_with_fee_loss_stale_scoped_not_atomic(&mut lv, &mut sv, req, true)
        .unwrap();
    // Concrete walk 100 -> 95 -> 91 -> 87 -> 85 (each step <= 5%).
    let mut price = P0;
    let mut slot = 1u64;
    while price != P1 {
        let step = (price * MOVE_BPS / 10_000).max(1);
        price = price.saturating_sub(step).max(P1);
        slot += 1;
        g.accrue_asset_to_not_atomic(0, slot, price, 0, true).unwrap();
        g.markets[0].engine.asset.raw_oracle_target_price = V16PodU64::new(price);
    }

    let vault0 = g.header.vault.get();
    let c_tot0 = g.header.c_tot.get();
    let ins0 = g.header.insurance.get();
    let short_cap0 = sv.header.capital.get();

    // The real refresh path (K/F settle + negative-pnl principal settlement).
    let refreshed = g.full_account_refresh_not_atomic(&mut lv);
    assert!(refreshed.is_ok());

    let loss: u128 = 10 * (P0 - P1) as u128; // 150
    let paid = if d < loss { d } else { loss };
    assert!(lv.header.capital.get() == d - paid);
    assert!(lv.header.pnl.get() == paid as i128 - loss as i128);
    assert!(sv.header.capital.get() == short_cap0);
    assert!(g.header.vault.get() == vault0);
    assert!(g.header.insurance.get() == ins0);
    assert!(g.header.c_tot.get() == c_tot0 - paid);
    // Senior solvency, and its exact margin.
    let senior = g.header.c_tot.get() + g.header.insurance.get();
    assert!(senior <= g.header.vault.get());
    assert!(g.header.vault.get() - senior == paid);
    if d < loss {
        assert!(g.header.negative_pnl_account_count.get() == 1);
        assert!(g.header.bankruptcy_hlock_active != 0);
    } else {
        assert!(g.header.negative_pnl_account_count.get() == 0);
    }
    cover!(d < loss, "bankrupt: partial principal settlement, deficit remains");
    cover!(d == loss, "exact: capital covers the loss to the atom");
    cover!(d > loss, "solvent: full settlement, capital left");
}

#[cfg(kani)]
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_e2e_settle_through_refresh_preserves_senior_solvency() {
    let d: u8 = kani::any();
    kani::assume(d >= 100 && d <= 227);
    check_settle_preserves_senior_solvency(d as u128);
}

#[cfg(not(kani))]
#[test]
fn native_exhaustive_settle_domain() {
    for d in 100..=227u128 {
        check_settle_preserves_senior_solvency(d);
    }
}
