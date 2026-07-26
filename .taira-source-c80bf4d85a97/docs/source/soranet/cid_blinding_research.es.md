---
lang: es
direction: ltr
source: docs/source/soranet/cid_blinding_research.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6c262e28e4d76572505d4d076cf9935ef82ddafccf317cc78ab37f176af0982f
source_last_modified: "2026-01-03T18:08:01.351289+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SoraNet CID Blinding Research Notes
summary: Evaluation of the per-circuit CID blinding strategy adopted for SNNet-2a.
---

# CID Blinding Research (SNNet-2a)

## Motivation

Popular manifest CIDs were previously blinded as `BLAKE3(salt ∥ cid)` using the
daily salt published by the Salt Council. While this blocks long-term replay
correlation, a passive observer watching multiple clients during the same epoch
could still spot that the same position in the cache was requested repeatedly.
SNNet-2a explores per-request randomness that keeps gateway storage
deterministic but makes cross-circuit correlation impractical.

## Design Summary

- The Salt Council still publishes a 32-byte salt per epoch via
  `SaltAnnouncementV1`.
- Gateways now accept the `Sora-Req-Blinded-CID` header containing
  `BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)` encoded with URL-safe,
  no-pad Base64. The salt epoch is advertised through the
  `Sora-Req-Salt-Epoch` header.
- Responses echo both `Sora-Req-Blinded-CID` (when supplied) and the canonical
  manifest identifier via `Sora-Content-CID`, allowing clients to verify gateway
  behaviour.
- `iroha_crypto::soranet::blinding` also exposes `CircuitBlindingKey` helpers
  for per-circuit and per-request derivations. These APIs remain available for
  the SoraNet handshake rollout, but the gateway currently ships the canonical
  (`salt ∥ cid`) variant while the Noise transport is wired up.

## Prototype & Evaluation

`iroha_crypto` includes targeted unit tests covering the new primitives:

- `canonical_cache_key_matches_reference` locks the canonical digest against a
  known hex output to prevent accidental domain changes.
- `resolver_matches_manifest_by_blinded_cid` exercises the gateway-side resolver
  that maps `Sora-Req-Blinded-CID` values back to canonical manifests.
- The higher-level `CircuitBlindingKey` tests remain in place so the handshake
  rollout can adopt per-circuit/request derivations without regressing coverage.

Developers can experiment with the API directly:

```rust
use iroha_crypto::soranet::blinding::canonical_cache_key;

let epoch_salt = [0x11; 32];
let cid = hex::decode("bafkreigh2akiscaildc4p").unwrap(); // canonical CID bytes

let cache_key = canonical_cache_key(&epoch_salt, &cid);

println!("cache={:x?}", cache_key.as_bytes());
```

For quick experiments the CLI exposes `--salt-hex`/`--salt-epoch` (or `--blinded-cid`) on `iroha_cli fetch`, which computes the same digest and emits the required headers automatically.

Clients that already participate in the Noise handshake can extend the example
with `CircuitBlindingKey` to derive per-circuit/request identifiers; the gateway
will begin honouring those values once the SoraNet session manager threads the
handshake secret into the resolver.

```rust
use iroha_crypto::soranet::blinding::{CircuitBlindingKey, RequestNonce};

let shared_secret = [0x07; 32]; // Output of the Noise hybrid handshake
let circuit_key = CircuitBlindingKey::derive(&epoch_salt, &shared_secret)?;
let nonce = RequestNonce::random(&mut rand::thread_rng());
let request_key = circuit_key.request_scoped_blinded(&cid, &nonce);
# Ok::<(), iroha_crypto::soranet::blinding::BlindingError>(())
```

The gateway stores content against `cache_key`, while overlay requests use the
same value for the initial rollout. Once Noise secrets are available the
nonce-bound digest can be passed over `Sora-Req-Blinded-CID` transparently.

## Next Steps

- Thread `CircuitBlindingKey` through the SoraNet exit gateway so Torii
  endpoints accept request-scoped digests derived from the Noise handshake.
- Extend integration test fixtures (`tests/interop/soranet`) to replay captured
  circuits with randomized nonces.
- Capture counter metrics (`soranet_request_blinded_total`) once the gateway
  accepts the new envelope to monitor adoption during the rollout.
