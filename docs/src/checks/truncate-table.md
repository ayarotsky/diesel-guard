# TRUNCATE TABLE

**Check name:** `TruncateTableCheck`

**Lock type:** ACCESS EXCLUSIVE (blocks all operations)

## When this fires

`TRUNCATE TABLE` acquires an ACCESS EXCLUSIVE lock, blocking all reads and writes on the table. Unlike `DELETE`, it cannot be batched or throttled, making it risky on large tables in production.

```sql
TRUNCATE TABLE users;
TRUNCATE TABLE orders, order_items;
```

## When TRUNCATE is actually fine

TRUNCATE is a legitimate operation in many migration contexts:

- **Lookup / seed tables** — small, fast to re-populate, often truncated before re-seeding
- **Staging or test environments** — no live traffic, large-table risk doesn't apply
- **Known-empty tables** — e.g. clearing a table that was just created in the same migration
- **Maintenance windows** — explicit downtime where locking is acceptable

In these cases, the check produces noise. Use one of the escape hatches below.

## Good alternative (large production tables)

Use batched `DELETE` to remove rows incrementally while allowing concurrent access:

```sql
-- Delete rows in small batches to allow concurrent access
DELETE FROM users WHERE id IN (
  SELECT id FROM users LIMIT 1000
);

-- Repeat the batched DELETE until all rows are removed

-- Optional: Reset sequences if needed
ALTER SEQUENCE users_id_seq RESTART WITH 1;

-- Optional: Reclaim space
VACUUM users;
```

## Escape hatches

### Per-statement: safety-assured block

Use this when TRUNCATE is intentional in a specific migration:

```sql
-- safety-assured:start
-- Safe because: lookup table, always fewer than 100 rows
TRUNCATE TABLE countries;
-- safety-assured:end
```

### Project-wide: downgrade to a warning

Report TRUNCATE but don't fail CI. Useful when your project uses TRUNCATE routinely (e.g. seeding migrations) but you still want visibility:

```toml
# diesel-guard.toml
warn_checks = ["TruncateTableCheck"]
```

Warnings appear in output with ⚠️ and do **not** cause a non-zero exit code.

### Project-wide: disable entirely

```toml
# diesel-guard.toml
disable_checks = ["TruncateTableCheck"]
```
