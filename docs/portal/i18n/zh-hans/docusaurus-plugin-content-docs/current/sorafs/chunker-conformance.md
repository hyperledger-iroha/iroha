---
id: chunker-conformance
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Chunker Conformance Guide
sidebar_label: Chunker Conformance
description: Requirements and workflows for preserving the deterministic SF1 chunker profile across fixtures and SDKs.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意规范来源
:::

本指南规定了每个实施必须遵循的要求
与 SoraFS 确定性分块配置文件 (SF1) 一致。它还
记录再生工作流程、签名策略和验证步骤，以便
跨 SDK 的灯具消费者保持同步。

## 规范配置文件

- 型材手柄：`sorafs.sf1@1.0.0`
- 输入种子（十六进制）：`0000000000dec0ded`
- 目标大小：262144 字节 (256KiB)
- 最小大小：65536 字节 (64KiB)
- 最大大小：524288 字节 (512KiB)
- 滚动多项式：`0x3DA3358B4DC173`
- 齿轮表种子：`sorafs-v1-gear`
- 中断掩码：`0x0000FFFF`

参考实现：`sorafs_chunker::chunk_bytes_with_digests_profile`。
任何 SIMD 加速都必须产生相同的边界和摘要。

## 夹具捆绑包

`cargo run --locked -p sorafs_chunker --bin export_vectors` 重新生成
固定装置并在 `fixtures/sorafs_chunker/` 下发出以下文件：

- `sf1_profile_v1.{json,rs,ts,go}` — Rust 的规范块边界，
  TypeScript 和 Go 消费者。每个文件都将规范句柄通告为
  `profile_aliases` 中的第一个（也是唯一一个）条目。该顺序由以下机构强制执行
  `ensure_charter_compliance` 并且不得更改。
- `manifest_blake3.json` — BLAKE3 验证的清单涵盖每个夹具文件。
- `manifest_signatures.json` — 舱单上的理事会签名 (Ed25519)
  消化。
- `sf1_profile_v1_backpressure.json` 和 `fuzz/` 内的原始语料库 —
  chunker 背压测试使用的确定性流场景。

### 签名政策

灯具再生**必须**包含有效的理事会签名。发电机
拒绝无符号输出，除非显式传递 `--allow-unsigned`（预期
仅用于本地实验）。签名信封仅供附加，
对每个签名者进行重复数据删除。

添加理事会签名：

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## 验证

CI 助手 `ci/check_sorafs_fixtures.sh` 重放生成器
`--locked`。如果夹具漂移或签名丢失，作业就会失败。使用
在每晚工作流程中以及提交夹具更改之前执行此脚本。

手动验证步骤：

1. 运行 `cargo test -p sorafs_chunker`。
2.本地调用`ci/check_sorafs_fixtures.sh`。
3. 确认 `git status -- fixtures/sorafs_chunker` 干净。

## 升级剧本

当提出新的 chunker 配置文件或更新 SF1 时：

另请参阅：[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
元数据要求、提案模板和验证清单。

1. 起草具有新参数的 `ChunkProfileUpgradeProposalV1`（请参阅 RFC SF-1）。
2. 通过 `export_vectors` 重新生成装置并记录新的清单摘要。
3. 以所需的理事会法定人数签署清单。所有签名必须
   附加到 `manifest_signatures.json`。
4. 更新受影响的 SDK 固定装置 (Rust/Go/TS) 并确保跨运行时奇偶校验。
5. 如果参数发生变化，重新生成模糊语料库。
6. 使用新的配置文件句柄、种子和摘要更新本指南。
7. 提交更改以及更新的测试和路线图更新。

不遵循此过程而影响块边界或摘要的更改
无效，不得合并。