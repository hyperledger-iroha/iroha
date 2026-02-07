---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-プリミティブ
title: ポストクォンティークのプリミティブ SoraNet
サイドバーラベル: プリミティブ PQ
説明: `soranet_pq` とマニエールのアンサンブルはハンドシェイクを行わず、SoraNet コンソメ レス ヘルパー ML-KEM/ML-DSA を参照します。
---

:::note ソースカノニク
:::

Le crate `soranet_pq` コンテンツは、SoraNet ツールのリレー、クライアントおよび構成要素に関するポストクォンティック シュール レスケルの内容を示します。 Kyber (ML-KEM) と Dilithium (ML-DSA) をカプセル化することにより、PQClean とヘルパー HKDF および RNG を保護し、実装の同一性を示す表面のプロトコルに適応します。

## `soranet_pq` を参照してください

- **ML-KEM-512/768/1024:** 生成は、カプセル化とカプセル化解除の平均伝播エラー全体の周期を決定します。
- **ML-DSA-44/65/87:** 署名/検証アベック転写、ドメインの分離。
- **HKDF エチケット:** `derive_labeled_hkdf` アップリケは、ハンドシェイクのレタペ (`DH/es`、`KEM/1`、...) を介して名前空間から派生し、衝突を避けて転写ハイブリッドを作成します。
- **Aleatoire ヘッジ:** `hedged_chacha20_rng` は、システムのエントロピーと破壊の中間状態を決定するための種子を組み合わせます。

`Zeroizing` および CI は、PQClean のサポート対象のバインディングを実行するための秘密を保持しています。

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

## コメント l'utiliser

1. **Ajoutez la dependance** ワークスペースのラシーン用木箱:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **装飾品の選択** aux point d'appel。最初のハンドシェイク ハイブリッドを注ぎ、`MlKemSuite::MlKem768` と `MlDsaSuite::MlDsa65` を使用します。

3. **ラベルのラベルを取得します。** `HkdfDomain::soranet("KEM/1")` (等価物) を使用して、転写の連鎖を確認し、全体のラベルを決定します。

4. **RNG ヘッジを利用** 秘密を返信する:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

SoraNet の中心的なハンドシェイクと CID (`iroha_crypto::soranet`) の盲目ヘルパーのハンドシェイクは、PQClean の補助ミームのない下流の伝統的なミーム実装を管理するユーティリティの指示です。

## 検証のチェックリスト

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- README の使用例の監査 (`crates/soranet_pq/README.md`)
- ハンドシェイクの概念に関する文書を 1 時間かけて取得し、SoraNet が到着するまでのハイブリッドを確認します。