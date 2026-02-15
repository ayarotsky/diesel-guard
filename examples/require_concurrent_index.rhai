// Require CONCURRENTLY on CREATE INDEX.
// Catches: CREATE INDEX idx ON t(col);
// Inspect: diesel-guard dump-ast --sql "CREATE INDEX idx ON t(id);"

let stmt = node.IndexStmt;
if stmt == () { return; }

if !stmt.concurrent {
    let idx_name = if stmt.idxname != "" { stmt.idxname } else { "(unnamed)" };
    #{
        operation: "INDEX without CONCURRENTLY: " + idx_name,
        problem: "Creating index '" + idx_name + "' without CONCURRENTLY blocks writes on the table.",
        safe_alternative: "Use CREATE INDEX CONCURRENTLY:\n  CREATE INDEX CONCURRENTLY " + idx_name + " ON ...;"
    }
}
