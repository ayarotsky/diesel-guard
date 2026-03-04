-- Safe: Add sequence and column separately without SERIAL
CREATE SEQUENCE IF NOT EXISTS users_id_seq;
ALTER TABLE users ADD COLUMN IF NOT EXISTS id INTEGER;
ALTER TABLE users ALTER COLUMN id SET DEFAULT nextval('users_id_seq');
