#!/usr/bin/env python3
"""Mutation harness for the CHECKER (not the translator). For a module (cfg, inv, goal) it builds
mutants of the abstract program and of the certificate, predicts with the Python mirror whether
Lean's `checkCert` accepts each, and writes a Lean file in which both predictions are
kernel-checked theorems:

    rejected : every mutant in `rej` makes checkCert false
    accepted : every mutant in `acc` makes checkCert true

A mutant that is accepted is, by `checkCert_sound`, a correct proof about the mutated program; the
table lists them so that a reader can see they are harmless mutations and not vacuous proofs.

Families
  P-write   `.write []` inserted at the start of a block that assigns `Ok` to the return value
  P-noest   a post-clause of a call that establishes checks (`chk hs _`, hs nonempty) loses hs
  P-guard   a switch key 0 becomes 1 (and 1 becomes 0); cmpEq <-> cmpNe; a payload guard value v in a
            call's post-clause becomes v+1
  C-entry   certificate: block 0 claims a check holds unconditionally
  C-dead    certificate: the state of a block that assigns `Ok` is replaced by the unreachable state
  C-claim   certificate: a block claims `check k holds` where the certificate does not entail it
"""
import extract2 as E

PO = E.PO
BOTTOM = E.St(((E.ZBOT, "eq", 0), (E.ZBOT, "eq", 1)), ())


def ok_sites(ir, inv, exempt):
    return [b for b, (st, t) in enumerate(ir) if ("setD", 0, 0) in st and inv[b] is not None and b not in exempt]


def program_mutants(ir, inv, exempt):
    out = []
    for b in ok_sites(ir, inv, exempt):
        st, t = ir[b]
        out.append(("P-write", b, ([("write", [])] + list(st), t), "write before the Ok of bb%d" % b))
    for b, (st, t) in enumerate(ir):
        if inv[b] is None or b in exempt:
            continue
        if t[0] == "call":
            for i, (gs, c) in enumerate(t[4]):
                if c != "bot" and c[0]:
                    post = list(t[4])
                    post[i] = (gs, ((), c[1]))
                    out.append(("P-noest", b, (st, ("call", t[1], t[2], t[3], post)), "bb%d: call no longer establishes %s" % (b, list(c[0]))))
                if any(g[0] >= PO for g in gs):
                    post = list(t[4])
                    post[i] = (tuple((g[0], g[1], g[2] + 1) if g[0] >= PO else g for g in gs), c)
                    out.append(("P-guard", b, (st, ("call", t[1], t[2], t[3], post)), "bb%d: payload value of a post-clause shifted by 1" % b))
        if t[0] == "switch" and any(v in (0, 1) for v, _ in t[2]):
            cases = [((1 - v) if v in (0, 1) else v, tb) for v, tb in t[2]]
            out.append(("P-guard", b, (st, ("switch", t[1], cases, t[3])), "bb%d: switch keys 0 and 1 exchanged" % b))
        for i, s in enumerate(st):
            if s[0] in ("cmpEq", "cmpNe"):
                ns = list(st)
                ns[i] = ("cmpNe" if s[0] == "cmpEq" else "cmpEq",) + s[1:]
                out.append(("P-guard", b, (ns, t), "bb%d: comparison inverted" % b))
    return out


def cert_mutants(ir, inv, init, exempt):
    out = []
    for k in init:
        if inv[0] is not None:
            out.append(("C-entry", 0, E.St(inv[0].facts, inv[0].cls + (((), ((k,), ())),)), "entry claims check %d holds" % k))
    for b in ok_sites(ir, inv, exempt):
        out.append(("C-dead", b, BOTTOM, "bb%d (assigns Ok) claimed unreachable" % b))
    for b, a in enumerate(inv):
        if a is None or b in exempt or b == 0:
            continue
        for k in init:
            if not E.entails(a, ((), ((k,), ()))):
                out.append(("C-claim", b, E.St(a.facts, a.cls + (((), ((k,), ())),)), "bb%d claims check %d holds" % (b, k)))
                break
    return out


def run(ns, ir, inv, goal, exempt, init, max_claim=60):
    """Returns (rows, lean_text). rows: (family, description, verdict, goal_reprovable)"""
    pm = program_mutants(ir, inv, exempt)
    cm = cert_mutants(ir, inv, init, exempt)
    claims = [m for m in cm if m[0] == "C-claim"]
    if len(claims) > max_claim:          # an even sample, to bound the kernel's work
        step = len(claims) / float(max_claim)
        keep = set(id(claims[int(i * step)]) for i in range(max_claim))
        cm = [m for m in cm if m[0] != "C-claim" or id(m) in keep]
    rows, prej, pacc, crej, cacc = [], [], [], [], []
    for fam, b, blk, desc in pm:
        ir2 = list(ir)
        ir2[b] = blk
        accepted = not E.verify(ir2, inv, goal, init, exempt)
        inv2 = E.fixpoint2(ir2, init, exempt)
        reprov = all(E.ret_entailed(ir2, inv2, c, exempt) for c in goal)
        rows.append((fam, desc, "accepted" if accepted else "rejected", reprov))
        (pacc if accepted else prej).append((b, blk))
    for fam, b, stt, desc in cm:
        inv2 = list(inv)
        inv2[b] = stt
        accepted = not E.verify(ir, inv2, goal, init, exempt)
        rows.append((fam, desc, "accepted" if accepted else "rejected", None))
        (cacc if accepted else crej).append((b, stt))

    def blk(x):
        return "(%d, ⟨[%s], %s⟩)" % (x[0], ", ".join(E.lean_stmt(s) for s in x[1][0]), E.lean_term(x[1][1]))

    def stl(x):
        return "(%d, %s)" % (x[0], E.lean_state(x[1]))
    o = ["import ClauseFlow.%s" % ns, "", "/- GENERATED by proofs/strong2/valflow/mutate.py. Mutants of the abstract program and of the",
         "certificate of %s; the checker's verdict on each is a kernel-checked theorem. -/" % ns, "",
         "namespace ClauseFlow.Mut.%s" % ns.replace(".", "_"), "", "open ClauseFlow", "open ClauseFlow.%s" % ns, ""]
    for nm, xs, f in (("progRej", prej, blk), ("progAcc", pacc, blk)):
        o.append("def %s : List (Nat × BlockD) := [" % nm)
        o.append(",\n".join("  " + f(x) for x in xs))
        o.append("]\n")
    for nm, xs, f in (("certRej", crej, stl), ("certAcc", cacc, stl)):
        o.append("def %s : List (Nat × Option AState) := [" % nm)
        o.append(",\n".join("  " + f(x) for x in xs))
        o.append("]\n")
    o.append("/-- Every program mutant in `progRej` is rejected with the original certificate. -/")
    o.append("theorem prog_rejected : (progRej.all fun p => !(checkCert goal (cfg.set p.1 p.2) inv)) = true := by decide +kernel\n")
    o.append("theorem prog_accepted : (progAcc.all fun p => checkCert goal (cfg.set p.1 p.2) inv) = true := by decide +kernel\n")
    o.append("/-- Every certificate mutant in `certRej` is rejected for the original program. -/")
    o.append("theorem cert_rejected : (certRej.all fun p => !(checkCert goal cfg (inv.set p.1 p.2))) = true := by decide +kernel\n")
    o.append("theorem cert_accepted : (certAcc.all fun p => checkCert goal cfg (inv.set p.1 p.2)) = true := by decide +kernel\n")
    o.append("end ClauseFlow.Mut.%s" % ns.replace(".", "_"))
    return rows, "\n".join(o) + "\n"
