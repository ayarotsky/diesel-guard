-- Unsafe: REFRESH MATERIALIZED VIEW without CONCURRENTLY blocks all reads
SET lock_timeout = '2s';
SET statement_timeout = '60s';
REFRESH MATERIALIZED VIEW my_view;