# kernel-proofs branch certification

Fresh full-branch re-certification AFTER the production src/v16.rs kernel
refactor (the inherited master certification does not cover this branch).
Every artifact verified IN ISOLATION against the exact kernel-calling
production code on this branch.

Branch HEAD at certification: see git log (kernel-proofs).

| layer | count | result | notes |
|---|---:|---|---|
| suite (tests/proofs_v16.rs) | 215 | 215/215 PASS | re-run against kernel-refactored bodies (position-delta, trade, liquidation, close, B-settlement families all exercise the extracted kernels) |
| contracts (src/v16_proofs.rs, -Z function-contracts) | 40 | 40/40 PASS | leaf/flow contracts + all 7 production kernels |
| closure (src/v16_proofs.rs, plain) | 17 | 17/17 PASS | inductive ledger/status closure |
| close-rank witness | 1 | PASS (389s) | kernel_advance_close_ledger rank, plain-witness form |

Total: 273/273 base + 4 added kernels (batch-projection, restart counters/slot) all PASS; the changed trade-batch/restart/leg production paths are re-verified by a targeted suite re-run: 47/47 PASS (kani_recert), so the batch + restart kernel extractions preserved suite-level semantics. Plus 2 whole-body composition frames (attach 117s, clear_leg 227s) reached via stub_verified(kernel)+stub(division).

## The 11 production kernels (extracted from the intractable bodies, production calls them)
| kernel | property proven | solver |
|---|---|---|
| kernel_resize_leg_same_side | exact OI/weight delta, complete leg+asset frame | 25s |
| kernel_attach_leg | leg built exactly from side snapshots; OI/count/weight exact | 14s |
| kernel_clear_leg | count/obligation/dust/OI/weight case-split exact | 13s |
| kernel_advance_leg_b_snap | LIVENESS RANK: b_snap advances by exactly delta_b | 1s |
| kernel_advance_close_ledger | LIVENESS RANK: residual decreases by exactly booked total; sticky finalize | 389s (witness) |
| kernel_initial_margin_gate | EXACT total decision: Ok <=> valid cert + equity >= IM | 1s |
| kernel_locked_margin_gate | positive PnL credit never satisfies IM under h-lock | 4s |
| kernel_accumulate_batch_trade | #37 batch projection: batch outcome = EXACT fold of per-fill outcomes | 6s |
| asset_restart_next_counters | #29 restart bumps the 4 global counters by exactly 1 (fresh market identity) | 4s |
| restarted_asset_slot_preserving_insurance_budget | #29 restart preserves insurance budget exactly, zero carried OI/count/barrier/spend (witness) | 6s |

## What this certifies, and what it does not
CERTIFIES: every inventory artifact holds against the branch's actual
production code; the kernel refactor preserved all proof-level semantics
(not merely runtime-test parity); the boundary theorem (scripts/boundary_audit.py,
55/55) and both liveness-rank components are production-proven.

DOES NOT close (documented boundary, scripts/no-steal-theorem.md): the
universal GlobalValidState contract theorem over every public transition,
monolithic-body exact frames (intractable tier), and the composed global
no-DoS rank theorem. The kernels are the contractable stages those would
compose from; the composition itself remains future work.
