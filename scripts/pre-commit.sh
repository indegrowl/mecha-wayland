#!/usr/bin/env bash
set -euo pipefail

echo "[pre-commit] Checking formatting."
if ! cargo fmt --all --check; then
    echo ""
    echo "[pre-commit] Formatting issues found. Run 'just fmt' to fix them."
    exit 1
fi

echo "[pre-commit] Running tests."
if ! cargo test --workspace; then
    echo ""
    echo "[pre-commit] Tests failed."
    exit 1
fi
