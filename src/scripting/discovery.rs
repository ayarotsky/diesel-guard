use super::{
    CustomCheckFile, MAX_CUSTOM_CHECK_DIR_ENTRIES, MAX_CUSTOM_CHECK_FILES,
    MAX_CUSTOM_CHECK_SOURCE_BYTES, ScriptError, ScriptSource,
};
use camino::Utf8Path;
use std::io::Read;

/// Discover loadable `.rhai` custom check files in a directory.
pub(super) fn discover_custom_check_files(
    dir: &Utf8Path,
) -> (Vec<CustomCheckFile>, Vec<ScriptError>) {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(read_dir) => read_dir,
        Err(e) => {
            return (
                Vec::new(),
                vec![ScriptError {
                    file: dir.to_string(),
                    message: format!("Failed to read directory: {e}"),
                }],
            );
        }
    };
    collect_custom_check_files(dir, read_dir)
}

/// Collect and sort custom check files from an open directory iterator.
pub(super) fn collect_custom_check_files(
    dir: &Utf8Path,
    read_dir: std::fs::ReadDir,
) -> (Vec<CustomCheckFile>, Vec<ScriptError>) {
    let mut files = Vec::new();
    let mut errors = Vec::new();
    for (index, entry) in read_dir.enumerate() {
        if !process_custom_check_dir_entry(dir, index, entry, &mut files, &mut errors) {
            break;
        }
    }

    sort_custom_check_files(&mut files);
    (files, errors)
}

/// Process one directory entry and return whether scanning should continue.
pub(super) fn process_custom_check_dir_entry(
    dir: &Utf8Path,
    index: usize,
    entry: std::io::Result<std::fs::DirEntry>,
    files: &mut Vec<CustomCheckFile>,
    errors: &mut Vec<ScriptError>,
) -> bool {
    if !custom_check_entry_limit_allows(dir, index, errors) {
        return false;
    }

    let Some(file) = custom_check_file_from_entry(dir, entry, errors) else {
        return true;
    };
    push_custom_check_file(dir, file, files, errors)
}

/// Sort custom check files by filename for deterministic loading.
pub(super) fn sort_custom_check_files(files: &mut [CustomCheckFile]) {
    files.sort_by(|left, right| left.path.file_name().cmp(&right.path.file_name()));
}

/// Enforce the maximum number of directory entries inspected.
pub(super) fn custom_check_entry_limit_allows(
    dir: &Utf8Path,
    index: usize,
    errors: &mut Vec<ScriptError>,
) -> bool {
    if index < MAX_CUSTOM_CHECK_DIR_ENTRIES {
        return true;
    }

    errors.push(ScriptError {
        file: dir.to_string(),
        message: format!(
            "Custom checks directory has more than {MAX_CUSTOM_CHECK_DIR_ENTRIES} entries"
        ),
    });
    false
}

/// Convert a readable directory entry into a candidate custom check file.
pub(super) fn custom_check_file_from_entry(
    dir: &Utf8Path,
    entry: std::io::Result<std::fs::DirEntry>,
    errors: &mut Vec<ScriptError>,
) -> Option<CustomCheckFile> {
    let entry = readable_custom_check_entry(dir, entry, errors)?;
    let path = rhai_entry_path(&entry)?;
    if !custom_check_entry_is_regular_file(&entry, &path, errors) {
        return None;
    }
    Some(custom_check_file(path))
}

/// Return a directory entry or record the entry-read error.
pub(super) fn readable_custom_check_entry(
    dir: &Utf8Path,
    entry: std::io::Result<std::fs::DirEntry>,
    errors: &mut Vec<ScriptError>,
) -> Option<std::fs::DirEntry> {
    match entry {
        Ok(entry) => Some(entry),
        Err(e) => {
            errors.push(ScriptError {
                file: dir.to_string(),
                message: format!("Failed to read directory entry: {e}"),
            });
            None
        }
    }
}

/// Return the path for entries with a `.rhai` extension.
pub(super) fn rhai_entry_path(entry: &std::fs::DirEntry) -> Option<std::path::PathBuf> {
    let path = entry.path();
    path.extension()
        .is_some_and(|ext| ext == "rhai")
        .then_some(path)
}

/// Verify that a `.rhai` entry is a regular file.
pub(super) fn custom_check_entry_is_regular_file(
    entry: &std::fs::DirEntry,
    path: &std::path::Path,
    errors: &mut Vec<ScriptError>,
) -> bool {
    custom_check_file_type_is_regular(entry.file_type(), path, errors)
}

/// Verify that a resolved `.rhai` entry type is a regular file.
pub(super) fn custom_check_file_type_is_regular(
    file_type: std::io::Result<std::fs::FileType>,
    path: &std::path::Path,
    errors: &mut Vec<ScriptError>,
) -> bool {
    let file_type = match file_type {
        Ok(file_type) => file_type,
        Err(e) => {
            errors.push(ScriptError {
                file: path.display().to_string(),
                message: format!("Failed to inspect file type: {e}"),
            });
            return false;
        }
    };
    if file_type.is_file() {
        return true;
    }

    errors.push(ScriptError {
        file: path.display().to_string(),
        message: "Custom check path is not a regular file".to_string(),
    });
    false
}

/// Build custom check file metadata from a filesystem path.
pub(super) fn custom_check_file(path: std::path::PathBuf) -> CustomCheckFile {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();
    CustomCheckFile { path, stem }
}

/// Append a custom check file while enforcing the file count limit.
pub(super) fn push_custom_check_file(
    dir: &Utf8Path,
    file: CustomCheckFile,
    files: &mut Vec<CustomCheckFile>,
    errors: &mut Vec<ScriptError>,
) -> bool {
    if files.len() >= MAX_CUSTOM_CHECK_FILES {
        errors.push(ScriptError {
            file: dir.to_string(),
            message: format!(
                "Custom checks directory has more than {MAX_CUSTOM_CHECK_FILES} .rhai files"
            ),
        });
        return false;
    }
    files.push(file);
    true
}

/// Read a custom check script while enforcing the per-script source size limit.
pub(super) fn read_script_source(path: &std::path::Path) -> std::io::Result<ScriptSource> {
    let file = std::fs::File::open(path)?;
    let mut reader = file.take(MAX_CUSTOM_CHECK_SOURCE_BYTES.saturating_add(1));
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    let bytes_read = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if bytes_read > MAX_CUSTOM_CHECK_SOURCE_BYTES {
        return Ok(ScriptSource::TooLarge);
    }
    String::from_utf8(bytes)
        .map(|source| ScriptSource::Source(source, bytes_read))
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
}
