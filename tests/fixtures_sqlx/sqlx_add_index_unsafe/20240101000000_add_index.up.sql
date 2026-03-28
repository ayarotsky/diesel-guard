-- Unsafe: CREATE INDEX without CONCURRENTLY blocks writes
SET lock_timeout = '2s';
SET statement_timeout = '60s';
CREATE INDEX idx_users_email ON users(email);