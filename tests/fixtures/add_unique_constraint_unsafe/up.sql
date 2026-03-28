-- Unsafe: ADD UNIQUE constraint acquires ACCESS EXCLUSIVE lock
SET lock_timeout = '2s';
SET statement_timeout = '60s';

ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE (email);