---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9b84ca0af25021c9ef883fb207c2af7226569cb8e750e05edbb15384474f86d0
source_last_modified: "2026-01-05T09:28:11.858804+00:00"
translation_last_reviewed: 2026-02-07
id: chunker-registry-charter
title: SoraFS Chunker Registry Charter
sidebar_label: Chunker Registry Charter
description: Governance charter for chunker profile submissions and approvals.
translator: machine-google-reviewed
---

:::注意规范来源
:::

# SoraFS Chunker 注册管理章程

> **批准时间：** 2025 年 10 月 29 日由 Sora 议会基础设施小组批准（参见
> `docs/source/sorafs/council_minutes_2025-10-29.md`)。任何修改都需要
> 正式治理投票；实施团队必须将此文件视为
> 在替代章程获得批准之前保持规范。

本章程定义了 SoraFS 分块器发展的流程和角色
注册表。它通过描述新功能如何补充 [Chunker Profile Authoring Guide](./chunker-profile-authoring.md)

## 范围

该章程适用于 `sorafs_manifest::chunker_registry` 中的每个条目以及
任何使用注册表的工具（清单 CLI、provider-advert CLI、
SDK）。它强制执行别名和句柄不变量检查
`chunker_registry::ensure_charter_compliance()`：

- 配置文件 ID 是单调递增的正整数。
- 规范句柄 `namespace.name@semver` **必须** 作为第一个出现
- 别名字符串经过修剪、独特，并且不会与规范句柄发生冲突
  其他条目。

## 角色

- **作者** – 准备提案、重新生成装置并收集
  决定论的证据。
- **工具工作组 (TWG)** – 使用已发布的建议验证提案
  检查清单并确保注册表不变量保持不变。
- **治理委员会 (GC)** – 审查 TWG 报告，签署提案
  信封，并批准发布/弃用时间表。
- **存储团队** – 维护注册表实施并发布
  文档更新。

## 生命周期工作流程

1. **提案提交**
   - 作者运行创作指南中的验证清单并创建
     下的 `ChunkerProfileProposalV1` JSON
     `docs/source/sorafs/proposals/`。
   - 包括 CLI 输出：
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - 提交包含固定装置、提案、确定性报告和注册表的 PR
     更新。

2. **工具审查 (TWG)**
   - 重播验证清单（夹具、模糊、清单/PoR 管道）。
   - 运行 `cargo test -p sorafs_car --chunker-registry` 并确保
     `ensure_charter_compliance()` 通过新条目。
   - 验证 CLI 行为（`--list-profiles`、`--promote-profile`、流式传输
     `--json-out=-`) 反映了更新的别名和句柄。
   - 制作一份简短的报告，总结调查结果和通过/失败状态。

3. **理事会批准 (GC)**
   - 审查 TWG 报告和提案元数据。
   - 签署提案摘要 (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     并将签名附加到与理事会一起保存的理事会信封上
     固定装置。
   - 在治理会议记录中记录投票结果。

4. **发表**
   - 合并 PR，更新：
     - `sorafs_manifest::chunker_registry_data`。
     - 文档（`chunker_registry.md`，创作/一致性指南）。
     - 赛程和决定论报告。
   - 通知运营商和 SDK 团队新的配置文件和计划的推出。

5. **弃用/日落**
   - 取代现有配置文件的提案必须包含双重发布
     窗口（宽限期）和升级计划。
     在注册表中并更新迁移分类帐。

6. **紧急变更**
   - 删除或修补程序需要理事会投票并获得多数批准。
   - TWG 必须记录风险缓解步骤并更新事件日志。

## 工具期望

- `sorafs_manifest_chunk_store` 和 `sorafs_manifest_stub` 暴露：
  - `--list-profiles` 用于注册检查。
  - `--promote-profile=<handle>` 生成使用的规范元数据块
    推广个人资料时。
  - `--json-out=-` 将报告流式传输到标准输出，从而实现可重复的审查
    日志。
- `ensure_charter_compliance()` 在启动时在相关二进制文件中被调用
  （`manifest_chunk_store`、`provider_advert_stub`）。如果是新的，CI 测试一定会失败
  条目违反了章程。

## 记录保存

- 将所有确定性报告存储在 `docs/source/sorafs/reports/` 中。
- 引用分块决策的理事会会议记录如下
  `docs/source/sorafs/migration_ledger.md`。
- 每次主要注册表更改后更新 `roadmap.md` 和 `status.md`。

## 参考文献

- 创作指南：[Chunker Profile创作指南](./chunker-profile-authoring.md)
- 一致性检查表：`docs/source/sorafs/chunker_conformance.md`
- 注册表参考：[Chunker 配置文件注册表](./chunker-registry.md)