-- Safe: Add unique constraint via CONCURRENTLY index then USING INDEX
CREATE UNIQUE INDEX CONCURRENTLY users_email_idx ON users(email);

-- Step 2 (Optional): Add constraint using the existing index
-- This is instant since the index already exists
ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE USING INDEX users_email_idx;
