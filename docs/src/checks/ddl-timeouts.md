# DDL Timeouts

**Check name:** `DdlTimeoutCheck`

**Lock type:** Applies before DDL that can wait on locks or run for a long time

## Bad

Running DDL without timeouts can leave a migration waiting indefinitely for a lock or a long-running statement. While it waits, later application queries can queue behind the migration and amplify the outage.

```sql
ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;
```

## Good

Set both `lock_timeout` and `statement_timeout` before DDL:

```sql
SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;
```

`lock_timeout` makes the migration fail fast when it cannot acquire a lock. `statement_timeout` bounds the overall statement runtime.

## Covered DDL

This check is intentionally broad. It applies to schema-changing PostgreSQL DDL, including table, type, enum, domain, sequence, index, view, materialized view, trigger, rule, policy, function, extension, schema, database, foreign data wrapper, foreign table, user mapping, operator class/family, statistics, tablespace, publication, subscription, role, privilege, cast, conversion, transform, collation, text search, comment, security label, `TRUNCATE`, `REINDEX`, and `REFRESH MATERIALIZED VIEW` statements.

`SET LOCAL` also satisfies this check for transaction-wrapped migrations:

```sql
SET LOCAL lock_timeout = '2s';
SET LOCAL statement_timeout = '60s';
ALTER TABLE users ADD COLUMN admin BOOLEAN;
```

Only non-disabled timeout values count. Values such as `0`, `'0'`, and `'0ms'` disable PostgreSQL timeouts and will still be reported. `RESET`, `RESET ALL`, and `SET ... DEFAULT` also clear timeout state for later DDL.

The timeout parser rejects common disabled zero values, but it does not evaluate every PostgreSQL interval expression. Prefer simple, explicit nonzero values such as `'2s'` and `'60s'`.

Teams that configure these timeouts at the connection or role level can disable this check in `diesel-guard.toml`.
