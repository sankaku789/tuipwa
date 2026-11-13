use crate::Result;
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    #[serde(default)]
    pub schema_version: i32,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub icon: Option<String>,
    pub title: Option<String>,
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub arguments: Vec<String>,
    pub working_directory: Option<String>,
}

// Match System.Text.Json's typed, last-property-wins behavior. A Value-based
// intermediate would incorrectly hide a type error in an earlier duplicate.
impl<'de> Deserialize<'de> for Manifest {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Manifest;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a manifest v1 object")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> std::result::Result<Manifest, M::Error> {
                let mut app = Manifest::default();
                let mut arguments: Option<Vec<Option<String>>> = Some(vec![]);
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "schemaVersion" => app.schema_version = map.next_value()?,
                        "id" => app.id = map.next_value::<Option<String>>()?.unwrap_or_default(),
                        "name" => {
                            app.name = map.next_value::<Option<String>>()?.unwrap_or_default()
                        }
                        "icon" => app.icon = map.next_value()?,
                        "title" => app.title = map.next_value()?,
                        "mode" => {
                            app.mode = map.next_value::<Option<String>>()?.unwrap_or_default()
                        }
                        "command" => {
                            app.command = map.next_value::<Option<String>>()?.unwrap_or_default()
                        }
                        "arguments" => arguments = map.next_value()?,
                        "workingDirectory" => app.working_directory = map.next_value()?,
                        _ => {
                            return Err(serde::de::Error::custom(format!(
                                "unknown manifest field: {key}"
                            )));
                        }
                    }
                }
                app.arguments = arguments
                    .ok_or_else(|| serde::de::Error::custom("arguments cannot be null"))?
                    .into_iter()
                    .map(|arg| {
                        arg.ok_or_else(|| serde::de::Error::custom("argument cannot be null"))
                    })
                    .collect::<std::result::Result<_, M::Error>>()?;
                Ok(app)
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}

pub fn check_id(id: &str) -> Result<()> {
    let mut chars = id.chars();
    if !chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        || !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "._-".contains(c))
    {
        return Err("id は [a-z0-9][a-z0-9._-]* が必要です".into());
    }
    Ok(())
}
fn text(s: &str) -> Result<()> {
    if s.trim().is_empty() || s.chars().any(char::is_control) {
        return Err("空白のみ・制御文字を含むフィールドは使えません".into());
    }
    Ok(())
}
// Lexical normalization matches Path.GetFullPath, including nonexistent commands.
pub fn absolute(path: &Path, base: &Path) -> PathBuf {
    let joined = if path.is_absolute() {
        path.to_owned()
    } else {
        base.join(path)
    };
    let mut out = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}
impl Manifest {
    pub fn load(path: &Path) -> Result<Self> {
        let path = absolute(path, &std::env::current_dir()?);
        let bytes = std::fs::read(&path)?;
        // File.ReadAllText in the v1 implementation accepts a UTF-8 BOM.
        let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes);
        let app: Self = serde_json::from_slice(bytes)?;
        app.validate(path.parent().unwrap())
    }

    /// Loads a manifest that is already stored in the registry.
    ///
    /// The schema rules stay exactly the same as manifest v1, but a missing
    /// icon file does not break `run`, `list`, `show` and `remove`: the icon is
    /// decoration and can be restored or replaced later.
    pub fn load_registered(path: &Path) -> Result<Self> {
        let path = absolute(path, &std::env::current_dir()?);
        let bytes = std::fs::read(&path)?;
        let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes);
        let app: Self = serde_json::from_slice(bytes)?;
        app.validate_with(path.parent().unwrap(), false)
    }

    pub fn validate(self, base: &Path) -> Result<Self> {
        self.validate_with(base, true)
    }

    fn validate_with(mut self, base: &Path, require_icon_file: bool) -> Result<Self> {
        if self.schema_version != 1 {
            return Err("schemaVersion は整数1が必要です".into());
        }
        check_id(&self.id)?;
        text(&self.name)?;
        text(&self.command)?;
        if self.mode != "native" {
            return Err("mode=native のみ対応します".into());
        }
        if let Some(title) = &self.title {
            text(title)?;
        }
        if self.arguments.iter().any(|s| s.contains('\0')) {
            return Err("arguments に NUL は使えません".into());
        }
        let cwd = if let Some(cwd) = &self.working_directory {
            text(cwd)?;
            absolute(Path::new(cwd), base)
        } else {
            base.to_owned()
        };
        if !cwd.is_dir() {
            return Err("作業ディレクトリがありません".into());
        }
        self.working_directory = Some(cwd.to_str().ok_or("非UTF-8パス")?.into());
        if let Some(icon) = &self.icon {
            text(icon)?;
            let path = absolute(Path::new(icon), base);
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if !["png", "ico"].contains(&ext.as_str()) {
                return Err("icon は既存PNG/ICOファイルが必要です".into());
            }
            if require_icon_file && !path.is_file() {
                return Err("icon は既存PNG/ICOファイルが必要です".into());
            }
            self.icon = Some(path.to_str().ok_or("非UTF-8パス")?.into());
        }
        if self.command.contains('/') {
            self.command = absolute(Path::new(&self.command), base)
                .to_str()
                .ok_or("非UTF-8パス")?
                .into();
        }
        if self.title.is_none() {
            self.title = Some(self.name.clone());
        }
        Ok(self)
    }

    /// Serializes the manifest in the manifest v1 shape.
    pub fn to_v1_json(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec_pretty(self)?)
    }
}

/// OS-independent protocol v1. Linux transport and PTY details are not fields.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LaunchRequest {
    pub protocol_version: u32,
    pub request_id: String,
    pub app_id: String,
    pub title: String,
    pub icon: Option<String>,
    pub command: String,
    pub arguments: Vec<String>,
    pub working_directory: String,
}
impl LaunchRequest {
    pub fn new(app: Manifest, request_id: String) -> Self {
        Self {
            protocol_version: 1,
            request_id,
            app_id: app.id,
            title: app.title.unwrap(),
            icon: app.icon,
            command: app.command,
            arguments: app.arguments,
            working_directory: app.working_directory.unwrap(),
        }
    }
}
pub fn desktop_id(id: &str) -> String {
    format!("tuipwa-{id}")
}
