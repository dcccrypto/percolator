//! v22kani WEAK agent (2026-10-20): proof-only forwarding shims. cfg(kani) child module of v16; never in a
//! program build. Each function forwards to ONE private production function with no logic of its own (same
//! pattern as kani_v22_shims.rs / kani_audit_1017_shims.rs).
#![allow(dead_code)]
use super::*;

/// The production refresh-required predicate the auto-crank classifier uses for `stale` (v16.rs:2486).
pub fn kani_weak1020_live_account_refresh_required(
    live: bool,
    cert_current: bool,
    reset_obligation: bool,
    released_obligation: bool,
    lapsed_source_backing: bool,
) -> bool {
    V16Core::kernel_live_account_refresh_required(
        live,
        cert_current,
        reset_obligation,
        released_obligation,
        lapsed_source_backing,
    )
}

/// The production summary assembler (v16.rs:2769).
#[allow(clippy::too_many_arguments)]
pub fn kani_weak1020_actionable_summary_from_signals(
    stale: bool,
    b_stale: bool,
    pending_close: bool,
    expired_close: bool,
    liquidatable: bool,
    source_liens_releasable: bool,
    recovery_eligible: bool,
    resolved_winner: bool,
) -> ActionableSummaryV16 {
    V16Core::actionable_summary_from_signals(
        stale,
        b_stale,
        pending_close,
        expired_close,
        liquidatable,
        source_liens_releasable,
        recovery_eligible,
        resolved_winner,
    )
}
