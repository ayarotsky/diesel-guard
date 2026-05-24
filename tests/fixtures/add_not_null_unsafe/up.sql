-- Unsafe: Adding NOT NULL constraint requires full table scan and ACCESS EXCLUSIVE lock
SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER TABLE users ALTER COLUMN email SET NOT NULL;