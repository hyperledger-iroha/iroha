# Iroha SDK Compatibility Analysis

**Date**: 2026-01-21
**Iroha Repo**: `/Users/sdi/work/iroha`
**Android Repo**: `/Users/sdi/work/laos-android-offline-payments`

## Overview

This document summarizes the compatibility differences between the upstream Iroha Java SDK (in `i23` and `i23_local` branches) and our local Android SDK snapshot.

## Branch Comparison: i23 vs i23_local

### Commit Status

| Branch | Unique Commits |
|--------|----------------|
| i23_local | +2 commits ("fix for android transfers") |
| i23 | +11 commits (consensus, identity refactor, etc.) |

### Key Differences

#### ToriiRequestBuilder.java - Encoding Approaches

| Aspect | i23 | i23_local |
|--------|-----|-----------|
| Content-Type | `application/x-norito` | `application/json` |
| Body Encoding | Binary norito | JSON with Base64 fields |
| Accept Header | `application/x-norito, application/json` | `application/json` |

#### Rust-side Fix (const_vec.rs)

Present only in **i23_local**:

```rust
// Before (i23): expects NRT0 header
norito::codec::decode_adaptive::<Vec<T>>(bytes)

// After (i23_local): streaming decode without NRT0 header
let mut cursor = std::io::Cursor::new(bytes);
<Vec<T> as norito::codec::Decode>::decode(&mut cursor)
```

This fix is critical for decoding instruction payloads from Android SDK.

## Our Local Fixes (laos-android-offline-payments)

### 1. Ed25519Signer.java - Blake2b Hashing (Critical)

**Problem**: Upstream SDK signs raw message directly, but Iroha expects Blake2b-256 hash with LSB set to 1.

**Our Fix**:
```java
// Sign the Blake2b-256 hash, not raw payload
final byte[] hash = blake2bHashWithLsb(message);
signature.update(hash);
return signature.sign();

private static byte[] blake2bHashWithLsb(final byte[] data) {
    final Blake2bDigest digest = new Blake2bDigest(256);
    digest.update(data, 0, data.length);
    final byte[] hash = new byte[digest.getDigestSize()];
    digest.doFinal(hash, 0);
    // Rust sets LSB to 1 (see Hash::prehashed in iroha_crypto)
    hash[hash.length - 1] = (byte) (hash[hash.length - 1] | 1);
    return hash;
}
```

**Evidence**: Rust code in `crates/iroha_crypto/src/hash.rs`:
```rust
pub fn prehashed(mut hash: [u8; Self::LENGTH]) -> Self {
    hash[Self::LENGTH - 1] |= 1;
    // ...
}
```

### 2. ToriiRequestBuilder.java - Endpoint Change

| Aspect | Upstream (both branches) | Our Local |
|--------|--------------------------|-----------|
| SUBMIT_PATH | `/v1/pipeline/transactions` | `/transaction` |
| Content-Type | varies | `application/x-norito` |
| Accept | varies | `application/json` |

### 3. extractIh58Address() - REST API Compatibility

New Iroha API rejects `@domain` suffix in URL paths. Added helper to extract IH58 address:

```kotlin
fun extractIh58Address(accountId: AccountId): String {
    val value = accountId.toString()
    val atIndex = value.lastIndexOf('@')
    require(atIndex > 0) { "Invalid account ID format: $value" }
    return value.substring(0, atIndex)
}
```

### 4. AccountId Struct Encoding

Updated `TransactionPayloadAdapter` to encode authority as AccountId struct (domain + controller) instead of plain string.

## Comparison Matrix

| Fix | i23 | i23_local | Our Local | Status |
|-----|-----|-----------|-----------|--------|
| Ed25519 Blake2b hash | ❌ | ❌ | ✅ | **Critical - needs upstream** |
| `/transaction` endpoint | ❌ | ❌ | ✅ | May need upstream |
| ConstVec decode fix | ❌ | ✅ | N/A | Rust-side, merge to i23 |
| extractIh58Address | ❌ | ❌ | ✅ | New API compatibility |
| AccountId struct encoding | ? | ? | ✅ | Needs verification |

## Recommendations

1. **Merge i23_local to i23**: The ConstVec decode fix is required for Android compatibility
2. **Upstream Ed25519Signer fix**: The Blake2b hashing is critical for signature verification
3. **Clarify endpoint**: Determine if `/transaction` or `/v1/pipeline/transactions` is the correct endpoint
4. **Sync SDK**: After upstream fixes are merged, run `/sync-iroha-sdk` to update local snapshot

## Related Commits

### Iroha Repo (i23_local)
- `d8b8ab5b9` - fix for android transfers (ensure_domain_matches visibility)
- `ed4cc7e4d` - fix for android transfers (ConstVec decode)

### laos-android-offline-payments
- `e421d06f7` - Fix Iroha SDK compatibility with new REST API
- `0fd6accb9` - Fix P2P transfer E2E test key injection and signing
- `2b5b7190f` - Fix P2P transfer signature verification and norito encoding
