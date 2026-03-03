# Introduction

Catch dangerous Postgres migrations before they take down production.

✓ Detects operations that lock tables or cause downtime<br>
✓ Provides safe alternatives for each blocking operation<br>
✓ Works with both Diesel and SQLx migration frameworks<br>
✓ Supports safety-assured blocks for verified operations<br>
✓ Extensible with [custom checks](custom-checks.md)<br>

Works with [Diesel](https://diesel.rs) and [SQLx](https://github.com/launchbadge/sqlx).

## How It Works

diesel-guard analyzes your migration SQL using Postgres's own parser (`pg_query` via libpg_query) and checks each statement against a set of safety rules. When it finds a potentially dangerous operation, it reports:

- **The operation** — what SQL was detected
- **The problem** — what lock it acquires, why it's dangerous, and under what conditions
- **A safe alternative** — a step-by-step approach that achieves the same goal without the risk
