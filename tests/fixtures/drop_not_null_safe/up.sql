-- Safe: SET DEFAULT does not remove a NOT NULL constraint
ALTER TABLE users ALTER COLUMN email SET DEFAULT 'noreply@example.com';
