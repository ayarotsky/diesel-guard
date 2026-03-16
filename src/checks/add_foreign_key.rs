use crate::checks::Check;
use crate::checks::pg_helpers::{
    alter_table_cmds, cmd_def_as_constraint, fk_cols_constraint, ref_columns_constraint,
    ref_table_constraint,
};
use crate::{Config, MigrationContext, Violation};
use pg_query::NodeEnum;
use pg_query::protobuf::ConstrType;

pub struct AddForeignKeyCheck;

impl Check for AddForeignKeyCheck {
    fn check(&self, node: &NodeEnum, _config: &Config, _ctx: &MigrationContext) -> Vec<Violation> {
        let Some((table_name, cmds)) = alter_table_cmds(node) else {
            return vec![];
        };
        cmds.iter().filter_map(|cmd| {
            let constraint = cmd_def_as_constraint(cmd)?;
            if constraint.contype != ConstrType::ConstrForeign as i32 {
                return None;
            }

            if !constraint.initially_valid {
                return None;
            }

            let fk_cols = fk_cols_constraint(constraint);

            let ref_table = ref_table_constraint(constraint);

            let ref_cols = ref_columns_constraint(constraint);

            let constraint_name = if constraint.conname.is_empty() {
                "<unnamed>".to_string()
            } else {
                constraint.conname.clone()
            };

            Some(Violation::new(
                "ADD FOREIGN KEY",
                format!("Adding a foreign key constraint '{constraint_name}' on table '{table_name}' ({fk_cols}) without NOT VALID scans the entire table to validate existing rows,\
             acquiring ShareRowExclusiveLock for the duration. On large tables this blocks writes and is a common cause of migration-induced outages."),
                format!(
                    r#"For a safer foreign key addition on large tables:

1. Create a foreign key without any immediate validation:
   ALTER TABLE {table} ADD CONSTRAINT {constraint_name}
    FOREIGN KEY ({columns}) REFERENCES {ref_table} ({ref_cols}) NOT VALID;

2. Step 2 (separate migration, acquires ShareUpdateExclusiveLock only)
  ALTER TABLE {table_name} VALIDATE CONSTRAINT {constraint_name};

Benefits:
- Table remains readable and writable during foreign key creation
- No blocking of SELECT, INSERT, UPDATE, or DELETE operations
- Safe for production deployments on large tables
"#,
                    table = table_name,
                    columns = fk_cols,
                )))
        }).collect()
    }
}
