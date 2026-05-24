-- Unsafe: Short integer types in primary keys
SET lock_timeout = '2s';
SET statement_timeout = '60s';
-- These risk ID exhaustion

-- INT exhausts at ~2.1 billion records
CREATE TABLE users (
    id INT PRIMARY KEY,
    name TEXT
);

-- SMALLINT exhausts at ~32,767 records
CREATE TABLE posts (
    id SMALLINT PRIMARY KEY,
    title TEXT
);

-- INT in composite PK still risks exhaustion per partition
CREATE TABLE events (
    tenant_id BIGINT,
    event_id INT,
    PRIMARY KEY (tenant_id, event_id)
);

-- ALTER TABLE with ADD CONSTRAINT PRIMARY KEY
CREATE TABLE products (
    name TEXT
);

ALTER TABLE products
    ADD COLUMN id INT,
    ADD CONSTRAINT pk_products PRIMARY KEY (id);