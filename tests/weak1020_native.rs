//! v22kani WEAK agent (2026-10-20): NATIVE exhaustive companion of the Kani row
//! `proof_v17_lp_vault_deposit_redeem_no_profit`. The Kani harness stubs `wide_mul_div_floor_u128` (U256/U512 long
//! division is intractable for CBMC); here the REAL primitive runs, over EVERY (amount, total_shares, nav) in
//! [0, 255]^3 (16.7M round trips), removing the stub condition on that domain. Not Kani; evidence only.
#![cfg(feature = "fork-facade")]

use percolator::lp_vault::{lp_atoms_for_redemption, lp_shares_for_deposit};
use percolator::V16Error;

#[test]
fn weak1020_native_lp_round_trip_never_profits_u8_exhaustive() {
    let mut round_trips = 0u64;
    let mut strict = 0u64;
    for a in 0u128..=255 {
        for ts in 0u128..=255 {
            for nav in 0u128..=255 {
                let minted = lp_shares_for_deposit(a, ts, nav);
                if ts == 0 {
                    assert_eq!(minted, Ok(a));
                    continue;
                }
                if nav == 0 {
                    assert_eq!(minted, Err(V16Error::InvalidConfig));
                    continue;
                }
                let s = match minted {
                    Ok(s) => s,
                    Err(e) => {
                        assert_eq!(e, V16Error::LpVaultZeroSharesMinted);
                        assert!(a * ts < nav);
                        continue;
                    }
                };
                assert_eq!(s, a * ts / nav, "real primitive == exact floor");
                let back = lp_atoms_for_redemption(s, ts + s, nav + a).unwrap();
                assert_eq!(back, s * (nav + a) / (ts + s), "real primitive == exact floor (u16 operands)");
                assert!(back <= a, "a={a} ts={ts} nav={nav} s={s} back={back}");
                round_trips += 1;
                strict += (back < a) as u64;
            }
        }
    }
    assert!(round_trips > 1_000_000 && strict > 0);
}

/// Sampled at production width: 2^24 (amount, total_shares, nav) triples with each operand drawn at a random bit
/// width in [1, 64] (xorshift, fixed seed), the REAL primitive (U256/U512 path for wide products).
#[test]
fn weak1020_native_lp_round_trip_never_profits_u64_sampled() {
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    let mut checked = 0u64;
    for _ in 0..(1u64 << 24) {
        let mut draw = || {
            let bits = (next() % 64) + 1;
            (next() >> (64 - bits)) as u128
        };
        let (a, ts, nav) = (draw(), draw(), draw());
        if ts == 0 || nav == 0 || a == 0 {
            continue;
        }
        let s = match lp_shares_for_deposit(a, ts, nav) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let back = lp_atoms_for_redemption(s, ts + s, nav + a).unwrap();
        assert!(back <= a, "a={a} ts={ts} nav={nav} s={s} back={back}");
        checked += 1;
    }
    assert!(checked > 1_000_000);
}
