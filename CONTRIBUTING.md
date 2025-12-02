# Contributing to diesel-guard

Thank you for your interest in contributing to **diesel-guard**! We appreciate your help in making Diesel migrations safer for everyone.

## Getting Help

If you need help using **diesel-guard**, please open a [GitHub Discussion](https://github.com/ayarotsky/diesel-guard/discussions) or search Stack Overflow. Undocumented features are generally unsupported.

## Reporting Bugs

Before reporting a bug:
1. Search [existing issues](https://github.com/ayarotsky/diesel-guard/issues) to see if it's already been reported
2. Test against the latest version from the main branch
3. Verify the issue is reproducible

When reporting a bug, please include:
- **Detailed steps to reproduce** - Include the exact SQL statement or migration file
- **Expected vs actual behavior** - What should happen and what actually happens
- **Version information** - Output of `diesel-guard --version` and `rustc --version`
- **Complete error messages** - Include full backtraces for panics or errors

## Suggesting New Checks

We welcome suggestions for new migration safety checks! Before proposing:
1. Check the [Coming Soon section](README.md#coming-soon-phase-2) to see if it's already planned
2. Review `AGENTS.md` to understand the check implementation pattern

When suggesting a new check, please include:
- **The unsafe operation** - What migration pattern should be detected
- **Why it's dangerous** - Lock type, blocking behavior, or data integrity issue
- **Safe alternative** - How developers should do this safely
- **PostgreSQL version specifics** - If behavior differs across versions

Create an issue with `[Check]` in the title, for example: `[Check] REFRESH MATERIALIZED VIEW without CONCURRENTLY`

## Pull Requests

We love pull requests! Here's how to contribute code:

### Before You Start

1. **Open an issue first** for significant changes to discuss the approach
2. **Check AGENTS.md** for implementation patterns and conventions
3. **One check per PR** - Makes review easier and faster

### Development Setup

```bash
# Clone the repository
git clone https://github.com/ayarotsky/diesel-guard.git
cd diesel-guard

# Run tests
cargo test

# Check code quality
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

### Testing

The project has comprehensive test coverage with both unit and integration tests.

#### Run All Tests

```bash
cargo test
```

This runs:
- **Unit tests** - Individual check modules, parser, and safety checker
- **Integration tests** - Fixture files are automatically verified

#### Run Specific Test Suites

```bash
# Run only unit tests (in src/)
cargo test --lib

# Run only integration tests (fixtures)
cargo test --test fixtures_test

# Run tests for a specific check
cargo test add_column
cargo test add_index
cargo test drop_column
```

#### Test Structure

**Unit Tests** (`src/checks/*.rs`):
- Each check module has its own test suite
- Uses shared test utilities from `src/checks/test_utils.rs`
- Tests individual SQL statement parsing and violation detection
- Prefer using `assert_detects_violation!` and `assert_allows!` macros

**Integration Tests** (`tests/fixtures_test.rs`):
- Automatically verifies all fixture files behave correctly
- Tests both safe and unsafe migrations
- Validates directory-level scanning

### Adding a New Check

See `AGENTS.md` for detailed step-by-step instructions. Summary:

1. Create `src/checks/your_check.rs` with check implementation
2. Add unit tests using shared macros
3. Register in `src/checks/mod.rs`
4. Create test fixtures in `tests/fixtures/`
5. Add integration tests in `tests/fixtures_test.rs`
6. Update README.md to document the check
7. Ensure all tests pass and code quality checks pass

### Code Style

- **Follow existing patterns** - Look at similar checks for guidance
- **Use descriptive names** - `AddNotNullCheck` not `NotNullCheck`
- **Keep it simple** - Don't over-engineer, solve the specific problem
- **Document clearly** - Module-level docs (//!) explaining the check
- **Be accurate** - Specify exact lock types (ACCESS EXCLUSIVE, SHARE, etc.)
- **No exaggeration** - Say "depends on table size" not "takes hours"

### Commit Messages

- Use clear, descriptive commit messages
- Reference issue numbers when applicable
- Examples:
  - `Add FOREIGN KEY constraint check (#42)`
  - `Fix false positive in ALTER TYPE detection`
  - `Update README with REINDEX check`

### Code Quality

Before submitting your PR:

```bash
# Format code
cargo fmt

# Run linter (no warnings allowed)
cargo clippy --all-targets --all-features -- -D warnings

# Run all tests
cargo test

# Build the project
cargo build --release
```

All of these must pass for your PR to be merged.

### What Makes a Good PR

- ✅ **Minimal changes** - Only what's necessary for the feature/fix
- ✅ **Tests included** - Both unit and integration tests
- ✅ **Documentation updated** - README.md and code comments
- ✅ **Follows conventions** - Matches existing code patterns
- ✅ **Clean commit history** - Logical, well-described commits
- ✅ **No clippy warnings** - All linting passes
- ✅ **Formatted code** - `cargo fmt` applied

### What to Avoid

- ❌ Large refactoring mixed with new features
- ❌ Changing unrelated code
- ❌ Multiple checks in one PR
- ❌ Missing tests or documentation
- ❌ Bypassing code quality checks

## Working with AI Assistants

If you're using an AI coding assistant (like Claude or GitHub Copilot), please refer them to:
- **AGENTS.md** - Contains detailed implementation patterns and common pitfalls
- **Existing checks** - Use as reference examples for structure and style

This helps maintain consistency across contributions.

## PostgreSQL Version Testing

When implementing checks that behave differently across PostgreSQL versions:
- Document version-specific behavior in check comments
- Include version caveats in violation messages
- Test against multiple PostgreSQL versions if possible

Currently, we target PostgreSQL 9.6+ to match Diesel's requirements.

## Questions?

Feel free to:
- Open a [GitHub Discussion](https://github.com/ayarotsky/diesel-guard/discussions)
- Comment on relevant issues
- Ask for clarification in your PR

We're here to help!

---

This guide is released under [CC0](https://creativecommons.org/publicdomain/zero/1.0/) (public domain). Use it for your own projects!
