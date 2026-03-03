# Generated Columns

**Check name:** `GeneratedColumnCheck`

**Lock type:** ACCESS EXCLUSIVE + full table rewrite

## Bad

Adding a `GENERATED ALWAYS AS ... STORED` column acquires an ACCESS EXCLUSIVE lock and triggers a full table rewrite because Postgres must compute and store the expression value for every existing row.

```sql
ALTER TABLE products ADD COLUMN total_price INTEGER GENERATED ALWAYS AS (price * quantity) STORED;
```

## Good

```sql
-- Step 1: Add a regular nullable column
ALTER TABLE products ADD COLUMN total_price INTEGER;

-- Step 2: Backfill in batches (outside migration)
UPDATE products SET total_price = price * quantity WHERE total_price IS NULL;

-- Step 3: Optionally add NOT NULL constraint
ALTER TABLE products ALTER COLUMN total_price SET NOT NULL;

-- Step 4: Use a trigger for new rows
CREATE FUNCTION compute_total_price() RETURNS TRIGGER AS $$
BEGIN
  NEW.total_price := NEW.price * NEW.quantity;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_total_price
BEFORE INSERT OR UPDATE ON products
FOR EACH ROW EXECUTE FUNCTION compute_total_price();
```

**Note:** Postgres does not support VIRTUAL generated columns (only STORED). For new empty tables, `GENERATED STORED` columns are acceptable.
