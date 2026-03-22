use crate::violation::{Severity, Violation};
use colored::Colorize;
use serde_json;
use std::fmt::Write;

pub struct OutputFormatter;

impl OutputFormatter {
    /// Format violations as colored text for terminal
    pub fn format_text(file_path: &str, violations: &[Violation]) -> String {
        let mut output = String::new();

        let has_errors = violations.iter().any(|v| v.severity == Severity::Error);
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

        for violation in violations {
            let (icon, label) = match violation.severity {
                Severity::Error => ("❌", violation.operation.as_str().red().bold()),
                Severity::Warning => ("⚠️ ", violation.operation.as_str().yellow().bold()),
            };

            write!(output, "{icon} {label}\n\n").unwrap();

            writeln!(output, "{}", "Problem:".white().bold()).unwrap();
            write!(output, "  {}\n\n", violation.problem).unwrap();

            writeln!(output, "{}", "Safe alternative:".green().bold()).unwrap();
            for line in violation.safe_alternative.lines() {
                writeln!(output, "  {line}").unwrap();
            }

            output.push('\n');
        }

        output
    }

    /// Format violations as JSON
    pub fn format_json(results: &[(String, Vec<Violation>)]) -> String {
        serde_json::to_string_pretty(results).unwrap_or_else(|_| "{}".into())
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
