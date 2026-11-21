# T15 work list (`close_resolved_account_not_atomic`)

State at the end of session `e2e-lean-strong-1121c` (2026-11-21). Engine text:
`dcccrypto/percolator@c38b6b1e`, `src/v16.rs` blob `7d3edfe8`, unpatched. Line numbers are of that
file. "Header check" is `validate_shape()` as compiled without `test`/`kani`/`audit-scan`
(the deployed, header-only predicate); "account check" is `account.validate_with_market(..)`.

The function has 7 `Ok(..)` assignments: six `Ok(ProgressOnly)` and one `Ok(Closed { .. })`.

| Return | Header row (T15.header) | Account row (T15.account) |
|---|---|---|
| `:26594` ProgressOnly (after source preparation) | kernel-checked | kernel-checked |
| `:26605` ProgressOnly (lapsed backing expired) | kernel-checked | kernel-checked |
| `:26614` ProgressOnly (B chunk) | kernel-checked | **false as stated**: witness test |
| `:26631` ProgressOnly (legs / negative pnl / stale left) | **false as stated**: witness test | open |
| `:26634` ProgressOnly (payout not ready) | **false as stated**: witness test | open |
| `:26641` ProgressOnly (one source domain realized) | kernel-checked | kernel-checked |
| `:26702` Closed | kernel-checked | kernel-checked |

"kernel-checked" rows: `T15_header_checked_paths.row`, `T15_account_checked_paths.row`
(every `Ok` return except the excluded blocks), and `S_close_resolved_account_not_atomic.main`
(whole function, no exclusion, no assumed callee: `Ok(Closed)` implies both checks hold).
They are statements about the abstract program produced from MIR; see `RULES.md` for what is
trusted (rules R1 to R10, assumptions A1 to A16). The `Ok(Closed)` theorem is the one result
here that needs rule R9 (enum values), the rule two audit rounds found holes in; the per-return
rows do not need it (`RULES.md`, section 7).

## 1. What the witness test shows, and what it does not

`tests/strong2_t15_progress_only_shape.rs`, run on the unmodified engine with default features
(`out/t15-witness/default_features.log`: 7 passed, 0 failed, 1 ignored):

- `t15_header_refuted_progress_only_26634_*` and `..._26631_*`: before the call
  `validate_shape() == Ok` and `validate_with_market == Ok`; the call returns
  `Ok(ProgressOnly)`; after it `validate_shape() == Err(InvalidConfig)`. Clause violated:
  `pnl_pos_bound_tot_num >= source_claim_bound_total_num` (`src/v16.rs:10106`); the junior
  totals were lowered by the K-loss settlement, the claim total was not.
- `t15_account_refuted_at_26614_reserved_pnl_above_pnl_after_b_chunk`: both checks `Ok` before,
  `Ok(ProgressOnly)`, after it `validate_with_market == Err(InvalidLeg)` with
  `reserved_pnl (100) > max(pnl, 0) (90)` (`src/v16.rs:6495`).

Limits of this evidence:

- The pre-states are built with public entry points **plus direct writes to public fields**
  (listed in the test's doc comments). Nothing shows these pre-states are reachable through the
  wrapper. The tests refute the row **as literally stated** ("check Ok before and call Ok
  implies check Ok after"); they do not show a reachable bad state.
- Built with `--features audit-scan` (`out/t15-witness/audit_scan_feature.log`) no violation is
  observed: the header pre-states fail the full scan before the call, and in the account case
  the call returns `Err(InvalidLeg)`.
- On `aeyakovenko/percolator@08d4fd09` (scratch copy, two API edits, `out-08d4fd09/t15-witness/`)
  the header violation and the account violation are observed too. 5 tests pass and 2 fail; the
  2 failures are at the assertion that classifies WHICH return was taken (the single-asset case
  keeps its leg there, so the return is that text's `:23037`, the counterpart of `:26631`), after
  the hypothesis and violation assertions passed. No violation at the counterpart of `:26634`
  (`:23040`) was observed on that text.

Consequence for the reductions. `T15_header_conditional.row` proves the whole header row IF
`settle_account_side_effects_not_atomic`, `settle_negative_pnl_from_principal_not_atomic` and
`settle_resolved_bankruptcy_negative_pnl` each preserve the header check on `Ok`. With the
witness, those three assumptions cannot all be true of the engine (under `RULES.md`). The same
holds for the six assumptions of `T15_account_conditional.row`. The reductions are kept as
kernel-checked implications; they are not a route to the literal rows.

## 2. Still open

1. **T15.account at `:26631` and `:26634`.** Not proved, not refuted. Random search
   (`t15_random_search`, 300,000 iterations this session, 3,000,000 in session 1121b; logs in
   `out/t15-witness/`) found account-check violations only at `:26614`. A directed attempt
   (`reserved_pnl = pnl` before a K-loss settlement on this path) gave no violation: the path
   lowers `reserved_pnl` with `pnl` (observed after-state `pnl = reserved_pnl = 70225`).
   Obstacle for a proof: between entry and these returns the account is written by
   `settle_account_side_effects_not_atomic`, `sync_account_fee_to_slot_not_atomic`,
   `settle_negative_pnl_from_principal_not_atomic`, `settle_resolved_bankruptcy_negative_pnl`,
   `detach_solvent_active_legs_for_resolved_close`, and no runtime check runs afterwards. A proof
   needs the arithmetic content of `validate_with_market` (for instance
   `reserved_pnl <= max(pnl, 0)`, leg and bitmap consistency) carried through those writes. The
   clause domain here knows only "this check ran and nothing it reads was written since".
2. **A true whole-row statement.** The literal rows are false. A candidate that is not refuted
   by the witnesses: strengthen the hypothesis to the full audit scan
   (`validate_shape_full_audit_scan`, which ties `source_claim_bound_total_num` to the slots, and
   the per-account audit). Not attempted: it is a value-level invariant proof, outside this method.
3. **Reachability** of the witness pre-states through public entry points alone. Not attempted.

## 3. Where each check can be lost (untrusted report of `valflow/blame.py`)

Region names are header fields of the check's read-set. This is a reading aid, not a result.

### `close_resolved_account_not_atomic`, check 0

- writes that can change the check, by region: `backing_provider_earnings_total`, `bankruptcy_hlock_active`, `c_tot`, `insurance`, `insurance_domain_budget_remaining_total`, `loss_stale_active`, `payout_snapshot_captured`, `pnl_matured_pos_tot`, `pnl_pos_bound_tot`, `pnl_pos_bound_tot_num`, `pnl_pos_tot`, `resolved_payout_ledger`, `source_claim_bound_total_num`, `source_fresh_backing_total_num`, `source_insurance_credit_reserved_total_atoms`, `vault`
- through callees without a summary for this check: `create_resolved_payout_receipt_if_needed`, `set_account_pnl`, `settle_account_side_effects_not_atomic`, `settle_negative_pnl_from_principal_not_atomic`, `settle_resolved_bankruptcy_negative_pnl`
- writes through a pointer of unknown region: none
- calls outside the market methods given something writable: none

### `settle_account_side_effects_not_atomic`, check 0

- writes that can change the check, by region: `backing_provider_earnings_total`, `bankruptcy_hlock_active`, `c_tot`, `insurance`, `insurance_domain_budget_remaining_total`, `loss_stale_active`, `pnl_matured_pos_tot`, `pnl_pos_bound_tot`, `pnl_pos_bound_tot_num`, `pnl_pos_tot`, `resolved_payout_ledger`, `source_claim_bound_total_num`, `source_fresh_backing_total_num`, `source_insurance_credit_reserved_total_atoms`
- through callees without a summary for this check: `settle_account_b_chunk`, `settle_account_kf_effects_not_atomic`, `settle_negative_pnl_from_principal_core_not_atomic`
- writes through a pointer of unknown region: none
- calls outside the market methods given something writable: none

### `sync_account_fee_to_slot_not_atomic`, check 0

- writes that can change the check, by region: `backing_provider_earnings_total`, `bankruptcy_hlock_active`, `c_tot`, `insurance`, `insurance_domain_budget_remaining_total`, `loss_stale_active`, `pnl_matured_pos_tot`, `pnl_pos_bound_tot`, `pnl_pos_bound_tot_num`, `pnl_pos_tot`, `resolved_payout_ledger`, `source_claim_bound_total_num`, `source_fresh_backing_total_num`, `source_insurance_credit_reserved_total_atoms`
- through callees without a summary for this check: `settle_account_side_effects_not_atomic`, `settle_negative_pnl_from_principal_core_not_atomic`
- writes through a pointer of unknown region: none
- calls outside the market methods given something writable: none

### `settle_negative_pnl_from_principal_not_atomic`, check 0

- writes that can change the check, by region: `bankruptcy_hlock_active`, `c_tot`
- through callees without a summary for this check: `settle_negative_pnl_from_principal_core_not_atomic`
- writes through a pointer of unknown region: none
- calls outside the market methods given something writable: none

### `settle_resolved_bankruptcy_negative_pnl`, check 0

- writes that can change the check, by region: `backing_provider_earnings_total`, `bankruptcy_hlock_active`, `insurance`, `insurance_domain_budget_remaining_total`, `pnl_matured_pos_tot`, `pnl_pos_bound_tot`, `pnl_pos_bound_tot_num`, `pnl_pos_tot`, `resolved_payout_ledger`, `source_claim_bound_total_num`, `source_fresh_backing_total_num`, `source_insurance_credit_reserved_total_atoms`
- through callees without a summary for this check: `book_bankruptcy_residual_chunk_for_account_core`, `clear_resolved_unattributed_negative_pnl`, `consume_domain_insurance_for_negative_pnl`, `mark_bankruptcy_hlock_event`, `set_account_pnl`, `try_clear_bankruptcy_hlock_if_healthy`
- writes through a pointer of unknown region: none
- calls outside the market methods given something writable: none

### `detach_solvent_active_legs_for_resolved_close`, check 0

- writes that can change the check, by region: none
- through callees without a summary for this check: none
- writes through a pointer of unknown region: none
- calls outside the market methods given something writable: none

### `prepare_one_source_domain_for_resolved_close_not_atomic`, check 0

- writes that can change the check, by region: `backing_provider_earnings_total`, `resolved_payout_ledger`, `source_claim_bound_total_num`, `source_fresh_backing_total_num`, `source_insurance_credit_reserved_total_atoms`
- through callees without a summary for this check: `expire_source_backing_bucket_not_atomic`
- writes through a pointer of unknown region: none
- calls outside the market methods given something writable: none

### `expire_one_lapsed_source_backing_for_resolved_active_leg_not_atomic`, check 0

- writes that can change the check, by region: `backing_provider_earnings_total`, `resolved_payout_ledger`, `source_claim_bound_total_num`, `source_fresh_backing_total_num`, `source_insurance_credit_reserved_total_atoms`
- through callees without a summary for this check: `expire_source_backing_bucket_not_atomic`
- writes through a pointer of unknown region: none
- calls outside the market methods given something writable: none

### `close_resolved_account_not_atomic`, check 1

- writes that can change the check, by region: `<account memory>`
- through callees without a summary for this check: `clear_fully_diluted_resolved_receipt_if_terminal`, `create_resolved_payout_receipt_if_needed`, `detach_solvent_active_legs_for_resolved_close`, `expire_one_lapsed_source_backing_for_resolved_active_leg_not_atomic`, `realize_one_source_domain_for_resolved_close_not_atomic`, `set_account_pnl`, `settle_account_side_effects_not_atomic`, `settle_negative_pnl_from_principal_not_atomic`, `settle_resolved_bankruptcy_negative_pnl`
- writes through a pointer of unknown region: none
- calls outside the market methods given something writable: `MarketGroupV16ViewMut::<'_, T>::burn_impaired_account_source_claim_fields`; `MarketGroupV16ViewMut::<'_, T>::impair_account_source_credit_counterparty_lien_fields`; `MarketGroupV16ViewMut::<'_, T>::record_account_funding_flow`; `MarketGroupV16ViewMut::<'_, T>::record_account_residual_crystallized_loss`; `MarketGroupV16ViewMut::<'_, T>::sync_unattributed_loss_lock`; `PortfolioV16ViewMut::<'_>::compact_source_domains`; `PortfolioV16ViewMut::<'_>::reset_source_domain_slot_if_empty`; `PortfolioV16ViewMut::<'_>::source_domain_mut_or_insert`

### `settle_account_side_effects_not_atomic`, check 1

- writes that can change the check, by region: `<account memory>`
- through callees without a summary for this check: `mark_leg_b_stale`, `settle_account_b_chunk`, `settle_account_kf_effects_not_atomic`, `settle_negative_pnl_from_principal_core_not_atomic`
- writes through a pointer of unknown region: none
- calls outside the market methods given something writable: `MarketGroupV16ViewMut::<'_, T>::burn_impaired_account_source_claim_fields`; `MarketGroupV16ViewMut::<'_, T>::impair_account_source_credit_counterparty_lien_fields`; `MarketGroupV16ViewMut::<'_, T>::record_account_funding_flow`; `MarketGroupV16ViewMut::<'_, T>::record_account_residual_crystallized_loss`; `MarketGroupV16ViewMut::<'_, T>::sync_unattributed_loss_lock`; `PortfolioV16ViewMut::<'_>::compact_source_domains`; `PortfolioV16ViewMut::<'_>::reset_source_domain_slot_if_empty`; `PortfolioV16ViewMut::<'_>::source_domain_mut_or_insert`

### `sync_account_fee_to_slot_not_atomic`, check 1

- writes that can change the check, by region: `<account memory>`
- through callees without a summary for this check: `settle_account_side_effects_not_atomic`, `settle_negative_pnl_from_principal_core_not_atomic`
- writes through a pointer of unknown region: none
- calls outside the market methods given something writable: `MarketGroupV16ViewMut::<'_, T>::burn_impaired_account_source_claim_fields`; `MarketGroupV16ViewMut::<'_, T>::impair_account_source_credit_counterparty_lien_fields`; `MarketGroupV16ViewMut::<'_, T>::record_account_funding_flow`; `MarketGroupV16ViewMut::<'_, T>::record_account_residual_crystallized_loss`; `MarketGroupV16ViewMut::<'_, T>::sync_unattributed_loss_lock`; `PortfolioV16ViewMut::<'_>::compact_source_domains`; `PortfolioV16ViewMut::<'_>::reset_source_domain_slot_if_empty`; `PortfolioV16ViewMut::<'_>::source_domain_mut_or_insert`

### `settle_negative_pnl_from_principal_not_atomic`, check 1

- writes that can change the check, by region: `<account memory>`
- through callees without a summary for this check: `settle_negative_pnl_from_principal_core_not_atomic`
- writes through a pointer of unknown region: none
- calls outside the market methods given something writable: `MarketGroupV16ViewMut::<'_, T>::record_account_residual_crystallized_loss`; `MarketGroupV16ViewMut::<'_, T>::sync_unattributed_loss_lock`

### `settle_resolved_bankruptcy_negative_pnl`, check 1

- writes that can change the check, by region: `<account memory>`
- through callees without a summary for this check: `begin_close_progress_ledger`, `book_bankruptcy_residual_chunk_for_account_core`, `clamp_close_residual_to_account_loss`, `clear_resolved_unattributed_negative_pnl`, `consume_domain_insurance_for_negative_pnl`, `mark_bankruptcy_hlock_event`, `set_account_pnl`, `try_clear_bankruptcy_hlock_if_healthy`
- writes through a pointer of unknown region: none
- calls outside the market methods given something writable: `MarketGroupV16ViewMut::<'_, T>::burn_impaired_account_source_claim_fields`; `MarketGroupV16ViewMut::<'_, T>::impair_account_source_credit_counterparty_lien_fields`; `MarketGroupV16ViewMut::<'_, T>::sync_unattributed_loss_lock`; `PortfolioV16ViewMut::<'_>::compact_source_domains`; `PortfolioV16ViewMut::<'_>::reset_source_domain_slot_if_empty`; `PortfolioV16ViewMut::<'_>::source_domain_mut_or_insert`

### `detach_solvent_active_legs_for_resolved_close`, check 1

- writes that can change the check, by region: `<account memory>`
- through callees without a summary for this check: `apply_position_delta`, `begin_full_drain_reset_inner`, `begin_zero_oi_residue_resets`, `clear_leg_at_slot_inner`
- writes through a pointer of unknown region: none
- calls outside the market methods given something writable: none

### `prepare_one_source_domain_for_resolved_close_not_atomic`, check 1

- writes that can change the check, by region: `<account memory>`
- through callees without a summary for this check: `collect_account_backing_utilization_fee_for_domain_not_atomic`, `expire_source_backing_bucket_not_atomic`, `release_account_source_credit_lien_for_domain_not_atomic`, `retire_source_credit_lien_from_counterparty_impaired_not_atomic`
- writes through a pointer of unknown region: none
- calls outside the market methods given something writable: `MarketGroupV16ViewMut::<'_, T>::impair_account_source_credit_counterparty_lien_fields`; `PortfolioV16ViewMut::<'_>::compact_source_domains`; `PortfolioV16ViewMut::<'_>::reset_source_domain_slot_if_empty`

### `expire_one_lapsed_source_backing_for_resolved_active_leg_not_atomic`, check 1

- writes that can change the check, by region: none
- through callees without a summary for this check: `expire_source_backing_bucket_not_atomic`
- writes through a pointer of unknown region: none
- calls outside the market methods given something writable: none

