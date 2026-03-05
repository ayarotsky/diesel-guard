# SERIAL Primary Keys

**Check name:** `AddSerialColumnCheck`

**Lock type:** ACCESS EXCLUSIVE + full table rewrite

## Bad

Adding SERIAL columns to existing tables triggers a full table rewrite and
ACCESS EXCLUSIVE lock.

```sql
ALTER TABLE users ADD COLUMN id SERIAL;
ALTER TABLE users ADD COLUMN order_number BIGSERIAL;
```

## Good

For existing tables, create the sequence separately, add the column without a default, then backfill:

```sql
-- Step 1: Create a sequence
CREATE SEQUENCE users_id_seq;

-- Step 2: Add the column WITHOUT default (fast, no rewrite)
ALTER TABLE users ADD COLUMN id INTEGER;

-- Outside migration: Backfill existing rows in batches
UPDATE users SET id = nextval('users_id_seq') WHERE id IS NULL;

-- Step 3: Set default for future inserts only
ALTER TABLE users ALTER COLUMN id SET DEFAULT nextval('users_id_seq');

-- Step 4: Set NOT NULL if needed (Postgres 11+: safe if all values present)
ALTER TABLE users ALTER COLUMN id SET NOT NULL;

-- Step 5: Set sequence ownership
ALTER SEQUENCE users_id_seq OWNED BY users.id;
```

**Key insight:** For existing large tables, staged sequence/backfill migration avoids
the rewrite-heavy `ADD COLUMN ... SERIAL`.

For `CREATE TABLE ... SERIAL` guidance, see [Create Table with SERIAL](create-table-serial.md).
