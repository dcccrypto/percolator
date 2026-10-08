# S10: unclaimed-backing rebalance, Kani design (NOT RUN; one run on the final code)

Revision 2026-10-08 (after the fourth review, U0-U11).

Function: `rebalance_unclaimed_backing_across_asset_domains_not_atomic(asset_index)` (`src/v16.rs`).
A pure re-attribution between the two source domains of ONE asset: no token, no account field, no
claim changes. Callers: the end of `apply_account_kf_settlement_entry` (after the leg's own cohort
discharge and `set_asset_state`, under the V1 guard) and `s10_retry_for_unpositioned_asset` (end of
`permissionless_crank_not_atomic`, Refresh action only, asset with no stored position).

## Rule

1. **Budget (third review, T3/T4; fourth review, U4/U11):** `s10_moves_left` per
   `MarketGroupV16ViewMut` (= one view) is **0 by default** (`new`). Only `new_crank` (test model
   only: the program never calls it) and `permissionless_crank_not_atomic` with the `Refresh` action
   set it to `S10_MAX_MOVES_PER_INSTRUCTION` (2), and the Refresh grant is withheld on a view that
   called `deny_s10_budget` (the wrapper's tag-77 redemption does so for its inline refreshes: two
   14-leg refreshes are about 1.25M CU). Production grants therefore exist in exactly two places:
   the tag-5 crank when the engine selects the Refresh plan, and tag 106 (`handle_settle_holding_rent`).
   **Consequences that are part of the specification, not defects to prove away:** the repair is
   Live only; `close_resolved_account_not_atomic` and every Resolved path carry no budget and repair
   nothing; a pending move waits until a Refresh crank runs a settle entry of the asset with both
   stale counts 0 (or accrues an asset with no stored position), and waits indefinitely if none does. So trades, batches (including the
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
   check is FIRST (before any read); then the idle fast path on the raw slot (return when neither
   domain holds loser cash: `fresh == 0`, or under rule A `fresh <= principal`); then bucket and
   mirror; then source-credit state. The fast path is a refinement: it returns exactly when both
   passes of the loop would `continue` at their first check.
6. **Guards inside:** source `Fresh` and `expiry_slot > now`; destination accepts a booking
   (`loss_domain_accepts_realized_backing`) and is not `Impaired`.
7. **Moved backing's expiry:** it takes the destination's expiry (an existing Fresh bucket keeps its
   own; Empty/Expired opens at `now + horizon`), losing the source bucket's lifetime.
8. **Receivable refill:** the add delta first repays the destination's `provider_receivable`.

## Proof obligations (reviewer's U11 list, replaces T8), to be proved once on the final code

1. **Mirror setter.** The only writers of `provider_principal_*` are `adjust_slot_provider_principal`,
   the wholly-empty reset in `set_backing_bucket_for_domain` and the activation reset. `add` raises
   the field by exactly `delta` (overflow is an error, state unchanged); `!add` lowers it by
   `min(delta, current)`. The wrapper cannot be proved in the engine crate: its
   `vault_pot_owned_adjust` is the single wrapper path to the setter, and the evidence is the
   integration tests that compare the mirror with the WRAPPER LEDGER principal (not the vault-owned
   counter, which the same helper moves) after every pot funding and draw: Earn lifecycle, recall,
   tag 91 rebalance both ways, cross-pot redemption, Resolved settle, junior terminal sweep, Resolved
   redemption (`s10_mirror_equals_ledger_principal_*` in the wrapper suite).
2. **Share bound.** `moved <= fresh_unliened(src) - provider_fresh_by_rule(rule, principal(src),
   bucket(src))` for every shape-valid bucket and both rules; rule A never takes fresh below
   `min(fresh, principal)`; a provider-less bucket (principal 0) is wholly movable. The default rule
   is a constant pinned by a test, not a proof input.
3. **Budget.** `s10_moves_left == 0` and `s10_grant_denied == false` at construction (`new`); the
   budget becomes non-zero only in `new_crank` and in `permissionless_crank_not_atomic` with the
   Refresh action on a view that was not denied; `deny_s10_budget` makes it 0 for the life of the
   view. Every path that reaches `apply_account_kf_settlement_entry` or the retry hook respects the
   budget and the dust floor. A skipped move (budget, fast path, dust floor, provider share, guards)
   is a no-op returning `Ok`.
4. **Guard.** `moved > 0` implies both stale counts are 0 (settle entry), or additionally both stored
   position counts are 0 (retry hook); and `moved >= S10_MIN_MOVE_ATOMS * BOUND_SCALE`.
5. **Fast-path refinement (new).** For every shape-valid state, the function with the idle fast path
   and the function without it produce the same post-state and result.
6. **Totality and conservation.** For every shape-valid state the function returns `Ok` (expired
   source, Impaired / Empty / Expired destination, receivable cases). Total `fresh_unliened` and
   `fresh_reserved` across the two domains, claims and exact claims, liened and impaired backing,
   insurance credit reserved, `spent_backing`, vault, insurance, `c_tot`, every account and the
   mirror are unchanged; `moved` is a multiple of `BOUND_SCALE`; destination `provider_receivable`
   never grows; a second call moves 0; ledger and reservation proofs hold for both domains; the
   destination expiry rule; the settle-entry and crank proofs gain both domains and the two mirror
   fields in their frame.
7. **Covers (vacuity detector).** A move that fires with provider principal and loser cash both
   present; a move skipped by the provider share; by the budget; by a denied grant; by the dust
   floor; by V1; and the fast-path exit taken with a non-zero principal.

## What no proof here will say (state it wherever the result is quoted)

- **"Providers are never worse than on the base engine" is FALSE and is not an obligation.** The
  share bound holds at move time only. Loser cash that has moved out no longer cushions the provider
  against later claim consumption of the same bucket. Reviewer's seed 3000121: a move takes 2,441,725
  of loser cash from a bucket holding 10.1M fresh against a 2.925M principal; later consumption leaves
  fresh 3,257,400 against a 3,632,500 principal (375,100 short, 10 percent) where the base engine holds
  5,699,125. Counts in the reviewer's lien-heavy generator: 5, 14, 2, 9 of 4,000 worlds, the same
  under rule A and rule B. The stronger property needs a different rule (founder decision): never
  move loser cash out of a bucket in which a provider holds principal.
- **Resolved.** No obligation covers a Resolved repair, because there is none.
- **Liveness of the repair.** Nothing proves a pending move is ever made; it depends on a Refresh
  crank completing a cohort.

Test seams: `rebalance_unclaimed_backing_for_test_not_atomic`, `set_s10_moves_left_for_test`,
`s10_provider_fresh_for_test`, `s10_provider_fresh_by_rule_for_test` (all `#[doc(hidden)]`).

## Not provable by Kani (simulation gates)

All under the PRODUCTION budget model (a move budget for a refresh crank only; trades, accrual,
deposits, resolve and the Resolved close through a default view): every account at or below ideal in
the cadence, attacker, cash-out, LP-victim and forced-deficit sweeps; the 600 / 900 / 16-mask worlds;
the mirror-equals-ledger-principal tests; the LiteSVM pot tests (Earn, junior tranche, rescue,
ordinary provider); and the "later consumption" check on the provider cushion (reviewer's U3
generator), whose expected result is a small non-zero count, not zero.
See `finding-stranded-backing-2026-10-07.md`.
