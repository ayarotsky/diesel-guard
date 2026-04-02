-- Safe: Dropping NOT NULL is a metadata-only change
SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER TABLE users ALTER COLUMN email DROP NOT NULL;