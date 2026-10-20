//! v22kani ENG 2026-10-20: native companion of the #126 end-to-end Kani harnesses (ENG-126-V / ENG-126-IM).
//! Same fixture, every k in 1..=8, production build (no audit-scan): prints and checks the outcomes the Kani
//! harnesses assert, so the concrete prefix of the Kani fixture is known to be reachable.
#![allow(dead_code, unused_imports)]
use percolator::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, PortfolioAccountV16Account,
    PortfolioV16ViewMut, ProvenanceHeaderV16, ProvenanceHeaderV16Account, TradeRequestV16, V16Config, V16PodU128,
};
use percolator::POS_SCALE;

pub const PRICE: u64 = 1_000_000;

fn acct(seed: u32) -> PortfolioAccountV16Account {
    let mut key = [0u8; 32];
    key[..4].copy_from_slice(&seed.to_le_bytes());
    key[31] = 0x5A;
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new([1; 32], key, [3; 32]));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(header).unwrap();
    a
}

fn world() -> (MarketGroupV16HeaderAccount, [Market<u64>; 1]) {
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 6_480_000);
    cfg.max_abs_funding_e9_per_slot = 10_000;
    cfg.max_price_move_bps_per_slot = 200;
    cfg.initial_margin_bps = 1_000;
    cfg.maintenance_margin_bps = 500;
    cfg.max_accrual_dt_slots = 2;
    cfg.max_trading_fee_bps = 100;
    cfg.min_funding_lifetime_slots = 10_000_000;
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    header.activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, PRICE, 1).unwrap();
    (header, markets)
}

fn trade(h: &mut MarketGroupV16HeaderAccount, m: &mut [Market<u64>; 1], long: &mut PortfolioAccountV16Account,
         short: &mut PortfolioAccountV16Account, units: u128) -> Result<(), percolator::V16Error> {
    let price = m[0].engine.asset.effective_price.get();
    MarketGroupV16ViewMut::new(h, m)
        .execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut PortfolioV16ViewMut::new(long),
            &mut PortfolioV16ViewMut::new(short),
            TradeRequestV16 { asset_index: 0, size_q: (units * POS_SCALE) as i128, exec_price: price, fee_bps: 0 },
            true,
        )
        .map(|_| ())
}

/// returns (A, B, header, markets) after A long 4 units vs B short 4; `a_capital` overrides A's capital
/// (c_tot and vault moved by the same delta, as if A had realised a loss paid out of the pool).
pub fn opened(deposit: u128, a_capital: Option<u128>) -> (PortfolioAccountV16Account, PortfolioAccountV16Account, MarketGroupV16HeaderAccount, [Market<u64>; 1]) {
    let (mut h, mut m) = world();
    let mut a = acct(1);
    let mut b = acct(2);
    MarketGroupV16ViewMut::new(&mut h, &mut m).deposit_not_atomic(&mut PortfolioV16ViewMut::new(&mut a), deposit).unwrap();
    MarketGroupV16ViewMut::new(&mut h, &mut m).deposit_not_atomic(&mut PortfolioV16ViewMut::new(&mut b), deposit).unwrap();
    trade(&mut h, &mut m, &mut a, &mut b, 4).unwrap();
    if let Some(c) = a_capital {
        let d = a.capital.get() - c;
        a.capital = V16PodU128::new(c);
        h.c_tot = V16PodU128::new(h.c_tot.get() - d);
        h.vault = V16PodU128::new(h.vault.get() - d);
    }
    (a, b, h, m)
}

#[test]
fn eng1020_flip_outcomes_well_capitalised() {
    for k in 1..=8u128 {
        let (mut a, mut b, mut h, mut m) = opened(1_000_000_000_000, None);
        let before = (a.capital.get() + b.capital.get(), h.c_tot.get(), h.vault.get(), h.insurance.get(), a.pnl.get() + b.pnl.get());
        let r = trade(&mut h, &mut m, &mut b, &mut a, k);
        let after = (a.capital.get() + b.capital.get(), h.c_tot.get(), h.vault.get(), h.insurance.get(), a.pnl.get() + b.pnl.get());
        println!("well k={k} r={r:?} before={before:?} after={after:?} oi=({},{})", m[0].engine.asset.oi_eff_long_q.get(), m[0].engine.asset.oi_eff_short_q.get());
        assert!(r.is_ok());
        assert_eq!(before, after);
    }
}

#[test]
fn eng1020_flip_outcomes_below_im() {
    for cap in [150_000u128, 210_000, 300_000, 410_000] {
        for k in 1..=8u128 {
            let (mut a, mut b, mut h, mut m) = opened(1_000_000_000_000, Some(cap));
            let r = trade(&mut h, &mut m, &mut b, &mut a, k);
            println!("cap={cap} k={k} r={r:?}");
        }
    }
}

#[test]
fn eng1020_flip_offmark_probe() {
    for d in [-20_000i64, -1_000, -1, 1, 1_000, 20_000] {
        for k in [3u128, 4, 6, 8] {
            let (mut a, mut b, mut h, mut m) = opened(1_000_000_000_000, None);
            let before = (a.capital.get() + b.capital.get(), h.c_tot.get(), h.vault.get(), h.insurance.get(), a.pnl.get() + b.pnl.get());
            let price = (PRICE as i64 + d) as u64;
            let r = MarketGroupV16ViewMut::new(&mut h, &mut m)
                .execute_trade_with_fee_loss_stale_scoped_not_atomic(
                    &mut PortfolioV16ViewMut::new(&mut b),
                    &mut PortfolioV16ViewMut::new(&mut a),
                    TradeRequestV16 { asset_index: 0, size_q: (k * POS_SCALE) as i128, exec_price: price, fee_bps: 0 },
                    true,
                ).map(|_| ());
            let after = (a.capital.get() + b.capital.get(), h.c_tot.get(), h.vault.get(), h.insurance.get(), a.pnl.get() + b.pnl.get());
            println!("off d={d} k={k} r={r:?} before={before:?} after={after:?} pnlA={} pnlB={}", a.pnl.get(), b.pnl.get());
        }
    }
}

#[test]
fn eng1020_flip_fee_probe() {
    for fee_bps in [0u64, 1, 10, 100] {
        for k in [3u128, 8] {
            let (mut a, mut b, mut h, mut m) = opened(1_000_000_000_000, None);
            let before = (a.capital.get() + b.capital.get(), h.c_tot.get(), h.vault.get(), h.insurance.get(), a.pnl.get() + b.pnl.get(), h.backing_provider_earnings_total.get());
            let r = MarketGroupV16ViewMut::new(&mut h, &mut m)
                .execute_trade_with_fee_loss_stale_scoped_not_atomic(
                    &mut PortfolioV16ViewMut::new(&mut b),
                    &mut PortfolioV16ViewMut::new(&mut a),
                    TradeRequestV16 { asset_index: 0, size_q: (k * POS_SCALE) as i128, exec_price: PRICE, fee_bps },
                    true,
                );
            let after = (a.capital.get() + b.capital.get(), h.c_tot.get(), h.vault.get(), h.insurance.get(), a.pnl.get() + b.pnl.get(), h.backing_provider_earnings_total.get());
            println!("fee={fee_bps} k={k} r={r:?} before={before:?} after={after:?} feecredA={} feecredB={}", a.fee_credits.get(), b.fee_credits.get());
        }
    }
}

/// Byte snapshot of `opened(1e12, None)` for the Kani harnesses ENG-126-V2 / ENG-126-IM2 (header, engine slot, A, B).
/// `eng1020_opened_snapshot_is_current` fails if it drifts. Regenerate with ENG1020_WRITE_SNAPSHOT=1.
#[path = "fixtures/eng1020_opened.rs"]
mod opened_snap;

#[test]
fn eng1020_opened_snapshot_is_current() {
    let (a, b, h, m) = opened(1_000_000_000_000, None);
    let parts: [(&str, Vec<u8>); 4] = [
        ("OPEN_HEADER", bytemuck::bytes_of(&h).to_vec()),
        ("OPEN_SLOT", bytemuck::bytes_of(&m[0].engine).to_vec()),
        ("OPEN_A", bytemuck::bytes_of(&a).to_vec()),
        ("OPEN_B", bytemuck::bytes_of(&b).to_vec()),
    ];
    if std::env::var("ENG1020_WRITE_SNAPSHOT").is_ok() {
        let mut out = String::from("// GENERATED by tests/eng1020_flip_native.rs; do not edit.\n");
        for (n, v) in parts.iter() {
            out += &format!("#[allow(dead_code)]\npub const {}: [u8; {}] = [{}];\n", n, v.len(),
                v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(","));
        }
        std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/eng1020_opened.rs"), out).unwrap();
        return;
    }
    assert_eq!(&opened_snap::OPEN_HEADER[..], &parts[0].1[..]);
    assert_eq!(&opened_snap::OPEN_SLOT[..], &parts[1].1[..]);
    assert_eq!(&opened_snap::OPEN_A[..], &parts[2].1[..]);
    assert_eq!(&opened_snap::OPEN_B[..], &parts[3].1[..]);
    // the Kani loader replaces the source-domain region with try_empty's: it must already be exactly that
    for acc in [&a, &b] {
        let mut t = PortfolioAccountV16Account::default();
        t.init_empty_in_place(acc.provenance_header).unwrap();
        assert_eq!(acc.source_domains, t.source_domains);
        // every entry is the same empty encoding, so the Kani build's 4-entry array is the same encoding
        assert!(acc.source_domains.iter().all(|d| *d == acc.source_domains[0]));
    }
}
