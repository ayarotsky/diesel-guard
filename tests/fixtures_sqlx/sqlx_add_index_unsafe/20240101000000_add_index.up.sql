-- Unsafe: CREATE INDEX without CONCURRENTLY blocks writes
CREATE INDEX idx_users_email ON users(email);
