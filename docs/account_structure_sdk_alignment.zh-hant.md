---
lang: zh-hant
direction: ltr
source: docs/account_structure_sdk_alignment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 164bd373091ae3280f9f90fcfd915a90088b0c79b8f3759ffd2548edb64d0a90
source_last_modified: "2026-01-28T17:11:30.632934+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 針對 SDK 和編解碼器所有者的 I105 推出說明

團隊：Rust SDK、TypeScript/JavaScript SDK、Python SDK、Kotlin SDK、編解碼器工具

上下文：`docs/account_structure.md` 現在反映運輸 I105 帳戶 ID
實施。請將 SDK 行為和測試與規範規範保持一致。

主要參考資料：
- 地址編解碼器 + 標頭佈局 — `docs/account_structure.md` §2
- 曲線註冊表 — `docs/source/references/address_curve_registry.md`
- 規範 v1 域處理 — `docs/source/references/address_norm_v1.md`
- 夾具向量 — `fixtures/account/address_vectors.json`

行動項目：
1. **規範輸出：** `AccountId::to_string()`/顯示器必須僅發出 I105
   （無 `@domain` 後綴）。規範十六進制用於調試 (`0x...`)。
2. **Accepted inputs:** parsers MUST accept only canonical I105 account literals. Reject i105-default `sora...`, canonical hex (`0x...`), any `@<domain>` suffix, alias literals, legacy `norito:<hex>`, and `uaid:` / `opaque:` parser forms.
3. **Resolvers:** canonical account parsing has no default-domain binding, scoped inference, or fallback resolver path. Use `ScopedAccountId` only on interfaces that explicitly require `<account>@<domain>`.
4. **I105校驗和：**使用Blake2b-512 over `I105PRE || prefix || payload`，取
   前 2 個字節。壓縮字母基數為 **105**。
5. **曲線選通：** SDK 默認僅適用於 Ed25519。提供明確的選擇加入
   ML‑DSA/GOST/SM（Swift 構建標誌；JS/Android `configureCurveSupport`）。做
   不假設 secp256k1 在 Rust 之外默認啟用。
6. **無 CAIP-10：** 尚未發布 CAIP-10 映射；不要暴露或
   取決於 CAIP-10 轉換。

編解碼器/測試更新後請確認；可以跟踪未解決的問題
在帳戶尋址 RFC 線程中。