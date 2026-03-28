-- Unsafe: REINDEX without CONCURRENTLY blocks all operations
SET lock_timeout = '2s';
SET statement_timeout = '60s';
REINDEX INDEX idx_users_email;