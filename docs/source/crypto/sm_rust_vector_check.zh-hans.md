---
lang: zh-hans
direction: ltr
source: docs/source/crypto/sm_rust_vector_check.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ce2f95b8b287c18c39232418333fbefdd300c030391be9dbfa4e29a3fd5f3e14
source_last_modified: "2025-12-29T18:16:35.946190+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//！使用 RustCrypto crate 验证 SM2 附件 D 向量的注意事项。

# SM2 附件 D 矢量验证 (RustCrypto)

本演练介绍了我们使用 RustCrypto 的 `sm2` 包验证（和调试）GM/T 0003 附录 D 示例的步骤。规范附件示例 1 数据（身份 `ALICE123@YAHOO.COM`、消息 `"message digest"` 和已发布的 `(r, s)`）现在记录在 `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml` 中。 OpenSSL/Tongsuo/gmssl 愉快地验证签名（参见 `sm_vectors.md`），但 RustCrypto 的 `sm2 v0.13.3` 仍然拒绝 `signature::Error` 的点，因此 CLI 奇偶性得到确认，而 Rust 工具仍然等待上游修复。

## 临时板条箱

```bash
cargo new /tmp/sm2_verify --bin
cd /tmp/sm2_verify
```

`Cargo.toml`：

```toml
[package]
name = "sm2_verify"
version = "0.1.0"
edition = "2024"

[dependencies]
hex = "0.4"
sm2 = "0.13.3"
```

`src/main.rs`：

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

## 调查结果

- 根据规范附件示例 1 `(r, s)` 进行验证当前失败，因为 `sm2::VerifyingKey::from_sec1_bytes` 返回 `signature::Error`；跟踪上游/根本原因（可能是由于板条箱当前版本中的曲线参数不匹配）。
- 该工具可以使用 `sm2 v0.13.3` 进行干净的编译，一旦 RustCrypto（或修补后的分叉）接受附件示例 1 点/签名对，它将成为自动回归测试。
- 使用 `sm_vectors.md` 中的命令 OpenSSL/Tongsuo/gmssl 验证成功； LibreSSL（macOS 默认）仍然缺乏 SM2/SM3 支持，因此存在本地差距。

## 后续步骤

1. 一旦 `sm2` 公开接受附件示例 1 点的 API（或在上游确认曲线参数后），请重新测试，以便线束可以在本地通过。
2. 在 CI 管道中保持 CLI 健全性检查 (OpenSSL/Tongsuo/gmssl)，以保护规范的附件示例，直到 RustCrypto 修复落地。
3. 在 RustCrypto 和 OpenSSL 奇偶校验成功后，将工具提升到 Iroha 的回归套件中。