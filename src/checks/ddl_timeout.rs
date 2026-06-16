//! Detection for DDL before lock_timeout and statement_timeout are configured.
//!
//! This check is sequence-aware: it tracks `SET lock_timeout` and
//! `SET statement_timeout` statements before DDL in the same SQL input.

use crate::ViolationList;
use crate::checks::pg_helpers::NodeEnum;
use crate::checks::{Config, MigrationContext, StatementContext, StatementListCheck};
use crate::violation::Violation;
use pg_query::protobuf::{VariableSetKind, a_const};

pub struct DdlTimeoutCheck;

impl StatementListCheck for DdlTimeoutCheck {
    fn check(
        &self,
        statements: &[StatementContext<'_>],
        _config: &Config,
        _ctx: &MigrationContext,
    ) -> ViolationList {
        let mut has_lock_timeout = false;
        let mut has_statement_timeout = false;
        let mut violations = Vec::new();

        for stmt in statements {
            match timeout_transition(stmt.node) {
                Some(TimeoutTransition::Set(TimeoutSetting::LockTimeout)) => {
                    has_lock_timeout = true;
                    continue;
                }
                Some(TimeoutTransition::Set(TimeoutSetting::StatementTimeout)) => {
                    has_statement_timeout = true;
                    continue;
                }
                Some(TimeoutTransition::Clear(TimeoutSetting::LockTimeout)) => {
                    has_lock_timeout = false;
                    continue;
                }
                Some(TimeoutTransition::Clear(TimeoutSetting::StatementTimeout)) => {
                    has_statement_timeout = false;
                    continue;
                }
                Some(TimeoutTransition::ClearAll) => {
                    has_lock_timeout = false;
                    has_statement_timeout = false;
                    continue;
                }
                None => {}
            }

            if stmt.ignored || !is_ddl(stmt.node) || (has_lock_timeout && has_statement_timeout) {
                continue;
            }

            violations.push((
                stmt.line,
                Violation::new(
                    "DDL without lock_timeout/statement_timeout",
                    missing_timeout_problem(has_lock_timeout, has_statement_timeout),
                    r"Set both lock_timeout and statement_timeout before running DDL:

SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;

This makes migrations fail fast instead of waiting indefinitely on locks or long-running statements.",
                ),
            ));
        }

        violations
    }
}

enum TimeoutSetting {
    LockTimeout,
    StatementTimeout,
}

enum TimeoutTransition {
    Set(TimeoutSetting),
    Clear(TimeoutSetting),
    ClearAll,
}

fn timeout_transition(node: &NodeEnum) -> Option<TimeoutTransition> {
    let NodeEnum::VariableSetStmt(stmt) = node else {
        return None;
    };

    let setting = if stmt.name.eq_ignore_ascii_case("lock_timeout") {
        Some(TimeoutSetting::LockTimeout)
    } else if stmt.name.eq_ignore_ascii_case("statement_timeout") {
        Some(TimeoutSetting::StatementTimeout)
    } else {
        None
    };

    match VariableSetKind::try_from(stmt.kind).ok()? {
        VariableSetKind::VarSetValue => {
            let setting = setting?;
            if timeout_value_is_disabled(stmt.args.first()) {
                Some(TimeoutTransition::Clear(setting))
            } else {
                Some(TimeoutTransition::Set(setting))
            }
        }
        VariableSetKind::VarSetDefault | VariableSetKind::VarReset => {
            setting.map(TimeoutTransition::Clear)
        }
        VariableSetKind::VarResetAll => Some(TimeoutTransition::ClearAll),
        VariableSetKind::Undefined
        | VariableSetKind::VarSetCurrent
        | VariableSetKind::VarSetMulti => None,
    }
}

fn timeout_value_is_disabled(value: Option<&pg_query::protobuf::Node>) -> bool {
    let Some(NodeEnum::AConst(value)) = value.and_then(|node| node.node.as_ref()) else {
        return false;
    };

    match &value.val {
        Some(a_const::Val::Ival(value)) => value.ival == 0,
        Some(a_const::Val::Fval(value)) => is_zeroish_timeout_literal(&value.fval),
        Some(a_const::Val::Sval(value)) => is_zeroish_timeout_literal(&value.sval),
        _ => false,
    }
}

fn is_zeroish_timeout_literal(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }

    let unit_start = normalized
        .char_indices()
        .find(|(_, ch)| {
            !(ch.is_ascii_digit() || matches!(ch, '.' | '+' | '-' | ' ' | '\t' | '\n' | '\r'))
        })
        .map_or(normalized.len(), |(index, _)| index);

    let (amount, unit) = normalized.split_at(unit_start);
    let is_zero = amount
        .trim()
        .parse::<f64>()
        .is_ok_and(|amount| amount == 0.0);

    is_zero
        && (unit.trim().is_empty()
            || matches!(
                unit.trim(),
                "us" | "usec"
                    | "usecs"
                    | "microsecond"
                    | "microseconds"
                    | "ms"
                    | "msec"
                    | "msecs"
                    | "millisecond"
                    | "milliseconds"
                    | "s"
                    | "sec"
                    | "secs"
                    | "second"
                    | "seconds"
                    | "min"
                    | "mins"
                    | "minute"
                    | "minutes"
                    | "h"
                    | "hr"
                    | "hrs"
                    | "hour"
                    | "hours"
                    | "d"
                    | "day"
                    | "days"
            ))
}

fn is_ddl(node: &NodeEnum) -> bool {
    matches!(
        node,
        NodeEnum::AlterCollationStmt(_)
            | NodeEnum::AlterDefaultPrivilegesStmt(_)
            | NodeEnum::AlterDomainStmt(_)
            | NodeEnum::AlterDatabaseRefreshCollStmt(_)
            | NodeEnum::AlterDatabaseSetStmt(_)
            | NodeEnum::AlterDatabaseStmt(_)
            | NodeEnum::AlterEventTrigStmt(_)
            | NodeEnum::AlterEnumStmt(_)
            | NodeEnum::AlterExtensionStmt(_)
            | NodeEnum::AlterExtensionContentsStmt(_)
            | NodeEnum::AlterFdwStmt(_)
            | NodeEnum::AlterForeignServerStmt(_)
            | NodeEnum::AlterFunctionStmt(_)
            | NodeEnum::AlterObjectSchemaStmt(_)
            | NodeEnum::AlterOwnerStmt(_)
            | NodeEnum::AlterPolicyStmt(_)
            | NodeEnum::AlterOperatorStmt(_)
            | NodeEnum::AlterOpFamilyStmt(_)
            | NodeEnum::AlterSeqStmt(_)
            | NodeEnum::AlterStatsStmt(_)
            | NodeEnum::AlterSubscriptionStmt(_)
            | NodeEnum::AlterTableStmt(_)
            | NodeEnum::AlterTableMoveAllStmt(_)
            | NodeEnum::AlterTableSpaceOptionsStmt(_)
            | NodeEnum::AlterTsdictionaryStmt(_)
            | NodeEnum::AlterTsconfigurationStmt(_)
            | NodeEnum::AlterTypeStmt(_)
            | NodeEnum::AlterSystemStmt(_)
            | NodeEnum::AlterObjectDependsStmt(_)
            | NodeEnum::AlterUserMappingStmt(_)
            | NodeEnum::AlterPublicationStmt(_)
            | NodeEnum::AlterRoleStmt(_)
            | NodeEnum::AlterRoleSetStmt(_)
            | NodeEnum::CompositeTypeStmt(_)
            | NodeEnum::CreatedbStmt(_)
            | NodeEnum::CreateAmStmt(_)
            | NodeEnum::CreateCastStmt(_)
            | NodeEnum::CreateConversionStmt(_)
            | NodeEnum::CreateDomainStmt(_)
            | NodeEnum::CreateEnumStmt(_)
            | NodeEnum::CreateEventTrigStmt(_)
            | NodeEnum::CreateExtensionStmt(_)
            | NodeEnum::CreateFdwStmt(_)
            | NodeEnum::CreateForeignServerStmt(_)
            | NodeEnum::CreateForeignTableStmt(_)
            | NodeEnum::CreateFunctionStmt(_)
            | NodeEnum::CreateOpClassStmt(_)
            | NodeEnum::CreateOpFamilyStmt(_)
            | NodeEnum::CreatePlangStmt(_)
            | NodeEnum::CreatePolicyStmt(_)
            | NodeEnum::CreatePublicationStmt(_)
            | NodeEnum::CreateRangeStmt(_)
            | NodeEnum::CreateRoleStmt(_)
            | NodeEnum::CreateSchemaStmt(_)
            | NodeEnum::CreateSeqStmt(_)
            | NodeEnum::CreateStatsStmt(_)
            | NodeEnum::CreateStmt(_)
            | NodeEnum::CreateSubscriptionStmt(_)
            | NodeEnum::CreateTableAsStmt(_)
            | NodeEnum::CreateTableSpaceStmt(_)
            | NodeEnum::CreateTransformStmt(_)
            | NodeEnum::CreateTrigStmt(_)
            | NodeEnum::CreateUserMappingStmt(_)
            | NodeEnum::CommentStmt(_)
            | NodeEnum::DropStmt(_)
            | NodeEnum::DropdbStmt(_)
            | NodeEnum::DropOwnedStmt(_)
            | NodeEnum::DropRoleStmt(_)
            | NodeEnum::DropSubscriptionStmt(_)
            | NodeEnum::DropTableSpaceStmt(_)
            | NodeEnum::DropUserMappingStmt(_)
            | NodeEnum::DefineStmt(_)
            | NodeEnum::GrantRoleStmt(_)
            | NodeEnum::GrantStmt(_)
            | NodeEnum::ImportForeignSchemaStmt(_)
            | NodeEnum::IndexStmt(_)
            | NodeEnum::RefreshMatViewStmt(_)
            | NodeEnum::ReindexStmt(_)
            | NodeEnum::ReassignOwnedStmt(_)
            | NodeEnum::RenameStmt(_)
            | NodeEnum::ReplicaIdentityStmt(_)
            | NodeEnum::RuleStmt(_)
            | NodeEnum::SecLabelStmt(_)
            | NodeEnum::TruncateStmt(_)
            | NodeEnum::ViewStmt(_)
    )
}

fn missing_timeout_problem(has_lock_timeout: bool, has_statement_timeout: bool) -> String {
    let (missing, verb) = match (has_lock_timeout, has_statement_timeout) {
        (false, false) => ("lock_timeout and statement_timeout", "are"),
        (false, true) => ("lock_timeout", "is"),
        (true, false) => ("statement_timeout", "is"),
        (true, true) => unreachable!("no missing timeout when both are set"),
    };

    format!(
        "DDL runs before {missing} {verb} configured. Without timeouts, migrations can wait \
        indefinitely for locks or long-running statements, delaying production traffic."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::pg_helpers::extract_node;

    fn check_sql(sql: &str) -> ViolationList {
        let parsed = pg_query::parse(sql).unwrap();
        let statements = parsed
            .protobuf
            .stmts
            .iter()
            .enumerate()
            .filter_map(|(index, raw_stmt)| {
                let node = extract_node(raw_stmt)?;
                Some(StatementContext {
                    node,
                    line: index + 1,
                    ignored: false,
                })
            })
            .collect::<Vec<_>>();

        DdlTimeoutCheck.check(
            &statements,
            &Config::default(),
            &MigrationContext::default(),
        )
    }

    #[test]
    fn test_detects_ddl_without_timeouts() {
        let violations = check_sql("ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;");

        assert_eq!(violations.len(), 1);
        assert_eq!(
            violations[0].1.operation,
            "DDL without lock_timeout/statement_timeout"
        );
        assert!(
            violations[0]
                .1
                .problem
                .contains("lock_timeout and statement_timeout")
        );
    }

    #[test]
    fn test_allows_ddl_after_both_timeouts() {
        let violations = check_sql(
            r"
SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;
        ",
        );

        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn test_detects_missing_statement_timeout() {
        let violations = check_sql(
            r"
SET lock_timeout = '2s';
ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;
        ",
        );

        assert_eq!(violations.len(), 1);
        assert!(violations[0].1.problem.contains("statement_timeout"));
    }

    #[test]
    fn test_set_local_counts_as_timeout_assignment() {
        let violations = check_sql(
            r"
SET LOCAL lock_timeout = '2s';
SET LOCAL statement_timeout = '60s';
ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;
        ",
        );

        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn test_reset_does_not_count_as_timeout_assignment() {
        let violations = check_sql(
            r"
RESET lock_timeout;
SET statement_timeout = '60s';
ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;
        ",
        );

        assert_eq!(violations.len(), 1);
        assert!(violations[0].1.problem.contains("lock_timeout"));
    }

    #[test]
    fn test_reset_clears_previous_timeout_assignment() {
        let violations = check_sql(
            r"
SET lock_timeout = '2s';
SET statement_timeout = '60s';
RESET lock_timeout;
ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;
        ",
        );

        assert_eq!(violations.len(), 1);
        assert!(violations[0].1.problem.contains("lock_timeout"));
    }

    #[test]
    fn test_default_clears_previous_timeout_assignment() {
        let violations = check_sql(
            r"
SET lock_timeout = '2s';
SET statement_timeout = '60s';
SET statement_timeout = DEFAULT;
ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;
        ",
        );

        assert_eq!(violations.len(), 1);
        assert!(violations[0].1.problem.contains("statement_timeout"));
    }

    #[test]
    fn test_reset_all_clears_timeout_assignments() {
        let violations = check_sql(
            r"
SET lock_timeout = '2s';
SET statement_timeout = '60s';
RESET ALL;
ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;
        ",
        );

        assert_eq!(violations.len(), 1);
        assert!(
            violations[0]
                .1
                .problem
                .contains("lock_timeout and statement_timeout")
        );
    }

    #[test]
    fn test_zero_timeout_values_do_not_satisfy_check() {
        let violations = check_sql(
            r"
SET lock_timeout = 0;
SET statement_timeout = '0ms';
ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;
        ",
        );

        assert_eq!(violations.len(), 1);
        assert!(
            violations[0]
                .1
                .problem
                .contains("lock_timeout and statement_timeout")
        );
    }

    #[test]
    fn test_additional_zero_timeout_units_do_not_satisfy_check() {
        for value in [
            "0us",
            "0 usecs",
            "0 microseconds",
            "0msec",
            "0 seconds",
            "0min",
            "0 hours",
            "0d",
            "0 days",
        ] {
            let sql = format!(
                r"
SET lock_timeout = '{value}';
SET statement_timeout = '60s';
ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;
        "
            );

            let violations = check_sql(&sql);

            assert_eq!(violations.len(), 1, "expected {value:?} to be disabled");
            assert!(violations[0].1.problem.contains("lock_timeout"));
        }
    }

    #[test]
    fn test_fractional_nonzero_timeout_values_satisfy_check() {
        for value in ["0.5s", "0.001 seconds", "1ms", "100us", "1 day"] {
            let sql = format!(
                r"
SET lock_timeout = '{value}';
SET statement_timeout = '60s';
ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;
        "
            );

            let violations = check_sql(&sql);

            assert_eq!(
                violations.len(),
                0,
                "expected {value:?} to satisfy lock_timeout"
            );
        }
    }

    #[test]
    fn test_zero_timeout_clears_previous_assignment() {
        let violations = check_sql(
            r"
SET lock_timeout = '2s';
SET statement_timeout = '60s';
SET lock_timeout = '0';
ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;
        ",
        );

        assert_eq!(violations.len(), 1);
        assert!(violations[0].1.problem.contains("lock_timeout"));
    }

    #[test]
    fn test_ignores_non_ddl() {
        let violations = check_sql("UPDATE users SET active = false WHERE id = 1;");

        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn test_detects_broad_ddl_statement_kinds_without_timeouts() {
        for sql in [
            "CREATE TYPE mood AS ENUM ('happy', 'sad');",
            "CREATE TRIGGER users_updated BEFORE UPDATE ON users FOR EACH ROW EXECUTE FUNCTION touch_updated_at();",
            "CREATE POLICY users_policy ON users USING (true);",
            "ALTER POLICY users_policy ON users USING (false);",
            "CREATE FUNCTION touch_updated_at() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NEW; END $$;",
            "ALTER FUNCTION touch_updated_at() OWNER TO postgres;",
            "CREATE DATABASE analytics;",
            "CREATE PUBLICATION pub_all FOR ALL TABLES;",
            "ALTER PUBLICATION pub_all SET (publish = 'insert, update');",
            "CREATE SUBSCRIPTION sub CONNECTION 'host=localhost' PUBLICATION pub WITH (connect = false);",
            "ALTER SUBSCRIPTION sub DISABLE;",
            "DROP SUBSCRIPTION sub;",
            "CREATE CAST (text AS int4) WITH INOUT AS ASSIGNMENT;",
            "CREATE DEFAULT CONVERSION myconv FOR 'UTF8' TO 'LATIN1' FROM utf8_to_latin1;",
            "CREATE TRANSFORM FOR hstore LANGUAGE plpgsql (FROM SQL WITH FUNCTION hstore_recv(internal), TO SQL WITH FUNCTION hstore_send(hstore));",
            "ALTER COLLATION \"C\" REFRESH VERSION;",
            "ALTER TEXT SEARCH DICTIONARY english_stem (StopWords = english);",
            "CREATE ROLE app_user;",
            "ALTER ROLE app_user SET search_path = public;",
            "DROP ROLE app_user;",
            "GRANT SELECT ON TABLE users TO app_user;",
            "ALTER DEFAULT PRIVILEGES GRANT SELECT ON TABLES TO app_user;",
            "COMMENT ON TABLE users IS 'customer records';",
            "SECURITY LABEL FOR selinux ON TABLE users IS 'system_u:object_r:sepgsql_table_t:s0';",
        ] {
            let violations = check_sql(sql);

            assert_eq!(violations.len(), 1, "expected DDL violation for {sql}");
            assert_eq!(
                violations[0].1.operation,
                "DDL without lock_timeout/statement_timeout"
            );
        }
    }
}
