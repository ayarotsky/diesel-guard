// Ban TRUNCATE statements (too dangerous for production).
// Catches: TRUNCATE users, orders;
// Inspect: diesel-guard dump-ast --sql "TRUNCATE users, orders;"

let stmt = node.TruncateStmt;
if stmt == () { return; }

let violations = [];
for rel in stmt.relations {
    let rv = rel.node.RangeVar;
    if rv != () {
        let name = rv.relname;
        violations.push(#{
            operation: "TRUNCATE: " + name,
            problem: "TRUNCATE acquires ACCESS EXCLUSIVE lock on '" + name + "' and cannot be rolled back easily.",
            safe_alternative: "Use batched DELETE instead:\n  DELETE FROM " + name + " WHERE id IN (SELECT id FROM " + name + " LIMIT 1000);"
        });
    }
}
violations
