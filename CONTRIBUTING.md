# 開発ガイド

利用者向けの導入・コマンドは [README](README.md)、登録 JSON は [manifest v1](docs/MANIFEST.md)、第三者ライセンスは [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)を参照してください。

## 開発者向けビルド・実行（dev）

通常の導入とシステムパッケージは [README](README.md#インストール)を参照してください。コマンドはリポジトリのルートで実行します。Rust CLI だけの変更なら、毎回 Ghostty をビルドする必要はありません。

### CLI だけをビルド・実行

CLI 単体のビルドには Rust / Cargo、C コンパイラ、pkg-config、GLib / GIO の開発ファイルが必要です。`make test` には Python 3 も必要です。

```sh
cargo build --locked
cargo run --locked -- add btop
cargo run --locked -- list
cargo run --locked -- show btop
cargo run --locked -- update btop --name "Monitor"
cargo run --locked -- remove btop
```

`build.rs` が `src/platform/linux/ipc.c` をコンパイルして Rust とリンクします。出力は `target/debug/tuipwa` です。登録・一覧・更新・削除・検証には GUI ホストは不要です。

### dev でウィンドウを起動

通常ビルドのシステム依存を揃え、初回に `make` でホストを作ります。

```sh
make
cargo run --locked -- add btop
cargo run --locked -- run btop
```

ホストの検索順は `TUIPWA_GHOSTTY` の明示指定 → CLI と同じディレクトリ → debug ビルド時のリポジトリ内 `dist/bin/ghostty-host` → ユーザー領域のインストール先です。明示指定が不正な場合は別のホストへフォールバックしません。debug ビルドでは、インストール済みホストより現在のチェックアウトで作ったホストを優先します。別の対応済みホストを使う場合だけ `TUIPWA_GHOSTTY=/absolute/path/to/ghostty-host` で指定します。Ghostty のバージョン・パッチを変更した場合は `make` を再実行します。

`cargo run` で登録した `.desktop` の `Exec` は `target/debug/tuipwa` になります。常用する場合は `make install` 後の `tuipwa`、または手動配置した `dist/bin/tuipwa` で登録し直してください。

### テスト・静的検査

```sh
make test              # Rust 単体テスト + Python 配布物検査の単体テスト
make lint              # cargo clippy --locked --all-targets -- -D warnings
cargo fmt --all --check
make check-dist        # 生成済み dist/ の配布境界・通知を検査（readelf が必要）
```

`check-dist` は GTK 系ライブラリの混入・主要な動的依存・通知ファイルの存在を対象とし、ライセンス条件全体の監査ではありません。

GUI テストには [Xvfb](https://www.x.org/releases/current/doc/man/man1/Xvfb.1.xhtml)、[Weston](https://wayland.pages.freedesktop.org/weston/)、xauth、D-Bus（`dbus-run-session`）、GLib のツール（`gdbus`）、libXtst、nano、vim、btop が必要です。

```sh
TUIPWA_NATIVE="$PWD/dist/bin/tuipwa" \
  dbus-run-session -- xvfb-run -a env GDK_BACKEND=x11 LIBGL_ALWAYS_SOFTWARE=1 \
  python3 tests/linux/gui.py

TUIPWA_NATIVE="$PWD/dist/bin/tuipwa" \
  dbus-run-session -- python3 tests/linux/wayland.py
```

GUI テストは一時 XDG 領域を使います。X11 版は並行起動・同一ホスト PID・独立 PTY・argv/cwd・入力・正常/異常終了・ホスト再起動・回収・watchdog を確認します。Wayland 版は Weston 上の app_id・タイトル・ウィンドウ終了を確認します。KDE のタスクバー表示や IME の実操作を保証するテストではありません。ソフトウェア GL やセッション環境に依存するため、GUI テストは CI では実行しません。

## ビルドと配布

`make` はユーザー領域へインストールせず、CLI・専用ホスト・リソース・通知を `dist/` に生成します。常用環境へ反映するときだけ `make install` を実行してください。

1. Zig 0.17.0 と Ghostty 1.4.0 の公式アーカイブを取得し、SHA-256 を照合します。
2. `native/linux/patches/0001-window-identity.patch` を適用します。
3. Zig でホスト、`cargo build --release --locked` で CLI をビルドします。
4. ライセンス通知を収集し、GTK 系ライブラリの非同梱・動的依存・通知の存在を検査します。

Ghostty の基準コミットは `83fb95624b0439ee660c2679b43b38c8b63769cd`（v1.4.0）です。生成済み GTK リソースを含む公式アーカイブを使います。ビルドは省メモリ向けの `ReleaseSmall`・`-j1` 固定で、性能評価用ではありません。HarfBuzz と glslang は、現 Makefile では Ghostty が固定する依存を使います。

キャッシュは `${XDG_CACHE_HOME:-~/.cache}/tuipwa-native` です。`TUIPWA_BUILD_CACHE=/absolute/path make` または `make CACHE=/absolute/path` で変更できます。パッチ内容のハッシュごとに Ghostty のソースツリーを分離します。

### 配置・ライセンス

手動配置では `dist/` 全体を固定した場所に置き、`bin/tuipwa` を直接呼び出します。隣の `ghostty-host` と `share/` が必要です。実行先の依存不足は `ldd dist/bin/tuipwa` と `ldd dist/bin/ghostty-host` で確認し、OS のパッケージから導入してください。GTK 系ライブラリ本体をコピーしてはいけません。

`make install` は CLI を Cargo 標準先へ、ホスト・リソース・通知を `${XDG_DATA_HOME:-~/.local/share}/tuipwa/native/` へ導入します。`CARGO_INSTALL_ROOT` 等の Cargo 標準設定を使えます。相対パスの XDG 変数は採用しません。CLI の場所を変えた場合は `.desktop` の絶対パスが変わるため再登録が必要です。

本体の MIT、Ghostty の MIT、上流依存・フォント・Rust 依存の通知、システム依存向けの LGPL 本文を維持します。通知収集・`check-dist` はライセンス監査を代替しません。収集範囲は `Makefile` と第三者通知を参照してください。

### CI

[GitHub Actions](.github/workflows/ci.yml)では、Ubuntu 上の fmt・Clippy・単体テストと、Debian 13 上の配布物ビルド・検査・隔離先へのインストール・CLI スモークテストを実行します。GUI テストは手動実行です。

## 依存ライブラリ・ツール

### ネイティブホスト・システム依存

**パッチ済み Ghostty は同梱し、GTK 等のシステム共有ライブラリは同梱しません。** システム依存はビルド先・実行先の OS パッケージから導入します。通常の未改修 Ghostty をインストール済みでも、専用ホストの代わりにはなりません。

| 依存 | 用途・扱い |
|---|---|
| [Ghostty](https://github.com/ghostty-org/ghostty) 1.4.0 | 端末描画・PTY・IME。専用パッチを適用して同梱 |
| [GTK 4](https://www.gtk.org/) / [libadwaita](https://gnome.pages.gitlab.gnome.org/libadwaita/) | ウィンドウ・UI。システムへ動的リンク |
| [GLib / GIO / GObject](https://docs.gtk.org/) | D-Bus・オブジェクト基盤。CLI とホストから動的リンク |
| [gtk4-layer-shell](https://github.com/wmww/gtk4-layer-shell) | Ghostty の Wayland 統合用システム依存 |
| [Wayland](https://wayland.freedesktop.org/) / [libxkbcommon](https://xkbcommon.org/) / [X.Org](https://www.x.org/) | ウィンドウシステム・キー入力 |
| [Mesa / EGL / OpenGL](https://www.mesa3d.org/) / [SPIRV-Cross](https://github.com/KhronosGroup/SPIRV-Cross) | GPU 描画・シェーダー関連 |
| [fontconfig](https://www.freedesktop.org/wiki/Software/fontconfig/) / [FreeType](https://freetype.org/) | フォント探索・描画。システムへ動的リンク |
| [HarfBuzz](https://harfbuzz.github.io/) / [glslang](https://github.com/KhronosGroup/glslang) | 文字組版・シェーダー処理。現ビルドでは Ghostty が固定する依存を使用 |
| [libpng](https://www.libpng.org/pub/png/libpng.html) / [zlib](https://zlib.net/) / [Oniguruma](https://github.com/kkos/oniguruma) | Ghostty の画像・圧縮・正規表現処理。システムへ動的リンク |
| [libxml2](https://gitlab.gnome.org/GNOME/libxml2) / [GObject Introspection](https://gi.readthedocs.io/) / [Blueprint](https://gnome.pages.gitlab.gnome.org/blueprint-compiler/) / [gettext](https://www.gnu.org/software/gettext/) | 主にホストのビルド・リソース生成 |

### Rust の直接依存

バージョンの解決結果と推移的依存は [Cargo.lock](Cargo.lock) で固定します。

| クレート | 用途 |
|---|---|
| [serde](https://crates.io/crates/serde) / [serde_json](https://crates.io/crates/serde_json) | マニフェスト・IPC の JSON 入出力 |
| [libc](https://crates.io/crates/libc) | Linux のプロセス・ソケット・シグナル制御 |
| [fontdue](https://crates.io/crates/fontdue) / [png](https://crates.io/crates/png) | アイコンの文字描画・PNG 入出力 |
| [cc](https://crates.io/crates/cc) / [pkg-config](https://crates.io/crates/pkg-config) | ビルド時の C コンパイル・GIO 検出 |

ビルドには [Rust / Cargo](https://www.rust-lang.org/tools/install) の stable ツールチェーン（edition 2024 対応）、C/C++ ツールチェーン、Make、Python 3、binutils 等が必要です。[Zig](https://ziglang.org/) 0.17.0 は `make` が取得するため手動導入は不要です。各依存のライセンスは [第三者通知](THIRD_PARTY_NOTICES.md)と配布物の `share/licenses/` を参照してください。

## 責務・プロトコル

```
Rust CLI (manifest / registry)
  -> Linux runtime flock (起動・受付を直列化、終了で自動解放)
  -> org.tuipwa.NativeHost (1つのGhostty GTK Application)
  -> org.gtk.Actions.Activate("new-window-command", argv)
  -> 独立Window -> Surface -> Unix PTY -> ネイティブsession補助 -> TUI
```

- `model.rs`: プラットフォーム非依存のManifest/LaunchRequest v1。
- `registry.rs`: Linux XDGとdesktop。GTK/PTYへの依存はありません。
- `icons/render.rs` / `icons/font.rs` / `icons/png.rs`: アイコン生成。外部サービスやスクリプトに依存しません。
- `icons/mod.rs` / `meta.rs`: アイコン決定とメタデータ管理（カテゴリ・対応MIMEタイプを含みます）。
- `category.rs`: freedesktop.org のカテゴリ検証・正規化。
- `cli.rs`: 公開コマンドの引数解析と実装。
- `platform/linux/runtime.rs` / `platform/linux/ipc.c`: D-Bus、起動競合、同一ユーザーUnixソケット、セッション管理。
- 上流はVT/PTY/フォント/IME/GPU/ウィンドウ終了判定を担当します。各ウィンドウに小さなネイティブ補助プロセスが1個ありますが、GUIホストは全アプリで1個です。.NETランタイムは一切利用しません。

### 受付

1. `${XDG_RUNTIME_DIR}/tuipwa-native/launch.lock`をflockします。ディレクトリの所有UID・実体・0700を確認します。
2. D-Busの名前所有PIDがなければホストを起動し、登録を待ちます。GApplicationの単一インスタンス機構も併用します。
3. 既存GTK actionへタイトル、cwd、安定Wayland ID、`-e`に補助プロセスのargvを送ります。呼出先はwell-known名から解決したunique ownerです。
4. 補助が0700ディレクトリ内のランダムな0600ソケットへ接続します。双方SO_PEERCREDで同一UIDを確認します。ランチャーは`LaunchRequest` JSON＋LFを送り、補助は`Ack` JSON＋LFを返します。
5. ACKまでロックを保持します。リクエストID、ホストPID、接続元セッションPIDを照合します。ACKには子PID、TTY、起動エラーも含みます。

LaunchRequestの`protocolVersion=1`、`requestId`、`appId`、`title`、`icon`、`command`、`arguments[]`、`workingDirectory`はモデルに定義しています。iconはdesktopエントリ経由でコンポジターが解決します。シェルでの再解釈は行いません。

ホストstderrは専用runtimeディレクトリの`host.log`へ出力し、短命ランチャーのパイプを継承しません。30秒の起動待ち上限を設け、GUI/ドライバー障害でCLIが永久停止するのを防ぎます。受付後のタイムアウトは成否不明と報告し、同じ生存ホストへ再送しません。ホスト消失時のみ新ホストへ再送できます（旧ウィンドウは既にありません）。exactly-onceの永続配信保証は提供しません。起動前にランチャーが死んでも、未使用ホストは30秒の上流パッチ内watchdogで終了します。残るlockファイルはデーモンではなく、次回そのまま再利用できます。

セッション補助は子を独立foreground process groupで起動し、子プロセスが終了したら終了状態をPTYへ表示して自身も終了します。これによりPTYが閉じ、Ghosttyが対応サーフェスを閉じます（最後のウィンドウなら共有ホストも終了します）。正常終了・非ゼロ終了・シグナル終了のいずれも同じ扱いで、`tuipwa run`ランチャーの終了は契機にしません。起動自体に失敗した場合だけ、原因を読めるようウィンドウを保持します。ウィンドウclose/HUP/ホスト死亡時に子グループへSIGHUP/SIGCONTを送り、1秒の終了猶予後に残存グループをSIGKILLして回収します。Linux PDEATHSIGも使います。自ら新セッション化して逃げたプロセスまでのcgroup封じ込めは行いません。

Windows/WSL用コードはありません。将来はmodelを維持し、Linux transport/sessionをNamed Pipe/ConPTY等へ置き換えます。v1へWindowsフィールドは加えません。

## ウィンドウ識別（共有ホストとは別判定）

|名前|役割|
|---|---|
|`org.tuipwa.NativeHost`|固定GTK Application IDとセッションD-Bus well-known名。全アプリ共通|
|`tuipwa-<manifest-id>`|ウィンドウ別Wayland app_id。manifest IDから衝突なく安定生成|
|`tuipwa-<manifest-id>.desktop`|desktop file ID。Icon/Exec/Nameを提供|

パッチは新規Windowの**present直後**に`gdk_wayland_toplevel_set_application_id`を呼びます。GTK 4.18.6ではGtkWidget.realizeだけではxdg_toplevelがなく、setterが何もせず戻ることを実験で確認しました。このため最初のwire-level IDは一瞬共通ホストIDになり、その直後にアプリ別IDへ更新されます。初期IDだけを採用するコンポジターでの関連付け、およびhide/unmap後の再mapでの識別維持は保証しません（通常の最小化は別です）。GTK Application IDは変更しません。タイトルの前後空白/cwdも上流IPCのtrimで失われないよう修正します。

Wayland APIの呼出し成功だけではKDEのアイコン・グループ化成功としません。実行ログとKDE実機確認の状態は別途記録します。X11は共通ホストのWM_CLASSへフォールバックし、各ウィンドウの固定タイトルとdesktopメニューアイコンを提供します。個別IDホストへの切り替えはしません。任意ファイルアイコンをGTKヘッダーへ直接埋め込む機能はありません（WaylandタスクバーはdesktopのIconを使用します）。

## KDE Plasma実機確認手順

Wayland APIの呼出し成功だけではKDEのアイコン・グループ化成功を意味しません。以下は実機で確認する操作と確認観点です。**本リポジトリの自動テストはKDEタスクバーの挙動を検証していません**（自動テストはXvfb上のX11とWestonを使います）。

前提: `dist/`を固定した場所に配置します。`dist/bin/tuipwa`の隣に`ghostty-host`が必要で、PATHの設定は不要です。セッションはWayland（`WAYLAND_DISPLAY`あり）で起動します。

```sh
TUIPWA=/opt/tuipwa/dist/bin/tuipwa        # dist を置いた場所
"$TUIPWA" add btop
"$TUIPWA" add vim --id notes --name "Notes"
"$TUIPWA" show notes          # Icon= が生成PNGを指していることを確認
```

1. **タスクバーアイコン表示**: `tuipwa run notes`を起動し、Plasmaのタスクバーに生成したアイコン（モノグラム入りPNG）が表示されるか確認してください。`~/.local/share/tuipwa/icons/notes.png`を開いて描画内容を確認すると確実です。
2. **ウィンドウ別グループ化**: 2つ以上のTUIを同時に起動し、各ウィンドウが別々のタスクバー項目になるか確認してください。`tuipwa-<id>`の`app_id`が Wayland wire level で送られていることはWestonテストで確認済みですが、PlasmaがWaylandアプリIDと`StartupWMClass`のどちらで関連付けるかは実機確認が必要です。
3. **ピン留め**: 生成アイコンをタスクバーに右クリックでピン留めし、アプリを終了してもピンが残るか、再起動時に同じアイコンが出るか確認してください。
4. **再起動時の再現**: `tuipwa remove notes`→`tuipwa add vim --id notes`と登録し直して、アイコンが同じモノグラム・配色で生成されるか確認してください（IDハッシュは安定なので同じ色になります）。
5. **更新の反映**: `tuipwa update notes --color "#0F766E"`のあと、既存ウィンドウではなく**次回起動**から新アイコンが出るか確認してください。タスクバーの既存アイコンはキャッシュされるため、古いアイコンのままでも仕様どおりです。
6. **カテゴリ表示**: `tuipwa add btop --category System --category Monitor`のあと、Plasmaのメニューで想定した階層に表示されるか確認してください。`tuipwa show btop`の`カテゴリ:`行と`~/.local/share/applications/tuipwa-btop.desktop`の`Categories=`が一致していることを先に確認します。メニュー反映はデスクトップ環境のキャッシュに依存するため、本リポジトリの自動テスト対象外です。

7. **ファイルを開く**: `tuipwa update nano --mime-type text/plain`のあと、Dolphinで空白・日本語を含む名前のテキストファイルを複数選び、「このアプリケーションで開く」から登録したnanoへ渡してください。`.desktop`の`Exec=`の末尾が`-- %F`、`MimeType=`が`text/plain;`になっていることと、`update-desktop-database`の警告がないことを先に確認します。通常のメニュー起動も維持されること、`--clear-mime-types`で対応を解除できること、既定アプリの設定が変更されないことを確認してください。KDE実機での候補表示は未検証です。

## 変更時の注意

- manifest v1 にプラットフォーム固有のフィールドを追加せず、アイコン・カテゴリ・対応MIMEタイプの追加情報は `meta` に分離します。
- 登録の複数ファイル更新は完全には原子的ではありません。各ファイルは一時ファイル＋renameで更新し、途中失敗にはロールバックしますが、強制終了時まで保証しません。
- `.desktop` の所有者確認、引数・フィールドのエスケープ、管理領域外への書き込み・削除の拒否を維持してください。
- テストや調査では一時 XDG 領域を使い、実ユーザーの登録・既存 Ghostty を変更しないでください。
- 日本語の説明文は敬体、第三者通知は簡潔な英語で記述します。ライセンス原文は改変しません。
