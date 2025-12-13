-- Unsafe: Adding JSON column can break SELECT DISTINCT queries
ALTER TABLE users ADD COLUMN properties JSON;
