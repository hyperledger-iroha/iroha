---
lang: zh-hant
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c90383149066d2e43cef962e6fe946f939277c3f7d22f3ee4688db8cc96b23b2
source_last_modified: "2026-01-05T09:28:11.912107+00:00"
translation_last_reviewed: 2026-02-07
id: pq-primitives
title: SoraNet Post-Quantum Primitives
sidebar_label: PQ Primitives
description: Overview of the `soranet_pq` crate and how the SoraNet handshake consumes ML-KEM/ML-DSA helpers.
translator: machine-google-reviewed
---

:::注意規範來源
:::

`soranet_pq` 箱包含每個 SoraNet 所使用的後量子構建塊
中繼、客戶端和工具組件依賴。它封裝了 PQClean 支持的 Kyber
(ML-KEM) 和 Dilithium (ML-DSA) 套件和層位於協議友好的 HKDF 和
對沖 RNG 助手，因此所有表面共享相同的實現。

## `soranet_pq` 中包含哪些內容

- **ML-KEM-512/768/1024：** 確定性密鑰生成、封裝和
  具有恆定時間錯誤傳播的解封裝助手。
- **ML-DSA-44/65/87：** 獨立簽名/驗證有線
  域分隔的轉錄本。
- **標記為 HKDF:** `derive_labeled_hkdf` 命名空間的每個派生都帶有
  握手階段（`DH/es`、`KEM/1`，...），因此混合轉錄本保持無碰撞。
- **對沖隨機性：** `hedged_chacha20_rng` 混合確定性種子
  具有實時操作系統熵並在下降時將中間狀態歸零。

所有秘密都位於 `Zeroizing` 容器內，CI 執行 PQClean
每個支持的平台上的綁定。

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

1. **將依賴項添加到位於工作區根目錄之外的 crate：

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **在呼叫站點選擇正確的套件**。對於初始混合握手
   工作，使用 `MlKemSuite::MlKem768` 和 `MlDsaSuite::MlDsa65`。

3. **使用標籤派生密鑰。 ** 使用 `HkdfDomain::soranet("KEM/1")` （和同級）
   因此轉錄本鏈接在節點之間保持確定性。

4. **在對後備秘密進行採樣時使用對沖 RNG**：

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

核心 SoraNet 握手和 CID 致盲助手 (`iroha_crypto::soranet`)
直接拉取這些實用程序，這意味著下游 crate 繼承相同的
無需鏈接 PQClean 綁定本身的實現。

## 驗證清單

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- 審核自述文件使用示例 (`crates/soranet_pq/README.md`)
- 混合動力落地後更新 SoraNet 握手設計文檔