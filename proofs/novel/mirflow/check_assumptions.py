#!/usr/bin/env python3
"""Source checks behind the Rust-level assumptions of the method (see METHOD-RULES.md, section A).
Prints one line per check; exits 1 if any fails. These are text scans, not proofs."""
import os
import re
import sys

root = sys.argv[1]
mir_path = sys.argv[2] if len(sys.argv) > 2 else None
files = {}
for dp, _, fs in os.walk(os.path.join(root, "src")):
    for f in fs:
        if f.endswith(".rs"):
            p = os.path.join(dp, f)
            files[os.path.relpath(p, root)] = open(p).read().split("\n")
ok = True


def strip_comment(l):
    return l.split("//")[0]


def scan(pat, allow=None):
    hits = []
    for f, lines in sorted(files.items()):
        for i, l in enumerate(lines, 1):
            c = strip_comment(l)
            if re.search(pat, c) and not (allow and re.search(allow, c)):
                hits.append("%s:%d: %s" % (f, i, l.strip()[:100]))
    return hits


def report(name, hits, expect_none=True):
    global ok
    good = (not hits) if expect_none else bool(hits)
    ok = ok and good
    print("%s  %s%s" % ("PASS" if good else "FAIL", name, "" if good and expect_none else " (%d hits)" % len(hits)))
    for h in hits[:12]:
        print("      " + h)


report("A1 crate denies unsafe code (#![deny(unsafe_code)] in src/lib.rs)",
       [l for l in files.get("src/lib.rs", []) if l.strip() == "#![deny(unsafe_code)]"], expect_none=False)
report("A1 no unsafe block or unsafe fn (only `unsafe trait` / `unsafe impl` marker items allowed)",
       scan(r"\bunsafe\b", allow=r"\bunsafe (trait|impl)\b|unsafe_code"))
report("A1 `#[allow(unsafe_code)]` appears only directly above an `unsafe trait`/`unsafe impl`",
       [h for f, lines in sorted(files.items()) for i, l in enumerate(lines)
        if "allow(unsafe_code)" in strip_comment(l)
        and not re.search(r"\bunsafe (trait|impl)\b", lines[i + 1] if i + 1 < len(lines) else "")
        for h in ["%s:%d: %s" % (f, i + 1, l.strip())]])
A2_PAT = r"\b(Cell|RefCell|UnsafeCell|Mutex|RwLock|OnceCell|OnceLock|LazyLock)\b|\bAtomic[A-Z]\w*|static\s+mut\b|thread_local!"
A2_NAME = ("A2 no interior mutability or shared mutable state "
           "(Cell, RefCell, UnsafeCell, Mutex, RwLock, Atomic*, OnceCell, OnceLock, static mut, thread_local)")
src_hits = scan(A2_PAT)
if mir_path is None:
    report(A2_NAME + " [source text, every configuration]", src_hits)
else:
    # The MIR dump is the compiled configuration: items behind a disabled `cfg` are not in it.
    mir_hits = []
    with open(mir_path) as f:
        for i, l in enumerate(f, 1):
            c = l.split("//")[0]
            if re.search(A2_PAT, c) or re.match(r"^static\b", c):
                mir_hits.append("MIR:%d: %s" % (i, l.strip()[:100]))
    report(A2_NAME + " and no `static` item [MIR of the analysed build]", mir_hits)
    if src_hits:
        print("INFO  the source text has %d such line(s) in code that is not in the analysed build "
              "(behind a `cfg`):" % len(src_hits))
        for h in src_hits[:12]:
            print("      " + h)
report("A2 no raw pointers (*const, *mut, as_ptr, as_mut_ptr, addr_of)",
       scan(r"\*const\b|\*mut\b|\bas_ptr\b|\bas_mut_ptr\b|addr_of"))
report("A3 no clock, randomness, environment, file or network access (std::time, rand, std::env, std::fs, std::net)",
       scan(r"\bstd::(time|env|fs|net|io|process|thread)\b|\brand::|\bSystemTime\b|\bInstant\b"))
report("A3 no extern blocks / FFI", scan(r"\bextern\s+\"|\bextern\s*\{"))


report("A6 no user type named `Result` (the tool reads `Result::Ok/Err` as core's)",
       scan(r"\b(enum|struct|union|trait)\s+Result\b"))
report("A5 no method of the market view has type parameters of its own (`fn name<T..>` inside "
       "`impl<'a, T> MarketGroupV16ViewMut<'a, T>`); lifetimes are allowed",
       [h for h in (lambda: [
           "src/v16.rs:%d: %s" % (i + 1, l.strip()[:100])
           for i, l, inside in (lambda ls: [
               (i, l, ins) for i, (l, ins) in enumerate(zip(ls, __import__("itertools").accumulate(
                   ls, lambda acc, x: True if x.startswith("impl<'a, T> MarketGroupV16ViewMut<'a, T> {")
                   else (False if x.startswith("}") else acc), initial=False)))])(files["src/v16.rs"])
           if inside and re.search(r"\bfn\s+\w+\s*<\s*(?!')[A-Za-z]", strip_comment(l))])()])


def struct_fields(name):
    lines = files["src/v16.rs"]
    for i, l in enumerate(lines):
        if re.match(r"^pub struct %s<" % name, l):
            out = []
            for m in lines[i + 1:]:
                if m.startswith("}"):
                    return out
                if not m.strip().startswith("//") and m.strip():
                    out.append(m.strip())
    return None


for name in ("MarketGroupV16View", "PortfolioV16View"):
    fs = struct_fields(name)
    bad = fs is None or any(not re.match(r"^pub \w+: &'a (?!mut\b)", f) for f in fs)
    ok = ok and not bad
    print("%s  A4 by-value view `%s` holds only shared references: %s" % ("FAIL" if bad else "PASS", name, fs))

deps = []
inside = False
for l in open(os.path.join(root, "Cargo.toml")):
    if l.startswith("["):
        inside = l.strip() == "[dependencies]"
    elif inside and l.strip():
        deps.append(l.strip())
print("INFO  dependencies of the library: %s" % deps)
print("INFO  `bytemuck` is used through its safe API; its own `unsafe` is outside this crate and is assumed correct.")
sys.exit(0 if ok else 1)
