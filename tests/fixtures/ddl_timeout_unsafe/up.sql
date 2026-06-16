-- Unsafe: DDL runs without lock_timeout or statement_timeout
ALTER TABLE users ADD COLUMN admin BOOLEAN;
