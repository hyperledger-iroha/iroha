---
lang: zh-hans
direction: ltr
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 69c9a740261d0c367d52870fc1f48775ae48307056ba9b79d2f811e0c0849f20
source_last_modified: "2026-07-11T15:09:39+04:00"
translation_last_reviewed: 2026-07-11
translator: machine-assisted
---

> 本页是截至 2026-07-11 的本地化简要摘要，并非完整的规范性译文。准确的
> 类型、API 契约和发布要求以[英文规范页](bridge_proofs.md)为准。

# SCCP V1 跨链证明——简要摘要

## 首发范围

- SCCP V1 是封闭接口：仅支持 Ethereum mainnet、BSC mainnet 和 TRON
  mainnet，SORA 侧唯一端点为 `sora-taira`。任何其他网络配置或 SORA
  身份都会被拒绝。
- `SubmitBridgeProof` 只接受与路由绑定的类型化 `NativeProtocol` 和
  `SccpDestination` 证明。通用 `Ics` 与 `TransparentZk` payload 提交并未
  开放，系统会以 fail-closed 方式拒绝它们。

## 类型化注册表与历史

- `SccpRegistryV1` 是类型化、仅追加的注册表。每条 lane 最多保留 64 个
  路由修订和 4,096 个 native trust anchor。记录不会被隐式淘汰；超过上限
  的下一次追加会被原子拒绝。
- Anchor 区间使用已认证的共识进度坐标：Ethereum 使用 finalized beacon
  slot，BSC/TRON 使用 finalized native block height。旧 anchor 的有效期包含
  后继 checkpoint，但不得越过它。
- 持久 inbound 记录分别保存 event/finality height 和
  `anchor_interval_height`。lane+anchor high-water 只能升高；后继 checkpoint
  不得低于它。Snapshot hydration 会完整重算索引，并拒绝缺失、陈旧或多余
  的值。重复使用 message id 或 replay 同样会被拒绝。

## 单次验证与确定性限额

- 每份 native 或 destination 证明只做一次规范解码和一次昂贵的密码学验证。
  在密码学运算前，共识先预留保守且与硬件无关的工作量估算。
- `[zk.sccp]` 为证明数量/字节、native headers、Ethereum light-client
  updates、header bytes、secp256k1 recoveries、BLS 聚合检查/签名贡献以及
  BN254 pairing-product checks 设置强制非零的 per-proof、per-transaction
  和 per-block 限额。这些准入限额绑定共识，所有验证者必须一致。

## Torii 边界

`/v1/bridge/proofs/submit` 和 `/v1/bridge/messages` 使用端点专属的 HTTP body
上限。系统会在读取 body 前检查身份验证、rate limit 和 `Content-Length`；
chunked body 只会读取到硬上限。请求过大返回 `413`，畸形 transport/JSON
单独返回 `400`。Detached transaction payload 上限为 16 MiB，signature
payload 上限为 16 KiB。
