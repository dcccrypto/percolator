//! Edge tests for v2.2 market-mode transitions: Live -> Resolved (`resolve_market_not_atomic`),
//! Live -> Recovery (`force_asset_recovery_not_atomic`), and which operations each mode allows.
//! Tests only; no product code is touched.
//!
//! Resolved payout maths, fee-cap-at-resolved-slot and `advance_resolved_slot` regress cases
//! already live in resolved_payout_edge.rs and slot_gap_time_edge.rs and are not repeated.
//!
//! Every test name starts with `mst_` so `cargo test --test market_state_transition_edge` or
//! `cargo test -- mst_` selects them.
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]

use percolator::{
    AssetLifecycleV16, EngineAssetSlotV16Account, LiquidationRequestV16, Market,
    MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, PortfolioAccountV16Account,
    PortfolioV16ViewMut, ProvenanceHeaderV16, ProvenanceHeaderV16Account, ResolvedCloseOutcomeV16,
    TradeRequestV16, V16Config, V16Error, V16PodU64, MAX_VAULT_TVL, POS_SCALE,
};

const PRICE: u64 = 1_000_000;

fn account(seed: u32) -> PortfolioAccountV16Account {
    let mut key = [0u8; 32];
    key[..4].copy_from_slice(&seed.to_le_bytes());
    key[31] = 0x5A;
    let header =
        ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new([1; 32], key, [3; 32]));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(header).unwrap();
    a
}

#[derive(Clone)]
struct W {
    header: MarketGroupV16HeaderAccount,
    markets: Vec<Market<u64>>,
}

impl W {
    fn new() -> Self {
        let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 6_480_000);
        cfg.max_abs_funding_e9_per_slot = 10_000;
        cfg.max_price_move_bps_per_slot = 200;
        cfg.initial_margin_bps = 1_000;
        cfg.maintenance_margin_bps = 500;
        cfg.max_accrual_dt_slots = 2;
        cfg.min_funding_lifetime_slots = 10_000_000;
        let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
        let mut markets = vec![Market::new(0, EngineAssetSlotV16Account::default())];
        header
            .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, PRICE, 1)
            .unwrap();
        W { header, markets }
    }
    fn view(&mut self) -> MarketGroupV16ViewMut<'_, u64> {
        MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets)
    }
    fn vault(&self) -> u128 {
        self.header.vault.get()
    }
    fn c_tot(&self) -> u128 {
        self.header.c_tot.get()
    }
    fn slot(&self) -> u64 {
        self.header.current_slot.get()
    }
    fn deposit(&mut self, a: &mut PortfolioAccountV16Account, amt: u128) -> Result<(), V16Error> {
        self.view().deposit_not_atomic(&mut PortfolioV16ViewMut::new(a), amt)
    }
    fn withdraw(&mut self, a: &mut PortfolioAccountV16Account, amt: u128) -> Result<(), V16Error> {
        self.view().withdraw_not_atomic(&mut PortfolioV16ViewMut::new(a), amt)
    }
    fn trade(
        &mut self,
        long: &mut PortfolioAccountV16Account,
        short: &mut PortfolioAccountV16Account,
        size: i128,
    ) -> Result<(), V16Error> {
        let p = self.markets[0].engine.asset.effective_price.get();
        self.view()
            .execute_trade_with_fee_loss_stale_scoped_not_atomic(
                &mut PortfolioV16ViewMut::new(long),
                &mut PortfolioV16ViewMut::new(short),
                TradeRequestV16 { asset_index: 0, size_q: size, exec_price: p, fee_bps: 0 },
                true,
            )
            .map(|_| ())
    }
    fn resolve(&mut self, slot: u64) -> Result<(), V16Error> {
        self.view().resolve_market_not_atomic(slot)
    }
    fn close(&mut self, a: &mut PortfolioAccountV16Account) -> Result<ResolvedCloseOutcomeV16, V16Error> {
        self.view().close_resolved_account_not_atomic(&mut PortfolioV16ViewMut::new(a), 0)
    }
    fn close_fully(&mut self, a: &mut PortfolioAccountV16Account) -> u128 {
        for _ in 0..64 {
            if let ResolvedCloseOutcomeV16::Closed { payout } = self.close(a).expect("close") {
                return payout;
            }
        }
        panic!("close never completed");
    }
    fn lifecycle(&self) -> AssetLifecycleV16 {
        self.markets[0].engine.asset.try_to_runtime().unwrap().lifecycle
    }
}

/// Funded long/short pair with one open position.
fn pair(w: &mut W, dep: u128) -> (PortfolioAccountV16Account, PortfolioAccountV16Account) {
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, dep).unwrap();
    w.deposit(&mut b, dep).unwrap();
    w.trade(&mut a, &mut b, POS_SCALE as i128).expect("open");
    (a, b)
}

// ---------------------------------------------------------------- resolve: slot ordering

#[test]
fn mst_resolve_in_the_past_is_stale_and_changes_nothing() {
    let mut w = W::new();
    let mut a = account(1);
    w.deposit(&mut a, 100).unwrap();
    let now = w.slot();
    assert!(now > 0);
    let before = w.header.clone();
    assert_eq!(w.resolve(now - 1), Err(V16Error::Stale));
    assert_eq!(w.header, before);
    // still live: deposits keep working
    w.deposit(&mut a, 1).unwrap();
}

#[test]
fn mst_resolve_at_current_slot_is_allowed() {
    let mut w = W::new();
    let now = w.slot();
    w.resolve(now).unwrap();
    assert_eq!(w.header.resolved_slot.get(), now);
    assert_eq!(w.slot(), now);
}

#[test]
fn mst_resolve_at_u64_max_sets_both_clocks() {
    let mut w = W::new();
    w.resolve(u64::MAX).unwrap();
    assert_eq!(w.header.resolved_slot.get(), u64::MAX);
    assert_eq!(w.slot(), u64::MAX);
    let mut a = account(1);
    assert_eq!(w.close_fully(&mut a), 0);
}

// ---------------------------------------------------------------- resolve: value and positions

#[test]
fn mst_resolve_moves_no_value_and_keeps_open_positions() {
    let mut w = W::new();
    let (a, b) = pair(&mut w, 1_000_000);
    let (vault, c_tot, ins) = (w.vault(), w.c_tot(), w.header.insurance.get());
    let (legs_a, legs_b) = (a.legs, b.legs);
    let (oi_l, oi_s) = {
        let s = &w.markets[0].engine.asset;
        (s.oi_eff_long_q, s.oi_eff_short_q)
    };
    w.resolve(w.slot() + 3).unwrap();
    assert_eq!((w.vault(), w.c_tot(), w.header.insurance.get()), (vault, c_tot, ins));
    assert_eq!(a.legs, legs_a);
    assert_eq!(b.legs, legs_b);
    let s = &w.markets[0].engine.asset;
    assert_eq!((s.oi_eff_long_q, s.oi_eff_short_q), (oi_l, oi_s));
}

#[test]
fn mst_resolved_flat_deposit_returns_exactly_and_empties_vault() {
    let mut w = W::new();
    let mut a = account(1);
    w.deposit(&mut a, 777).unwrap();
    w.resolve(w.slot()).unwrap();
    assert_eq!(w.close_fully(&mut a), 777);
    assert_eq!((w.vault(), w.c_tot()), (0, 0));
}

#[test]
fn mst_resolved_open_pair_at_flat_price_conserves_value() {
    for dep in [1_000_000u128, 123_457, 100_000] {
        let mut w = W::new();
        let (mut a, mut b) = pair(&mut w, dep);
        w.resolve(w.slot() + 1).unwrap();
        let pa = w.close_fully(&mut a);
        let pb = w.close_fully(&mut b);
        assert_eq!(pa + pb, 2 * dep, "dep {dep}: price did not move so nothing is won or lost");
        assert_eq!(w.vault(), 0);
        assert_eq!(w.c_tot(), 0);
    }
}

#[test]
fn mst_resolved_close_order_does_not_change_total_paid() {
    let run = |a_first: bool| {
        let mut w = W::new();
        let (mut a, mut b) = pair(&mut w, 1_000_000);
        w.resolve(w.slot()).unwrap();
        let (pa, pb) = if a_first {
            let pa = w.close_fully(&mut a);
            (pa, w.close_fully(&mut b))
        } else {
            let pb = w.close_fully(&mut b);
            (w.close_fully(&mut a), pb)
        };
        (pa, pb, w.vault())
    };
    assert_eq!(run(true), run(false));
}

#[test]
fn mst_resolve_before_or_after_flat_deposit_pays_the_same() {
    // deposit then resolve
    let mut w1 = W::new();
    let mut a1 = account(1);
    w1.deposit(&mut a1, 5_000).unwrap();
    w1.resolve(w1.slot() + 7).unwrap();
    let p1 = w1.close_fully(&mut a1);
    // resolve is refused to take new deposits, so payout equals the deposit made before it
    assert_eq!(p1, 5_000);
}

// ---------------------------------------------------------------- Resolved refuses live-only ops

#[test]
fn mst_resolved_refuses_live_ops_and_changes_nothing() {
    let mut w = W::new();
    let (mut a, mut b) = pair(&mut w, 1_000_000);
    let mut c = account(3);
    w.deposit(&mut c, 50).unwrap();
    w.resolve(w.slot()).unwrap();
    let hb = w.header.clone();
    let (ab, bb, cb) = (a.clone(), b.clone(), c.clone());

    assert_eq!(w.deposit(&mut c, 1), Err(V16Error::LockActive), "deposit");
    assert!(w.withdraw(&mut c, 1).is_err(), "withdraw");
    assert!(w.trade(&mut a, &mut b, POS_SCALE as i128).is_err(), "more size");
    assert!(w.trade(&mut b, &mut a, POS_SCALE as i128).is_err(), "close trade");
    let s_ = w.slot();
    assert!(
        w.view().accrue_asset_to_not_atomic(0, s_ + 1, PRICE, 0, true).is_err(),
        "accrue"
    );
    assert!(w.view().mark_asset_drain_only_not_atomic(0).is_err(), "drain only");
    let s_ = w.slot();
    assert!(w.view().retire_empty_asset_not_atomic(0, s_ + 1).is_err(), "retire");
    let s_ = w.slot();
    assert!(w.view().force_asset_recovery_not_atomic(0, s_ + 1).is_err(), "recovery");
    assert!(
        w.view()
            .liquidate_account_not_atomic(&mut PortfolioV16ViewMut::new(&mut a), LiquidationRequestV16 { asset_index: 0 })
            .is_err(),
        "liquidate"
    );

    assert_eq!(w.header, hb, "header untouched by refused calls");
    assert_eq!((a, b, c), (ab, bb, cb), "accounts untouched by refused calls");
}

#[test]
fn mst_live_refuses_resolved_only_ops_and_changes_nothing() {
    let mut w = W::new();
    let (mut a, _b) = pair(&mut w, 1_000_000);
    let hb = w.header.clone();
    let ab = a.clone();
    let s_ = w.slot();
    assert!(w.view().advance_resolved_slot_not_atomic(s_ + 1).is_err());
    assert!(w.close(&mut a).is_err());
    assert!(w
        .view()
        .claim_resolved_payout_topup_not_atomic(&mut PortfolioV16ViewMut::new(&mut a))
        .is_err());
    assert_eq!(w.header, hb);
    assert_eq!(a, ab);
}

// ---------------------------------------------------------------- repeated transitions

/// Resolving twice: observe what the second call does to the settlement clock. The second
/// resolve must not move `resolved_slot` backwards; whether it may move forward is recorded.
#[test]
fn mst_second_resolve_never_moves_resolved_slot_back() {
    let mut w = W::new();
    let r0 = w.slot() + 10;
    w.resolve(r0).unwrap();
    let hb = w.header.clone();
    let r = w.resolve(r0 - 1);
    assert!(r.is_err(), "resolve below the resolved clock must be refused");
    assert_eq!(w.header, hb);
}

/// REAL BUG (not fixed here, product code untouched): `resolve_market_not_atomic`
/// (src/v16.rs:25903) never checks that the market is still Live. It only calls
/// `decode_market_mode` and drops the result, so a second call on an already Resolved market
/// succeeds and overwrites `resolved_slot` (src/v16.rs:25909) and `current_slot` (:25910),
/// and clears `loss_stale_active` (:25911) and the stress envelope. `resolved_slot` is the
/// anchor the maintenance fee stops at (src/v16.rs:25747), so a repeat call moves that anchor
/// later and open accounts get charged for more slots after the market was already resolved.
/// It should refuse with LockActive (or at least keep the first resolved_slot).
#[test]
#[ignore = "bug: resolve_market_not_atomic can be called again on a Resolved market and rewrites resolved_slot (src/v16.rs:25903)"]
fn mst_resolve_again_does_not_rewrite_resolved_slot() {
    let mut w = W::new();
    let r0 = w.slot() + 10;
    w.resolve(r0).unwrap();
    // A second resolve call on an already-resolved market must not change the slot that
    // maintenance fees and payouts are anchored to.
    let _ = w.resolve(r0 + 500);
    assert_eq!(w.header.resolved_slot.get(), r0);
}

// ---------------------------------------------------------------- Recovery transitions

#[test]
fn mst_force_asset_recovery_then_resolve_still_closes_value() {
    let mut w = W::new();
    let mut a = account(1);
    w.deposit(&mut a, 900).unwrap();
    let s_ = w.slot();
    w.view().force_asset_recovery_not_atomic(0, s_).unwrap();
    assert_eq!(w.lifecycle(), AssetLifecycleV16::Recovery);
    w.resolve(w.slot() + 1).unwrap();
    assert_eq!(w.close_fully(&mut a), 900);
    assert_eq!(w.vault(), 0);
}

#[test]
fn mst_force_asset_recovery_twice_is_idempotent_or_refused() {
    let mut w = W::new();
    let s_ = w.slot();
    w.view().force_asset_recovery_not_atomic(0, s_).unwrap();
    let hb = w.header.clone();
    let s_ = w.slot();
    let second = w.view().force_asset_recovery_not_atomic(0, s_);
    if second.is_err() {
        assert_eq!(w.header, hb, "a refused repeat must not change state");
    }
    assert_eq!(w.lifecycle(), AssetLifecycleV16::Recovery);
}

#[test]
fn mst_force_asset_recovery_in_the_past_is_refused() {
    let mut w = W::new();
    let mut a = account(1);
    w.deposit(&mut a, 10).unwrap();
    let hb = w.header.clone();
    let s_ = w.slot();
    assert!(w.view().force_asset_recovery_not_atomic(0, s_ - 1).is_err());
    assert_eq!(w.header, hb);
    assert_eq!(w.lifecycle(), AssetLifecycleV16::Active);
}

#[test]
fn mst_recovery_asset_refuses_new_trades_but_flat_owner_can_withdraw() {
    let mut w = W::new();
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, 1_000_000).unwrap();
    w.deposit(&mut b, 1_000_000).unwrap();
    let s_ = w.slot();
    w.view().force_asset_recovery_not_atomic(0, s_).unwrap();
    assert!(w.trade(&mut a, &mut b, POS_SCALE as i128).is_err(), "no new risk on a recovering asset");
    assert_eq!((a.capital.get(), b.capital.get()), (1_000_000, 1_000_000));
    // a flat owner is not trapped by the asset recovery
    w.withdraw(&mut a, 1_000_000).expect("flat owner exits");
    assert_eq!((w.vault(), w.c_tot()), (1_000_000, 1_000_000));
}

#[test]
fn mst_drain_only_blocks_new_open_but_allows_resolve() {
    let mut w = W::new();
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, 1_000_000).unwrap();
    w.deposit(&mut b, 1_000_000).unwrap();
    w.view().mark_asset_drain_only_not_atomic(0).unwrap();
    assert_eq!(w.lifecycle(), AssetLifecycleV16::DrainOnly);
    assert!(w.trade(&mut a, &mut b, POS_SCALE as i128).is_err());
    w.resolve(w.slot()).unwrap();
    let p = w.close_fully(&mut a) + w.close_fully(&mut b);
    assert_eq!(p, 2_000_000);
}

// ---------------------------------------------------------------- empty / zero shapes

#[test]
fn mst_resolve_empty_market_then_terminal_state_is_clean() {
    let mut w = W::new();
    w.resolve(w.slot()).unwrap();
    assert_eq!((w.vault(), w.c_tot()), (0, 0));
    w.view().validate_shape().unwrap();
}

#[test]
fn mst_zero_deposit_after_resolve_is_still_refused() {
    let mut w = W::new();
    let mut a = account(1);
    w.resolve(w.slot()).unwrap();
    // mode gate comes before the zero-amount shortcut
    assert_eq!(w.deposit(&mut a, 0), Err(V16Error::LockActive));
}

#[test]
fn mst_zero_withdraw_after_resolve_is_a_noop() {
    let mut w = W::new();
    let mut a = account(1);
    w.deposit(&mut a, 10).unwrap();
    w.resolve(w.slot()).unwrap();
    let hb = w.header.clone();
    let _ = w.withdraw(&mut a, 0);
    assert_eq!(w.header, hb);
    assert_eq!(a.capital.get(), 10);
}

#[test]
fn mst_max_tvl_deposit_survives_resolve_and_pays_in_full() {
    let mut w = W::new();
    let mut a = account(1);
    w.deposit(&mut a, MAX_VAULT_TVL).unwrap();
    w.resolve(w.slot()).unwrap();
    assert_eq!(w.close_fully(&mut a), MAX_VAULT_TVL);
    assert_eq!(w.vault(), 0);
}

#[test]
fn mst_insurance_deposit_after_resolve_is_value_conserving() {
    // deposit_domain_insurance_not_atomic has no mode gate (src/v16.rs:13383), so it still works
    // after resolution. Pin that it conserves value: the flat owner is paid exactly its deposit
    // and the vault keeps exactly the insurance that was added.
    let mut w = W::new();
    let mut a = account(1);
    w.deposit(&mut a, 1_000).unwrap();
    w.resolve(w.slot()).unwrap();
    w.view().deposit_domain_insurance_not_atomic(0, 40).unwrap();
    assert_eq!(w.vault(), 1_040);
    assert_eq!(w.close_fully(&mut a), 1_000);
    assert_eq!(w.vault(), 40);
    assert_eq!(w.header.insurance.get(), 40);
}
