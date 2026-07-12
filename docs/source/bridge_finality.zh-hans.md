---
lang: zh-hans
direction: ltr
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1cbd248fe14e63d00f002f09e1663181f3ab9bd99124ffeb89c56763b784046b
source_last_modified: "2026-07-12"
translation_last_reviewed: 2026-07-12
translator: machine-google-reviewed
---

<!--
SPDX-License-Identifier: Apache-2.0
-->

# 桥接最终性证明

本文定义首次发布的桥接最终性格式。证明携带 Sumeragi v2 生成并持久化的精确最终性证据。
证明外层的模式版本为 `1`，其中的共识协议版本为 `2`。不存在 Sumeragi v1 证书投影、
解码器或回退路径。

## 精确证明格式

采用 Norito 或 Norito JSON 编码的 `BridgeFinalityProof` 只有三个字段：

```text
{ version, block_header, finality_artifact }
```

- `version` 必须为 `1`；
- `block_header` 是请求高度的规范 `BlockHeader`；
- `finality_artifact` 是该区块持久化的精确 `V2FinalityArtifact`。它按高度上下文中的
  验证器名册顺序，持久地内嵌每个验证器的 BLS-normal PoP（`validator_set_pops`）。

该制品包含完整且不可变的 `HeightContext`、精确的 `BlockSubject`、区块哈希、CommitQC
和与名册对齐的 PoP。高度上下文冻结链、epoch、名册、`DualQuorum`、DA 布局和 leader
seed 等共识数据。结束 epoch 的父区块上下文还包含可选的 `next_epoch_snapshot`；该字段
参与 context id，因此父 CommitQC 会先认证它，之后它才能授权子名册。最终快照还会认证
`epoch_end_height`、下一名册对齐的 `validator_set_pops` 以及下一 epoch 的参数。

## 持久化与验证

Kura 在发布最终性或淘汰区块体之前，先将精确的规范 header 和由根认证的 SCCP archive
写入不可变 retained-block 记录，再把精确 V2 artifact 保存到单独的不可变最终性记录。
两次写入都具有幂等、禁止覆盖的语义，并拒绝同一高度的冲突。`build_finality_proof`
只读取 retained header 和已验证的最终性记录，绝不读取历史区块体，也不会用可变 world
state 替换 PoP。重启清点会再次验证 header/archive/artifact/hash 的关联。淘汰区块体不会
使有效 proof 失效；记录缺失、损坏、冲突或不可验证时一律关闭失败。

无状态验证器严格核对版本、链、高度、header 哈希、规范前驱和 view、上下文、subject 和
CommitQC，并验证制品中的全部 PoP。签名者索引必须严格递增且在范围内；CommitQC 必须同时满足验证器数量
和投票权重两个 quorum，并且针对精确 Sumeragi v2 投票 preimage 的 BLS 聚合签名必须有效。

## 信任锚与后继验证

单个证明只能说明它在自身携带的名册下内部一致。`BridgeFinalityVerifier` 在接受首个证明前
必须获得显式信任的 `HeightContextId`。此后它只接受紧邻的下一高度，并使用前一个冻结名册
及其 PoP 验证子上下文的父 CommitQC。epoch 内的子制品复制前一制品的 PoP；在 epoch 边界，
epoch、名册、quorum、seed 和 PoP 必须匹配前一个父上下文中由其 CommitQC 认证的
`next_epoch_snapshot`，包括已认证的 `epoch_end_height`。旧高度、跳跃高度和未链接后继都会被拒绝。

SCCP 使用同一个 `BridgeFinalityProof`。不能只信任消息自带名册下的签名；必须从治理固定的
checkpoint context/artifact 开始，验证到消息制品为止的每个紧邻后继。

## Bundle 与 API

`BridgeFinalityBundle` 恰好为 `{ commitment, finality_proof }`。commitment 为
`{ chain_id, height_context_id, block_height, block_hash }`。

- `GET /v1/bridge/finality/{height}` 返回 `BridgeFinalityProof`；
- `GET /v1/bridge/finality/bundle/{height}` 返回 `BridgeFinalityBundle`。

若 retained canonical header 或精确的持久 v2 制品缺失或无效，两个端点都会关闭失败。
淘汰历史区块体不会使有效 proof 无法取得。消费者必须拒绝未知字段、不支持的版本和已废弃的证明形状。
