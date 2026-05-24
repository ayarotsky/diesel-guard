-- Unsafe: Wide index with 4 columns
SET lock_timeout = '2s';
SET statement_timeout = '60s';
CREATE INDEX CONCURRENTLY idx_users_composite ON users(tenant_id, user_id, email, status);