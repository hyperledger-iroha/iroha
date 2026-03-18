---
lang: zh-hans
direction: ltr
source: docs/source/crypto/dependency_audits.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 04e4cf26ed0ce9f9782be8aae9d16425a7a87fdbd1986cbcbca68a27ba0a3afe
source_last_modified: "2025-12-29T18:16:35.939138+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 加密依赖审计

## Streebog（`streebog` 箱子）

- **树中的版本：** `0.11.0-rc.2` 在 `vendor/streebog` 下销售（启用 `gost` 功能时使用）。
- **消费者：** `crates/iroha_crypto::signature::gost`（HMAC-Streebog DRBG + 消息哈希）。
- **状态：** 仅限发布候选版本。目前没有非 RC 板条箱提供所需的 API 表面，
  因此，我们在树内镜像板条箱以实现可审计性，同时跟踪上游的最终版本。
- **审查检查点：**
  - 通过 Wycheproof 套件和 TC26 装置验证哈希输出
    `cargo test -p iroha_crypto --features gost`（参见 `crates/iroha_crypto/tests/gost_wycheproof.rs`）。
  - `cargo bench -p iroha_crypto --bench gost_sign --features gost`
    与电流相关性的每条 TC26 曲线一起练习 Ed25519/Secp256k1。
  - `cargo run -p iroha_crypto --bin gost_perf_check --features gost`
    将较新的测量值与签入的中位数进行比较（在 CI 中使用 `--summary-only`，添加
    重新设定基线时为 `--write-baseline crates/iroha_crypto/benches/gost_perf_baseline.json`）。
  - `scripts/gost_bench.sh`包裹工作台+检查流程；传递 `--write-baseline` 来更新 JSON。
    有关端到端工作流程，请参阅 `docs/source/crypto/gost_performance.md`。
- **缓解措施：** `streebog` 仅通过将密钥归零的确定性包装器调用；
  签名者用操作系统熵对冲随机数，以避免灾难性的 RNG 失败。
- **下一步行动：** 关注 RustCrypto 的 streebog `0.11.x` 版本；一旦标签落地，请处理
  升级为标准依赖项碰撞（验证校验和、检查差异、记录出处以及
  放下出售的镜子）。