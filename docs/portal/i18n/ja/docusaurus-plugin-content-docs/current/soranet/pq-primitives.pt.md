---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-プリミティブ
タイトル: SoraNet の Primitivos pos-quanticos
サイドバーラベル: プリミティブ PQ
説明: Visao geral はクレート `soranet_pq` とハンドシェイクを行い、SoraNet コンソーム ヘルパー ML-KEM/ML-DSA を実行します。
---

:::note フォンテ カノニカ
エスタ・ページナ・エスペルハ`docs/source/soranet/pq_primitives.md`。マンテンハ・アンバスはコピア・シンクロニザダスとして。
:::

O crate `soranet_pq` は、ブロック、pos-quanticos、todo リレー、クライアントおよびコンポーネントのツールを管理し、SoraNet 構成を実行します。 Ele は、PQClean、HKDF の補助ヘルパー、および RNG ヘッジ アミガベーションのスイートとして Kyber (ML-KEM) と Dilithium (ML-DSA) をサポートし、同一性を実現するためのプロトコルとして機能します。

## お願いします `soranet_pq`

- **ML-KEM-512/768/1024:** 不安定な決定性、カプセル化解除とカプセル化解除のヘルパーによるテンポ定数のエラーの伝達。
- **ML-DSA-44/65/87:** assinatura/verificacao separadas para transcricoes com separacao de dominio。
- **HKDF com rotulo:** `derive_labeled_hkdf` adiciona namespace a cada derivacao com o estagio do handshake (`DH/es`, `KEM/1`, ...) パラメータ ヒブリダス フィケム セム コリソ。
- **Aleatoriedade ヘッジ:** `hedged_chacha20_rng` ミスチュラの種子の決定性は、SO e zera o estado intermediario ao descartar を参照してください。

`Zeroizing` は、プラットフォーム サポートとして PQClean のバインディングを実行する CI です。

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

## コモ消費者

1. **依存関係のある** ワークスペース用のクレート:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **ポントス デ シャマダの**コレッタ スイートを選択**。トラバルホが最初にハンドシェイクを行う場合は、`MlKemSuite::MlKem768` または `MlDsaSuite::MlDsa65` を使用します。

3. **コンロチュロスを導出します。** `HkdfDomain::soranet("KEM/1")` (類似) を使用して、暗号化された決定的なエントリ ノードを暗号化します。

4. **RNG ヘッジを使用** フォールバックのセグレドス:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

SoraNet の OS ヘルパーと CID (`iroha_crypto::soranet`) のブラインディング ディレタメントを行うハンドシェイク セントラル。独自の管理者向けに SEM リンカー バインディングを実装するための重要な下流ヘルダム クレートを管理します。

## 検証チェックリスト

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- 使用例の README を参照 (`crates/soranet_pq/README.md`)
- ドキュメントのデザイン、ハンドシェイク、SoraNet Quando OS ハイブリッド チェガレムの実現