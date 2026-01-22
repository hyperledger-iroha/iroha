# Iroha SDK Patches

Patches for upstream iroha repository to fix Android SDK compatibility issues.

## Patch Files

### Java SDK Patches (apply to `java/iroha_android/`)

| Patch | Description | Priority |
|-------|-------------|----------|
| `0001-ed25519-signer-blake2b-hash.patch` | Add Blake2b-256 hashing with LSB=1 to Ed25519Signer | **Critical** |
| `0002-torii-request-builder-transaction-endpoint.patch` | Use `/transaction` endpoint instead of `/v1/pipeline/transactions` | High |

### Rust Patches (from i23_local branch)

| Patch | Description | Priority |
|-------|-------------|----------|
| `0003-rust-const-vec-decode-fix.patch` | Fix ConstVec decoding to use streaming decode without NRT0 header | **Critical** |
| `0004-rust-ensure-domain-matches-public.patch` | Make `ensure_domain_matches` public | Low |

## How to Apply

### Java SDK Patches

```bash
cd /path/to/iroha
git checkout i23

# Apply Ed25519Signer fix (critical)
git apply /path/to/patches/0001-ed25519-signer-blake2b-hash.patch

# Apply endpoint fix
git apply /path/to/patches/0002-torii-request-builder-transaction-endpoint.patch
```

### Rust Patches

These patches are already in `i23_local` branch. To merge them to `i23`:

```bash
cd /path/to/iroha
git checkout i23
git cherry-pick ed4cc7e4d  # const_vec decode fix
git cherry-pick d8b8ab5b9  # ensure_domain_matches visibility
```

Or apply the patch files:

```bash
git apply /path/to/patches/0003-rust-const-vec-decode-fix.patch
git apply /path/to/patches/0004-rust-ensure-domain-matches-public.patch
```

## Technical Details

### 0001: Ed25519Signer Blake2b Hash

**Problem**: Iroha's Rust implementation signs `Blake2b-256(payload)` with LSB set to 1, but the Java SDK signs the raw payload.

**Solution**: Hash the payload with Blake2b-256 and set LSB to 1 before signing.

**Evidence**: `crates/iroha_crypto/src/hash.rs`:
```rust
pub fn prehashed(mut hash: [u8; Self::LENGTH]) -> Self {
    hash[Self::LENGTH - 1] |= 1;
    // ...
}
```

### 0002: Transaction Endpoint

**Problem**: New Iroha API uses `/transaction` endpoint, not `/v1/pipeline/transactions`.

**Solution**: Change `SUBMIT_PATH` constant.

### 0003: ConstVec Decode Fix

**Problem**: `decode_adaptive()` expects NRT0 header, but instruction bytes don't have it.

**Solution**: Use streaming decode with current flags context.

### 0004: ensure_domain_matches Visibility

**Problem**: `ensure_domain_matches` was `pub(crate)`, limiting external validation.

**Solution**: Make it `pub`.

## Verification

After applying patches, run the P2P transfer test:

```bash
cd /path/to/laos-android-offline-payments
IROHA_DIR=/path/to/iroha ./gradlew --quiet :core:iroha:iroha3:test \
    --tests "org.hyperledger.iroha.android.P2PTransferJvmTest"
```
