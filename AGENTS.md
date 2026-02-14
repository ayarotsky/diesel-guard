# AGENTS.md - diesel-guard

Detects unsafe PostgreSQL migration patterns before they cause production incidents. Parses SQL using `pg_query` (PostgreSQL's actual parser via libpg_query) and identifies operations that acquire dangerous locks or trigger table rewrites.

**Core Tech:** Rust, `pg_query`, Diesel/SQLx migrations, PostgreSQL 9.6+

## How to Add a New Check

1. **Create** `src/checks/your_check.rs` — implement `Check` trait, add `#[cfg(test)]` unit tests using `assert_detects_violation!` / `assert_allows!` macros. Follow existing checks for patterns.
2. **Register** in `src/checks/mod.rs` — add `mod`, `pub use`, and `register_check` call (all alphabetically). Check names are derived from struct names automatically.
3. **Create fixtures** — `tests/fixtures/your_operation_{safe,unsafe}/up.sql`. First line MUST be `-- Safe: ...` or `-- Unsafe: ...`.
4. **Update integration tests** in `tests/fixtures_test.rs` — add to `safe_fixtures` vec, add detection test, update `test_check_entire_fixtures_directory` counts.
5. **Update README** — add to "Supported Checks" section.
6. **Verify** — `cargo test && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings`

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
