-- Unsafe: Drop index without CONCURRENTLY
SET lock_timeout = '2s';
SET statement_timeout = '60s';
DROP INDEX idx_users_email;