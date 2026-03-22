-- Unsafe
ALTER TABLE orders ADD CONSTRAINT check_amount CHECK (amount > 0);
