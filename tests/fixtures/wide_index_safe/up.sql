-- Safe: Index with 3 columns
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_users_composite ON users(tenant_id, user_id, email);
