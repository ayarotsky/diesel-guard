use crate::checks::pg_helpers::{
    ConstrType, NodeEnum, alter_table_cmds, cmd_def_as_constraint, constraint_display_name,
};
use crate::checks::{Check, CheckDescription, Config, MigrationContext};
use crate::violation::Violation;

pub struct AddExcludeConstraintCheck;

impl Check for AddExcludeConstraintCheck {
    fn describe(&self) -> CheckDescription {
        CheckDescription {
            operation: "ADD EXCLUDE constraint".into(),
            problem: "Adding an exclusion constraint scans the entire table while holding a SHARE ROW EXCLUSIVE lock. \
                      Unlike CHECK or FOREIGN KEY, there is no NOT VALID escape hatch — exclusion constraints must \
                      be validated immediately.".into(),
            safe_alternative: "Add the constraint during a low-traffic window, define it at table creation time, \
                               or use application-level enforcement if the table is too large to lock safely.".into(),
            script_path: None,
        }
    }

    fn check(&self, node: &NodeEnum, _config: &Config, _ctx: &MigrationContext) -> Vec<Violation> {
        let Some((table_name, cmds)) = alter_table_cmds(node) else {
            return vec![];
        };

        cmds.iter()
            .filter_map(|cmd| {
                let c = cmd_def_as_constraint(cmd)?;

                if c.contype != ConstrType::ConstrExclusion as i32 {
                    return None;
                }

                let constraint_name = constraint_display_name(c);

                Some(Violation::new(
                    self.describe().operation,
                    format!(
                        "Adding exclusion constraint '{constraint_name}' on table '{table_name}' \
                        scans the entire table while holding a SHARE ROW EXCLUSIVE lock. \
                        Unlike CHECK or FOREIGN KEY constraints, there is no NOT VALID escape hatch — \
                        exclusion constraints must be validated immediately."
                    ),
                    format!(
                        r"There is no non-blocking path for adding an exclusion constraint to an existing table.

Options:
- Add the constraint during a low-traffic window and accept the full-table scan cost
- Define the constraint at table creation time to avoid scanning existing rows:
  CREATE TABLE {table_name} (..., CONSTRAINT {constraint_name} EXCLUDE USING <method> (<elements>));
- Use application-level enforcement if the table is too large to lock safely"
                    ),
                ))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_allows, assert_detects_violation};

    #[test]
    fn test_detects_exclude_constraint() {
        assert_detects_violation!(
            AddExcludeConstraintCheck,
            "ALTER TABLE meeting_rooms ADD CONSTRAINT no_double_booking EXCLUDE USING gist (room_id WITH =);",
            "ADD EXCLUDE constraint"
        );
    }

    #[test]
    fn test_ignores_check_constraint() {
        assert_allows!(
            AddExcludeConstraintCheck,
            "ALTER TABLE orders ADD CONSTRAINT check_amount CHECK (amount > 0) NOT VALID;"
        );
    }

    #[test]
    fn test_ignores_add_column() {
        assert_allows!(
            AddExcludeConstraintCheck,
            "ALTER TABLE users ADD COLUMN email TEXT;"
        );
    }

    #[test]
    fn test_ignores_create_table() {
        assert_allows!(
            AddExcludeConstraintCheck,
            "CREATE TABLE users (id BIGINT PRIMARY KEY, email TEXT);"
        );
    }
}
