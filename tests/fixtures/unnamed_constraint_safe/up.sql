-- Safe: Properly named constraints (CHECK and FOREIGN KEY)

-- Named CHECK constraint (safe)
ALTER TABLE users ADD CONSTRAINT users_age_check CHECK (age >= 0);

-- Named FOREIGN KEY constraint (safe)
ALTER TABLE posts ADD CONSTRAINT posts_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id);
