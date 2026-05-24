-- Unsafe: Add SERIAL column to existing table
SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER TABLE users ADD COLUMN id SERIAL;