//! v22kani LIVE (2026-10-20), audit gap G9: liquidation LIVENESS ("an account below maintenance
//! gets liquidated"). Proof-only `cfg(kani)` child module of `v16`, so the harnesses can call and
//! stub the PRIVATE liquidation kernels. Never in a program build; no production line changes.
//!
//! Design, assumptions, covers and mutants: `~/wt-v22kani-1020/reports/LIVE.md` (written before
//! the runs). Every stub here either (a) over-approximates a production function whose own
//! totality is proved by a sibling harness in this file (E1 uses E2/E3/E4), or (b) is a
//! production-faithful (PF) stub of a kani/test/audit-scan-only validator that the deployed SBF
//! build does not compile (E7).
#![allow(dead_code)]
use super::*;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};

// =============================================================================================
// LV-E1: the engine's close-size selector always returns a close in [1, old_abs_q]; a bankrupt
// or negative-PnL account is always fully closed; a partial close is returned only when the
// production health projection answered "healthy" for exactly that size.
// =============================================================================================

static E1_LAST_Q_HI: AtomicU64 = AtomicU64::new(0);
static E1_LAST_Q_LO: AtomicU64 = AtomicU64::new(0);
/// 0 = no call, 1 = Ok(true), 2 = Ok(false), 3 = Err(NonProgress)
static E1_LAST_ANS: AtomicU8 = AtomicU8::new(0);
static E1_CALLS: AtomicU64 = AtomicU64::new(0);

fn e1_put_last(q: u128, ans: u8) {
    E1_LAST_Q_HI.store((q >> 64) as u64, Ordering::Relaxed);
    E1_LAST_Q_LO.store(q as u64, Ordering::Relaxed);
    E1_LAST_ANS.store(ans, Ordering::Relaxed);
    E1_CALLS.fetch_add(1, Ordering::Relaxed);
}
fn e1_last_q() -> u128 {
    ((E1_LAST_Q_HI.load(Ordering::Relaxed) as u128) << 64) | E1_LAST_Q_LO.load(Ordering::Relaxed) as u128
}

/// Over-approximation of `liquidation_projected_healthy_after_close`: ANY of its Ok answers, and
/// for a partial close also `Err(NonProgress)` (the fee-floor refusal). LV-E2 proves the real fn
/// never errs at the full close; LV-E3 proves it errs only with `NonProgress` on a partial close.
#[allow(clippy::too_many_arguments)]
fn e1_projected_any(
    _config: V16Config,
    _cert: HealthCertV16,
    _capital: u128,
    _pnl: i128,
    _side: SideV16,
    old_abs_q: u128,
    _effective_price: u64,
    _raw_target_price: u64,
    _fee_bps: u64,
    close_q: u128,
) -> V16Result<bool> {
    let pick: u8 = kani::any();
    let res = if close_q == old_abs_q {
        if pick % 2 == 0 { Ok(true) } else { Ok(false) }
    } else {
        match pick % 3 {
            0 => Ok(true),
            1 => Ok(false),
            _ => Err(V16Error::NonProgress),
        }
    };
    let code = match res {
        Ok(true) => 1,
        Ok(false) => 2,
        Err(_) => 3,
    };
    e1_put_last(close_q, code);
    res
}

/// Over-approximation of `liquidation_partial_search_hi`: any Ok value <= old_abs_q (LV-E4 proves
/// the real fn returns Ok(h) with h <= old_abs_q under a validated config).
fn e1_search_hi_any(_config: V16Config, old_abs_q: u128, _effective_price: u64) -> V16Result<u128> {
    let h: u128 = kani::any();
    kani::assume(h <= old_abs_q);
    Ok(h)
}

#[kani::proof]
#[kani::unwind(52)]
#[kani::stub(super::liquidation_projected_healthy_after_close, e1_projected_any)]
#[kani::stub(super::liquidation_partial_search_hi, e1_search_hi_any)]
fn kani_live_e1_selector_size_in_range_and_partial_only_when_healthy() {
    let config = V16Config::public_user_fund(1, 0, 1);
    let equity: i128 = kani::any();
    let cert = HealthCertV16 { certified_equity: equity, valid: true, ..HealthCertV16::default() };
    let capital: u128 = kani::any();
    let pnl: i128 = kani::any();
    let old: u128 = kani::any();
    kani::assume(old >= 1 && old <= MAX_POSITION_ABS_Q);
    let price: u64 = kani::any();
    let fee_bps: u64 = kani::any();
    E1_LAST_ANS.store(0, Ordering::Relaxed);
    E1_CALLS.store(0, Ordering::Relaxed);

    let r = liquidation_engine_close_request_q(
        config, cert, capital, pnl, SideV16::Long, old, price, price, fee_bps,
    );
    kani::cover!(r.is_ok(), "the selector ran and returned");
    // never refuses
    assert!(r.is_ok(), "close-size selector refused a positioned account");
    let q = match r {
        Ok(q) => q,
        Err(_) => 0,
    };
    // in range: a liquidation always closes something and never more than the leg
    assert!(q >= 1 && q <= old, "close size out of [1, old]");
    // bankrupt / negative PnL: full close, no search
    if equity < 0 || pnl < 0 {
        assert_eq!(q, old);
        assert_eq!(E1_CALLS.load(Ordering::Relaxed), 0);
    }
    // partial only when the production projection said healthy for exactly that size
    if q < old {
        assert_eq!(e1_last_q(), q, "partial size is not the last size the projection judged");
        assert_eq!(E1_LAST_ANS.load(Ordering::Relaxed), 1, "partial close returned without a healthy projection");
    }
    kani::cover!(equity < 0 && q == old, "bankrupt: full close");
    kani::cover!(equity >= 0 && pnl >= 0 && q == old && E1_CALLS.load(Ordering::Relaxed) == 1, "full close not healthy: full close");
    kani::cover!(q < old && q > 1, "partial close chosen by the search");
    kani::cover!(q == 1 && old > 1, "minimal partial close of one atom");
    kani::cover!(equity >= 0 && pnl >= 0 && q == old && E1_CALLS.load(Ordering::Relaxed) >= 3, "search fell back to the full close");
}

// =============================================================================================
// LV-E2 / LV-E3: the production health projection is TOTAL on states the engine can certify:
// at the full close it never errs; at a partial close it errs only with NonProgress, and only
// when the configured absolute fee floor is on.
// =============================================================================================

struct ProjectionInputs {
    config: V16Config,
    cert: HealthCertV16,
    capital: u128,
    pnl: i128,
    side: SideV16,
    old: u128,
    price: u64,
    raw: u64,
}

/// A validated config (the config fields the projection reads, under the bounds
/// `validate_public_user_fund_shape` enforces) and a certificate whose maintenance requirement
/// contains this leg's own requirement (the refresh certificate sums exactly
/// `liquidation_leg_maintenance_requirement` per leg; nothing changes between the refresh and the
/// sizing inside one liquidation call).
fn projection_inputs() -> ProjectionInputs {
    let mut config = V16Config::public_user_fund(1, 0, 1);
    config.maintenance_margin_bps = kani::any();
    config.liquidation_fee_bps = kani::any();
    config.min_liquidation_abs = kani::any();
    config.liquidation_fee_cap = kani::any();
    config.min_nonzero_mm_req = kani::any();
    kani::assume(config.maintenance_margin_bps <= MAX_MARGIN_BPS);
    kani::assume(config.liquidation_fee_bps <= MAX_MARGIN_BPS);
    kani::assume(config.min_liquidation_abs <= config.liquidation_fee_cap);
    kani::assume(config.liquidation_fee_cap <= MAX_PROTOCOL_FEE_ABS);
    // product question Q4 (no upper bound in the validator): the floor is at most the largest
    // account notional, i.e. a market whose floor exceeds every possible position is excluded
    kani::assume(config.min_nonzero_mm_req >= 1 && config.min_nonzero_mm_req <= MAX_ACCOUNT_NOTIONAL);
    let side = if kani::any() { SideV16::Long } else { SideV16::Short };
    let old: u128 = kani::any();
    kani::assume(old >= 1 && old <= MAX_POSITION_ABS_Q);
    let price: u64 = kani::any();
    kani::assume(price >= 1 && price <= MAX_ORACLE_PRICE);
    let raw: u64 = kani::any();
    kani::assume(raw >= 1 && raw <= MAX_ORACLE_PRICE);
    let leg_m = liquidation_leg_maintenance_requirement(config, old, side, price, raw);
    // the refresh computed exactly this term; it never fails in range (part of the claim)
    assert!(leg_m.is_ok(), "leg maintenance requirement errs in range");
    let leg_m = match leg_m {
        Ok(v) => v,
        Err(_) => 0,
    };
    let other: u128 = kani::any();
    // other legs: at most 15 more legs of at most MAX_ACCOUNT_NOTIONAL notional + lag penalty each
    kani::assume(other <= 32 * MAX_ACCOUNT_NOTIONAL);
    let maint = other + leg_m;
    // capital is bounded by the vault; equity by capital plus a PnL the i128 accounting holds
    let capital: u128 = kani::any();
    kani::assume(capital <= MAX_VAULT_TVL);
    let pnl: i128 = kani::any();
    let equity: i128 = kani::any();
    kani::assume(equity > -(1i128 << 120) && equity < (1i128 << 120));
    kani::assume(pnl > -(1i128 << 120) && pnl < (1i128 << 120));
    let cert = HealthCertV16 {
        certified_equity: equity,
        certified_maintenance_req: maint,
        certified_liq_deficit: if equity < 0 { equity.unsigned_abs() } else { maint.saturating_sub(equity as u128) },
        valid: true,
        ..HealthCertV16::default()
    };
    ProjectionInputs { config, cert, capital, pnl, side, old, price, raw }
}

#[kani::proof]
#[kani::unwind(4)]
fn kani_live_e2_full_close_projection_never_errs() {
    let p = projection_inputs();
    let r = liquidation_projected_healthy_after_close(
        p.config, p.cert, p.capital, p.pnl, p.side, p.old, p.price, p.raw, p.config.liquidation_fee_bps, p.old,
    );
    assert!(r.is_ok(), "full-close projection errs on a certifiable state");
    kani::cover!(r == Ok(true), "full close restores maintenance");
    kani::cover!(r == Ok(false), "full close does not restore maintenance");
    kani::cover!(p.raw != p.price && r.is_ok(), "target-lag penalty present");
    kani::cover!(p.pnl >= 0 && p.capital > 0 && p.config.liquidation_fee_bps > 0 && r.is_ok(), "fee charged in the projection");
    kani::cover!(p.old == MAX_POSITION_ABS_Q && p.price == MAX_ORACLE_PRICE && r.is_ok(), "largest position at the largest price");
}

#[kani::proof]
#[kani::unwind(4)]
fn kani_live_e3_partial_projection_errs_only_on_the_fee_floor() {
    let p = projection_inputs();
    let close_q: u128 = kani::any();
    kani::assume(close_q >= 1 && close_q < p.old);
    let r = liquidation_projected_healthy_after_close(
        p.config, p.cert, p.capital, p.pnl, p.side, p.old, p.price, p.raw, p.config.liquidation_fee_bps, close_q,
    );
    assert!(r.is_ok() || r == Err(V16Error::NonProgress), "partial projection errs with something other than the fee floor");
    if r == Err(V16Error::NonProgress) {
        assert!(p.config.min_liquidation_abs != 0, "NonProgress without a fee floor");
    }
    kani::cover!(r == Ok(true), "healthy partial");
    kani::cover!(r == Ok(false), "unhealthy partial");
    kani::cover!(r == Err(V16Error::NonProgress), "sub-floor partial refused");
}

// =============================================================================================
// LV-E4: the partial-search bound is total and never above the leg.
// =============================================================================================

#[kani::proof]
#[kani::unwind(4)]
fn kani_live_e4_partial_search_bound_is_total_and_within_the_leg() {
    let mut config = V16Config::public_user_fund(1, 0, 1);
    config.maintenance_margin_bps = kani::any();
    config.min_nonzero_mm_req = kani::any();
    kani::assume(config.maintenance_margin_bps <= MAX_MARGIN_BPS);
    kani::assume(config.min_nonzero_mm_req >= 1 && config.min_nonzero_mm_req <= MAX_ACCOUNT_NOTIONAL);
    let old: u128 = kani::any();
    kani::assume(old >= 1 && old <= MAX_POSITION_ABS_Q);
    let price: u64 = kani::any();
    kani::assume(price >= 1 && price <= MAX_ORACLE_PRICE);
    let r = liquidation_partial_search_hi(config, old, price);
    assert!(r.is_ok(), "partial-search bound errs on a validated config");
    let h = match r {
        Ok(h) => h,
        Err(_) => 0,
    };
    assert!(h <= old);
    kani::cover!(h == 0 && config.maintenance_margin_bps == 0, "no proportional margin: no partial search");
    kani::cover!(h == 0 && config.maintenance_margin_bps > 0, "leg inside the floor region: no partial search");
    kani::cover!(h > 0 && h < old, "partial search over a proper prefix");
}

// =============================================================================================
// LV-E5: the public crank dispatches Liquidate for every liquidatable account, on a leg the
// liquidation kernel accepts.
// =============================================================================================

fn any_lifecycle() -> AssetLifecycleV16 {
    match kani::any::<u8>() % 6 {
        0 => AssetLifecycleV16::Disabled,
        1 => AssetLifecycleV16::PendingActivation,
        2 => AssetLifecycleV16::Active,
        3 => AssetLifecycleV16::DrainOnly,
        4 => AssetLifecycleV16::Retired,
        _ => AssetLifecycleV16::Recovery,
    }
}
fn any_side_mode() -> SideModeV16 {
    match kani::any::<u8>() % 3 {
        0 => SideModeV16::Normal,
        1 => SideModeV16::DrainOnly,
        _ => SideModeV16::ResetPending,
    }
}

#[kani::proof]
#[kani::unwind(18)]
fn kani_live_e5_liquidatable_account_is_dispatched_to_liquidate() {
    // (a) per-leg classification
    let active: bool = kani::any();
    let lifecycle = any_lifecycle();
    let basis: i128 = kani::any();
    let matched_oi: u128 = kani::any();
    let mode = any_side_mode();
    let asset_epoch: u64 = kani::any();
    let leg_epoch: u64 = kani::any();
    let b_pending: bool = kani::any();
    let (_b, refresh, liq, _reset) = V16Core::kernel_auto_crank_leg_flags(
        active, lifecycle, basis, matched_oi, mode, asset_epoch, leg_epoch, b_pending,
    );
    let prior_reset = V16Core::kernel_is_prior_reset_obligation(mode, asset_epoch, leg_epoch);
    let reducible_lifecycle = matches!(lifecycle, AssetLifecycleV16::Active | AssetLifecycleV16::DrainOnly);
    // every leg the liquidation kernel can reduce is flagged, and only those
    assert_eq!(liq, active && reducible_lifecycle && basis != 0 && matched_oi != 0 && !prior_reset);
    // a flagged leg passes the kernel's own lifecycle gate (`require_asset_live_reducible`)
    if liq {
        assert!(reducible_lifecycle && refresh);
    }
    // (b) the first flagged slot is chosen
    let flags: [bool; V16_MAX_PORTFOLIO_ASSETS_N] = kani::any();
    let first = V16Core::first_actionable_slot(flags);
    match first {
        Some(s) => {
            assert!(s < V16_MAX_PORTFOLIO_ASSETS_N && flags[s]);
        }
        None => {
            let mut i = 0;
            while i < V16_MAX_PORTFOLIO_ASSETS_N {
                assert!(!flags[i]);
                i += 1;
            }
        }
    }
    // (c) the plan selector: Liquidate whenever liquidatable and no higher-priority work exists
    let summary = V16Core::actionable_summary_from_signals(
        kani::any(), kani::any(), kani::any(), kani::any(), kani::any(), kani::any(), kani::any(), kani::any(),
    );
    let liq_slot: usize = kani::any();
    let plan = V16Core::select_auto_crank_plan(
        summary,
        kani::any(),
        liq_slot,
        None,
        PermissionlessRecoveryReasonV16::ExplicitLossOrDustAuditOverflow,
    );
    let higher = summary.expired_close
        || summary.recovery_eligible
        || summary.resolved_winner
        || summary.pending_close
        || summary.b_stale;
    if summary.liquidatable && !higher {
        assert_eq!(plan, AutoCrankPlanV16::Liquidate { asset_index: liq_slot });
    }
    if matches!(plan, AutoCrankPlanV16::Liquidate { .. }) {
        assert!(summary.liquidatable && !higher);
    }
    kani::cover!(liq, "a liquidatable leg");
    kani::cover!(!liq && active && basis != 0 && matched_oi != 0 && reducible_lifecycle && prior_reset, "prior-reset obligation is not liquidation work");
    kani::cover!(summary.liquidatable && !higher && summary.stale && summary.source_liens_releasable, "liquidation outranks refresh and lien release");
    kani::cover!(summary.liquidatable && summary.b_stale, "B settlement first");
}

// =============================================================================================
// LV-E6 (#126, engine 17cdea8a): a liquidation's position change is a strict reduction toward
// zero, so the new side-flip risk-increase classifier never marks it risk-increasing and the
// trade risk gate never refuses it. (The liquidation kernel does not call the trade preflight at
// all; this proves the gate would admit it even if it did.)
// =============================================================================================

#[kani::proof]
#[kani::unwind(4)]
fn kani_live_e6_liquidation_reduction_is_never_risk_increasing() {
    let basis: i128 = kani::any();
    kani::assume(basis != 0 && basis.unsigned_abs() <= MAX_POSITION_ABS_Q);
    let side = if basis > 0 { SideV16::Long } else { SideV16::Short };
    let requested: u128 = kani::any();
    kani::assume(requested >= 1 && requested <= MAX_POSITION_ABS_Q);
    let r = V16Core::kernel_reduce_position_delta(basis, side, requested);
    assert!(r.is_ok());
    let (q, delta) = match r {
        Ok(v) => v,
        Err(_) => (0, 0),
    };
    let next = basis + delta;
    assert_eq!(q, requested.min(basis.unsigned_abs()));
    assert_eq!(delta.unsigned_abs(), q);
    assert!(next == 0 || (next > 0) == (basis > 0), "liquidation flipped the side");
    assert!(next.unsigned_abs() < basis.unsigned_abs(), "liquidation did not reduce");
    assert!(!position_change_increases_risk(basis, next), "#126 classifier marks a liquidation risk-increasing");
    assert!(!trade_account_requires_initial_margin(basis, next), "#126 IM gate would apply to a liquidation");
    let stale: bool = kani::any();
    let lag: bool = kani::any();
    assert!(trade_preflight_risk_gate(false, stale, lag, false).is_ok());
    kani::cover!(next == 0, "full close");
    kani::cover!(next != 0 && basis < 0, "partial close of a short");
    kani::cover!(next != 0 && basis > 0, "partial close of a long");
}

// =============================================================================================
// LV-E7: the full production kernel. A below-maintenance account (production certificate) is
// liquidated with progress; an account at or above maintenance is refused with NonProgress and
// keeps its capital and position. Reachable state: built through public entries only; the
// symbolic inputs are the deposit (capital) and the domain insurance.
// =============================================================================================

/// PF stubs: these two validators are compiled only under `cfg(any(test, kani, audit-scan))`;
/// the deployed SBF build (wrapper dependency line `features = ["fork-facade"]`) does not have
/// them, so replacing them with Ok(()) is exactly the deployed program.
pub fn live_pf_full_audit_scan_absent<'a, T>(_s: &MarketGroupV16View<'a, T>) -> V16Result<()>
where
    T: 'a,
{
    Ok(())
}
pub fn live_pf_slots_shape_absent<S: MarketSlotV16View>(_h: &MarketGroupV16HeaderAccount, _slots: &[S]) -> V16Result<()> {
    Ok(())
}

const E7_P0: u64 = 1_000_000;
const E7_UNIT_Q: i128 = 10_000;
const E7_INS_DOMAIN: usize = 1;

fn e7_account(seed: u8) -> PortfolioAccountV16Account {
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new([1; 32], [seed; 32], [3; 32]));
    let mut a = PortfolioAccountV16Account::default();
    let _ = a.init_empty_in_place(header);
    a
}

fn e7_abs_basis(a: &PortfolioAccountV16Account) -> u128 {
    let mut s = 0u128;
    let mut i = 0usize;
    while i < V16_MAX_PORTFOLIO_ASSETS_N {
        if let Ok(l) = a.legs[i].try_to_runtime() {
            if l.active {
                s += l.basis_pos_q.unsigned_abs();
            }
        }
        i += 1;
    }
    s
}

#[kani::proof]
#[kani::unwind(18)]
#[kani::stub(super::MarketGroupV16View::validate_shape_full_audit_scan, live_pf_full_audit_scan_absent)]
#[kani::stub(super::MarketGroupV16HeaderAccount::validate_dynamic_market_slots_shape, live_pf_slots_shape_absent)]
fn kani_live_e7_liquidate_progresses_below_maintenance() {
    // Fixture = tests/live_g9_native.rs `e7_fixture` (native twin validated on a 45-point grid).
    let mut cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    cfg.maintenance_margin_bps = 1_000;
    cfg.initial_margin_bps = 1_000;
    cfg.max_price_move_bps_per_slot = 500;
    cfg.max_abs_funding_e9_per_slot = 0;
    cfg.min_funding_lifetime_slots = 1;
    cfg.liquidation_fee_bps = 50;
    cfg.liquidation_fee_cap = 1_000_000;
    cfg.min_liquidation_abs = 2;
    cfg.min_nonzero_mm_req = 100;
    cfg.min_nonzero_im_req = 101;
    cfg.max_accrual_dt_slots = 1;
    cfg.max_bankrupt_close_lifetime_slots = 100;
    let header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0);
    assert!(header.is_ok(), "fixture config refused (validated natively: must not happen)");
    let mut header = match header {
        Ok(h) => h,
        Err(_) => return,
    };
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    let _ = header.activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, E7_P0, 1);
    let mut t = e7_account(10);
    let mut cp = e7_account(11);
    let capital: u128 = kani::any();
    kani::assume(capital >= 2_000 && capital <= 4_800);
    let insurance: u128 = kani::any();
    kani::assume(insurance <= 600);

    let mut g = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    while g.markets[0].engine.asset.slot_last.get() < 2 {
        let next = g.markets[0].engine.asset.slot_last.get() + 1;
        let _ = g.accrue_asset_to_not_atomic(0, next, E7_P0, 0, true);
    }
    let setup_ok = {
        let mut tv = PortfolioV16ViewMut::new(&mut t);
        let mut cv = PortfolioV16ViewMut::new(&mut cp);
        g.deposit_not_atomic(&mut tv, capital).is_ok()
            && g.deposit_not_atomic(&mut cv, 1_000_000).is_ok()
            && g.deposit_domain_insurance_not_atomic(E7_INS_DOMAIN, insurance).is_ok()
            && g
                .execute_trade_with_fee_loss_stale_scoped_not_atomic(
                    &mut tv,
                    &mut cv,
                    TradeRequestV16 { asset_index: 0, size_q: 2 * E7_UNIT_Q, exec_price: E7_P0, fee_bps: 0 },
                    true,
                )
                .is_ok()
    };
    kani::assume(setup_ok);
    let mut p = E7_P0;
    let mut k = 0;
    while k < 3 {
        p -= p * 49 / 1000;
        let slot = g.header.current_slot.get() + 1;
        let ok = g.accrue_asset_to_not_atomic(0, slot, p, 0, true).is_ok();
        kani::assume(ok);
        g.markets[0].engine.asset.raw_oracle_target_price = V16PodU64::new(p);
        k += 1;
    }
    // the keeper's refresh (public crank) certifies the account at the current price
    let now_slot = g.header.current_slot.get();
    let refreshed = {
        let mut tv = PortfolioV16ViewMut::new(&mut t);
        g.permissionless_crank_not_atomic(
            &mut tv,
            PermissionlessCrankRequestV16 {
                now_slot,
                asset_index: 0,
                effective_price: p,
                funding_rate_e9: 0,
                action: PermissionlessCrankActionV16::Refresh,
            },
        )
    };
    kani::assume(refreshed.is_ok());
    let c0 = t.health_cert.try_to_runtime();
    assert!(c0.is_ok());
    let c0 = match c0 {
        Ok(c) => c,
        Err(_) => return,
    };
    // one refresh crank leaves the account current (the native twin asserts the same)
    assert!(c0.valid && t.stale_state == 0 && t.b_stale_state == 0, "refresh left the account stale");
    let cap0 = t.capital.get();
    let pos0 = e7_abs_basis(&t);
    kani::cover!(c0.valid, "fixture reached a certified account");

    // THE CALL: the production liquidation kernel (the crank's Liquidate dispatch target)
    let r = {
        let mut tv = PortfolioV16ViewMut::new(&mut t);
        g.liquidate_account_not_atomic(&mut tv, LiquidationRequestV16 { asset_index: 0 })
    };
    let c1 = t.health_cert.try_to_runtime();
    let legs_after = e7_abs_basis(&t);

    if c0.certified_liq_deficit != 0 {
        // LIVENESS: below maintenance => Ok with progress
        assert!(r.is_ok(), "below-maintenance account refused");
        if let Ok(out) = r {
            assert!(out.closed_q >= 1);
            assert!(legs_after < pos0, "no position progress");
            if let Ok(c1) = c1 {
                // ends flat or at/above maintenance
                assert!(legs_after == 0 || c1.certified_liq_deficit == 0, "partial liquidation left the account below maintenance");
            }
            kani::cover!(legs_after != 0, "partial liquidation");
            kani::cover!(legs_after == 0 && c0.certified_equity >= 0, "full close of a solvent account");
            kani::cover!(c0.certified_equity < 0, "bankrupt account liquidated");
            kani::cover!(out.insurance_used > 0, "insurance-touching branch");
            kani::cover!(out.residual_booked + out.explicit_loss > 0, "bad-debt residual booked");
            kani::cover!(out.fee_charged > 0, "liquidation fee charged");
        }
    } else {
        // SAFETY converse: at or above maintenance => NonProgress, capital and position unchanged
        assert_eq!(r, Err(V16Error::NonProgress));
        assert_eq!(t.capital.get(), cap0);
        assert_eq!(legs_after, pos0);
        kani::cover!(c0.certified_equity >= 0 && (c0.certified_equity as u128) == c0.certified_maintenance_req, "exactly at maintenance: refused");
        kani::cover!(c0.certified_equity >= 0 && (c0.certified_equity as u128) > c0.certified_maintenance_req, "above maintenance: refused");
    }
}

// =============================================================================================
// LV-E8 (split step for the NO-VERDICT E2/E3): the only symbolic x symbolic multiply in the
// projection's notional terms. The production `liquidation_risk_notional_ceil` is total on the
// protocol caps and its result never exceeds MAX_ACCOUNT_NOTIONAL (so the fee function's
// `fee_notional > MAX_ACCOUNT_NOTIONAL` refusal is unreachable for any position at any price).
// =============================================================================================

#[kani::proof]
#[kani::unwind(4)]
fn kani_live_e8_risk_notional_is_total_and_within_the_account_cap() {
    let abs_q: u128 = kani::any();
    let price: u64 = kani::any();
    kani::assume(abs_q <= MAX_POSITION_ABS_Q && price <= MAX_ORACLE_PRICE);
    let r = liquidation_risk_notional_ceil(abs_q, price);
    assert!(r.is_ok(), "risk notional errs inside the caps");
    let n = match r {
        Ok(n) => n,
        Err(_) => 0,
    };
    assert!(n <= MAX_ACCOUNT_NOTIONAL, "notional above the account cap: the fee function would refuse");
    if abs_q > 0 && price > 0 {
        assert!(n >= 1);
    }
    kani::cover!(abs_q == MAX_POSITION_ABS_Q && price == MAX_ORACLE_PRICE && n == MAX_ACCOUNT_NOTIONAL, "the cap is met exactly at the extremes");
    kani::cover!(n > 0 && n < 1000, "small notional");
}
