-- Unsafe: TIMESTAMP without time zone can cause timezone issues
SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER TABLE events ADD COLUMN created_at TIMESTAMP;