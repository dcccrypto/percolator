//! LIVEFIX (v22live, 2026-10-20): rollback-faithful randomized Live -> Resolved walk, used as
//! the before/after measurement for F-LIVE-1 / F-LIVE-2 (their changes act in the Live phase:
//! random trades, a random price path with random partial refreshes; then Resolved closes).
//!
//! The walk (`S16acWorld`, `s16ac_walk`) is copied VERBATIM from the v22med branch
//! `fix/v22med-1020-s1-6ac` (e59aa6a6, tests/v16_spec_tests.rs, AUD-S1-6), with its fixtures
//! from tests/v16_spec_tests.rs, into its own file so it does not conflict with that branch.
//! Every engine call is a transaction (an Err restores market + accounts, as the SVM does), and a
//! deterministic drain at the end means "not closed" = stuck, not "ran out of steps".
#![allow(clippy::type_complexity)]

use percolator::{
    AutoCrankObservationV16, AutoCrankOutcomeV16, AutoCrankWorkV16, EngineAssetSlotV16Account,
    Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, PortfolioAccountV16Account,
    PortfolioV16ViewMut, ProvenanceHeaderV16, ProvenanceHeaderV16Account, ResolvedCloseOutcomeV16,
    TradeRequestV16, V16Config, V16Error, POS_SCALE,
};

fn ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    ([1; 32], [2; 32], [3; 32])
}

fn market_fixture(
    market_slots: u32,
    init_price: u64,
) -> (MarketGroupV16HeaderAccount, Vec<Market<u64>>) {
    let (market_id, _, _) = ids();
    let max_portfolio_assets =
        market_slots.min(percolator::V16_MAX_PORTFOLIO_ASSETS_N as u32) as u16;
    let cfg =
        V16Config::public_user_fund_with_market_slots(max_portfolio_assets, market_slots, 0, 10);
    let mut header =
        MarketGroupV16HeaderAccount::new_dynamic(market_id, cfg, market_slots, 0).unwrap();
    let mut markets = (0..market_slots)
        .map(|i| Market::new(i as u64, EngineAssetSlotV16Account::default()))
        .collect::<Vec<_>>();
    for i in 0..market_slots as usize {
        header
            .activate_empty_asset_slot_not_atomic(
                i as u32,
                &mut markets[i].engine,
                init_price,
                (i + 1) as u64,
            )
            .unwrap();
    }
    (header, markets)
}

fn account_fixture(_market_slots: u32, account_seed: u8) -> PortfolioAccountV16Account {
    let (market_id, _, owner) = ids();
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        market_id,
        [account_seed; 32],
        owner,
    ));
    let mut account = PortfolioAccountV16Account::default();
    account.init_empty_in_place(header).unwrap();
    account
}

/// LIVEFIX measurement switch: mid-path trades that can create liens (default off).
fn liens_in_live() -> bool {
    std::env::var("LIVEFIX_WALK_LIENS").map(|v| v == "1").unwrap_or(false)
}

/// LIVEFIX measurement switch (default off = the v22med walk unchanged).
fn keeper_expires_in_live() -> bool {
    std::env::var("LIVEFIX_WALK_KEEPER").map(|v| v == "1").unwrap_or(false)
}

fn signed_q(q: u128) -> i128 {
    i128::try_from(q).unwrap()
}

struct S16Lcg(u64);
impl S16Lcg {
    fn n(&mut self, m: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) % m
    }
}

struct S16acWorld {
    header: MarketGroupV16HeaderAccount,
    markets: Vec<Market<u64>>,
    acc: [PortfolioAccountV16Account; 4],
}

impl S16acWorld {
    fn tx<R>(
        &mut self,
        f: impl FnOnce(&mut MarketGroupV16ViewMut<'_, u64>, &mut [PortfolioAccountV16Account; 4]) -> Result<R, V16Error>,
    ) -> Result<R, V16Error> {
        let (h, m, a) = (self.header, self.markets.clone(), self.acc);
        let r = {
            let mut mk = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
            f(&mut mk, &mut self.acc)
        };
        if r.is_err() {
            self.header = h;
            self.markets = m;
            self.acc = a;
        } else {
            // R-E L-2: in every committed state, a side with no stored position has no stale
            // account and no pending obligation, which is why `domain_creditor_claims_final`'s
            // stale/pending terms never decide it (see the doc at that predicate).
            let asset = self.markets[0].engine.asset.try_to_runtime().unwrap();
            for (stored, stale, pending) in [
                (asset.stored_pos_count_long, asset.stale_account_count_long, asset.pending_obligation_count_long),
                (asset.stored_pos_count_short, asset.stale_account_count_short, asset.pending_obligation_count_short),
            ] {
                assert!(stale <= stored && pending <= stored, "stored {stored} stale {stale} pending {pending}");
            }
        }
        r
    }
    fn close(&mut self, who: usize, paid: &mut [u128; 4], closed: &mut [bool; 4], auto: bool) -> Result<bool, V16Error> {
        let r = if auto {
            self.tx(|m, acc| {
                let observations = [AutoCrankObservationV16 {
                    asset_index: 0,
                    effective_price: m.markets[0].engine.asset.effective_price.get(),
                    funding_rate_e9: 0,
                }];
                let work = AutoCrankWorkV16 {
                    now_slot: m.header.current_slot.get(),
                    observations: &observations,
                    resolved_close_fee_rate_per_slot: 0,
                };
                match m.permissionless_auto_crank_not_atomic(&mut PortfolioV16ViewMut::new(&mut acc[who]), work)?.outcome {
                    AutoCrankOutcomeV16::ResolvedClose(ResolvedCloseOutcomeV16::Closed { payout }) => Ok(Some(payout)),
                    _ => Ok(None),
                }
            })
        } else {
            self.tx(|m, acc| {
                match m.close_resolved_account_not_atomic(&mut PortfolioV16ViewMut::new(&mut acc[who]), 0)? {
                    ResolvedCloseOutcomeV16::Closed { payout } => Ok(Some(payout)),
                    _ => Ok(None),
                }
            })
        };
        match r {
            Ok(Some(p)) => {
                paid[who] += p;
                closed[who] = true;
                Ok(true)
            }
            Ok(None) => Ok(false),
            Err(e) => Err(e),
        }
    }
    fn topup(&mut self, who: usize, paid: &mut [u128; 4]) {
        if !self.acc[who].resolved_payout_receipt.try_to_runtime().unwrap().present {
            return;
        }
        if let Ok(t) = self.tx(|m, acc| m.claim_resolved_payout_topup_not_atomic(&mut PortfolioV16ViewMut::new(&mut acc[who]))) {
            paid[who] += t;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct S16acOut {
    paid: [u128; 4],
    closed: [bool; 4],
    vault: u128,
    insurance: u128,
    deposits: u128,
    last_err: [Option<V16Error>; 4],
}

/// Random trades, random price path with random partial refreshes, Resolved, then `steps`
/// random close / auto-crank close / top-up / expiry / clock actions, then a deterministic
/// drain (clock far forward, expire both buckets, every open account tries to close in a
/// rotating order). None when the scenario could not be built.
fn s16ac_walk(seed: u64, steps: usize) -> Option<S16acOut> {
    let mut r = S16Lcg(seed * 7919 + 1);
    // LIVEFIX: a second, independent stream so the v22med draw order is unchanged when off.
    let mut r2 = S16Lcg(seed * 104_729 + 7);
    let (header, markets) = market_fixture(1, 100);
    let mut w = S16acWorld {
        header,
        markets,
        acc: [account_fixture(1, 221), account_fixture(1, 222), account_fixture(1, 223), account_fixture(1, 224)],
    };
    let mut deposits = 0u128;
    for k in 0..4 {
        let c = [200u128, 400, 800, 1000][r.n(4) as usize];
        w.tx(|m, acc| m.deposit_not_atomic(&mut PortfolioV16ViewMut::new(&mut acc[k]), c)).unwrap();
        deposits += c;
    }
    let mut trades = 0;
    for _ in 0..(1 + r.n(3)) {
        let x = r.n(4) as usize;
        let mut y = r.n(4) as usize;
        if y == x {
            y = (y + 1) % 4;
        }
        let sz = (1 + r.n(3)) as u128;
        let ok = w
            .tx(|m, acc| {
                let (lo, hi) = (x.min(y), x.max(y));
                let (a, b) = acc.split_at_mut(hi);
                let (first, second) = (&mut a[lo], &mut b[0]);
                let (l, s) = if x < y { (first, second) } else { (second, first) };
                m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
                    &mut PortfolioV16ViewMut::new(l),
                    &mut PortfolioV16ViewMut::new(s),
                    TradeRequestV16 { asset_index: 0, size_q: signed_q(sz * POS_SCALE), exec_price: 100, fee_bps: 0 },
                    true,
                )
            })
            .is_ok();
        if ok {
            trades += 1;
        }
    }
    if trades == 0 {
        return None;
    }
    let mut slot = w.header.current_slot.get();
    let mut p = 100u64;
    for _ in 0..(1 + r.n(4)) {
        let tgt = [60u64, 80, 120, 160, 220, 300][r.n(6) as usize];
        while p != tgt {
            slot += 1;
            p = if tgt > p { (p + 10).min(tgt) } else { p.saturating_sub(10).max(tgt) };
            if w
                .tx(|m, _| {
                    m.set_asset_raw_oracle_target_not_atomic(0, p)?;
                    m.accrue_asset_to_not_atomic(0, slot, p, 0, true)
                })
                .is_err()
            {
                return None;
            }
            for k in 0..4 {
                if r.n(3) == 0 {
                    let _ = w.tx(|m, acc| m.full_account_refresh_not_atomic(&mut PortfolioV16ViewMut::new(&mut acc[k])));
                }
            }
            if liens_in_live() && r2.n(4) == 0 {
                // a mid-path trade at the mark: a winner may lien its gain's backing for margin
                let x = r2.n(4) as usize;
                let y = (x + 1 + r2.n(3) as usize) % 4;
                let sz = (1 + r2.n(3)) as u128;
                let _ = w.tx(|m, acc| {
                    let (lo, hi) = (x.min(y), x.max(y));
                    let (a, b) = acc.split_at_mut(hi);
                    let (first, second) = (&mut a[lo], &mut b[0]);
                    let (l, s) = if x < y { (first, second) } else { (second, first) };
                    m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
                        &mut PortfolioV16ViewMut::new(l),
                        &mut PortfolioV16ViewMut::new(s),
                        TradeRequestV16 { asset_index: 0, size_q: signed_q(sz * POS_SCALE), exec_price: p, fee_bps: 0 },
                        true,
                    )
                });
            }
            if keeper_expires_in_live() {
                // a Live keeper running the permissionless expiry (wrapper tag 89) every slot
                for d in 0..2 {
                    let _ = w.tx(|m, _| { let s = m.header.current_slot.get(); m.expire_source_backing_bucket_not_atomic(d, s) });
                }
            }
        }
    }
    if w.tx(|m, _| m.resolve_market_not_atomic(slot)).is_err() {
        return None;
    }
    let mut paid = [0u128; 4];
    let mut closed = [false; 4];
    let mut last_err: [Option<V16Error>; 4] = [None; 4];
    for _ in 0..steps {
        let who = r.n(4) as usize;
        match r.n(8) {
            6 if !closed[who] => {
                let _ = w.close(who, &mut paid, &mut closed, true);
            }
            0..=2 if !closed[who] => {
                let _ = w.close(who, &mut paid, &mut closed, false);
            }
            3 if !closed[who] => w.topup(who, &mut paid),
            4 => {
                for d in 0..2 {
                    let _ = w.tx(|m, _| { let s = m.header.current_slot.get(); m.expire_source_backing_bucket_not_atomic(d, s) });
                }
            }
            5 => {
                let dt = 1 + r.n(3_000_000);
                let _ = w.tx(|m, _| { let now = m.header.current_slot.get() + dt; m.advance_resolved_slot_not_atomic(now) });
            }
            _ => {}
        }
    }
    for round in 0..40usize {
        let _ = w.tx(|m, _| { let now = m.header.current_slot.get() + 5_000_000; m.advance_resolved_slot_not_atomic(now) });
        for d in 0..2 {
            let _ = w.tx(|m, _| { let s = m.header.current_slot.get(); m.expire_source_backing_bucket_not_atomic(d, s) });
        }
        for j in 0..4usize {
            let who = (j + round) % 4;
            if closed[who] {
                continue;
            }
            if matches!(w.close(who, &mut paid, &mut closed, true), Ok(true)) {
                continue;
            }
            for _ in 0..6 {
                match w.close(who, &mut paid, &mut closed, false) {
                    Ok(true) => break,
                    Ok(false) => {}
                    Err(e) => {
                        last_err[who] = Some(e);
                        break;
                    }
                }
            }
            if !closed[who] {
                w.topup(who, &mut paid);
            }
        }
        if closed.iter().all(|c| *c) {
            break;
        }
    }
    let mk = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
    mk.validate_shape().unwrap();
    Some(S16acOut { paid, closed, vault: w.header.vault.get(), insurance: w.header.insurance.get(), deposits, last_err })
}

fn s16ac_walk_stats(steps: usize) -> (usize, Vec<u64>, Vec<u64>, u128) {
    let (mut built, mut stuck, mut leaky, mut left) = (0usize, vec![], vec![], 0u128);
    for seed in 0..6000u64 {
        let Some(o) = s16ac_walk(seed, steps) else { continue };
        built += 1;
        if o.paid.iter().sum::<u128>() + o.vault != o.deposits + o.insurance {
            leaky.push(seed);
        }
        if o.closed.iter().all(|c| *c) {
            left += o.vault - o.insurance;
        } else {
            stuck.push(seed);
        }
    }
    (built, stuck, leaky, left)
}


/// Per-seed dump for a base-vs-branch comparison: `LIVEFIX_WALK_DUMP=<file>`.
#[test]
#[ignore = "measurement harness; run with --ignored and LIVEFIX_WALK_DUMP set"]
fn livefix_walk_dump() {
    use std::io::Write;
    let path = std::env::var("LIVEFIX_WALK_DUMP").expect("LIVEFIX_WALK_DUMP");
    let mut f = std::fs::File::create(path).unwrap();
    for steps in [80usize, 0] {
        for seed in 0..6000u64 {
            if let Some(o) = s16ac_walk(seed, steps) {
                writeln!(
                    f,
                    "{steps} {seed} {} {} {} {} {} {} {} {} {} {}",
                    o.paid[0], o.paid[1], o.paid[2], o.paid[3],
                    o.closed.iter().all(|c| *c) as u8,
                    o.vault, o.insurance, o.deposits,
                    o.paid.iter().sum::<u128>() + o.vault == o.deposits + o.insurance,
                    o.last_err.iter().filter(|e| e.is_some()).count()
                )
                .unwrap();
            }
        }
    }
}
