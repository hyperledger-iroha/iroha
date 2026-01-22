<!-- Japanese translation of docs/fraud_playbook.md -->

---
lang: ja
direction: ltr
source: docs/fraud_playbook.md
status: complete
translator: manual
---

# 不正対策ガバナンス プレイブック

このドキュメントは、PSP の不正防止スタック向けに必要となる足場をまとめたものです。マイクロサービスや SDK が鋭意開発中であっても、分析・監査ワークフロー・フォールバック手順に関する期待値を明文化し、今後の実装が安全に台帳へ接続できるようにします。

## サービス概要

1. **API ゲートウェイ** – 同期的な `RiskQuery` ペイロードを受け取り、特徴量集約へフォワードしたうえで `FraudAssessment` 応答を台帳フローへ戻します。高可用構成（アクティブ／アクティブ）が必須であり、リクエストの偏りを避けるために決定的ハッシュを用いたリージョンペアを使用してください。
2. **特徴量集約** – スコアリング用の特徴量ベクトルを構成します。`FeatureInput` のハッシュのみを出力し、機微なペイロードはオフチェーンに留めます。各テナントで遅延ヒストグラム、キュー深度ゲージ、リプレイカウンタを公開する observability が必要です。
3. **リスクエンジン** – ルール／モデルを評価し、決定的な `FraudAssessment` を生成します。ルールの実行順序が安定していることを保証し、各アセスメント ID ごとに監査ログを取得してください。

## 分析とモデルプロモーション

- **異常検知**: テナントごとの判定率の逸脱を検知するストリーミングジョブを維持します。アラートはガバナンスダッシュボードへ送信し、四半期レビュー用にサマリを保存します。
- **グラフ分析**: リレーショナルエクスポートに対して夜間のグラフトラバーサルを実施し、共謀クラスターを特定します。得られた知見は裏付けとなる証跡参照とともに `GovernanceExport` としてガバナンスポータルに出力します。
- **フィードバック取り込み**: 手動審査の結果やチャージバックレポートを精査し、特徴量の差分に変換して学習データセットへ組み込みます。取り込み状況メトリクスを公開し、フィードが滞留していないかリスクチームが把握できるようにします。
- **モデルプロモーションパイプライン**: 候補モデルの評価（オフライン指標、カナリースコアリング、ロールバック準備）を自動化します。プロモーション時には署名付き `FraudAssessment` サンプルセットを生成し、`GovernanceExport` の `model_version` フィールドを更新してください。

## 監査ワークフロー

1. 最新の `GovernanceExport` を取得し、`policy_digest` がリスクチームから提供されたマニフェストと一致することを検証します。
2. サンプリング期間において、ルールごとの集計結果が台帳側での意思決定総数と整合することを確認します。
3. 異常検知およびグラフ分析レポートをレビューし、未解決の課題を洗い出します。エスカレーションと想定される対応責任者を記録してください。
4. レビューチェックリストへ署名し、Norito でエンコードした成果物をガバナンスポータルへ保管して再現性を確保します。

## フォールバック手順

- **エンジン障害**: リスクエンジンが 60 秒を超えて停止した場合、ゲートウェイはレビュー専用モードへ切り替え、全リクエストに `AssessmentDecision::Review` を返すとともにオペレータへアラートを発報します。
- **テレメトリ欠損**: メトリクスやトレースが 5 分間欠落した場合、自動モデルプロモーションを停止し、オンコールエンジニアへ通知してください。
- **モデル退行**: デプロイ後のフィードバックで不正損失の増加が示唆された場合、直前の署名済みモデルバンドルへロールバックし、是正アクションをロードマップへ追記します。

## データ共有契約

- 保存期間、暗号化、インシデント通報 SLA を管轄別にまとめた付録を管理してください。パートナーは `FraudAssessment` エクスポートを受領する前に付録へ署名する必要があります。
- 連携ごとのデータ最小化方針（例: アカウント識別子のハッシュ化、カード番号の桁落とし）を文書化します。
- 規制要件が変更された場合、もしくは年次で協定を更新してください。

## レッドチーム演習

- 演習は四半期ごとに実施します。次回は **2026-01-15** に予定されており、特徴量の汚染、リプレイ増幅、署名偽造を想定したシナリオを扱います。
- 発見事項を不正脅威モデルへ反映し、`roadmap.md` の「Fraud & Telemetry Governance Loop」ワークストリームにタスクを追加してください。

## API スキーマ

ゲートウェイは `crates/iroha_data_model::fraud` で実装された Norito 型と 1 対 1 で対応する具体的な JSON エンベロープを公開しています。

- **リスク照会** – `POST /v1/fraud/query` は `RiskQuery` スキーマを受け付けます:
  - `query_id`（`[u8; 32]`、hex エンコード）
  - `subject`（`AccountId`, `canonical IH58 literal; optional @<domain> hint`）
  - `operation`（`RiskOperation` に対応するタグ付き enum。JSONの `type` は enum バリアントに一致）
  - `related_asset`（`AssetId`, 任意）
  - `features`（`FeatureInput` に対応する `{ key: String, value_hash: hex32 }` の配列）
  - `issued_at_ms`（`u64`）
  - `context`（`RiskContext`。`tenant_id`、任意の `session_id`、任意の `reason` を保持）
- **リスク判定** – `POST /v1/fraud/assessment` は `FraudAssessment` ペイロードを受け付けます（ガバナンスエクスポートでも利用）:
  - `query_id`, `engine_id`, `risk_score_bps`, `confidence_bps`, `decision`（`AssessmentDecision` enum）、`rule_outcomes`（`{ rule_id, score_delta_bps, rationale? }` の配列）
  - `generated_at_ms`
  - `signature`（任意。Norito エンコード済みアセスメントを Base64 包装）
- **ガバナンスエクスポート** – `GET /v1/fraud/governance/export` は `governance` フィーチャーを有効にすると `GovernanceExport` 構造体を返し、アクティブなパラメータ、最新の施行、モデルバージョン、ポリシーダイジェスト、`DecisionAggregate` ヒストグラムを束ねます。

`crates/iroha_data_model/src/fraud/types.rs` のラウンドトリップテストにより、これらのスキーマが Norito コーデックとバイナリ互換であることが保証され、`integration_tests/tests/fraud_monitoring_requires_assessment_bands.rs` では intake／decision パイプライン全体のエンドツーエンドが検証されています。

## PSP SDK リファレンス

PSP 向けの連携例を以下の言語スタブで追跡しています。

- **Rust** – `integration_tests/tests/fraud_monitoring_requires_assessment_bands.rs` はワークスペースの `iroha` クライアントを用いて `RiskQuery` メタデータを生成し、受付可否を検証します。
- **TypeScript** – `docs/source/governance_api.md` は PSP デモダッシュボードで使用する軽量 Torii ゲートウェイの REST サーフェスを記述しており、スクリプト化されたクライアントはスモークドリル用の `scripts/ci/schedule_fraud_scoring.sh` に実装されています。
- **Swift & Kotlin** – 既存の SDK（`IrohaSwift` と `crates/iroha_cli/docs/multisig.md` で参照されるもの）が `fraud_assessment_*` フィールドを付与する Torii メタデータのフックを提供します。PSP 固有のヘルパーは `status.md` の「Fraud & Telemetry Governance Loop」マイルストーンで追跡され、これら SDK のトランザクションビルダを再利用しています。

これらのリファレンスはマイクロサービスゲートウェイと同期し、PSP 実装者が常に最新のスキーマと各言語向けのサンプルコードパスを参照できるよう維持されます。
