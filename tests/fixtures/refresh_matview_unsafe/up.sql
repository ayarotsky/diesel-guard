-- Unsafe: Refresh materialized view without CONCURRENTLY
SET lock_timeout = '2s';
SET statement_timeout = '60s';
REFRESH MATERIALIZED VIEW my_view;