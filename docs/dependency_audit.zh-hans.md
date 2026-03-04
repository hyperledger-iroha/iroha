---
lang: zh-hans
direction: ltr
source: docs/dependency_audit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb0a770fac1086462d949dbf17dd5a05f133169e57d50b0d90ddb48ae05f2853
source_last_modified: "2026-01-05T09:28:11.822642+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//！依赖性审计摘要

日期：2025-09-01

范围：工作区范围内对 Cargo.toml 文件中声明并在 Cargo.lock 中解决的所有 crate 进行审查。针对 RustSec 咨询数据库进行货物审计，并手动审查包的合法性和算法的“主包”选择。

工具/命令运行：
- `cargo tree -d --workspace --locked --offline` – 检查重复版本
- `cargo audit` – 扫描 Cargo.lock 是否存在已知漏洞和拉扯的板条箱

发现安全公告（现在 0 个漏洞；2 个警告）：
- 横梁通道 — RUSTSEC-2025-0024
  - 修正：在 `crates/ivm/Cargo.toml` 中撞到 `0.5.15`。

  - 修正：在 `crates/iroha_torii/Cargo.toml` 中将 `pprof` 翻转为 `prost-codec`。

- 环 — RUSTSEC-2025-0009
  - 修复：增加了 QUIC/TLS 堆栈（`quinn 0.11`、`rustls 0.23`、`tokio-rustls 0.26`）并将 WS 堆栈更新为 `tungstenite/tokio-tungstenite 0.24`。通过 `cargo update -p ring --precise 0.17.12` 强制锁定 `ring 0.17.12`。

其余建议：无。剩余警告：`backoff`（未维护）、`derivative`（未维护）。

合法性和“主要箱子”评估（焦点）：
- 哈希：`sha2` (RustCrypto)、`blake2` (RustCrypto)、`tiny-keccak`（广泛使用）——规范选择。
- AEAD/对称：`aes-gcm`、`chacha20poly1305`、`aead` 特征 (RustCrypto) — 规范。
- 签名/ECC：`ed25519-dalek`、`x25519-dalek`（dalek 项目）、`k256`（RustCrypto）、`secp256k1`（libsecp 绑定）——全部合法；更喜欢单个 secp256k1 堆栈（`k256` 对于纯 Rust 或 `secp256k1` 对于 libsecp）以减少表面积。
- BLS12-381/ZK：`blstrs`、`halo2_*` — 广泛应用于生产 ZK 生态系统；合法的。
- PQ：`pqcrypto-dilithium`、`pqcrypto-traits` — 合法参考包。
- TLS：`rustls`、`tokio-rustls`、`hyper-rustls` — 规范的现代 Rust TLS 堆栈。
- 噪声：`snow` — 规范实现。
- 序列化：`parity-scale-codec` 是 SCALE 的规范。 Serde 已从整个工作区的生产依赖关系中删除； Norito 派生/编写器涵盖每个运行时路径。任何残留的 Serde 引用都存在于历史文档、护栏脚本或仅限测试的白名单中。
- FFI/库：`libsodium-sys-stable`、`openssl` — 合法；在生产路径中更喜欢 Rustls 而不是 OpenSSL（当前代码已经这样做了）。

建议：
-解决警告：
  - 考虑用 `retry`/`futures-retry` 或本地指数退避助手替换 `backoff`。
  - 将派生的 `derivative` 替换为手动实现或 `derive_more`（如果适用）。
- 中：尽可能统一 `k256` 或 `secp256k1`，以减少重复实现（仅在真正需要时才保留两者）。
- Medium：查看 `poseidon-primitives 0.2.0` 的出处以了解 ZK 的使用情况；如果可行，请考虑与 Arkworks/Halo2 原生 Poseidon 实现保持一致，以尽量减少并行生态系统。

注意事项：
- `cargo tree -d` 显示预期的重复主要版本（`bitflags` 1/2、多个 `ring`），其本身不会带来安全风险，但会增加构建表面。
- 没有观察到类似拼写错误的板条箱；所有名称和来源都解析为知名的生态系统板条​​箱或内部工作区成员。
- 实验性：添加了 `iroha_crypto` 功能 `bls-backend-blstrs`，以开始将 BLS 迁移到仅 blstrs 的后端（启用后消除对 arkworks 的依赖）。默认保留 `w3f-bls` 以避免行为/编码更改。对齐计划：
  - 在 `crates/iroha_crypto/tests/bls_backend_compat.rs` 中添加往返固定装置，一次导出密钥并在两个后端之间断言相等，涵盖 `SecretKey`、`PublicKey` 和签名聚合。

后续行动（建议的工作项目）：
- 将 Serde 护栏保留在 CI 中（`scripts/check_no_direct_serde.sh`、`scripts/deny_serde_json.sh`），以便无法引入新的生产用途。

为此次审核进行的测试：
- 使用最新的咨询数据库运行 `cargo audit`；验证了四个建议及其依赖关系树。
- 搜索受影响板条箱的直接依赖声明以查明修复位置。