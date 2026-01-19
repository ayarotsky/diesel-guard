-- Unsafe: Drop multiple columns in one statement
ALTER TABLE users DROP COLUMN email, DROP COLUMN phone;
