-- Unsafe: Create index without CONCURRENTLY
SET lock_timeout = '2s';
SET statement_timeout = '60s';
CREATE INDEX idx_users_email ON users(email);