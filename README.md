# Catch unsafe PostgreSQL migrations in Diesel before they take down production

## The Problem

Diesel migrations are powerful but dangerous. One bad migration can:
- Lock tables for hours
- Crash running app instances
- Cause data loss
- Take down production

Rails has 3 popular gems solving this (6.5k+ stars combined), but **Diesel has nothing**.

## Installation

```bash
cargo install diesel_guard
```

Or add to your project:

```bash
cargo install --path .
```

## Usage

### Check a single migration

```bash
diesel-guard check migrations/2024_01_01_create_users/up.sql
```

### Check all migrations

```bash
diesel-guard check migrations/
```

### JSON output for CI/CD

```bash
diesel-guard check migrations/ --format json
```

### Allow unsafe operations

```bash
diesel-guard check migrations/ --allow-unsafe
```

### Example Output

```
❌ Unsafe migration detected in migrations/2024_01_01_create_users/up.sql

❌ ADD COLUMN with DEFAULT

Problem:
  Adding column 'admin' with DEFAULT locks table 'users' while backfilling on PostgreSQL < 11.
  This can take hours on large tables and block all reads/writes.

Safe alternative:
  1. Add the column without a default:
     ALTER TABLE users ADD COLUMN admin BOOLEAN;

  2. Backfill data in batches (outside migration):
     UPDATE users SET admin = <value> WHERE admin IS NULL;

  3. Add default for new rows only:
     ALTER TABLE users ALTER COLUMN admin SET DEFAULT <value>;

  Note: For PostgreSQL 11+, this is safe if the default is a constant value.

❌ 1 unsafe migration(s) detected
```

## Currently Supported Checks

### ADD COLUMN with DEFAULT

**Unsafe:**
```sql
ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;
```

**Safe:**
```sql
-- Step 1: Add column without default
ALTER TABLE users ADD COLUMN admin BOOLEAN;

-- Step 2: Backfill in batches (outside migration)
UPDATE users SET admin = FALSE WHERE admin IS NULL;

-- Step 3: Add default for new rows
ALTER TABLE users ALTER COLUMN admin SET DEFAULT FALSE;
```

### DROP COLUMN

**Unsafe:**
```sql
ALTER TABLE users DROP COLUMN email;
```

**Safe:**
```sql
-- Step 1: Mark column as unused in application code

-- Step 2: Deploy application without column references

-- Step 3: (Optional) Set column to NULL to reclaim space
ALTER TABLE users ALTER COLUMN email DROP NOT NULL;
UPDATE users SET email = NULL;

-- Step 4: Drop in later migration after confirming unused
ALTER TABLE users DROP COLUMN email;
```

### ADD INDEX without CONCURRENTLY

**Unsafe:**
```sql
CREATE INDEX idx_users_email ON users(email);
CREATE UNIQUE INDEX idx_users_username ON users(username);
```

**Safe:**
```sql
-- Use CONCURRENTLY to avoid locking the table
CREATE INDEX CONCURRENTLY idx_users_email ON users(email);
CREATE UNIQUE INDEX CONCURRENTLY idx_users_username ON users(username);
```

**Important:** Because CONCURRENTLY cannot be run inside a transaction block, you need to add a `metadata.toml` file to your migration directory:

```toml
# migrations/2024_01_01_add_user_index/metadata.toml
run_in_transaction = false
```

Without this configuration, Diesel will try to run the migration in a transaction and it will fail.

### ALTER COLUMN TYPE

**Unsafe:**
```sql
ALTER TABLE users ALTER COLUMN age TYPE BIGINT;
ALTER TABLE users ALTER COLUMN data TYPE JSONB USING data::JSONB;
```

**Safe:**
```sql
-- Multi-step approach:

-- Step 1: Add new column with desired type
ALTER TABLE users ADD COLUMN age_new BIGINT;

-- Step 2: Backfill data in batches (outside migration)
UPDATE users SET age_new = age::BIGINT;

-- Step 3: Deploy application to use new column

-- Step 4: Drop old column
ALTER TABLE users DROP COLUMN age;

-- Step 5: Rename new column
ALTER TABLE users RENAME COLUMN age_new TO age;
```

**Note:** Some type changes are safe and don't require a table rewrite:
- `VARCHAR(n)` to `VARCHAR(m)` where m > n (PostgreSQL 9.2+)
- `VARCHAR` to `TEXT`
- Numeric precision increases

### ADD NOT NULL constraint

**Unsafe:**
```sql
ALTER TABLE users ALTER COLUMN email SET NOT NULL;
```

**Safe:**
```sql
-- Multi-step approach for large tables:

-- Step 1: Add CHECK constraint without validating existing rows
ALTER TABLE users ADD CONSTRAINT email_not_null CHECK (email IS NOT NULL) NOT VALID;

-- Step 2: Validate constraint separately (allows concurrent operations)
ALTER TABLE users VALIDATE CONSTRAINT email_not_null;

-- Step 3: Add NOT NULL constraint (instant if CHECK exists)
ALTER TABLE users ALTER COLUMN email SET NOT NULL;

-- Step 4: Optionally drop redundant CHECK constraint
ALTER TABLE users DROP CONSTRAINT email_not_null;
```

**Note:** The VALIDATE step uses SHARE UPDATE EXCLUSIVE lock, which allows concurrent reads and writes but blocks other schema changes. This is much safer than the direct SET NOT NULL approach which requires a full table scan with ACCESS EXCLUSIVE lock.

## Development

### Testing

The project has comprehensive test coverage with both unit and integration tests.

#### Run All Tests

```bash
cargo test
```

This runs:
- **Unit tests** - Individual check modules, parser, and safety checker
- **Integration tests** - Fixture files are automatically verified

#### Run Specific Test Suites

```bash
# Run only unit tests (in src/)
cargo test --lib

# Run only integration tests (fixtures)
cargo test --test fixtures_test

# Run tests for a specific check
cargo test add_column
cargo test add_index
cargo test drop_column
```

#### Test Structure

**Unit Tests** (`src/checks/*.rs`):
- Each check module has its own test suite
- Uses shared test utilities from `src/checks/test_utils.rs`
- Tests individual SQL statement parsing and violation detection

**Integration Tests** (`tests/fixtures_test.rs`):
- Automatically verifies all fixture files behave correctly
- Tests both safe and unsafe migrations
- Validates directory-level scanning

### Build

```bash
cargo build --release
```

### Code Quality

```bash
# Format code
cargo fmt

# Run linter
cargo clippy --all-targets --all-features -- -D warnings
```

## License

MIT
