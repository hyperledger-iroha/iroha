---
lang: ru
direction: ltr
source: docs/source/crypto/sm_rust_vector_check.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ce2f95b8b287c18c39232418333fbefdd300c030391be9dbfa4e29a3fd5f3e14
source_last_modified: "2026-01-03T18:07:57.109606+00:00"
translation_last_reviewed: 2026-01-30
---

//! Notes on verifying SM2 Annex D vectors using RustCrypto crates.

# SM2 Annex D Vector Verification (RustCrypto)

This walkthrough captures the steps we used to validate (and debug) the GM/T 0003 Annex D example with RustCrypto’s `sm2` crate. The canonical Annex Example 1 data (identity `ALICE123@YAHOO.COM`, message `"message digest"`, and the published `(r, s)`) is now recorded in `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml`. OpenSSL/Tongsuo/gmssl happily verify the signature (see `sm_vectors.md`), but RustCrypto’s `sm2 v0.13.3` still rejects the point with `signature::Error`, so CLI parity is confirmed while the Rust harness remains pending an upstream fix.

## Temporary crate

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

## Findings

- Verifying against the canonical Annex Example 1 `(r, s)` currently fails because `sm2::VerifyingKey::from_sec1_bytes` returns `signature::Error`; track upstream/root cause (likely due to curve-parameter mismatch in the crate’s current release).
- The harness compiles cleanly with `sm2 v0.13.3` and will become an automated regression test once RustCrypto (or a patched fork) accepts the Annex Example 1 point/signature pair.
- OpenSSL/Tongsuo/gmssl verification succeeds with the commands in `sm_vectors.md`; LibreSSL (macOS default) still lacks SM2/SM3 support, hence the local gap.

## Next steps

1. Re-test once `sm2` exposes an API that accepts the Annex Example 1 point (or after upstream confirms the curve parameters) so the harness can pass locally.
2. Keep a CLI sanity check (OpenSSL/Tongsuo/gmssl) in CI pipelines to guard the canonical Annex Example until the RustCrypto fix lands.
3. Promote the harness into Iroha’s regression suite after both RustCrypto and OpenSSL parity checks succeed.
