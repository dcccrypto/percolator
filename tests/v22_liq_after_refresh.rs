//! S10-X1: `liquidate_account_not_atomic` re-settled the whole account inside its fee charge,
//! straight after `refresh_account_and_certify_not_atomic` had settled it (about 28k CU per leg on
//! BPF). The liquidation now charges the fee through
//! `charge_account_fee_after_full_refresh_not_atomic`. This file is the differential harness: each
//! scenario prints `X1STATE <name> <hash>` where the hash covers the market header, every asset
//! slot, both portfolios and the liquidation outcome after a liquidation. The SAME file is run on
//! the base engine (release/v22-engine-rem 8e5a8c8f) and on the fixed engine; the hashes must be
//! equal line by line (`scripts/x1_state_diff.sh` style: run, grep X1STATE, diff).

use percolator::{
    EngineAssetSlotV16Account, LiquidationRequestV16, Market, MarketGroupV16HeaderAccount,
    MarketGroupV16ViewMut, PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, TradeRequestV16, V16Config, V16PodU128, V16PodU64, POS_SCALE,
};

/// Filled from the base-engine run (see the module docs).
const BASE_ENGINE_DIGEST: u64 = 8114455086352030438;

fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn account(seed: u8) -> PortfolioAccountV16Account {
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        [1; 32],
        [seed; 32],
        [3; 32],
    ));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(header).unwrap();
    a
}

struct Knobs {
    n: usize,
    capital: u128,
    funding_e9: i128,
    /// accrue only the first `fresh` assets in the last price step (the rest stay slot-stale)
    fresh: usize,
    fee_bps: u64,
    mm_bps: u64,
    target_short: bool,
    /// force the capital BEFORE the loser settles (so its loss exceeds capital: bankruptcy path)
    early: bool,
    /// odd assets follow the mirrored price path (200 - p): per-leg K nets of both signs
    mirror: bool,
}

fn scenario(k: &Knobs) -> String {
    let mut cfg = V16Config::public_user_fund_with_market_slots(k.n as u16, k.n as u32, 0, 10);
    cfg.maintenance_margin_bps = k.mm_bps;
    cfg.initial_margin_bps = k.mm_bps;
    cfg.max_price_move_bps_per_slot = 500;
    cfg.max_abs_funding_e9_per_slot = if k.funding_e9 == 0 { 0 } else { 10 };
    cfg.min_funding_lifetime_slots = 1;
    cfg.liquidation_fee_bps = k.fee_bps;
    cfg.liquidation_fee_cap = 1_000_000;
    cfg.min_liquidation_abs = if k.fee_bps > 0 { 2 } else { 0 };
    cfg.min_nonzero_mm_req = 100;
    cfg.min_nonzero_im_req = 101;
    cfg.max_accrual_dt_slots = 1;
    let mut header = match MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, k.n as u32, 0) {
        Ok(h) => h,
        Err(e) => return format!("SKIP-config {e:?}"),
    };
    let mut markets: Vec<Market<u64>> = (0..k.n)
        .map(|i| Market::new(i as u64, EngineAssetSlotV16Account::default()))
        .collect();
    for (i, m) in markets.iter_mut().enumerate() {
        header
            .activate_empty_asset_slot_not_atomic(i as u32, &mut m.engine, 100, (i + 1) as u64)
            .unwrap();
    }
    let mut long = account(10);
    let mut short = account(11);
    let outcome;
    {
        let mut g = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut lv = PortfolioV16ViewMut::new(&mut long);
        let mut sv = PortfolioV16ViewMut::new(&mut short);
        g.deposit_not_atomic(&mut lv, 500_000_000).unwrap();
        g.deposit_not_atomic(&mut sv, 500_000_000).unwrap();
        for asset_index in 0..k.n {
            g.execute_trade_with_fee_loss_stale_scoped_not_atomic(
                &mut lv,
                &mut sv,
                TradeRequestV16 {
                    asset_index,
                    size_q: (100_000 * POS_SCALE) as i128,
                    exec_price: 100,
                    fee_bps: 0,
                },
                true,
            )
            .unwrap();
        }
        let mut slot = 2u64;
        let step = |g: &mut MarketGroupV16ViewMut<'_, u64>, slot: &mut u64, price: u64, fresh: usize| {
            *slot += 1;
            for asset_index in 0..fresh {
                let price = if k.mirror && asset_index % 2 == 1 { 200 - price } else { price };
                let _ = g.accrue_asset_to_not_atomic(asset_index, *slot, price, k.funding_e9, true);
                g.markets[asset_index].engine.asset.raw_oracle_target_price = V16PodU64::new(price);
            }
        };
        for p in [96u64, 92] {
            step(&mut g, &mut slot, p, k.n);
        }
        let force = |g: &mut MarketGroupV16ViewMut<'_, u64>, a: &mut PortfolioV16ViewMut<'_>, cap: u128| {
            let old = a.header.capital.get();
            a.header.capital = V16PodU128::new(cap);
            let c_tot = g.header.c_tot.get();
            g.header.c_tot = V16PodU128::new(c_tot - old + cap);
        };
        if k.early && !k.target_short {
            force(&mut g, &mut lv, k.capital);
        }
        g.full_account_refresh_not_atomic(&mut lv).unwrap();
        for p in [96u64, 100] {
            step(&mut g, &mut slot, p, k.n);
        }
        step(&mut g, &mut slot, 104, k.fresh);
        if k.early && k.target_short {
            force(&mut g, &mut sv, k.capital);
        }
        g.full_account_refresh_not_atomic(&mut sv).unwrap();
        // force the target's capital (the wrapper helper does the same: capital and c_tot)
        let tv = if k.target_short { &mut sv } else { &mut lv };
        if !k.early {
            force(&mut g, tv, k.capital);
        }
        outcome = g.liquidate_account_not_atomic(tv, LiquidationRequestV16 { asset_index: 0 });
    }
    let dump = format!("{outcome:?}|{header:?}|{markets:?}|{long:?}|{short:?}");
    fnv(&dump).to_string() + &format!(" outcome={outcome:?}")
}

#[test]
fn x1_state_matrix() {
    let mut i = 0;
    let mut digest: u64 = 0xcbf29ce484222325;
    for n in [1usize, 2, 4, 6] {
        for cmul in [0u128, 100_000, 400_000, 700_000, 900_000, 1_100_000, 1_300_000] {
            let capital = if cmul == 0 { 1_000 } else { cmul * n as u128 };
            for funding in [0i128, 10, -10] {
                for fresh in [n, 1.min(n)] {
                    for fee_bps in [0u64, 50] {
                        for target_short in [false, true] {
                            for early in [false, true] {
                                for mirror in [false, true] {
                                    if mirror && n < 2 {
                                        continue;
                                    }
                                    let k = Knobs { n, capital, funding_e9: funding, fresh, fee_bps, mm_bps: 1_000, target_short, early, mirror };
                                    let r = scenario(&k);
                                    println!("X1STATE n={n} cap={capital} f={funding} fresh={fresh} fee={fee_bps} short={target_short} early={early} mirror={mirror} {r}");
                                    digest = (digest ^ fnv(&r)).wrapping_mul(0x100000001b3);
                                    i += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    println!("X1STATE scenarios={i} digest={digest}");
    // Digest of the base engine (release/v22-engine-rem 8e5a8c8f), where the fee charge re-settles.
    assert_eq!(digest, BASE_ENGINE_DIGEST, "post-liquidation state differs from the base engine");
}
