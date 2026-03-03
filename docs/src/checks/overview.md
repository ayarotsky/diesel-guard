# Checks

diesel-guard ships with 24 built-in safety checks covering the most common Postgres migration hazards.

| Check | Operation | Lock Type |
|---|---|---|
| [ADD COLUMN with DEFAULT](add-column-default.md) | `ALTER TABLE ... ADD COLUMN ... DEFAULT` | ACCESS EXCLUSIVE + table rewrite |
| [Adding an Index](adding-index.md) | `CREATE INDEX` without `CONCURRENTLY` | SHARE |
| [Adding a UNIQUE Constraint](adding-unique-constraint.md) | `ALTER TABLE ... ADD UNIQUE` | ACCESS EXCLUSIVE |
| [Changing Column Type](changing-column-type.md) | `ALTER TABLE ... ALTER COLUMN ... TYPE` | ACCESS EXCLUSIVE + table rewrite |
| [CHAR Fields](char-field.md) | `CHAR`/`CHARACTER` column types | — (best practice) |
| [Creating an Extension](creating-extension.md) | `CREATE EXTENSION` | — (requires superuser) |
| [Dropping a Column](dropping-column.md) | `ALTER TABLE ... DROP COLUMN` | ACCESS EXCLUSIVE |
| [Dropping a Constraint](dropping-constraint.md) | Unnamed `UNIQUE`/`FOREIGN KEY`/`CHECK` constraints | — (best practice) |
| [Dropping a Database](dropping-database.md) | `DROP DATABASE` | Exclusive access |
| [Dropping an Index](dropping-index.md) | `DROP INDEX` without `CONCURRENTLY` | ACCESS EXCLUSIVE |
| [Dropping a Primary Key](dropping-primary-key.md) | `ALTER TABLE ... DROP CONSTRAINT ... pkey` | ACCESS EXCLUSIVE |
| [Dropping a Table](dropping-table.md) | `DROP TABLE` | ACCESS EXCLUSIVE |
| [Generated Columns](generated-column.md) | `ADD COLUMN ... GENERATED ALWAYS AS ... STORED` | ACCESS EXCLUSIVE + table rewrite |
| [JSON Fields](json-field.md) | `ADD COLUMN ... JSON` | — (best practice) |
| [Wide Indexes](multiple-column-index.md) | `CREATE INDEX` with 4+ columns | — (best practice) |
| [Renaming a Column](renaming-column.md) | `ALTER TABLE ... RENAME COLUMN` | ACCESS EXCLUSIVE |
| [Renaming a Table](renaming-table.md) | `ALTER TABLE ... RENAME TO` | ACCESS EXCLUSIVE |
| [REINDEX](reindexing.md) | `REINDEX` without `CONCURRENTLY` | ACCESS EXCLUSIVE |
| [SERIAL Primary Keys](serial-primary-key.md) | `ADD COLUMN ... SERIAL/BIGSERIAL` | ACCESS EXCLUSIVE + table rewrite |
| [SET NOT NULL](set-not-null.md) | `ALTER TABLE ... ALTER COLUMN ... SET NOT NULL` | ACCESS EXCLUSIVE |
| [Short Primary Keys](short-primary-key.md) | `SMALLINT`/`INT` primary keys | — (best practice) |
| [TIMESTAMP Fields](timestamp-field.md) | `TIMESTAMP` without time zone | — (best practice) |
| [TRUNCATE TABLE](truncating-table.md) | `TRUNCATE TABLE` | ACCESS EXCLUSIVE |
| [Unnamed Constraints](unnamed-constraint.md) | Constraints without explicit names | — (best practice) |

Need project-specific rules beyond these? See [Custom Checks](../custom-checks.md).
