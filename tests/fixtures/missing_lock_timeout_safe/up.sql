-- Safe: DDL with lock_timeout and statement_timeout configured
SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER TABLE users ADD COLUMN admin BOOLEAN;
