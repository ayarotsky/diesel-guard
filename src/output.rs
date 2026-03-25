use crate::checks::Finding;
use crate::violation::Severity;
use colored::Colorize;
use serde::Serialize;
use serde_json;
use std::fmt::Write;

pub struct OutputFormatter;

/// Compute 1-indexed line and column from a byte offset in `sql`.
fn byte_offset_to_line_col(sql: &str, offset: usize) -> (u32, u32) {
    let offset = offset.min(sql.len());
    let prefix = &sql[..offset];
    let newlines = prefix.bytes().filter(|&b| b == b'\n').count();
    let line = u32::try_from(newlines)
        .unwrap_or(u32::MAX)
        .saturating_add(1);
    let col_start = prefix.rfind('\n').map_or(0, |i| i + 1);
    let col_bytes = prefix.len() - col_start;
    let col = u32::try_from(col_bytes)
        .unwrap_or(u32::MAX)
        .saturating_add(1);
    (line, col)
}

/// Read the SQL source for a file path, used to compute line numbers from spans.
/// Returns `None` for stdin (`-`) or if reading fails.
fn read_sql_for_path(file_path: &str) -> Option<String> {
    if file_path == "-" {
        return None;
    }
    std::fs::read_to_string(file_path).ok()
}

impl OutputFormatter {
    /// Format findings as colored text for terminal
    pub fn format_text(file_path: &str, findings: &[Finding]) -> String {
        let sql = read_sql_for_path(file_path);
        let mut output = String::new();

        let has_errors = findings.iter().any(|f| f.severity == Severity::Error);
        let header_icon = if has_errors { "❌" } else { "⚠️" };
        let header_label = if has_errors {
            "Unsafe migration detected in".red().bold()
        } else {
            "Migration warnings in".yellow().bold()
        };

        let display_path = if file_path == "-" {
            "<stdin>"
        } else {
            file_path
        };
        write!(
            output,
            "{} {} {}\n\n",
            header_icon,
            header_label,
            display_path.yellow()
        )
        .unwrap();

        for finding in findings {
            let (icon, label) = match finding.severity {
                Severity::Error => ("❌", finding.operation.as_str().red().bold()),
                Severity::Warning => ("⚠️ ", finding.operation.as_str().yellow().bold()),
            };

            if let Some(ref sql_src) = sql {
                let (line, _) = byte_offset_to_line_col(sql_src, finding.span.offset());
                write!(output, "{icon} {label} (line {line})\n\n").unwrap();
            } else {
                write!(output, "{icon} {label}\n\n").unwrap();
            }

            writeln!(output, "{}", "Problem:".white().bold()).unwrap();
            write!(output, "  {}\n\n", finding.problem).unwrap();

            writeln!(output, "{}", "Safe alternative:".green().bold()).unwrap();
            for line in finding.safe_alternative.lines() {
                writeln!(output, "  {line}").unwrap();
            }

            output.push('\n');
        }

        output
    }

    /// Format findings as JSON
    pub fn format_json(results: &[(String, Vec<Finding>)]) -> String {
        #[derive(Serialize)]
        struct JsonFinding<'a> {
            operation: &'a str,
            problem: &'a str,
            safe_alternative: &'a str,
            severity: Severity,
            line: Option<u32>,
        }

        #[derive(Serialize)]
        struct JsonFileResult<'a> {
            file: &'a str,
            findings: Vec<JsonFinding<'a>>,
        }

        let output: Vec<JsonFileResult<'_>> = results
            .iter()
            .map(|(path, findings)| {
                let sql = read_sql_for_path(path);
                JsonFileResult {
                    file: if path == "-" { "<stdin>" } else { path },
                    findings: findings
                        .iter()
                        .map(|f| {
                            let line = sql
                                .as_deref()
                                .map(|s| byte_offset_to_line_col(s, f.span.offset()).0);
                            JsonFinding {
                                operation: &f.operation,
                                problem: &f.problem,
                                safe_alternative: &f.safe_alternative,
                                severity: f.severity,
                                line,
                            }
                        })
                        .collect(),
                }
            })
            .collect();

        serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".into())
    }

    /// Format findings as GitHub Actions workflow commands for inline PR annotations.
    ///
    /// Emits `::error` / `::warning` commands that GitHub renders as annotations in
    /// the pull-request diff. Activated automatically when `GITHUB_ACTIONS=true` or
    /// via `--format github`.
    pub fn format_github(results: &[(String, Vec<Finding>)]) -> String {
        let mut output = String::new();

        for (file_path, findings) in results {
            let sql = read_sql_for_path(file_path);

            for finding in findings {
                let level = match finding.severity {
                    Severity::Error => "error",
                    Severity::Warning => "warning",
                };

                let (start_line, start_col, end_line, end_col) = if let Some(ref sql_src) = sql {
                    let (sl, sc) = byte_offset_to_line_col(sql_src, finding.span.offset());
                    let end_offset =
                        (finding.span.offset() + finding.span.len()).min(sql_src.len());
                    let (el, ec) = byte_offset_to_line_col(sql_src, end_offset);
                    (sl, sc, el, ec)
                } else {
                    (1, 1, 1, 1)
                };

                // Encode the message: newlines must be %0A per GitHub spec
                let message = format!(
                    "{}%0A%0A{}%0A%0ASafe alternative:%0A{}",
                    finding.problem,
                    finding.operation,
                    finding.safe_alternative.replace('\n', "%0A"),
                );
                let title = &finding.operation;

                writeln!(
                    output,
                    "::{level} file={file_path},line={start_line},endLine={end_line},col={start_col},endColumn={end_col},title={title}::{message}"
                )
                .unwrap();
            }
        }

        output
    }

    /// Format summary
    pub fn format_summary(total_errors: usize, total_warnings: usize) -> String {
        match (total_errors, total_warnings) {
            (0, 0) => format!("{}", "✅ No unsafe migrations detected!".green().bold()),
            (0, w) => format!(
                "\n{} {} migration warning(s) detected (not blocking)",
                "⚠️ ",
                w.to_string().yellow().bold()
            ),
            (e, 0) => format!(
                "\n{} {} unsafe migration(s) detected",
                "❌".red(),
                e.to_string().red().bold()
            ),
            (e, w) => format!(
                "\n{} {} unsafe migration(s) and {} warning(s) detected",
                "❌".red(),
                e.to_string().red().bold(),
                w.to_string().yellow().bold()
            ),
        }
    }
}
