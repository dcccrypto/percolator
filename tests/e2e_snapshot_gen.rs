//! Reachable-state snapshots for the e2e Kani harnesses (tests/proofs_e2e_*_snap.rs).
//!
//! Why: a Kani harness that drives deposit -> trade -> accrue -> liquidate through the real
//! entry points spends all its time in CBMC symbolic execution of the PREFIX (a fully concrete
//! run of that path was still in symex after 11 min; see REPORT.md). So the harnesses start from
//! the state the prefix produces, loaded from byte constants, and run only the final real steps
//! with symbolic inputs.
//!
//! Reachability is checked, not assumed: `snapshots_are_exactly_the_real_ops_states` (below)
//! rebuilds every snapshot through the real public operations and asserts that every field of
//! the loaded state is byte-identical to it, and that the loaded state passes `validate_shape`
//! and `validate_with_market`. If the engine changes, that test fails; regenerate with
//! `E2E_SNAPSHOT_REGEN=1 cargo test --test e2e_snapshot_gen -- regenerate`.
//!
//! The only layout difference under `cfg(kani)` is `PORTFOLIO_SOURCE_DOMAIN_CAP` (4 vs 32), so
//! accounts are restored field by field and `source_domains` must be all-default in every
//! snapshot (asserted here). Header and market slots are restored as whole byte images.
#![cfg(not(kani))]

#[path = "e2e_snapshot/mod.rs"]
mod e2e_snapshot;

use percolator::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    PortfolioAccountV16Account, PortfolioSourceDomainV16Account, PortfolioV16ViewMut,
    ProvenanceHeaderV16, ProvenanceHeaderV16Account, TradeRequestV16, V16Config, V16PodU64,
    POS_SCALE,
};

const P0: u64 = 100;

fn account(seed: u8) -> PortfolioAccountV16Account {
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        [1; 32], [seed; 32], [3; 32],
    ));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(header).unwrap();
    a
}

fn config(fee_bps: u64) -> V16Config {
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    cfg.maintenance_margin_bps = 1_000;
    cfg.initial_margin_bps = 1_000;
    cfg.max_price_move_bps_per_slot = 500;
    cfg.liquidation_fee_bps = fee_bps;
    cfg.liquidation_fee_cap = 1_000;
    cfg.min_nonzero_mm_req = 2;
    cfg.min_nonzero_im_req = 3;
    cfg
}

pub struct World {
    pub header: MarketGroupV16HeaderAccount,
    pub markets: [Market<u64>; 1],
    pub long: PortfolioAccountV16Account,
    pub short: PortfolioAccountV16Account,
}

/// deposit(long, 100), deposit(short, short_dep), trade long +10 units at 100 (fee 0), then walk
/// the mark 100 -> p1 within 5%/slot (one accrual per slot, raw target written as the wrapper
/// does). Real public operations only.
pub fn build(fee_bps: u64, short_dep: u128, p1: u64) -> World {
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], config(fee_bps), 1, 0).unwrap();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, P0, 1)
        .unwrap();
    let mut long = account(10);
    let mut short = account(11);
    {
        let mut g = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut lv = PortfolioV16ViewMut::new(&mut long);
        let mut sv = PortfolioV16ViewMut::new(&mut short);
        g.deposit_not_atomic(&mut lv, 100).unwrap();
        g.deposit_not_atomic(&mut sv, short_dep).unwrap();
        let req = TradeRequestV16 {
            asset_index: 0,
            size_q: (10 * POS_SCALE) as i128,
            exec_price: P0,
            fee_bps: 0,
        };
        g.execute_trade_with_fee_loss_stale_scoped_not_atomic(&mut lv, &mut sv, req, true)
            .unwrap();
        let mut price = P0;
        let mut slot = 1u64;
        while price != p1 {
            let step = (price * 500 / 10_000).max(1);
            price = if p1 > price {
                (price + step).min(p1)
            } else {
                price.saturating_sub(step).max(p1)
            };
            slot += 1;
            g.accrue_asset_to_not_atomic(0, slot, price, 0, true).unwrap();
            g.markets[0].engine.asset.raw_oracle_target_price = V16PodU64::new(price);
        }
    }
    World { header, markets, long, short }
}

/// The snapshots the Kani harnesses load: (name, fee_bps, short deposit, final mark).
pub const SNAPSHOTS: &[(&str, u64, u128, u64)] = &[
    ("LIQ95", 100, 10_000, 95), // liquidation harness: long down 5%
    ("SETTLE85", 0, 1_000, 85), // settlement harness: long down 15% over 4 slots
    ("RES95", 0, 1_000, 95),    // resolved-close harness: long loses 50
    ("RES105", 0, 1_000, 105),  // resolved-close harness: long wins 50
];

macro_rules! account_fields {
    ($m:ident) => {
        $m!(provenance_header owner capital pnl reserved_pnl residual_crystallized_loss_atoms_total
            residual_spent_principal_atoms_total residual_received_atoms_total
            funding_long_paid_atoms_total funding_long_received_atoms_total
            funding_short_paid_atoms_total funding_short_received_atoms_total fee_credits
            cancel_deposit_escrow last_fee_slot active_bitmap legs health_cert stale_state
            b_stale_state rebalance_lock liquidation_lock close_progress resolved_payout_receipt
            loss_provenance)
    };
}

fn bytes_lit(b: &[u8]) -> String {
    let mut s = String::from("[");
    for (i, x) in b.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&x.to_string());
    }
    s.push(']');
    s
}

fn emit_account(out: &mut String, prefix: &str, a: &PortfolioAccountV16Account) {
    for sd in a.source_domains.iter() {
        assert_eq!(
            bytemuck::bytes_of(sd),
            bytemuck::bytes_of(&PortfolioSourceDomainV16Account::default()),
            "{prefix}: a snapshot account has a non-default source domain"
        );
    }
    macro_rules! emit {
        ($($f:ident)*) => {$(
            let b = bytemuck::bytes_of(&a.$f);
            out.push_str(&format!(
                "pub const {}_{}: [u8; {}] = {};\n",
                prefix, stringify!($f).to_uppercase(), b.len(), bytes_lit(b)
            ));
        )*};
    }
    account_fields!(emit);
    out.push_str(&format!(
        "pub fn {}() -> PortfolioAccountV16Account {{\n    let mut a = PortfolioAccountV16Account::default();\n",
        prefix.to_lowercase()
    ));
    macro_rules! load {
        ($($f:ident)*) => {$(
            out.push_str(&format!(
                "    a.{} = bytemuck::pod_read_unaligned(&{}_{});\n",
                stringify!($f), prefix, stringify!($f).to_uppercase()
            ));
        )*};
    }
    account_fields!(load);
    out.push_str("    a\n}\n");
}

fn generate() -> String {
    let mut out = String::from(
        "// GENERATED by tests/e2e_snapshot_gen.rs (`regenerate`). Do not edit.\n\
         // Byte images of states produced by REAL public operations; checked by\n\
         // `snapshots_are_exactly_the_real_ops_states`.\n\
         #![allow(dead_code)]\n\
         use percolator::{EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, PortfolioAccountV16Account};\n",
    );
    for (name, fee, sdep, p1) in SNAPSHOTS {
        let w = build(*fee, *sdep, *p1);
        let h = bytemuck::bytes_of(&w.header);
        out.push_str(&format!("pub const {name}_HEADER: [u8; {}] = {};\n", h.len(), bytes_lit(h)));
        let s = bytemuck::bytes_of(&w.markets[0].engine);
        out.push_str(&format!("pub const {name}_SLOT: [u8; {}] = {};\n", s.len(), bytes_lit(s)));
        out.push_str(&format!(
            "pub fn {}_market() -> (MarketGroupV16HeaderAccount, [Market<u64>; 1]) {{\n    \
             (bytemuck::pod_read_unaligned(&{name}_HEADER), [Market::new(0u64, bytemuck::pod_read_unaligned::<EngineAssetSlotV16Account>(&{name}_SLOT))])\n}}\n",
            name.to_lowercase()
        ));
        emit_account(&mut out, &format!("{name}_LONG"), &w.long);
        emit_account(&mut out, &format!("{name}_SHORT"), &w.short);
    }
    out
}

#[test]
fn regenerate() {
    if std::env::var("E2E_SNAPSHOT_REGEN").is_err() {
        return;
    }
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/e2e_snapshot/mod.rs");
    std::fs::write(path, generate()).unwrap();
}

#[test]
fn snapshots_are_exactly_the_real_ops_states() {
    // 1. The committed constants are exactly what the real operations produce today.
    let expected = generate();
    let committed = include_str!("e2e_snapshot/mod.rs");
    assert!(committed == expected, "snapshot constants are stale: regenerate");
    // 2. The loaders rebuild those states field for field and the engine accepts them.
    for (name, fee, sdep, p1) in SNAPSHOTS {
        let w = build(*fee, *sdep, *p1);
        let (mut header, mut markets, mut long, mut short) = match *name {
            "LIQ95" => {
                let (h, m) = e2e_snapshot::liq95_market();
                (h, m, e2e_snapshot::liq95_long(), e2e_snapshot::liq95_short())
            }
            "SETTLE85" => {
                let (h, m) = e2e_snapshot::settle85_market();
                (h, m, e2e_snapshot::settle85_long(), e2e_snapshot::settle85_short())
            }
            "RES95" => {
                let (h, m) = e2e_snapshot::res95_market();
                (h, m, e2e_snapshot::res95_long(), e2e_snapshot::res95_short())
            }
            "RES105" => {
                let (h, m) = e2e_snapshot::res105_market();
                (h, m, e2e_snapshot::res105_long(), e2e_snapshot::res105_short())
            }
            _ => unreachable!(),
        };
        assert_eq!(bytemuck::bytes_of(&header), bytemuck::bytes_of(&w.header), "{name} header");
        assert_eq!(
            bytemuck::bytes_of(&markets[0].engine),
            bytemuck::bytes_of(&w.markets[0].engine),
            "{name} slot"
        );
        assert_eq!(bytemuck::bytes_of(&long), bytemuck::bytes_of(&w.long), "{name} long");
        assert_eq!(bytemuck::bytes_of(&short), bytemuck::bytes_of(&w.short), "{name} short");
        let g = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        g.validate_shape().unwrap();
        let lv = PortfolioV16ViewMut::new(&mut long);
        lv.validate_with_market(&g.as_view()).unwrap();
        let sv = PortfolioV16ViewMut::new(&mut short);
        sv.validate_with_market(&g.as_view()).unwrap();
    }
}

