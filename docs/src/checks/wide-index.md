# Wide Indexes

**Check name:** `WideIndexCheck`

**Lock type:** None (best practice warning)

## Bad

Indexes with 4 or more columns are rarely effective. Postgres can only use multi-column indexes efficiently when filtering on the leftmost columns in order. Wide indexes also increase storage costs and slow down write operations (INSERT, UPDATE, DELETE).

```sql
-- 4+ columns: rarely useful
CREATE INDEX idx_users_search ON users(tenant_id, email, name, status);
CREATE INDEX idx_orders_composite ON orders(user_id, product_id, status, created_at);
```

## Good

Use narrower, more targeted indexes based on actual query patterns:

```sql
-- Option 1: Partial index for specific query pattern
CREATE INDEX idx_users_active_email ON users(email)
WHERE status = 'active';

-- Option 2: Separate indexes for different queries
CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_status ON users(status);

-- Option 3: Covering index with INCLUDE (Postgres 11+)
-- Includes extra columns for SELECT without adding them to index keys
CREATE INDEX idx_users_email_covering ON users(email)
INCLUDE (name, status);

-- Option 4: Two-column composite (still useful for some patterns)
CREATE INDEX idx_users_tenant_email ON users(tenant_id, email);
```

## When Wide Indexes Might Be Acceptable

- Composite foreign keys matching the referenced table's primary key
- Specific, verified query patterns that need all columns in order
- Use `safety-assured` if you've confirmed the index is necessary

**Performance tip:** Postgres can combine multiple indexes using bitmap scans. Two separate indexes often outperform one wide index.
