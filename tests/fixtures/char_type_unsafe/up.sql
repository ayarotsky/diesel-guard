-- Unsafe: CHAR type causes padding issues
ALTER TABLE users ADD COLUMN IF NOT EXISTS country_code CHAR(2);
