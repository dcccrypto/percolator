#!/usr/bin/env python3
"""For a function and a check k: where can check k be lost? Untrusted reporting aid (it reads the
translator's own logs); it proves nothing. Used to write the T15 work list.

Output per function, transitively through market-method callees that have no summary speaking
about check k: the header fields of the check's read-set that are written, writes through
pointers of unknown region, and calls outside the analysed methods that may write.
"""
import re
import sys
import os

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import run2 as R  # noqa: E402
E, B, RS = R.E, R.B, R.RS


def blame(prov, names, path, k, memo, depth=0):
    if path in memo:
        return memo[path]
    memo[path] = {"fields": set(), "unknown": set(), "external": set(), "via": set()}
    out = memo[path]
    try:
        r = prov.analyse(path)
    except E.X.Unsupported as e:
        out["unknown"].add("unsupported: %s" % str(e)[:60])
        return out
    tr = r["tr"]
    if k not in tr.checks:
        return out
    for b, place, reg, same in tr.write_log:
        if k in same:
            continue
        m = re.match(r"^\(*\(\*_\d+\)\.(\d+): ", place)
        if reg is not None and reg[0] == "hdr" and (reg[1] is not None or m):
            n = reg[1] if reg[1] is not None else int(m.group(1))
            out["fields"].add(names[n] if n < len(names) else "field %d" % n)
        elif reg == ("mkt",):
            out["fields"].add("<market slot contents>")
        else:
            roots = tr.prov(E.X.locals_in(place)[0])
            acct = [a for a, _ in tr.accounts]
            if roots and all(x in acct for x in roots if x != E.X.ALL) and E.X.ALL not in roots:
                out["fields"].add("<account memory>")
            else:
                out["unknown"].add("%s bb%d: %s" % (path.split("::")[-1], b, place[:50]))
    for b, fnpath, same, spoken in tr.call_log:
        if k in same or k in spoken:
            continue
        if fnpath in prov.fns:
            sub = blame(prov, names, fnpath, k, memo, depth + 1)
            out["via"].add(fnpath.split("::")[-1])
            for key in ("fields", "unknown", "external"):
                out[key] |= sub[key]
        else:
            out["external"].add(fnpath[:90])
    return out


def main():
    mir, src, k = sys.argv[1], sys.argv[2], int(sys.argv[3])
    pmir, psrc = os.environ["ENUM_PROBE_MIR"], os.environ["ENUM_PROBE_SRC"]
    fns, unsupported = B.load(mir, src)
    E.ENUMS.clear()
    E.ENUMS.update(E.enum_table(pmir, psrc))
    rs = RS.compute(mir, src)
    E.READ = {"fields": {0: rs["shape"][0], 1: rs["account"][0]}, "mk": {0: rs["shape"][1], 1: rs["account"][1]}}
    prov = R.Provider(fns)
    prov.unsupported.update(unsupported)
    memo = {}
    for name in sys.argv[4:]:
        o = blame(prov, rs["names"], R.P + name, k, memo)
        print("### `%s`, check %d\n" % (name, k))
        print("- writes that can change the check, by region: %s" % (", ".join("`%s`" % f for f in sorted(o["fields"])) or "none"))
        print("- through callees without a summary for this check: %s" % (", ".join("`%s`" % f for f in sorted(o["via"])) or "none"))
        print("- writes through a pointer of unknown region: %s" % ("; ".join(sorted(o["unknown"])) or "none"))
        print("- calls outside the market methods given something writable: %s\n" % ("; ".join("`%s`" % f for f in sorted(o["external"])) or "none"))


if __name__ == "__main__":
    main()
