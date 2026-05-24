-- Unsafe: Rename column breaks running instances
SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER TABLE users RENAME COLUMN email TO email_address;