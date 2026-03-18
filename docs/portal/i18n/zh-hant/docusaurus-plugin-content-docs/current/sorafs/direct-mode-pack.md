---
id: direct-mode-pack
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/direct-mode-pack.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Direct-Mode Fallback Pack (SNNet-5a)
sidebar_label: Direct-Mode Fallback Pack
description: Required configuration, compliance checks, and rollout steps when operating SoraFS in direct Torii/QUIC mode during the SNNet-5a transition.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意規範來源
:::

SoraNet 電路仍然是 SoraFS 的默認傳輸，但路線圖項目 **SNNet-5a** 需要受監管的回退，以便操作員可以在匿名推出完成時保持確定性的讀取訪問。該包捕獲在直接 Torii/QUIC 模式下運行 SoraFS 所需的 CLI/SDK 旋鈕、配置文件、合規性測試和部署清單，而無需觸及隱私傳輸。

回退適用於暫存和受監管的生產環境，直到 SNNet-5 到 SNNet-9 清除其準備就緒大門。將下面的工件與通常的 SoraFS 部署資料一起保存，以便運營商可以根據需要在匿名模式和直接模式之間切換。

## 1. CLI 和 SDK 標誌

- `sorafs_cli fetch --transport-policy=direct-only …` 禁用中繼調度並強制執行 Torii/QUIC 傳輸。 CLI 幫助現在將 `direct-only` 列為可接受的值。
- SDK 每當公開“直接模式”切換時都必須設置 `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)`。 `iroha::ClientOptions` 和 `iroha_android` 中生成的綁定轉發相同的枚舉。
- 網關線束（`sorafs_fetch`，Python 綁定）可以通過共享的 Norito JSON 幫助程序解析僅直接切換，以便自動化接收相同的行為。

在面向合作夥伴的運行手冊中記錄該標誌，並通過 `iroha_config` 而不是環境變量來切換功能。

## 2. 網關策略配置文件

使用 Norito JSON 保留確定性協調器配置。 `docs/examples/sorafs_direct_mode_policy.json` 中的示例配置文件編碼：

- `transport_policy: "direct_only"` — 拒絕僅宣傳 SoraNet 中繼傳輸的提供商。
- `max_providers: 2` — 將直接對等點限制為最可靠的 Torii/QUIC 端點。根據地區合規津貼進行調整。
- `telemetry_region: "regulated-eu"` — 標記發出的指標，以便遙測儀表板和審核區分後備運行。
- 保守的重試預算（`retry_budget: 2`、`provider_failure_threshold: 3`）以避免掩蓋配置錯誤的網關。

在向操作員公開策略之前，通過 `sorafs_cli fetch --config`（自動化）或 SDK 綁定 (`config_from_json`) 加載 JSON。保留記分板輸出 (`persist_path`) 以進行審計跟踪。

網關端強制旋鈕在 `docs/examples/sorafs_gateway_direct_mode.toml` 中捕獲。該模板鏡像 `iroha app sorafs gateway direct-mode enable` 的輸出，禁用信封/准入檢查、連接速率限制默認值，並使用計劃派生的主機名和清單摘要填充 `direct_mode` 表。在將代碼片段提交到配置管理之前，將佔位符值替換為您的部署計劃。

## 3. 合規性測試套件

直接模式準備就緒現在包括 Orchestrator 和 CLI 包中的覆蓋範圍：

- 當每個候選廣告僅支持SoraNet中繼時，`direct_only_policy_rejects_soranet_only_providers`保證`TransportPolicy::DirectOnly`快速失敗。 【crates/sorafs_orchestrator/src/lib.rs:7238】
- `direct_only_policy_prefers_direct_transports_when_available` 確保存在 Torii/QUIC 傳輸時使用，並且 SoraNet 中繼被排除在會話之外。 【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` 解析 `docs/examples/sorafs_direct_mode_policy.json` 以確保文檔與幫助程序實用程序保持一致。 【crates/sorafs_orchestrator/src/lib.rs:7509】【docs/examples/sorafs_direct_mode_policy.json:1】
- `fetch_command_respects_direct_transports` 針對模擬的 Torii 網關練習 `sorafs_cli fetch --transport-policy=direct-only`，為固定直接傳輸的受監管環境提供煙霧測試。 【crates/sorafs_car/tests/sorafs_cli.rs:2733】
- `scripts/sorafs_direct_mode_smoke.sh` 將相同的命令與策略 JSON 和記分板持久性包裝在一起，以實現部署自動化。

在發布更新之前運行重點套件：

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

如果工作區編譯由於上游更改而失敗，請在 `status.md` 中記錄阻塞錯誤，並在依賴項趕上後重新運行。

## 4. 自動煙霧運行

僅 CLI 覆蓋範圍不會顯示特定於環境的回歸（例如，網關策略漂移或明顯不匹配）。 `scripts/sorafs_direct_mode_smoke.sh` 中有一個專用的煙霧助手，並將 `sorafs_cli fetch` 與直接模式編排器策略、記分板持久性和摘要捕獲封裝在一起。

用法示例：

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```

- 該腳本尊重 CLI 標誌和 key=value 配置文件（請參閱 `docs/examples/sorafs_direct_mode_smoke.conf`）。在運行之前使用生產值填充清單摘要和提供商廣告條目。
- `--policy` 默認為 `docs/examples/sorafs_direct_mode_policy.json`，但可以提供由 `sorafs_orchestrator::bindings::config_to_json` 生成的任何編排器 JSON。 CLI 通過 `--orchestrator-config=PATH` 接受策略，從而無需手動調整標誌即可實現可重複運行。
- 當 `sorafs_cli` 不在 `PATH` 上時，幫助程序從
  `sorafs_orchestrator` 板條箱（釋放配置文件）因此煙霧運行鍛煉了
  運輸直接模式管道。
- 輸出：
  - 組裝的有效負載（`--output`，默認為 `artifacts/sorafs_direct_mode/payload.bin`）。
  - 獲取包含遙測區域和用於推出證據的提供商報告的摘要（`--summary`，默認與有效負載一起）。
  - 記分板快照保留到策略 JSON 中聲明的路徑（例如，`fetch_state/direct_mode_scoreboard.json`）。將其與變更單中的摘要一起存檔。
- 採用門自動化：獲取完成後，幫助程序使用持久記分板和摘要路徑調用 `cargo xtask sorafs-adoption-check`。所需的仲裁默認為命令行上提供的提供程序的數量；當您需要更大的樣本時，用 `--min-providers=<n>` 覆蓋它。採用報告寫在摘要旁邊（`--adoption-report=<path>` 可以設置自定義位置），並且每當您提供匹配的 CLI 標誌時，助手都會默認傳遞 `--require-direct-only`（匹配後備）和 `--require-telemetry`。使用 `XTASK_SORAFS_ADOPTION_FLAGS` 轉發其他 xtask 參數（例如，在批准的降級期間使用 `--allow-single-source`，以便門既容忍又強制執行回退）。僅在運行本地診斷時跳過 `--skip-adoption-check` 的採用門；該路線圖要求每次受監管的直接模式運行都包含採用報告包。

## 5. 推出清單

1. **配置凍結：** 將直接模式 JSON 配置文件存儲在 `iroha_config` 存儲庫中，並將哈希值記錄在更改票證中。
2. **網關審核：** 在翻轉直接模式之前確認 Torii 端點強制執行 TLS、功能 TLV 和審核日誌記錄。將網關策略模板發布給運營商。
3. **合規性簽字：** 與合規性/監管審查人員共享更新後的劇本，並獲得在匿名覆蓋之外運行的批准。
4. **試運行：** 執行合規性測試套件以及針對已知良好的 Torii 提供商的暫存獲取。存檔記分板輸出和 CLI 摘要。
5. **生產切換：** 宣布更改窗口，將 `transport_policy` 翻轉為 `direct_only`（如果您選擇了 `soranet-first`），並監控直接模式儀表板（`sorafs_fetch` 延遲、提供商故障計數器）。記錄回滾計劃，以便您可以在 SNNet-4/5/5a/5b/6a/7/8/12/13 在 `roadmap.md:532` 中畢業後首先返回到 SoraNet。
6. **變更後審核：** 將記分板快照、獲取摘要和監控結果附加到變更單中。使用生效日期和任何異常情況更新 `status.md`。

將清單與 `sorafs_node_ops` 操作手冊放在一起，以便操作員可以在實時切換之前排練工作流程。當 SNNet-5 升級到 GA 時，在確認生產遙測中的奇偶校驗後退出後備。

## 6. 證據和收養門要求

直接模式捕獲仍然需要滿足 SF-6c 採用門檻。捆綁
每次運行的記分板、摘要、清單信封和採用報告
`cargo xtask sorafs-adoption-check` 可以驗證回退姿勢。失踪
字段迫使門失敗，因此在變化中記錄預期的元數據
門票。

- **傳輸元數據：** `scoreboard.json` 必須聲明
  `transport_policy="direct_only"`（和翻轉 `transport_policy_override=true`
  當您強制降級時）。保留配對的匿名策略字段
  即使它們繼承了默認值，也會被填充，以便審閱者可以看到您是否
  偏離了分階段的匿名計劃。
- **提供商計數器：** 僅網關會話必須持續 `provider_count=0`
  並用 Torii 提供者的數量填充 `gateway_provider_count=<n>`
  使用過。避免手動編輯 JSON — CLI/SDK 已導出計數並
  採用門拒絕忽略分割的捕獲。
- **顯性證據：** Torii網關參與時，傳遞簽名的
  `--gateway-manifest-envelope <path>`（或同等的 SDK）所以
  `gateway_manifest_provided` 加上 `gateway_manifest_id`/`gateway_manifest_cid`
  記錄在 `scoreboard.json` 中。確保 `summary.json` 攜帶匹配
  `manifest_id`/`manifest_cid`；如果任一文件是，則採用檢查失敗
  缺少這對。
- **遙測期望：** 當遙測伴隨捕獲時，運行
  門與 `--require-telemetry` 因此採用報告證明了指標
  發出。氣隙排練可以省略旗幟，但 CI 和改簽
  應記錄缺席情況。

示例：

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
```將 `adoption_report.json` 附在記分板、摘要、清單旁邊
信封和煙木捆。這些文物反映了 CI 採用工作的內容
(`ci/check_sorafs_orchestrator_adoption.sh`) 強制並保持直接模式
降級可審核。