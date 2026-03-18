---
lang: ja
direction: ltr
source: docs/source/soranet_gateway_bug_bounty.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ef770782a60b646faacc449ab09a1c09311b6e5201311da4e4f959f2d917b575
source_last_modified: "2026-01-03T18:07:56.918673+00:00"
translation_last_reviewed: 2026-01-22
---

<!-- 日本語訳: docs/source/soranet_gateway_bug_bounty.md -->

# SNNet-15H1 — ペネトレーションテスト & バグバウンティ キット

`cargo xtask soranet-bug-bounty` を使って、SoraGlobal Gateway CDN の bug bounty
プログラム向けに再現可能なパケットを生成する。ヘルパーは設定が edge、control-plane、
課金サーフェスをカバーしていることを検証し、次を出力する:

- `bug_bounty_overview.md` — オーナー、パートナー、スコープ/ケイデンス、SLA、報奨、ダッシュボード/ポリシーへのリンク。
- `triage_checklist.md` — 受け付けチャネル、重複ポリシー、証拠要件、プレイブック、サーフェスごとのチェックポイント。
- `remediation_template.md` — 開示ウィンドウと証拠プレースホルダーを備えた決定的レポートテンプレート。
- `bug_bounty_summary.json` — ガバナンスパケット向け Norito JSON サマリー (出力ディレクトリからの相対パス)。

## 使用方法

```
cargo xtask soranet-bug-bounty \
  --config fixtures/soranet_bug_bounty/sample_plan.json \
  --output-dir artifacts/soranet/gateway/bug_bounty/snnet-15h1
```

オプション:
- `--config <path>`: オーナー、パートナー、スコープ、SLA、トリアージ、報奨、レポートを記述した Norito JSON プラン。必須。
- `--output-dir <path>`: 出力先ディレクトリ。既定は `artifacts/soranet/gateway/bug_bounty`。

スコープのガードレール:
- 必須領域: `edge`, `control-plane`, `billing`。いずれかが欠けていたりターゲットが空の場合、コマンドは即時に失敗する。
- 各領域のケイデンスは空であってはならない。

## 証跡バンドルの形
- サマリーフィールド (`program`, `slug`, オーナー、パートナー、スコープ、SLA、報奨、トリアージ、reporting) は相対出力パスと共に JSON に書き込まれる。
- Markdown ファイルには監査用に生成タイムスタンプが記載される。
- remediation テンプレートは設定の開示ウィンドウを反映し、下流のアップロードが公開ポリシーと整合する。

## 決定性
- 全てのコンテンツは提供された設定から生成され、ネットワーク呼び出しは行わない。
- 出力ディレクトリ内のファイル名は固定で、CI スナップショットとガバナンスバンドルの安定性を保つ。
