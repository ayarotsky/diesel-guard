# AGENTS.md - diesel-guard

Detects unsafe PostgreSQL migration patterns before they cause production incidents. Parses SQL using `pg_query` (PostgreSQL's actual parser via libpg_query) and identifies operations that acquire dangerous locks or trigger table rewrites. Supports both built-in Rust checks and user-defined Rhai script checks.

**Core Tech:** Rust, `pg_query`, Rhai scripting, Diesel/SQLx migrations, PostgreSQL 9.6+

## Architecture

- `src/checks/pg_helpers.rs` — Public AST navigation functions; `extract_node()` unwraps `RawStmt` → `NodeEnum`
- `src/checks/mod.rs` — `Check` trait (`Send + Sync`), `Registry` with `add_check()`, `check_stmts_with_context`
- `src/parser/mod.rs` — `parse()` wraps `pg_query::parse()`, `parse_with_metadata()` adds safety-assured blocks
- `src/safety_checker.rs` — Entry point; loads custom Rhai checks from `custom_checks_dir` config
- `src/scripting.rs` — `CustomCheck` (implements `Check`), sandboxed Rhai engine, `load_custom_checks()`
- `src/ast_dump.rs` — `dump_ast()` for `dump-ast` CLI subcommand (JSON AST output)
- `src/config.rs` — Config with `custom_checks_dir: Option<String>`; `disable_checks` warns (not errors) on unknown names

## How to Add a Built-in Check

1. **Create** `src/checks/your_check.rs` — implement `Check` trait, add `#[cfg(test)]` unit tests using `assert_detects_violation!` / `assert_allows!` macros. Follow existing checks for patterns.
2. **Register** in `src/checks/mod.rs` — add `mod`, `pub use`, and `register_check` call (all alphabetically). Check names are derived from struct names automatically.
3. **Create fixtures** — `tests/fixtures/your_operation_{safe,unsafe}/up.sql`. First line MUST be `-- Safe: ...` or `-- Unsafe: ...`.
4. **Update integration tests** in `tests/fixtures_test.rs` — add to `safe_fixtures` vec, add detection test, update `test_check_entire_fixtures_directory` counts.
5. **Update README** — add to "Supported Checks" section.
6. **Verify** — `cargo test && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings`

## How Custom Rhai Checks Work

- Users place `.rhai` files in a directory and set `custom_checks_dir` in `diesel-guard.toml`
- Each script receives a `node` variable (pg_query AST node serialized via `rhai::serde::to_dynamic()`)
- Scripts access fields like `node.IndexStmt.concurrent`, `node.CreateStmt.relation.relname`
- Return protocol: `()` = no violation, `#{ operation, problem, safe_alternative }` = one, array of maps = multiple
- Check name = filename stem (e.g., `require_concurrent.rhai` → `require_concurrent`); disableable via `disable_checks`
- Safety-assured blocks automatically skip custom checks (same `check_stmts_with_context` path)
- Engine limits: `max_operations(100_000)`, `max_string_size(10_000)`, `max_array_size(1_000)`, `max_map_size(1_000)`
- Runtime errors and invalid return values are logged as warnings to stderr, never panic
- `dump-ast` CLI subcommand helps users inspect AST structure: `diesel-guard dump-ast --sql "..."`
- See `examples/no_unlogged_tables.rhai` for a reference script

## Naming Conventions

- **Check structs**: `YourOperationCheck`
- **Tests**: `test_detects_*` (violation found), `test_allows_*` (safe variant), `test_ignores_*` (unrelated operation)
- **Fixtures**: `{operation}_{safe|unsafe}` or `{operation}_{variant}_{safe|unsafe}`

## Non-Obvious Gotchas

- **RenameStmt separation**: `ALTER TABLE t RENAME COLUMN/TO` is `RenameStmt` in pg_query, NOT `AlterTableStmt`. Check `rename_type` field to distinguish column vs table renames.
- **FK columns vs constraint keys**: `constraint_columns_str()` reads from `Constraint.keys` — works for UNIQUE/CHECK/PK. FK columns are in `fk_attrs`, not `keys`.
- **Protobuf default values**: Fields with value 0 may be omitted. Match on node type presence rather than `subtype == 0`.
- **Fixture counts**: When adding fixtures, update both file count and violation count in `test_check_entire_fixtures_directory`. Some fixtures produce multiple violations due to check overlaps — read the assertion message for the breakdown.
- **Macros position**: Keep macros before `mod test_helpers` in `test_utils.rs` to avoid `clippy::items_after_test_module`.
- **Rhai `sync` feature**: Required because `Check` trait is `Send + Sync`. Without it, `CustomCheck` won't compile.
- **`Violation.operation` is `String`**: Not `&'static str`. Changed to support runtime-built strings from Rhai. `"literal".into()` works automatically for built-in checks.
- **`disable_checks` validation is relaxed**: Unknown names produce a warning, not an error, so users can disable custom check names without the validator rejecting them.
- **`extract_node()` in `pg_helpers`**: Use this instead of manually unwrapping `raw_stmt.stmt.as_ref().and_then(|n| n.node.as_ref())`. It's the single source of truth for `RawStmt → NodeEnum` extraction.
