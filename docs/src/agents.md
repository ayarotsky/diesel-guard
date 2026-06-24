# AI Agents

diesel-guard ships an instruction file at `skills/diesel-guard/SKILL.md` that teaches an AI agent to run it and fix migrations for you. Point your agent at it.

## What you can ask

Once the agent has the instructions, ask it things like:

* "Check my migrations and fix anything unsafe."
* "Tell me whether this migration is safe to ship."
* "Explain why AddIndexCheck fired and how to fix it."
* "Write a custom check that bans X."

The agent runs `diesel-guard`, reads each finding's safe alternative, and rewrites the migration until it passes.
