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

## Phase 1 Complete ✅

This is the Phase 1 MVP with core infrastructure:

- ✅ SQL parser integration (sqlparser)
- ✅ Error types and formatting
- ✅ Basic CLI structure
- ✅ Two checks working end-to-end: **ADD COLUMN with DEFAULT** and **DROP COLUMN**

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

### 1. ADD COLUMN with DEFAULT

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

### 2. DROP COLUMN

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

## Coming Soon (Phase 2)

- ADD INDEX without CONCURRENTLY
- ALTER COLUMN TYPE
- ADD NOT NULL constraint

## Development

### Run tests

```bash
cargo test
```

### Build

```bash
cargo build --release
```

### Test on fixtures

```bash
# Test all fixtures at once
cargo run -- check tests/fixtures/

# Test specific scenarios
cargo run -- check tests/fixtures/add_column_safe/up.sql              # Should pass ✅
cargo run -- check tests/fixtures/add_column_with_default/up.sql      # Should fail ❌
cargo run -- check tests/fixtures/drop_column/up.sql                  # Should fail ❌
cargo run -- check tests/fixtures/drop_column_if_exists/up.sql        # Should fail ❌
cargo run -- check tests/fixtures/drop_multiple_columns/up.sql        # Should fail ❌
```

## License

MIT
