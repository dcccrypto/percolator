#!/usr/bin/env python3
"""Scans behind the extra assumptions of the strong2 rules (RULES.md, A10..A14) and a stricter
re-scan of the sibling's A6/A9 names (audit round 3 showed the per-line scans can be evaded by a
raw identifier `r#Result`, by a crate module named `core`, and in principle by a macro).

usage: check_assumptions2.py SRC_DIR MIR [EXPANDED_SOURCE]
EXPANDED_SOURCE: output of `cargo rustc -- -Zunpretty=expanded` (macros expanded); when given,
the name scans run on it as well. Text scans are evidence, not proof. Exits 1 if any fails."""
import os
import re
import sys

root, mir_path = sys.argv[1], sys.argv[2]
expanded = sys.argv[3] if len(sys.argv) > 3 else None
texts = {}
for dp, _, fs in os.walk(os.path.join(root, "src")):
    for f in fs:
        if f.endswith(".rs"):
            p = os.path.join(dp, f)
            texts[os.path.relpath(p, root)] = open(p).read()
if expanded:
    texts["<macro-expanded crate>"] = open(expanded).read()
ok = True


def strip_comments(t):
    t = re.sub(r"/\*.*?\*/", " ", t, flags=re.S)
    return "\n".join(l.split("//")[0] for l in t.split("\n"))


def scan(pat):
    """Matches over the whole text (so a line break inside an item header does not hide it)."""
    hits = []
    for f, t in sorted(texts.items()):
        c = strip_comments(t)
        for m in re.finditer(pat, c, flags=re.S):
            line = c.count("\n", 0, m.start()) + 1
            hits.append("%s:%d: %s" % (f, line, " ".join(m.group(0).split())[:100]))
    return hits


def report(name, hits):
    global ok
    ok = ok and not hits
    print("%s  %s%s" % ("PASS" if not hits else "FAIL", name, "" if not hits else " (%d hits)" % len(hits)))
    for h in hits[:12]:
        print("      " + h)


ITEM = r"\b(?:trait|struct|enum|union|type|fn|mod|const|static|macro_rules!)\s+(?:r#)?"
TRUSTED = r"(?:Try|FromResidual|Infallible|ControlFlow|Result|Ok|Err|Option|Some|None|is_some|is_none)\b"
report("A9/A12 (strict) no item, alias or import named Try, FromResidual, Infallible, ControlFlow, Result, Ok, Err, "
       "Option, Some, None, is_some, is_none; raw identifiers and line breaks included",
       scan(ITEM + TRUSTED + r"|\bas\s+(?:r#)?" + TRUSTED))
report("A12 no raw identifier of a trusted name anywhere (`r#Result`, `r#Option`, ...)",
       scan(r"\br#(?:Try|FromResidual|Infallible|ControlFlow|Result|Ok|Err|Option|Some|None|core|std|alloc)\b"))
report("A12 no module, alias or extern crate named core, std or alloc (a crate module `core` prints like the real one)",
       scan(r"\bmod\s+(?:r#)?(?:core|std|alloc)\b|\bas\s+(?:r#)?(?:core|std|alloc)\b|\bextern\s+crate\s+\w+\s+as\s+(?:core|std|alloc)\b"))
macros = scan(r"\bmacro_rules!\s*\w+|\bmacro\s+\w+\s*[({]")
if expanded:
    print("INFO  macro definitions in the source: %d; the name scans above also ran on the macro-expanded crate" % len(
        [h for h in macros if not h.startswith("<macro-expanded")]))
else:
    report("A12 the crate defines no macro (no expanded source was given, so a macro could hide an item)", macros)
bad, n_pred, n_read, ctor = [], 0, 0, []
with open(mir_path) as f:
    for i, l in enumerate(f, 1):
        c = l.split("//")[0]
        if re.search(r"\bOption::<.*>::is_(some|none)\(", c):
            n_pred += 1
        if re.search(r"\bv16::Option\b|\bcrate::Option\b|\bv16::Result\b|\bcrate::Result\b", c):
            bad.append("MIR:%d: %s" % (i, l.strip()[:100]))
        # a body in this crate's MIR whose path starts with core/std/alloc, or a constructor of a
        # trusted name, means the crate defines such an item
        if re.match(r"^fn (?:core|std|alloc)::|^fn (?:\w+::)*(?:Result|Option|ControlFlow)::(?:Ok|Err|Some|None|Continue|Break)\(", c):
            ctor.append("MIR:%d: %s" % (i, l.strip()[:100]))
        if re.search(r"as (Continue|Ok)\)\.0", c):
            n_read += 1
report("A12 `Option`/`Result` are never printed with a crate path in the MIR", bad)
report("A12 the MIR dump has no body under core::/std::/alloc:: and no constructor body of a trusted enum", ctor)
print("INFO  A10 (a variant's field is read only when the value is of that variant) is a property of "
      "rustc's MIR for safe code; it is not checked by a scan. Payload reads in this MIR: %d" % n_read)
print("INFO  A11 (core: Option::None = 0, Some = 1; is_some/is_none; Try::branch keeps the Ok payload) "
      "is read from core's source, not checked. is_some/is_none calls in this MIR: %d" % n_pred)
sys.exit(0 if ok else 1)
