#!/usr/bin/env python3
"""Driver: value-indexed callee summaries (bottom-up, on demand) and the Lean modules for the
target rows. Nothing is "proved" by this script: a result counts only if its Lean module
compiles (run_strong2.sh builds them and records the result)."""
import argparse
import csv
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

    def analyse(self, path, exempt=(), assume=None):
        fn = self.fns[path]
        tr = E.T2(fn, self, assume)
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
        return {"summary": summ, "ir": ir, "inv": inv, "tr": tr, "fn": fn, "cls": cls, "ok_only": ok_only,
                "g0": g0, "exempt": tuple(exempt)}


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


def rows_for(r):
    """Row theorems to state for a module whose goal is r["cls"]."""
    out = []
    for gs, c in r["cls"]:
        if c == "bot":
            continue
        extra = [g for g in gs if g not in r["g0"]]
        if not extra:
            text = "Every %sreturn:" % ("`Ok` " if r["ok_only"] else "")
        else:
            text = "Every %sreturn with %s:" % ("`Ok` " if r["ok_only"] else "", " and ".join(
                "%s %s %d" % ("payload" if g[0] == PO else "value", "=" if g[1] == "eq" else "≠", g[2]) for g in extra))
        text += " checks %s hold, checks %s are preserved (0 = validate_shape, k = validate_with_market of account k)." % (
            list(c[0]), list(c[1]))
        out.append((list(gs), list(c[0]), list(c[1]), text))
    return out


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
    ap.add_argument("--ledger", default=os.path.join(HERE, "..", "..", "novel", "ledger", "proof_ledger_08d4fd09.tsv"))
    ap.add_argument("--sibling", default=None, help="the sibling run's ledger_results.tsv for the same revision")
    ap.add_argument("--t15", default="close_resolved_account_not_atomic")
    a = ap.parse_args()

    fns, unsupported = B.load(a.mir, a.src)
    E.ENUMS.clear()
    E.ENUMS.update(E.enum_table(a.src))
    prov = Provider(fns)
    prov.unsupported.update(unsupported)
    for d in (a.gen, a.mirdir, a.out):
        os.makedirs(d, exist_ok=True)
    if a.clean:
        for f in os.listdir(a.gen):
            if f.endswith(".lean"):
                os.remove(os.path.join(a.gen, f))
    results = {"pin": a.pin, "enums": len(E.ENUMS), "summaries": {}, "t15": {}}
    modules = []
    needed = []

    def need(path):
        if path in needed or path not in prov.done:
            return
        for _, q in prov.done[path]["tr"].used2:
            need(q)
        needed.append(path)

    def write_mir(fn):
        hdr, body = fn.hdr_body
        text, sha = E.fn_text_sha(hdr, body)
        open(os.path.join(a.mirdir, fn.name + ".mir"), "w").write(text)
        return sha

    # ---- ledger rows -------------------------------------------------------------------
    sib = {}
    if a.sibling and os.path.exists(a.sibling):
        for r in csv.reader(open(a.sibling), delimiter="\t"):
            sib[r[0]] = r[4]
    led = [["row", "function", "ledger_class", "strong2", "sibling", "lean_module"]]
    for r in list(csv.reader(open(a.ledger), delimiter="\t"))[1:]:
        m = re.match(r"^(T\d+)\.(header|account)$", r[0])
        if not m:
            continue
        name = r[2]
        cm = re.match(r"\[([a-z-]+)\]", r[6])
        klass = cm.group(1) if cm else ""
        path = P + name
        if path not in fns:
            led.append([r[0], name, klass, "no-mir", sib.get(r[0], ""), ""])
            continue
        prov.summary(path)
        if path in prov.unsupported or path not in prov.done:
            led.append([r[0], name, klass, "unsupported", sib.get(r[0], ""), ""])
            continue
        d = prov.done[path]
        ks = [0] if m.group(2) == "header" else [k for k in d["tr"].checks if k >= 1]
        if not ks:
            led.append([r[0], name, klass, "no-account-argument", sib.get(r[0], ""), ""])
            continue
        g0 = d["g0"]
        if all(E.ret_entailed(d["ir"], d["inv"], (g0, ((k,), ()))) for k in ks):
            res = "est"
        elif all(E.ret_entailed(d["ir"], d["inv"], (g0, ((), (k,)))) for k in ks):
            res = "pres"
        else:
            res = "open"
        if res != "open":
            need(path)
        led.append([r[0], name, klass, res, sib.get(r[0], ""), "S_" + name if res != "open" else ""])
    with open(os.path.join(a.out, "ledger_results.tsv"), "w") as f:
        csv.writer(f, delimiter="\t", lineterminator="\n").writerows(led)

    # ---- T15: partial rows, the Closed outcome, and the reduction to assumed callees ----
    path = P + a.t15
    if path in fns and path in prov.done:
        d = prov.done[path]
        need(path)
        fn, tr = d["fn"], d["tr"]
        sha = write_mir(fn)
        lines = span_lines(a.spans_mir, a.t15)
        ok_sites = sorted(b for b, (st, t) in enumerate(d["ir"]) if ("setD", 0, 0) in st and d["inv"][b] is not None)
        t15 = {"fn": a.t15, "mir_sha256": sha, "blocks": len(fn.blocks),
               "ok_sites": {str(b): lines.get(b) for b in ok_sites}, "rows": {}}
        g0 = d["g0"]
        callees = []
        for b in range(len(fn.blocks)):
            term = fn.blocks[b][1]
            if " -> " in term:
                sa = X.split_assign(term.rsplit(" -> ", 1)[0])
                if sa is not None:
                    try:
                        fp, _ = X.split_call(sa[1])
                    except X.Unsupported:
                        continue
                    if fp in fns and fp not in callees:
                        callees.append(fp)
        for suffix, k in ((".header", 0), (".account", 1)):
            if k not in tr.checks:
                continue
            cl = (g0, ((), (k,)))
            entry = {}
            if E.ret_entailed(d["ir"], d["inv"], cl):
                entry["whole"] = "S_" + a.t15
            else:
                sites = E.open_ok_sites(d["ir"], d["inv"], cl)
                r2 = prov.analyse(path, exempt=tuple(sites))
                ok2 = E.ret_entailed(r2["ir"], r2["inv"], cl, tuple(sites))
                entry.update({"exempt": sites, "exempt_lines": {str(b): lines.get(b) for b in sites},
                              "proved_ok_sites": len(ok_sites) - len(sites), "n_ok_sites": len(ok_sites),
                              "checked_paths_module": None})
                if ok2:
                    ns = "T15_%s_checked_paths" % suffix[1:]
                    doc = doc_for(r2, path, a.pin, sha, sorted(set(r2["tr"].used2)),
                                  "row T15%s, PARTIAL: executions that do not pass through block(s) %s\n"
                                  "(source lines of their `Ok(..)`: %s)." % (
                                      suffix, sites, ", ".join("bb%d → v16.rs:%s" % (b, lines.get(b)) for b in sites)))
                    open(os.path.join(a.gen, ns + ".lean"), "w").write(
                        E.emit_lean(a.genns, ns, doc, r2["ir"], r2["inv"], [cl], sites, tr.checks,
                                    row=[(list(g0), [], [k], "Every `Ok` return: check %d is preserved." % k)]))
                    modules.append(ns)
                    entry["checked_paths_module"] = ns
                    for _, q in r2["tr"].used2:
                        need(q)
                # reduction: the smallest set of direct callees (greedy) whose assumed preservation
                # of this check closes the row
                def assume_for(cs):
                    A = {}
                    for q in cs:
                        gq = ((0, "eq", 0),) if fns[q].ret.startswith("Result<") else ()
                        A[q] = [(gq, ((), (k,)))]
                    return A
                cur = list(callees)
                r3 = prov.analyse(path, assume=assume_for(cur))
                if E.ret_entailed(r3["ir"], r3["inv"], cl):
                    for q in list(callees):
                        trial = [x for x in cur if x != q]
                        rt = prov.analyse(path, assume=assume_for(trial))
                        if E.ret_entailed(rt["ir"], rt["inv"], cl):
                            cur = trial
                    r3 = prov.analyse(path, assume=assume_for(cur))
                    used_assumed = sorted(set(q for _, q in r3["tr"].used_assumed))
                    ns = "T15_%s_conditional" % suffix[1:]
                    doc = doc_for(r3, path, a.pin, sha, sorted(set(r3["tr"].used2)),
                                  "row T15%s, CONDITIONAL. ASSUMED (not proved) for each callee below: if it returns Ok\n"
                                  "and check %d held before the call, it holds after. These assumptions are built into\n"
                                  "the abstract program (extra post-clauses at the call sites):\n%s" % (
                                      suffix, k, "\n".join("  ASSUMED %s" % q for q in used_assumed)))
                    open(os.path.join(a.gen, ns + ".lean"), "w").write(
                        E.emit_lean(a.genns, ns, doc, r3["ir"], r3["inv"], [cl], [], tr.checks,
                                    row=[(list(g0), [], [k], "IF the ASSUMED callee contracts in this file's header hold: "
                                          "every `Ok` return preserves check %d." % k)]))
                    modules.append(ns)
                    entry["conditional_module"] = ns
                    entry["assumed"] = [q[len(P):] for q in used_assumed]
                    for _, q in r3["tr"].used2:
                        need(q)
                else:
                    entry["conditional_module"] = None
            t15["rows"]["T15" + suffix] = entry
        results["t15"] = t15

    # ---- one module per function whose summary is used or is a ledger row ----
    for path in needed:
        r = prov.done[path]
        if not r["cls"]:
            continue
        fn, tr = r["fn"], r["tr"]
        name = path[len(P):]
        sha = write_mir(fn)
        bad = E.verify(r["ir"], r["inv"], r["cls"], tr.checks)
        ns = "S_" + name
        doc = doc_for(r, path, a.pin, sha, sorted(set(tr.used2)),
                      "summary clauses (the goal):\n" + "\n".join("  " + describe_cl(c) for c in r["cls"]))
        open(os.path.join(a.gen, ns + ".lean"), "w").write(
            E.emit_lean(a.genns, ns, doc, r["ir"], r["inv"], r["cls"], [], tr.checks, row=rows_for(r)))
        modules.append(ns)
        results["summaries"][name] = {"clauses": [describe_cl(c) for c in r["cls"]], "raw": r["cls"],
                                      "mir_sha256": sha, "blocks": len(fn.blocks), "mirror_problems": len(bad),
                                      "uses": sorted(set(q[len(P):] for _, q in tr.used2))}
    results["unsupported"] = prov.unsupported
    results["analysed_methods"] = len(prov.done)
    results["modules"] = modules
    allf = os.path.join(os.path.dirname(a.gen.rstrip("/")), "All%s.lean" % a.genns)
    with open(allf, "w") as f:
        for m in modules:
            f.write("import ClauseFlow.%s.%s\n" % (a.genns, m))
        f.write("\n")
        for m in modules:
            f.write("#print axioms ClauseFlow.%s.%s.main\n" % (a.genns, m))
    json.dump(results, open(os.path.join(a.out, "run.json"), "w"), indent=1, sort_keys=True, default=str)
    n = {}
    for r in led[1:]:
        n[r[3]] = n.get(r[3], 0) + 1
    print("ledger rows:", json.dumps(n, sort_keys=True))
    newly = [r[0] for r in led[1:] if r[3] in ("est", "pres") and r[4] not in ("est", "pres")]
    lost = [r[0] for r in led[1:] if r[4] in ("est", "pres") and r[3] not in ("est", "pres")]
    print("rows this run closes that the sibling run left open:", newly)
    print("rows the sibling run closed that this run does not:", lost)
    print("T15:", json.dumps(results["t15"].get("rows", {}), sort_keys=True))
    print("modules: %d; methods analysed: %d; plain crate enums: %d" % (len(modules), len(prov.done), len(E.ENUMS)))


if __name__ == "__main__":
    main()
