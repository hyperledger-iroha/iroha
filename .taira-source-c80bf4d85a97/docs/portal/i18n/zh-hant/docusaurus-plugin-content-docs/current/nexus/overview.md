---
id: nexus-overview
lang: zh-hant
direction: ltr
source: docs/portal/docs/nexus/overview.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Sora Nexus overview
description: High-level summary of the Iroha 3 (Sora Nexus) architecture with pointers to the canonical mono-repo docs.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Nexus (Iroha 3) 通過多通道執行、治理範圍擴展了 Iroha 2
數據空間以及跨每個 SDK 的共享工具。此頁面反映了新的
`docs/source/nexus_overview.md` 在 mono-repo 中進行了簡要介紹，以便門戶讀者可以
快速了解架構各個部分如何組合在一起。

## 發佈線

- **Iroha 2** – 用於聯盟或專用網絡的自託管部署。
- **Iroha 3 / Sora Nexus** – 運營商所在的多車道公共網絡
  註冊數據空間（DS）並繼承共享治理、結算和
  可觀察性工具。
- 兩條線均從同一工作區（IVM + Kotodama 工具鏈）編譯，因此 SDK
  修復、ABI 更新和 Norito 裝置仍然可移植。運營商下載
  `iroha3-<version>-<os>.tar.zst` 捆綁包加入 Nexus；參考
  `docs/source/sora_nexus_operator_onboarding.md` 用於全屏清單。

## 構建塊

|組件|總結|門戶掛鉤|
|------------|---------|--------------|
|數據空間（DS）|治理定義的執行/存儲域，擁有一個或多個通道，聲明驗證器集、隱私類別、費用 + DA 策略。 |有關清單架構，請參閱 [Nexus 規範](./nexus-spec)。 |
|車道 |執行的確定性分片；發出全球 NPoS 環令的承諾。通道類別包括 `default_public`、`public_custom`、`private_permissioned` 和 `hybrid_confidential`。 | [車道模型](./nexus-lane-model) 捕獲幾何圖形、存儲前綴和保留。 |
|過渡計劃|佔位符標識符、路由階段和雙配置文件打包跟踪單通道部署如何演變為 Nexus。 | [轉換說明](./nexus-transition-notes) 記錄每個遷移階段。 |
|空間目錄|存儲 DS 清單 + 版本的註冊表合同。操作員在加入之前根據此目錄協調目錄條目。 |清單差異跟踪器位於 `docs/source/project_tracker/nexus_config_deltas/` 下。 |
|巷目錄| `[nexus]` 配置部分，將通道 ID 映射到別名、路由策略和 DA 閾值。 `irohad --sora --config … --trace-config` 打印已解析的目錄以供審核。 |使用 `docs/source/sora_nexus_operator_onboarding.md` 進行 CLI 演練。 |
|結算路由器|連接私人 CBDC 通道與公共流動性通道的 XOR 傳輸協調器。 | `docs/source/cbdc_lane_playbook.md` 詳細說明了策略旋鈕和遙測門。 |
|遙測/SLO | `dashboards/grafana/nexus_*.json` 下的儀表板 + 警報捕獲通道高度、DA 積壓、結算延遲和治理隊列深度。 | [遙測修復計劃](./nexus-telemetry-remediation) 詳細說明了儀表板、警報和審計證據。 |

## 推出快照

|相|焦點 |退出標準|
|--------|---------|----------------|
| N0 – 內測 |理事會管理的註冊商 (`.sora`)、手動操作員入職、靜態車道目錄。 |簽署的 DS 清單 + 排練的治理交接。 |
| N1 – 公開發布 |新增`.nexus`後綴、拍賣、自助註冊、異或結算接線。 |解析器/網關同步測試、計費協調儀表板、爭議桌面演習。 |
| N2 – 擴展 |推出 `.dao`、經銷商 API、分析、爭議門戶、管理員記分卡。 |合規工件版本化、在線政策陪審團工具包、財務透明度報告。 |
| NX-12/13/14門|合規引擎、遙測儀表板和文檔必須在合作夥伴試點之前一起交付。 | [Nexus 概述](./nexus-overview) + [Nexus 操作](./nexus-operations) 已發布，儀表板已連接，策略引擎已合併。 |

## 操作員職責

1. **配置衛生** – 保持 `config/config.toml` 與已發布的通道同步 &
   數據空間目錄；將 `--trace-config` 輸出與每個發行票一起存檔。
2. **清單跟踪** – 使目錄條目與最新的空間保持一致
   加入或升級節點之前的目錄捆綁。
3. **遙測覆蓋** – 公開 `nexus_lanes.json`、`nexus_settlement.json`、
   以及相關的 SDK 儀表板；將警報發送至 PagerDuty 並根據遙測修復計劃進行季度審查。
4. **事件報告** – 遵循嚴重性矩陣
   [Nexus 操作](./nexus-operations) 並在五個工作日內提交 RCA。
5. **治理準備** – 參加影響您的車道的 Nexus 理事會投票
   每季度排練回滾指令（通過跟踪
   `docs/source/project_tracker/nexus_config_deltas/`）。

## 另請參閱

- 規範概述：`docs/source/nexus_overview.md`
- 詳細規格：[./nexus-spec](./nexus-spec)
- 車道幾何形狀：[./nexus-lane-model](./nexus-lane-model)
- 過渡計劃：[./nexus-transition-notes](./nexus-transition-notes)
- 遙測修復計劃：[./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- 操作手冊：[./nexus-operations](./nexus-operations)
- 操作員入門指南：`docs/source/sora_nexus_operator_onboarding.md`