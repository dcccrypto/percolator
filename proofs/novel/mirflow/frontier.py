#!/usr/bin/env python3
"""For a function and a check, reduce "f preserves the check on every Ok return" to callee
contracts, recursively, until functions are reached whose own body writes the market with no
check after it ("leaves"). Output: the reduction tree and the set of leaves.

Every node of the tree is a statement of the tool about the abstract program ("with these callee
contracts assumed, the certificate exists"); the leaves are what remains to be proved by a method
that reasons about values (Kani, Aristotle). Nothing here is kernel-checked except through the
modules run_all.sh builds; this is a map of obligations.

usage: frontier.py --mir M --src S --fn NAME --check K
"""
import argparse
import os
import re
import sys

sys.path.insert(0, os.path.dirname(__file__))
import extract as X
import batch as B

P = "MarketGroupV16ViewMut::<'_, T>::"


def callee_paths(fn, fns):
    out = []
    for b, (st, term, _) in fn.blocks.items():
        if " -> " not in term:
            continue
        sa = X.split_assign(term.rsplit(" -> ", 1)[0])
        if sa is None:
            continue
        try:
            p, _ = X.split_call(sa[1])
        except X.Unsupported:
            continue
        if p in fns and p not in out:
            out.append(p)
    return out


def reduce(path, k, fns, summ):
    """(closed, minimal list of callees whose preservation of check k must be assumed)."""
    fn = fns[path]
    cands = [p for p in callee_paths(fn, fns)
             if p != path and not (p in summ and (k in summ[p]["est"] or k in summ[p]["pres"]))
             and re.match(r"^&mut ", fns[p].types[fns[p].args[0]])]

    def attempt(assumed):
        tab = dict(summ)
        for p in assumed:
            base = dict(summ.get(p, {"args": B.roles_of(fns[p]), "est": [], "pres": [],
                                     "uncond": not fns[p].ret.startswith("Result<"), "uses": []}))
            base["pres"] = sorted(set(base["pres"]) | {k})
            base["status"] = "ASSUMED"
            base["direct"] = True
            tab[p] = base
        try:
            info, _ = X.run_one(fn.hdr_body[0], fn.hdr_body[1], tab, [], [k], "", "X")
        except X.Unsupported as e:
            return None
        return info

    info = attempt(cands)
    if info is None:
        return False, [], "not translated"
    if info["open_returns"]:
        return False, cands, "own writes"
    assumed = list(cands)
    for p in list(cands):
        trial = [q for q in assumed if q != p]
        i2 = attempt(trial)
        if i2 is not None and not i2["open_returns"]:
            assumed = trial
    return True, assumed, ""


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mir", required=True)
    ap.add_argument("--src", required=True)
    ap.add_argument("--fn", required=True)
    ap.add_argument("--check", type=int, default=0)
    a = ap.parse_args()
    fns, unsupported = B.load(a.mir, a.src)
    summ, _ = B.summarize(fns, unsupported)
    k = a.check
    memo, leaves, lines = {}, {}, []

    def walk(path, depth, stack):
        name = path[len(P):]
        if path in unsupported:
            leaves[name] = "not translated by the tool"
            lines.append("%s- `%s`: **leaf** (not translated by the tool)" % ("  " * depth, name))
            return
        if path in stack:
            lines.append("%s- `%s`: (recursive call, see above)" % ("  " * depth, name))
            return
        if path in memo:
            lines.append("%s- `%s`: (see above)" % ("  " * depth, name))
            return
        closed, assumed, why = reduce(path, k, fns, summ)
        memo[path] = (closed, assumed)
        if not closed:
            leaves[name] = why or "own writes"
            lines.append("%s- `%s`: **leaf** (%s: writes the market in its own body, or through a non-method "
                         "callee, with no check after)" % ("  " * depth, name, why or "own writes"))
            return
        if not assumed:
            lines.append("%s- `%s`: certified with no assumption" % ("  " * depth, name))
            return
        lines.append("%s- `%s`: reduces to %d callee contract(s)" % ("  " * depth, name, len(assumed)))
        for q in assumed:
            walk(q, depth + 1, stack + [path])

    walk(P + a.fn, 0, [])
    print("# Reduction of `%s` preserving check %d to leaf obligations\n" % (a.fn, k))
    print("Read: a function \"reduces to\" its listed callees when the tool finds a certificate for it with "
          "exactly those callees assumed to preserve the check on their Ok returns. A leaf is a function for "
          "which no such certificate exists even with every market-writing method callee assumed.\n")
    print("\n".join(lines))
    print("\n## Leaves (%d)\n" % len(leaves))
    for n in sorted(leaves):
        hdr = fns[P + n].hdr_body[0] if P + n in fns else ""
        print("- `%s` (%s; MIR blocks: %s)" % (n, leaves[n], len(fns[P + n].blocks) if P + n in fns else "?"))


if __name__ == "__main__":
    main()
