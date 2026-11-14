//! Crank / refresh ordering edge cases for the v2.2 risk engine (job P15, tests only).
//!
//! Extends `issue172_crank_order_double_charge.rs`. Spec requirement 27 and the K/F settlement
//! text say the end state must not depend on which account is refreshed first, how often a crank
//! is repeated, or whether the engine catches up over many slots at once or one slot at a time.
//!
//! Everything goes through the public engine API.

use percolator::{
    AutoCrankObservationV16, AutoCrankPlanV16, AutoCrankWorkV16, EngineAssetSlotV16Account, Market,
    MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, PermissionlessCrankActionV16,
    PermissionlessCrankRequestV16, PortfolioAccountV16Account, PortfolioV16ViewMut,
    ProvenanceHeaderV16, ProvenanceHeaderV16Account, TradeRequestV16, V16Config, BOUND_SCALE,
    POS_SCALE,
};

const START_PRICE: u64 = 1_000_000;
const CAPITAL: u128 = 1_000_000_000_000;
const RATE: i128 = 700;

type Slab = (MarketGroupV16HeaderAccount, Vec<Market<u64>>);
type Acct = PortfolioAccountV16Account;

fn market(cap_bps: u64, margins: Option<(u64, u64)>, start: u64) -> Slab {
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    cfg.max_price_move_bps_per_slot = cap_bps;
    if let Some((im, mm)) = margins {
        cfg.initial_margin_bps = im;
        cfg.maintenance_margin_bps = mm;
    } else {
        cfg.max_abs_funding_e9_per_slot = 1_000;
        cfg.max_accrual_dt_slots = 50;
        cfg.min_funding_lifetime_slots = 50;
    }
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut markets = vec![Market::new(0, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, start, 1)
        .unwrap();
    (header, markets)
}

fn account(seed: u32, slab: &mut Slab, deposit: u128) -> Acct {
    let mut key = [0u8; 32];
    key[..4].copy_from_slice(&seed.to_le_bytes());
    key[31] = 0x5A;
    let h =
        ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new([1; 32], key, [3; 32]));
    let mut a = Acct::default();
    a.init_empty_in_place(h).unwrap();
    let mut m = MarketGroupV16ViewMut::new(&mut slab.0, &mut slab.1);
    m.deposit_not_atomic(&mut PortfolioV16ViewMut::new(&mut a), deposit)
        .unwrap();
    a
}

fn trade(slab: &mut Slab, long: &mut Acct, short: &mut Acct, units: i128, price: u64) {
    let mut m = MarketGroupV16ViewMut::new(&mut slab.0, &mut slab.1);
    m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
        &mut PortfolioV16ViewMut::new(long),
        &mut PortfolioV16ViewMut::new(short),
        TradeRequestV16 {
            asset_index: 0,
            size_q: units * POS_SCALE as i128,
            exec_price: price,
            fee_bps: 0,
        },
        true,
    )
    .unwrap();
}

fn accrue(slab: &mut Slab, slot: u64, price: u64, rate: i128) {
    let mut m = MarketGroupV16ViewMut::new(&mut slab.0, &mut slab.1);
    m.set_asset_raw_oracle_target_not_atomic(0, price).unwrap();
    m.accrue_asset_to_not_atomic(0, slot, price, rate, true)
        .unwrap();
}

fn refresh(slab: &mut Slab, a: &mut Acct) {
    let mut m = MarketGroupV16ViewMut::new(&mut slab.0, &mut slab.1);
    m.full_account_refresh_not_atomic(&mut PortfolioV16ViewMut::new(a))
        .unwrap();
}

fn crank_refresh_once(slab: &mut Slab, a: &mut Acct) {
    let now_slot = slab.0.current_slot.get();
    let price = slab.1[0]
        .engine
        .asset
        .try_to_runtime()
        .unwrap()
        .effective_price;
    let mut m = MarketGroupV16ViewMut::new(&mut slab.0, &mut slab.1);
    m.permissionless_crank_not_atomic(
        &mut PortfolioV16ViewMut::new(a),
        PermissionlessCrankRequestV16 {
            now_slot,
            asset_index: 0,
            effective_price: price,
            funding_rate_e9: 0,
            action: PermissionlessCrankActionV16::Refresh,
        },
    )
    .expect("Refresh crank");
}

/// Auto-crank one account to a fixpoint (bounded). Returns the number of non-idle steps.
fn auto_crank(slab: &mut Slab, a: &mut Acct) -> usize {
    let now_slot = slab.0.current_slot.get();
    let price = slab.1[0]
        .engine
        .asset
        .try_to_runtime()
        .unwrap()
        .effective_price;
    for n in 0..64 {
        let mut m = MarketGroupV16ViewMut::new(&mut slab.0, &mut slab.1);
        let r = m
            .permissionless_auto_crank_not_atomic(
                &mut PortfolioV16ViewMut::new(a),
                AutoCrankWorkV16 {
                    now_slot,
                    observations: &[AutoCrankObservationV16 {
                        asset_index: 0,
                        effective_price: price,
                        funding_rate_e9: 0,
                    }],
                    resolved_close_fee_rate_per_slot: 0,
                },
            )
            .unwrap();
        if r.selected == AutoCrankPlanV16::NoAction {
            return n;
        }
    }
    panic!("auto crank reached no fixpoint");
}

#[derive(Debug, PartialEq, Eq)]
struct Snap {
    accounts: Vec<(u128, i128)>,
    vault: u128,
    c_tot: u128,
    insurance: u128,
    k_f: (i128, i128, i128, i128),
    /// Per side: claims, fresh_reserved, spent, provider_receivable, fresh_unliened.
    backing: [[u128; 5]; 2],
    unowned: u128,
}

fn snap(slab: &Slab, accts: &[Acct]) -> Snap {
    let a = slab.1[0].engine.asset.try_to_runtime().unwrap();
    let e = &slab.1[0].engine;
    let backing = [
        (e.source_credit_long, e.backing_long),
        (e.source_credit_short, e.backing_short),
    ]
    .map(|(c, b)| {
        let (c, b) = (c.try_to_runtime().unwrap(), b.try_to_runtime().unwrap());
        [
            c.positive_claim_bound_num / BOUND_SCALE,
            c.fresh_reserved_backing_num / BOUND_SCALE,
            c.spent_backing_num / BOUND_SCALE,
            c.provider_receivable_num / BOUND_SCALE,
            b.fresh_unliened_backing_num / BOUND_SCALE,
        ]
    });
    let h = &slab.0;
    let owned = h.c_tot.get()
        + h.insurance.get()
        + h.backing_provider_earnings_total.get()
        + h.source_fresh_backing_total_num.get() / BOUND_SCALE;
    Snap {
        backing,
        unowned: h.vault.get() - owned,
        accounts: accts
            .iter()
            .map(|x| (x.capital.get(), x.pnl.get()))
            .collect(),
        vault: slab.0.vault.get(),
        c_tot: slab.0.c_tot.get(),
        insurance: slab.0.insurance.get(),
        k_f: (a.k_long, a.k_short, a.f_long_num, a.f_short_num),
    }
}

fn validate(slab: &mut Slab, accts: &mut [Acct]) {
    let m = MarketGroupV16ViewMut::new(&mut slab.0, &mut slab.1);
    m.validate_shape().unwrap();
    for a in accts.iter_mut() {
        PortfolioV16ViewMut::new(a)
            .validate_with_market(&m.as_view())
            .unwrap();
    }
}

fn permutations(n: usize) -> Vec<Vec<usize>> {
    fn go(cur: &mut Vec<usize>, used: &mut Vec<bool>, out: &mut Vec<Vec<usize>>) {
        if cur.len() == used.len() {
            out.push(cur.clone());
            return;
        }
        for i in 0..used.len() {
            if !used[i] {
                used[i] = true;
                cur.push(i);
                go(cur, used, out);
                cur.pop();
                used[i] = false;
            }
        }
    }
    let mut out = vec![];
    go(&mut vec![], &mut vec![false; n], &mut out);
    out
}

/// Four accounts, two long and two short with different sizes, entry prices and funding.
/// Price walks inside the per-slot cap so nobody is liquidated.
/// `per_slot`: accrue every slot; otherwise one jump over the same span (same end price).
fn build(per_slot: bool, cap_bps: u64, slots: u64, end_price: u64) -> (Slab, Vec<Acct>) {
    let mut s = market(cap_bps, None, START_PRICE);
    let mut l1 = account(1, &mut s, CAPITAL);
    let mut l2 = account(2, &mut s, CAPITAL);
    let mut s1 = account(3, &mut s, CAPITAL);
    let mut s2 = account(4, &mut s, CAPITAL);
    trade(&mut s, &mut l1, &mut s1, 7, START_PRICE);
    trade(&mut s, &mut l2, &mut s2, 5, START_PRICE);
    trade(&mut s, &mut l1, &mut s2, 3, START_PRICE);
    let start_slot = 1;
    let last = start_slot + slots;
    if per_slot {
        let diff = end_price as i128 - START_PRICE as i128;
        for i in 1..=slots {
            let p = (START_PRICE as i128 + diff * i as i128 / slots as i128) as u64;
            accrue(&mut s, start_slot + i, p, RATE);
        }
    } else {
        accrue(&mut s, last, end_price, RATE);
    }
    (s, vec![l1, l2, s1, s2])
}

// ---------------------------------------------------------------------------------------------

/// Refreshing the same four accounts in any of the 24 orders gives one end state.
#[test]
fn crank_refresh_all_24_orders_agree() {
    let mut reference: Option<Snap> = None;
    for perm in permutations(4) {
        let (mut s, mut accts) = build(true, 100, 10, START_PRICE + START_PRICE / 200);
        for &i in &perm {
            refresh(&mut s, &mut accts[i]);
        }
        validate(&mut s, &mut accts);
        let got = snap(&s, &accts);
        match &reference {
            None => reference = Some(got),
            Some(r) => assert_eq!(&got, r, "refresh order {perm:?} changed the end state"),
        }
    }
}

/// Same, but the order is also mixed with a second refresh of an earlier account in between.
#[test]
fn crank_refresh_interleaved_repeats_agree_with_single_pass() {
    let (mut s0, mut a0) = build(true, 100, 10, START_PRICE - START_PRICE / 150);
    for i in 0..4 {
        refresh(&mut s0, &mut a0[i]);
    }
    let want = snap(&s0, &a0);
    for perm in permutations(4) {
        let (mut s, mut a) = build(true, 100, 10, START_PRICE - START_PRICE / 150);
        for &i in &perm {
            refresh(&mut s, &mut a[i]);
            refresh(&mut s, &mut a[perm[0]]);
        }
        validate(&mut s, &mut a);
        assert_eq!(snap(&s, &a), want, "interleaved order {perm:?}");
    }
}

/// A second (third, fourth) refresh in the same slot is a no-op: no double charge.
#[test]
fn crank_repeated_refresh_in_one_slot_is_idempotent() {
    let (mut s, mut a) = build(true, 100, 10, START_PRICE + START_PRICE / 100);
    let before = snap(&s, &a);
    for i in 0..4 {
        refresh(&mut s, &mut a[i]);
    }
    assert_ne!(
        snap(&s, &a).accounts,
        before.accounts,
        "refresh must settle something"
    );
    let once = snap(&s, &a);
    for _ in 0..3 {
        for i in 0..4 {
            refresh(&mut s, &mut a[i]);
        }
    }
    assert_eq!(snap(&s, &a), once);
    // The Refresh crank on a settled account changes nothing either.
    for i in 0..4 {
        crank_refresh_once(&mut s, &mut a[i]);
        crank_refresh_once(&mut s, &mut a[i]);
    }
    assert_eq!(snap(&s, &a), once);
    validate(&mut s, &mut a);
}

/// Auto-crank on a settled, healthy account selects NoAction and mutates nothing.
#[test]
fn crank_auto_crank_after_fixpoint_is_a_noop() {
    let (mut s, mut a) = build(true, 100, 10, START_PRICE + START_PRICE / 100);
    // Cranking one account can move shared state another account then settles, so sweep
    // until a whole pass is idle (bounded).
    let mut passes = 0;
    while (0..4).map(|i| auto_crank(&mut s, &mut a[i])).sum::<usize>() > 0 {
        passes += 1;
        assert!(passes < 8, "sweeps never went idle");
    }
    let once = snap(&s, &a);
    for i in 0..4 {
        assert_eq!(
            auto_crank(&mut s, &mut a[i]),
            0,
            "account {i} not at fixpoint"
        );
    }
    assert_eq!(snap(&s, &a), once);
}

/// Auto-crank order over all 24 permutations agrees.
#[test]
fn crank_auto_crank_all_24_orders_agree() {
    let mut reference: Option<Snap> = None;
    for perm in permutations(4) {
        let (mut s, mut a) = build(true, 100, 10, START_PRICE + START_PRICE / 200);
        for &i in &perm {
            auto_crank(&mut s, &mut a[i]);
        }
        validate(&mut s, &mut a);
        let got = snap(&s, &a);
        match &reference {
            None => reference = Some(got),
            Some(r) => assert_eq!(&got, r, "auto-crank order {perm:?} changed the end state"),
        }
    }
}

/// Refresh and auto-crank reach the same state (one fixed order).
#[test]
fn crank_refresh_and_auto_crank_agree() {
    let (mut s1, mut a1) = build(true, 100, 10, START_PRICE + START_PRICE / 200);
    let (mut s2, mut a2) = build(true, 100, 10, START_PRICE + START_PRICE / 200);
    for i in 0..4 {
        refresh(&mut s1, &mut a1[i]);
        auto_crank(&mut s2, &mut a2[i]);
    }
    assert_eq!(snap(&s1, &a1), snap(&s2, &a2));
}

/// Catching up over many slots in one accrue vs one slot at a time, same end price: per-account
/// equity must agree to within a few atoms (funding rounding differs by path).
#[test]
fn crank_one_jump_vs_slot_by_slot_agree_within_rounding() {
    let end = START_PRICE + START_PRICE / 200;
    let (mut sa, mut aa) = build(false, 100, 10, end);
    let (mut sb, mut ab) = build(true, 100, 10, end);
    for i in 0..4 {
        refresh(&mut sa, &mut aa[i]);
        refresh(&mut sb, &mut ab[i]);
    }
    validate(&mut sa, &mut aa);
    validate(&mut sb, &mut ab);
    let (x, y) = (snap(&sa, &aa), snap(&sb, &ab));
    assert_eq!(x.vault, y.vault);
    assert_eq!(x.insurance, y.insurance);
    let tot = |s: &Snap| {
        s.accounts
            .iter()
            .map(|(c, p)| *c as i128 + *p)
            .sum::<i128>()
    };
    // Zero-sum book: total equity is conserved up to rounding dust either way.
    assert!((tot(&x) - tot(&y)).abs() <= 8, "{} vs {}", tot(&x), tot(&y));
    for (p, q) in x.accounts.iter().zip(&y.accounts) {
        let (e1, e2) = (p.0 as i128 + p.1, q.0 as i128 + q.1);
        assert!((e1 - e2).abs() <= 8, "per-account equity {e1} vs {e2}");
    }
}

/// Partial cranks: refreshing only half of the accounts, then accruing more slots, then
/// refreshing everyone, equals refreshing everyone once at the end.
#[test]
fn crank_partial_then_more_slots_equals_single_catch_up() {
    let (mut sa, mut aa) = build(true, 100, 10, START_PRICE + START_PRICE / 200);
    accrue(&mut sa, 13, START_PRICE + START_PRICE / 100, RATE);
    let (mut sb, mut ab) = build(true, 100, 10, START_PRICE + START_PRICE / 200);
    refresh(&mut sb, &mut ab[0]);
    refresh(&mut sb, &mut ab[3]);
    accrue(&mut sb, 13, START_PRICE + START_PRICE / 100, RATE);
    for i in 0..4 {
        refresh(&mut sa, &mut aa[i]);
        refresh(&mut sb, &mut ab[i]);
    }
    let (x, y) = (snap(&sa, &aa), snap(&sb, &ab));
    assert_eq!(x.k_f, y.k_f);
    assert_eq!(x.vault, y.vault);
    let tot = |s: &Snap| {
        s.accounts
            .iter()
            .map(|(c, p)| *c as i128 + *p)
            .sum::<i128>()
    };
    assert!((tot(&x) - tot(&y)).abs() <= 8);
    for (p, q) in x.accounts.iter().zip(&y.accounts) {
        assert!(((p.0 as i128 + p.1) - (q.0 as i128 + q.1)).abs() <= 8);
    }
}

// Liquidation scenario (issue172 case 2 shape): a thin short is wiped by capped price rises.
/// Cranking the loser, the winner, and two bystanders in all 24 orders gives the same terminal
/// state, including the unowned remainder of the vault.
fn liq_build() -> (Slab, Vec<Acct>) {
    let p0 = 10_000;
    let mut s = market(2_000, Some((5_000, 2_500)), p0);
    let mut loser = account(1, &mut s, 692_730);
    let mut winner = account(2, &mut s, 904_405);
    let mut b1 = account(3, &mut s, 1_000_000);
    let mut b2 = account(4, &mut s, 1_000_000);
    trade(&mut s, &mut winner, &mut loser, 85, p0);
    trade(&mut s, &mut b1, &mut b2, 5, p0);
    let mut p = p0;
    for slot in 2..8 {
        p += p * 2_000 / 10_000;
        accrue(&mut s, slot, p, 0);
    }
    (s, vec![loser, winner, b1, b2])
}

/// Cranking the loser, the winner and two bystanders in all 24 orders (twice each) gives one
/// terminal state, backing domains and unowned value included. The loser must be wiped out.
#[test]
fn crank_liquidation_all_24_orders_agree() {
    let mut reference: Option<Snap> = None;
    for perm in permutations(4) {
        let (mut s, mut a) = liq_build();
        for _ in 0..2 {
            for &i in &perm {
                auto_crank(&mut s, &mut a[i]);
            }
        }
        validate(&mut s, &mut a);
        let got = snap(&s, &a);
        assert_eq!(
            got.accounts[0].0 as i128 + got.accounts[0].1,
            0,
            "loser not wiped out"
        );
        match &reference {
            None => reference = Some(got),
            Some(r) => assert_eq!(&got, r, "liquidation crank order {perm:?} diverged"),
        }
    }
}
