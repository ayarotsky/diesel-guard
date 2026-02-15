// Require IF EXISTS on DROP TABLE statements.
// Catches: DROP TABLE t;
// Inspect: diesel-guard dump-ast --sql "DROP TABLE t;"

let stmt = node.DropStmt;
if stmt == () { return; }

if stmt.remove_type == pg::OBJECT_TABLE && !stmt.missing_ok {
    #{
        operation: "DROP TABLE without IF EXISTS",
        problem: "DROP TABLE without IF EXISTS will error if the table doesn't exist, potentially breaking migrations.",
        safe_alternative: "Use IF EXISTS:\n  DROP TABLE IF EXISTS <table_name>;"
    }
}
