#!/usr/bin/env python3
"""Driver: value-indexed callee summaries (bottom-up, on demand) and the Lean modules for the
target rows. Nothing is "proved" by this script: a result counts only if its Lean module
compiles (run_strong2.sh builds them and records the result)."""
import argparse
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import extract2 as E  # noqa: E402
X = E.X
sys.path.insert(0, os.path.join(HERE, "..", "..", "novel", "mirflow"))
import batch as B  # noqa: E402

P = "MarketGroupV16ViewMut::<'_, T>::"
PO = E.PO


def span_lines(spans_mir, fn_name):
    out, cur, inside = {}, None, False
    with open(spans_mir) as f:
        for ln in f:
            if ln.startswith("fn "):
                inside = bool(re.match(r"^fn v16::<impl at [^>]*>::%s\(" % re.escape(fn_name), ln))
                continue
            if not inside:
                continue
            m = re.match(r"^    bb(\d+)", ln)
            if m:
                cur = int(m.group(1))
            m = re.search(r"_0 = .*::Ok\(.*// scope \d+ at src/v16\.rs:(\d+):", ln)
            if m and cur is not None:
                out.setdefault(cur, int(m.group(1)))
    return out


class Provider:
    """Summaries on demand. A summary of f is computed only from summaries of functions whose
    computation finished before f's started being used: a call met while its callee is still in
    progress (recursion) gets no summary."""

    def __init__(self, fns, assumed=None):
        self.fns = fns
        self.done = {}
        self.inprog = set()
        self.unsupported = {}
        self.assumed = assumed or {}
        self.order = []

    def summary(self, path):
        if path in self.assumed:
            return self.assumed[path]
        if path in self.done:
            return self.done[path]["summary"]
        if path not in self.fns or path in self.inprog or path in self.unsupported:
            return None
        self.inprog.add(path)
        try:
            r = self.analyse(path)
        except X.Unsupported as e:
            self.unsupported[path] = str(e)
            r = None
        self.inprog.discard(path)
        if r is None:
            return None
        self.done[path] = r
        self.order.append(path)
        return r["summary"]

    def analyse(self, path, exempt=()):
        fn = self.fns[path]
        tr = E.T2(fn, self)
        ir = tr.translate()
        ok_only = fn.ret.startswith("Result<")
        inv = E.fixpoint2(ir, tr.checks, exempt)
        g0 = (((0, "eq", 0),) if ok_only else ())
        cls = []
        nret = sum(1 for b, (st, t) in enumerate(ir) if t[0] == "ret" and inv[b] is not None)
        if nret:
            vals = (0, 1)
            vl = PO if ok_only else 0
            botv = set()
            for v in vals:
                gv = tuple(sorted(g0 + ((vl, "eq", v),)))
                if E.ret_entailed(ir, inv, (gv, "bot"), exempt):
                    cls.append((gv, "bot"))
                    botv.add(v)
            for k in tr.checks:
                if E.ret_entailed(ir, inv, (g0, ((k,), ())), exempt):
                    cls.append((g0, ((k,), ())))
                    continue
                pres_all = E.ret_entailed(ir, inv, (g0, ((), (k,))), exempt)
                if pres_all:
                    cls.append((g0, ((), (k,))))
                for v in vals:
                    if v in botv:
                        continue
                    gv = tuple(sorted(g0 + ((vl, "eq", v),)))
                    if E.ret_entailed(ir, inv, (gv, ((k,), ())), exempt):
                        cls.append((gv, ((k,), ())))
                    elif not pres_all and E.ret_entailed(ir, inv, (gv, ((), (k,))), exempt):
                        cls.append((gv, ((), (k,))))
        cls = merge(cls)
        summ = {"args": B.roles_of(fn), "cls": cls, "checks": tr.checks, "ok_only": ok_only} if cls else None
        return {"summary": summ, "ir": ir, "inv": inv, "tr": tr, "fn": fn, "cls": cls, "ok_only": ok_only}


def merge(cls):
    """Clauses with the same guards become one clause."""
    by = {}
    order = []
    for gs, c in cls:
        if gs not in by:
            by[gs] = [set(), set(), False]
            order.append(gs)
        if c == "bot":
            by[gs][2] = True
        else:
            by[gs][0] |= set(c[0])
            by[gs][1] |= set(c[1])
    out = []
    for gs in order:
        h, p, bot = by[gs]
        out.append((gs, "bot") if bot else (gs, (tuple(sorted(h)), tuple(sorted(p - h)))))
    return out


def describe_cl(c):
    gs, cc = c
    names = {0: "result", PO: "payload"}
    g = " ∧ ".join("%s %s %d" % (names.get(x[0], "_%d" % x[0]), "=" if x[1] == "eq" else "≠", x[2]) for x in gs) or "always"
    if cc == "bot":
        return "%s: impossible" % g
    parts = []
    if cc[0]:
        parts.append("checks %s hold" % list(cc[0]))
    if cc[1]:
        parts.append("checks %s preserved" % list(cc[1]))
    return "%s: %s" % (g, "; ".join(parts))


def doc_for(r, path, pin, sha, used, note=""):
    fn, tr = r["fn"], r["tr"]
    return ("GENERATED by proofs/strong2/valflow (extract2.py, run2.py). Do not edit.\n"
            "function: %s\n%s\nsha256 of the function's MIR text: %s\n"
            "MIR blocks: %d, locals: %d. market root: _%s; accounts: %s\n"
            "result = local 0 (0 is Ok); payload = local %d (ghost: integer value or enum discriminant\n"
            "of the Ok payload; for Option: None = 0, Some = 1).\n"
            "callee summaries used (block, callee), each certified by its own module in this directory:\n%s\n%s"
            % (fn.path, pin, sha, len(fn.blocks), len(fn.types), tr.market,
               ", ".join("_%d (%s)" % x for x in tr.accounts) or "none", PO,
               "\n".join("  bb%d %s" % u for u in used) or "  none", note))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mir", required=True)
    ap.add_argument("--spans-mir", required=True)
    ap.add_argument("--src", required=True)
    ap.add_argument("--gen", required=True)
    ap.add_argument("--mirdir", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--pin", required=True)
    ap.add_argument("--genns", default="Gen")
    ap.add_argument("--clean", action="store_true", help="remove generated .lean files of an earlier run from --gen")
    ap.add_argument("--targets", default="T18:convert_released_pnl_to_capital_not_atomic,T15:close_resolved_account_not_atomic")
    a = ap.parse_args()

    fns, unsupported = B.load(a.mir, a.src)
    prov = Provider(fns)
    prov.unsupported.update(unsupported)
    os.makedirs(a.gen, exist_ok=True)
    if a.clean:
        for f in os.listdir(a.gen):
            if f.endswith(".lean"):
                os.remove(os.path.join(a.gen, f))
    os.makedirs(a.mirdir, exist_ok=True)
    os.makedirs(a.out, exist_ok=True)
    results = {"targets": {}, "summaries": {}, "pin": a.pin}
    modules = []
    needed = []   # callee paths whose summaries a target (transitively) used

    def need(path):
        if path in needed or path not in prov.done:
            return
        for _, q in prov.done[path]["tr"].used2:
            need(q)
        needed.append(path)

    for spec in a.targets.split(","):
        row, name = spec.split(":")
        path = P + name
        if path not in fns:
            results["targets"][row] = {"fn": name, "status": "not a method of the market view in this MIR"}
            continue
        try:
            r = prov.analyse(path)
        except X.Unsupported as e:
            results["targets"][row] = {"fn": name, "status": "unsupported: %s" % e}
            continue
        fn, tr, ir, inv = r["fn"], r["tr"], r["ir"], r["inv"]
        hdr, body = fn.hdr_body
        text, sha = E.fn_text_sha(hdr, body)
        open(os.path.join(a.mirdir, name + ".mir"), "w").write(text)
        lines = span_lines(a.spans_mir, name)
        info = {"fn": name, "mir_sha256": sha, "blocks": len(fn.blocks), "ir_blocks": len(ir),
                "checks": tr.checks, "accounts": tr.accounts, "summary_clauses": [describe_cl(c) for c in r["cls"]],
                "uses": sorted(set(q for _, q in tr.used2)), "rows": {}}
        for _, q in tr.used2:
            need(q)
        ok_sites = sorted(b for b, (st, t) in enumerate(ir) if ("setD", 0, 0) in st and inv[b] is not None)
        info["ok_sites"] = {str(b): lines.get(b) for b in ok_sites}
        for suffix, k in ((".header", 0), (".account", 1)):
            if k not in tr.checks:
                continue
            g0 = ((0, "eq", 0),)
            entry = {}
            kind = None
            if E.ret_entailed(ir, inv, (g0, ((k,), ()))):
                kind, row_cl, rowarg = "est", (g0, ((k,), ())), ([k], [])
            elif E.ret_entailed(ir, inv, (g0, ((), (k,)))):
                kind, row_cl, rowarg = "pres", (g0, ((), (k,))), ([], [k])
            ns = "%s_%s" % (row, suffix[1:])
            if kind is not None:
                bad = E.verify(ir, inv, [row_cl], tr.checks)
                entry = {"status": "whole row, %s" % kind, "module": ns, "exempt": [], "mirror_problems": len(bad)}
                doc = doc_for(r, path, a.pin, sha, sorted(set(tr.used2)),
                              "row %s%s: every Ok return; no excluded block, no assumed callee." % (row, suffix))
                open(os.path.join(a.gen, ns + ".lean"), "w").write(
                    E.emit_lean(a.genns, ns, doc, ir, inv, [row_cl], [], tr.checks, row=rowarg))
                modules.append(ns)
            else:
                # which Ok sites are open, and the same analysis with them excluded
                cl = (g0, ((), (k,)))
                sites = E.open_ok_sites(ir, inv, cl)
                r2 = Provider.analyse(prov, path, exempt=tuple(sites))
                ir2, inv2 = r2["ir"], r2["inv"]
                ok2 = E.ret_entailed(ir2, inv2, cl, tuple(sites))
                bad = E.verify(ir2, inv2, [cl], tr.checks, tuple(sites))
                entry = {"status": "partial: Ok returns outside the excluded blocks, pres" if ok2 else "open",
                         "module": ns + "_checked_paths" if ok2 else None,
                         "exempt": sites, "exempt_lines": {str(b): lines.get(b) for b in sites},
                         "proved_ok_sites": len(ok_sites) - len(sites), "ok_sites": len(ok_sites),
                         "mirror_problems": len(bad)}
                if ok2:
                    doc = doc_for(r2, path, a.pin, sha, sorted(set(r2["tr"].used2)),
                                  "row %s%s, PARTIAL: executions that do not pass through block(s) %s\n"
                                  "(source lines of their `Ok(..)`: %s)." % (
                                      row, suffix, sites, ", ".join("bb%d → v16.rs:%s" % (b, lines.get(b)) for b in sites)))
                    open(os.path.join(a.gen, ns + "_checked_paths.lean"), "w").write(
                        E.emit_lean(a.genns, ns + "_checked_paths", doc, ir2, inv2, [cl], sites, tr.checks, row=([], [k])))
                    modules.append(ns + "_checked_paths")
            info["rows"][row + suffix] = entry
        results["targets"][row] = info

    # modules for every summary a target used, callees first
    for path in needed:
        r = prov.done[path]
        if not r["cls"]:
            continue
        fn, tr = r["fn"], r["tr"]
        name = path[len(P):]
        hdr, body = fn.hdr_body
        text, sha = E.fn_text_sha(hdr, body)
        open(os.path.join(a.mirdir, name + ".mir"), "w").write(text)
        bad = E.verify(r["ir"], r["inv"], r["cls"], tr.checks)
        ns = "S_" + name
        doc = doc_for(r, path, a.pin, sha, sorted(set(tr.used2)),
                      "summary clauses (the goal):\n" + "\n".join("  " + describe_cl(c) for c in r["cls"]))
        open(os.path.join(a.gen, ns + ".lean"), "w").write(
            E.emit_lean(a.genns, ns, doc, r["ir"], r["inv"], r["cls"], [], tr.checks))
        modules.append(ns)
        results["summaries"][name] = {"clauses": [describe_cl(c) for c in r["cls"]], "raw": r["cls"],
                                      "mir_sha256": sha, "blocks": len(fn.blocks), "mirror_problems": len(bad),
                                      "uses": sorted(set(q[len(P):] for _, q in tr.used2))}
    results["unsupported"] = prov.unsupported
    results["analysed_methods"] = len(prov.done)
    allf = os.path.join(os.path.dirname(a.gen.rstrip("/")), "All%s.lean" % a.genns)
    with open(allf, "w") as f:
        for m in modules:
            f.write("import ClauseFlow.%s.%s\n" % (a.genns, m))
        f.write("\n")
        for m in modules:
            f.write("#print axioms ClauseFlow.%s.%s.main\n" % (a.genns, m))
    json.dump(results, open(os.path.join(a.out, "run.json"), "w"), indent=1, sort_keys=True, default=str)
    for row, info in results["targets"].items():
        print(row, json.dumps(info.get("rows", info), sort_keys=True))
    print("modules: %d; summaries used: %d; methods analysed: %d" % (len(modules), len(results["summaries"]), len(prov.done)))


if __name__ == "__main__":
    main()
