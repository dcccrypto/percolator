//! v22-risk-1113 leniency scoreboard (tests only; no engine change).
//!
//! Headline metric for the v2.2 risk-engine leniency work: across deterministic honest-trader
//! scenarios, the atoms an honest (solvent, non-attacking) trader is credited or paid LESS than
//! Toly's spec entitles it to, and the liquidations that succeed although the spec says the
//! account is healthy. Each scenario compares against either
//!   * a control twin world that differs only in something the spec says must not affect the
//!     honest trader (another trader's choice to pledge, keeper/refresh order, refresh cadence,
//!     close order), or
//!   * the spec's own formula (fork spec.md §4.5 fees, §7 maintenance/liquidation, §8.8 resolved
//!     payout), computed independently in the test.
//! Rounding the spec itself mandates is not counted (each scenario states its allowance).
//!
//! `leniency_scoreboard` prints one `LENIENCY <scenario>: <atoms>` line per scenario and a total
//!   LENIENCY_SCORE honest_atoms_lost=<n> avoidable_liquidations=<m>
//! and asserts the score never rises above the recorded ceiling (a ratchet: a fix lowers the
//! ceiling in the same PR; a regression fails CI).
//!
//!   cargo test --test v22_leniency_scoreboard leniency_scoreboard -- --nocapture | grep LENIENCY
#![allow(dead_code)]

use percolator::*;

const MARKET_GROUP: [u8; 32] = [1; 32];
const OWNER: [u8; 32] = [3; 32];

const W1: usize = 0;
const W2: usize = 1;
const L: usize = 2;
const M: usize = 3;
const N: usize = 4;

/// Source domain of long profits on asset 0: (asset 0, Short).
const GAIN_DOMAIN: usize = 1;
/// Domain a long's loss on asset 0 is booked into: (asset 0, Long).
const LOSS_DOMAIN: usize = 0;

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

/// Engine state plus portfolios. Every engine call goes through `exec`, which restores the
/// prior state when the call fails, as the SVM transaction rollback the engine relies on
/// would.
#[derive(Clone)]
struct World {
    header: MarketGroupV16HeaderAccount,
    markets: Vec<Market<u64>>,
    accounts: Vec<PortfolioAccountV16Account>,
}

impl World {
    /// One asset activated at `price` under the stock public configuration
    /// `V16Config::public_user_fund_with_market_slots` (100% initial and maintenance margin).
    fn new(price: u64, portfolios: usize) -> Self {
        Self::with_config(
            V16Config::public_user_fund_with_market_slots(1, 1, 0, 10),
            price,
            portfolios,
        )
    }

    fn with_config(config: V16Config, price: u64, portfolios: usize) -> Self {
        Self::with_assets(config, price, 1, portfolios)
    }

    fn with_assets(config: V16Config, price: u64, assets: usize, portfolios: usize) -> Self {
        let mut header =
            MarketGroupV16HeaderAccount::new_dynamic(MARKET_GROUP, config, assets as u32, 0)
                .unwrap();
        let mut markets: Vec<Market<u64>> = (0..assets)
            .map(|i| Market::new(i as u64, EngineAssetSlotV16Account::default()))
            .collect();
        for (i, m) in markets.iter_mut().enumerate() {
            header
                .activate_empty_asset_slot_not_atomic(i as u32, &mut m.engine, price, i as u64 + 1)
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
        f: impl FnOnce(
            &mut MarketGroupV16ViewMut<'_, u64>,
            &mut PortfolioV16ViewMut<'_>,
        ) -> V16Result<R>,
    ) -> V16Result<R> {
        self.exec(|market, accounts| f(market, &mut PortfolioV16ViewMut::new(&mut accounts[i])))
    }

    fn slot(&self) -> u64 {
        self.header.current_slot.get()
    }

    fn asset(&self) -> AssetStateV16 {
        self.markets[0].engine.asset.try_to_runtime().unwrap()
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

    /// Decoding validates the stored rate against the engine's own formula.
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

    fn capital(&self, i: usize) -> u128 {
        self.accounts[i].capital.get()
    }

    fn pnl(&self, i: usize) -> i128 {
        self.accounts[i].pnl.get()
    }

    fn deposit(&mut self, i: usize, amount: u128) {
        self.on(i, |m, a| m.deposit_not_atomic(a, amount)).unwrap();
    }

    /// `long` buys `units` of asset 0 from `short` at `price`.
    fn trade(&mut self, long: usize, short: usize, units: u128, price: u64) -> V16Result<()> {
        self.trade_q(long, short, units * POS_SCALE, price)
    }

    /// `long` buys `size_q` base quantity (POS_SCALE per unit) from `short` at `price`.
    fn trade_q(&mut self, long: usize, short: usize, size_q: u128, price: u64) -> V16Result<()> {
        self.exec(|market, accounts| {
            let (l, s) = two(accounts, long, short);
            let request = TradeRequestV16 {
                asset_index: 0,
                size_q: size_q as i128,
                exec_price: price,
                fee_bps: 0,
            };
            market
                .execute_trade_with_fee_loss_stale_scoped_not_atomic(
                    &mut PortfolioV16ViewMut::new(l),
                    &mut PortfolioV16ViewMut::new(s),
                    request,
                    true,
                )
                .map(|_| ())
        })
    }

    fn refresh(&mut self, i: usize) -> V16Result<HealthCertV16> {
        self.on(i, |m, a| m.full_account_refresh_not_atomic(a))
    }

    fn liquidate(&mut self, i: usize) -> V16Result<()> {
        self.on(i, |m, a| {
            m.liquidate_account_not_atomic(a, LiquidationRequestV16 { asset_index: 0 })
                .map(|_| ())
        })
    }

    fn convert(&mut self, i: usize) -> V16Result<u128> {
        self.on(i, |m, a| m.convert_released_pnl_to_capital_not_atomic(a))
    }

    fn close_resolved(&mut self, i: usize) -> V16Result<ResolvedCloseOutcomeV16> {
        self.on(i, |m, a| m.close_resolved_account_not_atomic(a, 0))
    }

    /// Close a resolved account, calling the bounded continuation until it reports `Closed`.
    fn close_resolved_fully(&mut self, i: usize) -> u128 {
        for _ in 0..8 {
            if let ResolvedCloseOutcomeV16::Closed { payout } = self.close_resolved(i).unwrap() {
                return payout;
            }
        }
        panic!("account {i} did not close");
    }

    /// The single public permissionless crank, given one oracle observation.
    fn crank(&mut self, i: usize) -> V16Result<AutoCrankResultV16> {
        let now_slot = self.slot();
        let price = self.asset().effective_price;
        self.on(i, |m, a| {
            let observations = [AutoCrankObservationV16 {
                asset_index: 0,
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

    /// Bring asset 0 up to the current slot without moving its price.
    fn catch_up(&mut self) {
        let price = self.asset().effective_price;
        while self.asset().slot_last < self.slot() {
            let now = self.slot();
            self.exec(|m, _| {
                m.accrue_asset_to_not_atomic(0, now, price, 0, true)
                    .map(|_| ())
            })
            .unwrap();
        }
    }

    /// Advance the clock to `to_slot`, one slot at a time, at the current price.
    fn tick_to(&mut self, to_slot: u64) {
        self.catch_up();
        let price = self.asset().effective_price;
        while self.slot() < to_slot {
            let next = self.slot() + 1;
            self.exec(|m, _| {
                m.accrue_asset_to_not_atomic(0, next, price, 0, true)
                    .map(|_| ())
            })
            .unwrap();
        }
    }

    /// Publish a new oracle target for asset 0 and accrue one slot to it.
    fn move_price(&mut self, price: u64) {
        self.catch_up();
        let next = self.slot() + 1;
        self.exec(|m, _| {
            m.set_asset_raw_oracle_target_not_atomic(0, price)?;
            m.accrue_asset_to_not_atomic(0, next, price, 0, true)
                .map(|_| ())
        })
        .unwrap();
    }

    fn resolve(&mut self) {
        let now = self.slot();
        self.exec(|m, _| m.resolve_market_not_atomic(now)).unwrap();
    }
}

/// Independent readings and checks, computed from account and domain state only.
impl World {
    /// (claims, fresh backing, liened backing) of a domain in whole atoms.
    fn domain_atoms(&self, domain: usize) -> (u128, u128, u128) {
        let s = self.source_credit(domain);
        for num in [
            s.positive_claim_bound_num,
            s.fresh_reserved_backing_num,
            s.valid_liened_backing_num,
        ] {
            assert_eq!(num % BOUND_SCALE, 0, "scenario keeps whole atoms");
        }
        (
            s.positive_claim_bound_num / BOUND_SCALE,
            s.fresh_reserved_backing_num / BOUND_SCALE,
            s.valid_liened_backing_num / BOUND_SCALE,
        )
    }

    fn rate(&self, domain: usize) -> u128 {
        self.source_credit(domain).credit_rate_num
    }

    /// Backing a domain can still credit to unliened face, BOUND_SCALE units.
    fn available_num(&self, domain: usize) -> u128 {
        let s = self.source_credit(domain);
        (s.fresh_reserved_backing_num - s.valid_liened_backing_num)
            + (s.insurance_credit_reserved_num
                - s.valid_liened_insurance_num
                - s.impaired_liened_insurance_num)
    }

    fn source_slot(&self, i: usize, domain: usize) -> Option<PortfolioSourceDomainV16Account> {
        self.accounts[i]
            .source_domains
            .iter()
            .copied()
            .find(|s| s.is_occupied() && s.domain.get() as usize == domain)
    }

    /// (locked face, effective reserve in atoms, pledged counterparty backing) of `i`'s lien.
    fn lien(&self, i: usize, domain: usize) -> (u128, u128, u128) {
        self.source_slot(i, domain).map_or((0, 0, 0), |s| {
            (
                s.source_claim_liened_num.get(),
                s.source_lien_effective_reserved.get(),
                s.source_lien_counterparty_backing_num.get(),
            )
        })
    }

    /// Whole atoms account `i` can be credited from `domain` right now: its valid lien
    /// reserve, plus its unliened face at the domain's stored rate, never more than the
    /// domain's unliened backing.
    fn creditable(&self, i: usize, domain: usize) -> u128 {
        let Some(s) = self.source_slot(i, domain) else {
            return 0;
        };
        let bucket = self.bucket(domain);
        let bucket_live =
            bucket.status == BackingBucketStatusV16::Fresh && bucket.expiry_slot > self.slot();
        let lien = if bucket_live {
            s.source_lien_effective_reserved.get()
        } else {
            s.source_lien_effective_reserved.get()
                - s.source_lien_counterparty_backing_num.get() / BOUND_SCALE
        };
        let unliened = s.source_claim_bound_num.get()
            - s.source_claim_liened_num.get()
            - s.source_claim_impaired_num.get();
        let soft = (unliened * self.rate(domain) / CREDIT_RATE_SCALE / BOUND_SCALE)
            .min(self.available_num(domain) / BOUND_SCALE);
        lien + soft
    }

    /// Property (a): everything the domain's claimants can be credited, liens included, is
    /// covered by the domain's fresh backing plus its unimpaired insurance credit.
    fn assert_domain_backs_all_credit(&self, domain: usize) {
        let s = self.source_credit(domain);
        let backing = (s.fresh_reserved_backing_num + s.insurance_credit_reserved_num
            - s.impaired_liened_insurance_num)
            / BOUND_SCALE;
        let credited: u128 = (0..self.accounts.len())
            .map(|i| self.creditable(i, domain))
            .sum();
        assert!(
            credited <= backing,
            "domain {domain} credits {credited} against {backing} of backing"
        );
    }

    /// Vault conservation over the senior stack, and both domains never over-credited.
    fn assert_sound(&self) {
        let h = &self.header;
        let senior = h.c_tot.get()
            + h.insurance.get()
            + h.backing_provider_earnings_total.get()
            + h.source_fresh_backing_total_num.get() / BOUND_SCALE;
        assert!(h.vault.get() >= senior, "vault below senior stack");
        let capital: u128 = self.accounts.iter().map(|a| a.capital.get()).sum();
        assert_eq!(h.c_tot.get(), capital, "c_tot drifted from account capital");
        for domain in [LOSS_DOMAIN, GAIN_DOMAIN] {
            let b = self.bucket(domain);
            let s = self.source_credit(domain);
            assert_eq!(
                s.fresh_reserved_backing_num,
                b.fresh_unliened_backing_num + b.valid_liened_backing_num
            );
            // The domain's liened backing is exactly what its claimants pledged, and no
            // claimant pledges more backing than the face it locked.
            let mut pledged = 0u128;
            for i in 0..self.accounts.len() {
                let (face, _, backing) = self.lien(i, domain);
                assert!(backing <= face, "lien pledges more backing than face");
                pledged += backing;
            }
            if b.status == BackingBucketStatusV16::Fresh {
                assert_eq!(s.valid_liened_backing_num, pledged);
            }
            self.assert_domain_backs_all_credit(domain);
        }
    }
}

/// The rate the header rule gives for whole-atom domain totals (no insurance credit).
fn expected_rate(claims: u128, fresh: u128, liened: u128) -> u128 {
    assert!(liened <= fresh && liened <= claims);
    if claims == 0 {
        return CREDIT_RATE_SCALE;
    }
    if fresh == liened {
        return 0;
    }
    if claims == liened {
        return CREDIT_RATE_SCALE;
    }
    ((fresh - liened) * CREDIT_RATE_SCALE / (claims - liened)).min(CREDIT_RATE_SCALE)
}

/// W1 and W2 are each long 10 against L from 100. After +10%, L's realised loss of 200 has
/// left its capital and is the fresh backing of `GAIN_DOMAIN`; each winner's gain of 100 is a
/// claim on it. The domain is exactly fully backed: 200 of backing for 200 of claims.
fn two_winners(w1_capital: u128) -> World {
    let mut w = World::new(100, 5);
    w.deposit(W1, w1_capital);
    w.deposit(W2, 1_000);
    w.deposit(L, 4_000);
    w.trade(W1, L, 10, 100).unwrap();
    w.trade(W2, L, 10, 100).unwrap();
    w.move_price(110);
    w.refresh(L).unwrap();
    w.refresh(W1).unwrap();
    w.refresh(W2).unwrap();
    assert_eq!(w.capital(L), 4_000 - 200);
    assert_eq!((w.pnl(W1), w.pnl(W2)), (100, 100));
    assert_eq!(w.domain_atoms(GAIN_DOMAIN), (200, 200, 0));
    assert_eq!(w.rate(GAIN_DOMAIN), CREDIT_RATE_SCALE);
    w.assert_sound();
    w
}

/// W2 adds one unit at 110 against M, so its initial margin becomes 11 * 110 = 1_210.
///
/// With `pledge`, W2 tops its capital up to 1_135 only, and the engine covers the remaining
/// 75 with a lien on 75 of W2's gain: 75 of the domain's backing becomes liened and 75 of
/// W2's face is locked. Without it (the control world) W2 tops up to the full 1_210 and no
/// lien is created. Positions and prices are identical in both worlds.
fn w2_adds_one_unit(w: &mut World, pledge: bool) {
    w.deposit(W2, if pledge { 135 } else { 210 });
    w.deposit(M, 110);
    w.trade(W2, M, 1, 110).unwrap();
    let expected_lien = if pledge {
        (75 * BOUND_SCALE, 75, 75 * BOUND_SCALE)
    } else {
        (0, 0, 0)
    };
    assert_eq!(w.lien(W2, GAIN_DOMAIN), expected_lien);
    assert_eq!(
        w.domain_atoms(GAIN_DOMAIN),
        (200, 200, if pledge { 75 } else { 0 })
    );
}

fn lien_and_control_worlds(w1_capital: u128) -> (World, World) {
    let mut pledged = two_winners(w1_capital);
    w2_adds_one_unit(&mut pledged, true);
    let mut control = two_winners(w1_capital);
    w2_adds_one_unit(&mut control, false);
    (pledged, control)
}

// ---------------------------------------------------------------------------------------------
// Widened scenario set (v2): leveraged configuration, fees, funding, ADL, resolution, cranks.
// ---------------------------------------------------------------------------------------------

/// 10% initial and maintenance margin, 1% liquidation fee (min 2 atoms; minimum nonzero MM 100), trading fee up to 1%,
/// funding enabled. Everything else is the stock public configuration.
fn lev_config() -> V16Config {
    let mut c = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    c.maintenance_margin_bps = 1_000;
    c.initial_margin_bps = 1_000;
    c.max_trading_fee_bps = 100;
    c.liquidation_fee_bps = 100;
    c.liquidation_fee_cap = 1_000_000;
    c.min_liquidation_abs = 2;
    c.min_nonzero_mm_req = 100;
    c.min_nonzero_im_req = 101;
    c.max_price_move_bps_per_slot = 500;
    c.max_accrual_dt_slots = 1;
    c.max_abs_funding_e9_per_slot = FUNDING_E9;
    // Backing stays fresh for longer than any scenario runs, so no scenario is measuring the
    // (separate, spec'd) lapse of stale counterparty backing.
    c.min_funding_lifetime_slots = LIFETIME;
    c
}

const LIFETIME: u64 = 1_000;

/// Largest funding rate the validator admits (10_000) minus a prime offset, so per-slot
/// funding is never a whole number of atoms.
const FUNDING_E9: u64 = 9_973;

impl World {
    fn lev(price: u64, portfolios: usize) -> Self {
        Self::with_config(lev_config(), price, portfolios)
    }

    /// Walk the oracle to `target` in steps of at most 4% (the config admits 5% per slot).
    fn walk_price(&mut self, target: u64) {
        loop {
            let p = self.asset().effective_price;
            if p == target {
                return;
            }
            let step = (p / 25).max(1);
            let next = if target > p {
                (p + step).min(target)
            } else {
                p.saturating_sub(step).max(target)
            };
            self.move_price(next);
        }
    }

    fn trade_fee(
        &mut self,
        long: usize,
        short: usize,
        size_q: u128,
        price: u64,
        fee_bps: u64,
    ) -> V16Result<()> {
        self.exec(|market, accounts| {
            let (l, s) = two(accounts, long, short);
            let request = TradeRequestV16 {
                asset_index: 0,
                size_q: size_q as i128,
                exec_price: price,
                fee_bps,
            };
            market
                .execute_trade_with_fee_loss_stale_scoped_not_atomic(
                    &mut PortfolioV16ViewMut::new(l),
                    &mut PortfolioV16ViewMut::new(s),
                    request,
                    true,
                )
                .map(|_| ())
        })
    }

    fn liquidate_outcome(&mut self, i: usize) -> V16Result<LiquidationOutcomeV16> {
        self.on(i, |m, a| {
            m.liquidate_account_not_atomic(a, LiquidationRequestV16 { asset_index: 0 })
        })
    }

    /// Accrue one slot at the current price with `funding_rate_e9`.
    fn funding_slot(&mut self, funding_rate_e9: i128) {
        self.catch_up();
        let price = self.asset().effective_price;
        let next = self.slot() + 1;
        self.exec(|m, _| {
            m.accrue_asset_to_not_atomic(0, next, price, funding_rate_e9, true)
                .map(|_| ())
        })
        .unwrap();
    }

    /// The permissionless crank for account `i`, observing `now_slot` and funding.
    fn crank_at(&mut self, i: usize, now_slot: u64, funding_rate_e9: i128) -> V16Result<()> {
        let price = self.asset().effective_price;
        self.on(i, |m, a| {
            let observations = [AutoCrankObservationV16 {
                asset_index: 0,
                effective_price: price,
                funding_rate_e9,
            }];
            m.permissionless_auto_crank_not_atomic(
                a,
                AutoCrankWorkV16 {
                    now_slot,
                    observations: &observations,
                    resolved_close_fee_rate_per_slot: 0,
                },
            )
            .map(|_| ())
        })
    }

    /// One keeper slot: accrue one slot with `funding_rate_e9`, then for each account in
    /// `order` run the permissionless crank (which also expires lapsed backing) and a full
    /// refresh. Returns the number of refreshes the engine refused.
    fn keeper_slot(&mut self, order: &[usize], funding_rate_e9: i128) -> u32 {
        self.funding_slot(funding_rate_e9);
        let now = self.slot();
        let mut refused = 0;
        for &i in order {
            let _ = self.crank_at(i, now, funding_rate_e9);
            if self.refresh(i).is_err() {
                refused += 1;
            }
        }
        refused
    }

    /// Capital plus PnL of `i`: its value net of everything already booked to it.
    fn value(&self, i: usize) -> i128 {
        self.capital(i) as i128 + self.pnl(i)
    }

    fn total_deposits_and_insurance(&self) -> u128 {
        self.header.vault.get()
    }

    /// Close one account of an already resolved market, following progress-only results
    /// (bounded); `None` if it never reaches `Closed`.
    fn resolve_and_close_one(&mut self, i: usize) -> Option<u128> {
        for _ in 0..16 {
            match self.close_resolved(i) {
                Ok(ResolvedCloseOutcomeV16::Closed { payout }) => return Some(payout),
                Ok(_) => continue,
                Err(_) => return None,
            }
        }
        None
    }

    /// Resolve, then close every account in `order` (repeating the order a few times, as a
    /// keeper would, for close paths that report progress only). Returns the payout of each
    /// account, `None` for an account that never closed.
    fn resolve_and_close(&mut self, order: &[usize]) -> Vec<Option<u128>> {
        self.resolve();
        let mut out = vec![None; self.accounts.len()];
        for _ in 0..6 {
            for &i in order {
                if out[i].is_some() {
                    continue;
                }
                for _ in 0..8 {
                    match self.close_resolved(i) {
                        Ok(ResolvedCloseOutcomeV16::Closed { payout }) => {
                            out[i] = Some(payout);
                            break;
                        }
                        Ok(_) => continue,
                        Err(_) => break,
                    }
                }
            }
        }
        out
    }
}

/// Scoreboard accumulator. `lost` entries are atoms (clamped at 0 when totalled).
#[derive(Default)]
struct Score {
    lost: Vec<(String, i128)>,
    avoidable_liquidations: u32,
}

impl Score {
    fn lose(&mut self, name: &str, atoms: i128) {
        self.lost.push((name.to_string(), atoms));
    }
    fn avoidable(&mut self, name: &str) {
        self.avoidable_liquidations += 1;
        self.lost
            .push((format!("{name} [AVOIDABLE LIQUIDATION]"), 0));
    }
}

/// Spec §4.5 liquidation fee for closing `q_close_q` at `oracle_price` under `lev_config`.
fn spec_liq_fee(q_close_q: u128, oracle_price: u64) -> u128 {
    let c = lev_config();
    if q_close_q == 0 {
        return 0;
    }
    let closed_notional = q_close_q * oracle_price as u128 / POS_SCALE;
    let raw = (closed_notional * c.liquidation_fee_bps as u128).div_ceil(10_000);
    raw.max(c.min_liquidation_abs).min(c.liquidation_fee_cap)
}

/// S1-S5 (v1): another claimant's lien on a fully backed gain domain (spec §2.2, credit rate
/// 1.0 for a fully backed domain). Pledged world vs lien-free control twin.
fn lien_rate_scenarios(score: &mut Score) {
    // S1: W1's margin equity beside W2's lien, and whether W1 becomes liquidatable.
    {
        let (mut w, mut control) = lien_and_control_worlds(1_030);
        // Control calls must succeed; a refused pledged-world call is the trader's full loss.
        let ce = control.refresh(W1).unwrap().certified_equity;
        let e = w.refresh(W1).map(|c| c.certified_equity).unwrap_or(0);
        score.lose("S1 W1 margin equity", ce - e);
        let e2 = w.refresh(W2).map(|c| c.certified_equity).unwrap_or(0);
        score.lose("S1 W2 own margin equity", (1_135 + 100) - e2);
        if w.liquidate(W1).is_ok() && control.liquidate(W1).is_err() {
            score.avoidable("S1 W1 beside lien");
        }
    }
    // S2: flat W1 converts its fully backed gain beside the live lien.
    {
        let (mut w, mut control) = lien_and_control_worlds(1_030);
        for world in [&mut w, &mut control] {
            world.deposit(N, 5_000);
            world.trade(N, W1, 10, 110).unwrap();
        }
        let c = control.convert(W1).unwrap() as i128;
        assert!(c > 0, "S2 control conversion must pay out");
        let p = w.convert(W1).unwrap_or(0) as i128;
        score.lose("S2 W1 conversion", c - p);
    }
    // S3: resolved close of W1 while flat W2's lien is still pledged.
    {
        let (mut w, mut control) = lien_and_control_worlds(1_030);
        for world in [&mut w, &mut control] {
            world.deposit(N, 5_000);
            world.trade(N, W1, 10, 110).unwrap();
            world.trade(N, W2, 11, 110).unwrap();
            world.resolve();
            for i in [L, M, N] {
                world.close_resolved_fully(i);
            }
        }
        let c = control.resolve_and_close_one(W1).expect("S3 control close") as i128;
        let p = w.resolve_and_close_one(W1).unwrap_or(0) as i128;
        score.lose("S3 W1 resolved close payout", c - p);
    }
    // S4: a loss on the lien holder must not cut W1's remaining backed gain.
    {
        let (mut w, mut control) = lien_and_control_worlds(1_030);
        let mut eq = [0i128; 2];
        for (world, e) in [&mut w, &mut control].into_iter().zip(&mut eq) {
            world.move_price(105);
            for i in [W1, W2, L, M] {
                let _ = world.refresh(i);
            }
            *e = world.refresh(W1).map(|c| c.certified_equity).unwrap_or(0);
        }
        assert!(eq[1] > 0, "S4 control refresh must succeed");
        score.lose("S4 W1 equity after lien-holder loss", eq[1] - eq[0]);
    }
    // S5: W1's creditable gain in the pledged world vs control.
    {
        let (w, control) = lien_and_control_worlds(1_030);
        let c = control.creditable(W1, GAIN_DOMAIN) as i128;
        let p = w.creditable(W1, GAIN_DOMAIN) as i128;
        score.lose("S5 W1 creditable gain", c - p);
    }
}

/// S6: liquidation near the maintenance threshold (spec §7: healthy iff Eq_net >= MM_req;
/// §4.5 liquidation fee on the closed notional only).
///
/// W1 is long 100 at 100 against L; the oracle moves to 95, so MM_req = ceil(9_500 * 10%) =
/// 950 and W1's equity is `capital - 500`. With capital 1_450 W1 sits exactly at maintenance
/// and must not be liquidatable. With 1_449 it is one atom short; liquidation may close it,
/// but W1 must keep its equity minus the spec fee on what was closed.
fn near_threshold_liquidation(score: &mut Score) {
    for capital in [1_450u128, 1_449] {
        let mut w = World::lev(100, 5);
        w.deposit(W1, capital);
        w.deposit(L, 100_000);
        w.trade(W1, L, 100, 100).unwrap();
        w.walk_price(95);
        let _ = w.refresh(L);
        let pre = w.refresh(W1).unwrap();
        let eq = capital as i128 - 500;
        let mm = 950i128;
        let liq = w.liquidate_outcome(W1);
        if eq >= mm {
            // Report what the certificate said too, so a stricter threshold is visible.
            score.lose(
                "S6 W1 certified equity at exactly MM (vs spec)",
                eq - pre.certified_equity,
            );
            if liq.is_ok() {
                score.avoidable("S6 W1 liquidated at exactly MM");
            }
        } else {
            match liq {
                Ok(o) => {
                    let fee = spec_liq_fee(o.closed_q, 95);
                    score.lose(
                        "S6 W1 liquidation fee above spec §4.5",
                        o.fee_charged as i128 - fee as i128,
                    );
                    let _ = w.refresh(W1);
                    score.lose(
                        "S6 W1 value lost beyond realised loss + spec fee",
                        (eq - fee as i128) - w.value(W1),
                    );
                }
                Err(e) => panic!("S6 W1 one atom below MM must be liquidatable: {e:?}"),
            }
        }
    }
}

/// Close everyone after resolution and return (total payout, accounts left open, payouts).
fn close_all(w: &mut World, order: &[usize]) -> (u128, usize, Vec<Option<u128>>) {
    let p = w.resolve_and_close(order);
    let total = p.iter().flatten().sum();
    let open = order.iter().filter(|&&i| p[i].is_none()).count();
    (total, open, p)
}

/// Value stranded after everyone closed: deposits that were paid neither to a trader nor to
/// insurance (fees), minus the spec's own floor allowance of one atom per positive-PnL payout
/// (§8.8 `floor(PNL_i * num / den)`).
fn stranded(deposits: u128, w: &World, paid: u128, floor_allowance: u128) -> i128 {
    deposits as i128 - paid as i128 - w.header.insurance.get() as i128 - floor_allowance as i128
}

/// S7: bankruptcy and ADL ordering (spec §5.4: the deficit is socialised pro rata on the
/// opposing side; a winner's loss must not depend on the order winners are touched).
///
/// W1 long 10 and W2 long 30 against L short 40 at 100, L with 450. The oracle jumps to 115:
/// L owes 600 and is bankrupt by 150. L is liquidated; then the winners are refreshed in
/// either order, the market resolves and everyone closes. Winners collectively must receive
/// every atom L paid (no fees reach winners; L's fee is uncollectible).
fn adl_ordering(score: &mut Score) {
    let mut results = Vec::new();
    for order in [[W1, W2], [W2, W1]] {
        let mut w = World::lev(100, 5);
        w.deposit(W1, 1_000);
        w.deposit(W2, 3_000);
        w.deposit(L, 450);
        w.trade(W1, L, 10, 100).unwrap();
        w.trade(W2, L, 30, 100).unwrap();
        let deposits = w.total_deposits_and_insurance();
        w.walk_price(115);
        w.liquidate(L).expect("S7 bankrupt L must be liquidatable");
        for i in order {
            let _ = w.refresh(i);
        }
        if w.liquidate(W1).is_ok() || w.liquidate(W2).is_ok() {
            score.avoidable("S7 winner liquidated after ADL");
        }
        let (paid, open, p) = close_all(&mut w, &[L, order[0], order[1]]);
        assert_eq!(open, 0, "S7 every account must close");
        results.push((p, true));
        score.lose(
            &format!("S7 stranded after ADL, order {:?} (open {open})", order),
            stranded(deposits, &w, paid, 2),
        );
    }
    // Pro-rata entitlement: deposit plus (q_i / OI) of the 450 L paid. The spec floors each
    // payout (§8.8), so the half atom of W1's 1_112.5 may land on either winner; only a payout
    // below floor(entitlement) is a loss.
    for (p, _) in &results {
        for (i, q) in [(W1, 10u128), (W2, 30)] {
            let deposit = if i == W1 { 1_000 } else { 3_000 };
            // §5.4 rounds the K loss up (ceil), so one atom per winner below the floor of
            // the exact share is spec rounding.
            let floor_share = deposit + 450 * q / 40;
            score.lose(
                &format!("S7 W{} below floor(pro-rata share) - 1 (spec ceil)", i + 1),
                floor_share as i128 - 1 - p[i].unwrap() as i128,
            );
        }
    }
    let (a, b) = (&results[0].0, &results[1].0);
    let diff: i128 = [W1, W2]
        .iter()
        .map(|&i| (a[i].unwrap_or(0) as i128 - b[i].unwrap_or(0) as i128).abs())
        .sum();
    score.lose(
        &format!("S7 info: touch order moves {diff} sub-floor rounding atom(s) (not counted)"),
        0,
    );
}

/// S8: fee and funding rounding.
///
/// (a) Spec §4.5 trading fee = ceil(notional * bps / 10_000), charged once per side. W1 buys
///     7 units from L at the oracle price 100 with 30 bps: each side pays ceil(2.1) = 3.
/// (b) Funding cadence: W1 long 700, L short 700, funding for 30 slots. In one world a keeper
///     cranks and refreshes both every slot, in the twin only at the end; the spec's accrual is exact
///     (§5.3), so the value of each must not depend on how often it was touched.
/// (c) Funding conservation: after resolution and close, what longs paid in funding reaches
///     shorts (no stranded value beyond the per-payout floor).
fn fee_and_funding_rounding(score: &mut Score) {
    {
        let mut w = World::lev(100, 5);
        w.deposit(W1, 1_000);
        w.deposit(L, 1_000);
        w.trade_fee(W1, L, 7 * POS_SCALE, 100, 30).unwrap();
        let _ = w.refresh(W1);
        let _ = w.refresh(L);
        let spec_fee = (700u128 * 30).div_ceil(10_000) as i128;
        score.lose(
            "S8a W1 trading fee above spec",
            (1_000 - w.value(W1)) - spec_fee,
        );
        score.lose(
            "S8a L (maker) trading fee above spec",
            (1_000 - w.value(L)) - spec_fee,
        );
    }
    let mut values = Vec::new();
    for every_slot in [true, false] {
        let mut w = World::lev(100, 5);
        w.deposit(W1, 10_000);
        w.deposit(L, 10_000);
        w.trade_q(W1, L, 700 * POS_SCALE, 100).unwrap();
        let deposits = w.total_deposits_and_insurance();
        let mut refused = 0;
        for _ in 0..30 {
            if every_slot {
                refused += w.keeper_slot(&[W1, L], FUNDING_E9 as i128);
            } else {
                w.funding_slot(FUNDING_E9 as i128);
            }
        }
        refused += w.keeper_slot(&[W1, L], 0);
        values.push((w.value(W1), w.value(L)));
        if refused != 0 {
            score.lose(
                &format!("S8b refreshes refused (every_slot={every_slot}): {refused}"),
                0,
            );
        }
        if !every_slot {
            let (paid, open, _) = close_all(&mut w, &[W1, L]);
            score.lose(
                &format!("S8c stranded funding after close (open {open})"),
                stranded(deposits, &w, paid, 1),
            );
        }
    }
    let (f, s) = (values[0], values[1]);
    assert!(
        s.0 < 10_000 && s.1 > 10_000,
        "S8b funding must have moved value"
    );
    // §5.2 floors each touch, so a cadence difference is spec rounding: reported, not counted.
    score.lose(
        &format!(
            "S8b info: funding cadence every-slot vs once moves W1 {} / L {} (not counted)",
            s.0 - f.0,
            s.1 - f.1
        ),
        0,
    );
}

/// S9: resolved-market payout (spec §8.8: a fully backed winner is paid floor(PNL * num / den)
/// with num == den; close order must not matter).
///
/// W1 long 7/3, W2 long 13/3 against L short 20/3 units at 100; the oracle moves to 103 and
/// the market resolves. Every winner's gain is backed atom for atom by L's loss.
fn resolved_payout(score: &mut Score) {
    let mut results = Vec::new();
    for order in [[W1, W2], [W2, W1]] {
        let mut w = World::lev(100, 5);
        w.deposit(W1, 500);
        w.deposit(W2, 500);
        w.deposit(L, 1_000);
        w.trade_q(W1, L, 7 * POS_SCALE / 3, 100).unwrap();
        w.trade_q(W2, L, 13 * POS_SCALE / 3, 100).unwrap();
        let deposits = w.total_deposits_and_insurance();
        w.walk_price(103);
        for i in [L, W1, W2] {
            let _ = w.refresh(i);
        }
        // Entitlement from the trades alone: deposit + floor(size_q * (103 - 100) / POS_SCALE)
        // (the sizes are POS_SCALE / 3 fractions, truncated when the trade was placed).
        let entitled: Vec<i128> = [7u128, 13]
            .iter()
            .map(|&n| 500 + ((n * POS_SCALE / 3) * 3 / POS_SCALE) as i128)
            .collect();
        let (paid, open, p) = close_all(&mut w, &[L, order[0], order[1]]);
        for (k, &i) in [W1, W2].iter().enumerate() {
            score.lose(
                &format!(
                    "S9 W{} resolved payout vs capital+PnL, order {:?}",
                    i + 1,
                    order
                ),
                entitled[k] - p[i].unwrap_or(0) as i128,
            );
        }
        score.lose(
            &format!(
                "S9 stranded after resolved close, order {:?} (open {open})",
                order
            ),
            stranded(deposits, &w, paid, 2),
        );
        assert_eq!(open, 0, "S9 every account must close");
        results.push(p);
    }
    let moved: i128 = [W1, W2]
        .iter()
        .map(|&i| (results[0][i].unwrap() as i128 - results[1][i].unwrap() as i128).abs())
        .sum::<i128>()
        / 2;
    score.lose(
        &format!("S9 info: close order moves {moved} atom(s) between winners (counted above)"),
        0,
    );
}

/// S10: crank order (spec §8.7: the keeper's candidate order decides only which accounts are
/// processed, never what an honest account is worth).
///
/// W1 long 700, W2 long 1_100 against L short 1_800 at 100; funding accrues over 20 slots while the
/// keeper cranks and refreshes W1, W2, L in one order in one world and L, W2, W1 in the twin.
fn crank_order(score: &mut Score) {
    let mut values = Vec::new();
    for order in [[W1, W2, L], [L, W2, W1]] {
        let mut w = World::lev(100, 5);
        w.deposit(W1, 10_000);
        w.deposit(W2, 15_000);
        w.deposit(L, 30_000);
        w.trade_q(W1, L, 700 * POS_SCALE, 100).unwrap();
        w.trade_q(W2, L, 1_100 * POS_SCALE, 100).unwrap();
        let mut refused = 0;
        for _ in 0..20 {
            refused += w.keeper_slot(&order, FUNDING_E9 as i128);
        }
        if refused != 0 {
            score.lose(
                &format!("S10 refreshes refused, order {:?}: {refused}", order),
                0,
            );
        }
        for i in [W1, W2, L] {
            let _ = w.refresh(i);
        }
        values.push([w.value(W1), w.value(W2), w.value(L)]);
    }
    assert!(values[0][2] > 30_000, "S10 funding must have moved value");
    let moved: i128 = (0..3)
        .map(|k| (values[0][k] - values[1][k]).abs())
        .sum::<i128>()
        / 2;
    // A per-touch floor difference would be §5.2 rounding: reported, not counted.
    score.lose(
        &format!("S10 info: crank order moves {moved} atom(s) (not counted)"),
        0,
    );
}

/// S11: holding-fee rent stays junior to losses (fork-only rule, not in spec.md: v2.2 rent
/// invariant I-R7 in
/// docs/v22-band-rent-kani-design.md and src/band_rent.rs: "capital that an unsettled loss
/// already owns is never taken for rent"; upstream spec v16.9.1 preamble: a refresh applies the
/// newly observed negative deltas before the nonnegative ones so losses reserve backing first).
///
/// Two-asset market (a growth market: rent on, no band; reachable on mainnet builds with
/// `max_portfolio_assets > 1`), rent on longs only. X is long 10_000 units of each asset
/// against L. Both assets fall 100 -> 85 while rent accrues on X's legs, so X's two losses
/// (150_000 each) exceed its 200_000 of capital. Every atom of rent X pays in the refresh that
/// leaves it insolvent is capital its losses owned: it reaches insurance/LP instead of L, the
/// honest counterparty whose gain those losses back.
fn rent_junior_to_losses(score: &mut Score) {
    let (rent_paid, insolvent) = rent_two_losing_legs(200_000);
    score.lose(
        &format!("S11 rent taken from loss-owned capital (X insolvent={insolvent})"),
        if insolvent { rent_paid as i128 } else { 0 },
    );
}

/// The S11 world with `capital` for X; returns (rent X paid in its final refresh, X
/// insolvent after it).
fn rent_two_losing_legs(capital: u128) -> (u128, bool) {
    let mut c = lev_config();
    c.max_portfolio_assets = 2;
    c.max_market_slots = 2;
    c.rent_max_e9_per_slot = band_rent::MAX_RENT_E9_PER_SLOT;
    let rent = band_rent::MAX_RENT_E9_PER_SLOT;
    let (x, l) = (W1, L);
    let mut w = World::with_assets(c, 100, 2, 5);
    w.deposit(x, capital);
    w.deposit(l, 1_000_000);
    for asset in 0..2usize {
        w.exec(|m, accounts| {
            let (a, b) = two(accounts, x, l);
            m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
                &mut PortfolioV16ViewMut::new(a),
                &mut PortfolioV16ViewMut::new(b),
                TradeRequestV16 {
                    asset_index: asset as _,
                    size_q: (10_000 * POS_SCALE) as i128,
                    exec_price: 100,
                    fee_bps: 0,
                },
                true,
            )
            .map(|_| ())
        })
        .unwrap();
    }
    // Walk both assets down 4 per slot (< 5% cap), accruing rent on longs.
    let mut price = 100u64;
    while price > 85 {
        price = (price - 4).max(85);
        let next = w.slot() + 1;
        for asset in 0..2usize {
            w.exec(|m, _| {
                m.set_asset_raw_oracle_target_not_atomic(asset, price)?;
                m.accrue_asset_to_with_rent_not_atomic(asset, next, price, 0, rent, 0, true)
                    .map(|_| ())
            })
            .unwrap();
        }
    }
    let unrouted = |w: &World| -> u128 {
        (0..2)
            .map(|i| {
                w.markets[i]
                    .engine
                    .asset
                    .try_to_runtime()
                    .unwrap()
                    .rent_unrouted_atoms
            })
            .sum()
    };
    let before = unrouted(&w);
    let refreshed = w.refresh(x);
    assert!(
        refreshed.is_ok(),
        "S11 X's refresh must run (else nothing is measured)"
    );
    let rent_paid = unrouted(&w)
        .checked_sub(before)
        .expect("refresh never lowers unrouted rent");
    (rent_paid, w.value(x) < 0)
}

/// Rent stays junior without being forgiven: a solvent account with the same two losing legs
/// pays every atom of its rent, exactly as when each leg was charged right after its delta.
#[test]
fn solvent_two_losing_legs_pay_full_rent() {
    let (rent_paid, insolvent) = rent_two_losing_legs(1_000_000);
    println!("RENT solvent two-leg rent paid: {rent_paid}");
    assert!(!insolvent);
    assert_eq!(rent_paid, SOLVENT_TWO_LEG_RENT);
}

/// Measured on 91911d5b (rent charged per leg) with the same world.
const SOLVENT_TWO_LEG_RENT: u128 = 72;

/// Golden score (91911d5b baseline: 228 / 1; 192 / 1 with the rent fix). Any change, better or worse, must update these
/// in the same PR, so a scenario that silently stops measuring cannot pass as an improvement.
const CEILING_ATOMS: i128 = 192;
const CEILING_AVOIDABLE: u32 = 1;

#[test]
fn leniency_scoreboard() {
    let mut score = Score::default();
    lien_rate_scenarios(&mut score);
    near_threshold_liquidation(&mut score);
    adl_ordering(&mut score);
    fee_and_funding_rounding(&mut score);
    resolved_payout(&mut score);
    crank_order(&mut score);
    rent_junior_to_losses(&mut score);

    let total: i128 = score.lost.iter().map(|(_, v)| (*v).max(0)).sum();
    for (name, v) in &score.lost {
        println!("LENIENCY {name}: {v}");
    }
    println!(
        "LENIENCY_SCORE honest_atoms_lost={total} avoidable_liquidations={}",
        score.avoidable_liquidations
    );
    assert_eq!(
        total, CEILING_ATOMS,
        "honest_atoms_lost changed: update the golden value"
    );
    assert_eq!(score.avoidable_liquidations, CEILING_AVOIDABLE);
}
