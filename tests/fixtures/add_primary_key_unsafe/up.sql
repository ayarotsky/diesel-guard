-- Unsafe: Add primary key to existing table
SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER TABLE users ADD PRIMARY KEY (id);