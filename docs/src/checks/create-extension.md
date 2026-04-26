# Creating an Extension

**Check name:** `CreateExtensionCheck`

**Lock type:** Requires superuser privileges

## Bad

Creating an extension in migrations often requires superuser privileges, which application database users typically don't have in production environments.

```sql
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE EXTENSION uuid_ossp;
```

## Good

Install extensions outside of application migrations:

```sql
-- For local development: add to database setup scripts
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- For production: use infrastructure automation
-- (Ansible, Terraform, or manual DBA installation)
```

## Best Practices

- Document required extensions in your project README
- Include extension installation in database provisioning scripts
- Use infrastructure automation (Ansible, Terraform) for production
- Have your DBA or infrastructure team install extensions before deployment

Common extensions that require this approach: `pg_trgm`, `uuid-ossp`, `hstore`, `postgis`, `pg_stat_statements`.
