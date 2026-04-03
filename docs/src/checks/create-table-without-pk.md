# CREATE TABLE without PRIMARY KEY

**Operation:** `CREATE TABLE without PRIMARY KEY`

## Why this is dangerous

Tables without a primary key cannot participate in logical replication. PostgreSQL replication slots require each replicated table to have either a primary key or an explicit `REPLICA IDENTITY` setting (`FULL` or `USING INDEX`). Without one, any attempt to replicate the table will fail or require manual intervention.

Beyond replication, a primary key provides:
- A guaranteed unique row identifier for updates, deletes, and `ON CONFLICT` clauses
- A natural target for foreign key references from other tables
- An implicit index that speeds up single-row lookups

## What diesel-guard checks

Any `CREATE TABLE` statement that defines no primary key — neither inline on a column nor as a separate table-level constraint — is flagged.

Exceptions (not flagged):
- `CREATE TEMP TABLE` — temporary tables are session-scoped and never replicated
- `CREATE TABLE (LIKE other)` — PK inheritance depends on `INCLUDING CONSTRAINTS`, which cannot be determined at parse time

## Bad

```sql
CREATE TABLE events (
  name    TEXT,
  payload JSONB
);
```

## Good

```sql
-- Option 1: identity column (recommended)
CREATE TABLE events (
  id      BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  name    TEXT,
  payload JSONB
);

-- Option 2: UUID
CREATE TABLE events (
  id      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  name    TEXT,
  payload JSONB
);

-- Option 3: separate constraint
CREATE TABLE events (
  id      BIGINT GENERATED ALWAYS AS IDENTITY,
  name    TEXT,
  payload JSONB,
  PRIMARY KEY (id)
);
```

## Escape hatch

If the table is intentionally without a primary key (for example, a log table where you plan to set `REPLICA IDENTITY FULL`), wrap the statement in a safety-assured block:

```sql
-- safety-assured:start
CREATE TABLE audit_log (
  recorded_at TIMESTAMPTZ,
  message     TEXT
);
-- safety-assured:end
```
