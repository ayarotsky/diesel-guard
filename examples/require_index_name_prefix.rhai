// Enforce naming convention: index names must start with "idx_".
// Catches: CREATE INDEX users_email ON users(email);
// Inspect: diesel-guard dump-ast --sql "CREATE INDEX users_email ON t(id);"

let stmt = node.IndexStmt;
if stmt == () { return; }

let name = stmt.idxname;
if name == "" { return; }  // Skip unnamed indexes

if name.starts_with("idx_") { return; }

#{
    operation: "Index naming violation: " + name,
    problem: "Index '" + name + "' does not follow naming convention. Index names should start with 'idx_'.",
    safe_alternative: "Rename the index:\n  CREATE INDEX idx_" + name + " ON ...;"
}
