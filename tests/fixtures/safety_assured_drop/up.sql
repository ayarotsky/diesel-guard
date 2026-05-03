-- Safe: Drop column wrapped in safety-assured block
SET lock_timeout = '2s';
SET statement_timeout = '60s';
-- safety-assured:start
ALTER TABLE users DROP COLUMN deprecated_field;
-- safety-assured:end