-- Safe: Using TEXT or VARCHAR instead of CHAR
SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER TABLE users ADD COLUMN country_code VARCHAR(2);