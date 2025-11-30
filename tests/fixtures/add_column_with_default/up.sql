-- Unsafe: Add column with DEFAULT value
ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;
