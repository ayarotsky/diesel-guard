-- Safe: Add column without DEFAULT
SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER TABLE users ADD COLUMN email VARCHAR(255);