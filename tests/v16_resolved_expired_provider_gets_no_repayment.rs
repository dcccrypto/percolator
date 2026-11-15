//! A provider whose backing expired is out nothing and is not repaid by a late resolved loss.
//!
//! upstream 5e3af071 repays the provider that fronted a gain from a late resolved loss,
//! capped at `principal - fresh reserved backing`. That cap is only as good as the principal
//! it reads: upstream reduces provider principal whenever fresh reserved backing leaves
//! without being consumed (withdraw, expiry, impairment). v2.2's S10 mirror was reduced on
//! withdraw only, so after an expiry the provider still "had" its whole principal and a late
//! loss was booked to it as backing it could then withdraw: value it never put in.
//!
//! Scenario (two assets at 1.00, no fees, 100% margin): a provider backs domain 1 (asset 0,
//! short losers) with 1,000M. X is long 1,000 of asset 0 against Y and Y2 (500 each) and
//! long 1,000 of asset 1 against W.
//!   1. Asset 0 rises 4%; Y settles (20M loser-paid backing), X settles (40M claim).
//!   2. Asset 1 falls 4%; X and W settle: X pays its loss with the domain-1 face, 40M of
//!      backing is consumed, a receivable is left in domain 1.
//!   3. The market resolves, the whole domain-1 backing lapses and is expired: the provider
//!      is out nothing that is still owed (the backing it lost went to the junior pool).
//!   4. Y2 closes late: its loss must not be booked into domain 1 as provider backing.
mod kf_world;

use kf_world::{World, BACKING_LIFETIME, DEPOSIT};
use percolator::BOUND_SCALE;

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
fn expired_provider_is_not_repaid_by_a_late_resolved_loss() {
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
    assert!(s.provider_receivable_num != 0, "scenario: a receivable exists");
    assert_eq!(principal(&w, PROVIDER_DOMAIN), PRINCIPAL * BOUND_SCALE);

    w.slot += 1;
    let resolved_slot = w.slot;
    w.op(|m, _| m.resolve_market_not_atomic(resolved_slot)).unwrap();
    let end_slot = resolved_slot + 2 * BACKING_LIFETIME;
    w.op(|m, _| m.advance_resolved_slot_not_atomic(end_slot)).unwrap();
    w.op(|m, _| m.expire_source_backing_bucket_not_atomic(PROVIDER_DOMAIN, end_slot))
        .unwrap();
    // Expiry took the provider's principal with the backing that lapsed.
    assert_eq!(w.source(PROVIDER_DOMAIN).fresh_reserved_backing_num, 0);
    assert_eq!(
        principal(&w, PROVIDER_DOMAIN),
        0,
        "expiry releases the provider principal behind the lapsed backing"
    );

    // Y2's late loss is not repaid to the provider.
    let mut booked_to_provider = 0u128;
    for _ in 0..64 {
        let before = w.source(PROVIDER_DOMAIN).fresh_reserved_backing_num;
        let _ = w.op(|m, a| m.close_resolved_account_not_atomic(&mut a[Y2], 0));
        booked_to_provider += w.source(PROVIDER_DOMAIN).fresh_reserved_backing_num - before;
        if w.accts[Y2].resolved_payout_receipt.present != 0 || booked_to_provider != 0 {
            break;
        }
    }
    assert_eq!(booked_to_provider, 0, "a provider that is out nothing is repaid nothing");
    assert_eq!(w.source(PROVIDER_DOMAIN).fresh_reserved_backing_num, 0);
}
