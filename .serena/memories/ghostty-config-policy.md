# Ghosttyユーザー設定の適用境界

## 状態
active

## 決定と理由
ユーザーは既存Ghostty設定の利用を希望し、キーバインドについては選択肢を確認したうえで「無効化を維持」を明示選択した。見た目は共通化するが、TuiPWAをタブ付き汎用端末に戻す意図ではない。ユーザーキー設定を有効にする変更は、この判断を撤回したことにはしない。

## 罠
Ghostty 1.4.0では標準config → CLI → config-fileの再帰読込の順になる。CLIにkeybind=clear等を渡すだけではincludeから上書きされる。TuiPWAホスト判定を再帰読込前に捕捉し、読込後にCLIを再適用するパッチが必要。includeでclassを変えられた後にホスト判定するとこの防護が抜ける。Ghostty更新時にこの順序を再確認する。

## コード起点
- native/linux/patches/0001-window-identity.patch — Config.load内tuipwa_host — include後もホスト制御を優先する境界
- tests/linux/gui.py — ghostty_config / 背景色OSC 11問合せ — 実ホストでの回帰検証

## 検証
専用ホストをmakeで再ビルドし配布物検査成功。隔離Xvfb+DBus GUIテストでlegacy configとconfig.ghosttyとincludeの読み込み（背景色#123456）、includeのclass・起動制御・終了キー設定よりTuiPWA制御が優先することを確認。既存nano/vim/btop等のGUIケースも成功。Rust75件・Python8件・Clippy成功。今回の変更に対するWayland/KDE実機検証は未実施。
