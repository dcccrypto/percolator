//! Expiry releases the provider principal behind the backing that lapsed.
//!
//! upstream 5e3af071 repays the provider that fronted a gain from a late resolved loss,
//! capped at `principal - fresh reserved backing`. That cap is only as good as the principal
//! it reads: upstream reduces provider principal whenever fresh reserved backing leaves
//! without being consumed (withdraw, expiry, impairment). v2.2's S10 mirror was reduced on
//! withdraw only, so after an expiry it still said the provider had its whole principal.
//!
//! What this test pins: the principal after expiry (the `left` assertion). Without the
//! release it stays 1,000M and that assertion fails. What it does not pin: the amount booked
//! to the provider by the late loss. Here the late loss (20M) equals what the provider is
//! still out (20M), so the booked amount is 20M with or without the release; it is asserted
//! as a conservation check only. A scenario where the cap binds needs a late loss larger than
//! `principal - fresh reserved`, which this close order cannot produce (the loss total is
//! bounded by the claim it pays).
//!
//! Scenario (two assets at 1.00, no fees, 100% margin): a provider backs domain 1 (asset 0,
//! short losers) with 1,000M. X is long 1,000 of asset 0 against Y and Y2 (500 each) and
//! long 1,000 of asset 1 against W.
//!   1. Asset 0 rises 4%; Y settles (20M loser-paid backing), X settles (40M claim).
//!   2. Asset 1 falls 4%; X and W settle: X pays its loss with the domain-1 face, 40M of
//!      backing is consumed, a receivable is left in domain 1.
//!   3. The market resolves, the whole domain-1 backing lapses and is expired: fresh reserved
//!      backing was 980M, so expiry releases 980M of the 1,000M principal and 20M is left.
//!   4. Y2 closes late with a 20M loss: it closes, and the provider is booked exactly the
//!      20M it is still out.
mod kf_world;

use kf_world::{World, BACKING_LIFETIME, DEPOSIT};
use percolator::{ResolvedCloseOutcomeV16, BOUND_SCALE};

const X: usize = 0;
const Y: usize = 1;
const W: usize = 2;
const Y2: usize = 3;
const M: u128 = 1_000_000;
const PROVIDER_DOMAIN: usize = 1;
const PRINCIPAL: u128 = 1_000 * M;

fn principal(w: &World, domain: usize) -> u128 {
    let slot = &w.markets[domain / 2].engine;
    if domain % 2 == 0 {
        slot.provider_principal_long.get()
    } else {
        slot.provider_principal_short.get()
    }
}

#[test]
fn expiry_releases_provider_principal_and_a_late_loss_repays_only_what_is_left() {
    let mut w = World::new(2, &[DEPOSIT; 4]);
    w.provider_deposit(PROVIDER_DOMAIN, PRINCIPAL);
    w.trade(X, Y, 0, 500);
    w.trade(X, Y2, 0, 500);
    w.trade(X, W, 1, 1_000);
    w.step(0, 1_040_000, 0);
    w.refresh(Y);
    w.refresh(X);
    w.step(1, 960_000, 0);
    w.refresh_all(&[X, W]);
    let s = w.source(PROVIDER_DOMAIN);
    assert!(
        s.provider_receivable_num != 0,
        "scenario: a receivable exists"
    );
    assert_eq!(principal(&w, PROVIDER_DOMAIN), PRINCIPAL * BOUND_SCALE);

    w.slot += 1;
    let resolved_slot = w.slot;
    w.op(|m, _| m.resolve_market_not_atomic(resolved_slot))
        .unwrap();
    let end_slot = resolved_slot + 2 * BACKING_LIFETIME;
    w.op(|m, _| m.advance_resolved_slot_not_atomic(end_slot))
        .unwrap();
    w.op(|m, _| m.expire_source_backing_bucket_not_atomic(PROVIDER_DOMAIN, end_slot))
        .unwrap();
    // Expiry took the provider's principal with the backing that lapsed.
    assert_eq!(w.source(PROVIDER_DOMAIN).fresh_reserved_backing_num, 0);
    // Principal 1,000M; fresh reserved was 980M (1,000M - 40M consumed + 20M loser-paid, which
    // never was principal). Expiry releases all 980M, leaving 20M: the most the provider can
    // still be out (upstream 5e3af071 arithmetic). Before the fix principal stayed at 1,000M.
    let left = principal(&w, PROVIDER_DOMAIN);
    assert_eq!(
        left,
        20 * M * BOUND_SCALE,
        "expiry releases the provider principal behind the lapsed backing"
    );

    // Y2 closes late. Every close attempt's error is surfaced (none is discarded) and Y2 must
    // really finish, so a close that always fails cannot pass.
    let mut booked_to_provider = 0u128;
    let mut closed = false;
    for _ in 0..64 {
        let before = w.source(PROVIDER_DOMAIN).fresh_reserved_backing_num;
        match w
            .op(|m, a| m.close_resolved_account_not_atomic(&mut a[Y2], 0))
            .unwrap_or_else(|e| panic!("resolved close of Y2: {e:?}"))
        {
            ResolvedCloseOutcomeV16::ProgressOnly => {}
            ResolvedCloseOutcomeV16::Closed { .. } => closed = true,
        }
        booked_to_provider += w.source(PROVIDER_DOMAIN).fresh_reserved_backing_num - before;
        if closed {
            break;
        }
    }
    assert!(closed, "Y2 never finished closing");
    // Conservation, not a regression guard (see the header): the late 20M loss repays exactly
    // the 20M the provider is still out, never more.
    assert_eq!(
        booked_to_provider, left,
        "late loss booked {booked_to_provider} to the provider, which is still out {left}"
    );
}
