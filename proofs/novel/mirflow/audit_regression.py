#!/usr/bin/env python3
"""Regression tests from the independent audit: functions built to make the extractor certify
something false. They are inserted into a scratch copy of the pinned source, MIR is rebuilt, and
the tool is asked the question the audit asked. A sound tool refuses (or rejects as unsupported)
every case except the ones marked harmless.

usage: audit_regression.py SRC_DIR WORK_DIR NIGHTLY > table.md
"""
import json
import os
import shutil
import subprocess
import sys

src_dir, work, nightly = sys.argv[1], sys.argv[2], sys.argv[3]
here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
import batch as B
import extract as X

ANCHOR = """impl<'a, T> MarketGroupV16ViewMut<'a, T> {
    pub fn validate_shape(&self) -> V16Result<()> {
        self.as_view().validate_shape()
    }
"""
# (function, req, reqp) -> True if a certificate is the correct answer
CASES = {
    "audit_a1": ("0", "", False, "HOLE 1 (fixed): reborrow of the market stored into a second market argument, then written"),
    "audit_a1s": ("0", "", False, "HOLE 1 (fixed): reborrow stored into an `Option<&mut Header>` argument, then written"),
    "audit_a2": ("1", "", False, "HOLE 1 (fixed): reborrow of account 1 stored into account 2, then written"),
    "audit_p1": ("", "0", False, "HOLE 1 (fixed): same alias, preservation asked, no check at all"),
    "audit_a3": ("0", "", False, "HOLE 2 (fixed): string literal \\\"a < b\\\" before a `&mut` argument"),
    "audit_a3gt": ("0", "", False, "HOLE 2 (fixed): literal with `>`"),
    "audit_a3br": ("0", "", False, "HOLE 2 (fixed): literal with `[`"),
    "audit_a3ch": ("0", "", False, "HOLE 2 (fixed): char literal `<`"),
    "audit_s3": ("0", "", False, "HOLE 2 (fixed): literal with `-> y {`"),
    "audit_a3ctl": ("0", "", False, "control: plain literal, header written after the check"),
    "audit_s1": ("0", "", False, "literal `(`"),
    "audit_s2": ("0", "", False, "literal `)`"),
    "audit_s4": ("0", "", False, "literal `<` as only argument, result stored to the header"),
    "audit_b1": ("0", "", False, "`Option<&mut>` + `as_mut`"),
    "audit_b1t": ("0", "", False, "`Option::take`"),
    "audit_b1p": ("0", "", False, "`if let Some(h) = o`"),
    "audit_b2": ("0", "", False, "tuple holding `&mut`"),
    "audit_b4": ("0", "", False, "array of `&mut`, `for` loop"),
    "audit_b5": ("0", "", False, "`mem::replace`"),
    "audit_b6": ("0", "", False, "check result overwritten through `&mut r`"),
    "audit_b6b": ("0", "", False, "check result overwritten in a tuple field"),
    "audit_b7": ("0", "", False, "conditional `r` or `Ok(())`"),
    "audit_b8": ("0", "", False, "loop: check, then write on a later iteration"),
    "audit_b9": ("0", "", False, "closure taking `&mut Header`"),
    "audit_b11": ("0", "", False, "write to `self.protection_observed` (not read by the check; refusing is conservative)"),
    "audit_b12": ("0", "", False, "`is_ok()` inverted"),
    "audit_b13": ("0", "", False, "`match` inverted"),
    "audit_b14": ("0", "", False, "check, write via reborrow chain, return the check's result"),
    "audit_b15": ("0", "", False, "write via `&mut [u8; 4]` index"),
    "audit_b16": ("0", "", False, "`iter_mut().for_each`"),
    "audit_b17": ("0", "", False, "destructuring `&mut` pattern + `fill`"),
    "audit_c2": ("0", "", False, "checking helper called on the OTHER market"),
    "audit_c2ok": ("0", "", True, "checking helper called on self (harmless: certificate is correct)"),
    "audit_c3": ("1", "", True, "helper checks its 2nd account; caller passes them swapped; asking for the right one"),
    "audit_c3#2": ("2", "", False, "same, asking for the wrong account"),
    "audit_c5": ("0", "", True, "helper panics on a failed check (harmless; a refusal is only imprecise)"),
    "r2_c3": ("0", "", False, "ROUND 2 HOLE 1 (fixed): `Ok(Err::<(), E>)` (fn item `Err` as payload) was read as `Err`"),
    "r2_c3#p": ("", "0", False, "same, preservation asked"),
    "r2_c4": ("0", "", False, "ROUND 2 HOLE 1 (fixed): caller of the above through its summary"),
    "audit_c5w": ("0", "", False, "same, then a header write"),
}

d = os.path.join(work, "audit-regression")
if os.path.isdir(d):
    shutil.rmtree(d)
os.makedirs(os.path.join(d, "src"))
for f in ("Cargo.toml", "Cargo.lock"):
    shutil.copy(os.path.join(src_dir, f), d)
for f in os.listdir(os.path.join(src_dir, "src")):
    shutil.copy(os.path.join(src_dir, "src", f), os.path.join(d, "src"))
text = open(os.path.join(d, "src", "v16.rs")).read()
assert text.count(ANCHOR) == 1
open(os.path.join(d, "src", "v16.rs"), "w").write(
    text.replace(ANCHOR, ANCHOR + open(os.path.join(here, "audit_cases", "cases.rs")).read()))
mir = os.path.join(d, "out.mir")
r = subprocess.run(["cargo", "+" + nightly, "rustc", "-j", "4", "--release", "--lib", "--",
                    "-Zunpretty=mir", "-Zinline-mir=no"], cwd=d, stdout=open(mir, "w"),
                   stderr=subprocess.PIPE, text=True)
if r.returncode != 0:
    print("the audit cases do not compile:\n" + r.stderr[-2000:])
    sys.exit(2)
fns, unsupported = B.load(mir, os.path.join(d, "src", "v16.rs"))
summ, _ = B.summarize(fns, unsupported)
json.dump({"summaries": summ}, open(os.path.join(d, "summ.json"), "w"))

print("| Case | What it does | Asked | Sound answer | Tool | Verdict |")
print("|---|---|---|---|---|---|")
unsound = 0
for key, (req, reqp, ok_expected, desc) in CASES.items():
    name = key.split("#")[0]
    p = subprocess.run([sys.executable, os.path.join(here, "extract.py"), "--mir", mir, "--fn", name, "--ns", "A",
                        "--req", req, "--reqp", reqp, "--summaries", os.path.join(d, "summ.json"), "--report"],
                       stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    got = {0: "certified", 1: "refused", 3: "refused (rejected as unsupported)"}.get(p.returncode, "error rc=%d" % p.returncode)
    if p.returncode == 3:
        got += ": " + p.stderr.strip().replace("UNSUPPORTED: ", "")[:70].replace("|", "/")
    verdict = "ok"
    if p.returncode == 0 and not ok_expected:
        verdict = "UNSOUND"
        unsound += 1
    elif p.returncode != 0 and ok_expected:
        verdict = "imprecise"
    elif p.returncode not in (0, 1, 3):
        verdict = "ERROR"
        unsound += 1
    asked = ("est " + req) if req else ("pres " + reqp)
    print("| %s | %s | %s | %s | %s | %s |" % (name, desc, asked, "certified" if ok_expected else "refused", got, verdict))
print("\nunsound answers: %d of %d cases" % (unsound, len(CASES)))
shutil.rmtree(d)
sys.exit(1 if unsound else 0)
