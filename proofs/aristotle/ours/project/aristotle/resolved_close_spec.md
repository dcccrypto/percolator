# Resolved close: the payout never exceeds what the snapshot holds (RC1..RC7)

Source: dcccrypto/percolator, engine revision `c38b6b1e` (`src/v16.rs` git blob `7d3edfe8`).
This folder is a cut of that engine, not the engine:

| File | What it is |
|---|---|
| `src/wide_math.rs` | The engine's `src/wide_math.rs`, unchanged. |
| `src/resolved_payout.rs` | Copied byte for byte from `src/v16.rs`; each block carries its line range. `V16Error`, `ResolvedPayoutLedgerV16`, `ResolvedPayoutReceiptV16`, `validate_resolved_payout_receipt_value`, `apply_resolved_payout_receipt_payment`, `V16Core::kernel_recompute_resolved_payout_rate`, `V16Core::bound_num_from_amount`, `resolved_receipt_claimable_against_ledger`. |
| `src/resolved_payout/close_path.rs` | NOT engine code. A transcription over plain values of the value-moving part of the engine's resolved close, calling the copied functions: `create_resolved_payout_receipt_if_needed`, `close_resolved_payout_tail`, `claim_resolved_payout_topup_core`, and the loop `close_all`. |

## What the engine does

After a market is resolved, the engine captures a snapshot once: `snapshot_residual` is what the
vault holds beyond capital, insurance and other senior claims, and
`terminal_claim_bound_unreceipted_num` is an upper bound (scaled by `BOUND_SCALE`) on all positive
pnl. Closing an account with positive pnl gives it a receipt for its pnl (`terminal_positive_claim_face`),
moves that amount of bound from "unreceipted" to "exact receipts", and pays
`floor(face * rate_num / rate_den) - paid_effective`, where
`rate_den = exact + unreceipted` and `rate_num = min(snapshot_residual * BOUND_SCALE, rate_den)`.
The close pays `min(capital + that amount, vault)`, and then sets the account's capital to 0
whatever was paid.

## Differences between `close_path.rs` and the engine

Field access: the engine reads and writes zero-copy account views; `close_path.rs` uses plain
struct fields, and drops the encode/decode calls.

- DIFF 1. `create_resolved_payout_receipt_if_needed` does not call
  `initialize_resolved_payout_ledger_if_needed`: the snapshot is assumed captured (part of `Inv`).
- DIFF 2. `close_resolved_payout_tail` starts where the engine's
  `close_resolved_account_not_atomic` has finished settling the account (engine lines
  26584-26642 return `ProgressOnly` otherwise). Of that condition only `pnl >= 0` matters to the
  tail, and it is an explicit guard here.
- DIFF 3. `clear_fully_diluted_resolved_receipt_if_terminal` is omitted. It replaces a receipt
  whose claimable amount is 0 by the empty receipt and writes nothing else.
- DIFF 4. Writes that move no value in this state are omitted (`reserved_pnl`, `fee_credits`,
  `health_cert.valid`, the positive-pnl aggregates behind `set_account_pnl`), and so are the
  engine's self-checks after the writes (`TokenValueFlowProofV16 ... .validate()`,
  `validate_shape`, `validate_with_market`), which can only turn an `Ok` into an `Err`.
- Not modelled at all: the two engine operations that change the rate after the snapshot,
  `refine_resolved_unreceipted_bound_not_atomic` and `kernel_credit_post_snapshot_residual`.
  RC1 is stated for any ledger satisfying `Inv`, so it holds after them if `Inv` does; that they
  keep `Inv` is not claimed here.

## The invariant `Inv(pool, accounts)`

For a `PoolV16` and a slice of `ClaimantV16` (all sums are over the slice, as mathematical
integers):

- `rate`: `kernel_recompute_resolved_payout_rate(pool.ledger)` returns `Ok(pool.ledger)`
  (the stored rate is the computed one, and the two checked operations do not overflow).
- `live`: `pool.ledger.payout_halted == false`.
- `receipts`: `validate_resolved_payout_receipt_value(a.receipt)` returns `Ok` for every account.
- `cover`: the sum of `terminal_positive_claim_face * BOUND_SCALE` over accounts whose receipt is
  present is at most `pool.ledger.terminal_claim_exact_receipts_num`.
- `paidLe`: for every present receipt, `paid_effective <= floor(face * rate_num / rate_den)`.
- `mirror`: `pool.c_tot` equals the sum of `capital`.
- `backed`: `pool.c_tot + pool.insurance + max(snapshot_residual - P, 0) <= pool.vault`, where
  `P` is the sum of `paid_effective` over present receipts.

The engine's snapshot state satisfies it when `c_tot` is the sum of capital and
`c_tot + insurance <= vault`: no receipts, `exact = 0`, `P = 0`, and `snapshot_residual` is at
most `vault - c_tot - insurance` (`residual()`, engine `src/v16.rs:10602`).

## Properties

Each is for every slice length and all values the types allow. "Returns" means returns `Ok`.
Primed values are after the call. `G` is the sum over present receipts of
`floor(face * rate_num / rate_den)`.

- RC1. `Inv` implies `G <= snapshot_residual` and `P <= G`.
- RC2. If `Inv` and `close_resolved_payout_tail(pool, &mut accounts[i])` returns `payout`, then
  `Inv` holds after; `vault' + payout == vault`; `payout == capital_i + (P' - P)`;
  `c_tot' + capital_i == c_tot`; the account has `capital == 0` and `pnl == 0`; if its receipt
  is present, `paid_effective' == floor(face * rate_num / rate_den)`; no other account changed;
  `insurance`, `snapshot_residual`, `current_payout_rate_num` and `current_payout_rate_den` did
  not change. (The `min(.., vault)` cap never cuts the payout, so no capital is zeroed unpaid.)
- RC3. If `Inv`, `pnl_i >= 0`, and either the account's receipt is present or
  `pnl_i * BOUND_SCALE <= terminal_claim_bound_unreceipted_num`, then
  `close_resolved_payout_tail` does not return `Err` and does not panic.
- RC4. If `Inv` and `close_all(pool, accounts, order)` returns `total`, then `Inv` holds after;
  `vault' + total == vault`; `total == (C - C') + (P' - P)` with `C` the sum of capital;
  `P' <= snapshot_residual`; `c_tot' + insurance' <= vault'`; `insurance' == insurance`; every
  account whose index is in `order` has `capital == 0` and `pnl == 0`.
- RC5. If in addition every index below `accounts.len()` occurs in `order`, then `c_tot' == 0`,
  `total == C + (P' - P) <= C + snapshot_residual - P`, and `insurance <= vault'`.
- RC6. If `Inv`, `order2` is a permutation of `order1`, and `close_all` returns on `order1`, then
  from the same starting state it returns on `order2` with the same `total`, the same pool and
  the same accounts.
- RC7. If `Inv` and `claim_resolved_payout_topup_core(pool, &mut accounts[i])` returns `payout`,
  then `Inv` holds after, `vault' + payout == vault`, `P' == P + payout`, and capital is unchanged.

## What is already known, and how

- The arithmetic of `src/wide_math.rs`, including `wide_mul_div_floor_u128(a, b, d) == floor(a*b/d)`
  when `d > 0` and the quotient fits in `u128`: proved by Aristotle for the upstream engine
  (aeyakovenko/percolator, `proofs/proof_ledger.tsv`, layer A). That file is not this one: `src/wide_math.rs`
  here differs from upstream master's in three hunks (`git diff`), none of them in
  `wide_mul_div_floor_u128` or in anything above line 1420 of the file, which is where that function's
  callees are. So the contract is used as a hypothesis here, not as a proved fact.
- Kani (bounded model checking) harnesses in the engine repository about this path, for one or
  two accounts and bounded values: `proof_v16_resolved_receipt_payment_cannot_exceed_terminal_claim`,
  `proof_v16_resolved_receipt_claimable_is_rate_monotone_and_overpaid_fails_closed`,
  `proof_v16_public_resolved_payout_topup_pays_min_claimable_and_vault`,
  `proof_v16_two_resolved_receipts_are_order_independent_when_snapshot_funded` (all in
  `tests/proofs_v16.rs`), and the end-to-end harnesses of `tests/proofs_e2e_resolved_close.rs`
  (`proof_e2e_resolved_close_pays_exactly_owed_and_conserves_value`,
  `proof_e2e_resolved_close_pays_exactly_owed_fixed_price`,
  `proof_e2e_resolved_close_flat_account_with_loss_pays_capital_net_of_loss`: one asset, two
  accounts, 8-bit amounts).
- None of them quantifies over the number of accounts or the length of the close sequence. That
  is the whole of this task.
