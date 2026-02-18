# Kani Proof Strength Audit Results

Generated: 2026-02-18 (deep 5-point audit per `scripts/audit-proof-strength.md`)

146 proof harnesses across `/home/anatoly/percolator/tests/kani.rs`.

Methodology: Each proof analyzed for (1) input classification, (2) branch coverage against source,
(3) invariant strength, (4) vacuity risk, (5) symbolic collapse.

---

## Classification Summary

| Classification | Count | Description |
|---|---|---|
| STRONG | 117 | Symbolic inputs exercise key branches, appropriate invariant, non-vacuous |
| WEAK | 22 | Misses branches, uses weaker invariant, or symbolic collapse (see details) |
| UNIT TEST | 7 | Intentional: base cases, stdlib, meta-test, regression |
| VACUOUS | 0 | All proofs have non-vacuity assertions or are trivially reachable |

Previous simple audit: 139 STRONG, 0 WEAK, 7 UNIT TEST.
The deep 5-point audit reclassified 34 proofs from STRONG to WEAK using stricter criteria:
branch coverage against source code, full invariant checks, and symbolic collapse analysis.

Post-audit fix: 13 Category B proofs upgraded WEAK→STRONG by adding canonical_inv pre+post
assertions and sync_engine_aggregates where missing. 1 Category B proof (proof_liveness_after_loss_writeoff)
already had canonical_inv; its issue is branch coverage (Category A), not missing invariant.
All 13 strengthened proofs verified with `cargo kani`.

---

## WEAK Proofs by Category

### Category A: Branch Coverage Gaps (symbolic inputs but key branches locked)

| Proof | Line | Issue | Recommendation |
|---|---|---|---|
| `fast_i2_deposit_preserves_conservation` | 580 | All 6 deposit() branches locked (fresh account at slot=0: no fees, no PnL, no warmup) | Add symbolic pnl/fee_credits/now_slot to unlock branches |
| `i5_warmup_bounded_by_pnl` | 708 | pnl > 0 only; negative PnL path never tested | Extend pnl range to include negatives or document companion coverage |
| `i8_equity_with_positive_pnl` | 822 | Only exercises eq_i > 0 branch; flooring-at-zero branch unreachable | Paired with line 844 (negative); consider merging for single STRONG proof |
| `pnl_withdrawal_requires_warmup` | 914 | capital=0 means withdrawal fails on InsufficientBalance, not warmup guard | Give non-zero capital; test warmup-specific rejection |
| `negative_pnl_withdrawable_is_zero` | 1003 | Concrete slot=1000; property is trivially true (clamp_pos_i128 of negative = 0) | Make slot symbolic; property inherently trivial but universality claim weak |
| `fast_valid_preserved_by_deposit` | 1763 | Fresh account: fee accrual (dt=0), fee debt, warmup, loss settlement all locked to no-op | Add symbolic capital/pnl/now_slot; use test_params_with_maintenance_fee |
| `fast_valid_preserved_by_withdraw` | 1783 | No position: margin checks never exercised; withdraw <= deposit always succeeds | Add symbolic position; allow withdraw > deposit to test InsufficientBalance |
| `fast_valid_preserved_by_execute_trade` | 1808 | Concrete 100K capitals: h=1 always, margin always passes; NoOpMatcher: trade PnL=0 | Reduce capitals near margin boundary; add pre-existing positions |
| `proof_close_account_requires_flat_and_paid` | 2500 | 3 boolean selectors (8 paths) with concrete values; closer to enumerated test | Use symbolic capital/pnl/position values instead of boolean selectors |
| `proof_close_account_includes_warmed_pnl` | 2709 | slope=10K, elapsed=200: warmup cap >> pnl, so conversion always 100%; h always 1 | Symbolic slope/insurance to exercise partial conversion and h < 1 |
| `proof_gap3_conservation_crank_funding_positions` | 6169 | oracle_1=1M concrete locks entry price; funding delta too small for meaningful settlement | Make oracle_1 and size symbolic |
| `proof_gap3_multi_step_lifecycle_conservation` | 6229 | oracle_1=1M concrete; funding_rate range (-10,10) negligible; over-capitalized accounts | Widen funding range; reduce capital near margin |
| `proof_gap5_fee_credits_trade_then_settle_bounded` | 6568 | Concrete oracle: NoOpMatcher trade PnL always 0; fee formula hard-coded | Make oracle symbolic to exercise PnL+fee interactions |
| `proof_gap5_fee_credits_saturating_near_max` | 6642 | trade size=50 concrete; only tests i128::MAX neighborhood saturation | Make size symbolic to vary fee credit increment |
| `proof_lifecycle_trade_then_touch_full_conservation` | 7525 | size=100 concrete; 50K capital >> margin; mark/funding trivial vs deposits | Make size symbolic; reduce capital near margin |
| `proof_lifecycle_trade_warmup_withdraw_topup_conservation` | 7652 | Oracle 2-valued (bool), not symbolic; massively solvent so h=1 always | Make oracle fully symbolic; reduce LP deposit for h < 1 |

### Category B: Missing canonical_inv — RESOLVED (13 of 14 upgraded to STRONG)

All proofs below now have canonical_inv pre+post assertions and proper sync_engine_aggregates.
Verified with `cargo kani` (all pass).

| Proof | Line | Fix Applied |
|---|---|---|
| `fast_neg_pnl_settles_into_capital_independent_of_warm_cap` | 1909 | +sync_engine_aggregates +canonical_inv pre+post |
| `fast_withdraw_cannot_bypass_losses_when_position_zero` | 1951 | +sync_engine_aggregates +canonical_inv pre+post |
| `fast_neg_pnl_after_settle_implies_zero_capital` | 1989 | +sync_engine_aggregates +canonical_inv pre+post; bounded slope ≤10K |
| `withdraw_calls_settle_enforces_pnl_or_zero_capital_post` | 2069 | +canonical_inv pre+post |
| `proof_fee_credits_never_inflate_from_settle` | 2320 | +canonical_inv pre+post |
| `proof_close_account_rejects_positive_pnl` | 2677 | +sync_engine_aggregates +canonical_inv pre+post |
| `proof_close_account_negative_pnl_written_off` | 2762 | sync→sync_engine_aggregates +canonical_inv pre+post |
| `proof_set_risk_reduction_threshold_updates` | 2796 | +canonical_inv pre+post |
| `proof_keeper_crank_forgives_half_slots` | 2868 | +canonical_inv pre+post |
| `proof_net_extraction_bounded_with_fee_credits` | 2939 | +canonical_inv post |
| `gc_respects_full_dust_predicate` | 3388 | +sync_engine_aggregates +canonical_inv pre+post; fixed blocker=0 PA1 |
| `gc_frees_only_true_dust` | 3517 | +sync_engine_aggregates +canonical_inv pre+post |
| `withdrawal_maintains_margin_above_maintenance` | 3585 | +canonical_inv setup+Ok-path |

Remaining Category B (1 proof, reclassified to Category A):

| Proof | Line | Issue | Recommendation |
|---|---|---|---|
| `proof_liveness_after_loss_writeoff` | 5659 | Already has canonical_inv; real issue is branch coverage (A pre-resolved, no writeoff exercised) | Give A negative PnL, settle, then verify B can withdraw |

### Category C: Uses inv_structural instead of canonical_inv

| Proof | Line | Issue | Recommendation |
|---|---|---|---|
| `proof_add_user_structural_integrity` | 4082 | Uses inv_structural; misses inv_accounting, inv_aggregates, inv_per_account | Upgrade to canonical_inv; or document as intentional structural test |
| `proof_close_account_structural_integrity` | 4128 | Uses inv_structural; same gap | Upgrade to canonical_inv |
| `proof_gc_dust_structural_integrity` | 4467 | Uses inv_structural; strictly weaker than proof_gc_dust_preserves_inv (L4423) | Upgrade to canonical_inv or remove (subsumed) |

### Category D: Trivially True Assertions

| Proof | Line | Issue | Recommendation |
|---|---|---|---|
| `proof_gap4_margin_extreme_values_no_panic` | 6448 | Asserts `_eq <= u128::MAX` which is trivially true for any u128 | Add meaningful assertion (e.g., equity monotonicity with capital) |

---

## UNIT TEST Proofs (7)

Intentional: base cases, meta-tests, and tests that cannot meaningfully benefit from symbolic inputs.

| Proof | Line | Reason |
|---|---|---|
| `saturating_arithmetic_prevents_overflow` | 958 | Tests stdlib saturating arithmetic, not percolator |
| `funding_p5_invalid_bounds_return_overflow` | 1282 | Symbolic bool selects 2 concrete error paths (guard check) |
| `proof_total_open_interest_initial` | 2555 | Trivial base case: new engine has OI == 0 |
| `proof_inv_holds_for_new_engine` | 3696 | Constructor base case: new engine satisfies INV |
| `kani_cross_lp_close_no_pnl_teleport` | 5232 | Concrete cross-LP regression with custom matchers |
| `proof_NEGATIVE_bypass_set_pnl_breaks_invariant` | 7032 | Negative should_panic meta-test (validates non-vacuity) |
| `proof_init_in_place_satisfies_inv` | 7366 | Constructor base case: init_in_place satisfies INV |

---

## STRONG Proofs (117)

All remaining proofs are STRONG: symbolic inputs exercise key branches of the function-under-test,
appropriate invariant (canonical_inv or property-specific) is checked, non-vacuous via assert_ok!
or explicit reachability assertions. Includes 13 proofs upgraded from Category B by adding
canonical_inv pre+post assertions.

Notable strongest proofs:
- `proof_lq7_symbolic_oracle_liquidation` (L3126): canonical_inv + OI + dust + N1 boundary
- `proof_liq_partial_symbolic` (L3195): partial+full close with MM guard
- `proof_variation_margin_no_pnl_teleport` (L4846): dual-engine comparison with symbolic prices
- `proof_haircut_ratio_formula_correctness` (L5296): 7 properties across all haircut branches
- `proof_settle_warmup_preserves_inv` (L4246): 8 symbolic inputs covering all section 6.2 branches
- `proof_touch_account_full_preserves_inv` (L7183): 4 symbolic inputs through full 6-step settlement

---

## Cross-Cutting Observations

1. **No vacuous proofs**: Every proof with Ok-path assertions includes `assert_ok!` or explicit
   `assert!(result.is_ok())` non-vacuity checks. The negative proof uses `#[kani::should_panic]`.

2. **NoOpMatcher limitation**: All execute_trade proofs use NoOpMatcher (exec_price = oracle_price),
   so trade_pnl is always 0. No proof exercises non-zero trade PnL from the matching engine.
   `proof_gap4_trade_partial_fill_diff_price_no_panic` is the only exception (uses PartialFillDiffPriceMatcher).

3. **Category B resolved** (13 of 14 proofs): These formerly asserted correct functional behavior
   without the full 5-component invariant check. All 13 now have `canonical_inv` pre+post and
   are upgraded to STRONG. The remaining 1 (proof_liveness_after_loss_writeoff) already had
   canonical_inv; its issue is branch coverage and was reclassified to Category A.

4. **Category A proofs with locked branches** are harder to fix: they require restructuring the
   engine state setup (adding pre-existing positions, PnL, fee history) to unlock the dormant branches.

5. **Systemic pattern**: Fresh-account proofs (Categories A1-A3) always lock deposit/withdraw
   branches because `new() + add_user()` produces a zero-state account where fees=0, pnl=0,
   warmup=inactive. A helper function building a "seasoned account" with symbolic state would
   strengthen many proofs at once.
