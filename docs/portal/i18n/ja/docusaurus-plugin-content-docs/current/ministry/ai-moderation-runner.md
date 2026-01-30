---
lang: ja
direction: ltr
source: docs/portal/docs/ministry/ai-moderation-runner.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: AIモデレーション・ランナー仕様
summary: 情報省 (MINFO-1) の成果物に向けた決定論的なモデレーション委員会設計。
---

# AIモデレーション・ランナー仕様

本仕様は **MINFO-1 — AIモデレーション基準の確立** のドキュメント要件を満たす。情報省のモデレーションサービスに対する決定論的な実行契約を定義し、各ゲートウェイがアピールおよび透明性フロー (SFM-4/SFM-4b) に先立って同一のパイプラインを実行できるようにする。ここに記載された挙動は、明示的に情報用と記載されない限り規範的である。

## 1. 目標と範囲
- ゲートウェイのコンテンツ（オブジェクト、マニフェスト、メタデータ、音声）を異種モデルで評価する再現可能なモデレーション委員会を提供する。
- オペレーター間の決定論的実行を保証する：固定オプセット、シード付きトークナイズ、制限付き精度、バージョン化されたアーティファクト。
- 監査に耐えるアーティファクト（マニフェスト、スコアカード、キャリブレーション証跡、透明性ダイジェスト）を生成し、ガバナンスDAGに公開できるようにする。
- 生データを収集せずにSREがドリフト、誤検知、停止を検知できるテレメトリを提供する。

## 2. 決定論的実行契約
- **Runtime:** AVX2 を無効化し `--enable-extended-minimal-build` でビルドした ONNX Runtime 1.19.x（CPU backend）。CUDA/Metal ランタイムは本番で明示的に禁止。
- **Opset:** `opset=17`。より新しい opset を対象とするモデルは、受け入れ前にダウングレードと検証を行う。
- **シード導出:** 各評価は `BLAKE3(content_digest || manifest_id || run_nonce)` から RNG シードを導出する。`run_nonce` はガバナンス承認済みマニフェスト由来。シードはビームサーチやドロップアウト切替などの確率的要素に供給され、ビット単位で再現可能にする。
- **スレッディング:** モデルごとに1ワーカー。ランナーのオーケストレーターが並行性を調整し、共有状態の競合を回避する。BLAS はシングルスレッド動作。
- **数値:** FP16 の累積は禁止。FP32 中間値を使い、集約前に出力を小数4桁にクランプする。

## 3. 委員会構成
ベースライン委員会は3つのモデルファミリで構成される。ガバナンスはモデルを追加できるが、最小クォーラムは維持されなければならない。

| ファミリ | ベースラインモデル | 目的 |
|--------|----------------|---------|
| Vision | OpenCLIP ViT-H/14（セーフティ微調整） | 視覚的な禁制品、暴力、CSAM 指標を検出。 |
| Multimodal | LLaVA-1.6 34B Safety | テキスト＋画像の相互作用、文脈手掛かり、ハラスメントを捕捉。 |
| Perceptual | pHash + aHash + NeuralHash-lite アンサンブル | 既知の悪性素材の近似重複検出とリコール。 |

各モデルエントリは以下を指定する：
- `model_id` (UUID)
- `artifact_digest` (OCI イメージの BLAKE3-256)
- `weights_digest` (ONNX または結合 safetensors blob の BLAKE3-256)
- `opset` (`17` であること)
- `weight` (委員会の重み、既定 `1.0`)
- `critical_labels` (`Escalate` を即時に発火させるラベル集合)
- `max_eval_ms` (決定論的 watchdog のガードレール)

## 4. Norito マニフェストと結果

### 4.1 委員会マニフェスト
```norito
struct AiModerationManifestV1 {
    manifest_id: Uuid,
    issued_at: Timestamp,
    runner_hash: Digest32,
    runtime_version: String,
    models: Vec<AiModerationModelV1>,
    calibration_dataset: DatasetReferenceV1,
    calibration_hash: Digest32,
    thresholds: AiModerationThresholdsV1,
    run_nonce: Digest32,
    governance_signature: Signature,
}

struct AiModerationModelV1 {
    model_id: Uuid,
    family: AiModerationFamilyV1, // vision | multimodal | perceptual | audio
    artifact_digest: Digest32,
    weights_digest: Digest32,
    opset: u8,
    weight: f32,
    critical_labels: Vec<String>,
    max_eval_ms: u32,
}
```

### 4.2 評価結果
```norito
struct AiModerationResultV1 {
    manifest_id: Uuid,
    request_id: Uuid,
    content_digest: Digest32,
    content_uri: String,
    content_class: ModerationContentClassV1, // manifest | chunk | metadata | audio
    model_scores: Vec<AiModerationModelScoreV1>,
    combined_score: f32,
    verdict: ModerationVerdictV1, // pass | quarantine | escalate
    executed_at: Timestamp,
    execution_ms: u32,
    runner_hash: Digest32,
    annotations: Option<Vec<String>>,
}

struct AiModerationModelScoreV1 {
    model_id: Uuid,
    score: f32,
    threshold: f32,
    confidence: f32,
    label: Option<String>,
}
```

ランナーは、透明性ログ用に決定論的な `AiModerationDigestV1`（シリアライズ結果の BLAKE3）を生成し、判定が `pass` でない場合はモデレーション台帳に結果を追記しなければならない。

### 4.3 敵対コーパスマニフェスト

ゲートウェイ運用者は、キャリブレーション実行から導出された知覚ハッシュ／埋め込みの「ファミリ」を列挙する付随マニフェストを取り込む：

```norito
struct AdversarialCorpusManifestV1 {
    schema_version: u16,                // must equal 1
    issued_at_unix: u64,
    cohort_label: Option<String>,       // e.g. "2026-Q1"
    families: Vec<AdversarialPerceptualFamilyV1>,
}

struct AdversarialPerceptualFamilyV1 {
    family_id: Uuid,
    description: String,
    variants: Vec<AdversarialPerceptualVariantV1>,
}

struct AdversarialPerceptualVariantV1 {
    variant_id: Uuid,
    attack_vector: String,
    reference_cid_b64: Option<String>,
    perceptual_hash: Option<Digest32>,   // Goldilocks hash, BLAKE3 domain separated
    hamming_radius: u8,                  // ≤ 32
    embedding_digest: Option<Digest32>,  // BLAKE3 of quantised embedding vector
    notes: Option<String>,
}
```

スキーマは `crates/iroha_data_model/src/sorafs/moderation.rs` にあり、`AdversarialCorpusManifestV1::validate()` により検証される。マニフェストにより、gateway の denylist ローダは個別バイトではなく近似重複クラスター全体をブロックする `perceptual_family` エントリを構成できる。実行可能なフィクスチャ（`docs/examples/ai_moderation_perceptual_registry_202602.json`）が期待レイアウトを示し、サンプル denylist に直接供給される。

## 5. 実行パイプライン
1. ガバナンスDAGから `AiModerationManifestV1` を読み込む。`runner_hash` または `runtime_version` が配備済みバイナリと一致しない場合は拒否。
2. OCI digest 経由でモデルアーティファクトを取得し、ロード前に digest を検証する。
3. コンテンツ種別ごとに評価バッチを構成し、`(content_digest, manifest_id)` の順序で並べて決定論的な集約を保証する。
4. 導出したシードで各モデルを実行する。知覚ハッシュは多数決でアンサンブルを統合し、`[0,1]` のスコアにする。
5. 重み付きクリップ比で `combined_score` を集計：
   ```
   combined = Σ_i weight_i * clamp(score_i / threshold_i, 0, 1) / Σ_i weight_i
   ```
6. `ModerationVerdictV1` を生成：
   - いずれかの `critical_labels` が発火するか `combined ≥ thresholds.escalate` の場合 `escalate`
   - `thresholds.quarantine` を超え `escalate` 未満なら `quarantine`
   - それ以外は `pass`
7. `AiModerationResultV1` を永続化し、下流プロセスをキューに投入：
   - 隔離サービス（判定が escalate/quarantine の場合）
   - 透明性ログ書き込み (`ModerationLedgerV1`)
   - テレメトリ・エクスポータ

## 6. キャリブレーションと評価
- **Datasets:** ベースラインのキャリブレーションはポリシーチーム承認の混合コーパスを使用。参照は `calibration_dataset` に記録。
- **Metrics:** 各モデルと総合判定について Brier、Expected Calibration Error (ECE)、AUROC を算出。月次再キャリブレーションは `Brier ≤ 0.18` および `ECE ≤ 0.05` を満たさなければならない。結果は SoraFS レポートツリーに保存（例: [2026年2月キャリブレーション](../sorafs/reports/ai-moderation-calibration-202602.md)）。
- **Schedule:** 月次再キャリブレーション（第1月曜）。ドリフト警告が出た場合は緊急再キャリブレーションを許可。
- **Process:** キャリブレーションセットで決定論的評価パイプラインを実行し、`thresholds` を再生成、マニフェストを更新し、ガバナンス投票へ。

## 7. パッケージングとデプロイ
- `docker buildx bake -f docker/ai_moderation.hcl` で OCI イメージをビルド。
- イメージには以下を含む：
  - 固定済み Python 環境 (`poetry.lock`) または Rust バイナリ `Cargo.lock`
  - ハッシュ付き ONNX 重みを置く `models/` ディレクトリ
  - HTTP/gRPC API を公開する `run_moderation.py`（または Rust 版）
- `registry.sora.net/ministry/ai-moderation/<model>@sha256:<digest>` にアーティファクトを公開。
- ランナーバイナリは `sorafs_ai_runner` crate の一部として出荷。ビルドパイプラインがマニフェストハッシュをバイナリへ埋め込み（`/v1/info` で公開）。

## 8. テレメトリと可観測性
- Prometheus メトリクス:
  - `moderation_requests_total{verdict}`
  - `moderation_model_score_bucket{model_id,label}`
  - `moderation_combined_score_bucket`
  - `moderation_inference_latency_seconds_bucket`
  - `moderation_runner_manifest_info{manifest_id, runtime_version}`
- ログ: `request_id`, `manifest_id`, `verdict`, 保存結果の digest を含む JSON lines。生スコアはログ内で小数2桁に丸めてマスク。
- ダッシュボードは `dashboards/grafana/ministry_moderation_overview.json` に保存（最初のキャリブレーション報告と同時に公開）。
- アラート閾値:
  - 取り込み停止（`moderation_requests_total` が10分間停止）
  - ドリフト検出（平均モデルスコアの差分が7日移動平均比で>20%）
  - 偽陽性バックログ（隔離キューが30分超で50件以上）

## 9. ガバナンスと変更管理
- マニフェストは二重署名が必須：省の評議会メンバー + モデレーションSREリード。署名は `AiModerationManifestV1.governance_signature` に記録。
- 変更は Torii の `ModerationManifestChangeProposalV1` を経由。ハッシュはガバナンスDAGに登録され、提案が成立するまでデプロイはブロック。
- ランナーバイナリは `runner_hash` を埋め込み、CI はハッシュ不一致でデプロイを拒否。
- 透明性: 週次 `ModerationScorecardV1` がボリューム、判定ミックス、アピール結果を要約し、Sora議会ポータルで公開。

## 10. セキュリティとプライバシー
- コンテンツの digest は BLAKE3。生ペイロードは隔離外に保存されない。
- 隔離アクセスは Just‑In‑Time 承認が必要で、全アクセスをログ化。
- ランナーは不信頼コンテンツをサンドボックス化し、512 MiB のメモリ制限と 120 秒の wall‑clock ガードを適用。
- 差分プライバシーはここでは適用しない。ゲートウェイは隔離 + 監査フローに依存する。レダクション方針は gateway コンプライアンス計画 (`docs/source/sorafs_gateway_compliance_plan.md`、ポータル版は未反映) に従う。

## 11. キャリブレーション公開 (2026-02)
- **マニフェスト:** `docs/examples/ai_moderation_calibration_manifest_202602.json`
  はガバナンス署名済み `AiModerationManifestV1`（ID
  `c9bdf0b2-63a3-4a90-8d70-908d119c2c7e`）、データセット参照
  `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`、ランナーハッシュ
  `ea3c0fd0ff4bd4510e94c7c293b261f601cc0c4f9fbacd99b0401d233a7cdc20`、および
  2026‑02 の閾値（`quarantine = 0.42`, `escalate = 0.78`）を記録。
- **スコアボード:** `docs/examples/ai_moderation_calibration_scorecard_202602.json`
  と読みやすいレポート
  `[SoraFS Reports › AI Moderation Calibration 2026-02](../sorafs/reports/ai-moderation-calibration-202602.md)`
  が各モデルの Brier、ECE、AUROC、判定ミックスを記録。総合指標は目標を満たした（`Brier = 0.126`, `ECE = 0.034`）。
- **ダッシュボード & アラート:** `dashboards/grafana/ministry_moderation_overview.json`
  と `dashboards/alerts/ministry_moderation_rules.yml`（回帰テストは
  `dashboards/alerts/tests/ministry_moderation_rules.test.yml`）が、ロールアウトに必要な取り込み/遅延/ドリフト監視を提供。

## 12. 再現性スキーマとバリデータ (MINFO-1b)
- 正式な Norito 型は SoraFS スキーマと同じ場所の
  `crates/iroha_data_model/src/sorafs/moderation.rs` に存在。`ModerationReproManifestV1`/`ModerationReproBodyV1` はマニフェストUUID、ランナーハッシュ、モデル digest、閾値セット、シード材料を保持。
  `ModerationReproManifestV1::validate` はスキーマ版
  (`MODERATION_REPRO_MANIFEST_VERSION_V1`) を強制し、各マニフェストに少なくとも1モデルと署名者があることを保証し、`SignatureOf<ModerationReproBodyV1>` を検証して機械可読な要約を返す。
- オペレーターは
  `sorafs_cli moderation validate-repro --manifest=PATH [--format=json|norito]`
  で共有バリデータを実行できる（`crates/sorafs_orchestrator/src/bin/sorafs_cli.rs` 実装）。CLI は
  `docs/examples/ai_moderation_calibration_manifest_202602.json` の JSON アーティファクトまたは生 Norito を受け入れ、検証成功時にモデル/署名数とマニフェスト時刻を出力。
- ゲートウェイと自動化は同じヘルパを利用し、スキーマのドリフト、digest欠落、署名検証失敗時に再現性マニフェストを決定論的に拒否できる。
- adversarial corpus のバンドルも同様：
  `sorafs_cli moderation validate-corpus --manifest=PATH [--format=json|norito]`
  が `AdversarialCorpusManifestV1` を解析し、スキーマ版を強制し、ファミリやバリアント、指紋メタデータが欠けたマニフェストを拒否する。成功時は発行時刻、コホートラベル、ファミリ/バリアント数を出力し、セクション 4.3 の gateway denylist 更新前に証拠を固定できる。

## 13. オープンフォローアップ
- 2026-03-02 以降の月次再キャリブレーションはセクション6の手順に従い、`ai-moderation-calibration-<YYYYMM>.md` を更新マニフェスト/スコアカードと共に SoraFS レポートツリーに公開する。
- MINFO-1b と MINFO-1c（再現性マニフェストのバリデータおよび adversarial corpus レジストリ）はロードマップで別途追跡する。
