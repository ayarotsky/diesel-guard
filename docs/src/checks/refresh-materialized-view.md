# REFRESH MATERIALIZED VIEW

**Check name:** `RefreshMatViewCheck`

**Lock type:** ACCESS EXCLUSIVE (blocks all reads on the view)

## Bad

Refreshing a materialized view without CONCURRENTLY acquires an ACCESS EXCLUSIVE on the view, blocking all reads (SELECT) until the refresh completes. Duration depends on view complexity and underlying data size.

```sql
REFRESH MATERIALIZED VIEW my_view;
```

## Good

Use CONCURRENTLY to refresh the view without blocking reads:

```sql
REFRESH MATERIALIZED VIEW CONCURRENTLY my_view;
```

**Important:** CONCURRENTLY cannot run inside a transaction block, and requires a unique index on the materialized view.

Create the unique index before using the concurrent refresh:

```sql
CREATE UNIQUE INDEX ON my_view(id);
```

**For Diesel migrations:** Add a `metadata.toml` file to your migration directory:

```toml
# migrations/2024_01_01_refresh_my_view/metadata.toml
run_in_transaction = false
```

**For SQLx migrations:** Add the no-transaction directive at the top of your migration file:

```sql
-- no-transaction
REFRESH MATERIALIZED VIEW CONCURRENTLY my_view;
```

**Note:** If CONCURRENTLY fails, the view data remains unchanged — there is no partial update. Check that a unique index exists on the view before using this option.
