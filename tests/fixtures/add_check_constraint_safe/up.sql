-- Safe
-- Step 1 (no table scans; lock acquired momentarily)
ALTER TABLE orders ADD CONSTRAINT check_amount CHECK (amount > 0) NOT VALID;

-- Step 2 (separate migration, acquires ShareUpdateExclusiveLock only)
ALTER TABLE orders VALIDATE CONSTRAINT check_amount;