-- Unsafe: GENERATED STORED column triggers full table rewrite
ALTER TABLE products ADD COLUMN total_price INTEGER GENERATED ALWAYS AS (price * quantity) STORED;
