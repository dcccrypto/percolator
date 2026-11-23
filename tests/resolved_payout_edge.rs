//! Edge-case tests for the v2.2 resolved-market close and payout paths:
//! `close_resolved_account_not_atomic`, `claim_resolved_payout_topup_not_atomic`,
//! `advance_resolved_slot_not_atomic`, `advance_terminal_slab_not_atomic` (and through them
//! the private `capital_and_resolved_payout_to_external_out` value-flow proof).
//! Tests only; no product code is touched.
//!
//! Every test name starts with `resolved_payout_` so `cargo test -- resolved_payout` selects them.
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]

use percolator::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, ResolvedCloseOutcomeV16, TerminalSlabOutcomeV16, TradeRequestV16,
    V16Config, V16Error, V16PodU64, MAX_VAULT_TVL, POS_SCALE,
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
    slot: u64,
    n: usize,
}

impl W {
    fn new(n: usize) -> Self {
        let mut cfg =
            V16Config::public_user_fund_with_market_slots(n as u16, n as u32, 0, 6_480_000);
        cfg.max_abs_funding_e9_per_slot = 10_000;
        cfg.max_price_move_bps_per_slot = 200;
        cfg.initial_margin_bps = 1_000;
        cfg.maintenance_margin_bps = 500;
        cfg.max_accrual_dt_slots = 2;
        cfg.min_funding_lifetime_slots = 10_000_000;
        let mut header =
            MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, n as u32, 0).unwrap();
        let mut markets: Vec<Market<u64>> = (0..n as u64)
            .map(|i| Market::new(i, EngineAssetSlotV16Account::default()))
            .collect();
        for i in 0..n {
            header
                .activate_empty_asset_slot_not_atomic(
                    i as u32,
                    &mut markets[i].engine,
                    PRICE,
                    1 + 2 * i as u64,
                )
                .unwrap();
        }
        W { header, markets, slot: 2 * n as u64 - 1, n }
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
    fn deposit(&mut self, a: &mut PortfolioAccountV16Account, amt: u128) {
        self.view()
            .deposit_not_atomic(&mut PortfolioV16ViewMut::new(a), amt)
            .unwrap();
    }
    fn trade(
        &mut self,
        long: &mut PortfolioAccountV16Account,
        short: &mut PortfolioAccountV16Account,
        size: i128,
    ) {
        let p = self.markets[0].engine.asset.effective_price.get();
        let mut m = self.view();
        m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut PortfolioV16ViewMut::new(long),
            &mut PortfolioV16ViewMut::new(short),
            TradeRequestV16 { asset_index: 0, size_q: size, exec_price: p, fee_bps: 0 },
            true,
        )
        .unwrap();
    }
    fn move_price(&mut self, bps: i64) {
        let steps = (bps.abs() + 149) / 150;
        let mut left = bps;
        for _ in 0..steps {
            let d = left.clamp(-150, 150);
            left -= d;
            self.slot += 1;
            let old = self.markets[0].engine.asset.effective_price.get() as i128;
            let new = (old + old * d as i128 / 10_000) as u64;
            let slot = self.slot;
            let mut m = self.view();
            m.accrue_asset_to_not_atomic(0, slot, new, 0, true).expect("accrue");
            m.markets[0].engine.asset.raw_oracle_target_price = V16PodU64::new(new);
        }
    }
    fn refresh(&mut self, a: &mut PortfolioAccountV16Account) {
        let mut m = self.view();
        let mut budget = percolator::S10_MAX_MOVES_PER_INSTRUCTION;
        m.full_account_refresh_with_s10_budget_not_atomic(&mut PortfolioV16ViewMut::new(a), &mut budget)
            .unwrap();
    }
    fn resolve(&mut self) {
        let slot = self.slot;
        self.view().resolve_market_not_atomic(slot).unwrap();
    }
    fn close(
        &mut self,
        a: &mut PortfolioAccountV16Account,
    ) -> Result<ResolvedCloseOutcomeV16, V16Error> {
        self.view().close_resolved_account_not_atomic(&mut PortfolioV16ViewMut::new(a), 0)
    }
    /// Call `close` until it reports `Closed` (bounded), returning the payout.
    fn close_fully(&mut self, a: &mut PortfolioAccountV16Account) -> u128 {
        for _ in 0..64 {
            if let ResolvedCloseOutcomeV16::Closed { payout } = self.close(a).expect("close") {
                return payout;
            }
        }
        panic!("close never completed");
    }
    /// Close two accounts by alternating calls (a winner may need the loser closed first).
    /// Returns total paid to (a, b). `a_first` picks who is called first.
    fn close_pair(
        &mut self,
        a: &mut PortfolioAccountV16Account,
        b: &mut PortfolioAccountV16Account,
        a_first: bool,
    ) -> (u128, u128) {
        let (mut pa, mut pb) = (None, None);
        for _ in 0..16 {
            for turn_a in [a_first, !a_first] {
                if turn_a && pa.is_none() {
                    if let ResolvedCloseOutcomeV16::Closed { payout } = self.close(a).unwrap() {
                        pa = Some(payout);
                    }
                } else if !turn_a && pb.is_none() {
                    if let ResolvedCloseOutcomeV16::Closed { payout } = self.close(b).unwrap() {
                        pb = Some(payout);
                    }
                }
            }
            if pa.is_some() && pb.is_some() {
                return (pa.unwrap(), pb.unwrap());
            }
        }
        panic!("pair never closed");
    }
    fn topup(&mut self, a: &mut PortfolioAccountV16Account) -> Result<u128, V16Error> {
        self.view().claim_resolved_payout_topup_not_atomic(&mut PortfolioV16ViewMut::new(a))
    }
    fn advance(&mut self, slot: u64) -> Result<(), V16Error> {
        self.view().advance_resolved_slot_not_atomic(slot)
    }
    fn terminal(
        &mut self,
        slot: u64,
        start: usize,
        reserved: u128,
    ) -> Result<TerminalSlabOutcomeV16, V16Error> {
        self.view().advance_terminal_slab_not_atomic(slot, start, reserved)
    }
    fn cur_slot(&self) -> u64 {
        self.header.current_slot.get()
    }
}

/// Two accounts, A long / B short one unit, price moved `bps`, then resolved.
/// Returns (world, A, B, deposit each).
fn winner_loser(bps: i64) -> (W, PortfolioAccountV16Account, PortfolioAccountV16Account, u128) {
    let dep = 1_000_000u128;
    let mut w = W::new(1);
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, dep);
    w.deposit(&mut b, dep);
    w.trade(&mut a, &mut b, POS_SCALE as i128);
    w.move_price(bps);
    w.refresh(&mut a);
    w.refresh(&mut b);
    w.resolve();
    (w, a, b, dep)
}

// ---------------------------------------------------------------- close: zero and max

#[test]
fn resolved_payout_close_empty_account_pays_zero() {
    let mut w = W::new(1);
    let mut a = account(1);
    w.resolve();
    assert_eq!(w.close(&mut a), Ok(ResolvedCloseOutcomeV16::Closed { payout: 0 }));
    assert_eq!((w.vault(), w.c_tot()), (0, 0));
}

#[test]
fn resolved_payout_close_flat_account_returns_exact_deposit() {
    let mut w = W::new(1);
    let mut a = account(1);
    w.deposit(&mut a, 12_345);
    w.resolve();
    assert_eq!(w.close(&mut a), Ok(ResolvedCloseOutcomeV16::Closed { payout: 12_345 }));
    assert_eq!((w.vault(), w.c_tot(), a.capital.get()), (0, 0, 0));
}

#[test]
fn resolved_payout_close_one_atom_dust() {
    let mut w = W::new(1);
    let mut a = account(1);
    w.deposit(&mut a, 1);
    w.resolve();
    assert_eq!(w.close(&mut a), Ok(ResolvedCloseOutcomeV16::Closed { payout: 1 }));
    assert_eq!(w.vault(), 0);
}

#[test]
fn resolved_payout_close_max_tvl_deposit_pays_in_full() {
    let mut w = W::new(1);
    let mut a = account(1);
    w.deposit(&mut a, MAX_VAULT_TVL);
    w.resolve();
    assert_eq!(w.close(&mut a), Ok(ResolvedCloseOutcomeV16::Closed { payout: MAX_VAULT_TVL }));
    assert_eq!((w.vault(), w.c_tot()), (0, 0));
}

#[test]
fn resolved_payout_close_in_live_mode_is_refused_and_changes_nothing() {
    let mut w = W::new(1);
    let mut a = account(1);
    w.deposit(&mut a, 500);
    let (hb, ab) = (w.header.clone(), a.clone());
    assert_eq!(w.close(&mut a), Err(V16Error::LockActive));
    assert_eq!(w.header, hb);
    assert_eq!(a, ab);
}

// ---------------------------------------------------------------- repeated calls

#[test]
fn resolved_payout_second_close_pays_nothing_more() {
    let mut w = W::new(1);
    let mut a = account(1);
    w.deposit(&mut a, 700);
    w.resolve();
    assert_eq!(w.close(&mut a), Ok(ResolvedCloseOutcomeV16::Closed { payout: 700 }));
    let vault_after_first = w.vault();
    // A repeat must never pay again. It may succeed with zero or be refused.
    match w.close(&mut a) {
        Ok(ResolvedCloseOutcomeV16::Closed { payout }) => assert_eq!(payout, 0),
        Ok(ResolvedCloseOutcomeV16::ProgressOnly) | Err(_) => {}
    }
    assert_eq!(w.vault(), vault_after_first);
    assert_eq!(w.c_tot(), 0);
}

#[test]
fn resolved_payout_topup_before_any_receipt_pays_zero_or_refuses() {
    let (mut w, mut a, _b, _) = winner_loser(300);
    let vault = w.vault();
    match w.topup(&mut a) {
        Ok(p) => assert_eq!(p, 0),
        Err(_) => {}
    }
    assert_eq!(w.vault(), vault);
}

#[test]
fn resolved_payout_topup_in_live_mode_is_lock_active() {
    let mut w = W::new(1);
    let mut a = account(1);
    w.deposit(&mut a, 10);
    let hb = w.header.clone();
    assert_eq!(w.topup(&mut a), Err(V16Error::LockActive));
    assert_eq!(w.header, hb);
}

#[test]
fn resolved_payout_topup_is_idempotent_after_close() {
    let (mut w, mut a, mut b, _) = winner_loser(300);
    let _ = w.close_pair(&mut a, &mut b, true);
    let vault = w.vault();
    for _ in 0..3 {
        if let Ok(p) = w.topup(&mut a) {
            assert_eq!(p, 0, "nothing left to top up after a full close");
        }
        if let Ok(p) = w.topup(&mut b) {
            assert_eq!(p, 0);
        }
    }
    assert_eq!(w.vault(), vault);
}

// ---------------------------------------------------------------- conservation

#[test]
fn resolved_payout_winner_and_loser_conserve_total_value() {
    for bps in [1i64, 50, 300, 1_000, -1, -50, -300, -1_000] {
        let (mut w, mut a, mut b, dep) = winner_loser(bps);
        let total_in = 2 * dep;
        assert_eq!(w.vault(), total_in);
        let (pa, pb) = w.close_pair(&mut a, &mut b, true);
        let ta = w.topup(&mut a).unwrap_or(0);
        let tb = w.topup(&mut b).unwrap_or(0);
        let paid = pa + pb + ta + tb;
        assert!(paid <= total_in, "bps {bps}: paid {paid} > in {total_in}");
        assert_eq!(w.vault(), total_in - paid, "bps {bps}: vault must equal in - out");
        // No insurance was funded, so the zero-sum book pays out (almost) everything.
        assert!(w.vault() <= 1, "bps {bps}: dust left {}", w.vault());
        assert_eq!(w.c_tot(), 0, "bps {bps}");
    }
}

#[test]
fn resolved_payout_winner_gets_more_than_deposit_loser_less() {
    let (mut w, mut a, mut b, dep) = winner_loser(300);
    let (pa, pb) = w.close_pair(&mut a, &mut b, true);
    assert!(pa > dep, "long won: {pa}");
    assert!(pb < dep, "short lost: {pb}");
}

#[test]
fn resolved_payout_zero_price_move_returns_deposits() {
    let (mut w, mut a, mut b, dep) = winner_loser(0);
    assert_eq!(w.close_pair(&mut a, &mut b, true), (dep, dep));
    assert_eq!(w.vault(), 0);
}

// ---------------------------------------------------------------- ordering

#[test]
fn resolved_payout_close_order_does_not_change_total_paid() {
    let mut totals = Vec::new();
    for first_is_a in [true, false] {
        let (mut w, mut a, mut b, _) = winner_loser(300);
        let (pa, pb) = w.close_pair(&mut a, &mut b, first_is_a);
        let ta = w.topup(&mut a).unwrap_or(0);
        let tb = w.topup(&mut b).unwrap_or(0);
        totals.push((pa + ta, pb + tb, w.vault()));
    }
    assert_eq!(totals[0], totals[1], "close order changed payouts: {totals:?}");
}

// ---------------------------------------------------------------- advance_resolved_slot

#[test]
fn resolved_payout_advance_slot_same_slot_is_ok_and_noop() {
    let (mut w, ..) = winner_loser(100);
    let (s, v) = (w.cur_slot(), w.vault());
    assert_eq!(w.advance(s), Ok(()));
    assert_eq!((w.cur_slot(), w.vault()), (s, v));
}

#[test]
fn resolved_payout_advance_slot_backwards_is_stale() {
    let (mut w, ..) = winner_loser(100);
    let s = w.cur_slot();
    assert_eq!(w.advance(s - 1), Err(V16Error::Stale));
    assert_eq!(w.cur_slot(), s);
}

#[test]
fn resolved_payout_advance_slot_forward_to_u64_max() {
    let (mut w, ..) = winner_loser(100);
    let v = w.vault();
    assert_eq!(w.advance(u64::MAX), Ok(()));
    assert_eq!(w.cur_slot(), u64::MAX);
    assert_eq!(w.vault(), v);
    // and can never go back
    assert_eq!(w.advance(u64::MAX - 1), Err(V16Error::Stale));
    assert_eq!(w.advance(u64::MAX), Ok(()));
}

#[test]
fn resolved_payout_advance_slot_in_live_mode_is_lock_active() {
    let mut w = W::new(1);
    let s = w.cur_slot();
    assert_eq!(w.advance(s + 5), Err(V16Error::LockActive));
    assert_eq!(w.cur_slot(), s);
}

#[test]
fn resolved_payout_advance_slot_keeps_resolved_slot_and_payout() {
    let (mut w, mut a, mut b, dep) = winner_loser(0);
    let rs = w.header.resolved_slot.get();
    w.advance(rs + 1_000).unwrap();
    assert_eq!(w.header.resolved_slot.get(), rs, "resolved_slot is fixed at resolution");
    assert_eq!(w.close_pair(&mut a, &mut b, true), (dep, dep));
}

// ---------------------------------------------------------------- terminal slab

#[test]
fn resolved_payout_terminal_slab_refused_while_accounts_remain() {
    let (mut w, a, _b, _) = winner_loser(100);
    let s = w.cur_slot();
    assert_eq!(w.terminal(s, 0, 0), Err(V16Error::LockActive));
}

#[test]
fn resolved_payout_terminal_slab_in_live_mode_is_lock_active() {
    let mut w = W::new(1);
    let s = w.cur_slot();
    assert_eq!(w.terminal(s, 0, 0), Err(V16Error::LockActive));
}

#[test]
fn resolved_payout_terminal_slab_empty_market_is_ready_with_zero_retired() {
    let mut w = W::new(1);
    w.resolve();
    let s = w.cur_slot();
    assert_eq!(w.terminal(s, 0, 0), Ok(TerminalSlabOutcomeV16::ReadyToClose { retired: 0 }));
}

#[test]
fn resolved_payout_terminal_slab_start_index_out_of_range_is_invalid_config() {
    let mut w = W::new(1);
    w.resolve();
    let s = w.cur_slot();
    assert_eq!(w.terminal(s, 1, 0), Err(V16Error::InvalidConfig));
    assert_eq!(w.terminal(s, usize::MAX, 0), Err(V16Error::InvalidConfig));
}

#[test]
fn resolved_payout_terminal_slab_backwards_slot_is_stale() {
    let mut w = W::new(1);
    w.resolve();
    let s = w.cur_slot();
    w.advance(s + 10).unwrap();
    assert_eq!(w.terminal(s, 0, 0), Err(V16Error::Stale));
}

#[test]
fn resolved_payout_terminal_slab_advances_the_clock() {
    let mut w = W::new(1);
    w.resolve();
    let s = w.cur_slot();
    let _ = w.terminal(s + 7, 0, 0);
    assert_eq!(w.cur_slot(), s + 7);
}

#[test]
fn resolved_payout_terminal_slab_after_all_closed_conserves_value() {
    let (mut w, mut a, mut b, dep) = winner_loser(300);
    let _ = w.close_pair(&mut a, &mut b, true);
    let _ = (w.topup(&mut a), w.topup(&mut b));
    let vault_before = w.vault();
    let s = w.cur_slot();
    match w.terminal(s, 0, 0) {
        Ok(TerminalSlabOutcomeV16::ReadyToClose { retired }) => {
            assert_eq!(w.vault(), vault_before - retired, "retired atoms leave the vault");
        }
        Ok(_) => {}
        Err(_) => {}
    }
    assert!(w.vault() <= vault_before);
}

#[test]
fn resolved_payout_terminal_slab_reserve_never_exceeds_vault_drop() {
    let (mut w, mut a, mut b, _) = winner_loser(300);
    let _ = w.close_pair(&mut a, &mut b, true);
    let before = w.vault();
    let s = w.cur_slot();
    if let Ok(TerminalSlabOutcomeV16::ReadyToClose { retired }) = w.terminal(s, 0, u128::MAX) {
        assert!(retired <= before);
        assert_eq!(w.vault(), before - retired);
    }
}


#[test]
fn resolved_payout_winner_close_waits_for_loser_then_pays() {
    // Ordering: the winner's first close only makes progress while the loser still owes.
    let (mut w, mut a, mut b, dep) = winner_loser(300);
    assert_eq!(w.close(&mut a), Ok(ResolvedCloseOutcomeV16::ProgressOnly));
    assert_eq!(w.vault(), 2 * dep, "a progress-only call pays nothing");
    let pb = match w.close(&mut b).unwrap() {
        ResolvedCloseOutcomeV16::Closed { payout } => payout,
        other => panic!("loser should close in one call: {other:?}"),
    };
    assert!(pb < dep);
    let pa = match w.close(&mut a).unwrap() {
        ResolvedCloseOutcomeV16::Closed { payout } => payout,
        other => panic!("winner should now close: {other:?}"),
    };
    assert_eq!(pa + pb, 2 * dep);
    assert_eq!(w.vault(), 0);
}
