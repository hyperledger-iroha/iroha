---
lang: zh-hans
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/node-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a37b7ca6ae1aa64e6289ecc44b48ef29c1c884abc039123c1a03b9c35b2e7120
source_last_modified: "2026-01-22T14:35:36.900283+00:00"
translation_last_reviewed: 2026-02-07
id: node-operations
title: Node Operations Runbook
sidebar_label: Node Operations Runbook
description: Validate the embedded `sorafs-node` deployment inside Torii.
translator: machine-google-reviewed
---

:::注意规范来源
镜子 `docs/source/sorafs/runbooks/sorafs_node_ops.md`。保持两个副本在各个版本之间保持一致。
:::

## 概述

此操作手册引导操作员验证 Torii 内的嵌入式 `sorafs-node` 部署。每个部分都直接映射到 SF-3 可交付成果：pin/fetch 往返、重新启动恢复、配额拒绝和 PoR 采样。

## 1.先决条件

- 在 `torii.sorafs.storage` 中启用存储工作线程：

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

- 确保 Torii 进程具有对 `data_dir` 的读/写访问权限。
- 记录声明后，确认节点通过 `GET /v1/sorafs/capacity/state` 公布预期容量。
- 启用平滑后，仪表板会同时显示原始和平滑后的 GiB·小时/PoR 计数器，以突出显示无抖动趋势以及现货值。

### CLI 试运行（可选）

在公开 HTTP 端点之前，您可以使用捆绑的 CLI 对存储后端进行健全性检查。【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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

这些命令打印 Norito JSON 摘要并拒绝块配置文件或摘要不匹配，这使得它们对于 Torii 接线之前的 CI 烟雾检查非常有用。【crates/sorafs_node/tests/cli.rs#L1】

一旦 Torii 上线，您就可以通过 HTTP 检索相同的工件：

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

两个端点均由嵌入式存储工作人员提供服务，因此 CLI 冒烟测试和网关探测保持同步。【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#L1259】

## 2. Pin → 获取往返

1. 生成清单 + 有效负载包（例如使用 `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`）。
2. 使用base64编码提交清单：

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   请求 JSON 必须包含 `manifest_b64` 和 `payload_b64`。成功的响应将返回 `manifest_id_hex` 和有效负载摘要。
3. 获取固定数据：

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   对 `data_b64` 字段进行 Base64 解码并验证其与原始字节匹配。

## 3. 重新启动恢复练习

1. 固定至少一个上述清单。
2. 重新启动Torii进程（或整个节点）。
3. 重新提交提取请求。有效负载必须仍然可检索，并且返回的摘要必须与重新启动前的值匹配。
4. 检查 `GET /v1/sorafs/storage/state` 以确认 `bytes_used` 反映重新启动后保留的清单。

## 4. 配额拒绝测试

1. 暂时将 `torii.sorafs.storage.max_capacity_bytes` 降低到一个较小的值（例如单个清单的大小）。
2. 固定一清单；请求应该成功。
3. 尝试固定第二个类似大小的清单。 Torii 必须使用 HTTP `400` 和包含 `storage capacity exceeded` 的错误消息拒绝请求。
4. 完成后恢复正常容量限制。

## 5. PoR 采样探针

1. 固定清单。
2. 索取 PoR 样品：

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. 验证响应包含 `samples` 和请求的计数，并且每个证明都针对存储的清单根进行验证。

## 6. 自动化挂钩

- CI/冒烟测试可以重复使用添加的目标检查：

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```其中涵盖 `pin_fetch_roundtrip`、`pin_survives_restart`、`pin_quota_rejection` 和 `por_sampling_returns_verified_proofs`。
- 仪表板应跟踪：
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` 和 `torii_sorafs_storage_fetch_inflight`
  - PoR 成功/失败计数器通过 `/v1/sorafs/capacity/state` 出现
  - 通过 `sorafs_node_deal_publish_total{result=success|failure}` 发布和解尝试

遵循这些练习可确保嵌入式存储工作线程能够在节点向更广泛的网络通告容量之前摄取数据、在重新启动后幸存、遵守配置的配额并生成确定性 PoR 证明。