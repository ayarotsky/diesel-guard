-- Unsafe: Add column with default value (table rewrite on PG < 11)
ALTER TABLE users ADD COLUMN IF NOT EXISTS admin BOOLEAN DEFAULT FALSE;
