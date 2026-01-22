---
lang: ja
direction: ltr
source: docs/source/sdk/swift/readiness/screenshots/2026-02-28/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4c3e0d606966f67b7fdb2ef7862bcaa955c117cb9ae645f5c3626c75c199ec35
source_last_modified: "2026-03-01T14:02:06.725705+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/sdk/swift/readiness/screenshots/2026-02-28/README.md -->

# スクリーンショット索引 — Swift テレメトリ Dry-Run (2026-02-28)

リハーサルの PNG/JPEG キャプチャをここに追加する。ファイルは
`s3://sora-readiness/swift/telemetry/20260228/` に保存し、リポジトリの肥大化を回避する。
下表はアーカイブノートで参照される正準オブジェクト名を一覧化する。

| ファイル | シナリオ | 説明 |
|------|----------|-------------|
| `s3://sora-readiness/swift/telemetry/20260228/20260228-salt-drift-alert.png` | A | `mobile_parity.swift` のアラートパネル。`swift.telemetry.redaction.salt_drift` の発火とクリアを表示。 |
| `s3://sora-readiness/swift/telemetry/20260228/20260228-override-ledger.png` | B | `scripts/swift_status_export.py telemetry-override create …` で作成した override 台帳エントリ。 |
| `s3://sora-readiness/swift/telemetry/20260228/20260228-exporter-outage.png` | C | OTLP コレクタ停止のエクスポータ障害ダッシュボード。抑制 + 復旧タイムラインを強調。 |
| `s3://sora-readiness/swift/telemetry/20260228/20260228-offline-queue.png` | D | airplane-mode 実行の前後を比較する offline queue リプレイメトリクス。 |
| `s3://sora-readiness/swift/telemetry/20260228/20260228-connect-latency.png` | E | Connect レイテンシのヒストグラム。450 ms の誘発スパイクとアラート注釈を表示。 |
