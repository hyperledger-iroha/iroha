---
id: pq-primitives
lang: zh-hans
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraNet Post-Quantum Primitives
sidebar_label: PQ Primitives
description: Overview of the `soranet_pq` crate and how the SoraNet handshake consumes ML-KEM/ML-DSA helpers.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意规范来源
:::

`soranet_pq` 箱包含每个 SoraNet 所使用的后量子构建块
中继、客户端和工具组件依赖。它封装了 PQClean 支持的 Kyber
(ML-KEM) 和 Dilithium (ML-DSA) 套件和层位于协议友好的 HKDF 和
对冲 RNG 助手，因此所有表面共享相同的实现。

## `soranet_pq` 中包含哪些内容

- **ML-KEM-512/768/1024：** 确定性密钥生成、封装和
  具有恒定时间错误传播的解封装助手。
- **ML-DSA-44/65/87：** 独立签名/验证有线
  域分隔的转录本。
- **标记为 HKDF:** `derive_labeled_hkdf` 命名空间的每个派生都带有
  握手阶段（`DH/es`、`KEM/1`，...），因此混合转录本保持无碰撞。
- **对冲随机性：** `hedged_chacha20_rng` 混合确定性种子
  具有实时操作系统熵并在下降时将中间状态归零。

所有秘密都位于 `Zeroizing` 容器内，CI 执行 PQClean
每个支持的平台上的绑定。

```rust
use soranet_pq::{
    encapsulate_mlkem, decapsulate_mlkem, generate_mlkem_keypair, MlKemSuite,
    derive_labeled_hkdf, HkdfDomain, HkdfSuite,
};

let kem = generate_mlkem_keypair(MlKemSuite::MlKem768);
let (client_secret, ciphertext) = encapsulate_mlkem(MlKemSuite::MlKem768, kem.public_key()).unwrap();
let server_secret = decapsulate_mlkem(MlKemSuite::MlKem768, kem.secret_key(), ciphertext.as_bytes()).unwrap();
assert_eq!(client_secret.as_bytes(), server_secret.as_bytes());

let okm = derive_labeled_hkdf(
    HkdfSuite::Sha3_256,
    None,
    client_secret.as_bytes(),
    HkdfDomain::soranet("KEM/1"),
    b"soranet-transcript",
    32,
).unwrap();
```

## 如何食用

1. **将依赖项添加到位于工作区根目录之外的 crate：

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **在呼叫站点选择正确的套件**。对于初始混合握手
   工作，使用 `MlKemSuite::MlKem768` 和 `MlDsaSuite::MlDsa65`。

3. **使用标签派生密钥。** 使用 `HkdfDomain::soranet("KEM/1")` （和同级）
   因此转录本链接在节点之间保持确定性。

4. **在对后备秘密进行采样时使用对冲 RNG**：

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

核心 SoraNet 握手和 CID 致盲助手 (`iroha_crypto::soranet`)
直接拉取这些实用程序，这意味着下游 crate 继承相同的
无需链接 PQClean 绑定本身的实现。

## 验证清单

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- 审核自述文件使用示例 (`crates/soranet_pq/README.md`)
- 混合动力落地后更新 SoraNet 握手设计文档