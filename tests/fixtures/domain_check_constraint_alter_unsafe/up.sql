-- Unsafe: Alter domain to add CHECK constraint
ALTER DOMAIN positive_int ADD CONSTRAINT pos_check CHECK (VALUE > 0);
