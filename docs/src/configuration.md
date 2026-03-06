# Configuration

Create a `diesel-guard.toml` file in your project root to customize behavior.

## Initialize

Generate a documented configuration file:

```sh
diesel-guard init
```

Use `--force` to overwrite an existing file:

```sh
diesel-guard init --force
```

## All Options

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

# Disable specific checks (blacklist)
disable_checks = ["AddColumnCheck"]

# Run only specific checks (whitelist). Cannot be used with disable_checks.
enable_checks = ["AddIndexCheck", "AddNotNullCheck"]

# Downgrade specific checks to warnings instead of errors.
# Warnings are reported in output but do not cause a non-zero exit code.
# Useful for checks like TruncateTableCheck that are context-dependent.
warn_checks = ["TruncateTableCheck"]

# Directory containing custom Rhai check scripts
custom_checks_dir = "checks"

# Target Postgres major version.
# When set, version-aware checks adjust their behavior accordingly.
# Example: setting 11 allows ADD COLUMN with constant DEFAULT (safe on PG 11+),
# but still warns for volatile defaults like DEFAULT now() on all versions.
postgres_version = 16
```

## Available Check Names

Use these names in `disable_checks` (blacklist), `enable_checks` (whitelist), or `warn_checks` (downgrade to warning):

| Check Name | Operation |
|---|---|
| `AddColumnCheck` | ADD COLUMN with DEFAULT |
| `AddIndexCheck` | CREATE INDEX without CONCURRENTLY; CONCURRENTLY inside a transaction |
| `AddJsonColumnCheck` | ADD COLUMN with JSON type |
| `AddNotNullCheck` | ALTER COLUMN SET NOT NULL |
| `AddPrimaryKeyCheck` | ADD PRIMARY KEY to existing table |
| `AddSerialColumnCheck` | ADD COLUMN with SERIAL |
| `AddUniqueConstraintCheck` | ADD UNIQUE constraint via ALTER TABLE |
| `AlterColumnTypeCheck` | ALTER COLUMN TYPE |
| `CharTypeCheck` | CHAR/CHARACTER column types |
| `CreateExtensionCheck` | CREATE EXTENSION |
| `DropColumnCheck` | DROP COLUMN |
| `DropDatabaseCheck` | DROP DATABASE |
| `DropIndexCheck` | DROP INDEX without CONCURRENTLY; CONCURRENTLY inside a transaction |
| `DropPrimaryKeyCheck` | DROP PRIMARY KEY |
| `DropTableCheck` | DROP TABLE |
| `GeneratedColumnCheck` | ADD COLUMN with GENERATED STORED |
| `ReindexCheck` | REINDEX without CONCURRENTLY; CONCURRENTLY inside a transaction |
| `RenameColumnCheck` | RENAME COLUMN |
| `RenameTableCheck` | RENAME TABLE |
| `ShortIntegerPrimaryKeyCheck` | SMALLINT/INT/INTEGER primary keys |
| `TimestampTypeCheck` | TIMESTAMP without time zone |
| `TruncateTableCheck` | TRUNCATE TABLE |
| `UnnamedConstraintCheck` | Unnamed constraints (UNIQUE, FOREIGN KEY, CHECK) |
| `WideIndexCheck` | Indexes with 4+ columns |

Custom check names are the filename stem of the `.rhai` file (e.g., `require_concurrent_index.rhai` → `require_concurrent_index`).
