#![cfg(kani)]
//! v2.2 final Kani run, engine obligations (design: percolator-ops/ledger
//! kani-v22-final-run-design-2026-10-09.md, rev2, rev2.1). PROOF-ONLY FILE.
//!
//! House rules (design section 2.0): the real production function is called (private ones through
//! the forwarding shims in `src/kani_v22_shims.rs`); fixtures are built with the real constructors
//! and then constrained by the real validators (`kani::assume(..validate_shape().is_ok())`) with a
//! probe cover right after the assumption; frames compare whole Pod accounts with `==`; every
//! claimed branch has a cover. Each harness names its obligation id and its mutant ids
//! (rev2 R4.5 / rev2.1).
#![allow(clippy::too_many_arguments, dead_code, unused_imports, unused_mut)]

use percolator::v16::{
    adjust_slot_provider_principal, kani_available_backing_num_for_source_credit_state,
    kani_expected_source_credit_rate_num_for_state, repay_pnl_postconditions_hold,
    AssetStateV16, AssetStateV16Account, BackingBucketStatusV16, BackingBucketV16,
    BackingBucketV16Account, EngineAssetSlotV16Account, InsuranceCreditReservationV16,
    InsuranceCreditReservationV16Account, KfDriftSideV16, LiquidationRequestV16, Market,
    MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, PermissionlessCrankActionV16,
    PermissionlessCrankRequestV16, PermissionlessRecoveryReasonV16, PortfolioAccountV16Account,
    PortfolioLegV16, PortfolioLegV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, SideV16, SourceCreditStateV16, SourceCreditStateV16Account,
    TradeRequestV16, V16Config, V16Error, V16PodI128, V16PodU128, V16PodU64, V16Result,
    S10_MAX_MOVES_PER_INSTRUCTION, S10_MIN_MOVE_ATOMS, S10_PROTECT_FULL_PROVIDER_PRINCIPAL,
};
use percolator::{ADL_ONE, BOUND_SCALE, CREDIT_RATE_SCALE, MAX_VAULT_TVL, MIN_A_SIDE, POS_SCALE, SOCIAL_WEIGHT_SCALE};

type View<'a> = MarketGroupV16ViewMut<'a, u64>;

// =====================================================================================
// Shared fixtures
// =====================================================================================

fn ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    ([1; 32], [2; 32], [3; 32])
}

fn empty_account_fixture(market_id: [u8; 32], account_tag: u8) -> PortfolioAccountV16Account {
    let mut account_id = [0u8; 32];
    account_id[0] = account_tag;
    let mut owner = [0u8; 32];
    owner[0] = account_tag;
    PortfolioAccountV16Account::try_empty(ProvenanceHeaderV16Account::from_runtime(
        &ProvenanceHeaderV16::new(market_id, account_id, owner),
    ))
    .unwrap()
}

/// One active asset built with the real constructor (engine PR #147 lesson).
fn one_market_only_fixture() -> (MarketGroupV16HeaderAccount, [Market<u64>; 1]) {
    let (market_id, _, _) = ids();
    let cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(market_id, cfg, 1, 0).unwrap();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    {
        let mut view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        view.activate_empty_market_not_atomic(0, 100, 1).unwrap();
    }
    (header, markets)
}

/// S10 fixture slot clock: buckets' expiry slots are `NOW - 128 + u8`, so both lapsed and
/// unexpired buckets are reachable.
const NOW: u64 = 200;

/// One backing domain of the S10 fixture, in whole atoms (A-W: `u16` atoms, a sub-atom claim
/// remainder, `u8` expiry offset). Every amount the engine stores is `atoms * BOUND_SCALE`.
#[derive(Clone, Copy)]
struct Dom {
    status: u8,
    fr: u16,
    valid: u16,
    cons: u16,
    imp: u16,
    exp: u8,
    claims: u16,
    rem: u16,
    exact_full: bool,
    spent_extra: u8,
    pp: u16,
    ins: u8,
}

fn any_dom(with_insurance: bool) -> Dom {
    Dom {
        status: kani::any(),
        fr: kani::any(),
        valid: kani::any(),
        cons: kani::any(),
        imp: kani::any(),
        exp: kani::any(),
        claims: kani::any(),
        rem: kani::any(),
        exact_full: kani::any(),
        spent_extra: kani::any(),
        pp: kani::any(),
        ins: if with_insurance { kani::any() } else { 0 },
    }
}

fn status_of(s: u8) -> BackingBucketStatusV16 {
    match s % 4 {
        0 => BackingBucketStatusV16::Empty,
        1 => BackingBucketStatusV16::Fresh,
        2 => BackingBucketStatusV16::Expired,
        _ => BackingBucketStatusV16::Impaired,
    }
}

fn n(atoms: u16) -> u128 {
    atoms as u128 * BOUND_SCALE
}

fn dom_bucket(market_id: u64, d: Dom) -> BackingBucketV16 {
    let status = status_of(d.status);
    BackingBucketV16 {
        market_id,
        fresh_unliened_backing_num: n(d.fr),
        valid_liened_backing_num: n(d.valid),
        consumed_liened_backing_num: n(d.cons),
        impaired_liened_backing_num: n(d.imp),
        utilization_fee_earnings: 0,
        expiry_slot: if status == BackingBucketStatusV16::Empty { 0 } else { NOW - 128 + d.exp as u64 },
        status,
    }
}

fn dom_source(d: Dom) -> SourceCreditStateV16 {
    let bound = n(d.claims) + d.rem as u128;
    let mut s = SourceCreditStateV16 {
        positive_claim_bound_num: bound,
        exact_positive_claim_num: if d.exact_full { n(d.claims) } else { 0 },
        fresh_reserved_backing_num: n(d.fr) + n(d.valid),
        spent_backing_num: n(d.cons) + d.spent_extra as u128 * BOUND_SCALE,
        provider_receivable_num: n(d.cons),
        valid_liened_backing_num: n(d.valid),
        impaired_liened_backing_num: n(d.imp),
        insurance_credit_reserved_num: d.ins as u128 * BOUND_SCALE,
        valid_liened_insurance_num: 0,
        impaired_liened_insurance_num: 0,
        credit_rate_num: 0,
        credit_epoch: 0,
    };
    // the stored rate is whatever the (real or, in the `_m` twins, modelled) rate function gives:
    // a state whose rate cannot be computed is filtered out by the shape assumption below
    s.credit_rate_num = kani_expected_source_credit_rate_num_for_state(s).unwrap_or(u128::MAX);
    s
}

/// The S10 world: one Live asset at `NOW`, both domains poked from `l` / `s`, mirrors set, header
/// aggregates recomputed by the real `refresh_header_aggregate_totals_for_test`, then A-SHAPE:
/// `kani::assume(validate_shape().is_ok())` (under cfg(kani) this includes the full audit scan:
/// A-AUDIT). Returns `None` when the fixture is not shape-valid (callers `assume` it away).
fn s10_world(l: Dom, s: Dom) -> Option<(MarketGroupV16HeaderAccount, [Market<u64>; 1])> {
    let (mut header, mut markets) = one_market_only_fixture();
    header.current_slot = V16PodU64::new(NOW);
    header.slot_last = V16PodU64::new(NOW);
    markets[0].engine.asset.slot_last = V16PodU64::new(NOW);
    let market_id = markets[0].engine.asset.market_id.get();
    let ins_atoms = l.ins as u128 + s.ins as u128;
    {
        let e = &mut markets[0].engine;
        e.backing_long = BackingBucketV16Account::from_runtime(&dom_bucket(market_id, l));
        e.backing_short = BackingBucketV16Account::from_runtime(&dom_bucket(market_id, s));
        e.source_credit_long = SourceCreditStateV16Account::from_runtime(&dom_source(l));
        e.source_credit_short = SourceCreditStateV16Account::from_runtime(&dom_source(s));
        e.insurance_reservation_long =
            InsuranceCreditReservationV16Account::from_runtime(&InsuranceCreditReservationV16 {
                insurance_credit_reserved_num: l.ins as u128 * BOUND_SCALE,
                ..InsuranceCreditReservationV16::EMPTY
            });
        e.insurance_reservation_short =
            InsuranceCreditReservationV16Account::from_runtime(&InsuranceCreditReservationV16 {
                insurance_credit_reserved_num: s.ins as u128 * BOUND_SCALE,
                ..InsuranceCreditReservationV16::EMPTY
            });
        e.provider_principal_long = V16PodU128::new(n(l.pp));
        e.provider_principal_short = V16PodU128::new(n(s.pp));
    }
    header.vault = V16PodU128::new(MAX_VAULT_TVL);
    header.insurance = V16PodU128::new(ins_atoms);
    {
        let mut v = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        if v.refresh_header_aggregate_totals_for_test().is_err() {
            return None;
        }
    }
    let claims_num = header.source_claim_bound_total_num.get();
    header.pnl_pos_bound_tot_num = V16PodU128::new(claims_num);
    header.pnl_pos_bound_tot = V16PodU128::new(View::kani_v22_amount_from_bound_num(claims_num).ok()?);
    header.pnl_pos_tot = V16PodU128::new(0);
    header.pnl_matured_pos_tot = V16PodU128::new(0);
    let ok = MarketGroupV16ViewMut::new(&mut header, &mut markets).validate_shape().is_ok();
    if ok {
        Some((header, markets))
    } else {
        None
    }
}

fn bucket(m: &[Market<u64>; 1], d: usize) -> BackingBucketV16 {
    if d == 0 { m[0].engine.backing_long.try_to_runtime().unwrap() } else { m[0].engine.backing_short.try_to_runtime().unwrap() }
}
fn source(m: &[Market<u64>; 1], d: usize) -> SourceCreditStateV16 {
    if d == 0 { m[0].engine.source_credit_long.try_to_runtime().unwrap() } else { m[0].engine.source_credit_short.try_to_runtime().unwrap() }
}
fn pp_of(m: &[Market<u64>; 1], d: usize) -> u128 {
    if d == 0 { m[0].engine.provider_principal_long.get() } else { m[0].engine.provider_principal_short.get() }
}
fn av(s: SourceCreditStateV16) -> u128 {
    kani_available_backing_num_for_source_credit_state(s).unwrap()
}

/// Independent spec of the S10 move (E-S10-6): the first direction, in the code's order
/// (long->short, then short->long), whose guards hold and whose rounded amount reaches the dust
/// floor fires with exactly that amount (rule A: loser cash = fresh above the principal mirror).
/// Returns `(src, dst, amount_num)`, or `None` when nothing may move.
fn s10_spec(m: &[Market<u64>; 1], budget: u8) -> Option<(usize, usize, u128)> {
    if budget == 0 {
        return None;
    }
    for (src, dst) in [(0usize, 1usize), (1, 0)] {
        let (bs, ss, bd, sd) = (bucket(m, src), source(m, src), bucket(m, dst), source(m, dst));
        let lc = bs.fresh_unliened_backing_num.saturating_sub(pp_of(m, src));
        if lc == 0 || bs.status != BackingBucketStatusV16::Fresh || bs.expiry_slot <= NOW || bs.fresh_unliened_backing_num == 0 {
            continue;
        }
        let accepts = match bd.status {
            BackingBucketStatusV16::Empty | BackingBucketStatusV16::Expired => true,
            BackingBucketStatusV16::Fresh => bd.expiry_slot > NOW,
            BackingBucketStatusV16::Impaired => false,
        };
        if !accepts {
            continue;
        }
        let excess = lc.min(av(ss).saturating_sub(ss.positive_claim_bound_num));
        let shortfall = sd.positive_claim_bound_num.saturating_sub(av(sd));
        let x = excess.min(shortfall) / BOUND_SCALE * BOUND_SCALE;
        if x < S10_MIN_MOVE_ATOMS * BOUND_SCALE {
            continue;
        }
        return Some((src, dst, x));
    }
    None
}

/// The S10 body shared by the real-rate harness and its rate-model twin (E-S10-1, 2, 5, 6, 7, 9,
/// 10). `with_insurance` selects the `_ins` twin (insurance credit reserved in the domains).
fn s10_move_body(with_insurance: bool) {
    let l = any_dom(with_insurance);
    let s = any_dom(with_insurance);
    let world = s10_world(l, s);
    kani::assume(world.is_some());
    let (mut header, mut markets) = world.unwrap();
    kani::cover!(true, "S10 fixture is shape-valid");
    // A-RULE: S10_PROTECT_FULL_PROVIDER_PRINCIPAL is a const (rule A); the spec below assumes rule A,
    // so flipping the const (mutant S10-E1) turns the exactness assertion red.
    let budget0: u8 = kani::any();
    kani::assume(budget0 <= S10_MAX_MOVES_PER_INSTRUCTION);
    let mut budget = budget0;
    let (h0, m0) = (header, markets);
    let spec = s10_spec(&m0, budget0);
    let r = {
        let mut v = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        v.rebalance_unclaimed_backing_for_test_not_atomic(0, &mut budget)
    };
    // E-S10-7 totality: every shape-valid state returns Ok (no new fail-closed path in a settle)
    assert!(r.is_ok(), "S10 move is total on shape-valid states");

    let (b0, b1) = ((bucket(&m0, 0), bucket(&m0, 1)), (bucket(&markets, 0), bucket(&markets, 1)));
    let pre = [b0.0, b0.1];
    let post = [b1.0, b1.1];
    let spre = [source(&m0, 0), source(&m0, 1)];
    let spost = [source(&markets, 0), source(&markets, 1)];

    // E-S10-6 exactness (independent spec) and E-S10-2 bound; E-S10-5 budget
    match spec {
        Some((src, dst, x)) => {
            assert_eq!(post[src].fresh_unliened_backing_num, pre[src].fresh_unliened_backing_num - x);
            assert_eq!(post[dst].fresh_unliened_backing_num, pre[dst].fresh_unliened_backing_num + x);
            assert!(x % BOUND_SCALE == 0 && x >= S10_MIN_MOVE_ATOMS * BOUND_SCALE);
            assert!(post[src].fresh_unliened_backing_num >= pre[src].fresh_unliened_backing_num.min(pp_of(&m0, src)));
            assert!(av(spost[dst]) <= spost[dst].positive_claim_bound_num, "destination never over-covered");
            assert!(av(spost[src]) >= spost[src].positive_claim_bound_num, "source claimants keep coverage");
            assert_eq!(budget, budget0 - 1);
            assert_eq!(header.risk_epoch.get(), h0.risk_epoch.get() + 2);
            assert_eq!(spost[0].credit_epoch, spre[0].credit_epoch + 1);
            assert_eq!(spost[1].credit_epoch, spre[1].credit_epoch + 1);
            // E-S10-9 destination expiry rule
            if pre[dst].status == BackingBucketStatusV16::Fresh && pre[dst].expiry_slot > NOW {
                assert_eq!(post[dst].expiry_slot, pre[dst].expiry_slot);
            } else {
                assert!(post[dst].expiry_slot > NOW);
            }
            assert_eq!(post[dst].status, BackingBucketStatusV16::Fresh);
            // E-S10-9 source status transition (prepare_counterparty_backing_withdraw_delta)
            if post[src].fresh_unliened_backing_num == 0 && post[src].valid_liened_backing_num == 0 {
                if post[src].impaired_liened_backing_num != 0 {
                    assert_eq!(post[src].status, BackingBucketStatusV16::Impaired);
                } else if post[src].consumed_liened_backing_num != 0 {
                    assert_eq!(post[src].status, BackingBucketStatusV16::Expired);
                } else {
                    assert_eq!(post[src].status, BackingBucketStatusV16::Empty);
                }
            } else {
                assert_eq!(post[src].status, pre[src].status);
            }
            // receivable refill on the destination only
            let refill = x.min(spre[dst].provider_receivable_num);
            assert_eq!(spost[dst].provider_receivable_num, spre[dst].provider_receivable_num - refill);
            assert_eq!(post[dst].consumed_liened_backing_num, pre[dst].consumed_liened_backing_num - refill);
            assert_eq!(spost[src].provider_receivable_num, spre[src].provider_receivable_num);
            // E-S10-9 ledger
            let v = MarketGroupV16ViewMut::new(&mut header, &mut markets);
            assert!(v.kani_validate_source_domain_ledger_current(0).is_ok());
            assert!(v.kani_validate_source_domain_ledger_current(1).is_ok());
        }
        None => {
            assert!(header == h0 && markets == m0, "nothing moves => byte-identical");
            assert_eq!(budget, budget0);
        }
    }
    // E-S10-1 conservation (also trivially true when nothing moved)
    assert_eq!(
        pre[0].fresh_unliened_backing_num + pre[1].fresh_unliened_backing_num,
        post[0].fresh_unliened_backing_num + post[1].fresh_unliened_backing_num
    );
    assert_eq!(
        spre[0].fresh_reserved_backing_num + spre[1].fresh_reserved_backing_num,
        spost[0].fresh_reserved_backing_num + spost[1].fresh_reserved_backing_num
    );
    for d in 0..2 {
        assert_eq!(post[d].valid_liened_backing_num, pre[d].valid_liened_backing_num);
        assert_eq!(post[d].impaired_liened_backing_num, pre[d].impaired_liened_backing_num);
        assert_eq!(spost[d].spent_backing_num, spre[d].spent_backing_num);
        assert_eq!(spost[d].positive_claim_bound_num, spre[d].positive_claim_bound_num);
        assert_eq!(spost[d].exact_positive_claim_num, spre[d].exact_positive_claim_num);
        assert_eq!(spost[d].insurance_credit_reserved_num, spre[d].insurance_credit_reserved_num);
        assert_eq!(spost[d].valid_liened_insurance_num, spre[d].valid_liened_insurance_num);
        assert_eq!(spost[d].impaired_liened_insurance_num, spre[d].impaired_liened_insurance_num);
        assert!(spost[d].provider_receivable_num <= spre[d].provider_receivable_num);
        assert_eq!(
            pre[d].consumed_liened_backing_num - post[d].consumed_liened_backing_num,
            spre[d].provider_receivable_num - spost[d].provider_receivable_num
        );
        assert_eq!(pp_of(&markets, d), pp_of(&m0, d), "provider mirror untouched (rule A)");
    }
    let (e0, e1) = (&m0[0].engine, &markets[0].engine);
    assert!(e1.asset == e0.asset && e1.kf_drift_long == e0.kf_drift_long && e1.kf_drift_short == e0.kf_drift_short);
    assert!(e1.kf_pending_credit_long == e0.kf_pending_credit_long && e1.kf_pending_credit_short == e0.kf_pending_credit_short);
    assert!(e1.insurance_reservation_long == e0.insurance_reservation_long && e1.insurance_reservation_short == e0.insurance_reservation_short);
    assert!(e1.insurance_domain_budget_long == e0.insurance_domain_budget_long && e1.insurance_domain_budget_short == e0.insurance_domain_budget_short);
    assert!(e1.pending_domain_loss_barrier_long == e0.pending_domain_loss_barrier_long && e1.pending_domain_loss_barrier_short == e0.pending_domain_loss_barrier_short);
    assert!(header.vault == h0.vault && header.insurance == h0.insurance && header.c_tot == h0.c_tot);
    // header frame: only risk_epoch and the backing aggregates may differ
    let mut hx = header;
    hx.risk_epoch = h0.risk_epoch;
    hx.backing_provider_earnings_total = h0.backing_provider_earnings_total;
    hx.source_fresh_backing_total_num = h0.source_fresh_backing_total_num;
    assert!(hx == h0, "header frame");

    // E-S10-10 covers
    let fired = |s: usize| matches!(spec, Some((src, _, _)) if src == s);
    kani::cover!(fired(0), "fires long -> short");
    kani::cover!(fired(1), "fires short -> long");
    kani::cover!(
        spec.map_or(false, |(src, _, _)| pp_of(&m0, src) > 0 && pre[src].fresh_unliened_backing_num > pp_of(&m0, src)),
        "fires with provider principal and loser cash present"
    );
    kani::cover!(
        spec.is_none() && (0..2).any(|d| pre[d].fresh_unliened_backing_num > 0 && pre[d].fresh_unliened_backing_num <= pp_of(&m0, d)),
        "skipped by the provider share"
    );
    kani::cover!(budget0 == 0 && s10_spec(&m0, 1).is_some(), "skipped by the budget");
    kani::cover!(
        budget0 > 0 && spec.is_none() && {
            // a would-be move below the dust floor
            let bs = pre[0];
            let x = bs.fresh_unliened_backing_num.saturating_sub(pp_of(&m0, 0))
                .min(av(spre[0]).saturating_sub(spre[0].positive_claim_bound_num))
                .min(spre[1].positive_claim_bound_num.saturating_sub(av(spre[1])));
            bs.status == BackingBucketStatusV16::Fresh && bs.expiry_slot > NOW && x > 0 && x < S10_MIN_MOVE_ATOMS * BOUND_SCALE
        },
        "skipped by the dust floor"
    );
    kani::cover!(
        budget0 > 0 && pre[0].status == BackingBucketStatusV16::Fresh && pre[0].expiry_slot <= NOW
            && pre[0].fresh_unliened_backing_num > pp_of(&m0, 0)
            && spre[1].positive_claim_bound_num > av(spre[1]) + S10_MIN_MOVE_ATOMS * BOUND_SCALE,
        "skipped: source bucket lapsed (Fresh, expiry <= now)"
    );
    kani::cover!(
        budget0 > 0 && pre[1].status == BackingBucketStatusV16::Impaired
            && pre[0].status == BackingBucketStatusV16::Fresh && pre[0].expiry_slot > NOW
            && pre[0].fresh_unliened_backing_num > pp_of(&m0, 0) + S10_MIN_MOVE_ATOMS * BOUND_SCALE
            && spre[1].positive_claim_bound_num > av(spre[1]) + S10_MIN_MOVE_ATOMS * BOUND_SCALE,
        "skipped: destination Impaired"
    );
    kani::cover!(spec.map_or(false, |(_, dst, _)| pre[dst].status == BackingBucketStatusV16::Empty), "destination Empty reopened");
    kani::cover!(spec.map_or(false, |(_, dst, _)| pre[dst].status == BackingBucketStatusV16::Expired), "destination Expired reopened");
    kani::cover!(spec.map_or(false, |(_, dst, _)| spre[dst].provider_receivable_num > 0), "receivable refill > 0");
    kani::cover!(
        spec.map_or(false, |(src, dst, x)| x < (pre[src].fresh_unliened_backing_num.saturating_sub(pp_of(&m0, src)))
            .min(av(spre[src]).saturating_sub(spre[src].positive_claim_bound_num))
            .min(spre[dst].positive_claim_bound_num.saturating_sub(av(spre[dst])))),
        "rounding to whole atoms binds"
    );
}

/// Deterministic checked rate model (design rev2 R2.4, A-RATE): keeps the real shape and
/// `available` error checks, so it errs exactly where the real function errs before its U256
/// step, then returns a fixed function of (claims, available) capped at the scale.
fn s10_rate_model(state: SourceCreditStateV16) -> V16Result<u128> {
    View::kani_v22_source_credit_shape_static(state)?;
    let available = kani_available_backing_num_for_source_credit_state(state)?;
    if state.positive_claim_bound_num == 0 || available >= state.positive_claim_bound_num {
        Ok(CREDIT_RATE_SCALE)
    } else {
        Ok(0)
    }
}

/// E-S10-1, 2, 5, 6, 7, 9, 10 on the REAL U256 rate. Class L (heavy lane), fallback: the `_m` twin.
/// Assumptions A-SHAPE (+A-AUDIT), A-W, A-LIVE, A-RULE. Mutants S10-M1, M2, M3, M4 (may be
/// equivalent), E1, F1, F2, B1, X1, X2.
#[kani::proof]
#[kani::solver(cadical)]
fn proof_v22_s10_move_conserves_bounds_exact_total() {
    s10_move_body(false);
}

/// Rate-model twin of the above (evidence label "model-rate" unless gate G-RATE passes).
#[kani::proof]
#[kani::stub(percolator::v16::V16Core::expected_source_credit_rate_num_for_state, s10_rate_model)]
#[kani::solver(cadical)]
fn proof_v22_s10_move_conserves_bounds_exact_total_m() {
    s10_move_body(false);
}

/// `_ins` twin: insurance credit reserved in the domains (`available` includes it; claimants keep
/// coverage). Mutant: `excess` computed from fresh backing only. Model rate.
#[kani::proof]
#[kani::stub(percolator::v16::V16Core::expected_source_credit_rate_num_for_state, s10_rate_model)]
#[kani::solver(cadical)]
fn proof_v22_s10_move_conserves_bounds_exact_total_ins_m() {
    s10_move_body(true);
}

/// E-S10-8 idempotence: after an Ok call with budget >= 1, a second call (budget 2) moves nothing.
/// Model rate. Mutant S10-O1.
#[kani::proof]
#[kani::stub(percolator::v16::V16Core::expected_source_credit_rate_num_for_state, s10_rate_model)]
#[kani::solver(cadical)]
fn proof_v22_s10_move_is_idempotent_m() {
    let world = s10_world(any_dom(false), any_dom(false));
    kani::assume(world.is_some());
    let (mut header, mut markets) = world.unwrap();
    kani::cover!(true, "S10 fixture is shape-valid");
    let b1: u8 = kani::any();
    kani::assume(b1 >= 1 && b1 <= S10_MAX_MOVES_PER_INSTRUCTION);
    let fired_first = s10_spec(&markets, b1).is_some();
    let mut budget = b1;
    let r1 = MarketGroupV16ViewMut::new(&mut header, &mut markets).rebalance_unclaimed_backing_for_test_not_atomic(0, &mut budget);
    kani::assume(r1.is_ok());
    let (h1, m1) = (header, markets);
    let mut budget2 = S10_MAX_MOVES_PER_INSTRUCTION;
    let r2 = MarketGroupV16ViewMut::new(&mut header, &mut markets).rebalance_unclaimed_backing_for_test_not_atomic(0, &mut budget2);
    assert!(r2.is_ok());
    assert!(header == h1 && markets == m1, "second call moves nothing");
    assert_eq!(budget2, S10_MAX_MOVES_PER_INSTRUCTION);
    kani::cover!(fired_first, "first call fired");
    kani::cover!(!fired_first, "first call idle");
}

/// E-S10-4 idle identity (fast path): no loser cash in either domain => Ok, byte-identical, for
/// any budget. Real rate (the fast path returns before any rate work). Kills S10-M3 together
/// with the exactness assertion of the main harness.
#[kani::proof]
#[kani::solver(cadical)]
fn proof_v22_s10_idle_fast_path_is_identity() {
    let (l, s) = (any_dom(false), any_dom(false));
    kani::assume(l.fr <= l.pp && s.fr <= s.pp);
    let world = s10_world(l, s);
    kani::assume(world.is_some());
    let (mut header, mut markets) = world.unwrap();
    kani::cover!(true, "S10 fixture is shape-valid");
    let budget0: u8 = kani::any();
    kani::assume(budget0 <= S10_MAX_MOVES_PER_INSTRUCTION);
    let mut budget = budget0;
    let (h0, m0) = (header, markets);
    let r = MarketGroupV16ViewMut::new(&mut header, &mut markets).rebalance_unclaimed_backing_for_test_not_atomic(0, &mut budget);
    assert!(r.is_ok());
    assert!(header == h0 && markets == m0);
    assert_eq!(budget, budget0);
    kani::cover!(l.fr > 0, "idle with fresh backing below the principal");
    kani::cover!(l.fr == 0 && s.fr == 0, "idle with both domains empty of fresh");
    kani::cover!(budget0 == 0, "budget 0");
    kani::cover!(budget0 == 2, "budget 2");
}

/// E-S10-3 share formula and the fast-path predicate, full u128 width. Mutants: rule A returns the
/// rule-B formula; saturating_sub -> wrapping.
#[kani::proof]
#[kani::solver(cadical)]
fn proof_v22_s10_share_formula_and_fast_predicate() {
    let p: u128 = kani::any();
    let fresh: u128 = kani::any();
    let b = BackingBucketV16 {
        consumed_liened_backing_num: kani::any(),
        impaired_liened_backing_num: kani::any(),
        valid_liened_backing_num: kani::any(),
        fresh_unliened_backing_num: fresh,
        ..BackingBucketV16::EMPTY
    };
    let a = View::s10_provider_fresh_by_rule_for_test(true, p, b);
    let rb = View::s10_provider_fresh_by_rule_for_test(false, p, b);
    assert_eq!(a, p, "rule A protects the full principal");
    assert_eq!(
        rb,
        p.saturating_sub(b.consumed_liened_backing_num)
            .saturating_sub(b.impaired_liened_backing_num)
            .saturating_sub(b.valid_liened_backing_num)
    );
    if p == 0 {
        assert_eq!(a, 0);
        assert_eq!(rb, 0, "a provider-less bucket is wholly movable");
    }
    assert_eq!(View::s10_provider_fresh_for_test(p, b), a, "compiled rule is A");
    // the fast-path predicate (`:15097-15099`) skips exactly when the slow pass would `continue`
    let fast_skip = fresh == 0 || fresh <= p;
    let slow_skip = fresh.saturating_sub(a) == 0 || fresh == 0;
    assert_eq!(fast_skip, slow_skip);
    kani::cover!(p > 0 && rb == 0, "rule B gives 0 with principal");
    kani::cover!(rb > 0, "rule B gives > 0");
    kani::cover!(fresh > 0 && fresh <= p, "fresh below principal (idle)");
    kani::cover!(fresh > p, "loser cash present");
}

/// E-S10-11 retry hook guard: budget 0 or any of the four counters non-zero => identity; else the
/// hook equals the move (state and result on a clone). Model rate. Mutant S10-H1.
#[kani::proof]
#[kani::stub(percolator::v16::V16Core::expected_source_credit_rate_num_for_state, s10_rate_model)]
#[kani::solver(cadical)]
fn proof_v22_s10_retry_hook_guard_m() {
    let (l, s) = (any_dom(false), any_dom(false));
    let world = s10_world(l, s);
    kani::assume(world.is_some());
    let (mut header, mut markets) = world.unwrap();
    let c: [u8; 4] = kani::any();
    {
        let a = &mut markets[0].engine.asset;
        a.stale_account_count_long = V16PodU64::new(c[0] as u64);
        a.stale_account_count_short = V16PodU64::new(c[1] as u64);
        a.stored_pos_count_long = V16PodU64::new(c[2] as u64);
        a.stored_pos_count_short = V16PodU64::new(c[3] as u64);
    }
    kani::assume(MarketGroupV16ViewMut::new(&mut header, &mut markets).validate_shape().is_ok());
    kani::cover!(true, "hook fixture is shape-valid");
    let b0: u8 = kani::any();
    kani::assume(b0 <= S10_MAX_MOVES_PER_INSTRUCTION);
    let (h0, m0) = (header, markets);
    let (mut hc, mut mc) = (header, markets);
    let mut b = b0;
    let mut bc = b0;
    let r = MarketGroupV16ViewMut::new(&mut header, &mut markets).kani_v22_s10_retry(0, &mut b);
    let any_count = c.iter().any(|x| *x != 0);
    if b0 == 0 || any_count {
        assert!(r.is_ok());
        assert!(header == h0 && markets == m0, "hook skipped => identity");
    } else {
        let rc = MarketGroupV16ViewMut::new(&mut hc, &mut mc).rebalance_unclaimed_backing_for_test_not_atomic(0, &mut bc);
        assert_eq!(r, rc);
        assert!(header == hc && markets == mc && b == bc, "hook == the move");
    }
    kani::cover!(b0 > 0 && c[0] != 0 && s10_spec(&m0, b0).is_some(), "skip: stale long");
    kani::cover!(b0 > 0 && c[1] != 0 && s10_spec(&m0, b0).is_some(), "skip: stale short");
    kani::cover!(b0 > 0 && c[2] != 0 && s10_spec(&m0, b0).is_some(), "skip: stored long");
    kani::cover!(b0 > 0 && c[3] != 0 && s10_spec(&m0, b0).is_some(), "skip: stored short");
    kani::cover!(b0 > 0 && !any_count && s10_spec(&m0, b0).is_some(), "hook fires a move");
}

/// E-S10-12 grant (rev2 R1.1: concrete fixture, symbolic only over action, mode and grant).
/// One asset, no stored position, an empty account; 50,000 atoms of loser cash in the long domain,
/// a 10,000-atom claims shortfall in the short domain. For any action other than Refresh, and any
/// grant, no S10 move happens (both domains byte-identical); outside Live every non-Recover action
/// is `Err(LockActive)` before any mutation; Refresh with grant > 0 moves. Mutants S10-G1 (must
/// go red: the Refresh cover becomes unsatisfiable) and S10-G2 (expected to SURVIVE: 10-09 ruling;
/// if this fixture kills it, report to the reviewer).
#[kani::proof]
#[kani::solver(cadical)]
fn proof_v22_s10_grant_only_refresh_moves() {
    let l = Dom { status: 1, fr: 50_000, valid: 0, cons: 0, imp: 0, exp: 200, claims: 0, rem: 0, exact_full: true, spent_extra: 0, pp: 0, ins: 0 };
    let s = Dom { status: 0, fr: 0, valid: 0, cons: 0, imp: 0, exp: 0, claims: 10_000, rem: 0, exact_full: true, spent_extra: 0, pp: 0, ins: 0 };
    let world = s10_world(l, s);
    kani::assume(world.is_some());
    let (mut header, mut markets) = world.unwrap();
    kani::cover!(true, "grant fixture is shape-valid");
    let (market_id, _, _) = ids();
    let mut account = empty_account_fixture(market_id, 2);
    let sel: u8 = kani::any();
    let reason_sel: u8 = kani::any();
    let reason = match reason_sel % 9 {
        0 => PermissionlessRecoveryReasonV16::BelowProgressFloor,
        1 => PermissionlessRecoveryReasonV16::BlockedSegmentHeadroomOrRepresentability,
        2 => PermissionlessRecoveryReasonV16::AccountBSettlementCannotProgress,
        3 => PermissionlessRecoveryReasonV16::BIndexHeadroomExhausted,
        4 => PermissionlessRecoveryReasonV16::ActiveBankruptCloseCannotProgress,
        5 => PermissionlessRecoveryReasonV16::ExplicitLossOrDustAuditOverflow,
        6 => PermissionlessRecoveryReasonV16::OracleOrTargetUnavailableByAuthenticatedPolicy,
        7 => PermissionlessRecoveryReasonV16::CounterOrEpochOverflowDeclaredRecovery,
        _ => PermissionlessRecoveryReasonV16::BandPinExpired,
    };
    let action = match sel % 4 {
        0 => PermissionlessCrankActionV16::Refresh,
        1 => PermissionlessCrankActionV16::SettleB { asset_index: 0 },
        2 => PermissionlessCrankActionV16::Liquidate(LiquidationRequestV16 { asset_index: 0 }),
        _ => PermissionlessCrankActionV16::Recover(reason),
    };
    let is_refresh = sel % 4 == 0;
    let is_recover = sel % 4 == 3;
    let mode: u8 = kani::any();
    kani::assume(mode <= 2);
    header.mode = mode; // 0 Live, 1 Resolved, 2 Recovery (set after the shape assumption)
    let grant: u8 = kani::any();
    let price = markets[0].engine.asset.effective_price.get();
    let (h0, m0, a0) = (header, markets, account);
    let r = {
        let mut v = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut acc = PortfolioV16ViewMut::new(&mut account);
        v.permissionless_crank_s10_not_atomic(
            &mut acc,
            PermissionlessCrankRequestV16 { now_slot: NOW, asset_index: 0, effective_price: price, funding_rate_e9: 0, action },
            grant,
        )
    };
    let moved = bucket(&markets, 0).fresh_unliened_backing_num < bucket(&m0, 0).fresh_unliened_backing_num;
    let domains_unchanged = markets[0].engine.backing_long == m0[0].engine.backing_long
        && markets[0].engine.backing_short == m0[0].engine.backing_short
        && markets[0].engine.source_credit_long == m0[0].engine.source_credit_long
        && markets[0].engine.source_credit_short == m0[0].engine.source_credit_short;
    if !is_refresh {
        assert!(domains_unchanged, "no S10 move outside the Refresh action, any grant");
    }
    if mode != 0 && !is_recover {
        assert_eq!(r, Err(V16Error::LockActive));
        assert!(header == h0 && markets == m0 && account == a0, "refused before any mutation");
    }
    if is_refresh && grant == 0 {
        assert!(domains_unchanged, "Refresh without a grant moves nothing");
    }
    kani::cover!(is_refresh && mode == 0 && grant > 0 && r.is_ok() && moved, "Refresh with a grant moves");
    kani::cover!(sel % 4 == 1 && grant == 2 && mode == 0, "SettleB reached with grant 2");
    kani::cover!(sel % 4 == 2 && grant == 2 && mode == 0, "Liquidate reached with grant 2");
    kani::cover!(is_recover && grant == 2, "Recover reached with grant 2");
    kani::cover!(mode == 1, "Resolved reached");
    kani::cover!(mode == 2, "Recovery reached");
}

/// E-S10-13 mirror setter, symbolic Pod slot at full width. Mutants S10-E4, S10-N2.
#[kani::proof]
#[kani::solver(cadical)]
fn proof_v22_s10_mirror_setter() {
    let bytes: [u8; core::mem::size_of::<EngineAssetSlotV16Account>()] = kani::any();
    let slot0: EngineAssetSlotV16Account = bytemuck::pod_read_unaligned(&bytes);
    let mut slot = slot0;
    let short: bool = kani::any();
    let d: u128 = kani::any();
    let add: bool = kani::any();
    let before = if short { slot0.provider_principal_short.get() } else { slot0.provider_principal_long.get() };
    let r = adjust_slot_provider_principal(&mut slot, short, d, add);
    let after = if short { slot.provider_principal_short.get() } else { slot.provider_principal_long.get() };
    let mut rest = slot;
    rest.provider_principal_long = slot0.provider_principal_long;
    rest.provider_principal_short = slot0.provider_principal_short;
    assert!(rest == slot0, "no other byte of the slot changes");
    let other_after = if short { slot.provider_principal_long.get() } else { slot.provider_principal_short.get() };
    let other_before = if short { slot0.provider_principal_long.get() } else { slot0.provider_principal_short.get() };
    assert_eq!(other_after, other_before, "the other side is untouched");
    if add {
        match before.checked_add(d) {
            Some(v) => {
                assert!(r.is_ok());
                assert_eq!(after, v);
            }
            None => {
                assert_eq!(r, Err(V16Error::ArithmeticOverflow));
                assert!(slot == slot0, "overflow writes nothing");
            }
        }
    } else {
        assert!(r.is_ok());
        assert_eq!(after, before.saturating_sub(d));
    }
    kani::cover!(add && before.checked_add(d).is_none(), "add overflow");
    kani::cover!(!add && d > before, "subtract saturates");
    kani::cover!(add && short && d > 0 && r.is_ok(), "add short");
    kani::cover!(!add && !short && d > 0 && d <= before, "subtract long");
}

/// E-S10-14 wholly-empty reset in `set_backing_bucket_for_domain` (shim). Mutant S10-R1.
#[kani::proof]
#[kani::solver(cadical)]
fn proof_v22_s10_wholly_empty_reset() {
    let world = s10_world(any_dom(false), any_dom(false));
    kani::assume(world.is_some());
    let (mut header, mut markets) = world.unwrap();
    kani::cover!(true, "fixture is shape-valid");
    let d: usize = if kani::any::<bool>() { 1 } else { 0 };
    let nd = any_dom(false);
    let market_id = markets[0].engine.asset.market_id.get();
    let mut nb = dom_bucket(market_id, nd);
    nb.utilization_fee_earnings = kani::any::<u16>() as u128;
    let old = bucket(&markets, d);
    let (h0, m0) = (header, markets);
    let r = MarketGroupV16ViewMut::new(&mut header, &mut markets).kani_v22_set_backing_bucket_for_domain(d, nb);
    kani::assume(r.is_ok());
    let wholly_empty = nb.fresh_unliened_backing_num == 0 && nb.valid_liened_backing_num == 0
        && nb.consumed_liened_backing_num == 0 && nb.impaired_liened_backing_num == 0;
    if wholly_empty {
        assert_eq!(pp_of(&markets, d), 0);
    } else {
        assert_eq!(pp_of(&markets, d), pp_of(&m0, d));
    }
    assert_eq!(pp_of(&markets, 1 - d), pp_of(&m0, 1 - d), "other side's mirror untouched");
    assert_eq!(bucket(&markets, d), nb);
    assert_eq!(
        header.backing_provider_earnings_total.get() + old.utilization_fee_earnings,
        h0.backing_provider_earnings_total.get() + nb.utilization_fee_earnings
    );
    kani::cover!(wholly_empty && pp_of(&m0, d) > 0, "reset fires");
    kani::cover!(!wholly_empty && nb.fresh_unliened_backing_num == 0 && pp_of(&m0, d) > 0, "fresh 0 but not wholly empty: no reset");
}

// =====================================================================================
// Real-operation worlds (concrete amounts) for E-S10-17, E-X1-1, E-W4-3/4
// =====================================================================================

const PRICE: u64 = 1_000_000;

fn scenario_account(seed: u32) -> PortfolioAccountV16Account {
    let mut key = [0u8; 32];
    key[..4].copy_from_slice(&seed.to_le_bytes());
    key[31] = 0x5A;
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new([1; 32], key, [3; 32]));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(header).unwrap();
    a
}

/// `n_assets`-asset world, the configuration of `tests/v22_s10_cap.rs` (assets activated at slots
/// 1, 3, 5: the activation cooldown).
struct World<const N: usize> {
    header: MarketGroupV16HeaderAccount,
    markets: [Market<u64>; N],
    slot: u64,
}

impl<const N: usize> World<N> {
    fn new() -> Self {
        let mut cfg = V16Config::public_user_fund_with_market_slots(N as u16, N as u32, 0, 6_480_000);
        cfg.max_abs_funding_e9_per_slot = 10_000;
        cfg.max_price_move_bps_per_slot = 200;
        cfg.initial_margin_bps = 1_000;
        cfg.maintenance_margin_bps = 500;
        cfg.max_accrual_dt_slots = 2;
        cfg.min_funding_lifetime_slots = 10_000_000;
        let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, N as u32, 0).unwrap();
        let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default()); N];
        for (i, m) in markets.iter_mut().enumerate() {
            m.wrapper = i as u64;
        }
        for i in 0..N {
            header.activate_empty_asset_slot_not_atomic(i as u32, &mut markets[i].engine, PRICE, 1 + 2 * i as u64).unwrap();
        }
        World { header, markets, slot: 8 }
    }
    fn view(&mut self) -> MarketGroupV16ViewMut<'_, u64> {
        MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets)
    }
    fn deposit(&mut self, a: &mut PortfolioAccountV16Account, amt: u128) {
        self.view().deposit_not_atomic(&mut PortfolioV16ViewMut::new(a), amt).unwrap();
    }
    fn trade(&mut self, i: usize, long: &mut PortfolioAccountV16Account, short: &mut PortfolioAccountV16Account, size: u128) {
        let price = self.markets[i].engine.asset.effective_price.get();
        self.view()
            .execute_trade_with_fee_loss_stale_scoped_not_atomic(
                &mut PortfolioV16ViewMut::new(long),
                &mut PortfolioV16ViewMut::new(short),
                TradeRequestV16 { asset_index: i, size_q: size as i128, exec_price: price, fee_bps: 0 },
                true,
            )
            .unwrap();
    }
    fn tick(&mut self, bps: i64) {
        self.slot += 1;
        for i in 0..N {
            let old = self.markets[i].engine.asset.effective_price.get() as i128;
            let new = (old + old * bps as i128 / 10_000) as u64;
            let slot = self.slot;
            self.view().accrue_asset_to_not_atomic(i, slot, new, 0, true).unwrap();
            self.markets[i].engine.asset.raw_oracle_target_price = V16PodU64::new(new);
        }
    }
    fn refresh(&mut self, a: &mut PortfolioAccountV16Account, budget: u8) -> V16Result<()> {
        let mut b = budget;
        self.view().full_account_refresh_with_s10_budget_not_atomic(&mut PortfolioV16ViewMut::new(a), &mut b).map(|_| ())
    }
}

/// The stranded state of `tests/v22_s10_cap.rs::stranded` on `N` assets: the maker settled alone
/// at the peak, the price fell below entry, the trader settled. Returns (world, maker, trader).
fn stranded<const N: usize>() -> (World<N>, PortfolioAccountV16Account, PortfolioAccountV16Account) {
    let mut w = World::<N>::new();
    let mut a = scenario_account(300);
    let mut maker = scenario_account(0);
    w.deposit(&mut a, 1_000_000_000_000_000);
    w.deposit(&mut maker, 1_000_000_000_000_000);
    for i in 0..N {
        w.trade(i, &mut a, &mut maker, 400 * POS_SCALE);
    }
    for _ in 0..3 {
        w.tick(150);
    }
    w.refresh(&mut maker, S10_MAX_MOVES_PER_INSTRUCTION).unwrap();
    for _ in 0..8 {
        w.tick(-150);
    }
    w.refresh(&mut a, S10_MAX_MOVES_PER_INSTRUCTION).unwrap();
    (w, maker, a)
}

/// E-S10-17 (rev2 R1.1) settle-entry frame with a firing move, as a DIFFERENTIAL frame: the
/// maker's recovery refresh (the settle entry that completes the cohort) from byte-identical
/// states with budget 0 and with budget b. The runs may differ ONLY in the two domains' buckets
/// and source credit, `risk_epoch` and the backing aggregates, and the account's certificate
/// epoch; `kf_pending_credit` is identical. Class L, memory-heavy (RSS cap). Fallback: LiteSVM
/// `v22_s10_cap`.
#[kani::proof]
#[kani::solver(cadical)]
fn proof_v22_s10_settle_entry_frame() {
    let (w, maker, _a) = stranded::<1>();
    kani::cover!(true, "stranded world built");
    let b: u8 = kani::any();
    kani::assume(b <= S10_MAX_MOVES_PER_INSTRUCTION);
    let (mut w0, mut m0) = (World::<1> { header: w.header, markets: w.markets, slot: w.slot }, maker);
    let (mut wb, mut mb) = (World::<1> { header: w.header, markets: w.markets, slot: w.slot }, maker);
    let r0 = w0.refresh(&mut m0, 0);
    let rb = wb.refresh(&mut mb, b);
    assert_eq!(r0.is_ok(), rb.is_ok());
    kani::assume(r0.is_ok());
    let e0 = &w0.markets[0].engine;
    let eb = &wb.markets[0].engine;
    assert!(eb.kf_pending_credit_long == e0.kf_pending_credit_long && eb.kf_pending_credit_short == e0.kf_pending_credit_short);
    let mut ex = *eb;
    ex.backing_long = e0.backing_long;
    ex.backing_short = e0.backing_short;
    ex.source_credit_long = e0.source_credit_long;
    ex.source_credit_short = e0.source_credit_short;
    assert!(ex == *e0, "slot frame: only the two domains differ");
    let mut hx = wb.header;
    hx.risk_epoch = w0.header.risk_epoch;
    hx.backing_provider_earnings_total = w0.header.backing_provider_earnings_total;
    hx.source_fresh_backing_total_num = w0.header.source_fresh_backing_total_num;
    assert!(hx == w0.header, "header frame");
    let mut ax = mb;
    ax.health_cert = m0.health_cert;
    assert!(ax == m0, "account frame: only the certificate's epochs may differ");
    let moved = eb.backing_short != e0.backing_short || eb.backing_long != e0.backing_long;
    if b == 0 {
        assert!(!moved);
    }
    kani::cover!(b > 0 && moved, "the move fires inside the settle entry");
}

// =====================================================================================
// E-X1-1 refinement of the liquidation-path fee charge
// =====================================================================================

/// Shared X1 world: two assets, both accounts hold a leg on each (>= 2 legs, >= 2 domains), both
/// refreshed with budget 0 (so every leg is at its current K/F/B/rent snapshots: the refinement
/// precondition), then symbolic `kf_pending_credit` per domain, a symbolic fee, an optional
/// B-stale flag and an optional non-Live mode.
fn x1_catch_up<const N: usize>(w: &mut World<N>) {
    // advance every asset to the current slot (accrual is bounded per call by max_accrual_dt_slots)
    for _ in 0..8 {
        for i in 0..N {
            let cur = w.header.current_slot.get();
            let p = w.markets[i].engine.asset.effective_price.get();
            let _ = w.view().accrue_asset_to_not_atomic(i, cur, p, 0, true);
        }
    }
}

/// Review M2: three concrete worlds built through REAL operations, selected symbolically.
/// * 0, base: two assets, the trader long both, three +1.5% ticks, both refreshed (budget 0).
/// * 1, lien: the trader (capital 1_000_000) wins, accrual caught up, both refreshed, then
///   INCREASES its asset-0 long through the real trade path, which liens its source-backed PnL;
///   refreshed. The charged account is the trader (liened).
/// * 2, negative PnL: the maker (capital 1_000_000, exactly enough initial margin) loses more than its
///   capital over eight +1.5% ticks; refreshed; the charged account is the maker (pnl < 0, capital 0).
/// Native replay (`kani-work/fx/tests/x1.rs`, mock-cfg build calling the same shims): variant 1
/// liened = true, both fee paths Ok; variant 2 pnl = -11_920, both fee paths Ok.
fn x1_world() -> (World<2>, PortfolioAccountV16Account, u128, bool) {
    let variant: u8 = kani::any();
    kani::assume(variant < 3);
    let mut w = World::<2>::new();
    let mut a = scenario_account(300);
    let mut maker = scenario_account(0);
    let (dep_a, dep_m, size, ticks) = match variant {
        0 => (1_000_000_000_000_000u128, 1_000_000_000_000_000u128, 400 * POS_SCALE, 3usize),
        1 => (1_000_000u128, 1_000_000_000_000_000u128, 4 * POS_SCALE, 3usize),
        _ => (1_000_000_000_000_000u128, 1_000_000u128, 4 * POS_SCALE, 8usize),
    };
    w.deposit(&mut a, dep_a);
    w.deposit(&mut maker, dep_m);
    for i in 0..2 {
        w.trade(i, &mut a, &mut maker, size);
    }
    for _ in 0..ticks {
        w.tick(150);
    }
    if variant == 1 {
        x1_catch_up(&mut w);
    }
    w.refresh(&mut maker, 0).unwrap();
    w.refresh(&mut a, 0).unwrap();
    if variant == 1 {
        w.trade(0, &mut a, &mut maker, 4 * POS_SCALE); // the lien-creating increase (real path)
        w.refresh(&mut maker, 0).unwrap();
        w.refresh(&mut a, 0).unwrap();
    }
    let pick_maker: bool = if variant == 2 { true } else if variant == 1 { false } else { kani::any() };
    let acc = if pick_maker { maker } else { a };
    for i in 0..2 {
        let pl: i32 = kani::any();
        let ps: i32 = kani::any();
        w.markets[i].engine.kf_pending_credit_long = V16PodI128::new(pl as i128 * BOUND_SCALE as i128 / 1_000);
        w.markets[i].engine.kf_pending_credit_short = V16PodI128::new(ps as i128 * BOUND_SCALE as i128 / 1_000);
    }
    let fee = kani::any::<u16>() as u128;
    let non_live: bool = kani::any();
    if non_live {
        w.header.mode = 1;
    }
    (w, acc, fee, pick_maker)
}

fn any_pending_above_claims(m: &[Market<u64>; 2]) -> bool {
    m.iter().any(|mk| {
        let e = &mk.engine;
        let cl = e.source_credit_long.try_to_runtime().map(|s| s.positive_claim_bound_num).unwrap_or(0);
        let cs = e.source_credit_short.try_to_runtime().map(|s| s.positive_claim_bound_num).unwrap_or(0);
        e.kf_pending_credit_long.get() > cl as i128 || e.kf_pending_credit_short.get() > cs as i128
    })
}

/// E-X1-1 `_ok_state`: on both `Ok`, header, both slots and the account are byte-identical
/// (as `x1_diff_charge` compares, `:23088-23090`). Class L, memory-heavy. Fallback: `x1-diff`.
/// Mutants X1-M1 (noclamp), X1-M2, X1-M3.
#[kani::proof]
#[kani::solver(cadical)]
fn proof_v22_x1_fee_refinement_ok_state() {
    let (w, acc, fee, _) = x1_world();
    kani::cover!(true, "X1 world built");
    let (mut h1, mut mk1, mut a1) = (w.header, w.markets, acc);
    let (mut h2, mut mk2, mut a2) = (w.header, w.markets, acc);
    let clamp_case = any_pending_above_claims(&w.markets);
    let r_new = MarketGroupV16ViewMut::new(&mut h1, &mut mk1).kani_v22_charge_fee_after_full_refresh(&mut PortfolioV16ViewMut::new(&mut a1), fee);
    let r_ref = MarketGroupV16ViewMut::new(&mut h2, &mut mk2).kani_v22_charge_fee_reference(&mut PortfolioV16ViewMut::new(&mut a2), fee);
    if r_new.is_ok() && r_ref.is_ok() {
        assert_eq!(r_new, r_ref);
        assert!(h1 == h2, "header identical");
        assert!(mk1 == mk2, "every slot identical (incl. kf_pending_credit of every domain)");
        assert!(a1 == a2, "account identical");
    }
    let active_legs = acc.legs.iter().filter(|l| l.active == 1).count();
    let liened = acc.source_domains.iter().any(|d| d.source_claim_liened_num.get() > 0);
    kani::cover!(active_legs >= 2 && r_new.is_ok() && r_ref.is_ok(), ">= 2 legs, both Ok");
    kani::cover!(clamp_case && r_new.is_ok() && r_ref.is_ok(), "pending credit above claims: the clamp fires");
    kani::cover!(acc.pnl.get() < 0 && r_new.is_ok(), "negative PnL (capital exhausted), Ok");
    kani::cover!(r_new.map_or(false, |c| c > 0), "fee charged > 0");
    kani::cover!(liened, "an account holding a lien");
}

/// E-X1-1 `_result`: the two results are equal for every input, incl. the error variant
/// (B-stale and non-Live). Class L. Mutant X1-M4 (Live gate dropped).
#[kani::proof]
#[kani::solver(cadical)]
fn proof_v22_x1_fee_refinement_result() {
    let (w, mut acc, fee, _) = x1_world();
    kani::cover!(true, "X1 world built");
    let b_stale: bool = kani::any();
    if b_stale {
        acc.b_stale_state = 1;
    }
    let (mut h1, mut mk1, mut a1) = (w.header, w.markets, acc);
    let (mut h2, mut mk2, mut a2) = (w.header, w.markets, acc);
    let r_new = MarketGroupV16ViewMut::new(&mut h1, &mut mk1).kani_v22_charge_fee_after_full_refresh(&mut PortfolioV16ViewMut::new(&mut a1), fee);
    let r_ref = MarketGroupV16ViewMut::new(&mut h2, &mut mk2).kani_v22_charge_fee_reference(&mut PortfolioV16ViewMut::new(&mut a2), fee);
    assert_eq!(r_new, r_ref);
    kani::cover!(w.header.mode != 0 && r_new == Err(V16Error::LockActive), "non-Live: both LockActive");
    kani::cover!(b_stale && w.header.mode == 0 && r_new.is_err(), "B-stale: both refuse");
    kani::cover!(r_new.is_ok(), "both Ok");
}

/// E-X1-5 / E-S9-1: the only pricing reader of `kf_pending_credit` is clamped to [0, claims].
/// Mutants X1-M5, X1-M6.
#[kani::proof]
#[kani::solver(cadical)]
fn proof_v22_pending_credit_reader_clamped() {
    let (mut header, mut markets) = one_market_only_fixture();
    let stored: i128 = kani::any();
    let claims_atoms: u64 = kani::any();
    let claims = claims_atoms as u128 * BOUND_SCALE;
    let short: bool = kani::any();
    let s = SourceCreditStateV16 {
        positive_claim_bound_num: claims,
        exact_positive_claim_num: 0,
        credit_rate_num: if claims == 0 { CREDIT_RATE_SCALE } else { 0 },
        ..SourceCreditStateV16::EMPTY
    };
    if short {
        markets[0].engine.kf_pending_credit_short = V16PodI128::new(stored);
        markets[0].engine.source_credit_short = SourceCreditStateV16Account::from_runtime(&s);
    } else {
        markets[0].engine.kf_pending_credit_long = V16PodI128::new(stored);
        markets[0].engine.source_credit_long = SourceCreditStateV16Account::from_runtime(&s);
    }
    let v = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let r = v.kani_v22_kf_pending_credit_num(if short { 1 } else { 0 });
    kani::assume(r.is_ok());
    let x = r.unwrap();
    assert!(x <= claims);
    if stored <= 0 {
        assert_eq!(x, 0);
    } else if (stored as u128) <= claims {
        assert_eq!(x, stored as u128);
    } else {
        assert_eq!(x, claims);
    }
    kani::cover!(stored < 0, "negative stored");
    kani::cover!(stored > 0 && stored as u128 > claims, "stored above claims");
    kani::cover!(stored > 0 && (stored as u128) < claims, "stored inside");
}

/// E-S9-3 (rev2 R1.4): the S9 protective rate stays in [stored, SCALE], claims 0 => stored,
/// `denominator == 0 || available >= denominator` => SCALE, and is monotone non-decreasing in
/// `pending`. The S9 `claims - 1` clamp is NOT in the code (F8 gap): `pending >= claims` gives 1.
/// u16 atoms (U256 path). Mutants S9-M1, S9-M2.
#[kani::proof]
#[kani::solver(cadical)]
fn proof_v22_s9_protective_rate() {
    let claims = n(kani::any());
    let fresh = n(kani::any());
    let stored: u64 = kani::any();
    kani::assume(stored as u128 <= CREDIT_RATE_SCALE);
    let s = SourceCreditStateV16 {
        positive_claim_bound_num: claims,
        fresh_reserved_backing_num: fresh,
        credit_rate_num: stored as u128,
        ..SourceCreditStateV16::EMPTY
    };
    let p1 = n(kani::any());
    let p2 = n(kani::any());
    kani::assume(p1 <= p2);
    let r1 = View::kani_v22_source_credit_protective_rate(s, p1);
    let r2 = View::kani_v22_source_credit_protective_rate(s, p2);
    kani::assume(r1.is_ok() && r2.is_ok());
    let (r1, r2) = (r1.unwrap(), r2.unwrap());
    if claims == 0 {
        assert_eq!(r1, stored as u128);
    } else {
        assert!(r1 >= stored as u128 && r1 <= CREDIT_RATE_SCALE);
        let den = claims.saturating_sub(p1);
        if den == 0 || fresh >= den {
            assert_eq!(r1, CREDIT_RATE_SCALE);
        }
        assert!(r1 <= r2, "monotone in pending");
    }
    kani::cover!(claims > 0 && p1 >= claims, "pending >= claims gives rate 1 (F8/S9 gap)");
    kani::cover!(claims > 0 && r1 < r2, "strictly increasing in pending");
    kani::cover!(claims > 0 && r1 == stored as u128 && stored > 0, "stored rate is the floor");
}

// =====================================================================================
// Leg cap and ADL
// =====================================================================================

/// E-CAP-1 validator: `validate_with_market` Ok => every slot at or above `max_portfolio_assets`
/// has bitmap bit 0 and an empty leg. Symbolic tail from `cap` (rev2 R2.1). Mutant CAP-M1.
#[kani::proof]
#[kani::unwind(33)]
#[kani::solver(cadical)]
fn proof_v22_leg_cap_validator() {
    let cap: u16 = kani::any();
    kani::assume(cap >= 1 && cap <= 4);
    let (market_id, _, _) = ids();
    let cfg = V16Config::public_user_fund_with_market_slots(cap, 4, 0, 10);
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(market_id, cfg, 1, 0).unwrap();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    MarketGroupV16ViewMut::new(&mut header, &mut markets).activate_empty_market_not_atomic(0, 100, 1).unwrap();
    let mut account = empty_account_fixture(market_id, 2);
    let leg_bytes: [u8; core::mem::size_of::<PortfolioLegV16Account>()] = kani::any();
    // review M8: the symbolic leg sits at ANY slot in [cap, V16_MAX_PORTFOLIO_ASSETS_N), so a
    // `==`-for-`>=` defect in the validator is caught (not only the slot exactly at the cap).
    let slot: usize = kani::any();
    kani::assume(slot >= cap as usize && slot < percolator::V16_MAX_PORTFOLIO_ASSETS_N);
    account.legs[slot] = bytemuck::pod_read_unaligned(&leg_bytes);
    let bit: bool = kani::any();
    let mut bitmap = account.active_bitmap.map(V16PodU64::get);
    if bit {
        bitmap[slot / 64] |= 1u64 << (slot % 64);
    }
    account.active_bitmap = bitmap.map(V16PodU64::new);
    let v = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let r = PortfolioV16ViewMut::new(&mut account).as_view().validate_with_market(&v.as_view());
    let empty = account.legs[slot] == PortfolioLegV16Account::from_runtime(&PortfolioLegV16::EMPTY);
    if r.is_ok() {
        assert!(!bit && empty, "nothing lives at or above the cap");
    }
    kani::cover!(!empty && r == Err(V16Error::HiddenLeg), "non-empty leg at slot cap refused");
    kani::cover!(empty && !bit && r.is_ok(), "empty slot at the cap accepted");
    kani::cover!(cap == 4 && r.is_ok(), "cap 4");
    kani::cover!(!empty && slot > cap as usize && r == Err(V16Error::HiddenLeg), "non-empty leg strictly above the cap refused");
}

/// E-CAP-1 batch-length refusal (rev2.1 5b), site `:22230`
/// (`fork_execute_batch_after_tail_validation_with_threshold_not_atomic`, reached from `:22187`):
/// two valid requests on a two-asset market with cap 1 are refused with `InvalidConfig` BEFORE
/// any mutation. Mutant CAP-M2.
#[cfg(feature = "fork-facade")]
#[kani::proof]
#[kani::solver(cadical)]
fn proof_v22_batch_len_refused_fork_threshold() {
    batch_len_body(true);
}

/// Same at site `:22346` (`execute_batch_with_fee_after_tail_validation_not_atomic`, reached from
/// `:22095`). Mutant CAP-M3.
#[kani::proof]
#[kani::solver(cadical)]
fn proof_v22_batch_len_refused_with_fee() {
    batch_len_body(false);
}

fn batch_len_body(fork: bool) {
    let mut cfg = V16Config::public_user_fund_with_market_slots(2, 2, 0, 10);
    cfg.max_portfolio_assets = 1;
    let (market_id, _, _) = ids();
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(market_id, cfg, 2, 0).unwrap();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default()), Market::new(1u64, EngineAssetSlotV16Account::default())];
    {
        let mut v = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        v.activate_empty_market_not_atomic(0, 100, 1).unwrap();
        v.activate_empty_market_not_atomic(1, 100, 3).unwrap();
    }
    let mut long = empty_account_fixture(market_id, 5);
    let mut short = empty_account_fixture(market_id, 6);
    let size: u8 = kani::any();
    kani::assume(size >= 1);
    let reqs = [
        TradeRequestV16 { asset_index: 0, size_q: size as i128 * POS_SCALE as i128, exec_price: 100, fee_bps: 0 },
        TradeRequestV16 { asset_index: 1, size_q: size as i128 * POS_SCALE as i128, exec_price: 100, fee_bps: 0 },
    ];
    let len: usize = if kani::any::<bool>() { 2 } else { 1 };
    let (h0, m0, l0, s0) = (header, markets, long, short);
    let r = {
        let mut v = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut lv = PortfolioV16ViewMut::new(&mut long);
        let mut sv = PortfolioV16ViewMut::new(&mut short);
        #[cfg(feature = "fork-facade")]
        let fork_result = if fork {
            Some(v.kani_v22_fork_batch_after_tail_validation(&mut lv, &mut sv, &reqs[..len], None, true))
        } else {
            None
        };
        #[cfg(not(feature = "fork-facade"))]
        let fork_result: Option<V16Result<percolator::v16::BatchTradeOutcomeV16>> = { let _ = fork; None };
        if let Some(fr) = fork_result {
            fr
        } else {
            v.kani_v22_batch_with_fee_after_tail_validation(&mut lv, &mut sv, &reqs[..len], true)
        }
    };
    if len == 2 {
        assert_eq!(r, Err(V16Error::InvalidConfig));
        assert!(header == h0 && markets == m0 && long == l0 && short == s0, "refused before any mutation");
    }
    kani::cover!(len == 2, "len == cap + 1 refused");
    kani::cover!(len == 1 && r != Err(V16Error::InvalidConfig), "len == cap passes the length check");
}

/// E-ADL-1: a risk-increasing route passes the ADL gate only at A == ADL_ONE; attach copies the
/// side's A into a_basis; resize keeps a_basis and both remainders (also E-REM-4 resize half).
/// Mutant ADL-M1.
#[kani::proof]
#[kani::solver(cadical)]
fn proof_v22_attached_legs_have_unit_a() {
    let (mut header, mut markets) = one_market_only_fixture();
    // review M3: symbolic side (both branches of the ADL gate) and a resize leg whose a_basis is
    // its OWN symbolic value, so a resize that rewrites a_basis to the asset's A is visible.
    let short: bool = kani::any();
    let side = if short { SideV16::Short } else { SideV16::Long };
    let a_side: u128 = kani::any();
    kani::assume(a_side >= MIN_A_SIDE && a_side <= ADL_ONE);
    if short {
        markets[0].engine.asset.a_short = V16PodU128::new(a_side);
    } else {
        markets[0].engine.asset.a_long = V16PodU128::new(a_side);
    }
    let cur_abs: i64 = kani::any();
    let new_abs: i64 = kani::any();
    kani::assume(cur_abs >= 0 && new_abs >= 0); // same side: risk-increasing iff |new| > |cur|
    let (cur, new) = if short { (-(cur_abs as i128), -(new_abs as i128)) } else { (cur_abs as i128, new_abs as i128) };
    let g = MarketGroupV16ViewMut::new(&mut header, &mut markets).kani_require_position_change_adl_safe(0, cur, new);
    if g.is_ok() && new_abs > cur_abs {
        assert_eq!(a_side, ADL_ONE, "an increase is admitted only at unit A");
    }
    let asset = markets[0].engine.asset.try_to_runtime();
    kani::assume(asset.is_ok());
    let asset = asset.unwrap();
    let basis = kani::any::<u32>() as i128 + 1;
    let signed_basis = if short { -basis } else { basis };
    let att = View::kani_v22_attach_leg(asset, side, signed_basis, basis as u128, 0, 0, 0);
    if let Ok((_, leg)) = att {
        assert_eq!(leg.a_basis, a_side, "attach copies the side's A");
        assert!(leg.k_rem_num == 0 && leg.f_rem_num == 0, "attach starts at remainder 0");
    }
    // resize keeps a_basis (its own value, not the asset's A) and the remainders
    let leg_a: u128 = kani::any();
    kani::assume(leg_a >= MIN_A_SIDE && leg_a <= ADL_ONE);
    let mut leg = PortfolioLegV16 { active: true, side, basis_pos_q: signed_basis, a_basis: leg_a, loss_weight: basis as u128, ..PortfolioLegV16::EMPTY };
    leg.k_rem_num = kani::any::<u64>() as u128;
    leg.f_rem_num = kani::any::<u64>() as u128;
    let new_basis = kani::any::<u32>() as i128 + 1;
    let new_signed = if short { -new_basis } else { new_basis };
    let rs = View::kani_v22_resize_leg_same_side(leg, asset, new_signed, new_basis as u128, false, basis as u128, new_basis as u128);
    if let Ok((leg2, _)) = rs {
        assert!(leg2.a_basis == leg.a_basis && leg2.k_rem_num == leg.k_rem_num && leg2.f_rem_num == leg.f_rem_num);
    }
    kani::cover!(g.is_ok() && new_abs > cur_abs && a_side == ADL_ONE && !short, "long increase admitted at unit A");
    kani::cover!(g.is_ok() && new_abs > cur_abs && a_side == ADL_ONE && short, "short increase admitted at unit A");
    kani::cover!(g.is_err() && new_abs > cur_abs && a_side < ADL_ONE, "increase refused under scaled A");
    kani::cover!(att.is_ok() && a_side == ADL_ONE, "attach ok");
    kani::cover!(rs.is_ok() && leg.a_basis != a_side, "resize keeps an a_basis that differs from the asset's A");
    kani::cover!(rs.is_ok() && short, "short resize");
}

// =====================================================================================
// W-4 repay from released PnL (engine #285)
// =====================================================================================

/// E-W4-1 post-condition predicate, full u128 width. Mutants W4-E1..E5.
#[kani::proof]
#[kani::solver(cadical)]
fn proof_v22_w4_postcondition_predicate() {
    let (vb, va, ib, ia, cb, ca, tb, ta, t): (u128, u128, u128, u128, u128, u128, u128, u128, u128) =
        (kani::any(), kani::any(), kani::any(), kani::any(), kani::any(), kani::any(), kani::any(), kani::any(), kani::any());
    let got = repay_pnl_postconditions_hold(vb, va, ib, ia, cb, ca, tb, ta, t);
    let c1 = va == vb;
    let c2 = ia.checked_sub(ib) == Some(t);
    let c3 = ca <= cb;
    let c4 = ta <= tb;
    let c5 = c3 && c4 && cb - ca == tb - ta;
    assert_eq!(got, c1 && c2 && c3 && c4 && c5);
    kani::cover!(!c1 && c2 && c5, "vault clause alone false");
    kani::cover!(c1 && !c2 && c5, "insurance clause alone false");
    kani::cover!(c1 && c2 && !c3 && c4, "capital clause alone false");
    kani::cover!(c1 && c2 && c3 && !c4, "c_tot clause alone false");
    kani::cover!(c1 && c2 && c3 && c4 && !c5, "equal-fall clause alone false");
    kani::cover!(got && ca < cb, "all true with capital falling");
}

/// E-W4-3/4 fixture (review M1; label per round 2): a FIELD-SEEDED base (the claim state: pnl,
/// source_domains[0], five header totals, source_credit_short, backing_short) copied verbatim from
/// the engine spec suite's `w4_world` (`tests/v16_spec_tests.rs:14330-14382`, which
/// `w4_repay_from_released_pnl_with_open_exposure_moves_value_only_into_insurance` pins at capacity
/// > 0), followed by REAL ops: deposits, a funded trade, a refresh (and, in the harness, resolve).
/// Evidence label: "seeded spec-suite fixture + real ops" (shape-valid under the cfg(kani) full audit
/// scan, not reached by a production path). `capital == 0` makes the same trade take a lien through
/// the real trade path (the spec's case (e): lien-held). Native replay (`kani-work/fx/tests/w4.rs`):
/// capital 1_000 -> capacity 100, repay(50,50) = Ok(100), repay(10,10) = Ok(20); capital 0 -> liened,
/// capacity 0, repay -> Err(LockActive).
fn w4_world(claim: u128, capital: u128) -> (MarketGroupV16HeaderAccount, [Market<u64>; 1], PortfolioAccountV16Account) {
    let (market_id, _, owner) = ids();
    let cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(market_id, cfg, 1, 0).unwrap();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    header.activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, 1, 1).unwrap();
    let acct = |seed: u8| {
        let h = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(market_id, [seed; 32], owner));
        let mut a = PortfolioAccountV16Account::default();
        a.init_empty_in_place(h).unwrap();
        a
    };
    let mut long_h = acct(8);
    let mut short_h = acct(9);
    let claim_num = claim * BOUND_SCALE;
    long_h.pnl = V16PodI128::new(claim as i128);
    long_h.source_domains[0].domain = percolator::V16PodU32::new(1);
    long_h.source_domains[0].source_claim_market_id = V16PodU64::new(1);
    long_h.source_domains[0].source_claim_bound_num = V16PodU128::new(claim_num);
    header.pnl_pos_tot = V16PodU128::new(claim);
    header.pnl_pos_bound_tot_num = V16PodU128::new(claim_num);
    header.pnl_pos_bound_tot = V16PodU128::new(claim);
    header.source_claim_bound_total_num = V16PodU128::new(claim_num);
    header.source_fresh_backing_total_num = V16PodU128::new(claim_num);
    header.vault = V16PodU128::new(claim + header.vault.get());
    markets[0].engine.source_credit_short = SourceCreditStateV16Account::from_runtime(&SourceCreditStateV16 {
        positive_claim_bound_num: claim_num,
        exact_positive_claim_num: claim_num,
        fresh_reserved_backing_num: claim_num,
        credit_rate_num: CREDIT_RATE_SCALE,
        ..SourceCreditStateV16::EMPTY
    });
    markets[0].engine.backing_short = BackingBucketV16Account::from_runtime(&BackingBucketV16 {
        market_id: 1,
        fresh_unliened_backing_num: claim_num,
        expiry_slot: 100,
        status: BackingBucketStatusV16::Fresh,
        ..BackingBucketV16::EMPTY
    });
    {
        let mut m = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut short = PortfolioV16ViewMut::new(&mut short_h);
        m.deposit_not_atomic(&mut short, 1_000).unwrap();
        let mut long = PortfolioV16ViewMut::new(&mut long_h);
        m.deposit_not_atomic(&mut long, capital).unwrap();
        m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut long,
            &mut short,
            TradeRequestV16 { asset_index: 0, size_q: (10 * POS_SCALE) as i128, exec_price: 1, fee_bps: 0 },
            true,
        )
        .unwrap();
        m.full_account_refresh_not_atomic(&mut long).unwrap();
    }
    (header, markets, long_h)
}

/// Review M1 (E-W4-4): the claim domain's credit becomes INSURANCE-backed through real ops: fund
/// the domain's insurance (`deposit_domain_insurance_not_atomic`), reserve insurance credit with the
/// kani|fuzz writer `reserve_insurance_credit_not_atomic`, then withdraw the counterparty fresh
/// backing (`withdraw_fresh_counterparty_backing_not_atomic`). Native replay: capacity 100 (> 0),
/// and repay(10,10) / (50,50) / (1,0) = Err(InvalidConfig) with insurance unchanged: the refusal is
/// the post-condition (insurance delta != total), NOT the capacity-0 LockActive.
fn w4_insurance_world() -> (MarketGroupV16HeaderAccount, [Market<u64>; 1], PortfolioAccountV16Account) {
    let (mut h, mut mk, a) = w4_world(100, 1_000);
    {
        let mut m = MarketGroupV16ViewMut::new(&mut h, &mut mk);
        m.deposit_domain_insurance_not_atomic(1, 200).unwrap();
        m.reserve_insurance_credit_not_atomic(1, 100 * BOUND_SCALE).unwrap();
        m.withdraw_fresh_counterparty_backing_not_atomic(1, 100).unwrap();
    }
    (h, mk, a)
}

fn w4_body(insurance_backed: bool) {
    let liened_variant: bool = if insurance_backed { false } else { kani::any() };
    let (mut header, mut markets, mut acc) = if insurance_backed {
        w4_insurance_world()
    } else if liened_variant {
        w4_world(100, 0)
    } else {
        w4_world(100, 1_000)
    };
    kani::cover!(true, "W4 world built");
    let resolved: bool = if insurance_backed { false } else { kani::any() };
    if resolved {
        let slot = header.current_slot.get();
        kani::assume(MarketGroupV16ViewMut::new(&mut header, &mut markets).resolve_market_not_atomic(slot).is_ok());
    }
    let amount_a = kani::any::<u8>() as u128;
    let amount_b = kani::any::<u8>() as u128;
    let liened = acc.source_domains.iter().any(|d| d.source_claim_liened_num.get() > 0);
    // capacity as read by the function: on a clone, after the same first refresh
    let cap = {
        let (mut hc, mut mc, mut ac) = (header, markets, acc);
        let mut v = MarketGroupV16ViewMut::new(&mut hc, &mut mc);
        let mut av_ = PortfolioV16ViewMut::new(&mut ac);
        match v.full_account_refresh_not_atomic(&mut av_) {
            Ok(_) => v.released_pnl_insurance_repay_capacity(&av_.as_view()).unwrap_or(0),
            Err(_) => 0,
        }
    };
    let (h0, a0) = (header, acc);
    let r = MarketGroupV16ViewMut::new(&mut header, &mut markets)
        .repay_insurance_from_released_pnl_not_atomic(&mut PortfolioV16ViewMut::new(&mut acc), 0, amount_a, 1, amount_b);
    if let Ok(t) = r {
        if t > 0 {
            assert_eq!(t, amount_a + amount_b);
            assert!(t <= cap, "t <= capacity");
            assert_eq!(header.vault.get(), h0.vault.get());
            assert_eq!(header.insurance.get() - h0.insurance.get(), t);
            assert!(acc.capital.get() <= a0.capital.get() && header.c_tot.get() <= h0.c_tot.get());
            assert_eq!(a0.capital.get() - acc.capital.get(), h0.c_tot.get() - header.c_tot.get());
            for i in 0..acc.source_domains.len() {
                assert_eq!(acc.source_domains[i].source_claim_liened_num, a0.source_domains[i].source_claim_liened_num);
                assert_eq!(acc.source_domains[i].source_claim_impaired_num, a0.source_domains[i].source_claim_impaired_num);
            }
        }
    }
    if liened && amount_a + amount_b > 0 {
        assert!(r.is_err(), "liened => Err");
    }
    if resolved && amount_a + amount_b > 0 {
        assert!(r.is_err(), "not Live => Err");
    }
    if insurance_backed {
        assert!(r.map_or(true, |t| t == 0), "insurance-credit-backed repay refused");
        kani::cover!(cap > 0 && amount_a + amount_b > 0 && r == Err(V16Error::InvalidConfig), "capacity > 0 and refused at the insurance-delta post-condition");
    } else {
        kani::cover!(r.map_or(false, |t| t > 0) && amount_a > 0 && amount_b > 0, "both sweep legs non-zero and Ok");
        kani::cover!(r.map_or(false, |t| t > 0 && t == cap), "t == capacity");
        kani::cover!(r.map_or(false, |t| t > 0 && t < cap), "t < capacity");
        kani::cover!(liened && amount_a > 0 && r.is_err(), "liened account refused");
        kani::cover!(resolved && amount_a > 0 && r.is_err(), "Resolved refused");
    }
}

/// E-W4-3 engine-level repay (rev2 R1.3, rev2.1 5a). Class L, memory-heavy. Fallback: LiteSVM
/// end-to-end. Mutants W4-L, W4-C, W4-V, W4-S.
#[kani::proof]
#[kani::solver(cadical)]
fn proof_v22_w4_repay_engine_level() {
    w4_body(false);
}

/// E-W4-4 insurance-credit branch refused. Class L. Mutant W4-I.
#[kani::proof]
#[kani::solver(cadical)]
fn proof_v22_w4_insurance_credit_branch_refused() {
    w4_body(true);
}

// =====================================================================================
// Leg remainders (#281, R2) and #277
// =====================================================================================

const DEN: u128 = ADL_ONE * POS_SCALE;

/// E-REM-1: the fast path, when it answers, equals the wide path; incl. the early-return arm
/// (`:27297-27302`, any a_basis). Class L (U256 on the wide side). Mutants REM-M1, REM-M2.
#[kani::proof]
#[kani::solver(cadical)]
fn proof_v22_rem_fast_equals_wide() {
    let early: bool = kani::any();
    if early {
        let a: u128 = kani::any();
        kani::assume(a >= MIN_A_SIDE && a <= ADL_ONE);
        let den = a * POS_SCALE;
        let carry: u128 = kani::any();
        kani::assume(carry < den);
        let basis_zero: bool = kani::any();
        let basis: u128 = if basis_zero { 0 } else { kani::any::<u32>() as u128 };
        let t: i64 = kani::any();
        let then = t as i128;
        let now = if basis_zero { then + kani::any::<i32>() as i128 } else { then };
        let f = View::kani_v22_scaled_adl_delta_with_carry_fast(basis, a, then, now, carry);
        assert_eq!(f, Some((0, carry)));
        let wide = percolator::wide_math::wide_signed_mul_div_floor_with_carry_from_k_pair(basis, then, now, den, carry);
        assert_eq!(wide, (0, carry));
        kani::cover!(basis_zero, "early arm: zero basis");
        kani::cover!(!basis_zero && a < ADL_ONE, "early arm: no index move, scaled A");
    } else {
        let basis = kani::any::<u32>() as u128;
        let j: i16 = kani::any();
        let k: i16 = kani::any();
        let c: u32 = kani::any();
        kani::assume((c as u128) < POS_SCALE);
        let then = j as i128 * ADL_ONE as i128;
        let now = then + k as i128 * ADL_ONE as i128;
        let carry = c as u128 * ADL_ONE;
        let f = View::kani_v22_scaled_adl_delta_with_carry_fast(basis, ADL_ONE, then, now, carry);
        if let Some(q) = f {
            let wide = percolator::wide_math::wide_signed_mul_div_floor_with_carry_from_k_pair(basis, then, now, DEN, carry);
            assert_eq!(q, wide);
        }
        kani::cover!(f.map_or(false, |(q, _)| q < 0), "negative quotient (remainder fix-up)");
        kani::cover!(f.map_or(false, |(q, _)| q > 0), "positive quotient");
        kani::cover!(f.is_some() && c > 0 && k != 0, "carry > 0");
    }
}

/// E-REM-2 partition invariance on the production fast path. Mutant REM-M3.
#[kani::proof]
#[kani::solver(cadical)]
fn proof_v22_rem_partition_invariance() {
    let basis = kani::any::<u16>() as u128;
    let k1: i8 = kani::any();
    let k2: i8 = kani::any();
    let c: u32 = kani::any();
    kani::assume((c as u128) < POS_SCALE);
    let t0: i128 = 0;
    let t1 = k1 as i128 * ADL_ONE as i128;
    let t2 = t1 + k2 as i128 * ADL_ONE as i128;
    let carry = c as u128 * ADL_ONE;
    let a = View::kani_v22_scaled_adl_delta_with_carry_fast(basis, ADL_ONE, t0, t1, carry);
    kani::assume(a.is_some());
    let (q1, r1) = a.unwrap();
    let b = View::kani_v22_scaled_adl_delta_with_carry_fast(basis, ADL_ONE, t1, t2, r1);
    let w = View::kani_v22_scaled_adl_delta_with_carry_fast(basis, ADL_ONE, t0, t2, carry);
    kani::assume(b.is_some() && w.is_some());
    let (q2, r2) = b.unwrap();
    let (q, r) = w.unwrap();
    assert!(r1 < DEN && r2 < DEN);
    assert_eq!(q, q1 + q2);
    assert_eq!(r, r2);
    kani::cover!(k1 > 0 && k2 < 0 && basis > 0, "sign reversal between halves");
    kani::cover!(k1 > 0 && k2 > 0 && basis > 0 && c > 0, "same sign, with carry");
}

fn rem_body(generic_a: bool) {
    let mut asset = AssetStateV16::default();
    let sel: u8 = kani::any();
    let a = if !generic_a { ADL_ONE } else {
        match sel % 3 {
            0 => MIN_A_SIDE,
            1 => ADL_ONE / 2,
            _ => ADL_ONE,
        }
    };
    let kn: i16 = kani::any();
    let fnow: i16 = kani::any();
    asset.k_long = kn as i128 * ADL_ONE as i128;
    asset.f_long_num = fnow as i128 * ADL_ONE as i128;
    asset.a_long = a;
    let ks: i16 = kani::any();
    let fs: i16 = kani::any();
    let den = a * POS_SCALE;
    let k_rem: u128 = if generic_a { kani::any::<u64>() as u128 } else { kani::any::<u32>() as u128 * ADL_ONE };
    let f_rem: u128 = if generic_a { kani::any::<u64>() as u128 } else { kani::any::<u32>() as u128 * ADL_ONE };
    let leg = PortfolioLegV16 {
        active: true,
        side: SideV16::Long,
        basis_pos_q: kani::any::<u16>() as i128 + 1,
        a_basis: a,
        k_snap: ks as i128 * ADL_ONE as i128,
        f_snap: fs as i128 * ADL_ONE as i128,
        k_rem_num: k_rem,
        f_rem_num: f_rem,
        epoch_snap: asset.epoch_long,
        loss_weight: 1,
        ..PortfolioLegV16::EMPTY
    };
    let r = View::kani_v22_leg_kf_components(asset, leg);
    if k_rem >= den || f_rem >= den {
        assert_eq!(r, Err(percolator::v16::V16Error::InvalidLeg));
    }
    if let Ok((_, _, _, _, kr, fr, _)) = r {
        assert!(kr < den && fr < den, "rem < den preserved");
    }
    kani::cover!(k_rem == den - ADL_ONE || (generic_a && k_rem == den - 1), "rem at den - 1 step");
    kani::cover!(k_rem >= den, "rem == den refused");
    kani::cover!(r.is_ok() && kn != ks, "settles a non-zero delta");
}

/// E-REM-3 on the fast-path domain (A == ADL_ONE). Class M. Mutant REM-M4.
#[kani::proof]
#[kani::solver(cadical)]
fn proof_v22_rem_settle_keeps_rem_below_den() {
    rem_body(false);
}

/// E-REM-3 generic-A twin (wide path). Class L (heavy lane).
#[kani::proof]
#[kani::solver(cadical)]
fn proof_v22_rem_settle_keeps_rem_below_den_generic_a() {
    rem_body(true);
}

/// E-REM-4 attach zeroes both remainders (both sides). Mutant REM-M5. (Resize half: in
/// `proof_v22_attached_legs_have_unit_a`.)
#[kani::proof]
#[kani::solver(cadical)]
fn proof_v22_rem_attach_resize() {
    let mut asset = AssetStateV16::default();
    asset.a_long = ADL_ONE;
    asset.a_short = ADL_ONE;
    let short: bool = kani::any();
    let side = if short { SideV16::Short } else { SideV16::Long };
    let basis = kani::any::<u32>() as i128 + 1;
    let r = View::kani_v22_attach_leg(asset, side, if short { -basis } else { basis }, basis as u128, 0, 0, 0);
    if let Ok((_, leg)) = r {
        assert!(leg.k_rem_num == 0 && leg.f_rem_num == 0);
    }
    kani::cover!(r.is_ok() && short, "attach short");
    kani::cover!(r.is_ok() && !short, "attach long");
}

/// E-REM-5: `is_empty_encoding` implies `try_to_runtime() == Ok(EMPTY)`, over 217 symbolic bytes.
#[kani::proof]
#[kani::solver(cadical)]
fn proof_v22_empty_leg_encoding() {
    let bytes: [u8; core::mem::size_of::<PortfolioLegV16Account>()] = kani::any();
    let leg: PortfolioLegV16Account = bytemuck::pod_read_unaligned(&bytes);
    let e = leg.is_empty_encoding();
    if e {
        assert_eq!(leg.try_to_runtime(), Ok(PortfolioLegV16::EMPTY));
    }
    kani::cover!(e, "empty encoding");
    kani::cover!(!e, "non-empty encoding");
}

/// E-REM-6 (#277) `kernel_kf_hidden_loss_bound` (`:1029`): exact formula including the
/// `2 * stale` rounding term; S1 fail-closed. 2 stale legs, u16 weights, u8 drift. Mutant REM-M6.
#[kani::proof]
#[kani::solver(cadical)]
fn proof_v22_kf_hidden_loss_bound() {
    let stale: u8 = kani::any();
    kani::assume(stale <= 2);
    let d = KfDriftSideV16 {
        gen_epoch: 0,
        laggard_count: kani::any::<u8>() as u64 % 3,
        drift_gen: kani::any::<u8>() as u128,
        drift_prior: kani::any::<u8>() as u128,
        stale_weight: kani::any::<u16>() as u128,
        laggard_weight: kani::any::<u16>() as u128,
    };
    let r = View::kani_v22_kf_hidden_loss_bound(stale as u64, d);
    let den = SOCIAL_WEIGHT_SCALE * POS_SCALE;
    let ceil = |a: u128, b: u128| (a * b + den - 1) / den;
    if stale == 0 {
        assert_eq!(r, Some(0));
    } else if d.stale_weight < stale as u128 {
        assert_eq!(r, None, "S1 fail-closed");
    } else {
        let prior = if d.laggard_count == 0 { 0 } else { ceil(d.laggard_weight, d.drift_prior) };
        assert_eq!(r, Some(ceil(d.stale_weight, d.drift_gen) + prior + 2 * stale as u128));
    }
    kani::cover!(stale == 2 && r.is_some(), "2-leg rounding witness");
    kani::cover!(stale > 0 && r.is_none(), "fail-closed");
    kani::cover!(stale > 0 && d.laggard_count > 0 && r.is_some(), "laggard term");
}

/// E-REM-6, semantic half (review M9): for a 2-leg cohort the bound is >= the TRUE hidden loss.
/// Label (round 2): BOUNDED, K-only (dF = 0), gen-term only (no laggard / prior term): it exercises
/// one of the two floor atoms per leg that the `+2*stale` term pays for.
/// Two stale legs (unit A, so loss_weight == abs basis) with their own carried remainders take an
/// adverse K move of `g` (a multiple of ADL_ONE: the fast path the production settle uses at unit
/// A); each leg's realised loss is the magnitude of the REAL `scaled_adl_delta_with_carry_fast`
/// quotient. The tracker that describes this cohort has stale_weight = w1 + w2, drift_gen = |dK|,
/// no laggards. Assert |q1| + |q2| <= kernel_kf_hidden_loss_bound(2, drift). Bounded: u16 bases,
/// u8 move, carries < DEN. Mutant REM-M6 is killed by the formula harness above; this harness
/// carries the "bound covers the loss" meaning.
#[kani::proof]
#[kani::solver(cadical)]
fn proof_v22_kf_hidden_loss_bound_covers_two_leg_loss() {
    let b1 = kani::any::<u16>() as u128 + 1;
    let b2 = kani::any::<u16>() as u128 + 1;
    let g = kani::any::<u8>() as i128;
    let c1 = kani::any::<u32>() as u128;
    let c2 = kani::any::<u32>() as u128;
    kani::assume(c1 < POS_SCALE && c2 < POS_SCALE);
    let dk = -(g * ADL_ONE as i128); // adverse K move for the long side
    let q1 = View::kani_v22_scaled_adl_delta_with_carry_fast(b1, ADL_ONE, 0, dk, c1 * ADL_ONE);
    let q2 = View::kani_v22_scaled_adl_delta_with_carry_fast(b2, ADL_ONE, 0, dk, c2 * ADL_ONE);
    kani::assume(q1.is_some() && q2.is_some());
    let loss = q1.unwrap().0.unsigned_abs() + q2.unwrap().0.unsigned_abs();
    let d = KfDriftSideV16 {
        gen_epoch: 0,
        laggard_count: 0,
        drift_gen: (g as u128) * ADL_ONE,
        drift_prior: 0,
        stale_weight: b1 + b2,
        laggard_weight: 0,
    };
    let r = View::kani_v22_kf_hidden_loss_bound(2, d);
    assert!(r.is_some());
    assert!(loss <= r.unwrap(), "the bound covers the cohort's true hidden loss");
    kani::cover!(loss > 0, "a real loss");
    kani::cover!(loss > 0 && r.unwrap() - loss <= 4, "bound within its 2*stale slack of the loss (tight)");
}


/// E-REM-5b (r2, memcmp unwind fix, option B): under cfg(kani) `is_empty_encoding` defaults to a
/// field-wise compare (`percolator::v16::kani_v22_is_empty_encoding_fieldwise`); this proves it equals the
/// PRODUCTION byte compare over all 217 symbolic bytes, in both directions. The production path is the
/// unchanged body of `PortfolioLegV16Account::is_empty_encoding`, taken with the proof-only switch on
/// (`kani_v22_set_leg_byte_compare(true)`); then the switch is turned off and the same leg is tested again.
/// Also: the raw 217-byte equality equals the derived field-wise `==` (no padding, no hidden bytes).
/// #[kani::unwind(218)]: the 217-byte memcmp plus its exit test; no other loop. Cost S.
#[kani::proof]
#[kani::unwind(218)]
#[kani::solver(cadical)]
fn proof_v22_empty_leg_bytes_eq_fieldwise() {
    use percolator::v16::{kani_v22_is_empty_encoding_fieldwise, kani_v22_set_leg_byte_compare, PORTFOLIO_LEG_V16_EMPTY_ACCOUNT};
    let bytes: [u8; core::mem::size_of::<PortfolioLegV16Account>()] = kani::any();
    let leg: PortfolioLegV16Account = bytemuck::pod_read_unaligned(&bytes);
    kani_v22_set_leg_byte_compare(true);
    let prod = leg.is_empty_encoding(); // production: active == 0 && bytes_of(leg) == bytes_of(EMPTY)
    kani_v22_set_leg_byte_compare(false);
    let shim = leg.is_empty_encoding(); // proof-only default: field-wise
    assert_eq!(shim, kani_v22_is_empty_encoding_fieldwise(&leg));
    assert!(!prod || shim, "byte-equal => field-wise equal");
    assert!(!shim || prod, "field-wise equal => byte-equal");
    let raw_eq = bytes[..] == *bytemuck::bytes_of(&PORTFOLIO_LEG_V16_EMPTY_ACCOUNT);
    assert_eq!(raw_eq, leg == PORTFOLIO_LEG_V16_EMPTY_ACCOUNT, "217-byte equality == derived field-wise ==");
    kani::cover!(prod && shim, "equal (the empty encoding)");
    kani::cover!(!prod && !shim, "unequal");
    kani::cover!(leg.active == 0 && !prod, "active == 0 but some other byte differs");
}
