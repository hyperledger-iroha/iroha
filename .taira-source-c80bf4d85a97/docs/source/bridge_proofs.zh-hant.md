---
lang: zh-hant
direction: ltr
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 74e29801129deccb6d5640d414289c47cf13fa9e0229fb55212b6c7710d7c5f7
source_last_modified: "2026-07-12T07:38:49.568351+00:00"
translation_last_reviewed: 2026-07-12
translator: machine-assisted
---

> 本頁是截至 2026-07-11 的本地化簡要摘要，並非完整的規範性譯文。準確的
> 類型、API 契約和發布要求以[英文規範頁](bridge_proofs.md)為準。

# SCCP V1 跨鏈證明——簡要摘要

## 首發範圍

- SCCP V1 是封閉介面：僅支援 Ethereum mainnet、BSC mainnet 和 TRON
  mainnet，SORA 側唯一端點為 `sora-taira`。任何其他網路設定或 SORA
  身分都會被拒絕。
- `SubmitBridgeProof` 只接受與路由綁定的型別化 `NativeProtocol` 和
  `SccpDestination` 證明。通用 `Ics` 與 `TransparentZk` payload 提交並未
  開放，系統會以 fail-closed 方式拒絕它們。

## 型別化註冊表與歷史

- `SccpRegistryV1` 是型別化、僅追加的註冊表。每條 lane 最多保留 64 個
  路由修訂和 4,096 個 native trust anchor。記錄不會被隱式淘汰；超過上限
  的下一次追加會被原子拒絕。
- Anchor 區間使用已認證的共識進度座標：Ethereum 使用 finalized beacon
  slot，BSC/TRON 使用 finalized native block height。舊 anchor 的有效期包含
  後繼 checkpoint，但不得越過它。
- 持久 inbound 記錄分別保存 event/finality height 和
  `anchor_interval_height`。lane+anchor high-water 只能升高；後繼 checkpoint
  不得低於它。Snapshot hydration 會完整重算索引，並拒絕缺失、陳舊或多餘
  的值。重複使用 message id 或 replay 同樣會被拒絕。

TRON 來源路由使用精確的
`transferToTaira(bytes,uint256,uint64 expectedNonce)` ABI。只有在
`expectedNonce == transferNonce` 時執行才會成功，之後系統會在遞增 storage
前將同一值寫入 canonical payload。Native 准入依 payload recipient、縮放後的
金額和 nonce 重建完整 ABI 呼叫。因此，已停用的雙參數 selector、過期或超前的
nonce，以及已耗盡的 `uint64` nonce 都會以 fail-closed 方式拒絕。

## 單次驗證與確定性限額

- 每份 native 或 destination 證明只做一次規範解碼和一次昂貴的密碼學驗證。
  在密碼學運算前，共識先預留保守且與硬體無關的工作量估算。
- `[zk.sccp]` 為證明數量/位元組、native headers、Ethereum light-client
  updates、header bytes、secp256k1 recoveries、BLS 聚合檢查/簽名貢獻以及
  BN254 pairing-product checks 設定強制非零的 per-proof、per-transaction
  和 per-block 限額。這些准入限額綁定共識，所有驗證者必須一致。

## 出站承諾、留存與探索

每則成功的 outbound message 都依區塊執行順序取得連續的 `commitment_index`
（`0..=511`）。V1 的固定上限是每區塊 512 則訊息、每則訊息 4,096 bytes 的 canonical
payload。`[zk.sccp]` 同時以 `max_pending_outbound_messages`（預設 `65536`）和
`max_pending_outbound_payload_bytes`（預設 `268435456`）限制待處理 payload 狀態。

Kura 在發布 finality 或淘汰 block body 之前，以 immutable 方式保存精確的 canonical
header 和由 root 認證的 SCCP archive。重建 proof、bundle、proof request 和 recent
history 不讀取歷史 block body，也不把 mutable WSV payload copy 當作證明材料。
destination proof 被接受後，待處理 payload 及其計費會 atomically 移除，並替換為保留
locator/index 的 fixed terminal descriptor。待處理狀態有硬上限；terminal records 和
immutable Kura history 為永久 replay protection 而有意持續增長。
`GET /v1/sccp/messages/recent` 使用 compound cursor `{ from, after_index }`。
Immutable evidence 計入 total/operator disk usage，但不計入 evictable-body budget。

## Torii 邊界

`/v1/bridge/proofs/submit` 和 `/v1/bridge/messages` 使用端點專屬的 HTTP body
上限。系統會在讀取 body 前檢查身分驗證、rate limit 和 `Content-Length`；
chunked body 只會讀取到硬上限。請求過大回傳 `413`，畸形 transport/JSON
另外回傳 `400`。Detached transaction payload 上限為 16 MiB，signature
payload 上限為 16 KiB。
