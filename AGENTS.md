# AGENTS.md - diesel-guard

This document provides context for AI coding agents working on **diesel-guard**. It covers architecture, implementation patterns, and conventions for maintaining consistency across contributions.

## Project Overview

**diesel-guard** detects unsafe PostgreSQL migration patterns before they cause production incidents. It parses SQL using `sqlparser` and identifies operations that acquire dangerous locks or trigger table rewrites.

**Core Technology:**
- Language: Rust
- SQL Parser: `sqlparser`
- Frameworks: Diesel and SQLx migrations
- Target: PostgreSQL 9.6+

## Architecture

```
src/
├── lib.rs                # Main library exports
├── main.rs               # CLI entry point (check, init commands)
├── safety_checker.rs     # Main checker that processes files/directories
├── violation.rs          # Violation struct with operation/problem/solution
├── error.rs              # Error handling with miette
├── output.rs             # Output formatting (text/JSON)
├── config.rs             # Configuration loading/validation
├── checks/               # One file per check + mod.rs (registry) + test_utils.rs (macros)
├── parser/               # SQL parsing + regex detectors for syntax sqlparser can't handle
└── adapters/             # Framework adapters
    ├── mod.rs            # MigrationAdapter trait
    ├── diesel.rs         # Diesel migration handling
    └── sqlx.rs           # SQLx migration handling

tests/
├── fixtures/             # Diesel migration fixtures
├── fixtures_sqlx/        # SQLx migration fixtures
├── fixtures_test.rs      # Fixture integration tests
├── safety_assured_test.rs # Safety-assured block tests
├── config_test.rs        # Configuration tests
└── init_test.rs          # Init command tests
```

**Key Components:**
- **Check trait**: All safety checks implement this trait (`fn check(&self, stmt: &Statement) -> Vec<Violation>`)
- **Registry**: Holds all registered checks, filters by config, runs them against statements
- **SafetyChecker**: Main API for checking files/directories (uses `&Utf8Path` from camino, not `&Path`)
- **Violation**: Contains operation name, problem description, and safe alternative
- **MigrationAdapter trait**: Abstracts framework-specific migration discovery (Diesel, SQLx)
- **Config**: Loads settings from `diesel-guard.toml` (framework, start_after, disable_checks)

**REINDEX is a special case:** sqlparser cannot parse REINDEX at all, so detection happens via raw SQL regex in `SafetyChecker::detect_reindex_violations()` BEFORE parsing. The `ReindexCheck` struct in `src/checks/reindex.rs` is a **stub** (always returns empty) — it exists solely so `disable_checks = ["ReindexCheck"]` works. If you need to add another check for syntax sqlparser can't parse, follow this same pattern.

## How to Add a New Check

Follow these 7 steps for consistent implementation:

### 1. Create the Check Module

Create `src/checks/your_check.rs`:

```rust
//! Detection for YOUR_OPERATION.
//!
//! Document: lock type, table rewrite behavior, and PostgreSQL version specifics.

use crate::checks::Check;
use crate::violation::Violation;
use sqlparser::ast::{Statement, /* relevant AST types */};

pub struct YourCheck;

impl Check for YourCheck {
    fn check(&self, stmt: &Statement) -> Vec<Violation> {
        let mut violations = vec![];

        // Pattern match on Statement and extract relevant parts
        if let Statement::YourPattern { ... } = stmt {
            violations.push(Violation::new(
                "OPERATION NAME",
                "Problem description: lock type, duration factors",
                "Safe alternative: numbered steps with code examples",
            ));
        }

        violations
    }
}
```

**Critical Requirements:**
- Module-level documentation (//!) explaining the check
- Accurate lock type specification (ACCESS EXCLUSIVE, SHARE, SHARE UPDATE EXCLUSIVE)
- Qualified duration claims ("depends on table size" NOT "takes hours")
- Multi-step solutions with actual SQL code examples

### 2. Add Unit Tests

In the same file, add tests using shared macros:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_allows, assert_detects_violation};

    #[test]
    fn test_detects_your_operation() {
        assert_detects_violation!(
            YourCheck,
            "SQL statement that should be detected;",
            "OPERATION NAME"
        );
    }

    #[test]
    fn test_ignores_safe_variant() {
        assert_allows!(YourCheck, "Safe SQL statement;");
    }
}
```

**Available Test Macros** (from `src/checks/test_utils.rs`):
- `assert_detects_violation!(check, sql, expected_operation)` - Asserts exactly 1 violation with matching operation
- `assert_allows!(check, sql)` - Asserts no violations found
- Both macros handle SQL parsing automatically

**When to use explicit tests:** For complex assertions (e.g., checking `violation.problem` contains specific text), write explicit test code instead of using macros. See `add_index.rs:test_detects_create_unique_index_without_concurrently` for example.

### 3. Register the Check

Update `src/checks/mod.rs`:

```rust
// 1. Add module declaration (alphabetically)
mod your_check;

// 2. Add public export (alphabetically)
pub use your_check::YourCheck;

// 3. Add to register_enabled_checks() method (alphabetically)
fn register_enabled_checks(&mut self, config: &Config) {
    // ... existing checks ...
    self.register_check(config, YourCheck);
}
```

**Note**: Check names are automatically extracted at runtime using `std::any::type_name`. No need to manually update a constant - the check name will be derived from the struct name (e.g., `YourCheck` becomes `"YourCheck"`).

### 4. Create Test Fixtures

Create fixture directories:

```bash
mkdir -p tests/fixtures/your_operation_unsafe
mkdir -p tests/fixtures/your_operation_safe  # if applicable
```

**Fixture Naming Convention (MUST follow):**

Pattern: `{operation}_{safe|unsafe}` or `{operation}_{variant}_{safe|unsafe}`

| Pattern | Example | Use Case |
|---------|---------|----------|
| `{op}_safe` | `add_index_safe` | Safe variant of operation |
| `{op}_unsafe` | `add_index_unsafe` | Unsafe variant of operation |
| `{op}_{variant}_unsafe` | `alter_column_type_using_unsafe` | Variant with specific behavior |
| `{op}_{variant}_safe` | `drop_not_null_safe` | Safe variant with specific behavior |

**Fixture File Content:**

Each `up.sql` MUST start with a comment describing safe/unsafe status:
- Format: `-- Unsafe: Brief description` or `-- Safe: Brief description`
- See existing fixtures for examples

**Safe/Unsafe Pairs:**

Most checks should have both `_safe` and `_unsafe` fixtures. Exceptions:
- Operations with no safe alternative in migrations (e.g., `create_extension_unsafe`, `truncate_table_unsafe`)
- Parser limitation tests (e.g., `unique_using_index_parser_limitation`)

Add migration files:
- `tests/fixtures/your_operation_unsafe/up.sql` - Example that should be detected
- `tests/fixtures/your_operation_safe/up.sql` - Example that should pass

**Special Case - CONCURRENTLY operations:**
If safe variant requires `run_in_transaction = false` (like CREATE INDEX CONCURRENTLY), add:
- `tests/fixtures/your_operation_safe/metadata.toml` with `run_in_transaction = false`

### 5. Update Integration Tests

In `tests/fixtures_test.rs`:
- Add safe fixture to `safe_fixtures` vec in `test_safe_fixtures_pass`
- Add a specific detection test for the unsafe variant (see existing tests for pattern — uses `Utf8Path::new`, not `Path::new`)
- Update `test_check_entire_fixtures_directory` counts (see Pitfall #2 below)

### 6. Update README

Add to "Supported Checks" section with Unsafe/Safe SQL examples. Remove from "Coming Soon" if it was listed there.

### 7. Verify Everything

```bash
cargo test           # All tests pass
cargo fmt            # Code is formatted
cargo clippy --all-targets --all-features -- -D warnings  # No warnings
```

## Code Style & Conventions

### Lock Type Accuracy

Be precise about PostgreSQL lock types:

- **ACCESS EXCLUSIVE**: Blocks everything (ADD/DROP COLUMN, ALTER TYPE, ADD NOT NULL)
- **SHARE**: Blocks writes only (CREATE INDEX without CONCURRENTLY)
- **SHARE UPDATE EXCLUSIVE**: Allows reads/writes, blocks schema changes (VALIDATE CONSTRAINT)

### Violation Description Writing

✅ **Good:**
- "requires a full table scan to verify..."
- "Duration depends on table size"
- "acquires ACCESS EXCLUSIVE lock, blocking all operations"

❌ **Avoid:**
- "will lock the table for hours..."
- "can take significant time..." (too vague)
- Absolute time claims without qualification

### Solution Format (MUST follow)

Violation solutions MUST use numbered steps with actual SQL examples. This is a strict requirement for consistency.

**Required structure:**
1. Each step must be numbered
2. Each step must include the actual SQL command
3. Include explanation text between steps as needed
4. Always end with a safety-assured escape hatch option

**Reference:** See `char_type.rs` for a well-formatted example.

**Common mistakes to avoid:**
- Providing SQL without numbered steps
- Using prose paragraphs instead of structured steps
- Missing the safety-assured escape hatch option

### Naming Conventions

- **Check structs**: `YourOperationCheck` (descriptive, ends with "Check")
- **Test functions**:
  - `test_detects_*` - Detection tests (e.g., `test_detects_char_column_alter_table`)
  - `test_allows_*` - Safe variants within check's domain (e.g., `test_allows_varchar_column`)
  - `test_ignores_*` - Unrelated operations outside check's domain (e.g., `test_ignores_other_statements`)
- **Fixture directories**: `your_operation_unsafe`, `your_operation_safe`

## sqlparser AST Patterns

Before implementing a new check, search for similar patterns in existing checks:

```bash
rg "AlterTableOperation::" --type rust
rg "CreateIndex" --type rust
```

**Avoid nested if-let** (clippy `collapsible_match` warning) — collapse into single pattern match. See existing checks for examples.

## Common Pitfalls

### 1. Forgetting Registry Updates

**Symptom**: New check doesn't run
**Fix**: Add the check to `register_enabled_checks()` method in `src/checks/mod.rs`. The check name is automatically extracted, so no manual constant updates needed.

### 2. Incorrect Fixture Counts

**Symptom**: `test_check_entire_fixtures_directory()` fails
**Fix**: Update the file count and total violation count assertions in that test. Some fixtures produce multiple violations due to check overlaps — read the test's assertion message for the breakdown.

### 3. Macros After Test Module

**Symptom**: `clippy::items_after_test_module` warning
**Fix**: Keep macros before `mod test_helpers` in `test_utils.rs`

## Parser: Custom Detection for Unsupported Syntax

sqlparser cannot parse certain PostgreSQL-specific patterns. Regex-based detectors in `src/parser/` handle these. Each detector uses `LazyLock<Regex>` with `(?i)` flag.

**Detection strategy:** On parse error, `parse_with_metadata()` checks SQL against safe pattern detectors. If a safe pattern matches, it returns empty statements (no violations). Otherwise, it returns the original parse error.

**Limitation:** When a safe pattern causes a parse error, ALL statements in the file are skipped (sqlparser fails on the entire file).

**When to add a new detector:**
1. The syntax is PostgreSQL-specific and sqlparser doesn't support it
2. The pattern is well-defined and reliably detectable with regex
3. Create detector in `src/parser/`, update `parse_with_metadata()` in `src/parser/mod.rs`

## Safety-Assured Blocks

Users can wrap SQL in `-- safety-assured:start` / `-- safety-assured:end` blocks to bypass checks. Directives are case-insensitive. Line ranges are exclusive of the comment lines themselves. Statement-to-line mapping is heuristic-based (keyword matching) since sqlparser doesn't preserve source positions.

## Framework Adapters

The tool supports multiple migration frameworks through the `MigrationAdapter` trait.

### Diesel Adapter (`src/adapters/diesel.rs`)

**Migration Structure:**
```
migrations/
└── YYYYMMDDHHMMSS_description/
    ├── up.sql
    ├── down.sql        # Optional, checked if check_down=true
    └── metadata.toml   # Optional, for run_in_transaction setting
```

**Features:**
- Directory-based migrations with timestamp prefix
- `metadata.toml` support for `run_in_transaction = false` (required for CONCURRENTLY)
- Timestamp formats: `YYYYMMDDHHMMSS` or `YYYY_MM_DD_HHMMSS` (separators normalized)

### SQLx Adapter (`src/adapters/sqlx.rs`)

Supports four migration formats:

**1. Suffix-based (most common):**
```
migrations/
├── YYYYMMDDHHMMSS_description.up.sql
└── YYYYMMDDHHMMSS_description.down.sql
```

**2. Single-file with markers:**
```sql
-- migrations/YYYYMMDDHHMMSS_description.sql
-- migrate:up
CREATE TABLE users (...);

-- migrate:down
DROP TABLE users;
```

**3. Directory-based (Diesel-compatible):**
```
migrations/
└── YYYYMMDDHHMMSS_description/
    ├── up.sql
    └── down.sql
```

**4. Simple single-file (up-only):**
```
migrations/
└── YYYYMMDDHHMMSS_description.sql
```

**SQLx-specific Features:**
- `-- no-transaction` comment directive for CONCURRENTLY operations
- Direction-aware parsing for marker-based format (`parse_sql_with_direction()`)
- All four formats auto-detected

### Configuration

`diesel-guard.toml` settings:

```toml
framework = "diesel"           # Required: "diesel" or "sqlx"
start_after = "20240101000000" # Skip migrations before timestamp
check_down = false             # Also check down.sql files
disable_checks = ["AddColumnCheck"]  # Disable specific checks
```

**Timestamp normalization:** Separators (underscores, hyphens) are stripped for comparison.

## Verification

```bash
cargo test                                                    # all tests pass
cargo clippy --all-targets --all-features -- -D warnings      # no warnings
cargo fmt --check                                             # formatted
```
