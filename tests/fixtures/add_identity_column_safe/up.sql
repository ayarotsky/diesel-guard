-- Safe: Add column with sequence separately
SET lock_timeout = '2s';
SET statement_timeout = '60s';
CREATE SEQUENCE users_id_seq;
ALTER TABLE users ADD COLUMN id BIGINT;
ALTER TABLE users ALTER COLUMN id SET DEFAULT nextval('users_id_seq');
ALTER SEQUENCE users_id_seq OWNED BY users.id;
