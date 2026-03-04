-- Unsafe: Add JSON column
ALTER TABLE users ADD COLUMN IF NOT EXISTS properties JSON;
