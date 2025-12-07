-- Example of properly named constraints in a migration

-- Named UNIQUE constraint
ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE (email);

-- Named CHECK constraint
ALTER TABLE users ADD CONSTRAINT users_age_check CHECK (age >= 0);

-- Named FOREIGN KEY constraint
ALTER TABLE posts ADD CONSTRAINT posts_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id);
