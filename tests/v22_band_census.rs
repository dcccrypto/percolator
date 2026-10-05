//! v2.2 band census (design §12 top risk 1): EVERY position-changing path keeps the
//! certification cohorts exact, and every health-certificate write reaches the
//! certification hook.
//!
//! Two layers:
//! 1. **Structural census** over `src/v16.rs`: every certificate write is followed by
//!    `band_observe_cert_not_atomic`; side position counts change only inside the two
//!    kernels that carry the band attach/detach hooks; a leg is emptied only after
//!    `kernel_clear_leg`; the anchor advances only inside `band_prepare_accrual`.
//!    A future edit that adds a bypass fails here before it can ship.
//! 2. **Dynamic census**: drive every position-changing engine entry point on a band
//!    market and check the cohorts against the leg census after each one (I-B2), and
//!    that the C-event paths (risk-increasing trade, healthy partial-liquidation
//!    remainder, healthy refresh) actually certify.
//!
//! Negative controls: deleting any one hook makes layer 1 fail (and usually layer 2).

use percolator::POS_SCALE;
use percolator::{
    AdlWindDownBoundV16, AdlWindDownRequestV16, EngineAssetSlotV16Account, LiquidationRequestV16,
    Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, PortfolioAccountV16Account,
    PortfolioV16ViewMut, ProvenanceHeaderV16, ProvenanceHeaderV16Account, RebalanceRequestV16,
    SideV16, TradeRequestV16, V16Config, V16Error,
};

const SRC: &str = include_str!("../src/v16.rs");

fn fn_body<'a>(src: &'a str, signature: &str) -> &'a str {
    let start = src
        .find(signature)
        .unwrap_or_else(|| panic!("function not found: {signature}"));
    let open = start + src[start..].find('{').unwrap();
    let mut depth = 0usize;
    for (k, c) in src[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &src[open..open + k + 1];
                }
            }
            _ => {}
        }
    }
    panic!("unbalanced body for {signature}");
}

/// Strips every in-crate `#[cfg(test)] mod NAME { .. }` block so test fixtures
/// that poke engine state directly do not count as production sites.
fn production_src() -> &'static str {
    let mut out = String::with_capacity(SRC.len());
    let mut rest = SRC;
    while let Some(at) = rest.find("#[cfg(test)]\nmod ") {
        out.push_str(&rest[..at]);
        let body = fn_body(&rest[at..], "mod ");
        let end = at + rest[at..].find(body).unwrap() + body.len();
        rest = &rest[end..];
    }
    out.push_str(rest);
    Box::leak(out.into_boxed_str())
}

#[test]
fn census_every_certificate_write_reaches_the_band_hook() {
    let src = production_src();
    let needle = "account.header.health_cert = HealthCertV16Account::from_runtime(&cert);";
    let mut sites = 0;
    let mut unhooked = Vec::new();
    let mut from = 0;
    while let Some(off) = src[from..].find(needle) {
        let at = from + off;
        sites += 1;
        // The hook must follow within the next few lines (before any return).
        let window_end = src[at..]
            .match_indices('\n')
            .nth(5)
            .map(|(k, _)| at + k)
            .unwrap_or(src.len());
        let window = &src[at..window_end];
        if !window.contains("band_observe_cert_not_atomic(") {
            let line = src[..at].matches('\n').count() + 1;
            unhooked.push(line);
        }
        from = at + needle.len();
    }
    assert_eq!(
        sites, 6,
        "the set of certificate-write sites changed: re-audit the band hook census"
    );
    assert!(
        unhooked.is_empty(),
        "certificate writes without the band hook at lines {unhooked:?}"
    );
}

#[test]
fn census_side_position_counts_change_only_inside_the_hooked_kernels() {
    let src = production_src();
    let mut writers = Vec::new();
    for side in ["long", "short"] {
        let pat = format!("asset.stored_pos_count_{side} = asset");
        let mut from = 0;
        while let Some(off) = src[from..].find(&pat) {
            let at = from + off;
            // Name the enclosing fn.
            let head = &src[..at];
            let fn_at = head.rfind("fn ").unwrap();
            let name: String = head[fn_at + 3..]
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            writers.push(name);
            from = at + pat.len();
        }
    }
    writers.sort();
    writers.dedup();
    assert_eq!(
        writers,
        vec![
            "add_open_interest_for_new_position".to_string(),
            "kernel_clear_leg".to_string()
        ],
        "stored position counts must change only in the attach/clear kernels"
    );
    // add_open_interest_for_new_position is reached only through kernel_attach_leg
    // (and a #[cfg(kani)] shim that forwards to it for the proofs).
    let mut callers = Vec::new();
    for (at, _) in src.match_indices("add_open_interest_for_new_position(") {
        let head = &src[..at];
        if head.ends_with("fn ") || head.ends_with("fn kani_") {
            continue; // a definition, not a call
        }
        let fn_at = head.rfind("fn ").unwrap();
        let name: String = head[fn_at + 3..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        callers.push(name);
    }
    callers.sort();
    assert_eq!(
        callers,
        vec![
            "kani_add_open_interest_for_new_position".to_string(),
            "kernel_attach_leg".to_string()
        ]
    );
    let attach = fn_body(src, "pub(crate) fn kernel_attach_leg(");
    assert!(attach.contains("add_open_interest_for_new_position("));
    assert!(
        attach.contains("kernel_band_attach("),
        "attach kernel lost the band hook"
    );
    assert!(
        attach.contains("rent_snap"),
        "attach kernel lost the rent snapshot"
    );
    let clear = fn_body(src, "pub(crate) fn kernel_clear_leg(");
    assert!(
        clear.contains("kernel_band_detach("),
        "clear kernel lost the band hook"
    );
}

#[test]
fn census_legs_are_emptied_only_after_the_clear_kernel() {
    let src = production_src();
    let needle = "PortfolioLegV16Account::from_runtime(&PortfolioLegV16::EMPTY)";
    let mut owners = Vec::new();
    let mut from = 0;
    while let Some(off) = src[from..].find(needle) {
        let at = from + off;
        let head = &src[..at];
        let fn_at = head.rfind("fn ").unwrap();
        let name: String = head[fn_at + 3..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        owners.push(name);
        from = at + needle.len();
    }
    owners.sort();
    assert_eq!(
        owners,
        vec![
            "clear_leg_at_slot_inner".to_string(),
            "init_empty_in_place".to_string()
        ],
        "a leg may be emptied only by the clear path (or account init)"
    );
    let clear = fn_body(src, "fn clear_leg_at_slot_inner(");
    let k = clear
        .find("V16Core::kernel_clear_leg(")
        .expect("clear path calls the kernel");
    let e = clear.find("PortfolioLegV16::EMPTY").unwrap();
    assert!(
        k < e,
        "the kernel (with the detach hook) runs before the leg is emptied"
    );
}

#[test]
fn census_anchor_advances_only_in_the_accrual_gate() {
    let src = production_src();
    let calls: Vec<_> = src
        .match_indices("V16Core::kernel_band_reanchor(")
        .collect();
    assert_eq!(calls.len(), 1, "exactly one re-anchor site");
    let gate = fn_body(src, "fn band_prepare_accrual(");
    assert!(gate.contains("V16Core::kernel_band_reanchor("));
    assert!(gate.contains("kernel_band_reanchor_ready("));
    // Both accrual entry points run the gate before mutating.
    for entry in [
        "pub fn accrue_asset_to_with_rent_not_atomic(",
        "pub fn accrue_asset_path_with_rent_to_not_atomic(",
    ] {
        let body = fn_body(src, entry);
        let g = body
            .find("band_prepare_accrual(")
            .expect("entry runs the gate");
        let m = body.find("add_non_min_i128(").expect("entry mutates K");
        assert!(
            g < m,
            "{entry}: the band gate must run before any K/F mutation"
        );
    }
    // The legacy entry points delegate (no third, ungated accrual path).
    assert!(fn_body(src, "pub fn accrue_asset_to_not_atomic(")
        .contains("accrue_asset_to_with_rent_not_atomic("));
    assert!(fn_body(src, "pub fn accrue_asset_path_to_not_atomic(")
        .contains("accrue_asset_path_with_rent_to_not_atomic("));
    let k_writers = src.matches("asset.k_long = add_non_min_i128(").count();
    assert_eq!(k_writers, 2, "K moves only in the two gated accrual bodies");
}

// ---------------------------------------------------------------------------
// Dynamic census over every position-changing entry point
// ---------------------------------------------------------------------------

const P0: u64 = 1_000_000;
const MARKET_ID: [u8; 32] = [7; 32];

fn band_cfg() -> V16Config {
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
    c.max_bankrupt_close_lifetime_slots = 1_000_000;
    c.rent_max_e9_per_slot = 23;
    c.band_bps = 130;
    c.band_max_epoch_slots = 600;
    c.band_max_pin_slots = 9_000;
    c
}

struct W {
    header: MarketGroupV16HeaderAccount,
    markets: Vec<Market<u64>>,
    accts: Vec<PortfolioAccountV16Account>,
    now: u64,
}

impl W {
    fn new(n: usize, deposit: u128) -> Self {
        let mut header =
            MarketGroupV16HeaderAccount::new_dynamic(MARKET_ID, band_cfg(), 1, 0).unwrap();
        let mut markets = vec![Market::new(0, EngineAssetSlotV16Account::default())];
        header
            .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, P0, 1)
            .unwrap();
        let mut accts = Vec::new();
        for i in 0..n {
            let prov = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
                MARKET_ID,
                [i as u8 + 1; 32],
                [9; 32],
            ));
            let mut a = PortfolioAccountV16Account::default();
            a.init_empty_in_place(prov).unwrap();
            accts.push(a);
        }
        let mut w = Self {
            header,
            markets,
            accts,
            now: 1,
        };
        for i in 0..n {
            w.one(i, |m, a| m.deposit_not_atomic(a, deposit)).unwrap();
        }
        w
    }
    fn one<R>(
        &mut self,
        i: usize,
        f: impl FnOnce(&mut MarketGroupV16ViewMut<'_, u64>, &mut PortfolioV16ViewMut<'_>) -> R,
    ) -> R {
        let mut m = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let mut a = PortfolioV16ViewMut::new(&mut self.accts[i]);
        f(&mut m, &mut a)
    }
    fn two<R>(
        &mut self,
        i: usize,
        j: usize,
        f: impl FnOnce(
            &mut MarketGroupV16ViewMut<'_, u64>,
            &mut PortfolioV16ViewMut<'_>,
            &mut PortfolioV16ViewMut<'_>,
        ) -> R,
    ) -> R {
        assert_ne!(i, j);
        let (lo, hi) = (i.min(j), i.max(j));
        let (l, r) = self.accts.split_at_mut(hi);
        let (x, y) = (&mut l[lo], &mut r[0]);
        let (ai, aj) = if i < j { (x, y) } else { (y, x) };
        let mut m = MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets);
        let mut vi = PortfolioV16ViewMut::new(ai);
        let mut vj = PortfolioV16ViewMut::new(aj);
        f(&mut m, &mut vi, &mut vj)
    }
    fn asset(&self) -> percolator::AssetStateV16 {
        self.markets[0].engine.asset.try_to_runtime().unwrap()
    }
    fn trade(&mut self, long: usize, short: usize, q: i128) -> Result<(), V16Error> {
        let p = self.asset().effective_price;
        self.two(long, short, |m, l, s| {
            m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
                l,
                s,
                TradeRequestV16 {
                    asset_index: 0,
                    size_q: q,
                    exec_price: p,
                    fee_bps: 0,
                },
                true,
            )
            .map(|_| ())
        })
    }
    fn refresh(&mut self, i: usize) -> Result<percolator::HealthCertV16, V16Error> {
        let mut last = Err(V16Error::Stale);
        for _ in 0..8 {
            last = self.one(i, |m, a| m.full_account_refresh_not_atomic(a));
            if last != Err(V16Error::Stale) {
                break;
            }
        }
        last
    }
    fn accrue_to(&mut self, price: u64) {
        let now = self.now;
        self.one(0, |m, _| {
            m.accrue_asset_to_not_atomic(0, now, price, 0, true)
        })
        .unwrap();
    }
    fn leg(&self, i: usize) -> percolator::PortfolioLegV16 {
        self.accts[i].legs[0].try_to_runtime().unwrap()
    }
    fn census(&self, what: &str) {
        let a = self.asset();
        let mut c = [0u64; 6];
        for acct in &self.accts {
            let leg = acct.legs[0].try_to_runtime().unwrap();
            if !leg.active {
                continue;
            }
            let s = usize::from(leg.side == SideV16::Short);
            c[s] += 1;
            if a.band_epoch != 0 && leg.band_epoch_snap < a.band_epoch {
                c[2 + s] += 1;
            }
            if leg.band_liq_pending {
                c[4 + s] += 1;
            }
        }
        assert_eq!(
            [
                a.stored_pos_count_long,
                a.stored_pos_count_short,
                a.band_uncertified_long,
                a.band_uncertified_short,
                a.band_liq_pending_long,
                a.band_liq_pending_short
            ],
            c,
            "census after {what}"
        );
    }
    fn certified(&self, i: usize) -> bool {
        let leg = self.leg(i);
        leg.active && leg.band_epoch_snap == self.asset().band_epoch
    }
}

/// Puts the book into a fresh epoch with every leg UNcertified, so each path's
/// own certification effect is observable.
fn fresh_epoch(w: &mut W) {
    w.now += 1;
    let p = w.asset().effective_price;
    for i in 0..w.accts.len() {
        if w.leg(i).active {
            w.refresh(i).unwrap();
        }
    }
    w.accrue_to(p);
}

#[test]
fn census_every_trade_route_attach_resize_flip_clear_batch() {
    let mut w = W::new(6, 10_000_000);
    // Attach (open from flat): C-event (b).
    w.trade(0, 1, (5 * POS_SCALE) as i128).unwrap();
    w.census("attach");
    assert!(
        w.certified(0) && w.certified(1),
        "an IM-approved open certifies"
    );

    fresh_epoch(&mut w);
    assert!(!w.certified(0));
    // Resize up (risk-increasing): certifies the grown leg.
    w.trade(0, 2, POS_SCALE as i128).unwrap();
    w.census("resize up");
    assert!(w.certified(0), "a risk-increasing resize certifies (b)");

    fresh_epoch(&mut w);
    // Resize down (reduce): the post-fill certificate is a fresh health statement,
    // so a healthy reducer is certified too.
    w.trade(3, 0, POS_SCALE as i128).unwrap();
    w.census("resize down");
    assert!(w.certified(0));

    fresh_epoch(&mut w);
    // Flip (clear + attach on the other side).
    w.trade(4, 0, (10 * POS_SCALE) as i128).unwrap();
    w.census("flip");
    assert_eq!(w.leg(0).side, SideV16::Short);
    assert!(w.certified(0));

    fresh_epoch(&mut w);
    // Close to flat (clear): the leg leaves every cohort.
    let q = w.leg(1).basis_pos_q.unsigned_abs() as i128;
    w.trade(1, 5, q).unwrap();
    assert!(!w.leg(1).active);
    w.census("clear");

    fresh_epoch(&mut w);
    // Batch: 5 (short 5) reduces, 1 (flat) attaches short, in one instruction.
    let p = w.asset().effective_price;
    w.two(5, 1, |m, l, s| {
        m.execute_batch_with_fee_loss_stale_scoped_not_atomic(
            l,
            s,
            &[TradeRequestV16 {
                asset_index: 0,
                size_q: POS_SCALE as i128,
                exec_price: p,
                fee_bps: 0,
            }],
            true,
        )
    })
    .unwrap();
    w.census("batch");
    assert!(
        w.certified(5) && w.certified(1),
        "batch fills certify both accounts"
    );
}

#[test]
fn census_liquidation_partial_full_rebalance_and_adl_wind_down() {
    let mut w = W::new(6, 2_000_000);
    w.trade(0, 1, (18 * POS_SCALE) as i128).unwrap(); // near-IM long
    w.trade(2, 3, (18 * POS_SCALE) as i128).unwrap(); // near-IM long
    w.trade(5, 4, POS_SCALE as i128).unwrap();
    // Walk the price down, certifying everyone each epoch, until 0 is liquidatable.
    let mut partial_seen = false;
    let mut full_seen = false;
    for _ in 0..600 {
        w.now += 3;
        let a = w.asset();
        let (lo, _) = percolator::band_rent::band_bounds(a.band_anchor_price, 130).unwrap();
        let next = (a.effective_price - a.effective_price / 100).max(lo);
        w.accrue_to(next.max(1));
        for i in 0..6 {
            if !w.leg(i).active {
                continue;
            }
            let cert = w.refresh(i).unwrap();
            w.census("refresh");
            if cert.certified_liq_deficit != 0 {
                assert!(
                    w.leg(i).band_liq_pending,
                    "unhealthy refresh marks liq-pending"
                );
                let before = w.leg(i).basis_pos_q.unsigned_abs();
                let out = w
                    .one(i, |m, a| {
                        m.liquidate_account_not_atomic(a, LiquidationRequestV16 { asset_index: 0 })
                    })
                    .unwrap();
                assert_eq!(
                    (out.insurance_used, out.residual_booked, out.explicit_loss),
                    (0, 0, 0)
                );
                w.census("liquidation");
                if w.leg(i).active {
                    assert!(w.leg(i).basis_pos_q.unsigned_abs() < before);
                    partial_seen = true;
                    // A healthy partial remainder is certified (c) and no longer pending.
                    if w.accts[i]
                        .health_cert
                        .try_to_runtime()
                        .unwrap()
                        .certified_liq_deficit
                        == 0
                    {
                        assert!(w.certified(i) && !w.leg(i).band_liq_pending);
                    }
                } else {
                    full_seen = true;
                }
            }
        }
        if partial_seen && full_seen {
            break;
        }
        if !w.leg(0).active && !w.leg(2).active {
            break;
        }
    }
    assert!(
        partial_seen || full_seen,
        "non-vacuity: a liquidation happened"
    );

    // Owner rebalance (unilateral reduce) on whatever short remains.
    fresh_epoch(&mut w);
    if w.leg(4).active {
        let r = w.one(4, |m, a| {
            m.rebalance_reduce_position_not_atomic(
                a,
                RebalanceRequestV16 {
                    asset_index: 0,
                    reduce_q: POS_SCALE / 2,
                },
            )
        });
        assert!(r.is_ok(), "rebalance reduce: {r:?}");
        w.census("rebalance reduce");
    }

    // ADL wind-down, when the unilateral closes left the book eligible. A forced
    // close never runs at a lagging mark (review M-1): align the target first.
    let p = w.asset().effective_price;
    w.one(0, |m, _| m.set_asset_raw_oracle_target_not_atomic(0, p))
        .unwrap();
    let eligible = w.one(0, |m, _| {
        m.adl_wind_down_eligible(
            0,
            AdlWindDownBoundV16::DustNotional {
                max_notional_atoms: u128::MAX,
            },
        )
    });
    assert_eq!(
        eligible,
        Ok(true),
        "non-vacuity: the unilateral closes left an ADL book"
    );
    let mut wound_down = 0;
    {
        for i in 0..6 {
            if w.leg(i).active {
                let r = w.one(i, |m, a| {
                    m.wind_down_adl_position_not_atomic(
                        a,
                        AdlWindDownRequestV16 {
                            asset_index: 0,
                            bound: AdlWindDownBoundV16::DustNotional {
                                max_notional_atoms: u128::MAX,
                            },
                        },
                    )
                });
                wound_down += usize::from(r.is_ok());
                w.census("adl wind-down");
            }
        }
    }
    assert!(wound_down > 0, "non-vacuity: an ADL wind-down landed");
}

#[test]
fn census_recovery_and_resolved_close_paths() {
    let mut w = W::new(4, 10_000_000);
    w.trade(0, 1, (5 * POS_SCALE) as i128).unwrap();
    w.trade(2, 3, (5 * POS_SCALE) as i128).unwrap();
    let now = w.now + 1;
    w.now = now;
    w.one(0, |m, _| m.force_asset_recovery_not_atomic(0, now))
        .unwrap();
    // Recovery pair close: two opposite legs close against each other.
    let closed = w.two(0, 1, |m, a, b| {
        m.force_close_recovery_pair_not_atomic(a, b, 0, 5 * POS_SCALE)
    });
    assert!(closed.is_ok(), "recovery pair close: {closed:?}");
    w.census("recovery pair close");
    // Forfeit / resolved close of whatever remains.
    let mut forfeited = 0;
    for i in 0..4 {
        if w.leg(i).active {
            forfeited += usize::from(
                w.one(i, |m, a| m.forfeit_recovery_leg_not_atomic(a, 0, u128::MAX))
                    .is_ok(),
            );
            w.census("recovery forfeit");
        }
    }
    assert!(forfeited > 0, "non-vacuity: a recovery forfeit landed");
    let now = w.now + 1;
    w.one(0, |m, _| m.resolve_market_not_atomic(now)).unwrap();
    let mut closes = 0;
    for i in 0..4 {
        for _ in 0..8 {
            if w.one(i, |m, a| m.close_resolved_account_not_atomic(a, 0))
                .is_err()
            {
                break;
            }
            closes += 1;
            w.census("resolved close");
        }
    }
    assert!(closes > 0, "non-vacuity: resolved closes landed");
    for i in 0..4 {
        assert!(
            !w.leg(i).active,
            "every leg detached through the census-checked paths"
        );
    }
}
