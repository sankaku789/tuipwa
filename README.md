<img src="assets/icon.svg" width="64" height="64" alt="">

# TuiPWA

[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

日本語 | [English](README.en.md)

TUI アプリを、独立したデスクトップアプリのように使える Linux x64 向けランチャーです。[Ghostty](https://ghostty.org/) を共有ホストとして使い、アプリごとにタブなしのウィンドウを開きます。

- 登録したアプリをデスクトップメニューから起動できます。
- アプリごとに独立したウィンドウ・PTY を持ち、終了するとウィンドウも閉じます。

## インストール

必要な依存パッケージは以下です（ライブラリは開発ファイルを含みます）。

- [Rust / Cargo](https://www.rust-lang.org/tools/install)（stable）
- C/C++ コンパイラ、Make、CMake、Ninja、pkg-config、Git、curl、CA 証明書、patch、Python 3、binutils、xz
- GTK 4、libadwaita、gtk4-layer-shell、GLib / GIO / GObject、GObject Introspection、Blueprint、gettext
- fontconfig、FreeType、HarfBuzz、libpng、zlib、Oniguruma、libxml2、SPIRV-Cross
- Wayland、wayland-protocols、libxkbcommon、X11、libXcursor、libXi、libXrandr、EGL / OpenGL
- Adwaita / hicolor アイコンテーマ、システムフォント

Zig はビルド時に自動取得します。

```sh
git clone https://github.com/sankaku789/tuipwa.git
cd tuipwa
make install
```

CLI は通常 `~/.cargo/bin` へ、専用ホストは通常 `~/.local/share/tuipwa/native/` へ導入されます。Cargo の `bin/` が PATH に入っていれば、そのまま `tuipwa` を使えます。`make install` に sudo は不要です。

パッチ済み Ghostty は自動でビルド・導入します。GTK 等は OS のパッケージを使用し、同梱しません。初回のビルドには時間がかかります。

## 使い方

起動する TUI アプリは、あらかじめインストールしてください。まずは登録して起動します。

```sh
tuipwa add btop
tuipwa run btop
```

登録したアプリはデスクトップメニューからも起動できます。`run`・`show`・`update`・`remove` では登録 ID を指定します（既定は実行ファイル名です）。

### コマンド一覧

| コマンド | 操作 |
|---|---|
| `tuipwa add <command>` | アプリを登録します |
| `tuipwa run <id> [id ...]` | 登録したアプリを起動します |
| `tuipwa list` | 登録一覧を表示します |
| `tuipwa show <id>` | 登録内容を表示します |
| `tuipwa update <id> [オプション]` | 指定した項目だけ更新します |
| `tuipwa remove <id>` | 登録を解除します。TUI 本体は削除しません |
| `tuipwa validate <manifest.json>` | マニフェストを検証します |

### 名前・引数・作業ディレクトリを指定

```sh
tuipwa add vim --id notes --name "Notes" --cwd ~/Documents -- ~/notes.txt
tuipwa run notes
tuipwa show notes
```

`--id` は登録 ID、`--name` はメニュー表示名、`--title` はウィンドウタイトル、`--cwd` は作業ディレクトリです。TUI に渡す引数は `--` の後ろへ指定します。シェルによる再解釈は行いません。

### アイコン・カテゴリを指定

```sh
tuipwa update btop --color "#2563EB" --category System --category Monitor
tuipwa add vim --id editor --icon ./editor.png
```

`--color` は生成アイコンの背景色、`--icon` は既存の PNG/ICO を指定します。両者は同時に指定できません。`add` でどちらも省略するとアイコンを自動決定します。カテゴリの既定は `Utility` です。

同じ ID は重複登録できません。登録済みのアプリには `update` を使います。

### 更新・登録解除

```sh
tuipwa update notes --name "My Notes"
tuipwa update notes -- ~/Documents/todo.txt       # 引数を置き換えます
tuipwa update notes --clear-args                 # 引数を空にします
tuipwa update btop --color "#0F766E"
tuipwa update btop --auto-icon                    # 自動生成アイコンに戻します
tuipwa remove notes
```

更新は次回起動から反映されます。

### JSON から登録

```sh
tuipwa validate examples/nano.json
tuipwa add --manifest examples/nano.json
```

全オプションは `tuipwa --help`、JSON の形式は [manifest v1](docs/MANIFEST.md)を参照してください。

### ヘルプの言語

`LC_ALL` → `LC_MESSAGES` → `LANG` の順で空でない値を参照し、日本語ロケールでは日本語、それ以外や未設定の場合は英語で表示します。

```sh
env LC_ALL=C tuipwa --help                  # 英語
env LC_ALL=ja_JP.UTF-8 tuipwa --help       # 日本語
```

言語の切り替えはヘルプ本文のみが対象です。その他の出力やエラーメッセージは全面翻訳していません。

## 開発・仕様

- [CONTRIBUTING.md](CONTRIBUTING.md) — dev ビルド、テスト、リンク付き依存一覧、内部構成、配布・実機確認
- [manifest v1](docs/MANIFEST.md) — 登録 JSON の仕様

## ライセンス

TuiPWA 本体は [MIT License](LICENSE)です。Ghostty・依存ライブラリ・フォントはそれぞれのライセンスに従います。詳細は [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)を参照してください。
