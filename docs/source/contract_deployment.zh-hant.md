---
lang: zh-hant
direction: ltr
source: docs/source/contract_deployment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0f2b1d7d027d715eac5a3ca8be29dea8f0e76013e948947a4de66108ac561f34
source_last_modified: "2026-01-22T14:58:53.689594+00:00"
translation_last_reviewed: 2026-02-07
title: Contract Deployment (.to) — API & Workflow
translator: machine-google-reviewed
---

狀態：由 Torii、CLI 和核心入學測試實施和執行（2025 年 11 月）。

## 概述

- 通過將已編譯的 IVM 字節碼 (`.to`) 提交到 Torii 或通過發布來部署
  `RegisterSmartContractCode`/`RegisterSmartContractBytes`指令
  直接。
- 節點在本地重新計算 `code_hash` 和規範的 ABI 哈希值；不匹配
  果斷地拒絕。
- 存儲的工件位於鏈上 `contract_manifests` 和
  `contract_code` 註冊表。清單僅引用哈希值並且保持很小；
  代碼字節由 `code_hash` 鍵入。
- 受保護的命名空間可能需要在實施之前製定治理提案
  部署被承認。准入路徑查找提案有效負載並
  強制 `(namespace, contract_id, code_hash, abi_hash)` 相等時
  命名空間受到保護。

## 存儲的工件和保留

- `RegisterSmartContractCode` 插入/覆蓋給定的清單
  `code_hash`。當相同的哈希值已經存在時，它將被新的哈希值替換
  明顯。
- `RegisterSmartContractBytes` 將編譯後的程序存儲在
  `contract_code[code_hash]`。如果哈希的字節已經存在，它們必須匹配
  確切地說；不同的字節會引發不變違規。
- 代碼大小受自定義參數 `max_contract_code_bytes` 的限制
  （默認 16 MiB）。之前用 `SetParameter(Custom)` 事務覆蓋它
  註冊較大的工件。
- 保留不受限制：清單和代碼在明確之前保持可用
  在未來的治理工作流程中刪除。沒有 TTL 或自動 GC。

## 准入管道

- 驗證器解析 IVM 標頭，強制執行 `version_major == 1`，並檢查
  `abi_version == 1`。未知版本立即拒絕；沒有運行時間
  切換。
- 當 `code_hash` 的清單已存在時，驗證可確保
  存儲的 `code_hash`/`abi_hash` 等於提交的計算值
  程序。不匹配會產生 `Manifest{Code,Abi}HashMismatch` 錯誤。
- 針對受保護名稱空間的事務必須包含元數據密鑰
  `gov_namespace` 和 `gov_contract_id`。錄取路徑對比
  反對已頒布的 `DeployContract` 提案；如果不存在匹配的提案
  交易被拒絕，代碼為 `NotPermitted`。

## Torii 端點（功能 `app_api`）- `POST /v1/contracts/deploy`
  - 請求正文：`DeployContractDto`（有關字段詳細信息，請參閱 `docs/source/torii_contracts_api.md`）。
  - Torii 解碼 Base64 有效負載，計算兩個哈希值，構建清單，
    並提交 `RegisterSmartContractCode` plus
    `RegisterSmartContractBytes` 在代表簽名的交易中
    來電者。
  - 響應：`{ ok, code_hash_hex, abi_hash_hex }`。
  - 錯誤：base64 無效、ABI 版本不受支持、缺少權限
    (`CanRegisterSmartContractCode`)，超出尺寸上限，治理門控。
- `POST /v1/contracts/code`
  - 接受 `RegisterContractCodeDto`（權限、私鑰、清單）並僅提交
    `RegisterSmartContractCode`。當清單單獨暫存時使用
    字節碼。
- `POST /v1/contracts/instance`
  - 接受 `DeployAndActivateInstanceDto`（權限、私鑰、命名空間/contract_id、`code_b64`、可選清單覆蓋）並以原子方式部署+激活。
- `POST /v1/contracts/instance/activate`
  - 接受 `ActivateInstanceDto`（權限、私鑰、命名空間、contract_id、`code_hash`）並僅提交激活指令。
- `GET /v1/contracts/code/{code_hash}`
  - 返回 `{ manifest: { code_hash, abi_hash } }`。
    其他清單字段在內部保留，但此處省略
    穩定的API。
- `GET /v1/contracts/code-bytes/{code_hash}`
  - 返回 `{ code_b64 }`，其中存儲的 `.to` 圖像編碼為 base64。

所有合約生命週期端點共享一個通過配置的專用部署限制器
`torii.deploy_rate_per_origin_per_sec`（每秒令牌數）和
`torii.deploy_burst_per_origin`（突發令牌）。默認值為 4 req/s，突發
8 對於從 `X-API-Token`、遠程 IP 或端點提示派生的每個令牌/密鑰。
將任一字段設置為 `null` 以禁用受信任操作員的限制器。當
限制器觸發，Torii 遞增
`torii_contract_throttled_total{endpoint="code|deploy|instance|activate"}` 遙測計數器和
返回 HTTP 429；任何處理程序錯誤都會增加
`torii_contract_errors_total{endpoint=…}` 用於警報。

## 治理集成和受保護的命名空間- 設置自定義參數`gov_protected_namespaces`（命名空間的JSON數組
  字符串）以啟用准入門控。 Torii 暴露助手
  `/v1/gov/protected-namespaces` 和 CLI 通過以下方式鏡像它們
  `iroha_cli app gov protected set` / `iroha_cli app gov protected get`。
- 使用 `ProposeDeployContract`（或 Torii）創建的提案
  `/v1/gov/proposals/deploy-contract` 端點）捕獲
  `(namespace, contract_id, code_hash, abi_hash, abi_version)`。
- 一旦公投通過，`EnactReferendum` 標記提案已頒布並且
  准入將接受攜帶匹配元數據和代碼的部署。
- 交易必須包含元數據對 `gov_namespace=a namespace` 和
  `gov_contract_id=an identifier`（並且應該設置 `contract_namespace` /
  `contract_id` 用於呼叫時間綁定）。 CLI 幫助程序填充這些
  當您通過 `--namespace`/`--contract-id` 時自動。
- 當啟用受保護的命名空間時，隊列准入會拒絕嘗試
  將現有的 `contract_id` 重新綁定到不同的命名空間；使用已製定的
  建議或在部署到其他地方之前撤銷以前的綁定。
- 如果通道清單將驗證器法定人數設置為高於 1，請包括
  `gov_manifest_approvers`（驗證者帳戶 ID 的 JSON 數組），以便隊列可以計數
  與交易授權一起的額外批准。車道也拒絕
  引用清單中不存在的命名空間的元數據
  `protected_namespaces` 設置。

## CLI 助手

- `iroha_cli app contracts deploy --authority <id> --private-key <hex> --code-file <path>`
  提交 Torii 部署請求（動態計算哈希值）。
- `iroha_cli app contracts deploy-activate --authority <id> --private-key <hex> --namespace <ns> --contract-id <id> --code-file <path>`
  構建清單（使用提供的密鑰簽名），註冊字節+清單，
  並在一筆交易中激活 `(namespace, contract_id)` 綁定。使用
  `--dry-run` 打印計算出的哈希值和指令計數，無需
  提交，並使用 `--manifest-out` 保存簽名的清單 JSON。
- `iroha_cli app contracts manifest build --code-file <path> [--sign-with <hex>]` 計算
  `code_hash`/`abi_hash` 用於編譯的 `.to` 並可選擇簽署清單，
  打印 JSON 或寫入 `--out`。
- `iroha_cli app contracts simulate --authority <id> --private-key <hex> --code-file <path> --gas-limit <u64>`
  運行離線 VM 傳遞並報告 ABI/哈希元數據以及排隊的 ISI
  （計數和指令 ID）無需接觸網絡。附加
  `--namespace/--contract-id` 用於鏡像呼叫時間元數據。
- `iroha_cli app contracts manifest get --code-hash <hex>` 通過 Torii 獲取清單
  並可選擇將其寫入磁盤。
- `iroha_cli app contracts code get --code-hash <hex> --out <path>` 下載
  存儲的 `.to` 圖像。
- `iroha_cli app contracts instances --namespace <ns> [--table]` 列表已激活
  合約實例（清單+元數據驅動）。
- 治理助手（`iroha_cli app gov deploy propose`、`iroha_cli app gov enact`、
  `iroha_cli app gov protected set/get`）編排受保護的命名空間工作流程並
  公開 JSON 工件以供審核。

## 測試和覆蓋範圍

- `crates/iroha_core/tests/contract_code_bytes.rs` 覆蓋代碼下的單元測試
  存儲、冪等性和大小上限。
- `crates/iroha_core/tests/gov_enact_deploy.rs` 通過驗證清單插入
  制定和 `crates/iroha_core/tests/gov_protected_gate.rs` 練習
  端到端的受保護命名空間准入。
- Torii 路由包括請求/響應單元測試，並且 CLI 命令具有
  集成測試確保 JSON 往返保持穩定。

請參閱 `docs/source/governance_api.md` 了解詳細的公投有效負載和
投票工作流程。