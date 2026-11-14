use crate::{
    Result, category,
    cli::{self, Context},
    icons::{self, render as icon},
    meta::{AppMeta, IconMode},
    model::*,
    registry::{Registry, entry},
};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "tuipwa-test-{}",
            crate::platform::linux::runtime::request_id().unwrap()
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn raw() -> serde_json::Value {
    serde_json::json!({"schemaVersion":1,"id":"editor","name":"日本語 Editor","mode":"native","command":"vim"})
}
fn load(temp: &Temp, value: &serde_json::Value) -> Result<Manifest> {
    let p = temp.path("app.json");
    fs::write(&p, serde_json::to_vec(value)?)?;
    Manifest::load(&p)
}

/// Creates an isolated environment for the command line interface.
fn context(temp: &Temp) -> Context {
    let config = temp.path("config");
    let data = temp.path("data");
    fs::create_dir_all(&config).unwrap();
    fs::create_dir_all(&data).unwrap();
    fs::create_dir_all(temp.path("home")).unwrap();
    Context {
        registry: Registry {
            manifests: config.join("tuipwa/apps"),
            meta: config.join("tuipwa/meta"),
            desktops: data.join("applications"),
            icons: data.join("tuipwa/icons"),
        },
        home: temp.path("home"),
        cwd: temp.0.clone(),
        launcher: temp.path("tuipwa"),
        path_dirs: vec![temp.path("bin")],
        data_dirs: vec![data.clone()],
    }
}

/// Creates an executable file and returns its path.
fn executable(temp: &Temp, name: &str) -> PathBuf {
    let dir = temp.path("bin");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| v.to_string()).collect()
}

#[test]
fn manifest_defaults_and_argv() {
    let t = Temp::new();
    let mut v = raw();
    v["arguments"] = serde_json::json!([
        "",
        "a b",
        "\"x\"",
        "$(touch /tmp/not-executed)",
        "日本語",
        "\n"
    ]);
    let m = load(&t, &v).unwrap();
    assert_eq!(m.title.as_deref(), Some("日本語 Editor"));
    assert_eq!(m.working_directory.as_deref(), t.0.to_str());
    assert_eq!(
        m.arguments,
        vec![
            "",
            "a b",
            "\"x\"",
            "$(touch /tmp/not-executed)",
            "日本語",
            "\n"
        ]
    );
}
#[test]
fn reject_v1_invalid_inputs() {
    let t = Temp::new();
    for (key, val) in [
        ("schemaVersion", serde_json::json!(2)),
        ("schemaVersion", serde_json::json!(1.0)),
        ("id", serde_json::json!("../x")),
        ("id", serde_json::json!("A")),
        ("name", serde_json::Value::Null),
        ("title", serde_json::json!("\n")),
        ("command", serde_json::json!(" ")),
        ("mode", serde_json::json!("wsl")),
        ("arguments", serde_json::Value::Null),
        ("arguments", serde_json::json!([null])),
        ("arguments", serde_json::json!(["\u{0}"])),
        ("unknown", serde_json::json!(1)),
        ("workingDirectory", serde_json::json!("missing")),
        ("icon", serde_json::json!("missing.png")),
    ] {
        let mut v = raw();
        v[key] = val;
        assert!(load(&t, &v).is_err(), "accepted {key}: {v}");
    }
}
#[test]
fn paths_are_lexical_and_relative_to_manifest() {
    let t = Temp::new();
    let mut v = raw();
    v["command"] = serde_json::json!("./missing/../vim");
    v["workingDirectory"] = serde_json::json!(".");
    let m = load(&t, &v).unwrap();
    assert_eq!(m.command, t.path("vim").to_str().unwrap());
    assert_eq!(absolute(Path::new("/../../x"), &t.0), Path::new("/x"));
}
#[test]
fn registry_no_overwrite_rollback_and_ownership() {
    let t = Temp::new();
    let m = load(&t, &raw()).unwrap();
    let r = Registry {
        manifests: t.path("apps"),
        meta: t.path("meta"),
        desktops: t.path("desktop"),
        icons: t.path("icons"),
    };
    let launcher = Path::new("/opt/a b/tuipwa");
    let meta = AppMeta::new(&m.id);
    r.install(&m, &meta, launcher).unwrap();
    assert!(r.install(&m, &meta, launcher).is_err());
    assert_eq!(r.list().unwrap().len(), 1);
    let desktop = r.desktops.join("tuipwa-editor.desktop");
    fs::write(&desktop, "external").unwrap();
    assert!(r.unregister("editor").is_err());
    fs::remove_file(&desktop).unwrap();
    r.unregister("editor").unwrap();
    fs::write(&desktop, "external").unwrap();
    assert!(r.install(&m, &meta, launcher).is_err());
    assert!(!r.manifests.join("editor.json").exists());
    assert_eq!(fs::read_to_string(&desktop).unwrap(), "external");
}
#[test]
fn desktop_escaping_and_identity() {
    let t = Temp::new();
    let m = load(&t, &raw()).unwrap();
    let s = entry(
        &m,
        &["Utility".to_owned()],
        Path::new("/opt/a b/\"$`%\\native"),
    )
    .unwrap();
    assert!(s.contains("StartupWMClass=tuipwa-editor\n"));
    assert!(s.contains("Categories=Utility;\n"));
    assert!(s.contains("%%"));
    assert!(s.contains("日本語\\sEditor"));
    assert!(entry(&m, &["Utility".to_owned()], Path::new("relative")).is_err());
    assert_ne!(desktop_id("a-b"), desktop_id("a.b"));
}
#[test]
fn utf8_bom_and_existing_examples() {
    let t = Temp::new();
    let path = t.path("bom.json");
    let mut bytes = vec![0xef, 0xbb, 0xbf];
    bytes.extend(serde_json::to_vec(&raw()).unwrap());
    fs::write(&path, bytes).unwrap();
    assert_eq!(Manifest::load(&path).unwrap().id, "editor");
    for path in [
        "examples/nano.json",
        "examples/btop.json",
        "examples/vim.json",
    ] {
        Manifest::load(Path::new(path)).unwrap();
    }
}

#[test]
fn duplicate_keys_match_typed_last_value_behavior() {
    let t = Temp::new();
    let path = t.path("duplicate.json");
    let valid = r#"{"schemaVersion":2,"schemaVersion":1,"id":null,"id":"editor","name":null,"name":"Editor","mode":"native","command":"vim","arguments":null,"arguments":[null],"arguments":["kept"]}"#;
    fs::write(&path, valid).unwrap();
    let app = Manifest::load(&path).unwrap();
    assert_eq!(app.arguments, ["kept"]);
    fs::write(&path, valid.replace("\"name\":null", "\"name\":23")).unwrap();
    assert!(
        Manifest::load(&path).is_err(),
        "an earlier type error must not be hidden"
    );
}

#[test]
fn request_roundtrip_keeps_argv() {
    let t = Temp::new();
    let m = load(&t, &raw()).unwrap();
    let req = LaunchRequest::new(m, "request".into());
    let copy: LaunchRequest = serde_json::from_slice(&serde_json::to_vec(&req).unwrap()).unwrap();
    assert_eq!(copy.protocol_version, 1);
    assert_eq!(copy.app_id, "editor");
    assert_eq!(copy.request_id, "request");
}

// --- command line interface -----------------------------------------------

#[test]
fn add_registers_command_from_path() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    let code = cli::execute(&args(&["add", "btop"]), &c).unwrap();
    assert_eq!(code, 0);
    let manifest = c.registry.manifest_path("btop").unwrap();
    assert!(manifest.is_file());
    let app = c.registry.load("btop").unwrap();
    assert_eq!(app.name, "btop");
    assert_eq!(app.title.as_deref(), Some("btop"));
    assert!(app.command.ends_with("/btop"));
    assert!(c.registry.desktop_path("btop").unwrap().is_file());
    // The working directory defaults to the home directory.
    assert_eq!(app.working_directory.as_deref(), c.home.to_str());
}

#[test]
fn add_registers_absolute_and_relative_paths() {
    let t = Temp::new();
    let c = context(&t);
    let path = executable(&t, "nano");
    assert_eq!(
        cli::execute(&args(&["add", path.to_str().unwrap()]), &c).unwrap(),
        0
    );
    assert!(c.registry.manifest_path("nano").unwrap().is_file());

    // A relative path is resolved against the current directory.
    let relative = Path::new("bin/relative-tui");
    fs::create_dir_all(t.path("bin")).unwrap();
    fs::write(t.0.join(relative), "#!/bin/sh\n").unwrap();
    fs::set_permissions(t.0.join(relative), fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        cli::execute(&args(&["add", relative.to_str().unwrap()]), &c).unwrap(),
        0
    );
    let app = c.registry.load("relative-tui").unwrap();
    assert_eq!(app.command, t.0.join(relative).to_str().unwrap());
}

#[test]
fn add_rejects_file_without_execute_permission() {
    let t = Temp::new();
    let c = context(&t);
    let path = t.path("not-executable");
    fs::write(&path, "text").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(cli::execute(&args(&["add", path.to_str().unwrap()]), &c).is_err());
    assert!(!c.registry.manifest_path("not-executable").unwrap().exists());
}

#[test]
fn add_rejects_invalid_and_duplicate_ids() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    assert!(
        cli::execute(&args(&["add", "btop", "--id", "../evil"]), &c).is_err(),
        "an invalid id must be rejected"
    );
    assert_eq!(cli::execute(&args(&["add", "btop"]), &c).unwrap(), 0);
    assert!(
        cli::execute(&args(&["add", "btop"]), &c).is_err(),
        "a duplicate id must be rejected"
    );
    // No serial number is appended automatically.
    assert!(!c.registry.manifest_path("btop-2").unwrap().exists());
}

#[test]
fn add_keeps_arguments_verbatim() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    let argv = [
        "日本語",
        "空白 を含む",
        "\"quoted\"",
        "$(touch /tmp/tuipwa-not-executed)",
        "*",
        "--flag",
    ];
    let mut command = args(&["add", "btop"]);
    command.push("--".into());
    command.extend(argv.iter().map(|v| v.to_string()));
    assert_eq!(cli::execute(&command, &c).unwrap(), 0);
    let app = c.registry.load("btop").unwrap();
    assert_eq!(app.arguments, argv);
    assert!(!Path::new("/tmp/tuipwa-not-executed").exists());
}

#[test]
fn add_from_manifest_keeps_v1_semantics() {
    let t = Temp::new();
    let c = context(&t);
    let mut v = raw();
    v["id"] = serde_json::json!("editor");
    v["command"] = serde_json::json!("./vim");
    v["workingDirectory"] = serde_json::json!(".");
    let path = t.path("app.json");
    fs::write(&path, serde_json::to_vec(&v).unwrap()).unwrap();
    assert_eq!(
        cli::execute(&args(&["add", "--manifest", path.to_str().unwrap()]), &c).unwrap(),
        0
    );
    let app = c.registry.load("editor").unwrap();
    // Paths stay relative to the directory of the manifest.
    assert_eq!(app.command, t.path("vim").to_str().unwrap());
    assert_eq!(app.working_directory.as_deref(), t.0.to_str());
}

#[test]
fn add_manifest_conflicts_with_direct_options() {
    let t = Temp::new();
    let c = context(&t);
    let path = t.path("app.json");
    fs::write(&path, serde_json::to_vec(&raw()).unwrap()).unwrap();
    for extra in [
        vec!["add", "--manifest", "APP", "--name", "x"],
        vec!["add", "--manifest", "APP", "--id", "x"],
        vec!["add", "--manifest", "APP", "--color", "#2563EB"],
        vec!["add", "--manifest", "APP", "vim"],
    ] {
        let mut command = args(&extra);
        for value in command.iter_mut() {
            if value == "APP" {
                *value = path.to_str().unwrap().to_owned();
            }
        }
        assert_eq!(
            cli::execute(&command, &c).unwrap(),
            2,
            "conflicting options must be rejected: {extra:?}"
        );
    }
}

#[test]
fn show_reports_registration_details() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    let mut command = args(&["add", "btop", "--name", "System Monitor"]);
    command.push("--".into());
    command.push("--flag".into());
    assert_eq!(cli::execute(&command, &c).unwrap(), 0);
    let app = c.registry.load("btop").unwrap();
    let meta = AppMeta::load(&c.registry.meta_path("btop").unwrap())
        .unwrap()
        .unwrap();
    let desktop = c.registry.desktop_path("btop").unwrap();
    let text = cli::describe(&app, Some(&meta), &desktop).unwrap();
    assert!(text.contains("ID: btop"));
    assert!(text.contains("表示名: System Monitor"));
    assert!(text.contains("ウィンドウタイトル: System Monitor"));
    assert!(text.contains("実行コマンド: "));
    assert!(text.contains("引数: --flag"));
    assert!(text.contains("作業ディレクトリ: "));
    assert!(text.contains("アイコン設定方式: "));
    assert!(text.contains("アイコン参照先: "));
    assert!(text.contains(&desktop.display().to_string()));
    // The command itself works as well.
    assert_eq!(cli::execute(&args(&["show", "btop"]), &c).unwrap(), 0);
}

#[test]
fn update_changes_only_the_given_fields() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    assert_eq!(cli::execute(&args(&["add", "btop"]), &c).unwrap(), 0);
    let mut command = args(&["update", "btop", "--name", "System Monitor"]);
    command.push("--".into());
    command.push("/home/user/notes.txt".into());
    assert_eq!(cli::execute(&command, &c).unwrap(), 0);
    let app = c.registry.load("btop").unwrap();
    assert_eq!(app.name, "System Monitor");
    assert_eq!(app.arguments, ["/home/user/notes.txt"]);
    // The title was derived from the name, so it follows.
    assert_eq!(app.title.as_deref(), Some("System Monitor"));
    // The command is unchanged.
    assert!(app.command.ends_with("/btop"));
    // The desktop entry follows the new name.
    let desktop = fs::read_to_string(c.registry.desktop_path("btop").unwrap()).unwrap();
    assert!(desktop.contains("Name=System\\sMonitor"));
}

#[test]
fn update_clear_args_and_replacement() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    let mut command = args(&["add", "btop"]);
    command.push("--".into());
    command.push("first".into());
    assert_eq!(cli::execute(&command, &c).unwrap(), 0);
    assert_eq!(
        cli::execute(&args(&["update", "btop", "--clear-args"]), &c).unwrap(),
        0
    );
    assert!(c.registry.load("btop").unwrap().arguments.is_empty());
    let mut command = args(&["update", "btop"]);
    command.push("--".into());
    command.push("second".into());
    command.push("third".into());
    assert_eq!(cli::execute(&command, &c).unwrap(), 0);
    assert_eq!(
        c.registry.load("btop").unwrap().arguments,
        ["second", "third"]
    );
}

#[test]
fn update_rejects_conflicting_options() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    assert_eq!(cli::execute(&args(&["add", "btop"]), &c).unwrap(), 0);
    let icon = t.path("icon.png");
    crate::icons::png::write_png(&icon, &[0u8; 4], 1, 1).unwrap();
    for command in [
        vec!["update", "btop", "--icon", "ICON", "--color", "#2563EB"],
        vec!["update", "btop", "--auto-icon", "--color", "#2563EB"],
        vec!["update", "btop", "--auto-icon", "--icon", "ICON"],
    ] {
        let mut values = args(&command);
        for value in values.iter_mut() {
            if value == "ICON" {
                *value = icon.to_str().unwrap().to_owned();
            }
        }
        assert_eq!(
            cli::execute(&values, &c).unwrap(),
            2,
            "conflicting options must be rejected: {command:?}"
        );
    }
    let mut values = args(&["update", "btop", "--clear-args"]);
    values.push("--".into());
    values.push("x".into());
    assert_eq!(cli::execute(&values, &c).unwrap(), 2);
}

#[test]
fn update_changes_the_command_and_the_working_directory() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    let other = executable(&t, "htop");
    assert_eq!(cli::execute(&args(&["add", "btop"]), &c).unwrap(), 0);
    assert_eq!(
        cli::execute(
            &args(&["update", "btop", "--command", other.to_str().unwrap()]),
            &c
        )
        .unwrap(),
        0
    );
    let app = c.registry.load("btop").unwrap();
    assert_eq!(app.command, other.to_str().unwrap());
    // A command that does not exist is rejected.
    assert!(
        cli::execute(
            &args(&["update", "btop", "--command", "/nonexistent/tuipwa-command"]),
            &c
        )
        .is_err()
    );
    // The working directory is changed and has to exist.
    let cwd = t.path("cwd");
    fs::create_dir_all(&cwd).unwrap();
    let mut command = args(&["update", "btop", "--cwd"]);
    command.push(cwd.to_str().unwrap().to_owned());
    assert_eq!(cli::execute(&command, &c).unwrap(), 0);
    assert_eq!(
        c.registry
            .load("btop")
            .unwrap()
            .working_directory
            .as_deref(),
        Some(cwd.to_str().unwrap())
    );
    assert!(cli::execute(&args(&["update", "btop", "--cwd", "/nonexistent/dir"]), &c).is_err());
}

#[test]
fn a_registration_created_by_the_cli_is_valid_for_the_dotnet_reader() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    let mut command = args(&["add", "btop", "--name", "日本語 Editor"]);
    command.push("--".into());
    command.push("引数 付き".into());
    assert_eq!(cli::execute(&command, &c).unwrap(), 0);
    let path = c.registry.manifest_path("btop").unwrap();
    let bytes = fs::read(&path).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    // The rules of src/TuiPWA.Core/AppManifest.cs.
    assert_eq!(value["schemaVersion"], 1);
    assert!(check_id(value["id"].as_str().unwrap()).is_ok());
    assert!(!value["name"].as_str().unwrap().trim().is_empty());
    assert_eq!(value["mode"], "native");
    assert!(!value["command"].as_str().unwrap().trim().is_empty());
    assert!(value["arguments"].is_array());
    assert!(value["workingDirectory"].as_str().unwrap().starts_with('/'));
    let icon = PathBuf::from(value["icon"].as_str().unwrap());
    assert!(icon.is_file());
    assert_eq!(icon.extension().and_then(|e| e.to_str()), Some("png"));
}

#[test]
fn update_rejects_an_id_change() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    assert_eq!(cli::execute(&args(&["add", "btop"]), &c).unwrap(), 0);
    assert_eq!(
        cli::execute(&args(&["update", "btop", "--id", "other"]), &c).unwrap(),
        2,
        "the id of a registration must not change"
    );
    assert!(c.registry.manifest_path("btop").unwrap().is_file());
    assert!(!c.registry.manifest_path("other").unwrap().exists());
}

#[test]
fn remove_deletes_registration_and_generated_icon() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    assert_eq!(cli::execute(&args(&["add", "btop"]), &c).unwrap(), 0);
    let icon = c.registry.icon_path("btop").unwrap();
    assert!(icon.is_file());
    assert_eq!(cli::execute(&args(&["remove", "btop"]), &c).unwrap(), 0);
    assert!(!c.registry.manifest_path("btop").unwrap().exists());
    assert!(!c.registry.desktop_path("btop").unwrap().exists());
    assert!(!icon.exists());
    assert!(!c.registry.meta_path("btop").unwrap().exists());
    assert!(cli::execute(&args(&["remove", "btop"]), &c).is_err());
}

#[test]
fn remove_keeps_a_user_supplied_image() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    let image = t.path("btop.png");
    crate::icons::png::write_png(&image, &[0u8; 4], 1, 1).unwrap();
    let mut command = args(&["add", "btop", "--icon"]);
    command.push(image.to_str().unwrap().to_owned());
    assert_eq!(cli::execute(&command, &c).unwrap(), 0);
    assert_eq!(cli::execute(&args(&["remove", "btop"]), &c).unwrap(), 0);
    assert!(image.is_file(), "a user image must not be deleted");
}

#[test]
fn externally_replaced_desktop_entry_is_protected() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    assert_eq!(cli::execute(&args(&["add", "btop"]), &c).unwrap(), 0);
    let desktop = c.registry.desktop_path("btop").unwrap();
    fs::write(&desktop, "[Desktop Entry]\nName=other\n").unwrap();
    assert!(
        cli::execute(&args(&["remove", "btop"]), &c).is_err(),
        "an externally replaced entry must not be deleted"
    );
    assert!(desktop.is_file());
    assert!(
        cli::execute(&args(&["update", "btop", "--name", "x"]), &c).is_err(),
        "an externally replaced entry must not be overwritten"
    );
}

#[test]
fn failed_update_keeps_the_registration_consistent() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    assert_eq!(cli::execute(&args(&["add", "btop"]), &c).unwrap(), 0);
    let manifest = c.registry.manifest_path("btop").unwrap();
    let before = fs::read_to_string(&manifest).unwrap();
    let icon = c.registry.icon_path("btop").unwrap();
    let icon_before = fs::read(&icon).unwrap();
    // The desktop entry cannot be written, so the update has to be rolled back.
    let desktop = c.registry.desktop_path("btop").unwrap();
    fs::remove_file(&desktop).unwrap();
    fs::create_dir_all(&desktop).unwrap();
    assert!(cli::execute(&args(&["update", "btop", "--name", "x"]), &c).is_err());
    assert_eq!(fs::read_to_string(&manifest).unwrap(), before);
    // The regenerated icon is restored as well.
    assert_eq!(fs::read(&icon).unwrap(), icon_before);
    let meta = AppMeta::load(&c.registry.meta_path("btop").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(meta.id, "btop");
}

#[test]
fn list_and_validate_work_for_registered_apps() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    assert_eq!(cli::execute(&args(&["add", "btop"]), &c).unwrap(), 0);
    assert_eq!(cli::execute(&args(&["list"]), &c).unwrap(), 0);
    assert_eq!(cli::execute(&args(&["list", "extra"]), &c).unwrap(), 2);
    let path = t.path("app.json");
    fs::write(&path, serde_json::to_vec(&raw()).unwrap()).unwrap();
    assert_eq!(
        cli::execute(&args(&["validate", path.to_str().unwrap()]), &c).unwrap(),
        0
    );
    assert_eq!(cli::execute(&args(&["validate"]), &c).unwrap(), 2);
}

#[test]
fn unknown_command_and_option_are_usage_errors() {
    let t = Temp::new();
    let c = context(&t);
    assert_eq!(cli::execute(&args(&["frobnicate"]), &c).unwrap(), 2);
    assert_eq!(
        cli::execute(&args(&["add", "btop", "--nope", "x"]), &c).unwrap(),
        2
    );
    assert_eq!(cli::execute(&args(&["add"]), &c).unwrap(), 2);
    assert_eq!(cli::execute(&args(&["update"]), &c).unwrap(), 2);
    assert_eq!(cli::execute(&args(&["show"]), &c).unwrap(), 2);
    assert_eq!(cli::execute(&args(&["remove"]), &c).unwrap(), 2);
}

#[test]
fn help_does_not_expose_the_internal_session_argument() {
    let t = Temp::new();
    let c = context(&t);
    assert_eq!(cli::execute(&args(&["--help"]), &c).unwrap(), 0);
    for locale in ["C", "ja_JP.UTF-8"] {
        assert!(!cli::usage_for_locale(Some(locale), None, None).contains("__session"));
    }
}

#[test]
fn help_selects_language_using_locale_precedence() {
    let english = cli::usage_for_locale(None, None, None);
    let japanese = cli::usage_for_locale(None, None, Some("ja_JP.UTF-8"));
    assert!(english.starts_with("Usage:"));
    assert!(japanese.starts_with("使用法:"));
    for locale in ["ja", "ja_JP.UTF-8", "ja-JP", "ja_JP@modifier"] {
        assert_eq!(cli::usage_for_locale(Some(locale), None, None), japanese);
    }
    for locale in [
        "C",
        "C.UTF-8",
        "POSIX",
        "en_US.UTF-8",
        "fr_FR.UTF-8",
        "jargon",
    ] {
        assert_eq!(
            cli::usage_for_locale(Some(locale), None, Some("ja")),
            english
        );
    }
    assert_eq!(
        cli::usage_for_locale(Some(""), Some("ja"), Some("en")),
        japanese
    );
    assert_eq!(
        cli::usage_for_locale(Some("ja"), Some("en"), Some("en")),
        japanese
    );
    assert_eq!(cli::usage_for_locale(None, Some("en"), Some("ja")), english);
    assert_eq!(
        cli::usage_for_locale(Some(""), Some(""), Some("ja")),
        japanese
    );
    assert_eq!(cli::usage_for_locale(Some(""), Some(""), Some("")), english);
    let flags = |help: &str| {
        help.split_whitespace()
            .filter(|word| word.starts_with("--"))
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    assert_eq!(flags(english), flags(japanese));
}

// --- categories -----------------------------------------------------------

fn list(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

#[test]
fn categories_are_validated_and_normalized() {
    assert_eq!(
        category::normalize(&list(&["Utility"])).unwrap(),
        ["Utility"]
    );
    // The order is kept and repeated names are dropped.
    assert_eq!(
        category::normalize(&list(&["System", "Monitor", "System"])).unwrap(),
        ["System", "Monitor"]
    );
    assert_eq!(
        category::normalize(&list(&["Utility", "X-MyApp"])).unwrap(),
        ["Utility", "X-MyApp"]
    );
    assert_eq!(
        category::normalize(&list(&["AudioVideo", "Audio"])).unwrap(),
        ["AudioVideo", "Audio"]
    );
    for bad in [
        &["Monitor"][..],                // no main category
        &["System;Monitor"][..],         // desktop separator
        &["System\nMonitor"][..],        // line break
        &["System Monitor"][..],         // whitespace
        &[""][..],                       // empty
        &["monitor"][..],                // category names are case sensitive
        &["X-"][..],                     // empty vendor name
        &["Audio"][..],                  // Audio requires AudioVideo
        &["Screensaver", "Utility"][..], // reserved, needs OnlyShowIn
        &["NoSuchCategory"][..],         // unknown without X- prefix
    ] {
        assert!(
            category::normalize(&list(bad)).is_err(),
            "must be rejected: {bad:?}"
        );
    }
}

#[test]
fn desktop_entry_uses_the_configured_categories() {
    let t = Temp::new();
    let m = load(&t, &raw()).unwrap();
    let launcher = Path::new("/opt/tuipwa");
    let text = entry(&m, &list(&["System", "Monitor"]), launcher).unwrap();
    assert!(text.contains("Categories=System;Monitor;\n"));
    assert!(!text.contains("Utility"));
    // Metadata is editable, so the entry validates again before writing.
    assert!(entry(&m, &list(&["System;Hidden=true"]), launcher).is_err());
    assert!(entry(&m, &list(&["Monitor"]), launcher).is_err());
}

#[test]
fn add_uses_and_shows_the_categories() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    assert_eq!(cli::execute(&args(&["add", "btop"]), &c).unwrap(), 0);
    let meta = AppMeta::load(&c.registry.meta_path("btop").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(meta.categories(), ["Utility"]);
    let desktop = fs::read_to_string(c.registry.desktop_path("btop").unwrap()).unwrap();
    assert!(desktop.contains("Categories=Utility;\n"));

    executable(&t, "vim");
    assert_eq!(
        cli::execute(
            &args(&[
                "add",
                "vim",
                "--category",
                "System",
                "--category",
                "Monitor"
            ]),
            &c
        )
        .unwrap(),
        0
    );
    let desktop_path = c.registry.desktop_path("vim").unwrap();
    let desktop = fs::read_to_string(&desktop_path).unwrap();
    assert!(desktop.contains("Categories=System;Monitor;\n"));
    // An explicit main category is not mixed with the default.
    assert!(!desktop.contains("Utility"));
    let app = c.registry.load("vim").unwrap();
    let meta = AppMeta::load(&c.registry.meta_path("vim").unwrap())
        .unwrap()
        .unwrap();
    assert!(
        cli::describe(&app, Some(&meta), &desktop_path)
            .unwrap()
            .contains("カテゴリ: System;Monitor;")
    );
}

#[test]
fn update_replaces_keeps_and_resets_categories() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    assert_eq!(
        cli::execute(
            &args(&[
                "add",
                "btop",
                "--category",
                "System",
                "--category",
                "Monitor"
            ]),
            &c
        )
        .unwrap(),
        0
    );
    let desktop = c.registry.desktop_path("btop").unwrap();
    // An update without --category keeps the stored list.
    assert_eq!(
        cli::execute(&args(&["update", "btop", "--name", "Monitor"]), &c).unwrap(),
        0
    );
    assert!(
        fs::read_to_string(&desktop)
            .unwrap()
            .contains("Categories=System;Monitor;\n")
    );
    let icon = c.registry.icon_path("btop").unwrap();
    let icon_before = fs::read(&icon).unwrap();
    // --category replaces the whole list and normalizes duplicates.
    assert_eq!(
        cli::execute(
            &args(&[
                "update",
                "btop",
                "--category",
                "Development",
                "--category",
                "Development",
                "--category",
                "Debugger"
            ]),
            &c
        )
        .unwrap(),
        0
    );
    assert!(
        fs::read_to_string(&desktop)
            .unwrap()
            .contains("Categories=Development;Debugger;\n")
    );
    // --reset-categories returns to the default.
    assert_eq!(
        cli::execute(&args(&["update", "btop", "--reset-categories"]), &c).unwrap(),
        0
    );
    assert!(
        fs::read_to_string(&desktop)
            .unwrap()
            .contains("Categories=Utility;\n")
    );
    // A category-only update keeps the existing generated icon.
    assert_eq!(fs::read(&icon).unwrap(), icon_before);
}

#[test]
fn invalid_categories_are_rejected_before_writing() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    assert_eq!(cli::execute(&args(&["add", "btop"]), &c).unwrap(), 0);
    let desktop = c.registry.desktop_path("btop").unwrap();
    let before = fs::read_to_string(&desktop).unwrap();
    for bad in ["Monitor", "System;Hidden=true", "System\nMonitor", "X-", ""] {
        assert!(
            cli::execute(&args(&["update", "btop", "--category", bad]), &c).is_err(),
            "must be rejected: {bad:?}"
        );
        assert_eq!(fs::read_to_string(&desktop).unwrap(), before);
    }
    assert_eq!(
        cli::execute(
            &args(&[
                "update",
                "btop",
                "--reset-categories",
                "--category",
                "System"
            ]),
            &c
        )
        .unwrap(),
        2
    );
    // A failed `add` leaves no registration behind.
    executable(&t, "htop");
    assert!(cli::execute(&args(&["add", "htop", "--category", "Monitor"]), &c).is_err());
    assert!(!c.registry.manifest_path("htop").unwrap().exists());
    assert!(!c.registry.desktop_path("htop").unwrap().exists());
}

#[test]
fn manifest_import_uses_the_default_categories() {
    let t = Temp::new();
    let c = context(&t);
    let path = t.path("app.json");
    fs::write(&path, serde_json::to_vec(&raw()).unwrap()).unwrap();
    assert_eq!(
        cli::execute(&args(&["add", "--manifest", path.to_str().unwrap()]), &c).unwrap(),
        0
    );
    let meta = AppMeta::load(&c.registry.meta_path("editor").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(meta.categories(), ["Utility"]);
    let desktop = fs::read_to_string(c.registry.desktop_path("editor").unwrap()).unwrap();
    assert!(desktop.contains("Categories=Utility;\n"));
}

#[test]
fn older_metadata_without_categories_keeps_working() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    assert_eq!(cli::execute(&args(&["add", "btop"]), &c).unwrap(), 0);
    let path = c.registry.meta_path("btop").unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value.as_object_mut().unwrap().remove("categories");
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let meta = AppMeta::load(&path).unwrap().unwrap();
    assert!(meta.categories.is_none());
    assert_eq!(meta.categories(), ["Utility"]);
    // The old file stays readable and the category update still works.
    assert_eq!(
        cli::execute(&args(&["update", "btop", "--category", "System"]), &c).unwrap(),
        0
    );
    let meta = AppMeta::load(&path).unwrap().unwrap();
    assert_eq!(meta.categories(), ["System"]);
}

#[test]
fn category_update_does_not_regenerate_the_icon() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    assert_eq!(cli::execute(&args(&["add", "btop"]), &c).unwrap(), 0);
    let icon = c.registry.icon_path("btop").unwrap();
    fs::remove_file(&icon).unwrap();
    assert_eq!(
        cli::execute(&args(&["update", "btop", "--category", "System"]), &c).unwrap(),
        0
    );
    // A category-only update must not rewrite the monogram.
    assert!(!icon.exists());
}

#[test]
fn failed_category_update_restores_the_metadata() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    assert_eq!(cli::execute(&args(&["add", "btop"]), &c).unwrap(), 0);
    let meta_path = c.registry.meta_path("btop").unwrap();
    let before = fs::read(&meta_path).unwrap();
    // Make the desktop entry unwritable so the update fails after the
    // metadata was prepared.
    let desktop = c.registry.desktop_path("btop").unwrap();
    fs::remove_file(&desktop).unwrap();
    fs::create_dir_all(&desktop).unwrap();
    assert!(cli::execute(&args(&["update", "btop", "--category", "System"]), &c).is_err());
    assert_eq!(fs::read(&meta_path).unwrap(), before);
}

#[test]
fn remove_deletes_the_category_metadata() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    assert_eq!(
        cli::execute(&args(&["add", "btop", "--category", "System"]), &c).unwrap(),
        0
    );
    let meta = c.registry.meta_path("btop").unwrap();
    assert!(meta.is_file());
    assert_eq!(cli::execute(&args(&["remove", "btop"]), &c).unwrap(), 0);
    assert!(!meta.exists());
}

// --- icons ----------------------------------------------------------------

#[test]
fn same_id_yields_the_same_color() {
    for id in ["btop", "editor", "a", "日本語"] {
        assert_eq!(icon::color_for_id(id), icon::color_for_id(id));
    }
    // A different id may pick the same palette entry, that is allowed.
    assert_eq!(icon::color_for_id("btop"), icon::color_for_id("btop"));
    // The hash is case sensitive, so different ids differ.
    let mut seen = std::collections::HashSet::new();
    for id in ["btop", "BTOP", "editor", "EDITOR"] {
        seen.insert(icon::color_for_id(id).hex());
    }
    assert!(seen.len() >= 2);
}

#[test]
fn palette_colors_stay_within_a_readable_range() {
    for color in icon::PALETTE {
        let luminance = color.luminance();
        assert!(
            (0.02..0.6).contains(&luminance),
            "palette color {} is too extreme",
            color.hex()
        );
        let (text, outline) = icon::text_colors(&color);
        assert!(
            text.contrast(&color) >= 4.5,
            "text {} on {} has no contrast",
            text.hex(),
            color.hex()
        );
        assert!(text.contrast(&outline) >= 4.5);
    }
}

#[test]
fn color_codes_are_validated() {
    assert!(icon::parse_color("#2563EB").is_ok());
    assert!(icon::parse_color("#2563eb").is_ok());
    for value in ["2563EB", "#2563E", "#2563EBB", "#GG63EB", "red", ""] {
        assert!(
            icon::parse_color(value).is_err(),
            "accepted an invalid color: {value}"
        );
    }
}

#[test]
fn generated_icon_is_a_valid_png() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    assert_eq!(cli::execute(&args(&["add", "btop"]), &c).unwrap(), 0);
    let icon = c.registry.icon_path("btop").unwrap();
    let (pixels, width, height) = crate::icons::png::read_png(&icon).unwrap();
    assert_eq!((width, height), (icon::SIZE, icon::SIZE));
    assert_eq!(pixels.len(), (width * height * 4) as usize);
    // The file starts with the PNG signature.
    let bytes = fs::read(&icon).unwrap();
    assert_eq!(
        &bytes[..8],
        &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]
    );
}

#[test]
fn icon_is_legible_on_light_and_dark_backgrounds() {
    // The text color contrasts with the background and the outline has the
    // opposite tone, so the glyph stays visible on both taskbar styles.
    for background in [
        icon::Rgb::new(0xF9, 0xFA, 0xFB),
        icon::Rgb::new(0x11, 0x18, 0x27),
        icon::Rgb::new(0x25, 0x63, 0xEB),
        icon::Rgb::new(0xCA, 0x8A, 0x04),
    ] {
        let (text, outline) = icon::text_colors(&background);
        assert!(text.contrast(&background) >= 4.5);
        assert!(text.contrast(&outline) >= 4.5);
    }
}

#[test]
fn the_rendered_icon_contains_text_and_outline_pixels() {
    for background in [
        icon::Rgb::new(0x11, 0x18, 0x27),
        icon::Rgb::new(0xF9, 0xFA, 0xFB),
    ] {
        let pixels = icon::render("BT", background);
        let (text, outline) = icon::text_colors(&background);
        let mut has_text = false;
        let mut has_outline = false;
        let mut has_background = false;
        for pixel in pixels.chunks(4) {
            let color = icon::Rgb::new(pixel[0], pixel[1], pixel[2]);
            if color == text {
                has_text = true;
            } else if color == outline {
                has_outline = true;
            } else if color == background {
                has_background = true;
            }
        }
        assert!(has_text, "the glyph has to be drawn");
        assert!(has_outline, "the glyph has to have an outline");
        assert!(has_background, "the tile has to be drawn");
    }
}

#[test]
fn monogram_is_taken_from_the_display_name() {
    assert_eq!(icon::monogram("btop"), "BT");
    assert_eq!(icon::monogram("System Monitor"), "SM");
    assert_eq!(icon::monogram("日本語 Editor"), "日E");
    assert_eq!(icon::monogram("---"), "-");
}

#[test]
fn explicit_icon_suppresses_the_generation() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    let image = t.path("custom.png");
    crate::icons::png::write_png(&image, &[0u8; 4], 1, 1).unwrap();
    let mut command = args(&["add", "btop", "--icon"]);
    command.push(image.to_str().unwrap().to_owned());
    assert_eq!(cli::execute(&command, &c).unwrap(), 0);
    assert!(
        !c.registry.icon_path("btop").unwrap().exists(),
        "no icon must be generated when one is given"
    );
    let meta = AppMeta::load(&c.registry.meta_path("btop").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(meta.icon().unwrap().mode, IconMode::Explicit);
    assert_eq!(
        meta.icon().unwrap().path.as_deref(),
        Some(image.to_str().unwrap())
    );
    assert!(!meta.owns_icon());
}

#[test]
fn color_suppresses_the_system_icon_search() {
    let t = Temp::new();
    let c = context(&t);
    // A desktop entry of the system that provides an icon.
    let applications = c.data_dirs[0].join("applications");
    fs::create_dir_all(&applications).unwrap();
    let icon_dir = c.data_dirs[0].join("icons/hicolor/48x48/apps");
    fs::create_dir_all(&icon_dir).unwrap();
    let image = icon_dir.join("btop.png");
    crate::icons::png::write_png(&image, &[0u8; 4], 1, 1).unwrap();
    fs::write(
        applications.join("btop.desktop"),
        "[Desktop Entry]\nExec=/usr/bin/btop\nIcon=btop\n",
    )
    .unwrap();
    executable(&t, "btop");

    // Without a color the system icon is used.
    assert_eq!(cli::execute(&args(&["add", "btop"]), &c).unwrap(), 0);
    let meta = AppMeta::load(&c.registry.meta_path("btop").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(meta.icon().unwrap().mode, IconMode::System);
    assert!(!meta.owns_icon());

    // With a color the system icon is not used.
    assert_eq!(cli::execute(&args(&["remove", "btop"]), &c).unwrap(), 0);
    assert_eq!(
        cli::execute(&args(&["add", "btop", "--color", "#2563EB"]), &c).unwrap(),
        0
    );
    let meta = AppMeta::load(&c.registry.meta_path("btop").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(meta.icon().unwrap().mode, IconMode::Color);
    assert_eq!(meta.icon().unwrap().color.as_deref(), Some("#2563EB"));
    assert!(c.registry.icon_path("btop").unwrap().is_file());
}

#[test]
fn update_color_regenerates_the_icon() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    assert_eq!(cli::execute(&args(&["add", "btop"]), &c).unwrap(), 0);
    let icon = c.registry.icon_path("btop").unwrap();
    let before = fs::read(&icon).unwrap();
    assert_eq!(
        cli::execute(&args(&["update", "btop", "--color", "#0F766E"]), &c).unwrap(),
        0
    );
    let meta = AppMeta::load(&c.registry.meta_path("btop").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(meta.icon().unwrap().mode, IconMode::Color);
    assert_eq!(meta.icon().unwrap().color.as_deref(), Some("#0F766E"));
    let after = fs::read(&icon).unwrap();
    assert_ne!(before, after, "the icon has to be generated again");
    // The manifest points at the generated file.
    let app = c.registry.load("btop").unwrap();
    assert_eq!(app.icon.as_deref(), Some(icon.to_str().unwrap()));
}

#[test]
fn update_name_updates_the_monogram() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    assert_eq!(cli::execute(&args(&["add", "btop"]), &c).unwrap(), 0);
    let icon = c.registry.icon_path("btop").unwrap();
    let before = fs::read(&icon).unwrap();
    assert_eq!(
        cli::execute(&args(&["update", "btop", "--name", "My Notes"]), &c).unwrap(),
        0
    );
    let after = fs::read(&icon).unwrap();
    assert_ne!(before, after, "the monogram has to be regenerated");
}

#[test]
fn update_auto_icon_switches_back_to_the_generated_icon() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    let image = t.path("custom.png");
    crate::icons::png::write_png(&image, &[0u8; 4], 1, 1).unwrap();
    let mut command = args(&["add", "btop", "--icon"]);
    command.push(image.to_str().unwrap().to_owned());
    assert_eq!(cli::execute(&command, &c).unwrap(), 0);
    assert_eq!(
        cli::execute(&args(&["update", "btop", "--auto-icon"]), &c).unwrap(),
        0
    );
    let meta = AppMeta::load(&c.registry.meta_path("btop").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(meta.icon().unwrap().mode, IconMode::Auto);
    assert!(c.registry.icon_path("btop").unwrap().is_file());
    // The user image is still there.
    assert!(image.is_file());
}

#[test]
fn update_does_not_replace_a_manual_icon() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    let image = t.path("custom.png");
    crate::icons::png::write_png(&image, &[0u8; 4], 1, 1).unwrap();
    let mut command = args(&["add", "btop", "--icon"]);
    command.push(image.to_str().unwrap().to_owned());
    assert_eq!(cli::execute(&command, &c).unwrap(), 0);
    assert_eq!(
        cli::execute(&args(&["update", "btop", "--name", "Other"]), &c).unwrap(),
        0
    );
    let app = c.registry.load("btop").unwrap();
    assert_eq!(app.icon.as_deref(), Some(image.to_str().unwrap()));
    assert!(!c.registry.icon_path("btop").unwrap().exists());
}

// --- compatibility and safety ---------------------------------------------

#[test]
fn existing_v1_manifest_is_accepted_unchanged() {
    let t = Temp::new();
    let c = context(&t);
    // A registration of the .NET implementation, without any metadata.
    let manifest = serde_json::json!({
        "schemaVersion": 1,
        "id": "legacy",
        "name": "Legacy",
        "icon": null,
        "title": "Legacy",
        "mode": "native",
        "command": "/usr/bin/legacy",
        "arguments": [],
        "workingDirectory": "/tmp"
    });
    let path = c.registry.manifest_path("legacy").unwrap();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    let app = c.registry.load("legacy").unwrap();
    assert_eq!(app.id, "legacy");
    assert_eq!(app.command, "/usr/bin/legacy");
    // The commands work without metadata.
    assert_eq!(cli::execute(&args(&["list"]), &c).unwrap(), 0);
    assert_eq!(cli::execute(&args(&["show", "legacy"]), &c).unwrap(), 0);
    assert_eq!(cli::execute(&args(&["remove", "legacy"]), &c).unwrap(), 0);
}

#[test]
fn unknown_manifest_fields_are_rejected() {
    let t = Temp::new();
    let mut v = raw();
    v["iconColor"] = serde_json::json!("#2563EB");
    assert!(load(&t, &v).is_err());
}

#[test]
fn a_written_manifest_uses_only_the_v1_fields() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    assert_eq!(cli::execute(&args(&["add", "btop"]), &c).unwrap(), 0);
    let bytes = fs::read(c.registry.manifest_path("btop").unwrap()).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let object = value.as_object().unwrap();
    // The .NET implementation rejects unknown fields, so the set of fields
    // has to stay exactly the manifest v1 one.
    let mut names: Vec<&str> = object.keys().map(String::as_str).collect();
    names.sort();
    assert_eq!(
        names,
        [
            "arguments",
            "command",
            "icon",
            "id",
            "mode",
            "name",
            "schemaVersion",
            "title",
            "workingDirectory"
        ]
    );
    // The stored manifest is readable by the manifest v1 loader.
    let m = Manifest::load(&c.registry.manifest_path("btop").unwrap()).unwrap();
    assert_eq!(m.id, "btop");
    assert_eq!(m.schema_version, 1);
    // The icon metadata lives in its own file and not in the manifest.
    assert!(
        !c.registry
            .meta_path("btop")
            .unwrap()
            .to_str()
            .unwrap()
            .is_empty()
    );
    let meta_bytes = fs::read(c.registry.meta_path("btop").unwrap()).unwrap();
    let meta: serde_json::Value = serde_json::from_slice(&meta_bytes).unwrap();
    assert!(meta.get("iconColor").is_none());
}

#[test]
fn an_existing_registration_is_not_overwritten() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    assert_eq!(cli::execute(&args(&["add", "btop"]), &c).unwrap(), 0);
    let manifest = c.registry.manifest_path("btop").unwrap();
    let before = fs::read_to_string(&manifest).unwrap();
    assert!(cli::execute(&args(&["add", "btop"]), &c).is_err());
    assert_eq!(fs::read_to_string(&manifest).unwrap(), before);
}

#[test]
fn a_missing_icon_file_does_not_break_the_registration() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    assert_eq!(cli::execute(&args(&["add", "btop"]), &c).unwrap(), 0);
    fs::remove_file(c.registry.icon_path("btop").unwrap()).unwrap();
    assert_eq!(cli::execute(&args(&["list"]), &c).unwrap(), 0);
    assert_eq!(cli::execute(&args(&["show", "btop"]), &c).unwrap(), 0);
    assert_eq!(cli::execute(&args(&["remove", "btop"]), &c).unwrap(), 0);
}

#[test]
fn paths_outside_the_managed_area_are_not_written_or_deleted() {
    let t = Temp::new();
    let c = context(&t);
    // An id with a path separator is rejected before any file is touched.
    assert!(cli::execute(&args(&["add", "btop", "--id", "../escape"]), &c).is_err());
    assert!(!t.path("config/escape.json").exists());
    // A symbolic link in the managed icon directory is not followed.
    let icons = c.registry.icons.clone();
    fs::create_dir_all(&icons).unwrap();
    let outside = t.path("outside.png");
    fs::write(&outside, "keep").unwrap();
    std::os::unix::fs::symlink(&outside, icons.join("link.png")).unwrap();
    assert!(
        !crate::registry::remove_managed_icon(&icons, &icons.join("link.png")).unwrap(),
        "a symbolic link must not be deleted"
    );
    assert!(outside.is_file());
    // A file outside the managed directory is not deleted either.
    assert!(
        !crate::registry::remove_managed_icon(&icons, &outside).unwrap(),
        "a file outside the managed directory must not be deleted"
    );
    assert!(outside.is_file());
}

#[test]
fn the_registration_is_isolated_from_the_real_xdg_directories() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    assert_eq!(cli::execute(&args(&["add", "btop"]), &c).unwrap(), 0);
    // Everything stays inside the temporary directory.
    assert!(c.registry.manifest_path("btop").unwrap().starts_with(&t.0));
    assert!(c.registry.desktop_path("btop").unwrap().starts_with(&t.0));
    assert!(c.registry.icon_path("btop").unwrap().starts_with(&t.0));
    assert!(c.registry.meta_path("btop").unwrap().starts_with(&t.0));
}

#[test]
fn run_files_append_absolute_paths_without_changing_the_registration() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "nano");
    cli::execute(
        &args(&[
            "add",
            "nano",
            "--cwd",
            c.home.to_str().unwrap(),
            "--",
            "--linenumbers",
        ]),
        &c,
    )
    .unwrap();
    let paths = [
        c.registry.manifest_path("nano").unwrap(),
        c.registry.meta_path("nano").unwrap(),
        c.registry.desktop_path("nano").unwrap(),
    ];
    let before: Vec<_> = paths.iter().map(|path| fs::read(path).unwrap()).collect();
    let absolute = t.path("absolute.txt");
    let files = [
        "new file.txt",
        "日本語.txt",
        "-filename",
        "$(touch NEVER)",
        "link/../memo.txt",
        "line\nbreak.txt",
        absolute.to_str().unwrap(),
    ];
    let mut command = args(&["nano", "--"]);
    command.extend(args(&files));
    let mut apps = cli::prepare_run_apps(&command, &c).unwrap();
    assert_eq!(apps.len(), 1);
    let request = LaunchRequest::new(apps.remove(0), "request".into());
    let mut expected = args(&["--linenumbers"]);
    expected.extend(
        files
            .iter()
            .map(|file| c.cwd.join(file).to_str().unwrap().to_owned()),
    );
    assert_eq!(request.arguments, expected);
    assert_eq!(request.working_directory, c.home.to_str().unwrap());
    assert!(!t.path("new file.txt").exists());
    assert!(!t.path("NEVER").exists());
    for (path, bytes) in paths.iter().zip(before) {
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
}

#[test]
fn run_without_files_keeps_multiple_ids_and_registered_arguments() {
    let t = Temp::new();
    let c = context(&t);
    for id in ["nano", "vim"] {
        executable(&t, id);
        cli::execute(&args(&["add", id, "--", "registered.txt"]), &c).unwrap();
    }
    let apps = cli::prepare_run_apps(&args(&["nano", "vim"]), &c).unwrap();
    assert_eq!(
        apps.iter().map(|app| app.id.as_str()).collect::<Vec<_>>(),
        ["nano", "vim"]
    );
    for app in apps {
        assert_eq!(app.arguments, args(&["registered.txt"]));
    }
    let apps = cli::prepare_run_apps(&args(&["nano", "--"]), &c).unwrap();
    assert_eq!(apps[0].arguments, args(&["registered.txt"]));
}

#[test]
fn run_rejects_ambiguous_or_invalid_file_arguments_before_launch() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "nano");
    cli::execute(&args(&["add", "nano"]), &c).unwrap();
    assert_eq!(cli::execute(&args(&["run"]), &c).unwrap(), 2);
    for command in [
        args(&["--", "memo.txt"]),
        args(&["nano", "nano", "--", "memo.txt"]),
        args(&["nano", "--", ""]),
        args(&["nano", "--", "bad\0path"]),
        args(&["nano", "memo.txt"]),
        args(&["missing", "--", "memo.txt"]),
    ] {
        assert!(cli::prepare_run_apps(&command, &c).is_err(), "{command:?}");
    }
}

#[test]
fn run_file_paths_preserve_symlink_parent_semantics() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "nano");
    cli::execute(&args(&["add", "nano"]), &c).unwrap();
    fs::create_dir_all(t.path("target/sub")).unwrap();
    fs::write(t.path("target/memo.txt"), "correct file").unwrap();
    std::os::unix::fs::symlink(t.path("target/sub"), t.path("link")).unwrap();
    let apps = cli::prepare_run_apps(&args(&["nano", "--", "link/../memo.txt"]), &c).unwrap();
    assert_eq!(
        fs::read_to_string(&apps[0].arguments[0]).unwrap(),
        "correct file"
    );
}

#[test]
fn the_desktop_entry_calls_the_public_launcher() {
    let t = Temp::new();
    let c = context(&t);
    executable(&t, "btop");
    assert_eq!(cli::execute(&args(&["add", "btop"]), &c).unwrap(), 0);
    let desktop = fs::read_to_string(c.registry.desktop_path("btop").unwrap()).unwrap();
    assert!(desktop.contains("Exec="));
    assert!(desktop.contains("run"));
    assert!(desktop.contains("btop"));
    assert!(desktop.contains("X-TuiPWA-Id=btop"));
    assert!(desktop.contains("Icon="));
    assert!(desktop.contains(&format!("StartupWMClass={}", desktop_id("btop"))));
}

#[test]
fn host_lookup_supports_user_install_and_existing_priorities() {
    use crate::platform::linux::runtime::resolve_host_path;
    let t = Temp::new();
    let executable = t.path("cargo/bin/tuipwa");
    let data = t.path("data");
    let installed = data.join("tuipwa/native/bin/ghostty-host");
    fs::create_dir_all(installed.parent().unwrap()).unwrap();
    fs::write(&installed, b"host").unwrap();
    let dev = t.path("dist/bin/ghostty-host");
    let resolve = |explicit: Option<&Path>, dev_host: Option<&Path>| {
        resolve_host_path(explicit, &executable, dev_host, Some(&data))
    };
    assert_eq!(resolve(None, None).unwrap(), installed);
    assert!(resolve_host_path(None, &executable, None, Some(Path::new("relative"))).is_err());
    assert!(resolve(Some(Path::new("missing")), None).is_err());
    assert!(resolve(Some(&t.path("missing")), None).is_err());
    fs::create_dir_all(dev.parent().unwrap()).unwrap();
    fs::write(&dev, b"host").unwrap();
    assert_eq!(resolve(None, Some(&dev)).unwrap(), dev);
    let sibling = executable.parent().unwrap().join("ghostty-host");
    fs::create_dir_all(sibling.parent().unwrap()).unwrap();
    fs::write(&sibling, b"host").unwrap();
    assert_eq!(resolve(None, Some(&dev)).unwrap(), sibling);
    assert_eq!(resolve(Some(&installed), Some(&dev)).unwrap(), installed);
    fs::remove_file(&sibling).unwrap();
    fs::remove_file(&dev).unwrap();
    fs::remove_file(&installed).unwrap();
    assert!(resolve(None, None).is_err());
}

#[test]
fn the_system_icon_search_matches_the_command_exactly() {
    let t = Temp::new();
    let data = t.path("data");
    let applications = data.join("applications");
    fs::create_dir_all(&applications).unwrap();
    let icon_dir = data.join("icons/hicolor/48x48/apps");
    fs::create_dir_all(&icon_dir).unwrap();
    let image = icon_dir.join("btop.png");
    crate::icons::png::write_png(&image, &[0u8; 4], 1, 1).unwrap();
    fs::write(
        applications.join("btop.desktop"),
        "[Desktop Entry]\nExec=/usr/bin/btop --flag\nIcon=btop\n",
    )
    .unwrap();
    // The command matches exactly.
    assert_eq!(
        icons::find_system_icon("/usr/bin/btop", std::slice::from_ref(&data)),
        Some(image.clone())
    );
    // A similar name is not matched.
    assert_eq!(icons::find_system_icon("/usr/bin/btopp", &[data]), None);
    assert_eq!(icons::find_system_icon("/usr/bin/htop", &[]), None);
}
