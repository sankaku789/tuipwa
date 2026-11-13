# Ghostty Native Host 検証境界

## 状態
unfinished（PoC実装と自動GUI検証済み、KDE/IME実機確認待ち）

## 決定と理由
既存.NET製品は比較用に無変更のまま、指定poc/ghostty-native-hostブランチだけで実験を分離する。共通GUIホストの成立とKDEのアプリ別識別の成立を混同しないこと。実行したXvfb/Westonテストは成功しているが、KDE taskbar/desktopアイコン・グループ化と日本語IME実操作は未検証。アプリ別ホストへの暗黙変更は禁止。

## 再発しやすい罠
GTK4.18のgdk_wayland_toplevel_set_application_idはGtkWidget.realize後でもxdg_toplevel未生成だとno-op。present直後に設定すると実送信されるが、最初のIDは共通IDで後から更新される。APIの存在だけで成功判定しない。未使用ホストは起動watchdogで終了させる。短命ランチャーからホストへのstderr継承は呼出元pipeのEOFを阻害するためruntimeログへ分離。TUIのforeground process group設定は子のexec前に行う（後だとSIGTTIN競合）。

## コード起点
- mem:native-verification-results — Wayland設定タイミングの重要な実験結果 / 残る制約・未検証 — 実証と非実証の境界

## 次
KDE Plasma Wayland実機で別アイコンを持つ2登録のdesktop起動、taskbar grouping、最小化復元、日本語変換候補位置を確認する。既存.NET版の過去のIME成功報告を新ネイティブ版の証拠に流用しない。
