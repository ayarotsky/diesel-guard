-- Unsafe
SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER TABLE orders ADD CONSTRAINT check_amount CHECK (amount > 0);