# strong2 results (Gen)

Pin: engine: dcccrypto/percolator@c38b6b1e, src/v16.rs blob 7d3edfe86be1a80f49793cf551c39687a8766a4d; rustc 1.99.0-nightly (d0babd8b6 2026-07-15); cargo rustc -j 4 --release --lib -- -Zunpretty=mir -Zinline-mir=no

`lake build`: Build completed successfully (25 jobs).

## Rows

| Row | Function | Result | Lean module | Status | Excluded Ok sites |
|---|---|---|---|---|---|
| T15.account | `close_resolved_account_not_atomic` | partial: Ok returns outside the excluded blocks, pres (4 of 7 `Ok(..)` sites) | `T15_account_checked_paths` | kernel-checked | bb104 → v16.rs:26631, bb113 → v16.rs:26634, bb55 → v16.rs:26614 |
| T15.header | `close_resolved_account_not_atomic` | partial: Ok returns outside the excluded blocks, pres (5 of 7 `Ok(..)` sites) | `T15_header_checked_paths` | kernel-checked | bb104 → v16.rs:26631, bb113 → v16.rs:26634 |
| T18.account | `convert_released_pnl_to_capital_not_atomic` | whole row, pres | `T18_account` | kernel-checked | none |
| T18.header | `convert_released_pnl_to_capital_not_atomic` | whole row, pres | `T18_header` | kernel-checked | none |

For each target: MIR sha256, blocks, and the callee summaries its abstract program uses.

- T15 `close_resolved_account_not_atomic`: MIR sha256 `45f0ed9671e3d808a6c3d83eab81363e8242c2214e5ba0fcc42130eda498d34d`, 257 blocks; uses: `clear_fully_diluted_resolved_receipt_if_terminal`, `expire_one_lapsed_source_backing_for_resolved_active_leg_not_atomic`, `prepare_one_source_domain_for_resolved_close_not_atomic`, `realize_one_source_domain_for_resolved_close_not_atomic`, `sync_account_fee_to_slot_not_atomic`
- T18 `convert_released_pnl_to_capital_not_atomic`: MIR sha256 `5c7737fcb443166aa0e5d651a54c88b4a51491da28f19c9901c21d0ca1d1ead0`, 49 blocks; uses: `convert_released_pnl_to_capital_core_not_atomic`, `release_account_source_credit_liens_if_unneeded_not_atomic`

## Callee summaries used, each certified by its own module

A summary is a list of clauses about the callee's return: `result = 0` is `Ok`; `payload` is the
integer value or enum discriminant of the `Ok` payload (Option: None = 0, Some = 1).

| Callee | Clauses | Module status | Uses |
|---|---|---|---|
| `apply_released_pnl_conversion_core_not_atomic` | result = 0 ∧ payload = 0: impossible | kernel-checked | `apply_released_pnl_conversion_with_consumption_core_not_atomic` |
| `apply_released_pnl_conversion_with_consumption_core_not_atomic` | result = 0 ∧ payload = 0: impossible | kernel-checked | none |
| `charge_account_fee_current_not_atomic` | result = 0 ∧ payload = 0: checks [0, 1] preserved | kernel-checked | none |
| `clear_fully_diluted_resolved_receipt_if_terminal` | result = 0: checks [0] preserved | kernel-checked | none |
| `collect_account_backing_utilization_fee_for_domain_not_atomic` | result = 0 ∧ payload = 0: checks [0] preserved | kernel-checked | none |
| `convert_released_pnl_to_capital_core_not_atomic` | result = 0 ∧ payload = 0: checks [0, 1] preserved | kernel-checked | `apply_released_pnl_conversion_core_not_atomic` |
| `expire_one_lapsed_source_backing_for_resolved_active_leg_not_atomic` | result = 0 ∧ payload = 0: checks [0, 1] preserved | kernel-checked | none |
| `prepare_one_source_domain_for_resolved_close_not_atomic` | result = 0: checks [0] preserved<br>result = 0 ∧ payload = 1: checks [0, 1] hold | kernel-checked | `collect_account_backing_utilization_fee_for_domain_not_atomic`, `release_account_source_credit_lien_for_domain_not_atomic`, `retire_source_credit_lien_from_counterparty_impaired_not_atomic` |
| `realize_one_source_domain_for_resolved_close_not_atomic` | result = 0 ∧ payload = 0: checks [0] preserved | kernel-checked | `apply_released_pnl_conversion_with_consumption_core_not_atomic` |
| `release_account_source_credit_lien_for_domain_not_atomic` | result = 0: checks [0] preserved | kernel-checked | `release_source_credit_lien_from_counterparty_terminal_not_atomic`, `release_source_credit_lien_from_insurance_not_atomic`, `release_source_credit_lien_from_insurance_terminal_not_atomic` |
| `release_account_source_credit_liens_if_unneeded_core_not_atomic` | result = 0: checks [0, 1] hold | kernel-checked | `collect_account_backing_utilization_fee_for_domain_not_atomic`, `release_account_source_credit_lien_for_domain_not_atomic`, `retire_source_credit_lien_from_counterparty_impaired_not_atomic` |
| `release_account_source_credit_liens_if_unneeded_not_atomic` | result = 0: checks [0, 1] hold | kernel-checked | `release_account_source_credit_liens_if_unneeded_core_not_atomic` |
| `release_source_credit_lien_from_counterparty_terminal_not_atomic` | result = 0: checks [0] preserved | kernel-checked | none |
| `release_source_credit_lien_from_insurance_not_atomic` | result = 0: checks [0] hold | kernel-checked | none |
| `release_source_credit_lien_from_insurance_terminal_not_atomic` | result = 0: checks [0] preserved | kernel-checked | none |
| `retire_source_credit_lien_from_counterparty_impaired_not_atomic` | result = 0: checks [0] preserved | kernel-checked | none |
| `sync_account_fee_to_slot_not_atomic` | result = 0: checks [1] hold; checks [0] preserved<br>result = 0 ∧ payload = 1: checks [0] hold | kernel-checked | `charge_account_fee_current_not_atomic` |

Methods analysed to find these: 188. Unsupported (no summary; calls to them are treated as able to write everything): 0.
