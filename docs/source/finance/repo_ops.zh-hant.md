---
lang: zh-hant
direction: ltr
source: docs/source/finance/repo_ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 42c328443065e102a65180421d515e4e3040a35175c348ea25fd83edab1236b4
source_last_modified: "2026-01-22T16:26:46.567961+00:00"
translation_last_reviewed: 2026-02-07
title: Repo Operations & Evidence Guide
summary: Governance, lifecycle, and audit requirements for repo/reverse-repo flows (roadmap F1).
translator: machine-google-reviewed
---

# 回購操作和證據指南（路線圖 F1）

回購計劃以確定性方式解鎖雙邊和三方融資
Norito 指令、CLI/SDK 幫助程序和 ISO 20022 奇偶校驗。這篇筆記捕捉到了
滿足路線圖里程碑 **F1 — 回購協議所需的運營合同
生命週期文檔和工具**。它補充了面向工作流程的
[`repo_runbook.md`](./repo_runbook.md) 通過闡明：

- 跨 CLI/SDK/運行時的生命週期表面（`crates/iroha_cli/src/main.rs:3821`，
  `python/iroha_python/iroha_python_rs/src/lib.rs:2216`，
  `crates/iroha_core/src/smartcontracts/isi/repo.rs:1`);
- 確定性證明/證據捕獲（`integration_tests/tests/repo.rs:1`）；
- 三方託管和抵押品替代行為；和
- 治理期望（雙重控制、審計跟踪、回滾手冊）。

## 1. 範圍和驗收標準

路線圖項目 F1 仍然有四個主題；該文件列舉了
所需的工件以及已滿足它們的代碼/測試的鏈接：

|要求 |證據|
|----------|----------|
|回購→逆回購→替代的確定性結算證明 | `integration_tests/tests/repo.rs` 捕獲端到端流程、重複 ID 防護、保證金節奏檢查以及抵押品替代成功/失敗案例。該套件作為 `cargo test --workspace` 的一部分運行。 `crates/iroha_core/src/smartcontracts/isi/repo.rs` (`repo_deterministic_lifecycle_proof_matches_fixture`) 上的確定性生命週期摘要工具可快照啟動 → 餘量 → 替換幀，以便審核員可以區分規範的有效負載。 |
|三方報導|運行時強制執行託管人感知流：`RepoAgreement::custodian` + `RepoAccountRole::Custodian` 事件（`crates/iroha_data_model/src/repo.rs:74`、`crates/iroha_data_model/src/events/data/events.rs:742`）。 |
|抵押替代測試|反向腿不變量拒絕抵押不足的替代（`crates/iroha_core/src/smartcontracts/isi/repo.rs:417`），並且集成測試斷言在替代往返後賬本正確清除（`integration_tests/tests/repo.rs:261`）。 |
|追加保證金節奏和參與者執行 | `integration_tests/tests/repo.rs::repo_margin_call_enforces_cadence_and_participant_rules` 練習 `RepoMarginCallIsi`，證明節奏一致的調度、拒絕過早調用以及僅限參與者授權。 |
|治理批准的操作手冊 |本指南和 `repo_runbook.md` 提供 CLI/SDK 程序、欺詐/回滾步驟以及用於審計的證據捕獲說明。 |

## 2. 生命週期表面

### 2.1 CLI 和 Norito 構建器

- `iroha app repo initiate|unwind|margin|margin-call` 包裹 `RepoIsi`，
  `ReverseRepoIsi` 和 `RepoMarginCallIsi`
  （`crates/iroha_cli/src/main.rs:3821`）。每個子命令支持 `--input` /
  `--output`，因此服務台可以在之前暫存指令有效負載以進行雙重批准
  提交。託管人路由通過 `--custodian` 表示。
- `repo query list|get` 使用 `FindRepoAgreements` 來快照協議並且可以
  被重定向到 JSON 工件以獲取證據包。
- `crates/iroha_cli/tests/cli_smoke.rs:2637`下的CLI煙霧測試確保
  對於審核員來說，發送到文件的路徑保持穩定。

### 2.2 SDK 和自動化掛鉤- Python 綁定公開 `RepoAgreementRecord`、`RepoCashLeg`、
  `RepoCollateralLeg`，以及便利建設者
  (`python/iroha_python/iroha_python_rs/src/lib.rs:2216`) 因此自動化可以
  在本地組裝交易並評估 `next_margin_check_after`。
- JS/Swift 助手通過以下方式重用相同的 Norito 佈局
  `javascript/iroha_js/src/instructionBuilders.js` 和
  `IrohaSwift/Sources/IrohaSwift/ConfidentialEncryptedPayload.swift` 用於備註
  處理； SDK 在線程化存儲庫治理旋鈕時應參考此文檔。

### 2.3 賬本事件和遙測

每個生命週期操作都會發出 `AccountEvent::Repo(...)` 記錄，其中包含
`RepoAccountEvent::{Initiated,Settled,MarginCalled}` 有效負載範圍為
參與者角色 (`crates/iroha_data_model/src/events/data/events.rs:742`)。推
將這些事件放入您的 SIEM/日誌聚合器中以獲得防篡改的審核日誌
用於桌面操作、追加保證金通知和託管人通知。

### 2.4 配置傳播和驗證

節點從 `[settlement.repo]` 節中攝取回購治理旋鈕
`iroha_config` (`crates/iroha_config/src/parameters/user.rs:4071`)。對待那個
作為治理證據合約的一部分的片段——將其放入版本控制中
與存儲庫數據包一起並對其進行哈希處理，然後再將更改推送到您的
自動化或 ConfigMap。最小的配置文件如下所示：

```toml
[settlement.repo]
default_haircut_bps = 1500
margin_frequency_secs = 86400
eligible_collateral = ["4fEiy2n5VMFVfi6BzDJge519zAzg", "7dk8Pj8Bqo6XUqch4K2sF8MCM1zd"]

[settlement.repo.collateral_substitution_matrix]
"4fEiy2n5VMFVfi6BzDJge519zAzg" = ["7dk8Pj8Bqo6XUqch4K2sF8MCM1zd", "6zK1LDcJ3FvkpfoZQ8kHUaW6sA7F"]
```

操作清單：

1. 將上面的代碼片段（或您的生產變體）提交到配置存儲庫中
   提供 `irohad` 並將其 SHA-256 記錄在治理數據包中，以便
   審閱者可以區分您計劃部署的字節。
2. 在整個隊列中滾動更改（systemd 單元、Kubernetes ConfigMap 等）
   並重新啟動每個節點。推出後立即捕獲 Torii
   出處的配置快照：

   ```bash
   curl -sS "${TORII_URL}/v1/configuration" \
     -H "Authorization: Bearer ${TOKEN}" | jq .
   ```

   `ToriiClient.get_configuration()` 在 Python SDK 中可用，用於相同的
   自動化需要輸入證據時的目的。 【python/iroha_python/src/iroha_python/client.py:5791】
3. 通過查詢證明運行時現在強制執行所請求的節奏/髮型
   `FindRepoAgreements`（或 `iroha app repo margin --agreement-id ...`）和
   檢查嵌入的 `RepoGovernance` 值。存儲 JSON 響應
   在 `artifacts/finance/repo/<agreement>/agreements_after.json` 下；那些價值觀
   源自 `[settlement.repo]`，因此當
   Torii 的 `/v1/configuration` 快照不足。
4. 將兩個工件（TOML 片段和 Torii/CLI 快照）保留在
   在提交治理請求之前收集證據。審核員必須能夠
   重放該代碼片段，驗證其哈希值，並將其與運行時視圖關聯起來。

此工作流程確保存儲庫台永遠不會依賴臨時環境變量，即
配置路徑保持確定性，每個治理票證都帶有
路線圖 F1 中預計有相同的 `iroha_config` 樣張集。

### 2.5 確定性證明工具

單元測試 `repo_deterministic_lifecycle_proof_matches_fixture`（參見
`crates/iroha_core/src/smartcontracts/isi/repo.rs`）序列化每個階段
將 repo 生命週期轉換為 Norito JSON 框架，將其與規範的固定裝置進行比較
`crates/iroha_core/tests/fixtures/repo_lifecycle_proof.json`，並散列
捆綁（夾具摘要跟踪
`crates/iroha_core/tests/fixtures/repo_lifecycle_proof.digest`）。通過以下方式在本地運行：

```bash
cargo test -p iroha_core \
  -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
```該測試現在作為默認 `cargo test -p iroha_core` 套件的一部分運行，因此 CI
guards the snapshot automatically.每當回購語義或固定裝置發生變化時，
使用以下命令刷新 JSON 和摘要：

```bash
scripts/regen_repo_proof_fixture.sh
```

助手使用固定的 `rust-toolchain.toml` 通道，重寫燈具
在 `crates/iroha_core/tests/fixtures/` 下，並重新運行確定性工具
因此簽入的快照/摘要與運行時行為保持同步
審核員將重播。

### 2.4 Torii API 表面

- `GET /v1/repo/agreements` 返回帶有可選分頁、過濾的活動協議
  (`filter={...}`)、排序和地址格式化參數。使用它進行快速審核或
  當原始 JSON 有效負載足夠時，儀表板。
- `POST /v1/repo/agreements/query` 接受結構化查詢信封（分頁、排序、
  `FilterExpr`、`fetch_size`），因此下游服務可以確定性地翻閱賬本。
- JavaScript SDK 現在公開 `listRepoAgreements`、`queryRepoAgreements` 和迭代器
  幫助器，以便 browser/Node.js 工具接收與 Rust/Python 相同類型的 DTO。

### 2.4 配置默認值

節點將 `[settlement.repo]` 讀入
啟動期間`iroha_config::parameters::actual::Repo`；任何回購指令
將參數保留為零的情況根據之前的默認值進行歸一化
記錄在鏈上。 【crates/iroha_core/src/smartcontracts/isi/repo.rs:40】這個
讓治理在不觸及每個 SDK 的情況下提高（或降低）基準政策
呼叫站點，前提是政策變更已完整記錄。

- `default_haircut_bps` – `RepoGovernance::haircut_bps()` 時的後備理髮
  等於零。運行時將其限制在 10000bps 的硬上限以保持
  配置正常。 【crates/iroha_core/src/smartcontracts/isi/repo.rs:44】
- `margin_frequency_secs` – `RepoMarginCallIsi` 的節奏。清零請求
  繼承這個值，因此縮短節奏迫使辦公桌留出更多餘量
  默認情況下經常發生。 【crates/iroha_core/src/smartcontracts/isi/repo.rs:49】
- `eligible_collateral` – `AssetDefinitionId` 的可選允許列表。當
  列表非空 `RepoIsi` 拒絕集合之外的任何質押，防止
  意外加入未經審查的債券。 【crates/iroha_core/src/smartcontracts/isi/repo.rs:57】
- `collateral_substitution_matrix` – map of original collateral →
  允許的替代品。 `ReverseRepoIsi` 僅在以下情況下接受替換
  矩陣包含記錄的定義作為鍵以及其替換
  值數組；否則，平倉失敗，證明治理批准了
  梯子。 【crates/iroha_core/src/smartcontracts/isi/repo.rs:74】

這些旋鈕位於節點配置中的 `[settlement.repo]` 下，並且是
通過 `iroha_config::parameters::user::Repo` 解析，因此它們應該被捕獲在
每個治理證據包。 【crates/iroha_config/src/parameters/user.rs:3956】

```toml
[settlement.repo]
default_haircut_bps = 1750
margin_frequency_secs = 43200
eligible_collateral = ["4fEiy2n5VMFVfi6BzDJge519zAzg", "7dk8Pj8Bqo6XUqch4K2sF8MCM1zd"]

[settlement.repo.collateral_substitution_matrix]
"4fEiy2n5VMFVfi6BzDJge519zAzg" = ["7dk8Pj8Bqo6XUqch4K2sF8MCM1zd", "6zK1LDcJ3FvkpfoZQ8kHUaW6sA7F"]
```

**變更管理清單**1. 暫存提議的 TOML 片段（包括替換矩陣增量）、哈希
   使用 SHA-256，並將代碼片段和哈希值附加到治理中
   票證，以便審閱者可以逐字重現字節。
2. 引用提案/公投中的片段（例如通過
   治理 CLI 上的 `--notes` 字段）並收集所需的批准
   對於F1。保留已簽名的批准數據包並附加片段。
3. 在整個機群中滾動更改：更新 `[settlement.repo]`，重新啟動每個
   節點，然後捕獲 `GET /v1/configuration` 快照（或
   `ToriiClient.getConfiguration`) 證明每個對等點的應用值。
4.重新運行`integration_tests/tests/repo.rs` plus
   `repo_deterministic_lifecycle_proof_matches_fixture` 並接下來存儲日誌
   到配置差異，以便審核員可以看到新的默認值保留
   決定論。

如果沒有矩陣條目，運行時將拒絕更改資產的替換
定義，即使通用 `eligible_collateral` 列表允許；提交
配置快照和回購證據，以便審計員可以重現準確的
預訂回購時強制執行的政策。

### 2.5 配置證據和漂移檢測

Norito/`iroha_config` 管道現在公開已解決的回購策略
`iroha_config::parameters::actual::Repo`，因此治理數據包必須證明
每個同行應用的值——不僅僅是提議的 TOML。捕獲已解決的
每次推出後的配置及其摘要：

1. 從每個對等方獲取配置（`GET /v1/configuration` 或
   `ToriiClient.getConfiguration`) 並隔離存儲庫節：

   ```bash
   curl -s http://<torii-host>/v1/configuration \
     | jq -cS '.settlement.repo' \
     > artifacts/finance/repo/<agreement-id>/config/repo_config_actual.json
   ```

2. 對規範 JSON 進行哈希處理並將其記錄在證據清單中。當
   艦隊運行狀況良好，哈希值應該在對等點之間匹配，因為 `actual`
   將默認值與暫存的 `[settlement.repo]` 片段相結合：

   ```bash
   shasum -a 256 artifacts/finance/repo/<agreement-id>/config/repo_config_actual.json
   ```

3. 將 JSON + hash 附加到治理數據包中，並將該條目鏡像到
   清單已上傳到治理 DAG。如果任何同行報告有分歧
   digest, halt the rollout and reconcile the config/state drift before
   進行中。

### 2.6 治理批准和證據包

僅當存儲庫台將確定性數據包饋送到路線圖 F1 時，路線圖 F1 才會關閉
治理 DAG，因此每次更改（新的理髮、託管政策或抵押品）
矩陣）必須在安排投票之前運送相同的物品。 【docs/source/governance_playbook.md:1】

**攝入包**1. **跟踪模板** – 副本
   `docs/examples/finance/repo_governance_packet_template.md` 進入你的證據
   目錄（例如
   `artifacts/finance/repo/<agreement-id>/packet.md`) 並填寫元數據
   在開始對工件進行哈希處理之前進行阻止。模板保留治理
   理事會的節奏通過列出文件路徑、SHA-256 摘要和
   審稿人致謝集中在一處。
2. **指令有效負載** – 階段啟動、平倉和追加保證金
   帶有 `iroha app repo ... --output` 的說明，以便雙控審批者審核
   字節相同的有效負載。散列每個文件並將其存儲在
   `artifacts/finance/repo/<agreement-id>/` 位於桌子證據包旁邊
   本筆記其他地方引用。 【crates/iroha_cli/src/main.rs:3821】
3. **配置差異** – 包括確切的 `[settlement.repo]` TOML 片段
   （默認加上替換矩陣）及其 SHA-256。這證明了哪個
   一旦投票通過並反映結果，`iroha_config` 旋鈕將被激活
   在准入時標準化回購指令的運行時字段。 【crates/iroha_config/src/parameters/user.rs:3956】
4. **確定性測試** – 附上最新的
   `integration_tests/tests/repo.rs` 日誌和輸出
   `repo_deterministic_lifecycle_proof_matches_fixture` 所以審閱者看到
   與分階段指令對應的生命週期證明哈希。 【integration_tests/tests/repo.rs:1】【crates/iroha_core/src/smartcontracts/isi/repo.rs:1450】
5. **事件/遙測快照** – 導出最近的 `AccountEvent::Repo(*)`
   範圍內的服務台以及理事會需要的任何儀表板/指標的流
   判斷風險（例如，保證金漂移）。這給了審計師同樣的
   他們稍後會從 Torii 重建防篡改日誌。 【crates/iroha_data_model/src/events/data/events.rs:742】

**批准和記錄**

- 參考治理票證或公投中的人工製品哈希值，以及
  鏈接到分階段的數據包，以便理事會可以遵循標準儀式
  治理手冊中概述，無需追逐臨時路徑。 【docs/source/governance_playbook.md:8】
- 捕獲哪些雙重控制簽名者審查了分階段的指令文件並
  將他們的確認存儲在哈希值旁邊；這是鏈上證明
  回購台滿足“兩人規則”，儘管運行時也
  強制僅參與者執行。
- 當理事會發布治理批准記錄 (GAR) 時，反映
  在證據目錄中籤署會議記錄，以便將來替換或
  髮型更新可以引用確切的決策包，而不是重述
  理由。

**批准後推出**1. 應用批准的 `[settlement.repo]` 配置並重新啟動每個節點（或滾動
   通過您的自動化）。立即撥打 `GET /v1/configuration` 並存檔
   每個節點的響應，以便治理包顯示哪些對等點接受了
   改變。 【crates/iroha_torii/src/lib.rs:3225】
2. 重新運行確定性存儲庫測試並附加新日誌和構建
   元數據（git commit、工具鏈），以便審計員可以重現結算結果
   推出後的證明。
3. 使用證據存檔路徑、哈希值和更新治理跟踪器
   觀察者聯繫，以便以後的回購台可以繼承相同的流程，而不是
   重新導出清單。

**治理 DAG 出版物（必需）**

1. Tar 證據目錄（配置片段、指令負載、證明日誌、
   GAR/分鐘）並將其作為治理 DAG 管道
   `GovernancePayloadKind::PolicyUpdate` 有效負載，帶有註釋
   `agreement_id`、`iso_week` 以及建議的折扣/保證金值；的
   管道規範和 CLI 界面位於
   `docs/source/sorafs_governance_dag_plan.md`。
2.發布者更新IPNS頭後，記錄塊CID和頭CID
   在治理跟踪器和 GAR 中，這樣任何人都可以獲取不可變的數據
   稍後包。 `sorafs governance dag head` 和 `sorafs governance dag list`
   讓您在投票開始前確認節點已固定。
3. 將 CAR 文件或塊有效負載存儲在回購證據存檔旁邊，以便
   審計員可以將鏈上治理決策與確切的情況進行協調
   已批准的鏈下數據包。

### 2.7 生命週期快照刷新

每當回購語義發生變化（利率、結算數學、託管邏輯或
默認配置），刷新確定性生命週期快照，以便治理可以
引用新的摘要，無需對證明工具進行逆向工程。

1. 刷新固定工具鏈下的夾具：

   ```bash
   scripts/regen_repo_proof_fixture.sh --toolchain <toolchain> \
     --bundle-dir artifacts/finance/repo/<agreement>
   ```

   助手將輸出暫存在臨時目錄中，更新跟踪的裝置
   在 `crates/iroha_core/tests/fixtures/repo_lifecycle_proof.{json,digest}`，
   重新運行驗證測試以進行驗證，並且（當設置 `--bundle-dir` 時）
   將 `repo_proof_snapshot.json` 和 `repo_proof_digest.txt` 放入捆綁包中
   審計員目錄。
2. 在不接觸跟踪固定裝置的情況下導出工件（例如，試運行
   證據），直接設置環境助手：

   ```bash
   REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<agreement>/repo_proof_snapshot.json \
   REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<agreement>/repo_proof_digest.txt \
   cargo test -p iroha_core \
     -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
   ```

   `REPO_PROOF_SNAPSHOT_OUT` 從證明中接收美化後的 Norito JSON
   線束，而 `REPO_PROOF_DIGEST_OUT` 存儲大寫十六進制摘要（帶有
   為了方便起見，尾隨換行符）。當以下情況時，助手拒絕覆蓋文件
   父目錄不存在，因此首先構建 `artifacts/...` 樹。
3. 將兩個導出的文件附加到協議包（參見§3）並重新生成
   通過 `scripts/repo_evidence_manifest.py` 的清單，因此治理數據包
   明確引用刷新的證據工件。回購內固定裝置
   仍然是 CI 的真相來源。

### 2.8 應計利息和到期日治理**確定性利息數學。 ** `RepoIsi` 和 `ReverseRepoIsi` 得出現金
在放鬆時間欠 ACT/360 助手的
`compute_accrued_interest()`【crates/iroha_core/src/smartcontracts/isi/repo.rs:100】
以及 `expected_cash_settlement()` 內拒絕還款腿的守衛
其回報小於*本金+利息*。 【crates/iroha_core/src/smartcontracts/isi/repo.rs:132】
幫助器將 `rate_bps` 標準化為四位小數分數，並將其乘以
`elapsed_ms / (360 * 24h)` 使用 18 位小數，最後四捨五入到
現金支線的 `NumericSpec` 聲明的規模。保留治理包
可重現，捕獲為助手提供的四個值：

1. `cash_leg.quantity`（本金），
2.`rate_bps`，
3. `initiated_timestamp_ms`，以及
4. 您打算使用的展開時間戳（對於計劃的總帳條目，這是
   通常為 `maturity_timestamp_ms`，但緊急展開會記錄實際的
   `ReverseRepoIsi::settlement_timestamp_ms`）。

將元組存儲在分階段展開指令旁邊並附上簡短的證明
片段如：

```python
from decimal import Decimal
ACT_360_YEAR_MS = 24 * 60 * 60 * 1000 * 360

principal = Decimal("1000")
rate_bps = Decimal("1500")  # 150 bps
elapsed_ms = Decimal(maturity_ms - initiated_ms)
interest = principal * (rate_bps / Decimal(10_000)) * (elapsed_ms / Decimal(ACT_360_YEAR_MS))
expected_cash = principal + interest.quantize(Decimal("0.01"))
```

舍入的 `expected_cash` 必須與反向編碼的 `quantity` 匹配
回購指令。將腳本輸出（或計算器工作表）保存在
`artifacts/finance/repo/<agreement>/interest.json`，以便審核員可以重新計算
無需解釋您的交易電子表格即可得出數據。集成套件
已經強制執行相同的不變量
(`repo_roundtrip_transfers_balances_and_clears_agreement`)，但是操作證據
應該引用將要展開的確切值。 【integration_tests/tests/repo.rs:1】

**保證金和應計節奏。 ** 每項協議都會暴露節奏助手
`RepoAgreement::next_margin_check_after()` 和緩存的
`last_margin_check_timestamp_ms`，使辦公桌能夠證明利潤掃蕩
甚至在提交 `RepoMarginCallIsi` 之前就根據政策進行了安排
交易。 【crates/iroha_data_model/src/repo.rs:113】【crates/iroha_core/src/smartcontracts/isi/repo.rs:557】
每個追加保證金通知必須在證據包中包含三件文物：

1. `repo margin-call --agreement <id>` JSON輸出（或等效的SDK
   Payload），記錄了協議id、用於該協議的區塊時間戳
   檢查，以及觸發它的權限。 【crates/iroha_cli/src/main.rs:3821】
2. 協議快照（`repo query get --agreement-id <id>`）
   就在通話之前，以便審閱者可以確認節奏是否到期
   （比較 `current_timestamp_ms` 與 `next_margin_check_after()`）。
3. 發送到每個角色的 `AccountEvent::Repo::MarginCalled` SSE/NDJSON feed
   （發起人、交易對手和可選的託管人）因為運行時
   為每個參與者復制事件。 【crates/iroha_data_model/src/events/data/events.rs:742】

CI 已經通過以下方式行使了這些規則
`repo_margin_call_enforces_cadence_and_participant_rules`，拒絕呼叫
提前到達或來自未經授權的帳戶。 【integration_tests/tests/repo.rs:395】
重複證據檔案中的出處就是路線圖 F1 的結束
文檔差距：治理審核者可以看到與
運行時依賴，以及第 2.7 節中捕獲的確定性證明哈希
以及第 3.2 節中討論的清單。

### 2.8 三方託管批准和監控路線圖 **F1** 還提出了三方回購協議，其中抵押品存放在
託管人而非交易對手。運行時強制執行託管路徑
通過持久化 `RepoAgreement::custodian`，將質押資產路由至
託管人的帳戶在啟動和發出期間
`RepoAccountRole::Custodian` 每個生命週期步驟的事件，以便審核員可以看到
誰在每個時間戳持有抵押品。 【crates/iroha_data_model/src/repo.rs:74】【crates/iroha_core/src/smartcontracts/isi/repo.rs:252】【integration_tests/tests/repo.rs:951】
除了上面列出的雙邊證據外，每個三方回購協議還必須
在治理數據包被視為完整之前捕獲以下工件。

**額外攝入要求**

1. **保管人確認書。 ** 服務台必須存儲來自以下機構的簽名確認書：
   每個託管人確認回購標識符、託管窗口、路由
   賬戶和結算 SLA。附上已簽署的文件
   （`artifacts/finance/repo/<agreement>/custodian_ack_<custodian>.md`）
   並在治理包中引用它，以便審閱者可以看到
   第三方同意發起者/交易對手批准的相同字節。
2. **託管賬本快照。 ** 啟動將抵押品移至託管人
   account 和 unwind 將其返回給發起者；捕捉相關的
   `FindAssets` 輸出為每條腿之前和之後的託管人以便審核員
   可以確認餘額與階段指令相符。 【crates/iroha_core/src/smartcontracts/isi/repo.rs:252】【crates/iroha_core/src/smartcontracts/isi/repo.rs:1641】
3. **事件收據。 ** 鏡像所有角色的 `RepoAccountEvent` 流並
   將託管人有效負載與發起者/交易對手記錄一起存儲。
   運行時為每個角色發出單獨的事件
   `RepoAccountRole::{Initiator,Counterparty,Custodian}`，因此附上原始數據
   SSE feed proves that all three parties saw the same timestamps and
   結算金額。 【crates/iroha_data_model/src/events/data/events.rs:742】【integration_tests/tests/repo.rs:1508】
4. **託管人準備情況檢查表。 ** 當回購引用操作時
   墊片（例如，託管調節或長期指示），記錄
   用於排練工作流程的自動化聯繫人和命令（例如
   如 `iroha app repo initiate --custodian ... --dry-run`），因此審閱者可以到達
   演習期間的託管操作員。

|證據|命令/路徑|目的|
|----------|----------------|---------|
|託管人確認 (`custodian_ack_<custodian>.md`) |鏈接到 `docs/examples/finance/repo_governance_packet_template.md` 中引用的簽名註釋（使用 `docs/examples/finance/repo_custodian_ack_template.md` 作為種子）。 |顯示第三方在資產轉移之前接受了 repo id、託管 SLA 和結算渠道。 |
|託管資產快照 | `iroha json --query FindAssets '{ "id": "...#<custodian>" }' > artifacts/.../assets/custodian_<ts>.json` |證明留下/歸還的抵押品與 `RepoIsi` 編碼完全相同。 |
|託管人 `RepoAccountEvent` 飼料 | `torii-events --account <custodian> --event-type repo > artifacts/.../events/custodian.ndjson` |捕獲為啟動、追加保證金通知和展開而發出的運行時的 `RepoAccountRole::Custodian` 有效負載。 |
|託管演習日誌| `artifacts/.../governance/drills/<timestamp>-custodian.log` |記錄託管人執行回滾或結算腳本的試運行。 |重複使用相同的哈希工作流程 (`scripts/repo_evidence_manifest.py`)
託管人確認、資產快照和事件源保持三方關係
數據包可重現。當多個保管人參與一本書時，創建
每個保管人的子目錄，以便清單突出顯示哪些文件屬於
各方；治理票證應引用每個清單哈希和
匹配的確認文件。集成測試涵蓋
`repo_initiation_with_custodian_routes_collateral` 和
`reverse_repo_with_custodian_emits_events_for_all_parties` 已經強制執行
運行時行為——將他們的人工製品鏡像到證據包中就是什麼
讓路線圖 **F1** 為三方場景提供 GA 就緒文檔。 【integration_tests/tests/repo.rs:951】【integration_tests/tests/repo.rs:1508】

### 2.9 批准後配置快照

一旦治理批准變更並且 `[settlement.repo]` 節登陸
在集群中，從每個對等方捕獲經過身份驗證的配置快照，以便
審核員可以證明批准的值是有效的。 Torii 暴露了
用於此目的的 `/v1/configuration` 路線和所有 SDK 表面幫助程序，例如
`ToriiClient.getConfiguration`，因此捕獲工作流程適用於桌面腳本，
CI，或手動操作員運行。 【crates/iroha_torii/src/lib.rs:3225】【javascript/iroha_js/src/toriiClient.js:2115】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4681】

1. 在每個對等點之後立即調用 `GET /v1/configuration`（或 SDK 幫助程序）
   推出。將完整的 JSON 保留在下面
   `artifacts/finance/repo/<agreement>/config/peers/<peer-id>.json` and record
   `config/config_snapshot_index.md` 中的塊高度/集群時間戳。
   ```bash
   mkdir -p artifacts/finance/repo/<slug>/config/peers
   curl -fsSL https://peer01.example/v1/configuration \
     | jq '.' \
     > artifacts/finance/repo/<slug>/config/peers/peer01.json
   ```
2. 對每個快照 (`sha256sum config/peers/*.json`) 進行哈希處理並記錄下一個摘要
   到治理數據包模板中的對等 ID。這證明了哪些同行
   攝取策略以及哪個提交/工具鏈生成了快照。
3. 將每個快照中的 `.settlement.repo` 塊與暫存的塊進行比較
   `[settlement.repo]` TOML 片段；記錄任何漂移並重新運行
   `repo query get --agreement-id <id> --pretty` 所以證據包顯示
   運行時配置和標準化 `RepoGovernance` 值
   與協議一起存儲。 【crates/iroha_cli/src/main.rs:3821】
4. 將快照文件和摘要索引附加到證據清單（請參閱
   §3.2）因此治理記錄將批准的變更鏈接到實際的同行
   配置字節。治理模板已更新以包含此內容
   表，因此每個未來的回購數據包都帶有相同的證明。

捕獲這些快照可以彌補 `iroha_config` 文檔中指出的空白
路線圖中：審閱者現在可以將分階段的 TOML 與每個字節進行比較
同行報告，只要回購協議發生變化，審計師就可以重新進行比較
正在調查中。

## 3. 確定性證據工作流程1. **記錄指令出處**
   - 通過 `iroha app repo ... --output` 生成存儲庫/展開有效負載。
   - 將 `InstructionBox` JSON 存儲在
     `artifacts/finance/repo/<agreement-id>/initiation.json`。
2. **捕獲賬本狀態**
   - 之前運行 `iroha app repo query list --pretty > artifacts/.../agreements.json`
     並在結算後證明餘額已結清。
   - 可選擇通過 `iroha json` 或 SDK 幫助程序查詢 `FindAssets` 進行存檔
     回購分支中觸及的資產餘額。
3. **持久化事件流**
   - 通過 Torii SSE 訂閱 `AccountEvent::Repo` 或拉出並附加
     將 JSON 發送到證據目錄。這滿足了防篡改
     日誌記錄子句，因為事件是由觀察到的對等方簽名的
     每一次改變。
4. **運行確定性測試**
   - CI 已經運行 `integration_tests/tests/repo.rs`；對於手動簽核，
     執行`cargo test -p integration_tests repo::`並歸檔日誌加上
     `target/debug/deps/repo-*` JUnit 輸出。
5. **序列化治理和配置**
   - 簽入（或附加）該期間使用的 `[settlement.repo]` 配置，
     包括理髮/合格名單。這使得審核重播能夠匹配
     運行時規範化治理記錄在 `RepoAgreement` 中。

### 3.1 證據包佈局

將本節中提到的所有工件存儲在單一協議下
目錄，以便治理可以歸檔或散列一棵樹。推薦的佈局是：

```
artifacts/finance/repo/<agreement-id>/
├── agreements_before.json
├── agreements_after.json
├── initiation.json
├── unwind.json
├── margin/
│   └── 2026-04-30.json
├── events/
│   └── repo-events.ndjson
├── config/
│   ├── settlement_repo.toml
│   └── peers/
│       ├── peer01.json
│       └── peer02.json
├── repo_proof_snapshot.json
├── repo_proof_digest.txt
└── tests/
    └── repo_lifecycle.log
```

- `agreements_before/after.json` 捕獲 `repo query list` 輸出，以便審核員可以
  證明賬本已清除協議。
- `initiation.json`、`unwind.json` 和 `margin/*.json` 與 Norito 完全相同
  有效負載以 `iroha app repo ... --output` 上演。
- `events/repo-events.ndjson` 重播 `AccountEvent::Repo(*)` 流，同時
  `tests/repo_lifecycle.log` 保留 `cargo test` 證據。
- `repo_proof_snapshot.json` 和 `repo_proof_digest.txt` 來自快照
  刷新§2.7中的過程並讓審閱者重新計算生命週期哈希
  無需重新運行線束。
- `config/settlement_repo.toml` 包含 `[settlement.repo]` 片段
  （理髮、替換矩陣）在執行回購協議時處於活動狀態。
- `config/peers/*.json` 捕獲每個對等點的 `/v1/configuration` 快照，
  關閉暫存 TOML 和運行時值同行報告之間的循環
  超過 Torii。

### 3.2 哈希清單生成

將確定性清單附加到每個包，以便審核者可以驗證哈希值
無需解壓存檔。 `scripts/repo_evidence_manifest.py` 的助手
走協議目錄，記錄`size`，`sha256`，`blake2b`，最後一個
修改每個文件的時間戳，並寫入 JSON 摘要：

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/finance/repo/wonderland-2026q1 \
  --agreement-id 7mxD1tKRyv32je4kZwcWa9wa33bX \
  --output artifacts/finance/repo/wonderland-2026q1/manifest.json \
  --exclude 'scratch/*'
```

生成器按字典順序對路徑進行排序，在輸出文件存在時跳過它
在同一目錄中，並發出治理可以直接複製的總計
進入改簽機票。當省略 `--output` 時，清單將打印到
標準輸出，方便在案頭審查期間進行快速比較。
使用 `--exclude <glob>` 省略臨時材料（例如，`--exclude 'scratch/*' --exclude '*.tmp'`）
無需將文件移出捆綁包； glob 模式始終適用於
相對於 `--root` 的路徑。示例清單（為簡潔起見被截斷）：

```json
{
  "agreement_id": "7mxD1tKRyv32je4kZwcWa9wa33bX",
  "generated_at": "2026-04-30T11:58:43Z",
  "root": "/var/tmp/repo/wonderland-2026q1",
  "file_count": 5,
  "total_bytes": 1898,
  "files": [
    {
      "path": "agreements_after.json",
      "size": 512,
      "sha256": "6b6ca81b00d0d889272142ce1e6456872dd6b01ce77fcd1905f7374fc7c110cc",
      "blake2b": "5f0c7f03d15cd2a69a120f85df2a4a4a219a716e1f2ec5852a9eb4cdb443cbfe3c1e8cd02b3b7dbfb89ab51a1067f4107be9eab7d5b46a957c07994eb60bb070",
      "modified_at": "2026-04-30T11:42:01Z"
    },
    {
      "path": "initiation.json",
      "size": 274,
      "sha256": "7a1a0ec8c8c5d43485c3fee2455f996191f0e17a9a7d6b25fc47df0ba8de91e7",
      "blake2b": "ce72691b4e26605f2e8a6486d2b43a3c2b472493efd824ab93683a1c1d77e4cff40f5a8d99d138651b93bcd1b1cb5aa855f2c49b5f345d8fac41f5b221859621",
      "modified_at": "2026-04-30T11:39:55Z"
    }
  ]
}
```

將清單包含在證據包旁邊並引用其 SHA-256 哈希值
在治理提案中，各部門、運營商和審計員共享相同的信息
基本事實。

### 3.3 治理變更日誌和回滾演練

金融委員會預計每一次回購請求、削減調整或替代
矩陣更改以可鏈接的可複制治理數據包的方式到達
直接來自公投記錄。 【docs/source/governance_playbook.md:1】

1. **構建治理包**
   - 將協議的證據包複製到
     `artifacts/finance/repo/<agreement-id>/governance/`。
   - 添加`gar.json`（理事會批准記錄），`referendum.md`（誰批准
     以及他們審查了哪些哈希值），以及 `rollback_playbook.md`
     總結 `repo_runbook.md` 的逆轉過程
     §§4–5.【docs/source/finance/repo_runbook.md:1】
   - 捕獲第 3.2 節中的確定性清單哈希
     `hashes.txt`，以便審核者可以確認他們在 Torii 匹配中看到的有效負載
     分階段的字節。
2. **參考公投中的數據包**
   - 運行 `iroha app governance referendum submit`（或等效的 SDK
     helper）將來自 `hashes.txt` 的清單哈希包含在 `--notes` 中
     有效負載，因此 GAR 指向不可變的數據包。
   - 在治理跟踪器或票務系統中歸檔相同的哈希值，以便
     審計跟踪不依賴於儀表板屏幕截圖。
3. **記錄演練和回滾**
   - 公投通過後，使用存儲庫更新 `ops/drill-log.md`
     協議 ID、部署的配置哈希、GAR ID 和操作員聯繫方式，以便
     季度演練記錄包括財務行動。 【ops/drill-log.md:1】
   - 如果執行回滾演習，請附上簽名的
     `rollback_playbook.md` 和 `iroha app repo unwind` 下的 CLI 輸出
     `governance/drills/<timestamp>.log` 並使用相同的信息通知理事會
     治理手冊中描述的步驟。

佈局示例：

```
artifacts/finance/repo/<agreement-id>/governance/
├── gar.json
├── hashes.txt
├── referendum.md
├── rollback_playbook.md
└── drills/
    └── 2026-05-12T09-00Z.log
```

將 GAR、公投和演練工件與生命週期保持一致
有證據保證每次回購變更都滿足 F1 治理路線圖
酒吧，無需稍後購買定制門票。

### 3.4 生命週期治理清單

路線圖 **F1** 提出了啟動、應計/利潤和的治理覆蓋範圍
三方放鬆。下表匯總了確定性的批准
工件，並測試每個生命週期步驟的參考，以便財務部門可以引用
組裝數據包時的單一清單。|生命週期步驟|所需的批准和門票 |確定性工件和命令 |鏈接回歸覆蓋率 |
|----------------|------------------------------------------|------------------------------------------------|----------------------------|
| **發起（雙邊或三方）** |通過 `docs/examples/finance/repo_governance_packet_template.md` 記錄的雙控制簽核、具有 `[settlement.repo]` diff 和 GAR ID 的治理票據、設置 `--custodian` 時的託管人確認。 |通過`iroha --config client.toml --output repo initiate ...` 暫存指令。 發出生命週期證明快照（`REPO_PROOF_*` 環境變量）以及來自 `scripts/repo_evidence_manifest.py` 的捆綁清單。 附加最新的 `FindRepoAgreements` JSON 和 `[settlement.repo]` 片段（理髮、合格列表、替換矩陣）。 | `integration_tests/tests/repo.rs::repo_roundtrip_transfers_balances_and_clears_agreement`（雙邊）和 `integration_tests/tests/repo.rs::repo_roundtrip_with_custodian_routes_collateral`（三方）證明運行時與分階段的有效負載匹配。 |
| **追加保證金應計節奏** |辦公桌領導 + 風險經理批准治理包中記錄的節奏窗口；票證引用預定的 `RepoMarginCallIsi`。 |在調用 `iroha app repo margin-call` 之前捕獲 `iroha app repo margin --agreement-id` 輸出，對生成的 JSON 進行哈希處理，並將 `RepoAccountEvent::MarginCalled` SSE 負載存檔在證據包中。 將 CLI 日誌存儲在確定性證明哈希旁邊。 | `integration_tests/tests/repo.rs::repo_margin_call_enforces_cadence_and_participant_rules` 保證運行時拒絕過早調用和非參與者提交。 |
| **抵押品替代和到期解除** |治理變更記錄引用了所需的 `collateral_substitution_matrix` 條目和削減政策；理事會會議記錄列出了替換對 SHA-256 哈希值。 |使用 `iroha app repo unwind --output ... --settlement-timestamp-ms <planned>` 暫存展開部分，以便 ACT/360 計算 (§2.8) 和替換有效負載都可重現。 將 `[settlement.repo]` TOML 片段、替換清單以及生成的 `RepoAccountEvent::Settled` 有效負載包含在工件包中。 | `integration_tests/tests/repo.rs::repo_roundtrip_transfers_balances_and_clears_agreement` 內的替換往返執行不足與批准的替換流程，同時保持協議 ID 不變。 |
| **緊急放鬆/回滾演練** |事件指揮官 + 財務委員會批准 `docs/source/finance/repo_runbook.md`（第 4-5 節）中所述的回滾，並捕獲 `ops/drill-log.md` 中的條目。 |使用分階段回滾負載執行 `iroha app repo unwind`，將 CLI 日誌 + GAR 引用附加到 `governance/drills/<timestamp>.log`，然後重新運行 `repo_deterministic_lifecycle_proof_matches_fixture` 和 `scripts/repo_evidence_manifest.py` 幫助程序以證明演練之前/之後的確定性。 |快樂路徑展開由 `integration_tests/tests/repo.rs::repo_roundtrip_transfers_balances_and_clears_agreement` 涵蓋；遵循演練步驟可以使治理工件與該測試所執行的運行時保證保持一致。 |

**桌面時間表。 **1. 複製攝入模板，填寫元數據塊（協議 ID、GAR 票證、
   保管人、配置哈希），並創建證據目錄。
2. 將每條指令（`initiate`、`margin-call`、`unwind`、替換）暫存在
   `--output` 模式，散列 JSON，並在每個散列旁邊記錄批准。
3. 在暫存後立即發出生命週期證明快照和清單，以便
   治理審查者可以使用相同的回購協議重新計算摘要。
4. 鏡像受影響帳戶的 `RepoAccountEvent::*` SSE 負載並刪除
   `artifacts/finance/repo/<agreement-id>/events.ndjson` 中導出的 NDJSON
   在歸檔數據包之前。
5. 投票通過後，用 GAR 標識符更新 `hashes.txt`，
   配置哈希和清單校驗和，以便理事會可以跟踪部署
   無需重新運行本地腳本。

### 3.5 治理包快速入門

路線圖 F1 審閱者要求提供一份簡明的清單，以便他們在
組裝證據包。每當有回購請求時，請遵循以下順序
或政策變化正走向治理：

1. **導出生命週期證明工件。 **
   ```bash
   mkdir -p artifacts/finance/repo/<slug>
   REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<slug>/repo_proof_snapshot.json \
   REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<slug>/repo_proof_digest.txt \
   cargo test -p iroha_core \
     -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
   ```
   導出的 JSON + 摘要鏡像了下簽入的裝置
   `crates/iroha_core/tests/fixtures/`，以便審閱者可以重新計算生命週期
   框架而無需重新運行整個套件（請參閱§2.7）。您也可以致電
   `scripts/regen_repo_proof_fixture.sh --bundle-dir artifacts/finance/repo/<slug>`
   一步刷新並複制相同的文件。
2. **暫存並散列每條指令。 ** 生成啟動/保證金/展開
   有效負載為 `iroha app repo ... --output`。捕獲每個文件的 SHA-256
   （存儲在 `hashes/` 下）所以 `docs/examples/finance/repo_governance_packet_template.md`
   可以引用辦公桌審查的相同字節。
3. **保存賬本/配置快照。 ** 導出之前/之後的 `repo query list` 輸出
   結算，轉儲將要應用的 `[settlement.repo]` TOML 塊，以及
   將相關 `AccountEvent::Repo(*)` SSE 鏡像到
   `artifacts/finance/repo/<slug>/events/repo-events.ndjson`。 GAR之後
   通過，捕獲每個對等點的 `/v1/configuration` 快照（第 2.9 節）並存儲它們
   在 `config/peers/` 下，因此治理數據包證明部署成功。
4. **生成證據清單。 **
   ```bash
   python3 scripts/repo_evidence_manifest.py \
     --root artifacts/finance/repo/<slug> \
     --agreement-id <repo-id> \
     --output artifacts/finance/repo/<slug>/manifest.json
   ```
   將清單哈希包含在治理票證或 GAR 分鐘內，以便
   審核員可以在不下載原始包的情況下區分數據包（請參閱第 3.2 節）。
5. **組裝數據包。 ** 從以下位置複製模板
   `docs/examples/finance/repo_governance_packet_template.md`，填寫元數據，
   附加證明快照/摘要、清單、配置哈希、SSE 導出和測試
   日誌，然後在全民投票 `--notes` 字段中引用清單 SHA-256。
   將完成的 Markdown 存儲在工件旁邊，以便回滾繼承
   您運送以供批准的確切證據。

在暫存存儲庫請求後立即運行上述步驟意味著
理事會一召開，治理包就已準備就緒，避免了最後一刻
爭先恐後地重新創建哈希值或事件流。

## 4. 三方託管和抵押品替代- **託管人：** 通過 `--custodian <account>` 路線抵押品
  保管金庫；運行時強制帳戶存在並發出角色標記
  事件，以便保管人可以協調 (`RepoAccountRole::Custodian`)。國家
  機器拒絕託管人與任何一方匹配的協議。
- **抵押品替代：** 平倉部分可能會提供不同的抵押品
  替換期間的數量/系列，只要它**不小於**
  質押金額*和*替換矩陣允許配對； `ReverseRepoIsi`
  強制執行這兩個條件
  (`crates/iroha_core/src/smartcontracts/isi/repo.rs:414`–`437`)。整合
  測試套件同時測試拒絕路徑和成功替換
  往返 (`integration_tests/tests/repo.rs:261`–`359`)，而回購單元
  測試涵蓋了新的矩陣策略。
- **ISO 20022 映射：** 構建 ISO 信封或協調外部時
  系統，重用記錄在
  `docs/source/finance/settlement_iso_mapping.md` (`colr.007`, `sese.023`,
  `sese.025`），因此 Norito 有效負載和 ISO 確認保持同步。

## 5. 操作清單

### 每日開盤前

1. 通過`iroha app repo query list`導出未完成的協議。
2. 與庫存庫存進行比較並確保合格的抵押品配置
   與計劃的書相符。
3. 使用 `--output` 暫存即將到來的回購/解除並收集雙重批准。

### 日內監控

1. Subscribe to `AccountEvent::Repo` for initiator/counterparty/custodian
   賬戶；當意外啟動發生時發出警報。
2. 使用 `iroha app repo margin --agreement-id ID`（或
   `RepoAgreementRecord::next_margin_check_after`) 每小時檢測一次踏頻
   漂移；當 `is_due = true` 時觸發 `repo margin-call`。
3. 記錄所有帶有操作員姓名首字母的追加保證金通知，並將 CLI JSON 輸出附加到
   證據目錄。

### 日終+結算後

1. 重新運行 `repo query list` 並確認已解除的協議已被刪除。
2. 歸檔 `RepoAccountEvent::Settled` 有效負載並交叉檢查現金/抵押品
   通過 `FindAssets` 進行餘額。
3. 當回購演習或事件測試時，在 `ops/drill-log.md` 中歸檔演習條目
   跑；重用 `scripts/telemetry/log_sorafs_drill.sh` 時間戳約定。

## 6. 欺詐和回滾程序

- **雙重控制：**始終使用 `--output` 生成指令並存儲
  用於共同簽名的 JSON。在流程級別拒絕單方提交
  即使運行時強制執行發起者權限。
- **防篡改日誌記錄：** 將 `RepoAccountEvent` 流鏡像到您的
  SIEM，因此任何偽造的指令都可以被檢測到（缺少對等簽名）。
- **回滾：** 如果必須提前解除倉庫，請提交 `repo unwind`
  使用相同的協議 ID 並在您的事件中附加 `--notes` 字段
  跟踪器引用 GAR 批准的回滾劇本。
- **欺詐升級：** 如果出現未經授權的存儲庫，請導出違規內容
  `RepoAccountEvent` 有效負載，通過治理策略凍結帳戶，以及
  根據回購治理 SOP 通知理事會。

## 7. 報告和跟進

### 7.1 財務對賬和分類賬證據路線圖 **F1** 和全球定居點護欄 (roadmap.md#L1975-L1978)
要求每次回購審查都包括確定性的財務證明。製作一個
按照下面的清單按季度捆綁每本書。

1. **快照餘額。 ** 使用支持的 `FindAssets` 查詢
   `iroha ledger asset list` (`crates/iroha_cli/src/main_shared.rs`) 或
   `iroha_python` 幫助程序導出 `soraカタカナ...` 的異或餘額，
   `soraカタカナ...`，以及參與審核的每個台賬。商店
   下的 JSON
   `artifacts/finance/repo/<period>/treasury_assets.json`並記錄git
   隨附的 `README.md` 中的提交/工具鏈。
2. **交叉檢查賬本預測。 ** 重新運行
   `sorafs reserve ledger --quote <...> --json-out ...` 並標準化輸出
   通過 `scripts/telemetry/reserve_ledger_digest.py`。將摘要放在旁邊
   資產快照，以便審計人員可以將 XOR 總數與回購協議進行比較
   無需重放 CLI 即可進行賬本投影。
3. **發布調節說明。 ** 總結增量
   `artifacts/finance/repo/<period>/treasury_reconciliation.md` 參考：
   資產快照哈希、賬本摘要哈希以及所涵蓋的協議。
   鏈接財務治理跟踪器中的註釋，以便審閱者可以確認
   批准回購發行之前的財務覆蓋範圍。

### 7.2 演練和回滾演練證據

驗收標準還要求分階段回滾和事件演習。每個
演習或混亂排練必須收集以下文物：

1. `repo_runbook.md` 第 4-5 節檢查表，由事故指揮官簽署，並且
   財務委員會。
2. 排練的 CLI/SDK 日誌 (`repo initiate|margin-call|unwind`) 以及
   已存儲刷新的生命週期證明快照和證據清單 (§§2.7–3.2)
   在 `artifacts/finance/repo/drills/<timestamp>/` 下。
3. Alertmanager 或尋呼機轉錄顯示注入的信號和
   確認軌跡。將成績單放在鑽探製品旁邊，然後
   包括使用時的 Alertmanager 靜音 ID。
4. 引用 GAR id、清單哈希和鑽取的 `ops/drill-log.md` 條目
   捆綁路徑，以便將來的審核可以跟踪排練，而無需抓取聊天日誌。

### 7.3 治理跟踪器和文檔衛生

- 將此文檔 `repo_runbook.md` 和財務治理跟踪器保存在
  每當 CLI/SDK 或運行時行為發生變化時，都會保持同步；審稿人期望
  驗收表以保持準確。
- 附上完整的證據包（`agreements.json`、分階段說明、SSE
  成績單、配置快照、協調、演練工件和測試
  記錄）到跟踪器以進行每個季度的審查。
- 協調時參考 `docs/source/finance/settlement_iso_mapping.md`
  與 ISO 橋操作員保持一致，以便跨系統協調保持一致。

通過遵循本指南，運營商可以滿足路線圖 F1 接受標準：
捕獲確定性證明，三方和替代流
記錄和治理程序（雙重控制+事件記錄）
編碼樹內。