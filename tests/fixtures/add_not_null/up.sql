-- Adding NOT NULL constraint to existing column
-- This requires a full table scan to verify all values are non-null
-- Acquires ACCESS EXCLUSIVE lock, blocking all operations
ALTER TABLE users ALTER COLUMN email SET NOT NULL;
