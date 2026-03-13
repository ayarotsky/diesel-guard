-- Safe: Domain operations without CHECK constraints

CREATE DOMAIN email AS text;
ALTER DOMAIN email SET DEFAULT '';
ALTER DOMAIN email DROP DEFAULT;
ALTER DOMAIN email SET NOT NULL;
