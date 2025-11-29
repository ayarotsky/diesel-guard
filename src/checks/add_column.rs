use crate::checks::Check;
use crate::error::Result;
use crate::violation::Violation;
use sqlparser::ast::{AlterTableOperation, ColumnOption, Statement};

pub struct AddColumnCheck;

impl Check for AddColumnCheck {
    fn name(&self) -> &str {
        "add_column_with_default"
    }

    fn check(&self, stmt: &Statement) -> Result<Vec<Violation>> {
        let mut violations = vec![];

        if let Statement::AlterTable {
            name, operations, ..
        } = stmt
        {
            for op in operations {
                if let AlterTableOperation::AddColumn { column_def, .. } = op {
                    // Check if column has a DEFAULT value
                    let has_default = column_def
                        .options
                        .iter()
                        .any(|opt| matches!(opt.option, ColumnOption::Default(_)));

                    if has_default {
                        let column_name = &column_def.name;
                        let table_name = name.to_string();

                        violations.push(Violation::new(
                            "ADD COLUMN with DEFAULT",
                            format!(
                                "Adding column '{}' with DEFAULT locks table '{}' while backfilling on PostgreSQL < 11. \
                                This can take hours on large tables and block all reads/writes.",
                                column_name, table_name
                            ),
                            format!(
                                "1. Add the column without a default:\n   \
                                 ALTER TABLE {} ADD COLUMN {} {};\n\n\
                                 2. Backfill data in batches (outside migration):\n   \
                                 UPDATE {} SET {} = <value> WHERE {} IS NULL;\n\n\
                                 3. Add default for new rows only:\n   \
                                 ALTER TABLE {} ALTER COLUMN {} SET DEFAULT <value>;\n\n\
                                 Note: For PostgreSQL 11+, this is safe if the default is a constant value.",
                                table_name,
                                column_name,
                                column_def.data_type,
                                table_name,
                                column_name,
                                column_name,
                                table_name,
                                column_name
                            ),
                        ));
                    }
                }
            }
        }

        Ok(violations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlparser::dialect::PostgreSqlDialect;
    use sqlparser::parser::Parser;

    fn parse_sql(sql: &str) -> Statement {
        let dialect = PostgreSqlDialect {};
        Parser::parse_sql(&dialect, sql)
            .expect("Failed to parse SQL")
            .into_iter()
            .next()
            .expect("No statements found")
    }

    #[test]
    fn test_detects_add_column_with_default() {
        let check = AddColumnCheck;
        let stmt = parse_sql("ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;");

        let violations = check.check(&stmt).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].operation, "ADD COLUMN with DEFAULT");
    }

    #[test]
    fn test_allows_add_column_without_default() {
        let check = AddColumnCheck;
        let stmt = parse_sql("ALTER TABLE users ADD COLUMN admin BOOLEAN;");

        let violations = check.check(&stmt).unwrap();
        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn test_ignores_other_statements() {
        let check = AddColumnCheck;
        let stmt = parse_sql("CREATE TABLE users (id SERIAL PRIMARY KEY);");

        let violations = check.check(&stmt).unwrap();
        assert_eq!(violations.len(), 0);
    }
}
