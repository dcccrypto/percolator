//! Engine-level ADL / insurance / bankruptcy loss-sharing edge tests (tests only, no product code).
//!
//! Scope: where rounding_edge.rs (public kernels only) and liquidations_edge.rs stopped:
//! the insurance-spend vs B-index (haircut) split of a bankrupt close, conservation across it,
//! loss socialised never exceeding the deficit, ADL wind-down boundary, and the hlock clearing
//! once the winners have converted. Fixture: the f03 world (5 longs, a fat short, a thin short
//! that goes bankrupt by 2_150_000 atoms), with optional per-domain insurance seeded first.

use percolator::{
    v16_domain_count_for_market_slots, EngineAssetSlotV16Account, LiquidationRequestV16, Market,
    MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, PermissionlessCrankActionV16,
    PermissionlessCrankRequestV16, PortfolioAccountV16Account, PortfolioV16ViewMut,
    ProvenanceHeaderV16, ProvenanceHeaderV16Account, TradeRequestV16, V16Config, V16Error,
    V16PodU64,
};
use percolator::{ADL_ONE, POS_SCALE};

// ---------------------------------------------------------------- fixtures --
// fixture copied from tests/f03_regression.rs (itself from v16_spec_tests.rs @ 2c38570a)

fn ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    ([1; 32], [2; 32], [3; 32])
}

fn funding_market_fixture(init_price: u64) -> (MarketGroupV16HeaderAccount, Vec<Market<u64>>) {
    let (market_id, _, _) = ids();
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    cfg.max_abs_funding_e9_per_slot = 10_000;
    cfg.max_price_move_bps_per_slot = 9_000;
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(market_id, cfg, 1, 0).unwrap();
    let mut markets = vec![Market::new(0, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, init_price, 1)
        .unwrap();
    {
        let view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        view.validate_shape().unwrap();
    }
    (header, markets)
}

fn account_fixture(market_slots: u32, account_seed: u8) -> PortfolioAccountV16Account {
    let (market_id, _, owner) = ids();
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        market_id,
        [account_seed; 32],
        owner,
    ));
    let _ = v16_domain_count_for_market_slots(market_slots).unwrap();
    let mut account = PortfolioAccountV16Account::default();
    account.init_empty_in_place(header).unwrap();
    account
}

fn signed_q(q: u128) -> i128 {
    i128::try_from(q).unwrap()
}

// ------------------------------------------------------------------ world ---

const PRICE: u64 = 1_000_000;
const MARK: u64 = PRICE + 1_800_000;
const LOT: u128 = 3 * POS_SCALE;

struct World {
    header: MarketGroupV16HeaderAccount,
    markets: Vec<Market<u64>>,
    longs: Vec<PortfolioAccountV16Account>,
    short_big: PortfolioAccountV16Account,
    short_thin: PortfolioAccountV16Account,
    outcome: percolator::LiquidationOutcomeV16,
    vault_before: u128,
}

impl World {
    fn asset(&self) -> percolator::AssetStateV16 {
        self.markets[0].engine.asset.try_to_runtime().unwrap()
    }
    fn hlock(&self) -> u8 {
        self.header.bankruptcy_hlock_active
    }

    /// wrapper tag 5 `PermissionlessCrank`, action 0 -> `PermissionlessCrankActionV16::Refresh`
    /// (`percolator-prog` `handle_permissionless_crank_zero_copy`; the target
    /// portfolio's owner never signs).
    fn crank_refresh(&mut self, account: &mut PortfolioAccountV16Account, now_slot: u64) {
        let mut market = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let mut v = PortfolioV16ViewMut::new(account);
        market
            .permissionless_crank_not_atomic(
                &mut v,
                PermissionlessCrankRequestV16 {
                    now_slot,
                    asset_index: 0,
                    effective_price: MARK,
                    funding_rate_e9: 0,
                    action: PermissionlessCrankActionV16::Refresh,
                },
            )
            .expect("permissionless Refresh crank must succeed");
    }

    /// wrapper tag 3 `Trade` -> `execute_trade_with_fee_loss_stale_scoped_not_atomic`.
    /// Closes `longs[i]` to flat against the fat short; the engine routes this
    /// through `PositionRouteV16::Clear` -> `clear_leg_at_slot_inner` ->
    /// `V16Core::kernel_clear_leg`.
    fn close_long_to_flat(&mut self, i: usize) -> Result<(), V16Error> {
        let asset = self.asset();
        let leg = self.longs[i].legs[0].try_to_runtime().unwrap();
        let effective = leg.basis_pos_q * (asset.a_long as i128) / (ADL_ONE as i128);
        let mut market = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let mut lv = PortfolioV16ViewMut::new(&mut self.longs[i]);
        let mut sv = PortfolioV16ViewMut::new(&mut self.short_big);
        market
            .execute_trade_with_fee_loss_stale_scoped_not_atomic(
                &mut lv,
                &mut sv,
                TradeRequestV16 {
                    asset_index: 0,
                    size_q: -effective,
                    exec_price: MARK,
                    fee_bps: 0,
                },
                true,
            )
            .map(|_| ())
    }
}

/// Builds the chain state every step starts from, using only public entries:
///   * five long accounts each carrying `LOT`,
///   * one fat short taking the other side of `longs[0..4]`,
///   * one thin short taking the other side of `longs[4]`,
///   * a price move that puts the thin short under water past its capital,
///   * a PUBLIC permissionless liquidation of the thin short (wrapper tag 5,
///     action `Liquidate`) whose residual is socialized onto the long side.
fn bankrupt_world_ins(ins_d0: u128, ins_d1: u128) -> World {
    let (mut header, mut markets) = funding_market_fixture(PRICE);
    let mut longs: Vec<PortfolioAccountV16Account> =
        (0..5).map(|i| account_fixture(1, 100 + i as u8)).collect();
    let mut short_big = account_fixture(1, 200);
    let mut short_thin = account_fixture(1, 201);

    {
        let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        if ins_d0 > 0 {
            market
                .deposit_domain_insurance_not_atomic(0, ins_d0)
                .unwrap();
        }
        if ins_d1 > 0 {
            market
                .deposit_domain_insurance_not_atomic(1, ins_d1)
                .unwrap();
        }
        {
            let mut v = PortfolioV16ViewMut::new(&mut short_big);
            market.deposit_not_atomic(&mut v, 400_000_000).unwrap();
        }
        {
            let mut v = PortfolioV16ViewMut::new(&mut short_thin);
            market.deposit_not_atomic(&mut v, 3_250_000).unwrap();
        }
        for l in longs.iter_mut() {
            let mut v = PortfolioV16ViewMut::new(l);
            market.deposit_not_atomic(&mut v, 100_000_000).unwrap();
        }
        for l in longs.iter_mut().take(4) {
            let mut lv = PortfolioV16ViewMut::new(l);
            let mut sv = PortfolioV16ViewMut::new(&mut short_big);
            market
                .execute_trade_with_fee_loss_stale_scoped_not_atomic(
                    &mut lv,
                    &mut sv,
                    TradeRequestV16 {
                        asset_index: 0,
                        size_q: signed_q(LOT),
                        exec_price: PRICE,
                        fee_bps: 0,
                    },
                    true,
                )
                .unwrap();
        }
        {
            let mut lv = PortfolioV16ViewMut::new(&mut longs[4]);
            let mut sv = PortfolioV16ViewMut::new(&mut short_thin);
            market
                .execute_trade_with_fee_loss_stale_scoped_not_atomic(
                    &mut lv,
                    &mut sv,
                    TradeRequestV16 {
                        asset_index: 0,
                        size_q: signed_q(LOT),
                        exec_price: PRICE,
                        fee_bps: 0,
                    },
                    true,
                )
                .unwrap();
        }
        market
            .accrue_asset_to_not_atomic(0, 2, PRICE + 900_000, 0, true)
            .unwrap();
        market
            .accrue_asset_to_not_atomic(0, 3, MARK, 0, true)
            .unwrap();
        market.markets[0].engine.asset.raw_oracle_target_price = V16PodU64::new(MARK);
    }

    let vault_before = header.vault.get();
    let outcome = {
        let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut sv = PortfolioV16ViewMut::new(&mut short_thin);
        market
            .liquidate_account_not_atomic(&mut sv, LiquidationRequestV16 { asset_index: 0 })
            .expect("permissionless liquidation (wrapper tag 5, action Liquidate)")
    };

    World {
        header,
        markets,
        longs,
        short_big,
        short_thin,
        outcome,
        vault_before,
    }
}

fn bankrupt_world() -> World {
    bankrupt_world_ins(0, 0)
}

impl World {
    fn accounts(&self) -> Vec<&PortfolioAccountV16Account> {
        self.longs
            .iter()
            .chain([&self.short_big, &self.short_thin])
            .collect()
    }
    fn value(&self) -> i128 {
        self.header.insurance.get() as i128
            + self
                .accounts()
                .iter()
                .map(|a| a.capital.get() as i128 + a.pnl.get())
                .sum::<i128>()
    }
}

const DEFICIT: u128 = 2_150_000;

fn refresh_all(w: &mut World) {
    let slot = w.header.current_slot.get();
    for i in 0..5 {
        let mut a = core::mem::take(&mut w.longs[i]);
        for _ in 0..64 {
            w.crank_refresh(&mut a, slot);
            if !a.legs[0].try_to_runtime().unwrap().b_stale {
                break;
            }
        }
        w.longs[i] = a;
    }
    let mut s = core::mem::take(&mut w.short_big);
    for _ in 0..64 {
        w.crank_refresh(&mut s, slot);
        if !s.legs[0].try_to_runtime().unwrap().b_stale {
            break;
        }
    }
    w.short_big = s;
}

/// Waterfall: insurance first (domain 0 here), the remainder is booked to B; never both
/// over-charged, never explicit loss, insurance falls by exactly what was used, vault is
/// untouched by the close.
#[test]
fn waterfall_splits_deficit_between_insurance_and_b_at_every_boundary() {
    for ins in [0u128, 1, DEFICIT - 1, DEFICIT, DEFICIT + 1, 50_000_000] {
        let w = bankrupt_world_ins(ins, 0);
        let o = w.outcome;
        let used = ins.min(DEFICIT);
        assert_eq!(o.insurance_used, used, "ins={ins}");
        assert_eq!(o.residual_booked, DEFICIT - used, "ins={ins}");
        assert_eq!(o.explicit_loss, 0, "ins={ins}");
        assert_eq!(
            o.insurance_used + o.residual_booked + o.explicit_loss,
            DEFICIT
        );
        assert_eq!(w.header.insurance.get(), ins - used, "ins={ins}");
        assert_eq!(
            w.header.vault.get(),
            w.vault_before,
            "vault moves only on deposit/withdraw"
        );
        // B is touched iff something was left after insurance.
        assert_eq!(
            w.asset().b_long_num != 0,
            o.residual_booked > 0,
            "ins={ins}"
        );
        // hlock: latched iff the loss was not fully absorbed by insurance.
        assert_eq!(
            w.hlock() != 0,
            o.residual_booked > 0,
            "ins={ins} hlock={}",
            w.hlock()
        );
    }
}

/// Insurance sitting in the other domain must not be spent on this bankruptcy.
#[test]
fn other_domains_insurance_is_not_spent() {
    let w = bankrupt_world_ins(0, 5_000_000);
    assert_eq!(w.outcome.insurance_used, 0);
    assert_eq!(w.outcome.residual_booked, DEFICIT);
    assert_eq!(w.header.insurance.get(), 5_000_000);
}

/// Conservation: after the bankrupt close and a full settlement of every account, the claims on
/// the vault (capital + insurance) never exceed the vault, and winners absorb exactly the
/// booked residual (loss socialised never exceeds the deficit).
#[test]
fn loss_socialized_never_exceeds_deficit_and_vault_covers_claims() {
    for ins in [0u128, 1_000_000, DEFICIT] {
        let mut w = bankrupt_world_ins(ins, 0);
        refresh_all(&mut w);
        let capital: u128 = w.accounts().iter().map(|a| a.capital.get()).sum();
        assert!(
            capital + w.header.insurance.get() <= w.header.vault.get(),
            "ins={ins}: claims exceed the vault"
        );
        assert_eq!(
            w.header.c_tot.get(),
            capital,
            "ins={ins}: c_tot tracks account capital"
        );
        let pnl: i128 = w.accounts().iter().map(|a| a.pnl.get()).sum();
        let booked = w.outcome.residual_booked as i128;
        // zero-sum settlement: winners' positive pnl minus losers' (already in capital) is
        // short of the full gain by at most the socialised amount (+ a dust atom per leg).
        // Winners' settled gain + the socialised residual == the shorts' full loss (27_000_000),
        // up to < 1 atom of sub-atom B dust per winning leg (5 longs).
        let gap = pnl + booked - 27_000_000;
        assert!(
            (0..=5).contains(&gap),
            "ins={ins}: winners absorbed {} of booked {booked}",
            27_000_000 - pnl
        );
        // Each settled leg may carry < 1 atom of sub-atom B dust (b_rem, written off later as
        // explicit_unallocated_loss), so claims can exceed the vault by at most one atom per
        // leg (5 winning legs here) and never fall below it.
        let slack = w.value() - w.header.vault.get() as i128;
        assert!(
            (0..=5).contains(&slack),
            "ins={ins}: claims-vs-vault slack {slack}"
        );
    }
}

/// hlock clears (permissionlessly) once the winners have settled, closed and converted.
#[test]
fn hlock_clears_after_winners_settle_close_and_convert() {
    let mut w = bankrupt_world_ins(0, 0);
    assert_eq!(w.hlock(), 5);
    // The permissionless re-evaluation may clear as soon as no account is negative / stale /
    // b-stale; the pending loss lives in the B index and is charged at settlement, so it must
    // never let a winner convert more than its post-haircut gain. Check that below.
    let early: bool = {
        let mut m = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
        m.try_clear_bankruptcy_hlock_not_atomic().unwrap()
    };
    let mut converted = 0u128;
    refresh_all(&mut w);
    for i in 0..5 {
        w.close_long_to_flat(i).unwrap();
    }
    for i in 0..5 {
        let mut a = core::mem::take(&mut w.longs[i]);
        {
            let mut m = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
            let mut v = PortfolioV16ViewMut::new(&mut a);
            m.full_account_refresh_not_atomic(&mut v).unwrap();
            converted += m
                .convert_released_pnl_to_capital_not_atomic(&mut v)
                .unwrap();
        }
        w.longs[i] = a;
    }
    {
        let mut m = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
        let mut v = PortfolioV16ViewMut::new(&mut w.short_big);
        m.full_account_refresh_not_atomic(&mut v).unwrap();
        converted += m
            .convert_released_pnl_to_capital_not_atomic(&mut v)
            .unwrap();
    }
    let cleared = {
        let mut m = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
        m.try_clear_bankruptcy_hlock_not_atomic().unwrap()
    };
    assert!(
        early,
        "today the re-evaluation clears right after the close (B is charged at settlement)"
    );
    assert!(cleared);
    assert_eq!(w.hlock(), 0);
    assert_eq!(
        converted,
        27_000_000 - DEFICIT,
        "winners converted exactly their gain minus the socialised loss"
    );
    let capital: u128 = w.accounts().iter().map(|a| a.capital.get()).sum();
    assert!(
        capital + w.header.insurance.get() <= w.header.vault.get() + 6,
        "claims after conversion exceed vault"
    );
}

/// ADL quantity after a fully insured bankruptcy: matched OI, and DustNotional boundary of the
/// permissionless wind-down is exact (<= max passes, max-1 fails).
#[test]
fn adl_state_and_wind_down_dust_boundary() {
    let w = bankrupt_world_ins(100_000_000, 100_000_000);
    let a = w.asset();
    assert_eq!(
        a.oi_eff_long_q, a.oi_eff_short_q,
        "matched book after the close"
    );
    let notional = a.oi_eff_long_q * u128::from(a.effective_price) / POS_SCALE;
    let (mut hdr, mut mk) = (w.header, w.markets.clone());
    let m = MarketGroupV16ViewMut::new(&mut hdr, &mut mk);
    let adl = a.a_long != ADL_ONE || a.a_short != ADL_ONE;
    assert!(adl, "the unilateral bankrupt close scaled a side's A");
    use percolator::AdlWindDownBoundV16 as B;
    assert_eq!(
        m.adl_wind_down_eligible(
            0,
            B::DustNotional {
                max_notional_atoms: notional
            }
        )
        .unwrap(),
        adl
    );
    assert!(!m
        .adl_wind_down_eligible(
            0,
            B::DustNotional {
                max_notional_atoms: notional.saturating_sub(1)
            }
        )
        .unwrap());
    assert_eq!(m.adl_wind_down_eligible(0, B::EpisodeExpired).unwrap(), adl);
}
