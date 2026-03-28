-- Safe: Multiple operations wrapped in safety-assured blocks
SET lock_timeout = '2s';
SET statement_timeout = '60s';
-- safety-assured:start
ALTER TABLE users DROP COLUMN email;
ALTER TABLE posts DROP COLUMN body;
-- safety-assured:end

-- safety-assured:start
CREATE INDEX users_name_idx ON users(name);
-- safety-assured:end