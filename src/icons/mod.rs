mod font;
pub mod png;
pub mod render;

use self::render::{self as icon, Rgb};
use crate::{
    Result,
    meta::{IconMeta, IconMode},
    model::Manifest,
    registry::Registry,
};
use std::path::{Path, PathBuf};

/// A resolved icon decision for one registration.
#[derive(Debug, Clone)]
pub struct IconDecision {
    pub mode: IconMode,
    /// Background color for generated icons.
    pub color: Option<Rgb>,
    /// The file the registration points at.
    pub path: Option<PathBuf>,
}

impl IconDecision {
    pub fn explicit(path: PathBuf) -> Self {
        Self {
            mode: IconMode::Explicit,
            color: None,
            path: Some(path),
        }
    }
    pub fn system(path: PathBuf) -> Self {
        Self {
            mode: IconMode::System,
            color: None,
            path: Some(path),
        }
    }
    pub fn generated(mode: IconMode, color: Rgb, path: PathBuf) -> Self {
        Self {
            mode,
            color: Some(color),
            path: Some(path),
        }
    }
    /// True when TuiPWA owns the icon file and may delete it.
    pub fn owned_by_tuipwa(&self) -> bool {
        self.mode.owned_by_tuipwa()
    }
    pub fn to_meta(&self) -> IconMeta {
        IconMeta {
            mode: self.mode,
            color: self.color.map(|c| c.hex()),
            path: self
                .path
                .as_ref()
                .map(|p| p.to_str().unwrap_or_default().to_owned()),
        }
    }
}

/// Decides which icon a registration uses.
///
/// Priority: explicit image, explicit color, existing system icon, generated.
pub fn decide(
    registry: &Registry,
    id: &str,
    name: &str,
    command: &str,
    explicit: Option<&Path>,
    color: Option<Rgb>,
    data_dirs: &[PathBuf],
) -> Result<IconDecision> {
    if let Some(path) = explicit {
        return Ok(IconDecision::explicit(path.to_owned()));
    }
    if let Some(color) = color {
        let path = generate(registry, id, name, color)?;
        return Ok(IconDecision::generated(IconMode::Color, color, path));
    }
    if let Some(found) = find_system_icon(command, data_dirs) {
        return Ok(IconDecision::system(found));
    }
    let color = icon::color_for_id(id);
    let path = generate(registry, id, name, color)?;
    Ok(IconDecision::generated(IconMode::Auto, color, path))
}

/// Generates the PNG of an application and returns its path.
///
/// The file lives in the managed icon directory of the XDG data home so that
/// TuiPWA can own and delete it later.
pub fn generate(registry: &Registry, id: &str, name: &str, color: Rgb) -> Result<PathBuf> {
    let path = registry.icon_path(id)?;
    let pixels = icon::render(&icon::monogram(name), color);
    crate::icons::png::write_png(&path, &pixels, icon::SIZE, icon::SIZE)?;
    Ok(path)
}

/// Searches the desktop entries and icon themes of the system.
///
/// The match is deliberately strict: the executable name of a desktop entry
/// has to be exactly the command name, so a similarly named application is
/// never mistaken for the requested one.
pub fn find_system_icon(command: &str, data_dirs: &[PathBuf]) -> Option<PathBuf> {
    let program = Path::new(command)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(command);
    for desktop in crate::platform::linux::desktop::entries(data_dirs) {
        let Ok(content) = std::fs::read_to_string(&desktop) else {
            continue;
        };
        let mut in_entry = false;
        let mut exec = None;
        let mut icon = None;
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                in_entry = line == "[Desktop Entry]";
                continue;
            }
            if !in_entry {
                continue;
            }
            if let Some(value) = line.strip_prefix("Exec=") {
                exec = Some(value.trim());
            } else if let Some(value) = line.strip_prefix("Icon=") {
                icon = Some(value.trim());
            }
        }
        let Some(exec) = exec else { continue };
        let exec_program = exec.split_whitespace().next().unwrap_or("");
        let exec_name = Path::new(exec_program)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(exec_program);
        if exec_name != program {
            continue;
        }
        let Some(icon) = icon else { continue };
        if let Some(found) = resolve_icon_value(icon, data_dirs) {
            return Some(found);
        }
    }
    None
}

/// Resolves an `Icon=` value to an existing PNG or ICO file.
fn resolve_icon_value(value: &str, data_dirs: &[PathBuf]) -> Option<PathBuf> {
    if value.is_empty() {
        return None;
    }
    if value.contains('/') {
        let path = PathBuf::from(value);
        return usable_icon(&path).then_some(path);
    }
    let themes = theme_dirs(data_dirs);
    // The largest available size is preferred, so that the icon stays sharp
    // in a taskbar. The theme order breaks a tie.
    for size in [512, 256, 192, 128, 96, 64, 48, 32, 24, 16] {
        for theme in &themes {
            let path = theme.join(format!("{size}x{size}/apps/{value}.png"));
            if usable_icon(&path) {
                return Some(path);
            }
        }
    }
    for pixmaps in pixmap_dirs(data_dirs) {
        let path = pixmaps.join(format!("{value}.png"));
        if usable_icon(&path) {
            return Some(path);
        }
    }
    None
}

/// Only PNG and ICO files are usable: manifest v1 accepts no other format.
fn usable_icon(path: &Path) -> bool {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !["png", "ico"].contains(&extension.as_str()) {
        return false;
    }
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata.file_type().is_file(),
        Err(_) => false,
    }
}

fn theme_dirs(data_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut themes = Vec::new();
    for dir in data_dirs {
        let root = dir.join("icons");
        let Ok(read) = std::fs::read_dir(&root) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = read.flatten().map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            if path.is_dir() {
                themes.push(path);
            }
        }
    }
    themes
}

fn pixmap_dirs(data_dirs: &[PathBuf]) -> Vec<PathBuf> {
    data_dirs.iter().map(|d| d.join("pixmaps")).collect()
}

/// Applies an icon decision to a manifest, keeping the manifest v1 shape.
pub fn apply(manifest: &mut Manifest, decision: &IconDecision) {
    manifest.icon = decision
        .path
        .as_ref()
        .map(|p| p.to_str().unwrap_or_default().to_owned());
}
