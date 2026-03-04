---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8a4be274ad087f292559c4b83a120ad20316a1e1dfe0ccbfb9aad42235ac136b
source_last_modified: "2026-01-05T09:28:11.909870+00:00"
translation_last_reviewed: 2026-02-07
id: signing-ceremony
title: Signing Ceremony Replacement
description: How the Sora Parliament approves and distributes SoraFS chunker fixtures (SF-1b).
sidebar_label: Signing Ceremony
translator: machine-google-reviewed
---

> 路線圖：**SF-1b — Sora 議會固定裝置批准。 **

用於 SoraFS chunker 裝置的手動簽名儀式已停用。全部
批准現在通過 **Sora 議會**，這是一個基於抽籤的 DAO，
管轄 Nexus。議會成員通過 XOR 債券獲得公民身份，輪換
小組，並進行鏈上投票以批准、拒絕或回滾固定裝置
發布。本指南解釋了流程和開發人員工具。

## 議會概況

- **公民身份** — 運營商綁定所需的 XOR 以註冊為公民，並且
  獲得抽籤資格。
- **小組** — 職責劃分給輪換小組（基礎設施、
  適度、財政部……）。基礎設施面板擁有 SoraFS 夾具
  批准。
- **排序和輪換** — 面板座位按照指定的節奏重新繪製
  議會憲法因此沒有任何一個團體壟斷批准。

## 夾具審批流程

1. **提交提案**
   - 工具工作組上傳候選 `manifest_blake3.json` 捆綁包以及
     通過 `sorafs.fixtureProposal` 與鏈上註冊表進行固定差異。
   - 該提案記錄了 BLAKE3 摘要、語義版本和變更說明。
2. **審核與投票**
   - 基礎設施小組通過議會任務接收任務
     隊列。
   - 小組成員檢查 CI 製品、運行奇偶校驗測試和鑄件加權
     鏈上投票。
3. **最終確定**
   - 一旦達到法定人數，運行時就會發出一個批准事件，其中包括
     規范清單摘要和 Merkle 對夾具有效負載的承諾。
   - 該事件被鏡像到 SoraFS 註冊表中，以便客戶端可以獲取
     最新議會批准的清單。
4. **分配**
   - CLI 助手 (`cargo xtask sorafs-fetch-fixture`) 提取已批准的清單
     來自 Nexus RPC。存儲庫的 JSON/TS/Go 常量通過以下方式保持同步
     重新運行 `export_vectors` 並根據鏈上驗證摘要
     記錄。

## 開發人員工作流程

- 重新生成裝置：

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- 使用議會獲取助手下載批准的信封，驗證
  簽名，並刷新本地賽程。點 `--signatures` 處
  議會出版的信封；幫助者解析隨附的清單，
  重新計算 BLAKE3 摘要，並強制執行規範
  `sorafs.sf1@1.0.0` 配置文件。

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

如果清單位於不同的 URL，則傳遞 `--manifest`。未簽名的信封
除非為本地煙霧運行設置 `--allow-unsigned`，否則將被拒絕。

- 通過暫存網關驗證清單時，目標為 Torii 而不是
  本地有效負載：

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- 本地 CI 不再需要 `signer.json` 名冊。
  `ci/check_sorafs_fixtures.sh` 將回購狀態與最新的進行比較
  鏈上承諾，當它們出現分歧時就會失敗。

## 治理說明

- 議會憲法規定法定人數、輪換和升級——否
  需要板條箱級別的配置。
- 緊急回滾通過議會仲裁小組處理。的
  基礎設施小組提交了一份參考先前清單的恢復提案
  摘要，一旦獲得批准就會取代版本。
- 歷史批准仍可在 SoraFS 法醫註冊表中獲取
  重播。

## 常見問題解答

- **`signer.json`去哪兒了？ **  
  它被刪除了。所有簽名者歸屬都存在於鏈上； `manifest_signatures.json`
  存儲庫中只有一個必須與最新版本相匹配的開發人員固定裝置
  批准事件。

- **我們還需要本地 Ed25519 簽名嗎？ **  
  不會。議會的批准作為鏈上文物存儲。存在當地固定裝置
  為了可重複性，但根據議會摘要進行了驗證。

- **團隊如何監控審批？ **  
  訂閱 `ParliamentFixtureApproved` 事件或通過以下方式查詢註冊表
  Nexus RPC，用於檢索當前清單摘要和小組點名。