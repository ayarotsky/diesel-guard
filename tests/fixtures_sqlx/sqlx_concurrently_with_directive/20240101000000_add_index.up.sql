-- no-transaction

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_users_email ON users(email);
