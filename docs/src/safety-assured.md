# Safety-Assured Blocks

When you've manually verified an operation is safe, use `safety-assured` comment blocks to bypass checks:

```sql
-- safety-assured:start
ALTER TABLE users DROP COLUMN deprecated_column;
ALTER TABLE posts DROP COLUMN old_field;
-- safety-assured:end
```

All statements between the start and end markers are skipped by all checks — both built-in and custom.

## Multiple Blocks

```sql
-- safety-assured:start
ALTER TABLE users DROP COLUMN email;
-- safety-assured:end

-- This will be checked normally
CREATE INDEX users_email_idx ON users(email);

-- safety-assured:start
ALTER TABLE posts DROP COLUMN body;
-- safety-assured:end
```

## When to Use Safety-Assured

**Only use when you've taken proper precautions:**

1. **For DROP COLUMN:**
   - Stopped reading/writing the column in application code
   - Deployed those changes to production
   - Verified no references remain in your codebase

2. **For other operations:**
   ```sql
   -- safety-assured:start
   -- Safe because: table is empty, deployed in maintenance window
   ALTER TABLE new_table ADD COLUMN status TEXT DEFAULT 'pending';
   -- safety-assured:end
   ```

## Error Handling

diesel-guard will error if blocks are mismatched:

```
Error: Unclosed 'safety-assured:start' at line 1
```

Rules:
- Blocks cannot be nested — a second `start` before `end` is an error
- Unclosed blocks are errors
- Unmatched `end` directives (without a preceding `start`) are errors
- The directives are case-insensitive
