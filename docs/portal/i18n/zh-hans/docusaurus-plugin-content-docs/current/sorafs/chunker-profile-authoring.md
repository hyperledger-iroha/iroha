---
id: chunker-profile-authoring
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Chunker Profile Authoring Guide
sidebar_label: Chunker Authoring Guide
description: Checklist for proposing new SoraFS chunker profiles and fixtures.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意规范来源
:::

# SoraFS Chunker 配置文件创作指南

本指南介绍了如何为 SoraFS 提议和发布新的分块配置文件。
它补充了架构 RFC (SF-1) 和注册表参考 (SF-2a)
具有具体的创作要求、验证步骤和提案模板。
有关规范示例，请参阅
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
以及随附的试运行登录
`docs/source/sorafs/reports/sf1_determinism.md`。

## 概述

进入注册表的每个配置文件都必须：

- 公布相同的确定性 CDC 参数和多重哈希设置
  架构；
- 发布可重玩的装置（Rust/Go/TS JSON + 模糊语料库 + PoR 见证）
  下游 SDK 无需定制工具即可验证；
- 包括治理就绪元数据（命名空间、名称、semver）以及迁移
- 在理事会审查之前通过确定性差异套件。

请按照下面的清单准备满足这些规则的提案。

## 注册章程快照

在起草提案之前，请确认其符合强制执行的注册管理机构章程
通过 `sorafs_manifest::chunker_registry::ensure_charter_compliance()`：

- 配置文件 ID 是单调递增且无间隙的正整数。
- 规范句柄 (`namespace.name@semver`) 必须出现在别名列表中
  并且**必须**是第一个条目。
- 别名不得与其他规范句柄冲突或出现多次。
- 别名必须非空并删除空格。

方便的 CLI 助手：

```bash
# JSON listing of all registered descriptors (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Emit metadata for a candidate default profile (canonical handle + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

这些命令使提案与注册管理机构章程保持一致，并提供
治理讨论中所需的规范元数据。

## 所需的元数据

|领域|描述 |示例 (`sorafs.sf1@1.0.0`) |
|--------|-------------|------------------------------|
| `namespace` |相关配置文件的逻辑分组。 | `sorafs` |
| `name` |人类可读的标签。 | `sf1` |
| `semver` |参数集的语义版本字符串。 | `1.0.0` |
| `profile_id` |配置文件登陆后分配的单调数字标识符。保留下一个 ID，但不要重复使用现有号码。 | `1` |
| `profile_aliases` |在谈判期间向客户公开的可选附加句柄。始终将规范句柄作为第一个条目。 | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` |最小块长度（以字节为单位）。 | `65536` |
| `profile.target_size` |目标块长度（以字节为单位）。 | `262144` |
| `profile.max_size` |最大块长度（以字节为单位）。 | `524288` |
| `profile.break_mask` |滚动哈希（十六进制）使用的自适应掩码。 | `0x0000ffff` |
| `profile.polynomial` |齿轮多项式常数（十六进制）。 | `0x3da3358b4dc173` |
| `gear_seed` |用于派生 64KiB 齿轮表的种子。 | `sorafs-v1-gear` |
| `chunk_multihash.code` |每个块摘要的多重哈希代码。 | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` |规范固定装置包的摘要。 | `13fa...c482` |
| `fixtures_root` |包含重新生成的装置的相对目录。 | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` |确定性 PoR 采样的种子 (`splitmix64`)。 | `0xfeedbeefcafebabe`（示例）|

元数据必须同时出现在提案文档和生成的提案文档中
固定装置，以便注册表、CLI 工具和治理自动化可以确认
无需手动交叉引用的值。如有疑问，请运行 chunk-store 并
带有 `--json-out=-` 的清单 CLI，用于将计算的元数据流式传输以供审核
笔记。

### CLI 和注册表接触点

- `sorafs_manifest_chunk_store --profile=<handle>` – 重新运行块元数据，
  清单摘要，PoR 使用建议的参数进行检查。
- `sorafs_manifest_chunk_store --json-out=-` – 将块存储报告流式传输至
  用于自动比较的标准输出。
- `sorafs_manifest_stub --chunker-profile=<handle>` – 确认舱单和 CAR
  计划嵌入规范句柄和别名。
- `sorafs_manifest_stub --plan=-` – 反馈之前的 `chunk_fetch_specs`
  来验证更改后的偏移量/摘要。

在提案中记录命令输出（摘要、PoR 根、清单哈希）
这样审稿人就可以逐字重现它们。

## 确定性和验证清单

1. **重新生成灯具**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **运行奇偶校验套件** – `cargo test -p sorafs_chunker` 和
   跨语言差异工具 (`crates/sorafs_chunker/tests/vectors.rs`) 必须是
   绿色，新装置就位。
3. **重放模糊/背压语料库** – 执行 `cargo fuzz list` 和
   针对再生资产的流式处理（`fuzz/sorafs_chunker`）。
4. **验证可检索性证明见证** – 运行
   `sorafs_manifest_chunk_store --por-sample=<n>` 使用建议的配置文件和
   确认根与夹具清单匹配。
5. **CI 试运行** – 本地调用 `ci/check_sorafs_fixtures.sh`；脚本
   新的灯具和现有的 `manifest_signatures.json` 应该会成功。
6. **跨运行时确认** – 确保 Go/TS 绑定消耗重新生成的
   JSON 并发出相同的块边界和摘要。

在提案中记录命令和生成的摘要，以便工具工作组
可以重新运行它们而无需猜测。

### 清单/PoR 确认

重新生成固定装置后，运行完整的清单管道以确保 CAR
元数据和 PoR 证明保持一致：

```bash
# Validate chunk metadata + PoR with the new profile
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Generate manifest + CAR and capture chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Re-run using the saved fetch plan (guards against stale offsets)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

将输入文件替换为您的装置使用的任何代表性语料库
（例如，1GiB 确定性流）并将生成的摘要附加到
提案。

## 提案模板

提案作为已签入的 `ChunkerProfileProposalV1` Norito 记录提交
`docs/source/sorafs/proposals/`。下面的 JSON 模板说明了预期的结果
形状（根据需要替换您的值）：


提供匹配的 Markdown 报告 (`determinism_report`)，以捕获
命令输出、块摘要以及验证过程中遇到的任何偏差。

## 治理工作流程

1. **提交带有提案+固定装置的 PR。** 包括生成的资产、
   Norito提案，并更新为`chunker_registry_data.rs`。
2. **工具工作组审查。** 审查人员重新运行验证清单并确认
   该提案符合注册管理机构规则（无 ID 重用，满足确定性）。
3. **理事会信封。** 一旦获得批准，理事会成员签署提案摘要
   (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) 并附加它们
   与灯具一起存储的配置文件信封的签名。
4. **注册表发布。** 合并会影响注册表、文档和固定装置。的
   默认 CLI 保留在以前的配置文件上，直到治理声明
   迁移准备就绪。
5. **弃用跟踪。** 在迁移窗口之后，将注册表更新为
   分类帐。

## 创作技巧

- 甚至更喜欢二次幂边界以最小化边缘情况分块行为。
- 避免在未协调清单和网关的情况下更改多重哈希代码
- 保持齿轮表种子可读但全球唯一，以简化审核
  踪迹。
- 将任何基准测试工件（例如吞吐量比较）存储在
  `docs/source/sorafs/reports/` 供将来参考。

有关推出期间的运营预期，请参阅迁移分类账
（`docs/source/sorafs/migration_ledger.md`）。有关运行时一致性规则，请参阅
`docs/source/sorafs/chunker_conformance.md`。