# .NET / Avalonia PoC の検証結果

## 状態
superseded

## 背景
`POC_RESULTS.md` は 2026-11-13 に削除し、要点をこの memory へ移した。全文は git 履歴にある。当時の実装（.NET / Avalonia）は削除済みで、現行実装ではない。現行の検証は mem:native-verification-results。

## 実装
- .NET 10 / Avalonia 12.1.5
- Iciclecreek.Avalonia.Terminal 4.1.3（XTerm.NET 3.0.2 を依存解決）
- Porta.Pty 2.2.3
- 単一ウィンドウに TerminalControl。btop/nano/vim を PATH から選択し、引数で上書き。子の終了コードをタイトル表示。
- 指定の XTerm.NET 3.0.0 は NuGet メタデータの要求（3.0.2 以上）により使えなかった。
- Avalonia 12.0.0 の Tmds.DBus.Protocol 0.90.3 に NU1903 警告。12.1.5 へ統一して解消。

## 検証（2026-11-08）
- Debian、.NET SDK 10.0.403 を `~/.dotnet` に導入。ICU 不在のため `DOTNET_SYSTEM_GLOBALIZATION_INVARIANT=1`。
- Xvfb/デスクトップセッションが無く、Avalonia Headless + Skia で実行。
- `dotnet build TuiPWA.sln` 成功（0 warnings / 0 errors）。
- `dotnet run --project tests/Smoke` 成功: manifest/registry、独立 PTY、cwd/argv、サイズ、入力、正常・異常終了、icon、コマンド不在、close/race/reaping。
- Headless で `/usr/bin/btop` を実起動し、`cpu` 表示、リサイズ後の PTY 行列一致、下矢印が `ESC [ B` として送出、`q` で終了コード 0 を確認。矢印を btop が解釈したこと自体は未検証。

## 制限・未検証
- 実デスクトップの表示、OS タイトルバー、フォーカス、物理キーボード、IME、日本語表示、DPI、描画品質は未検証。
- Windows / ConPTY / WSL はスコープ外。RAM/CPU の独立アイドル測定は未実施。
- HeadlessUnitTestSession の Dispose がウィンドウ close 後に待ち続ける問題は原因未特定。通常アプリは同セッションを使わない。
- ICU 不在の invariant mode は国際化を制限する。

## コード起点
- 実装は削除済み。当時の `TuiPWA.sln` / `src/` / `tests/Smoke` は git 履歴にのみ存在する。
