<!-- Japanese translation of docs/source/iroha_monitor.md -->

---
lang: ja
direction: ltr
source: docs/source/iroha_monitor.md
status: complete
translator: manual
---

# Iroha Monitor

リニューアルされた Iroha Monitor は、軽量なターミナル UI に祭囃子風の ASCII アニメーションと伝統的な越殿楽テーマを組み合わせ、次の 2 つのワークフローに特化しています。

- **Spawn-lite モード** — ピアを模したステータス／メトリクスのスタブを一時的に起動。
- **Attach モード** — 既存の Torii HTTP エンドポイントへ接続。

画面は常に以下の領域で構成されます。

1. **Torii スカイラインヘッダー** — 更新周期と同期してスクロールする鳥居・富士山・鯉の波・星空のアニメーション。
2. **サマリーストリップ** — ブロック／トランザクション／ガスの集計値とリフレッシュタイミング。
3. **ピアテーブル＆囃子ログ** — 左にピア行、右に警告（タイムアウト、大きすぎるペイロードなど）を回転表示するイベントログ。
4. **オプションのガストレンド** — `--show-gas-trend` を指定すると、全ピアの総ガス使用量を示すスパークラインを追加。

主な変更点:
- 鯉や鳥居、提灯が登場する和風 ASCII アニメーション。
- シンプルになった CLI（`--spawn-lite`, `--attach`, `--interval` など）。
- オプションで越殿楽のイントロを再生（外部 MIDI プレーヤーまたは `builtin-synth` フラグによるソフトシンセ）。
- CI や高速スモーク用に `--no-theme` / `--no-audio` を用意。
- ピアごとの“気分”列で最新警告やコミット時間、稼働時間を表示。

## クイックスタート

スタブピアを起動:
```bash
cargo run -p iroha_monitor -- --spawn-lite --peers 3
```

既存の Torii に接続:
```bash
cargo run -p iroha_monitor -- \
  --attach http://127.0.0.1:8080 http://127.0.0.1:8081 \
  --interval 500
```

CI 向け（アニメ・音声OFF）:
```bash
cargo run -p iroha_monitor -- --spawn-lite --no-theme --no-audio
```

### CLI フラグ一覧
```
--spawn-lite         ローカルスタブを起動（--attach 未指定時の既定）
--attach <URL...>    既存の Torii エンドポイントへ接続
--interval <ms>      リフレッシュ周期（既定 800ms）
--peers <N>          spawn-lite 時のスタブ数（既定 4）
--no-theme           アニメーションイントロをスキップ
--no-audio           テーマ再生をミュート（アニメは表示）
--midi-player <cmd>  外部 MIDI プレーヤーコマンド
--midi-file <path>   `--midi-player` 用のカスタム MIDI
--show-gas-trend     ガススパークラインパネルを表示
--art-speed <1-8>    アニメ速度倍率（既定 1）
--art-theme <name>   night / dawn / sakura パレットを選択
--headless-max-frames <N>
                     ヘッドレスモードの最大フレーム数（0 は無制限）
```

## テーマイントロ

起動時は越殿楽を流しながら ASCII アニメーションを再生します。音声の選択順:

1. `--midi-player` 指定時はデモ MIDI（または `--midi-file`）を生成し外部コマンドを起動。
2. `builtin-synth` フィーチャー有効時は内蔵ソフトシンセで再生。
3. 音声無効または初期化失敗時はアニメのみ表示して即座に TUI に入ります。

CI や無音環境では `--no-theme` / `--no-audio` を利用してください。ソフトシンセは *MIDI synth design in Rust.pdf* に基づき、篳篥と龍笛の異旋律に笙の aitake を重ねるアレンジです。譜面データは `etenraku.rs` にあり、CPAL コールバックとデモ MIDI の両方を駆動します。

## UI 概要

- **ヘッダーアート** — `AsciiAnimator` がフレームごとに生成。鯉や提灯が漂う連続モーション。
- **サマリー帯** — オンラインピア数、報告ピア数、ブロック数、非空ブロック、承認／拒否、ガス、更新レートを表示。
- **ピアテーブル** — エイリアス／エンドポイント、ブロック、トランザクション、キューサイズ、ガス、レイテンシ、ムードを一覧。
- **祭囃子ログ** — 接続エラー、ペイロード超過、遅延などの警告を新着順で表示。

ショートカット:
- `n` / 右 / ↓ — 次のピアへフォーカス。
- `p` / 左 / ↑ — 前のピアへ戻る。
- `q` / Esc / Ctrl-C — 終了してターミナルを復元。

内部的には crossterm + ratatui の代替画面バッファを利用し、終了時にカーソルと画面を元に戻します。

## スモークテスト

両モードと HTTP 制限をカバーする統合テストが同梱されています。
- `spawn_lite_smoke_renders_frames`
- `attach_mode_with_stubs_runs_cleanly`
- `invalid_endpoint_surfaces_warning`
- `status_limit_warning_is_rendered`
- `attach_mode_with_slow_peer_renders_multiple_frames`
