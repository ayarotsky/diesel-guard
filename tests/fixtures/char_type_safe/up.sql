-- Safe: Use VARCHAR instead of CHAR
ALTER TABLE users ADD COLUMN IF NOT EXISTS country_code VARCHAR(2);
