---
lang: zh-hant
direction: ltr
source: docs/portal/docs/ministry/ai-moderation-runner.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 00cf1d37cf06d24b6eb7b2acba6b5c2ec3c3fae249b5cb6055384ca19ceaefac
source_last_modified: "2025-12-29T18:16:35.119787+00:00"
translation_last_reviewed: 2026-02-07
title: AI Moderation Runner Specification
summary: Deterministic moderation committee design for the Ministry of Information (MINFO-1) deliverable.
translator: machine-google-reviewed
---

# AI 審核跑步者規範

本規範滿足 **MINFO-1 — 建立 AI 的文檔部分
適度基線**。它定義了確定性執行合約
信息部審核服務，以便每個網關都可以相同地運行
上訴和透明度流程之前的管道（SFM-4/SFM-4b）。所有行為
除非明確標記為信息性的，否則此處描述的是規範性的。

## 1. 目標和範圍
- 提供可重複的審核委員會來評估網關內容
  （對象、清單、元數據、音頻）使用異構模型。
- 保證跨運算符的確定性執行：固定 opset、種子
  標記化、有限精度和版本化工件。
- 生成可供審核的工件：清單、記分卡、校准證據、
  以及適合在治理 DAG 中發布的透明度摘要。
- 表面遙測，以便 SRE 可以檢測漂移、誤報和停機時間
  無需收集原始用戶數據。

## 2. 確定性執行合約
- **運行時：** ONNX 運行時 1.19.x（CPU 後端）在禁用 AVX2 的情況下編譯並
  `--enable-extended-minimal-build` 以保持操作碼集固定。 CUDA/金屬
  生產環境中明確禁止運行時。
- **Opset：** `opset=17`。針對較新反對集的模型必須進行下轉換
  並在入學前進行驗證。
- **種子推導：** 每個評估都會從以下位置推導出一個 RNG 種子
  `run_nonce` 來的 `BLAKE3(content_digest || manifest_id || run_nonce)`
  來自治理批准的清單。種子提供所有隨機成分
  （集束搜索、退出切換），因此結果可以逐位重現。
- **線程：** 每個模型一名工人。並發由運行者協調
  協調器以避免共享狀態競爭條件。 BLAS 庫運行於
  單線程模式。
- **數字：** 禁止 FP16 累積。使用FP32中間體和夾具
  聚合之前輸出到小數點後四位。

## 3. 委員會組成
基線委員會包含三個模范家庭。治理可以添加
模型，但必須保持滿足最低法定人數。

|家庭|基線模型|目的|
|--------------------|----------------|---------|
|願景 | OpenCLIP ViT-H/14（安全微調）|檢測視覺違禁品、暴力、CSAM 指標。 |
|多式聯運 | LLaVA-1.6 34B 安全 |捕獲文本+圖像交互、上下文線索、騷擾。 |
|感性| pHash + aHash + NeuralHash-lite 集成 |快速接近重複檢測並召回已知不良材料。 |

每個模型條目指定：
- `model_id`（UUID）
- `artifact_digest`（OCI 圖像的 BLAKE3-256）
- `weights_digest`（ONNX 的 BLAKE3-256 或合併的安全張量 blob）
- `opset`（必須等於 `17`）
- `weight`（委員會權重，默認`1.0`）
- `critical_labels`（立即觸發 `Escalate` 的標籤集）
- `max_eval_ms`（確定性看門狗的護欄）

## 4. Norito 清單和結果

### 4.1 委員會清單
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

### 4.2 評估結果
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

跑步者必鬚髮出確定性的 `AiModerationDigestV1`（BLAKE3 超過
序列化結果）用於透明度日誌並將結果附加到審核中
分類帳時的判決不是 `pass`。

### 4.3 對抗性語料庫清單

網關操作員現在攝取一個枚舉感知的伴隨清單
從校準運行中得出的散列/嵌入“族”：

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

該模式位於 `crates/iroha_data_model/src/sorafs/moderation.rs` 中，並且是
通過 `AdversarialCorpusManifestV1::validate()` 驗證。清單允許
網關拒絕列表加載程序，用於填充阻止的 `perceptual_family` 條目
整個近乎重複的簇而不是單個字節。可運行的裝置
(`docs/examples/ai_moderation_perceptual_registry_202602.json`) 演示
預期的佈局並直接輸入到示例網關拒絕列表中。

## 5. 執行管道
1. 從治理 DAG 加載 `AiModerationManifestV1`。拒絕如果
   `runner_hash` 或 `runtime_version` 與部署的二進製文件不匹配。
2. 通過 OCI 摘要獲取模型工件，在加載之前驗證摘要。
3. 按內容類型構建評估批次；排序必須按
   `(content_digest, manifest_id)` 確保確定性聚合。
4. 使用派生種子執行每個模型。對於感知哈希，結合
   整體通過多數投票 -> 得分為 `[0,1]`。
5. 使用加權裁剪比率將分數匯總到 `combined_score` 中：
   ```
   combined = Σ_i weight_i * clamp(score_i / threshold_i, 0, 1) / Σ_i weight_i
   ```
6. 生成`ModerationVerdictV1`：
   - `escalate`（如果有 `critical_labels` 火災或 `combined ≥ thresholds.escalate`）。
   - `quarantine` 如果高於 `thresholds.quarantine` 但低於 `escalate`。
   - 否則為 `pass`。
7. 保留 `AiModerationResultV1` 並將下游進程排入隊列：
   - 檢疫服務（如果判決升級/檢疫）
   - 透明日誌寫入器 (`ModerationLedgerV1`)
   - 遙測出口商

## 6. 校準與評估
- **數據集：** 基線校準使用根據策略策劃的混合語料庫
  團隊批准。參考記錄在 `calibration_dataset` 中。
- **指標：** 計算 Brier 分數、預期校準誤差 (ECE) 和 AUROC
  每個模型和綜合判決。每月重新校準必須保持
  `Brier ≤ 0.18` 和 `ECE ≤ 0.05`。結果存儲在 SoraFS 報告樹中
  （例如，[2026 年 2 月校準](../sorafs/reports/ai-moderation-calibration-202602.md)）。
- **時間表：** 每月重新校準（第一個星期一）。緊急重新校準
  如果漂移引起火災，則允許。
- **過程：** 在校準集上運行確定性評估管道，
  重新生成 `thresholds`，更新清單，治理投票的階段更改。

## 7. 打包和部署
- 通過 `docker buildx bake -f docker/ai_moderation.hcl` 構建 OCI 映像。
- 圖片包括：
  - 鎖定的 Python env (`poetry.lock`) 或 Rust 二進製文件 `Cargo.lock`。
  - `models/` 目錄，包含哈希 ONNX 權重。
  - 入口點 `run_moderation.py`（或 Rust 等效項）公開 HTTP/gRPC API。
- 將工件發佈到 `registry.sora.net/ministry/ai-moderation/<model>@sha256:<digest>`。
- Runner 二進製文件作為 `sorafs_ai_runner` 板條箱的一部分發貨。構建管道
  在二進製文件中嵌入清單哈希（通過 `/v2/info` 公開）。

## 8. 遙測和可觀測性
- Prometheus 指標：
  - `moderation_requests_total{verdict}`
  - `moderation_model_score_bucket{model_id,label}`
  - `moderation_combined_score_bucket`
  - `moderation_inference_latency_seconds_bucket`
  - `moderation_runner_manifest_info{manifest_id, runtime_version}`
- 日誌：包含 `request_id`、`manifest_id`、`verdict` 的 JSON 行和摘要
  的存儲結果。原始分數在日誌中被編輯至小數點後兩位。
- 儀表板存儲在 `dashboards/grafana/ministry_moderation_overview.json` 中
  （與第一份校準報告一起發布）。
- 警報閾值：
  - 缺少攝取（`moderation_requests_total` 停滯 10 分鐘）。
  - 漂移檢測（與滾動 7 天平均值相比，平均模型得分增量 >20%）。
  - 誤報積壓（隔離隊列 > 50 個項目持續 > 30 分鐘）。

## 9. 治理和變更控制
- 清單需要雙重簽名：部委理事會成員 + 審核 SRE
  領先。簽名記錄在 `AiModerationManifestV1.governance_signature` 中。
- 更改遵循 `ModerationManifestChangeProposalV1` 至 Torii。哈希值
  進入治理DAG；部署被阻止，直到提案通過
  頒布。
- 運行程序二進製文件嵌入 `runner_hash`；如果哈希值不同，CI 將拒絕部署。
- 透明度：每週 `ModerationScorecardV1` 總結交易量、判決組合、
  以及上訴結果。發佈到 SoraParliament 門戶網站。

## 10. 安全與隱私
- 內容摘要使用 BLAKE3。原始有效負載永遠不會在隔離區之外持續存在。
- 進入隔離區需要即時批准；記錄所有訪問。
- 運行者沙箱不受信任的內容，強制執行 512 MiB 內存限制和 120 秒
  掛鍾守衛。
- 此處不應用差分隱私；網關依賴隔離+審核
  相反，工作流程。編輯策略遵循網關合規性計劃
  （`docs/source/sorafs_gateway_compliance_plan.md`；門戶副本待定）。

## 11. 校准出版物 (2026-02)
- **清單：** `docs/examples/ai_moderation_calibration_manifest_202602.json`
  記錄治理簽名的 `AiModerationManifestV1`（ID
  `c9bdf0b2-63a3-4a90-8d70-908d119c2c7e`)，數據集參考
  `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`，跑步者哈希
  `ea3c0fd0ff4bd4510e94c7c293b261f601cc0c4f9fbacd99b0401d233a7cdc20`，以及
  2026 年 2 月校準閾值（`quarantine = 0.42`、`escalate = 0.78`）。
- **記分板：** `docs/examples/ai_moderation_calibration_scorecard_202602.json`
  加上人類可讀的報告
  `[SoraFS Reports › AI Moderation Calibration 2026-02](../sorafs/reports/ai-moderation-calibration-202602.md)`
  捕獲每個模型的 Brier、ECE、AUROC 和判決組合。綜合指標
  達到目標（`Brier = 0.126`、`ECE = 0.034`）。
- **儀表板和警報：** `dashboards/grafana/ministry_moderation_overview.json`
  和 `dashboards/alerts/ministry_moderation_rules.yml`（回歸測試
  `dashboards/alerts/tests/ministry_moderation_rules.test.yml`) 提供
  推出所需的節制攝取/延遲/漂移監控故事。## 12. 再現性模式和驗證器 (MINFO-1b)
- Canonical Norito 類型現在與 SoraFS 模式的其餘部分一起存在
  `crates/iroha_data_model/src/sorafs/moderation.rs`。的
  `ModerationReproManifestV1`/`ModerationReproBodyV1` 結構捕獲
  清單 UUID、運行程序哈希、模型摘要、閾值集和種子材料。
  `ModerationReproManifestV1::validate` 強制模式版本
  (`MODERATION_REPRO_MANIFEST_VERSION_V1`)，確保每個艙單均載有
  至少一個模型和簽名者，並驗證每個 `SignatureOf<ModerationReproBodyV1>`
  在返回機器可讀的摘要之前。
- 操作員可以通過調用共享驗證器
  `sorafs_cli moderation validate-repro --manifest=PATH [--format=json|norito]`
  （在 `crates/sorafs_orchestrator/src/bin/sorafs_cli.rs` 中實現）。命令行界面
  接受以下發布的 JSON 工件
  `docs/examples/ai_moderation_calibration_manifest_202602.json` 或原始
  Norito 編碼並在清單旁邊打印模型/簽名計數
  驗證成功後的時間戳。
- 網關和自動化連接到同一個助手，因此再現性明顯
  當模式漂移、摘要丟失或
  簽名驗證失敗。
- 對抗性語料庫遵循相同的模式：
  `sorafs_cli moderation validate-corpus --manifest=PATH [--format=json|norito]`
  解析 `AdversarialCorpusManifestV1`，強制執行模式版本，並拒絕
  清單省略了家族、變體或指紋元數據。成功
  運行發出發佈時間時間戳、隊列標籤和家族/變體計數
  因此操作員可以在更新網關拒絕列表條目之前固定證據
  4.3 節中描述。

## 13. 公開跟進
- 2026 年 3 月 2 日之後的每月重新校準窗口繼續遵循
  第 6 條中的程序；發布 `ai-moderation-calibration-<YYYYMM>.md`
  以及 SoraFS 報告樹下更新的清單/記分卡包。
- MINFO-1b 和 MINFO-1c（可重複性清單驗證器加上對抗性
  語料庫註冊表）在路線圖中保持單獨跟踪。