-- Safe: Properly named constraints (CHECK and FOREIGN KEY)
SET lock_timeout = '2s';
SET statement_timeout = '60s';

-- Named CHECK constraint (safe)
ALTER TABLE users ADD CONSTRAINT users_age_check CHECK (age >= 0) NOT VALID;

-- Named FOREIGN KEY constraint (safe)
ALTER TABLE posts ADD CONSTRAINT posts_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) NOT VALID;