//! K/F losses first settled after resolution.
//!
//! Ported from upstream 78d180f2 (aeyakovenko/percolator alt/resolved-kf-loss-netting-narrow).
//! The fork has that commit's Resolved netting but not its stranded-loser-payment placement
//! (part 2, which needs a55aceb3). The two tests that need the placement are ignored with that
//! reason, and the three that do not are run. The bounded-placement tests are not ported.
//!
//! In Live a newly observed loss is netted before support-and-burn: the leg's own unpaid
//! gain (its face in the twin domain) cancels it one for one, fully backed face is realized
//! at par, and partially backed face is kept while the loss is paid from capital. Resolved
//! mode had none of this. A loss first settled after resolution burned whatever face the
//! account held at the rate the domain happened to show at that moment, and consumed the
//! backing behind it, a provider's principal included. Who was paid what then depended on
//! which permissionless close ran first, in markets where every trader is solvent.
//!
//! Every world here is fully collateralized (1e12 of capital each, 100% margin, no fees), so
//! there is exactly one fair wind-down: every trader is paid its equity, every provider
//! takes its principal back, and nothing is retired. Each test checks it in every close
//! order.
mod kf_world;

use kf_world::{World, BACKING_LIFETIME, DEPOSIT};
use percolator::ResolvedCloseOutcomeV16;

const M: u128 = 1_000_000;

fn orders(n: usize) -> Vec<Vec<usize>> {
    fn rec(left: &mut Vec<usize>, cur: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if left.is_empty() {
            out.push(cur.clone());
            return;
        }
        for i in 0..left.len() {
            let x = left.remove(i);
            cur.push(x);
            rec(left, cur, out);
            cur.pop();
            left.insert(i, x);
        }
    }
    let mut out = Vec::new();
    rec(&mut (0..n).collect(), &mut Vec::new(), &mut out);
    out
}

fn assert_fair_in_every_close_order(n: usize, world: impl Fn() -> World, payouts: &[u128]) {
    for order in orders(n) {
        let mut w = world();
        let fair = w.fair_wind_down();
        assert_eq!(fair.payouts, payouts, "the world's own books");
        assert_eq!(w.wind_down(&order), fair, "close order {order:?}");
    }
}

/// The account's own face in the domain its loss is owed to. A (0) is short 1,000 while the
/// mark falls 4% and everyone settles: A holds 40M of face in domain 0, which B (1) paid in.
/// A turns long against C (2) and the mark falls 4% more; the market resolves with nobody
/// settled. A owes domain 0 40M and holds 40M of face in it, B owes 40M more, C is owed 80M.
///
/// Before: when C's claim was registered first, the domain read one third backed and A's
/// loss burned all of A's face for 13.3M of support, then took 26.7M from A's capital.
#[test]
fn own_face_in_the_loss_domain_is_not_burned_at_a_stale_rate() {
    assert_fair_in_every_close_order(
        3,
        || {
            let mut w = World::new(1, &[DEPOSIT; 3]);
            w.trade(1, 0, 0, 1_000);
            w.step(0, 960_000, 0);
            w.refresh_all(&[0, 1, 2]);
            w.trade(0, 2, 0, 2_000);
            w.step(0, 920_000, 0);
            w
        },
        &[DEPOSIT, DEPOSIT - 80 * M, DEPOSIT + 80 * M],
    );
}

/// A gain that reverses before its loser ever paid. X (0) is long 1,000 against Y (1) and a
/// provider backs domain 1 with 1,000M. The mark rises 4% and only X settles: the provider
/// fronts X's 40M of face. The mark returns and the market resolves. Nobody gained or lost.
///
/// Before: X's reversal loss consumed 40M of the provider's principal as support. Nothing
/// claimed it, and it was retired: the provider recovered 960M.
#[test]
fn reversal_leaves_provider_fronted_twin_backing_with_the_provider() {
    assert_fair_in_every_close_order(
        2,
        || {
            let mut w = World::new(1, &[DEPOSIT; 2]);
            w.provider_deposit(1, 1_000 * M);
            w.trade(0, 1, 0, 1_000);
            w.step(0, 1_040_000, 0);
            w.refresh(0);
            w.step(0, 1_000_000, 0);
            w
        },
        &[DEPOSIT, DEPOSIT],
    );
}

/// A loser that paid at a peak. X (0) is long 500 against Y (1); a provider backs domain 0
/// (asset 0, long losers) with 1,000M. The mark rises 4% and only Y settles: it pays 20M
/// into domain 1. The mark returns. Y's regain is claimed from domain 0, which X, who never
/// had a gain, does not fund; Y's own 20M sits in domain 1, where nothing claims it.
///
/// Before: the provider in domain 0 paid Y's regain and was never repaid, while Y's 20M
/// lapsed into the junior pool and was retired. Now the payment follows the regain once the
/// long side has settled.
#[test]
#[ignore = "needs upstream 78d180f2 part 2 (stranded loser payment placement), not ported"]
fn a_payment_stranded_by_a_reversal_follows_the_regain() {
    for settle_in_live in [false, true] {
        assert_fair_in_every_close_order(
            2,
            || {
                let mut w = World::new(1, &[DEPOSIT; 2]);
                w.provider_deposit(0, 1_000 * M);
                w.trade(0, 1, 0, 500);
                w.step(0, 1_040_000, 0);
                w.refresh(1);
                assert_eq!(w.domain(1), (0, 20 * M));
                w.step(0, 1_000_000, 0);
                if settle_in_live {
                    w.refresh_all(&[0, 1]);
                }
                w
            },
            &[DEPOSIT, DEPOSIT],
        );
    }
}

/// Losers that settle after a partial reversal pay the net, and all of it belongs to the
/// winners' remaining claims. Two longs (0, 1) against two shorts (2, 3), a provider behind
/// domain 1 with 2,000M. The mark rises 4% and only the longs settle (the provider fronts
/// 32M and 36M of face), then falls back 1% and the market resolves.
///
/// The longs' give-back cancels a quarter of their face. If a short has already paid its net
/// loss by then, none of that payment answers for the cancelled face, and moving a share of
/// it to the long-loser domain would strand it there.
#[test]
#[ignore = "needs upstream 78d180f2 part 2 (stranded loser payment placement), not ported"]
fn net_payments_after_a_partial_reversal_stay_with_the_remaining_claims() {
    assert_fair_in_every_close_order(
        4,
        || {
            let mut w = World::new(1, &[DEPOSIT; 4]);
            w.provider_deposit(1, 2_000 * M);
            w.trade(0, 2, 0, 800);
            w.trade(1, 3, 0, 900);
            w.step(0, 1_040_000, 0);
            w.refresh_all(&[0, 1]);
            assert_eq!(w.domain(1), (68 * M, 2_000 * M));
            w.step(0, 1_030_000, 0);
            w
        },
        &[
            DEPOSIT + 24 * M,
            DEPOSIT + 27 * M,
            DEPOSIT - 24 * M,
            DEPOSIT - 27 * M,
        ],
    );
}

/// Closes that start only after every backing lifetime has passed still settle and pay
/// everyone exactly: a loss that can no longer become backing is left to the junior pool,
/// as the ordinary path leaves it.
#[test]
fn losses_first_settled_past_the_backing_horizon_still_close() {
    for order in orders(3) {
        let mut w = World::new(1, &[DEPOSIT; 3]);
        w.trade(1, 0, 0, 1_000);
        w.step(0, 960_000, 0);
        w.refresh_all(&[0, 1, 2]);
        w.trade(0, 2, 0, 2_000);
        w.step(0, 920_000, 0);
        w.slot += 1;
        let resolved_slot = w.slot;
        w.op(|m, _| m.resolve_market_not_atomic(resolved_slot))
            .unwrap();
        let late = resolved_slot + 2 * BACKING_LIFETIME;
        w.op(|m, _| m.advance_resolved_slot_not_atomic(late))
            .unwrap();
        let mut paid = [0u128; 3];
        let mut closed = [false; 3];
        for _ in 0..16 {
            for &i in &order {
                if closed[i] {
                    continue;
                }
                match w
                    .op(|m, a| m.close_resolved_account_not_atomic(&mut a[i], 0))
                    .unwrap_or_else(|e| panic!("order {order:?}: close of {i}: {e:?}"))
                {
                    ResolvedCloseOutcomeV16::Closed { payout } => {
                        paid[i] += payout;
                        closed[i] = true;
                    }
                    ResolvedCloseOutcomeV16::ProgressOnly => {
                        for d in 0..2 {
                            let _ = w.op(|m, _| m.expire_source_backing_bucket_not_atomic(d, late));
                        }
                    }
                }
            }
        }
        assert_eq!(closed, [true; 3], "order {order:?}");
        for i in 0..3 {
            if w.accts[i].resolved_payout_receipt.present != 0 {
                paid[i] += w
                    .op(|m, a| m.claim_resolved_payout_topup_not_atomic(&mut a[i]))
                    .unwrap_or(0);
            }
        }
        assert_eq!(
            paid,
            [DEPOSIT, DEPOSIT - 80 * M, DEPOSIT + 80 * M],
            "order {order:?}"
        );
        w.validate().unwrap();
    }
}
