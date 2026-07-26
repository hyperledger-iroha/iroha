---
lang: zh-hant
direction: ltr
source: docs/portal/docs/devportal/preview-integrity-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 182b1616ff32d0be52d0a6e33178fac4261e7e92a088048b11e5f93e4b005750
source_last_modified: "2026-01-05T09:28:11.838358+00:00"
translation_last_reviewed: 2026-02-07
id: preview-integrity-plan
title: Checksum-Gated Preview Plan
sidebar_label: Preview Integrity Plan
description: Implementation roadmap for securing the docs portal preview pipeline with checksum validation and notarised artefacts.
translator: machine-google-reviewed
---

該計劃概述了使每個門戶預覽製品在發布前可驗證所需的剩餘工作。目標是保證審閱者下載 CI 中內置的準確快照、校驗和清單不可變，並且可通過 SoraFS 和 Norito 元數據發現預覽。

## 目標

- **確定性構建：** 確保 `npm run build` 產生可重現的輸出並始終發出 `build/checksums.sha256`。
- **經過驗證的預覽：** 要求每個預覽製品都附帶校驗和清單，並在驗證失敗時拒絕發布。
- **Norito-發布的元數據：** 將預覽描述符（提交元數據、校驗和摘要、SoraFS CID）保留為 Norito JSON，以便治理工具可以審核版本。
- **運營商工具：**提供消費者可以在本地運行的一步驗證腳本（`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`）；該腳本現在端到端地包裝了校驗和+描述符驗證流程。標準預覽命令 (`npm run serve`) 現在會在 `docusaurus serve` 之前自動調用此幫助程序，因此本地快照保持校驗和門控（將 `npm run serve:verified` 保留為顯式別名）。

## 第一階段——CI 執行

1. 將 `.github/workflows/docs-portal-preview.yml` 更新為：
   - 在 Docusaurus 構建之後運行 `node docs/portal/scripts/write-checksums.mjs`（已在本地調用）。
   - 執行 `cd build && sha256sum -c checksums.sha256` 並因不匹配而使作業失敗。
   - 將構建目錄打包為 `artifacts/preview-site.tar.gz`，複製校驗和清單，調用 `scripts/generate-preview-descriptor.mjs`，並使用 JSON 配置執行 `scripts/sorafs-package-preview.sh`（請參閱 `docs/examples/sorafs_preview_publish.json`），以便工作流程發出元數據和確定性 SoraFS 包。
   - 上傳靜態站點、元數據工件（`docs-portal-preview`、`docs-portal-preview-metadata`）和 SoraFS 捆綁包（`docs-portal-preview-sorafs`），以便可以檢查清單、CAR 摘要和計劃，而無需重新運行構建。
2. 添加 CI 徽章註釋，總結拉取請求中的校驗和驗證結果（✅ 通過 `docs-portal-preview.yml` GitHub 腳本註釋步驟實現）。
3. 在 `docs/portal/README.md`（CI 部分）中記錄工作流程，並鏈接到發布清單中的驗證步驟。

## 驗證腳本

`docs/portal/scripts/preview_verify.sh` 驗證下載的預覽工件，無需手動 `sha256sum` 調用。共享本地快照時，使用 `npm run serve`（或顯式 `npm run serve:verified` 別名）運行腳本並一步啟動 `docusaurus serve`。驗證邏輯：

1. 針對 `build/checksums.sha256` 運行適當的 SHA 工具（`sha256sum` 或 `shasum -a 256`）。
2. （可選）比較預覽描述符的 `checksums_manifest` 摘要/文件名和預覽存檔摘要/文件名（如果提供）。
3. 當檢測到任何不匹配時退出非零，以便審閱者可以阻止被篡改的預覽。

示例用法（提取 CI 工件後）：

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

CI 和發布工程師在下載預覽包或將工件附加到發布票證時都應該調用該腳本。

## 第 2 階段 — SoraFS 發布

1. 通過以下作業擴展預覽工作流程：
   - 使用 `sorafs_cli car pack` 和 `manifest submit` 將構建的站點上傳到 SoraFS 臨時網關。
   - 捕獲返回的清單摘要和 SoraFS CID。
   - 將 `{ commit, branch, checksum_manifest, cid }` 序列化為 Norito JSON (`docs/portal/preview/preview_descriptor.json`)。
2. 將描述符與構建工件一起存儲，並在拉取請求註釋中公開 CID。
3. 添加在空運行模式下執行 `sorafs_cli` 的集成測試，以確保未來的更改保持元數據架構穩定。

## 第 3 階段 — 治理和審計

1. 發布 Norito 模式 (`PreviewDescriptorV1`)，描述 `docs/portal/schemas/` 下的描述符結構。
2. 更新 DOCS-SORA 發布清單以要求：
   - 針對上傳的 CID 運行 `sorafs_cli manifest verify`。
   - 在發布 PR 描述中記錄校驗和清單摘要和 CID。
3. 連接治理自動化，以在發布投票期間根據校驗和清單交叉檢查描述符。

## 可交付成果和所有權

|里程碑|所有者 |目標|筆記|
|------------|----------|--------|--------|
| CI 校驗和執行落地 |文檔 基礎設施 |第 1 週 |添加故障門+工件上傳。 |
| SoraFS 預覽發布 |文檔 基礎設施/存儲團隊 |第 2 週 |需要訪問暫存憑據和 Norito 架構更新。 |
|治理整合|文檔/DevRel 主管/治理工作組 |第 3 週 |發布架構+更新清單和路線圖條目。 |

## 開放問題

- 哪個 SoraFS 環境應保存預覽工件（暫存與專用預覽通道）？
- 發布前預覽描述符是否需要雙重簽名（Ed25519 + ML-DSA）？
- 在運行 `sorafs_cli` 時，CI 工作流程是否應該固定協調器配置 (`orchestrator_tuning.json`) 以保持清單的可重複性？

在 `docs/portal/docs/reference/publishing-checklist.md` 中捕獲決策，並在解決未知問題後更新此計劃。