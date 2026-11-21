#!/bin/sh
# wide_math_text_check.sh: compare src/wide_math.rs at our pinned commit c38b6b1e with upstream
# aeyakovenko/percolator add1d027 (the revision upstream's ledger row F3 names). Text only.
# Needs `git fetch upstream` to have been run (read-only). Run from anywhere inside the checkout.
set -eu
O=$(mktemp); U=$(mktemp); trap 'rm -f "$O" "$U"' EXIT
git show c38b6b1e:src/wide_math.rs > "$O"; git show add1d027:src/wide_math.rs > "$U"
echo "ours c38b6b1e blob $(git rev-parse c38b6b1e:src/wide_math.rs), $(wc -l < "$O" | tr -d ' ') lines"
echo "upstream add1d027 blob $(git rev-parse add1d027:src/wide_math.rs), $(wc -l < "$U" | tr -d ' ') lines"
echo "hunks (git diff add1d027 c38b6b1e):"; git diff add1d027 c38b6b1e -- :/src/wide_math.rs | grep '^@@' || true
cmpr() { a=$(sed -n "$2p" "$O" | shasum | cut -c1-12); b=$(sed -n "$3p" "$U" | shasum | cut -c1-12)
  [ "$a" = "$b" ] && r=identical || r=DIFFERENT; echo "$1: ours $2 vs upstream $3: $r"; }
cmpr "prefix" 1,1422 1,1422
echo "ours 1423-1425:"; sed -n 1423,1425p "$O"
cmpr "middle (contains wide_mul_div_floor_u128)" 1426,1631 1423,1628
cmpr "suffix" 1724,2217 1677,2170
echo "wide_mul_div_floor_u128 in ours at line $(grep -n '^pub fn wide_mul_div_floor_u128' "$O" | cut -d: -f1), upstream at line $(grep -n '^pub fn wide_mul_div_floor_u128' "$U" | cut -d: -f1)"
echo "uses, in ours, of the two functions inside the changed region 1632-1723:"
grep -n 'wide_signed_mul_div_floor_from_k_pair\|wide_signed_mul_div_floor_with_carry_from_k_pair' "$O" | grep -v 'assert\|"' || true
