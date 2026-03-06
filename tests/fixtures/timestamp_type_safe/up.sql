-- Safe: TIMESTAMPTZ with timezone awareness
ALTER TABLE events ADD COLUMN IF NOT EXISTS created_at TIMESTAMPTZ;
