---
title: SoraNet Capability Negotiation Profile
summary: First-release SNNet-1c capability TLVs and downgrade detection.
---

# SoraNet Capability Negotiation Profile

## Deliverables
- Capability TLVs (`snnet.pqkem`, `snnet.pqsig`, GREASE fillers).
- Dual-KDF extraction rules.
- Downgrade alarms and abort logic.
- Interop fixtures for relay/client implementations.

## TLV Format & Registry Process

- **TLV framing.**
  - Each capability record is encoded as `type:u16 BE | length:u16 BE | value`.
    There is no outer flags byte.
  - `snnet.pqkem`, `snnet.pqsig`, and `snnet.constant_rate` put their flags in
    the second byte of their own value; bit `0x01` is the only defined bit.
    `snnet.suite_list` instead uses bit `0x80` of its first suite byte as the
    required marker. Reserved flag bits are rejected.
  - Fixed-width integers inside values use the byte order assigned by the
    registry; padding and constant-rate cell sizes are little-endian.
  - Client TLVs appear inside the transcript in nondecreasing `type` order.
    Only `snnet.pqkem` and `snnet.pqsig` may repeat as algorithm capabilities,
    and each algorithm ID may occur only once. GREASE types may repeat.
  - The complete encoded capability vector is limited to 4,096 bytes.

- **Type registry.**
  - Formal registry maintained in `specs/soranet/capability_registry.md`. Type ranges:
    - `0x0100–0x01FF` — cryptography (PQ KEM, PQ signatures, transcript commits).
    - `0x0200–0x02FF` — transport/padding policies.
    - `0x7F00–0x7FFF` — GREASE / experimental.
  - Requests to allocate new TLVs go through SNNet-1c review; once approved, entries list:
    `type`, `name`, `flags`, `value format`, `mandatory presence`, `version introduced`.
  - The checked registry, Rust parser, and interop fixtures are updated together
    so other SDKs can verify byte-for-byte parity.

- **Existing assignments (v1).**
  | Type    | Name                   | Value format                                      | Required | Notes |
  |---------|------------------------|---------------------------------------------------|----------|-------|
  | 0x0101  | `snnet.pqkem`          | one `kem_id:u8, flags:u8` pair per TLV             | yes      | ML-KEM-768 (`0x01`) is the default required profile. |
  | 0x0102  | `snnet.pqsig`          | one `sig_id:u8, flags:u8` pair per TLV             | yes      | V1 accepts exactly one Dilithium3 (`0x01`) entry. |
  | 0x0103  | `snnet.transcript_commit` | 32-byte hash                                     | yes      | Binds microdescriptor to transcript. |
  | 0x0104  | `snnet.suite_list`     | non-empty ordered unique suite IDs; first-byte `0x80` is required | yes | V1 accepts only `0x04` and `0x05`; unknown, retired, or duplicate IDs reject the entire list. |
  | 0x0201  | `snnet.role`           | nonzero bitfield (`guard=0x01`, `middle=0x02`, `exit=0x04`) | relay only | Reserved bits are rejected; clients omit it. |
  | 0x0202  | `snnet.padding`        | `cell_bytes:u16` little-endian                     | yes      | `00 04` encodes 1024-byte cells. |
  | 0x0203  | `snnet.constant_rate`  | exactly `version:u8, flags:u8, cell_bytes:u16 LE`  | optional | V1 requires version 1 and 1024-byte cells. |

## Example Capability Sets

- **Hybrid (default)**
  ```
  ClientHello TLVs:
    snnet.pqkem             → [ id=0x01, flags=0x01 ]
    snnet.pqkem             → [ id=0x02, flags=0x00 ]
    snnet.pqsig             → [ id=0x01, flags=0x01 ]
    snnet.suite_list        → [ 0x84, 0x05 ]
    snnet.padding           → [ 0x00, 0x04 ]
    GREASE entries          → [ 0x7F19:{arbitrary bytes}, 0x7F42:{arbitrary bytes} ]
  Relay echoes selected algorithms and adds one valid `snnet.role` bitfield.
  ```
- **Classical-only capability sets** are not part of SNNet-16 v1 and MUST be
  rejected. Ed25519 relay identity authentication is separate from the
  transcript signature capability.
- **High-security KEM profile**
  - A policy may require ML-KEM-1024:
  ```
  snnet.pqkem  → [ id=0x02, flags=0x01 ]
  snnet.pqsig  → [ id=0x01, flags=0x01 ]
  snnet.padding → [ 0x00, 0x04 ]
  ```
  Negotiation fails unless the relay advertises that exact required KEM.

## GREASE Behaviour & Parsing

- Clients MUST include at least two GREASE TLVs per handshake, using types from
  `0x7F00–0x7FFF` in the client's canonical type order. Their payloads are
  opaque and there is no GREASE required flag.
- Relays preserve GREASE payloads in transcript processing and echo negotiated
  response entries without assigning semantics to their contents.
- Parsers:
  - Any unknown non-GREASE type → reject the capability vector.
  - Unknown GREASE types → ignore payload, but MUST keep them when computing transcript commitment `T`.
  - Repeated KEM/signature IDs are rejected even if their flags differ;
    duplicated GREASE types remain allowed.
- Telemetry:
  - Emit `soranet_capability_override` metric when required types are missing or when GREASE TLVs are stripped.
  - Downgrade alarms include the offending TLVs for debugging.

These additions complete the capability negotiation plan by specifying the TLV structure, registry mechanics,
reference capability sets, and GREASE handling expectations.
