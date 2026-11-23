//! The asset readers that moved from `asset_state` (full 54-field decode) to `asset_state_wire`
//! (src/v16.rs: same bounds check, `validate_wire_only`, raw field reads) must return exactly what
//! their old bodies returned, error included. The fuzz hook evaluates each reader next to its old
//! body; this drives it over in-range and out-of-range indexes and random corruptions of the asset
//! bytes (enum bytes, ADL indexes, counters, prices, i128::MIN in the K/F fields).
//! Needs `--features fuzz` (exposes the hook).
#![cfg(all(feature = "fuzz", not(kani)))]
use percolator::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    V16Config, V16PodI128, V16PodU128, V16PodU64, ADL_ONE,
};

const PRICE: u64 = 1_000;

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

fn group(max_slots: u32, band: bool) -> (MarketGroupV16HeaderAccount, Vec<Market<u64>>) {
    let cfg = V16Config::public_user_fund_with_market_slots(max_slots as u16, max_slots, 0, 10);
    let mut h = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, max_slots + 1, 0).unwrap();
    let mut m: Vec<Market<u64>> = (0..(max_slots + 1) as u64)
        .map(|i| Market::new(i, EngineAssetSlotV16Account::default()))
        .collect();
    for i in 0..max_slots as usize {
        h.activate_empty_asset_slot_not_atomic(i as u32, &mut m[i].engine, PRICE, 1 + i as u64)
            .unwrap();
    }
    if band {
        // the readers only look at these two raw header fields
        h.config.band_bps = V16PodU64::new(100);
        h.config.band_max_positions_per_side = V16PodU64::new(3);
    }
    (h, m)
}

fn check(h: &mut MarketGroupV16HeaderAccount, m: &mut [Market<u64>], max_slots: u32) {
    let v = MarketGroupV16ViewMut::new(h, m);
    for i in 0..(max_slots as usize + 2) {
        for (new, old) in v.asset_wire_readers_vs_reference_for_test(i) {
            assert_eq!(new, old, "asset_index={i}");
        }
    }
}

fn corrupt(r: &mut XorShift, slot: &mut EngineAssetSlotV16Account) {
    let a = &mut slot.asset;
    match r.below(10) {
        0 => a.lifecycle = r.below(8) as u8,
        1 => a.mode_long = r.below(5) as u8,
        2 => a.mode_short = r.below(5) as u8,
        3 => a.a_long = V16PodU128::new(if r.below(2) == 0 { ADL_ONE } else { r.below(ADL_ONE as u64) as u128 }),
        4 => a.a_short = V16PodU128::new(if r.below(2) == 0 { ADL_ONE } else { r.below(ADL_ONE as u64) as u128 }),
        5 => {
            a.stored_pos_count_long = V16PodU64::new(r.below(6));
            a.stored_pos_count_short = V16PodU64::new(r.below(6));
        }
        6 => a.raw_oracle_target_price = V16PodU64::new(PRICE + r.below(3) - 1),
        7 => a.effective_price = V16PodU64::new(PRICE + r.below(3) - 1),
        8 => {
            let v = V16PodI128::new(i128::MIN);
            match r.below(8) {
                0 => a.k_long = v,
                1 => a.k_short = v,
                2 => a.f_long_num = v,
                3 => a.f_short_num = v,
                4 => a.k_epoch_start_long = v,
                5 => a.k_epoch_start_short = v,
                6 => a.f_epoch_start_long_num = v,
                _ => a.f_epoch_start_short_num = v,
            }
        }
        _ => {
            let b = bytemuck::bytes_of_mut(a);
            let at = r.below(b.len() as u64) as usize;
            b[at] = r.next() as u8;
        }
    }
}

#[test]
fn wire_readers_match_old_bodies_on_fresh_groups() {
    for band in [false, true] {
        let (mut h, mut m) = group(3, band);
        check(&mut h, &mut m, 3);
    }
}

#[test]
fn wire_readers_match_old_bodies_on_corrupted_assets() {
    let mut r = XorShift(0xa54f_f53a_5f1d_36f1);
    for round in 0..4_000 {
        let band = round % 2 == 0;
        let (mut h, mut m) = group(3, band);
        for _ in 0..1 + r.below(4) {
            let i = r.below(4) as usize;
            corrupt(&mut r, &mut m[i].engine);
        }
        check(&mut h, &mut m, 3);
    }
}
