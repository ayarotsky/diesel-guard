-- Unsafe: CHAR type wastes storage and causes comparison issues
SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER TABLE users ADD COLUMN country_code CHAR(2);