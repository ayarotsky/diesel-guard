# Adding an EXCLUDE Constraint

**Check Name**: `AddExcludeConstraintCheck`

**Lock Type**: SHARE ROW EXCLUSIVE

## Bad

Adding an exclusion constraint scans the entire table to validate existing rows while holding a SHARE ROW EXCLUSIVE lock for the full duration. Unlike `CHECK` or `FOREIGN KEY` constraints, there is no `NOT VALID` escape hatch — exclusion constraints must be validated immediately.

```sql
ALTER TABLE meeting_rooms
    ADD CONSTRAINT no_double_booking
    EXCLUDE USING gist (room_id WITH =, during WITH &&);
```

## Good

There is no non-blocking path for adding an exclusion constraint to an existing populated table. Options:

- **Add during a low-traffic window** and accept the full-table scan cost.
- **Define the constraint at table creation time** to avoid scanning existing rows:

```sql
CREATE TABLE meeting_rooms (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    room_id INT NOT NULL,
    during TSTZRANGE NOT NULL,
    CONSTRAINT no_double_booking EXCLUDE USING gist (room_id WITH =, during WITH &&)
);
```

- **Use application-level enforcement** if the table is too large to lock safely during the migration.

## Escape Hatch

If you have reviewed this operation and confirmed it is safe (e.g., the table is empty or traffic is negligible), wrap it in a safety-assured block:

```sql
-- safety-assured:start
ALTER TABLE meeting_rooms
    ADD CONSTRAINT no_double_booking
    EXCLUDE USING gist (room_id WITH =, during WITH &&);
-- safety-assured:end
```
