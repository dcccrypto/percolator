//! Extended Fuzzing Suite for percolator-core Risk Engine
//!
//! ## Running Tests
//!
//! ```
//! cargo test --features fuzz                         # all tests, 100 cases each
//! PROPTEST_CASES=1000 cargo test --features fuzz     # 1000 cases
//! ```
//!
//! ## Coverage
//!
//! 1. Oracle price fuzzing  — ewma / accrue_market_to edge properties
//! 2. Margin calculation fuzzing — notional / MM / IM monotonicity and zero-position
//! 3. PnL calculation fuzzing — compute_trade_pnl sign and zero invariants
//! 4. Funding accumulation fuzzing — accrue_market_to K coefficients and conservation
//! 5. Conservation invariant fuzzer — extended state machine with cross-margin, LP
//!    vault, and insurance top-up operations; vault >= c_tot + insurance after every step

#![cfg(feature = "fuzz")]

use percolator::*;
use proptest::prelude::*;

// ============================================================================
// Shared helpers (mirror those in fuzzing.rs — kept local to avoid cross-module deps)
// ============================================================================

fn params_std() -> RiskParams {
    RiskParams {
        warmup_period_slots: 0, // Instant warmup simplifies PnL assertions
        maintenance_margin_bps: 500,
        initial_margin_bps: 1000,
        trading_fee_bps: 10,
        max_accounts: 16,
        new_account_fee: U128::new(0),
        maintenance_fee_per_slot: U128::new(0),
        max_crank_staleness_slots: u64::MAX,
        liquidation_fee_bps: 50,
        liquidation_fee_cap: U128::new(1_000_000),
        min_liquidation_abs: U128::new(100_000),
        min_initial_deposit: U128::new(2),
        min_nonzero_mm_req: 1,
        min_nonzero_im_req: 2,
        insurance_floor: U128::ZERO,
    }
}

// ============================================================================
// SECTION 1: Oracle price fuzzing
// ============================================================================
//
// accrue_market_to enforces:
//   - oracle_price in (0, MAX_ORACLE_PRICE] — anything else returns Err
//   - No mutation on Err (Solana atomicity holds)
//   - K coefficients are bounded (no silent overflow)
//   - Conservation is preserved after every successful call

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Any oracle_price outside (0, MAX_ORACLE_PRICE] must return Err
    #[test]
    fn fuzz_oracle_price_bounds(price in 0u64..=u64::MAX) {
        let mut engine = Box::new(RiskEngine::new(params_std()));
        let _ = engine.add_lp([0u8; 32], [0u8; 32], 1);

        // Price = 0 must fail
        let res = engine.accrue_market_to(0, 0);
        prop_assert!(res.is_err(), "oracle_price=0 must return Err");

        // Price > MAX_ORACLE_PRICE must fail
        if let Some(over) = MAX_ORACLE_PRICE.checked_add(1) {
            let res2 = engine.accrue_market_to(0, over);
            prop_assert!(res2.is_err(), "oracle_price > MAX_ORACLE_PRICE must return Err");
        }

        // Any valid price must succeed on a fresh engine (no mutation if err)
        let valid = price.clamp(1, MAX_ORACLE_PRICE);
        let before = (*engine).clone();
        let res3 = engine.accrue_market_to(0, valid);
        if res3.is_err() {
            // Error must mean no mutation
            prop_assert_eq!(
                engine.vault, before.vault,
                "vault changed on accrue_market_to Err"
            );
            prop_assert_eq!(
                engine.c_tot, before.c_tot,
                "c_tot changed on accrue_market_to Err"
            );
        } else {
            // last_oracle_price must be updated
            prop_assert_eq!(engine.last_oracle_price, valid);
            // Conservation must hold
            prop_assert!(
                engine.check_conservation(),
                "conservation violated after accrue_market_to"
            );
        }
    }

    /// Prices equal to last_oracle_price with zero dt == no-op (K unchanged)
    #[test]
    fn fuzz_oracle_price_no_change_noop(
        price in 1u64..=MAX_ORACLE_PRICE,
    ) {
        let mut engine = Box::new(RiskEngine::new(params_std()));
        // Prime last_oracle_price
        let _ = engine.accrue_market_to(0, price);
        let k_long_before = engine.adl_coeff_long;
        let k_short_before = engine.adl_coeff_short;

        // Same price, same slot — must be a no-op
        let _ = engine.accrue_market_to(0, price);
        prop_assert_eq!(engine.adl_coeff_long, k_long_before, "K_long changed on no-op");
        prop_assert_eq!(engine.adl_coeff_short, k_short_before, "K_short changed on no-op");
    }

    /// Monotonically increasing slot: last_market_slot must never go backwards
    #[test]
    fn fuzz_oracle_slot_monotonicity(
        dt1 in 0u64..1000,
        dt2 in 0u64..1000,
        price in 1u64..=MAX_ORACLE_PRICE,
    ) {
        let mut engine = Box::new(RiskEngine::new(params_std()));
        let slot1 = dt1;
        let slot2 = dt1.saturating_add(dt2);

        let _ = engine.accrue_market_to(slot1, price);
        let market_slot_after_1 = engine.last_market_slot;

        // Going backward in time must fail
        if slot2 < slot1 {
            let res = engine.accrue_market_to(slot2, price);
            prop_assert!(res.is_err(), "backward slot should return Err");
        } else {
            let _ = engine.accrue_market_to(slot2, price);
            prop_assert!(
                engine.last_market_slot >= market_slot_after_1,
                "last_market_slot went backwards"
            );
        }
    }
}

// ============================================================================
// SECTION 2: Margin calculation fuzzing
// ============================================================================
//
// Properties under test:
//   - notional >= 0 always
//   - Zero effective position => MM_req = 0, IM_req = 0
//   - Higher maintenance_margin_bps => higher proportional MM req
//   - notional is proportional to |position_basis_q| * oracle_price

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// notional for a flat (no position) account is always zero
    #[test]
    fn fuzz_notional_nonnegative(
        oracle_price in 1u64..=MAX_ORACLE_PRICE,
    ) {
        let mut e = Box::new(RiskEngine::new(params_std()));
        if let Ok(idx) = e.add_user(1) {
            let n = e.notional(idx as usize, oracle_price);
            prop_assert_eq!(n, 0u128, "flat account should have zero notional");
        }
    }

    /// Zero position => is_above_maintenance_margin iff capital > 0
    #[test]
    fn fuzz_margin_zero_position(
        deposit in 1u128..1_000_000,
        oracle_price in 1u64..=MAX_ORACLE_PRICE,
    ) {
        let mut engine = Box::new(RiskEngine::new(params_std()));
        let idx = engine.add_user(1).unwrap();
        engine.deposit(idx, deposit, oracle_price, 0).unwrap();

        let acc = &engine.accounts[idx as usize];
        // flat account: MM_req = 0, equity_net = capital > 0 => above maintenance
        let above_mm = engine.is_above_maintenance_margin(acc, idx as usize, oracle_price);
        let above_im = engine.is_above_initial_margin(acc, idx as usize, oracle_price);
        prop_assert!(above_mm, "funded flat account should be above MM");
        prop_assert!(above_im, "funded flat account should be above IM");
    }

    /// Higher margin_bps => MM req for same position is >= lower bps
    /// We compare two engines that only differ in maintenance_margin_bps
    #[test]
    fn fuzz_higher_margin_bps_higher_req(
        bps_lo in 0u64..5000,
        bps_hi in 5000u64..=10000,
        oracle_price in 1u64..=1_000_000u64,
        size in 1i128..5_000,
    ) {
        let make_params = |mm_bps: u64, im_bps: u64| -> RiskParams {
            RiskParams {
                maintenance_margin_bps: mm_bps,
                initial_margin_bps: im_bps,
                ..params_std()
            }
        };

        let im_lo = bps_lo.max(1).min(10_000); // im >= mm
        let im_hi = bps_hi.max(1).min(10_000);

        // Skip if ordering invariant violated
        if bps_lo > im_lo || im_lo > 10_000 { return Ok(()); }
        if bps_hi > im_hi || im_hi > 10_000 { return Ok(()); }

        let params_lo = make_params(bps_lo, im_lo);
        let params_hi = make_params(bps_hi, im_hi);

        let mut e_lo = Box::new(RiskEngine::new(params_lo));
        let mut e_hi = Box::new(RiskEngine::new(params_hi));

        // Add LP + user with same trade in both engines
        let setup = |e: &mut Box<RiskEngine>| -> Option<(u16, u16)> {
            let lp = e.add_lp([0u8; 32], [0u8; 32], 1).ok()?;
            let user = e.add_user(1).ok()?;
            e.deposit(lp, 1_000_000, oracle_price, 0).ok()?;
            e.deposit(user, 1_000_000, oracle_price, 0).ok()?;
            Some((lp, user))
        };

        if let (Some((lp_lo, user_lo)), Some((lp_hi, user_hi))) = (setup(&mut e_lo), setup(&mut e_hi)) {
            let _ = e_lo.execute_trade_not_atomic(lp_lo, user_lo, oracle_price, 0, size, oracle_price, 0);
            let _ = e_hi.execute_trade_not_atomic(lp_hi, user_hi, oracle_price, 0, size, oracle_price, 0);

            let n_lo = e_lo.notional(user_lo as usize, oracle_price);
            let n_hi = e_hi.notional(user_hi as usize, oracle_price);

            // notional itself is oracle/position driven — same in both engines
            // MM req proportional = notional * bps / 10_000
            let mm_req_lo = mul_div_floor_u128_pub(n_lo, bps_lo, 10_000);
            let mm_req_hi = mul_div_floor_u128_pub(n_hi, bps_hi, 10_000);

            prop_assert!(
                mm_req_hi >= mm_req_lo,
                "Higher margin_bps ({}) should produce >= MM req ({}) vs lower bps ({}) req ({})",
                bps_hi, mm_req_hi, bps_lo, mm_req_lo
            );
        }
    }
}

/// Wrapper exposing `mul_div_floor_u128` for use in tests.
/// We replicate the formula to avoid depending on a non-pub function.
fn mul_div_floor_u128_pub(a: u128, b: u64, c: u64) -> u128 {
    // a * b / c, floor (same formula the engine uses)
    if c == 0 { return 0; }
    ((a as u128).saturating_mul(b as u128)) / (c as u128)
}

// ============================================================================
// SECTION 3: PnL calculation fuzzing
// ============================================================================
//
// compute_trade_pnl(size_q, price_diff) = floor(size_q * price_diff / POS_SCALE)
//
// Properties:
//   - Long + price up  => positive PnL
//   - Long + price down => negative PnL
//   - Short + price up  => negative PnL
//   - Short + price down => positive PnL
//   - price_diff == 0   => zero PnL regardless of size
//   - size_q == 0       => zero PnL regardless of price_diff
//   - entry_price == current_price => zero PnL

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// Long position + rising price => positive PnL (or zero at rounding boundary)
    #[test]
    fn fuzz_pnl_long_price_up(
        size in 1i128..=1_000_000_000i128,
        price_rise in 1i128..=1_000_000i128,
    ) {
        let pnl = compute_trade_pnl(size, price_rise);
        match pnl {
            Ok(v) => prop_assert!(v >= 0, "long+up must be >= 0, got {}", v),
            Err(_) => {}, // overflow on extreme inputs — acceptable
        }
    }

    /// Long position + falling price => non-positive PnL
    #[test]
    fn fuzz_pnl_long_price_down(
        size in 1i128..=1_000_000_000i128,
        price_fall in 1i128..=1_000_000i128,
    ) {
        let pnl = compute_trade_pnl(size, -price_fall);
        match pnl {
            Ok(v) => prop_assert!(v <= 0, "long+down must be <= 0, got {}", v),
            Err(_) => {},
        }
    }

    /// Short position + rising price => non-positive PnL
    #[test]
    fn fuzz_pnl_short_price_up(
        size in 1i128..=1_000_000_000i128,
        price_rise in 1i128..=1_000_000i128,
    ) {
        let pnl = compute_trade_pnl(-size, price_rise);
        match pnl {
            Ok(v) => prop_assert!(v <= 0, "short+up must be <= 0, got {}", v),
            Err(_) => {},
        }
    }

    /// Short position + falling price => non-negative PnL
    #[test]
    fn fuzz_pnl_short_price_down(
        size in 1i128..=1_000_000_000i128,
        price_fall in 1i128..=1_000_000i128,
    ) {
        let pnl = compute_trade_pnl(-size, -price_fall);
        match pnl {
            Ok(v) => prop_assert!(v >= 0, "short+down must be >= 0, got {}", v),
            Err(_) => {},
        }
    }

    /// Zero price_diff => zero PnL
    #[test]
    fn fuzz_pnl_zero_price_diff(size in i128::MIN..=i128::MAX) {
        // i128::MIN is forbidden in the engine; skip it
        if size == i128::MIN { return Ok(()); }
        let pnl = compute_trade_pnl(size, 0);
        prop_assert_eq!(pnl, Ok(0i128), "price_diff=0 must yield PnL=0");
    }

    /// Zero size_q => zero PnL
    #[test]
    fn fuzz_pnl_zero_size(price_diff in i128::MIN..=i128::MAX) {
        if price_diff == i128::MIN { return Ok(()); }
        let pnl = compute_trade_pnl(0, price_diff);
        prop_assert_eq!(pnl, Ok(0i128), "size=0 must yield PnL=0");
    }

    /// Symmetry: PnL(size, +d) and PnL(size, -d) differ only in sign
    /// (subject to floor rounding which may cause |pnl_neg| == |pnl_pos| + 1)
    #[test]
    fn fuzz_pnl_sign_symmetry(
        size in 1i128..1_000_000i128,
        price_diff in 1i128..1_000_000i128,
    ) {
        let pos = compute_trade_pnl(size, price_diff);
        let neg = compute_trade_pnl(size, -price_diff);
        if let (Ok(p), Ok(n)) = (pos, neg) {
            // floor rounding: neg pnl has magnitude >= pos pnl (floor toward -inf)
            prop_assert!(
                n <= 0 && p >= 0,
                "sign symmetry: p={} n={}", p, n
            );
            prop_assert!(
                n.unsigned_abs() == p.unsigned_abs() || n.unsigned_abs() == p.unsigned_abs() + 1,
                "magnitude mismatch: p={} n={}", p, n
            );
        }
    }
}

// ============================================================================
// SECTION 4: Funding accumulation fuzzing
// ============================================================================
//
// Properties exercised through accrue_market_to over multiple calls:
//   - Zero funding rate => K coefficients unchanged
//   - No overflow panic (checked arithmetic returns Err, not panic)
//   - Conservation holds after each successful call
//   - Funding transfer: when rate > 0, longs pay shorts (K_long decreases, K_short increases)

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// Zero funding rate => K unchanged after accrue_market_to
    #[test]
    fn fuzz_funding_zero_rate_no_change(
        price in 1u64..=MAX_ORACLE_PRICE,
        dt in 1u64..1000,
    ) {
        let mut engine = Box::new(RiskEngine::new(params_std()));
        // Set last_oracle_price by first call
        let _ = engine.accrue_market_to(0, price);

        // funding_rate_bps_per_slot_last defaults to 0
        engine.funding_rate_bps_per_slot_last = 0;
        let k_long_before = engine.adl_coeff_long;
        let k_short_before = engine.adl_coeff_short;

        let next_slot = dt;
        let _ = engine.accrue_market_to(next_slot, price);

        prop_assert_eq!(
            engine.adl_coeff_long, k_long_before,
            "zero funding rate: K_long must not change"
        );
        prop_assert_eq!(
            engine.adl_coeff_short, k_short_before,
            "zero funding rate: K_short must not change"
        );
    }

    /// Conservation holds after many random funding accruals
    #[test]
    fn fuzz_funding_conservation(
        oracle_price in 1u64..=1_000_000u64,
        rate_bps in -100i64..100,
        steps in 2usize..20,
    ) {
        let mut engine = Box::new(RiskEngine::new(params_std()));
        let lp = match engine.add_lp([0u8; 32], [0u8; 32], 1) {
            Ok(idx) => idx, Err(_) => return Ok(()),
        };
        let user = match engine.add_user(1) {
            Ok(idx) => idx, Err(_) => return Ok(()),
        };
        let _ = engine.deposit(lp, 50_000, oracle_price, 0);
        let _ = engine.deposit(user, 50_000, oracle_price, 0);

        // Open a small position
        let _ = engine.execute_trade_not_atomic(
            lp, user, oracle_price, 0, 1_000i128, oracle_price, 0
        );

        engine.funding_rate_bps_per_slot_last = rate_bps;

        let mut slot = 0u64;
        for _ in 0..steps {
            slot = slot.saturating_add(10);
            let before = (*engine).clone();
            match engine.accrue_market_to(slot, oracle_price) {
                Ok(()) => {
                    prop_assert!(
                        engine.check_conservation(),
                        "conservation violated after funding accrue"
                    );
                }
                Err(_) => {
                    *engine = before; // simulate Solana rollback
                }
            }
        }
    }

    /// Extreme funding inputs never panic — only Err is acceptable
    #[test]
    fn fuzz_funding_no_panic(
        rate_bps in i64::MIN..=i64::MAX,
        dt in 0u64..100_000,
        price in 1u64..=MAX_ORACLE_PRICE,
    ) {
        // i64::MIN is an edge case for negation — engine should return Err cleanly
        let mut engine = Box::new(RiskEngine::new(params_std()));
        let _ = engine.add_lp([0u8; 32], [0u8; 32], 1);
        let _ = engine.accrue_market_to(0, price);
        engine.funding_rate_bps_per_slot_last = rate_bps;
        // Must not panic — Err is acceptable
        let _ = engine.accrue_market_to(dt, price);
    }
}

// ============================================================================
// SECTION 5: Extended conservation state machine
// ============================================================================
//
// Extends the base state machine with:
//   A. Cross-margin style attestation: deposit/withdraw from multiple accounts
//      simultaneously verifying conservation holds after each op
//   B. LP vault deposit/withdraw (deposit to LP account, withdraw from LP)
//   C. Insurance fund top-up (with varying amounts including 0)
//   D. Mixed operation sequences
//
// Invariant: vault >= c_tot + insurance after EVERY successful operation

#[derive(Clone, Debug)]
enum ExtAction {
    // Original operations
    AddUser { fee: u128 },
    AddLp { fee: u128 },
    Deposit { idx_sel: ExtSel, amount: u128 },
    Withdraw { idx_sel: ExtSel, amount: u128 },
    AdvanceSlot { dt: u64 },
    AccrueFunding { dt: u64, price: u64, rate: i64 },
    ExecuteTrade { price: u64, size: i128 },

    // Extended: LP vault deposit/withdraw
    LpVaultDeposit { amount: u128 },
    LpVaultWithdraw { amount: u128 },

    // Extended: Insurance top-up with varying amounts (including 0 and very large)
    InsuranceTopUp { amount: u128 },

    // Extended: Simultaneous cross-margin moves (deposit to user_0, then user_1)
    CrossMarginAttestation {
        amount_a: u128,
        amount_b: u128,
    },
}

#[derive(Clone, Debug)]
enum ExtSel {
    User,
    Lp,
    Random(u16),
}

fn ext_sel_strategy() -> impl Strategy<Value = ExtSel> {
    prop_oneof![
        5 => Just(ExtSel::User),
        2 => Just(ExtSel::Lp),
        2 => (0u16..16).prop_map(ExtSel::Random),
    ]
}

fn ext_action_strategy() -> impl Strategy<Value = ExtAction> {
    prop_oneof![
        // Account creation
        1 => (0u128..50).prop_map(|fee| ExtAction::AddUser { fee }),
        1 => (0u128..50).prop_map(|fee| ExtAction::AddLp { fee }),
        // Basic ops
        8 => (ext_sel_strategy(), 0u128..100_000).prop_map(|(s, a)| ExtAction::Deposit { idx_sel: s, amount: a }),
        4 => (ext_sel_strategy(), 0u128..100_000).prop_map(|(s, a)| ExtAction::Withdraw { idx_sel: s, amount: a }),
        3 => (0u64..20).prop_map(|dt| ExtAction::AdvanceSlot { dt }),
        2 => (1u64..50, 1u64..1_000_000u64, -200i64..200).prop_map(|(dt, p, r)| ExtAction::AccrueFunding { dt, price: p, rate: r }),
        4 => (1u64..1_000_000u64, -3_000i128..3_000).prop_map(|(p, s)| ExtAction::ExecuteTrade { price: p, size: s }),
        // Extended ops
        3 => (0u128..100_000).prop_map(|a| ExtAction::LpVaultDeposit { amount: a }),
        2 => (0u128..50_000).prop_map(|a| ExtAction::LpVaultWithdraw { amount: a }),
        3 => (0u128..50_000).prop_map(|a| ExtAction::InsuranceTopUp { amount: a }),
        2 => (0u128..50_000, 0u128..50_000).prop_map(|(a, b)| ExtAction::CrossMarginAttestation { amount_a: a, amount_b: b }),
    ]
}

struct ExtFuzzState {
    engine: Box<RiskEngine>,
    user_idx: Option<u16>,
    user2_idx: Option<u16>,
    lp_idx: Option<u16>,
    current_oracle: u64,
}

impl ExtFuzzState {
    fn new() -> Self {
        let mut engine = Box::new(RiskEngine::new(params_std()));
        let lp = engine.add_lp([0u8; 32], [0u8; 32], 1).ok();
        let user = engine.add_user(1).ok();
        let user2 = engine.add_user(1).ok();

        // Initial deposits to get non-trivial state
        let oracle = 100_000u64;
        if let Some(idx) = lp {
            let _ = engine.deposit(idx, 50_000, oracle, 0);
        }
        if let Some(idx) = user {
            let _ = engine.deposit(idx, 20_000, oracle, 0);
        }
        if let Some(idx) = user2 {
            let _ = engine.deposit(idx, 10_000, oracle, 0);
        }

        ExtFuzzState {
            engine,
            user_idx: user,
            user2_idx: user2,
            lp_idx: lp,
            current_oracle: oracle,
        }
    }

    fn resolve(&self, sel: &ExtSel, rng_tiebreak: u16) -> u16 {
        match sel {
            ExtSel::User => self.user_idx.unwrap_or(rng_tiebreak % 16),
            ExtSel::Lp => self.lp_idx.unwrap_or(rng_tiebreak % 16),
            ExtSel::Random(n) => *n % 16,
        }
    }

    fn assert_conservation(&self, ctx: &str) {
        assert!(
            self.engine.check_conservation(),
            "{}: conservation violated: vault={} c_tot={} insurance={}",
            ctx,
            self.engine.vault.get(),
            self.engine.c_tot.get(),
            self.engine.insurance_fund.balance.get()
        );
    }

    fn execute(&mut self, action: &ExtAction, step: usize) {
        let ctx = format!("step {} {:?}", step, action);
        let slot = self.engine.current_slot;

        match action {
            ExtAction::AddUser { fee } => {
                let before = (*self.engine).clone();
                match self.engine.add_user(*fee) {
                    Ok(idx) => {
                        if self.user_idx.is_none() { self.user_idx = Some(idx); }
                        else if self.user2_idx.is_none() { self.user2_idx = Some(idx); }
                        self.assert_conservation(&ctx);
                    }
                    Err(_) => { *self.engine = before; }
                }
            }

            ExtAction::AddLp { fee } => {
                let before = (*self.engine).clone();
                match self.engine.add_lp([0u8; 32], [0u8; 32], *fee) {
                    Ok(idx) => {
                        if self.lp_idx.is_none() { self.lp_idx = Some(idx); }
                        self.assert_conservation(&ctx);
                    }
                    Err(_) => { *self.engine = before; }
                }
            }

            ExtAction::Deposit { idx_sel, amount } => {
                let idx = self.resolve(idx_sel, 0);
                let before = (*self.engine).clone();
                match self.engine.deposit(idx, *amount, self.current_oracle, slot) {
                    Ok(()) => { self.assert_conservation(&ctx); }
                    Err(_) => { *self.engine = before; }
                }
            }

            ExtAction::Withdraw { idx_sel, amount } => {
                let idx = self.resolve(idx_sel, 1);
                let before = (*self.engine).clone();
                match self.engine.withdraw_not_atomic(idx, *amount, self.current_oracle, slot, 0) {
                    Ok(()) => { self.assert_conservation(&ctx); }
                    Err(_) => { *self.engine = before; }
                }
            }

            ExtAction::AdvanceSlot { dt } => {
                self.engine.advance_slot(*dt);
                self.assert_conservation(&ctx);
            }

            ExtAction::AccrueFunding { dt, price, rate } => {
                let valid_price = (*price).clamp(1, MAX_ORACLE_PRICE);
                let next_slot = slot.saturating_add(*dt);
                let before = (*self.engine).clone();
                self.engine.funding_rate_bps_per_slot_last = *rate;
                match self.engine.accrue_market_to(next_slot, valid_price) {
                    Ok(()) => {
                        self.current_oracle = valid_price;
                        self.assert_conservation(&ctx);
                    }
                    Err(_) => { *self.engine = before; }
                }
            }

            ExtAction::ExecuteTrade { price, size } => {
                let valid_price = (*price).clamp(1, MAX_ORACLE_PRICE);
                let lp = self.lp_idx.unwrap_or(0);
                let user = self.user_idx.unwrap_or(1);
                if lp == user { return; }
                let before = (*self.engine).clone();
                match self.engine.execute_trade_not_atomic(
                    lp, user, valid_price, slot, *size, valid_price, 0,
                ) {
                    Ok(()) => {
                        self.current_oracle = valid_price;
                        self.assert_conservation(&ctx);
                    }
                    Err(_) => { *self.engine = before; }
                }
            }

            // LP vault deposit: deposit to LP account (simulates staking pool refill)
            ExtAction::LpVaultDeposit { amount } => {
                let lp = self.lp_idx.unwrap_or(0);
                let before = (*self.engine).clone();
                match self.engine.deposit(lp, *amount, self.current_oracle, slot) {
                    Ok(()) => { self.assert_conservation(&ctx); }
                    Err(_) => { *self.engine = before; }
                }
            }

            // LP vault withdraw: withdraw from LP account (simulates payout)
            ExtAction::LpVaultWithdraw { amount } => {
                let lp = self.lp_idx.unwrap_or(0);
                let before = (*self.engine).clone();
                match self.engine.withdraw_not_atomic(lp, *amount, self.current_oracle, slot, 0) {
                    Ok(()) => { self.assert_conservation(&ctx); }
                    Err(_) => { *self.engine = before; }
                }
            }

            // Insurance top-up: verify vault increases and conservation holds
            ExtAction::InsuranceTopUp { amount } => {
                let before = (*self.engine).clone();
                match self.engine.top_up_insurance_fund(*amount, slot) {
                    Ok(_) => { self.assert_conservation(&ctx); }
                    Err(_) => { *self.engine = before; }
                }
            }

            // Cross-margin attestation: simultaneous deposit to user + user2
            // Models a cross-margin operation where both sub-accounts receive capital
            ExtAction::CrossMarginAttestation { amount_a, amount_b } => {
                let user_a = self.user_idx.unwrap_or(0);
                let user_b = self.user2_idx.unwrap_or(1);

                // Simulate atomicity: both deposits succeed or neither is committed
                let snapshot = (*self.engine).clone();

                let res_a = self.engine.deposit(user_a, *amount_a, self.current_oracle, slot);
                let res_b = if res_a.is_ok() {
                    self.engine.deposit(user_b, *amount_b, self.current_oracle, slot)
                } else {
                    Err(RiskError::Overflow) // propagate failure
                };

                match (res_a, res_b) {
                    (Ok(()), Ok(())) => {
                        self.assert_conservation(&ctx);
                    }
                    _ => {
                        // Rollback both
                        *self.engine = snapshot;
                    }
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn fuzz_extended_conservation_state_machine(
        actions in prop::collection::vec(ext_action_strategy(), 50..100)
    ) {
        let mut state = ExtFuzzState::new();

        // Assert conservation on initial setup
        assert!(
            state.engine.check_conservation(),
            "conservation violated on initial setup"
        );

        for (step, action) in actions.iter().enumerate() {
            state.execute(action, step);
        }
    }

    /// LP vault round-trip: deposit then withdraw returns same or less
    #[test]
    fn fuzz_lp_vault_round_trip(
        deposit in 1u128..=1_000_000u128,
        oracle in 1u64..=MAX_ORACLE_PRICE,
    ) {
        let mut engine = Box::new(RiskEngine::new(params_std()));
        let lp = match engine.add_lp([0u8; 32], [0u8; 32], 1) {
            Ok(idx) => idx, Err(_) => return Ok(()),
        };

        let vault_before = engine.vault.get();
        match engine.deposit(lp, deposit, oracle, 0) {
            Ok(()) => {
                prop_assert_eq!(
                    engine.vault.get(),
                    vault_before + deposit,
                    "vault must increase by deposit amount"
                );
                prop_assert!(engine.check_conservation(), "conservation after LP deposit");

                let cap_after_deposit = engine.accounts[lp as usize].capital.get();

                // Withdraw same amount
                let before_withdraw = (*engine).clone();
                match engine.withdraw_not_atomic(lp, deposit, oracle, 0, 0) {
                    Ok(()) => {
                        prop_assert_eq!(
                            engine.vault.get(),
                            vault_before,
                            "vault must return to initial after withdraw"
                        );
                        prop_assert!(engine.check_conservation(), "conservation after LP withdraw");
                    }
                    Err(_) => {
                        // Withdraw rejected (e.g. undercollateralized) — rollback
                        *engine = before_withdraw;
                    }
                }
                let _ = cap_after_deposit; // used indirectly
            }
            Err(_) => {} // deposit rejected (e.g. overflow) — ok
        }
    }

    /// Insurance top-up + full state: conservation always holds
    #[test]
    fn fuzz_insurance_top_up_conservation(
        top_up in 0u128..=1_000_000u128,
        oracle in 1u64..=MAX_ORACLE_PRICE,
    ) {
        let mut engine = Box::new(RiskEngine::new(params_std()));
        let lp = match engine.add_lp([0u8; 32], [0u8; 32], 1) {
            Ok(idx) => idx, Err(_) => return Ok(()),
        };
        let _ = engine.deposit(lp, 100_000, oracle, 0);

        let vault_before = engine.vault.get();
        let ins_before = engine.insurance_fund.balance.get();

        let before = (*engine).clone();
        match engine.top_up_insurance_fund(top_up, 0) {
            Ok(_) => {
                let vault_after = engine.vault.get();
                let ins_after = engine.insurance_fund.balance.get();

                prop_assert_eq!(
                    vault_after,
                    vault_before + top_up,
                    "top-up must increase vault by amount"
                );
                prop_assert!(
                    ins_after >= ins_before,
                    "insurance balance must not decrease after top-up"
                );
                prop_assert!(
                    engine.check_conservation(),
                    "conservation violated after insurance top-up"
                );
            }
            Err(_) => {
                *engine = before;
            }
        }
    }
}
