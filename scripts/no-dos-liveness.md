# Engine no-DoS / liveness: ActionableState → bounded successful continuation

Status: composition document for the pure-engine liveness argument, in the
shape the proof-frontier review asks for. It defines the actionable-state
predicate, maps every actionable class to a successful public continuation
and the machine-proven rank component that continuation decreases, and states
explicitly what is proven, what is backstopped, and what is assumed.

Convention (per scripts/no-steal-theorem.md / the review): public engine
contracts are Ok-postcondition contracts; `Err` fully reverts at the
execution boundary, so a rejected call is safe but is NOT progress. no-DoS is
the EXISTENCE of a successful continuation, not the classification of failures.

## The liveness target

```text
forall reachable valid state S:
  ActionableState(S) =>
    exists a public call C (account-local, bounded work) such that
      C(S) = Ok, and the post-state either
        - strictly decreases a well-founded rank, OR
        - reaches a current/healthy/certified state, OR
        - reduces/closes/liquidates a position, OR
        - records a terminal recovery state.
```

## ActionableState — the disjunction of concrete classes

A valid state is actionable iff at least one of:

| class | predicate (engine fields) | actor |
|---|---|---|
| A1 stale account | `stale_state != 0` or cert invalid for current epochs | permissionless crank / refresh |
| A2 b-stale leg | a leg with `b_stale` or `b_target > b_snap` | permissionless crank (B chunk) |
| A3 pending close residual | `close_progress.active && residual_remaining > 0 && now <= max_close_slot` | close-progress advance |
| A4 expired close | `close_progress.active && now > max_close_slot` | recovery declaration |
| A5 liquidatable | maintenance deficit with open risk | liquidate / recovery route |
| A6 recovery-eligible | a stuck class (B exhaustion, underbacking, lock/barrier failure, …) | permissionless recovery crank |
| A7 resolved winner | `mode == Resolved` with an open source-backed claim or capital | close_resolved (terminal) |

A state in NONE of these classes is current/healthy/terminal — there is
nothing to make progress on, so "all continuations reject" is not a DoS, it
is the absence of pending work.

## Class → successful continuation → machine-proven rank component

| class | successful continuation | rank / terminal effect | proven by |
|---|---|---|---|
| A2 b-stale | `settle_account_b_chunk` | `b_snap += exactly delta_b` toward target (distance strictly ↓) | **kernel_advance_leg_b_snap** (contract, full domain) |
| A3 pending residual | `advance_close_progress_ledger` | `residual_remaining -= exactly booked`; finalize at 0 | **kernel_advance_close_ledger** (rank witness, full domain) |
| A4 expired close | recovery declaration | terminal recovery recorded, value-neutral | proof_v16_expired_close_progress_declares_recovery_without_value_mutation |
| A5 liquidatable | liquidate (or route to recovery) | improves risk or routes terminal; never strands loss | proof_v16_liquidation_* (preflight accept/route + no-uncovered-loss) |
| A6 recovery-eligible | permissionless recovery crank | terminal recovery, accounting-neutral | proof_v16_permissionless_recovery_crank_is_accounting_neutral |
| A1 stale account | accrual/refresh crank | commits one bounded segment of protective progress before mutation | proof_v16_equity_active_accrual_with_progress_commits_one_bounded_segment; frame_crank (clock/cert only) |
| (clock) | empty-market crank | clock advances, value-flat (bounded) | proof_v16_public_permissionless_empty_market_crank_advances_clock... |
| A7 resolved winner | close_resolved | terminal: realize at rate then dematerialize | terminal-realization suite proofs + backing fuzz |

Every rank STEP (A2, A3) is a machine-proven property of PRODUCTION code (the
monoliths call the kernels). Every terminal route (A4, A5, A6) has a success
witness. The well-founded measure is lexicographic:
`(pending closes, Σ residual_remaining, Σ b-target − b_snap, stale count)` —
each listed continuation strictly decreases one component without increasing a
higher one (B advance can't create a close; close advance can't unstale a B
leg it didn't touch; recovery is terminal).

## What is proven, backstopped, and assumed — exactly

PROVEN (machine-checked):
- the rank STEPS for B settlement and close progress (the two kernels);
- the terminal routes for expired-close / liquidation-insufficient / recovery;
- that bounded account-local work suffices (Kani unwind bounds on the harnessed
  continuation paths; struct-sized loops, req #33/#34 STRONG).

BACKSTOPPED (not a single Kani theorem): the GATE-REACHABILITY half — that an
actionable state actually reaches its rank kernel through the monolithic body
interior (trade/liquidation/close), rather than the body rejecting for an
unrelated reason. This is the intractable tier (seven-way elimination,
src/v16_proofs.rs). It is covered by: the per-op gate proofs (the gates accept
the actionable shapes), the runtime validators on every Ok exit
(scripts/boundary_audit.py, 55/55), and the order-independence / extraction /
double-claim fuzz over full close sequences.

ASSUMED (named, outside the engine — the review's own caveat): an external
actor SUBMITS the successful continuation. The engine proves a successful
bounded continuation EXISTS; it cannot prove a cranker will call it without a
scheduler/fairness assumption. The permissionless crank-forward design (req
#35, STRONG) makes every such continuation callable by ANY actor, which is the
strongest the engine layer can provide.

## Honest claim

> For every actionable engine state, a bounded account-local successful
> public continuation EXISTS whose effect decreases a machine-proven rank or
> records terminal recovery; the rank steps and terminal routes are
> machine-checked, the gate-reachability through intractable bodies is
> validator+fuzz backstopped, and submission of the continuation is an
> explicit external scheduler assumption.

This is the complete no-DoS argument at the pure-engine boundary. The single
composed existential theorem over all classes is not expressible as one Kani
query (the intractable bodies); the decomposition above is its honest
realization.
