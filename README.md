# Diesel Guard

![Build Status](https://github.com/ayarotsky/diesel-guard/actions/workflows/ci.yml/badge.svg?branch=main)

Catch dangerous PostgreSQL migrations before they take down production.

✓ Detects operations that lock tables or cause downtime
✓ Provides safe alternatives for each blocking operation
✓ Works with both Diesel and SQLx migration frameworks
✓ Supports safety-assured blocks for verified operations

## Installation

```sh
cargo install diesel-guard
```

## How It Works

diesel-guard analyzes your migration SQL and catches dangerous operations before they reach production.

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

## Supported Frameworks

diesel-guard supports both **Diesel** and **SQLx** PostgreSQL migrations. The framework is configured via `diesel-guard.toml` (see [Configuration](#configuration)).

### Diesel

Diesel's directory-based migration structure:

```
migrations/
└── 2024_01_01_000000_create_users/
    ├── up.sql
    ├── down.sql
    └── metadata.toml (optional)
```

### SQLx

SQLx supports multiple migration file formats. diesel-guard handles all of them:

#### Format 1: Suffix-based (recommended)
Most common SQLx format with separate up/down files:

```
migrations/
├── 20240101000000_create_users.up.sql
└── 20240101000000_create_users.down.sql
```

#### Format 2: Single file (up-only)
Single migration file without rollback:

```
migrations/
└── 20240101000000_create_users.sql
```

#### Format 3: Marker-based
Single file with both up and down sections:

```sql
-- migrations/20240101000000_create_users.sql

-- migrate:up
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    email VARCHAR(255) NOT NULL
);

-- migrate:down
DROP TABLE users;
```

#### Format 4: Directory-based
Similar to Diesel but with SQLx timestamp format:

```
migrations/
└── 20240101000000_create_users/
    ├── up.sql
    └── down.sql
```

### Framework Configuration

diesel-guard requires explicit framework configuration in `diesel-guard.toml`:

```toml
# Framework configuration (REQUIRED)
framework = "diesel"  # or "sqlx"
```

Generate a config file with:
```sh
diesel-guard init
```

See the [Configuration](#configuration) section for all available options.

### SQLx Metadata Directives

SQLx uses comment directives for migration metadata. diesel-guard recognizes these and validates their usage:

```sql
-- no-transaction

CREATE INDEX CONCURRENTLY idx_users_email ON users(email);
```

diesel-guard will warn you if you use `CONCURRENTLY` operations without the `-- no-transaction` directive.

## Checks

- [Adding a column with a default value](#adding-a-column-with-a-default-value)
- [Dropping a column](#dropping-a-column)
- [Dropping a primary key](#dropping-a-primary-key)
- [Dropping a table](#dropping-a-table)
- [Dropping a database](#dropping-a-database)
- [Dropping an index non-concurrently](#dropping-an-index-non-concurrently)
- [Reindexing without CONCURRENTLY](#reindexing-without-concurrently)
- [Adding an index non-concurrently](#adding-an-index-non-concurrently)
- [Adding a UNIQUE constraint](#adding-a-unique-constraint)
- [Changing column type](#changing-column-type)
- [Adding a NOT NULL constraint](#adding-a-not-null-constraint)
- [Adding a primary key to an existing table](#adding-a-primary-key-to-an-existing-table)
- [Creating extensions](#creating-extensions)
- [Adding a stored GENERATED column](#adding-a-stored-generated-column)
- [Unnamed constraints](#unnamed-constraints)
- [Renaming a column](#renaming-a-column)
- [Renaming a table](#renaming-a-table)
- [Short integer primary keys](#short-integer-primary-keys)
- [Adding a SERIAL column to an existing table](#adding-a-serial-column-to-an-existing-table)
- [Adding a JSON column](#adding-a-json-column)
- [Using CHAR/CHARACTER types](#using-charcharacter-types)
- [Using TIMESTAMP without time zone](#using-timestamp-without-time-zone)
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

### Dropping a table

#### Bad

Dropping a table permanently deletes all data, indexes, triggers, and constraints. This operation acquires an ACCESS EXCLUSIVE lock and cannot be undone after the transaction commits. Foreign key relationships in other tables may block the drop or cause cascading deletes.

```sql
DROP TABLE users;
DROP TABLE IF EXISTS orders CASCADE;
```

#### Good

Before dropping a table in production, take these precautions:

```sql
-- Step 1: Verify the table is no longer in use
-- Check application code for references to this table
-- Monitor for queries against the table

-- Step 2: Check for foreign key dependencies
SELECT
  tc.table_name, kcu.column_name, rc.constraint_name
FROM information_schema.table_constraints tc
JOIN information_schema.key_column_usage kcu ON tc.constraint_name = kcu.constraint_name
JOIN information_schema.referential_constraints rc ON tc.constraint_name = rc.constraint_name
WHERE rc.unique_constraint_schema = 'public'
  AND rc.unique_constraint_name IN (
    SELECT constraint_name FROM information_schema.table_constraints
    WHERE table_name = 'users' AND constraint_type IN ('PRIMARY KEY', 'UNIQUE')
  );

-- Step 3: Ensure backups exist or data has been migrated

-- Step 4: Drop the table (use safety-assured if intentional)
-- safety-assured:start
DROP TABLE users;
-- safety-assured:end
```

**Important considerations:**
- Verify all application code references have been removed and deployed
- Check for foreign keys in other tables that reference this table
- Ensure data backups exist before dropping
- Consider renaming the table first (e.g., `users_deprecated`) and waiting before dropping

### Dropping a database

#### Bad

Dropping a database permanently deletes the entire database including all tables, data, and objects. This operation is irreversible. PostgreSQL requires exclusive access to the target database—all active connections must be terminated before the drop can proceed. The command cannot be executed inside a transaction block.

```sql
DROP DATABASE mydb;
DROP DATABASE IF EXISTS testdb;
```

#### Good

DROP DATABASE should almost never appear in application migrations. Database lifecycle should be managed through infrastructure automation or DBA operations.

```sql
-- For local development: use database setup scripts
-- For production: use infrastructure automation (Terraform, Ansible)
-- For test cleanup: coordinate with DBA or use dedicated test infrastructure

-- If absolutely necessary (e.g., test cleanup), use a safety-assured block:
-- safety-assured:start
DROP DATABASE test_db;
-- safety-assured:end
```

**Important considerations:**
- Database deletion should be handled by DBAs or infrastructure automation, not application migrations
- Ensure complete backups exist before proceeding
- Verify all connections to the database are terminated
- Consider using infrastructure tools (Terraform, Ansible) instead of migrations

**Note:** PostgreSQL 13+ supports `DROP DATABASE ... WITH (FORCE)` to terminate active connections automatically, but this makes the operation even more dangerous and should be used with extreme caution.

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

**Important:** CONCURRENTLY requires PostgreSQL 9.2+ and cannot run inside a transaction block.

**For Diesel migrations:** Add a `metadata.toml` file to your migration directory:

```toml
# migrations/2024_01_01_drop_user_index/metadata.toml
run_in_transaction = false
```

**For SQLx migrations:** Add the no-transaction directive at the top of your migration file:

```sql
-- no-transaction
DROP INDEX CONCURRENTLY idx_users_email;
```

**Note:** Dropping an index concurrently takes longer than a regular drop and uses more resources, but allows concurrent queries to continue. If it fails, the index may be left in an "invalid" state and should be dropped again.

### Reindexing without CONCURRENTLY

#### Bad

Reindexing without CONCURRENTLY acquires an ACCESS EXCLUSIVE lock on the table, blocking all operations until complete. Duration depends on index size.

```sql
REINDEX INDEX idx_users_email;
REINDEX TABLE users;
```

#### Good

Use CONCURRENTLY to reindex without blocking operations:

```sql
REINDEX INDEX CONCURRENTLY idx_users_email;
REINDEX TABLE CONCURRENTLY users;
```

**Important:** CONCURRENTLY requires PostgreSQL 12+ and cannot run inside a transaction block.

**For Diesel migrations:** Add a `metadata.toml` file to your migration directory:

```toml
# migrations/2024_01_01_reindex_users/metadata.toml
run_in_transaction = false
```

**For SQLx migrations:** Add the no-transaction directive at the top of your migration file:

```sql
-- no-transaction
REINDEX INDEX CONCURRENTLY idx_users_email;
```

**Note:** REINDEX CONCURRENTLY rebuilds the index without locking out writes. If it fails, the index may be left in an "invalid" state—check with `\d tablename` and run REINDEX again if needed.

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

**Important:** CONCURRENTLY cannot run inside a transaction block.

**For Diesel migrations:** Add a `metadata.toml` file to your migration directory:

```toml
# migrations/2024_01_01_add_user_index/metadata.toml
run_in_transaction = false
```

**For SQLx migrations:** Add the no-transaction directive at the top of your migration file:

```sql
-- no-transaction
CREATE INDEX CONCURRENTLY idx_users_email ON users(email);
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

### Adding a stored GENERATED column

Adding a `GENERATED ALWAYS AS ... STORED` column acquires an ACCESS EXCLUSIVE lock and triggers a full table rewrite because PostgreSQL must compute and store the expression value for every existing row.

#### Bad

```sql
ALTER TABLE products ADD COLUMN total_price INTEGER GENERATED ALWAYS AS (price * quantity) STORED;
```

#### Good

```sql
-- Step 1: Add a regular nullable column
ALTER TABLE products ADD COLUMN total_price INTEGER;

-- Step 2: Backfill in batches (outside migration)
UPDATE products SET total_price = price * quantity WHERE total_price IS NULL;

-- Step 3: Optionally add NOT NULL constraint
ALTER TABLE products ALTER COLUMN total_price SET NOT NULL;

-- Step 4: Use a trigger for new rows
CREATE FUNCTION compute_total_price() RETURNS TRIGGER AS $$
BEGIN
  NEW.total_price := NEW.price * NEW.quantity;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_total_price
BEFORE INSERT OR UPDATE ON products
FOR EACH ROW EXECUTE FUNCTION compute_total_price();
```

**Note:** PostgreSQL does not support VIRTUAL generated columns (only STORED). For new empty tables, GENERATED STORED columns are acceptable.

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

### Using CHAR/CHARACTER types

**Lock type:** None (best practice warning)

#### Bad

CHAR and CHARACTER types are fixed-length and padded with spaces. This wastes storage and can cause subtle bugs with string comparisons and equality checks.

```sql
ALTER TABLE users ADD COLUMN country_code CHAR(2);
CREATE TABLE products (sku CHARACTER(10) PRIMARY KEY);
```

#### Good

Use TEXT or VARCHAR instead:

```sql
-- For ALTER TABLE
ALTER TABLE users ADD COLUMN country_code TEXT;
ALTER TABLE users ADD COLUMN country_code VARCHAR(2);

-- For CREATE TABLE
CREATE TABLE products (sku TEXT);
CREATE TABLE products (sku VARCHAR(10));

-- Or TEXT with CHECK constraint for length validation
ALTER TABLE users ADD COLUMN country_code TEXT CHECK (length(country_code) = 2);
CREATE TABLE products (sku TEXT CHECK (length(sku) <= 10));
```

**Why CHAR is problematic:**
- Fixed-length padding wastes storage
- Trailing spaces affect equality comparisons (`'US' != 'US  '`)
- DISTINCT, GROUP BY, and joins may behave unexpectedly
- No performance benefit over VARCHAR or TEXT in PostgreSQL

### Using TIMESTAMP without time zone

**Lock type:** None (best practice warning)

#### Bad

TIMESTAMP (or TIMESTAMP WITHOUT TIME ZONE) stores values without timezone context, which can cause issues in multi-timezone applications, during DST transitions, and makes it difficult to determine the actual point in time represented.

```sql
-- ALTER TABLE
ALTER TABLE events ADD COLUMN created_at TIMESTAMP;
ALTER TABLE events ADD COLUMN updated_at TIMESTAMP WITHOUT TIME ZONE;

-- CREATE TABLE
CREATE TABLE events (
    id SERIAL PRIMARY KEY,
    created_at TIMESTAMP,
    updated_at TIMESTAMP WITHOUT TIME ZONE
);
```

#### Good

Use TIMESTAMPTZ (TIMESTAMP WITH TIME ZONE) instead:

```sql
-- ALTER TABLE
ALTER TABLE events ADD COLUMN created_at TIMESTAMPTZ;
ALTER TABLE events ADD COLUMN updated_at TIMESTAMP WITH TIME ZONE;

-- CREATE TABLE
CREATE TABLE events (
    id SERIAL PRIMARY KEY,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMP WITH TIME ZONE
);
```

**Why TIMESTAMPTZ is better:**
- Stores values in UTC internally and converts on input/output based on session timezone
- Provides consistent behavior across different timezones and server environments
- Handles DST transitions correctly
- Makes it clear what point in time is represented

**When TIMESTAMP without time zone might be acceptable:**
- Storing dates that are inherently timezone-agnostic (e.g., birth dates stored as midnight)
- Legacy systems where all data is known to be in a single timezone
- Use `safety-assured` if you've confirmed timezone-naive timestamps are appropriate

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
      - uses: ayarotsky/diesel-guard@v0.4.0
        with:
          path: migrations/
```

**Versioning:**
- The action automatically installs the diesel-guard CLI version matching the tag
- `@v0.4.0` installs diesel-guard v0.4.0
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
# Framework configuration (REQUIRED)
# Specify which migration framework you're using
# Valid values: "diesel" or "sqlx"
framework = "diesel"

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
- `AddJsonColumnCheck` - ADD COLUMN with JSON type
- `AddNotNullCheck` - ALTER COLUMN SET NOT NULL
- `AddPrimaryKeyCheck` - ADD PRIMARY KEY to existing table
- `AddSerialColumnCheck` - ADD COLUMN with SERIAL
- `AddUniqueConstraintCheck` - ADD UNIQUE constraint via ALTER TABLE
- `AlterColumnTypeCheck` - ALTER COLUMN TYPE
- `CharTypeCheck` - CHAR/CHARACTER column types
- `CreateExtensionCheck` - CREATE EXTENSION
- `DropColumnCheck` - DROP COLUMN
- `DropDatabaseCheck` - DROP DATABASE
- `DropIndexCheck` - DROP INDEX without CONCURRENTLY
- `DropPrimaryKeyCheck` - DROP PRIMARY KEY
- `DropTableCheck` - DROP TABLE
- `GeneratedColumnCheck` - ADD COLUMN with GENERATED STORED
- `ReindexCheck` - REINDEX without CONCURRENTLY
- `RenameColumnCheck` - RENAME COLUMN
- `RenameTableCheck` - RENAME TABLE
- `ShortIntegerPrimaryKeyCheck` - SMALLINT/INT/INTEGER primary keys
- `TimestampTypeCheck` - TIMESTAMP without time zone
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

## Contributing

We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and testing guide.

For AI assistants working on this project, see [AGENTS.md](AGENTS.md) for detailed implementation patterns.

## Credits

Inspired by [strong_migrations](https://github.com/ankane/strong_migrations) by Andrew Kane

## License

MIT
