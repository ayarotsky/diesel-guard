-- Unsafe: Rename table breaks running instances
SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER TABLE users RENAME TO customers;