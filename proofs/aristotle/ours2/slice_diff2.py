#!/usr/bin/env python3
"""slice_diff2.py [--write]: compare the upload in project/ with the engine at the pinned commit
(read with `git show c38b6b1e:...`). Prints section A (every copied block, byte comparison),
section B (every line of the upload that is not engine text, verbatim) and section C (unified
diffs, engine (-) against slice (+), for each stub and for the tail wrapper).
With --write, writes SLICE-DIFF.md = SLICE-DIFF.head.md + this output."""
import difflib, re, subprocess, sys, pathlib
HERE = pathlib.Path(__file__).resolve().parent
PIN = "c38b6b1e"
def git_show(path):
    return subprocess.run(["git", "show", f"{PIN}:{path}"], cwd=HERE, check=True, capture_output=True).stdout.decode()
v16 = git_show("src/v16.rs").split("\n")
wide = git_show("src/wide_math.rs")
sl = (HERE / "project/src/engine_slice.rs").read_text().split("\n")
out = []; P = out.append

# ---- A ----
P("## A. Copied blocks vs engine (byte comparison)\n")
P("Engine = `git show c38b6b1e:src/v16.rs`. Slice lines are lines of `project/src/engine_slice.rs`.\n")
P("| slice lines | engine lines | lines | first engine line | result |\n|---|---|---|---|---|")
engine_lines = 0; ne_lines = []; state = "top"; bad = 0; k = 0; other = []
while k < len(sl):
    l = sl[k]
    m = re.fullmatch(r"// ---- src/v16\.rs:(\d+)-(\d+) ----", l)
    if m:
        a, b = int(m.group(1)), int(m.group(2))
        e = k + 1
        while sl[e] != "// ---- end ----": e += 1
        ok = sl[k + 1:e] == v16[a - 1:b]
        bad |= not ok
        engine_lines += b - a + 1
        P(f"| {k + 2}-{e} | {a}-{b} | {b - a + 1} | `{v16[a - 1].strip()[:70]}` | {'byte-identical' if ok else 'DIFFERENT'} |")
        k = e + 1; continue
    if l.startswith("// >>> NOT ENGINE"):
        e = k + 1
        while sl[e] != "// <<< NOT ENGINE": e += 1
        ne_lines.append((k + 1, e + 1, sl[k:e + 1]))
        k = e + 1; continue
    if l.strip() != "": other.append((k + 1, l))
    k += 1
wide_ok = (HERE / "project/src/wide_math.rs").read_text() == wide
bad |= not wide_ok
P(f"\n`project/src/wide_math.rs` vs engine `src/wide_math.rs` ({len(wide.splitlines())} lines): {'byte-identical' if wide_ok else 'DIFFERENT'}.\n")
n_ne = sum(len(x[2]) for x in ne_lines)
P(f"`project/src/engine_slice.rs` has {len(sl) - 1} lines: {engine_lines} engine lines in {len([1 for l in sl if l == '// ---- end ----'])} blocks, "
  f"{n_ne} lines in NOT ENGINE regions (markers included), {2 * len([1 for l in sl if l == '// ---- end ----'])} block marker lines, "
  f"{len(other)} header comment lines, and blank lines.\n")
hdr_ok = all(l.startswith("//") for _, l in other)
P(f"Lines outside blocks and NOT ENGINE regions that are not comments or blank: {'none' if hdr_ok else 'SOME'}.\n")
bad |= not hdr_ok

# ---- B ----
P("## B. Every line of the upload that is not engine text\n")
P("### B.1 `project/src/engine_slice.rs`: the file header and the NOT ENGINE regions, verbatim\n")
P("```rust")
for n, l in other: P(f"{n:5}: {l}")
P("```\n")
for a, b, ls in ne_lines:
    P(f"Slice lines {a}-{b}:\n\n```rust")
    out.extend(ls)
    P("```\n")
for rel in ["project/src/engine_slice/harness.rs", "project/src/lib.rs", "project/Cargo.toml"]:
    P(f"### B.{2 + ['project/src/engine_slice/harness.rs', 'project/src/lib.rs', 'project/Cargo.toml'].index(rel)} `{rel}` (whole file, not engine)\n")
    P("```" + ("toml" if rel.endswith("toml") else "rust")); P((HERE / rel).read_text().rstrip("\n")); P("```\n")
P("`project/aristotle/resolved_close_spec.md` is the English spec (not engine).\n")

# ---- C ----
P("## C. Diff hunks: engine (-) vs slice (+)\n")
def item(lines, pat, start=0):
    i = next(k for k in range(start, len(lines)) if re.search(pat, lines[k]))
    # include doc/attribute lines directly above
    j = i
    ind = len(lines[i]) - len(lines[i].lstrip())
    while not (lines[j].startswith(" " * ind + "}") and len(lines[j].rstrip()) == ind + 1): j += 1
    return i, j
def udiff(title, e, s, n=1):
    P(f"### {title}\n\n```diff")
    P("\n".join(difflib.unified_diff(e, s, "engine src/v16.rs@" + PIN, "slice", lineterm="", n=n)))
    P("```\n")
def eng(a, b): return v16[a - 1:b]
def slc(pat):
    i, j = item(sl, pat); return sl[i:j + 1]
STRUCTS = [
    ("`MarketGroupV16View` (engine 6226-6229)", (6226, 6229), r"^pub struct MarketGroupV16View<"),
    ("`MarketGroupV16ViewMut` (engine 6231-6234)", (6231, 6234), r"^pub struct MarketGroupV16ViewMut<"),
    ("`MarketGroupV16HeaderAccount` (engine 9445-9494)", (9447, 9494), r"^pub struct MarketGroupV16HeaderAccount"),
    ("`HealthCertV16Account` (engine 27921-27935)", (27923, 27935), r"^pub struct HealthCertV16Account"),
    ("`PortfolioAccountV16Account` (engine 28180-28219)", (28182, 28219), r"^pub struct PortfolioAccountV16Account"),
    ("`PortfolioSourceDomainV16Account` (engine 28124-28145; STUB-DEAD use only)", (28126, 28145), r"^pub struct PortfolioSourceDomainV16Account"),
    ("`SourceCreditStateV16` (engine 6020-6033; STUB-DEAD use only)", (6020, 6033), r"^pub struct SourceCreditStateV16"),
]
for title, (a, b), pat in STRUCTS:
    udiff(title, eng(a, b), slc(pat), n=0)
P("The derive lines above these structs: the engine's are `#[repr(C)]` and `#[derive(Clone, Copy, Debug, [Default,] PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]`; the slice drops the two `bytemuck` derives (see B.1). The same holds for the five byte-wrapper structs whose bodies are engine blocks (`V16PodU64`, `V16PodU128`, `V16PodI128`, `ResolvedPayoutLedgerV16Account`, `ResolvedPayoutReceiptV16Account`):\n")
P("```diff\n-" + v16[8417] + "\n+#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]\n```\n")
def engfn(pat, start=0):
    i, j = item(v16, pat, start); return v16[i:j + 1], i + 1, j + 1
FNS = [
    ("as_view", r"^    pub fn as_view\(&self\) -> MarketGroupV16View<", r"^    pub fn as_view\(&self\) -> MarketGroupV16View<", 0),
    ("validate_shape (STUB-OK)", r"^    pub fn validate_shape\(&self\)", r"^    pub fn validate_shape\(&self\)", 10376),
    ("validate_with_market (STUB-OK)", r"^    pub fn validate_with_market<T>\(", r"^    pub fn validate_with_market\(", 6863),
    ("sync_unattributed_loss_lock (STUB-FRAME)", r"^    fn sync_unattributed_loss_lock\(", r"^    fn sync_unattributed_loss_lock\(", 0),
    ("burn_account_source_claim_bound_num (STUB-PATH)", r"^    fn burn_account_source_claim_bound_num\(", r"^    fn burn_account_source_claim_bound_num\(", 0),
    ("ensure_account_source_claim_market_id (STUB-DEAD)", r"^    fn ensure_account_source_claim_market_id\(", r"^    fn ensure_account_source_claim_market_id\(", 0),
    ("source_credit_for_domain (STUB-DEAD)", r"^    fn source_credit_for_domain\(", r"^    fn source_credit_for_domain\(", 0),
    ("set_source_credit_for_domain (STUB-DEAD)", r"^    fn set_source_credit_for_domain\(", r"^    fn set_source_credit_for_domain\(", 0),
    ("source_domain_mut_or_insert (STUB-DEAD)", r"^    fn source_domain_mut_or_insert\(", r"^    fn source_domain_mut_or_insert\(", 0),
    ("prepare_source_credit_domain_recompute_for_epoch (STUB-DEAD)", r"^    fn prepare_source_credit_domain_recompute_for_epoch\(", r"^    fn prepare_source_credit_domain_recompute_for_epoch\(", 0),
]
for title, ep, sp, start in FNS:
    e, a, b = engfn(ep, start)
    udiff(f"`{title.split(' ')[0]}`{title[len(title.split(' ')[0]):]}: engine {a}-{b}", e, slc(sp), n=1)
# the tail: the whole engine function against the wrapper function
e, a, b = engfn(r"^    pub fn close_resolved_account_not_atomic\(")
udiff(f"`close_resolved_account_not_atomic` (engine {a}-{b}) vs the wrapper `close_resolved_account_tail_not_atomic`", e,
      [l for l in slc(r"^    pub fn close_resolved_account_tail_not_atomic\(") if not l.startswith("// ----") and l not in ("// >>> NOT ENGINE", "// <<< NOT ENGINE")], n=1)
# impl header: for each engine block, the nearest `impl` line above it in the engine
P("### `impl` lines\n\nFor each engine block that the slice puts inside an `impl ... {` line of its own, the nearest line above it in the engine that starts with `impl`:\n")
P("| engine block | engine `impl` line above it | slice `impl` line |\n|---|---|---|")
for k, l in enumerate(sl):
    m = re.fullmatch(r"// ---- src/v16\.rs:(\d+)-(\d+) ----", l)
    if not m: continue
    a = int(m.group(1))
    if not v16[a - 1].startswith("    "): continue
    q = a - 1
    while not v16[q].startswith("impl"): q -= 1
    simpl = next(sl[j] for j in range(k, 0, -1) if sl[j].startswith("impl"))
    P(f"| {a}-{m.group(2)} | {q + 1}: `{v16[q]}` | `{simpl}` |")
P("")
text = "\n".join(out) + "\n"
if "--write" in sys.argv:
    (HERE / "SLICE-DIFF.md").write_text((HERE / "SLICE-DIFF.head.md").read_text() + "\n" + text)
    print("wrote SLICE-DIFF.md (%d bytes)" % (HERE / "SLICE-DIFF.md").stat().st_size)
else:
    print(text)
sys.exit(1 if bad else 0)
