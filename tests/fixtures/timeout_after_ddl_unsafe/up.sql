-- Unsafe: timeout set AFTER DDL does not protect the DDL statement
ALTER TABLE users ADD COLUMN admin BOOLEAN;
SET lock_timeout = '2s';
SET statement_timeout = '60s';
