use super::*;

const E6: u64 = 1_000_000;
const ORACLE_100K: u64 = 100_000 * E6;
const ONE_BASE: i128 = 1_000_000;

fn params_for_kani() -> RiskParams {
    RiskParams {
        warmup_period_slots: 1000,
        maintenance_margin_bps: 0,
        initial_margin_bps: 0,
        trading_fee_bps: 0,
        max_accounts: MAX_ACCOUNTS as u64,
        new_account_fee: U128::new(0),
        risk_reduction_threshold: U128::new(0),

        maintenance_fee_per_slot: U128::new(0),
        max_crank_staleness_slots: u64::MAX,

        liquidation_fee_bps: 0,
        liquidation_fee_cap: U128::new(0),

        liquidation_buffer_bps: 0,
        min_liquidation_abs: U128::new(0),
    }
}

struct P90kMatcher;
impl MatchingEngine for P90kMatcher {
    fn execute_match(
        &self,
        _lp_program: &[u8; 32],
        _lp_context: &[u8; 32],
        _lp_account_id: u64,
        oracle_price: u64,
        size: i128,
    ) -> Result<TradeExecution> {
        Ok(TradeExecution {
            price: oracle_price - (10_000 * E6),
            size,
        })
    }
}

struct AtOracleMatcher;
impl MatchingEngine for AtOracleMatcher {
    fn execute_match(
        &self,
        _lp_program: &[u8; 32],
        _lp_context: &[u8; 32],
        _lp_account_id: u64,
        oracle_price: u64,
        size: i128,
    ) -> Result<TradeExecution> {
        Ok(TradeExecution {
            price: oracle_price,
            size,
        })
    }
}

struct BadMatcherOpposite;
impl MatchingEngine for BadMatcherOpposite {
    fn execute_match(
        &self,
        _lp_program: &[u8; 32],
        _lp_context: &[u8; 32],
        _lp_account_id: u64,
        oracle_price: u64,
        size: i128,
    ) -> Result<TradeExecution> {
        Ok(TradeExecution {
            price: oracle_price,
            size: -size,
        })
    }
}

#[kani::proof]
fn kani_cross_lp_close_no_pnl_teleport() {
    let mut engine = RiskEngine::new(params_for_kani());

    let lp1 = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();
    let lp2 = engine.add_lp([3u8; 32], [4u8; 32], 0).unwrap();
    let user = engine.add_user(0).unwrap();

    // Fund everyone (keep values small but safe)
    engine.deposit(lp1, 50_000_000_000u128, 100).unwrap();
    engine.deposit(lp2, 50_000_000_000u128, 100).unwrap();
    engine.deposit(user, 50_000_000_000u128, 100).unwrap();

    // Trade 1 at slot 100
    engine
        .execute_trade(&P90kMatcher, lp1, user, 100, ORACLE_100K, ONE_BASE)
        .unwrap();

    // Trade 2 at slot 101 (close with LP2 at oracle)
    engine
        .execute_trade(&AtOracleMatcher, lp2, user, 101, ORACLE_100K, -ONE_BASE)
        .unwrap();

    // Slot and warmup assertions (verifies slot propagation)
    assert_eq!(engine.current_slot, 101);
    assert_eq!(engine.accounts[user as usize].warmup_started_at_slot, 101);
    assert_eq!(engine.accounts[lp2 as usize].warmup_started_at_slot, 101);

    // Teleport check: LP2 should not absorb LP1's earlier loss when closing at oracle.
    // settle_warmup_to_capital immediately settles negative PnL against capital,
    // so LP1's pnl field is 0 and capital is reduced by 10k*E6.
    // Some of the user's PnL may have partially settled to capital via warmup
    // during trade 2 (correct behavior: settle matured warmup before slope reset).
    let ten_k_e6: u128 = (10_000 * E6) as u128;
    let initial_cap = 50_000_000_000u128;
    assert_eq!(engine.accounts[user as usize].position_size.get(), 0);
    // Check total value rather than exact pnl (warmup may partially settle)
    let user_pnl = engine.accounts[user as usize].pnl.get() as u128;
    let user_cap = engine.accounts[user as usize].capital.get();
    assert_eq!(user_pnl + user_cap, initial_cap + ten_k_e6);
    assert_eq!(engine.accounts[lp1 as usize].pnl.get(), 0);
    assert_eq!(engine.accounts[lp1 as usize].capital.get(), initial_cap - ten_k_e6);
    assert_eq!(engine.accounts[lp2 as usize].pnl.get(), 0);
    assert_eq!(engine.accounts[lp2 as usize].capital.get(), initial_cap);

    // Conservation must hold
    assert!(engine.check_conservation(ORACLE_100K));
}

#[kani::proof]
fn kani_rejects_invalid_matcher_output() {
    let mut engine = RiskEngine::new(params_for_kani());

    let lp = engine.add_lp([1u8; 32], [2u8; 32], 0).unwrap();
    let user = engine.add_user(0).unwrap();

    engine.deposit(lp, 50_000_000_000u128, 10).unwrap();
    engine.deposit(user, 50_000_000_000u128, 10).unwrap();

    let res = engine.execute_trade(
        &BadMatcherOpposite,
        lp,
        user,
        10,
        ORACLE_100K,
        ONE_BASE,
    );

    assert!(matches!(res, Err(RiskError::InvalidMatchingEngine)));
}
