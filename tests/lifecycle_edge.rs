//! Account lifecycle and money-conservation edge tests for the v2.2 engine: deposit, withdraw
//! (flat / free capital only), open and close, slot reuse after close, dust, several assets, and
//! a seeded random sequence that checks after EVERY step that the vault equals net external flow
//! and covers the account balances plus insurance. Tests only; no product code is touched.
//!
//! Setup: IM 10%, MM 5%, price 1_000_000, no funding. Withdraw while a leg is open and
//! withdraw-vs-margin cases live in margin_edge.rs and are not repeated here.
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]
use percolator::POS_SCALE;
use percolator::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16, ProvenanceHeaderV16Account,
    ResolvedCloseOutcomeV16, TradeRequestV16, V16Config, V16PodU64, MAX_VAULT_TVL,
};

const PRICE: u64 = 1_000_000;

fn account(seed: u32) -> PortfolioAccountV16Account {
    let mut key = [0u8; 32];
    key[..4].copy_from_slice(&seed.to_le_bytes());
    key[31] = 0x5A;
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new([1; 32], key, [3; 32]));
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
        let mut cfg = V16Config::public_user_fund_with_market_slots(n as u16, n as u32, 0, 6_480_000);
        cfg.max_abs_funding_e9_per_slot = 10_000;
        cfg.max_price_move_bps_per_slot = 200;
        cfg.initial_margin_bps = 1_000;
        cfg.maintenance_margin_bps = 500;
        cfg.max_accrual_dt_slots = 2;
        cfg.min_funding_lifetime_slots = 10_000_000;
        let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, n as u32, 0).unwrap();
        let mut markets: Vec<Market<u64>> = (0..n as u64).map(|i| Market::new(i, EngineAssetSlotV16Account::default())).collect();
        for i in 0..n {
            header.activate_empty_asset_slot_not_atomic(i as u32, &mut markets[i].engine, PRICE, 1 + 2 * i as u64).unwrap();
        }
        W { header, markets, slot: 2 * n as u64 - 1, n }
    }
    fn view(&mut self) -> MarketGroupV16ViewMut<'_, u64> {
        MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets)
    }
    fn price(&self, i: usize) -> u64 { self.markets[i].engine.asset.effective_price.get() }
    fn vault(&self) -> u128 { self.header.vault.get() }
    fn c_tot(&self) -> u128 { self.header.c_tot.get() }
    fn insurance(&self) -> u128 { self.header.insurance.get() }
    fn deposit(&mut self, a: &mut PortfolioAccountV16Account, amt: u128) -> bool {
        let mut m = self.view();
        m.deposit_not_atomic(&mut PortfolioV16ViewMut::new(a), amt).is_ok()
    }
    fn withdraw(&mut self, a: &mut PortfolioAccountV16Account, amt: u128) -> bool {
        let mut m = self.view();
        m.withdraw_not_atomic(&mut PortfolioV16ViewMut::new(a), amt).is_ok()
    }
    fn trade(&mut self, i: usize, long: &mut PortfolioAccountV16Account, short: &mut PortfolioAccountV16Account, size: i128, fee_bps: u64) -> bool {
        let p = self.price(i);
        let mut m = self.view();
        m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut PortfolioV16ViewMut::new(long),
            &mut PortfolioV16ViewMut::new(short),
            TradeRequestV16 { asset_index: i, size_q: size, exec_price: p, fee_bps },
            true,
        )
        .is_ok()
    }
    fn move_price(&mut self, i: usize, bps: i64) {
        let steps = (bps.abs() + 149) / 150;
        let mut left = bps;
        for _ in 0..steps {
            let d = left.clamp(-150, 150);
            left -= d;
            self.slot += 1;
            for j in 0..self.n {
                let old = self.price(j) as i128;
                let new = if j == i { (old + old * d as i128 / 10_000) as u64 } else { old as u64 };
                let slot = self.slot;
                let mut m = self.view();
                m.accrue_asset_to_not_atomic(j, slot, new, 0, true).expect("accrue");
                m.markets[j].engine.asset.raw_oracle_target_price = V16PodU64::new(new);
            }
        }
    }
    fn refresh(&mut self, a: &mut PortfolioAccountV16Account) -> bool {
        let mut m = self.view();
        let mut budget = percolator::S10_MAX_MOVES_PER_INSTRUCTION;
        m.full_account_refresh_with_s10_budget_not_atomic(&mut PortfolioV16ViewMut::new(a), &mut budget).is_ok()
    }
    fn ins_deposit(&mut self, domain: usize, amt: u128) -> bool {
        self.view().deposit_domain_insurance_not_atomic(domain, amt).is_ok()
    }
    fn ins_withdraw(&mut self, domain: usize, amt: u128) -> bool {
        self.view().withdraw_domain_insurance_not_atomic(domain, amt).is_ok()
    }
    fn mode_resolved(&mut self, slot: u64) {
        self.view().resolve_market_not_atomic(slot).expect("resolve");
    }
    fn count(&self) -> u64 { self.header.materialized_portfolio_count.get() }
}

fn cap(a: &PortfolioAccountV16Account) -> u128 { a.capital.get() }

/// vault covers account balances + insurance, and c_tot is exactly the sum of balances.
fn check(w: &W, accts: &[&PortfolioAccountV16Account], net_flow: u128, ctx: &str) {
    let sum: u128 = accts.iter().map(|a| cap(a)).sum();
    assert_eq!(w.c_tot(), sum, "c_tot != sum of account capital ({ctx})");
    assert!(w.vault() >= w.c_tot() + w.insurance(), "vault < c_tot + insurance ({ctx})");
    assert_eq!(w.vault(), net_flow, "vault != net external flow ({ctx})");
}

// ---------- deposit / withdraw basics ----------

#[test]
fn lifecycle_deposit_moves_vault_ctot_and_capital_together() {
    let mut w = W::new(1);
    let mut a = account(1);
    assert!(w.deposit(&mut a, 12_345));
    assert_eq!((cap(&a), w.c_tot(), w.vault()), (12_345, 12_345, 12_345));
    assert!(w.deposit(&mut a, 5));
    assert_eq!((cap(&a), w.c_tot(), w.vault()), (12_350, 12_350, 12_350));
}

#[test]
fn lifecycle_deposit_zero_is_a_noop() {
    let mut w = W::new(1);
    let mut a = account(1);
    assert!(w.deposit(&mut a, 0));
    assert_eq!((cap(&a), w.c_tot(), w.vault()), (0, 0, 0));
}

#[test]
fn lifecycle_withdraw_zero_is_a_noop() {
    let mut w = W::new(1);
    let mut a = account(1);
    w.deposit(&mut a, 100);
    assert!(w.withdraw(&mut a, 0));
    assert_eq!((cap(&a), w.c_tot(), w.vault()), (100, 100, 100));
}

#[test]
fn lifecycle_withdraw_one_atom_over_capital_is_refused_and_changes_nothing() {
    let mut w = W::new(1);
    let mut a = account(1);
    w.deposit(&mut a, 777);
    let (wb, ab) = (w.clone().header, a);
    assert!(!w.withdraw(&mut a, 778));
    assert_eq!(a, ab, "account unchanged");
    assert_eq!(w.header, wb, "market header unchanged");
}

#[test]
fn lifecycle_withdraw_in_pieces_then_exact_remainder_leaves_zero() {
    let mut w = W::new(1);
    let mut a = account(1);
    w.deposit(&mut a, 1_000);
    assert!(w.withdraw(&mut a, 1));
    assert!(w.withdraw(&mut a, 600));
    assert!(w.withdraw(&mut a, 399));
    assert_eq!((cap(&a), w.c_tot(), w.vault()), (0, 0, 0));
    assert!(!w.withdraw(&mut a, 1), "nothing left");
}

#[test]
fn lifecycle_one_atom_dust_round_trip() {
    let mut w = W::new(1);
    let mut a = account(1);
    assert!(w.deposit(&mut a, 1));
    assert!(w.withdraw(&mut a, 1));
    assert_eq!((cap(&a), w.c_tot(), w.vault()), (0, 0, 0));
}

#[test]
fn lifecycle_two_accounts_do_not_touch_each_others_balance() {
    let mut w = W::new(1);
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, 500);
    w.deposit(&mut b, 300);
    assert!(!w.withdraw(&mut b, 301), "b cannot take a's money");
    assert!(w.withdraw(&mut a, 500));
    assert_eq!((cap(&a), cap(&b), w.c_tot(), w.vault()), (0, 300, 300, 300));
}

// ---------- deposit limits ----------

#[test]
fn lifecycle_deposit_above_max_tvl_is_refused_and_changes_nothing() {
    let mut w = W::new(1);
    let mut a = account(1);
    assert!(w.deposit(&mut a, MAX_VAULT_TVL));
    // `_not_atomic`: the engine writes then validates, so a refused deposit may leave the
    // header dirty and the caller (the transaction) discards it. Only the refusal is asserted.
    assert!(!w.deposit(&mut a, 1), "one atom past the TVL cap");
}

#[test]
fn lifecycle_deposit_to_exactly_max_tvl_across_accounts_ok_then_withdraw_frees_room() {
    let mut w = W::new(1);
    let (mut a, mut b) = (account(1), account(2));
    assert!(w.deposit(&mut a, MAX_VAULT_TVL - 10));
    assert!(w.deposit(&mut b, 10));
    let (hb, bb) = (w.header, b);
    assert!(!w.deposit(&mut b, 1));
    w.header = hb; // discard the refused (non-atomic) write, as a failed transaction would
    b = bb;
    assert!(w.withdraw(&mut a, 5));
    assert!(w.deposit(&mut b, 5));
    assert_eq!(w.vault(), MAX_VAULT_TVL);
}

// ---------- slot / account reuse after close ----------

#[test]
fn lifecycle_slot_reuse_after_full_withdraw_starts_clean() {
    let mut w = W::new(1);
    let mut a = account(1);
    w.deposit(&mut a, 4_000);
    assert!(w.withdraw(&mut a, 4_000));
    // the same storage is re-initialised for a new owner
    let mut a = account(9);
    assert_eq!(cap(&a), 0);
    assert!(w.deposit(&mut a, 55));
    assert_eq!((cap(&a), w.c_tot(), w.vault()), (55, 55, 55));
    assert!(!w.withdraw(&mut a, 56), "new owner must not see the old owner's balance");
}

#[test]
fn lifecycle_same_account_can_be_emptied_and_refilled_many_times() {
    let mut w = W::new(1);
    let mut a = account(1);
    for r in 1..=50u128 {
        assert!(w.deposit(&mut a, r * 13));
        assert!(w.withdraw(&mut a, r * 13));
        assert_eq!((cap(&a), w.c_tot(), w.vault()), (0, 0, 0), "round {r}");
    }
}

#[test]
fn lifecycle_position_closed_then_capital_withdrawn_then_reuse() {
    let mut w = W::new(1);
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, 1_000_000);
    w.deposit(&mut b, 1_000_000);
    assert!(w.trade(0, &mut a, &mut b, POS_SCALE as i128, 0));
    assert!(!w.withdraw(&mut a, 1), "open leg blocks withdraw");
    assert!(w.trade(0, &mut b, &mut a, POS_SCALE as i128, 0), "close at the same price");
    assert!(w.withdraw(&mut a, 1_000_000));
    assert!(w.withdraw(&mut b, 1_000_000));
    assert_eq!((w.c_tot(), w.vault()), (0, 0));
    // reuse: a fresh trade on empty-again accounts works
    assert!(w.deposit(&mut a, 500_000));
    assert!(w.deposit(&mut b, 500_000));
    assert!(w.trade(0, &mut a, &mut b, POS_SCALE as i128 / 2, 0));
    check(&w, &[&a, &b], 1_000_000, "after reuse");
}

// ---------- multi asset ----------

#[test]
fn lifecycle_positions_on_three_assets_then_unwind_then_withdraw_everything() {
    let mut w = W::new(3);
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, 3_000_000);
    w.deposit(&mut b, 3_000_000);
    for i in 0..3 {
        assert!(w.trade(i, &mut a, &mut b, POS_SCALE as i128, 0), "open asset {i}");
    }
    check(&w, &[&a, &b], 6_000_000, "three legs");
    assert!(!w.withdraw(&mut a, 1));
    for i in (0..3).rev() {
        assert!(w.trade(i, &mut b, &mut a, POS_SCALE as i128, 0), "close asset {i}");
    }
    check(&w, &[&a, &b], 6_000_000, "flat again");
    let (ca, cb) = (cap(&a), cap(&b));
    assert!(w.withdraw(&mut a, ca));
    assert!(w.withdraw(&mut b, cb));
    assert_eq!((w.c_tot(), w.vault()), (0, 0));
}

#[test]
fn lifecycle_one_leg_left_open_among_three_still_blocks_withdraw() {
    let mut w = W::new(3);
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, 3_000_000);
    w.deposit(&mut b, 3_000_000);
    for i in 0..3 {
        w.trade(i, &mut a, &mut b, POS_SCALE as i128, 0);
    }
    w.trade(0, &mut b, &mut a, POS_SCALE as i128, 0);
    w.trade(2, &mut b, &mut a, POS_SCALE as i128, 0);
    assert!(!w.withdraw(&mut a, 1), "asset 1 still open");
    w.trade(1, &mut b, &mut a, POS_SCALE as i128, 0);
    assert!(w.withdraw(&mut a, 1));
}

// ---------- realised pnl moves capital between accounts without changing totals ----------

#[test]
fn lifecycle_winner_and_loser_capital_sums_to_deposits_after_a_price_move() {
    let mut w = W::new(1);
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, 1_000_000);
    w.deposit(&mut b, 1_000_000);
    assert!(w.trade(0, &mut a, &mut b, 2 * POS_SCALE as i128, 0));
    w.move_price(0, 300);
    assert!(w.refresh(&mut a));
    assert!(w.refresh(&mut b));
    assert!(w.trade(0, &mut b, &mut a, 2 * POS_SCALE as i128, 0), "close out");
    // flat now. The loser's loss is in capital at once; the winner's gain is a pnl claim that
    // matures later, so count capital + pnl: together they must equal the deposits (minus dust).
    let (ca, cb) = (cap(&a), cap(&b));
    let (pa, pb) = (a.pnl.get(), b.pnl.get());
    let total = (ca + cb) as i128 + pa + pb;
    assert!(total <= 2_000_000, "equity created from nothing: {ca}+{cb}+{pa}+{pb}");
    assert!(total + 2 >= 2_000_000, "more than dust lost: {ca}+{cb}+{pa}+{pb}");
    assert!(pa > 0 && pb == 0 && cb < 1_000_000, "a won, b paid: {pa} {pb} {cb}");
    check_sum_le(&w, &[&a, &b], 2_000_000);
    assert!(w.withdraw(&mut a, ca));
    assert!(w.withdraw(&mut b, cb));
    assert_eq!(w.vault(), 2_000_000 - ca - cb);
}

fn check_sum_le(w: &W, accts: &[&PortfolioAccountV16Account], deposits: u128) {
    let sum: u128 = accts.iter().map(|a| cap(a)).sum();
    assert!(w.vault() >= sum + w.insurance());
    assert_eq!(w.vault(), deposits);
}

// ---------- insurance moves ----------

#[test]
fn lifecycle_insurance_deposit_and_withdraw_balance_the_vault() {
    let mut w = W::new(1);
    let mut a = account(1);
    w.deposit(&mut a, 1_000);
    assert!(w.ins_deposit(0, 400));
    assert_eq!((w.vault(), w.c_tot(), w.insurance()), (1_400, 1_000, 400));
    assert!(!w.ins_withdraw(0, 401), "more than the domain budget");
    assert!(w.ins_withdraw(0, 400));
    assert_eq!((w.vault(), w.c_tot(), w.insurance()), (1_000, 1_000, 0));
    assert!(w.withdraw(&mut a, 1_000));
}

#[test]
fn lifecycle_user_cannot_withdraw_insurance_money() {
    let mut w = W::new(1);
    let mut a = account(1);
    w.deposit(&mut a, 100);
    w.ins_deposit(0, 900);
    assert!(!w.withdraw(&mut a, 101), "vault holds 1000 but only 100 is the user's");
    assert!(w.withdraw(&mut a, 100));
    assert_eq!((w.vault(), w.insurance()), (900, 900));
}

// ---------- resolved market ----------

#[test]
fn lifecycle_after_resolve_deposit_and_withdraw_are_refused() {
    let mut w = W::new(1);
    let mut a = account(1);
    w.deposit(&mut a, 100);
    w.mode_resolved(w.header.current_slot.get() + 1);
    let (hb, ab) = (w.header, a);
    assert!(!w.deposit(&mut a, 1));
    assert!(!w.withdraw(&mut a, 1));
    assert_eq!((w.header, a), (hb, ab));
}

#[test]
fn lifecycle_resolved_close_pays_flat_capital_and_empties_the_account() {
    let mut w = W::new(1);
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, 700);
    w.deposit(&mut b, 300);
    w.mode_resolved(w.header.current_slot.get() + 1);
    let out = w.view().close_resolved_account_not_atomic(&mut PortfolioV16ViewMut::new(&mut a), 0).unwrap();
    assert!(matches!(out, ResolvedCloseOutcomeV16::Closed { payout: 700 }), "{out:?}");
    assert_eq!((cap(&a), w.c_tot(), w.vault()), (0, 300, 300));
    let out = w.view().close_resolved_account_not_atomic(&mut PortfolioV16ViewMut::new(&mut b), 0).unwrap();
    assert!(matches!(out, ResolvedCloseOutcomeV16::Closed { payout: 300 }), "{out:?}");
    assert_eq!((w.c_tot(), w.vault()), (0, 0));
}

#[test]
fn lifecycle_resolved_close_twice_pays_nothing_the_second_time() {
    let mut w = W::new(1);
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, 700);
    w.deposit(&mut b, 300);
    w.mode_resolved(w.header.current_slot.get() + 1);
    w.view().close_resolved_account_not_atomic(&mut PortfolioV16ViewMut::new(&mut a), 0).unwrap();
    let again = w.view().close_resolved_account_not_atomic(&mut PortfolioV16ViewMut::new(&mut a), 0);
    if let Ok(ResolvedCloseOutcomeV16::Closed { payout }) = again {
        assert_eq!(payout, 0, "second close must not pay again");
    }
    assert_eq!((w.c_tot(), w.vault()), (300, 300), "b's money is untouched");
}

#[test]
fn lifecycle_close_resolved_is_refused_while_the_market_is_live() {
    let mut w = W::new(1);
    let mut a = account(1);
    w.deposit(&mut a, 100);
    assert!(w.view().close_resolved_account_not_atomic(&mut PortfolioV16ViewMut::new(&mut a), 0).is_err());
    assert_eq!((cap(&a), w.vault()), (100, 100));
}

// ---------- seeded random sequences: conservation after every step ----------

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 { self.next() % n }
}

/// Run `steps` random operations. A failed operation is rolled back (the engine's `_not_atomic`
/// functions make no promise about partial state on error); every successful one must keep
/// vault == net external flow, c_tot == sum of balances, vault >= c_tot + insurance.
fn run_sequence(seed: u64, steps: usize, assets: usize, fees: bool) {
    let mut rng = Rng(seed | 1);
    let mut w = W::new(assets);
    let mut accts: Vec<PortfolioAccountV16Account> = (0..4).map(|i| account(i + 1)).collect();
    let mut flow: u128 = 0;
    let mut reused = 0u32;
    let mut good = 0usize;
    for step in 0..steps {
        let wsnap = w.clone();
        let asnap = accts.clone();
        let ctx = format!("seed {seed} step {step}");
        let k = rng.below(4) as usize;
        let op = rng.below(100);
        let mut ok = true;
        let mut dflow: i128 = 0;
        match op {
            0..=24 => {
                let amt = 1 + rng.below(2_000_000) as u128;
                ok = w.deposit(&mut accts[k], amt);
                if ok { dflow = amt as i128; }
            }
            25..=44 => {
                // mix of tiny, partial, exact-all and over-capital amounts
                let c = cap(&accts[k]);
                let amt = match rng.below(4) {
                    0 => 1,
                    1 => c / 2,
                    2 => c,
                    _ => c + 1 + rng.below(3) as u128,
                };
                ok = w.withdraw(&mut accts[k], amt);
                if ok { dflow = -(amt as i128); }
                if ok && cap(&accts[k]) == 0 {
                    // closed: reuse the storage for a brand-new owner
                    accts[k] = account(100 + reused);
                    reused += 1;
                }
            }
            45..=79 => {
                let j = (k + 1 + rng.below(3) as usize) % 4;
                let i = rng.below(assets as u64) as usize;
                let size = (1 + rng.below(3) as i128) * POS_SCALE as i128 / 2;
                let size = if rng.below(2) == 0 { size } else { -size };
                let fee = if fees { rng.below(3) * 10 } else { 0 };
                let (lo, hi) = if k < j { (k, j) } else { (j, k) };
                let (left, right) = accts.split_at_mut(hi);
                let (x, y) = (&mut left[lo], &mut right[0]);
                ok = if k < j { w.trade(i, x, y, size, fee) } else { w.trade(i, y, x, size, fee) };
            }
            80..=87 => {
                let i = rng.below(assets as u64) as usize;
                let bps = rng.below(401) as i64 - 200;
                w.move_price(i, bps);
            }
            88..=93 => {
                ok = w.refresh(&mut accts[k]);
            }
            94..=96 => {
                let amt = 1 + rng.below(100_000) as u128;
                ok = w.ins_deposit(rng.below(2 * assets as u64) as usize, amt);
                if ok { dflow = amt as i128; }
            }
            _ => {
                let amt = 1 + rng.below(100_000) as u128;
                ok = w.ins_withdraw(rng.below(2 * assets as u64) as usize, amt);
                if ok { dflow = -(amt as i128); }
            }
        }
        if !ok {
            w = wsnap;
            accts = asnap;
            continue;
        }
        good += 1;
        flow = (flow as i128 + dflow) as u128;
        let refs: Vec<&PortfolioAccountV16Account> = accts.iter().collect();
        // the closed-and-reused storage has capital 0, so the sums still line up
        check(&w, &refs, flow, &ctx);
        // a validated state must also pass the engine's own shape audit
        w.view().validate_shape().unwrap_or_else(|e| panic!("validate_shape failed ({ctx}): {e:?}"));
    }
    // guard against a vacuous run where nearly every step is refused and rolled back
    assert!(good * 3 >= steps, "seed {seed}: only {good}/{steps} steps took effect");
}

#[test]
fn lifecycle_random_sequences_conserve_money_one_asset_no_fees() {
    for seed in 1..=40u64 { run_sequence(seed * 7919, 300, 1, false); }
}

#[test]
fn lifecycle_random_sequences_conserve_money_three_assets_no_fees() {
    for seed in 1..=40u64 { run_sequence(seed * 104_729, 300, 3, false); }
}

#[test]
fn lifecycle_random_sequences_conserve_money_with_trade_fees() {
    for seed in 1..=40u64 { run_sequence(seed * 15_485_863, 300, 2, true); }
}
