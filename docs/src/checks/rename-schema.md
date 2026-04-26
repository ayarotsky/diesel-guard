# Renaming a Schema

**Check name:** `RenameSchemaCheck`

**Lock type:** ACCESS EXCLUSIVE (blocks on busy databases; breaks all references to schema objects)

## Bad

Renaming a schema breaks all application code, ORM models, and connection strings that reference any object within the schema. Unlike renaming a single table or column, a schema rename invalidates every qualified reference of the form `old_schema.table`, `old_schema.function`, `old_schema.type`, and so on — the blast radius is as wide as the schema itself.

```sql
ALTER SCHEMA myschema RENAME TO newschema;
```

## Good

Avoid renaming schemas in production. If a rename is unavoidable, use a `search_path` alias to maintain compatibility while migrating references:

```sql
-- Step 1: Add a search_path alias so both names resolve
ALTER DATABASE mydb SET search_path TO newschema, myschema;

-- Step 2: Rename the schema
ALTER SCHEMA myschema RENAME TO newschema;

-- Step 3: Update all application code, ORM models, and connection strings
-- to use the new schema name, then deploy.

-- Step 4: Remove the search_path alias after all references are updated
ALTER DATABASE mydb RESET search_path;
```

**Important:** This is a high-risk operation. Coordinate with all teams that own code referencing the schema before proceeding.
