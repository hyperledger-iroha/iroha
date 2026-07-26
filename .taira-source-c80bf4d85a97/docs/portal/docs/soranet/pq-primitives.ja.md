---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 328b1a5d15b139d45c60c7edc4909a15c7d983fc350cb4909bbdb31e81863776
source_last_modified: "2025-11-10T15:59:30.991051+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: pq-primitives
title: SoraNet ポスト量子プリミティブ
sidebar_label: PQ プリミティブ
description: `soranet_pq` クレートの概要と、SoraNet ハンドシェイクが ML-KEM/ML-DSA ヘルパーをどう利用するか。
---

:::note Canonical Source
このページは `docs/source/soranet/pq_primitives.md` を反映する。従来のドキュメント一式が廃止されるまで両方を同期しておくこと。
:::

`soranet_pq` クレートは、SoraNet の relay、client、tooling コンポーネントが依存するポスト量子の基盤を提供する。PQClean バックの Kyber (ML-KEM) と Dilithium (ML-DSA) を包み込み、プロトコル向けの HKDF と hedged RNG のヘルパーを重ねることで、あらゆる面が同一の実装を共有できるようにしている。

## `soranet_pq` に含まれるもの

- **ML-KEM-512/768/1024:** 決定論的な鍵生成、カプセル化/デカプセル化のヘルパー、定数時間のエラー伝播。
- **ML-DSA-44/65/87:** ドメイン分離されたトランスクリプトに紐づく分離署名/検証。
- **ラベル付き HKDF:** `derive_labeled_hkdf` がハンドシェイク段階 (`DH/es`, `KEM/1`, ...) ごとに派生へ namespace を付け、ハイブリッドトランスクリプトの衝突を防ぐ。
- **Hedged 乱数:** `hedged_chacha20_rng` が決定論 seed と OS エントロピーを混合し、破棄時に中間状態をゼロ化する。

すべての秘密は `Zeroizing` コンテナ内に保持され、CI は全サポートプラットフォームで PQClean bindings を検証する。

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

## 利用方法

1. **依存関係を追加** する (workspace root 外の crates 向け):

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **適切なスイートを選択** する。初期のハイブリッドハンドシェイクでは `MlKemSuite::MlKem768` と `MlDsaSuite::MlDsa65` を使用する。

3. **ラベル付きで鍵を導出** する。`HkdfDomain::soranet("KEM/1")` (および同系) を使い、トランスクリプトの連鎖をノード間で決定論的に保つ。

4. **Hedged RNG を使用** してフォールバック秘密をサンプルする:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

SoraNet のコアハンドシェイクと CID ブラインディングヘルパー (`iroha_crypto::soranet`) はこれらのユーティリティを直接利用するため、下流の crates は PQClean bindings をリンクせずに同一実装を継承できる。

## 検証チェックリスト

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- README の使用例を監査 (`crates/soranet_pq/README.md`)
- ハイブリッド導入後に SoraNet ハンドシェイク設計ドキュメントを更新
