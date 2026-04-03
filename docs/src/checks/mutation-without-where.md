# Mutation without WHERE

**Check name:** `MutationWithoutWhereCheck`

**Lock type:** ROW EXCLUSIVE (DELETE and UPDATE)

## When this fires

A `DELETE FROM table` or `UPDATE table SET ...` with no WHERE clause affects every row in the table. In a migration this is almost always a mistake — a forgotten filter rather than an intentional bulk mutation.

```sql
-- Fires on DELETE with no WHERE
DELETE FROM users;

-- Fires on UPDATE with no WHERE
UPDATE users SET active = false;
```

## Why it's dangerous

Both hold a ROW EXCLUSIVE lock and touch every row in the table. On large tables this can take minutes and queue up other transactions. UPDATE also rewrites every row, causing table bloat.

## Good alternative

Add a WHERE clause to target only the rows you intend to modify:

```sql
-- Targeted delete
DELETE FROM users WHERE deactivated_at < '2020-01-01';

-- Targeted update
UPDATE users SET active = false WHERE last_login < '2020-01-01';
```

If a backfill must touch every row, do it in batches outside the migration to avoid holding the lock for extended periods.

## Escape hatch

When the full-table mutation is intentional (e.g. a one-time seed reset or known-empty table), use a safety-assured block:

```sql
-- safety-assured:start
-- Safe because: lookup table, always fewer than 100 rows
DELETE FROM countries;
-- safety-assured:end
```

```sql
-- safety-assured:start
-- Safe because: one-time backfill on a table with fewer than 5k rows
UPDATE feature_flags SET enabled = false;
-- safety-assured:end
```
