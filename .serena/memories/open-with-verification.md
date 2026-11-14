# Open With の検証境界

## 状態
unfinished（機能実装済み、KDE実機確認は未実施）

## 決定・理由
ユーザーは「対応の宣言のみ、既定アプリには自動設定しない」を明示選択。mimeapps.list の書換えや xdg-mime default は追加しない。run の -- 後方は任意オプションではなくローカルファイル専用、実行元基準の絶対パス化を選択済み。アプリの登録済みcwdは維持する。

## 検証と罠
Rust 71件・Python 8件、fmt / Clippy / build 成功。隔離XDG領域で生成.desktopの起動先だけargv記録用プローブに置き換え、実GIO（gio launch）で複数ファイル、空白、日本語、リテラル%F、ファイルなしの展開を確認。これはnano/Ghostty GUIの実機検証ではない。
update-desktop-database は環境に存在しないためスタブで呼出先・追加/置換/解除/削除時の呼出し・失敗時警告を確認した。実ツールによるmimeinfo.cache更新とPlasma/Dolphinの候補表示は未検証。これらを「KDEで検証済み」と扱わない。

## コード起点
- src/registry.rs — Registry::refresh_desktop_database / entry — キャッシュ更新・.desktopの対応宣言
- CONTRIBUTING.md — KDE手動確認「ファイルを開く」 — 次の実機確認手順

## 次
desktop-file-utilsのあるKDE環境で、nanoのMIME対応を登録しDolphinから複数ファイルを開く。通常起動・対応解除・既定設定不変も確認する。
