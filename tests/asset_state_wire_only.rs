//! `AssetStateV16Account::validate_wire_only` (src/v16.rs) runs the fallible steps of
//! `try_to_runtime` without building the runtime value; three readers that need only raw fields
//! call it instead of the full decode. It must fail exactly when, and with the same error as,
//! `try_to_runtime`. This compares the two on targeted corruptions and on random account bytes,
//! and checks the raw fields those readers use against the decoded ones.
use percolator::{AssetStateV16Account, V16PodI128};

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
}

fn check(a: &AssetStateV16Account) {
    let full = a.try_to_runtime();
    assert_eq!(a.validate_wire_only().err(), full.err(), "{a:?}");
    if let Ok(rt) = full {
        assert_eq!(a.market_id.get(), rt.market_id);
        assert_eq!(a.stored_pos_count_long.get(), rt.stored_pos_count_long);
        assert_eq!(a.stored_pos_count_short.get(), rt.stored_pos_count_short);
        assert_eq!(
            a.stale_account_count_long.get(),
            rt.stale_account_count_long
        );
        assert_eq!(
            a.stale_account_count_short.get(),
            rt.stale_account_count_short
        );
    }
}

fn i128_fields(a: &mut AssetStateV16Account) -> [&mut V16PodI128; 8] {
    [
        &mut a.k_long,
        &mut a.k_short,
        &mut a.f_long_num,
        &mut a.f_short_num,
        &mut a.k_epoch_start_long,
        &mut a.k_epoch_start_short,
        &mut a.f_epoch_start_long_num,
        &mut a.f_epoch_start_short_num,
    ]
}

#[test]
fn wire_only_matches_full_decode_on_targeted_corruptions() {
    let base = AssetStateV16Account::default();
    assert!(base.try_to_runtime().is_ok());
    check(&base);
    // every byte value in each enum field, alone and together
    for v in 0..=255u8 {
        for field in 0..3 {
            let mut a = base;
            match field {
                0 => a.lifecycle = v,
                1 => a.mode_long = v,
                _ => a.mode_short = v,
            }
            check(&a);
        }
        let mut a = base;
        a.lifecycle = v;
        a.mode_long = v.wrapping_add(1);
        a.mode_short = v.wrapping_add(2);
        check(&a);
    }
    // i128::MIN in each signed field, alone, in pairs, and with a bad enum (order of errors)
    for i in 0..8 {
        for j in 0..8 {
            for bad_enum in [None, Some(0u8), Some(1), Some(2)] {
                let mut a = base;
                *i128_fields(&mut a)[i] = V16PodI128::new(i128::MIN);
                *i128_fields(&mut a)[j] = V16PodI128::new(i128::MIN);
                match bad_enum {
                    Some(0) => a.lifecycle = 200,
                    Some(1) => a.mode_long = 3,
                    Some(2) => a.mode_short = 255,
                    _ => {}
                }
                check(&a);
            }
            let mut a = base;
            *i128_fields(&mut a)[i] = V16PodI128::new(i128::MIN + 1);
            *i128_fields(&mut a)[j] = V16PodI128::new(i128::MAX);
            check(&a);
        }
    }
}

#[test]
fn wire_only_matches_full_decode_on_random_bytes() {
    let mut rng = XorShift(0x510e_527f_ade6_82d1);
    for round in 0..200_000u32 {
        let mut a = AssetStateV16Account::default();
        {
            let bytes = bytemuck::bytes_of_mut(&mut a);
            match round % 4 {
                // fully random
                0 => bytes.iter_mut().for_each(|b| *b = rng.next() as u8),
                // a few random bytes
                1 => {
                    for _ in 0..1 + rng.next() % 4 {
                        let i = (rng.next() % bytes.len() as u64) as usize;
                        bytes[i] = rng.next() as u8;
                    }
                }
                // one field-sized run of 0x80/0x00 (i128::MIN is 15 zero bytes then 0x80)
                2 => {
                    let i = (rng.next() % bytes.len() as u64) as usize;
                    let end = (i + 16).min(bytes.len());
                    bytes[i..end].iter_mut().for_each(|b| *b = 0);
                    bytes[end - 1] = 0x80;
                }
                _ => {
                    bytes.iter_mut().for_each(|b| *b = rng.next() as u8);
                }
            }
        }
        if round % 4 == 3 {
            // random body, valid enums, so the i128 checks decide
            a.lifecycle = (rng.next() % 6) as u8;
            a.mode_long = (rng.next() % 3) as u8;
            a.mode_short = (rng.next() % 3) as u8;
            if rng.next() % 2 == 0 {
                let k = (rng.next() % 8) as usize;
                *i128_fields(&mut a)[k] = V16PodI128::new(i128::MIN);
            }
        }
        check(&a);
    }
}
