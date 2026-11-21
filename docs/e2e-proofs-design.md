# End-to-end invariants: what Kani can close, what needs Lean/Aristotle

Status: design note, 2026-11-21. Branch `agent/J-20261121-ENG-E2E-PROOFS` (engine 3e5970f3 +
headline kernel proofs c38b6b1e). Companion: `proofs/aristotle/` (statements, prompts, ledger).

## Why this note exists

The existing Kani suite proves kernels (pure `V16Core` functions over full u128/i128 domains) and
a handful of public entry points on a concrete one-asset fixture with u8-sized amounts. The two
headline proofs of 2026-11-20 are kernel-level only (trigger kernels; fee kernel). Whole-engine
harnesses over symbolic account structs do not finish (tests/proofs_gh292_latch.rs header: the
~10 KB account / header structs force CBMC to unwind memcmp/copies; unwinding failures at 4, 10,
40; 25 min timeouts at 260). So "end to end" here has to mean: **call the real public entry point
on a state built by real public operations, with the economically relevant scalars symbolic over
a small honest range**, and everything else concrete. That is a bounded proof, not an inductive
one, and each harness states its bounds.

## The four invariants, ranked

| # | Invariant | Why it matters | Kani, end to end (bounded) | Needs unbounded (Lean/Aristotle) |
|---|---|---|---|---|
| a | Value conservation across trade + liquidate + settle + resolved close: `vault` moves only by external in/out; `c_tot + insurance + Σ(pnl claims)` moves only by booked losses/fees, nothing created | A leak here is a direct theft / insolvency bug | Per entry point, on a 1-asset, 2-account world: symbolic deposit/capital (u8..u16), concrete trade size & price, symbolic price step for liquidation; assert exact deltas of `vault`, `c_tot`, `insurance`, per-account capital, and `V >= C_tot + I` after each step. Chain them as a sequence in one harness only if it finishes; otherwise compose per-step post => next pre. | Arbitrary account count N, any configured asset count (`max_market_slots`), arbitrary amounts and arbitrary interleavings: an inductive invariant `M` (header totals = sums over accounts) preserved by every entry point. Kani cannot quantify over N accounts. |
| b | A solvent account is never liquidated through the FULL `liquidate_account_not_atomic` path | Wrongful liquidation is the most visible user harm | Real deposit -> real trade -> (symbolic) price accrual -> real `liquidate_account_not_atomic`. Assert: solvent by an independent spec formula => `Err(NonProgress)` and leg/capital/vault/c_tot/insurance unchanged. Insolvent => closes > 0. Kernel-level result already proved (headline #1). | That `account_liquidation_equity` / maintenance computed by refresh equal the spec formula for every multi-leg portfolio with haircuts and in-flight support. |
| c | Senior solvency `c_tot + insurance <= vault` after settle (negative-PnL principal settlement, resolved bankruptcy settlement) | The senior leg (deposits + insurance) must always be backed | Step-level inductive proof over FULL u128 domains (existing harness `proof_v16_inductive_settle_negative_pnl_preserves_senior_solvency`, which currently FAILS — see REPORT); plus a public-path harness through a real settle entry point on the fixture. | Preservation by every entry point for every reachable state (needs the global invariant). |
| d | Resolved close pays out exactly what is owed | Terminal exit: the user's final claim | Real `close_resolved_account_not_atomic` on a Resolved market, symbolic capital, symbolic negative pnl, symbolic senior/vault state: payout == exact spec value, vault/c_tot deltas exact, account zeroed. Positive-pnl receipts (payout snapshot) only if they finish. | Exactness for positive PnL against the payout snapshot over all accounts (pro-rata haircut), for every close order. |

## Honest bounds policy

* Real production functions only; no re-implementations. Where the harness has to compute the
  expected value, it uses an independent spec formula written in the harness (stated as such).
* Symbolic: the scalars that drive the property (capital, pnl, price step, insurance, surplus).
  Concrete: structural shape (1 asset slot, 1-2 accounts, leg slot 0), config, slots.
* Ranges are as large as finish in time on this machine (u8 to u16 typically); each harness
  prints its range in the doc comment and in REPORT.md.
* Non-vacuity: `kani::cover!` on each branch of the asserted implication.
* Negative control: a scratch mutation of the engine line that implements the property; the
  harness must then FAIL. Recorded, then restored (`git diff --stat src` empty after).
* If a full path does not finish: compose from the largest real sub-steps that do finish, and
  state precisely which link is the narrowed one.

## What stays for Lean/Aristotle

Arbitrary N accounts and any configured asset count; arbitrary amounts; arbitrary sequences. Prepared as
abstract-state statements in `proofs/aristotle/statements/Spec/E2E*.lean` plus prompts that tell
Aristotle (a) which Rust functions the abstract step refines, (b) what Kani already proves so it
only targets the gap. Nothing is submitted (no key).

## Results of the 2026-11-21 run (Kani 0.67.0; full detail in the task report)

Proven (SUCCESSFUL), real methods, full u128/i128 domains:
* `charge_trade_fee_taker_only_not_atomic`, the value step of a fill (tests/proofs_e2e_trade_core.rs, 148.6 s);
* `reserve_new_capital_backed_loss_for_source_domain_not_atomic`, the principal settlement the
  refresh path actually runs (tests/proofs_e2e_settle_core.rs, 1026 s);
* `settle_negative_pnl_from_principal_core_not_atomic` (corrected inductive harness, 73.5 s).
Each has a negative control that FAILS.

Did not close: every harness that runs a whole public entry point with an open position (trade,
refresh, liquidate, resolve + close), from a fresh market or from a real-ops byte snapshot
(tests/e2e_snapshot_gen.rs), was stopped after 13–66 min still in CBMC symbolic execution. A
fully concrete run of the liquidation path was also still in symbolic execution after 11 min, so
the cost is the path length over the ~10 KB structs, not the symbolic width. These harnesses
stay compiling behind `--features e2e-long`. Their check functions run natively over the same
bounded domains (`native_*` tests, all pass, with mutants caught), which is bounded testing, not
a proof.

Finding: on the real refresh path the loss leaves principal in
`reserve_new_capital_backed_loss_for_source_domain_not_atomic` (during K/F settlement), and
`settle_negative_pnl_from_principal_core_not_atomic` then sees `paid == 0`. A mutant of the
latter is not caught by refresh-path tests; a mutant of the former is.
