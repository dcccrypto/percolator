//! `PortfolioV16ViewMut::compact_source_domains` (src/v16.rs) now returns early when the table is
//! already compact: occupied slots form a prefix in ascending domain order and every slot after
//! them is all-zero bytes. This checks, on random and edge tables, that whenever the skip
//! predicate says yes the full compaction leaves every byte unchanged, and that the predicate says
//! yes for every canonical (compacted) table. Same idea as #319, on the #383 zero-tail compare.
//! Needs `--features fuzz` (exposes the hooks).
#![cfg(all(feature = "fuzz", not(kani)))]
use percolator::{
    compact_source_domain_table_for_test as compact,
    source_domain_table_already_compact_for_test as already_compact,
    PortfolioSourceDomainV16Account as Slot, PORTFOLIO_SOURCE_DOMAIN_CAP as CAP,
};

type Table = [Slot; CAP];

struct XorShift(u64);
impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Byte layout: domain 0..4, source_claim_market_id 4..12, the "occupied" counters after.
const TAG_BYTES: usize = 12;

fn occupied(r: &mut XorShift, domain: u32) -> Slot {
    let mut s = Slot::default();
    let b = bytemuck::bytes_of_mut(&mut s);
    b[..4].copy_from_slice(&domain.to_le_bytes());
    if r.below(2) == 0 {
        b[4 + r.below(8) as usize] = (r.next() as u8) | 1;
    }
    let len = b.len();
    for _ in 0..=r.below(3) {
        let at = TAG_BYTES + r.below((len - TAG_BYTES) as u64) as usize;
        b[at] = (r.next() as u8) | 1;
    }
    s
}

/// Not occupied but not default: domain or market id set.
fn tagged_empty(r: &mut XorShift) -> Slot {
    let mut s = Slot::default();
    let b = bytemuck::bytes_of_mut(&mut s);
    b[r.below(TAG_BYTES as u64) as usize] = (r.next() as u8) | 1;
    s
}

fn random_table(r: &mut XorShift) -> Table {
    let mut t = [Slot::default(); CAP];
    let n = r.below(CAP as u64 + 1) as usize;
    let mut d = r.below(4) as u32;
    for slot in t.iter_mut().take(n) {
        *slot = occupied(r, d);
        // nondecreasing, ties allowed; sometimes a descent
        d = if r.below(12) == 0 {
            d.saturating_sub(1 + r.below(3) as u32)
        } else {
            d + r.below(3) as u32
        };
    }
    match r.below(8) {
        0 if n >= 2 => t.swap(0, n - 1),
        1 if n < CAP => {
            let at = n + r.below((CAP - n) as u64) as usize;
            t[at] = tagged_empty(r);
        }
        2 if n < CAP && n > 0 => {
            // a hole inside the prefix
            let at = r.below(n as u64) as usize;
            t[at] = Slot::default();
        }
        3 if n < CAP => {
            // an occupied slot after a gap
            let at = n + r.below((CAP - n) as u64) as usize;
            let d = r.below(64) as u32;
            t[at] = occupied(r, d);
        }
        4 => {
            // one random byte anywhere
            let b: &mut [u8] = bytemuck::cast_slice_mut(&mut t);
            let at = r.below(b.len() as u64) as usize;
            b[at] ^= (r.next() as u8) | 1;
        }
        _ => {}
    }
    t
}

fn check(t: &Table) {
    let mut full = *t;
    compact(&mut full);
    if already_compact(t) {
        assert_eq!(
            bytemuck::bytes_of(&full),
            bytemuck::bytes_of(t),
            "skip predicate true but compaction changes the table"
        );
    }
    // a compacted table is always recognised, and compaction is idempotent
    assert!(
        already_compact(&full),
        "compacted table not recognised as compact"
    );
    let mut twice = full;
    compact(&mut twice);
    assert_eq!(bytemuck::bytes_of(&twice), bytemuck::bytes_of(&full));
}

#[test]
fn skip_matches_full_compaction_on_edges() {
    let mut r = XorShift(0x1f83_d9ab_fb41_bd6b);
    let empty = [Slot::default(); CAP];
    assert!(already_compact(&empty));
    check(&empty);
    for n in 0..=CAP {
        // ascending prefix of every length, then the same with ties and with one descent
        let mut t = [Slot::default(); CAP];
        for (i, slot) in t.iter_mut().take(n).enumerate() {
            *slot = occupied(&mut r, i as u32);
        }
        assert!(already_compact(&t));
        check(&t);
        if n >= 2 {
            let mut u = t;
            u.swap(n - 2, n - 1);
            assert!(!already_compact(&u));
            check(&u);
            let mut v = t;
            v[n - 1].domain = v[n - 2].domain;
            assert!(already_compact(&v));
            check(&v);
        }
        if n < CAP {
            // one non-zero byte at every 7th position of the tail
            let slot_len = core::mem::size_of::<Slot>();
            for at in (n * slot_len..CAP * slot_len).step_by(7) {
                let mut u = t;
                bytemuck::cast_slice_mut::<Slot, u8>(&mut u)[at] = 0x80;
                // a tagged-but-unoccupied slot (domain / market id byte) is never a compact tail
                if at % slot_len < 12 {
                    assert!(!already_compact(&u), "n={n} at={at}");
                }
                check(&u);
            }
        }
    }
}

#[test]
fn skip_matches_full_compaction_on_random_tables() {
    let mut r = XorShift(0x5be0_cd19_137e_2179);
    for _ in 0..100_000 {
        let t = random_table(&mut r);
        check(&t);
    }
}
