---
lang: zh-hant
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c3eddff3a7b9b5dc4eac39251c9df72d0533a6e1b5865c716d54dc3b1c5de164
source_last_modified: "2025-12-29T18:16:35.108030+00:00"
translation_last_reviewed: 2026-02-07
id: preview-feedback-w1-plan
title: W1 partner preflight plan
sidebar_label: W1 plan
description: Tasks, owners, and evidence checklist for the partner preview cohort.
translator: machine-google-reviewed
---

|項目 |詳情 |
| ---| ---|
|波| W1 — 合作夥伴和 Torii 集成商 |
|目標窗口| 2025 年第二季度第 3 週 |
|人工製品標籤（計劃中）| `preview-2025-04-12` |
|追踪器問題 | `DOCS-SORA-Preview-W1` |

## 目標

1. 確保合作夥伴預覽條款獲得法律+治理批准。
2. 暫存邀請包中使用的 Try it 代理和遙測快照。
3. 刷新經過校驗和驗證的預覽工件和探測結果。
4. 在發送邀請之前確定合作夥伴名冊 + 請求模板。

## 任務分解

|身份證 |任務|業主|到期 |狀態 |筆記|
| ---| ---| ---| ---| ---| ---|
| W1-P1 |獲得預覽條款附錄的法律批准 |文檔/DevRel 負責人 → 法律 | 2025-04-05 | ✅ 已完成 |合法票據 `DOCS-SORA-Preview-W1-Legal` 於 2025-04-05 簽署； PDF 附在跟踪器上。 |
| W1-P2 |捕獲嘗試代理暫存窗口 (2025-04-10) 並驗證代理運行狀況 |文檔/開發版本 + 操作 | 2025-04-06 | ✅ 已完成 | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` 於 2025-04-06 執行； CLI 記錄 + `.env.tryit-proxy.bak` 已存檔。 |
| W1-P3 |構建預覽工件 (`preview-2025-04-12`)，運行 `scripts/preview_verify.sh` + `npm run probe:portal`，存檔描述符/校驗和 |門戶網站 TL | 2025-04-08 | ✅ 已完成 | Artefact + 驗證日誌存儲在 `artifacts/docs_preview/W1/preview-2025-04-12/` 下；探頭輸出連接到跟踪器。 |
| W1-P4 |查看合作夥伴入會表格 (`DOCS-SORA-Preview-REQ-P01…P08`)，確認聯繫人 + 保密協議 |治理聯絡| 2025-04-07 | ✅ 已完成 |所有八項請求均獲得批准（最後兩項請求於 2025 年 4 月 11 日獲得批准）；跟踪器中鏈接的批准。 |
| W1-P5 |起草邀請文案（基於`docs/examples/docs_preview_invite_template.md`），為每個合作夥伴設置`<preview_tag>`和`<request_ticket>` |文檔/DevRel 領導 | 2025-04-08 | ✅ 已完成 |邀請草稿於 2025 年 4 月 12 日 15:00 UTC 連同工件鏈接一起發送。 |

## 飛行前檢查表

> 提示：運行 `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` 自動執行步驟 1-5（構建、校驗和驗證、門戶探測、鏈接檢查器和 Try it 代理更新）。該腳本記錄了一個 JSON 日誌，您可以將其附加到跟踪器問題。

1. `npm run build`（與 `DOCS_RELEASE_TAG=preview-2025-04-12`）重新生成 `build/checksums.sha256` 和 `build/release.json`。
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`。
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`。
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` 和描述符旁邊的存檔 `build/link-report.json`。
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora`（或通過`--tryit-target`提供適當的目標）；提交更新的 `.env.tryit-proxy` 並保留 `.bak` 進行回滾。
6. 使用日誌路徑更新 W1 跟踪器問題（描述符校驗和、探測輸出、嘗試代理更改、Grafana 快照）。

## 證據清單

- [x] 已簽署的法律批准（PDF 或票據鏈接）附在 `DOCS-SORA-Preview-W1` 上。
- [x] Grafana `docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals` 的屏幕截圖。
- [x] `preview-2025-04-12` 描述符 + 校驗和日誌存儲在 `artifacts/docs_preview/W1/` 下。
- [x] 填充了 `invite_sent_at` 時間戳的邀請名冊表（請參閱跟踪器 W1 日誌）。
- [x] 反饋工件鏡像在 [`preview-feedback/w1/log.md`](./log.md) 中，每個合作夥伴一行（2025 年 4 月 26 日更新，包含名冊/遙測/問題數據）。

隨著任務的進展更新此計劃；跟踪器引用它來保留路線圖
可審計。

## 反饋工作流程

1. 對於每個審閱者，將模板複製到
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   填寫元數據，並將完成的副本存儲在
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`。
2. 在實時日誌中總結邀請、遙測檢查點和未決問題
   [`preview-feedback/w1/log.md`](./log.md) 因此治理審核者可以重播整個浪潮
   無需離開存儲庫。
3. 當知識檢查或調查導出到達時，將它們附加到日誌中記錄的工件路徑中
   並交叉鏈接跟踪器問題。