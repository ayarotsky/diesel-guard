# Diesel Guard

![Build Status](https://github.com/ayarotsky/diesel-guard/actions/workflows/ci.yml/badge.svg?branch=main)

Catch unsafe PostgreSQL migrations in Diesel before they take down production.

✓ Detects operations that lock tables or cause downtime<br>
✓ Provides safe alternatives for each unsafe operation<br>
✓ Works with any Diesel project - zero configuration required<br>
✓ Supports safety-assured blocks for verified operations<br>

## Installation

```sh
cargo install diesel-guard
```

## How It Works

Diesel Guard analyzes your migration SQL and catches dangerous operations before they reach production.

```sh
diesel-guard check migrations/2024_01_01_create_users/up.sql
```

When it finds an unsafe operation, you'll see:

```
❌ Unsafe migration detected in migrations/2024_01_01_create_users/up.sql

❌ ADD COLUMN with DEFAULT

Problem:
  Adding column 'admin' with DEFAULT on table 'users' requires a full table rewrite on PostgreSQL < 11,
  which acquires an ACCESS EXCLUSIVE lock. On large tables, this can take significant time and block all operations.

Safe alternative:
  1. Add the column without a default:
     ALTER TABLE users ADD COLUMN admin BOOLEAN;

  2. Backfill data in batches (outside migration):
     UPDATE users SET admin = <value> WHERE admin IS NULL;

  3. Add default for new rows only:
     ALTER TABLE users ALTER COLUMN admin SET DEFAULT <value>;

  Note: For PostgreSQL 11+, this is safe if the default is a constant value.
```

## Checks

### Adding a column with a default value

#### Bad

In PostgreSQL versions before 11, adding a column with a default value requires a full table rewrite. This acquires an ACCESS EXCLUSIVE lock and can take hours on large tables, blocking all reads and writes.

```sql
ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;
```

#### Good

Add the column first, backfill the data separately, then add the default:

```sql
-- Migration 1: Add column without default
ALTER TABLE users ADD COLUMN admin BOOLEAN;

-- Outside migration: Backfill in batches
UPDATE users SET admin = FALSE WHERE admin IS NULL;

-- Migration 2: Add default for new rows only
ALTER TABLE users ALTER COLUMN admin SET DEFAULT FALSE;
```

**Note:** For PostgreSQL 11+, adding a column with a constant default value is instant and safe.

### Dropping a column

#### Bad

Dropping a column acquires an ACCESS EXCLUSIVE lock and typically triggers a table rewrite. This blocks all operations and can cause errors if application code is still referencing the column.

```sql
ALTER TABLE users DROP COLUMN email;
```

#### Good

Remove references from application code first, then drop the column in a later migration:

```sql
-- Step 1: Mark column as unused in application code
-- Deploy application code changes first

-- Step 2: (Optional) Set to NULL to reclaim space
ALTER TABLE users ALTER COLUMN email DROP NOT NULL;
UPDATE users SET email = NULL;

-- Step 3: Drop in later migration after confirming it's unused
ALTER TABLE users DROP COLUMN email;
```

PostgreSQL doesn't support `DROP COLUMN CONCURRENTLY`, so the table rewrite is unavoidable. Staging the removal minimizes risk.

### Adding an index non-concurrently

#### Bad

Creating an index without CONCURRENTLY acquires a SHARE lock, blocking all write operations (INSERT, UPDATE, DELETE) for the duration of the index build.

```sql
CREATE INDEX idx_users_email ON users(email);
CREATE UNIQUE INDEX idx_users_username ON users(username);
```

#### Good

Use CONCURRENTLY to allow concurrent writes during the index build:

```sql
CREATE INDEX CONCURRENTLY idx_users_email ON users(email);
CREATE UNIQUE INDEX CONCURRENTLY idx_users_username ON users(username);
```

**Important:** CONCURRENTLY cannot run inside a transaction block. Add a `metadata.toml` file to your migration directory:

```toml
# migrations/2024_01_01_add_user_index/metadata.toml
run_in_transaction = false
```

### Changing column type

#### Bad

Changing a column's type typically requires an ACCESS EXCLUSIVE lock and triggers a full table rewrite, blocking all operations.

```sql
ALTER TABLE users ALTER COLUMN age TYPE BIGINT;
ALTER TABLE users ALTER COLUMN data TYPE JSONB USING data::JSONB;
```

#### Good

Use a multi-step approach with a new column:

```sql
-- Migration 1: Add new column
ALTER TABLE users ADD COLUMN age_new BIGINT;

-- Outside migration: Backfill in batches
UPDATE users SET age_new = age::BIGINT;

-- Migration 2: Swap columns
ALTER TABLE users DROP COLUMN age;
ALTER TABLE users RENAME COLUMN age_new TO age;
```

**Safe type changes** (no rewrite on PostgreSQL 9.2+):
- Increasing VARCHAR length: `VARCHAR(50)` → `VARCHAR(100)`
- Converting to TEXT: `VARCHAR(255)` → `TEXT`
- Increasing numeric precision

### Adding a NOT NULL constraint

#### Bad

Adding a NOT NULL constraint requires scanning the entire table to verify all values are non-null. This acquires an ACCESS EXCLUSIVE lock and blocks all operations.

```sql
ALTER TABLE users ALTER COLUMN email SET NOT NULL;
```

#### Good

For large tables, use a CHECK constraint approach that allows concurrent operations:

```sql
-- Step 1: Add CHECK constraint without validating existing rows
ALTER TABLE users ADD CONSTRAINT email_not_null CHECK (email IS NOT NULL) NOT VALID;

-- Step 2: Validate separately (uses SHARE UPDATE EXCLUSIVE lock)
ALTER TABLE users VALIDATE CONSTRAINT email_not_null;

-- Step 3: Add NOT NULL constraint (instant if CHECK exists)
ALTER TABLE users ALTER COLUMN email SET NOT NULL;

-- Step 4: Optionally drop redundant CHECK constraint
ALTER TABLE users DROP CONSTRAINT email_not_null;
```

The VALIDATE step allows concurrent reads and writes, only blocking other schema changes. On PostgreSQL 12+, NOT NULL constraints are more efficient, but this approach still provides better control.

## Usage

### Check a single migration

```sh
diesel-guard check migrations/2024_01_01_create_users/up.sql
```

### Check all migrations

```sh
diesel-guard check migrations/
```

### JSON output for CI/CD

```sh
diesel-guard check migrations/ --format json
```

## Configuration

Create a `diesel-guard.toml` file in your project root to customize behavior.

### Initialize configuration

Generate a documented configuration file:

```sh
diesel-guard init
```

Use `--force` to overwrite an existing file:

```sh
diesel-guard init --force
```

### Configuration options

```toml
# Skip migrations before this timestamp
# Accepts: YYYYMMDDHHMMSS, YYYY_MM_DD_HHMMSS, or YYYY-MM-DD-HHMMSS
# Works with any migration directory format
start_after = "2024_01_01_000000"

# Also check down.sql files (default: false)
check_down = true

# Disable specific checks
disable_checks = ["AddColumnCheck"]
```

#### Available check names

- `AddColumnCheck` - ADD COLUMN with DEFAULT
- `AddIndexCheck` - CREATE INDEX without CONCURRENTLY
- `AddNotNullCheck` - ALTER COLUMN SET NOT NULL
- `AlterColumnTypeCheck` - ALTER COLUMN TYPE
- `DropColumnCheck` - DROP COLUMN

## Safety Assured

When you've manually verified an operation is safe, use `safety-assured` comment blocks to bypass checks:

```sql
-- safety-assured:start
ALTER TABLE users DROP COLUMN deprecated_column;
ALTER TABLE posts DROP COLUMN old_field;
-- safety-assured:end
```

### Multiple blocks

```sql
-- safety-assured:start
ALTER TABLE users DROP COLUMN email;
-- safety-assured:end

-- This will be checked normally
CREATE INDEX users_email_idx ON users(email);

-- safety-assured:start
ALTER TABLE posts DROP COLUMN body;
-- safety-assured:end
```

### When to use safety-assured

**Only use when you've taken proper precautions:**

1. **For DROP COLUMN:**
   - Stopped reading/writing the column in application code
   - Deployed those changes to production
   - Verified no references remain in your codebase

2. **For other operations:**
   ```sql
   -- safety-assured:start
   -- Safe because: table is empty, deployed in maintenance window
   ALTER TABLE new_table ADD COLUMN status TEXT DEFAULT 'pending';
   -- safety-assured:end
   ```

Diesel Guard will error if blocks are mismatched:

```
Error: Unclosed 'safety-assured:start' at line 1
```

## Coming Soon

### Constraint & lock-related

- **ADD FOREIGN KEY constraint** - Blocks writes during validation; use NOT VALID + separate VALIDATE
- **ADD UNIQUE constraint** - Blocks reads/writes; use CREATE UNIQUE INDEX CONCURRENTLY instead
- **ADD CHECK constraint** - Blocks during validation; use NOT VALID then VALIDATE separately
- **DROP INDEX without CONCURRENTLY** - Blocks all queries; use DROP INDEX CONCURRENTLY
- **REINDEX without CONCURRENTLY** - Blocks reads/writes; use REINDEX CONCURRENTLY (PostgreSQL 12+)

### Schema & data migration

- **RENAME COLUMN** - Causes errors in running instances; requires multi-step migration
- **RENAME TABLE** - Causes errors in running instances; use database views as intermediary
- **Adding stored GENERATED column** - Triggers full table rewrite with ACCESS EXCLUSIVE lock
- **Adding JSON/JSONB column** - Can break SELECT DISTINCT in older PostgreSQL versions

### Data safety & best practices

- **Auto-increment on existing table** - SERIAL columns trigger full table rewrite
- **Short integer primary keys** - SMALLINT/INT risk ID exhaustion; use BIGINT
- **Wide indexes** - Indexes with 3+ columns rarely help; consider partial indexes
- **Multiple foreign keys** - Can block all involved tables simultaneously
- **CREATE EXTENSION** - Often requires superuser; install outside migrations
- **Unnamed constraints** - Makes future migrations difficult; always name constraints explicitly

## Contributing

We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and testing guide.

For AI assistants working on this project, see [AGENTS.md](AGENTS.md) for detailed implementation patterns.

## Credits

Inspired by [strong_migrations](https://github.com/ankane/strong_migrations) by Andrew Kane

## License

MIT
