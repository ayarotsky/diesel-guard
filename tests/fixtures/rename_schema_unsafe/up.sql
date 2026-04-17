-- Unsafe: Rename schema breaks all references to objects within it
SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER SCHEMA myschema RENAME TO newschema;