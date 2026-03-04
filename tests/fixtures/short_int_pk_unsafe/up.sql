-- Unsafe: Short integer types in primary keys
-- These risk ID exhaustion

-- INT exhausts at ~2.1 billion records
CREATE TABLE IF NOT EXISTS users (
    id INT PRIMARY KEY,
    name TEXT
);

-- SMALLINT exhausts at ~32,767 records
CREATE TABLE IF NOT EXISTS posts (
    id SMALLINT PRIMARY KEY,
    title TEXT
);

-- INT in composite PK still risks exhaustion per partition
CREATE TABLE IF NOT EXISTS events (
    tenant_id BIGINT,
    event_id INT,
    PRIMARY KEY (tenant_id, event_id)
);

-- ALTER TABLE with ADD CONSTRAINT PRIMARY KEY
CREATE TABLE IF NOT EXISTS products (
    name TEXT
);

ALTER TABLE products
    -- Intentionally non-idempotent in this unsafe fixture to keep ADD PRIMARY KEY
    -- behavior covered alongside short integer PK detection.
    ADD COLUMN IF NOT EXISTS id INT,
    ADD CONSTRAINT pk_products PRIMARY KEY (id);
