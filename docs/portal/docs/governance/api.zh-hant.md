---
lang: zh-hant
direction: ltr
source: docs/portal/docs/governance/api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 14dbd50875bebd8d9f8c9f03f85cb458c909c9da956a7a7048c9dae8c885b969
source_last_modified: "2026-01-22T16:26:46.496232+00:00"
translation_last_reviewed: 2026-02-07
title: Governance App API — Endpoints (Draft)
translator: machine-google-reviewed
---

狀態：伴隨治理實施任務的草案/草圖。在實施過程中形狀可能會發生變化。決定論和RBAC政策是規範約束；當提供 `authority` 和 `private_key` 時，Torii 可以簽署/提交交易，否則客戶端構建並提交到 `/transaction`。

概述
- 所有端點都返回 JSON。對於交易生成流程，響應包括 `tx_instructions` — 一個或多個指令骨架的數組：
  - `wire_id`：指令類型的註冊表標識符
  - `payload_hex`：Norito 有效負載字節（十六進制）
- 如果提供了 `authority` 和 `private_key`（或選票 DTO 上的 `private_key`），則 Torii 簽署並提交交易，但仍返回 `tx_instructions`。
- 否則，客戶端使用其權限和 chain_id 組裝 SignedTransaction，然後簽名並 POST 到 `/transaction`。
- SDK覆蓋範圍：
- Python（`iroha_python`）：`ToriiClient.get_governance_proposal_typed`返回`GovernanceProposalResult`（標準化狀態/種類字段），`ToriiClient.get_governance_referendum_typed`返回`GovernanceReferendumResult`，`ToriiClient.get_governance_tally_typed`返回`GovernanceTally`， `ToriiClient.get_governance_locks_typed` 返回 `GovernanceLocksResult`，`ToriiClient.get_governance_unlock_stats_typed` 返回 `GovernanceUnlockStats`，`ToriiClient.list_governance_instances_typed` 返回 `GovernanceInstancesPage`，通過 README 使用示例在治理表面上強制執行類型化訪問。
- Python 輕量級客戶端 (`iroha_torii_client`)：`ToriiClient.finalize_referendum` 和 `ToriiClient.enact_proposal` 返回類型化的 `GovernanceInstructionDraft` 捆綁包（包裝 Torii 骨架 `tx_instructions`），避免在腳本組成 Finalize/Enact 流時進行手動 JSON 解析。
- JavaScript (`@iroha/iroha-js`)：`ToriiClient` 表面鍵入了提案、公投、統計、鎖定、解鎖統計數據的幫助程序，現在 `listGovernanceInstances(namespace, options)` 加上理事會端點（`getGovernanceCouncilCurrent`、`governanceDeriveCouncilVrf`、`governancePersistCouncil`、 `getGovernanceCouncilAudit`），因此 Node.js 客戶端可以對 `/v1/gov/instances/{ns}` 進行分頁，並在現有合約實例列表旁邊驅動 VRF 支持的工作流程。

端點

- 後 `/v1/gov/proposals/deploy-contract`
  - 請求（JSON）：
    {
      “命名空間”：“應用程序”，
      "contract_id": "my.contract.v1",
      “code_hash”：“blake2b32：…”| “…64十六進制”，
      "abi_hash": "blake2b32:…" | “…64十六進制”，
      “abi_版本”：“1”，
      “窗口”：{“下”：12345，“上”：12400}，
      "authority": "i105…?",
      “私鑰”：“……？”
    }
  - 響應（JSON）：
    { "ok": true, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - 驗證：節點將 `abi_hash` 規範化為提供的 `abi_version` 並拒絕不匹配。對於 `abi_version = "v1"`，預期值為 `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`。

合約 API（部署）
- 後 `/v1/contracts/deploy`
  - 請求：{ "authority": "i105...", "private_key": "...", "code_b64": "..." }
  - 行為：從 IVM 程序體計算 `code_hash`，從標頭 `abi_version` 計算 `abi_hash`，然後提交 `RegisterSmartContractCode`（清單）和 `RegisterSmartContractBytes`（完整的 `.to`）字節）代表`authority`。
  - 響應：{“ok”：true，“code_hash_hex”：“…”，“abi_hash_hex”：“…”}
  - 相關：
    - GET `/v1/contracts/code/{code_hash}` → 返回存儲的清單
    - 獲取 `/v1/contracts/code-bytes/{code_hash}` → 返回 `{ code_b64 }`
- 後 `/v1/contracts/instance`
  - 請求：{ "authority": "i105...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - 行為：部署提供的字節碼並立即通過 `ActivateContractInstance` 激活 `(namespace, contract_id)` 映射。
  - 響應：{“ok”：true，“namespace”：“apps”，“contract_id”：“calc.v1”，“code_hash_hex”：“…”，“abi_hash_hex”：“…”}

別名服務
- 後 `/v1/aliases/voprf/evaluate`
  - 請求：{“blinded_element_hex”：“…”}
  - 響應：{“evaluated_element_hex”：“…128hex”，“後端”：“blake2b512-mock”}
    - `backend` 反映了評估器的實現。當前值：`blake2b512-mock`。
  - 註釋：確定性模擬評估器應用帶有域分離 `iroha.alias.voprf.mock.v1` 的 Blake2b512。用於測試工具，直到生產 VOPRF 管道通過 Iroha 連接。
  - 錯誤：十六進制輸入格式錯誤時出現 HTTP `400`。 Torii 返回包含解碼器錯誤消息的 Norito `ValidationFail::QueryFailed::Conversion` 信封。
- 後 `/v1/aliases/resolve`
  - 請求: { "alias": "GB82 WEST 1234 5698 7654 32" }
  - 響應: { "alias": "GB82WEST12345698765432", "account_id": "i105...", "index": 0, "source": "iso_bridge" }
  - 注意：需要 ISO 橋運行時分段（`[iso_bridge.account_aliases]` 中的 `iroha_config`）。 Torii 通過在查找之前去除空格和大寫字母來標準化別名。當別名不存在時返回 404，當 ISO 橋接運行時被禁用時返回 503。
- 後 `/v1/aliases/resolve_index`
  - 請求：{“索引”：0}
  - 響應: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "i105...", "source": "iso_bridge" }
  - 注意：別名索引是根據配置順序（從 0 開始）確定性分配的。客戶端可以離線緩存響應，以構建別名證明事件的審計跟踪。

代碼 尺寸上限
- 自定義參數：`max_contract_code_bytes` (JSON u64)
  - 控制鏈上合約代碼存儲的最大允許大小（以字節為單位）。
  - 默認：16 MiB。當 `.to` 圖像長度超過上限並出現不變違規錯誤時，節點會拒絕 `RegisterSmartContractBytes`。
  - 運營商可以通過提交 `SetParameter(Custom)` 和 `id = "max_contract_code_bytes"` 以及數字有效負載來進行調整。

- 後 `/v1/gov/ballots/zk`
  - 請求：{ "authority": "i105...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - 響應：{“ok”：true，“accepted”：true，“tx_instructions”：[{…}]}
  - 注意事項：
    - 當電路的公共輸入包括 `owner`、`amount` 和 `duration_blocks`，並且證明根據配置的 VK 進行驗證時，節點使用 `owner` 創建或擴展 `election_id` 的治理鎖。方向保持隱藏（`unknown`）；僅更新金額/到期日。重新投票是單調的：金額和到期日僅增加（節點應用 max(amount, prev.amount) 和 max(expiry, prev.expiry)）。
    - 嘗試縮減金額或到期的 ZK 重新投票會被服務器端拒絕，並帶有 `BallotRejected` 診斷。
    - 合約執行必須在排隊 `SubmitBallot` 之前調用 `ZK_VOTE_VERIFY_BALLOT`；主機強制執行一次性鎖存。

- 後 `/v1/gov/ballots/plain`
  - 請求：{ "authority": "i105...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "i105...", "amount": "1000", "duration_blocks": 6000, "direction": "Aye|Nay|Abstain" }
  - 響應：{“ok”：true，“accepted”：true，“tx_instructions”：[{…}]}
  - 注意：重新投票只能延長——新的投票不能減少現有鎖定的數量或到期時間。 `owner`必須等於交易權限。最短持續時間為 `conviction_step_blocks`。

- 後 `/v1/gov/finalize`
  - 請求：{“referendum_id”：“r1”，“proposal_id”：“…64hex”，“authority”：“i105…？”，“private_key”：“…？” }
  - 響應：{ "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - 鏈上效應（當前腳手架）：制定已批准的部署提案會插入由 `code_hash` 鍵入的最小 `ContractManifest` 和預期的 `abi_hash`，並將提案標記為已實施。如果 `code_hash` 的清單已存在且具有不同的 `abi_hash`，則頒布將被拒絕。
  - 注意事項：
    - 對於ZK選舉，合約路徑必須在執行`FinalizeElection`之前調用`ZK_VOTE_VERIFY_TALLY`；主機強制執行一次性鎖存。 `FinalizeReferendum` 在選舉計票最終確定之前拒絕 ZK 公投。
    - 在 `h_end` 處自動關閉僅針對普通公投發出批准/拒絕； ZK 公投保持關閉狀態，直到提交最終計票並執行 `FinalizeReferendum`。
    - 投票率檢查僅使用批准+拒絕；棄權不計入投票率。

- 後 `/v1/gov/enact`
  - 請求：{“proposal_id”：“…64hex”，“preimage_hash”：“…64hex？”，“window”：{“lower”：0，“upper”：0}？ ，“authority”：“i105…？”，“private_key”：“…？” }
  - 響應：{ "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - 注：Torii在提供`authority`/`private_key`時提交簽名交易；否則，它會返回一個框架供客戶簽名和提交。原像是可選的並且當前是信息性的。

- 獲取 `/v1/gov/proposals/{id}`
  - 路徑 `{id}`：提案 ID 十六進制（64 個字符）
  - 響應：{“找到”：布爾，“建議”：{…}？ }

- 獲取 `/v1/gov/locks/{rid}`
  - 路徑 `{rid}`：公投 ID 字符串
  - 響應：{“found”：bool，“referendum_id”：“rid”，“locks”：{…}？ }

- 獲取 `/v1/gov/council/current`
  - 響應：{ "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - 註釋：返回存在的持久理事會；否則使用配置的權益資產和閾值得出確定性後備（鏡像 VRF 規範，直到實時 VRF 證明保留在鏈上）。- POST `/v1/gov/council/derive-vrf`（功能：gov_vrf）
  - 請求：{“committee_size”：21，“epoch”：123？ , "candidates": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - 行為：根據源自 `chain_id`、`epoch` 和最新區塊哈希信標的規範輸入驗證每個候選者的 VRF 證明；按輸出字節 desc 進行排序；返回頂部 `committee_size` 成員。不堅持。
  - 響應：{“epoch”：N，“members”：[{“account_id”：“…”} …]，“total_candidates”：M，“verified”：K }
  - 注：Normal = G1 中的 pk，G2 中的證明（96 字節）。小 = G2 中的 pk，G1 中的證明（48 字節）。輸入是域分隔的，包括 `chain_id`。

### 治理默認值 (iroha_config `gov.*`)

當不存在持久名冊時，Torii 使用的理事會後備通過 `iroha_config` 進行參數化：

```toml
[gov]
  vk_ballot.backend = "halo2/ipa"
  vk_ballot.name    = "ballot_v1"
  vk_tally.backend  = "halo2/ipa"
  vk_tally.name     = "tally_v1"
  plain_voting_enabled = false
  conviction_step_blocks = 100
  max_conviction = 6
  approval_q_num = 1
  approval_q_den = 2
  min_turnout = 0
  parliament_committee_size = 21
  parliament_term_blocks = 43200
  parliament_min_stake = 1
  parliament_eligibility_asset_id = "SORA#stake"
```

等效環境覆蓋：

```
GOV_VK_BACKEND=halo2/ipa
GOV_VK_NAME=ballot_v1
GOV_PARLIAMENT_COMMITTEE_SIZE=21
GOV_PARLIAMENT_TERM_BLOCKS=43200
GOV_PARLIAMENT_MIN_STAKE=1
GOV_PARLIAMENT_ELIGIBILITY_ASSET_ID=SORA#stake
GOV_ALIAS_TEU_MINIMUM=0
GOV_ALIAS_FRONTIER_TELEMETRY=true
```

`parliament_committee_size` 限制沒有持續存在理事會時返回的後備成員的數量，`parliament_term_blocks` 定義用於種子派生的紀元長度 (`epoch = floor(height / term_blocks)`)，`parliament_min_stake` 強制執行資格資產的最小權益（以最小單位），`parliament_eligibility_asset_id`選擇構建候選集時掃描的資產餘額。

治理 VK 驗證無法繞過：選票驗證始終需要具有內聯字節的 `Active` 驗證密鑰，並且環境不得依賴僅測試切換來跳過驗證。

RBAC
- 鏈上執行需要權限：
  - 提案：`CanProposeContractDeployment{ contract_id }`
  - 選票：`CanSubmitGovernanceBallot{ referendum_id }`
  - 頒布：`CanEnactGovernance`
  - 理事會管理（未來）：`CanManageParliament`

受保護的命名空間
- 自定義參數 `gov_protected_namespaces`（JSON 字符串數組）啟用部署到列出的命名空間的准入門控。
- 客戶端必須包含事務元數據密鑰才能針對受保護的命名空間進行部署：
  - `gov_namespace`：目標命名空間（例如，`"apps"`）
  - `gov_contract_id`：命名空間內的邏輯合約ID
- `gov_manifest_approvers`：驗證者帳戶 ID 的可選 JSON 數組。當通道清單聲明法定人數大於 1 時，准入需要交易權限加上列出的帳戶來滿足清單法定人數。
- 遙測通過 `governance_manifest_admission_total{result}` 公開整體准入計數器，以便操作員可以區分成功准入與 `missing_manifest`、`non_validator_authority`、`quorum_rejected`、`protected_namespace_rejected` 和 `runtime_hook_rejected` 路徑。
- 遙測通過 `governance_manifest_quorum_total{outcome}`（值 `satisfied` / `rejected`）顯示執行路徑，以便操作員可以審核缺失的批准。
- 通道強制執行在其清單中發布的命名空間允許列表。任何設置 `gov_namespace` 的事務都必須提供 `gov_contract_id`，並且命名空間必須出現在清單的 `protected_namespaces` 集中。啟用保護後，沒有此元數據的 `RegisterSmartContractCode` 提交將被拒絕。
- 准入強制執行元組 `(namespace, contract_id, code_hash, abi_hash)` 存在已頒布的治理提案；否則驗證失敗並出現 NotPermissed 錯誤。

運行時升級掛鉤
- 通道清單可以聲明 `hooks.runtime_upgrade` 來控制運行時升級指令（`ProposeRuntimeUpgrade`、`ActivateRuntimeUpgrade`、`CancelRuntimeUpgrade`）。
- 鉤子字段：
  - `allow`（布爾值，默認為 `true`）：當 `false` 時，所有運行時升級指令都會被拒絕。
  - `require_metadata`（布爾值，默認 `false`）：需要 `metadata_key` 指定的事務元數據條目。
  - `metadata_key`（字符串）：鉤子強制執行的元數據名稱。當需要元數據或存在允許列表時，默認為 `gov_upgrade_id`。
  - `allowed_ids`（字符串數組）：可選的元數據值白名單（修剪後）。當提供的值未列出時拒絕。
- 當掛鉤存在時，隊列准入會在事務進入隊列之前強制執行元數據策略。缺少元數據、空白值或白名單之外的值會產生確定性 `NotPermitted` 錯誤。
- 遙測通過 `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}` 跟踪執法結果。
- 滿足掛鉤的交易必須包含元數據 `gov_upgrade_id=<value>`（或清單定義的密鑰）以及清單法定人數所需的任何驗證器批准。

便利端點
- POST `/v1/gov/protected-namespaces` — 直接在節點上應用 `gov_protected_namespaces`。
  - 請求：{“命名空間”：[“應用程序”，“系統”]}
  - 響應：{“ok”：true，“applied”：1}
  - 註釋：用於管理/測試；如果已配置，則需要 API 令牌。對於生產，更喜歡提交帶有 `SetParameter(Custom)` 的簽名交易。

CLI 助手
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - 獲取命名空間的合約實例並交叉檢查：
    - Torii 存儲每個 `code_hash` 的字節碼，並且其 Blake2b-32 摘要與 `code_hash` 匹配。
    - 存儲在 `/v1/contracts/code/{code_hash}` 下的清單報告匹配 `code_hash` 和 `abi_hash` 值。
    - `(namespace, contract_id, code_hash, abi_hash)` 存在已頒布的治理提案，該提案是通過節點使用的相同提案 ID 散列得出的。
  - 輸出一份 JSON 報告，其中每個合同包含 `results[]`（問題、清單/代碼/提案摘要）以及一行摘要（除非被抑制）（`--no-summary`）。
  - 對於審核受保護的命名空間或驗證治理控制的部署工作流程很有用。
- `iroha app gov deploy meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - 發出將部署提交到受保護的命名空間時使用的 JSON 元數據框架，包括用於滿足清單仲裁規則的可選 `gov_manifest_approvers`。
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — 當 `min_bond_amount > 0` 時需要鎖定提示，並且任何提供的提示集必須包括 `owner`、`amount` 和 `duration_blocks`。
  - 驗證規範帳戶 ID，規範化 32 字節無效提示，並將提示合併到 `public_inputs_json`（使用 `--public <path>` 進行額外覆蓋）。
  - 無效符源自證明承諾（公共輸入）加上 `domain_tag`、`chain_id` 和 `election_id`； `--nullifier` 已根據提供的證明進行驗證。
  - 單行摘要現在顯示從編碼的 `CastZkBallot` 派生的確定性 `fingerprint=<hex>` 以及任何解碼的提示（`owner`、`amount`、`duration_blocks`、`direction`（如果提供））。
  - CLI 響應使用 `payload_fingerprint_hex` 加上解碼字段來註釋 `tx_instructions[]`，以便下游工具可以驗證骨架，而無需重新實現 Norito 解碼。
  - 一旦電路公開相同的值，提供鎖定提示允許節點為 ZK 選票發出 `LockCreated`/`LockExtended` 事件。
- `iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - `--owner` 接受規範的 I105 文字；可選的 `@<domain>` 後綴僅是路由提示。
  - 別名 `--lock-amount`/`--lock-duration-blocks` 鏡像 ZK 標誌名稱以實現腳本奇偶校驗。
  - 摘要輸出通過包含編碼的指令指紋和人類可讀的選票字段（`owner`、`amount`、`duration_blocks`、`direction`）來鏡像 `vote --mode zk`，在簽署框架之前提供快速確認。

實例列表
- GET `/v1/gov/instances/{ns}` — 列出命名空間的活動合約實例。
  - 查詢參數：
    - `contains`：按 `contract_id` 的子字符串過濾（區分大小寫）
    - `hash_prefix`：按 `code_hash_hex`（小寫）的十六進制前綴過濾
    - `offset`（默認 0）、`limit`（默認 100，最大 10_000）
    - `order`：`cid_asc`（默認）、`cid_desc`、`hash_asc`、`hash_desc` 之一
  - 響應：{ "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - SDK 幫助程序：`ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) 或 `ToriiClient.list_governance_instances_typed("apps", ...)` (Python)。

解鎖掃碼（操作員/審計）
- 獲取 `/v1/gov/unlocks/stats`
  - 響應：{ "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - 注：`last_sweep_height` 反映了過期鎖被清除和持久化的最新區塊高度。 `expired_locks_now` 是通過掃描 `expiry_height <= height_current` 的鎖定記錄來計算的。
- 後 `/v1/gov/ballots/zk-v1`
  - 請求（v1 樣式 DTO）：
    {
      "權威": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "…?",
      "election_id": "ref-1",
      “後端”：“halo2/ipa”，
      "envelope_b64": "AAECAwQ=",
      "root_hint": "0x…64hex？",
      "owner": "i105…?", // 規範 AccountId（I105 文字）
      "金額": "100？",
      “duration_blocks”：6000？ ，
      "direction": "贊成|反對|棄權？",
      "nullifier": "blake2b32:…64hex？"
    }
  - 響應：{“ok”：true，“accepted”：true，“tx_instructions”：[{…}]}- POST `/v1/gov/ballots/zk-v1/ballot-proof`（特徵：`zk-ballot`）
  - 直接接受 `BallotProof` JSON 並返回 `CastZkBallot` 骨架。
  - 要求：
    {
      "權威": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "…?",
      "election_id": "ref-1",
      “選票”：{
        “後端”：“halo2/ipa”，
        "envelope_bytes": "AAECAwQ=", // ZK1 或 H2* 容器的 base64
        "root_hint": null, // 可選的 32 字節十六進製字符串（資格根）
        "owner": null, // 可選的規範 AccountId（I105 文字）
        "nullifier": null, // 可選的 32 字節十六進製字符串（nullifier 提示）
        "amount": "100", // 可選的鎖定金額提示（十進製字符串）
        "duration_blocks": 6000, // 可選的鎖定持續時間提示
        "direction": "Aye" // 可選方向提示
      }
    }
  - 回應：
    {
      “好的”：正確的，
      “已接受”：正確，
      "reason": "構建交易骨架",
      “tx_指令”：[
        { "wire_id": "CastZkBallot", "payload_hex": "..." }
      ]
    }
  - 注意事項：
    - 服務器將選票中的可選 `root_hint`/`owner`/`amount`/`duration_blocks`/`direction`/`nullifier` 映射到 `public_inputs_json` `CastZkBallot`。
    - 信封字節被重新編碼為指令有效負載的 base64。
    - 當 Torii 提交選票時，響應 `reason` 更改為 `submitted transaction`。
    - 僅當啟用 `zk-ballot` 功能時，此端點才可用。

CastZkBallot驗證路徑
- `CastZkBallot` 解碼提供的 Base64 證明並拒絕空或格式錯誤的有效負載（`BallotRejected` 和 `invalid or empty proof`）。
- 如果提供 `public_inputs_json`，則它必須是 JSON 對象；非對像有效負載被拒絕。
- 主機從公投 (`vk_ballot`) 或治理默認值解析選票驗證密鑰，並要求記錄存在，為 `Active`，並攜帶內聯字節。
- 存儲的驗證密鑰字節使用 `hash_vk` 重新散列；任何承諾不匹配都會在驗證之前中止執行，以防止註冊表項被篡改（`BallotRejected` 與 `verifying key commitment mismatch`）。
- 證明字節通過 `zk::verify_backend` 分派到註冊後端；無效的轉錄本顯示為 `BallotRejected` 和 `invalid proof`，並且指令確定性失敗。
- 證明必須將投票承諾和資格根源公開為公共投入；根必須與選舉的 `eligible_root` 匹配，並且派生的無效符必須與任何提供的提示匹配。
- 成功的證明發出 `BallotAccepted`；重複的無效符、過時的資格根或鎖定回歸繼續產生本文檔前​​面描述的現有拒絕原因。

## 驗證者的不當行為和聯合共識

### 削減和監禁工作流程

每當驗證者違反協議時，共識就會發出 Norito 編碼的 `Evidence`。每個有效負載都會落在內存中的 `EvidenceStore` 中，如果看不見，則會具體化到 WSV 支持的 `consensus_evidence` 映射中。早於 `sumeragi.npos.reconfig.evidence_horizon_blocks`（默認 `7 200` 塊）的記錄將被拒​​絕，因此存檔仍受限制，但會為操作員記錄拒絕。範圍內的證據遵循聯合共識暫存規則（`mode_activation_height requires next_mode to be set in the same block`）、激活延遲（`sumeragi.npos.reconfig.activation_lag_blocks`，默認 `1`）和削減延遲（`sumeragi.npos.reconfig.slashing_delay_blocks`，默認 `259200`），因此治理可以在處罰之前取消處罰。

公認的犯罪行為一對一映射到 `EvidenceKind`；判別式是穩定的並且由數據模型強制執行：

```rust
use iroha_data_model::block::consensus::EvidenceKind;

let offences = [
    EvidenceKind::DoublePrepare,
    EvidenceKind::DoubleCommit,
    EvidenceKind::InvalidQc,
    EvidenceKind::InvalidProposal,
    EvidenceKind::Censorship,
];

for (expected, kind) in offences.iter().enumerate() {
    assert_eq!(*kind as u16, expected as u16);
}
```

- **DoublePrepare/DoubleCommit** — 驗證器為同一 `(phase,height,view,epoch)` 元組簽署了衝突的哈希值。
- **InvalidQc** — 聚合器傳播了形狀未通過確定性檢查的提交證書（例如，空簽名者位圖）。
- **InvalidProposal** — 領導者提出了一個未通過結構驗證的區塊（例如，破壞了鎖鏈規則）。
- **審查** - 簽名的提交收據顯示從未提議/提交的交易。

VRF 處罰在 `activation_lag_blocks` 後自動執行（違法者將被監禁）。除非治理取消懲罰，否則共識削減僅在 `slashing_delay_blocks` 窗口之後應用。

操作員和工具可以通過以下方式檢查和重新廣播有效負載：

- Torii：`GET /v1/sumeragi/evidence` 和 `GET /v1/sumeragi/evidence/count`。
- CLI：`iroha ops sumeragi evidence list`、`… count` 和 `… submit --evidence-hex <payload>`。

治理必須將證據字節視為規範證明：

1. **在有效負載過期之前收集**。將原始 Norito 字節與高度/視圖元數據一起存檔。
2. **如果需要取消**，在 `slashing_delay_blocks` 失效之前提交帶有證據負載的 `CancelConsensusEvidencePenalty`；該記錄標記為 `penalty_cancelled` 和 `penalty_cancelled_at_height`，並且不適用削減。
3. **通過將有效負載嵌入公投或 sudo 指令（例如 `Unregister::peer`）來實施懲罰**。執行重新驗證有效負載；格式錯誤或過時的證據將被確定性地拒絕。
4. **安排後續拓撲**，以便有問題的驗證者無法立即重新加入。具有更新名冊的典型流隊列 `SetParameter(Sumeragi::NextMode)` 和 `SetParameter(Sumeragi::ModeActivationHeight)`。
5. 通過 `/v1/sumeragi/evidence` 和 `/v1/sumeragi/status` **審核結果**，以確保證據反駁取得進展並由治理部門實施刪除。

### 聯合共識測序

聯合共識保證即將發出的驗證器集在新集開始提議之前最終確定邊界塊。運行時通過配對參數強制執行規則：

- `SumeragiParameter::NextMode` 和 `SumeragiParameter::ModeActivationHeight` 必須在**同一塊**中提交。 `mode_activation_height` 必須嚴格大於進行更新的區塊高度，提供至少一個區塊的滯後。
- `sumeragi.npos.reconfig.activation_lag_blocks`（默認 `1`）是防止零延遲切換的配置保護：
- `sumeragi.npos.reconfig.slashing_delay_blocks`（默認 `259200`）延遲共識削減，以便治理可以在處罰實施之前取消處罰。

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- 運行時和 CLI 通過 `/v1/sumeragi/params` 和 `iroha --output-format text ops sumeragi params` 公開分階段參數，以便操作員可以確認激活高度和驗證器名冊。
- 治理自動化應始終：
  1. 最終確定有證據支持的移除（或恢復）決定。
  2. 使用 `mode_activation_height = h_current + activation_lag_blocks` 對後續重新配置進行排隊。
  3. 監視 `/v1/sumeragi/status`，直到 `effective_consensus_mode` 翻轉到預期高度。

任何輪換驗證器或應用削減的腳本**不得**嘗試零延遲激活或省略交接參數；此類交易將被拒絕，並使網絡保持先前的模式。

## 遙測表面

- Prometheus 指標導出治理活動：
  - `governance_proposals_status{status}`（儀表）按狀態跟踪提案計數。
  - 當受保護的命名空間准入允許或拒絕部署時，`governance_protected_namespace_total{outcome}`（計數器）遞增。
  - `governance_manifest_activations_total{event}`（計數器）記錄清單插入（`event="manifest_inserted"`）和命名空間綁定（`event="instance_bound"`）。
- `/status` 包括一個 `governance` 對象，該對象鏡像提案計數、報告受保護命名空間總數，並列出最近的清單激活（命名空間、合約 ID、代碼/ABI 哈希、塊高度、激活時間戳）。操作員可以輪詢此字段以確認已更新清單的製定以及已強制執行受保護的命名空間門。
- Grafana 模板 (`docs/source/grafana_governance_constraints.json`) 和
  `telemetry.md` 中的遙測操作手冊展示瞭如何連接卡住警報
  提案、缺少清單激活或意外的受保護命名空間
  運行時升級期間的拒絕。