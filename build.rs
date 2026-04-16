use std::fmt::Write as _;
use std::fs;
use std::io::{self, BufRead};
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=src/checks/");

    let checks_dir = Path::new("src/checks");
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("check_descriptions.rs");

    let skip = ["mod.rs", "pg_helpers.rs", "test_utils.rs"];

    let mut entries: Vec<(String, String)> = fs::read_dir(checks_dir)
        .expect("src/checks/ must exist")
        .filter_map(Result::ok)
        .filter(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.ends_with(".rs") && !skip.iter().any(|s| *s == name.as_ref())
        })
        .filter_map(|e| extract_check_info(&e.path()))
        .collect();

    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut output = String::from("pub static CHECK_DESCRIPTIONS: &[(&str, &str)] = &[\n");
    for (name, description) in &entries {
        let escaped = description.replace('\\', "\\\\").replace('"', "\\\"");
        writeln!(output, "    ({name:?}, \"{escaped}\"),").unwrap();
    }
    output.push_str("];\n");

    fs::write(&out_path, output).expect("failed to write check_descriptions.rs");
}

/// Extract (struct_name, description) from a check source file.
///
/// Struct name: first `pub struct XxxCheck;` line.
/// Description: first `//! ` line (without the `//! ` prefix).
fn extract_check_info(path: &Path) -> Option<(String, String)> {
    let file = fs::File::open(path).ok()?;
    let reader = io::BufReader::new(file);

    let mut struct_name: Option<String> = None;
    let mut description: Option<String> = None;

    for line in reader.lines().map_while(Result::ok) {
        if description.is_none() {
            if let Some(rest) = line.strip_prefix("//! ") {
                description = Some(rest.trim_end_matches('.').to_string());
                continue;
            }
            // Empty //! line — skip but don't stop
            if line == "//!" {
                continue;
            }
        }

        if struct_name.is_none()
            && let Some(rest) = line.strip_prefix("pub struct ")
            && let Some(name) = rest.strip_suffix(';')
        {
            struct_name = Some(name.to_string());
        }

        if struct_name.is_some() && description.is_some() {
            break;
        }
    }

    Some((struct_name?, description.unwrap_or_default()))
}
