#!/usr/bin/env python3
"""mirflow extractor: rustc MIR text -> abstract IR (MirFlow.Core) + certificate -> Lean file.

TRUSTED PART: `translate` (MIR text -> IR). Rule of construction: anything not recognised either
aborts the extraction of the function (unknown statement/terminator) or is translated as the most
permissive instruction ("may write everything", "value unknown").

UNTRUSTED PART: `fixpoint` (computes the certificate). If it is wrong, Lean's `checkCert` evaluates
to false and the generated theorem does not compile.

The rules are documented in ../METHOD-RULES.md; keep the two in step.
"""
import argparse
import hashlib
import json
import re
import sys

ALL = "*"  # provenance: may point anywhere

MARKET_T = re.compile(r"^&(mut )?(v16::)?MarketGroupV16ViewMut<")
ACCT_MUT_T = re.compile(r"^&mut (v16::)?PortfolioV16ViewMut<")
ACCT_VIEW_T = re.compile(r"^&(v16::)?PortfolioV16View<")
# By-value types that hold only shared references (struct definitions checked by check_assumptions.py).
READONLY_BYVALUE = re.compile(r"^(v16::)?(PortfolioV16View<'_>|MarketGroupV16View<'_, T>)$")

SHAPE_FN = "MarketGroupV16ViewMut::<'_, T>::validate_shape"
MARKET_AS_VIEW_FN = "MarketGroupV16ViewMut::<'_, T>::as_view"
ACCT_CHECK_FN = {
    "mut": "PortfolioV16ViewMut::<'_>::validate_with_market::<T>",
    "view": "PortfolioV16View::<'_>::validate_with_market::<T>",
}
BRANCH_RE = re.compile(r"^<Result<.*> as Try>::branch$")
RESIDUAL_RE = re.compile(r"^<Result<.*> as FromResidual<Result<Infallible, .*>>>::from_residual$")
RESULT_VARIANT_RE = re.compile(r"^(?:const )?(?:[\w]+::)*Result::<.*?>::(Ok|Err)\b")


class Unsupported(Exception):
    pass


def is_plain(t):
    """A type that cannot carry a pointer into the world."""
    return not any(x in t for x in ("&", "'", "*", "{", "dyn ", "fn(", "impl "))


def is_shared_ref(t):
    return t.startswith("&") and not t.startswith("&mut ") and not re.match(r"^&'\w+ mut ", t)


def cannot_write(t):
    return is_plain(t) or is_shared_ref(t) or bool(READONLY_BYVALUE.match(t))


def split_top(s, sep=","):
    out, depth, cur, i = [], 0, "", 0
    while i < len(s):
        c = s[i]
        if c in "([{<":
            depth += 1
        elif c in ")]}":
            depth -= 1
        elif c == ">" and not (i > 0 and s[i - 1] in "-="):
            depth -= 1
        if c == sep and depth == 0:
            out.append(cur.strip())
            cur = ""
        else:
            cur += c
        i += 1
    if cur.strip():
        out.append(cur.strip())
    return out


def locals_in(s):
    return [int(x) for x in re.findall(r"(?<![\w.])_(\d+)\b", s)]


def bare_local(s):
    m = re.fullmatch(r"_(\d+)", s.strip())
    return int(m.group(1)) if m else None


def strip_operand(op):
    """'copy X' / 'move X' / 'no_retag copy X' -> X ; None for constants."""
    op = op.strip()
    for p in ("no_retag copy ", "no_retag move ", "copy ", "move "):
        if op.startswith(p):
            return op[len(p):].strip()
    if op.startswith("const "):
        return None
    if not locals_in(op):
        return None  # a function item or other constant printed without `const`
    raise Unsupported("operand: " + op)


class Fn:
    def __init__(self, header, body_lines):
        self.header = header
        m = re.match(r"^fn (.*?)\((.*)\) -> (.*) \{$", header)
        if not m:
            raise Unsupported("fn header: " + header)
        self.path, args, self.ret = m.group(1), m.group(2), m.group(3)
        self.name = self.path.split("::")[-1]
        self.types = {}
        self.args = []
        for a in split_top(args):
            am = re.match(r"^(?:mut )?_(\d+): (.*)$", a)
            if not am:
                raise Unsupported("arg: " + a)
            self.types[int(am.group(1))] = am.group(2)
            self.args.append(int(am.group(1)))
        self.blocks = {}  # id -> (stmts[str], term str, cleanup bool)
        cur = None
        for ln in body_lines:
            s = ln.strip()
            lm = re.match(r"^let (?:mut )?_(\d+): (.*);$", s)
            if cur is None and lm:
                self.types[int(lm.group(1))] = lm.group(2)
                continue
            bm = re.match(r"^bb(\d+)( \(cleanup\))?: \{$", s)
            if bm:
                cur = int(bm.group(1))
                self.blocks[cur] = ([], None, bool(bm.group(2)))
                continue
            if cur is not None:
                if s == "}":
                    cur = None
                    continue
                if not s or s.startswith("//"):
                    continue
                self.blocks[cur][0].append(s)
        for b, (st, _, cl) in list(self.blocks.items()):
            if not st:
                raise Unsupported("empty block bb%d" % b)
            self.blocks[b] = (st[:-1], st[-1], cl)
        if sorted(self.blocks) != list(range(len(self.blocks))):
            raise Unsupported("block numbering")


def parse_mir(path):
    fns, cur, hdr = [], None, None
    with open(path) as f:
        for ln in f:
            ln = ln.rstrip("\n")
            if cur is None:
                if ln.startswith("fn ") and ln.endswith("{"):
                    hdr, cur = ln, []
            else:
                if ln == "}":
                    fns.append((hdr, cur))
                    cur = None
                else:
                    cur.append(ln)
    return fns


def split_assign(s):
    """'PLACE = RHS;' -> (PLACE, RHS) or None. Splits at the first top-level ' = '."""
    depth = 0
    for i, c in enumerate(s):
        if c in "([{":
            depth += 1
        elif c in ")]}":
            depth -= 1
        elif depth == 0 and s.startswith(" = ", i):
            return s[:i], s[i + 3:].rstrip(";")
    return None


def split_call(rhs):
    """'FN(ARGS)' -> (FN, [args]). The argument list is the last balanced paren group."""
    if not rhs.endswith(")"):
        raise Unsupported("call: " + rhs)
    depth = 0
    for i in range(len(rhs) - 1, -1, -1):
        c = rhs[i]
        if c == ")":
            depth += 1
        elif c == "(":
            depth -= 1
            if depth == 0:
                return rhs[:i], split_top(rhs[i + 1:-1])
    raise Unsupported("call parens: " + rhs)


IGNORED_STMT = re.compile(
    r"^(StorageLive\(|FakeRead\(|PlaceMention\(|AscribeUserType\(|Coverage::|Retag\(|"
    r"ConstEvalCounter|nop;|Nop;|assume\(|BackwardIncompatibleDropHint\()")


class Translator:
    def __init__(self, fn, summaries=None):
        self.fn = fn
        self.summaries = summaries or {}
        self.notes = []
        self.used_summaries = []
        t = fn.types
        # roots and checks
        self.market = None
        self.accounts = []  # (local, kind)
        for a in fn.args:
            if MARKET_T.match(t[a]) and self.market is None:
                self.market = a
            elif ACCT_MUT_T.match(t[a]):
                self.accounts.append((a, "mut"))
            elif ACCT_VIEW_T.match(t[a]):
                self.accounts.append((a, "view"))
        self.roots = [a for a in fn.args if not is_plain(t[a])]
        # check k -> set of roots it reads
        self.deps = {}
        if self.market is not None:
            self.deps[0] = {self.market}
            for i, (a, _) in enumerate(self.accounts):
                self.deps[i + 1] = {self.market, a}
        self.checks = sorted(self.deps)
        self.fresh = 100000
        self._index()

    # ---- whole-function indexes (flow-insensitive) ----
    def _index(self):
        fn = self.fn
        self.assigns = {}   # local -> list of rhs strings (whole-local assignments, incl. call dests)
        self.partial = set()  # locals assigned through a projection (no deref)
        self.escaped = set()  # locals whose address is taken mutably / raw
        for b, (stmts, term, _) in fn.blocks.items():
            for s in stmts + [term]:
                sa = split_assign(s.rsplit(" -> ", 1)[0] if s is term else s)
                if sa is None:
                    continue
                place, rhs = sa
                bl = bare_local(place)
                if bl is not None:
                    self.assigns.setdefault(bl, []).append(rhs)
                elif "*" not in place:
                    ls = locals_in(place)
                    if ls:
                        self.partial.add(ls[0])
                if rhs.startswith("&mut ") or rhs.startswith("&raw ") or rhs.startswith("&two_phase "):
                    pl = rhs.split(" ", 2)[-1] if rhs.startswith("&raw ") else rhs.split(" ", 1)[1]
                    if "*" not in pl:
                        ls = locals_in(pl)
                        if ls:
                            self.escaped.add(ls[0])
        for r in self.roots:
            if r in self.assigns or r in self.partial or r in self.escaped:
                raise Unsupported("argument _%d is reassigned or mutably borrowed" % r)
        self._prov = {}

    def prov(self, l, seen=()):
        """Set of roots (or ALL) that pointers held in local l may be derived from."""
        if l in self._prov:
            return self._prov[l]
        t = self.fn.types.get(l)
        if t is None:
            return {ALL}
        if l in self.fn.args:
            r = set() if is_plain(t) else {l}
        elif is_plain(t):
            r = set()
        elif l in seen or l in self.partial or l in self.escaped or len(self.assigns.get(l, [])) != 1:
            r = {ALL}
        else:
            r = set()
            for m in locals_in(self.assigns[l][0]):
                r |= self.prov(m, seen + (l,))
        if not seen:
            self._prov[l] = r
        return r

    def same_for(self, S):
        if ALL in S:
            return []
        return [k for k in self.checks if not (self.deps[k] & S)]

    def resolves_to(self, l, root, depth=0):
        """Local l always holds a reference to *root (a reborrow chain of single assignments)."""
        if l == root:
            return True
        if depth > 8 or l in self.partial or l in self.escaped or len(self.assigns.get(l, [])) != 1:
            return False
        rhs = self.assigns[l][0]
        m = re.fullmatch(r"(?:&|&mut |copy |move )\(\*_(\d+)\)", rhs) or re.fullmatch(r"(?:copy|move) _(\d+)", rhs)
        return bool(m) and self.resolves_to(int(m.group(1)), root, depth + 1)

    def new_local(self):
        self.fresh += 1
        return self.fresh

    def src(self, m, stmts):
        """A local that may be read as a tracked value; escaped locals are replaced by a havoc'd temp."""
        if m in self.escaped:
            t = self.new_local()
            stmts.append(("havoc", t))
            return t
        return m

    def assign_local(self, l, rhs, stmts):
        if l in self.escaped:
            return
        rhs = rhs.strip()
        m = re.fullmatch(r"(?:no_retag )?(?:copy|move) _(\d+)", rhs) or re.fullmatch(r"discriminant\(_(\d+)\)", rhs)
        if m:
            stmts.append(("copy", l, self.src(int(m.group(1)), stmts)))
            return
        rm = RESULT_VARIANT_RE.match(rhs)
        if rm:
            stmts.append(("setD", l, 0 if rm.group(1) == "Ok" else 1))
            return
        cm = re.fullmatch(r"const (\d+)_(?:u8|u16|u32|u64|u128|usize|i8|i16|i32|i64|i128|isize)", rhs)
        if cm and int(cm.group(1)) < 2 ** 62:
            stmts.append(("setD", l, int(cm.group(1))))
            return
        if rhs in ("const true", "const false"):
            stmts.append(("setD", l, 1 if rhs == "const true" else 0))
            return
        stmts.append(("havoc", l))

    def store(self, place, stmts):
        """Effects of writing to PLACE (not a bare local)."""
        ls = locals_in(place)
        if not ls:
            raise Unsupported("place: " + place)
        base = ls[0]
        if "*" in place:
            same = self.same_for(self.prov(base))
            if same != self.checks:
                stmts.append(("write", same))
        else:
            if base not in self.escaped:
                stmts.append(("havoc", base))

    def stmt(self, s, stmts):
        if IGNORED_STMT.match(s):
            return
        m = re.fullmatch(r"StorageDead\(_(\d+)\);", s)
        if m:
            if int(m.group(1)) not in self.escaped:
                stmts.append(("havoc", int(m.group(1))))
            return
        m = re.fullmatch(r"Deinit\((.*)\);", s)
        if m:
            bl = bare_local(m.group(1))
            if bl is not None:
                if bl not in self.escaped:
                    stmts.append(("havoc", bl))
            else:
                self.store(m.group(1), stmts)
            return
        m = re.fullmatch(r"discriminant\((.*)\) = (\d+);", s)
        if m:
            bl = bare_local(m.group(1))
            if bl is not None:
                if bl not in self.escaped:
                    stmts.append(("setD", bl, int(m.group(2))))
            else:
                self.store(m.group(1), stmts)
            return
        sa = split_assign(s)
        if sa is None or not s.endswith(";"):
            raise Unsupported("statement: " + s)
        place, rhs = sa
        bl = bare_local(place)
        if bl is not None:
            self.assign_local(bl, rhs, stmts)
        else:
            self.store(place, stmts)

    def call(self, b, dest, fnpath, args, target, stmts, extra_blocks):
        """Returns the terminator for a call; may append a synthetic block for a projected destination."""
        fn = self.fn
        post = []
        dl = bare_local(dest)
        if dl is None or dl in self.escaped:
            if dl is None:
                self.store(dest, post)
            dl = self.new_local()
        if post:
            nb = len(fn.blocks) + len(extra_blocks)
            extra_blocks.append((post, ("goto", target)))
            target = nb
        places = [strip_operand(a) for a in args]
        arg_locals = [bare_local(p) if p is not None else None for p in places]

        if BRANCH_RE.match(fnpath) and len(args) == 1 and arg_locals[0] is not None:
            return ("branch", dl, self.src(arg_locals[0], stmts), target)
        if RESIDUAL_RE.match(fnpath) and len(args) == 1:
            return ("residual", dl, target)

        # what the call may write, by the types of what it is given
        S = set()
        for p in places + [None if not locals_in(fnpath) else fnpath]:
            if p is None:
                continue
            if p is fnpath:  # call through a local function value / closure
                for l in locals_in(p):
                    S |= self.prov(l)
                continue
            l = bare_local(p)
            if l is not None:
                t = fn.types[l]
                if not cannot_write(t):
                    S |= self.prov(l)
            else:
                tm = re.search(r": ([^:]*)\)$", p)
                t = tm.group(1) if tm else "&mut ?"
                ls = locals_in(p)
                if not cannot_write(t):
                    for l2 in ls[:1]:
                        S |= self.prov(l2)
                    if not ls:
                        S |= {ALL}
        same = self.same_for(S)
        pres, est, uncond = [], [], False

        # the runtime checks
        if fnpath == SHAPE_FN and len(args) == 1 and arg_locals[0] is not None \
                and self.market is not None and self.resolves_to(arg_locals[0], self.market):
            assert same == self.checks
            est = [0]
        for i, (a, kind) in enumerate(self.accounts):
            if fnpath == ACCT_CHECK_FN[kind] and len(args) == 2 and None not in arg_locals \
                    and self.resolves_to(arg_locals[0], a) and self.view_of_market_now(b, arg_locals[1]):
                assert same == self.checks
                est = [i + 1]

        # callee summaries (each one is an assumption or a certificate of this tool; see summaries file)
        sm = self.summaries.get(fnpath)
        if sm is not None and not est and sm.get("direct"):
            # An ASSUMED contract stated directly about this call site and this function's checks.
            pres = [k for k in sm.get("pres", []) if k in self.checks and k not in same]
            uncond = bool(sm.get("uncond", False))
            if pres:
                self.used_summaries.append((b, fnpath, sm))
        elif sm is not None and not est:
            # The callee's own certificate speaks about its market argument and its account
            # arguments. It applies here if those are this function's market and accounts.
            roles = sm["args"]  # "market" | "account" | "other", one per argument
            ok = len(roles) == len(args)
            amap, nacc = {}, 0
            if ok:
                for role, l in zip(roles, arg_locals):
                    if role == "market":
                        if l is not None and self.market is not None and self.resolves_to(l, self.market):
                            amap[0] = 0
                    elif role == "account":
                        nacc += 1
                        hit = [i for i, (a, _) in enumerate(self.accounts)
                               if l is not None and self.resolves_to(l, a)]
                        if hit:
                            amap[nacc] = hit[0] + 1
                if 0 not in amap:
                    amap = {}
                else:
                    # an account check also needs the market to be ours (it is, amap[0] exists)
                    pass
                pres = sorted(set(amap[k] for k in sm.get("pres", []) if k in amap))
                est = sorted(set(amap[k] for k in sm.get("est", []) if k in amap))
                pres = [k for k in pres if k not in same and k not in est]
                uncond = bool(sm.get("uncond", False))
                if pres or est:
                    self.used_summaries.append((b, fnpath, sm))
        return ("call", dl, target, same, pres, est, uncond)

    def view_of_market_now(self, b, r):
        """Local r is `&_v` where _v is the result of `self.as_view()` computed by the unique
        predecessor of block b, with no possible write in between."""
        fn = self.fn
        if r in self.escaped or len(self.assigns.get(r, [])) != 1:
            return False
        m = re.fullmatch(r"&_(\d+)", self.assigns[r][0])
        if not m:
            return False
        v = int(m.group(1))
        if v in self.escaped or v in self.partial or len(self.assigns.get(v, [])) != 1:
            return False
        preds = [p for p in fn.blocks if b in self.succs(p)]
        if len(preds) != 1:
            return False
        p = preds[0]
        pst, pterm, _ = fn.blocks[p]
        sa = split_assign(pterm.rsplit(" -> ", 1)[0])
        if sa is None or bare_local(sa[0]) != v:
            return False
        try:
            fnpath, args = split_call(sa[1])
        except Unsupported:
            return False
        if fnpath != MARKET_AS_VIEW_FN or len(args) != 1:
            return False
        q = bare_local(strip_operand(args[0]) or "")
        if q is None or not self.resolves_to(q, self.market):
            return False
        # no store through a pointer in block b before the check call (calls end blocks)
        for s in fn.blocks[b][0]:
            sa2 = split_assign(s)
            if sa2 is not None and bare_local(sa2[0]) is None and "*" in sa2[0]:
                return False
        return True

    def succs(self, b):
        term = self.fn.blocks[b][1]
        return [int(x) for x in re.findall(r"\bbb(\d+)\b", term.rsplit(" -> ", 1)[1])] if " -> " in term else []

    def term(self, b, term, stmts, extra_blocks):
        if term == "return;":
            return ("ret",)
        if term in ("unreachable;", "resume;", "abort;") or term.startswith("terminate("):
            return ("stop",)
        m = re.fullmatch(r"goto -> bb(\d+);", term)
        if m:
            return ("goto", int(m.group(1)))
        m = re.fullmatch(r"switchInt\((.*)\) -> \[(.*)\];", term)
        if m:
            pl = strip_operand(m.group(1))
            l = bare_local(pl) if pl is not None else None
            if l is None:
                l = self.new_local()
                stmts.append(("havoc", l))
            else:
                l = self.src(l, stmts)
            cases, other = [], None
            for c in m.group(2).split(", "):
                k, t = c.split(": bb")
                if k == "otherwise":
                    other = int(t)
                else:
                    cases.append((int(k), int(t)))
            if any(k < 0 for k, _ in cases):
                # negative keys are not modelled: forget the value, keep every edge
                l2 = self.new_local()
                stmts.append(("havoc", l2))
                return ("switch", l2, [(i, t) for i, (_, t) in enumerate(cases)], other)
            return ("switch", l, cases, other)
        m = re.fullmatch(r"assert\(.*\) -> \[success: bb(\d+), unwind.*\];", term)
        if m:
            return ("goto", int(m.group(1)))
        m = re.fullmatch(r"drop\((.*)\) -> \[return: bb(\d+), unwind.*\];", term)
        if m:
            pl = m.group(1)
            l = bare_local(pl)
            ls = locals_in(pl)
            t = self.fn.types.get(l, "&mut ?") if l is not None else "&mut ?"
            if not is_plain(t):
                S = self.prov(ls[0]) if ls else {ALL}
                same = self.same_for(S)
                if same != self.checks:
                    stmts.append(("write", same))
            if l is not None and l not in self.escaped:
                stmts.append(("havoc", l))
            return ("goto", int(m.group(2)))
        if " -> " in term:
            head, tail = term.rsplit(" -> ", 1)
            sa = split_assign(head)
            if sa is not None:
                dest, rhs = sa
                fnpath, args = split_call(rhs)
                tm = re.match(r"\[return: bb(\d+), unwind", tail)
                if tm:
                    return self.call(b, dest, fnpath, args, int(tm.group(1)), stmts, extra_blocks)
                if tail.startswith("unwind"):
                    return ("stop",)  # diverging call
        raise Unsupported("terminator: " + term)

    def translate(self):
        fn = self.fn
        out, extra = [], []
        for b in range(len(fn.blocks)):
            stmts_txt, term_txt, cleanup = fn.blocks[b]
            stmts = []
            for s in stmts_txt:
                self.stmt(s, stmts)
            t = self.term(b, term_txt, stmts, extra)
            out.append((stmts, t))
        return out + extra


# ---------------- certificate (untrusted) ----------------

def get_av(av, l):
    return av.get(l)


def t_stmt(s, a):
    H, Pr, av = a
    if s[0] == "havoc":
        av = {k: v for k, v in av.items() if k != s[1]}
    elif s[0] == "write":
        return clobber(s[1], a)
    elif s[0] == "setD":
        av = {k: v for k, v in av.items() if k != s[1]}
        av[s[1]] = ("const", s[2])
    elif s[0] == "copy":
        v = av.get(s[2])
        av = {k: x for k, x in av.items() if k != s[1]}
        if v is not None:
            av[s[1]] = v
    return (H, Pr, av)


def stable(v, same):
    return v[0] == "const" or (all(k in same for k in v[1]) and all(k in same for k in v[2]))


def clobber(same, a):
    H, Pr, av = a
    return (frozenset(k for k in H if k in same), frozenset(k for k in Pr if k in same),
            {l: v for l, v in av.items() if stable(v, same)})


def t_term(t, a):
    H, Pr, av = a
    k = t[0]
    if k == "goto":
        return [(t[1], a)]
    if k == "switch":
        _, l, cases, other = t
        v = av.get(l)
        oe = [(other, a)] if other is not None else []
        if v is not None and v[0] == "const":
            for c, tb in cases:
                if c == v[1]:
                    return [(tb, a)]
            return oe
        if v is not None and v[0] == "cond":
            return [(tb, (H | frozenset(v[1]), Pr | frozenset(v[2]), av) if c == 0 else a) for c, tb in cases] + oe
        return [(tb, a) for _, tb in cases] + oe
    if k == "call":
        _, dst, tb, same, pres, est, uncond = t
        H1, Pr1, av1 = clobber(same, a)
        hs = tuple(est) + tuple(x for x in pres if x in H)
        ps = tuple(x for x in pres if x in Pr)
        avk = {l: v for l, v in av1.items() if l != dst}
        if uncond:
            return [(tb, (H1 | frozenset(hs), Pr1 | frozenset(ps), avk))]
        avk[dst] = ("cond", hs, ps)
        return [(tb, (H1, Pr1, avk))]
    if k == "branch":
        _, dst, srcl, tb = t
        v = av.get(srcl)
        avk = {l: x for l, x in av.items() if l != dst}
        if v is not None and v[0] == "const":
            avk[dst] = ("const", 0 if v[1] == 0 else 1)
        elif v is not None:
            avk[dst] = v
        return [(tb, (H, Pr, avk))]
    if k == "residual":
        _, dst, tb = t
        avk = {l: x for l, x in av.items() if l != dst}
        avk[dst] = ("const", 1)
        return [(tb, (H, Pr, avk))]
    return []


BOT = "bot"


def as_cond(state, v):
    """The conditional fact 'if the local is 0 then hs hold and ps are preserved' that follows
    from value fact v in `state` (mirrors Lean's avLe)."""
    H, Pr, _ = state
    if v[0] == "const":
        return BOT if v[1] != 0 else (frozenset(H), frozenset(Pr) | frozenset(H))
    return (frozenset(v[1]) | H, frozenset(v[2]) | frozenset(v[1]) | Pr | H)


def join(x, y):
    if x is None:
        return y
    H = x[0] & y[0]
    Pr = ((x[0] | x[1]) & (y[0] | y[1])) - H
    av = {}
    for l, v in x[2].items():
        w = y[2].get(l)
        if w is None:
            continue
        if v == w:
            av[l] = v
            continue
        cx, cy = as_cond(x, v), as_cond(y, w)
        if cx == BOT and cy == BOT:
            continue
        if cx == BOT:
            hs, ps = cy
        elif cy == BOT:
            hs, ps = cx
        else:
            hs, ps = cx[0] & cy[0], cx[1] & cy[1]
        ps = ps - hs
        if hs or ps:
            av[l] = ("cond", tuple(sorted(hs)), tuple(sorted(ps)))
    return (H, Pr, av)


def fixpoint(ir, reqP, exempt=()):
    inv = [None] * len(ir)
    inv[0] = (frozenset(), frozenset(reqP), {})
    work = [0]
    while work:
        b = work.pop()
        if b in exempt:
            continue
        a = inv[b]
        for s in ir[b][0]:
            a = t_stmt(s, a)
        for tb, a2 in t_term(ir[b][1], a):
            a2 = (a2[0], a2[1], {l: v for l, v in a2[2].items() if not (v[0] == "cond" and not v[1] and not v[2])})
            j = join(inv[tb], a2)
            if j != inv[tb]:
                inv[tb] = j
                work.append(tb)
    return inv


def out_state(ir, inv, b):
    a = inv[b]
    for s in ir[b][0]:
        a = t_stmt(s, a)
    return a


def open_ok_sites(ir, inv, req, reqP):
    """Blocks that set the return value to Ok (`_0 = Ok(..)`) in a state without the goal facts."""
    out = []
    for b, (st, t) in enumerate(ir):
        if inv[b] is None:
            continue
        a = inv[b]
        for s in st:
            a = t_stmt(s, a)
            if s == ("setD", 0, 0):
                H, Pr, _ = a
                if not (all(k in H for k in req) and all(k in Pr or k in H for k in reqP)):
                    out.append(b)
                    break
    return out


def ret_status(ir, inv, req, reqP, ok_only, exempt=()):
    """For each reachable return block: 'err', 'ok' (goal facts present) or 'open'."""
    res = {}
    for b, (st, t) in enumerate(ir):
        if t[0] != "ret" or inv[b] is None or b in exempt:
            continue
        H, Pr, av = out_state(ir, inv, b)
        v = av.get(0)
        if ok_only and v is not None and v[0] == "const" and v[1] != 0:
            res[b] = "err"
        elif all(k in H for k in req) and all(k in Pr or k in H for k in reqP):
            res[b] = "ok"
        elif ok_only and v is not None and v[0] == "cond" and all(k in v[1] or k in H for k in req) \
                and all(k in v[2] or k in v[1] or k in Pr or k in H for k in reqP):
            res[b] = "ok"
        else:
            res[b] = "open"
    return res


# ---------------- Lean output ----------------

def L(xs):
    return "[" + ", ".join(str(x) for x in xs) + "]"


def lean_stmt(s):
    if s[0] == "havoc":
        return ".havoc %d" % s[1]
    if s[0] == "write":
        return ".write %s" % L(s[1])
    if s[0] == "setD":
        return ".setD %d %d" % (s[1], s[2])
    return ".copy %d %d" % (s[1], s[2])


def lean_term(t):
    k = t[0]
    if k == "goto":
        return ".goto %d" % t[1]
    if k == "switch":
        return ".switch %d [%s] %s" % (t[1], ", ".join("(%d, %d)" % c for c in t[2]),
                                        "none" if t[3] is None else "(some %d)" % t[3])
    if k == "call":
        return ".call %d %d %s %s %s %s" % (t[1], t[2], L(t[3]), L(t[4]), L(t[5]), "true" if t[6] else "false")
    if k == "branch":
        return ".branch %d %d %d" % (t[1], t[2], t[3])
    if k == "residual":
        return ".residual %d %d" % (t[1], t[2])
    return "." + k


def lean_aval(v):
    if v[0] == "const":
        return ".const %d" % v[1]
    return ".cond %s %s" % (L(v[1]), L(v[2]))


def lean_state(a):
    if a is None:
        return "none"
    H, Pr, av = a
    return "some ⟨%s, %s, [%s]⟩" % (L(sorted(H)), L(sorted(Pr)),
                                    ", ".join("(%d, %s)" % (l, lean_aval(v)) for l, v in sorted(av.items())))


def emit_lean(ns, doc, ir, inv, req, reqP, exempt, ok_only, checks, decide_tac):
    o = []
    o.append("import MirFlow.Core\n")
    o.append("/-\n" + doc + "\n-/\n")
    o.append("namespace MirFlow.Gen.%s\n" % ns)
    o.append("open MirFlow\n")
    o.append("def cfg : Cfg := [")
    for i, (st, t) in enumerate(ir):
        o.append("  /- bb%d -/ ⟨[%s], %s⟩%s" % (i, ", ".join(lean_stmt(s) for s in st), lean_term(t),
                                                  "," if i + 1 < len(ir) else ""))
    o.append("]\n")
    o.append("def inv : List (Option AState) := [")
    for i, a in enumerate(inv):
        o.append("  /- bb%d -/ %s%s" % (i, lean_state(a), "," if i + 1 < len(inv) else ""))
    o.append("]\n")
    o.append("def goal : Goal := ⟨%s, %s, %s, %s⟩\n" % (L(req), L(reqP), L(exempt), "true" if ok_only else "false"))
    o.append("theorem cert : checkCert goal cfg inv = true := by %s\n" % decide_tac)
    concl = []
    for k in req:
        concl.append("chk %d w1 = true" % k)
    for k in reqP:
        concl.append("(chk %d w0 = true → chk %d w1 = true)" % (k, k))
    hyps = "(hret : Returns chk %s cfg w0 b w1 env1)" % L(exempt)
    if ok_only:
        hyps += " (hok : env1 0 = 0)"
    o.append("/-- Checks are numbered: 0 = `self.validate_shape()` returns Ok; k ≥ 1 =\n"
             "`account_k.validate_with_market(&self.as_view())` returns Ok. -/")
    if exempt:
        o.append("/- The claim is about executions that do not pass through block(s) %s. -/" % L(exempt))
    o.append("theorem main {W : Type} (chk : Nat → W → Bool) {w0 w1 : W} {b : Block} {env1 : Env}\n"
             "    %s :\n    %s := by" % (hyps, " ∧ ".join(concl) if concl else "True"))
    if not concl:
        o.append("  trivial")
    else:
        o.append("  have h := checkCert_sound chk cert hret %s" % (
            "(fun _ => hok)" if ok_only else "(fun h => by cases h)"))
        parts = []
        idx = 0
        for k in req:
            parts.append("h.1 %d (by simp [goal])" % k)
        for k in reqP:
            parts.append("h.2 %d (by simp [goal])" % k)
        o.append("  exact ⟨%s⟩" % ", ".join(parts) if len(parts) > 1 else "  exact %s" % parts[0])
    o.append("\nend MirFlow.Gen.%s" % ns)
    return "\n".join(o) + "\n"


def find_fn(fns, name, impl=None):
    hits = []
    for hdr, body in fns:
        m = re.match(r"^fn (.*?)\(", hdr)
        if not m:
            continue
        path = m.group(1)
        if path.split("::")[-1] == name and (impl is None or impl in path):
            hits.append((hdr, body))
    return hits


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mir", required=True)
    ap.add_argument("--fn", required=True)
    ap.add_argument("--impl", default=None, help="substring of the MIR path, to pick one impl")
    ap.add_argument("--ns", required=True)
    ap.add_argument("--req", default="", help="comma list of checks that must hold at Ok returns")
    ap.add_argument("--reqp", default="", help="comma list of checks that must be preserved")
    ap.add_argument("--exempt", default="", help="comma list of return blocks to exclude; 'auto' = every open one")
    ap.add_argument("--summaries", default=None)
    ap.add_argument("--out", default=None)
    ap.add_argument("--fn-mir-out", default=None, help="write the function's MIR text here")
    ap.add_argument("--decide", default="decide +kernel")
    ap.add_argument("--report", action="store_true")
    ap.add_argument("--pin", default="", help="free text recorded in the Lean file (revision, blob ids)")
    a = ap.parse_args()

    fns = parse_mir(a.mir)
    hits = find_fn(fns, a.fn, a.impl)
    if len(hits) != 1:
        print("ERROR: %d MIR bodies named %s" % (len(hits), a.fn), file=sys.stderr)
        for h, _ in hits:
            print("  " + h[:160], file=sys.stderr)
        sys.exit(2)
    hdr, body = hits[0]
    summaries = json.load(open(a.summaries)) if a.summaries else {}
    if "summaries" in summaries:
        summaries = summaries["summaries"]
    req = [int(x) for x in a.req.split(",") if x]
    reqP = [int(x) for x in a.reqp.split(",") if x]
    info, lean = run_one(hdr, body, summaries, req, reqP, a.exempt, a.ns, a.pin, a.decide, a.fn_mir_out)
    if a.report:
        print(json.dumps(info))
    if a.out:
        open(a.out, "w").write(lean)
    sys.exit(0 if not info["open_returns"] else 1)


def run_one(hdr, body, summaries, req, reqP, exempt_arg, ns, pin="", decide_tac="decide +kernel", fn_mir_out=None):
    text = hdr + "\n" + "\n".join(body) + "\n}\n"
    sha = hashlib.sha256(text.encode()).hexdigest()
    if fn_mir_out:
        open(fn_mir_out, "w").write(text)
    fn = Fn(hdr, body)
    tr = Translator(fn, summaries)
    ir = tr.translate()
    for k in req + reqP:
        if k not in tr.checks:
            raise Unsupported("check %d is not defined for this signature (checks: %s)" % (k, tr.checks))
    ok_only = fn.ret.startswith("Result<")
    inv = fixpoint(ir, reqP)
    sites = open_ok_sites(ir, inv, req, reqP)
    exempt = sites if exempt_arg == "auto" else [int(x) for x in exempt_arg.split(",") if x]
    inv = fixpoint(ir, reqP, exempt)
    status = ret_status(ir, inv, req, reqP, ok_only, exempt)
    open_rets = sorted(b for b, s in status.items() if s == "open")
    used = sorted(set((b, p, s.get("status", "certified")) for b, p, s in tr.used_summaries))
    info = {
        "fn": fn.path, "mir_sha256": sha, "blocks": len(fn.blocks), "ir_blocks": len(ir),
        "locals": len(fn.types), "market": tr.market, "accounts": tr.accounts, "checks": tr.checks,
        "returns": {str(k): v for k, v in sorted(status.items())}, "open_returns": open_rets,
        "open_ok_sites": sites, "exempt": exempt, "ok_only": ok_only,
        "summaries_used": [[b, p, st] for b, p, st in used], "notes": tr.notes,
    }
    doc = ("GENERATED by proofs/novel/mirflow/extract.py. Do not edit.\n"
           "function: %s\n%s\nsha256 of the function's MIR text: %s\n"
           "MIR blocks: %d, locals: %d. market root: _%s; accounts: %s\n"
           "callee summaries used (block, callee, status):\n%s"
           % (fn.path, pin, sha, len(fn.blocks), len(fn.types), tr.market,
              ", ".join("_%d (%s)" % x for x in tr.accounts) or "none",
              "\n".join("  bb%d %s [%s]" % u for u in used) or "  none"))
    lean = emit_lean(ns, doc, ir, inv, req, reqP, exempt, ok_only, tr.checks, decide_tac)
    return info, lean


if __name__ == "__main__":
    try:
        main()
    except Unsupported as e:
        print("UNSUPPORTED: %s" % e, file=sys.stderr)
        sys.exit(3)
