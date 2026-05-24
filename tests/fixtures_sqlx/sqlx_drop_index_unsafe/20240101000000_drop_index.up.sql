-- Unsafe: DROP INDEX without CONCURRENTLY blocks all operations
SET lock_timeout = '2s';
SET statement_timeout = '60s';
DROP INDEX idx_users_email;