# Short Primary Keys

**Check name:** `ShortIntegerPrimaryKeyCheck`

**Lock type:** None (best practice warning)

## Bad

Using SMALLINT or INT for primary keys risks ID exhaustion. SMALLINT maxes out at ~32,767 records, and INT at ~2.1 billion. While 2.1 billion seems large, active applications can exhaust this faster than expected, especially with high-frequency inserts, soft deletes, or partitioned data.

Changing the type later requires an ALTER COLUMN TYPE operation with a full table rewrite and ACCESS EXCLUSIVE lock.

```sql
-- SMALLINT exhausts at ~32K records
CREATE TABLE users (id SMALLINT PRIMARY KEY);

-- INT exhausts at ~2.1B records
CREATE TABLE posts (id INT PRIMARY KEY);
CREATE TABLE events (id INTEGER PRIMARY KEY);

-- Composite PKs with short integers still risky
CREATE TABLE tenant_events (
    tenant_id BIGINT,
    event_id INT,  -- Will exhaust per tenant
    PRIMARY KEY (tenant_id, event_id)
);
```

## Good

Use BIGINT for all primary keys to avoid exhaustion:

```sql
-- BIGINT: effectively unlimited (~9.2 quintillion)
CREATE TABLE users (id BIGINT PRIMARY KEY);

-- BIGSERIAL: auto-incrementing BIGINT
CREATE TABLE posts (id BIGSERIAL PRIMARY KEY);

-- Composite PKs with all BIGINT
CREATE TABLE tenant_events (
    tenant_id BIGINT,
    event_id BIGINT,
    PRIMARY KEY (tenant_id, event_id)
);
```

**Storage overhead:** BIGINT uses 8 bytes vs INT's 4 bytes — only 4 extra bytes per row. For a 1 million row table, this is ~4MB of additional storage, which is negligible compared to the operational cost of changing column types later.

**Safe exceptions:** Small, finite lookup tables with <100 entries (e.g., status codes, country lists) can safely use smaller types. Use `safety-assured` to bypass the check for these cases.
