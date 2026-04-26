# Checks

diesel-guard ships safety checks covering the most common Postgres migration hazards.

| Check | Operation | Lock Type |
|---|---|---|
| [Add Check Constraint](add-check-constraint.md) | `ALTER TABLE ... ADD CONSTRAINT ... CHECK` without `NOT VALID` | ACCESS EXCLUSIVE |
| [Add Primary Key](add-primary-key.md) | `ALTER TABLE ... ADD PRIMARY KEY` | ACCESS EXCLUSIVE |
| [ADD COLUMN with DEFAULT](add-column-default.md) | `ALTER TABLE ... ADD COLUMN ... DEFAULT` | ACCESS EXCLUSIVE + table rewrite |
| [Adding an EXCLUDE Constraint](add-exclude-constraint.md) | `ALTER TABLE ... ADD CONSTRAINT ... EXCLUDE` | SHARE ROW EXCLUSIVE + full table scan |
| [Add Foreign Key](add-foreign-key.md) | `ALTER TABLE ... ADD FOREIGN KEY` without `NOT VALID` | ShareRowExclusiveLock |
| [Adding an Index](add-index.md) | `CREATE INDEX` without `CONCURRENTLY`; `CREATE INDEX CONCURRENTLY` inside a transaction | SHARE |
| [Adding a UNIQUE Constraint](add-unique-constraint.md) | `ALTER TABLE ... ADD UNIQUE` | ACCESS EXCLUSIVE |
| [Alter Column Type](alter-column-type.md) | `ALTER TABLE ... ALTER COLUMN ... TYPE` | ACCESS EXCLUSIVE + table rewrite |
| [CHAR Type](char-type.md) | `CHAR`/`CHARACTER` column types | — (best practice) |
| [Create Table with SERIAL](create-table-serial.md) | `SERIAL/BIGSERIAL/SMALLSERIAL` in `CREATE TABLE` | — (best practice) |
| [Create Extension](create-extension.md) | `CREATE EXTENSION` | — (requires superuser) |
| [Domain CHECK Constraint](add-domain-check-constraint.md) | `ALTER DOMAIN ... ADD CONSTRAINT ... CHECK` without `NOT VALID` | ACCESS EXCLUSIVE |
| [Drop Column](drop-column.md) | `ALTER TABLE ... DROP COLUMN` | ACCESS EXCLUSIVE |
| [Drop Constraint](drop-constraint.md) | Unnamed `UNIQUE`/`FOREIGN KEY`/`CHECK` constraints | — (best practice) |
| [Drop Database](drop-database.md) | `DROP DATABASE` | Exclusive access |
| [Drop Index](drop-index.md) | `DROP INDEX` without `CONCURRENTLY`; `DROP INDEX CONCURRENTLY` inside a transaction | ACCESS EXCLUSIVE |
| [Drop Primary Key](drop-primary-key.md) | `ALTER TABLE ... DROP CONSTRAINT ... pkey` | ACCESS EXCLUSIVE |
| [Drop Table](drop-table.md) | `DROP TABLE` | ACCESS EXCLUSIVE |
| [Generated Columns](generated-column.md) | `ADD COLUMN ... GENERATED ALWAYS AS ... STORED` | ACCESS EXCLUSIVE + table rewrite |
| [Idempotency Guards](idempotency-guards.md) | Missing `IF [NOT] EXISTS` guards on retry-sensitive DDL | — (retry safety) |
| [Add JSON Column](add-json-column.md) | `ADD COLUMN ... JSON` | — (best practice) |
| [Mutation without WHERE](mutation-without-where.md) | `DELETE FROM table` or `UPDATE table SET ...` without `WHERE` | ACCESS EXCLUSIVE / ROW EXCLUSIVE |
| [Wide Indexes](wide-index.md) | `CREATE INDEX` with 4+ columns | — (best practice) |
| [REFRESH MATERIALIZED VIEW](refresh-materialized-view.md) | `REFRESH MATERIALIZED VIEW` without `CONCURRENTLY`; `REFRESH MATERIALIZED VIEW CONCURRENTLY` inside a transaction | ACCESS EXCLUSIVE |
| [Rename Column](rename-column.md) | `ALTER TABLE ... RENAME COLUMN` | ACCESS EXCLUSIVE |
| [Rename Schema](rename-schema.md) | `ALTER SCHEMA ... RENAME TO` | ACCESS EXCLUSIVE |
| [Rename Table](rename-table.md) | `ALTER TABLE ... RENAME TO` | ACCESS EXCLUSIVE |
| [REINDEX](reindex.md) | `REINDEX` without `CONCURRENTLY`; `REINDEX CONCURRENTLY` inside a transaction | ACCESS EXCLUSIVE |
| [Add Serial Column](add-serial-column.md) | `ADD COLUMN ... SERIAL/BIGSERIAL` | ACCESS EXCLUSIVE + table rewrite |
| [SET NOT NULL](set-not-null.md) | `ALTER TABLE ... ALTER COLUMN ... SET NOT NULL` | ACCESS EXCLUSIVE |
| [Short Primary Keys](short-primary-key.md) | `SMALLINT`/`INT` primary keys | — (best practice) |
| [TIMESTAMP Type](timestamp-type.md) | `TIMESTAMP` without time zone | — (best practice) |
| [Truncate Table](truncate-table.md) | `TRUNCATE TABLE` | ACCESS EXCLUSIVE |
| [Unnamed Constraints](unnamed-constraint.md) | Constraints without explicit names | — (best practice) |

Need project-specific rules beyond these? See [Custom Checks](../custom-checks.md).
