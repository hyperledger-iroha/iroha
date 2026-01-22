---
lang: ja
direction: ltr
source: docs/source/project_tracker/norito_streaming_post_mvp.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4e6f0d09f24f7d3111fc53856393a0ad4be93f5a2f45c5150f053f1e153f1ec6
source_last_modified: "2026-01-06T15:14:01.036336+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/project_tracker/norito_streaming_post_mvp.md -->

<!-- Tracker detailing post-MVP Norito Streaming follow-ups. -->

# Norito Streaming — Post-MVP バックログ

| ID | タスク | 担当 | 目標 | ステータス | 次のアクション | 備考 |
|----|------|--------|--------|--------|-------------|-------|
| NSC-28b | バリデータテレメトリとセグメント監査で ±10ms の A/V 同期許容を強制 | Streaming Runtime TL, Telemetry Ops | Q2 2026 | 🟡 計画中 | テレメトリ信号仕様（ジッタ/ドリフトバケット）を作成し、バリデータ計測レビューをスケジュール（3/2 週）。 | 仕様アウトライン: `docs/source/project_tracker/nsc28b_av_sync_telemetry.md`。`StreamingTelemetry` に jitter/drift メトリクスを追加し、`SegmentAuditor` にゲートを追加、決定論的拒否のため loopback/impairment テストを拡張。 |
| NSC-30a | リレーのインセンティブ/評判フレームワーク設計（経済 + スキーマ） | Streaming WG, Economics WG | Q2 2026 | 🟡 計画中 | WG 横断ワークショップ（3/9）を立ち上げ、経済モデル文書の担当を割り当て。 | ワークショップ準備: `docs/source/project_tracker/nsc30a_relay_incentive_design.md`。インセンティブ白書、リレー証明 Norito スキーマ、テレメトリ項目、churn/failure モデルを含むシミュレーションハーネスを作成。 |
| NSC-30b | リレースコアリングと報酬パイプラインのエンドツーエンド実装 | Streaming WG, Torii Team | Q3 2026 | ⚪ NSC-30a 待ち | NSC-30a 設計のサインオフ待ち。Torii スキーマ変更提案のドラフトを準備。 | NSC-30a の成果に依存。設計確定後は同一ドキュメントと Torii バックログで追跡。 |
| NSC-37b | ZK チケット証明とホスト側検証の実装 | ZK Working Group, Core Host Team | Q3 2026 | ⚪ NSC-37a 待ち | `iroha_zkp_halo2` にプロトタイプ分岐を立て、必要なホストフックを列挙。 | 実装ノートは `nsc37a_zk_ticket_schema.md` の確定後に拡張。Core Host のロードマップと整合させる。 |
| NSC-42 | ブロック型コーデックの法務/特許レビュー完了 | Legal & Standards | Q2 2026 | 🟢 完了 | クローズ — 法務意見は記録済みで、ゲーティングは opt-in を維持。 | 署名済み記録は `docs/source/soranet/nsc-42-legal.md`。CABAC は `ENABLE_CABAC` + `[streaming.codec]` の背後、trellis は無効のまま、bundled rANS は `ENABLE_RANS_BUNDLES` を要求。法務姿勢は NOTICE/PATENTS/EXPORT に記録。 |
| NSC-55 | rANS 初期化テーブルの検証と特許姿勢の公開 | Codec Team | Q2 2026 | 🟢 完了 | クローズ — 決定論的テーブルと証拠バンドルがリポジトリ内に存在。 | 正準テーブル/ジェネレータ、bench、CSV/レポートパイプラインは `docs/source/project_tracker/nsc55_rans_validation_plan.md` に従って整備済み。CABAC オプション展開に向けた再現可能な rANS 姿勢を提供。 |
