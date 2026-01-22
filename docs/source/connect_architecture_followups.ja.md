---
lang: ja
direction: ltr
source: docs/source/connect_architecture_followups.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 476331772efe169a7a073b561fa9935e314ff89d6bfff440f7246606c1c02669
source_last_modified: "2026-01-03T18:07:58.049266+00:00"
translation_last_reviewed: 2026-01-22
---

<!-- 日本語訳: docs/source/connect_architecture_followups.md -->

# Connect アーキテクチャのフォローアップアクション

このメモは、クロス SDK の Connect アーキテクチャレビューから生まれたエンジニアリングのフォローアップを記録する。
各行は、作業がスケジュールされたら issue (Jira チケットまたは PR) に対応させること。
担当者がトラッキングチケットを作成したら表を更新すること。

| 項目 | 説明 | 担当 | トラッキング | ステータス |
|------|-------------|----------|----------|--------|
| 共有 back-off 定数 | 指数 back-off + ジッターのヘルパー (`connect_retry::policy`) を実装し、Swift/Android/JS SDK に公開する。 | Swift SDK, Android Networking TL, JS Lead | [IOS-CONNECT-001](project_tracker/connect_architecture_followups_ios.md#ios-connect-001) | 完了 — `connect_retry::policy` は決定的な splitmix64 サンプリングで実装され、Swift (`ConnectRetryPolicy`)、Android、JS SDK がミラーのヘルパーと golden テストを提供。 |
| ping/pong 強制 | 合意された 30 秒の cadence とブラウザ最小クランプで、設定可能なハートビート強制を追加し、メトリクス (`connect.ping_miss_total`) を公開する。 | Swift SDK, Android Networking TL, JS Lead | [IOS-CONNECT-002](project_tracker/connect_architecture_followups_ios.md#ios-connect-002) | 完了 — Torii は設定可能な heartbeat 間隔 (`ping_interval_ms`, `ping_miss_tolerance`, `ping_min_interval_ms`) を強制し、`connect.ping_miss_total` メトリクスを公開し、heartbeat 切断処理の回帰テストを同梱。SDK の機能スナップショットが新しいノブをクライアント向けに公開。 |
| オフラインキュー永続化 | Connect キュー向け Norito `.to` ジャーナルの writer/reader を実装する (Swift `FileManager`、Android 暗号化ストレージ、JS IndexedDB)。共有スキーマを使う。 | Swift SDK, Android Data Model TL, JS Lead | [IOS-CONNECT-003](project_tracker/connect_architecture_followups_ios.md#ios-connect-003) | 完了 — Swift/Android/JS が共有の `ConnectQueueJournal` + diagnostics ヘルパーを提供し、保持/オーバーフローのテストで証拠バンドルが SDK 間で決定的になることを保証。【IrohaSwift/Sources/IrohaSwift/ConnectQueueJournal.swift:1】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/ConnectQueueJournal.java:1】【javascript/iroha_js/src/connectQueueJournal.js:1】 |
| StrongBox アテステーションペイロード | `{platform,evidence_b64,statement_hash}` をウォレット承認フローに通し、dApp SDK に検証を追加する。 | Android Crypto TL, JS Lead | [IOS-CONNECT-004](project_tracker/connect_architecture_followups_ios.md#ios-connect-004) | 未着手 |
| ローテーション制御フレーム | `Control::RotateKeys` + `RotateKeysAck` を実装し、`cancelRequest(hash)` とローテーション API を全 SDK に公開する。 | Swift SDK, Android Networking TL, JS Lead | [IOS-CONNECT-005](project_tracker/connect_architecture_followups_ios.md#ios-connect-005) | 未着手 |
| テレメトリエクスポータ | 既存のテレメトリパイプライン (OpenTelemetry) に `connect.queue_depth`、`connect.reconnects_total`、`connect.latency_ms`、リプレイカウンタを送出する。 | Telemetry WG, SDK owners | [IOS-CONNECT-006](project_tracker/connect_architecture_followups_ios.md#ios-connect-006) | 未着手 |
| Swift CI ゲーティング | Connect 関連パイプラインで `make swift-ci` を実行し、fixture parity、ダッシュボード feed、Buildkite `ci/xcframework-smoke:<lane>:device_tag` メタデータが SDK 間で整合するようにする。 | Swift SDK Lead, Build Infra | [IOS-CONNECT-007](project_tracker/connect_architecture_followups_ios.md#ios-connect-007) | 未着手 |
| フォールバック インシデント報告 | XCFramework smoke ハーネスのインシデント (`xcframework_smoke_fallback`, `xcframework_smoke_strongbox_unavailable`) を Connect ダッシュボードへ配線して可視化する。 | Swift QA Lead, Build Infra | [IOS-CONNECT-008](project_tracker/connect_architecture_followups_ios.md#ios-connect-008) | 未着手 |
| コンプライアンス添付のパススルー | SDK が承認ペイロードの `attachments[]` と `compliance_manifest_id` を欠損なく受け入れ、転送できるようにする。 | Swift SDK, Android Data Model TL, JS Lead | [IOS-CONNECT-009](project_tracker/connect_architecture_followups_ios.md#ios-connect-009) | 未着手 |
| エラー分類の整合 | 共有 enum (`Transport`, `Codec`, `Authorization`, `Timeout`, `QueueOverflow`, `Internal`) をプラットフォーム固有のエラーへマッピングし、ドキュメント/例を整備する。 | Swift SDK, Android Networking TL, JS Lead | [IOS-CONNECT-010](project_tracker/connect_architecture_followups_ios.md#ios-connect-010) | 完了 — Swift/Android/JS SDK が共有の `ConnectError` ラッパー + テレメトリヘルパーを提供し、README/TypeScript/Java のドキュメントと TLS/timeout/HTTP/codec/queue ケースの回帰テストを同梱。【docs/source/connect_error_taxonomy.md:1】【IrohaSwift/Sources/IrohaSwift/ConnectError.swift:1】【java/iroha_android/src/test/java/org/hyperledger/iroha/android/connect/ConnectErrorTests.java:1】【javascript/iroha_js/test/connectError.test.js:1】 |
| ワークショップ決定ログ | 合意済みの決定をまとめた注釈付きデッキ/ノートを評議会アーカイブに公開する。 | SDK Program Lead | [IOS-CONNECT-011](project_tracker/connect_architecture_followups_ios.md#ios-connect-011) | 未着手 |

> トラッキング ID は担当者がチケットを作成したら埋める。issue の進捗に合わせて `Status` 列を更新すること。
