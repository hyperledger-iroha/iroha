---
id: staging-manifest-playbook
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Staging Manifest Playbook
sidebar_label: Staging Manifest Playbook
description: Checklist for enabling the Parliament-ratified chunker profile on staging Torii deployments.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意規範來源
:::

## 概述

本手冊介紹了在將更改推廣到生產環境之前，在臨時 Torii 部署上啟用議會批准的 chunker 配置文件。它假設 SoraFS 治理章程已獲得批准，並且規範的固定裝置在存儲庫中可用。

## 1.先決條件

1. 同步規範賽程和簽名：

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. 準備Torii在啟動時讀取的准入信封目錄（示例路徑）：`/var/lib/iroha/admission/sorafs`。
3. 確保 Torii 配置啟用發現緩存和准入強制：

   ```toml
   [torii.sorafs.discovery]
   discovery_enabled = true
   known_capabilities = ["torii_gateway", "chunk_range_fetch", "vendor_reserved"]

   [torii.sorafs.discovery.admission]
   envelopes_dir = "/var/lib/iroha/admission/sorafs"

   [torii.sorafs.storage]
   enabled = true

   [torii.sorafs.gateway]
   enforce_admission = true
   enforce_capabilities = true
   ```

## 2. 公佈錄取信封

1. 將批准的提供者准入信封複製到 `torii.sorafs.discovery.admission.envelopes_dir` 引用的目錄中：

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. 重新啟動 Torii（如果您使用即時重新加載來包裝加載程序，則發送 SIGHUP）。
3. 跟踪日誌以獲取准入消息：

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. 驗證發現傳播

1. 發布由您生成的簽名提供商廣告負載（Norito 字節）
   供應商管道：

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. 查詢發現端點並確認廣告以規範別名出現：

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   確保 `profile_aliases` 包含 `"sorafs.sf1@1.0.0"` 作為第一個條目。

## 4. 練習清單和計劃終點

1. 獲取清單元數據（如果強制執行准入，則需要流令牌）：

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. 檢查 JSON 輸出並驗證：
   - `chunk_profile_handle` 是 `sorafs.sf1@1.0.0`。
   - `manifest_digest_hex` 與確定性報告匹配。
   - `chunk_digests_blake3` 與重新生成的夾具對齊。

## 5. 遙測檢查

- 確認 Prometheus 公開新的配置文件指標：

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- 儀表板應在預期別名下顯示臨時提供程序，並在配置文件處於活動狀態時將掉電計數器保持為零。

## 6. 推出準備情況

1. 捕獲包含 URL、清單 ID 和遙測快照的簡短報告。
2. 在 Nexus 推出渠道以及計劃的生產激活窗口中共享報告。
3. 利益相關者簽字後，繼續執行生產檢查表（`chunker_registry_rollout_checklist.md` 中的第 4 節）。

保持此劇本的更新可確保每個分塊器/准入部署在分階段和生產中都遵循相同的確定性步驟。