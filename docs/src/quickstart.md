# Quick Start

## Check a Single Migration File

```sh
diesel-guard check migrations/2024_01_01_create_users/up.sql
```

## Check All Migrations in a Directory

```sh
diesel-guard check migrations/
```

## Pipe SQL Directly

```sh
cat migrations/2024_01_01_create_users/up.sql | diesel-guard check -
```

## Example Output

When diesel-guard finds an unsafe operation:

```
❌ Unsafe migration detected in migrations/2024_01_01_create_users/up.sql

❌ ADD COLUMN with DEFAULT

Problem:
  Adding column 'admin' with DEFAULT on table 'users' requires a full table rewrite on Postgres < 11,
  which acquires an ACCESS EXCLUSIVE lock. On large tables, this can take significant time and block all operations.

Safe alternative:
  1. Add the column without a default:
     ALTER TABLE users ADD COLUMN admin BOOLEAN;

  2. Backfill data in batches (outside migration):
     UPDATE users SET admin = <value> WHERE admin IS NULL;

  3. Add default for new rows only:
     ALTER TABLE users ALTER COLUMN admin SET DEFAULT <value>;

  Note: For Postgres 11+, this is safe if the default is a constant value.
```

## JSON Output

For CI/CD or programmatic processing:

```sh
diesel-guard check migrations/ --format json
```

## Inspect the AST

Use `dump-ast` to see the pg_query AST as JSON — essential for writing [custom checks](custom-checks.md):

```sh
diesel-guard dump-ast --sql "CREATE INDEX idx_users_email ON users(email);"
diesel-guard dump-ast --file migration.sql
```

Example output:

```json
[
  {
    "IndexStmt": {
      "access_method": "btree",
      "concurrent": false,
      "idxname": "idx_users_email",
      "if_not_exists": false,
      "index_params": [
        {
          "node": {
            "IndexElem": {
              "name": "email",
              ...
            }
          }
        }
      ],
      "relation": {
        "relname": "users",
        "relpersistence": "p",
        ...
      },
      "unique": false,
      ...
    }
  }
]
```

The AST structure shown here tells you exactly which fields are available when writing custom Rhai checks. For example, `node.IndexStmt.concurrent` maps to the `concurrent` field above.
