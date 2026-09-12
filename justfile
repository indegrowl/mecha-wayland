# `just` with no arguments lists the recipes.
default:
    @just --list

# Format every crate in the workspace.
fmt:
    cargo fmt --all

# Fail if any file is not formatted.
fmt-check:
    cargo fmt --all --check

# Run every test in the workspace.
test:
    cargo test --workspace

# What must pass before a commit: formatting, then tests.
check: fmt-check test

# Install the git pre-commit hook.
setup:
    ln -sf ../../scripts/pre-commit.sh .git/hooks/pre-commit
    @echo "pre-commit hook installed"
