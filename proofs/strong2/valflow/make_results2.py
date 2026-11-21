#!/usr/bin/env python3
"""RESULTS.md from run.json, ledger_results.tsv, lake_build.log and axioms.txt. A module is listed
as kernel-checked only if its `main` appears in axioms.txt with exactly [propext, Quot.sound] and
the build log ends in success."""
import csv
import json
import os
import re
import sys

out, genns = sys.argv[1], sys.argv[2]
r = json.load(open(os.path.join(out, "run.json")))
ax = re.sub(r"\n\s+", " ", open(os.path.join(out, "axioms.txt")).read())
build = open(os.path.join(out, "lake_build.log")).read().strip().split("\n")[-1]
good = set(re.findall(r"'ClauseFlow\.%s\.(\w+)\.main' depends on axioms: \[propext, Quot\.sound\]" % genns, ax))
okb = "Build completed successfully" in build
led = list(csv.reader(open(os.path.join(out, "ledger_results.tsv")), delimiter="\t"))[1:]


def st(m):
    return "kernel-checked" if (okb and m in good) else "NOT CHECKED"


print("# strong2 results (%s)\n" % genns)
print("Pin: %s\n" % r["pin"])
print("`lake build`: %s. Modules generated: %d; with `main` on axioms [propext, Quot.sound]: %d.\n" % (
    build, len(r["modules"]), len([m for m in r["modules"] if m in good])))
hdr = [x for x in led if x[0].endswith(".header") and x[3] in ("est", "pres") and st(x[5]) == "kernel-checked"]
acc = [x for x in led if x[0].endswith(".account") and x[3] in ("est", "pres") and st(x[5]) == "kernel-checked"]
newly = [x for x in hdr + acc if x[4] not in ("est", "pres")]
print("## Ledger rows (statement: every `Ok` return; `est` = the check holds after, `pres` = it holds after if it held before)\n")
print("- `.header` rows kernel-checked: **%d** (%d est, %d pres)" % (len(hdr), sum(x[3] == "est" for x in hdr), sum(x[3] == "pres" for x in hdr)))
print("- `.account` rows kernel-checked: **%d** (%d est, %d pres)" % (len(acc), sum(x[3] == "est" for x in acc), sum(x[3] == "pres" for x in acc)))
print("- of these, rows the sibling run (`proofs/novel`) left open on the same text: **%s**" % (
    ", ".join(x[0] for x in newly) or "none"))
print("- rows the sibling run closed that this run does not: %s\n" % (
    ", ".join(x[0] for x in led if x[4] in ("est", "pres") and x[3] not in ("est", "pres")) or "none"))
print("| Row | Function | Class | This run | Sibling | Lean theorem | Status |")
print("|---|---|---|---|---|---|---|")
for x in led:
    if x[3] in ("est", "pres"):
        print("| %s | `%s` | %s | %s | %s | `%s.row` | %s |" % (x[0], x[1], x[2] or "-", x[3], x[4] or "-", x[5], st(x[5])))
print("\nOpen in this run (function is in the MIR): %s\n" % ", ".join(x[0] for x in led if x[3] == "open"))
t = r.get("t15") or {}
if t:
    print("## T15 `%s`\n" % t["fn"])
    print("MIR sha256 `%s`, %d blocks. `Ok(..)` assignments (block → source line): %s.\n" % (
        t["mir_sha256"], t["blocks"], ", ".join("bb%s → v16.rs:%s" % kv for kv in sorted(t["ok_sites"].items(), key=lambda z: int(z[0])))))
    sm = r["summaries"].get(t["fn"])
    if sm:
        print("- `S_%s` (%s), whole function, no exclusion, no assumption: %s" % (
            t["fn"], st("S_" + t["fn"]), "; ".join(sm["clauses"])))
    for name, e in sorted(t["rows"].items()):
        if "whole" in e:
            print("- %s: whole row, `%s` (%s)" % (name, e["whole"], st(e["whole"])))
            continue
        ex = ", ".join("bb%s → v16.rs:%s" % kv for kv in sorted(e["exempt_lines"].items()))
        m = e.get("checked_paths_module")
        print("- %s partial: %d of %d `Ok(..)` sites; excluded: %s. `%s.row` (%s)" % (
            name, e["proved_ok_sites"], e["n_ok_sites"], ex, m, st(m) if m else "open"))
        m = e.get("conditional_module")
        if m:
            print("- %s reduction: whole row IF these callees preserve the check on `Ok` (ASSUMED, not proved): %s. `%s.row` (%s)" % (
                name, ", ".join("`%s`" % q for q in e["assumed"]), m, st(m)))
        else:
            print("- %s reduction: not found. Assuming preservation for every direct market-method callee does not close the row "
                  "(a write the row depends on is not behind such a callee)." % name)
print("\n## Callee summaries used (each is the goal of its own module)\n")
print("`result = 0` is `Ok`; `payload` is the integer value or enum discriminant of the `Ok` payload "
      "(Option: None = 0, Some = 1; a plain crate enum: its discriminant as read from the probe MIR, `ENUMS.txt`).\n")
print("| Function | Clauses | Module status | Uses |")
print("|---|---|---|---|")
for name, s in sorted(r["summaries"].items()):
    print("| `%s` | %s | %s | %s |" % (name, "<br>".join(s["clauses"]), st("S_" + name),
                                       ", ".join("`%s`" % u for u in s["uses"]) or "none"))
print("\nMethods analysed: %d. Unsupported (no summary; calls to them are treated as able to write"
      " everything): %d. Plain crate enums in the R9 table: %d." % (r["analysed_methods"], len(r["unsupported"]), r["enums"]))
for k, v in sorted(r["unsupported"].items()):
    print("- `%s`: %s" % (k.split("::")[-1], v[:160]))
