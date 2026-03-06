-- Unsafe: Generated column in ALTER TABLE (table rewrite)
ALTER TABLE products ADD COLUMN IF NOT EXISTS total_price INTEGER GENERATED ALWAYS AS (price * quantity) STORED;
