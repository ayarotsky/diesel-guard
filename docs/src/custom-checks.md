# Custom Checks

Built-in checks cover common Postgres migration hazards, but every project has unique rules — naming conventions, banned operations, team policies. Custom checks let you enforce these with simple [Rhai](https://rhai.rs) scripts.

Write your checks as `.rhai` files, point `custom_checks_dir` at the directory in `diesel-guard.toml`, and diesel-guard will run them alongside the built-in checks.

## Quick Start

1. Create a directory for your checks:

```sh
mkdir checks
```

2. Write a check script (e.g., `checks/require_concurrent_index.rhai`):

```rhai
let stmt = node.IndexStmt;
if stmt == () { return; }

if !stmt.concurrent {
    let idx_name = if stmt.idxname != "" { stmt.idxname } else { "(unnamed)" };
    #{
        operation: "INDEX without CONCURRENTLY: " + idx_name,
        problem: "Creating index '" + idx_name + "' without CONCURRENTLY blocks writes on the table.",
        safe_alternative: "Use CREATE INDEX CONCURRENTLY:\n  CREATE INDEX CONCURRENTLY " + idx_name + " ON ...;"
    }
}
```

3. Add to `diesel-guard.toml`:

```toml
custom_checks_dir = "checks"
```

4. Run as usual:

```sh
diesel-guard check migrations/
```

## How It Works

- Each `.rhai` script is called **once per SQL statement** in the migration
- The `node` variable contains the pg_query AST for that statement (a nested map)
- The `config` variable exposes the current `diesel-guard.toml` settings (e.g., `config.postgres_version`)
- Scripts match on a specific node type: `let stmt = node.IndexStmt;`
- If the node doesn't match, `node.IndexStmt` returns `()` — early-return with `if stmt == () { return; }`
- Return `()` for no violation, a map for one, or an array of maps for multiple
- Map keys: `operation`, `problem`, `safe_alternative` (all required strings)

## The `config` Variable

`config` gives scripts access to the user's configuration. Use it to make version-aware checks:

```rhai
// Only flag this on Postgres < 14
if config.postgres_version != () && config.postgres_version >= 14 { return; }
```

Available fields:

| Field | Type | Description |
|-------|------|-------------|
| `config.postgres_version` | integer or `()` | Target PG major version, or `()` if unset |
| `config.check_down` | bool | Whether down migrations are checked |
| `config.disable_checks` | array | Check names that are disabled |

## Using `dump-ast`

Use `dump-ast` to inspect the AST for any SQL statement. This is the easiest way to discover which fields are available:

```sh
diesel-guard dump-ast --sql "CREATE INDEX idx_users_email ON users(email);"
```

Key fields and how they map to Rhai (using `IndexStmt` as an example):

| JSON path | Rhai access | Description |
|-----------|-------------|-------------|
| `IndexStmt.concurrent` | `stmt.concurrent` | Whether `CONCURRENTLY` was specified |
| `IndexStmt.idxname` | `stmt.idxname` | Index name |
| `IndexStmt.unique` | `stmt.unique` | Whether it's a UNIQUE index |
| `IndexStmt.relation.relname` | `stmt.relation.relname` | Table name |
| `IndexStmt.index_params` | `stmt.index_params` | Array of indexed columns |

## Return Values

**No violation** — return `()` (either explicitly or by reaching the end of the script):

```rhai
let stmt = node.IndexStmt;
if stmt == () { return; }

if stmt.concurrent {
    return;  // All good, CONCURRENTLY is used
}
```

**Single violation** — return a map with `operation`, `problem`, and `safe_alternative`:

```rhai
#{
    operation: "INDEX without CONCURRENTLY: idx_users_email",
    problem: "Creating index without CONCURRENTLY blocks writes on the table.",
    safe_alternative: "Use CREATE INDEX CONCURRENTLY."
}
```

**Multiple violations** — return an array of maps:

```rhai
let violations = [];
for rel in stmt.relations {
    violations.push(#{
        operation: "TRUNCATE: " + rel.node.RangeVar.relname,
        problem: "TRUNCATE acquires ACCESS EXCLUSIVE lock.",
        safe_alternative: "Use batched DELETE instead."
    });
}
violations
```

## Common AST Node Types

| SQL | Node Type | Key Fields |
|-----|-----------|------------|
| `CREATE TABLE` | `CreateStmt` | `relation.relname`, `relation.relpersistence`, `table_elts` |
| `CREATE INDEX` | `IndexStmt` | `idxname`, `concurrent`, `unique`, `relation`, `index_params` |
| `ALTER TABLE` | `AlterTableStmt` | `relation`, `cmds` (array of `AlterTableCmd`) |
| `DROP TABLE/INDEX/...` | `DropStmt` | `remove_type`, `objects`, `missing_ok`, `behavior` |
| `ALTER TABLE RENAME` | `RenameStmt` | `rename_type`, `relation`, `subname`, `newname` |
| `TRUNCATE` | `TruncateStmt` | `relations` (array of Node-wrapped `RangeVar`) |
| `CREATE EXTENSION` | `CreateExtensionStmt` | `extname`, `if_not_exists` |
| `REINDEX` | `ReindexStmt` | `kind`, `concurrent`, `relation` |

**Note:** Column definitions (`ColumnDef`) are nested inside `CreateStmt.table_elts` and `AlterTableCmd.def`, not top-level nodes. Use `dump-ast` to explore the nesting for `ALTER TABLE ADD COLUMN` statements.

## `pg::` Constants

Protobuf enum fields like `DropStmt.remove_type` and `AlterTableCmd.subtype` are integer values. Instead of hard-coding magic numbers, use the built-in `pg::` module:

```rhai
// Instead of: stmt.remove_type == 42
if stmt.remove_type == pg::OBJECT_TABLE { ... }
```

### ObjectType

Used by `DropStmt.remove_type`, `RenameStmt.rename_type`, etc.

| Constant | Description |
|----------|-------------|
| `pg::OBJECT_INDEX` | Index |
| `pg::OBJECT_TABLE` | Table |
| `pg::OBJECT_COLUMN` | Column |
| `pg::OBJECT_DATABASE` | Database |
| `pg::OBJECT_SCHEMA` | Schema |
| `pg::OBJECT_SEQUENCE` | Sequence |
| `pg::OBJECT_VIEW` | View |
| `pg::OBJECT_FUNCTION` | Function |
| `pg::OBJECT_EXTENSION` | Extension |
| `pg::OBJECT_TRIGGER` | Trigger |
| `pg::OBJECT_TYPE` | Type |

### AlterTableType

Used by `AlterTableCmd.subtype`.

| Constant | Description |
|----------|-------------|
| `pg::AT_ADD_COLUMN` | ADD COLUMN |
| `pg::AT_COLUMN_DEFAULT` | SET DEFAULT / DROP DEFAULT |
| `pg::AT_DROP_NOT_NULL` | DROP NOT NULL |
| `pg::AT_SET_NOT_NULL` | SET NOT NULL |
| `pg::AT_DROP_COLUMN` | DROP COLUMN |
| `pg::AT_ALTER_COLUMN_TYPE` | ALTER COLUMN TYPE |
| `pg::AT_ADD_CONSTRAINT` | ADD CONSTRAINT |
| `pg::AT_DROP_CONSTRAINT` | DROP CONSTRAINT |
| `pg::AT_VALIDATE_CONSTRAINT` | VALIDATE CONSTRAINT |

### ConstrType

Used by `Constraint.contype`.

| Constant | Description |
|----------|-------------|
| `pg::CONSTR_NOTNULL` | NOT NULL |
| `pg::CONSTR_DEFAULT` | DEFAULT |
| `pg::CONSTR_IDENTITY` | IDENTITY |
| `pg::CONSTR_GENERATED` | GENERATED |
| `pg::CONSTR_CHECK` | CHECK |
| `pg::CONSTR_PRIMARY` | PRIMARY KEY |
| `pg::CONSTR_UNIQUE` | UNIQUE |
| `pg::CONSTR_EXCLUSION` | EXCLUSION |
| `pg::CONSTR_FOREIGN` | FOREIGN KEY |

### DropBehavior

Used by `DropStmt.behavior`.

| Constant | Description |
|----------|-------------|
| `pg::DROP_RESTRICT` | RESTRICT (default) |
| `pg::DROP_CASCADE` | CASCADE |

## Examples

The `examples/` directory contains ready-to-use scripts covering common patterns — naming conventions, banned operations, version-aware checks, and more. Browse them to get started or use as templates for your own checks.

## Disabling Custom Checks

Custom checks can be disabled in `diesel-guard.toml` using the filename stem as the check name:

```toml
# Disables checks/require_concurrent_index.rhai
disable_checks = ["require_concurrent_index"]
```

[`safety-assured` blocks](safety-assured.md) also suppress custom check violations — any SQL inside a safety-assured block is skipped by all checks, both built-in and custom.

## Debugging Tips

- **Inspect the AST:** Use `diesel-guard dump-ast --sql "..."` to see exactly what fields are available
- **Runtime errors:** Invalid field access or type errors produce stderr warnings — the check is skipped but other checks continue
- **Compilation errors:** Syntax errors in `.rhai` files are reported at startup
- **Infinite loops:** Scripts that exceed the operations limit are terminated safely with a warning
