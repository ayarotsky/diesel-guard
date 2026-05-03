-- Unsafe: REINDEX CONCURRENTLY inside a transaction
SET lock_timeout = '2s';
SET statement_timeout = '60s';
REINDEX INDEX CONCURRENTLY idx_users_email;