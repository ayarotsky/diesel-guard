# AGENTS.md - diesel_guard

This document provides context for AI coding agents working on diesel_guard. It covers architecture, implementation patterns, and conventions for maintaining consistency across contributions.

## Project Overview

**diesel_guard** detects unsafe PostgreSQL migration patterns in Diesel migrations before they cause production incidents. It parses SQL using `sqlparser` and identifies operations that acquire dangerous locks or trigger table rewrites.

**Core Technology:**
- Language: Rust
- SQL Parser: `sqlparser` (v0.59.0)
- Framework: Diesel ORM migrations
- Target: PostgreSQL 9.6+

## Architecture

```
src/
├── checks/           # Individual safety checks
│   ├── mod.rs       # CheckRegistry that runs all checks
│   ├── test_utils.rs # Shared test macros (assert_detects_violation!, assert_allows!)
│   ├── add_column.rs
│   ├── add_index.rs
│   ├── add_not_null.rs
│   ├── alter_column_type.rs
│   └── drop_column.rs
├── parser.rs        # SQL parsing wrapper
├── safety_checker.rs # Main checker that processes files/directories
└── violation.rs     # Violation struct with operation/problem/solution

tests/
├── fixtures/        # Test migration files (11 fixtures: 3 safe, 9 unsafe)
└── fixtures_test.rs # Integration tests
```

**Key Components:**
- **Check trait**: All safety checks implement this trait (`fn check(&self, stmt: &Statement) -> Result<Vec<Violation>>`)
- **CheckRegistry**: Holds all registered checks and runs them against statements
- **SafetyChecker**: Main API for checking files/directories
- **Violation**: Contains operation name, problem description, and safe solution

## How to Add a New Check

Follow these 7 steps for consistent implementation:

### 1. Create the Check Module

Create `src/checks/your_check.rs`:

```rust
//! Detection for YOUR_OPERATION.
//!
//! Document: lock type, table rewrite behavior, and PostgreSQL version specifics.

use crate::checks::Check;
use crate::error::Result;
use crate::violation::Violation;
use sqlparser::ast::{Statement, /* relevant AST types */};

pub struct YourCheck;

impl Check for YourCheck {
    fn name(&self) -> &str {
        "your_check_name"
    }

    fn check(&self, stmt: &Statement) -> Result<Vec<Violation>> {
        let mut violations = vec![];

        // Pattern match on Statement and extract relevant parts
        if let Statement::YourPattern { ... } = stmt {
            violations.push(Violation::new(
                "OPERATION NAME",
                "Problem description: lock type, duration factors",
                "Safe alternative: numbered steps with code examples",
            ));
        }

        Ok(violations)
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

// 3. Add to registry in CheckRegistry::new()
checks: vec![
    // ... existing checks ...
    Box::new(YourCheck),
]

// 4. Update test count in test_registry_creation()
assert_eq!(registry.checks.len(), N); // where N = total checks
```

### 4. Create Test Fixtures

Create fixture directories:

```bash
mkdir -p tests/fixtures/your_operation_unsafe
mkdir -p tests/fixtures/your_operation_safe  # if applicable
```

Add migration files:
- `tests/fixtures/your_operation_unsafe/up.sql` - Example that should be detected
- `tests/fixtures/your_operation_safe/up.sql` - Example that should pass

**Special Case - CONCURRENTLY operations:**
If safe variant requires `run_in_transaction = false` (like CREATE INDEX CONCURRENTLY), add:
- `tests/fixtures/your_operation_safe/metadata.toml` with `run_in_transaction = false`

### 5. Update Integration Tests

In `tests/fixtures_test.rs`:

```rust
// Add to safe_fixtures list if applicable
let safe_fixtures = vec![
    // ... existing ...
    "your_operation_safe",
];

// Add specific test for unsafe variant
#[test]
fn test_your_operation_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("your_operation_unsafe");
    let violations = checker.check_file(Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].operation, "OPERATION NAME");
}

// Update test_check_entire_fixtures_directory() counts:
// - Total fixture count
// - Unsafe fixture count
// - Total violations count
```

### 6. Update README

Add to "Currently Supported Checks" section:

```markdown
### N. YOUR CHECK NAME

**Unsafe:**
```sql
-- SQL that triggers detection
```

**Safe:**
```sql
-- Multi-step safe alternative
```

**Note:** Any important details about PostgreSQL versions, lock types, or edge cases.
```

Remove from "Coming Soon" if it was listed there.

### 7. Verify Everything

```bash
cargo test           # All tests pass (currently 40 tests)
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

### Solution Format

Use numbered steps with actual SQL examples:

```
1. Description of first step:
   SQL CODE HERE;

2. Description of second step:
   SQL CODE HERE;
```

### Test Macro Usage

- **Prefer macros** for simple detection tests (currently 14/29 unit tests use them)
- **Use explicit code** for complex assertions that check violation message content
- See existing checks for examples of both approaches

### Naming Conventions

- **Check structs**: `YourOperationCheck` (descriptive, ends with "Check")
- **Check name method**: `"your_operation"` (snake_case, matches operation)
- **Test functions**: `test_detects_your_operation`, `test_ignores_safe_variant`
- **Fixture directories**: `your_operation_unsafe`, `your_operation_safe`

## sqlparser AST Patterns

### Research Existing Patterns

Before implementing, search for similar patterns:

```bash
# Find how other checks use AlterTableOperation
rg "AlterTableOperation::" --type rust

# Find CreateIndex usage
rg "CreateIndex" --type rust
```

### Common AST Patterns

- `Statement::AlterTable { name, operations, .. }` - ALTER TABLE operations
- `Statement::CreateIndex(create_index)` - CREATE INDEX
- `Statement::Drop { object_type, .. }` - DROP operations
- `AlterTableOperation::AlterColumn { column_name, op }` - ALTER COLUMN
- `AlterTableOperation::AddColumn { column_def }` - ADD COLUMN
- `AlterTableOperation::DropColumn { column_names, .. }` - DROP COLUMN
- `AlterColumnOperation::SetNotNull` - SET NOT NULL
- `AlterColumnOperation::SetDataType { data_type, using, .. }` - ALTER TYPE
- `ColumnOption::Default(_)` - DEFAULT value on column

### Pattern Matching Best Practices

**Avoid nested if-let** (clippy warning):

```rust
// ❌ Bad - nested pattern matching
if let AlterTableOperation::AlterColumn { column_name, op } = op {
    if let AlterColumnOperation::SetDataType { data_type, using, .. } = op {
        // ...
    }
}

// ✅ Good - collapsed pattern
if let AlterTableOperation::AlterColumn {
    column_name,
    op: AlterColumnOperation::SetDataType { data_type, using, .. },
} = op {
    // ...
}
```

## Testing Strategy

### Unit Tests (`src/checks/*.rs`)

Each check module includes:
- Detection of unsafe patterns
- Verification that safe variants are allowed
- Edge cases (IF EXISTS, multiple columns, etc.)
- Operation-specific scenarios (USING clause, UNIQUE indexes, etc.)

**Test coverage goal**: Every code path in the `check()` method should have a test.

### Integration Tests (`tests/fixtures_test.rs`)

Validates end-to-end behavior:
- Safe fixtures produce zero violations
- Unsafe fixtures produce expected violations
- Directory scanning works correctly
- Fixture counts match expectations

**Current state**: 11 fixtures (3 safe, 9 unsafe) with 11 integration tests

## Common Pitfalls

### 1. Forgetting CheckRegistry Updates

**Symptom**: New check doesn't run
**Fix**: Update `CheckRegistry::new()` and `test_registry_creation()` count

### 2. Incorrect Fixture Counts

**Symptom**: `test_check_entire_fixtures_directory()` fails
**Fix**: Update total fixtures, unsafe count, and total violations count in test comments and assertions

### 3. Nested Pattern Matching

**Symptom**: `clippy::collapsible_match` warning
**Fix**: Combine nested `if let` into single pattern (see pattern matching section above)

### 4. Macros After Test Module

**Symptom**: `clippy::items_after_test_module` warning
**Fix**: Keep macros before `mod test_helpers` in `test_utils.rs`

### 5. Exaggerated Descriptions

**Symptom**: Violations sound alarmist or inaccurate
**Fix**: Use precise lock types, qualify duration statements, avoid absolute claims

### 6. Missing Fixture metadata.toml

**Symptom**: Safe CONCURRENTLY operation not tested correctly
**Fix**: Add `metadata.toml` with `run_in_transaction = false` for CONCURRENTLY operations

## Current Project State

- **Checks implemented**: 5
  - ADD COLUMN with DEFAULT
  - ADD INDEX without CONCURRENTLY
  - ADD NOT NULL constraint
  - ALTER COLUMN TYPE
  - DROP COLUMN

- **Test fixtures**: 11 (3 safe, 9 unsafe)

- **Test coverage**: 40 tests total
  - 29 unit tests
  - 11 integration tests

- **Code quality**: All passing
  - ✅ `cargo test`
  - ✅ `cargo fmt --check`
  - ✅ `cargo clippy --all-targets --all-features -- -D warnings`

- **Planned checks**: 18 checks in Coming Soon (Phase 2)
  - See README.md for complete list

## Dependencies

- **sqlparser**: v0.59.0 - SQL parsing
- **colored**: v3.0.0 - Terminal output formatting
- **thiserror**: v2.0.17 - Error handling
- **toml**: v0.9.8 - Metadata file parsing

## Build & Development Commands

```bash
# Run all tests
cargo test

# Run only unit tests
cargo test --lib

# Run only integration tests
cargo test --test fixtures_test

# Run tests for specific check
cargo test add_column
cargo test add_index

# Format code
cargo fmt

# Lint code
cargo clippy --all-targets --all-features -- -D warnings

# Build release binary
cargo build --release

# Initialize config file (for testing)
cargo run -- init
cargo run -- init --force  # overwrite existing

# Check migrations
cargo run -- check tests/fixtures/
```

## Additional Resources

- **CONTRIBUTING.md**: Human contributor guide, PR process, community guidelines
- **README.md**: User-facing documentation, usage examples, supported checks
- **tests/fixtures/**: Example migrations demonstrating safe and unsafe patterns

---

**For human contributors**: See CONTRIBUTING.md for development setup and PR guidelines.
