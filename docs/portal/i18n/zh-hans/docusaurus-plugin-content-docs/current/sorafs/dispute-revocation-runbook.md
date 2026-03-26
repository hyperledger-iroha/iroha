---
id: dispute-revocation-runbook
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Dispute & Revocation Runbook
sidebar_label: Dispute & Revocation Runbook
description: Governance workflow for filing SoraFS capacity disputes, coordinating revocations, and evacuating data deterministically.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意规范来源
:::

## 目的

本操作手册指导治理操作员提交 SoraFS 容量争议、协调撤销并确保确定性地完成数据疏散。

## 1. 评估事件

- **触发条件：** 检测到 SLA 违规（正常运行时间/PoR 故障）、复制不足或计费不一致。
- **确认遥测：** 为提供商捕获 `/v1/sorafs/capacity/state` 和 `/v1/sorafs/capacity/telemetry` 快照。
- **通知利益相关者：** 存储团队（提供商运营）、治理委员会（决策机构）、可观察性（仪表板更新）。

## 2. 准备证据包

1. 收集原始工件（遥测 JSON、CLI 日志、审核员注释）。
2. 规范化为确定性存档（例如 tarball）；记录：
   - BLAKE3-256 摘要 (`evidence_digest`)
   - 媒体类型（`application/zip`、`application/jsonl` 等）
   - 托管 URI（对象存储、SoraFS 引脚或 Torii 可访问端点）
3. 将捆绑包存储在具有一次写入访问权限的治理证据收集存储桶中。

## 3. 提出争议

1. 为 `sorafs_manifest_stub capacity dispute` 创建规范 JSON：

   ```json
   {
     "provider_id_hex": "<hex>",
     "complainant_id_hex": "<hex>",
     "replication_order_id_hex": "<hex or omit>",
     "kind": "replication_shortfall",
     "submitted_epoch": 1700100000,
     "description": "Provider failed to ingest order within SLA.",
     "requested_remedy": "Slash 10% stake and suspend adverts",
     "evidence": {
       "digest_hex": "<blake3-256>",
       "media_type": "application/zip",
       "uri": "https://evidence.sora.net/bundles/<id>.zip",
       "size_bytes": 1024
     }
   }
   ```

2. 运行 CLI：

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=soraカタカナ... \
     --private-key=ed25519:<key>
   ```

3. 审查 `dispute_summary.json`（确认类型、证据摘要、时间戳）。
4. 通过治理事务队列向 Torii `/v1/sorafs/capacity/dispute` 提交请求 JSON。捕获`dispute_id_hex`响应值；它是后续撤销行动和审计报告的基础。

## 4. 撤消和撤销

1. **宽限期：**通知提供商即将撤销；在策略允许的情况下允许撤出固定数据。
2. **生成`ProviderAdmissionRevocationV1`：**
   - 使用 `sorafs_manifest_stub provider-admission revoke` 并提供批准的理由。
   - 验证签名和撤销摘要。
3. **发布撤销：**
   - 向 Torii 提交撤销请求。
   - 确保提供商广告被屏蔽（预计 `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` 会攀升）。
4. **更新仪表板：** 将提供商标记为已撤销，引用争议 ID，并链接证据包。

## 5. 验尸和后续行动

- 在治理事件跟踪器中记录时间线、根本原因和补救措施。
- 确定赔偿（削减权益、费用回扣、客户退款）。
- 记录学习内容；如果需要，更新 SLA 阈值或监控警报。

## 6. 参考资料

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md`（争议部分）
- `docs/source/sorafs/provider_admission_policy.md`（撤销工作流程）
- 可观测性仪表板：`SoraFS / Capacity Providers`

## 清单

- [ ] 捕获并散列的证据包。
- [ ] 争议负载已在本地验证。
- [ ] Torii 争议交易已接受。
- [ ] 已执行撤销（如果获得批准）。
- [ ] 仪表板/操作手册已更新。
- [ ] 向治理委员会提交尸检报告。