// Prevent UNLOGGED tables in migrations.
// Catches: CREATE UNLOGGED TABLE t (id INT);
// Inspect: diesel-guard dump-ast --sql "CREATE UNLOGGED TABLE t (id INT);"

let rel = node.CreateStmt?.relation ?? return;
if rel.relpersistence != "u" { return; }

let table_name = rel.relname;
#{
    operation: "UNLOGGED TABLE: " + table_name,
    problem: "UNLOGGED tables are not crash-safe and are not replicated to standby servers.",
    safe_alternative: "Use a regular (logged) table instead:\n  CREATE TABLE " + table_name + " (...);"
}
