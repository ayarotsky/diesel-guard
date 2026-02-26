-- Unsafe: Unnamed constraints get auto-generated names from Postgres

-- Unnamed UNIQUE constraint
ALTER TABLE users ADD UNIQUE (email);

-- Unnamed CHECK constraint
ALTER TABLE users ADD CHECK (age >= 0);

-- Unnamed FOREIGN KEY constraint
ALTER TABLE posts ADD FOREIGN KEY (user_id) REFERENCES users(id);
