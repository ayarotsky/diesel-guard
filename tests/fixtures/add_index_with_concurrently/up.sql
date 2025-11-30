-- Safe: Create index with CONCURRENTLY
CREATE INDEX CONCURRENTLY idx_users_email ON users(email);
