-- migrate:up
ALTER TABLE users DROP COLUMN email;

-- migrate:down
ALTER TABLE users ADD COLUMN email VARCHAR(255);
