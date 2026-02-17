# Kani Proof Strength Audit Results

Generated: 2026-02-17

154 proof harnesses analyzed across `/home/anatoly/percolator/tests/kani.rs`.

---

## Classification Summary

| Classification | Count | Description |
|---|---|---|
| STRONG | ~65 | Symbolic inputs exercise key branches, canonical_inv or appropriate invariant, non-vacuous |
| WEAK | ~40 | Symbolic but misses branches, uses weaker invariant, or has symbolic collapse |
| UNIT TEST | ~49 | Concrete inputs, single execution path (some intentionally so) |
| VACUOUS | 0 | All proofs have non-vacuity assertions or are trivially reachable |

---

## Critical Findings (sorted by severity)

### Tier 1: Proofs claiming invariant coverage but exercising only trivial paths

| Proof | Line | Issue |
|---|---|---|
| `proof_keeper_crank_preserves_inv` | 4432 | `keeper_crank` is ~200 lines with 9+ branch families (liquidation, force-close, force-realize, GC, funding, sweep). Proof has 1 user, no LP, no positions — exercises effectively zero core logic. Only `now_slot` is symbolic. |
| `proof_withdraw_preserves_inv` | 4125 | User has zero position, so the entire margin enforcement path (IM check, MTM equity, MM safety belt with revert) is **completely skipped**. Only proves the trivial no-position withdrawal. |
| `proof_liquidate_preserves_inv` | 4263 | All inputs concrete. Liquidation has rich branching (partial/full close, fallback safety, fee calc, insolvency writeoff) — tested for exactly one state. |
| `proof_deposit_preserves_inv` | 4088 | Fee settlement locked to no-op (slot=0, fees=0, pnl=0). Deposit's most interesting behavior (fee debt payment, warmup settlement during deposit) never exercised. |
| `proof_touch_account_full_preserves_inv` | 7182 | Massively overcollateralized (50K capital, position notional ~200). `Err(Undercollateralized)` from `settle_maintenance_fee` can never fire. |

### Tier 2: `valid_state()` used instead of `canonical_inv()`

These 6 proofs use the weaker `valid_state()` which omits: freelist acyclicity, aggregate coherence (c_tot, pnl_pos_tot, OI sums), accounting conservation (vault >= c_tot + insurance), and PA2 (i128::MIN guard).

| Proof | Line | Function tested |
|---|---|---|
| `fast_valid_preserved_by_deposit` | 1670 | deposit |
| `fast_valid_preserved_by_withdraw` | 1690 | withdraw |
| `fast_valid_preserved_by_execute_trade` | 1715 | execute_trade |
| `fast_valid_preserved_by_settle_warmup_to_capital` | 1747 | settle_warmup_to_capital |
| `fast_valid_preserved_by_top_up_insurance_fund` | 1792 | top_up_insurance_fund |
| `fast_valid_preserved_by_garbage_collect_dust` | 3503 | garbage_collect_dust |

### Tier 3: Liquidation proofs are all concrete

Every liquidation proof uses near-identical concrete setup: position at entry=oracle=1M, pnl=0.

| Proof | Line | Specific limitation |
|---|---|---|
| `proof_lq1_liquidation_reduces_oi_and_enforces_safety` | 2847 | Single undercollateralized scenario |
| `proof_lq2_liquidation_preserves_conservation` | 2912 | Single matching-position scenario |
| `proof_lq3a_profit_routes_through_adl` | 2967 | Single ADL scenario |
| `proof_lq4_liquidation_fee_paid_to_insurance` | 3040 | Single fee-accounting scenario |
| `proof_lq6_n1_boundary_after_liquidation` | 3127 | Identical to LQ1 setup |
| `proof_liq_partial_1_safety_after_liquidation` | 3176 | Single partial-fill scenario |
| `proof_liq_partial_2_dust_elimination` | 3220 | Same as partial_1 |
| `proof_liq_partial_3_routing_is_complete` | 3266 | Two concrete accounts |
| `proof_liq_partial_4_conservation_preservation` | 3342 | Non-zero PnL but still single path |
| `proof_liq_partial_deterministic` | 3393 | Same as partial_1 |

None test: symbolic oracle prices diverging from entry, short positions, large mark PnL, oracle near boundary values, or the "above maintenance margin" branch (no liquidation needed).

### Tier 4: Frame proofs with branch gaps

| Proof | Line | Issue |
|---|---|---|
| `fast_frame_settle_warmup_only_mutates_one_account_and_warmup_globals` | 1571 | `pnl > 0` constraint locks out the negative-PnL settlement path entirely |
| `fast_frame_deposit_only_mutates_one_account_vault_and_warmup` | 1416 | Fresh accounts bypass fee settlement, warmup settlement |
| `fast_frame_withdraw_only_mutates_one_account_vault_and_warmup` | 1461 | Fresh accounts, no position — margin check skipped |

### Tier 5: Other notable weaknesses

| Proof | Line | Issue |
|---|---|---|
| `proof_haircut_ratio_bounded` | 6934 | vault always >= obligations, so `!solvent` branch (defense-in-depth) never tested |
| `proof_profit_conversion_payout_formula` | 5498 | Warmup slope set so `cap >> avail_gross` — partial warmup conversion never exercised |
| `proof_liveness_after_loss_writeoff` | 5638 | Account A starts already zeroed — no actual write-off happens despite the name |
| `proof_gap1_crank_with_fees_preserves_inv` | 5943 | Only `fee_credits` symbolic. No liquidation, force-realize, or sweep paths exercised |
| `proof_lifecycle_trade_crash_settle_loss_conservation` | 7582 | Binary bool gives exactly 2 oracle prices, not a symbolic range |
| `proof_flaw2_no_phantom_equity_after_mark_settlement` | 7895 | Position/entry/oracle all concrete; only pnl is symbolic. mark_pnl is fixed at exactly 250. |
| `proof_gap3_multi_step_lifecycle_conservation` | 6239 | oracle_1 and size concrete; oracle_2 narrow [950K, 1050K]. Small mark PnL only. |
| `proof_trade_creates_funding_settled_positions` | 4711 | Only positive delta — short user position never tested |

---

## Complete Proof Table

### Lines 580-1500

| proof_name | line | classification | notes |
|---|---|---|---|
| `fast_i2_deposit_preserves_conservation` | 580 | WEAK | Uses conservation_fast_no_funding not canonical_inv. Fresh account, no fees/position. |
| `fast_i2_withdraw_preserves_conservation` | 604 | WEAK | Same weaker invariant. Margin checks unreachable (no position). |
| `i5_warmup_determinism` | 638 | STRONG | Symbolic pnl, reserved, slope, slots. All branches reachable. |
| `i5_warmup_monotonicity` | 667 | STRONG | Symbolic pnl, slope, slots1 < slots2. |
| `i5_warmup_bounded_by_pnl` | 700 | STRONG | Symbolic pnl, reserved, slope, slots. |
| `i7_user_isolation_deposit` | 736 | WEAK | Concrete operation on user1. Only capital/pnl checked for user2, not all fields. |
| `i7_user_isolation_withdrawal` | 771 | WEAK | Same partial isolation check. |
| `i8_equity_with_positive_pnl` | 810 | STRONG | Symbolic principal and positive pnl. |
| `i8_equity_with_negative_pnl` | 832 | STRONG | Symbolic principal and negative pnl. Both sides of max(0,...) reachable. |
| `withdrawal_requires_sufficient_balance` | 868 | WEAK | Only tests InsufficientBalance path. Other error paths unreachable. |
| `pnl_withdrawal_requires_warmup` | 894 | STRONG | Symbolic pnl and withdraw amount. |
| `saturating_arithmetic_prevents_overflow` | 938 | STRONG | Full u128 range. |
| `zero_pnl_withdrawable_is_zero` | 961 | UNIT TEST | All concrete. |
| `negative_pnl_withdrawable_is_zero` | 976 | STRONG | Symbolic negative pnl. |
| `funding_p1_settlement_idempotent` | 998 | STRONG | Symbolic position, pnl, index. |
| `funding_p2_never_touches_principal` | 1045 | STRONG | Symbolic principal, position, funding_delta. |
| `funding_p3_bounded_drift_between_opposite_positions` | 1080 | STRONG | Symbolic position and delta. |
| `funding_p4_settle_before_position_change` | 1132 | STRONG | Symbolic initial_pos, delta1, new_pos, delta2. |
| `funding_p5_bounded_operations_no_overflow` | 1208 | STRONG | Symbolic price, rate, dt. |
| `funding_p5_invalid_bounds_return_overflow` | 1255 | STRONG | Symbolic boolean selects error scenarios. |
| `funding_zero_position_no_change` | 1278 | STRONG | Symbolic pnl and delta. |
| `proof_warmup_slope_nonzero_when_positive_pnl` | 1316 | WEAK | Only positive pnl. avail_gross==0 path unreachable. |
| `fast_frame_touch_account_only_mutates_one_account` | 1352 | STRONG | Symbolic position and funding_delta. |
| `fast_frame_deposit_only_mutates_one_account_vault_and_warmup` | 1416 | WEAK | Fresh accounts bypass fee and warmup settlement. |
| `fast_frame_withdraw_only_mutates_one_account_vault_and_warmup` | 1461 | WEAK | Fresh accounts, no position, margin checks skipped. |

### Lines 1500-2500

| proof_name | line | classification | notes |
|---|---|---|---|
| `fast_frame_execute_trade_only_mutates_two_accounts` | 1508 | STRONG | Symbolic delta. Frame property verified. |
| `fast_frame_settle_warmup_only_mutates_one_account_and_warmup_globals` | 1571 | WEAK | pnl > 0 locked — negative PnL path untested. |
| `fast_frame_update_warmup_slope_only_mutates_one_account` | 1616 | WEAK | pnl > 0 locked — avail_gross==0 path untested. |
| `fast_valid_preserved_by_deposit` | 1670 | WEAK | Uses valid_state not canonical_inv. |
| `fast_valid_preserved_by_withdraw` | 1690 | WEAK | Uses valid_state. Oracle/crank hardcoded. |
| `fast_valid_preserved_by_execute_trade` | 1715 | WEAK | Uses valid_state. Capital hardcoded large. |
| `fast_valid_preserved_by_settle_warmup_to_capital` | 1747 | WEAK | Uses valid_state not canonical_inv. Good symbolic coverage otherwise. |
| `fast_valid_preserved_by_top_up_insurance_fund` | 1792 | WEAK | Uses valid_state. |
| `fast_neg_pnl_settles_into_capital_independent_of_warm_cap` | 1822 | STRONG | Symbolic capital and loss. |
| `fast_withdraw_cannot_bypass_losses_when_position_zero` | 1864 | STRONG | Symbolic capital and loss. |
| `fast_neg_pnl_after_settle_implies_zero_capital` | 1902 | STRONG | Symbolic capital, loss, slope. |
| `neg_pnl_settlement_does_not_depend_on_elapsed_or_slope` | 1936 | STRONG | Symbolic capital, loss, slope, elapsed. |
| `withdraw_calls_settle_enforces_pnl_or_zero_capital_post` | 1982 | STRONG | Symbolic capital, loss, withdraw_amt. |
| `fast_maintenance_margin_uses_equity_including_negative_pnl` | 2026 | STRONG | Symbolic capital, pnl, position. |
| `fast_account_equity_computes_correctly` | 2081 | STRONG | Symbolic capital and pnl. |
| `withdraw_im_check_blocks_when_equity_after_withdraw_below_im` | 2133 | UNIT TEST | All concrete. |
| `neg_pnl_is_realized_immediately_by_settle` | 2166 | UNIT TEST | All concrete (capital=10K, loss=3K). |
| `proof_fee_credits_never_inflate_from_settle` | 2211 | UNIT TEST | All concrete. |
| `proof_settle_maintenance_deducts_correctly` | 2240 | UNIT TEST | All concrete. |
| `proof_keeper_crank_advances_slot_monotonically` | 2279 | UNIT TEST | All concrete. |
| `proof_keeper_crank_best_effort_settle` | 2332 | UNIT TEST | All concrete. |
| `proof_close_account_requires_flat_and_paid` | 2362 | STRONG | Three symbolic booleans, 8 combinations. |
| `proof_total_open_interest_initial` | 2417 | UNIT TEST | Tests constructor, no inputs. |
| `proof_require_fresh_crank_gates_stale` | 2431 | STRONG | Symbolic now_slot. |
| `proof_stale_crank_blocks_withdraw` | 2463 | STRONG | Symbolic stale_slot. |
| `proof_stale_crank_blocks_execute_trade` | 2486 | STRONG | Symbolic stale_slot. |
| `proof_close_account_rejects_positive_pnl` | 2515 | UNIT TEST | All concrete, single rejection. |

### Lines 2500-3800

| proof_name | line | classification | notes |
|---|---|---|---|
| `proof_close_account_includes_warmed_pnl` | 2543 | UNIT TEST | All concrete. |
| `proof_close_account_negative_pnl_written_off` | 2593 | UNIT TEST | All concrete. |
| `proof_set_risk_reduction_threshold_updates` | 2624 | STRONG | Symbolic threshold. |
| `proof_trading_credits_fee_to_user` | 2647 | UNIT TEST | All concrete. |
| `proof_keeper_crank_forgives_half_slots` | 2696 | STRONG | Symbolic now_slot. |
| `proof_net_extraction_bounded_with_fee_credits` | 2767 | STRONG | Multiple symbolic inputs. |
| `proof_lq1_liquidation_reduces_oi_and_enforces_safety` | 2847 | UNIT TEST | All concrete. Single liq scenario. |
| `proof_lq2_liquidation_preserves_conservation` | 2912 | UNIT TEST | All concrete. |
| `proof_lq3a_profit_routes_through_adl` | 2967 | UNIT TEST | All concrete. |
| `proof_lq4_liquidation_fee_paid_to_insurance` | 3040 | UNIT TEST | All concrete. |
| `proof_keeper_crank_best_effort_liquidation` | 3096 | UNIT TEST | All concrete. |
| `proof_lq6_n1_boundary_after_liquidation` | 3127 | UNIT TEST | All concrete. |
| `proof_liq_partial_1_safety_after_liquidation` | 3176 | UNIT TEST | All concrete. |
| `proof_liq_partial_2_dust_elimination` | 3220 | UNIT TEST | All concrete. |
| `proof_liq_partial_3_routing_is_complete` | 3266 | UNIT TEST | All concrete. |
| `proof_liq_partial_4_conservation_preservation` | 3342 | UNIT TEST | All concrete. |
| `proof_liq_partial_deterministic` | 3393 | UNIT TEST | All concrete. |
| `gc_never_frees_account_with_positive_value` | 3446 | STRONG | Symbolic bool + values. |
| `fast_valid_preserved_by_garbage_collect_dust` | 3503 | WEAK | Uses valid_state not canonical_inv. |
| `gc_respects_full_dust_predicate` | 3538 | STRONG | Symbolic blocker + values. |
| `crank_bounds_respected` | 3603 | STRONG | Symbolic now_slot. |
| `gc_frees_only_true_dust` | 3660 | UNIT TEST | All concrete multi-account. |
| `withdrawal_maintains_margin_above_maintenance` | 3722 | STRONG | Multiple symbolic inputs. Non-vacuity forced. |
| `withdrawal_rejects_if_below_initial_margin_at_oracle` | 3786 | UNIT TEST | All concrete. |

### Lines 3800-5200

| proof_name | line | classification | notes |
|---|---|---|---|
| `proof_inv_holds_for_new_engine` | 3816 | UNIT TEST | Valid base case. |
| `proof_inv_preserved_by_add_user` | 3842 | WEAK | Only fee is symbolic. Capacity/fee-short branches locked. |
| `proof_inv_preserved_by_add_lp` | 3872 | WEAK | Same as add_user. |
| `proof_execute_trade_preserves_inv` | 3905 | STRONG | Symbolic delta and oracle. canonical_inv. |
| `proof_execute_trade_conservation` | 3971 | STRONG | Symbolic cap, delta, price. |
| `proof_execute_trade_margin_enforcement` | 4021 | STRONG | Symbolic delta and price. |
| `proof_deposit_preserves_inv` | 4088 | WEAK | Fee settlement locked to no-op. |
| `proof_withdraw_preserves_inv` | 4125 | WEAK | No position — margin path skipped entirely. |
| `proof_add_user_structural_integrity` | 4169 | WEAK | inv_structural not canonical_inv. |
| `proof_close_account_structural_integrity` | 4203 | WEAK | inv_structural not canonical_inv. |
| `proof_liquidate_preserves_inv` | 4263 | WEAK | All concrete inputs. |
| `proof_settle_warmup_preserves_inv` | 4320 | STRONG | 8 symbolic inputs. All positive-PnL branches. |
| `proof_settle_warmup_negative_pnl_immediate` | 4377 | STRONG | 3 symbolic inputs. All negative-PnL branches. |
| `proof_keeper_crank_preserves_inv` | 4432 | WEAK | 1 user, no position, no LP. Crank logic untested. |
| `proof_gc_dust_preserves_inv` | 4470 | WEAK | All concrete dust account. |
| `proof_gc_dust_structural_integrity` | 4506 | WEAK | inv_structural not canonical_inv. |
| `proof_close_account_preserves_inv` | 4536 | STRONG | canonical_inv checked on close success path. |
| `proof_top_up_insurance_preserves_inv` | 4582 | STRONG | Symbolic amount. |
| `proof_sequence_deposit_trade_liquidate` | 4626 | UNIT TEST | All concrete. Liquidation not actually triggered. |
| `proof_sequence_deposit_crank_withdraw` | 4661 | WEAK | Partially symbolic. Margin rejection unreachable. |
| `proof_trade_creates_funding_settled_positions` | 4711 | WEAK | Only positive delta — no short positions. |
| `proof_crank_with_funding_preserves_inv` | 4767 | STRONG | Symbolic funding_rate. |
| `proof_variation_margin_no_pnl_teleport` | 4833 | STRONG | Symbolic open/close price, size. |
| `proof_trade_pnl_zero_sum` | 4927 | STRONG | Symbolic oracle and size. |
| `kani_no_teleport_cross_lp_close` | 5004 | UNIT TEST | All concrete. |
| `kani_rejects_invalid_matcher_output` | 5116 | UNIT TEST | Valid targeted rejection test. |

### Lines 5200-6500

| proof_name | line | classification | notes |
|---|---|---|---|
| `kani_cross_lp_close_no_pnl_teleport` | 5213 | UNIT TEST | All concrete. |
| `proof_haircut_ratio_formula_correctness` | 5279 | STRONG | 4 symbolic inputs. All haircut branches. |
| `proof_effective_equity_with_haircut` | 5352 | STRONG | 6 symbolic inputs. |
| `proof_principal_protection_across_accounts` | 5426 | STRONG | 4 symbolic inputs. Insolvency path exercised. |
| `proof_profit_conversion_payout_formula` | 5498 | WEAK | Warmup branch locked to full-conversion. |
| `proof_rounding_slack_bound` | 5575 | STRONG | 5 symbolic inputs, underbacked forced. |
| `proof_liveness_after_loss_writeoff` | 5638 | WEAK | No actual writeoff happens. |
| `proof_gap1_touch_account_err_no_mutation` | 5847 | UNIT TEST | Valid targeted Err-path test. |
| `proof_gap1_settle_mark_err_no_mutation` | 5894 | UNIT TEST | Valid targeted Err-path test. |
| `proof_gap1_crank_with_fees_preserves_inv` | 5943 | WEAK | Only fee_credits symbolic. Crank paths locked. |
| `proof_gap2_rejects_overfill_matcher` | 5994 | UNIT TEST | Valid boundary enforcement test. |
| `proof_gap2_rejects_zero_price_matcher` | 6019 | UNIT TEST | Valid boundary enforcement test. |
| `proof_gap2_rejects_max_price_exceeded_matcher` | 6044 | UNIT TEST | Valid boundary enforcement test. |
| `proof_gap2_execute_trade_err_preserves_inv` | 6072 | STRONG | Symbolic caps and size. canonical_inv on Err. |
| `proof_gap3_conservation_trade_entry_neq_oracle` | 6123 | STRONG | Symbolic oracle pair and size. MTM path. |
| `proof_gap3_conservation_crank_funding_positions` | 6183 | STRONG | Symbolic oracle and funding_rate. |
| `proof_gap3_multi_step_lifecycle_conservation` | 6239 | WEAK | Only oracle_2 and funding_rate symbolic. Narrow ranges. |
| `proof_gap4_trade_extreme_price_no_panic` | 6302 | UNIT TEST | 3 concrete price points. |
| `proof_gap4_trade_extreme_size_no_panic` | 6366 | UNIT TEST | 3 concrete size points. |
| `proof_gap4_trade_partial_fill_diff_price_no_panic` | 6427 | STRONG | Symbolic oracle and size. |
| `proof_gap4_margin_extreme_values_no_panic` | 6465 | UNIT TEST | Concrete extreme values. |
| `proof_gap5_fee_settle_margin_or_err` | 6520 | STRONG | Symbolic cap, size, fee_credits, slot. |
| `proof_gap5_fee_credits_trade_then_settle_bounded` | 6593 | WEAK | Only dt symbolic. |
| `proof_gap5_fee_credits_saturating_near_max` | 6662 | UNIT TEST | All concrete near-max values. |
| `proof_gap5_deposit_fee_credits_conservation` | 6707 | STRONG | Symbolic amount. |

### Lines 6500-8100

| proof_name | line | classification | notes |
|---|---|---|---|
| `proof_set_pnl_maintains_pnl_pos_tot` | 6771 | STRONG | Symbolic initial/new pnl. |
| `proof_set_capital_maintains_c_tot` | 6800 | STRONG | Symbolic initial/new capital. |
| `proof_force_close_with_set_pnl_preserves_invariant` | 6831 | STRONG | 4 symbolic inputs. |
| `proof_multiple_force_close_preserves_invariant` | 6884 | STRONG | Symbolic positions and settlement price. |
| `proof_haircut_ratio_bounded` | 6934 | STRONG (minor) | !solvent branch never reached (vault always solvent). |
| `proof_effective_pnl_bounded_by_actual` | 6963 | STRONG | Symbolic capital and pnl. |
| `proof_recompute_aggregates_correct` | 6992 | STRONG | Symbolic capital and pnl. |
| `proof_NEGATIVE_bypass_set_pnl_breaks_invariant` | 7032 | STRONG | should_panic negative proof. |
| `proof_settle_mark_to_oracle_preserves_inv` | 7075 | STRONG | Symbolic pos and oracle. |
| `proof_touch_account_preserves_inv` | 7128 | STRONG | Symbolic pos and funding_delta. |
| `proof_touch_account_full_preserves_inv` | 7182 | WEAK | Massively overcollateralized. Err paths unreachable. |
| `proof_settle_loss_only_preserves_inv` | 7243 | STRONG | Symbolic capital and loss. |
| `proof_accrue_funding_preserves_inv` | 7300 | STRONG | Symbolic rate, slot, oracle. |
| `proof_init_in_place_satisfies_inv` | 7355 | UNIT TEST | Valid base case. |
| `proof_set_pnl_preserves_conservation` | 7381 | STRONG | Symbolic initial/new pnl. |
| `proof_set_capital_decrease_preserves_conservation` | 7431 | WEAK | Only decrease path tested. |
| `proof_set_capital_aggregate_correct` | 7461 | STRONG | Both directions. |
| `proof_lifecycle_trade_then_touch_full_conservation` | 7513 | WEAK | Mostly concrete. Narrow symbolic range. |
| `proof_lifecycle_trade_crash_settle_loss_conservation` | 7582 | WEAK | Binary bool — exactly 2 paths, not symbolic. |
| `proof_lifecycle_trade_warmup_withdraw_topup_conservation` | 7638 | UNIT TEST | All concrete. |
| `proof_lifecycle_trade_warmup_withdraw_topup_conservation_alt_oracle` | 7712 | UNIT TEST | All concrete (alt oracle). |
| `proof_flaw1_debt_writeoff_requires_flat_position` | 7791 | UNIT TEST | Valid targeted rebuttal. |
| `proof_flaw1_gc_never_writes_off_with_open_position` | 7848 | UNIT TEST | Valid targeted rebuttal. |
| `proof_flaw2_no_phantom_equity_after_mark_settlement` | 7895 | WEAK | Only pnl symbolic. Position/entry/oracle concrete. |
| `proof_flaw2_withdraw_settles_before_margin_check` | 7967 | WEAK | Only w_amount symbolic. |
| `proof_flaw3_warmup_reset_increases_slope_proportionally` | 8038 | STRONG | Symbolic pnl1 and pnl2. |
| `proof_flaw3_warmup_converts_after_single_slot` | 8086 | STRONG | Symbolic pnl. |

---

## Recommended Strengthening Priority

**P0 — High-value, directly actionable:**
1. `proof_liquidate_preserves_inv` → symbolic oracle, capital, position size
2. `proof_withdraw_preserves_inv` → add non-zero position to exercise margin path
3. `proof_keeper_crank_preserves_inv` → add undercollateralized account to trigger liquidation/GC

**P1 — Medium effort, good coverage gain:**
4. `proof_deposit_preserves_inv` → non-zero slot/fees/pnl to exercise fee settlement
5. `proof_touch_account_full_preserves_inv` → reduce capital so Undercollateralized path is reachable
6. Liquidation suite (LQ1-LQ6, LIQ-PARTIAL-*) → at least one symbolic proof with oracle != entry

**P2 — Lower priority but systematic:**
7. Upgrade `fast_valid_preserved_by_*` proofs to use `canonical_inv` (or add parallel canonical_inv versions)
8. Frame proofs for deposit/withdraw/settle_warmup → use non-fresh accounts with fees/positions
