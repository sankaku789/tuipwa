use crate::{
    Result,
    icons::{
        self, IconDecision,
        render::{self as icon, Rgb},
    },
    meta::{AppMeta, IconMeta, IconMode},
    model::{Manifest, check_id},
    registry::Registry,
};
use std::path::{Path, PathBuf};

const USAGE_JA: &str = "\
使用法:
  tuipwa add [オプション] <command> [-- <引数>...]
  tuipwa add --manifest <manifest.json>
  tuipwa remove <id>
  tuipwa list
  tuipwa show <id>
  tuipwa update <id> [オプション] [-- <引数>...]
  tuipwa run <id> [id ...]
  tuipwa validate <manifest.json>

add のオプション:
  --id <id>            登録ID（既定は実行ファイル名）
  --name <name>        表示名（既定は実行ファイル名）
  --title <title>      ウィンドウタイトル（既定は表示名）
  --cwd <directory>    作業ディレクトリ（既定はホームディレクトリ）
  --icon <path>        既存PNG/ICOをアイコンに使う
  --color \"#RRGGBB\"    指定色のアイコンを自動生成する
  --category <name>    デスクトップカテゴリ（複数指定可、既定はUtility）
  --manifest <file>    既存のJSONマニフェストから登録する

update のオプション:
  --name <name>        表示名を変更する
  --title <title>      ウィンドウタイトルを変更する
  --cwd <directory>    作業ディレクトリを変更する
  --command <path>     実行コマンドを変更する
  --icon <path>        既存PNG/ICOをアイコンに使う
  --color \"#RRGGBB\"    指定色のアイコンを生成し直す
  --auto-icon          TuiPWAが自動生成するアイコンに戻す
  --category <name>    デスクトップカテゴリを置き換える（複数指定可）
  --reset-categories   デスクトップカテゴリをUtilityへ戻す
  --clear-args         引数を空にする";

const USAGE_EN: &str = "\
Usage:
  tuipwa add [options] <command> [-- <args>...]
  tuipwa add --manifest <manifest.json>
  tuipwa remove <id>
  tuipwa list
  tuipwa show <id>
  tuipwa update <id> [options] [-- <args>...]
  tuipwa run <id> [id ...]
  tuipwa validate <manifest.json>

Add options:
  --id <id>            Registration ID (default: executable name)
  --name <name>        Display name (default: executable name)
  --title <title>      Window title (default: display name)
  --cwd <directory>    Working directory (default: home directory)
  --icon <path>        Use an existing PNG/ICO icon
  --color \"#RRGGBB\"    Generate an icon with this background color
  --category <name>    Desktop category (repeatable; default: Utility)
  --manifest <file>    Register an existing JSON manifest

Update options:
  --name <name>        Change the display name
  --title <title>      Change the window title
  --cwd <directory>    Change the working directory
  --command <path>     Change the executable
  --icon <path>        Use an existing PNG/ICO icon
  --color \"#RRGGBB\"    Regenerate the icon with this background color
  --auto-icon          Restore a TuiPWA-generated icon
  --category <name>    Replace desktop categories (repeatable)
  --reset-categories   Reset desktop categories to Utility
  --clear-args         Clear all arguments";

pub(crate) fn usage_for_locale(
    lc_all: Option<&str>,
    lc_messages: Option<&str>,
    lang: Option<&str>,
) -> &'static str {
    let locale = [lc_all, lc_messages, lang]
        .into_iter()
        .flatten()
        .find(|value| !value.is_empty())
        .unwrap_or("C");
    let language = locale.split(['_', '-', '.', '@']).next().unwrap_or("");
    if language.eq_ignore_ascii_case("ja") {
        USAGE_JA
    } else {
        USAGE_EN
    }
}

fn usage() -> &'static str {
    usage_for_locale(
        std::env::var("LC_ALL").ok().as_deref(),
        std::env::var("LC_MESSAGES").ok().as_deref(),
        std::env::var("LANG").ok().as_deref(),
    )
}

/// Everything the command line interface needs from the environment.
///
/// Keeping the environment in one struct makes the commands testable without
/// touching the XDG directories of the running user.
pub struct Context {
    pub registry: Registry,
    /// Home directory of the user.
    pub home: PathBuf,
    /// Current working directory.
    pub cwd: PathBuf,
    /// The public launcher that is written into the desktop entries.
    pub launcher: PathBuf,
    /// Directories that are searched for executables.
    pub path_dirs: Vec<PathBuf>,
    /// XDG data directories that are searched for desktop entries and icons.
    pub data_dirs: Vec<PathBuf>,
}

impl Context {
    /// Builds the context from the environment of the process.
    pub fn from_env() -> Result<Self> {
        let home = PathBuf::from(std::env::var_os("HOME").ok_or("HOME がありません")?);
        let cwd = std::env::current_dir()?;
        let launcher = std::env::current_exe()?;
        let path_dirs = match std::env::var_os("PATH") {
            Some(path) => std::env::split_paths(&path).collect(),
            None => Vec::new(),
        };
        let data_dirs = system_data_dirs();
        Ok(Self {
            registry: Registry::new()?,
            home,
            cwd,
            launcher,
            path_dirs,
            data_dirs,
        })
    }
}

/// The XDG data directories that are searched for desktop entries and icons.
///
/// The data home of the user comes first, then the system wide directories.
pub fn system_data_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(data) = std::env::var_os("XDG_DATA_HOME").filter(|v| Path::new(v).is_absolute()) {
        dirs.push(PathBuf::from(data));
    }
    match std::env::var_os("XDG_DATA_DIRS") {
        Some(value) => dirs.extend(std::env::split_paths(&value)),
        None => {
            dirs.push(PathBuf::from("/usr/local/share"));
            dirs.push(PathBuf::from("/usr/share"));
        }
    }
    dirs
}

/// Parsed options of `add`.
#[derive(Default)]
struct AddOptions {
    manifest: Option<PathBuf>,
    id: Option<String>,
    name: Option<String>,
    title: Option<String>,
    cwd: Option<PathBuf>,
    icon: Option<PathBuf>,
    color: Option<String>,
    categories: Vec<String>,
    command: Option<String>,
    arguments: Vec<String>,
}

/// Parsed options of `update`.
#[derive(Default)]
struct UpdateOptions {
    name: Option<String>,
    title: Option<String>,
    cwd: Option<PathBuf>,
    command: Option<String>,
    icon: Option<PathBuf>,
    color: Option<String>,
    categories: Vec<String>,
    auto_icon: bool,
    reset_categories: bool,
    clear_args: bool,
    arguments: Option<Vec<String>>,
}

/// Result of the option scanner.
struct Options {
    values: Vec<(String, Option<String>)>,
    positionals: Vec<String>,
    arguments: Vec<String>,
}

/// Scans command line options.
///
/// Options may appear before or after positional arguments. Everything after
/// `--` is passed through untouched and is never interpreted by a shell.
fn scan(
    args: &[String],
    value_options: &[&str],
    flags: &[&str],
) -> std::result::Result<Options, String> {
    let mut values: Vec<(String, Option<String>)> = Vec::new();
    let mut positionals: Vec<String> = Vec::new();
    let mut arguments: Vec<String> = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == "--" {
            arguments.extend_from_slice(&args[index + 1..]);
            break;
        }
        if let Some(name) = arg.strip_prefix("--") {
            let (name, inline) = match name.split_once('=') {
                Some((name, value)) => (name.to_owned(), Some(value.to_owned())),
                None => (name.to_owned(), None),
            };
            if flags.contains(&name.as_str()) {
                if inline.is_some() {
                    return Err(format!("--{name} は値を受け取らないオプションです"));
                }
                values.push((name, None));
                index += 1;
                continue;
            }
            if !value_options.contains(&name.as_str()) {
                return Err(format!("未知のオプションです: --{name}"));
            }
            let value = match inline {
                Some(value) => value,
                None => {
                    index += 1;
                    match args.get(index) {
                        Some(value) => value.clone(),
                        None => return Err(format!("--{name} には値が必要です")),
                    }
                }
            };
            values.push((name, Some(value)));
            index += 1;
            continue;
        }
        positionals.push(arg.clone());
        index += 1;
    }
    Ok(Options {
        values,
        positionals,
        arguments,
    })
}

fn take<'a>(values: &'a [(String, Option<String>)], name: &str) -> Option<&'a str> {
    values
        .iter()
        .find(|(key, _)| key == name)
        .and_then(|(_, value)| value.as_deref())
}

fn has(values: &[(String, Option<String>)], name: &str) -> bool {
    values.iter().any(|(key, _)| key == name)
}

/// All values of a repeatable option, in command line order.
fn all<'a>(values: &'a [(String, Option<String>)], name: &str) -> Vec<&'a str> {
    values
        .iter()
        .filter(|(key, _)| key == name)
        .filter_map(|(_, value)| value.as_deref())
        .collect()
}

fn duplicate(values: &[(String, Option<String>)], name: &str) -> std::result::Result<(), String> {
    if values.iter().filter(|(key, _)| key == name).count() > 1 {
        return Err(format!("--{name} は一度だけ指定してください"));
    }
    Ok(())
}

/// Entry point of the public command line interface.
pub fn run(args: &[String]) -> Result<i32> {
    let context = Context::from_env()?;
    execute(args, &context)
}

/// Runs a command with an explicit context.
pub fn execute(args: &[String], context: &Context) -> Result<i32> {
    let usage = usage();
    let Some(command) = args.first() else {
        eprintln!("{usage}");
        return Ok(2);
    };
    match command.as_str() {
        "add" => add(&args[1..], context),
        "remove" => remove(&args[1..], context),
        "list" => list(&args[1..], context),
        "show" => show(&args[1..], context),
        "update" => update(&args[1..], context),
        "run" => run_apps(&args[1..], context),
        "validate" => validate(&args[1..], context),
        "--help" | "-h" | "help" => {
            println!("{usage}");
            Ok(0)
        }
        other => {
            eprintln!("TuiPWA: 未知のコマンドです: {other}\n{usage}");
            Ok(2)
        }
    }
}

fn usage_error(message: &str) -> Result<i32> {
    let usage = usage();
    eprintln!("TuiPWA: {message}\n{usage}");
    Ok(2)
}

fn validate(args: &[String], _context: &Context) -> Result<i32> {
    if args.len() != 1 {
        return usage_error("validate にはマニフェストを1つ指定してください");
    }
    let manifest = Manifest::load(Path::new(&args[0]))?;
    println!("有効: {}", manifest.id);
    Ok(0)
}

fn add(args: &[String], context: &Context) -> Result<i32> {
    let scanned = match scan(
        args,
        &[
            "manifest", "id", "name", "title", "cwd", "icon", "color", "category",
        ],
        &[],
    ) {
        Ok(scanned) => scanned,
        Err(message) => return usage_error(&message),
    };
    for name in ["manifest", "id", "name", "title", "cwd", "icon", "color"] {
        if let Err(message) = duplicate(&scanned.values, name) {
            return usage_error(&message);
        }
    }
    let options = AddOptions {
        manifest: take(&scanned.values, "manifest").map(PathBuf::from),
        id: take(&scanned.values, "id").map(str::to_owned),
        name: take(&scanned.values, "name").map(str::to_owned),
        title: take(&scanned.values, "title").map(str::to_owned),
        cwd: take(&scanned.values, "cwd").map(PathBuf::from),
        icon: take(&scanned.values, "icon").map(PathBuf::from),
        color: take(&scanned.values, "color").map(str::to_owned),
        categories: all(&scanned.values, "category")
            .into_iter()
            .map(str::to_owned)
            .collect(),
        command: scanned.positionals.first().cloned(),
        arguments: scanned.arguments.clone(),
    };
    if options.icon.is_some() && options.color.is_some() {
        return usage_error("--icon と --color は同時に指定できません");
    }
    if let Some(manifest) = &options.manifest {
        if options.id.is_some()
            || options.name.is_some()
            || options.title.is_some()
            || options.cwd.is_some()
            || options.icon.is_some()
            || options.color.is_some()
            || !options.categories.is_empty()
            || options.command.is_some()
        {
            return usage_error("--manifest と他の追加オプションは同時に指定できません");
        }
        return add_from_manifest(manifest, context);
    }
    let Some(command) = options.command.clone() else {
        return usage_error("add には実行コマンドを指定してください");
    };
    let registry = &context.registry;
    let (command_path, file_name) = resolve_command(&command, context)?;
    let id = match options.id.clone() {
        Some(id) => {
            check_id(&id)?;
            id
        }
        None => derive_id(&file_name)?,
    };
    if registry.manifest_path(&id)?.exists() {
        return Err(format!("既に登録されています: {id}").into());
    }
    let name = options.name.clone().unwrap_or_else(|| file_name.clone());
    // Manifest v1 rejects an empty name and control characters.
    if name.trim().is_empty() || name.chars().any(char::is_control) {
        return Err("表示名は空白のみ・制御文字を含む値を使えません".into());
    }
    let title = options.title.clone().unwrap_or_else(|| name.clone());
    if title.trim().is_empty() || title.chars().any(char::is_control) {
        return Err("ウィンドウタイトルは空白のみ・制御文字を含む値を使えません".into());
    }
    let cwd = match options.cwd.clone() {
        Some(cwd) => {
            let cwd = absolute(&cwd, &context.cwd)?;
            if !cwd.is_dir() {
                return Err(format!("作業ディレクトリがありません: {}", cwd.display()).into());
            }
            cwd
        }
        None => context.home.clone(),
    };
    let categories = if options.categories.is_empty() {
        vec![crate::category::DEFAULT.to_owned()]
    } else {
        crate::category::normalize(&options.categories)?
    };
    let color = match options.color.clone() {
        Some(color) => Some(icon::parse_color(&color)?),
        None => None,
    };
    let explicit = match options.icon.clone() {
        Some(icon) => {
            let icon = absolute(&icon, &context.cwd)?;
            if !is_usable_icon(&icon) {
                return Err("icon は既存のPNG/ICOファイルを指定してください".into());
            }
            Some(icon)
        }
        None => None,
    };
    let decision = icons::decide(
        registry,
        &id,
        &name,
        command_path.to_str().unwrap_or_default(),
        explicit.as_deref(),
        color,
        &context.data_dirs,
    )?;
    let mut app = Manifest {
        schema_version: 1,
        id,
        name,
        icon: None,
        title: Some(title),
        mode: "native".into(),
        command: command_path.to_str().ok_or("非UTF-8パス")?.into(),
        arguments: options.arguments.clone(),
        working_directory: Some(cwd.to_str().ok_or("非UTF-8パス")?.into()),
    };
    icons::apply(&mut app, &decision);
    let mut meta = AppMeta::new(&app.id);
    meta.set_icon(decision.to_meta());
    meta.set_categories(categories);
    if let Err(error) = registry.install(&app, &meta, &context.launcher) {
        // A generated icon of a failed registration is removed again.
        if decision.owned_by_tuipwa()
            && let Some(path) = decision.path.as_ref()
        {
            let _ = crate::registry::remove_managed_icon(&registry.icons, path);
        }
        return Err(error);
    }
    println!("登録: {}", app.id);
    Ok(0)
}

fn add_from_manifest(path: &Path, context: &Context) -> Result<i32> {
    let registry = &context.registry;
    // Manifest v1 validation and path resolution stay unchanged.
    let mut app = Manifest::load(path)?;
    if registry.manifest_path(&app.id)?.exists() {
        return Err(format!("既に登録されています: {}", app.id).into());
    }
    let explicit = match app.icon.clone() {
        Some(icon) => {
            let icon = PathBuf::from(icon);
            if !is_usable_icon(&icon) {
                return Err("icon は既存のPNG/ICOファイルを指定してください".into());
            }
            Some(icon)
        }
        None => None,
    };
    let name = app.name.clone();
    let decision = icons::decide(
        registry,
        &app.id,
        &name,
        &app.command,
        explicit.as_deref(),
        None,
        &context.data_dirs,
    )?;
    icons::apply(&mut app, &decision);
    let mut meta = AppMeta::new(&app.id);
    meta.set_icon(decision.to_meta());
    // Manifest v1 has no categories; the import uses the default.
    meta.set_categories(vec![crate::category::DEFAULT.to_owned()]);
    if let Err(error) = registry.install(&app, &meta, &context.launcher) {
        // A generated icon of a failed registration is removed again.
        if decision.owned_by_tuipwa()
            && let Some(path) = decision.path.as_ref()
        {
            let _ = crate::registry::remove_managed_icon(&registry.icons, path);
        }
        return Err(error);
    }
    println!("登録: {}", app.id);
    Ok(0)
}

fn remove(args: &[String], context: &Context) -> Result<i32> {
    if args.len() != 1 {
        return usage_error("remove には登録IDを1つ指定してください");
    }
    let id = args[0].clone();
    let registry = &context.registry;
    if !registry.manifest_path(&id)?.is_file() {
        return Err(format!("未登録です: {id}").into());
    }
    registry.unregister(&id)?;
    let meta_path = registry.meta_path(&id)?;
    let meta = AppMeta::load(&meta_path).ok().flatten();
    if let Some(meta) = meta {
        // Only icons that TuiPWA generated itself are deleted. A user image or
        // a system icon stays untouched.
        if meta.owns_icon()
            && let Some(path) = meta.icon_path()
        {
            let _ = crate::registry::remove_managed_icon(&registry.icons, &path);
        }
    }
    let _ = AppMeta::remove(&meta_path);
    println!("削除: {id}");
    Ok(0)
}

fn list(args: &[String], context: &Context) -> Result<i32> {
    if !args.is_empty() {
        return usage_error("list には引数を指定できません");
    }
    let registry = &context.registry;
    for app in registry.list()? {
        let meta = AppMeta::load(&registry.meta_path(&app.id)?).ok().flatten();
        let source = match meta.as_ref().and_then(|m| m.icon()) {
            Some(meta) => meta.mode.label(),
            None => "未記録",
        };
        println!("{}\t{}\t{}", app.id, app.name, source);
    }
    Ok(0)
}

fn show(args: &[String], context: &Context) -> Result<i32> {
    if args.len() != 1 {
        return usage_error("show には登録IDを1つ指定してください");
    }
    let id = args[0].clone();
    let registry = &context.registry;
    let app = registry.load(&id)?;
    let meta = AppMeta::load(&registry.meta_path(&id)?).ok().flatten();
    let desktop = registry.desktop_path(&app.id)?;
    println!("{}", describe(&app, meta.as_ref(), &desktop)?);
    Ok(0)
}

/// Renders the registration for a human reader.
pub fn describe(app: &Manifest, meta: Option<&AppMeta>, desktop: &Path) -> Result<String> {
    let icon_meta = meta.and_then(|m| m.icon());
    let categories = meta
        .map(|m| m.categories())
        .unwrap_or_else(|| vec![crate::category::DEFAULT.to_owned()]);
    let (source, reference) = match icon_meta {
        Some(meta) => (
            meta.mode.label().to_owned(),
            meta.path.clone().unwrap_or_default(),
        ),
        None => ("未記録".to_owned(), app.icon.clone().unwrap_or_default()),
    };
    Ok(format!(
        "ID: {}\n表示名: {}\nウィンドウタイトル: {}\n実行コマンド: {}\n引数: {}\n作業ディレクトリ: {}\nカテゴリ: {}\nアイコン設定方式: {}\nアイコン参照先: {}\ndesktopエントリ: {}",
        app.id,
        app.name,
        app.title.clone().unwrap_or_default(),
        app.command,
        if app.arguments.is_empty() {
            "(なし)".to_owned()
        } else {
            app.arguments.join(" ")
        },
        app.working_directory.clone().unwrap_or_default(),
        crate::category::desktop_value(&categories),
        source,
        reference,
        desktop.display()
    ))
}

fn update(args: &[String], context: &Context) -> Result<i32> {
    let scanned = match scan(
        args,
        &[
            "name", "title", "cwd", "command", "icon", "color", "id", "category",
        ],
        &["auto-icon", "clear-args", "reset-categories"],
    ) {
        Ok(scanned) => scanned,
        Err(message) => return usage_error(&message),
    };
    for name in ["name", "title", "cwd", "command", "icon", "color"] {
        if let Err(message) = duplicate(&scanned.values, name) {
            return usage_error(&message);
        }
    }
    if scanned.positionals.len() != 1 {
        return usage_error("update には登録IDを1つ指定してください");
    }
    if has(&scanned.values, "id") {
        return usage_error("IDは変更できません");
    }
    let id = scanned.positionals[0].clone();
    let options = UpdateOptions {
        name: take(&scanned.values, "name").map(str::to_owned),
        title: take(&scanned.values, "title").map(str::to_owned),
        cwd: take(&scanned.values, "cwd").map(PathBuf::from),
        command: take(&scanned.values, "command").map(str::to_owned),
        icon: take(&scanned.values, "icon").map(PathBuf::from),
        color: take(&scanned.values, "color").map(str::to_owned),
        categories: all(&scanned.values, "category")
            .into_iter()
            .map(str::to_owned)
            .collect(),
        auto_icon: has(&scanned.values, "auto-icon"),
        reset_categories: has(&scanned.values, "reset-categories"),
        clear_args: has(&scanned.values, "clear-args"),
        arguments: if scanned.arguments.is_empty() {
            None
        } else {
            Some(scanned.arguments.clone())
        },
    };
    if options.icon.is_some() && options.color.is_some() {
        return usage_error("--icon と --color は同時に指定できません");
    }
    if options.auto_icon && (options.icon.is_some() || options.color.is_some()) {
        return usage_error("--auto-icon は --icon や --color と同時に指定できません");
    }
    if options.reset_categories && !options.categories.is_empty() {
        return usage_error("--reset-categories と --category は同時に指定できません");
    }
    if options.clear_args && options.arguments.is_some() {
        return usage_error("--clear-args と -- の引数は同時に指定できません");
    }
    // Validate before any file is touched so an invalid list never reaches the
    // metadata or the desktop entry.
    let categories = if options.reset_categories {
        Some(vec![crate::category::DEFAULT.to_owned()])
    } else if options.categories.is_empty() {
        None
    } else {
        Some(crate::category::normalize(&options.categories)?)
    };
    let registry = &context.registry;
    if !registry.manifest_path(&id)?.is_file() {
        return Err(format!("未登録です: {id}").into());
    }
    // An externally replaced desktop entry is never overwritten.
    registry.check_desktop_owner(&id)?;
    let mut app = registry.load(&id)?;
    let meta = AppMeta::load(&registry.meta_path(&id)?)?;
    let previous = meta.as_ref().and_then(|m| m.icon()).cloned();
    let previous_name = app.name.clone();

    if let Some(name) = options.name.clone() {
        if name.trim().is_empty() || name.chars().any(char::is_control) {
            return Err("表示名は空白のみ・制御文字を含む値を使えません".into());
        }
        // A title that was derived from the old name follows the new name.
        if app.title.as_deref() == Some(app.name.as_str()) {
            app.title = Some(name.clone());
        }
        app.name = name;
    }
    if let Some(title) = options.title.clone() {
        if title.trim().is_empty() || title.chars().any(char::is_control) {
            return Err("ウィンドウタイトルは空白のみ・制御文字を含む値を使えません".into());
        }
        app.title = Some(title);
    }
    // The monogram is only regenerated when the name it is built from changed.
    let name_changed = app.name != previous_name;
    if let Some(cwd) = options.cwd.clone() {
        let cwd = absolute(&cwd, &context.cwd)?;
        if !cwd.is_dir() {
            return Err(format!("作業ディレクトリがありません: {}", cwd.display()).into());
        }
        app.working_directory = Some(cwd.to_str().ok_or("非UTF-8パス")?.into());
    }
    if let Some(command) = options.command.clone() {
        // Manifest v1 resolves a bare name through PATH at launch time, and a
        // path relative to the manifest. The update keeps that rule, but a
        // path has to be an executable regular file.
        app.command = if command.contains('/') {
            let (path, _) = resolve_command(&command, context)?;
            path.to_str().ok_or("非UTF-8パス")?.into()
        } else {
            command
        };
    }
    if let Some(arguments) = options.arguments.clone() {
        app.arguments = arguments;
    } else if options.clear_args {
        app.arguments = Vec::new();
    }

    let color = match options.color.clone() {
        Some(color) => Some(icon::parse_color(&color)?),
        None => None,
    };
    let explicit = match options.icon.clone() {
        Some(icon) => {
            let icon = absolute(&icon, &context.cwd)?;
            if !is_usable_icon(&icon) {
                return Err("icon は既存のPNG/ICOファイルを指定してください".into());
            }
            Some(icon)
        }
        None => None,
    };
    let decision = decide_update_icon(
        registry,
        &id,
        previous.as_ref(),
        explicit,
        color,
        options.auto_icon,
        name_changed,
    )?;
    let mut meta = meta.unwrap_or_else(|| AppMeta::new(&id));
    if let Some(categories) = categories {
        meta.set_categories(categories);
    }
    if let Some(decision) = &decision {
        meta.set_icon(decision.to_meta());
    } else if meta.icon().is_none()
        && let Some(path) = app.icon.clone()
    {
        // A registration without metadata keeps an unknown icon. It is
        // recorded as an external file, so it is never regenerated or deleted.
        meta.set_icon(IconMeta {
            mode: IconMode::Explicit,
            color: None,
            path: Some(path),
        });
    }
    // The manifest keeps its v1 shape; only the icon path may change.
    if let Some(decision) = &decision {
        icons::apply(&mut app, decision);
    }
    // The icon is written inside the update, so a failed update restores the
    // previous icon together with the manifest.
    let regenerate = || -> Result<()> {
        if let Some(decision) = &decision
            && let (Some(color), Some(path)) = (decision.color, decision.path.as_ref())
            && decision.mode.owned_by_tuipwa()
        {
            let pixels = icon::render(&icon::monogram(&app.name), color);
            crate::icons::png::write_png(path, &pixels, icon::SIZE, icon::SIZE)?;
        }
        Ok(())
    };
    registry.update(&app, &meta, &context.launcher, &regenerate)?;
    // A generated icon that is no longer referenced is removed. A user image
    // and a system icon are never touched.
    if let Some(previous) = previous
        && previous.mode.owned_by_tuipwa()
    {
        let previous_path = previous.path.clone().map(PathBuf::from);
        let still_used = match &decision {
            // No icon decision means the current icon is kept as-is.
            None => true,
            Some(decision) => {
                decision.owned_by_tuipwa() && decision.path.as_ref() == previous_path.as_ref()
            }
        };
        if !still_used && let Some(path) = previous_path {
            let _ = crate::registry::remove_managed_icon(&registry.icons, &path);
        }
    }
    println!("更新: {}", app.id);
    Ok(0)
}

/// Decides which icon an updated registration uses.
///
/// Without an icon option the current icon is kept. A monogram that TuiPWA
/// generated itself is refreshed when the display name changed. A user image
/// and a system icon are never replaced by a generated icon.
///
/// The returned decision is not written yet: the caller writes the file
/// inside the update, so a failed update restores the previous icon.
fn decide_update_icon(
    registry: &Registry,
    id: &str,
    previous: Option<&IconMeta>,
    explicit: Option<PathBuf>,
    color: Option<Rgb>,
    auto_icon: bool,
    name_changed: bool,
) -> Result<Option<IconDecision>> {
    if let Some(path) = explicit {
        return Ok(Some(IconDecision::explicit(path)));
    }
    if let Some(color) = color {
        return Ok(Some(IconDecision::generated(
            IconMode::Color,
            color,
            registry.icon_path(id)?,
        )));
    }
    if auto_icon {
        let color = icon::color_for_id(id);
        return Ok(Some(IconDecision::generated(
            IconMode::Auto,
            color,
            registry.icon_path(id)?,
        )));
    }
    let Some(previous) = previous else {
        // A registration without metadata keeps the icon of the manifest. The
        // provenance is unknown, so it is never treated as TuiPWA owned.
        return Ok(None);
    };
    if !previous.mode.owned_by_tuipwa() {
        // A manually set icon and a system icon stay as they are.
        return Ok(None);
    }
    if !name_changed {
        // A category, cwd or argument update does not change the monogram, so
        // the existing PNG is kept instead of being rewritten.
        return Ok(None);
    }
    // TuiPWA owns the icon: refresh the monogram with the current name.
    let color = previous
        .color
        .as_deref()
        .and_then(|c| icon::parse_color(c).ok())
        .unwrap_or_else(|| icon::color_for_id(id));
    Ok(Some(IconDecision::generated(
        previous.mode,
        color,
        registry.icon_path(id)?,
    )))
}

fn run_apps(args: &[String], context: &Context) -> Result<i32> {
    if args.is_empty() {
        return usage_error("run には登録IDを指定してください");
    }
    let registry = &context.registry;
    let apps = args
        .iter()
        .map(|id| registry.load(id))
        .collect::<Result<Vec<_>>>()?;
    for app in apps {
        let ack = crate::platform::linux::runtime::launch(&crate::model::LaunchRequest::new(
            app,
            crate::platform::linux::runtime::request_id()?,
        ))?;
        println!("{}", serde_json::to_string(&ack)?);
        if let Some(e) = ack.error {
            return Err(e.into());
        }
    }
    Ok(0)
}

/// Resolves a command into an absolute path of an executable regular file.
fn resolve_command(command: &str, context: &Context) -> Result<(PathBuf, String)> {
    let path = if command.contains('/') {
        absolute(Path::new(command), &context.cwd)?
    } else {
        search_path(command, &context.path_dirs)?
    };
    let metadata =
        std::fs::metadata(&path).map_err(|_| format!("実行ファイルが見つかりません: {command}"))?;
    if !metadata.is_file() {
        return Err(format!("実行ファイルではありません: {}", path.display()).into());
    }
    use std::os::unix::fs::PermissionsExt;
    if metadata.permissions().mode() & 0o111 == 0 {
        return Err(format!("実行権限がありません: {}", path.display()).into());
    }
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("非UTF-8パス")?
        .to_owned();
    Ok((path, file_name))
}

fn search_path(command: &str, path_dirs: &[PathBuf]) -> Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    for dir in path_dirs {
        let candidate = dir.join(command);
        if let Ok(metadata) = std::fs::metadata(&candidate)
            && metadata.is_file()
            && metadata.permissions().mode() & 0o111 != 0
        {
            return Ok(candidate);
        }
    }
    Err(format!("PATH に見つかりません: {command}").into())
}

/// Derives a registration id from the executable file name.
fn derive_id(file_name: &str) -> Result<String> {
    let candidate = file_name.to_ascii_lowercase();
    if check_id(&candidate).is_err() {
        return Err(format!(
            "実行ファイル名からIDを生成できません: {file_name}（--id で指定してください）"
        )
        .into());
    }
    Ok(candidate)
}

fn absolute(path: &Path, base: &Path) -> Result<PathBuf> {
    Ok(crate::model::absolute(path, base))
}

fn is_usable_icon(path: &Path) -> bool {
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
