-- Dropping NOT NULL constraint is safe
-- This is a metadata-only change in PostgreSQL
ALTER TABLE users ALTER COLUMN email DROP NOT NULL;
