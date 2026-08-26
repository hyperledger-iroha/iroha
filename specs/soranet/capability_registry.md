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
| `0x0102`   | `snnet.pqsig`           | `sig_id:u8` `flags:u8`                                                     | Algorithm identifiers MUST be unique; v1 accepts exactly one Dilithium3 (`0x01`) entry. |
| `0x0103`   | `snnet.transcript_commit` | 32-byte SHA-256 digest                                                     | Binds directory-advertised capability manifests into the transcript. |
| `0x0104`   | `snnet.suite_list`      | Non-empty ordered `u8` handshake-suite identifiers; bit `0x80` of the first byte is the suite-list required flag | Negotiates the Noise pattern before transport setup. |
| `0x0201`   | `snnet.role`            | nonzero `role_bits:u8` (`0x01` guard · `0x02` middle · `0x04` exit)         | Exactly one entry per relay echo; reserved bits are rejected; absent for clients. |
| `0x0202`   | `snnet.padding`         | `u16` padded cell size (little-endian)                                     | Negotiated circuit padding bucket size (bytes). |
| `0x0203`   | `snnet.constant_rate`   | exactly 4 bytes: `version:u8`, `flags:u8`, `cell_bytes:u16` (LE) | Advertises SNNet-17A pacing support. Version 1 requires `cell_bytes = 1024`. |
| `0x7Fxx`   | GREASE fillers          | Arbitrary bytes                                                            | Emit ≥2 per message; parsers MUST preserve order and ignore contents. |

Except for `snnet.pqkem`, `snnet.pqsig`, and GREASE fillers, capability TLVs are
singletons and duplicates are rejected in v1. Repeated `snnet.pqkem` or
`snnet.pqsig` TLVs must carry distinct algorithm IDs; repeating an ID is
rejected even when its flags differ. For `snnet.pqkem`, `snnet.pqsig`, and
`snnet.constant_rate`, bit `0x01` is the only defined flag;
every reserved flag bit is rejected. Implementations reject malformed payload
lengths and every unknown non-GREASE type rather than treating it as an
extension. The first-release `snnet.suite_list` accepts only unique `0x04` and
`0x05` identifiers; any unknown, retired, or duplicate identifier rejects the
entire list. Clients encode TLVs in nondecreasing type order, and the complete
encoded capability vector is limited to 4,096 bytes.

## Algorithm identifier registries

### `snnet.pqkem`

| `kem_id` | Meaning              | Status / Guidance |
|---------:|----------------------|-------------------|
| `0x00`   | ML-KEM-512 (Kyber)   | Lightweight PQ profile for latency-sensitive/mobile peers. |
| `0x01`   | ML-KEM-768 (Kyber)   | Default PQ profile; marked required in the first-release default policy. |
| `0x02`   | ML-KEM-1024 (Kyber)  | High-security tier / governance circuits; expect larger frames. |

### `snnet.pqsig`

| `sig_id` | Meaning        | Status / Guidance |
|---------:|----------------|-------------------|
| `0x00`   | Ed25519        | Rejected as `pqsig` in the first-release wire protocol. |
| `0x01`   | Dilithium3     | The only accepted first-release `pqsig` policy identifier. |
| `0x02`   | Falcon-512     | Rejected in the first-release wire protocol. |

`snnet.pqsig` is transcript-bound handshake policy and compatibility metadata,
not certificate metadata or an online signature-algorithm selector. Online
relay authentication unconditionally carries both Ed25519 and ML-DSA-65
signatures under the exact identities in the dual-signed authenticated
directory entry.

## Downgrade handling

- Clients mark non-negotiable KEM, signature, and constant-rate capabilities
  with `flags & 0x01`; `snnet.suite_list` uses the high bit of its first suite
  byte instead.
- Relays MUST abort the handshake (and emit downgrade telemetry) if they cannot echo a required capability.
- Every first-release NK2/NK3 handshake negotiates a PQ KEM. A missing, unsupported, or mismatched required `snnet.pqkem` echo aborts and raises downgrade telemetry; there is no classical-only compatibility path.
- Absence of `snnet.constant_rate` triggers a downgrade when clients request SNNet‑17A constant-rate transport. Fixture `snnet-cap-006-constant-rate` captures the warning slug and transcript hash; use it for regression in SDK harnesses.

## Change control

1. Propose additions via the SoraNet governance channel with updated fixtures, downgrade tests, and doc changes.
2. Update this registry, the handshake guide (`specs/soranet_handshake.md`), and any SDK manifests in the same PR.
3. Regenerate `iroha_crypto::soranet` fixtures (`cargo xtask soranet-fixtures`) so test vectors include the new identifiers.

Keep this document aligned with implementation changes to avoid mismatched capability negotiations across the network.
