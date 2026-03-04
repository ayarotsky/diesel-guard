-- Unsafe: TIMESTAMP without timezone
ALTER TABLE events ADD COLUMN IF NOT EXISTS created_at TIMESTAMP;
