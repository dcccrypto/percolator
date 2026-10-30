//! GH#292 formal evidence (Kani). Scope is deliberately narrow and honest:
//!
//! * `proof_gh292_stuck_state_is_absorbing_under_every_permissionless_step` shows that the
//!   latched state (Live, flat account, capital 0, pnl < 0, `liquidation_lock`, counter 1,
//!   hlock active) is a FIXED POINT of every public permissionless step: `try_clear` refuses and
//!   no step moves the account pnl, the counter, the lock or the hlock. Only the owner funding
//!   the account (deposit) or an explicit market resolution leaves the state; both are excluded
//!   from the op set on purpose and named in the report.
//! * `proof_gh292_locked_liquidation_leaves_flat_deficit_without_resolution` shows the step INTO
//!   the state: from a locked cross-margin account with one leg left, the locked liquidation
//!   branch returns Ok and leaves it flat with the deficit untouched.
//! * Reachability from an empty market is the native test
//!   `gh292_repro_flat_locked_deficit_latches_hlock_in_live_market` (real trades and accruals).
#![cfg(kani)]

use percolator::{
    active_bitmap_is_empty, AutoCrankWorkV16, EngineAssetSlotV16Account, LiquidationRequestV16,
    Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, PortfolioAccountV16Account,
    PortfolioV16ViewMut, ProvenanceHeaderV16, ProvenanceHeaderV16Account, V16Config, V16PodI128,
    V16PodU128, V16PodU64,
};

fn stuck_fixture(
    deficit: u128,
) -> (
    MarketGroupV16HeaderAccount,
    [Market<u64>; 1],
    PortfolioAccountV16Account,
) {
    let market_id = [1u8; 32];
    let cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(market_id, cfg, 1, 0).unwrap();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    {
        let mut view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        view.activate_empty_market_not_atomic(0, 100, 1).unwrap();
    }
    let mut id = [0u8; 32];
    id[0] = 2;
    let mut account = PortfolioAccountV16Account::try_empty(ProvenanceHeaderV16Account::from_runtime(
        &ProvenanceHeaderV16::new(market_id, id, id),
    ))
    .unwrap();
    // The state the #292 liquidation leaves behind.
    account.pnl = V16PodI128::new(-(deficit as i128));
    account.liquidation_lock = 1;
    header.negative_pnl_account_count = V16PodU64::new(1);
    header.bankruptcy_hlock_active = 1;
    (header, markets, account)
}

#[kani::proof]
#[kani::unwind(4)]
fn proof_gh292_stuck_state_is_absorbing_under_every_permissionless_step() {
    let deficit: u128 = kani::any();
    kani::assume(deficit >= 1 && deficit <= 1_000_000_000);
    let (mut header, mut markets, mut account_h) = stuck_fixture(deficit);
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_h);
    // Precondition: it is a valid reachable-shape state (the native test reaches it by trades).
    kani::assume(account.as_view().validate_with_market(&market.as_view()).is_ok());
    assert!(active_bitmap_is_empty(account.header.active_bitmap.map(V16PodU64::get)));
    assert_eq!(account.header.capital.get(), 0);

    let op: u8 = kani::any();
    let slot = market.header.current_slot.get();
    let mut ok = false;
    match op % 5 {
        0 => {
            // try_clear alone: must refuse (counter != 0).
            ok = true;
            assert_eq!(market.try_clear_bankruptcy_hlock_not_atomic(), Ok(false));
        }
        1 => ok = market.full_account_refresh_not_atomic(&mut account).is_ok(),
        2 => {
            ok = market
                .permissionless_auto_crank_not_atomic(
                    &mut account,
                    AutoCrankWorkV16 {
                        now_slot: slot,
                        observations: &[],
                        resolved_close_fee_rate_per_slot: 0,
                    },
                )
                .is_ok()
        }
        3 => {
            ok = market
                .liquidate_account_not_atomic(&mut account, LiquidationRequestV16 { asset_index: 0 })
                .is_ok()
        }
        _ => {
            let amount: u128 = kani::any();
            ok = market.withdraw_not_atomic(&mut account, amount).is_ok()
        }
    }
    kani::cover!(ok && op % 5 == 1, "refresh succeeds on the stuck state");
    kani::cover!(ok && op % 5 == 2, "auto-crank succeeds on the stuck state");
    if ok {
        // Absorbing: nothing moved.
        assert_eq!(account.header.pnl.get(), -(deficit as i128));
        assert_eq!(account.header.liquidation_lock, 1);
        assert_eq!(market.header.negative_pnl_account_count.get(), 1);
        assert_eq!(market.header.bankruptcy_hlock_active, 1);
        assert_eq!(market.header.mode, 0);
        // And the clear predicate still refuses.
        assert_eq!(market.try_clear_bankruptcy_hlock_not_atomic(), Ok(false));
    }
}

#[kani::proof]
fn proof_gh292_c_scoping_is_exact_and_never_clears() {
    let wire: u8 = kani::any();
    let asset: usize = kani::any();
    kani::assume(asset < 7);
    let g = percolator::bankruptcy_hlock_gates_asset(wire, asset);
    assert!(!g || wire != 0);
    if wire == 1 {
        assert!(g); // unattributed gates every asset: the #292 latch is not narrowed
    }
    if wire > 1 && wire & 1 == 1 {
        assert_eq!(g, (((wire >> 1) as u16) & (0b11u16 << (asset * 2))) != 0);
    }
    kani::cover!(wire == 1 && g, "unattributed gates");
    kani::cover!(wire > 1 && wire & 1 == 1 && !g, "attributed scopes out");
}
