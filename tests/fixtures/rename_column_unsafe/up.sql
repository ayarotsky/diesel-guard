-- Unsafe: Rename column breaks running instances
ALTER TABLE users RENAME COLUMN email TO email_address;
