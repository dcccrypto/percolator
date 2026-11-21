//! E2E Kani proof (a), largest trade sub-step that finishes: the value-moving step of every
//! fill, `charge_trade_fee_taker_only_not_atomic` (the real method; called by
//! `apply_trade_after_refresh_not_atomic`, src/v16.rs:23519), over FULL u128/i128 domains.
//!
//! Why this cut: the one-fill public entry point (tests/proofs_e2e_trade.rs) was still in CBMC
//! symbolic execution after 45 min (REPORT.md); upstream records the trade as beyond the solver
//! budget. A fill moves value ONLY through this step (upstream trade pilot TA.conservation:
//! vault unchanged, c_tot - fees, insurance + fees, pnl unchanged); the rest of the fill writes
//! positions. So the conservation of a fill is this step's conservation; what is NOT proven
//! here is that the rest of `apply_trade_after_refresh_not_atomic` writes no value field
//! (that is the trade-pilot frame property TA.frame, prompt stage_e2e_a_conservation.txt).
//!
//! Pre-state: two canonically initialised empty accounts (`try_empty`, as every engine account
//! is), symbolic capital and pnl; header with symbolic vault, c_tot, insurance.
//! Assumptions (the decomposed loop-free invariant of the inductive settle proof):
//!   c_tot + insurance <= vault; long.capital + short.capital <= c_tot; pnl > i128::MIN.
//! Statement: the call returns Ok((fee_long, fee_short)) and
//!   * exactness: the taker pays want(fee, taker) := 0 if fee == 0 or pnl < 0 else
//!     min(fee, capital); the maker pays want(fee - taker_fee, maker) (shortfall only);
//!   * vault unchanged; c_tot' = c_tot - fee_long - fee_short;
//!     insurance' = insurance + fee_long + fee_short; long/short capital lowered by exactly their fee;
//!     both pnl unchanged; fee_long + fee_short <= fee;
//!   * INV after: c_tot' + insurance' <= vault' and each capital' <= c_tot'.
#![cfg(kani)]

use percolator::v16::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, V16Config, V16PodI128, V16PodU128,
};

fn empty_account(market_id: [u8; 32], tag: u8) -> PortfolioAccountV16Account {
    let mut id = [0u8; 32];
    id[0] = tag;
    PortfolioAccountV16Account::try_empty(ProvenanceHeaderV16Account::from_runtime(
        &ProvenanceHeaderV16::new(market_id, id, id),
    ))
    .unwrap()
}

fn want(fee: u128, pnl: i128, capital: u128) -> u128 {
    if fee == 0 || pnl < 0 {
        0
    } else {
        fee.min(capital)
    }
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_e2e_trade_fee_step_conserves_value_full_domain() {
    let vault: u128 = kani::any();
    let c_tot: u128 = kani::any();
    let insurance: u128 = kani::any();
    let lcap: u128 = kani::any();
    let scap: u128 = kani::any();
    let lpnl: i128 = kani::any();
    let spnl: i128 = kani::any();
    let fee: u128 = kani::any();
    let taker_is_long: bool = kani::any();
    kani::assume(c_tot.checked_add(insurance).map_or(false, |s| s <= vault));
    kani::assume(lcap.checked_add(scap).map_or(false, |s| s <= c_tot));
    kani::assume(lpnl > i128::MIN && spnl > i128::MIN);

    let market_id = [1u8; 32];
    let cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(market_id, cfg, 1, 0).unwrap();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    header.vault = V16PodU128::new(vault);
    header.c_tot = V16PodU128::new(c_tot);
    header.insurance = V16PodU128::new(insurance);
    let mut lh = empty_account(market_id, 5);
    let mut sh = empty_account(market_id, 6);
    lh.capital = V16PodU128::new(lcap);
    lh.pnl = V16PodI128::new(lpnl);
    sh.capital = V16PodU128::new(scap);
    sh.pnl = V16PodI128::new(spnl);
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut lv = PortfolioV16ViewMut::new(&mut lh);
    let mut sv = PortfolioV16ViewMut::new(&mut sh);

    let r = market.kani_charge_trade_fee_taker_only_not_atomic(&mut lv, &mut sv, fee, taker_is_long);
    assert!(r.is_ok());
    let (fl, fs) = r.unwrap();

    let (tf, mf) = if taker_is_long { (fl, fs) } else { (fs, fl) };
    let (tcap, tpnl, mcap, mpnl) =
        if taker_is_long { (lcap, lpnl, scap, spnl) } else { (scap, spnl, lcap, lpnl) };
    assert!(tf == want(fee, tpnl, tcap));
    let shortfall = fee - tf;
    assert!(mf == if shortfall == 0 { 0 } else { want(shortfall, mpnl, mcap) });

    let moved = fl + fs;
    assert!(moved <= fee);
    assert!(market.header.vault.get() == vault);
    assert!(market.header.c_tot.get() == c_tot - moved);
    assert!(market.header.insurance.get() == insurance + moved);
    assert!(lv.header.capital.get() == lcap - fl);
    assert!(sv.header.capital.get() == scap - fs);
    assert!(lv.header.pnl.get() == lpnl && sv.header.pnl.get() == spnl);
    let c2 = market.header.c_tot.get();
    assert!(c2.checked_add(market.header.insurance.get()).map_or(false, |s| s <= vault));
    assert!(lv.header.capital.get() <= c2 && sv.header.capital.get() <= c2);

    kani::cover!(tf > 0 && mf > 0, "taker partial, maker pays the shortfall");
    kani::cover!(tf == 0 && mf == fee && fee > 0, "taker waived/empty, maker pays all");
    kani::cover!(moved > (1u128 << 100), "far above u8 fixture range");
    kani::cover!(fee > 0 && moved == 0, "both waived: nothing moves");
}
