<!-- Auto-generated stub for Chinese (Traditional) (zh-hant) translation. Replace this content with the full translation. -->

---
lang: zh-hant
direction: ltr
source: docs/source/soracloud/uploaded_private_models.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 97d6a421ce93a0e85be6cc99e828f965c9d8617d0ee27a772a2c9f2f646e77b7
source_last_modified: "2026-03-24T18:59:46.535846+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Soracloud 使用者上傳的模型和私有執行時間

本說明定義了 So Ra 用戶上傳的模型流程應如何落地
現有的 Soracloud 模型平面，無需發明並行運行時。

## 設計目標

增加僅 Soracloud 上傳的模型系統，讓客戶可以：

- 上傳自己的模型儲存庫；
- 將固定模型版本綁定到特務公寓或封閉競技場團隊；
- 使用加密輸入和加密模型/狀態運行私有推理；和
- 接收公開承諾、收據、定價和審計追蹤。

這不是 `ram_lfe` 功能。 `ram_lfe` 保留通用隱藏功能
子系統記錄在 `../universal_accounts_guide.md` 中。上傳模型
私有推理應該擴展 Soracloud 的現有模型註冊表，
工件、單元能力、FHE 和解密策略表面。

## 可重複使用的現有 Soracloud 表面

目前的 Soracloud 堆疊已經擁有正確的基礎物件：- `SoraModelRegistryV1`
  - 權威的每服務模型名稱和升級版本狀態。
- `SoraModelWeightVersionRecordV1`
  - 版本沿襲、升級、回滾、出處和再現性
    哈希值。
- `SoraModelArtifactRecordV1`
  - 確定性工件元資料已綁定到模型/權重管道。
- `SoraCapabilityPolicyV1.allow_model_inference`
  - 公寓/服務能力標誌應成為強制性的
    公寓綁定到上傳的模型。
- `SecretEnvelopeV1` 和 `CiphertextStateRecordV1`
  - 確定性加密位元組和密文狀態載體。
- `FheParamSetV1`、`FheExecutionPolicyV1`、`FheGovernanceBundleV1`、
  `DecryptionAuthorityPolicyV1` 和 `DecryptionRequestV1`
  - 用於加密執行和受控輸出的策略/治理層
    釋放。
- 目前Torii型號路線：
  - `/v1/soracloud/model/weight/{register,promote,rollback,status}`
  - `/v1/soracloud/model/artifact/{register,status}`
- 目前 Torii HF 共享租賃路由：
  - `/v1/soracloud/hf/{deploy,status,lease/leave,lease/renew}`

上傳的模型路徑應該會擴展這些表面。它不應該超載
HF 共享租約，且不應將 `ram_lfe` 重複使用為模型服務運行時。

## 規範上傳合約

Soracloud 的私有上傳模型合約應該只承認規範
擁抱臉式模型庫：- 所需的基礎文件：
  - `config.json`
  - 分詞器文件
  - 家庭需要時的處理器/預處理器文件
  - `*.safetensors`
- 在這一里程碑中承認的家庭團體：
  - 具有 RoPE/RMSNorm/SwiGLU/GQA 語意的僅解碼器因果 LM
  - LLaVA風格的文字+圖像模型
  - Qwen2-VL風格的文字+圖像模型
- 在此里程碑中被拒絕：
  - GGUF 作為上傳的私人運行時合約
  -ONNX
  - 缺少標記器/處理器資產
  - 不支援的架構/形狀
  - 音訊/視訊多模式套餐

為何簽訂此合約：

- 它與現有的已占主導地位的伺服器端模型佈局相匹配
  圍繞 safetensors 和 Hugging Face 儲存庫的生態系統；
- 它讓模型平面共享一個確定性標準化路徑
  所以Ra，Torii，以及運行時編譯；和
- 它避免將本機運行時匯入格式（例如 GGUF）與
  Soracloud 私有運行時合約。

HF 共享租約對於共享/公共來源匯入工作流程仍然有用，但是
私有上傳模型路徑將加密的編譯位元組儲存在鏈上，而不是
與從共享來源池租賃模型位元組相比。

## 分層模型平面設計

### 1. 來源與註冊層

擴展當前模型註冊表設計而不是替換它：- 新增 `SoraModelProvenanceKindV1::UserUpload`
  - 目前類型（`TrainingJob`、`HfImport`）不足以區分
    模型由 So Ra 等客戶端直接上傳和標準化。
- 保留 `SoraModelRegistryV1` 作為升級版本索引。
- 保留 `SoraModelWeightVersionRecordV1` 作為沿襲/升級/回滾記錄。
- 使用選購的上傳私有運行時擴充 `SoraModelArtifactRecordV1`
  參考文獻：
  - `private_bundle_root`
  - `chunk_manifest_root`
  - `compile_profile_hash`
  - `privacy_mode`

文物記錄仍是聯繫出處的確定性錨點，
可再現性元資料和捆綁標識在一起。體重記錄
仍然是升級版本的沿襲對象。

### 2. Bundle/Chunk 儲存層

為加密的上傳模型材料添加一流的 Soracloud 記錄：

- `SoraUploadedModelBundleV1`
  - `model_id`
  - `weight_version`
  - `family`
  - `modalities`
  - `runtime_format`
  - `bundle_root`
  - `chunk_count`
  - `plaintext_bytes`
  - `ciphertext_bytes`
  - `compile_profile_hash`
  - `pricing_policy`
  - `decryption_policy_ref`
- `SoraUploadedModelChunkV1`
  - `model_id`
  - `bundle_root`
  - `ordinal`
  - `offset_bytes`
  - `plaintext_len`
  - `ciphertext_len`
  - `ciphertext_hash`
  - 加密的有效負載（`SecretEnvelopeV1`）

確定性規則：- 明文字節在加密前被分成固定的 4 MiB 區塊；
- 區塊排序是嚴格且按序數驅動的；
- 區塊/根摘要在重播過程中保持穩定；和
- 每個加密分片必須保持在目前 `SecretEnvelopeV1` 以下
  密文上限為 `33,554,432` 位元組。

這個里程碑透過區塊將文字加密位元組儲存在鏈狀態中
記錄。它不會將私有上傳模型位元組卸載到 SoraFS。

由於鍊是公開的，上傳機密性必須來自真實的
Soracloud 持有的接收者金鑰，而不是來自公共派生的確定性金鑰
元數據。桌面應該獲取廣告的上傳加密收件人，
使用隨機的每次上傳捆綁金鑰加密區塊，並僅發布
收件人元資料加上封裝的捆綁金鑰信封以及密文。

### 3.編譯/執行時層

在 Soracloud 下新增專用的私有轉換器編譯器/執行時間層：- 標準化 BFV 支援的確定性低精度編譯推理
  現在，因為 CKKS 存在於模式討論中，但不存在於實作中
  本地運行時；
- 將承認的模型編譯成確定性的 Soracloud 私有 IR，涵蓋
  嵌入、線性/投影層、注意力矩陣相乘、RoPE、RMSNorm /
  LayerNorm 近似、MLP 區塊、視覺區塊投影和
  影像到解碼器投影機路徑；
- 使用確定性定點推理：
  - int8 權重
  - int16 激活
  - int32累加
  - 經批准的非線性多項式近似

此編譯器/運行時獨立於 `ram_lfe`。它可以重複使用 BFV 原語
和Soracloud FHE治理對象，但不是同一個執行引擎
或路線家族。

### 4. 推理/會話層

新增私有運行的會話和檢查點記錄：- `SoraPrivateCompileProfileV1`
  - `family`
  - `quantization`
  - `opset_version`
  - `max_context`
  - `max_images`
  - `vision_patch_policy`
  - `fhe_param_set`
  - `execution_policy`
- `SoraPrivateInferenceSessionV1`
  - `session_id`
  - `apartment`
  - `model_id`
  - `weight_version`
  - `bundle_root`
  - `input_commitments`
  - `token_budget`
  - `image_budget`
  - `status`
  - `receipt_root`
  - `xor_cost_nanos`
- `SoraPrivateInferenceCheckpointV1`
  - `session_id`
  - `step`
  - `ciphertext_state_root`
  - `receipt_hash`
  - `decrypt_request_id`
  - `released_token`
  - `compute_units`
  - `updated_at_ms`

私人執行意味著：

- 加密的提示/影像輸入；
- 加密模型權重和啟動；
- 輸出的顯式解密策略發佈；
- 公共運作時間收據和成本核算。

這並不意味著沒有承諾或可審計性的隱藏執行。

## 客戶的責任

因此 Ra 或其他客戶端應該在之前執行確定性本地預處理
上傳到 Soracloud：

- 分詞器應用程式；
- 將圖像預處理為承認的文字+圖像族的補丁張量；
- 確定性束歸一化；
- 令牌 ID 和影像補丁張量的客戶端加密。

Torii 應接收加密輸入加上公開承諾，而非原始提示
文字或原始圖像，用於私有路徑。

## API 和 ISI 計劃保留現有模型註冊表路由作為規範註冊表層並添加
頂部新的上傳/運行時路由：

- `POST /v1/soracloud/model/upload/init`
- `POST /v1/soracloud/model/upload/chunk`
- `POST /v1/soracloud/model/upload/finalize`
- `GET /v1/soracloud/model/upload/encryption-recipient`
- `POST /v1/soracloud/model/compile`
- `POST /v1/soracloud/model/allow`
- `POST /v1/soracloud/model/run-private`
- `GET /v1/soracloud/model/run-status`
- `POST /v1/soracloud/model/decrypt-output`

使用匹配的 Soracloud ISI 支援它們：

- 捆綁註冊
- 區塊追加/完成
- 編製錄取通知書
- 私人運行開始
- 檢查點記錄
- 輸出釋放

流程應該是：

1. upload/init 建立確定性捆綁會話和預期根；
2. upload/chunk 依序附加加密分片；
3.上傳/完成密封捆綁根和清單；
4.compile 產生一個綁定到的確定性私有編譯設定檔
   承認的捆綁；
5.模型/工件+模型/權重註冊表記錄引用上傳的包
   而不僅僅是培訓工作；
6.allow-model將上傳的模型綁定到已接納的公寓
   `allow_model_inference`；
7. run-private 記錄會話並發出檢查點/收據；
8. 解密輸出發布受管制的輸出資料。

## 定價與控制平面策略

擴展當前 Soracloud 充電/控制平面行為：- 運行上傳模型的公寓需要`allow_model_inference`；
- XOR 中的價格儲存、編譯、執行階段步驟和解密發布；
- 在此里程碑中，對上傳模型運行禁用敘述傳播；
- 將上傳的模型保留在 So Ra 的封閉競技場和出口門控流程中。

## 授權和綁定語義

上傳、編譯和運行是獨立的功能，應該保持獨立
模型飛機。

- 上傳模型包不得隱式授權公寓運行它；
- 編譯成功不得隱式將模型版本升級為目前版本；
- 公寓綁定應該透過 `allow-model` 風格突變來明確
  記錄：
  - 公寓，
  - 型號 ID，
  - 重量版本，
  - 束根，
  - 隱私模式，
  - 簽名者/審核順序；
- 綁定到上傳模型的公寓必須已經承認
  `allow_model_inference`；
- 突變路線應繼續需要相同的 Soracloud 簽名請求
  現有模式/工件/訓練路線所使用的規則，應該是
  由 `CanManageSoracloud` 或同等明確的授權機構保護
  模型。

這可以防止“我上傳了它，因此每個私人公寓都可以運行它”
漂移並保持公寓執行政策明確。

## 狀態和審計模型

新記錄需要權威的讀取和審計表面，而不僅僅是突變
路線。

推薦補充：- 上傳狀態
  - 透過 `service_name + model_name + weight_version` 或透過
    `model_id + bundle_root`；
- 編譯狀態
  - 透過`model_id + bundle_root + compile_profile_hash`查詢；
- 私人運作狀態
  - 透過 `session_id` 查詢，其中包含公寓/型號/版本上下文
    回應；
- 解密輸出狀態
  - 透過 `decrypt_request_id` 查詢。

審計應該停留在現有的 Soracloud 全域序列上，而不是
建立第二個每個功能計數器。新增一流的審核事件：

- 上傳初始化/完成
- 塊附加/密封
- 編譯被接受/編譯被拒絕
- 公寓模型允許/撤銷
- 私人運作開始/檢查點/完成/失敗
- 輸出釋放/拒絕

這使得上傳的模型活動在相同的權威重播中可見，並且
營運故事作為當前服務、訓練、模型權重、模型工件、
HF 共享租賃和公寓審計流程。

## 入學配額和州增長限制

只有當准入有界時，鏈上的文字加密模型位元組才可行
積極地。

實現應至少定義確定性限制：

- 每個上傳包的最大明文字節數；
- 每個包的最大加密位元組數；
- 每個包的最大區塊數；
- 每個機構/服務的最大並發在線上上傳會話數；
- 每個服務/公寓視窗的最大編譯作業數；
- 每個私人會話的最大保留檢查點數量；
- 每個會話的最大輸出釋放請求。Torii 和核心應拒絕超過先前聲明的限制的上傳
發生狀態放大。限制應該是配置驅動的，其中
適當的，但驗證結果必須在同儕之間保持確定性。

## 重播與編譯器決定論

私有編譯器/執行時間路徑比普通編譯器/執行時間路徑具有更高的確定性負擔
服務部署。

所需的不變量：

- 族群檢測和標準化必須產生穩定的規範束
  在發出任何編譯哈希之前；
- 編譯設定檔哈希必須綁定：
  - 標準化束根，
  - 家庭，
  - 量化配方，
  - opset版本，
  - FHE參數集，
  - 執行政策；
- 運行時必須避免不確定的核心、浮點漂移和
  硬體特定的減少可能會改變產出或收入
  同行。

在擴展承認的家庭設定之前，先為
每個系列類別和鎖定編譯輸出加上運行時收據與黃金
測試。

## 程式碼之前的剩餘設計差距

最大的未解決的實施問題現已縮小為具體的
後端決策：- 精确的上传/分块/请求 DTO 形状和 Norito 模式；
- 用于捆绑/块/会话/检查点查找的世界状态索引键；
- `iroha_config` 中的配额/默认配置放置；
- 模型/工件状态是否应该变得面向版本而不是
  当 `UserUpload` 存在时，以培训工作为导向；
- 公寓遺失時的精確撤銷行為
  `allow_model_inference` 或固定模型版本已回滾。

这些是下一个设计到代码的桥梁项目。建築佈局
該功能現在應該穩定了。

## 測試矩陣- 上傳驗證：
  - 接受規範的 HF safetensors 回購協議
  - 拒絕 GGUF、ONNX、缺少標記器/處理器資產、不支援
    架構和音訊/視訊多模式包
- 分塊：
  - 確定性叢根
  - 穩定的區塊排序
  - 精確重建
  - 信封天花板強制執行
- 註冊表一致性：
  - 重播下的捆綁/區塊/工件/權重提升正確性
- 編譯器：
  - 僅解碼器、LLaVA 型和 Qwen2-VL 型各一個小型夾具
  - 拒絕不支援的操作和形狀
- 私人運行時：
  - 加密的微型裝置端對端煙霧測試，具有穩定的收據和
    閾值輸出釋放
- 定價：
  - 上傳、編譯、執行階段步驟和解密的異或費用
- So Ra 整合：
  - 上傳、編譯、發布、綁定團隊、執行封閉競技場、檢查收據、
    儲存項目，重新打開，確定性地重新運行
- 安全：
  - 沒有出口門旁路
  - 沒有敘事自動傳播
  - 公寓綁定失敗，沒有 `allow_model_inference`

## 實作切片1. 新增缺少的資料模型欄位和新記錄類型。
2. 新增新的 Torii 請求/回應類型和路由處理程序。
3. 新增匹配的 Soracloud ISI 和世界狀態儲存。
4. 新增確定性捆綁/區塊驗證和鏈上加密位元組
   儲存。
5. 新增一個微型 BFV 支援的專用變壓器固定裝置/運行時路徑。
6. 擴展 CLI 模型命令以涵蓋上傳/編譯/私有運行流程。
7. 一旦後端路徑具有權威性，即可進行 Land So Ra 整合。