// Require CONCURRENTLY on CREATE INDEX.
// Also flags CONCURRENTLY inside a transaction (PostgreSQL will error at runtime).
// Inspect: diesel-guard dump-ast --sql "CREATE INDEX idx ON t(id);"

let stmt = node.IndexStmt ?? return;

if !stmt.concurrent {
    let idx_name = if stmt.idxname != "" { stmt.idxname } else { "(unnamed)" };
    return #{
        operation: "INDEX without CONCURRENTLY: " + idx_name,
        problem: "Creating index '" + idx_name + "' without CONCURRENTLY blocks writes on the table.",
        safe_alternative: "Use CREATE INDEX CONCURRENTLY:\n  CREATE INDEX CONCURRENTLY " + idx_name + " ON ...;"
    };
}

// CONCURRENTLY cannot run inside a transaction block
if ctx.run_in_transaction {
    let idx_name = if stmt.idxname != "" { stmt.idxname } else { "(unnamed)" };
    let hint = if ctx.no_transaction_hint != "" {
        ctx.no_transaction_hint
    } else {
        "Run this migration outside a transaction block."
    };
    #{
        operation: "INDEX CONCURRENTLY inside a transaction: " + idx_name,
        problem: "CREATE INDEX CONCURRENTLY cannot run inside a transaction block. PostgreSQL will raise an error at runtime.",
        safe_alternative: hint
    }
}
