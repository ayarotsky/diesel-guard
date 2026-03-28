-- Unsafe: Create unique index without CONCURRENTLY
SET lock_timeout = '2s';
SET statement_timeout = '60s';
CREATE UNIQUE INDEX idx_users_username ON users(username);