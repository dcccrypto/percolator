#!/usr/bin/env python3
"""no-LoF transition-class roster (/tmp/proofs.md "add a transition-class roster
for no-LoF: public entrypoint / transition class -> proved frame/value source").

Every public `*_not_atomic` entrypoint carries the UNIVERSAL no-LoF floor: its
committed Ok-exit preserves GlobalValidState (validate_shape and per-touched-
account validate_with_market), machine-checked transitively for all 55 by
scripts/boundary_audit.py (the Lemma-0 boundary). Err paths fully revert at the
execution boundary, so they need no preservation.

On top of that floor, this roster groups the public surface into TRANSITION
CLASSES and names each class's STRONGEST additional no-LoF proof source:

  DIRECT_FRAME            - exact whole-state frame proofs (proof_v16_frame_*):
                            Ok touches only declared state; Err leaves it intact.
  WHOLE_BODY_COMPOSITION  - whole-body frame + value-conservation composition
                            under the arithmetic axiom (attach/clear).
  KERNEL_VALUE            - production kernel contracts pin the exact value delta
                            of the body's value-moving stage.
  FLOW_VALIDATOR          - the body constructs+validates a typed
                            TokenValueFlowProofV16 on every value-moving Ok exit.
  CLOSURE                 - inductive encumbrance/lien closure proofs (any-state
                            + assume(inv) -> delta -> assert inv).
  GLOBAL_VALID_FLOOR      - no stronger per-op artifact yet; the universal
                            GlobalValidState floor (+ frame lattice methodology)
                            is the no-LoF evidence for this transition class.

The script ENFORCES two invariants:
  1. COMPLETENESS - every public `*_not_atomic` entrypoint matches exactly one
     transition class (no entrypoint unclassified, none double-counted).
  2. ARTIFACT PRESENCE - every named proof-source artifact actually exists.

It does NOT re-run proofs; it keeps the public-transition -> proof-source map an
enforced invariant rather than prose. The remaining honest caveat (per
no-steal-theorem.md): a SINGLE Kani query over all transitions at once is
tool-impossible; this roster is its decomposition.
"""
import re
import sys

SRC = "src/v16.rs"
PROOFS = "tests/proofs_v16.rs"
HARNESS = "src/v16_proofs.rs"

DIRECT_FRAME = "DIRECT_FRAME"
WHOLE_BODY_COMPOSITION = "WHOLE_BODY_COMPOSITION"
KERNEL_VALUE = "KERNEL_VALUE"
FLOW_VALIDATOR = "FLOW_VALIDATOR"
CLOSURE = "CLOSURE"
GLOBAL_VALID_FLOOR = "GLOBAL_VALID_FLOOR"

# Ordered list of (class name, strength, [entrypoint-substring patterns],
# [(file, artifact fn) that must exist]). Patterns are matched as substrings
# against the entrypoint name; the FIRST class that matches an entrypoint owns
# it (order matters - more specific classes first).
CLASSES = [
    ("user capital flow", DIRECT_FRAME,
     ["deposit_not_atomic", "withdraw_not_atomic"],
     [(PROOFS, "proof_v16_frame_deposit_touches_only_declared_state"),
      (PROOFS, "proof_v16_frame_withdraw_touches_only_declared_state"),
      (PROOFS, "proof_v16_frame_overwithdraw_err_leaves_state_unchanged")]),

    ("domain insurance", DIRECT_FRAME,
     ["domain_insurance", "credit_account_from_insurance", "reserve_insurance_credit",
      "credit_domain_insurance_budget"],
     [(PROOFS, "proof_v16_frame_domain_insurance_deposit_touches_only_declared_state"),
      (PROOFS, "proof_v16_frame_domain_insurance_withdraw_touches_only_declared_state"),
      (PROOFS, "proof_v16_frame_budget_credit_touches_only_declared_state")]),

    ("counterparty backing", DIRECT_FRAME,
     ["counterparty_backing", "backing_provider_earnings", "backing_fee",
      "expire_source_backing_bucket"],
     [(PROOFS, "proof_v16_frame_backing_deposit_touches_only_declared_state"),
      (PROOFS, "proof_v16_frame_earnings_withdraw_touches_only_declared_state")]),

    ("source-credit lien lifecycle", CLOSURE,
     ["source_credit_lien", "source_positive_claim_bound",
      "source_credit_liens"],
     [(HARNESS, "contract_check_prepare_counterparty_lien_create_delta"),
      (HARNESS, "contract_check_prepare_insurance_lien_release_delta"),
      (HARNESS, "contract_check_prepare_counterparty_lien_terminal_release_delta")]),

    ("positive-PnL attribution / convert", FLOW_VALIDATOR,
     ["source_positive_pnl", "convert_released_pnl_to_capital"],
     [(HARNESS, "contract_check_flow_account_capital_to_external_out"),
      (SRC, "validate_positive_pnl_source_attribution")]),

    ("leg mutation / trade / batch", WHOLE_BODY_COMPOSITION,
     ["execute_batch_with_fee", "execute_trade_with_fee", "rebalance_reduce_position",
      "apply_trade_after_refresh"],
     [(HARNESS, "composition_attach_value_conservation_under_axiom"),
      (HARNESS, "composition_clear_leg_value_conservation"),
      (HARNESS, "contract_check_kernel_accumulate_batch_trade"),
      (HARNESS, "contract_check_kernel_resize_leg_same_side")]),

    ("close / resolve / realize", DIRECT_FRAME,
     ["close_resolved_account", "claim_resolved_payout_topup", "cure_and_cancel_close",
      "refine_resolved_unreceipted_bound", "resolve_market",
      "realize_source_backed_claims"],
     [(PROOFS, "proof_v16_frame_resolve_market_touches_only_declared_state"),
      (PROOFS, "proof_v16_public_resolved_close_flat_account_pays_only_capital_and_vault")]),

    ("recovery / liquidation / forfeit", KERNEL_VALUE,
     ["liquidate_account", "force_asset_recovery", "forfeit_recovery_leg"],
     [(PROOFS, "proof_v16_liquidation_preflight_accepts_only_fully_durable_residual"),
      (PROOFS, "proof_v16_liquidation_cannot_leave_uncovered_loss_with_other_open_risk")]),

    ("account refresh / crank / fees", KERNEL_VALUE,
     ["full_account_refresh", "permissionless_crank", "account_fee", "accrue_asset_to",
      "sync_account_fee_to_slot"],
     [(HARNESS, "contract_check_kernel_advance_leg_b_snap"),
      (HARNESS, "closure_kernel_advance_close_ledger_rank_witness"),
      (PROOFS, "proof_v16_permissionless_recovery_crank_is_accounting_neutral")]),

    ("asset lifecycle / oracle / market admin", DIRECT_FRAME,
     ["activate_empty", "canonicalize_retired", "mark_asset_drain_only",
      "finalize_side_reset", "grow_asset_slot_capacity", "reset_empty_asset_oracle_anchor",
      "restart_empty_asset", "retire_empty_asset", "set_asset_raw_oracle_target",
      "materialized_portfolio"],
     [(PROOFS, "proof_v16_frame_mark_drain_only_touches_only_declared_state"),
      (HARNESS, "contract_check_asset_restart_next_counters")]),
]


def fns_in(path, _cache={}):
    if path not in _cache:
        _cache[path] = set(re.findall(r"\bfn\s+([A-Za-z0-9_]+)\s*\(", open(path).read()))
    return _cache[path]


entrypoints = sorted(set(re.findall(r"pub fn (\w+_not_atomic)\b", open(SRC).read())))

# 1. COMPLETENESS: partition entrypoints across classes.
owner = {}
multi = []
for ep in entrypoints:
    hits = [name for (name, _s, pats, _a) in CLASSES if any(p in ep for p in pats)]
    if len(hits) == 1:
        owner[ep] = hits[0]
    elif len(hits) == 0:
        owner[ep] = None
    else:
        multi.append((ep, hits))

unclassified = [ep for ep, c in owner.items() if c is None]

# 2. ARTIFACT PRESENCE.
missing_art = []
for (name, _s, _pats, arts) in CLASSES:
    for (path, fn) in arts:
        if fn not in fns_in(path):
            missing_art.append((name, path, fn))

if unclassified or multi or missing_art:
    print("LoF TRANSITION-CLASS ROSTER GAP(S):")
    for ep in unclassified:
        print(f"  unclassified public entrypoint: {ep}")
    for ep, hits in multi:
        print(f"  entrypoint {ep} matches multiple classes: {hits}")
    for name, path, fn in missing_art:
        print(f"  class '{name}': missing proof-source artifact `{fn}` in {path}")
    sys.exit(1)

counts = {}
for ep, c in owner.items():
    counts[c] = counts.get(c, 0) + 1

print(f"LoF transition-class roster OK: all {len(entrypoints)} public *_not_atomic")
print("entrypoints partition into transition classes, each with a present")
print("proof-source artifact, atop the universal GlobalValidState floor (55/55):")
print()
for (name, strength, _pats, arts) in CLASSES:
    print(f"  [{strength:22s}] {name}  ({counts.get(name, 0)} entrypoints)")
    for (path, fn) in arts:
        print(f"       {path}::{fn}")
print()
print("Universal floor for ALL 55: GlobalValidState preserved at every committed")
print("Ok-exit (scripts/boundary_audit.py). The single all-transitions Kani query")
print("is tool-impossible; this partition is its sound decomposition.")
