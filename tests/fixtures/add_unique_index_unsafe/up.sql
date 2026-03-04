-- Unsafe: Create unique index without CONCURRENTLY
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_username ON users(username);
