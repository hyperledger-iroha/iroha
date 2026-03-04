---
lang: zh-hant
direction: ltr
source: docs/source/confidential_assets.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 969ffd4cee6ee4880d5f754fb36adaf30dde532a29e4c6397cf0f358438bb57e
source_last_modified: "2026-01-22T16:26:46.566038+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
SPDX-License-Identifier: Apache-2.0
-->
# 機密資產和ZK傳輸設計

## 動機
- 提供選擇加入的受保護資產流，以便域名可以在不改變透明流通的情況下保護交易隱私。
- 為審計員和操作員提供電路和加密參數的生命週期控制（激活、輪換、撤銷）。

## 威脅模型
- 驗證者誠實但好奇：他們忠實地執行共識，但嘗試檢查賬本/狀態。
- 網絡觀察者可以看到區塊數據和八卦交易；沒有私人八卦渠道的假設。
- 超出範圍：賬本外流量分析、量子對手（根據 PQ 路線圖單獨跟踪）、賬本可用性攻擊。

## 設計概述
- 除了現有的透明餘額之外，資產還可以聲明一個“屏蔽池”；屏蔽流通通過加密承諾來表示。
- 註釋將 `(asset_id, amount, recipient_view_key, blinding, rho)` 封裝為：
  - 承諾：`Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`。
  - 無效符：`Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`，與音符順序無關。
  - 加密有效負載：`enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`。
- 交易傳輸 Norito 編碼的 `ConfidentialTransfer` 有效負載，其中包含：
  - 公共輸入：Merkle 錨、無效器、新承諾、資產 ID、電路版本。
  - 接收者和可選審核員的加密有效負載。
  - 零知識證明證明價值保存、所有權和授權。
- 驗證密鑰和參數集通過帶有激活窗口的賬本註冊表進行控制；節點拒絕驗證引用未知或已撤銷條目的證明。
- 共識標頭提交到活動的機密功能摘要，因此僅當註冊表和參數狀態匹配時才接受塊。
- 證明構造使用 Halo2（Plonkish）堆棧，無需可信設置； v1 中故意不支持 Groth16 或其他 SNARK 變體。

### 確定性賽程

機密備忘錄信封現在附帶 `fixtures/confidential/encrypted_payload_v1.json` 的規範固定裝置。該數據集捕獲正的 v1 包絡和負的畸形樣本，以便 SDK 可以斷言解析奇偶校驗。 Rust 數據模型測試 (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) 和 Swift 套件 (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) 都直接加載夾具，確保 Norito 編碼、錯誤表面和回歸覆蓋隨著編解碼器的發展保持一致。

Swift SDK 現在可以發出屏蔽指令，無需定制 JSON 膠水：構造一個
`ShieldRequest` 具有 32 字節票據承諾、加密有效負載和借記元數據，
然後調用 `IrohaSDK.submit(shield:keypair:)`（或 `submitAndWait`）來簽名並轉發
交易超過 `/v1/pipeline/transactions`。助手驗證承諾長度，
將 `ConfidentialEncryptedPayload` 插入 Norito 編碼器，並鏡像 `zk::Shield`
下面描述的佈局使錢包與 Rust 保持同步。## 共識承諾和能力門控
- 塊頭公開 `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`；摘要參與共識哈希，並且必須等於本地註冊表視圖才能接受塊。
- 治理可以通過將 `next_conf_features` 編程為未來的 `activation_height` 來進行階段升級；直到這個高度，區塊生產者必須繼續發出之前的摘要。
- 驗證節點必須使用 `confidential.enabled = true` 和 `assume_valid = false` 運行。如果任一條件失敗或本地 `conf_features` 出現分歧，則啟動檢查將拒絕加入驗證器集。
- P2P 握手元數據現在包括 `{ enabled, assume_valid, conf_features }`。宣傳不受支持的功能的同行會被 `HandshakeConfidentialMismatch` 拒絕，並且永遠不會進入共識輪換。
- 非驗證者觀察者可以設置 `assume_valid = true`；他們盲目應用機密增量，但不影響共識安全。## 資產保單
- 每個資產定義都帶有由創建者或通過治理設置的 `AssetConfidentialPolicy`：
  - `TransparentOnly`：默認模式；僅允許透明指令（`MintAsset`、`TransferAsset` 等），並且拒絕屏蔽操作。
  - `ShieldedOnly`：所有發行和轉讓必須使用保密指令； `RevealConfidential` 被禁止，因此餘額永遠不會公開出現。
  - `Convertible`：持有者可以使用下面的入口/出口指令在透明和屏蔽表示之間移動價值。
- 政策遵循受限的 FSM，以防止資金擱淺：
  - `TransparentOnly → Convertible`（立即啟用屏蔽池）。
  - `TransparentOnly → ShieldedOnly`（需要掛起的轉換和轉換窗口）。
  - `Convertible → ShieldedOnly`（強制最小延遲）。
  - `ShieldedOnly → Convertible`（需要遷移計劃，以便受保護的票據仍然可以使用）。
  - `ShieldedOnly → TransparentOnly` 是不允許的，除非屏蔽池為空或者治理編碼了取消屏蔽未處理票據的遷移。
- 治理指令通過 `ScheduleConfidentialPolicyTransition` ISI 設置 `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }`，並可能使用 `CancelConfidentialPolicyTransition` 中止計劃的更改。內存池驗證確保沒有交易跨越轉換高度，並且如果策略檢查會在塊中發生更改，則包含會確定性失敗。
- 當新塊打開時，會自動應用掛起的轉換：一旦塊高度進入轉換窗口（對於 `ShieldedOnly` 升級）或達到編程的 `effective_height`，運行時將更新 `AssetConfidentialPolicy`，刷新 `zk.policy` 元數據，並清除掛起的條目。如果 `ShieldedOnly` 轉換成熟時透明供應仍然存在，則運行時將中止更改並記錄警告，使先前的模式保持不變。
- 配置旋鈕 `policy_transition_delay_blocks` 和 `policy_transition_window_blocks` 強制執行最短通知和寬限期，讓錢包在開關周圍轉換票據。
- `pending_transition.transition_id`兼作審計句柄；治理在完成或取消轉換時必須引用它，以便操作員可以關聯入口/出口報告。
- `policy_transition_window_blocks` 默認為 720（約 12 小時，60 秒區塊時間）。節點會限制嘗試更短通知的治理請求。
- Genesis 清單和 CLI 流程顯示當前和待定的政策。准入邏輯在執行時讀取策略以確認每條機密指令均已獲得授權。
- 遷移清單 — 請參閱下面的“遷移順序”，了解 Milestone M0 跟踪的分階段升級計劃。

#### 通過 Torii 監控轉換錢包和審計員輪詢 `GET /v1/confidential/assets/{definition_id}/transitions` 進行檢查
活動 `AssetConfidentialPolicy`。 JSON 有效負載始終包含規範的
資產id，最新觀察到的區塊高度，策略的`current_mode`，模式為
在該高度有效（轉換窗口暫時報告 `Convertible`），並且
預期為 `vk_set_hash`/Poseidon/Pedersen 參數標識符。 Swift SDK消費者可以調用
`ToriiClient.getConfidentialAssetPolicy` 接收與類型化 DTO 相同的數據，無需
手寫解碼。當治理過渡懸而未決時，響應還嵌入：

- `transition_id` — `ScheduleConfidentialPolicyTransition` 返回的審核句柄。
- `previous_mode`/`new_mode`。
- `effective_height`。
- `conversion_window` 和派生的 `window_open_height` （錢包必須
  開始轉換為 ShieldedOnly 切換）。

響應示例：

```json
{
  "asset_id": "rose#wonderland",
  "block_height": 4217,
  "current_mode": "Convertible",
  "effective_mode": "Convertible",
  "vk_set_hash": "8D7A4B0A95AB1C33F04944F5D332F9A829CEB10FB0D0797E2D25AEFBAAF1155D",
  "poseidon_params_id": 7,
  "pedersen_params_id": 11,
  "pending_transition": {
    "transition_id": "BF2C6F9A4E9DF389B6F7E5E6B5487B39AE00D2A4B7C0FBF2C9FEF6D0A961C8ED",
    "previous_mode": "Convertible",
    "new_mode": "ShieldedOnly",
    "effective_height": 5000,
    "conversion_window": 720,
    "window_open_height": 4280
  }
}
```

`404` 響應表示不存在匹配的資產定義。當沒有過渡時
預定的 `pending_transition` 字段是 `null`。

### 策略狀態機|當前模式|下一個模式 |先決條件 |有效高度搬運|筆記|
|--------------------------------|--------------------------------|------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------------|
|只透明|敞篷車|治理已激活驗證程序/參數註冊表項。與 `effective_height ≥ current_height + policy_transition_delay_blocks` 一起提交 `ScheduleConfidentialPolicyTransition`。 |轉換恰好在 `effective_height` 處執行；屏蔽池立即可用。                   |用於在保持透明流的同時啟用機密性的默認路徑。               |
|只透明|僅屏蔽 |同上，加上 `policy_transition_window_blocks ≥ 1`。                                                         |運行時在 `effective_height - policy_transition_window_blocks` 處自動進入 `Convertible`；在 `effective_height` 處翻轉至 `ShieldedOnly`。 |在禁用透明指令之前提供確定性轉換窗口。   |
|敞篷車|僅屏蔽 |計劃轉換為 `effective_height ≥ current_height + policy_transition_delay_blocks`。治理應通過審計元數據進行認證（`transparent_supply == 0`）；運行時在切換時強制執行此操作。 |與上面相同的窗口語義。如果透明電源在 `effective_height` 處不為零，則轉換將在 `PolicyTransitionPrerequisiteFailed` 處中止。 |將資產鎖定在完全保密的流通狀態。                                     |
|僅屏蔽 |敞篷車|預定的過渡；沒有主動緊急提款（`withdraw_height` 未設置）。                                    |狀態翻轉為 `effective_height`；顯示坡道重新打開，同時屏蔽音符仍然有效。                           |用於維護窗口或審核員審查。                                          |
|僅屏蔽 |只透明|治理必須證明 `shielded_supply == 0` 或製定已簽署的 `EmergencyUnshield` 計劃（需要審核員簽名）。 |運行時在 `effective_height` 之前打開 `Convertible` 窗口；在最高峰時，機密指令會發生故障，並且資產會返回到僅透明模式。 |最後的出口。如果在窗口期間有任何機密票據花費，過渡將自動取消。 |
|任何 |與當前相同 | `CancelConfidentialPolicyTransition` 清除掛起的更改。                                                        | `pending_transition` 立即刪除。                                                                          |維持現狀；顯示完整性。                                             |上面未列出的過渡在治理提交期間將被拒絕。運行時在應用計劃的轉換之前檢查先決條件；失敗的先決條件會將資產推回到之前的模式，並通過遙測和塊事件發出 `PolicyTransitionPrerequisiteFailed`。

### 遷移排序

2. **分階段過渡：** 提交 `ScheduleConfidentialPolicyTransition` 以及尊重 `policy_transition_delay_blocks` 的 `effective_height`。當轉向 `ShieldedOnly` 時，指定轉換窗口 (`window ≥ policy_transition_window_blocks`)。
3. **發布操作員指南：** 記錄返回的 `transition_id` 並分發進/出匝道運行手冊。錢包和審計員訂閱 `/v1/confidential/assets/{id}/transitions` 以了解窗口打開高度。
4. **窗口強制：** 當窗口打開時，運行時將策略切換到 `Convertible`，發出 `PolicyTransitionWindowOpened { transition_id }`，並開始拒絕衝突的治理請求。
5. **完成或中止：** 在 `effective_height` 處，運行時驗證轉換先決條件（零透明供應、無緊急提款等）。成功將策略翻轉到請求的模式；失敗會發出 `PolicyTransitionPrerequisiteFailed`，清除掛起的轉換，並使策略保持不變。
6. **架構升級：** 成功轉換後，治理會提高資產架構版本（例如，`asset_definition.v2`），並且 CLI 工具在序列化清單時需要 `confidential_policy`。 Genesis 升級文檔指示操作員在重新啟動驗證器之前添加策略設置和註冊表指紋。

從啟用保密性開始的新網絡直接在創世中編碼所需的策略。在發布後更改模式時，他們仍然遵循上面的清單，以便轉換窗口保持確定性，並且錢包有時間進行調整。

### Norito 清單版本控制和激活- Genesis 清單必須包含自定義 `confidential_registry_root` 密鑰的 `SetParameter`。有效負載是與 `ConfidentialRegistryMeta { vk_set_hash: Option<String> }` 匹配的 Norito JSON：當沒有活動的驗證器條目時省略該字段 (`null`)，否則提供一個 32 字節的十六進製字符串 (`0x…`)，該字符串等於 `compute_vk_set_hash` 通過清單中提供的驗證器指令生成的哈希值。如果參數丟失或哈希值與編碼的註冊表寫入不一致，節點將拒絕啟動。
- 在線 `ConfidentialFeatureDigest::conf_rules_version` 嵌入清單佈局版本。對於 v1 網絡，它必須保持 `Some(1)` 並等於 `iroha_config::parameters::defaults::confidential::RULES_VERSION`。當規則集演變時，改變常量，重新生成清單，並同步推出二進製文件；混合版本會導致驗證器拒絕帶有 `ConfidentialFeatureDigestMismatch` 的塊。
- 激活清單應該捆綁註冊表更新、參數生命週期更改和策略轉換，以便摘要保持一致：
  1. 在離線狀態視圖中應用計劃的註冊表突變（`Publish*`、`Set*Lifecycle`），並使用 `compute_confidential_feature_digest` 計算激活後摘要。
  2. 使用計算出的散列發出 `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x…"})`，以便落後的對等方即使錯過中間註冊表指令也可以恢復正確的摘要。
  3. 附加 `ScheduleConfidentialPolicyTransition` 指令。每條指令必須引用治理髮布的`transition_id`；忘記它的清單將被運行時拒絕。
  4. 保留激活計劃中使用的清單字節、SHA-256 指紋和摘要。操作員在投票使清單生效之前驗證所有三個工件，以避免分區。
- 當部署需要延遲切換時，在配套自定義參數中記錄目標高度（例如 `custom.confidential_upgrade_activation_height`）。這為審計員提供了 Norito 編碼的證據，證明驗證者在摘要更改生效之前遵守了通知窗口。## 驗證器和參數生命週期
### ZK註冊表
- Ledger 存儲 `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }`，其中 `proving_system` 目前固定為 `Halo2`。
- `(circuit_id, version)`對是全球唯一的；註冊表維護一個二級索引，用於通過電路元數據進行查找。在入院期間嘗試註冊重複的配對會被拒絕。
- `circuit_id` 必須非空，並且必須提供 `public_inputs_schema_hash`（通常是驗證者規範公共輸入編碼的 Blake2b-32 哈希值）。准入會拒絕省略這些字段的記錄。
- 治理指令包括：
  - `PUBLISH` 添加僅包含元數據的 `Proposed` 條目。
  - `ACTIVATE { vk_id, activation_height }` 在紀元邊界安排條目激活。
  - `DEPRECATE { vk_id, deprecation_height }` 標記最終高度，校樣可以參考該條目。
  - `WITHDRAW { vk_id, withdraw_height }` 用於緊急關閉；受影響的資產在提款高峰後凍結機密支出，直到新條目激活。
- Genesis 清單自動發出 `confidential_registry_root` 自定義參數，其 `vk_set_hash` 與活動條目匹配；在節點加入共識之前，驗證會根據本地註冊表狀態交叉檢查此摘要。
- 註冊或更新驗證器需要`gas_schedule_id`；驗證強制要求註冊表項為 `Active`，存在於 `(circuit_id, version)` 索引中，並且 Halo2 證明提供 `OpenVerifyEnvelope`，其 `circuit_id`、`vk_hash` 和 `public_inputs_schema_hash` 與註冊表記錄匹配。

### 證明密鑰
- 證明密鑰保留在賬本外，但由與驗證者元數據一起發布的內容尋址標識符（`pk_cid`、`pk_hash`、`pk_len`）引用。
- 錢包 SDK 獲取 PK 數據、驗證哈希值並在本地緩存。

### Pedersen 和 Poseidon 參數
- 單獨的註冊表（`PedersenParams`、`PoseidonParams`）鏡像驗證器生命週期控件，每個註冊表都有 `params_id`、生成器/常量的哈希值、激活、棄用和撤回高度。

## 確定性排序和取消器
- 每項資產均維護 `CommitmentTree` 和 `next_leaf_index`；塊按確定性順序追加承諾：按塊順序迭代交易；在每個事務中，通過升序序列化 `output_idx` 迭代屏蔽輸出。
- `note_position` 源自樹偏移量，但 **不是** 無效符的一部分；它只提供證據見證人中的成員路徑。
- PRF設計保證了重組下的無效器穩定性； PRF 輸入綁定 `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`，錨點引用受 `max_anchor_age_blocks` 限制的歷史 Merkle 根。## 賬本流程
1. **MintConfidential { asset_id, amount,recipient_hint }**
   - 需要資產保單 `Convertible` 或 `ShieldedOnly`；准入檢查資產權限，檢索當前 `params_id`，採樣 `rho`，發出承諾，更新 Merkle 樹。
   - 發出 `ConfidentialEvent::Shielded` 以及新的承諾、Merkle 根增量和審計跟踪的交易調用哈希。
2. **TransferConfidential { asset_id、proof、circle_id、version、nullifiers、new_commitments、enc_payloads、anchor_root、memo }**
   - VM 系統調用使用註冊表項驗證證據；主機確保無效符未使用，確定性附加承諾，錨點是最近的。
   - Ledger 記錄 `NullifierSet` 條目，為接收者/審計者存儲加密的有效負載，並發出 `ConfidentialEvent::Transferred` 總結無效符、有序輸出、證明哈希和 Merkle 根。
3. **RevealConfidential { asset_id、proof、circle_id、version、nullifier、amount、recipient_account、anchor_root }**
   - 僅適用於 `Convertible` 資產；證明驗證票據價值等於顯示的金額，分類賬記入透明餘額，並通過標記已用的廢紙來銷毀受保護的票據。
   - 發出 `ConfidentialEvent::Unshielded` 以及公共金額、消耗的無效符、證明標識符和交易調用哈希。

## 數據模型添加
- `ConfidentialConfig`（新配置部分），帶有啟用標誌、`assume_valid`、氣體/限制旋鈕、錨點窗口、驗證器後端。
- `ConfidentialNote`、`ConfidentialTransfer` 和 `ConfidentialMint` Norito 模式，具有顯式版本字節 (`CONFIDENTIAL_ASSET_V1 = 0x01`)。
- `ConfidentialEncryptedPayload` 使用 `{ version, ephemeral_pubkey, nonce, ciphertext }` 包裝 AEAD 備忘錄字節，對於 XChaCha20-Poly1305 佈局，默認為 `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1`。
- 規範密鑰派生向量位於 `docs/source/confidential_key_vectors.json` 中； CLI 和 Torii 端點均針對這些裝置進行回歸。支出/無效/觀看階梯的面向錢包的衍生品發佈在 `fixtures/confidential/keyset_derivation_v1.json` 中，並由 Rust + Swift SDK 測試執行，以保證跨語言平價。
- `asset::AssetDefinition` 獲得 `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`。
- `ZkAssetState` 保留 `(backend, name, commitment)` 綁定以用於傳輸/取消屏蔽驗證器；執行會拒絕引用或內聯驗證密鑰與註冊承諾不匹配的證明，並在改變狀態之前根據已解析的後端密鑰驗證傳輸/取消屏蔽證明。
- `CommitmentTree`（每個具有邊境檢查點的資產）、`NullifierSet`，由存儲在世界狀態中的 `(chain_id, asset_id, nullifier)`、`ZkVerifierEntry`、`PedersenParams`、`PoseidonParams` 鍵入。
- Mempool 維護瞬態 `NullifierIndex` 和 `AnchorIndex` 結構，用於早期重複檢測和錨點年齡檢查。
- Norito 模式更新包括公共輸入的規範排序；往返測試確保編碼確定性。
- 加密的有效負載往返通過單元測試 (`crates/iroha_data_model/src/confidential.rs`) 鎖定，上面的錢包密鑰派生向量為審計員錨定了 AEAD 信封派生。 `norito.md` 記錄信封的在線標頭。## IVM 集成和系統調用
- 引入 `VERIFY_CONFIDENTIAL_PROOF` 系統調用接受：
  - `circuit_id`、`version`、`scheme`、`public_inputs`、`proof` 以及生成的 `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }`。
  - 系統調用從註冊表加載驗證者元數據，強制執行大小/時間限制，收取確定性氣體，並且僅在證明成功時應用增量。
- 主機公開只讀 `ConfidentialLedger` 特徵，用於檢索 Merkle 根快照和無效器狀態； Kotodama 庫提供見證程序集幫助程序和架構驗證。
- 更新了 Pointer-ABI 文檔以闡明證明緩衝區佈局和註冊表句柄。

## 節點能力協商
- Handshake 將 `feature_bits.confidential` 與 `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }` 一起進行廣告。驗證者參與需要 `confidential.enabled=true`、`assume_valid=false`、相同的驗證者後端標識符和匹配的摘要；不匹配導致與 `HandshakeConfidentialMismatch` 的握手失敗。
- 配置僅支持觀察者節點的 `assume_valid`：禁用時，遇到機密指令會產生確定性 `UnsupportedInstruction`，而不會出現恐慌；啟用後，觀察者應用聲明的狀態增量而不驗證證明。
- 如果禁用本地功能，Mempool 將拒絕機密交易。八卦過濾器避免向沒有匹配能力的對等方發送屏蔽交易，同時在大小限制內盲目轉發未知驗證者 ID。

### 揭示修剪和無效保留政策

機密賬本必須保留足夠的歷史記錄以證明票據的新鮮度並
重播治理驅動的審計。默認策略，由
`ConfidentialLedger`，是：

- **無效化保留：**保留用過的無效化*最少* `730` 天（24
  幾個月）後度過高度，或監管機構規定的窗口（如果更長）。
  運營商可以通過 `confidential.retention.nullifier_days` 擴展窗口。
  比保留窗口年輕的無效符必須保持可通過 Torii 進行查詢，因此
  審計員可以證明雙花缺席。
- **顯示修剪：**透明顯示（`RevealConfidential`）修剪
  區塊最終確定後立即進行相關票據承諾，但
  消耗的無效符仍受上述保留規則的約束。揭示相關
  事件（`ConfidentialEvent::Unshielded`）記錄公眾金額、接收者、
  和證明哈希，因此重建歷史揭示不需要修剪
  密文。
- **邊境檢查點：**承諾邊境維持滾動檢查點
  覆蓋 `max_anchor_age_blocks` 和保留窗口中較大的一個。節點
  僅在間隔內的所有無效符到期後才壓縮較舊的檢查點。
- **過時摘要修復：** 如果 `HandshakeConfidentialMismatch` 到期
  為了消化漂移，操作員應該 (1) 驗證無效器保留窗口
  跨集群對齊，(2) 運行 `iroha_cli app confidential verify-ledger` 以
  針對保留的無效集重新生成摘要，並且 (3) 重新部署
  刷新清單。任何過早修剪的無效符都必須從
  在重新加入網絡之前進行冷存儲。在操作手冊中記錄本地覆蓋；治理政策延伸
保留窗口必須更新節點配置和歸檔存儲計劃
步調一致。

### 驅逐和恢復流程

1. 在撥號過程中，`IrohaNetwork` 會比較公佈的功能。任何不匹配都會引發 `HandshakeConfidentialMismatch`；連接關閉，對等點保留在發現隊列中，而不會提升為 `Ready`。
2. 故障通過網絡服務日誌（包括遠程摘要和後端）顯示，並且 Sumeragi 從未安排對等點進行提案或投票。
3. 操作員通過調整驗證者註冊表和參數集（`vk_set_hash`、`pedersen_params_id`、`poseidon_params_id`）或通過將 `next_conf_features` 與商定的 `activation_height` 暫存來進行修復。一旦摘要匹配，下一次握手就會自動成功。
4. 如果過時的對等點設法廣播一個塊（例如，通過存檔重播），驗證器將使用 `BlockRejectionReason::ConfidentialFeatureDigestMismatch` 確定性地拒絕它，從而保持整個網絡的賬本狀態一致。

### 重放安全握手流程

1. 每次出站嘗試都會分配新的 Noise/X25519 密鑰材料。簽名的握手有效負載 (`handshake_signature_payload`) 連接本地和遠程臨時公鑰、Norito 編碼的通告套接字地址，以及使用 `handshake_chain_id` 編譯時的鏈標識符。消息在離開節點之前經過 AEAD 加密。
2. 響應方以相反的對等/本地密鑰順序重新計算有效負載，並驗證嵌入在 `HandshakeHelloV1` 中的 Ed25519 簽名。由於臨時密鑰和通告的地址都是簽名域的一部分，因此針對另一個對等點重放捕獲的消息或恢復過時的連接會導致驗證失敗。
3. 機密功能標誌和 `ConfidentialFeatureDigest` 位於 `HandshakeConfidentialMeta` 內部。接收器將元組 `{ enabled, assume_valid, verifier_backend, digest }` 與其本地配置的 `ConfidentialHandshakeCaps` 進行比較；在傳輸轉換為 `Ready` 之前，任何不匹配都會提前退出 `HandshakeConfidentialMismatch`。
4. 操作員必須重新計算摘要（通過 `compute_confidential_feature_digest`）並在重新連接之前使用更新的註冊表/策略重新啟動節點。宣傳舊摘要的節點繼續使握手失敗，從而防止過時狀態重新進入驗證器集。
5. 握手成功和失敗會更新標準 `iroha_p2p::peer` 計數器（`handshake_failure_count`，錯誤分類助手），並發出標有遠程對等 ID 和摘要指紋的結構化日誌條目。監視這些指示器以捕獲部署期間的重放嘗試或錯誤配置。## 密鑰管理和有效負載
- 每個帳戶的密鑰派生層次結構：
  - `sk_spend` → `nk`（無效鍵）、`ivk`（傳入查看鍵）、`ovk`（傳出查看鍵）、`fvk`。
- 加密的票據有效負載使用 AEAD 和 ECDH 派生的共享密鑰；可選的審計員查看鍵可以附加到每個資產策略的輸出。
- CLI 添加：`confidential create-keys`、`confidential send`、`confidential export-view-key`、用於解密備忘錄的審計工具，以及用於離線生成/檢查 Norito 備忘錄信封的 `iroha app zk envelope` 幫助程序。 Torii 通過 `POST /v1/confidential/derive-keyset` 公開相同的派生流程，返回十六進制和 base64 形式，以便錢包可以以編程方式獲取密鑰層次結構。

## Gas、限制和 DoS 控制
- 確定性氣體調度：
  - Halo2（Plonkish）：每個公共輸入的基礎 `250_000` 氣體 + `2_000` 氣體。
  - `5` 每個證明字節的 Gas 費用，加上每個無效器 (`300`) 和每個承諾 (`500`) 費用。
  - 操作員可以通過節點配置覆蓋這些常量（`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`）；更改在啟動時或配置層熱重載時傳播，並確定性地應用於整個集群。
- 硬限制（可配置的默認值）：
- `max_proof_size_bytes = 262_144`。
- `max_nullifiers_per_tx = 8`、`max_commitments_per_tx = 8`、`max_confidential_ops_per_block = 256`。
- `verify_timeout_ms = 750`、`max_anchor_age_blocks = 10_000`。超過 `verify_timeout_ms` 的證明會確定性地中止指令（治理選票發出 `proof verification exceeded timeout`，`VerifyProof` 返回錯誤）。
- 額外配額確保活性：`max_proof_bytes_block`、`max_verify_calls_per_tx`、`max_verify_calls_per_block` 和 `max_public_inputs` 綁定塊構建器； `reorg_depth_bound` (≥ `max_anchor_age_blocks`) 管理邊境檢查點保留。
- 運行時執行現在會拒絕超出這些每筆交易或每塊限制的交易，發出確定性 `InvalidParameter` 錯誤並使賬本狀態保持不變。
- Mempool 在調用驗證器之前通過 `vk_id`、證明長度和錨年齡預過濾機密交易以限制資源使用。
- 驗證在超時或違反約束時確定性停止；事務因顯式錯誤而失敗。 SIMD 後端是可選的，但不會改變 Gas 核算。

### 校準基線和驗收門
- **參考平台。 ** 校準運行必須涵蓋以下三個硬件配置文件。未能捕獲所有配置文件的運行在審核期間將被拒絕。|簡介 |建築| CPU/實例|編譯器標誌 |目的|
  | ---| ---| ---| ---| ---|
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) 或 Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` |無需向量內在函數即可建立下限值；用於調整後備成本表。 |
  | `baseline-avx2` | `x86_64` |英特爾至強金牌 6430 (24c) |默認發布 |驗證 AVX2 路徑；檢查 SIMD 加速是否保持在中性氣體的耐受範圍內。 |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | AWS Graviton3 (c7g.4xlarge) | AWS Graviton3 (c7g.4xlarge)默認發布 |確保 NEON 後端保持確定性並與 x86 計劃保持一致。 |

- **基準線束。 ** 所有氣體校準報告必須包含以下內容：
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` 確認確定性夾具。
  - 每當 VM 操作碼成本發生變化時，`CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`。

- **修復了隨機性。 ** 在運行工作台之前導出 `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1`，以便 `iroha_test_samples::gen_account_in` 切換到確定性 `KeyPair::from_seed` 路徑。線束打印一次`IROHA_CONF_GAS_SEED_ACTIVE=…`；如果變量丟失，審核必須失敗。任何新的校準實用程序在引入輔助隨機性時都必須繼續遵守此環境變量。

- **結果捕獲。 **
  - 將每個配置文件的標準摘要 (`target/criterion/**/raw.csv`) 上傳到發布工件中。
  - 將派生指標（`ns/op`、`gas/op`、`ns/gas`）以及使用的 git 提交和編譯器版本存儲在 `docs/source/confidential_assets_calibration.md` 中。
  - 維護每個配置文件的最後兩條基線；一旦最新的報告得到驗證，就刪除舊的快照。

- **驗收公差。 **
  - `baseline-simd-neutral` 和 `baseline-avx2` 之間的氣體增量必須保持 ≤ ±1.5%。
  - `baseline-simd-neutral` 和 `baseline-neon` 之間的氣體增量必須保持 ≤ ±2.0%。
  - 超過這些閾值的校準建議需要調整時間表或使用 RFC 來解釋差異和緩解措施。

- **審核清單。 ** 提交者負責：
  - 包括校準日誌中的 `uname -a`、`/proc/cpuinfo` 摘錄（模型、步進）和 `rustc -Vv`。
  - 驗證工作台輸出中回顯的 `IROHA_CONF_GAS_SEED`（工作台打印活動種子）。
  - 確保起搏器和機密驗證器功能標誌鏡像生產（使用遙測運行工作台時為 `--features confidential,telemetry`）。

## 配置和操作
- `iroha_config` 獲得 `[confidential]` 部分：
  ```toml
  [confidential]
  enabled = true
  assume_valid = false
  verifier_backend = "ark_bls12_381"
  max_proof_size_bytes = 262144
  max_nullifiers_per_tx = 8
  max_commitments_per_tx = 8
  max_confidential_ops_per_block = 256
  verify_timeout_ms = 750
  max_anchor_age_blocks = 10000
  max_proof_bytes_block = 1048576
  max_verify_calls_per_tx = 4
  max_verify_calls_per_block = 128
  max_public_inputs = 32
  reorg_depth_bound = 10000
  policy_transition_delay_blocks = 100
  policy_transition_window_blocks = 200
  tree_roots_history_len = 10000
  tree_frontier_checkpoint_interval = 100
  registry_max_vk_entries = 64
  registry_max_params_entries = 32
  registry_max_delta_per_block = 4
  ```
- 遙測發出聚合指標：`confidential_proof_verified`、`confidential_verifier_latency_ms`、`confidential_proof_bytes_total`、`confidential_nullifier_spent`、`confidential_commitments_appended`、`confidential_mempool_rejected_total{reason}` 和 `confidential_policy_transitions_total`，從不暴露明文數據。
- RPC 表面：
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`## 測試策略
- 確定性：區塊內的隨機交易洗牌會產生相同的默克爾根和無效集。
- 重組彈性：用錨點模擬多塊重組；無效器保持穩定，陳舊的錨被拒絕。
- Gas 不變量：驗證有或沒有 SIMD 加速的節點之間相同的 Gas 使用情況。
- 邊界測試：大小/gas 上限的證明、最大輸入/輸出計數、超時執行。
- 生命週期：驗證者和參數激活/棄用的治理操作、輪換支出測試。
- 政策 FSM：允許/不允許的轉換、待處理的轉換延遲以及有效高度附近的內存池拒絕。
- 註冊緊急情況：緊急提款將凍結受影響的資產 `withdraw_height`，並隨後拒絕證明。
- 能力門控：具有不匹配的 `conf_features` 拒絕塊的驗證器； `assume_valid=true` 的觀察者可以跟上而不影響共識。
- 狀態等效：驗證者/完整/觀察者節點在規範鏈上產生相同的狀態根。
- 負模糊測試：格式錯誤的證明、過大的有效負載和無效衝突確定性地拒絕。

## 傑出作品
- 對 Halo2 參數集（電路大小、查找策略）進行基準測試，並將結果記錄在校準手冊中，以便氣體/超時默認值可以在下一次 `confidential_assets_calibration.md` 刷新時進行更新。
- 最終確定審計師披露政策和相關的選擇性查看 API，一旦治理草案簽署，將批准的工作流程連接到 Torii 中。
- 擴展見證加密方案以涵蓋多接收者輸出和批量備忘錄，為 SDK 實施者記錄信封格式。
- 委託對電路、註冊表和參數輪換程序進行外部安全審查，並將結果存檔在內部審計報告旁邊。
- 指定審計員支出調節 API 並發布視圖密鑰範圍指南，以便錢包供應商可以實現相同的證明語義。## 實施階段
1. **階段 M0 — 停船強化**
   - ✅ 無效器推導現在遵循 Poseidon PRF 設計（`nk`、`rho`、`asset_id`、`chain_id`），並在賬本更新中強制執行確定性承諾排序。
   - ✅ 執行強制執行證明大小上限和每筆交易/每塊機密配額，拒絕具有確定性錯誤的超出預算的交易。
   - ✅ P2P 握手通告 `ConfidentialFeatureDigest`（後端摘要 + 註冊表指紋），並通過 `HandshakeConfidentialMismatch` 確定性地失敗不匹配。
   - ✅ 消除機密執行路徑中的恐慌，並為沒有匹配能力的節點添加角色門控。
   - ⚪ 強制執行驗證者超時預算和邊境檢查點的重組深度限制。
     - ✅ 執行驗證超時預算；超過 `verify_timeout_ms` 的證明現在確定性地失敗。
     - ✅ 前沿檢查點現在遵循 `reorg_depth_bound`，修剪早於配置窗口的檢查點，同時保持確定性快照。
   - 引入 `AssetConfidentialPolicy`、策略 FSM 和鑄造/轉移/顯示指令的執行門。
   - 在塊頭中提交 `conf_features` 並在註冊表/參數摘要出現分歧時拒絕驗證者參與。
2. **階段 M1 — 註冊表和參數**
   - 通過治理操作、創世錨定和緩存管理登陸 `ZkVerifierEntry`、`PedersenParams` 和 `PoseidonParams` 註冊表。
   - 連接系統調用以要求註冊表查找、gas Schedule ID、模式散列和大小檢查。
   - 提供加密有效負載格式 v1、錢包密鑰派生向量以及用於機密密鑰管理的 CLI 支持。
3. **M2 階段 — 氣體與性能**
   - 通過遙測技術實施確定性的 Gas Schedule、每塊計數器和基準測試工具（驗證延遲、證明大小、內存池拒絕）。
   - 強化多資產工作負載的 CommitmentTree 檢查點、LRU 加載和無效索引。
4. **M3 階段 — 輪換和錢包工具**
   - 實現多參數、多版本證明驗收；通過過渡運行手冊支持治理驅動的激活/棄用。
   - 提供錢包 SDK/CLI 遷移流程、審核員掃描工作流程和支出核對工具。
5. **M4 階段 — 審計和運營**
   - 提供審核員關鍵工作流程、選擇性披露 API 和操作手冊。
   - 安排外部加密/安全審查並在 `status.md` 中發布調查結果。

每個階段都會更新路線圖里程碑和相關測試，以維持區塊鍊網絡的確定性執行保證。

### SDK & Fixture Coverage (Phase M1)

加密有效負載 v1 現在附帶規範固定裝置，因此每個 SDK 都會生成
相同的 Norito 信封和交易哈希。 The golden artefacts live in
`fixtures/confidential/wallet_flows_v1.json` and are exercised directly by the
Rust and Swift suites (`crates/iroha_data_model/tests/confidential_wallet_fixtures.rs`,
`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialWalletFixturesTests.swift`）：

```bash
# Rust parity (verifies the signed hex + hash for every case)
cargo test -p iroha_data_model confidential_wallet_fixtures

# Swift parity (builds the same envelopes via TxBuilder/NativeBridge)
cd IrohaSwift && swift test --filter ConfidentialWalletFixturesTests
```每個裝置都會記錄案例標識符、簽名交易十六進制和預期
哈希。當 Swift 編碼器還無法生成案例時 — `zk-transfer-basic`
仍然由 `ZkTransfer` 構建器控制 - 測試套件發出 `XCTSkip`，因此
路線圖清楚地跟踪哪些流仍需要綁定。更新夾具
文件而不改變格式版本將使兩個套件都失敗，從而保留 SDK
以及鎖步中的 Rust 參考實現。

#### 快速構建器
`TxBuilder` 為每個公開異步和基於回調的幫助程序
保密請求 (`IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:1183`)。
建設者依賴 `connect_norito_bridge` 出口
（`crates/connect_norito_bridge/src/lib.rs:3337`，
`IrohaSwift/Sources/IrohaSwift/NativeBridge.swift:1014`) 所以生成的
有效負載與 Rust 主機編碼器逐字節匹配。示例：

```swift
let account = AccountId.make(publicKey: keypair.publicKey, domain: "wonderland")
let request = RegisterZkAssetRequest(
    chainId: chainId,
    authority: account,
    assetDefinitionId: "rose#wonderland",
    zkParameters: myZkParams,
    ttlMs: 60_000
)
let envelope = try TxBuilder(client: client)
    .buildRegisterZkAsset(request: request, keypair: keypair)
try await TxBuilder(client: client)
    .submit(registerZkAsset: request, keypair: keypair)
```

屏蔽/非屏蔽遵循相同的模式（`submit(shield:)`，
`submit(unshield:)`），並且 Swift 夾具測試重新運行構建器
確定性密鑰材料，以保證生成的交易哈希值保持不變
等於 `wallet_flows_v1.json` 中存儲的值。

#### JavaScript 構建器
JavaScript SDK 通過導出的事務助手鏡像相同的流程
來自 `javascript/iroha_js/src/transaction.js`。建築商如
`buildRegisterZkAssetTransaction` 和 `buildRegisterZkAssetInstruction`
(`javascript/iroha_js/src/instructionBuilders.js:1832`) 標準化驗證密鑰
標識符並發出 Rust 主機可以接受的 Norito 有效負載，無需任何
適配器。示例：

```js
import {
  buildRegisterZkAssetTransaction,
  signTransaction,
  ToriiClient,
} from "@hyperledger/iroha";

const unsigned = buildRegisterZkAssetTransaction({
  registration: {
    authority: "ih58...",
    assetDefinitionId: "rose#wonderland",
    zkParameters: {
      commit_params: "vk_shield",
      reveal_params: "vk_unshield",
    },
    metadata: { displayName: "Rose (Shielded)" },
  },
  chainId: "00000000-0000-0000-0000-000000000000",
});
const signed = signTransaction(unsigned, myKeypair);
await new ToriiClient({ baseUrl: "https://torii" }).submitTransaction(signed);
```

屏蔽、轉移和取消屏蔽構建器遵循相同的模式，給 JS
呼叫者與 Swift 和 Rust 具有相同的人體工程學設計。測試下
`javascript/iroha_js/test/transactionBuilder.test.js` 覆蓋歸一化
邏輯，而上面的裝置保持簽名的交易字節一致。

### 遙測和監控（M2 階段）

M2 階段現在直接通過 Prometheus 和 Grafana 導出 CommitmentTree 運行狀況：

- `iroha_confidential_tree_commitments`、`iroha_confidential_tree_depth`、`iroha_confidential_root_history_entries` 和 `iroha_confidential_frontier_checkpoints` 公開每個資產的實時 Merkle 前沿，而 `iroha_confidential_root_evictions_total` / `iroha_confidential_frontier_evictions_total` 計算 `zk.root_history_cap` 強制執行的 LRU 修剪和檢查點深度窗口。
- `iroha_confidential_frontier_last_checkpoint_height` 和 `iroha_confidential_frontier_last_checkpoint_commitments` 發布最新邊界檢查點的高度 + 承諾計數，因此重組鑽探和回滾可以證明檢查點前進並保留預期的有效負載量。
- Grafana 板 (`dashboards/grafana/confidential_assets.json`) 包括深度系列、驅逐率面板和現有的驗證器緩存小部件，因此操作員可以證明 CommitmentTree 深度即使在檢查點攪動時也不會崩潰。
- 一旦遵守承諾，但報告的深度在五分鐘內保持為零，則發出警報 `ConfidentialTreeDepthZero`（在 `dashboards/alerts/confidential_assets_rules.yml` 中）跳閘。

您可以在連接 Grafana 之前在本地驗證指標：

```bash
curl -s http://127.0.0.1:8180/metrics \
  | rg 'iroha_confidential_(tree_(commitments|depth)|root_history_entries|frontier_(checkpoints|last_checkpoint_height|last_checkpoint_commitments)|root_evictions_total|frontier_evictions_total){asset_id="xor#wonderland"}'
```

將其與同一刮擦上的 `rg 'iroha_confidential_tree_depth'` 配對，以確認深度隨著新的承諾而增長，而逐出計數器僅在歷史上限修剪條目時增加。這些值必須與您附加到治理證據包的 Grafana 儀表板導出一致。

#### 燃氣表遙測和警報M2 階段還將可配置的 Gas 乘數連接到遙測管道中，以便操作員可以在批准發布之前證明每個驗證器共享相同的驗證成本：

- `iroha_confidential_gas_base_verify` 鏡像 `confidential.gas.proof_base`（默認 `250_000`）。
- `iroha_confidential_gas_per_public_input`、`iroha_confidential_gas_per_proof_byte`、`iroha_confidential_gas_per_nullifier` 和 `iroha_confidential_gas_per_commitment` 在 `ConfidentialConfig` 中鏡像其各自的旋鈕。值在啟動時以及配置熱重載時更新； `irohad` (`crates/irohad/src/main.rs:1591,1642`) 通過 `Telemetry::set_confidential_gas_schedule` 推送活動計劃。

刮擦 CommitmentTree 指標旁邊的儀表，以確認同行之間的旋鈕是相同的：

```bash
# compare active multipliers across validators
for host in validator-a validator-b validator-c; do
  curl -s "http://$host:8180/metrics" \
    | rg 'iroha_confidential_gas_(base_verify|per_public_input|per_proof_byte|per_nullifier|per_commitment)'
done
```

Grafana 儀表板 `confidential_assets.json` 現在包括一個“Gas Schedule”面板，可呈現五個儀表並突出顯示差異。 `dashboards/alerts/confidential_assets_rules.yml` 中的警報規則涵蓋：
- `ConfidentialGasMismatch`：當任何偏差超過 3 分鐘時，檢查所有抓取目標和頁面中每個乘數的最大/最小值，提示操作員通過熱重載或重新部署來對齊 `confidential.gas`。
- `ConfidentialGasTelemetryMissing`：當 Prometheus 在 5 分鐘內無法抓取五個乘數中的任何一個時發出警告，表示缺少抓取目標或禁用遙測。

請隨身攜帶以下 PromQL，以便隨時進行隨叫隨到的調查：

```promql
# ensure every multiplier matches across validators (uses the same projection as the alert)
(max without(instance, job) (iroha_confidential_gas_per_public_input)
  - min without(instance, job) (iroha_confidential_gas_per_public_input)) == 0
```

在受控配置推出之外，偏差應保持為零。更改氣體表時，捕獲刮擦之前/之後的數據，將其附加到更改請求，並使用新的乘數更新 `docs/source/confidential_assets_calibration.md`，以便治理審核人員可以將遙測證據鏈接到校準報告。