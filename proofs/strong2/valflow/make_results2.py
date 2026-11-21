#!/usr/bin/env python3
"""RESULTS.md from run.json, lake_build.log and axioms.txt. A row is listed as kernel-checked only
if its module's `main` appears in axioms.txt with exactly [propext, Quot.sound] and the build
log ends in success."""
import json
import os
import re
import sys

out, genns = sys.argv[1], sys.argv[2]
r = json.load(open(os.path.join(out, "run.json")))
ax = open(os.path.join(out, "axioms.txt")).read().replace("\n ", " ")
build = open(os.path.join(out, "lake_build.log")).read().strip().split("\n")[-1]
good = set(re.findall(r"'ClauseFlow\.%s\.(\w+)\.main' depends on axioms: \[propext, Quot\.sound\]" % genns, ax))
okb = "Build completed successfully" in build


def st(m):
    return "kernel-checked" if (okb and m in good) else "NOT CHECKED"


print("# strong2 results (%s)\n" % genns)
print("Pin: %s\n" % r["pin"])
print("`lake build`: %s\n" % build)
print("## Rows\n")
print("| Row | Function | Result | Lean module | Status | Excluded Ok sites |")
print("|---|---|---|---|---|---|")
for row, info in sorted(r["targets"].items()):
    if "rows" not in info:
        print("| %s | `%s` | %s | | | |" % (row, info["fn"], info["status"]))
        continue
    for name, e in sorted(info["rows"].items()):
        ex = ", ".join("bb%s → v16.rs:%s" % kv for kv in sorted(e.get("exempt_lines", {}).items())) or "none"
        extra = ""
        if "ok_sites" in e:
            extra = " (%d of %d `Ok(..)` sites)" % (e["proved_ok_sites"], e["ok_sites"])
        m = e.get("module")
        print("| %s | `%s` | %s%s | `%s` | %s | %s |" % (name, info["fn"], e["status"], extra, m, st(m) if m else "open", ex))
print("\nFor each target: MIR sha256, blocks, and the callee summaries its abstract program uses.\n")
for row, info in sorted(r["targets"].items()):
    if "rows" not in info:
        continue
    print("- %s `%s`: MIR sha256 `%s`, %d blocks; uses: %s" % (
        row, info["fn"], info["mir_sha256"], info["blocks"],
        ", ".join("`%s`" % u.split("::")[-1] for u in info["uses"]) or "none"))
print("\n## Callee summaries used, each certified by its own module\n")
print("A summary is a list of clauses about the callee's return: `result = 0` is `Ok`; `payload` is the")
print("integer value or enum discriminant of the `Ok` payload (Option: None = 0, Some = 1).\n")
print("| Callee | Clauses | Module status | Uses |")
print("|---|---|---|---|")
for name, s in sorted(r["summaries"].items()):
    print("| `%s` | %s | %s | %s |" % (name, "<br>".join(s["clauses"]), st("S_" + name),
                                       ", ".join("`%s`" % u for u in s["uses"]) or "none"))
print("\nMethods analysed to find these: %d. Unsupported (no summary; calls to them are treated as"
      " able to write everything): %d." % (r["analysed_methods"], len(r["unsupported"])))
for k, v in sorted(r["unsupported"].items()):
    print("- `%s`: %s" % (k.split("::")[-1], v[:160]))
