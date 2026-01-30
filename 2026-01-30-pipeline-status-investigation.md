# Pipeline Status Polling Investigation

## Problem

`HttpClientTransport.waitForTransactionStatus()` hangs indefinitely against vanilla Iroha.
The SDK polls `GET /v1/pipeline/transactions/status?hash=<hash>` but the `CompletableFuture`
returned by `waitForTransactionStatus` never completes.

## Root Causes Found

### Bug 1: Hash Computation Mismatch (PRIMARY) -- SOLVED

The SDK computes a transaction hash that **does not match** Iroha's internal hash.
When the SDK queries the status endpoint with its hash, Iroha returns 404 ("not found").

**Root cause:** Iroha hashes the `TransactionEntrypoint::External` enum wrapper around
the signed transaction, not the signed transaction directly.

**Iroha's hash computation** (from `iroha_data_model/src/transaction/signed.rs`):
```rust
pub fn hash(&self) -> HashOf<Self> {
    let entry_hash = self.hash_as_entrypoint();
    HashOf::from_untyped_unchecked(Hash::from(entry_hash))
}

pub fn hash_as_entrypoint(&self) -> HashOf<TransactionEntrypoint> {
    HashOf::new(&TransactionEntrypoint::External(self.clone()))
}
```

`TransactionEntrypoint` is an enum:
```rust
pub enum TransactionEntrypoint {
    External(SignedTransaction),      // variant 0
    Time(TimeTriggerEntrypoint),      // variant 1
}
```

The Norito encoding of `TransactionEntrypoint::External(tx)` is:
```
u32_LE(0)                          // enum discriminant (4 bytes)
u64_LE(signedTxEncoding.length)    // length prefix (8 bytes)
signedTxEncoding                   // SignedTransactionEncoder.encode() output
```

Then: `Blake2b-256(above bytes) with LSB=1`

**Evidence from `hash mismatch investigation` test:**

| Method | Hash | Matches Iroha? |
|--------|------|----------------|
| SDK current (bare encoding) | `fcb95a30261aa956...` | NO |
| Versioned (0x01 prefix) | `ed3372a0022a8036...` | NO |
| **Entrypoint (u32+u64+payload)** | **`fbb42ebf64c46ff2...`** | **YES** |
| Payload only | `85c23c8be0935111...` | NO |
| Entrypoint compact len | `95801454c3d76c4f...` | NO |

Iroha log: `transaction accepted by Torii; enqueuing tx=fbb42ebf64c46ff23931150ec88c372287f68b9eb17c314145d54673c3eab547`

**Fix:** In `SignedTransactionHasher.canonicalBytes()`, prepend the `TransactionEntrypoint::External`
enum wrapper (4-byte u32 discriminant + 8-byte u64 length prefix) before the encoded signed
transaction bytes.

### Bug 2: Norito Response Parsing Crash (SECONDARY)

When the status endpoint returns a 404, the response body is Norito-encoded binary
(`Content-Type: application/x-norito`, body starts with `NRT0` magic bytes).

The SDK's `parsePipelineStatusPayload()` at `HttpClientTransport.java:822-835` assumes
JSON and calls `JsonParser.parse()`, which throws `IllegalStateException("Invalid number:
expected digit")` on the binary NRT0 data.

This exception is thrown inside a `whenComplete` handler (line 699). The exception is
**silently swallowed** by the CompletableFuture contract -- `whenComplete` handler exceptions
propagate to the return value of `whenComplete()`, but nobody holds that reference. The
separate `future` variable (the one returned to the caller) is never completed.

**Result:** `join()` or `get()` on the returned future hangs forever.

**Fix needed:** Two changes:
1. Wrap the `whenComplete` handler body in try-catch and call
   `future.completeExceptionally(e)` on any unhandled exception (safety net)
2. In `parsePipelineStatusPayload`, detect NRT0 magic bytes and return null
   (treat as "no parseable status") instead of attempting JSON parse

## Diagnostic Test Evidence

### Status endpoint responses:

| Hash type | HTTP Status | Content-Type | Body | SDK behavior |
|-----------|-------------|--------------|------|-------------|
| Dummy (non-zero) | 400 | `application/x-norito` | 103 bytes, error msg | SDK catches 400, future completes exceptionally |
| Real (SDK hash) | 404 | `application/x-norito` | 56 bytes, NRT0 binary | SDK allows 404 through -> `parsePipelineStatusPayload` throws -> `whenComplete` swallows -> **HANG** |
| Real (Iroha hash) | Not tested yet | - | - | Expected: status data or "committed" |

### Observer callback:
Never called. The exception fires at line 729 (`parsePipelineStatusPayload`), which is
**before** the observer invocation at line 743.

## Fixes Applied

All three fixes have been implemented and committed.

### Fix 1: `SignedTransactionHasher.canonicalBytes()` (PRIMARY)

Modified `canonicalBytes()` to wrap the encoded signed transaction in the
`TransactionEntrypoint::External` enum encoding: `u32_LE(0) + u64_LE(len) + payload`.
Patch: [`docs/patches/01-fix-hash-entrypoint-wrapper.patch`](../patches/01-fix-hash-entrypoint-wrapper.patch)

### Fix 2: `parsePipelineStatusPayload()` (SECONDARY)

Added NRT0 magic bytes detection — returns null instead of attempting JSON parse
on Norito binary responses.
Patch: [`docs/patches/02-fix-norito-response-parsing.patch`](../patches/02-fix-norito-response-parsing.patch)

### Fix 3: Exception safety in `pollPipelineStatus()` (DEFENSIVE)

Wrapped the `whenComplete` handler body in try-catch, completing the future
exceptionally on any unhandled exception.
Patch: [`docs/patches/03-fix-whencomplete-exception-safety.patch`](../patches/03-fix-whencomplete-exception-safety.patch)

## Files

| File | Role |
|------|------|
| `HttpClientTransport.java:672-835` | `pollPipelineStatus` + `parsePipelineStatusPayload` — fixed |
| `SignedTransactionHasher.java` | SDK hash computation — fixed |
| `SignedTransactionEncoder.java` | Transaction encoding for hashing |
| `TransferWirePayloadEncoder.java` | Asset transfer instruction encoding — **custom, not in upstream SDK** |
| `IrohaHash.java` | Blake2b-256 with LSB=1 prehash |
| `ToriiRequestBuilder.java:45-62` | Status endpoint URL construction |
| `PipelineStatusJvmTest.kt` | E2E test asserting transfer reaches success status |
| `iroha_data_model/src/transaction/signed.rs` | Iroha Rust hash reference implementation |

## Files Requiring Manual Restoration After SDK Sync

Syncing the iroha3 module from upstream repositories replaces its contents.
The following files are **not part of the upstream SDK** and must be manually
restored (via `git checkout` or re-added) after each sync:

| File | Why it's custom |
|------|-----------------|
| `TransferWirePayloadEncoder.java` | Encodes `iroha.transfer` wire instructions with Norito framing. Upstream SDK has no transfer instruction builder. |
| `SignedTransactionHasher.java` (entrypoint wrapper) | The `TransactionEntrypoint::External` wrapping in `canonicalBytes()` is a local fix not yet upstreamed. Apply `docs/patches/01-fix-hash-entrypoint-wrapper.patch` |
| `HttpClientTransport.java` (NRT0 detection + try-catch) | Local fixes for Norito response handling and exception safety. Apply `docs/patches/02-fix-norito-response-parsing.patch` and `docs/patches/03-fix-whencomplete-exception-safety.patch` |

## Iroha Source References

- `signed.rs:440-451`: `SignedTransaction::hash()` and `hash_as_entrypoint()` methods
- `signed.rs:183-187`: `TransactionEntrypoint` enum definition
- `hash.rs:59-69`: `Hash::new()` -- Blake2b-256 computation
- `hash.rs:39-48`: `Hash::prehashed()` -- LSB=1 marker
- `norito_derive/src/lib.rs:2852-2874`: Enum encoding (u32 discriminant)
