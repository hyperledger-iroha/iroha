---
lang: hy
direction: ltr
source: docs/source/soranet/blinded_cid_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1410a5ac472aaaeee439fa7f7fff7b4f5c6f2cafb4d9d7769dad89e6de6711f6
source_last_modified: "2025-12-29T18:16:36.182941+00:00"
translation_last_reviewed: 2026-02-07
title: SoraNet Blinded CID Research & Gateway Support
summary: Specification for SNNet-2a/SNNet-2 covering canonical CID blinding, salt ingestion, gateway headers, and follow-on work.
---

# Blinded CID Research & Gateway Support

## Goals & Scope
- Harden the blinded CID scheme so gateway requests no longer expose canonical manifest identifiers (SNNet-2a).
- Teach Torii gateways to resolve blinded lookups using the published SoraNet salt schedule while retaining deterministic storage (SNNet-2).
- Document follow-on work required for per-circuit/request blinding once the Noise handshake is threaded through the gateway.

## Canonical Blinding Scheme (SNNet-2a)
- The Salt Council publishes daily `SaltAnnouncementV1` payloads. Gateways load them from the directory configured via `sorafs_gateway.salt_schedule_dir`.
- The canonical blinded value is `BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)`.
- `iroha_crypto::soranet::blinding::canonical_cache_key` computes the digest. Additional helpers (`CircuitBlindingKey`, `RequestNonce`) are available for the future SoraNet handshake rollout.
- Unit tests lock the canonical hash output and verify the resolver implementation used by Torii.

## Gateway Behaviour (SNNet-2)
- Requests may include the following headers:
  - `Sora-Req-Blinded-CID`: URL-safe (no padding) base64 encoding of the canonical blinded digest.
  - `Sora-Req-Salt-Epoch`: decimal epoch identifier corresponding to the salt announcement.
  - `Sora-Req-Nonce`: reserved for future per-request derivations (currently validated as ASCII and otherwise ignored).
- Gateways maintain an in-memory cache keyed by `(epoch, blinded_cid)` and rebuild entries lazily from the on-disk manifest index. The resolver refuses unknown salt epochs with `428 Precondition Required`.
- To keep denial-of-service probes cheap to reject, gateways seed a per-epoch bloom filter from the manifest index; requests that miss the filter short-circuit without hitting disk while legitimate manifests continue to populate the filter as they arrive.
- Responses echo:
  - `Sora-Req-Blinded-CID` (when present in the request) so clients can verify the mapping.
  - `Sora-Content-CID`: canonical manifest identifier served by the gateway.
- Existing `x-sorafs-*` headers remain unchanged. If a canonical manifest identifier is still supplied in the path, the gateway ensures it matches the blinded lookup and returns `409 Conflict` on mismatches.

## Integration Notes
- `SorafsGateway` configuration now accepts `salt_schedule_dir`. When unset the gateway rejects blinded lookups with `503 Service Unavailable`.
- `iroha_torii::sorafs::BlindedCidResolver` caches successful resolutions and is exposed for integration tests.
- CLI/SDK tooling can compute blinding values using `iroha_crypto::soranet::blinding::canonical_cache_key`.
- `iroha_cli fetch` exposes `--blinded-cid`, `--salt-epoch`, and `--salt-hex` so developers can emit the new headers without crafting raw HTTP requests.

## Future Enhancements
- Thread the Noise hybrid handshake into Torii so gateways can derive `CircuitBlindingKey` instances per session and accept request-scoped values (`BLAKE3_keyed(key, "soranet.blinding.request.v1" ∥ nonce ∥ cid)`).
- Persist resolver caches across restarts and expose telemetry (`soranet_blinded_cid_hits_total`, `..._misses_total`).
- Extend OpenAPI definitions and SDKs with convenience helpers for the new headers.
- Enable governance policy enforcement for blinded-only mode (e.g., rejecting canonical identifiers when `anon-ok` is required).

This document tracks the first production-ready milestone where gateways accept blinded CIDs by header while keeping deterministic on-disk layout and Norito audit trails intact.
