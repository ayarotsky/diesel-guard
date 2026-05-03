-- Unsafe: Adding JSON column can break SELECT DISTINCT queries
SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER TABLE users ADD COLUMN properties JSON;