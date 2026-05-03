-- Safe: REINDEX CONCURRENTLY allows concurrent operations
SET lock_timeout = '2s';
SET statement_timeout = '60s';
REINDEX INDEX CONCURRENTLY idx_users_email;