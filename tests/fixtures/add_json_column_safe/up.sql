-- Safe: Using JSONB instead of JSON
ALTER TABLE users ADD COLUMN properties JSONB;
