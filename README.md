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

## Configuration

diesel_guard can be configured using a `diesel-guard.toml` file in your project root.

### Initialize Configuration

Create a configuration file with all available options documented:

```bash
diesel-guard init
```

This creates a `diesel-guard.toml` file in your current directory with all configuration options commented and documented. If the file already exists, use `--force` to overwrite:

```bash
diesel-guard init --force
```

### Configuration File

Alternatively, manually create a `diesel-guard.toml` file in the same directory where you run `diesel-guard`:

```toml
# Skip migrations before this timestamp
start_after = "2024_01_01_000000"

# Also check down.sql files
check_down = true

# Disable specific checks
disable_checks = ["AddColumnCheck"]
```

If the configuration file is missing, diesel_guard will use default values (check all migrations, only `up.sql`, all checks enabled).

If the configuration file has errors (invalid TOML syntax, invalid values), diesel_guard will print a warning and use default values.

### Configuration Options

#### `start_after` (Optional)

Skip checking migrations created before a specific timestamp. Useful when retrofitting diesel_guard into an existing project with old migrations you don't want to fix.

- **Format**: `"YYYY_MM_DD_HHMMSS"` (matching Diesel migration directory timestamp prefix)
- **Default**: None (all migrations are checked)
- **Example**: `start_after = "2024_01_01_000000"`

**How it works**: Diesel migration directories are named like `2024_01_01_000000_create_users`. The configuration compares the first 19 characters (the timestamp) and only checks migrations created **after** the specified timestamp.

#### `check_down` (Boolean)

Whether to check `down.sql` files in addition to `up.sql` files. By default, only `up.sql` files are checked since they represent forward migrations that will run in production.

- **Default**: `false`
- **Example**: `check_down = true`

**Use case**: Enable this if you want to ensure your rollback migrations are also safe for production.

#### `disable_checks` (Array of Strings)

Selectively disable specific safety checks. Check names are the Rust struct names from the codebase.

- **Valid names**:
  - `AddColumnCheck` - ADD COLUMN with DEFAULT
  - `AddIndexCheck` - CREATE INDEX without CONCURRENTLY
  - `AddNotNullCheck` - ALTER COLUMN SET NOT NULL
  - `AlterColumnTypeCheck` - ALTER COLUMN TYPE
  - `DropColumnCheck` - DROP COLUMN
- **Default**: `[]` (all checks enabled)
- **Example**: `disable_checks = ["AddColumnCheck", "DropColumnCheck"]`

**Use case**: Disable checks if you have specific reasons to allow certain operations (e.g., your PostgreSQL version makes some operations safe, or you're working on a low-traffic database).

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

## Coming Soon

### Constraint & Lock-Related Checks

- **ADD FOREIGN KEY constraint** - Blocks writes on both tables during validation. Requires multi-step approach with NOT VALID and separate VALIDATE CONSTRAINT.

- **ADD UNIQUE constraint** - Blocks reads and writes while building the underlying unique index. Should use CREATE UNIQUE INDEX CONCURRENTLY instead.

- **ADD CHECK constraint** - Blocks reads and writes while validating all existing rows against the constraint. Use NOT VALID then VALIDATE separately.

- **DROP INDEX without CONCURRENTLY** - Acquires ACCESS EXCLUSIVE lock, blocking all queries on the table. Should use DROP INDEX CONCURRENTLY.

- **REINDEX without CONCURRENTLY** - Acquires ACCESS EXCLUSIVE lock, blocking all reads and writes during index rebuild. Use REINDEX CONCURRENTLY (PostgreSQL 12+).

### Schema & Data Migration Checks

- **RENAME COLUMN** - Causes errors in running application instances that cache column names. Requires multi-step migration with dual-writing.

- **RENAME TABLE** - Causes errors in running application instances that reference the old table name. Use database views as intermediary.

- **Adding stored GENERATED column** - Triggers full table rewrite with ACCESS EXCLUSIVE lock, blocking all reads and writes.

- **Adding JSON/JSONB column** - JSON columns lack equality operator in older PostgreSQL versions, breaking SELECT DISTINCT and other queries.

### Data Safety & Best Practices

- **Adding auto-increment column to existing table** - Adding SERIAL or auto-increment columns to existing tables triggers a full table rewrite.

- **Primary key with short integer type** - Using SMALLINT or INT for primary keys creates risk of ID exhaustion on high-traffic tables. Use BIGINT instead.

- **Indexes with more than 3 columns** - Wide indexes rarely improve performance and waste storage. Consider partial indexes or restructuring queries.

- **Adding multiple foreign keys in one migration** - Multiple foreign keys in a single migration can block all involved tables simultaneously, multiplying lock contention.

- **CREATE EXTENSION in migrations** - Installing extensions can have unexpected side effects and typically requires superuser privileges. Install extensions separately outside migrations.

- **Unnamed constraints** - PostgreSQL generates random names for unnamed constraints, making future migrations difficult to write and maintain. Always explicitly name constraints.

## Contributing

We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, testing guide, and how to add new checks.

For AI assistants working on this project, see [AGENTS.md](AGENTS.md) for detailed implementation patterns.

## License

MIT
