# TuiPWA

登録した TUI アプリを、タブなし・通常タイトルバーの独立ウィンドウで実行する Linux x64 用ランチャー。本体は Ghostty をネイティブホストに使う Rust 実装（リポジトリ直下）。

2026-11 に .NET / Avalonia 版（旧 `src/`・`tests/`・`TuiPWA.sln`・.NET 向け docs）を削除した。必要な場合は git 履歴（削除コミットの親）から参照する。旧 PoC の記録は mem:poc-dotnet-results、現行の検証記録は mem:native-verification-results に残している。

- 公開バイナリは `tuipwa`。CLI は `add` / `remove` / `list` / `show` / `update` / `run` / `validate`。使い方は `README.md`、開発・内部構成は `CONTRIBUTING.md`。
- 共有 GUI ホスト `ghostty-host` を全アプリで 1 個。ウィンドウごとに独立 PTY と Wayland `app_id` = `tuipwa-<id>`。
- 保存先: `${XDG_CONFIG_HOME}/tuipwa/apps/<id>.json`、`tuipwa/meta/<id>.json`、`${XDG_DATA_HOME}/tuipwa/icons/<id>.png`、`${XDG_DATA_HOME}/applications/tuipwa-<id>.desktop`。
- manifest v1（`docs/MANIFEST.md`）は登録ファイルの仕様。アイコン再生成情報は meta に分離し、manifest v1 へ新フィールドを足さない。
- 通常導入は `make install`。`cargo install --path . --locked` で CLI を Cargo 標準先へ、ホスト・リソース・通知を `${XDG_DATA_HOME:-~/.local/share}/tuipwa/native/` へ配置する。ホスト検索は明示指定 → CLI の隣 → debug 時のリポジトリ dist → ユーザーデータ領域。不正な明示指定はフォールバックしない。sudo・シェル設定変更は行わない。
- ビルドのみはリポジトリのルートで `make`。Zig 0.17.0 と Ghostty 1.4.0 を取得・SHA-256 照合・パッチ適用し、`dist/` へ配置する。キャッシュは `~/.cache/tuipwa-native`、`TUIPWA_BUILD_CACHE` で変更可。
- 検証: `make test`（cargo test --locked + Python 配布物検査の単体テスト）、`make lint`（cargo clippy -- -D warnings）。GUI は `tests/linux/gui.py`（Xvfb/X11）と `tests/linux/wayland.py`（Weston）、`dbus-run-session` + `xvfb-run` で実行。KDE タスクバーの実機挙動は自動検証していない（手順は CONTRIBUTING.md）。
- 起動受付は `$XDG_RUNTIME_DIR/tuipwa-native/launch.lock` の flock → D-Bus `org.tuipwa.NativeHost` → `org.gtk.Actions.Activate("new-window-command")` → 補助プロセス経由で PTY。ACK までロックを保持し、30 秒でタイムアウトする。
- ウィンドウ識別は present 直後に `gdk_wayland_toplevel_set_application_id` を呼ぶため、最初の一瞬だけ共通ホスト ID になる。初期 ID のみで関連付けるコンポジターは保証外。

ユーザーの Wayland + Fcitx5 環境での実機報告は mem:ghostty-native-host-verification。旧 .NET 版の IME 成功報告（mem:linux-ime-verification）は新実装の証拠に流用しない。

配布方針: パッチ済み Ghostty＋共有ホストを維持し同梱する。GTK/libadwaita/GLib は OS の動的依存としてライブラリ本体を同梱しない（GTK 依存の撤去ではない）。TuiPWA 独自コード・文書・アセット・パッチは MIT（LICENSE）。第三者依存・Ghostty 組込みフォントは固有ライセンスを維持。`make` は MIT/第三者通知/LGPL 本文を配置し `native/linux/check-dist.py` で配布境界を検査する。再配布監査完了とは扱わない。README は概要・導入・主要コマンドとよく使うオプションの使用例を中心に保つ。利用に必要なコマンド説明まで外部文書へ追い出さない。README に必要なのは導入・主要コマンド・使用例であり、旧 CLI.md の詳細仕様・規則・保存先・エラー説明を網羅的に移植しない。CLI.md は削除だけでよいというユーザー方針を優先する。dev・テスト・リンク付き依存一覧・内部構成・配布・実機確認は CONTRIBUTING.md にまとめる。docs/ には手書き JSON の契約である MANIFEST.md のみ残し、BUILD.md・CLI.md・platform/linux.md は再作成しない。

Windows/WSL は Windows GUI ホストから WSL 内の TUI を起動する方式（B）を将来方針として選択。今回は再配置だけに限定し、Windows の仮実装・汎用バックエンド trait・workspace 化は行わない。Linux は `src/platform/linux/`、ホストパッチは `native/linux/patches/`、アイコン処理は `src/icons/` に配置。`registry.rs` / `cli.rs` 内の OS 依存分離は Windows 実装時まで保留する。

README.md は日本語のまま維持し、README.en.md を英語版として相互リンクする。ヘルプ本文は LC_ALL → LC_MESSAGES → LANG の空でない値を優先し、日本語ロケールだけ日本語、それ以外・未設定は英語にする。診断や登録結果等の全面翻訳は未対応（src/cli.rs の usage_for_locale）。公開する日本語文書（README・CONTRIBUTING・docs 本文）は敬体（です・ます調）に統一する。依存は必要なツール・ライブラリ名で示し、Debian 等への環境固定やディストリビューション別のパッケージ導入例は追加しない。CI の実際の実行環境とは区別する。第三者通知（THIRD_PARTY_NOTICES.md）は簡潔な英語で記述する。見出し・項目名などの名詞句、コマンド例、第三者ライセンス原文はそのまま保持する。

C# 言語サーバーは無効。Serena の symbol 操作は使わず、ファイル検索・読込で代替する。言語設定を勝手に変更しない。
