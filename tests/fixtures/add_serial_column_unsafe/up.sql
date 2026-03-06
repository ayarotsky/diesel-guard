-- Unsafe: Add SERIAL column directly
ALTER TABLE users ADD COLUMN IF NOT EXISTS id SERIAL;
