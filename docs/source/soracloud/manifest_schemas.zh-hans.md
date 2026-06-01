<!-- Auto-generated stub for Chinese (Simplified) (zh-hans) translation. Replace this content with the full translation. -->

---
lang: zh-hans
direction: ltr
source: docs/source/soracloud/manifest_schemas.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0724ed92da90d8d78a7095b5fc75523ea5dd4a1c885059e28c1bbb8118d1f8c
source_last_modified: "2026-03-26T06:12:11.480497+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# SoraCloud V1 清单架构

此页面定义了 SoraCloud 的第一个确定性 Norito 架构
Iroha 3 上的部署：

- `SoraContainerManifestV1`
- `SoraServiceManifestV1`
- `SoraStateBindingV1`
- `SoraDeploymentBundleV1`
- `AgentApartmentManifestV1`
- `FheParamSetV1`
- `FheExecutionPolicyV1`
- `FheGovernanceBundleV1`
- `FheJobSpecV1`
- `DecryptionAuthorityPolicyV1`
- `DecryptionRequestV1`
- `CiphertextQuerySpecV1`
- `CiphertextQueryResponseV1`
- `SecretEnvelopeV1`
- `CiphertextStateRecordV1`

Rust 定义位于 `crates/iroha_data_model/src/soracloud.rs` 中。

上传模型私有运行时记录有意与
这些 SCR 部署清单。他们应该扩展 Soracloud 模型平面
并重用 `SecretEnvelopeV1` / `CiphertextStateRecordV1` 来获取加密字节
和密文本机状态，而不是被编码为新服务/容器
体现出来。参见 `uploaded_private_models.md`。

## 范围

这些清单专为 `IVM` + 自定义 Sora 容器运行时而设计
(SCR) 方向（无 WASM，运行时准入中无 Docker 依赖性）。- `SoraContainerManifestV1` 捕获可执行包标识、运行时类型、
  能力策略、资源、生命周期探测设置和显式
  required-config 导出到运行时环境或安装的修订版
  树。
- `SoraServiceManifestV1` 捕获部署意图：服务身份、
  引用的容器清单哈希/版本、路由、部署策略以及
  状态绑定。
- `SoraStateBindingV1` 捕获确定性状态写入范围和限制
  （命名空间前缀、可变性模式、加密模式、项目/总配额）。
- `SoraDeploymentBundleV1` 结合容器+服务清单和强制
  确定性准入检查（清单哈希链接、模式对齐和
  能力/绑定一致性）。
- `AgentApartmentManifestV1` 捕获持久代理运行时策略：
  工具上限、政策上限、支出限制、状态配额、网络出口和
  升级行为。
- `FheParamSetV1` 捕获治理管理的 FHE 参数集：
  确定性后端/方案标识符、模数配置文件、安全性/深度
  边界和生命周期高度 (`activation`/`deprecation`/`withdraw`)。
- `FheExecutionPolicyV1` 捕获确定性密文执行限制：
  承认的有效负载大小、输入/输出扇入、深度/旋转/引导上限、
  和规范舍入模式。
- `FheGovernanceBundleV1` 将参数集和策略结合起来以实现确定性
  录取验证。- `FheJobSpecV1` 捕获确定性密文作业准入/执行
  requests：操作类、有序输入承诺、输出键和有界
  与策略+参数集相关联的深度/旋转/引导需求。
- `DecryptionAuthorityPolicyV1` 捕获治理管理的披露政策：
  权限模式（客户端持有与阈值服务），批准者法定人数/成员，
  碎玻璃津贴、管辖区标记、同意证据要求、
  TTL 界限和规范审核标记。
- `DecryptionRequestV1` 捕获与政策相关的披露尝试：
  密文密钥参考（`binding_name` + `state_key` + 承诺），
  理由、管辖权标签、可选同意证据哈希、TTL、
  打破玻璃意图/原因，以及治理哈希链接。
- `CiphertextQuerySpecV1` 捕获确定性仅密文查询意图：
  服务/绑定范围、键前缀过滤器、有界结果限制、元数据
  投影级别和证明包含切换。
- `CiphertextQueryResponseV1` 捕获披露最小化的查询输出：
  面向摘要的密钥引用、密文元数据、可选的包含证明、
  和响应级截断/序列上下文。
- `SecretEnvelopeV1` 捕获加密的有效负载材料本身：
  加密模式、密钥标识符/版本、随机数、密文字节和
  诚信承诺。
- `CiphertextStateRecordV1` 捕获密文本机状态条目结合公共元数据（内容类型、策略标签、承诺、有效负载大小）
  带有 `SecretEnvelopeV1`。
- 用户上传的私有模型包应该建立在这些密文本机的基础上
  记录：
  加密的权重/配置/处理器块处于状态中，而模型注册表，
  权重谱系、编译配置文件、推理会话和检查点仍然存在
  一流的 Soracloud 记录。

## 版本控制

- `SORA_CONTAINER_MANIFEST_VERSION_V1 = 1`
- `SORA_SERVICE_MANIFEST_VERSION_V1 = 1`
- `SORA_STATE_BINDING_VERSION_V1 = 1`
- `SORA_DEPLOYMENT_BUNDLE_VERSION_V1 = 1`
- `AGENT_APARTMENT_MANIFEST_VERSION_V1 = 1`
- `FHE_PARAM_SET_VERSION_V1 = 1`
- `FHE_EXECUTION_POLICY_VERSION_V1 = 1`
- `FHE_GOVERNANCE_BUNDLE_VERSION_V1 = 1`
- `FHE_JOB_SPEC_VERSION_V1 = 1`
- `DECRYPTION_AUTHORITY_POLICY_VERSION_V1 = 1`
- `DECRYPTION_REQUEST_VERSION_V1 = 1`
- `CIPHERTEXT_QUERY_SPEC_VERSION_V1 = 1`
- `CIPHERTEXT_QUERY_RESPONSE_VERSION_V1 = 1`
- `CIPHERTEXT_QUERY_PROOF_VERSION_V1 = 1`
- `SECRET_ENVELOPE_VERSION_V1 = 1`
- `CIPHERTEXT_STATE_RECORD_VERSION_V1 = 1`

验证会拒绝不支持的版本
`SoraCloudManifestError::UnsupportedVersion`。

## 确定性验证规则（V1）- 集装箱清单：
  - `bundle_path` 和 `entrypoint` 必须为非空。
  - `healthcheck_path`（如果设置）必须以 `/` 开头。
  - `config_exports` 可能仅引用在中声明的配置
    `required_config_names`。
  - config-export env 目标必须使用规范的环境变量名称
    （`[A-Za-z_][A-Za-z0-9_]*`）。
  - 配置导出文件目标必须保持相对，使用 `/` 分隔符，并且
    不得包含空段、`.` 或 `..` 段。
  - 配置导出不得以相同的环境变量或相对文件路径为目标
    比一次。
- 服务清单：
  - `service_version` 必须非空。
  - `container.expected_schema_version` 必须与容器架构 v1 匹配。
  - `rollout.canary_percent` 必须是 `0..=100`。
  - `route.path_prefix`（如果设置）必须以 `/` 开头。
  - 状态绑定名称必须是唯一的。
- 状态绑定：
  - `key_prefix` 必须非空且以 `/` 开头。
  - `max_item_bytes <= max_total_bytes`。
  - `ConfidentialState` 绑定不能使用明文加密。
- 部署包：
  - `service.container.manifest_hash` 必须与规范编码匹配
    容器清单哈希。
  - `service.container.expected_schema_version` 必须与容器架构匹配。
  - 可变状态绑定需要 `container.capabilities.allow_state_writes=true`。
  - 公共路线需要 `container.lifecycle.healthcheck_path`。
- 代理公寓清单：
  - `container.expected_schema_version` 必须与容器架构 v1 匹配。
  - 工具功能名称必须非空且唯一。- 策略能力名称必须是唯一的。
  - 支出限制资产必须非空且唯一。
  - 每个支出限额为 `max_per_tx_nanos <= max_per_day_nanos`。
  - 允许列表网络策略必须包含唯一的非空主机。
- FHE参数设置：
  - `backend` 和 `ciphertext_modulus_bits` 必须为非空。
  - 每个密文模数位大小必须在 `2..=120` 范围内。
  - 密文模数链序必须是非递增的。
  - `plaintext_modulus_bits` 必须小于最大密文模数。
  - `slot_count <= polynomial_modulus_degree`。
  - `max_multiplicative_depth < ciphertext_modulus_bits.len()`。
  - 生命周期高度排序必须严格：
    `activation < deprecation < withdraw`（如果存在）。
  - 生命周期状态要求：
    - `Proposed` 不允许弃用/撤回高度。
    - `Active` 需要 `activation_height`。
    - `Deprecated` 需要 `activation_height` + `deprecation_height`。
    - `Withdrawn` 需要 `activation_height` + `withdraw_height`。
- FHE执行政策：
  - `max_plaintext_bytes <= max_ciphertext_bytes`。
  - `max_output_ciphertexts <= max_input_ciphertexts`。
  - 参数集绑定必须与 `(param_set, version)` 匹配。
  - `max_multiplication_depth` 不得超过参数设置深度。
  - 策略准入拒绝 `Proposed` 或 `Withdrawn` 参数集生命周期。
- FHE 治理包：
  - 验证策略+参数集兼容性作为一种确定性准入有效负载。
- FHE 职位规格：
  - `job_id` 和 `output_state_key` 必须非空（`output_state_key` 以 `/` 开头）。- 输入集必须非空，并且输入键必须是唯一的规范路径。
  - 操作特定的约束很严格（`Add`/`Multiply` 多输入，
    `RotateLeft`/`Bootstrap` 单输入，具有互斥的深度/旋转/引导旋钮）。
  - 与政策挂钩的录取强制执行：
    - 策略/参数标识符和版本匹配。
    - 输入计数/字节、深度、旋转和引导限制在策略上限内。
    - 确定性的预计输出字节符合策略密文限制。
- 解密权限策略：
  - `approver_ids` 必须非空、唯一且严格按字典顺序排序。
  - `ClientHeld` 模式仅需要一名审批者，`approver_quorum=1`，
    和 `allow_break_glass=false`。
  - `ThresholdService` 模式需要至少两个审批者并且
    `approver_quorum <= approver_ids.len()`。
  - `jurisdiction_tag` 必须非空且不得包含控制字符。
  - `audit_tag` 必须非空且不得包含控制字符。
- 解密请求：
  - `request_id`、`state_key` 和 `justification` 必须为非空
    （`state_key` 以 `/` 开头）。
  - `jurisdiction_tag` 必须非空且不得包含控制字符。
  - 当 `break_glass=true` 时需要 `break_glass_reason`，当 `break_glass=true` 时必须省略 `break_glass_reason`
    `break_glass=false`。
  - 策略关联准入强制策略名称相等，不请求 TTL超过 `policy.max_ttl_blocks`，管辖权标签平等，打破玻璃
    门控和同意证据要求
    `policy.require_consent_evidence=true` 用于不破碎玻璃的请求。
- 密文查询规范：
  - `state_key_prefix` 必须非空且以 `/` 开头。
  - `max_results` 是确定性有界的 (`<=256`)。
  - 元数据投影是显式的（`Minimal` 仅摘要与 `Standard` 键可见）。
- 密文查询响应：
  - `result_count` 必须等于序列化行计数。
  - `Minimal` 投影不得暴露 `state_key`； `Standard` 必须公开它。
  - 行绝不能显示明文加密模式。
  - 包含证明（如果存在）必须包含非空方案 ID 和
    `anchor_sequence >= event_sequence`。
- 秘密信封：
  - `key_id`、`nonce` 和 `ciphertext` 必须为非空。
  - 随机数长度有界（`<=256` 字节）。
  - 密文长度有界（`<=33554432` 字节）。
- 密文状态记录：
  - `state_key` 必须非空且以 `/` 开头。
  - 元数据内容类型必须非空；标签必须是唯一的非空字符串。
  - `metadata.payload_bytes` 必须等于 `secret.ciphertext.len()`。
  - `metadata.commitment` 必须等于 `secret.commitment`。

## 规范装置

规范 JSON 装置存储在：- `fixtures/soracloud/sora_container_manifest_v1.json`
- `fixtures/soracloud/sora_service_manifest_v1.json`
- `fixtures/soracloud/sora_state_binding_v1.json`
- `fixtures/soracloud/sora_deployment_bundle_v1.json`
- `fixtures/soracloud/agent_apartment_manifest_v1.json`
- `fixtures/soracloud/fhe_param_set_v1.json`
- `fixtures/soracloud/fhe_execution_policy_v1.json`
- `fixtures/soracloud/fhe_governance_bundle_v1.json`
- `fixtures/soracloud/fhe_job_spec_v1.json`
- `fixtures/soracloud/decryption_authority_policy_v1.json`
- `fixtures/soracloud/decryption_request_v1.json`
- `fixtures/soracloud/ciphertext_query_spec_v1.json`
- `fixtures/soracloud/ciphertext_query_response_v1.json`
- `fixtures/soracloud/secret_envelope_v1.json`
- `fixtures/soracloud/ciphertext_state_record_v1.json`

夹具/往返测试：

- `crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs`