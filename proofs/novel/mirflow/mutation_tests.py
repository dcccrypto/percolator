#!/usr/bin/env python3
"""Mutation tests for the extractor: edit the pinned source of
`claim_resolved_payout_topup_not_atomic` (T14), rebuild MIR, and ask the tool to certify
check 0 (validate_shape) and check 1 (validate_with_market) at every Ok return.

Each mutation states what a sound tool must answer. A test FAILS if the tool certifies a check
that the mutation breaks (unsound), or refuses one that a harmless mutation keeps (imprecise;
reported, not counted as unsound). This is a test of the trusted translation, not a proof of it.

usage: mutation_tests.py SRC_DIR WORK_DIR NIGHTLY > table.md
"""
import json
import os
import shutil
import subprocess
import sys

src_dir, work, nightly = sys.argv[1], sys.argv[2], sys.argv[3]
here = os.path.dirname(os.path.abspath(__file__))

ORIG = """        self.preflight_claim_resolved_payout_topup(&account.as_view())?;
        let payout = self.claim_resolved_payout_topup_core_not_atomic(account)?;
        self.validate_shape()?;
        account.validate_with_market(&self.as_view())?;
        Ok(payout)
    }
"""
PRE = """        self.preflight_claim_resolved_payout_topup(&account.as_view())?;
        let payout = self.claim_resolved_payout_topup_core_not_atomic(account)?;
"""
CHK = """        self.validate_shape()?;
        account.validate_with_market(&self.as_view())?;
"""
END = "        Ok(payout)\n    }\n"

# name, replacement body, expected (check0 certified, check1 certified), description
M = [
    ("M00-original", ORIG, (True, True), "unchanged"),
    ("M01-header-write-after-checks", PRE + CHK + "        self.header.vault = V16PodU128::new(0);\n" + END,
     (False, False), "direct write to the market header after both checks"),
    ("M02-account-write-after-checks", PRE + CHK + "        account.header.health_cert.valid = 0;\n" + END,
     (True, False), "direct write to the account after both checks (market untouched)"),
    ("M03-no-shape-check", PRE + "        account.validate_with_market(&self.as_view())?;\n" + END,
     (False, True), "validate_shape removed"),
    ("M04-shape-result-ignored", PRE + "        let _ = self.validate_shape();\n        account.validate_with_market(&self.as_view())?;\n" + END,
     (False, True), "validate_shape called, result dropped"),
    ("M05-mut-callee-after-checks", PRE + CHK + "        self.recompute_resolved_payout_rate()?;\n" + END,
     (False, False), "a `&mut self` method called after the checks"),
    ("M06-account-callee-after-checks", PRE + CHK + "        account.compact_source_domains();\n" + END,
     (True, False), "a `&mut` account method called after the checks"),
    ("M07-write-through-reborrow", PRE + CHK + "        let h = &mut *self.header;\n        let v = &mut h.vault;\n        *v = V16PodU128::new(0);\n" + END,
     (False, False), "write through a chain of reborrows"),
    ("M08-write-in-closure", PRE + CHK + "        let mut f = || { self.header.vault = V16PodU128::new(0); };\n        f();\n" + END,
     (False, False), "write inside a closure that captures self"),
    ("M09-pure-read-after-checks", PRE + CHK + "        let _x = self.header.vault.get();\n        let _y = self.resolved_positive_payout_ready()?;\n" + END,
     (True, True), "reads and a `&self` method after the checks (harmless)"),
    ("M10-early-ok", PRE + "        if payout == 0 {\n            return Ok(0);\n        }\n" + CHK + END,
     (False, False), "early Ok return between the core and the checks"),
    ("M11-inverted-check", PRE + "        if self.validate_shape().is_err() {\n            return Ok(payout);\n        }\n        account.validate_with_market(&self.as_view())?;\n" + END,
     (False, False), "Ok returned exactly when validate_shape fails"),
    ("M12-check-then-core", "        self.preflight_claim_resolved_payout_topup(&account.as_view())?;\n" + CHK
     + "        let payout = self.claim_resolved_payout_topup_core_not_atomic(account)?;\n" + END,
     (False, False), "checks moved before the core"),
    ("M13-mem-swap", PRE + CHK + "        let mut other = V16PodU128::new(0);\n        core::mem::swap(&mut self.header.vault, &mut other);\n" + END,
     (False, False), "core::mem::swap on a header field after the checks"),
    ("M14-match-on-check", PRE + "        match self.validate_shape() {\n            Ok(()) => {}\n            Err(e) => return Err(e),\n        }\n        account.validate_with_market(&self.as_view())?;\n" + END,
     (True, True), "validate_shape handled by an explicit match (harmless)"),
    ("M15-markets-slice-reverse", PRE + CHK + "        self.markets.reverse();\n" + END,
     (False, False), "a `&mut [T]` method (`reverse`) on the market slice after the checks"),
    ("M16-local-write-after-checks", PRE + CHK + "        let mut scratch = [0u8; 4];\n        scratch[0] = payout as u8;\n        let _s = &mut scratch;\n" + END,
     (True, True), "write to a local array only (harmless)"),
    ("M17-check-other-account", PRE + "        self.validate_shape()?;\n        account.validate_with_market(&self.as_view())?;\n        account.header.capital = V16PodU128::new(1);\n        self.validate_shape()?;\n" + END,
     (True, False), "account written after its check, shape re-checked"),
]

src_file = os.path.join(src_dir, "src", "v16.rs")
text = open(src_file).read()
assert text.count(ORIG) == 1, "pinned function text not found exactly once"

rows = []
unsound = 0
for name, body, expect, desc in M:
    d = os.path.join(work, name)
    if os.path.isdir(d):
        shutil.rmtree(d)
    os.makedirs(os.path.join(d, "src"))
    for f in ("Cargo.toml", "Cargo.lock"):
        shutil.copy(os.path.join(src_dir, f), d)
    for f in os.listdir(os.path.join(src_dir, "src")):
        shutil.copy(os.path.join(src_dir, "src", f), os.path.join(d, "src"))
    open(os.path.join(d, "src", "v16.rs"), "w").write(text.replace(ORIG, body))
    mir = os.path.join(d, "out.mir")
    r = subprocess.run(["cargo", "+" + nightly, "rustc", "-j", "4", "--release", "--lib", "--",
                        "-Zunpretty=mir", "-Zinline-mir=no"], cwd=d, stdout=open(mir, "w"),
                       stderr=subprocess.PIPE, text=True)
    if r.returncode != 0:
        rows.append((name, desc, expect, ("compile-error", "compile-error"), "n/a"))
        continue
    got = []
    for k in (0, 1):
        p = subprocess.run([sys.executable, os.path.join(here, "extract.py"), "--mir", mir, "--fn",
                            "claim_resolved_payout_topup_not_atomic", "--ns", "Mut", "--req", str(k), "--report"],
                           stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        if p.returncode == 0:
            got.append(True)
        elif p.returncode == 1:
            got.append(False)
        else:
            got.append("unsupported")
    verdict = "ok"
    for e, g in zip(expect, got):
        if g is True and e is False:
            verdict = "UNSOUND"
            unsound += 1
        elif g is not True and e is True and verdict == "ok":
            verdict = "imprecise"
    rows.append((name, desc, expect, tuple(got), verdict))
    shutil.rmtree(d)

f = lambda b: {True: "certified", False: "refused", "unsupported": "refused (unsupported MIR)",
               "compile-error": "does not compile"}[b]
print("| Mutation | What it does | Sound answer (check 0, check 1) | Tool (check 0, check 1) | Verdict |")
print("|---|---|---|---|---|")
for name, desc, e, g, v in rows:
    print("| %s | %s | %s, %s | %s, %s | %s |" % (name, desc, f(e[0]), f(e[1]), f(g[0]), f(g[1]), v))
print("\nunsound answers: %d of %d mutations" % (unsound, len(rows)))
sys.exit(1 if unsound else 0)
