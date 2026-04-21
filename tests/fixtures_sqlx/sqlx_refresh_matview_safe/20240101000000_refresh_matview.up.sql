-- no-transaction
SET lock_timeout = '2s';
SET statement_timeout = '60s';
-- Safe: REFRESH MATERIALIZED VIEW CONCURRENTLY allows concurrent reads
REFRESH MATERIALIZED VIEW CONCURRENTLY my_view;