use crate::checks::{Check, CheckDoc};
use camino::Utf8Path;
use rhai::{AST, Dynamic, Engine};
use std::path::PathBuf;
use std::sync::Arc;

mod discovery;
mod engine;
mod eval;
mod load;
mod result;
mod scope;

use discovery::discover_custom_check_files;
#[cfg(test)]
use discovery::{
    custom_check_file_type_is_regular, process_custom_check_dir_entry, readable_custom_check_entry,
};
#[cfg(test)]
use engine::create_engine;
pub use load::load_custom_checks;
#[cfg(test)]
use result::parse_script_result;

pub const MAX_CUSTOM_CHECK_SOURCE_BYTES: u64 = 64 * 1024;
pub const MAX_CUSTOM_CHECK_FILES: usize = 512;
pub const MAX_CUSTOM_CHECK_DIR_ENTRIES: usize = 2_048;
pub const MAX_CUSTOM_CHECK_TOTAL_SOURCE_BYTES: u64 = 2 * 1024 * 1024;

struct CustomCheckFile {
    path: PathBuf,
    stem: String,
}

struct ScriptInputs {
    node: Dynamic,
    config: Dynamic,
    ctx: Dynamic,
}

struct CustomCheckLoadState {
    checks: Vec<Box<dyn Check>>,
    errors: Vec<ScriptError>,
    total_source_bytes: u64,
}

enum ScriptSource {
    Source(String, u64),
    TooLarge,
}

/// Error encountered while loading or running a custom Rhai check script.
#[derive(thiserror::Error, Debug)]
#[error("{file}: {message}")]
pub struct ScriptError {
    pub file: String,
    pub message: String,
}

/// A custom check backed by a compiled Rhai script.
pub struct CustomCheck {
    name: &'static str,
    engine: Arc<Engine>,
    ast: AST,
    path: String,
}

/// Return custom check names discovered from `.rhai` files in a directory.
pub fn custom_check_names(dir: &Utf8Path) -> Vec<String> {
    let (files, _errors) = discover_custom_check_files(dir);
    files.into_iter().map(|file| file.stem).collect()
}

impl CheckDoc for CustomCheck {}

#[cfg(test)]
mod tests;
