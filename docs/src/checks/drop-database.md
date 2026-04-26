# Dropping a Database

**Check name:** `DropDatabaseCheck`

**Lock type:** Exclusive access (all connections must be terminated)

## Bad

Dropping a database permanently deletes the entire database including all tables, data, and objects. This operation is irreversible. Postgres requires exclusive access to the target database — all active connections must be terminated before the drop can proceed. The command cannot be executed inside a transaction block.

```sql
DROP DATABASE mydb;
DROP DATABASE IF EXISTS testdb;
```

## Good

DROP DATABASE should almost never appear in application migrations. Database lifecycle should be managed through infrastructure automation or DBA operations.

```sql
-- For local development: use database setup scripts
-- For production: use infrastructure automation (Terraform, Ansible)
-- For test cleanup: coordinate with DBA or use dedicated test infrastructure

-- If absolutely necessary (e.g., test cleanup), use a safety-assured block:
-- safety-assured:start
DROP DATABASE test_db;
-- safety-assured:end
```

## Important Considerations

- Database deletion should be handled by DBAs or infrastructure automation, not application migrations
- Ensure complete backups exist before proceeding
- Verify all connections to the database are terminated
- Consider using infrastructure tools (Terraform, Ansible) instead of migrations

**Note:** Postgres 13+ supports `DROP DATABASE ... WITH (FORCE)` to terminate active connections automatically, but this makes the operation even more dangerous and should be used with extreme caution.
