---
lang: zh-hans
direction: ltr
source: docs/account_structure_sdk_alignment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 164bd373091ae3280f9f90fcfd915a90088b0c79b8f3759ffd2548edb64d0a90
source_last_modified: "2026-01-28T17:11:30.632934+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 针对 SDK 和编解码器所有者的 I105 推出说明

团队：Rust SDK、TypeScript/JavaScript SDK、Python SDK、Kotlin SDK、编解码器工具

上下文：`docs/account_structure.md` 现在反映运输 I105 帐户 ID
实施。请将 SDK 行为和测试与规范规范保持一致。

主要参考资料：
- 地址编解码器 + 标头布局 — `docs/account_structure.md` §2
- 曲线注册表 — `docs/source/references/address_curve_registry.md`
- 规范 v1 域处理 — `docs/source/references/address_norm_v1.md`
- 夹具向量 — `fixtures/account/address_vectors.json`

行动项目：
1. **规范输出：** `AccountId::to_string()`/显示器必须仅发出 I105
   （无 `@domain` 后缀）。规范十六进制用于调试 (`0x...`)。
2. **Accepted inputs:** parsers MUST accept only canonical I105 account literals. Reject i105-default `sora...`, canonical hex (`0x...`), any `@<domain>` suffix, alias literals, legacy `norito:<hex>`, and `uaid:` / `opaque:` parser forms.
3. **Resolvers:** canonical account parsing has no default-domain binding, scoped inference, or fallback resolver path. Use `ScopedAccountId` only on interfaces that explicitly require `<account>@<domain>`.
4. **I105校验和：**使用Blake2b-512 over `I105PRE || prefix || payload`，取
   前 2 个字节。压缩字母基数为 **105**。
5. **曲线选通：** SDK 默认仅适用于 Ed25519。提供明确的选择加入
   ML‑DSA/GOST/SM（Swift 构建标志；JS/Android `configureCurveSupport`）。做
   不假设 secp256k1 在 Rust 之外默认启用。
6. **无 CAIP-10：** 尚未发布 CAIP-10 映射；不要暴露或
   取决于 CAIP-10 转换。

编解码器/测试更新后请确认；可以跟踪未解决的问题
在帐户寻址 RFC 线程中。