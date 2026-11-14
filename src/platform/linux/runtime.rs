use crate::{
    Result,
    model::{LaunchRequest, desktop_id},
};
use serde::{Deserialize, Serialize};
use std::{
    ffi::CString,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
            net::{UnixListener, UnixStream},
            process::CommandExt,
        },
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicI32, Ordering},
    time::{Duration, Instant},
};

const HOST: &str = "org.tuipwa.NativeHost";
// A bounded startup wait prevents a broken GUI/driver from hanging a launcher forever.
const STARTUP: Duration = Duration::from_secs(30);
unsafe extern "C" {
    fn tuipwa_owner() -> libc::c_int;
    fn tuipwa_activate(
        args: *const *const libc::c_char,
        message: *mut libc::c_char,
        size: usize,
    ) -> libc::c_int;
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Ack {
    pub request_id: String,
    pub host_pid: u32,
    pub session_pid: u32,
    pub child_pid: Option<u32>,
    pub tty: String,
    pub error: Option<String>,
}
fn owner() -> Result<i32> {
    let pid = unsafe { tuipwa_owner() };
    if pid < 0 {
        Err("セッションD-Busへ接続できません".into())
    } else {
        Ok(pid)
    }
}
fn private_dir() -> Result<PathBuf> {
    let base =
        PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").ok_or("XDG_RUNTIME_DIR が必要です")?);
    let metadata = fs::symlink_metadata(&base)?;
    if !base.is_absolute()
        || !metadata.is_dir()
        || metadata.uid() != unsafe { libc::getuid() }
        || metadata.mode() & 0o077 != 0
    {
        return Err("XDG_RUNTIME_DIR は同一ユーザー所有・0700の実ディレクトリが必要です".into());
    }
    let path = base.join("tuipwa-native");
    match fs::create_dir(&path) {
        Ok(()) => fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.into()),
    }
    let m = fs::symlink_metadata(&path)?;
    if !m.is_dir() || m.uid() != unsafe { libc::getuid() } || m.mode() & 0o077 != 0 {
        return Err("安全でないruntimeディレクトリです".into());
    }
    Ok(path)
}
pub fn request_id() -> Result<String> {
    use std::io::Read;
    let mut bytes = [0u8; 16];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
struct SocketFile(PathBuf);
impl Drop for SocketFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
struct Lock(File);
impl Drop for Lock {
    fn drop(&mut self) {
        unsafe {
            libc::flock(self.0.as_raw_fd(), libc::LOCK_UN);
        }
    }
}
fn lock(dir: &Path) -> Result<Lock> {
    let f = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(dir.join("launch.lock"))?;
    if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(Lock(f))
}
fn host_path() -> Result<PathBuf> {
    let explicit = std::env::var_os("TUIPWA_GHOSTTY").map(PathBuf::from);
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
    // Prefer the current checkout's host for `cargo run`, not an older installed host.
    let dev = cfg!(debug_assertions)
        .then(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("dist/bin/ghostty-host"));
    resolve_host_path(
        explicit.as_deref(),
        &std::env::current_exe()?,
        dev.as_deref(),
        data_home.as_deref(),
    )
}

pub(crate) fn resolve_host_path(
    explicit: Option<&Path>,
    executable: &Path,
    dev: Option<&Path>,
    data_home: Option<&Path>,
) -> Result<PathBuf> {
    let candidates = if let Some(path) = explicit {
        // An invalid explicit override must not silently select another host.
        vec![path.to_owned()]
    } else {
        let sibling = executable
            .parent()
            .ok_or("実行ファイルの親ディレクトリがありません")?
            .join("ghostty-host");
        let mut paths = vec![sibling];
        paths.extend(dev.map(Path::to_owned));
        if let Some(data) = data_home.filter(|path| path.is_absolute()) {
            paths.push(data.join("tuipwa/native/bin/ghostty-host"));
        }
        paths
    };
    if let Some(path) = candidates
        .into_iter()
        .find(|path| path.is_absolute() && path.is_file())
    {
        return Ok(path);
    }
    Err("Ghosttyホストが見つかりません。`make install` で導入するか、TUIPWA_GHOSTTY にパッチ済みホストの絶対パスを指定してください".into())
}
/// User appearance settings are loaded normally; standalone-app controls win.
pub(crate) fn host_args() -> Vec<String> {
    vec![
        "--config-default-files=true".into(),
        format!("--class={HOST}"),
        "--gtk-single-instance=true".into(),
        "--initial-window=false".into(),
        "--quit-after-last-window-closed=true".into(),
        "--shell-integration=none".into(),
        "--confirm-close-surface=false".into(),
        "--window-show-tab-bar=never".into(),
        "--window-save-state=never".into(),
        "--keybind=clear".into(),
    ]
}

fn start_host(dir: &Path) -> Result<std::process::Child> {
    let log = OpenOptions::new()
        .append(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(dir.join("host.log"))?;
    let mut cmd = Command::new(host_path()?);
    cmd.args(host_args())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(log));
    // Detach the GUI from the short-lived launcher/desktop activation process group.
    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    Ok(cmd.spawn()?)
}
fn send(args: &[String]) -> Result<()> {
    let strings: Vec<CString> = args
        .iter()
        .map(|s| CString::new(s.as_str()))
        .collect::<std::result::Result<_, _>>()?;
    let mut ptrs: Vec<_> = strings.iter().map(|s| s.as_ptr()).collect();
    ptrs.push(std::ptr::null());
    let mut error = [0i8; 1024];
    if unsafe { tuipwa_activate(ptrs.as_ptr(), error.as_mut_ptr(), error.len()) } == 0 {
        return Err(unsafe { std::ffi::CStr::from_ptr(error.as_ptr()) }
            .to_string_lossy()
            .to_string()
            .into());
    }
    Ok(())
}
fn peer(stream: &UnixStream) -> Result<libc::ucred> {
    let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of_val(&cred) as libc::socklen_t;
    if unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut cred as *mut _ as *mut _,
            &mut len,
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    if cred.uid != unsafe { libc::getuid() } {
        return Err("異なるユーザーからの接続を拒否しました".into());
    }
    Ok(cred)
}
pub fn launch(req: &LaunchRequest) -> Result<Ack> {
    let dir = private_dir()?;
    let _lock = lock(&dir)?;
    let socket = SocketFile(dir.join(format!("{}.sock", req.request_id)));
    let listener = UnixListener::bind(&socket.0)?;
    fs::set_permissions(&socket.0, fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    let deadline = Instant::now() + STARTUP;
    let mut child: Option<std::process::Child> = None;
    let mut sent_pid = 0;
    let mut accepted = false;
    loop {
        let pid = owner()?;
        // Only a vanished primary may be retried; never resend to a live owner.
        if accepted && pid == 0 {
            sent_pid = 0;
            accepted = false;
            if let Some(mut old) = child.take() {
                let _ = old.try_wait();
            }
        }
        if pid == 0 {
            if let Some(c) = child.as_mut() {
                let c: &mut std::process::Child = c;
                if let Some(status) = c.try_wait()? {
                    return Err(format!("Ghosttyホスト起動失敗: {status}").into());
                }
            } else {
                child = Some(start_host(&dir)?);
            }
        } else if pid != sent_pid {
            let mut args = vec![
                format!("--title={}", req.title),
                format!("--working-directory={}", req.working_directory),
                "--shell-integration=none".into(),
                format!("--tuipwa-app-id={}", desktop_id(&req.app_id)),
            ];
            args.extend([
                "-e".into(),
                std::env::current_exe()?
                    .to_str()
                    .ok_or("非UTF-8パス")?
                    .into(),
                "__session".into(),
                socket.0.to_str().ok_or("非UTF-8パス")?.into(),
                req.request_id.clone(),
            ]);
            match send(&args) {
                Ok(()) => {
                    sent_pid = pid;
                    accepted = true;
                }
                Err(e) => {
                    if owner()? == pid {
                        return Err(e);
                    }
                }
            }
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                let cred = peer(&stream)?;
                stream.set_read_timeout(Some(STARTUP))?;
                stream.set_write_timeout(Some(STARTUP))?;
                serde_json::to_writer(&mut stream, req)?;
                stream.write_all(b"\n")?;
                let mut line = String::new();
                BufReader::new(stream).read_line(&mut line)?;
                let ack: Ack = serde_json::from_str(&line)?;
                if ack.request_id != req.request_id
                    || ack.host_pid != sent_pid as u32
                    || ack.session_pid != cred.pid as u32
                {
                    return Err("不正な起動ACKです".into());
                }
                return Ok(ack);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => return Err(e.into()),
        }
        if Instant::now() >= deadline {
            if !accepted && let Some(c) = child.as_mut() {
                let _ = c.kill();
                let _ = c.wait();
            }
            return Err(
                "起動ACKが30秒以内に届きませんでした（受付状態は不明、盲目的に再送しません）"
                    .into(),
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

static STOP: AtomicI32 = AtomicI32::new(0);
extern "C" fn stop(signal: i32) {
    STOP.store(signal, Ordering::Relaxed);
}
fn setup_signals() {
    unsafe {
        libc::signal(libc::SIGHUP, stop as *const () as usize);
        libc::signal(libc::SIGTERM, stop as *const () as usize);
        libc::signal(libc::SIGTTOU, libc::SIG_IGN);
        libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
    }
}
fn terminate_group(child: &mut std::process::Child) -> std::io::Result<()> {
    // Terminal-close semantics first; bound shutdown so an ignored HUP cannot
    // keep the closing GUI alive. One second is a PoC shutdown grace budget.
    let group = -(child.id() as i32);
    unsafe {
        libc::kill(group, libc::SIGHUP);
        libc::kill(group, libc::SIGCONT);
    }
    let deadline = Instant::now() + Duration::from_secs(1);
    while child.try_wait()?.is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    unsafe {
        libc::kill(group, libc::SIGKILL);
    }
    child.wait()?;
    Ok(())
}
pub fn session(path: &str, id: &str) -> Result<()> {
    let parent = unsafe { libc::getppid() };
    setup_signals();
    if unsafe { libc::getppid() } != parent {
        return Err("ホストが終了しました".into());
    }
    let mut stream = UnixStream::connect(path)?;
    peer(&stream)?;
    stream.set_read_timeout(Some(STARTUP))?;
    stream.set_write_timeout(Some(STARTUP))?;
    let mut line = String::new();
    BufReader::new(stream.try_clone()?).read_line(&mut line)?;
    let req: LaunchRequest = serde_json::from_str(&line)?;
    if req.protocol_version != 1 || req.request_id != id {
        return Err("未対応プロトコルまたはリクエストID不一致".into());
    }
    let mut command = Command::new(&req.command);
    command
        .args(&req.arguments)
        .current_dir(&req.working_directory);
    unsafe {
        command.pre_exec(|| {
            if libc::setpgid(0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            // Foreground BEFORE exec: otherwise a fast TUI can read stdin,
            // receive SIGTTIN, and remain stopped even after the parent sets it.
            if libc::tcsetpgrp(0, libc::getpgrp()) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            libc::signal(libc::SIGHUP, libc::SIG_DFL);
            libc::signal(libc::SIGTERM, libc::SIG_DFL);
            libc::signal(libc::SIGTTOU, libc::SIG_DFL);
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn();
    let mut ttybuf = [0i8; 256];
    let tty = unsafe {
        if libc::ttyname_r(0, ttybuf.as_mut_ptr(), ttybuf.len()) == 0 {
            std::ffi::CStr::from_ptr(ttybuf.as_ptr())
                .to_string_lossy()
                .into_owned()
        } else {
            String::new()
        }
    };
    let ack = Ack {
        request_id: id.into(),
        host_pid: parent as u32,
        session_pid: std::process::id(),
        child_pid: child.as_ref().ok().map(|c| c.id()),
        tty,
        error: child.as_ref().err().map(ToString::to_string),
    };
    serde_json::to_writer(&mut stream, &ack)?;
    stream.write_all(b"\n")?;
    drop(stream);
    match child.as_mut() {
        Ok(c) => loop {
            if STOP.load(Ordering::Relaxed) != 0 {
                terminate_group(c)?;
                return Ok(());
            }
            if let Some(status) = c.try_wait()? {
                unsafe {
                    libc::tcsetpgrp(0, libc::getpgrp());
                    libc::kill(-(c.id() as i32), libc::SIGKILL);
                }
                // The PTY may already be gone when the window is closing, so a
                // failed write must not turn the helper into a panic.
                let _ = writeln!(std::io::stdout(), "\r\n[TuiPWA] {} — {status}", req.title);
                // The registered command is done. Returning ends this session
                // helper, which closes the PTY and lets Ghostty close the
                // surface (and the shared host when it was the last window).
                // The launcher that sent the request exited long ago and is
                // not involved in this decision.
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(20));
        },
        Err(e) => {
            // A command that could not be started keeps its window so the
            // reason stays readable.
            eprintln!("\r\n[TuiPWA] 起動失敗: {e}\r\nウィンドウを閉じて終了してください。");
            while STOP.load(Ordering::Relaxed) == 0 {
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
    Ok(())
}
