# Diesel Guard

![Build Status](https://github.com/ayarotsky/diesel-guard/actions/workflows/ci.yml/badge.svg?branch=main)

Catch dangerous PostgreSQL migrations in Diesel before they take down production.

✓ Detects operations that lock tables or cause downtime<br>
✓ Provides safe alternatives for each blocking operation<br>
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

- [Adding a column with a default value](#adding-a-column-with-a-default-value)
- [Dropping a column](#dropping-a-column)
- [Dropping an index non-concurrently](#dropping-an-index-non-concurrently)
- [Adding an index non-concurrently](#adding-an-index-non-concurrently)
- [Adding a UNIQUE constraint](#adding-a-unique-constraint)
- [Changing column type](#changing-column-type)
- [Adding a NOT NULL constraint](#adding-a-not-null-constraint)
- [Creating extensions](#creating-extensions)
- [Unnamed constraints](#unnamed-constraints)
- [Renaming a column](#renaming-a-column)
- [Renaming a table](#renaming-a-table)
- [Short integer primary keys](#short-integer-primary-keys)
- [Adding a SERIAL column to an existing table](#adding-a-serial-column-to-an-existing-table)

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

### Dropping an index non-concurrently

#### Bad

Dropping an index without CONCURRENTLY acquires an ACCESS EXCLUSIVE lock on the table, blocking all queries (SELECT, INSERT, UPDATE, DELETE) until the drop operation completes.

```sql
DROP INDEX idx_users_email;
DROP INDEX IF EXISTS idx_users_username;
```

#### Good

Use CONCURRENTLY to drop the index without blocking queries:

```sql
DROP INDEX CONCURRENTLY idx_users_email;
DROP INDEX CONCURRENTLY IF EXISTS idx_users_username;
```

**Important:** CONCURRENTLY requires PostgreSQL 9.2+ and cannot run inside a transaction block. Add a `metadata.toml` file to your migration directory:

```toml
# migrations/2024_01_01_drop_user_index/metadata.toml
run_in_transaction = false
```

**Note:** Dropping an index concurrently takes longer than a regular drop and uses more resources, but allows concurrent queries to continue. If it fails, the index may be left in an "invalid" state and should be dropped again.

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

### Adding a UNIQUE constraint

#### Bad

Adding a UNIQUE constraint via ALTER TABLE acquires an ACCESS EXCLUSIVE lock, blocking all reads and writes during index creation. This is worse than CREATE INDEX without CONCURRENTLY.

```sql
ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE (email);
ALTER TABLE users ADD UNIQUE (email);  -- Unnamed is also bad
```

#### Good

Use CREATE UNIQUE INDEX CONCURRENTLY, then optionally add the constraint:

```sql
-- Step 1: Create the unique index concurrently
CREATE UNIQUE INDEX CONCURRENTLY users_email_idx ON users(email);

-- Step 2 (Optional): Add constraint using the existing index
-- This is instant since the index already exists
ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE USING INDEX users_email_idx;
```

**Important:** Requires `metadata.toml` with `run_in_transaction = false` (same as CREATE INDEX CONCURRENTLY).

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

### Creating extensions

#### Bad

Creating an extension in migrations often requires superuser privileges, which application database users typically don't have in production environments.

```sql
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE EXTENSION uuid_ossp;
```

#### Good

Install extensions outside of application migrations:

```sql
-- For local development: add to database setup scripts
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- For production: use infrastructure automation
-- (Ansible, Terraform, or manual DBA installation)
```

**Best practices:**
- Document required extensions in your project README
- Include extension installation in database provisioning scripts
- Use infrastructure automation (Ansible, Terraform) for production
- Have your DBA or infrastructure team install extensions before deployment

Common extensions that require this approach: `pg_trgm`, `uuid-ossp`, `hstore`, `postgis`, `pg_stat_statements`.

### Unnamed constraints

#### Bad

Adding constraints without explicit names results in auto-generated names from PostgreSQL. These names vary between databases and make future migrations difficult.

```sql
-- Unnamed UNIQUE constraint
ALTER TABLE users ADD UNIQUE (email);

-- Unnamed FOREIGN KEY constraint
ALTER TABLE posts ADD FOREIGN KEY (user_id) REFERENCES users(id);

-- Unnamed CHECK constraint
ALTER TABLE users ADD CHECK (age >= 0);
```

#### Good

Always name constraints explicitly using the CONSTRAINT keyword:

```sql
-- Named UNIQUE constraint
ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE (email);

-- Named FOREIGN KEY constraint
ALTER TABLE posts ADD CONSTRAINT posts_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id);

-- Named CHECK constraint
ALTER TABLE users ADD CONSTRAINT users_age_check CHECK (age >= 0);
```

**Best practices for constraint naming:**
- **UNIQUE**: `{table}_{column}_key` or `{table}_{column1}_{column2}_key`
- **FOREIGN KEY**: `{table}_{column}_fkey`
- **CHECK**: `{table}_{column}_check` or `{table}_{description}_check`

Named constraints make future migrations predictable:
```sql
-- Easy to reference in later migrations
ALTER TABLE users DROP CONSTRAINT users_email_key;
```

### Renaming a column

#### Bad

Renaming a column breaks running application instances immediately. Any code that references the old column name will fail after the rename is applied, causing downtime.

```sql
ALTER TABLE users RENAME COLUMN email TO email_address;
```

#### Good

Use a multi-step migration to maintain compatibility during the transition:

```sql
-- Migration 1: Add new column
ALTER TABLE users ADD COLUMN email_address VARCHAR(255);

-- Outside migration: Backfill in batches
UPDATE users SET email_address = email;

-- Migration 2: Add NOT NULL if needed
ALTER TABLE users ALTER COLUMN email_address SET NOT NULL;

-- Update application code to use email_address

-- Migration 3: Drop old column after deploying code changes
ALTER TABLE users DROP COLUMN email;
```

**Important:** The RENAME COLUMN operation itself is fast (brief ACCESS EXCLUSIVE lock), but the primary risk is application compatibility, not lock duration. All running instances must be updated to reference the new column name before the rename is applied.

### Renaming a table

#### Bad

Renaming a table breaks running application instances immediately. Any code that references the old table name will fail after the rename is applied. Additionally, this operation requires an ACCESS EXCLUSIVE lock which can block on busy tables.

```sql
ALTER TABLE users RENAME TO customers;
```

#### Good

Use a multi-step dual-write migration to safely rename the table:

```sql
-- Migration 1: Create new table
CREATE TABLE customers (LIKE users INCLUDING ALL);

-- Update application code to write to BOTH tables

-- Migration 2: Backfill data in batches
INSERT INTO customers
SELECT * FROM users
WHERE id > last_processed_id
LIMIT 10000;

-- Update application code to read from new table

-- Deploy updated application

-- Update application code to stop writing to old table

-- Migration 3: Drop old table
DROP TABLE users;
```

**Important:** This multi-step approach avoids the ACCESS EXCLUSIVE lock issues on large tables and ensures zero downtime. The migration requires multiple deployments coordinated with application code changes.

### Short integer primary keys

#### Bad

Using SMALLINT or INT for primary keys risks ID exhaustion. SMALLINT maxes out at ~32,767 records, and INT at ~2.1 billion. While 2.1 billion seems large, active applications can exhaust this faster than expected, especially with high-frequency inserts, soft deletes, or partitioned data.

Changing the type later requires an ALTER COLUMN TYPE operation with a full table rewrite and ACCESS EXCLUSIVE lock.

```sql
-- SMALLINT exhausts at ~32K records
CREATE TABLE users (id SMALLINT PRIMARY KEY);

-- INT exhausts at ~2.1B records
CREATE TABLE posts (id INT PRIMARY KEY);
CREATE TABLE events (id INTEGER PRIMARY KEY);

-- Composite PKs with short integers still risky
CREATE TABLE tenant_events (
    tenant_id BIGINT,
    event_id INT,  -- Will exhaust per tenant
    PRIMARY KEY (tenant_id, event_id)
);
```

#### Good

Use BIGINT for all primary keys to avoid exhaustion:

```sql
-- BIGINT: effectively unlimited (~9.2 quintillion)
CREATE TABLE users (id BIGINT PRIMARY KEY);

-- BIGSERIAL: auto-incrementing BIGINT
CREATE TABLE posts (id BIGSERIAL PRIMARY KEY);

-- Composite PKs with all BIGINT
CREATE TABLE tenant_events (
    tenant_id BIGINT,
    event_id BIGINT,
    PRIMARY KEY (tenant_id, event_id)
);
```

**Storage overhead:** BIGINT uses 8 bytes vs INT's 4 bytes - only 4 extra bytes per row. For a 1 million row table, this is ~4MB of additional storage, which is negligible compared to the operational cost of changing column types later.

**Safe exceptions:** Small, finite lookup tables with <100 entries (e.g., status codes, country lists) can safely use smaller types. Use `safety-assured` to bypass the check for these cases.

### Adding a SERIAL column to an existing table

#### Bad

Adding a SERIAL column to an existing table triggers a full table rewrite because PostgreSQL must populate sequence values for all existing rows. This acquires an ACCESS EXCLUSIVE lock and blocks all operations.

```sql
ALTER TABLE users ADD COLUMN id SERIAL;
ALTER TABLE users ADD COLUMN order_number BIGSERIAL;
```

#### Good

Create the sequence separately, add the column without a default, then backfill:

```sql
-- Step 1: Create a sequence
CREATE SEQUENCE users_id_seq;

-- Step 2: Add the column WITHOUT default (fast, no rewrite)
ALTER TABLE users ADD COLUMN id INTEGER;

-- Outside migration: Backfill existing rows in batches
UPDATE users SET id = nextval('users_id_seq') WHERE id IS NULL;

-- Step 3: Set default for future inserts only
ALTER TABLE users ALTER COLUMN id SET DEFAULT nextval('users_id_seq');

-- Step 4: Set NOT NULL if needed (PostgreSQL 11+: safe if all values present)
ALTER TABLE users ALTER COLUMN id SET NOT NULL;

-- Step 5: Set sequence ownership
ALTER SEQUENCE users_id_seq OWNED BY users.id;
```

**Key insight:** Adding a column with `DEFAULT nextval(...)` on an existing table still triggers a table rewrite. The solution is to add the column first without any default, backfill separately, then set the default for future rows only.

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
- `AddSerialColumnCheck` - ADD COLUMN with SERIAL
- `AddUniqueConstraintCheck` - ADD UNIQUE constraint via ALTER TABLE
- `AlterColumnTypeCheck` - ALTER COLUMN TYPE
- `CreateExtensionCheck` - CREATE EXTENSION
- `DropColumnCheck` - DROP COLUMN
- `DropIndexCheck` - DROP INDEX without CONCURRENTLY
- `RenameColumnCheck` - RENAME COLUMN
- `RenameTableCheck` - RENAME TABLE
- `ShortIntegerPrimaryKeyCheck` - SMALLINT/INT/INTEGER primary keys
- `UnnamedConstraintCheck` - Unnamed constraints (UNIQUE, FOREIGN KEY, CHECK)

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
- **ADD CHECK constraint** - Blocks during validation; use NOT VALID then VALIDATE separately
- **ADD EXCLUSION constraint** - Blocks all operations during validation (no safe workaround)
- **ADD PRIMARY KEY to existing table** - Blocks all operations, creates index; use CREATE UNIQUE INDEX CONCURRENTLY first
- **DROP PRIMARY KEY** - Breaks foreign key relationships and removes critical constraint
- **FOREIGN KEY with CASCADE** - Can cause unintended cascading deletes/updates and data loss
- **REINDEX without CONCURRENTLY** - Blocks reads/writes; use REINDEX CONCURRENTLY (PostgreSQL 12+)

### Schema & data migration

- **Adding stored GENERATED column** - Triggers full table rewrite with ACCESS EXCLUSIVE lock
- **Adding JSON/JSONB column** - Can break SELECT DISTINCT in older PostgreSQL versions
- **DROP TABLE with multiple foreign keys** - Extended locks on multiple tables simultaneously
- **TRUNCATE TABLE** - Acquires ACCESS EXCLUSIVE lock, blocks all operations, cannot be batched

### Data safety & best practices

- **Wide indexes** - Indexes with 3+ columns rarely help; consider partial indexes
- **Multiple foreign keys** - Can block all involved tables simultaneously
- **Replacing indexes** - Dropping old index before creating replacement risks query performance
- **Mismatched foreign key column types** - Foreign key column type differs from referenced primary key

## Contributing

We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and testing guide.

For AI assistants working on this project, see [AGENTS.md](AGENTS.md) for detailed implementation patterns.

## Credits

Inspired by [strong_migrations](https://github.com/ankane/strong_migrations) by Andrew Kane

## License

MIT
