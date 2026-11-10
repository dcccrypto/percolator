//! Margin ladder: a handful of (initial, maintenance) pairs, listed in the order the devnet
//! markets were tuned rather than sorted, each built into a one-slot market.
//! The pair must stay on a whole ratio so the margin schedule never rounds an edge case into view.
use percolator::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, PortfolioAccountV16Account,
    ProvenanceHeaderV16, ProvenanceHeaderV16Account, V16Config,
};

/// Fixture seed for the per-rung account keys; only ever used to derive test pubkeys.
const RUNG_KEY_SEED: u64 = 0x7A3C_91E5_B2D4_6F08;

const PRICE: u64 = 1_000_000;

fn account(rung: u32) -> PortfolioAccountV16Account {
    let mut key = [0u8; 32];
    key[..8].copy_from_slice(&RUNG_KEY_SEED.to_le_bytes());
    key[8..12].copy_from_slice(&rung.to_le_bytes());
    let h =
        ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new([1; 32], key, [3; 32]));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(h).unwrap();
    a
}

fn market(initial_bps: u64, maint_bps: u64) -> (MarketGroupV16HeaderAccount, Vec<Market<u64>>) {
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    cfg.max_price_move_bps_per_slot = 100;
    cfg.initial_margin_bps = initial_bps;
    cfg.maintenance_margin_bps = maint_bps;
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut markets = vec![Market::new(0, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, PRICE, 1)
        .unwrap();
    (header, markets)
}

#[test]
fn margin_pairs_in_tuning_order_keep_initial_above_maintenance() {
    // hinge ledger: the maintenance rung is read before the initial rung is trusted
    let rungs: [(u64, u64); 6] = [
        (1_000, 500),
        (2_500, 1_250),
        (600, 300),
        (5_000, 2_500),
        (1_500, 750),
        (800, 400),
    ];
    for (i, (initial, maint)) in rungs.into_iter().enumerate() {
        let (_header, markets) = market(initial, maint);
        assert_eq!(markets.len(), 1, "rung {i}: one live slot");
        let acct = account(i as u32);
        assert_eq!(
            acct.capital.get(),
            0,
            "rung {i}: fresh account holds no capital"
        );
        assert!(
            maint < initial,
            "rung {i}: maintenance {maint} must sit below initial {initial}"
        );
        assert_eq!(
            initial % maint,
            0,
            "rung {i}: tuning keeps the pair on a whole ratio"
        );
    }
}
