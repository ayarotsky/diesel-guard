//! Detection for CREATE EXTENSION in migrations.
//!
//! This check identifies `CREATE EXTENSION` statements in migration files.
//!
//! CREATE EXTENSION often requires superuser privileges in Postgres, which
//! application database users typically don't have in production environments.
//! Additionally, extensions are typically infrastructure concerns that should
//! be managed outside of application migrations.
//!
//! Extensions should be installed manually or through infrastructure automation
//! (Ansible, Terraform, etc.) with appropriate privileges before running migrations.

use crate::checks::pg_helpers::NodeEnum;
use crate::checks::{Check, CheckDescription, Config, MigrationContext};
use crate::violation::Violation;

pub struct CreateExtensionCheck;

impl Check for CreateExtensionCheck {
    fn describe(&self) -> Vec<CheckDescription> {
        vec![CheckDescription {
            operation: "CREATE EXTENSION".into(),
            problem: "Creating extension '<name>' in a migration requires superuser privileges, which \
                      application database users typically lack in production. Extensions are infrastructure \
                      concerns that should be managed outside application migrations.".into(),
            safe_alternative: "Install the extension outside of migrations:\n\n\
                               1. For local development, add to your database setup scripts:\n   \
                               CREATE EXTENSION <if_not_exists><name>;\n\n\
                               2. For production, use infrastructure automation (Ansible, Terraform, etc.):\n   \
                               - Include extension installation in database provisioning\n   \
                               - Grant appropriate privileges to superuser/admin role\n   \
                               - Run before deploying application migrations\n\n\
                               3. Document required extensions in your project README\n\n\
                               Note: Common extensions like pg_trgm, uuid-ossp, hstore, and postgis should be\n\
                               installed by your DBA or infrastructure team before application deployment.".into(),
            script_path: None,
        }]
    }

    fn check(&self, node: &NodeEnum, _config: &Config, _ctx: &MigrationContext) -> Vec<Violation> {
        let descriptions = self.describe();
        let desc = &descriptions[0];
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
            desc.operation.clone(),
            desc.problem.replace("<name>", extension_name),
            desc.safe_alternative
                .replace("<name>", extension_name)
                .replace("<if_not_exists>", if_not_exists_str),
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
