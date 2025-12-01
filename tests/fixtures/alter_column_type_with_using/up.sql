-- Unsafe: Alter column type with USING clause
ALTER TABLE users ALTER COLUMN data TYPE JSONB USING data::JSONB;
