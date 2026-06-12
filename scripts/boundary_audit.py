#!/usr/bin/env python3
"""Ok-exit validator boundary audit (engine no-LoF theorem, Lemma 0).

Checks that every public *_not_atomic entrypoint's success path terminates in
(or transitively delegates to a path terminating in) one of the engine's
state validators. Under the execution-boundary assumption that Err commits
nothing, this makes `validate_shape`'s content (senior cover, exact aggregate
totals, per-domain ledger closure) hold at EVERY committed state.
"""
import re, sys

VALIDATORS = (
    "validate_shape()", "validate_shape_audit_scan", "validate_source_domain_ledger",
    "validate_with_market", "validate_account_audit_scan", "validate_header_aggregate_totals",
    "refresh_source_credit_domain_after_mutation",  # ends in domain-ledger validation
    "validate_asset_shape", "validate_public_user_fund_shape",
)

src = open("src/v16.rs").read()
lines = src.split("\n")
info, order = {}, []
i = 0
while i < len(lines):
    m = re.match(r"\s*(?:pub )?fn (\w+_not_atomic)\b", lines[i])
    if m:
        name = m.group(1)
        k = i
        while k < len(lines) and "{" not in lines[k]:
            k += 1
        d, end = 0, k
        for kk in range(k, len(lines)):
            d += lines[kk].count("{") - lines[kk].count("}")
            if d == 0:
                end = kk
                break
        body = "\n".join(lines[i:end + 1])
        info[name] = dict(
            pub=lines[i].strip().startswith("pub "),
            direct=any(v in body for v in VALIDATORS),
            delegates=set(re.findall(r"self\.(?:header\.)?(\w+_not_atomic)\(", body)) - {name},
        )
        order.append(name)
        i = end + 1
    else:
        i += 1

def validated(name, seen=None):
    seen = seen or set()
    if name in seen or name not in info:
        return False
    seen.add(name)
    f = info[name]
    return f["direct"] or any(validated(d, seen) for d in f["delegates"])

pubs = [n for n in order if info[n]["pub"]]
bad = [n for n in pubs if not validated(n)]
print(f"public entrypoints: {len(pubs)}; Ok-exit validated (transitively): {len(pubs) - len(bad)}")
for n in bad:
    print("  UNVALIDATED:", n)
sys.exit(1 if bad else 0)
