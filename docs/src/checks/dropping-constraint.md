# Dropping a Constraint (Unnamed Constraints)

**Check name:** `UnnamedConstraintCheck`

**Lock type:** None (best practice warning)

This check flags constraints added without an explicit name. Auto-generated names vary between databases and make future `DROP CONSTRAINT` operations unpredictable.

## Bad

Adding constraints without explicit names results in auto-generated names from Postgres. These names vary between databases and make future migrations difficult.

```sql
-- Unnamed UNIQUE constraint
ALTER TABLE users ADD UNIQUE (email);

-- Unnamed FOREIGN KEY constraint
ALTER TABLE posts ADD FOREIGN KEY (user_id) REFERENCES users(id);

-- Unnamed CHECK constraint
ALTER TABLE users ADD CHECK (age >= 0);
```

## Good

Always name constraints explicitly using the CONSTRAINT keyword:

```sql
-- Named UNIQUE constraint
ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE (email);

-- Named FOREIGN KEY constraint
ALTER TABLE posts ADD CONSTRAINT posts_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id);

-- Named CHECK constraint
ALTER TABLE users ADD CONSTRAINT users_age_check CHECK (age >= 0);
```

## Naming Conventions

- **UNIQUE**: `{table}_{column}_key` or `{table}_{column1}_{column2}_key`
- **FOREIGN KEY**: `{table}_{column}_fkey`
- **CHECK**: `{table}_{column}_check` or `{table}_{description}_check`

Named constraints make future migrations predictable:

```sql
-- Easy to reference in later migrations
ALTER TABLE users DROP CONSTRAINT users_email_key;
```
