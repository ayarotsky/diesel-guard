-- no-transaction
SET lock_timeout = '2s';
SET statement_timeout = '60s';
-- Safe: REINDEX CONCURRENTLY allows concurrent operations
REINDEX INDEX CONCURRENTLY idx_users_email;