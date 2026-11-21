#!/usr/bin/env python3
"""check_statements.py <iterations>

Random search for counterexamples to RC1..RC7 of statements/Spec/ResolvedClose.lean, on a
hand-written Python mirror of statements/Spec/ResolvedCloseModel.lean (same definitions, with
BOUND_SCALE and the u128 limit as parameters so that small values stress rounding and overflow).
An assertion failure is a counterexample; finishing without one proves nothing. It is a mirror of
the Lean model, not the Lean model and not the Rust.
"""
import random, itertools, sys
def run(SC, UM, seed, iters):
    rnd = random.Random(seed)
    ck = lambda n: n if n <= UM else None
    def recompute(l):
        tot = ck(l['E'] + l['U'])
        if tot is None: return None
        l = dict(l)
        if tot == 0: l['num'] = 1; l['den'] = 1; return l
        x = ck(l['S'] * SC)
        if x is None: return None
        l['num'] = min(x, tot); l['den'] = tot; return l
    EMPTY = dict(present=False, prior=0, live=0, face=0, paid=0, fin=False)
    def rok(r):
        if r['present']:
            return r['face']*SC <= UM and r['face']*SC <= r['prior'] and r['paid'] <= r['face'] and r['fin'] == (r['paid'] == r['face'])
        return r == EMPTY
    gross = lambda r, l: (r['face']*l['num'] // l['den']) if l['den'] else 0
    def claim(r, l):
        if not rok(r): return None
        if not r['present']: return 0
        if l['halted']: return None
        if l['den'] == 0 or gross(r, l) > UM: return None
        return gross(r, l) - r['paid'] if r['paid'] <= gross(r, l) else None
    def applyp(r, x):
        if not rok(r): return None
        if x == 0: return r
        if not r['present']: return None
        if r['face'] - r['paid'] < x: return None
        p = ck(r['paid'] + x)
        if p is None: return None
        r2 = dict(r, paid=p, fin=(p == r['face']))
        return r2 if rok(r2) else None
    def create(p, a):
        if a['r']['present']: return p, a
        face = max(a['pnl'], 0); x = ck(face*SC)
        if x is None: return None
        if p['l']['U'] < x: return None
        e = ck(p['l']['E'] + x)
        if e is None: return None
        l = recompute(dict(p['l'], U=p['l']['U']-x, E=e))
        if l is None: return None
        return dict(p, l=l), dict(a, r=dict(present=True, prior=x, live=0, face=face, paid=0, fin=False))
    def close_tail(p, a):
        if a['pnl'] < 0: return None
        if a['pnl'] > 0 or a['r']['present']:
            c = create(p, a)
            if c is None: return None
            p1, a1 = c; cl = claim(a1['r'], p1['l'])
            if cl is None: return None
            has = True
        else: p1, a1, cl, has = p, a, 0, False
        s = ck(a1['cap'] + cl)
        if s is None: return None
        payout = min(s, p1['vault']); cp = min(a1['cap'], payout); rp = payout - cp
        r = applyp(a1['r'], rp) if has else a1['r']
        if r is None: return None
        return dict(p1, vault=p1['vault']-payout, ctot=p1['ctot']-min(a1['cap'], p1['ctot'])), dict(cap=0, pnl=0, r=r), payout
    def topup(p, a):
        c = claim(a['r'], p['l'])
        if c is None: return None
        if c == 0: return p, a, 0
        payout = min(c, p['vault']); r = applyp(a['r'], payout)
        if r is None: return None
        return dict(p, vault=p['vault']-payout), dict(a, r=r), payout
    def step(f, s, i):
        if i >= len(s['a']): return None
        t = f(s['p'], s['a'][i])
        if t is None: return None
        acc = list(s['a']); acc[i] = t[1]
        return dict(p=t[0], a=acc), t[2]
    def close_all(s, order):
        if not order: return s, 0
        t = step(close_tail, s, order[0])
        if t is None: return None
        u = close_all(t[0], order[1:])
        if u is None: return None
        tt = ck(t[1] + u[1])
        return None if tt is None else (u[0], tt)
    sumcap = lambda A: sum(a['cap'] for a in A)
    sumpaid = lambda A: sum(a['r']['paid'] for a in A if a['r']['present'])
    sumface = lambda A: sum(a['r']['face']*SC for a in A if a['r']['present'])
    sumgross = lambda l, A: sum(gross(a['r'], l) for a in A if a['r']['present'])
    def inv(s):
        p, A, l = s['p'], s['a'], s['p']['l']
        return (recompute(l) == l and not l['halted'] and all(rok(a['r']) for a in A) and sumface(A) <= l['E']
                and all(a['r']['paid'] <= gross(a['r'], l) for a in A if a['r']['present'])
                and p['ctot'] == sumcap(A) and p['ctot'] + p['ins'] + max(l['S'] - sumpaid(A), 0) <= p['vault'])
    stats = dict(inv=0, rc2=0, rc3=0, rc4=0, rc5=0, rc6=0, rc7=0, haircut=0, partial=0)
    M = 12
    for _ in range(iters):
        n = rnd.randint(0, 4); A = []
        for _ in range(n):
            if rnd.random() < .5:
                face = rnd.randint(0, M); pr = face*SC + rnd.choice([0, 0, rnd.randint(0, 3)]); paid = rnd.randint(0, face)
                r = dict(present=True, prior=pr, live=rnd.choice([0, 0, 1]), face=face, paid=paid, fin=rnd.choice([paid == face]*5 + [paid != face]))
            else: r = dict(EMPTY) if rnd.random() < .9 else dict(EMPTY, paid=1)
            A.append(dict(cap=rnd.randint(0, M), pnl=rnd.randint(-2, M), r=r))
        E = sumface(A) + rnd.choice([0, 0, rnd.randint(0, 2*SC)]) - rnd.choice([0]*9 + [1]); E = max(E, 0)
        l = dict(S=rnd.randint(0, 3*M), E=E, U=rnd.choice([0, rnd.randint(0, 3*M*SC)]), num=0, den=0, halted=rnd.random() < .03)
        l2 = recompute(l)
        if l2 is not None and rnd.random() < .95: l = l2
        ctot = sumcap(A) + rnd.choice([0]*15 + [1]); ins = rnd.randint(0, M)
        vault = ctot + ins + max(l['S'] - sumpaid(A), 0) + rnd.choice([0, 0, rnd.randint(0, 5)]) - rnd.choice([0]*9 + [1]); vault = max(vault, 0)
        s = dict(p=dict(vault=vault, ctot=ctot, ins=ins, l=l), a=A)
        if not inv(s): continue
        stats['inv'] += 1
        assert sumgross(l, A) <= l['S'] and sumpaid(A) <= sumgross(l, A), ('RC1', s)
        if l['num'] < l['den']: stats['haircut'] += 1
        for i in range(n + 1):
            t = step(close_tail, s, i)
            a = A[i] if i < n else None
            if a is not None and vault <= UM and a['pnl'] >= 0 and (a['r']['present'] or max(a['pnl'], 0)*SC <= l['U']):
                stats['rc3'] += 1; assert t is not None, ('RC3', s, i)
            if t is not None:
                s2, pay = t; stats['rc2'] += 1; a2 = s2['a'][i]; p2 = s2['p']
                assert inv(s2), ('RC2 inv', s, i)
                assert p2['vault'] + pay == vault and pay + sumpaid(A) == a['cap'] + sumpaid(s2['a']) and p2['ctot'] + a['cap'] == ctot, ('RC2 val', s, i)
                assert len(s2['a']) == n and a2['cap'] == 0 and a2['pnl'] == 0 and (not a2['r']['present'] or a2['r']['paid'] == gross(a2['r'], p2['l'])), ('RC2 acct', s, i)
                assert all(s2['a'][j] == A[j] for j in range(n) if j != i) and p2['ins'] == ins and (p2['l']['S'], p2['l']['num'], p2['l']['den']) == (l['S'], l['num'], l['den']), ('RC2 frame', s, i)
                if a2['r']['present'] and 0 < a2['r']['paid'] < a2['r']['face']: stats['partial'] += 1
            t = step(topup, s, i)
            if t is not None:
                s2, pay = t; stats['rc7'] += 1
                assert inv(s2) and s2['p']['vault'] + pay == vault and sumpaid(s2['a']) == sumpaid(A) + pay and sumcap(s2['a']) == sumcap(A), ('RC7', s, i)
        order = [rnd.randint(0, max(n - 1, 0) + (rnd.random() < .05)) for _ in range(rnd.randint(0, 6))] if rnd.random() < .5 else rnd.sample(range(n), n)
        t = close_all(s, order)
        if t is not None:
            s2, tot = t; stats['rc4'] += 1; p2 = s2['p']
            assert inv(s2) and p2['vault'] + tot == vault and tot + sumcap(s2['a']) + sumpaid(A) == sumcap(A) + sumpaid(s2['a']), ('RC4', s, order)
            assert sumpaid(s2['a']) <= l['S'] and p2['ctot'] + p2['ins'] <= p2['vault'] and p2['ins'] == ins, ('RC4b', s, order)
            assert all(s2['a'][i]['cap'] == 0 and s2['a'][i]['pnl'] == 0 for i in order if i < n), ('RC4c', s, order)
            if all(i in order for i in range(n)):
                stats['rc5'] += 1
                assert p2['ctot'] == 0 and tot + sumpaid(A) == sumcap(A) + sumpaid(s2['a']) and tot + sumpaid(A) <= sumcap(A) + l['S'] and p2['ins'] <= p2['vault'], ('RC5', s, order)
            o2 = list(order); rnd.shuffle(o2); stats['rc6'] += 1
            assert close_all(s, o2) == t, ('RC6', s, order, o2)
    return stats
for SC, UM in [(10**12, 2**128-1), (4, 2**128-1), (3, 400), (1, 60), (7, 2000)]:
    print(SC, UM, run(SC, UM, 1121, int(sys.argv[1])))
