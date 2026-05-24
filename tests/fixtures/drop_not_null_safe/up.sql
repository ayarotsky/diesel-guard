-- Safe: SET DEFAULT does not remove a NOT NULL constraint
SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER TABLE users ALTER COLUMN email SET DEFAULT 'noreply@example.com';
