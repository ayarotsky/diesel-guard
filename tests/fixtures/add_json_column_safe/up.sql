-- Safe: Using JSONB instead of JSON
SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER TABLE users ADD COLUMN properties JSONB;