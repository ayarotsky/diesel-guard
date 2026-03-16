-- Safe
-- Step 1 (no table scan, no lock)
ALTER TABLE orders ADD CONSTRAINT fk_user_id
    FOREIGN KEY (user_id) REFERENCES users(id) NOT VALID;

-- Step 2 (separate migration, acquires ShareUpdateExclusiveLock only)
ALTER TABLE orders VALIDATE CONSTRAINT fk_user_id;