-- Safe: Add column without default
ALTER TABLE users ADD COLUMN IF NOT EXISTS email VARCHAR(255);
