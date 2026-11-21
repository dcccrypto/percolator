#!/usr/bin/env python3
"""Rule R9 table from the compiler instead of from source text (audit round 4, holes H2/H3).

The earlier table was parsed from `src/v16.rs` text and could be fooled (explicit discriminants on
tuple variants, commented-out or cfg'd-out decoy declarations). Now the text parse only PROPOSES
candidates (untrusted). For each candidate enum NAME with proposed variants V.. this file writes

    #[inline(never)] pub fn s2_enum_probe_NAME(x: &NAME) -> u32 {
        match *x { NAME::V0 { .. } => 7000, NAME::V1 { .. } => 7001, .., _ => 6999 }
    }

appended at the top level of a scratch copy of `src/v16.rs`. rustc resolves the names and compiles
the match; the TRUSTED reader below takes, from the MIR of the probe,

    _d = discriminant((*_1));  switchInt(move _d) -> [D: bbX, .., otherwise: bbZ]
    bbX: { _0 = const 700i_u32; .. }

and records variant Vi -> discriminant D. Anything else in a probe body (another shape, a block
that is not exactly one constant assignment, a repeated variant or discriminant, an argument type
other than `&NAME` / `&v16::NAME`) drops the whole enum from the table: rule R9 then refuses it.

Usage:  enum_probe.py gen SRC/src/v16.rs SCRATCH/src/v16.rs      (writes the scratch file)
        table(probe_mir_path) -> {NAME: {VARIANT: discriminant}}  (from Python)
"""
import re
import sys

BASE = 7000
PREFIX = "s2_enum_probe_"
MARKER = "// ---- strong2 rule R9 probes (scratch copy only; not part of the engine) ----"


def propose(src_text):
    """Untrusted: candidate plain enums and their variant names, from the text."""
    lines = src_text.split("\n")
    out = {}
    i = 0
    while i < len(lines):
        m = re.match(r"^(?:#\[[^\]]*\]\s*)*(?:pub(?:\([a-z ]+\))? )?enum (\w+) \{(.*)$", lines[i])
        if not m:
            i += 1
            continue
        name, body, depth = m.group(1), [m.group(2)], 1 + m.group(2).count("{") - m.group(2).count("}")
        i += 1
        while i < len(lines) and depth > 0:
            body.append(lines[i])
            depth += lines[i].count("{") - lines[i].count("}")
            i += 1
        text = re.sub(r"//[^\n]*", "", "\n".join(body))
        text = re.sub(r"#\[[^\]]*\]", "", text)
        # drop nested groups, keep the top-level comma-separated variant heads
        flat, d = [], 0
        for ch in text:
            if ch in "({":
                d += 1
            elif ch in ")}":
                d -= 1
            elif d == 0:
                flat.append(ch)
        vs = []
        for part in "".join(flat).split(","):
            vm = re.match(r"^\s*(\w+)", part)
            if vm:
                vs.append(vm.group(1))
        if vs:
            out.setdefault(name, [])
            for v in vs:
                if v not in out[name]:
                    out[name].append(v)
    return out


def probe_text(src_text):
    cands = propose(src_text)
    if MARKER in src_text or PREFIX in src_text:
        raise SystemExit("the source already contains the probe marker or prefix: refusing")
    out = ["", MARKER]
    for name in sorted(cands):
        if not re.fullmatch(r"[A-Za-z_]\w*", name):
            continue
        arms = "".join("        %s::%s { .. } => %d,\n" % (name, v, BASE + 1 + j) for j, v in enumerate(cands[name]))
        out.append("// variants: %s" % " ".join(cands[name]))
        out.append("#[allow(unreachable_patterns, dead_code)]\n#[inline(never)]\npub fn %s%s(x: &%s) -> u32 {\n"
                   "    match *x {\n%s        _ => %d,\n    }\n}" % (PREFIX, name, name, arms, BASE))
    return "\n".join(out) + "\n", cands


# ---------------------------------------------------------------- TRUSTED reader
def table(probe_mir_path, src_probe_path):
    """{NAME: {VARIANT: discriminant}} for every probe whose MIR has exactly the expected shape.
    Variant names come from the `// variants:` line the generator wrote above each probe in
    `src_probe_path`: the i-th name there is the arm returning BASE+1+i (written by `probe_text`;
    rustc resolved `NAME::Vi { .. }` against the real declaration, or the build failed)."""
    names = {}
    # The probe names must be ours alone: no `s2_enum_probe_` anywhere in the crate sources except
    # after the single marker line that `probe_text` appends at the end of src/v16.rs (a function
    # of that name elsewhere, or a second marker, empties the table).
    import os
    sdir = os.path.dirname(os.path.abspath(src_probe_path))
    for root, _, files in os.walk(sdir):
        for f in files:
            fp = os.path.join(root, f)
            if os.path.abspath(fp) != os.path.abspath(src_probe_path) and PREFIX in open(fp, errors="replace").read():
                return {}
    whole = open(src_probe_path).read()
    if whole.count(MARKER) != 1 or PREFIX in whole[:whole.index(MARKER)]:
        return {}
    lines = whole[whole.index(MARKER):].split("\n")
    for i, ln in enumerate(lines):
        m = re.match(r"^pub fn %s(\w+)\(x: &(\w+)\) -> u32 \{$" % PREFIX, ln)
        if m and m.group(1) == m.group(2) and i >= 3 and lines[i - 3].startswith("// variants: "):
            arms, j = [], i + 2
            while j < len(lines) and lines[j].strip() != "_ => %d," % BASE:
                am = re.fullmatch(r"\s+(\w+)::(\w+) \{ \.\. \} => (\d+),", lines[j])
                if not am or am.group(1) != m.group(1):
                    arms = None
                    break
                arms.append((am.group(2), int(am.group(3))))
                j += 1
            if arms and [k for _, k in arms] == list(range(BASE + 1, BASE + 1 + len(arms))) \
                    and len(set(v for v, _ in arms)) == len(arms) and m.group(1) not in names:
                names[m.group(1)] = dict((k, v) for v, k in arms)
            else:
                names[m.group(1)] = None
    text = open(probe_mir_path).read()
    out = {}
    for m in re.finditer(r"^fn (?:v16::)?%s(\w+)\(_1: &(?:v16::)?(\w+)\) -> u32 \{\n(.*?)^\}\n" % PREFIX, text, re.S | re.M):
        name, ty, body = m.group(1), m.group(2), m.group(3)
        if name != ty or not names.get(name) or name in out:
            out[name] = None
            continue
        out[name] = read_probe(body, names[name])
    return {k: v for k, v in out.items() if v}


def read_probe(body, arm_names):
    blocks = {}
    for bm in re.finditer(r"^    bb(\d+): \{\n(.*?)^    \}\n", body, re.S | re.M):
        blocks[int(bm.group(1))] = [s.strip() for s in bm.group(2).strip().split("\n")]
    if 0 not in blocks or len(blocks[0]) != 2:
        return None
    dm = re.fullmatch(r"_(\d+) = discriminant\(\(\*_1\)\);", blocks[0][0])
    sm = re.fullmatch(r"switchInt\(move _(\d+)\) -> \[(.*)\];", blocks[0][1])
    if not dm or not sm or dm.group(1) != sm.group(1):
        return None
    res, seen_d = {}, set()
    for part in sm.group(2).split(", "):
        pm = re.fullmatch(r"(\d+|otherwise): bb(\d+)", part)
        if not pm:
            return None
        if pm.group(1) == "otherwise":
            continue
        d, b = int(pm.group(1)), int(pm.group(2))
        st = blocks.get(b)
        if not st or len(st) != 2 or st[1] not in ("return;", ) and not re.fullmatch(r"goto -> bb\d+;", st[1]):
            return None
        cm = re.fullmatch(r"_0 = const (\d+)_u32;", st[0])
        if not cm or d in seen_d:
            return None
        seen_d.add(d)
        k = int(cm.group(1))
        if k == BASE:
            continue   # a discriminant that falls to the wildcard arm: variant not in the table
        v = arm_names.get(k)
        if v is None or v in res:
            return None
        res[v] = d
    return res or None


if __name__ == "__main__":
    if sys.argv[1] == "gen":
        t = open(sys.argv[2]).read()
        p, c = probe_text(t)
        open(sys.argv[3], "w").write(t + p)
        print("probes written: %d" % len(c))
    else:
        tb = table(sys.argv[2], sys.argv[3])
        for k in sorted(tb):
            print(k, tb[k])
        print("enums in the table: %d" % len(tb))
