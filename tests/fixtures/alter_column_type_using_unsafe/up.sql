-- Unsafe: Alter column type with USING clause
SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER TABLE users ALTER COLUMN data TYPE JSONB USING data::JSONB;