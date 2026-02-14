-- Safe: Drop column wrapped in safety-assured block
-- safety-assured:start
ALTER TABLE users DROP COLUMN deprecated_field;
-- safety-assured:end
