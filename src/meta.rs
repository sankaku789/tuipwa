use crate::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// How the icon of a registration was decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IconMode {
    /// TuiPWA generated the PNG from the palette color of the app id.
    Auto,
    /// TuiPWA generated the PNG from a user supplied color.
    Color,
    /// The user pointed at an image. The file belongs to the user.
    Explicit,
    /// An existing system icon was found. The file belongs to the system.
    System,
}

impl IconMode {
    /// True when TuiPWA owns the icon file and may delete it.
    pub fn owned_by_tuipwa(&self) -> bool {
        matches!(self, IconMode::Auto | IconMode::Color)
    }
    pub fn label(&self) -> &'static str {
        match self {
            IconMode::Auto => "自動生成",
            IconMode::Color => "指定色",
            IconMode::Explicit => "明示指定",
            IconMode::System => "システムアイコン",
        }
    }
}

/// Icon metadata of one registration.
///
/// This is deliberately kept out of the manifest v1 file: manifest v1 rejects
/// unknown fields and is shared with the .NET implementation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IconMeta {
    pub mode: IconMode,
    /// Background color of generated icons.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// The icon file the registration points at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// Validate concrete MIME types before placing them in a desktop string list.
/// Parameters, wildcards, whitespace and desktop-entry delimiters are rejected.
pub fn normalize_mime_types(values: &[String]) -> Result<Vec<String>> {
    let mut normalized = Vec::new();
    for value in values {
        let valid_token = |token: &str| {
            !token.is_empty()
                && token
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"!#$&^_.+-".contains(&b))
        };
        if !value
            .split_once('/')
            .is_some_and(|(kind, subtype)| valid_token(kind) && valid_token(subtype))
        {
            return Err(format!(
                "MIMEタイプは text/plain のような type/subtype で指定してください: {value:?}"
            )
            .into());
        }
        let value = value.to_ascii_lowercase();
        if !normalized.contains(&value) {
            normalized.push(value);
        }
    }
    Ok(normalized)
}

/// Per registration metadata owned by TuiPWA.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppMeta {
    pub meta_version: u32,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<IconMeta>,
    /// Desktop menu categories of the registration.
    ///
    /// Missing means the default `Utility`, so metadata written by older
    /// versions keeps working unchanged. The list is stored here instead of the
    /// manifest because manifest v1 rejects unknown fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub categories: Option<Vec<String>>,
    /// Local file types accepted through the desktop's Open With menu.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mime_types: Vec<String>,
}

impl AppMeta {
    pub fn new(id: &str) -> Self {
        Self {
            meta_version: 1,
            id: id.to_owned(),
            icon: None,
            categories: None,
            mime_types: Vec::new(),
        }
    }

    /// Loads the metadata of a registration.
    ///
    /// A missing file is not an error: registrations created by other versions
    /// stay usable. A broken file is reported as an error.
    pub fn load(path: &Path) -> Result<Option<Self>> {
        if !path.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(path)?;
        let meta: Self = serde_json::from_slice(&bytes)?;
        if meta.meta_version != 1 {
            return Err("メタデータのバージョンに未対応です".into());
        }
        Ok(Some(meta))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        crate::registry::write_atomic(path, &serde_json::to_vec_pretty(self)?)
    }

    pub fn remove(path: &Path) -> Result<()> {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn icon(&self) -> Option<&IconMeta> {
        self.icon.as_ref()
    }

    /// Categories of the registration; `Utility` when none were stored.
    pub fn categories(&self) -> Vec<String> {
        self.categories
            .clone()
            .filter(|categories| !categories.is_empty())
            .unwrap_or_else(|| vec![crate::category::DEFAULT.to_owned()])
    }

    pub fn set_categories(&mut self, categories: Vec<String>) {
        self.categories = Some(categories);
    }

    pub fn set_icon(&mut self, icon: IconMeta) {
        self.icon = Some(icon);
    }

    /// Path of the icon file, if the metadata records one.
    pub fn icon_path(&self) -> Option<PathBuf> {
        self.icon
            .as_ref()
            .and_then(|i| i.path.as_ref().map(PathBuf::from))
    }

    /// True when TuiPWA generated the icon and may delete it.
    pub fn owns_icon(&self) -> bool {
        self.icon().is_some_and(|i| i.mode.owned_by_tuipwa())
    }
}
