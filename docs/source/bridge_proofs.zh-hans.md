---
lang: zh-hans
direction: ltr
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 74e29801129deccb6d5640d414289c47cf13fa9e0229fb55212b6c7710d7c5f7
source_last_modified: "2026-07-12T07:38:49.568351+00:00"
translation_last_reviewed: 2026-07-12
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

TRON 源路由使用精确的
`transferToTaira(bytes,uint256,uint64 expectedNonce)` ABI。仅当
`expectedNonce == transferNonce` 时执行才会成功，随后系统会在递增 storage
之前将同一值写入规范 payload。Native 准入根据 payload recipient、缩放后的
金额和 nonce 重建完整 ABI 调用。因此，已弃用的双参数 selector、过期或超前的
nonce，以及已耗尽的 `uint64` nonce 都会以 fail-closed 方式拒绝。

## 单次验证与确定性限额

- 每份 native 或 destination 证明只做一次规范解码和一次昂贵的密码学验证。
  在密码学运算前，共识先预留保守且与硬件无关的工作量估算。
- `[zk.sccp]` 为证明数量/字节、native headers、Ethereum light-client
  updates、header bytes、secp256k1 recoveries、BLS 聚合检查/签名贡献以及
  BN254 pairing-product checks 设置强制非零的 per-proof、per-transaction
  和 per-block 限额。这些准入限额绑定共识，所有验证者必须一致。

## 出站承诺、留存与发现

每条成功的 outbound message 都按区块执行顺序获得连续的 `commitment_index`
（`0..=511`）。V1 的固定上限是每区块 512 条消息、每条消息 4,096 字节规范
payload。`[zk.sccp]` 同时使用 `max_pending_outbound_messages`（默认 `65536`）和
`max_pending_outbound_payload_bytes`（默认 `268435456`）限制待处理 payload 状态。

Kura 在发布最终性或淘汰区块体之前，以不可变方式保存精确的规范 header 和由根认证的
SCCP archive。重建 proof、bundle、proof request 和 recent history 不读取历史区块体，
也不把可变 WSV payload 副本当作证明材料。destination proof 被接受后，待处理 payload
及其计费会原子删除，并替换为保留 locator/index 的固定大小 terminal descriptor。
待处理状态有硬上限；terminal records 和不可变 Kura history 为永久防重放而有意持续增长。
`GET /v1/sccp/messages/recent` 使用复合 cursor `{ from, after_index }`。不可变证据
计入总磁盘/运营者磁盘用量，但不计入可淘汰区块体预算。

## Torii 边界

`/v1/bridge/proofs/submit` 和 `/v1/bridge/messages` 使用端点专属的 HTTP body
上限。系统会在读取 body 前检查身份验证、rate limit 和 `Content-Length`；
chunked body 只会读取到硬上限。请求过大返回 `413`，畸形 transport/JSON
单独返回 `400`。Detached transaction payload 上限为 16 MiB，signature
payload 上限为 16 KiB。
