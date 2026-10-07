# S10: unclaimed-backing rebalance, Kani design (NOT RUN; one run on the final code)

Function: `rebalance_unclaimed_backing_across_asset_domains_not_atomic(asset_index)` (`src/v16.rs`).
A pure re-attribution between the two source domains of ONE asset: no token, no account field, no
claim changes. Callers: the end of `apply_account_kf_settlement_entry` (after the leg's own cohort
discharge and `set_asset_state`, under the V1 guard) and `s10_retry_for_unpositioned_asset` (end of
`permissionless_crank_not_atomic`, Refresh action only, asset with no stored position).

## Rule

1. **Budget (third review, T3/T4):** `s10_moves_left` per `MarketGroupV16ViewMut` (= one instruction)
   is **0 by default** (`new`). Only `new_crank` and `permissionless_crank_not_atomic` with the
   `Refresh` action set it to `S10_MAX_MOVES_PER_INSTRUCTION` (2). So trades, batches (including the
   band-scoped batch of the bound-vault-LP route), liquidations, recovery entry points, withdraws and
   every future entry point are budget-free unless explicitly granted one; nothing has to remember to
   zero it. The retry hook is reachable only from the refresh crank.
2. **V1 guard (settle entry):** both `stale_account_count_*` are 0 (every stored leg settled to the
   current K/F cohort). Retry hook: additionally both `stored_pos_count_*` are 0.
3. **Ownership:** `provider_principal_{long,short}` mirrors the principal of every provider of a
   bucket. It has ONE writer, `adjust_slot_provider_principal` (add; subtract saturating at 0), called
   by the engine's `deposit_/withdraw_fresh_counterparty_backing_not_atomic` (tag 50 providers) and by
   the wrapper's `vault_pot_owned_adjust` (every Earn / LP-vault pot funding and draw: tags 75, 77,
   91, the fee crank, sibling top-up, recall, rescue, resolved settle/absorb: 17 add sites and 4
   inline subtractions, all through that helper), plus the wholly-empty reset in
   `set_backing_bucket_for_domain` and the activation reset.
4. **Share (founder choice, `S10_PROTECT_FULL_PROVIDER_PRINCIPAL`, default `true`):**
   rule A `provider_fresh = principal` (the wrapper withdraws up to the full ledger principal, so
   nothing a provider or vault holder could withdraw on the base engine ever moves);
   rule B `provider_fresh = sat(principal - consumed - impaired - valid_liened)` (the ledger NAV's
   arithmetic; moves loser cash that base would have let the provider withdraw). Only
   `loser_cash = fresh_unliened - provider_fresh` may move.
5. **Amount:** `min(loser_cash(src), available(src) - claims(src), claims(dst) - available(dst))`, whole
   atoms, both directions, `>= S10_MIN_MOVE_ATOMS` (1,000, founder-tunable), else skipped. The budget
   check is FIRST (before any read), then bucket and mirror, then source-credit state.
6. **Guards inside:** source `Fresh` and `expiry_slot > now`; destination accepts a booking
   (`loss_domain_accepts_realized_backing`) and is not `Impaired`.
7. **Moved backing's expiry:** it takes the destination's expiry (an existing Fresh bucket keeps its
   own; Empty/Expired opens at `now + horizon`), losing the source bucket's lifetime.
8. **Receivable refill:** the add delta first repays the destination's `provider_receivable`.

## Proof obligations (reviewer's T8 list), to be proved once

1. **Mirror writer contract:** `adjust_slot_provider_principal` is the only function that writes
   `provider_principal_*` other than the wholly-empty reset and the activation reset; `add` raises it
   by exactly `delta` (no overflow), `!add` lowers it by `min(delta, current)`. The wrapper cannot be
   proved in the engine crate, so the wrapper's `vault_pot_owned_adjust` calls this one setter and
   that is the only wrapper path to the field (wrapper test: after every pot funding/draw the mirror
   equals the vault-owned counter, `s10_every_pot_funding_and_draw_drives_the_mirror`).
2. **Share-formula bound:** `moved <= fresh_unliened(src) - provider_fresh_by_rule(rule, principal(src),
   bucket(src))` for every shape-valid bucket and both rules; a provider-less bucket (principal 0) is
   wholly movable; rule A never moves anything below `min(fresh, principal)`.
3. **Budget:** `s10_moves_left == 0` on entry of every public entry point except
   `permissionless_crank_not_atomic` (Refresh) and `new_crank` views; every call that reaches
   `apply_account_kf_settlement_entry` or the retry hook respects the budget and the dust floor; a
   skipped move leaves state unchanged and returns `Ok`.
4. **Guard:** `moved > 0` implies the V1 guard (settle entry) or no stored position (hook), and
   `moved >= S10_MIN_MOVE_ATOMS * BOUND_SCALE`.
5. **Totality:** for every shape-valid state the function returns `Ok`: expired source, Impaired /
   Empty / Expired destination, receivable cases, dust floor, budget early exit.
6. **Conservation:** total `fresh_unliened`/`fresh_reserved` across the two domains, liened and impaired
   backing, insurance credit reserved, `spent_backing`, claims and exact claims, vault, insurance,
   `c_tot`, every account, and the mirror are unchanged; destination `provider_receivable` never grows.
7. **Bound / idempotence / ledger / expiry / frame / Resolved snapshot (#223/#224):** as in the previous
   revision (moved is a multiple of `BOUND_SCALE`; a second call moves 0; ledger and reservation proofs
   hold for both domains; destination expiry rule; the settle-entry and crank proofs gain both domains
   and the two mirror fields in their frame).
8. **Covers (vacuity detector):** a move that fires with `provider_principal > 0` and non-zero loser
   cash; **a move skipped because the provider share protects it (rule A and rule B)**; **a move
   skipped for the budget**; a move skipped for the dust floor; the V1-false branch.

Test seams: `rebalance_unclaimed_backing_for_test_not_atomic`, `set_s10_moves_left_for_test`,
`s10_provider_fresh_for_test`, `s10_provider_fresh_by_rule_for_test` (all `#[doc(hidden)]`).

## Not provable by Kani (simulation gates)

Every account at or below ideal in the cadence, attacker, cash-out, LP-victim and forced-shortfall
sweeps, the 600 / 900 / 16-mask worlds, and the LiteSVM pot tests (Earn, junior tranche, rescue,
ordinary provider). See `finding-stranded-backing-2026-10-07.md`.
