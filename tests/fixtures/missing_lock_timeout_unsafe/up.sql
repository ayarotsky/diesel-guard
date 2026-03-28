-- Unsafe: DDL without lock_timeout or statement_timeout
ALTER TABLE users ADD COLUMN admin BOOLEAN;
