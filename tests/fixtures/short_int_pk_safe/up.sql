-- Safe: BIGINT and BIGSERIAL primary keys
-- These avoid ID exhaustion

-- BIGINT primary key
CREATE TABLE IF NOT EXISTS users (
    id BIGINT PRIMARY KEY,
    name TEXT
);

-- BIGSERIAL primary key (auto-incrementing BIGINT)
CREATE TABLE IF NOT EXISTS posts (
    id BIGSERIAL PRIMARY KEY,
    title TEXT
);

-- Composite PK with all BIGINT columns
CREATE TABLE IF NOT EXISTS events (
    tenant_id BIGINT,
    event_id BIGINT,
    PRIMARY KEY (tenant_id, event_id)
);

-- INT is safe for non-PK columns
CREATE TABLE IF NOT EXISTS lookups (
    id BIGINT PRIMARY KEY,
    code INT UNIQUE,
    name TEXT
);
