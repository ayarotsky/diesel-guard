# Dropping a Primary Key

**Check name:** `DropPrimaryKeyCheck`

**Lock type:** ACCESS EXCLUSIVE (blocks all operations)

## Bad

Dropping a primary key removes the critical uniqueness constraint and breaks foreign key relationships in other tables that reference this table. It also acquires an ACCESS EXCLUSIVE lock, blocking all operations.

```sql
-- Breaks foreign keys that reference users(id)
ALTER TABLE users DROP CONSTRAINT users_pkey;
```

## Good

If you must change your primary key strategy, use a multi-step migration approach:

```sql
-- Step 1: Identify all foreign key dependencies
SELECT
  tc.table_name, kcu.column_name, rc.constraint_name
FROM information_schema.table_constraints tc
JOIN information_schema.key_column_usage kcu ON tc.constraint_name = kcu.constraint_name
JOIN information_schema.referential_constraints rc ON tc.constraint_name = rc.unique_constraint_name
WHERE tc.table_name = 'users' AND tc.constraint_type = 'PRIMARY KEY';

-- Step 2: Create the new primary key FIRST (if migrating to a new key)
ALTER TABLE users ADD CONSTRAINT users_new_pkey PRIMARY KEY (uuid);

-- Step 3: Update all foreign keys to reference the new key
-- (This may require adding new columns to referencing tables)
ALTER TABLE posts ADD COLUMN user_uuid UUID;
UPDATE posts SET user_uuid = users.uuid FROM users WHERE posts.user_id = users.id;
ALTER TABLE posts ADD CONSTRAINT posts_user_uuid_fkey FOREIGN KEY (user_uuid) REFERENCES users(uuid);

-- Step 4: Only after all foreign keys are migrated, drop the old key
ALTER TABLE users DROP CONSTRAINT users_pkey;

-- Step 5: Clean up old columns
ALTER TABLE posts DROP COLUMN user_id;
```

## Important Considerations

- Review ALL tables with foreign keys to this table
- Consider a transition period where both old and new keys exist
- Update application code to use the new key before dropping the old one
- Test thoroughly in a staging environment first

**Limitation:** This check relies on Postgres naming conventions (e.g., `users_pkey`). It may not detect primary keys with custom names. Future versions will support database connections for accurate verification.
