//! `validate_with_market` and the refresh loop (src/v16.rs) test a decoded inactive leg for
//! emptiness with the raw `is_empty_encoding()` byte compare instead of `PortfolioLegV16::is_empty()`.
//! For every image that decodes, the two must agree. This checks it on the empty encoding, on
//! every single-byte change of it, and on random images (active ones included).
use percolator::{PortfolioLegV16Account, PORTFOLIO_LEG_V16_EMPTY_ACCOUNT};

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

fn check(leg: &PortfolioLegV16Account) -> bool {
    match leg.try_to_runtime() {
        Ok(decoded) => {
            assert_eq!(decoded.is_empty(), leg.is_empty_encoding(), "{leg:?}");
            true
        }
        Err(_) => false,
    }
}

#[test]
fn empty_encoding_and_every_single_byte_change() {
    let empty = PORTFOLIO_LEG_V16_EMPTY_ACCOUNT;
    assert!(check(&empty));
    assert!(empty.try_to_runtime().unwrap().is_empty());
    let len = core::mem::size_of::<PortfolioLegV16Account>();
    let mut decoded_non_empty = 0;
    for at in 0..len {
        for v in [1u8, 2, 0x80, 0xff] {
            let mut leg = empty;
            let b = bytemuck::bytes_of_mut(&mut leg);
            b[at] ^= v;
            if check(&leg) && !leg.try_to_runtime().unwrap().is_empty() {
                decoded_non_empty += 1;
            }
        }
    }
    // no single-byte change of the empty encoding decodes to a non-empty leg: inactive ones are
    // refused (HiddenLeg / InvalidConfig), active=1 fails validate_active_leg (loss_weight 0)
    assert_eq!(decoded_non_empty, 0);
}

#[test]
fn random_images() {
    let mut r = XorShift(0xcbbb_9d5d_c105_9ed8);
    let len = core::mem::size_of::<PortfolioLegV16Account>();
    for round in 0..200_000u32 {
        let mut leg = PORTFOLIO_LEG_V16_EMPTY_ACCOUNT;
        {
            let b = bytemuck::bytes_of_mut(&mut leg);
            match round % 3 {
                0 => b.iter_mut().for_each(|x| *x = r.next() as u8),
                _ => {
                    for _ in 0..1 + r.next() % 3 {
                        let at = (r.next() % len as u64) as usize;
                        b[at] = r.next() as u8;
                    }
                }
            }
        }
        if round % 5 == 0 {
            leg.active = (r.next() % 3) as u8;
        }
        check(&leg);
    }
}
