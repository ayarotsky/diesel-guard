# Claude Guide for diesel_guard

This document helps AI assistants (and developers) understand the project structure and how to add new safety checks.

## Project Overview

**diesel_guard** detects unsafe PostgreSQL migration patterns in Diesel migrations before they cause production incidents. It parses SQL using `sqlparser` and identifies operations that acquire dangerous locks or trigger table rewrites.

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

## How to Add a New Check

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

**Key Points:**
- Use descriptive module-level documentation (//!)
- Be accurate about lock types (ACCESS EXCLUSIVE, SHARE, SHARE UPDATE EXCLUSIVE)
- Qualify duration claims ("depends on table size" not "takes hours")
- Solutions should be multi-step with actual SQL examples

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

**Available Macros** (see `src/checks/test_utils.rs`):
- `assert_detects_violation!(check, sql, expected_operation)` - Asserts exactly 1 violation with matching operation name
- `assert_allows!(check, sql)` - Asserts no violations found
- Both macros handle SQL parsing automatically

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

**Special Case:** If the safe variant requires `run_in_transaction = false` (like CONCURRENTLY), add:
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

// Update test_check_entire_fixtures_directory() counts
// - Update total fixture count
// - Update unsafe fixture count
// - Update total violations count
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
cargo test           # All tests pass
cargo fmt            # Code is formatted
cargo clippy --all-targets --all-features -- -D warnings  # No warnings
```

## Important Patterns & Conventions

### Lock Type Accuracy
- **ACCESS EXCLUSIVE**: Blocks everything (ADD/DROP COLUMN, ALTER TYPE, ADD NOT NULL)
- **SHARE**: Blocks writes only (CREATE INDEX without CONCURRENTLY)
- **SHARE UPDATE EXCLUSIVE**: Allows reads/writes, blocks schema changes (VALIDATE CONSTRAINT)

### Description Writing
- ✅ "requires a full table scan to verify..."
- ❌ "will lock the table for hours..."
- ✅ "Duration depends on table size"
- ❌ Absolute time claims without qualification

### Solution Format
Use numbered steps with actual SQL:
```
1. Description of first step:
   SQL CODE HERE;

2. Description of second step:
   SQL CODE HERE;
```

### Test Macro Usage
Prefer macros for simple cases (14/29 unit tests use them). For complex assertions (checking violation.problem contains specific text), use explicit code - see `add_index.rs:test_detects_create_unique_index_without_concurrently`.

### sqlparser AST Research
Before implementing, use `Grep` to find similar patterns:
```bash
# Find how other checks use AlterTableOperation
rg "AlterTableOperation::" --type rust
```

Common AST patterns:
- `Statement::AlterTable { name, operations, .. }` - ALTER TABLE operations
- `Statement::CreateIndex(create_index)` - CREATE INDEX
- `AlterTableOperation::AlterColumn { column_name, op }` - ALTER COLUMN
- `AlterColumnOperation::SetNotNull` - SET NOT NULL
- `AlterColumnOperation::SetDataType { data_type, using, .. }` - ALTER TYPE

## Testing Strategy

**Unit Tests** (`src/checks/*.rs`):
- Test detection of unsafe patterns
- Test that safe variants are allowed
- Test edge cases (IF EXISTS, multiple columns, etc.)

**Integration Tests** (`tests/fixtures_test.rs`):
- Verify real migration files work end-to-end
- Test safe fixtures produce no violations
- Test unsafe fixtures produce expected violations
- Test directory scanning

## Common Pitfalls

1. **Forgetting to update CheckRegistry count** - Update `test_registry_creation()` when adding checks
2. **Incorrect fixture counts** - Update `test_check_entire_fixtures_directory()` when adding fixtures
3. **Nested pattern matching** - Clippy warns about collapsible matches; combine into single pattern
4. **Macros after test module** - Clippy warns; keep macros before `mod test_helpers` in test_utils.rs
5. **Exaggerated descriptions** - Be technically accurate about lock behavior and duration

## Current State

- **5 checks implemented**: ADD COLUMN with DEFAULT, ADD INDEX without CONCURRENTLY, ADD NOT NULL, ALTER COLUMN TYPE, DROP COLUMN
- **11 fixtures**: 3 safe, 9 unsafe
- **40 tests total**: 29 unit + 11 integration
- **All checks passing** with cargo test, fmt, and clippy
