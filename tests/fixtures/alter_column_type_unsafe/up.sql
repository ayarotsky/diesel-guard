-- Unsafe: Alter column type
SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER TABLE users ALTER COLUMN age TYPE BIGINT;