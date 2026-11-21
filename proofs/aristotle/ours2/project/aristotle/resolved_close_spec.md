# Resolved close on the engine's own text (RCs1..RCs7, F3)

Source: dcccrypto/percolator, engine revision `c38b6b1e` (`src/v16.rs` git blob `7d3edfe8`,
`src/wide_math.rs` git blob `97c6b16d`).

## What is in this crate

| File | What it is |
|---|---|
| `src/wide_math.rs` | The engine's `src/wide_math.rs`, the whole file, unchanged. |
| `src/engine_slice.rs` | Generated. Every block between `// ---- src/v16.rs:A-B ----` and `// ---- end ----` is lines A..B of the engine's `src/v16.rs`, byte for byte. Everything else in the file is between `// >>> NOT ENGINE` and `// <<< NOT ENGINE`. |
| `src/engine_slice/harness.rs` | NOT engine code: `close_one`, `topup_one` (build the two views, call one engine function) and the loop `close_all`. |
| `src/lib.rs` | NOT engine code: `BOUND_SCALE` (equal to the engine's `src/lib.rs:25`) and the module lines. |

Engine text in `src/engine_slice.rs` (line ranges of `src/v16.rs`):

- byte wrappers and codecs: `V16PodU64`, `V16PodU128`, `V16PodI128` with `new`/`get`,
  `encode_bool`, `decode_bool`, `ResolvedPayoutLedgerV16Account` and
  `ResolvedPayoutReceiptV16Account` with `from_runtime`/`try_to_runtime`;
- the ledger and receipt values, `validate_resolved_payout_receipt_value`,
  `apply_resolved_payout_receipt_payment`, `V16Core::kernel_recompute_resolved_payout_rate`,
  `bound_num_from_amount`, `amount_from_bound_num`;
- `residual`, `backing_provider_earnings_total`, `source_fresh_backing_total_atoms`,
  `junior_claim_bound`;
- `recompute_resolved_payout_rate`, `initialize_resolved_payout_ledger_if_needed`,
  `create_resolved_payout_receipt_if_needed`, `resolved_receipt_claimable_now`,
  `resolved_receipt_claimable_against_ledger`;
- `resolved_positive_payout_ready`, `resolved_payout_rate_is_terminal`,
  `clear_fully_diluted_resolved_receipt_if_terminal`;
- `claim_resolved_payout_topup_core_not_atomic`;
- `set_account_pnl`, `set_account_pnl_inner`;
- `TokenValueFlowProofV16` with `empty`, `capital_and_resolved_payout_to_external_out`, `debit`,
  `credit`, `validate`;
- the tail of `close_resolved_account_not_atomic`, lines 26643-26702 (from
  `let mut payout_receipt = None;` to `Ok(ResolvedCloseOutcomeV16::Closed { payout })`), as the
  body of `close_resolved_account_tail_not_atomic`. The four signature lines and the closing brace
  of that function are the only non-engine text there.

## What is NOT engine text, and what each stub means

- The receivers. The engine's `MarketGroupV16ViewMut<'a, T>` has `header` and `markets`; here it
  has `header` only. `MarketGroupV16HeaderAccount` has 21 of the engine's 47 fields and
  `PortfolioAccountV16Account` has 6 of the engine's fields: same names, same types, same order;
  the other fields are absent. The byte-wrapper structs lose their `bytemuck` derives.
- STUB-OK: `validate_shape()` and `validate_with_market()` return `Ok(())`. In the engine they are
  read-only audits that can return `Err`. A stub of this kind can only turn an engine `Err` into
  `Ok`: a statement about the runs of this crate that return `Ok` covers every engine run that
  returns `Ok`, and the engine may additionally return `Err` there.
- STUB-FRAME: `sync_unattributed_loss_lock` returns `Ok(())`. The engine function writes two
  account bytes that are not in this crate and can return `Err`. Same consequence as STUB-OK.
- STUB-PATH: `burn_account_source_claim_bound_num` returns `Ok(())`. The engine function does
  exactly that when `burn_num == 0` or when the account has no source-claim bound left. For an
  account that reaches the tail with positive pnl and a nonzero source-claim bound, the engine runs
  code that is not here and that can write fields of this crate. That engine path is not covered
  by any statement below.
- STUB-DEAD: five functions named only inside `if let Some(domain) = increase_domain` of
  `set_account_pnl_inner`. `set_account_pnl` passes `source_domain = None`, so that branch is
  never taken here. Their bodies are `panic!`, so "does not panic" for a caller shows this too.
- Not in the crate at all: the steps of `close_resolved_account_not_atomic` before its tail
  (engine lines 26585-26642). They return `ProgressOnly` or `Err` unless, among other things, the
  account has `pnl >= 0` (26626-26632). `pnl >= 0` is a precondition of the properties below;
  nothing here shows that those steps keep `Inv`. Also not here: the two engine operations that
  change the rate after the snapshot (`refine_resolved_unreceipted_bound_not_atomic`,
  `kernel_credit_post_snapshot_residual`).

## What the code does

The first close that needs a receipt captures a snapshot once
(`initialize_resolved_payout_ledger_if_needed`): `snapshot_residual` is the vault minus capital,
insurance and two other senior amounts, and `terminal_claim_bound_unreceipted_num` is the header's
bound on all positive pnl (scaled by `BOUND_SCALE`). Closing an account with positive pnl gives it
a receipt for its pnl (`terminal_positive_claim_face`), moves that amount of bound from
"unreceipted" to "exact receipts", and pays
`floor(face * rate_num / rate_den) - paid_effective`, with `rate_den = exact + unreceipted` and
`rate_num = min(snapshot_residual * BOUND_SCALE, rate_den)`. The close pays
`min(capital + that amount, vault)` and then sets the account's capital to 0 whatever was paid.
When the rate is terminal (no unreceipted bound left, and the rate is full or no fresh backing is
left) and a receipt has nothing more to claim, `clear_fully_diluted_resolved_receipt_if_terminal`
replaces it by the empty receipt.

## Notation

All values are the decoded ones (`x.get()`, `try_to_runtime()`); sums are mathematical integers.

- `M` is a `MarketGroupV16HeaderAccount`, `A` a slice of `PortfolioAccountV16Account`.
- `L = M.resolved_payout_ledger.try_to_runtime()`; `r_i = A[i].resolved_payout_receipt.try_to_runtime()`.
- `CL` is a ghost list of pairs `(face, paid)`: the receipts the engine has cleared so far. It is
  a history variable, not program state.
- `R` is the list of `(face, paid)` of every present `r_i`, followed by `CL`.
- `gross(face) = floor(face * L.current_payout_rate_num / L.current_payout_rate_den)`.
- `P` is the sum of `paid` over `R`; `G` the sum of `gross(face)` over `R`; `C` the sum of
  `A[i].capital`.

## The invariant `Inv(M, A, CL)`

- `wf`: `M.payout_snapshot_captured` is 0 or 1; `M.resolved_payout_ledger.try_to_runtime()` and
  every `A[i].resolved_payout_receipt.try_to_runtime()` return `Ok` (every bool byte is 0 or 1).
- `receipts`: `validate_resolved_payout_receipt_value(r_i)` returns `Ok` for every account.
- `mirror`: `M.c_tot` equals `C`.
- If `M.payout_snapshot_captured == 0` (no snapshot yet): no `r_i` is present, `CL` is empty, and
  `M.c_tot + M.insurance <= M.vault`.
- If `M.payout_snapshot_captured == 1`:
  - `rate`: `kernel_recompute_resolved_payout_rate(L)` returns `Ok(L)`;
  - `live`: `L.payout_halted == false`;
  - `cover`: the sum of `face * BOUND_SCALE` over `R` is at most `L.terminal_claim_exact_receipts_num`;
  - `paidLe`: `paid <= gross(face)` for every element of `R`;
  - `backed`: `M.c_tot + M.insurance + max(L.snapshot_residual - P, 0) <= M.vault`.

Differences from the invariant of the first run (a hand transcription of this path), each forced
by engine text that the transcription did not have:

1. The no-snapshot case, because `initialize_resolved_payout_ledger_if_needed` is now in the code.
2. `wf`, because the byte decoders are now in the code.
3. The ghost list `CL`. With `clear_fully_diluted_resolved_receipt_if_terminal` in the code, the
   first run's `backed` (with `P` over present receipts only) is NOT kept by a close: clearing a
   paid receipt lowers that `P` while the money has left the vault. Example (found by running this
   crate): one account, `vault = 632`, `c_tot = 166`, `insurance = 466`, `snapshot_residual = 104`,
   exact receipts `351 * BOUND_SCALE`, unreceipted 0, rate `104/351`, receipt face 351 paid 104,
   capital 166. The close pays 166 and clears the receipt; afterwards `P = 0` over present
   receipts and `c_tot + insurance + snapshot_residual = 570 > vault = 466`. With the cleared
   receipt kept in `CL`, `P` stays 104 and `backed` holds.

## Properties

Each is for every slice length and all values the types allow. "Returns" means returns `Ok`.
Primed values are after the call. `i < A.len()`.

- **RCs1.** If `Inv(M, A, CL)` and `M.payout_snapshot_captured == 1`, then for every present
  `r_i`, `resolved_receipt_claimable_against_ledger(r_i, L)` returns `gross(face_i) - paid_i`;
  `G <= L.snapshot_residual`; and `P <= G`. (`G` and `P` include `CL`. The arithmetic is for a
  list of any length.)
- **RCs2.** If `Inv(M, A, CL)`, `A[i].pnl >= 0`, and `close_one(&mut M, &mut A[i])` returns
  `payout`, then there is `CL'`, equal to `CL` or to `CL` with one pair appended (the account's
  `(face, paid)` at the moment the engine cleared its receipt), such that:
  `Inv(M', A', CL')`; `M'.vault + payout == M.vault`; `payout == A[i].capital + (P' - P)`;
  `M'.c_tot + A[i].capital == M.c_tot`; `A'[i]` has `capital == 0`, `pnl == 0`,
  `reserved_pnl == 0`, `fee_credits == 0`, `health_cert.valid == 0`; if `r'_i` is present then
  `paid'_i == gross'(face'_i)`; no other account changed; `M'.insurance == M.insurance`; if the
  snapshot was captured before, `snapshot_residual`, `current_payout_rate_num` and
  `current_payout_rate_den` did not change. No execution of the call panics.
  (So the `min(.., vault)` cap never cuts the payout and no capital is zeroed unpaid.)
- **RCs4.** If `Inv(M, A, CL)`, every `A[j].pnl >= 0`, and `close_all(&mut M, &mut A, order)`
  returns `total`, then there is `CL'` extending `CL` such that: `Inv(M', A', CL')`;
  `M'.vault + total == M.vault`; `total == (C - C') + (P' - P)`;
  `M'.c_tot + M'.insurance <= M'.vault`; `M'.insurance == M.insurance`; if
  `M'.payout_snapshot_captured == 1` then `P' <= L'.snapshot_residual`; every account whose index
  is in `order` has `capital == 0` and `pnl == 0`. `order` is any slice (repeats allowed).
- **RCs5.** If in addition every index below `A.len()` occurs in `order`, then `M'.c_tot == 0`,
  `total == C + (P' - P)`, `M.insurance <= M'.vault`, and, if `M'.payout_snapshot_captured == 1`,
  `total <= C + L'.snapshot_residual - P`.
- **RCs7.** If `Inv(M, A, CL)` and `topup_one(&mut M, &mut A[i])` returns `payout`, then there is
  `CL'` (equal to `CL`, or `CL` with the account's `(face, paid)` appended when its receipt was
  cleared) such that `Inv(M', A', CL')`, `M'.vault + payout == M.vault`, `P' == P + payout`;
  `A[i].capital`, `A[i].pnl`, `M.c_tot`, `M.insurance` and every other account are unchanged.
  No execution of the call panics.
- **F3.** `wide_mul_div_floor_u128(a, b, d)` (in `src/wide_math.rs`) panics exactly when `d == 0`
  or `floor(a*b/d) >= 2^128`; otherwise it returns `floor(a*b/d)`. For all `u128` inputs; the
  product is the mathematical one.
- **RC3s.** If `Inv(M, A, CL)`, `A[i].pnl >= 0`, and
  - (receipt) `r_i` is present, or `A[i].pnl == 0`, or the snapshot is captured and
    `A[i].pnl * BOUND_SCALE <= L.terminal_claim_bound_unreceipted_num`, or the snapshot is not
    captured and `A[i].pnl * BOUND_SCALE <= M.pnl_pos_bound_tot_num` and
    `M.residual() * BOUND_SCALE <= u128::MAX`;
  - (aggregates) `A[i].pnl <= M.pnl_pos_tot` and `M.pnl_pos_tot * BOUND_SCALE <= u128::MAX`;

  then `close_one(&mut M, &mut A[i])` does not return `Err` and does not panic. This is about the
  code of this crate. The engine can still return `Err` from `validate_shape`,
  `validate_with_market`, `sync_unattributed_loss_lock` and
  `burn_account_source_claim_bound_num`, which are stubs here.
- **RC6s.** If `Inv(M, A, CL)`, every `A[j].pnl >= 0`, `order2` is a permutation of `order1`, and
  `close_all` returns `total` on `order1`, then from the same start it returns on `order2` with
  the same `total`, the same `M'`, and for every account the same `capital`, `pnl`,
  `reserved_pnl`, `fee_credits` and `health_cert`. The receipts of an account in the two results
  are equal, or one is the empty receipt and the other is present with `paid == gross(face)`.
  (They are not always equal: which receipts get cleared depends on the order. Found by running
  this crate.)

## What is already known, and how

- First run (same engine revision, a hand transcription of this path over plain structs, without
  the byte codecs, the snapshot capture, the receipt clearing, `set_account_pnl` and the
  token-flow check): the analogues of RCs1, RCs2, RCs4, RCs5, RCs7, RC3 and RC6 were proved by
  Aristotle, assuming the `wide_mul_div_floor_u128` contract. The proofs were not returned.
- `wide_mul_div_floor_u128`: proved by Aristotle on the upstream engine (aeyakovenko/percolator,
  `proofs/proof_ledger.tsv`, row F3, engine `add1d027`; the chain there is the `U256`
  representation, `U512::mul_u256`, `div_rem_by_u256`, then `mul_div_floor_u256` (row D4), then
  F3). Text comparison of this crate's `src/wide_math.rs` with upstream `add1d027`: lines 1-1422
  are identical; lines 1423-1425 here are two comment lines and an `#[allow(dead_code)]`; lines
  1426-1631 here are identical to upstream lines 1423-1628, and they contain
  `wide_mul_div_floor_u128` (1623-1629 here); every difference is after line 1631, in
  `wide_signed_mul_div_floor_from_k_pair` and a new function next to it, which
  `wide_mul_div_floor_u128` does not call.
- Kani (bounded model checking) in the engine repository:
  `proof_v17_wide_mul_div_floor_stub_correct` (`tests/proofs_v17_fork.rs`) checks the production
  `wide_mul_div_floor_u128` against `a*b/d` for 8-bit inputs only; the 16-bit harness in the same
  file is documented there as intractable and is not run.
  `proof_v16_mul_div_floor_u256_matches_small_reference` (`tests/proofs_v16_arithmetic.rs`) covers
  small values. For the close path: `proof_v16_resolved_receipt_payment_cannot_exceed_terminal_claim`,
  `proof_v16_resolved_receipt_claimable_is_rate_monotone_and_overpaid_fails_closed`,
  `proof_v16_public_resolved_payout_topup_pays_min_claimable_and_vault`,
  `proof_v16_two_resolved_receipts_are_order_independent_when_snapshot_funded`
  (`tests/proofs_v16.rs`) and the three harnesses of `tests/proofs_e2e_resolved_close.rs` (one
  asset, two accounts, 8-bit amounts).
- None of these quantifies over the number of accounts or the length of the close sequence, and
  none covers `wide_mul_div_floor_u128` beyond 8-bit inputs. Those two gaps are this task.
- A random search on this crate (about 1.2 million closes from random `Inv` states) found no
  counterexample to RCs1, RCs2, RCs4, RCs5, RCs7, RC3s, RC6s. That is a test, not a proof.
