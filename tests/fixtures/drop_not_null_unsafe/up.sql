-- Unsafe: Dropping NOT NULL constraint changes a contract that application code may depend on
ALTER TABLE users ALTER COLUMN email DROP NOT NULL;
