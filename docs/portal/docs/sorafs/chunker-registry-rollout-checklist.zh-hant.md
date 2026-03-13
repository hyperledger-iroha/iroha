---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a117889e81f876c00129ade76a9a04aa39181add2378ef5c19110b7be30f9d6f
source_last_modified: "2026-01-05T09:28:11.859335+00:00"
translation_last_reviewed: 2026-02-07
id: chunker-registry-rollout-checklist
title: SoraFS Chunker Registry Rollout Checklist
sidebar_label: Chunker Rollout Checklist
description: Step-by-step rollout plan for chunker registry updates.
translator: machine-google-reviewed
---

:::注意規範來源
:::

# SoraFS 註冊表部署清單

該清單記錄了推廣新分塊配置文件所需的步驟或
治理後從審核到生產的提供商准入捆綁包
憲章已獲得批准。

> **範圍：** 適用於所有修改的版本
> `sorafs_manifest::chunker_registry`、提供者入學信封，或
> 規範夾具捆綁包 (`fixtures/sorafs_chunker/*`)。

## 1. 飛行前驗證

1. 重新生成裝置並驗證確定性：
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. 確認確定性哈希值
   `docs/source/sorafs/reports/sf1_determinism.md`（或相關配置文件
   報告）與重新生成的工件相匹配。
3. 確保 `sorafs_manifest::chunker_registry` 編譯為
   `ensure_charter_compliance()` 通過運行：
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. 更新提案檔案：
   - `docs/source/sorafs/proposals/<profile>.json`
   - `docs/source/sorafs/council_minutes_*.md` 下的理事會會議記錄條目
   - 決定論報告

## 2. 治理簽核

1. 向 Sora 提交工具工作組報告和提案摘要
   議會基礎設施小組。
2. 將批准詳細信息記錄在
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`。
3. 將議會簽名的信封與賽程表一起公佈：
   `fixtures/sorafs_chunker/manifest_signatures.json`。
4. 驗證信封可通過治理獲取幫助程序訪問：
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. 分階段部署

請參閱 [暫存清單手冊](./staging-manifest-playbook) 了解
這些步驟的詳細演練。

1. 部署 Torii，並啟用 `torii.sorafs` 發現和准入
   強制執行已開啟 (`enforce_admission = true`)。
2. 將批准的提供者准入信封推送至暫存登記處
   `torii.sorafs.discovery.admission.envelopes_dir` 引用的目錄。
3. 驗證提供商廣告是否通過發現 API 傳播：
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. 使用治理標頭練習清單/計劃端點：
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. 確認遙測儀表板 (`torii_sorafs_*`) 和警報規則報告
   新的配置文件沒有錯誤。

## 4. 生產部署

1. 對生產 Torii 節點重複暫存步驟。
2. 宣布激活窗口（日期/時間、寬限期、回滾計劃）
   運營商和SDK渠道。
3. 合併包含以下內容的發布 PR：
   - 更新了固定裝置和信封
   - 文檔變更（章程參考、決定論報告）
   - 路線圖/狀態刷新
4. 標記版本並存檔已簽名的工件以查找出處。

## 5. 推出後審核

1. 捕獲最終指標（發現計數、獲取成功率、錯誤
   直方圖）推出後 24 小時。
2. 使用簡短摘要和確定性報告的鏈接更新 `status.md`。
3. 將任何後續任務（例如，其他配置文件創作指南）歸檔到
   `roadmap.md`。