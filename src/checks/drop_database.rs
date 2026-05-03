//! Detection for DROP DATABASE operations.
//!
//! This check identifies `DROP DATABASE` statements, which permanently delete
//! entire databases including all tables, data, and objects. DROP DATABASE
//! is one of the most destructive operations possible and should almost never
//! appear in application migrations.
//!
//! DROP DATABASE requires exclusive access to the target database. Postgres will
//! refuse to execute the command if any active connections exist. Unlike table
//! operations, DROP DATABASE cannot be executed inside a transaction block.
//! There is no table rewrite involved; the entire database is removed at the
//! filesystem level.
//!
//! Postgres 13+ supports `DROP DATABASE ... WITH (FORCE)` to automatically
//! terminate active connections, making the operation even more dangerous.
//!
//! The recommended approach is to handle database lifecycle through infrastructure
//! automation or DBA operations, not application migrations.

use crate::checks::pg_helpers::NodeEnum;
use crate::checks::{Check, CheckDescription, Config, MigrationContext, if_exists_clause};
use crate::violation::Violation;

pub struct DropDatabaseCheck;

impl Check for DropDatabaseCheck {
    fn describe(&self) -> Vec<CheckDescription> {
        vec![CheckDescription {
            operation: "DROP DATABASE".into(),
            problem: "Dropping database '<name>' permanently deletes the entire database including all \
                      tables, data, and objects. This operation requires exclusive access (all connections \
                      must be terminated) and cannot run inside a transaction block.".into(),
            safe_alternative: "DROP DATABASE should almost never appear in application migrations.\n\n\
                               If you need to drop a database:\n\n\
                               1. Verify this is intentional and coordinate with your DBA:\n   \
                               -- Confirm database '<name>' is scheduled for removal\n\n\
                               2. Create a complete backup before proceeding:\n   \
                               pg_dump -Fc <name> > <name>_backup.dump\n\n\
                               3. Terminate all active connections to the database:\n   \
                               SELECT pg_terminate_backend(pid)\n   \
                               FROM pg_stat_activity\n   \
                               WHERE datname = '<name>' AND pid <> pg_backend_pid();\n\n\
                               4. Drop the database (outside of application migrations):\n   \
                               DROP DATABASE<if_exists> <name>;\n\n\
                               If this is intentional (e.g., test cleanup), use a safety-assured block:\n   \
                               -- safety-assured:start\n   \
                               DROP DATABASE<if_exists> <name>;\n   \
                               -- safety-assured:end\n\n\
                               Note: Postgres 13+ supports WITH (FORCE) to auto-terminate connections, but \
                               this is even more dangerous.".into(),
            script_path: None,
        }]
    }

    fn check(&self, node: &NodeEnum, _config: &Config, _ctx: &MigrationContext) -> Vec<Violation> {
        let descriptions = self.describe();
        let desc = &descriptions[0];
        let NodeEnum::DropdbStmt(drop_db) = node else {
            return vec![];
        };

        let db_name = &drop_db.dbname;
        let if_exists_str = if_exists_clause(drop_db.missing_ok);

        vec![Violation::new(
            desc.operation.clone(),
            desc.problem.replace("<name>", db_name),
            desc.safe_alternative
                .replace("<name>", db_name)
                .replace("<if_exists>", if_exists_str),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_allows, assert_detects_violation};

    #[test]
    fn test_detects_drop_database() {
        assert_detects_violation!(DropDatabaseCheck, "DROP DATABASE mydb;", "DROP DATABASE");
    }

    #[test]
    fn test_detects_drop_database_if_exists() {
        assert_detects_violation!(
            DropDatabaseCheck,
            "DROP DATABASE IF EXISTS mydb;",
            "DROP DATABASE"
        );
    }

    #[test]
    fn test_ignores_drop_table() {
        assert_allows!(DropDatabaseCheck, "DROP TABLE users;");
    }

    #[test]
    fn test_ignores_drop_index() {
        assert_allows!(DropDatabaseCheck, "DROP INDEX idx_users_email;");
    }

    #[test]
    fn test_ignores_create_database() {
        assert_allows!(DropDatabaseCheck, "CREATE DATABASE mydb;");
    }
}
