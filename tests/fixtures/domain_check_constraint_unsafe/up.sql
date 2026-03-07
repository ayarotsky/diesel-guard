-- Unsafe: Domain CHECK constraints are global and cannot be rolled out with NOT VALID

CREATE DOMAIN email AS text CHECK (VALUE ~* '^[^@]+@[^@]+$');
ALTER DOMAIN email ADD CONSTRAINT email_has_at_check CHECK (VALUE ~* '^[^@]+@[^@]+$');
