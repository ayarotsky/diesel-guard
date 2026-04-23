-- Safe: Statements are guarded for idempotent retries
CREATE TABLE IF NOT EXISTS users (
    id BIGINT PRIMARY KEY
);

CREATE INDEX CONCURRENTLY IF NOT EXISTS users_id_idx ON users(id);
ALTER TABLE users ADD COLUMN IF NOT EXISTS email TEXT;
DROP TABLE IF EXISTS archived_users;
DROP INDEX IF EXISTS users_legacy_idx;
ALTER TABLE users DROP COLUMN IF EXISTS legacy_email;
