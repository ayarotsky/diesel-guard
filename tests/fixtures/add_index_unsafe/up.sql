-- Unsafe: Create index without CONCURRENTLY
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
