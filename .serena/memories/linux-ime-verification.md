# Linux IME の判断と実機確認

> 履歴記録。この記録は .NET / Avalonia 版についてのもので、実装は削除済み。参照している `src/` と `docs/MVP.md` は git 履歴にのみ存在する。現在の本体はリポジトリ直下の Rust 実装（`docs/NATIVE.md`）。

## 状態
active

## 背景・理由
Linux native MVP の実機起動はユーザーが確認済み。初回は日本語 IME 入力だけ不可だった。ユーザー環境は Wayland + Fcitx5 で、AVALONIA_IM_MODULE / GTK_IM_MODULE / QT_IM_MODULE / XMODIFIERS が未設定。AVALONIA_IM_MODULE=fcitx の明示で入力成功し、Avalonia の環境変数による自動検出だけでは接続先が分からないことが切り分けられた。

## 決定・避けるべき罠
明示設定を優先し、未設定時のみセッション D-Bus から所有者を照会する。Fcitx5 は IBus 互換サービスも同じ接続で提供していたため、両サービス名の存在だけで競合扱いにしてはいけない。同一所有者なら fcitx、別所有者なら勝手に選ばない。Fcitx 固定や OS の設定変更はしない。

## コード起点
- src/TuiPWA.Platform.Linux/LinuxIme.cs — LinuxIme.SelectModule / ConfigureAsync — 所有者による判定と適用境界。Serena のファイル検索で実在確認（C# 言語サーバーなし）。
- docs/MVP.md — MVP 到達結果 — 実機報告と未検証範囲の記録。

## 検証・境界
7e6cd45 の適用後、ユーザーから正常動作の確認を得た（本会話の「ちゃんとできてるね」）。実機の起動・日本語 IME 自動接続成功として扱い、候補位置、全キー、アイコン、メニュー起動、複数ウィンドウの操作まで確認済みと拡大しない。IBus 単独の実変換は未検証。ビルド・通常 Headless スモーク・隔離 D-Bus 判定テストは成功。変更は origin/main に push 済み。
