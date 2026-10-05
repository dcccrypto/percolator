//! P2b bounded lock exits (Devnet v2 Phase 2b, Builder D).
//!
//!   L1  the bankruptcy hlock clear predicate is scoped to the bankrupt claim-source domain;
//!   L2  a bounded, permissionless exit from ADL reduce-only (`wind_down_adl_position_not_atomic`);
//!   E7  ADL reduce-only and loss-stale refusals get their own engine errors.
//!
//! Every test here drives the production zero-copy view through public engine entries. The
//! in-test negative controls flip exactly one input (the hlock byte, the wind-down bound) and
//! assert the opposite outcome; the file-copy mutation controls are recorded in the PR.

use percolator::{
    bankruptcy_hlock_domain_mask, bankruptcy_hlock_is_active, bankruptcy_hlock_is_unattributed,
    bankruptcy_hlock_mark_domain, bankruptcy_hlock_mark_unattributed,
    v16_domain_count_for_market_slots, validate_bankruptcy_hlock_wire, AdlWindDownBoundV16,
    AdlWindDownRequestV16, EngineAssetSlotV16Account, LiquidationRequestV16, Market,
    MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, PermissionlessCrankActionV16,
    PermissionlessCrankRequestV16, PortfolioAccountV16Account, PortfolioV16ViewMut,
    ProvenanceHeaderV16, ProvenanceHeaderV16Account, RebalanceRequestV16, SideModeV16,
    TradeRequestV16, V16Config, V16Error, V16PodU64,
};
use percolator::{ADL_ONE, BANKRUPTCY_HLOCK_ACTIVE_BIT, POS_SCALE};

// ---------------------------------------------------------------- fixtures --
// Same fixtures as tests/f03_regression.rs (copied from tests/v16_spec_tests.rs).

fn ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    ([1; 32], [2; 32], [3; 32])
}

fn market_fixture_slots(
    market_slots: u32,
    init_price: u64,
) -> (MarketGroupV16HeaderAccount, Vec<Market<u64>>) {
    market_fixture_activated(market_slots, market_slots, init_price)
}

/// `market_slots` configured, only the first `activated` of them activated (the deployed
/// slabs are 14 configured slots with only asset 0 ever activated).
fn market_fixture_activated(
    market_slots: u32,
    activated: u32,
    init_price: u64,
) -> (MarketGroupV16HeaderAccount, Vec<Market<u64>>) {
    let (market_id, _, _) = ids();
    let max_portfolio_assets =
        market_slots.min(percolator::V16_MAX_PORTFOLIO_ASSETS_N as u32) as u16;
    let mut cfg =
        V16Config::public_user_fund_with_market_slots(max_portfolio_assets, market_slots, 0, 10);
    cfg.max_abs_funding_e9_per_slot = 10_000;
    cfg.max_price_move_bps_per_slot = 9_000;
    let mut header =
        MarketGroupV16HeaderAccount::new_dynamic(market_id, cfg, market_slots, 0).unwrap();
    let mut markets = (0..market_slots)
        .map(|i| Market::new(i as u64, EngineAssetSlotV16Account::default()))
        .collect::<Vec<_>>();
    for (i, market) in markets.iter_mut().enumerate().take(activated as usize) {
        header
            .activate_empty_asset_slot_not_atomic(
                i as u32,
                &mut market.engine,
                init_price,
                (i + 1) as u64,
            )
            .unwrap();
    }
    {
        let view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        view.validate_shape().unwrap();
    }
    (header, markets)
}

fn account_fixture(market_slots: u32, account_seed: u8) -> PortfolioAccountV16Account {
    let (market_id, _, owner) = ids();
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        market_id,
        [account_seed; 32],
        owner,
    ));
    let _ = v16_domain_count_for_market_slots(market_slots).unwrap();
    let mut account = PortfolioAccountV16Account::default();
    account.init_empty_in_place(header).unwrap();
    account
}

fn signed_q(q: u128) -> i128 {
    i128::try_from(q).unwrap()
}

const PRICE: u64 = 1_000_000;
const DIP: u64 = 700_000;
const MARK: u64 = PRICE + 1_800_000;
const LOT: u128 = 3 * POS_SCALE;

type Header = MarketGroupV16HeaderAccount;
type Markets = Vec<Market<u64>>;

fn trade(
    header: &mut Header,
    markets: &mut Markets,
    long: &mut PortfolioAccountV16Account,
    short: &mut PortfolioAccountV16Account,
    size_q: i128,
    price: u64,
) -> Result<(), V16Error> {
    let mut market = MarketGroupV16ViewMut::new(header, markets);
    let mut lv = PortfolioV16ViewMut::new(long);
    let mut sv = PortfolioV16ViewMut::new(short);
    market
        .execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut lv,
            &mut sv,
            TradeRequestV16 {
                asset_index: 0,
                size_q,
                exec_price: price,
                fee_bps: 0,
            },
            true,
        )
        .map(|_| ())
}

fn deposit(header: &mut Header, markets: &mut Markets, a: &mut PortfolioAccountV16Account, amt: u128) {
    let mut market = MarketGroupV16ViewMut::new(header, markets);
    let mut v = PortfolioV16ViewMut::new(a);
    market.deposit_not_atomic(&mut v, amt).unwrap();
}

fn accrue(header: &mut Header, markets: &mut Markets, slot: u64, price: u64) {
    let mut market = MarketGroupV16ViewMut::new(header, markets);
    market
        .accrue_asset_to_not_atomic(0, slot, price, 0, true)
        .unwrap();
    market.markets[0].engine.asset.raw_oracle_target_price = V16PodU64::new(price);
}

/// wrapper tag 5 PermissionlessCrank / Refresh, repeated until the account is no longer
/// b-stale (the keeper does exactly this).
fn refresh(header: &mut Header, markets: &mut Markets, a: &mut PortfolioAccountV16Account) {
    let now_slot = header.current_slot.get();
    let price = markets[0].engine.asset.try_to_runtime().unwrap().effective_price;
    for _ in 0..64 {
        let mut market = MarketGroupV16ViewMut::new(header, markets);
        let mut v = PortfolioV16ViewMut::new(a);
        market
            .permissionless_crank_not_atomic(
                &mut v,
                PermissionlessCrankRequestV16 {
                    now_slot,
                    asset_index: 0,
                    effective_price: price,
                    funding_rate_e9: 0,
                    action: PermissionlessCrankActionV16::Refresh,
                },
            )
            .expect("permissionless Refresh crank");
        if a.b_stale_state == 0 && a.stale_state == 0 {
            break;
        }
    }
}

fn try_clear(header: &mut Header, markets: &mut Markets) -> bool {
    let mut market = MarketGroupV16ViewMut::new(header, markets);
    market.try_clear_bankruptcy_hlock_not_atomic().unwrap()
}

fn domain_claims(markets: &Markets, domain: usize) -> u128 {
    let slot = &markets[domain / 2].engine;
    let s = if domain & 1 == 0 {
        &slot.source_credit_long
    } else {
        &slot.source_credit_short
    };
    s.positive_claim_bound_num.get()
}

// ===================================================================== L1 ===

#[test]
fn l1_wire_encoding_is_legacy_compatible_and_sticky() {
    // Legacy value 1 is "active, unattributed" and absorbs every later attribution.
    assert!(bankruptcy_hlock_is_unattributed(1));
    assert_eq!(bankruptcy_hlock_mark_domain(1, 0), 1);
    assert_eq!(bankruptcy_hlock_mark_domain(1, 1), 1);
    // From inactive, an attribution sets bit 0 and the domain bit.
    assert_eq!(bankruptcy_hlock_mark_domain(0, 0), 0b011);
    assert_eq!(bankruptcy_hlock_mark_domain(0, 1), 0b101);
    assert_eq!(bankruptcy_hlock_mark_domain(0b011, 1), 0b111);
    assert_eq!(bankruptcy_hlock_domain_mask(0b111), 0b11);
    // Idempotent.
    assert_eq!(bankruptcy_hlock_mark_domain(0b101, 1), 0b101);
    // An unattributed event collapses any attribution to 1.
    assert_eq!(bankruptcy_hlock_mark_unattributed(0b111), 1);
    assert_eq!(bankruptcy_hlock_mark_unattributed(0), 1);
    // A domain the byte cannot represent is unattributed.
    assert_eq!(bankruptcy_hlock_mark_domain(0, 7), 1);
    // Every marked value is active and bit 0 is always set.
    for wire in 0u8..=255 {
        for d in 0..9 {
            let m = bankruptcy_hlock_mark_domain(wire, d);
            assert!(bankruptcy_hlock_is_active(m));
            assert_eq!(m & BANKRUPTCY_HLOCK_ACTIVE_BIT, 1);
        }
    }
    // Shape check: bit 0 must be set when anything else is; no bit past the domain count.
    assert!(validate_bankruptcy_hlock_wire(0, 2).is_ok());
    assert!(validate_bankruptcy_hlock_wire(1, 2).is_ok());
    assert!(validate_bankruptcy_hlock_wire(0b111, 2).is_ok());
    assert_eq!(validate_bankruptcy_hlock_wire(0b010, 2), Err(V16Error::InvalidConfig));
    assert_eq!(validate_bankruptcy_hlock_wire(0b1001, 2), Err(V16Error::InvalidConfig));
    assert!(validate_bankruptcy_hlock_wire(0b1001, 4).is_ok());
    assert!(validate_bankruptcy_hlock_wire(0xff, 28).is_ok());
}

/// Episode 1 leaves a flat SHORT winner `x` whose claim is sourced from domain 0
/// (asset 0, Long). Episode 2 is the f03 world: five longs, a fat and a thin short, a price
/// rise, and a PUBLIC liquidation of the thin short whose residual is socialised onto the
/// longs. Returns (header, markets, x, y, longs, short_big, short_thin).
struct World {
    header: Header,
    markets: Markets,
    x: PortfolioAccountV16Account,
    longs: Vec<PortfolioAccountV16Account>,
    short_big: PortfolioAccountV16Account,
}

fn two_episode_world(market_slots: u32) -> World {
    two_episode_world_activated(market_slots, market_slots)
}

fn two_episode_world_activated(market_slots: u32, activated: u32) -> World {
    let (mut header, mut markets) = market_fixture_activated(market_slots, activated, PRICE);
    let mut x = account_fixture(market_slots, 50);
    let mut y = account_fixture(market_slots, 51);
    deposit(&mut header, &mut markets, &mut x, 100_000_000);
    deposit(&mut header, &mut markets, &mut y, 100_000_000);
    // Episode 1: y long / x short, price dips, both close flat. x keeps a domain-0 claim.
    trade(&mut header, &mut markets, &mut y, &mut x, signed_q(LOT), PRICE).unwrap();
    accrue(&mut header, &mut markets, 2, DIP);
    trade(&mut header, &mut markets, &mut x, &mut y, signed_q(LOT), DIP).unwrap();
    assert!(x.pnl.get() > 0, "x is a flat winner");
    assert!(!x.legs[0].try_to_runtime().unwrap().active);
    accrue(&mut header, &mut markets, 3, PRICE);

    // Episode 2: the f03 bankruptcy world, started at slot 3.
    let mut longs: Vec<PortfolioAccountV16Account> =
        (0..5).map(|i| account_fixture(market_slots, 100 + i as u8)).collect();
    let mut short_big = account_fixture(market_slots, 200);
    let mut short_thin = account_fixture(market_slots, 201);
    deposit(&mut header, &mut markets, &mut short_big, 400_000_000);
    deposit(&mut header, &mut markets, &mut short_thin, 3_250_000);
    for l in longs.iter_mut() {
        deposit(&mut header, &mut markets, l, 100_000_000);
    }
    for l in longs.iter_mut().take(4) {
        trade(&mut header, &mut markets, l, &mut short_big, signed_q(LOT), PRICE).unwrap();
    }
    trade(
        &mut header,
        &mut markets,
        &mut longs[4],
        &mut short_thin,
        signed_q(LOT),
        PRICE,
    )
    .unwrap();
    accrue(&mut header, &mut markets, 4, PRICE + 900_000);
    accrue(&mut header, &mut markets, 5, MARK);
    assert_eq!(header.bankruptcy_hlock_active, 0, "no bankruptcy yet");
    {
        let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut sv = PortfolioV16ViewMut::new(&mut short_thin);
        market
            .liquidate_account_not_atomic(&mut sv, LiquidationRequestV16 { asset_index: 0 })
            .expect("permissionless liquidation (wrapper tag 5, action Liquidate)");
    }
    for l in longs.iter_mut() {
        refresh(&mut header, &mut markets, l);
    }
    refresh(&mut header, &mut markets, &mut short_big);
    World {
        header,
        markets,
        x,
        longs,
        short_big,
    }
}

fn dump_claims(tag: &str, w: &World) {
    println!("[{tag}] d0={} d1={} ppt={} hlock={:#x}", domain_claims(&w.markets, 0), domain_claims(&w.markets, 1), w.header.pnl_pos_tot.get(), w.header.bankruptcy_hlock_active);
    let mut all: Vec<(&str, &PortfolioAccountV16Account)> = vec![("x", &w.x), ("short_big", &w.short_big)];
    for l in &w.longs { all.push(("long", l)); }
    for (n, a) in all {
        let mut doms = String::new();
        for sd in a.source_domains.iter() {
            if sd.source_claim_bound_num.get() != 0 { doms += &format!(" d{}={}", sd.domain.get(), sd.source_claim_bound_num.get()); }
        }
        println!("   {n}: cap={} pnl={}{}", a.capital.get(), a.pnl.get(), doms);
    }
}

#[test]
fn l1_hlock_clears_when_the_bankrupt_domain_claims_retire_while_unrelated_claims_remain() {
    let mut w = two_episode_world(1);
    // A bankrupt SHORT in a Live single-asset group: attributed to claim-source domain 1
    // (asset 0, Short), the domain the long winners source from.
    assert_eq!(w.header.bankruptcy_hlock_active, 0b101);
    assert!(domain_claims(&w.markets, 1) != 0, "long winners hold domain-1 claims");
    assert!(domain_claims(&w.markets, 0) != 0, "x holds an unrelated domain-0 claim");

    // PROOF DIRECTION: while any domain-1 claim remains, nothing clears it.
    assert!(!try_clear(&mut w.header, &mut w.markets));
    assert_eq!(w.header.bankruptcy_hlock_active, 0b101);

    // The price falls back below the entry: every long gives its profit (its domain-1 claim) back, as when
    // the winners close or convert. Shorts gain, but their claims source from domain 0.
    accrue(&mut w.header, &mut w.markets, 6, DIP);
    for i in 0..4 {
        refresh(&mut w.header, &mut w.markets, &mut w.longs[i]);
        // longs[4] is not settled yet and still holds its domain-1 claim: no clear.
        assert!(domain_claims(&w.markets, 1) != 0);
        assert!(!try_clear(&mut w.header, &mut w.markets));
        assert_eq!(w.header.bankruptcy_hlock_active, 0b101);
    }
    refresh(&mut w.header, &mut w.markets, &mut w.longs[4]);
    refresh(&mut w.header, &mut w.markets, &mut w.short_big);

    dump_claims("after reversal", &w);
    assert_eq!(domain_claims(&w.markets, 1), 0, "every domain-1 claim is gone");
    assert!(
        w.header.pnl_pos_tot.get() > 0,
        "unrelated winners still hold profit: the pre-P2b predicate (pnl_pos_tot == 0) would \
         keep the hlock latched here"
    );

    // The ordinary keeper Refresh that retired the last domain-1 claim cleared it.
    assert_eq!(
        w.header.bankruptcy_hlock_active, 0,
        "the scoped predicate cleared the hlock on the refresh that retired the last claim"
    );

    // NEGATIVE CONTROL (same state, legacy/unattributed byte): the global predicate holds.
    let mut legacy = (w.header, w.markets.clone());
    legacy.0.bankruptcy_hlock_active = 1;
    assert!(!try_clear(&mut legacy.0, &mut legacy.1));
    assert_eq!(legacy.0.bankruptcy_hlock_active, 1);

    // NEGATIVE CONTROL (same state, attributed to domain 0 too): domain 0 still has claims.
    let mut both = (w.header, w.markets.clone());
    both.0.bankruptcy_hlock_active = 0b111;
    assert!(!try_clear(&mut both.0, &mut both.1));

    // POSITIVE CONTROL (same state, re-latched to domain 1 only): clears.
    let mut again = (w.header, w.markets.clone());
    again.0.bankruptcy_hlock_active = 0b101;
    assert!(try_clear(&mut again.0, &mut again.1));
    let _ = &w.x;
}

#[test]
fn l1_scoped_clear_still_requires_every_other_health_term() {
    let mut w = two_episode_world(1);
    accrue(&mut w.header, &mut w.markets, 6, DIP);
    for i in 0..5 {
        refresh(&mut w.header, &mut w.markets, &mut w.longs[i]);
    }
    refresh(&mut w.header, &mut w.markets, &mut w.short_big);
    assert_eq!(domain_claims(&w.markets, 1), 0);

    // A pending domain-loss barrier (an open bankruptcy close with residual) holds it.
    // (The refreshes above already cleared it through the ordinary refresh path; each case
    // below re-latches the attributed byte on a copy and flips exactly one other term.)
    assert_eq!(w.header.bankruptcy_hlock_active, 0, "refresh cleared it automatically");
    let mut barrier = (w.header, w.markets.clone());
    barrier.0.bankruptcy_hlock_active = 0b101;
    barrier.1[0].engine.pending_domain_loss_barrier_short = V16PodU64::new(1);
    assert!(!try_clear(&mut barrier.0, &mut barrier.1));
    // A negative-PnL account, a stale certificate, or a b-stale account holds it.
    for field in 0..3 {
        let mut h = (w.header, w.markets.clone());
        h.0.bankruptcy_hlock_active = 0b101;
        match field {
            0 => h.0.negative_pnl_account_count = V16PodU64::new(1),
            1 => h.0.stale_certificate_count = V16PodU64::new(1),
            _ => h.0.b_stale_account_count = V16PodU64::new(1),
        }
        assert!(!try_clear(&mut h.0, &mut h.1), "health term {field} must hold the hlock");
    }
    // Resolved/Recovery mode falls back to the global predicate.
    let mut resolved = (w.header, w.markets.clone());
    resolved.0.bankruptcy_hlock_active = 0b101;
    resolved.0.mode = 1;
    {
        let mut market = MarketGroupV16ViewMut::new(&mut resolved.0, &mut resolved.1);
        // Raw predicate only: the clear itself does not decode the rest of a Resolved header.
        let _ = market.try_clear_bankruptcy_hlock_not_atomic();
    }
    assert!(bankruptcy_hlock_is_active(resolved.0.bankruptcy_hlock_active));

    // Control: the same copy with every term clean clears.
    let mut clean = (w.header, w.markets.clone());
    clean.0.bankruptcy_hlock_active = 0b101;
    assert!(try_clear(&mut clean.0, &mut clean.1));
}

#[test]
fn l1_deployed_shape_many_configured_slots_one_activated_is_attributed() {
    // The deployed slabs: 14 configured slots, only asset 0 ever activated.
    let w = two_episode_world_activated(14, 1);
    assert_eq!(w.header.config.max_market_slots.get(), 14);
    assert_eq!(w.header.bankruptcy_hlock_active, 0b101);
}

#[test]
fn l1_multi_asset_group_records_unattributed_and_keeps_the_global_predicate() {
    let mut w = two_episode_world(2);
    assert_eq!(
        w.header.bankruptcy_hlock_active, 1,
        "a cross-margin group cannot attribute a deficit to one domain"
    );
    accrue(&mut w.header, &mut w.markets, 6, DIP);
    for i in 0..5 {
        refresh(&mut w.header, &mut w.markets, &mut w.longs[i]);
    }
    refresh(&mut w.header, &mut w.markets, &mut w.short_big);
    assert_eq!(domain_claims(&w.markets, 1), 0);
    assert!(w.header.pnl_pos_tot.get() > 0);
    assert!(!try_clear(&mut w.header, &mut w.markets));
}

// ===================================================================== E7 ===

fn adl_world() -> (Header, Markets, PortfolioAccountV16Account, PortfolioAccountV16Account, PortfolioAccountV16Account, PortfolioAccountV16Account) {
    // Two longs against two shorts; one long exits UNILATERALLY (tag 44 RebalanceReduce),
    // which ADLs the short side: A_short = 3/4.
    let (mut header, mut markets) = market_fixture_slots(1, 100);
    let mut l1 = account_fixture(1, 10);
    let mut l2 = account_fixture(1, 11);
    let mut s1 = account_fixture(1, 12);
    let mut s2 = account_fixture(1, 13);
    for a in [&mut l1, &mut l2, &mut s1, &mut s2] {
        deposit(&mut header, &mut markets, a, 10_000);
    }
    trade(&mut header, &mut markets, &mut l1, &mut s1, signed_q(2 * POS_SCALE), 100).unwrap();
    trade(&mut header, &mut markets, &mut l2, &mut s2, signed_q(2 * POS_SCALE), 100).unwrap();
    {
        let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut v = PortfolioV16ViewMut::new(&mut l1);
        market
            .rebalance_reduce_position_not_atomic(
                &mut v,
                RebalanceRequestV16 {
                    asset_index: 0,
                    reduce_q: POS_SCALE,
                },
            )
            .unwrap();
    }
    let a = markets[0].engine.asset.try_to_runtime().unwrap();
    assert_eq!(a.a_short, ADL_ONE * 3 / 4);
    assert_eq!(a.a_long, ADL_ONE);
    (header, markets, l1, l2, s1, s2)
}

#[test]
fn e7_adl_reduce_only_open_returns_adl_reduce_only_not_lock_active() {
    let (mut header, mut markets, mut l1, mut l2, mut s1, _s2) = adl_world();
    let mut fresh_long = account_fixture(1, 20);
    let mut fresh_short = account_fixture(1, 21);
    deposit(&mut header, &mut markets, &mut fresh_long, 10_000);
    deposit(&mut header, &mut markets, &mut fresh_short, 10_000);
    // Opening (attach) on either side: AdlReduceOnly.
    assert_eq!(
        trade(&mut header, &mut markets, &mut fresh_long, &mut fresh_short, signed_q(POS_SCALE), 100),
        Err(V16Error::AdlReduceOnly)
    );
    // Enlarging an existing leg: AdlReduceOnly.
    assert_eq!(
        trade(&mut header, &mut markets, &mut l2, &mut fresh_short, signed_q(POS_SCALE), 100),
        Err(V16Error::AdlReduceOnly)
    );
    // Reducing matched risk is still allowed (close-only, not frozen).
    trade(&mut header, &mut markets, &mut s1, &mut l1, signed_q(POS_SCALE / 2), 100)
        .expect("a matched reduce stays live under ADL");
}

#[test]
fn e7_loss_stale_open_returns_loss_stale_not_lock_active() {
    let (mut header, mut markets) = market_fixture_slots(1, 100);
    let mut l = account_fixture(1, 30);
    let mut s = account_fixture(1, 31);
    deposit(&mut header, &mut markets, &mut l, 1_000);
    deposit(&mut header, &mut markets, &mut s, 1_000);
    trade(&mut header, &mut markets, &mut l, &mut s, signed_q(POS_SCALE), 100).unwrap();
    accrue(&mut header, &mut markets, 3, 101);
    assert_eq!(
        trade(&mut header, &mut markets, &mut l, &mut s, signed_q(POS_SCALE), 101),
        Err(V16Error::LossStale)
    );
}

#[test]
fn e7_lifecycle_lock_keeps_lock_active() {
    let (mut header, mut markets) = market_fixture_slots(1, 100);
    let mut l = account_fixture(1, 32);
    let mut s = account_fixture(1, 33);
    deposit(&mut header, &mut markets, &mut l, 1_000);
    deposit(&mut header, &mut markets, &mut s, 1_000);
    {
        let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        market.mark_asset_drain_only_not_atomic(0).unwrap();
    }
    assert_eq!(
        trade(&mut header, &mut markets, &mut l, &mut s, signed_q(POS_SCALE), 100),
        Err(V16Error::LockActive),
        "an asset-lifecycle lock is not ADL and keeps code 21"
    );
}

// ===================================================================== L2 ===

fn wind_down(
    header: &mut Header,
    markets: &mut Markets,
    a: &mut PortfolioAccountV16Account,
    bound: AdlWindDownBoundV16,
) -> Result<(u128, bool), V16Error> {
    let mut market = MarketGroupV16ViewMut::new(header, markets);
    let mut v = PortfolioV16ViewMut::new(a);
    market
        .wind_down_adl_position_not_atomic(
            &mut v,
            AdlWindDownRequestV16 {
                asset_index: 0,
                bound,
            },
        )
        .map(|o| (o.closed_q, o.adl_cleared))
}

fn total_value(header: &Header, accounts: &[&PortfolioAccountV16Account]) -> i128 {
    let mut v = header.insurance.get() as i128;
    for a in accounts {
        v += a.capital.get() as i128 + a.pnl.get();
    }
    v
}

#[test]
fn l2_wind_down_refuses_a_market_not_in_adl() {
    let (mut header, mut markets) = market_fixture_slots(1, 100);
    let mut l = account_fixture(1, 40);
    let mut s = account_fixture(1, 41);
    deposit(&mut header, &mut markets, &mut l, 1_000);
    deposit(&mut header, &mut markets, &mut s, 1_000);
    trade(&mut header, &mut markets, &mut l, &mut s, signed_q(POS_SCALE), 100).unwrap();
    assert_eq!(
        wind_down(&mut header, &mut markets, &mut l, AdlWindDownBoundV16::EpisodeExpired),
        Err(V16Error::NonProgress),
        "no ADL, no forced close: holders keep their positions"
    );
}

#[test]
fn l2_dust_bound_is_checked_by_the_engine() {
    let (mut header, mut markets, _l1, mut l2, _s1, _s2) = adl_world();
    // OI is 3 * POS_SCALE effective on each side at price 100 -> notional 300.
    let a = markets[0].engine.asset.try_to_runtime().unwrap();
    assert_eq!(a.oi_eff_long_q, 3 * POS_SCALE);
    assert_eq!(
        wind_down(
            &mut header,
            &mut markets,
            &mut l2,
            AdlWindDownBoundV16::DustNotional { max_notional_atoms: 299 }
        ),
        Err(V16Error::NonProgress),
        "above the dust threshold the engine refuses"
    );
    let (closed, _) = wind_down(
        &mut header,
        &mut markets,
        &mut l2,
        AdlWindDownBoundV16::DustNotional {
            max_notional_atoms: 300,
        },
    )
    .expect("at the dust threshold the engine allows it");
    assert_eq!(closed, 2 * POS_SCALE);
}

#[test]
fn l2_wind_down_exits_adl_conserves_value_and_reopens_only_after_reset() {
    let (mut header, mut markets, mut l1, mut l2, mut s1, mut s2) = adl_world();
    // Settle everyone first so the value ledger below is exact.
    for a in [&mut l1, &mut l2, &mut s1, &mut s2] {
        refresh(&mut header, &mut markets, a);
    }
    let mut fresh_long = account_fixture(1, 60);
    let mut fresh_short = account_fixture(1, 61);
    deposit(&mut header, &mut markets, &mut fresh_long, 10_000);
    deposit(&mut header, &mut markets, &mut fresh_short, 10_000);
    let vault0 = header.vault.get();
    let value0 = total_value(&header, &[&l1, &l2, &s1, &s2]);

    // Close the LONG side (2 legs: l1 has 1 lot left, l2 has 2 lots). Shorts decay via A.
    let (closed1, cleared1) =
        wind_down(&mut header, &mut markets, &mut l1, AdlWindDownBoundV16::EpisodeExpired)
            .unwrap();
    assert_eq!(closed1, POS_SCALE);
    assert!(!cleared1);
    assert!(!l1.legs[0].try_to_runtime().unwrap().active);

    // Mid wind-down nobody can open into either side.
    assert_eq!(
        trade(&mut header, &mut markets, &mut fresh_long, &mut fresh_short, signed_q(POS_SCALE), 100),
        Err(V16Error::AdlReduceOnly)
    );

    let (closed2, cleared2) =
        wind_down(&mut header, &mut markets, &mut l2, AdlWindDownBoundV16::EpisodeExpired)
            .unwrap();
    assert_eq!(closed2, 2 * POS_SCALE);
    assert!(cleared2, "the last leg of a side zeroes both sides and resets A");
    let a = markets[0].engine.asset.try_to_runtime().unwrap();
    assert_eq!((a.a_long, a.a_short), (ADL_ONE, ADL_ONE));
    assert_eq!((a.oi_eff_long_q, a.oi_eff_short_q), (0, 0));

    // Reset survivors are stale; opens are still refused until the reset finalizes.
    assert_eq!(a.mode_short, SideModeV16::ResetPending);
    assert_eq!(
        trade(&mut header, &mut markets, &mut fresh_long, &mut fresh_short, signed_q(POS_SCALE), 100),
        Err(V16Error::AdlReduceOnly),
        "no early open into a side that is still resetting"
    );
    // The keeper's ordinary Refresh settles the survivors; the reset finalizes.
    refresh(&mut header, &mut markets, &mut s1);
    refresh(&mut header, &mut markets, &mut s2);
    {
        let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        for side in [percolator::SideV16::Long, percolator::SideV16::Short] {
            let _ = market.finalize_side_reset_not_atomic(0, side);
        }
    }
    let a = markets[0].engine.asset.try_to_runtime().unwrap();
    assert_eq!((a.mode_long, a.mode_short), (SideModeV16::Normal, SideModeV16::Normal));
    assert!(!s1.legs[0].try_to_runtime().unwrap().active);
    assert!(!s2.legs[0].try_to_runtime().unwrap().active);

    // CONSERVATION: no value created or destroyed, no fee, vault untouched.
    assert_eq!(header.vault.get(), vault0);
    assert_eq!(total_value(&header, &[&l1, &l2, &s1, &s2]), value0);
    {
        let market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        market.validate_shape().unwrap();
    }

    // The market is open again.
    trade(&mut header, &mut markets, &mut fresh_long, &mut fresh_short, signed_q(POS_SCALE), 100)
        .expect("after the reset both sides reopen");
}

#[test]
fn l2_wind_down_refuses_an_account_with_a_liquidation_deficit() {
    // Thin short, big move up: the short is past maintenance (a deficit). Wind-down must
    // refuse it (liquidation owns deficits); the keeper liquidates instead.
    let (mut header, mut markets) = market_fixture_slots(1, PRICE);
    let mut thin = account_fixture(1, 70);
    let mut l1 = account_fixture(1, 71);
    let mut l2 = account_fixture(1, 72);
    let mut s2 = account_fixture(1, 73);
    deposit(&mut header, &mut markets, &mut thin, 3_250_000);
    for a in [&mut l1, &mut l2, &mut s2] {
        deposit(&mut header, &mut markets, a, 100_000_000);
    }
    trade(&mut header, &mut markets, &mut l1, &mut thin, signed_q(LOT), PRICE).unwrap();
    trade(&mut header, &mut markets, &mut l2, &mut s2, signed_q(2 * LOT), PRICE).unwrap();
    // ADL the short side: l2 exits one lot unilaterally.
    {
        let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut v = PortfolioV16ViewMut::new(&mut l2);
        market
            .rebalance_reduce_position_not_atomic(
                &mut v,
                RebalanceRequestV16 {
                    asset_index: 0,
                    reduce_q: LOT,
                },
            )
            .unwrap();
    }
    assert!(markets[0].engine.asset.try_to_runtime().unwrap().a_short < ADL_ONE);
    accrue(&mut header, &mut markets, 2, PRICE + 900_000);
    accrue(&mut header, &mut markets, 3, MARK);
    assert_eq!(
        wind_down(&mut header, &mut markets, &mut thin, AdlWindDownBoundV16::EpisodeExpired),
        Err(V16Error::LockActive)
    );
    // A healthy account in the same market can still be wound down.
    wind_down(&mut header, &mut markets, &mut s2, AdlWindDownBoundV16::EpisodeExpired)
        .expect("a healthy holder is wound down");
}

// ============================================ L2: 10-04 STONK / Jimothy shapes ===
//
// STONK and Jimothy left ADL on their own (one reset each side) between the 10-04 audit and
// the 10-05 snapshot, so their ADL books no longer exist on chain to replay byte-for-byte
// (Percolator's does: tests/p2b_live_replay.rs). These rebuild the 10-04 SHAPE through public
// entries only: the holder counts and both A factors from the 10-04 scan, with two helper
// accounts whose unilateral exits (tag 44) produce the A factors, exactly as on chain.

fn adl_shape_replay(n_long: usize, n_short: usize, a_long_target: f64, a_short_target: f64) {
    let (mut header, mut markets) = market_fixture_slots(1, PRICE);
    let mut longs: Vec<_> = (0..n_long).map(|i| account_fixture(1, 80 + i as u8)).collect();
    let mut shorts: Vec<_> = (0..n_short).map(|i| account_fixture(1, 90 + i as u8)).collect();
    let mut t = account_fixture(1, 98);
    let mut u = account_fixture(1, 99);
    for a in longs.iter_mut().chain(shorts.iter_mut()) {
        deposit(&mut header, &mut markets, a, 10_000_000_000);
    }
    deposit(&mut header, &mut markets, &mut t, 100_000_000_000);
    deposit(&mut header, &mut markets, &mut u, 100_000_000_000);
    for l in longs.iter_mut() {
        for s in shorts.iter_mut() {
            trade(&mut header, &mut markets, l, s, signed_q(LOT), PRICE).unwrap();
        }
    }
    let real = (n_long * n_short) as u128 * LOT;
    let helper = 10 * real;
    trade(&mut header, &mut markets, &mut t, &mut u, signed_q(helper), PRICE).unwrap();
    let oi0 = real + helper;
    let q_t = ((oi0 as f64) * (1.0 - a_short_target)) as u128;
    let rr = |a: &mut PortfolioAccountV16Account, q: u128, header: &mut Header, markets: &mut Markets| {
        let mut market = MarketGroupV16ViewMut::new(header, markets);
        let mut v = PortfolioV16ViewMut::new(a);
        market
            .rebalance_reduce_position_not_atomic(
                &mut v,
                RebalanceRequestV16 {
                    asset_index: 0,
                    reduce_q: q,
                },
            )
            .unwrap();
    };
    rr(&mut t, q_t, &mut header, &mut markets);
    let oi1 = markets[0].engine.asset.try_to_runtime().unwrap().oi_eff_long_q;
    let q_u = ((oi1 as f64) * (1.0 - a_long_target)) as u128;
    rr(&mut u, q_u, &mut header, &mut markets);
    let a = markets[0].engine.asset.try_to_runtime().unwrap();
    let al = a.a_long as f64 / ADL_ONE as f64;
    let as_ = a.a_short as f64 / ADL_ONE as f64;
    println!("shape {n_long}L/{n_short}S: A = {al:.3}/{as_:.3} (10-04: {a_long_target}/{a_short_target})");
    assert!((al - a_long_target).abs() < 0.01 && (as_ - a_short_target).abs() < 0.01);

    // Everyone settled; snapshot the ledger.
    let mut all: Vec<&mut PortfolioAccountV16Account> = longs.iter_mut().chain(shorts.iter_mut()).collect();
    all.push(&mut t);
    all.push(&mut u);
    for a in all.iter_mut() {
        refresh(&mut header, &mut markets, a);
    }
    let vault0 = header.vault.get();
    let value0 = {
        let refs: Vec<&PortfolioAccountV16Account> = all.iter().map(|a| &**a).collect();
        total_value(&header, &refs)
    };

    // Wind down the side with fewer legs.
    let long_side_legs = all.iter().filter(|a| {
        let l = a.legs[0].try_to_runtime().unwrap();
        l.active && l.side == percolator::SideV16::Long
    }).count();
    let short_side_legs = all.iter().filter(|a| {
        let l = a.legs[0].try_to_runtime().unwrap();
        l.active && l.side == percolator::SideV16::Short
    }).count();
    let wind_side = if long_side_legs <= short_side_legs {
        percolator::SideV16::Long
    } else {
        percolator::SideV16::Short
    };
    let mut steps = 0;
    let mut cleared = false;
    for a in all.iter_mut() {
        let l = a.legs[0].try_to_runtime().unwrap();
        if l.active && l.side == wind_side && !cleared {
            let (_, c) = wind_down(&mut header, &mut markets, a, AdlWindDownBoundV16::EpisodeExpired)
                .expect("wind-down step");
            steps += 1;
            cleared = c;
        }
    }
    assert!(cleared, "bounded: one step per leg of the smaller side ({steps} steps)");
    for a in all.iter_mut() {
        refresh(&mut header, &mut markets, a);
    }
    {
        let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        for side in [percolator::SideV16::Long, percolator::SideV16::Short] {
            let _ = market.finalize_side_reset_not_atomic(0, side);
        }
        market.validate_shape().unwrap();
    }
    let a = markets[0].engine.asset.try_to_runtime().unwrap();
    assert_eq!((a.a_long, a.a_short), (ADL_ONE, ADL_ONE));
    assert_eq!((a.mode_long, a.mode_short), (SideModeV16::Normal, SideModeV16::Normal));
    assert_eq!(header.vault.get(), vault0);
    let refs: Vec<&PortfolioAccountV16Account> = all.iter().map(|a| &**a).collect();
    assert_eq!(total_value(&header, &refs), value0, "conservation");
    // Reopens.
    let (x, y) = all.split_at_mut(1);
    trade(&mut header, &mut markets, x[0], y[0], signed_q(POS_SCALE), PRICE)
        .expect("reopened after the wind-down");
}

#[test]
fn l2_replay_shape_stonk_10_04() {
    adl_shape_replay(1, 2, 0.966, 0.193);
}

#[test]
fn l2_replay_shape_jimothy_10_04() {
    adl_shape_replay(3, 1, 0.706, 0.661);
}

// ======================================== L2 conservation property (proptest) ===
//
// Random matched book, a random unilateral exit that ADLs one side, random price path, then a
// wind-down of a random side. Properties, on every case:
//   P1 the wind-down always exits ADL in at most (#legs on the wound side) steps;
//   P2 no step ever opens or enlarges a position, and opens stay refused until both resets
//      finalize;
//   P3 conservation: with every account settled before and after, vault and
//      insurance + sum(capital + pnl) are EXACTLY unchanged by the wind-down;
//   P4 the engine's own shape/conservation validation passes after every step.

use proptest::prelude::*;

fn all_refresh(header: &mut Header, markets: &mut Markets, accts: &mut [PortfolioAccountV16Account]) {
    for a in accts.iter_mut() {
        refresh(header, markets, a);
    }
}

fn abs_leg(a: &PortfolioAccountV16Account) -> u128 {
    let l = a.legs[0].try_to_runtime().unwrap();
    if l.active {
        l.basis_pos_q.unsigned_abs()
    } else {
        0
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn l2_prop_wind_down_always_exits_and_conserves_value(
        sizes_l in prop::collection::vec(1u128..=6, 1..=4),
        sizes_s in prop::collection::vec(1u128..=6, 1..=4),
        exit_tenths in 1u128..=9,
        moves in prop::collection::vec(-150_000i64..=150_000, 0..=3),
        wind_long in any::<bool>(),
    ) {
        let (mut header, mut markets) = market_fixture_slots(1, PRICE);
        let n_l = sizes_l.len();
        let n_s = sizes_s.len();
        let tot_l: u128 = sizes_l.iter().sum();
        let tot_s: u128 = sizes_s.iter().sum();
        // Accounts: longs, shorts, then one balancing account on the lighter side.
        let mut accts: Vec<PortfolioAccountV16Account> =
            (0..n_l + n_s + 1).map(|i| account_fixture(1, 110 + i as u8)).collect();
        for a in accts.iter_mut() {
            deposit(&mut header, &mut markets, a, 50_000_000_000);
        }
        let mut rem_l: Vec<u128> = sizes_l.iter().map(|x| x * LOT).collect();
        let mut rem_s: Vec<u128> = sizes_s.iter().map(|x| x * LOT).collect();
        let bal = n_l + n_s;
        if tot_l > tot_s { rem_s.push((tot_l - tot_s) * LOT); } else { rem_l.push((tot_s - tot_l) * LOT); }
        let long_idx: Vec<usize> = (0..n_l).chain(if tot_l <= tot_s { Some(bal) } else { None }).collect();
        let short_idx: Vec<usize> = (n_l..n_l + n_s).chain(if tot_l > tot_s { Some(bal) } else { None }).collect();
        let (mut i, mut j) = (0usize, 0usize);
        while i < long_idx.len() && j < short_idx.len() {
            let q = rem_l[i].min(rem_s[j]);
            if q > 0 {
                let (li, si) = (long_idx[i], short_idx[j]);
                let (lo, hi) = accts.split_at_mut(li.max(si));
                let (la, sa) = if li < si { (&mut lo[li], &mut hi[0]) } else { (&mut hi[0], &mut lo[si]) };
                trade(&mut header, &mut markets, la, sa, signed_q(q), PRICE).unwrap();
                rem_l[i] -= q;
                rem_s[j] -= q;
            }
            if rem_l[i] == 0 { i += 1; }
            if rem_s[j] == 0 { j += 1; }
        }
        // A unilateral exit (tag 44) by the first long: ADLs the short side.
        let reduce = abs_leg(&accts[long_idx[0]]) * exit_tenths / 10;
        if reduce > 0 {
            let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
            let mut v = PortfolioV16ViewMut::new(&mut accts[long_idx[0]]);
            market
                .rebalance_reduce_position_not_atomic(&mut v, RebalanceRequestV16 { asset_index: 0, reduce_q: reduce })
                .unwrap();
        }
        // Price path.
        let mut slot = header.current_slot.get();
        let mut px = PRICE as i64;
        for m in &moves {
            slot += 1;
            px = (px + m).max(100_000);
            accrue(&mut header, &mut markets, slot, px as u64);
            all_refresh(&mut header, &mut markets, &mut accts);
        }
        let a0 = markets[0].engine.asset.try_to_runtime().unwrap();
        prop_assume!(a0.a_long != ADL_ONE || a0.a_short != ADL_ONE);
        all_refresh(&mut header, &mut markets, &mut accts);
        let vault0 = header.vault.get();
        let refs: Vec<&PortfolioAccountV16Account> = accts.iter().collect();
        let value0 = total_value(&header, &refs);
        let side = if wind_long { percolator::SideV16::Long } else { percolator::SideV16::Short };
        let wound: Vec<usize> = (0..accts.len())
            .filter(|&k| {
                let l = accts[k].legs[0].try_to_runtime().unwrap();
                l.active && l.side == side
            })
            .collect();
        let mut fresh_l = account_fixture(1, 250);
        let mut fresh_s = account_fixture(1, 251);
        deposit(&mut header, &mut markets, &mut fresh_l, 10_000_000_000);
        deposit(&mut header, &mut markets, &mut fresh_s, 10_000_000_000);
        let vault0 = vault0 + 20_000_000_000;

        let mut cleared = false;
        let mut steps = 0usize;
        for &k in &wound {
            if cleared { break; }
            let before = abs_leg(&accts[k]);
            let others_before: Vec<u128> = accts.iter().map(abs_leg).collect();
            match wind_down(&mut header, &mut markets, &mut accts[k], AdlWindDownBoundV16::EpisodeExpired) {
                Ok((closed, c)) => {
                    steps += 1;
                    prop_assert!(closed > 0);
                    cleared = c;
                }
                Err(e) => prop_assert!(false, "wind-down step refused: {e:?}"),
            }
            // P2: no position grew.
            prop_assert!(abs_leg(&accts[k]) < before || before == 0);
            for (idx, a) in accts.iter().enumerate() {
                prop_assert!(abs_leg(a) <= others_before[idx]);
            }
            // P2: opens refused while ADL or the reset is pending.
            prop_assert_eq!(
                trade(&mut header, &mut markets, &mut fresh_l, &mut fresh_s, signed_q(POS_SCALE), px as u64),
                Err(V16Error::AdlReduceOnly)
            );
            // P4
            let market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
            prop_assert_eq!(market.validate_shape(), Ok(()));
        }
        // P1
        prop_assert!(cleared, "ADL must clear within {} steps", wound.len());
        prop_assert!(steps <= wound.len());
        all_refresh(&mut header, &mut markets, &mut accts);
        {
            let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
            for s in [percolator::SideV16::Long, percolator::SideV16::Short] {
                let _ = market.finalize_side_reset_not_atomic(0, s);
            }
            prop_assert_eq!(market.validate_shape(), Ok(()));
        }
        let a = markets[0].engine.asset.try_to_runtime().unwrap();
        prop_assert_eq!((a.a_long, a.a_short), (ADL_ONE, ADL_ONE));
        prop_assert_eq!((a.mode_long, a.mode_short), (SideModeV16::Normal, SideModeV16::Normal));
        prop_assert_eq!((a.oi_eff_long_q, a.oi_eff_short_q), (0, 0));
        // P3
        prop_assert_eq!(header.vault.get(), vault0);
        let refs: Vec<&PortfolioAccountV16Account> = accts.iter().collect();
        prop_assert_eq!(total_value(&header, &refs), value0);
        // Reopens.
        prop_assert_eq!(
            trade(&mut header, &mut markets, &mut fresh_l, &mut fresh_s, signed_q(POS_SCALE), px as u64),
            Ok(())
        );
    }
}

// ================================ security-review follow-ups (M-1 engine half, L-3) ===

#[test]
fn m1_wind_down_refuses_while_the_mark_lags_the_oracle_target() {
    let (mut header, mut markets, _l1, mut l2, _s1, _s2) = adl_world();
    // Oracle target moved, effective price has not caught up yet: refused.
    markets[0].engine.asset.raw_oracle_target_price = V16PodU64::new(101);
    assert_eq!(
        wind_down(&mut header, &mut markets, &mut l2, AdlWindDownBoundV16::EpisodeExpired),
        Err(V16Error::LockActive),
        "no forced close at a lagging mark"
    );
    assert!(l2.legs[0].try_to_runtime().unwrap().active);
    // NEGATIVE CONTROL: target == effective, the same call closes.
    markets[0].engine.asset.raw_oracle_target_price = V16PodU64::new(100);
    wind_down(&mut header, &mut markets, &mut l2, AdlWindDownBoundV16::EpisodeExpired)
        .expect("fresh mark: wind-down proceeds");
}

#[test]
fn l3_mh_wind_down_refuses_while_a_domain_loss_barrier_is_pending() {
    for side_short in [false, true] {
        let (mut header, mut markets, _l1, mut l2, _s1, _s2) = adl_world();
        if side_short {
            markets[0].engine.pending_domain_loss_barrier_short = V16PodU64::new(1);
        } else {
            markets[0].engine.pending_domain_loss_barrier_long = V16PodU64::new(1);
        }
        assert_eq!(
            wind_down(&mut header, &mut markets, &mut l2, AdlWindDownBoundV16::EpisodeExpired),
            Err(V16Error::LockActive),
            "a forced close must not run over an in-flight bankruptcy close (short={side_short})"
        );
    }
    // NEGATIVE CONTROL: no barrier, same world, closes.
    let (mut header, mut markets, _l1, mut l2, _s1, _s2) = adl_world();
    wind_down(&mut header, &mut markets, &mut l2, AdlWindDownBoundV16::EpisodeExpired)
        .expect("no barrier: wind-down proceeds");
}

#[test]
fn l3_mg_flat_account_without_a_close_ledger_stays_unattributed() {
    // Single-asset Live group (attribution would otherwise be written). A FLAT account with a
    // deficit and no open close ledger has no bankrupt side to name: it must be recorded
    // unattributed (byte 1, global predicate), never guessed onto a domain.
    let (mut header, mut markets) = market_fixture_slots(1, 100);
    let mut a = account_fixture(1, 66);
    a.pnl = percolator::V16PodI128::new(-50);
    header.negative_pnl_account_count =
        V16PodU64::new(header.negative_pnl_account_count.get() + 1);
    assert!(!a.legs[0].try_to_runtime().unwrap().active);
    assert!(!a.close_progress.try_to_runtime().unwrap().active);
    {
        let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut v = PortfolioV16ViewMut::new(&mut a);
        let _ = market.full_account_refresh_not_atomic(&mut v);
    }
    assert_eq!(
        header.bankruptcy_hlock_active, 1,
        "flat, no ledger: unattributed (not domain 0 or 1)"
    );
    // NEGATIVE CONTROL: the same deficit on an account WITH a short leg is attributed to
    // domain 1 (asset 0, Short) -- the mark helper is not simply always-unattributed.
    let (mut header, mut markets) = market_fixture_slots(1, 100);
    let mut l = account_fixture(1, 67);
    let mut s = account_fixture(1, 68);
    deposit(&mut header, &mut markets, &mut l, 10_000);
    deposit(&mut header, &mut markets, &mut s, 10_000);
    trade(&mut header, &mut markets, &mut l, &mut s, signed_q(POS_SCALE), 100).unwrap();
    s.capital = percolator::V16PodU128::new(0);
    header.c_tot = percolator::V16PodU128::new(header.c_tot.get() - 10_000);
    header.vault = percolator::V16PodU128::new(header.vault.get() - 10_000);
    s.pnl = percolator::V16PodI128::new(-50);
    header.negative_pnl_account_count =
        V16PodU64::new(header.negative_pnl_account_count.get() + 1);
    {
        let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut v = PortfolioV16ViewMut::new(&mut s);
        let _ = market.full_account_refresh_not_atomic(&mut v);
    }
    assert_eq!(header.bankruptcy_hlock_active, 0b101, "short leg: attributed to domain 1");
}
