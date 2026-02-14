-- Safe: Dropping NOT NULL is a metadata-only change
ALTER TABLE users ALTER COLUMN email DROP NOT NULL;
