use crate::ViolationList;
use crate::violation::Severity;
use colored::Colorize;
use serde_json;
use std::fmt::Write;

pub struct OutputFormatter;

impl OutputFormatter {
    /// Format violations as colored text for terminal
    pub fn format_text(file_path: &str, violations: &ViolationList) -> String {
        let mut output = String::new();

        let has_errors = violations
            .iter()
            .any(|(_, v)| v.severity == Severity::Error);
        let header_icon = if has_errors { "❌" } else { "⚠️" };
        let header_label = if has_errors {
            "Unsafe migration detected in".red().bold()
        } else {
            "Migration warnings in".yellow().bold()
        };

        write!(
            output,
            "{} {} {}\n\n",
            header_icon,
            header_label,
            file_path.yellow()
        )
        .unwrap();

        for (line, violation) in violations {
            let (icon, label) = match violation.severity {
                Severity::Error => ("❌", violation.operation.as_str().red().bold()),
                Severity::Warning => ("⚠️ ", violation.operation.as_str().yellow().bold()),
            };

            write!(
                output,
                "{icon} {label}  {}\n\n",
                format!("(line {line})").dimmed()
            )
            .unwrap();

            writeln!(output, "{}", "Problem:".white().bold()).unwrap();
            write!(output, "  {}\n\n", violation.problem).unwrap();

            writeln!(output, "{}", "Safe alternative:".green().bold()).unwrap();
            for safe_line in violation.safe_alternative.lines() {
                writeln!(output, "  {safe_line}").unwrap();
            }

            output.push('\n');
        }

        output
    }

    /// Format violations as JSON
    pub fn format_json(results: &[(String, ViolationList)]) -> String {
        use serde_json::{Value, json};
        let output: Vec<Value> = results
            .iter()
            .map(|(file, violations)| {
                json!({
                    "file": file,
                    "violations": violations.iter().map(|(line, v)| json!({
                        "line": line,
                        "check_name": v.check_name,
                        "operation": v.operation,
                        "problem": v.problem,
                        "safe_alternative": v.safe_alternative,
                        "severity": v.severity,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();
        serde_json::to_string_pretty(&output).unwrap_or_else(|_| "[]".into())
    }

    /// Format violations as GitHub Actions workflow commands.
    ///
    /// Produces `::error` / `::warning` annotations that GitHub renders inline
    /// on the diff when run inside a GitHub Actions workflow.
    pub fn format_github(file_path: &str, violations: &ViolationList) -> String {
        let mut output = String::new();
        for (line, violation) in violations {
            let level = match violation.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
            };
            let raw = format!("{}: {}", violation.operation, violation.problem);
            let file = encode_property(file_path);
            let message = encode_data(&raw);
            writeln!(output, "::{level} file={file},line={line}::{message}").unwrap();
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

// Encodes a string for use as a workflow command property value (the `key=value`
// pairs before `::`). GitHub Actions requires `%`, `\r`, `\n`, `:`, and `,` to
// be percent-encoded so they don't break the `::cmd key=val,key=val::data` wire
// format.
// See https://docs.github.com/en/actions/writing-workflows/choosing-what-your-workflow-does/workflow-commands-for-github-actions#using-workflow-commands-to-access-toolkit-functions
fn encode_property(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '%' => out.push_str("%25"),
            '\r' => out.push_str("%0D"),
            '\n' => out.push_str("%0A"),
            ':' => out.push_str("%3A"),
            ',' => out.push_str("%2C"),
            _ => out.push(c),
        }
    }
    out
}

// Encodes a string for use as workflow command data (the part after `::`).
// Only `%`, `\r`, and `\n` need escaping here — `:` and `,` are allowed in
// the data section and do not require encoding.
fn encode_data(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '%' => out.push_str("%25"),
            '\r' => out.push_str("%0D"),
            '\n' => out.push_str("%0A"),
            _ => out.push(c),
        }
    }
    out
}
