# S10: unclaimed-backing rebalance, Kani design (NOT RUN; one run on the final code)

Function: `rebalance_unclaimed_backing_across_asset_domains_not_atomic(asset_index)` (`src/v16.rs`).
A pure re-attribution between the two source domains of ONE asset: no token, no account field, no
claim changes. Callers: the end of `apply_account_kf_settlement_entry` (after the leg's own cohort
discharge and `set_asset_state`, only under the V1 guard) and `s10_retry_for_unpositioned_asset`
(end of `accrue_asset_to_not_atomic` / `accrue_asset_path_to_not_atomic`, only when the asset has no
stored position).

## Rule

1. **V1 guard (settle entry):** `stale_account_count_long == 0 && stale_account_count_short == 0`
   (every stored leg settled to the current K/F cohort). Idle cost: two compares. Accrual retry:
   additionally `stored_pos_count_* == 0`.
2. **Ownership (re-review R1.1):** `provider_principal_{long,short}` mirrors the wrapper ledger's
   principal (`+` deposit, saturating `-` withdraw, reset when the bucket is wholly empty). The part
   of a bucket the provider owns is `provider_fresh = principal - consumed - impaired - valid_liened`
   (saturating), exactly the wrapper ledger's `principal - (loss - recovery)` with loss = consumed +
   impaired. Only `loser_cash = fresh_unliened - provider_fresh` may move. A receivable refill
   raises `provider_fresh` (provider recovery), consumption lowers it (provider loss): engine and
   wrapper agree on who owns each atom at every step. Invariant in one sentence: *fresh backing
   beyond the provider's ledger share is loser cash, and only that may move; a provider-less bucket
   (principal 0) is wholly loser cash.*
3. **Amount:** `min(loser_cash(src), available(src) - claims(src), claims(dst) - available(dst))`,
   whole atoms, both directions, and `>= S10_MIN_MOVE_ATOMS` atoms (dust floor, founder-tunable,
   default 1,000), else skipped.
4. **Guards inside:** source `Fresh` and `expiry_slot > now`; destination accepts a booking
   (`loss_domain_accepts_realized_backing`) and is not `Impaired`. The budget check is FIRST, before
   any read, then bucket and principal mirror before any source-credit state.
5. **Budget:** `s10_moves_left` per `MarketGroupV16ViewMut` (= one instruction), default
   `S10_MAX_MOVES_PER_INSTRUCTION` = 2, forced to 0 inside `execute_batch_with_fee_loss_stale_scoped`
   (trades, batches) and `liquidate_account_not_atomic` (liquidations): an 11-leg liquidation crank
   is 1,375,975 CU of 1.4M and an 11-leg batch about 1.33M, so no firing may be added there.
   Order within an instruction is deterministic: account order, then the account's leg plan order
   (phase, source domain, slot) = ascending asset index. A skipped move is retried by the next
   settle entry of the asset and, with no stored position, by the next accrual of the asset.
6. **Moved backing's expiry:** it joins the destination bucket and takes the destination's expiry
   (an existing Fresh bucket keeps its own; Empty/Expired opens at `now + horizon`), losing the
   source bucket's own lifetime (later or earlier).
7. **Receivable refill:** the add delta first repays the destination's `provider_receivable`.

## Proof obligations (reviewer's R8 list, rewritten for the provider mirror), to be proved once

1. Guard: `moved > 0` implies the V1 guard (settle entry) or no stored position (accrual retry), AND
   `moved <= loser_cash(src)` and `fresh_unliened(src) - moved >= provider_fresh(src)`.
2. Ownership invariant (the key one): after every writer of a bucket (`set_backing_bucket_for_domain`,
   15 call sites) and of the mirror (deposit, withdraw, wholly-empty reset), `provider_fresh <=
   fresh_unliened` holds whenever it held before and the writer is not an expiry/lapse; a deposit
   raises principal and fresh together; a withdraw lowers principal by at most the amount; the move
   never changes principal, consumed, impaired or liened. Prove per writer with one contract on the
   setter.
3. Attribution agreement: `provider_fresh` equals the wrapper ledger's available principal
   `total_principal - (consumed + impaired) - liened` when the mirror equals the ledger principal.
4. Cap: moves fired per view `<= S10_MAX_MOVES_PER_INSTRUCTION`; zero in trade/batch/liquidation
   views; a skipped move leaves state unchanged and returns `Ok`; the guard is re-evaluated on every
   settle entry and every accrual of the asset (frame lemma).
5. Totality: for every shape-valid state the function returns `Ok` (no new fail-closed path inside a
   settle): expired source, Impaired destination, Empty/Expired destination, receivable cases, the
   dust floor and the budget early exit.
6. Conservation: total `fresh_unliened` and `fresh_reserved` across the two domains, liened and
   impaired backing, insurance credit reserved, `spent_backing`, claims and exact claims, vault,
   insurance, `c_tot` and every account are unchanged; the destination `provider_receivable` never grows.
7. Bound: `moved <= min(loser_cash(src), available(src) - claims(src), claims(dst) - available(dst))`,
   `moved % BOUND_SCALE == 0`, `moved >= S10_MIN_MOVE_ATOMS * BOUND_SCALE`, destination available
   never above claims, source covered claimants never lose coverage.
8. Idempotence and symmetry: a second call moves 0. Ledger and expiry: `validate_source_domain_ledger`
   and `reservation_encumbrance_proof_for_domain` hold for both domains; destination expiry rule; the
   source bucket status transition matches `prepare_counterparty_backing_withdraw_delta`.
9. Frame: the settle-entry proofs gain both domains of the asset and the two mirror fields in the
   frame; the accrual functions gain the same frame for the retry; nothing else.
10. Covers (vacuity detector): a move that fires with `provider_principal > 0` and a non-zero
    `loser_cash`; a move skipped for the budget; a move skipped for the dust floor; the V1-false branch.
11. Resolved: the move under Resolved mode preserves the payout-snapshot invariants (#223/#224 open
    upstream; both read the same buckets).

Contract shim to add: `kani_rebalance_unclaimed_backing_across_asset_domains_not_atomic` next to the
R1 shims (the test seam `rebalance_unclaimed_backing_for_test_not_atomic` already bypasses V1).

## Not provable by Kani (simulation gates)

Every account at or below ideal in random-cadence, attacker-schedule, cash-out, LP-victim and the
reviewer's forced-shortfall (`sec_ring_exploit`) sweeps, and the 600 / 900 / 16-mask worlds. See
`finding-stranded-backing-2026-10-07.md`.
