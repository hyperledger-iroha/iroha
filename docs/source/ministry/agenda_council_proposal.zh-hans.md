---
lang: zh-hans
direction: ltr
source: docs/source/ministry/agenda_council_proposal.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2a7a47fdf0c80d189c912baafa5d6ce81a17a4c90f2b1797e532989a56f5060
source_last_modified: "2025-12-29T18:16:35.977493+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 议程理事会提案架构 (MINFO-2a)

路线图参考：**MINFO-2a — 提案格式验证器。**

议程委员会工作流程批量处理公民提交的黑名单和政策变更
治理小组审查提案之前。该文件定义了
规范的有效负载模式、证据要求和重复检测规则
由新验证器 (`cargo xtask ministry-agenda validate`) 消耗，因此
提案者可以在将 JSON 提交上传到门户之前在本地检查它们。

## 负载概述

议程提案使用 `AgendaProposalV1` Norito 架构
（`iroha_data_model::ministry::AgendaProposalV1`）。当以下情况时，字段被编码为 JSON：
通过 CLI/门户界面提交。

|领域 |类型 |要求|
|--------|------|--------------|
| `version` | `1` (u16) |必须等于 `AGENDA_PROPOSAL_VERSION_V1`。 |
| `proposal_id` |字符串 (`AC-YYYY-###`) |稳定的标识符；在验证期间强制执行。 |
| `submitted_at_unix_ms` | u64 | 64自 Unix 纪元以来的毫秒数。 |
| `language` |字符串| BCP-47 标签（`"en"`、`"ja-JP"` 等）。 |
| `action` |枚举（`add-to-denylist`、`remove-from-denylist`、`amend-policy`）|要求部采取行动。 |
| `summary.title` |字符串|建议≤256 个字符。 |
| `summary.motivation` |字符串|为什么需要采取该行动。 |
| `summary.expected_impact` |字符串|行动被接受后的结果。 |
| `tags[]` |小写字符串 |可选分类标签。允许的值：`csam`、`malware`、`fraud`、`harassment`、`impersonation`、`policy-escalation`、`terrorism`、`spam`。 |
| `targets[]` |对象|一个或多个哈希族条目（见下文）。 |
| `evidence[]` |对象|一份或多份证据附件（见下文）。 |
| `submitter.name` |字符串|显示名称或组织。 |
| `submitter.contact` |字符串|电子邮件、Matrix 手柄或电话；从公共仪表板编辑。 |
| `submitter.organization` |字符串（可选）|在审阅者 UI 中可见。 |
| `submitter.pgp_fingerprint` |字符串（可选）| 40 进制大写指纹。 |
| `duplicates[]` |字符串|对先前提交的提案 ID 的可选引用。 |

### 目标条目 (`targets[]`)

每个目标代表提案引用的哈希族摘要。

|领域 |描述 |验证 |
|--------|-------------|------------|
| `label` |审阅者上下文的友好名称。 |非空。 |
| `hash_family` |哈希标识符（`blake3-256`、`sha256` 等）。 | ASCII 字母/数字/`-_.`，≤48 个字符。 |
| `hash_hex` |以小写十六进制编码的摘要。 | ≥16 个字节（32 个十六进制字符）并且必须是有效的十六进制。 |
| `reason` |简短描述为什么应该对摘要采取行动。 |非空。 |

验证器拒绝同一组中重复的 `hash_family:hash_hex` 对
当相同的指纹已存在于提案和报告中时，提案和报告会发生冲突
重复注册表（见下文）。

### 证据附件 (`evidence[]`)

证据条目记录了审阅者可以获取支持上下文的位置。|领域|类型 |笔记|
|--------|------|--------|
| `kind` |枚举（`url`、`torii-case`、`sorafs-cid`、`attachment`）|确定消化要求。 |
| `uri` |字符串| HTTP(S) URL、Torii 案例 ID 或 SoraFS URI。 |
| `digest_blake3_hex` |字符串| `sorafs-cid` 和 `attachment` 类型需要；对于其他人来说是可选的。 |
| `description` |字符串|供审阅者选择的自由格式文本。 |

### 重复注册表

操作员可以维护现有指纹的注册表以防止重复
案例。验证器接受格式如下的 JSON 文件：

```json
{
  "entries": [
    {
      "hash_family": "blake3-256",
      "hash_hex": "0d714bed4b7c63c23a2cf8ee9ce6c3cde1007907c427b4a0754e8ad31c91338d",
      "proposal_id": "AC-2025-014",
      "note": "Already handled in 2025-08 incident"
    }
  ]
}
```

当提案目标与条目匹配时，验证器将中止，除非
指定了 `--allow-registry-conflicts`（仍然发出警告）。
使用 [`cargo xtask ministry-agenda impact`](impact_assessment_tooling.md) 来
生成交叉引用副本的公投就绪摘要
注册表和策略快照。

## CLI 用法

Lint 单个提案并根据重复的注册表检查它：

```bash
cargo xtask ministry-agenda validate \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json
```

通过 `--allow-registry-conflicts` 将重复命中降级为警告
执行历史审计。

CLI 依赖于相同的 Norito 模式和验证助手
`iroha_data_model`，因此 SDK/门户可以重用 `AgendaProposalV1::validate`
一致行为的方法。

## 排序 CLI (MINFO-2b)

路线图参考：**MINFO-2b — 多时隙抽签和审核日志。**

议程委员会名册现在通过确定性抽签进行管理，因此公民
可以独立审核每次抽奖。使用新命令：

```bash
cargo xtask ministry-agenda sortition \
  --roster docs/examples/ministry/agenda_council_roster.json \
  --slots 3 \
  --seed 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef \
  --out artifacts/ministry/agenda_sortition_2026Q1.json
```

- `--roster` — 描述每个合格成员的 JSON 文件：

  ```json
  {
    "format_version": 1,
    "members": [
      {
        "member_id": "citizen:ada",
        "weight": 2,
        "role": "citizen",
        "organization": "Artemis Cooperative"
      },
      {
        "member_id": "citizen:erin",
        "weight": 1,
        "role": "citizen",
        "eligible": false
      }
    ]
  }
  ```

  示例文件位于
  `docs/examples/ministry/agenda_council_roster.json`。可选字段（角色、
  组织、联系人、元数据）都在 Merkle 叶中捕获，因此审计员
  可以证明抽签的名单。

- `--slots` — 待填补的理事会席位数量。
- `--seed` — 32 字节 BLAKE3 种子（64 个小写十六进制字符）记录在
  抽签的治理记录。
- `--out` — 可选输出路径。省略时，JSON 摘要将打印到
  标准输出。

### 输出摘要

该命令发出 `SortitionSummary` JSON blob。样本输出存储在
`docs/examples/ministry/agenda_sortition_summary_example.json`。关键领域：

|领域|描述 |
|--------|-------------|
| `algorithm` |分类标签 (`agenda-sortition-blake3-v1`)。 |
| `roster_digest` |名册文件的 BLAKE3 + SHA-256 摘要（用于确认对同一成员列表进行审核）。 |
| `seed_hex` / `slots` |回显 CLI 输入，以便审核员可以重现抽签结果。 |
| `merkle_root_hex` |名册 Merkle 树的根（`hash_node`/`hash_leaf` 助手位于 `xtask/src/ministry_agenda.rs` 中）。 |
| `selected[]` |每个槽位的条目，包括规范成员元数据、合格索引、原始花名册索引、确定性绘制熵、叶哈希和 Merkle 证明同级。 |

### 验证平局1. 获取 `roster_path` 引用的名册并验证其 BLAKE3/SHA-256
   摘要与摘要相匹配。
2. 使用相同的种子/时段/名册重新运行 CLI；生成的 `selected[].member_id`
   顺序应与发布的摘要相符。
3. 对于特定成员，使用序列化成员 JSON 计算 Merkle 叶子
   (`norito::json::to_vec(&sortition_member)`) 并折叠每个证明哈希。决赛
   摘要必须等于 `merkle_root_hex`。示例摘要中的帮助程序显示
   如何组合 `eligible_index`、`leaf_hash_hex` 和 `merkle_proof[]`。

这些人工制品满足 MINFO-2b 对可验证随机性的要求，
k-of-m 选择和仅附加审核日志，直到连接链上 API。

## 验证错误参考

`AgendaProposalV1::validate` 发出 `AgendaProposalValidationError` 变体
每当有效负载无法进行 linting 时。下表总结了最常见的
错误，以便门户审核者可以将 CLI 输出转换为可操作的指导。|错误 |意义|修复|
|--------|---------|-------------|
| `UnsupportedVersion { expected, found }` |有效负载 `version` 与验证器支持的架构不同。 |使用最新的架构包重新生成 JSON，以便版本与 `expected` 匹配。 |
| `MissingProposalId` / `InvalidProposalIdFormat { value }` | `proposal_id` 为空或不是 `AC-YYYY-###` 形式。 |重新提交之前，请按照记录的格式填充唯一标识符。 |
| `MissingSubmissionTimestamp` | `submitted_at_unix_ms` 为零或缺失。 |记录提交时间戳（以 Unix 毫秒为单位）。 |
| `InvalidLanguageTag { value }` | `language` 不是有效的 BCP-47 标签。 |使用标准标签，例如 `en`、`ja-JP` 或 BCP-47 识别的其他区域设置。 |
| `MissingSummaryField { field }` | `summary.title`、`.motivation` 或 `.expected_impact` 之一为空。 |为指定的摘要字段提供非空文本。 |
| `MissingSubmitterField { field }` | `submitter.name` 或 `submitter.contact` 缺失。 |提供缺少的提交者元数据，以便审核者可以联系提案者。 |
| `InvalidTag { value }` | `tags[]` 条目不在允许列表中。 |删除标记或将其重命名为已记录的值之一（`csam`、`malware` 等）。 |
| `MissingTargets` | `targets[]` 数组为空。 |提供至少一个目标哈希族条目。 |
| `MissingTargetLabel { index }` / `MissingTargetReason { index }` |目标条目缺少 `label` 或 `reason` 字段。 |在重新提交之前填写索引条目的必填字段。 |
| `InvalidHashFamily { index, value }` |不支持的 `hash_family` 标签。 |将哈希系列名称限制为 ASCII 字母数字加 `-_`。 |
| `InvalidHashHex { index, value }` / `TargetDigestTooShort { index }` |摘要不是有效的十六进制或短于 16 字节。 |为索引目标提供小写十六进制摘要（≥32 个十六进制字符）。 |
| `DuplicateTarget { index, fingerprint }` |目标摘要复制较早的条目或注册表指纹。 |删除重复项或将支持证据合并为单个目标。 |
| `MissingEvidence` |没有提供证据附件。 |附上至少一份链接到复制材料的证据记录。 |
| `MissingEvidenceUri { index }` |证据条目缺少 `uri` 字段。 |提供索引证据条目的可获取 URI 或案例标识符。 |
| `MissingEvidenceDigest { index }` / `InvalidEvidenceDigest { index, value }` |需要摘要的证据条目（SoraFS CID 或附件）丢失或具有无效的 `digest_blake3_hex`。 |为索引条目提供 64 个字符的小写 BLAKE3 摘要。 |

## 示例

- `docs/examples/ministry/agenda_proposal_example.json` — 规范，
  带有两个证据附件的干净提案有效负载。
- `docs/examples/ministry/agenda_duplicate_registry.json` — 启动注册表
  包含单个 BLAKE3 指纹和基本原理。

集成门户工具或编写 CI 时，将这些文件重复用作模板
检查自动提交。