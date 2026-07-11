---
lang: zh-hant
direction: ltr
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 465d8cf704022986b169ab93133517428f8cf2ffe01a498cbda458f4a5b2e69b
source_last_modified: "2026-07-11T15:09:39+04:00"
translation_last_reviewed: 2026-07-11
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

## 單次驗證與確定性限額

- 每份 native 或 destination 證明只做一次規範解碼和一次昂貴的密碼學驗證。
  在密碼學運算前，共識先預留保守且與硬體無關的工作量估算。
- `[zk.sccp]` 為證明數量/位元組、native headers、Ethereum light-client
  updates、header bytes、secp256k1 recoveries、BLS 聚合檢查/簽名貢獻以及
  BN254 pairing-product checks 設定強制非零的 per-proof、per-transaction
  和 per-block 限額。這些准入限額綁定共識，所有驗證者必須一致。

## Torii 邊界

`/v1/bridge/proofs/submit` 和 `/v1/bridge/messages` 使用端點專屬的 HTTP body
上限。系統會在讀取 body 前檢查身分驗證、rate limit 和 `Content-Length`；
chunked body 只會讀取到硬上限。請求過大回傳 `413`，畸形 transport/JSON
另外回傳 `400`。Detached transaction payload 上限為 16 MiB，signature
payload 上限為 16 KiB。
