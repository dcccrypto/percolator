#!/usr/bin/env bash
# make_upload.sh <dest-dir>
# Build the Aristotle upload for ours2/v3: the engine crate exactly as committed at c38b6b1e
# (no file edited) plus the spec file aristotle/resolved_close_kernels_spec.md.
# Every engine file is written by `git show c38b6b1e:<path>` and its blob id is checked.
set -euo pipefail
REV=c38b6b1e
DEST=${1:?dest dir}
HERE=$(cd "$(dirname "$0")/.." && pwd)          # proofs/aristotle/ours2
cd "$HERE"
[ ! -e "$DEST" ] || { echo "refusing: $DEST exists" >&2; exit 2; }
mkdir -p "$DEST/src" "$DEST/tests" "$DEST/.cargo" "$DEST/aristotle"
FILES="Cargo.toml Cargo.lock .cargo/config.toml src/lib.rs src/v16.rs src/wide_math.rs src/band_rent.rs tests/v16_spec_tests.rs tests/v16_fuzzing.rs"
for f in $FILES; do
  git show "$REV:$f" > "$DEST/$f"
  want=$(git rev-parse "$REV:$f"); got=$(git hash-object "$DEST/$f")
  [ "$want" = "$got" ] || { echo "blob mismatch $f" >&2; exit 3; }
  echo "$got  $f"
done
cp v3/aristotle/resolved_close_kernels_spec.md "$DEST/aristotle/"
echo "$(git hash-object "$DEST/aristotle/resolved_close_kernels_spec.md")  aristotle/resolved_close_kernels_spec.md (ours2/v3)"
# the two pins named in the plan
[ "$(git rev-parse $REV:src/v16.rs | cut -c1-8)" = 7d3edfe8 ] && [ "$(git rev-parse $REV:src/wide_math.rs | cut -c1-8)" = 97c6b16d ] && echo "pins ok: v16.rs 7d3edfe8, wide_math.rs 97c6b16d"
