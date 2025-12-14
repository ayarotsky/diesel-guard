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
- [Dropping a primary key](#dropping-a-primary-key)
- [Dropping an index non-concurrently](#dropping-an-index-non-concurrently)
- [Adding an index non-concurrently](#adding-an-index-non-concurrently)
- [Adding a UNIQUE constraint](#adding-a-unique-constraint)
- [Changing column type](#changing-column-type)
- [Adding a NOT NULL constraint](#adding-a-not-null-constraint)
- [Adding a primary key to an existing table](#adding-a-primary-key-to-an-existing-table)
- [Creating extensions](#creating-extensions)
- [Unnamed constraints](#unnamed-constraints)
- [Renaming a column](#renaming-a-column)
- [Renaming a table](#renaming-a-table)
- [Short integer primary keys](#short-integer-primary-keys)
- [Adding a SERIAL column to an existing table](#adding-a-serial-column-to-an-existing-table)
- [Adding a JSON column](#adding-a-json-column)
- [Truncating a table](#truncating-a-table)
- [Wide indexes](#wide-indexes)

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

### Dropping a primary key

#### Bad

Dropping a primary key removes the critical uniqueness constraint and breaks foreign key relationships in other tables that reference this table. It also acquires an ACCESS EXCLUSIVE lock, blocking all operations.

```sql
-- Breaks foreign keys that reference users(id)
ALTER TABLE users DROP CONSTRAINT users_pkey;
```

#### Good

If you must change your primary key strategy, use a multi-step migration approach:

```sql
-- Step 1: Identify all foreign key dependencies
SELECT
  tc.table_name, kcu.column_name, rc.constraint_name
FROM information_schema.table_constraints tc
JOIN information_schema.key_column_usage kcu ON tc.constraint_name = kcu.constraint_name
JOIN information_schema.referential_constraints rc ON tc.constraint_name = rc.unique_constraint_name
WHERE tc.table_name = 'users' AND tc.constraint_type = 'PRIMARY KEY';

-- Step 2: Create the new primary key FIRST (if migrating to a new key)
ALTER TABLE users ADD CONSTRAINT users_new_pkey PRIMARY KEY (uuid);

-- Step 3: Update all foreign keys to reference the new key
-- (This may require adding new columns to referencing tables)
ALTER TABLE posts ADD COLUMN user_uuid UUID;
UPDATE posts SET user_uuid = users.uuid FROM users WHERE posts.user_id = users.id;
ALTER TABLE posts ADD CONSTRAINT posts_user_uuid_fkey FOREIGN KEY (user_uuid) REFERENCES users(uuid);

-- Step 4: Only after all foreign keys are migrated, drop the old key
ALTER TABLE users DROP CONSTRAINT users_pkey;

-- Step 5: Clean up old columns
ALTER TABLE posts DROP COLUMN user_id;
```

**Important considerations:**
- Review ALL tables with foreign keys to this table
- Consider a transition period where both old and new keys exist
- Update application code to use the new key before dropping the old one
- Test thoroughly in a staging environment first

**Limitation:** This check relies on PostgreSQL naming conventions (e.g., `users_pkey`). It may not detect primary keys with custom names. Future versions will support database connections for accurate verification.

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
ALTER TABLE users ADD CONSTRAINT users_email_not_null_check CHECK (email IS NOT NULL) NOT VALID;

-- Step 2: Validate separately (uses SHARE UPDATE EXCLUSIVE lock)
ALTER TABLE users VALIDATE CONSTRAINT users_email_not_null_check;

-- Step 3: Add NOT NULL constraint (instant if CHECK exists)
ALTER TABLE users ALTER COLUMN email SET NOT NULL;

-- Step 4: Optionally drop redundant CHECK constraint
ALTER TABLE users DROP CONSTRAINT users_email_not_null_check;
```

The VALIDATE step allows concurrent reads and writes, only blocking other schema changes. On PostgreSQL 12+, NOT NULL constraints are more efficient, but this approach still provides better control.

### Adding a primary key to an existing table

#### Bad

Adding a primary key constraint to an existing table acquires an ACCESS EXCLUSIVE lock, blocking all operations (reads and writes). The operation must also create an index to enforce uniqueness, which compounds the lock duration on large tables.

```sql
-- Blocks all operations while creating index and adding constraint
ALTER TABLE users ADD PRIMARY KEY (id);
ALTER TABLE users ADD CONSTRAINT users_pkey PRIMARY KEY (id);
```

#### Good

Use CREATE UNIQUE INDEX CONCURRENTLY first, then add the primary key constraint using the existing index:

```sql
-- Step 1: Create unique index concurrently (allows concurrent operations)
CREATE UNIQUE INDEX CONCURRENTLY users_pkey ON users(id);

-- Step 2: Add PRIMARY KEY using the existing index (fast, minimal lock)
ALTER TABLE users ADD CONSTRAINT users_pkey PRIMARY KEY USING INDEX users_pkey;
```

**Important:** The CONCURRENTLY approach requires `metadata.toml` with `run_in_transaction = false`:

```toml
# migrations/2024_01_01_add_primary_key/metadata.toml
run_in_transaction = false
```

**Why this works:**
- Step 1: Creates the index without blocking operations (only prevents concurrent schema changes)
- Step 2: Adding the constraint is nearly instant since the index already exists

**Note:** This approach requires PostgreSQL 11+. For earlier versions, you must use the unsafe `ALTER TABLE ADD PRIMARY KEY` during a maintenance window.

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

### Adding a JSON column

#### Bad

In PostgreSQL, the `json` type has no equality operator, which breaks existing `SELECT DISTINCT` queries and other operations that require comparing values.

```sql
ALTER TABLE users ADD COLUMN properties JSON;
```

#### Good

Use `jsonb` instead of `json`:

```sql
ALTER TABLE users ADD COLUMN properties JSONB;
```

**Benefits of JSONB over JSON:**
- Has proper equality and comparison operators (supports DISTINCT, GROUP BY, UNION)
- Supports indexing (GIN indexes for efficient queries)
- Faster to process (binary format, no reparsing)
- Generally better performance for most use cases

**Note:** The only advantage of JSON over JSONB is that it preserves exact formatting and key order, which is rarely needed in practice.

### Truncating a table

#### Bad

TRUNCATE TABLE acquires an ACCESS EXCLUSIVE lock, blocking all operations (reads and writes) on the table. Unlike DELETE, TRUNCATE cannot be batched or throttled, making it unsuitable for large tables in production environments.

```sql
TRUNCATE TABLE users;
TRUNCATE TABLE orders, order_items;
```

#### Good

Use DELETE with batching to incrementally remove rows while allowing concurrent access:

```sql
-- Delete rows in small batches to allow concurrent access
DELETE FROM users WHERE id IN (
  SELECT id FROM users LIMIT 1000
);

-- Repeat the batched DELETE until all rows are removed
-- (Can be done outside migration with monitoring)

-- Optional: Reset sequences if needed
ALTER SEQUENCE users_id_seq RESTART WITH 1;

-- Optional: Reclaim space
VACUUM users;
```

**Important:** If you absolutely must use TRUNCATE (e.g., in a test environment or during a maintenance window), use a `safety-assured` block:

```sql
-- safety-assured:start
-- Safe because: running in test environment / maintenance window
TRUNCATE TABLE users;
-- safety-assured:end
```

### Wide indexes

#### Bad

Indexes with 4 or more columns are rarely effective. PostgreSQL can only use multi-column indexes efficiently when filtering on the leftmost columns in order. Wide indexes also increase storage costs and slow down write operations (INSERT, UPDATE, DELETE).

```sql
-- 4+ columns: rarely useful
CREATE INDEX idx_users_search ON users(tenant_id, email, name, status);
CREATE INDEX idx_orders_composite ON orders(user_id, product_id, status, created_at);
```

#### Good

Use narrower, more targeted indexes based on actual query patterns:

```sql
-- Option 1: Partial index for specific query pattern
CREATE INDEX idx_users_active_email ON users(email)
WHERE status = 'active';

-- Option 2: Separate indexes for different queries
CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_status ON users(status);

-- Option 3: Covering index with INCLUDE (PostgreSQL 11+)
-- Includes extra columns for SELECT without adding them to index keys
CREATE INDEX idx_users_email_covering ON users(email)
INCLUDE (name, status);

-- Option 4: Two-column composite (still useful for some patterns)
CREATE INDEX idx_users_tenant_email ON users(tenant_id, email);
```

**When wide indexes might be acceptable:**
- Composite foreign keys matching the referenced table's primary key
- Specific, verified query patterns that need all columns in order
- Use `safety-assured` if you've confirmed the index is necessary

**Performance tip:** PostgreSQL can combine multiple indexes using bitmap scans. Two separate indexes often outperform one wide index.

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

## CI/CD Integration

### GitHub Actions

Add `diesel-guard` to your CI pipeline to automatically check migrations on pull requests.

#### Option 1: GitHub Action (Recommended)

Use the official GitHub Action:

```yaml
name: Check Migrations
on: [pull_request]

jobs:
  check-migrations:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      # Pin to specific version (recommended for stability)
      - uses: ayarotsky/diesel-guard@v0.2.0
        with:
          path: migrations/
```

**Versioning:**
- The action automatically installs the diesel-guard CLI version matching the tag
- `@v0.2.0` installs diesel-guard v0.2.0
- `@main` installs the latest version

**Alternatives:**

```yaml
# Always use latest (gets new checks and fixes automatically)
- uses: ayarotsky/diesel-guard@main
  with:
    path: migrations/
```

This will:
- ✅ Install diesel-guard
- ✅ Check your migrations for unsafe patterns
- ✅ Display detailed violation reports in workflow logs
- ✅ Fail the workflow if violations are detected

#### Option 2: Manual Installation

For more control or custom workflows:

```yaml
name: Check Migrations
on: [pull_request]

jobs:
  check-migrations:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Rust toolchain
        uses: actions-rust-lang/setup-rust-toolchain@v1
        with:
          toolchain: stable

      - name: Install diesel-guard
        run: cargo install diesel-guard

      - name: Check DB migrations
        run: diesel-guard check migrations/
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
- `AddPrimaryKeyCheck` - ADD PRIMARY KEY to existing table
- `AddSerialColumnCheck` - ADD COLUMN with SERIAL
- `AddUniqueConstraintCheck` - ADD UNIQUE constraint via ALTER TABLE
- `AlterColumnTypeCheck` - ALTER COLUMN TYPE
- `CreateExtensionCheck` - CREATE EXTENSION
- `DropColumnCheck` - DROP COLUMN
- `DropIndexCheck` - DROP INDEX without CONCURRENTLY
- `DropPrimaryKeyCheck` - DROP PRIMARY KEY
- `RenameColumnCheck` - RENAME COLUMN
- `RenameTableCheck` - RENAME TABLE
- `ShortIntegerPrimaryKeyCheck` - SMALLINT/INT/INTEGER primary keys
- `TruncateTableCheck` - TRUNCATE TABLE
- `UnnamedConstraintCheck` - Unnamed constraints (UNIQUE, FOREIGN KEY, CHECK)
- `WideIndexCheck` - Indexes with 4+ columns

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

## Path to v1

`diesel-guard` is on a roadmap to **v1** with 3 major phases, each adding progressively more sophisticated migration safety capabilities.

### Phase 1: Complete Static Analysis Coverage

**Goal:** Implement all linter checks that work through SQL parsing alone, without requiring database connection.

**Status:** 18/40 checks implemented (45%)

#### Constraint & Lock Safety (5 remaining)
- **ADD FOREIGN KEY constraint** - Detect missing NOT VALID; recommend two-step validation
- **ADD CHECK constraint** - Detect missing NOT VALID; recommend separate VALIDATE
- **ADD EXCLUSION constraint** - Warn about blocking behavior (no safe workaround)
- **FOREIGN KEY with CASCADE** - Flag ON DELETE CASCADE and ON UPDATE CASCADE
- **REINDEX without CONCURRENTLY** - Recommend REINDEX CONCURRENTLY (PostgreSQL 12+)

#### Table & Schema Operations (7 remaining)
- **VACUUM FULL** - Warn about ACCESS EXCLUSIVE lock; recommend regular VACUUM
- **CLUSTER** - Warn about full table rewrite with ACCESS EXCLUSIVE lock
- **Adding stored GENERATED column** - Detect table rewrite trigger
- **DROP TABLE** - Configurable prevention of accidental drops
- **DROP DATABASE** - Prevent catastrophic database deletion
- **DROP NOT NULL constraint** - Warn about breaking existing clients
- **LOCK TABLE statements** - Flag explicit lock acquisitions

#### Type & Best Practices (5 remaining)
- **CHAR/CHARACTER field types** - Recommend TEXT or VARCHAR instead
- **VARCHAR with size limit** - Recommend TEXT with CHECK constraint
- **TIMESTAMP without time zone** - Recommend TIMESTAMPTZ
- **SERIAL/BIGSERIAL types** - Recommend IDENTITY columns
- **Mismatched foreign key column types** - Detect type inconsistencies via SQL parsing

#### Transaction & Robustness (3 remaining)
- **CREATE INDEX CONCURRENTLY in transaction** - Detect and prevent (will fail)
- **Transaction nesting** - Detect problematic nested BEGIN/COMMIT blocks
- **Non-robust statements** - Encourage IF EXISTS/IF NOT EXISTS for idempotency

#### Data Safety (2 remaining)
- **UPDATE/DELETE without WHERE clause** - Prevent accidental mass modifications
- **Uncommitted transactions** - Detect unclosed transaction blocks

### Phase 2: Database-Connected Intelligence

**Goal:** Add optional database connection to provide context-aware checking based on actual table state, relationships, and PostgreSQL version.

**Benefits:** More accurate warnings, version-specific advice, and detection of issues impossible to catch through static analysis alone.

#### New Database-Connected Checks

1. **Table size awareness** - Warn differently for operations on large vs small tables
   - Operations safe on small tables become dangerous above threshold (configurable, default: 1GB or 100K rows)
   - Example: ALTER COLUMN TYPE on 100-row table vs 10M-row table

2. **View and materialized view dependencies** - Detect dependent objects before dangerous operations
   - Flag DROP COLUMN when views reference the column
   - Flag ALTER COLUMN TYPE when materialized views depend on it

3. **Foreign key relationship mapping** - Analyze actual FK graph in database
   - Detect DROP TABLE when other tables reference it (with FK counts)
   - Warn about CASCADE operations with actual downstream table list
   - Flag ADD FOREIGN KEY when referencing table lacks proper index

4. **Partition hierarchy detection** - Understand partitioned table structures
   - Recommend DETACH PARTITION CONCURRENTLY for PG 14+
   - Warn about operations on parent vs child partitions

5. **Trigger and function dependencies** - Detect triggers that reference columns/tables
   - Flag DROP COLUMN when triggers use it
   - Flag ALTER COLUMN TYPE when triggers may break

6. **Missing timeout settings** - Verify timeout configuration at database/user level
   - Query `SHOW lock_timeout` and `SHOW statement_timeout` to check effective values
   - Only warn if both migration file AND database have no timeouts configured
   - Avoids false positives when timeouts are set at database/role level
   - Recommend `SET lock_timeout = '2s'` and `SET statement_timeout = '5s'` in migrations

#### Existing Checks Enhanced by Database Connection

1. **DROP PRIMARY KEY** (currently limited) - Currently relies on naming conventions (e.g., `users_pkey`)
   - **With DB:** Query information_schema to find actual primary key name
   - **With DB:** List all foreign keys that reference this PK

2. **ADD COLUMN with DEFAULT** (currently version-agnostic)
   - **With DB:** Query PostgreSQL version to give accurate warnings
   - **With DB:** PG 11+ with constant defaults → mark as safe, no warning needed

3. **ALTER COLUMN TYPE** (currently assumes all unsafe)
   - **With DB:** Detect safe type changes (VARCHAR(50)→VARCHAR(100), VARCHAR→TEXT)
   - **With DB:** Check actual column type to avoid false positives

4. **Short Integer Primary Keys** (currently checks all tables)
   - **With DB:** Query actual row counts to assess urgency
   - **With DB:** Calculate time-to-exhaustion based on insert rate from pg_stat_user_tables

5. **CREATE EXTENSION** (currently always warns)
   - **With DB:** Check if extension already exists
   - **With DB:** Verify current user has sufficient privileges

6. **ADD NOT NULL** (currently always recommends CHECK constraint)
   - **With DB:** On PG 12+, verify all values non-null → direct SET NOT NULL is safe
   - **With DB:** Detect if equivalent CHECK constraint already exists

#### Configuration

```toml
# diesel-guard.toml
# Optional: connect to database for enhanced checking
url = "postgresql://user:pass@localhost/dbname"
enabled = false  # Default: false (static analysis only)

# Table size thresholds for warnings
large_table_rows = 100000
large_table_size_mb = 1024
```

### Phase 3: Diesel Integration - All-in-One Migration Suite

**Goal:** Integration with `diesel_migrations` to become the single tool for both safety checking and migration execution.

#### Core Integration Features

1. **Native Diesel CLI replacement**
   ```bash
   # Instead of:
   diesel migration run
   diesel migration revert

   # Users can:
   diesel-guard migrate run    # Checks safety, then runs
   diesel-guard migrate revert # Checks down.sql safety, then reverts
   diesel-guard migrate status # Show migration status + safety summary
   ```

2. **Automatic safety checking before execution**
   - Run all safety checks before applying any migration

3. **Migration generation with safe templates**
   ```bash
   diesel-guard migration generate add_user_email

   # Generates migration with safety comments and safe patterns:
   # up.sql:
   # -- Migration: Add email column to users
   # -- Safe pattern: Add without default, backfill separately
   #
   # SET lock_timeout = '2s';
   # SET statement_timeout = '5s';
   #
   # ALTER TABLE users ADD COLUMN email TEXT;
   ```

4. **Interactive migration review**
   ```bash
   diesel-guard migrate review

   # Shows:
   # ✓ 2024_01_01_create_users - Safe
   # ⚠ 2024_01_02_add_email - 1 warning (ADD COLUMN with DEFAULT)
   # ✗ 2024_01_03_drop_column - 1 violation (DROP COLUMN)
   #
   # Run migrations? (y/N/review)
   ```

5. **Diesel schema.rs validation**
   - Detect schema.rs drift from actual database
   - Validate that migrations will produce expected schema
   - Warn about schema changes not reflected in migrations

6. **Migration splitting assistant**
   ```bash
   diesel-guard migrate split 2024_01_02_add_email

   # Automatically splits unsafe migration into safe multi-step migrations:
   # 2024_01_02_01_add_email_column
   # 2024_01_02_02_add_email_default
   ```

#### Configuration

```toml
# diesel-guard.toml
# Enforce timeout settings when running migrations
enforce_timeouts = true

# Timeouts applied if migration doesn't specify them
# These are set before each migration runs
lock_timeout = "2s"
statement_timeout = "5s"
idle_in_transaction_session_timeout = "10s"

# Include timeout settings in generated migrations
include_timeouts = true
```

## Contributing

We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and testing guide.

For AI assistants working on this project, see [AGENTS.md](AGENTS.md) for detailed implementation patterns.

## Credits

Inspired by [strong_migrations](https://github.com/ankane/strong_migrations) by Andrew Kane

## License

MIT
