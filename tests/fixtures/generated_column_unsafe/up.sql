-- Unsafe: GENERATED STORED column triggers full table rewrite
SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER TABLE products ADD COLUMN total_price INTEGER GENERATED ALWAYS AS (price * quantity) STORED;