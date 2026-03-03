# JSON Fields

**Check name:** `AddJsonColumnCheck`

**Lock type:** None (best practice warning)

## Bad

In Postgres, the `json` type has no equality operator, which breaks existing `SELECT DISTINCT` queries and other operations that require comparing values.

```sql
ALTER TABLE users ADD COLUMN properties JSON;
```

## Good

Use `jsonb` instead of `json`:

```sql
ALTER TABLE users ADD COLUMN properties JSONB;
```

## Benefits of JSONB over JSON

- Has proper equality and comparison operators (supports DISTINCT, GROUP BY, UNION)
- Supports indexing (GIN indexes for efficient queries)
- Faster to process (binary format, no reparsing)
- Generally better performance for most use cases

**Note:** The only advantage of JSON over JSONB is that it preserves exact formatting and key order, which is rarely needed in practice.
