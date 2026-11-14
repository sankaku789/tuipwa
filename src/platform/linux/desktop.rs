//! Read existing desktop declarations without executing their commands.
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

pub(crate) fn entries(data_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut entries = Vec::new();
    for dir in data_dirs {
        let Ok(read) = fs::read_dir(dir.join("applications")) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = read.flatten().map(|e| e.path()).collect();
        paths.sort();
        entries.extend(
            paths
                .into_iter()
                .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("desktop")),
        );
    }
    entries
}

/// First usable declaration in XDG search order. Hidden entries mask lower
/// priority entries with the same desktop filename, including their MIME data.
pub(crate) fn find_mime_types(command: &str, data_dirs: &[PathBuf]) -> Vec<String> {
    let mut seen = HashSet::new();
    for path in entries(data_dirs) {
        if !seen.insert(path.file_name().unwrap().to_owned()) {
            continue;
        }
        let Ok(content) = fs::read_to_string(path) else {
            continue;
        };
        let mut in_entry = false;
        let (mut kind, mut exec, mut mime_types) = (None, None, None);
        let (mut hidden, mut tuipwa) = (false, false);
        for line in content.lines().map(str::trim) {
            if line.starts_with('[') {
                in_entry = line == "[Desktop Entry]";
                continue;
            }
            if !in_entry {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "Type" => kind = Some(value),
                "Exec" => exec = Some(value),
                "MimeType" => mime_types = Some(value),
                "Hidden" => hidden = value == "true",
                "X-TuiPWA-Id" => tuipwa = true,
                _ => {}
            }
        }
        if hidden
            || tuipwa
            || kind != Some("Application")
            || !exec.is_some_and(|exec| matches_command(exec, command))
        {
            continue;
        }
        let Some(mime_types) = mime_types else {
            continue;
        };
        let values: Vec<String> = mime_types
            .split(';')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        if let Ok(types) = crate::meta::normalize_mime_types(&values)
            && !types.is_empty()
        {
            return types;
        }
    }
    Vec::new()
}

fn matches_command(exec: &str, command: &str) -> bool {
    // Recognize plain and simply quoted executable paths. Escaped paths and
    // wrappers such as `env ...` are deliberately left to explicit --mime-type.
    let program = if let Some(quoted) = exec.strip_prefix('"') {
        let Some((program, rest)) = quoted.split_once('"') else {
            return false;
        };
        if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
            return false;
        }
        program
    } else {
        exec.split_whitespace().next().unwrap_or("")
    };
    if program.is_empty() || program.contains(['\\', '%', '"', '\'', '`', '$']) {
        return false;
    }
    let path = Path::new(program);
    if path.is_absolute() {
        path == Path::new(command)
            || fs::canonicalize(path)
                .ok()
                .zip(fs::canonicalize(command).ok())
                .is_some_and(|(left, right)| left == right)
    } else {
        !program.contains('/')
            && Path::new(command)
                .file_name()
                .is_some_and(|name| name == program)
    }
}
