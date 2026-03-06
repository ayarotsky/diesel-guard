-- Unsafe: Wide index with 4 columns
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_users_composite ON users(tenant_id, user_id, email, status);
