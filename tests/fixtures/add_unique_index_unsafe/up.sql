-- Unsafe: Create unique index without CONCURRENTLY
CREATE UNIQUE INDEX idx_users_username ON users(username);
