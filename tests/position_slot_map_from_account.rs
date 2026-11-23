//! `PositionSlotMapV16::from_account` (src/v16.rs) now checks a decoded inactive leg for
//! emptiness on its raw bytes (`decoded_leg_slot_is_empty`, #399) instead of
//! `PortfolioLegV16::is_empty()`. This compares the whole result (map or error) with the
//! pre-change loop on accounts mixing valid active legs, empty slots, corrupted slots and
//! bitmap bits that do or do not match the legs.
//! Needs `--features fuzz` (exposes the hook).
#![cfg(all(feature = "fuzz", not(kani)))]
use percolator::{
    position_slot_map_from_account_vs_reference_for_test as compare, PortfolioAccountV16Account,
    PortfolioLegV16, PortfolioLegV16Account, PortfolioV16View, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, SideV16, V16PodU64, ADL_ONE, PORTFOLIO_LEG_V16_EMPTY_ACCOUNT,
    V16_MAX_PORTFOLIO_ASSETS_N,
};

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

fn empty_account() -> PortfolioAccountV16Account {
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        [1; 32], [2; 32], [3; 32],
    ));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(header).unwrap();
    a
}

fn active_leg(r: &mut XorShift, asset_index: u32) -> PortfolioLegV16Account {
    let basis = 1 + r.below(1_000_000_000) as i128;
    let mut leg = PortfolioLegV16::EMPTY;
    leg.active = true;
    leg.asset_index = asset_index;
    leg.market_id = 1 + asset_index as u64;
    leg.side = if r.below(2) == 0 { SideV16::Long } else { SideV16::Short };
    leg.basis_pos_q = basis;
    leg.a_basis = ADL_ONE;
    leg.loss_weight = basis as u128;
    PortfolioLegV16Account::from_runtime(&leg)
}

fn set_bit(a: &mut PortfolioAccountV16Account, slot: usize, on: bool) {
    let word = slot / 64;
    let mut v = a.active_bitmap[word].get();
    if on {
        v |= 1 << (slot % 64);
    } else {
        v &= !(1 << (slot % 64));
    }
    a.active_bitmap[word] = V16PodU64::new(v);
}

fn check(a: &PortfolioAccountV16Account) {
    let (new, old) = compare(&PortfolioV16View::new(a));
    assert_eq!(new, old);
}

#[test]
fn empty_and_simple_accounts() {
    let mut r = XorShift(0x6a09_e667_bb67_ae85);
    let a = empty_account();
    check(&a);
    for n in 0..=V16_MAX_PORTFOLIO_ASSETS_N {
        let mut a = empty_account();
        for slot in 0..n {
            a.legs[slot] = active_leg(&mut r, slot as u32);
            set_bit(&mut a, slot, true);
        }
        check(&a);
    }
}

#[test]
fn every_single_byte_change_of_an_empty_slot() {
    let len = core::mem::size_of::<PortfolioLegV16Account>();
    for at in 0..len {
        for v in [1u8, 0x80, 0xff] {
            for bit in [false, true] {
                let mut a = empty_account();
                let mut leg = PORTFOLIO_LEG_V16_EMPTY_ACCOUNT;
                bytemuck::bytes_of_mut(&mut leg)[at] ^= v;
                a.legs[3] = leg;
                set_bit(&mut a, 3, bit);
                check(&a);
            }
        }
    }
}

#[test]
fn random_accounts() {
    let mut r = XorShift(0x3c6e_f372_a54f_f53a);
    let len = core::mem::size_of::<PortfolioLegV16Account>();
    for _ in 0..20_000 {
        let mut a = empty_account();
        for slot in 0..V16_MAX_PORTFOLIO_ASSETS_N {
            match r.below(6) {
                0 | 1 => {
                    let asset = r.below(V16_MAX_PORTFOLIO_ASSETS_N as u64) as u32;
                    a.legs[slot] = active_leg(&mut r, asset);
                    set_bit(&mut a, slot, r.below(8) != 0);
                }
                2 => {
                    let mut leg = PORTFOLIO_LEG_V16_EMPTY_ACCOUNT;
                    let at = r.below(len as u64) as usize;
                    bytemuck::bytes_of_mut(&mut leg)[at] = r.next() as u8;
                    a.legs[slot] = leg;
                    set_bit(&mut a, slot, r.below(4) == 0);
                }
                _ => set_bit(&mut a, slot, r.below(16) == 0),
            }
        }
        check(&a);
    }
}
