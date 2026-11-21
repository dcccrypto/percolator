#!/usr/bin/env python3
"""slice_diff.py: print, for the Aristotle upload in project/, (a) byte-identity of every copied block
against the engine source at the pinned commit, (b) unified diffs of each transcribed function
against the engine function it transcribes. Reads the engine from `git show c38b6b1e:...`."""
import difflib, re, subprocess, sys, pathlib
HERE = pathlib.Path(__file__).resolve().parent
PIN = "c38b6b1e"
def git_show(path):
    return subprocess.run(["git","show",f"{PIN}:{path}"],cwd=HERE,check=True,capture_output=True).stdout.decode()
v16 = git_show("src/v16.rs").split("\n")
wide = git_show("src/wide_math.rs")
out = []
P = out.append
# (a) copied blocks
slice_rs = (HERE/"project/src/resolved_payout.rs").read_text()
P("## A. Copied blocks vs engine (byte comparison)\n")
P("| file | engine range | result |\n|---|---|---|")
for m in re.finditer(r"// ---- src/v16\.rs:(\d+)-(\d+) ----\n", slice_rs):
    a,b = int(m.group(1)), int(m.group(2))
    block = "\n".join(v16[a-1:b])
    rest = slice_rs[m.end():]
    ok = block in rest
    P(f"| project/src/resolved_payout.rs | src/v16.rs:{a}-{b} | {'byte-identical' if ok else 'DIFFERENT'} |")
P(f"| project/src/wide_math.rs | whole src/wide_math.rs | {'byte-identical' if (HERE/'project/src/wide_math.rs').read_text()==wide else 'DIFFERENT'} |")
# (b) transcribed functions
cp = (HERE/"project/src/resolved_payout/close_path.rs").read_text().split("\n")
def fn_text(lines, start_pat):
    i = next(k for k,l in enumerate(lines) if re.search(start_pat,l))
    ind = len(lines[i]) - len(lines[i].lstrip())
    j = i
    while not (lines[j].startswith(" "*ind+"}") and len(lines[j].rstrip())==ind+1): j += 1
    return lines[i:j+1]
def dedent(ls):
    return [l[4:] if l.startswith("    ") else l for l in ls]
pairs = [
 ("create_resolved_payout_receipt_if_needed", r"fn create_resolved_payout_receipt_if_needed\(", r"pub fn create_resolved_payout_receipt_if_needed\("),
 ("claim_resolved_payout_topup_core_not_atomic -> claim_resolved_payout_topup_core", r"fn claim_resolved_payout_topup_core_not_atomic\(", r"pub fn claim_resolved_payout_topup_core\("),
]
P("\n## B. Transcribed functions: unified diff, engine (-) vs slice (+)\n")
for title, ep, sp in pairs:
    e = dedent(fn_text(v16, ep)); s = fn_text(cp, sp)
    P(f"### {title}\n\n```diff")
    P("\n".join(difflib.unified_diff(e, s, "engine src/v16.rs@"+PIN, "slice close_path.rs", lineterm="", n=2)))
    P("```\n")
# tail: engine lines of close_resolved_account_not_atomic from 'let mut payout_receipt' to end
i = next(k for k,l in enumerate(v16) if "let mut payout_receipt = None;" in l and k>26000)
j = next(k for k in range(i,len(v16)) if "Ok(ResolvedCloseOutcomeV16::Closed { payout })" in v16[k])
e = dedent(dedent(v16[i:j+1])) if False else [l[8:] if l.startswith(" "*8) else l for l in v16[i:j+1]]
s_full = fn_text(cp, r"pub fn close_resolved_payout_tail\(")
s = [l[4:] if l.startswith("    ") else l for l in s_full]
P(f"### tail of close_resolved_account_not_atomic (engine src/v16.rs:{i+1}-{j+1}) -> close_resolved_payout_tail\n\n```diff")
P("\n".join(difflib.unified_diff(e, s, "engine src/v16.rs@"+PIN, "slice close_path.rs", lineterm="", n=2)))
P("```\n")
print("\n".join(out))
