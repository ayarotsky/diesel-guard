-- Safe: Create index with CONCURRENTLY
SET lock_timeout = '2s';
SET statement_timeout = '60s';
CREATE INDEX CONCURRENTLY idx_users_email ON users(email);