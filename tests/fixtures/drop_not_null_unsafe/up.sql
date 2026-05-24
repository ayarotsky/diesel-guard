-- Unsafe: Dropping NOT NULL constraint changes a contract that application code may depend on
SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER TABLE users ALTER COLUMN email DROP NOT NULL;
