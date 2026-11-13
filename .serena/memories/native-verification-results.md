# Ghostty native host の検証結果

## 状態
active

## 背景
`docs/RESULTS.md` は 2026-11-13 に削除し、要点をこの memory へ移した。削除時点の全文は git 履歴（削除コミットの親）にある。旧 .NET PoC は mem:poc-dotnet-results。

## 判定
- 共通 GUI ホスト化: 成立（自動実行で確認）
- Wayland のウィンドウ別 app_id: 成立（Weston 上の実プロトコルで確認）
- KDE Plasma の desktop 関連付け・アイコン・グループ化: 未検証。成功扱いしない。

## 検証環境
- Debian GNU/Linux 13 (trixie), x86_64
- Ghostty v1.4.0 / 83fb95624b0439ee660c2679b43b38c8b63769cd
- Zig 0.17.0、Rust/Cargo 1.99.1
- GTK 4.18.6 / libadwaita 1.7.6 / GLib 2.84.4
- Weston 14.0.2 headless + pixman
- Ghostty ビルド: ReleaseSmall, baseline CPU, j1
- 公式 tar SHA-256: cb188ac6c3a8278d5fd7bfef15d8c017104c87f638b5560eb38024a71ae0f005

## 自動検証（2026-11-13 再実行）
- `cargo test --locked`: 61 passed（記録当時は 51）
- `cargo clippy --locked --all-targets -- -D warnings`: 成功
- `cargo fmt --all --check`: 成功
- `tests/gui.py`: PASS。同時初回起動が同一ホスト、独立 PTY、argv/cwd、固定タイトル、ランチャー終了後もウィンドウ維持、SIGTERM で対象のみ終了、nano/btop/vim の正常終了で自動クローズ、`/bin/true`・`/bin/false` の即時終了、最後のウィンドウでホスト終了、ホスト SIGKILL 後の復旧、起動失敗時のウィンドウ保持、runtime 権限、未使用ホストの watchdog。
- `tests/wayland.py`: PASS。nano/btop/vim を起動し、xdg_toplevel ごとに `tuipwa-nano` / `tuipwa-btop` / `tuipwa-vim` の app_id とタイトルを確認。close で片方のみ終了、最後の close と `/bin/true` の即時終了でホスト終了。
- `ldd dist/bin/tuipwa`: GLib/GIO/libc 等のみ。managed runtime なし。

## Wayland 設定タイミングの知見
- GTK 4.18.6 の `gdk_wayland_toplevel_set_application_id` は xdg_toplevel 未生成時に no-op。
- `GtkWidget.realize` 後・present 前では実送信されず、全ウィンドウが `org.tuipwa.NativeHost` のまま。
- present 直後に呼ぶと個別 ID が送信される。初期 ID は共通 ID で、その後更新される。
- 初期 ID だけで関連付けるシェルでは一瞬グループが変動し得る。hide/unmap→remap で共通 ID が再送されるケースへの再適用は未実装。
- X11 は共通 WM_CLASS + 固定タイトル + desktop の Icon にフォールバック。GTK ヘッダーへ任意 PNG/ICO は埋め込まない。

## 未検証・制約
- KDE Plasma のタスクバーアイコン、ウィンドウ別グループ化、ピン留め、メニューでのカテゴリ階層表示は実機未確認。
- `desktop-file-validate` は環境に無く未実行。Categories は単体テストと CI スモークで検証。
- 生成アイコンの実タスクバー表示、システムアイコンのテーマ継承・優先順位は未評価。
- push 後の GitHub Actions 結果は未確認（この memory 作成時点）。
- 同一 UID の悪意あるプロセスは隔離しない。ACK は bounded だが exactly-once ではない。setsid 等で離脱する子孫は封じ込めない。フォント/GTK 依存の再配布監査は未実施。
- 性能ベンチマークは未実施。独立レビュアーは使わず自己レビュー。

## コード起点
- `tests/gui.py` — `windows` — X11/Xvfb のウィンドウ列挙と自動クローズ E2E
- `tests/wayland.py` — `close` — Weston 上の close とホスト終了
- `src/linux.rs` — `session` — 子終了でセッション補助を終了させる実装
- `docs/NATIVE.md` — `KDE Plasma実機確認手順` — 実機確認の観点
