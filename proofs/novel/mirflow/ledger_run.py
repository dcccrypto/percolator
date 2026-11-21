#!/usr/bin/env python3
"""Run the checked-exit analysis over the T rows of Toly's proof ledger and generate the Lean
modules. Nothing is "proved" by this script: a row counts only if its Lean module compiles
(run_all.sh builds them and records the result)."""
import argparse
import csv
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(__file__))
import extract as X
import batch as B

P = "MarketGroupV16ViewMut::<'_, T>::"


def span_lines(spans_mir, fn_name):
    """block -> source line of its `_0 = ...Ok(` statement, from a MIR dump with spans."""
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


def mod_name(fn_name):
    return "S_" + fn_name


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mir", required=True)
    ap.add_argument("--spans-mir", required=True)
    ap.add_argument("--src", required=True)
    ap.add_argument("--ledger", required=True)
    ap.add_argument("--gen", required=True, help="directory for generated Lean modules")
    ap.add_argument("--mirdir", required=True, help="directory for per-function MIR text")
    ap.add_argument("--out", required=True, help="directory for tables")
    ap.add_argument("--pin", required=True)
    a = ap.parse_args()

    fns, unsupported = B.load(a.mir, a.src)
    summ, rounds = B.summarize(fns, unsupported)
    for s in summ.values():
        s["status"] = "certified"
    json.dump({"summaries": summ, "unsupported": unsupported, "rounds": rounds, "methods": len(fns)},
              open(os.path.join(a.out, "summaries.json"), "w"), indent=1, sort_keys=True)

    # acyclicity of the "uses" relation among summaries
    def cyc():
        state = {}
        def visit(p):
            if state.get(p) == 1:
                return True
            if state.get(p) == 2:
                return False
            state[p] = 1
            for q in summ.get(p, {}).get("uses", []):
                if q in summ and visit(q):
                    return True
            state[p] = 2
            return False
        return any(visit(p) for p in list(summ))
    cyclic = cyc()

    rows = list(csv.reader(open(a.ledger), delimiter="\t"))
    results = []
    need = set()
    for r in rows[1:]:
        m = re.match(r"^(T\d+)\.(header|account)$", r[0])
        if not m:
            continue
        fn_name, status = r[2], r[3]
        cls = re.match(r"\[([a-z-]+)\]", r[6])
        cls = cls.group(1) if cls else ""
        path = P + fn_name
        fn = fns.get(path)
        if fn is None:
            results.append([r[0], fn_name, cls, status, "no-mir", "", ""])
            continue
        s = summ.get(path)
        checks = B.analyse(fn, summ)["checks"] if path not in unsupported else []
        ks = [0] if m.group(2) == "header" else [k for k in checks if k >= 1]
        if not ks:
            results.append([r[0], fn_name, cls, status, "no-account-argument", "", ""])
            continue
        if path in unsupported:
            results.append([r[0], fn_name, cls, status, "unsupported", "", unsupported[path][:80]])
            continue
        if s is not None and all(k in s["est"] for k in ks):
            res = "est"
        elif s is not None and all(k in s["est"] or k in s["pres"] for k in ks):
            res = "pres"
        else:
            res = "open"
        if res != "open":
            need.add(path)
        results.append([r[0], fn_name, cls, status, res, mod_name(fn_name) if res != "open" else "", ""])

    # transitive closure of certified summaries used
    work = list(need)
    while work:
        p = work.pop()
        for q in summ[p]["uses"]:
            if q in summ and q not in need:
                need.add(q)
                work.append(q)

    os.makedirs(a.gen, exist_ok=True)
    mods = []
    for path in sorted(need):
        fn = fns[path]
        s = summ[path]
        ns = mod_name(fn.name)
        info, lean = X.run_one(fn.hdr_body[0], fn.hdr_body[1], summ, s["est"], s["pres"], "", ns, a.pin,
                               fn_mir_out=os.path.join(a.mirdir, fn.name + ".mir"))
        assert not info["open_returns"], (path, info)
        open(os.path.join(a.gen, ns + ".lean"), "w").write(lean)
        mods.append(ns)

    # ---- T15: the full resolved close ----
    t15 = fns[P + "close_resolved_account_not_atomic"]
    lines = span_lines(a.spans_mir, "close_resolved_account_not_atomic")
    t15_info = {}
    for ns, req in (("T15_header_checked_paths", [0]), ("T15_account_checked_paths", [1])):
        info, lean = X.run_one(t15.hdr_body[0], t15.hdr_body[1], summ, req, [], "auto", ns, a.pin,
                               fn_mir_out=os.path.join(a.mirdir, t15.name + ".mir"))
        assert not info["open_returns"], info
        info["exempt_lines"] = {str(b): lines.get(b) for b in info["exempt"]}
        open(os.path.join(a.gen, ns + ".lean"), "w").write(lean)
        mods.append(ns)
        t15_info[ns] = info

    # conditional form: which callee contracts (preservation of the check) close the whole row?
    def callee_paths(fn):
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

    for ns, k in (("T15_header_conditional", 0), ("T15_account_conditional", 1)):
        cands = [p for p in callee_paths(t15)
                 if not (p in summ and (k in summ[p]["est"] or k in summ[p]["pres"]))
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
            info, lean = X.run_one(t15.hdr_body[0], t15.hdr_body[1], tab, [], [k], "", ns, a.pin)
            return info, lean

        assumed = list(cands)
        info, lean = attempt(assumed)
        if info["open_returns"]:
            t15_info[ns] = {"result": "not closed even with every callee contract assumed",
                            "candidates": cands, "open_ok_sites": info["open_ok_sites"]}
            continue
        for p in list(cands):
            trial = [q for q in assumed if q != p]
            i2, l2 = attempt(trial)
            if not i2["open_returns"]:
                assumed, info, lean = trial, i2, l2
        open(os.path.join(a.gen, ns + ".lean"), "w").write(lean)
        mods.append(ns)
        info["assumed_callee_contracts"] = [p[len(P):] for p in assumed]
        t15_info[ns] = info
        for b, p, st in info["summaries_used"]:
            if st == "certified" and p not in need:
                # certified summaries used here must have their own module
                fn = fns[p]
                s = summ[p]
                nsq = mod_name(fn.name)
                i3, l3 = X.run_one(fn.hdr_body[0], fn.hdr_body[1], summ, s["est"], s["pres"], "", nsq, a.pin,
                                   fn_mir_out=os.path.join(a.mirdir, fn.name + ".mir"))
                assert not i3["open_returns"]
                open(os.path.join(a.gen, nsq + ".lean"), "w").write(l3)
                need.add(p)
                mods.append(nsq)
                work = [p]
                while work:
                    x = work.pop()
                    for q in summ[x]["uses"]:
                        if q in summ and q not in need:
                            need.add(q)
                            work.append(q)
                            fq = fns[q]
                            i4, l4 = X.run_one(fq.hdr_body[0], fq.hdr_body[1], summ, summ[q]["est"],
                                               summ[q]["pres"], "", mod_name(fq.name), a.pin,
                                               fn_mir_out=os.path.join(a.mirdir, fq.name + ".mir"))
                            open(os.path.join(a.gen, mod_name(fq.name) + ".lean"), "w").write(l4)
                            mods.append(mod_name(fq.name))

    json.dump(t15_info, open(os.path.join(a.out, "t15.json"), "w"), indent=1, sort_keys=True)
    with open(os.path.join(a.out, "ledger_results.tsv"), "w") as f:
        f.write("row\tfunction\tledger_class\tledger_status\ttool_result\tlean_module\tnote\n")
        for r in results:
            f.write("\t".join(r) + "\n")
    mods = sorted(set(mods))
    with open(os.path.join(os.path.dirname(a.gen.rstrip("/")), "All.lean"), "w") as f:
        f.write("-- GENERATED by ledger_run.py: imports every generated module and prints the axioms of each theorem.\n")
        for m in mods:
            f.write("import MirFlow.Gen.%s\n" % m)
        f.write("\n")
        for m in mods:
            f.write("#print axioms MirFlow.Gen.%s.main\n" % m)
    json.dump({"modules": mods, "summary_graph_cyclic": cyclic, "rounds": rounds,
               "methods": len(fns), "summaries": len(summ), "unsupported": unsupported},
              open(os.path.join(a.out, "run.json"), "w"), indent=1, sort_keys=True)
    print("modules: %d; summary graph cyclic: %s" % (len(mods), cyclic))


if __name__ == "__main__":
    main()
