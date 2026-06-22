use super::{
    CustomCheck, CustomCheckFile, CustomCheckLoadState, MAX_CUSTOM_CHECK_SOURCE_BYTES,
    MAX_CUSTOM_CHECK_TOTAL_SOURCE_BYTES, ScriptError, ScriptSource,
    discovery::{discover_custom_check_files, read_script_source},
    engine::create_engine,
};
use crate::checks::Check;
use crate::config::Config;
use camino::Utf8Path;
use rhai::{AST, Engine};
use std::sync::Arc;

/// Load all `.rhai` files from a directory and compile them into custom checks.
///
/// Returns successfully compiled checks and any errors encountered.
/// Compilation errors are non-fatal — they're collected as `ScriptError`s.
pub fn load_custom_checks(
    dir: &Utf8Path,
    config: &crate::config::Config,
) -> (Vec<Box<dyn Check>>, Vec<ScriptError>) {
    let engine = Arc::new(create_engine());
    let (entries, errors) = discover_custom_check_files(dir);
    let state = load_custom_check_entries(dir, config, &engine, entries, errors);
    (state.checks, state.errors)
}

pub(super) fn load_custom_check_entries(
    dir: &Utf8Path,
    config: &Config,
    engine: &Arc<Engine>,
    entries: Vec<CustomCheckFile>,
    errors: Vec<ScriptError>,
) -> CustomCheckLoadState {
    let mut state = CustomCheckLoadState {
        checks: Vec::new(),
        errors,
        total_source_bytes: 0,
    };
    for entry in entries {
        if !load_custom_check_entry(dir, config, engine, entry, &mut state) {
            break;
        }
    }
    state
}

pub(super) fn load_custom_check_entry(
    dir: &Utf8Path,
    config: &Config,
    engine: &Arc<Engine>,
    entry: CustomCheckFile,
    state: &mut CustomCheckLoadState,
) -> bool {
    if !config.is_check_enabled(&entry.stem) {
        return true;
    }

    let Some(source) = script_source_for_entry(dir, &entry, state) else {
        return state.total_source_bytes <= MAX_CUSTOM_CHECK_TOTAL_SOURCE_BYTES;
    };
    compile_custom_check(engine, entry, &source, &mut state.checks, &mut state.errors);
    true
}

pub(super) fn script_source_for_entry(
    dir: &Utf8Path,
    entry: &CustomCheckFile,
    state: &mut CustomCheckLoadState,
) -> Option<String> {
    match read_script_source(&entry.path) {
        Ok(ScriptSource::Source(source, bytes_read)) => {
            add_script_source_bytes(dir, bytes_read, state).then_some(source)
        }
        Ok(ScriptSource::TooLarge) => {
            push_oversized_script_error(entry, &mut state.errors);
            None
        }
        Err(error) => {
            push_script_read_error(entry, &error, &mut state.errors);
            None
        }
    }
}

pub(super) fn add_script_source_bytes(
    dir: &Utf8Path,
    bytes_read: u64,
    state: &mut CustomCheckLoadState,
) -> bool {
    let next_total = state.total_source_bytes.saturating_add(bytes_read);
    if next_total > MAX_CUSTOM_CHECK_TOTAL_SOURCE_BYTES {
        push_total_script_size_error(dir, &mut state.errors);
        state.total_source_bytes = next_total;
        return false;
    }
    state.total_source_bytes = next_total;
    true
}

pub(super) fn push_total_script_size_error(dir: &Utf8Path, errors: &mut Vec<ScriptError>) {
    errors.push(ScriptError {
        file: dir.to_string(),
        message: format!(
            "Custom check scripts are larger than {MAX_CUSTOM_CHECK_TOTAL_SOURCE_BYTES} bytes in total"
        ),
    });
}

pub(super) fn push_oversized_script_error(entry: &CustomCheckFile, errors: &mut Vec<ScriptError>) {
    errors.push(ScriptError {
        file: entry.path.display().to_string(),
        message: format!(
            "Custom check script is larger than {MAX_CUSTOM_CHECK_SOURCE_BYTES} bytes"
        ),
    });
}

pub(super) fn push_script_read_error(
    entry: &CustomCheckFile,
    error: &std::io::Error,
    errors: &mut Vec<ScriptError>,
) {
    errors.push(ScriptError {
        file: entry.path.display().to_string(),
        message: format!("Failed to read: {error}"),
    });
}

pub(super) fn compile_custom_check(
    engine: &Arc<Engine>,
    entry: CustomCheckFile,
    source: &str,
    checks: &mut Vec<Box<dyn Check>>,
    errors: &mut Vec<ScriptError>,
) {
    match engine.compile(source) {
        Ok(ast) => push_compiled_custom_check(engine, entry, ast, checks),
        Err(error) => push_script_compile_error(&entry, &error, errors),
    }
}

pub(super) fn push_compiled_custom_check(
    engine: &Arc<Engine>,
    entry: CustomCheckFile,
    ast: AST,
    checks: &mut Vec<Box<dyn Check>>,
) {
    // Match the pre-split behavior: check names live for the process lifetime.
    let name: &'static str = Box::leak(entry.stem.into_boxed_str());
    checks.push(Box::new(CustomCheck {
        name,
        engine: Arc::clone(engine),
        ast,
        path: entry.path.display().to_string(),
    }));
}

pub(super) fn push_script_compile_error(
    entry: &CustomCheckFile,
    error: &rhai::ParseError,
    errors: &mut Vec<ScriptError>,
) {
    errors.push(ScriptError {
        file: entry.path.display().to_string(),
        message: format!("Compilation error: {error}"),
    });
}
