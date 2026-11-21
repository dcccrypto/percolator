//! `source_domain_tail_is_zero` (src/v16.rs) lets two source-domain table walks
//! (`validate_source_credit_shape_with_market`, `source_claim_bound_sum_num`) stop early when the
//! rest of the 32-slot table is all default slots. It must say "zero" only when every slot from
//! `from` on equals `PortfolioSourceDomainV16Account::default()`: one non-zero byte anywhere in
//! the tail, in any field, must make it false.
//! Needs `--features fuzz` (exposes the test hook, same as tests/rounding_edge.rs).
#![cfg(feature = "fuzz")]
use percolator::{
    source_domain_tail_is_zero_for_test as tail_is_zero, PortfolioSourceDomainV16Account,
    V16PodU128, V16PodU32, V16PodU64, PORTFOLIO_SOURCE_DOMAIN_CAP,
};

type Table = [PortfolioSourceDomainV16Account; PORTFOLIO_SOURCE_DOMAIN_CAP];

fn reference(t: &Table, from: usize) -> bool {
    t.iter()
        .skip(from)
        .all(|d| *d == PortfolioSourceDomainV16Account::default())
}

fn empty() -> Table {
    [PortfolioSourceDomainV16Account::default(); PORTFOLIO_SOURCE_DOMAIN_CAP]
}

#[test]
fn zero_tail_on_empty_and_past_end() {
    let t = empty();
    for from in 0..=PORTFOLIO_SOURCE_DOMAIN_CAP + 2 {
        assert!(tail_is_zero(&t, from), "from={from}");
        assert_eq!(
            tail_is_zero(&t, from),
            reference(&t, from.min(PORTFOLIO_SOURCE_DOMAIN_CAP))
        );
    }
}

#[test]
fn any_single_nonzero_byte_in_the_tail_is_seen() {
    let slot_len = core::mem::size_of::<PortfolioSourceDomainV16Account>();
    let total = slot_len * PORTFOLIO_SOURCE_DOMAIN_CAP;
    for from in [
        0usize,
        1,
        2,
        PORTFOLIO_SOURCE_DOMAIN_CAP / 2,
        PORTFOLIO_SOURCE_DOMAIN_CAP - 1,
    ] {
        for pos in 0..total {
            for value in [1u8, 0x80, 0xff] {
                let mut t = empty();
                bytemuck::cast_slice_mut::<_, u8>(&mut t)[pos] = value;
                let expect = reference(&t, from);
                assert_eq!(
                    expect,
                    pos < from * slot_len,
                    "reference sanity pos={pos} from={from}"
                );
                assert_eq!(
                    tail_is_zero(&t, from),
                    expect,
                    "pos={pos} from={from} value={value}"
                );
            }
        }
    }
}

#[test]
fn occupied_prefix_then_zero_tail() {
    // The shape the early exit is for: k occupied/tagged slots, then defaults.
    for k in 0..=PORTFOLIO_SOURCE_DOMAIN_CAP {
        let mut t = empty();
        for (i, slot) in t.iter_mut().enumerate().take(k) {
            slot.domain = V16PodU32::new(i as u32);
            slot.source_claim_market_id = V16PodU64::new(1);
            slot.source_claim_bound_num = V16PodU128::new(1_000_000_000_000);
        }
        for from in 0..=PORTFOLIO_SOURCE_DOMAIN_CAP {
            assert_eq!(
                tail_is_zero(&t, from),
                reference(&t, from),
                "k={k} from={from}"
            );
            assert_eq!(tail_is_zero(&t, from), from >= k, "k={k} from={from}");
        }
    }
}

#[test]
fn hole_then_nonzero_slot_is_not_a_zero_tail() {
    // A default slot followed later by a tagged-but-unoccupied or occupied slot: the walk must
    // continue (the validator then rejects or checks that slot), so the tail is not zero.
    for gap in 1..PORTFOLIO_SOURCE_DOMAIN_CAP {
        let mut t = empty();
        t[gap].domain = V16PodU32::new(3);
        for from in 0..=gap {
            assert!(!tail_is_zero(&t, from), "gap={gap} from={from}");
        }
        assert!(tail_is_zero(&t, gap + 1));
        let mut t = empty();
        t[gap].source_lien_fee_last_slot = V16PodU64::new(7);
        assert!(!tail_is_zero(&t, 0) && !tail_is_zero(&t, gap));
    }
}
