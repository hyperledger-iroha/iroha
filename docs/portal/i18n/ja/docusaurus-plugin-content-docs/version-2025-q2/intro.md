---
lang: ja
direction: ltr
source: docs/portal/docs/intro.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9f775ae297c910da91c6ce97e97ee36fb87f60218fcfb97639ace6eba39f2252
source_last_modified: "2026-01-03T18:07:57+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SORA Nexus 開発者ポータルへようこそ

SORA Nexus 開発者ポータルには、インタラクティブなドキュメント、SDK がバンドルされています
チュートリアル、および Nexus オペレーターおよび Hyperledger Iroha の API リファレンス
貢献者。実践的なガイドを表示することで、メインのドキュメント サイトを補完します。
このリポジトリから直接仕様を生成しました。ランディング ページには現在、
テーマ別 Norito/SoraFS エントリ ポイント、署名付き OpenAPI スナップショット、および専用
Norito ストリーミング リファレンス。投稿者はストリーミング コントロール プレーンを見つけることができます。
ルート仕様を掘り下げずに契約します。

## ここでできること

- **Norito を学習** – 概要とクイックスタートから始めて、
  シリアル化モデルとバイトコード ツール。
- **ブートストラップ SDK** – 今すぐ JavaScript と Rust のクイックスタートに従ってください。パイソン、
  レシピが移行されると、Swift ガイドと Android ガイドが加わります。
- **API リファレンスを参照** – Torii OpenAPI ページは最新の REST をレンダリングします
  仕様と構成テーブルは正規のマークダウンにリンクされています。
  ソース。
- **展開の準備** – 運用ランブック (テレメトリ、決済、Nexus)
  オーバーレイ) は `docs/source/` から移植されており、このサイトに次のように表示されます。
  移行が進みます。

## 現在のステータス

- ✅ テーマ Docusaurus v3 ランディング、更新されたタイポグラフィ、グラデーション駆動
  ヒーロー/カード、および Norito ストリーミング概要を含むリソース タイル。
- ✅ Torii OpenAPI プラグインが `npm run sync-openapi` に接続され、署名付きスナップショット付き
  `buildSecurityHeaders` によって強制されるチェックと CSP ガード。
- ✅ CI で実行されるプレビューとプローブ カバレッジ (`docs-portal-preview.yml` +
  `scripts/portal-probe.mjs`)、ストリーミング ドキュメント、SoraFS クイックスタートをゲート中、
  およびアーティファクトが公開される前の参照チェックリスト。
- ✅ Norito、SoraFS、SDK クイックスタートとリファレンス セクションは、
  サイドバー; `docs/source/` からの新しいインポート (ストリーミング、オーケストレーション、Runbook)
  作成されたとおりにここに着陸します。

## 参加する

- ローカル開発コマンドについては、`docs/portal/README.md` を参照してください (`npm install`、
  `npm run start`、`npm run build`)。
- コンテンツ移行タスクは、`DOCS-*` ロードマップ項目とともに追跡されます。
  貢献は歓迎です - `docs/source/` からセクションを移植し、ページを追加します
  サイドバーに。
- 生成されたアーティファクト (仕様、構成テーブル) を追加する場合は、ビルドを文書化します。
  コマンドを使用すると、将来の寄稿者が簡単に更新できるようになります。