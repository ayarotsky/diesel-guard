-- Unsafe: TIMESTAMP without time zone can cause timezone issues
ALTER TABLE events ADD COLUMN created_at TIMESTAMP;
