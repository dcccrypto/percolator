#!/usr/bin/env python3
"""Bottom-up callee summaries over every method of MarketGroupV16ViewMut in a MIR dump.

For each method, and each runtime check k it could speak about, the strongest of:
  est  : every Ok return leaves a world where check k holds;
  pres : every Ok return leaves a world where check k holds if it held on entry;
  none.
Summaries are computed by iteration from the empty table, so each one is derived only from
summaries already derived (no circular justification). Output: a JSON table used by extract.py,
plus per-function open Ok sites.

This script only *finds* summaries. A summary counts as proved only when the Lean file
generated for it by extract.py compiles (see run_all.sh).
"""
import argparse
import json
import re
import sys

sys.path.insert(0, __import__("os").path.dirname(__file__))
import extract as X


def impl_types(src_path):
    """line number -> call-site path prefix, for inherent impls of the market view."""
    out = {}
    with open(src_path) as f:
        for i, ln in enumerate(f, 1):
            if re.match(r"^impl<'a, T> MarketGroupV16ViewMut<'a, T> \{", ln):
                out[i] = "MarketGroupV16ViewMut::<'_, T>::"
    return out


def roles_of(fn):
    r = []
    seen_market = False
    for a in fn.args:
        t = fn.types[a]
        if X.MARKET_T.match(t) and not seen_market:
            r.append("market")
            seen_market = True
        elif X.ACCT_MUT_T.match(t) or X.ACCT_VIEW_T.match(t):
            r.append("account")
        else:
            r.append("other")
    return r


def analyse(fn, summaries):
    tr = X.Translator(fn, summaries)
    ir = tr.translate()
    ok_only = fn.ret.startswith("Result<")
    inv = X.fixpoint(ir, tr.checks)
    est, pres = set(tr.checks), set(tr.checks)
    nret = 0
    for b, (st, t) in enumerate(ir):
        if t[0] != "ret" or inv[b] is None:
            continue
        nret += 1
        H, Pr, av = X.out_state(ir, inv, b)
        v = av.get(0)
        if ok_only and v is not None and v[0] == "const" and v[1] != 0:
            continue
        hs = set(H)
        ps = set(Pr) | hs
        if ok_only and v is not None and v[0] == "cond":
            hs |= set(v[1])
            ps |= set(v[1]) | set(v[2])
        est &= hs
        pres &= ps
    pres -= est
    return {"args": roles_of(fn), "est": sorted(est), "pres": sorted(pres), "uncond": not ok_only,
            "checks": tr.checks, "returns": nret,
            "uses": sorted(set(p for _, p, _ in tr.used_summaries))}


def load(mir, src):
    impls = impl_types(src)
    fns, unsupported = {}, {}
    for hdr, body in X.parse_mir(mir):
        m = re.match(r"^fn v16::<impl at src/v16\.rs:(\d+):\d+: \d+:\d+>::(\w+)\(", hdr)
        if not m or int(m.group(1)) not in impls:
            continue
        path = impls[int(m.group(1))] + m.group(2)
        try:
            fn = X.Fn(hdr, body)
        except X.Unsupported as e:
            unsupported[path] = str(e)
            continue
        if not any(X.MARKET_T.match(fn.types[x]) for x in fn.args):
            continue
        fn.hdr_body = (hdr, body)
        fns[path] = fn
    return fns, unsupported


def summarize(fns, unsupported):
    summ = {}
    rounds = 0
    while True:
        rounds += 1
        changed = False
        for path, fn in fns.items():
            if path in unsupported:
                continue
            try:
                s = analyse(fn, summ)
            except X.Unsupported as e:
                unsupported[path] = str(e)
                continue
            if s["returns"] == 0:
                continue
            old = summ.get(path)
            if old is None or (old["est"], old["pres"]) != (s["est"], s["pres"]) or old["uses"] != s["uses"]:
                if s["est"] or s["pres"] or old is not None:
                    summ[path] = s
                    changed = True
        if not changed:
            break
        if rounds > 50:
            raise SystemExit("summary iteration did not converge in 50 rounds")
    return summ, rounds


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mir", required=True)
    ap.add_argument("--src", required=True)
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    fns, unsupported = load(a.mir, a.src)
    summ, rounds = summarize(fns, unsupported)
    json.dump({"summaries": summ, "unsupported": unsupported, "rounds": rounds, "methods": len(fns)},
              open(a.out, "w"), indent=1, sort_keys=True)
    print("methods with a market argument: %d; with a summary: %d; unsupported: %d; rounds: %d"
          % (len(fns), len(summ), len(unsupported), rounds))
    for p, e in sorted(unsupported.items()):
        print("UNSUPPORTED %s: %s" % (p, e[:200]))


if __name__ == "__main__":
    main()
