-- no-transaction
SET lock_timeout = '2s';
SET statement_timeout = '60s';
-- Safe: DROP INDEX CONCURRENTLY allows concurrent operations
DROP INDEX CONCURRENTLY idx_users_email;