-- Safe: Alter domain to add CHECK constraint with NOT VALID
ALTER DOMAIN positive_int ADD CONSTRAINT pos_check CHECK (VALUE > 0) NOT VALID;
