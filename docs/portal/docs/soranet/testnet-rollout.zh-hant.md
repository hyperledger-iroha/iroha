---
lang: zh-hant
direction: ltr
source: docs/portal/docs/soranet/testnet-rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 887123dcafff50fb243d9788415b759da3691876e44b3cd7c800eede25a5ab09
source_last_modified: "2026-01-05T09:28:11.916414+00:00"
translation_last_reviewed: 2026-02-07
id: testnet-rollout
title: SoraNet testnet rollout (SNNet-10)
sidebar_label: Testnet Rollout (SNNet-10)
description: Phased activation plan, onboarding kit, and telemetry gates for SoraNet testnet promotions.
translator: machine-google-reviewed
---

:::注意規範來源
:::

SNNet-10 協調整個網絡中 SoraNet 匿名覆蓋的分階段激活。使用此計劃將路線圖項目符號轉化為具體的可交付成果、操作手冊和遙測門，以便每個操作員在 SoraNet 成為默認傳輸之前了解期望。

## 啟動階段

|相|時間表（目標）|範圍 |所需文物|
|--------|--------------------|--------------------|--------------------|
| **T0 — 封閉測試網** | 2026 年第四季度 | ≥3 個 ASN 中的 20-50 個中繼由核心貢獻者運營。 |測試網入門套件、守衛固定煙霧套件、基線延遲 + PoW 指標、掉電演練日誌。 |
| **T1 — 公開測試版** | 2027 年第一季度 | ≥100 個繼電器，啟用保護輪換，強制退出綁定，SDK beta 默認為帶有 `anon-guard-pq` 的 SoraNet。 |更新了入職工具包、操作員驗證清單、目錄發布 SOP、遙測儀表板包、事件演練報告。 |
| **T2 — 主網默認** | 2027 年第 2 季度（SNNet-6/7/9 完成時門控）|生產網絡默認為SoraNet； obfs/MASQUE 傳輸和 PQ 棘輪強制執行已啟用。 |治理批准會議紀要、僅直接回滾程序、降級警報、簽署的成功指標報告。 |

**沒有跳過路徑**——每個階段都必須在升級之前運送前一階段的遙測和治理工件。

## 測試網入門套件

每個中繼操作員都會收到一個包含以下文件的確定性包：

|文物|描述 |
|----------|-------------|
| `01-readme.md` |概述、聯繫點和時間表。 |
| `02-checklist.md` |飛行前檢查表（硬件、網絡可達性、防護策略驗證）。 |
| `03-config-example.toml` |最小 SoraNet 中繼 + 協調器配置與 SNNet-9 合規性塊保持一致，包括固定最新防護快照哈希的 `guard_directory` 塊。 |
| `04-telemetry.md` |連接 SoraNet 隱私指標儀表板和警報閾值的說明。 |
| `05-incident-playbook.md` |帶有升級矩陣的斷電/降級響應程序。 |
| `06-verification-report.md` |一旦冒煙測試通過，模板操作員就會完成並返回。 |

渲染副本位於 `docs/examples/soranet_testnet_operator_kit/` 中。每次促銷都會刷新套件；版本號跟踪階段（例如，`testnet-kit-vT0.1`）。

對於公共測試版 (T1) 操作員，`docs/source/soranet/snnet10_beta_onboarding.md` 中的簡明入門簡介總結了先決條件、遙測可交付成果和提交工作流程，同時指向確定性套件和驗證器幫助程序。

`cargo xtask soranet-testnet-feed` 生成 JSON 源，該源聚合了階段門模板引用的升級窗口、中繼名冊、指標報告、演練證據和附件哈希。首先使用 `cargo xtask soranet-testnet-drill-bundle` 簽署鑽探日誌和附件，以便源可以記錄 `drill_log.signed = true`。

## 成功指標

階段之間的晉升通過以下遙測數據進行控制，收集至少兩週：

- `soranet_privacy_circuit_events_total`：95%的電路完成，沒有掉電或降級事件；剩餘 5% 受 PQ 供應限制。
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}`：每天 <1% 的提取會話會在計劃的演練之外觸發斷電。
- `soranet_privacy_gar_reports_total`：預期 GAR 類別組合的偏差在 ±10% 以內；峰值必須由批准的政策更新來解釋。
- PoW 選票成功率：3s目標窗口內≥99%；通過 `soranet_privacy_throttles_total{scope="congestion"}` 報告。
- 每個區域的延遲（第 95 個百分位）：電路完全構建後 <200 毫秒，通過 `soranet_privacy_rtt_millis{percentile="p95"}` 捕獲。

儀表板和警報模板位於 `dashboard_templates/` 和 `alert_templates/` 中；將它們鏡像到您的遙測存儲庫中，並將它們添加到 CI lint 檢查中。在請求升級之前，使用 `cargo xtask soranet-testnet-metrics` 生成面向治理的報告。

階段關提交必須遵循 `docs/source/soranet/snnet10_stage_gate_template.md`，它鏈接到存儲在 `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md` 下的可立即復制的 Markdown 表單。

## 驗證清單

在進入每個階段之前，操作員必須簽署以下內容：

- ✅ 用當前入場信封籤署的接力廣告。
- ✅ 防護輪旋轉煙霧測試（`tools/soranet-relay --check-rotation`）通過。
- ✅ `guard_directory` 指向最新的 `GuardDirectorySnapshotV2` 工件，並且 `expected_directory_hash_hex` 與委員會摘要匹配（中繼啟動記錄經過驗證的哈希值）。
- ✅ PQ 棘輪指標 (`sorafs_orchestrator_pq_ratio`) 保持在請求階段的目標閾值之上。
- ✅ GAR 合規性配置與最新標籤匹配（請參閱 SNNet-9 目錄）。
- ✅ 降級警報模擬（禁用收集器，預計 5 分鐘內發出警報）。
- ✅ 使用記錄的緩解步驟執行 PoW/DoS 演練。

入門套件中包含預填充模板。操作員在收到生產憑證之前將完整的報告提交給治理幫助台。

## 治理和報告

- **變更控制：** 晉升需要治理委員會的批准，記錄在理事會會議記錄中並附在狀態頁面上。
- **狀態摘要：**發布每週更新，總結繼電器計數、PQ 比率、斷電事件和未完成的操作項目（節奏開始後存儲在 `docs/source/status/soranet_testnet_digest.md` 中）。
- **回滾：**維護已簽署的回滾計劃，在 30 分鐘內將網絡返回到上一階段，包括 DNS/guard 緩存失效和客戶端通信模板。

## 支持資產

- `cargo xtask soranet-testnet-kit [--out <dir>]` 將 `xtask/templates/soranet_testnet/` 中的入門套件具體化到目標目錄中（默認為 `docs/examples/soranet_testnet_operator_kit/`）。
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` 評估 SNNet-10 成功指標並發出適合治理審查的結構化通過/失敗報告。示例快照位於 `docs/examples/soranet_testnet_metrics_sample.json` 中。
- Grafana 和 Alertmanager 模板位於 `dashboard_templates/soranet_testnet_overview.json` 和 `alert_templates/soranet_testnet_rules.yml` 下；將它們複製到您的遙測存儲庫中或將它們連接到 CI lint 檢查中。
- SDK/門戶消息傳遞的降級通信模板位於 `docs/source/soranet/templates/downgrade_communication_template.md` 中。
- 每週狀態摘要應使用 `docs/source/status/soranet_testnet_weekly_digest.md` 作為規範形式。

拉取請求應與任何人工製品或遙測更改一起更新此頁面，以便推出計劃保持規範。