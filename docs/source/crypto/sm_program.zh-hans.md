---
lang: zh-hans
direction: ltr
source: docs/source/crypto/sm_program.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 08e2e1e4a54390d9142d6788aad2385e93282a33423b9fc7f3418e3633f3f86a
source_last_modified: "2026-01-23T23:46:10.134857+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//！ Hyperledger Iroha v2 的 SM2/SM3/SM4 支持架构简介。

# SM 程序架构简介

## 目的
定义在 Iroha v2 堆栈中引入中国国家密码 (SM2/SM3/SM4) 的技术计划、供应链态势和风险边界，同时保持确定性执行和可审计性。

## 范围
- **共识关键路径：** `iroha_crypto`、`iroha`、`irohad`、IVM 主机、Kotodama 内在函数。
- **客户端 SDK 和工具：** Rust CLI、Kagami、Python/JS/Swift SDK、创世实用程序。
- **配置和序列化：** `iroha_config` 旋钮、Norito 数据模型标签、清单处理、多编解码器更新。
- **测试与合规性：** 单元/属性/互操作套件、Wycheproof 安全带、性能分析、出口/监管指南。 *（状态：RustCrypto 支持的 SM 堆栈合并；可选的 `sm_proptest` 模糊套件和 OpenSSL 奇偶校验工具可用于扩展 CI。）*

超出范围：PQ 算法、共识路径中的非确定性主机加速； wasm/`no_std` 版本已退役。

## 算法输入和可交付成果
|神器|业主|到期|笔记|
|----------|--------|-----|--------|
| SM算法特征设计(`SM-P0`) |加密工作组 | 2025年02月 |功能门控、依赖性审计、风险登记。 |
|核心 Rust 集成 (`SM-P1`) |加密工作组/数据模型 | 2025 年 3 月 |基于 RustCrypto 的验证/哈希/AEAD 帮助程序、Norito 扩展、固定装置。 |
|签名 + VM 系统调用 (`SM-P2`) | IVM 核心/SDK 程序 | 2025 年 4 月 |确定性签名包装器、系统调用、Kotodama 覆盖范围。 |
|可选的提供商和运营支持 (`SM-P3`) |平台运营/性能工作组 | 2025 年 6 月 | OpenSSL/Tongsuo 后端、ARM 内在函数、遥测、文档。 |

## 选定的库
- **主要：** RustCrypto 箱（`sm2`、`sm3`、`sm4`），启用了 `rfc6979` 功能并且 SM3 绑定到确定性随机数。
- **可选 FFI：** OpenSSL 3.x 提供商 API 或 Tongsuo，用于需要经过认证的堆栈或硬件引擎的部署；在共识二进制文件中默认情况下具有功能门控和禁用功能。### 核心库集成状态
- `iroha_crypto::sm` 在统一的 `sm` 功能下公开 SM3 哈希、SM2 验证和 SM4 GCM/CCM 帮助程序，并通过 SDK 提供确定性 RFC6979 签名路径`Sm2PrivateKey`.【crates/iroha_crypto/src/sm.rs:1049】【crates/iroha_crypto/src/sm.rs:1128】【crates/iroha_crypto/src/sm.rs:1236】
- Norito/Norito-JSON 标签和多编解码器帮助程序涵盖 SM2 公钥/签名和 SM3/SM4 有效负载，因此指令可以确定性地序列化主机.【crates/iroha_data_model/src/isi/registry.rs:407】【crates/iroha_data_model/tests/sm_norito_roundtrip.rs:12】
- 已知答案套件验证 RustCrypto 集成（`sm3_sm4_vectors.rs`、`sm2_negative_vectors.rs`）并作为 CI 的 `sm` 功能作业的一部分运行，在节点继续签名时保持验证确定性Ed25519.【板条箱/iroha_crypto/tests/sm3_sm4_vectors.rs:15】【板条箱/iroha_crypto/tests/sm2_male_vectors.rs:1】
- 可选的 `sm` 功能构建验证：`cargo check -p iroha_crypto --features sm --locked`（冷 7.9 秒/暖 0.23 秒）和 `cargo check -p iroha_crypto --no-default-features --features "std sm" --locked`（1.0 秒）均成功；启用该功能会添加 11 个箱子（`base64ct`、`ghash`、`opaque-debug`、`pem-rfc7468`、`pkcs8`、`polyval`、`primeorder`、 `sm2`、`sm3`、`sm4`、`sm4-gcm`）。调查结果记录在 `docs/source/crypto/sm_rustcrypto_spike.md` 中。【docs/source/crypto/sm_rustcrypto_spike.md:1】
- BouncyCastle/GmSSL 否定验证装置位于 `crates/iroha_crypto/tests/fixtures/sm/sm2_negative_vectors.json` 下，确保规范故障案例（r=0、s=0、区分 ID 不匹配、公钥被篡改）与广泛部署保持一致提供者.【crates/iroha_crypto/tests/sm2_male_vectors.rs:1】【crates/iroha_crypto/tests/fixtures/sm/sm2_male_vectors.json:1】
- `sm-ffi-openssl` 现在编译供应商的 OpenSSL 3.x 工具链（`openssl` crate `vendored` 功能），因此即使系统 LibreSSL/OpenSSL 缺乏 SM 算法，预览构建和测试也始终以支持现代 SM 的提供程序为目标。【crates/iroha_crypto/Cargo.toml:59】
- `sm_accel` 现在在运行时检测 AArch64 NEON，并通过 x86_64/RISC-V 调度将 SM3/SM4 挂钩线程化，同时遵循配置旋钮 `crypto.sm_intrinsics` (`auto`/`force-enable`/`force-disable`)。当向量后端不存在时，调度程序仍然通过标量 RustCrypto 路径进行路由，因此工作台和策略切换在主机之间的行为一致。【crates/iroha_crypto/src/sm.rs:702】【crates/iroha_crypto/src/sm.rs:733】

### Norito 架构和数据模型表面| Norito 类型/消费者 |代表|限制和注意事项|
|------------------------------------|----------------|---------------------|
| `Sm3Digest` (`iroha_crypto::Sm3Digest`) |裸露：32 字节 blob · JSON：大写十六进制字符串 (`"4F4D..."`) |规范 Norito 元组包装 `[u8; 32]`。 JSON/Bare 解码拒绝长度 ≠32。`sm_norito_roundtrip::sm3_digest_norito_roundtrip` 涵盖的往返行程。 |
| `Sm2PublicKey` / `Sm2Signature` |多编解码器前缀 blob（`0x1306` 临时）|公钥对未压缩的 SEC1 点进行编码；签名为 `(r∥s)`（每个 32 字节），带有 DER 解析防护。 |
| `Sm4Key` |裸露：16 字节 blob |归零暴露于 Kotodama/CLI 的包装器。故意省略了 JSON 序列化；操作员应通过 blob（合约）或 CLI 十六进制 (`--key-hex`) 传递密钥。 |
| `sm4_gcm_seal/open` 操作数 | 4 个 blob 元组：`(key, nonce, aad, payload)` |密钥 = 16 字节；随机数 = 12 字节；标签长度固定为 16 字节。返回 `(ciphertext, tag)`； Kotodama/CLI 发出十六进制和 Base64 帮助程序。【crates/ivm/tests/sm_syscalls.rs:728】 |
| `sm4_ccm_seal/open` 操作数 | `r14` 中 4 个 blob 的元组（密钥、随机数、aad、有效负载）+ 标签长度 |随机数 7–13 字节；标签长度 ∈ {4,6,8,10,12,14,16}。 `sm` 功能暴露了 `sm-ccm` 标志后面的 CCM。 |
| Kotodama 内在函数（`sm::hash`、`sm::seal_gcm`、`sm::open_gcm`，...）映射到上面的 SCALL |输入验证反映主机规则；格式错误的大小会引发 `ExecutionError::Type`。 |

使用快速参考：
- **合约/测试中的 SM3 哈希：** `Sm3Digest::hash(b"...")` (Rust) 或 Kotodama `sm::hash(input_blob)`。 JSON 需要 64 个十六进制字符。
- **SM4 AEAD 通过 CLI:** `iroha tools crypto sm4 gcm-seal --key-hex <32 hex> --nonce-hex <24 hex> --plaintext-hex …` 生成十六进制/base64 密文+标签对。使用匹配的 `gcm-open` 进行解密。
- **多编解码器字符串：** SM2 公钥/签名从 `PublicKey::from_str`/`Signature::from_bytes` 接受的多基字符串解析，使 Norito 清单和帐户 ID 能够携带 SM 签名者。

数据模型使用者应将 SM4 键和标签视为瞬态 blob；切勿将原始密钥保留在链上。当需要审计时，合约应仅存储密文/标签输出或派生摘要（例如密钥的 SM3）。

### 供应链和许可
|组件|许可证|缓解措施 |
|------------|---------|------------|
| `sm2`、`sm3`、`sm4` | Apache-2.0 / 麻省理工学院 |跟踪上游提交、供应商（如果需要同步发布）、在验证者签署 GA 之前安排第三方审核。 |
| `rfc6979` | Apache-2.0 / 麻省理工学院 |已经在其他算法中使用；确认 `k` 与 SM3 摘要的确定性结合。 |
|可选OpenSSL/通锁 | Apache-2.0 / BSD 风格 |保留 `sm-ffi-openssl` 功能，需要明确的操作员选择加入和打包清单。 |### 功能标志和所有权
|表面|默认|维护者 |笔记|
|--------|---------|------------|--------|
| `iroha_crypto/sm-core`、`sm-ccm`、`sm` |关闭 |加密工作组 |启用 RustCrypto SM 原语； `sm` 为需要经过身份验证的加密的客户端捆绑了 CCM 帮助程序。 |
| `ivm/sm` |关闭 | IVM 核心团队 |构建 SM 系统调用（`sm3_hash`、`sm2_verify`、`sm4_gcm_*`、`sm4_ccm_*`）。主机门控源自 `crypto.allowed_signing`（存在 `sm2`）。 |
| `iroha_crypto/sm_proptest` |关闭 |质量保证/加密工作组 |属性测试工具涵盖格式错误的签名/标签。仅在扩展 CI 中启用。 |
| `crypto.allowed_signing` + `default_hash` | `["ed25519"]`，`blake2b-256` |配置工作组/操作员工作组 | `sm2` 加上 `sm3-256` 散列的存在可启用 SM 系统调用/签名；删除 `sm2` 将返回仅验证模式。 |
|可选 `sm-ffi-openssl`（预览）|关闭 |平台运营 | OpenSSL/Tongsuo 提供商集成的占位符功能；在认证和包装 SOP 落地之前保持禁用状态。 |

网络策略现在公开 `network.require_sm_handshake_match` 和
`network.require_sm_openssl_preview_match`（均默认为 `true`）。清除任一标志允许
混合部署，其中仅 Ed25519 的观察者连接到支持 SM 的验证器；不匹配的是
记录在`WARN`，但共识节点应保持默认启用，以防止意外
SM 感知和 SM 禁用对等点之间的差异。
CLI 通过“iroha_cli app sorafs 握手更新”显示这些切换
--allow-sm-handshake-mismatch` and `--allow-sm-openssl-preview-mismatch`, or the matching `--require-*`
旗帜恢复严格执行。#### OpenSSL/通所预览(`sm-ffi-openssl`)
- **范围。** 构建仅预览的提供程序垫片 (`OpenSslProvider`)，用于验证 OpenSSL 运行时可用性并公开 OpenSSL 支持的 SM3 哈希、SM2 验证和 SM4-GCM 加密/解密，同时保持选择加入。共识二进制文件必须继续使用 RustCrypto 路径； FFI 后端严格选择加入边缘验证/签名试点。
- **构建先决条件。** 使用 `cargo build -p iroha_crypto --features "sm sm-ffi-openssl"` 进行编译，并确保工具链链接到 OpenSSL/Tongsuo 3.0+（支持 SM2/SM3/SM4 的 `libcrypto`）。不鼓励静态链接；更喜欢由操作员管理的动态库。
- **开发人员冒烟测试。** 运行 `scripts/sm_openssl_smoke.sh` 来执行 `cargo check -p iroha_crypto --features "sm sm-ffi-openssl"`，然后执行 `cargo test -p iroha_crypto --features "sm sm-ffi-openssl" --test sm_openssl_smoke -- --nocapture`；当 OpenSSL ≥3 开发标头不可用（或缺少 `pkg-config`）时，助手会自动跳过并显示烟雾输出，以便开发人员可以查看 SM2 验证是运行还是退回到 Rust 实现。
- **Rust 脚手架。** `openssl_sm` 模块现在通过 OpenSSL 路由 SM3 哈希、SM2 验证（ZA 预哈希 + SM2 ECDSA）和 SM4 GCM 加密/解密，并产生涵盖预览切换和无效密钥/随机数/标签长度的结构化错误； SM4 CCM 保持纯 Rust 状态，直到额外的 FFI 垫片落地。
- **跳过行为。** 当 OpenSSL ≥3.0 标头或库不存在时，烟雾测试会打印一个跳过横幅（通过 `-- --nocapture`），但仍然成功退出，因此 CI 可以区分环境差距和真正的回归。
- **运行时护栏。** 默认情况下禁用 OpenSSL 预览；在尝试使用 FFI 路径之前，通过配置 (`crypto.enable_sm_openssl_preview` / `OpenSslProvider::set_preview_enabled(true)`) 启用它。将生产集群保持在仅验证模式（从 `allowed_signing` 中省略 `sm2`），直到提供商毕业，依赖确定性 RustCrypto 回退，并将签名试点限制在隔离环境中。
- **打包清单。** 在部署清单中记录提供程序版本、安装路径和完整性哈希值。运营商必须提供安装脚本来安装已批准的 OpenSSL/Tongsuo 版本，将其注册到操作系统信任存储（如果需要），并在维护时段后进行固定升级。
- **后续步骤。** 未来的里程碑添加确定性 SM4 CCM FFI 绑定、CI 烟雾作业（请参阅 `ci/check_sm_openssl_stub.sh`）和遥测。跟踪 `roadmap.md` 中 SM-P3.1.x 下的进度。

#### 代码所有权快照
- **加密工作组：** `iroha_crypto`、SM 装置、合规性文档。
- **IVM 核心：** 系统调用实现、Kotodama 内在函数、主机门控。
- **配置工作组：** `crypto.allowed_signing`/`default_hash`，最新版本，最新版本。
- **SD​​K 程序：** 跨 CLI/Kagami/SDK 的 SM 感知工具、共享夹具。
- **平台运营和性能工作组：**加速挂钩、遥测、操作员支持。

## 配置迁移手册从仅使用 Ed25519 的网络转向支持 SM 的部署的运营商应该
遵循分阶段流程
[`sm_config_migration.md`](sm_config_migration.md)。该指南涵盖构建
验证、`iroha_config` 分层（`defaults` → `user` → `actual`）、创世
通过 `kagami` 覆盖（例如 `kagami genesis generate --allowed-signing sm2 --default-hash sm3-256`）、飞行前验证和回滚进行重新生成
进行规划，以便配置快照和清单在整个系统中保持一致
舰队。

## 确定性策略
- 对 SDK 和可选主机签名中的所有 SM2 签名路径强制执行 RFC6979 派生的随机数；验证者仅接受规范的 r∥s 编码。
- 控制平面通信（流）仍然是 Ed25519； SM2 仅限于数据平面签名，除非治理批准扩展。
- 内在函数 (ARM SM3/SM4) 仅限于具有运行时功能检测和软件回退的确定性验证/哈希操作。

## Norito & 编码计划
1. 使用 `Sm2PublicKey`、`Sm2Signature`、`Sm3Digest`、`Sm4Key` 扩展 `iroha_data_model` 中的算法枚举。
2、将SM2签名序列化为大端定宽`r∥s`数组（32+32字节），以避免DER歧义；适配器中处理的转换。 *（完成：在 `Sm2Signature` 帮助程序中实现；Norito/JSON 往返到位。）*
3. 如果使用多格式，请注册多编解码器标识符（`sm3-256`、`sm2-pub`、`sm4-key`）、更新装置和文档。 *（进展：`sm2-pub` 临时代码 `0x1306` 现已使用派生密钥进行验证；SM3/SM4 代码等待最终分配，通过 `sm_known_answers.toml` 进行跟踪。）*
4. 更新 Norito 黄金测试，涵盖往返和拒绝格式错误的编码（短/长 r 或 s、无效曲线参数）。## 主机和虚拟机集成计划 (SM-2)
1. 实现主机端 `sm3_hash` 系统调用镜像现有的 GOST 哈希 shim；重用 `Sm3Digest::hash` 并公开确定性错误路径。 *（已着陆：主机返回 Blob TLV；请参阅 `DefaultHost` 实现和 `sm_syscalls.rs` 回归。）*
2. 使用 `sm2_verify` 扩展 VM 系统调用表，该表接受规范的 r∥s 签名、验证区分 ID 并将故障映射到确定性返回代码。 *（完成：主机 + Kotodama 内在函数返回 `1/0`；回归套件现在涵盖截断的签名、格式错误的公钥、非 blob TLV 和 UTF-8/空/不匹配的 `distid` 有效负载。）*
3. 为 `sm4_gcm_seal`/`sm4_gcm_open`（以及可选的 CCM）系统调用提供显式随机数/标签大小调整 (RFC 8998)。 *（完成：GCM 使用固定的 12 字节随机数 + 16 字节标签；CCM 支持 7-13 字节随机数，标签长度为 {4,6,8,10,12,14,16}，通过 `r14` 控制；Kotodama 将这些公开为 `sm::seal_gcm/open_gcm` 和`sm::seal_ccm/open_ccm`。）在开发人员手册中记录随机数重用策略。*
4. 连线 Kotodama 烟雾合约和 IVM 集成测试，涵盖正面和负面案例（标签更改、签名格式错误、算法不受支持）。 *（通过 SM3/SM2/SM4 的 `crates/ivm/tests/kotodama_sm_syscalls.rs` 镜像主机回归完成。）*
5. 更新系统调用允许列表、策略和 ABI 文档 (`crates/ivm/docs/syscalls.md`)，并在添加新条目后刷新哈希清单。

### 主机和虚拟机集成状态
- DefaultHost、CoreHost 和 WsvHost 公开 SM3/SM2/SM4 系统调用并在 `sm_enabled` 上对它们进行门控，当运行时标志为时返回 `PermissionDenied` false.【crates/ivm/src/host.rs:915】【crates/ivm/src/core_host.rs:833】【crates/ivm/src/mock_wsv.rs:2307】
- `crypto.allowed_signing` 门控通过管道/执行器/状态进行线程化，因此生产节点通过配置确定性地选择加入；添加 `sm2` 切换 SM 助手可用性。`【crates/iroha_core/src/smartcontracts/ivm/host.rs:170】【crates/iroha_core/src/state.rs:7673】【crates/iroha_core/src/executor.rs:683】
- 回归覆盖测试 SM3 哈希、SM2 验证和 SM4 GCM/CCM 密封/开放的启用和禁用路径（DefaultHost/CoreHost/WsvHost） 【crates/ivm/tests/sm_syscalls.rs:129】【crates/ivm/tests/sm_syscalls.rs:733】【crates/ivm/tests/sm_syscalls.rs:1036】

## 配置线程
- 将 `crypto.allowed_signing`、`crypto.default_hash`、`crypto.sm2_distid_default` 和可选的 `crypto.enable_sm_openssl_preview` 添加到 `iroha_config`。确保数据模型功能管道镜像加密箱（`iroha_data_model` 公开 `sm` → `iroha_crypto/sm`）。
- 将配置连接到准入策略，以便清单/创世文件定义允许的算法；控制平面默认保留 Ed25519。### CLI 和 SDK 工作 (SM-3)
1. **Torii CLI** (`crates/iroha_cli`)：添加 SM2 keygen/导入/导出（distid 感知）、SM3 哈希帮助程序和 SM4 AEAD 加密/解密命令。更新交互式提示和文档。
2. **Genesis 工具**（`xtask`、`scripts/`）：允许清单声明允许的签名算法和默认哈希值，如果在没有相应配置旋钮的情况下启用 SM，则会快速失败。 *（完成：`RawGenesisTransaction`现在带有一个`crypto`块，其中包含`default_hash`/`allowed_signing`/`sm2_distid_default`；`ManifestCrypto::validate`和`kagami genesis validate`拒绝不一致的SM设置和默认/起源广告快照。）*
3. **SDK 界面**：
   - Rust (`iroha_client`)：公开具有确定性默认值的 SM2 签名/验证助手、SM3 哈希、SM4 AEAD 包装器。
   - Python/JS/Swift：镜像 Rust API；重用 `sm_known_answers.toml` 中的分段装置进行跨语言测试。
4. 记录在 CLI/SDK 快速入门中启用 SM 的操作员工作流程，并确保 JSON/YAML 配置接受新的算法标签。

#### CLI 进度
- `cargo run -p iroha_cli --features sm -- crypto sm2 keygen --distid CN12345678901234` 现在发出描述 SM2 密钥对的 JSON 有效负载以及 `client.toml` 片段（`public_key_config`、`private_key_hex`、`distid`）。该命令接受 `--seed-hex` 进行确定性生成，并镜像主机使用的 RFC 6979 派生。
- `cargo xtask sm-operator-snippet --distid CN12345678901234` 包装了注册机/导出流程，一步写入相同的 `sm2-key.json`/`client-sm2.toml` 输出。使用 `--json-out <path|->` / `--snippet-out <path|->` 重定向文件或将其流式传输到标准输出，从而删除 `jq` 的自动化依赖性。
- `iroha_cli tools crypto sm2 import --private-key-hex <hex> [--distid ...]` 从现有材料中导出相同的元数据，以便操作员可以在准入之前验证区分 ID。
- `iroha_cli tools crypto sm2 export --private-key-hex <hex> --emit-json` 打印配置片段（包括 `allowed_signing`/`sm2_distid_default` 指南），并可选择重新发出 JSON 密钥清单以进行脚本编写。
- `iroha_cli tools crypto sm3 hash --data <string>` 散列任意有效负载； `--data-hex` / `--file` 涵盖二进制输入，并且该命令报告清单工具的十六进制和 Base64 摘要。
- `iroha_cli tools crypto sm4 gcm-seal --key-hex <KEY> --nonce-hex <NONCE> --plaintext-hex <PT>`（和 `gcm-open`）包装主机 SM4-GCM 帮助程序和表面 `ciphertext_hex`/`tag_hex` 或明文有效负载。 `sm4 ccm-seal` / `sm4 ccm-open` 为 CCM 提供相同的 UX，内置随机数长度（7-13 字节）和标签长度（4,6,8,10,12,14,16）验证；这两个命令都可以选择将原始字节发送到磁盘。## 测试策略
### 单元/已知答案测试
- SM3 的 GM/T 0004 和 GB/T 32905 矢量（例如 `"abc"`）。
- SM4 的 GM/T 0002 和 RFC 8998 矢量（块 + GCM/CCM）。
- GM/T 0003/GB/T 32918 SM2 示例（Z 值、签名验证），包括 ID 为 `ALICE123@YAHOO.COM` 的附件示例 1。
- 临时夹具暂存文件：`crates/iroha_crypto/tests/fixtures/sm_known_answers.toml`。
- Wycheproof 衍生的 SM2 回归套件 (`crates/iroha_crypto/tests/sm2_wycheproof.rs`) 现在带有 52 个案例的语料库，该语料库将确定性装置（附件 D、SDK 种子）与位翻转、消息篡改和截断签名负数分层。清理后的 JSON 存在于 `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json` 中，`sm2_fuzz.rs` 直接使用它，因此快乐路径和篡改场景在模糊/属性运行中保持一致。附件 附件 附件 附件 附件 附件 附件 附件请注意以下事项。
- `cargo xtask sm-wycheproof-sync --input <wycheproof-sm2.json>`（或 `--input-url <https://…>`）确定性地修剪任何上游丢弃（生成器标签可选）并重写 `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json`。在C2SP发布官方语料库之前，手动下载fork并通过helper喂它们；它标准化键、计数和标志，以便审阅者可以对差异进行推理。
- SM2/SM3 Norito 往返在 `crates/iroha_data_model/tests/sm_norito_roundtrip.rs` 中验证。
- `crates/ivm/tests/sm_syscalls.rs` 中的 SM3 主机系统调用回归（SM 功能）。
- SM2 验证 `crates/ivm/tests/sm_syscalls.rs` 中的系统调用回归（成功 + 失败案例）。

### 属性和回归测试
- SM2 的 Proptest 拒绝无效曲线、非规范 r/s 和随机数重用。 *（在 `crates/iroha_crypto/tests/sm2_fuzz.rs` 中可用，在 `sm_proptest` 后面门控；通过 `cargo test -p iroha_crypto --features "sm sm_proptest"` 启用。）*
- Wycheproof SM4 矢量（块/AES 模式）适用于各种模式；跟踪上游 SM2 添加。 `sm3_sm4_vectors.rs` 现在针对 GCM 和 CCM 执行标签位翻转、截断标签和密文篡改。

### 互操作和性能
- 用于 SM2 签名/验证、SM3 摘要和 SM4 ECB/GCM 的 RustCrypto ↔ OpenSSL/Tongsuo 奇偶校验套件位于 `crates/iroha_crypto/tests/sm_cli_matrix.rs` 中；使用 `scripts/sm_interop_matrix.sh` 调用它。 CCM 奇偶校验向量现在在 `sm3_sm4_vectors.rs` 中运行；一旦上游 CLI 公开 CCM 帮助程序，CLI 矩阵支持就会随之而来。
- SM3 NEON 帮助程序现在通过 `sm_accel::is_sm3_enabled` 运行时端到端运行 Armv8 压缩/填充路径（功能 + env 覆盖在 SM3/SM4 上镜像）。黄金摘要（零/`"abc"`/长块 + 随机长度）和强制禁用测试与标量 RustCrypto 后端保持同等，并且 Criterion 微基准 (`crates/sm3_neon/benches/digest.rs`) 捕获 AArch64 主机上的标量与 NEON 吞吐量。
- Perf 线束镜像 `scripts/gost_bench.sh`，用于比较 Ed25519/SHA-2 与 SM2/SM3/SM4 并验证容差阈值。#### Arm64 基准（本地 Apple 芯片；标准 `sm_perf`，2025 年 12 月 5 日更新）
- `scripts/sm_perf.sh` 现在运行 Criterion 基准，并针对 `crates/iroha_crypto/benches/sm_perf_baseline.json` 强制执行中位数（在 aarch64 macOS 上记录；默认情况下容差 25%，基线元数据捕获主机三重）。新的 `--mode` 标志使工程师可以捕获标量、NEON 与 `sm-neon-force` 数据点，而无需编辑脚本；当前捕获包（原始 JSON + 聚合摘要）位于 `artifacts/sm_perf/2026-03-lab/m3pro_native/` 下，并使用 `cpu_label="m3-pro-native"` 标记每个有效负载。
- 加速模式现在自动选择标量基线作为比较目标。 `scripts/sm_perf.sh` 通过 `sm_perf_check` 线程 `--compare-baseline/--compare-tolerance/--compare-label`，针对标量参考发出每个基准增量，并在减速超过配置的阈值时失败。基线的每个基准公差驱动比较保护（SM3 在 Apple 标量基线上的上限为 12%，而 SM3 比较增量现在允许与标量参考相比高达 70%，以避免波动）； Linux 基线重复使用相同的比较图，因为它们是从 `neoverse-proxy-macos` 捕获中导出的，如果中位数不同，我们将在裸机 Neoverse 运行后收紧它们。当捕获更严格的边界（例如，`--compare-tolerance 0.20`）时显式传递 `--compare-tolerance` 并使用 `--compare-label` 来注释替代参考主机。
- CI 参考机上记录的基线现在位于 `crates/iroha_crypto/benches/sm_perf_baseline_aarch64_macos_scalar.json`、`sm_perf_baseline_aarch64_macos_auto.json` 和 `sm_perf_baseline_aarch64_macos_neon_force.json` 中。使用 `scripts/sm_perf.sh --mode scalar --write-baseline`、`--mode auto --write-baseline` 或 `--mode neon-force --write-baseline`（在捕获之前设置 `SM_PERF_CPU_LABEL`）刷新它们，并将生成的 JSON 与运行日志一起存档。将聚合的帮助器输出 (`artifacts/.../aggregated.json`) 与 PR 一起保留，以便审核者可以审核每个样本。 Linux/Neoverse 基线现在以 `sm_perf_baseline_aarch64_unknown_linux_gnu_{mode}.json` 发布，从 `artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/aggregated.json` 升级（CPU 标签 `neoverse-proxy-macos`，aarch64 macOS/Linux 的 SM3 比较容差 0.70）；当可以收紧容差时，在裸机 Neoverse 主机上重新运行。
- 基线 JSON 文件现在可以携带可选的 `tolerances` 对象，以加强每个基准的护栏。示例：
  ```json
  {
    "benchmarks": { "...": 12.34 },
    "tolerances": {
      "sm4_vs_chacha20poly1305_encrypt/sm4_gcm_encrypt": 0.08,
      "sm3_vs_sha256_hash/sm3_hash": 0.12
    }
  }
  ```
  `sm_perf_check` 应用这些分数限制（示例中为 8% 和 12%），同时对未列出的任何基准使用全局 CLI 容差。
- 比较防护还可以在比较基线中遵守 `compare_tolerances`。使用此选项可以允许针对标量参考的更宽松的增量（例如，标量基线中的 `\"sm3_vs_sha256_hash/sm3_hash\": 0.70`），同时保持主 `tolerances` 严格以进行直接基线检查。- 签入的 Apple Silicon 基线现在配备了混凝土护栏：SM2/SM4 操作允许根据方差有 12-20% 的漂移，而 SM3/ChaCha 比较则为 8-12%。标量基线的 `sm3` 容差现已收紧至 0.12； `unknown_linux_gnu` 文件镜像 `neoverse-proxy-macos` 导出，具有相同的容差图（SM3 比较 0.70）和元数据注释，表明它们是为 Linux gateway 提供的，直到可以重新运行裸机 Neoverse。
- SM2 签名：每次操作 298μs（Ed25519：32μs）⇒ 慢约 9.2 倍；验证：267μs（Ed25519：41μs）⇒〜6.5×慢。
- SM3 散列（4KiB 有效负载）：11.2μs，与 11.3μs 的 SHA-256 有效奇偶校验（约 356MiB/s 与 353MiB/s）。
- SM4-GCM 密封/打开（1KiB 有效负载，12 字节随机数）：15.5μs 与 ChaCha20-Poly1305 相比，1.78μs（约 64MiB/s 与 525MiB/s）。
- 为再现性而捕获的基准工件 (`target/criterion/sm_perf*`)； Linux 基线源自 `artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/`（CPU 标签 `neoverse-proxy-macos`，SM3 比较容差 0.70），并且可以在实验室时间开放后在裸机 Neoverse 主机 (`SM-4c.1`) 上刷新以收紧容差。

#### 跨架构捕获清单
- 在目标机器**（x86_64 工作站、Neoverse ARM 服务器等）上运行 `scripts/sm_perf_capture_helper.sh`。通过 `--cpu-label <host>` 来标记捕获并（在矩阵模式下运行时）预填充生成的计划/命令以进行实验室安排。帮助程序打印特定于模式的命令：
  1. 使用正确的功能集执行 Criterion 套件，并且
  2. 将中位数写入 `crates/iroha_crypto/benches/sm_perf_baseline_${arch}_${os}_${mode}.json`。
- 首先捕获标量基线，然后重新运行 `auto`（以及 AArch64 平台上的 `neon-force`）的帮助程序。使用有意义的 `SM_PERF_CPU_LABEL`，以便审阅者可以跟踪 JSON 元数据中的主机详细信息。
- 每次运行后，存档原始 `target/criterion/sm_perf*` 目录并将其与生成的基线一起包含在 PR 中。一旦连续两次运行稳定，就收紧每个基准的容差（有关参考格式，请参阅 `sm_perf_baseline_aarch64_macos_*.json`）。
- 记录本节中的中位数 + 公差，并在涵盖新架构时更新 `status.md`/`roadmap.md`。 Linux 基线现在从 `neoverse-proxy-macos` 捕获中签入（元数据记录了导出到 aarch64-unknown-linux-gnu 门）；当这些实验室插槽可用时，在裸机 Neoverse/x86_64 主机上重新运行作为后续操作。

#### ARMv8 SM3/SM4 内在函数与标量路径
`sm_accel`（请参阅 `crates/iroha_crypto/src/sm.rs:739`）为 NEON 支持的 SM3/SM4 帮助器提供运行时调度层。该功能受到三个级别的保护：|层|控制|笔记|
|--------|---------|--------|
|编译时间| `--features sm`（现在在 `aarch64` 上自动引入 `sm-neon`）或 `sm-neon-force`（测试/基准测试）|构建 NEON 模块并链接 `sm3-neon`/`sm4-neon`。 |
|运行时自动检测 | `sm4_neon::is_supported()` |仅适用于公开 AES/PMULL 等效项的 CPU（例如 Apple M 系列、Neoverse V1/N2）。屏蔽 NEON 或 FEAT_SM4 的虚拟机会回退到标量代码。 |
|操作员覆盖| `crypto.sm_intrinsics` (`auto`/`force-enable`/`force-disable`) |启动时应用配置驱动的调度；仅将 `force-enable` 用于受信任环境中的分析，并在验证标量回退时首选 `force-disable`。 |

**性能范围（Apple M3 Pro；中值记录在 `sm_perf_baseline_aarch64_macos_{mode}.json` 中）：**

|模式| SM3 摘要 (4KiB) | SM4-GCM 密封 (1KiB) |笔记|
|------|--------------------|----------------------|--------|
|标量 | 11.6μs | 11.6μs 15.9μs | 15.9μs确定性 RustCrypto 路径；在编译 `sm` 功能的任何地方都使用它，但 NEON 不可用。 |
|霓虹灯汽车 |比标量快约 2.7 倍 |比标量快约 2.3 倍 |当前的 NEON 内核 (SM-5a.2c) 将调度一次加宽四个字并使用双队列扇出；确切的中位数因主机而异，因此请参阅基准 JSON 元数据。 |
|霓虹力量|镜像 NEON auto 但完全禁用回退 |与 NEON 汽车相同 |通过 `scripts/sm_perf.sh --mode neon-force` 行使；即使在默认为标量模式的主机上也能保持 CI 诚实。 |

**确定性和部署指南**
- 内在函数永远不会改变可观察的结果 - 当加速路径不可用时，`sm_accel` 返回 `None`，因此标量助手运行。因此，只要标量实现正确，共识代码路径就保持确定性。
- **不要**对是否使用 NEON 路径进行业务逻辑门控。将加速度纯粹视为性能提示，并仅通过遥测技术公开状态（例如，`sm_intrinsics_enabled` 仪表）。
- 接触 SM 代码后始终运行 `ci/check_sm_perf.sh`（或 `make check-sm-perf`），以便 Criterion 工具使用每个基线 JSON 中嵌入的容差来验证标量路径和加速路径。
- 在进行基准测试或调试时，更喜欢配置旋钮 `crypto.sm_intrinsics` 而不是编译时标志；使用 `sm-neon-force` 重新编译会完全禁用标量回退，而 `force-enable` 只是轻推运行时检测。
- 在发行说明中记录所选策略：生产版本应将策略保留在 `Auto` 中，让每个验证器独立发现硬件功能，同时仍共享相同的二进制工件。
- 避免交付混合静态链接的供应商内在函数（例如第三方 SM4 库）的二进制文件，除非它们遵循相同的调度和测试流程，否则我们的基线工具将无法捕获性能回归。#### x86_64 Rosetta 基线（Apple M3 Pro；捕获于 2025 年 12 月 1 日）
- 基线位于 `crates/iroha_crypto/benches/sm_perf_baseline_x86_64_macos_{scalar,auto,neon_force}.json` (cpu_label=`m3-pro-rosetta`) 中，原始 + 聚合捕获位于 `artifacts/sm_perf/2026-03-lab/m3pro_rosetta/` 下。
- x86_64 上的每个基准容差对于 SM2 设置为 20%，对于 Ed25519/SHA-256 设置为 15%，对于 SM4/ChaCha 设置为 12%。 `scripts/sm_perf.sh` 现在在非 AArch64 主机上将加速比较容差默认为 25%，因此标量与自动保持紧密，同时在 AArch64 上为共享 `m3-pro-native` 基线留下 5.25 的余量，直到 Neoverse 重新运行。

|基准|标量 |汽车 |霓虹力量 |自动与标量 |霓虹灯与标量 |霓虹灯与汽车|
|------------|--------|------|------------|----------------|----------------------------|------------|
| sm2_vs_ed25519_sign/ed25519_sign | sm2_vs_ed25519_sign/ed25519_sign |    57.43 | 57.43  57.12 | 57.12      55.77 | 55.77          -0.53% |         -2.88% |        -2.36% |
| sm2_vs_ed25519_sign/sm2_sign | sm2_vs_ed25519_sign/sm2_sign |   572.76 | 572.76 568.71 | 568.71     557.83 | 557.83          -0.71% |         -2.61% |        -1.91% |
| sm2_vs_ed25519_verify/验证/ed25519 |    69.03 |  68.42 |      66.28 |          -0.88% |         -3.97% |        -3.12% |
| sm2_vs_ed25519_verify/验证/sm2 |   521.73 | 521.73 514.50 | 514.50     502.17 | 502.17          -1.38% |         -3.75% |        -2.40% |
| sm3_vs_sha256_hash/sha256_hash | sm3_vs_sha256_hash/sha256_hash |    16.78 | 16.78  16.58 | 16.58      16.16 | 16.16          -1.19% |         -3.69% |        -2.52% |
| sm3_vs_sha256_hash/sm3_hash | sm3_vs_sha256_hash/sm3_hash |    15.78 | 15.78  15.51 | 15.51      15.04 | 15.04          -1.71% |         -4.69% |        -3.03% |
| sm4_vs_chacha20poly1305_decrypt/chacha20poly1305_decrypt | sm4_vs_chacha20poly1305_decrypt     1.96 | 1.96   1.97 | 1.97       1.97 | 1.97           0.39% |          0.16% |        -0.23% |
| sm4_vs_chacha20poly1305_decrypt/sm4_gcm_decrypt | sm4_vs_chacha20poly1305_decrypt/sm4_gcm_decrypt |    16.26 | 16.26  16.38 | 16.38      16.26 | 16.26           0.72% |         -0.01% |        -0.72% |
| sm4_vs_chacha20poly1305_加密/chacha20poly1305_加密 |     1.96 | 1.96   2.00 | 2.00       1.93 | 1.93           2.23% |         -1.14% |        -3.30% |
| sm4_vs_chacha20poly1305_加密/sm4_gcm_加密 |    16.60 | 16.60  16.58 | 16.58      16.15 | 16.15          -0.10% |         -2.66% |        -2.57% |

#### x86_64 /其他非aarch64目标
- 当前版本仍然仅在 x86_64 上提供确定性 RustCrypto 标量路径；保持 `sm` 启用，但在 SM-4c.1b 登陆之前**不**注入外部 AVX2/VAES 内核。运行时策略镜像 ARM：默认为 `Auto`，遵循 `crypto.sm_intrinsics`，并显示相同的遥测仪表。
- Linux/x86_64 捕获仍有待记录；重用该硬件上的帮助程序，并将中位数与上面的 Rosetta 基线和容差图一起放入 `sm_perf_baseline_x86_64_unknown_linux_gnu_{mode}.json` 中。**常见陷阱**
1. **虚拟化 ARM 实例：** 许多云公开 NEON，但隐藏 `sm4_neon::is_supported()` 检查的 SM4/AES 扩展。预期这些环境中的标量路径并相应地捕获性能基线。
2. **部分覆盖：** 在运行之间混合保留的 `crypto.sm_intrinsics` 值会导致性能读数不一致。在实验票证中记录预期的覆盖，并在捕获新基线之前重置配置。
3. **CI 奇偶校验：** 一些 macOS 运行程序在 NEON 处于活动状态时不允许基于计数器的性能采样。将 `scripts/sm_perf_capture_helper.sh` 输出附加到 PR，以便审核者可以确认加速路径已被执行，即使跑步者隐藏了这些计数器。
4. **未来 ISA 变体 (SVE/SVE2)：** 当前内核采用 NEON 通道形状。在移植到 SVE/SVE2 之前，使用专用变体扩展 `sm_accel::NeonPolicy`，以便我们可以保持 CI、遥测和操作旋钮保持一致。

SM-5a/SM-4c.1 下跟踪的行动项目确保 CI 捕获每个新架构的奇偶校验证明，并且路线图保持在 🈺 直到 Neoverse/x86 基线和 NEON 与标量容差收敛。

## 合规性和监管说明

### 标准和规范性参考资料
- **GM/T 0002-2012** (SM4)、**GM/T 0003-2012** + **GB/T 32918 系列** (SM2)、**GM/T 0004-2012** + **GB/T 32905/32907** (SM3) 和 **RFC 8998** 管理我们的算法定义、测试向量和 KDF 绑定灯具消耗。【docs/source/crypto/sm_vectors.md#L79】
- `docs/source/crypto/sm_compliance_brief.md` 中的合规简介将这些标准与工程、SRE 和法律团队的归档/导出责任交叉链接；每当 GM/T 目录修订时，请及时更新该摘要。

### 中国大陆监管工作流程
1. **产品备案（开发备案）：** 在从中国大陆运送支持 SM 的二进制文件之前，请将工件清单、确定性构建步骤和依赖项列表提交给省级密码管理部门。归档模板和合规性检查表位于 `docs/source/crypto/sm_compliance_brief.md` 和附件目录（`sm_product_filing_template.md`、`sm_sales_usage_filing_template.md`、`sm_export_statement_template.md`）中。
2. **销售/使用备案（销售/使用备案）：** 在岸上运行支持 SM 的节点的运营商必须注册其部署范围、密钥管理态势和遥测计划。归档时附上已签名的清单以及 `iroha_sm_*` 指标快照。
3. **认可的测试：** 关键基础设施运营商可能需要经过认证的实验室报告。提供可重现的构建脚本、SBOM 导出和 Wycheproof/互操​​作工件（见下文），以便下游审核员可以在不更改代码的情况下重现向量。
4. **状态跟踪：** 在放行单和 `status.md` 中记录已完成的备案；缺少备案会阻止从仅验证试点升级到签名试点。### 出口和分销态势
- 根据**美国 EAR 类别 5 第 2 部分**和**欧盟法规 2021/821 附件 1 (5D002)**，将支持 SM 的二进制文件视为受控项目。源代码的发布仍然符合开源/ENC 豁免的资格，但重新分发到禁运目的地仍需要法律审查。
- 发布清单必须捆绑引用 ENC/TSU 基础的导出语句，并列出 OpenSSL/Tongsuo 构建标识符（如果打包了 FFI 预览版）。
- 当运营商需要陆上配送以避免跨境转移问题时，优先选择区域本地包装（例如大陆镜像）。

### 操作员文档和证据
- 将该架构简介与 `docs/source/crypto/sm_operator_rollout.md` 中的部署清单以及 `docs/source/crypto/sm_compliance_brief.md` 中的合规性归档指南配对。
- 使创世/操作员快速入门在 `docs/genesis.md`、`docs/genesis.he.md` 和 `docs/genesis.ja.md` 之间保持同步； SM2/SM3 CLI 工作流程是面向操作员的用于播种 `crypto` 清单的事实来源。
- 将 OpenSSL/Tongsuo 出处、`scripts/sm_openssl_smoke.sh` 输出和 `scripts/sm_interop_matrix.sh` 奇偶校验日志与每个版本捆绑包一起存档，以便合规性和审计合作伙伴拥有确定性的工件。
- 每当合规范围发生变化（新的司法管辖区、备案完成或出口决定）时更新 `status.md`，以保持程序状态可发现。
- 遵循 `docs/source/release_dual_track_runbook.md` 中捕获的分阶段准备情况审核 (`SM-RR1`–`SM-RR3`)；仅验证阶段、试点阶段和 GA 签名阶段之间的升级需要此处列举的工件。

## 互操作食谱

### RustCrypto ↔ OpenSSL/通索矩阵
1. 确保 OpenSSL/Tongsuo CLI 可用（`IROHA_SM_CLI="openssl /opt/tongsuo/bin/openssl"` 允许显式工具选择）。
2、运行`scripts/sm_interop_matrix.sh`；它调用 `cargo test -p iroha_crypto --test sm_cli_matrix --features sm` 并对每个提供商执行 SM2 签名/验证、SM3 摘要和 SM4 ECB/GCM 流程，跳过任何不存在的 CLI。【scripts/sm_interop_matrix.sh#L1】
3. 将生成的 `target/debug/deps/sm_cli_matrix*.log` 文件与发布工件存档。

### OpenSSL 预览烟雾（打包门）
1. 安装 OpenSSL ≥3.0 开发标头并确保 `pkg-config` 可以找到它们。
2、执行`scripts/sm_openssl_smoke.sh`；助手运行 `cargo check`/`cargo test --test sm_openssl_smoke`，通过 FFI 后端执行 SM3 哈希、SM2 验证和 SM4-GCM 往返（测试工具显式启用预览）。【scripts/sm_openssl_smoke.sh#L1】
3. 将任何不可跳过的失败视为释放阻塞；捕获控制台输出以获取审计证据。

### 确定性夹具刷新
- 在每次合规性归档之前重新生成 SM 装置（`sm_vectors.md`、`fixtures/sm/…`），然后重新运行奇偶校验矩阵和烟雾控制装置，以便审核员在归档的同时收到新的确定性记录。## 外部审计准备
- `docs/source/crypto/sm_audit_brief.md` 打包了外部审查的背景、范围、时间表和联系人。
- 审计工件位于 `docs/source/crypto/attachments/`（OpenSSL 烟雾日志、货物树快照、货物元数据导出、工具包出处）和 `fuzz/sm_corpus_manifest.json`（源自现有回归向量的确定性 SM 模糊种子）下。在 macOS 上，烟雾日志当前记录了跳过的运行，因为工作区依赖循环阻止了 `cargo check`；没有循环的 Linux 构建将充分利用预览后端。
- 于 2026 年 1 月 30 日分发给加密工作组、平台运营、安全和 Docs/DevRel 领导，以便在 RFQ 发送之前进行协调。

### 审计参与状态

- **Trail of Bits（CN 密码学实践）** — 工作说明书于 **2026-02-21** 执行，启动 **2026-02-24**，现场工作窗口 **2026-02-24–2026-03-22**，最终报告到期 **2026-04-15**。每周三 09:00UTC 与加密货币工作组领导和安全工程联络员进行每周状态检查。有关联系人、可交付成果和证据附件，请参阅 [`sm_audit_brief.md`](sm_audit_brief.md#engagement-status)。
- **NCC Group APAC（应急时段）** — 保留 2026 年 5 月的窗口期，作为后续/并行审查，以防其他调查结果或监管机构要求需要第二意见。参与细节和升级挂钩与 `sm_audit_brief.md` 中的 Trail of Bits 条目一起记录。

## 风险与缓解措施

完整寄存器：详见[`sm_risk_register.md`](sm_risk_register.md)
概率/影响评分、监控触发因素和签核历史记录。的
下面的摘要跟踪了发布工程中出现的标题项目。
|风险|严重性 |业主|缓解措施 |
|------|----------|--------|------------|
| RustCrypto SM 板条箱缺乏外部审计 |高|加密工作组 | Bits/NCC 集团的合约追踪，在审计报告被接受之前保持仅验证状态。 |
|跨 SDK 的确定性随机数回归 |高| SDK 项目负责人 |跨 SDK CI 共享固定装置；强制执行规范的 r∥s 编码；添加跨 SDK 集成测试（在 SM-3c 中跟踪）。 |
|内在函数中特定于 ISA 的错误 |中等|绩效工作组 |功能门内在函数，需要 ARM 上的 CI 覆盖，维护软件回退。硬件验证矩阵维护在 `sm_perf.md` 中。 |
|合规性模糊性阻碍了采用 |中等|文件和法律联络 |在 GA 之前发布合规简介和操作员清单 (SM-6a/SM-6b)；收集法律意见。 `sm_compliance_brief.md` 中提供的归档清单。 |
| FFI 后端随提供商更新而变化 |中等|平台运营|固定提供商版本、添加奇偶校验测试、保持 FFI 后端选择加入直至封装稳定 (SM-P3)。 |## 未决问题/后续行动
1. 选择具有 Rust SM 算法经验的独立审计合作伙伴。
   - **答复（2026-02-24）：** Trail of Bits 的 CN 密码学实践签署了初步审计 SOW（于 2026 年 2 月 24 日启动，于 2026 年 4 月 15 日交付），并且 NCC Group APAC 拥有 5 月份的应急时段，以便监管机构可以在不重新开放采购的情况下请求第二次审查。参与范围、联系人和清单位于 [`sm_audit_brief.md`](sm_audit_brief.md#engagement-status) 中，并镜像在 `sm_audit_vendor_landscape.md` 中。
2. 继续向上游追踪官方 Wycheproof SM2 数据集；该工作区目前提供了一个精心策划的 52 个案例套件（确定性夹具 + 合成篡改案例）并将其输入 `sm2_wycheproof.rs`/`sm2_fuzz.rs`。上游 JSON 落地后，通过 `cargo xtask sm-wycheproof-sync` 更新语料库。
   - 跟踪 Bouncy Castle 和 GmSSL 负矢量套件；许可清除后导入 `sm2_fuzz.rs` 以补充现有语料库。
3. 定义 SM 采用监控的基线遥测（指标、日志记录）。
4. 决定 Kotodama/VM 曝光的 SM4 AEAD 默认为 GCM 还是 CCM。
5. 跟踪附件示例 1 (ID `ALICE123@YAHOO.COM`) 的 RustCrypto/OpenSSL 奇偶校验：确认库支持已发布的公钥和 `(r, s)`，以便装置可以升级到回归测试。

## 行动项目
- [x] 在安全跟踪器中完成依赖性审核和捕获。
- [x] 确认 RustCrypto SM 箱的审计合作伙伴参与（SM-P0 后续）。 Trail of Bits（CN 密码学实践）拥有主要审查权，启动/交付日期记录在 `sm_audit_brief.md` 中，NCC Group APAC 保留了 2026 年 5 月的应急时段，以满足监管机构或治理的后续要求。
- [x] 扩大 SM4 CCM 防篡改案例 (SM-4a) 的 Wycheproof 覆盖范围。
- [x] 在 SDK 中引入规范的 SM2 签名装置并连接到 CI (SM-3c/SM-1b.1)；由 `scripts/check_sm2_sdk_fixtures.py` 保护（参见 `ci/check_sm2_sdk_fixtures.sh`）。

## 合规性附录（国家商业密码）

- **分类：** SM2/SM3/SM4 遵循中国*国家商业密码*制度（《中华人民共和国密码法》第 3 条）。在 Iroha 软件中传送这些算法并不**将项目置于核心/公共（国家机密）层，但在 PRC 部署中使用它们的运营商必须遵守商业加密备案和 MLPS 义务。【docs/source/crypto/sm_chinese_crypto_law_brief.md:14】
- **标准沿袭：** 将公共文档与 GM/T 规范的官方 GB/T 转换保持一致：

|算法| GB/T参考| GM/T原点|笔记|
|------------|----------------|-------------|--------|
| SM2 | GB/T32918（所有部分） | GM/T0003 | ECC数字签名+密钥交换； Iroha 向 SDK 公开核心节点中的验证和确定性签名。 |
| SM3 | GB/T32905| GM/T0004 | 256 位哈希；跨标量和 ARMv8 加速路径的确定性哈希。 |
| SM4 | GB/T32907| GM/T0002 | 128位分组密码； Iroha 提供 GCM/CCM 帮助程序并确保跨实现的大端奇偶校验。 |- **功能清单：** Torii `/v1/node/capabilities` 端点通告以下 JSON 形状，以便操作员和工具可以以编程方式使用 SM 清单：

```json
{
  "abi_version": 1,
  "data_model_version": 1,
  "crypto": {
    "sm": {
      "enabled": true,
      "default_hash": "sm3-256",
      "allowed_signing": ["ed25519"],
      "sm2_distid_default": "1234567812345678",
      "openssl_preview": false,
      "acceleration": {
        "scalar": true,
        "neon_sm3": false,
        "neon_sm4": false,
        "policy": "auto"
      }
    }
  }
}
```

CLI 子命令 `iroha runtime capabilities` 在本地显示相同的负载，打印一行摘要以及 JSON 广告以收集合规性证据。

- **文档交付成果：**发布识别上述算法/标准的发行说明和 SBOM，并将完整的合规简介 (`sm_chinese_crypto_law_brief.md`) 与发布工件捆绑在一起，以便运营商可以将其附加到省级备案中。【docs/source/crypto/sm_chinese_crypto_law_brief.md:59】
- **操作员交接：**提醒部署者MLPS2.0/GB/T39786-2021需要加密应用评估、SM密钥管理SOP和≥6年的证据保留；让他们查看合规简报中的运营商清单。【docs/source/crypto/sm_chinese_crypto_law_brief.md:43】【docs/source/crypto/sm_chinese_crypto_law_brief.md:74】

## 沟通计划
- **观众：** Crypto WG 核心成员、发布工程、安全审查委员会、SDK 项目负责人。
- **工件：** `sm_program.md`、`sm_lock_refresh_plan.md`、`sm_vectors.md`、`sm_wg_sync_template.md`、路线图摘录（SM-0 .. SM-7a）。
- **渠道：** 每周加密工作组同步议程 + 后续电子邮件总结行动项目并请求批准锁定刷新和依赖项获取（草案于 2025 年 1 月 19 日分发）。
- **所有者：** 加密货币工作组领导（可接受代表）。