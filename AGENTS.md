# AGENTS.md - diesel-guard

This document provides context for AI coding agents working on **diesel-guard**. It covers architecture, implementation patterns, and conventions for maintaining consistency across contributions.

## Project Overview

**diesel-guard** detects unsafe PostgreSQL migration patterns before they cause production incidents. It parses SQL using `sqlparser` and identifies operations that acquire dangerous locks or trigger table rewrites.

**Core Technology:**
- Version: 0.4.0
- Language: Rust
- SQL Parser: `sqlparser` (v0.60.0)
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
├── checks/               # Individual safety checks (23 checks)
│   ├── mod.rs            # Registry that runs all checks
│   ├── test_utils.rs     # Shared test macros (assert_detects_violation!, assert_allows!)
│   ├── add_column.rs
│   ├── add_index.rs
│   ├── add_json_column.rs
│   ├── add_not_null.rs
│   ├── add_primary_key.rs
│   ├── add_serial_column.rs
│   ├── add_unique_constraint.rs
│   ├── alter_column_type.rs
│   ├── char_type.rs
│   ├── create_extension.rs
│   ├── drop_column.rs
│   ├── drop_database.rs
│   ├── drop_index.rs
│   ├── drop_primary_key.rs
│   ├── drop_table.rs
│   ├── generated_column.rs
│   ├── rename_column.rs
│   ├── rename_table.rs
│   ├── short_int_primary_key.rs
│   ├── timestamp_type.rs
│   ├── truncate_table.rs
│   ├── unnamed_constraint.rs
│   └── wide_index.rs
├── parser/               # SQL parsing
│   ├── mod.rs            # SqlParser with custom detection fallbacks
│   ├── comment_parser.rs # Safety-assured block parsing
│   ├── drop_index_concurrently_detector.rs  # Safe DROP INDEX CONCURRENTLY detection
│   ├── primary_key_using_index_detector.rs  # Safe PRIMARY KEY USING INDEX detection
│   └── unique_using_index_detector.rs       # Safe UNIQUE USING INDEX detection
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
- **SafetyChecker**: Main API for checking files/directories
- **Violation**: Contains operation name, problem description, and safe alternative
- **MigrationAdapter trait**: Abstracts framework-specific migration discovery (Diesel, SQLx)
- **Config**: Loads settings from `diesel-guard.toml` (framework, start_after, disable_checks)

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

**Examples of correct naming:**
- `add_column_with_default_unsafe` (not `add_column_with_default`)
- `add_index_safe` (not `add_index_with_concurrently`)
- `drop_column_unsafe` (not `drop_column`)
- `alter_column_type_using_unsafe` (not `alter_column_type_with_using`)

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
```

### 6. Update README

Add to "Supported Checks" section:

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

### Test Macro Usage

- **Prefer macros** for simple detection tests (currently 14/29 unit tests use them)
- **Use explicit code** for complex assertions that check violation message content
- See existing checks for examples of both approaches

### Naming Conventions

- **Check structs**: `YourOperationCheck` (descriptive, ends with "Check")
- **Test functions**:
  - `test_detects_*` - Detection tests (e.g., `test_detects_char_column_alter_table`)
  - `test_allows_*` - Safe variants within check's domain (e.g., `test_allows_varchar_column`)
  - `test_ignores_*` - Unrelated operations outside check's domain (e.g., `test_ignores_other_statements`)
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

### Integration Tests (`tests/`)

**`tests/fixtures_test.rs`** - Diesel fixture tests:
- Safe fixtures produce zero violations
- Unsafe fixtures produce expected violations
- Directory scanning works correctly
- Fixture counts match expectations

**`tests/safety_assured_test.rs`** - Safety-assured block tests:
- End-to-end checking with blocks
- Multiple blocks in one file
- Edge cases (interleaved blocks, same keywords in/out of blocks)

**`tests/config_test.rs`** - Configuration tests:
- Loading and validation of `diesel-guard.toml`
- Framework validation (diesel/sqlx)
- Check name validation against registry
- Timestamp format validation

**`tests/init_test.rs`** - CLI init command tests:
- Config file creation
- Force overwrite behavior

**SQLx fixtures** (`tests/fixtures_sqlx/`) test all four SQLx migration formats.

## Common Pitfalls

### 1. Forgetting Registry Updates

**Symptom**: New check doesn't run
**Fix**: Add the check to `register_enabled_checks()` method in `src/checks/mod.rs`. The check name is automatically extracted, so no manual constant updates needed.

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

## Parser Implementation

### Custom Detection for Unsupported Syntax

sqlparser 0.60 cannot parse certain PostgreSQL-specific safe patterns. Three detectors handle these:

**1. UNIQUE USING INDEX** (`src/parser/unique_using_index_detector.rs`):
```sql
ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE USING INDEX users_email_idx;
```
- Safe pattern: promotes existing index to constraint without table lock
- Pattern: `(?i)ALTER\s+TABLE\s+\S+\s+ADD\s+CONSTRAINT\s+\S+\s+UNIQUE\s+USING\s+INDEX\s+\S+`

**2. PRIMARY KEY USING INDEX** (`src/parser/primary_key_using_index_detector.rs`):
```sql
ALTER TABLE users ADD CONSTRAINT users_pkey PRIMARY KEY USING INDEX users_id_idx;
```
- Safe pattern: promotes existing unique index to primary key without table lock
- Pattern: `(?i)ALTER\s+TABLE\s+\S+\s+ADD\s+CONSTRAINT\s+\S+\s+PRIMARY\s+KEY\s+USING\s+INDEX\s+\S+`

**3. DROP INDEX CONCURRENTLY** (`src/parser/drop_index_concurrently_detector.rs`):
```sql
DROP INDEX CONCURRENTLY users_email_idx;
```
- Safe pattern: non-blocking index removal
- Pattern: `(?i)DROP\s+INDEX\s+CONCURRENTLY`

**Detection Strategy:**
- When `parse_with_metadata()` encounters a parse error, it checks SQL against all three patterns
- If any safe pattern detected: Returns empty statements (no violations)
- If no pattern matches: Returns the original parse error

**Implementation Details:**
- Uses `std::sync::LazyLock` for regex compilation
- Case-insensitive matching via `(?i)` flag
- Each detector has comprehensive test coverage

**Important Limitation:**

⚠️ **Multi-statement files**: When a safe pattern causes a parse error, ALL statements in the file are skipped. This is because sqlparser fails on the entire file when it encounters unsupported syntax.

**Mitigation:**
- Migrations typically use one statement per file, so this is rarely an issue
- Users can split multi-statement files into separate migration files
- Use `-- safety-assured` blocks for known-safe multi-statement migrations

**When to Add Custom Detection:**

Only add regex-based detection when:
1. The syntax is PostgreSQL-specific and sqlparser doesn't support it
2. The syntax represents a **safe pattern** that should not trigger violations
3. The pattern is well-defined and can be reliably detected with regex

**How to Add Custom Detection:**
1. Create detector module in `src/parser/` with regex pattern and tests
2. Update `parse_with_metadata()` in `src/parser/mod.rs` to check for pattern on parse errors
3. Return empty statements if safe pattern detected, otherwise return original error
4. Add integration tests to verify the pattern is recognized correctly

## Safety-Assured Implementation

Users can wrap SQL in `-- safety-assured:start` / `-- safety-assured:end` blocks to bypass checks.

### Architecture

**Comment Parser** (`src/parser/comment_parser.rs`):
- Scans SQL line-by-line for directives
- Builds `IgnoreRange` structs with start/end line numbers
- Validates matching pairs (errors on unclosed/unmatched blocks)
- Simple start/end directives: `-- safety-assured:start` and `-- safety-assured:end`

**Parser** (`src/parser/mod.rs`):
- `parse_with_metadata()` returns `ParsedSql` with:
  - AST statements
  - Statement line numbers (heuristic-based)
  - Ignore ranges from comment parser
- `extract_statement_lines()` maps statements to source lines using keyword matching

**Registry** (`src/checks/mod.rs`):
- `check_statements_with_context()` filters checks based on ignore ranges
- `is_line_ignored()` checks if a line falls within any range
- All checks are bypassed for statements within safety-assured blocks

### Key Implementation Details

**Line Number Handling:**
- All line numbers are 1-indexed (matching editor conventions)
- Ignore ranges are exclusive of start/end comment lines
  - Line 5: `-- safety-assured:start`
  - Line 6-9: Statements (IGNORED)
  - Line 10: `-- safety-assured:end`

**Statement Line Extraction:**
- Heuristic-based since sqlparser doesn't preserve positions
- Searches for SQL keywords (ALTER, CREATE, DROP, etc.)
- Matches statements to lines in order of appearance
- Skips already-matched lines to handle multiple statements

**Directive Matching:**
- Directives are case-insensitive (`-- SAFETY-ASSURED:START` works)
- All checks are bypassed when a statement is within a block
- No support for check-specific ignoring (keeps implementation simple)

**Known Limitations:**
- **Statement line tracking is heuristic-based**: The `extract_statement_lines` method in `src/parser/mod.rs` uses keyword matching to identify where statements begin in the source SQL. This approach has some edge case limitations:
  - **Rare fallback to line 1**: If keyword matching fails (statement doesn't start with any known SQL keyword), the method defaults to line 1 and logs a warning to stderr. This should be rare in practice as the keyword list covers standard SQL operations.
  - **Impact if fallback occurs**: When fallback occurs, statements may be incorrectly included or excluded from safety-assured blocks depending on whether line 1 falls within a block's range.
  - **Edge cases**: Multiple statements on the same line, or statements with very unusual formatting, may not track correctly.
- **Nested blocks**: Allowed and work as sequential blocks due to stack behavior in comment parser
- **Debugging**: If fallback occurs, warnings are logged to stderr with the keyword and statement preview to help identify problematic SQL

### Safety-Assured Testing

**Unit tests** (`src/parser/comment_parser.rs`): Block parsing, case insensitivity, error cases

**Fixtures** (`tests/fixtures/safety_assured_*`): `safety_assured_drop`, `safety_assured_multiple`

**Edge cases**: See `tests/safety_assured_test.rs` for interleaved blocks, same keywords in/out of blocks, nested blocks

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

## Current Project State

- **Checks implemented**: 23
  - ADD COLUMN with DEFAULT (PostgreSQL < 11)
  - ADD INDEX without CONCURRENTLY
  - ADD JSON column (should use JSONB)
  - ADD NOT NULL constraint
  - ADD PRIMARY KEY to existing table
  - ADD SERIAL column
  - ADD UNIQUE constraint via ALTER TABLE
  - ALTER COLUMN TYPE
  - CHAR/CHARACTER column types
  - CREATE EXTENSION
  - DROP COLUMN
  - DROP DATABASE
  - DROP INDEX without CONCURRENTLY
  - DROP PRIMARY KEY
  - DROP TABLE
  - GENERATED STORED column (table rewrite)
  - RENAME COLUMN
  - RENAME TABLE
  - Short integer primary keys (SMALLINT/INT)
  - TIMESTAMP without time zone (recommend TIMESTAMPTZ)
  - TRUNCATE TABLE
  - Unnamed constraints (UNIQUE, FOREIGN KEY, CHECK)
  - Wide indexes (4+ columns)

- **Code quality**: All passing
  - ✅ `cargo test`
  - ✅ `cargo fmt --check`
  - ✅ `cargo clippy --all-targets --all-features -- -D warnings`

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
