// Limit indexes to at most 3 columns.
// Catches: CREATE INDEX idx ON t(a, b, c, d);
// Inspect: diesel-guard dump-ast --sql "CREATE INDEX idx ON t(a, b, c, d);"

let max_cols = 3;
let stmt = node.IndexStmt;
if stmt == () { return; }

let col_count = stmt.index_params.len();

if col_count > max_cols {
    let idx_name = if stmt.idxname != "" { stmt.idxname } else { "(unnamed)" };
    #{
        operation: "Wide index: " + idx_name + " (" + col_count + " columns)",
        problem: "Index '" + idx_name + "' has " + col_count + " columns (limit: " + max_cols + "). Wide indexes are rarely effective and slow down writes.",
        safe_alternative: "Use narrower indexes targeting specific query patterns, or partial/covering indexes."
    }
}
