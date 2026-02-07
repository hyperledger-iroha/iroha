---
lang: ka
direction: ltr
source: docs/source/crypto/sm_vectors.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd46471945188bcb95c8ee411c48acc8915a92b408df196caa65bf25f0596732
source_last_modified: "2026-01-05T18:22:23.402400+00:00"
translation_last_reviewed: 2026-02-07
---

//! Reference test vectors for SM2/SM3/SM4 integration work.

# SM Vectors Staging Notes

This document aggregates publicly available known-answer tests that seed the SM2/SM3/SM4 harnesses before the automated import scripts land. Machine-readable copies live in:

- `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml` (annex vectors, RFC 8998 cases, Annex Example 1).
- `fixtures/sm/sm2_fixture.json` (shared deterministic SDK fixture consumed by Rust/Python/JavaScript tests).
- `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json` — curated 52-case corpus (deterministic fixtures + synthesized bit-flip/message/tail truncation negatives) mirrored under Apache-2.0 while the upstream SM2 suite is pending. `crates/iroha_crypto/tests/sm2_wycheproof.rs` verifies these vectors using the standard SM2 verifier when possible and falls back to a pure BigInt implementation of the Annex domain when required.

## SM2 Signature Verification Against OpenSSL / Tongsuo / GmSSL

Annex Example 1 (Fp-256) uses the identity `ALICE123@YAHOO.COM` (ENTLA 0x0090), message `"message digest"`, and the public key shown below. A copy‑and‑paste OpenSSL/Tongsuo workflow is:

```bash
# 1. Public key (SubjectPublicKeyInfo)
cat > pubkey.pem <<'PEM'
-----BEGIN PUBLIC KEY-----
MFkwEwYHKoZIzj0CAQYIKoEcz1UBgi0DQgAECuTHeYqg8RlHG+4RglvkYgK7eeKlhESV6XwE/03y
VIp8AkD4jxzU4WNSpzwXt/FvBzU+U6F21oSp/gxrt5joVw==
-----END PUBLIC KEY-----
PEM

# 2. Message (no trailing newline)
printf "message digest" > msg.bin

# 3. Signature (DER form)
cat > sig.b64 <<'EOF'
MEQCIEDx7Fn3k9n0ngnc70kTDUGU95+x7tLKpVus20nE51XRAiBvxtrDLF1c8Qx337IPfC62Z6RX
hy+wnsVjJ6Z+x97r5w==
EOF
base64 -d sig.b64 > sig.der

# 4. Verify (expects "Signature Verified Successfully")
openssl pkeyutl -verify -pubin -inkey pubkey.pem \
  -in msg.bin -sigfile sig.der \
  -rawin -digest sm3 \
  -pkeyopt distid:ALICE123@YAHOO.COM
```

* OpenSSL 3.x documents the `distid:` / `hexdistid:` options. Some OpenSSL 1.1.1 builds expose the knob as `sm2_id:`—use whichever appears in `openssl pkeyutl -help`.
* GmSSL exports the same `pkeyutl` surface; older builds also accepted `-pkeyopt sm2_id:...`.
* LibreSSL (default on macOS/OpenBSD) does **not** implement SM2/SM3, so the command above fails there. Use OpenSSL ≥ 1.1.1, Tongsuo, or GmSSL.

The DER helper emits `3044022040F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D102206FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7`, which matches the annex signature.

The annex also prints the user-information hash and the resulting digest:

```
ZA = F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A
e  = SM3(ZA || "message digest")
   = B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76
```

You can confirm with OpenSSL:

```bash
echo -n 'F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A' \
  | xxd -r -p > za.bin
printf "message digest" >> za.bin
openssl dgst -sm3 -binary za.bin | xxd -p -c 256
# -> b524f552cd82b8b028476e005c377fb19a87e6fc682d48bb5d42e3d9b9effe76
```

Python sanity check for the curve equation:

```python
p = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFF", 16)
a = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC", 16)
b = int("28E9FA9E9D9F5E344D5A9E4BCF6509A7F39789F515AB8F92DDBCBD414D940E93", 16)
x = int("0AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A", 16)
y = int("7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857", 16)
assert (pow(y, 2, p) - (pow(x, 3, p) + a * x + b)) % p == 0
```

## SM3 Hash Vectors

| Input | Hex Encoding | Digest (hex) | Source |
|-------|--------------|--------------|--------|
| `""` (empty string) | `""` | `1ab21d8355cfa17f8e61194831e81a8f22bec8c728fefb747ed035eb5082aa2b` | GM/T 0004-2012 Annex A.1 |
| `"abc"` | `616263` | `66c7f0f462eeedd9d1f2d46bdc10e4e24167c4875cf2f7a2297da02b8f4ba8e0` | GM/T 0004-2012 Annex A.2 |
| `"abcd"` repeated 16 times (64 bytes) | `61626364` ×16 | `debe9ff92275b8a138604889c18e5a4d6fdb70e5387e5765293dcba39c0c5732` | GB/T 32905-2016 Annex A |

## SM4 Block Cipher (ECB) Vectors

| Key (hex) | Plaintext (hex) | Ciphertext (hex) | Source |
|-----------|-----------------|------------------|--------|
| `0123456789abcdeffedcba9876543210` | `0123456789abcdeffedcba9876543210` | `681edf34d206965e86b3e94f536e4246` | GM/T 0002-2012 Annex A.1 |
| `0123456789abcdeffedcba9876543210` | `000102030405060708090a0b0c0d0e0f` | `59b50808d3dcf921fa30b5b3c1dddc19` | GM/T 0002-2012 Annex A.2 |
| `0123456789abcdeffedcba9876543210` | `ffeeddccbbaa99887766554433221100` | `1c3b3f56186b70819d3f5aa11fe2c8b6` | GM/T 0002-2012 Annex A.3 |

## SM4-GCM Authenticated Encryption

| Key | IV | AAD | Plaintext | Ciphertext | Tag | Source |
|-----|----|-----|-----------|------------|-----|--------|
| `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `d9313225f88406e5a55909c5aff5269a` | `42831ec2217774244b7221b784d0d49c` | `4d5c2af327cd64a62cf35abd2ba6fab4` | RFC 8998 Appendix A.2 |

## SM4-CCM Authenticated Encryption

| Key | Nonce | AAD | Plaintext | Ciphertext | Tag | Source |
|-----|-------|-----|-----------|------------|-----|--------|
| `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `202122232425262728292a2b2c2d2e2f` | `7162015b4dac2555` | `4d26de5a` | RFC 8998 Appendix A.3 |

### Wycheproof Negative Cases (SM4 GCM/CCM)

These cases inform the regression suite in `crates/iroha_crypto/tests/sm3_sm4_vectors.rs`. Each case must fail verification.

| Mode | TC ID | Description | Key | Nonce | AAD | Ciphertext | Tag | Notes |
|------|-------|-------------|-----|-------|-----|------------|-----|-------|
| GCM | 1 | Tag bit flip | `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `42831ec2217774244b7221b784d0d49c` | `5d5c2af327cd64a62cf35abd2ba6fab4` | Wycheproof-derived invalid tag |
| CCM | 17 | Tag bit flip | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de5a` | Wycheproof-derived invalid tag |
| CCM | 18 | Truncated tag (3 bytes) | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de` | Ensures short tags fail authentication |
| CCM | 19 | Ciphertext bit flip | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2554` | `5d26de5a` | Detect tampered payload |

## SM2 Deterministic Signature Reference

| Field | Value (hex unless noted) | Source |
|-------|--------------------------|--------|
| Curve parameters | `sm2p256v1` (a = `FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC`, etc.) | GM/T 0003.5-2012 Annex A |
| User ID (`distid`) | ASCII `"ALICE123@YAHOO.COM"` (ENTLA 0x0090) | GM/T 0003 Annex D |
| Public key | `040AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857` | GM/T 0003 Annex D |
| Message | `"message digest"` (hex `6d65737361676520646967657374`) | GM/T 0003 Annex D |
| ZA | `F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A` | GM/T 0003 Annex D |
| `e = SM3(ZA || M)` | `B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76` | GM/T 0003 Annex D |
| Signature `(r,s)` | `40F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D1`, `6FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7` | GM/T 0003 Annex D |
| Multicodec (provisional) | `8626550012414C494345313233405941484F4F2E434F4D040AE4…` (`sm2-pub`, varint `0x1306`) | Derived from Annex Example 1 |
| Prefixed multihash | `sm2:8626550012414C494345313233405941484F4F2E434F4D040AE4…` | Derived (matches `sm_known_answers.toml`) |

SM2 multihash payloads are encoded as `distid_len (u16 BE) || distid bytes || SEC1 uncompressed key (65 bytes)`.

### Rust SDK Deterministic Signing Fixture (SM-3c)

The structured vectors array includes the Rust/Python/JavaScript parity payload
so every client signs the same SM2 message with a shared seed and
distinguishing identifier.

| Field | Value (hex unless noted) | Notes |
|-------|--------------------------|-------|
| Distinguishing ID | `"iroha-sdk-sm2-fixture"` | Shared across Rust/Python/JS SDKs |
| Seed | `"iroha-rust-sdk-sm2-deterministic-fixture"` (hex `69726F68612D727573742D73646B2D736D322D64657465726D696E69737469632D66697874757265`) | Input to `Sm2PrivateKey::from_seed` |
| Private key | `E64AE95930A2324DCF11BB1657FBBC2CD2BEF7515664BE7A15CA6AE5BCE0B7CA` | Derived deterministically from the seed |
| Public key (SEC1 uncompressed) | `0494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | Matches deterministic derivation |
| Public key multihash | `862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | Output of `PublicKey::to_string()` |
| Prefixed multihash | `sm2:862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | Output of `PublicKey::to_prefixed_string()` |
| ZA | `6B0229C3FBFE7A5DC6EE27228E18E512752DCCE9191EB381242A7342D568EDDE` | Computed via `Sm2PublicKey::compute_z` |
| Message | `"Rust SDK SM2 signing fixture v1"` (hex `527573742053444B20534D32207369676E696E672066697874757265207631`) | Canonical payload for SDK parity tests |
| Signature `(r, s)` | `4E026F2E0FB75A6418C95C046D70DB7B95558BE30FB2001906EBE2091FA1AF76`, `299CFF374026D9E0C49C6BD4A99A1473BA17EFF429B3FAD083B1577F7559BDF5` | Produced via deterministic signing |

- Cross-SDK consumption:
  - `fixtures/sm/sm2_fixture.json` now exposes a `vectors` array. The Rust crypto regression suite (`crates/iroha_crypto/tests/sm2_fixture_vectors.rs`), Rust client helpers (`crates/iroha/src/sm.rs`), Python bindings (`python/iroha_python/tests/test_crypto.py`), and JavaScript SDK (`javascript/iroha_js/test/crypto.sm2.fixture.test.js`) all parse these fixtures.
  - `crates/iroha/tests/sm_signing.rs` exercises deterministic signing and verifies that the on-chain multihash/multicodec outputs match the fixture.
  - Admission-time regression suites (`crates/iroha_core/tests/admission_batching.rs`) assert SM2 payloads are rejected unless `allowed_signing` includes `sm2` *and* `default_hash` is `sm3-256`, covering the configuration constraints end-to-end.
- 追加カバレッジ: 異常系（無効な曲線、異常な `r/s`、`distid` 改ざん）は `crates/iroha_crypto/tests/sm2_fuzz.rs` の property テストで網羅済みです。Annex Example 1 の正規ベクトルは `sm_known_answers.toml` に多言語対応の multicodec 形式で引き続き提供しています。
- Rust code now exposes `Sm2PublicKey::compute_z` so ZA fixtures can be generated programmatically; see `sm2_compute_z_matches_annex_example` for the Annex D regression.

## Next Actions
- Monitor admission-time regressions (`admission_batching.rs`) to ensure config gating continues to enforce SM2 enablement boundaries.
- Extend coverage with Wycheproof SM4 GCM/CCM cases and derive property-based fuzz targets for SM2 signature verification. ✅ (Invalid-case subset captured in `sm3_sm4_vectors.rs`).
- LLM prompt for alternative IDs: *"Provide the SM2 signature for Annex Example 1 when the distinguishing ID is set to 1234567812345678 instead of ALICE123@YAHOO.COM, and outline the new ZA/e values."*
