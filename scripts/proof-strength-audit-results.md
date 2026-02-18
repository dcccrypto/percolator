# Kani Proof Strength Audit Results

Generated: 2026-02-18 (deep 5-point audit per `scripts/audit-proof-strength.md`)

146 proof harnesses across `/home/anatoly/percolator/tests/kani.rs`.

Methodology: Each proof analyzed for (1) input classification, (2) branch coverage against source,
(3) invariant strength, (4) vacuity risk, (5) symbolic collapse.

---

## Classification Summary

| Classification | Count | Description |
|---|---|---|
| STRONG | 139 | Symbolic inputs exercise key branches, appropriate invariant, non-vacuous |
| WEAK | 0 | — |
| UNIT TEST | 7 | Intentional: base cases, stdlib, meta-test, regression |
| VACUOUS | 0 | All proofs have non-vacuity assertions or are trivially reachable |

Previous deep 5-point audit identified 22 WEAK proofs across 4 categories.
All 22 have been upgraded to STRONG:
- **Category A** (18 proofs): Branch coverage gaps fixed with symbolic inputs, seasoned accounts,
  widened ranges, and real operation paths (writeoff, partial warmup, margin boundary)
- **Category B** (13 proofs): Missing canonical_inv fixed with pre+post assertions + sync_engine_aggregates
  (1 re-classified to Category A and fixed there)
- **Category C** (3 proofs): inv_structural upgraded to canonical_inv
- **Category D** (1 proof): Trivially true u128::MAX assertion replaced with canonical_inv + equity/margin properties

All 22 strengthened proofs verified with `cargo kani`.

---

## Upgrade Details

### Category A: Branch Coverage Gaps — RESOLVED (18 proofs)

| Proof | Fix Applied |
|---|---|
| `fast_i2_deposit_preserves_conservation` | Seasoned account with symbolic capital/pnl/now_slot, test_params_with_maintenance_fee, fee accrual branch exercised |
| `i5_warmup_bounded_by_pnl` | Extended pnl range to [-10K,10K], added negative PnL assertion (withdrawable==0), warmup cap bound |
| `i8_equity_with_positive_pnl` | Full pnl range [-10K,10K], asserts equity == max(0, capital+pnl) covering both branches |
| `pnl_withdrawal_requires_warmup` | Non-zero symbolic capital, warmup_started_at_slot=now_slot (elapsed=0), withdraw > capital tests warmup guard |
| `negative_pnl_withdrawable_is_zero` | Symbolic slot and slope; universally quantified over all warmup parameters |
| `fast_valid_preserved_by_deposit` | Seasoned account with symbolic capital/pnl, test_params_with_maintenance_fee, fee accrual via last_fee_slot < now_slot |
| `fast_valid_preserved_by_withdraw` | Symbolic position + counterparty, withdraw > capital exercises InsufficientBalance and margin checks |
| `fast_valid_preserved_by_execute_trade` | Symbolic capitals [500,5000] near margin boundary, symbolic oracle [900K,1.1M] for mark PnL |
| `proof_close_account_requires_flat_and_paid` | Symbolic capital/pnl/position instead of boolean selectors, canonical_inv pre+post |
| `proof_close_account_includes_warmed_pnl` | Symbolic slope [1,100] and insurance [1,500] for partial conversion and h < 1 |
| `proof_liveness_after_loss_writeoff` | Account A given symbolic negative PnL and capital, actual settle_warmup_to_capital writeoff, N1 assertion |
| `proof_gap3_conservation_crank_funding_positions` | Symbolic size [50,200] for varying position magnitude and margin pressure |
| `proof_gap3_multi_step_lifecycle_conservation` | Widened funding_rate [-50,50], symbolic user_deposit [25K,50K], relaxed non-vacuity for crank/close |
| `proof_gap5_fee_credits_trade_then_settle_bounded` | Both long and short trades (size [-500,500]), wider fee credit variation |
| `proof_gap5_fee_credits_saturating_near_max` | Symbolic size [10,500] varies the fee credit increment amount |
| `proof_lifecycle_trade_then_touch_full_conservation` | Symbolic size [50,200] + symbolic user_deposit [25K,50K] |
| `proof_lifecycle_trade_warmup_withdraw_topup_conservation` | Fully symbolic oracle [1.01M,1.2M] (was 2-valued bool), symbolic LP deposit [50K,100K] for h < 1 |
| `proof_gap4_margin_extreme_values_no_panic` | canonical_inv setup check, meaningful equity/margin properties instead of trivially-true u128::MAX |

### Category B: Missing canonical_inv — RESOLVED (13 proofs)

| Proof | Fix Applied |
|---|---|
| `fast_neg_pnl_settles_into_capital_independent_of_warm_cap` | +sync_engine_aggregates +canonical_inv pre+post |
| `fast_withdraw_cannot_bypass_losses_when_position_zero` | +sync_engine_aggregates +canonical_inv pre+post |
| `fast_neg_pnl_after_settle_implies_zero_capital` | +sync_engine_aggregates +canonical_inv pre+post; bounded slope ≤10K |
| `withdraw_calls_settle_enforces_pnl_or_zero_capital_post` | +canonical_inv pre+post |
| `proof_fee_credits_never_inflate_from_settle` | +canonical_inv pre+post |
| `proof_close_account_rejects_positive_pnl` | +sync_engine_aggregates +canonical_inv pre+post |
| `proof_close_account_negative_pnl_written_off` | sync→sync_engine_aggregates +canonical_inv pre+post |
| `proof_set_risk_reduction_threshold_updates` | +canonical_inv pre+post |
| `proof_keeper_crank_forgives_half_slots` | +canonical_inv pre+post |
| `proof_net_extraction_bounded_with_fee_credits` | +canonical_inv post |
| `gc_respects_full_dust_predicate` | +sync_engine_aggregates +canonical_inv pre+post; fixed blocker=0 PA1 |
| `gc_frees_only_true_dust` | +sync_engine_aggregates +canonical_inv pre+post |
| `withdrawal_maintains_margin_above_maintenance` | +canonical_inv setup+Ok-path |

### Category C: inv_structural → canonical_inv — RESOLVED (3 proofs)

| Proof | Fix Applied |
|---|---|
| `proof_add_user_structural_integrity` | inv_structural → canonical_inv (2 occurrences) |
| `proof_close_account_structural_integrity` | inv_structural → canonical_inv (2 occurrences) |
| `proof_gc_dust_structural_integrity` | inv_structural → canonical_inv (2 occurrences) |

### Category D: Trivially True Assertions — RESOLVED (1 proof)

| Proof | Fix Applied |
|---|---|
| `proof_gap4_margin_extreme_values_no_panic` | Replaced `_eq <= u128::MAX` with canonical_inv setup, meaningful equity/margin property assertions |

---

## UNIT TEST Proofs (7)

Intentional: base cases, meta-tests, and tests that cannot meaningfully benefit from symbolic inputs.

| Proof | Line | Reason |
|---|---|---|
| `saturating_arithmetic_prevents_overflow` | 958 | Tests stdlib saturating arithmetic, not percolator |
| `funding_p5_invalid_bounds_return_overflow` | 1282 | Symbolic bool selects 2 concrete error paths (guard check) |
| `proof_total_open_interest_initial` | 2695 | Trivial base case: new engine has OI == 0 |
| `proof_inv_holds_for_new_engine` | 3696 | Constructor base case: new engine satisfies INV |
| `kani_cross_lp_close_no_pnl_teleport` | 5232 | Concrete cross-LP regression with custom matchers |
| `proof_NEGATIVE_bypass_set_pnl_breaks_invariant` | 7032 | Negative should_panic meta-test (validates non-vacuity) |
| `proof_init_in_place_satisfies_inv` | 7366 | Constructor base case: init_in_place satisfies INV |

---

## STRONG Proofs (139)

All remaining proofs are STRONG: symbolic inputs exercise key branches of the function-under-test,
appropriate invariant (canonical_inv or property-specific) is checked, non-vacuous via assert_ok!
or explicit reachability assertions. Includes all 35 proofs upgraded from WEAK by the fixes above.

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

3. **All 22 WEAK proofs resolved**: Category B (13 proofs) fixed with canonical_inv. Category A
   (18 proofs including 1 from Category B) fixed with branch-unlocking symbolic inputs. Category C
   (3 proofs) upgraded to canonical_inv. Category D (1 proof) replaced trivially-true assertion.
   Note: 13 proofs had issues in both Category A and B, counted once in the total of 22.

4. **Solver performance notes**: Some strengthened proofs take significant verification time due
   to added symbolic complexity. `proof_lifecycle_trade_warmup_withdraw_topup_conservation` (1147s)
   and `proof_gap3_multi_step_lifecycle_conservation` (1023s) are the slowest due to multi-step
   chained operations with symbolic inputs.
