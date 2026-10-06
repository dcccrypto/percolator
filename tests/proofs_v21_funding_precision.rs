#![cfg(kani)]
//! fix/v21-funding-precision: `kernel_funding_index_deltas` is sign-mirrored, never lets the
//! receiver get more than the payer pays per unit of A, and is exact on a balanced ADL_ONE book.

use percolator::v16::kani_funding_index_deltas;
use percolator::ADL_ONE;

#[kani::proof]
#[kani::unwind(2)]
#[kani::solver(cadical)]
fn proof_v21_funding_index_deltas_mirror_and_conserve() {
    let mag: u32 = kani::any();
    let negative: bool = kani::any();
    let a_long: u64 = kani::any();
    let a_short: u64 = kani::any();
    kani::assume(mag != 0);
    kani::assume(a_long as u128 <= ADL_ONE && a_long != 0);
    kani::assume(a_short as u128 <= ADL_ONE && a_short != 0);
    let n = if negative { -(mag as i128) } else { mag as i128 };
    let (fl, fs) = kani_funding_index_deltas(n, a_long as u128, a_short as u128).unwrap();
    let (ml, ms) = kani_funding_index_deltas(-n, a_short as u128, a_long as u128).unwrap();
    // mirror: long under +n == short under -n (with A swapped)
    assert_eq!((fl, fs), (ms, ml));
    // payer is the side whose index falls; receiver's rises
    let (payer, pa, recv, ra) = if n > 0 {
        (-fl, a_long as u128, fs, a_short as u128)
    } else {
        (-fs, a_short as u128, fl, a_long as u128)
    };
    assert!(payer >= 0 && recv >= 0);
    // payer / a_payer >= |n| / 1e9 >= recv / a_recv (no value from rounding)
    assert!((payer as u128) * 1_000_000_000 >= (mag as u128) * pa);
    assert!((recv as u128) * 1_000_000_000 <= (mag as u128) * ra);
    // within one index unit of exact on each side
    assert!((payer as u128) * 1_000_000_000 < (mag as u128) * pa + 1_000_000_000);
    assert!((recv as u128 + 1) * 1_000_000_000 > (mag as u128) * ra);
    kani::cover!(n > 0 && payer > recv, "asymmetric A: payer index moves more than receiver");
    kani::cover!(n < 0, "shorts pay");
}

#[kani::proof]
#[kani::unwind(2)]
#[kani::solver(cadical)]
fn proof_v21_funding_index_deltas_exact_on_balanced_book() {
    let mag: u32 = kani::any();
    let negative: bool = kani::any();
    kani::assume(mag != 0);
    let n = if negative { -(mag as i128) } else { mag as i128 };
    let (fl, fs) = kani_funding_index_deltas(n, ADL_ONE, ADL_ONE).unwrap();
    assert_eq!(fl, -fs);
    assert_eq!(fl, -n * (ADL_ONE / 1_000_000_000) as i128);
    kani::cover!(mag < 1_000, "sub-unit funding is no longer floored away");
}
