# Introduction

Linter for dangerous Postgres migration patterns in Diesel and SQLx. Prevents downtime caused by unsafe schema changes.

✓ Detects operations that lock tables or cause downtime<br>
✓ Provides safe alternatives for each blocking operation<br>
✓ Supports [safety-assured blocks](safety-assured.md) for verified operations<br>
✓ Extensible with [custom checks](custom-checks.md)<br>
✓ Works standalone or with any [AI agent](agents.md) (Claude, Codex, Copilot, Gemini)<br>

> ⭐ If this looks useful, a [star on GitHub](https://github.com/ayarotsky/diesel-guard) helps more developers find it.

## How It Works

diesel-guard analyzes your migration SQL using Postgres's own parser (`pg_query` via libpg_query) and checks each statement against a set of safety rules. When it finds a potentially dangerous operation, it reports:

- **The operation** — what SQL was detected
- **The problem** — what lock it acquires, why it's dangerous, and under what conditions
- **A safe alternative** — a step-by-step approach that achieves the same goal without the risk
