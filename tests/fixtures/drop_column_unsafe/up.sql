-- Unsafe: Drop column from table
SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER TABLE users DROP COLUMN email;