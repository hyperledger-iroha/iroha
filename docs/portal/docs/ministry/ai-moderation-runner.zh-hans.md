---
lang: zh-hans
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

# AI 审核跑步者规范

本规范满足 **MINFO-1 — 建立 AI 的文档部分
适度基线**。它定义了确定性执行合约
信息部审核服务，以便每个网关都可以相同地运行
上诉和透明度流程之前的管道（SFM-4/SFM-4b）。所有行为
除非明确标记为信息性的，否则此处描述的是规范性的。

## 1. 目标和范围
- 提供可重复的审核委员会来评估网关内容
  （对象、清单、元数据、音频）使用异构模型。
- 保证跨运算符的确定性执行：固定 opset、种子
  标记化、有限精度和版本化工件。
- 生成可供审核的工件：清单、记分卡、校准证据、
  以及适合在治理 DAG 中发布的透明度摘要。
- 表面遥测，以便 SRE 可以检测漂移、误报和停机时间
  无需收集原始用户数据。

## 2. 确定性执行合约
- **运行时：** ONNX 运行时 1.19.x（CPU 后端）在禁用 AVX2 的情况下编译并
  `--enable-extended-minimal-build` 以保持操作码集固定。 CUDA/金属
  生产环境中明确禁止运行时。
- **Opset：** `opset=17`。针对较新反对集的模型必须进行下转换
  并在入学前进行验证。
- **种子推导：** 每个评估都会从以下位置推导出一个 RNG 种子
  `run_nonce` 来的 `BLAKE3(content_digest || manifest_id || run_nonce)`
  来自治理批准的清单。种子提供所有随机成分
  （集束搜索、退出切换），因此结果可以逐位重现。
- **线程：** 每个模型一名工人。并发由运行者协调
  协调器以避免共享状态竞争条件。 BLAS 库运行于
  单线程模式。
- **数字：** 禁止 FP16 累积。使用FP32中间体和夹具
  聚合之前输出到小数点后四位。

## 3. 委员会组成
基线委员会包含三个模范家庭。治理可以添加
模型，但必须保持满足最低法定人数。

|家庭|基线模型|目的|
|--------------------|----------------|---------|
|愿景 | OpenCLIP ViT-H/14（安全微调）|检测视觉违禁品、暴力、CSAM 指标。 |
|多式联运 | LLaVA-1.6 34B 安全 |捕获文本+图像交互、上下文线索、骚扰。 |
|感性 | pHash + aHash + NeuralHash-lite 集成 |快速接近重复检测并召回已知不良材料。 |

每个模型条目指定：
- `model_id`（UUID）
- `artifact_digest`（OCI 图像的 BLAKE3-256）
- `weights_digest`（ONNX 的 BLAKE3-256 或合并的安全张量 blob）
- `opset`（必须等于 `17`）
- `weight`（委员会权重，默认`1.0`）
- `critical_labels`（立即触发 `Escalate` 的标签集）
- `max_eval_ms`（确定性看门狗的护栏）

## 4. Norito 清单和结果

### 4.1 委员会清单
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

### 4.2 评估结果
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

跑步者必须发出确定性的 `AiModerationDigestV1`（BLAKE3 超过
序列化结果）用于透明度日志并将结果附加到审核中
分类帐时的判决不是 `pass`。

### 4.3 对抗性语料库清单

网关操作员现在摄取一个枚举感知的伴随清单
从校准运行中得出的散列/嵌入“族”：

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

该模式位于 `crates/iroha_data_model/src/sorafs/moderation.rs` 中，并且是
通过 `AdversarialCorpusManifestV1::validate()` 验证。清单允许
网关拒绝列表加载程序，用于填充阻止的 `perceptual_family` 条目
整个近乎重复的簇而不是单个字节。可运行的装置
(`docs/examples/ai_moderation_perceptual_registry_202602.json`) 演示
预期的布局并直接输入到示例网关拒绝列表中。

## 5. 执行管道
1. 从治理 DAG 加载 `AiModerationManifestV1`。拒绝如果
   `runner_hash` 或 `runtime_version` 与部署的二进制文件不匹配。
2. 通过 OCI 摘要获取模型工件，在加载之前验证摘要。
3. 按内容类型构建评估批次；排序必须按
   `(content_digest, manifest_id)` 确保确定性聚合。
4. 使用派生种子执行每个模型。对于感知哈希，结合
   整体通过多数投票 -> 得分为 `[0,1]`。
5. 使用加权裁剪比率将分数汇总到 `combined_score` 中：
   ```
   combined = Σ_i weight_i * clamp(score_i / threshold_i, 0, 1) / Σ_i weight_i
   ```
6. 生成`ModerationVerdictV1`：
   - `escalate`（如果有 `critical_labels` 火灾或 `combined ≥ thresholds.escalate`）。
   - `quarantine` 如果高于 `thresholds.quarantine` 但低于 `escalate`。
   - 否则为 `pass`。
7. 保留 `AiModerationResultV1` 并将下游进程排入队列：
   - 检疫服务（如果判决升级/检疫）
   - 透明日志写入器 (`ModerationLedgerV1`)
   - 遥测出口商

## 6. 校准与评估
- **数据集：** 基线校准使用根据策略策划的混合语料库
  团队批准。参考记录在 `calibration_dataset` 中。
- **指标：** 计算 Brier 分数、预期校准误差 (ECE) 和 AUROC
  每个模型和综合判决。每月重新校准必须保持
  `Brier ≤ 0.18` 和 `ECE ≤ 0.05`。结果存储在 SoraFS 报告树中
  （例如，[2026 年 2 月校准](../sorafs/reports/ai-moderation-calibration-202602.md)）。
- **时间表：** 每月重新校准（第一个星期一）。紧急重新校准
  如果漂移引起火灾，则允许。
- **过程：** 在校准集上运行确定性评估管道，
  重新生成 `thresholds`，更新清单，治理投票的阶段更改。

## 7. 打包和部署
- 通过 `docker buildx bake -f docker/ai_moderation.hcl` 构建 OCI 映像。
- 图片包括：
  - 锁定的 Python env (`poetry.lock`) 或 Rust 二进制文件 `Cargo.lock`。
  - `models/` 目录，包含哈希 ONNX 权重。
  - 入口点 `run_moderation.py`（或 Rust 等效项）公开 HTTP/gRPC API。
- 将工件发布到 `registry.sora.net/ministry/ai-moderation/<model>@sha256:<digest>`。
- Runner 二进制文件作为 `sorafs_ai_runner` 板条箱的一部分发货。构建管道
  在二进制文件中嵌入清单哈希（通过 `/v1/info` 公开）。

## 8. 遥测和可观测性
- Prometheus 指标：
  - `moderation_requests_total{verdict}`
  - `moderation_model_score_bucket{model_id,label}`
  - `moderation_combined_score_bucket`
  - `moderation_inference_latency_seconds_bucket`
  - `moderation_runner_manifest_info{manifest_id, runtime_version}`
- 日志：包含 `request_id`、`manifest_id`、`verdict` 的 JSON 行和摘要
  的存储结果。原始分数在日志中被编辑至小数点后两位。
- 仪表板存储在 `dashboards/grafana/ministry_moderation_overview.json` 中
  （与第一份校准报告一起发布）。
- 警报阈值：
  - 缺少摄取（`moderation_requests_total` 停滞 10 分钟）。
  - 漂移检测（与滚动 7 天平均值相比，平均模型得分增量 >20%）。
  - 误报积压（隔离队列 > 50 个项目持续 > 30 分钟）。

## 9. 治理和变更控制
- 清单需要双重签名：部委理事会成员 + 审核 SRE
  领先。签名记录在 `AiModerationManifestV1.governance_signature` 中。
- 更改遵循 `ModerationManifestChangeProposalV1` 至 Torii。哈希值
  进入治理DAG；部署被阻止，直到提案通过
  颁布。
- 运行程序二进制文件嵌入 `runner_hash`；如果哈希值不同，CI 将拒绝部署。
- 透明度：每周 `ModerationScorecardV1` 总结交易量、判决组合、
  以及上诉结果。发布到 SoraParliament 门户网站。

## 10. 安全与隐私
- 内容摘要使用 BLAKE3。原始有效负载永远不会在隔离区之外持续存在。
- 进入隔离区需要即时批准；记录所有访问。
- 运行者沙箱不受信任的内容，强制执行 512 MiB 内存限制和 120 秒
  挂钟守卫。
- 此处不应用差分隐私；网关依赖隔离+审核
  相反，工作流程。编辑策略遵循网关合规性计划
  （`docs/source/sorafs_gateway_compliance_plan.md`；门户副本待定）。

## 11. 校准出版物 (2026-02)
- **清单：** `docs/examples/ai_moderation_calibration_manifest_202602.json`
  记录治理签名的 `AiModerationManifestV1`（ID
  `c9bdf0b2-63a3-4a90-8d70-908d119c2c7e`)，数据集参考
  `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`，跑步者哈希
  `ea3c0fd0ff4bd4510e94c7c293b261f601cc0c4f9fbacd99b0401d233a7cdc20`，以及
  2026 年 2 月校准阈值（`quarantine = 0.42`、`escalate = 0.78`）。
- **记分板：** `docs/examples/ai_moderation_calibration_scorecard_202602.json`
  加上人类可读的报告
  `[SoraFS Reports › AI Moderation Calibration 2026-02](../sorafs/reports/ai-moderation-calibration-202602.md)`
  捕获每个模型的 Brier、ECE、AUROC 和判决组合。综合指标
  达到了目标（`Brier = 0.126`、`ECE = 0.034`）。
- **仪表板和警报：** `dashboards/grafana/ministry_moderation_overview.json`
  和 `dashboards/alerts/ministry_moderation_rules.yml`（回归测试
  `dashboards/alerts/tests/ministry_moderation_rules.test.yml`) 提供
  推出所需的节制摄取/延迟/漂移监控故事。## 12. 再现性模式和验证器 (MINFO-1b)
- Canonical Norito 类型现在与 SoraFS 模式的其余部分一起存在
  `crates/iroha_data_model/src/sorafs/moderation.rs`。的
  `ModerationReproManifestV1`/`ModerationReproBodyV1` 结构捕获
  清单 UUID、运行程序哈希、模型摘要、阈值集和种子材料。
  `ModerationReproManifestV1::validate` 强制模式版本
  (`MODERATION_REPRO_MANIFEST_VERSION_V1`)，确保每个舱单均载有
  至少一个模型和签名者，并验证每个 `SignatureOf<ModerationReproBodyV1>`
  在返回机器可读的摘要之前。
- 操作员可以通过调用共享验证器
  `sorafs_cli moderation validate-repro --manifest=PATH [--format=json|norito]`
  （在 `crates/sorafs_orchestrator/src/bin/sorafs_cli.rs` 中实现）。命令行界面
  接受以下发布的 JSON 工件
  `docs/examples/ai_moderation_calibration_manifest_202602.json` 或原始
  Norito 编码并在清单旁边打印模型/签名计数
  验证成功后的时间戳。
- 网关和自动化连接到同一个助手，因此再现性明显
  当模式漂移、摘要丢失或
  签名验证失败。
- 对抗性语料库遵循相同的模式：
  `sorafs_cli moderation validate-corpus --manifest=PATH [--format=json|norito]`
  解析 `AdversarialCorpusManifestV1`，强制执行模式版本，并拒绝
  清单省略了家族、变体或指纹元数据。成功
  运行发出发布时间时间戳、队列标签和家族/变体计数
  因此操作员可以在更新网关拒绝列表条目之前固定证据
  4.3 节中描述。

## 13. 公开跟进
- 2026 年 3 月 2 日之后的每月重新校准窗口继续遵循
  第 6 条中的程序；发布 `ai-moderation-calibration-<YYYYMM>.md`
  以及 SoraFS 报告树下更新的清单/记分卡包。
- MINFO-1b 和 MINFO-1c（可重复性清单验证器加上对抗性
  语料库注册表）在路线图中保持单独跟踪。