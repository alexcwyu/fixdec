#!/usr/bin/env bash
#
# fixdec release quality gate.
#
# Runs the full check / lint / test matrix that must pass before a release.
# Mirrors the CI jobs in .github/workflows/rust.yml so the same gate can be run
# locally. Any failing step aborts with a non-zero exit.
#
# Usage:
#   ./scripts/quality-gate.sh            # full gate (fast differential)
#   DIFF_ITERS=1000000 ./scripts/quality-gate.sh   # heavier differential
#
set -euo pipefail
cd "$(dirname "$0")/.."

DIFF_ITERS="${DIFF_ITERS:-200000}"

run() {
    echo
    echo "==> $*"
    "$@"
}

# 1. Builds / checks across feature combinations (incl. no_std).
run cargo check --no-default-features
run cargo check --no-default-features --features full
run cargo check --no-default-features --features rkyv
run cargo check --all-features
run cargo check --benches --all-features

# 2. Tests.
run cargo test --all-features

# 3. Differential fuzz vs rust_decimal + integer oracle (add/sub/mul/div).
DIFF_ITERS="$DIFF_ITERS" run cargo test --release --all-features --test differential

# 4. sqrt differential vs rust_decimal + f64 + exact integer oracle.
run cargo run --release --example sqrt_diff -- "$DIFF_ITERS"

# 5. Lint gate: warnings are errors across the whole workspace.
run cargo clippy --all-features --all-targets -- -D warnings

echo
echo "quality gate: PASS"
