#!/usr/bin/env python3
"""no-DoS liveness: every ActionableState class is covered by >=1 named,
machine-checked witness proof, AND each witness is classified by STRENGTH
(/tmp/proofs.md "extend actionable_class_coverage.py to mark witness strength":
kernel existential / public-body route / protective segment / terminal suite /
external scheduler).

ActionableState is the 7-class disjunction in scripts/no-dos-liveness.md. For
no-DoS to be "every actionable state has a successful bounded continuation",
each class must map to at least one EXISTING proof artifact that witnesses its
continuation. The STRENGTH tier records HOW close the witness is to a full
public-entrypoint routing theorem:

  KERNEL_EXISTENTIAL  - proves ActionableClass(S) => EXISTS a successful
                        rank-decreasing call the proven rank kernel accepts
                        (witness exhibited at the kernel boundary).
  PUBLIC_BODY_ROUTE   - drives a REAL production routing/preflight function on
                        an actionable state and proves it routes to the
                        witness / terminal recovery (not merely a kernel; the
                        production decision fn itself is executed).
  PROTECTIVE_SEGMENT  - proves the actionable state commits one bounded segment
                        of protective progress before any mutation.
  TERMINAL_SUITE      - terminal realization/dematerialization witness.

What remains BACKSTOPPED is narrower than "all routing": only reaching the
PUBLIC_BODY_ROUTE function through the FULL monolithic public-entrypoint
interior (the state-size wall, scripts/no-steal-theorem.md). External actor
submission is an explicit scheduler assumption, outside the engine.

This script fails if any class loses its witness (renamed/deleted proof) or is
left unclassified - turning the class->continuation->strength table into an
enforced invariant rather than prose. It does NOT re-run the proofs.
"""
import re
import sys

KERNEL_EXISTENTIAL = "kernel existential (rank-decreasing call the proven kernel accepts)"
PUBLIC_BODY_ROUTE = "public-body route (drives the real production routing/preflight fn)"
PROTECTIVE_SEGMENT = "protective segment (one bounded protective commit before mutation)"
TERMINAL_SUITE = "terminal-realization suite witness"

# class -> (strength, production fn the witness drives, file, [fn names present])
COVERAGE = {
    "A1 stale account": (
        PROTECTIVE_SEGMENT,
        "refresh_account_and_certify (accrual segment)",
        "tests/proofs_v16.rs",
        ["proof_v16_equity_active_accrual_with_progress_commits_one_bounded_segment"],
    ),
    "A2 b-stale leg": (
        KERNEL_EXISTENTIAL,
        "kernel_advance_leg_b_snap",
        "src/v16_proofs.rs",
        ["liveness_b_stale_leg_has_advancing_chunk"],
    ),
    "A3 pending close residual": (
        KERNEL_EXISTENTIAL,
        "kernel_advance_close_ledger",
        "src/v16_proofs.rs",
        ["liveness_pending_close_has_rank_decreasing_advance"],
    ),
    "A4 expired close": (
        PUBLIC_BODY_ROUTE,
        "ensure_close_progress_not_expired -> declare_permissionless_recovery",
        "tests/proofs_v16.rs",
        ["proof_v16_expired_close_progress_declares_recovery_without_value_mutation"],
    ),
    "A5 liquidatable": (
        PUBLIC_BODY_ROUTE,
        "preflight_liquidation_residual_durability (accept | route-to-recovery)",
        "tests/proofs_v16.rs",
        [
            "proof_v16_liquidation_preflight_accepts_only_fully_durable_residual",
            "proof_v16_liquidation_preflight_routes_insufficient_residual_capacity_to_recovery",
        ],
    ),
    "A6 recovery-eligible": (
        PUBLIC_BODY_ROUTE,
        "permissionless_crank_not_atomic (recover action)",
        "tests/proofs_v16.rs",
        ["proof_v16_permissionless_recovery_crank_is_accounting_neutral"],
    ),
    "A7 resolved winner": (
        TERMINAL_SUITE,
        "close_resolved terminal realization",
        "tests/proofs_v16.rs",
        [
            "proof_v16_resolved_winddown_releases_liened_source_claim",
            "proof_v16_public_resolved_close_flat_account_pays_only_capital_and_vault",
        ],
    ),
}

VALID_STRENGTHS = {KERNEL_EXISTENTIAL, PUBLIC_BODY_ROUTE, PROTECTIVE_SEGMENT, TERMINAL_SUITE}

_cache = {}


def fns_in(path):
    if path not in _cache:
        text = open(path).read()
        _cache[path] = set(re.findall(r"\bfn\s+([A-Za-z0-9_]+)\s*\(", text))
    return _cache[path]


missing = []
unclassified = []
for cls, (strength, _driver, path, fns) in COVERAGE.items():
    if strength not in VALID_STRENGTHS:
        unclassified.append(cls)
    present = fns_in(path)
    for fn in fns:
        if fn not in present:
            missing.append((cls, path, fn))

if missing or unclassified:
    print("ACTIONABLE-CLASS COVERAGE GAP(S):")
    for cls, path, fn in missing:
        print(f"  {cls}: missing witness `{fn}` in {path}")
    for cls in unclassified:
        print(f"  {cls}: witness strength not classified")
    sys.exit(1)

# Tier rollup.
tiers = {}
for cls, (strength, _d, _p, _f) in COVERAGE.items():
    tiers.setdefault(strength, []).append(cls)

print(f"actionable-class coverage OK: all {len(COVERAGE)} ActionableState classes")
print("have a present, named machine-checked witness, classified by strength:")
print()
print(f"  {len(tiers.get(KERNEL_EXISTENTIAL, []))} kernel-existential, "
      f"{len(tiers.get(PUBLIC_BODY_ROUTE, []))} public-body-route, "
      f"{len(tiers.get(PROTECTIVE_SEGMENT, []))} protective-segment, "
      f"{len(tiers.get(TERMINAL_SUITE, []))} terminal-suite")
print()
for cls, (strength, driver, path, fns) in COVERAGE.items():
    print(f"  {cls:28s} [{strength}]")
    print(f"       drives: {driver}")
    for fn in fns:
        print(f"       {path}::{fn}")
print()
print("Remaining backstop (narrow): reaching the public-body-route fn through the")
print("FULL monolithic public-entrypoint interior (state-size wall). External actor")
print("submission is an explicit scheduler assumption, outside the engine.")
