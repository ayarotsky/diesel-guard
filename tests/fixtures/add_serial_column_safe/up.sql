-- Safe: Add sequence and column separately without SERIAL
SET lock_timeout = '2s';
SET statement_timeout = '60s';
CREATE SEQUENCE users_id_seq;
ALTER TABLE users ADD COLUMN id INTEGER;
ALTER TABLE users ALTER COLUMN id SET DEFAULT nextval('users_id_seq');