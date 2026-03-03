# CHAR Fields

**Check name:** `CharTypeCheck`

**Lock type:** None (best practice warning)

## Bad

CHAR and CHARACTER types are fixed-length and padded with spaces. This wastes storage and can cause subtle bugs with string comparisons and equality checks.

```sql
ALTER TABLE users ADD COLUMN country_code CHAR(2);
CREATE TABLE products (sku CHARACTER(10) PRIMARY KEY);
```

## Good

Use TEXT or VARCHAR instead:

```sql
-- For ALTER TABLE
ALTER TABLE users ADD COLUMN country_code TEXT;
ALTER TABLE users ADD COLUMN country_code VARCHAR(2);

-- For CREATE TABLE
CREATE TABLE products (sku TEXT);
CREATE TABLE products (sku VARCHAR(10));

-- Or TEXT with CHECK constraint for length validation
ALTER TABLE users ADD COLUMN country_code TEXT CHECK (length(country_code) = 2);
CREATE TABLE products (sku TEXT CHECK (length(sku) <= 10));
```

## Why CHAR Is Problematic

- Fixed-length padding wastes storage
- Trailing spaces affect equality comparisons (`'US' != 'US  '`)
- DISTINCT, GROUP BY, and joins may behave unexpectedly
- No performance benefit over VARCHAR or TEXT in Postgres
