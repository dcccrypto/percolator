#!/usr/bin/env python3
"""Read-set of the runtime checks: which top-level fields of the market header, and whether the
contents of the market slots, a check can read. TRUSTED (rule R10 of RULES.md uses its result).

Method: type-directed, flow-insensitive, conservative. Inside every function reachable from the
check, a local whose declared type is a reference to the header, a market view (by value or by
reference) or the market slice is a *carrier*. A carrier may only be used in the listed forms;
any other use makes the answer ALL. A header field counts as read as soon as a place
`(*hdr).N` appears anywhere (read, borrowed or written). A call that is given a carrier must
resolve to exactly one MIR body, which is then analysed too; otherwise ALL.
"""
import re
import sys
import os

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "novel", "mirflow"))
import extract as X  # noqa: E402

ALL = "ALL"
HDR = re.compile(r"^&(mut )?(v16::)?MarketGroupV16HeaderAccount$")
VIEWREF = re.compile(r"^&(mut )?(v16::)?MarketGroupV16View(Mut)?<'_, T>$")
VIEWVAL = re.compile(r"^(v16::)?MarketGroupV16View(Mut)?<'_, T>$")
MKT = re.compile(r"^&(mut )?\[(v16::)?Market<T>\]$")
SUSPECT = re.compile(r"MarketGroupV16HeaderAccount|MarketGroupV16View|\bMarket<")


def kind(t):
    if t is None:
        return "other"
    if HDR.match(t):
        return "hdr"
    if VIEWREF.match(t):
        return "viewref"
    if VIEWVAL.match(t):
        return "viewval"
    if MKT.match(t):
        return "mkt"
    if "{closure@" in t:
        return "clo"
    if re.search(r"MarketGroupV16HeaderAccount|MarketGroupV16View", t):
        return "suspect"
    if re.search(r"\bMarket<", t):
        return "mktany"     # a reference to (part of) the slot array: slot contents, never the header
    return None


def index(mir_path, src_path):
    """(TypeName or '', fn name) -> [Fn]; inherent impls only (a trait impl is keyed apart)."""
    lines = open(src_path).read().split("\n")
    out = {}
    for hdr, body in X.parse_mir(mir_path):
        cm = re.match(r"^fn v16::.*::\{closure#\d+\}\(_1: &?(?:mut )?(\{closure@[^}]*\})", hdr)
        m = re.match(r"^fn v16::<impl at src/v16\.rs:(\d+):\d+: \d+:\d+>::(\w+)\(", hdr)
        if cm:
            key = ("<closure>", cm.group(1))
        elif m:
            ln = lines[int(m.group(1)) - 1]
            im = re.match(r"^impl(?:<[^>]*>)? (\w+)(?:<[^{]*>)? \{", ln)
            key = (im.group(1), m.group(2)) if im and " for " not in ln else ("<trait>", m.group(2))
        else:
            m = re.match(r"^fn v16::(\w+)\(", hdr)
            if not m:
                continue
            key = ("", m.group(1))
        try:
            fn = X.Fn(hdr, body)
        except X.Unsupported:
            fn = None
        out.setdefault(key, []).append(fn)
    return out


def call_key(fnpath):
    """`Type::<..>::name::<..>` -> (Type, name); `name` / `v16::name` -> ('', name); else None."""
    p = fnpath
    if p.startswith("v16::"):
        p = p[5:]
    m = re.fullmatch(r"(\w+)", p)
    if m:
        return ("", m.group(1))
    m = re.match(r"^(\w+)(::<)?", p)
    if not m or p.startswith("<"):
        return None
    rest = p[len(m.group(1)):]
    if rest.startswith("::<"):
        k = X._close_angle(rest, 2)
        if k < 0:
            return None
        rest = rest[k + 1:]
    m2 = re.fullmatch(r"::(\w+)(::<.*>)?", rest)
    if not m2:
        return None
    return (m.group(1), m2.group(1))


def read_set(idx, start):
    """start: list of (Type, name). Returns (fields: set or ALL, markets_contents: bool, why, functions)."""
    fields, mk, why = set(), False, []
    todo, done = list(start), set()
    while todo:
        key = todo.pop()
        if key in done:
            continue
        done.add(key)
        fl = idx.get(key, [])
        if len(fl) != 1 or fl[0] is None:
            return ALL, True, ["cannot resolve %s::%s to one MIR body (%d)" % (key[0], key[1], len(fl))], done
        fn = fl[0]
        car = {l: kind(t) for l, t in fn.types.items() if kind(t)}
        if any(k == "mktany" for k in car.values()):
            mk = True
        car = {l: k for l, k in car.items() if k != "mktany"}
        for b, (stmts, term, _) in fn.blocks.items():
            for s in stmts + [term]:
                ls = [l for l in X.locals_in(s) if l in car]
                if not ls:
                    continue
                if any(car[l] == "suspect" for l in ls):
                    return ALL, True, ["%s bb%d: value of a type that mentions the market: %s" % (key[1], b, s[:90])], done
                if re.match(r"^Storage(Live|Dead)\(_\d+\);$", s):
                    continue
                t = s
                # header field accesses
                for m in re.finditer(r"\(\*_(\d+)\)\.(\d+): ", s):
                    if car.get(int(m.group(1))) == "hdr":
                        fields.add(int(m.group(2)))
                t = re.sub(r"\(\*_(\d+)\)\.(\d+): ", lambda m: "(HDRFIELD: " if car.get(int(m.group(1))) == "hdr" else m.group(0), t)
                # market slice: length only, or contents
                t2 = t
                for m in re.finditer(r"(?:Len\(\(\*_(\d+)\)\)|PtrMetadata\((?:copy|move) _(\d+)\))", t):
                    l = int(m.group(1) or m.group(2))
                    if car.get(l) == "mkt":
                        t2 = t2.replace(m.group(0), "LEN")
                t = t2
                rest = [l for l in X.locals_in(t) if l in car]
                if not rest:
                    continue
                # closures: one that captures a carrier has its body analysed, however it is used
                cm = re.fullmatch(r"_(\d+) = (\{closure@[^}]*\})(?: \{ (.*) \})?;", s)
                if cm and car.get(int(cm.group(1))) == "clo":
                    caps = X.split_top(cm.group(3)) if cm.group(3) else []
                    if all(re.fullmatch(r"\w+: (?:copy|move) _\d+", c) or re.fullmatch(r"\w+: &(?:mut )?_\d+", c) for c in caps):
                        if any(re.search(r"&(?:mut )?_\d+$", c) and car.get(X.locals_in(c)[0]) for c in caps):
                            return ALL, True, ["%s bb%d: closure captures a carrier by reference" % (key[1], b)], done
                        if any(l in car for c in caps for l in X.locals_in(c)):
                            todo.append(("<closure>", cm.group(2)))
                        continue
                    return ALL, True, ["%s bb%d: closure creation not understood: %s" % (key[1], b, s[:90])], done
                cap = re.fullmatch(r"_(\d+) = (?:no_retag )?(?:copy|move) \((?:_(\d+)|\(\*_(\d+)\))\.\d+: (.*)\);", s)
                if cap and car.get(int(cap.group(2) or cap.group(3))) == "clo":
                    if kind(cap.group(4)) == car.get(int(cap.group(1))) and kind(cap.group(4)) != "suspect":
                        continue
                    return ALL, True, ["%s bb%d: capture read not understood: %s" % (key[1], b, s[:90])], done
                if all(car[l] == "clo" for l in rest):
                    continue
                if s is term and " -> " in s and X.split_assign(s.rsplit(" -> ", 1)[0]) is not None:
                    dest, rhs = X.split_assign(t.rsplit(" -> ", 1)[0])
                    try:
                        fnpath, args = X.split_call(rhs)
                    except X.Unsupported:
                        return ALL, True, ["%s bb%d: call not parsed" % (key[1], b)], done
                    if any(l in car for l in X.locals_in(fnpath)):
                        return ALL, True, ["%s bb%d: carrier in a callee expression" % (key[1], b)], done
                    given = [l for a in args for l in X.locals_in(a) if l in car]
                    if any(car[l] == "mkt" for l in given):
                        mk = True
                    given = [l for l in given if car[l] not in ("mkt", "clo")]
                    if given:
                        for a in args:
                            la = [l for l in X.locals_in(a) if l in car and car[l] not in ("mkt", "clo")]
                            if la and not re.fullmatch(r"(?:copy|move) _\d+", a.strip()):
                                return ALL, True, ["%s bb%d: carrier passed inside an expression: %s" % (key[1], b, a[:60])], done
                        ck = call_key(fnpath)
                        if ck is None:
                            return ALL, True, ["%s bb%d: carrier passed to %s" % (key[1], b, fnpath[:80])], done
                        todo.append(ck)
                    continue
                sa = X.split_assign(s) if s.endswith(";") else None
                if sa is None:
                    return ALL, True, ["%s bb%d: carrier in %s" % (key[1], b, s[:90])], done
                place, rhs = sa
                r = rhs[len("no_retag "):] if rhs.startswith("no_retag ") else rhs
                pl = X.bare_local(place)
                ok = False
                if pl is not None and pl in car:
                    m = re.fullmatch(r"(?:copy|move) \(\(\*_(\d+)\)\.([01]): (.*)\)", r) or re.fullmatch(r"(?:copy|move) \(_(\d+)\.([01]): (.*)\)", r)
                    if m and car.get(int(m.group(1))) in ("viewref", "viewval"):
                        ok = (m.group(2) == "0" and car[pl] == "hdr") or (m.group(2) == "1" and car[pl] == "mkt")
                    m = re.fullmatch(r"(?:copy|move) _(\d+)", r) or re.fullmatch(r"&(?:mut )?\(\*_(\d+)\)", r)
                    if m and car.get(int(m.group(1))) == car[pl]:
                        ok = True
                    m = re.fullmatch(r"&(?:mut )?_(\d+)", r)
                    if m and car.get(int(m.group(1))) == "viewval" and car[pl] == "viewref":
                        ok = True
                    m = re.fullmatch(r"MarketGroupV16View::<'_, T> \{ header: (?:copy|move) _(\d+), markets: (?:copy|move) _(\d+) \}", r)
                    if m and car.get(int(m.group(1))) == "hdr" and car.get(int(m.group(2))) == "mkt" and car[pl] == "viewval":
                        ok = True
                if not ok:
                    if all(car[l] == "mkt" for l in rest):
                        mk = True       # any other use of the slice pointer: contents may be read
                        continue
                    return ALL, True, ["%s bb%d: carrier in %s" % (key[1], b, s[:110])], done
        why.append("%s::%s" % key)
    return fields, mk, why, done


def header_fields(src_path):
    lines = open(src_path).read().split("\n")
    out, inside = [], False
    for ln in lines:
        if ln.startswith("pub struct MarketGroupV16HeaderAccount"):
            inside = True
            continue
        if inside:
            if ln.startswith("}"):
                break
            m = re.match(r"^    pub (\w+):", ln)
            if m:
                out.append(m.group(1))
    return out


def compute(mir_path, src_path):
    idx = index(mir_path, src_path)
    r0 = read_set(idx, [("MarketGroupV16ViewMut", "validate_shape")])
    r1 = read_set(idx, [("PortfolioV16ViewMut", "validate_with_market"), ("PortfolioV16View", "validate_with_market")])
    return {"names": header_fields(src_path), "shape": r0, "account": r1}


if __name__ == "__main__":
    res = compute(sys.argv[1], sys.argv[2])
    names = res["names"]
    for k in ("shape", "account"):
        f, mk, why, done = res[k]
        print("== %s: functions analysed: %s" % (k, sorted("%s::%s" % x for x in done)))
        if f == ALL:
            print("   ALL:", why)
        else:
            print("   header fields read (%d of %d): %s" % (len(f), len(names), ", ".join("%d:%s" % (i, names[i] if i < len(names) else "?") for i in sorted(f))))
            print("   NOT read: %s" % ", ".join("%d:%s" % (i, n) for i, n in enumerate(names) if i not in f))
            print("   market slot contents may be read: %s" % mk)
