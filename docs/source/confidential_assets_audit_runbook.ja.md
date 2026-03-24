---
lang: ja
direction: ltr
source: docs/source/confidential_assets_audit_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8691a94d23e589f46d8e8cf2359d6d9a31f7c38c5b7bf0def69c88d2dd081765
source_last_modified: "2026-01-22T15:38:30.658489+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! `roadmap.md:M4` によって参照される機密資産の監査および運用ハンドブック。

# 機密資産の監査と運用ランブック

このガイドは、監査人およびオペレーターが依存する証拠表面を統合します。
機密資産フローを検証するとき。ローテーション プレイブックを補完します
(`docs/source/confidential_assets_rotation.md`) および校正台帳
(`docs/source/confidential_assets_calibration.md`)。

## 1. 選択的開示とイベントフィード

- すべての機密命令は構造化された `ConfidentialEvent` ペイロードを発行します
  (`Shielded`、`Transferred`、`Unshielded`) でキャプチャされました
  `crates/iroha_data_model/src/events/data/events.rs:198` によってシリアル化されています。
  実行者 (`crates/iroha_core/src/smartcontracts/isi/world.rs:3699` ～ `4021`)。
  回帰スイートは具体的なペイロードを実行するため、監査人は信頼できる
  決定的な JSON レイアウト (`crates/iroha_core/tests/zk_confidential_events.rs:19` ～ `299`)。
- Torii は、標準の SSE/WebSocket パイプライン経由でこれらのイベントを公開します。監査役
  `ConfidentialEventFilter` (`crates/iroha_data_model/src/events/data/filters.rs:82`) を使用してサブスクライブします。
  オプションで単一の資産定義にスコープを設定します。 CLI の例:

  ```bash
  iroha ledger events data watch --filter '{ "confidential": { "asset_definition_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM" } }'
  ```

- ポリシーのメタデータと保留中の移行は、以下から入手できます。
  `GET /v1/confidential/assets/{definition_id}/transitions`
  (`crates/iroha_torii/src/routing.rs:15205`)、Swift SDK によってミラーリングされます
  (`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:3245`) に文書化されています。
  機密資産設計と SDK ガイドの両方
  (`docs/source/confidential_assets.md:70`、`docs/source/sdk/swift/index.md:334`)。

## 2. テレメトリー、ダッシュボード、および校正証拠

- ランタイムメトリクスのサーフェスツリーの深さ、コミットメント/フロンティア履歴、ルートエビクション
  カウンタとベリファイアキャッシュのヒット率
  (`crates/iroha_telemetry/src/metrics.rs:5760` ～ `5815`)。 Grafana ダッシュボード
  `dashboards/grafana/confidential_assets.json` は関連するパネルを出荷し、
  `docs/source/confidential_assets.md:401` に文書化されたワークフローを使用したアラート。
- 署名付きログが保存されたキャリブレーション実行 (NS/op、gas/op、ns/gas)
  `docs/source/confidential_assets_calibration.md`。最新のAppleシリコン
  NEON の実行は次の場所にアーカイブされています。
  `docs/source/confidential_assets_calibration_neon_20260428.log`、および同じ
  台帳には、SIMD ニュートラルおよび AVX2 プロファイルの一時的な免除が記録されるまで
  x86 ホストがオンラインになります。

## 3. インシデント対応とオペレーターのタスク

- ローテーション/アップグレード手順は、
  `docs/source/confidential_assets_rotation.md`、新しいステージング方法をカバー
  パラメータバンドル、ポリシーアップグレードのスケジュール、ウォレット/監査者への通知。の
  トラッカー (`docs/source/project_tracker/confidential_assets_phase_c.md`) リスト
  ランブックの所有者とリハーサルの期待。
- 制作リハーサルまたは緊急窓口の場合、オペレーターは証拠を添付します。
  `status.md` エントリ (マルチレーン リハーサル ログなど) には次のものが含まれます。
  `curl` ポリシー移行の証明、Grafana スナップショット、および関連イベント
  監査人がミント→転送→公開のタイムラインを再構築できるようにダイジェストを作成します。

## 4. 外部レビューの頻度

- セキュリティレビュー範囲: 秘密回線、パラメータレジストリ、ポリシー
  トランジションとテレメトリ。この文書と校正台帳フォーム
  ベンダーに送信される証拠パケット。レビューのスケジュールは次の方法で追跡されます。
  `docs/source/project_tracker/confidential_assets_phase_c.md` の M4。
- オペレータは、ベンダーの調査結果やフォローアップで `status.md` を最新の状態に保つ必要があります。
  アクションアイテム。外部レビューが完了するまで、このランブックは
  運用ベースラインの監査人がテストできる。