-- Safe: Create index with CONCURRENTLY and idempotency guard
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_users_email ON users(email);
