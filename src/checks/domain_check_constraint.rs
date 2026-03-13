//! Detection for domain CHECK constraints in migrations.
//!
//! This check identifies domain CHECK constraints introduced through
//! `CREATE DOMAIN ... CHECK (...)` and `ALTER DOMAIN ... ADD [CONSTRAINT] CHECK (...)`.
//!
//! Adding a CHECK constraint to an existing domain forces Postgres to validate every
//! column using that domain across all tables. This can become an expensive global
//! scan, and domain constraints do not support a `NOT VALID` escape hatch.
//!
//! Creating a domain with a CHECK constraint is also flagged conservatively because
//! domain-level constraints are global policy and cannot be rolled out incrementally
//! the way table-level CHECK constraints can.

use crate::checks::pg_helpers::{ConstrType, NodeEnum};
use crate::checks::{Check, Config};
use crate::violation::Violation;
use pg_query::protobuf::{Constraint, Node};

pub struct DomainCheckConstraintCheck;

impl Check for DomainCheckConstraintCheck {
    fn check(&self, node: &NodeEnum, _config: &Config) -> Vec<Violation> {
        match node {
            NodeEnum::CreateDomainStmt(create) => {
                let domain_name = qualified_name(&create.domainname);

                create
                    .constraints
                    .iter()
                    .filter_map(constraint_from_node)
                    .filter(|constraint| constraint.contype == ConstrType::ConstrCheck as i32)
                    .map(|constraint| create_domain_violation(&domain_name, constraint))
                    .collect()
            }
            NodeEnum::AlterDomainStmt(alter) => {
                if alter.subtype != "C" {
                    return vec![];
                }

                let Some(def) = alter.def.as_ref() else {
                    return vec![];
                };
                let Some(constraint) = constraint_from_node(def) else {
                    return vec![];
                };
                if constraint.contype != ConstrType::ConstrCheck as i32 {
                    return vec![];
                }

                vec![alter_domain_violation(
                    &qualified_name(&alter.type_name),
                    constraint,
                )]
            }
            _ => vec![],
        }
    }
}

fn constraint_from_node(node: &Node) -> Option<&Constraint> {
    match &node.node {
        Some(NodeEnum::Constraint(constraint)) => Some(constraint.as_ref()),
        _ => None,
    }
}

fn qualified_name(nodes: &[Node]) -> String {
    let name = nodes
        .iter()
        .filter_map(|node| match &node.node {
            Some(NodeEnum::String(value)) => Some(value.sval.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(".");

    if name.is_empty() {
        "<unknown>".to_string()
    } else {
        name
    }
}

fn constraint_name(constraint: &Constraint) -> &str {
    if constraint.conname.is_empty() {
        "<unnamed>"
    } else {
        &constraint.conname
    }
}

fn create_domain_violation(domain_name: &str, constraint: &Constraint) -> Violation {
    Violation::new(
        "DOMAIN CHECK constraint",
        format!(
            "Creating domain '{domain}' with CHECK constraint '{constraint}' is flagged conservatively. \
            Domain-level CHECK constraints are global policy and do not support the incremental \
            NOT VALID/VALIDATE CONSTRAINT rollout that table-level CHECK constraints do.",
            domain = domain_name,
            constraint = constraint_name(constraint)
        ),
        format!(
            r#"Prefer table-level or column-level CHECK constraints when you need an online rollout:

1. Keep the domain definition simple during the migration:
   CREATE DOMAIN {domain} AS <base_type>;

2. Add CHECK constraints per table with NOT VALID:
   ALTER TABLE users ADD CONSTRAINT users_email_check CHECK (email ~* '^[^@]+@[^@]+$') NOT VALID;

3. Validate each table separately:
   ALTER TABLE users VALIDATE CONSTRAINT users_email_check;

If you truly need a domain-level CHECK constraint on '{domain}', schedule it for a maintenance window instead of a regular online migration."#,
            domain = domain_name
        ),
    )
}

fn alter_domain_violation(domain_name: &str, constraint: &Constraint) -> Violation {
    Violation::new(
        "DOMAIN CHECK constraint",
        format!(
            "Adding CHECK constraint '{constraint}' to existing domain '{domain}' forces Postgres to validate every column using that domain across all tables. \
            This can become a global full scan while holding locks, and domain constraints have no NOT VALID escape hatch.",
            domain = domain_name,
            constraint = constraint_name(constraint)
        ),
        format!(
            r#"Prefer per-table CHECK constraints that can be rolled out incrementally:

1. Add a CHECK constraint with NOT VALID on each affected table:
   ALTER TABLE users ADD CONSTRAINT users_email_check CHECK (email ~* '^[^@]+@[^@]+$') NOT VALID;

2. Validate each table separately:
   ALTER TABLE users VALIDATE CONSTRAINT users_email_check;

3. Repeat for each table that needs the invariant.

If the invariant must live on domain '{domain}', schedule the domain CHECK constraint for a maintenance window instead of an online migration."#,
            domain = domain_name
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        assert_allows, assert_detects_n_violations, assert_detects_violation,
        assert_detects_violation_containing,
    };

    #[test]
    fn test_detects_create_domain_check_constraint() {
        assert_detects_violation_containing!(
            DomainCheckConstraintCheck,
            "CREATE DOMAIN email AS text CHECK (VALUE ~* '^[^@]+@[^@]+$');",
            "DOMAIN CHECK constraint",
            "flagged conservatively",
            "incremental"
        );
    }

    #[test]
    fn test_detects_create_domain_named_check_constraint() {
        assert_detects_violation!(
            DomainCheckConstraintCheck,
            "CREATE DOMAIN email AS text CONSTRAINT email_check CHECK (VALUE ~* '^[^@]+@[^@]+$');",
            "DOMAIN CHECK constraint"
        );
    }

    #[test]
    fn test_detects_alter_domain_add_named_check_constraint() {
        assert_detects_violation_containing!(
            DomainCheckConstraintCheck,
            "ALTER DOMAIN email ADD CONSTRAINT email_check CHECK (VALUE ~* '^[^@]+@[^@]+$');",
            "DOMAIN CHECK constraint",
            "validate every column using that domain",
            "global full scan"
        );
    }

    #[test]
    fn test_detects_alter_domain_add_unnamed_check_constraint() {
        assert_detects_violation!(
            DomainCheckConstraintCheck,
            "ALTER DOMAIN email ADD CHECK (VALUE ~* '^[^@]+@[^@]+$');",
            "DOMAIN CHECK constraint"
        );
    }

    #[test]
    fn test_detects_multiple_create_domain_check_constraints() {
        assert_detects_n_violations!(
            DomainCheckConstraintCheck,
            "CREATE DOMAIN email AS text CHECK (VALUE <> '') CONSTRAINT email_format_check CHECK (VALUE ~* '^[^@]+@[^@]+$');",
            2,
            "DOMAIN CHECK constraint"
        );
    }

    #[test]
    fn test_allows_create_domain_without_check_constraint() {
        assert_allows!(
            DomainCheckConstraintCheck,
            "CREATE DOMAIN email AS text NOT NULL;"
        );
    }

    #[test]
    fn test_allows_alter_domain_set_default() {
        assert_allows!(
            DomainCheckConstraintCheck,
            "ALTER DOMAIN email SET DEFAULT '';"
        );
    }

    #[test]
    fn test_allows_alter_domain_drop_default() {
        assert_allows!(
            DomainCheckConstraintCheck,
            "ALTER DOMAIN email DROP DEFAULT;"
        );
    }

    #[test]
    fn test_allows_alter_domain_set_not_null() {
        assert_allows!(
            DomainCheckConstraintCheck,
            "ALTER DOMAIN email SET NOT NULL;"
        );
    }

    #[test]
    fn test_allows_alter_domain_drop_not_null() {
        assert_allows!(
            DomainCheckConstraintCheck,
            "ALTER DOMAIN email DROP NOT NULL;"
        );
    }

    #[test]
    fn test_allows_alter_domain_drop_constraint() {
        assert_allows!(
            DomainCheckConstraintCheck,
            "ALTER DOMAIN email DROP CONSTRAINT email_check;"
        );
    }

    #[test]
    fn test_allows_alter_domain_validate_constraint() {
        assert_allows!(
            DomainCheckConstraintCheck,
            "ALTER DOMAIN email VALIDATE CONSTRAINT email_check;"
        );
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(
            DomainCheckConstraintCheck,
            "ALTER TABLE users ADD CONSTRAINT users_email_check CHECK (email <> '');"
        );
    }
}
