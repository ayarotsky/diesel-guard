// Require IF EXISTS on DROP TABLE statements.
// Catches: DROP TABLE t;
// Inspect: diesel-guard dump-ast --sql "DROP TABLE t;"

let stmt = node.DropStmt;
if stmt == () { return; }

// remove_type 42 = TABLE (ObjectType::OBJECT_TABLE in pg_query v6 / PG17)
if stmt.remove_type == 42 && !stmt.missing_ok {
    #{
        operation: "DROP TABLE without IF EXISTS",
        problem: "DROP TABLE without IF EXISTS will error if the table doesn't exist, potentially breaking migrations.",
        safe_alternative: "Use IF EXISTS:\n  DROP TABLE IF EXISTS <table_name>;"
    }
}
