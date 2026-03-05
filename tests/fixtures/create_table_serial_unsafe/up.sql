-- Unsafe: SERIAL pseudo-type in CREATE TABLE
CREATE TABLE events (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL
);
