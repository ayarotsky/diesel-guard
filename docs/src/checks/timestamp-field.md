# TIMESTAMP Fields

**Check name:** `TimestampTypeCheck`

**Lock type:** None (best practice warning)

## Bad

TIMESTAMP (or TIMESTAMP WITHOUT TIME ZONE) stores values without timezone context, which can cause issues in multi-timezone applications, during DST transitions, and makes it difficult to determine the actual point in time represented.

```sql
-- ALTER TABLE
ALTER TABLE events ADD COLUMN created_at TIMESTAMP;
ALTER TABLE events ADD COLUMN updated_at TIMESTAMP WITHOUT TIME ZONE;

-- CREATE TABLE
CREATE TABLE events (
    id SERIAL PRIMARY KEY,
    created_at TIMESTAMP,
    updated_at TIMESTAMP WITHOUT TIME ZONE
);
```

## Good

Use TIMESTAMPTZ (TIMESTAMP WITH TIME ZONE) instead:

```sql
-- ALTER TABLE
ALTER TABLE events ADD COLUMN created_at TIMESTAMPTZ;
ALTER TABLE events ADD COLUMN updated_at TIMESTAMP WITH TIME ZONE;

-- CREATE TABLE
CREATE TABLE events (
    id SERIAL PRIMARY KEY,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMP WITH TIME ZONE
);
```

## Why TIMESTAMPTZ Is Better

- Stores values in UTC internally and converts on input/output based on session timezone
- Provides consistent behavior across different timezones and server environments
- Handles DST transitions correctly
- Makes it clear what point in time is represented

## When TIMESTAMP Without Time Zone Might Be Acceptable

- Storing dates that are inherently timezone-agnostic (e.g., birth dates stored as midnight)
- Legacy systems where all data is known to be in a single timezone
- Use `safety-assured` if you've confirmed timezone-naive timestamps are appropriate
