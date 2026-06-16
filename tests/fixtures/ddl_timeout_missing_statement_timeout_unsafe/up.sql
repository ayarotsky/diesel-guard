-- Unsafe: DDL runs with lock_timeout but no statement_timeout
SET lock_timeout = '2s';
ALTER TABLE users ADD COLUMN admin BOOLEAN;
