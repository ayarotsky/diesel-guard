use crate::violation::{Severity, Violation};
use colored::*;
use serde_json;

pub struct OutputFormatter;

impl OutputFormatter {
    /// Format violations as colored text for terminal
    pub fn format_text(file_path: &str, violations: &[Violation]) -> String {
        let mut output = String::new();

        output.push_str(&format!(
            "{} {}\n\n",
            "❌ Unsafe migration detected in".red().bold(),
            file_path.yellow()
        ));

        for violation in violations {
            let severity_icon = match violation.severity {
                Severity::Error => "❌",
                Severity::Warning => "⚠️",
            };

            output.push_str(&format!(
                "{} {}\n\n",
                severity_icon,
                violation.operation.red().bold()
            ));

            output.push_str(&format!("{}\n", "Problem:".white().bold()));
            output.push_str(&format!("  {}\n\n", violation.problem));

            output.push_str(&format!("{}\n", "Safe alternative:".green().bold()));
            for line in violation.safe_alternative.lines() {
                output.push_str(&format!("  {}\n", line));
            }

            output.push_str("\n");
        }

        output
    }

    /// Format violations as JSON
    pub fn format_json(results: &[(String, Vec<Violation>)]) -> String {
        serde_json::to_string_pretty(results).unwrap_or_else(|_| "{}".to_string())
    }

    /// Print summary
    pub fn print_summary(total_violations: usize) {
        if total_violations == 0 {
            println!("{}", "✅ No unsafe migrations detected!".green().bold());
        } else {
            println!(
                "\n{} {} unsafe migration(s) detected",
                "❌".red(),
                total_violations.to_string().red().bold()
            );
        }
    }
}
