-- Unsafe: Drop primary key constraint
SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER TABLE users DROP CONSTRAINT users_pkey;