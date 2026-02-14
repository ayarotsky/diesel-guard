-- Unsafe: Adding NOT NULL constraint requires full table scan and ACCESS EXCLUSIVE lock
ALTER TABLE users ALTER COLUMN email SET NOT NULL;
