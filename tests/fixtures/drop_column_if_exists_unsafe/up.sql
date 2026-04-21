-- Unsafe: Drop column with IF EXISTS
SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER TABLE users DROP COLUMN IF EXISTS old_column;