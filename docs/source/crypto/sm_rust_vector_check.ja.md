---
lang: ja
direction: ltr
source: docs/source/crypto/sm_rust_vector_check.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ce2f95b8b287c18c39232418333fbefdd300c030391be9dbfa4e29a3fd5f3e14
source_last_modified: "2026-01-03T18:07:57.109606+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! RustCrypto クレートを使用した SM2 Annex D ベクターの検証に関するメモ。

# SM2 Annex D ベクター検証 (RustCrypto)

このウォークスルーでは、RustCrypto の `sm2` クレートを使用して GM/T 0003 Annex D のサンプルを検証 (およびデバッグ) するために使用した手順をキャプチャします。正規の付録例 1 データ (アイデンティティ `ALICE123@YAHOO.COM`、メッセージ `"message digest"`、公開された `(r, s)`) は、`crates/iroha_crypto/tests/fixtures/sm_known_answers.toml` に記録されるようになりました。 OpenSSL/Tongsuo/gmssl は署名を喜んで検証します (`sm_vectors.md` を参照) が、RustCrypto の `sm2 v0.13.3` は依然として `signature::Error` とのポイントを拒否するため、Rust ハーネスが上流の修正を保留している間、CLI パリティが確認されます。

## 一時的なクレート

```bash
cargo new /tmp/sm2_verify --bin
cd /tmp/sm2_verify
```

`Cargo.toml`:

```toml
[package]
name = "sm2_verify"
version = "0.1.0"
edition = "2024"

[dependencies]
hex = "0.4"
sm2 = "0.13.3"
```

`src/main.rs`:

```rust
use hex::FromHex;
use sm2::dsa::{signature::Verifier, Signature, VerifyingKey};

fn main() {
    let distid = "ALICE123@YAHOO.COM";
    let sig_bytes = <Vec<u8>>::from_hex(
        "40f1ec59f793d9f49e09dcef49130d4194f79fb1eed2caa55bacdb49c4e755d16fc6dac32c5d5cf10c77dfb20f7c2eb667a457872fb09ec56327a67ec7deebe7",
    )
    .expect("signature hex");
    let sig_array = <[u8; 64]>::try_from(sig_bytes.as_slice()).unwrap();
    let signature = Signature::from_bytes(&sig_array).unwrap();

    let public_key = <Vec<u8>>::from_hex(
        "040ae4c7798aa0f119471bee11825be46202bb79e2a5844495e97c04ff4df2548a7c0240f88f1cd4e16352a73c17b7f16f07353e53a176d684a9fe0c6bb798e857",
    )
    .expect("public key hex");

    // This still returns Err with RustCrypto 0.13.3 – track upstream.
    let verifying_key = VerifyingKey::from_sec1_bytes(distid, &public_key).unwrap();

    verifying_key
        .verify(b"message digest", &signature)
        .expect("signature verified");
}
```

## 調査結果

- 正規の付録に対する検証例 1 `(r, s)` は、`sm2::VerifyingKey::from_sec1_bytes` が `signature::Error` を返すため、現在失敗しています。上流/根本原因を追跡します (クレートの現在のリリースでのカーブ パラメータの不一致が原因である可能性があります)。
- ハーネスは `sm2 v0.13.3` で正常にコンパイルされ、RustCrypto (またはパッチ適用されたフォーク) が付録の例 1 のポイント/署名ペアを受け入れると、自動回帰テストになります。
- OpenSSL/Tongsuo/gmssl 検証は、`sm_vectors.md` のコマンドで成功します。 LibreSSL (macOS のデフォルト) にはまだ SM2/SM3 サポートが不足しているため、ローカルギャップが生じます。

## 次のステップ

1. `sm2` が付録例 1 ポイントを受け入れる API を公開したら (またはアップストリームが曲線パラメータを確認した後)、ハーネスがローカルで通過できるようにしてから再テストします。
2. RustCrypto 修正が適用されるまで、正規の付録サンプルを保護するために、CI パイプラインで CLI 健全性チェック (OpenSSL/Tongsuo/gmssl) を維持します。
3. RustCrypto と OpenSSL の両方のパリティ チェックが成功した後、ハーネスを Iroha の回帰スイートにプロモートします。