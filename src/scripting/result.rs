use crate::violation::Violation;
use rhai::Dynamic;

/// Parse the return value of a Rhai script into violations.
///
/// Accepted return types:
/// - `()` — no violation
/// - `#{ operation: "...", problem: "...", safe_alternative: "..." }` — one violation
/// - Array of maps — multiple violations
pub(super) fn parse_script_result(check_name: &str, result: Dynamic) -> Vec<Violation> {
    if result.is_unit() {
        return vec![];
    }

    if result.is_map() {
        return vec![map_to_violation(check_name, result)];
    }

    if result.is_array() {
        return result
            .into_array()
            .unwrap_or_default()
            .into_iter()
            .map(|value| map_to_violation(check_name, value))
            .collect();
    }

    vec![script_error_violation(
        check_name,
        format!(
            "Custom check returned {}, expected (), map, or array",
            result.type_name()
        ),
        "Fix the custom check script to return a valid type.",
    )]
}

/// Convert a Rhai map Dynamic to a Violation.
fn map_to_violation(check_name: &str, value: Dynamic) -> Violation {
    let type_name = value.type_name().to_owned();
    let Some(map) = value.try_cast::<rhai::Map>() else {
        return script_error_violation(
            check_name,
            format!(
                "Custom check returned a non-map array element: expected a map, got {type_name}"
            ),
            "Fix the custom check script to return maps with operation, problem, and safe_alternative keys.",
        );
    };

    let mut issues = Vec::new();
    let operation = required_string_field(&map, "operation", &mut issues);
    let problem = required_string_field(&map, "problem", &mut issues);
    let safe_alternative = required_string_field(&map, "safe_alternative", &mut issues);

    match (operation, problem, safe_alternative) {
        (Some(operation), Some(problem), Some(safe_alternative)) => {
            Violation::new(operation, problem, safe_alternative)
        }
        _ => script_error_violation(
            check_name,
            format!(
                "Custom check returned an invalid map: {}",
                issues.join(", ")
            ),
            "Fix the custom check script to return all three required string keys.",
        ),
    }
}

/// Read a required string field from a Rhai map and record why it is invalid.
fn required_string_field(map: &rhai::Map, key: &str, issues: &mut Vec<String>) -> Option<String> {
    match map.get(key) {
        None => {
            issues.push(format!("'{key}' is missing"));
            None
        }
        Some(value) => {
            if let Ok(value) = value.clone().into_string() {
                Some(value)
            } else {
                issues.push(format!(
                    "'{key}' must be a string (got {})",
                    value.type_name()
                ));
                None
            }
        }
    }
}

fn script_error_violation(check_name: &str, problem: String, safe_alternative: &str) -> Violation {
    Violation::new(
        format!("SCRIPT ERROR: {check_name}"),
        problem,
        safe_alternative,
    )
}
