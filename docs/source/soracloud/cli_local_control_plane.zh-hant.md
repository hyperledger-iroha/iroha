<!-- Auto-generated stub for Chinese (Traditional) (zh-hant) translation. Replace this content with the full translation. -->

---
lang: zh-hant
direction: ltr
source: docs/source/soracloud/cli_local_control_plane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 567b63e9b61afaecfa5d85aa60f0348c856557e171559885ffaba45168ce61dc
source_last_modified: "2026-03-26T06:12:11.480025+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Soracloud CLI 和控制平面

Soracloud v1 是一個權威的、僅限 IVM 的運行時。

- `iroha app soracloud init` 是唯一的離線指令。它搭起鷹架
  `container_manifest.json`、`service_manifest.json` 和選購模板
  Soracloud 服務的工件。
- 所有其他 Soracloud CLI 命令僅支援網路並且需要
  `--torii-url`。
- CLI 不維護任何本機 Soracloud 控制平面鏡像或狀態
  文件。
- Torii 直接從公共 Soracloud 狀態和突變路線提供服務
  權威的世界狀態加上嵌入式 Soracloud 執行時間管理器。

## 運行時範圍- Soracloud v1 僅接受 `SoraContainerRuntimeV1::Ivm`。
- `NativeProcess` 仍然被拒絕。
- 有序郵箱執行直接運行承認的 IVM 處理程序。
- 水化和物化來自承諾的 SoraFS/DA 內容而不是
  比合成本地快照。
- `SoraContainerManifestV1` 現在攜帶 `required_config_names` 和
  `required_secret_names`，加上顯式 `config_exports`。部署、升級、
  當有效的權威材料集出現時，回滾失敗關閉
  不滿足那些宣告的綁定或當配置導出的目標是
  非必要的配置或重複的環境/檔案目標。
- 承諾的服務配置條目現已具體化
  `services/<service>/<version>/configs/<config_name>` 作為規範 JSON
  有效負載文件。
- 明確配置環境導出被投影到
  `services/<service>/<version>/effective_env.json`，檔案匯出為
  物化下
  `services/<service>/<version>/config_exports/<relative_path>`。出口
  值使用引用的配置條目的規範 JSON 有效負載文字。
- Soracloud IVM 處理程序現在可以讀取這些權威配置有效負載
  直接通過運行時主機`ReadConfig`面，如此普通
  `query`/`update` 處理程序不需要猜測節點本地檔案路徑
  使用已提交的服務配置。
- 承諾服務秘密信封現已具體化
  `services/<service>/<version>/secret_envelopes/<secret_name>` 為
  權威的信封文件。- 普通 Soracloud IVM 處理程序現在可以讀取那些提交的秘密
  包絡直接通過運行時主機 `ReadSecretEnvelope` 表面。
- 遺留的私有執行時間後備樹現在已從提交同步
  `secrets/<service>/<version>/<secret_name>` 下的部署狀態，因此
  舊的原始秘密讀取路徑和權威控制平面點
  相同的位元組。
- 私有運行時 `ReadSecret` 現在解決了權威部署問題
  `service_secrets` 首先且僅回退到舊節點本地
  `secrets/<service>/<version>/...` 未提交時的物化文件樹
  所請求的金鑰存在服務秘密條目。
- 秘密攝取仍故意比配置攝取更窄：
  `ReadSecretEnvelope` 是公共安全的普通處理程序合約，而
  `ReadSecret` 仍然僅私有運行時，並且仍然返回已提交的
  信封密文字節而不是明文掛載合約。
- 運行時服務計劃現在公開相應的攝取功能
  布林值加上聲明的 `config_exports` 和有效的預計
  環境，因此狀態消費者可以判斷是否進行了具體的修訂
  支援主機配置讀取、主機秘密信封讀取、私人原始秘密
  讀取和明確配置注入，無需從處理程序推斷它
  獨自上課。

## CLI 指令- `iroha app soracloud init`
  - 僅離線鷹架。
  - 支援 `baseline`、`site`、`webapp` 和 `pii-app` 範本。
- `iroha app soracloud deploy`
  - 在本地驗證 `SoraDeploymentBundleV1` 准入規則，簽署
    請求，並呼叫 `POST /v1/soracloud/deploy`。
  - `--initial-configs <path>` 和 `--initial-secrets <path>` 現在可以附加
    權威的內聯服務配置/秘密映射以原子方式
    首先部署，以便在首次准入時可以滿足所需的綁定。
  - CLI 現在使用以下任一方式對 HTTP 請求進行規範簽名
    `X-Iroha-Account`、`X-Iroha-Signature`、`X-Iroha-Timestamp-Ms` 和
    `X-Iroha-Nonce` 對於普通單簽名帳戶，或
    `X-Iroha-Account` 加 `X-Iroha-Witness` 時
    `soracloud.http_witness_file` 指向多重簽章見證 JSON 負載；
    Torii 傳回確定性草案交易指令集和
    CLI 然後透過正常的 Iroha 用戶端提交真實交易
    車道。
  - Torii 也強制執行 SCR 主機准入上限和故障關閉功能
    在接受突變之前進行檢查。
- `iroha app soracloud upgrade`
  - 驗證並簽署新的捆綁版本，然後調用
    `POST /v1/soracloud/upgrade`。
  - 相同的 `--initial-configs <path>` / `--initial-secrets <path>` 流量是
    可用於升級期間的原子材料更新。
  - 在升級之前，相同的 SCR 主機准入檢查在伺服器端運行
    承認了。
- `iroha app soracloud status`- 從`GET /v1/soracloud/status`查詢權威服務狀態。
- `iroha app soracloud config-*`
  - `config-set`、`config-delete` 和 `config-status` 僅支援 Torii。
  - CLI 簽署規範的服務配置來源有效負載和調用
    `POST /v1/soracloud/service/config/set`，
    `POST /v1/soracloud/service/config/delete`，和
    `GET /v1/soracloud/service/config/status`。
  - 配置條目保留在權威部署狀態並保留
    附加在部署/升級/回滾修訂變更中。
  - 當活動修訂版仍然聲明時，`config-delete` 現在無法關閉
    `container.required_config_names` 中的命名配置。
- `iroha app soracloud secret-*`
  - `secret-set`、`secret-delete` 和 `secret-status` 僅支援 Torii。
  - CLI 簽署規範的服務秘密來源有效負載和調用
    `POST /v1/soracloud/service/secret/set`，
    `POST /v1/soracloud/service/secret/delete`，和
    `GET /v1/soracloud/service/secret/status`。
  - 秘密條目作為權威 `SecretEnvelopeV1` 記錄保存
    處於部署狀態並能承受正常的服務修訂變更。
  - 當活動修訂版仍然聲明時，`secret-delete` 現在無法關閉
    `container.required_secret_names` 中命名的秘密。
- `iroha app soracloud rollback`
  - 簽署回滾元資料並呼叫 `POST /v1/soracloud/rollback`。
- `iroha app soracloud rollout`
  - 簽署推出元資料並呼叫 `POST /v1/soracloud/rollout`。
- `iroha app soracloud agent-*`
  - 所有公寓生命週期、錢包、郵箱和自治命令
    僅支援 Torii。
- `iroha app soracloud training-*`
  - 所有訓練作業指令僅受 Torii 支援。- `iroha app soracloud model-*`
  - 所有模型工件、權重、上傳模型和私有執行時間指令
    僅由 Torii 支援。
  - 上傳模型/私有運行時表面現在位於同一個家族：
    `model-upload-encryption-recipient`、`model-upload-init`、
    `model-upload-chunk`, `model-upload-finalize`, `model-upload-status`,
    `model-compile`, `model-compile-status`, `model-allow`,
    `model-run-private`、`model-run-status`、`model-decrypt-output` 和
    `model-publish-private`。
  - `model-run-private` 現在隱藏草稿然後最終確定運行時握手
    CLI 並傳回權威的最終確定後會話狀態。
  - `model-publish-private` 現在支援準備好的
    捆綁/分塊/最終確定/編譯/允許發布計劃和更高級別
    文件草案。草案現在有 `source: PrivateModelSourceV1`，
    它接受 `LocalDir { path }` 或
    `HuggingFaceSnapshot { repo, revision }`。
  - 當使用 `--draft-file` 呼叫時，CLI 會對聲明的來源進行標準化
    進入確定性臨時樹，驗證 v1 HF safetensors 合約，
    根據活動確定性地序列化和加密捆綁包
    Torii 上傳接收者，將其分成固定大小的加密區塊，
    可選擇透過 `--emit-plan-file` 寫入準備好的計劃，然後
    執行上傳/完成/編譯/允許序列。
  - `HuggingFaceSnapshot` 修訂是強制性的，必須固定提交
    SHA；類似分支的引用會被拒絕並失敗關閉。- v1 中承認的來源佈局故意變窄：`config.json`，
    標記器資產、一個或多個 `*.safetensors` 分片以及可選
    支援影像的模型的處理器/預處理器元資料。 GGUF、ONNX、
    其他非安全張量權重和任意嵌套自訂佈局是
    被拒絕了。
  - 當使用 `--plan-file` 呼叫時，CLI 仍然消耗已經
    準備好發布計劃文件並在計劃上傳時失敗關閉
    收件者不再與權威 Torii 收件者配對。
  - 有關分層這些路由的設計，請參閱 `uploaded_private_models.md`
    到現有的模型註冊表和工件/權重記錄。
- `model-host` 控制平面路線
  - Torii現已公開權威
    `POST /v1/soracloud/model-host/advertise`，
    `POST /v1/soracloud/model-host/heartbeat`，
    `POST /v1/soracloud/model-host/withdraw`，和
    `GET /v1/soracloud/model-host/status`。
  - 這些路由持續存在選擇加入驗證器主機功能廣告
    權威的世界狀態，讓業者檢查哪些驗證者是
    目前廣告模特兒-主持人能力。
  - `iroha app soracloud model-host-advertise`，
    `model-host-heartbeat`、`model-host-withdraw` 和
    `model-host-status` 現在簽署了與
    原始API並直接呼叫匹配的Torii路由。
- `iroha app soracloud hf-*`
  - `hf-deploy`、`hf-status`、`hf-lease-leave` 和 `hf-lease-renew` 是
    僅支援 Torii。- `hf-deploy` 和 `hf-lease-renew` 現在也自動承認確定性
    為請求的 `service_name` 產生 HF 推理服務，以及
    自動承認確定性生成的 HF 單元
    `apartment_name` 當請求時，在共享租約突變之前
    已提交。
  - 重複使用將失敗關閉：如果指定的服務/公寓已存在，但已存在
    不是該規範源預期產生的高頻部署，高頻
    突變被拒絕，而不是默默地將租約綁定到不相關的
    Soracloud 物件。
  - 當附加嵌入式執行時間管理器時，`hf-status` 現在也
    傳回規範來源的運行時投影，包括綁定
    服務/公寓、排隊的下一窗口可見性和本地捆綁/
    工件快取未命中； `importer_pending` 遵循運行時投影
    而不是僅僅依賴權威的源枚舉。
  - 當 `hf-deploy` 或 `hf-lease-renew` 承認產生的 HF 服務時
    與共享租賃突變相同的交易，權威的 HF
    來源現在立即翻轉到 `Ready` 並且 `importer_pending` 保持不變
    響應中的 `false`。
  - HF 租賃狀態和突變響應現在也暴露任何權威
    放置快照已附加到活動租用窗口，包括分配的主機、合格主機數、熱主機數和單獨的主機數
    儲存與計算費用欄位。
  - `hf-deploy` 和 `hf-lease-renew` 現在派生規範的 HF 資源
    在提交之前，從已解析的 Hugging Face 儲存庫元資料中取得設定檔
    突變：
    - Torii 檢查儲存庫 `siblings`，偏好 `.gguf`
      `.safetensors` 透過 PyTorch 權重佈局，將所選檔案引導至
      派生 `required_model_bytes`，並將其對應到第一個版本
      後端/格式加上 RAM/磁碟層；
    - 當沒有即時驗證器主機廣告時，租賃准入失敗關閉
      滿足該設定檔；和
    - 當主機集可用時，活動視窗現在會記錄
      確定性權益加權放置和單獨的計算預留
      費用與現有的儲存租賃會計一起。
  - 後來加入活躍 HF 窗口的會員現在按比例支付存儲費用，
    僅計算剩餘視窗的份額，而較早的成員
    獲得相同的確定性存儲退款和計算退款會計
    從那晚加入。
  - 嵌入式執行時間管理器可以綜合產生的 HF 存根包
    本地，因此這些生成的服務無需等待即可實現
    僅為佔位符推理包提交了 SoraFS 有效負載。- 嵌入式執行時間管理器現在也導入列入白名單的 Hugging Face 儲存庫
    文件到 `soracloud_runtime.state_dir/hf_sources/<source_id>/files/` 和
    保留本地 `import_manifest.json` 以及已解析的提交，導入
    文件、跳過的文件以及任何導入器錯誤。
  - 產生的 HF `metadata` 本機讀取現在返回本機匯入清單，
    包括導入的文件清單以及是否本地執行和
    為該節點啟用了網橋回退。
  - 產生的 HF `infer` 本地讀取現在更喜歡在節點上執行
    導入的共享位元組：
    - `irohad` 在本地實作嵌入式Python適配器腳本
      Soracloud運行時狀態目錄並透過以下方式呼叫它
      `soracloud_runtime.hf.local_runner_program`；
    - 嵌入式運轉器首先檢查確定性的固定裝置節
      `config.json`（用於測試），然後載入導入的來源
      目錄通過`transformers.pipeline(..., local_files_only=True)`所以
      該模型針對共享本地導入執行，而不是拉取
      新的集線器位元組；和
    - 如果 `soracloud_runtime.hf.allow_inference_bridge_fallback = true` 和
      配置了`soracloud_runtime.hf.inference_token`，運行時間下降
      僅在本機執行時傳回配置的 HF 推理基本 URL
      不可用或失敗，且呼叫者明確選擇加入
      `x-soracloud-hf-allow-bridge-fallback: 1`、`true` 或 `yes`。
  - 運行時投影現在將 HF 來源保留在 `PendingImport` 中，直到存在成功的本機匯入清單，匯入器失敗表現為
    運行時 `Failed` 加 `last_error`，而不是默默報告 `Ready`。
  - 生成的 HF 公寓現在消耗經過批准的自治運行
    節點本地運行時路徑：
    - `agent-autonomy-run` 現在遵循兩步驟流程：首先簽名
      突變記錄權威批准並返回確定性
      草案交易，然後第二個簽名的最終確定請求詢問
      嵌入式運行時管理器來執行已核准的邊界運行
      產生HF `/infer`服務並返回任何權威後續
      作為另一個確定性草案的指示；
    - 批准的運行記錄現在也保持規範
      `request_commitment`，所以後面產生的服務回執即可
      綁定到確切的權威自主批准；
    - 批准現在可以保留可選的規範 `workflow_input_json`
      身體；如果存在，嵌入式執行時會轉送確切的 JSON 負載
      到產生的 HF `/infer` 處理程序，當不存在時，它會回退到
      舊的 `run_label`-as-`inputs` 信封帶有權威
      `artifact_hash` / `provenance_hash` / `budget_units` / `run_id` 攜帶
      作為結構化參數；
    - `workflow_input_json` 現在也可以選擇確定性順序多步驟執行
      `{ "workflow_version": 1, "steps": [...] }`，其中每个步骤运行一个
      產生的 HF `/infer` 請求和後續步驟可以參考先前的輸出
      通过 `${run.*}`、`${previous.text|json|result_commitment}` 和
      `${steps.<step_id>.text|json|result_commitment}` 占位符；和
    - 突變反應和 `agent-autonomy-status` 現在都浮出水面
      節點本地執行摘要（如果可用），包括成功/失敗，
      綁定服務修訂、確定性結果承諾、檢查點
      / 日誌工件雜湊值、產生的服務 `AuditReceipt` 以及
      解析的 JSON 响应正文。
    - 當產生的服務收據存在時，Torii 將其記錄到
      权威的 `soracloud_runtime_receipts` 并公开了结果
      關於最近運行狀態的權威運行時收據以及
      节点本地执行摘要。
    - 產生的 HF 自主路徑現在也記錄了專門的權威
      公寓 `AutonomyRunExecuted` 審核事件與最近運作狀態
      返回執行審核以及權威的運行時收據。
  - `hf-lease-renew` 现在有两种模式：
    - 如果目前視窗已過期或耗盡，它會立即開啟一個新視窗
      窗戶；
    - 如果目前視窗仍然處於活動狀態，它將呼叫者作為呼叫者排隊
      下一窗口贊助商，收取全部下一窗口儲存和計算費用預先預訂費用，堅持確定性的下一個窗口
      安置計劃，並透過 `hf-status` 公開排隊的贊助
      直到後來的突變使池向前滾動。
  - 產生的 HF 公共 `/infer` 入口現在解析權威
    放置，並且當接收節點不是熱主節點時，代理
    透過 Soracloud P2P 控制訊息向指定的主主機請求；
    嵌入式運行時在直接副本/未分配的本機上仍然無法關閉
    執行和產生的 HF 運行時收據帶有 `placement_id`，
    驗證者，以及來自權威放置記錄的同儕歸因。
  - 當代理程式到主路徑逾時、在回應之前關閉時，或
    返回權威機構的非客戶端運行時故障
    主節點，入口節點現在報告 `AssignedHeartbeatMiss`
    Primary 和 Enqueues `ReconcileSoracloudModelHosts` 透過相同的
    內部突變泳道。
  - 權威的過期主機協調記錄現在保留
    模型主機違規證據，重複使用公用通道驗證器斜線路徑，
    並套用預設的 HF 共享租賃懲罰政策：
    `warmup_no_show_slash_bps=500`，
    `assigned_heartbeat_miss_slash_bps=250`，
    `assigned_heartbeat_miss_strike_threshold=3`，和
    `advert_contradiction_slash_bps=1000`。
  - 本地分配的 HF 運行時健康狀況現在也提供相同的證據路徑：本地 `Warming` 主機上的協調時間導入/預熱失敗
    `WarmupNoShow`，以及本地暖主節點上的常駐工作人員故障
    透過正常方式發出節流 `AssignedHeartbeatMiss` 報告
    交易隊列。
  - 現在協調還可以預先啟動並調查當地的常駐高頻工人
    分配的熱主機/熱主機，包括副本，因此副本可能會失敗
    在任何之前關閉權威 `AssignedHeartbeatMiss` 路徑
    公共 `/infer` 請求曾經到達主資料庫。
  - 當本地探測成功時，運行時現在也會發出一個
    本地驗證器的權威 `model-host-heartbeat` 突變
    分配的主機仍然是 `Warming` 或活動主機廣告需要 TTL
    刷新，因此成功的本地準備可以促進同樣的權威
    手動心跳將更新的放置/廣告狀態。
  - 運行時發出本地 `WarmupNoShow` 或
    `AssignedHeartbeatMiss`，它現在也入隊
    `ReconcileSoracloudModelHosts` 通過相同的內部突變泳道所以
    權威的故障轉移/回填立即開始，而不是等待
    稍後定期進行主機到期掃描。
  - 當公共產生的 HF 入口甚至更早失敗時，因為已提交
    放置沒有可代理的熱主節點，Torii 現在詢問運行時
    處理排隊相同的權威`ReconcileSoracloudModelHosts` 立即指令而不是等待
    用於稍後到期或工作失敗訊號。
  - 當公共產生的 HF 入口確實收到代理成功回應時，
    Torii 現在驗證包含的運行時收據仍然證明執行
    積極安置的承諾熱情初選；缺失或不匹配
    展示位置歸因現在無法關閉，並暗示相同的權威
    `ReconcileSoracloudModelHosts` 路徑而不是返回
    非權威回應。 Torii 現在也拒絕代理成功
    當運行時接收承諾或認證策略執行時的回應
    與即將返回的響應不匹配，以及相同的錯誤收據
    路徑還提供遠端主 `AssignedHeartbeatMiss` 報告
    鉤子。
  - 代理程式產生的 HF 執行失敗現在請求相同的
    上報後權威`ReconcileSoracloudModelHosts`路徑
    遠端主要健康故障，而不是等待稍後到期
    掃。
  - Torii 現在將每個掛起的產生的 HF 代理請求綁定到
    它所針對的權威主要同行。來自錯誤的代理回應
    對等點現在被忽略，而不是毒害待處理的請求，因此僅
    權威主節點可以完成或失敗該請求。代理商
    來自具有不受支援的代理回應模式的預期對等方的回應版本仍然失敗關閉而不是被接受只是因為它
    `request_id` 符合待處理的請求。如果回答錯誤的同行
    本身仍然是該放置的分配的 generated-HF 主機，
    運行時現在透過現有的報告主機
    `WarmupNoShow` / `AssignedHeartbeatMiss` 證據路徑基於其
    權威分配狀態，也暗示權威
    `ReconcileSoracloudModelHosts`，所以過時的主/複製權限漂移
    為控制循環提供訊號，而不是僅在入口處被忽略。
  - 傳入的 Soracloud 代理執行現在也僅限於預期的
    已提交的熱主資料庫上產生的 HF `infer` 查詢案例。非高頻
    公共本地讀取路由和產生的 HF 請求傳遞到節點
    這不再是權威的暖主節點，現在改為失敗關閉
    透過 P2P 代理路徑執行。權威初選現在也
    在執行前重新計算規範產生的 HF 請求承諾，
    因此偽造或不匹配的代理信封無法關閉。
  - 當指定的副本或過時的前主節點拒絕傳入的副本時
    generated-HF代理執行，因為它不再是權威的
    溫暖的主要，接收端運行時現在也提示
    `ReconcileSoracloudModelHosts` 而不是只依賴呼叫方
    路由視圖。- 當相同的傳入產生的 HF 代理權限失敗發生時
    本地權威主節點本身，運行時現在將其視為
    一流的宿主健康訊號：溫暖的初選自我報告
    `AssignedHeartbeatMiss`，暖化初選自我報告 `WarmupNoShow`，
    並且兩條路徑立即重複使用相同的權威
    `ReconcileSoracloudModelHosts` 控制迴路。
  - 當同一個非主要接收者仍然是權威接收者之一時
    分配的主機並且可以從提交的鏈狀態解析熱主節點，
    現在它將產生的 HF 請求重新代理到該主節點
    立即失敗。未指派的驗證器失敗關閉而不是執行
    作為通用中間 HF 代理躍點，原始入口節點仍然
    根據權威放置驗證返回的運行時收據
    狀態。如果指定副本到主節點的前向跳躍在
    請求實際上被調度，接收方運行時報告
    遠程主健康故障及權威提示
    `ReconcileSoracloudModelHosts`；如果本地分配的副本甚至不能
    嘗試向前跳躍，因為它自己的代理傳輸/運行時丟失，
    該故障現在被視為本地分配主機故障，而不是
    歸咎於初級。
  - 協調現在也會在下列情況下自動發出 `AdvertContradiction`本機驗證器配置的運行時對等 ID 與
    此驗證器的權威 `model-host-advertise` 對等 ID。
  - 有效的模型主機重新通告突變現在也同步權威
    分配主機 `peer_id` / `host_class` 元資料並重新計算目前
    主機類別變更時的安置預約費用。
  - 矛盾的模型宿主重新通告突變現在會立即發出
    `AdvertContradiction` 證據，應用現有驗證器斜線/驅逐
    路徑，並刷新受影響的展示位置，而不僅僅是驗證失敗。
  - 剩餘的 HF 託管工作現在是：
    - 超出本地範圍的更廣泛的跨節點/運行時叢集健康訊號
      驗證者的直接工作/預熱觀察加上分配的主機
      當遠端對等內部健康狀況應該時，接收方權限失敗
      也提供權威的重新平衡/斜線路徑。
  - 產生的 HF 本機執行現在保留一個常駐的每個來源 Python 工作線程
    在 `irohad` 下存活，在重複的 `/infer` 中重複使用載入的模型
    調用，並在本地導入時確定性地重新啟動該工作程序
    明顯變化或進程退出。
  - 這些路由不是私人上傳模型路徑。 HF 共享租約維持不變
    專注於共享來源/導入成員資格而不是加密的鏈上
    私有模型位元組。- 產生的 HF 自主批准現在支援確定性順序
    多步驟請求範圍，但更廣泛的非線性/工具使用
    編排和工件圖執行仍然是後續工作
    超越連結的 `/infer` 步驟。

## 狀態語義

`/v1/soracloud/status` 和相關代理程式/訓練/模型狀態端點現在
反映權威的運行時狀態：

- 承認來自承諾的世界狀態的服務修訂；
- 來自嵌入式運行時管理器的運行時水合/物化狀態；
- 真實信箱執行回執和失敗狀態；
- 出版的期刊/檢查點工件；
- 快取和運行時運作狀況而不是佔位符狀態墊片。

如果權威運行時材料已過時或不可用，則讀取將失敗關閉
而不是退回到本地狀態鏡像。

`/v1/soracloud/status` 是 v1 中唯一記錄的 Soracloud 狀態端點。
沒有單獨的 `/v1/soracloud/registry` 路由。

## 移除本地腳手架

這些舊的本地模擬概念在 v1 中不再存在：

- CLI 本機登錄/狀態檔或登錄路徑選項
- Torii-本地檔案支援的控制平面鏡像

## 範例

```bash
iroha app soracloud deploy \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080 \
  --api-token <token-if-required> \
  --timeout-secs 10
```

## 註釋- 在簽署和提交請求之前，本機驗證仍會運作。
- 標準 Soracloud 突變端點不再接受原始 `authority` /
  `private_key` 用於部署、升級、回溯、推出、代理的 JSON 字段
  生命週期、訓練、模型宿主和模型權重路徑； Torii 驗證
  這些請求來自規範的 HTTP 簽名標頭。
- 多重簽章控制的 Soracloud 擁有者現在使用 `X-Iroha-Witness`；點
  `soracloud.http_witness_file` 位於您希望 CLI 執行的確切見證 JSON
  重放下一個突變請求，如果以下情況，Torii 將失敗關閉：
  見證主體帳戶或規範請求哈希不符。
- `hf-deploy` 和 `hf-lease-renew` 現在包含客戶端簽署的輔助
  確定性產生的 HF 服務/公寓工件的來源，
  因此 Torii 不再需要呼叫者私鑰來承認那些後續操作
  對象。
- `agent-autonomy-run` 和 `model/run-private` 現在使用草稿然後定稿
  流程：第一個簽名的突變記錄了權威批准/開始，
  第二個簽名的完成請求執行運行時路徑並返回
  任何權威的後續指示作為確定性草案
  交易。
- `model/decrypt-output` 現在返回權威的私有推理
  檢查點作為確定性草案交易，僅由外部簽名交易，而不是透過嵌入的 Torii 持有的私鑰。
- ZK 附件 CRUD 現在對已簽署的 Iroha 帳戶進行金鑰租用，並且仍然有效
  啟用後，將 API 令牌視為額外的存取門。
- 公共 Soracloud 本地讀取入口現在應用明確的每 IP 速率，並且
  並發限制並在本地或之前重新檢查公共路由可見性
  代理執行。
- 私有運行時功能實施發生在 Soracloud 主機 ABI 內部，
  不在 CLI 或 Torii 本地腳手架內。
- `ram_lfe` 仍然是一個單獨的隱藏功能子系統。用戶上傳的私有
  變壓器執行應重複使用 Soracloud FHE/解密治理和
  模型註冊表，而不是 `ram_lfe` 請求路徑。
- 運行時健康狀況、水合作用和執行情況來自
  `[soracloud_runtime]` 配置和提交狀態，而不是環境
  切換。