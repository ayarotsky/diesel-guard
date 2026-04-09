-- Unsafe: Statements without idempotency guards can fail on retry
CREATE TABLE users (
    id BIGINT PRIMARY KEY
);

CREATE INDEX CONCURRENTLY users_id_idx ON users(id);
ALTER TABLE users ADD COLUMN email TEXT;
