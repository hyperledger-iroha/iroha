---
lang: pt
direction: ltr
source: docs/source/soranet/capability_registry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5d6b51fa1cb2cbe647a7a2d7c8c4e991a7d864b33c60238b16e99f848ff01d3a
source_last_modified: "2026-01-06T03:59:04.259785+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SoraNet Capability Registry
summary: Canonical mapping of `snnet.*` capability TLVs, identifiers, and adoption guidance.
---

This appendix tracks the negotiated capability TLVs used by the SoraNet handshake.  
Every entry lists the type code, payload layout, allowed values, and adoption notes.
Implementations MUST mirror these values exactly; new identifiers require governance
approval plus updated fixtures and downgrade tests.

## Capability TLVs

| Type (hex) | Label                   | Payload layout                                                             | Notes |
|-----------:|-------------------------|----------------------------------------------------------------------------|-------|
| `0x0101`   | `snnet.pqkem`           | `kem_id:u8` `flags:u8`                                                     | `flags & 0x01` denotes a *required* KEM; relays MUST honour required entries or abort. |
| `0x0102`   | `snnet.pqsig`           | `sig_id:u8` `flags:u8`                                                     | Multiple TLVs permitted when advertising dual-signature support. |
| `0x0103`   | `snnet.transcript_commit` | 32-byte SHA-256 digest                                                     | Binds directory-advertised capability manifests into the transcript. |
| `0x0201`   | `snnet.role`            | `role_bits:u8` (`0x01` guard · `0x02` middle · `0x04` exit)                | Exactly one entry per relay echo; absent for clients. |
| `0x0202`   | `snnet.padding`         | `u16` padded cell size (little-endian)                                     | Negotiated circuit padding bucket size (bytes). |
| `0x0203`   | `snnet.constant_rate`   | `version:u8`, `flags:u8`, `cell_bytes:u16` (LE), followed by a `cell_bytes`-long constant-rate envelope sample | Advertises SNNet-17A pacing support. The envelope sample MUST match the relay’s dummy cell encoding. |
| `0x7Fxx`   | GREASE fillers          | Arbitrary bytes                                                            | Emit ≥2 per message; parsers MUST preserve order and ignore contents. |

Except for `snnet.pqkem`, `snnet.pqsig`, and GREASE fillers, capability TLVs are
singletons and duplicates are rejected in v1.

## Algorithm identifier registries

### `snnet.pqkem`

| `kem_id` | Meaning              | Status / Guidance |
|---------:|----------------------|-------------------|
| `0x00`   | ML-KEM-512 (Kyber)   | Lightweight PQ profile for latency-sensitive/mobile peers. |
| `0x01`   | ML-KEM-768 (Kyber)   | Default PQ profile; SHOULD be marked required once SNNet-16 lands. |
| `0x02`   | ML-KEM-1024 (Kyber)  | High-security tier / governance circuits; expect larger frames. |

### `snnet.pqsig`

| `sig_id` | Meaning        | Status / Guidance |
|---------:|----------------|-------------------|
| `0x00`   | Ed25519        | Classical baseline (always available for interoperability). |
| `0x01`   | Dilithium3     | Preferred PQ signature; mark required when PQ-only transport is enforced. |
| `0x02`   | Falcon-512     | Optional profile for constrained deployments; governance-managed rollout. |

## Downgrade handling

- Clients MUST mark any capability that is non-negotiable with the required flag (`flags & 0x01`).
- Relays MUST abort the handshake (and emit downgrade telemetry) if they cannot echo a required capability.
- Absence of `snnet.pqkem` from a relay echo indicates classical-only support; PQ-preferring clients abort and raise alarms.
- Absence of `snnet.constant_rate` triggers a downgrade when clients request SNNet‑17A constant-rate transport. Fixture `snnet-cap-006-constant-rate` captures the warning slug and transcript hash; use it for regression in SDK harnesses.

## Change control

1. Propose additions via the SoraNet governance channel with updated fixtures, downgrade tests, and doc changes.
2. Update this registry, the handshake guide (`docs/source/soranet_handshake.md`), and any SDK manifests in the same PR.
3. Regenerate `iroha_crypto::soranet` fixtures (`cargo xtask soranet-fixtures`) so test vectors include the new identifiers.

Keep this document aligned with implementation changes to avoid mismatched capability negotiations across the network.
