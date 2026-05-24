-- Unsafe: Drop table permanently deletes all data
SET lock_timeout = '2s';
SET statement_timeout = '60s';
DROP TABLE users;