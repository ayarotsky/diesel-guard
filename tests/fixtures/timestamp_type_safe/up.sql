-- Safe: Using TIMESTAMPTZ instead of TIMESTAMP
SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER TABLE events ADD COLUMN created_at TIMESTAMPTZ;