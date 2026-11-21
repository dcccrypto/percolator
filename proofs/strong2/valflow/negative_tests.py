#!/usr/bin/env python3
"""Hole-hunt for the strong2 translator. Attack functions (neg_cases/cases2.rs, and the sibling's
proofs/novel/mirflow/audit_cases/cases.rs) are inserted into a scratch copy of the pinned source,
MIR is rebuilt, and the tool is asked a question whose sound answer is known. A sound tool refuses
(or rejects as unsupported) every case not marked True.

usage: negative_tests.py SRC_DIR WORK_DIR NIGHTLY > table.md     (exit 1 if any unsound answer)
"""
import os
import shutil
import subprocess
import sys

src_dir, work, nightly = sys.argv[1], sys.argv[2], sys.argv[3]
here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
import run2 as R
import extract2 as E
X, B = E.X, R.B

ANCHOR = """impl<'a, T> MarketGroupV16ViewMut<'a, T> {
    pub fn validate_shape(&self) -> V16Result<()> {
        self.as_view().validate_shape()
    }
"""
# name[#tag] -> (kind, check, certificate is the correct answer, what it does)
NEW = {
    "s2_n1": ("pres", 0, True, "R3/R2/R8: callee returns Ok(0) only before writing; caller validates iff c != 0 (the T18 shape)"),
    "s2_n1#est": ("est", 0, False, "same, asking that the check HOLDS (false: Ok(0) path never checks)"),
    "s2_n2": ("pres", 0, False, "callee writes on every path"),
    "s2_n3": ("pres", 0, False, "callee's no-write value is 1, caller tests 0"),
    "s2_n3ok": ("pres", 0, True, "callee's no-write value is 1, caller tests 1 (harmless)"),
    "s2_n4": ("pres", 0, False, "inverted test: validates when c == 0"),
    "s2_n5": ("pres", 0, False, "R1: result local reassigned by a writing callee before `?`"),
    "s2_n6": ("pres", 0, False, "value changed after the payload read"),
    "s2_n7": ("pres", 0, False, "test is on a different variable"),
    "s2_n8": ("pres", 0, False, "write between the call and the test"),
    "s2_n9": ("est", 0, False, "R4: callee writes before Ok(Some) and validates before Ok(None); caller returns on Some"),
    "s2_n10": ("pres", 0, True, "R4: callee validates before Ok(Some), no write before Ok(None) (the T15 :26594 shape)"),
    "s2_n10#est": ("est", 0, False, "same, asking that the check HOLDS (false on the None path)"),
    "s2_n11": ("pres", 0, False, "R7: is_some() path returns without a check after a write"),
    "s2_n12": ("pres", 0, True, "R7: validates iff is_some() (harmless)"),
    "s2_n12n": ("pres", 0, False, "R7: validates iff is_none() (inverted)"),
    "s2_n13": ("pres", 0, False, "R6: copy taken, source reassigned, test on the new value"),
    "s2_n14": ("pres", 0, False, "match on the Result of a writing callee, Ok(0) arm unchecked"),
    "s2_n14ok": ("pres", 0, True, "match on the Result of the no-write-on-0 callee (harmless)"),
    "s2_n16": ("pres", 0, False, "R8: tail-call wrapper of a writing callee"),
    "s2_n16ok": ("pres", 0, True, "R8: tail-call wrapper of the no-write-on-0 callee (harmless)"),
    "s2_n17": ("pres", 0, False, "R3: callee returns the constant Ok(0) after writing"),
    "s2_n18": ("pres", 0, False, "R3: callee's payload local is modified before the return"),
    "s2_n19": ("pres", 0, False, "R3: callee returns a truncating cast of a nonzero value"),
    "s2_n20": ("pres", 0, False, "R3: callee returns x - 1"),
    "s2_n21": ("pres", 0, False, "bool payload: Ok(false) also after a write"),
    "s2_n22": ("pres", 0, False, "R1: result assigned in a loop, last by a writing callee"),
    "s2_n23": ("pres", 0, False, "result overwritten through `&mut r` (escaped local)"),
    "s2_n24": ("pres", 0, False, "R4: Option local reassigned to None after a write"),
    "s2_n25": ("pres", 0, False, "two calls; the test is on the first result, the second wrote"),
    "s2_n26": ("pres", 0, False, "test `c > 1` (c == 1 path wrote, unchecked)"),
    "s2_n27": ("pres", 0, True, "match Ok(v) => v, Err(_) => 0 (harmless: Err of this callee is impossible after a write... refusal is only imprecise)"),
}
NEW.update({
    "s3_f1": ("pres", 0, True, "R10: writes header field materialized_portfolio_count, which validate_shape never reads (the T29/T47 shape)"),
    "s3_f2": ("pres", 0, False, "R10: writes c_tot (read)"),
    "s3_f3": ("pres", 0, False, "R10: helper given `&mut header.vault` (read)"),
    "s3_f4": ("pres", 0, True, "R10: helper given `&mut header.materialized_portfolio_count` (not read)"),
    "s3_f5": ("pres", 0, False, "R10: helper given the whole header, writes c_tot"),
    "s3_f6": ("pres", 0, False, "R10: pointer is one of two fields, one of them read"),
    "s3_f7": ("pres", 0, True, "R10: overwrites a market slot; validate_shape (this build) reads only the slice length"),
    "s3_f7a": ("pres", 1, False, "R10: overwrites a market slot; the ACCOUNT check may read slots"),
    "s3_f9": ("pres", 0, True, "R10: writes header.mode, not read by validate_shape"),
    "s3_f9#acct": ("pres", 1, False, "R10: writes header.mode, which the account check reads"),
    "s3_f10": ("pres", 0, False, "R10: overwrites header.config (read)"),
    "s3_f11": ("pres", 0, False, "R10: unread field through a sub-borrow, then c_tot through the parent"),
    "s3_f12": ("pres", 0, False, "R10: bytes_of_mut(header)[40] = 0xff"),
    "s3_f13": ("pres", 0, False, "R10: mem::swap of an unread field with a read one"),
    "s3_f14": ("pres", 0, True, "R10: mem::swap of two unread fields"),
    "s3_f15": ("pres", 0, False, "R10: callee writes c_tot"),
    "s3_f15ok": ("pres", 0, True, "R10: callee writes only an unread field"),
    "s3_f16": ("pres", 0, False, "R10: pointer chosen between an unread and a read field of the same type"),
    "s3_f17": ("pres", 0, False, "R10: array of field pointers, one read"),
    "s3_f18": ("pres", 0, False, "R10: nested field of config (top-level field 1, read)"),
    "s3_f19": ("pres", 0, False, "R10: slice reborrowed, then vault written"),
})
A3 = {}
for ln in open(os.path.join(here, "neg_cases", "audit3_cases.txt")):
    ln = ln.rstrip("\n")
    if not ln.strip():
        continue
    nm, kind, k, exp, desc = ln.split(" ", 4)
    key = nm if nm not in A3 else nm + "#" + kind + k
    A3[key] = (kind, int(k), exp == "T", "[audit 3] " + desc)
A4 = {}
for ln in open(os.path.join(here, "neg_cases", "audit4_cases.txt")):
    ln = ln.rstrip("\n")
    if not ln.strip():
        continue
    nm, kind, k, exp, desc = ln.split(" ", 4)
    key = nm if nm not in A4 else nm + "#" + kind + k
    A4[key] = (kind, int(k), exp == "T", "[audit 4] " + desc)
A3.update(A4)
sys.path.insert(0, os.path.join(here, "..", "..", "novel", "mirflow"))
OLD = {}
src_old = open(os.path.join(here, "..", "..", "novel", "mirflow", "audit_regression.py")).read()
exec(src_old[src_old.index("CASES = {"):src_old.index("d = os.path.join(work")], {}, OLD)
OLD = OLD["CASES"]

d = os.path.join(work, "negative-tests")
if os.path.isdir(d):
    shutil.rmtree(d)
os.makedirs(os.path.join(d, "src"))
for f in ("Cargo.toml", "Cargo.lock"):
    shutil.copy(os.path.join(src_dir, f), d)
for f in os.listdir(os.path.join(src_dir, "src")):
    shutil.copy(os.path.join(src_dir, "src", f), os.path.join(d, "src"))
text = open(os.path.join(d, "src", "v16.rs")).read()
assert text.count(ANCHOR) == 1
extra = open(os.path.join(here, "..", "..", "novel", "mirflow", "audit_cases", "cases.rs")).read() + \
    open(os.path.join(here, "neg_cases", "cases2.rs")).read() + \
    open(os.path.join(here, "neg_cases", "cases3.rs")).read() + \
    open(os.path.join(here, "neg_cases", "audit3_cases.rs")).read() + \
    open(os.path.join(here, "neg_cases", "audit4_cases.rs")).read()
tail = open(os.path.join(here, "neg_cases", "audit3_extra.rs")).read() + \
    open(os.path.join(here, "neg_cases", "audit4_extra.rs")).read()
# cases that do not compile against this revision's types are dropped by name (env SKIP=a,b)
SKIP = [x for x in os.environ.get("SKIP", "").split(",") if x]
for nm in SKIP:
    i = extra.index("    pub fn %s(" % nm) if ("    pub fn %s(" % nm) in extra else extra.index("    pub fn %s<" % nm)
    j = extra.index("\n    }\n", i) + len("\n    }\n")
    extra = extra[:i] + extra[j:]
    for k in [k for k in list(NEW) + list(OLD) if k.split("#")[0] == nm]:
        NEW.pop(k, None)
        OLD.pop(k, None)
import enum_probe
full = text.replace(ANCHOR, ANCHOR + extra) + "\n" + tail
# the R9 probes go into the same scratch copy, so the table is read from this very build
open(os.path.join(d, "src", "v16.rs"), "w").write(full + enum_probe.probe_text(full)[0])
mir = os.path.join(d, "out.mir")
r = subprocess.run(["cargo", "+" + nightly, "rustc", "-j", "4", "--release", "--lib", "--",
                    "-Zunpretty=mir", "-Zinline-mir=no"], cwd=d, stdout=open(mir, "w"),
                   stderr=subprocess.PIPE, text=True)
if r.returncode != 0:
    print("the cases do not compile:\n" + r.stderr[-3000:])
    sys.exit(2)
fns, unsupported = B.load(mir, os.path.join(d, "src", "v16.rs"))
E.ENUMS.clear()
E.ENUMS.update(E.enum_table(mir, os.path.join(d, "src", "v16.rs")))
rs = R.RS.compute(mir, os.path.join(d, "src", "v16.rs"))
E.READ = {"fields": {0: rs["shape"][0], 1: rs["account"][0]}, "mk": {0: rs["shape"][1], 1: rs["account"][1]}}
prov = R.Provider(fns)
prov.unsupported.update(unsupported)


def ask(name, kind, k):
    path = R.P + name
    if path in prov.unsupported or path not in fns:
        return "refused (rejected as unsupported): " + prov.unsupported.get(path, "not found")[:60].replace("|", "/")
    try:
        res = prov.analyse(path)
    except X.Unsupported as e:
        return "refused (rejected as unsupported): " + str(e)[:60].replace("|", "/")
    tr, ir, inv = res["tr"], res["ir"], res["inv"]
    if k not in tr.checks:
        return "refused (check not defined for this signature)"
    g0 = ((0, "eq", 0),) if res["ok_only"] else ()
    cl = (g0, ((k,), ())) if kind == "est" else (g0, ((), (k,)))
    if E.ret_entailed(ir, inv, cl) and not E.verify(ir, inv, [cl], tr.checks):
        return "certified"
    return "refused"


print("| Case | What it does | Asked | Sound answer | Tool | Verdict |")
print("|---|---|---|---|---|---|")
unsound = imprecise = 0
allc = [(k, v[0], v[1], v[2], v[3]) for k, v in list(NEW.items()) + list(A3.items())]
for key, (req, reqp, ok_expected, desc) in OLD.items():
    allc.append((key, "est" if req else "pres", int(req or reqp), ok_expected, "[sibling audit] " + desc))
for key, kind, k, ok_expected, desc in allc:
    name = key.split("#")[0]
    got = ask(name, kind, k)
    verdict = "ok"
    if got == "certified" and not ok_expected:
        verdict = "UNSOUND"
        unsound += 1
    elif got != "certified" and ok_expected:
        verdict = "imprecise"
        imprecise += 1
    print("| %s | %s | %s %d | %s | %s | %s |" % (key, desc, kind, k, "certified" if ok_expected else "refused", got, verdict))
print("\nunsound answers: %d of %d cases; imprecise (sound refusals of a true statement): %d" % (unsound, len(allc), imprecise))

# ---- unit tests that need no Rust build -----------------------------------------------------
print("\n## Unit tests\n")


class _F:  # a minimal stand-in for X.Fn
    def __init__(self, types):
        self.types, self.args, self.blocks = types, [], {}


def unit_assign(types, l, rhs):
    t = E.T2.__new__(E.T2)
    t.fn, t.escaped, t.partial, t.assigns, t.cur = _F(types), set(), set(), {}, (0, 0)
    t.fn.blocks = {0: ([], "return;", False)}
    out = []
    t.assign_local(l, rhs, out)
    return out


u1 = unit_assign({3: "u64", 4: "core::result::Result<u128, v16::V16Error>"}, 3, "copy ((_4 as Ok).0: u128) as u64 (IntToInt)")
ok1 = u1 == [("havoc", 3)]
print("- audit 3 risk R-a, `_3 = copy ((_4 as Ok).0: u128) as u64 (IntToInt)` must not be a payload read: %s -> %s" % (u1, "ok" if ok1 else "UNSOUND"))
u2 = unit_assign({6: "Result<u128, u8>"}, 6, "Result::<u128, u8>::Ok(copy _2)")
ok2 = u2 == [("havoc", 6)]
print("- audit 3 hole H1, `Result::<..>::Ok(..)` into a local not declared `core::result::Result<..>` must pin nothing: %s -> %s" % (u2, "ok" if ok2 else "UNSOUND"))
unsound += (not ok1) + (not ok2)
# audit 3 holes H1/H2: the assumption scan must reject a crate with the homonym items
for nm in ("homonym_h1_lib.rs", "homonym_h2_lib.rs"):
    hd = os.path.join(work, "negative-homonym")
    if os.path.isdir(hd):
        shutil.rmtree(hd)
    os.makedirs(os.path.join(hd, "src"))
    for f in os.listdir(os.path.join(src_dir, "src")):
        shutil.copy(os.path.join(src_dir, "src", f), os.path.join(hd, "src"))
    open(os.path.join(hd, "src", "lib.rs"), "a").write("\n" + open(os.path.join(here, "neg_cases", nm)).read())
    p = subprocess.run([sys.executable, os.path.join(here, "check_assumptions2.py"), hd, mir],
                       stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    good = p.returncode == 1
    print("- audit 3 %s appended to src/lib.rs: check_assumptions2.py exit %d -> %s" % (
        nm, p.returncode, "ok (rejected)" if good else "UNSOUND (accepted)"))
    unsound += not good
    shutil.rmtree(hd)
print("\nunsound in total: %d" % unsound)
shutil.rmtree(d)
sys.exit(1 if unsound else 0)
