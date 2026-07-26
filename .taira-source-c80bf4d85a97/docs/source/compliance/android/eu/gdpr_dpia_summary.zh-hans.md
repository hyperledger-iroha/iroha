---
lang: zh-hans
direction: ltr
source: docs/source/compliance/android/eu/gdpr_dpia_summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8ef338a20104dc5d15094e28a1332a604b68bdcfef1ff82fea784d43fdbd10b5
source_last_modified: "2025-12-29T18:16:35.925474+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# GDPR DPIA 摘要 — Android SDK 遥测 (AND7)

|领域 |价值|
|--------|--------|
|评估日期| 2026-02-12 |
|处理活动| Android SDK 遥测导出到共享 OTLP 后端 |
|控制器/处理器| SORA Nexus Ops（控制器）、合作伙伴操作员（联合控制器）、Hyperledger Iroha 贡献者（处理器）|
|参考文档 | `docs/source/sdk/android/telemetry_redaction.md`、`docs/source/android_support_playbook.md`、`docs/source/android_runbook.md` |

## 1. 处理说明

- **目的：** 提供支持 AND7 可观察性（延迟、重试、证明运行状况）所需的操作遥测，同时镜像 Rust 节点模式（`telemetry_redaction.md` 的第 2 节）。
- **系统：** Android SDK 工具 -> OTLP 导出器 -> 由 SRE 管理的共享遥测收集器（请参阅支持手册第 8 节）。
- **数据主体：** 使用基于 Android SDK 的应用程序的运营商工作人员；下游 Torii 端点（权限字符串根据遥测策略进行哈希处理）。

## 2. 数据清单和缓解措施

|频道|领域 | PII 风险 |缓解措施 |保留|
|--------|--------|----------|------------|------------|
|迹线（`android.torii.http.request`、`android.torii.http.retry`）|路由、状态、延迟 |低（无 PII）|使用 Blake2b + 旋转盐进行哈希处理的权限；没有导出有效负载主体。 | 7-30 天（每个遥测文档）。 |
|事件 (`android.keystore.attestation.result`) |别名标签、安全级别、认证摘要 |中等（运营数据）|别名散列 (`alias_label`)、`actor_role_masked` 记录用于使用 Norito 审核令牌进行覆盖。 | 90 天成功，365 天覆盖/失败。 |
|指标（`android.pending_queue.depth`、`android.telemetry.export.status`）|队列计数、导出器状态 |低|仅汇总计数。 | 90 天。 |
|设备轮廓仪表 (`android.telemetry.device_profile`) | SDK专业，硬件层|中等|分桶（模拟器/消费者/企业），无 OEM 或序列号。 | 30天。 |
|网络上下文事件 (`android.telemetry.network_context`) |网络类型、漫游标志|中等|运营商名称完全被删除；支持合规性要求以避免订阅者数据。 | 7天。 |

## 3. 合法依据和权利

- **法律依据：** 合法利益（第 6(1)(f) 条）——确保受监管账本客户的可靠运行。
- **必要性测试：** 仅限于运行状况的指标（无用户内容）；散列权限通过仅授权支持人员可用的可逆映射（通过覆盖工作流程）确保与 Rust 节点的奇偶性。
- **平衡测试：** 遥测的范围是操作员控制的设备，而不是最终用户数据。覆盖需要由支持 + 合规部门（支持手册 §3 + §9）审核的已签名 Norito 工件。
- **数据主体权利：** 操作员联系支持工程（剧本§2）请求遥测数据导出/删除。密文覆盖和日志（遥测文档§信号清单）可在 30 天内实现。

## 4. 风险评估|风险|可能性|影响 |残留缓解|
|------|------------|--------|----------|
|通过哈希权威重新识别 |低|中等|通过`android.telemetry.redaction.salt_version`记录盐的旋转；储存在安全保险库中的盐；覆盖季度审计。 |
|通过配置文件存储桶进行设备指纹识别 |低|中等|仅导出tier+SDK专业；支持 Playbook 禁止 OEM/串行数据的升级请求。 |
|覆盖滥用 PII 泄露 |极低|高|已记录 Norito 覆盖请求，24 小时内过期，需要 SRE 批准（`docs/source/android_runbook.md` §3）。 |
|欧盟以外的遥测存储 |中等|中等|收集器部署在欧盟+日本地区；通过 OTLP 后端配置强制执行保留策略（记录在支持手册第 8 节中）。 |

鉴于上述控制和持续监控，剩余风险被认为是可以接受的。

## 5. 行动和后续行动

1. **季度审查：** 验证遥测模式、盐轮换和覆盖日志；文档位于 `docs/source/sdk/android/telemetry_redaction_minutes_YYYYMMDD.md`。
2. **跨 SDK 对齐：** 与 Swift/JS 维护者协调，以维护一致的哈希/分桶规则（在路线图 AND7 中跟踪）。
3. **合作伙伴通信：** 在合作伙伴入门套件中包含 DPIA 摘要（支持手册第 9 节）并从 `status.md` 链接到此文档。