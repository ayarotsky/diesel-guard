# SERIAL Primary Keys

**Check name:** `AddSerialColumnCheck`

**Lock type:** ACCESS EXCLUSIVE + full table rewrite

## Bad

Adding a SERIAL column to an existing table triggers a full table rewrite because Postgres must populate sequence values for all existing rows. This acquires an ACCESS EXCLUSIVE lock and blocks all operations.

```sql
ALTER TABLE users ADD COLUMN id SERIAL;
ALTER TABLE users ADD COLUMN order_number BIGSERIAL;
```

## Good

Create the sequence separately, add the column without a default, then backfill:

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

**Key insight:** Adding a column with `DEFAULT nextval(...)` on an existing table still triggers a table rewrite. The solution is to add the column first without any default, backfill separately, then set the default for future rows only.
