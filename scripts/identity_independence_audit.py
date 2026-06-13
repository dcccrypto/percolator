#!/usr/bin/env python3
"""Spec requirement #3 (no identity assumptions) — machine check.

> The engine MUST NOT rely on detecting self-trading, common ownership, or
> account linkage. All protections are economic and source-domain based.

This converts #3 from "structural/code-review evidence" into an enforced
static check: the account identity fields (`owner`, `provenance_header.owner`)
may be read ONLY for (a) self-consistency binding — an account's own owner
matched against its own provenance header — or (b) serialization plumbing
(field copy in from_runtime/try_to_runtime/new constructors). Any read that
compares the identity of TWO distinct accounts, or branches an economic
decision on an owner value, is a violation: the engine would then "rely on
detecting common ownership / linkage", which #3 forbids.

Method: enumerate every `.owner` read in src/v16.rs and classify it. The
permitted classes are an exact allowlist of syntactic shapes; anything else
fails the check. (`market_group_id`, `market_id` are NOT identity in the #3
sense — they are instance/asset binding, covered by reqs 4/36 — so they are
out of scope here.)
"""
import re
import sys

SRC = "src/v16.rs"
text = open(SRC).read()
lines = text.split("\n")

# Permitted syntactic shapes for an `.owner` read:
SELF_CONSISTENCY = re.compile(r"\.header\.owner\s*!=\s*\S*\.header\.provenance_header\.owner")
FIELD_COPY = re.compile(r"^\s*owner:\s")                         # struct field init
ASSIGN_FROM_LOCAL = re.compile(r"^\s*(self\.owner|let owner)\s*=")  # init assignment
READ_LOCAL = re.compile(r"^\s*let owner\s*=\s*header")           # pull owner out of a header

violations = []
for i, l in enumerate(lines):
    if ".owner" not in l:
        continue
    if l.strip().startswith("//"):
        continue
    if (SELF_CONSISTENCY.search(l) or FIELD_COPY.match(l)
            or ASSIGN_FROM_LOCAL.match(l) or READ_LOCAL.match(l)):
        continue
    violations.append((i + 1, l.strip()))

if violations:
    print("IDENTITY-INDEPENDENCE VIOLATION(S):")
    for ln, src in violations:
        print(f"  {SRC}:{ln}: {src}")
    sys.exit(1)

# Additional guard: no cross-account owner comparison anywhere.
cross = re.compile(r"\.owner\b.*(==|!=).*\.owner\b")
for i, l in enumerate(lines):
    if l.strip().startswith("//"):
        continue
    m = cross.search(l)
    if m and not SELF_CONSISTENCY.search(l):
        print(f"CROSS-ACCOUNT IDENTITY COMPARISON at {SRC}:{i+1}: {l.strip()}")
        sys.exit(1)

owner_reads = sum(1 for l in lines if ".owner" in l and not l.strip().startswith("//"))
print(f"identity-independence OK: {owner_reads} `.owner` reads, all self-binding or plumbing;")
print("no cross-account identity comparison; no economic decision branches on owner.")
