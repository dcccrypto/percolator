// End-to-end integration tests with realistic trading scenarios
// Tests complete user journeys with multiple participants

#[cfg(feature = "test")]
use percolator::i128::U128;
#[cfg(feature = "test")]
use percolator::*;

#[cfg(feature = "test")]
fn default_params() -> RiskParams {
    RiskParams {
        warmup_period_slots: 100,
        maintenance_margin_bps: 500, // 5%
        initial_margin_bps: 1000,    // 10%
        trading_fee_bps: 10,         // 0.1%
        max_accounts: 64,
        new_account_fee: U128::new(0),
        maintenance_fee_per_slot: U128::new(0),
        max_crank_staleness_slots: u64::MAX,
        liquidation_fee_bps: 50,
        liquidation_fee_cap: U128::new(100_000),
        min_liquidation_abs: U128::new(0),
        min_initial_deposit: U128::new(2),
        min_nonzero_mm_req: 1,
        min_nonzero_im_req: 2,
        insurance_floor: U128::ZERO,
    }
}

/// Helper: create i128 position size from base quantity (scaled by POS_SCALE)
#[cfg(feature = "test")]
fn pos_q(qty: i64) -> i128 {
    let abs_val = (qty as i128).unsigned_abs();
    let scaled = abs_val.checked_mul(POS_SCALE).unwrap();
    if qty < 0 {
        -(scaled as i128)
    } else {
        scaled as i128
    }
}

/// Helper: crank to make trades/withdrawals work
#[cfg(feature = "test")]
fn crank(engine: &mut RiskEngine, slot: u64, oracle_price: u64) {
    let _ = engine.keeper_crank_not_atomic(slot, oracle_price, &[], 64, 0i64);
}

// ============================================================================
// E2E Test 1: Complete User Journey
// ============================================================================

#[test]
#[cfg(feature = "test")]
fn test_e2e_complete_user_journey() {
    // Scenario: Alice and Bob trade, experience PNL, warmup, withdrawal

    let mut engine = Box::new(RiskEngine::new(default_params()));

    // Initialize insurance fund
    let _ = engine.top_up_insurance_fund(50_000, 0);

    // Add two users with capital
    let alice = engine.add_user(0).unwrap();
    let bob = engine.add_user(0).unwrap();

    let oracle_price: u64 = 100; // 100 quote per base

    // Users deposit principal
    engine.deposit(alice, 100_000, oracle_price, 0).unwrap();
    engine.deposit(bob, 150_000, oracle_price, 0).unwrap();

    // Make crank fresh
    crank(&mut engine, 0, oracle_price);

    // === Phase 1: Trading ===

    // Alice goes long 50 base, Bob takes the other side (short)
    engine
        .execute_trade_not_atomic(alice, bob, oracle_price, 0, pos_q(50), oracle_price, 0i64)
        .unwrap();

    // Check effective positions
    let alice_eff = engine.effective_pos_q(alice as usize);
    let bob_eff = engine.effective_pos_q(bob as usize);
    assert!(alice_eff > 0, "Alice should be long");
    assert!(bob_eff < 0, "Bob should be short");

    // Conservation should hold
    assert!(engine.check_conservation(), "Conservation after trade");

    // === Phase 2: Price Movement ===

    let new_price: u64 = 120; // +20%

    // Accrue market to new price
    engine.advance_slot(10);
    let slot = engine.current_slot;
    engine.accrue_market_to(slot, new_price).unwrap();

    // Settle side effects for Alice (should have positive PnL from long)
    engine.settle_side_effects(alice as usize).unwrap();

    let alice_pnl = engine.accounts[alice as usize].pnl;
    // Long position + price up = positive PnL
    assert!(
        alice_pnl > 0,
        "Alice should have positive PnL after price increase"
    );

    // === Phase 3: PNL Warmup ===

    // Advance some slots
    engine.advance_slot(50);

    // Touch to settle and convert warmup
    let slot = engine.current_slot;
    engine
        .touch_account_full_not_atomic(alice as usize, new_price, slot)
        .unwrap();

    // The key invariant is conservation
    assert!(engine.check_conservation(), "Conservation after warmup");

    // === Phase 4: Close positions and withdraw_not_atomic ===

    let slot = engine.current_slot;
    crank(&mut engine, slot, new_price);

    // Alice closes her position (sell)
    let alice_pos = engine.effective_pos_q(alice as usize);
    if alice_pos != 0 {
        let abs_pos = alice_pos.unsigned_abs() as i128;
        let slot = engine.current_slot;
        // alice_pos > 0 (long), so closing means b buys from a (swap a,b with positive size)
        engine
            .execute_trade_not_atomic(bob, alice, new_price, slot, abs_pos, new_price, 0i64)
            .unwrap();
    }

    // Advance for full warmup
    engine.advance_slot(200);
    let slot = engine.current_slot;
    engine
        .touch_account_full_not_atomic(alice as usize, new_price, slot)
        .unwrap();

    // Alice withdraws some capital
    let slot = engine.current_slot;
    crank(&mut engine, slot, new_price);
    let alice_cap = engine.accounts[alice as usize].capital.get();
    if alice_cap > 1000 {
        let slot = engine.current_slot;
        engine
            .withdraw_not_atomic(alice, 1000, new_price, slot, 0i64)
            .unwrap();
    }

    assert!(engine.check_conservation(), "Conservation after withdrawal");
}

// ============================================================================
// E2E Test 2: Funding Complete Cycle
// ============================================================================

#[test]
#[cfg(feature = "test")]
fn test_e2e_funding_complete_cycle() {
    // Scenario: Users trade, positive funding rate accrues (longs pay shorts),
    // then positions flip. Verifies funding actually changes account PnL.

    let mut engine = Box::new(RiskEngine::new(default_params()));
    let _ = engine.top_up_insurance_fund(50_000, 0);

    let alice = engine.add_user(0).unwrap();
    let bob = engine.add_user(0).unwrap();

    let oracle_price: u64 = 100;

    engine.deposit(alice, 200_000, oracle_price, 0).unwrap();
    engine.deposit(bob, 200_000, oracle_price, 0).unwrap();

    crank(&mut engine, 0, oracle_price);

    // Alice goes long, Bob goes short
    engine
        .execute_trade_not_atomic(alice, bob, oracle_price, 0, pos_q(100), oracle_price, 0i64)
        .unwrap();

    // Record capital before funding (settle_losses converts PnL to capital changes,
    // so we track capital, not PnL directly)
    let alice_cap_before = engine.accounts[alice as usize].capital.get();
    let bob_cap_before = engine.accounts[bob as usize].capital.get();

    // Store a positive funding rate: longs pay shorts (500 bps/slot)
    // keeper_crank_not_atomic stores r_last = 500 via recompute_r_last_from_final_state
    engine.advance_slot(1);
    let slot1 = engine.current_slot;
    engine
        .keeper_crank_not_atomic(slot1, oracle_price, &[], 64, 500i64)
        .unwrap();

    // Now r_last = 500. Advance time so next accrue_market_to applies funding.
    engine.advance_slot(20);
    let slot2 = engine.current_slot;

    // This crank accrues the market (which applies 20 slots of funding at rate 500)
    // then touches both accounts (settle_side_effects realizes the K delta into PnL,
    // then settle_losses transfers negative PnL from capital)
    engine
        .keeper_crank_not_atomic(
            slot2,
            oracle_price,
            &[(alice, None), (bob, None)],
            64,
            500i64,
        )
        .unwrap();

    let alice_cap_after = engine.accounts[alice as usize].capital.get();
    let bob_cap_after = engine.accounts[bob as usize].capital.get();

    // Alice (long) paid funding → capital decreased (loss settled from principal)
    assert!(
        alice_cap_after < alice_cap_before,
        "positive rate: long capital must decrease from funding (before={}, after={})",
        alice_cap_before,
        alice_cap_after
    );

    // Bob (short) received funding → PnL positive, but it goes to reserved_pnl
    // (warmup). Bob's capital stays the same but PnL + reserved goes up.
    // Check that bob didn't lose capital like alice did.
    assert!(
        bob_cap_after >= bob_cap_before,
        "positive rate: short capital must not decrease from funding (before={}, after={})",
        bob_cap_before,
        bob_cap_after
    );

    // Net check: alice lost more capital than bob (funding is zero-sum at K level,
    // but floor rounding means payers lose weakly more than receivers gain)
    let alice_loss = alice_cap_before - alice_cap_after;
    assert!(alice_loss > 0, "alice must have lost capital from funding");

    assert!(engine.check_conservation(), "Conservation after funding");

    // === Positions Flip ===
    let slot = engine.current_slot;

    // Alice closes long and opens short (total -200 base)
    engine
        .execute_trade_not_atomic(
            bob,
            alice,
            oracle_price,
            slot,
            pos_q(200),
            oracle_price,
            0i64,
        )
        .unwrap();

    // Now Alice is short and Bob is long
    let alice_eff = engine.effective_pos_q(alice as usize);
    let bob_eff = engine.effective_pos_q(bob as usize);
    assert!(alice_eff < 0, "Alice should now be short");
    assert!(bob_eff > 0, "Bob should now be long");

    assert!(
        engine.check_conservation(),
        "Conservation after position flip"
    );
}

#[test]
#[cfg(feature = "test")]
fn test_e2e_negative_funding_rate() {
    // Negative funding rate: shorts pay longs

    let mut engine = Box::new(RiskEngine::new(default_params()));
    let _ = engine.top_up_insurance_fund(50_000, 0);

    let alice = engine.add_user(0).unwrap();
    let bob = engine.add_user(0).unwrap();

    let oracle_price: u64 = 100;

    engine.deposit(alice, 200_000, oracle_price, 0).unwrap();
    engine.deposit(bob, 200_000, oracle_price, 0).unwrap();

    crank(&mut engine, 0, oracle_price);

    // Alice long, Bob short
    engine
        .execute_trade_not_atomic(alice, bob, oracle_price, 0, pos_q(100), oracle_price, 0i64)
        .unwrap();

    let alice_cap_before = engine.accounts[alice as usize].capital.get();
    let bob_cap_before = engine.accounts[bob as usize].capital.get();

    // Store negative rate: shorts pay longs (-500 bps/slot)
    engine.advance_slot(1);
    let slot1 = engine.current_slot;
    engine
        .keeper_crank_not_atomic(slot1, oracle_price, &[], 64, -500i64)
        .unwrap();

    // Advance and settle
    engine.advance_slot(20);
    let slot2 = engine.current_slot;
    engine
        .keeper_crank_not_atomic(
            slot2,
            oracle_price,
            &[(alice, None), (bob, None)],
            64,
            -500i64,
        )
        .unwrap();

    let alice_cap_after = engine.accounts[alice as usize].capital.get();
    let bob_cap_after = engine.accounts[bob as usize].capital.get();

    // Negative rate: shorts pay, longs receive
    // Bob (short) paid funding → capital decreased (loss settled from principal)
    assert!(
        bob_cap_after < bob_cap_before,
        "negative rate: short capital must decrease (before={}, after={})",
        bob_cap_before,
        bob_cap_after
    );

    // Alice (long) received → capital must not decrease
    assert!(
        alice_cap_after >= alice_cap_before,
        "negative rate: long capital must not decrease (before={}, after={})",
        alice_cap_before,
        alice_cap_after
    );

    let bob_loss = bob_cap_before - bob_cap_after;
    assert!(
        bob_loss > 0,
        "bob must have lost capital from negative funding"
    );

    assert!(
        engine.check_conservation(),
        "Conservation with negative funding"
    );

    // Fork-specific amm tests

    #[test]
    fn test_e2e_oracle_attack_protection() {
        // Scenario: Attacker tries to exploit oracle manipulation but gets limited by warmup + ADL

        let mut engine = Box::new(RiskEngine::new(default_params()));
        engine.insurance_fund.balance = U128::new(30_000);

        let lp = engine.add_lp([1u8; 32], [2u8; 32], 10_000).unwrap();
        engine.accounts[lp as usize].capital = U128::new(200_000);
        engine.vault = U128::new(200_000);

        // Honest user
        let honest_user = engine.add_user(10_000).unwrap();
        engine.deposit(honest_user, 20_000, 0).unwrap();

        // Attacker
        let attacker = engine.add_user(10_000).unwrap();
        engine.deposit(attacker, 10_000, 0).unwrap();
        engine.vault = U128::new(230_000);

        // === Phase 1: Normal Trading ===

        // Honest user opens long position
        engine
            .execute_trade(&MATCHER, lp, honest_user, 0, 1_000_000, 5_000)
            .unwrap();

        // === Phase 2: Oracle Manipulation Attempt ===

        // Attacker opens large position during manipulation
        engine
            .execute_trade(&MATCHER, lp, attacker, 0, 1_000_000, 20_000)
            .unwrap();

        // Oracle gets manipulated to $2 (fake 100% gain)
        let fake_price = 2_000_000;

        // Attacker tries to close and realize fake profit
        engine
            .execute_trade(&MATCHER, lp, attacker, 0, fake_price, -20_000)
            .unwrap();
        // execute_trade automatically calls update_warmup_slope() after realizing PNL

        // Attacker has massive fake PNL
        let attacker_fake_pnl = clamp_pos_i128(engine.accounts[attacker as usize].pnl.get());
        assert!(attacker_fake_pnl > 10_000); // Huge profit from manipulation

        // === Phase 3: Warmup Limiting ===

        // Due to warmup rate limiting, attacker's PNL warms up slowly
        // Max warmup rate = insurance_fund * 0.5 / (T/2)
        let expected_max_rate = engine.insurance_fund.balance * 5000 / 50 / 10_000;

        println!("Attacker fake PNL: {}", attacker_fake_pnl);
        println!("Insurance fund: {}", engine.insurance_fund.balance);
        println!("Expected max warmup rate: {}", expected_max_rate);
        println!("Actual warmup rate: {}", engine.total_warmup_rate);
        println!(
            "Attacker slope: {}",
            engine.accounts[attacker as usize].warmup_slope_per_step
        );

        // Verify that warmup slope was actually set
        assert!(
            engine.accounts[attacker as usize].warmup_slope_per_step > 0,
            "Attacker's warmup slope should be set after realizing PNL"
        );

        // Verify rate limiting is working (attacker's slope should be constrained)
        // In a stressed system, individual slope may be less than ideal due to capacity limits
        let ideal_slope = attacker_fake_pnl / engine.params.warmup_period_slots as u128;
        println!("Ideal slope (no limiting): {}", ideal_slope);
        println!(
            "Actual slope (with limiting): {}",
            engine.accounts[attacker as usize].warmup_slope_per_step
        );

        // Advance only 10 slots (manipulation is detected quickly)
        engine.advance_slot(10);

        let attacker_warmed = engine.withdrawable_pnl(&engine.accounts[attacker as usize]);
        println!("Attacker withdrawable after 10 slots: {}", attacker_warmed);

        // Only a small fraction should be withdrawable
        // Expected: slope was capped by warmup rate limiting + only 10 slots elapsed
        assert!(
            attacker_warmed < attacker_fake_pnl / 5,
            "Most fake PNL should still be warming up (got {} out of {})",
            attacker_warmed,
            attacker_fake_pnl
        );

        // === Phase 4: Oracle Reverts, ADL Triggered ===

        // Oracle reverts to true price, creating loss
        // ADL is triggered to socialize the loss

        engine.apply_adl(attacker_fake_pnl).unwrap();

        // Attacker's unwrapped (still warming) PNL gets haircutted
        let attacker_after_adl = clamp_pos_i128(engine.accounts[attacker as usize].pnl.get());

        // Most of the fake PNL should be gone
        assert!(
            attacker_after_adl < attacker_fake_pnl / 2,
            "ADL should haircut most of the unwrapped PNL"
        );

        // === Phase 5: Honest User Protected ===

        // Honest user's principal should be intact
        assert_eq!(
            engine.accounts[honest_user as usize].capital.get(),
            20_000,
            "I1: Principal never reduced"
        );

        // Insurance fund took some hit, but limited
        assert!(
            engine.insurance_fund.balance >= 20_000,
            "Insurance fund protected by warmup rate limiting"
        );

        println!("✅ E2E test passed: Oracle manipulation attack protection works correctly");
        println!("   Attacker fake PNL: {}", attacker_fake_pnl);
        println!("   Attacker after ADL: {}", attacker_after_adl);
        println!(
            "   Attack mitigation: {}%",
            (attacker_fake_pnl - attacker_after_adl) * 100 / attacker_fake_pnl
        );
    }

    #[test]
    fn test_e2e_warmup_rate_limiting_stress() {
        // Scenario: Many users with large PNL, warmup capacity gets constrained

        let mut engine = Box::new(RiskEngine::new(default_params()));

        // Small insurance fund to test capacity limits
        engine.insurance_fund.balance = U128::new(20_000);

        let lp = engine.add_lp([1u8; 32], [2u8; 32], 10_000).unwrap();
        engine.accounts[lp as usize].capital = U128::new(500_000);
        engine.vault = U128::new(500_000);

        // Add 10 users
        let mut users = Vec::new();
        for _ in 0..10 {
            let user = engine.add_user(10_000).unwrap();
            engine.deposit(user, 5_000, 0).unwrap();
            users.push(user);
        }
        engine.vault = U128::new(550_000);

        // All users open large long positions
        for &user in &users {
            engine
                .execute_trade(&MATCHER, lp, user, 0, 1_000_000, 10_000)
                .unwrap();
        }

        // Price moves up 50% - huge unrealized PNL
        let boom_price = 1_500_000;

        // Close all positions to realize massive PNL
        for &user in &users {
            engine
                .execute_trade(&MATCHER, lp, user, 0, boom_price, -10_000)
                .unwrap();
            // execute_trade automatically calls update_warmup_slope() after PNL changes
        }

        // Each user should have large positive PNL (~5000 each = 50k total)
        let mut total_pnl = 0i128;
        for &user in &users {
            assert!(engine.accounts[user as usize].pnl.get() > 1_000);
            total_pnl += engine.accounts[user as usize].pnl.get();
        }
        println!("Total realized PNL across all users: {}", total_pnl);

        // Verify warmup rate limiting is enforced
        // Max warmup rate = insurance_fund * 0.5 / (T/2)
        // Note: Insurance fund may have increased from fees, so max_rate may be slightly higher
        let max_rate = engine.insurance_fund.balance * 5000 / 50 / 10_000;
        assert!(max_rate >= 200, "Max rate should be at least 200");

        println!("Insurance fund balance: {}", engine.insurance_fund.balance);
        println!("Calculated max warmup rate: {}", max_rate);
        println!("Actual total warmup rate: {}", engine.total_warmup_rate);

        // CRITICAL: Verify that warmup slopes were actually set by update_warmup_slope()
        // If total_warmup_rate is 0, it means update_warmup_slope() was never called
        assert!(engine.total_warmup_rate > 0,
            "Warmup slopes should be set after PNL changes (update_warmup_slope called by execute_trade)");

        // Total warmup rate should not exceed this (allow small rounding tolerance)
        assert!(
            engine.total_warmup_rate <= max_rate + 5,
            "Warmup rate {} significantly exceeds limit {}",
            engine.total_warmup_rate,
            max_rate
        );

        // CRITICAL: Verify rate limiting is actually constraining the system
        // Calculate what the total would be WITHOUT rate limiting
        let total_pnl_u128 = total_pnl as u128;
        let ideal_total_slope = total_pnl_u128 / engine.params.warmup_period_slots as u128;
        println!("Ideal total slope (no limiting): {}", ideal_total_slope);

        // If ideal > max_rate, then rate limiting MUST be active
        if ideal_total_slope > max_rate {
            assert_eq!(
                engine.total_warmup_rate, max_rate,
                "Rate limiting should cap total slope at max_rate when demand exceeds capacity"
            );
            println!(
                "✅ Rate limiting is ACTIVE: capped at {} (would be {} without limiting)",
                engine.total_warmup_rate, ideal_total_slope
            );
        } else {
            println!(
                "ℹ️  Rate limiting not triggered: demand ({}) below capacity ({})",
                ideal_total_slope, max_rate
            );
        }

        // Users with higher PNL should get proportionally more capacity
        // But sum of all slopes should be capped
        let total_slope: u128 = users
            .iter()
            .map(|&u| engine.accounts[u as usize].warmup_slope_per_step)
            .sum();

        assert_eq!(
            total_slope, engine.total_warmup_rate,
            "Sum of individual slopes must equal total_warmup_rate"
        );
        assert!(
            total_slope <= max_rate,
            "Total slope must not exceed max rate"
        );

        println!("✅ E2E test passed: Warmup rate limiting under stress works correctly");
        println!("   Total slope: {}, Max rate: {}", total_slope, max_rate);
    }
}
