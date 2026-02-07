---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-プリミティブ
title: ソラネットのプリミティバス・ポスクアンティカス
サイドバーラベル: プリミティバス PQ
説明: SoraNet 消費ヘルパー ML-KEM/ML-DSA のハンドシェイクに関する `soranet_pq` の履歴書。
---

:::ノート フエンテ カノニカ
エスタページナリフレジャ`docs/source/soranet/pq_primitives.md`。 Manten ambas versiones sincronizadas hasta que el conjunto de documentacion heredado se retire.
:::

El crate `soranet_pq` は、SoraNet のツールのリレー、クライアント、およびコンポーネントを含むブロック poscuanticos を管理します。 Kyber (ML-KEM) および Dilithium (ML-DSA) の PQClean および HKDF ヘルパーの集合体および RNG ヘッジ アプトス パラレル プロトコル パラケーター クラスの実装の同一性を監視します。

## キューには `soranet_pq` が含まれます

- **ML-KEM-512/768/1024:** クラーベスでの決定性の生成、カプセル化とカプセル化解除によるエラーの伝播、および定数。
- **ML-DSA-44/65/87:** ファームード/検証は、ドメインの分離と転写を分離します。
- **HKDF のエチケット:** `derive_labeled_hkdf` 集合名前空間は、ハンドシェイク (`DH/es`, `KEM/1`, ...) との衝突のない転写パラメタです。
- **Aleatoriedad ヘッジ:** `hedged_chacha20_rng` メズクラ セミリャス 決定論的コンエントロピア デル SO は、再帰的な問題を解決します。

`Zeroizing` と、PQClean のプラットフォームからのバインディングの CI の削除。

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

## コモ・コンスミルロ

1. **Agrega la dependencia** ワークスペース ルートのクレート:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **ラマダの正しい選択**。米国 `MlKemSuite::MlKem768` および `MlDsaSuite::MlDsa65` の最初の握手日です。

3. **Derva claves con etiquetas.** 米国 `HkdfDomain::soranet("KEM/1")` (y 等価物) は、エンカデナミエント デ トランスクリプション サイトの決定性を確認します。

4. **米国エルRNGヘッジ** al muestrear Secretos de respaldo:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

SoraNet の中央ハンドシェイクは、CID (`iroha_crypto::soranet`) の盲目的なヘルパーを消費し、重要な機能をダウンストリームで実装し、PQClean とミスモスのバインディングを実行します。

## 検証のチェックリスト

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- README の監査 (`crates/soranet_pq/README.md`)
- SoraNet のハンドシェイクに関する文書とハイブリッドを実際に確認