-- Safe: Create unique index concurrently first, then add primary key using the index
CREATE UNIQUE INDEX CONCURRENTLY IF NOT EXISTS users_pkey ON users(id);
ALTER TABLE users ADD CONSTRAINT users_pkey PRIMARY KEY USING INDEX users_pkey;
