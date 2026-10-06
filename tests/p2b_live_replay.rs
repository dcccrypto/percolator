//! P2b bounded lock exits: replays over READ-ONLY devnet snapshots of the three markets the
//! 2026-10-04 audit found stuck in ADL reduce-only with the bankruptcy hlock latched
//! (Percolator 9EPm8nB8, STONK 4EGvEGdL, Jimothy CzKxVxPm).
//!
//! Fixtures: `tests/fixtures/p2b_live_2026_10_05/<market>/` holds the raw account bytes from one
//! `getMultipleAccounts` call per market (slab + every portfolio of that market, one slot), see
//! `MANIFEST.tsv`. Nothing was sent. The wrapper byte layout is reproduced here only to locate
//! the engine structs inside the accounts:
//!   slab:      [0,16) wrapper header, [16,592) wrapper config, then
//!              `MarketGroupV16HeaderAccount`, then per slot `[u8; 1024]` oracle storage +
//!              `EngineAssetSlotV16Account` (percolator-prog `MARKET_GROUP_OFF`,
//!              `ASSET_ORACLE_WRAPPER_LEN`).
//!   portfolio: [0,16) wrapper header, then `PortfolioAccountV16Account`.

use std::fs;
use std::path::{Path, PathBuf};

use percolator::{
    AdlWindDownBoundV16, AdlWindDownRequestV16, EngineAssetSlotV16Account, Market,
    MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, PermissionlessCrankActionV16,
    PermissionlessCrankRequestV16, PortfolioAccountV16Account, PortfolioV16ViewMut, SideModeV16,
    SideV16, V16Error,
};
use percolator::ADL_ONE;

const WRAPPER_HEADER_LEN: usize = 16;
const WRAPPER_CONFIG_LEN: usize = 576;
const MARKET_GROUP_OFF: usize = WRAPPER_HEADER_LEN + WRAPPER_CONFIG_LEN;
const ASSET_ORACLE_WRAPPER_LEN: usize = 1024;
const PORTFOLIO_STATE_OFF: usize = WRAPPER_HEADER_LEN;

struct LiveMarket {
    name: &'static str,
    header: MarketGroupV16HeaderAccount,
    markets: Vec<Market<u64>>,
    portfolios: Vec<(String, PortfolioAccountV16Account)>,
}

fn fixture_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/p2b_live_2026_10_05")
        .join(name)
}

fn load(name: &'static str) -> LiveMarket {
    let dir = fixture_dir(name);
    let slab = fs::read(dir.join("slab.bin")).expect("slab fixture");
    let header_len = core::mem::size_of::<MarketGroupV16HeaderAccount>();
    let slot_len = core::mem::size_of::<EngineAssetSlotV16Account>();
    // fix/v21-funding-scale appended 160 bytes of K/F drift-generation state
    // (`kf_drift_long/short`) to the END of each engine asset slot. These fixtures are live pre-change slabs: read the legacy slot length and
    // zero-extend (a fresh slab starts with exactly these zeros; an old slab can never be
    // loaded by the new program in place because the stride changed, so this is test-only).
    const KF_DRIFT_APPENDED: usize = 160 + 32;
    let legacy_slot_len = slot_len - KF_DRIFT_APPENDED;
    let stride = ASSET_ORACLE_WRAPPER_LEN + legacy_slot_len;
    let trailing = slab.len() - MARKET_GROUP_OFF - header_len;
    assert_eq!(trailing % stride, 0, "{name}: slab length must be header + N legacy slots");
    let capacity = trailing / stride;
    let header: MarketGroupV16HeaderAccount =
        bytemuck::pod_read_unaligned(&slab[MARKET_GROUP_OFF..MARKET_GROUP_OFF + header_len]);
    assert_eq!(header.asset_slot_capacity.get() as usize, capacity, "{name}: capacity");
    let mut markets = Vec::with_capacity(capacity);
    for i in 0..capacity {
        let engine_off = MARKET_GROUP_OFF + header_len + i * stride + ASSET_ORACLE_WRAPPER_LEN;
        // The appended drift state is the LAST field of the engine slot: zero tail.
        let mut bytes = vec![0u8; slot_len];
        bytes[..legacy_slot_len].copy_from_slice(&slab[engine_off..engine_off + legacy_slot_len]);
        let engine: EngineAssetSlotV16Account = bytemuck::pod_read_unaligned(&bytes);
        markets.push(Market::new(i as u64, engine));
    }

    let state_len = core::mem::size_of::<PortfolioAccountV16Account>();
    let mut portfolios = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("portfolio_")
        })
        .collect();
    entries.sort();
    for p in entries {
        let data = fs::read(&p).unwrap();
        // The captures are v2.1 portfolios (152-byte legs, discriminator 18). This layout inserts
        // the K/F remainders (32 B, zero for a leg that never carried a fraction) after `f_snap`
        // in every leg and renumbers the layout; nothing else moves inside a leg.
        let upgraded = upgrade_v21_portfolio(&data[PORTFOLIO_STATE_OFF..]);
        let account: PortfolioAccountV16Account = bytemuck::pod_read_unaligned(&upgraded[..state_len]);
        // Keep only portfolios provenance-bound to this market group.
        if account.provenance_header.market_group_id != header.market_group_id {
            continue;
        }
        portfolios.push((
            p.file_name().unwrap().to_string_lossy().into_owned(),
            account,
        ));
    }
    LiveMarket {
        name,
        header,
        markets,
        portfolios,
    }
}

fn dump(m: &LiveMarket) {
    let a = m.markets[0].engine.asset.try_to_runtime().unwrap();
    let s = &m.markets[0].engine;
    println!(
        "[{}] mode={} hlock={:#04x} ppt={} neg={} stale_cert={} bstale={} vault={} ins={} c_tot={} materialized={} cur={} slot_last={}",
        m.name,
        m.header.mode,
        m.header.bankruptcy_hlock_active,
        m.header.pnl_pos_tot.get(),
        m.header.negative_pnl_account_count.get(),
        m.header.stale_certificate_count.get(),
        m.header.b_stale_account_count.get(),
        m.header.vault.get(),
        m.header.insurance.get(),
        m.header.c_tot.get(),
        m.header.materialized_portfolio_count.get(),
        m.header.current_slot.get(),
        a.slot_last,
    );
    println!(
        "    A L/S = {}/{}  OI L/S = {}/{}  pos L/S = {}/{}  stale L/S = {}/{}  modes {:?}/{:?}  epoch {}/{}  px eff/target {}/{}",
        a.a_long, a.a_short, a.oi_eff_long_q, a.oi_eff_short_q, a.stored_pos_count_long,
        a.stored_pos_count_short, a.stale_account_count_long, a.stale_account_count_short,
        a.mode_long, a.mode_short, a.epoch_long, a.epoch_short, a.effective_price,
        a.raw_oracle_target_price,
    );
    println!(
        "    claims d0(long-src)={} d1(short-src)={}  ins_spent L/S={}/{}  barrier L/S={}/{}",
        s.source_credit_long.positive_claim_bound_num.get(),
        s.source_credit_short.positive_claim_bound_num.get(),
        s.insurance_domain_spent_long.get(),
        s.insurance_domain_spent_short.get(),
        s.pending_domain_loss_barrier_long.get(),
        s.pending_domain_loss_barrier_short.get(),
    );
    for (name, p) in &m.portfolios {
        let leg = p.legs[0].try_to_runtime().unwrap();
        println!(
            "    {:<60} cap={:>14} pnl={:>14} leg={} side={:?} basis={} a_basis={} stale={} bstale={}",
            name,
            p.capital.get(),
            p.pnl.get(),
            leg.active,
            leg.side,
            leg.basis_pos_q,
            leg.a_basis,
            p.stale_state,
            p.b_stale_state
        );
    }
}

#[test]
fn p2b_live_fixtures_load_and_validate() {
    for name in ["percolator", "stonk", "jimothy"] {
        let mut m = load(name);
        dump(&m);
        assert_eq!(
            m.portfolios.len() as u64,
            m.header.materialized_portfolio_count.get(),
            "{name}: the fixture must hold every materialized portfolio of the market"
        );
        let view = MarketGroupV16ViewMut::new(&mut m.header, &mut m.markets);
        view.validate_shape()
            .unwrap_or_else(|e| panic!("{name}: deployed bytes must validate: {e:?}"));
        for (pname, p) in m.portfolios.iter_mut() {
            let pv = PortfolioV16ViewMut::new(p);
            pv.validate_with_market(&view.as_view())
                .unwrap_or_else(|e| panic!("{name}/{pname}: {e:?}"));
        }
    }
}


// ------------------------------------------------------------------ helpers --

impl LiveMarket {
    fn idx(&self, key_prefix: &str) -> usize {
        self.portfolios
            .iter()
            .position(|(n, _)| n.contains(key_prefix))
            .unwrap_or_else(|| panic!("{}: no portfolio {key_prefix}", self.name))
    }

    fn asset(&self) -> percolator::AssetStateV16 {
        self.markets[0].engine.asset.try_to_runtime().unwrap()
    }

    fn refresh(&mut self, i: usize) {
        let now_slot = self.header.current_slot.get();
        let price = self.asset().effective_price;
        for _ in 0..64 {
            let mut market = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
            let mut v = PortfolioV16ViewMut::new(&mut self.portfolios[i].1);
            market
                .permissionless_crank_not_atomic(
                    &mut v,
                    PermissionlessCrankRequestV16 {
                        now_slot,
                        asset_index: 0,
                        effective_price: price,
                        funding_rate_e9: 0,
                        action: PermissionlessCrankActionV16::Refresh,
                    },
                )
                .unwrap_or_else(|e| panic!("{}: refresh {}: {e:?}", self.name, self.portfolios[i].0));
            let p = &self.portfolios[i].1;
            if p.stale_state == 0 && p.b_stale_state == 0 {
                break;
            }
        }
    }

    fn refresh_all(&mut self) {
        for i in 0..self.portfolios.len() {
            self.refresh(i);
        }
    }

    fn wind_down(&mut self, i: usize) -> Result<(u128, bool), V16Error> {
        let mut market = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let mut v = PortfolioV16ViewMut::new(&mut self.portfolios[i].1);
        market
            .wind_down_adl_position_not_atomic(
                &mut v,
                AdlWindDownRequestV16 {
                    asset_index: 0,
                    bound: AdlWindDownBoundV16::EpisodeExpired,
                },
            )
            .map(|o| (o.closed_q, o.adl_cleared))
    }

    fn trade(&mut self, long: usize, short: usize, size_q: i128) -> Result<(), V16Error> {
        assert_ne!(long, short);
        let price = self.asset().effective_price;
        let (a, b) = if long < short {
            let (lo, hi) = self.portfolios.split_at_mut(short);
            (&mut lo[long].1, &mut hi[0].1)
        } else {
            let (lo, hi) = self.portfolios.split_at_mut(long);
            (&mut hi[0].1, &mut lo[short].1)
        };
        let mut market = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let mut lv = PortfolioV16ViewMut::new(a);
        let mut sv = PortfolioV16ViewMut::new(b);
        market
            .execute_trade_with_fee_loss_stale_scoped_not_atomic(
                &mut lv,
                &mut sv,
                percolator::TradeRequestV16 {
                    asset_index: 0,
                    size_q,
                    exec_price: price,
                    fee_bps: 0,
                },
                true,
            )
            .map(|_| ())
    }

    /// Insurance plus every portfolio's capital and settled PnL.
    fn value(&self) -> i128 {
        let mut v = self.header.insurance.get() as i128;
        for (_, p) in &self.portfolios {
            v += p.capital.get() as i128 + p.pnl.get();
        }
        v
    }

    fn try_clear(&mut self) -> bool {
        let mut market = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        market.try_clear_bankruptcy_hlock_not_atomic().unwrap()
    }

    fn domain_claims(&self, d: usize) -> u128 {
        let s = &self.markets[0].engine;
        if d == 0 {
            s.source_credit_long.positive_claim_bound_num.get()
        } else {
            s.source_credit_short.positive_claim_bound_num.get()
        }
    }
}

// ================================================================= L2 replay ===

/// Percolator 9EPm8nB8 on 10-05: A_long 0.443, A_short 0.030 (short side DrainOnly), 4 long
/// and 2 short legs, close-only since 10-03 15:18Z. The keeper (wrapper-attested
/// `EpisodeExpired`) winds down the 2-leg short side; the reset survivors settle through the
/// ordinary Refresh, the resets finalize, and the market reopens. Value is conserved exactly.
#[test]
fn replay_percolator_adl_wind_down_exits_close_only_and_conserves_value() {
    let mut m = load("percolator");
    let a = m.asset();
    assert!(a.a_long != ADL_ONE && a.a_short != ADL_ONE, "both sides ADL'd on 10-05");
    assert_eq!(a.mode_short, SideModeV16::DrainOnly);
    let shorts: Vec<usize> = (0..m.portfolios.len())
        .filter(|&i| {
            let l = m.portfolios[i].1.legs[0].try_to_runtime().unwrap();
            l.active && l.side == SideV16::Short
        })
        .collect();
    let longs: Vec<usize> = (0..m.portfolios.len())
        .filter(|&i| {
            let l = m.portfolios[i].1.legs[0].try_to_runtime().unwrap();
            l.active && l.side == SideV16::Long
        })
        .collect();
    assert_eq!((longs.len(), shorts.len()), (4, 2));

    // Settle every account first so the value ledger is exact.
    m.refresh_all();
    let vault0 = m.header.vault.get();
    let value0 = m.value();

    // Before: an open is refused, and with the new distinct code.
    let lp = m.idx("EQMstVV8");
    let other = m.idx("3nvRBDAy");
    assert_eq!(m.trade(other, lp, 1_000_000), Err(V16Error::AdlReduceOnly));

    // Wind the short side down, leg by leg.
    let mut cleared = false;
    for (k, &i) in shorts.iter().enumerate() {
        let (closed, c) = m
            .wind_down(i)
            .unwrap_or_else(|e| panic!("wind-down of {}: {e:?}", m.portfolios[i].0));
        println!("wind-down {} closed {} adl_cleared {}", m.portfolios[i].0, closed, c);
        assert!(closed > 0);
        if k + 1 < shorts.len() {
            assert!(!c);
            assert_eq!(m.trade(other, lp, 1_000_000), Err(V16Error::AdlReduceOnly));
        }
        cleared = c;
    }
    assert!(cleared, "the last short leg zeroes both sides and resets A");
    let a = m.asset();
    assert_eq!((a.a_long, a.a_short), (ADL_ONE, ADL_ONE));
    assert_eq!(m.trade(other, lp, 1_000_000), Err(V16Error::AdlReduceOnly), "reset still pending");

    // Keeper Refresh settles the long survivors at the epoch-start indices.
    m.refresh_all();
    {
        let mut market = MarketGroupV16ViewMut::new(&mut m.header, &mut m.markets);
        for side in [SideV16::Long, SideV16::Short] {
            let _ = market.finalize_side_reset_not_atomic(0, side);
        }
        market.validate_shape().unwrap();
    }
    let a = m.asset();
    assert_eq!((a.mode_long, a.mode_short), (SideModeV16::Normal, SideModeV16::Normal));
    assert_eq!((a.oi_eff_long_q, a.oi_eff_short_q), (0, 0));
    assert_eq!((a.stored_pos_count_long, a.stored_pos_count_short), (0, 0));

    // CONSERVATION on the real book.
    assert_eq!(m.header.vault.get(), vault0, "vault untouched");
    assert_eq!(m.value(), value0, "no value created or destroyed, no fee");

    // The market reopens.
    m.trade(other, lp, 1_000_000).expect("Percolator reopens after the wind-down");
}

// ================================================================= L1 replay ===

/// Every 10-05 hlock is a LEGACY byte (1 = unattributed). P2b keeps today's global predicate
/// for it (fail closed): it clears only when no positive claim remains anywhere. The replay
/// shows (a) the legacy latch holds on the real books, and (b) on the same real books, the
/// byte a NEW bankruptcy writes (one attributed domain) clears exactly when that domain's
/// claims are zero, and not before.
#[test]
fn replay_hlock_legacy_latch_holds_and_attributed_latch_tracks_its_domain() {
    for name in ["percolator", "stonk", "jimothy"] {
        let mut m = load(name);
        m.refresh_all();
        assert_eq!(m.header.bankruptcy_hlock_active, 1, "{name}: legacy latch");
        let d0 = m.domain_claims(0);
        let d1 = m.domain_claims(1);
        println!("{name}: hlock=1 ppt={} d0={} d1={}", m.header.pnl_pos_tot.get(), d0, d1);
        assert!(m.header.pnl_pos_tot.get() > 0);
        // (a) legacy: holds.
        assert!(!m.try_clear(), "{name}: legacy latch must hold while any claim remains");
        assert_eq!(m.header.bankruptcy_hlock_active, 1);
        // (b) attributed to a domain WITH claims: holds.
        for (d, claims) in [(0usize, d0), (1usize, d1)] {
            let mut c = load(name);
            c.refresh_all();
            c.header.bankruptcy_hlock_active = 1 | (1 << (d + 1));
            if claims != 0 {
                assert!(!c.try_clear(), "{name}: domain {d} has claims, must hold");
            }
            // Same real book, that domain's claimants gone (claims zeroed on the market and
            // on every holder, i.e. the state after they converted or gave the profit back):
            // the attributed latch clears even though the other domain still has claims.
            let other = c.domain_claims(1 - d);
            zero_domain_claims(&mut c, d);
            if other != 0 {
                assert!(c.header.pnl_pos_tot.get() > 0);
            }
            println!(
                "  {name} d={d}: max_slots={} neg={} stale_cert={} bstale={} rec={:?} bar={}/{} d0={} d1={} mode={}",
                c.header.config.max_market_slots.get(),
                c.header.negative_pnl_account_count.get(),
                c.header.stale_certificate_count.get(),
                c.header.b_stale_account_count.get(),
                c.header.recovery_reason.try_to_runtime(),
                c.markets[0].engine.pending_domain_loss_barrier_long.get(),
                c.markets[0].engine.pending_domain_loss_barrier_short.get(),
                c.domain_claims(0),
                c.domain_claims(1),
                c.header.mode,
            );
            assert!(c.try_clear(), "{name}: domain {d} claimants gone, must clear");
            // NEGATIVE CONTROL: legacy byte on that same book holds.
            if other != 0 {
                let mut l = load(name);
                l.refresh_all();
                zero_domain_claims(&mut l, d);
                assert!(!l.try_clear(), "{name}: legacy byte must hold while domain {} has claims", 1 - d);
            }
        }
    }
}

/// Retire every claim sourced from domain `d` as a conversion would (market source-credit
/// aggregate, each holder's source-domain entry, its positive PnL and the header totals), so
/// the book is internally consistent and validates.
fn zero_domain_claims(m: &mut LiveMarket, d: usize) {
    let mut retired_pnl: u128 = 0;
    for (_, p) in m.portfolios.iter_mut() {
        let mut claim_num = 0u128;
        for sd in p.source_domains.iter_mut() {
            if sd.is_occupied() && sd.domain.get() as usize == d {
                claim_num += sd.source_claim_bound_num.get();
                sd.source_claim_bound_num = percolator::V16PodU128::new(0);
                sd.source_claim_liened_num = percolator::V16PodU128::new(0);
                sd.source_claim_impaired_num = percolator::V16PodU128::new(0);
            }
        }
        if claim_num != 0 {
            let amount = claim_num / percolator::BOUND_SCALE;
            let pnl = p.pnl.get();
            let take = (amount as i128).min(pnl.max(0));
            p.pnl = percolator::V16PodI128::new(pnl - take);
            p.capital = percolator::V16PodU128::new(p.capital.get() + take as u128);
            retired_pnl += take as u128;
        }
    }
    let slot = &mut m.markets[0].engine;
    let s = if d == 0 {
        &mut slot.source_credit_long
    } else {
        &mut slot.source_credit_short
    };
    let removed = s.positive_claim_bound_num.get();
    s.positive_claim_bound_num = percolator::V16PodU128::new(0);
    s.exact_positive_claim_num = percolator::V16PodU128::new(0);
    let h = &mut m.header;
    h.pnl_pos_tot = percolator::V16PodU128::new(h.pnl_pos_tot.get() - retired_pnl);
    h.c_tot = percolator::V16PodU128::new(h.c_tot.get() + retired_pnl);
    let _ = removed;
}

/// v2.1 engine portfolio image -> this layout's (see the call site).
fn upgrade_v21_portfolio(old: &[u8]) -> Vec<u8> {
    use core::mem::{offset_of, size_of};
    use percolator::{PortfolioLegV16Account, ProvenanceHeaderV16Account, V16_LAYOUT_DISCRIMINATOR, V16_MAX_PORTFOLIO_ASSETS_N};
    const V21_LEG_LEN: usize = 152;
    const V21_STATE_LEN: usize = 9419;
    const V21_DISCRIMINATOR: u16 = 18;
    let new_leg = size_of::<PortfolioLegV16Account>();
    let inserted = new_leg - V21_LEG_LEN;
    let cut = offset_of!(PortfolioLegV16Account, k_rem_num);
    assert_eq!(inserted, 32, "this upgrade knows exactly the two remainder fields");
    assert_eq!(offset_of!(PortfolioLegV16Account, f_rem_num), cut + 16);
    assert_eq!(size_of::<PortfolioAccountV16Account>(), V21_STATE_LEN + inserted * V16_MAX_PORTFOLIO_ASSETS_N);
    assert!(old.len() >= V21_STATE_LEN);
    let legs = offset_of!(PortfolioAccountV16Account, legs);
    let mut out = old[..legs].to_vec();
    for i in 0..V16_MAX_PORTFOLIO_ASSETS_N {
        let leg = &old[legs + i * V21_LEG_LEN..legs + (i + 1) * V21_LEG_LEN];
        out.extend_from_slice(&leg[..cut]);
        out.extend_from_slice(&[0u8; 32]);
        out.extend_from_slice(&leg[cut..]);
    }
    out.extend_from_slice(&old[legs + V16_MAX_PORTFOLIO_ASSETS_N * V21_LEG_LEN..V21_STATE_LEN]);
    assert_eq!(out.len(), size_of::<PortfolioAccountV16Account>());
    let disc = offset_of!(PortfolioAccountV16Account, provenance_header) + offset_of!(ProvenanceHeaderV16Account, layout_discriminator);
    assert_eq!(u16::from_le_bytes([out[disc], out[disc + 1]]), V21_DISCRIMINATOR, "capture is a v2.1 portfolio");
    out[disc..disc + 2].copy_from_slice(&V16_LAYOUT_DISCRIMINATOR.to_le_bytes());
    out
}
