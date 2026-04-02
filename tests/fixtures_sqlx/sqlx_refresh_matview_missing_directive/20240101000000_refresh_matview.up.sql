-- Unsafe: REFRESH MATERIALIZED VIEW CONCURRENTLY inside a transaction
SET lock_timeout = '2s';
SET statement_timeout = '60s';
REFRESH MATERIALIZED VIEW CONCURRENTLY my_view;