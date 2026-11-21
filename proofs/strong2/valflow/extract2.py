#!/usr/bin/env python3
"""valflow extractor: rustc MIR text -> abstract IR of ClauseFlow.Core + certificate -> Lean file.

It extends the sibling extractor proofs/novel/mirflow/extract.py (imported as X, not edited).

TRUSTED PART: class `T2` (MIR text -> IR). The rules it adds to X.Translator are written in
../RULES.md (R1..R8); keep the two in step. Everything the base class does is unchanged.

UNTRUSTED PART: everything from `# ---- certificate` down. It mirrors the Lean checker to find a
certificate; if it is wrong, `checkCert` evaluates to false and the generated theorem does not
compile.
"""
import hashlib
import os
import re
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "novel", "mirflow"))
import extract as X  # noqa: E402

PO = 1000000        # payload ghost of local l is local PO + l            (rule R1)
ZBOT = 999999999    # a local no program mentions; used to write the unreachable state

INT_T = {"u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize", "bool"}
CONST_INT = re.compile(r"(?:const )?(\d+)_(?:u8|u16|u32|u64|u128|usize|i8|i16|i32|i64|i128|isize)")


def const_val(s):
    """Value of an integer/bool constant operand below 2^62, else None."""
    s = s.strip()
    m = CONST_INT.fullmatch(s)
    if m and int(m.group(1)) < 2 ** 62:
        return int(m.group(1))
    if s in ("const true", "true"):
        return 1
    if s in ("const false", "false"):
        return 0
    return None


def enum_variant(rhs, enum, variants):
    """Variant name if rhs is `[const ][core::<mod>::]ENUM::<ARGS>::VARIANT[(...)]` with ARGS the
    whole bracket-matched generic list and nothing after the variant's own parentheses."""
    r = rhs.strip()
    if r.startswith("const "):
        r = r[6:]
    for pfx in ("core::option::", "core::result::"):
        if r.startswith(pfx):
            r = r[len(pfx):]
    if not r.startswith(enum + "::<"):
        return None, None
    k = X._close_angle(r, len(enum) + 2)
    if k < 0:
        return None, None
    rest = r[k + 1:]
    for v in variants:
        if rest == "::" + v:
            return v, None
        if rest.startswith("::" + v + "(") and rest.endswith(")"):
            inner = rest[len(v) + 3:-1]
            # the parenthesis opened after the variant must close at the very end
            depth = 0
            for i, c in enumerate(rest[len(v) + 2:]):
                depth += c == "("
                depth -= c == ")"
                if depth == 0 and i != len(rest) - len(v) - 3:
                    return None, None
            return v, inner
    return None, None


def is_option_pred(fnpath, name):
    """Exactly `Option::<ARGS>::is_some` / `is_none` with ARGS bracket-matched."""
    if not fnpath.startswith("Option::<"):
        return False
    k = X._close_angle(fnpath, len("Option::"))
    return k > 0 and fnpath[k + 1:] == "::" + name


class T2(X.Translator):
    """provider: object with .summary(fnpath) -> None | {"args": roles, "cls": [clause]}"""

    def __init__(self, fn, provider=None):
        super().__init__(fn, {})
        self.provider = provider
        self.used2 = []   # (block, fnpath)
        self.cur = None   # (block, index of the statement being translated)

    def decl(self, l):
        return self.fn.types.get(l, "")

    def is_intlike(self, l):
        return self.decl(l) in INT_T

    # ---- R6: in-block copy propagation for a comparison operand
    def copy_origin(self, t):
        """If, in the current block before the current statement, the last assignment to _t is
        `_t = copy|move _s` and nothing after it assigns or kills _s or _t, return s; else t."""
        b, i = self.cur
        stmts = self.fn.blocks[b][0]
        for j in range(i - 1, -1, -1):
            s = stmts[j]
            ls = X.locals_in(s.split(" = ", 1)[0]) if " = " in s else X.locals_in(s)
            if s.startswith("StorageLive(") or X.IGNORED_STMT.match(s):
                continue
            sa = X.split_assign(s)
            if sa is not None and X.bare_local(sa[0]) == t:
                m = re.fullmatch(r"(?:copy|move) _(\d+)", sa[1].strip())
                if not m:
                    return t
                src = int(m.group(1))
                if src in self.escaped or t in self.escaped or not self.is_intlike(src):
                    return t
                # nothing between j and i may touch src or t
                for k in range(j + 1, i):
                    sk = stmts[k]
                    if X.IGNORED_STMT.match(sk) and not sk.startswith("StorageLive(_%d)" % src):
                        continue
                    lhs = sk.split(" = ", 1)[0] if " = " in sk else sk
                    if src in X.locals_in(lhs) or t in X.locals_in(lhs):
                        return t
                return src
            if t in ls:
                return t
        return t

    def assign_local(self, l, rhs, stmts):
        if l in self.escaped:
            return
        r = rhs.strip()
        # R2: read of the payload of variant 0
        m = re.fullmatch(r"(?:copy|move) \(\(_(\d+) as (Continue|Ok)\)\.0: (.*)\)", r)
        if m:
            y, var = int(m.group(1)), m.group(2)
            ty = self.decl(y)
            okty = ty.startswith("core::ops::ControlFlow<") if var == "Continue" else ty.startswith("core::result::Result<")
            if okty and y not in self.escaped and y < PO:
                stmts.append(("copy", l, PO + y))
                return
            stmts.append(("havoc", l))
            return
        # R3: Ok with a payload
        v, inner = enum_variant(r, "Result", ("Ok", "Err"))
        if v is not None and self.decl(l).startswith("core::result::Result<"):
            if v == "Err":
                stmts.append(("setD", l, 1))
                return
            pay = ("havoc",)
            if inner is not None:
                mm = re.fullmatch(r"(?:copy|move) _(\d+)", inner.strip())
                cv = const_val(inner)
                ov, _ = enum_variant(inner, "Option", ("None", "Some"))
                if mm and int(mm.group(1)) not in self.escaped:
                    pay = ("copy", int(mm.group(1)))
                elif cv is not None:
                    pay = ("const", cv)
                elif ov is not None:
                    pay = ("const", 0 if ov == "None" else 1)
            stmts.append(("okv", l, pay))
            return
        # R4: Option variants
        v, inner = enum_variant(r, "Option", ("None", "Some"))
        if v is not None and self.decl(l).startswith("core::option::Option<"):
            stmts.append(("setD", l, 0 if v == "None" else 1))
            return
        # R5: comparison of an integer/bool local with a constant
        m = re.fullmatch(r"(Eq|Ne)\((.*)\)", r)
        if m:
            try:
                ops = X.split_top(m.group(2))
            except X.Unsupported:
                ops = []
            if len(ops) == 2:
                for a, c in ((ops[0], ops[1]), (ops[1], ops[0])):
                    cv = const_val(c) if c.startswith("const ") else None
                    am = re.fullmatch(r"(?:copy|move) _(\d+)", a.strip())
                    if cv is not None and am:
                        t = int(am.group(1))
                        if self.is_intlike(t) and t not in self.escaped:
                            s = self.copy_origin(t)
                            if s != l and s not in self.escaped:
                                stmts.append(("cmpEq" if m.group(1) == "Eq" else "cmpNe", l, s, cv))
                                return
            stmts.append(("havoc", l))
            return
        super().assign_local(l, rhs, stmts)

    def call(self, b, dest, fnpath, args, target, stmts, extra_blocks):
        fn = self.fn
        post_blk = []
        dl = X.bare_local(dest)
        if dl is None or dl in self.escaped:
            if dl is None:
                dt = self.place_type(dest)
                self.store(dest, post_blk, "const 0_u8" if (dt is not None and X.is_plain(dt)) else None)
            dl = self.new_local()
        if post_blk:
            nb = len(fn.blocks) + len(extra_blocks)
            extra_blocks.append((post_blk, ("goto", target)))
            target = nb
        places = [X.strip_operand(a) for a in args]
        arg_locals = [X.bare_local(p) if p is not None else None for p in places]

        if X.is_result_branch(fnpath) and len(args) == 1 and arg_locals[0] is not None:
            s = self.src(arg_locals[0], stmts)
            return ("branch", dl, s, PO + dl, PO + s, target)
        if X.is_result_residual(fnpath) and len(args) == 1:
            stmts.append(("setD", dl, 1))
            return ("goto", target)
        # R7: Option::is_some / is_none on `&_o`
        for name, kind in (("is_some", "cmpNe"), ("is_none", "cmpEq")):
            if is_option_pred(fnpath, name) and len(args) == 1 and arg_locals[0] is not None:
                r = arg_locals[0]
                if r not in self.escaped and r not in self.partial and len(self.assigns.get(r, [])) == 1:
                    m = re.fullmatch(r"&_(\d+)", self.assigns[r][0])
                    if m:
                        o = int(m.group(1))
                        if o not in self.escaped and o != dl and self.decl(o).startswith("core::option::Option<") \
                                and self.decl(dl) == "bool":
                            stmts.append((kind, dl, o, 0))
                            return ("goto", target)

        # what the call may write, by the types of what it is given (as in the base class)
        S = set()
        for p in places + [None if not X.locals_in(fnpath) else fnpath]:
            if p is None:
                continue
            if p is fnpath:
                for l in X.locals_in(p):
                    S |= self.prov(l)
                continue
            l = X.bare_local(p)
            if l is not None:
                t = fn.types[l]
                if not X.cannot_write(t):
                    S |= self.prov(l)
            else:
                tm = re.search(r": ([^:]*)\)$", p)
                t = tm.group(1) if tm else "&mut ?"
                ls = X.locals_in(p)
                if not X.cannot_write(t):
                    for l2 in ls[:1]:
                        S |= self.prov(l2)
                    if not ls:
                        S |= {X.ALL}
        same = self.same_for(S)
        post = []
        est = []
        if fnpath == X.SHAPE_FN and len(args) == 1 and arg_locals[0] is not None \
                and self.market is not None and self.resolves_to(arg_locals[0], self.market):
            assert same == self.checks
            est = [0]
        for i, (a, kind) in enumerate(self.accounts):
            if fnpath == X.ACCT_CHECK_FN[kind] and len(args) == 2 and None not in arg_locals \
                    and self.resolves_to(arg_locals[0], a) and self.view_of_market_now(b, arg_locals[1]):
                assert same == self.checks
                est = [i + 1]
        if est:
            post.append((((dl, "eq", 0),), (tuple(est), ())))
        else:
            sm = self.provider.summary(fnpath) if self.provider is not None else None
            if sm is not None:
                # R8: the callee's certified clauses speak about its market and account arguments
                roles = sm["args"]
                amap, nacc = {}, 0
                if len(roles) == len(args):
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
                    for gs, c in sm["cls"]:
                        gs2 = tuple(sorted((dl if g[0] == 0 else PO + dl, g[1], g[2]) for g in gs))
                        if any(g[0] not in (0, PO) for g in gs):
                            continue
                        if c == "bot":
                            post.append((gs2, "bot"))
                        else:
                            hs = tuple(sorted(set(amap[k] for k in c[0] if k in amap)))
                            ps = tuple(sorted(set(amap[k] for k in c[1] if k in amap)))
                            # a clause that only says "preserved" about checks the call cannot
                            # change anyway carries no information
                            if hs or any(k not in same for k in ps):
                                post.append((gs2, (hs, ps)))
                    if post:
                        self.used2.append((b, fnpath))
        return ("call", [dl, PO + dl], target, same, post)

    def translate(self):
        fn = self.fn
        out, extra = [], []
        for b in range(len(fn.blocks)):
            stmts_txt, term_txt, cleanup = fn.blocks[b]
            stmts = []
            for i, s in enumerate(stmts_txt):
                self.cur = (b, i)
                self.stmt(s, stmts)
            self.cur = (b, len(stmts_txt))
            t = self.term(b, term_txt, stmts, extra)
            if t[0] == "residual":
                raise X.Unsupported("residual not rewritten")
            out.append((stmts, t))
        ir = out + extra
        # R1: every instruction that assigns local l also resets or sets its payload ghost
        res = []
        for stmts, t in ir:
            ns = []
            for s in stmts:
                k = s[0]
                if k == "okv":
                    _, l, pay = s
                    ns.append(("setD", l, 0))
                    if pay[0] == "copy":
                        ns.append(("copy", PO + l, pay[1]))
                    elif pay[0] == "const":
                        ns.append(("setD", PO + l, pay[1]))
                    else:
                        ns.append(("havoc", PO + l))
                elif k == "havoc" or k == "setD" or k in ("cmpEq", "cmpNe"):
                    ns.append(s)
                    if s[1] < PO:
                        ns.append(("havoc", PO + s[1]))
                elif k == "copy":
                    ns.append(s)
                    if s[1] < PO:
                        if s[2] < PO:
                            ns.append(("copy", PO + s[1], PO + s[2]))
                        else:
                            ns.append(("havoc", PO + s[1]))
                else:
                    ns.append(s)
            res.append((ns, t))
        return res


# ---- certificate (untrusted): a mirror of ClauseFlow.Core ----------------------------------

def t_conflict(s, t):
    (a, x), (b, y) = s, t
    if a == "eq" and b == "eq":
        return x != y
    if a == "ne" and b == "ne":
        return False
    return x == y


def t_imp(s, t):
    (a, x), (b, y) = s, t
    if a == "eq" and b == "eq":
        return x == y
    if a == "eq" and b == "ne":
        return x != y
    if a == "ne" and b == "ne":
        return x == y
    return False


def neg(g):
    return (g[0], "ne" if g[1] == "eq" else "eq", g[2])


def inconsistent(L):
    by = {}
    for g in L:
        by.setdefault(g[0], []).append((g[1], g[2]))
    for ts in by.values():
        for s in ts:
            for t in ts:
                if t_conflict(s, t):
                    return True
    return False


def idx(L):
    by = {}
    for g in L:
        by.setdefault(g[0], []).append((g[1], g[2]))
    return by


def implied_i(by, g):
    return any(t_imp(t, (g[1], g[2])) for t in by.get(g[0], ()))


def unit_lits(cls, L):
    by = idx(L)
    out = []
    for gs, c in cls:
        if c == "bot":
            rest = [g for g in gs if not implied_i(by, g)]
            if len(rest) == 1:
                out.append(neg(rest[0]))
    return out


def closure(cls, L):
    L1 = unit_lits(cls, L) + list(L)
    return unit_lits(cls, L1) + L1


class St:
    __slots__ = ("facts", "cls")

    def __init__(self, facts, cls):
        self.facts = tuple(facts)
        self.cls = tuple(cls)

    def key(self):
        return (frozenset(self.facts), frozenset(self.cls))


def entails(a, c):
    gs, cc = c
    L = closure(a.cls, list(gs) + list(a.facts))
    if inconsistent(L):
        return True
    by = idx(L)
    F = [d for d in a.cls if all(implied_i(by, g) for g in d[0])]
    if any(d[1] == "bot" for d in F):
        return True
    if cc == "bot":
        return False
    hs, ps = cc
    H = set()
    P = set()
    for d in F:
        H |= set(d[1][0])
        P |= set(d[1][1])
    return all(k in H for k in hs) and all(k in P or k in H for k in ps)


def fact_cl(f):
    return ((neg(f),), "bot")


def dead(a):
    L = closure(a.cls, list(a.facts))
    if inconsistent(L):
        return True
    return False


def mentions(l, c):
    return any(g[0] == l for g in c[0])


def kill(l, a):
    return St([f for f in a.facts if f[0] != l], [c for c in a.cls if not mentions(l, c)])


def restrict(same, c):
    if c == "bot":
        return c
    return (tuple(k for k in c[0] if k in same), tuple(k for k in c[1] if k in same))


def clobber(same, a):
    return St(a.facts, [(gs, restrict(same, c)) for gs, c in a.cls])


def rename_lit(m, l, g):
    return (l, g[1], g[2]) if g[0] == m else g


def t_stmt(s, a):
    k = s[0]
    if k == "havoc":
        return kill(s[1], a)
    if k == "write":
        return clobber(s[1], a)
    if k == "setD":
        a1 = kill(s[1], a)
        return St(((s[1], "eq", s[2]),) + a1.facts, a1.cls)
    if k == "copy":
        _, l, m = s
        if l == m:
            return a
        a1 = kill(l, a)
        return St(tuple((l, f[1], f[2]) for f in a1.facts if f[0] == m) + a1.facts,
                  tuple((tuple(rename_lit(m, l, g) for g in c[0]), c[1]) for c in a1.cls if mentions(m, c)) + a1.cls)
    if k in ("cmpEq", "cmpNe"):
        _, l, m, n = s
        if l == m:
            return kill(l, a)
        a1 = kill(l, a)
        if k == "cmpEq":
            c1, c2 = (((l, "eq", 0), (m, "eq", n)), "bot"), (((l, "ne", 0), (m, "ne", n)), "bot")
            teq, tne = ("ne", 0), ("eq", 0)
        else:
            c1, c2 = (((l, "ne", 0), (m, "eq", n)), "bot"), (((l, "eq", 0), (m, "ne", n)), "bot")
            teq, tne = ("eq", 0), ("ne", 0)

        def swap(g):
            if g == (m, "eq", n):
                return (l,) + teq
            if g == (m, "ne", n):
                return (l,) + tne
            return g
        mapped = tuple((tuple(swap(g) for g in c[0]), c[1]) for c in a1.cls if mentions(m, c))
        return St(a1.facts, (c1, c2) + mapped + a1.cls)
    raise AssertionError(s)


def known(a, i):
    out = []
    for gs, c in a.cls:
        if not gs and c != "bot":
            out += list(c[i])
    return out


def rebase(H, P, c):
    if c == "bot":
        return c
    return (tuple(c[0]) + tuple(k for k in c[1] if k in H), tuple(k for k in c[1] if k in P))


def t_term(t, a):
    k = t[0]
    if k == "goto":
        return [(t[1], a)]
    if k == "switch":
        _, l, cases, other = t
        out = [(tb, St(((l, "eq", v),) + a.facts, a.cls)) for v, tb in cases]
        if other is not None:
            out.append((other, St(tuple((l, "ne", v) for v, _ in cases) + a.facts, a.cls)))
        return out
    if k == "call":
        _, dsts, tb, same, post = t
        a1 = clobber(same, a)
        for d in dsts:
            a1 = kill(d, a1)
        H, P = known(a, 0), known(a, 1)
        return [(tb, St(a1.facts, tuple((gs, rebase(H, P, c)) for gs, c in post) + a1.cls))]
    if k == "branch":
        _, dst, src, pd, ps, tb = t
        a1 = kill(dst, kill(pd, a))
        if dst == src or dst == ps or pd == src or pd == ps or dst == pd:
            return [(tb, a1)]
        pool = a1.cls + tuple(fact_cl(f) for f in a1.facts)
        moved = tuple((((dst, "eq", 0),) + tuple(rename_lit(ps, pd, g) for g in c[0] if g != (src, "eq", 0)), c[1])
                      for c in pool if mentions(src, c) or mentions(ps, c))
        fixed = ((((src, "eq", 0), (dst, "ne", 0)), "bot"), (((src, "ne", 0), (dst, "ne", 1)), "bot"),
                 (((dst, "eq", 0), (src, "ne", 0)), "bot"))
        return [(tb, St(a1.facts, fixed + moved + a1.cls))]
    return []


def t_stmts(stmts, a):
    for s in stmts:
        a = t_stmt(s, a)
    return a


# ---- liveness (only to keep certificates small; a wrong answer makes a proof fail, not pass)

def liveness(ir):
    n = len(ir)
    use_def = []
    for stmts, t in ir:
        use_def.append((stmts, t))
    live_in = [set() for _ in range(n)]

    def succs(t):
        k = t[0]
        if k == "goto":
            return [t[1]]
        if k == "switch":
            return [b for _, b in t[2]] + ([t[3]] if t[3] is not None else [])
        if k == "call":
            return [t[2]]
        if k == "branch":
            return [t[5]]
        return []
    changed = True
    while changed:
        changed = False
        for b in range(n - 1, -1, -1):
            stmts, t = ir[b]
            live = set()
            for s in succs(t):
                live |= live_in[s]
            k = t[0]
            if k == "ret":
                live |= {0, PO}
            elif k == "switch":
                live |= {t[1]}
            elif k == "call":
                live -= set(t[1])
            elif k == "branch":
                live -= {t[1], t[3]}
                live |= {t[2], t[4]}
            for s in reversed(stmts):
                k = s[0]
                if k in ("havoc", "setD"):
                    live.discard(s[1])
                elif k == "copy":
                    if s[1] != s[2]:
                        live.discard(s[1])
                    live.add(s[2])
                elif k in ("cmpEq", "cmpNe"):
                    if s[1] != s[2]:
                        live.discard(s[1])
                    live.add(s[2])
            if live != live_in[b]:
                live_in[b] = live
                changed = True
    return live_in


def normalize(a, live):
    """A tidy state that follows from `a` (python-side only; each item is re-checked by `entails`)."""
    L = closure(a.cls, list(a.facts))
    by = idx(L)
    facts = []
    for g in dict.fromkeys(L):
        if g[0] not in live:
            continue
        if g[1] == "ne" and any(t[0] == "eq" for t in by[g[0]]):
            continue
        facts.append(g)
    H, P = set(), set()
    guarded = {}
    for gs, c in a.cls:
        rest = [g for g in gs if not implied_i(by, g)]
        if any(any(t_conflict(t, (g[1], g[2])) for t in by.get(g[0], ())) for g in rest):
            continue
        if any(g[0] not in live for g in rest):
            continue
        rest = tuple(sorted(set(rest)))
        if c == "bot":
            if rest:
                guarded.setdefault(rest, [set(), set(), False])[2] = True
            continue
        if not rest:
            H |= set(c[0])
            P |= set(c[1])
        else:
            e = guarded.setdefault(rest, [set(), set(), False])
            e[0] |= set(c[0])
            e[1] |= set(c[1])
    P -= H
    cls = []
    if H or P:
        cls.append(((), (tuple(sorted(H)), tuple(sorted(P)))))
    for gs, (h, p, bot) in sorted(guarded.items()):
        if bot:
            cls.append((gs, "bot"))
            continue
        h = h - H
        p = p - H - P - h
        if h or p:
            cls.append((gs, (tuple(sorted(h)), tuple(sorted(p)))))
    return St(facts, cls)


def gen_cands(x, vocab=(0, PO)):
    """Guard-extended versions of the clauses of x: what may survive a join with a state where
    x's facts do not hold. Guards are x's own facts, and combinations of literals about the
    return value and its payload."""
    out = []
    seen = {0, 1}
    for gs, c in x.cls:
        for g in gs:
            if g[0] == PO:
                seen.add(g[2])
    for f in x.facts:
        if f[0] == PO:
            seen.add(f[2])
    plits = [(PO, t, v) for v in sorted(seen) for t in ("eq", "ne")]
    combos = [((0, "eq", 0),)] + [(p,) for p in plits] + [((0, "eq", 0), p) for p in plits]
    for gs, c in x.cls:
        if c == "bot":
            continue
        for f in x.facts:
            out.append((tuple(sorted(set(gs + (f,)))), c))
        for G in combos:
            out.append((tuple(sorted(set(gs + G))), c))
    for f in x.facts:
        for f2 in x.facts:
            if f2 != f and f[0] in vocab and f2[0] in vocab:
                out.append((tuple(sorted({f, neg(f2)})), "bot"))
    return out


CAP = 40


def fixpoint2(ir, init, exempt=()):
    n = len(ir)
    live = liveness(ir)
    inv = [None] * n
    incoming = [dict() for _ in range(n)]
    visits = [0] * n
    incoming[0][("entry", 0)] = St((), (((), ((), tuple(init))),))
    # reverse postorder, so that a block is usually recomputed after its predecessors
    order, seen_b = [], set()

    def succs_of(t):
        k = t[0]
        if k == "goto":
            return [t[1]]
        if k == "switch":
            return [x for _, x in t[2]] + ([t[3]] if t[3] is not None else [])
        if k == "call":
            return [t[2]]
        if k == "branch":
            return [t[5]]
        return []
    stack = [(0, iter(succs_of(ir[0][1])))]
    seen_b.add(0)
    while stack:
        b0, it = stack[-1]
        nxt = next((x for x in it if x not in seen_b), None)
        if nxt is None:
            order.append(b0)
            stack.pop()
        else:
            seen_b.add(nxt)
            stack.append((nxt, iter(succs_of(ir[nxt][1]))))
    rpo = {b0: i for i, b0 in enumerate(reversed(order))}
    dirty = {0}

    def recompute(b):
        raws = list(incoming[b].values())
        if not raws:
            return None
        old = inv[b]
        visits[b] += 1
        cf, cc = [], []
        if old is not None and visits[b] > CAP:
            cf += list(old.facts)
            cc += list(old.cls)
        if visits[b] <= CAP or old is None:
            for r in raws:
                nr = normalize(r, live[b])
                cf += list(nr.facts)
                cc += list(nr.cls)
                if len(raws) > 1:
                    cc += [c for c in gen_cands(nr) if all(g[0] in live[b] for g in c[0])]
        cf = list(dict.fromkeys(cf))
        cc = list(dict.fromkeys(cc))
        facts = [f for f in cf if all(entails(r, fact_cl(f)) for r in raws)]
        cls = [c for c in cc if all(entails(r, c) for r in raws)]
        # drop clauses that follow from the kept facts and the other kept clauses (smaller certificate)
        st = St(facts, cls)
        st = prune(st)
        return st

    while dirty:
        b = min(dirty, key=lambda x: rpo.get(x, n))
        dirty.discard(b)
        new = recompute(b)
        if new is None:
            continue
        if inv[b] is not None and new.key() == inv[b].key():
            continue
        inv[b] = new
        if b in exempt:
            continue
        stmts, t = ir[b]
        a1 = t_stmts(stmts, new)
        edges = t_term(t, a1)
        seen = set()
        for i, (tb, raw) in enumerate(edges):
            key = (b, i)
            seen.add(tb)
            if dead(raw):
                if key in incoming[tb]:
                    del incoming[tb][key]
            else:
                incoming[tb][key] = raw
            dirty.add(tb)
    return inv


def prune(st):
    """Remove clauses entailed by the rest (keeps the state equivalent for the mirror)."""
    cls = list(st.cls)
    i = 0
    while i < len(cls):
        c = cls[i]
        rest = St(st.facts, cls[:i] + cls[i + 1:])
        if c[1] != "bot" and not c[1][0] and not c[1][1]:
            cls.pop(i)
            continue
        if entails(rest, c) and c[0]:
            cls.pop(i)
            continue
        i += 1
    return St(st.facts, cls)


def verify(ir, inv, goal_cls, init, exempt=()):
    """Mirror of checkCert. Returns a list of problems (empty = the Lean check should pass)."""
    bad = []
    BOTTOM = St(((ZBOT, "eq", 0), (ZBOT, "eq", 1)), ())

    def edge_ok(tb, raw):
        if dead(raw):
            return True
        c = inv[tb] if inv[tb] is not None else BOTTOM
        return all(entails(raw, fact_cl(f)) for f in c.facts) and all(entails(raw, x) for x in c.cls)
    if inv[0] is None or not edge_ok(0, St((), (((), ((), tuple(init))),))):
        bad.append(("entry", 0))
    for b, (stmts, t) in enumerate(ir):
        if b in exempt or inv[b] is None:
            continue
        a1 = t_stmts(stmts, inv[b])
        for tb, raw in t_term(t, a1):
            if not edge_ok(tb, raw):
                bad.append(("edge", b, tb))
        if t[0] == "ret":
            for c in goal_cls:
                if not entails(a1, c):
                    bad.append(("ret", b, c))
    return bad


def ret_entailed(ir, inv, c, exempt=()):
    for b, (stmts, t) in enumerate(ir):
        if t[0] != "ret" or inv[b] is None or b in exempt:
            continue
        if not entails(t_stmts(stmts, inv[b]), c):
            return False
    return True


def open_ok_sites(ir, inv, c):
    """Blocks that set `_0 = Ok` in a state from which clause c (guards dropped) does not follow."""
    out = []
    for b, (stmts, t) in enumerate(ir):
        if inv[b] is None:
            continue
        a = inv[b]
        for s in stmts:
            a = t_stmt(s, a)
            if s == ("setD", 0, 0):
                if not entails(a, ((), c[1])):
                    out.append(b)
                break
    return out


# ---- Lean output ---------------------------------------------------------------------------

def L(xs):
    return "[" + ", ".join(str(x) for x in xs) + "]"


def lean_lit(g):
    return "(%d, .%s %d)" % (g[0], g[1], g[2])


def lean_cl(c):
    gs, cc = c
    if cc == "bot":
        return "⟨[%s], .bot⟩" % ", ".join(lean_lit(g) for g in gs)
    return "⟨[%s], .chk %s %s⟩" % (", ".join(lean_lit(g) for g in gs), L(cc[0]), L(cc[1]))


def lean_stmt(s):
    k = s[0]
    if k == "write":
        return ".write %s" % L(s[1])
    return ".%s %s" % (k, " ".join(str(x) for x in s[1:]))


def lean_term(t):
    k = t[0]
    if k == "goto":
        return ".goto %d" % t[1]
    if k == "switch":
        return ".switch %d [%s] %s" % (t[1], ", ".join("(%d, %d)" % c for c in t[2]),
                                        "none" if t[3] is None else "(some %d)" % t[3])
    if k == "call":
        return ".call %s %d %s [%s]" % (L(t[1]), t[2], L(t[3]), ", ".join(lean_cl(c) for c in t[4]))
    if k == "branch":
        return ".branch %d %d %d %d %d" % t[1:]
    return "." + k


def lean_state(a):
    if a is None:
        return "some ⟨[(%d, .eq 0), (%d, .eq 1)], []⟩" % (ZBOT, ZBOT)
    return "some ⟨[%s], [%s]⟩" % (", ".join(lean_lit(f) for f in a.facts), ", ".join(lean_cl(c) for c in a.cls))


def emit_lean(genns, ns, doc, ir, inv, goal_cls, exempt, init, row=None, decide_tac="decide +kernel"):
    o = ["import ClauseFlow.Core\n", "/-\n" + doc + "\n-/\n", "namespace ClauseFlow.%s.%s\n" % (genns, ns),
         "open ClauseFlow\n", "def cfg : Cfg := ["]
    for i, (st, t) in enumerate(ir):
        o.append("  /- bb%d -/ ⟨[%s], %s⟩%s" % (i, ", ".join(lean_stmt(s) for s in st), lean_term(t),
                                                  "," if i + 1 < len(ir) else ""))
    o.append("]\n")
    o.append("def inv : List (Option AState) := [")
    for i, a in enumerate(inv):
        o.append("  /- bb%d -/ %s%s" % (i, lean_state(a), "," if i + 1 < len(inv) else ""))
    o.append("]\n")
    o.append("def goal : Goal := ⟨[%s], %s, %s⟩\n" % (", ".join(lean_cl(c) for c in goal_cls), L(exempt), L(init)))
    o.append("theorem cert : checkCert goal cfg inv = true := by %s\n" % decide_tac)
    o.append("/-- Checks are numbered: 0 = `self.validate_shape()` returns Ok; k ≥ 1 =\n"
             "`account_k.validate_with_market(&self.as_view())` returns Ok. Local 0 is the return value\n"
             "(0 = `Ok`), local %d its payload ghost. -/" % PO)
    o.append("theorem main {W : Type} (chk : Nat → W → Bool) {w0 w1 : W} {b : Block} {env1 : Env}\n"
             "    (hret : Returns chk %s cfg w0 b w1 env1) :\n"
             "    ∀ c, c ∈ goal.cls → Clause.sat chk w0 w1 env1 c :=\n"
             "  checkCert_sound chk cert hret\n" % L(exempt))
    if row is not None:
        est, pres = row
        concl = ["chk %d w1 = true" % k for k in est] + ["(chk %d w0 = true → chk %d w1 = true)" % (k, k) for k in pres]
        cl = ((((0, "eq", 0),), (tuple(est), tuple(pres))))
        o.append("/-- The ledger row: every `Ok` return%s. -/" % (
            " that does not pass through block(s) %s" % L(exempt) if exempt else ""))
        o.append("theorem row {W : Type} (chk : Nat → W → Bool) {w0 w1 : W} {b : Block} {env1 : Env}\n"
                 "    (hret : Returns chk %s cfg w0 b w1 env1) (hok : env1 0 = 0) :\n    %s := by"
                 % (L(exempt), " ∧ ".join(concl)))
        o.append("  have h := main chk hret %s (by simp [goal])" % lean_cl(cl))
        o.append("  have h2 := h (by intro g hg; simp at hg; subst hg; exact hok)")
        parts = ["h2.1 %d (by simp)" % k for k in est] + ["h2.2 %d (by simp)" % k for k in pres]
        o.append("  exact ⟨%s⟩" % ", ".join(parts) if len(parts) > 1 else "  exact %s" % parts[0])
        o.append("")
    o.append("end ClauseFlow.%s.%s" % (genns, ns))
    return "\n".join(o) + "\n"


def fn_text_sha(hdr, body):
    text = hdr + "\n" + "\n".join(body) + "\n}\n"
    return text, hashlib.sha256(text.encode()).hexdigest()
