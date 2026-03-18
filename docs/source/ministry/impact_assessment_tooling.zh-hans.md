---
lang: zh-hans
direction: ltr
source: docs/source/ministry/impact_assessment_tooling.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 89be62d7bb2bb79fd994d207489d310ef4c997be53447fbee8ac1f7b758d3beb
source_last_modified: "2025-12-29T18:16:35.978367+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# 影响评估工具 (MINFO-4b)

路线图参考：**MINFO-4b — 影响评估工具。**  
所有者：治理委员会/Analytics

本注释记录了现在的 `cargo xtask ministry-agenda impact` 命令
生成公投数据包所需的自动哈希族差异。的
工具使用经过验证的议程理事会提案、重复注册表以及
可选的拒绝名单/策略快照，以便审核者可以准确地看到哪些
指纹是新的，与现有策略相冲突，以及有多少条目
每个哈希家族都有贡献。

## 输入

1. **议程提案。** 随后的一个或多个文件
   [`docs/source/ministry/agenda_council_proposal.md`](agenda_council_proposal.md)。
   使用 `--proposal <path>` 显式传递它们或将命令指向
   通过 `--proposal-dir <dir>` 的目录以及该路径下的每个 `*.json` 文件
   包括在内。
2. **重复注册表（可选）。** 匹配的 JSON 文件
   `docs/examples/ministry/agenda_duplicate_registry.json`。冲突是
   报告为 `source = "duplicate_registry"`。
3. **策略快照（可选）。** 列出每个策略的轻量级清单
   GAR/部委政策已强制实施指纹。装载机期望
   架构如下所示（参见
   [`docs/examples/ministry/policy_snapshot_example.json`](../../examples/ministry/policy_snapshot_example.json)
   完整的样本）：

```json
{
  "snapshot_id": "denylist-2026-03",
  "generated_at": "2026-03-31T12:00:00Z",
  "entries": [
    {
      "hash_family": "blake3-256",
      "hash_hex": "…",
      "policy_id": "denylist-2025-014-entry-01",
      "note": "Already quarantined by GAR case CSAM-2025-014."
    }
  ]
}
```

任何 `hash_family:hash_hex` 指纹与提议目标匹配的条目都是
在 `source = "policy_snapshot"` 下报告，并引用 `policy_id`。

## 用法

```bash
cargo xtask ministry-agenda impact \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json \
  --policy-snapshot docs/examples/ministry/policy_snapshot_example.json \
  --out artifacts/ministry/impact/AC-2026-001.json
```

可以通过重复的 `--proposal` 标志或通过
提供包含整个公投批次的目录：

```bash
cargo xtask ministry-agenda impact \
  --proposal-dir artifacts/ministry/proposals/2026-03-31 \
  --registry state/agenda_duplicate_registry.json \
  --out artifacts/ministry/impact/2026-03-31.json
```

当省略 `--out` 时，该命令将生成的 JSON 打印到标准输出。

## 输出

该报告是一份已签署的人工制品（将其记录在公投数据包的下方）
`artifacts/ministry/impact/`目录），结构如下：

```json
{
  "format_version": 1,
  "generated_at": "2026-03-31T12:34:56Z",
  "totals": {
    "proposals_analyzed": 4,
    "targets_analyzed": 17,
    "registry_conflicts": 2,
    "policy_conflicts": 1,
    "hash_families": [
      { "hash_family": "blake3-256", "targets": 12, "registry_conflicts": 2, "policy_conflicts": 0 },
      { "hash_family": "sha256", "targets": 5, "registry_conflicts": 0, "policy_conflicts": 1 }
    ]
  },
  "proposals": [
    {
      "proposal_id": "AC-2026-001",
      "action": "add-to-denylist",
      "total_targets": 2,
      "source_path": "docs/examples/ministry/agenda_proposal_example.json",
      "hash_families": [
        { "hash_family": "blake3-256", "targets": 2, "registry_conflicts": 1, "policy_conflicts": 0 }
      ],
      "conflicts": [
        {
          "source": "duplicate_registry",
          "hash_family": "blake3-256",
          "hash_hex": "0d714bed…1338d",
          "reference": "AC-2025-014",
          "note": "Already quarantined."
        }
      ],
      "registry_conflicts": 1,
      "policy_conflicts": 0
    }
  ]
}
```

将此 JSON 与中立摘要一起附加到每个公投档案中，以便
小组成员、陪审员和治理观察员可以看到确切的爆炸半径
每个提案。输出是确定性的（按哈希族排序）并且可以安全地
包含在 CI/运行手册中；如果重复的注册表或策略快照发生更改，
在投票开始之前重新运行命令并附加刷新的工件。

> **下一步：** 将生成的影响报告输入
> [`cargo xtask ministry-panel packet`](referendum_packet.md) 所以
> `ReferendumPacketV1` 档案包含哈希家族细分和
> 正在审查的提案的详细冲突列表。