-- Unsafe: ADD UNIQUE constraint acquires ACCESS EXCLUSIVE lock

ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE (email);
