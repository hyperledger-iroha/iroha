---
id: chunker-registry-charter
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Chunker Registry Charter
sidebar_label: Chunker Registry Charter
description: Governance charter for chunker profile submissions and approvals.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意規範來源
:::

# SoraFS Chunker 註冊管理章程

> **批准時間：** 2025 年 10 月 29 日由 Sora 議會基礎設施小組批准（參見
> `docs/source/sorafs/council_minutes_2025-10-29.md`)。任何修改都需要
> 正式治理投票；實施團隊必須將此文件視為
> 在替代章程獲得批准之前保持規範。

本章程定義了 SoraFS 分塊器發展的流程和角色
註冊表。它通過描述新功能如何補充 [Chunker Profile Authoring Guide](./chunker-profile-authoring.md)

## 範圍

該章程適用於 `sorafs_manifest::chunker_registry` 中的每個條目以及
任何使用註冊表的工具（清單 CLI、provider-advert CLI、
SDK）。它強制執行別名和句柄不變量檢查
`chunker_registry::ensure_charter_compliance()`：

- 配置文件 ID 是單調遞增的正整數。
- 規範句柄 `namespace.name@semver` **必須** 作為第一個出現
- 別名字符串經過修剪、獨特，並且不會與規範句柄發生衝突
  其他條目。

## 角色

- **作者** – 準備提案、重新生成裝置並收集
  決定論的證據。
- **工具工作組 (TWG)** – 使用已發布的建議驗證提案
  檢查清單並確保註冊表不變量保持不變。
- **治理委員會 (GC)** – 審查 TWG 報告，簽署提案
  信封，並批准發布/棄用時間表。
- **存儲團隊** – 維護註冊表實施並發布
  文檔更新。

## 生命週期工作流程

1. **提案提交**
   - 作者運行創作指南中的驗證清單並創建
     下的 `ChunkerProfileProposalV1` JSON
     `docs/source/sorafs/proposals/`。
   - 包括 CLI 輸出：
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - 提交包含固定裝置、提案、確定性報告和註冊表的 PR
     更新。

2. **工具審查 (TWG)**
   - 重播驗證清單（夾具、模糊、清單/PoR 管道）。
   - 運行 `cargo test -p sorafs_car --chunker-registry` 並確保
     `ensure_charter_compliance()` 通過新條目。
   - 驗證 CLI 行為（`--list-profiles`、`--promote-profile`、流式傳輸
     `--json-out=-`) 反映了更新的別名和句柄。
   - 製作一份簡短的報告，總結調查結果和通過/失敗狀態。

3. **理事會批准 (GC)**
   - 審查 TWG 報告和提案元數據。
   - 簽署提案摘要 (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     並將簽名附加到與理事會一起保存的理事會信封上
     固定裝置。
   - 在治理會議記錄中記錄投票結果。

4. **發表**
   - 合併 PR，更新：
     - `sorafs_manifest::chunker_registry_data`。
     - 文檔（`chunker_registry.md`，創作/一致性指南）。
     - 賽程和決定論報告。
   - 通知運營商和 SDK 團隊新的配置文件和計劃的推出。

5. **棄用/日落**
   - 取代現有配置文件的提案必須包含雙重發布
     窗口（寬限期）和升級計劃。
     在註冊表中並更新遷移分類帳。

6. **緊急變更**
   - 刪除或修補程序需要理事會投票並獲得多數批准。
   - TWG 必須記錄風險緩解步驟並更新事件日誌。

## 工具期望

- `sorafs_manifest_chunk_store` 和 `sorafs_manifest_stub` 暴露：
  - `--list-profiles` 用於註冊檢查。
  - `--promote-profile=<handle>` 生成使用的規范元數據塊
    推廣個人資料時。
  - `--json-out=-` 將報告流式傳輸到標準輸出，從而實現可重複的審查
    日誌。
- `ensure_charter_compliance()` 在啟動時在相關二進製文件中被調用
  （`manifest_chunk_store`、`provider_advert_stub`）。如果是新的，CI 測試一定會失敗
  條目違反了章程。

## 記錄保存

- 將所有確定性報告存儲在 `docs/source/sorafs/reports/` 中。
- 引用分塊決策的理事會會議記錄如下
  `docs/source/sorafs/migration_ledger.md`。
- 每次主要註冊表更改後更新 `roadmap.md` 和 `status.md`。

## 參考文獻

- 創作指南：[Chunker Profile創作指南](./chunker-profile-authoring.md)
- 一致性檢查表：`docs/source/sorafs/chunker_conformance.md`
- 註冊表參考：[Chunker 配置文件註冊表](./chunker-registry.md)