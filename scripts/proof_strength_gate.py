#!/usr/bin/env python3
"""proof_strength_gate.py — fail the build if any Kani harness certifies PASS
while one of its kani::cover! witnesses is UNSATISFIABLE / UNREACHABLE.

WHY THIS EXISTS. The suite carries ~448 hand-placed `kani::cover!` witnesses
whose whole purpose is to prove a harness is NON-VACUOUS (the asserted branch is
actually reachable). But the runners (scripts/isolated_runner.sh:25-45) gate on
PASS / TIMEOUT / FAIL only. A harness whose cover is DEAD still reports
`VERIFICATION:- SUCCESSFUL` — so the cover witnesses are documentation a human
must read, never a machine invariant. That gap is exactly what let
`proof_v16_expired_backing_yields_zero_realizable_support_after_expiry` certify
green while reporting "1 of 2 cover properties satisfied": its
`kani::cover!(backing == claim, ...)` was UNSATISFIABLE (backing welded to 2,
claim to 3), so the fully-backed branch was never verified, yet the suite
counted it as a passing proof.

This gate closes that hole. It is build-failing, in the same executable-
invariant mold as scripts/boundary_audit.py and lof_transition_class_roster.py.

CHECK 1 — COVER NON-VACUITY (dynamic, from Kani logs).
  For every harness log, FAIL if either:
    * the cover summary "** N of M cover properties satisfied" has N < M, or
    * any individual ".cover.<n>" check reports Status UNSATISFIABLE / UNREACHABLE.
  A dead cover means the harness asserts something on a branch the solver proved
  unreachable: the proof is vacuous on that branch.

CHECK 2 — GUARDED-ASSERT WITNESS (static, from source).
  Flag every harness whose body guards its assertions behind `if let Ok(..)`,
  `.is_ok()`, or `if <result> ==`/`!= Ok/Err` *and* carries no `kani::cover!`
  proving the guarded branch is reachable. Such a harness passes trivially if
  the guard is never taken.

USAGE
  # Parse the per-harness logs an isolated run already produced:
  scripts/proof_strength_gate.py --logs <dir>
  # Static guarded-assert scan over the harness sources:
  scripts/proof_strength_gate.py --source tests/proofs_v16.rs src/v16_proofs.rs
  # Both (CI default):
  scripts/proof_strength_gate.py --logs kani_logs --source tests/proofs_v16.rs

Exit code is the number of violations (0 = clean), so it fails any CI step.
"""
import argparse
import glob
import os
import re
import sys

SUMMARY_RE = re.compile(r"\*\*\s*(\d+)\s+of\s+(\d+)\s+cover properties satisfied")
COVER_CHECK_RE = re.compile(r"\.cover\.\d+")
STATUS_RE = re.compile(r"Status:\s*(UNSATISFIABLE|UNREACHABLE|SATISFIED)")
HARNESS_RE = re.compile(r"Checking harness\s+(\S+?)\.\.\.")
VERDICT_RE = re.compile(r"VERIFICATION:-\s*(SUCCESSFUL|FAILED)")


def scan_log(path):
    """Return list of (harness, kind, detail) violations found in one Kani log."""
    text = open(path, errors="replace").read()
    harness = None
    m = HARNESS_RE.search(text)
    if m:
        harness = m.group(1)
    harness = harness or os.path.basename(path)
    violations = []

    # (a) summary N of M
    for sm in SUMMARY_RE.finditer(text):
        n, total = int(sm.group(1)), int(sm.group(2))
        if n < total:
            violations.append(
                (harness, "DEAD_COVER_SUMMARY",
                 f"{n} of {total} cover properties satisfied — {total - n} dead cover(s)"))

    # (b) individual cover checks with a dead status. Walk lines and pair a
    # ".cover.<n>" line with the next Status line.
    lines = text.splitlines()
    pending_cover = None
    pending_desc = None
    for ln in lines:
        if COVER_CHECK_RE.search(ln):
            pending_cover = ln.strip()
            pending_desc = None
            continue
        if pending_cover is not None:
            dm = re.search(r'Description:\s*"(.*)"', ln)
            if dm:
                pending_desc = dm.group(1)
            sm = STATUS_RE.search(ln)
            if sm:
                status = sm.group(1)
                if status in ("UNSATISFIABLE", "UNREACHABLE"):
                    violations.append(
                        (harness, "DEAD_COVER",
                         f'{status} cover: "{pending_desc or pending_cover}"'))
                pending_cover = None
    return violations


def scan_logs(log_dir):
    out = []
    paths = sorted(glob.glob(os.path.join(log_dir, "**", "*.log"), recursive=True)) \
        or sorted(glob.glob(os.path.join(log_dir, "*")))
    for p in paths:
        if os.path.isfile(p):
            out.extend(scan_log(p))
    return out


# ---- Static guarded-assert-without-cover scan ----
HARNESS_DEF = re.compile(r"^\s*fn\s+(proof_\w+|closure_\w+|contract_\w+)\s*\(")
GUARD_PAT = re.compile(r"if\s+let\s+Ok|\.is_ok\(\)|\bif\b.*\bresult\b.*==\s*Ok|if\s+result\.is_")
COVER_PAT = re.compile(r"kani::cover!")
ASSERT_PAT = re.compile(r"\bassert!|\bassert_eq!")


def scan_source(paths):
    violations = []
    for path in paths:
        if not os.path.isfile(path):
            continue
        lines = open(path, errors="replace").read().splitlines()
        i = 0
        n = len(lines)
        while i < n:
            m = HARNESS_DEF.match(lines[i])
            if not m:
                i += 1
                continue
            name = m.group(1)
            # collect the body until the next harness or attribute boundary
            depth = 0
            body = []
            j = i
            started = False
            while j < n:
                body.append(lines[j])
                depth += lines[j].count("{") - lines[j].count("}")
                if "{" in lines[j]:
                    started = True
                if started and depth <= 0:
                    break
                j += 1
            blob = "\n".join(body)
            if GUARD_PAT.search(blob) and ASSERT_PAT.search(blob) and not COVER_PAT.search(blob):
                violations.append(
                    (name, "GUARDED_ASSERT_NO_COVER",
                     "guards assertions behind Ok/is_ok with no kani::cover! "
                     "witnessing the guarded branch is reachable"))
            i = j + 1
    return violations


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--logs", help="directory of per-harness Kani logs")
    ap.add_argument("--source", nargs="*", default=[],
                    help="harness source files for the static guarded-assert scan")
    ap.add_argument("--strict", action="store_true",
                    help="also fail the build on WARNINGs (missing cover witnesses)")
    args = ap.parse_args()

    violations = []
    if args.logs:
        violations += scan_logs(args.logs)
    for v in scan_source(args.source):
        violations.append(v)

    if not args.logs and not args.source:
        ap.error("nothing to check: pass --logs and/or --source")

    # Severity split. ERRORs are CONFIRMED vacuity (a dead cover proven
    # UNSATISFIABLE/UNREACHABLE by the solver) and FAIL the build. WARNs are
    # LATENT risk (a guarded assertion with no machine witness that its branch
    # is reachable) — the proof may be fine today by construction, but nothing
    # enforces it stays that way; these are advisory unless --strict.
    errors = [v for v in violations if v[1] in ("DEAD_COVER", "DEAD_COVER_SUMMARY")]
    warns = [v for v in violations if v[1] == "GUARDED_ASSERT_NO_COVER"]

    if not errors and not warns:
        print("proof_strength_gate: OK — no dead covers, no unwitnessed guarded asserts.")
        return 0

    if errors:
        print(f"proof_strength_gate: {len(errors)} ERROR(S) — CONFIRMED vacuous "
              f"proof(s) certifying PASS with a solver-proven-dead cover:\n")
        for harness, kind, detail in errors:
            print(f"  [{kind}] {harness}\n      {detail}")
        print("  -> the harness asserts on a branch the solver proved unreachable. "
              "Strengthen the fixture so the cover is SATISFIABLE, or remove it.\n")

    if warns:
        # De-duplicate by harness for the advisory list.
        seen = []
        for h, k, d in warns:
            if h not in seen:
                seen.append(h)
        print(f"proof_strength_gate: {len(seen)} WARNING(S) — guarded assertions with "
              f"no kani::cover! reachability witness (latent vacuity risk; not "
              f"confirmed vacuous):\n")
        for h in seen:
            print(f"  [GUARDED_ASSERT_NO_COVER] {h}")
        print("  -> add `kani::cover!(result.is_ok())` (or equivalent) so the Ok "
              "branch's reachability becomes a machine-checked invariant, not an "
              "assumption that holds by construction today.\n")

    # Build-failing on confirmed errors; warnings fail only under --strict.
    return len(errors) + (len(warns) if args.strict else 0)


if __name__ == "__main__":
    sys.exit(main())
