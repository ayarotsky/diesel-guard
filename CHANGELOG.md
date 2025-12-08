# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.2.0 - 2025-12-07

### Added

- `CreateExtension` check - Detects `CREATE EXTENSION` without `IF NOT EXISTS` clause
- `UnnamedConstraint` check - Detects unnamed constraints (`CHECK`, `UNIQUE`, `FOREIGN KEY`)
- `RenameColumn` check - Detects `ALTER TABLE ... RENAME COLUMN` operations
- `AddSerialColumn` check - Detects adding `SERIAL` or `BIGSERIAL` columns to existing tables
- `RenameTable` check - Detects `ALTER TABLE ... RENAME TO` operations

### Removed

- `allow_unsafe` configuration option (the CLI flag `--allow-unsafe` remains available)

### Changed

- Refactoring and code improvements after v0.1 release
- CI dependency updates (actions/checkout 4→6, softprops/action-gh-release 1→2)

## 0.1.1 - 2025-12-05

### Added

- Support for multiple timestamp formats in `start_after` configuration:
  - `YYYYMMDDHHMMSS` (no separators)
  - `YYYY_MM_DD_HHMMSS` (underscore separators)
  - `YYYY-MM-DD-HHMMSS` (dash separators)
- Migration directories are now checked in alphanumeric order for deterministic results

### Fixed

- Fixed safety-assured blocks being ignored when SQL keywords appear within identifiers (e.g., `CREATE` in `CREATED_AT`)
- Statement line tracking now correctly matches whole keywords instead of prefixes

## 0.1.0 - 2025-12-05

### Added

- Initial release of diesel-guard
- Detection of unsafe PostgreSQL migration operations:
  - ADD COLUMN with DEFAULT value
  - CREATE INDEX without CONCURRENTLY
  - ALTER COLUMN TYPE changes
  - DROP COLUMN operations
  - ALTER COLUMN SET NOT NULL constraints
- Safe alternative suggestions for each detected unsafe operation
- CLI commands:
  - `check` - Analyze migration files for unsafe operations
  - `init` - Generate configuration file template
- Configuration file support (`diesel-guard.toml`):
  - `start_after` - Skip migrations before timestamp
  - `check_down` - Toggle checking down.sql files
  - `disable_checks` - Disable specific safety checks
- Safety-assured comment blocks to bypass checks for verified operations
- Multiple output formats:
  - Human-readable colored output (default)
  - JSON output for CI/CD integration
- `--allow-unsafe` flag to report without failing
- Support for single file or directory scanning
- Detailed error messages with file location and line numbers
