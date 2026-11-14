//! Edge cases for the v2.2 stress envelope and fee policy updates (tests only).
//!
//! Three areas:
//!   * stress envelope (`apply_stress_envelope_progress`, `clear_stress_envelope_v16`): zero
//!     input, the exact trigger boundary, accumulation over many calls, saturation at u128::MAX,
//!     the epoch/slot/active-close reset rules, repeated clears, and that no value field moves;
//!   * fee policy update (`apply_fee_policy_update_not_atomic`): bounds,
//!     rejection leaves config byte-unchanged, idempotence, and a fee change in the middle of an
//!     open position;
//!   * account backing fee (`charge_account_backing_fee_not_atomic`): value conservation over
//!     repeated charges, overflow of the fee sum, and negative-PnL refusal.
//!
//! Every test name carries `stress_envelope`, `fee_policy` or `backing_fee`, so the area-wide
//! count is `cargo test --features fork-facade -- stress_envelope fee_policy backing_fee`.
//! This file alone: `cargo test --features fork-facade --test stress_fee_policy_edge`.
//! The stress envelope constant and the fee policy update are fork-facade surface, so the whole
//! file is gated on that feature (it builds to zero tests without it).

#![cfg(feature = "fork-facade")]

use percolator::{
    EngineAssetSlotV16Account, HealthCertV16, HealthCertV16Account, Market,
    MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, PortfolioAccountV16Account,
    PortfolioV16ViewMut, ProvenanceHeaderV16, ProvenanceHeaderV16Account, V16Config, V16Error,
    V16PodI128, V16PodU128, V16PodU64, STRESS_ENVELOPE_TRIGGER_BPS_E9, V16_EMPTY_ACTIVE_BITMAP,
};

const TRIGGER: u128 = STRESS_ENVELOPE_TRIGGER_BPS_E9;

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

type Fixture = (MarketGroupV16HeaderAccount, Vec<Market<u64>>);

fn market(market_slots: u32, price: u64, max_trading_fee_bps: u64) -> Fixture {
    let mut cfg =
        V16Config::public_user_fund_with_market_slots(market_slots as u16, market_slots, 0, 10);
    cfg.max_trading_fee_bps = max_trading_fee_bps;
    let mut header =
        MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, market_slots, 0).unwrap();
    let mut markets = (0..market_slots)
        .map(|i| Market::new(i as u64, EngineAssetSlotV16Account::default()))
        .collect::<Vec<_>>();
    for (i, slot) in markets.iter_mut().enumerate() {
        header
            .activate_empty_asset_slot_not_atomic(i as u32, &mut slot.engine, price, (i + 1) as u64)
            .unwrap();
    }
    MarketGroupV16ViewMut::new(&mut header, &mut markets)
        .validate_shape()
        .unwrap();
    (header, markets)
}

fn account(seed: u8) -> PortfolioAccountV16Account {
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        [1; 32], [seed; 32], [3; 32],
    ));
    let mut account = PortfolioAccountV16Account::default();
    account.init_empty_in_place(header).unwrap();
    account
}

/// Snapshot of the envelope fields: (accumulator, start_slot, start_epoch, flag byte).
fn envelope(h: &MarketGroupV16HeaderAccount) -> (u128, u64, u64, u8) {
    (
        h.stress_consumption_bps_e9_since_envelope.get(),
        h.stress_envelope_start_slot.get(),
        h.stress_envelope_start_credit_epoch.get(),
        h.threshold_stress_active,
    )
}

const IDLE: (u128, u64, u64, u8) = (0, u64::MAX, u64::MAX, 0);

/// Value fields the envelope must never touch.
fn value_fields(h: &MarketGroupV16HeaderAccount) -> [u128; 4] {
    [
        h.vault.get(),
        h.c_tot.get(),
        h.insurance.get(),
        h.pnl_pos_tot.get(),
    ]
}

/// Puts the envelope into the "active, opened at (slot, epoch)" state the writer would produce.
fn open_envelope(h: &mut MarketGroupV16HeaderAccount, acc: u128, slot: u64, epoch: u64) {
    h.threshold_stress_active = 1;
    h.stress_consumption_bps_e9_since_envelope = V16PodU128::new(acc);
    h.stress_envelope_start_slot = V16PodU64::new(slot);
    h.stress_envelope_start_credit_epoch = V16PodU64::new(epoch);
}

// ---------------------------------------------------------------------------------------------
// Stress envelope
// ---------------------------------------------------------------------------------------------

/// Zero consumption is an early return: nothing changes, not even a stale envelope that the next
/// nonzero call would reset.
#[test]
fn stress_envelope_zero_consumption_is_a_noop_even_on_a_stale_envelope() {
    let (mut header, mut markets) = market(1, 100, 0);
    {
        let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        m.apply_stress_envelope_progress(0, 5).unwrap();
    }
    assert_eq!(envelope(&header), IDLE);

    open_envelope(&mut header, TRIGGER, 10, 1);
    header.risk_epoch = V16PodU64::new(2);
    let before = envelope(&header);
    {
        let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        m.apply_stress_envelope_progress(0, 11).unwrap();
        m.validate_shape().unwrap();
    }
    assert_eq!(envelope(&header), before);
}

/// One unit below the trigger stays off with sentinels intact; the exact trigger flips it on and
/// stamps the call's slot and the current risk epoch.
#[test]
fn stress_envelope_trigger_boundary_is_inclusive() {
    let (mut header, mut markets) = market(1, 100, 0);
    {
        let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        m.apply_stress_envelope_progress(TRIGGER - 1, 7).unwrap();
        m.validate_shape().unwrap();
    }
    assert_eq!(envelope(&header), (TRIGGER - 1, u64::MAX, u64::MAX, 0));

    let epoch = header.risk_epoch.get();
    {
        let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        m.apply_stress_envelope_progress(1, 8).unwrap();
        m.validate_shape().unwrap();
    }
    assert_eq!(envelope(&header), (TRIGGER, 8, epoch, 1));
}

/// Many small calls add up exactly (no rounding, no loss); the envelope opens on the call that
/// crosses the trigger, and is stamped with that call's slot.
#[test]
fn stress_envelope_accumulates_exactly_over_repeated_calls() {
    let (mut header, mut markets) = market(1, 100, 0);
    let step = TRIGGER / 4 + 1; // 4 steps cross, 3 do not
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    for i in 0..3u64 {
        m.apply_stress_envelope_progress(step, 100 + i).unwrap();
        assert_eq!(m.header.threshold_stress_active, 0);
    }
    assert_eq!(
        m.header.stress_consumption_bps_e9_since_envelope.get(),
        3 * step
    );
    m.apply_stress_envelope_progress(step, 103).unwrap();
    assert_eq!(m.header.threshold_stress_active, 1);
    assert_eq!(m.header.stress_envelope_start_slot.get(), 103);
    assert_eq!(
        m.header.stress_consumption_bps_e9_since_envelope.get(),
        4 * step
    );
    m.validate_shape().unwrap();
}

/// Once open (same epoch), more consumption only grows the accumulator; the start stamps are
/// not rewritten by later calls in later slots.
#[test]
fn stress_envelope_already_open_is_not_restamped() {
    let (mut header, mut markets) = market(1, 100, 0);
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let epoch = m.header.risk_epoch.get();
    m.apply_stress_envelope_progress(TRIGGER, 20).unwrap();
    for slot in 21..25u64 {
        m.apply_stress_envelope_progress(5, slot).unwrap();
    }
    assert_eq!(
        (
            m.header.stress_consumption_bps_e9_since_envelope.get(),
            m.header.stress_envelope_start_slot.get(),
            m.header.stress_envelope_start_credit_epoch.get(),
            m.header.threshold_stress_active,
        ),
        (TRIGGER + 20, 20, epoch, 1)
    );
}

/// The accumulator saturates at u128::MAX instead of overflowing, and stays there on repeat.
#[test]
fn stress_envelope_accumulator_saturates_at_u128_max() {
    let (mut header, mut markets) = market(1, 100, 0);
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    m.apply_stress_envelope_progress(u128::MAX, 1).unwrap();
    assert_eq!(m.header.threshold_stress_active, 1);
    for _ in 0..3 {
        m.apply_stress_envelope_progress(u128::MAX, 1).unwrap();
        m.apply_stress_envelope_progress(1, 1).unwrap();
    }
    assert_eq!(
        m.header.stress_consumption_bps_e9_since_envelope.get(),
        u128::MAX
    );
    assert_eq!(m.header.stress_envelope_start_slot.get(), 1);
    m.validate_shape().unwrap();
}

/// Reset rule: an open envelope is cleared only when the risk epoch moved past its start epoch
/// AND the slot differs AND no active close is running. Each condition alone blocks the reset.
#[test]
fn stress_envelope_reset_needs_epoch_advance_and_new_slot() {
    // Epoch advanced, new slot: reset, so only the fresh delta remains and the flag drops.
    let (mut header, mut markets) = market(1, 100, 0);
    open_envelope(&mut header, TRIGGER, 10, 1);
    header.risk_epoch = V16PodU64::new(2);
    {
        let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        m.apply_stress_envelope_progress(3, 11).unwrap();
        m.validate_shape().unwrap();
    }
    assert_eq!(envelope(&header), (3, u64::MAX, u64::MAX, 0));

    // Epoch advanced but same slot as the start: no reset.
    let (mut header, mut markets) = market(1, 100, 0);
    open_envelope(&mut header, TRIGGER, 10, 1);
    header.risk_epoch = V16PodU64::new(2);
    {
        let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        m.apply_stress_envelope_progress(3, 10).unwrap();
    }
    assert_eq!(envelope(&header), (TRIGGER + 3, 10, 1, 1));

    // Same epoch, new slot: no reset.
    let (mut header, mut markets) = market(1, 100, 0);
    open_envelope(&mut header, TRIGGER, 10, 1);
    header.risk_epoch = V16PodU64::new(1);
    {
        let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        m.apply_stress_envelope_progress(3, 99).unwrap();
    }
    assert_eq!(envelope(&header), (TRIGGER + 3, 10, 1, 1));
}

/// A loss-stale (active close) market keeps the envelope open across an epoch advance.
#[test]
fn stress_envelope_active_close_blocks_the_reset() {
    let (mut header, mut markets) = market(1, 100, 0);
    open_envelope(&mut header, TRIGGER, 10, 1);
    header.risk_epoch = V16PodU64::new(2);
    header.loss_stale_active = 1;
    {
        let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        m.apply_stress_envelope_progress(3, 11).unwrap();
    }
    assert_eq!(envelope(&header), (TRIGGER + 3, 10, 1, 1));
}

/// After a reset, a fresh delta that by itself reaches the trigger reopens the envelope with
/// the NEW slot and epoch, not the old ones.
#[test]
fn stress_envelope_reset_then_reopen_restamps() {
    let (mut header, mut markets) = market(1, 100, 0);
    open_envelope(&mut header, TRIGGER * 2, 10, 1);
    header.risk_epoch = V16PodU64::new(4);
    {
        let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        m.apply_stress_envelope_progress(TRIGGER, 50).unwrap();
        m.validate_shape().unwrap();
    }
    assert_eq!(envelope(&header), (TRIGGER, 50, 4, 1));
}

/// Documented behavior: a below-trigger accumulator has no start epoch, so the epoch reset never
/// applies to it and it keeps growing across epochs until it opens an envelope.
#[test]
fn stress_envelope_below_trigger_accumulator_carries_across_epochs() {
    let (mut header, mut markets) = market(1, 100, 0);
    let half = TRIGGER / 2;
    {
        let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        m.apply_stress_envelope_progress(half, 1).unwrap();
    }
    header.risk_epoch = V16PodU64::new(header.risk_epoch.get() + 5);
    let epoch = header.risk_epoch.get();
    {
        let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        m.apply_stress_envelope_progress(TRIGGER - half, 2).unwrap();
    }
    assert_eq!(envelope(&header), (TRIGGER, 2, epoch, 1));
}

/// Clear is idempotent, restores the exact idle pattern that validate_shape requires, and works
/// from any state (idle, below trigger, open, saturated).
#[test]
fn stress_envelope_clear_is_idempotent_from_any_state() {
    for acc in [0u128, 1, TRIGGER - 1, TRIGGER, u128::MAX] {
        let (mut header, mut markets) = market(1, 100, 0);
        let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        m.apply_stress_envelope_progress(acc, 3).unwrap();
        for _ in 0..3 {
            m.clear_stress_envelope_v16();
            m.validate_shape().unwrap();
        }
        assert_eq!(envelope(m.header), IDLE, "acc {acc}");
        // And it can be reopened after a clear.
        m.apply_stress_envelope_progress(TRIGGER, 4).unwrap();
        assert_eq!(m.header.threshold_stress_active, 1);
    }
}

/// Conservation: no envelope operation moves value (vault, c_tot, insurance, pnl_pos_tot), even
/// on a funded market.
#[test]
fn stress_envelope_never_moves_value() {
    let (mut header, mut markets) = market(1, 100, 0);
    let mut a = account(31);
    {
        let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut acct = PortfolioV16ViewMut::new(&mut a);
        m.deposit_not_atomic(&mut acct, 1_234).unwrap();
    }
    let before = value_fields(&header);
    {
        let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        m.apply_stress_envelope_progress(1, 1).unwrap();
        m.apply_stress_envelope_progress(TRIGGER, 2).unwrap();
        m.apply_stress_envelope_progress(u128::MAX, 3).unwrap();
        m.clear_stress_envelope_v16();
        m.validate_shape().unwrap();
    }
    assert_eq!(value_fields(&header), before);
    assert_eq!(a.capital.get(), 1_234);
}

/// Resolution clears an open envelope back to idle.
#[test]
fn stress_envelope_resolve_clears_open_envelope() {
    let (mut header, mut markets) = market(1, 100, 0);
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    m.apply_stress_envelope_progress(TRIGGER + 9, 3).unwrap();
    let slot = m.header.current_slot.get();
    m.resolve_market_not_atomic(slot).unwrap();
    assert_eq!(envelope(m.header), IDLE);
}

// ---------------------------------------------------------------------------------------------
// Fee policy update (fork-facade)
// ---------------------------------------------------------------------------------------------

mod fee_policy {
    use super::*;
    use percolator::{
        FeePolicyUpdateV16, TradeRequestV16, MAX_MARGIN_BPS, MAX_PROTOCOL_FEE_ABS, POS_SCALE,
    };

    fn upd(trade: u64, liq: u64, cap: u128, min: u128) -> FeePolicyUpdateV16 {
        FeePolicyUpdateV16 {
            max_trading_fee_bps: trade,
            liquidation_fee_bps: liq,
            liquidation_fee_cap: cap,
            min_liquidation_abs: min,
        }
    }

    fn cfg_bytes(h: &MarketGroupV16HeaderAccount) -> Vec<u8> {
        bytemuck::bytes_of(&h.config).to_vec()
    }

    fn assert_rejected_unchanged(update: FeePolicyUpdateV16, why: &str) {
        let (mut header, _markets) = market(1, 100, 7);
        let before = cfg_bytes(&header);
        assert!(
            header.apply_fee_policy_update_not_atomic(update).is_err(),
            "{why}: should be rejected"
        );
        assert_eq!(
            cfg_bytes(&header),
            before,
            "{why}: config must be unchanged"
        );
    }

    /// The trading-fee bound is inclusive at MAX_MARGIN_BPS (10_000) and rejects one above.
    #[test]
    fn fee_policy_trading_fee_bound_is_inclusive() {
        for bps in [0u64, 1, MAX_MARGIN_BPS] {
            let (mut header, _markets) = market(1, 100, 7);
            header
                .apply_fee_policy_update_not_atomic(upd(bps, 0, 0, 0))
                .unwrap();
            assert_eq!(header.config.max_trading_fee_bps.get(), bps);
        }
        assert_rejected_unchanged(upd(MAX_MARGIN_BPS + 1, 0, 0, 0), "trade bps 10_001");
        assert_rejected_unchanged(upd(u64::MAX, 0, 0, 0), "trade bps u64::MAX");
    }

    /// Shape bounds on the liquidation fields: min > cap, cap > MAX_PROTOCOL_FEE_ABS and
    /// liquidation bps > 10_000 are all rejected with the config left byte-for-byte unchanged.
    #[test]
    fn fee_policy_liquidation_shape_bounds_reject_without_mutation() {
        assert_rejected_unchanged(upd(0, 0, 4, 5), "min > cap");
        assert_rejected_unchanged(upd(0, 0, MAX_PROTOCOL_FEE_ABS + 1, 0), "cap too large");
        assert_rejected_unchanged(upd(0, 0, u128::MAX, 0), "cap u128::MAX");
        assert_rejected_unchanged(upd(0, MAX_MARGIN_BPS + 1, 0, 0), "liq bps 10_001");
    }

    /// The default config has 100% maintenance margin and a 100%-per-slot price move budget, so the
    /// solvency envelope leaves no room for any nonzero liquidation fee: even 1 bps or a floor of
    /// 1 atom is rejected (and nothing changes). A cap alone (no bps, no floor) is harmless.
    #[test]
    fn fee_policy_liquidation_fee_rejected_when_envelope_has_no_room() {
        assert_rejected_unchanged(upd(0, 1, 1, 0), "1 bps liquidation fee");
        assert_rejected_unchanged(upd(0, 0, 1, 1), "1 atom liquidation floor");
        let (mut header, _markets) = market(1, 100, 7);
        header
            .apply_fee_policy_update_not_atomic(upd(0, 0, MAX_PROTOCOL_FEE_ABS, 0))
            .unwrap();
        assert_eq!(
            header.config.liquidation_fee_cap.get(),
            MAX_PROTOCOL_FEE_ABS
        );
    }

    /// Only Live markets accept the update; a resolved market refuses with LockActive.
    #[test]
    fn fee_policy_update_refused_after_resolve() {
        let (mut header, mut markets) = market(1, 100, 7);
        {
            let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
            let slot = m.header.current_slot.get();
            m.resolve_market_not_atomic(slot).unwrap();
        }
        let before = cfg_bytes(&header);
        assert_eq!(
            header.apply_fee_policy_update_not_atomic(upd(1, 0, 0, 0)),
            Err(V16Error::LockActive)
        );
        assert_eq!(cfg_bytes(&header), before);
    }

    /// Repeating the same update is idempotent, and an update touches nothing outside the four
    /// fee fields: no epoch bump, no envelope or value change, the rest of the config is equal.
    #[test]
    fn fee_policy_update_is_idempotent_and_isolated() {
        let (mut header, mut markets) = market(1, 100, 7);
        {
            let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
            m.apply_stress_envelope_progress(5, 1).unwrap();
        }
        let mut before = header;
        header
            .apply_fee_policy_update_not_atomic(upd(3, 0, 9, 0))
            .unwrap();
        let once = cfg_bytes(&header);
        for _ in 0..3 {
            header
                .apply_fee_policy_update_not_atomic(upd(3, 0, 9, 0))
                .unwrap();
            assert_eq!(cfg_bytes(&header), once);
        }
        // Expected: the old header with only the four fields swapped.
        before.config.max_trading_fee_bps = V16PodU64::new(3);
        before.config.liquidation_fee_cap = V16PodU128::new(9);
        assert_eq!(bytemuck::bytes_of(&header), bytemuck::bytes_of(&before));
        MarketGroupV16ViewMut::new(&mut header, &mut markets)
            .validate_shape()
            .unwrap();
    }

    fn q(lots: u128) -> i128 {
        (lots * POS_SCALE) as i128
    }

    fn req(size_q: i128, fee_bps: u64) -> TradeRequestV16 {
        TradeRequestV16 {
            asset_index: 0,
            size_q,
            exec_price: 100,
            fee_bps,
        }
    }

    /// Mid-position fee change: open at 10 bps, lower the cap to 5, then add to the position.
    /// The old fee is now refused, 5 bps is charged exactly, raising the cap again re-allows 10,
    /// and capital + insurance is conserved through every step.
    #[test]
    fn fee_policy_change_mid_position_applies_to_next_trade_only() {
        let (mut header, mut markets) = market(1, 100, 10);
        let mut la = account(41);
        let mut sa = account(42);
        let total = |h: &MarketGroupV16HeaderAccount,
                     a: &PortfolioAccountV16Account,
                     b: &PortfolioAccountV16Account| {
            a.capital.get() + b.capital.get() + h.insurance.get()
        };
        {
            let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
            let mut long = PortfolioV16ViewMut::new(&mut la);
            let mut short = PortfolioV16ViewMut::new(&mut sa);
            m.deposit_not_atomic(&mut long, 50_000).unwrap();
            m.deposit_not_atomic(&mut short, 50_000).unwrap();
            // 100 lots at 100: notional 10_000 -> fee == bps.
            let out = m
                .execute_trade_with_fee_loss_stale_scoped_not_atomic(
                    &mut long,
                    &mut short,
                    req(q(100), 10),
                    true,
                )
                .unwrap();
            assert_eq!(out.fee_a + out.fee_b, 10);
        }
        let t0 = total(&header, &la, &sa);
        assert_eq!(t0, 100_000);

        header
            .apply_fee_policy_update_not_atomic(upd(5, 0, 0, 0))
            .unwrap();
        {
            let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
            let mut long = PortfolioV16ViewMut::new(&mut la);
            let mut short = PortfolioV16ViewMut::new(&mut sa);
            let cap_before = long.header.capital.get();
            assert_eq!(
                m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
                    &mut long,
                    &mut short,
                    req(q(100), 10),
                    true,
                ),
                Err(V16Error::InvalidConfig)
            );
            assert_eq!(long.header.capital.get(), cap_before);
            let out = m
                .execute_trade_with_fee_loss_stale_scoped_not_atomic(
                    &mut long,
                    &mut short,
                    req(q(100), 5),
                    true,
                )
                .unwrap();
            assert_eq!(out.fee_a + out.fee_b, 5);
            m.validate_shape().unwrap();
        }
        assert_eq!(total(&header, &la, &sa), t0);

        header
            .apply_fee_policy_update_not_atomic(upd(10, 0, 0, 0))
            .unwrap();
        {
            let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
            let mut long = PortfolioV16ViewMut::new(&mut la);
            let mut short = PortfolioV16ViewMut::new(&mut sa);
            let out = m
                .execute_trade_with_fee_loss_stale_scoped_not_atomic(
                    &mut long,
                    &mut short,
                    req(-q(100), 10),
                    true,
                )
                .unwrap();
            assert_eq!(out.fee_a + out.fee_b, 10);
            m.validate_shape().unwrap();
        }
        assert_eq!(total(&header, &la, &sa), t0);
    }

    /// Setting the trading fee cap to zero mid-position makes every nonzero fee invalid, but a
    /// zero-fee trade still goes through.
    #[test]
    fn fee_policy_zero_cap_mid_position_allows_only_free_trades() {
        let (mut header, mut markets) = market(1, 100, 10);
        let mut la = account(43);
        let mut sa = account(44);
        {
            let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
            let mut long = PortfolioV16ViewMut::new(&mut la);
            let mut short = PortfolioV16ViewMut::new(&mut sa);
            m.deposit_not_atomic(&mut long, 50_000).unwrap();
            m.deposit_not_atomic(&mut short, 50_000).unwrap();
            m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
                &mut long,
                &mut short,
                req(q(10), 10),
                true,
            )
            .unwrap();
        }
        header
            .apply_fee_policy_update_not_atomic(upd(0, 0, 0, 0))
            .unwrap();
        let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut long = PortfolioV16ViewMut::new(&mut la);
        let mut short = PortfolioV16ViewMut::new(&mut sa);
        assert_eq!(
            m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
                &mut long,
                &mut short,
                req(-q(10), 1),
                true,
            ),
            Err(V16Error::InvalidConfig)
        );
        let out = m
            .execute_trade_with_fee_loss_stale_scoped_not_atomic(
                &mut long,
                &mut short,
                req(-q(10), 0),
                true,
            )
            .unwrap();
        assert_eq!((out.fee_a, out.fee_b), (0, 0));
        m.validate_shape().unwrap();
    }
}

// ---------------------------------------------------------------------------------------------
// Account backing fee
// ---------------------------------------------------------------------------------------------

fn current_cert(
    m: &MarketGroupV16ViewMut<'_, u64>,
    equity: i128,
    im: u128,
) -> HealthCertV16Account {
    HealthCertV16Account::from_runtime(&HealthCertV16 {
        certified_equity: equity,
        certified_initial_req: im,
        certified_maintenance_req: im,
        cert_oracle_epoch: m.header.oracle_epoch.get(),
        cert_funding_epoch: m.header.funding_epoch.get(),
        cert_risk_epoch: m.header.risk_epoch.get(),
        cert_asset_set_epoch: m.header.asset_set_epoch.get(),
        active_bitmap_at_cert: V16_EMPTY_ACTIVE_BITMAP,
        valid: true,
        ..HealthCertV16::default()
    })
}

/// A fresh counterparty bucket on domain 0 expiring at `expiry`, and an account with `capital`
/// and a current certificate (IM 0).
fn backing_fee_fixture(capital: u128, expiry: u64) -> (Fixture, PortfolioAccountV16Account) {
    let (mut header, mut markets) = market(1, 100, 0);
    let mut a = account(21);
    header.vault = V16PodU128::new(capital);
    header.c_tot = V16PodU128::new(capital);
    a.capital = V16PodU128::new(capital);
    {
        let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        m.deposit_fresh_counterparty_backing_not_atomic(0, 1, expiry)
            .unwrap();
        a.health_cert = current_cert(&m, capital as i128, 0);
    }
    ((header, markets), a)
}

/// Repeated charges conserve value: capital + provider earnings + insurance stays equal to the
/// starting capital, the vault never moves, and once capital is exhausted the next charge is
/// refused.
#[test]
fn backing_fee_repeated_charges_conserve_total_value() {
    let ((mut header, mut markets), mut a) = backing_fee_fixture(100, 1_000);
    let vault = header.vault.get();
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut acct = PortfolioV16ViewMut::new(&mut a);
    let mut charged = 0u128;
    // 33 * 3 = 99, one atom left.
    for _ in 0..33 {
        charged += m
            .charge_account_backing_fee_not_atomic(&mut acct, 0, 2, 1, 1)
            .unwrap();
        let earnings = m.markets[0]
            .engine
            .backing_long
            .utilization_fee_earnings
            .get();
        let total = acct.header.capital.get() + earnings + m.header.insurance.get();
        assert_eq!(total, 100);
        assert_eq!(m.header.vault.get(), vault);
        assert_eq!(m.header.c_tot.get(), acct.header.capital.get());
    }
    assert_eq!(charged, 99);
    assert_eq!(acct.header.capital.get(), 1);
    assert_eq!(
        m.charge_account_backing_fee_not_atomic(&mut acct, 0, 1, 1, 1),
        Err(V16Error::LockActive)
    );
    assert_eq!(
        m.charge_account_backing_fee_not_atomic(&mut acct, 0, 1, 1, 0),
        Ok(1)
    );
    assert_eq!(acct.header.capital.get(), 0);
    m.validate_shape().unwrap();
}

/// A provider + insurance sum that overflows u128 is refused before anything is moved.
#[test]
fn backing_fee_sum_overflow_is_refused_without_mutation() {
    let ((mut header, mut markets), mut a) = backing_fee_fixture(100, 1_000);
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut acct = PortfolioV16ViewMut::new(&mut a);
    assert_eq!(
        m.charge_account_backing_fee_not_atomic(&mut acct, 0, u128::MAX, 1, 1),
        Err(V16Error::ArithmeticOverflow)
    );
    assert_eq!(
        m.charge_account_backing_fee_not_atomic(&mut acct, 0, u128::MAX, 1, 0),
        Err(V16Error::LockActive)
    );
    assert_eq!(acct.header.capital.get(), 100);
    assert_eq!(m.header.c_tot.get(), 100);
    assert_eq!(m.header.insurance.get(), 0);
}

/// An account with negative PnL cannot pay a backing fee out of capital, even a 1-atom one.
#[test]
fn backing_fee_refused_on_negative_pnl() {
    let ((mut header, mut markets), mut a) = backing_fee_fixture(100, 1_000);
    a.pnl = V16PodI128::new(-1);
    let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut acct = PortfolioV16ViewMut::new(&mut a);
    let r = m.charge_account_backing_fee_not_atomic(&mut acct, 0, 0, 1, 1);
    assert_eq!(r, Err(V16Error::LockActive));
    assert_eq!(acct.header.capital.get(), 100);
    assert_eq!(m.header.insurance.get(), 0);
    // Control: the same call succeeds once PnL is back to zero.
    acct.header.pnl = V16PodI128::new(0);
    assert_eq!(
        m.charge_account_backing_fee_not_atomic(&mut acct, 0, 0, 1, 1),
        Ok(1)
    );
}
