---
lang: zh-hans
direction: ltr
source: docs/source/crypto/sm_rustcrypto_spike.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1f133d9489c4bcfae2212e6c5dc098f39c3dea3e5cd42855ba76e8c9b73b4d03
source_last_modified: "2025-12-29T18:16:35.946614+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//！ RustCrypto SM 集成尖峰的注释。

# RustCrypto SM 尖峰笔记

## 目标
验证引入 RustCrypto 的 `sm2`、`sm3` 和 `sm4` 包（加上 `rfc6979`、`ccm`、`gcm`）作为可选依赖项可以在`iroha_crypto` crate 并在将功能标志连接到更广泛的工作空间之前产生可接受的构建时间。

## 建议的依赖关系图

|板条箱 |建议版本 |特点|笔记|
|--------|--------------------|----------|--------|
| `sm2` | `0.13`（RustCrypto/签名）| `std` |取决于 `elliptic-curve`；验证 MSRV 与工作区匹配。 |
| `sm3` | `0.5.0-rc.1`（RustCrypto/哈希）|默认 | API 与 `sha2` 并行，与现有的 `digest` 特征集成。 |
| `sm4` | `0.5.1`（RustCrypto/块密码）|默认 |适用于密码特征； AEAD 包装推迟到稍后的峰值。 |
| `rfc6979` | `0.4` |默认 |重用于确定性随机数推导。 |

*版本反映截至 2024 年 12 月的当前版本；着陆前与 `cargo search` 确认。*

## 明显变更（草案）

```toml
[features]
sm = ["dep:sm2", "dep:sm3", "dep:sm4", "dep:rfc6979"]

[dependencies]
sm2 = { version = "0.13", optional = true, default-features = false, features = ["std"] }
sm3 = { version = "0.5.0-rc.1", optional = true }
sm4 = { version = "0.5.1", optional = true }
rfc6979 = { version = "0.4", optional = true, default-features = false }
```

后续：引脚 `elliptic-curve` 以匹配 `iroha_crypto` 中已有的版本（当前为 `0.13.8`）。

## 尖峰清单
- [x] 向 `crates/iroha_crypto/Cargo.toml` 添加可选依赖项和功能。
- [x] 在 `cfg(feature = "sm")` 后面创建 `signature::sm` 模块，并使用占位符结构来确认接线。
- [x] 运行 `cargo check -p iroha_crypto --features sm` 确认编译；记录构建时间和新的依赖项计数 (`cargo tree --features sm`)。
- [x] 使用 `cargo check -p iroha_crypto --features sm --locked` 确认仅标准姿势；不再支持 `no_std` 版本。
- [x] `docs/source/crypto/sm_program.md` 中的文件结果（计时、依赖树增量）。

## 捕捉观察结果
- 与基线相比的额外编译时间。
- `cargo builtinsize` 的二进制大小影响（如果可测量）。
- 任何 MSRV 或功能冲突（例如，与 `elliptic-curve` 次要版本）。
- 发出的警告（不安全代码、const-fn 门控）可能需要上游补丁。

## 待处理项目
- 在膨胀工作区依赖关系图之前等待加密工作组批准。
- 确认是否向供应商 crate 进行审查或依赖 crates.io（可能需要镜像）。
- 在标记清单完成之前，根据 `sm_lock_refresh_plan.md` 协调 `Cargo.lock` 刷新。
- 一旦获得批准即可使用 `scripts/sm_lock_refresh.sh` 来重新生成锁定文件和依赖关系树。

## 2025-01-19 尖峰日志
- 在 `iroha_crypto` 中添加了可选依赖项（`sm2 0.13`、`sm3 0.5.0-rc.1`、`sm4 0.5.1`、`rfc6979 0.4`）和 `sm` 功能标志。
- 存根 `signature::sm` 模块以在编译期间执行哈希/分组密码 API。
- `cargo check -p iroha_crypto --features sm --locked` 现在可以解析依赖关系图，但会因 `Cargo.lock` 更新要求而中止；存储库策略禁止锁定文件编辑，因此编译运行将保持挂起状态，直到我们协调允许的锁定刷新。## 2026-02-12 尖峰日志
- 解决了之前的锁文件阻止程序 - 依赖关系已被捕获 - 因此 `cargo check -p iroha_crypto --features sm --locked` 成功（在开发 Mac 上冷构建 7.9 秒；增量重新运行 0.23 秒）。
- `cargo check -p iroha_crypto --no-default-features --features "std sm" --locked` 在 1.0 秒内通过，确认可选功能在仅 `std` 配置中编译（不保留 `no_std` 路径）。
- 启用 `sm` 功能的依赖项增量引入了 11 个 crate：`base64ct`、`ghash`、`opaque-debug`、`pem-rfc7468`、`pkcs8`、`polyval`、 `primeorder`、`sm2`、`sm3`、`sm4` 和 `sm4-gcm`。 （`rfc6979` 已经是基线图的一部分。）
- 对于未使用的 NEON 策略助手，构建警告仍然存在；保持原样，直到计量平滑运行时重新启用这些代码路径。