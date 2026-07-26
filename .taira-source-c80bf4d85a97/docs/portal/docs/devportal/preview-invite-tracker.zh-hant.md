---
lang: zh-hant
direction: ltr
source: docs/portal/docs/devportal/preview-invite-tracker.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dba1278df19fd3003f943bfc9f4168cd820a4cfcebc8a64daee248fa95e6f18f
source_last_modified: "2025-12-29T18:16:35.112669+00:00"
translation_last_reviewed: 2026-02-07
id: preview-invite-tracker
title: Preview invite tracker
sidebar_label: Preview tracker
description: Wave-by-wave status log for the checksum-gated docs portal preview program.
translator: machine-google-reviewed
---

該跟踪器記錄每個文檔門戶預覽波，以便 DOCS-SORA 所有者和
治理審核者可以查看哪個群組處於活動狀態、誰批准了邀請、
以及哪些文物仍需要關注。每當發送邀請時更新它，
撤銷或推遲，以便審計跟踪保留在存儲庫內。

## 波浪狀態

|波|隊列|追踪器問題 |批准人 |狀態 |目標窗口|筆記|
| ---| ---| ---| ---| ---| ---| ---|
| **W0 – 核心維護者** |文檔 + SDK 維護人員驗證校驗和流程 | `DOCS-SORA-Preview-W0` (GitHub/ops 跟踪器) |文檔/DevRel 負責人 + Portal TL | 🈴已完成 | 2025 年第 2 季度第 1-2 週 |邀請於 2025 年 3 月 25 日發送，遙測保持綠色，退出摘要於 2025 年 4 月 8 日發布。 |
| **W1 – 合作夥伴** | NDA 下的 SoraFS 運營商、Torii 集成商 | `DOCS-SORA-Preview-W1` |文檔/DevRel 領導 + 治理聯絡員 | 🈴已完成 | 2025 年第二季度第 3 週 |邀請於 2025-04-12 → 2025-04-26 進行，所有八個合作夥伴均已確認； [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) 中捕獲的證據和 [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) 中的退出摘要。 |
| **W2 – 社區** |精心策劃的社區候補名單（一次≤ 25 名）| `DOCS-SORA-Preview-W2` |文檔/DevRel 主管 + 社區經理 | 🈴已完成 | 2025 年第 3 季度第 1 週（暫定）|邀請於 2025-06-15 → 2025-06-29 運行，遙測始終呈綠色； [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md) 中捕獲的證據+發現。 |
| **W3 – 測試版群組** |金融/可觀測性測試版 + SDK 合作夥伴 + 生態系統倡導者 | `DOCS-SORA-Preview-W3` |文檔/DevRel 領導 + 治理聯絡員 | 🈴已完成 | 2026 年第一季度第 8 週 |邀請日期為 2026-02-18 → 2026-02-28；摘要 + 通過 `preview-20260218` 波生成的門戶數據（請參閱 [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)）。 |

> 注意：將每個跟踪器問題鏈接到相應的預覽請求票證並
> 將它們歸檔到 `docs-portal-preview` 項目下，以便保留批准
> 可發現的。

## 活動任務 (W0)

- ✅ 刷新預檢工件（GitHub Actions `docs-portal-preview` 運行 2025-03-24，描述符通過 `scripts/preview_verify.sh` 使用標籤 `preview-2025-03-24` 進行驗證）。
- ✅ 捕獲的遙測基線（`docs.preview.integrity`、`TryItProxyErrors` 儀表板快照保存到 W0 跟踪器問題）。
- ✅ 使用帶有預覽標籤 `preview-2025-03-24` 的 [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) 鎖定外展副本。
- ✅ 記錄前五個維護者的接收請求（票證 `DOCS-SORA-Preview-REQ-01` … `-05`）。
- ✅ 在連續 7 個綠色遙測日後，於 2025 年 3 月 25 日 10:00–10:20 發送前 5 個邀請；確認信息存儲在 `DOCS-SORA-Preview-W0` 中。
- ✅ 監控遙測 + 主機辦公時間（截至 2025 年 3 月 31 日每日簽到；檢查點日誌如下）。
- ✅ 收集中點反饋/問題並將其標記為 `docs-preview/w0`（請參閱 [W0 摘要](./preview-feedback/w0/summary.md)）。
- ✅ 發布浪潮摘要 + 邀請退出確認（退出捆綁日期為 2025-04-08；請參閱 [W0 摘要](./preview-feedback/w0/summary.md)）。
- ✅ W3 beta 波追踪；治理審查後根據需要安排未來的浪潮。

## W1合作夥伴浪潮總結

- ✅ **法律和治理批准。 ** 合作夥伴附錄於 2025 年 4 月 5 日簽署；批准已上傳至 `DOCS-SORA-Preview-W1`。
- ✅ **遙測 + 嘗試分段。 ** 更改票證 `OPS-TRYIT-147` 於 2025-04-06 執行，其中 `docs.preview.integrity`、`TryItProxyErrors` 和 `DocsPortal/GatewayRefusals` 的 Grafana 快照已存檔。
- ✅ **Artefact + 校驗和準備。 ** `preview-2025-04-12` 捆綁包已驗證；描述符/校驗和/探測日誌存儲在 `artifacts/docs_preview/W1/preview-2025-04-12/` 下。
- ✅ **邀請名冊 + 派遣。 ** 所有八個合作夥伴請求 (`DOCS-SORA-Preview-REQ-P01…P08`) 均獲得批准；邀請於 2025 年 4 月 12 日 15:00–15:21UTC 發送，並按審閱者記錄確認。
- ✅ **反饋儀器。 ** 每日辦公時間+記錄的遙測檢查點；請參閱 [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) 了解摘要。
- ✅ **最終名冊/退出日誌。 ** [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) 現在記錄截至 2025 年 4 月 26 日的邀請/確認時間戳、遙測證據、測驗導出和工件指針，以便治理可以重播這波浪潮。

## 邀請日誌——W0核心維護者

|審稿人 ID |角色 |索取門票 |邀請已發送 (UTC) |預計退出（UTC）|狀態 |筆記|
| ---| ---| ---| ---| ---| ---| ---|
|文檔核心-01 |門戶維護者 | `DOCS-SORA-Preview-REQ-01` | 2025-03-25 10:05 | 2025-04-08 10:00 |活躍|確認校驗和驗證；專注於導航/側邊欄審查。 |
| sdk-rust-01 | Rust SDK 負責人 | `DOCS-SORA-Preview-REQ-02` | 2025-03-25 10:08 | 2025-04-08 10:00 |活躍|測試 SDK 配方 + Norito 快速入門。 |
| SDK-js-01 | JS SDK 維護者 | `DOCS-SORA-Preview-REQ-03` | 2025-03-25 10:12 | 2025-04-08 10:00 |活躍|驗證 Try it 控制台 + ISO 流程。 |
| sorafs-ops-01 | sorafs-ops-01 SoraFS 操作員聯絡 | `DOCS-SORA-Preview-REQ-04` | 2025-03-25 10:15 | 2025-04-08 10:00 |活躍|審核 SoraFS 運行手冊 + 編排文檔。 |
|可觀察性-01 |可觀察性 TL | `DOCS-SORA-Preview-REQ-05` | 2025-03-25 10:18 | 2025-04-08 10:00 |活躍|審查遙測/事件附錄；擁有 Alertmanager 覆蓋範圍。 |

所有邀請都引用相同的 `docs-portal-preview` 工件（運行 2025-03-24，
標籤 `preview-2025-03-24`）和捕獲的驗證記錄
`DOCS-SORA-Preview-W0`。任何添加/暫停都必須記錄在兩個表中
在繼續下一波之前，先處理上面的問題和跟踪器問題。

## 檢查點日誌 — W0

|日期 (UTC) |活動 |筆記|
| ---| ---| ---|
| 2025-03-26 |遙測基線審查+辦公時間| `docs.preview.integrity` + `TryItProxyErrors` 保持綠色；辦公時間確認所有審閱者已完成校驗和驗證。 |
| 2025-03-27 |中點反饋摘要已發布 | [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md) 中捕獲的摘要；兩個小導航問題記錄為 `docs-preview/w0` 標籤，沒有事件報告。 |
| 2025-03-31 |最後一周遙測抽查|最後出境前辦公時間；審閱者確認剩餘的文檔任務正常進行，沒有發出警報。 |
| 2025-04-08 |退出摘要+邀請關閉|確認已完成的審查，撤銷臨時訪問權限，將調查結果存檔在 [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08) 中；在準備 W1 之前更新了跟踪器。 |

## 邀請日誌——W1合作夥伴

|審稿人 ID |角色 |索取門票 |邀請已發送 (UTC) |預計退出（UTC）|狀態 |筆記|
| ---| ---| ---| ---| ---| ---| ---|
|索拉夫-op-01 | SoraFS 運營商（歐盟）| `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 2025-04-26 15:00 |已完成 | 2025-04-20 交付協調器操作反饋；退出確認 15:05UTC。 |
|索拉夫-op-02 | SoraFS 操作員 (JP) | `DOCS-SORA-Preview-REQ-P02` | 2025-04-12 15:03 | 2025-04-26 15:00 |已完成 |在 `docs-preview/w1` 中記錄了推出指導意見；退出確認 15:10UTC。 |
|索拉夫-op-03 | SoraFS 運營商（美國）| `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 2025-04-26 15:00 |已完成 |提交爭議/黑名單編輯；退出確認 15:12UTC。 |
|鳥居-int-01 | Torii 積分器 | `DOCS-SORA-Preview-REQ-P04` | 2025-04-12 15:09 | 2025-04-26 15:00 |已完成 |嘗試一下授權演練已接受；退出確認 15:14UTC。 |
|鳥居-int-02 | Torii 積分器 | `DOCS-SORA-Preview-REQ-P05` | 2025-04-12 15:12 | 2025-04-26 15:00 |已完成 |記錄 RPC/OAuth 文檔評論；退出確認 15:16UTC。 |
| sdk-partner-01 | sdk-partner-01 | SDK 合作夥伴 (Swift) | `DOCS-SORA-Preview-REQ-P06` | 2025-04-12 15:15 | 2025-04-26 15:00 |已完成 |預覽誠信反饋合併；退出確認 15:18UTC。 |
| sdk-partner-02 | sdk-partner-02 | SDK 合作夥伴 (Android) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 2025-04-26 15:00 |已完成 |遙測/編輯審核已完成；退出確認 15:22UTC。 |
|網關操作-01 |網關運營商| `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 2025-04-26 15:00 |已完成 |已提交網關 DNS 運行手冊評論；退出確認 15:24UTC。 |

## 檢查點日誌 — W1

|日期 (UTC) |活動 |筆記|
| ---| ---| ---|
| 2025-04-12 |邀請派遣+實物驗證 |所有八個合作夥伴都通過電子郵件發送了 `preview-2025-04-12` 描述符/存檔；確認存儲在跟踪器中。 |
| 2025-04-13 |遙測基線審查 |已審核 `docs.preview.integrity`、`TryItProxyErrors` 和 `DocsPortal/GatewayRefusals` 儀表板 — 全面呈綠色；辦公時間確認校驗和驗證完成。 |
| 2025-04-18 |中波辦公時間 | `docs.preview.integrity` 保持綠色；兩個文檔記錄在 `docs-preview/w1` 下（導航措辭 + 嘗試屏幕截圖）。 |
| 2025-04-22 |最終遙測抽查|代理+儀表板仍然健康；沒有提出新的問題，退出前在跟踪器中註意到。 |
| 2025-04-26 |退出摘要+邀請關閉|所有合作夥伴均確認審核已完成，邀請已撤銷，證據已存檔在 [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26) 中。 |

## W3 beta 隊列回顧

- ✅ 於 2026 年 2 月 18 日發送邀請，並在當天記錄了校驗和驗證 + 確認。
- ✅ 在 `docs-preview/20260218` 下收集的反饋，涉及治理問題 `DOCS-SORA-Preview-20260218`；通過 `npm run --prefix docs/portal preview:wave -- --wave preview-20260218` 生成的摘要 + 摘要。
- ✅ 最終遙測檢查後，訪問權限於 2026-02-28 被撤銷；跟踪器 + 門戶表已更新，顯示 W3 已完成。

## 邀請日誌——W2社區|審稿人 ID |角色 |索取門票 |邀請已發送 (UTC) |預計退出（UTC）|狀態 |筆記|
| ---| ---| ---| ---| ---| ---| ---|
|通訊卷01 |社區審閱者 (SDK) | `DOCS-SORA-Preview-REQ-C01` | 2025-06-15 16:00 | 2025-06-29 16:00 |已完成 |確認時間 16:06UTC；專注於SDK快速入門； 2025 年 6 月 29 日確認退出。 |
|通訊卷 02 |社區審核員（治理）| `REQ-C02` | 2025-06-15 16:03 | 2025-06-29 16:00 |已完成 |治理/SNS 審核已完成； 2025 年 6 月 29 日確認退出。 |
|通訊卷 03 |社區審閱者 (Norito) | `REQ-C03` | 2025-06-15 16:06 | 2025-06-29 16:00 |已完成 | Norito 已記錄演練反饋；退出確認 2025-06-29。 |
|通訊卷 04 |社區審閱者 (SoraFS) | `REQ-C04` | 2025-06-15 16:09 | 2025-06-29 16:00 |已完成 | SoraFS 操作手冊審核已完成；退出確認 2025-06-29。 |
|通訊卷 05 |社區審閱者（輔助功能）| `REQ-C05` | 2025-06-15 16:12 | 2025-06-29 16:00 |已完成 |共享輔助功能/用戶體驗註釋；退出確認 2025-06-29。 |
|通訊卷 06 |社區審閱者（本地化）| `REQ-C06` | 2025-06-15 16:15 | 2025-06-29 16:00 |已完成 |記錄本地化反饋；退出確認 2025-06-29。 |
|通訊卷 07 |社區審閱者（手機）| `REQ-C07` | 2025-06-15 16:18 | 2025-06-29 16:00 |已完成 |已交付移動 SDK 文檔檢查；退出確認 2025-06-29。 |
|通訊卷 08 |社區審閱者（可觀察性）| `REQ-C08` | 2025-06-15 16:21 | 2025-06-29 16:00 |已完成 |可觀察性附錄審查已完成；退出確認 2025-06-29。 |

## 檢查點日誌 — W2

|日期 (UTC) |活動 |筆記|
| ---| ---| ---|
| 2025-06-15 |邀請派遣+實物驗證 | `preview-2025-06-15` 描述符/檔案與 8 位社區審閱者共享；確認存儲在跟踪器中。 |
| 2025-06-16 |遙測基線審查 | `docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals` 儀表板綠色；嘗試代理日誌顯示社區令牌處於活動狀態。 |
| 2025-06-18 |辦公時間和問題分類 |收集了兩條建議（`docs-preview/w2 #1` 工具提示措辭、`#2` 本地化側邊欄）——均發送至文檔。 |
| 2025-06-21 |遙測檢查+文檔修復|文檔地址為 `docs-preview/w2 #1/#2`；儀表板仍然是綠色的，沒有發生任何事件。 |
| 2025-06-24 |最後一周辦公時間 |審稿人確認了剩餘的反饋提交；沒有警報火災。 |
| 2025-06-29 |退出摘要+邀請關閉|記錄確認、撤銷預覽訪問、存檔遙測快照 + 工件（請參閱 [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)）。 |
| 2025-04-15 |辦公時間和問題分類 |在 `docs-preview/w1` 下記錄了兩個文檔建議；沒有觸發任何事件或警報。 |

## 報告掛鉤

- 每個星期三，更新上面的跟踪表以及主動邀請問題
  帶有簡短的狀態說明（已發送的邀請、活躍的審閱者、事件）。
- 當 Wave 結束時，附加反饋摘要路徑（例如，
  `docs/portal/docs/devportal/preview-feedback/w0/summary.md`）並將其鏈接到
  `status.md`。
- 如果[預覽邀請流程]中有任何暫停條件(./preview-invite-flow.md)
  觸發器，在恢復邀請之前在此處添加修復步驟。