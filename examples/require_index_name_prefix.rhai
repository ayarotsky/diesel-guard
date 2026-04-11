// Enforce naming convention: index names must start with "idx_".
// Catches: CREATE INDEX users_email ON users(email);
// Inspect: diesel-guard dump-ast --sql "CREATE INDEX users_email ON t(id);"

let name = node.IndexStmt?.idxname ?? return;
if name == "" || name.starts_with("idx_") { return; }

#{
    operation: "Index naming violation: " + name,
    problem: "Index '" + name + "' does not follow naming convention. Index names should start with 'idx_'.",
    safe_alternative: "Rename the index:\n  CREATE INDEX idx_" + name + " ON ...;"
}
