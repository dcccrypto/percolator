#!/usr/bin/env python3
"""Attacks on the trusted reader of enum_probe.py, on hand-edited copies of a real probe build.
usage: enum_probe_unit.py BUILD_DIR ENUM   (BUILD_DIR has src/v16.rs with probes and out.mir;
ENUM is a two-variant enum of that build, e.g. ResolvedCloseOutcomeV16). Exit 1 if an attack is accepted."""
import os
import shutil
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import enum_probe as P

b, EN = sys.argv[1], sys.argv[2]
S, M = open(os.path.join(b, "src", "v16.rs")).read(), open(os.path.join(b, "out.mir")).read()


def run(edit_src=None, edit_mir=None, extra=None):
    d = tempfile.mkdtemp()
    os.makedirs(os.path.join(d, "src"))
    open(os.path.join(d, "src", "v16.rs"), "w").write(edit_src(S) if edit_src else S)
    open(os.path.join(d, "out.mir"), "w").write(edit_mir(M) if edit_mir else M)
    if extra:
        open(os.path.join(d, "src", "lib.rs"), "w").write(extra)
    t = P.table(os.path.join(d, "out.mir"), os.path.join(d, "src", "v16.rs"))
    shutil.rmtree(d)
    return t


def body(m, f):
    i = m.index("fn %s%s(" % (P.PREFIX, EN))
    j = m.index("\n}\n", i) + 3
    return m[:i] + f(m[i:j]) + m[j:]


base = run()
assert EN in base and len(base[EN]) == 2, "control: %s must be in the table" % EN
v = sorted(base[EN], key=lambda k: base[EN][k])
arm = lambda k: "%s::%s { .. } => %d" % (EN, v[k], P.BASE + 1 + k)
decl = "enum %s " % EN
cases = [
    ("prefix in another source file", dict(extra="pub fn %sZed(x: &Zed) -> u32 { 0 }\n" % P.PREFIX), lambda t: len(t) == 0),
    ("second marker line", dict(edit_src=lambda s: s.replace(decl, P.MARKER + "\n" + decl, 1)), lambda t: len(t) == 0),
    ("prefix before the marker", dict(edit_src=lambda s: s.replace(decl, "/* %s%s */ " % (P.PREFIX, EN) + decl, 1)), lambda t: len(t) == 0),
    ("duplicated probe body in the MIR", dict(edit_mir=lambda m: m + m[m.index("fn %s%s(" % (P.PREFIX, EN)):m.index("\n}\n", m.index("fn %s%s(" % (P.PREFIX, EN))) + 3]), lambda t: EN not in t),
    ("two arms return one constant", dict(edit_mir=lambda m: body(m, lambda x: x.replace("const %d_u32" % (P.BASE + 1), "const %d_u32" % (P.BASE + 2)))), lambda t: EN not in t),
    ("extra statement in an arm block", dict(edit_mir=lambda m: body(m, lambda x: x.replace("        _0 = const %d_u32;" % (P.BASE + 1), "        _0 = const %d_u32;\n        _0 = const %d_u32;" % (P.BASE + 2, P.BASE + 1), 1))), lambda t: EN not in t),
    ("argument type of another enum", dict(edit_mir=lambda m: m.replace("fn %s%s(_1: &%s)" % (P.PREFIX, EN, EN), "fn %s%s(_1: &V16Error)" % (P.PREFIX, EN))), lambda t: EN not in t),
    ("arm constants out of sequence in the probe text", dict(edit_src=lambda s: s.replace(arm(0), "@@").replace(arm(1), "%s::%s { .. } => %d" % (EN, v[1], P.BASE + 1)).replace("@@", "%s::%s { .. } => %d" % (EN, v[0], P.BASE + 2))), lambda t: EN not in t),
    ("discriminant read of another place", dict(edit_mir=lambda m: body(m, lambda x: x.replace("discriminant((*_1))", "discriminant(_1)"))), lambda t: EN not in t),
]
bad = 0
print("control: %s -> %s" % (EN, base[EN]))
for name, kw, good in cases:
    ok = good(run(**kw))
    bad += not ok
    print("- %s: %s" % (name, "rejected (ok)" if ok else "ACCEPTED (UNSOUND)"))
print("reader attacks accepted: %d of %d" % (bad, len(cases)))
sys.exit(1 if bad else 0)
