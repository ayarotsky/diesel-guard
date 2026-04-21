-- Unsafe: CREATE EXTENSION can acquire exclusive locks
SET lock_timeout = '2s';
SET statement_timeout = '60s';
CREATE EXTENSION IF NOT EXISTS pg_trgm;