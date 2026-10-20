//! v22kani LIVE (2026-10-20): VERBATIM replay of upstream aeyakovenko/percolator#230 (OPEN,
//! V12/Zellic) on the fork head (engine 1c96c22f). Only change: the fork's
//! `execute_trade_with_fee_loss_stale_scoped_not_atomic` takes a 4th `taker_is_long_account`
//! argument (fee routing only; fee_bps = 0). G9 relevance: it blocks liquidation of the losing side.
//! Proof of concept: liens on expired backing are reconciled only when the holder is flat.
//!
//! Save as `tests/poc_h01_unreconciled_impaired_liens.rs` in the percolator crate root, then run
//! `cargo test --test poc_h01_unreconciled_impaired_liens`.
#![allow(dead_code)]

use percolator::*;

const MARKET_GROUP: [u8; 32] = [1; 32];
const OWNER: [u8; 32] = [3; 32];

fn portfolio(seed: u8) -> PortfolioAccountV16Account {
    let provenance = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        MARKET_GROUP,
        [seed; 32],
        OWNER,
    ));
    let mut account = PortfolioAccountV16Account::default();
    account.init_empty_in_place(provenance).unwrap();
    account
}

fn two<T>(items: &mut [T], a: usize, b: usize) -> (&mut T, &mut T) {
    assert_ne!(a, b);
    if a < b {
        let (lo, hi) = items.split_at_mut(b);
        (&mut lo[a], &mut hi[0])
    } else {
        let (lo, hi) = items.split_at_mut(a);
        (&mut hi[0], &mut lo[b])
    }
}

/// Engine state plus portfolios. Every engine call goes through `exec`, which
/// restores the prior state when the call fails, as the SVM transaction
/// rollback that the engine relies on would.
#[derive(Clone)]
struct World {
    header: MarketGroupV16HeaderAccount,
    markets: Vec<Market<u64>>,
    accounts: Vec<PortfolioAccountV16Account>,
}

impl World {
    /// `asset_slots` assets activated at `price` under the stock public
    /// configuration `V16Config::public_user_fund_with_market_slots`.
    fn new(
        asset_slots: u32,
        price: u64,
        portfolios: usize,
        tweak: impl FnOnce(&mut V16Config),
    ) -> Self {
        let max_assets = asset_slots.min(V16_MAX_PORTFOLIO_ASSETS_N as u32) as u16;
        let mut config =
            V16Config::public_user_fund_with_market_slots(max_assets, asset_slots, 0, 10);
        tweak(&mut config);
        let mut header =
            MarketGroupV16HeaderAccount::new_dynamic(MARKET_GROUP, config, asset_slots, 0).unwrap();
        let mut markets: Vec<Market<u64>> = (0..asset_slots)
            .map(|i| Market::new(u64::from(i), EngineAssetSlotV16Account::default()))
            .collect();
        for (i, market) in markets.iter_mut().enumerate() {
            header
                .activate_empty_asset_slot_not_atomic(
                    i as u32,
                    &mut market.engine,
                    price,
                    i as u64 + 1,
                )
                .unwrap();
        }
        let accounts = (0..portfolios).map(|i| portfolio(10 + i as u8)).collect();
        Self {
            header,
            markets,
            accounts,
        }
    }

    fn exec<R>(
        &mut self,
        f: impl FnOnce(
            &mut MarketGroupV16ViewMut<'_, u64>,
            &mut [PortfolioAccountV16Account],
        ) -> V16Result<R>,
    ) -> V16Result<R> {
        let snapshot = self.clone();
        let out = {
            let mut market = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
            f(&mut market, &mut self.accounts)
        };
        if out.is_err() {
            *self = snapshot;
        }
        out
    }

    fn on<R>(
        &mut self,
        i: usize,
        f: impl FnOnce(&mut MarketGroupV16ViewMut<'_, u64>, &mut PortfolioV16ViewMut<'_>) -> V16Result<R>,
    ) -> V16Result<R> {
        self.exec(|market, accounts| f(market, &mut PortfolioV16ViewMut::new(&mut accounts[i])))
    }

    fn slot(&self) -> u64 {
        self.header.current_slot.get()
    }

    fn asset(&self, i: usize) -> AssetStateV16 {
        self.markets[i].engine.asset.try_to_runtime().unwrap()
    }

    /// Source domain `d` is (asset d / 2, Long) when d is even, else (asset d / 2, Short).
    fn bucket(&self, domain: usize) -> BackingBucketV16 {
        let slot = &self.markets[domain / 2].engine;
        if domain % 2 == 0 {
            slot.backing_long.try_to_runtime().unwrap()
        } else {
            slot.backing_short.try_to_runtime().unwrap()
        }
    }

    fn source_credit(&self, domain: usize) -> SourceCreditStateV16 {
        let slot = &self.markets[domain / 2].engine;
        if domain % 2 == 0 {
            slot.source_credit_long.try_to_runtime().unwrap()
        } else {
            slot.source_credit_short.try_to_runtime().unwrap()
        }
    }

    /// Vault value owed to no senior claimant, i.e. the junior pool.
    fn residual(&self) -> u128 {
        let h = &self.header;
        h.vault.get()
            - h.c_tot.get()
            - h.insurance.get()
            - h.backing_provider_earnings_total.get()
            - h.source_fresh_backing_total_num.get() / BOUND_SCALE
    }

    fn deposit(&mut self, i: usize, amount: u128) {
        self.on(i, |m, a| m.deposit_not_atomic(a, amount)).unwrap();
    }

    fn withdraw(&mut self, i: usize, amount: u128) -> V16Result<()> {
        self.on(i, |m, a| m.withdraw_not_atomic(a, amount))
    }

    /// `long` buys `size_q` base quantity of `asset` from `short` at `price`.
    fn trade_q(
        &mut self,
        long: usize,
        short: usize,
        asset: usize,
        size_q: i128,
        price: u64,
    ) -> V16Result<()> {
        self.exec(|market, accounts| {
            let (l, s) = two(accounts, long, short);
            let request = TradeRequestV16 {
                asset_index: asset,
                size_q,
                exec_price: price,
                fee_bps: 0,
            };
            market
                .execute_trade_with_fee_loss_stale_scoped_not_atomic(
                    &mut PortfolioV16ViewMut::new(l),
                    &mut PortfolioV16ViewMut::new(s),
                    request,
                    true, // fork arg: taker side, only routes the trade fee (fee_bps 0 here)
                )
                .map(|_| ())
        })
    }

    fn trade(
        &mut self,
        long: usize,
        short: usize,
        asset: usize,
        units: u128,
        price: u64,
    ) -> V16Result<()> {
        self.trade_q(long, short, asset, (units * POS_SCALE) as i128, price)
    }

    fn refresh(&mut self, i: usize) -> V16Result<HealthCertV16> {
        self.on(i, |m, a| m.full_account_refresh_not_atomic(a))
    }

    fn liquidate(&mut self, i: usize, asset: usize) -> V16Result<()> {
        self.on(i, |m, a| {
            m.liquidate_account_not_atomic(a, LiquidationRequestV16 { asset_index: asset })
                .map(|_| ())
        })
    }

    /// Owner-authorised unilateral reduction.
    fn reduce(&mut self, i: usize, asset: usize, units: u128) -> V16Result<()> {
        self.on(i, |m, a| {
            m.rebalance_reduce_position_not_atomic(
                a,
                RebalanceRequestV16 {
                    asset_index: asset,
                    reduce_q: units * POS_SCALE,
                },
            )
            .map(|_| ())
        })
    }

    fn convert(&mut self, i: usize) -> V16Result<u128> {
        self.on(i, |m, a| m.convert_released_pnl_to_capital_not_atomic(a))
    }

    fn close_resolved(&mut self, i: usize) -> V16Result<ResolvedCloseOutcomeV16> {
        self.on(i, |m, a| m.close_resolved_account_not_atomic(a, 0))
    }

    fn claim_topup(&mut self, i: usize) -> V16Result<u128> {
        self.on(i, |m, a| m.claim_resolved_payout_topup_not_atomic(a))
    }

    /// The single public permissionless crank, given one oracle observation.
    fn crank(
        &mut self,
        i: usize,
        now_slot: u64,
        asset: usize,
        price: u64,
    ) -> V16Result<AutoCrankResultV16> {
        self.on(i, |m, a| {
            let observations = [AutoCrankObservationV16 {
                asset_index: asset,
                effective_price: price,
                funding_rate_e9: 0,
            }];
            m.permissionless_auto_crank_not_atomic(
                a,
                AutoCrankWorkV16 {
                    now_slot,
                    observations: &observations,
                    resolved_close_fee_rate_per_slot: 0,
                },
            )
        })
    }

    /// Bring `asset` up to the current slot without moving its price.
    fn catch_up(&mut self, asset: usize) {
        let price = self.asset(asset).effective_price;
        while self.asset(asset).slot_last < self.slot() {
            let now = self.slot();
            self.exec(|m, _| {
                m.accrue_asset_to_not_atomic(asset, now, price, 0, true)
                    .map(|_| ())
            })
            .unwrap();
        }
    }

    /// Advance the clock to `to_slot`, one slot at a time, at `asset`'s current price.
    fn tick_to(&mut self, asset: usize, to_slot: u64) {
        self.catch_up(asset);
        let price = self.asset(asset).effective_price;
        while self.slot() < to_slot {
            let next = self.slot() + 1;
            self.exec(|m, _| {
                m.accrue_asset_to_not_atomic(asset, next, price, 0, true)
                    .map(|_| ())
            })
            .unwrap();
        }
    }

    /// Publish a new oracle target for `asset` and accrue one slot to it.
    fn move_price(&mut self, asset: usize, price: u64) {
        self.catch_up(asset);
        let next = self.slot() + 1;
        self.exec(|m, _| {
            m.set_asset_raw_oracle_target_not_atomic(asset, price)?;
            m.accrue_asset_to_not_atomic(asset, next, price, 0, true)
                .map(|_| ())
        })
        .unwrap();
    }

    /// What keepers do continuously: expire every lapsed Fresh backing bucket.
    fn expire_lapsed(&mut self) {
        let now = self.slot();
        for domain in 0..self.markets.len() * 2 {
            let bucket = self.bucket(domain);
            if bucket.status == BackingBucketStatusV16::Fresh && bucket.expiry_slot <= now {
                self.exec(|m, _| m.expire_source_backing_bucket_not_atomic(domain, now))
                    .unwrap();
            }
        }
    }

    fn resolve(&mut self) {
        let now = self.slot();
        self.exec(|m, _| m.resolve_market_not_atomic(now)).unwrap();
    }
}

/// W (0) goes long against L (1). After a 10% move, W liens the backing created
/// by L's loss to add one unit against M (2), and the public crank expires the
/// bucket. Portfolios 3 and 4 are new traders.
fn expired_bucket_with_open_lien_holder() -> World {
    let mut w = World::new(1, 100, 5, |_| {});
    w.deposit(0, 1_000);
    w.deposit(1, 1_000);
    w.trade(0, 1, 0, 10, 100).unwrap();

    // Price +10%. L's realised loss of 100 leaves its capital and becomes
    // counterparty backing on source domain 1 (asset 0, Short). W's gain is a
    // claim on the same domain.
    w.move_price(0, 110);
    w.refresh(1).unwrap();
    w.refresh(0).unwrap();
    assert_eq!(w.accounts[1].capital.get(), 900);
    assert_eq!(w.bucket(1).fresh_unliened_backing_num, 100 * BOUND_SCALE);

    // W uses its gain as initial margin for one more unit, which liens all 100.
    w.deposit(0, 110);
    w.deposit(2, 110);
    w.trade(0, 2, 0, 1, 110).unwrap();
    assert_eq!(w.bucket(1).valid_liened_backing_num, 100 * BOUND_SCALE);

    // The bucket's lifetime was fixed when it was first funded. When it lapses
    // the crank expires it and writes the liened backing off as impaired.
    let expiry = w.bucket(1).expiry_slot;
    w.tick_to(0, expiry);
    let crank = w.crank(0, expiry, 0, 110).unwrap();
    assert_eq!(
        crank.outcome,
        AutoCrankOutcomeV16::Progressed(PermissionlessProgressOutcomeV16::SourceBackingExpired {
            domain: 1
        })
    );
    assert_eq!(w.bucket(1).status, BackingBucketStatusV16::Impaired);
    assert_eq!(w.bucket(1).valid_liened_backing_num, 0);
    assert_eq!(w.source_credit(1).fresh_reserved_backing_num, 0);

    // W's account-local lien is untouched.
    assert_eq!(
        w.accounts[0].source_domains[0].source_lien_effective_reserved.get(),
        100
    );
    w
}

#[test]
#[ignore = "upstream #230 expectation verbatim; the fork diverges (LossStale), see g9_230_fork_losing_side"]
fn losing_side_cannot_be_settled_or_liquidated() {
    let mut w = expired_bucket_with_open_lien_holder();

    // Price +10% again. Every short now has a loss that must be reserved on
    // domain 1, and every settlement path fails.
    w.move_price(0, 121);
    let now = w.slot();
    for short in [1, 2] {
        assert_eq!(w.refresh(short), Err(V16Error::LockActive));
        assert_eq!(w.liquidate(short, 0), Err(V16Error::LockActive));
        assert_eq!(
            w.crank(short, now, 0, 121).map(|_| ()),
            Err(V16Error::LockActive)
        );
    }
    // The long side settles normally.
    w.refresh(0).unwrap();

    // Nobody can open new risk on asset 0 while the shorts' K/F cohort is unsettled.
    w.deposit(3, 1_000);
    w.deposit(4, 1_000);
    assert_eq!(w.trade(3, 4, 0, 1, 121), Err(V16Error::LockActive));

    // The price keeps rising. L's loss now exceeds its capital and L still
    // cannot be liquidated.
    w.move_price(0, 210);
    assert!(10 * (210 - 110) > w.accounts[1].capital.get());
    assert_eq!(w.liquidate(1, 0), Err(V16Error::LockActive));

    // Once W is completely flat the crank retires the impaired lien, after
    // which L settles again, now carrying bad debt.
    w.reduce(0, 0, 11).unwrap();
    let now = w.slot();
    let release = w.crank(0, now, 0, 210).unwrap();
    assert_eq!(release.selected, AutoCrankPlanV16::ReleaseSourceLiens);
    assert_eq!(w.bucket(1).status, BackingBucketStatusV16::Expired);
    w.refresh(1).unwrap();
    assert_eq!(w.accounts[1].capital.get(), 0);
    assert_eq!(w.accounts[1].pnl.get(), -100);
}

#[test]
#[ignore = "upstream #230 expectation verbatim; the fork certifies 1_110 (no overstatement), see g9_230_fork_lien_holder"]
fn lien_holder_overstates_equity_and_then_cannot_be_settled() {
    let mut w = expired_bucket_with_open_lien_holder();

    // W's certificate still counts the written-off 100. Real equity is the
    // 1,110 of capital against a 1,210 maintenance requirement.
    let cert = w.refresh(0).unwrap();
    assert_eq!(w.accounts[0].capital.get(), 1_110);
    assert_eq!(cert.certified_maintenance_req, 1_210);
    assert_eq!(cert.certified_equity, 1_210);
    assert_eq!(cert.certified_liq_deficit, 0);
    assert_eq!(w.liquidate(0, 0), Err(V16Error::NonProgress));

    // Price -10%. The loss burns W's liened claim face, and returning the
    // lien's backing to the impaired bucket underflows.
    w.move_price(0, 100);
    let now = w.slot();
    assert_eq!(w.refresh(0), Err(V16Error::CounterUnderflow));
    assert_eq!(w.liquidate(0, 0), Err(V16Error::CounterUnderflow));
    assert_eq!(w.reduce(0, 0, 11), Err(V16Error::CounterUnderflow));
    assert_eq!(
        w.crank(0, now, 0, 100).map(|_| ()),
        Err(V16Error::CounterUnderflow)
    );
    // More collateral does not help.
    w.deposit(0, 10_000);
    assert_eq!(w.refresh(0), Err(V16Error::CounterUnderflow));

    // Neither does forcing asset 0 into Recovery: the dead-leg forfeit and the
    // Recovery pair close fail the same way.
    let mut forced = w.clone();
    let now = forced.slot();
    forced
        .exec(|m, _| m.force_asset_recovery_not_atomic(0, now))
        .unwrap();
    assert_eq!(
        forced.on(0, |m, a| m
            .forfeit_recovery_leg_not_atomic(a, 0, MAX_VAULT_TVL)
            .map(|_| ())),
        Err(V16Error::CounterUnderflow)
    );
    assert_eq!(
        forced.exec(|m, accounts| {
            let (long, short) = two(accounts, 0, 1);
            m.force_close_recovery_pair_not_atomic(
                &mut PortfolioV16ViewMut::new(long),
                &mut PortfolioV16ViewMut::new(short),
                0,
                u128::MAX,
            )
            .map(|_| ())
        }),
        Err(V16Error::CounterUnderflow)
    );

    // The shorts' gains sit on domain 0, which W's unsettled loss never funds,
    // and nobody can open new risk on asset 0.
    w.refresh(1).unwrap();
    w.refresh(2).unwrap();
    assert_eq!(w.source_credit(0).credit_rate_num, 0);
    w.deposit(3, 1_000);
    w.deposit(4, 1_000);
    assert_eq!(w.trade(3, 4, 0, 1, 100), Err(V16Error::LockActive));

    // Only the resolved close path reconciles the account-local lien.
    w.resolve();
    assert_eq!(w.close_resolved(0), Ok(ResolvedCloseOutcomeV16::ProgressOnly));
    assert!(w.close_resolved(0).is_ok());
}


// ---------------------------------------------------------------------------------------------
// Fork characterisation (v22kani LIVE): the same world on engine 1c96c22f. The question for G9 is
// whether the losing short L (account 1) can be liquidated while W (account 0) holds its
// account-local lien on the expired (Impaired) bucket.
// ---------------------------------------------------------------------------------------------

#[test]
fn g9_230_fork_losing_side() {
    let mut w = expired_bucket_with_open_lien_holder();
    w.move_price(0, 121);
    let now = w.slot();
    for short in [1, 2] {
        let r = w.refresh(short).map(|_| ());
        let l = w.liquidate(short, 0);
        let c = w.crank(short, now, 0, 121).map(|x| x.selected);
        eprintln!("230@fork p=121 short={short}: refresh={r:?} liquidate={l:?} crank={c:?}");
    }
    eprintln!("230@fork p=121 long W refresh={:?}", w.refresh(0).map(|c| (c.certified_equity, c.certified_maintenance_req)));
    w.move_price(0, 210);
    let now = w.slot();
    let cap1 = w.accounts[1].capital.get();
    let l = w.liquidate(1, 0);
    let c = w.crank(1, now, 0, 210).map(|x| (x.selected, x.outcome));
    eprintln!("230@fork p=210 L: capital={cap1} liquidate={l:?} crank={c:?}");
    let r = w.refresh(1).map(|c| (c.certified_equity, c.certified_maintenance_req, c.certified_liq_deficit));
    eprintln!("230@fork p=210 L refresh={r:?}");
    let l2 = w.liquidate(1, 0);
    eprintln!("230@fork p=210 L liquidate after refresh={l2:?}; legs={}", w.accounts[1].legs.iter().filter(|x| x.active != 0).count());
}

#[test]
fn g9_230_fork_lien_holder() {
    let mut w = expired_bucket_with_open_lien_holder();
    let mut w2 = w.clone();
    let lw = w2.liquidate(0, 0);
    let now = w2.slot();
    let cw = w2.crank(0, now, 0, 110).map(|x| (x.selected, x.outcome));
    eprintln!("230@fork W below maintenance by 100 after the expiry: liquidate={lw:?} then crank={cw:?} legs={}", w2.accounts[0].legs.iter().filter(|x| x.active != 0).count());
    let cert = w.refresh(0);
    eprintln!("230@fork W cert={:?} capital={}", cert.as_ref().map(|c| (c.certified_equity, c.certified_maintenance_req, c.certified_liq_deficit)), w.accounts[0].capital.get());
    w.move_price(0, 100);
    let r = w.refresh(0).map(|c| (c.certified_equity, c.certified_maintenance_req, c.certified_liq_deficit));
    let l = w.liquidate(0, 0);
    eprintln!("230@fork p=100 W refresh={r:?} liquidate={l:?}");
}

/// F-LIVE-2 RED replay: a below-maintenance (later bankrupt) account on the losing side must be
/// liquidatable. On the head every liquidation of it returns LockActive while the lien holder W
/// keeps a position (Impaired backing bucket, upstream #230).
#[test]
#[ignore = "RED on head: F-LIVE-2 replay (upstream #230, Impaired bucket blocks liquidation)"]
fn g9_f_live_2_red_bankrupt_losing_side_account_is_liquidatable() {
    let mut w = expired_bucket_with_open_lien_holder();
    w.move_price(0, 210);
    assert!(10 * (210 - 110) > w.accounts[1].capital.get(), "precondition: L's loss exceeds its capital");
    let r = w.liquidate(1, 0);
    assert!(r.is_ok(), "bankrupt losing-side account not liquidatable: {r:?}");
}
