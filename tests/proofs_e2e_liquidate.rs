//! E2E Kani proof (b): a solvent account is never liquidated through the FULL public
//! `liquidate_account_not_atomic` path, and an insolvent one is (exactness, so the refusal is not
//! a gate that refuses everything).
//!
//! World (built by REAL public operations, nothing poked except the wrapper-owned raw oracle
//! target, exactly as tests/liquidations_edge.rs does): one asset at P0 = 100, config
//! mm = im = 10%, max move 5%/slot, liquidation fee 1% capped at 1_000, min_nonzero_mm_req 2.
//!   1. `deposit_not_atomic(target, 100)`, `deposit_not_atomic(counterparty, 10_000)`;
//!   2. `execute_trade_with_fee_loss_stale_scoped_not_atomic`: target long 10 units at 100
//!      (notional 1_000, IM 100), fee 0;
//!   3. `deposit_not_atomic(target, extra)` with SYMBOLIC `extra`;
//!   4. `accrue_asset_to_not_atomic(0, 2, P1, 0, true)` (mark move 100 -> P1), raw target := P1;
//!   5. `liquidate_account_not_atomic(target, asset 0)` -- the full path: mode/asset checks,
//!      refresh + certify, trigger gate, sizing, close, fee, waterfall.
//!
//! Spec formula (independent of the engine, written here): after the move the target's
//! equity is `100 + extra - 10 * (100 - P1)` and its maintenance requirement is
//! `max(ceil(10 * P1 * 10%), 2) = P1`. solvent := equity >= requirement.
//!
//! Statement. For every `extra` in the range below:
//!   solvent  ==> result == Err(NonProgress), the target's leg is still active with basis
//!                exactly +10 units, its capital is exactly the spec equity (only the mark loss
//!                was settled: no fee, no close), and vault and insurance are unchanged;
//!   !solvent ==> result is Ok with 0 < closed_q (the account is liquidated).
//! In both cases vault is unchanged (no external flow) and `c_tot + insurance <= vault`.
//!
//! Bounds. One asset slot, two accounts, one leg; trade size / prices concrete; `extra` symbolic
//! in 0..=63 (the solvency boundary is extra = 45 at P1 = 95). Variant 2 makes P1 symbolic in
//! 95..=100 as well. Why not wider: every symbolic bit flows through the refresh / K-index /
//! margin wide-math; see REPORT.md for timings.
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
    EngineAssetSlotV16Account, LiquidationRequestV16, Market, MarketGroupV16HeaderAccount,
    MarketGroupV16ViewMut, PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, TradeRequestV16, V16Config, V16Error, V16PodU64, POS_SCALE,
};

const P0: u64 = 100;
const Q_UNITS: u128 = 10;

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
    cfg.liquidation_fee_bps = 100;
    cfg.liquidation_fee_cap = 1_000;
    cfg.min_nonzero_mm_req = 2;
    cfg.min_nonzero_im_req = 3;
    cfg
}

fn check_solvent_never_liquidated(extra: u128, p1: u64) {
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], config(), 1, 0).unwrap();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, P0, 1)
        .unwrap();
    let mut tgt = account(10);
    let mut cp = account(11);
    let mut g = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut tv = PortfolioV16ViewMut::new(&mut tgt);
    let mut cv = PortfolioV16ViewMut::new(&mut cp);

    g.deposit_not_atomic(&mut tv, 100).unwrap();
    g.deposit_not_atomic(&mut cv, 10_000).unwrap();
    let req = TradeRequestV16 {
        asset_index: 0,
        size_q: (Q_UNITS * POS_SCALE) as i128,
        exec_price: P0,
        fee_bps: 0,
    };
    g.execute_trade_with_fee_loss_stale_scoped_not_atomic(&mut tv, &mut cv, req, true)
        .unwrap();
    g.deposit_not_atomic(&mut tv, extra).unwrap();
    g.accrue_asset_to_not_atomic(0, 2, p1, 0, true).unwrap();
    g.markets[0].engine.asset.raw_oracle_target_price = V16PodU64::new(p1);

    let vault_before = g.header.vault.get();
    let insurance_before = g.header.insurance.get();

    // Independent spec formula.
    let loss = Q_UNITS * (P0 - p1) as u128;
    let equity = 100 + extra - loss; // never negative in this range
    let mm = core::cmp::max((Q_UNITS * p1 as u128).div_ceil(10), 2);
    let solvent = equity >= mm;

    // The real, full liquidation entry point.
    let result = g.liquidate_account_not_atomic(&mut tv, LiquidationRequestV16 { asset_index: 0 });

    let leg = tv.header.legs[0].try_to_runtime().unwrap();
    if solvent {
        assert!(result == Err(V16Error::NonProgress));
        assert!(leg.active);
        assert!(leg.basis_pos_q == (Q_UNITS * POS_SCALE) as i128);
        assert!(tv.header.capital.get() == equity);
        assert!(g.header.insurance.get() == insurance_before);
        cover!(equity == mm, "exact maintenance boundary: refused");
        cover!(equity > mm, "strictly solvent: refused");
    } else {
        let out = result.unwrap();
        assert!(out.closed_q > 0);
        cover!(equity + 1 == mm, "one atom below maintenance: liquidated");
        cover!(out.fee_charged > 0, "insolvent close charges a liquidation fee");
    }
    assert!(g.header.vault.get() == vault_before);
    assert!(g.header.c_tot.get() + g.header.insurance.get() <= g.header.vault.get());
}

#[cfg(kani)]
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_e2e_liquidate_solvent_account_is_refused_insolvent_is_closed() {
    let extra: u8 = kani::any();
    kani::assume(extra <= 63);
    check_solvent_never_liquidated(extra as u128, 95);
}

#[cfg(kani)]
#[kani::proof]
#[kani::unwind(34)]
#[kani::solver(cadical)]
fn proof_e2e_liquidate_solvent_refused_symbolic_price() {
    let extra: u8 = kani::any();
    let p1: u8 = kani::any();
    kani::assume(extra <= 63);
    kani::assume((95..=100).contains(&p1));
    check_solvent_never_liquidated(extra as u128, p1 as u64);
}

#[cfg(not(kani))]
#[test]
fn native_exhaustive_liquidate_domain() {
    for p1 in 95..=100u64 {
        for extra in 0..=63u128 {
            check_solvent_never_liquidated(extra, p1);
        }
    }
}
