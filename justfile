# diesel-guard — task runner
# Run `just` (or `just --list`) to see available recipes.
#
# Install just: cargo install just
# Install dev tools: just install-tools

set shell := ["bash", "-cu"]

# List all available recipes
default:
    @just --list

# ── Lint & Format ─────────────────────────────────────────────────────────────

# Check formatting and lint
lint:
    cargo fmt --all -- --check
    cargo clippy --all-targets --all-features -- -D warnings

# Verify doc-comments compile
doc:
    cargo doc --no-deps

# ── Build ─────────────────────────────────────────────────────────────────────

# Compile a release build with audit metadata embedded
build-release:
    cargo auditable build --release

# ── Coverage ──────────────────────────────────────────────────────────────────

# Generate an lcov coverage report (requires cargo-tarpaulin; Linux-friendly)
coverage:
    cargo tarpaulin --all-features --out lcov --exclude-files src/main.rs src/checks/test_utils.rs

# ── Composite ─────────────────────────────────────────────────────────────────

# Fast pre-commit gate: lint + tests
check: lint test

# Full CI pipeline — mirrors ci.yml; run before opening a PR
ci: lint doc build-release test
    cargo deny check
    cargo audit
    @echo "CI pipeline passed locally."

# ── Testing ───────────────────────────────────────────────────────────────────

# Run the full test suite
test:
    cargo test --all-features

# ── Tool Installation ─────────────────────────────────────────────────────────

# Install tools required for development and CI (idempotent)
install-tools:
    cargo install --locked cargo-deny
    cargo install --locked cargo-audit
    cargo install --locked cargo-auditable
