-- safety-assured:start
ALTER TABLE users DROP COLUMN email;
ALTER TABLE posts DROP COLUMN body;
-- safety-assured:end

-- safety-assured:start
CREATE INDEX users_name_idx ON users(name);
-- safety-assured:end
