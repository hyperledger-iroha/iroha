---
lang: zh-hant
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/node-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a37b7ca6ae1aa64e6289ecc44b48ef29c1c884abc039123c1a03b9c35b2e7120
source_last_modified: "2026-01-22T14:35:36.900283+00:00"
translation_last_reviewed: 2026-02-07
id: node-operations-zh-hant
title: Node Operations Runbook
sidebar_label: Node Operations Runbook
description: Validate the embedded `sorafs-node` deployment inside Torii.
translator: machine-google-reviewed
slug: /sorafs/node-operations-zh-hant
---

:::注意規範來源
鏡子 `docs/source/sorafs/runbooks/sorafs_node_ops.md`。保持兩個副本在各個版本之間保持一致。
:::

## 概述

此操作手冊引導操作員驗證 Torii 內的嵌入式 `sorafs-node` 部署。每個部分都直接映射到 SF-3 可交付成果：pin/fetch 往返、重新啟動恢復、配額拒絕和 PoR 採樣。

## 1.先決條件

- 在 `torii.sorafs.storage` 中啟用存儲工作線程：

  ```toml
  [torii.sorafs.storage]
  enabled = true
  data_dir = "./storage/sorafs"
  max_capacity_bytes = 21474836480    # 20 GiB
  max_parallel_fetches = 32
  max_pins = 1000
  por_sample_interval_secs = 600

  [torii.sorafs.storage.metering_smoothing]
  gib_hours_enabled = true
  gib_hours_alpha = 0.25
  por_success_enabled = true
  por_success_alpha = 0.25
  ```

- 確保 Torii 進程具有對 `data_dir` 的讀/寫訪問權限。
- 記錄聲明後，確認節點通過 `GET /v1/sorafs/capacity/state` 公佈預期容量。
- 啟用平滑後，儀表板會同時顯示原始和平滑後的 GiB·小時/PoR 計數器，以突出顯示無抖動趨勢以及現貨值。

### CLI 試運行（可選）

在公開 HTTP 端點之前，您可以使用捆綁的 CLI 對存儲後端進行健全性檢查。 【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin

cargo run -p sorafs_node --bin sorafs-node export \
  --data-dir ./storage/sorafs \
  --manifest-id <hex> \
  --manifest-out ./out/manifest.to \
  --payload-out ./out/payload.bin
```

這些命令打印 Norito JSON 摘要並拒絕塊配置文件或摘要不匹配，這使得它們對於 Torii 接線之前的 CI 煙霧檢查非常有用。 【crates/sorafs_node/tests/cli.rs#L1】

一旦 Torii 上線，您就可以通過 HTTP 檢索相同的工件：

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

兩個端點均由嵌入式存儲工作人員提供服務，因此 CLI 冒煙測試和網關探測保持同步。 【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#L1259】

## 2. Pin → 獲取往返

1. 生成清單 + 有效負載包（例如使用 `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`）。
2. 使用base64編碼提交清單：

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   請求 JSON 必須包含 `manifest_b64` 和 `payload_b64`。成功的響應將返回 `manifest_id_hex` 和有效負載摘要。
3. 獲取固定數據：

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   對 `data_b64` 字段進行 Base64 解碼並驗證其與原始字節匹配。

## 3. 重新啟動恢復練習

1. 固定至少一個上述清單。
2. 重新啟動Torii進程（或整個節點）。
3. 重新提交提取請求。有效負載必須仍然可檢索，並且返回的摘要必須與重新啟動前的值匹配。
4. 檢查 `GET /v1/sorafs/storage/state` 以確認 `bytes_used` 反映重新啟動後保留的清單。

## 4. 配額拒絕測試

1. 暫時將 `torii.sorafs.storage.max_capacity_bytes` 降低到一個較小的值（例如單個清單的大小）。
2. 固定一清單；請求應該成功。
3. 嘗試固定第二個類似大小的清單。 Torii 必須使用 HTTP `400` 和包含 `storage capacity exceeded` 的錯誤消息拒絕請求。
4. 完成後恢復正常容量限制。

## 5. PoR 採樣探針

1. 固定清單。
2. 索取 PoR 樣品：

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. 驗證響應包含 `samples` 和請求的計數，並且每個證明都針對存儲的清單根進行驗證。

## 6. 自動化掛鉤

- CI/冒煙測試可以重複使用添加的目標檢查：

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```其中涵蓋 `pin_fetch_roundtrip`、`pin_survives_restart`、`pin_quota_rejection` 和 `por_sampling_returns_verified_proofs`。
- 儀表板應跟踪：
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` 和 `torii_sorafs_storage_fetch_inflight`
  - PoR 成功/失敗計數器通過 `/v1/sorafs/capacity/state` 出現
  - 通過 `sorafs_node_deal_publish_total{result=success|failure}` 發布和解嘗試

遵循這些練習可確保嵌入式存儲工作線程能夠在節點向更廣泛的網絡通告容量之前攝取數據、在重新啟動後倖存、遵守配置的配額並生成確定性 PoR 證明。
