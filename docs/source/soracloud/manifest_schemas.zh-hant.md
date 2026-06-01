<!-- Auto-generated stub for Chinese (Traditional) (zh-hant) translation. Replace this content with the full translation. -->

---
lang: zh-hant
direction: ltr
source: docs/source/soracloud/manifest_schemas.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0724ed92da90d8d78a7095b5fc75523ea5dd4a1c885059e28c1bbb8118d1f8c
source_last_modified: "2026-03-26T06:12:11.480497+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# SoraCloud V1 清單架構

此頁面定義了 SoraCloud 的第一個確定性 Norito 架構
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

Rust 定義位於 `crates/iroha_data_model/src/soracloud.rs` 中。

上傳模型私有運行時記錄有意與
這些 SCR 部署清單。他們應該擴展 Soracloud 模型平面
並重複使用 `SecretEnvelopeV1` / `CiphertextStateRecordV1` 來取得加密位元組
和密文本機狀態，而不是被編碼為新服務/容器
體現出來。參見 `uploaded_private_models.md`。

## 範圍

這些清單專為 `IVM` + 自訂 Sora 容器運行時而設計
(SCR) 方向（無 WASM，運轉時准入中無 Docker 依賴性）。- `SoraContainerManifestV1` 擷取可執行套件標識、執行時間類型、
  能力策略、資源、生命週期探測設定與顯式
  required-config 匯出到執行時間環境或安裝的修訂版
  樹。
- `SoraServiceManifestV1` 捕捉部署意圖：服務身分、
  引用的容器清單雜湊/版本、路由、部署策略以及
  狀態綁定。
- `SoraStateBindingV1` 捕獲確定性狀態寫入範圍和限制
  （命名空間前綴、可變性模式、加密模式、項目/總配額）。
- `SoraDeploymentBundleV1` 結合容器+服務清單和強制
  確定性准入檢查（清單哈希連結、模式對齊和
  能力/綁定一致性）。
- `AgentApartmentManifestV1` 捕獲持久代理運行時策略：
  工具上限、政策上限、支出限制、狀態配額、網路出口和
  升級行為。
- `FheParamSetV1` 擷取治理管理的 FHE 參數集：
  確定性後端/方案標識符、模數設定檔、安全性/深度
  邊界和生命週期高度 (`activation`/`deprecation`/`withdraw`)。
- `FheExecutionPolicyV1` 捕獲確定性密文執行限制：
  承認的有效負載大小、輸入/輸出扇入、深度/旋轉/引導上限、
  和規範舍入模式。
- `FheGovernanceBundleV1` 將參數集和策略結合起來以實現確定性
  錄取驗證。- `FheJobSpecV1` 捕獲確定性密文作業准入/執行
  requests：操作類別、有序輸入承諾、輸出鍵和有界
  與策略+參數集相關聯的深度/旋轉/引導需求。
- `DecryptionAuthorityPolicyV1` 捕捉治理管理的揭露政策：
  權限模式（客戶端持有與閾值服務），批准者法定人數/成員，
  碎玻璃津貼、管轄區標記、同意證據要求、
  TTL 界限和規範審核標記。
- `DecryptionRequestV1` 捕獲與政策相關的揭露嘗試：
  密文金鑰參考（`binding_name` + `state_key` + 承諾），
  理由、管轄權標籤、可選同意證據雜湊、TTL、
  打破玻璃意圖/原因，以及治理哈希鏈結。
- `CiphertextQuerySpecV1` 捕獲確定性僅密文查詢意圖：
  服務/綁定範圍、鍵前綴過濾器、有界結果限制、元數據
  投影等級和證明包含切換。
- `CiphertextQueryResponseV1` 擷取揭露最小化的查詢輸出：
  以摘要為主的金鑰引用、密文元資料、可選的包含證明、
  和響應級截斷/序列上下文。
- `SecretEnvelopeV1` 擷取加密的有效負載材料本身：
  加密模式、金鑰標識符/版本、隨機數、密文字節和
  誠信承諾。
- `CiphertextStateRecordV1` 擷取密文本機狀態條目結合公共元資料（內容類型、策略標籤、承諾、有效負載大小）
  附 `SecretEnvelopeV1`。
- 使用者上傳的私有模型包應該建立在這些密文本機的基礎上
  記錄：
  加密的權重/配置/處理器區塊處於狀態中，而模型註冊表，
  權重譜系、編譯設定檔、推理會話和檢查點仍然存在
  一流的 Soracloud 記錄。

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

驗證會拒絕不支援的版本
`SoraCloudManifestError::UnsupportedVersion`。

## 確定性驗證規則（V1）- 貨櫃清單：
  - `bundle_path` 和 `entrypoint` 必須為非空。
  - `healthcheck_path`（如果設定）必須以 `/` 開頭。
  - `config_exports` 可能僅引用在中聲明的配置
    `required_config_names`。
  - config-export env 目標必須使用規範的環境變數名稱
    （`[A-Za-z_][A-Za-z0-9_]*`）。
  - 配置匯出檔案目標必須保持相對，使用 `/` 分隔符，並且
    不得包含空段、`.` 或 `..` 段。
  - 配置匯出不得以相同的環境變數或相對檔案路徑為目標
    比一次。
- 服務清單：
  - `service_version` 必須非空。
  - `container.expected_schema_version` 必須與容器架構 v1 相符。
  - `rollout.canary_percent` 必須是 `0..=100`。
  - `route.path_prefix`（如果設定）必須以 `/` 開頭。
  - 狀態綁定名稱必須是唯一的。
- 狀態綁定：
  - `key_prefix` 必須非空且以 `/` 開頭。
  - `max_item_bytes <= max_total_bytes`。
  - `ConfidentialState` 綁定不能使用明文加密。
- 部署套件：
  - `service.container.manifest_hash` 必須與規範編碼匹配
    容器清單哈希。
  - `service.container.expected_schema_version` 必須與容器架構相符。
  - 可變狀態綁定需要 `container.capabilities.allow_state_writes=true`。
  - 公共路線需要 `container.lifecycle.healthcheck_path`。
- 代理公寓清單：
  - `container.expected_schema_version` 必須與容器架構 v1 相符。
  - 工具功能名稱必須非空且唯一。- 策略能力名稱必須是唯一的。
  - 支出限制資產必須非空且唯一。
  - 每個支出限額為 `max_per_tx_nanos <= max_per_day_nanos`。
  - 允許清單網路原則必須包含唯一的非空主機。
- FHE參數設定：
  - `backend` 和 `ciphertext_modulus_bits` 必須為非空。
  - 每個密文模數位大小必須在 `2..=120` 範圍內。
  - 密文模數鏈序必須是非遞增的。
  - `plaintext_modulus_bits` 必須小於最大密文模。
  - `slot_count <= polynomial_modulus_degree`。
  - `max_multiplicative_depth < ciphertext_modulus_bits.len()`。
  - 生命週期高度排序必須嚴格：
    `activation < deprecation < withdraw`（如果存在）。
  - 生命週期狀態需求：
    - `Proposed` 不允許棄用/撤回高度。
    - `Active` 需要 `activation_height`。
    - `Deprecated` 需要 `activation_height` + `deprecation_height`。
    - `Withdrawn` 需要 `activation_height` + `withdraw_height`。
- FHE執行政策：
  - `max_plaintext_bytes <= max_ciphertext_bytes`。
  - `max_output_ciphertexts <= max_input_ciphertexts`。
  - 參數集綁定必須與 `(param_set, version)` 相符。
  - `max_multiplication_depth` 不得超過參數設定深度。
  - 策略准入拒絕 `Proposed` 或 `Withdrawn` 參數集生命週期。
- FHE 治理包：
  - 驗證策略+參數集相容性作為一種確定性准入有效負載。
- FHE 職位規格：
  - `job_id` 和 `output_state_key` 必須非空（`output_state_key` 以 `/` 開頭）。- 輸入集必須非空，且輸入鍵必須是唯一的規範路徑。
  - 操作特定的約束很嚴格（`Add`/`Multiply` 多輸入，
    `RotateLeft`/`Bootstrap` 單輸入，具有互斥的深度/旋轉/引導旋鈕）。
  - 與政策掛鉤的錄取強制執行：
    - 策略/參數標識符和版本匹配。
    - 輸入計數/位元組、深度、旋轉和引導限制在策略上限內。
    - 確定性的預計輸出位元組符合策略密文限制。
- 解密權限策略：
  - `approver_ids` 必須非空、唯一且嚴格依照字典順序排序。
  - `ClientHeld` 模式只需要一名審核者，`approver_quorum=1`，
    和 `allow_break_glass=false`。
  - `ThresholdService` 模式需要至少兩個審核者並且
    `approver_quorum <= approver_ids.len()`。
  - `jurisdiction_tag` 必須非空且不得包含控製字元。
  - `audit_tag` 必須非空且不得包含控製字元。
- 解密請求：
  - `request_id`、`state_key` 和 `justification` 必須為非空
    （`state_key` 以 `/` 開頭）。
  - `jurisdiction_tag` 必須非空且不得包含控製字元。
  - 當 `break_glass=true` 時需要 `break_glass_reason`，當 `break_glass=true` 時必須省略 `break_glass_reason`
    `break_glass=false`。
  - 策略關聯准入強制策略名稱相等，不要求 TTL超過 `policy.max_ttl_blocks`，管轄權標籤平等，打破玻璃
    門控和同意證據要求
    `policy.require_consent_evidence=true` 用於不破碎玻璃的請求。
- 密文查詢規範：
  - `state_key_prefix` 必須非空且以 `/` 開頭。
  - `max_results` 是確定性有界的 (`<=256`)。
  - 元資料投影是明確的（`Minimal` 僅摘要與 `Standard` 鍵可見）。
- 密文查詢回應：
  - `result_count` 必須等於序列化行計數。
  - `Minimal` 投影不得暴露 `state_key`； `Standard` 必須公開它。
  - 行絕對不能顯示明文加密模式。
  - 包含證明（如果存在）必須包含非空方案 ID 和
    `anchor_sequence >= event_sequence`。
- 秘密信封：
  - `key_id`、`nonce` 和 `ciphertext` 必須為非空。
  - 隨機數長度有界（`<=256` 位元組）。
  - 密文長度有界（`<=33554432` 位元組）。
- 密文狀態記錄：
  - `state_key` 必須非空且以 `/` 開頭。
  - 元資料內容類型必須非空；標籤必須是唯一的非空字串。
  - `metadata.payload_bytes` 必須等於 `secret.ciphertext.len()`。
  - `metadata.commitment` 必須等於 `secret.commitment`。

## 規範裝置

規範 JSON 裝置存放在：- `fixtures/soracloud/sora_container_manifest_v1.json`
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

夾具/往返測試：

- `crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs`