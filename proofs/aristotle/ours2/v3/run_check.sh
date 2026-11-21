#!/usr/bin/env bash
# run_check.sh: build the upload in a scratch dir, append check_part1.rs to the scratch copy of
# src/v16.rs (never to the upload or the repo), run the random test. A test, not a proof.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
W=${1:-/tmp/e2e-toly-1121/scratch_check}
rm -rf "$W"
"$HERE/../tools/make_upload.sh" "$W" > /dev/null
cat "$HERE/check_part1.rs" >> "$W/src/v16.rs"
cd "$W"
CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-/tmp/e2e-toly-1121/target_check} cargo test --release --offline -j 4 --lib ours2_v3_part1_check 2>&1 | grep -E '^test |test result|error|panicked' 
