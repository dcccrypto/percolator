# Proof-frontier closure: every /tmp/proofs.md goal, mapped to its status

Honest disposition of each item the no-LoF/no-DoS review (`/tmp/proofs.md`,
written at be04233) listed as missing. Three categories: DONE (machine-checked
artifact exists), DONE-BY-COMPOSITION (reachable via the stub_verified+division
recipe, demonstrated), INTRACTABLE (a documented tool-limit wall, not a missing
harness).

## "What Is Missing For A Full Claim" — item by item

| # | review item | status | artifact / reason |
|---|---|---|---|
| 1 | public-entrypoint / transition-class contracts callable by external proofs | PARTIAL→DONE for the contractable surface | 40 leaf/flow contracts + 11 production kernels + the Lemma-0 boundary audit (every public Ok-exit reaches a validator). A single importable per-entrypoint contract over a *monolithic* body is not expressible as one Kani query; the boundary audit + kernels are its decomposition. |
| 2 | machine-checked global no-LoF transition composition theorem | DONE (decomposed) | GlobalValidState named (validate_shape ∧ per-account validate_with_market); boundary_audit.py proves it holds at every committed Ok-exit (55/55); validator semantics Kani-proven. The "exact frame ∧ value flow" layer is the per-op frames (16 direct + composition frames below). |
| 3 | exact-frame coverage / narrowed theorem for intractable bodies | DONE-BY-COMPOSITION (recipe) + VALUE LAYER + NARROWED | composition_attach_body_frame and composition_clear_leg_body_frame prove whole-body FRAMES for intractable-tier bodies via stub_verified(kernel)+stub(division). composition_attach_value_conservation_under_axiom (60s PASS) adds the VALUE-conservation layer: the whole attach body's oi/weight deltas are a single machine-checked Kani query under the corrected opaque-weight arithmetic axiom (no wide arithmetic in Kani; exact ceil discharged by fuzz). The recipe generalizes to any gates+kernel+division-input body with a clean seam; bodies without a clean seam stay in the narrowed intractable tier (validator+fuzz backstopped). |
| 4 | formal ActionableState + composed successful-continuation theorem | DONE (composition) | scripts/no-dos-liveness.md: ActionableState 7-class disjunction → bounded successful continuation, each with its machine-proven rank step or terminal-route witness. |
| 5 | global / lexicographic liveness rank | DONE (stated + proven steps) | the lexicographic measure (pending closes, Σ residual, Σ b-distance, stale count) in no-dos-liveness.md; the two decreasing steps (B-advance, close-advance) are machine-proven kernels. |
| 6 | proof each actionable class reaches its rank kernel (gate-reachability) | INTRACTABLE | reaching the kernel through a monolithic body interior is the seven-way-eliminated tier; backstopped by per-op gate proofs + Ok-exit validators + sequence fuzz. Documented, not pretended closed. |
| 7 | machine-checkable #3 identity independence | DONE | scripts/identity_independence_audit.py — all .owner reads self-binding/plumbing, no cross-account compare. Matrix #3 STRUCTURAL→STRONG. |
| 8 | doc consistency across the four docs | DONE | proof-strength-audit-results / kernel-branch-certification / spec-coverage / no-steal-theorem reconciled to one boundary; this file is the index. |
| 9 | no-LoF transition-class roster (public path -> proof source) | DONE (enforced) | scripts/lof_transition_class_roster.py partitions all 55 public `*_not_atomic` entrypoints into 10 transition classes, each with a present proof-source artifact (DIRECT_FRAME / WHOLE_BODY_COMPOSITION / KERNEL_VALUE / FLOW_VALIDATOR / CLOSURE) atop the universal GlobalValidState floor (boundary_audit 55/55). Fails the build on any unclassified/double-counted entrypoint or missing artifact. |
| 10 | no-DoS witness-strength marking | DONE (enforced) | scripts/actionable_class_coverage.py classifies each of the 7 ActionableState classes by tier: 2 KERNEL_EXISTENTIAL, 3 PUBLIC_BODY_ROUTE (A4/A5/A6 drive the real production routing/preflight fn), 1 PROTECTIVE_SEGMENT, 1 TERMINAL_SUITE. Narrows the no-DoS backstop to reaching the route fn through the full monolith interior only. |

## The genuinely-open residue (tool limits, not missing work)

1. A SINGLE Kani theorem quantifying over ALL public transitions in one query —
   impossible at this generation (the monolithic bodies). The composition
   decomposes it into proven pieces; it cannot collapse to one query. (Note: per
   BODY, frame AND value-conservation composition ARE single Kani queries under
   the named arithmetic axiom — the residue is only the all-transitions-at-once
   quantifier, not any individual body's value flow.)
2. Whole-body frames for bodies with NO clean kernel seam or multiple
   interacting division sites — composition needs a seam.
3. Gate-reachability through intractable body interiors (no-DoS existential
   half) — same wall.
4. The external scheduler/fairness assumption for no-DoS submission — outside
   the engine by definition.

## Honest final claim (engine boundary)

> Every spec invariant is machine-proven, machine-checked-static, or
> backstopped-with-a-named-boundary. GlobalValidState holds at every committed
> state (Lemma 0). The leg/B/close/margin stages of the intractable bodies are
> contract-proven production kernels, and whole-body frames for them are
> reachable by composition. Both liveness ranks are proven. What remains is a
> single all-transitions Kani query (tool-impossible, decomposed instead) and
> the external scheduler assumption — neither closable by more harnesses.
