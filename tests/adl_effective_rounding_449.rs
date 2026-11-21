//! Issue percolator-prog#449: per-leg ADL-effective quantities are ceilings,
//! so after a quantity ADL the sum of a side's leg ceilings can exceed the
//! side's aggregate effective OI. These tests pin what the engine does with
//! that surplus on every exit route.
//!
//! The first two were written against the engine before the fix and pass
//! unchanged. The third characterised the defect: the surplus grew with every
//! same-side reduction. It now pins the fix: each side keeps the sum of its
//! legs' raw basis and derives its A index from it, so the surplus stays
//! below the leg count and the exact exposure never exceeds the effective OI.
//! The last two pin what keeps health certificates sound under the fix:
//! an index that rises while other legs share the side advances the risk epoch,
//! and the certificate issued after a fill describes the position the fill left.
//!
//! Adapted from aeyakovenko/percolator 85b923a6 and the post-fix version in
//! c030aa40 (PR #270). Only change: the trade call passes our extra
//! `taker_is_long_account` argument. Assertions are upstream's, unchanged.
//! The last test is a dcccrypto addition (from PR #381) that checks the
//! invariant directly after every op.

use percolator::{
    active_bitmap_is_empty, AssetStateV16, EngineAssetSlotV16Account, Market,
    MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, PortfolioAccountV16Account,
    PortfolioV16ViewMut, ProvenanceHeaderV16, ProvenanceHeaderV16Account, RebalanceRequestV16,
    SideV16, TradeRequestV16, V16Config, V16PodU64, ADL_ONE, POS_SCALE,
};

const PRICE: u64 = 1_000_000;

fn market() -> (MarketGroupV16HeaderAccount, Vec<Market<u64>>) {
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    cfg.max_price_move_bps_per_slot = 9_000;
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut markets = vec![Market::new(0, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, PRICE, 1)
        .unwrap();
    (header, markets)
}

fn account(seed: u8) -> PortfolioAccountV16Account {
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        [1; 32], [seed; 32], [3; 32],
    ));
    let mut account = PortfolioAccountV16Account::default();
    account.init_empty_in_place(header).unwrap();
    account
}

fn asset(markets: &[Market<u64>]) -> AssetStateV16 {
    markets[0].engine.asset.try_to_runtime().unwrap()
}

fn raw_and_basis(acct: &PortfolioAccountV16Account) -> Option<(SideV16, u128, u128, u64)> {
    let leg = acct.legs[0].try_to_runtime().unwrap();
    leg.active.then(|| {
        (
            leg.side,
            leg.basis_pos_q.unsigned_abs(),
            leg.a_basis,
            leg.epoch_snap,
        )
    })
}

fn ceil_eff(raw: u128, a_basis: u128, a: u128) -> u128 {
    (raw * a + a_basis - 1) / a_basis
}

/// Per side: (sum of current-epoch leg ceilings, exact sum numerator over ADL_ONE)
fn census(markets: &[Market<u64>], accts: &[PortfolioAccountV16Account]) -> [(u128, u128); 2] {
    let a = asset(markets);
    let mut out = [(0u128, 0u128); 2];
    for acct in accts {
        if let Some((side, raw, a_basis, epoch)) = raw_and_basis(acct) {
            let (cur_a, cur_epoch, i) = match side {
                SideV16::Long => (a.a_long, a.epoch_long, 0),
                SideV16::Short => (a.a_short, a.epoch_short, 1),
            };
            if epoch != cur_epoch {
                continue;
            }
            assert_eq!(a_basis, ADL_ONE, "test universe opens every leg at unit A");
            out[i].0 += ceil_eff(raw, a_basis, cur_a);
            out[i].1 += raw * cur_a; // exact effective * ADL_ONE
        }
    }
    out
}

fn trade(
    header: &mut MarketGroupV16HeaderAccount,
    markets: &mut [Market<u64>],
    buyer: &mut PortfolioAccountV16Account,
    seller: &mut PortfolioAccountV16Account,
    q: u128,
) -> Result<(), percolator::V16Error> {
    let mut market = MarketGroupV16ViewMut::new(header, markets);
    let mut b = PortfolioV16ViewMut::new(buyer);
    let mut s = PortfolioV16ViewMut::new(seller);
    market
        .execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut b,
            &mut s,
            TradeRequestV16 {
                asset_index: 0,
                size_q: i128::try_from(q).unwrap(),
                exec_price: PRICE,
                fee_bps: 0,
            },
            // dcccrypto v2.2 adaptation: extra taker flag; fee_bps is 0 so it
            // has no economic effect here.
            true,
        )
        .map(|_| ())
}

fn rebalance(
    header: &mut MarketGroupV16HeaderAccount,
    markets: &mut [Market<u64>],
    acct: &mut PortfolioAccountV16Account,
    q: u128,
) -> Result<u128, percolator::V16Error> {
    let mut market = MarketGroupV16ViewMut::new(header, markets);
    let mut a = PortfolioV16ViewMut::new(acct);
    market
        .rebalance_reduce_position_not_atomic(
            &mut a,
            RebalanceRequestV16 {
                asset_index: 0,
                reduce_q: q,
            },
        )
        .map(|o| o.reduced_q)
}

fn deposit(
    header: &mut MarketGroupV16HeaderAccount,
    markets: &mut [Market<u64>],
    acct: &mut PortfolioAccountV16Account,
) {
    let mut market = MarketGroupV16ViewMut::new(header, markets);
    let mut a = PortfolioV16ViewMut::new(acct);
    market
        .deposit_not_atomic(&mut a, 1_000_000_000_000)
        .unwrap();
}

/// Longs L1=500_000, L2=250_000 against one short S=750_000; S unilaterally
/// reduces 500_000 => a_long = floor(1e15/3), OI 250_000/250_000, leg
/// ceilings 166_667 + 83_334 = 250_001 (the #449 state).
fn issue_state() -> (
    MarketGroupV16HeaderAccount,
    Vec<Market<u64>>,
    Vec<PortfolioAccountV16Account>,
) {
    let (mut header, mut markets) = market();
    let mut accts = vec![account(10), account(11), account(12)];
    for a in accts.iter_mut() {
        deposit(&mut header, &mut markets, a);
    }
    let (l, s) = accts.split_at_mut(2);
    trade(&mut header, &mut markets, &mut l[0], &mut s[0], 500_000).unwrap();
    trade(&mut header, &mut markets, &mut l[1], &mut s[0], 250_000).unwrap();
    assert_eq!(
        rebalance(&mut header, &mut markets, &mut s[0], 500_000).unwrap(),
        500_000
    );
    (header, markets, accts)
}

#[test]
fn issue_449_reproduces_ceiling_surplus_without_engine_oi_mismatch() {
    let (_header, markets, accts) = issue_state();
    let a = asset(&markets);
    assert_eq!(a.a_long, ADL_ONE / 3);
    assert_eq!(a.oi_eff_long_q, 250_000);
    assert_eq!(a.oi_eff_short_q, 250_000);
    let c = census(&markets, &accts);
    assert_eq!(c[0].0, 166_667 + 83_334);
    assert_eq!(c[1].0, 250_000);
    // Exact (un-rounded) long exposure stays at or below aggregate OI here.
    assert!(c[0].1 <= a.oi_eff_long_q * ADL_ONE);
}

#[derive(Clone, Copy, Debug)]
enum Exit {
    Rebalance,
    TradeWithShort,
}

/// Close each long in the given order; return per-step errors and final state.
fn exit_all(order: [usize; 2], how: Exit) {
    let (mut header, mut markets, mut accts) = issue_state();
    for &i in order.iter() {
        let eff = {
            let a = asset(&markets);
            let (_, raw, ab, _) = raw_and_basis(&accts[i]).unwrap();
            ceil_eff(raw, ab, a.a_long)
        };
        let (l, s) = accts.split_at_mut(2);
        match how {
            Exit::Rebalance => {
                let got = rebalance(&mut header, &mut markets, &mut l[i], u128::MAX >> 8)
                    .unwrap_or_else(|e| panic!("{order:?} {how:?} leg {i}: {e:?}"));
                let a = asset(&markets);
                assert!(got <= eff);
                assert_eq!(a.oi_eff_long_q, a.oi_eff_short_q);
            }
            Exit::TradeWithShort => {
                // Short S buys back from long i: capped by S's effective size,
                // since S may not flip while A != ADL_ONE.
                let a = asset(&markets);
                let s_eff = raw_and_basis(&s[0])
                    .map(|(_, raw, ab, _)| ceil_eff(raw, ab, a.a_short))
                    .unwrap_or(0);
                let q = eff.min(s_eff).min(a.oi_eff_long_q);
                if q > 0 {
                    trade(&mut header, &mut markets, &mut s[0], &mut l[i], q)
                        .unwrap_or_else(|e| panic!("{order:?} {how:?} leg {i}: {e:?}"));
                }
                // One unit beyond aggregate OI is rejected, never saturated.
                let a = asset(&markets);
                if let Some((_, raw, ab, ep)) = raw_and_basis(&l[i]) {
                    if ep == a.epoch_long && ceil_eff(raw, ab, a.a_long) > a.oi_eff_long_q {
                        assert!(trade(&mut header, &mut markets, &mut s[0], &mut l[i], 1).is_err());
                    }
                }
            }
        }
    }
    // OI must be exhausted on both sides and any residue reset, not stranded.
    let a = asset(&markets);
    assert_eq!(
        (a.oi_eff_long_q, a.oi_eff_short_q),
        (0, 0),
        "{order:?} {how:?}"
    );
    for acct in accts.iter_mut() {
        let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut v = PortfolioV16ViewMut::new(acct);
        market.full_account_refresh_not_atomic(&mut v).unwrap();
        assert!(
            active_bitmap_is_empty(v.header.active_bitmap.map(V16PodU64::get)),
            "{order:?} {how:?}: residue leg not cleared by refresh"
        );
        // flat price, zero fees: nobody gains or loses value
        assert_eq!(v.header.pnl.get(), 0, "{order:?} {how:?}");
        assert_eq!(
            v.header.capital.get(),
            1_000_000_000_000,
            "{order:?} {how:?}"
        );
    }
    let market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    market.validate_shape().unwrap();
}

#[test]
fn issue_449_every_exit_order_and_route_drains_exactly() {
    for order in [[0, 1], [1, 0]] {
        exit_all(order, Exit::Rebalance);
        exit_all(order, Exit::TradeWithShort);
    }
}

/// Repeated unilateral ADL rounds interleaved with 1-unit bilateral long
/// reductions: the history that made the surplus compound before the fix.
/// The ceiling surplus D = sum(ceil) - OI and the exact surplus
/// G = sum(exact) - OI were then bounded only by the number of same-side
/// reducing operations (D reached 2001 with four legs, G 1999.33 raw units),
/// because a resize kept the largest raw basis whose own ceiling equalled the
/// target and so gave up less exposure than the effective OI it removed.
///
/// With A derived from the side's totals after every operation, and a resize
/// giving up at least the raw basis that carries the removed effective OI:
/// - G is never positive, and is short of zero by less than
///   `raw_basis_sum / ADL_ONE` raw units (the floor of the derived A);
/// - D is never negative and stays below the number of legs;
/// - a 10% move creates no equity in the closed universe.
#[test]
fn issue_449_resize_rounding_surplus_stays_below_the_leg_count() {
    const N: usize = 4;
    let (mut header, mut markets) = market();
    // accts[0..N] longs, accts[N] = ADL'ing short, accts[N+1] = short reducer
    let mut accts: Vec<_> = (0..N + 2).map(|i| account(20 + i as u8)).collect();
    for a in accts.iter_mut() {
        deposit(&mut header, &mut markets, a);
    }
    {
        let (l, s) = accts.split_at_mut(N);
        for (i, leg) in l.iter_mut().enumerate() {
            let q = 1_000 * POS_SCALE + 7_777 * (i as u128 + 1);
            trade(&mut header, &mut markets, leg, &mut s[0], q / 2).unwrap();
            trade(&mut header, &mut markets, leg, &mut s[1], q - q / 2).unwrap();
        }
    }
    // One large ADL first so A/a_basis is well below 1 (coarse raw grid).
    {
        let (_, s) = accts.split_at_mut(N);
        let s0_eff = raw_and_basis(&s[0]).unwrap().1;
        rebalance(&mut header, &mut markets, &mut s[0], s0_eff * 2 / 3).unwrap();
    }
    let mut max_d = 0u128;
    let mut min_g_num = 0i128;
    let rounds = 2_000u128;
    for _ in 0..rounds {
        let (l, s) = accts.split_at_mut(N);
        rebalance(&mut header, &mut markets, &mut s[0], 1).unwrap();
        for leg in l.iter_mut() {
            trade(&mut header, &mut markets, &mut s[1], leg, 1).unwrap();
        }
        let a = asset(&markets);
        assert_eq!(a.oi_eff_long_q, a.oi_eff_short_q);
        let c = census(&markets, &accts);
        // the engine's own total is the sum of the legs' raw basis
        let raw_sum: u128 = accts[..N]
            .iter()
            .filter_map(raw_and_basis)
            .map(|(_, raw, _, _)| raw)
            .sum();
        assert_eq!(a.raw_basis_sum_long_q, raw_sum);
        assert_eq!(a.a_long, ADL_ONE * a.oi_eff_long_q / raw_sum);
        // exact long exposure: never above the OI, short of it by less than
        // raw_sum / ADL_ONE raw units
        let g = c[0].1 as i128 - (a.oi_eff_long_q * ADL_ONE) as i128;
        assert!(g <= 0 && g > -(raw_sum as i128), "G_num = {g}");
        // ceilings: cover the OI, exceed it by less than one per leg
        assert!(c[0].0 >= a.oi_eff_long_q, "ceilings below the long OI");
        let d = c[0].0 - a.oi_eff_long_q;
        assert!(d < N as u128, "D = {d}");
        max_d = max_d.max(d);
        min_g_num = min_g_num.min(g);
        // short side is never haircut here, so its legs are exact
        assert_eq!(c[1].0, a.oi_eff_short_q);
        assert_eq!(a.a_short, ADL_ONE);
    }
    let a = asset(&markets);
    let c = census(&markets, &accts);
    let g_num = c[0].1 as i128 - (a.oi_eff_long_q * ADL_ONE) as i128;
    eprintln!(
        "rounds={rounds} n={N} a_long={} oi={} final D={} max D={} min G={:.6} final G={:.6}",
        a.a_long,
        a.oi_eff_long_q,
        c[0].0 - a.oi_eff_long_q,
        max_d,
        min_g_num as f64 / ADL_ONE as f64,
        g_num as f64 / ADL_ONE as f64,
    );

    // Value consequence: the matched book stays zero-sum. Move the price up
    // 10%; the longs' aggregate gain cannot exceed the shorts' loss, and each
    // account's settlement rounds against it by less than one atom.
    let new_price = PRICE + PRICE / 10;
    {
        let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        market
            .accrue_asset_to_not_atomic(0, 2, new_price, 0, true)
            .unwrap();
        market.markets[0].engine.asset.raw_oracle_target_price = V16PodU64::new(new_price);
    }
    let mut sum_pnl = 0i128;
    for acct in accts.iter_mut() {
        let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut v = PortfolioV16ViewMut::new(acct);
        market.full_account_refresh_not_atomic(&mut v).unwrap();
        sum_pnl += v.header.pnl.get() + v.header.capital.get() as i128 - 1_000_000_000_000;
    }
    eprintln!("sum (capital+pnl-deposit) after +10% = {sum_pnl}");
    assert!(
        sum_pnl <= 0 && sum_pnl > -(N as i128 + 2),
        "closed universe after +10%: {sum_pnl} atoms"
    );
}

fn long_effective(markets: &[Market<u64>], acct: &PortfolioAccountV16Account) -> u128 {
    let (_, raw, a_basis, _) = raw_and_basis(acct).expect("long leg");
    ceil_eff(raw, a_basis, asset(markets).a_long)
}

/// Whether the account's certificate is still current by its risk epoch. A certificate that is
/// current must be no healthier than a full refresh of the same account in the same state.
fn current_certificate_is_conservative(
    header: &MarketGroupV16HeaderAccount,
    markets: &[Market<u64>],
    acct: &PortfolioAccountV16Account,
) -> bool {
    let cached = acct.health_cert.try_to_runtime().unwrap();
    if !cached.valid || cached.cert_risk_epoch != header.risk_epoch.get() {
        return false;
    }
    let mut header_copy = header.clone();
    let mut markets_copy = markets.to_vec();
    let mut acct_copy = acct.clone();
    let mut market = MarketGroupV16ViewMut::new(&mut header_copy, &mut markets_copy);
    let mut view = PortfolioV16ViewMut::new(&mut acct_copy);
    let full = market.full_account_refresh_not_atomic(&mut view).unwrap();
    assert!(
        cached.certified_initial_req >= full.certified_initial_req
            && cached.certified_maintenance_req >= full.certified_maintenance_req
            && cached.certified_worst_case_loss >= full.certified_worst_case_loss
            && cached.certified_equity <= full.certified_equity,
        "a current certificate is healthier than the account's state: {cached:?} against {full:?}"
    );
    true
}

/// A reduction on a haircut side is rounded against the reducing leg, so the
/// side's A index can rise, and with it the effective quantity of the side's
/// other legs, by a unit and without any action of their owners. A health
/// certificate issued before the rise would understate their requirement, so
/// the rise advances the risk epoch and no earlier certificate is current.
/// A falling index needs no new epoch, and neither does a leg that is alone on
/// its side: its own account is certified again by the route that changed it.
#[test]
fn issue_449_an_index_rise_retires_earlier_certificates() {
    // Two longs share the haircut side; L1 gives up one unit at a time. A further haircut of
    // an odd size first takes the side off the exact 1:3 ratio of `issue_state`, where a
    // reduction gives up whole raw units and nothing is rounded.
    let (mut header, mut markets, mut accts) = issue_state();
    {
        let (_, s) = accts.split_at_mut(2);
        assert_eq!(
            rebalance(&mut header, &mut markets, &mut s[0], 12_345).unwrap(),
            12_345
        );
    }
    let mut index_rises = 0u32;
    let mut peer_quantity_rises = 0u32;
    for step in 0..2_000 {
        // The peer holds an exact, current certificate before the other long reduces.
        {
            let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
            let mut peer = PortfolioV16ViewMut::new(&mut accts[1]);
            market.full_account_refresh_not_atomic(&mut peer).unwrap();
        }
        let a_before = asset(&markets).a_long;
        let epoch_before = header.risk_epoch.get();
        let peer_before = long_effective(&markets, &accts[1]);
        {
            let (l, s) = accts.split_at_mut(2);
            trade(&mut header, &mut markets, &mut s[0], &mut l[0], 1).unwrap();
        }
        let a_after = asset(&markets).a_long;
        let rose = a_after > a_before;
        assert_eq!(
            header.risk_epoch.get() > epoch_before,
            rose,
            "step {step}: index {a_before} -> {a_after}, risk epoch {epoch_before} -> {}",
            header.risk_epoch.get()
        );
        // The peer did nothing. Its effective quantity moves up only with the index, and
        // every such move is covered by a new risk epoch: its certificate is then no longer
        // current. While the certificate is still current it is no healthier than a full
        // refresh of the same account.
        let peer_after = long_effective(&markets, &accts[1]);
        let current = current_certificate_is_conservative(&header, &markets, &accts[1]);
        assert_eq!(current, !rose, "step {step}");
        if peer_after > peer_before {
            assert!(rose && peer_after == peer_before + 1, "step {step}");
            peer_quantity_rises += 1;
        }
        index_rises += u32::from(rose);
    }
    assert!(index_rises > 0 && peer_quantity_rises > 0);

    // A haircut lowers the index. The peer's certificate, exact before it, stays current and
    // is only more conservative afterwards: no new epoch.
    {
        let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut peer = PortfolioV16ViewMut::new(&mut accts[1]);
        market.full_account_refresh_not_atomic(&mut peer).unwrap();
    }
    let a_before = asset(&markets).a_long;
    let epoch_before = header.risk_epoch.get();
    {
        let (_, s) = accts.split_at_mut(2);
        assert_eq!(
            rebalance(&mut header, &mut markets, &mut s[0], 1_000).unwrap(),
            1_000
        );
    }
    assert!(asset(&markets).a_long < a_before);
    assert_eq!(header.risk_epoch.get(), epoch_before);
    assert!(current_certificate_is_conservative(
        &header, &markets, &accts[1]
    ));

    // One long alone on its haircut side: its index rises as it reduces, and no other leg
    // is there to be affected.
    let (mut header, mut markets) = market();
    let mut accts = vec![account(40), account(41)];
    for a in accts.iter_mut() {
        deposit(&mut header, &mut markets, a);
    }
    let mut sole_rises = 0u32;
    {
        let (l, s) = accts.split_at_mut(1);
        trade(&mut header, &mut markets, &mut l[0], &mut s[0], 750_000).unwrap();
        rebalance(&mut header, &mut markets, &mut s[0], 512_345).unwrap();
        let epoch_before = header.risk_epoch.get();
        for _ in 0..500 {
            let a_before = asset(&markets).a_long;
            trade(&mut header, &mut markets, &mut s[0], &mut l[0], 1).unwrap();
            sole_rises += u32::from(asset(&markets).a_long > a_before);
        }
        assert_eq!(header.risk_epoch.get(), epoch_before);
    }
    assert!(sole_rises > 0);
    eprintln!(
        "index rises with a peer: {index_rises}, peer quantity rises: {peer_quantity_rises}, \
         sole-leg index rises: {sole_rises}"
    );
}

/// A single-fill trade certifies both accounts incrementally, from the position each leg had
/// and the position it has now. On a haircut side the reducing leg can end below the quantity
/// the fill asked for, so the certificate has to be built from the leg as stored: it must equal
/// a full refresh of the same account in the same state.
#[test]
fn issue_449_a_fill_certifies_the_position_it_left() {
    let (mut header, mut markets, mut accts) = issue_state();
    {
        let (_, s) = accts.split_at_mut(2);
        assert_eq!(
            rebalance(&mut header, &mut markets, &mut s[0], 12_345).unwrap(),
            12_345
        );
    }
    // Haircuts leave certificates alone (they only got more conservative), so the two accounts'
    // certificates still describe their opening positions. Refresh them: from here on each
    // fill builds its certificate from an exact one.
    for who in [0, 2] {
        let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut view = PortfolioV16ViewMut::new(&mut accts[who]);
        market.full_account_refresh_not_atomic(&mut view).unwrap();
    }
    let mut below_request = 0u32;
    for step in 0..500 {
        let before = long_effective(&markets, &accts[0]);
        {
            let (l, s) = accts.split_at_mut(2);
            trade(&mut header, &mut markets, &mut s[0], &mut l[0], 1).unwrap();
        }
        let after = long_effective(&markets, &accts[0]);
        assert!(
            after < before && before - after <= 3,
            "step {step}: {before} -> {after}"
        );
        below_request += u32::from(before - after > 1);
        for who in [0, 2] {
            let stored = accts[who].health_cert.try_to_runtime().unwrap();
            let mut header_copy = header.clone();
            let mut markets_copy = markets.clone();
            let mut account_copy = accts[who].clone();
            let mut market = MarketGroupV16ViewMut::new(&mut header_copy, &mut markets_copy);
            let mut view = PortfolioV16ViewMut::new(&mut account_copy);
            let full = market.full_account_refresh_not_atomic(&mut view).unwrap();
            assert_eq!(stored, full, "step {step}: account {who} after the fill");
        }
    }
    assert!(
        below_request > 0,
        "no fill left the reducer below its request"
    );
}

/// dcccrypto addition (not upstream, from PR #381): the invariant the post-fix
/// engine (upstream c030aa40) holds, checked directly after every op. With
/// `R` the side's current-epoch raw basis sum and G the exact-over-OI residue
/// `sum(raw * A) - oi_eff * ADL_ONE`:
///   (1) per side, exact exposure sum(raw * A / a_basis) <= oi_eff (G <= 0)
///   (2) per side, the exposure is short of oi_eff by less than R / ADL_ONE
///       raw units (G > -R): the floor of the derived A is the only gap
///   (3) on a side the op reduced, G rises by less than the pre-op R (less
///       than R / ADL_ONE raw units): the re-derived floor can move G up
///       inside (-R, 0], but no op moves a raw-grid step of exposure into it
/// On the engine before the port it fails at the first 1-unit long reduce
/// after the ADL ("side 0 residue grew by 0.333333 raw units in one op", far
/// above the R / ADL_ONE window of about 0.000004 raw units here).
#[test]
fn issue_449_invariant_exact_exposure_within_oi_eff_and_no_per_op_gain() {
    const N: usize = 4;
    let (mut header, mut markets) = market();
    let mut accts: Vec<_> = (0..N + 2).map(|i| account(60 + i as u8)).collect();
    for a in accts.iter_mut() {
        deposit(&mut header, &mut markets, a);
    }
    {
        let (l, s) = accts.split_at_mut(N);
        for (i, leg) in l.iter_mut().enumerate() {
            let q = 1_000 * POS_SCALE + 7_777 * (i as u128 + 1);
            trade(&mut header, &mut markets, leg, &mut s[0], q / 2).unwrap();
            trade(&mut header, &mut markets, leg, &mut s[1], q - q / 2).unwrap();
        }
    }
    // `reduced[s]`: the op reduced a leg on side s, so (3) applies to it. A
    // passive (ADL'd) side is held to (1) and (2) only. `prev` is the previous
    // check's (G, R) per side.
    let check = |markets: &[Market<u64>],
                 accts: &[PortfolioAccountV16Account],
                 prev: ([i128; 2], [u128; 2]),
                 reduced: [bool; 2],
                 op: &str| {
        let a = asset(markets);
        let c = census(markets, accts);
        let oi = [a.oi_eff_long_q, a.oi_eff_short_q];
        let g = [0, 1].map(|s| c[s].1 as i128 - (oi[s] * ADL_ONE) as i128);
        // current-epoch raw basis per side, from the legs themselves
        let mut r = [0u128; 2];
        for acct in accts {
            if let Some((side, raw, _, epoch)) = raw_and_basis(acct) {
                let (cur_epoch, i) = match side {
                    SideV16::Long => (a.epoch_long, 0),
                    SideV16::Short => (a.epoch_short, 1),
                };
                if epoch == cur_epoch {
                    r[i] += raw;
                }
            }
        }
        let (prev_g, prev_r) = prev;
        for s in 0..2 {
            if reduced[s] {
                assert!(
                    g[s] - prev_g[s] < prev_r[s] as i128,
                    "{op}: side {s} residue grew by {:.6} raw units in one op",
                    (g[s] - prev_g[s]) as f64 / ADL_ONE as f64
                );
            }
            assert!(
                g[s] <= 0,
                "{op}: side {s} exact exposure exceeds oi_eff by {:.6} raw units",
                g[s] as f64 / ADL_ONE as f64
            );
            assert!(
                r[s] == 0 || g[s] > -(r[s] as i128),
                "{op}: side {s} exact exposure is {:.6} raw units short of oi_eff",
                -g[s] as f64 / ADL_ONE as f64
            );
        }
        (g, r)
    };
    let mut g = check(&markets, &accts, ([0, 0], [0, 0]), [false, false], "open");
    {
        let (_, s) = accts.split_at_mut(N);
        let s0_raw = raw_and_basis(&s[0]).unwrap().1;
        rebalance(&mut header, &mut markets, &mut s[0], s0_raw * 2 / 3).unwrap();
    }
    g = check(&markets, &accts, g, [false, true], "initial ADL");
    for r in 0..50 {
        {
            let (_, s) = accts.split_at_mut(N);
            rebalance(&mut header, &mut markets, &mut s[0], 1).unwrap();
        }
        g = check(
            &markets,
            &accts,
            g,
            [false, true],
            &format!("round {r} short rebalance"),
        );
        for i in 0..N {
            {
                let (l, s) = accts.split_at_mut(N);
                trade(&mut header, &mut markets, &mut s[1], &mut l[i], 1).unwrap();
            }
            g = check(
                &markets,
                &accts,
                g,
                [true, true],
                &format!("round {r} long {i} reduce"),
            );
        }
    }
}
