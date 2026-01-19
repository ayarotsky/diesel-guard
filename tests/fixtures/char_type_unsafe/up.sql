-- Unsafe: CHAR type wastes storage and causes comparison issues
ALTER TABLE users ADD COLUMN country_code CHAR(2);
