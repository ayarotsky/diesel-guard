-- Safe: Create unique index concurrently first, then add primary key using the index
SET lock_timeout = '2s';
SET statement_timeout = '60s';
CREATE UNIQUE INDEX CONCURRENTLY users_pkey ON users(id);
ALTER TABLE users ADD CONSTRAINT users_pkey PRIMARY KEY USING INDEX users_pkey;