---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-プリミティブ
title: SoraNet ポスト量子プリミティブ
サイドバー_ラベル: PQ プリミティブ
説明: `soranet_pq` クレートの概要 説明 SoraNet ハンドシェイク ML-KEM/ML-DSA ヘルパー 説明 説明
---

:::note 正規ソース
یہ صفحہ `docs/source/soranet/pq_primitives.md` کی عکاسی کرتا ہے۔ドキュメント セットの引退 دونوں کاپیاں 同期 رکھیں۔
:::

`soranet_pq` クレートポスト量子ビルディング ブロックの構成SoraNet リレークライアントのツール コンポーネントの構成PQClean を利用した Kyber (ML-KEM) ダイリチウム (ML-DSA) スイート ラップ、プロトコルフレンドリーな HKDF、ヘッジされた RNG ヘルパー、表面処理実装の共有

## `soranet_pq` میں کیا شامل ہے

- **ML-KEM-512/768/1024:** 決定論的キー生成、カプセル化、カプセル化解除ヘルパー、定数時間エラー伝播。
- **ML-DSA-44/65/87:** 分離された署名/検証、ドメイン分離トランスクリプト、有線接続。
- **ラベル付き HKDF:** `derive_labeled_hkdf` の派生ハンドシェイク ステージ (`DH/es`、`KEM/1`、...) 名前空間、衝突のないハイブリッド トランスクリプト。
- **ヘッジされたランダム性:** `hedged_chacha20_rng` 決定性シード、ライブ OS エントロピー、ブレンド、中間状態のドロップ、ゼロ化。

秘密 `Zeroizing` コンテナー サポートされているプラ​​ットフォーム CI サポートされているプラ​​ットフォーム PQClean バインディング 演習

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

## ستعمال کیسے کریں

1. **依存関係の説明** クレートの作成とワークスペースのルートの作成:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **スイートスイート منتخب کریں** サイトを呼び出す پر۔ハイブリッド ハンドシェイク `MlKemSuite::MlKem768` `MlDsaSuite::MlDsa65` ハイブリッド ハンドシェイク

3. **ラベルのキーは次の結果を導き出します** `HkdfDomain::soranet("KEM/1")` (応答) トランスクリプト チェーン ノードの決定性りょく

4. **ヘッジされた RNG の例** フォールバック シークレットのサンプル例:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

SoraNet コア ハンドシェイク、CID ブラインディング ヘルパー (`iroha_crypto::soranet`) ユーティリティ ユーティリティ、ダウンストリーム クレートPQClean バインディングのリンク 実装は継承を継承します

## 検証チェックリスト

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- README 使用例と監査 (`crates/soranet_pq/README.md`)
- ハイブリッドの開発、SoraNet ハンドシェイク設計ドキュメントの開発、開発、開発