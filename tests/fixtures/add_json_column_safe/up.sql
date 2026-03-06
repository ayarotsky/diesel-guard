-- Safe: Add JSONB column
ALTER TABLE users ADD COLUMN IF NOT EXISTS properties JSONB;
