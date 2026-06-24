---
name: diesel-guard
description: Lints Diesel and SQLx Postgres migrations for unsafe schema changes that lock tables or cause downtime, and authors custom Rhai checks. Use when reviewing, writing, or fixing SQL migrations, when diesel-guard reports a violation, when configuring diesel-guard.toml, or when the user mentions migration safety, table locks, or zero-downtime migrations.
---

# diesel-guard

diesel-guard is a CLI that lints Diesel and SQLx Postgres migrations using PostgreSQL's own parser
(libpg_query). It flags operations that take dangerous locks or rewrite tables, and prints a safe
alternative for each one. It needs no database connection — it reads `.sql` files directly. Use it to
vet a migration before it ships, to fix a reported violation, or to add project-specific rules.

## Core workflow: run → interpret → fix

1. **Run** the linter:
   ```sh
   diesel-guard check                 # checks ./migrations/ by default
   diesel-guard check path/to/up.sql  # a single file
   cat up.sql | diesel-guard check -  # stdin
   diesel-guard check migrations/ --format json   # text (default) | json | github
   ```

2. **Interpret** the result. Exit code is the contract:
   - `0` — clean, or only warnings (warnings never block).
   - `1` — at least one error-level violation, **or** a fatal error (e.g. invalid `diesel-guard.toml`,
     unparsable SQL, or a bad invocation). Read stderr to tell the two apart.

   Each violation has three fields: `operation` (what was flagged), `problem` (why it's dangerous),
   and `safe_alternative` (what to do instead). In JSON each also carries `check_name`, `line`, and
   `severity`.

3. **Fix** by applying the printed `safe_alternative`, then re-run to confirm `0` (see below).

## Fixing violations

Apply the `safe_alternative` printed for the violation, then re-run until the exit code is `0`. Prefer
fixing the migration over suppressing the check.

Get the full reasoning and exact rewrite for any check by name — including the most common ones
(`AddColumnCheck`, `AddIndexCheck`, `AddNotNullCheck`) — from the tool itself:

```sh
diesel-guard explain <CheckName>
```

Treat that output and the violation's own `safe_alternative` as the source of truth; do not recite
fixes from memory.

## Escape hatches (only when the operation is genuinely verified safe)

- **Suppress a range** — wrap statements (no nesting):
  ```sql
  -- safety-assured:start
  ALTER TABLE users DROP COLUMN legacy_field;
  -- safety-assured:end
  ```
- **Disable named checks for the whole file** (comma-separated):
  ```sql
  -- diesel-guard:disable AddColumnCheck, DropColumnCheck
  ```
- **Diesel per-migration** — in that migration's `metadata.toml`:
  ```toml
  run_in_transaction = false        # allows CONCURRENTLY operations
  disable_checks = ["AddColumnCheck"]
  ```
- **SQLx per-migration** — make the migration non-transactional (required for `CONCURRENTLY`) by
  putting this as the **first line** of the file:
  ```sql
  -- no-transaction
  ```

## Configuring `diesel-guard.toml`

Run `diesel-guard init` to scaffold the file (use `--force` to overwrite). Keys:

- `framework` (**required**) — `"diesel"` or `"sqlx"`. Case-sensitive.
- `start_after` — skip migrations older than this timestamp (Diesel accepts `YYYYMMDDHHMMSS`,
  `YYYY_MM_DD_HHMMSS`, `YYYY-MM-DD-HHMMSS`; SQLx uses the version number). Good for retrofitting.
- `check_down` (default `false`) — also check rollback/down migrations.
- `disable_checks` — blacklist of check names to skip.
- `enable_checks` — whitelist; only these run. **Mutually exclusive** with `disable_checks`.
- `warn_checks` — demote these checks to warnings (reported, but exit stays `0`).
- `custom_checks_dir` — directory of `.rhai` custom checks.
- `postgres_version` — target major version (e.g. `16`); silences checks that are safe from that
  version onward.

## Discovering checks (the source of truth)

The tool serves the live, config-aware list — do not hardcode it:

```sh
diesel-guard list-checks                 # every check: NAME, TYPE, SEVERITY, ENABLED
diesel-guard list-checks --format json
diesel-guard explain AddIndexCheck       # full description + safe alternative for one check
```

## Migration layouts

- **Diesel** — one directory per migration containing `up.sql` (and optional `down.sql`,
  `metadata.toml`). `check` scans `up.sql` recursively.
- **SQLx** — flat files: `<version>_<name>.up.sql` / `.down.sql`, or single-file `<version>_<name>.sql`.

## Writing custom checks

Write project-specific rules in [Rhai](https://rhai.rs) with full access to the parsed SQL AST — no
forking required.

### Setup

Point `custom_checks_dir` at a directory of `.rhai` files in `diesel-guard.toml`:

```toml
custom_checks_dir = "checks"
```

Every `.rhai` file in that directory becomes a check. They load in alphabetical order. A check's name
is its filename stem (`require_concurrent_index.rhai` → `require_concurrent_index`), so it can be
listed by `list-checks`, disabled via `disable_checks`/`enable_checks`, and explained via `explain`.
Compilation errors are **non-fatal** — they become warnings on stderr and the other checks still run.
Safety-assured blocks and `-- diesel-guard:disable` apply to custom checks too.

### Inspect the AST with dump-ast

Before writing a check, see exactly what node a statement produces:

```sh
diesel-guard dump-ast --sql "CREATE INDEX idx ON t(id);"
diesel-guard dump-ast --file migrations/.../up.sql
```

The output strips the outer `RawStmt`/`Node` wrappers and starts at the concrete node type (e.g.
`{"IndexStmt": {...}}`) — that is precisely the shape your script receives as `node`.

### Script inputs

Each script runs once per parsed statement with three variables in scope:

- `node` — the AST node. Reach into the concrete type by name: `node.IndexStmt.concurrent`,
  `node.CreateStmt.relation.relname`, `node.DropStmt.remove_type`. A field that doesn't apply to the
  current statement is absent, so guard with `??` (see below).
- `config` — the active configuration, e.g. `config.postgres_version` (an integer, or `()` when unset).
- `ctx` — per-migration context: `ctx.run_in_transaction` (bool) and `ctx.no_transaction_hint` (a
  framework-specific string explaining how to make the migration non-transactional).

### Return protocol

Return exactly one of:

- `()` — no violation.
- `#{ operation, problem, safe_alternative }` — one violation. All three values must be strings.
- `[#{ ... }, #{ ... }]` — an array of such maps for multiple violations.

A bad return value (wrong type, a map missing a key, or a non-string value) **and** any runtime error
thrown while the script runs do not crash diesel-guard — each produces a `SCRIPT ERROR: <check-name>`
violation. That violation is **error severity by default**, so it makes `check` exit `1` just like a
real finding; add the check's name to `warn_checks` in `diesel-guard.toml` to demote it to a warning.
Scripts never panic.

(This is distinct from *compile* errors at load time, described under [Setup](#setup), which are
non-fatal and reported as warnings on stderr.)

### `pg::` constants

Reference pg_query protobuf enum values by name instead of raw integers via the `pg::` module:

- Object types: `pg::OBJECT_INDEX`, `pg::OBJECT_TABLE`, `pg::OBJECT_COLUMN`, `pg::OBJECT_DATABASE`,
  `pg::OBJECT_SCHEMA`, `pg::OBJECT_SEQUENCE`, `pg::OBJECT_VIEW`, `pg::OBJECT_FUNCTION`,
  `pg::OBJECT_EXTENSION`, `pg::OBJECT_TRIGGER`, `pg::OBJECT_TYPE`
- ALTER TABLE subtypes: `pg::AT_ADD_COLUMN`, `pg::AT_COLUMN_DEFAULT`, `pg::AT_DROP_NOT_NULL`,
  `pg::AT_SET_NOT_NULL`, `pg::AT_DROP_COLUMN`, `pg::AT_ALTER_COLUMN_TYPE`, `pg::AT_ADD_CONSTRAINT`,
  `pg::AT_DROP_CONSTRAINT`, `pg::AT_VALIDATE_CONSTRAINT`
- Constraint types: `pg::CONSTR_NOTNULL`, `pg::CONSTR_DEFAULT`, `pg::CONSTR_IDENTITY`,
  `pg::CONSTR_GENERATED`, `pg::CONSTR_CHECK`, `pg::CONSTR_PRIMARY`, `pg::CONSTR_UNIQUE`,
  `pg::CONSTR_EXCLUSION`, `pg::CONSTR_FOREIGN`
- Drop behavior: `pg::DROP_RESTRICT`, `pg::DROP_CASCADE`

`src/scripting.rs` in the diesel-guard source is the authoritative list if you need one not shown here.

### `describe()`

Optionally define `fn describe()` returning a string. `diesel-guard explain <name>` shows it.

```rhai
fn describe() {
    "Requires CONCURRENTLY on every CREATE INDEX."
}
```

### Engine limits

Per script: `max_operations` 100,000 · `max_string_size` 10,000 · `max_array_size` 1,000 ·
`max_map_size` 1,000.

### Worked example

Require `CONCURRENTLY` on `CREATE INDEX`, and also flag `CONCURRENTLY` used inside a transaction
(Postgres rejects that at runtime):

```rhai
let stmt = node.IndexStmt ?? return;

if !stmt.concurrent {
    let idx_name = if stmt.idxname != "" { stmt.idxname } else { "(unnamed)" };
    return #{
        operation: "INDEX without CONCURRENTLY: " + idx_name,
        problem: "Creating index '" + idx_name + "' without CONCURRENTLY blocks writes on the table.",
        safe_alternative: "Use CREATE INDEX CONCURRENTLY:\n  CREATE INDEX CONCURRENTLY " + idx_name + " ON ...;"
    };
}

if ctx.run_in_transaction {
    let hint = if ctx.no_transaction_hint != "" { ctx.no_transaction_hint }
               else { "Run this migration outside a transaction block." };
    #{
        operation: "INDEX CONCURRENTLY inside a transaction",
        problem: "CREATE INDEX CONCURRENTLY cannot run inside a transaction block; Postgres errors at runtime.",
        safe_alternative: hint
    }
}
```

`node.IndexStmt ?? return;` bails when the statement isn't a `CREATE INDEX`.

### More examples

Five more complete, runnable checks. Together with the worked example above they cover the common
patterns: the `??` guard, optional chaining (`?.`), `pg::` constants, returning an array, and
iterating child nodes. Copy one into your `custom_checks_dir` as a starting point.

**Require `IF EXISTS` on `DROP TABLE`** — uses a `pg::` constant and `missing_ok`:

```rhai
let stmt = node.DropStmt ?? return;
if stmt.remove_type != pg::OBJECT_TABLE || stmt.missing_ok { return; }

#{
    operation: "DROP TABLE without IF EXISTS",
    problem: "DROP TABLE without IF EXISTS will error if the table doesn't exist, potentially breaking migrations.",
    safe_alternative: "Use IF EXISTS:\n  DROP TABLE IF EXISTS <table_name>;"
}
```

**Ban `UNLOGGED` tables** — optional chaining on a nested field (`relpersistence` is `"u"` when unlogged):

```rhai
let rel = node.CreateStmt?.relation ?? return;
if rel.relpersistence != "u" { return; }

let table_name = rel.relname;
#{
    operation: "UNLOGGED TABLE: " + table_name,
    problem: "UNLOGGED tables are not crash-safe and are not replicated to standby servers.",
    safe_alternative: "Use a regular (logged) table instead:\n  CREATE TABLE " + table_name + " (...);"
}
```

**Ban `TRUNCATE`** — iterate child nodes and return an array of violations:

```rhai
let stmt = node.TruncateStmt ?? return;

let violations = [];
for rel in stmt.relations {
    let name = rel.node?.RangeVar?.relname ?? continue;
    violations.push(#{
        operation: "TRUNCATE: " + name,
        problem: "TRUNCATE acquires ACCESS EXCLUSIVE lock on '" + name + "', blocking all reads and writes for the duration. Unlike DELETE, it also resets sequences and skips triggers.",
        safe_alternative: "Use batched DELETE instead:\n  DELETE FROM " + name + " WHERE id IN (SELECT id FROM " + name + " LIMIT 1000);"
    });
}
violations
```

**Limit index width to 3 columns** — read a config-like constant and count `index_params`:

```rhai
let max_cols = 3;
let stmt = node.IndexStmt ?? return;

let col_count = stmt.index_params.len();
if col_count > max_cols {
    let idx_name = if stmt.idxname != "" { stmt.idxname } else { "(unnamed)" };
    #{
        operation: "Wide index: " + idx_name + " (" + col_count + " columns)",
        problem: "Index '" + idx_name + "' has " + col_count + " columns (limit: " + max_cols + "). Wide indexes are rarely effective and slow down writes.",
        safe_alternative: "Use narrower indexes targeting specific query patterns, or partial/covering indexes."
    }
}
```

**Enforce an index naming convention** — string method on a field:

```rhai
let name = node.IndexStmt?.idxname ?? return;
if name == "" || name.starts_with("idx_") { return; }

#{
    operation: "Index naming violation: " + name,
    problem: "Index '" + name + "' does not follow naming convention. Index names should start with 'idx_'.",
    safe_alternative: "Rename the index:\n  CREATE INDEX idx_" + name + " ON ...;"
}
```
