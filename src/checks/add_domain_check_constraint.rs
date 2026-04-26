use crate::checks::pg_helpers::constraint_display_name;
use crate::checks::{Check, CheckDescription};
use crate::{Config, MigrationContext, Violation};
use pg_query::NodeEnum;
use pg_query::protobuf::ConstrType;

pub struct AddDomainCheckConstraintCheck;

impl Check for AddDomainCheckConstraintCheck {
    fn describe(&self) -> CheckDescription {
        CheckDescription {
            operation: "ADD CHECK CONSTRAINT TO DOMAIN".into(),
            problem: "Adding a CHECK constraint to a domain without NOT VALID causes Postgres to \
                      validate all columns using this domain across all tables, which can be a slow, \
                      lock-holding full-scan operation.".into(),
            safe_alternative: "Add the constraint with NOT VALID first, then validate in a separate migration.".into(),
            script_path: None,
        }
    }

    fn check(&self, node: &NodeEnum, _config: &Config, _ctx: &MigrationContext) -> Vec<Violation> {
        // Only ALTER DOMAIN ADD CONSTRAINT is dangerous on existing domains.
        // CREATE DOMAIN is always safe: the domain is new, so no columns use it yet.
        let NodeEnum::AlterDomainStmt(stmt) = node else {
            return vec![];
        };

        // subtype 'C' = ADD CONSTRAINT
        if stmt.subtype != "C" {
            return vec![];
        }

        let Some(constraint) =
            stmt.def
                .as_ref()
                .and_then(|d| d.node.as_ref())
                .and_then(|n| match n {
                    NodeEnum::Constraint(c) => Some(c.as_ref()),
                    _ => None,
                })
        else {
            return vec![];
        };

        if constraint.contype != ConstrType::ConstrCheck as i32 {
            return vec![];
        }

        // initially_valid=false means NOT VALID was specified — defers validation, safe.
        if !constraint.initially_valid {
            return vec![];
        }

        let domain_name = stmt
            .type_name
            .iter()
            .filter_map(|n| match &n.node {
                Some(NodeEnum::String(s)) => Some(s.sval.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(".");
        let constraint_name = constraint_display_name(constraint);

        vec![Violation::new(
            self.describe().operation,
            format!(
                "Adding CHECK constraint '{constraint_name}' to domain '{domain_name}' without \
NOT VALID causes Postgres to validate all columns using this domain across all tables, \
which can be a slow, lock-holding full-scan operation."
            ),
            format!(
                "Add the constraint with NOT VALID first, then validate in a separate migration:\n\n\
1. Add without validation (fast, no full scan):\n   \
ALTER DOMAIN {domain_name} ADD CONSTRAINT {constraint_name} CHECK <expr> NOT VALID;\n\n\
2. Validate in a separate migration:\n   \
ALTER DOMAIN {domain_name} VALIDATE CONSTRAINT {constraint_name};"
            ),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_allows, assert_detects_violation};

    #[test]
    fn test_detects_alter_domain_add_check_without_not_valid() {
        assert_detects_violation!(
            AddDomainCheckConstraintCheck,
            "ALTER DOMAIN positive_int ADD CONSTRAINT pos_check CHECK (VALUE > 0);",
            "ADD CHECK CONSTRAINT TO DOMAIN"
        );
    }

    #[test]
    fn test_allows_alter_domain_add_check_not_valid() {
        assert_allows!(
            AddDomainCheckConstraintCheck,
            "ALTER DOMAIN positive_int ADD CONSTRAINT pos_check CHECK (VALUE > 0) NOT VALID;"
        );
    }

    #[test]
    fn test_allows_create_domain_with_check() {
        // CREATE DOMAIN is always safe: no tables use the domain yet.
        assert_allows!(
            AddDomainCheckConstraintCheck,
            "CREATE DOMAIN positive_int AS INTEGER CHECK (VALUE > 0);"
        );
    }

    #[test]
    fn test_allows_alter_domain_drop_constraint() {
        assert_allows!(
            AddDomainCheckConstraintCheck,
            "ALTER DOMAIN positive_int DROP CONSTRAINT pos_check;"
        );
    }

    #[test]
    fn test_allows_alter_domain_set_default() {
        assert_allows!(
            AddDomainCheckConstraintCheck,
            "ALTER DOMAIN positive_int SET DEFAULT 0;"
        );
    }

    #[test]
    fn test_ignores_non_domain_nodes() {
        assert_allows!(AddDomainCheckConstraintCheck, "CREATE TABLE foo (id int);");
    }
}
