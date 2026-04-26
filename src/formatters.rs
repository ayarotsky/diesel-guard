use crate::ViolationList;
use crate::checks::Check;
use crate::config::Config;
use crate::violation::Severity;
use colored::Colorize;
use serde_json;
use std::fmt::Write;

pub trait Formatter {
    fn format_results(&self, results: &[(String, ViolationList)]) -> String;
    fn format_checks(&self, checks: &[&dyn Check], config: &Config) -> String;
    fn format_explain(&self, check: &dyn Check, config: &Config) -> String;
}

pub struct JsonFormatter;
pub struct TextFormatter;
pub struct GithubFormatter;

impl Formatter for JsonFormatter {
    fn format_results(&self, results: &[(String, ViolationList)]) -> String {
        let output: Vec<_> = results
            .iter()
            .map(|(file, violations)| {
                serde_json::json!({
                    "file": file,
                    "violations": violations.iter().map(|(line, v)| serde_json::json!({
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

    fn format_checks(&self, checks: &[&dyn Check], config: &Config) -> String {
        let json: Vec<_> = checks
            .iter()
            .map(|c| {
                let mut obj = serde_json::json!({
                    "name": c.name(),
                    "type": if c.script_path().is_some() { "custom" } else { "builtin" },
                    "severity": if config.is_check_warning(c.name()) { "warning" } else { "error" },
                    "enabled": config.is_check_enabled(c.name()),
                });
                if let Some(path) = c.script_path() {
                    obj["script_path"] = serde_json::Value::String(path.to_string());
                }
                obj
            })
            .collect();
        serde_json::to_string_pretty(&json).unwrap_or_else(|_| "[]".into())
    }

    fn format_explain(&self, check: &dyn Check, config: &Config) -> String {
        let mut obj = serde_json::json!({
            "name": check.name(),
            "type": if check.script_path().is_some() { "custom" } else { "builtin" },
            "severity": if config.is_check_warning(check.name()) { "warning" } else { "error" },
            "enabled": config.is_check_enabled(check.name()),
        });
        if let Some(path) = check.script_path() {
            obj["script_path"] = serde_json::Value::String(path.to_string());
        }
        let doc = check.doc().map(str::to_owned).or_else(|| check.describe());
        obj["doc"] = match doc {
            Some(text) => serde_json::Value::String(text),
            None => serde_json::Value::Null,
        };
        serde_json::to_string_pretty(&obj).unwrap_or_else(|_| "{}".into())
    }
}

impl Formatter for TextFormatter {
    fn format_results(&self, results: &[(String, ViolationList)]) -> String {
        if results.is_empty() {
            return format!("{}\n", "✅ No unsafe migrations detected!".green().bold());
        }
        let total_errors: usize = results
            .iter()
            .flat_map(|(_, v)| v)
            .filter(|(_, v)| v.severity == Severity::Error)
            .count();
        let total_warnings: usize = results
            .iter()
            .flat_map(|(_, v)| v)
            .filter(|(_, v)| v.severity == Severity::Warning)
            .count();
        let mut out = String::new();
        for (file_path, violations) in results {
            out.push_str(&format_file_violations(file_path, violations));
        }
        out.push_str(&format_summary(total_errors, total_warnings));
        out.push('\n');
        out
    }

    fn format_checks(&self, checks: &[&dyn Check], config: &Config) -> String {
        let mut out = String::new();
        writeln!(
            out,
            "{:<40} {:<10} {:<10} ENABLED",
            "NAME", "TYPE", "SEVERITY"
        )
        .unwrap();
        for c in checks {
            let check_type = if c.script_path().is_some() {
                "custom"
            } else {
                "builtin"
            };
            let severity = if config.is_check_warning(c.name()) {
                "warning"
            } else {
                "error"
            };
            let enabled = if config.is_check_enabled(c.name()) {
                "yes"
            } else {
                "no"
            };
            writeln!(
                out,
                "{:<40} {:<10} {:<10} {}",
                c.name(),
                check_type,
                severity,
                enabled
            )
            .unwrap();
        }
        out
    }

    fn format_explain(&self, check: &dyn Check, config: &Config) -> String {
        let check_type = if check.script_path().is_some() {
            "custom"
        } else {
            "builtin"
        };
        let severity = if config.is_check_warning(check.name()) {
            "warning"
        } else {
            "error"
        };
        let enabled_str = if config.is_check_enabled(check.name()) {
            "enabled"
        } else {
            "disabled"
        };
        let mut out = format!(
            "{} ({check_type} | {severity} | {enabled_str})\n\n",
            check.name()
        );
        match check.doc().map(str::to_owned).or_else(|| check.describe()) {
            Some(text) => out.push_str(&text),
            None if check.script_path().is_some() => {
                out.push_str("No description available. Add fn describe() to the script:\n");
                write!(out, "  {}", check.script_path().unwrap()).unwrap();
            }
            None => out.push_str("No documentation available.\n"),
        }
        out
    }
}

impl Formatter for GithubFormatter {
    fn format_results(&self, results: &[(String, ViolationList)]) -> String {
        let mut out = String::new();
        for (file_path, violations) in results {
            for (line, violation) in violations {
                let level = match violation.severity {
                    Severity::Error => "error",
                    Severity::Warning => "warning",
                };
                let raw = format!("{}: {}", violation.operation, violation.problem);
                let file = encode_property(file_path);
                let message = encode_data(&raw);
                writeln!(out, "::{level} file={file},line={line}::{message}").unwrap();
            }
        }
        out
    }

    fn format_checks(&self, checks: &[&dyn Check], config: &Config) -> String {
        TextFormatter.format_checks(checks, config)
    }

    fn format_explain(&self, check: &dyn Check, config: &Config) -> String {
        TextFormatter.format_explain(check, config)
    }
}

fn format_file_violations(file_path: &str, violations: &ViolationList) -> String {
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

fn format_summary(total_errors: usize, total_warnings: usize) -> String {
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

// Encodes a string for use as a workflow command property value (the `key=value`
// pairs before `::`). GitHub Actions requires `%`, `\r`, `\n`, `:`, and `,` to
// be percent-encoded so they don't break the `::cmd key=val,key=val::data` wire
// format.
// See https://docs.github.com/en/actions/writing-workflows/choosing-what-its-workflow-does/workflow-commands-for-github-actions#using-workflow-commands-to-access-toolkit-functions
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
