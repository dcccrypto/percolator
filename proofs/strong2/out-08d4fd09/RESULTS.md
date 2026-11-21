# strong2 results (GenR08d4fd09)

Pin: engine: aeyakovenko/percolator@08d4fd09, src/v16.rs blob a4674830cfcd5eeaa6cb39696a0620db7c5b0a9b; rustc 1.99.0-nightly (d0babd8b6 2026-07-15); cargo rustc -j 4 --release --lib -- -Zunpretty=mir -Zinline-mir=no

`lake build`: Build completed successfully (114 jobs).. Modules generated: 53; with `main` on axioms [propext, Quot.sound]: 53.

## Ledger rows (statement: every `Ok` return; `est` = the check holds after, `pres` = it holds after if it held before)

- `.header` rows kernel-checked: **33** (23 est, 10 pres)
- `.account` rows kernel-checked: **12** (8 est, 4 pres)
- of these, rows the sibling run (`proofs/novel`) left open on the same text: **T18.header, T18.account**
- rows the sibling run closed that this run does not: none

| Row | Function | Class | This run | Sibling | Lean theorem | Status |
|---|---|---|---|---|---|---|
| T06.header | `add_account_source_positive_pnl_not_atomic` | final | pres | pres | `S_add_account_source_positive_pnl_not_atomic.row` | kernel-checked |
| T06.account | `add_account_source_positive_pnl_not_atomic` | - | est | est | `S_add_account_source_positive_pnl_not_atomic.row` | kernel-checked |
| T11.header | `canonicalize_retired_empty_asset_slot_not_atomic` | final | est | est | `S_canonicalize_retired_empty_asset_slot_not_atomic.row` | kernel-checked |
| T12.header | `canonicalize_spent_backing_history_not_atomic` | final | est | est | `S_canonicalize_spent_backing_history_not_atomic.row` | kernel-checked |
| T13.header | `charge_account_backing_fee_not_atomic` | final | pres | pres | `S_charge_account_backing_fee_not_atomic.row` | kernel-checked |
| T13.account | `charge_account_backing_fee_not_atomic` | - | pres | pres | `S_charge_account_backing_fee_not_atomic.row` | kernel-checked |
| T14.header | `claim_resolved_payout_topup_not_atomic` | final | est | est | `S_claim_resolved_payout_topup_not_atomic.row` | kernel-checked |
| T14.account | `claim_resolved_payout_topup_not_atomic` | - | est | est | `S_claim_resolved_payout_topup_not_atomic.row` | kernel-checked |
| T18.header | `convert_released_pnl_to_capital_not_atomic` | final | pres | open | `S_convert_released_pnl_to_capital_not_atomic.row` | kernel-checked |
| T18.account | `convert_released_pnl_to_capital_not_atomic` | - | pres | open | `S_convert_released_pnl_to_capital_not_atomic.row` | kernel-checked |
| T21.header | `credit_account_from_insurance_not_atomic` | final | pres | pres | `S_credit_account_from_insurance_not_atomic.row` | kernel-checked |
| T21.account | `credit_account_from_insurance_not_atomic` | - | pres | pres | `S_credit_account_from_insurance_not_atomic.row` | kernel-checked |
| T22.header | `credit_backing_provider_earnings_not_atomic` | final | pres | pres | `S_credit_backing_provider_earnings_not_atomic.row` | kernel-checked |
| T23.header | `credit_domain_insurance_budget_not_atomic` | delegates | est | est | `S_credit_domain_insurance_budget_not_atomic.row` | kernel-checked |
| T24.header | `credit_domain_insurance_budgets_not_atomic` | final | est | est | `S_credit_domain_insurance_budgets_not_atomic.row` | kernel-checked |
| T25.header | `cure_and_cancel_close_not_atomic` | final | est | est | `S_cure_and_cancel_close_not_atomic.row` | kernel-checked |
| T25.account | `cure_and_cancel_close_not_atomic` | - | est | est | `S_cure_and_cancel_close_not_atomic.row` | kernel-checked |
| T26.header | `deposit_domain_insurance_not_atomic` | final | est | est | `S_deposit_domain_insurance_not_atomic.row` | kernel-checked |
| T27.header | `deposit_fresh_counterparty_backing_not_atomic` | final | est | est | `S_deposit_fresh_counterparty_backing_not_atomic.row` | kernel-checked |
| T28.header | `deposit_not_atomic` | final | pres | pres | `S_deposit_not_atomic.row` | kernel-checked |
| T28.account | `deposit_not_atomic` | - | est | est | `S_deposit_not_atomic.row` | kernel-checked |
| T30.header | `execute_batch_with_fee_loss_stale_scoped_not_atomic` | final | est | est | `S_execute_batch_with_fee_loss_stale_scoped_not_atomic.row` | kernel-checked |
| T30.account | `execute_batch_with_fee_loss_stale_scoped_not_atomic` | - | est | est | `S_execute_batch_with_fee_loss_stale_scoped_not_atomic.row` | kernel-checked |
| T31.header | `execute_trade_with_fee_loss_stale_scoped_not_atomic` | delegates | est | est | `S_execute_trade_with_fee_loss_stale_scoped_not_atomic.row` | kernel-checked |
| T31.account | `execute_trade_with_fee_loss_stale_scoped_not_atomic` | - | est | est | `S_execute_trade_with_fee_loss_stale_scoped_not_atomic.row` | kernel-checked |
| T34.header | `force_asset_recovery_not_atomic` | final | est | est | `S_force_asset_recovery_not_atomic.row` | kernel-checked |
| T35.header | `force_close_recovery_pair_not_atomic` | delegates | est | est | `S_force_close_recovery_pair_not_atomic.row` | kernel-checked |
| T35.account | `force_close_recovery_pair_not_atomic` | - | est | est | `S_force_close_recovery_pair_not_atomic.row` | kernel-checked |
| T43.header | `mark_asset_drain_only_not_atomic` | final | pres | pres | `S_mark_asset_drain_only_not_atomic.row` | kernel-checked |
| T46.header | `recredit_terminal_claim_free_residual_for_asset_not_atomic` | final | est | est | `S_recredit_terminal_claim_free_residual_for_asset_not_atomic.row` | kernel-checked |
| T52.header | `reset_empty_asset_oracle_anchor_not_atomic` | final | est | est | `S_reset_empty_asset_oracle_anchor_not_atomic.row` | kernel-checked |
| T53.header | `resolve_market_not_atomic` | final | est | est | `S_resolve_market_not_atomic.row` | kernel-checked |
| T54.header | `restart_empty_asset_preserving_insurance_budget_not_atomic` | final | est | est | `S_restart_empty_asset_preserving_insurance_budget_not_atomic.row` | kernel-checked |
| T55.header | `retire_empty_asset_not_atomic` | final | est | est | `S_retire_empty_asset_not_atomic.row` | kernel-checked |
| T56.header | `retire_terminal_unbudgeted_insurance_not_atomic` | final | est | est | `S_retire_terminal_unbudgeted_insurance_not_atomic.row` | kernel-checked |
| T57.header | `set_asset_raw_oracle_target_not_atomic` | delegates | est | est | `S_set_asset_raw_oracle_target_not_atomic.row` | kernel-checked |
| T58.header | `set_asset_raw_oracle_targets_not_atomic` | final | est | est | `S_set_asset_raw_oracle_targets_not_atomic.row` | kernel-checked |
| T59.header | `sync_account_fee_to_slot_not_atomic` | final | pres | pres | `S_sync_account_fee_to_slot_not_atomic.row` | kernel-checked |
| T59.account | `sync_account_fee_to_slot_not_atomic` | - | est | est | `S_sync_account_fee_to_slot_not_atomic.row` | kernel-checked |
| T60.header | `withdraw_backing_provider_earnings_not_atomic` | final | pres | pres | `S_withdraw_backing_provider_earnings_not_atomic.row` | kernel-checked |
| T61.header | `withdraw_domain_insurance_not_atomic` | final | est | est | `S_withdraw_domain_insurance_not_atomic.row` | kernel-checked |
| T62.header | `withdraw_fresh_counterparty_backing_not_atomic` | final | est | est | `S_withdraw_fresh_counterparty_backing_not_atomic.row` | kernel-checked |
| T63.header | `withdraw_not_atomic` | final | pres | pres | `S_withdraw_not_atomic.row` | kernel-checked |
| T63.account | `withdraw_not_atomic` | - | pres | pres | `S_withdraw_not_atomic.row` | kernel-checked |
| T64.header | `withdraw_resolved_counterparty_backing_surplus_not_atomic` | delegates | est | est | `S_withdraw_resolved_counterparty_backing_surplus_not_atomic.row` | kernel-checked |

Open in this run (function is in the MIR): T01.header, T02.header, T09.header, T10.header, T15.header, T15.account, T29.header, T29.account, T32.header, T33.header, T36.header, T36.account, T37.header, T37.account, T42.header, T42.account, T44.header, T44.account, T45.header, T45.account, T47.header, T47.account

## T15 `close_resolved_account_not_atomic`

MIR sha256 `f7bfa0fd95144bceaf6fa8eb21d4dd3138581290898e0c16d1112a2cc99f0f72`, 313 blocks. `Ok(..)` assignments (block → source line): bb22 → v16.rs:22970, bb41 → v16.rs:22982, bb60 → v16.rs:22991, bb77 → v16.rs:23006, bb95 → v16.rs:23018, bb143 → v16.rs:23037, bb152 → v16.rs:23040, bb178 → v16.rs:23048, bb191 → v16.rs:23060, bb292 → v16.rs:23122.

- `S_close_resolved_account_not_atomic` (kernel-checked), whole function, no exclusion, no assumption: result = 0 ∧ payload = 1: checks [0, 1] hold
- T15.account partial: 7 of 10 `Ok(..)` sites; excluded: bb143 → v16.rs:23037, bb152 → v16.rs:23040, bb77 → v16.rs:23006. `T15_account_checked_paths.row` (kernel-checked)
- T15.account reduction: whole row IF these callees preserve the check on `Ok` (ASSUMED, not proved): `detach_solvent_active_legs_for_resolved_close`, `expire_one_lapsed_source_backing_for_resolved_active_leg_not_atomic`, `prepare_one_source_domain_for_resolved_close_not_atomic`, `settle_negative_pnl_from_principal_not_atomic`, `settle_resolved_account_kf_not_atomic`, `settle_resolved_account_side_effects_after_kf_not_atomic`, `settle_resolved_bankruptcy_negative_pnl`. `T15_account_conditional.row` (kernel-checked)
- T15.header partial: 8 of 10 `Ok(..)` sites; excluded: bb143 → v16.rs:23037, bb152 → v16.rs:23040. `T15_header_checked_paths.row` (kernel-checked)
- T15.header reduction: whole row IF these callees preserve the check on `Ok` (ASSUMED, not proved): `detach_solvent_active_legs_for_resolved_close`, `settle_negative_pnl_from_principal_not_atomic`, `settle_resolved_account_kf_not_atomic`, `settle_resolved_account_side_effects_after_kf_not_atomic`, `settle_resolved_bankruptcy_negative_pnl`. `T15_header_conditional.row` (kernel-checked)

## Callee summaries used (each is the goal of its own module)

`result = 0` is `Ok`; `payload` is the integer value or enum discriminant of the `Ok` payload (Option: None = 0, Some = 1; a plain crate enum: its variant index).

| Function | Clauses | Module status | Uses |
|---|---|---|---|
| `add_account_source_positive_pnl_not_atomic` | result = 0: checks [1] hold; checks [0] preserved | kernel-checked | none |
| `apply_released_pnl_conversion_core_not_atomic` | result = 0 ∧ payload = 0: impossible | kernel-checked | `apply_released_pnl_conversion_with_consumption_core_not_atomic` |
| `apply_released_pnl_conversion_with_consumption_core_not_atomic` | result = 0 ∧ payload = 0: impossible | kernel-checked | none |
| `canonicalize_retired_empty_asset_slot_not_atomic` | result = 0: checks [0] hold | kernel-checked | none |
| `canonicalize_spent_backing_history_not_atomic` | result = 0: checks [0] hold | kernel-checked | none |
| `charge_account_backing_fee_not_atomic` | result = 0: checks [0, 1] preserved<br>result = 0 ∧ payload = 1: checks [0, 1] hold | kernel-checked | none |
| `charge_account_fee_current_not_atomic` | result = 0 ∧ payload = 0: checks [0, 1] preserved | kernel-checked | none |
| `claim_resolved_payout_topup_not_atomic` | result = 0: checks [0, 1] hold | kernel-checked | none |
| `clear_fully_diluted_resolved_receipt_if_terminal` | result = 0: checks [0] preserved | kernel-checked | none |
| `close_resolved_account_not_atomic` | result = 0 ∧ payload = 1: checks [0, 1] hold | kernel-checked | `clear_fully_diluted_resolved_receipt_if_terminal`, `expire_one_lapsed_source_backing_for_resolved_active_leg_not_atomic`, `prepare_one_source_domain_for_resolved_close_not_atomic`, `realize_one_source_domain_for_resolved_close_not_atomic`, `sync_account_fee_to_slot_not_atomic` |
| `collect_account_backing_utilization_fee_for_domain_not_atomic` | result = 0 ∧ payload = 0: checks [0] preserved | kernel-checked | none |
| `convert_released_pnl_to_capital_core_not_atomic` | result = 0 ∧ payload = 0: checks [0, 1] preserved | kernel-checked | `apply_released_pnl_conversion_core_not_atomic` |
| `convert_released_pnl_to_capital_not_atomic` | result = 0: checks [0, 1] preserved<br>result = 0 ∧ payload = 1: checks [0, 1] hold | kernel-checked | `convert_released_pnl_to_capital_core_not_atomic` |
| `credit_account_from_insurance_not_atomic` | result = 0: checks [0, 1] preserved | kernel-checked | none |
| `credit_backing_provider_earnings_not_atomic` | result = 0: checks [0] preserved | kernel-checked | none |
| `credit_domain_insurance_budget_not_atomic` | result = 0: checks [0] hold | kernel-checked | `credit_domain_insurance_budgets_not_atomic` |
| `credit_domain_insurance_budgets_not_atomic` | result = 0: checks [0] hold | kernel-checked | none |
| `cure_and_cancel_close_not_atomic` | result = 0: checks [0, 1] hold | kernel-checked | `cure_and_cancel_close_with_cert_not_atomic` |
| `cure_and_cancel_close_with_cert_not_atomic` | result = 0: checks [0, 1] hold | kernel-checked | none |
| `deposit_domain_insurance_not_atomic` | result = 0: checks [0] hold | kernel-checked | none |
| `deposit_fresh_counterparty_backing_not_atomic` | result = 0: checks [0] hold | kernel-checked | none |
| `deposit_not_atomic` | result = 0: checks [1] hold; checks [0] preserved | kernel-checked | none |
| `execute_batch_with_fee_loss_stale_scoped_not_atomic` | result = 0: checks [0, 1, 2] hold | kernel-checked | none |
| `execute_trade_with_fee_loss_stale_scoped_not_atomic` | result = 0: checks [0, 1, 2] hold | kernel-checked | `execute_batch_with_fee_loss_stale_scoped_not_atomic` |
| `expire_one_lapsed_source_backing_for_resolved_active_leg_not_atomic` | result = 0 ∧ payload = 0: checks [0, 1] preserved | kernel-checked | none |
| `force_asset_recovery_not_atomic` | result = 0: checks [0] hold | kernel-checked | none |
| `force_close_recovery_pair_not_atomic` | result = 0 ∧ payload = 0: impossible<br>result = 0: checks [0, 1, 2] hold | kernel-checked | `execute_trade_with_fee_loss_stale_scoped_not_atomic` |
| `mark_asset_drain_only_not_atomic` | result = 0: checks [0] preserved | kernel-checked | none |
| `normalize_terminal_empty_asset_history_not_atomic` | result = 0: checks [0] hold | kernel-checked | none |
| `prepare_one_source_domain_for_resolved_close_not_atomic` | result = 0: checks [0] preserved<br>result = 0 ∧ payload = 1: checks [0, 1] hold | kernel-checked | `collect_account_backing_utilization_fee_for_domain_not_atomic`, `release_account_source_credit_lien_for_domain_not_atomic`, `retire_source_credit_lien_from_counterparty_impaired_not_atomic` |
| `realize_one_source_domain_for_resolved_close_not_atomic` | result = 0 ∧ payload = 0: checks [0] preserved | kernel-checked | `apply_released_pnl_conversion_with_consumption_core_not_atomic` |
| `recredit_terminal_claim_free_residual_for_asset_not_atomic` | result = 0: checks [0] hold | kernel-checked | none |
| `release_account_source_credit_lien_for_domain_not_atomic` | result = 0: checks [0] preserved | kernel-checked | `release_source_credit_lien_from_counterparty_terminal_not_atomic`, `release_source_credit_lien_from_insurance_terminal_not_atomic` |
| `release_source_credit_lien_from_counterparty_terminal_not_atomic` | result = 0: checks [0] preserved | kernel-checked | none |
| `release_source_credit_lien_from_insurance_terminal_not_atomic` | result = 0: checks [0] preserved | kernel-checked | none |
| `reset_empty_asset_oracle_anchor_not_atomic` | result = 0: checks [0] hold | kernel-checked | none |
| `resolve_market_not_atomic` | result = 0: checks [0] hold | kernel-checked | none |
| `restart_empty_asset_preserving_insurance_budget_not_atomic` | result = 0: checks [0] hold | kernel-checked | `normalize_terminal_empty_asset_history_not_atomic` |
| `retire_empty_asset_not_atomic` | result = 0: checks [0] hold | kernel-checked | `normalize_terminal_empty_asset_history_not_atomic` |
| `retire_source_credit_lien_from_counterparty_impaired_not_atomic` | result = 0: checks [0] preserved | kernel-checked | none |
| `retire_terminal_unbudgeted_insurance_not_atomic` | result = 0: checks [0] hold | kernel-checked | none |
| `set_asset_raw_oracle_target_not_atomic` | result = 0: checks [0] hold | kernel-checked | `set_asset_raw_oracle_targets_not_atomic` |
| `set_asset_raw_oracle_targets_not_atomic` | result = 0: checks [0] hold | kernel-checked | none |
| `sync_account_fee_to_slot_not_atomic` | result = 0: checks [1] hold; checks [0] preserved<br>result = 0 ∧ payload = 1: checks [0] hold | kernel-checked | `charge_account_fee_current_not_atomic` |
| `withdraw_backing_provider_earnings_not_atomic` | result = 0: checks [0] preserved | kernel-checked | none |
| `withdraw_domain_insurance_not_atomic` | result = 0: checks [0] hold | kernel-checked | none |
| `withdraw_fresh_counterparty_backing_not_atomic` | result = 0: checks [0] hold | kernel-checked | none |
| `withdraw_not_atomic` | result = 0: checks [0, 1] preserved | kernel-checked | none |
| `withdraw_resolved_counterparty_backing_surplus_not_atomic` | result = 0: checks [0] hold | kernel-checked | `withdraw_fresh_counterparty_backing_not_atomic` |

Methods analysed: 321. Unsupported (no summary; calls to them are treated as able to write everything): 2. Plain crate enums in the R9 table: 24.
- `burn_account_source_claim_bound_num`: argument _4 is reassigned or mutably borrowed
- `burn_account_source_lien_face_not_atomic`: argument _6 is reassigned or mutably borrowed
