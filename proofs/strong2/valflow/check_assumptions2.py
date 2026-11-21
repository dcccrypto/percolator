#!/usr/bin/env python3
"""Source and MIR scans behind the extra assumptions of the strong2 rules (RULES.md, A10..A12).
Text scans are evidence, not proof. Exits 1 if any fails."""
import os
import re
import sys

root, mir_path = sys.argv[1], sys.argv[2]
files = {}
for dp, _, fs in os.walk(os.path.join(root, "src")):
    for f in fs:
        if f.endswith(".rs"):
            p = os.path.join(dp, f)
            files[os.path.relpath(p, root)] = open(p).read().split("\n")
ok = True


def scan(pat):
    hits = []
    for f, lines in sorted(files.items()):
        for i, l in enumerate(lines, 1):
            c = l.split("//")[0]
            if re.search(pat, c):
                hits.append("%s:%d: %s" % (f, i, l.strip()[:100]))
    return hits


def report(name, hits):
    global ok
    ok = ok and not hits
    print("%s  %s%s" % ("PASS" if not hits else "FAIL", name, "" if not hits else " (%d hits)" % len(hits)))
    for h in hits[:12]:
        print("      " + h)


report("A12 the crate defines or imports-as no item named Option, Some, None, is_some, is_none",
       scan(r"\b(trait|struct|enum|union|type|fn|mod|const|static)\s+(Option|Some|None|is_some|is_none)\b"
            r"|\bas\s+(Option|Some|None)\b"))
# In the MIR of the analysed build: every `Option::<..>::is_some/is_none` call is printed without a
# trait qualification, and core's Option is never printed with a crate path.
bad, n_pred, n_read = [], 0, 0
with open(mir_path) as f:
    for i, l in enumerate(f, 1):
        c = l.split("//")[0]
        if re.search(r"\bOption::<.*>::is_(some|none)\(", c):
            n_pred += 1
        if re.search(r"\bv16::Option\b|\bcrate::Option\b", c):
            bad.append("MIR:%d: %s" % (i, l.strip()[:100]))
        if re.search(r"as (Continue|Ok)\)\.0", c):
            n_read += 1
report("A12 `Option` is never printed with a crate path in the MIR", bad)
print("INFO  A10 (a variant's field is read only when the value is of that variant) is a property of "
      "rustc's MIR for safe code; it is not checked by a scan. Payload reads in this MIR: %d" % n_read)
print("INFO  A11 (core: Option::None = 0, Some = 1; is_some/is_none; Try::branch keeps the Ok payload) "
      "is read from core's source, not checked. is_some/is_none calls in this MIR: %d" % n_pred)
sys.exit(0 if ok else 1)
