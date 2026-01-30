---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/intro.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7717846f070df2b9c0db35b672b75f33f41f934ec87ccbd8205d72449d6d3488
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ja
direction: ltr
source: docs/portal/docs/intro.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# SORA Nexus 開発者ポータルへようこそ

SORA Nexus 開発者ポータルは、Nexus オペレーターと Hyperledger Iroha の貢献者向けに、インタラクティブなドキュメント、SDK チュートリアル、API リファレンスをまとめています。本リポジトリから直接生成された実践的ガイドと仕様を前面に出すことで、メインの docs サイトを補完します。ランディングページには、Norito/SoraFS のテーマ別エントリポイント、署名済み OpenAPI スナップショット、専用の Norito Streaming リファレンスが用意され、貢献者はルート仕様を掘り返さずにストリーミング制御プレーンのコントラクトを見つけられます。

## ここでできること

- **Norito を学ぶ** - 概要とクイックスタートから始め、シリアライゼーションモデルとバイトコードツールを理解します。
- **SDK を立ち上げる** - まずは JavaScript と Rust のクイックスタートを利用。Python、Swift、Android のガイドはレシピ移行に合わせて追加されます。
- **API リファレンスを閲覧** - Torii OpenAPI ページが最新の REST 仕様を描画し、設定テーブルはカノニカルな Markdown ソースへリンクします。
- **デプロイ準備** - 運用 runbook（telemetry、settlement、Nexus overlays）は `docs/source/` から移行中で、移行の進行に合わせてここに掲載されます。

## 現在の状況

- ✅ リフレッシュされたタイポグラフィ、グラデーション主導の hero/カード、Norito Streaming 概要を含むリソースタイルを備えたテーマ付き Docusaurus v3 ランディング。
- ✅ Torii OpenAPI プラグインを `npm run sync-openapi` に接続し、署名済みスナップショット検証と `buildSecurityHeaders` による CSP ガードを実施。
- ✅ Preview と probe のカバレッジが CI (`docs-portal-preview.yml` + `scripts/portal-probe.mjs`) で実行され、ストリーミング文書、SoraFS クイックスタート、参照チェックリストの公開前ゲートになっています。
- ✅ Norito、SoraFS、SDK のクイックスタートと参照セクションがサイドバーに公開済み。`docs/source/` からの新規取り込み（streaming, orchestration, runbooks）も作成に合わせてここへ追加されます。

## 参加方法

- ローカル開発コマンドは `docs/portal/README.md` を参照（`npm install`, `npm run start`, `npm run build`）。
- コンテンツ移行タスクは `DOCS-*` ロードマップ項目と併せて追跡されています。貢献歓迎 - `docs/source/` のセクションを移植し、サイドバーにページを追加してください。
- 生成物（specs, config tables）を追加する場合は、ビルドコマンドを記載して将来の貢献者が容易に再生成できるようにしてください。
