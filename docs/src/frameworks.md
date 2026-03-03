# Framework Adapters

diesel-guard supports both **Diesel** and **SQLx** Postgres migrations. The framework is configured via `diesel-guard.toml` (see [Configuration](configuration.md)).

## Diesel

Diesel uses a directory-based migration structure:

```
migrations/
└── 2024_01_01_000000_create_users/
    ├── up.sql
    ├── down.sql
    └── metadata.toml (optional)
```

Supported timestamp formats for directory names:
- `YYYY_MM_DD_HHMMSS` (e.g., `2024_01_01_000000`)
- `YYYY-MM-DD-HHMMSS`
- `YYYYMMDDHHMMSS`

## SQLx

SQLx supports multiple migration file formats. diesel-guard handles all of them.

### Format 1: Suffix-based (recommended)

Most common SQLx format with separate up/down files:

```
migrations/
├── 20240101000000_create_users.up.sql
└── 20240101000000_create_users.down.sql
```

### Format 2: Single file (up-only)

Single migration file without rollback:

```
migrations/
└── 20240101000000_create_users.sql
```

SQLx versions are any positive integer (e.g., `1`, `001`, `42`, `20240101000000`). Short numeric versions use numeric comparison for `start_after` filtering; 14-digit timestamps use string comparison.

## Framework Configuration

diesel-guard requires explicit framework configuration in `diesel-guard.toml`:

```toml
# Framework configuration (REQUIRED)
framework = "diesel"  # or "sqlx"
```

Generate a config file with:

```sh
diesel-guard init
```

## SQLx Metadata Directives

SQLx uses comment directives for migration metadata. diesel-guard recognizes these and validates their usage:

```sql
-- no-transaction

CREATE INDEX CONCURRENTLY idx_users_email ON users(email);
```

diesel-guard will warn you if you use `CONCURRENTLY` operations without the `-- no-transaction` directive, since `CONCURRENTLY` cannot run inside a transaction block.

## Diesel Metadata

For Diesel, use a `metadata.toml` file in the migration directory to run outside a transaction:

```toml
# migrations/2024_01_01_add_user_index/metadata.toml
run_in_transaction = false
```

This is required for any migration using `CONCURRENTLY` operations (CREATE INDEX CONCURRENTLY, DROP INDEX CONCURRENTLY, REINDEX CONCURRENTLY).
