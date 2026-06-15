#!/usr/bin/env python3
"""cover_vacuity_gate.py — build-failing gate against VACUOUS Kani proofs.

A `kani::proof` reports VERIFICATION:- SUCCESSFUL even when one of its
`kani::cover!` witnesses is UNSATISFIABLE/UNREACHABLE — i.e. the harness asserts
something on a branch the solver proved can never be taken, so the named
property is never actually exercised. The suite's runners bucket harnesses on
exit code (PASS/TIMEOUT/FAIL) only, so a dead cover slips through green. This
gate parses the per-harness Kani logs and FAILS when a cover is dead, turning
the cover witnesses into a machine-checked non-vacuity invariant.

It fails on either signal Kani emits:
  * the cover summary "** N of M cover properties satisfied" with N < M, or
  * any individual ".cover.<n>" check whose Status is UNSATISFIABLE/UNREACHABLE.

Usage:
  scripts/cover_vacuity_gate.py <logdir> [<logdir> ...]
    (recurses for *.log; each file is one harness's `cargo kani` output)

Exit code = number of vacuous harnesses (0 = clean), so it fails any CI step.
This is a log-only reader: no engine state, no network, no subprocess.
"""
import re
import sys
from pathlib import Path

SUMMARY = re.compile(r"\*\*\s*(\d+)\s+of\s+(\d+)\s+cover properties satisfied")
COVER_LINE = re.compile(r"\.cover\.\d+")
STATUS = re.compile(r"Status:\s*(UNSATISFIABLE|UNREACHABLE|SATISFIED)")
DESC = re.compile(r'Description:\s*"(.*?)"')
HARNESS = re.compile(r"Checking harness\s+(\S+?)\.\.\.")


def scan(path):
    text = path.read_text(errors="replace")
    m = HARNESS.search(text)
    harness = m.group(1) if m else path.stem
    dead = []

    for sm in SUMMARY.finditer(text):
        n, total = int(sm.group(1)), int(sm.group(2))
        if n < total:
            dead.append(f"summary: {n} of {total} cover properties satisfied "
                        f"({total - n} dead)")

    # pair each ".cover.<n>" line with the following Status line
    pending, desc = False, None
    for line in text.splitlines():
        if COVER_LINE.search(line):
            pending, desc = True, None
            continue
        if pending:
            dm = DESC.search(line)
            if dm:
                desc = dm.group(1)
            sm = STATUS.search(line)
            if sm:
                if sm.group(1) in ("UNSATISFIABLE", "UNREACHABLE"):
                    dead.append(f'{sm.group(1)} cover: "{desc or "?"}"')
                pending = False
    return harness, dead


def main(argv):
    if len(argv) < 2:
        print("usage: cover_vacuity_gate.py <logdir> [<logdir> ...]", file=sys.stderr)
        return 2
    logs = []
    for d in argv[1:]:
        p = Path(d)
        logs.extend(sorted(p.rglob("*.log")) if p.is_dir() else [p])
    if not logs:
        print("cover_vacuity_gate: no logs found", file=sys.stderr)
        return 2

    violations = []
    for log in logs:
        harness, dead = scan(log)
        for d in dead:
            violations.append((harness, d))

    if violations:
        print(f"cover_vacuity_gate: {len(violations)} VACUOUS cover(s) — "
              f"harness(es) certifying PASS on a solver-dead branch:\n")
        for harness, d in violations:
            print(f"  [{harness}] {d}")
        print("\n  -> the harness asserts on an unreachable branch; strengthen the "
              "fixture so every cover is SATISFIABLE, or remove the dead cover.")
        return len(violations)
    print(f"cover_vacuity_gate: OK — {len(logs)} harness log(s), every cover SATISFIABLE.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
