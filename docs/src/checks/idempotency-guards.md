# Idempotency Guards

**Check names:** `IdempotencyCreateCheck`, `IdempotencyIndexCheck`, `IdempotencyAlterCheck`, `IdempotencyDropCheck`

**Lock type:** — (retry safety)

## Bad

Statements without idempotency guards can fail when a migration is retried after partial success.

```sql
CREATE TABLE users (id BIGINT PRIMARY KEY);
CREATE INDEX CONCURRENTLY users_email_idx ON users(email);
ALTER TABLE users ADD COLUMN email TEXT;

DROP TABLE users;
DROP INDEX users_email_idx;
ALTER TABLE users DROP COLUMN email;
```

## Good

Add guards so statements can be re-run safely.

```sql
CREATE TABLE IF NOT EXISTS users (id BIGINT PRIMARY KEY);
CREATE INDEX CONCURRENTLY IF NOT EXISTS users_email_idx ON users(email);
ALTER TABLE users ADD COLUMN IF NOT EXISTS email TEXT;

DROP TABLE IF EXISTS users;
DROP INDEX IF EXISTS users_email_idx;
ALTER TABLE users DROP COLUMN IF EXISTS email;
```

## Covered Operations

- `CREATE TABLE` requires `IF NOT EXISTS`
- `CREATE INDEX` (including `CONCURRENTLY`) requires `IF NOT EXISTS`
- `ALTER TABLE ... ADD COLUMN` requires `IF NOT EXISTS`
- `DROP TABLE` requires `IF EXISTS`
- `DROP INDEX` requires `IF EXISTS`
- `ALTER TABLE ... DROP COLUMN` requires `IF EXISTS`
