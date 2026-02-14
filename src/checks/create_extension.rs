//! Detection for CREATE EXTENSION in migrations.
//!
//! This check identifies `CREATE EXTENSION` statements in migration files.
//!
//! CREATE EXTENSION often requires superuser privileges in PostgreSQL, which
//! application database users typically don't have in production environments.
//! Additionally, extensions are typically infrastructure concerns that should
//! be managed outside of application migrations.
//!
//! Extensions should be installed manually or through infrastructure automation
//! (Ansible, Terraform, etc.) with appropriate privileges before running migrations.

use crate::checks::pg_helpers::NodeEnum;
use crate::checks::Check;
use crate::violation::Violation;

pub struct CreateExtensionCheck;

impl Check for CreateExtensionCheck {
    fn check(&self, node: &NodeEnum) -> Vec<Violation> {
        let NodeEnum::CreateExtensionStmt(ext) = node else {
            return vec![];
        };

        let extension_name = &ext.extname;
        let if_not_exists_str = if ext.if_not_exists {
            "IF NOT EXISTS "
        } else {
            ""
        };

        vec![Violation::new(
            "CREATE EXTENSION",
            format!(
                "Creating extension '{extension}' in a migration requires superuser privileges, which application \
                database users typically lack in production. Extensions are infrastructure concerns that should be \
                managed outside application migrations.",
                extension = extension_name
            ),
            format!(
                r#"Install the extension outside of migrations:

1. For local development, add to your database setup scripts:
   CREATE EXTENSION {if_not_exists}{extension};

2. For production, use infrastructure automation (Ansible, Terraform, etc.):
   - Include extension installation in database provisioning
   - Grant appropriate privileges to superuser/admin role
   - Run before deploying application migrations

3. Document required extensions in your project README

Note: Common extensions like pg_trgm, uuid-ossp, hstore, and postgis should be
installed by your DBA or infrastructure team before application deployment."#,
                if_not_exists = if_not_exists_str,
                extension = extension_name
            ),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_allows, assert_detects_violation};

    #[test]
    fn test_detects_create_extension() {
        assert_detects_violation!(
            CreateExtensionCheck,
            "CREATE EXTENSION pg_trgm;",
            "CREATE EXTENSION"
        );
    }

    #[test]
    fn test_detects_create_extension_if_not_exists() {
        assert_detects_violation!(
            CreateExtensionCheck,
            "CREATE EXTENSION IF NOT EXISTS uuid_ossp;",
            "CREATE EXTENSION"
        );
    }

    #[test]
    fn test_ignores_other_create_statements() {
        assert_allows!(
            CreateExtensionCheck,
            "CREATE TABLE users (id SERIAL PRIMARY KEY);"
        );
    }

    #[test]
    fn test_ignores_create_index() {
        assert_allows!(
            CreateExtensionCheck,
            "CREATE INDEX idx_users_email ON users(email);"
        );
    }
}
