-- Unsafe: REINDEX without CONCURRENTLY blocks all operations
REINDEX INDEX idx_users_email;
