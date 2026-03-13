---
id: staging-manifest-playbook
lang: zh-hans
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

:::注意规范来源
:::

## 概述

本手册介绍了在将更改推广到生产环境之前，在临时 Torii 部署上启用议会批准的 chunker 配置文件。它假设 SoraFS 治理章程已获得批准，并且规范的固定装置在存储库中可用。

## 1.先决条件

1. 同步规范赛程和签名：

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. 准备Torii在启动时读取的准入信封目录（示例路径）：`/var/lib/iroha/admission/sorafs`。
3. 确保 Torii 配置启用发现缓存和准入强制：

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

## 2. 公布录取信封

1. 将批准的提供者准入信封复制到 `torii.sorafs.discovery.admission.envelopes_dir` 引用的目录中：

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. 重新启动 Torii（如果您使用即时重新加载来包装加载程序，则发送 SIGHUP）。
3. 跟踪日志以获取准入消息：

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. 验证发现传播

1. 发布由您生成的签名提供商广告负载（Norito 字节）
   供应商管道：

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. 查询发现端点并确认广告以规范别名出现：

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   确保 `profile_aliases` 包含 `"sorafs.sf1@1.0.0"` 作为第一个条目。

## 4. 练习清单和计划终点

1. 获取清单元数据（如果强制执行准入，则需要流令牌）：

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. 检查 JSON 输出并验证：
   - `chunk_profile_handle` 是 `sorafs.sf1@1.0.0`。
   - `manifest_digest_hex` 与确定性报告匹配。
   - `chunk_digests_blake3` 与重新生成的夹具对齐。

## 5. 遥测检查

- 确认 Prometheus 公开新的配置文件指标：

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- 仪表板应在预期别名下显示临时提供程序，并在配置文件处于活动状态时将掉电计数器保持为零。

## 6. 推出准备情况

1. 捕获包含 URL、清单 ID 和遥测快照的简短报告。
2. 在 Nexus 推出渠道以及计划的生产激活窗口中共享报告。
3. 利益相关者签字后，继续执行生产检查表（`chunker_registry_rollout_checklist.md` 中的第 4 节）。

保持此剧本的更新可确保每个分块器/准入部署在分阶段和生产中都遵循相同的确定性步骤。