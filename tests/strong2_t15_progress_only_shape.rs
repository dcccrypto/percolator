//! STRONG2 / T15 refutation attempts for `close_resolved_account_not_atomic` (src/v16.rs:26580).
//!
//! Ledger rows under test (as literally stated):
//!   T15.header  : validate_shape() Ok before && call returns Ok  ==>  validate_shape() Ok after.
//!   T15.account : account.validate_with_market() Ok before && call returns Ok ==> Ok after.
//!
//! WHICH PREDICATE: `MarketGroupV16View::validate_shape` (src/v16.rs:9996) additionally runs
//! `validate_shape_full_audit_scan` under `cfg(any(test, kani, feature = "audit-scan"))`. This is
//! an integration test, so the library is compiled WITHOUT cfg(test). Built without
//! `--features audit-scan` the predicate evaluated here is the deployed header-only one. Built
//! with `--features audit-scan` the engine re-validates inside the callees
//! (`validate_shape_audit_scan` / `validate_account_audit_scan`, src/v16.rs:10438/10450), so the
//! same inputs are expected to be rejected with Err; each test prints which build it ran under.
//!
//! The pre-states are built with public engine entries plus the direct field pokes listed in each
//! test. They are statement-precision witnesses (the stated hypotheses do not imply the stated
//! conclusion); no claim is made that the poked pre-states are reachable through the wrapper.
//!
//! Run:
//!   cargo test -j 4 --test strong2_t15_progress_only_shape -- --nocapture --test-threads=1
#![allow(dead_code)]

use percolator::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, ResolvedCloseOutcomeV16, TradeRequestV16, V16Config, V16Error,
    V16PodI128, V16PodU128, V16PodU64, BOUND_SCALE, POS_SCALE, SOCIAL_LOSS_DEN,
};

const PRICE: u64 = 1_000_000;
const AUDIT_SCAN: bool = cfg!(feature = "audit-scan");

fn account(seed: u32) -> PortfolioAccountV16Account {
    let mut key = [0u8; 32];
    key[..4].copy_from_slice(&seed.to_le_bytes());
    key[31] = 0x5A;
    let header =
        ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new([1; 32], key, [3; 32]));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(header).unwrap();
    a
}

/// Fixture shape copied from `tests/resolved_payout_edge.rs` (`W`).
#[derive(Clone)]
struct W {
    header: MarketGroupV16HeaderAccount,
    markets: Vec<Market<u64>>,
    slot: u64,
}

impl W {
    fn new(n: usize, public_b_chunk_atoms: Option<u128>) -> Self {
        let mut cfg =
            V16Config::public_user_fund_with_market_slots(n as u16, n as u32, 0, 6_480_000);
        cfg.max_abs_funding_e9_per_slot = 10_000;
        cfg.max_price_move_bps_per_slot = 200;
        cfg.initial_margin_bps = 1_000;
        cfg.maintenance_margin_bps = 500;
        cfg.max_accrual_dt_slots = 2;
        cfg.min_funding_lifetime_slots = 10_000_000;
        if let Some(v) = public_b_chunk_atoms {
            cfg.public_b_chunk_atoms = v;
        }
        let mut header =
            MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, n as u32, 0).unwrap();
        let mut markets: Vec<Market<u64>> = (0..n as u64)
            .map(|i| Market::new(i, EngineAssetSlotV16Account::default()))
            .collect();
        for i in 0..n {
            header
                .activate_empty_asset_slot_not_atomic(
                    i as u32,
                    &mut markets[i].engine,
                    PRICE,
                    1 + 2 * i as u64,
                )
                .unwrap();
        }
        W { header, markets, slot: 2 * n as u64 - 1 }
    }
    fn view(&mut self) -> MarketGroupV16ViewMut<'_, u64> {
        MarketGroupV16ViewMut::new(&mut self.header, &mut self.markets)
    }
    fn deposit(&mut self, a: &mut PortfolioAccountV16Account, amt: u128) {
        self.view()
            .deposit_not_atomic(&mut PortfolioV16ViewMut::new(a), amt)
            .unwrap();
    }
    fn trade(
        &mut self,
        asset: u32,
        long: &mut PortfolioAccountV16Account,
        short: &mut PortfolioAccountV16Account,
        size: i128,
    ) {
        let p = self.markets[asset as usize].engine.asset.effective_price.get();
        let mut m = self.view();
        m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut PortfolioV16ViewMut::new(long),
            &mut PortfolioV16ViewMut::new(short),
            TradeRequestV16 { asset_index: asset as usize, size_q: size, exec_price: p, fee_bps: 0 },
            true,
        )
        .unwrap();
    }
    /// Move asset `asset`'s price by `bps` through the public accrual entry; other assets are
    /// accrued to the same slot at an unchanged price.
    fn move_price(&mut self, asset: usize, bps: i64) {
        let steps = (bps.abs() + 149) / 150;
        let mut left = bps;
        let n = self.markets.len();
        for _ in 0..steps {
            let d = left.clamp(-150, 150);
            left -= d;
            self.slot += 1;
            let slot = self.slot;
            for i in 0..n {
                let old = self.markets[i].engine.asset.effective_price.get() as i128;
                let new = if i == asset { (old + old * d as i128 / 10_000) as u64 } else { old as u64 };
                let mut m = self.view();
                m.accrue_asset_to_not_atomic(i, slot, new, 0, true).expect("accrue");
                m.markets[i].engine.asset.raw_oracle_target_price = V16PodU64::new(new);
            }
        }
    }
    fn resolve(&mut self) {
        let slot = self.slot;
        self.view().resolve_market_not_atomic(slot).unwrap();
    }
    fn shape(&mut self) -> Result<(), V16Error> {
        self.view().validate_shape()
    }
    fn acct_ok(&mut self, a: &mut PortfolioAccountV16Account) -> Result<(), V16Error> {
        let m = self.view();
        let v = PortfolioV16ViewMut::new(a);
        v.validate_with_market(&m.as_view())
    }
    fn close(
        &mut self,
        a: &mut PortfolioAccountV16Account,
    ) -> Result<ResolvedCloseOutcomeV16, V16Error> {
        self.view().close_resolved_account_not_atomic(&mut PortfolioV16ViewMut::new(a), 0)
    }
    /// Public fee sync at the resolved slot with a zero rate: in Resolved mode this does not
    /// settle K/F; it only stamps `last_fee_slot = resolved_slot`.
    fn fee_sync_at_resolved_slot(&mut self, a: &mut PortfolioAccountV16Account) {
        let rs = self.header.resolved_slot.get();
        self.view()
            .sync_account_fee_to_slot_not_atomic(&mut PortfolioV16ViewMut::new(a), rs, 0)
            .unwrap();
        assert_eq!(a.last_fee_slot.get(), rs);
    }
}

fn bitmap_empty(a: &PortfolioAccountV16Account) -> bool {
    a.active_bitmap.iter().all(|w| w.get() == 0)
}

fn dump_header(tag: &str, h: &MarketGroupV16HeaderAccount) {
    println!(
        "  [{tag}] vault={} c_tot={} insurance={} pnl_pos_tot={} pnl_matured_pos_tot={} \
         pnl_pos_bound_tot={} pnl_pos_bound_tot_num={} source_claim_bound_total_num={} \
         negative_pnl_account_count={} resolved_payout_blocker_count={}",
        h.vault.get(),
        h.c_tot.get(),
        h.insurance.get(),
        h.pnl_pos_tot.get(),
        h.pnl_matured_pos_tot.get(),
        h.pnl_pos_bound_tot.get(),
        h.pnl_pos_bound_tot_num.get(),
        h.source_claim_bound_total_num.get(),
        h.negative_pnl_account_count.get(),
        h.resolved_payout_blocker_count.get(),
    );
}

/// Pokes for the header witness (all on public Pod fields):
///   account: `pnl = +x` (no source-domain claims, which `validate_with_market` accepts:
///            the attribution check at src/v16.rs:6505 only runs when the claim sum is nonzero);
///   header : `pnl_pos_tot += x`, `pnl_pos_bound_tot_num += x*BOUND_SCALE`,
///            `pnl_pos_bound_tot` recomputed (so the junior aggregates agree with the account),
///            `source_claim_bound_total_num = pnl_pos_bound_tot_num` (the aggregate-clause
///            boundary `pnl_pos_bound_tot_num >= source_claim_bound_total_num`, src/v16.rs:10106).
fn poke_unattributed_positive_pnl_with_tight_bound(
    w: &mut W,
    a: &mut PortfolioAccountV16Account,
    x: u128,
) {
    assert_eq!(a.pnl.get(), 0);
    a.pnl = V16PodI128::new(x as i128);
    w.header.pnl_pos_tot = V16PodU128::new(w.header.pnl_pos_tot.get() + x);
    let num = w.header.pnl_pos_bound_tot_num.get() + x * BOUND_SCALE;
    w.header.pnl_pos_bound_tot_num = V16PodU128::new(num);
    w.header.pnl_pos_bound_tot = V16PodU128::new(num.div_ceil(BOUND_SCALE));
    w.header.source_claim_bound_total_num = V16PodU128::new(num);
}

fn run_header_witness(n_assets: usize, slot_claim: bool) {
    println!("build: audit-scan feature = {AUDIT_SCAN}; assets = {n_assets}");
    let dep = 1_000_000u128;
    let mut w = W::new(n_assets, None);
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, dep);
    w.deposit(&mut b, dep);
    for i in 0..n_assets {
        w.trade(i as u32, &mut a, &mut b, POS_SCALE as i128);
    }
    // A is long; asset 0 falls 3%. A is NOT refreshed, so its asset-0 leg carries an unsettled
    // K loss into the Resolved market.
    w.move_price(0, -300);
    w.resolve();
    w.fee_sync_at_resolved_slot(&mut a);
    poke_unattributed_positive_pnl_with_tight_bound(&mut w, &mut a, 100_000);
    if slot_claim {
        // Extra pokes (market slot 0, long source domain): give the header claim total a
        // matching per-domain claim so the pre-state is ALSO accepted by the full audit scan
        // (checked by running this test under `--features audit-scan`).
        let s = w.header.source_claim_bound_total_num;
        w.markets[0].engine.source_credit_long.positive_claim_bound_num = s;
        w.markets[0].engine.source_credit_long.exact_positive_claim_num = s;
        println!("  slot poke: slot0.source_credit_long.{{positive_claim_bound_num,exact_positive_claim_num}} = {}", s.get());
    }

    let shape_before = w.shape();
    let acct_before = w.acct_ok(&mut a);
    dump_header("before", &w.header);
    println!(
        "  [before] account pnl={} capital={} reserved_pnl={} last_fee_slot={} resolved_slot={} \
         bitmap_empty={}",
        a.pnl.get(),
        a.capital.get(),
        a.reserved_pnl.get(),
        a.last_fee_slot.get(),
        w.header.resolved_slot.get(),
        bitmap_empty(&a)
    );
    println!("  [before] validate_shape={shape_before:?} validate_with_market={acct_before:?}");

    let outcome = w.close(&mut a);

    let shape_after = w.shape();
    let acct_after = w.acct_ok(&mut a);
    dump_header("after", &w.header);
    println!(
        "  [after] account pnl={} capital={} reserved_pnl={} bitmap_empty={} b_stale_state={} \
         stale_state={}",
        a.pnl.get(),
        a.capital.get(),
        a.reserved_pnl.get(),
        bitmap_empty(&a),
        a.b_stale_state,
        a.stale_state
    );
    println!("  [after] outcome={outcome:?}");
    println!("  [after] validate_shape={shape_after:?} validate_with_market={acct_after:?}");

    if AUDIT_SCAN {
        // Under audit-scan the header pokes alone (no matching slot claim) fail the full scan.
        println!("  audit-scan build: no deployed-predicate claim is made from this run");
        return;
    }

    // Hypotheses of T15.header (and of variant B: the account is valid too).
    assert_eq!(shape_before, Ok(()), "hypothesis: validate_shape Ok before");
    assert_eq!(acct_before, Ok(()), "variant B hypothesis: validate_with_market Ok before");
    // The call returns Ok(ProgressOnly).
    assert_eq!(outcome, Ok(ResolvedCloseOutcomeV16::ProgressOnly));
    // Conclusion of T15.header is violated.
    assert_eq!(shape_after, Err(V16Error::InvalidConfig), "validate_shape after");
    assert!(
        w.header.pnl_pos_bound_tot_num.get() < w.header.source_claim_bound_total_num.get(),
        "violated clause: src/v16.rs:10106 pnl_pos_bound_tot_num >= source_claim_bound_total_num"
    );
    // Which return: the post-state fails validate_shape, so it is not a return that follows a
    // successful validate_shape (26605, 26614, 26641, 26702); 26594 needs account source claims,
    // of which there are none. That leaves 26631 / 26634, decided by the condition at 26626-26629.
    assert!(a.source_domains.iter().all(|s| s.source_claim_bound_num.get() == 0));
    let cond_26626 = !bitmap_empty(&a) || a.pnl.get() < 0 || a.b_stale_state != 0 || a.stale_state != 0;
    println!(
        "  return taken: src/v16.rs:{}",
        if cond_26626 { 26631 } else { 26634 }
    );
    if n_assets == 1 {
        assert!(!cond_26626, "single leg detached: expected the 26634 return");
        assert!(a.pnl.get() > 0);
    } else {
        assert!(cond_26626, "a second leg is still attached: expected the 26631 return");
    }
}

/// T15.header, variant B, return at src/v16.rs:26634.
#[test]
fn t15_header_refuted_progress_only_26634_bound_below_source_claim_total() {
    run_header_witness(1, false);
}

/// T15.header, variant B, return at src/v16.rs:26631.
#[test]
fn t15_header_refuted_progress_only_26631_bound_below_source_claim_total() {
    run_header_witness(2, false);
}

/// Control: the same pokes WITHOUT the `last_fee_slot == resolved_slot` stamp. Then
/// `sync_account_fee_to_slot_not_atomic` does not take its early `Ok(0)` return
/// (src/v16.rs:25942) and reaches its own `validate_shape()` (src/v16.rs:25964).
#[test]
fn t15_header_control_without_fee_stamp_the_call_is_rejected() {
    println!("build: audit-scan feature = {AUDIT_SCAN}");
    let dep = 1_000_000u128;
    let mut w = W::new(1, None);
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, dep);
    w.deposit(&mut b, dep);
    w.trade(0, &mut a, &mut b, POS_SCALE as i128);
    w.move_price(0, -300);
    w.resolve();
    poke_unattributed_positive_pnl_with_tight_bound(&mut w, &mut a, 100_000);
    let shape_before = w.shape();
    let acct_before = w.acct_ok(&mut a);
    let last_fee_slot_before = a.last_fee_slot.get();
    let outcome = w.close(&mut a);
    println!(
        "  last_fee_slot(before)={} resolved_slot={} shape_before={shape_before:?} \
         acct_before={acct_before:?} outcome={outcome:?} shape_after={:?}",
        last_fee_slot_before,
        w.header.resolved_slot.get(),
        w.shape()
    );
    assert!(last_fee_slot_before < w.header.resolved_slot.get());
    if AUDIT_SCAN {
        return;
    }
    assert_eq!(shape_before, Ok(()));
    assert_eq!(acct_before, Ok(()));
    assert_eq!(outcome, Err(V16Error::InvalidConfig));
}

/// Control: the same flow with `source_claim_bound_total_num` left untouched (0). The junior
/// aggregates move in lockstep and the shape survives.
#[test]
fn t15_header_control_without_tight_source_claim_total_shape_holds() {
    println!("build: audit-scan feature = {AUDIT_SCAN}");
    let dep = 1_000_000u128;
    let mut w = W::new(1, None);
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, dep);
    w.deposit(&mut b, dep);
    w.trade(0, &mut a, &mut b, POS_SCALE as i128);
    w.move_price(0, -300);
    w.resolve();
    w.fee_sync_at_resolved_slot(&mut a);
    poke_unattributed_positive_pnl_with_tight_bound(&mut w, &mut a, 100_000);
    w.header.source_claim_bound_total_num = V16PodU128::new(0);
    let shape_before = w.shape();
    let outcome = w.close(&mut a);
    let shape_after = w.shape();
    println!("  shape_before={shape_before:?} outcome={outcome:?} shape_after={shape_after:?}");
    if AUDIT_SCAN {
        return;
    }
    assert_eq!(shape_before, Ok(()));
    assert_eq!(outcome, Ok(ResolvedCloseOutcomeV16::ProgressOnly));
    assert_eq!(shape_after, Ok(()));
}

/// T15.header, variant B, return at 26634, with the additional slot-level claim poke. Under
/// `--features audit-scan` this prints whether the pre-state passes the FULL validate_shape.
#[test]
fn t15_header_refuted_progress_only_26634_with_matching_slot_claim() {
    run_header_witness(1, true);
}

// ------------------------------------------------------------------------------------------
// T15.account at the src/v16.rs:26614 return (AccountBChunk: only validate_shape is re-run).
// ------------------------------------------------------------------------------------------

/// Pokes:
///   config (at market creation, a legal config value): `public_b_chunk_atoms = 10`;
///   market slot 0: `asset.b_long_num += d` (a pending B delta for A's long leg, sized to about
///                  1000 atoms of loss, so one 10-atom chunk leaves `remaining_after != 0`);
///   account: `pnl = +x`, `reserved_pnl = r` (`r <= x`, accepted by `validate_with_market`);
///   header : `pnl_pos_tot += x`, `pnl_pos_bound_tot_num += x*BOUND_SCALE`, `pnl_pos_bound_tot`
///            recomputed. `source_claim_bound_total_num` is NOT touched.
fn run_account_bchunk_witness(x: u128, r: u128) -> (Result<(), V16Error>, Result<(), V16Error>, Result<ResolvedCloseOutcomeV16, V16Error>, Result<(), V16Error>, Result<(), V16Error>, PortfolioAccountV16Account) {
    let dep = 1_000_000u128;
    let mut w = W::new(1, Some(10));
    let (mut a, mut b) = (account(1), account(2));
    w.deposit(&mut a, dep);
    w.deposit(&mut b, dep);
    w.trade(0, &mut a, &mut b, POS_SCALE as i128);
    w.resolve();
    let leg = a.legs[0].try_to_runtime().unwrap();
    assert!(leg.active && leg.loss_weight != 0);
    let d = 1_000 * SOCIAL_LOSS_DEN / leg.loss_weight + 1;
    let b_now = w.markets[0].engine.asset.b_long_num.get();
    w.markets[0].engine.asset.b_long_num = V16PodU128::new(b_now + d);
    if x != 0 {
        poke_unattributed_positive_pnl_with_tight_bound(&mut w, &mut a, x);
        w.header.source_claim_bound_total_num = V16PodU128::new(0);
    }
    a.reserved_pnl = V16PodU128::new(r);

    let shape_before = w.shape();
    let acct_before = w.acct_ok(&mut a);
    println!(
        "  [before] pnl={} reserved_pnl={} leg.b_snap={} asset.b_long_num={} loss_weight={} \
         b_stale_state={}",
        a.pnl.get(),
        a.reserved_pnl.get(),
        leg.b_snap,
        w.markets[0].engine.asset.b_long_num.get(),
        leg.loss_weight,
        a.b_stale_state
    );
    println!("  [before] validate_shape={shape_before:?} validate_with_market={acct_before:?}");
    let outcome = w.close(&mut a);
    let shape_after = w.shape();
    let acct_after = w.acct_ok(&mut a);
    let leg_after = a.legs[0].try_to_runtime().unwrap();
    println!(
        "  [after] pnl={} reserved_pnl={} leg.active={} leg.b_snap={} leg.b_stale={} \
         asset.b_long_num={} b_stale_state={}",
        a.pnl.get(),
        a.reserved_pnl.get(),
        leg_after.active,
        leg_after.b_snap,
        leg_after.b_stale,
        w.markets[0].engine.asset.b_long_num.get(),
        a.b_stale_state
    );
    println!("  [after] outcome={outcome:?}");
    println!("  [after] validate_shape={shape_after:?} validate_with_market={acct_after:?}");
    (shape_before, acct_before, outcome, shape_after, acct_after, a)
}

/// T15.account at 26614: `reserved_pnl` is not clamped by the B-chunk pnl write
/// (`settle_account_b_chunk` -> `set_account_pnl_after_domain_first_source_claim_burn`,
/// src/v16.rs:17617), so `reserved_pnl > max(pnl, 0)` (src/v16.rs:6495) after the call.
#[test]
fn t15_account_refuted_at_26614_reserved_pnl_above_pnl_after_b_chunk() {
    println!("build: audit-scan feature = {AUDIT_SCAN}");
    let (shape_before, acct_before, outcome, shape_after, acct_after, a) =
        run_account_bchunk_witness(100, 100);
    if AUDIT_SCAN {
        println!("  audit-scan build: no deployed-predicate claim is made from this run");
        return;
    }
    assert_eq!(shape_before, Ok(()));
    assert_eq!(acct_before, Ok(()), "hypothesis: validate_with_market Ok before");
    assert_eq!(outcome, Ok(ResolvedCloseOutcomeV16::ProgressOnly));
    assert_eq!(shape_after, Ok(()));
    assert_eq!(acct_after, Err(V16Error::InvalidLeg), "validate_with_market after");
    assert!(a.reserved_pnl.get() > a.pnl.get().max(0) as u128);
    // Which return: the leg is still B-stale with B remaining, i.e. the walk at 26607 returned
    // AccountBChunk; every later return needs the B walk to have completed first.
    let leg = a.legs[0].try_to_runtime().unwrap();
    assert!(leg.active && leg.b_stale && a.b_stale_state != 0);
}

/// Control for the above: no `reserved_pnl` poke. Printed for comparison.
#[test]
fn t15_account_control_at_26614_without_reserved_pnl() {
    println!("build: audit-scan feature = {AUDIT_SCAN}");
    let (shape_before, acct_before, outcome, shape_after, acct_after, _a) =
        run_account_bchunk_witness(100, 0);
    if AUDIT_SCAN {
        return;
    }
    assert_eq!(shape_before, Ok(()));
    assert_eq!(acct_before, Ok(()));
    assert_eq!(outcome, Ok(ResolvedCloseOutcomeV16::ProgressOnly));
    assert_eq!(shape_after, Ok(()));
    assert_eq!(acct_after, Ok(()));
}

// ------------------------------------------------------------------------------------------
// Deterministic randomized search (seeded xorshift64*). `#[ignore]`d: run explicitly with
//   cargo test -j 4 --test strong2_t15_progress_only_shape t15_random_search -- --ignored --nocapture
// It asserts nothing about the engine; it prints counts and the first witnesses per class.
// ------------------------------------------------------------------------------------------

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn pick_u128(r: &mut Rng, cur: u128) -> u128 {
    match r.below(16) {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 10,
        4 => 1_000,
        5 => 30_000,
        6 => 100_000,
        7 => 1_000_000,
        8 => cur.wrapping_add(1),
        9 => cur.saturating_sub(1),
        10 => cur / 2,
        11 => cur.saturating_mul(2),
        12 => cur.saturating_add(r.below(200_000) as u128),
        13 => cur.saturating_sub(r.below(200_000) as u128),
        14 => (r.below(200_000) as u128) * BOUND_SCALE,
        _ => r.below(3_000_000) as u128,
    }
}

const N_MUT: u64 = 53;

/// Applies mutation `k`; returns its name. Fields poked: see the names.
fn mutate(k: u64, r: &mut Rng, w: &mut W, a: &mut PortfolioAccountV16Account) -> &'static str {
    let h = &mut w.header;
    macro_rules! u128f {
        ($f:expr, $n:literal) => {{
            $f = V16PodU128::new(pick_u128(r, $f.get()));
            $n
        }};
    }
    macro_rules! u64f {
        ($f:expr, $n:literal) => {{
            $f = V16PodU64::new(pick_u128(r, $f.get() as u128).min(u64::MAX as u128) as u64);
            $n
        }};
    }
    match k {
        0 => {
            let v = pick_u128(r, a.pnl.get().unsigned_abs()).min(i128::MAX as u128 / 4) as i128;
            a.pnl = V16PodI128::new(if r.below(2) == 0 { v } else { -v });
            "a.pnl"
        }
        1 => u128f!(a.capital, "a.capital"),
        2 => u128f!(a.reserved_pnl, "a.reserved_pnl"),
        3 => {
            let v = pick_u128(r, a.fee_credits.get().unsigned_abs()).min(i128::MAX as u128 / 4) as i128;
            a.fee_credits = V16PodI128::new(-v);
            "a.fee_credits"
        }
        4 => u64f!(a.last_fee_slot, "a.last_fee_slot"),
        5 => {
            a.liquidation_lock = r.below(2) as u8;
            "a.liquidation_lock"
        }
        6 => {
            a.loss_provenance = percolator::V16PodU32::new(match r.below(5) {
                0 => 0,
                1 => 1,
                2 => 2,
                3 => 3,
                _ => u32::MAX,
            });
            "a.loss_provenance"
        }
        7 => {
            a.stale_state = r.below(2) as u8;
            "a.stale_state"
        }
        8 => {
            a.b_stale_state = r.below(2) as u8;
            "a.b_stale_state"
        }
        9 => u128f!(a.residual_crystallized_loss_atoms_total, "a.residual_crystallized"),
        10 => u128f!(a.residual_spent_principal_atoms_total, "a.residual_spent"),
        11 => {
            // A structurally plausible open close ledger on asset 0.
            let gross = pick_u128(r, a.pnl.get().unsigned_abs()).max(1);
            let side = r.below(2) as u8;
            a.close_progress.active = 1;
            a.close_progress.finalized = 0;
            a.close_progress.canceled = 0;
            a.close_progress.close_id = V16PodU64::new(1);
            a.close_progress.asset_index = percolator::V16PodU32::new(0);
            a.close_progress.market_id = w.markets[0].engine.asset.market_id;
            a.close_progress.domain_side = side;
            a.close_progress.gross_loss_at_close_start = V16PodU128::new(gross);
            a.close_progress.drift_reference_slot = h.current_slot;
            a.close_progress.max_close_slot =
                V16PodU64::new(h.current_slot.get() + r.below(3) * 1_000_000);
            a.close_progress.residual_remaining = V16PodU128::new(gross);
            "a.close_progress=open"
        }
        12 => u128f!(a.close_progress.residual_remaining, "a.cp.residual_remaining"),
        13 => u128f!(a.close_progress.gross_loss_at_close_start, "a.cp.gross"),
        14 => u128f!(a.close_progress.insurance_spent, "a.cp.insurance_spent"),
        15 => u128f!(a.close_progress.b_loss_booked, "a.cp.b_loss_booked"),
        16 => u128f!(a.close_progress.explicit_loss_assigned, "a.cp.explicit"),
        17 => {
            a.close_progress.finalized = r.below(2) as u8;
            "a.cp.finalized"
        }
        18 => {
            a.close_progress.canceled = r.below(2) as u8;
            "a.cp.canceled"
        }
        19 => u128f!(a.close_progress.quantity_adl_applied_q, "a.cp.qty_adl"),
        20 => u128f!(h.vault, "h.vault"),
        21 => u128f!(h.c_tot, "h.c_tot"),
        22 => u128f!(h.insurance, "h.insurance"),
        23 => u128f!(h.pnl_pos_tot, "h.pnl_pos_tot"),
        24 => {
            let v = pick_u128(r, h.pnl_pos_bound_tot_num.get());
            h.pnl_pos_bound_tot_num = V16PodU128::new(v);
            h.pnl_pos_bound_tot = V16PodU128::new(v.div_ceil(BOUND_SCALE));
            "h.pnl_pos_bound_tot_num(+tot)"
        }
        25 => u128f!(h.pnl_matured_pos_tot, "h.pnl_matured_pos_tot"),
        26 => u128f!(h.source_claim_bound_total_num, "h.source_claim_bound_total_num"),
        27 => u128f!(h.insurance_domain_budget_remaining_total, "h.ins_budget_remaining_total"),
        28 => u128f!(
            h.source_insurance_credit_reserved_total_atoms,
            "h.source_ins_credit_reserved_total"
        ),
        29 => u64f!(h.negative_pnl_account_count, "h.negative_pnl_account_count"),
        30 => u64f!(h.stale_certificate_count, "h.stale_certificate_count"),
        31 => u64f!(h.b_stale_account_count, "h.b_stale_account_count"),
        32 => {
            h.bankruptcy_hlock_active = r.below(4) as u8;
            "h.bankruptcy_hlock_active"
        }
        33 => u128f!(h.backing_provider_earnings_total, "h.backing_provider_earnings_total"),
        34 => u128f!(h.source_fresh_backing_total_num, "h.source_fresh_backing_total_num"),
        35 => u64f!(h.resolved_payout_blocker_count, "h.resolved_payout_blocker_count"),
        36 => {
            // account-consistent positive pnl: pnl=+x with matching junior aggregates
            let x = 1 + r.below(200_000) as u128;
            a.pnl = V16PodI128::new(x as i128);
            h.pnl_pos_tot = V16PodU128::new(h.pnl_pos_tot.get() + x);
            let num = h.pnl_pos_bound_tot_num.get() + x * BOUND_SCALE;
            h.pnl_pos_bound_tot_num = V16PodU128::new(num);
            h.pnl_pos_bound_tot = V16PodU128::new(num.div_ceil(BOUND_SCALE));
            "a.pnl=+x(with header junior totals)"
        }
        37 => {
            // account-consistent negative pnl
            let x = 1 + r.below(2_500_000) as u128;
            if a.pnl.get() >= 0 {
                h.negative_pnl_account_count =
                    V16PodU64::new(h.negative_pnl_account_count.get() + 1);
            }
            a.pnl = V16PodI128::new(-(x as i128));
            "a.pnl=-x(with header negative count)"
        }
        38 => {
            a.last_fee_slot = h.resolved_slot;
            "a.last_fee_slot=resolved_slot"
        }
        39 => u128f!(
            w.markets[0].engine.insurance_domain_budget_long,
            "slot0.insurance_domain_budget_long"
        ),
        40 => u128f!(
            w.markets[0].engine.insurance_domain_budget_short,
            "slot0.insurance_domain_budget_short"
        ),
        41 => {
            let cur = w.markets[0].engine.asset.b_long_num.get();
            w.markets[0].engine.asset.b_long_num =
                V16PodU128::new(cur + (1 + r.below(2_000) as u128) * (SOCIAL_LOSS_DEN / 1_000_000));
            "slot0.asset.b_long_num+="
        }
        42 => {
            let cur = w.markets[0].engine.asset.b_short_num.get();
            w.markets[0].engine.asset.b_short_num =
                V16PodU128::new(cur + (1 + r.below(2_000) as u128) * (SOCIAL_LOSS_DEN / 1_000_000));
            "slot0.asset.b_short_num+="
        }
        43 => {
            // insurance funded in lockstep with the vault (keeps the senior stack clause)
            let x = 1 + r.below(500_000) as u128;
            h.vault = V16PodU128::new(h.vault.get() + x);
            h.insurance = V16PodU128::new(h.insurance.get() + x);
            "h.vault+=x,h.insurance+=x"
        }
        // ---- boundary ("tight") mutations: put one validate_shape / validate_with_market
        // ---- inequality exactly at equality.
        44 => {
            h.source_claim_bound_total_num = h.pnl_pos_bound_tot_num;
            "TIGHT h.source_claim_bound_total_num=pnl_pos_bound_tot_num"
        }
        45 => {
            a.reserved_pnl = V16PodU128::new(a.pnl.get().max(0) as u128);
            "TIGHT a.reserved_pnl=max(pnl,0)"
        }
        46 => {
            h.pnl_matured_pos_tot = h.pnl_pos_tot;
            "TIGHT h.pnl_matured_pos_tot=pnl_pos_tot"
        }
        47 => {
            h.insurance_domain_budget_remaining_total = h.insurance;
            "TIGHT h.ins_budget_remaining_total=insurance"
        }
        48 => {
            h.source_insurance_credit_reserved_total_atoms = h.insurance;
            "TIGHT h.source_ins_credit_reserved_total=insurance"
        }
        49 => {
            let senior = h.c_tot.get() + h.insurance.get() + h.backing_provider_earnings_total.get();
            h.source_fresh_backing_total_num =
                V16PodU128::new(h.vault.get().saturating_sub(senior) * BOUND_SCALE);
            "TIGHT h.source_fresh_backing_total_num=(vault-senior)*SCALE"
        }
        50 => {
            let v = h.pnl_pos_tot.get() * BOUND_SCALE;
            h.pnl_pos_bound_tot_num = V16PodU128::new(v);
            h.pnl_pos_bound_tot = h.pnl_pos_tot;
            "TIGHT h.pnl_pos_bound_tot_num=exact"
        }
        51 => {
            // slot-level insurance budget for both domains of asset 0, with the header total
            let x = h.insurance.get() / 2;
            w.markets[0].engine.insurance_domain_budget_long = V16PodU128::new(x);
            w.markets[0].engine.insurance_domain_budget_short = V16PodU128::new(x);
            h.insurance_domain_budget_remaining_total = V16PodU128::new(
                (2 * x)
                    .saturating_sub(w.markets[0].engine.insurance_domain_spent_long.get())
                    .saturating_sub(w.markets[0].engine.insurance_domain_spent_short.get()),
            );
            "slot0.insurance_domain_budget_{long,short}=insurance/2(+header total)"
        }
        _ => {
            a.residual_spent_principal_atoms_total = a.residual_crystallized_loss_atoms_total;
            "TIGHT a.residual_spent=residual_crystallized"
        }
    }
}

/// Base worlds, all built through public entries only. Returns (world, subject, other).
fn base_world(kind: u64) -> (W, PortfolioAccountV16Account, PortfolioAccountV16Account) {
    let chunk = if kind % 2 == 1 { Some(10) } else { None };
    match kind / 2 {
        // flat depositor
        0 => {
            let mut w = W::new(1, chunk);
            let (mut a, mut b) = (account(1), account(2));
            w.deposit(&mut a, 1_000_000);
            w.deposit(&mut b, 1_000_000);
            w.resolve();
            (w, a, b)
        }
        // long loser with an unsettled -3% K loss / short winner
        1 | 2 => {
            let mut w = W::new(1, chunk);
            let (mut a, mut b) = (account(1), account(2));
            w.deposit(&mut a, 1_000_000);
            w.deposit(&mut b, 1_000_000);
            w.trade(0, &mut a, &mut b, POS_SCALE as i128);
            w.move_price(0, -300);
            w.resolve();
            if kind / 2 == 1 { (w, a, b) } else { (w, b, a) }
        }
        // thinly margined long with an unsettled loss larger than its capital / its winner
        3 | 4 => {
            let mut w = W::new(1, chunk);
            let (mut a, mut b) = (account(1), account(2));
            w.deposit(&mut a, 120_000);
            w.deposit(&mut b, 1_000_000);
            w.trade(0, &mut a, &mut b, POS_SCALE as i128);
            w.move_price(0, -2_000);
            w.resolve();
            if kind / 2 == 3 { (w, a, b) } else { (w, b, a) }
        }
        // two assets, legs on both, loss on asset 0
        5 => {
            let mut w = W::new(2, chunk);
            let (mut a, mut b) = (account(1), account(2));
            w.deposit(&mut a, 1_000_000);
            w.deposit(&mut b, 1_000_000);
            w.trade(0, &mut a, &mut b, POS_SCALE as i128);
            w.trade(1, &mut a, &mut b, POS_SCALE as i128);
            w.move_price(0, -300);
            w.resolve();
            (w, a, b)
        }
        // matched book, no price move
        _ => {
            let mut w = W::new(1, chunk);
            let (mut a, mut b) = (account(1), account(2));
            w.deposit(&mut a, 1_000_000);
            w.deposit(&mut b, 1_000_000);
            w.trade(0, &mut a, &mut b, POS_SCALE as i128);
            w.resolve();
            (w, a, b)
        }
    }
}

const N_KINDS: u64 = 14;

#[test]
#[ignore]
fn t15_random_search() {
    use std::collections::BTreeMap;
    let iters: u64 = std::env::var("T15_ITERS").ok().and_then(|v| v.parse().ok()).unwrap_or(300_000);
    println!("build: audit-scan feature = {AUDIT_SCAN}; iterations = {iters}; seed = 0x5EED_7151");
    std::panic::set_hook(Box::new(|_| {}));
    let bases: Vec<_> = (0..N_KINDS).map(base_world).collect();
    let mut rng = Rng(0x5EED_7151);
    let mut c: BTreeMap<&'static str, u64> = BTreeMap::new();
    let mut witnesses: BTreeMap<String, (u64, String)> = BTreeMap::new();
    for it in 0..iters {
        let kind = rng.below(N_KINDS);
        let (mut w, mut a, _b) = bases[kind as usize].clone();
        // optionally advance the subject by 0..=2 honest close calls first
        let pre = rng.below(4);
        for _ in 0..pre.min(2) {
            let _ = w.close(&mut a);
        }
        let n_mut = rng.below(5);
        let mut names: Vec<&'static str> = Vec::new();
        for _ in 0..n_mut {
            let k = rng.below(N_MUT);
            names.push(mutate(k, &mut rng, &mut w, &mut a));
        }
        // boundary phase: 0..=2 "tight" mutations applied last
        for _ in 0..rng.below(3) {
            let k = 44 + rng.below(N_MUT - 44);
            names.push(mutate(k, &mut rng, &mut w, &mut a));
        }
        *c.entry("00 total").or_default() += 1;
        let shape_before = w.shape();
        if shape_before.is_err() {
            *c.entry("01 skipped: validate_shape Err before").or_default() += 1;
            continue;
        }
        let acct_before = w.acct_ok(&mut a);
        let (mut w2, mut a2) = (w.clone(), a.clone());
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let out = w2.close(&mut a2);
            (out, w2, a2)
        }));
        let (outcome, mut w2, mut a2) = match res {
            Ok(v) => v,
            Err(_) => {
                *c.entry("02 PANIC inside call").or_default() += 1;
                witnesses
                    .entry("PANIC".to_string())
                    .or_insert((it, format!("kind={kind} pre={pre} muts={names:?}")));
                continue;
            }
        };
        match outcome {
            Err(_) => {
                *c.entry("03 call Err").or_default() += 1;
                continue;
            }
            Ok(ResolvedCloseOutcomeV16::Closed { .. }) => {
                *c.entry("04 call Ok(Closed)").or_default() += 1;
            }
            Ok(ResolvedCloseOutcomeV16::ProgressOnly) => {
                *c.entry(if acct_before.is_ok() {
                    "05 call Ok(ProgressOnly), account valid before (variant B)"
                } else {
                    "06 call Ok(ProgressOnly), account INVALID before (variant A only)"
                })
                .or_default() += 1;
            }
        }
        let closed = matches!(outcome, Ok(ResolvedCloseOutcomeV16::Closed { .. }));
        let shape_after = w2.shape();
        let acct_after = w2.acct_ok(&mut a2);
        // Heuristic return classification for ProgressOnly: a still-B-stale active leg means the
        // B walk did not complete, i.e. the 26614 return.
        let b_leg_stale = a2
            .legs
            .iter()
            .any(|l| l.try_to_runtime().map(|l| l.active && l.b_stale).unwrap_or(false));
        let has_claims = a2.source_domains.iter().any(|s| s.source_claim_bound_num.get() != 0);
        let ret = if closed {
            "Closed"
        } else if b_leg_stale {
            "26614?"
        } else if has_claims {
            "26594/26631/26634?"
        } else {
            "26605/26631/26634/26641"
        };
        if shape_after.is_err() {
            let key = if acct_before.is_ok() {
                "10 P_header VIOLATED, account valid before (variant B)"
            } else {
                "11 P_header VIOLATED, account invalid before (variant A only)"
            };
            *c.entry(key).or_default() += 1;
            let mut ns = names.clone();
            ns.sort();
            ns.dedup();
            witnesses
                .entry(format!("{key} | ret={ret} | kind={kind} | {ns:?}"))
                .or_insert((it, format!("pre={pre} shape_after={shape_after:?}")));
        }
        if acct_before.is_ok() && acct_after.is_err() {
            let key = if b_leg_stale {
                "20 P_account VIOLATED, return classified 26614"
            } else if closed {
                "22 P_account VIOLATED on Closed"
            } else {
                "21 P_account VIOLATED, return classified not-26614"
            };
            *c.entry(key).or_default() += 1;
            let mut ns = names.clone();
            ns.sort();
            ns.dedup();
            witnesses
                .entry(format!("{key} | ret={ret} | kind={kind} | {ns:?}"))
                .or_insert((it, format!("pre={pre} acct_after={acct_after:?} pnl={} reserved={} lock={} bitmap_empty={}", a2.pnl.get(), a2.reserved_pnl.get(), a2.liquidation_lock, bitmap_empty(&a2))));
        }
    }
    let _ = std::panic::take_hook();
    println!("counts:");
    for (k, v) in &c {
        println!("  {k}: {v}");
    }
    println!("distinct witness classes: {}", witnesses.len());
    for (k, (it, d)) in witnesses.iter().take(120) {
        println!("  first@{it}: {k} :: {d}");
    }
}
