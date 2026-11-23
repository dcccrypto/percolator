//! Market config validation: where the accept/reject line sits, field by field and in pairs
//! (job P39, tests only). Also a seeded random search for a config that `validate_public_user_fund`
//! ACCEPTS but that later panics, makes liquidation impossible, or lets a solvent account go
//! insolvent in a single accrual step.
//!
//! Every test name contains `config_envelope` so `cargo test -- config_envelope` selects the area.
#![allow(dead_code)]

use percolator::{
    EngineAssetSlotV16Account, LiquidationRequestV16, Market, MarketGroupV16HeaderAccount,
    MarketGroupV16ViewMut, PermissionlessCrankActionV16, PermissionlessCrankRequestV16,
    PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, TradeRequestV16, V16Config, V16Error, MAX_PROTOCOL_FEE_ABS,
    POS_SCALE,
};

fn ok(c: &V16Config) -> bool {
    c.validate_public_user_fund().is_ok()
}

/// A valid 1-asset market: MM 5%, IM 10%, 1%/slot move cap over 2 slots, no fees.
fn base() -> V16Config {
    let mut c = V16Config::public_user_fund_with_market_slots(1, 1, 0, 6_480_000);
    c.maintenance_margin_bps = 500;
    c.initial_margin_bps = 1_000;
    c.max_price_move_bps_per_slot = 100;
    c.max_accrual_dt_slots = 2;
    c.min_funding_lifetime_slots = 2;
    c.min_nonzero_mm_req = 2;
    c.min_nonzero_im_req = 3;
    c
}

/// `base()` with real, non-clamped liquidation fee room (cap huge, floor 0).
fn base_fee(fee_bps: u64) -> V16Config {
    let mut c = base();
    c.liquidation_fee_bps = fee_bps;
    c.liquidation_fee_cap = u64::MAX as u128;
    c
}

/// 10x band market (same numbers as tests/band_rent_edge.rs).
fn band() -> V16Config {
    let mut c = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    c.maintenance_margin_bps = 500;
    c.initial_margin_bps = 1_000;
    c.min_nonzero_mm_req = 10_000;
    c.min_nonzero_im_req = 20_000;
    c.liquidation_fee_bps = 50;
    c.liquidation_fee_cap = 1_000_000_000_000;
    c.max_abs_funding_e9_per_slot = 111;
    c.max_price_move_bps_per_slot = 100;
    c.max_accrual_dt_slots = 3;
    c.min_funding_lifetime_slots = 3;
    c.max_trading_fee_bps = 100;
    c.rent_max_e9_per_slot = 23;
    c.max_bankrupt_close_lifetime_slots = 1_000_000;
    c.band_bps = 130;
    c.band_max_epoch_slots = 600;
    c.band_max_pin_slots = 9_000;
    c.band_max_positions_per_side = percolator::band_rent::BAND_MAX_POSITIONS_PER_SIDE;
    c.band_min_leg_notional = 1;
    c
}

/// Set a field (any integer type) on a copy of `$base` and report accept/reject.
macro_rules! with {
    ($base:expr, $f:ident = $v:expr) => {{
        let mut c = $base;
        c.$f = $v as _;
        c
    }};
}

/// Assert the accept/reject answer for each `(value, accepted)` pair of one field.
macro_rules! edge {
    ($base:expr, $f:ident, [$(($v:expr, $want:expr)),+ $(,)?]) => {{
        $(
            let c = with!($base, $f = $v);
            assert_eq!(
                ok(&c), $want,
                "{} = {}: validate said {:?}", stringify!($f), $v as u128, c.validate_public_user_fund()
            );
        )+
    }};
}

// ---------------------------------------------------------------- single-field boundaries

#[test]
fn config_envelope_base_fixtures_are_accepted() {
    assert!(ok(&base()));
    assert!(ok(&base_fee(100)));
    assert!(ok(&band()));
    assert!(ok(&V16Config::public_user_fund(1, 0, 1)));
}

#[test]
fn config_envelope_asset_count_and_slot_count_bounds() {
    // 1..=16 assets, slots >= assets, slots 0 refused.
    let mk = |a: u16, s: u32| V16Config::public_user_fund_with_market_slots(a, s, 0, 10);
    assert!(ok(&mk(1, 1)));
    assert!(ok(&mk(16, 16)));
    assert!(!ok(&mk(17, 17)));
    assert!(!ok(&mk(0, 1)));
    assert!(!ok(&mk(1, 0)));
    assert!(!ok(&mk(2, 1)));
    assert!(ok(&mk(1, 5)));
    assert!(ok(&mk(16, 1000)));
    assert!(ok(&mk(1, u32::MAX)));
}

#[test]
fn config_envelope_h_min_h_max_bounds() {
    let b = base();
    edge!(
        b,
        h_max,
        [(0u64, false), (1u64, true), (2u64, true), (u64::MAX, true)]
    );
    // h_min = 0 in base; h_min <= h_max
    assert!(ok(&with!(with!(b, h_max = 5u64), h_min = 5u64)));
    assert!(!ok(&with!(with!(b, h_max = 5u64), h_min = 6u64)));
    assert!(!ok(&with!(with!(b, h_max = 0u64), h_min = 0u64)));
}

#[test]
fn config_envelope_margin_bps_bounds() {
    let b = base();
    // IM: >= MM, <= 10_000
    edge!(
        b,
        initial_margin_bps,
        [
            (499u64, false),
            (500u64, true),
            (10_000u64, true),
            (10_001u64, false)
        ]
    );
    // MM at the fixture: 1% move over 2 slots = 200 bps of loss; MM must be strictly above it
    // (201 ok, 200 refused). With fee 0 the loss never exceeds the requirement, so the 200 refusal
    // looks like the exact validator giving up (step limit) rather than a real shortfall: it is
    // conservative, not unsafe.
    edge!(
        b,
        maintenance_margin_bps,
        [
            (0u64, false),
            (1u64, false),
            (199u64, false),
            (200u64, false),
            (201u64, true),
            (399u64, true),
            (400u64, true),
            (500u64, true),
            (1_000u64, true),
            (1_001u64, false)
        ]
    );
    // MM above IM is refused even if the envelope is fine.
    assert!(!ok(&with!(
        with!(b, maintenance_margin_bps = 2_000u64),
        initial_margin_bps = 1_999u64
    )));
    assert!(ok(&with!(
        with!(b, maintenance_margin_bps = 2_000u64),
        initial_margin_bps = 2_000u64
    )));
    // full-margin market: MM = IM = 100% is the simplest accepted config
    let full = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    assert!(ok(&full));
    edge!(
        full,
        initial_margin_bps,
        [(10_000u64, true), (10_001u64, false)]
    );
}

#[test]
fn config_envelope_min_nonzero_requirement_pair() {
    let b = base();
    // 0 < mm_req < im_req.
    edge!(
        b,
        min_nonzero_mm_req,
        [
            (0u64, false),
            (1u64, true),
            (2u64, true),
            (3u64, false),
            (4u64, false)
        ]
    );
    edge!(
        b,
        min_nonzero_im_req,
        [
            (0u64, false),
            (1u64, false),
            (2u64, false),
            (3u64, true),
            (1_000u64, true)
        ]
    );
    // mm_req = 1 needs im_req >= 2
    let c1 = with!(b, min_nonzero_mm_req = 1u64);
    edge!(c1, min_nonzero_im_req, [(1u64, false), (2u64, true)]);
    // very large floors (im_req kept above mm_req so only the envelope can refuse them)
    for mm in [1u128 << 60, 1u128 << 100, u128::MAX / 2] {
        let mut c = b;
        c.min_nonzero_mm_req = mm;
        c.min_nonzero_im_req = mm + 1;
        // Observation, not a safety bug: 2^60 and 2^100 are ACCEPTED (u128::MAX/2 is not). A floor
        // above MAX_VAULT_TVL (1e16) means no account can ever hold a position, so the market is
        // accepted but unusable. Pinned here so a change is noticed.
        assert_eq!(ok(&c), mm != u128::MAX / 2, "mm_req {mm}");
    }
}

#[test]
fn config_envelope_price_move_cap_bounds() {
    let b = base();
    // move * dt (2) is the price budget; at MM 5% it must stay strictly under 500 bps: 249 ok, 250 refused.
    edge!(
        b,
        max_price_move_bps_per_slot,
        [
            (0u64, false),
            (1u64, true),
            (100u64, true),
            (200u64, true),
            (249u64, true),
            (250u64, false),
            (251u64, false),
            (500u64, false),
            (10_000u64, false),
            (10_001u64, false),
            (u64::MAX, false)
        ]
    );
    // the shape cap (<= 10_000) is separate from the envelope: full-margin, dt=1 accepts exactly 10_000.
    let full = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    edge!(
        full,
        max_price_move_bps_per_slot,
        [(10_000u64, true), (10_001u64, false), (0u64, false)]
    );
}

#[test]
fn config_envelope_accrual_dt_and_funding_lifetime_pair() {
    let b = base();
    // dt multiplies the price budget: 100 bps/slot at MM 5% allows dt up to 4 (dt 5 = 500 bps = MM).
    // (the lifetime is raised with dt so the pair rule does not hide the envelope)
    for (dt, want) in [
        (1u64, true),
        (2, true),
        (3, true),
        (4, true),
        (5, false),
        (100, false),
    ] {
        let mut c = b;
        c.max_accrual_dt_slots = dt;
        c.min_funding_lifetime_slots = dt;
        assert_eq!(ok(&c), want, "dt {dt}");
    }
    // dt alone, lifetime left at 2: dt > lifetime is the pair rule
    edge!(
        b,
        max_accrual_dt_slots,
        [
            (0u64, false),
            (1u64, true),
            (2u64, true),
            (3u64, false),
            (u64::MAX, false)
        ]
    );
    // lifetime must be >= dt (and is separately bounded by funding headroom)
    let d2 = b;
    edge!(
        d2,
        min_funding_lifetime_slots,
        [(0u64, false), (1u64, false), (2u64, true), (u64::MAX, true)]
    );
    // with funding at its max, the lifetime has a ceiling (headroom: ADL_ONE*maxP*rate*slots <= i128::MAX)
    let mut f = base();
    f.max_abs_funding_e9_per_slot = 10_000;
    f.max_accrual_dt_slots = 1;
    f.max_price_move_bps_per_slot = 10;
    edge!(
        f,
        min_funding_lifetime_slots,
        [
            (1_000_000u64, true),
            (100_000_000u64, false),
            (10_000_000_000u64, false)
        ]
    );
}

#[test]
fn config_envelope_funding_rate_bounds() {
    let b = base();
    edge!(
        b,
        max_abs_funding_e9_per_slot,
        [
            (0u64, true),
            (1u64, true),
            (100u64, true),
            (10_000u64, true),
            (10_001u64, false),
            (u64::MAX, false)
        ]
    );
    // funding eats the same loss budget as price. On the 10x band market (BSL over 2E slots)
    // the funding rate has a real ceiling: 4143 ok, 4144 refused (rent fixed at 23).
    let bd = band();
    edge!(
        bd,
        max_abs_funding_e9_per_slot,
        [
            (0u64, true),
            (111u64, true),
            (4_143u64, true),
            (4_144u64, false),
            (10_000u64, false)
        ]
    );
    // rent shares that budget: with funding 111 the rent ceiling is 4055
    edge!(
        bd,
        rent_max_e9_per_slot,
        [(4_055u64, true), (4_056u64, false)]
    );
}

#[test]
fn config_envelope_trading_fee_cap_bounds() {
    let b = base();
    edge!(
        b,
        max_trading_fee_bps,
        [
            (0u64, true),
            (1u64, true),
            (10_000u64, true),
            (10_001u64, false),
            (u64::MAX, false)
        ]
    );
}

#[test]
fn config_envelope_liquidation_fee_bps_bounds_at_mm_5pct() {
    // P33: with cap huge, 5% MM, 1%x2 move: fee <= 161 bps (the envelope says so, not 243).
    let mut lo = None;
    for fee in 0..=500u64 {
        let c = base_fee(fee);
        if ok(&c) {
            lo = Some(fee);
        } else {
            // the accepted set is a prefix: no accept after the first reject
            assert!(
                !ok(&base_fee(fee + 1)) || fee + 1 > 500,
                "non-prefix accept set at fee {fee}"
            );
        }
    }
    assert_eq!(
        lo,
        Some(161),
        "largest accepted liquidation fee at MM 5%, move 100x2"
    );
    edge!(
        base_fee(0),
        liquidation_fee_bps,
        [
            (0u64, true),
            (161u64, true),
            (162u64, false),
            (10_000u64, false),
            (10_001u64, false)
        ]
    );
    // when the cap is 0 the fee is always clamped to zero, so ANY fee_bps up to 10_000 is accepted
    edge!(
        base(),
        liquidation_fee_bps,
        [(10_000u64, true), (10_001u64, false)]
    );
}

#[test]
fn config_envelope_liquidation_fee_limit_by_mm() {
    // largest accepted fee_bps (cap huge) per MM, move 100 x dt 2 = 200 bps loss.
    let max_fee = |mm: u64| {
        let mut best = None;
        for fee in 0..=2_000u64 {
            let mut c = base_fee(fee);
            c.maintenance_margin_bps = mm;
            c.initial_margin_bps = mm.max(1_000);
            if ok(&c) {
                best = Some(fee);
            }
        }
        best
    };
    assert_eq!(max_fee(100), None); // MM below the price budget: even fee 0 refused
    assert_eq!(max_fee(200), None);
    assert_eq!(max_fee(300), Some(0)); // zero fee ok, any fee refused (mm_req=2 floor: at N=66 loss 2 + fee 1 > 2)
    assert_eq!(max_fee(500), Some(161));
    assert_eq!(max_fee(1_000), Some(476));
}

#[test]
fn config_envelope_liquidation_fee_cap_and_floor_pair() {
    let b = base();
    // cap <= MAX_PROTOCOL_FEE_ABS; min_liquidation_abs <= cap
    edge!(
        b,
        liquidation_fee_cap,
        [(0u64, true), (1u64, true), (1u128 << 60, true)]
    );
    assert!(ok(&with!(b, liquidation_fee_cap = MAX_PROTOCOL_FEE_ABS)));
    assert!(!ok(&with!(
        b,
        liquidation_fee_cap = MAX_PROTOCOL_FEE_ABS + 1
    )));
    assert!(!ok(&with!(b, liquidation_fee_cap = u128::MAX)));
    // floor above cap is refused
    assert!(!ok(&with!(b, min_liquidation_abs = 1u64))); // cap is 0 in base
    let c = with!(b, liquidation_fee_cap = 100u64);
    assert!(!ok(&with!(c, min_liquidation_abs = 101u64)));
    // a fee floor costs real margin: with cap 100, floor 1 is the largest accepted at
    // mm_req 2 / im_req 3; floor 2 is refused (loss + floor fee > max(mm_req, 5%) at small N)
    assert!(ok(&with!(c, min_liquidation_abs = 1u64)));
    assert!(!ok(&with!(c, min_liquidation_abs = 2u64)));
    assert!(!ok(&with!(c, min_liquidation_abs = 100u64)));
    // a larger mm_req floor buys room for a larger fee floor
    let mut roomy = with!(c, min_liquidation_abs = 30u64);
    assert!(!ok(&roomy));
    roomy.min_nonzero_mm_req = 100;
    roomy.min_nonzero_im_req = 200;
    assert!(ok(&roomy));
}

#[test]
fn config_envelope_nonzero_fee_needs_mm_req_2_im_req_3() {
    // P33: a nonzero liquidation fee cannot be paid at tiny notionals with mm_req = 1.
    let mut c = base_fee(100);
    assert!(ok(&c));
    c.min_nonzero_mm_req = 1;
    c.min_nonzero_im_req = 2;
    assert!(!ok(&c), "fee > 0 with mm_req=1 must be refused");
    c.min_nonzero_mm_req = 2;
    c.min_nonzero_im_req = 2;
    assert!(!ok(&c), "mm_req must be < im_req");
    c.min_nonzero_im_req = 3;
    assert!(ok(&c));
}

#[test]
fn config_envelope_backing_fee_bounds() {
    let b = base();
    edge!(
        b,
        backing_fee_base_rate_e9_per_slot,
        [
            (0u64, true),
            (1_000_000_000u64, true),
            (1_000_000_001u64, false),
            (u64::MAX, false)
        ]
    );
    // the three rates are SUMMED against the cap
    let s = with!(
        with!(b, backing_fee_base_rate_e9_per_slot = 400_000_000u64),
        backing_fee_slope_at_kink_e9_per_slot = 300_000_000u64
    );
    edge!(
        s,
        backing_fee_slope_above_kink_e9_per_slot,
        [(300_000_000u64, true), (300_000_001u64, false)]
    );
    edge!(
        b,
        backing_fee_kink_util_bps,
        [
            (0u64, false),
            (1u64, true),
            (9_999u64, true),
            (10_000u64, false)
        ]
    );
    edge!(
        b,
        backing_freshness_buckets,
        [(0u64, false), (1u64, true), (2u64, false), (255u64, false)]
    );
}

#[test]
fn config_envelope_chunk_and_lifetime_counters_nonzero() {
    let b = base();
    edge!(
        b,
        max_account_b_settlement_chunks,
        [(0u64, false), (1u64, true), (u64::MAX, true)]
    );
    edge!(
        b,
        max_bankrupt_close_chunks,
        [(0u64, false), (1u64, true), (u64::MAX, true)]
    );
    edge!(
        b,
        max_bankrupt_close_lifetime_slots,
        [(0u64, false), (1u64, true), (u64::MAX, true)]
    );
    edge!(
        b,
        asset_activation_cooldown_slots,
        [(0u64, false), (1u64, true), (u64::MAX, true)]
    );
    // upstream 083a0d08: the public B chunk is the constant PUBLIC_B_CHUNK_ATOMS, not a config field.
    edge!(
        b,
        max_recovery_fallback_deviation_bps,
        [(0u64, true), (10_000u64, true), (10_001u64, false)]
    );
}

#[test]
fn config_envelope_mandatory_policy_flags_cannot_be_turned_off() {
    let b = base();
    for i in 0..10 {
        let mut c = b;
        match i {
            0 => c.margin_mode_realizable_full_shared_cross_margin = false,
            1 => c.source_credit_lien_required = false,
            2 => c.insurance_credit_reservation_required = false,
            3 => c.permissionless_recovery_enabled = false,
            4 => c.recovery_fallback_price_enabled = false,
            5 => c.recovery_fallback_envelope_enabled = false,
            6 => c.credit_lien_revalidation_required = false,
            7 => c.stale_certificate_penalty_enabled = false,
            8 => c.full_refresh_required_for_favorable_actions = false,
            _ => c.public_liveness_profile_crank_forward = false,
        }
        assert!(!ok(&c), "flag {i} off must be refused");
    }
}

// ---------------------------------------------------------------- band + rent

#[test]
fn config_envelope_band_field_bounds() {
    let b = band();
    edge!(
        b,
        band_bps,
        [
            (1u64, true),
            (130u64, true),
            (131u64, true),
            (200u64, false),
            (2_000u64, false),
            (2_001u64, false)
        ]
    );
    edge!(
        b,
        band_max_epoch_slots,
        [
            (0u64, false),
            (149u64, false),
            (150u64, true),
            (600u64, true),
            (1_000u64, true),
            (100_000u64, false)
        ]
    );
    edge!(
        b,
        band_max_pin_slots,
        [
            (0u64, false),
            (4_799u64, false),
            (4_800u64, true),
            (9_000u64, true)
        ]
    );
    edge!(
        b,
        band_max_positions_per_side,
        [(0u64, false), (1u64, true), (256u64, true), (257u64, false)]
    );
    edge!(
        b,
        band_min_leg_notional,
        [(0u64, false), (1u64, true), (u64::MAX, true)]
    );
    edge!(
        b,
        rent_max_e9_per_slot,
        [
            (0u64, true),
            (23u64, true),
            (1_000u64, true),
            (4_055u64, true),
            (4_056u64, false),
            (10_000u64, false),
            (10_001u64, false)
        ]
    );
}

#[test]
fn config_envelope_band_requires_single_asset_and_off_means_zero_clocks() {
    let b = band();
    for (a, s) in [(2u16, 2u32), (1, 2), (2, 3)] {
        let mut c = b;
        c.max_portfolio_assets = a;
        c.max_market_slots = s;
        assert!(
            !ok(&c),
            "band market with {a} assets / {s} slots must be refused"
        );
    }
    // band off: every band clock must be 0 (canonical encoding)
    let mut off = b;
    off.band_bps = 0;
    assert!(!ok(&off));
    off.band_max_epoch_slots = 0;
    assert!(!ok(&off));
    off.band_max_pin_slots = 0;
    assert!(!ok(&off));
    off.band_max_positions_per_side = 0;
    assert!(!ok(&off));
    off.band_min_leg_notional = 0;
    assert!(
        ok(&off),
        "all-zero band clocks with band_bps 0 is the legal off encoding"
    );
}

#[test]
fn config_envelope_band_safety_law_limits_epoch_and_band_width() {
    // the BSL charges funding + rent over 2E slots: epoch length can break it
    let b = band();
    assert!(ok(&b));
    let mut found_reject = false;
    for e in [600u64, 1_200, 2_400, 4_800, 9_600, 19_200, 38_400] {
        let mut c = b;
        c.band_max_epoch_slots = e;
        c.band_max_pin_slots = e * 8;
        if !ok(&c) {
            found_reject = true;
            // once rejected, longer epochs stay rejected (more loss time)
            let mut longer = c;
            longer.band_max_epoch_slots = e * 2;
            longer.band_max_pin_slots = e * 16;
            assert!(!ok(&longer), "epoch {e} refused but {} accepted", e * 2);
        }
    }
    assert!(
        found_reject,
        "an epoch long enough must break the Band Safety Law"
    );
    // wider band is worse: 131 ok, 200 refused (see bounds test); zero funding+rent does not rescue 200
    let mut wide = b;
    wide.band_bps = 200;
    wide.max_abs_funding_e9_per_slot = 0;
    wide.rent_max_e9_per_slot = 0;
    assert!(!ok(&wide));
}

// ---------------------------------------------------------------- monotonicity (pair properties)

fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn rnd(s: &mut u64, lo: u64, hi: u64) -> u64 {
    lo + splitmix(s) % (hi - lo + 1)
}

/// A random config; shape is valid, the envelope may or may not hold.
fn random_cfg(s: &mut u64) -> V16Config {
    let mut c = V16Config::public_user_fund_with_market_slots(1, 1, 0, 6_480_000);
    let mm = rnd(s, 200, 3_000);
    c.maintenance_margin_bps = mm;
    c.initial_margin_bps = (mm + rnd(s, 0, 3_000)).min(10_000);
    c.max_price_move_bps_per_slot = rnd(s, 1, 150);
    c.max_accrual_dt_slots = rnd(s, 1, 4);
    c.min_funding_lifetime_slots = c.max_accrual_dt_slots + rnd(s, 0, 1_000);
    c.max_abs_funding_e9_per_slot = if rnd(s, 0, 1) == 0 { 0 } else { rnd(s, 0, 200) };
    c.liquidation_fee_bps = rnd(s, 0, 600);
    c.liquidation_fee_cap = [0u128, 1_000, 1_000_000, 1_000_000_000_000][rnd(s, 0, 3) as usize];
    c.min_liquidation_abs = if rnd(s, 0, 2) == 0 {
        c.liquidation_fee_cap / rnd(s, 1, 50) as u128
    } else {
        0
    };
    c.min_nonzero_mm_req = rnd(s, 1, 20) as u128;
    c.min_nonzero_im_req = c.min_nonzero_mm_req + rnd(s, 1, 20) as u128;
    c.max_trading_fee_bps = rnd(s, 0, 100);
    c.rent_max_e9_per_slot = if rnd(s, 0, 3) == 0 { rnd(s, 0, 50) } else { 0 };
    c
}

#[test]
fn config_envelope_accept_set_is_downward_closed_in_loss_inputs() {
    // For 80 random accepted configs: lowering the fee, the move cap, the funding rate,
    // the rent, or the fee floor (all else equal) can never turn accept into reject; and
    // raising MM (IM kept at 100%) can never turn accept into reject.
    let mut seed = 0xC0FF_EE00_u64;
    let mut checked = 0;
    let mut tries = 0;
    while checked < 80 && tries < 100_000 {
        tries += 1;
        let c = random_cfg(&mut seed);
        if !ok(&c) {
            continue;
        }
        checked += 1;
        for fee in [
            0,
            c.liquidation_fee_bps / 2,
            c.liquidation_fee_bps.saturating_sub(1),
        ] {
            assert!(
                ok(&with!(c, liquidation_fee_bps = fee)),
                "lower fee {fee} rejected: {c:?}"
            );
        }
        for mv in [
            1,
            c.max_price_move_bps_per_slot / 2 + 1,
            c.max_price_move_bps_per_slot - 1 + u64::from(c.max_price_move_bps_per_slot == 1),
        ] {
            assert!(
                ok(&with!(c, max_price_move_bps_per_slot = mv)),
                "lower move {mv} rejected: {c:?}"
            );
        }
        assert!(
            ok(&with!(c, max_abs_funding_e9_per_slot = 0u64)),
            "zero funding rejected: {c:?}"
        );
        assert!(
            ok(&with!(c, rent_max_e9_per_slot = 0u64)),
            "zero rent rejected: {c:?}"
        );
        assert!(
            ok(&with!(c, min_liquidation_abs = 0u64)),
            "zero fee floor rejected: {c:?}"
        );
        let mut m = c;
        m.initial_margin_bps = 10_000;
        for mm in [
            c.maintenance_margin_bps + 1,
            c.maintenance_margin_bps * 2,
            10_000,
        ] {
            m.maintenance_margin_bps = mm.min(10_000);
            assert!(ok(&m), "higher MM {mm} rejected: {c:?}");
        }
    }
    assert_eq!(checked, 80, "generator produced too few accepted configs");
}

// ---------------------------------------------------------------- scenario on accepted configs

fn account(seed: u8) -> PortfolioAccountV16Account {
    let h = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        [1; 32], [seed; 32], [3; 32],
    ));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(h).unwrap();
    a
}

fn refresh(
    g: &mut MarketGroupV16ViewMut<'_, u64>,
    acct: &mut PortfolioAccountV16Account,
    slot: u64,
) -> Result<(), V16Error> {
    let mut v = PortfolioV16ViewMut::new(acct);
    for _ in 0..16 {
        match g.full_account_refresh_not_atomic(&mut v) {
            Err(V16Error::BStale) => {
                let price = g.markets[0].engine.asset.effective_price.get();
                g.permissionless_crank_not_atomic(
                    &mut v,
                    PermissionlessCrankRequestV16 {
                        now_slot: slot,
                        asset_index: 0,
                        effective_price: price,
                        funding_rate_e9: 0,
                        action: PermissionlessCrankActionV16::Refresh,
                    },
                )?;
            }
            r => return r.map(|_| ()),
        }
    }
    Err(V16Error::BStale)
}

#[derive(Default, Debug)]
struct Tally {
    accepted: usize,
    traded: usize,
    trade_refused: usize,
    moved: usize,
    went_negative: usize,
    liq_needed: usize,
    liq_ok: usize,
    liq_impossible: usize,
    notes: Vec<String>,
}

/// One accepted config: open a position at (about) the initial margin, move the price the
/// maximum the config permits in one accrual, and check what happened to the loser.
fn scenario(cfg: V16Config, seed: &mut u64, t: &mut Tally) {
    let p0: u64 = [1_000u64, 10_000, 100_000, 1_000_000][rnd(seed, 0, 3) as usize];
    let size_units = rnd(seed, 1, 10_000) as i128;
    let size_q = size_units * POS_SCALE as i128;
    let notional = size_units as u128 * p0 as u128; // size * price / POS_SCALE
    let trader_is_long = rnd(seed, 0, 1) == 0;
    let fee_bps = rnd(seed, 0, cfg.max_trading_fee_bps);
    let im = cfg.initial_margin_bps as u128;
    let need = core::cmp::max(cfg.min_nonzero_im_req, (notional * im + 9_999) / 10_000);
    let trade_fee = (notional * fee_bps as u128 + 9_999) / 10_000;
    let trader_cap = need + trade_fee + 2;
    const BIG: u128 = 1_000_000_000_000_000;

    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0)
        .expect("accepted config must build a market group");
    let mut markets = vec![Market::new(0u64, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, p0, 1)
        .expect("accepted config must activate an asset");
    let (mut trader, mut other) = (account(10), account(11));
    let mut g = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    {
        let mut tv = PortfolioV16ViewMut::new(&mut trader);
        let mut ov = PortfolioV16ViewMut::new(&mut other);
        g.deposit_not_atomic(&mut tv, trader_cap).unwrap();
        g.deposit_not_atomic(&mut ov, BIG).unwrap();
        let r = if trader_is_long {
            g.execute_trade_with_fee_loss_stale_scoped_not_atomic(
                &mut tv,
                &mut ov,
                TradeRequestV16 {
                    asset_index: 0,
                    size_q,
                    exec_price: p0,
                    fee_bps,
                },
                true,
            )
        } else {
            g.execute_trade_with_fee_loss_stale_scoped_not_atomic(
                &mut ov,
                &mut tv,
                TradeRequestV16 {
                    asset_index: 0,
                    size_q,
                    exec_price: p0,
                    fee_bps,
                },
                true,
            )
        };
        match r {
            Ok(_) => t.traded += 1,
            Err(e) => {
                t.trade_refused += 1;
                if t.notes.len() < 6 {
                    t.notes.push(format!(
                        "trade refused {e:?} at need={need} cap={trader_cap} cfg={cfg:?}"
                    ));
                }
                return;
            }
        }
    }
    // worst one-step adverse move the config allows
    let dt = cfg.max_accrual_dt_slots;
    // worst-case funding too: the loser pays at the configured ceiling
    let funding: i128 = if trader_is_long {
        cfg.max_abs_funding_e9_per_slot as i128
    } else {
        -(cfg.max_abs_funding_e9_per_slot as i128)
    };
    let slot0 = g.markets[0].engine.asset.slot_last.get();
    let delta = (p0 as u128 * cfg.max_price_move_bps_per_slot as u128 * dt as u128 / 10_000) as u64;
    let newp = if trader_is_long {
        p0 - delta
    } else {
        p0 + delta
    };
    g.accrue_asset_to_not_atomic(0, slot0 + dt, newp, funding, true)
        .expect("the maximum move the config permits must accrue");
    t.moved += 1;
    let slot1 = slot0 + dt;
    refresh(&mut g, &mut trader, slot1).expect("refresh after max move");
    let eq = trader.capital.get() as i128 + trader.pnl.get();
    if eq < 0 {
        t.went_negative += 1;
        t.notes.push(format!("SOLVENT->INSOLVENT eq={eq} cfg={cfg:?} size={size_units} long={trader_is_long} fee_bps={fee_bps}"));
    }
    // clearly under MM -> liquidation must be possible
    let n_after = size_units as u128 * newp as u128;
    let mm_prop = n_after * cfg.maintenance_margin_bps as u128 / 10_000;
    let mm_after = core::cmp::max(mm_prop.saturating_sub(2), cfg.min_nonzero_mm_req);
    if eq >= 0 && (eq as u128) < mm_after {
        t.liq_needed += 1;
        let r = g.liquidate_account_not_atomic(
            &mut PortfolioV16ViewMut::new(&mut trader),
            LiquidationRequestV16 { asset_index: 0 },
        );
        match r {
            Ok(_) => t.liq_ok += 1,
            Err(e) => {
                t.liq_impossible += 1;
                t.notes.push(format!(
                    "LIQ IMPOSSIBLE {e:?} eq={eq} mm_after={mm_after} cfg={cfg:?}"
                ));
            }
        }
    }
    assert!(
        g.header.vault.get() >= g.header.c_tot.get() + g.header.insurance.get(),
        "V < C + I cfg={cfg:?}"
    );
    g.validate_shape().expect("shape after scenario");
}

#[test]
fn config_envelope_random_accepted_configs_600_survive_max_move() {
    let mut seed = 0x5EED_0001_u64;
    let mut t = Tally::default();
    let mut tries = 0usize;
    let mut rejected = 0usize;
    while t.accepted < 600 && tries < 2_000_000 {
        tries += 1;
        let c = random_cfg(&mut seed);
        if !ok(&c) {
            rejected += 1;
            continue;
        }
        t.accepted += 1;
        scenario(c, &mut seed, &mut t);
    }
    println!("tries={tries} rejected={rejected} {t:#?}");
    assert_eq!(t.accepted, 600);
    assert!(t.liq_needed > 0 && t.liq_ok == t.liq_needed, "{t:#?}");
    assert!(
        t.traded >= 300,
        "most accepted configs should be tradable: {t:#?}"
    );
    assert_eq!(t.went_negative, 0, "{:#?}", t.notes);
    assert_eq!(t.liq_impossible, 0, "{:#?}", t.notes);
}

#[test]
fn config_envelope_random_accepted_configs_with_fees_survive_max_move() {
    // same, but force a real liquidation fee (cap huge) so fee paths run, 500 more accepted configs
    let mut seed = 0x5EED_0002_u64;
    let mut t = Tally::default();
    let mut tries = 0usize;
    while t.accepted < 500 && tries < 2_000_000 {
        tries += 1;
        let mut c = random_cfg(&mut seed);
        c.liquidation_fee_cap = u64::MAX as u128;
        c.min_liquidation_abs = 0;
        c.liquidation_fee_bps = rnd(&mut seed, 1, 300);
        if !ok(&c) {
            continue;
        }
        t.accepted += 1;
        scenario(c, &mut seed, &mut t);
    }
    println!("tries={tries} {t:#?}");
    assert_eq!(t.accepted, 500);
    assert!(t.traded > 0 && t.liq_needed > 0, "{t:#?}");
    assert_eq!(t.went_negative, 0, "{:#?}", t.notes);
    assert_eq!(t.liq_impossible, 0, "{:#?}", t.notes);
}

/// Push a random accepted config to its edge: the largest fee, then the largest move cap,
/// the envelope still accepts. Then run the max-move scenario on it.
fn push_to_edge(mut c: V16Config, seed: &mut u64) -> V16Config {
    c.liquidation_fee_cap = u64::MAX as u128;
    c.min_liquidation_abs = 0;
    let mut best_fee = 0;
    for f in (0..=600u64) {
        if ok(&with!(c, liquidation_fee_bps = f)) {
            best_fee = f;
        } else if f > best_fee + 40 {
            break;
        }
    }
    c.liquidation_fee_bps = if rnd(seed, 0, 1) == 0 {
        best_fee
    } else {
        best_fee / 2
    };
    let mut best_mv = c.max_price_move_bps_per_slot;
    for mv in best_mv..=1_000 {
        if ok(&with!(c, max_price_move_bps_per_slot = mv)) {
            best_mv = mv;
        } else {
            break;
        }
    }
    c.max_price_move_bps_per_slot = best_mv;
    c
}

#[test]
fn config_envelope_random_edge_configs_500_survive_max_move() {
    let mut seed = 0x5EED_0003_u64;
    let mut t = Tally::default();
    let mut tries = 0usize;
    while t.accepted < 500 && tries < 100_000 {
        tries += 1;
        let c = random_cfg(&mut seed);
        if !ok(&c) {
            continue;
        }
        let e = push_to_edge(c, &mut seed);
        // pushing fee alone could leave the config accepted only at the edge; must still be accepted
        assert!(ok(&e), "edge config lost acceptance: {e:?}");
        t.accepted += 1;
        // several scenarios per edge config (sizes and prices differ)
        for _ in 0..4 {
            scenario(e, &mut seed, &mut t);
        }
    }
    println!("tries={tries} {t:#?}");
    assert_eq!(t.accepted, 500);
    assert_eq!(t.went_negative, 0, "{:#?}", t.notes);
    assert_eq!(t.liq_impossible, 0, "{:#?}", t.notes);
}
