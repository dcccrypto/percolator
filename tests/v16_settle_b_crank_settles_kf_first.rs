//! Issue #274: the permissionless crank's B-settlement step must settle the account's pending K/F
//! first, as every other B path does, so the order in which cranks reach a bankrupt loser and its
//! winner cannot change terminal values (spec, K/F settlement and requirement 27).
//!
//! One asset. Account 0 (the loser) is short, account 1 (the winner) is long. The mark rises at
//! the cap until the loser's loss exceeds its deposit; nobody touches an account during the rise.
//! The liquidation books the loser's shortfall into B, so the winner's next crank is planned as a
//! B chunk (B-stale outranks a refresh). Before the fix that chunk landed the winner's B share as a
//! debt ahead of its K/F gain, and the gain then cured the debt from the backing the loser had paid
//! into the winner's source domain. Every case is run in both orders and wound down.
//!
//! Written from the engine source and `tests/kf_world`.

mod kf_world;
use kf_world::*;

const W_DEP: u128 = 10_000_000;
const UNITS: u128 = 9;

fn world(l_dep: u128, provider: u128) -> World {
    let mut w = World::with_config(1, &[l_dep, W_DEP], |cfg| {
        cfg.initial_margin_bps = 1_000;
        cfg.maintenance_margin_bps = 500;
        cfg.max_price_move_bps_per_slot = 400;
        cfg.max_abs_funding_e9_per_slot = 0;
    });
    if provider != 0 {
        // A provider backs the domain the winner's gains are claimed from: the short side's
        // loss domain.
        w.provider_deposit(World::loss_domain(0, false), provider);
    }
    w.trade(1, 0, 0, UNITS);
    let mut px = PX0;
    for _ in 0..4 {
        px = px * 104 / 100;
        w.step(0, px, 0);
    }
    w
}

#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    before_wind_down: Vec<(u128, i128)>,
    junior_residual: i128,
    domain1: (u128, u128),
    wind_down: WindDown,
}

fn run(l_dep: u128, provider: u128, loser_first: bool) -> Outcome {
    let mut w = world(l_dep, provider);
    if loser_first {
        w.crank(0);
        w.crank(1);
    } else {
        w.crank(1);
        w.crank(0);
        w.crank(1);
    }
    // Bring both current again, in a fixed order, before the wind-down.
    w.crank(0);
    w.crank(1);
    let before_wind_down = (0..2)
        .map(|i| (w.accts[i].capital.get(), w.accts[i].pnl.get()))
        .collect();
    let junior_residual = w.junior_residual();
    let domain1 = w.domain(1);
    let wind_down = w.wind_down(&[0, 1]);
    Outcome {
        before_wind_down,
        junior_residual,
        domain1,
        wind_down,
    }
}

fn compare(tag: &str, l_dep: u128, provider: u128) -> (Outcome, Outcome) {
    let winner_first = run(l_dep, provider, false);
    let loser_first = run(l_dep, provider, true);
    println!("{tag}: winner first: {winner_first:?}");
    println!("{tag}: loser first:  {loser_first:?}");
    (winner_first, loser_first)
}

#[test]
fn v16_settle_b_crank_order_cannot_change_terminal_values() {
    let mut differs = Vec::new();
    for (tag, l_dep, provider) in [
        ("solvent control", 2_000_000u128, 0u128),
        ("default, no provider", 1_000_000, 0),
        ("default, provider-backed domain", 1_000_000, 300_000),
        ("small default", 1_400_000, 0),
    ] {
        let (a, b) = compare(tag, l_dep, provider);
        if a != b {
            differs.push(tag);
        }
        // Absolute, not only order-equal: once both accounts are current nothing is unowned,
        // and the wind-down retires nothing (the provider takes its principal back).
        for outcome in [&a, &b] {
            assert_eq!(
                outcome.junior_residual, 0,
                "{tag}: unowned value while Live"
            );
            assert_eq!(
                outcome.wind_down.retired, 0,
                "{tag}: value retired at wind-down"
            );
            assert_eq!(
                outcome.wind_down.vault_end, 0,
                "{tag}: value left in the vault"
            );
            if provider != 0 {
                assert!(
                    outcome.wind_down.provider_out[1] + 1 >= provider,
                    "{tag}: provider short"
                );
            }
        }
    }
    assert!(
        differs.is_empty(),
        "crank order changes the outcome in: {differs:?}"
    );
}

/// Liveness beside the order: the provider's backing on the winner's source domain lapses
/// during the rise, so the winner's K/F settlement meets a lapsed bucket.
fn run_lapsed(loser_first: bool) -> Result<WindDown, String> {
    let mut w = World::with_config(1, &[1_000_000, W_DEP], |cfg| {
        cfg.initial_margin_bps = 1_000;
        cfg.maintenance_margin_bps = 500;
        cfg.max_price_move_bps_per_slot = 400;
        cfg.max_abs_funding_e9_per_slot = 0;
    });
    let domain = World::loss_domain(0, false);
    let expiry = w.slot + 3;
    w.op(|m, _| m.deposit_fresh_counterparty_backing_not_atomic(domain, 300_000, expiry))
        .map_err(|e| format!("provider deposit: {e:?}"))?;
    w.provider[domain] += 300_000;
    w.trade(1, 0, 0, UNITS);
    let mut px = PX0;
    for _ in 0..4 {
        px = px * 104 / 100;
        w.step(0, px, 0);
    }
    let order: &[usize] = if loser_first {
        &[0, 1, 0, 1]
    } else {
        &[1, 0, 1, 0, 1]
    };
    for &i in order {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| w.crank(i)));
        if r.is_err() {
            return Err(format!(
                "crank of account {i} failed (order loser_first={loser_first})"
            ));
        }
    }
    Ok(w.wind_down(&[0, 1]))
}

#[test]
// v2.2 fork: the first crank here is a Refresh, and the provider's bucket sits in the winner's
// latent favorable domain, not its source roster. Expiring latent-domain backing is upstream #243
// (ea8cc9c5) and bf745fd4, which v2.2 rc23 does not have (they are parked in dcccrypto#311). Without
// them the Refresh returns Stale / LockActive in both orders, with or without f1d6b3f7.
#[ignore = "needs upstream #243 ea8cc9c5 latent-domain expiry (not on v2.2 rc23)"]
fn v16_settle_b_crank_with_lapsed_provider_backing_is_order_independent() {
    let winner_first = run_lapsed(false);
    let loser_first = run_lapsed(true);
    println!("lapsed: winner first: {winner_first:?}");
    println!("lapsed: loser first:  {loser_first:?}");
    assert!(winner_first.is_ok() && loser_first.is_ok());
    assert_eq!(winner_first, loser_first);
}

/// The loser's paid-in backing (engine-created, lifetime `h_max`) lapses after the liquidation and
/// before the winner is cranked.
fn run_lapsed_after_liquidation(loser_first: bool) -> Result<WindDown, String> {
    let mut w = World::with_config(1, &[1_000_000, W_DEP], |cfg| {
        cfg.initial_margin_bps = 1_000;
        cfg.maintenance_margin_bps = 500;
        cfg.max_price_move_bps_per_slot = 400;
        cfg.max_abs_funding_e9_per_slot = 0;
        cfg.h_max = 6;
    });
    w.trade(1, 0, 0, UNITS);
    let mut px = PX0;
    for _ in 0..4 {
        px = px * 104 / 100;
        w.step(0, px, 0);
    }
    let crank = |w: &mut World, i: usize| -> Result<(), String> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| w.crank(i))).map_err(|_| {
            format!(
                "crank of account {i} failed at slot {} (loser_first={loser_first})",
                w.slot
            )
        })
    };
    if loser_first {
        crank(&mut w, 0)?;
    } else {
        crank(&mut w, 1)?;
        crank(&mut w, 0)?;
    }
    for _ in 0..10 {
        w.step(0, px, 0);
    }
    crank(&mut w, 1)?;
    crank(&mut w, 0)?;
    crank(&mut w, 1)?;
    Ok(w.wind_down(&[0, 1]))
}

#[test]
fn v16_settle_b_crank_after_the_loser_backing_lapses_keeps_the_winner_whole() {
    let winner_first = run_lapsed_after_liquidation(false);
    let loser_first = run_lapsed_after_liquidation(true);
    println!("lapsed2: winner first: {winner_first:?}");
    println!("lapsed2: loser first:  {loser_first:?}");
    assert!(winner_first.is_ok() && loser_first.is_ok());
    assert_eq!(winner_first, loser_first);
    // The winner's gain (9 x 169,858 = 1,528,722) less its B share of the loser's shortfall
    // (528,722) is exactly the 1,000,000 the loser paid in: 11,000,000 in either order. Before
    // the fix the loser-first order paid the winner 9,471,279 and retired 1,528,721.
    let wind_down = winner_first.unwrap();
    assert_eq!(wind_down.payouts[1], 11_000_000);
    assert_eq!(wind_down.retired, 0);
}

// ---- v2.2 fork addition (not upstream) ----

use percolator::{
    AutoCrankObservationV16, AutoCrankOutcomeV16, AutoCrankPlanV16, AutoCrankWorkV16,
    BackingBucketStatusV16, PermissionlessProgressOutcomeV16,
};
//
// v2.2 already settled K/F before the B chunk (#310, port of 137662a9), so the three upstream
// cases above pass here with or without f1d6b3f7. What f1d6b3f7 adds for v2.2 is the expiry
// step: the B chunk now expires a lapsed backing bucket in the account's source roster before it
// settles K/F against it, as the refresh does. This case puts the winner's gain domain in its
// roster (an early refresh), lets the provider's backing there lapse after the liquidation, and
// checks that the B-chunk crank expires that bucket before it books anything.

fn run_roster_lapse(
    loser_first: bool,
) -> (Vec<AutoCrankOutcomeV16>, BackingBucketStatusV16, WindDown) {
    let mut w = World::with_config(1, &[1_000_000, W_DEP], |cfg| {
        cfg.initial_margin_bps = 1_000;
        cfg.maintenance_margin_bps = 500;
        cfg.max_price_move_bps_per_slot = 400;
        cfg.max_abs_funding_e9_per_slot = 0;
    });
    let domain = World::loss_domain(0, false);
    let expiry = w.slot + 5;
    w.op(|m, _| m.deposit_fresh_counterparty_backing_not_atomic(domain, 300_000, expiry))
        .expect("provider deposit");
    w.provider[domain] += 300_000;
    w.trade(1, 0, 0, UNITS);
    let mut px = PX0;
    for k in 0..4 {
        px = px * 104 / 100;
        w.step(0, px, 0);
        if k == 2 {
            // The winner's gain is credited from `domain`, which enters its source roster.
            w.crank(1);
        }
    }
    if loser_first {
        w.crank(0);
    } else {
        w.crank(1);
        w.crank(0);
    }
    for _ in 0..5 {
        w.step(0, px, 0);
    }
    assert!(w.slot > expiry, "the provider's backing has lapsed");
    assert_eq!(w.bucket(domain).status, BackingBucketStatusV16::Fresh);
    // The winner is B-stale: its next crank is planned as a B chunk.
    let now_slot = w.slot;
    let observations = [AutoCrankObservationV16 {
        asset_index: 0,
        effective_price: px,
        funding_rate_e9: 0,
    }];
    let first = w
        .op(|m, a| {
            m.permissionless_auto_crank_not_atomic(
                &mut a[1],
                AutoCrankWorkV16 {
                    now_slot,
                    observations: &observations,
                    resolved_close_fee_rate_per_slot: 0,
                },
            )
        })
        .expect("winner's first crank after the lapse");
    assert_eq!(
        first.selected,
        AutoCrankPlanV16::SettleBChunk { asset_index: 0 }
    );
    let status_after_first = w.bucket(domain).status;
    let outcomes = vec![first.outcome];
    w.crank(1);
    w.crank(0);
    w.crank(1);
    (outcomes, status_after_first, w.wind_down(&[0, 1]))
}

#[test]
fn v16_settle_b_crank_expires_lapsed_roster_backing_before_settling() {
    let domain = World::loss_domain(0, false);
    let winner_first = run_roster_lapse(false);
    let loser_first = run_roster_lapse(true);
    println!("roster lapse: winner first: {winner_first:?}");
    println!("roster lapse: loser first:  {loser_first:?}");
    for (outcomes, status, _) in [&winner_first, &loser_first] {
        // Before the port the B chunk ran against the lapsed Fresh bucket
        // (AccountBChunk) and only the following refresh expired it.
        assert_eq!(
            outcomes[0],
            AutoCrankOutcomeV16::Progressed(
                PermissionlessProgressOutcomeV16::SourceBackingExpired { domain }
            )
        );
        assert_ne!(*status, BackingBucketStatusV16::Fresh);
    }
    assert_eq!(winner_first.2, loser_first.2);
    assert_eq!(winner_first.2.vault_end, 0);
}
