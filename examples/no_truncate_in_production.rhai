// Ban TRUNCATE statements (too dangerous for production).
// Catches: TRUNCATE users, orders;
// Inspect: diesel-guard dump-ast --sql "TRUNCATE users, orders;"

let stmt = node.TruncateStmt ?? return;

let violations = [];
for rel in stmt.relations {
    let name = rel.node?.RangeVar?.relname ?? continue;
    violations.push(#{
        operation: "TRUNCATE: " + name,
        problem: "TRUNCATE acquires ACCESS EXCLUSIVE lock on '" + name + "', blocking all reads and writes for the duration. Unlike DELETE, it also resets sequences and skips triggers.",
        safe_alternative: "Use batched DELETE instead:\n  DELETE FROM " + name + " WHERE id IN (SELECT id FROM " + name + " LIMIT 1000);"
    });
}
violations
