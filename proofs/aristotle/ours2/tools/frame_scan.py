#!/usr/bin/env python3
"""frame_scan.py: a TEXT scan (not a proof) behind the STUB-FRAME hypothesis.
For each stubbed engine function, take every function whose name is reachable from it by calls
found textually in src/v16.rs at the pinned commit (same-named functions in different impls are
all included, so the set over-approximates), and list the header fields those bodies assign
(`<x>.header.<field> = ...` or `header.<field> = ...`). Then report which of the slice's header
fields appear."""
import re, subprocess, pathlib, sys
HERE = pathlib.Path(__file__).resolve().parent
PIN = "c38b6b1e"
src = subprocess.run(["git", "show", PIN + ":src/v16.rs"], cwd=HERE, check=True, capture_output=True).stdout.decode()
lines = src.split("\n")
fn_re = re.compile(r"^(\s*)(?:pub(?:\([a-z]+\))? )?(?:const )?(?:unsafe )?fn ([A-Za-z0-9_]+)\s*[<(]")
bodies = {}  # name -> list of (start, end)
i = 0
while i < len(lines):
    m = fn_re.match(lines[i])
    if not m:
        i += 1; continue
    ind, name = m.group(1), m.group(2)
    j = i
    # body ends at the first line that is exactly indent + "}" ; declarations without body end with ';'
    while j < len(lines) and lines[j] != ind + "}":
        if lines[j].rstrip().endswith(";") and "{" not in "".join(lines[i:j + 1]):
            break
        j += 1
    bodies.setdefault(name, []).append((i, j))
    i += 1
names = set(bodies)
call_re = re.compile(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*(?:::<[^>]*>)?\(")
def callees(name):
    out = set()
    for a, b in bodies[name]:
        for l in lines[a + 1:b + 1]:
            if l.lstrip().startswith("//"): continue
            for c in call_re.findall(l):
                if c in names: out.add(c)
    return out
SLICE_MARKET = "vault insurance c_tot pnl_pos_tot pnl_pos_bound_tot_num pnl_pos_bound_tot pnl_matured_pos_tot backing_provider_earnings_total source_fresh_backing_total_num resolved_payout_blocker_count stale_certificate_count b_stale_account_count negative_pnl_account_count risk_epoch current_slot mode resolved_slot payout_snapshot payout_snapshot_pnl_pos_tot payout_snapshot_captured resolved_payout_ledger".split()
SLICE_ACCOUNT = "capital pnl reserved_pnl fee_credits health_cert resolved_payout_receipt".split()
w_re = re.compile(r"\b(self|account|market|group|view|portfolio)?\.?header\s*\.\s*([a-z_0-9]+)(?:\.[a-z_0-9]+)*\s*(?:=[^=]|\+=|-=)")
for root in sys.argv[1:] or ["burn_account_source_claim_bound_num", "sync_unattributed_loss_lock"]:
    seen, todo = set(), [root]
    while todo:
        n = todo.pop()
        if n in seen: continue
        seen.add(n); todo.extend(callees(n) - seen)
    writes = {}
    for n in sorted(seen):
        for a, b in bodies[n]:
            for k in range(a, b + 1):
                l = lines[k]
                if l.lstrip().startswith("//"): continue
                for recv, field in w_re.findall(l):
                    writes.setdefault((recv or "?", field), []).append("%s:%d" % (n, k + 1))
    print("## %s: %d function names reachable by text" % (root, len(seen)))
    hit = False
    for (recv, field), where in sorted(writes.items()):
        tag = ""
        if recv in ("self", "market", "group", "?") and field in SLICE_MARKET: tag = "  <-- market field of the slice"
        if recv in ("account", "portfolio", "?") and field in SLICE_ACCOUNT: tag = "  <-- account field of the slice"
        if tag: hit = True
        print("  %s.header.%s  (%d sites, first %s)%s" % (recv, field, len(where), where[0], tag))
    print("  => slice fields assigned somewhere in this set: %s" % ("YES (see marks)" if hit else "none"))
