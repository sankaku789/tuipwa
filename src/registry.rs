use crate::{
    Result,
    model::{Manifest, check_id, desktop_id},
};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

/// Directory layout of the TuiPWA registry.
///
/// The manifest and desktop entry locations are the manifest v1 contract and
/// are shared with the .NET implementation. Icons and metadata are managed by
/// TuiPWA only.
pub struct Registry {
    /// `${XDG_CONFIG_HOME}/tuipwa/apps/<id>.json` (manifest v1)
    pub manifests: PathBuf,
    /// `${XDG_CONFIG_HOME}/tuipwa/meta/<id>.json` (TuiPWA metadata)
    pub meta: PathBuf,
    /// `${XDG_DATA_HOME}/applications/tuipwa-<id>.desktop`
    pub desktops: PathBuf,
    /// `${XDG_DATA_HOME}/tuipwa/icons/<id>.png` (generated icons)
    pub icons: PathBuf,
}

fn xdg(key: &str, fallback: PathBuf) -> PathBuf {
    std::env::var_os(key)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or(fallback)
}

impl Registry {
    pub fn new() -> Result<Self> {
        let home = PathBuf::from(std::env::var_os("HOME").ok_or("HOME がありません")?);
        let config = xdg("XDG_CONFIG_HOME", home.join(".config"));
        let data = xdg("XDG_DATA_HOME", home.join(".local/share"));
        Ok(Self {
            manifests: config.join("tuipwa/apps"),
            meta: config.join("tuipwa/meta"),
            desktops: data.join("applications"),
            icons: data.join("tuipwa/icons"),
        })
    }

    pub fn manifest_path(&self, id: &str) -> Result<PathBuf> {
        check_id(id)?;
        Ok(self.manifests.join(format!("{id}.json")))
    }

    pub fn meta_path(&self, id: &str) -> Result<PathBuf> {
        check_id(id)?;
        Ok(self.meta.join(format!("{id}.json")))
    }

    pub fn desktop_path(&self, id: &str) -> Result<PathBuf> {
        check_id(id)?;
        Ok(self.desktops.join(format!("{}.desktop", desktop_id(id))))
    }

    pub fn icon_path(&self, id: &str) -> Result<PathBuf> {
        check_id(id)?;
        Ok(self.icons.join(format!("{id}.png")))
    }

    pub fn load(&self, id: &str) -> Result<Manifest> {
        Manifest::load_registered(&self.manifest_path(id)?)
    }

    pub fn list(&self) -> Result<Vec<Manifest>> {
        if !self.manifests.exists() {
            return Ok(vec![]);
        }
        let mut paths = fs::read_dir(&self.manifests)?
            .map(|e| e.map(|e| e.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        paths.sort();
        paths
            .into_iter()
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .map(|p| Manifest::load_registered(&p))
            .collect()
    }

    /// Installs a registration together with its metadata.
    ///
    /// The manifest, the desktop entry and the metadata are written one after
    /// the other. A failure removes the files that were created by this call,
    /// so a failed installation leaves no partial registration behind.
    pub fn install(
        &self,
        app: &Manifest,
        meta: &crate::meta::AppMeta,
        launcher: &Path,
    ) -> Result<()> {
        let manifest = self.manifest_path(&app.id)?;
        if manifest.exists() {
            return Err(format!("既に登録されています: {}", app.id).into());
        }
        let desktop = self.desktop_path(&app.id)?;
        let meta_path = self.meta_path(&app.id)?;
        let entry = entry(app, &meta.categories(), launcher)?;
        fs::create_dir_all(&self.manifests)?;
        fs::create_dir_all(&self.desktops)?;
        if desktop.exists() {
            return Err(format!("既に登録されています: {}", app.id).into());
        }
        let mut snapshot = Snapshot::new();
        snapshot.capture(&manifest)?;
        snapshot.capture(&desktop)?;
        snapshot.capture(&meta_path)?;
        let result = (|| -> Result<()> {
            write_atomic(&manifest, &app.to_v1_json()?)?;
            write_atomic(&desktop, entry.as_bytes())?;
            meta.save(&meta_path)?;
            Ok(())
        })();
        if result.is_err() {
            snapshot.restore();
        }
        result
    }

    /// Updates a registration and its metadata.
    ///
    /// All files are written through temporary files and renames. The rename
    /// of a single file is atomic; the update of several files is not atomic as
    /// a whole, so the previous content is restored when a later step fails.
    ///
    /// `regenerate` is called after the snapshot was taken and before the
    /// manifest is written. A generated icon is therefore restored together
    /// with the manifest when the update fails.
    pub fn update(
        &self,
        app: &Manifest,
        meta: &crate::meta::AppMeta,
        launcher: &Path,
        regenerate: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        let manifest = self.manifest_path(&app.id)?;
        let desktop = self.desktop_path(&app.id)?;
        let meta_path = self.meta_path(&app.id)?;
        let entry = entry(app, &meta.categories(), launcher)?;
        fs::create_dir_all(&self.manifests)?;
        fs::create_dir_all(&self.desktops)?;
        let mut snapshot = Snapshot::new();
        snapshot.capture(&manifest)?;
        snapshot.capture(&desktop)?;
        snapshot.capture(&meta_path)?;
        snapshot.capture(&self.icon_path(&app.id)?)?;
        let result = (|| -> Result<()> {
            regenerate()?;
            write_atomic(&manifest, &app.to_v1_json()?)?;
            write_atomic(&desktop, entry.as_bytes())?;
            meta.save(&meta_path)?;
            Ok(())
        })();
        if result.is_err() {
            snapshot.restore();
        }
        result
    }

    /// Removes a registration. The desktop entry ownership is verified first.
    pub fn unregister(&self, id: &str) -> Result<()> {
        let manifest = self.manifest_path(id)?;
        if !manifest.is_file() {
            return Err(format!("未登録です: {id}").into());
        }
        let desktop = self.desktop_path(id)?;
        let desktop_content = if desktop.exists() {
            let content = fs::read_to_string(&desktop)?;
            if !content.lines().any(|l| l == format!("X-TuiPWA-Id={id}")) {
                return Err("外部で置換されたdesktopエントリです".into());
            }
            Some(content)
        } else {
            None
        };
        // The manifest is the source of truth. When the desktop entry cannot
        // be removed, the manifest is restored so that the registration stays
        // consistent instead of leaving a half removed registration behind.
        let manifest_content = fs::read(&manifest)?;
        fs::remove_file(&manifest)?;
        if desktop_content.is_some()
            && let Err(error) = fs::remove_file(&desktop)
        {
            let _ = write_atomic(&manifest, &manifest_content);
            return Err(error.into());
        }
        Ok(())
    }

    /// Verifies that the desktop entry still belongs to this registration.
    pub fn check_desktop_owner(&self, id: &str) -> Result<()> {
        let desktop = self.desktop_path(id)?;
        if desktop.exists()
            && !fs::read_to_string(&desktop)?
                .lines()
                .any(|l| l == format!("X-TuiPWA-Id={id}"))
        {
            return Err("外部で置換されたdesktopエントリです".into());
        }
        Ok(())
    }
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Writes a file through a temporary file and a rename.
///
/// The rename is atomic within one directory. Files that span several
/// directories are not updated atomically as a whole; the caller rolls back
/// the already written files when a later step fails.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path.parent().ok_or("親ディレクトリがありません")?;
    fs::create_dir_all(dir)?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("不正なファイル名です")?;
    let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp = dir.join(format!(".{name}.tmp{}-{unique}", std::process::id()));
    let write = || -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(())
    };
    if let Err(err) = write() {
        let _ = fs::remove_file(&temp);
        return Err(err);
    }
    if let Err(err) = fs::rename(&temp, path) {
        let _ = fs::remove_file(&temp);
        return Err(err.into());
    }
    Ok(())
}

/// A snapshot of files that a multi file update may touch.
///
/// The snapshot lets a failed update restore the previous content instead of
/// leaving a half written registration behind.
pub struct Snapshot {
    entries: Vec<(PathBuf, Option<Vec<u8>>)>,
}

impl Snapshot {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn capture(&mut self, path: &Path) -> Result<()> {
        let content = match fs::read(path) {
            Ok(bytes) => Some(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        self.entries.push((path.to_owned(), content));
        Ok(())
    }

    pub fn restore(self) {
        for (path, content) in self.entries.into_iter().rev() {
            match content {
                Some(bytes) => {
                    let _ = write_atomic(&path, &bytes);
                }
                None => {
                    let _ = fs::remove_file(&path);
                }
            }
        }
    }
}

/// Removes a file that TuiPWA generated inside its managed icon directory.
///
/// Files outside the managed directory and symbolic links are never removed.
pub fn remove_managed_icon(icons: &Path, path: &Path) -> Result<bool> {
    let base = icons.canonicalize().unwrap_or_else(|_| icons.to_owned());
    let parent = path.parent().unwrap_or(Path::new(""));
    let parent = parent.canonicalize().unwrap_or_else(|_| parent.to_owned());
    if parent != base {
        return Ok(false);
    }
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e.into()),
    };
    if !metadata.file_type().is_file() {
        return Ok(false);
    }
    fs::remove_file(path)?;
    Ok(true)
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
        .replace(' ', "\\s")
}
fn quote(s: &str) -> Result<String> {
    if s.chars().any(char::is_control) {
        return Err("ランチャーパスに制御文字は使えません".into());
    }
    Ok(format!(
        "\"{}\"",
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('`', "\\`")
            .replace('$', "\\$")
            .replace('%', "%%")
    )
    .replace('\\', "\\\\"))
}
pub fn entry(app: &Manifest, categories: &[String], launcher: &Path) -> Result<String> {
    let path = launcher.to_str().ok_or("非UTF-8パス")?;
    if !launcher.is_absolute() || path.contains('=') {
        return Err("ランチャーは=を含まない絶対パスが必要です".into());
    }
    // Re-validate on the way out: metadata is user editable, and a `;` or a
    // newline must never reach the desktop entry as an extra field.
    let categories = crate::category::normalize(categories)?;
    Ok(format!(
        "[Desktop Entry]\nType=Application\nVersion=1.0\nName={}\nExec={} {} {}\n{}Terminal=false\nCategories={}\nStartupWMClass={}\nX-TuiPWA-Id={}\n",
        escape(&app.name),
        quote(path)?,
        quote("run")?,
        quote(&app.id)?,
        app.icon
            .as_ref()
            .map(|s| format!("Icon={}\n", escape(s)))
            .unwrap_or_default(),
        crate::category::desktop_value(&categories),
        desktop_id(&app.id),
        app.id
    ))
}
