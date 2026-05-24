-- Unsafe: Drop database permanently deletes entire database
SET lock_timeout = '2s';
SET statement_timeout = '60s';
DROP DATABASE mydb;
