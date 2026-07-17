# ReiView

透過型 PC モニタリングオーバーレイ。

![screenshot](https://img.shields.io/badge/platform-windows-blue)

## 機能

- CPU 使用率（グローバル + コア別 + スパークライン 60s）
- CPU 温度 / 周波数
- メモリ使用率 / SWAP
- VRAM 使用率（DXGI）
- ディスク使用率（C:）
- ネットワーク転送量（down / up）
- バッテリー残量・充電状態
- プロセス TOP3（CPU / MEM）
- 時計 / 稼働時間
- クリックスルー（lock）→ 通常（movable）切替
- 手動リサイズ（右下グリップ）

## 使い方

1. [Releases](https://github.com/kubobeem/reiviwer/releases) から `ReiView.exe` をダウンロード
2. 実行するだけ
3. `[locked]` をクリック → `[movable]` にするとドラッグ移動可能
4. 右下グリップでサイズ変更

## ビルド

```sh
cargo build --release
```

## 技術スタック

- Rust
- eframe / egui
- sysinfo
- windows（Win32 API）

## ライセンス

MIT
