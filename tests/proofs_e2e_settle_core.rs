//! E2E Kani proof (c), the principal-settlement step the REFRESH path actually uses, over FULL
//! u128 domains.
//!
//! Finding (native negative controls, REPORT.md): in a real deposit -> trade -> mark -> refresh
//! world the loss is moved from principal by
//! `reserve_new_capital_backed_loss_for_source_domain_not_atomic` during the K/F settlement,
//! BEFORE `settle_negative_pnl_from_principal_core_not_atomic` runs (the latter then sees
//! `paid == 0`; see the FIX-1b comment at src/v16.rs:17251). A mutant of the settle core is not
//! caught by the refresh-path tests; a mutant of this function is. So senior solvency of the
//! refresh needs THIS step, and the corrected inductive settle harness alone does not cover it.
//!
//! Called: the real method through its `cfg(kani)` shim
//! `kani_reserve_new_capital_backed_loss_for_source_domain_not_atomic(account, 0, 0, loss)`
//! (domain 0 activated, nothing pre-encumbered, `support_paid = 0` as in the shim).
//! Pre-state: canonical empty account (`try_empty`) with symbolic capital and pnl = -loss;
//! header with symbolic vault, c_tot, insurance.
//! Assumptions: c_tot + insurance <= vault; capital <= c_tot; 0 < loss <= i128::MAX.
//! Statement, on every Ok return (and Ok is reachable for values far above u8):
//!   backing = min(loss, capital); capital' = capital - backing; c_tot' = c_tot - backing;
//!   pnl' = -loss + backing; vault and insurance unchanged;
//!   INV after: c_tot' + insurance' <= vault' and capital' <= c_tot'.
//! And: whenever min(loss, capital) * BOUND_SCALE fits in u128, the call returns Ok.
#![cfg(kani)]

use percolator::v16::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, V16Config, V16PodI128, V16PodU128, V16PodU64,
};
use percolator::BOUND_SCALE;

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_e2e_refresh_principal_settlement_preserves_senior_solvency_full_domain() {
    let vault: u128 = kani::any();
    let c_tot: u128 = kani::any();
    let insurance: u128 = kani::any();
    let capital: u128 = kani::any();
    let loss: u128 = kani::any();
    kani::assume(c_tot.checked_add(insurance).map_or(false, |s| s <= vault));
    kani::assume(capital <= c_tot);
    kani::assume(loss > 0 && loss <= i128::MAX as u128);

    let market_id = [1u8; 32];
    let cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(market_id, cfg, 1, 0).unwrap();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    {
        let mut view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        view.activate_empty_market_not_atomic(0, 100, 1).unwrap();
    }
    header.vault = V16PodU128::new(vault);
    header.c_tot = V16PodU128::new(c_tot);
    header.insurance = V16PodU128::new(insurance);
    header.negative_pnl_account_count = V16PodU64::new(1);
    let mut id = [0u8; 32];
    id[0] = 2;
    let mut ah = PortfolioAccountV16Account::try_empty(ProvenanceHeaderV16Account::from_runtime(
        &ProvenanceHeaderV16::new(market_id, id, id),
    ))
    .unwrap();
    ah.capital = V16PodU128::new(capital);
    ah.pnl = V16PodI128::new(-(loss as i128));
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut ah);

    let backing = loss.min(capital);
    let r = market.kani_reserve_new_capital_backed_loss_for_source_domain_not_atomic(
        &mut account,
        0,
        0,
        loss,
    );
    if backing.checked_mul(BOUND_SCALE).is_some() {
        assert!(r.is_ok());
    }
    if r.is_ok() {
        assert!(account.header.capital.get() == capital - backing);
        assert!(market.header.c_tot.get() == c_tot - backing);
        assert!(account.header.pnl.get() == -(loss as i128) + backing as i128);
        assert!(market.header.vault.get() == vault);
        assert!(market.header.insurance.get() == insurance);
        let c2 = market.header.c_tot.get();
        assert!(c2.checked_add(market.header.insurance.get()).map_or(false, |s| s <= vault));
        assert!(account.header.capital.get() <= c2);
        kani::cover!(backing > 0 && backing < loss, "capital-capped: deficit remains");
        kani::cover!(backing > 0 && backing == loss, "loss fully backed by capital");
        // 2^80, not 2^90: an Ok return needs booked * BOUND_SCALE (1e12) to fit in u128, so
        // backing < 2^88.2 on every Ok path (a 2^90 cover was UNSATISFIED in the first run).
        kani::cover!(backing > (1u128 << 80), "far above the u8 fixture range");
    }
}
