-- Safe: Using TEXT or VARCHAR instead of CHAR
ALTER TABLE users ADD COLUMN country_code VARCHAR(2);
